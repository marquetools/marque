// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use super::*;
use std::str::FromStr;

fn round_trip(s: &str) -> bool {
    IsmDate::from_str(s)
        .map(|d| d.to_string() == s)
        .unwrap_or(false)
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
