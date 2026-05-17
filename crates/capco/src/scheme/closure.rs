// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CAPCO closure-rule catalog — `FDR_DOMINATORS` + 7× `CLOSURE_NOFORN_*` +
//! `CLOSURE_REL_TO_USA_NATO` + the aggregating `CAPCO_CLOSURE_RULES` static.
//!
//! Implements the §4.7 implicit-fact propagation catalog from
//! `docs/plans/2026-05-01-lattice-design.md` §3 (e) and
//! `marque-applied.md` §4.7. The `MarkingScheme::closure_rules()` impl on
//! `CapcoScheme` exposes it as the public catalog surface per
//! `decisions.md` D18.

use marque_scheme::{ClosureRule, FactRef, Severity, TokenRef};
use smallvec::{SmallVec, smallvec};

use super::*;

// ---------------------------------------------------------------------------
// Closure-rule catalog + family predicates
// ---------------------------------------------------------------------------

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
    // It is parsed as `DissemControl::Eyes` (deprecated 2017-10-01 per
    // §H.8 p157 but still recognized for legacy-input compatibility), and
    // requires its own `TOK_EYES` sentinel + `satisfies_attrs` /
    // `iter_present_tokens` wiring — `CAT_REL_TO` fallthrough does NOT
    // cover it because `CAT_REL_TO` only checks `attrs.rel_to`. Including
    // EYES here ensures EYES-only portions correctly suppress the
    // implicit-NOFORN trio rows.
    TokenRef::Token(TOK_EYES),
];

// `FDR_OR_RELIDO_INCOMPAT` (the Trio 2 / Trio 3 extended suppressor
// covering FD&R dominators + RELIDO-incompatible tokens like FGI / JOINT
// / NATO / ORCON / LES-NF / SBU-NF) is intentionally absent from the
// active catalog. It was previously consumed by two Trio 2 placeholder
// rows (`CLOSURE_RELIDO_US_CLASS`, `CLOSURE_RELIDO_RSEN_FOUO`) whose
// over-broad triggers (`AnyInCategory(CAT_CLASSIFICATION)` and
// `Token(TOK_RSEN)`/`Token(TOK_FOUO)`) over-fired on SCI-bearing
// markings before the SCI rows could add their suppressors.
//
// The Trio 2 rows will return once per-compartment sentinels exist and
// the engine consults runtime severity per-row (per `decisions.md` D19 B).
// Until then, the suppressor knowledge lives only in the inline comments
// on E054/E055/E056/E057 rows; the algebraic shape is documented in
// `marque-applied.md` §4.7.1.

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
///
/// Both UCNI sentinels are required in the trigger list: per issue
/// #407, `TOK_UCNI` resolves to `AeaMarking::DoeUcni` only and the
/// DOD variant resolves through the distinct `TOK_DCNI` sentinel
/// (see `predicates::satisfies::aea_marking_to_token` at
/// `AeaMarking::DodUcni => Some(TOK_DCNI)`). The §B.3 Table 2 p21
/// caveated→NOFORN algebra is grammar-agnostic over which sentinel
/// surfaces the UCNI marking, so both rows compose through the same
/// closure label.
const CLOSURE_NOFORN_UCNI: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/noforn-if-ucni",
    label: "CAPCO-2016 §B.3 Table 2 p21",
    triggers: &[TokenRef::Token(TOK_UCNI), TokenRef::Token(TOK_DCNI)],
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
    // BOTH triggers are required to cover the two FGI sources:
    //   - `TokenRef::Token(TOK_FGI_MARKER)` is satisfied by
    //     `MarkingClassification::Fgi` (foreign-classified portions
    //     like `//GBR SECRET`) because `satisfies_attrs`'s
    //     classification arm emits `TOK_FGI_MARKER` for that case.
    //   - `TokenRef::AnyInCategory(CAT_FGI_MARKER)` is satisfied by
    //     `attrs.fgi_marker` (explicit `FGI` token).
    // `AnyInCategory` is NOT a superset of the token form — they cover
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

/// Trio 1, row 6: RSEN / IMCON / DEA SENSITIVE imply NOFORN unless FD&R-marked.
///
/// Risk Sensitive (RSEN), Controlled Imagery (IMCON), and DEA Sensitive (DSEN)
/// are caveat markings per §B.3 p20 Note (the structural caveated/uncaveated
/// definition: "Caveated means bears no FD&R markings, but has one or more
/// AEA markings, SAP markings, and/or dissemination control marking(s)").
/// Their implicit release posture is NOFORN when no explicit FD&R decision
/// is present.
///
/// All three rows ride on the §B.3 Table 2 p21 row "Classified, caveated,
/// on/after 28 Jun 2010 → NOFORN" — the marking-template pages (§H.8 p132
/// for RSEN, §H.8 p142 for IMCON, §H.8 p159 for DSEN) are the per-marking
/// definitions; §B.3 Table 2 p21 is the cross-cutting NOFORN-implication.
/// RSEN closes a coverage gap noted in the lattice-design follow-up: it is
/// a caveat by the same §B.3 p20 Note definition that justifies IMCON/DSEN
/// inclusion.
const CLOSURE_NOFORN_RSEN_IMCON_DSEN: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/noforn-if-rsen-imcon-dsen",
    label: "CAPCO-2016 §B.3 Table 2 p21",
    triggers: &[
        TokenRef::Token(TOK_RSEN),
        TokenRef::Token(TOK_IMCON),
        TokenRef::Token(TOK_DSEN),
    ],
    suppressors: FDR_DOMINATORS,
    cone: &[TokenRef::Token(TOK_NOFORN)],
    cone_derived: None,
    default_severity: Severity::Info,
};

/// `cone_derived` helper for `CLOSURE_REL_TO_USA_NATO` — emits the
/// open-vocab `CountryCode::NATO` tetragraph fact.
///
/// `CountryCode::USA` is carried via the static `cone` field through
/// `TOK_USA`, which `apply_fact_add`'s `CAT_REL_TO` arm special-cases to
/// `CountryCode::USA`. NATO has no equivalent closed-vocab sentinel — it
/// routes through the open-vocab
/// `FactRef::OpenVocab(CapcoOpenVocabRef::CountryCode(_))` path
/// established for JOINT co-owner coverage (E014).
///
/// Constant-output (parameter unused): the cone facts are static — USA
/// and NATO regardless of marking shape. Closure-rule monotonicity is
/// vacuous on a constant-output function; the rule-level monotonicity
/// attestation (FDR_DOMINATORS suppressors are stable dominators that
/// no rule's cone adds) is the same one the seven `CLOSURE_NOFORN_*`
/// rows rely on.
fn rel_to_usa_nato_derived_cone(_m: &CapcoMarking) -> SmallVec<[FactRef<CapcoScheme>; 2]> {
    smallvec![FactRef::OpenVocab(CapcoOpenVocabRef::CountryCode(
        marque_ism::CountryCode::NATO
    ))]
}

/// Bare NATO classification ⇒ implicit `REL TO USA, NATO`
/// unless FD&R-marked.
///
/// **Authority is example-derived.** The CAPCO-2016 manual moves the
/// authoritative NATO grammar to Appendix B (§H.2 p55 explicitly
/// redirects: "Manual Appendix B   –   NATO Protective Markings"),
/// which is not vendored in `crates/capco/docs/CAPCO-2016.md`. The
/// in-manual surfaces we can cite are:
///
/// - **§G.1 Table 4 p38** — registers the NATO classification markings
///   (`COSMIC TOP SECRET`/`CTS`, `NATO SECRET`/`NS`, `NATO CONFIDENTIAL`/`NC`,
///   `NATO RESTRICTED`/`NR`, `NATO UNCLASSIFIED`/`NU`) with the explicit
///   pointer "NATO Protective Markings, refer to Appendix B".
/// - **§G.2 Table 5 p40** — alliance-reciprocity ARH grounding: every
///   NATO classification level row reads "Requires NATO read-in" (the
///   treaty default for NATO-marked information in USG hands).
/// - **§H.7 p127 Notional Example Page 2** — the worked example
///   `(//CTS//BOHEMIA//REL TO USA, NATO)` demonstrating the *form*
///   that a NATO portion in a US document carries REL TO USA, NATO.
///
/// §H.7 p127 is a notional example, not MUST-prose: it shows the
/// structural pattern for a `CTS + BOHEMIA SAP` portion with an
/// explicit `REL TO USA, NATO`, and the prose attached to the example
/// describes that specific portion ("releasable back to NATO"). The
/// implication "bare NATO ⇒ REL TO USA, NATO" is *derived* from the
/// example + §G.2 Table 5 alliance-reciprocity reading, not stated
/// prescriptively in the manual's vendored text. The closure row's
/// `Severity::Info` calibration is deliberate precisely because the
/// authority is example-derived (D20): the byte-level surface remains
/// the responsibility of the `Severity::Suggest` text-layer rule
/// (S007) which a human reviewer can override.
///
/// **D20 layer separation (decisions.md 916-973)**: this row fires at
/// `Severity::Info` (silent fact propagation at the lattice layer); the
/// text-layer surface (`Severity::Suggest` byte diff
/// `(//NS)` → `(//NS//REL TO USA, NATO)`) is the S007 rule. The two
/// layers are complementary — no double-audit on the same inference.
///
/// **Suppressors (D20)**: `FDR_DOMINATORS`. When the page already carries
/// an explicit FD&R decision (NOFORN, REL TO, RELIDO, DISPLAY ONLY,
/// EYES), the closure does not fire — the explicit decision supersedes
/// the implicit one. NOFORN-vs-REL TO conflict is the §H.8 p145
/// supersession overlay's responsibility (it owns the conflict path);
/// FD&R suppression here merely prevents the closure from racing.
///
/// **Cone shape**: USA via the static `cone` (`TOK_USA`, which
/// `apply_fact_add` routes to `CountryCode::USA` on CAT_REL_TO); NATO
/// via `cone_derived` returning `FactRef::OpenVocab(CountryCode::NATO)`
/// because `CountryCode::NATO` has no closed-vocab `TokenId`. Both facts
/// route to CAT_REL_TO via `CapcoScheme::category_of`.
const CLOSURE_REL_TO_USA_NATO: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/rel-to-usa-nato-if-nato-classification",
    label: "CAPCO-2016 §H.7 p127 (example-derived) + §G.2 Table 5 p40",
    triggers: &[TokenRef::Token(TOK_NATO_CLASS)],
    suppressors: FDR_DOMINATORS,
    cone: &[TokenRef::Token(TOK_USA)],
    cone_derived: Some(rel_to_usa_nato_derived_cone),
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
/// # Coalesced triggers (current limitation)
///
/// Several per-marking unconditional implications (HCS-O/P[sub], SI-G,
/// TK-BLFH/KAND/IDIT) would naturally use `AnyInCategory(CAT_SCI)` as a
/// proxy trigger because per-compartment sentinels (`TOK_HCS_O`, `TOK_SI_G`,
/// etc.) do not exist yet. They are intentionally omitted until those
/// sentinels land — the broad proxy would fire NOFORN/ORCON on any SCI
/// marking, not just the specific compartments, which is unsound.
pub(super) static CAPCO_CLOSURE_RULES: &[ClosureRule<CapcoScheme>] = &[
    // Trio 1: implicit NOFORN rows — token-level triggers, no proxies,
    // load-bearing for the closure-operator hot path.
    CLOSURE_NOFORN_SAR,
    CLOSURE_NOFORN_AEA_RD,
    CLOSURE_NOFORN_UCNI,
    CLOSURE_NOFORN_FGI,
    CLOSURE_NOFORN_ORCON,
    CLOSURE_NOFORN_RSEN_IMCON_DSEN,
    CLOSURE_NOFORN_NONICCONTROLS,
    // Trio 3: implicit `REL TO USA, NATO` for bare NATO classification.
    // Fires at `Severity::Info` (silent lattice-layer fact propagation);
    // S007 owns the text-layer `Severity::Suggest` byte-diff per D20.
    // NATO routes via `cone_derived` (open-vocab `CountryCode::NATO`),
    // USA via the static cone (`TOK_USA` → `CountryCode::USA` through
    // `apply_fact_add`'s CAT_REL_TO arm).
    CLOSURE_REL_TO_USA_NATO,
    // Trio 2 (implicit RELIDO) and the per-marking unconditional SCI
    // implications (HCS-O, HCS-P[sub], SI-G, TK-BLFH, TK-KAND, TK-IDIT)
    // are intentionally absent. They require per-compartment sentinels
    // (TOK_HCS_O, TOK_SI_G, etc.) that do not yet exist; the alternative
    // — proxy triggers via `AnyInCategory(CAT_SCI)` /
    // `AnyInCategory(CAT_CLASSIFICATION)` — would over-fire on bare SI /
    // bare TK / any classified marking. A `Severity::Off` catalog-data
    // dormancy gate would contradict D19 B (severity is runtime-resolved,
    // not catalog-baked). The rows will return once the per-marking
    // sentinels land and the engine consults runtime severity per-row.
];
