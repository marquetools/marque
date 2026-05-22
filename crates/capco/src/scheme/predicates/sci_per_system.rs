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
use crate::fact_bitmask::fact_bit;

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
/// Post-T044: O(1) prefix check on the canonical predicate-ID form —
/// every catalog row's `name` MUST start with `marking.sci.`. This
/// prefix is uniquely scoped to the per-system catalog
/// (`portion.sci.*` is reserved for standalone SCI rules). The
/// `sci_per_system_catalog_naming_convention` test in
/// `crates/capco/tests/sci_per_system_catalog.rs` enforces the
/// invariant at build time.
pub(crate) fn is_sci_per_system_catalog_name(name: &str) -> bool {
    name.starts_with("marking.sci.")
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
///
/// # Tier-3 bitmask fast path (PR-H / issue #371)
///
/// All 5 SCI per-system rows have `bitmask_trigger: Some(_)`. The
/// dispatch order is:
///
/// 1. **Trigger mask gate**: `(bits & trigger) == 0` → no fire, return
///    empty in O(1) without calling `presence()`.
/// 2. **Presence confirmation** (coarse rows only, `trigger_exact: false`):
///    call `presence(attrs)` to rule out false positives from the
///    over-approximating `SCI_PRESENT` mask.
/// 3. **US-class early-out + companion check**: if no US classification
///    atom is set (`US_COLLATERAL_CLASSIFIED | US_UNCLASSIFIED == 0`),
///    the emit functions would return empty (all rows are US-only per §H.4);
///    otherwise check that all `companion_required` bits are set and no
///    `companion_forbidden` bit is set.
/// 4. **Structural fallthrough**: reached only when the trigger fires AND
///    the companion check indicates a possible violation — `sci_per_system_emit`
///    performs the full structural evaluation and produces `ConstraintViolation`s.
///
/// Emission is byte-identical to the pre-PR-H structural form: the fast
/// path reaches `sci_per_system_emit` on the same inputs (trigger confirmed,
/// companion not satisfied), and `sci_per_system_emit` is unchanged.
pub(crate) fn sci_per_system_catalog_eval(
    attrs: &marque_ism::CanonicalAttrs,
    bits: marque_scheme::FactBitmask,
    name: &'static str,
) -> Vec<ConstraintViolation> {
    let Some(row) = sci_per_system_row_by_name(name) else {
        return Vec::new();
    };

    // ── Tier-3 bitmask fast path (all 5 rows have Some trigger) ─────────────
    if let Some(trigger_mask) = row.bitmask_trigger {
        let bits_u128 = bits.bits();

        // Step 1: trigger gate — short-circuit if the marking family is absent.
        if (bits_u128 & trigger_mask) == 0 {
            return Vec::new();
        }

        // Step 2: presence confirmation for the one coarse-gate row
        // (HCS-P-NOFORN uses SCI_PRESENT as trigger; all other rows are exact).
        if !row.bitmask_trigger_exact && !(row.presence)(attrs) {
            return Vec::new();
        }

        // Step 3: US-class early-out + companion check.
        // All 5 emit functions open with `us_level(attrs).is_none()` → return
        // empty immediately (rows apply only to US-classified portions per §H.4).
        // Bitmask equivalent: if neither US_COLLATERAL_CLASSIFIED nor
        // US_UNCLASSIFIED is set, no US classification is present → no violation.
        let us_class_mask =
            (1u128 << fact_bit::US_COLLATERAL_CLASSIFIED) | (1u128 << fact_bit::US_UNCLASSIFIED);
        let no_us_class = (bits_u128 & us_class_mask) == 0;
        let companion_satisfied = no_us_class
            || ((bits_u128 & row.bitmask_companion_required) == row.bitmask_companion_required
                && (bits_u128 & row.bitmask_companion_forbidden) == 0);
        if companion_satisfied {
            return Vec::new();
        }
    }

    // ── Structural path (violation path) ────────────────────────────────────
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
        marque_scheme::Span::new(0, 0),
        marque_scheme::Scope::Portion,
        row,
    )
    .into_iter()
    .map(|d| ConstraintViolation {
        constraint_label: row.name,
        // Bridge layer per PM-C-1: render the typed `Diagnostic.message:
        // Message` to a `String` for the ConstraintViolation
        // (marque-scheme, graph-leaf) carrier. The audit consumer reads
        // the structured form via the engine's bridge_constraint_diagnostic,
        // which re-types this String into a typed `Message` at the
        // Diagnostic emit boundary. This local render uses the closed
        // template label — no document bytes flow through.
        message: d.message.template().as_str().to_owned(),
        citation: row.citation,
        span: None,
        severity: None,
    })
    .collect()
}
