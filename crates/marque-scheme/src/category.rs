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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CategoryId(pub u32);

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

/// Per-category aggregation operator. Applied during `project_banner`
/// over the values from all portions.
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
    /// Not used in Phase A.
    Custom,
}

/// Expansion function for composite tokens (tetragraphs → member
/// trigraphs). Returning `None` means "not a composite; leave the
/// token alone." Composite expansions are returned as borrowed
/// static slices to avoid per-call heap allocation in hot paths.
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
}
