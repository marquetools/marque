// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use super::*;
use std::cmp::Ordering;
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
