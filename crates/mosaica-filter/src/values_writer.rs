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
//! The reader decodes a **single** batch and refuses a second (`read_values`), because it borrows
//! its values from the batch's buffers rather than copying them. So a column written in chunks is
//! still one batch:
//!
//! - **A flat column** is laid out at its final size when the writer is created, which needs its
//!   value count, and each chunk is written at its offset as it arrives
//!   ([`mosaica_authz::arrow_file::InPlaceFile`]). Nothing is held and nothing is written twice.
//! - **Codes written a block at a time** cannot be: their encoded length is known only once the
//!   presence bitmap handed to `finish` divides them into blocks. They are spooled to a file beside
//!   the column as they arrive, and `finish` maps the spool, encodes it a block at a time, and
//!   writes the one record batch.
//!
//! Either way the bytes are the bytes [`write_value_column`] would have written, which
//! `values_writer.rs`'s byte-identity test pins rather than asserts — the fold's "equivalent to a
//! single build" claim is byte-identity, so it has to be checked as one.
//!
//! **Values are written native-endian**, which is not a portability lapse: the target is
//! byte-identity with the column the one-shot writer builds, and that column is whatever the
//! machine's `ScalarBuffer` holds. Both paths therefore agree on any platform, and both are
//! little-endian on every platform this runs on.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::array::ArrayRef;
use arrow::buffer::{Buffer, ScalarBuffer};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use croaring::{Bitmap, Portable};
use mosaica_authz::arrow_file::{mapped_buffer, write_single_batch, FileLayout, InPlaceFile};

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

/// The schema of a value column of type `ty`. The column carries no validity buffer of its own: a
/// value column's absences live in the presence bitmap.
fn value_schema(ty: DataType) -> Arc<Schema> {
    Arc::new(Schema::new(vec![Field::new("value", ty, false)]))
}

/// Write one record batch as an Arrow IPC file.
fn write_batch(path: &Path, batch: &RecordBatch) -> io::Result<()> {
    write_single_batch(path, batch.schema(), batch.columns().to_vec())
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
    write_single_batch(values_path, value_schema(ty), vec![array])?;

    if let Some(p) = presence {
        write_presence(presence_path, p)?;
    }
    Ok(())
}

/// One spool file: bytes appended as they arrive, mapped back as an Arrow buffer at assembly.
/// The file is removed however the spool ends; a mapping taken by [`Spool::map`] outlives it.
struct Spool {
    file: SpoolFile,
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
            file: SpoolFile(path),
            writer: BufWriter::new(file),
            len: 0,
        })
    }

    /// Flush and map the spool as a buffer of exactly `expect` bytes.
    ///
    /// The length check is not ceremony: the buffer's length is what the array will read through,
    /// so a spool shorter than the offsets account for would hand the array bytes belonging to
    /// nothing, and a longer one would mean values were counted that were not written.
    fn map(self, expect: usize) -> io::Result<Buffer> {
        let file = self
            .writer
            .into_inner()
            .map_err(io::IntoInnerError::into_error)?;
        if self.len != expect as u64 {
            return Err(invalid(format!(
                "value column spool {} holds {} bytes but the column accounts for {expect}",
                self.file.0.display(),
                self.len
            )));
        }
        // The map reads the page cache the flush wrote to; nothing reads the spool after a crash.
        let buffer = mapped_buffer(&file)?;
        if buffer.len() != expect {
            return Err(invalid(format!(
                "value column spool {} mapped {} bytes, not the {expect} expected",
                self.file.0.display(),
                buffer.len()
            )));
        }
        Ok(buffer)
    }
}

/// Where a column's bytes go as they are pushed.
enum Sink {
    /// A flat column's values buffer, written in place at `at`.
    InPlace { file: InPlaceFile, at: u64 },
    /// Codes waiting for presence to divide them into blocks.
    Spool(Spool),
}

/// Somewhere [`spool_values`] writes bytes in order.
trait Out {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()>;

    /// Write what `scratch` holds and empty it.
    fn drain(&mut self, scratch: &mut Vec<u8>) -> io::Result<()> {
        self.write(scratch)?;
        scratch.clear();
        Ok(())
    }
}

impl Out for Spool {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.writer.write_all(bytes)?;
        self.len += bytes.len() as u64;
        Ok(())
    }
}

impl Out for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        match self {
            Sink::InPlace { file, at } => {
                file.put(0, *at, bytes)?;
                *at += bytes.len() as u64;
                Ok(())
            }
            Sink::Spool(spool) => spool.write(bytes),
        }
    }
}

/// Bytes accumulated before a write. Values are converted one at a time, so this bounds the
/// conversion buffer at 64 KB whatever size chunk a caller pushes.
const SCRATCH: usize = 1 << 16;

/// A value column written in bounded chunks: the same `values.arrow` (and presence bitmap)
/// [`write_value_column`] produces, without the column ever being whole in memory.
///
/// Values arrive in **entity order** — for a partial column, in the order of the presence
/// bitmap's set bits, which is the same rank-addressed order the scan reads them back in.
///
/// A writer dropped without finishing removes what it wrote, so an abandoned fold leaves neither
/// a column-sized file nor a spool behind.
pub struct ValueColumnWriter {
    kind: ColumnKind,
    values_path: PathBuf,
    presence_path: PathBuf,
    sink: Sink,
    /// How many values the caller said it would push, where it said so.
    rows: Option<usize>,
    count: usize,
    scratch: Vec<u8>,
}

impl ValueColumnWriter {
    /// Create the writer for a column of `kind` that will be pushed `rows` values.
    ///
    /// A flat column is laid out at `values_path` now and needs `rows`. A column written a block
    /// at a time is spooled beside `values_path` until [`Self::finish`], and `rows`, where given,
    /// is checked there.
    pub fn create(
        values_path: &Path,
        presence_path: &Path,
        kind: ColumnKind,
        rows: Option<usize>,
    ) -> io::Result<Self> {
        let sink = if kind.blocked() {
            let mut name = values_path.as_os_str().to_os_string();
            name.push(".spool");
            Sink::Spool(Spool::create(PathBuf::from(name))?)
        } else {
            let rows = rows.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "value column at {}: a {kind:?} column is laid out before its values \
                         arrive, so its writer needs the number of values it will be pushed",
                        values_path.display()
                    ),
                )
            })?;
            let layout = FileLayout::new(value_schema(kind.arrow_type()), rows)?;
            Sink::InPlace {
                file: InPlaceFile::create(values_path, layout)?,
                at: 0,
            }
        };
        Ok(ValueColumnWriter {
            kind,
            values_path: values_path.to_path_buf(),
            presence_path: presence_path.to_path_buf(),
            sink,
            rows,
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
    /// written as bytes and read back at the declared width — every value after it addressed
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
        if let Some(rows) = self.rows.filter(|rows| self.count + chunk.len() > *rows) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "value column at {}: {} values pushed to a column created for {rows}",
                    self.values_path.display(),
                    self.count + chunk.len()
                ),
            ));
        }
        let sink = &mut self.sink;
        let scratch = &mut self.scratch;
        if self.kind.blocked() {
            let mut at = 0;
            let mut codes = Vec::with_capacity(SCRATCH);
            while at < chunk.len() {
                let take = (chunk.len() - at).min(SCRATCH);
                codes.clear();
                chunk.extend_codes(at, take, &mut codes);
                match self.kind {
                    ColumnKind::Ordinals16 => {
                        let narrow = codes.iter().map(|&c| c as u16);
                        spool_values(sink, narrow, u16::to_ne_bytes, scratch)?
                    }
                    _ => spool_values(sink, codes.iter().copied(), u32::to_ne_bytes, scratch)?,
                }
                at += take;
            }
            self.count += chunk.len();
            return Ok(());
        }
        macro_rules! scalars {
            ($v:expr) => {{
                let v = $v;
                spool_values(sink, v.iter().copied(), |x| x.to_ne_bytes(), scratch)?;
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

    /// Finish the column, and write the presence bitmap where presence is partial. `presence`
    /// follows [`write_value_column`]'s rule exactly — `None` means every entity in `0..len()`
    /// carries a value.
    pub fn finish(mut self, presence: Option<&Bitmap>) -> io::Result<()> {
        check_presence(self.count, presence)?;
        self.sink.drain(&mut self.scratch)?;
        if let Some(rows) = self.rows.filter(|rows| *rows != self.count) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "value column at {}: {} values pushed to a column created for {rows}",
                    self.values_path.display(),
                    self.count
                ),
            ));
        }
        match self.sink {
            Sink::InPlace { file, .. } => file.finish()?,
            Sink::Spool(spool) => {
                let code_bytes = self.kind.code_bytes().expect("only a blocked kind is spooled");
                let buffer = spool.map(self.count * code_bytes.bytes() as usize)?;
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
            }
        }
        if let Some(p) = presence {
            write_presence(&self.presence_path, p)?;
        }
        Ok(())
    }
}

/// A spool removed when dropped, so an abandoned write leaves none behind.
struct SpoolFile(PathBuf);

impl Drop for SpoolFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Append `values` to `sink` in native byte order, through `scratch`, which is written out
/// whenever it is full and carries its remainder to the next call: a caller pushing many short
/// runs costs one write a [`SCRATCH`], not one a run. [`Out::drain`] writes the last of it.
fn spool_values<T: Copy, const N: usize>(
    out: &mut impl Out,
    values: impl IntoIterator<Item = T>,
    bytes: impl Fn(T) -> [u8; N],
    scratch: &mut Vec<u8>,
) -> io::Result<()> {
    for v in values {
        scratch.extend_from_slice(&bytes(v));
        if scratch.len() >= SCRATCH {
            out.drain(scratch)?;
        }
    }
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
    let ranks = presence.map(mosaica_roaring::BlockRanks::of);
    let counts = ordinals::block_counts(values.len(), ranks.as_ref());
    let mut name = values_path.as_os_str().to_os_string();
    name.push(".blocks.spool");
    let mut data = Spool::create(PathBuf::from(name))?;
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
        scratch.extend_from_slice(&[0u8; 3][..(size - written) as usize]);
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
    data.drain(&mut scratch)?;
    let parts = Parts {
        code_bytes,
        widths: ScalarBuffer::from(widths),
        counts: ScalarBuffer::from(counts),
        dict_lens: ScalarBuffer::from(dict_lens),
        data: ScalarBuffer::new(data.map(bytes as usize)?, 0, (bytes / 4) as usize),
    };
    write_batch(values_path, &ordinals::batch_of_parts(parts)?)
}
