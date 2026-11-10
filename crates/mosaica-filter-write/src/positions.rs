//! The writer of a text layer's positions file. `mosaica_filter::positions` describes the format
//! and reads it.

use std::io;
use std::path::Path;

use bitpacking::{BitPacker, BitPacker4x};
use mosaica_authz::RecordWriter;
use mosaica_filter::{put_varint, POSITIONS_BLOCK, POSITIONS_COLUMN};

/// Where a record column's records go: its file, or memory that holds them until they can be
/// written to it.
pub(crate) trait RecordSink {
    /// A whole record.
    fn append(&mut self, record: &[u8]) -> io::Result<()>;
    /// Begin a record written in pieces, whose first `head` bytes its end writes back.
    fn begin_record(&mut self, head: usize) -> io::Result<()>;
    fn extend_record(&mut self, bytes: &[u8]) -> io::Result<()>;
    fn end_record(&mut self, head: &[u8]) -> io::Result<()>;
}

impl RecordSink for RecordWriter {
    fn append(&mut self, record: &[u8]) -> io::Result<()> {
        RecordWriter::append(self, record)
    }
    fn begin_record(&mut self, head: usize) -> io::Result<()> {
        RecordWriter::begin_record(self, head)
    }
    fn extend_record(&mut self, bytes: &[u8]) -> io::Result<()> {
        RecordWriter::extend_record(self, bytes)
    }
    fn end_record(&mut self, head: &[u8]) -> io::Result<()> {
        RecordWriter::end_record(self, head)
    }
}

/// Records held in memory end to end, and one being written in pieces after them: what a range of
/// a parallel merge holds until its turn, and a batch of keys on its way to a writer.
#[derive(Default)]
pub struct HeldRecords {
    bytes: Vec<u8>,
    /// Where each whole record ends.
    ends: Vec<usize>,
    /// The record being written: where it starts, and the length of its head.
    open: Option<(usize, usize)>,
}

impl HeldRecords {
    /// Hold `record`, after the others.
    pub fn push(&mut self, record: &[u8]) {
        self.bytes.extend_from_slice(record);
        self.ends.push(self.bytes.len());
    }

    /// The whole records held.
    pub fn len(&self) -> usize {
        self.ends.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ends.is_empty()
    }

    /// Whole record `i`.
    pub fn record(&self, i: usize) -> &[u8] {
        let start = i.checked_sub(1).map_or(0, |before| self.ends[before]);
        &self.bytes[start..self.ends[i]]
    }

    /// Each whole record, in order.
    pub fn records(&self) -> impl Iterator<Item = &[u8]> {
        (0..self.len()).map(|i| self.record(i))
    }

    /// The bytes held.
    pub fn bytes(&self) -> usize {
        self.bytes.len()
    }

    pub fn clear(&mut self) {
        self.bytes.clear();
        self.ends.clear();
        self.open = None;
    }
}

impl RecordSink for HeldRecords {
    fn append(&mut self, record: &[u8]) -> io::Result<()> {
        self.push(record);
        Ok(())
    }
    fn begin_record(&mut self, head: usize) -> io::Result<()> {
        self.open = Some((self.bytes.len(), head));
        self.bytes.resize(self.bytes.len() + head, 0);
        Ok(())
    }
    fn extend_record(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn end_record(&mut self, head: &[u8]) -> io::Result<()> {
        let (start, len) = self
            .open
            .take()
            .ok_or_else(|| invalid("a held record ended that never began"))?;
        if head.len() != len {
            return Err(invalid(format!(
                "a held record began with a {len}-byte head and ended with {} bytes of it",
                head.len()
            )));
        }
        self.bytes[start..start + len].copy_from_slice(head);
        self.ends.push(self.bytes.len());
        Ok(())
    }
}

/// Writes a positions file record by record, each record streamed into its sink block by block
/// and its block table written back over its head when it ends.
pub(crate) struct PositionsWriter<R = RecordWriter> {
    file: R,
    state: Encoder,
}

/// The open record of a [`PositionsWriter`], which moves with the record from one sink to another.
#[derive(Default)]
struct Encoder {
    /// Items the open term's posting holds, and how many of them have been pushed.
    items: u64,
    pushed: u64,
    open: bool,
    /// The length of the open record's head, which its block table is written back over.
    head: usize,
    /// The open record's block table, and how many bytes of it are written, the table included.
    table: Vec<u8>,
    written: u64,
    blocks: u64,
    /// The open block.
    block: Raw,
    bytes: Vec<u8>,
}

impl PositionsWriter {
    pub(crate) fn create(path: &Path) -> io::Result<PositionsWriter> {
        Ok(PositionsWriter::over(RecordWriter::create(
            path,
            POSITIONS_COLUMN,
        )?))
    }

    pub(crate) fn finish(self) -> io::Result<()> {
        if self.state.open {
            return Err(invalid(
                "the positions file finished inside a term's record",
            ));
        }
        self.file.finish()
    }

    /// Go on with the record `held` left open, writing what it holds of it into this writer's
    /// file. `held` keeps its whole records.
    pub(crate) fn adopt(&mut self, held: &mut PositionsWriter<HeldRecords>) -> io::Result<()> {
        if self.state.open {
            return Err(invalid(
                "a term's positions were adopted inside another term's record",
            ));
        }
        let Some((start, head)) = held.file.open.take() else {
            return Ok(());
        };
        self.file.begin_record(head)?;
        self.file.extend_record(&held.file.bytes[start + head..])?;
        held.file.bytes.truncate(start);
        std::mem::swap(&mut self.state, &mut held.state);
        held.state.open = false;
        Ok(())
    }
}

impl<R: RecordSink> PositionsWriter<R> {
    /// Go on with the open record from what `part` holds of its items, which must be the next
    /// ones: its leading items complete the open block, its whole blocks are written as they
    /// stand with the block table moved by what the record holds before them, and its trailing
    /// items open the next block.
    pub(crate) fn adopt_part(&mut self, part: &PartPositions) -> io::Result<()> {
        self.go_on(part.items, part.from)?;
        self.take_raw(&part.lead)?;
        if !self.state.block.counts.is_empty() && !part.ends.is_empty() {
            return Err(invalid("a part's whole blocks began inside the open block"));
        }
        let mut start = 0;
        for &end in &part.ends {
            self.append_block(&part.blocks[start..end])?;
            self.state.pushed += POSITIONS_BLOCK as u64;
            start = end;
        }
        self.take_raw(&part.tail)
    }

    /// Go on with the open record of `items` items at item `from`, where it must be.
    pub(crate) fn go_on(&self, items: u64, from: u64) -> io::Result<()> {
        let state = &self.state;
        match state.open && state.items == items && state.pushed == from {
            true => Ok(()),
            false => Err(invalid(format!(
                "a part of a term's positions from item {from} of {items} arrived where the record \
                 is at item {} of {}",
                state.pushed, state.items
            ))),
        }
    }

    /// Add `items` to the open block, writing it once it is whole.
    fn take_raw(&mut self, items: &Raw) -> io::Result<()> {
        let state = &mut self.state;
        if state.block.counts.len() + items.counts.len() > POSITIONS_BLOCK {
            return Err(invalid("a part's items pass the open block's edge"));
        }
        state.block.counts.extend_from_slice(&items.counts);
        state.block.gaps.extend_from_slice(&items.gaps);
        state.pushed += items.counts.len() as u64;
        if state.block.counts.len() == POSITIONS_BLOCK {
            self.write_block()?;
        }
        Ok(())
    }
}

/// Items' positions as a block holds them before it is encoded: each item's count less one, and
/// every item's position gaps end to end.
#[derive(Default)]
struct Raw {
    counts: Vec<u32>,
    gaps: Vec<u32>,
}

impl Raw {
    /// Add an item's positions, checked to be what the format holds.
    fn push(&mut self, positions: &[u32]) -> io::Result<()> {
        let Some((&first, _)) = positions.split_first() else {
            return Err(invalid("an item holding a term holds it at a position"));
        };
        if !positions.is_sorted_by(|a, b| a < b) {
            return Err(invalid("an item's positions must ascend strictly"));
        }
        self.counts.push(positions.len() as u32 - 1);
        self.gaps.push(first);
        self.gaps.extend(positions.windows(2).map(|w| w[1] - w[0]));
        Ok(())
    }

    /// Encode the block it holds into `out`.
    fn encode(&self, out: &mut Vec<u8>) {
        put_run(&self.counts, out);
        for run in self.gaps.chunks(POSITIONS_BLOCK) {
            put_run(run, out);
        }
    }

    fn clear(&mut self) {
        self.counts.clear();
        self.gaps.clear();
    }

    fn bytes(&self) -> usize {
        4 * (self.counts.len() + self.gaps.len())
    }
}

/// The items of a term's positions record from item `from` on, of a part of the term that a range
/// merges while the record is open elsewhere: the items before the first block edge it reaches,
/// held raw for the record's open block, then the whole blocks it encodes, then the items past the
/// last of them, held raw to open the next. [`PositionsWriter::adopt_part`] takes them.
pub(crate) struct PartPositions {
    /// The term's items, the item the part begins at, and the next item.
    items: u64,
    from: u64,
    at: u64,
    lead: Raw,
    /// The whole blocks, end to end, and where each ends.
    blocks: Vec<u8>,
    ends: Vec<usize>,
    tail: Raw,
}

impl PartPositions {
    /// The part of a record of `items` items from item `from`.
    pub(crate) fn new(items: u64, from: u64) -> PartPositions {
        PartPositions {
            items,
            from,
            at: from,
            lead: Raw::default(),
            blocks: Vec::new(),
            ends: Vec::new(),
            tail: Raw::default(),
        }
    }

    /// The next item's positions, which must ascend strictly.
    pub(crate) fn push(&mut self, positions: &[u32]) -> io::Result<()> {
        if self.at == self.items {
            return Err(invalid(format!(
                "an item's positions arrived past the {} items the term's posting holds",
                self.items
            )));
        }
        let edge = self.from.next_multiple_of(POSITIONS_BLOCK as u64);
        match self.at < edge {
            true => self.lead.push(positions)?,
            false => self.tail.push(positions)?,
        }
        self.at += 1;
        if self.tail.counts.len() == POSITIONS_BLOCK {
            self.tail.encode(&mut self.blocks);
            self.ends.push(self.blocks.len());
            self.tail.clear();
        }
        Ok(())
    }

    /// The bytes it holds.
    pub(crate) fn bytes(&self) -> usize {
        self.lead.bytes() + self.blocks.len() + self.tail.bytes()
    }
}

impl PositionsWriter<HeldRecords> {
    pub(crate) fn held() -> PositionsWriter<HeldRecords> {
        PositionsWriter::over(HeldRecords::default())
    }

    pub(crate) fn records(&self) -> &HeldRecords {
        &self.file
    }

    pub(crate) fn clear(&mut self) {
        self.file.clear();
        self.state.open = false;
    }
}

impl<R: RecordSink> PositionsWriter<R> {
    fn over(file: R) -> PositionsWriter<R> {
        PositionsWriter {
            file,
            state: Encoder {
                block: Raw {
                    counts: Vec::with_capacity(POSITIONS_BLOCK),
                    gaps: Vec::new(),
                },
                ..Encoder::default()
            },
        }
    }

    /// Begin the next term's record, for a posting of `items` items.
    pub(crate) fn begin(&mut self, items: u64) -> io::Result<()> {
        if self.state.open {
            return Err(invalid(
                "a term's positions began before the last one ended",
            ));
        }
        if items == 0 {
            return Err(invalid(
                "a term with no items has no positions record to write",
            ));
        }
        let head = (items.div_ceil(POSITIONS_BLOCK as u64) - 1)
            .checked_mul(8)
            .and_then(|h| usize::try_from(h).ok())
            .ok_or_else(|| invalid("a term's block table does not fit in memory"))?;
        self.file.begin_record(head)?;
        let state = &mut self.state;
        state.items = items;
        state.pushed = 0;
        state.open = true;
        state.head = head;
        state.table.clear();
        state.written = head as u64;
        state.blocks = 0;
        state.block.clear();
        Ok(())
    }

    /// The next item's positions, which must ascend strictly.
    pub(crate) fn push(&mut self, positions: &[u32]) -> io::Result<()> {
        let state = &mut self.state;
        if !state.open || state.pushed == state.items {
            return Err(invalid(format!(
                "an item's positions arrived past the {} items the term's posting holds",
                state.items
            )));
        }
        state.block.push(positions)?;
        state.pushed += 1;
        if state.block.counts.len() == POSITIONS_BLOCK {
            self.write_block()?;
        }
        Ok(())
    }

    /// End the term's record, every item pushed.
    pub(crate) fn end(&mut self) -> io::Result<()> {
        if !self.state.open || self.state.pushed != self.state.items {
            return Err(invalid(format!(
                "a term's positions ended after {} of the {} items its posting holds",
                self.state.pushed, self.state.items
            )));
        }
        if !self.state.block.counts.is_empty() {
            self.write_block()?;
        }
        self.file.end_record(&self.state.table)?;
        self.state.open = false;
        Ok(())
    }

    /// Write a whole record as another positions file holds it, between terms.
    pub(crate) fn copy(&mut self, record: &[u8]) -> io::Result<()> {
        if self.state.open {
            return Err(invalid(
                "a term's positions were copied inside another term's record",
            ));
        }
        self.file.append(record)
    }

    fn write_block(&mut self) -> io::Result<()> {
        let mut bytes = std::mem::take(&mut self.state.bytes);
        bytes.clear();
        self.state.block.encode(&mut bytes);
        let appended = self.append_block(&bytes);
        self.state.bytes = bytes;
        self.state.block.clear();
        appended
    }

    /// Append an encoded block to the open record, naming it in the record's block table.
    fn append_block(&mut self, block: &[u8]) -> io::Result<()> {
        let state = &mut self.state;
        if state.blocks > 0 {
            state.table.extend_from_slice(&state.written.to_le_bytes());
        }
        self.file.extend_record(block)?;
        state.written += block.len() as u64;
        state.blocks += 1;
        Ok(())
    }
}

/// One run: bit-packed when it holds a whole run's values, as varints when it holds fewer.
fn put_run(values: &[u32], out: &mut Vec<u8>) {
    if values.len() < POSITIONS_BLOCK {
        for &value in values {
            put_varint(value, out);
        }
        return;
    }
    let packer = BitPacker4x::new();
    let width = packer.num_bits(values);
    out.push(width);
    let at = out.len();
    out.resize(at + BitPacker4x::compressed_block_size(width), 0);
    packer.compress(values, &mut out[at..], width);
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mosaica_filter::{ColumnPositions, POSITIONS_FILE};
    use mosaica_types::AttrLocalId;

    /// Item `i` of term `t`'s positions: a count from 1 to 300, so some items hold more than a run,
    /// with gaps from 1 to well past a byte.
    fn item(t: usize, i: usize) -> Vec<u32> {
        let count = 1 + (t * 31 + i * 7) % 300;
        let mut at = ((t + i) % 5) as u32;
        (0..count)
            .map(|k| {
                let here = at;
                at += 1 + ((k * 13 + i) % 700) as u32;
                here
            })
            .collect()
    }

    fn write(dir: &Path, terms: &[Vec<Vec<u32>>]) -> ColumnPositions {
        let path = dir.join(POSITIONS_FILE);
        let mut writer = PositionsWriter::create(&path).unwrap();
        for items in terms {
            writer.begin(items.len() as u64).unwrap();
            for positions in items {
                writer.push(positions).unwrap();
            }
            writer.end().unwrap();
        }
        writer.finish().unwrap();
        ColumnPositions::open(&path, false).unwrap()
    }

    /// **Every item's positions read back as written, in order and by seeking**, for postings
    /// either side of a block and of several blocks with a partial one last.
    #[test]
    fn positions_round_trip_and_seek_across_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let sizes = [1usize, 2, 127, 128, 129, 256, 300, 1000];
        let terms: Vec<Vec<Vec<u32>>> = sizes
            .iter()
            .enumerate()
            .map(|(t, &n)| (0..n).map(|i| item(t, i)).collect())
            .collect();
        let file = write(dir.path(), &terms);
        assert_eq!(file.record_count(), terms.len() as u32);
        for (t, items) in terms.iter().enumerate() {
            let term = AttrLocalId::new(t as u32);
            let mut cursor = file.cursor(term, items.len() as u64).unwrap();
            for (rank, want) in items.iter().enumerate() {
                assert_eq!(cursor.positions(rank as u64).unwrap(), want.as_slice());
            }
            // Backwards, by strides that cross blocks, and the same item twice.
            let mut cursor = file.cursor(term, items.len() as u64).unwrap();
            for rank in (0..items.len()).rev().step_by(97).chain([0, 0]) {
                assert_eq!(
                    cursor.positions(rank as u64).unwrap(),
                    items[rank].as_slice()
                );
            }
            assert!(cursor.positions(items.len() as u64).is_err());
        }
    }

    /// **Gaps at every width read back**: a packed run 32 bits wide, whose last position is
    /// `u32::MAX`, and gaps past 2¹⁶ in a packed run and in a varint tail.
    #[test]
    fn wide_gaps_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let mut widest: Vec<u32> = (0..127).collect();
        widest.push(u32::MAX);
        let sparse: Vec<u32> = (0..200).map(|k| k * 70_000).collect();
        let terms = vec![vec![widest.clone(), sparse.clone(), vec![1 << 20]]];
        let file = write(dir.path(), &terms);
        let mut cursor = file.cursor(AttrLocalId::new(0), 3).unwrap();
        assert_eq!(cursor.positions(0).unwrap(), widest.as_slice());
        assert_eq!(cursor.positions(1).unwrap(), sparse.as_slice());
        assert_eq!(cursor.positions(2).unwrap(), &[1 << 20]);
    }

    /// **A run shorter than 128 values is varints**: one item holding a term once is two bytes,
    /// its count less one and its position, with no block table and no padding.
    #[test]
    fn a_short_run_is_written_as_varints() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(POSITIONS_FILE);
        let mut writer = PositionsWriter::create(&path).unwrap();
        writer.begin(1).unwrap();
        writer.push(&[300]).unwrap();
        writer.end().unwrap();
        writer.finish().unwrap();
        let array = mosaica_authz::open_record_column(&path, false, POSITIONS_COLUMN).unwrap();
        let mut want = vec![0u8];
        put_varint(300, &mut want);
        assert_eq!(arrow::array::Array::len(&array), 1);
        assert_eq!(array.value(0), want.as_slice());
    }

    /// The writer refuses what the reader could not make sense of: items past the posting, too
    /// few, none, and positions that do not ascend.
    #[test]
    fn the_writer_refuses_what_the_format_cannot_hold() {
        let dir = tempfile::tempdir().unwrap();
        let mut writer = PositionsWriter::create(&dir.path().join("p")).unwrap();
        assert!(writer.begin(0).is_err());
        writer.begin(1).unwrap();
        assert!(writer.push(&[]).is_err());
        assert!(writer.push(&[3, 3]).is_err());
        assert!(writer.push(&[4, 2]).is_err());
        assert!(writer.end().is_err(), "no item was pushed");
        writer.push(&[1, 2]).unwrap();
        assert!(writer.push(&[1]).is_err(), "one item past the posting");
        writer.end().unwrap();
    }
}
