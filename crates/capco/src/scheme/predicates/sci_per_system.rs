// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR-E SCI per-system catalog presence predicates plus the
//! trait-path catalog walker (`sci_per_system_catalog_eval`).
//! Lifted from the monolithic `predicates.rs` per the issue #466
//! Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).

use super::super::constraints::sci_per_system_emit;
use super::super::*;
use super::presence::{anchors_on, compartment_has_sub, has_compartment, is_tk_noforn_compartment};

// ---------------------------------------------------------------------------
// Family-presence predicates (one per PR-E catalog row)
// ---------------------------------------------------------------------------

/// HCS-O — any HCS-anchored marking carrying the "O" compartment.
/// §H.4 p64.
#[inline]
pub(crate) fn presence_hcs_o(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::SciControlBare;
    attrs
        .sci_markings
        .iter()
        .any(|m| anchors_on(m, SciControlBare::Hcs) && has_compartment(m, "O"))
}

/// HCS-P (any) — any HCS-anchored marking carrying the "P" compartment,
/// with or without sub-compartments. §H.4 p66 (and p68 inheriting NOFORN).
#[inline]
pub(crate) fn presence_hcs_p_any(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::SciControlBare;
    attrs
        .sci_markings
        .iter()
        .any(|m| anchors_on(m, SciControlBare::Hcs) && has_compartment(m, "P"))
}

/// HCS-P [SUB] — any HCS-anchored marking carrying a "P" compartment
/// with at least one sub-compartment. §H.4 p68. By §H.4 grammar, P is
/// the only HCS compartment that can carry sub-compartments, so this
/// coincides with `presence_hcs_comp_sub` from the class-floor catalog
/// in practice; we keep a separate predicate here to make the row
/// surface-explicit ("requires ORCON / forbids ORCON-USGOV on
/// sub-compartmented HCS-P").
#[inline]
pub(crate) fn presence_hcs_p_sub(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::SciControlBare;
    attrs
        .sci_markings
        .iter()
        .any(|m| anchors_on(m, SciControlBare::Hcs) && compartment_has_sub(m, "P"))
}

/// SI-G — any SI-anchored marking carrying the "G" compartment, with or
/// without sub-compartments. §H.4 p80 (and p81 inheriting ORCON).
#[inline]
pub(crate) fn presence_si_g(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::SciControlBare;
    attrs
        .sci_markings
        .iter()
        .any(|m| anchors_on(m, SciControlBare::Si) && has_compartment(m, "G"))
}

/// TK with BLFH/IDIT/KAND compartment — any TK-anchored marking carrying
/// at least one of the three NOFORN-required compartments. §H.4 p87 +
/// p91 + p95.
#[inline]
pub(crate) fn presence_tk_compartment_noforn(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs.sci_markings.iter().any(is_tk_noforn_compartment)
}

// ---------------------------------------------------------------------------
// Catalog dispatch
// ---------------------------------------------------------------------------

/// Returns true if `name` is a catalog row name dispatched by
/// [`sci_per_system_catalog_eval`]. Used by `evaluate_custom_by_attrs`
/// to route on the table.
///
/// O(1) prefix check — every catalog row's `name` MUST start with
/// `sci-per-system/`. The `sci_per_system_catalog_naming_convention`
/// test in `crates/capco/tests/sci_per_system_catalog.rs` enforces the
/// invariant at build time.
pub(crate) fn is_sci_per_system_catalog_name(name: &str) -> bool {
    name.starts_with("sci-per-system/")
}

/// Resolve a catalog row by `name`. Returns `None` for unknown names.
///
/// Walked only on the trait/validate path (5-row catalog → linear scan,
/// ≪1 µs). The walker hot path uses [`sci_per_system_catalog`] then
/// [`sci_per_system_emit`] directly with no name lookup.
pub(crate) fn sci_per_system_row_by_name(name: &str) -> Option<&'static SciPerSystemRow> {
    SCI_PER_SYSTEM_CATALOG.iter().find(|row| row.name == name)
}

/// Dispatch a single catalog row by name and return any
/// `ConstraintViolation`s. Trait-path entry point used by
/// [`MarkingScheme::validate`] →
/// [`marque_scheme::constraint::evaluate`] when the catalog row's
/// `Constraint::Custom` arm fires.
///
/// Note: PR-E rows produce `FixProposal` values on the walker path,
/// but `ConstraintViolation` doesn't carry a fix — the trait/validate
/// path drops the fix (this is the same divergence PR D's class-floor
/// catalog has). The engine path is the only path that produces
/// `AppliedFix` records, and the engine path always uses the walker.
pub(crate) fn sci_per_system_catalog_eval(
    attrs: &marque_ism::CanonicalAttrs,
    name: &'static str,
) -> Vec<ConstraintViolation> {
    let Some(row) = sci_per_system_row_by_name(name) else {
        return Vec::new();
    };
    // Trait-path doesn't have a candidate span (the engine's
    // bridge_sci_per_system_diagnostics direct path does). The
    // emitted Diagnostics are projected to ConstraintViolation
    // below which drops the fix payload — so the candidate_span
    // a Diagnostic's fix would have keyed on isn't observed here.
    // Pass an empty span as a sentinel; the resulting fix would be
    // dropped by the engine's `!f.span.is_empty()` filter even if a
    // hypothetical caller threaded it through.
    sci_per_system_emit(
        attrs,
        marque_ism::Span::new(0, 0),
        marque_scheme::Scope::Portion,
        row,
    )
    .into_iter()
    .map(|d| ConstraintViolation {
        constraint_label: row.name,
        message: String::from(d.message),
        citation: row.citation,
        span: None,
        severity: None,
    })
    .collect()
}
