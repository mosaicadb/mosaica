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
//! A **sorted run** is written by [`TextRunWriter`] and read back by a run reader inside
//! [`TextRunMerge`], which merges runs k-way on the term. [`cascade_sorted_runs`] reduces more runs
//! than one merge may hold open. A malformed run is refused as [`io::ErrorKind::InvalidData`].
//!
//! **What a merge may hold is an [`Allowance`]**, the one rule the build and the service both size
//! a term index's pass by. Its fan-in, and the read buffer each run's cursor gets, are what the
//! allowance pays for, under the share of the process's file descriptors a merge may take
//! ([`merge_descriptors`]). Every cursor on a run reads it at its own offset through one shared
//! descriptor.
//! Every error a run's reader or writer returns names its file, an I/O failure as a
//! [`SpillFileError`], and every refusal the merge makes of its own names its term.

use std::fmt::Display;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mosaica_filter::{common_prefix_len, put_varint, take_varint, VarintError};
use mosaica_types::splitmix64;

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
}

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

    /// A writer whose buffer is `buffer` bytes, for a cascade pass that charges it to its
    /// allowance.
    fn buffered(path: &Path, payload: RunPayload, buffer: usize) -> io::Result<TextRunWriter> {
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
        })
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
        let shared = common_prefix_len(&self.prev, term);
        self.record.clear();
        put_varint(shared as u32, &mut self.record);
        put_varint((term.len() - shared) as u32, &mut self.record);
        self.record.extend_from_slice(&term[shared..]);
        put_varint(entities, &mut self.record);
        self.writer
            .write_all(&self.record)
            .map_err(|e| at(&self.path, e))?;

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
        let written = self
            .writer
            .write_all(&record)
            .map_err(|e| at(&self.path, e));
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
        let (writer, path) = (&mut self.writer, &self.path);
        body(&mut |piece| writer.write_all(piece).map_err(|e| at(path, e)))?;
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

    /// Flush, fsync, and hand back the receipt a run's reader must be given.
    pub fn finish(self) -> io::Result<SpillReceipt> {
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

/// Streams a text run back **term by term**, verifying count and anchor at end of stream.
///
/// **The head's entities are not decoded until they are asked for.** A k-way merge holds one open
/// reader per run and compares only their head terms, so a reader that eagerly materialised each
/// head's entity list would put *k* whole posting lists in memory to choose between *k* strings —
/// which is the residency this whole pass exists to bound. [`Self::advance`] reads the term and
/// its length and stops; [`Self::next_entity`] decodes them, and `advance` drains whatever a
/// caller left rather than trusting it to have consumed the record.
///
/// Verification is terminal — a running sum can only be checked once the stream ends — and the
/// merge drains every run to completion.
struct TextRunReader {
    path: PathBuf,
    payload: RunPayload,
    reader: BufReader<RunCursor>,
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
    expect_count: u64,
    expect_anchor: u64,
    count: u64,
    anchor: u64,
    done: bool,
}

/// A cursor's place in a run's file. It reads at its own offset (`pread`), so every cursor on one
/// run shares the run's one descriptor.
struct RunCursor {
    file: Arc<File>,
    at: u64,
}

impl Read for RunCursor {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        #[cfg(unix)]
        let read = std::os::unix::fs::FileExt::read_at(&*self.file, buf, self.at)?;
        #[cfg(windows)]
        let read = std::os::windows::fs::FileExt::seek_read(&*self.file, buf, self.at)?;
        self.at += read as u64;
        Ok(read)
    }
}

impl TextRunReader {
    /// A reader of the run `receipt` names, from its start, through `file`, which is that run's
    /// open file, with a read buffer of `buffer` bytes.
    fn open(
        file: Arc<File>,
        receipt: &SpillReceipt,
        payload: RunPayload,
        buffer: usize,
    ) -> TextRunReader {
        TextRunReader {
            path: receipt.path.clone(),
            payload,
            positions: Vec::new(),
            reader: BufReader::with_capacity(buffer, RunCursor { file, at: 0 }),
            term: Vec::new(),
            mark: 0,
            pending: 0,
            taken: 0,
            last_entity: 0,
            expect_count: receipt.count,
            expect_anchor: receipt.anchor,
            count: 0,
            anchor: 0,
            done: false,
        }
    }

    /// Move to the next term, returning `false` at a *verified* end of stream.
    fn advance(&mut self) -> io::Result<bool> {
        if self.done {
            return Ok(false);
        }
        // Whatever the caller did not take: decoded and anchored, never skipped by seeking, so a
        // malformation inside a record the merge had no use for is still caught.
        while self.pending > 0 {
            self.decode_entity()?;
        }
        if at_end(&mut self.reader).map_err(|e| at(&self.path, e))? {
            self.verify_end()?;
            return Ok(false);
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
            .map_err(|e| at(&self.path, e))?;
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
        if self.count + count as u64 > self.expect_count {
            return Err(self.malformed(&format!(
                "trailing data: more pairs than the receipt's count {}",
                self.expect_count
            )));
        }
        self.mark = fingerprint(&self.term);
        self.pending = count;
        self.taken = 0;
        self.last_entity = 0;
        Ok(true)
    }

    /// The head term. Meaningful only after [`Self::advance`] returned `true`.
    fn term(&self) -> &[u8] {
        &self.term
    }

    /// How many entities the head carries, decoded or not.
    fn pending(&self) -> u32 {
        self.pending
    }

    /// The term's positions in the entity [`Self::next_entity`] last returned: empty in a
    /// [`RunPayload::Rows`] run.
    fn positions(&self) -> &[u32] {
        &self.positions
    }

    /// The head's next entity, or `None` once it is drained.
    ///
    /// **One at a time, because the runs do not partition entity space.** A run's entities ascend
    /// within its own record, but the build divides the arena and not entity space, so a term's
    /// merged list is produced by comparing the runs' heads. That needs a head to look at.
    fn next_entity(&mut self) -> io::Result<Option<u32>> {
        if self.pending == 0 {
            return Ok(None);
        }
        self.decode_entity().map(Some)
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
        self.count += 1;
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

    fn verify_end(&mut self) -> io::Result<()> {
        if self.count != self.expect_count {
            return Err(self.malformed(&format!(
                "pair count mismatch: decoded {} pairs but the receipt says {}",
                self.count, self.expect_count
            )));
        }
        if self.anchor != self.expect_anchor {
            return Err(self.malformed(&format!(
                "content anchor mismatch: recomputed {:#018x} but the receipt says {:#018x} — \
                 the file's bytes are not the bytes that were written",
                self.anchor, self.expect_anchor
            )));
        }
        self.done = true;
        Ok(())
    }

    fn next_u32(&mut self) -> io::Result<u32> {
        read_varint(&mut self.reader, take_varint).map_err(|e| {
            if e.kind() == io::ErrorKind::InvalidData {
                self.malformed(&e.to_string())
            } else {
                at(&self.path, e)
            }
        })
    }

    fn malformed(&self, detail: &str) -> io::Error {
        malformed(&self.path, detail)
    }
}

// --------------------------------------------------------------------------------------------
// The merge, and the cascade above it
// --------------------------------------------------------------------------------------------

/// A k-way merge over open run cursors, yielding each distinct term once with its entities in
/// ascending order.
///
/// The heap holds **run indices**, and the comparison reaches into the readers — so a term is
/// never copied into the heap and the merge allocates nothing per record.
///
/// **The entities are merged, not concatenated.** A run holds entities from all over entity space
/// and two runs' lists interleave — [`Self::drain`] compares their heads. What that costs is a
/// comparison per posting against a copy per posting; what it buys is a sequential read of a file
/// larger than the machine.
pub struct TextRunMerge {
    cursors: Vec<TextRunReader>,
    heap: Vec<usize>,
    /// The selected term. Held as bytes because that is what the cursors compare on, and because
    /// the caller needs it after the runs carrying it have left the heap.
    term: Vec<u8>,
    /// The runs whose head is the selected term, ascending — drained in this order.
    selected: Vec<usize>,
    /// Entities the selected term carries across all of them.
    postings: u64,
    /// The selected runs' unconsumed heads, `(entity, run)`, smallest first. Held on the merge
    /// rather than built per term so a corpus's whole vocabulary costs one allocation.
    heads: std::collections::BinaryHeap<std::cmp::Reverse<(u32, usize)>>,
}

impl TextRunMerge {
    /// A merge over `receipts`, each run read through a buffer of `buffer` bytes.
    pub fn open(
        receipts: &[SpillReceipt],
        payload: RunPayload,
        buffer: usize,
    ) -> io::Result<TextRunMerge> {
        let mut cursors: Vec<TextRunReader> = Vec::with_capacity(receipts.len());
        let mut heap: Vec<usize> = Vec::with_capacity(receipts.len());
        for receipt in receipts {
            let file = File::open(&receipt.path).map_err(|e| at(&receipt.path, e))?;
            let mut reader = TextRunReader::open(Arc::new(file), receipt, payload, buffer);
            // A run with no terms at all — an empty chunk — verifies its receipt here and takes no
            // place in the heap.
            if reader.advance()? {
                heap.push(cursors.len());
            }
            cursors.push(reader);
        }
        for root in (0..heap.len() / 2).rev() {
            text_heap_sift_down(&mut heap, &cursors, root);
        }
        Ok(TextRunMerge {
            cursors,
            heap,
            term: Vec::new(),
            selected: Vec::new(),
            postings: 0,
            heads: std::collections::BinaryHeap::new(),
        })
    }

    /// Select the next distinct term, or `false` when every run is exhausted. The previous term's
    /// runs are advanced here rather than by the caller, so a caller that took fewer entities than
    /// the term carried still leaves the stream where the next record starts.
    pub fn next_term(&mut self) -> io::Result<bool> {
        for &run in &self.selected {
            if self.cursors[run].advance()? {
                text_heap_push(&mut self.heap, &self.cursors, run);
            }
        }
        self.selected.clear();
        self.postings = 0;
        let Some(&first) = self.heap.first() else {
            return Ok(false);
        };
        self.term.clear();
        self.term.extend_from_slice(self.cursors[first].term());
        while self
            .heap
            .first()
            .is_some_and(|&top| self.cursors[top].term() == self.term.as_slice())
        {
            let top = text_heap_pop(&mut self.heap, &self.cursors);
            self.postings += self.cursors[top].pending() as u64;
            self.selected.push(top);
        }
        Ok(true)
    }

    pub fn term(&self) -> &[u8] {
        &self.term
    }

    /// How many entities the selected term carries. Known before a single one is decoded, which is
    /// what lets the caller choose an encoder without buffering to find out.
    pub fn postings(&self) -> u64 {
        self.postings
    }

    /// Feed the selected term's entities to `sink`, ascending, each with the term's positions in it
    /// (none in a run of rows), checking the ascent as it goes. A failure of the runs reaches the
    /// caller through `lift`, in the sink's error type.
    ///
    /// The check is over the *merged* list, not each run's: a run's own ascent is the writer's
    /// business, and what could go wrong here is the merge. It is also what catches a position
    /// carried by two runs — which cannot happen in either caller, because a text entity has
    /// exactly one live arena record and one window holds it, and a keyword row is visited once by
    /// one walk — and is exactly the failure a silent `>=` would hide. `encode_posting` re-checks
    /// the slice path, but the bitmap path has no such check to make — a `Bitmap` is a set — which
    /// is why the test lives here rather than there.
    pub fn drain<E>(
        &mut self,
        lift: impl Fn(io::Error) -> E,
        sink: &mut impl FnMut(u32, &[u32]) -> Result<(), E>,
    ) -> Result<(), E> {
        self.heads.clear();
        for i in 0..self.selected.len() {
            let run = self.selected[i];
            if let Some(entity) = self.cursors[run].next_entity().map_err(&lift)? {
                self.heads.push(std::cmp::Reverse((entity, run)));
            }
        }
        let mut last: Option<u32> = None;
        while let Some(std::cmp::Reverse((entity, run))) = self.heads.pop() {
            if let Some(previous) = last {
                if entity <= previous {
                    return Err(lift(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "run merge: term {:?}'s entity {entity} does not ascend past \
                             {previous}: two runs carry the same entity for one term, or the same \
                             row for one key",
                            String::from_utf8_lossy(&self.term)
                        ),
                    )));
                }
            }
            last = Some(entity);
            // A run's head entity is the last it decoded, so its positions are still the reader's.
            sink(entity, self.cursors[run].positions())?;
            if let Some(next) = self.cursors[run].next_entity().map_err(&lift)? {
                self.heads.push(std::cmp::Reverse((next, run)));
            }
        }
        Ok(())
    }
}

/// Order two runs by their head term, ties broken by run index so that equal terms leave the heap
/// in ascending entity order.
fn text_run_before(cursors: &[TextRunReader], a: usize, b: usize) -> bool {
    match cursors[a].term().cmp(cursors[b].term()) {
        std::cmp::Ordering::Less => true,
        std::cmp::Ordering::Greater => false,
        std::cmp::Ordering::Equal => a < b,
    }
}

fn text_heap_sift_down(heap: &mut [usize], cursors: &[TextRunReader], mut node: usize) {
    loop {
        let left = node * 2 + 1;
        if left >= heap.len() {
            return;
        }
        let right = left + 1;
        let child = if right < heap.len() && text_run_before(cursors, heap[right], heap[left]) {
            right
        } else {
            left
        };
        if !text_run_before(cursors, heap[child], heap[node]) {
            return;
        }
        heap.swap(node, child);
        node = child;
    }
}

fn text_heap_push(heap: &mut Vec<usize>, cursors: &[TextRunReader], run: usize) {
    heap.push(run);
    let mut node = heap.len() - 1;
    while node > 0 {
        let parent = (node - 1) / 2;
        if !text_run_before(cursors, heap[node], heap[parent]) {
            return;
        }
        heap.swap(node, parent);
        node = parent;
    }
}

fn text_heap_pop(heap: &mut Vec<usize>, cursors: &[TextRunReader]) -> usize {
    let top = heap[0];
    let last = heap.pop().expect("the heap is not empty");
    if !heap.is_empty() {
        heap[0] = last;
        text_heap_sift_down(heap, cursors, 0);
    }
    top
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

/// What one term index's pass may hold, and on how many threads it runs.
///
/// **One rule, for the build and the service.** [`Allowance::new`] takes what is to be had (a
/// build's `--memory-budget`, or the memory a server has available) less what the rest of the
/// pass's stage holds beside it. Everything the pass holds is charged against the one figure: its
/// accumulators while it tokenises, a share each ([`Self::per_worker`]), and after them the read
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
        let least = threads as u64 * WORKER_BYTES_MIN as u64;
        Allowance {
            bytes: budget.saturating_sub(resident).max(least),
            threads,
        }
    }

    /// What each of the pass's accumulators may allocate before it spills.
    pub fn per_worker(&self) -> usize {
        usize::try_from(self.bytes / self.threads.max(1) as u64).unwrap_or(usize::MAX)
    }

    /// **The fan-in: the most runs one merge level opens**, with `ranges` merges in flight at once,
    /// F = M / (W × B) with each cursor at [`RUN_BUFFER_MIN`]. Held under [`merge_descriptors`],
    /// since every cursor on one run shares its descriptor and the runs are what is counted, and
    /// three at least, so that a cascade pass reduces two runs into one with its writer beside
    /// them.
    ///
    /// One level over R runs fits while R is at most F. R is the run bytes over a run's size, and a
    /// run's size follows the allowance over the threads, so one level holds a corpus whose run
    /// bytes grow as the square of the allowance. Above that the cascade takes a pass.
    pub fn fan_in(&self, ranges: usize) -> usize {
        let by_memory = self.bytes / (ranges.max(1) as u64 * RUN_BUFFER_MIN as u64);
        usize::try_from(by_memory)
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
        let fan_in = self.fan_in(1);
        let mut passes = 0;
        while runs > fan_in {
            runs = runs.div_ceil(cascade_group(runs, fan_in));
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

/// The runs one cascade group merges where `runs` are left over a fan-in of `fan_in`: the fewest
/// that bring the pass's output to the fan-in, so the next level is the final merge, and at most
/// one less than the fan-in, for the group's writer.
fn cascade_group(runs: usize, fan_in: usize) -> usize {
    runs.div_ceil(fan_in).clamp(2, fan_in - 1)
}

/// Reduce `receipts` to at most the allowance's fan-in, deleting each pass's inputs as it goes, and
/// return what is left and the passes taken. `family` names the intermediate files, so two column
/// families cascading in one directory cannot collide. On an error the cascade removes every file
/// it wrote, and leaves the caller's runs to the caller.
///
/// **A pass merges groups of runs into one intermediate run each, several groups at once.** Each
/// group is as small as brings the pass's output under the fan-in ([`cascade_group`]), and as many
/// groups run at once as their cursors' and writers' buffers fit in the allowance, up to its
/// threads. So the groups in flight hold no more than one merge level would, and a corpus whose
/// runs are under the square of the fan-in takes one pass. An intermediate run is written by the
/// same writer as an accumulator's, so the cascade adds a pass and not a format.
///
/// Groups are taken in order and each group merges in order, so the entity ordering the final
/// merge relies on survives every pass.
pub fn cascade_sorted_runs(
    column_dir: &Path,
    family: &str,
    payload: RunPayload,
    mut receipts: Vec<SpillReceipt>,
    allowance: Allowance,
) -> io::Result<(Vec<SpillReceipt>, usize)> {
    let fan_in = allowance.fan_in(1);
    let mut pass = 0usize;
    while receipts.len() > fan_in {
        let group = cascade_group(receipts.len(), fan_in);
        let in_flight = (fan_in / (group + 1)).clamp(1, allowance.threads.max(1));
        let buffer = allowance.buffer(in_flight * (group + 1));
        let path =
            |index: usize| column_dir.join(format!("{family}-merge-{pass:02}-{index:05}.spill"));
        // Deleted per group rather than per pass: a corpus that reaches the cascade at all is one
        // whose runs are large, and holding a whole pass's inputs beside a whole pass's outputs
        // would double the spill's peak on disk.
        let merge = |(index, runs): (usize, &[SpillReceipt])| {
            merge_group(runs, &path(index), payload, buffer)
                .and_then(|run| remove_runs(runs).map(|()| run))
        };
        let groups: Vec<&[SpillReceipt]> = receipts.chunks(group).collect();
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
            Ok(merged) => receipts = merged,
            Err(e) => {
                for index in 0..groups.len() {
                    let _ = std::fs::remove_file(path(index));
                }
                if pass > 0 {
                    let _ = remove_runs(&receipts);
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
    let mut merge = TextRunMerge::open(runs, payload, buffer)?;
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

    /// The read or write was refused as malformed, rather than failing for I/O.
    fn assert_refused<T: std::fmt::Debug>(result: io::Result<T>) {
        assert!(
            matches!(&result, Err(e) if e.kind() == io::ErrorKind::InvalidData),
            "{result:?}"
        );
    }

    /// A reader of the run `receipt` names.
    fn open_reader(receipt: &SpillReceipt, payload: RunPayload) -> TextRunReader {
        let file = File::open(&receipt.path).unwrap();
        TextRunReader::open(Arc::new(file), receipt, payload, RUN_BUFFER_MIN)
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
        let mut reader = open_reader(&receipt, RunPayload::Positions);
        for (term, items) in &records {
            assert!(reader.advance().unwrap());
            assert_eq!(reader.term(), *term);
            for (entity, positions) in items {
                assert_eq!(reader.next_entity().unwrap(), Some(*entity));
                assert_eq!(reader.positions(), positions.as_slice());
            }
        }
        assert!(!reader.advance().unwrap());

        // The last byte is the last position's gap: changed, the anchor no longer agrees.
        let mut bytes = fs::read(&path).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        fs::write(&path, &bytes).unwrap();
        let mut reader = open_reader(&receipt, RunPayload::Positions);
        assert!((|| -> io::Result<()> {
            while reader.advance()? {}
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
        let mut reader = open_reader(receipt, RunPayload::Rows);
        let mut out = Vec::new();
        while reader.advance()? {
            let term = String::from_utf8(reader.term().to_vec()).unwrap();
            let expected = reader.pending();
            let mut entities = Vec::new();
            while let Some(entity) = reader.next_entity()? {
                entities.push(entity);
            }
            assert_eq!(
                entities.len() as u32,
                expected,
                "`pending` promised the count"
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
        // The last byte is an entity delta: flipping it changes a posting and nothing else, which
        // is exactly the failure a count check alone would miss.
        let last = fs::read(&path).unwrap().len() - 1;
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
        // A whole extra record, well formed: `zz` with one entity. Only the receipt's pair count
        // separates it from a legitimate stream.
        append(&path, &[0, 2, b'z', b'z', 1, 3]);
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

    /// **The allowance is what the budget leaves**, and never less than the pass runs in. Its
    /// fan-in is what it pays for at the smallest buffer, divided among the ranges in flight, and
    /// each cursor's buffer is its share, between the smallest and the largest.
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
        assert_eq!(allowance.fan_in(1), 64.min(merge_descriptors()));
        assert_eq!(allowance.fan_in(4), 16);
        assert_eq!(allowance.buffer(64), RUN_BUFFER_MIN);
        assert_eq!(allowance.buffer(1_000), RUN_BUFFER_MIN);
        assert_eq!(allowance.buffer(1), SPILL_BUF_BYTES);
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
            let mut merge =
                TextRunMerge::open(receipts, RunPayload::Rows, RUN_BUFFER_MIN).unwrap();
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

        // A fan-in of twelve: groups of four, two at once, and one pass.
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
        assert!(
            left.len() <= allowance.fan_in(1),
            "{} runs left",
            left.len()
        );
        assert_eq!(passes, allowance.cascade_passes(runs as usize));
        assert_eq!(passes, 1);
        assert_eq!(merged(&left), expected);
        assert!(receipts.iter().all(|run| !run.path.exists()));
        remove_runs(&left).unwrap();
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    proptest! {
        /// Any sorted, distinct term stream with ascending entity lists round-trips exactly.
        #[test]
        fn any_text_run_round_trips(
            raw in prop::collection::vec(
                (prop::collection::vec(prop::num::u8::ANY, 1..6),
                 prop::collection::vec(prop::num::u32::ANY, 1..8)),
                0..40),
        ) {
            let mut records: Vec<(Vec<u8>, Vec<u32>)> = raw
                .into_iter()
                .map(|(term, mut entities)| {
                    entities.sort_unstable();
                    entities.dedup();
                    (term, entities)
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

            let mut reader = open_reader(&receipt, RunPayload::Rows);
            let mut read: Vec<(Vec<u8>, Vec<u32>)> = Vec::new();
            while reader.advance().unwrap() {
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
