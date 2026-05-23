// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use std::fmt;
use std::str::FromStr;

use super::{ApproxQualifier, IsmDate, ParseApproxQualifierError, ParseIsmDateError, UtcOffset};
use crate::date::calendar::{validate_date, validate_year_month};

impl fmt::Display for ParseIsmDateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid ISM date: {}", self.msg)
    }
}

impl std::error::Error for ParseIsmDateError {}

impl FromStr for IsmDate {
    type Err = ParseIsmDateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_ism_date(s)
    }
}

impl FromStr for UtcOffset {
    type Err = ParseIsmDateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Z" => Ok(UtcOffset::UTC),
            _ if (s.starts_with('+') || s.starts_with('-')) && s.len() == 6 => {
                let b = s.as_bytes();
                if b[3] != b':' {
                    return Err(ParseIsmDateError::new(
                        "UTC offset missing ':' separator (expected ±HH:MM)",
                    ));
                }
                let sign: i8 = if s.starts_with('+') { 1 } else { -1 };
                let oh =
                    parse_2digits(&b[1..3]).ok_or(ParseIsmDateError::new("invalid offset hour"))?;
                let om = parse_2digits(&b[4..6])
                    .ok_or(ParseIsmDateError::new("invalid offset minute"))?;
                UtcOffset::from_hhmm(sign, oh, om)
                    .ok_or(ParseIsmDateError::new("UTC offset out of range"))
            }
            _ => Err(ParseIsmDateError::new(
                "unrecognized UTC offset (expected Z or ±HH:MM)",
            )),
        }
    }
}

impl fmt::Display for ParseApproxQualifierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid approx qualifier")
    }
}

impl std::error::Error for ParseApproxQualifierError {}

impl FromStr for ApproxQualifier {
    type Err = ParseApproxQualifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1st qtr" => Ok(ApproxQualifier::FirstQtr),
            "2nd qtr" => Ok(ApproxQualifier::SecondQtr),
            "3rd qtr" => Ok(ApproxQualifier::ThirdQtr),
            "4th qtr" => Ok(ApproxQualifier::FourthQtr),
            "circa" => Ok(ApproxQualifier::Circa),
            "early" => Ok(ApproxQualifier::Early),
            "mid" => Ok(ApproxQualifier::Mid),
            "late" => Ok(ApproxQualifier::Late),
            _ => Err(ParseApproxQualifierError),
        }
    }
}

fn parse_ism_date(s: &str) -> Result<IsmDate, ParseIsmDateError> {
    let bytes = s.as_bytes();
    match bytes.len() {
        4 if all_ascii_digits(bytes) => {
            let y = parse_4digit_year(bytes)?;
            Ok(IsmDate::Year(y))
        }
        8 if all_ascii_digits(bytes) => {
            let y = parse_4digit_year(&bytes[0..4])?;
            let m = parse_2digits(&bytes[4..6])
                .ok_or(ParseIsmDateError::new("invalid month digits"))?;
            let d =
                parse_2digits(&bytes[6..8]).ok_or(ParseIsmDateError::new("invalid day digits"))?;
            validate_date(y, m, d)?;
            Ok(IsmDate::Date(y, m, d))
        }
        7 if bytes[4] == b'-' => {
            let y = parse_4digit_year(&bytes[0..4])?;
            let m = parse_2digits(&bytes[5..7])
                .ok_or(ParseIsmDateError::new("invalid month digits"))?;
            validate_year_month(y, m)?;
            Ok(IsmDate::YearMonth(y, m))
        }
        10 if bytes[4] == b'-' && bytes[7] == b'-' => {
            let y = parse_4digit_year(&bytes[0..4])?;
            let m = parse_2digits(&bytes[5..7])
                .ok_or(ParseIsmDateError::new("invalid month digits"))?;
            let d =
                parse_2digits(&bytes[8..10]).ok_or(ParseIsmDateError::new("invalid day digits"))?;
            validate_date(y, m, d)?;
            Ok(IsmDate::Date(y, m, d))
        }
        _ if bytes.len() >= 16
            && bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes[10] == b'T'
            && bytes[13] == b':' =>
        {
            parse_datetime_or_hourmin(s)
        }
        _ => Err(ParseIsmDateError::new("unrecognized date format")),
    }
}

fn parse_datetime_or_hourmin(s: &str) -> Result<IsmDate, ParseIsmDateError> {
    if !s.is_ascii() {
        return Err(ParseIsmDateError::new(
            "date string contains non-ASCII characters",
        ));
    }
    let bytes = s.as_bytes();

    let y = parse_4digit_year(&bytes[0..4])?;
    let m = parse_2digits(&bytes[5..7]).ok_or(ParseIsmDateError::new("invalid month digits"))?;
    let d = parse_2digits(&bytes[8..10]).ok_or(ParseIsmDateError::new("invalid day digits"))?;
    validate_date(y, m, d)?;

    let h = parse_2digits(&bytes[11..13]).ok_or(ParseIsmDateError::new("invalid hour digits"))?;
    let min =
        parse_2digits(&bytes[14..16]).ok_or(ParseIsmDateError::new("invalid minute digits"))?;
    if h > 23 {
        return Err(ParseIsmDateError::new("hour out of range"));
    }
    if min > 59 {
        return Err(ParseIsmDateError::new("minute out of range"));
    }

    let rest = &s[16..];

    if rest.is_empty() || rest.starts_with('Z') || rest.starts_with('+') || rest.starts_with('-') {
        let offset = parse_offset(rest)?;
        return Ok(IsmDate::DateHourMin {
            year: y,
            month: m,
            day: d,
            hour: h,
            minute: min,
            offset,
        });
    }

    if !rest.starts_with(':') || rest.len() < 3 {
        return Err(ParseIsmDateError::new("expected ':SS' in dateTime"));
    }
    let sec_bytes = &rest.as_bytes()[1..3];
    let sec = parse_2digits(sec_bytes).ok_or(ParseIsmDateError::new("invalid second digits"))?;
    if sec > 59 {
        return Err(ParseIsmDateError::new("second out of range"));
    }

    let after_sec = &rest[3..];

    let (nanosecond, after_frac) = if let Some(frac_str) = after_sec.strip_prefix('.') {
        let digit_end = frac_str.bytes().take_while(|b| b.is_ascii_digit()).count();
        if digit_end == 0 {
            return Err(ParseIsmDateError::new("empty fractional seconds"));
        }
        let frac_digits = &frac_str[..digit_end];
        let ns = parse_frac_as_nanoseconds(frac_digits)?;
        (ns, &frac_str[digit_end..])
    } else {
        (0u32, after_sec)
    };

    let offset = parse_offset(after_frac)?;

    Ok(IsmDate::DateTime {
        year: y,
        month: m,
        day: d,
        hour: h,
        minute: min,
        second: sec,
        nanosecond,
        offset,
    })
}

fn parse_offset(s: &str) -> Result<Option<UtcOffset>, ParseIsmDateError> {
    if s.is_empty() {
        return Ok(None);
    }
    s.parse::<UtcOffset>().map(Some)
}

fn parse_frac_as_nanoseconds(frac: &str) -> Result<u32, ParseIsmDateError> {
    if frac.len() > 9 {
        return Err(ParseIsmDateError::new(
            "fractional seconds: more than 9 digits",
        ));
    }
    let mut padded = [b'0'; 9];
    padded[..frac.len()].copy_from_slice(frac.as_bytes());
    let ns: u32 = std::str::from_utf8(&padded)
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or(ParseIsmDateError::new("fractional seconds not numeric"))?;
    Ok(ns)
}

fn parse_4digit_year(bytes: &[u8]) -> Result<i32, ParseIsmDateError> {
    if bytes.len() != 4 || !all_ascii_digits(bytes) {
        return Err(ParseIsmDateError::new("year must be exactly 4 digits"));
    }
    Ok(parse_digits_as_i32(bytes))
}

#[inline]
fn parse_2digits(bytes: &[u8]) -> Option<u8> {
    if bytes.len() == 2 && bytes[0].is_ascii_digit() && bytes[1].is_ascii_digit() {
        Some((bytes[0] - b'0') * 10 + (bytes[1] - b'0'))
    } else {
        None
    }
}

#[inline]
fn parse_digits_as_i32(bytes: &[u8]) -> i32 {
    bytes
        .iter()
        .fold(0i32, |acc, b| acc * 10 + (*b - b'0') as i32)
}

#[inline]
fn all_ascii_digits(bytes: &[u8]) -> bool {
    bytes.iter().all(|b| b.is_ascii_digit())
}
