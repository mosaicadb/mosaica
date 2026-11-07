//! An integer or timestamp field's value written as a string, as JSON and `corpus.toml` carry it.
//!
//! Digits, optionally after a `-` or `+`, are the integer itself, on a timestamp field too, where
//! they count microseconds since the Unix epoch. A timestamp field also takes an RFC 3339 date
//! (`2020-01-01`, midnight UTC) or date-time with an offset (`2020-01-01T09:30:00Z`,
//! `2020-01-01T09:30:00.25+01:00`), the date and time parted by `T`, `t` or a space. A date-time with no offset names no instant and is refused, as
//! is a seventh fractional digit, which a microsecond cannot hold. A leap second, `23:59:60`, is
//! the second after `23:59:59`.

use std::fmt;

/// How a string spells a field's value: the field's kind decides whether a date is one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrittenAs {
    /// An integer field of any width: digits only.
    Integer,
    /// A `timestamp_us` field: digits, or an RFC 3339 date or date-time.
    Timestamp,
}

/// Why a string is not a value of its field. Its `Display` follows the quoted string in a
/// sentence: `'2020-01-01T09:30' {error}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrittenError {
    NotAnInteger,
    NotATimestamp,
    NoOffset,
    TooPrecise,
    OutOfRange,
}

impl fmt::Display for WrittenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            WrittenError::NotAnInteger => {
                "is not an integer; write its decimal digits, optionally after a '-' or '+'"
            }
            WrittenError::NotATimestamp => {
                "is not a timestamp; write a date such as 2026-04-01, a date-time with an offset \
                 such as 2026-04-01T09:30:00Z, or microseconds since the Unix epoch as digits"
            }
            WrittenError::NoOffset => {
                "is a date-time with no offset; add Z or an offset such as +01:00 after the time"
            }
            WrittenError::TooPrecise => {
                "has more than six fractional-second digits; a timestamp holds microseconds, so \
                 write at most six"
            }
            WrittenError::OutOfRange => {
                "is outside what its field holds; write a value that fits the field's declared \
                 type"
            }
        })
    }
}

impl std::error::Error for WrittenError {}

impl WrittenAs {
    /// `Timestamp` where `timestamp` holds, else `Integer`.
    pub fn of(timestamp: bool) -> WrittenAs {
        if timestamp {
            WrittenAs::Timestamp
        } else {
            WrittenAs::Integer
        }
    }

    /// The value `text` writes. Digits past what an `i128` holds are refused; whether the value
    /// fits the field's own width is the caller's question.
    pub fn parse(self, text: &str) -> Result<i128, WrittenError> {
        let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
        if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
            return text.parse::<i128>().map_err(|_| WrittenError::OutOfRange);
        }
        match self {
            WrittenAs::Integer => Err(WrittenError::NotAnInteger),
            WrittenAs::Timestamp => rfc3339_micros(text).map(i128::from),
        }
    }
}

/// A timestamp written as text, in microseconds since the Unix epoch.
pub fn timestamp_us(text: &str) -> Result<i64, WrittenError> {
    let micros = WrittenAs::Timestamp.parse(text)?;
    i64::try_from(micros).map_err(|_| WrittenError::OutOfRange)
}

/// `YYYY-MM-DD`, or that, `T` or a space, `hh:mm:ss`, an optional fraction and `Z` or `±hh:mm`.
fn rfc3339_micros(text: &str) -> Result<i64, WrittenError> {
    let b = text.as_bytes();
    let bad = WrittenError::NotATimestamp;
    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return Err(bad);
    }
    let year = number(&b[0..4]).ok_or(bad)?;
    let month = number(&b[5..7]).ok_or(bad)?;
    let day = number(&b[8..10]).ok_or(bad)?;
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return Err(bad);
    }
    let days = days_from_civil(year, month, day);
    if b.len() == 10 {
        return Ok(days * 86_400_000_000);
    }
    if !matches!(b[10], b'T' | b't' | b' ') {
        return Err(bad);
    }
    let time = &b[11..];
    if time.len() < 8 || time[2] != b':' || time[5] != b':' {
        return Err(bad);
    }
    let hour = number(&time[0..2]).ok_or(bad)?;
    let minute = number(&time[3..5]).ok_or(bad)?;
    let second = number(&time[6..8]).ok_or(bad)?;
    if hour > 23 || minute > 59 || second > 60 {
        return Err(bad);
    }
    let mut rest = &time[8..];
    let mut fraction = 0;
    if let Some(after) = rest.strip_prefix(b".") {
        let width = after.iter().take_while(|b| b.is_ascii_digit()).count();
        if width == 0 {
            return Err(bad);
        }
        if width > 6 {
            return Err(WrittenError::TooPrecise);
        }
        fraction = number(&after[..width]).ok_or(bad)? * 10_i64.pow(6 - width as u32);
        rest = &after[width..];
    }
    let offset = match rest {
        [] => return Err(WrittenError::NoOffset),
        [b'Z' | b'z'] => 0,
        [sign @ (b'+' | b'-'), h1, h2, b':', m1, m2] => {
            let hours = number(&[*h1, *h2]).ok_or(bad)?;
            let minutes = number(&[*m1, *m2]).ok_or(bad)?;
            if hours > 23 || minutes > 59 {
                return Err(bad);
            }
            let seconds = hours * 3600 + minutes * 60;
            if *sign == b'-' {
                -seconds
            } else {
                seconds
            }
        }
        _ => return Err(bad),
    };
    let seconds = days * 86_400 + hour * 3600 + minute * 60 + second - offset;
    Ok(seconds * 1_000_000 + fraction)
}

/// ASCII digits as a number, or `None` if any byte is not a digit.
fn number(digits: &[u8]) -> Option<i64> {
    digits.iter().try_fold(0_i64, |n, &b| {
        b.is_ascii_digit().then(|| n * 10 + i64::from(b - b'0'))
    })
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days from 1970-01-01 to a proleptic Gregorian date.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(text: &str) -> Result<i64, WrittenError> {
        timestamp_us(text)
    }

    #[test]
    fn a_date_is_midnight_utc() {
        assert_eq!(ts("2020-01-01"), Ok(1_577_836_800_000_000));
        assert_eq!(ts("2026-04-01"), Ok(1_775_001_600_000_000));
        assert_eq!(ts("1970-01-01"), Ok(0));
        assert_eq!(ts("0001-01-01"), Ok(-62_135_596_800_000_000));
    }

    #[test]
    fn a_date_time_counts_its_offset_and_fraction() {
        assert_eq!(ts("2026-04-01T00:00:00Z"), Ok(1_775_001_600_000_000));
        assert_eq!(ts("2026-04-01t00:00:00z"), Ok(1_775_001_600_000_000));
        assert_eq!(ts("2020-01-01T01:00:00+01:00"), Ok(1_577_836_800_000_000));
        assert_eq!(ts("1969-12-31T23:59:59.5Z"), Ok(-500_000));
        assert_eq!(
            ts("2000-02-29T12:30:15.123456-05:30"),
            Ok(951_847_215_123_456)
        );
        assert_eq!(
            ts("9999-12-31T23:59:59.999999Z"),
            Ok(253_402_300_799_999_999)
        );
    }

    #[test]
    fn a_leap_second_is_the_next_second() {
        assert_eq!(ts("2016-12-31T23:59:60Z"), ts("2017-01-01T00:00:00Z"));
        assert_eq!(ts("2017-01-01T00:00:00Z"), Ok(1_483_228_800_000_000));
    }

    #[test]
    fn a_space_may_part_the_date_and_time() {
        assert_eq!(ts("2026-04-01 00:00:00Z"), ts("2026-04-01T00:00:00Z"));
        assert_eq!(ts("2026-04-01 01:00:00.5+01:00"), Ok(1_775_001_600_500_000));
    }

    #[test]
    fn digits_are_microseconds_whatever_they_look_like() {
        assert_eq!(ts("20200101"), Ok(20_200_101));
        assert_eq!(ts("-42"), Ok(-42));
        assert_eq!(ts("+42"), Ok(42));
        assert_eq!(WrittenAs::Integer.parse("+5"), Ok(5));
        assert_eq!(WrittenAs::Integer.parse("-42"), Ok(-42));
        assert_eq!(
            WrittenAs::Integer.parse("170141183460469231731687303715884105727"),
            Ok(i128::MAX)
        );
    }

    #[test]
    fn refusals() {
        use WrittenError::*;
        assert_eq!(ts("2020-01-01T12:00:00"), Err(NoOffset));
        assert_eq!(ts("2020-01-01T12:00:00.5"), Err(NoOffset));
        assert_eq!(ts("2020-01-01T12:00:00.1234567Z"), Err(TooPrecise));
        assert_eq!(ts("9223372036854775808"), Err(OutOfRange));
        assert_eq!(ts("2020-01-01 12:00:00"), Err(NoOffset));
        for text in ["+", "-", "5+", "++5", "+-5", "-+5", " 5"] {
            assert_eq!(WrittenAs::Integer.parse(text), Err(NotAnInteger), "{text:?}");
        }
        assert_eq!(
            WrittenAs::Integer.parse("170141183460469231731687303715884105728"),
            Err(OutOfRange)
        );
        assert_eq!(WrittenAs::Integer.parse("2020-01-01"), Err(NotAnInteger));
        for text in [
            "",
            "-",
            "2019-02-29",
            "2020-13-01",
            "2020-01-00",
            "2020-1-01",
            "2020-01-01T12:00Z",
            "2020-01-01T24:00:00Z",
            "2020-01-01T12:00:61Z",
            "2020-01-01T12:00:00.Z",
            "2020-01-01T12:00:00+0100",
            "2020-01-01T12:00:00+24:00",
            "2020-01-01Z",
            "12:00:00Z",
        ] {
            assert_eq!(ts(text), Err(NotATimestamp), "{text:?}");
        }
    }
}
