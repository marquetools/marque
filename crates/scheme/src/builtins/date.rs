// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::lattice::{
    BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice,
};

/// Date-valued lattice storing an ISO-8601 `YYYY` or `YYYYMMDD` string.
///
/// Join picks the lexicographically greater string (which is also the
/// chronologically later date under that encoding — same rationale as
/// `marque_capco::lattice::DeclassifyOnLattice::from_attrs_iter`).
/// Bottom is the absent date (`None`).
///
/// # Validation
///
/// The inner field is private and construction is gated through
/// [`MaxDate::present`] (panicking on invalid) or [`MaxDate::try_present`]
/// (fallible). Accepted inputs are exactly `[0-9]{4}` or `[0-9]{8}` —
/// the two forms CAPCO's `declassify_on` uses. This is what makes
/// [`BoundedMeetSemilattice::top`] lawful: its sentinel `99991231` is strictly
/// greater than every representable value under the lex ordering.
/// Without this gate, a caller could construct e.g. `"ZZZZ"` whose
/// lex order is greater than `99991231`, breaking `top ⊔ a = top`.
///
/// # Storage
///
/// We store the owned string rather than a reference so the lattice
/// value can outlive any single input portion.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MaxDate {
    inner: Option<Box<str>>,
}

impl MaxDate {
    #[inline]
    pub fn absent() -> Self {
        Self { inner: None }
    }

    /// Construct from a validated date string. Panics if `s` is neither
    /// exactly 4 nor exactly 8 ASCII digits.
    ///
    /// Prefer [`MaxDate::try_present`] at system boundaries where input
    /// is untrusted; `present` is the convenience entry for scheme
    /// code that already has a known-valid string.
    #[inline]
    pub fn present(s: impl Into<Box<str>>) -> Self {
        let boxed = s.into();
        assert!(
            Self::is_valid(&boxed),
            "MaxDate::present: expected YYYY or YYYYMMDD; got {boxed:?}"
        );
        Self { inner: Some(boxed) }
    }

    /// Fallible constructor. Returns `None` unless `s` matches the
    /// accepted `[0-9]{4}` or `[0-9]{8}` shape.
    #[inline]
    pub fn try_present(s: impl Into<Box<str>>) -> Option<Self> {
        let boxed = s.into();
        if Self::is_valid(&boxed) {
            Some(Self { inner: Some(boxed) })
        } else {
            None
        }
    }

    #[inline]
    pub fn as_deref(&self) -> Option<&str> {
        self.inner.as_deref()
    }

    fn is_valid(s: &str) -> bool {
        (s.len() == 4 || s.len() == 8) && s.bytes().all(|b| b.is_ascii_digit())
    }
}

impl JoinSemilattice for MaxDate {
    #[inline]
    fn join(&self, other: &Self) -> Self {
        match (&self.inner, &other.inner) {
            (None, None) => Self { inner: None },
            (Some(a), None) => Self {
                inner: Some(a.clone()),
            },
            (None, Some(b)) => Self {
                inner: Some(b.clone()),
            },
            (Some(a), Some(b)) => Self {
                inner: Some(if a >= b { a.clone() } else { b.clone() }),
            },
        }
    }
}

impl MeetSemilattice for MaxDate {
    #[inline]
    fn meet(&self, other: &Self) -> Self {
        match (&self.inner, &other.inner) {
            (None, _) | (_, None) => Self { inner: None },
            (Some(a), Some(b)) => Self {
                inner: Some(if a <= b { a.clone() } else { b.clone() }),
            },
        }
    }
}

impl BoundedJoinSemilattice for MaxDate {
    fn bottom() -> Self {
        Self { inner: None }
    }
}

impl BoundedMeetSemilattice for MaxDate {
    fn top() -> Self {
        Self {
            inner: Some("99991231".into()),
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::lattice::{
        BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice,
    };

    #[test]
    fn max_date_join_picks_later() {
        let a = MaxDate::present("20301231");
        let b = MaxDate::present("20481231");
        assert_eq!(a.join(&b), b);
    }

    #[test]
    fn max_date_bottom_is_absent() {
        assert_eq!(MaxDate::bottom(), MaxDate::absent());
    }

    #[test]
    fn max_date_join_absent_returns_present() {
        let a = MaxDate::present("20301231");
        let b = MaxDate::absent();
        assert_eq!(a.join(&b), a);
        assert_eq!(b.join(&a), a);
    }

    #[test]
    fn max_date_year_prefix_of_yyyymmdd_in_same_year() {
        let year = MaxDate::present("2030");
        let date = MaxDate::present("20300601");
        assert_eq!(year.join(&date), date);
    }

    #[test]
    fn max_date_try_present_accepts_yyyy() {
        assert!(MaxDate::try_present("2030").is_some());
    }

    #[test]
    fn max_date_try_present_accepts_yyyymmdd() {
        assert!(MaxDate::try_present("20301231").is_some());
    }

    #[test]
    fn max_date_try_present_rejects_wrong_length() {
        assert!(MaxDate::try_present("").is_none());
        assert!(MaxDate::try_present("30").is_none());
        assert!(MaxDate::try_present("203").is_none());
        assert!(MaxDate::try_present("20301").is_none());
        assert!(MaxDate::try_present("203012310").is_none());
    }

    #[test]
    fn max_date_try_present_rejects_non_digits() {
        assert!(MaxDate::try_present("ZZZZ").is_none());
        assert!(MaxDate::try_present("2030AAAA").is_none());
        assert!(MaxDate::try_present("203O").is_none());
    }

    #[test]
    #[should_panic(expected = "MaxDate::present: expected YYYY or YYYYMMDD")]
    fn max_date_present_panics_on_invalid() {
        let _ = MaxDate::present("ZZZZ");
    }

    #[test]
    fn max_date_top_dominates_every_valid_value() {
        let t = MaxDate::top();
        for s in ["2000", "20001231", "20991231", "99981231", "99991230"] {
            let a = MaxDate::present(s);
            assert_eq!(t.join(&a), t, "top ⊔ {s} must equal top");
        }
    }

    #[test]
    fn max_date_absent_and_as_deref() {
        assert!(MaxDate::absent().as_deref().is_none());
        let d = MaxDate::present("20301231");
        assert_eq!(d.as_deref(), Some("20301231"));
    }

    #[test]
    fn max_date_default_is_absent() {
        assert_eq!(MaxDate::default(), MaxDate::absent());
    }

    #[test]
    fn max_date_join_none_none_is_none() {
        assert_eq!(
            MaxDate::absent().join(&MaxDate::absent()),
            MaxDate::absent()
        );
    }

    #[test]
    fn max_date_join_equal_dates() {
        let a = MaxDate::present("20301231");
        assert_eq!(a.join(&a.clone()), a);
    }

    #[test]
    fn max_date_join_picks_left_when_greater() {
        let a = MaxDate::present("20481231");
        let b = MaxDate::present("20301231");
        assert_eq!(a.join(&b), a);
    }

    #[test]
    fn max_date_meet_picks_earlier() {
        let a = MaxDate::present("20301231");
        let b = MaxDate::present("20481231");
        assert_eq!(a.meet(&b), a);
        assert_eq!(b.meet(&a), a);
    }

    #[test]
    fn max_date_meet_equal_dates() {
        let a = MaxDate::present("20301231");
        assert_eq!(a.meet(&a.clone()), a);
    }

    #[test]
    fn max_date_meet_absent_collapses_to_absent() {
        let a = MaxDate::present("20301231");
        assert_eq!(a.meet(&MaxDate::absent()), MaxDate::absent());
        assert_eq!(MaxDate::absent().meet(&a), MaxDate::absent());
        assert_eq!(
            MaxDate::absent().meet(&MaxDate::absent()),
            MaxDate::absent()
        );
    }

    #[test]
    fn max_date_top_is_sentinel() {
        let t = MaxDate::top();
        assert_eq!(t.as_deref(), Some("99991231"));
        let d = MaxDate::present("20481231");
        assert_eq!(t.join(&d), t);
    }
}
