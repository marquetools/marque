// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Class-floor catalog helpers: name → row resolution, span anchors,
//! policy-satisfaction predicate, and the trait-path catalog walker
//! (`class_floor_catalog_eval`). Lifted from the monolithic
//! `predicates.rs` per the issue #466 Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).

use marque_ism::{CanonicalAttrs, Classification, Span, TokenKind};

use super::super::constraints::class_floor_emit;
use super::super::*;

/// Returns true if `name` is a catalog row name dispatched by
/// [`class_floor_catalog_eval`]. Used by `evaluate_custom_by_attrs`
/// to route on the table.
///
/// PR D R3.2 (R3 C1): O(1) prefix check. Every catalog row's `name`
/// follows one of two prefix conventions (see [`ClassFloorRow`]
/// docstring):
///
///   - `E058/<purpose>` for rows replacing a retired legacy rule.
///   - `class-floor/<marking>` for rows with no retired-rule
///     predecessor.
///
/// New catalog rows MUST follow one of these prefixes; the
/// `class_floor_catalog_naming_convention` test in
/// `crates/capco/tests/class_floor_catalog.rs` enforces the
/// invariant at build time so adding a row that doesn't follow the
/// convention fails CI.
pub(crate) fn is_class_floor_catalog_name(name: &str) -> bool {
    name.starts_with("E058/") || name.starts_with("class-floor/")
}

/// Resolve a catalog row by `name`. Returns `None` for unknown
/// names.
///
/// Walked only on the trait/validate path (27-row catalog → linear
/// scan, ≪1 µs) — the walker hot path uses
/// [`class_floor_catalog`] then [`class_floor_eval_row`] directly
/// with no name lookup. A build-time perfect-hash lookup
/// (`phf::Map`) is deferred unless the trait path shows up as a
/// measurable hotspot in profiling.
pub(crate) fn class_floor_row_by_name(name: &str) -> Option<&'static ClassFloorRow> {
    CLASS_FLOOR_CATALOG.iter().find(|row| row.name == name)
}

/// Resolve the diagnostic span anchor for a class-floor catalog row.
///
/// Lifted from `rules_declarative::class_floor_anchor_span` in PR
/// 3c.B Commit 7.3 when the `DeclarativeClassFloorRule` walker
/// retired into the engine's constraint-catalog bridge. Per PM
/// directive #2 of the original PR 3b.D plan, the span anchors at
/// the marking token (not the classification token) so the
/// diagnostic UX puts the squiggle under the offending presence.
/// Reads `row.primary_kind` directly (the PR D R2 perf-3
/// optimization hoisted from the retired `primary_token_kind_for_row`
/// string-match table into a struct field on `ClassFloorRow`).
/// Falls back to the first `Classification` token span if no
/// axis-specific span is found, and finally to `Span::new(0, 0)` if
/// neither is present.
pub(crate) fn class_floor_anchor_span(attrs: &CanonicalAttrs, row: &ClassFloorRow) -> Span {
    if let Some(kind) = row.primary_kind
        && let Some(span) = first_span_of_optional(attrs, kind)
    {
        return span;
    }
    // Some rows have no single primary kind (e.g., NATO rows have no
    // marking-side token; `row.primary_kind == None`). Try
    // classification as a fallback.
    if let Some(span) = first_span_of_optional(attrs, TokenKind::Classification) {
        return span;
    }
    Span::new(0, 0)
}

/// Returns the first span of a given token kind in the attrs'
/// `token_spans`, or `None` if the kind is absent. Lifted from
/// `rules_declarative::first_span_of_optional` in PR 3c.B Commit
/// 7.3 alongside [`class_floor_anchor_span`].
pub(crate) fn first_span_of_optional(attrs: &CanonicalAttrs, kind: TokenKind) -> Option<Span> {
    attrs
        .token_spans
        .iter()
        .find(|t| t.kind == kind)
        .map(|t| t.span)
}

/// Dispatch a single catalog row by name and return at most one
/// `ConstraintViolation`. The trait-path entry point used by
/// [`MarkingScheme::validate`] →
/// [`marque_scheme::constraint::evaluate`] when the catalog row's
/// `Constraint::Custom` arm fires.
///
/// PR 3c.B Commit 7.3: the walker hot-path equivalent
/// (`class_floor_eval_row`) retired alongside
/// `DeclarativeClassFloorRule`; the engine's constraint-catalog
/// bridge invokes this function via `evaluate_custom` → here, and
/// fields are populated in [`class_floor_emit`] so no second emitter
/// path is needed.
pub(crate) fn class_floor_catalog_eval(
    attrs: &marque_ism::CanonicalAttrs,
    name: &'static str,
) -> Vec<ConstraintViolation> {
    class_floor_row_by_name(name)
        .and_then(|row| class_floor_emit(attrs, row))
        .map(|v| vec![v])
        .unwrap_or_default()
}

/// Returns true when the classification axis satisfies the floor policy.
///
/// The two policy variants take different views of the classification axis:
///
/// - **`AtLeast(floor)`** uses `MarkingClassification::effective_level`
///   so NATO / FGI / JOINT classifications get reciprocal-raised to
///   their US-equivalent level per `marque-applied.md` §3.4.1 Note (i)
///   (CTS → TS, NS → S, NC → C, NR → R, NU → U). This is the C1 fix
///   from PR #324 R1: before the fix, the NATO catalog rows
///   (BALK / BOHEMIA / ATOMAL) queried `attrs.us_classification()`,
///   which returns `None` for non-US classification kinds, so the
///   reciprocal-raised NATO floors always failed and always emitted a
///   spurious diagnostic — guaranteed false positive on every
///   well-formed NATO portion. The `effective_level()` accessor
///   already lives in `marque-ism` and is the canonical answer to
///   "what's the effective classification level for ordering?";
///   capco-side we just consume it.
///
///   Behavior on a `None` classification (no classification token
///   parsed at all) stays as "fail the floor" — this preserves
///   retired-E022 / retired-E027 semantics where a CNWDI / SAR marking
///   without any classification context is treated as malformed and
///   the floor diagnostic fires.
///
/// - **`EqualsU`** keeps `attrs.us_classification()` semantics. The
///   UCNI ceiling per CAPCO-2016 §H.6 p116 (DOD UCNI) and §H.6 p118
///   (DOE UCNI) is "May only be used with UNCLASSIFIED" — strictly the
///   US-classification system, not reciprocal-raised. A NATO-class
///   portion carrying UCNI is malformed input (UCNI is US AEA,
///   parallel to NATO ATOMAL); other rules catch the malformed shape.
pub(crate) fn class_floor_satisfied(
    attrs: &marque_ism::CanonicalAttrs,
    policy: ClassFloorPolicy,
) -> bool {
    match policy {
        ClassFloorPolicy::AtLeast(floor) => match attrs.classification.as_ref() {
            // Reciprocal-raise via `effective_level()`. NATO / FGI /
            // JOINT classifications return their US-equivalent level
            // for the comparison; US classifications return as-is.
            Some(c) => c.effective_level() >= floor,
            // No classification parsed at all → fail the floor.
            // Preserves retired-E022 / retired-E027 behavior on the
            // "classification is missing" case.
            None => false,
        },
        ClassFloorPolicy::EqualsU => match attrs.us_classification() {
            // Equals-U is the UCNI ceiling. `Some(Unclassified)` is the
            // only allowed state; everything else (including `None` for
            // pure-FGI / NATO / JOINT) fails. Mirrors retired E025
            // semantics: UCNI is US AEA and a non-US classification
            // carrying UCNI is malformed.
            Some(Classification::Unclassified) => true,
            _ => false,
        },
    }
}
