// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! SCI per-system catalog — `CompanionForm` + `RULE_E059` +
//! `SciPerSystemKind` + `SciPerSystemRow` + the 5-row
//! `SCI_PER_SYSTEM_CATALOG`.
//!
//! Carved out from `scheme/mod.rs` per the Stage 2 PR B hub-split
//! (issue #466). Module contents are byte-identical to the pre-split
//! source — imports adjusted to reach the `presence_*` helpers via
//! `super::predicates::*`, the `emit_*_companions` closures via
//! `super::actions::*`, and the `CapcoScheme` type via `super::*`
//! (the parent module's `pub use self::adapter::CapcoScheme` re-export).

use super::actions::*;
use super::predicates::*;
use super::*;
use crate::fact_bitmask::fact_bit;

// ===========================================================================
// PR 3b.E (T026e) — SCI per-system catalog (§H.4)
// ===========================================================================
//
// `sci_per_system_catalog_eval` is the static-table dispatcher for the 5
// `Constraint::Custom` rows declared by `build_constraints` under the
// "PR 3b.E (T026e) — SCI per-system catalog (§H.4)" section header.
//
// Each row's predicate has a uniform shape: "if SCI marking M is present in
// `attrs`, the portion's IC dissem block must satisfy F(M)" where F(M) is
// either a companion-required check (NOFORN must appear) or a multi-branch
// check covering required-and-forbidden companions (ORCON required, ORCON-
// USGOV forbidden, etc.). The table stores one entry per row carrying:
//
//   - `name`: catalog row identifier (matches `Constraint::Custom { name }`,
//      and starts with the `sci-per-system/` prefix)
//   - `marking_label`: human-readable marking name for the diagnostic
//   - `presence`: predicate `fn(&CanonicalAttrs) -> bool` checking whether
//      the family pattern is present
//   - `kind`: dispatch tag — `CompanionRequired` (single dissem-control
//      insertion) or `Custom` (closure for multi-branch emit logic)
//   - `severity`: per-row default `Severity` (typically `Warn`; missing
//      companions remain fixable even when no IC dissem block exists,
//      because the structural `FactAdd` intent is applied to the parsed
//      marking and canonical re-rendering synthesizes the block)
//   - `citation`: per-row §-citation matching `Constraint::Custom { label }`
//
// Diagnostic-span anchoring is NOT a row field — companion-insertion
// branches anchor the diagnostic at the offending SCI marking token via
// `first_sci_span(attrs)`, while token-replacement branches (e.g., the
// OC-USGOV → OC fix in row #1 / #3 / #4) anchor both the diagnostic and
// the fix at the dissem token's own span so the user sees the offending
// dissem token directly. See the per-emit-fn doc comments for the
// branch-specific anchor.
//
// The catalog is consumed by `CapcoScheme::bridge_sci_per_system_diagnostics`
// (in `adapter.rs`), which is the engine's direct emit path for E059
// diagnostics + fixes. PR 3c.B Commit 7.4 retired the original
// `DeclarativeSciPerSystemRule` walker in favor of the direct bridge so
// the catalog's per-row fixes (companion-insertion at the dissem-block
// anchor and `ORCON-USGOV → ORCON` token replacement) could ride
// alongside the diagnostics without threading a fix table through the
// `ConstraintViolation` envelope.
//
// FORWARD LINK to PR 4 (per-category Lattice impls): once `marque-scheme`
// exposes `Constraint::CompanionRequired<Set>` / `Forbid<Set>` primitives
// (or the equivalent ImplTable / closure-operator machinery from
// `marque-applied.md` §3.4.6), these rows can re-classify from
// `Constraint::Custom` to a primitive form without changing per-row
// semantics. See `docs/plans/2026-05-08-pr3b-E-sci-per-system-collapse-plan.md`
// §1 for the architectural rationale.

/// Companion form (abbreviated vs full) inferred from the dissem-token
/// text observed on a portion. Used to keep the inserted token's surface
/// form consistent with the existing block (so `(S//HCS-O//OC)` inserts
/// `/NF`, not `/NOFORN`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum CompanionForm {
    /// Short form: `OC`, `NF`, `OC-USGOV`. Used when the first observed
    /// dissem token on the portion is a portion/abbrev surface form.
    Abbreviated,
    /// Full form: `ORCON`, `NOFORN`. Used otherwise (banner long-form or
    /// no dissem block yet).
    Full,
}

/// Walker rule ID shared by every SCI per-system catalog emit body.
/// `RuleId::new` is `const fn`, so this is a zero-cost replacement for
/// the four prior inline `RuleId::new("E059")` call sites (one per
/// row-emit helper). Hoisting also makes a future rule-ID change a
/// single edit.
pub(crate) const RULE_E059: marque_rules::RuleId = marque_rules::RuleId::new("E059");

/// Dispatch tag for an SCI per-system catalog row's emit body. Two
/// variants keep the `match row.kind` arm count under the ≤3-branch
/// reviewer-attestation cap (§7(b) of the PR 3b.E plan).
#[derive(Copy, Clone)]
pub(crate) enum SciPerSystemKind {
    /// Single dissem-control insertion. The row encodes "if marking M is
    /// present, dissem control D must appear; if absent, emit a
    /// zero-width insertion fix at the end of the IC dissem block." The
    /// only PR-E rows using this kind are the NOFORN-only rows (#2 and
    /// #5).
    CompanionRequired {
        /// The dissem control whose presence is required.
        dissem: marque_ism::DissemControl,
        /// Component for the diagnostic message (e.g., "NOFORN").
        token_name: &'static str,
    },
    /// Custom multi-branch emit. The row encodes a closure that produces
    /// the full emit list, used by rows whose emit logic spans 2-3 distinct
    /// branches with row-specific text and span logic (rows #1, #3, #4).
    /// The `candidate_span` argument is the full marking-scope span
    /// (portion or banner) that the engine's `synthesize_fixes` path
    /// uses to look up the parsed marking for `apply_intent` +
    /// `render_canonical`. `fix_scope` is the scope discriminator
    /// embedded in any `FactAdd` / `Recanonicalize` intent the row
    /// emits — `Scope::Portion` for portion candidates, `Scope::Page`
    /// for banner candidates.
    Custom(
        fn(
            &marque_ism::CanonicalAttrs,
            marque_scheme::Span,
            marque_scheme::Scope,
            &SciPerSystemRow,
        ) -> Vec<marque_rules::Diagnostic<CapcoScheme>>,
    ),
}

/// One catalog row. The walker dispatches over `&[SciPerSystemRow]`;
/// each row owns its presence predicate, dispatch kind, severity,
/// citation, and human-readable marking label.
///
/// # Naming-prefix invariant
///
/// Every row's `name` MUST start with `sci-per-system/`. The
/// `sci_per_system_catalog_naming_convention` test in
/// `crates/capco/tests/sci_per_system_catalog.rs` enforces this at build
/// time so adding a row that doesn't follow the convention fails CI.
/// The prefix is what makes [`is_sci_per_system_catalog_name`] dispatch
/// O(1) instead of a linear catalog scan.
#[derive(Copy, Clone)]
pub(crate) struct SciPerSystemRow {
    /// Catalog row name — matches the `Constraint::Custom { name }` of
    /// the same logical row. MUST start with `sci-per-system/`.
    pub(crate) name: &'static str,
    /// Human-readable marking name for the diagnostic message
    /// (e.g., `"HCS-O"`, `"TK-{BLFH|IDIT|KAND}"`).
    pub(crate) marking_label: &'static str,
    /// Marking-presence predicate.
    pub(crate) presence: fn(&marque_ism::CanonicalAttrs) -> bool,
    /// Dispatch kind — `CompanionRequired` (single-token) or `Custom`
    /// (multi-branch closure).
    pub(crate) kind: SciPerSystemKind,
    /// Default severity (typically `Warn`).
    pub(crate) severity: marque_rules::Severity,
    /// Tier-3 bitmask trigger (PR-H / issue #371).
    ///
    /// `Some(mask)` when the row has a closed-atom trigger: the
    /// `sci_per_system_catalog_eval` fast path returns empty immediately
    /// when `(bits & mask) == 0` without calling `presence()`.
    ///
    /// `None` is intentionally unused for all 5 SCI per-system rows
    /// (all have closed-atom triggers) but kept for forward-compatibility
    /// parity with [`ClassFloorRow`].
    pub(crate) bitmask_trigger: Option<u128>,
    /// When `true`, the trigger mask is exact (no false positives) and
    /// `presence()` confirmation is skipped. When `false`, the mask is a
    /// coarse gate and `presence()` must confirm before the companion
    /// check runs.
    pub(crate) bitmask_trigger_exact: bool,
    /// Bitmask of companion bits that MUST all be set (AND-mask) for the
    /// row to be "violation-free" from a bitmask perspective.
    /// Zero means no required-companion bits (the row is satisfied by
    /// absence alone, or the "forbidden" mask does all the work).
    pub(crate) bitmask_companion_required: u128,
    /// Bitmask of companion bits where ANY set bit indicates a violation
    /// (forbidden-bit OR-mask).
    /// Zero means no forbidden bits.
    pub(crate) bitmask_companion_forbidden: u128,
    /// Per-row §-citation, matching `Constraint::Custom { label }`.
    /// PR 3c.2.C C5 retired the emission path through this field per
    /// PM-C-1 (catalog row citations stay `&'static str` for
    /// citation-lint scanning); use [`Self::citation_typed`] at emit
    /// time.
    pub(crate) citation: &'static str,
    /// Typed [`marque_rules::Citation`] used at emission time. Must
    /// agree with [`Self::citation`].
    pub(crate) citation_typed: marque_rules::Citation,
}

// ---------------------------------------------------------------------------
// The catalog — 5 rows at §H.4 family granularity
// ---------------------------------------------------------------------------

pub(crate) const SCI_PER_SYSTEM_CATALOG: &[SciPerSystemRow] = &[
    // Row #1 — HCS-O companions (ORCON + NOFORN required, ORCON-USGOV
    // forbidden). §H.4 p64.
    //
    // Tier-3 bitmask: SCI_HCS_O (bit 41) is the exact trigger — only set
    // when HCS-O is present. Companion satisfied when: no US class, OR
    // (ORCON bit set AND NOFORN bit set AND ORCON_USGOV bit clear).
    // Structural `emit_hcs_o_companions` fires when: !has_orcon ||
    // !has_noforn || usgov_entry.is_some(). Non-firing complement:
    // (Oc || OcUsgov) && Nf && usgov_entry.is_none(). OcUsgov is in
    // companion_forbidden so the structural path runs when OcUsgov is
    // present, preserving the OcUsgov→Oc replacement fix in production.
    SciPerSystemRow {
        name: "sci-per-system/HCS-O-companions",
        marking_label: "HCS-O",
        presence: presence_hcs_o,
        kind: SciPerSystemKind::Custom(emit_hcs_o_companions),
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §H.4 p64",
        citation_typed: marque_rules::capco(marque_rules::SectionLetter::H, 4, 64),
        bitmask_trigger: Some(1u128 << fact_bit::SCI_HCS_O),
        bitmask_trigger_exact: true,
        bitmask_companion_required: (1u128 << fact_bit::ORCON) | (1u128 << fact_bit::NOFORN),
        bitmask_companion_forbidden: 1u128 << fact_bit::ORCON_USGOV,
    },
    // Row #2 — HCS-P NOFORN (NOFORN required). §H.4 p66.
    //
    // Tier-3 bitmask: coarse gate on SCI_PRESENT (bit 37) — bare HCS-P
    // (no sub-compartments) only sets this bit; HCS-P with sub-compartments
    // also sets SCI_HCS_P_SUB (bit 42). Using SCI_PRESENT as the coarse
    // gate catches both cases; presence_hcs_p_any confirms HCS-P
    // specifically. Companion satisfied when: no US class, OR NOFORN set.
    SciPerSystemRow {
        name: "sci-per-system/HCS-P-NOFORN",
        marking_label: "HCS-P",
        presence: presence_hcs_p_any,
        kind: SciPerSystemKind::CompanionRequired {
            dissem: marque_ism::DissemControl::Nf,
            token_name: "NOFORN",
        },
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §H.4 p66",
        citation_typed: marque_rules::capco(marque_rules::SectionLetter::H, 4, 66),
        bitmask_trigger: Some(1u128 << fact_bit::SCI_PRESENT),
        bitmask_trigger_exact: false,
        bitmask_companion_required: 1u128 << fact_bit::NOFORN,
        bitmask_companion_forbidden: 0,
    },
    // Row #3 — HCS-P sub-compartment companions (ORCON required,
    // ORCON-USGOV forbidden). §H.4 p68. NOFORN is covered by row #2.
    //
    // Tier-3 bitmask: SCI_HCS_P_SUB (bit 42) is exact — only set when
    // HCS-P has at least one sub-compartment. Companion satisfied when:
    // no US class, OR (ORCON set AND ORCON_USGOV clear).
    SciPerSystemRow {
        name: "sci-per-system/HCS-P-sub-companions",
        marking_label: "HCS-P sub-compartment",
        presence: presence_hcs_p_sub,
        kind: SciPerSystemKind::Custom(emit_hcs_p_sub_companions),
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §H.4 p68",
        citation_typed: marque_rules::capco(marque_rules::SectionLetter::H, 4, 68),
        bitmask_trigger: Some(1u128 << fact_bit::SCI_HCS_P_SUB),
        bitmask_trigger_exact: true,
        bitmask_companion_required: 1u128 << fact_bit::ORCON,
        bitmask_companion_forbidden: 1u128 << fact_bit::ORCON_USGOV,
    },
    // Row #4 — SI-G companions (ORCON required, ORCON-USGOV forbidden).
    // §H.4 p80.
    //
    // Tier-3 bitmask: SCI_SI_G (bit 40) is exact — only set when SI-G is
    // present. Companion satisfied when: no US class, OR (ORCON set AND
    // ORCON_USGOV clear). Mirrors HCS-P-sub structure (same emit logic).
    SciPerSystemRow {
        name: "sci-per-system/SI-G-companions",
        marking_label: "SI-G",
        presence: presence_si_g,
        kind: SciPerSystemKind::Custom(emit_si_g_companions),
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §H.4 p80",
        citation_typed: marque_rules::capco(marque_rules::SectionLetter::H, 4, 80),
        bitmask_trigger: Some(1u128 << fact_bit::SCI_SI_G),
        bitmask_trigger_exact: true,
        bitmask_companion_required: 1u128 << fact_bit::ORCON,
        bitmask_companion_forbidden: 1u128 << fact_bit::ORCON_USGOV,
    },
    // Row #5 — TK compartment NOFORN (BLFH/IDIT/KAND require NOFORN).
    // §H.4 p87 (TK-BLFH) + p91 (TK-IDIT) + p95 (TK-KAND).
    // Typed Citation anchors at §H.4 p87; the p91 / p95
    // cross-references live in the row's `citation` documentation
    // field above.
    //
    // Tier-3 bitmask: three-bit OR trigger (SCI_TK_BLFH | SCI_TK_IDIT |
    // SCI_TK_KAND, bits 43-45) — exact, since each atom is set only when
    // its named compartment is present. presence_tk_compartment_noforn
    // is skipped on the fast path. Companion satisfied when: no US class,
    // OR NOFORN set.
    SciPerSystemRow {
        name: "sci-per-system/TK-compartment-NOFORN",
        marking_label: "TK-{BLFH|IDIT|KAND}",
        presence: presence_tk_compartment_noforn,
        kind: SciPerSystemKind::CompanionRequired {
            dissem: marque_ism::DissemControl::Nf,
            token_name: "NOFORN",
        },
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §H.4 p87 + p91 + p95",
        citation_typed: marque_rules::capco(marque_rules::SectionLetter::H, 4, 87),
        bitmask_trigger: Some(
            (1u128 << fact_bit::SCI_TK_BLFH)
                | (1u128 << fact_bit::SCI_TK_IDIT)
                | (1u128 << fact_bit::SCI_TK_KAND),
        ),
        bitmask_trigger_exact: true,
        bitmask_companion_required: 1u128 << fact_bit::NOFORN,
        bitmask_companion_forbidden: 0,
    },
];
