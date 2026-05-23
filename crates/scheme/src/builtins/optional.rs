// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::lattice::{
    BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice,
};
use std::collections::BTreeMap;

/// Multiset of observation counts per value, with `join` taking the
/// **per-key max** of counts across both operands.
///
/// Useful for schemes (corporate, medical) that want "most common
/// sensitivity" semantics instead of "most restrictive." Storage keeps a
/// count per value; [`ModeSet::mode`] returns the value with the
/// highest observed count.
///
/// # Why per-key max (not sum)
///
/// A lattice `join` must be idempotent: `a ⊔ a = a`. If `join` summed
/// counts, `a.join(&a)` would double every count and violate the
/// contract. Per-key max gives an idempotent join while still tracking
/// "the highest frequency observed for this value across any source."
/// For callers that need sum-of-observations semantics (counting total
/// votes), combine multisets via [`ModeSet::extend_counts`] rather
/// than `join` — that method deliberately does not claim lattice
/// semantics.
///
/// Bottom is the empty multiset; `top()` would require a distinguished
/// "every value with infinite count" sentinel and is not provided
/// generically (see `BoundedLattice` note on [`crate::builtins::FlatSet`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeSet<T: Ord + Clone>(BTreeMap<T, u32>);

impl<T: Ord + Clone> Default for ModeSet<T> {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}

impl<T: Ord + Clone> ModeSet<T> {
    pub fn from_iter_counted<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut m: BTreeMap<T, u32> = BTreeMap::new();
        for v in iter {
            *m.entry(v).or_insert(0) += 1;
        }
        Self(m)
    }

    /// Return the mode (most-frequent value). `None` on an empty
    /// multiset; ties broken by `Ord` (smaller value wins).
    pub fn mode(&self) -> Option<&T> {
        self.0
            .iter()
            .max_by(|(ak, av), (bk, bv)| av.cmp(bv).then_with(|| bk.cmp(ak)))
            .map(|(k, _)| k)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Sum observation counts across `other` into `self`. **Not a
    /// lattice operation** — sums are not idempotent. Callers who want
    /// "how many total votes did each value get across N sources" use
    /// this; callers who want "highest frequency observed across any
    /// single source" use [`JoinSemilattice::join`].
    pub fn extend_counts(&self, other: &Self) -> Self {
        let mut out = self.0.clone();
        for (k, v) in &other.0 {
            if let Some(slot) = out.get_mut(k) {
                *slot += v;
            } else {
                out.insert(k.clone(), *v);
            }
        }
        Self(out)
    }
}

impl<T: Ord + Clone> JoinSemilattice for ModeSet<T> {
    #[inline]
    fn join(&self, other: &Self) -> Self {
        let mut out = self.0.clone();
        for (k, v) in &other.0 {
            if let Some(slot) = out.get_mut(k) {
                if *v > *slot {
                    *slot = *v;
                }
            } else {
                out.insert(k.clone(), *v);
            }
        }
        Self(out)
    }
}

impl<T: Ord + Clone> MeetSemilattice for ModeSet<T> {
    #[inline]
    fn meet(&self, other: &Self) -> Self {
        let mut out: BTreeMap<T, u32> = BTreeMap::new();
        for (k, v_self) in &self.0 {
            if let Some(v_other) = other.0.get(k) {
                out.insert(k.clone(), *v_self.min(v_other));
            }
        }
        Self(out)
    }
}

/// Optional wrapper around any inner join-semilattice. `None` is the
/// bottom — the join with `None` is whichever operand has a value,
/// and the join of two `Some`s calls the inner type's `join`.
///
/// Use for optional single-value categories where the "value-present"
/// case already has a semilattice (e.g.,
/// `OptionalSingleton<OrdMax<Level>>` for a scheme where classification
/// is optional).
///
/// The struct bound is relaxed to `JoinSemilattice` so that callers
/// with join-only inner types (e.g., `OptionalSingleton<DissemSet>`)
/// can construct values without a `meet` impl. The wrapper implements
/// [`MeetSemilattice`] conditionally — only when `L: MeetSemilattice`.
/// This means `OptionalSingleton<L>` is a full [`crate::lattice::Lattice`]
/// when `L` is a full lattice, and a join-semilattice when `L` is join-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionalSingleton<L: JoinSemilattice>(pub Option<L>);

impl<L: JoinSemilattice> Default for OptionalSingleton<L> {
    fn default() -> Self {
        Self(None)
    }
}

impl<L: JoinSemilattice> OptionalSingleton<L> {
    #[inline]
    pub fn absent() -> Self {
        Self(None)
    }

    #[inline]
    pub fn present(l: L) -> Self {
        Self(Some(l))
    }
}

impl<L: JoinSemilattice> JoinSemilattice for OptionalSingleton<L> {
    #[inline]
    fn join(&self, other: &Self) -> Self {
        match (&self.0, &other.0) {
            (None, None) => Self(None),
            (Some(a), None) => Self(Some(a.clone())),
            (None, Some(b)) => Self(Some(b.clone())),
            (Some(a), Some(b)) => Self(Some(a.join(b))),
        }
    }
}

impl<L: JoinSemilattice + MeetSemilattice> MeetSemilattice for OptionalSingleton<L> {
    #[inline]
    fn meet(&self, other: &Self) -> Self {
        match (&self.0, &other.0) {
            (None, _) | (_, None) => Self(None),
            (Some(a), Some(b)) => Self(Some(a.meet(b))),
        }
    }
}

impl<L: JoinSemilattice> BoundedJoinSemilattice for OptionalSingleton<L> {
    fn bottom() -> Self {
        Self(None)
    }
}

impl<L: JoinSemilattice + BoundedMeetSemilattice> BoundedMeetSemilattice for OptionalSingleton<L> {
    fn top() -> Self {
        Self(Some(L::top()))
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::builtins::{MaxDate, OrdMax};
    use crate::lattice::{
        BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice,
    };

    #[test]
    fn mode_set_mode_returns_max_frequency() {
        let m = ModeSet::from_iter_counted(vec!["a", "b", "b", "c"]);
        assert_eq!(m.mode(), Some(&"b"));
    }

    #[test]
    fn mode_set_join_is_idempotent() {
        let a = ModeSet::from_iter_counted(vec!["a", "b", "b", "c"]);
        assert_eq!(a.join(&a), a);
    }

    #[test]
    fn mode_set_join_takes_per_key_max() {
        let a = ModeSet::from_iter_counted(vec!["x", "x", "x", "y"]);
        let b = ModeSet::from_iter_counted(vec!["x", "y", "y", "z", "z", "z", "z", "z"]);
        let j = a.join(&b);
        assert_eq!(j.mode(), Some(&"z"));
    }

    #[test]
    fn mode_set_extend_counts_sums_observations() {
        let a = ModeSet::from_iter_counted(vec!["a", "b"]);
        let b = ModeSet::from_iter_counted(vec!["b", "c"]);
        let combined = a.extend_counts(&b);
        assert_eq!(combined.mode(), Some(&"b"));
    }

    #[test]
    fn optional_singleton_join_none_none() {
        let a: OptionalSingleton<OrdMax<u32>> = OptionalSingleton::absent();
        let b: OptionalSingleton<OrdMax<u32>> = OptionalSingleton::absent();
        assert_eq!(a.join(&b), a);
    }

    #[test]
    fn optional_singleton_join_some_none() {
        let a = OptionalSingleton::present(OrdMax(3_u32));
        let b: OptionalSingleton<OrdMax<u32>> = OptionalSingleton::absent();
        assert_eq!(a.join(&b), a);
        assert_eq!(b.join(&a), a);
    }

    #[test]
    fn optional_singleton_join_some_some() {
        let a = OptionalSingleton::present(OrdMax(3_u32));
        let b = OptionalSingleton::present(OrdMax(7_u32));
        assert_eq!(a.join(&b), OptionalSingleton::present(OrdMax(7)));
    }

    #[test]
    fn mode_set_default_is_empty() {
        let m: ModeSet<&str> = ModeSet::default();
        assert!(m.is_empty());
        assert_eq!(m.mode(), None);
    }

    #[test]
    fn mode_set_mode_ties_broken_by_ord() {
        let m = ModeSet::from_iter_counted(vec!["b", "a"]);
        assert_eq!(m.mode(), Some(&"a"));
    }

    #[test]
    fn mode_set_is_empty_on_populated_returns_false() {
        let m = ModeSet::from_iter_counted(vec!["a"]);
        assert!(!m.is_empty());
    }

    #[test]
    fn mode_set_extend_counts_preserves_self() {
        let a = ModeSet::from_iter_counted(vec!["a"]);
        let combined = a.extend_counts(&ModeSet::default());
        assert_eq!(combined, a);
    }

    #[test]
    fn mode_set_meet_is_per_key_min() {
        let a = ModeSet::from_iter_counted(vec!["x", "x", "y"]);
        let b = ModeSet::from_iter_counted(vec!["x", "y", "y"]);
        let m = a.meet(&b);
        assert_eq!(m.mode(), Some(&"x"));
    }

    #[test]
    fn mode_set_meet_drops_keys_missing_on_either_side() {
        let a = ModeSet::from_iter_counted(vec!["a", "b"]);
        let b = ModeSet::from_iter_counted(vec!["a", "c"]);
        let m = a.meet(&b);
        assert_eq!(m.mode(), Some(&"a"));
    }

    #[test]
    fn optional_singleton_meet_some_some_calls_inner() {
        let a = OptionalSingleton::present(OrdMax(3_u32));
        let b = OptionalSingleton::present(OrdMax(7_u32));
        assert_eq!(a.meet(&b), OptionalSingleton::present(OrdMax(3)));
    }

    #[test]
    fn optional_singleton_meet_none_collapses() {
        let a = OptionalSingleton::present(OrdMax(3_u32));
        let none: OptionalSingleton<OrdMax<u32>> = OptionalSingleton::absent();
        assert_eq!(a.meet(&none), none);
        assert_eq!(none.meet(&a), none);
        assert_eq!(none.meet(&none), none);
    }

    #[test]
    fn optional_singleton_default_is_absent() {
        let a: OptionalSingleton<OrdMax<u32>> = OptionalSingleton::default();
        assert_eq!(a, OptionalSingleton::absent());
    }

    #[test]
    fn optional_singleton_bounded_top_wraps_inner_top() {
        let t: OptionalSingleton<MaxDate> = OptionalSingleton::top();
        assert_eq!(t, OptionalSingleton::present(MaxDate::top()));
    }

    #[test]
    fn optional_singleton_bounded_bottom_is_absent() {
        let b: OptionalSingleton<MaxDate> = OptionalSingleton::bottom();
        assert_eq!(b, OptionalSingleton::absent());
    }
}
