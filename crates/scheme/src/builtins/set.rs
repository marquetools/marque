// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use super::merge_sorted_union;
use crate::lattice::{JoinSemilattice, MeetSemilattice};

/// Set-based lattice: the powerset of `T` with join = union, meet =
/// intersect.
///
/// Stored as a sorted `Vec<T>` so two `FlatSet` values compare by
/// membership (ordering is deterministic given `T: Ord`). The wrapper
/// preserves de-duplication and canonical order on every operation.
///
/// Bottom is the empty set; `BoundedLattice::top()` is only well-defined
/// when the total universe of `T` is known, so this type does not
/// implement `BoundedLattice` generically. Schemes that need `top()`
/// instantiate a bounded variant on their own newtype.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct FlatSet<T: Ord + Clone>(Vec<T>);

impl<T: Ord + Clone> FlatSet<T> {
    /// Construct from any iterable; de-duplicated and sorted.
    pub fn from_iter_sorted<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut v: Vec<T> = iter.into_iter().collect();
        v.sort();
        v.dedup();
        Self(v)
    }

    /// Empty set — the lattice bottom.
    #[inline]
    pub fn empty() -> Self {
        Self(Vec::new())
    }

    /// View of the underlying tokens in canonical (sorted) order.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// Whether the set contains a given token.
    #[inline]
    pub fn contains(&self, t: &T) -> bool {
        self.0.binary_search(t).is_ok()
    }

    /// Number of tokens in the set.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the set is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T: Ord + Clone> JoinSemilattice for FlatSet<T> {
    #[inline]
    fn join(&self, other: &Self) -> Self {
        Self(merge_sorted_union(&self.0, &other.0))
    }
}

impl<T: Ord + Clone> MeetSemilattice for FlatSet<T> {
    #[inline]
    fn meet(&self, other: &Self) -> Self {
        let mut out: Vec<T> = Vec::with_capacity(self.0.len().min(other.0.len()));
        let (mut i, mut j) = (0, 0);
        while i < self.0.len() && j < other.0.len() {
            match self.0[i].cmp(&other.0[j]) {
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
                std::cmp::Ordering::Equal => {
                    out.push(self.0[i].clone());
                    i += 1;
                    j += 1;
                }
            }
        }
        Self(out)
    }
}

/// Set-based lattice with **inverted** operations: `join = intersect`,
/// `meet = union`.
///
/// Used for REL TO before tetragraph expansion: the banner releasable
/// countries are the countries releasable by *every* portion — the
/// intersection, not the union. The trait's `join` is what the engine
/// calls during `project`, so for this category it has to shrink the
/// set, not grow it.
///
/// Bottom here is the universe (every country ever); top is the empty
/// set. The bottom/top flip mirrors the operator flip. Because "universe
/// of countries" isn't enumerable at the trait level, we don't implement
/// `BoundedLattice` generically; schemes provide their own bound.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct IntersectSet<T: Ord + Clone>(Vec<T>);

impl<T: Ord + Clone> IntersectSet<T> {
    pub fn from_iter_sorted<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut v: Vec<T> = iter.into_iter().collect();
        v.sort();
        v.dedup();
        Self(v)
    }

    #[inline]
    pub fn empty() -> Self {
        Self(Vec::new())
    }

    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T: Ord + Clone> JoinSemilattice for IntersectSet<T> {
    /// Join = intersection (flipped).
    #[inline]
    fn join(&self, other: &Self) -> Self {
        let mut out: Vec<T> = Vec::with_capacity(self.0.len().min(other.0.len()));
        let (mut i, mut j) = (0, 0);
        while i < self.0.len() && j < other.0.len() {
            match self.0[i].cmp(&other.0[j]) {
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
                std::cmp::Ordering::Equal => {
                    out.push(self.0[i].clone());
                    i += 1;
                    j += 1;
                }
            }
        }
        Self(out)
    }
}

impl<T: Ord + Clone> MeetSemilattice for IntersectSet<T> {
    /// Meet = union (flipped).
    #[inline]
    fn meet(&self, other: &Self) -> Self {
        Self(merge_sorted_union(&self.0, &other.0))
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::lattice::{JoinSemilattice, MeetSemilattice};

    #[test]
    fn flat_set_join_is_sorted_union() {
        let a = FlatSet::from_iter_sorted(vec!["SI", "TK"]);
        let b = FlatSet::from_iter_sorted(vec!["TK", "HCS"]);
        let j = a.join(&b);
        assert_eq!(j.as_slice(), &["HCS", "SI", "TK"]);
    }

    #[test]
    fn flat_set_meet_is_intersection() {
        let a = FlatSet::from_iter_sorted(vec!["SI", "TK"]);
        let b = FlatSet::from_iter_sorted(vec!["TK", "HCS"]);
        assert_eq!(a.meet(&b).as_slice(), &["TK"]);
    }

    #[test]
    fn flat_set_dedupes_on_construction() {
        let a = FlatSet::from_iter_sorted(vec!["SI", "SI", "TK"]);
        assert_eq!(a.as_slice(), &["SI", "TK"]);
    }

    #[test]
    fn flat_set_absorption() {
        let a = FlatSet::from_iter_sorted(vec!["A", "B"]);
        let b = FlatSet::from_iter_sorted(vec!["B", "C"]);
        assert_eq!(a.join(&a.meet(&b)), a);
        assert_eq!(a.meet(&a.join(&b)), a);
    }

    #[test]
    fn flat_set_empty_join_identity() {
        let a = FlatSet::from_iter_sorted(vec!["A"]);
        let e: FlatSet<&str> = FlatSet::empty();
        assert_eq!(a.join(&e), a);
        assert_eq!(e.join(&a), a);
    }

    #[test]
    fn intersect_set_join_is_intersection() {
        let a = IntersectSet::from_iter_sorted(vec!["USA", "GBR", "CAN"]);
        let b = IntersectSet::from_iter_sorted(vec!["USA", "GBR", "DEU"]);
        assert_eq!(a.join(&b).as_slice(), &["GBR", "USA"]);
    }

    #[test]
    fn intersect_set_meet_is_union() {
        let a = IntersectSet::from_iter_sorted(vec!["USA"]);
        let b = IntersectSet::from_iter_sorted(vec!["GBR"]);
        assert_eq!(a.meet(&b).as_slice(), &["GBR", "USA"]);
    }

    #[test]
    fn flat_set_empty_accessors() {
        let e: FlatSet<&str> = FlatSet::empty();
        assert!(e.is_empty());
        assert_eq!(e.len(), 0);
        assert_eq!(e.as_slice(), &[] as &[&str]);
        assert!(!e.contains(&"SI"));
    }

    #[test]
    fn flat_set_populated_accessors() {
        let a = FlatSet::from_iter_sorted(vec!["SI", "TK"]);
        assert!(!a.is_empty());
        assert_eq!(a.len(), 2);
        assert!(a.contains(&"SI"));
        assert!(a.contains(&"TK"));
        assert!(!a.contains(&"HCS"));
    }

    #[test]
    fn flat_set_default_is_empty() {
        let a: FlatSet<u32> = FlatSet::default();
        assert!(a.is_empty());
    }

    #[test]
    fn flat_set_join_left_suffix_remainder() {
        let a = FlatSet::from_iter_sorted(vec!["A", "Z"]);
        let b = FlatSet::from_iter_sorted(vec!["A"]);
        assert_eq!(a.join(&b).as_slice(), &["A", "Z"]);
    }

    #[test]
    fn flat_set_join_right_suffix_remainder() {
        let a = FlatSet::from_iter_sorted(vec!["A"]);
        let b = FlatSet::from_iter_sorted(vec!["A", "Z"]);
        assert_eq!(a.join(&b).as_slice(), &["A", "Z"]);
    }

    #[test]
    fn flat_set_meet_empty_when_disjoint() {
        let a = FlatSet::from_iter_sorted(vec!["A", "B"]);
        let b = FlatSet::from_iter_sorted(vec!["C", "D"]);
        assert!(a.meet(&b).is_empty());
    }

    #[test]
    fn intersect_set_empty_accessors() {
        let e: IntersectSet<&str> = IntersectSet::empty();
        assert!(e.is_empty());
        assert_eq!(e.len(), 0);
        assert_eq!(e.as_slice(), &[] as &[&str]);
    }

    #[test]
    fn intersect_set_populated_accessors() {
        let a = IntersectSet::from_iter_sorted(vec!["USA", "GBR"]);
        assert!(!a.is_empty());
        assert_eq!(a.len(), 2);
        assert_eq!(a.as_slice(), &["GBR", "USA"]);
    }

    #[test]
    fn intersect_set_default_is_empty() {
        let a: IntersectSet<u32> = IntersectSet::default();
        assert!(a.is_empty());
    }

    #[test]
    fn intersect_set_meet_unions_with_left_suffix() {
        let a = IntersectSet::from_iter_sorted(vec!["A", "Z"]);
        let b = IntersectSet::from_iter_sorted(vec!["A"]);
        assert_eq!(a.meet(&b).as_slice(), &["A", "Z"]);
    }

    #[test]
    fn intersect_set_meet_unions_with_right_suffix() {
        let a = IntersectSet::from_iter_sorted(vec!["A"]);
        let b = IntersectSet::from_iter_sorted(vec!["A", "Z"]);
        assert_eq!(a.meet(&b).as_slice(), &["A", "Z"]);
    }

    #[test]
    fn intersect_set_meet_interleaves_less_branch() {
        let a = IntersectSet::from_iter_sorted(vec!["A", "M"]);
        let b = IntersectSet::from_iter_sorted(vec!["Z"]);
        assert_eq!(a.meet(&b).as_slice(), &["A", "M", "Z"]);
    }

    #[test]
    fn intersect_set_join_empty_on_disjoint() {
        let a = IntersectSet::from_iter_sorted(vec!["A"]);
        let b = IntersectSet::from_iter_sorted(vec!["B"]);
        assert!(a.join(&b).is_empty());
    }
}
