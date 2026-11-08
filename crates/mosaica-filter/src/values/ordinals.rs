//! A column of codes, stored per block of 2¹⁶ entities in the narrowest encoding the block allows:
//! a keyword layer's ordinals, and a category's codes where its vocabulary is declared `u16` or
//! `u32` wide.
//!
//! A keyword ordinal is a `u32` position in the layer's sorted dictionary, and a category code a
//! number of the vocabulary's declared width, `w` bytes. Stored flat that is `w` bytes per entity
//! whatever the block holds, and a scan that misses the page cache reads every one of them. A block
//! here is the span of one Roaring container, so its present entities hold consecutive slots, and
//! it is stored one of four ways:
//!
//! | Width | Stored | Bytes for `n` values, `d` distinct |
//! |---|---|---|
//! | `Const` | the one code the block holds | 4 |
//! | `U8` | the block's distinct codes, sorted, and a byte per value indexing them | `4d + n` |
//! | `U16` | the same, with two bytes per value | `4d + 2n` |
//! | `Plain` | each value's code, at the column's width | `wn` |
//!
//! The writer takes the smallest, so no block is larger than flat storage. A block no entity
//! carries a value in is `Empty` and holds nothing.
//!
//! A scan turns its question about ordinals into one about each block's own positions (a binary
//! search for `eq`, `in` and a range, a test of each distinct ordinal for a set) and walks the
//! block's values through [`ValueColumn::walk_one`], the traversal every other column takes. A
//! block costs that translation and its walk whether or not it holds the value sought, and the
//! walk is the counted one, so a scan compares one value per candidate entity, as over flat
//! storage, whatever it is looking for.

use std::io;

use arrow::buffer::ScalarBuffer;
use croaring::Bitmap;
use mosaica_roaring::{span, BlockRanks, Blocks};

use super::{add_scan_work, counted, CodeSet, Packing, ScanWork, ValueColumn, PARALLEL_MIN};

/// Bits of an entity id below its block: a block is one Roaring container's span.
pub(crate) const BLOCK_BITS: u32 = 16;

/// Entities a block spans, and so the most values one holds.
pub(crate) const BLOCK_LEN: usize = 1 << BLOCK_BITS;

/// How one block stores its values. The tag is the byte the file holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Width {
    Empty,
    Const,
    U8,
    U16,
    Plain,
}

impl Width {
    pub(crate) fn tag(self) -> u8 {
        match self {
            Width::Empty => 0,
            Width::Const => 1,
            Width::U8 => 2,
            Width::U16 => 3,
            Width::Plain => 4,
        }
    }

    fn of_tag(tag: u8) -> Option<Width> {
        Some(match tag {
            0 => Width::Empty,
            1 => Width::Const,
            2 => Width::U8,
            3 => Width::U16,
            4 => Width::Plain,
            _ => return None,
        })
    }

    /// The smallest width for `n` values holding `d` distinct codes, `0 < d <= n <= BLOCK_LEN`, in
    /// a column whose codes are `code_bytes` wide. A tie goes to the narrower.
    pub(crate) fn smallest(n: usize, d: usize, code_bytes: CodeBytes) -> Width {
        let plain = code_bytes.bytes() as usize * n;
        if d == 1 && 4 < plain {
            return Width::Const;
        }
        if d <= 256 && 4 * d + n <= plain {
            Width::U8
        } else if 4 * d + 2 * n < plain {
            Width::U16
        } else {
            Width::Plain
        }
    }
}

/// How many values each block holds: a whole block's worth up to the last for a column every entity
/// carries a value in, and, for one only some carry a value in, the count its presence's `ranks`
/// give each block.
///
/// The writer cuts the column with this and the reader checks the file against it, so a file whose
/// blocks do not fit the column's addressing is refused at open rather than read off by one block.
pub(crate) fn block_counts(len: usize, ranks: Option<&BlockRanks>) -> Vec<u32> {
    match ranks {
        None => (0..len.div_ceil(BLOCK_LEN))
            .map(|k| (len - k * BLOCK_LEN).min(BLOCK_LEN) as u32)
            .collect(),
        Some(ranks) => ranks.blocks().map(|k| ranks.count_in(k) as u32).collect(),
    }
}

#[derive(Clone, Copy, Debug)]
struct Block {
    width: Width,
    count: u32,
    /// The block's first slot.
    slot0: u64,
    dict_len: u32,
    /// Where the block starts in [`BlockOrdinals::data`]: its dictionary, then its values.
    at: u64,
}

/// How wide a column's codes are, and so a `Plain` block's values: a keyword's ordinals and a `u32`
/// category's codes are four bytes, a `u16` category's two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeBytes {
    Two,
    Four,
}

impl CodeBytes {
    pub(crate) fn bytes(self) -> u64 {
        match self {
            CodeBytes::Two => 2,
            CodeBytes::Four => 4,
        }
    }

    fn of(bytes: &str) -> Option<CodeBytes> {
        match bytes {
            "2" => Some(CodeBytes::Two),
            "4" => Some(CodeBytes::Four),
            _ => None,
        }
    }
}

impl Width {
    /// Bytes one value takes in a column of `code_bytes` codes.
    fn value_bytes(self, code_bytes: CodeBytes) -> u64 {
        match self {
            Width::Empty | Width::Const => 0,
            Width::U8 => 1,
            Width::U16 => 2,
            Width::Plain => code_bytes.bytes(),
        }
    }
}

/// Bytes a block takes in [`Parts::data`]: its dictionary's codes, its values, and the padding
/// that starts the next block on a four-byte boundary.
pub(crate) fn block_bytes(width: Width, count: u32, dict_len: u32, code_bytes: CodeBytes) -> u64 {
    (4 * u64::from(dict_len) + width.value_bytes(code_bytes) * u64::from(count))
        .next_multiple_of(4)
}

/// The arrays a [`BlockOrdinals`] is stored as, each one column of the file, and the width of its
/// codes, which the file records beside them.
pub(crate) struct Parts {
    pub code_bytes: CodeBytes,
    /// One [`Width::tag`] per block.
    pub widths: ScalarBuffer<u8>,
    /// Values per block.
    pub counts: ScalarBuffer<u32>,
    /// Distinct ordinals per block: one for `Const`, the dictionary's length for `U8` and `U16`,
    /// none for `Empty` and `Plain`.
    pub dict_lens: ScalarBuffer<u32>,
    /// The blocks in order, each its distinct ordinals ascending, then its values at its width
    /// (positions in that dictionary, or codes for `Plain`) in native byte order, padded to a
    /// whole `u32`. A block is one stretch of the file, so a scan reads the file in order.
    pub data: ScalarBuffer<u32>,
}

/// A column's codes in slot order, stored per block (see the module header).
#[derive(Debug)]
pub struct BlockOrdinals {
    len: usize,
    code_bytes: CodeBytes,
    blocks: Vec<Block>,
    widths: ScalarBuffer<u8>,
    counts: ScalarBuffer<u32>,
    dict_lens: ScalarBuffer<u32>,
    data: ScalarBuffer<u32>,
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

impl BlockOrdinals {
    /// Check `parts` describe a whole column and index it. The data's length must be what the
    /// blocks account for, so no read can run past it.
    pub(crate) fn from_parts(parts: Parts) -> io::Result<Self> {
        let n = parts.widths.len();
        if parts.counts.len() != n || parts.dict_lens.len() != n {
            return Err(invalid(format!(
                "ordinal blocks: {n} widths, {} counts and {} dictionary lengths; each block has \
                 one of each",
                parts.counts.len(),
                parts.dict_lens.len()
            )));
        }
        let mut blocks = Vec::with_capacity(n);
        let (mut slot, mut at) = (0u64, 0u64);
        for k in 0..n {
            let (count, dict_len) = (parts.counts[k], parts.dict_lens[k]);
            let width = Width::of_tag(parts.widths[k])
                .ok_or_else(|| invalid(format!("ordinal block {k}: unknown width tag")))?;
            let fits = count as usize <= BLOCK_LEN
                && match width {
                    Width::Empty => count == 0 && dict_len == 0,
                    Width::Const => count > 0 && dict_len == 1,
                    Width::U8 => count > 0 && (1..=256).contains(&dict_len) && dict_len <= count,
                    Width::U16 => count > 0 && dict_len >= 1 && dict_len <= count,
                    Width::Plain => count > 0 && dict_len == 0,
                };
            if !fits {
                return Err(invalid(format!(
                    "ordinal block {k}: a {width:?} block cannot hold {count} values with \
                     {dict_len} distinct"
                )));
            }
            blocks.push(Block {
                width,
                count,
                slot0: slot,
                dict_len,
                at,
            });
            slot += u64::from(count);
            at += block_bytes(width, count, dict_len, parts.code_bytes);
        }
        if at != 4 * parts.data.len() as u64 {
            return Err(invalid(format!(
                "ordinal blocks account for {at} bytes but the file holds {}",
                4 * parts.data.len()
            )));
        }
        Ok(BlockOrdinals {
            len: slot as usize,
            code_bytes: parts.code_bytes,
            blocks,
            widths: parts.widths,
            counts: parts.counts,
            dict_lens: parts.dict_lens,
            data: parts.data,
        })
    }

    /// The arrays this column is stored as, sharing its buffers.
    pub(crate) fn parts(&self) -> Parts {
        Parts {
            code_bytes: self.code_bytes,
            widths: self.widths.clone(),
            counts: self.counts.clone(),
            dict_lens: self.dict_lens.clone(),
            data: self.data.clone(),
        }
    }

    /// Values held, one per present entity.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// How wide the column's codes are.
    pub fn code_bytes(&self) -> CodeBytes {
        self.code_bytes
    }

    /// How many blocks are stored at each width, in the order `Empty`, `Const`, `U8`, `U16`,
    /// `Plain`, and the bytes the values take: what a measurement reports.
    pub fn census(&self) -> ([usize; 5], u64) {
        let mut widths = [0usize; 5];
        for b in &self.blocks {
            widths[usize::from(b.width.tag())] += 1;
        }
        (widths, (4 * self.data.len() + 9 * self.blocks.len()) as u64)
    }

    /// Refuse a column whose blocks are not the ones the column's addressing cuts it into: every
    /// entity's where `ranks` is `None`, otherwise those of the presence `ranks` were taken of.
    pub(crate) fn check_addressing(&self, ranks: Option<&BlockRanks>) -> io::Result<()> {
        let want = block_counts(self.len, ranks);
        let fits = want.len() == self.blocks.len()
            && want.iter().zip(&self.blocks).all(|(w, b)| *w == b.count);
        if !fits {
            return Err(invalid(format!(
                "ordinal blocks divide {} values into {} blocks other than the column's presence \
                 does; rewrite the column",
                self.len,
                self.blocks.len()
            )));
        }
        Ok(())
    }

    fn block_dict(&self, b: &Block) -> &[u32] {
        let at = (b.at / 4) as usize;
        &self.data[at..at + b.dict_len as usize]
    }

    /// Where block `b`'s values start in the data, in bytes.
    fn values_at(b: &Block) -> usize {
        (b.at + 4 * u64::from(b.dict_len)) as usize
    }

    fn values8(&self, b: &Block) -> &[u8] {
        let at = Self::values_at(b);
        &self.data.inner().as_slice()[at..at + b.count as usize]
    }

    fn values16(&self, b: &Block) -> &[u16] {
        let at = Self::values_at(b) / 2;
        &self.data.inner().typed_data::<u16>()[at..at + b.count as usize]
    }

    fn values32(&self, b: &Block) -> &[u32] {
        let at = Self::values_at(b) / 4;
        &self.data[at..at + b.count as usize]
    }

    /// `n` two-byte values from block `b`'s first, which may run on into the blocks after it.
    fn run16(&self, b: &Block, n: usize) -> &[u16] {
        let at = Self::values_at(b) / 2;
        &self.data.inner().typed_data::<u16>()[at..at + n]
    }

    /// `n` four-byte values from block `b`'s first, likewise.
    fn run32(&self, b: &Block, n: usize) -> &[u32] {
        let at = Self::values_at(b) / 4;
        &self.data[at..at + n]
    }

    /// Whether block `k` and the one after it are both `Plain`, with the second's values starting
    /// where the first's end, so the two read as one slice.
    fn plain_follows(&self, k: usize) -> bool {
        let (Some(a), Some(b)) = (self.blocks.get(k), self.blocks.get(k + 1)) else {
            return false;
        };
        a.width == Width::Plain
            && b.width == Width::Plain
            && b.at == a.at + self.code_bytes.bytes() * u64::from(a.count)
    }

    /// The block holding `slot`, which must be below [`Self::len`].
    fn block_of_slot(&self, slot: usize) -> usize {
        self.blocks
            .partition_point(|b| b.slot0 + u64::from(b.count) <= slot as u64)
    }

    /// The ordinal at `slot`.
    pub(crate) fn at(&self, slot: usize) -> u32 {
        let b = &self.blocks[self.block_of_slot(slot)];
        let mut found = u32::MAX;
        self.decode(b, slot - b.slot0 as usize, 1, |v| found = v);
        found
    }

    /// Block `b`'s values from its `i`-th, `len` of them. A position past the block's dictionary
    /// reads as `u32::MAX`, an ordinal no dictionary holds.
    fn decode(&self, b: &Block, i: usize, len: usize, mut f: impl FnMut(u32)) {
        let dict = self.block_dict(b);
        let at = |p: usize| dict.get(p).copied().unwrap_or(u32::MAX);
        match b.width {
            Width::Empty => {}
            Width::Const => (0..len).for_each(|_| f(at(0))),
            Width::U8 => self.values8(b)[i..i + len].iter().for_each(|&p| f(at(usize::from(p)))),
            Width::U16 => self.values16(b)[i..i + len].iter().for_each(|&p| f(at(usize::from(p)))),
            Width::Plain => match self.code_bytes {
                CodeBytes::Two => self.values16(b)[i..i + len]
                    .iter()
                    .for_each(|&v| f(u32::from(v))),
                CodeBytes::Four => self.values32(b)[i..i + len].iter().for_each(|&v| f(v)),
            },
        }
    }

    /// Visit the ordinals in slots `start..start + len`, in order.
    pub(crate) fn for_each_in(&self, start: usize, len: usize, mut f: impl FnMut(u32)) {
        if len == 0 {
            return;
        }
        let end = start + len;
        let mut k = self.block_of_slot(start);
        let mut slot = start;
        while slot < end {
            let b = &self.blocks[k];
            let block_end = (b.slot0 + u64::from(b.count)) as usize;
            let i = slot - b.slot0 as usize;
            self.decode(b, i, block_end.min(end) - slot, &mut f);
            slot = block_end;
            k += 1;
        }
    }
}

/// What an ordinal scan asks of each value.
pub(crate) enum OrdinalTest<'a> {
    Eq(u32),
    /// Sorted and distinct.
    In(&'a [u32]),
    /// Inclusive at both ends; `lo > hi` matches nothing and still scans.
    Range { lo: u32, hi: u32 },
    Set(&'a CodeSet),
}

/// An [`OrdinalTest`] restated over one block's dictionary positions.
enum LocalTest {
    Eq(u32),
    /// Half-open.
    Range(u32, u32),
    Set(CodeSet),
}

impl OrdinalTest<'_> {
    fn global(&self, v: u32) -> bool {
        match self {
            OrdinalTest::Eq(x) => v == *x,
            OrdinalTest::In(w) => w.binary_search(&v).is_ok(),
            OrdinalTest::Range { lo, hi } => *lo <= v && v <= *hi,
            OrdinalTest::Set(s) => s.contains(v),
        }
    }

    /// `dict` is a block's distinct ordinals, ascending and not empty.
    fn local(&self, dict: &[u32]) -> LocalTest {
        let top = dict.len() as u32 - 1;
        match self {
            OrdinalTest::Eq(x) => {
                LocalTest::Eq(dict.binary_search(x).map_or(u32::MAX, |i| i as u32))
            }
            OrdinalTest::Range { lo, hi } => LocalTest::Range(
                dict.partition_point(|v| v < lo) as u32,
                dict.partition_point(|v| v <= hi) as u32,
            ),
            OrdinalTest::In(w) => {
                let mut set = CodeSet::over_domain(top);
                for x in w.iter() {
                    if let Ok(i) = dict.binary_search(x) {
                        set.insert(i as u32);
                    }
                }
                LocalTest::Set(set)
            }
            OrdinalTest::Set(global) => {
                let mut set = CodeSet::over_domain(top);
                for (i, v) in dict.iter().enumerate() {
                    if global.contains(*v) {
                        set.insert(i as u32);
                    }
                }
                LocalTest::Set(set)
            }
        }
    }
}

/// The positions of a `Const` block's values: every one is the block dictionary's first entry.
static CONST_CODES: [u8; BLOCK_LEN] = [0; BLOCK_LEN];

impl ValueColumn {
    /// Entities of `candidate` whose ordinal passes `test`, block by block. Run on a rayon pool, a
    /// large candidate's blocks are cut into runs of consecutive blocks, a few per thread, and the
    /// runs scanned in parallel: each thread then reads its part of the file in order, which is
    /// what the kernel's read-ahead serves where the column is not resident.
    pub(super) fn scan_blocks(
        &self,
        ordinals: &BlockOrdinals,
        candidate: &Bitmap,
        test: &OrdinalTest<'_>,
    ) -> Bitmap {
        use rayon::prelude::*;
        let keys: Vec<u32> = Blocks::new(candidate)
            .take_while(|&k| (k as usize) < ordinals.blocks.len())
            .collect();
        if keys.is_empty() {
            return Bitmap::new();
        }
        let run = |keys: &[u32]| {
            counted(|| {
                let mut found = Vec::new();
                let mut packing = Packing::Undecided;
                let mut i = 0;
                while i < keys.len() {
                    let mut j = i;
                    while j + 1 < keys.len()
                        && keys[j + 1] == keys[j] + 1
                        && ordinals.plain_follows(keys[j] as usize)
                    {
                        j += 1;
                    }
                    found.push(self.scan_span(
                        ordinals,
                        (keys[i], keys[j]),
                        candidate,
                        test,
                        &mut packing,
                    ));
                    i = j + 1;
                }
                Bitmap::fast_or(&found.iter().collect::<Vec<_>>())
            })
        };
        let parallel = rayon::current_thread_index().is_some()
            && rayon::current_num_threads() > 1
            && candidate.cardinality() >= PARALLEL_MIN;
        let parts: Vec<(Bitmap, ScanWork)> = if parallel {
            let per = keys.len().div_ceil(rayon::current_num_threads() * 4);
            keys.par_chunks(per).map(run).collect()
        } else {
            vec![run(&keys)]
        };
        let mut found = Vec::with_capacity(parts.len());
        for (part, work) in &parts {
            add_scan_work(*work);
            found.push(part);
        }
        Bitmap::fast_or(&found)
    }

    /// Blocks `k0..=k1` of [`Self::scan_blocks`], one block or a run of `Plain` blocks whose
    /// values lie end to end: their values walked as one slice under a column addressing them
    /// alone, so packing, the per-entity walk and the work count are the ones every scan uses, and
    /// a run of blocks stored as plain codes costs one walk as a flat column's would. `packing`
    /// carries the decision whether to pack from the first block a scan packs to the rest, as
    /// one walk of a flat column does.
    fn scan_span(
        &self,
        ordinals: &BlockOrdinals,
        (k0, k1): (u32, u32),
        candidate: &Bitmap,
        test: &OrdinalTest<'_>,
        packing: &mut Packing,
    ) -> Bitmap {
        let b = &ordinals.blocks[k0 as usize];
        let n: usize = (k0..=k1)
            .map(|k| ordinals.blocks[k as usize].count as usize)
            .sum();
        if n == 0 {
            return Bitmap::new();
        }
        // The span is walked with its first entity at 0, so a column every entity carries a value
        // in keeps its positional walk, whole-block packing included.
        let within = Bitmap::from_range(*span(k0).start()..=*span(k1).end());
        let first = i64::from(k0 << BLOCK_BITS);
        let candidate = candidate.and(&within).add_offset(-first);
        let column = ValueColumn::addressing(
            self.presence
                .as_ref()
                .map(|presence| presence.and(&within).add_offset(-first)),
        );
        // Each arm hands the walk its own predicate, so each is compiled for its own test.
        macro_rules! walk {
            ($values:expr, $pred:expr) => {
                column.walk_one_packing(&candidate, $values, &$pred, packing)
            };
        }
        macro_rules! walk_positions {
            ($codes:expr) => {{
                let codes = $codes;
                match test.local(ordinals.block_dict(b)) {
                    LocalTest::Eq(c) => walk!(codes, |v| u32::from(*v) == c),
                    LocalTest::Range(lo, hi) => walk!(codes, |v| {
                        let v = u32::from(*v);
                        lo <= v && v < hi
                    }),
                    LocalTest::Set(set) => walk!(codes, |v| set.contains(u32::from(*v))),
                }
            }};
        }
        macro_rules! walk_codes {
            ($values:expr) => {{
                let values = $values;
                match test {
                    OrdinalTest::Eq(x) => walk!(values, |v| u32::from(*v) == *x),
                    OrdinalTest::In(w) => walk!(values, |v| w.binary_search(&u32::from(*v)).is_ok()),
                    OrdinalTest::Range { lo, hi } => walk!(values, |v| {
                        let v = u32::from(*v);
                        *lo <= v && v <= *hi
                    }),
                    OrdinalTest::Set(s) => walk!(values, |v| s.contains(u32::from(*v))),
                }
            }};
        }
        let found = match b.width {
            Width::Empty => Bitmap::new(),
            Width::Const => {
                let held = test.global(ordinals.block_dict(b)[0]);
                walk!(&CONST_CODES[..n], |_: &u8| held)
            }
            Width::U8 => walk_positions!(ordinals.values8(b)),
            Width::U16 => walk_positions!(ordinals.values16(b)),
            Width::Plain => match ordinals.code_bytes {
                CodeBytes::Two => walk_codes!(ordinals.run16(b, n)),
                CodeBytes::Four => walk_codes!(ordinals.run32(b, n)),
            },
        };
        found.add_offset(first)
    }
}

/// The schema metadata key the width of the column's codes is recorded under.
const CODE_BYTES_KEY: &str = "code_bytes";

/// The file's columns, in order: each is one row holding one of [`Parts`]' arrays.
const FIELDS: [&str; 4] = ["width", "count", "dict_len", "data"];

fn list_type(item: arrow::datatypes::DataType) -> arrow::datatypes::DataType {
    arrow::datatypes::DataType::LargeList(std::sync::Arc::new(arrow::datatypes::Field::new(
        "item", item, false,
    )))
}

/// One array as a one-row list column, borrowing its buffer.
fn list_of<T: arrow::datatypes::ArrowPrimitiveType>(
    values: ScalarBuffer<T::Native>,
) -> arrow::array::ArrayRef {
    use arrow::array::{LargeListArray, PrimitiveArray};
    use arrow::buffer::OffsetBuffer;
    let n = values.len() as i64;
    let item = std::sync::Arc::new(arrow::datatypes::Field::new("item", T::DATA_TYPE, false));
    std::sync::Arc::new(LargeListArray::new(
        item,
        OffsetBuffer::new(ScalarBuffer::from(vec![0i64, n])),
        std::sync::Arc::new(PrimitiveArray::<T>::new(values, None)),
        None,
    ))
}

/// The record batch a [`BlockOrdinals`] file holds: one row, a list column per array of `parts`.
pub(crate) fn batch_of_parts(parts: Parts) -> io::Result<arrow::record_batch::RecordBatch> {
    use arrow::datatypes::{DataType, Field, Schema, UInt32Type, UInt8Type};
    let types = [DataType::UInt8, DataType::UInt32, DataType::UInt32, DataType::UInt32];
    let fields: Vec<Field> = FIELDS
        .iter()
        .zip(types)
        .map(|(name, ty)| Field::new(*name, list_type(ty), false))
        .collect();
    let columns = vec![
        list_of::<UInt8Type>(parts.widths),
        list_of::<UInt32Type>(parts.counts),
        list_of::<UInt32Type>(parts.dict_lens),
        list_of::<UInt32Type>(parts.data),
    ];
    let schema = Schema::new(fields)
        .with_metadata([(CODE_BYTES_KEY, parts.code_bytes.bytes().to_string())]);
    arrow::record_batch::RecordBatch::try_new(std::sync::Arc::new(schema), columns)
        .map_err(|e| invalid(e.to_string()))
}

/// The arrays of a [`BlockOrdinals`] file, borrowed from the batch, or `None` where the batch is
/// some other value column's.
pub(crate) fn parts_of_batch(
    batch: &arrow::record_batch::RecordBatch,
    path: &std::path::Path,
) -> io::Result<Option<Parts>> {
    use arrow::array::{Array, LargeListArray, PrimitiveArray};
    use arrow::datatypes::{UInt32Type, UInt8Type};
    let schema = batch.schema_ref();
    let named: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();
    if named.first() != Some(&FIELDS[0]) {
        return Ok(None);
    }
    if named != FIELDS || batch.num_rows() != 1 {
        return Err(invalid(format!(
            "ordinal blocks at {}: expected one row of the columns {FIELDS:?}, found {} rows of \
             {named:?}",
            path.display(),
            batch.num_rows()
        )));
    }
    fn array<T: arrow::datatypes::ArrowPrimitiveType>(
        batch: &arrow::record_batch::RecordBatch,
        i: usize,
        path: &std::path::Path,
    ) -> io::Result<ScalarBuffer<T::Native>> {
        let wrong = || {
            invalid(format!(
                "ordinal blocks at {}: column '{}' is not a list of {}",
                path.display(),
                FIELDS[i],
                T::DATA_TYPE
            ))
        };
        let list = batch
            .column(i)
            .as_any()
            .downcast_ref::<LargeListArray>()
            .ok_or_else(wrong)?;
        let values = list
            .values()
            .as_any()
            .downcast_ref::<PrimitiveArray<T>>()
            .ok_or_else(wrong)?;
        let offsets = list.value_offsets();
        if values.null_count() != 0 || offsets != [0, values.len() as i64] {
            return Err(wrong());
        }
        Ok(values.values().clone())
    }
    let code_bytes = schema
        .metadata()
        .get(CODE_BYTES_KEY)
        .and_then(|b| CodeBytes::of(b))
        .ok_or_else(|| {
            invalid(format!(
                "ordinal blocks at {}: the schema records no code width of 2 or 4 bytes",
                path.display()
            ))
        })?;
    Ok(Some(Parts {
        code_bytes,
        widths: array::<UInt8Type>(batch, 0, path)?,
        counts: array::<UInt32Type>(batch, 1, path)?,
        dict_lens: array::<UInt32Type>(batch, 2, path)?,
        data: array::<UInt32Type>(batch, 3, path)?,
    }))
}

/// One block's values restated as its encoding: its distinct ordinals and each value's position
/// among them. Reused across blocks so a column is encoded with one allocation of each.
#[derive(Default)]
pub(crate) struct BlockEncoder {
    keyed: Vec<u64>,
    /// The block's distinct ordinals, ascending.
    pub dict: Vec<u32>,
    /// Each value's position in [`Self::dict`].
    pub positions: Vec<u16>,
}

impl BlockEncoder {
    /// Encode `values`, at most a block's worth of a column of `code_bytes` codes, and return the
    /// width to store them at. The dictionary and positions are left in `self` for every width but
    /// `Empty` and `Plain`.
    pub(crate) fn encode(&mut self, values: &[u32], code_bytes: CodeBytes) -> Width {
        debug_assert!(values.len() <= BLOCK_LEN, "one block's values");
        self.dict.clear();
        self.positions.clear();
        if values.is_empty() {
            return Width::Empty;
        }
        // Each value keyed by its ordinal above its position, so one sort yields the dictionary
        // and every position at once.
        self.keyed.clear();
        self.keyed.extend(
            values
                .iter()
                .enumerate()
                .map(|(i, &v)| (u64::from(v) << BLOCK_BITS) | i as u64),
        );
        self.keyed.sort_unstable();
        self.positions.resize(values.len(), 0);
        for &key in &self.keyed {
            let v = (key >> BLOCK_BITS) as u32;
            if self.dict.last() != Some(&v) {
                self.dict.push(v);
            }
            self.positions[(key & (BLOCK_LEN as u64 - 1)) as usize] = (self.dict.len() - 1) as u16;
        }
        Width::smallest(values.len(), self.dict.len(), code_bytes)
    }
}
