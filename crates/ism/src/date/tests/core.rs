// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use super::*;
use std::cmp::Ordering;
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
