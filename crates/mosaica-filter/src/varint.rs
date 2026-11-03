//! LEB128 varints over `u32`, written to and read from byte slices. The record blob's blocks and
//! the text index's positions records use this one pair.

/// Append `value` to `out` as a LEB128 varint, at most five bytes.
pub fn put_varint(mut value: u32, out: &mut Vec<u8>) {
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

/// Why [`take_varint`] refused, phrased to follow the name of what was being read.
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
            VarintError::TooWide => "holds a varint that does not fit u32",
            VarintError::Padded => "holds a varint padded with a redundant continuation byte",
        })
    }
}

/// One LEB128 varint from `bytes` at `cursor`, advancing it. Refuses a truncated varint, one wider
/// than `u32`, and one with a redundant trailing zero byte, so each value has one encoding.
pub fn take_varint(bytes: &[u8], cursor: &mut usize) -> Result<u32, VarintError> {
    let mut value: u32 = 0;
    let mut shift = 0u32;
    loop {
        let Some(byte) = bytes.get(*cursor).copied() else {
            return Err(VarintError::Truncated);
        };
        *cursor += 1;
        let payload = u32::from(byte & 0x7f);
        let Some(shifted) = payload
            .checked_shl(shift)
            .filter(|v| (v >> shift) == payload)
        else {
            return Err(VarintError::TooWide);
        };
        value |= shifted;
        if byte & 0x80 == 0 {
            if shift > 0 && byte == 0 {
                return Err(VarintError::Padded);
            }
            return Ok(value);
        }
        shift += 7;
        if shift >= 32 {
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
    }
}
