// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! ISM date precision-tier types modeling the XSD `ISO8601DateTimeType` union.
//!
//! The ISM XSD defines declassification and other date attributes as a *union*
//! of ISO 8601 precision tiers (see `IC-ISM.xsd`):
//!
//! ```text
//! ISO8601DateTimeType = dateTime | dateHourMinType | date | gYearMonth | gYear
//! ```
//!
//! Each tier represents a temporal *span*, not a point:
//! - `xsd:gYear` "2003" spans the entire calendar year.
//! - `xsd:gYearMonth` "2003-04" spans the entire month of April 2003.
//! - `xsd:date` "2003-04-15" spans a single calendar day.
//! - `dateHourMinType` spans a single minute (HH:MM, no seconds).
//! - `xsd:dateTime` spans a single instant (seconds + fractional seconds).
//!
//! Use [`IsmDate::contains`] for span-containment checks. Total ordering
//! (`<`) is deliberately **not** implemented because the semantics are
//! ambiguous across precision tiers ("is `Year(2003)` less than
//! `Date(2003-04-15)`?"). For lattice-max operations (`MaxDate`), use
//! [`IsmDate::end_cmp`], which compares the end-of-span instants.
//!
//! # Schema observations
//!
//! - `dateHourMinType`'s doc text says "includes seconds/milliseconds"
//!   but the regex restricts to `HH:MM` only. The regex is authoritative.
//! - All zoned types support optional `Z` or `±HH:MM` offset.
//! - `xsd:gYear` / `xsd:gYearMonth` are date-only with no time component.
//! - [`ApproxQualifier`] is a companion axis from `DateApproximationVocabType`,
//!   not a member of the main union. Combine with an `IsmDate` in
//!   [`ApproxIsmDate`].
//!
//! # WASM safety
//!
//! All types are WASM-safe. The underlying `jiff` calendar arithmetic uses
//! `features = ["std"]` (no tzdb I/O) and `jiff::civil` types that work on
//! `wasm32-unknown-unknown`.

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use jiff::civil;

// ---------------------------------------------------------------------------
// UtcOffset
// ---------------------------------------------------------------------------

/// A UTC offset suitable for `DateHourMin` and `DateTime` precision tiers.
///
/// Stored as signed integer minutes from UTC in the range −1440..=+1440.
/// `None` in the parent type represents a *floating* (offset-naive) time.
///
/// # Examples
///
/// ```
/// use marque_ism::date::UtcOffset;
///
/// let eastern = UtcOffset::from_hhmm(-1, 5, 0).unwrap(); // -05:00
/// assert_eq!(eastern.to_string(), "-05:00");
///
/// assert_eq!(UtcOffset::UTC.to_string(), "Z");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UtcOffset {
    /// Signed offset in minutes: +05:30 → 330, −07:00 → −420.
    pub minutes: i16,
}

impl UtcOffset {
    /// UTC (zero offset).
    pub const UTC: Self = Self { minutes: 0 };

    /// Construct from a sign and hours/minutes.
    ///
    /// `sign` must be `1` or `-1`. Returns `None` if components are out of
    /// range (`hours > 23`, `minutes > 59`, or total magnitude > 1440).
    pub fn from_hhmm(sign: i8, hours: u8, minutes: u8) -> Option<Self> {
        if !matches!(sign, 1 | -1) || hours > 23 || minutes > 59 {
            return None;
        }
        let total = (hours as i16 * 60 + minutes as i16) * sign as i16;
        if total.abs() > 1440 {
            return None;
        }
        Some(Self { minutes: total })
    }

    /// Total offset in seconds (for jiff `tz::Offset::from_seconds`).
    pub fn to_seconds(self) -> i32 {
        self.minutes as i32 * 60
    }
}

impl fmt::Display for UtcOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.minutes == 0 {
            write!(f, "Z")
        } else {
            let sign = if self.minutes >= 0 { '+' } else { '-' };
            let abs = self.minutes.unsigned_abs();
            write!(f, "{}{:02}:{:02}", sign, abs / 60, abs % 60)
        }
    }
}

// ---------------------------------------------------------------------------
// IsmDate
// ---------------------------------------------------------------------------

/// ISM date precision-tier union, mirroring `ISO8601DateTimeType`.
///
/// Each variant represents the span for its precision tier:
///
/// | Variant | XSD type | Span |
/// |---------|----------|------|
/// | [`Year`] | `xsd:gYear` | entire calendar year |
/// | [`YearMonth`] | `xsd:gYearMonth` | entire calendar month |
/// | [`Date`] | `xsd:date` | single calendar day |
/// | [`DateHourMin`] | `dateHourMinType` | single minute (HH:MM, no seconds) |
/// | [`DateTime`] | `xsd:dateTime` | precise instant |
///
/// # Parsing
///
/// `IsmDate` implements [`FromStr`]. Accepted forms:
///
/// | Input | Variant |
/// |-------|---------|
/// | `YYYY` (e.g. `2003`) | `Year` |
/// | `YYYY-MM` (e.g. `2003-04`) | `YearMonth` |
/// | `YYYY-MM-DD` (e.g. `2003-04-15`) | `Date` |
/// | `YYYYMMDD` (CAPCO no-hyphen form, e.g. `20030415`) | `Date` |
/// | `YYYY-MM-DDTHH:MM`, optionally `Z` or `±HH:MM` | `DateHourMin` |
/// | `YYYY-MM-DDTHH:MM:SS[.frac][Z|±HH:MM]` | `DateTime` |
///
/// # Display
///
/// [`fmt::Display`] produces canonical ISO 8601 form (with hyphens and `T`
/// separator) suitable for round-trip: `IsmDate::from_str(&date.to_string())
/// == Ok(date)`.
///
/// [`Year`]: IsmDate::Year
/// [`YearMonth`]: IsmDate::YearMonth
/// [`Date`]: IsmDate::Date
/// [`DateHourMin`]: IsmDate::DateHourMin
/// [`DateTime`]: IsmDate::DateTime
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IsmDate {
    /// `xsd:gYear` — e.g. `"2003"`. Represents the span Jan 1 – Dec 31 of
    /// the given year (inclusive).
    Year(i32),

    /// `xsd:gYearMonth` — e.g. `"2003-04"`. Represents the entire month in
    /// the given year.
    YearMonth(i32, u8),

    /// `xsd:date` — e.g. `"2003-04-15"`. A single calendar day.
    ///
    /// Also accepts the CAPCO no-hyphen form `"YYYYMMDD"` on input;
    /// [`Display`] always produces `"YYYY-MM-DD"`.
    ///
    /// [`Display`]: fmt::Display
    Date(i32, u8, u8),

    /// `dateHourMinType` — e.g. `"2003-04-15T14:30Z"`.
    ///
    /// Date + hour + minute with optional UTC offset. The XSD regex restricts
    /// to HH:MM only (no seconds or fractional seconds); sub-minute precision
    /// is represented by [`DateTime`] instead.
    ///
    /// [`DateTime`]: IsmDate::DateTime
    DateHourMin {
        year: i32,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        /// `None` for floating (offset-naive) time.
        offset: Option<UtcOffset>,
    },

    /// `xsd:dateTime` — full ISO 8601 with seconds, optional fractional
    /// seconds, and optional UTC offset.
    DateTime {
        year: i32,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        /// Fractional seconds as nanoseconds (0..=999_999_999).
        nanosecond: u32,
        /// `None` for floating (offset-naive) time.
        offset: Option<UtcOffset>,
    },
}

impl IsmDate {
    // -----------------------------------------------------------------------
    // Component accessors
    // -----------------------------------------------------------------------

    /// The year component, always present.
    #[inline]
    pub fn year(&self) -> i32 {
        match self {
            IsmDate::Year(y) => *y,
            IsmDate::YearMonth(y, _) => *y,
            IsmDate::Date(y, _, _) => *y,
            IsmDate::DateHourMin { year, .. } => *year,
            IsmDate::DateTime { year, .. } => *year,
        }
    }

    /// Month component, if present (1–12).
    #[inline]
    pub fn month(&self) -> Option<u8> {
        match self {
            IsmDate::Year(_) => None,
            IsmDate::YearMonth(_, m) => Some(*m),
            IsmDate::Date(_, m, _) => Some(*m),
            IsmDate::DateHourMin { month, .. } => Some(*month),
            IsmDate::DateTime { month, .. } => Some(*month),
        }
    }

    /// Day component, if present (1–31).
    #[inline]
    pub fn day(&self) -> Option<u8> {
        match self {
            IsmDate::Year(_) | IsmDate::YearMonth(_, _) => None,
            IsmDate::Date(_, _, d) => Some(*d),
            IsmDate::DateHourMin { day, .. } => Some(*day),
            IsmDate::DateTime { day, .. } => Some(*day),
        }
    }

    // -----------------------------------------------------------------------
    // Span containment
    // -----------------------------------------------------------------------

    /// Returns `true` if `point` falls within the temporal span this date
    /// represents.
    ///
    /// Semantics: a coarser `IsmDate` (e.g. `Year(2003)`) represents a span
    /// (all of 2003). A finer one (e.g. `Date(2003, 6, 15)`) represents a
    /// narrower span. `self.contains(point)` is `true` iff the span of
    /// `point` is entirely within the span of `self`.
    ///
    /// `Year(2003)` contains:
    /// - `Year(2003)` ✓ (same span)
    /// - `YearMonth(2003, 4)` ✓ (April 2003 ⊂ 2003)
    /// - `Date(2003, 12, 31)` ✓
    /// - `Date(2004, 1, 1)` ✗
    ///
    /// `YearMonth(2003, 4)` contains:
    /// - `Year(2003)` ✗ (coarser than self)
    /// - `Date(2003, 4, 1)` ✓
    /// - `Date(2003, 5, 1)` ✗
    ///
    /// # Timezone handling
    ///
    /// Offsets are compared in their *represented* form, not after UTC
    /// normalisation. `DateHourMin { hour: 14, offset: UTC }` and
    /// `DateHourMin { hour: 9, offset: -05:00 }` are the same civil instant
    /// but `contains` does not normalise across offsets.
    pub fn contains(&self, point: &IsmDate) -> bool {
        // Year must always match.
        if self.year() != point.year() {
            return false;
        }
        match self {
            IsmDate::Year(_) => {
                // Any same-year date is contained.
                true
            }
            IsmDate::YearMonth(_, sm) => {
                // Point must have at least month precision and must be in the
                // same month. A Year-only point is coarser than self.
                match point {
                    IsmDate::Year(_) => false,
                    _ => point.month() == Some(*sm),
                }
            }
            IsmDate::Date(_, sm, sd) => {
                // Point must have at least day precision and must be on the
                // same day.
                match point {
                    IsmDate::Year(_) | IsmDate::YearMonth(_, _) => false,
                    _ => point.month() == Some(*sm) && point.day() == Some(*sd),
                }
            }
            IsmDate::DateHourMin {
                month: sm,
                day: sd,
                hour: sh,
                minute: smin,
                offset: soff,
                ..
            } => {
                // Point must be at sub-day precision and match all components.
                match point {
                    IsmDate::DateHourMin {
                        month,
                        day,
                        hour,
                        minute,
                        offset,
                        ..
                    } => {
                        month == sm
                            && day == sd
                            && hour == sh
                            && minute == smin
                            && offset == soff
                    }
                    IsmDate::DateTime {
                        month,
                        day,
                        hour,
                        minute,
                        offset,
                        ..
                    } => {
                        month == sm
                            && day == sd
                            && hour == sh
                            && minute == smin
                            && offset == soff
                    }
                    _ => false,
                }
            }
            IsmDate::DateTime {
                month: sm,
                day: sd,
                hour: sh,
                minute: smin,
                second: ss,
                nanosecond: sns,
                offset: soff,
                ..
            } => {
                if let IsmDate::DateTime {
                    month,
                    day,
                    hour,
                    minute,
                    second,
                    nanosecond,
                    offset,
                    ..
                } = point
                {
                    month == sm
                        && day == sd
                        && hour == sh
                        && minute == smin
                        && second == ss
                        && nanosecond == sns
                        && offset == soff
                } else {
                    false
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // End-of-span comparison (for MaxDate lattice semantics)
    // -----------------------------------------------------------------------

    /// Compare two `IsmDate` values by their **end-of-span** instants.
    ///
    /// This is the correct comparator for the `MaxDate` lattice operation:
    /// "which declassification date is the *latest* (most conservative)?"
    ///
    /// `Year(2003).end_cmp(Date(2003, 6, 15))` returns `Greater` because the
    /// year 2003 extends through December 31, whereas June 15 ends earlier.
    ///
    /// Coarser spans fill in the *maximum* value for unspecified components:
    /// - `Year(y)` end = (y, 12, 31, 23, 59, 59, 999_999_999)
    /// - `YearMonth(y, m)` end = (y, m, last-day-of-month, 23, 59, 59, 999_999_999)
    /// - `Date(y, m, d)` end = (y, m, d, 23, 59, 59, 999_999_999)
    /// - `DateHourMin` end = (y, m, d, H, M, 59, 999_999_999)
    /// - `DateTime` end = the precise instant
    ///
    /// UTC offsets are **not** normalised; comparison is on the civil
    /// end-of-span components.
    pub fn end_cmp(&self, other: &IsmDate) -> Ordering {
        let a = self.end_components();
        let b = other.end_components();
        a.cmp(&b)
    }

    /// Returns the end-of-span as a sortable `YYYYMMDD` string for use as a
    /// [`crate::MaxDate`][marque-scheme `MaxDate`] key.
    ///
    /// The string is always 8 ASCII digits. Lex order on these strings is
    /// chronological, so `MaxDate`'s lex join produces the correct
    /// span-aware "latest" date.
    ///
    /// - `Year(y)` → `"{y:04}1231"` (December 31 of year)
    /// - `YearMonth(y, m)` → last day of month
    /// - `Date(y, m, d)` → `"{y:04}{m:02}{d:02}"`
    /// - `DateHourMin / DateTime` → the date component only
    pub fn to_maxdate_str(&self) -> Box<str> {
        let (y, m, d, _, _, _, _) = self.end_components();
        format!("{:04}{:02}{:02}", y, m, d).into_boxed_str()
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// End-of-span tuple `(year, month, day, hour, minute, second, nanosecond)`.
    ///
    /// Unspecified components are filled with their maximum values so that
    /// `end_cmp` correctly orders coarser dates AFTER finer ones that fall
    /// within the same span.
    fn end_components(&self) -> (i32, u8, u8, u8, u8, u8, u32) {
        match self {
            IsmDate::Year(y) => (*y, 12, 31, 23, 59, 59, 999_999_999),
            IsmDate::YearMonth(y, m) => {
                let d = days_in_month(*y, *m);
                (*y, *m, d, 23, 59, 59, 999_999_999)
            }
            IsmDate::Date(y, m, d) => (*y, *m, *d, 23, 59, 59, 999_999_999),
            IsmDate::DateHourMin {
                year,
                month,
                day,
                hour,
                minute,
                ..
            } => (*year, *month, *day, *hour, *minute, 59, 999_999_999),
            IsmDate::DateTime {
                year,
                month,
                day,
                hour,
                minute,
                second,
                nanosecond,
                ..
            } => (*year, *month, *day, *hour, *minute, *second, *nanosecond),
        }
    }
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

impl fmt::Display for IsmDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IsmDate::Year(y) => write!(f, "{:04}", y),
            IsmDate::YearMonth(y, m) => write!(f, "{:04}-{:02}", y, m),
            IsmDate::Date(y, m, d) => write!(f, "{:04}-{:02}-{:02}", y, m, d),
            IsmDate::DateHourMin {
                year,
                month,
                day,
                hour,
                minute,
                offset,
            } => {
                write!(
                    f,
                    "{:04}-{:02}-{:02}T{:02}:{:02}",
                    year, month, day, hour, minute
                )?;
                if let Some(o) = offset {
                    write!(f, "{o}")?;
                }
                Ok(())
            }
            IsmDate::DateTime {
                year,
                month,
                day,
                hour,
                minute,
                second,
                nanosecond,
                offset,
            } => {
                write!(
                    f,
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
                    year, month, day, hour, minute, second
                )?;
                if *nanosecond > 0 {
                    // Emit the shortest unambiguous fractional form.
                    let s = format!("{:09}", nanosecond);
                    let trimmed = s.trim_end_matches('0');
                    write!(f, ".{trimmed}")?;
                }
                if let Some(o) = offset {
                    write!(f, "{o}")?;
                }
                Ok(())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// FromStr
// ---------------------------------------------------------------------------

/// Parse error for [`IsmDate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseIsmDateError {
    msg: &'static str,
}

impl fmt::Display for ParseIsmDateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid ISM date: {}", self.msg)
    }
}

impl std::error::Error for ParseIsmDateError {}

impl ParseIsmDateError {
    const fn new(msg: &'static str) -> Self {
        Self { msg }
    }
}

impl FromStr for IsmDate {
    type Err = ParseIsmDateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_ism_date(s)
    }
}

// ---------------------------------------------------------------------------
// Internal parsing helpers
// ---------------------------------------------------------------------------

/// Parse `s` into an [`IsmDate`], accepting all XSD forms plus the CAPCO
/// no-hyphen `YYYYMMDD` form.
fn parse_ism_date(s: &str) -> Result<IsmDate, ParseIsmDateError> {
    let bytes = s.as_bytes();
    match bytes.len() {
        // xsd:gYear — "YYYY"
        4 if all_ascii_digits(bytes) => {
            let y = parse_4digit_year(bytes)?;
            Ok(IsmDate::Year(y))
        }
        // CAPCO no-hyphen date — "YYYYMMDD"
        8 if all_ascii_digits(bytes) => {
            let y = parse_4digit_year(&bytes[0..4])?;
            let m = parse_2digits(&bytes[4..6])
                .ok_or(ParseIsmDateError::new("invalid month digits"))?;
            let d = parse_2digits(&bytes[6..8])
                .ok_or(ParseIsmDateError::new("invalid day digits"))?;
            validate_date(y, m, d)?;
            Ok(IsmDate::Date(y, m, d))
        }
        // xsd:gYearMonth — "YYYY-MM"
        7 if bytes[4] == b'-' => {
            let y = parse_4digit_year(&bytes[0..4])?;
            let m = parse_2digits(&bytes[5..7])
                .ok_or(ParseIsmDateError::new("invalid month digits"))?;
            validate_year_month(y, m)?;
            Ok(IsmDate::YearMonth(y, m))
        }
        // xsd:date — "YYYY-MM-DD"
        10 if bytes[4] == b'-' && bytes[7] == b'-' => {
            let y = parse_4digit_year(&bytes[0..4])?;
            let m = parse_2digits(&bytes[5..7])
                .ok_or(ParseIsmDateError::new("invalid month digits"))?;
            let d = parse_2digits(&bytes[8..10])
                .ok_or(ParseIsmDateError::new("invalid day digits"))?;
            validate_date(y, m, d)?;
            Ok(IsmDate::Date(y, m, d))
        }
        // dateHourMinType or xsd:dateTime — "YYYY-MM-DDTHH:..."
        _ if bytes.len() >= 16
            && bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes[10] == b'T'
            && bytes[13] == b':' =>
        {
            parse_datetime_or_hourmind(s)
        }
        _ => Err(ParseIsmDateError::new("unrecognised date format")),
    }
}

/// Dispatch between `DateHourMin` and `DateTime` once the date portion has
/// been identified.
fn parse_datetime_or_hourmind(s: &str) -> Result<IsmDate, ParseIsmDateError> {
    let bytes = s.as_bytes();

    let y = parse_4digit_year(&bytes[0..4])?;
    let m = parse_2digits(&bytes[5..7])
        .ok_or(ParseIsmDateError::new("invalid month digits"))?;
    let d = parse_2digits(&bytes[8..10])
        .ok_or(ParseIsmDateError::new("invalid day digits"))?;
    validate_date(y, m, d)?;

    let h = parse_2digits(&bytes[11..13])
        .ok_or(ParseIsmDateError::new("invalid hour digits"))?;
    let min = parse_2digits(&bytes[14..16])
        .ok_or(ParseIsmDateError::new("invalid minute digits"))?;
    if h > 23 {
        return Err(ParseIsmDateError::new("hour out of range"));
    }
    if min > 59 {
        return Err(ParseIsmDateError::new("minute out of range"));
    }

    let rest = &s[16..];

    // dateHourMinType: pattern ends here (no `:SS` part), followed by
    // optional offset or end-of-string.
    if rest.is_empty() || rest.starts_with('Z') || rest.starts_with('+') || rest.starts_with('-')
    {
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

    // xsd:dateTime: expect `:SS` next.
    if !rest.starts_with(':') || rest.len() < 3 {
        return Err(ParseIsmDateError::new("expected ':SS' in dateTime"));
    }
    let sec_bytes = rest[1..3].as_bytes();
    let sec = parse_2digits(sec_bytes).ok_or(ParseIsmDateError::new("invalid second digits"))?;
    if sec > 59 {
        return Err(ParseIsmDateError::new("second out of range"));
    }

    let after_sec = &rest[3..];

    // Optional fractional seconds: ".ddd..."
    let (nanosecond, after_frac) = if after_sec.starts_with('.') {
        let frac_str = &after_sec[1..];
        // Collect consecutive digit characters.
        let digit_end = frac_str
            .bytes()
            .take_while(|b| b.is_ascii_digit())
            .count();
        if digit_end == 0 {
            return Err(ParseIsmDateError::new("empty fractional seconds"));
        }
        let frac_digits = &frac_str[..digit_end];
        // Normalise to 9 digits (nanosecond precision).
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

/// Parse an optional timezone suffix (`""`, `"Z"`, `"+HH:MM"`, `"-HH:MM"`).
/// Returns `Ok(None)` for an empty string (floating / offset-naive).
fn parse_offset(s: &str) -> Result<Option<UtcOffset>, ParseIsmDateError> {
    match s {
        "" => Ok(None),
        "Z" => Ok(Some(UtcOffset::UTC)),
        _ if (s.starts_with('+') || s.starts_with('-')) && s.len() == 6 => {
            let sign: i8 = if s.starts_with('+') { 1 } else { -1 };
            let oh = parse_2digits(s[1..3].as_bytes())
                .ok_or(ParseIsmDateError::new("invalid offset hour"))?;
            let om = parse_2digits(s[4..6].as_bytes())
                .ok_or(ParseIsmDateError::new("invalid offset minute"))?;
            UtcOffset::from_hhmm(sign, oh, om)
                .ok_or(ParseIsmDateError::new("UTC offset out of range"))
                .map(Some)
        }
        _ => Err(ParseIsmDateError::new("unrecognised timezone suffix")),
    }
}

/// Convert up to 9 fractional-second digits to nanoseconds.
fn parse_frac_as_nanoseconds(frac: &str) -> Result<u32, ParseIsmDateError> {
    if frac.len() > 9 {
        return Err(ParseIsmDateError::new(
            "fractional seconds: more than 9 digits",
        ));
    }
    // Left-align: pad with trailing zeros to 9 digits.
    let mut padded = [b'0'; 9];
    padded[..frac.len()].copy_from_slice(frac.as_bytes());
    let ns: u32 = std::str::from_utf8(&padded)
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or(ParseIsmDateError::new("fractional seconds not numeric"))?;
    Ok(ns)
}

// ---------------------------------------------------------------------------
// Validation helpers (use jiff for date validity)
// ---------------------------------------------------------------------------

/// Validate a complete year/month/day triple using jiff's civil::Date.
fn validate_date(year: i32, month: u8, day: u8) -> Result<(), ParseIsmDateError> {
    let y = i16::try_from(year).map_err(|_| ParseIsmDateError::new("year out of i16 range"))?;
    civil::Date::new(y, month as i8, day as i8)
        .map_err(|_| ParseIsmDateError::new("invalid calendar date"))?;
    Ok(())
}

/// Validate year/month (for `YearMonth` variant).
fn validate_year_month(year: i32, month: u8) -> Result<(), ParseIsmDateError> {
    if !(1..=12).contains(&month) {
        return Err(ParseIsmDateError::new("month out of range 1–12"));
    }
    let _y = i16::try_from(year).map_err(|_| ParseIsmDateError::new("year out of i16 range"))?;
    Ok(())
}

/// Number of days in the given month, using jiff for leap-year correctness.
fn days_in_month(year: i32, month: u8) -> u8 {
    let y = i16::try_from(year).unwrap_or(2000); // fallback for hypothetical overflow
    civil::Date::new(y, month as i8, 1)
        .map(|d| d.days_in_month() as u8)
        .unwrap_or(30) // fallback — should not happen for valid inputs
}

// ---------------------------------------------------------------------------
// Low-level byte-parsing utilities
// ---------------------------------------------------------------------------

/// Parse exactly 4 ASCII decimal digits as a signed year.
fn parse_4digit_year(bytes: &[u8]) -> Result<i32, ParseIsmDateError> {
    if bytes.len() != 4 || !all_ascii_digits(bytes) {
        return Err(ParseIsmDateError::new("year must be exactly 4 digits"));
    }
    Ok(parse_digits_as_i32(bytes))
}

/// Parse exactly 2 ASCII decimal digits as a `u8`. Returns `None` if the
/// bytes are not exactly two ASCII digits.
#[inline]
fn parse_2digits(bytes: &[u8]) -> Option<u8> {
    if bytes.len() == 2 && bytes[0].is_ascii_digit() && bytes[1].is_ascii_digit() {
        Some((bytes[0] - b'0') * 10 + (bytes[1] - b'0'))
    } else {
        None
    }
}

/// Convert a slice of ASCII digits to `i32` (no overflow check — callers
/// limit input length).
#[inline]
fn parse_digits_as_i32(bytes: &[u8]) -> i32 {
    bytes
        .iter()
        .fold(0i32, |acc, b| acc * 10 + (*b - b'0') as i32)
}

/// Returns `true` iff every byte in `bytes` is an ASCII decimal digit.
#[inline]
fn all_ascii_digits(bytes: &[u8]) -> bool {
    bytes.iter().all(|b| b.is_ascii_digit())
}

// ---------------------------------------------------------------------------
// ApproxQualifier
// ---------------------------------------------------------------------------

/// Approximation qualifier from `DateApproximationVocabType`.
///
/// Paired with an [`IsmDate`] in [`ApproxIsmDate`] to express constructions
/// like "circa 1995" or "early 2003". The qualifier is informational and does
/// not affect [`IsmDate::contains`] or [`IsmDate::end_cmp`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApproxQualifier {
    /// `"1st qtr"` — first quarter of the year.
    FirstQtr,
    /// `"2nd qtr"` — second quarter.
    SecondQtr,
    /// `"3rd qtr"` — third quarter.
    ThirdQtr,
    /// `"4th qtr"` — fourth quarter.
    FourthQtr,
    /// `"circa"` — approximately.
    Circa,
    /// `"early"` — early portion of the period.
    Early,
    /// `"mid"` — middle portion of the period.
    Mid,
    /// `"late"` — late portion of the period.
    Late,
}

impl fmt::Display for ApproxQualifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ApproxQualifier::FirstQtr => "1st qtr",
            ApproxQualifier::SecondQtr => "2nd qtr",
            ApproxQualifier::ThirdQtr => "3rd qtr",
            ApproxQualifier::FourthQtr => "4th qtr",
            ApproxQualifier::Circa => "circa",
            ApproxQualifier::Early => "early",
            ApproxQualifier::Mid => "mid",
            ApproxQualifier::Late => "late",
        };
        f.write_str(s)
    }
}

/// Parse error for [`ApproxQualifier`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseApproxQualifierError;

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

// ---------------------------------------------------------------------------
// ApproxIsmDate
// ---------------------------------------------------------------------------

/// An [`IsmDate`] paired with an optional [`ApproxQualifier`].
///
/// Models the `DateApproximationVocabType` companion axis. Example: "circa
/// 1995" is `ApproxIsmDate { date: IsmDate::Year(1995), qualifier:
/// Some(ApproxQualifier::Circa) }`.
///
/// The qualifier is preserved for round-trip and display but does not affect
/// span containment or ordering semantics.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ApproxIsmDate {
    pub date: IsmDate,
    pub qualifier: Option<ApproxQualifier>,
}

impl fmt::Display for ApproxIsmDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(q) = self.qualifier {
            write!(f, "{} {}", q, self.date)
        } else {
            write!(f, "{}", self.date)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // -----------------------------------------------------------------------
    // Round-trip: Display → FromStr
    // -----------------------------------------------------------------------

    fn round_trip(s: &str) -> bool {
        IsmDate::from_str(s)
            .map(|d| d.to_string() == s)
            .unwrap_or(false)
    }

    #[test]
    fn round_trip_year() {
        assert!(round_trip("2003"));
        assert!(round_trip("1900"));
        assert!(round_trip("9999"));
    }

    #[test]
    fn round_trip_year_month() {
        assert!(round_trip("2003-04"));
        assert!(round_trip("2003-12"));
        assert!(round_trip("2003-01"));
    }

    #[test]
    fn round_trip_date() {
        assert!(round_trip("2003-04-15"));
        assert!(round_trip("2000-02-29")); // leap year
    }

    #[test]
    fn round_trip_date_hour_min_utc() {
        assert!(round_trip("2003-04-15T14:30Z"));
    }

    #[test]
    fn round_trip_date_hour_min_offset() {
        assert!(round_trip("2003-04-15T14:30-05:00"));
        assert!(round_trip("2003-04-15T14:30+05:30"));
    }

    #[test]
    fn round_trip_date_hour_min_floating() {
        assert!(round_trip("2003-04-15T14:30"));
    }

    #[test]
    fn round_trip_datetime_utc() {
        assert!(round_trip("2003-04-15T14:30:00Z"));
    }

    #[test]
    fn round_trip_datetime_with_millis() {
        assert!(round_trip("2003-04-15T14:30:00.123Z"));
    }

    #[test]
    fn round_trip_datetime_with_micros() {
        assert!(round_trip("2003-04-15T14:30:00.123456Z"));
    }

    #[test]
    fn round_trip_datetime_floating() {
        assert!(round_trip("2003-04-15T14:30:00"));
    }

    // -----------------------------------------------------------------------
    // CAPCO no-hyphen YYYYMMDD input
    // -----------------------------------------------------------------------

    #[test]
    fn capco_yyyymmdd_parses_to_date() {
        let d = IsmDate::from_str("20030415").unwrap();
        assert_eq!(d, IsmDate::Date(2003, 4, 15));
    }

    #[test]
    fn capco_year_only_parses_to_year() {
        let d = IsmDate::from_str("2035").unwrap();
        assert_eq!(d, IsmDate::Year(2035));
    }

    #[test]
    fn capco_display_uses_iso_form() {
        let d = IsmDate::from_str("20030415").unwrap();
        assert_eq!(d.to_string(), "2003-04-15");
    }

    // -----------------------------------------------------------------------
    // Validation rejects invalid dates
    // -----------------------------------------------------------------------

    #[test]
    fn rejects_invalid_month() {
        assert!(IsmDate::from_str("2003-13").is_err());
        assert!(IsmDate::from_str("2003-00").is_err());
    }

    #[test]
    fn rejects_invalid_day() {
        assert!(IsmDate::from_str("2003-02-29").is_err()); // 2003 not leap year
        assert!(IsmDate::from_str("2003-04-31").is_err()); // April has 30 days
    }

    #[test]
    fn accepts_leap_day_in_leap_year() {
        assert!(IsmDate::from_str("2000-02-29").is_ok()); // 2000 is leap
        assert!(IsmDate::from_str("2004-02-29").is_ok()); // 2004 is leap
    }

    // -----------------------------------------------------------------------
    // IsmDate::contains
    // -----------------------------------------------------------------------

    #[test]
    fn year_contains_same_year() {
        let y = IsmDate::Year(2003);
        assert!(y.contains(&IsmDate::Year(2003)));
    }

    #[test]
    fn year_contains_year_month() {
        let y = IsmDate::Year(2003);
        assert!(y.contains(&IsmDate::YearMonth(2003, 4)));
        assert!(!y.contains(&IsmDate::YearMonth(2004, 1)));
    }

    #[test]
    fn year_contains_date() {
        let y = IsmDate::Year(2003);
        assert!(y.contains(&IsmDate::Date(2003, 12, 31)));
        assert!(!y.contains(&IsmDate::Date(2004, 1, 1)));
    }

    #[test]
    fn year_month_does_not_contain_year() {
        let ym = IsmDate::YearMonth(2003, 4);
        assert!(!ym.contains(&IsmDate::Year(2003)));
    }

    #[test]
    fn year_month_contains_same_month_date() {
        let ym = IsmDate::YearMonth(2003, 4);
        assert!(ym.contains(&IsmDate::Date(2003, 4, 1)));
        assert!(ym.contains(&IsmDate::Date(2003, 4, 30)));
        assert!(!ym.contains(&IsmDate::Date(2003, 5, 1)));
    }

    #[test]
    fn date_does_not_contain_coarser() {
        let d = IsmDate::Date(2003, 4, 15);
        assert!(!d.contains(&IsmDate::Year(2003)));
        assert!(!d.contains(&IsmDate::YearMonth(2003, 4)));
    }

    #[test]
    fn date_contains_self() {
        let d = IsmDate::Date(2003, 4, 15);
        assert!(d.contains(&IsmDate::Date(2003, 4, 15)));
    }

    #[test]
    fn date_contains_hour_min_on_same_day() {
        let d = IsmDate::Date(2003, 4, 15);
        assert!(d.contains(&IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 14,
            minute: 30,
            offset: None,
        }));
    }

    #[test]
    fn date_does_not_contain_hour_min_different_day() {
        let d = IsmDate::Date(2003, 4, 15);
        assert!(!d.contains(&IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 16,
            hour: 0,
            minute: 0,
            offset: None,
        }));
    }

    // -----------------------------------------------------------------------
    // end_cmp / to_maxdate_str
    // -----------------------------------------------------------------------

    #[test]
    fn year_end_cmp_is_greater_than_mid_year_date() {
        let year = IsmDate::Year(2003);
        let mid = IsmDate::Date(2003, 6, 15);
        // Year(2003) ends on Dec 31; Date(2003,6,15) ends on Jun 15.
        assert_eq!(year.end_cmp(&mid), Ordering::Greater);
    }

    #[test]
    fn year_month_end_cmp_greater_than_early_date_in_month() {
        let ym = IsmDate::YearMonth(2003, 4); // ends Apr 30
        let d = IsmDate::Date(2003, 4, 1); // ends Apr 1
        assert_eq!(ym.end_cmp(&d), Ordering::Greater);
    }

    #[test]
    fn to_maxdate_str_year() {
        assert_eq!(&*IsmDate::Year(2003).to_maxdate_str(), "20031231");
    }

    #[test]
    fn to_maxdate_str_year_month_april() {
        assert_eq!(&*IsmDate::YearMonth(2003, 4).to_maxdate_str(), "20030430");
    }

    #[test]
    fn to_maxdate_str_year_month_february_non_leap() {
        assert_eq!(&*IsmDate::YearMonth(2003, 2).to_maxdate_str(), "20030228");
    }

    #[test]
    fn to_maxdate_str_year_month_february_leap() {
        assert_eq!(&*IsmDate::YearMonth(2000, 2).to_maxdate_str(), "20000229");
    }

    #[test]
    fn to_maxdate_str_date() {
        assert_eq!(
            &*IsmDate::Date(2003, 4, 15).to_maxdate_str(),
            "20030415"
        );
    }

    // -----------------------------------------------------------------------
    // ApproxQualifier round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn approx_qualifier_round_trip() {
        for q in [
            ApproxQualifier::FirstQtr,
            ApproxQualifier::SecondQtr,
            ApproxQualifier::ThirdQtr,
            ApproxQualifier::FourthQtr,
            ApproxQualifier::Circa,
            ApproxQualifier::Early,
            ApproxQualifier::Mid,
            ApproxQualifier::Late,
        ] {
            let s = q.to_string();
            assert_eq!(ApproxQualifier::from_str(&s).unwrap(), q);
        }
    }

    // -----------------------------------------------------------------------
    // UtcOffset
    // -----------------------------------------------------------------------

    #[test]
    fn utc_offset_display_utc() {
        assert_eq!(UtcOffset::UTC.to_string(), "Z");
    }

    #[test]
    fn utc_offset_display_positive() {
        let o = UtcOffset::from_hhmm(1, 5, 30).unwrap();
        assert_eq!(o.to_string(), "+05:30");
    }

    #[test]
    fn utc_offset_display_negative() {
        let o = UtcOffset::from_hhmm(-1, 5, 0).unwrap();
        assert_eq!(o.to_string(), "-05:00");
    }

    #[test]
    fn utc_offset_rejects_invalid() {
        assert!(UtcOffset::from_hhmm(1, 24, 0).is_none()); // hours > 23
        assert!(UtcOffset::from_hhmm(1, 0, 60).is_none()); // minutes > 59
    }
}
