// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Category descriptors and aggregation operators.
//!
//! A marking is a product over categories (classification, SCI, dissem,
//! REL TO, declassify-on, ...). Each category declares:
//!
//! - `ordering_rank` — where it appears in the canonical left-to-right
//!   rendering of the marking.
//! - `aggregation` — how values from multiple portions combine when
//!   projecting to a banner.
//! - `cardinality` — 1 / 0..1 / 0..N.
//! - `intra_ordering` — how values within this category sort
//!   (e.g., "USA first, then alphabetical" for REL TO).
//! - `expansion` — optional: expand composite tokens (tetragraphs) to
//!   atomic tokens before aggregation.
//!
//! The operator is applied by the engine during `project_banner`; the
//! `Custom` variant is the escape hatch for rules that can't be
//! expressed as one of the enumerated operators.

/// Opaque category identifier. Each scheme assigns stable ids to its
/// categories; the engine only compares them for equality.
///
/// # Reserved sentinel: [`CategoryId::MARKING`]
///
/// [`CategoryId::MARKING`] (numeric value `0`) is reserved across all
/// schemes as the multi-category "whole-marking" sentinel — used by
/// audit-record emit when a fix's structural payload spans more than one
/// category (the [`ReplacementIntent::Recanonicalize`](crate::ReplacementIntent::Recanonicalize)
/// arm re-renders an entire `Scope::Page` / `Scope::Document`, not a
/// single category's axis). Schemes MUST assign concrete categories
/// starting at `CategoryId(1)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CategoryId(pub u32);

impl CategoryId {
    /// Reserved multi-category "whole-marking" sentinel.
    ///
    /// Used by the audit-record emit path when a fix's
    /// [`ReplacementIntent`](crate::ReplacementIntent) re-renders an
    /// entire `Scope::Page` / `Scope::Document` rather than naming a
    /// single category. The renderer projects this to the JSON
    /// `replacement.canonical.category` field as the literal string
    /// `"Marking"`.
    ///
    /// All other [`CategoryId`] values are scheme-allocated (CAPCO
    /// assigns its categories starting at `CategoryId(1)`; see
    /// `marque-capco`'s `CAT_CLASSIFICATION` and successors).
    pub const MARKING: CategoryId = CategoryId(0);
}

/// Opaque token identifier within a scheme. Used by supersession
/// relations and constraint predicates to reference specific tokens
/// without coupling to a scheme-specific enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TokenId(pub u32);

/// How many tokens from a category can appear in one marking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    /// Exactly one (classification level).
    One,
    /// Zero or one (optional single value, e.g., declassification exemption).
    Optional,
    /// Zero or more (dissem controls, trigraphs, SCI compartments).
    Many,
}

/// Ordering rule for values within a single category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntraOrdering {
    /// Source order preserved (rare; used when the scheme's grammar
    /// gives meaning to appearance order).
    AsWritten,
    /// Lexical ascending sort.
    Alphabetical,
    /// Numeric-then-alphabetical per CAPCO §A.6 p15 (SCI, SAR).
    /// Digit-prefixed tokens sort numerically and precede alpha tokens.
    NumericThenAlpha,
    /// A single token must appear first; the rest sort per the nested
    /// ordering. REL TO's "USA first, then alphabetical" case.
    FixedFirst {
        first: TokenId,
        rest: Box<IntraOrdering>,
    },
}

/// Per-category aggregation operator.
///
/// # Status
///
/// `AggregationOp` is **not on the runtime dispatch path**. The engine
/// reduces categories by calling [`Lattice::join`](crate::Lattice::join)
/// on the category's value type; it does not consult this enum to decide
/// which reducer to run. The variants survive as:
///
/// 1. **Build-time shorthand** for authors declaring flat categories
///    (the values here map to the built-in lattice constructors in
///    [`crate::builtins`]).
/// 2. **Inspection metadata** returned from [`Category::shape`] so
///    tooling can render a scheme's aggregation semantics without
///    instantiating markings.
///
/// In particular, [`AggregationOp::Custom`] does not drive a runtime
/// dispatch — it's a marker meaning "this category has a bespoke
/// `impl Lattice`."
///
/// The inputs to each operator are the *atomic* tokens: any composite
/// token (e.g., FVEY → {USA, GBR, CAN, AUS, NZL}) is expanded by the
/// category's `expansion` function before the operator runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AggregationOp {
    /// Maximum by the category's total order. Used for classification
    /// level and declassification date (see `MaxDate`, kept distinct
    /// because it reads a datetime field, not a token ordinal).
    Max,
    /// Set union. Used for SCI, SAR, AEA, dissem (before supersession).
    Union,
    /// Set intersection. Used for REL TO country lists.
    Intersect,
    /// Union, then drop any token whose presence is obviated by a
    /// superseding token on the same set. Each pair `(superseding,
    /// superseded)` means "if `superseding` appears, drop `superseded`."
    ///
    /// Stored as `Box<[...]>` (rather than `Vec`) so schemes can
    /// define category tables without resizable-allocation overhead;
    /// the boxed slice is constructed once at scheme build time and
    /// then treated as immutable.
    ///
    /// Used for NOFORN ⊐ REL TO at the banner level.
    UnionWithSupersession(Box<[(TokenId, TokenId)]>),
    /// Max over date-typed values. Separate from `Max` because the
    /// engine's lookup path differs.
    MaxDate,
    /// Pick the most frequent value. Reserved for corporate / medical
    /// schemes that don't want "most restrictive" semantics.
    Mode,
    /// Escape hatch: caller-defined reducer over the category's tokens.
    Custom,
}

/// Expansion function for composite tokens (tetragraphs → member
/// trigraphs). Returning `None` means "not a composite; leave the
/// token alone."
///
/// Returns a borrowed `&'static [TokenId]` rather than an owned
/// `Vec<TokenId>` so the hot path (projection, render) can expand
/// without heap allocation. Composite membership is expected to be a
/// compile-time static table (e.g., FVEY = `&[USA, GBR, CAN, AUS,
/// NZL]` as a `const`). A scheme that needs dynamic expansion at
/// runtime should model that through a category-level escape hatch
/// instead.
pub type ExpansionFn = fn(TokenId) -> Option<&'static [TokenId]>;

/// A category of tokens within a marking scheme.
#[derive(Debug, Clone)]
pub struct Category {
    pub id: CategoryId,
    pub name: &'static str,
    /// Left-to-right position in the canonical marking (lower = earlier).
    pub ordering_rank: u16,
    pub cardinality: Cardinality,
    pub aggregation: AggregationOp,
    pub intra_ordering: IntraOrdering,
    /// Optional expansion for composite tokens.
    pub expansion: Option<ExpansionFn>,
}

/// Runtime-inspectable shape of a category's lattice.
///
/// `AggregationOp` is not a runtime dispatch point — the engine reduces
/// categories by calling `Lattice::join` on their values.
/// `CategoryShape` is the inspection counterpart to `AggregationOp`:
/// a scheme-exploration UI or docs generator can walk
/// `scheme.categories()`, call `Category::shape()` on each one, and
/// render the aggregation semantics without instantiating any marking
/// values. The engine never consults this.
///
/// `Product` and `Optional` nest recursively so composed lattices
/// (`Product<FlatSet<T>, OrdMax<U>>`) can describe their structure in
/// full. `Custom` is the terminator for schemes whose category has a
/// bespoke `impl Lattice` that doesn't decompose into built-ins (SCI's
/// compartment tree, SAR's program hierarchy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CategoryShape {
    /// `OrdMax` / `OrdMin` — total-order lattice.
    Ordinal,
    /// `FlatSet` — powerset with join = union.
    FlatSet,
    /// `IntersectSet` — powerset with join = intersect.
    IntersectSet,
    /// `SupersessionSet` — union with post-filter supersession.
    Supersession,
    /// `MaxDate` — date-valued lattice.
    Date,
    /// `ModeSet` — multiset with mode-picking join.
    Mode,
    /// Any of the above lifted to `Option<L>` via `OptionalSingleton`.
    Optional(Box<CategoryShape>),
    /// `Product<A, B>` — component-wise join on a tuple.
    Product(Vec<CategoryShape>),
    /// A custom `impl Lattice` outside the built-in palette.
    Custom,
}

impl Category {
    /// Inspect this category's lattice shape. Defaults to a mapping
    /// from the `AggregationOp` build-time shorthand; schemes with
    /// bespoke `impl Lattice` categories override via a wrapper helper
    /// (see `CapcoScheme::category_shape` in `marque-capco`).
    pub fn shape(&self) -> CategoryShape {
        match &self.aggregation {
            AggregationOp::Max => CategoryShape::Ordinal,
            AggregationOp::MaxDate => CategoryShape::Date,
            AggregationOp::Union => CategoryShape::FlatSet,
            AggregationOp::Intersect => CategoryShape::IntersectSet,
            AggregationOp::UnionWithSupersession(_) => CategoryShape::Supersession,
            AggregationOp::Mode => CategoryShape::Mode,
            AggregationOp::Custom => CategoryShape::Custom,
        }
    }
}

// ---------------------------------------------------------------------------
// Generic reducers that consume AggregationOp
// ---------------------------------------------------------------------------

/// Apply `AggregationOp::Max` over a slice of values by `Ord`.
///
/// Returns `None` if the slice is empty. The comparison uses the type's
/// natural `Ord`, which the scheme author is responsible for wiring
/// correctly (e.g., implementing `Ord` on the classification enum so
/// that `U < C < S < TS`).
pub fn reduce_max<T: Ord + Clone>(values: &[T]) -> Option<T> {
    values.iter().max().cloned()
}

/// Apply `AggregationOp::Union` over a slice, preserving first-seen
/// order and discarding duplicates. Kept stable rather than sorted so
/// that callers who want a particular intra-category order can post-
/// sort with the category's `IntraOrdering`.
///
/// Uses a [`HashSet`](std::collections::HashSet) for O(n) dedup
/// tracking (the output is still ordered). Tokens must be `Hash + Eq`
/// — which covers every token shape the existing schemes use
/// (`TokenId`, `&str`, owned strings, enum variants).
#[inline]
pub fn reduce_union<T: Eq + std::hash::Hash + Clone>(values: &[T]) -> Vec<T> {
    let mut out: Vec<T> = Vec::with_capacity(values.len());
    let mut seen: std::collections::HashSet<&T> =
        std::collections::HashSet::with_capacity(values.len());
    for v in values {
        if seen.insert(v) {
            out.push(v.clone());
        }
    }
    out
}

/// Apply `AggregationOp::Intersect` over a slice of sets. Returns the
/// tokens present in every set.
///
/// An empty input returns an empty result (vacuous truth does not help
/// here — the caller should check `portions.is_empty()` before calling
/// this on the empty case).
#[inline]
pub fn reduce_intersect<T: Eq + Clone>(sets: &[Vec<T>]) -> Vec<T> {
    let Some((first, rest)) = sets.split_first() else {
        return Vec::new();
    };
    first
        .iter()
        .filter(|v| rest.iter().all(|s| s.contains(v)))
        .cloned()
        .collect()
}

/// Apply `AggregationOp::UnionWithSupersession`. Unions the values, then
/// drops any token appearing on the right side of a pair whose left
/// side is present in the union.
///
/// Precomputes a `HashSet` of the unioned tokens so the supersession
/// filter is O(n + k) rather than O(n·k) over the supersession pairs.
#[inline]
pub fn reduce_union_with_supersession<T: Eq + std::hash::Hash + Clone>(
    values: &[T],
    supersession: &[(T, T)],
) -> Vec<T> {
    let unioned = reduce_union(values);
    let present: std::collections::HashSet<&T> = unioned.iter().collect();
    unioned
        .iter()
        .filter(|t| {
            !supersession
                .iter()
                .any(|(superseding, superseded)| superseded == *t && present.contains(superseding))
        })
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum Level {
        #[allow(dead_code)]
        U,
        C,
        S,
        TS,
    }

    #[test]
    fn max_reduces_to_peak() {
        assert_eq!(
            reduce_max(&[Level::C, Level::TS, Level::S]),
            Some(Level::TS)
        );
        assert_eq!(reduce_max::<Level>(&[]), None);
    }

    #[test]
    fn union_deduplicates_preserving_order() {
        let v = reduce_union(&["SI", "TK", "SI", "HCS"]);
        assert_eq!(v, vec!["SI", "TK", "HCS"]);
    }

    #[test]
    fn union_empty_slice() {
        let v: Vec<&str> = reduce_union(&[]);
        assert_eq!(v, Vec::<&str>::new());
    }

    #[test]
    fn union_single_element() {
        let v = reduce_union(&["SI"]);
        assert_eq!(v, vec!["SI"]);
    }

    #[test]
    fn union_no_duplicates() {
        let v = reduce_union(&["SI", "TK", "HCS"]);
        assert_eq!(v, vec!["SI", "TK", "HCS"]);
    }

    #[test]
    fn union_all_duplicates() {
        let v = reduce_union(&["TK", "TK", "TK"]);
        assert_eq!(v, vec!["TK"]);
    }

    #[test]
    fn union_no_duplicates_preserves_order() {
        let v = reduce_union(&["A", "B", "A", "C"]);
        assert_eq!(v, vec!["A", "B", "C"]);
    }

    #[test]
    fn intersect_returns_common_subset() {
        let a = vec!["USA", "GBR", "CAN"];
        let b = vec!["USA", "GBR", "DEU"];
        let c = vec!["USA", "GBR"];
        assert_eq!(reduce_intersect(&[a, b, c]), vec!["USA", "GBR"]);
    }

    #[test]
    fn intersect_empty_on_disjoint() {
        let a = vec!["AUS"];
        let b = vec!["GBR"];
        assert_eq!(reduce_intersect::<&str>(&[a, b]), Vec::<&str>::new());
    }

    #[test]
    fn intersect_empty_on_no_portions() {
        assert_eq!(reduce_intersect::<&str>(&[]), Vec::<&str>::new());
    }

    #[test]
    fn supersession_drops_superseded_when_superseding_present() {
        // Model: NOFORN (1) supersedes REL TO (2). Union result contains
        // both, so the superseded REL TO is dropped.
        let values = [1_u8, 2];
        let supers = [(1_u8, 2_u8)];
        let out = reduce_union_with_supersession(&values, &supers);
        assert_eq!(out, vec![1]);
    }

    #[test]
    fn supersession_is_noop_when_superseding_absent() {
        let values = [2_u8];
        let supers = [(1_u8, 2_u8)];
        let out = reduce_union_with_supersession(&values, &supers);
        assert_eq!(out, vec![2]);
    }

    // CategoryShape from Category::shape() — exercises every arm.

    fn mk(op: AggregationOp) -> Category {
        Category {
            id: CategoryId(1),
            name: "c",
            ordering_rank: 0,
            cardinality: Cardinality::Many,
            aggregation: op,
            intra_ordering: IntraOrdering::AsWritten,
            expansion: None,
        }
    }

    #[test]
    fn shape_max_is_ordinal() {
        assert_eq!(mk(AggregationOp::Max).shape(), CategoryShape::Ordinal);
    }

    #[test]
    fn shape_max_date_is_date() {
        assert_eq!(mk(AggregationOp::MaxDate).shape(), CategoryShape::Date);
    }

    #[test]
    fn shape_union_is_flat_set() {
        assert_eq!(mk(AggregationOp::Union).shape(), CategoryShape::FlatSet);
    }

    #[test]
    fn shape_intersect_is_intersect_set() {
        assert_eq!(
            mk(AggregationOp::Intersect).shape(),
            CategoryShape::IntersectSet
        );
    }

    #[test]
    fn shape_union_with_supersession_is_supersession() {
        let c = mk(AggregationOp::UnionWithSupersession(Box::new([(
            TokenId(1),
            TokenId(2),
        )])));
        assert_eq!(c.shape(), CategoryShape::Supersession);
    }

    #[test]
    fn shape_mode_is_mode() {
        assert_eq!(mk(AggregationOp::Mode).shape(), CategoryShape::Mode);
    }

    #[test]
    fn shape_custom_is_custom() {
        assert_eq!(mk(AggregationOp::Custom).shape(), CategoryShape::Custom);
    }

    #[test]
    fn intra_ordering_fixed_first_constructs() {
        // Exercise the FixedFirst IntraOrdering variant — it's
        // constructed only when a scheme declares it.
        let o = IntraOrdering::FixedFirst {
            first: TokenId(104),
            rest: Box::new(IntraOrdering::Alphabetical),
        };
        match o {
            IntraOrdering::FixedFirst { first, rest } => {
                assert_eq!(first, TokenId(104));
                assert!(matches!(*rest, IntraOrdering::Alphabetical));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn intra_ordering_other_variants_exist() {
        let _n = IntraOrdering::NumericThenAlpha;
        let _a = IntraOrdering::AsWritten;
        let _l = IntraOrdering::Alphabetical;
    }

    #[test]
    fn cardinality_variants() {
        // Cardinality::One / Optional / Many — exercise each.
        let _o = Cardinality::One;
        let _op = Cardinality::Optional;
        let _m = Cardinality::Many;
    }
}
