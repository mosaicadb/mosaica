//! Writing a value column: the whole-column construction, and the streaming one beside it.
//!
//! **This lives apart from [`crate::values`] because that module is the scan's hot path**, and
//! adding code to it has cost 30–80% five times over — most recently the extent functions, which
//! were moved to `extent.rs` to recover it. The mechanism is inlining: the scan's traversal hands
//! its work to a closure, and a closure that grows past the inliner's threshold stops being
//! inlined into `for_each_run`, at which point a call is paid per candidate entity. Nothing here
//! is reachable from a scan, so nothing here can be the code that grows it.
//!
//! # Why a value column can be written twice over
//!
//! [`write_value_column`] takes a whole [`Codes`] and borrows its buffers into the record batch
//! without copying them: that is the right shape when the column is already materialised, which is
//! what an extent's flush hands it.
//!
//! [`ValueColumnWriter`] is for the producer that does *not* have the column in hand — the fold,
//! which merges a base column with every per-flush extent, and the build, which walks its staged
//! values. At 10⁹ a `u32` column is 4 GB, and materialising it to hand to the one-shot writer is
//! exactly the transient the fold's memory plan says the pass avoids (`filter-index.md` §6.2).
//!
//! Streaming cannot be done by appending record batches: the reader decodes a **single** batch and
//! refuses a second (`read_values`), because it borrows its values from the batch's buffers rather
//! than copying them, and concatenating is the copy that construction exists to avoid. So this is
//! the repo's spool-then-assemble discipline — [`mosaica_authz::PostingsSpool`]'s, applied to this
//! format: values are spooled to a temporary file as they arrive, only the running count is held,
//! and `finish` memory-maps the spool as the array's own buffer and writes the one record batch
//! from it. The bytes are the bytes [`write_value_column`] would have written, which
//! `values_writer.rs`'s byte-identity test pins rather than asserts — the fold's "equivalent to a
//! single build" claim is byte-identity, so it has to be checked as one.
//!
//! **The spooled bytes are native-endian**, which is not a portability lapse: the target is
//! byte-identity with the column the one-shot writer builds, and that column is whatever the
//! machine's `ScalarBuffer` holds. Both paths therefore agree on any platform, and both are
//! little-endian on every platform this runs on.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::Arc;

use arrow::array::ArrayRef;
use arrow::buffer::{Buffer, ScalarBuffer};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use croaring::{Bitmap, Portable};

use crate::values::ordinals::{self, BlockEncoder, CodeBytes, Parts, Width};
use crate::values::Codes;

/// Which family and width a column stores, decided before its first value arrives.
///
/// The streaming writer needs this at creation rather than inferring it from the first chunk: a
/// column with no values at all still has a declared type, and a file whose type depended on
/// whether any entity carried a value would be a different artefact for the same schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnKind {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    /// `u32` codes, a keyword layer's ordinals or a `u32` category's codes, written a block at a
    /// time ([`crate::BlockOrdinals`]).
    /// Pushed as `u32` chunks like any other column; the encoding is chosen at
    /// [`ValueColumnWriter::finish`], once the presence that divides the column into blocks is known.
    Ordinals,
    /// A `u16` category's codes, written a block at a time. Pushed as `u16` chunks.
    Ordinals16,
}

impl ColumnKind {
    /// The kind a materialised chunk carries.
    pub fn of(codes: &Codes) -> Self {
        match codes {
            Codes::U8(_) => ColumnKind::U8,
            Codes::U16(_) => ColumnKind::U16,
            Codes::U32(_) => ColumnKind::U32,
            Codes::U64(_) => ColumnKind::U64,
            Codes::I8(_) => ColumnKind::I8,
            Codes::I16(_) => ColumnKind::I16,
            Codes::I32(_) => ColumnKind::I32,
            Codes::I64(_) => ColumnKind::I64,
            Codes::F32(_) => ColumnKind::F32,
            Codes::F64(_) => ColumnKind::F64,
            Codes::Ordinals(o) => match o.code_bytes() {
                CodeBytes::Two => ColumnKind::Ordinals16,
                CodeBytes::Four => ColumnKind::Ordinals,
            },
        }
    }

    /// The width of the codes a blocked kind writes, `None` for a column written flat.
    fn code_bytes(self) -> Option<CodeBytes> {
        match self {
            ColumnKind::Ordinals => Some(CodeBytes::Four),
            ColumnKind::Ordinals16 => Some(CodeBytes::Two),
            _ => None,
        }
    }

    /// Whether the column is written a block at a time.
    fn blocked(self) -> bool {
        self.code_bytes().is_some()
    }

    /// The Arrow type the file declares.
    fn arrow_type(self) -> DataType {
        match self {
            ColumnKind::U8 => DataType::UInt8,
            ColumnKind::U16 => DataType::UInt16,
            ColumnKind::U32 | ColumnKind::Ordinals => DataType::UInt32,
            ColumnKind::Ordinals16 => DataType::UInt16,
            ColumnKind::U64 => DataType::UInt64,
            ColumnKind::I8 => DataType::Int8,
            ColumnKind::I16 => DataType::Int16,
            ColumnKind::I32 => DataType::Int32,
            ColumnKind::I64 => DataType::Int64,
            ColumnKind::F32 => DataType::Float32,
            ColumnKind::F64 => DataType::Float64,
        }
    }

    /// Bytes per value as spooled. **Every value column is fixed-width**: a keyword's values are
    /// `u32` ordinals into its layer's dictionary, and the dictionary is a separate artefact this
    /// writer knows nothing about, so there is no variable-width case to spool an offset array for.
    /// Codes written a block at a time are spooled at their own width.
    fn width(self) -> usize {
        match self {
            ColumnKind::U8 | ColumnKind::I8 => 1,
            ColumnKind::U16 | ColumnKind::I16 | ColumnKind::Ordinals16 => 2,
            ColumnKind::U32 | ColumnKind::I32 | ColumnKind::F32 | ColumnKind::Ordinals => 4,
            ColumnKind::U64 | ColumnKind::I64 | ColumnKind::F64 => 8,
        }
    }
}

/// Serialise a presence bitmap in the **one** encoding every producer of a value column writes.
///
/// **The file's bytes must be a function of the entity set alone**, and without this they are a
/// function of how the producer's bitmap happened to be built: the build's presence arrives from
/// repeated `add`, where the fold's arrives as a union of its layers' `present()` — already
/// run-compressed for a universal base — and croaring serialises the two encodings differently.
/// The same entity set would then produce two different files, which is exactly the byte-identity
/// `filter-index.md` §6.2 claims between a folded column and a freshly built one. Run-optimising
/// here settles it in the direction that is also the smaller file.
fn presence_bytes(presence: &Bitmap) -> Vec<u8> {
    let mut normalised = presence.clone();
    normalised.run_optimize();
    normalised.serialize::<Portable>()
}

/// Write the entities a layer holds a value for, read back by [`crate::read_presence`].
pub fn write_presence(path: &Path, presence: &Bitmap) -> io::Result<()> {
    std::fs::write(path, presence_bytes(presence))
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// **A presence bitmap must have exactly one set bit per value.** A mismatch pairs every entity
/// after the discrepancy with another entity's value — a wrong answer with no symptom, which is
/// why [`crate::ValueColumn::partial`] refuses it at open. Refusing it at write as well means the
/// file never exists to be opened, and costs a bitmap cardinality: O(containers), not O(entities).
fn check_presence(values: usize, presence: Option<&Bitmap>) -> io::Result<()> {
    let Some(p) = presence else {
        return Ok(());
    };
    if p.cardinality() != values as u64 {
        return Err(invalid(format!(
            "value column: presence has {} entities but {} values were supplied",
            p.cardinality(),
            values
        )));
    }
    Ok(())
}

/// The single schema/batch/IPC-writer invocation behind both writers.
///
/// Byte-identity between the whole-column path and the streaming one requires this to be literally
/// the same code rather than two copies that could drift — the same argument
/// [`mosaica_authz::PostingsSpool`] and `write_posting_records` share their own writer under.
/// The column carries no validity buffer on either path: a value column's absences live in the
/// presence bitmap, and a spurious all-valid buffer would change the file's bytes.
fn write_value_array(path: &Path, array: ArrayRef, ty: DataType) -> io::Result<()> {
    let schema = Arc::new(Schema::new(vec![Field::new("value", ty, false)]));
    let batch =
        RecordBatch::try_new(schema.clone(), vec![array]).map_err(|e| invalid(e.to_string()))?;
    write_batch(path, &batch)
}

/// Write one record batch as an Arrow IPC file.
fn write_batch(path: &Path, batch: &RecordBatch) -> io::Result<()> {
    let file = File::create(path)?;
    let mut w = arrow::ipc::writer::FileWriter::try_new(file, batch.schema_ref())
        .map_err(|e| io::Error::other(e.to_string()))?;
    w.write(batch)
        .map_err(|e| io::Error::other(e.to_string()))?;
    w.finish().map_err(|e| io::Error::other(e.to_string()))?;
    Ok(())
}

/// Write a value column, and its presence bitmap where presence is partial.
///
/// `presence` is `None` when every entity in `0..codes.len()` carries a value. Writing an
/// all-ones bitmap instead would be correct and would cost the scan its fast path, so the
/// distinction is carried in the file set rather than in the bitmap's contents.
pub fn write_value_column(
    values_path: &Path,
    presence_path: &Path,
    codes: &Codes,
    presence: Option<&Bitmap>,
) -> io::Result<()> {
    use arrow::array::{UInt16Array, UInt32Array, UInt8Array};

    check_presence(codes.len(), presence)?;

    // The array borrows the `Codes` buffers rather than rebuilding them, so writing a column costs
    // no second copy of it.
    let (array, ty): (ArrayRef, DataType) = match codes {
        Codes::U8(v) => (Arc::new(UInt8Array::new(v.clone(), None)), DataType::UInt8),
        Codes::U16(v) => (
            Arc::new(UInt16Array::new(v.clone(), None)),
            DataType::UInt16,
        ),
        Codes::U32(v) => (
            Arc::new(UInt32Array::new(v.clone(), None)),
            DataType::UInt32,
        ),
        Codes::U64(v) => (
            Arc::new(arrow::array::UInt64Array::new(v.clone(), None)),
            DataType::UInt64,
        ),
        Codes::I8(v) => (
            Arc::new(arrow::array::Int8Array::new(v.clone(), None)),
            DataType::Int8,
        ),
        Codes::I16(v) => (
            Arc::new(arrow::array::Int16Array::new(v.clone(), None)),
            DataType::Int16,
        ),
        Codes::I32(v) => (
            Arc::new(arrow::array::Int32Array::new(v.clone(), None)),
            DataType::Int32,
        ),
        Codes::I64(v) => (
            Arc::new(arrow::array::Int64Array::new(v.clone(), None)),
            DataType::Int64,
        ),
        Codes::F32(v) => (
            Arc::new(arrow::array::Float32Array::new(v.clone(), None)),
            DataType::Float32,
        ),
        Codes::F64(v) => (
            Arc::new(arrow::array::Float64Array::new(v.clone(), None)),
            DataType::Float64,
        ),
        Codes::Ordinals(o) => {
            write_batch(values_path, &ordinals::batch_of_parts(o.parts())?)?;
            if let Some(p) = presence {
                write_presence(presence_path, p)?;
            }
            return Ok(());
        }
    };
    write_value_array(values_path, array, ty)?;

    if let Some(p) = presence {
        write_presence(presence_path, p)?;
    }
    Ok(())
}

/// One spool file: bytes appended as they arrive, mapped back as an Arrow buffer at assembly.
struct Spool {
    path: PathBuf,
    writer: BufWriter<File>,
    len: u64,
}

impl Spool {
    fn create(path: PathBuf) -> io::Result<Self> {
        // Read access as well as write: `map` memory-maps this same handle, and mapping a
        // write-only descriptor fails with EACCES.
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;
        Ok(Spool {
            path,
            writer: BufWriter::new(file),
            len: 0,
        })
    }

    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writer.write_all(bytes)?;
        self.len += bytes.len() as u64;
        Ok(())
    }

    /// Flush, fsync and map the spool as a buffer of exactly `expect` bytes.
    ///
    /// The length check is not ceremony: the buffer's length is what the array will read through,
    /// so a spool shorter than the offsets account for would hand the array bytes belonging to
    /// nothing, and a longer one would mean values were counted that were not written.
    fn map(self, expect: usize) -> io::Result<Buffer> {
        let file = self
            .writer
            .into_inner()
            .map_err(io::IntoInnerError::into_error)?;
        // The spool is about to be read back through a memory map; its bytes must be durable and
        // visible before the map is taken.
        file.sync_all()?;
        if self.len != expect as u64 {
            return Err(invalid(format!(
                "value column spool {} holds {} bytes but the column accounts for {expect}",
                self.path.display(),
                self.len
            )));
        }
        if expect == 0 {
            // memmap2 rejects zero-length maps, and an empty buffer is what the one-shot path
            // produces for an empty column anyway. Allocated rather than taken from an empty
            // `Vec`, whose dangling pointer carries `u8`'s alignment and would be rejected by
            // every wider `ScalarBuffer` — `MutableBuffer` is aligned for any of them.
            return Ok(arrow::buffer::MutableBuffer::new(0).into());
        }
        let mapping = unsafe { memmap2::Mmap::map(&file) }?;
        if mapping.len() != expect {
            return Err(invalid(format!(
                "value column spool {} mapped {} bytes, not the {expect} expected",
                self.path.display(),
                mapping.len()
            )));
        }
        let arc: Arc<memmap2::Mmap> = Arc::new(mapping);
        // SAFETY: the same argument as `PostingsSpool::finish`'s — `arc` owns the mapping for as
        // long as any `Buffer` built from it is alive (it is captured as the buffer's
        // `Allocation`), the mapping is valid for `expect` bytes for its whole lifetime, and
        // `memmap2::Mmap` never returns a null base pointer. The mapping's base is page-aligned,
        // which satisfies the alignment every `ScalarBuffer` built over it requires.
        let ptr = NonNull::new(arc.as_ptr() as *mut u8)
            .expect("memmap2::Mmap never returns a null base pointer");
        Ok(unsafe { Buffer::from_custom_allocation(ptr, expect, arc) })
    }
}

/// Bytes accumulated before a spool write. Values are converted one at a time, so this bounds the
/// conversion buffer at 64 KB whatever size chunk a caller pushes.
const SCRATCH: usize = 1 << 16;

/// A value column written in bounded chunks: the same `values.arrow` (and presence bitmap)
/// [`write_value_column`] produces, without the column ever being whole in memory.
///
/// Values arrive in **entity order** — for a partial column, in the order of the presence
/// bitmap's set bits, which is the same rank-addressed order the scan reads them back in.
///
/// The spool file is a sibling of `values.arrow` and is removed when the column is written; a
/// writer dropped without finishing removes it too, so an abandoned fold does not leave a 4 GB
/// transient behind.
pub struct ValueColumnWriter {
    kind: ColumnKind,
    values_path: PathBuf,
    presence_path: PathBuf,
    values: Option<Spool>,
    count: usize,
    scratch: Vec<u8>,
}

impl ValueColumnWriter {
    /// Create the writer for a column of `kind`, spooling beside `values_path`.
    pub fn create(values_path: &Path, presence_path: &Path, kind: ColumnKind) -> io::Result<Self> {
        let mut name = values_path.as_os_str().to_os_string();
        name.push(".spool");
        let values = Spool::create(PathBuf::from(name))?;
        Ok(ValueColumnWriter {
            kind,
            values_path: values_path.to_path_buf(),
            presence_path: presence_path.to_path_buf(),
            values: Some(values),
            count: 0,
            scratch: Vec::with_capacity(SCRATCH),
        })
    }

    /// How many values have been pushed. The presence bitmap handed to [`Self::finish`] must have
    /// exactly this cardinality.
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Append a chunk of values, in entity order.
    ///
    /// The chunk's family must match the declared kind. A chunk of the wrong width would be
    /// spooled as bytes and read back at the declared width — every value after it addressed
    /// wrongly, with no error anywhere — so it is refused rather than reinterpreted.
    pub fn push(&mut self, chunk: &Codes) -> io::Result<()> {
        let kind = ColumnKind::of(chunk);
        let accepted = kind == self.kind
            || matches!(
                (self.kind, kind),
                (ColumnKind::Ordinals, ColumnKind::U32) | (ColumnKind::Ordinals16, ColumnKind::U16)
            );
        if !accepted {
            return Err(invalid(format!(
                "value column at {}: declared {:?} but a chunk carried {kind:?}",
                self.values_path.display(),
                self.kind
            )));
        }
        if self.kind.blocked() {
            let mut at = 0;
            let mut codes = Vec::with_capacity(SCRATCH);
            while at < chunk.len() {
                let take = (chunk.len() - at).min(SCRATCH);
                codes.clear();
                chunk.extend_codes(at, take, &mut codes);
                for &c in &codes {
                    match self.kind {
                        ColumnKind::Ordinals16 => {
                            self.scratch.extend_from_slice(&(c as u16).to_ne_bytes())
                        }
                        _ => self.scratch.extend_from_slice(&c.to_ne_bytes()),
                    }
                }
                Self::flush_scratch(&mut self.values, &mut self.scratch)?;
                at += take;
            }
            self.count += chunk.len();
            return Ok(());
        }
        macro_rules! scalars {
            ($v:expr) => {{
                let v = $v;
                for x in v.iter() {
                    self.scratch.extend_from_slice(&x.to_ne_bytes());
                    if self.scratch.len() >= SCRATCH {
                        Self::flush_scratch(&mut self.values, &mut self.scratch)?;
                    }
                }
                Self::flush_scratch(&mut self.values, &mut self.scratch)?;
                self.count += v.len();
            }};
        }
        match chunk {
            Codes::U8(v) => scalars!(v),
            Codes::U16(v) => scalars!(v),
            Codes::U32(v) => scalars!(v),
            Codes::U64(v) => scalars!(v),
            Codes::I8(v) => scalars!(v),
            Codes::I16(v) => scalars!(v),
            Codes::I32(v) => scalars!(v),
            Codes::I64(v) => scalars!(v),
            Codes::F32(v) => scalars!(v),
            Codes::F64(v) => scalars!(v),
            Codes::Ordinals(_) => unreachable!("a blocked chunk is accepted only by a blocked kind"),
        }
        Ok(())
    }

    fn flush_scratch(values: &mut Option<Spool>, scratch: &mut Vec<u8>) -> io::Result<()> {
        if scratch.is_empty() {
            return Ok(());
        }
        let spool = values
            .as_mut()
            .expect("the values spool is taken only by finish, which consumes the writer");
        spool.write(scratch)?;
        scratch.clear();
        Ok(())
    }

    /// Assemble the column: one record batch over the mapped spool, and the presence bitmap where
    /// presence is partial. `presence` follows [`write_value_column`]'s rule exactly — `None`
    /// means every entity in `0..len()` carries a value.
    pub fn finish(mut self, presence: Option<&Bitmap>) -> io::Result<()> {
        check_presence(self.count, presence)?;
        Self::flush_scratch(&mut self.values, &mut self.scratch)?;
        let values = self
            .values
            .take()
            .expect("the values spool is taken only here");
        let values_spool = values.path.clone();

        let buffer = values.map(self.count * self.kind.width())?;
        if let Some(code_bytes) = self.kind.code_bytes() {
            match code_bytes {
                CodeBytes::Two => {
                    let pushed = ScalarBuffer::<u16>::new(buffer, 0, self.count);
                    write_ordinal_blocks(&self.values_path, &pushed, presence, code_bytes)?;
                }
                CodeBytes::Four => {
                    let pushed = ScalarBuffer::<u32>::new(buffer, 0, self.count);
                    write_ordinal_blocks(&self.values_path, &pushed, presence, code_bytes)?;
                }
            }
            std::fs::remove_file(&values_spool)?;
            if let Some(p) = presence {
                write_presence(&self.presence_path, p)?;
            }
            return Ok(());
        }
        macro_rules! borrowed {
            ($arr:ty) => {
                Arc::new(<$arr>::new(ScalarBuffer::new(buffer, 0, self.count), None)) as ArrayRef
            };
        }
        let array: ArrayRef = match self.kind {
            ColumnKind::U8 => borrowed!(arrow::array::UInt8Array),
            ColumnKind::U16 => borrowed!(arrow::array::UInt16Array),
            ColumnKind::U32 => borrowed!(arrow::array::UInt32Array),
            ColumnKind::U64 => borrowed!(arrow::array::UInt64Array),
            ColumnKind::I8 => borrowed!(arrow::array::Int8Array),
            ColumnKind::I16 => borrowed!(arrow::array::Int16Array),
            ColumnKind::I32 => borrowed!(arrow::array::Int32Array),
            ColumnKind::I64 => borrowed!(arrow::array::Int64Array),
            ColumnKind::F32 => borrowed!(arrow::array::Float32Array),
            ColumnKind::F64 => borrowed!(arrow::array::Float64Array),
            ColumnKind::Ordinals | ColumnKind::Ordinals16 => {
                unreachable!("blocked codes are written above")
            }
        };
        // The array — and with it the mapping — is dropped inside, so the spool is only removed
        // once `values.arrow` is fully written.
        write_value_array(&self.values_path, array, self.kind.arrow_type())?;
        std::fs::remove_file(&values_spool)?;
        if let Some(p) = presence {
            write_presence(&self.presence_path, p)?;
        }
        Ok(())
    }
}

impl Drop for ValueColumnWriter {
    /// A writer that never reached [`ValueColumnWriter::finish`] — an error part-way through a
    /// fold, a panic — leaves its spool behind otherwise, and that spool is the size of the column.
    fn drop(&mut self) {
        if let Some(spool) = self.values.take() {
            let _ = std::fs::remove_file(&spool.path);
        }
    }
}

/// A spool removed when dropped, so an abandoned write leaves none behind.
struct SpoolFile(PathBuf);

impl Drop for SpoolFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Append `values` to `spool` in native byte order, through `scratch`.
fn spool_values<T: Copy, const N: usize>(
    spool: &mut Spool,
    values: impl IntoIterator<Item = T>,
    bytes: impl Fn(T) -> [u8; N],
    scratch: &mut Vec<u8>,
) -> io::Result<()> {
    for v in values {
        scratch.extend_from_slice(&bytes(v));
        if scratch.len() >= SCRATCH {
            spool.write(scratch)?;
            scratch.clear();
        }
    }
    spool.write(scratch)?;
    scratch.clear();
    Ok(())
}

/// Write `values`, a column's codes in slot order, as [`crate::BlockOrdinals`]: each
/// block of the column `presence` addresses encoded at its smallest width. The encoded data is
/// spooled as it is produced, so only one block is held in memory.
fn write_ordinal_blocks<T: Copy + Into<u32>>(
    values_path: &Path,
    values: &[T],
    presence: Option<&Bitmap>,
    code_bytes: CodeBytes,
) -> io::Result<()> {
    let counts = ordinals::block_counts(values.len(), presence);
    let mut name = values_path.as_os_str().to_os_string();
    name.push(".blocks.spool");
    let guard = SpoolFile(PathBuf::from(name));
    let mut data = Spool::create(guard.0.clone())?;
    let mut widths = Vec::with_capacity(counts.len());
    let mut dict_lens = Vec::with_capacity(counts.len());
    let mut encoder = BlockEncoder::default();
    let mut wide: Vec<u32> = Vec::with_capacity(ordinals::BLOCK_LEN);
    let mut scratch = Vec::with_capacity(SCRATCH);
    let (mut slot, mut bytes) = (0usize, 0u64);
    for &count in &counts {
        wide.clear();
        wide.extend(values[slot..slot + count as usize].iter().map(|&c| c.into()));
        let block = wide.as_slice();
        slot += count as usize;
        let width = encoder.encode(block, code_bytes);
        let dict: &[u32] = match width {
            Width::Empty | Width::Plain => &[],
            Width::Const | Width::U8 | Width::U16 => &encoder.dict,
        };
        widths.push(width.tag());
        dict_lens.push(dict.len() as u32);
        spool_values(&mut data, dict.iter().copied(), u32::to_ne_bytes, &mut scratch)?;
        let positions = encoder.positions.iter().copied();
        match width {
            Width::U8 => {
                spool_values(&mut data, positions.map(|p| p as u8), u8::to_ne_bytes, &mut scratch)?
            }
            Width::U16 => spool_values(&mut data, positions, u16::to_ne_bytes, &mut scratch)?,
            Width::Plain => match code_bytes {
                CodeBytes::Two => {
                    let at = block.iter().map(|&c| c as u16);
                    spool_values(&mut data, at, u16::to_ne_bytes, &mut scratch)?
                }
                CodeBytes::Four => {
                    spool_values(&mut data, block.iter().copied(), u32::to_ne_bytes, &mut scratch)?
                }
            },
            Width::Empty | Width::Const => {}
        }
        let size = ordinals::block_bytes(width, count, dict.len() as u32, code_bytes);
        let written = 4 * dict.len() as u64 + match width {
            Width::U8 => u64::from(count),
            Width::U16 => 2 * u64::from(count),
            Width::Plain => code_bytes.bytes() * u64::from(count),
            Width::Empty | Width::Const => 0,
        };
        data.write(&[0u8; 3][..(size - written) as usize])?;
        bytes += size;
    }
    if slot != values.len() {
        return Err(invalid(format!(
            "value column at {}: its presence divides {slot} values into blocks but {} were \
             pushed",
            values_path.display(),
            values.len()
        )));
    }
    let parts = Parts {
        code_bytes,
        widths: ScalarBuffer::from(widths),
        counts: ScalarBuffer::from(counts),
        dict_lens: ScalarBuffer::from(dict_lens),
        data: ScalarBuffer::new(data.map(bytes as usize)?, 0, (bytes / 4) as usize),
    };
    write_batch(values_path, &ordinals::batch_of_parts(parts)?)?;
    drop(guard);
    Ok(())
}
