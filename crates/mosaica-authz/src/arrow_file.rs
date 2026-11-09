//! Arrow IPC files of exactly one record batch: the one writer, the one decoder, and the layout of
//! a fixed-width file filled in place.
//!
//! Every Arrow file in a bundle is one of these, and each is written through
//! [`write_single_batch`] or [`InPlaceFile`] and read through [`decode_single_batch`], so two files
//! that hold the same batch hold the same bytes, and every reader refuses the same things.
//!
//! ## A file filled in place
//!
//! The layout of a file carrying one batch of non-nullable fixed-width columns is a function of
//! the schema and the row count alone: every length in the IPC metadata is a fixed-width integer,
//! so the schema message, the record-batch metadata, each buffer's offset in the body and the
//! footer are all known before the first row exists. [`FileLayout`] learns them and
//! [`InPlaceFile`] reserves the file at its final size, writes that framing, and takes each
//! column's values at their own offset, in any order. Nothing is written twice and no column is
//! held whole.
//!
//! The framing is produced by Arrow's own `FileWriter`, never by a second encoder, run once over a
//! batch of the real schema and row count whose buffers are untouched anonymous mappings, into a
//! [`Recorder`] that keeps every byte it is handed except the values buffers, whose position and
//! length it records, and the validity bitmaps, which are all ones and are kept as their position
//! and length. An untouched mapping costs address space and no resident page, and the writer reads
//! none of it: no field of the metadata depends on a value. The writer does allocate each column's
//! bitmap, `rows / 8` bytes, while the layout is learnt.
//!
//! This depends on the writer handing each values buffer to the sink as the array's own bytes.
//! An `arrow` whose writer copied them would leave a column's values unrecognised, and the layout
//! is refused rather than made wrong: [`FileLayout::new`] checks every column's buffer was found
//! at its full length, and keeps no framing write longer than [`MAX_FRAMING_WRITE`] that is not
//! all ones, which a copy of an untouched mapping never is.

use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::Arc;

use arrow::array::{make_array, ArrayData, ArrayRef};
use arrow::buffer::{Buffer, MutableBuffer};
use arrow::datatypes::{DataType, SchemaRef};
use arrow::ipc::reader::{read_footer_length, FileDecoder};
use arrow::ipc::writer::FileWriter;
use arrow::ipc::{root_as_footer, root_as_message, MetadataVersion};
use arrow::record_batch::RecordBatch;

/// Write `columns` under `schema` as an Arrow IPC file of one record batch, uncompressed, at
/// `path`. Not synced: a torn file fails its manifest digest, and the caller syncs what it
/// publishes.
pub fn write_single_batch(
    path: &Path,
    schema: SchemaRef,
    columns: Vec<ArrayRef>,
) -> io::Result<()> {
    let batch =
        RecordBatch::try_new(schema.clone(), columns).map_err(|e| invalid_data(e.to_string()))?;
    let file = File::create(path)?;
    let mut writer = FileWriter::try_new(BufWriter::new(file), &schema)
        .map_err(|e| invalid_data(e.to_string()))?;
    writer
        .write(&batch)
        .map_err(|e| invalid_data(e.to_string()))?;
    writer.finish().map_err(|e| invalid_data(e.to_string()))?;
    writer
        .into_inner()
        .map_err(|e| invalid_data(e.to_string()))?
        .into_inner()
        .map_err(io::IntoInnerError::into_error)?;
    Ok(())
}

/// Memory-map `file` whole as an Arrow buffer, without copying. Empty files map as an empty
/// buffer.
pub fn mapped_buffer(file: &File) -> io::Result<Buffer> {
    map(file, false)
}

/// [`mapped_buffer`] for a file read once, front to back: the kernel is told so, and reads ahead
/// and drops behind. A kernel that declines the hint leaves a correct mapping, so that is no error.
pub fn mapped_buffer_streamed(file: &File) -> io::Result<Buffer> {
    map(file, true)
}

fn map(file: &File, sequential: bool) -> io::Result<Buffer> {
    if file.metadata()?.len() == 0 {
        // memmap2 refuses a zero-length map.
        return Ok(MutableBuffer::new(0).into());
    }
    let mapping = unsafe { memmap2::Mmap::map(file) }?;
    if sequential {
        let _ = mapping.advise(memmap2::Advice::Sequential);
    }
    let len = mapping.len();
    let arc: Arc<memmap2::Mmap> = Arc::new(mapping);
    // SAFETY: `arc` owns the mapping for as long as any Buffer built from it is alive, and the
    // mapping is valid for `len` bytes for its entire lifetime.
    let ptr = NonNull::new(arc.as_ptr() as *mut u8)
        .expect("memmap2::Mmap never returns a null base pointer");
    Ok(unsafe { Buffer::from_custom_allocation(ptr, len, arc) })
}

/// Read the file at `path` into an owned buffer aligned for any Arrow type, which
/// [`decode_single_batch`] and the record container ([`crate::records`]) require of the buffers
/// they borrow from.
pub fn read_buffer(path: &Path) -> io::Result<Buffer> {
    let mut file = File::open(path)?;
    let len = usize::try_from(file.metadata()?.len())
        .map_err(|_| invalid_data(format!("{}: larger than memory", path.display())))?;
    let mut buffer = MutableBuffer::from_len_zeroed(len);
    file.read_exact(buffer.as_slice_mut())?;
    Ok(buffer.into())
}

/// Decode the one record batch of an Arrow IPC file held in `buffer`, borrowing its buffers.
/// `what` names the file in error messages.
///
/// Refused: a file of any other number of batches, a dictionary-encoded column, a compressed
/// batch, and a buffer misaligned for its type. The last two would each decode by allocating a
/// copy, so a file mapped to avoid holding it would be held all the same, with no error.
pub fn decode_single_batch(buffer: &Buffer, what: &str) -> io::Result<RecordBatch> {
    const FOOTER_TRAILER_LEN: usize = 10; // 4-byte footer length + 6-byte "ARROW1" magic
    if buffer.len() < FOOTER_TRAILER_LEN {
        return Err(invalid_data(format!(
            "{what}: file too short to contain a footer"
        )));
    }

    let trailer_start = buffer.len() - FOOTER_TRAILER_LEN;
    let trailer: [u8; FOOTER_TRAILER_LEN] = buffer[trailer_start..]
        .try_into()
        .expect("view length matches FOOTER_TRAILER_LEN");
    let footer_len =
        read_footer_length(trailer).map_err(|e| invalid_data(format!("{what}: {e}")))?;
    if footer_len > trailer_start {
        return Err(invalid_data(format!(
            "{what}: footer length exceeds file size"
        )));
    }

    let footer = root_as_footer(&buffer[trailer_start - footer_len..trailer_start])
        .map_err(|e| invalid_data(format!("{what}: invalid footer: {e}")))?;

    if footer.dictionaries().is_some_and(|d| !d.is_empty()) {
        return Err(invalid_data(format!(
            "{what}: dictionary-encoded columns are not supported"
        )));
    }

    let schema_fb = footer
        .schema()
        .ok_or_else(|| invalid_data(format!("{what}: footer has no schema")))?;
    let schema: SchemaRef = Arc::new(
        arrow::ipc::convert::try_fb_to_schema(schema_fb)
            .map_err(|e| invalid_data(format!("{what}: invalid schema: {e}")))?,
    );

    let version: MetadataVersion = footer.version();
    let decoder = FileDecoder::new(schema, version).with_require_alignment(true);

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
    let (offset, body_len, meta_len) = checked_block_range(block, buffer.len(), what)?;
    let data = buffer.slice_with_length(offset, body_len + meta_len);
    reject_if_compressed(&data[..meta_len], what)?;

    decoder
        .read_record_batch(block, &data)
        .map_err(|e| invalid_data(format!("{what}: {e}")))?
        .ok_or_else(|| invalid_data(format!("{what}: record batch block decoded to nothing")))
}

/// Refuse a record-batch block whose body is compressed. Arrow offers no way to ask after the
/// decode, by which point the buffer has been decompressed into a copy, so the block's message
/// metadata is read here first.
fn reject_if_compressed(meta: &[u8], what: &str) -> io::Result<()> {
    let too_short = || invalid_data(format!("{what}: message metadata too short"));
    // A continuation marker, then the length; or, in the legacy framing, the length alone.
    let message = match meta {
        [0xff, 0xff, 0xff, 0xff, rest @ ..] => rest.get(4..).ok_or_else(too_short)?,
        [_, _, _, _, rest @ ..] => rest,
        _ => return Err(too_short()),
    };
    let message = root_as_message(message)
        .map_err(|e| invalid_data(format!("{what}: invalid message metadata: {e}")))?;
    let batch = message.header_as_record_batch().ok_or_else(|| {
        invalid_data(format!(
            "{what}: expected a record batch message, found header type {:?}",
            message.header_type()
        ))
    })?;
    if batch.compression().is_some() {
        return Err(invalid_data(format!(
            "{what}: compressed record batches are not supported; write it uncompressed"
        )));
    }
    Ok(())
}

/// A footer `Block`'s `(offset, bodyLength, metaDataLength)`, checked against the file length.
/// The fields are `i64` in the flatbuffer and `Buffer::slice_with_length` panics out of bounds, so
/// a corrupt footer is refused here first.
fn checked_block_range(
    block: &arrow::ipc::Block,
    buffer_len: usize,
    what: &str,
) -> io::Result<(usize, usize, usize)> {
    let offset = usize::try_from(block.offset())
        .map_err(|_| invalid_data(format!("{what}: block offset is negative")))?;
    let body_len = usize::try_from(block.bodyLength())
        .map_err(|_| invalid_data(format!("{what}: block bodyLength is negative")))?;
    let meta_len = usize::try_from(block.metaDataLength())
        .map_err(|_| invalid_data(format!("{what}: block metaDataLength is negative")))?;
    let end = body_len
        .checked_add(meta_len)
        .and_then(|len| offset.checked_add(len))
        .ok_or_else(|| invalid_data(format!("{what}: block offset + length overflows")))?;
    if end > buffer_len {
        return Err(invalid_data(format!(
            "{what}: block range [{offset}, {end}) exceeds file length {buffer_len}"
        )));
    }
    Ok((offset, body_len, meta_len))
}

pub(crate) fn invalid_data(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

pub(crate) fn invalid_input(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.into())
}

/// The largest write the framing keeps as bytes. Any larger write that is neither a values buffer
/// nor a validity bitmap is a buffer the [`Recorder`] failed to recognise, so the layout is
/// refused rather than kept as a column-sized `Vec`.
const MAX_FRAMING_WRITE: usize = 1 << 16;

/// How much of a validity bitmap is written per call.
const FILL_CHUNK: usize = 1 << 20;

/// Where every byte of a single-batch file of fixed-width columns goes, for a given schema and
/// row count.
pub struct FileLayout {
    /// The file's final length.
    len: u64,
    /// Everything that is not a values buffer, at its offset: magic, messages, padding, footer.
    framing: Vec<(u64, Vec<u8>)>,
    /// The validity bitmaps, at their offset and length: Arrow writes one for every column,
    /// non-nullable or not, `rows / 8` bytes of `0xFF`.
    validity: Vec<(u64, u64)>,
    /// Each column's values buffer: its offset in the file and its length. A column of no rows
    /// has an empty buffer and Arrow writes nothing for it.
    values: Vec<(u64, u64)>,
    /// One entry per column: the bytes of one value, or `None` for a `Boolean` column, whose
    /// buffer is one bit a row.
    widths: Vec<Option<usize>>,
    rows: usize,
}

impl FileLayout {
    /// The layout of a file of `rows` rows under `schema`, every column non-nullable and of a
    /// fixed width: a primitive type or `Boolean`.
    pub fn new(schema: SchemaRef, rows: usize) -> io::Result<FileLayout> {
        let widths: Vec<Option<usize>> = schema
            .fields()
            .iter()
            .map(|field| match field.data_type() {
                DataType::Boolean => Ok(None),
                ty => ty.primitive_width().map(Some).ok_or_else(|| {
                    invalid_input(format!(
                        "column '{}' is {ty:?}, which is not fixed-width, so a file of it cannot \
                         be laid out before its values are known; declare a fixed-width type",
                        field.name()
                    ))
                }),
            })
            .collect::<io::Result<_>>()?;

        // One untouched mapping per column for its values. Each span is the buffer's own length,
        // not the mapping's: two mappings can be adjacent, and a span rounded up would claim a
        // neighbour's first byte.
        let mut regions: Vec<(memmap2::MmapMut, usize)> = Vec::with_capacity(widths.len());
        for width in &widths {
            let bytes = buffer_bytes(*width, rows)?;
            regions.push((untouched(bytes)?, bytes));
        }

        let mut columns: Vec<ArrayRef> = Vec::with_capacity(widths.len());
        for (index, (region, bytes)) in regions.iter().enumerate() {
            // SAFETY: the mapping outlives every buffer built over it: `regions` is dropped below,
            // after the writer has finished and the batch is gone.
            let values = unsafe { over(region, *bytes) };
            // SAFETY: the buffer is the right length for `rows` values of the column's type, which
            // is all an unvalidated primitive or boolean array with no nulls needs.
            let data = unsafe {
                ArrayData::builder(schema.field(index).data_type().clone())
                    .len(rows)
                    .add_buffer(values)
                    .build_unchecked()
            };
            columns.push(make_array(data));
        }
        let batch = RecordBatch::try_new(schema.clone(), columns)
            .map_err(|e| invalid_input(e.to_string()))?;

        let mut sink = Recorder {
            at: 0,
            spans: regions
                .iter()
                .map(|(region, bytes)| (region.as_ptr() as usize, *bytes))
                .collect(),
            framing: Vec::new(),
            validity_runs: Vec::new(),
            values: vec![(0, 0); widths.len()],
            failed: None,
        };
        {
            let mut writer =
                FileWriter::try_new(&mut sink, &schema).map_err(|e| invalid_data(e.to_string()))?;
            writer
                .write(&batch)
                .map_err(|e| invalid_data(e.to_string()))?;
            writer.finish().map_err(|e| invalid_data(e.to_string()))?;
        }
        drop(batch);
        drop(regions);
        for (column, width) in widths.iter().enumerate() {
            let bytes = buffer_bytes(*width, rows)? as u64;
            if bytes > 0 && sink.values[column].1 != bytes {
                sink.failed.get_or_insert_with(|| {
                    format!(
                        "arrow's file writer did not hand over column '{}' as its own {bytes} \
                         bytes, so the in-place layout cannot say where its values belong. The \
                         writer's shape has changed and `arrow_file` has to follow it",
                        schema.field(column).name()
                    )
                });
            }
        }
        if let Some(message) = sink.failed {
            return Err(io::Error::new(io::ErrorKind::Unsupported, message));
        }
        Ok(FileLayout {
            len: sink.at,
            framing: sink.framing,
            validity: sink.validity_runs,
            values: sink.values,
            widths,
            rows,
        })
    }
}

/// A demand-paged anonymous mapping of at least `bytes`, which nothing reads.
fn untouched(bytes: usize) -> io::Result<memmap2::MmapMut> {
    memmap2::MmapOptions::new()
        .len(bytes.max(1))
        .map_anon()
        .map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("reserving {bytes} bytes of address space to lay out a file: {e}"),
            )
        })
}

/// `bytes` of `region` as a buffer that does not own it.
///
/// # Safety
///
/// The buffer must not outlive `region`.
unsafe fn over(region: &memmap2::MmapMut, bytes: usize) -> Buffer {
    let ptr = NonNull::new(region.as_ptr() as *mut u8).expect("a mapping's pointer is not null");
    unsafe { Buffer::from_custom_allocation(ptr, bytes, Arc::new(())) }
}

/// What the layout pass writes into: every byte but the values buffers is kept, and each values
/// buffer is recognised by the mapping its bytes come from.
struct Recorder {
    at: u64,
    spans: Vec<(usize, usize)>,
    framing: Vec<(u64, Vec<u8>)>,
    validity_runs: Vec<(u64, u64)>,
    values: Vec<(u64, u64)>,
    failed: Option<String>,
}

impl Write for Recorder {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let start = buf.as_ptr() as usize;
        let within = |&(base, len): &(usize, usize)| {
            len > 0 && start >= base && start + buf.len() <= base + len
        };
        match self.spans.iter().position(within) {
            Some(column) => self.values[column] = (self.at, buf.len() as u64),
            None if buf.len() <= MAX_FRAMING_WRITE => self.framing.push((self.at, buf.to_vec())),
            // A validity bitmap: Arrow allocates and writes one of ones for a column with no
            // nulls. A values buffer copied from its untouched mapping would be zeros, never this.
            None if buf.iter().all(|byte| *byte == 0xFF) => {
                self.validity_runs.push((self.at, buf.len() as u64))
            }
            None => {
                self.failed.get_or_insert_with(|| {
                    format!(
                        "arrow's file writer handed {} bytes that are neither one of the batch's \
                         buffers nor framing, so the in-place layout cannot say where they \
                         belong. The writer's shape has changed and `arrow_file` has to follow it",
                        buf.len()
                    )
                });
            }
        }
        self.at += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A file reserved at its final size, its framing written, filled column by column.
///
/// A file dropped before [`Self::finish`] is removed, so a failed write leaves no file of the
/// right length and the wrong contents behind.
pub struct InPlaceFile {
    file: File,
    path: PathBuf,
    layout: FileLayout,
    finished: bool,
}

impl InPlaceFile {
    /// Reserve the file at `path` and write its framing. Every values buffer reads as zero until
    /// it is filled.
    pub fn create(path: &Path, layout: FileLayout) -> io::Result<InPlaceFile> {
        let file = File::create(path)?;
        let created = InPlaceFile {
            file,
            path: path.to_path_buf(),
            layout,
            finished: false,
        };
        created.file.set_len(created.layout.len)?;
        for (at, bytes) in &created.layout.framing {
            created.file.write_all_at(bytes, *at)?;
        }
        // Each validity bitmap is all ones, written a chunk at a time.
        let longest = created.layout.validity.iter().map(|&(_, len)| len).max();
        let ones = vec![0xFFu8; FILL_CHUNK.min(longest.unwrap_or(0) as usize)];
        for &(at, len) in &created.layout.validity {
            let mut written = 0u64;
            while written < len {
                let take = ones.len().min((len - written) as usize);
                created.file.write_all_at(&ones[..take], at + written)?;
                written += take as u64;
            }
        }
        Ok(created)
    }

    /// How many rows the layout was made for.
    pub fn rows(&self) -> usize {
        self.layout.rows
    }

    /// The bytes of one value in column `column`, or `None` for a `Boolean` column.
    pub fn width(&self, column: usize) -> Option<usize> {
        self.layout.widths.get(column).copied().flatten()
    }

    /// Write `bytes` into column `column`'s values buffer, `at` bytes in.
    ///
    /// `at` is `row × width`, in native byte order. A `Boolean` column's buffer is one bit a row,
    /// least significant first, so `at` is `row / 8` and a run must start on a byte boundary.
    pub fn put(&self, column: usize, at: u64, bytes: &[u8]) -> io::Result<()> {
        let (offset, len) = *self.layout.values.get(column).ok_or_else(|| {
            invalid_input(format!(
                "{}: there is no column {column}; the file has {}",
                self.path.display(),
                self.layout.values.len()
            ))
        })?;
        at.checked_add(bytes.len() as u64)
            .filter(|end| *end <= len)
            .ok_or_else(|| {
                invalid_input(format!(
                    "{}: column {column} is {len} bytes and a run of {} at {at} would pass its end",
                    self.path.display(),
                    bytes.len()
                ))
            })?;
        self.file.write_all_at(bytes, offset + at)
    }

    /// Keep the file. It is not synced: the caller syncs what it publishes.
    pub fn finish(mut self) -> io::Result<()> {
        self.finished = true;
        Ok(())
    }
}

impl Drop for InPlaceFile {
    fn drop(&mut self) {
        if !self.finished {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// A column's values buffer in bytes: `rows` values, or `rows` bits for a `Boolean`.
fn buffer_bytes(width: Option<usize>, rows: usize) -> io::Result<usize> {
    match width {
        Some(width) => rows
            .checked_mul(width)
            .ok_or_else(|| invalid_input(format!("{rows} rows of {width} bytes overflow usize"))),
        None => Ok(rows.div_ceil(8)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{Array, Int64Array, UInt32Array};
    use arrow::datatypes::{Field, Schema};

    fn schema(ty: DataType) -> SchemaRef {
        Arc::new(Schema::new(vec![Field::new("value", ty, false)]))
    }

    fn written(dir: &Path, values: Vec<i64>) -> PathBuf {
        let path = dir.join("one.arrow");
        write_single_batch(
            &path,
            schema(DataType::Int64),
            vec![Arc::new(Int64Array::from(values))],
        )
        .unwrap();
        path
    }

    #[test]
    fn a_file_reads_back_from_a_map_and_from_an_owned_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = written(dir.path(), vec![-1, 2, 3]);
        for buffer in [
            mapped_buffer(&File::open(&path).unwrap()).unwrap(),
            read_buffer(&path).unwrap(),
        ] {
            let batch = decode_single_batch(&buffer, "one").unwrap();
            let column = batch
                .column(0)
                .as_any()
                .downcast_ref::<Int64Array>()
                .unwrap();
            assert_eq!(column.values().as_ref(), &[-1, 2, 3]);
        }
    }

    #[test]
    fn a_second_batch_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("two.arrow");
        let schema = schema(DataType::UInt32);
        let mut writer = FileWriter::try_new(File::create(&path).unwrap(), &schema).unwrap();
        for _ in 0..2 {
            let batch = RecordBatch::try_new(
                schema.clone(),
                vec![Arc::new(UInt32Array::from(vec![1u32]))],
            )
            .unwrap();
            writer.write(&batch).unwrap();
        }
        writer.finish().unwrap();
        decode_single_batch(&read_buffer(&path).unwrap(), "two").unwrap_err();
    }

    /// A buffer that cannot be borrowed at its type's alignment is refused, not copied.
    #[test]
    fn a_misaligned_buffer_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = written(dir.path(), vec![7; 64]);
        let mut shifted = vec![0u8];
        shifted.extend(std::fs::read(&path).unwrap());
        let buffer = Buffer::from_vec(shifted).slice(1);
        decode_single_batch(&buffer, "shifted").unwrap_err();
    }

    /// A dictionary-encoded column is refused: its values would not be the file's own buffers.
    #[test]
    fn a_dictionary_column_is_refused() {
        use arrow::array::DictionaryArray;
        use arrow::datatypes::Int32Type;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dict.arrow");
        let array: DictionaryArray<Int32Type> = vec!["a", "b", "a"].into_iter().collect();
        let schema = Arc::new(Schema::new(vec![Field::new(
            "value",
            array.data_type().clone(),
            false,
        )]));
        write_single_batch(&path, schema, vec![Arc::new(array)]).unwrap();
        decode_single_batch(&read_buffer(&path).unwrap(), "dict").unwrap_err();
    }

    /// A file filled in place is the file Arrow writes for the same batch, at a row count whose
    /// validity bitmaps are past the largest write the layout keeps as framing.
    #[test]
    fn a_file_filled_in_place_is_the_file_arrow_writes() {
        use arrow::array::{BooleanArray, UInt8Array};

        let dir = tempfile::tempdir().unwrap();
        let rows = 8 * MAX_FRAMING_WRITE + 13;
        let ints: Vec<i64> = (0..rows as i64).map(|i| i * 7 - 3).collect();
        let bytes: Vec<u8> = (0..rows).map(|i| i as u8).collect();
        let bools: Vec<bool> = (0..rows).map(|i| i % 3 == 0).collect();
        let schema = Arc::new(Schema::new(vec![
            Field::new("ints", DataType::Int64, false),
            Field::new("bytes", DataType::UInt8, false),
            Field::new("bools", DataType::Boolean, false),
        ]));

        let whole = dir.path().join("whole.arrow");
        let bool_array = BooleanArray::from(bools);
        let packed = bool_array.values().inner().as_slice().to_vec();
        write_single_batch(
            &whole,
            schema.clone(),
            vec![
                Arc::new(Int64Array::from(ints.clone())),
                Arc::new(UInt8Array::from(bytes.clone())),
                Arc::new(bool_array),
            ],
        )
        .unwrap();

        let in_place = dir.path().join("in-place.arrow");
        let file = InPlaceFile::create(&in_place, FileLayout::new(schema, rows).unwrap()).unwrap();
        let int_bytes: Vec<u8> = ints.iter().flat_map(|v| v.to_ne_bytes()).collect();
        file.put(0, 0, &int_bytes).unwrap();
        file.put(1, 0, &bytes).unwrap();
        file.put(2, 0, &packed).unwrap();
        file.finish().unwrap();

        assert_eq!(std::fs::read(&in_place).unwrap(), std::fs::read(&whole).unwrap());
    }

    #[test]
    fn a_file_dropped_before_it_is_finished_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("laid-out.arrow");
        let layout = FileLayout::new(schema(DataType::Int64), 3).unwrap();
        let file = InPlaceFile::create(&path, layout).unwrap();
        file.put(0, 0, &5i64.to_ne_bytes()).unwrap();
        drop(file);
        assert!(!path.exists());
    }

    #[test]
    fn a_column_of_no_fixed_width_has_no_layout() {
        FileLayout::new(schema(DataType::Utf8), 3).err().unwrap();
    }
}
