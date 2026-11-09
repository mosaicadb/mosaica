//! The record container: a file of variable-length records, record *i* at ordinal *i*, each with a
//! `u32` key when the file is keyed. Every postings, positions and delta-tier file is one.
//!
//! Integers are little-endian, and the file is laid out as follows:
//!
//! | Bytes | Holds |
//! |---|---|
//! | `V` | the records, back to back |
//! | 0 to 7 | zeros, to the next multiple of 8 |
//! | `8 (n + 1)` | the offsets, `i64`: record *i* is bytes `o[i]..o[i + 1]` of the values, with `o[0] = 0` and `o[n] = V` |
//! | `4 n`, keyed files only | the keys, `u32`, strictly ascending |
//! | 48 | the trailer |
//!
//! The trailer is the magic `MOSAREC\0`, the container version as a `u32`, one byte that is 1 when
//! keys are present and 0 when not, three zero bytes, the column name zero-padded to 16 bytes, the
//! record count `n` as a `u64` and `V` as a `u64`.
//!
//! [`RecordWriter`] writes each record into the file as it arrives, so the records, which are nearly
//! all of the file's bytes, are written once. The offsets and keys follow the records because the
//! records' total length is not known until the last one arrives, so the writer spools them to
//! unnamed files beside the file and `finish` appends them. A record written in pieces reserves its
//! head, which its end writes back in place.
//!
//! The reader maps the file, or reads it into a buffer aligned for `i64`, and borrows the values,
//! offsets and keys from it without copying. It refuses a file whose length is not what its trailer
//! describes, or whose offsets do not ascend within the values, before any record is read.

use std::fs::File;
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use arrow::array::LargeBinaryArray;
use arrow::buffer::{Buffer, MutableBuffer, OffsetBuffer, ScalarBuffer};

use crate::postings::{invalid_data, mapped_buffer};

const MAGIC: [u8; 8] = *b"MOSAREC\0";
const VERSION: u32 = 1;
const COLUMN_LEN: usize = 16;
const TRAILER_LEN: usize = 48;

/// Writes a record container record by record. A writer dropped before [`Self::finish`] succeeds
/// removes its file.
pub struct RecordWriter {
    path: PathBuf,
    file: BufWriter<File>,
    offsets: BufWriter<File>,
    /// The keys of a keyed file, and the last key appended.
    keys: Option<(BufWriter<File>, Option<u32>)>,
    column: [u8; COLUMN_LEN],
    /// The values' length so far, which is the next record's offset.
    written: i64,
    records: u64,
    /// A record being written in pieces: the bytes written since it began, and the length of the
    /// head its end writes back over its first bytes.
    open: Option<(usize, usize)>,
    finished: bool,
}

impl RecordWriter {
    /// Create (truncating) the file at `path`, whose one column is named `column`.
    pub fn create(path: &Path, column: &str) -> io::Result<Self> {
        RecordWriter::new(path, column, false)
    }

    /// [`Self::create`] for a keyed file, whose records are appended with
    /// [`Self::append_keyed`].
    pub fn create_keyed(path: &Path, column: &str) -> io::Result<Self> {
        RecordWriter::new(path, column, true)
    }

    fn new(path: &Path, column: &str, keyed: bool) -> io::Result<Self> {
        let mut name = [0u8; COLUMN_LEN];
        if column.len() > COLUMN_LEN || column.as_bytes().contains(&0) {
            return Err(invalid_input(format!(
                "a record column's name is at most {COLUMN_LEN} bytes with no NUL; '{column}' \
                 is not"
            )));
        }
        name[..column.len()].copy_from_slice(column.as_bytes());
        let dir = match path.parent() {
            Some(dir) if !dir.as_os_str().is_empty() => dir,
            _ => Path::new("."),
        };
        let mut offsets = BufWriter::new(tempfile::tempfile_in(dir)?);
        offsets.write_all(&0i64.to_le_bytes())?;
        let keys = match keyed {
            true => Some((BufWriter::new(tempfile::tempfile_in(dir)?), None)),
            false => None,
        };
        Ok(RecordWriter {
            path: path.to_path_buf(),
            file: BufWriter::new(File::create(path)?),
            offsets,
            keys,
            column: name,
            written: 0,
            records: 0,
            open: None,
            finished: false,
        })
    }

    /// How many records have been appended.
    pub fn len(&self) -> u64 {
        self.records
    }

    pub fn is_empty(&self) -> bool {
        self.records == 0
    }

    /// Append the next record of an unkeyed file.
    pub fn append(&mut self, record: &[u8]) -> io::Result<()> {
        self.unkeyed("append")?;
        self.write_record(record)
    }

    /// Append the next record of a keyed file, under `key`, which must be above every key appended
    /// before it.
    pub fn append_keyed(&mut self, key: u32, record: &[u8]) -> io::Result<()> {
        let Some((keys, last)) = self.keys.as_mut() else {
            return Err(invalid_input(
                "a keyed record was appended to an unkeyed file; call append instead",
            ));
        };
        if let Some(last) = *last {
            if key <= last {
                return Err(invalid_input(format!(
                    "key {key} follows {last}, but a keyed file's keys must ascend strictly"
                )));
            }
        }
        keys.write_all(&key.to_le_bytes())?;
        *last = Some(key);
        self.write_record(record)
    }

    fn write_record(&mut self, record: &[u8]) -> io::Result<()> {
        if self.open.is_some() {
            return Err(invalid_input(
                "a record was appended while another was being written",
            ));
        }
        let next = next_offset(self.written, record.len())?;
        self.file.write_all(record)?;
        self.push_offset(next)
    }

    fn push_offset(&mut self, next: i64) -> io::Result<()> {
        self.offsets.write_all(&next.to_le_bytes())?;
        self.written = next;
        self.records += 1;
        Ok(())
    }

    fn unkeyed(&self, call: &str) -> io::Result<()> {
        match self.keys {
            Some(_) => Err(invalid_input(format!(
                "{call} was called on a keyed file; append each record with its key through \
                 append_keyed"
            ))),
            None => Ok(()),
        }
    }

    /// Begin a record of an unkeyed file written in pieces, with `head_len` bytes at its head that
    /// [`Self::end_record`] fills once the rest is written.
    pub fn begin_record(&mut self, head_len: usize) -> io::Result<()> {
        self.unkeyed("begin_record")?;
        if self.open.is_some() {
            return Err(invalid_input(
                "a record began while another was being written",
            ));
        }
        self.file.write_all(&vec![0u8; head_len])?;
        self.open = Some((head_len, head_len));
        Ok(())
    }

    /// Append `bytes` to the record [`Self::begin_record`] began.
    pub fn extend_record(&mut self, bytes: &[u8]) -> io::Result<()> {
        let Some((written, _)) = self.open.as_mut() else {
            return Err(invalid_input("record bytes arrived with no record begun"));
        };
        self.file.write_all(bytes)?;
        *written += bytes.len();
        Ok(())
    }

    /// Close the record [`Self::begin_record`] began, writing `head` over its first bytes.
    pub fn end_record(&mut self, head: &[u8]) -> io::Result<()> {
        let Some((written, head_len)) = self.open.take() else {
            return Err(invalid_input("a record ended with no record begun"));
        };
        if head.len() != head_len {
            return Err(invalid_input(format!(
                "a record's head is {} bytes where {head_len} were reserved",
                head.len()
            )));
        }
        let start = self.written;
        if !head.is_empty() {
            self.file.seek(SeekFrom::Start(start as u64))?;
            self.file.write_all(head)?;
            self.file.seek(SeekFrom::End(0))?;
        }
        self.push_offset(next_offset(start, written)?)
    }

    /// Append the offsets, the keys and the trailer, completing the file.
    pub fn finish(mut self) -> io::Result<()> {
        self.write_tail()?;
        self.finished = true;
        Ok(())
    }

    fn write_tail(&mut self) -> io::Result<()> {
        if self.open.is_some() {
            return Err(invalid_input(
                "the file finished while a record was being written",
            ));
        }
        let values = self.written as u64;
        self.file.write_all(&[0u8; 8][..padding(values) as usize])?;
        append_spooled(&mut self.offsets, &mut self.file, 8 * (self.records + 1))?;
        if let Some((keys, _)) = self.keys.as_mut() {
            append_spooled(keys, &mut self.file, 4 * self.records)?;
        }
        let mut trailer = [0u8; TRAILER_LEN];
        trailer[0..8].copy_from_slice(&MAGIC);
        trailer[8..12].copy_from_slice(&VERSION.to_le_bytes());
        trailer[12] = u8::from(self.keys.is_some());
        trailer[16..32].copy_from_slice(&self.column);
        trailer[32..40].copy_from_slice(&self.records.to_le_bytes());
        trailer[40..48].copy_from_slice(&values.to_le_bytes());
        self.file.write_all(&trailer)?;
        self.file.flush()
    }
}

impl Drop for RecordWriter {
    fn drop(&mut self) {
        if !self.finished {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Copy the whole of `spool`, which must be `len` bytes, onto the end of `out`.
fn append_spooled(
    spool: &mut BufWriter<File>,
    out: &mut BufWriter<File>,
    len: u64,
) -> io::Result<()> {
    spool.flush()?;
    let file = spool.get_mut();
    file.seek(SeekFrom::Start(0))?;
    let copied = io::copy(file, out)?;
    if copied != len {
        return Err(invalid_data(format!(
            "a record file's spool held {copied} bytes where {len} were written to it"
        )));
    }
    Ok(())
}

/// The next offset, refused where the values would pass `i64::MAX` bytes.
fn next_offset(last: i64, record_len: usize) -> io::Result<i64> {
    i64::try_from(record_len)
        .ok()
        .and_then(|len| last.checked_add(len))
        .ok_or_else(|| {
            invalid_input(format!(
                "a record of {record_len} bytes takes the file past i64::MAX bytes"
            ))
        })
}

/// The zeros after `values` bytes that bring the offsets to a multiple of 8.
fn padding(values: u64) -> u64 {
    values.wrapping_neg() % 8
}

/// Open an unkeyed record file whose column is named `column`, and return its records without
/// copying them. When `mmap` is `true` the file is mapped, and otherwise read into memory.
pub fn open_record_column(path: &Path, mmap: bool, column: &str) -> io::Result<LargeBinaryArray> {
    Ok(open_records(path, mmap, column, false)?.0)
}

/// [`open_record_column`] for a keyed file, mapped, returning its keys beside its records.
pub(crate) fn open_keyed_record_column(
    path: &Path,
    column: &str,
) -> io::Result<(ScalarBuffer<u32>, LargeBinaryArray)> {
    let (records, keys) = open_records(path, true, column, true)?;
    Ok((keys.unwrap_or_default(), records))
}

fn open_records(
    path: &Path,
    mmap: bool,
    column: &str,
    keyed: bool,
) -> io::Result<(LargeBinaryArray, Option<ScalarBuffer<u32>>)> {
    let what = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let damaged = |detail: String| invalid_data(format!("{what}: {detail}"));
    let file = File::open(path)?;
    let len = file.metadata()?.len();
    if len < TRAILER_LEN as u64 {
        return Err(damaged(format!(
            "is {len} bytes, shorter than the {TRAILER_LEN}-byte trailer of a record file"
        )));
    }
    let buffer = match mmap {
        true => mapped_buffer(&file)?,
        false => read_aligned(file, len)?,
    };
    let len = buffer.len();
    if len < TRAILER_LEN {
        return Err(damaged(format!("is {len} bytes, shorter than its trailer")));
    }
    let trailer = &buffer[len - TRAILER_LEN..];
    let u64_at = |at: usize| u64::from_le_bytes(trailer[at..at + 8].try_into().expect("8 bytes"));
    if trailer[0..8] != MAGIC {
        return Err(damaged(
            "is not a record file: its trailer has no magic".into(),
        ));
    }
    let version = u32::from_le_bytes(trailer[8..12].try_into().expect("4 bytes"));
    if version != VERSION {
        return Err(damaged(format!(
            "is record file version {version}, and this build reads version {VERSION}; rebuild \
             the bundle"
        )));
    }
    let has_keys = match trailer[12] {
        0 => false,
        1 => true,
        other => {
            return Err(damaged(format!(
                "has keys flag {other}, which is not 0 or 1"
            )))
        }
    };
    if trailer[13..16] != [0; 3] {
        return Err(damaged("has nonzero reserved trailer bytes".into()));
    }
    let name = &trailer[16..32];
    let name = &name[..name.iter().position(|b| *b == 0).unwrap_or(COLUMN_LEN)];
    if name != column.as_bytes() || has_keys != keyed {
        return Err(damaged(format!(
            "holds column '{}' {}, where {} column '{column}' was expected",
            String::from_utf8_lossy(name),
            if has_keys {
                "with keys"
            } else {
                "without keys"
            },
            if keyed { "a keyed" } else { "an unkeyed" },
        )));
    }
    let records = u64_at(32);
    let values = u64_at(40);

    let entries = records.checked_add(1);
    let offsets_at = values.checked_add(padding(values));
    let keys_at = entries
        .and_then(|e| e.checked_mul(8))
        .zip(offsets_at)
        .and_then(|(o, at)| at.checked_add(o));
    let keys_len = records.checked_mul(if keyed { 4 } else { 0 });
    let expected = keys_at
        .zip(keys_len)
        .and_then(|(at, k)| at.checked_add(k))
        .and_then(|end| end.checked_add(TRAILER_LEN as u64));
    if expected != Some(len as u64) {
        return Err(damaged(format!(
            "is {len} bytes, where its trailer's {records} records over {values} bytes of values \
             make a file of {}",
            expected.map_or_else(|| "more than 2^64 bytes".to_string(), |e| e.to_string())
        )));
    }
    // The file's length bounds every figure below, so none of them overflows `usize`.
    let (records, values) = (records as usize, values as usize);
    let offsets_at = values + padding(values as u64) as usize;
    if buffer[values..offsets_at].iter().any(|b| *b != 0) {
        return Err(damaged("has nonzero padding after its values".into()));
    }
    let offsets_bytes = buffer.slice_with_length(offsets_at, 8 * (records + 1));
    if offsets_bytes
        .as_ptr()
        .align_offset(std::mem::align_of::<i64>())
        != 0
    {
        return Err(damaged("has offsets that are not aligned in memory".into()));
    }
    let offsets = ScalarBuffer::<i64>::new(offsets_bytes, 0, records + 1);
    if offsets[0] != 0
        || offsets[records] != values as i64
        || offsets.windows(2).any(|w| w[0] > w[1])
    {
        return Err(damaged(format!(
            "has offsets that do not ascend from 0 to its {values} bytes of values"
        )));
    }
    // SAFETY: the offsets were checked just above to be non-negative and non-decreasing.
    let offsets = unsafe { OffsetBuffer::new_unchecked(offsets) };
    let array = LargeBinaryArray::try_new(offsets, buffer.slice_with_length(0, values), None)
        .map_err(|e| damaged(e.to_string()))?;

    let keys = keyed.then(|| {
        let at = offsets_at + 8 * (records + 1);
        ScalarBuffer::<u32>::new(buffer.slice_with_length(at, 4 * records), 0, records)
    });
    if let Some(keys) = &keys {
        if keys.windows(2).any(|w| w[0] >= w[1]) {
            return Err(damaged(
                "has keys that do not ascend strictly, so a lookup could not find them".into(),
            ));
        }
    }
    Ok((array, keys))
}

/// Read `file`, `len` bytes, into a buffer aligned for any integer the file holds.
fn read_aligned(mut file: File, len: u64) -> io::Result<Buffer> {
    let len = usize::try_from(len)
        .map_err(|_| invalid_data("a record file is larger than memory can address"))?;
    let mut buffer = MutableBuffer::from_len_zeroed(len);
    file.read_exact(buffer.as_slice_mut())?;
    Ok(buffer.into())
}

fn invalid_input(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::Array;

    fn write(path: &Path, keys: Option<&[u32]>, records: &[&[u8]]) {
        let mut writer = match keys {
            Some(_) => RecordWriter::create_keyed(path, "c").unwrap(),
            None => RecordWriter::create(path, "c").unwrap(),
        };
        for (at, record) in records.iter().enumerate() {
            match keys {
                Some(keys) => writer.append_keyed(keys[at], record).unwrap(),
                None => writer.append(record).unwrap(),
            }
        }
        writer.finish().unwrap();
    }

    fn values(array: &LargeBinaryArray) -> Vec<&[u8]> {
        (0..array.len()).map(|i| array.value(i)).collect()
    }

    /// **Keyed and unkeyed files read back as written**, mapped or read, empty or not, with values
    /// that need padding; and a writer dropped before it finishes leaves no file.
    #[test]
    fn records_and_keys_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let records: [&[u8]; 4] = [b"abc", b"", b"defg", &[7; 300]];
        let keys = [0, 9, 1 << 20, u32::MAX];
        for n in [0, 1, records.len()] {
            let path = dir.path().join(format!("plain-{n}"));
            write(&path, None, &records[..n]);
            for mmap in [false, true] {
                let array = open_record_column(&path, mmap, "c").unwrap();
                assert_eq!(values(&array), &records[..n]);
            }
            let path = dir.path().join(format!("keyed-{n}"));
            write(&path, Some(&keys), &records[..n]);
            let (read_keys, array) = open_keyed_record_column(&path, "c").unwrap();
            assert_eq!(&read_keys[..], &keys[..n]);
            assert_eq!(values(&array), &records[..n]);
        }

        let path = dir.path().join("dropped");
        let mut writer = RecordWriter::create(&path, "c").unwrap();
        writer.append(b"abc").unwrap();
        drop(writer);
        assert!(!path.exists());
    }

    /// **A damaged file is refused with an error**, mapped or read, never read short or past its
    /// end: truncated, extended, without the padding that aligns its offsets, with offsets that do
    /// not ascend from zero to the values' length, with a trailer whose figures overflow, and with
    /// keys that do not ascend.
    #[test]
    fn a_damaged_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let records: [&[u8]; 3] = [b"abc", b"", b"defg"];
        let plain = dir.path().join("plain");
        write(&plain, None, &records);
        let keyed = dir.path().join("keyed");
        write(&keyed, Some(&[1, 2, 3]), &records);
        // Seven bytes of values, one of padding, then four offsets.
        fn offset(b: &mut [u8], i: usize, to: i64) {
            b[8 + 8 * i..16 + 8 * i].copy_from_slice(&to.to_le_bytes())
        }
        fn trailer(b: &[u8]) -> usize {
            b.len() - TRAILER_LEN
        }
        type Damage = fn(&mut Vec<u8>);
        let cases: [(&str, bool, Damage); 12] = [
            ("empty", false, |b| b.clear()),
            ("truncated by a byte", false, |b| {
                b.pop();
            }),
            ("only its trailer", false, |b| {
                b.drain(..b.len() - TRAILER_LEN);
            }),
            ("extended by a byte", false, |b| b.push(0)),
            ("extended by an offset", false, |b| b.extend([0; 8])),
            ("without its padding", false, |b| {
                b.remove(7);
            }),
            ("offsets that descend", false, |b| offset(b, 1, 5)),
            ("a first offset above zero", false, |b| offset(b, 0, 1)),
            ("a last offset short of the values", false, |b| {
                offset(b, 3, 6)
            }),
            ("a record count that overflows", false, |b| {
                let at = trailer(b) + 32;
                b[at..at + 8].copy_from_slice(&u64::MAX.to_le_bytes());
            }),
            ("a values length that overflows", false, |b| {
                let at = trailer(b) + 40;
                b[at..at + 8].copy_from_slice(&(u64::MAX - 3).to_le_bytes());
            }),
            ("keys that repeat", true, |b| {
                let at = trailer(b) - 4;
                b[at..at + 4].copy_from_slice(&2u32.to_le_bytes());
            }),
        ];
        let damaged = dir.path().join("damaged");
        for (case, is_keyed, damage) in cases {
            let mut bytes = std::fs::read(if is_keyed { &keyed } else { &plain }).unwrap();
            damage(&mut bytes);
            std::fs::write(&damaged, &bytes).unwrap();
            let refused = |result: io::Result<()>| {
                let err = result.expect_err(case);
                assert_eq!(err.kind(), io::ErrorKind::InvalidData, "{case}: {err}");
            };
            if is_keyed {
                refused(open_keyed_record_column(&damaged, "c").map(drop));
                continue;
            }
            for mmap in [false, true] {
                refused(open_record_column(&damaged, mmap, "c").map(drop));
            }
        }
    }

    #[test]
    fn an_offset_past_i64_max_is_refused() {
        assert_eq!(next_offset(i64::MAX - 4, 4).unwrap(), i64::MAX);
        assert!(next_offset(i64::MAX, 1).is_err());
    }
}
