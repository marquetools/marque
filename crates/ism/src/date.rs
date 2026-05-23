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
//!   implementation lives in this module. Same `validate_date` +
//!   `days_in_month` surface; SC-008 native↔WASM parity tests enforce
//!   output agreement.

use std::cmp::Ordering;

mod calendar;
mod normalize;
mod parse;

use self::calendar::days_in_month;

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
/// [`Display`] produces canonical ISO 8601 form (with hyphens and
/// `T` separator) suitable for round-trip:
/// `IsmDate::from_str(&date.to_string()) == Ok(date)`.
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
    /// [`std::fmt::Display`] always produces `"YYYY-MM-DD"`.
    Date(i32, u8, u8),

    /// `dateHourMinType` — e.g. `"2003-04-15T14:30Z"`.
    ///
    /// Date + hour + minute with optional UTC offset. The XSD regex restricts
    /// to HH:MM only (no seconds or fractional seconds); sub-minute precision
    /// is represented by [`DateTime`] instead.
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

    /// Returns `true` if `point` falls within the temporal span this date
    /// represents.
    pub fn contains(&self, point: &IsmDate) -> bool {
        if self.year() != point.year() {
            return false;
        }
        match self {
            IsmDate::Year(_) => true,
            IsmDate::YearMonth(_, sm) => match point {
                IsmDate::Year(_) => false,
                _ => point.month() == Some(*sm),
            },
            IsmDate::Date(_, sm, sd) => match point {
                IsmDate::Year(_) | IsmDate::YearMonth(_, _) => false,
                _ => point.month() == Some(*sm) && point.day() == Some(*sd),
            },
            IsmDate::DateHourMin {
                month: sm,
                day: sd,
                hour: sh,
                minute: smin,
                offset: soff,
                ..
            } => match point {
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
            },
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

    /// Compare two `IsmDate` values by their **end-of-span** instants.
    pub fn end_cmp(&self, other: &IsmDate) -> Ordering {
        let a = self.end_components();
        let b = other.end_components();
        a.cmp(&b)
    }

    /// Returns the end-of-span as a sortable `YYYYMMDD` string for use as a
    /// MaxDate lattice key.
    pub fn to_maxdate_str(&self) -> Box<str> {
        let (y, m, d, _, _, _, _, _) = self.end_components();
        format!("{:04}{:02}{:02}", y, m, d).into_boxed_str()
    }

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

/// Parse error for [`IsmDate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseIsmDateError {
    msg: &'static str,
}

impl ParseIsmDateError {
    const fn new(msg: &'static str) -> Self {
        Self { msg }
    }
}

/// Approximation qualifier from `DateApproximationVocabType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApproxQualifier {
    FirstQtr,
    SecondQtr,
    ThirdQtr,
    FourthQtr,
    Circa,
    Early,
    Mid,
    Late,
}

/// Parse error for [`ApproxQualifier`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseApproxQualifierError;

/// An [`IsmDate`] paired with an optional [`ApproxQualifier`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ApproxIsmDate {
    pub date: IsmDate,
    pub qualifier: Option<ApproxQualifier>,
}

#[cfg(test)]
mod tests;
