// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::lattice::{JoinSemilattice, MeetSemilattice};

/// Total-order lattice with `join = max`.
///
/// Use for any category with a natural "most restrictive" semantics
/// expressible as a total order — the classification ladder, date
/// ordering, sensitivity levels. `T` must implement [`Ord`]; the join
/// picks the greater element, the meet picks the lesser.
///
/// This type is *not* `BoundedLattice` generically — a bounded
/// implementation requires knowing `T`'s `MIN` and `MAX`, which the
/// standard library only exposes on specific numeric types. For typed
/// scheme enums, implement `BoundedJoinSemilattice` and
/// `BoundedMeetSemilattice` on a local newtype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrdMax<T: Ord + Clone>(pub T);

impl<T: Ord + Clone> JoinSemilattice for OrdMax<T> {
    #[inline]
    fn join(&self, other: &Self) -> Self {
        if self.0 >= other.0 {
            self.clone()
        } else {
            other.clone()
        }
    }
}

impl<T: Ord + Clone> MeetSemilattice for OrdMax<T> {
    #[inline]
    fn meet(&self, other: &Self) -> Self {
        if self.0 <= other.0 {
            self.clone()
        } else {
            other.clone()
        }
    }
}

/// Total-order lattice with `join = min` (inverted semantics).
///
/// Use for "most specific" reductions — CAPCO declassification
/// exemptions where the entry with the longest default duration wins,
/// treated as the smaller value on a restrictiveness axis. `join` picks
/// the lesser element; `meet` picks the greater.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrdMin<T: Ord + Clone>(pub T);

impl<T: Ord + Clone> JoinSemilattice for OrdMin<T> {
    #[inline]
    fn join(&self, other: &Self) -> Self {
        if self.0 <= other.0 {
            self.clone()
        } else {
            other.clone()
        }
    }
}

impl<T: Ord + Clone> MeetSemilattice for OrdMin<T> {
    #[inline]
    fn meet(&self, other: &Self) -> Self {
        if self.0 >= other.0 {
            self.clone()
        } else {
            other.clone()
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::lattice::{JoinSemilattice, MeetSemilattice};

    #[test]
    fn ord_max_join_picks_greater() {
        assert_eq!(OrdMax(3_u32).join(&OrdMax(7)), OrdMax(7));
    }

    #[test]
    fn ord_max_meet_picks_lesser() {
        assert_eq!(OrdMax(3_u32).meet(&OrdMax(7)), OrdMax(3));
    }

    #[test]
    fn ord_max_idempotent() {
        let v = OrdMax(5_u32);
        assert_eq!(v.join(&v), v);
        assert_eq!(v.meet(&v), v);
    }

    #[test]
    fn ord_max_absorption() {
        let a = OrdMax(3_u32);
        let b = OrdMax(7_u32);
        assert_eq!(a.join(&a.meet(&b)), a);
        assert_eq!(a.meet(&a.join(&b)), a);
    }

    #[test]
    fn ord_min_join_picks_lesser() {
        assert_eq!(OrdMin(3_u32).join(&OrdMin(7)), OrdMin(3));
    }

    #[test]
    fn ord_min_meet_picks_greater() {
        assert_eq!(OrdMin(3_u32).meet(&OrdMin(7)), OrdMin(7));
    }

    #[test]
    fn ord_max_join_equal_operands_picks_self() {
        let v = OrdMax(5_u32);
        assert_eq!(v.join(&v.clone()), v);
    }

    #[test]
    fn ord_max_meet_equal_operands_picks_self() {
        let v = OrdMax(5_u32);
        assert_eq!(v.meet(&v.clone()), v);
    }

    #[test]
    fn ord_min_join_equal_operands_picks_self() {
        let v = OrdMin(5_u32);
        assert_eq!(v.join(&v.clone()), v);
    }

    #[test]
    fn ord_min_meet_equal_operands_picks_self() {
        let v = OrdMin(5_u32);
        assert_eq!(v.meet(&v.clone()), v);
    }

    #[test]
    fn ord_min_laws_smoke() {
        assert_eq!(OrdMin(3_u32).join(&OrdMin(7_u32)), OrdMin(3));
        assert_eq!(OrdMin(7_u32).join(&OrdMin(3_u32)), OrdMin(3));
        assert_eq!(OrdMin(3_u32).meet(&OrdMin(7_u32)), OrdMin(7));
        assert_eq!(OrdMin(7_u32).meet(&OrdMin(3_u32)), OrdMin(7));
    }
}
