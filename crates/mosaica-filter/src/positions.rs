//! A text layer's word positions: for each term, where in each item's token sequence it occurs.
//!
//! `positions.arrow` is one Arrow IPC file with one non-null `LargeBinary` column, record *i* term
//! ordinal *i*'s, beside the postings it is read with. A term's record holds one entry per item of
//! its posting, in the posting's ascending item order, so the item at rank *k* of the posting has
//! entry *k*. An entry is the number of times the term occurs in the item and then each position:
//! the token's index in the sequence `Analyser::for_each_token` yields, the first absolute and each
//! later one less the one before.
//!
//! The entries are grouped in blocks of [`POSITIONS_BLOCK`] items. With `n` items in the posting
//! there are `B = ⌈n / 128⌉` blocks, and the record is:
//!
//! - `B − 1` little-endian `u64`s, the byte offset from the record's start of blocks 1 to `B − 1`.
//!   Block 0 begins after them.
//! - Each block: its items' counts less one, as one run, then the position gaps of all its items in
//!   item order, as runs of 128 with a shorter one last.
//!
//! A run of 128 values is one byte holding the bit width and then `BitPacker4x`'s 16 bytes per bit.
//! A run of fewer values, which is the last block's counts and the last run of each block's gaps,
//! is each value as a LEB128 varint.

use std::io;
use std::path::Path;

use arrow::array::{Array, LargeBinaryArray};
use bitpacking::{BitPacker, BitPacker4x};
use mosaica_types::AttrLocalId;

use crate::varint::take_varint;

/// A text column's positions file within its directory.
pub const POSITIONS_FILE: &str = "positions.arrow";

/// The name of the one column a positions file holds.
pub const POSITIONS_COLUMN: &str = "positions";

/// Items per block, and values per bit-packed run.
pub const POSITIONS_BLOCK: usize = BitPacker4x::BLOCK_LEN;

/// A text layer's three record counts. Record *i* of its dictionary, postings and positions is
/// term *i*'s in each, so a layer whose counts differ would answer a term from another's records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextLayerCounts {
    pub terms: u32,
    pub postings: u32,
    pub positions: u32,
}

impl TextLayerCounts {
    pub fn of(
        dict: &crate::SortedDict,
        postings: &crate::ColumnPostings,
        positions: &ColumnPositions,
    ) -> TextLayerCounts {
        TextLayerCounts {
            terms: dict.len(),
            postings: postings.record_count(),
            positions: positions.record_count(),
        }
    }

    pub fn agree(&self) -> bool {
        self.terms == self.postings && self.terms == self.positions
    }
}

impl std::fmt::Display for TextLayerCounts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} terms, {} postings records and {} positions records",
            self.terms, self.postings, self.positions
        )
    }
}

/// One text layer's positions file.
#[derive(Debug)]
pub struct ColumnPositions {
    array: LargeBinaryArray,
}

impl ColumnPositions {
    /// Open `path`, mapped when `mmap` is `true` and read into memory otherwise. A record's bytes
    /// are checked as they are decoded, not here.
    pub fn open(path: &Path, mmap: bool) -> io::Result<Self> {
        Ok(ColumnPositions {
            array: mosaica_authz::open_record_column(path, mmap, POSITIONS_COLUMN)?,
        })
    }

    /// The number of records, one per term of the layer's dictionary.
    pub fn record_count(&self) -> u32 {
        self.array.len() as u32
    }

    /// A cursor over `term`'s record, whose posting holds `items` items.
    pub fn cursor(&self, term: AttrLocalId, items: u64) -> io::Result<PositionsCursor<'_>> {
        let ordinal = term.raw() as usize;
        if ordinal >= self.array.len() {
            return Err(malformed(format!(
                "term {ordinal} has no positions record; the file holds {}",
                self.array.len()
            )));
        }
        let record = self.array.value(ordinal);
        let blocks = items.div_ceil(POSITIONS_BLOCK as u64);
        let table = blocks
            .saturating_sub(1)
            .checked_mul(8)
            .and_then(|t| usize::try_from(t).ok())
            .filter(|t| *t <= record.len())
            .ok_or_else(|| {
                malformed(format!(
                    "term {ordinal}'s record of {} bytes cannot hold the block table for {items} \
                     items",
                    record.len()
                ))
            })?;
        Ok(PositionsCursor {
            record,
            ordinal,
            items,
            table,
            block: None,
            counts: Vec::with_capacity(POSITIONS_BLOCK),
            starts: Vec::with_capacity(POSITIONS_BLOCK + 1),
            positions: Vec::new(),
        })
    }
}

/// One term's positions, read item by item by the item's rank in the term's posting. Moving to an
/// item decodes only the block that holds it, and reading the same item again decodes nothing.
pub struct PositionsCursor<'a> {
    record: &'a [u8],
    ordinal: usize,
    items: u64,
    /// Bytes of the block table at the record's head.
    table: usize,
    /// The block decoded into the three buffers below.
    block: Option<u64>,
    counts: Vec<u32>,
    /// Where each item's positions begin in `positions`, with one past the last.
    starts: Vec<usize>,
    positions: Vec<u32>,
}

impl PositionsCursor<'_> {
    /// The positions of the item at `rank` in the posting, ascending.
    pub fn positions(&mut self, rank: u64) -> io::Result<&[u32]> {
        if rank >= self.items {
            return Err(malformed(format!(
                "rank {rank} is past the {} items of term {}'s posting",
                self.items, self.ordinal
            )));
        }
        let block = rank / POSITIONS_BLOCK as u64;
        if self.block != Some(block) {
            self.decode(block)?;
        }
        let at = (rank % POSITIONS_BLOCK as u64) as usize;
        Ok(&self.positions[self.starts[at]..self.starts[at + 1]])
    }

    fn block_start(&self, block: u64) -> io::Result<usize> {
        if block == 0 {
            return Ok(self.table);
        }
        let at = (block as usize - 1) * 8;
        let offset = u64::from_le_bytes(
            self.record[at..at + 8]
                .try_into()
                .expect("an eight-byte slice"),
        );
        usize::try_from(offset)
            .ok()
            .filter(|o| *o >= self.table && *o <= self.record.len())
            .ok_or_else(|| self.refuse(format!("block {block} starts at {offset}, outside it")))
    }

    fn decode(&mut self, block: u64) -> io::Result<()> {
        self.block = None;
        let start = self.block_start(block)?;
        let last = block + 1 == self.items.div_ceil(POSITIONS_BLOCK as u64);
        let end = match last {
            true => self.record.len(),
            false => self.block_start(block + 1)?,
        };
        if end < start {
            return Err(self.refuse(format!("block {block} ends before it starts")));
        }
        let record = self.record;
        let bytes = &record[start..end];
        let items = (self.items - block * POSITIONS_BLOCK as u64).min(POSITIONS_BLOCK as u64);
        let mut cursor = 0usize;
        self.counts.clear();
        take_run(bytes, &mut cursor, items as usize, &mut self.counts)
            .map_err(|why| self.refuse(format!("block {block}'s counts {why}")))?;
        let mut total = 0usize;
        self.starts.clear();
        for count in &mut self.counts {
            self.starts.push(total);
            *count = count
                .checked_add(1)
                .ok_or_else(|| malformed("a count overflows u32".to_string()))?;
            total += *count as usize;
        }
        self.starts.push(total);
        // A packed run takes at least its width byte and a varint at least a byte, so no honest
        // block holds more values than this, and a count claiming more is refused before it sizes
        // anything.
        if total > bytes.len().saturating_mul(POSITIONS_BLOCK) {
            return Err(self.refuse(format!(
                "block {block} claims {total} positions in {} bytes",
                bytes.len()
            )));
        }
        self.positions.clear();
        while self.positions.len() < total {
            let run = (total - self.positions.len()).min(POSITIONS_BLOCK);
            take_run(bytes, &mut cursor, run, &mut self.positions)
                .map_err(|why| self.refuse(format!("block {block}'s positions {why}")))?;
        }
        if cursor != bytes.len() {
            return Err(self.refuse(format!(
                "block {block} holds {} bytes past its positions",
                bytes.len() - cursor
            )));
        }
        for item in 0..self.counts.len() {
            let mut previous: Option<u32> = None;
            for slot in self.starts[item]..self.starts[item + 1] {
                let gap = self.positions[slot];
                let position = match previous {
                    None => Some(gap),
                    Some(p) => p.checked_add(gap).filter(|_| gap > 0),
                };
                let Some(position) = position else {
                    return Err(self.refuse(format!(
                        "block {block} item {item}'s positions do not ascend"
                    )));
                };
                self.positions[slot] = position;
                previous = Some(position);
            }
        }
        self.block = Some(block);
        Ok(())
    }

    fn refuse(&self, detail: String) -> io::Error {
        malformed(format!(
            "term {}'s positions record: {detail}",
            self.ordinal
        ))
    }
}

/// Decode a run of `len` values from `bytes` at `cursor` onto `out`: bit-packed when `len` is a
/// whole run, varints otherwise.
fn take_run(
    bytes: &[u8],
    cursor: &mut usize,
    len: usize,
    out: &mut Vec<u32>,
) -> Result<(), String> {
    if len < POSITIONS_BLOCK {
        for _ in 0..len {
            out.push(take_varint(bytes, cursor).map_err(|e| e.to_string())?);
        }
        return Ok(());
    }
    let width = *bytes.get(*cursor).ok_or("ends before a run's bit width")?;
    if width > 32 {
        return Err(format!("holds a run {width} bits wide"));
    }
    let packed = BitPacker4x::compressed_block_size(width);
    let from = *cursor + 1;
    let run = bytes
        .get(from..from + packed)
        .ok_or("ends inside a bit-packed run")?;
    let at = out.len();
    out.resize(at + POSITIONS_BLOCK, 0);
    BitPacker4x::new().decompress(run, &mut out[at..], width);
    *cursor = from + packed;
    Ok(())
}

fn malformed(detail: String) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{POSITIONS_FILE}: {detail}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::put_varint;

    fn file_of(records: &[Vec<u8>]) -> (tempfile::TempDir, ColumnPositions) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(POSITIONS_FILE);
        let mut spool = mosaica_authz::PostingsSpool::for_column(
            &path.with_extension("spool"),
            POSITIONS_COLUMN,
        )
        .unwrap();
        for record in records {
            spool.append(record).unwrap();
        }
        spool.finish(&path).unwrap();
        let file = ColumnPositions::open(&path, false).unwrap();
        (dir, file)
    }

    fn varints(values: &[u32]) -> Vec<u8> {
        let mut out = Vec::new();
        for &v in values {
            put_varint(v, &mut out);
        }
        out
    }

    /// Two items in one short block: counts less one, then gaps, all varints.
    #[test]
    fn a_short_record_reads_as_its_varints_say() {
        let (_dir, file) = file_of(&[varints(&[1, 0, 4, 3, 9])]);
        let mut cursor = file.cursor(AttrLocalId::new(0), 2).unwrap();
        assert_eq!(cursor.positions(0).unwrap(), &[4, 7]);
        assert_eq!(cursor.positions(1).unwrap(), &[9]);
    }

    /// **A record that does not say what its posting needs is refused**, never read past: too
    /// short, bytes left over, positions that repeat, a block table outside the record, and a
    /// count claiming more positions than the bytes could hold.
    #[test]
    fn a_malformed_record_is_refused() {
        let mut huge_count = varints(&[u32::MAX - 1]);
        huge_count.push(0);
        let (_dir, file) = file_of(&[
            varints(&[0]),
            varints(&[0, 4, 5]),
            varints(&[1, 4, 0]),
            vec![0xff; 8],
            huge_count,
        ]);
        let read = |term: u32, items: u64, rank: u64| {
            file.cursor(AttrLocalId::new(term), items)
                .and_then(|mut c| c.positions(rank).map(<[u32]>::to_vec))
        };
        assert!(read(0, 1, 0).is_err(), "truncated");
        assert!(read(1, 1, 0).is_err(), "trailing bytes");
        assert!(read(2, 1, 0).is_err(), "a repeated position");
        assert!(
            read(3, 200, 150).is_err(),
            "a block offset outside the record"
        );
        assert!(
            read(3, 1_000_000, 0).is_err(),
            "a table longer than the record"
        );
        assert!(read(4, 1, 0).is_err(), "more positions than bytes");
        assert!(read(9, 1, 0).is_err(), "a term past the file");
        assert_eq!(
            read(1, 1, 0).err().unwrap().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
