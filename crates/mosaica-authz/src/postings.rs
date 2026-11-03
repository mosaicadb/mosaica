//! CSR postings writer and reader for `terms/postings.arrow`.
//!
//! `terms/postings.arrow` is one Arrow IPC FILE holding a single record batch with one
//! `LargeBinaryArray` column named `posting`; row ordinal = term_id. Each record is
//! `u8 tag ‖ payload`: tag 0 is a sorted `u32` little-endian entity array, used when
//! `count <= small_term_threshold` including the empty case; tag 1 is portable-serialised
//! Roaring bitmap bytes.
//!
//! Postings are entity-space only; `RowId` never appears here.

use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::Arc;

use arrow::array::{Array, ArrayRef, LargeBinaryArray, LargeBinaryBuilder};
use arrow::buffer::{Buffer, OffsetBuffer, ScalarBuffer};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use arrow::ipc::reader::{read_footer_length, FileDecoder};
use arrow::ipc::writer::FileWriter;
use arrow::ipc::{root_as_footer, MetadataVersion};
use arrow::record_batch::RecordBatch;
use croaring::{Bitmap, BitmapView, Portable};

use mosaica_types::TermId;

const POSTING_COLUMN_NAME: &str = "posting";

/// Write `postings.arrow`: `per_term[t]` is term *t*'s sorted entity list. A record uses tag 0
/// (raw little-endian `u32` array) when `per_term[t].len() <= small_term_threshold as usize`,
/// including the empty case, and tag 1 (portable Roaring bytes) otherwise.
///
/// `per_term[t]` must already be sorted strictly ascending with no duplicates. The check runs
/// unconditionally, not just in debug builds: these are authorisation masks, and an unsorted or
/// duplicated input would otherwise silently diverge in content depending on which side of
/// `small_term_threshold` a term's count lands.
pub fn write_postings(
    path: &Path,
    per_term: &[Vec<u32>],
    small_term_threshold: u32,
) -> io::Result<()> {
    let mut records = Vec::with_capacity(per_term.len());
    for (t, entities) in per_term.iter().enumerate() {
        records.push(encode_posting(t, entities, small_term_threshold)?);
    }
    write_posting_records(path, &records)
}

/// Encode term `t`'s sorted entity list into its on-disk record (`u8 tag ‖ payload`), applying
/// the same tag rule and sortedness check as [`write_postings`]. Split out so a build can encode
/// each term as soon as its list is complete and keep only the compressed records.
pub fn encode_posting(
    t: usize,
    entities: &[u32],
    small_term_threshold: u32,
) -> io::Result<Vec<u8>> {
    if !entities.windows(2).all(|w| w[0] < w[1]) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "write_postings: term {t}'s entity list must be sorted strictly ascending \
                 (no duplicates)"
            ),
        ));
    }

    let mut record = Vec::new();
    if (entities.len() as u64) <= small_term_threshold as u64 {
        record.push(0u8);
        for entity in entities {
            record.extend_from_slice(&entity.to_le_bytes());
        }
    } else {
        let mut bitmap = Bitmap::of(entities);
        bitmap.run_optimize();
        record.push(1u8);
        record.extend_from_slice(&bitmap.serialize::<Portable>());
    }
    Ok(record)
}

/// [`encode_posting`] from a `Bitmap` rather than a `&[u32]`, avoiding materialising a large
/// term's bitmap as a `Vec<u32>` first. Byte-identical to [`encode_posting`] over the same set:
/// both apply the same tag rule, tag 0 writes the same ascending `u32` LEs, and tag 1 runs the
/// same `run_optimize` before the same `Portable` serialisation.
///
/// No sortedness check, and none is needed: a `Bitmap` is already a sorted, deduplicated set.
pub fn encode_posting_bitmap(bitmap: &Bitmap, small_term_threshold: u32) -> io::Result<Vec<u8>> {
    let cardinality = bitmap.cardinality();
    let mut record = Vec::new();
    if cardinality <= small_term_threshold as u64 {
        record.push(0u8);
        // Ascending by construction, since `Bitmap`'s iterator is ordered.
        for entity in bitmap.iter() {
            record.extend_from_slice(&entity.to_le_bytes());
        }
    } else {
        // Cloned because `run_optimize` mutates the caller's own working set.
        let mut optimised = bitmap.clone();
        optimised.run_optimize();
        record.push(1u8);
        record.extend_from_slice(&optimised.serialize::<Portable>());
    }
    Ok(record)
}

/// Write `postings.arrow` from already-encoded records (see [`encode_posting`]); record ordinal
/// = term id. Byte-for-byte the same file [`write_postings`] would write from the same postings.
pub fn write_posting_records(path: &Path, records: &[Vec<u8>]) -> io::Result<()> {
    let mut builder = LargeBinaryBuilder::new();
    for record in records {
        builder.append_value(record);
    }
    write_record_array(path, POSTING_COLUMN_NAME, builder.finish())
}

/// The single schema/batch/IPC-writer invocation behind every file this crate writes, so a
/// buffered and a spooled path stay byte-identical.
pub(crate) fn write_single_batch(
    path: &Path,
    schema: SchemaRef,
    columns: Vec<ArrayRef>,
) -> io::Result<()> {
    let batch = RecordBatch::try_new(schema.clone(), columns)
        .map_err(|e| invalid_data(e.to_string()))?;

    let file = File::create(path)?;
    let mut writer = FileWriter::try_new(BufWriter::new(file), &schema)
        .map_err(|e| invalid_data(e.to_string()))?;
    writer
        .write(&batch)
        .map_err(|e| invalid_data(e.to_string()))?;
    writer.finish().map_err(|e| invalid_data(e.to_string()))?;
    Ok(())
}

/// [`write_single_batch`] with one non-null `LargeBinary` column named `column`, record *i* the
/// *i*-th term's. The column carries no validity buffer: a spurious all-valid buffer would change
/// the file bytes.
fn write_record_array(path: &Path, column: &str, array: LargeBinaryArray) -> io::Result<()> {
    let schema = Arc::new(Schema::new(vec![Field::new(
        column,
        DataType::LargeBinary,
        false,
    )]));
    write_single_batch(path, schema, vec![Arc::new(array)])
}

/// Memory-map `file` whole as an Arrow buffer, without copying.
pub(crate) fn mapped_buffer(file: &File) -> io::Result<Buffer> {
    let mapping = unsafe { memmap2::Mmap::map(file) }?;
    let len = mapping.len();
    let arc: Arc<memmap2::Mmap> = Arc::new(mapping);
    // SAFETY: `arc` owns the mapping for as long as any Buffer built from it is alive, and the
    // mapping is valid for `len` bytes for its entire lifetime.
    let ptr = NonNull::new(arc.as_ptr() as *mut u8)
        .expect("memmap2::Mmap never returns a null base pointer");
    Ok(unsafe { Buffer::from_custom_allocation(ptr, len, arc) })
}

/// The spool file behind [`PostingsSpool`] and [`crate::KeyedPostingsSpool`], so a build does not
/// hold every record in memory at once: encoded records are appended to a temporary file as they
/// arrive, and the Arrow offset table to an unnamed file beside it. [`Self::finish`] maps both as
/// the column's buffers, byte-for-byte what a buffered writer would produce.
pub(crate) struct RecordSpool {
    spool_path: PathBuf,
    writer: BufWriter<File>,
    /// Arrow LargeBinary offsets as native-endian `i64`s with a leading 0: entries `i` and `i + 1`
    /// bound record `i`. The file has no name, so it goes however the spool ends.
    offsets: BufWriter<File>,
    last: i64,
    records: usize,
    /// A record being written in pieces: the bytes written since it began, and the length of the
    /// head its end writes back over its first bytes.
    open: Option<(usize, usize)>,
}

impl RecordSpool {
    /// Create (truncating) the spool file at `spool_path`.
    pub(crate) fn create(spool_path: &Path) -> io::Result<Self> {
        // Read access is required too: `finish` memory-maps the spool through this same handle.
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(spool_path)?;
        let dir = match spool_path.parent() {
            Some(dir) if !dir.as_os_str().is_empty() => dir,
            _ => Path::new("."),
        };
        let mut offsets = BufWriter::new(tempfile::tempfile_in(dir)?);
        offsets.write_all(&0i64.to_ne_bytes())?;
        Ok(RecordSpool {
            spool_path: spool_path.to_path_buf(),
            writer: BufWriter::new(file),
            offsets,
            last: 0,
            records: 0,
            open: None,
        })
    }

    pub(crate) fn append(&mut self, record: &[u8]) -> io::Result<()> {
        if self.open.is_some() {
            return Err(invalid_input("a record was appended while another was being written"));
        }
        let next = next_offset(self.last, record.len())?;
        self.writer.write_all(record)?;
        self.push_offset(next)
    }

    fn push_offset(&mut self, next: i64) -> io::Result<()> {
        self.offsets.write_all(&next.to_ne_bytes())?;
        self.last = next;
        self.records += 1;
        Ok(())
    }

    /// Begin a record written in pieces, with `head_len` bytes at its head that
    /// [`Self::end_record`] fills once the rest is written.
    pub(crate) fn begin_record(&mut self, head_len: usize) -> io::Result<()> {
        if self.open.is_some() {
            return Err(invalid_input("a record began while another was being written"));
        }
        self.writer.write_all(&vec![0u8; head_len])?;
        self.open = Some((head_len, head_len));
        Ok(())
    }

    /// Append `bytes` to the record [`Self::begin_record`] began.
    pub(crate) fn extend_record(&mut self, bytes: &[u8]) -> io::Result<()> {
        let Some((written, _)) = self.open.as_mut() else {
            return Err(invalid_input("record bytes arrived with no record begun"));
        };
        self.writer.write_all(bytes)?;
        *written += bytes.len();
        Ok(())
    }

    /// Close the record [`Self::begin_record`] began, writing `head` over its first bytes.
    pub(crate) fn end_record(&mut self, head: &[u8]) -> io::Result<()> {
        let Some((written, head_len)) = self.open.take() else {
            return Err(invalid_input("a record ended with no record begun"));
        };
        if head.len() != head_len {
            return Err(invalid_input(format!(
                "a record's head is {} bytes where {head_len} were reserved",
                head.len()
            )));
        }
        let last = self.last;
        if !head.is_empty() {
            let start = u64::try_from(last).map_err(|_| invalid_data("negative spool offset"))?;
            self.writer.seek(SeekFrom::Start(start))?;
            self.writer.write_all(head)?;
            self.writer.seek(SeekFrom::End(0))?;
        }
        self.push_offset(next_offset(last, written)?)
    }

    /// Build the posting column over maps of the spool and its offsets, hand that column to
    /// `write`, and delete the spool file once `write` has succeeded.
    pub(crate) fn finish(
        self,
        write: impl FnOnce(LargeBinaryArray) -> io::Result<()>,
    ) -> io::Result<()> {
        if self.open.is_some() {
            return Err(invalid_input("the spool finished while a record was being written"));
        }
        let total = usize::try_from(self.last)
            .map_err(|_| invalid_data("postings spool total exceeds usize on this platform"))?;
        let values = map_spooled(self.writer, total)?;
        let entries = self.records + 1;
        let offsets = map_spooled(self.offsets, entries * std::mem::size_of::<i64>())?;
        let offsets = OffsetBuffer::new(ScalarBuffer::new(offsets, 0, entries));
        let array = LargeBinaryArray::try_new(offsets, values, None)
            .map_err(|e| invalid_data(e.to_string()))?;
        write(array)?;
        std::fs::remove_file(&self.spool_path)
    }
}

/// Flush and fsync a spool file and map it as a buffer, which must be `len` bytes. The mapping
/// outlives the file handle.
fn map_spooled(writer: BufWriter<File>, len: usize) -> io::Result<Buffer> {
    let file = writer
        .into_inner()
        .map_err(io::IntoInnerError::into_error)?;
    // The spool is read back through a memory map; its bytes must be durable before the map is
    // taken.
    file.sync_all()?;
    if len == 0 {
        // memmap2 rejects zero-length maps.
        return Ok(Buffer::from_vec(Vec::<u8>::new()));
    }
    let buffer = mapped_buffer(&file)?;
    if buffer.len() != len {
        return Err(invalid_data(format!(
            "postings spool is {} bytes but the offset table accounts for {len}",
            buffer.len()
        )));
    }
    Ok(buffer)
}

/// Streaming counterpart to [`write_posting_records`]: encoded records and their Arrow offsets
/// are spooled to temporary files as they arrive, record ordinal = term id. `finish` writes
/// `postings.arrow` byte-for-byte the same as [`write_posting_records`] would from the same
/// records.
///
/// [`Self::for_column`] spools any file of that shape, one record per term in a column of another
/// name, and [`Self::begin_record`] writes a record larger than is worth holding.
pub struct PostingsSpool {
    spool: RecordSpool,
    column: &'static str,
}

impl PostingsSpool {
    /// Create (truncating) the spool file at `spool_path`.
    pub fn create(spool_path: &Path) -> io::Result<Self> {
        PostingsSpool::for_column(spool_path, POSTING_COLUMN_NAME)
    }

    /// [`Self::create`] for a file whose one column is named `column`.
    pub fn for_column(spool_path: &Path, column: &'static str) -> io::Result<Self> {
        Ok(PostingsSpool {
            spool: RecordSpool::create(spool_path)?,
            column,
        })
    }

    /// Append the next term's encoded record. Records must arrive in term order; ordinal in the
    /// finished file = term id.
    pub fn append(&mut self, record: &[u8]) -> io::Result<()> {
        self.spool.append(record)
    }

    /// Begin the next term's record, to be written in pieces after `head_len` bytes that
    /// [`Self::end_record`] fills.
    pub fn begin_record(&mut self, head_len: usize) -> io::Result<()> {
        self.spool.begin_record(head_len)
    }

    /// Append `bytes` to the record [`Self::begin_record`] began.
    pub fn extend_record(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.spool.extend_record(bytes)
    }

    /// Close the record [`Self::begin_record`] began, its head now `head`.
    pub fn end_record(&mut self, head: &[u8]) -> io::Result<()> {
        self.spool.end_record(head)
    }

    /// Flush and fsync the spool, write the file at `path` from it, and delete the spool file on
    /// success.
    pub fn finish(self, path: &Path) -> io::Result<()> {
        let column = self.column;
        self.spool
            .finish(|array| write_record_array(path, column, array))
    }
}

/// Bounds-check the next Arrow offset. LargeBinary offsets are `i64`; a spool whose running
/// total would exceed `i64::MAX` cannot be represented and must fail closed, not wrap.
fn next_offset(last: i64, record_len: usize) -> io::Result<i64> {
    let len = i64::try_from(record_len).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("posting record of {record_len} bytes exceeds i64::MAX"),
        )
    })?;
    last.checked_add(len).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "postings spool exceeds i64::MAX total bytes",
        )
    })
}

/// A borrowed view of one term's postings, tied to the lifetime of the [`PostingsReader`] that
/// produced it.
#[derive(Debug)]
pub enum PostingRef<'a> {
    /// Tag 0: raw little-endian `u32` entity ids, sorted ascending.
    Array(&'a [u8]),
    /// Tag 1: a Roaring bitmap view deserialised, no-copy, from the portable wire format.
    Roaring(BitmapView<'a>),
}

impl<'a> PostingRef<'a> {
    /// How many entities this posting holds.
    pub fn cardinality(&self) -> u64 {
        match self {
            PostingRef::Array(bytes) => (bytes.len() / 4) as u64,
            PostingRef::Roaring(view) => view.cardinality(),
        }
    }

    /// This posting's entities, ascending.
    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        let (array, view) = match self {
            PostingRef::Array(bytes) => {
                // Validated as a multiple of 4 once, at open time; not re-checked per lookup.
                debug_assert!(
                    bytes.len() % 4 == 0,
                    "tag-0 posting payload length must be a multiple of 4 (validated at open)"
                );
                (bytes.as_chunks::<4>().0, None)
            }
            PostingRef::Roaring(view) => (&[][..], Some(view.iter())),
        };
        let array = array.iter().map(|chunk| u32::from_le_bytes(*chunk));
        array.chain(view.into_iter().flatten())
    }

    /// Append this posting's entities to `out`, ascending.
    pub fn extend_into(&self, out: &mut Vec<u32>) {
        out.reserve(self.cardinality() as usize);
        out.extend(self.iter());
    }

    /// This posting as a bitmap: the mapped view itself, or an array posting decoded.
    pub fn into_bitmap(self) -> PostingBitmap<'a> {
        match self {
            PostingRef::Roaring(view) => PostingBitmap::View(view),
            array => {
                let mut entities = Vec::with_capacity(array.cardinality() as usize);
                array.extend_into(&mut entities);
                PostingBitmap::Owned(Bitmap::of(&entities))
            }
        }
    }
}

/// A posting as a bitmap, borrowed from the mapped file where its encoding allows.
#[derive(Debug)]
pub enum PostingBitmap<'a> {
    View(BitmapView<'a>),
    Owned(Bitmap),
}

impl std::ops::Deref for PostingBitmap<'_> {
    type Target = Bitmap;

    fn deref(&self) -> &Bitmap {
        match self {
            PostingBitmap::View(view) => view,
            PostingBitmap::Owned(bitmap) => bitmap,
        }
    }
}

/// Union `postings` into one bitmap: Roaring sources through [`Bitmap::fast_or`], tag-0 arrays
/// decoded, concatenated, sorted and folded in with `add_many`. Not `run_optimize`d; a caller
/// that persists the result does that itself.
pub fn union_postings<'a>(postings: impl IntoIterator<Item = PostingRef<'a>>) -> Bitmap {
    let mut views: Vec<BitmapView<'a>> = Vec::new();
    let mut small: Vec<u32> = Vec::new();
    for posting in postings {
        match posting {
            PostingRef::Roaring(view) => views.push(view),
            array => array.extend_into(&mut small),
        }
    }

    let refs: Vec<&Bitmap> = views.iter().map(|view| &**view).collect();
    let mut union = if refs.is_empty() {
        Bitmap::new()
    } else {
        Bitmap::fast_or(&refs)
    };
    small.sort_unstable();
    union.add_many(&small);
    union
}

/// Reads `postings.arrow`. Holds the backing bytes, either an owned buffer or a memory map;
/// [`PostingRef`]s returned by [`PostingsReader::posting`] borrow from it without copying.
#[derive(Debug)]
pub struct PostingsReader {
    array: LargeBinaryArray,
}

impl PostingsReader {
    /// Open `path`. When `mmap` is `true`, the file is memory-mapped and decoded zero-copy from
    /// the map; when `false`, it is read into an owned buffer and decoded zero-copy from that.
    /// Either way, no per-record copy happens on open or on lookup.
    pub fn open(path: &Path, mmap: bool) -> io::Result<Self> {
        let array = open_record_column(path, mmap, POSTING_COLUMN_NAME)?;
        validate_records(&array)?;
        Ok(PostingsReader { array })
    }

    /// The number of terms (records) in this postings file.
    pub fn term_count(&self) -> u32 {
        self.array.len() as u32
    }

    /// Return term `t`'s postings, or `None` if this file carries no record for it.
    ///
    /// `None` is an ordinary answer, not a failure: a flush publishes a sparse tier holding only
    /// the terms present in its flushed set, and a promoted descriptor's ordinal sits at or
    /// above the base's term count, so the base has nothing for it either.
    pub fn posting(&self, t: TermId) -> io::Result<Option<PostingRef<'_>>> {
        self.posting_at(t.raw())
    }

    /// The same lookup, addressed by a bare record ordinal. Prefer [`Self::posting`] inside this
    /// crate.
    pub fn posting_at(&self, ordinal: u32) -> io::Result<Option<PostingRef<'_>>> {
        let idx = ordinal as usize;
        if idx >= self.array.len() {
            return Ok(None);
        }
        read_posting(&self.array, idx).map(Some)
    }
}

/// Open a file of one non-null `LargeBinary` column named `column`, as [`PostingsSpool`] writes
/// it, and return that column without copying its records. When `mmap` is `true` the file is
/// memory-mapped, and otherwise read into an owned buffer.
pub fn open_record_column(path: &Path, mmap: bool, column: &str) -> io::Result<LargeBinaryArray> {
    let buffer = if mmap {
        mapped_buffer(&File::open(path)?)?
    } else {
        Buffer::from_vec(std::fs::read(path)?)
    };
    let what = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let batch = decode_single_batch(&buffer, &what)?;
    if batch.num_columns() != 1 {
        return Err(invalid_data(format!(
            "{what}: expected exactly one column, found {}",
            batch.num_columns()
        )));
    }
    let field = batch.schema_ref().field(0).clone();
    if field.name() != column || field.data_type() != &DataType::LargeBinary {
        return Err(invalid_data(format!(
            "{what}: expected LargeBinary column named '{column}', found '{}' ({:?})",
            field.name(),
            field.data_type()
        )));
    }
    batch
        .column(0)
        .as_any()
        .downcast_ref::<LargeBinaryArray>()
        .cloned()
        .ok_or_else(|| invalid_data(format!("{what}: column 0 is not a LargeBinaryArray")))
}

/// Decode record `idx` of a validated posting column into a borrowed [`PostingRef`]. Shared by
/// [`PostingsReader`] and [`crate::DeltaTier`], so the `unsafe` below is discharged in one place.
pub(crate) fn read_posting(array: &LargeBinaryArray, idx: usize) -> io::Result<PostingRef<'_>> {
    let bytes = array.value(idx);
    let (tag, payload) = bytes
        .split_first()
        .ok_or_else(|| invalid_data(format!("posting record {idx} has no tag byte")))?;

    match tag {
        0 => Ok(PostingRef::Array(payload)),
        1 => {
            // SAFETY: every tag-1 payload was validated once, at open time, by
            // `validate_records`, which confirms the payload is exactly the bitmap's serialised
            // size with no truncation or trailing garbage.
            let view = unsafe { BitmapView::deserialize::<Portable>(payload) };
            Ok(PostingRef::Roaring(view))
        }
        other => Err(invalid_data(format!(
            "posting record {idx} has unknown tag byte {other}"
        ))),
    }
}

/// Validate every record in `array` once, at `open` time, so later lookups never hand unchecked
/// bytes to the unsafe zero-copy `BitmapView::deserialize` for tag-1 records. A malformed record
/// fails `open` closed rather than causing undefined behaviour on first lookup.
pub(crate) fn validate_records(array: &LargeBinaryArray) -> io::Result<()> {
    for idx in 0..array.len() {
        let bytes = array.value(idx);
        let (tag, payload) = bytes
            .split_first()
            .ok_or_else(|| invalid_data(format!("postings.arrow: term {idx} has no tag byte")))?;

        match tag {
            0 => {
                if payload.len() % 4 != 0 {
                    return Err(invalid_data(format!(
                        "postings.arrow: term {idx} tag-0 payload length {} is not a multiple \
                         of 4",
                        payload.len()
                    )));
                }
            }
            1 => {
                let bitmap = Bitmap::try_deserialize::<Portable>(payload).ok_or_else(|| {
                    invalid_data(format!(
                        "postings.arrow: term {idx} tag-1 payload is not a valid portable \
                         Roaring bitmap"
                    ))
                })?;
                let consumed = bitmap.get_serialized_size_in_bytes::<Portable>();
                if consumed != payload.len() {
                    return Err(invalid_data(format!(
                        "postings.arrow: term {idx} tag-1 payload has {} trailing byte(s) \
                         beyond the {consumed}-byte serialised bitmap (payload is \
                         {} bytes) — BitmapView::deserialize requires an exact-length buffer",
                        payload.len().saturating_sub(consumed),
                        payload.len()
                    )));
                }
            }
            other => {
                return Err(invalid_data(format!(
                    "postings.arrow: term {idx} has unknown tag byte {other}"
                )))
            }
        }
    }
    Ok(())
}

/// Decode the single record batch of an Arrow IPC FILE held in `buffer`, without copying its
/// buffers, subject to alignment. `what` names the artefact in error messages. Public because
/// the attribute value column is read the same way, for the same reason.
pub fn decode_single_batch(buffer: &Buffer, what: &str) -> io::Result<RecordBatch> {
    const FOOTER_TRAILER_LEN: usize = 10; // 4-byte footer length + 6-byte "ARROW1" magic
    if buffer.len() < FOOTER_TRAILER_LEN {
        return Err(invalid_data(format!("{what}: file too short to contain a footer")));
    }

    let trailer_start = buffer.len() - FOOTER_TRAILER_LEN;
    let trailer: [u8; FOOTER_TRAILER_LEN] = buffer[trailer_start..]
        .try_into()
        .expect("view length matches FOOTER_TRAILER_LEN");
    let footer_len =
        read_footer_length(trailer).map_err(|e| invalid_data(format!("{what}: {e}")))?;
    if footer_len > trailer_start {
        return Err(invalid_data(format!("{what}: footer length exceeds file size")));
    }

    let footer = root_as_footer(&buffer[trailer_start - footer_len..trailer_start])
        .map_err(|e| invalid_data(format!("{what}: invalid footer: {e}")))?;

    let schema_fb = footer
        .schema()
        .ok_or_else(|| invalid_data(format!("{what}: footer has no schema")))?;
    let schema: SchemaRef = Arc::new(
        arrow::ipc::convert::try_fb_to_schema(schema_fb)
            .map_err(|e| invalid_data(format!("{what}: invalid schema: {e}")))?,
    );

    let version: MetadataVersion = footer.version();
    let mut decoder = FileDecoder::new(schema, version);

    if let Some(dictionaries) = footer.dictionaries() {
        for block in dictionaries.iter() {
            let (offset, block_len) = checked_block_range(block, buffer.len(), what)?;
            let data = buffer.slice_with_length(offset, block_len);
            decoder
                .read_dictionary(block, &data)
                .map_err(|e| invalid_data(format!("{what}: {e}")))?;
        }
    }

    let batches = footer
        .recordBatches()
        .ok_or_else(|| invalid_data(format!("{what}: footer has no record batches")))?;
    if batches.len() != 1 {
        return Err(invalid_data(format!(
            "{what}: expected exactly one record batch, found {}",
            batches.len()
        )));
    }

    let block = batches.get(0);
    let (offset, block_len) = checked_block_range(block, buffer.len(), what)?;
    let data = buffer.slice_with_length(offset, block_len);

    decoder
        .read_record_batch(block, &data)
        .map_err(|e| invalid_data(format!("{what}: {e}")))?
        .ok_or_else(|| invalid_data(format!("{what}: record batch block decoded to nothing")))
}

/// Validate a footer `Block`'s `(offset, bodyLength + metaDataLength)` against the file length,
/// returning them as checked `usize`s. A corrupt footer could report a negative value or an
/// overflowing sum, and `Buffer::slice_with_length` panics on out-of-bounds input, so every
/// field is checked here first.
fn checked_block_range(
    block: &arrow::ipc::Block,
    buffer_len: usize,
    what: &str,
) -> io::Result<(usize, usize)> {
    let offset = usize::try_from(block.offset())
        .map_err(|_| invalid_data(format!("{what}: block offset is negative")))?;
    let body_len = usize::try_from(block.bodyLength())
        .map_err(|_| invalid_data(format!("{what}: block bodyLength is negative")))?;
    let meta_len = usize::try_from(block.metaDataLength())
        .map_err(|_| invalid_data(format!("{what}: block metaDataLength is negative")))?;
    let block_len = body_len
        .checked_add(meta_len)
        .ok_or_else(|| invalid_data(format!("{what}: block length overflows")))?;
    let end = offset
        .checked_add(block_len)
        .ok_or_else(|| invalid_data(format!("{what}: block offset + length overflows")))?;
    if end > buffer_len {
        return Err(invalid_data(format!(
            "{what}: block range [{offset}, {end}) exceeds file length {buffer_len}"
        )));
    }
    Ok((offset, block_len))
}

pub(crate) fn invalid_data(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

fn invalid_input(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rand::SeedableRng;
    use std::collections::BTreeSet;
    use tempfile::TempDir;

    #[test]
    fn threshold_boundary_32_is_array_33_is_roaring() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("postings.arrow");

        let at_threshold: Vec<u32> = (0..32).collect();
        let over_threshold: Vec<u32> = (0..33).collect();
        write_postings(&path, &[at_threshold.clone(), over_threshold.clone()], 32).unwrap();

        let reader = PostingsReader::open(&path, false).unwrap();
        match reader
            .posting(TermId::new(0))
            .unwrap()
            .expect("term 0 is present")
        {
            PostingRef::Array(bytes) => assert_eq!(bytes.len(), 32 * 4),
            PostingRef::Roaring(_) => panic!("count == threshold must stay tag 0"),
        };
        match reader
            .posting(TermId::new(1))
            .unwrap()
            .expect("term 1 is present")
        {
            PostingRef::Roaring(bm) => assert_eq!(bm.cardinality(), 33),
            PostingRef::Array(_) => panic!("count == threshold + 1 must be tag 1"),
        };
    }

    proptest! {
        #[test]
        fn random_per_term_sets_round_trip(
            seed in any::<u64>(),
            term_sizes in prop::collection::vec(0usize..200, 1..12),
            mmap in any::<bool>(),
        ) {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let mut per_term = Vec::new();
            for size in term_sizes {
                let mut set: BTreeSet<u32> = BTreeSet::new();
                while set.len() < size {
                    set.insert(rand::Rng::gen_range(&mut rng, 0..1_000_000u32));
                }
                per_term.push(set.into_iter().collect::<Vec<u32>>());
            }

            let temp = TempDir::new().unwrap();
            let path = temp.path().join("postings.arrow");
            write_postings(&path, &per_term, 32).unwrap();

            let reader = PostingsReader::open(&path, mmap).unwrap();
            prop_assert_eq!(reader.term_count() as usize, per_term.len());

            for (t, expected) in per_term.iter().enumerate() {
                let got: Vec<u32> = match reader.posting(TermId::new(t as u32)).unwrap().expect("every term is present") {
                    PostingRef::Array(bytes) => bytes
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|c| u32::from_le_bytes(*c))
                        .collect(),
                    PostingRef::Roaring(bm) => bm.iter().collect(),
                };
                prop_assert_eq!(&got, expected);
            }
        }
    }

    #[test]
    fn next_offset_rejects_i64_overflow() {
        assert_eq!(next_offset(i64::MAX - 4, 4).unwrap(), i64::MAX);
        let err = next_offset(i64::MAX, 1).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        let err = next_offset(i64::MAX - 3, 4).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn write_postings_rejects_unsorted_input() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("postings.arrow");
        let err = write_postings(&path, &[vec![5, 3, 9]], 32).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn write_postings_rejects_duplicate_entities() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("postings.arrow");
        let err = write_postings(&path, &[vec![3, 3, 9]], 32).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn record_bytes_match_the_tagged_format_exactly() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("postings.arrow");

        let singleton: Vec<u32> = vec![5];
        let large: Vec<u32> = (1..=1000u32).collect();
        write_postings(&path, &[singleton, large], 32).unwrap();

        let file = File::open(&path).unwrap();
        let mut reader = arrow::ipc::reader::FileReader::try_new(file, None).unwrap();
        let batch = reader.next().unwrap().unwrap();
        assert!(reader.next().is_none(), "expected exactly one record batch");

        let array = batch
            .column(0)
            .as_any()
            .downcast_ref::<LargeBinaryArray>()
            .unwrap();

        // Term 0: tag 0 ‖ u32 LE 5 -> exactly [0, 5, 0, 0, 0].
        assert_eq!(array.value(0), &[0u8, 5, 0, 0, 0]);

        assert_eq!(array.value(1)[0], 1u8);
    }

    #[test]
    fn open_rejects_wrong_column_schema() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("postings.arrow");
        write_wrong_type_column(&path).unwrap();

        let err = PostingsReader::open(&path, false).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn open_rejects_unknown_tag_byte() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("postings.arrow");
        write_raw_records(&path, &[&[2u8, 1, 2, 3, 4]]).unwrap();

        let err = PostingsReader::open(&path, false).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn open_rejects_zero_length_record() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("postings.arrow");
        write_raw_records(&path, &[&[]]).unwrap();

        let err = PostingsReader::open(&path, false).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn a_term_this_file_does_not_carry_reads_as_absent() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("postings.arrow");
        write_postings(&path, &[vec![1, 2, 3]], 32).unwrap();

        let reader = PostingsReader::open(&path, false).unwrap();
        assert!(reader.posting(TermId::new(5)).unwrap().is_none());
        assert!(reader.posting(TermId::new(0)).unwrap().is_some());
    }

    #[test]
    fn open_rejects_truncated_tag1_record() {
        let temp = TempDir::new().unwrap();
        let valid_path = temp.path().join("valid.arrow");
        let large: Vec<u32> = (1..=1000u32).collect();
        write_postings(&valid_path, &[large], 32).unwrap();

        let file = File::open(&valid_path).unwrap();
        let mut reader = arrow::ipc::reader::FileReader::try_new(file, None).unwrap();
        let batch = reader.next().unwrap().unwrap();
        let array = batch
            .column(0)
            .as_any()
            .downcast_ref::<LargeBinaryArray>()
            .unwrap();
        let full_record = array.value(0);
        assert_eq!(full_record[0], 1u8, "expected a tag-1 record to truncate");
        let truncated = &full_record[..full_record.len() / 2];

        let truncated_path = temp.path().join("truncated.arrow");
        write_raw_records(&truncated_path, &[truncated]).unwrap();

        let err = PostingsReader::open(&truncated_path, false).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    fn write_raw_records(path: &Path, records: &[&[u8]]) -> io::Result<()> {
        let mut builder = LargeBinaryBuilder::new();
        for record in records {
            builder.append_value(record);
        }
        let array = builder.finish();
        let schema = Arc::new(Schema::new(vec![Field::new(
            POSTING_COLUMN_NAME,
            DataType::LargeBinary,
            false,
        )]));
        let batch = RecordBatch::try_new(schema.clone(), vec![Arc::new(array)])
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        let file = File::create(path)?;
        let mut writer = FileWriter::try_new(BufWriter::new(file), &schema)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        writer
            .write(&batch)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        writer
            .finish()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        Ok(())
    }

    fn write_wrong_type_column(path: &Path) -> io::Result<()> {
        use arrow::array::UInt32Array;

        let array = UInt32Array::from(vec![1u32, 2, 3]);
        let schema = Arc::new(Schema::new(vec![Field::new(
            POSTING_COLUMN_NAME,
            DataType::UInt32,
            false,
        )]));
        let batch = RecordBatch::try_new(schema.clone(), vec![Arc::new(array)])
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        let file = File::create(path)?;
        let mut writer = FileWriter::try_new(BufWriter::new(file), &schema)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        writer
            .write(&batch)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        writer
            .finish()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        Ok(())
    }
}
