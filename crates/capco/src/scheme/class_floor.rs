// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Class-floor catalog — `ClassFloorPolicy` + `ClassFloorRow` + the
//! 27-row `CLASS_FLOOR_CATALOG`.
//!
//! Imports the `presence_*` helpers from `super::predicates::*` and
//! the `Classification` / `TokenKind` types directly from `marque_ism`.

use marque_ism::{Classification, TokenKind};
use marque_scheme::{Citation, SectionLetter, capco};

use crate::fact_bitmask::fact_bit;

use super::predicates::*;

// ===========================================================================
// Class-floor catalog dispatch
// ===========================================================================
//
// `class_floor_catalog_eval` is the static-table dispatcher for the 27
// `Constraint::Custom` class-floor rows declared by `build_constraints`.
//
// Each row's predicate has a uniform shape: "if marking M is present in
// `attrs`, the page's classification must satisfy F(M)" where F(M) is
// either a floor (`level >= floor`) or an equality (`level == U`). The
// table stores one entry per row carrying:
//
//   - `name`: catalog row identifier (matches `Constraint::Custom { name }`)
//   - `marking_label`: human-readable marking name for the diagnostic
//   - `presence`: predicate `fn(&CanonicalAttrs) -> bool` checking whether
//      the family pattern is present
//   - `policy`: `ClassFloorPolicy` — either `AtLeast(level)` or `EqualsU`
//   - `severity`: `Severity` — `Error` for enumerated rows, `Warn` for
//      passthrough rows
//   - `citation`: per-row §-citation matching `Constraint::Custom { label }`
//   - `passthrough`: `true` for unknown-floor passthrough rows (drives the
//      diagnostic message variant)
//
// The catalog is consumed by the engine's class-floor bridge, which
// reads `CapcoScheme::has_diagnostic_constraints` to short-circuit
// the walk when the catalog has nothing to fire and otherwise runs
// the standard `MarkingScheme::validate()` → `Vec<ConstraintViolation>`
// path. The bridge is the only consumer.
//
// Once `TokenRef::ClassAtLeast(ClassLevel)` or `Constraint::ClassFloor`
// lands as a primitive in `marque-scheme`, these rows can re-classify
// from `Constraint::Custom` to the new primitive form without changing
// per-row semantics.

/// Floor policy for a class-floor catalog row.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ClassFloorPolicy {
    /// Classification level must be ≥ this floor (TS / S / C semantics).
    AtLeast(Classification),
    /// Classification must be exactly UNCLASSIFIED. Used by the UCNI
    /// ceiling rows.
    EqualsU,
}

/// One catalog row. The walker dispatches over the `&[ClassFloorRow]`
/// table; each row owns its presence predicate, floor policy, severity,
/// citation, and human-readable marking label for diagnostic messages.
///
/// # Naming-prefix invariant
///
/// Every row's `name` is a canonical predicate ID of the form
/// `banner.<axis>.<floor-or-ceiling>-<marking>`. Five axis ×
/// floor/ceiling discriminators appear in the catalog:
///
///   - `banner.classification.floor-<marking>` — SCI / SAR / NATO-SAP
///     classification-anchored rows.
///   - `banner.classification.floor-passthrough-<marking>` — unknown-
///     floor passthrough rows (BUR, HCS-X, KLM, MVL).
///   - `banner.aea.floor-<marking>` — AEA-anchored rows (RD, FRD,
///     TFNI, ATOMAL, CNWDI, RD-SG, FRD-SG).
///   - `banner.aea.ceiling-<marking>` — AEA ceiling rows (DOD-UCNI,
///     DOE-UCNI; classified-to-UNCLASSIFIED ceiling).
///   - `banner.dissem.floor-<marking>` — IC dissem rows (RSEN, IMCON,
///     ORCON family, EYES-ONLY).
///
/// The discriminator used by [`is_class_floor_catalog_name`] is the
/// `.floor-` / `.ceiling-` substring — uniquely scoped to this
/// catalog. The `class_floor_catalog_naming_convention` test in
/// `crates/capco/tests/class_floor_catalog.rs` enforces this at
/// build time; adding a row whose name doesn't match the convention
/// will fail CI.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ClassFloorRow {
    /// Catalog row name — matches the `Constraint::Custom { name }` of
    /// the same logical row. The name IS the canonical
    /// `(scheme="capco", predicate_id=name)` 2-tuple's predicate
    /// component; the engine's constraint-catalog bridge constructs
    /// `RuleId::new("capco", row.name)` directly. Must contain
    /// `.floor-` or `.ceiling-` per the naming convention above.
    pub(crate) name: &'static str,
    /// Human-readable marking name for the diagnostic message
    /// (e.g., `"CNWDI"`, `"HCS-P sub-compartment"`, `"BUR family"`).
    pub(crate) marking_label: &'static str,
    /// Marking-presence predicate.
    pub(crate) presence: fn(&marque_ism::CanonicalAttrs) -> bool,
    /// Floor policy.
    pub(crate) policy: ClassFloorPolicy,
    /// Per-row severity (`Error` for enumerated rows, `Warn` for
    /// passthrough rows).
    pub(crate) severity: marque_rules::Severity,
    /// Per-row typed §-citation, matching `Constraint::Custom { label }`.
    /// The catalog declaration is structurally constructed via
    /// `capco(...)` / `capco_section(...)` / `capco_table(...)`, and
    /// the engine's constraint-catalog bridge reads this field
    /// directly. Display via the [`Citation`](marque_scheme::Citation)
    /// `Display` impl produces the canonical `§<L>.<sub> p<page>`
    /// form when emitted into diagnostics.
    ///
    /// Passthrough rows (`passthrough: true`) use
    /// [`AuthoritativeSource::EngineInternal`](marque_scheme::AuthoritativeSource::EngineInternal)
    /// because their citation references the engine's own policy
    /// document — not a CAPCO-2016 anchor. The page number for
    /// passthrough rows is a synthetic `1` since the policy doc has no
    /// canonical page anchor; the stable identifier is the source kind,
    /// not the page.
    pub(crate) citation: marque_scheme::Citation,
    /// True for the unknown-floor passthrough rows. Drives the
    /// diagnostic message variant (passthrough rows quote the
    /// passthrough-policy framing).
    pub(crate) passthrough: bool,
    /// Diagnostic-span anchor token kind. Used by
    /// [`class_floor_anchor_span`] when populating
    /// `ConstraintViolation::span` in [`class_floor_emit`]. `None`
    /// means "fall back to the classification span" (NATO rows where
    /// the classification token IS the marking surface).
    pub(crate) primary_kind: Option<marque_ism::TokenKind>,

    // ----- Bitmask compilation fields (issue #650 tier-2) -----
    //
    // These fields compile the per-row structural presence predicate to a
    // bitmask fast path. The dispatcher in `predicates/class_floor.rs`
    // (`class_floor_catalog_eval`) reads them as follows:
    //
    //   1. FGI/JOINT early-out: their classification levels are absent from
    //      the bitmask chain fields; the structural path handles them.
    //   2. Trigger mask gate (O(1)): `(bits & bitmask_trigger) == 0` → no fire.
    //   3. Presence confirmation: for coarse-gate rows (`bitmask_trigger_exact:
    //      false`), call `presence(attrs)` to rule out false positives.
    //   4. Floor/ceiling test via chain extract.
    //
    // The four passthrough rows (BUR, HCS-X, KLM, MVL) keep
    // `bitmask_trigger: None` because their markings are open-vocab ISM-known
    // tokens outside the closed atom inventory — no dedicated bit exists.
    /// OR-of-atom-bits; when `(bits & bitmask_trigger) != 0`, the marking
    /// family this row gates on may be present. `None` for the four
    /// passthrough rows (open-vocab atoms outside the closed inventory).
    /// Coarse-gate rows (where the mask over-approximates) carry the coarse
    /// mask here; `bitmask_trigger_exact: false` signals that `presence()`
    /// must still confirm.
    pub(crate) bitmask_trigger: Option<u128>,

    /// `true` when `bitmask_trigger` is precisely equivalent to the row's
    /// `presence()` predicate — mask hit is the answer; no `presence()`
    /// call needed. `false` for coarse-grained masks.
    pub(crate) bitmask_trigger_exact: bool,
}

// ---------------------------------------------------------------------------
// The catalog — 27 rows at family granularity
// ---------------------------------------------------------------------------

/// Sentinel `Citation` for the four passthrough rows. Their `citation`
/// field references the engine's own policy (the
/// policy document, NOT CAPCO-2016), so the typed citation routes
/// through `AuthoritativeSource::EngineInternal`. The Display impl
/// drops the §/page suffix for this source, rendering as
/// `[engine-internal]`.
pub(crate) const PASSTHROUGH_CITATION: Citation = {
    // SectionRef::new(SectionLetter::A) with no subsection is a valid
    // bare-section reference; the AuthoritativeSource::EngineInternal
    // Display arm drops both §/page entirely. The page value is
    // synthetic; pick `1` as the stable sentinel.
    use marque_scheme::SectionRef;
    Citation::new(
        marque_scheme::AuthoritativeSource::EngineInternal,
        SectionRef::new(SectionLetter::A),
        match core::num::NonZeroU16::new(1) {
            Some(p) => p,
            None => unreachable!(),
        },
    )
};

pub(crate) const CLASS_FLOOR_CATALOG: &[ClassFloorRow] = &[
    // ---- Floor TS (5 rows) ------------------------------------
    ClassFloorRow {
        name: "banner.classification.floor-hcs-comp-sub",
        marking_label: "HCS sub-compartment markings",
        presence: presence_hcs_comp_sub,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 4, 60),
        // §H.4 section start — SCI grammar anchor for §3.4.6 family
        // floor invariants. The HCS-P sub-compartment guidance lives
        // at §H.4 p68 in the per-system block; the cross-system §3.4.6
        // anchor is the section's General Information at p60.
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        // SCI_HCS_P_SUB is the dedicated sentinel for HCS-P with
        // sub-compartments (bit 42). Exact: `presence_hcs_comp_sub`
        // fires exactly when this bit is set.
        bitmask_trigger: Some(1u128 << fact_bit::SCI_HCS_P_SUB),
        bitmask_trigger_exact: true,
    },
    ClassFloorRow {
        name: "banner.classification.floor-si-comp",
        marking_label: "SI compartments",
        presence: presence_si_comp,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 4, 60),
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        // SCI_SI_G (bit 40) gates on SI-G — the registered SI
        // compartment. Coarse: `presence_si_comp` also fires on
        // SI-ECRU and SI-NONBOOK which have no dedicated atom bit.
        bitmask_trigger: Some(1u128 << fact_bit::SCI_SI_G),
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "banner.classification.floor-tk-blfh",
        marking_label: "TK-BLFH (BLUEFISH)",
        presence: presence_tk_blfh,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 4, 60),
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        // SCI_TK_BLFH (bit 43) is the dedicated BLUEFISH sentinel.
        // Exact: `presence_tk_blfh` fires exactly when this bit is set.
        bitmask_trigger: Some(1u128 << fact_bit::SCI_TK_BLFH),
        bitmask_trigger_exact: true,
    },
    // BALK and BOHEMIA: NATO Special Access Programs per CAPCO-2016
    // §G.2 p40 + §H.7 p127. PR 9c.1 T134 corrected the structural
    // model — BALK/BOHEMIA now live in `sci_markings` as
    // `SciControlSystem::NatoSap` entries (not as fused
    // `NatoClassification::*Balk/*Bohemia` variants which were retired
    // as a wrong fusion of classification and control-marking
    // semantics). The presence predicates fire on the SCI axis;
    // the floor checks effective US-equivalent classification level
    // (typically TS for NATO SAPs per §G.2 p40).
    //
    // Severity = Warn at the catalog row level: §G.2 p40's citation depth is
    // too soft to drive Error — the manual identifies BOHEMIA/BALK as
    // SAPs and lists them in the ARH table but does not enumerate a
    // classification floor with the precision §H.6 has for RD/CNWDI.
    // A Warn-with-suggest fires when the data is structurally
    // inconsistent (BALK/BOHEMIA marked but classification < TS) and
    // surfaces an actionable suggestion without blocking.
    ClassFloorRow {
        name: "banner.classification.floor-balk",
        marking_label: "BALK (NATO)",
        presence: presence_balk,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Warn,
        citation: capco(SectionLetter::G, 2, 40),
        passthrough: false,
        // `None` falls through to the Classification token span. The
        // parser writes the BALK SciMarking but does
        // not push a `TokenKind::SciSystem` span for the legacy
        // compound text (`CTS-BALK` is a single Classification token
        // that carries both the bare-class and the companion semantic);
        // anchoring at the Classification token is the right UX.
        primary_kind: None,
        // AEA_BALK (bit 50) is the NATO SAP sentinel for BALK.
        // Exact: `presence_balk` fires exactly when this bit is set.
        bitmask_trigger: Some(1u128 << fact_bit::AEA_BALK),
        bitmask_trigger_exact: true,
    },
    ClassFloorRow {
        name: "banner.classification.floor-bohemia",
        marking_label: "BOHEMIA (NATO)",
        presence: presence_bohemia,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Warn,
        citation: capco(SectionLetter::G, 2, 40),
        passthrough: false,
        primary_kind: None,
        // AEA_BOHEMIA (bit 49) is the NATO SAP sentinel for BOHEMIA.
        // Exact: `presence_bohemia` fires exactly when this bit is set.
        bitmask_trigger: Some(1u128 << fact_bit::AEA_BOHEMIA),
        bitmask_trigger_exact: true,
    },
    // ---- Floor S (8 rows) -------------------------------------
    ClassFloorRow {
        name: "banner.classification.floor-hcs-comp",
        marking_label: "HCS-O / HCS-P (compartment, no sub-compartment)",
        presence: presence_hcs_comp_only,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 4, 60),
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        // SCI_PRESENT (bit 37): coarse gate. HCS-O sets SCI_HCS_O (41)
        // which also sets SCI_PRESENT; bare HCS-P (no sub-compartments)
        // sets SCI_PRESENT only (SCI_HCS_P_SUB bit 42 requires sub-comps).
        // Using SCI_HCS_O alone would miss bare HCS-P inputs.
        // `presence_hcs_comp_only` confirms: rejects non-HCS, HCS-X,
        // and HCS-P-with-sub-compartments (covered by HCS-comp-sub).
        bitmask_trigger: Some(1u128 << fact_bit::SCI_PRESENT),
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "banner.classification.floor-rsv-comp",
        marking_label: "RSV compartment",
        presence: presence_rsv_comp,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 4, 60),
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        // SCI_PRESENT (bit 37): coarse gate; no `SCI_RSV` atom exists
        // in the closed inventory. `presence_rsv_comp` confirms by
        // checking `sci_markings` for a Published(Rsv) entry with
        // non-empty compartments. Per Q5: adding a `SCI_RSV` atom
        // would expand the closure-rule suppressor surface; deferred.
        bitmask_trigger: Some(1u128 << fact_bit::SCI_PRESENT),
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "banner.classification.floor-tk",
        marking_label: "TK / TK-IDIT / TK-KAND",
        presence: presence_tk_family,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 4, 60),
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        // SCI_PRESENT (bit 37): coarse gate covering all TK forms —
        // bare TK, TK-IDIT, TK-KAND, and TK-BLFH. Bare TK (no
        // compartments) sets only SCI_PRESENT; the specific sentinel
        // bits (SCI_TK_BLFH/IDIT/KAND) are only set when those named
        // compartments appear. Using SCI_PRESENT ensures bare TK is
        // not missed. Coarse: `presence_tk_family` confirms (excludes
        // TK-BLFH and non-TK SCI systems). False-positive rate is
        // higher than using the specific bits alone, but correctness
        // requires it for the bare-TK path.
        bitmask_trigger: Some(1u128 << fact_bit::SCI_PRESENT),
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "banner.aea.floor-rd-sg",
        marking_label: "RD-SIGMA",
        presence: presence_rd_sigma,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        // RD-SIGMA marking template lives on §H.6 p108; FRD-SIGMA on
        // p113 (the latter retains p113 below).
        citation: capco(SectionLetter::H, 6, 108),
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        // AEA_RD (bit 22): coarse gate. Fires when any RD marking is
        // present. Coarse: `presence_rd_sigma` confirms by checking
        // `rd.sigma.is_empty() == false` (bare RD has no sigma numbers).
        bitmask_trigger: Some(1u128 << fact_bit::AEA_RD),
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "banner.aea.floor-frd-sg",
        marking_label: "FRD-SIGMA",
        presence: presence_frd_sigma,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 6, 113),
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        // AEA_FRD (bit 23): coarse gate. Mirror of RD-SG for FRD.
        // `presence_frd_sigma` confirms by checking sigma non-empty.
        bitmask_trigger: Some(1u128 << fact_bit::AEA_FRD),
        bitmask_trigger_exact: false,
    },
    // CNWDI — replaces retired E022. Walker-prefixed name per PM
    // directive #5.
    ClassFloorRow {
        name: "banner.aea.floor-cnwdi",
        marking_label: "CNWDI",
        presence: presence_rd_cnwdi,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 6, 104),
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        // AEA_RD (bit 22): coarse gate. CNWDI is a sub-classification
        // within RD (`rd.cnwdi == true`); any RD marker may carry it.
        // `presence_rd_cnwdi` confirms by checking `rd.cnwdi`.
        bitmask_trigger: Some(1u128 << fact_bit::AEA_RD),
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "banner.dissem.floor-rsen",
        marking_label: "RSEN",
        presence: presence_rsen,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 8, 149),
        passthrough: false,
        primary_kind: Some(TokenKind::DissemControl),
        // RSEN (bit 6) is the closed-vocab IC dissem sentinel.
        // Exact: `presence_rsen` fires exactly when this bit is set.
        bitmask_trigger: Some(1u128 << fact_bit::RSEN),
        bitmask_trigger_exact: true,
    },
    ClassFloorRow {
        name: "banner.dissem.floor-imcon",
        marking_label: "IMCON",
        presence: presence_imcon,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 8, 144),
        passthrough: false,
        primary_kind: Some(TokenKind::DissemControl),
        // IMCON (bit 7) is the closed-vocab IC dissem sentinel.
        // Exact: `presence_imcon` fires exactly when this bit is set.
        bitmask_trigger: Some(1u128 << fact_bit::IMCON),
        bitmask_trigger_exact: true,
    },
    // ---- Floor C (8 rows) -------------------------------------
    ClassFloorRow {
        name: "banner.classification.floor-si",
        marking_label: "SI (bare)",
        presence: presence_si_bare,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 4, 60),
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        // SCI_PRESENT (bit 37): coarse gate; bare SI (no compartments)
        // has no dedicated atom bit. `presence_si_bare` confirms by
        // checking that SI is present with no compartments or
        // sub-compartments.
        bitmask_trigger: Some(1u128 << fact_bit::SCI_PRESENT),
        bitmask_trigger_exact: false,
    },
    // SAR — replaces retired E027.
    ClassFloorRow {
        name: "banner.classification.floor-sar",
        marking_label: "SAR",
        presence: presence_sar,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 5, 99),
        // §H.5 section start — SAR section anchor (no per-page
        // sub-section was specified in the `citation` string; the
        // citation index puts §H.5 start at p99).
        passthrough: false,
        primary_kind: Some(TokenKind::SarIndicator),
        // SAR_PRESENT (bit 36) is the structural SAR sentinel.
        // Exact: `presence_sar` fires exactly when this bit is set.
        bitmask_trigger: Some(1u128 << fact_bit::SAR_PRESENT),
        bitmask_trigger_exact: true,
    },
    ClassFloorRow {
        name: "banner.aea.floor-rd",
        marking_label: "RD",
        presence: presence_rd_bare,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 6, 104),
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        // AEA_RD (bit 22): coarse gate. Fires when any RD marking is
        // present. Coarse: `presence_rd_bare` excludes CNWDI (rd.cnwdi)
        // and SIGMA (rd.sigma non-empty); the mask over-approximates.
        bitmask_trigger: Some(1u128 << fact_bit::AEA_RD),
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "banner.aea.floor-frd",
        marking_label: "FRD",
        presence: presence_frd_bare,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 6, 104),
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        // AEA_FRD (bit 23): coarse gate. Mirror of RD for FRD.
        // `presence_frd_bare` excludes FRD-SIGMA; the mask
        // over-approximates.
        bitmask_trigger: Some(1u128 << fact_bit::AEA_FRD),
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "banner.aea.floor-tfni",
        marking_label: "TFNI",
        presence: presence_tfni,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 6, 107),
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        // AEA_TFNI (bit 24) is the closed-vocab TFNI sentinel.
        // Exact: `presence_tfni` fires exactly when this bit is set.
        bitmask_trigger: Some(1u128 << fact_bit::AEA_TFNI),
        bitmask_trigger_exact: true,
    },
    // ATOMAL: AEA-axis marking per
    // CAPCO-2016 §H.7 p122 worked example
    // (`SECRET//RD/ATOMAL//FGI NATO//NOFORN`). The class floor is the
    // same Confidential lower-bound as the rest of §H.6's AEA family
    // (RD/FRD/TFNI). Severity stays `Error` because §H.7 p122 is a
    // direct, worked-example-grounded citation (parallel depth to
    // §H.6's class-floor citations for RD/FRD), distinguishing it from
    // the softer §G.2 p40 BALK/BOHEMIA citation.
    //
    // `primary_kind: None` (falls back to Classification): same
    // rationale as BALK/BOHEMIA — legacy compound text like `NCA` /
    // `CTSA` is a single `TokenKind::Classification` carrying both
    // the bare-class and the AEA companion semantic; the parser does
    // not emit a separate `TokenKind::AeaMarking` span for the
    // canonicalized companion write. Anchoring at the Classification
    // token is the right UX for the legacy-compound case.
    ClassFloorRow {
        name: "banner.aea.floor-atomal",
        marking_label: "ATOMAL (NATO)",
        presence: presence_atomal,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 7, 122),
        passthrough: false,
        primary_kind: None,
        // AEA_ATOMAL (bit 48) is the NATO AEA sentinel for ATOMAL.
        // Exact: `presence_atomal` fires exactly when this bit is set.
        bitmask_trigger: Some(1u128 << fact_bit::AEA_ATOMAL),
        bitmask_trigger_exact: true,
    },
    ClassFloorRow {
        name: "banner.dissem.floor-orcon",
        marking_label: "ORCON / ORCON-USGOV",
        presence: presence_orcon_family,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 8, 136),
        passthrough: false,
        primary_kind: Some(TokenKind::DissemControl),
        // ORCON (bit 3) | ORCON_USGOV (bit 4): covers the ORCON
        // family per §H.8 p136/p140. Exact: `presence_orcon_family`
        // is equivalent to `dissem.contains(Oc) || dissem.contains(OcUsgov)`.
        bitmask_trigger: Some((1u128 << fact_bit::ORCON) | (1u128 << fact_bit::ORCON_USGOV)),
        bitmask_trigger_exact: true,
    },
    ClassFloorRow {
        name: "banner.dissem.floor-eyes-only",
        marking_label: "EYES ONLY",
        presence: presence_eyes_only,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 8, 152),
        passthrough: false,
        primary_kind: Some(TokenKind::DissemControl),
        // EYES (bit 5) is the closed-vocab IC dissem sentinel for
        // EYES ONLY per §H.8 p157. Exact: `presence_eyes_only` fires
        // exactly when this bit is set.
        bitmask_trigger: Some(1u128 << fact_bit::EYES),
        bitmask_trigger_exact: true,
    },
    // ---- Floor =U (2 rows; UCNI split) ----------
    ClassFloorRow {
        name: "banner.aea.ceiling-dod-ucni",
        marking_label: "DOD UCNI",
        presence: presence_dod_ucni,
        policy: ClassFloorPolicy::EqualsU,
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 6, 116),
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        // AEA_DOD_UCNI (bit 26) is the closed-vocab DOD UCNI sentinel.
        // Exact: `presence_dod_ucni` fires exactly when this bit is set.
        // EqualsU policy: `extract_us_class_level(bits) == Some(Unclassified)`
        // (US chain == 1) — UCNI is restricted to US-UNCLASSIFIED portions
        // per §H.6 p116 (DOD UCNI is US AEA, not NATO-applicable).
        bitmask_trigger: Some(1u128 << fact_bit::AEA_DOD_UCNI),
        bitmask_trigger_exact: true,
    },
    ClassFloorRow {
        name: "banner.aea.ceiling-doe-ucni",
        marking_label: "DOE UCNI",
        presence: presence_doe_ucni,
        policy: ClassFloorPolicy::EqualsU,
        severity: marque_rules::Severity::Error,
        citation: capco(SectionLetter::H, 6, 118),
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        // AEA_DOE_UCNI (bit 25) is the closed-vocab DOE UCNI sentinel.
        // Exact: `presence_doe_ucni` fires exactly when this bit is set.
        // EqualsU policy: mirrors DOD UCNI — DOE UCNI is US AEA,
        // restricted to US-UNCLASSIFIED portions per §H.6 p118.
        bitmask_trigger: Some(1u128 << fact_bit::AEA_DOE_UCNI),
        bitmask_trigger_exact: true,
    },
    // ---- Unknown-floor passthrough (4 rows; Warn) ---------------
    //
    // These rows cite an engine-internal policy
    // ([`PASSTHROUGH_CITATION`]), not a CAPCO section — the markings
    // are ISM-known tokens with no enumerated CAPCO classification
    // floor. The citation renders as `[engine-internal]` via
    // `AuthoritativeSource::EngineInternal`.
    ClassFloorRow {
        name: "banner.classification.floor-passthrough-bur",
        marking_label: "BUR family",
        presence: presence_passthrough_bur,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Warn,
        citation: PASSTHROUGH_CITATION,
        // Passthrough rows reference the engine policy doc, not
        // CAPCO-2016. Routes through AuthoritativeSource::EngineInternal
        // so Display renders `[engine-internal]`.
        passthrough: true,
        primary_kind: Some(TokenKind::SciSystem),
        // Passthrough rows have no atom bit in the closed inventory;
        // their markings are open-vocab ISM-known tokens. Structural
        // fallthrough path only.
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "banner.classification.floor-passthrough-hcs-x",
        marking_label: "HCS-X",
        presence: presence_passthrough_hcs_x,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Warn,
        citation: PASSTHROUGH_CITATION,
        passthrough: true,
        primary_kind: Some(TokenKind::SciSystem),
        // HCS-X presence requires compartment-string read (`identifier == "X"`);
        // no SCI_HCS_X atom exists for a pure-bitmask test. Structural only.
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "banner.classification.floor-passthrough-klm",
        marking_label: "KLM family",
        presence: presence_passthrough_klm,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Warn,
        citation: PASSTHROUGH_CITATION,
        passthrough: true,
        primary_kind: Some(TokenKind::SciSystem),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "banner.classification.floor-passthrough-mvl",
        marking_label: "MVL",
        presence: presence_passthrough_mvl,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Warn,
        citation: PASSTHROUGH_CITATION,
        passthrough: true,
        primary_kind: Some(TokenKind::SciSystem),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
];
