//! A file of `(key, value)` in ascending key order: what one field's holdings are kept in between
//! the files of a build.
//!
//! An integer key is written as its gap from the one before, in LEB128, and a keyword's hash whole;
//! the value follows in four bytes. The receipt carries the count and a mixed sum of what was
//! written, and the reader checks both at the end.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use mosaica_filter::{put_varint64, take_varint64};
use mosaica_filter_write::{SpillReceipt, SPILL_BUF_BYTES};
use mosaica_store::key_index::Key;
use mosaica_types::splitmix64;

use crate::error::{BuildError, Result};
use crate::spill::read_varint;

fn entry_anchor(key: u128, value: u32) -> u64 {
    splitmix64(key as u64 ^ (key >> 64) as u64).wrapping_add(splitmix64(u64::from(value) << 1 | 1))
}

pub(super) struct RunWriter<K: Key> {
    path: PathBuf,
    writer: BufWriter<File>,
    record: Vec<u8>,
    last: u128,
    count: u64,
    anchor: u64,
    key: PhantomData<K>,
}

impl<K: Key> RunWriter<K> {
    pub fn create(path: &Path) -> Result<Self> {
        let file = File::create(path).map_err(|e| BuildError::io(path, e))?;
        Ok(RunWriter {
            path: path.to_path_buf(),
            writer: BufWriter::with_capacity(SPILL_BUF_BYTES, file),
            record: Vec::new(),
            last: 0,
            count: 0,
            anchor: 0,
            key: PhantomData,
        })
    }

    /// The next entry; its key is at or above the last one's.
    pub fn push(&mut self, key: K, value: u32) -> Result<()> {
        let wide = key.widen();
        debug_assert!(wide >= self.last, "a run's keys ascend");
        self.record.clear();
        if K::WIDTH <= 8 {
            put_varint64((wide - self.last) as u64, &mut self.record);
        } else {
            self.record.resize(K::WIDTH, 0);
            key.write(&mut self.record);
        }
        self.record.extend_from_slice(&value.to_le_bytes());
        self.writer
            .write_all(&self.record)
            .map_err(|e| BuildError::io(&self.path, e))?;
        self.last = wide;
        self.count += 1;
        self.anchor = self.anchor.wrapping_add(entry_anchor(wide, value));
        Ok(())
    }

    pub fn finish(self) -> Result<SpillReceipt> {
        let RunWriter {
            path,
            writer,
            count,
            anchor,
            ..
        } = self;
        writer
            .into_inner()
            .map_err(|e| BuildError::io(&path, e.into_error()))?;
        Ok(SpillReceipt {
            path,
            count,
            anchor,
        })
    }
}

pub(super) struct RunReader<K: Key> {
    receipt: SpillReceipt,
    reader: BufReader<File>,
    last: u128,
    left: u64,
    anchor: u64,
    key: PhantomData<K>,
}

impl<K: Key> RunReader<K> {
    pub fn open(receipt: &SpillReceipt) -> Result<Self> {
        let file = File::open(&receipt.path).map_err(|e| BuildError::io(&receipt.path, e))?;
        Ok(RunReader {
            receipt: receipt.clone(),
            reader: BufReader::with_capacity(SPILL_BUF_BYTES, file),
            last: 0,
            left: receipt.count,
            anchor: 0,
            key: PhantomData,
        })
    }

    /// The next entry, or `None` past the last, which is where the run is checked.
    pub fn next_entry(&mut self) -> Result<Option<(K, u32)>> {
        if self.left == 0 {
            let mut rest = [0u8; 1];
            let trailing = self
                .reader
                .read(&mut rest)
                .map_err(|e| BuildError::io(&self.receipt.path, e))?;
            if trailing != 0 || self.anchor != self.receipt.anchor {
                return Err(self.not_this_run("its entries do not add up to its receipt"));
            }
            return Ok(None);
        }
        let wide = if K::WIDTH <= 8 {
            let gap = read_varint(&mut self.reader, &self.receipt.path, "key run", take_varint64)
                .map_err(|e| match e {
                    BuildError::Invalid(_) => {
                        self.not_this_run("a key gap is not a varint of at most 64 bits")
                    }
                    e => e,
                })?;
            self.last + u128::from(gap)
        } else {
            let mut bytes = [0u8; 16];
            self.reader
                .read_exact(&mut bytes[..K::WIDTH])
                .map_err(|e| BuildError::io(&self.receipt.path, e))?;
            K::read(&bytes[..K::WIDTH]).widen()
        };
        let mut value = [0u8; 4];
        self.reader
            .read_exact(&mut value)
            .map_err(|e| BuildError::io(&self.receipt.path, e))?;
        let value = u32::from_le_bytes(value);
        self.last = wide;
        self.left -= 1;
        self.anchor = self.anchor.wrapping_add(entry_anchor(wide, value));
        Ok(Some((K::narrow(wide), value)))
    }

    fn not_this_run(&self, why: &str) -> BuildError {
        BuildError::Invalid(format!(
            "{} is not the run this build wrote: {why}. Build again",
            self.receipt.path.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_reads_back_what_was_written_at_either_width() {
        let dir = tempfile::tempdir().unwrap();
        let ints: Vec<(u64, u32)> = vec![(0, 7), (0, 8), (5, 1), (1 << 40, 2), (u64::MAX, 3)];
        let path = dir.path().join("ints");
        let mut writer = RunWriter::<u64>::create(&path).unwrap();
        for &(key, value) in &ints {
            writer.push(key, value).unwrap();
        }
        let receipt = writer.finish().unwrap();
        let mut reader = RunReader::<u64>::open(&receipt).unwrap();
        let mut back = Vec::new();
        while let Some(entry) = reader.next_entry().unwrap() {
            back.push(entry);
        }
        assert_eq!(back, ints);

        let words: Vec<(u128, u32)> = vec![(3, 1), (u128::MAX - 1, 2)];
        let path = dir.path().join("words");
        let mut writer = RunWriter::<u128>::create(&path).unwrap();
        for &(key, value) in &words {
            writer.push(key, value).unwrap();
        }
        let receipt = writer.finish().unwrap();
        let mut reader = RunReader::<u128>::open(&receipt).unwrap();
        let mut back = Vec::new();
        while let Some(entry) = reader.next_entry().unwrap() {
            back.push(entry);
        }
        assert_eq!(back, words);
    }

    #[test]
    fn a_run_whose_bytes_changed_is_refused_at_its_end() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run");
        let mut writer = RunWriter::<u64>::create(&path).unwrap();
        writer.push(10, 1).unwrap();
        writer.push(20, 2).unwrap();
        let receipt = writer.finish().unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        std::fs::write(&path, bytes).unwrap();
        let mut reader = RunReader::<u64>::open(&receipt).unwrap();
        let refused = loop {
            match reader.next_entry() {
                Ok(Some(_)) => continue,
                Ok(None) => break false,
                Err(_) => break true,
            }
        };
        assert!(refused);
    }

    /// **A gap whose bytes never end is refused, not overflowed.**
    #[test]
    fn a_run_whose_gap_runs_past_64_bits_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run");
        let mut writer = RunWriter::<u64>::create(&path).unwrap();
        writer.push(10, 1).unwrap();
        let receipt = writer.finish().unwrap();
        std::fs::write(&path, [0xffu8; 16]).unwrap();
        let mut reader = RunReader::<u64>::open(&receipt).unwrap();
        assert!(reader.next_entry().is_err());
    }

    /// A run of one entry whose bytes are `bytes`, read back.
    fn read_one(bytes: &[u8]) -> Result<Option<(u64, u32)>> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run");
        let mut writer = RunWriter::<u64>::create(&path).unwrap();
        writer.push(10, 1).unwrap();
        let receipt = writer.finish().unwrap();
        std::fs::write(&path, bytes).unwrap();
        RunReader::<u64>::open(&receipt).unwrap().next_entry()
    }

    /// **A gap of ten bytes whose last carries bits past 64 is refused**, not truncated to fit.
    #[test]
    fn a_gap_wider_than_64_bits_is_refused() {
        let mut bytes = vec![0xffu8; 9];
        bytes.push(0x02);
        bytes.extend_from_slice(&1u32.to_le_bytes());
        assert!(matches!(read_one(&bytes), Err(BuildError::Invalid(_))));
    }

    #[test]
    fn a_run_that_ends_inside_a_gap_is_refused() {
        assert!(matches!(read_one(&[0x8a]), Err(BuildError::Invalid(_))));
    }
}
