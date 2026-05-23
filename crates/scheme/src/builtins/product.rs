// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::lattice::{
    BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice,
};

/// Pair semilattice: `Product(a, b)` joins component-wise.
///
/// Trivially generalizes to n-ary products via nested `Product`s. For
/// CAPCO's ten-category marking, we use a struct instead of a tower of
/// `Product`s (readability), but this constructor is the right shape
/// for shallow composition.
///
/// The struct has no algebraic constraint on `A` and `B` — it
/// implements [`JoinSemilattice`] when both factors do, and
/// [`MeetSemilattice`] when both factors do. This means `Product<A, B>`
/// is a full [`crate::lattice::Lattice`] when both factors are full
/// lattices, and a join-semilattice when either factor is join-only.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Product<A, B>(pub A, pub B);

impl<A, B> Default for Product<A, B>
where
    A: Default,
    B: Default,
{
    fn default() -> Self {
        Self(A::default(), B::default())
    }
}

impl<A: JoinSemilattice, B: JoinSemilattice> JoinSemilattice for Product<A, B> {
    #[inline]
    fn join(&self, other: &Self) -> Self {
        Self(self.0.join(&other.0), self.1.join(&other.1))
    }
}

impl<A: MeetSemilattice, B: MeetSemilattice> MeetSemilattice for Product<A, B> {
    #[inline]
    fn meet(&self, other: &Self) -> Self {
        Self(self.0.meet(&other.0), self.1.meet(&other.1))
    }
}

impl<A: BoundedJoinSemilattice, B: BoundedJoinSemilattice> BoundedJoinSemilattice
    for Product<A, B>
{
    fn bottom() -> Self {
        Self(A::bottom(), B::bottom())
    }
}

impl<A: BoundedMeetSemilattice, B: BoundedMeetSemilattice> BoundedMeetSemilattice
    for Product<A, B>
{
    fn top() -> Self {
        Self(A::top(), B::top())
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::builtins::MaxDate;
    use crate::builtins::OrdMax;
    use crate::lattice::{
        BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice,
    };

    #[test]
    fn product_join_is_componentwise() {
        let a = Product(OrdMax(3_u32), OrdMax(10_u32));
        let b = Product(OrdMax(7_u32), OrdMax(5_u32));
        assert_eq!(a.join(&b), Product(OrdMax(7), OrdMax(10)));
    }

    #[test]
    fn product_meet_is_componentwise() {
        let a = Product(OrdMax(3_u32), OrdMax(10_u32));
        let b = Product(OrdMax(7_u32), OrdMax(5_u32));
        assert_eq!(a.meet(&b), Product(OrdMax(3), OrdMax(5)));
    }

    #[test]
    fn product_default_uses_inner_defaults() {
        let p: Product<MaxDate, MaxDate> = Product::default();
        assert_eq!(p, Product(MaxDate::default(), MaxDate::default()));
    }

    #[test]
    fn product_bounded_top_uses_inner_tops() {
        let t: Product<MaxDate, MaxDate> = Product::top();
        assert_eq!(t, Product(MaxDate::top(), MaxDate::top()));
    }

    #[test]
    fn product_bounded_bottom_uses_inner_bottoms() {
        let b: Product<MaxDate, MaxDate> = Product::bottom();
        assert_eq!(b, Product(MaxDate::bottom(), MaxDate::bottom()));
    }
}
