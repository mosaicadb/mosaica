//! Spill files: the receipt and anchor that make a spill file's bytes trustworthy, and the sorted
//! run a term index or a keyword dictionary spills and merges.
//!
//! Every spill is **fail-closed**: a writer's `finish` returns a [`SpillReceipt`] carrying the
//! record count and a content anchor (a wrapping sum of [`splitmix64`] over each record), and every
//! read path verifies both before its contents are trusted. A truncated, tampered or
//! doubly-appended spill file surfaces as an error, never as a silent partial read. The build's
//! band, member and identity run files and `mosaica_store`'s partition bucket files carry the same
//! receipt.
//!
//! A **sorted run** is written by [`TextRunWriter`] and read back by [`TextRunReader`], one of the
//! sources [`TermMerge`] merges. A run ends in a sparse index of its terms, so that a reader can
//! start at a term: a merge in term ranges ([`crate::merge`]) starts each range's readers there,
//! and checks each run's receipt against what all its ranges decoded of it ([`RunSet`]).
//! [`cascade_sorted_runs`] reduces more runs than one merge may hold open. A malformed run is
//! refused as [`io::ErrorKind::InvalidData`].
//!
//! **What a merge may hold is an [`Allowance`]**, the one rule the build and the service both size
//! a term index's pass by. Its fan-in, and the read buffer each run's cursor gets, are what the
//! allowance pays for, under the share of the process's file descriptors a merge may take
//! ([`merge_descriptors`]). Every cursor on a run reads it at its own offset through one shared
//! descriptor ([`RunFile`]).
//! Every error a run's reader or writer returns names its file, an I/O failure as a
//! [`SpillFileError`], and every refusal the merge makes of its own names its term.

use std::fmt::Display;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use mosaica_filter::{
    common_prefix_len, put_varint, take_varint, SortedDict, SortedDictWriter, VarintError,
};
use mosaica_types::splitmix64;

use crate::merge::{TermMerge, TermRange, TermSource};

/// Buffer size for spill I/O, both directions, and the largest read buffer a merge gives a run's
/// cursor. Four mebibytes: large enough that the syscall cost is noise against the encode/decode
/// work.
pub const SPILL_BUF_BYTES: usize = 4 << 20;

/// What `finish` hands back and every read path verifies against: the file, how many records
/// it holds, and the content anchor over those records.
///
/// The receipt lives in the writer's memory, never on disk: a receipt stored beside the file it
/// vouches for could be tampered with in the same incident, and the process that wrote the spill
/// is the only reader it will ever have.
///
/// The anchor sums mixed values and not raw ones because a plain sum can be *compensated*: replace
/// records `{1, 3}` with `{2, 2}` and count and sum both survive. It defends against torn writes,
/// truncation and doubly-appended files, not against an adversary with write access to the spill
/// directory.
#[derive(Debug, Clone)]
pub struct SpillReceipt {
    pub path: PathBuf,
    /// Records written. Each kind of spill file says what one record is.
    pub count: u64,
    /// Wrapping sum of [`splitmix64`] over a value each record determines, which each kind of spill
    /// file defines.
    pub anchor: u64,
}

/// An I/O failure on one spill file, carried inside the [`io::Error`] the run layer returns so a
/// caller can name the file in its own terms.
#[derive(Debug)]
pub struct SpillFileError {
    pub path: PathBuf,
    pub source: io::Error,
}

impl Display for SpillFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.source)
    }
}

impl std::error::Error for SpillFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// `e`, with the file it concerns named.
fn at(path: &Path, e: io::Error) -> io::Error {
    io::Error::new(
        e.kind(),
        SpillFileError {
            path: path.to_path_buf(),
            source: e,
        },
    )
}

/// A text run at `path` that is not what its writer may write or its reader may read.
fn malformed(path: &Path, detail: impl Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("text run {}: {detail}", path.display()),
    )
}

/// What `reader` holds buffered, refilling it when it holds nothing: empty only at end of file.
/// Retries `Interrupted`, which a refill may see.
fn buffered<R: Read>(reader: &mut BufReader<R>) -> io::Result<&[u8]> {
    while let Err(e) = reader.fill_buf() {
        if e.kind() != io::ErrorKind::Interrupted {
            return Err(e);
        }
    }
    Ok(reader.buffer())
}

/// Whether `reader` is at end of file — a record boundary, which is the only place a spill reader
/// may legitimately be.
pub fn at_end<R: Read>(reader: &mut BufReader<R>) -> io::Result<bool> {
    Ok(buffered(reader)?.is_empty())
}

/// One varint inside a record of a spill, read from `reader` and decoded by `take`
/// ([`take_varint`] or [`mosaica_filter::take_varint64`]). A malformed varint is refused as
/// [`io::ErrorKind::InvalidData`] with the malformation alone for its message, for the caller to
/// name the file.
///
/// The varint is decoded from the bytes the reader holds where it is whole among them, and
/// gathered a byte at a time where it crosses the buffer's end.
pub fn read_varint<R: Read, T>(
    reader: &mut BufReader<R>,
    take: impl Fn(&[u8], &mut usize) -> Result<T, VarintError>,
) -> io::Result<T> {
    let mut cursor = 0;
    let mut taken = take(buffered(reader)?, &mut cursor);
    if matches!(taken, Err(VarintError::Truncated)) {
        let mut bytes = [0u8; 10];
        let mut len = 0;
        while len < bytes.len() {
            let Some(&byte) = buffered(reader)?.first() else {
                break;
            };
            reader.consume(1);
            bytes[len] = byte;
            len += 1;
            if byte & 0x80 == 0 {
                break;
            }
        }
        taken = take(&bytes[..len], &mut 0);
    } else {
        reader.consume(cursor);
    }
    let detail = match taken {
        Ok(value) => return Ok(value),
        Err(VarintError::Truncated) => "truncated mid-record".to_string(),
        Err(e) => format!("a record {e}"),
    };
    Err(io::Error::new(io::ErrorKind::InvalidData, detail))
}

// --------------------------------------------------------------------------------------------
// Text-index run files
// --------------------------------------------------------------------------------------------

/// FNV-1a over a term's bytes — the fingerprint a run file's anchor mixes in place of the `u32`
/// term id the band files have.
///
/// A text run's records are keyed by the term *string*: there is no term id yet, because assigning
/// one is what the merge these files feed does. So the anchor cannot use a band file's packed pair,
/// and it folds the key's bytes to 64 bits instead. Not cryptographic, for the reason
/// [`SpillReceipt`] gives.
pub(crate) fn fingerprint(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Append one entity of a term to `out` in the run format, with the term's `positions` in it, and
/// return what the pair adds to the receipt's anchor.
///
/// `gap` is the entity's distance from the term's previous entity, or the entity itself for the
/// term's first, and `mark` is the term's [`fingerprint`]. The positions are what `payload` holds:
/// none in a [`RunPayload::Rows`] run, and some, strictly ascending, in a
/// [`RunPayload::Positions`] one.
pub(crate) fn encode_entity(
    out: &mut Vec<u8>,
    payload: RunPayload,
    mark: u64,
    gap: u32,
    entity: u32,
    positions: &[u32],
) -> u64 {
    put_varint(gap, out);
    let mut anchor = splitmix64(mark ^ entity as u64);
    if payload == RunPayload::Positions {
        put_varint(positions.len() as u32, out);
        let mut previous = 0;
        for &position in positions {
            put_varint(position - previous, out);
            previous = position;
            anchor = anchor.wrapping_add(splitmix64(
                mark ^ entity as u64 ^ ((position as u64 + 1) << 32),
            ));
        }
    }
    anchor
}

/// Appends one **text run**: a sorted, distinct term stream with each term's ascending entity
/// list, spilled by a text index's [`crate::TermAccumulator`] and consumed once by its merge.
///
/// **The keyword column's dictionary pass spills the same file**, with a key for a term and a row
/// for an entity: the two passes differ in what they merge the stream into and in nothing this
/// writer sees.
///
/// **On disk:** no header, then one record per term —
/// `varint(shared) ‖ varint(suffix_len) ‖ suffix ‖ varint(count) ‖ varint(entity₀) ‖
/// varint(entityᵢ − entityᵢ₋₁)…`. A text index's run ([`RunPayload::Positions`]) follows each
/// entity with `varint(occurrences) ‖ varint(position₀) ‖ varint(positionⱼ − positionⱼ₋₁)…`, the
/// term's positions in that item. `encode_entity` writes each entity. `shared` is the term's
/// common prefix with its predecessor *in this file*, which is the same front coding the dictionary
/// itself uses and for the same reason: a sorted term stream repeats its prefixes, and the corpus
/// that makes this file large is exactly the one whose terms share them. Integrity is external, via
/// the [`SpillReceipt`].
///
/// **After the records, a sparse index**, so that a reader can start at a term rather than at the
/// file's start: the first record at or after every [`RUN_INDEX_STRIDE`] bytes is written with
/// nothing shared, and its term is a key of a [`SortedDict`] written after the records, whose
/// ordinal names that record's offset in a `u64` array after it. A trailer ends the file:
/// `records_len u64 ‖ index_len u64 ‖ "MSRI"`. [`RunFile`] reads it.
///
/// **The entity delta is per TERM, not per file**, which is where this differs from a band file. A
/// band file's records ascend in entity across the whole file because its emitter walks items in
/// assignment order; a run's do not — the run is term-major, so entities restart at each term's
/// own first. Both are refused at the write site rather than decoded into a wrong posting later:
/// terms must ascend strictly across the file, entities strictly within a record.
pub struct TextRunWriter {
    path: PathBuf,
    payload: RunPayload,
    writer: BufWriter<File>,
    record: Vec<u8>,
    prev: Vec<u8>,
    terms: u64,
    /// Entities still owed for the open term. A record's count is written before its entities, so
    /// a term is only complete when every one it promised has arrived — and `finish` refuses a
    /// file whose last record never was.
    owed: u32,
    /// The last entity written for the open term, for the delta.
    last_entity: u32,
    started: bool,
    /// Records written: `(term, entity)` pairs, so a truncation anywhere in a record shows up.
    count: u64,
    anchor: u64,
    /// [`fingerprint`] of the open term, folded once per record rather than once per pair.
    mark: u64,
    /// Bytes of records written, the index's terms and their offsets, and the offset at or after
    /// which the next record is one of them.
    written: u64,
    index: SortedDictWriter<Vec<u8>>,
    offsets: Vec<u64>,
    next_point: u64,
}

/// Bytes of a run between the records its sparse index names: what a reader that starts at a
/// term decodes at most before it reaches the term.
pub const RUN_INDEX_STRIDE: u64 = 8 << 10;

/// The last four bytes of a run.
const RUN_MAGIC: [u8; 4] = *b"MSRI";

/// The bytes of a run's trailer: the records' and the index's lengths, and [`RUN_MAGIC`].
const RUN_TRAILER: u64 = 20;

/// What a run carries for each entity of a term: nothing more, as the keyword column's runs, or the
/// term's positions in it, as the text index's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunPayload {
    Rows,
    Positions,
}

impl TextRunWriter {
    pub fn create(path: &Path, payload: RunPayload) -> io::Result<TextRunWriter> {
        TextRunWriter::buffered(path, payload, SPILL_BUF_BYTES)
    }

    /// A writer whose buffer is `buffer` bytes, for a pass that charges it to its allowance.
    pub(crate) fn buffered(
        path: &Path,
        payload: RunPayload,
        buffer: usize,
    ) -> io::Result<TextRunWriter> {
        let file = File::create(path).map_err(|e| at(path, e))?;
        Ok(TextRunWriter {
            path: path.to_path_buf(),
            payload,
            writer: BufWriter::with_capacity(buffer, file),
            record: Vec::new(),
            prev: Vec::new(),
            terms: 0,
            owed: 0,
            last_entity: 0,
            started: false,
            count: 0,
            anchor: 0,
            mark: 0,
            written: 0,
            index: SortedDictWriter::new(Vec::new()).map_err(io::Error::from)?,
            offsets: Vec::new(),
            next_point: 0,
        })
    }

    /// Write `bytes` into the records.
    fn put(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writer
            .write_all(bytes)
            .map_err(|e| at(&self.path, e))?;
        self.written += bytes.len() as u64;
        Ok(())
    }

    /// Open a record: the term, and how many entities will follow.
    ///
    /// **Split from [`Self::push_entity`] because a merge cannot hold the slice.** A worker
    /// spilling its own accumulator has the entity list in hand; a cascade pass merging sixty-four
    /// runs into one has a *stream*, and the term it is writing may be carried by a quarter of the
    /// corpus. Buffering that to satisfy a slice signature would reintroduce, in the merge, the
    /// residency the merge exists to bound.
    pub fn begin(&mut self, term: &[u8], entities: u32) -> io::Result<()> {
        if self.owed > 0 {
            return Err(malformed(
                &self.path,
                format_args!(
                    "term {:?} opens while {} entities are still owed on {:?}",
                    String::from_utf8_lossy(term),
                    self.owed,
                    String::from_utf8_lossy(&self.prev)
                ),
            ));
        }
        if self.terms > 0 && term <= self.prev.as_slice() {
            return Err(malformed(
                &self.path,
                format_args!(
                    "terms must ascend strictly: {:?} follows {:?}",
                    String::from_utf8_lossy(term),
                    String::from_utf8_lossy(&self.prev)
                ),
            ));
        }
        if term.is_empty() {
            return Err(malformed(&self.path, "the empty string is not a term"));
        }
        if entities == 0 {
            return Err(malformed(
                &self.path,
                format_args!(
                    "term {:?} carries no entity — a term exists in this file only because \
                     something carried it",
                    String::from_utf8_lossy(term)
                ),
            ));
        }
        let point = self.written >= self.next_point;
        let shared = match point {
            true => {
                let key = std::str::from_utf8(term).map_err(|e| {
                    malformed(
                        &self.path,
                        format_args!("term {term:?} is not UTF-8 ({e}); a run's terms are text"),
                    )
                })?;
                self.index.push(key).map_err(io::Error::from)?;
                self.offsets.push(self.written);
                self.next_point = self.written + RUN_INDEX_STRIDE;
                0
            }
            false => common_prefix_len(&self.prev, term),
        };
        let mut record = std::mem::take(&mut self.record);
        record.clear();
        put_varint(shared as u32, &mut record);
        put_varint((term.len() - shared) as u32, &mut record);
        record.extend_from_slice(&term[shared..]);
        put_varint(entities, &mut record);
        let written = self.put(&record);
        self.record = record;
        written?;

        self.mark = fingerprint(term);
        self.prev.clear();
        self.prev.extend_from_slice(term);
        self.terms += 1;
        self.owed = entities;
        self.started = false;
        self.last_entity = 0;
        Ok(())
    }

    /// Append one entity to the open record, with the term's positions in it for a
    /// [`RunPayload::Positions`] run and none for a [`RunPayload::Rows`] one. Entities must ascend
    /// strictly within a term, and positions within an entity — refused here, at the write site,
    /// rather than decoded into a wrong posting a stage later.
    pub fn push_entity(&mut self, entity: u32, positions: &[u32]) -> io::Result<()> {
        if self.owed == 0 {
            return Err(malformed(
                &self.path,
                format_args!("entity {entity} arrives with no open term"),
            ));
        }
        let delta = if self.started {
            entity
                .checked_sub(self.last_entity)
                .filter(|d| *d > 0)
                .ok_or_else(|| {
                    malformed(
                        &self.path,
                        format_args!(
                            "term {:?}'s entities must ascend strictly: {entity} follows {}",
                            String::from_utf8_lossy(&self.prev),
                            self.last_entity
                        ),
                    )
                })?
        } else {
            entity
        };
        let positioned = match self.payload {
            RunPayload::Rows => positions.is_empty(),
            RunPayload::Positions => !positions.is_empty() && positions.is_sorted_by(|a, b| a < b),
        };
        if !positioned {
            return Err(malformed(
                &self.path,
                format_args!(
                    "term {:?}'s entity {entity} carries positions {positions:?}, which a {:?} \
                     run cannot hold: none in a run of rows, and some, strictly ascending, in a \
                     run of positions",
                    String::from_utf8_lossy(&self.prev),
                    self.payload
                ),
            ));
        }
        let mut record = std::mem::take(&mut self.record);
        record.clear();
        let anchor = encode_entity(
            &mut record,
            self.payload,
            self.mark,
            delta,
            entity,
            positions,
        );
        self.last_entity = entity;
        self.started = true;
        let written = self.put(&record);
        self.record = record;
        written?;
        self.wrote(1, anchor);
        Ok(())
    }

    /// Write a whole record: `term`, and its `count` entities already encoded by
    /// [`encode_entity`], the first with its entity for its gap, and `anchor` the sum of what each
    /// call returned. `body` hands the encoded bytes, in order and in as many pieces as it likes,
    /// to the function it is given.
    ///
    /// **The body is trusted**: it is written as given, and nothing here decodes it. A body that
    /// does not hold `count` well-formed entities ascending, or whose anchor is not `anchor`, is
    /// refused by the run's reader when it checks the receipt.
    pub(crate) fn push_body(
        &mut self,
        term: &[u8],
        count: u32,
        anchor: u64,
        body: impl FnOnce(&mut dyn FnMut(&[u8]) -> io::Result<()>) -> io::Result<()>,
    ) -> io::Result<()> {
        self.begin(term, count)?;
        body(&mut |piece| self.put(piece))?;
        self.wrote(count, anchor);
        Ok(())
    }

    /// Count `pairs` of the open term's entities as written, and fold their `anchor` into the
    /// receipt.
    fn wrote(&mut self, pairs: u32, anchor: u64) {
        self.owed -= pairs;
        self.count += pairs as u64;
        self.anchor = self.anchor.wrapping_add(anchor);
    }

    /// Write the index and the trailer, flush, fsync, and hand back the receipt a run's reader
    /// must be given.
    pub fn finish(mut self) -> io::Result<SpillReceipt> {
        if self.owed > 0 {
            return Err(malformed(
                &self.path,
                format_args!(
                    "{} entities are still owed on term {:?}",
                    self.owed,
                    String::from_utf8_lossy(&self.prev)
                ),
            ));
        }
        let records = self.written;
        let index = std::mem::replace(
            &mut self.index,
            SortedDictWriter::new(Vec::new()).map_err(io::Error::from)?,
        );
        let (mut tail, stats) = index.into_output().map_err(io::Error::from)?;
        let index_len = stats.bytes;
        for offset in &self.offsets {
            tail.extend_from_slice(&offset.to_le_bytes());
        }
        tail.extend_from_slice(&records.to_le_bytes());
        tail.extend_from_slice(&index_len.to_le_bytes());
        tail.extend_from_slice(&RUN_MAGIC);
        self.put(&tail)?;
        let TextRunWriter {
            path,
            writer,
            count,
            anchor,
            ..
        } = self;
        let file = writer.into_inner().map_err(|e| at(&path, e.into_error()))?;
        file.sync_all().map_err(|e| at(&path, e))?;
        Ok(SpillReceipt {
            path,
            count,
            anchor,
        })
    }
}

/// A run opened for its readers: its file, which every reader on it reads at its own offset
/// (`pread`) so that all of them share one descriptor, its receipt, and its sparse index, mapped
/// rather than read.
pub struct RunFile {
    path: PathBuf,
    file: File,
    count: u64,
    anchor: u64,
    /// Bytes of records, which the index follows.
    records: u64,
    index: SortedDict,
    /// Where the index's offsets start in the file.
    offsets_at: u64,
}

/// One key of a source, and the bytes from it to the source's next sample: what a merge divides
/// its term space by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sample {
    pub key: Vec<u8>,
    pub bytes: u64,
}

impl RunFile {
    /// Open the run `receipt` names, checking that its trailer, index and length agree.
    pub fn open(receipt: &SpillReceipt) -> io::Result<RunFile> {
        let path = &receipt.path;
        // Writable as well, so that its spans can be given back as they are read.
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| at(path, e))?;
        let len = file.metadata().map_err(|e| at(path, e))?.len();
        let mut trailer = [0u8; RUN_TRAILER as usize];
        if len < RUN_TRAILER {
            return Err(malformed(path, format_args!("{len} bytes hold no trailer")));
        }
        read_at(&file, &mut trailer, len - RUN_TRAILER).map_err(|e| at(path, e))?;
        if trailer[16..] != RUN_MAGIC {
            return Err(malformed(path, "the trailer does not end the file"));
        }
        let records = u64::from_le_bytes(trailer[..8].try_into().expect("eight bytes"));
        let index_len = u64::from_le_bytes(trailer[8..16].try_into().expect("eight bytes"));
        let index_end = records
            .checked_add(index_len)
            .filter(|end| *end <= len - RUN_TRAILER)
            .ok_or_else(|| {
                malformed(
                    path,
                    format_args!(
                        "{len} bytes cannot hold {records} bytes of records and a {index_len}-byte \
                         index"
                    ),
                )
            })?;
        let index = SortedDict::map_within(&file, records, index_len)
            .map_err(|e| malformed(path, format_args!("its index: {e}")))?;
        if index_end + 8 * u64::from(index.len()) + RUN_TRAILER != len {
            return Err(malformed(
                path,
                format_args!(
                    "{len} bytes where {records} of records, a {index_len}-byte index and {} \
                     offsets make {}",
                    index.len(),
                    index_end + 8 * u64::from(index.len()) + RUN_TRAILER
                ),
            ));
        }
        Ok(RunFile {
            path: path.clone(),
            file,
            count: receipt.count,
            anchor: receipt.anchor,
            records,
            index,
            offsets_at: index_end,
        })
    }

    /// The bytes its records take.
    pub fn records(&self) -> u64 {
        self.records
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The offset of the record index entry `ordinal` names.
    fn offset(&self, ordinal: u32) -> io::Result<u64> {
        let mut bytes = [0u8; 8];
        read_at(
            &self.file,
            &mut bytes,
            self.offsets_at + 8 * u64::from(ordinal),
        )
        .map_err(|e| at(&self.path, e))?;
        let offset = u64::from_le_bytes(bytes);
        if offset >= self.records {
            return Err(malformed(
                &self.path,
                format_args!(
                    "index entry {ordinal} names offset {offset}, past the {} bytes of records",
                    self.records
                ),
            ));
        }
        Ok(offset)
    }

    /// Where a reader starting at `key` begins: the last record the index names whose term is at
    /// or before `key`, with that term, or the file's start where there is none.
    pub fn start_of(&self, key: &[u8]) -> io::Result<(u64, Option<Vec<u8>>)> {
        let index_error = |e: mosaica_filter::DictError| malformed(&self.path, e);
        let rank = self.index.rank(key).map_err(index_error)?;
        let mut scratch = Vec::new();
        let at = match rank < self.index.len()
            && self
                .index
                .key_of(rank, &mut scratch)
                .map_err(index_error)?
                .as_bytes()
                == key
        {
            true => rank,
            false if rank == 0 => return Ok((0, None)),
            false => rank - 1,
        };
        let term = self.index.key_of(at, &mut scratch).map_err(index_error)?;
        Ok((self.offset(at)?, Some(term.as_bytes().to_vec())))
    }

    /// About `samples` of its index's terms, evenly spaced, each with the bytes of records from it
    /// to the next.
    pub fn samples(&self, samples: usize) -> io::Result<Vec<Sample>> {
        let n = self.index.len() as usize;
        let mut scratch = Vec::new();
        let mut taken: Vec<(Vec<u8>, u64)> = Vec::new();
        for i in 0..samples.min(n) {
            let ordinal = (i * n / samples.min(n)) as u32;
            let key = self
                .index
                .key_of(ordinal, &mut scratch)
                .map_err(|e| malformed(&self.path, e))?;
            taken.push((key.as_bytes().to_vec(), self.offset(ordinal)?));
        }
        let ends: Vec<u64> = taken
            .iter()
            .skip(1)
            .map(|(_, offset)| *offset)
            .chain([self.records])
            .collect();
        Ok(taken
            .into_iter()
            .zip(ends)
            .map(|((key, offset), end)| Sample {
                key,
                bytes: end.saturating_sub(offset),
            })
            .collect())
    }

    /// Check what the readers that covered every record between them decoded, summed, against
    /// the run's receipt.
    pub fn check(&self, pairs: u64, anchor: u64) -> io::Result<()> {
        check_receipt(&self.path, (pairs, anchor), (self.count, self.anchor))
    }

    /// Give back the disk the records before `offset` take, where the file system can.
    pub fn release_before(&self, offset: u64) {
        let offset = offset.min(self.records) / RELEASE_ALIGN * RELEASE_ALIGN;
        if offset > 0 {
            mosaica_types::process::release_span(&self.file, 0, offset);
        }
    }
}

/// The runs a merge in term ranges reads: each opened once, where each range starts in each, and
/// what the ranges have decoded of each so far.
pub struct RunSet {
    files: Vec<RunFile>,
    /// Per range, the offset in each run its readers start from.
    starts: Vec<Vec<u64>>,
    decoded: Mutex<Vec<(u64, u64)>>,
}

impl RunSet {
    pub fn open(receipts: &[SpillReceipt]) -> io::Result<RunSet> {
        Ok(RunSet {
            files: receipts
                .iter()
                .map(RunFile::open)
                .collect::<io::Result<_>>()?,
            starts: Vec::new(),
            decoded: Mutex::new(vec![(0, 0); receipts.len()]),
        })
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// The bytes their records take.
    pub fn records(&self) -> u64 {
        self.files.iter().map(RunFile::records).sum()
    }

    /// `each` keys sampled from each run's index.
    pub fn samples(&self, each: usize) -> io::Result<Vec<Sample>> {
        let mut samples = Vec::new();
        for file in &self.files {
            samples.extend(file.samples(each)?);
        }
        Ok(samples)
    }

    /// Find where each of `ranges` starts in each run, from the runs' indexes.
    pub fn place(&mut self, ranges: &[TermRange]) -> io::Result<()> {
        self.starts = ranges
            .iter()
            .map(|range| {
                self.files
                    .iter()
                    .map(|file| match &range.lo {
                        Some(lo) => file.start_of(lo).map(|(offset, _)| offset),
                        None => Ok(0),
                    })
                    .collect()
            })
            .collect::<io::Result<_>>()?;
        Ok(())
    }

    /// A reader of each run, in order, from its start.
    pub fn readers(
        &self,
        payload: RunPayload,
        buffer: usize,
    ) -> impl Iterator<Item = TextRunReader<'_>> {
        self.files
            .iter()
            .map(move |file| TextRunReader::open(file, payload, buffer))
    }

    /// Count what `readers`, one a run in order, decoded of one range.
    pub fn tally<'a>(&self, readers: impl Iterator<Item = &'a TextRunReader<'a>>) {
        let mut decoded = self.decoded.lock().unwrap_or_else(PoisonError::into_inner);
        for (sum, reader) in decoded.iter_mut().zip(readers) {
            let (pairs, anchor) = reader.share();
            *sum = (sum.0 + pairs, sum.1.wrapping_add(anchor));
        }
    }

    /// Range `range` and every range before it are written: give back what no later range reads.
    pub fn written(&self, range: usize) {
        if let Some(next) = self.starts.get(range + 1) {
            for (file, &offset) in self.files.iter().zip(next) {
                file.release_before(offset);
            }
        }
    }

    /// Check what the ranges decoded of each run against its receipt.
    pub fn check(self) -> io::Result<()> {
        let decoded = self
            .decoded
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        for (file, (pairs, anchor)) in self.files.iter().zip(decoded) {
            file.check(pairs, anchor)?;
        }
        Ok(())
    }
}

/// What a span of a run is given back in multiples of: a file system's block, or a multiple of it.
const RELEASE_ALIGN: u64 = 64 << 10;

/// `buf.len()` bytes of `file` at `offset`.
fn read_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<()> {
    #[cfg(unix)]
    return std::os::unix::fs::FileExt::read_exact_at(file, buf, offset);
    #[cfg(windows)]
    {
        let mut done = 0;
        while done < buf.len() {
            match std::os::windows::fs::FileExt::seek_read(
                file,
                &mut buf[done..],
                offset + done as u64,
            )? {
                0 => return Err(io::ErrorKind::UnexpectedEof.into()),
                n => done += n,
            }
        }
        Ok(())
    }
}

/// Refuse a run whose decoded pairs and anchor are not its receipt's.
fn check_receipt(
    path: &Path,
    (pairs, anchor): (u64, u64),
    (count, expect): (u64, u64),
) -> io::Result<()> {
    if pairs != count {
        return Err(malformed(
            path,
            format_args!("pair count mismatch: decoded {pairs} pairs but the receipt says {count}"),
        ));
    }
    if anchor != expect {
        return Err(malformed(
            path,
            format_args!(
                "content anchor mismatch: recomputed {anchor:#018x} but the receipt says \
                 {expect:#018x}: the file's bytes are not the bytes that were written"
            ),
        ));
    }
    Ok(())
}

/// Streams a text run back **term by term**, as a [`TermSource`], from its start or from a term.
///
/// **The head's entities are not decoded until they are asked for.** A k-way merge holds one open
/// reader per run and compares only their head terms, so a reader that eagerly materialised each
/// head's entity list would put *k* whole posting lists in memory to choose between *k* strings,
/// which is the residency this whole pass exists to bound. [`TermSource::next_term`] reads the term
/// and its length and stops; [`TermSource::next_entity`] decodes them, and `next_term` drains
/// whatever a caller left rather than trusting it to have consumed the record.
///
/// A reader that read the whole run checks count and anchor against the receipt as it reaches the
/// end, since a running sum can only be checked once the stream ends. A reader over a range of the
/// run's terms decodes only that range, and its [`Self::share`] is what [`RunFile::check`] sums.
pub struct TextRunReader<'r> {
    run: &'r RunFile,
    payload: RunPayload,
    reader: BufReader<RunCursor<'r>>,
    /// The term's positions in the entity [`Self::next_entity`] last returned.
    positions: Vec<u32>,
    /// The head term, front-decoded against its predecessor.
    term: Vec<u8>,
    /// [`fingerprint`] of `term`, held so the anchor costs one fold per record and not one per
    /// pair.
    mark: u64,
    /// Entities of the head not yet decoded.
    pending: u32,
    /// Entities of the head already decoded — what makes the first delta absolute and the rest
    /// relative, and it cannot be inferred from `last_entity` because entity 0 is a legal first.
    taken: u32,
    /// The last entity decoded within the head, for the delta.
    last_entity: u32,
    pairs: u64,
    anchor: u64,
    /// Whether the reader started at the run's first term, so that at its end it has decoded all of
    /// it; whether the head was read by [`TermSource::seek`] and is yet to be yielded; whether the
    /// records are exhausted.
    whole: bool,
    peeked: bool,
    done: bool,
}

/// A cursor's place in a run's records, read at its own offset (`pread`), so every cursor on one
/// run shares the run's one descriptor. It reads nothing past the records.
struct RunCursor<'r> {
    file: &'r File,
    at: u64,
    end: u64,
}

impl Read for RunCursor<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let room = usize::try_from(self.end - self.at).unwrap_or(usize::MAX);
        let len = room.min(buf.len());
        let buf = &mut buf[..len];
        if buf.is_empty() {
            return Ok(0);
        }
        #[cfg(unix)]
        let read = std::os::unix::fs::FileExt::read_at(self.file, buf, self.at)?;
        #[cfg(windows)]
        let read = std::os::windows::fs::FileExt::seek_read(self.file, buf, self.at)?;
        self.at += read as u64;
        Ok(read)
    }
}

impl TermSource for TextRunReader<'_> {
    fn next_term(&mut self) -> io::Result<bool> {
        if self.peeked {
            self.peeked = false;
            return Ok(true);
        }
        if self.done {
            return Ok(false);
        }
        // Whatever the caller did not take: decoded and anchored, never skipped by seeking, so a
        // malformation inside a record the merge had no use for is still caught.
        while self.pending > 0 {
            self.decode_entity()?;
        }
        match self.read_head()? {
            Some(_) => Ok(true),
            None => {
                if self.whole {
                    check_receipt(
                        &self.run.path,
                        (self.pairs, self.anchor),
                        (self.run.count, self.run.anchor),
                    )?;
                }
                Ok(false)
            }
        }
    }

    /// From the last record the index names at or before `key`, the records before `key` are
    /// decoded, so that a damaged one is still refused, and not counted in [`Self::share`]: the
    /// reader of the range before this one counts them.
    fn seek(&mut self, key: &[u8]) -> io::Result<()> {
        if self.pairs > 0 || !self.term.is_empty() || self.peeked {
            return Err(self.malformed("a reader seeks before its first term"));
        }
        let (offset, mut named) = self.run.start_of(key)?;
        self.reader.get_mut().at = offset;
        self.whole = false;
        let counted = (self.pairs, self.anchor);
        while let Some(shared) = self.read_head()? {
            if let Some(named) = named.take() {
                if shared != 0 || self.term != named {
                    return Err(self.malformed(&format!(
                        "the index names {:?} at offset {offset}, where the record is {:?}",
                        String::from_utf8_lossy(&named),
                        String::from_utf8_lossy(&self.term)
                    )));
                }
            }
            if self.term.as_slice() >= key {
                self.peeked = true;
                break;
            }
            while self.pending > 0 {
                self.decode_entity()?;
            }
        }
        (self.pairs, self.anchor) = counted;
        Ok(())
    }

    fn term(&self) -> &[u8] {
        &self.term
    }

    fn count(&self) -> u64 {
        u64::from(self.taken) + u64::from(self.pending)
    }

    /// One at a time, because the runs do not partition entity space: a run's entities ascend
    /// within its own record, but the build divides the arena and not entity space, so a term's
    /// merged list is produced by comparing the runs' heads.
    fn next_entity(&mut self) -> io::Result<Option<u32>> {
        if self.pending == 0 {
            return Ok(None);
        }
        self.decode_entity().map(Some)
    }

    fn positions(&self) -> &[u32] {
        &self.positions
    }
}

impl<'r> TextRunReader<'r> {
    /// A reader of `run`, from its start, with a read buffer of `buffer` bytes. Several readers
    /// may read one run at once.
    pub fn open(run: &'r RunFile, payload: RunPayload, buffer: usize) -> TextRunReader<'r> {
        TextRunReader {
            run,
            payload,
            positions: Vec::new(),
            reader: BufReader::with_capacity(
                buffer,
                RunCursor {
                    file: &run.file,
                    at: 0,
                    end: run.records,
                },
            ),
            term: Vec::new(),
            mark: 0,
            pending: 0,
            taken: 0,
            last_entity: 0,
            pairs: 0,
            anchor: 0,
            whole: true,
            peeked: false,
            done: false,
        }
    }

    /// The pairs this reader decoded and what they add to the anchor, for [`RunFile::check`].
    pub fn share(&self) -> (u64, u64) {
        (self.pairs, self.anchor)
    }

    /// Read the next record's term and count, returning its shared prefix, or `None` at the end of
    /// the records.
    fn read_head(&mut self) -> io::Result<Option<usize>> {
        if at_end(&mut self.reader).map_err(|e| at(&self.run.path, e))? {
            self.done = true;
            return Ok(None);
        }
        let shared = self.next_u32()? as usize;
        if shared > self.term.len() {
            return Err(self.malformed(&format!(
                "shared prefix {shared} exceeds the previous term's {} bytes",
                self.term.len()
            )));
        }
        let suffix_len = self.next_u32()? as usize;
        self.term.truncate(shared);
        // Read through `take` and grow rather than `resize`-then-`read_exact`: `suffix_len` comes
        // off the file, and a corrupted one would otherwise be an allocation of whatever it says
        // before a single byte is checked against what the file actually holds.
        let got = (&mut self.reader)
            .take(suffix_len as u64)
            .read_to_end(&mut self.term)
            .map_err(|e| at(&self.run.path, e))?;
        if got != suffix_len {
            return Err(self.malformed("truncated inside a term's bytes"));
        }
        if self.term.is_empty() {
            return Err(self.malformed("the empty string is not a term"));
        }
        // Refused rather than decoded: the writer refuses a zero-entity record, so one here is a
        // malformed file and not a term that lost its postings.
        let count = self.next_u32()?;
        if count == 0 {
            return Err(self.malformed("a term with no entities"));
        }
        if self.pairs + count as u64 > self.run.count {
            return Err(self.malformed(&format!(
                "trailing data: more pairs than the receipt's count {}",
                self.run.count
            )));
        }
        self.mark = fingerprint(&self.term);
        self.pending = count;
        self.taken = 0;
        self.last_entity = 0;
        Ok(Some(shared))
    }

    fn decode_entity(&mut self) -> io::Result<u32> {
        let delta = self.next_u32()?;
        let entity = if self.taken == 0 {
            delta
        } else {
            if delta == 0 {
                return Err(self.malformed("an entity delta of 0 repeats an entity"));
            }
            let last = self.last_entity;
            last.checked_add(delta).ok_or_else(|| {
                self.malformed(&format!("entity delta {delta} overflows u32 above {last}"))
            })?
        };
        self.last_entity = entity;
        self.taken += 1;
        self.pending -= 1;
        self.pairs += 1;
        self.anchor = self
            .anchor
            .wrapping_add(splitmix64(self.mark ^ entity as u64));
        self.positions.clear();
        if self.payload == RunPayload::Positions {
            let occurrences = self.next_u32()?;
            if occurrences == 0 {
                return Err(self.malformed("an entity holding its term at no position"));
            }
            let mut position = 0u32;
            for j in 0..occurrences {
                let gap = self.next_u32()?;
                if j > 0 && gap == 0 {
                    return Err(self.malformed("a position gap of 0 repeats a position"));
                }
                position = position
                    .checked_add(gap)
                    .ok_or_else(|| self.malformed("a position overflows u32"))?;
                self.positions.push(position);
                self.anchor = self.anchor.wrapping_add(splitmix64(
                    self.mark ^ entity as u64 ^ ((position as u64 + 1) << 32),
                ));
            }
        }
        Ok(entity)
    }

    fn next_u32(&mut self) -> io::Result<u32> {
        read_varint(&mut self.reader, take_varint).map_err(|e| {
            if e.kind() == io::ErrorKind::InvalidData {
                self.malformed(&e.to_string())
            } else {
                at(&self.run.path, e)
            }
        })
    }

    fn malformed(&self, detail: &str) -> io::Error {
        malformed(&self.run.path, detail)
    }
}

// --------------------------------------------------------------------------------------------
// The allowance, and the fan-in it pays for
// --------------------------------------------------------------------------------------------

/// The smallest read buffer a merge gives a run's cursor. A quarter of a mebibyte, at which a
/// refill's system call is still small against decoding what it read.
pub const RUN_BUFFER_MIN: usize = 256 << 10;

/// The least a term index's pass is given for each of its threads: what one accumulator runs in
/// without spilling every few documents.
pub const WORKER_BYTES_MIN: usize = 8 << 20;

/// The most one chunk of a pass holds in memory, an accumulator's or a keyword chunk's: 2 GiB.
/// Their keys, chains and rows are addressed by `u32` offsets and counts, and half of what those
/// address leaves room for a chunk to pass its bound by one document before it spills.
pub const CHUNK_BYTES_MAX: usize = 1 << 31;

/// What one term index's pass may hold, and on how many threads it runs.
///
/// **One rule, for the build and the service.** [`Allowance::new`] takes what is to be had (a
/// build's `--memory-budget`, or a server's share of the memory it may take) less what the rest of
/// the pass's stage holds beside it. Everything the pass holds is charged against the one figure:
/// its chunks while it reads the column, a share each ([`Self::chunk`]), and after them the read
/// buffers of the merge and of any cascade pass before it, and those passes' writers. So a stated
/// budget is not exceeded by a pass that grows to fill it.
///
/// It is never less than [`WORKER_BYTES_MIN`] a thread, which is what the pass runs in. A build
/// charges that floor in its model, so a budget that cannot pay for it is refused before the build
/// starts rather than exceeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Allowance {
    pub bytes: u64,
    pub threads: usize,
}

impl Allowance {
    /// The allowance a pass on `threads` threads gets where `budget` bytes are to be had and the
    /// rest of its stage holds `resident` of them.
    pub fn new(budget: u64, resident: u64, threads: usize) -> Allowance {
        let threads = threads.max(1);
        Allowance {
            bytes: budget.saturating_sub(resident).max(Self::floor(threads)),
            threads,
        }
    }

    /// The least a pass on `threads` threads is given: [`WORKER_BYTES_MIN`] each.
    pub fn floor(threads: usize) -> u64 {
        threads.max(1) as u64 * WORKER_BYTES_MIN as u64
    }

    /// What is left of it once `bytes` of it are held.
    pub fn less(&self, bytes: u64) -> Allowance {
        Allowance {
            bytes: self.bytes.saturating_sub(bytes),
            threads: self.threads,
        }
    }

    /// What each of `chunks` held at once may allocate before it spills: an equal share, and at
    /// most [`CHUNK_BYTES_MAX`].
    pub fn chunk(&self, chunks: usize) -> usize {
        usize::try_from(self.bytes / chunks.max(1) as u64)
            .unwrap_or(usize::MAX)
            .min(CHUNK_BYTES_MAX)
    }

    /// What each of the pass's accumulators may allocate before it spills, one a thread.
    pub fn per_worker(&self) -> usize {
        self.chunk(self.threads)
    }

    /// **The fan-in: the most runs one merge opens**, F = M / B with each cursor at
    /// [`RUN_BUFFER_MIN`]. Held under [`merge_descriptors`], since every cursor on one run shares
    /// its descriptor and the runs are what is counted, and three at least, so that a cascade pass
    /// reduces two runs into one with its writer beside them.
    ///
    /// One level over R runs fits while R is at most F. R is the run bytes over a run's size, and a
    /// run's size follows the allowance over the threads, so one level holds a corpus whose run
    /// bytes grow as the square of the allowance. Above that the cascade takes a pass.
    pub fn fan_in(&self) -> usize {
        usize::try_from(self.bytes / RUN_BUFFER_MIN as u64)
            .unwrap_or(usize::MAX)
            .min(merge_descriptors())
            .max(3)
    }

    /// The read buffer each of `cursors` open at once is given: an equal share of the allowance,
    /// from [`RUN_BUFFER_MIN`] to [`SPILL_BUF_BYTES`].
    pub fn buffer(&self, cursors: usize) -> usize {
        usize::try_from(self.bytes / cursors.max(1) as u64)
            .unwrap_or(usize::MAX)
            .clamp(RUN_BUFFER_MIN, SPILL_BUF_BYTES)
    }

    /// The cascade passes [`cascade_sorted_runs`] takes over `runs` runs under this allowance.
    pub fn cascade_passes(&self, mut runs: usize) -> usize {
        let mut passes = 0;
        while runs > self.fan_in() {
            let pass = CascadePass::of(runs, self.fan_in(), self.threads);
            runs -= pass.taken - pass.groups;
            passes += 1;
        }
        passes
    }
}

/// The share of this process's file descriptors one merge's runs may take: half the soft limit.
/// The other half is what the process holds open beside the merge (the source files, the files
/// being written, a server's sockets), none of which scales with the run count, so half is headroom
/// rather than an estimate.
pub fn merge_descriptors() -> usize {
    usize::try_from(mosaica_types::process::descriptor_limit() / 2).unwrap_or(usize::MAX)
}

/// What one cascade pass merges: the leading `taken` runs, in `groups` groups of as near one width
/// as they divide into, each group into one run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CascadePass {
    taken: usize,
    groups: usize,
}

impl CascadePass {
    /// **The fewest leading runs that bring `runs` to `fan_in`**, at a width that lets `threads`
    /// groups be in flight at once. Merging g runs into one takes g − 1 off the count, so k groups
    /// over the excess E = R − F take E + k runs. The groups are at least `threads`, where the
    /// excess allows, so that they run at once; then the width that fits `threads` groups and
    /// their writers in the fan-in; then the widest a group can be, F − 1, which serves one group
    /// at a time. Where even that width cannot reach the fan-in in one pass, every run is merged in
    /// groups of F − 1, and the next pass takes what is left.
    fn of(runs: usize, fan_in: usize, threads: usize) -> CascadePass {
        let excess = runs - fan_in;
        let at = |width: usize| {
            let groups = excess.div_ceil(width - 1).max(threads.min(excess));
            (excess + groups <= runs).then_some(CascadePass {
                taken: excess + groups,
                groups,
            })
        };
        let parallel = (fan_in / threads.max(1))
            .saturating_sub(1)
            .clamp(2, fan_in - 1);
        at(parallel)
            .or_else(|| at(fan_in - 1))
            .unwrap_or(CascadePass {
                taken: runs,
                groups: runs.div_ceil(fan_in - 1),
            })
    }

    /// The groups' runs, in order, the first `taken % groups` one wider than the rest.
    fn split<T>(self, runs: &[T]) -> Vec<&[T]> {
        let (width, wider) = (self.taken / self.groups, self.taken % self.groups);
        let mut groups = Vec::with_capacity(self.groups);
        let mut at = 0;
        for group in 0..self.groups {
            let end = at + width + usize::from(group < wider);
            groups.push(&runs[at..end]);
            at = end;
        }
        groups
    }
}

/// Reduce `receipts` to at most the allowance's fan-in, deleting each pass's inputs as it goes, and
/// return what is left and the passes taken. `family` names the intermediate files, so two column
/// families cascading in one directory cannot collide. On an error the cascade removes every file
/// it wrote, and leaves the caller's runs to the caller.
///
/// **A pass merges only as many leading runs as bring the count to the fan-in**, in groups of one
/// run each, several groups at once ([`CascadePass::of`]). As many groups run at once as their
/// cursors' and writers' buffers fit in the allowance, up to its threads, so the groups in flight
/// hold no more than one merge level would. The runs past the groups are left as they are, so a
/// pass rewrites about the excess over the fan-in rather than every run, and a corpus whose runs
/// are under about the square of the fan-in takes one pass. An intermediate run is written by the
/// same writer as an accumulator's, so the cascade adds a pass and not a format.
///
/// Each group's run takes the place of its runs, and each group merges in order, so the entity
/// ordering the final merge relies on survives every pass.
pub fn cascade_sorted_runs(
    column_dir: &Path,
    family: &str,
    payload: RunPayload,
    mut receipts: Vec<SpillReceipt>,
    allowance: Allowance,
) -> io::Result<(Vec<SpillReceipt>, usize)> {
    let fan_in = allowance.fan_in();
    let mut written: Vec<PathBuf> = Vec::new();
    let mut pass = 0usize;
    while receipts.len() > fan_in {
        let plan = CascadePass::of(receipts.len(), fan_in, allowance.threads);
        let widest = plan.taken.div_ceil(plan.groups);
        let in_flight = (fan_in / (widest + 1)).clamp(1, allowance.threads.max(1));
        let buffer = allowance.buffer(in_flight * (widest + 1));
        let path =
            |index: usize| column_dir.join(format!("{family}-merge-{pass:02}-{index:05}.spill"));
        written.extend((0..plan.groups).map(path));
        // Deleted per group rather than per pass: a corpus that reaches the cascade at all is one
        // whose runs are large, and holding a whole pass's inputs beside a whole pass's outputs
        // would double the spill's peak on disk.
        let merge = |(index, runs): (usize, &[SpillReceipt])| {
            merge_group(runs, &path(index), payload, buffer)
                .and_then(|run| remove_runs(runs).map(|()| run))
        };
        // The pages the last pass's threads, or the accumulators before it, freed in their own
        // arenas go back before this pass's buffers are taken, so the two are not held at once.
        mosaica_types::process::trim_heap();
        let groups = plan.split(&receipts[..plan.taken]);
        let merged: io::Result<Vec<SpillReceipt>> = match in_flight {
            1 => groups.iter().copied().enumerate().map(merge).collect(),
            _ => rayon::ThreadPoolBuilder::new()
                .num_threads(in_flight)
                .build()
                .map_err(io::Error::other)
                .and_then(|pool| {
                    use rayon::prelude::*;
                    pool.install(|| groups.par_iter().copied().enumerate().map(merge).collect())
                }),
        };
        match merged {
            Ok(mut merged) => {
                merged.extend(receipts.drain(plan.taken..));
                receipts = merged;
            }
            Err(e) => {
                for path in &written {
                    let _ = std::fs::remove_file(path);
                }
                return Err(e);
            }
        }
        pass += 1;
    }
    Ok((receipts, pass))
}

/// Merge `runs` into one run at `path`, each run and the writer through a buffer of `buffer`
/// bytes.
fn merge_group(
    runs: &[SpillReceipt],
    path: &Path,
    payload: RunPayload,
    buffer: usize,
) -> io::Result<SpillReceipt> {
    let files = runs
        .iter()
        .map(RunFile::open)
        .collect::<io::Result<Vec<_>>>()?;
    let mut merge = TermMerge::open(
        files
            .iter()
            .map(|run| TextRunReader::open(run, payload, buffer))
            .collect(),
    )?;
    let mut writer = TextRunWriter::buffered(path, payload, buffer)?;
    while merge.next_term()? {
        let count = u32::try_from(merge.postings()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "text run merge: term {:?} carries more entities than a u32 can count",
                    String::from_utf8_lossy(merge.term())
                ),
            )
        })?;
        writer.begin(merge.term(), count)?;
        merge.drain(std::convert::identity, &mut |entity, positions| {
            writer.push_entity(entity, positions)
        })?;
    }
    writer.finish()
}

/// Remove every run's file, carrying on past a failure, and return the first failure. A file that
/// is already gone is not one.
pub(crate) fn remove_runs<'a>(runs: impl IntoIterator<Item = &'a SpillReceipt>) -> io::Result<()> {
    let mut first = Ok(());
    for run in runs {
        if let Err(e) = std::fs::remove_file(&run.path) {
            if e.kind() != io::ErrorKind::NotFound && first.is_ok() {
                first = Err(at(&run.path, e));
            }
        }
    }
    first
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::fs;

    fn flip_byte(path: &Path, index: usize) {
        let mut bytes = fs::read(path).unwrap();
        bytes[index] ^= 0x40;
        fs::write(path, bytes).unwrap();
    }

    fn truncate_by(path: &Path, n: usize) {
        let bytes = fs::read(path).unwrap();
        fs::write(path, &bytes[..bytes.len() - n]).unwrap();
    }

    fn append(path: &Path, extra: &[u8]) {
        let mut bytes = fs::read(path).unwrap();
        bytes.extend_from_slice(extra);
        fs::write(path, bytes).unwrap();
    }

    /// The records' length a run's trailer gives.
    fn open_run_len(bytes: &[u8]) -> usize {
        let at = bytes.len() - 20;
        u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap()) as usize
    }

    /// The read or write was refused as malformed, rather than failing for I/O.
    fn assert_refused<T: std::fmt::Debug>(result: io::Result<T>) {
        assert!(
            matches!(&result, Err(e) if e.kind() == io::ErrorKind::InvalidData),
            "{result:?}"
        );
    }

    /// The run `receipt` names, opened.
    fn open_run(receipt: &SpillReceipt) -> RunFile {
        RunFile::open(receipt).unwrap()
    }

    /// A reader of `run` from its start.
    fn open_reader(run: &RunFile, payload: RunPayload) -> TextRunReader<'_> {
        TextRunReader::open(run, payload, RUN_BUFFER_MIN)
    }

    /// One whole record of a run of rows.
    fn push(writer: &mut TextRunWriter, term: &[u8], entities: &[u32]) -> io::Result<()> {
        writer.begin(term, entities.len() as u32)?;
        for &entity in entities {
            writer.push_entity(entity, &[])?;
        }
        Ok(())
    }

    /// **A run of positions gives back each entity's positions** beside it, a repeated position
    /// and a run of rows carrying positions are refused, and the anchor covers the positions: a
    /// file whose positions differ from the receipt's fails at its end.
    #[test]
    fn a_text_run_carries_each_entitys_positions() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("positions.spill");
        let records = [
            (
                b"alpha".as_slice(),
                vec![(3u32, vec![0u32, 7, 200]), (9, vec![4])],
            ),
            (b"beta".as_slice(), vec![(1, vec![1_000_000])]),
        ];
        let mut writer = TextRunWriter::create(&path, RunPayload::Positions).unwrap();
        for (term, items) in &records {
            writer.begin(term, items.len() as u32).unwrap();
            for (entity, positions) in items {
                writer.push_entity(*entity, positions).unwrap();
            }
        }
        let receipt = writer.finish().unwrap();
        let run = open_run(&receipt);
        let mut reader = open_reader(&run, RunPayload::Positions);
        for (term, items) in &records {
            assert!(reader.next_term().unwrap());
            assert_eq!(reader.term(), *term);
            for (entity, positions) in items {
                assert_eq!(reader.next_entity().unwrap(), Some(*entity));
                assert_eq!(reader.positions(), positions.as_slice());
            }
        }
        assert!(!reader.next_term().unwrap());

        // The last byte of the records is the last position's gap: changed, the anchor no longer
        // agrees.
        let last = run.records() as usize - 1;
        drop(run);
        let mut bytes = fs::read(&path).unwrap();
        bytes[last] ^= 1;
        fs::write(&path, &bytes).unwrap();
        let run = open_run(&receipt);
        let mut reader = open_reader(&run, RunPayload::Positions);
        assert!((|| -> io::Result<()> {
            while reader.next_term()? {}
            Ok(())
        })()
        .is_err());

        let mut writer =
            TextRunWriter::create(&temp.path().join("refused.spill"), RunPayload::Positions)
                .unwrap();
        writer.begin(b"gamma", 3).unwrap();
        assert!(writer.push_entity(1, &[]).is_err(), "no position");
        assert!(
            writer.push_entity(1, &[4, 4]).is_err(),
            "a repeated position"
        );
        let mut rows =
            TextRunWriter::create(&temp.path().join("rows.spill"), RunPayload::Rows).unwrap();
        rows.begin(b"delta", 1).unwrap();
        assert!(
            rows.push_entity(1, &[0]).is_err(),
            "positions in a run of rows"
        );
    }

    fn write_text_run(path: &Path, records: &[(&str, Vec<u32>)]) -> SpillReceipt {
        let mut writer = TextRunWriter::create(path, RunPayload::Rows).unwrap();
        for (term, entities) in records {
            push(&mut writer, term.as_bytes(), entities).unwrap();
        }
        writer.finish().unwrap()
    }

    fn read_text_run(receipt: &SpillReceipt) -> io::Result<Vec<(String, Vec<u32>)>> {
        let run = RunFile::open(receipt)?;
        let mut reader = open_reader(&run, RunPayload::Rows);
        let mut out = Vec::new();
        while reader.next_term()? {
            let term = String::from_utf8(reader.term().to_vec()).unwrap();
            let expected = reader.count();
            let mut entities = Vec::new();
            while let Some(entity) = reader.next_entity()? {
                entities.push(entity);
            }
            assert_eq!(
                entities.len() as u64,
                expected,
                "`count` promised the count"
            );
            out.push((term, entities));
        }
        Ok(out)
    }
    #[test]
    fn text_run_round_trips() {
        let temp = tempfile::TempDir::new().unwrap();
        let long: Vec<(&str, Vec<u32>)> = vec![
            // Entity 0 first, which is where an absolute-vs-delta confusion in the first slot
            // would show: a decoder that treated it as a delta from a non-zero register reads a
            // different entity, and every posting after it shifts.
            ("aardvark", vec![0]),
            ("aardvark2", vec![0, 1, 2, 63, 64, 65, u32::MAX]),
            // Shares a long prefix with its predecessor — the front coding's own case.
            ("aardvark2b", vec![7]),
            ("zebra", (0..5_000u32).map(|e| e * 3).collect()),
            ("日本語", vec![1, 2]),
        ];
        let cases: Vec<Vec<(&str, Vec<u32>)>> = vec![vec![], vec![("only", vec![9])], long];
        for (i, records) in cases.iter().enumerate() {
            let path = temp.path().join(format!("text-run-{i}.spill"));
            let receipt = write_text_run(&path, records);
            assert_eq!(
                receipt.count,
                records.iter().map(|(_, e)| e.len() as u64).sum::<u64>()
            );
            let read = read_text_run(&receipt).unwrap();
            let want: Vec<(String, Vec<u32>)> = records
                .iter()
                .map(|(t, e)| (t.to_string(), e.clone()))
                .collect();
            assert_eq!(read, want);
        }
    }

    #[test]
    fn text_run_flip_is_an_anchor_mismatch() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("text-run.spill");
        let records = vec![("alpha", vec![1, 5, 9]), ("bravo", vec![2, 4])];
        let receipt = write_text_run(&path, &records);
        // The last byte of the records is an entity delta: flipping it changes a posting and
        // nothing else, which is exactly the failure a count check alone would miss.
        let last = open_run(&receipt).records() as usize - 1;
        flip_byte(&path, last);
        assert_refused(read_text_run(&receipt));
    }

    #[test]
    fn text_run_truncation_is_caught() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("text-run.spill");
        let records = vec![("alpha", vec![1, 5, 9]), ("bravo", vec![2, 4])];
        for cut in [1usize, 2, 3, 4, 5] {
            let receipt = write_text_run(&path, &records);
            truncate_by(&path, cut);
            assert_refused(read_text_run(&receipt));
        }
    }

    #[test]
    fn text_run_trailing_data_is_caught() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("text-run.spill");
        let records = vec![("alpha", vec![1, 5, 9])];
        let receipt = write_text_run(&path, &records);
        // A whole extra record, well formed: `zz` with one entity, after the trailer, and the
        // same record among the records with the trailer moved to agree. Only the receipt's pair
        // count separates the second from a legitimate stream.
        let whole = fs::read(&path).unwrap();
        append(&path, &[0, 2, b'z', b'z', 1, 3]);
        assert_refused(read_text_run(&receipt));
        let records_len = open_run_len(&whole);
        let mut grown = whole[..records_len].to_vec();
        grown.extend_from_slice(&[0, 2, b'z', b'z', 1, 3]);
        grown.extend_from_slice(&whole[records_len..whole.len() - 20]);
        grown.extend_from_slice(&(records_len as u64 + 6).to_le_bytes());
        grown.extend_from_slice(&whole[whole.len() - 12..]);
        fs::write(&path, grown).unwrap();
        assert_refused(read_text_run(&receipt));
    }

    #[test]
    fn text_run_refuses_an_emitter_that_does_not_ascend() {
        let temp = tempfile::TempDir::new().unwrap();
        // A writer apiece: a refused push may already have written a record's header, so the file
        // one leaves behind is only ever discarded — which is what a build that fails here does.
        let refused: Vec<(&str, &[u8], &[u32])> = vec![
            ("a term that regresses", b"alpha", &[3]),
            ("a term that repeats", b"bravo", &[3]),
            ("a term with no entity", b"charlie", &[]),
            ("an entity that repeats", b"charlie", &[5, 5]),
            ("an entity that regresses", b"charlie", &[5, 4]),
        ];
        for (i, (what, term, entities)) in refused.into_iter().enumerate() {
            let path = temp.path().join(format!("refused-{i}.spill"));
            let mut writer = TextRunWriter::create(&path, RunPayload::Rows).unwrap();
            push(&mut writer, b"bravo", &[1, 2]).unwrap();
            assert!(push(&mut writer, term, entities).is_err(), "{what}");
        }
    }

    /// **A run read in term ranges, each started from the run's index, gives back every record
    /// once**, and what the ranges decoded between them checks against the receipt. A range
    /// whose index entry names another record is refused.
    #[test]
    fn a_run_read_in_ranges_gives_back_each_record_once() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("ranges.spill");
        let records: Vec<(String, Vec<u32>)> = (0..4_000u32)
            .map(|t| (format!("term{t:05}"), (0..5).map(|k| k * 7 + t).collect()))
            .collect();
        let receipt = write_text_run(
            &path,
            &records
                .iter()
                .map(|(term, entities)| (term.as_str(), entities.clone()))
                .collect::<Vec<_>>(),
        );
        let run = open_run(&receipt);
        let points = run.samples(usize::MAX).unwrap();
        assert!(points.len() > 4, "{} index entries", points.len());

        let mut read: Vec<(String, Vec<u32>)> = Vec::new();
        let (mut pairs, mut anchor) = (0u64, 0u64);
        for range in crate::merge::split(run.samples(16).unwrap(), 5) {
            let mut merge =
                TermMerge::range(vec![open_reader(&run, RunPayload::Rows)], &range).unwrap();
            while merge.next_term().unwrap() {
                let mut entities = Vec::new();
                merge
                    .drain(std::convert::identity, &mut |entity, _| {
                        entities.push(entity);
                        Ok(())
                    })
                    .unwrap();
                read.push((String::from_utf8(merge.term().to_vec()).unwrap(), entities));
            }
            let (p, a) = merge.into_sources()[0].share();
            (pairs, anchor) = (pairs + p, anchor.wrapping_add(a));
        }
        assert_eq!(read, records);
        run.check(pairs, anchor).unwrap();
        assert_refused(run.check(pairs - 1, anchor));

        // Index entry 1 named at entry 2's offset.
        let bytes = fs::read(&path).unwrap();
        let tail = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap()) as usize;
        let offsets = tail(bytes.len() - 20) + tail(bytes.len() - 12);
        let mut damaged = bytes.clone();
        damaged.copy_within(offsets + 16..offsets + 24, offsets + 8);
        drop(run);
        fs::write(&path, damaged).unwrap();
        let run = open_run(&receipt);
        let mut reader = open_reader(&run, RunPayload::Rows);
        assert_refused(reader.seek(&points[1].key));
    }

    /// **The allowance is what the budget leaves**, and never less than the pass runs in. Its
    /// fan-in is what it pays for at the smallest buffer, and each cursor's buffer is its share,
    /// between the smallest and the largest.
    #[test]
    fn the_allowance_is_what_the_budget_leaves_and_pays_for_the_fan_in() {
        assert_eq!(Allowance::new(8 << 30, 1 << 30, 4).bytes, 7 << 30);
        assert_eq!(
            Allowance::new(8 << 30, 1 << 30, 4).per_worker(),
            (7 << 30) / 4
        );
        let floor = Allowance::new(1 << 30, 2 << 30, 4);
        assert_eq!(floor.bytes, 4 * WORKER_BYTES_MIN as u64);

        let allowance = Allowance::new(64 * RUN_BUFFER_MIN as u64, 0, 1);
        assert_eq!(allowance.fan_in(), 64.min(merge_descriptors()));
        assert_eq!(allowance.buffer(64), RUN_BUFFER_MIN);
        assert_eq!(allowance.buffer(1_000), RUN_BUFFER_MIN);
        assert_eq!(allowance.buffer(1), SPILL_BUF_BYTES);
    }

    /// **No chunk outgrows what its `u32` offsets address**, however large the allowance: an
    /// accumulator's share and a keyword chunk's whole allowance both stop at [`CHUNK_BYTES_MAX`],
    /// and a share under it is the allowance's own.
    #[test]
    fn a_chunk_stays_within_what_its_offsets_address() {
        let large = Allowance::new(64 << 30, 0, 4);
        assert_eq!(large.per_worker(), CHUNK_BYTES_MAX);
        assert_eq!(large.chunk(1), CHUNK_BYTES_MAX);
        assert!((CHUNK_BYTES_MAX as u64) < u64::from(u32::MAX));
        let small = Allowance::new(4 << 30, 0, 4);
        assert_eq!(small.per_worker(), 1 << 30);
    }

    /// **A cascade pass merges only the leading runs that bring the count to the fan-in**, in at
    /// least as many groups as there are threads, and takes every run only where no narrower pass
    /// reaches it.
    #[test]
    fn a_cascade_pass_merges_the_excess_and_no_more() {
        // 104 runs over a fan-in of 100 on four threads: four groups of two.
        let pass = CascadePass::of(104, 100, 4);
        assert_eq!(
            pass,
            CascadePass {
                taken: 8,
                groups: 4
            }
        );
        let widths: Vec<usize> = pass.split(&[0u8; 8]).iter().map(|g| g.len()).collect();
        assert_eq!(widths, [2, 2, 2, 2]);
        // An excess too large for groups that run four at once takes groups of F - 1.
        let pass = CascadePass::of(400, 12, 4);
        assert_eq!(pass.taken, 400);
        assert!(pass
            .split(&[0u8; 400])
            .iter()
            .all(|g| (2..=11).contains(&g.len())));
        // Every pass's output count is what the next one starts from.
        for (runs, fan_in, threads) in [(104, 100, 4), (40, 12, 4), (5_000, 100, 12), (13, 12, 1)] {
            let pass = CascadePass::of(runs, fan_in, threads);
            let left = runs - pass.taken + pass.groups;
            assert!(left < runs, "{runs} runs over {fan_in}");
            if pass.taken < runs {
                assert_eq!(left, fan_in, "{runs} runs over {fan_in}");
            }
        }
    }

    /// **More runs than the fan-in are cascaded, several groups at once, in the passes the
    /// allowance forecasts**, and what is left merges to what the runs held, with every
    /// intermediate file gone.
    #[test]
    fn a_cascade_reduces_the_runs_to_the_fan_in_and_keeps_every_entity() {
        let temp = tempfile::TempDir::new().unwrap();
        let terms = ["alpha", "beta", "delta", "gamma"];
        let runs = 40u32;
        let receipts: Vec<SpillReceipt> = (0..runs)
            .map(|run| {
                let records: Vec<(&str, Vec<u32>)> = terms
                    .iter()
                    .enumerate()
                    .filter(|(t, _)| !(run as usize + t).is_multiple_of(3))
                    .map(|(_, term)| (*term, (0..50).map(|k| k * runs + run).collect()))
                    .collect();
                write_text_run(&temp.path().join(format!("run-{run}.spill")), &records)
            })
            .collect();
        let merged = |receipts: &[SpillReceipt]| {
            let runs: Vec<RunFile> = receipts.iter().map(open_run).collect();
            let mut merge = TermMerge::open(
                runs.iter()
                    .map(|run| open_reader(run, RunPayload::Rows))
                    .collect(),
            )
            .unwrap();
            let mut out = Vec::new();
            while merge.next_term().unwrap() {
                let mut entities = Vec::new();
                merge
                    .drain(std::convert::identity, &mut |entity, _| {
                        entities.push(entity);
                        Ok(())
                    })
                    .unwrap();
                out.push((merge.term().to_vec(), entities));
            }
            out
        };
        let expected = merged(&receipts);

        // A fan-in of twelve over forty runs: the excess of 28 takes four groups of eight, one at
        // a time, and the eight runs past them are left as they are.
        let allowance = Allowance {
            bytes: 12 * RUN_BUFFER_MIN as u64,
            threads: 4,
        };
        let (left, passes) = cascade_sorted_runs(
            temp.path(),
            "test",
            RunPayload::Rows,
            receipts.clone(),
            allowance,
        )
        .unwrap();
        assert!(left.len() <= allowance.fan_in(), "{} runs left", left.len());
        assert_eq!(passes, allowance.cascade_passes(runs as usize));
        assert_eq!(passes, 1);
        assert_eq!(merged(&left), expected);
        let left_paths: Vec<&PathBuf> = left.iter().map(|run| &run.path).collect();
        let (kept, merged_away): (Vec<&SpillReceipt>, Vec<&SpillReceipt>) = receipts
            .iter()
            .partition(|run| left_paths.contains(&&run.path));
        assert_eq!(
            kept.len(),
            8,
            "the runs past the groups are left as they are"
        );
        assert!(merged_away.iter().all(|run| !run.path.exists()));
        remove_runs(&left).unwrap();
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    proptest! {
        /// Any sorted, distinct term stream with ascending entity lists round-trips exactly.
        #[test]
        fn any_text_run_round_trips(
            raw in prop::collection::vec(
                ("\\PC{1,5}", prop::collection::vec(prop::num::u32::ANY, 1..8)),
                0..40),
        ) {
            let mut records: Vec<(Vec<u8>, Vec<u32>)> = raw
                .into_iter()
                .map(|(term, mut entities): (String, Vec<u32>)| {
                    entities.sort_unstable();
                    entities.dedup();
                    (term.into_bytes(), entities)
                })
                .collect();
            records.sort_by(|a, b| a.0.cmp(&b.0));
            records.dedup_by(|a, b| a.0 == b.0);

            let temp = tempfile::TempDir::new().unwrap();
            let path = temp.path().join("text-run.spill");
            let mut writer = TextRunWriter::create(&path, RunPayload::Rows).unwrap();
            for (term, entities) in &records {
                push(&mut writer, term, entities).unwrap();
            }
            let receipt = writer.finish().unwrap();

            let run = open_run(&receipt);
            let mut reader = open_reader(&run, RunPayload::Rows);
            let mut read: Vec<(Vec<u8>, Vec<u32>)> = Vec::new();
            while reader.next_term().unwrap() {
                let term = reader.term().to_vec();
                let mut entities = Vec::new();
                while let Some(entity) = reader.next_entity().unwrap() {
                    entities.push(entity);
                }
                read.push((term, entities));
            }
            prop_assert_eq!(read, records);
        }
    }
}
