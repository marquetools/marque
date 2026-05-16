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
//! All types are WASM-safe. Calendar arithmetic flows through one of two
//! interchangeable backends selected at compile time by the `dates`
//! feature on `marque-ism`:
//!
//! - **`dates` on** (native CLI / server / extract): `jiff::civil::Date`
//!   for validity + `days_in_month`. Built with `features = ["std"]`
//!   (no tzdb I/O), works on `wasm32-unknown-unknown` but excluded from
//!   the WASM artifact for bundle-size reasons (issue #455).
//! - **`dates` off** (WASM artifact): a hand-rolled proleptic Gregorian
//!   implementation lives in this file. Same `validate_date` +
//!   `days_in_month` surface; SC-008 native↔WASM parity tests enforce
//!   output agreement.

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

#[cfg(feature = "dates")]
use jiff::civil;

// ---------------------------------------------------------------------------
// UtcOffset
// ---------------------------------------------------------------------------

/// A UTC offset suitable for `DateHourMin` and `DateTime` precision tiers.
///
/// Stored as signed integer minutes from UTC in the range −1439..=+1439
/// (i.e. ±23:59). The maximum representable offset is ±23:59; offsets of
/// ±24:00 or larger are rejected by [`UtcOffset::from_hhmm`].
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
    /// range (`hours > 23`, `minutes > 59`). The maximum representable
    /// offset magnitude is 23:59 (1439 minutes).
    pub fn from_hhmm(sign: i8, hours: u8, minutes: u8) -> Option<Self> {
        if !matches!(sign, 1 | -1) || hours > 23 || minutes > 59 {
            return None;
        }
        let total = (hours as i16 * 60 + minutes as i16) * sign as i16;
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
    /// normalization. `DateHourMin { hour: 14, offset: UTC }` and
    /// `DateHourMin { hour: 9, offset: -05:00 }` are the same civil instant
    /// but `contains` does not normalize across offsets.
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
                    } => month == sm && day == sd && hour == sh && minute == smin && offset == soff,
                    IsmDate::DateTime {
                        month,
                        day,
                        hour,
                        minute,
                        offset,
                        ..
                    } => month == sm && day == sd && hour == sh && minute == smin && offset == soff,
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
    /// When civil end-of-span components are equal, the value with the more
    /// negative UTC offset (i.e. further behind UTC, representing a later UTC
    /// instant) is considered Greater. For example, `2003-04-15T10:30-05:00`
    /// (= 15:30 UTC) compares Greater than `2003-04-15T10:30Z` (= 10:30 UTC).
    /// Floating (offset-naive) values treat offset as zero for tie-breaking.
    pub fn end_cmp(&self, other: &IsmDate) -> Ordering {
        let a = self.end_components();
        let b = other.end_components();
        a.cmp(&b)
    }

    /// Returns the end-of-span as a sortable `YYYYMMDD` string for use as a
    /// MaxDate lattice key.
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
        let (y, m, d, _, _, _, _, _) = self.end_components();
        format!("{:04}{:02}{:02}", y, m, d).into_boxed_str()
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// End-of-span tuple `(year, month, day, hour, minute, second, nanosecond, utc_tie_break)`.
    ///
    /// Unspecified components are filled with their maximum values so that
    /// `end_cmp` correctly orders coarser dates AFTER finer ones that fall
    /// within the same span.
    ///
    /// The last element (`utc_tie_break`) is `-offset.minutes` (negated).
    /// When all civil components are equal, a more negative UTC offset means
    /// a later UTC instant; negating makes "later UTC" → larger value → Greater.
    /// Floating (offset-naive) values use 0 as the tie-breaker.
    fn end_components(&self) -> (i32, u8, u8, u8, u8, u8, u32, i16) {
        match self {
            IsmDate::Year(y) => (*y, 12, 31, 23, 59, 59, 999_999_999, 0),
            IsmDate::YearMonth(y, m) => {
                let d = days_in_month(*y, *m);
                (*y, *m, d, 23, 59, 59, 999_999_999, 0)
            }
            IsmDate::Date(y, m, d) => (*y, *m, *d, 23, 59, 59, 999_999_999, 0),
            IsmDate::DateHourMin {
                year,
                month,
                day,
                hour,
                minute,
                offset,
            } => {
                // Negate offset.minutes: a more-negative offset = later UTC instant = Greater.
                let utc_tb = offset.map_or(0_i16, |o| -o.minutes);
                (*year, *month, *day, *hour, *minute, 59, 999_999_999, utc_tb)
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
                let utc_tb = offset.map_or(0_i16, |o| -o.minutes);
                (
                    *year,
                    *month,
                    *day,
                    *hour,
                    *minute,
                    *second,
                    *nanosecond,
                    utc_tb,
                )
            }
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

impl FromStr for UtcOffset {
    type Err = ParseIsmDateError;

    /// Parse a standalone ISO 8601 UTC offset string.
    ///
    /// Accepted forms:
    /// - `"Z"` → UTC (zero offset)
    /// - `"+HH:MM"` → positive offset (e.g. `"+05:30"`)
    /// - `"-HH:MM"` → negative offset (e.g. `"-05:00"`)
    ///
    /// Returns `Err` for any other form (e.g. `"EST"`, `"UTC"`, `"+0530"`).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Z" => Ok(UtcOffset::UTC),
            _ if (s.starts_with('+') || s.starts_with('-')) && s.len() == 6 => {
                let b = s.as_bytes();
                // Require `:` separator at index 3 — rejects `+05-30` and `+0530`.
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
            let d =
                parse_2digits(&bytes[6..8]).ok_or(ParseIsmDateError::new("invalid day digits"))?;
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
            let d =
                parse_2digits(&bytes[8..10]).ok_or(ParseIsmDateError::new("invalid day digits"))?;
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
        _ => Err(ParseIsmDateError::new("unrecognized date format")),
    }
}

/// Dispatch between `DateHourMin` and `DateTime` once the date portion has
/// been identified.
fn parse_datetime_or_hourmind(s: &str) -> Result<IsmDate, ParseIsmDateError> {
    // All ISM date strings are pure ASCII. Reject multi-byte UTF-8 up front
    // so every subsequent fixed byte-offset slice is panic-safe.
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

    // dateHourMinType: pattern ends here (no `:SS` part), followed by
    // optional offset or end-of-string.
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

    // xsd:dateTime: expect `:SS` next.
    if !rest.starts_with(':') || rest.len() < 3 {
        return Err(ParseIsmDateError::new("expected ':SS' in dateTime"));
    }
    let sec_bytes = &rest.as_bytes()[1..3];
    let sec = parse_2digits(sec_bytes).ok_or(ParseIsmDateError::new("invalid second digits"))?;
    if sec > 59 {
        return Err(ParseIsmDateError::new("second out of range"));
    }

    let after_sec = &rest[3..];

    // Optional fractional seconds: ".ddd..."
    let (nanosecond, after_frac) = if let Some(frac_str) = after_sec.strip_prefix('.') {
        // Collect consecutive digit characters.
        let digit_end = frac_str.bytes().take_while(|b| b.is_ascii_digit()).count();
        if digit_end == 0 {
            return Err(ParseIsmDateError::new("empty fractional seconds"));
        }
        let frac_digits = &frac_str[..digit_end];
        // Normalize to 9 digits (nanosecond precision).
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
            let b = s.as_bytes();
            // Require the `:` separator at index 3 explicitly so that inputs
            // like `"+05-30"` (wrong separator) or `"+0530"` (missing one)
            // are rejected rather than accidentally parsed.
            if b[3] != b':' {
                return Err(ParseIsmDateError::new(
                    "UTC offset missing ':' separator (expected ±HH:MM)",
                ));
            }
            let sign: i8 = if s.starts_with('+') { 1 } else { -1 };
            let oh =
                parse_2digits(&b[1..3]).ok_or(ParseIsmDateError::new("invalid offset hour"))?;
            let om =
                parse_2digits(&b[4..6]).ok_or(ParseIsmDateError::new("invalid offset minute"))?;
            UtcOffset::from_hhmm(sign, oh, om)
                .ok_or(ParseIsmDateError::new("UTC offset out of range"))
                .map(Some)
        }
        _ => Err(ParseIsmDateError::new("unrecognized timezone suffix")),
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
// Validation helpers
// ---------------------------------------------------------------------------
//
// Two interchangeable backends — the `dates` feature flips between them
// at compile time. CLI / server / extract pull `dates` on (jiff path);
// WASM excludes jiff and uses the hand-rolled Gregorian path. SC-008
// native↔WASM parity tests enforce identical observable behavior.

/// Validate a complete year/month/day triple.
#[cfg(feature = "dates")]
fn validate_date(year: i32, month: u8, day: u8) -> Result<(), ParseIsmDateError> {
    let y = i16::try_from(year).map_err(|_| ParseIsmDateError::new("year out of i16 range"))?;
    civil::Date::new(y, month as i8, day as i8)
        .map_err(|_| ParseIsmDateError::new("invalid calendar date"))?;
    Ok(())
}

/// Validate a complete year/month/day triple (hand-rolled proleptic Gregorian).
///
/// Mirrors `jiff::civil::Date::new`'s validity check. Accepts the full
/// `i16` year range (`i16::MIN..=i16::MAX`, i.e. `-32768..=32767`) since
/// `jiff::civil::Date::new` takes `i16` for the year; rejects months
/// outside `1..=12` and days outside `1..=days_in_month(year, month)`.
/// In practice the upstream parser (`parse_4digit_year`) constrains
/// inputs to the `0..=9999` ASCII-digit range, so the wider i16 window
/// only matters for programmatic `IsmDate` construction.
#[cfg(not(feature = "dates"))]
fn validate_date(year: i32, month: u8, day: u8) -> Result<(), ParseIsmDateError> {
    i16::try_from(year).map_err(|_| ParseIsmDateError::new("year out of i16 range"))?;
    if !(1..=12).contains(&month) {
        return Err(ParseIsmDateError::new("invalid calendar date"));
    }
    let max_day = days_in_month(year, month);
    if day < 1 || day > max_day {
        return Err(ParseIsmDateError::new("invalid calendar date"));
    }
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

/// Number of days in the given month (jiff path).
#[cfg(feature = "dates")]
fn days_in_month(year: i32, month: u8) -> u8 {
    let y = i16::try_from(year).unwrap_or(2000); // fallback for hypothetical overflow
    civil::Date::new(y, month as i8, 1)
        .map(|d| d.days_in_month() as u8)
        .unwrap_or(30) // fallback — should not happen for valid inputs
}

/// Number of days in the given month (hand-rolled proleptic Gregorian).
///
/// Mirrors `jiff::civil::Date::days_in_month` for the in-range year set.
/// Returns 30 for invalid month inputs (parity with the `unwrap_or(30)`
/// branch above; valid inputs never hit it). Leap-year rule (proleptic
/// Gregorian): divisible by 4 AND (not divisible by 100 OR divisible by
/// 400). Note: Rust `%` follows the dividend's sign, but `n % m == 0` is
/// sign-independent for exact multiples, so the rule is correct for all
/// integer years (positive, zero, and negative).
#[cfg(not(feature = "dates"))]
fn days_in_month(year: i32, month: u8) -> u8 {
    // Year-overflow fallback matches the jiff path below: jiff returns
    // `unwrap_or(2000)` for the year on `i16::try_from` overflow and then
    // computes `days_in_month` against year=2000 (a leap year), so
    // February returns 29. We mirror that by falling through to the
    // normal computation with year=2000 instead of returning a constant.
    let y = if i16::try_from(year).is_err() {
        2000
    } else {
        year
    };
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
            if leap { 29 } else { 28 }
        }
        _ => 30, // matches jiff path's fallback for invalid month
    }
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
    fn date_end_cmp_greater_than_date_hour_min_same_day() {
        // Date(y,m,d) end = (y,m,d, 23,59,59,999_999_999).
        // DateHourMin { hour:22, minute:30 } end = (y,m,d, 22,30,59,999_999_999).
        // The full day outlasts even a very late DateHourMin.
        let day = IsmDate::Date(2003, 4, 15);
        let t = IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 22,
            minute: 30,
            offset: None,
        };
        assert_eq!(day.end_cmp(&t), Ordering::Greater);
    }

    #[test]
    fn date_hour_min_end_cmp_later_time_is_greater() {
        let earlier = IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 10,
            minute: 0,
            offset: None,
        };
        let later = IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 14,
            minute: 30,
            offset: None,
        };
        assert_eq!(later.end_cmp(&earlier), Ordering::Greater);
        assert_eq!(earlier.end_cmp(&later), Ordering::Less);
    }

    #[test]
    fn date_hour_min_end_cmp_equal_times_is_equal() {
        let a = IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 14,
            minute: 30,
            offset: None,
        };
        let b = a.clone();
        assert_eq!(a.end_cmp(&b), Ordering::Equal);
    }

    #[test]
    fn date_hour_min_end_cmp_same_civil_negative_offset_is_greater() {
        // 10:30-05:00 = 15:30 UTC > 10:30Z = 10:30 UTC
        let utc = IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 10,
            minute: 30,
            offset: Some(UtcOffset::UTC),
        };
        let eastern = IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 10,
            minute: 30,
            offset: Some(UtcOffset::from_hhmm(-1, 5, 0).unwrap()), // -05:00
        };
        // Eastern is later in UTC, so it should be Greater.
        assert_eq!(eastern.end_cmp(&utc), Ordering::Greater);
        assert_eq!(utc.end_cmp(&eastern), Ordering::Less);
    }

    #[test]
    fn date_hour_min_end_cmp_same_civil_positive_offset_is_less() {
        // 10:30+05:30 = 05:00 UTC < 10:30Z = 10:30 UTC
        let utc = IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 10,
            minute: 30,
            offset: Some(UtcOffset::UTC),
        };
        let india = IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 10,
            minute: 30,
            offset: Some(UtcOffset::from_hhmm(1, 5, 30).unwrap()), // +05:30
        };
        // India is earlier in UTC, so it should be Less.
        assert_eq!(india.end_cmp(&utc), Ordering::Less);
        assert_eq!(utc.end_cmp(&india), Ordering::Greater);
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
        assert_eq!(&*IsmDate::Date(2003, 4, 15).to_maxdate_str(), "20030415");
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

    #[test]
    fn utc_offset_from_str_z_is_utc() {
        assert_eq!("Z".parse::<UtcOffset>().unwrap(), UtcOffset::UTC);
    }

    #[test]
    fn utc_offset_from_str_positive() {
        let o = "+05:30".parse::<UtcOffset>().unwrap();
        assert_eq!(o, UtcOffset::from_hhmm(1, 5, 30).unwrap());
    }

    #[test]
    fn utc_offset_from_str_negative() {
        let o = "-05:00".parse::<UtcOffset>().unwrap();
        assert_eq!(o, UtcOffset::from_hhmm(-1, 5, 0).unwrap());
    }

    #[test]
    fn utc_offset_from_str_round_trip() {
        // "+00:00" canonicalizes to "Z" so it is excluded from this round-trip.
        for s in ["Z", "+05:30", "-05:00", "+23:59", "-23:59"] {
            let parsed: UtcOffset = s.parse().unwrap();
            assert_eq!(parsed.to_string(), s, "round-trip failed for {s:?}");
        }
        // Both "+00:00" and "Z" parse to UTC; canonical display is "Z".
        let zero: UtcOffset = "+00:00".parse().unwrap();
        assert_eq!(zero, UtcOffset::UTC);
        assert_eq!(zero.to_string(), "Z");
    }

    #[test]
    fn utc_offset_from_str_rejects_invalid() {
        for bad in [
            "EST", "UTC", "utc", "+0530", "+05-30", "05:30", "", "+24:00",
        ] {
            assert!(bad.parse::<UtcOffset>().is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn parse_offset_rejects_wrong_separator() {
        // `+05-30` has `-` instead of `:` at index 3 — must be rejected.
        let err = IsmDate::from_str("2003-04-15T10:30+05-30");
        assert!(
            err.is_err(),
            "offset with wrong separator should be Err, got {err:?}"
        );
        // `+0530` (missing separator entirely) is 5 bytes, not 6 — also rejected.
        let err2 = IsmDate::from_str("2003-04-15T10:30+0530");
        assert!(
            err2.is_err(),
            "offset without separator should be Err, got {err2:?}"
        );
    }

    #[test]
    fn parse_datetime_rejects_non_ascii() {
        // Multi-byte UTF-8 must not cause a panic in the byte-offset slicer.
        let result = IsmDate::from_str("2003-04-15T10:30\u{00E9}");
        assert!(result.is_err(), "non-ASCII should be Err, got {result:?}");
    }

    // -----------------------------------------------------------------------
    // UtcOffset additional coverage
    // -----------------------------------------------------------------------

    #[test]
    fn utc_offset_from_hhmm_invalid_sign_zero() {
        assert!(
            UtcOffset::from_hhmm(0, 5, 0).is_none(),
            "sign=0 must be rejected"
        );
    }

    #[test]
    fn utc_offset_from_hhmm_invalid_sign_two() {
        assert!(
            UtcOffset::from_hhmm(2, 5, 0).is_none(),
            "sign=2 must be rejected"
        );
    }

    #[test]
    fn utc_offset_from_hhmm_invalid_sign_minus_two() {
        assert!(
            UtcOffset::from_hhmm(-2, 5, 0).is_none(),
            "sign=-2 must be rejected"
        );
    }

    #[test]
    fn utc_offset_from_hhmm_max_valid_boundary() {
        // ±23:59 is the maximum representable offset (1439 minutes).
        let pos = UtcOffset::from_hhmm(1, 23, 59).unwrap();
        assert_eq!(pos.minutes, 23 * 60 + 59);
        let neg = UtcOffset::from_hhmm(-1, 23, 59).unwrap();
        assert_eq!(neg.minutes, -(23 * 60 + 59));
    }

    #[test]
    fn utc_offset_from_hhmm_rejects_hours_24() {
        assert!(
            UtcOffset::from_hhmm(1, 24, 0).is_none(),
            "hours=24 must be rejected"
        );
    }

    #[test]
    fn utc_offset_from_hhmm_rejects_minutes_60() {
        assert!(
            UtcOffset::from_hhmm(1, 0, 60).is_none(),
            "minutes=60 must be rejected"
        );
    }

    #[test]
    fn utc_offset_to_seconds_utc_is_zero() {
        assert_eq!(UtcOffset::UTC.to_seconds(), 0);
    }

    #[test]
    fn utc_offset_to_seconds_positive() {
        // +05:30 = 330 minutes = 19800 seconds
        let o = UtcOffset::from_hhmm(1, 5, 30).unwrap();
        assert_eq!(o.to_seconds(), 5 * 3600 + 30 * 60);
    }

    #[test]
    fn utc_offset_to_seconds_negative() {
        // -05:00 = -300 minutes = -18000 seconds
        let o = UtcOffset::from_hhmm(-1, 5, 0).unwrap();
        assert_eq!(o.to_seconds(), -5 * 3600);
    }

    #[test]
    fn utc_offset_from_hhmm_zero_positive_sign() {
        // Both sign=1 and sign=-1 produce UTC for 0 hours / 0 minutes.
        let pos = UtcOffset::from_hhmm(1, 0, 0).unwrap();
        let neg = UtcOffset::from_hhmm(-1, 0, 0).unwrap();
        assert_eq!(pos, UtcOffset::UTC);
        assert_eq!(neg, UtcOffset::UTC);
    }

    // -----------------------------------------------------------------------
    // IsmDate component accessors
    // -----------------------------------------------------------------------

    #[test]
    fn year_accessor_all_variants() {
        assert_eq!(IsmDate::Year(2003).year(), 2003);
        assert_eq!(IsmDate::YearMonth(2003, 4).year(), 2003);
        assert_eq!(IsmDate::Date(2003, 4, 15).year(), 2003);
        assert_eq!(
            IsmDate::DateHourMin {
                year: 2003,
                month: 4,
                day: 15,
                hour: 10,
                minute: 30,
                offset: None,
            }
            .year(),
            2003
        );
        assert_eq!(
            IsmDate::DateTime {
                year: 2003,
                month: 4,
                day: 15,
                hour: 10,
                minute: 30,
                second: 0,
                nanosecond: 0,
                offset: None,
            }
            .year(),
            2003
        );
    }

    #[test]
    fn month_accessor_all_variants() {
        assert_eq!(IsmDate::Year(2003).month(), None);
        assert_eq!(IsmDate::YearMonth(2003, 4).month(), Some(4));
        assert_eq!(IsmDate::Date(2003, 4, 15).month(), Some(4));
        assert_eq!(
            IsmDate::DateHourMin {
                year: 2003,
                month: 4,
                day: 15,
                hour: 10,
                minute: 30,
                offset: None,
            }
            .month(),
            Some(4)
        );
        assert_eq!(
            IsmDate::DateTime {
                year: 2003,
                month: 4,
                day: 15,
                hour: 10,
                minute: 30,
                second: 0,
                nanosecond: 0,
                offset: None,
            }
            .month(),
            Some(4)
        );
    }

    #[test]
    fn day_accessor_all_variants() {
        assert_eq!(IsmDate::Year(2003).day(), None);
        assert_eq!(IsmDate::YearMonth(2003, 4).day(), None);
        assert_eq!(IsmDate::Date(2003, 4, 15).day(), Some(15));
        assert_eq!(
            IsmDate::DateHourMin {
                year: 2003,
                month: 4,
                day: 15,
                hour: 10,
                minute: 30,
                offset: None,
            }
            .day(),
            Some(15)
        );
        assert_eq!(
            IsmDate::DateTime {
                year: 2003,
                month: 4,
                day: 15,
                hour: 10,
                minute: 30,
                second: 0,
                nanosecond: 0,
                offset: None,
            }
            .day(),
            Some(15)
        );
    }

    // -----------------------------------------------------------------------
    // IsmDate::contains — DateHourMin and DateTime cases
    // -----------------------------------------------------------------------

    #[test]
    fn date_hour_min_contains_itself() {
        let t = IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 14,
            minute: 30,
            offset: Some(UtcOffset::UTC),
        };
        assert!(t.contains(&t.clone()));
    }

    #[test]
    fn date_hour_min_does_not_contain_coarser() {
        let t = IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 14,
            minute: 30,
            offset: None,
        };
        assert!(!t.contains(&IsmDate::Year(2003)));
        assert!(!t.contains(&IsmDate::YearMonth(2003, 4)));
        assert!(!t.contains(&IsmDate::Date(2003, 4, 15)));
    }

    #[test]
    fn date_hour_min_contains_datetime_same_minute() {
        let dhm = IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 14,
            minute: 30,
            offset: None,
        };
        // DateTime within the same HH:MM must be contained.
        let dt = IsmDate::DateTime {
            year: 2003,
            month: 4,
            day: 15,
            hour: 14,
            minute: 30,
            second: 45,
            nanosecond: 0,
            offset: None,
        };
        assert!(dhm.contains(&dt));
    }

    #[test]
    fn date_hour_min_does_not_contain_datetime_different_minute() {
        let dhm = IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 14,
            minute: 30,
            offset: None,
        };
        let dt = IsmDate::DateTime {
            year: 2003,
            month: 4,
            day: 15,
            hour: 14,
            minute: 31,
            second: 0,
            nanosecond: 0,
            offset: None,
        };
        assert!(!dhm.contains(&dt));
    }

    #[test]
    fn date_hour_min_does_not_contain_datetime_different_offset() {
        // Offsets are compared in their represented form (no UTC normalization).
        let dhm = IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 14,
            minute: 30,
            offset: Some(UtcOffset::UTC),
        };
        let dt = IsmDate::DateTime {
            year: 2003,
            month: 4,
            day: 15,
            hour: 14,
            minute: 30,
            second: 0,
            nanosecond: 0,
            offset: Some(UtcOffset::from_hhmm(-1, 5, 0).unwrap()),
        };
        assert!(!dhm.contains(&dt));
    }

    #[test]
    fn datetime_contains_itself() {
        let dt = IsmDate::DateTime {
            year: 2003,
            month: 4,
            day: 15,
            hour: 14,
            minute: 30,
            second: 45,
            nanosecond: 123_456_789,
            offset: Some(UtcOffset::UTC),
        };
        assert!(dt.contains(&dt.clone()));
    }

    #[test]
    fn datetime_does_not_contain_coarser() {
        let dt = IsmDate::DateTime {
            year: 2003,
            month: 4,
            day: 15,
            hour: 14,
            minute: 30,
            second: 45,
            nanosecond: 0,
            offset: None,
        };
        assert!(!dt.contains(&IsmDate::Year(2003)));
        assert!(!dt.contains(&IsmDate::YearMonth(2003, 4)));
        assert!(!dt.contains(&IsmDate::Date(2003, 4, 15)));
    }

    #[test]
    fn datetime_does_not_contain_datehourmin() {
        let dt = IsmDate::DateTime {
            year: 2003,
            month: 4,
            day: 15,
            hour: 14,
            minute: 30,
            second: 45,
            nanosecond: 0,
            offset: None,
        };
        let dhm = IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 14,
            minute: 30,
            offset: None,
        };
        assert!(!dt.contains(&dhm));
    }

    #[test]
    fn year_contains_datehourmin_same_year() {
        let y = IsmDate::Year(2003);
        let t = IsmDate::DateHourMin {
            year: 2003,
            month: 6,
            day: 15,
            hour: 10,
            minute: 0,
            offset: None,
        };
        assert!(y.contains(&t));
    }

    #[test]
    fn year_contains_datetime_same_year() {
        let y = IsmDate::Year(2003);
        let dt = IsmDate::DateTime {
            year: 2003,
            month: 12,
            day: 31,
            hour: 23,
            minute: 59,
            second: 59,
            nanosecond: 0,
            offset: None,
        };
        assert!(y.contains(&dt));
    }

    #[test]
    fn year_does_not_contain_datehourmin_different_year() {
        let y = IsmDate::Year(2003);
        let t = IsmDate::DateHourMin {
            year: 2004,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            offset: None,
        };
        assert!(!y.contains(&t));
    }

    #[test]
    fn year_month_contains_datehourmin_same_month() {
        let ym = IsmDate::YearMonth(2003, 4);
        let t = IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 10,
            minute: 0,
            offset: None,
        };
        assert!(ym.contains(&t));
    }

    #[test]
    fn year_month_does_not_contain_datehourmin_different_month() {
        let ym = IsmDate::YearMonth(2003, 4);
        let t = IsmDate::DateHourMin {
            year: 2003,
            month: 5,
            day: 1,
            hour: 0,
            minute: 0,
            offset: None,
        };
        assert!(!ym.contains(&t));
    }

    #[test]
    fn date_contains_datetime_same_day() {
        let d = IsmDate::Date(2003, 4, 15);
        let dt = IsmDate::DateTime {
            year: 2003,
            month: 4,
            day: 15,
            hour: 23,
            minute: 59,
            second: 59,
            nanosecond: 999_999_999,
            offset: None,
        };
        assert!(d.contains(&dt));
    }

    #[test]
    fn date_does_not_contain_datetime_different_day() {
        let d = IsmDate::Date(2003, 4, 15);
        let dt = IsmDate::DateTime {
            year: 2003,
            month: 4,
            day: 16,
            hour: 0,
            minute: 0,
            second: 0,
            nanosecond: 0,
            offset: None,
        };
        assert!(!d.contains(&dt));
    }

    // -----------------------------------------------------------------------
    // IsmDate::end_cmp — additional cross-tier and same-tier cases
    // -----------------------------------------------------------------------

    #[test]
    fn year_end_cmp_same_year_is_equal() {
        assert_eq!(
            IsmDate::Year(2003).end_cmp(&IsmDate::Year(2003)),
            Ordering::Equal
        );
    }

    #[test]
    fn year_end_cmp_different_years() {
        assert_eq!(
            IsmDate::Year(2004).end_cmp(&IsmDate::Year(2003)),
            Ordering::Greater
        );
        assert_eq!(
            IsmDate::Year(2003).end_cmp(&IsmDate::Year(2004)),
            Ordering::Less
        );
    }

    #[test]
    fn year_month_end_cmp_same_month_is_equal() {
        assert_eq!(
            IsmDate::YearMonth(2003, 4).end_cmp(&IsmDate::YearMonth(2003, 4)),
            Ordering::Equal
        );
    }

    #[test]
    fn year_month_end_cmp_different_months_same_year() {
        // April ends Apr 30; May ends May 31 → May > April.
        assert_eq!(
            IsmDate::YearMonth(2003, 5).end_cmp(&IsmDate::YearMonth(2003, 4)),
            Ordering::Greater
        );
        assert_eq!(
            IsmDate::YearMonth(2003, 4).end_cmp(&IsmDate::YearMonth(2003, 5)),
            Ordering::Less
        );
    }

    #[test]
    fn date_end_cmp_same_date_is_equal() {
        assert_eq!(
            IsmDate::Date(2003, 4, 15).end_cmp(&IsmDate::Date(2003, 4, 15)),
            Ordering::Equal
        );
    }

    #[test]
    fn date_end_cmp_later_date_is_greater() {
        assert_eq!(
            IsmDate::Date(2003, 4, 16).end_cmp(&IsmDate::Date(2003, 4, 15)),
            Ordering::Greater
        );
    }

    #[test]
    fn datetime_end_cmp_same_instant_is_equal() {
        let dt = IsmDate::DateTime {
            year: 2003,
            month: 4,
            day: 15,
            hour: 10,
            minute: 30,
            second: 45,
            nanosecond: 0,
            offset: None,
        };
        assert_eq!(dt.end_cmp(&dt.clone()), Ordering::Equal);
    }

    #[test]
    fn datetime_end_cmp_later_second_is_greater() {
        let earlier = IsmDate::DateTime {
            year: 2003,
            month: 4,
            day: 15,
            hour: 10,
            minute: 30,
            second: 44,
            nanosecond: 0,
            offset: None,
        };
        let later = IsmDate::DateTime {
            year: 2003,
            month: 4,
            day: 15,
            hour: 10,
            minute: 30,
            second: 45,
            nanosecond: 0,
            offset: None,
        };
        assert_eq!(later.end_cmp(&earlier), Ordering::Greater);
        assert_eq!(earlier.end_cmp(&later), Ordering::Less);
    }

    #[test]
    fn datetime_end_cmp_nanosecond_tiebreak() {
        let a = IsmDate::DateTime {
            year: 2003,
            month: 4,
            day: 15,
            hour: 10,
            minute: 30,
            second: 45,
            nanosecond: 0,
            offset: None,
        };
        let b = IsmDate::DateTime {
            year: 2003,
            month: 4,
            day: 15,
            hour: 10,
            minute: 30,
            second: 45,
            nanosecond: 1,
            offset: None,
        };
        assert_eq!(b.end_cmp(&a), Ordering::Greater);
    }

    #[test]
    fn date_hour_min_floating_is_treated_as_offset_zero() {
        // Floating DateHourMin uses offset=0 for tie-breaking; same civil time
        // as UTC means they compare Equal.
        let floating = IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 10,
            minute: 30,
            offset: None,
        };
        let utc = IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 10,
            minute: 30,
            offset: Some(UtcOffset::UTC),
        };
        // utc_tie_break is -offset.minutes; for UTC that's 0; for floating that's
        // also 0. So they compare Equal on the tie-break.
        assert_eq!(floating.end_cmp(&utc), Ordering::Equal);
    }

    #[test]
    fn year_end_cmp_vs_year_month_same_year() {
        // Year(2003) ends Dec 31 23:59:59; YearMonth(2003, 6) ends Jun 30 23:59:59.
        assert_eq!(
            IsmDate::Year(2003).end_cmp(&IsmDate::YearMonth(2003, 6)),
            Ordering::Greater
        );
        assert_eq!(
            IsmDate::YearMonth(2003, 12).end_cmp(&IsmDate::Year(2003)),
            Ordering::Equal // Dec 31 == Dec 31
        );
    }

    // -----------------------------------------------------------------------
    // to_maxdate_str — DateHourMin and DateTime cases
    // -----------------------------------------------------------------------

    #[test]
    fn to_maxdate_str_date_hour_min() {
        // DateHourMin uses the date component only.
        let t = IsmDate::DateHourMin {
            year: 2003,
            month: 4,
            day: 15,
            hour: 14,
            minute: 30,
            offset: None,
        };
        assert_eq!(&*t.to_maxdate_str(), "20030415");
    }

    #[test]
    fn to_maxdate_str_datetime() {
        let dt = IsmDate::DateTime {
            year: 2003,
            month: 4,
            day: 15,
            hour: 14,
            minute: 30,
            second: 45,
            nanosecond: 0,
            offset: Some(UtcOffset::UTC),
        };
        assert_eq!(&*dt.to_maxdate_str(), "20030415");
    }

    #[test]
    fn to_maxdate_str_all_months_days_in_month() {
        // Verify days_in_month for all 12 months in a non-leap year (2003).
        let expected = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        for (i, &days) in expected.iter().enumerate() {
            let month = (i + 1) as u8;
            let ym = IsmDate::YearMonth(2003, month);
            let s = ym.to_maxdate_str();
            let day_part: u8 = s[6..].parse().unwrap();
            assert_eq!(
                day_part, days,
                "2003-{month:02} should end on day {days}, got {day_part}"
            );
        }
    }

    #[test]
    fn to_maxdate_str_february_leap_year() {
        // 2000 is a leap year: February has 29 days.
        assert_eq!(&*IsmDate::YearMonth(2000, 2).to_maxdate_str(), "20000229");
        // 1900 is NOT a leap year (divisible by 100 but not 400).
        assert_eq!(&*IsmDate::YearMonth(1900, 2).to_maxdate_str(), "19000228");
    }

    // -----------------------------------------------------------------------
    // ApproxIsmDate Display
    // -----------------------------------------------------------------------

    #[test]
    fn approx_ism_date_display_without_qualifier() {
        let a = ApproxIsmDate {
            date: IsmDate::Year(2003),
            qualifier: None,
        };
        assert_eq!(a.to_string(), "2003");
    }

    #[test]
    fn approx_ism_date_display_with_qualifier() {
        let a = ApproxIsmDate {
            date: IsmDate::Year(1995),
            qualifier: Some(ApproxQualifier::Circa),
        };
        assert_eq!(a.to_string(), "circa 1995");
    }

    #[test]
    fn approx_ism_date_display_all_qualifiers() {
        let pairs = [
            (ApproxQualifier::FirstQtr, "1st qtr 2003"),
            (ApproxQualifier::SecondQtr, "2nd qtr 2003"),
            (ApproxQualifier::ThirdQtr, "3rd qtr 2003"),
            (ApproxQualifier::FourthQtr, "4th qtr 2003"),
            (ApproxQualifier::Circa, "circa 2003"),
            (ApproxQualifier::Early, "early 2003"),
            (ApproxQualifier::Mid, "mid 2003"),
            (ApproxQualifier::Late, "late 2003"),
        ];
        for (qualifier, expected) in pairs {
            let a = ApproxIsmDate {
                date: IsmDate::Year(2003),
                qualifier: Some(qualifier),
            };
            assert_eq!(a.to_string(), expected, "qualifier={qualifier:?}");
        }
    }

    // -----------------------------------------------------------------------
    // ParseIsmDateError and ParseApproxQualifierError Display
    // -----------------------------------------------------------------------

    #[test]
    fn parse_ism_date_error_display() {
        let err = IsmDate::from_str("not-a-date").unwrap_err();
        let s = err.to_string();
        assert!(
            s.contains("invalid ISM date"),
            "error display should mention 'invalid ISM date', got: {s:?}"
        );
    }

    #[test]
    fn parse_approx_qualifier_error_display() {
        let err = ApproxQualifier::from_str("bogus").unwrap_err();
        let s = err.to_string();
        assert!(
            s.contains("invalid approx qualifier"),
            "error display should mention 'invalid approx qualifier', got: {s:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Parsing edge cases not covered above
    // -----------------------------------------------------------------------

    #[test]
    fn rejects_short_strings() {
        for s in ["", "2", "20", "200", "20030"] {
            assert!(
                IsmDate::from_str(s).is_err(),
                "should reject short string {s:?}"
            );
        }
    }

    #[test]
    fn rejects_nine_char_string() {
        // 9 chars doesn't match any pattern.
        assert!(IsmDate::from_str("200304150").is_err());
    }

    #[test]
    fn rejects_day_zero_in_date() {
        assert!(IsmDate::from_str("2003-04-00").is_err());
    }

    #[test]
    fn rejects_day_32_in_date() {
        assert!(IsmDate::from_str("2003-01-32").is_err());
    }

    #[test]
    fn rejects_yyyymmdd_month_13() {
        assert!(IsmDate::from_str("20031301").is_err());
    }

    #[test]
    fn rejects_yyyymmdd_day_00() {
        assert!(IsmDate::from_str("20030400").is_err());
    }

    #[test]
    fn rejects_datehourmin_hour_out_of_range() {
        assert!(IsmDate::from_str("2003-04-15T24:00").is_err());
        assert!(IsmDate::from_str("2003-04-15T25:00Z").is_err());
    }

    #[test]
    fn rejects_datehourmin_minute_out_of_range() {
        assert!(IsmDate::from_str("2003-04-15T10:60").is_err());
        assert!(IsmDate::from_str("2003-04-15T10:99Z").is_err());
    }

    #[test]
    fn rejects_datetime_second_out_of_range() {
        assert!(IsmDate::from_str("2003-04-15T10:30:60Z").is_err());
        assert!(IsmDate::from_str("2003-04-15T10:30:99").is_err());
    }

    #[test]
    fn rejects_fractional_seconds_empty() {
        // A period with no digits after it is invalid.
        assert!(IsmDate::from_str("2003-04-15T10:30:00.Z").is_err());
        assert!(IsmDate::from_str("2003-04-15T10:30:00.").is_err());
    }

    #[test]
    fn rejects_fractional_seconds_too_many_digits() {
        // More than 9 fractional digits must be rejected.
        assert!(IsmDate::from_str("2003-04-15T10:30:00.1234567890Z").is_err());
    }

    #[test]
    fn accepts_fractional_seconds_9_digits() {
        // Exactly 9 digits (nanosecond precision) must be accepted.
        assert!(IsmDate::from_str("2003-04-15T10:30:00.123456789Z").is_ok());
    }

    #[test]
    fn rejects_bad_offset_in_datetime() {
        assert!(IsmDate::from_str("2003-04-15T10:30:00+99:99").is_err());
        assert!(IsmDate::from_str("2003-04-15T10:30:00+24:00").is_err());
    }

    #[test]
    fn rejects_bad_offset_in_datehourmin() {
        assert!(IsmDate::from_str("2003-04-15T10:30+99:99").is_err());
        assert!(IsmDate::from_str("2003-04-15T10:30+24:00").is_err());
    }

    #[test]
    fn rejects_unknown_suffix_after_datehourmin() {
        // Anything after HH:MM that is not empty, Z, or ±HH:MM is invalid.
        assert!(IsmDate::from_str("2003-04-15T10:30:garbage").is_err());
    }

    #[test]
    fn rejects_year_with_non_digit_separator() {
        // 7-char string where bytes[4] != b'-' falls to the catch-all error.
        assert!(IsmDate::from_str("2003X04").is_err());
    }

    #[test]
    fn rejects_date_with_wrong_separator() {
        assert!(IsmDate::from_str("2003/04/15").is_err());
    }

    #[test]
    fn round_trip_datetime_with_nanos() {
        // 9-digit fractional seconds round-trips.
        assert!(round_trip("2003-04-15T14:30:00.123456789Z"));
    }

    #[test]
    fn round_trip_datetime_with_negative_offset() {
        assert!(round_trip("2003-04-15T14:30:00-05:00"));
    }

    #[test]
    fn round_trip_date_hour_min_negative_offset() {
        assert!(round_trip("2003-04-15T14:30-07:00"));
    }

    #[test]
    fn round_trip_year_month_january() {
        assert!(round_trip("2003-01"));
    }

    #[test]
    fn round_trip_year_month_december() {
        assert!(round_trip("2003-12"));
    }

    #[test]
    fn capco_yyyymmdd_rejects_invalid_calendar_date() {
        // YYYYMMDD with month 13 must not silently succeed.
        assert!(IsmDate::from_str("20031301").is_err());
        // YYYYMMDD with day 0 must fail.
        assert!(IsmDate::from_str("20030400").is_err());
        // Non-leap February 29.
        assert!(IsmDate::from_str("20030229").is_err());
    }

    #[test]
    fn utc_offset_from_str_all_canonical_forms() {
        // Positive offset round-trips correctly.
        let o: UtcOffset = "+12:00".parse().unwrap();
        assert_eq!(o.minutes, 720);
        assert_eq!(o.to_string(), "+12:00");

        // Negative offset round-trips correctly.
        let o: UtcOffset = "-12:00".parse().unwrap();
        assert_eq!(o.minutes, -720);
        assert_eq!(o.to_string(), "-12:00");
    }

    // -----------------------------------------------------------------------
    // Calendar validity — runs under both backends (jiff + hand-rolled).
    // Issue #455: confirms the proleptic Gregorian leap-year rule is correct
    // for century years and the proleptic year 0. These pass through the
    // public `IsmDate::from_str` API so they exercise whichever backend is
    // compiled in. The direct `days_in_month` calls cover negative years
    // which are unreachable via the parser but documented as correct by
    // the doc comment.
    // -----------------------------------------------------------------------

    #[test]
    fn leap_year_century_rule_via_from_str() {
        // 1900: divisible by 100 but not 400 → NOT leap.
        assert!(IsmDate::from_str("19000229").is_err());
        assert!(IsmDate::from_str("19000228").is_ok());

        // 2000: divisible by 400 → leap.
        assert!(IsmDate::from_str("20000229").is_ok());

        // 2100: divisible by 100 but not 400 → NOT leap.
        assert!(IsmDate::from_str("21000229").is_err());
        assert!(IsmDate::from_str("21000228").is_ok());

        // 2400: divisible by 400 → leap.
        assert!(IsmDate::from_str("24000229").is_ok());
    }

    #[test]
    fn leap_year_proleptic_zero_via_from_str() {
        // Year 0000: 0 % 400 == 0 → leap in the proleptic Gregorian system.
        assert!(IsmDate::from_str("00000229").is_ok());
        assert!(IsmDate::from_str("00000228").is_ok());
    }

    #[test]
    fn days_in_month_negative_year_unreachable_but_consistent() {
        // `IsmDate::from_str` rejects negative years (4-digit ASCII parser),
        // so `days_in_month(-400, 2)` is unreachable via the parser. We
        // assert it anyway to document that the Rust `%` sign semantics
        // don't break the leap rule at exact multiples — `(-400) % 400`
        // is 0, so `-400` is correctly classified as leap.
        assert_eq!(days_in_month(-400, 2), 29);
        // `-100`: divisible by 100 but `(-100) % 400 != 0` → not leap.
        assert_eq!(days_in_month(-100, 2), 28);
        // `-4`: divisible by 4, `(-4) % 100 != 0` → leap.
        assert_eq!(days_in_month(-4, 2), 29);
    }

    #[test]
    fn days_in_month_all_months_non_leap() {
        // Exhaustive non-leap February + every other month at year 2003.
        assert_eq!(days_in_month(2003, 1), 31);
        assert_eq!(days_in_month(2003, 2), 28);
        assert_eq!(days_in_month(2003, 3), 31);
        assert_eq!(days_in_month(2003, 4), 30);
        assert_eq!(days_in_month(2003, 5), 31);
        assert_eq!(days_in_month(2003, 6), 30);
        assert_eq!(days_in_month(2003, 7), 31);
        assert_eq!(days_in_month(2003, 8), 31);
        assert_eq!(days_in_month(2003, 9), 30);
        assert_eq!(days_in_month(2003, 10), 31);
        assert_eq!(days_in_month(2003, 11), 30);
        assert_eq!(days_in_month(2003, 12), 31);
        // Invalid month → fallback (30 — matches both backends).
        assert_eq!(days_in_month(2003, 0), 30);
        assert_eq!(days_in_month(2003, 13), 30);
    }
}
