//! Segment file writers: `columns.arrow`, `morton.u32`, `permutation.bin` (contracts §2.1/§2.6).
//! Byte formats here are fixed by the contracts spec — do not vary them.
//!
//! ## One writer, several producers
//!
//! [`SegmentWriter`] takes segment rows one at a time, in row order, and holds none of them:
//! `columns.arrow` is laid out at its final size when the writer is created
//! ([`mosaica_authz::arrow_file::InPlaceFile`]) and each column's values are written at their own
//! offset as rows arrive, so each byte is written once. That needs the row count up front, which
//! every producer has: [`write_segment`] holds its rows, a merge sums its inputs, and the fold
//! counts the rows its tombstones leave.
//!
//! Its producers are [`write_segment`], for an already-sorted in-memory batch (flush), and the
//! k-way merges of [`crate::merge::execute_merge`] and [`crate::fold::fold_row_space`], for rows
//! streamed off mapped inputs. The build fills the same layout bucket by bucket
//! ([`columns_layout`]), and its bytes are held equal to this writer's for the same rows
//! (`segment_roundtrip::a_column_file_filled_in_place_is_byte_identical_to_one_written_whole`).

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow::datatypes::{DataType, Field, Schema, SchemaRef, TimeUnit};
use mosaica_authz::arrow_file::{FileLayout, InPlaceFile};

use mosaica_spatial::split32;
use mosaica_spatial::tiler::{ScalarType, ScalarValue, TilerItem};
use mosaica_types::{EntityId, MosaicaId};

use crate::permutation::{
    pages_for, payload_start, PAGE_ABSENT, PAGE_BYTES, PAGE_ENTRIES, PAGE_SHIFT,
};

const PERMUTATION_MAGIC: &[u8; 4] = b"MSPM";
/// The paged form. Version 1 was the flat array it replaced, and a reader refuses it on this field
/// alone (`crate::permutation`).
const PERMUTATION_VERSION: u16 = 3;
const PERMUTATION_ABSENT: u32 = 0xFFFF_FFFF;

/// The header's width, from the one module that defines the layout.
const PERMUTATION_HEADER_BYTES: usize = crate::permutation::HEADER_LEN;

/// The largest satisfiable `permutation.bin` bound: entity ids must fit `u32` in
/// `bundle_format = 1` (R1), so the highest addressable id is `2^32 - 1` and the bound — one past
/// it — is `2^32`. A bound above this names no entity that could ever occupy a slot.
const PERMUTATION_MAX_BOUND: u64 = 1 << 32;

/// Write `columns.arrow` (Arrow IPC file format, one record batch, uncompressed buffers) and
/// `morton.u32` (raw little-endian `u32` codes, no header) into `dir`.
///
/// `items` and `codes` must already be in row order (i.e. the output of
/// [`mosaica_spatial::tiler::sort_batch`]) and the same length; row *i*'s Morton code is
/// `codes[i]`. `scalar_schema` declares the name and Arrow type of each item's leading
/// `scalars`, in the order they appear in `TilerItem::scalars`, and `indexed` those of the values
/// after them, which the bands copy and `columns.arrow` does not hold.
///
/// **One of [`SegmentWriter`]'s producers, not a second writer** — see the module doc. The caller
/// already holds every row, so nothing here is streamed *in*; what the delegation buys is that
/// this path and the merge's cannot drift apart in layout.
pub fn write_segment(
    dir: &Path,
    items: &[TilerItem],
    codes: &[u32],
    scalar_schema: &[(String, ScalarType)],
    indexed: &[(String, ScalarType)],
) -> io::Result<()> {
    if items.len() != codes.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "write_segment: items.len() ({}) != codes.len() ({})",
                items.len(),
                codes.len()
            ),
        ));
    }

    let mut writer = SegmentWriter::create(dir, scalar_schema, indexed, items.len())?;
    let drawn = scalar_schema.len();
    for (item, code) in items.iter().zip(codes) {
        let (scalars, after) = item.scalars.split_at(drawn.min(item.scalars.len()));
        let value = |k: usize| {
            after.get(k).cloned().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "write_segment: mosaica_id {} has no value for indexed column {k}",
                        item.mosaica_id.raw()
                    ),
                )
            })
        };
        writer.append(SegmentRow {
            mosaica_id: item.mosaica_id,
            morton: *code,
            // The residual is the low half of the same `split32` whose high half the tiler
            // returned as this row's code, so the stored pair is one splitting of one position
            // rather than two derivations that could disagree.
            residual: split32(item.qx, item.qy).1,
            scalars,
            indexed: &value,
        })?;
    }
    writer.finish()?;
    Ok(())
}

/// Writes one segment's `cuts.u32` and `cell-codes.u32` from the Morton codes of its rows,
/// arriving in row order.
///
/// A cell's start and its code are written the first time a code is seen, so `cuts.u32` is the
/// run-length index of `morton.u32` and `cell-codes.u32` the code of each run:
/// `crate::read::CutIndex` and [`crate::bands`] describe what each is for. Nothing is held but the
/// previous code and two `BufWriter`s.
///
/// **One writer for every producer.** The build's bounded assembly emits `morton.u32` positionally
/// and [`SegmentWriter`] emits it row by row, so the two paths have no code in common but this; a
/// build that indexed its segment differently from a flush would serve two different selections
/// from one bundle.
///
/// **It counts nothing.** The cell count is the length of either file, and the build's
/// `OccupancyRun` derives the same figure from the same codes; a third counter would be a third
/// thing that could disagree.
pub struct CutWriter {
    out: BufWriter<File>,
    codes: BufWriter<File>,
    last: Option<u32>,
    row: u32,
}

impl CutWriter {
    /// Create `cuts.u32` and `cell-codes.u32` in `dir`, which must exist.
    pub fn create(dir: &Path) -> io::Result<Self> {
        Ok(CutWriter {
            out: BufWriter::new(File::create(dir.join(crate::read::CutIndex::FILE))?),
            codes: BufWriter::new(File::create(dir.join(crate::bands::CELL_CODES_FILE))?),
            last: None,
            row: 0,
        })
    }

    /// Take the next row's Morton code. Codes must arrive in the row order the segment is written
    /// in, which is ascending; a repeated code continues the cell it started.
    pub fn push(&mut self, morton: u32) -> io::Result<()> {
        if self.last != Some(morton) {
            self.out.write_all(&self.row.to_le_bytes())?;
            self.codes.write_all(&morton.to_le_bytes())?;
            self.last = Some(morton);
        }
        self.row += 1;
        Ok(())
    }

    /// Flush both files.
    pub fn finish(mut self) -> io::Result<()> {
        self.out.flush()?;
        self.codes.flush()
    }
}

/// One row of a segment: what [`SegmentWriter::append`] takes, in row order.
///
/// **The code and its residual are carried, never a coordinate.** Both producers already hold the
/// pair and neither may re-derive it — a merge reads it off its inputs' mapped bytes, and
/// dequantise-then-requantise would move every point by up to a cell on every merge, silently
/// (write-path §7).
pub struct SegmentRow<'a> {
    pub mosaica_id: MosaicaId,
    pub morton: u32,
    /// The low half of the row's 64-bit interleaved position; `morton` is the high half.
    pub residual: u32,
    /// The declared scalars, positionally against the `scalar_schema` the writer was created with.
    pub scalars: &'a [ScalarValue],
    /// The row's value of the `k`th of the `indexed` columns the writer was created with, or
    /// [`ScalarValue::Null`]. Asked only of a row the bands hold ([`crate::bands::BandWriter`]).
    pub indexed: &'a dyn Fn(usize) -> io::Result<ScalarValue>,
}

/// A [`SegmentRow::indexed`] for a writer created with no indexed columns, which is never asked.
pub fn no_indexed(k: usize) -> io::Result<ScalarValue> {
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("no indexed column {k} was declared to this writer"),
    ))
}

/// Writes one segment's `morton.u32` and `columns.arrow` from rows arriving in
/// `(morton, mosaica_id)` order, holding no row and no column.
///
/// See the module doc for why this is the only writer that takes rows. Memory is a staging buffer
/// of [`STAGE_BYTES`] per column and two `BufWriter`s.
///
/// **Every column is fixed-width.** `render` on a string type is refused at the declaration
/// (per-point-attributes §4.3), and a `utf8` column reaching here is refused at
/// [`SegmentWriter::create`], as it is at the build.
///
/// **Values are written native-endian; the interchange files are not.** A column is an Arrow
/// values buffer, which arrow reads at the host's own endianness. `morton.u32` and
/// `permutation.bin` are byte formats fixed little-endian by contracts §2.6.
pub struct SegmentWriter {
    morton: BufWriter<File>,
    cuts: CutWriter,
    bands: crate::bands::BandWriter,
    dir: PathBuf,
    schema: SchemaRef,
    columns: InPlaceFile,
    /// One per schema column, in schema order: `mosaica_id`, `residual`, then the declared
    /// scalars.
    staged: Vec<Staged>,
    rows: usize,
    /// The last `(morton, mosaica_id)` appended — the row-order check's only state.
    last_key: Option<(u32, u64)>,
}

/// How many bytes of a column are held before they are written at their offset.
const STAGE_BYTES: usize = 1 << 16;

/// One column's values not yet written, and where they go.
#[derive(Default)]
struct Staged {
    bytes: Vec<u8>,
    /// The offset in the column's values buffer that `bytes` begins at.
    at: u64,
    /// A `Boolean` column's partial byte, least significant bit first, and how many of its bits
    /// are rows.
    bits: u8,
    bit_len: u8,
}

impl Staged {
    fn write(&mut self, columns: &InPlaceFile, column: usize) -> io::Result<()> {
        columns.put(column, self.at, &self.bytes)?;
        self.at += self.bytes.len() as u64;
        self.bytes.clear();
        Ok(())
    }

    fn push(&mut self, bytes: &[u8], columns: &InPlaceFile, column: usize) -> io::Result<()> {
        self.bytes.extend_from_slice(bytes);
        if self.bytes.len() >= STAGE_BYTES {
            self.write(columns, column)?;
        }
        Ok(())
    }
}

impl SegmentWriter {
    /// Create the segment's files under `dir`, which must exist, for exactly `rows` rows.
    /// `scalar_schema` is `columns.arrow`'s tail; `indexed` is the columns the bands copy beside
    /// it, whose values each row's [`SegmentRow::indexed`] answers.
    pub fn create(
        dir: &Path,
        scalar_schema: &[(String, ScalarType)],
        indexed: &[(String, ScalarType)],
        rows: usize,
    ) -> io::Result<Self> {
        let schema = columns_schema(scalar_schema);
        let columns = InPlaceFile::create(
            &dir.join("columns.arrow"),
            FileLayout::new(schema.clone(), rows)?,
        )?;
        Ok(SegmentWriter {
            morton: BufWriter::new(File::create(dir.join("morton.u32"))?),
            cuts: CutWriter::create(dir)?,
            bands: crate::bands::BandWriter::create(dir, indexed)?,
            dir: dir.to_path_buf(),
            staged: (0..schema.fields().len()).map(|_| Staged::default()).collect(),
            schema,
            columns,
            rows: 0,
            last_key: None,
        })
    }

    /// Append the next row. Rows must arrive in `(morton, mosaica_id)` order — contracts §2.6's
    /// row order, which `tile_ranges` binary-searches.
    ///
    /// **A `debug_assert`, because the fail-closed backstop is at the read.**
    /// `MortonSlice::load` refuses a non-ascending `morton.u32` at every open, and the engine's
    /// merge unit loads the segment it just wrote before publishing it — so an out-of-order
    /// producer is a refused publication in release and a failed test in debug, never a bundle
    /// that serves nonsense.
    pub fn append(&mut self, row: SegmentRow<'_>) -> io::Result<()> {
        let key = (row.morton, row.mosaica_id.raw());
        debug_assert!(
            self.last_key.is_none_or(|last| last <= key),
            "SegmentWriter::append: rows must arrive in (morton, mosaica_id) order"
        );
        self.last_key = Some(key);
        if self.rows == self.columns.rows() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "write_segment: mosaica_id {} is a row past the {} the segment was created \
                     for",
                    row.mosaica_id.raw(),
                    self.columns.rows()
                ),
            ));
        }

        self.morton.write_all(&row.morton.to_le_bytes())?;
        self.cuts.push(row.morton)?;
        self.bands
            .push(row.mosaica_id.raw(), row.morton, row.residual, row.indexed)?;
        let columns = &self.columns;
        self.staged[0].push(&row.mosaica_id.raw().to_ne_bytes(), columns, 0)?;
        self.staged[1].push(&row.residual.to_ne_bytes(), columns, 1)?;
        for (idx, staged) in self.staged.iter_mut().enumerate().skip(FIXED_COLUMN_COUNT) {
            let field = self.schema.field(idx);
            let value = row.scalars.get(idx - FIXED_COLUMN_COUNT).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "write_segment: mosaica_id {} is missing scalar '{}' at index {}",
                        row.mosaica_id.raw(),
                        field.name(),
                        idx - FIXED_COLUMN_COUNT
                    ),
                )
            })?;
            append_scalar(staged, columns, idx, field, value, row.mosaica_id)?;
        }
        self.rows += 1;
        Ok(())
    }

    /// Write the last staged values, then the band file from the finished `columns.arrow`, and
    /// return the row count, which must be the count the writer was created for.
    pub fn finish(self) -> io::Result<usize> {
        let SegmentWriter {
            mut morton,
            cuts,
            bands,
            dir,
            columns,
            mut staged,
            rows,
            ..
        } = self;
        if rows != columns.rows() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "write_segment: {rows} rows were appended to a segment created for {}",
                    columns.rows()
                ),
            ));
        }
        morton.flush()?;
        drop(morton);
        cuts.finish()?;
        for (column, staged) in staged.iter_mut().enumerate() {
            // A `Boolean` column's trailing partial byte is real: without it a row count that is
            // not a multiple of eight loses up to seven values.
            if staged.bit_len > 0 {
                staged.bytes.push(staged.bits);
            }
            staged.write(&columns, column)?;
        }
        columns.finish()?;
        bands.finish(&dir)?;
        Ok(rows)
    }
}

/// The two columns contracts §2.6 fixes; everything after them is a declared scalar.
const FIXED_COLUMN_COUNT: usize = 2;

/// Stage one declared scalar, refusing a value that is not of the column's declared type.
///
/// **A mismatch fails the write rather than coercing or dropping.** Dropping shortens the column
/// and shifts every later row of it into another row's place — every value present, every value
/// against the wrong identity, and no error anywhere.
fn append_scalar(
    staged: &mut Staged,
    columns: &InPlaceFile,
    column: usize,
    field: &Field,
    value: &ScalarValue,
    mosaica_id: MosaicaId,
) -> io::Result<()> {
    macro_rules! put {
        ($v:expr) => {
            staged.push(&$v.to_ne_bytes(), columns, column)
        };
    }
    match (field.data_type(), value) {
        (DataType::Boolean, ScalarValue::Bool(v)) => {
            staged.bits |= u8::from(*v) << staged.bit_len;
            staged.bit_len += 1;
            if staged.bit_len < 8 {
                return Ok(());
            }
            let byte = staged.bits;
            (staged.bits, staged.bit_len) = (0, 0);
            staged.push(&[byte], columns, column)
        }
        (DataType::UInt8, ScalarValue::U8(v)) => put!(v),
        (DataType::UInt16, ScalarValue::U16(v)) => put!(v),
        (DataType::UInt32, ScalarValue::U32(v)) => put!(v),
        (DataType::UInt64, ScalarValue::U64(v)) => put!(v),
        (DataType::Int8, ScalarValue::I8(v)) => put!(v),
        (DataType::Int16, ScalarValue::I16(v)) => put!(v),
        (DataType::Int32, ScalarValue::I32(v)) => put!(v),
        (DataType::Int64, ScalarValue::I64(v)) => put!(v),
        (DataType::Timestamp(TimeUnit::Microsecond, None), ScalarValue::TimestampUs(v)) => put!(v),
        (DataType::Float32, ScalarValue::F32(v)) => put!(v),
        (DataType::Float64, ScalarValue::F64(v)) => put!(v),
        (ty, got) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "write_segment: mosaica_id {} scalar '{}' expected {ty:?}, got {got:?}",
                mosaica_id.raw(),
                field.name()
            ),
        )),
    }
}

/// `columns.arrow`'s schema: the fixed columns, then `scalars` in declared order.
pub fn columns_schema(scalars: &[(String, ScalarType)]) -> SchemaRef {
    let mut fields = fixed_fields();
    for (name, ty) in scalars {
        fields.push(Field::new(name, arrow_type_of(*ty), false));
    }
    Arc::new(Schema::new(fields))
}

/// The layout of a `columns.arrow` of `rows` rows carrying `scalars`, for a producer that fills
/// it in place in its own order, as the build does a Morton bucket at a time. Refuses a scalar
/// that is not fixed-width.
pub fn columns_layout(scalars: &[(String, ScalarType)], rows: usize) -> io::Result<FileLayout> {
    FileLayout::new(columns_schema(scalars), rows)
}

/// The fixed, non-nullable fields of `columns.arrow` (contracts §2.6), in column order. The reader
/// (`read::validate_schema`) checks each per column.
///
/// `residual` is the *low* half of the point's 64-bit interleaved position; the high half is
/// the cell code in `morton.u32`, and concatenating them recovers the whole. It replaces the
/// `x`/`y` `f32` pair. With `priority` cut too (decision 0046), the fixed table is **two columns
/// and 12 bytes per row**, where the original four were 18. Nothing reads a coordinate off a
/// segment: what is stored is the position in the grid's own units, at 32 bits per axis rather
/// than an `f32`'s 24-bit mantissa.
fn fixed_fields() -> Vec<Field> {
    // No `priority` column (decision 0046). It was 2 B/row written and read by nothing at query
    // time — the selection comparator reads the full `mosaica_id`, of which priority is the high
    // 16 bits — and format 1 is unpublished, so cutting it now is free where cutting it later is
    // a break. Re-adding it is additive (the reader matches columns by name) and is licensed the
    // day a measured prefix-scan optimisation asks for it.
    vec![
        Field::new("mosaica_id", DataType::UInt64, false),
        Field::new("residual", DataType::UInt32, false),
    ]
}

/// The Arrow type each declared scalar width becomes in `columns.arrow`'s schema. Paired with
/// `read::validate_schema`'s accepted set: a type added to one and not the other is a segment the
/// writer emits and the reader refuses.
pub(crate) fn arrow_type_of(ty: ScalarType) -> DataType {
    match ty {
        ScalarType::Bool => DataType::Boolean,
        ScalarType::U8 => DataType::UInt8,
        ScalarType::U16 => DataType::UInt16,
        ScalarType::U32 => DataType::UInt32,
        ScalarType::U64 => DataType::UInt64,
        ScalarType::I8 => DataType::Int8,
        ScalarType::I16 => DataType::Int16,
        ScalarType::I32 => DataType::Int32,
        ScalarType::I64 => DataType::Int64,
        ScalarType::F32 => DataType::Float32,
        ScalarType::F64 => DataType::Float64,
        // `None` for the timezone: these are instants, and a per-column zone would be a second
        // place a time's meaning is decided.
        ScalarType::TimestampUs => DataType::Timestamp(TimeUnit::Microsecond, None),
        // **Unreachable.** `render` on either string type is refused at the declaration, and a
        // `Utf8` column is refused by `columns_layout`, which every writer of the file lays it out
        // with.
        ScalarType::Utf8 | ScalarType::Keyword | ScalarType::Text => DataType::Utf8,
    }
}

/// Write `permutation.bin` (R4), the two-level paged entity→row map — see
/// [`crate::permutation`] for the byte layout and for why it is paged.
/// `items_in_row_order[i]` is the entity occupying row `i`; its slot gets `i`. Every other slot in
/// a page some entity does occupy reads the row-absent sentinel `0xFFFF_FFFF`, and a page no
/// entity occupies is not written at all.
///
/// Returns an error — never panics — if `bound` exceeds `2^32` (entity IDs are `u64` in general
/// but must fit `u32` in `bundle_format = 1`, R1, so no larger bound is satisfiable) or if any
/// entity ID is `>= bound`. The bound is checked before anything is allocated or opened, so an
/// unsatisfiable bound costs nothing; see [`PagePlan::new`].
pub fn write_permutation(
    path: &Path,
    items_in_row_order: &[EntityId],
    bound: u64,
) -> io::Result<()> {
    write_permutation_iter(path, items_in_row_order.iter().copied(), bound)
}

/// [`write_permutation`] over an iterator of entities in row order, so a caller holding its row
/// order in a packed form need not materialise a `Vec<EntityId>` (8 bytes per row) alongside it.
/// Byte-for-byte identical output — it is [`PermutationWriter`]'s second producer.
///
/// **The iterator is walked twice, which is why it must be `Clone`.** The first pass learns which
/// pages the view occupies; only then can the file be sized to them. The alternative — size for
/// every page, then compact — is what [`PermutationWriter::create`] does for the scatter producer
/// that cannot know its pages up front, and it costs a `bound × 4`-byte fill that this path
/// avoids entirely. Every caller here holds its row order in a `Vec` or derives it from a range,
/// so the second walk is a re-read of memory already in hand and nothing is buffered to enable it.
pub fn write_permutation_iter<I>(path: &Path, items_in_row_order: I, bound: u64) -> io::Result<()>
where
    I: IntoIterator<Item = EntityId>,
    I::IntoIter: Clone,
{
    let items = items_in_row_order.into_iter();
    let plan = PagePlan::of_entities(bound, items.clone())?;
    let mut writer = PermutationWriter::create_planned(path, &plan)?;
    for (row, entity_id) in items.enumerate() {
        let row = u32::try_from(row).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("write_permutation: row index {row} does not fit in u32"),
            )
        })?;
        writer.set(entity_id, row)?;
    }
    writer.finish()
}

/// Which pages of entity space a view occupies — everything [`PermutationWriter::create_planned`]
/// needs to lay the file out before a single row is scattered into it.
///
/// One `bool` per page: 64 KB at the `u32` entity ceiling, and a few bytes for the views that
/// motivate the paging.
#[derive(Debug, Clone)]
pub struct PagePlan {
    bound: u64,
    present: Vec<bool>,
}

impl PagePlan {
    /// An empty plan over `[0, bound)`.
    ///
    /// **The bound ceiling is checked here, before any file exists.** A caller deriving a bound
    /// from a corrupt shard's `high_water` gets a refusal from a `Vec` allocation, not from a
    /// filled volume.
    pub fn new(bound: u64) -> io::Result<Self> {
        check_bound(bound)?;
        let pages = usize::try_from(pages_for(bound)).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("write_permutation: bound {bound} does not fit in usize"),
            )
        })?;
        Ok(PagePlan {
            bound,
            present: vec![false; pages],
        })
    }

    /// Declare that `entity` will be given a row.
    pub fn insert(&mut self, entity: EntityId) -> io::Result<()> {
        let raw = entity.raw();
        if raw >= self.bound {
            return Err(out_of_bound(raw, self.bound));
        }
        self.present[(raw >> PAGE_SHIFT) as usize] = true;
        Ok(())
    }

    /// The plan for a view whose entities are `entities`, in any order.
    pub fn of_entities<I: IntoIterator<Item = EntityId>>(
        bound: u64,
        entities: I,
    ) -> io::Result<Self> {
        let mut plan = PagePlan::new(bound)?;
        for entity in entities {
            plan.insert(entity)?;
        }
        Ok(plan)
    }

    /// How many pages carry slots.
    pub fn present_pages(&self) -> usize {
        self.present.iter().filter(|&&p| p).count()
    }
}

/// Writes `permutation.bin` **through a mapping**, scattering `perm[entity] = row` in any order.
///
/// **Scatter order is free, which is why this is a writer and not an iterator.** The compaction
/// fold's pass 1 emits rows in `(morton, mosaica_id)` order and learns `perm[entity]` in that
/// order, which is *not* entity order — a sequential writer would have to buffer the whole
/// mapping to reorder, which is the cost this avoids. Bytes live in a mapping rather than in a
/// `Vec` for the same reason: anonymous memory the kernel can only swap, against page cache it can
/// write back, and `Permutation::load` treats the file the same way at read.
///
/// # Two constructors, one output
///
/// [`Self::create_planned`] is for a producer that knows its pages up front — the build, which
/// holds its row order in memory — and writes only those pages. [`Self::create`] is for the fold,
/// which does not: it lays out **every** page of `[0, bound)`, scatters into them, and compacts
/// the present ones down at [`Self::finish`]. The compaction moves each present page to its
/// canonical slot, which is never above where it already sits, so it is one forward `copy_within`
/// pass and a truncation.
///
/// The two produce **the same bytes for the same mapping** — the encoding is canonical, so there
/// is nothing left for them to disagree about, and
/// `segment_roundtrip::a_scattered_permutation_is_byte_identical_to_a_sequential_one` pins it.
/// What differs is what the write costs: the scatter path still writes `page_count × 256 KiB` of
/// sentinel before the first row lands (4 GB at a 10⁹-entity bound), and the artifact is small
/// only after the truncation. **The paging shrinks what a sparse view stores and maps, not what
/// the fold's scatter transiently dirties** — that is the same figure compaction §3's pre-flight
/// budget already carries.
pub struct PermutationWriter {
    file: File,
    map: memmap2::MmapMut,
    bound: u64,
    payload_start: usize,
    /// Where each page's slots live in the payload, or [`PAGE_ABSENT`] — one entry per page of
    /// `[0, bound)`, so this is also the page count. In the scatter layout it is the identity
    /// until [`Self::finish`] compacts it.
    slot_of_page: Vec<u32>,
    /// Scatter layout only: which pages a `set` has landed in. `None` is the planned layout, whose
    /// present set is fixed at construction.
    touched: Option<Vec<bool>>,
}

impl PermutationWriter {
    /// Create `path` holding every page of `[0, bound)`, every slot the row-absent sentinel,
    /// compacted to the pages actually written at [`Self::finish`].
    ///
    /// **The fill is not optional and not free.** A freshly extended file reads as zeros, and zero
    /// is row 0 — a real row belonging to a real entity — so an unfilled slot would serve one
    /// entity's coordinates under every id that never got a row. The sentinel is `0xFFFF_FFFF`, so
    /// this writes the whole payload as `0xFF` up front.
    ///
    /// **The bound ceiling is checked before the file is opened, and that ordering is the point.**
    /// The fill above is proportional to `bound`, so validating it afterwards means writing
    /// `bound × 4` bytes to disk in order to discover the caller asked for something no entity
    /// could ever occupy — 32 GB for a bound of `2^33`, paid in full before the error is raised.
    pub fn create(path: &Path, bound: u64) -> io::Result<Self> {
        let plan = PagePlan::new(bound)?;
        let page_count = plan.present.len();
        let mut writer = Self::open(path, bound, page_count, page_count)?;
        // The identity: page `p` scatters into slot `p`, and `finish` moves it down to the slot
        // its rank among the touched pages gives it.
        for (page, slot) in writer.slot_of_page.iter_mut().enumerate() {
            *slot = page as u32;
        }
        writer.touched = Some(vec![false; page_count]);
        Ok(writer)
    }

    /// Create `path` holding exactly the pages `plan` declares, every slot the row-absent
    /// sentinel. A [`Self::set`] for an entity in an undeclared page is an error, not a silent
    /// drop.
    pub fn create_planned(path: &Path, plan: &PagePlan) -> io::Result<Self> {
        let mut writer = Self::open(path, plan.bound, plan.present.len(), plan.present_pages())?;
        let mut next: u32 = 0;
        for (page, present) in plan.present.iter().enumerate() {
            if *present {
                writer.slot_of_page[page] = next;
                next += 1;
            }
        }
        Ok(writer)
    }

    fn open(path: &Path, bound: u64, page_count: usize, payload_pages: usize) -> io::Result<Self> {
        let payload_start = payload_start(page_count);
        let len = payload_start + payload_pages * PAGE_BYTES;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        file.set_len(len as u64)?;

        // SAFETY: this process created and sized the file two statements ago and holds the only
        // handle to it; nothing else maps or truncates it for the writer's lifetime. The
        // concurrently-truncated-backing-file hazard `Permutation::load` documents is the same
        // one and is an operational property, not one this call can check.
        let mut map = unsafe { memmap2::MmapMut::map_mut(&file) }?;
        map[..4].copy_from_slice(PERMUTATION_MAGIC);
        map[4..6].copy_from_slice(&PERMUTATION_VERSION.to_le_bytes());
        map[6..8].copy_from_slice(&(PAGE_SHIFT as u16).to_le_bytes());
        map[8..16].copy_from_slice(&bound.to_le_bytes());
        map[16..20].copy_from_slice(&(page_count as u32).to_le_bytes());
        // `present_count` and the directory are written at `finish`, when the scatter layout knows
        // them. The header's remaining bytes and the padding stay zero, which is what the reader
        // requires of the padding.
        map[payload_start..].fill(0xFF);

        Ok(PermutationWriter {
            file,
            map,
            bound,
            payload_start,
            slot_of_page: vec![PAGE_ABSENT; page_count],
            touched: None,
        })
    }

    /// Record that `entity` occupies `row`. Every check [`write_permutation_iter`] made is made
    /// here, at the same cost — the duplicate test is a slot read the scatter was doing anyway.
    ///
    /// **R1's "entity ids fit `u32`" is enforced by the bound, not by a second test here.**
    /// The constructors refuse any bound above `2^32`, so `raw < self.bound` already implies
    /// `raw < 2^32` and a separate u32-fit check could never fire. Reinstating one would read as
    /// live defence against a case the constructor has already made unreachable.
    pub fn set(&mut self, entity: EntityId, row: u32) -> io::Result<()> {
        let raw = entity.raw();
        if raw >= self.bound {
            return Err(out_of_bound(raw, self.bound));
        }
        // Closes a format ambiguity permanently: a row index equal to the row-absent sentinel
        // would be indistinguishable on disk from "entity has no row".
        if row == PERMUTATION_ABSENT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "write_permutation: row index {row} collides with the row-absent sentinel \
                     (0xFFFF_FFFF)"
                ),
            ));
        }
        let page = (raw >> PAGE_SHIFT) as usize;
        let slot = self.slot_of_page[page];
        if slot == PAGE_ABSENT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "write_permutation: entity id {raw} falls in page {page}, which the page plan \
                     does not hold"
                ),
            ));
        }
        if let Some(touched) = self.touched.as_mut() {
            touched[page] = true;
        }
        let at = self.payload_start
            + slot as usize * PAGE_BYTES
            + ((raw as usize) & (PAGE_ENTRIES - 1)) * 4;
        let existing = u32::from_le_bytes(
            self.map[at..at + 4]
                .try_into()
                .expect("a four-byte window is four bytes"),
        );
        if existing != PERMUTATION_ABSENT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "write_permutation: entity id {raw} appears at both row {existing} and row \
                     {row}"
                ),
            ));
        }
        // Explicit little-endian, never a native cast: the slot array is an interchange byte
        // format (contracts §2.6), unlike the spools above, which become in-memory Arrow buffers.
        self.map[at..at + 4].copy_from_slice(&row.to_le_bytes());
        Ok(())
    }

    /// Compact the scatter layout, write the directory, and flush.
    pub fn finish(mut self) -> io::Result<()> {
        if let Some(touched) = self.touched.take() {
            // Each present page moves to the slot its rank gives it, which is at or below where it
            // sits — so a forward pass never overwrites a page it has yet to move.
            let mut next: u32 = 0;
            for (page, written) in touched.iter().enumerate() {
                if !written {
                    self.slot_of_page[page] = PAGE_ABSENT;
                    continue;
                }
                let from = self.payload_start + page * PAGE_BYTES;
                let to = self.payload_start + next as usize * PAGE_BYTES;
                if from != to {
                    self.map.copy_within(from..from + PAGE_BYTES, to);
                }
                self.slot_of_page[page] = next;
                next += 1;
            }
        }
        let present: u32 = self
            .slot_of_page
            .iter()
            .filter(|&&slot| slot != PAGE_ABSENT)
            .count() as u32;
        self.map[20..24].copy_from_slice(&present.to_le_bytes());
        for (page, &slot) in self.slot_of_page.iter().enumerate() {
            let at = PERMUTATION_HEADER_BYTES + page * 4;
            self.map[at..at + 4].copy_from_slice(&slot.to_le_bytes());
        }
        let len = self.payload_start + present as usize * PAGE_BYTES;

        let PermutationWriter { map, file, .. } = self;
        map.flush()?;
        // The mapping is dropped before the file shrinks: reading through a mapping past a
        // truncation is a fault, not an error, and the scatter layout always shrinks unless every
        // page was written.
        drop(map);
        file.set_len(len as u64)?;
        Ok(())
    }
}

fn check_bound(bound: u64) -> io::Result<()> {
    if bound > PERMUTATION_MAX_BOUND {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "write_permutation: bound {bound} exceeds the largest satisfiable bound \
                 {PERMUTATION_MAX_BOUND} (entity ids must fit u32 in bundle_format = 1)"
            ),
        ));
    }
    Ok(())
}

fn out_of_bound(raw: u64, bound: u64) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("write_permutation: entity id {raw} is out of bound (bound = {bound})"),
    )
}
