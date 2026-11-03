//! LEB128 varints over `u32` and `u64`, written to and read from byte slices. The record blob's
//! blocks, the text index's positions records and the build's spill files use these pairs.

/// Append `value` to `out` as a LEB128 varint, at most five bytes.
pub fn put_varint(value: u32, out: &mut Vec<u8>) {
    put_varint64(u64::from(value), out);
}

/// Append `value` to `out` as a LEB128 varint, at most ten bytes. A value that fits `u32` takes
/// the bytes [`put_varint`] writes.
pub fn put_varint64(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// Why [`take_varint`] or [`take_varint64`] refused, phrased to follow the name of what was being
/// read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarintError {
    Truncated,
    TooWide,
    Padded,
}

impl std::fmt::Display for VarintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            VarintError::Truncated => "ends in the middle of a varint",
            VarintError::TooWide => "holds a varint wider than the integer it encodes",
            VarintError::Padded => "holds a varint padded with a redundant continuation byte",
        })
    }
}

/// One LEB128 varint from `bytes` at `cursor`, advancing it. Refuses a truncated varint, one wider
/// than `u32`, and one with a redundant trailing zero byte, so each value has one encoding.
pub fn take_varint(bytes: &[u8], cursor: &mut usize) -> Result<u32, VarintError> {
    take::<32>(bytes, cursor).map(|value| value as u32)
}

/// [`take_varint`] at the `u64` width.
pub fn take_varint64(bytes: &[u8], cursor: &mut usize) -> Result<u64, VarintError> {
    take::<64>(bytes, cursor)
}

/// One varint of at most `BITS` bits. `cursor` is advanced past every byte read, refused or not.
#[inline]
fn take<const BITS: u32>(bytes: &[u8], cursor: &mut usize) -> Result<u64, VarintError> {
    let mut value: u64 = 0;
    let mut shift = 0u32;
    loop {
        let Some(byte) = bytes.get(*cursor).copied() else {
            return Err(VarintError::Truncated);
        };
        *cursor += 1;
        let payload = u64::from(byte & 0x7f);
        if BITS - shift < 7 && payload >> (BITS - shift) != 0 {
            return Err(VarintError::TooWide);
        }
        value |= payload << shift;
        if byte & 0x80 == 0 {
            if shift > 0 && byte == 0 {
                return Err(VarintError::Padded);
            }
            return Ok(value);
        }
        shift += 7;
        if shift >= BITS {
            return Err(VarintError::TooWide);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_width_round_trips_and_malformed_ones_refuse() {
        let values = [0u32, 1, 127, 128, 16_383, 16_384, u32::MAX >> 4, u32::MAX];
        let mut bytes = Vec::new();
        for &v in &values {
            put_varint(v, &mut bytes);
        }
        let mut cursor = 0;
        for &v in &values {
            assert_eq!(take_varint(&bytes, &mut cursor), Ok(v));
        }
        assert_eq!(cursor, bytes.len());

        assert_eq!(take_varint(&[0x80], &mut 0), Err(VarintError::Truncated));
        assert_eq!(take_varint(&[0x80, 0x00], &mut 0), Err(VarintError::Padded));
        assert_eq!(
            take_varint(&[0xff, 0xff, 0xff, 0xff, 0x1f], &mut 0),
            Err(VarintError::TooWide)
        );
        assert_eq!(
            take_varint(&[0xff, 0xff, 0xff, 0xff, 0x8f], &mut 0),
            Err(VarintError::TooWide)
        );
    }

    /// The `u64` pair writes a `u32` value as the `u32` pair does, reads back every width, and
    /// refuses the same shapes at its own width.
    #[test]
    fn the_u64_pair_extends_the_u32_one() {
        for v in [0u32, 127, 128, u32::MAX] {
            let (mut narrow, mut wide) = (Vec::new(), Vec::new());
            put_varint(v, &mut narrow);
            put_varint64(u64::from(v), &mut wide);
            assert_eq!(narrow, wide);
        }

        let values = [0u64, 1, 1 << 35, u64::from(u32::MAX) + 1, u64::MAX >> 1, u64::MAX];
        let mut bytes = Vec::new();
        for &v in &values {
            put_varint64(v, &mut bytes);
        }
        let mut cursor = 0;
        for &v in &values {
            assert_eq!(take_varint64(&bytes, &mut cursor), Ok(v));
        }
        assert_eq!(cursor, bytes.len());

        let widest = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01];
        for len in 0..widest.len() {
            assert_eq!(
                take_varint64(&widest[..len], &mut 0),
                Err(VarintError::Truncated)
            );
        }
        let mut over = widest;
        over[9] = 0x03;
        assert_eq!(take_varint64(&over, &mut 0), Err(VarintError::TooWide));
        over[9] = 0x81;
        assert_eq!(take_varint64(&over, &mut 0), Err(VarintError::TooWide));
        assert_eq!(take_varint64(&[0x80, 0x00], &mut 0), Err(VarintError::Padded));
    }
}
