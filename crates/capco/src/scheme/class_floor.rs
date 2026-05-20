// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Class-floor catalog — `ClassFloorPolicy` + `ClassFloorRow` + the
//! 27-row `CLASS_FLOOR_CATALOG`.
//!
//! Carved out from `scheme/mod.rs` per the Stage 2 PR B hub-split
//! (issue #466). Module contents are byte-identical to the pre-split
//! source — imports adjusted to pick up the `presence_*` helpers from
//! `super::predicates::*` (the same glob `mod.rs` used pre-split) and
//! the `Classification` / `TokenKind` types directly from `marque_ism`.

use marque_ism::{Classification, TokenKind};
use marque_rules::{Citation, SectionLetter, capco};

use crate::fact_bitmask::fact_bit;

use super::predicates::*;

// ===========================================================================
// PR 3b.D (T026d) — Class-floor catalog dispatch (§3.4.6)
// ===========================================================================
//
// `class_floor_catalog_eval` is the static-table dispatcher for the 27
// `Constraint::Custom` rows declared by `build_constraints` under the
// "PR 3b.D (T026d) — class-floor catalog (§3.4.6)" section header.
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
//      passthrough rows (§3.4.6 Q-3.4.6b)
//   - `citation`: per-row §-citation matching `Constraint::Custom { label }`
//   - `passthrough`: `true` for unknown-floor passthrough rows (drives the
//      diagnostic message variant)
//
// The catalog is consumed by the engine's class-floor bridge, which
// reads `CapcoScheme::has_diagnostic_constraints` to short-circuit
// the walk when the catalog has nothing to fire and otherwise runs
// the standard `MarkingScheme::validate()` → `Vec<ConstraintViolation>`
// path. PR 3c.B Commit 7.3 retired the original
// `DeclarativeClassFloorRule` walker; the bridge is the only consumer
// today.
//
// FORWARD LINK to PR 3.7 (T108b): once `TokenRef::ClassAtLeast(ClassLevel)`
// or `Constraint::ClassFloor` lands as a primitive in `marque-scheme`,
// these rows can re-classify from `Constraint::Custom` to the new
// primitive form without changing per-row semantics. See
// `docs/plans/2026-05-08-pr3b-D-class-floor-catalog-plan.md` §3 for the
// architectural rationale.

/// Floor policy for a class-floor catalog row.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ClassFloorPolicy {
    /// Classification level must be ≥ this floor (TS / S / C semantics).
    AtLeast(Classification),
    /// Classification must be exactly UNCLASSIFIED. Used by the UCNI
    /// ceiling rows (§2.4 of the planning doc).
    EqualsU,
}

/// One catalog row. The walker dispatches over the `&[ClassFloorRow]`
/// table; each row owns its presence predicate, floor policy, severity,
/// citation, and human-readable marking label for diagnostic messages.
///
/// # Naming-prefix invariant (PR D R3.2)
///
/// Every row's `name` MUST start with one of two prefixes:
///
///   - **`E058/<purpose>`** — for rows replacing a retired legacy rule
///     (the four E022 / E025 / E027 successors:
///     `E058/CNWDI-classification-floor`,
///     `E058/SAR-classification-floor`,
///     `E058/DOD-UCNI-classification-ceiling`,
///     `E058/DOE-UCNI-classification-ceiling`).
///   - **`class-floor/<marking>`** — for rows with no retired-rule
///     predecessor (e.g., `class-floor/HCS-comp-sub`,
///     `class-floor/SI-comp`, `class-floor/BALK`,
///     `class-floor/passthrough-BUR`).
///
/// The prefix invariant is what makes the
/// [`is_class_floor_catalog_name`] dispatch routing O(1) instead of
/// a linear catalog scan. The
/// `class_floor_catalog_naming_convention` test in
/// `crates/capco/tests/class_floor_catalog.rs` enforces this at
/// build time; adding a row whose name doesn't match either prefix
/// will fail CI.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ClassFloorRow {
    /// Catalog row name — matches the `Constraint::Custom { name }` of
    /// the same logical row. MUST start with `E058/` or
    /// `class-floor/` per the naming-prefix invariant above.
    pub(crate) name: &'static str,
    /// Human-readable marking name for the diagnostic message
    /// (e.g., `"CNWDI"`, `"HCS-P sub-compartment"`, `"BUR family"`).
    pub(crate) marking_label: &'static str,
    /// Marking-presence predicate.
    pub(crate) presence: fn(&marque_ism::CanonicalAttrs) -> bool,
    /// Floor policy.
    pub(crate) policy: ClassFloorPolicy,
    /// Per-row severity (`Error` for enumerated rows, `Warn` for
    /// passthrough rows per §3.4.6 Q-3.4.6b).
    pub(crate) severity: marque_rules::Severity,
    /// Per-row §-citation, matching `Constraint::Custom { label }`.
    /// PR 3c.2.C C7 retired the bridge-emission path through this
    /// field per PM-C-1 R-C1 (catalog row citations stay `&'static str`
    /// for citation-lint scanning); use [`Self::citation_typed`] at
    /// emit time.
    pub(crate) citation: &'static str,
    /// Typed [`marque_rules::Citation`] used at emission time. Must
    /// agree with [`Self::citation`]. Per PR 3c.2.C C7 the engine
    /// bridge (`message_by_name`/`citation_by_name`) reads this field
    /// so the bridge-emitted `Diagnostic.citation` carries the real
    /// per-row CAPCO anchor instead of the `[engine-internal]`
    /// sentinel fallback (R-C1).
    ///
    /// Passthrough rows (`passthrough: true`) use
    /// [`AuthoritativeSource::EngineInternal`](marque_rules::AuthoritativeSource::EngineInternal)
    /// because their `citation` field references `marque-applied.md
    /// Section 3.7`, the engine's own policy document — not a CAPCO-2016
    /// anchor. The page number for passthrough rows is a synthetic `1`
    /// since marque-applied.md has no canonical page anchor; the
    /// stable identifier is the source kind, not the page.
    pub(crate) citation_typed: marque_rules::Citation,
    /// True for the unknown-floor passthrough rows. Drives the
    /// diagnostic message variant (passthrough rows quote the §3.7
    /// passthrough-policy framing).
    pub(crate) passthrough: bool,
    /// Diagnostic-span anchor token kind. Used by
    /// [`class_floor_anchor_span`] when populating
    /// `ConstraintViolation::span` in [`class_floor_emit`]. `None`
    /// means "fall back to the classification span" (NATO rows where
    /// the classification token IS the marking surface).
    pub(crate) primary_kind: Option<marque_ism::TokenKind>,

    // ----- Bitmask compilation fields (PR-G / issue #650 tier-2) -----
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
// The catalog — 27 rows at §3.4.6 family granularity
// ---------------------------------------------------------------------------

/// Sentinel `Citation` for the four passthrough rows. Their `citation`
/// field references `marque-applied.md Section 3.7` (the engine's own
/// policy document, NOT CAPCO-2016), so the typed citation routes
/// through `AuthoritativeSource::EngineInternal`. The Display impl
/// drops the §/page suffix for this source, rendering as
/// `[engine-internal]`.
const PASSTHROUGH_CITATION: Citation = {
    // SectionRef::new(SectionLetter::A) with no subsection is a valid
    // bare-section reference; the AuthoritativeSource::EngineInternal
    // Display arm drops both §/page entirely. The page value is
    // synthetic; pick `1` as the stable sentinel.
    use marque_rules::SectionRef;
    Citation::new(
        marque_rules::AuthoritativeSource::EngineInternal,
        SectionRef::new(SectionLetter::A),
        match core::num::NonZeroU16::new(1) {
            Some(p) => p,
            None => unreachable!(),
        },
    )
};

pub(crate) const CLASS_FLOOR_CATALOG: &[ClassFloorRow] = &[
    // ---- §2.1 Floor TS (5 rows) ------------------------------------
    ClassFloorRow {
        name: "class-floor/HCS-comp-sub",
        marking_label: "HCS sub-compartment markings",
        presence: presence_hcs_comp_sub,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        // §H.4 section start — SCI grammar anchor for §3.4.6 family
        // floor invariants. The HCS-P sub-compartment guidance lives
        // at §H.4 p68 in the per-system block; the cross-system §3.4.6
        // anchor is the section's General Information at p60.
        citation_typed: capco(SectionLetter::H, 4, 60),
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        // SCI_HCS_P_SUB is the dedicated sentinel for HCS-P with
        // sub-compartments (bit 42). Exact: `presence_hcs_comp_sub`
        // fires exactly when this bit is set.
        bitmask_trigger: Some(1u128 << fact_bit::SCI_HCS_P_SUB),
        bitmask_trigger_exact: true,
    },
    ClassFloorRow {
        name: "class-floor/SI-comp",
        marking_label: "SI compartments",
        presence: presence_si_comp,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        citation_typed: capco(SectionLetter::H, 4, 60),
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        // SCI_SI_G (bit 40) gates on SI-G — the registered SI
        // compartment. Coarse: `presence_si_comp` also fires on
        // SI-ECRU and SI-NONBOOK which have no dedicated atom bit.
        bitmask_trigger: Some(1u128 << fact_bit::SCI_SI_G),
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "class-floor/TK-BLFH",
        marking_label: "TK-BLFH (BLUEFISH)",
        presence: presence_tk_blfh,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        citation_typed: capco(SectionLetter::H, 4, 60),
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
    // Severity = Warn at the catalog row level per PR 9c.1 D5 (the
    // architect's pre-flight decision): §G.2 p40's citation depth is
    // too soft to drive Error — the manual identifies BOHEMIA/BALK as
    // SAPs and lists them in the ARH table but does not enumerate a
    // classification floor with the precision §H.6 has for RD/CNWDI.
    // A Warn-with-suggest fires when the data is structurally
    // inconsistent (BALK/BOHEMIA marked but classification < TS) and
    // surfaces an actionable suggestion without blocking.
    ClassFloorRow {
        name: "class-floor/BALK",
        marking_label: "BALK (NATO)",
        presence: presence_balk,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §G.2 p40",
        citation_typed: capco(SectionLetter::G, 2, 40),
        passthrough: false,
        // `None` falls through to the Classification token span. PR
        // 9c.1 Commit 3's parser writes the BALK SciMarking but does
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
        name: "class-floor/BOHEMIA",
        marking_label: "BOHEMIA (NATO)",
        presence: presence_bohemia,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §G.2 p40",
        citation_typed: capco(SectionLetter::G, 2, 40),
        passthrough: false,
        primary_kind: None,
        // AEA_BOHEMIA (bit 49) is the NATO SAP sentinel for BOHEMIA.
        // Exact: `presence_bohemia` fires exactly when this bit is set.
        bitmask_trigger: Some(1u128 << fact_bit::AEA_BOHEMIA),
        bitmask_trigger_exact: true,
    },
    // ---- §2.2 Floor S (8 rows) -------------------------------------
    ClassFloorRow {
        name: "class-floor/HCS-comp",
        marking_label: "HCS-O / HCS-P (compartment, no sub-compartment)",
        presence: presence_hcs_comp_only,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        citation_typed: capco(SectionLetter::H, 4, 60),
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "class-floor/RSV-comp",
        marking_label: "RSV compartment",
        presence: presence_rsv_comp,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        citation_typed: capco(SectionLetter::H, 4, 60),
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "class-floor/TK",
        marking_label: "TK / TK-IDIT / TK-KAND",
        presence: presence_tk_family,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        citation_typed: capco(SectionLetter::H, 4, 60),
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "class-floor/RD-SG",
        marking_label: "RD-SIGMA",
        presence: presence_rd_sigma,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p113",
        citation_typed: capco(SectionLetter::H, 6, 113),
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "class-floor/FRD-SG",
        marking_label: "FRD-SIGMA",
        presence: presence_frd_sigma,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p113",
        citation_typed: capco(SectionLetter::H, 6, 113),
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    // CNWDI — replaces retired E022. Walker-prefixed name per PM
    // directive #5.
    ClassFloorRow {
        name: "E058/CNWDI-classification-floor",
        marking_label: "CNWDI",
        presence: presence_rd_cnwdi,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p104",
        citation_typed: capco(SectionLetter::H, 6, 104),
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "class-floor/RSEN",
        marking_label: "RSEN",
        presence: presence_rsen,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.8 p149",
        citation_typed: capco(SectionLetter::H, 8, 149),
        passthrough: false,
        primary_kind: Some(TokenKind::DissemControl),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "class-floor/IMCON",
        marking_label: "IMCON",
        presence: presence_imcon,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.8 p144",
        citation_typed: capco(SectionLetter::H, 8, 144),
        passthrough: false,
        primary_kind: Some(TokenKind::DissemControl),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    // ---- §2.3 Floor C (8 rows) -------------------------------------
    ClassFloorRow {
        name: "class-floor/SI",
        marking_label: "SI (bare)",
        presence: presence_si_bare,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        citation_typed: capco(SectionLetter::H, 4, 60),
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    // SAR — replaces retired E027.
    ClassFloorRow {
        name: "E058/SAR-classification-floor",
        marking_label: "SAR",
        presence: presence_sar,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.5",
        // §H.5 section start — SAR section anchor (no per-page
        // sub-section was specified in the `citation` string; the
        // citation index puts §H.5 start at p99).
        citation_typed: capco(SectionLetter::H, 5, 99),
        passthrough: false,
        primary_kind: Some(TokenKind::SarIndicator),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "class-floor/RD",
        marking_label: "RD",
        presence: presence_rd_bare,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p104",
        citation_typed: capco(SectionLetter::H, 6, 104),
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "class-floor/FRD",
        marking_label: "FRD",
        presence: presence_frd_bare,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p104",
        citation_typed: capco(SectionLetter::H, 6, 104),
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "class-floor/TFNI",
        marking_label: "TFNI",
        presence: presence_tfni,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p107",
        citation_typed: capco(SectionLetter::H, 6, 107),
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    // ATOMAL: PR 9c.1 T134 reclassified as AEA-axis marking per
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
        name: "class-floor/ATOMAL",
        marking_label: "ATOMAL (NATO)",
        presence: presence_atomal,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.7 p122",
        citation_typed: capco(SectionLetter::H, 7, 122),
        passthrough: false,
        primary_kind: None,
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "class-floor/ORCON",
        marking_label: "ORCON / ORCON-USGOV",
        presence: presence_orcon_family,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.8 p136",
        citation_typed: capco(SectionLetter::H, 8, 136),
        passthrough: false,
        primary_kind: Some(TokenKind::DissemControl),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "class-floor/EYES-ONLY",
        marking_label: "EYES ONLY",
        presence: presence_eyes_only,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.8 p152",
        citation_typed: capco(SectionLetter::H, 8, 152),
        passthrough: false,
        primary_kind: Some(TokenKind::DissemControl),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    // ---- §2.4 Floor =U (2 rows; UCNI split per PM decision) ----------
    ClassFloorRow {
        name: "E058/DOD-UCNI-classification-ceiling",
        marking_label: "DOD UCNI",
        presence: presence_dod_ucni,
        policy: ClassFloorPolicy::EqualsU,
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p116",
        citation_typed: capco(SectionLetter::H, 6, 116),
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "E058/DOE-UCNI-classification-ceiling",
        marking_label: "DOE UCNI",
        presence: presence_doe_ucni,
        policy: ClassFloorPolicy::EqualsU,
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p118",
        citation_typed: capco(SectionLetter::H, 6, 118),
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    // ---- §2.6 Unknown-floor passthrough (4 rows; Warn) ---------------
    //
    // `row.citation` uses the `Section 3.7` form (not `§3.7`) because
    // the citation-lint tool (FR-018) parses `§N.M` in `citation:`
    // struct-field literals as a CAPCO section reference and would
    // flag `§3` as a bare section without subsection letter (CAPCO
    // sections are A-K, not digits). The cross-document
    // `marque-applied.md` prefix doesn't currently disambiguate.
    //
    // The corresponding `Constraint::Custom { label: "marque-applied.md §3.7 ..." }`
    // entries in `build_constraints()` keep the `§3.7` form because
    // the lint only scans `citation:`, `message:`, and
    // `constraint_label:` struct fields (not `label:`). The bridge's
    // user-visible `Diagnostic.citation` IS the `§3.7` form because
    // `marque_scheme::constraint::evaluate` overrides
    // `ConstraintViolation::citation` from the constraint's `label`
    // field after `evaluate_custom` returns — so the lint is happy
    // AND end users see the canonical `§3.7` form. `row.citation` is
    // internal scratch (never user-visible post-7.3) after the
    // `evaluate` override step.
    //
    // Tracking issue: the citation-lint tool's CAPCO-context
    // implicit-treatment of `citation:` fields should learn to
    // recognize cross-document prefixes (`<word>.md §`) so the
    // `§3.7` form can be used uniformly. Not in scope here.
    ClassFloorRow {
        name: "class-floor/passthrough-BUR",
        marking_label: "BUR family",
        presence: presence_passthrough_bur,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Warn,
        citation: "marque-applied.md Section 3.7 (passthrough); CAPCO-2016 unmapped",
        // Passthrough rows reference marque-applied.md (engine policy
        // doc), not CAPCO-2016. Routes through AuthoritativeSource::
        // EngineInternal so Display renders `[engine-internal]`.
        citation_typed: PASSTHROUGH_CITATION,
        passthrough: true,
        primary_kind: Some(TokenKind::SciSystem),
        // Passthrough rows have no atom bit in the closed inventory;
        // their markings are open-vocab ISM-known tokens. Structural
        // fallthrough path only.
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "class-floor/passthrough-HCS-X",
        marking_label: "HCS-X",
        presence: presence_passthrough_hcs_x,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Warn,
        citation: "marque-applied.md Section 3.7 (passthrough); CAPCO-2016 unmapped",
        citation_typed: PASSTHROUGH_CITATION,
        passthrough: true,
        primary_kind: Some(TokenKind::SciSystem),
        // HCS-X presence requires compartment-string read (`identifier == "X"`);
        // no SCI_HCS_X atom exists for a pure-bitmask test. Structural only.
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "class-floor/passthrough-KLM",
        marking_label: "KLM family",
        presence: presence_passthrough_klm,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Warn,
        citation: "marque-applied.md Section 3.7 (passthrough); CAPCO-2016 unmapped",
        citation_typed: PASSTHROUGH_CITATION,
        passthrough: true,
        primary_kind: Some(TokenKind::SciSystem),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
    ClassFloorRow {
        name: "class-floor/passthrough-MVL",
        marking_label: "MVL",
        presence: presence_passthrough_mvl,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Warn,
        citation: "marque-applied.md Section 3.7 (passthrough); CAPCO-2016 unmapped",
        citation_typed: PASSTHROUGH_CITATION,
        passthrough: true,
        primary_kind: Some(TokenKind::SciSystem),
        bitmask_trigger: None,
        bitmask_trigger_exact: false,
    },
];
