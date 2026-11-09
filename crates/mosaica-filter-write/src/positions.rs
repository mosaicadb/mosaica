//! The writer of a text layer's positions file. `mosaica_filter::positions` describes the format
//! and reads it.

use std::io;
use std::path::Path;

use bitpacking::{BitPacker, BitPacker4x};
use mosaica_authz::RecordWriter;
use mosaica_filter::{put_varint, POSITIONS_BLOCK, POSITIONS_COLUMN};

/// Writes a positions file record by record, each record streamed into the file block by block
/// and its block table written back over its head when it ends.
pub(crate) struct PositionsWriter {
    file: RecordWriter,
    /// Items the open term's posting holds, and how many of them have been pushed.
    items: u64,
    pushed: u64,
    open: bool,
    /// The open record's block table, and how many bytes of it are written, the table included.
    table: Vec<u8>,
    written: u64,
    blocks: u64,
    /// The open block: its items' counts less one, and their position gaps.
    counts: Vec<u32>,
    gaps: Vec<u32>,
    bytes: Vec<u8>,
}

impl PositionsWriter {
    pub(crate) fn create(path: &Path) -> io::Result<PositionsWriter> {
        Ok(PositionsWriter {
            file: RecordWriter::create(path, POSITIONS_COLUMN)?,
            items: 0,
            pushed: 0,
            open: false,
            table: Vec::new(),
            written: 0,
            blocks: 0,
            counts: Vec::with_capacity(POSITIONS_BLOCK),
            gaps: Vec::new(),
            bytes: Vec::new(),
        })
    }

    /// Begin the next term's record, for a posting of `items` items.
    pub(crate) fn begin(&mut self, items: u64) -> io::Result<()> {
        if self.open {
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
        self.items = items;
        self.pushed = 0;
        self.open = true;
        self.table.clear();
        self.written = head as u64;
        self.blocks = 0;
        self.counts.clear();
        self.gaps.clear();
        Ok(())
    }

    /// The next item's positions, which must ascend strictly.
    pub(crate) fn push(&mut self, positions: &[u32]) -> io::Result<()> {
        if !self.open || self.pushed == self.items {
            return Err(invalid(format!(
                "an item's positions arrived past the {} items the term's posting holds",
                self.items
            )));
        }
        let Some((&first, _)) = positions.split_first() else {
            return Err(invalid("an item holding a term holds it at a position"));
        };
        if !positions.is_sorted_by(|a, b| a < b) {
            return Err(invalid("an item's positions must ascend strictly"));
        }
        self.counts.push(positions.len() as u32 - 1);
        self.gaps.push(first);
        self.gaps.extend(positions.windows(2).map(|w| w[1] - w[0]));
        self.pushed += 1;
        if self.counts.len() == POSITIONS_BLOCK {
            self.write_block()?;
        }
        Ok(())
    }

    /// End the term's record, every item pushed.
    pub(crate) fn end(&mut self) -> io::Result<()> {
        if !self.open || self.pushed != self.items {
            return Err(invalid(format!(
                "a term's positions ended after {} of the {} items its posting holds",
                self.pushed, self.items
            )));
        }
        if !self.counts.is_empty() {
            self.write_block()?;
        }
        self.file.end_record(&self.table)?;
        self.open = false;
        Ok(())
    }

    /// Write a whole record as another positions file holds it, between terms.
    pub(crate) fn copy(&mut self, record: &[u8]) -> io::Result<()> {
        if self.open {
            return Err(invalid(
                "a term's positions were copied inside another term's record",
            ));
        }
        self.file.append(record)
    }

    fn write_block(&mut self) -> io::Result<()> {
        if self.blocks > 0 {
            self.table.extend_from_slice(&self.written.to_le_bytes());
        }
        self.bytes.clear();
        put_run(&self.counts, &mut self.bytes);
        for run in self.gaps.chunks(POSITIONS_BLOCK) {
            put_run(run, &mut self.bytes);
        }
        self.file.extend_record(&self.bytes)?;
        self.written += self.bytes.len() as u64;
        self.blocks += 1;
        self.counts.clear();
        self.gaps.clear();
        Ok(())
    }

    pub(crate) fn finish(self) -> io::Result<()> {
        if self.open {
            return Err(invalid(
                "the positions file finished inside a term's record",
            ));
        }
        self.file.finish()
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
