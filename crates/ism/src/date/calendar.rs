// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use super::ParseIsmDateError;

#[cfg(feature = "dates")]
use jiff::civil;

/// Validate a complete year/month/day triple.
#[cfg(feature = "dates")]
pub(super) fn validate_date(year: i32, month: u8, day: u8) -> Result<(), ParseIsmDateError> {
    let y = i16::try_from(year).map_err(|_| ParseIsmDateError::new("year out of i16 range"))?;
    civil::Date::new(y, month as i8, day as i8)
        .map_err(|_| ParseIsmDateError::new("invalid calendar date"))?;
    Ok(())
}

/// Validate a complete year/month/day triple (hand-rolled proleptic Gregorian).
#[cfg(not(feature = "dates"))]
pub(super) fn validate_date(year: i32, month: u8, day: u8) -> Result<(), ParseIsmDateError> {
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
pub(super) fn validate_year_month(year: i32, month: u8) -> Result<(), ParseIsmDateError> {
    if !(1..=12).contains(&month) {
        return Err(ParseIsmDateError::new("month out of range 1–12"));
    }
    let _y = i16::try_from(year).map_err(|_| ParseIsmDateError::new("year out of i16 range"))?;
    Ok(())
}

/// Number of days in the given month (jiff path).
#[cfg(feature = "dates")]
pub(super) fn days_in_month(year: i32, month: u8) -> u8 {
    let y = i16::try_from(year).unwrap_or(2000);
    civil::Date::new(y, month as i8, 1)
        .map(|d| d.days_in_month() as u8)
        .unwrap_or(30)
}

/// Number of days in the given month (hand-rolled proleptic Gregorian).
#[cfg(not(feature = "dates"))]
pub(super) fn days_in_month(year: i32, month: u8) -> u8 {
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
        _ => 30,
    }
}
