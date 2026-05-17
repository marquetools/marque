// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Built-in lattice constructors.
//!
//! Phase B ships a small family of generic lattice types that cover the
//! bulk of category shapes across CAPCO, CUI, and NATO. A scheme author
//! picks a constructor appropriate to their category rather than writing
//! `impl JoinSemilattice` / `impl MeetSemilattice` by hand every time.
//!
//! | Constructor            | Shape                                      | CAPCO example           | Laws                |
//! |------------------------|--------------------------------------------|-------------------------|---------------------|
//! | [`OrdMax`]             | total order, join = `max`                  | classification ladder   | Full lattice        |
//! | [`OrdMin`]             | total order, join = `min`                  | "most specific" picks   | Full lattice        |
//! | [`FlatSet`]            | powerset, join = union, meet = intersect   | SCI / SAR / dissem      | Full lattice        |
//! | [`IntersectSet`]       | inverted powerset, join = intersect        | REL TO (pre-expansion)  | Full lattice        |
//! | [`SupersessionSet`]    | union, then drop superseded tokens         | NOFORN ⊐ REL TO (intra) | **Join-only**       |
//! | [`ModeSet`]            | multiset, join = most-frequent             | corporate sensitivity   | Full lattice        |
//! | [`MaxDate`]            | dates, join = later, bottom = absent       | declassify-on           | Full lattice        |
//! | [`OptionalSingleton`]  | lifts any `JoinSemilattice` to `Option<L>` | optional single fields  | Mirrors inner type  |
//! | [`Product`]            | tuple product of two semilattices          | composed sub-lattices   | Mirrors inner types |
//!
//! `SupersessionSet` implements only [`JoinSemilattice`] — the supersession
//! overlay is a join-side post-filter and the meet direction is
//! non-idempotent on inputs that contain both a dominated token and its
//! dominator. See the type-level doc for the counterexample.
//!
//! `OptionalSingleton<L>` and `Product<A, B>` mirror their inner type(s):
//! if the inner type(s) are full lattices, the wrapper is a full lattice
//! (via the blanket impl); if the inner type(s) are join-only, the wrapper
//! is join-only.
//!
//! # Contract
//!
//! The usual lattice laws (commutative, associative, idempotent join
//! and meet; absorption) are verified by unit tests in this module on
//! small example instances. The property tests in `marque-capco` extend
//! the checks to the CAPCO structural lattices that consume these
//! primitives.

use crate::lattice::{
    BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice,
};
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

impl<T: Ord + Clone> JoinSemilattice for FlatSet<T> {
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

impl<T: Ord + Clone> JoinSemilattice for IntersectSet<T> {
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
}

impl<T: Ord + Clone> MeetSemilattice for IntersectSet<T> {
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
// SupersessionSet — union, then drop superseded tokens (join-only)
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
/// # Join-semilattice only
///
/// `SupersessionSet` implements [`JoinSemilattice`] but NOT
/// [`MeetSemilattice`]. The supersession overlay is a join-side
/// post-filter — it is monotone with respect to union but not with
/// respect to set-inclusion, so the dual absorption law
/// `a ⊓ (a ⊔ b) = a` fails whenever `a` contains a dominated token
/// and `b` contains its dominator. Counterexample:
/// `a = {R}`, `b = {N}`, supersession `= [(N→R)]`:
/// `a ⊔ b = {N}` (R dropped), so `a ⊓ {N} = {} ≠ a`.
///
/// Additionally, `from_iter_sorted` does not apply the overlay on
/// construction, so user-constructed inputs containing both a dominated
/// token and its dominator can fail join-idempotence
/// (`a.join(&a) ≠ a`) if the overlay was not pre-applied.
///
/// **Cross-category supersession** (like CAPCO's NOFORN in `dissem`
/// clearing `rel_to` — two different categories) can't be expressed
/// with this primitive because the superseding and superseded tokens
/// live in different category storage. That's what `PageRewrite`
/// (in `marque-scheme`) exists for.
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

    /// Construct from an iterable; de-duplicated and sorted. The supersession
    /// overlay is **not** applied at construction time — callers that need
    /// canonical state after construction should call
    /// `SupersessionSet::new` and then `join` the values in.
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

impl<T: Ord + Clone + 'static> JoinSemilattice for SupersessionSet<T> {
    /// Join = union then apply supersession. Both operands must carry
    /// the same supersession table (pointer equality) — schemes
    /// construct a single `&'static [(T, T)]` per category and use it
    /// everywhere. The `debug_assert!` catches mis-wired test setups
    /// that combine two sets with different tables (which would
    /// silently produce order-dependent results in release builds
    /// because the output carries `self.supersession` unconditionally).
    #[inline]
    fn join(&self, other: &Self) -> Self {
        debug_assert!(
            std::ptr::eq(self.supersession, other.supersession),
            "SupersessionSet::join called on operands with different supersession tables; \
             the lattice laws (commutativity, associativity) only hold when both sides share \
             the same category table"
        );
        let flat = FlatSet(self.set.clone()).join(&FlatSet(other.set.clone()));
        let filtered = Self::apply_supersession(flat.0, self.supersession);
        Self {
            set: filtered,
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
    /// Per-key max of counts. Idempotent: `a.join(&a) = a`.
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

    /// Validate a candidate date string.
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
    /// Sentinel top. `99991231` is strictly greater than every
    /// `[0-9]{4}` and `[0-9]{8}` value under lex order, so the
    /// `top ⊔ a = top` law holds for every validly-constructed
    /// `MaxDate`. See the type-level docs for the validation gate.
    fn top() -> Self {
        Self {
            inner: Some("99991231".into()),
        }
    }
}

// `MaxDate` gets `Lattice` and `BoundedLattice` via the blanket impls.

// ---------------------------------------------------------------------------
// OptionalSingleton — lift a semilattice to `Option<L>` with absent bottom
// ---------------------------------------------------------------------------

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
/// This means `OptionalSingleton<L>` is a full [`Lattice`] when `L` is
/// a full lattice, and a join-semilattice when `L` is join-only.
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

impl<L: MeetSemilattice + JoinSemilattice> MeetSemilattice for OptionalSingleton<L> {
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

impl<L: MeetSemilattice + JoinSemilattice + BoundedMeetSemilattice> BoundedMeetSemilattice
    for OptionalSingleton<L>
{
    fn top() -> Self {
        Self(Some(L::top()))
    }
}

// `OptionalSingleton<L>` gets `Lattice` / `BoundedLattice` via blanket impls
// when `L` satisfies both halves.

// ---------------------------------------------------------------------------
// Product — tuple product of two semilattices
// ---------------------------------------------------------------------------

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
/// is a full [`Lattice`] when both factors are full lattices, and a
/// join-semilattice when either factor is join-only.
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

impl<A: JoinSemilattice + BoundedJoinSemilattice, B: JoinSemilattice + BoundedJoinSemilattice>
    BoundedJoinSemilattice for Product<A, B>
{
    fn bottom() -> Self {
        Self(A::bottom(), B::bottom())
    }
}

impl<A: MeetSemilattice + BoundedMeetSemilattice, B: MeetSemilattice + BoundedMeetSemilattice>
    BoundedMeetSemilattice for Product<A, B>
{
    fn top() -> Self {
        Self(A::top(), B::top())
    }
}

// `Product<A,B>` gets `Lattice` / `BoundedLattice` via blanket impls
// when both factors satisfy both halves.

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
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

    #[test]
    fn supersession_is_join_semilattice_only() {
        // Compile-time gate: SupersessionSet satisfies JoinSemilattice but not
        // MeetSemilattice. This test confirms the type-system enforcement.
        fn _assert_join<T: JoinSemilattice>() {}
        _assert_join::<SupersessionSet<u8>>();
        // The following would fail to compile (expected):
        // fn _assert_lattice<T: Lattice>() {}
        // _assert_lattice::<SupersessionSet<u8>>();
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
        assert!(MaxDate::try_present("203O").is_none()); // O, not 0
    }

    #[test]
    #[should_panic(expected = "MaxDate::present: expected YYYY or YYYYMMDD")]
    fn max_date_present_panics_on_invalid() {
        // Confirms the invariant gate: `present` rejects non-digit
        // strings that would otherwise sort after the top() sentinel.
        let _ = MaxDate::present("ZZZZ");
    }

    #[test]
    fn max_date_top_dominates_every_valid_value() {
        // The BoundedLattice law `top ⊔ a = top` — verified on every
        // representative valid input. This was the Phase B invariant
        // at risk before the field became private + validated.
        let t = MaxDate::top();
        for s in ["2000", "20001231", "20991231", "99981231", "99991230"] {
            let a = MaxDate::present(s);
            assert_eq!(t.join(&a), t, "top ⊔ {s} must equal top");
        }
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

    // -----------------------------------------------------------------
    // Additional coverage — constructors, accessors, equal-branch paths
    // -----------------------------------------------------------------

    // OrdMax / OrdMin — equal-operand and commutative branches

    #[test]
    fn ord_max_join_equal_operands_picks_self() {
        // Exercises the `self.0 >= other.0` branch when they're equal.
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
        // Join = min; meet = max. Quick spot check across asymmetric pairs.
        assert_eq!(OrdMin(3_u32).join(&OrdMin(7_u32)), OrdMin(3));
        assert_eq!(OrdMin(7_u32).join(&OrdMin(3_u32)), OrdMin(3));
        assert_eq!(OrdMin(3_u32).meet(&OrdMin(7_u32)), OrdMin(7));
        assert_eq!(OrdMin(7_u32).meet(&OrdMin(3_u32)), OrdMin(7));
    }

    // FlatSet — accessors and edge branches

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
        // Left has a trailing element that falls after everything in right.
        let a = FlatSet::from_iter_sorted(vec!["A", "Z"]);
        let b = FlatSet::from_iter_sorted(vec!["A"]);
        assert_eq!(a.join(&b).as_slice(), &["A", "Z"]);
    }

    #[test]
    fn flat_set_join_right_suffix_remainder() {
        // Right has a trailing element that falls after everything in left.
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

    // IntersectSet — accessors and edge branches

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
        // `as_slice` is sorted canonically.
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
        // Triggers the `Ordering::Less` arm inside IntersectSet::meet
        // (push from self, advance self).
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

    // SupersessionSet — constructors, accessors, and edge cases

    static SUP_COV: &[(u8, u8)] = &[(1, 2)];

    #[test]
    fn supersession_set_new_is_empty() {
        let s: SupersessionSet<u8> = SupersessionSet::new(SUP_COV);
        assert!(s.is_empty());
        assert_eq!(s.as_slice(), &[] as &[u8]);
    }

    #[test]
    fn supersession_set_populated_accessors() {
        let s = SupersessionSet::from_iter_sorted(vec![2_u8, 3], SUP_COV);
        assert!(!s.is_empty());
        assert_eq!(s.as_slice(), &[2, 3]);
    }

    #[test]
    fn supersession_set_from_iter_dedupes() {
        let s = SupersessionSet::from_iter_sorted(vec![2_u8, 2, 3, 3], SUP_COV);
        assert_eq!(s.as_slice(), &[2, 3]);
    }

    #[test]
    fn supersession_set_join_preserves_non_superseded() {
        // No superseding element present — supersession is a no-op.
        let a = SupersessionSet::from_iter_sorted(vec![2_u8, 3], SUP_COV);
        let b = SupersessionSet::from_iter_sorted(vec![3_u8, 4], SUP_COV);
        let j = a.join(&b);
        // Union is {2, 3, 4}; no `1` anywhere, so `2` survives.
        assert_eq!(j.as_slice(), &[2, 3, 4]);
    }

    // ModeSet — accessors, mode(), extend_counts, and meet

    #[test]
    fn mode_set_default_is_empty() {
        let m: ModeSet<&str> = ModeSet::default();
        assert!(m.is_empty());
        assert_eq!(m.mode(), None);
    }

    #[test]
    fn mode_set_mode_ties_broken_by_ord() {
        // Two values tied at count 1 — smaller Ord wins per doc comment.
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
        let a = ModeSet::from_iter_counted(vec!["x", "x", "y"]); // {x:2, y:1}
        let b = ModeSet::from_iter_counted(vec!["x", "y", "y"]); // {x:1, y:2}
        let m = a.meet(&b);
        // Per-key min: {x:1, y:1}.
        assert_eq!(m.mode(), Some(&"x"));
    }

    #[test]
    fn mode_set_meet_drops_keys_missing_on_either_side() {
        let a = ModeSet::from_iter_counted(vec!["a", "b"]);
        let b = ModeSet::from_iter_counted(vec!["a", "c"]);
        let m = a.meet(&b);
        // Only `a` is common.
        assert_eq!(m.mode(), Some(&"a"));
    }

    // MaxDate — accessors and edge branches

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
        // Exercises the `a >= b` branch when they're equal.
        let a = MaxDate::present("20301231");
        assert_eq!(a.join(&a.clone()), a);
    }

    #[test]
    fn max_date_join_picks_left_when_greater() {
        // Exercises the `a >= b` branch.
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
        // Top dominates any real date under join.
        let d = MaxDate::present("20481231");
        assert_eq!(t.join(&d), t);
    }

    // OptionalSingleton — meet paths and BoundedLattice

    #[test]
    fn optional_singleton_meet_some_some_calls_inner() {
        let a = OptionalSingleton::present(OrdMax(3_u32));
        let b = OptionalSingleton::present(OrdMax(7_u32));
        // Inner OrdMax::meet picks the lesser.
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

    // Product — BoundedLattice + Default

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
