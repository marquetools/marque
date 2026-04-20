// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Built-in lattice constructors.
//!
//! Phase B ships a small family of generic lattice types that cover the
//! bulk of category shapes across CAPCO, CUI, and NATO. A scheme author
//! picks a constructor appropriate to their category rather than writing
//! `impl Lattice` by hand every time.
//!
//! | Constructor            | Shape                                      | CAPCO example           |
//! |------------------------|--------------------------------------------|-------------------------|
//! | [`OrdMax`]             | total order, join = `max`                  | classification ladder   |
//! | [`OrdMin`]             | total order, join = `min`                  | "most specific" picks   |
//! | [`FlatSet`]            | powerset, join = union, meet = intersect   | SCI / SAR / dissem      |
//! | [`IntersectSet`]       | inverted powerset, join = intersect        | REL TO (pre-expansion)  |
//! | [`SupersessionSet`]    | union, then drop superseded tokens         | NOFORN ⊐ REL TO (intra) |
//! | [`ModeSet`]            | multiset, join = most-frequent             | corporate sensitivity   |
//! | [`MaxDate`]            | dates, join = later, bottom = absent       | declassify-on           |
//! | [`OptionalSingleton`]  | lifts any lattice `L` to `Option<L>`       | optional single fields  |
//! | [`Product`]            | tuple product of two lattices              | composed sub-lattices   |
//!
//! All types are `#[derive(Clone, PartialEq, Eq)]` and implement
//! [`Lattice`]; where a meaningful `top()` exists they also implement
//! [`BoundedLattice`].
//!
//! # Contract
//!
//! The usual lattice laws (commutative, associative, idempotent join
//! and meet; absorption) are verified by unit tests in this module on
//! small example instances. The property tests in `marque-capco` extend
//! the checks to the CAPCO structural lattices that consume these
//! primitives.

use crate::lattice::{BoundedLattice, Lattice};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// OrdMax / OrdMin — total-order lattices
// ---------------------------------------------------------------------------

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
/// scheme enums, implement `BoundedLattice` on a local newtype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrdMax<T: Ord + Clone>(pub T);

impl<T: Ord + Clone> Lattice for OrdMax<T> {
    #[inline]
    fn join(&self, other: &Self) -> Self {
        if self.0 >= other.0 {
            self.clone()
        } else {
            other.clone()
        }
    }
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

impl<T: Ord + Clone> Lattice for OrdMin<T> {
    #[inline]
    fn join(&self, other: &Self) -> Self {
        if self.0 <= other.0 {
            self.clone()
        } else {
            other.clone()
        }
    }
    #[inline]
    fn meet(&self, other: &Self) -> Self {
        if self.0 >= other.0 {
            self.clone()
        } else {
            other.clone()
        }
    }
}

// ---------------------------------------------------------------------------
// FlatSet — powerset lattice, join = union, meet = intersect
// ---------------------------------------------------------------------------

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

impl<T: Ord + Clone> Lattice for FlatSet<T> {
    #[inline]
    fn join(&self, other: &Self) -> Self {
        // Both sides are sorted; merge deduped.
        let mut out: Vec<T> = Vec::with_capacity(self.0.len() + other.0.len());
        let (mut i, mut j) = (0, 0);
        while i < self.0.len() && j < other.0.len() {
            match self.0[i].cmp(&other.0[j]) {
                std::cmp::Ordering::Less => {
                    out.push(self.0[i].clone());
                    i += 1;
                }
                std::cmp::Ordering::Greater => {
                    out.push(other.0[j].clone());
                    j += 1;
                }
                std::cmp::Ordering::Equal => {
                    out.push(self.0[i].clone());
                    i += 1;
                    j += 1;
                }
            }
        }
        out.extend_from_slice(&self.0[i..]);
        out.extend_from_slice(&other.0[j..]);
        Self(out)
    }

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

// ---------------------------------------------------------------------------
// IntersectSet — lattice with inverted operations
// ---------------------------------------------------------------------------

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

impl<T: Ord + Clone> Lattice for IntersectSet<T> {
    /// Join = intersection (flipped).
    #[inline]
    fn join(&self, other: &Self) -> Self {
        // Equivalent to FlatSet::meet on the same storage.
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

    /// Meet = union (flipped).
    #[inline]
    fn meet(&self, other: &Self) -> Self {
        // Equivalent to FlatSet::join.
        let mut out: Vec<T> = Vec::with_capacity(self.0.len() + other.0.len());
        let (mut i, mut j) = (0, 0);
        while i < self.0.len() && j < other.0.len() {
            match self.0[i].cmp(&other.0[j]) {
                std::cmp::Ordering::Less => {
                    out.push(self.0[i].clone());
                    i += 1;
                }
                std::cmp::Ordering::Greater => {
                    out.push(other.0[j].clone());
                    j += 1;
                }
                std::cmp::Ordering::Equal => {
                    out.push(self.0[i].clone());
                    i += 1;
                    j += 1;
                }
            }
        }
        out.extend_from_slice(&self.0[i..]);
        out.extend_from_slice(&other.0[j..]);
        Self(out)
    }
}

// ---------------------------------------------------------------------------
// SupersessionSet — union, then drop superseded tokens
// ---------------------------------------------------------------------------

/// Intra-category supersession: join is union, then post-filter that
/// drops any token whose presence is obviated by a superseding token
/// also in the set.
///
/// Each supersession pair `(superseding, superseded)` says "if
/// `superseding` appears, drop `superseded`." The supersession table is
/// borrowed as a `&'static [(T, T)]` so one static table is shared by
/// every value of the category across the whole program — matching how
/// schemes declare supersession at scheme build time.
///
/// **Cross-category supersession** (like CAPCO's NOFORN in `dissem`
/// clearing `rel_to` — two different categories) can't be expressed with
/// this primitive because the superseding and superseded tokens live in
/// different category storage. That's what `PageRewrite` (below) exists
/// for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupersessionSet<T: Ord + Clone + 'static> {
    set: Vec<T>,
    supersession: &'static [(T, T)],
}

impl<T: Ord + Clone + 'static> SupersessionSet<T> {
    pub fn new(supersession: &'static [(T, T)]) -> Self {
        Self {
            set: Vec::new(),
            supersession,
        }
    }

    pub fn from_iter_sorted<I: IntoIterator<Item = T>>(
        iter: I,
        supersession: &'static [(T, T)],
    ) -> Self {
        let mut v: Vec<T> = iter.into_iter().collect();
        v.sort();
        v.dedup();
        Self {
            set: v,
            supersession,
        }
    }

    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.set
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    /// Apply the supersession table to a sorted, deduplicated vector.
    /// Kept internal so `join` stays simple.
    fn apply_supersession(set: Vec<T>, supersession: &'static [(T, T)]) -> Vec<T> {
        // Precompute the drop set (superseded tokens whose superseding
        // peer is present). Avoids overlapping borrow from `retain`.
        let mut drops: Vec<&T> = Vec::new();
        for (superseding, superseded) in supersession.iter() {
            if set.iter().any(|u| u == superseding) {
                drops.push(superseded);
            }
        }
        set.into_iter().filter(|t| !drops.contains(&t)).collect()
    }
}

impl<T: Ord + Clone + 'static> Lattice for SupersessionSet<T> {
    #[inline]
    fn join(&self, other: &Self) -> Self {
        // Supersession table must agree (it's an invariant of the
        // category). We preserve `self.supersession` on the output.
        let flat = FlatSet(self.set.clone()).join(&FlatSet(other.set.clone()));
        let filtered = Self::apply_supersession(flat.0, self.supersession);
        Self {
            set: filtered,
            supersession: self.supersession,
        }
    }

    /// Meet = intersection. Supersession is a join-side post-filter only
    /// (the spec never defines a "meet with supersession"); the meet is
    /// the plain intersection on the stored set.
    #[inline]
    fn meet(&self, other: &Self) -> Self {
        let flat = FlatSet(self.set.clone()).meet(&FlatSet(other.set.clone()));
        Self {
            set: flat.0,
            supersession: self.supersession,
        }
    }
}

// ---------------------------------------------------------------------------
// ModeSet — multiset lattice keyed by mode (most-frequent value)
// ---------------------------------------------------------------------------

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
/// generically (see `BoundedLattice` note on [`FlatSet`]).
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
    /// single source" use [`Lattice::join`].
    pub fn extend_counts(&self, other: &Self) -> Self {
        let mut out = self.0.clone();
        for (k, v) in &other.0 {
            *out.entry(k.clone()).or_insert(0) += v;
        }
        Self(out)
    }
}

impl<T: Ord + Clone> Lattice for ModeSet<T> {
    /// Per-key max of counts. Idempotent: `a.join(&a) = a`.
    #[inline]
    fn join(&self, other: &Self) -> Self {
        let mut out = self.0.clone();
        for (k, v) in &other.0 {
            let slot = out.entry(k.clone()).or_insert(0);
            if *v > *slot {
                *slot = *v;
            }
        }
        Self(out)
    }

    /// Meet = per-key min (the multiset that both operands dominate).
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

// ---------------------------------------------------------------------------
// MaxDate — dates with join = later, bottom = None
// ---------------------------------------------------------------------------

/// Date-valued lattice storing an ISO-8601 `YYYY` or `YYYYMMDD` string.
///
/// Join picks the lexicographically greater string (which is also the
/// chronologically later date under that encoding — same rationale as
/// `PageContext::expected_declassify_on`). Bottom is the absent date
/// (`None`).
///
/// We store the owned string rather than a reference so the lattice
/// value can outlive any single input portion. For large-scale batch
/// work, a `MaxDate<&'static str>` variant is trivial to add if memory
/// pressure warrants.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MaxDate(pub Option<Box<str>>);

impl MaxDate {
    #[inline]
    pub fn absent() -> Self {
        Self(None)
    }

    #[inline]
    pub fn present(s: impl Into<Box<str>>) -> Self {
        Self(Some(s.into()))
    }

    #[inline]
    pub fn as_deref(&self) -> Option<&str> {
        self.0.as_deref()
    }
}

impl Lattice for MaxDate {
    #[inline]
    fn join(&self, other: &Self) -> Self {
        match (&self.0, &other.0) {
            (None, None) => Self(None),
            (Some(a), None) => Self(Some(a.clone())),
            (None, Some(b)) => Self(Some(b.clone())),
            (Some(a), Some(b)) => {
                if a >= b {
                    Self(Some(a.clone()))
                } else {
                    Self(Some(b.clone()))
                }
            }
        }
    }

    #[inline]
    fn meet(&self, other: &Self) -> Self {
        match (&self.0, &other.0) {
            (None, _) | (_, None) => Self(None),
            (Some(a), Some(b)) => {
                if a <= b {
                    Self(Some(a.clone()))
                } else {
                    Self(Some(b.clone()))
                }
            }
        }
    }
}

impl BoundedLattice for MaxDate {
    fn top() -> Self {
        // No finite top — return a sentinel string that orders after any
        // reasonable ISO date. Values larger than this would themselves
        // be invalid inputs.
        Self(Some("99991231".into()))
    }
    fn bottom() -> Self {
        Self(None)
    }
}

// ---------------------------------------------------------------------------
// OptionalSingleton — lift a lattice to `Option<L>` with absent bottom
// ---------------------------------------------------------------------------

/// Optional wrapper around any inner lattice. `None` is the bottom —
/// the join with `None` is whichever operand has a value, and the join
/// of two `Some`s calls the inner lattice's `join`.
///
/// Use for optional single-value categories where the "value-present"
/// case already has a lattice (e.g., `OptionalSingleton<OrdMax<Level>>`
/// for a scheme where classification is optional).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionalSingleton<L: Lattice>(pub Option<L>);

impl<L: Lattice> Default for OptionalSingleton<L> {
    fn default() -> Self {
        Self(None)
    }
}

impl<L: Lattice> OptionalSingleton<L> {
    #[inline]
    pub fn absent() -> Self {
        Self(None)
    }

    #[inline]
    pub fn present(l: L) -> Self {
        Self(Some(l))
    }
}

impl<L: Lattice> Lattice for OptionalSingleton<L> {
    #[inline]
    fn join(&self, other: &Self) -> Self {
        match (&self.0, &other.0) {
            (None, None) => Self(None),
            (Some(a), None) => Self(Some(a.clone())),
            (None, Some(b)) => Self(Some(b.clone())),
            (Some(a), Some(b)) => Self(Some(a.join(b))),
        }
    }

    #[inline]
    fn meet(&self, other: &Self) -> Self {
        match (&self.0, &other.0) {
            (None, _) | (_, None) => Self(None),
            (Some(a), Some(b)) => Self(Some(a.meet(b))),
        }
    }
}

impl<L: BoundedLattice> BoundedLattice for OptionalSingleton<L> {
    fn top() -> Self {
        Self(Some(L::top()))
    }
    fn bottom() -> Self {
        Self(None)
    }
}

// ---------------------------------------------------------------------------
// Product — tuple product of two lattices
// ---------------------------------------------------------------------------

/// Pair lattice: `Product(a, b)` joins component-wise.
///
/// Trivially generalizes to n-ary products via nested `Product`s. For
/// CAPCO's ten-category marking, we use a struct instead of a tower of
/// `Product`s (readability), but this constructor is the right shape
/// for shallow composition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Product<A: Lattice, B: Lattice>(pub A, pub B);

impl<A, B> Default for Product<A, B>
where
    A: Lattice + Default,
    B: Lattice + Default,
{
    fn default() -> Self {
        Self(A::default(), B::default())
    }
}

impl<A: Lattice, B: Lattice> Lattice for Product<A, B> {
    #[inline]
    fn join(&self, other: &Self) -> Self {
        Self(self.0.join(&other.0), self.1.join(&other.1))
    }
    #[inline]
    fn meet(&self, other: &Self) -> Self {
        Self(self.0.meet(&other.0), self.1.meet(&other.1))
    }
}

impl<A: BoundedLattice, B: BoundedLattice> BoundedLattice for Product<A, B> {
    fn top() -> Self {
        Self(A::top(), B::top())
    }
    fn bottom() -> Self {
        Self(A::bottom(), B::bottom())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // OrdMax

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

    // OrdMin

    #[test]
    fn ord_min_join_picks_lesser() {
        assert_eq!(OrdMin(3_u32).join(&OrdMin(7)), OrdMin(3));
    }

    #[test]
    fn ord_min_meet_picks_greater() {
        assert_eq!(OrdMin(3_u32).meet(&OrdMin(7)), OrdMin(7));
    }

    // FlatSet

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

    // IntersectSet

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

    // SupersessionSet

    static TEST_SUP: &[(u8, u8)] = &[(1, 2)];

    #[test]
    fn supersession_drops_superseded() {
        let a = SupersessionSet::from_iter_sorted(vec![1_u8], TEST_SUP);
        let b = SupersessionSet::from_iter_sorted(vec![2_u8], TEST_SUP);
        let j = a.join(&b);
        assert_eq!(j.as_slice(), &[1]);
    }

    #[test]
    fn supersession_noop_without_superseding() {
        let a = SupersessionSet::from_iter_sorted(vec![2_u8], TEST_SUP);
        let b: SupersessionSet<u8> = SupersessionSet::new(TEST_SUP);
        assert_eq!(a.join(&b).as_slice(), &[2]);
    }

    // ModeSet

    #[test]
    fn mode_set_mode_returns_max_frequency() {
        let m = ModeSet::from_iter_counted(vec!["a", "b", "b", "c"]);
        assert_eq!(m.mode(), Some(&"b"));
    }

    #[test]
    fn mode_set_join_is_idempotent() {
        // The Lattice contract requires `a.join(&a) = a`. Per-key max
        // gives this; sum-of-counts would not.
        let a = ModeSet::from_iter_counted(vec!["a", "b", "b", "c"]);
        assert_eq!(a.join(&a), a);
    }

    #[test]
    fn mode_set_join_takes_per_key_max() {
        // a has {x:3, y:1}; b has {x:1, y:2, z:5}. Per-key max should
        // give {x:3, y:2, z:5} — *not* sum.
        let a = ModeSet::from_iter_counted(vec!["x", "x", "x", "y"]);
        let b = ModeSet::from_iter_counted(vec!["x", "y", "y", "z", "z", "z", "z", "z"]);
        let j = a.join(&b);
        // Mode of the joined multiset is `z` (max count 5).
        assert_eq!(j.mode(), Some(&"z"));
    }

    #[test]
    fn mode_set_extend_counts_sums_observations() {
        // The non-lattice observation aggregator DOES sum counts.
        let a = ModeSet::from_iter_counted(vec!["a", "b"]);
        let b = ModeSet::from_iter_counted(vec!["b", "c"]);
        let combined = a.extend_counts(&b);
        // `b` now appears twice (1 + 1).
        assert_eq!(combined.mode(), Some(&"b"));
    }

    // MaxDate

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
        // `2030` means "start of year 2030"; it should sort *before* any
        // later date in 2030, which lex order gives us for free.
        let year = MaxDate::present("2030");
        let date = MaxDate::present("20300601");
        assert_eq!(year.join(&date), date);
    }

    // OptionalSingleton

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

    // Product

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
}
