// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CAPCO closure-rule catalog — `FDR_DOMINATORS` + 7× `CLOSURE_NOFORN_*` +
//! the aggregating `CAPCO_CLOSURE_RULES` static.
//!
//! Carved out from `scheme/mod.rs` per the Stage 2 PR B hub-split
//! (issue #466). Module contents are byte-identical to the pre-split
//! source — imports adjusted to reach `marque_scheme::ClosureRule` /
//! `Severity` / `TokenRef` directly and to pick up the `CAT_*` /
//! `TOK_*` constants from the parent module via `use super::*;`.

use marque_scheme::{ClosureRule, Severity, TokenRef};

use super::*;

// ---------------------------------------------------------------------------
// Stage D (PR 3.7 T108c) — Closure-rule catalog + family predicates
// ---------------------------------------------------------------------------
//
// The CAPCO §4.7 implicit-fact propagation catalog. See
// `docs/plans/2026-05-01-lattice-design.md` §3 (e) and
// `marque-applied.md` §4.7 for the algebraic treatment.
//
// Engine wiring at `Engine::project` is deferred to PR 4 (T112). This
// module ships the catalog data; the `MarkingScheme::closure_rules()`
// impl on `CapcoScheme` exposes it as the public catalog surface per D18.

// --- Shared suppressor slices ---
//
// FD&R-dominator family: any of these present on a marking/page means an
// explicit FD&R decision exists; the implicit-default trio (Trio 1, 2, 3)
// should NOT fire. Per CAPCO-2016 §B.3.a p19 (canonical enumeration —
// "NOFORN, REL TO, RELIDO, or DISPLAY ONLY"), §B.3 Table 2 pp 21-22
// (scenario-summary table, derivative), and `marque-applied.md` §4.7.1.
//
// Includes:
//   - NOFORN (most restrictive FD&R, top of chain per §H.8 p145)
//   - RELIDO (deferred-release per SFDRA arrangement, §H.8 p154)
//   - DISPLAY ONLY (viewing-only FD&R, §H.8 p163)
//   - REL TO (any country list; `AnyInCategory` covers all partial lists,
//     §H.8 p150)
//   - EYES (US/[LIST] EYES ONLY is an FD&R marking at §H.8 p157)
//
// Note: LES-NF and SBU-NF are NOT included. They are non-IC dissem controls
// that carry NOFORN treatment via PageRewrite, not FD&R markers themselves.
// §B.3.a p19 is the authoritative enumeration of the FD&R set; §B.3 Table 2
// pp 21-22 is the per-scenario marking-summary table (derivative, not the
// definition).
//
// Algebraic note (re: `marque-applied.md` §4.7.3 has_fdr definition):
// §4.7.3 defines `has_fdr(x)` to include LES-NF / SBU-NF for the
// table-design-property monotonicity proof. The in-tree FDR_DOMINATORS
// omits them because (a) LES-NF and SBU-NF entail NOFORN through their
// own PageRewrite (so the operational behavior is preserved — when LES-NF
// is present, NOFORN is added via PageRewrite, and the Trio-1 row would
// then be suppressed by the post-PageRewrite NOFORN regardless), and
// (b) the §4.7.3 case-2 table-design property is preserved per-row because
// the suppressed cone {NOFORN} is exactly the fact that LES-NF / SBU-NF's
// PageRewrite would have added. The monotonicity proof holds via the
// downstream PageRewrite step rather than via FDR_DOMINATORS membership;
// the Trio-1 row is permitted to over-fire on bare-LES-NF / bare-SBU-NF
// because the PageRewrite supplies the suppressor fact downstream.
// `pub(crate)` so the `Vocabulary::is_fdr_dissem` override in
// `crates/capco/src/vocabulary.rs` and the bidirectional value-pin test
// (`mod fdr_dissem_pin` in the same file) can read this slice as the
// single source-of-truth.
//
// **Maintenance contract.** This slice and the neighboring
// `is_fdr_dominator` function answer *different* questions about
// the FD&R family, and the two enumerations are independent on
// purpose:
//   - `FDR_DOMINATORS` (this slice) enumerates **FD&R-set
//     membership** per §B.3.a p19 — the four canonical FD&R
//     markings (NOFORN / REL TO / RELIDO / DISPLAY ONLY) plus the
//     §H.8 p157 EYES legacy. `Vocabulary::is_fdr_dissem` walks
//     this slice and is the authoritative FD&R-membership API.
//   - `is_fdr_dominator` (below) enumerates **FD&R dominators
//     *over* RELIDO** for the `Constraint::ConflictsWithFamily`
//     dispatch on the RELIDO conflict catalog (E054/E055). It
//     deliberately **excludes RELIDO itself** because RELIDO-vs-
//     RELIDO is a tautology in the conflict family — there is no
//     such conflict to detect.
// The intersection of the two sets is "FD&R members that conflict
// with RELIDO" (NOFORN, DISPLAY ONLY, REL TO, EYES). The slice is
// the strict superset. Do not collapse them: a future refactor
// that delegates `is_fdr_dissem` through `is_fdr_dominator` will
// silently under-fire on RELIDO and is pinned against in
// `vocabulary.rs::fdr_dissem_pin::relido_admits_despite_is_fdr_dominator_excluding_it`.
//
// Adding a `Token` entry to this slice requires:
//   1. Considering whether the new token should also dominate
//      RELIDO. If yes, add a parallel arm to `is_fdr_dominator`'s
//      `matches!`. If no, leave `is_fdr_dominator` alone.
//   2. The `Vocabulary::is_fdr_dissem` override picks up the new
//      entry automatically — it iterates this slice directly.
// Adding an `AnyInCategory(CAT_X)` entry requires updating the
// override's per-category routing in `vocabulary.rs` because the
// override receives a single `TokenId` and dispatches through
// `capco_token_category` rather than passing a `TokenRef`.
pub(crate) static FDR_DOMINATORS: &[TokenRef] = &[
    TokenRef::Token(TOK_NOFORN),
    TokenRef::Token(TOK_RELIDO),
    TokenRef::Token(TOK_DISPLAY_ONLY),
    TokenRef::AnyInCategory(CAT_REL_TO),
    // EYES (USA/[LIST] EYES ONLY) is an FD&R marking per §H.8 p157.
    // The sentinel (`TOK_EYES`), the `satisfies_attrs` arm, and the
    // `iter_present_tokens` mapping all land in PR 3.7 rev 3 so that
    // EYES-only portions correctly suppress the implicit-NOFORN
    // trio rows. Per Copilot PR 3.7 review pass 3: an earlier rev
    // claimed EYES was covered via `CAT_REL_TO` fallthrough, which
    // was false — `CAT_REL_TO` only checks `attrs.rel_to`. EYES is
    // a `DissemControl::Eyes` variant produced by the parser
    // (deprecated 2017-10-01 per §H.8 p157 but still recognized for
    // legacy-input compatibility).
    TokenRef::Token(TOK_EYES),
];

// `FDR_OR_RELIDO_INCOMPAT` (the Trio 2 / Trio 3 extended suppressor
// covering FD&R dominators + RELIDO-incompatible tokens like FGI / JOINT
// / NATO / ORCON / LES-NF / SBU-NF) was removed from the active catalog
// in PR 3.7 rev 4. It was consumed by `CLOSURE_RELIDO_US_CLASS` and
// `CLOSURE_RELIDO_RSEN_FOUO` (the Trio 2 placeholder rows), both of
// which retired alongside the SCI per-marking placeholder rows because
// their over-broad triggers (`AnyInCategory(CAT_CLASSIFICATION)` and
// `Token(TOK_RSEN)`/`Token(TOK_FOUO)`) would over-fire on SCI-bearing
// markings before the SCI rows could add their suppressors.
//
// PR 4 (T112) re-introduces the suppressor data when the Trio 2 rows
// land with proper triggers + the closure() engine wiring + runtime-
// resolved severity (per D19 B). For now the suppressor knowledge
// lives only in the inline comments on E054/E055/E056/E057 rows; the
// algebraic shape is documented in `marque-applied.md` §4.7.1.

// --- The implicit-default trio (FD&R-suppressed) ---

// Trio 1 triggers: all markings that imply NOFORN when no explicit FD&R
// decision is present. Per `marque-applied.md` §4.7.1 implicit_NOFORN list.
// One row per trigger group (grouped by source §-citation for traceability).

/// Trio 1, row 1: SAR programs imply NOFORN unless FD&R-marked.
///
/// SAR program identifiers live on `CAT_SAR`. Any SAR marking is a
/// US-originator-controlled marking for which NOFORN is the implicit
/// release posture. CAPCO-2016 §H.5 (pp99-102) governs SAR markings;
/// the NOFORN implication flows from §B.3 Table 2 p21.
const CLOSURE_NOFORN_SAR: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/noforn-if-sar",
    label: "CAPCO-2016 §B.3 Table 2 p21",
    triggers: &[TokenRef::AnyInCategory(CAT_SAR)],
    suppressors: FDR_DOMINATORS,
    cone: &[TokenRef::Token(TOK_NOFORN)],
    cone_derived: None,
    default_severity: Severity::Info,
};

/// Trio 1, row 2: RD / FRD / TFNI imply NOFORN unless FD&R-marked.
///
/// Atomic Energy Act markings (Restricted Data, Formerly Restricted Data,
/// Transclassified Foreign Nuclear Information) carry NOFORN by definition
/// for the IC marking context. Per CAPCO-2016 §H.6 (pp104-121) and
/// §B.3 Table 2 p21.
const CLOSURE_NOFORN_AEA_RD: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/noforn-if-aea",
    label: "CAPCO-2016 §B.3 Table 2 p21",
    triggers: &[
        TokenRef::Token(TOK_RD),
        TokenRef::Token(TOK_FRD),
        TokenRef::Token(TOK_TFNI),
    ],
    suppressors: FDR_DOMINATORS,
    cone: &[TokenRef::Token(TOK_NOFORN)],
    cone_derived: None,
    default_severity: Severity::Info,
};

/// Trio 1, row 3: DOD/DOE UCNI implies NOFORN unless FD&R-marked.
///
/// Unclassified Controlled Nuclear Information markings carry a NOFORN
/// treatment in the IC context per §B.3 Table 2 p21. The UCNI marking
/// itself is constrained to UNCLASSIFIED per §H.6 DCNI pp116-117 (DoD)
/// and §H.6 UCNI pp118-119 (DoE); the NOFORN closure fires regardless
/// of class.
const CLOSURE_NOFORN_UCNI: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/noforn-if-ucni",
    label: "CAPCO-2016 §B.3 Table 2 p21",
    triggers: &[TokenRef::Token(TOK_UCNI)],
    suppressors: FDR_DOMINATORS,
    cone: &[TokenRef::Token(TOK_NOFORN)],
    cone_derived: None,
    default_severity: Severity::Info,
};

/// Trio 1, row 4: Any FGI atom implies NOFORN unless FD&R-marked.
///
/// Foreign Government Information markings carry an implicit NOFORN posture
/// because the equity belongs to a foreign government and its release requires
/// FD&R authority. Per CAPCO-2016 §H.7 (pp122-130) and §B.3 Table 2 p21.
const CLOSURE_NOFORN_FGI: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/noforn-if-fgi",
    label: "CAPCO-2016 §H.7 p122",
    // BOTH triggers are required to cover the two FGI sources per
    // Copilot PR 3.7 review #12:
    //   - `TokenRef::Token(TOK_FGI_MARKER)` is satisfied by
    //     `MarkingClassification::Fgi` (foreign-classified portions
    //     like `//GBR SECRET`) because `satisfies_attrs`'s
    //     classification arm emits `TOK_FGI_MARKER` for that case.
    //   - `TokenRef::AnyInCategory(CAT_FGI_MARKER)` is satisfied by
    //     `attrs.fgi_marker` (explicit `FGI` token).
    // An earlier cleanup dropped the explicit token thinking
    // `AnyInCategory` was a superset; it is NOT — they cover
    // disjoint FGI surfaces. Both must be present so a foreign-
    // classified portion like `//GBR SECRET` reaches the
    // implicit-NOFORN closure.
    triggers: &[
        TokenRef::Token(TOK_FGI_MARKER),
        TokenRef::AnyInCategory(CAT_FGI_MARKER),
    ],
    suppressors: FDR_DOMINATORS,
    cone: &[TokenRef::Token(TOK_NOFORN)],
    cone_derived: None,
    default_severity: Severity::Info,
};

/// Trio 1, row 5: ORCON / ORCON-USGOV implies NOFORN unless FD&R-marked.
///
/// ORCON and ORCON-USGOV require originator approval before further
/// dissemination; their implicit release posture is NOFORN when no explicit
/// FD&R decision is present. Per CAPCO-2016 §H.8 p136 (ORCON) and
/// §H.8 p139 (ORCON-USGOV), cross-referenced with §B.3 Table 2 p21.
const CLOSURE_NOFORN_ORCON: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/noforn-if-orcon",
    label: "CAPCO-2016 §B.3 Table 2 p21",
    triggers: &[TokenRef::Token(TOK_ORCON), TokenRef::Token(TOK_ORCON_USGOV)],
    suppressors: FDR_DOMINATORS,
    cone: &[TokenRef::Token(TOK_NOFORN)],
    cone_derived: None,
    default_severity: Severity::Info,
};

/// Trio 1, row 6: IMCON / DEA SENSITIVE imply NOFORN unless FD&R-marked.
///
/// Controlled Imagery (IMCON) and DEA Sensitive (DSEN) are originator-
/// controlled markings whose implicit release posture is NOFORN. Per
/// CAPCO-2016 §H.8 p142 (IMCON) and §H.8 p159 (DEA SENSITIVE), cross-
/// referenced with §B.3 Table 2 p21.
const CLOSURE_NOFORN_IMCON_DSEN: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/noforn-if-imcon-dsen",
    label: "CAPCO-2016 §B.3 Table 2 p21",
    triggers: &[TokenRef::Token(TOK_IMCON), TokenRef::Token(TOK_DSEN)],
    suppressors: FDR_DOMINATORS,
    cone: &[TokenRef::Token(TOK_NOFORN)],
    cone_derived: None,
    default_severity: Severity::Info,
};

/// Trio 1, row 7: Non-IC controls LIMDIS / LES / SBU / SSI imply NOFORN
/// unless FD&R-marked.
///
/// These non-IC dissemination controls have a NOFORN-equivalent treatment in
/// the IC marking context when no explicit FD&R decision is present. Per
/// CAPCO-2016 §H.9 p170 (LIMDIS), §H.9 p181 (LES), §H.9 p176 (SBU),
/// §H.9 p189 (SSI), cross-referenced with §B.3 Table 2 p21.
const CLOSURE_NOFORN_NONICCONTROLS: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/noforn-if-non-ic-controls",
    label: "CAPCO-2016 §B.3 Table 2 p21",
    triggers: &[
        TokenRef::Token(TOK_LIMDIS),
        TokenRef::Token(TOK_LES),
        TokenRef::Token(TOK_SBU),
        TokenRef::Token(TOK_SSI),
    ],
    suppressors: FDR_DOMINATORS,
    cone: &[TokenRef::Token(TOK_NOFORN)],
    cone_derived: None,
    default_severity: Severity::Info,
};

/// The full static CAPCO closure-rule catalog.
///
/// Rows are grouped by the three-trio framing from `marque-applied.md` §4.7.1:
///   1. Trio 1 — implicit NOFORN (FD&R-suppressed)
///   2. Trio 2 — implicit RELIDO (FD&R + RELIDO-incompatible-suppressed)
///   3. Trio 3 — implicit REL TO USA, NATO (FD&R-suppressed)
///   4. Per-marking unconditional implications (unsuppressed)
///
/// Per-row monotonicity attestation (§4.7.3 table-design property, case 2):
/// Every suppressor fact either contains the cone's intent or makes it
/// redundant. For Trio 1/3 (FDR_DOMINATORS): the suppressor is always a
/// manifest FD&R decision that supersedes the implicit default. For Trio 2
/// (FDR_OR_RELIDO_INCOMPAT): same, plus RELIDO-incompatible tokens make the
/// RELIDO cone inapplicable by definition. Unconditional rows have no
/// suppressor — monotonicity is trivial (empty suppressor → no case 2).
///
/// # Coalesced triggers (PR 3.7 limitation)
///
/// Several per-marking unconditional implications (HCS-O/P[sub], SI-G,
/// TK-BLFH/KAND/IDIT) currently use `AnyInCategory(CAT_SCI)` as a proxy
/// trigger because per-compartment sentinels (`TOK_HCS_O`, `TOK_SI_G`, etc.)
/// do not yet exist. This makes the catalog CONSERVATIVE (fires NOFORN/ORCON
/// on any SCI marking, not just the specific compartments) rather than
/// PRECISE. The engine call-site at PR 4 will add precise triggers
/// alongside the per-compartment sentinels (T112 follow-up).
pub(super) static CAPCO_CLOSURE_RULES: &[ClosureRule<CapcoScheme>] = &[
    // Trio 1: implicit NOFORN rows — these have correct token-level
    // triggers and ship as functional catalog data. The Trio 1 rows
    // are the load-bearing closure-operator entries the engine wires
    // through `Engine::project` at PR 4.
    CLOSURE_NOFORN_SAR,
    CLOSURE_NOFORN_AEA_RD,
    CLOSURE_NOFORN_UCNI,
    CLOSURE_NOFORN_FGI,
    CLOSURE_NOFORN_ORCON,
    CLOSURE_NOFORN_IMCON_DSEN,
    CLOSURE_NOFORN_NONICCONTROLS,
    // Trio 2 (implicit RELIDO), Trio 3 (implicit REL TO USA, NATO),
    // and the per-marking unconditional SCI implications (HCS-O,
    // HCS-P[sub], SI-G, TK-BLFH, TK-KAND, TK-IDIT) were REMOVED
    // from the active catalog in PR 3.7 rev 4 per Copilot review
    // pass 4. Three reasons:
    //   1. Their triggers proxy via broad `AnyInCategory(CAT_SCI)` or
    //      `AnyInCategory(CAT_CLASSIFICATION)` because per-compartment
    //      sentinels (TOK_HCS_O, TOK_SI_G, etc.) don't exist yet —
    //      they over-fire on bare `SI` / bare `TK` / any classified
    //      marking respectively.
    //   2. The Trio 3 cone was an `AnyInCategory(CAT_REL_TO)`
    //      placeholder, structurally incapable of adding the specific
    //      `REL TO USA, NATO` fact.
    //   3. The previous "Severity::Off as catalog-data dormancy gate"
    //      mitigation contradicted D19 B (severity is runtime-resolved,
    //      not catalog-baked), so any user enabling these rows via
    //      `[closure_rules]` config would trigger the over-firing.
    // PR 4 (T112) lands these rows with proper sentinels, real
    // cone-addition machinery (open-vocab FactAdd for the Trio 3
    // country-list case), and the engine wiring to consult runtime
    // severity per-row.
];
