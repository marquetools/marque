// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CAPCO closure-rule catalog (residual) — `FDR_DOMINATORS` +
//! `CLOSURE_REL_TO_USA_NATO` + the aggregating `CAPCO_CLOSURE_RULES`
//! static.
//!
//! # Post-PR-D shape (issue #371)
//!
//! PR-D of the FactBitmask refactor wired `CapcoScheme::closure` to the
//! bitmask Kleene fast path (`CLOSURE_TABLE` + `close()` in
//! `closure_table.rs`). Nine of the ten original `ClosureRule` fn-pointer
//! statics — `CLOSURE_NOFORN_CAVEATED`, the six per-marking SCI
//! implications (HCS-O / HCS-P[sub] / SI-G / TK-BLFH / TK-IDIT / TK-KAND),
//! and the two Trio 2 RELIDO rows — were retired in PR-D because their
//! triggers / suppressors / cones all live in the closed-vocab atom
//! inventory and compile cleanly to bitmask form.
//!
//! Only `CLOSURE_REL_TO_USA_NATO` survives in fn-pointer form. The row's
//! `cone_derived` injects `CountryCode::NATO` into `rel_to` via an
//! open-vocab `FactRef::OpenVocab(_)` — there is no closed-vocab `TokenId`
//! for NATO as a tetragraph, so it cannot be projected onto a bit and
//! cannot ride the bitmask cone path. The closure body in
//! `marking_scheme_impl.rs::CapcoScheme::closure` applies the static USA
//! leg of Row 7 through the bitmask `REL_TO_USA` atom and runs the
//! surviving `cone_derived` once after the Kleene fixpoint converges.
//!
//! The `FDR_DOMINATORS` slice stays in this file as the source-of-truth
//! enumeration of the FD&R-membership set: it's still consumed by the
//! `MASK_FDR_DOMINATORS` projection in `fact_bitmask.rs` (as the
//! corpus-side definition the bitmask suppressor is derived from), by
//! the `Vocabulary::is_fdr_dissem` override in `vocabulary.rs`, and by
//! `CLOSURE_REL_TO_USA_NATO` itself as Row 7's suppressor.
//!
//! `FDR_OR_RELIDO_INCOMPAT` and `RELIDO_US_CLASS_SUPPRESSORS` remain in
//! this file because the in-file `#[cfg(test)]` modules (the
//! `phase2_closure_pin` SCI-suppression iterator, the `phase3_closure_pin`
//! US-classification suppression iterator) iterate them as fixture
//! enumerations. The production closure path no longer reads them. A
//! follow-on PR (when the test fixtures move to a bitmask-driven
//! iteration over `MASK_*` constants) will retire them.
//!
//! # Historical note
//!
//! Trio 1 was originally split into seven token-grouped rows for
//! §-citation locality and consolidated by PR #522 (D18 rationale 2)
//! into the single `CLOSURE_NOFORN_CAVEATED` row that PR-D then
//! retired into the `CLOSURE_TABLE` Row 0.

use marque_scheme::{ClosureRule, FactRef, SectionLetter, Severity, TokenRef, capco};
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

// `FDR_OR_RELIDO_INCOMPAT` — the Trio 2 / Trio 3 extended suppressor.
//
// Covers FD&R dominators (everything in `FDR_DOMINATORS`) plus the
// RELIDO-incompatible tokens enumerated in `marque-applied.md`
// §4.7.1 `has_relido_incompatible`: foreign-equity / origination
// markings (any FGI atom, any JOINT atom, any NATO atom) plus the
// six per-compartment SCI sentinels (SI-G, HCS-O, HCS-P[sub],
// TK-BLFH, TK-IDIT, TK-KAND) whose per-marking unconditional
// implications make RELIDO inapplicable by definition (per
// `marque-applied.md` §4.7.5 Trio 2 exclusion list: "Excludes SCI
// controls that already carry NOFORN implication: SI-G, HCS-O,
// HCS-P[sub], TK-BLFH, TK-KAND, TK-IDIT — those go through the
// implicit-NOFORN path").
//
// LES-NF and SBU-NF are not enumerated separately because their
// presence is represented as `dissem_us: [Les | Sbu, Noforn]` —
// `TOK_NOFORN` (already in `FDR_DOMINATORS`) covers them via the
// `iter_present_tokens` emission of `TokenRef::Token(TOK_NOFORN)`
// for the `Noforn` element.
//
// Algebraic note: per `marque-applied.md` §4.7.3 case 2
// (table-design property), every suppressor either contains the
// suppressed cone's intent (NOFORN ⊐ RELIDO via §H.8 p145
// supersession chain) or makes the cone inapplicable
// (RELIDO-incompatible tokens prevent the RELIDO cone from being
// meaningful by definition). The six SCI compartment sentinels are
// admitted under the second clause: their per-marking
// unconditional implications (NOFORN / ORCON per §H.4 templates)
// make RELIDO inapplicable per CAPCO-2016 §H.4 marking-template
// authority.
//
// Per-token authority table:
//
// | Token                       | Authority                  |
// |-----------------------------|----------------------------|
// | (all `FDR_DOMINATORS`)      | §B.3.a p19, §H.8 p157 EYES |
// | `TOK_FGI_MARKER`            | §H.7 p123                  |
// | `AnyInCategory(CAT_FGI_MARKER)` | §H.7 p123              |
// | `TOK_FGI_CLASS`             | §H.7 p123                  |
// | `TOK_JOINT`                 | §H.3 p56                   |
// | `TOK_NATO_CLASS`            | §G.1 Table 4 p38 / §H.7 p127 |
// | `TOK_SI_G`                  | §H.4 p80                   |
// | `TOK_HCS_O`                 | §H.4 p64                   |
// | `TOK_HCS_P_SUB`             | §H.4 p68                   |
// | `TOK_TK_BLFH`               | §H.4 p87                   |
// | `TOK_TK_IDIT`               | §H.4 p91                   |
// | `TOK_TK_KAND`               | §H.4 p95                   |
//
// `pub(crate)` so the in-file `phase2_closure_pin` test module — the
// sole consumer post-PR-D — can iterate the slice as the TokenRef
// source-of-truth that backstops the bitmask `MASK_FDR_OR_RELIDO_INCOMPAT`
// projection.  `#[cfg(test)]` (rather than `#[allow(dead_code)]`)
// enforces the test-only contract: any future production caller is a
// compile error in a `cfg(not(test))` build, not a silently-pinged
// dead-code warning.
#[cfg(test)]
pub(crate) static FDR_OR_RELIDO_INCOMPAT: &[TokenRef] = &[
    // FD&R dominators (NOFORN ⊐ RELIDO per §H.8 p145; REL TO / RELIDO
    // / DISPLAY ONLY / EYES are explicit FD&R decisions). Listed
    // inline rather than spread-imported from `FDR_DOMINATORS` so the
    // slice is a compile-time constant readable in one place.
    TokenRef::Token(TOK_NOFORN),
    TokenRef::Token(TOK_RELIDO),
    TokenRef::Token(TOK_DISPLAY_ONLY),
    TokenRef::AnyInCategory(CAT_REL_TO),
    TokenRef::Token(TOK_EYES),
    // Foreign-equity / origination — §H.7 p123 (FGI), §H.3 p56
    // (JOINT), §G.1 Table 4 p38 + §H.7 p127 (NATO).
    TokenRef::Token(TOK_FGI_MARKER),
    TokenRef::AnyInCategory(CAT_FGI_MARKER),
    TokenRef::Token(TOK_FGI_CLASS),
    TokenRef::Token(TOK_JOINT),
    TokenRef::Token(TOK_NATO_CLASS),
    // Per-compartment SCI sentinels carrying NOFORN/ORCON per-marking
    // unconditional implications (§H.4 marking templates). Including
    // them in this slice makes the Trio 2 `CLOSURE_RELIDO_SCI` row's
    // suppression of bare-SI-G correct without depending on Kleene-
    // fixpoint ordering — see the `CLOSURE_RELIDO_SCI` row's
    // doc-comment for the SI-G-specific rationale (SI-G's per-marking
    // cone is `{ORCON}` only, so NOFORN-via-Trio-1-fixpoint does not
    // cover the SI-G suppression case).
    TokenRef::Token(TOK_SI_G),
    TokenRef::Token(TOK_HCS_O),
    TokenRef::Token(TOK_HCS_P_SUB),
    TokenRef::Token(TOK_TK_BLFH),
    TokenRef::Token(TOK_TK_IDIT),
    TokenRef::Token(TOK_TK_KAND),
];

// Trio 1 (`CLOSURE_NOFORN_CAVEATED`) was retired in PR-D of the
// FactBitmask refactor (issue #371). The 20-trigger caveated-NOFORN
// row was bit-packed into `CLOSURE_TABLE` Row 0 with the
// `ROW0_NOFORN_IF_CAVEATED_TRIGGERS` mask (21 source `TokenRef`
// entries collapse to 20 atom bits — the `TOK_FGI_MARKER` +
// `AnyInCategory(CAT_FGI_MARKER)` redundant pair both project to
// `fact_bit::FGI_PRESENT`). The §-citation chain — universal §B.3
// p20 Note + §B.3 Table 2 p21 algebraic anchor + per-trigger Section
// H authorities — is preserved verbatim on the `CLOSURE_TABLE` Row 0
// `label` field and on the `ROW0_NOFORN_IF_CAVEATED_TRIGGERS`
// doc-comment in `closure_table.rs`.

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
pub(super) const CLOSURE_REL_TO_USA_NATO: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/rel-to-usa-nato-if-nato-classification",
    display_label: "Bare NATO classification implies REL TO USA, NATO",
    label: capco(SectionLetter::H, 7, 127),
    triggers: &[TokenRef::Token(TOK_NATO_CLASS)],
    suppressors: FDR_DOMINATORS,
    cone: &[TokenRef::Token(TOK_USA)],
    cone_derived: Some(rel_to_usa_nato_derived_cone),
    default_severity: Severity::Info,
};

// The six per-marking unconditional SCI implication rules
// (CLOSURE_HCS_O_IMPLIES_NF_OC, CLOSURE_HCS_P_SUB_IMPLIES_NF_OC,
// CLOSURE_SI_G_IMPLIES_OC, CLOSURE_TK_BLFH_IMPLIES_NF,
// CLOSURE_TK_IDIT_IMPLIES_NF, CLOSURE_TK_KAND_IMPLIES_NF) were
// retired in PR-D of the FactBitmask refactor (issue #371). The
// trigger sentinels (TOK_HCS_O / TOK_HCS_P_SUB / TOK_SI_G / TOK_TK_*)
// and their NOFORN / ORCON cones all live in the closed-vocab atom
// inventory, so each retired rule became a single positional row in
// `CLOSURE_TABLE` (Rows 1–6 in `closure_table.rs`). The §H.4
// per-marking authority chain — §H.4 p64 (HCS-O), §H.4 p68
// (HCS-P[sub]), §H.4 p80 (SI-G), §H.4 p87 / p91 / p95 (TK-BLFH /
// TK-IDIT / TK-KAND) — is preserved on the per-row `label` fields
// in `closure_table.rs`.

// ---------------------------------------------------------------------------
// Trio 2 — implicit RELIDO suppressor slices (retained for test fixtures)
// ---------------------------------------------------------------------------
//
// The two Trio 2 closure rules (`CLOSURE_RELIDO_SCI` +
// `CLOSURE_RELIDO_US_CLASS`) were retired in PR-D of the FactBitmask
// refactor (issue #371). Both rules' triggers (`AnyInCategory(CAT_SCI)`
// → `fact_bit::SCI_PRESENT`; `TOK_US_COLLATERAL_CLASSIFIED` →
// `fact_bit::US_COLLATERAL_CLASSIFIED`) and cone (`TOK_RELIDO` →
// `fact_bit::RELIDO`) compile to closed-vocab bit form. The suppressors
// project to `MASK_FDR_OR_RELIDO_INCOMPAT` and
// `MASK_RELIDO_US_CLASS_SUPPRESSORS` in `fact_bitmask.rs`. See
// `CLOSURE_TABLE` Rows 8–9 in `closure_table.rs` for the bitmask form
// + §-citation preservation.
//
// The `FDR_OR_RELIDO_INCOMPAT` and `RELIDO_US_CLASS_SUPPRESSORS`
// `TokenRef` slices stay because the in-file `#[cfg(test)]` modules
// (`phase2_closure_pin::every_relido_incompat_entry_suppresses_trio2_relido`,
// `phase3_closure_pin::every_us_class_suppressor_entry_suppresses_trio2_relido`)
// iterate them as fixture enumerations — for each suppressor entry they
// build a `CapcoMarking` carrying the matching attrs-axis presence and
// assert `scheme.closure(...)` (now the bitmask path) does not inject
// RELIDO. The slices remain the source-of-truth list the tests
// enumerate. A follow-on PR migrating the tests to iterate
// `MASK_FDR_OR_RELIDO_INCOMPAT` / `MASK_RELIDO_US_CLASS_SUPPRESSORS`
// bits will retire them.

// `RELIDO_US_CLASS_SUPPRESSORS` — the suppressor slice formerly attached
// to the retired `CLOSURE_RELIDO_US_CLASS` rule.
//
// Encodes the FD&R precedence rule from CAPCO-2016 §D.2 Table 3
// pp.28-30: an explicit FD&R decision (NOFORN, RELIDO, REL TO,
// DISPLAY ONLY, EYES) supersedes the implicit-RELIDO default. The
// slice contains the FD&R dominator set per §H.8 p145 (five
// entries) plus six per-compartment SCI sentinels whose case-2
// monotonicity property is satisfied by closure composition — see
// the per-block doc-comments below.
//
// **Monotonicity (load-bearing).** Every suppressor in this slice
// has the `marque-applied.md` Section 4.7.3 case-2 property:
// "the suppressor either contains the cone's intent or makes the
// cone inapplicable." The five FD&R dominators satisfy this
// directly — adding any of them to a marking lifts the dissem-
// axis state to a point at or above {RELIDO} in the
// SupersessionSet lattice (NOFORN ⊐ RELIDO; REL TO / DISPLAY ONLY
// / EYES are mutually-exclusive with RELIDO and supersede it in
// §D.2 Table 3 rows 12-17; RELIDO contains RELIDO trivially). The
// six SCI per-compartment sentinels satisfy it by composition —
// each compartment's per-marking row adds NOFORN or ORCON, and
// CAVEATED (Trio 1) promotes ORCON → NOFORN, so the composite
// Cl(y) contains NOFORN ⊐ RELIDO whenever any of these
// compartments is present. The composite monotonicity invariant
// (`m1 ⊑ m2 ⇒ closure(m1) ⊑ closure(m2)`) is verified by the
// `proptest_closure` harness on synthetic schemes and by the
// `us_class_monotone_under_added_*` Phase 3 pins.
//
// **Why "no other dissem" lives in the trigger, not here.** A
// prior revision of this PR encoded the `marque-applied.md`
// Section 4.7.5 "no other dissem" qualifier as a set of category-
// level suppressors (CAT_DISSEM, CAT_NON_IC_DISSEM, CAT_AEA,
// CAT_SCI, CAT_SAR, CAT_NON_US_CLASSIFICATION) plus a
// `TOK_US_UNCLASSIFIED` suppressor for the §H.8 p154 carve-out.
// Copilot review (PR #544 HIGH) correctly flagged this as
// anti-monotone: adding any other dissem-axis fact to a bare
// US-Secret marking would suppress the rule, leaving
// `Cl(y).dissem_us` lacking RELIDO even though `Cl(x).dissem_us`
// contained it — `Cl(x) ⊑ Cl(y)` would not hold. The redesign
// moves the gate into the trigger (`TOK_US_COLLATERAL_CLASSIFIED`)
// and relies on closure composition (Trio 1
// `CLOSURE_NOFORN_CAVEATED` injects NOFORN on any caveat marker;
// NOFORN then supersedes RELIDO via `with_noforn_injected`) to
// produce the correct §B.3 Table 2 p21 semantic on caveated
// inputs.
//
// **Conflict-variant note.** `TOK_US_COLLATERAL_CLASSIFIED` fires
// on `MarkingClassification::Conflict { us, foreign }` when the
// US side is collateral classified, because
// `attrs.us_classification()` returns the resolved US side. The
// closure adds RELIDO on Conflict markings whose US side is
// classified collateral; this is acceptable because Conflict is
// a parser-flagged structural error condition (a single marking
// declaring both US and non-US classification), and the implicit
// RELIDO addition is downstream of the Conflict diagnostic. The
// foreign-equity side will independently trigger
// `CLOSURE_NOFORN_CAVEATED` (any FGI / NATO atom in the dissem
// axis) which injects NOFORN and supersedes the RELIDO via the
// §H.8 p145 overlay. Pinned by
// `phase3_closure_pin::us_class_conflict_variant_pin`.
// `#[cfg(test)]` enforces the test-only contract: the
// `phase3_closure_pin` test module — the sole consumer post-PR-D —
// iterates this slice as the TokenRef source-of-truth that backstops
// the bitmask `MASK_RELIDO_US_CLASS_SUPPRESSORS` projection.  Any
// future production caller is a compile error in a `cfg(not(test))`
// build, not a silently-pinged dead-code warning.
#[cfg(test)]
const RELIDO_US_CLASS_SUPPRESSORS: &[TokenRef] = &[
    // FD&R dominators — every entry supersedes RELIDO via the
    // §D.2 Table 3 pp.28-30 / §H.8 p145 precedence overlay,
    // satisfying the §4.7.3 case-2 monotonicity property
    // directly.
    TokenRef::Token(TOK_NOFORN),
    TokenRef::Token(TOK_RELIDO),
    TokenRef::Token(TOK_DISPLAY_ONLY),
    TokenRef::AnyInCategory(CAT_REL_TO),
    TokenRef::Token(TOK_EYES),
    // Per-compartment SCI sentinels (`marque-applied.md`
    // Section 4.7.5 exclusion list): SI-G / HCS-O / HCS-P[sub] /
    // TK-BLFH / TK-IDIT / TK-KAND. Each compartment's per-marking
    // closure row adds NOFORN or ORCON; CAVEATED (Trio 1) then
    // promotes ORCON → NOFORN (it's in CAVEATED's trigger list).
    // The composite Kleene fixpoint therefore yields NOFORN
    // whenever any of these compartments is present, and NOFORN
    // supersedes RELIDO via §H.8 p145. So although none of these
    // sentinels directly dominates RELIDO at the same iteration,
    // their presence guarantees the final-state dissem axis
    // contains NOFORN ⊐ RELIDO — satisfying the §4.7.3 case-2
    // "makes the cone inapplicable" clause via closure composition.
    //
    // Why this is necessary: SI-G specifically adds ORCON only
    // (no direct NOFORN). Without suppressing US_CLASS on SI-G,
    // US_CLASS would fire on iteration 1 and inject RELIDO; RELIDO
    // would then suppress CAVEATED on iteration 2 (RELIDO is in
    // FDR_DOMINATORS), preventing the NOFORN injection and leaving
    // RELIDO in the final state — semantically wrong per §4.7.5.
    // Suppressing US_CLASS directly on SI-G lets CAVEATED→NOFORN
    // run in iteration 2 with no RELIDO present to block it.
    TokenRef::Token(TOK_SI_G),
    TokenRef::Token(TOK_HCS_O),
    TokenRef::Token(TOK_HCS_P_SUB),
    TokenRef::Token(TOK_TK_BLFH),
    TokenRef::Token(TOK_TK_IDIT),
    TokenRef::Token(TOK_TK_KAND),
];

// `CLOSURE_RELIDO_US_CLASS` was retired in PR-D — see `CLOSURE_TABLE`
// Row 9 in `closure_table.rs` for the bitmask form. Primary authority
// (CAPCO-2016 §B.3 Table 2 p21, rooted in ICD 403; grammar: §H.8 p154)
// is preserved on the bitmask row's `label` field.

/// The residual CAPCO closure-rule catalog.
///
/// Post-PR-D this slice carries only `CLOSURE_REL_TO_USA_NATO`, the
/// hybrid bitmask + open-vocab row whose `cone_derived` NATO tetragraph
/// injection cannot be projected onto a closed-vocab bit. The other 9
/// rows live in `CLOSURE_TABLE` (`scheme/closure_table.rs`) and run
/// through the bitmask Kleene fast path in
/// `CapcoScheme::closure`. Three trait-level concerns still consume this
/// slice:
///
/// - The `MarkingScheme::closure_rules` trait override exposes the
///   public catalog surface per `decisions.md` D18 — it advertises
///   the rules the scheme owns at the trait boundary regardless of
///   internal dispatch strategy.
/// - The `post_4b_lattice_inventory_pin.rs` positional-list test
///   asserts a closed set of rule names against this slice (now
///   1 row); the 10-row bitmask catalog has its own parallel pin
///   against `CLOSURE_TABLE`.
/// - A future `[closure_rules]` severity-override config path
///   (analogous to the existing `[rules]` section in
///   `crates/config/`) will need a runtime override map keyed by
///   rule name. No such resolver exists in the repo today; the path
///   that lands it MUST also handle the post-PR-D discovery-surface
///   gap tracked in issue #644 (the 9 bitmask rules' names live on
///   `CLOSURE_TABLE` row `label` fields, not on this slice).
pub(super) static CAPCO_CLOSURE_RULES: &[ClosureRule<CapcoScheme>] = &[CLOSURE_REL_TO_USA_NATO];

// ---------------------------------------------------------------------------
// Runtime suppression pin for `FDR_DOMINATORS` × `CLOSURE_NOFORN_CAVEATED`
// ---------------------------------------------------------------------------

/// Runtime companion to `vocabulary::fdr_dissem_pin`.
///
/// `fdr_dissem_pin` walks `FDR_DOMINATORS` at the `Vocabulary::is_fdr_dissem`
/// predicate layer. This module walks the same slice at the
/// `MarkingScheme::closure` runtime layer: for each `FDR_DOMINATORS` entry,
/// build a Trio 1 trigger (classified Secret + ORCON) plus that entry as a
/// suppressor, and assert `CLOSURE_NOFORN_CAVEATED` does not inject NOFORN.
/// The two surfaces are independently testable per the issue: the predicate
/// can resolve correctly while the runtime suppression wiring drifts (or
/// vice versa), so each gets its own pin.
///
/// The fixture-construction `match` is exhaustive against the *patterns*
/// `FDR_DOMINATORS` uses today (`TokenRef::Token(t)` for the four sentinel
/// dominators, `TokenRef::AnyInCategory(c)` for `CAT_REL_TO`). A future
/// addition that fits an existing pattern but a new `TokenId` /
/// `CategoryId` falls through to the panic arm and fails the test with a
/// message naming the unmapped entry — the right failure mode for the
/// source-of-truth drift this pin is supposed to catch.
///
/// Authority: §B.3.a p19 (core FD&R enumeration:
/// NOFORN/REL TO/RELIDO/DISPLAY ONLY); §H.8 p157 (EYES designated FD&R,
/// deprecated 2017-10-01 but still recognized). The two citations
/// together cover the full `FDR_DOMINATORS` slice — `§B.3.a p19` alone
/// would mis-attribute the EYES arm.
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod fdr_dominators_runtime_pin {
    use super::*;
    use marque_ism::{
        CanonicalAttrs, Classification, CountryCode, DissemControl, MarkingClassification,
    };
    use marque_scheme::MarkingScheme;

    /// Build the per-FDR-dominator suppression fixture: classified Secret
    /// and ORCON (a Trio 1 trigger from `CLOSURE_NOFORN_CAVEATED`) and the
    /// given suppressor. The fixture mutates `dissem_us` for the four
    /// `Token(...)` dominators and `rel_to` for `AnyInCategory(CAT_REL_TO)`
    /// because the runtime `satisfies_attrs` resolution routes each
    /// `TokenRef` against the matching `CanonicalAttrs` axis.
    fn fixture_with_suppressor(suppressor: &TokenRef) -> CapcoMarking {
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Us(Classification::Secret));
        // ORCON is the Trio 1 trigger — present on every fixture so the
        // CAVEATED row would fire absent the suppressor.
        let mut dissem = vec![DissemControl::Oc];
        match suppressor {
            // NOTE: The `TOK_NOFORN` arm is a fixture-completeness check,
            // not a suppression-correctness check. The cone fact `Nf`
            // would dedup against the fixture-planted `Nf` regardless of
            // whether the suppressor fires — a broken suppressor here is
            // observationally identical to a working one. Kept in the
            // iteration so a future `FDR_DOMINATORS` addition matching
            // the `Token(...)` pattern is not silently skipped. See the
            // test-function doc-comment for the full rationale.
            TokenRef::Token(t) if *t == TOK_NOFORN => dissem.push(DissemControl::Nf),
            TokenRef::Token(t) if *t == TOK_RELIDO => dissem.push(DissemControl::Relido),
            TokenRef::Token(t) if *t == TOK_DISPLAY_ONLY => dissem.push(DissemControl::Displayonly),
            TokenRef::Token(t) if *t == TOK_EYES => dissem.push(DissemControl::Eyes),
            TokenRef::AnyInCategory(c) if *c == CAT_REL_TO => {
                // `REL TO USA, GBR` — `AnyInCategory(CAT_REL_TO)` matches
                // any non-empty `attrs.rel_to`; USA is the required leader
                // per §H.8 p150 and GBR a representative partner trigraph.
                a.rel_to = vec![CountryCode::USA, CountryCode::GBR].into_boxed_slice();
            }
            other => panic!(
                "fdr_dominators_runtime_pin: no fixture mapping for \
                 FDR_DOMINATORS entry {other:?}. A new dominator was added \
                 to the slice; extend the match in fixture_with_suppressor \
                 with a `CanonicalAttrs` mutation that the runtime \
                 `satisfies_attrs` resolution will recognize as that token \
                 (or category) being present.",
            ),
        }
        a.dissem_us = dissem.into_boxed_slice();
        CapcoMarking::new(a)
    }

    /// Source-of-truth pin: every entry in `FDR_DOMINATORS` must suppress
    /// `CLOSURE_NOFORN_CAVEATED`. A drift in the slice or in the
    /// `satisfies_attrs` resolution for any entry fails this test with a
    /// message naming the failing entry.
    ///
    /// The assertion shape is "closure adds no facts" rather than "NOFORN
    /// is absent from post-closure `dissem_us`". The latter is unworkable
    /// for the `TOK_NOFORN` case (where the fixture must populate
    /// `dissem_us` with `Nf` for `satisfies_attrs(TOK_NOFORN)` to resolve
    /// true, so post-closure `Nf` is unavoidably present). The
    /// length-stability assertion is uniformly meaningful: if a
    /// suppressor fails for any non-self-referential dominator, the
    /// CAVEATED row fires and adds a `Nf` fact, growing `dissem_us` by
    /// one. The `TOK_NOFORN` arm is a trivial smoke check (the cone fact
    /// would dedup against the pre-existing fact, so growth would be
    /// zero even on a broken suppressor) but is included so the iteration
    /// covers the full slice without a special-case skip.
    #[test]
    fn every_fdr_dominator_suppresses_caveated_noforn_injection() {
        let scheme = CapcoScheme::new();
        for suppressor in FDR_DOMINATORS {
            let m = fixture_with_suppressor(suppressor);
            let dissem_before = m.0.dissem_us.clone();
            let rel_to_before = m.0.rel_to.clone();
            let closed = scheme.closure(m);
            assert_eq!(
                closed.0.dissem_us.len(),
                dissem_before.len(),
                "FDR_DOMINATORS entry {:?} did NOT suppress \
                 `CLOSURE_NOFORN_CAVEATED`: closure grew `dissem_us` from \
                 {} to {} despite the explicit FD&R decision being \
                 present. Either the suppressor wiring drifted or \
                 `satisfies_attrs(...)` no longer resolves this entry \
                 against the populated attrs axis. Authority: §B.3.a p19 \
                 (core FD&R enumeration) + §H.8 p157 (EYES). Pre-closure \
                 dissem_us = {:?}; post-closure dissem_us = {:?}, \
                 rel_to = {:?}.",
                suppressor,
                dissem_before.len(),
                closed.0.dissem_us.len(),
                dissem_before,
                closed.0.dissem_us,
                closed.0.rel_to,
            );
            assert_eq!(
                closed.0.rel_to.len(),
                rel_to_before.len(),
                "FDR_DOMINATORS entry {:?} did NOT suppress \
                 `CLOSURE_NOFORN_CAVEATED`: closure grew `rel_to` from \
                 {} to {}. CAVEATED has cone `{{NOFORN}}` (no rel_to \
                 facts), so growth here means a different closure row \
                 fired unexpectedly. Pre-closure rel_to = {:?}; \
                 post-closure rel_to = {:?}.",
                suppressor,
                rel_to_before.len(),
                closed.0.rel_to.len(),
                rel_to_before,
                closed.0.rel_to,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Issue #524 Phase 2 — per-marking unconditional + Trio 2 RELIDO pin
// ---------------------------------------------------------------------------

/// Phase 2 closure-row pins.
///
/// Covers:
///   1. Each per-marking unconditional row fires its specified cone
///      on a minimal trigger fixture (`per_marking_*` tests).
///   2. The Trio 2 `CLOSURE_RELIDO_SCI` row fires RELIDO on a bare
///      SCI control absent any suppressor.
///   3. Every entry in `FDR_OR_RELIDO_INCOMPAT` suppresses
///      `CLOSURE_RELIDO_SCI` when paired with a bare SCI control —
///      the runtime companion to `FDR_OR_RELIDO_INCOMPAT`'s source-
///      of-truth role for Trio 2 suppression.
///   4. The grammar-shape sentinel `TOK_HCS_P_SUB` discriminates
///      bare HCS-P (no sub) from HCS-P with sub-compartments.
///
/// Authority: CAPCO-2016 §H.4 marking templates (p64 HCS-O, p66/p68
/// HCS-P, p80 SI-G, p87 TK-BLFH, p91 TK-IDIT, p95 TK-KAND); §H.8
/// p154 (RELIDO foundational citation for Trio 2).
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod phase2_closure_pin {
    use super::*;
    use marque_ism::{
        CanonicalAttrs, Classification, CountryCode, DissemControl, MarkingClassification,
        SciCompartment, SciControlBare, SciControlSystem, SciMarking,
    };
    use marque_scheme::MarkingScheme;
    use smol_str::SmolStr;

    /// Build a `CapcoMarking` with a single SciMarking anchored on
    /// `system` carrying one compartment `identifier`. Optional
    /// `sub_compartments` are attached to the compartment.
    fn sci_marking(
        system: SciControlBare,
        identifier: &str,
        sub_compartments: Vec<&str>,
    ) -> CapcoMarking {
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
        let comp = SciCompartment::new(
            SmolStr::new(identifier),
            sub_compartments
                .into_iter()
                .map(SmolStr::new)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let marking = SciMarking::new(SciControlSystem::Published(system), Box::new([comp]), None);
        a.sci_markings = Box::new([marking]);
        CapcoMarking::new(a)
    }

    /// Build a `CapcoMarking` that triggers `CLOSURE_RELIDO_SCI` (any
    /// SCI control present) but carries the given suppressor in the
    /// matching attrs axis so the row should NOT fire.
    fn relido_sci_suppression_fixture(suppressor: &TokenRef) -> CapcoMarking {
        // Start with a bare SI compartment that does NOT match any
        // per-compartment NOFORN-implying sentinel — picks a synthetic
        // compartment name `Z9` so neither TOK_SI_G nor any TK sentinel
        // fires. The SCI presence still triggers Trio 2.
        let mut m = sci_marking(SciControlBare::Si, "Z9", vec![]);
        match suppressor {
            // FD&R-axis tokens populate dissem_us / rel_to.
            TokenRef::Token(t) if *t == TOK_NOFORN => {
                m.0.dissem_us = Box::new([DissemControl::Nf]);
            }
            TokenRef::Token(t) if *t == TOK_RELIDO => {
                m.0.dissem_us = Box::new([DissemControl::Relido]);
            }
            TokenRef::Token(t) if *t == TOK_DISPLAY_ONLY => {
                m.0.dissem_us = Box::new([DissemControl::Displayonly]);
            }
            TokenRef::Token(t) if *t == TOK_EYES => {
                m.0.dissem_us = Box::new([DissemControl::Eyes]);
            }
            TokenRef::AnyInCategory(c) if *c == CAT_REL_TO => {
                m.0.rel_to = vec![CountryCode::USA, CountryCode::GBR].into_boxed_slice();
            }
            // FGI marker — populates the FGI marker axis. SourceConcealed
            // is the FGI bare form (§H.7 p122); Acknowledged carries a
            // non-empty country list (§H.7 p123).
            TokenRef::Token(t) if *t == TOK_FGI_MARKER => {
                m.0.fgi_marker = Some(marque_ism::FgiMarker::SourceConcealed);
            }
            TokenRef::AnyInCategory(c) if *c == CAT_FGI_MARKER => {
                m.0.fgi_marker = marque_ism::FgiMarker::acknowledged([CountryCode::GBR]);
            }
            // FGI classification — Fgi variant carries `countries` + `level`.
            TokenRef::Token(t) if *t == TOK_FGI_CLASS => {
                m.0.classification =
                    Some(MarkingClassification::Fgi(marque_ism::FgiClassification {
                        countries: vec![CountryCode::GBR].into_boxed_slice(),
                        level: Classification::Secret,
                    }));
            }
            // JOINT classification — Joint variant carries `level` +
            // `countries` (must include USA).
            TokenRef::Token(t) if *t == TOK_JOINT => {
                m.0.classification = Some(MarkingClassification::Joint(
                    marque_ism::JointClassification {
                        level: Classification::Secret,
                        countries: vec![CountryCode::USA, CountryCode::GBR].into_boxed_slice(),
                    },
                ));
            }
            // NATO classification — `NatoSecret` is the §G.1 Table 4 p38
            // NS variant.
            TokenRef::Token(t) if *t == TOK_NATO_CLASS => {
                m.0.classification = Some(MarkingClassification::Nato(
                    marque_ism::NatoClassification::NatoSecret,
                ));
            }
            // Per-compartment SCI sentinels — replace the fixture's
            // SCI marking with the matching compartment shape.
            TokenRef::Token(t) if *t == TOK_SI_G => {
                m = sci_marking(SciControlBare::Si, "G", vec![]);
            }
            TokenRef::Token(t) if *t == TOK_HCS_O => {
                m = sci_marking(SciControlBare::Hcs, "O", vec![]);
            }
            TokenRef::Token(t) if *t == TOK_HCS_P_SUB => {
                m = sci_marking(SciControlBare::Hcs, "P", vec!["ABCD"]);
            }
            TokenRef::Token(t) if *t == TOK_TK_BLFH => {
                m = sci_marking(SciControlBare::Tk, "BLFH", vec![]);
            }
            TokenRef::Token(t) if *t == TOK_TK_IDIT => {
                m = sci_marking(SciControlBare::Tk, "IDIT", vec![]);
            }
            TokenRef::Token(t) if *t == TOK_TK_KAND => {
                m = sci_marking(SciControlBare::Tk, "KAND", vec![]);
            }
            other => panic!(
                "phase2_closure_pin: no fixture mapping for \
                 FDR_OR_RELIDO_INCOMPAT entry {other:?}. A new entry \
                 was added to the slice; extend the match in \
                 relido_sci_suppression_fixture with a `CanonicalAttrs` \
                 mutation that the runtime `satisfies_attrs` resolution \
                 will recognize as that token being present.",
            ),
        }
        m
    }

    /// HCS-O ⇒ {NOFORN, ORCON}. §H.4 p64.
    #[test]
    fn per_marking_hcs_o_implies_nf_oc() {
        let scheme = CapcoScheme::new();
        let m = sci_marking(SciControlBare::Hcs, "O", vec![]);
        let closed = scheme.closure(m);
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Nf),
            "HCS-O closure should add NOFORN; dissem_us = {:?}",
            closed.0.dissem_us
        );
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Oc),
            "HCS-O closure should add ORCON; dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    /// HCS-P [sub] ⇒ {NOFORN, ORCON}. §H.4 p68.
    #[test]
    fn per_marking_hcs_p_sub_implies_nf_oc() {
        let scheme = CapcoScheme::new();
        let m = sci_marking(SciControlBare::Hcs, "P", vec!["JJJ"]);
        let closed = scheme.closure(m);
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Nf),
            "HCS-P[sub] closure should add NOFORN; dissem_us = {:?}",
            closed.0.dissem_us
        );
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Oc),
            "HCS-P[sub] closure should add ORCON; dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    /// Bare HCS-P (no sub) does NOT trip the HCS-P[sub] closure
    /// row — ORCON must not be added. §H.4 p66 Example Banner Line
    /// is `SECRET//HCS-P//NOFORN` (no ORCON); §H.4 p68 distinguishes
    /// the sub-compartmented form (which adds ORCON via
    /// `CLOSURE_HCS_P_SUB_IMPLIES_NF_OC`). Bare HCS-P additionally
    /// triggers `CLOSURE_RELIDO_SCI` (it sits in `CAT_SCI` and is
    /// not in `FDR_OR_RELIDO_INCOMPAT`), so RELIDO may appear in
    /// post-closure `dissem_us`; this test asserts only that ORCON
    /// is absent — the load-bearing property for the p66/p68
    /// distinction.
    #[test]
    fn per_marking_hcs_p_bare_does_not_imply_orcon() {
        let scheme = CapcoScheme::new();
        let m = sci_marking(SciControlBare::Hcs, "P", vec![]);
        let closed = scheme.closure(m);
        assert!(
            !closed.0.dissem_us.contains(&DissemControl::Oc),
            "bare HCS-P closure must NOT add ORCON (§H.4 p66 vs p68 \
             distinguishes bare from sub-compartmented). dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    /// SI-G ⇒ {ORCON}. §H.4 p80. NOFORN must NOT be in SI-G's cone
    /// — the §H.4 p80 Example Banner Line is `TOP SECRET//SI-G//ORCON`
    /// (ORCON only). NOFORN may appear from another closure row but
    /// not from SI-G's per-marking row.
    #[test]
    fn per_marking_si_g_implies_oc_only() {
        let scheme = CapcoScheme::new();
        let m = sci_marking(SciControlBare::Si, "G", vec![]);
        let closed = scheme.closure(m);
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Oc),
            "SI-G closure should add ORCON; dissem_us = {:?}",
            closed.0.dissem_us
        );
        // SI-G is in FDR_OR_RELIDO_INCOMPAT, so Trio 2 RELIDO must
        // NOT fire either.
        assert!(
            !closed.0.dissem_us.contains(&DissemControl::Relido),
            "SI-G must be excluded from Trio 2 RELIDO (per \
             marque-applied §4.7.5); dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    /// TK-BLFH ⇒ {NOFORN}. §H.4 p87.
    #[test]
    fn per_marking_tk_blfh_implies_nf() {
        let scheme = CapcoScheme::new();
        let m = sci_marking(SciControlBare::Tk, "BLFH", vec![]);
        let closed = scheme.closure(m);
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Nf),
            "TK-BLFH closure should add NOFORN; dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    /// TK-IDIT ⇒ {NOFORN}. §H.4 p91.
    #[test]
    fn per_marking_tk_idit_implies_nf() {
        let scheme = CapcoScheme::new();
        let m = sci_marking(SciControlBare::Tk, "IDIT", vec![]);
        let closed = scheme.closure(m);
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Nf),
            "TK-IDIT closure should add NOFORN; dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    /// TK-KAND ⇒ {NOFORN}. §H.4 p95.
    #[test]
    fn per_marking_tk_kand_implies_nf() {
        let scheme = CapcoScheme::new();
        let m = sci_marking(SciControlBare::Tk, "KAND", vec![]);
        let closed = scheme.closure(m);
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Nf),
            "TK-KAND closure should add NOFORN; dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    /// Idempotence: closing an already-closed marking is stable.
    /// Picks HCS-O which exercises both NOFORN and ORCON cone facts.
    #[test]
    fn per_marking_idempotent() {
        let scheme = CapcoScheme::new();
        let m = sci_marking(SciControlBare::Hcs, "O", vec![]);
        let once = scheme.closure(m);
        let twice = scheme.closure(once.clone());
        assert_eq!(
            once, twice,
            "closure must be idempotent (Constitution Principle II algebraic \
             contract); once = {once:?}, twice = {twice:?}"
        );
    }

    /// Trio 2: bare SCI control (here SI-Z9, no per-marking
    /// sentinel match) implies RELIDO unless suppressed.
    #[test]
    fn trio2_relido_fires_on_bare_sci() {
        let scheme = CapcoScheme::new();
        let m = sci_marking(SciControlBare::Si, "Z9", vec![]);
        let closed = scheme.closure(m);
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Relido),
            "bare SCI should fire CLOSURE_RELIDO_SCI; dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    /// Source-of-truth pin: every entry in `FDR_OR_RELIDO_INCOMPAT`
    /// must suppress `CLOSURE_RELIDO_SCI` when paired with a bare
    /// SCI control. Drift in the slice or in the `satisfies_attrs`
    /// resolution for any entry fails this test with a message
    /// naming the failing entry.
    #[test]
    fn every_relido_incompat_entry_suppresses_trio2_relido() {
        let scheme = CapcoScheme::new();
        for suppressor in FDR_OR_RELIDO_INCOMPAT {
            // RELIDO itself is observationally identical to a working
            // suppressor (cone is `{RELIDO}`, fixture has `RELIDO` →
            // dedup) — kept in the iteration for completeness but
            // skipped from the strict assertion.
            let is_self_relido = matches!(suppressor, TokenRef::Token(t) if *t == TOK_RELIDO);
            let m = relido_sci_suppression_fixture(suppressor);
            let closed = scheme.closure(m);
            if !is_self_relido {
                // No non-RELIDO fixture seeds `DissemControl::Relido`
                // in the attrs (the `TOK_RELIDO` arm does, and is
                // skipped via `is_self_relido`). Therefore any
                // post-closure `Relido` came from
                // `CLOSURE_RELIDO_SCI` firing — exactly what the
                // suppressor is supposed to prevent. Assert
                // strict absence.
                assert!(
                    !closed.0.dissem_us.contains(&DissemControl::Relido),
                    "FDR_OR_RELIDO_INCOMPAT entry {suppressor:?} did NOT suppress \
                     `CLOSURE_RELIDO_SCI`: RELIDO appeared in post-closure dissem_us \
                     despite the suppressor being present. Either the suppressor \
                     wiring drifted or `satisfies_attrs(...)` no longer resolves this \
                     entry against the populated attrs axis. Authority: \
                     marque-applied §4.7.1 has_relido_incompatible. \
                     post-closure dissem_us = {:?}, fgi_marker = {:?}, classification \
                     = {:?}, rel_to = {:?}",
                    closed.0.dissem_us,
                    closed.0.fgi_marker,
                    closed.0.classification,
                    closed.0.rel_to,
                );
            }
        }
    }

    /// Coexistence: a marking carrying BOTH HCS-P[sub] (cone:
    /// {NOFORN, ORCON}) and TK-BLFH (cone: {NOFORN}) — the kind of
    /// commingled portion that §H.4 commingling rules permit
    /// (e.g., `TOP SECRET//HCS-P JJJ/TK-BLFH//ORCON/NOFORN`) — must
    /// close to exactly `{NOFORN, ORCON}` and must NOT produce
    /// RELIDO from Trio 2.
    ///
    /// Two independent suppression paths converge here:
    ///   1. Direct token: both `TOK_HCS_P_SUB` and `TOK_TK_BLFH` are
    ///      in `FDR_OR_RELIDO_INCOMPAT`.
    ///   2. Kleene chain: both per-marking rows add NOFORN, and
    ///      NOFORN ∈ `FDR_OR_RELIDO_INCOMPAT`.
    ///
    /// Idempotence preserves NOFORN as a singleton in `dissem_us`
    /// despite two cone rows adding it.
    #[test]
    fn coexistence_hcs_p_sub_and_tk_blfh_produces_nf_oc_no_relido() {
        let scheme = CapcoScheme::new();
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
        let hcs_p = SciCompartment::new(
            SmolStr::new("P"),
            vec![SmolStr::new("JJJ")].into_boxed_slice(),
        );
        let tk_blfh = SciCompartment::new(SmolStr::new("BLFH"), Box::new([]));
        let hcs_marking = SciMarking::new(
            SciControlSystem::Published(SciControlBare::Hcs),
            Box::new([hcs_p]),
            None,
        );
        let tk_marking = SciMarking::new(
            SciControlSystem::Published(SciControlBare::Tk),
            Box::new([tk_blfh]),
            None,
        );
        a.sci_markings = Box::new([hcs_marking, tk_marking]);
        let m = CapcoMarking::new(a);
        let closed = scheme.closure(m);
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Nf),
            "HCS-P[sub] + TK-BLFH should produce NOFORN; dissem_us = {:?}",
            closed.0.dissem_us
        );
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Oc),
            "HCS-P[sub] + TK-BLFH should produce ORCON (from HCS-P[sub] row); \
             dissem_us = {:?}",
            closed.0.dissem_us
        );
        assert!(
            !closed.0.dissem_us.contains(&DissemControl::Relido),
            "HCS-P[sub] + TK-BLFH must NOT produce RELIDO (both compartments \
             are in FDR_OR_RELIDO_INCOMPAT); dissem_us = {:?}",
            closed.0.dissem_us
        );
        // Idempotence: NOFORN appears once, not twice, despite two
        // per-marking rows adding it.
        let nf_count = closed
            .0
            .dissem_us
            .iter()
            .filter(|d| **d == DissemControl::Nf)
            .count();
        assert_eq!(
            nf_count, 1,
            "NOFORN must be deduplicated by closure (idempotence); \
             observed {nf_count} occurrences in dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    /// Per-compartment NOFORN-cone rows (HCS-O, TK-BLFH/IDIT/KAND,
    /// HCS-P[sub]) suppress Trio 2 RELIDO via the Kleene-fixpoint
    /// NOFORN-injection-then-suppression chain. This is a separate
    /// assertion from the `FDR_OR_RELIDO_INCOMPAT` source-of-truth
    /// pin: the chain-via-NOFORN suppression and the direct-token
    /// suppression are two independent paths to the same outcome
    /// for these compartments, and both should hold.
    #[test]
    fn noforn_implying_sci_compartments_suppress_trio2_via_kleene() {
        let scheme = CapcoScheme::new();
        let fixtures = [
            (SciControlBare::Hcs, "O", vec![]),
            (SciControlBare::Hcs, "P", vec!["ABCD"]),
            (SciControlBare::Tk, "BLFH", vec![]),
            (SciControlBare::Tk, "IDIT", vec![]),
            (SciControlBare::Tk, "KAND", vec![]),
        ];
        for (system, comp, sub) in fixtures {
            let m = sci_marking(system, comp, sub.clone());
            let closed = scheme.closure(m);
            assert!(
                !closed.0.dissem_us.contains(&DissemControl::Relido),
                "{system:?}-{comp} (sub={sub:?}) should suppress Trio 2 RELIDO; \
                 dissem_us = {:?}",
                closed.0.dissem_us
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Issue #524 Phase 3 — Trio 2 RELIDO completion pins (US_CLASS)
// ---------------------------------------------------------------------------

/// Phase 3 closure-row pins.
///
/// Covers:
///   1. `CLOSURE_RELIDO_US_CLASS` fires RELIDO on US collateral
///      classifications (Restricted / Confidential / Secret /
///      TopSecret) absent any FD&R-dominator suppressor.
///   2. The §H.8 p154 Unclassified carve-out: bare US Unclassified
///      portions do NOT trigger implicit RELIDO (gated at the
///      trigger level via `TOK_US_COLLATERAL_CLASSIFIED` — the
///      predicate doesn't match `Us(Unclassified)`). Agencies
///      whose internal policy requires U → RELIDO will opt in
///      via a future style rule.
///   3. Every entry in `RELIDO_US_CLASS_SUPPRESSORS` (the
///      FD&R-dominator set: NOFORN / RELIDO / DISPLAY ONLY /
///      REL TO / EYES) suppresses `CLOSURE_RELIDO_US_CLASS` when
///      paired with a US-classified marking — runtime companion to
///      the slice's source-of-truth role.
///   4. Conflict-variant pin documenting the deliberate firing of
///      the closure on `MarkingClassification::Conflict` whose US
///      side is collateral classified.
///   5. Idempotence under repeated closure application.
///
/// Authority: `marque-applied.md` Section 4.7.5 (Trio 2 trigger
/// list); CAPCO-2016 §B.3 Table 2 p21 (defaulting rule — the
/// primary obligation); §H.8 p154 (RELIDO grammar + Unclassified
/// carve-out); §D.2 Table 3 pp.28-30 / §H.8 p145 (FD&R precedence
/// supporting the suppressor monotonicity).
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod phase3_closure_pin {
    use super::*;
    use marque_ism::{
        CanonicalAttrs, Classification, CountryCode, DissemControl, ForeignClassification,
        MarkingClassification, NatoClassification, SciCompartment, SciControlBare,
        SciControlSystem, SciMarking,
    };
    use marque_scheme::MarkingScheme;
    use smol_str::SmolStr;

    /// Build a `CapcoMarking` with a single US classification and
    /// nothing else populated. Trigger for `CLOSURE_RELIDO_US_CLASS`
    /// when `level` is Restricted/Confidential/Secret/TopSecret;
    /// the trigger predicate (`TOK_US_COLLATERAL_CLASSIFIED`)
    /// doesn't fire on `Us(Unclassified)`, so the §H.8 p154
    /// carve-out is enforced at the trigger level (not via a
    /// suppressor) when `level` is Unclassified.
    fn us_classified(level: Classification) -> CapcoMarking {
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Us(level));
        CapcoMarking::new(a)
    }

    /// Mutate a US-Secret base marking so the given FD&R-dominator
    /// suppressor is present in the matching attrs axis. Used by
    /// `every_us_class_suppressor_entry_suppresses_trio2_relido`.
    ///
    /// Panics on unmapped suppressors so a drift in
    /// `RELIDO_US_CLASS_SUPPRESSORS` fails loudly with a fixture-
    /// extension prompt (same pattern as
    /// `phase2_closure_pin::relido_sci_suppression_fixture`).
    fn us_class_suppression_fixture(suppressor: &TokenRef) -> CapcoMarking {
        let mut m = us_classified(Classification::Secret);
        match suppressor {
            // FD&R dominators populating dissem_us / rel_to. All five
            // satisfy the §4.7.3 case-2 monotonicity property — see
            // `RELIDO_US_CLASS_SUPPRESSORS`'s doc-comment.
            TokenRef::Token(t) if *t == TOK_NOFORN => {
                m.0.dissem_us = Box::new([DissemControl::Nf]);
            }
            TokenRef::Token(t) if *t == TOK_RELIDO => {
                m.0.dissem_us = Box::new([DissemControl::Relido]);
            }
            TokenRef::Token(t) if *t == TOK_DISPLAY_ONLY => {
                m.0.dissem_us = Box::new([DissemControl::Displayonly]);
            }
            TokenRef::Token(t) if *t == TOK_EYES => {
                m.0.dissem_us = Box::new([DissemControl::Eyes]);
            }
            TokenRef::AnyInCategory(c) if *c == CAT_REL_TO => {
                m.0.rel_to = vec![CountryCode::USA, CountryCode::GBR].into_boxed_slice();
            }
            // Per-compartment SCI sentinels — each attaches the
            // matching SCI marking to a US-TopSecret base (SI-G's
            // class floor is TS per §H.4 p80; the other compartments
            // are also TS-only, so TS keeps all fixtures within
            // their valid class envelopes). The composite closure
            // produces NOFORN via SI-G → ORCON → CAVEATED → NOFORN
            // (or directly via HCS/TK per-marking rows that add
            // NOFORN themselves), satisfying §4.7.3 case-2
            // monotonicity through composition.
            TokenRef::Token(t) if *t == TOK_SI_G => {
                let comp = SciCompartment::new(SmolStr::new("G"), Box::new([]));
                m.0.classification = Some(MarkingClassification::Us(Classification::TopSecret));
                m.0.sci_markings = Box::new([SciMarking::new(
                    SciControlSystem::Published(SciControlBare::Si),
                    Box::new([comp]),
                    None,
                )]);
            }
            TokenRef::Token(t) if *t == TOK_HCS_O => {
                let comp = SciCompartment::new(SmolStr::new("O"), Box::new([]));
                m.0.classification = Some(MarkingClassification::Us(Classification::TopSecret));
                m.0.sci_markings = Box::new([SciMarking::new(
                    SciControlSystem::Published(SciControlBare::Hcs),
                    Box::new([comp]),
                    None,
                )]);
            }
            TokenRef::Token(t) if *t == TOK_HCS_P_SUB => {
                let comp = SciCompartment::new(
                    SmolStr::new("P"),
                    vec![SmolStr::new("ABCD")].into_boxed_slice(),
                );
                m.0.classification = Some(MarkingClassification::Us(Classification::TopSecret));
                m.0.sci_markings = Box::new([SciMarking::new(
                    SciControlSystem::Published(SciControlBare::Hcs),
                    Box::new([comp]),
                    None,
                )]);
            }
            TokenRef::Token(t) if *t == TOK_TK_BLFH => {
                let comp = SciCompartment::new(SmolStr::new("BLFH"), Box::new([]));
                m.0.classification = Some(MarkingClassification::Us(Classification::TopSecret));
                m.0.sci_markings = Box::new([SciMarking::new(
                    SciControlSystem::Published(SciControlBare::Tk),
                    Box::new([comp]),
                    None,
                )]);
            }
            TokenRef::Token(t) if *t == TOK_TK_IDIT => {
                let comp = SciCompartment::new(SmolStr::new("IDIT"), Box::new([]));
                m.0.classification = Some(MarkingClassification::Us(Classification::TopSecret));
                m.0.sci_markings = Box::new([SciMarking::new(
                    SciControlSystem::Published(SciControlBare::Tk),
                    Box::new([comp]),
                    None,
                )]);
            }
            TokenRef::Token(t) if *t == TOK_TK_KAND => {
                let comp = SciCompartment::new(SmolStr::new("KAND"), Box::new([]));
                m.0.classification = Some(MarkingClassification::Us(Classification::TopSecret));
                m.0.sci_markings = Box::new([SciMarking::new(
                    SciControlSystem::Published(SciControlBare::Tk),
                    Box::new([comp]),
                    None,
                )]);
            }
            other => panic!(
                "phase3_closure_pin: no fixture mapping for \
                 RELIDO_US_CLASS_SUPPRESSORS entry {other:?}. A new entry \
                 was added to the slice; extend the match in \
                 us_class_suppression_fixture with a `CanonicalAttrs` \
                 mutation that the runtime `satisfies_attrs` resolution \
                 will recognize as that token being present. NOTE: only \
                 suppressors satisfying §4.7.3 case-2 monotonicity (direct \
                 FD&R supersession, or composition-via-NOFORN) are valid \
                 additions — see the slice's doc-comment for the redesign \
                 history.",
            ),
        }
        m
    }

    // -----------------------------------------------------------------
    // CLOSURE_RELIDO_US_CLASS — positive firing
    // -----------------------------------------------------------------

    /// Bare US collateral classification (Restricted / Confidential /
    /// Secret / TopSecret) with no other dissem present implies
    /// RELIDO. Primary authority: CAPCO-2016 §B.3 Table 2 p21.
    #[test]
    fn us_class_fires_on_collateral_levels() {
        let scheme = CapcoScheme::new();
        let levels = [
            Classification::Restricted,
            Classification::Confidential,
            Classification::Secret,
            Classification::TopSecret,
        ];
        for level in levels {
            let m = us_classified(level);
            let closed = scheme.closure(m);
            assert!(
                closed.0.dissem_us.contains(&DissemControl::Relido),
                "bare US {level:?} should fire CLOSURE_RELIDO_US_CLASS; dissem_us = {:?}",
                closed.0.dissem_us
            );
        }
    }

    /// Monotonicity sanity pins for `CLOSURE_RELIDO_US_CLASS`.
    ///
    /// `MarkingScheme::closure` contract requires
    /// `m1 ⊑ m2 ⇒ closure(m1) ⊑ closure(m2)`. The
    /// `proptest_closure::closure_is_monotone` harness in
    /// `marque-scheme` covers synthetic schemes; these pins cover
    /// the specific Capco-side scenarios Copilot flagged in the
    /// PR #544 review (HIGH on the prior anti-monotone suppressor
    /// design that this revision replaces).
    ///
    /// In the SupersessionSet dissem lattice:
    ///   - `{} ⊑ {Relido} ⊑ {Nf}` (NOFORN supersedes RELIDO via
    ///     §H.8 p145).
    ///   - `{Relido} ⊑ {Relido, X}` for any X that coexists with
    ///     RELIDO at the same lattice point.
    ///
    /// Each scenario below picks a pair `(x, y)` with `x ⊑ y` (y
    /// has strictly more facts on some axis) and asserts the
    /// post-closure dissem axis preserves the order.
    #[test]
    fn us_class_monotone_under_added_fouo() {
        // x = (S). y = (S, FOUO). FOUO is NOT in CAVEATED's
        // trigger list, so CAVEATED doesn't fire on y. US_CLASS
        // fires on both (FOUO is not a US_CLASS suppressor in the
        // redesigned slice). Both Cl(x) and Cl(y) carry RELIDO.
        let scheme = CapcoScheme::new();
        let cl_x = scheme.closure(us_classified(Classification::Secret));
        let mut a_y = CanonicalAttrs::default();
        a_y.classification = Some(MarkingClassification::Us(Classification::Secret));
        a_y.dissem_us = Box::new([DissemControl::Fouo]);
        let cl_y = scheme.closure(CapcoMarking::new(a_y));
        // Cl(x) ⊑ Cl(y): RELIDO ∈ Cl(x), RELIDO ∈ Cl(y).
        assert!(
            cl_x.0.dissem_us.contains(&DissemControl::Relido),
            "Cl(x = bare S) should contain Relido; dissem_us = {:?}",
            cl_x.0.dissem_us
        );
        assert!(
            cl_y.0.dissem_us.contains(&DissemControl::Relido),
            "Cl(y = S + FOUO) should still contain Relido (FOUO is not a \
             US_CLASS suppressor); dissem_us = {:?}",
            cl_y.0.dissem_us
        );
    }

    #[test]
    fn us_class_monotone_under_added_si_g() {
        // x = (S). y = (S, SI-G). Cl(x) has Relido. Cl(y) has
        // SI-G → ORCON (per-marking row) → NOFORN (CAVEATED on
        // ORCON in iter 2); NOFORN ⊐ Relido via §H.8 p145
        // supersession, so monotonicity holds via lattice
        // ordering (not subset).
        let scheme = CapcoScheme::new();
        let cl_x = scheme.closure(us_classified(Classification::Secret));
        let mut a_y = CanonicalAttrs::default();
        a_y.classification = Some(MarkingClassification::Us(Classification::TopSecret));
        let comp = SciCompartment::new(SmolStr::new("G"), Box::new([]));
        a_y.sci_markings = Box::new([SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            Box::new([comp]),
            None,
        )]);
        let cl_y = scheme.closure(CapcoMarking::new(a_y));
        assert!(
            cl_x.0.dissem_us.contains(&DissemControl::Relido),
            "Cl(x = bare S) should contain Relido; dissem_us = {:?}",
            cl_x.0.dissem_us
        );
        // Cl(y) should contain NOFORN (which ⊐ Relido) and NOT
        // contain Relido (composite path: SI-G → ORCON → CAVEATED
        // → NOFORN; SI-G in US_CLASS suppressor prevents premature
        // RELIDO injection).
        assert!(
            cl_y.0.dissem_us.contains(&DissemControl::Nf),
            "Cl(y = S + SI-G) should contain Nf (via SI-G → ORCON → \
             CAVEATED → NOFORN composition); dissem_us = {:?}",
            cl_y.0.dissem_us
        );
        assert!(
            !cl_y.0.dissem_us.contains(&DissemControl::Relido),
            "Cl(y) should NOT contain Relido — Nf supersedes it; \
             dissem_us = {:?}",
            cl_y.0.dissem_us
        );
        // Monotonicity in the lattice: {Relido} ⊑ {Nf} via §H.8
        // p145 supersession.
    }

    /// §H.8 p154 Unclassified carve-out (Issue #524 Phase 3): bare
    /// US `Unclassified` does NOT trigger `CLOSURE_RELIDO_US_CLASS`
    /// — CAPCO explicitly carves out unclassified content from the
    /// implicit-RELIDO default. Enforced at the trigger level via
    /// `TOK_US_COLLATERAL_CLASSIFIED` not firing on
    /// `Us(Unclassified)` (keeps the rule monotone). Agencies
    /// whose internal policy mandates U → RELIDO will land as an
    /// opt-in style rule in a future PR.
    #[test]
    fn us_class_excluded_for_unclassified() {
        let scheme = CapcoScheme::new();
        let m = us_classified(Classification::Unclassified);
        let closed = scheme.closure(m);
        assert!(
            !closed.0.dissem_us.contains(&DissemControl::Relido),
            "bare US Unclassified must NOT trigger CLOSURE_RELIDO_US_CLASS \
             per §H.8 p154 ('Explicit foreign disclosure and release \
             markings are not required on unclassified information'); \
             dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    /// Source-of-truth pin: every entry in
    /// `RELIDO_US_CLASS_SUPPRESSORS` must result in RELIDO being
    /// absent from the post-closure dissem axis when paired with a
    /// US-classified marking. Drift in the slice (or in the
    /// `satisfies_attrs` resolution for any entry) fails this test
    /// naming the entry.
    ///
    /// **`TOK_RELIDO` carve-out.** The cone itself; RELIDO is
    /// present in the fixture seed, so the assertion would always
    /// pass trivially. Skipped via `is_self_relido` guard.
    ///
    /// The remaining four entries (NOFORN, DISPLAY_ONLY, EYES,
    /// CAT_REL_TO) exercise true FD&R-dominator suppression — each
    /// supersedes RELIDO via the §H.8 p145 / §D.2 Table 3 pp.28-30
    /// precedence overlay.
    #[test]
    fn every_us_class_suppressor_entry_suppresses_trio2_relido() {
        let scheme = CapcoScheme::new();
        for suppressor in RELIDO_US_CLASS_SUPPRESSORS {
            let is_self_relido = matches!(suppressor, TokenRef::Token(t) if *t == TOK_RELIDO);
            let m = us_class_suppression_fixture(suppressor);
            let closed = scheme.closure(m);
            if !is_self_relido {
                assert!(
                    !closed.0.dissem_us.contains(&DissemControl::Relido),
                    "RELIDO_US_CLASS_SUPPRESSORS entry {suppressor:?} did NOT suppress \
                     `CLOSURE_RELIDO_US_CLASS`: RELIDO appeared in post-closure dissem_us \
                     despite the suppressor being present. Either the suppressor wiring \
                     drifted or `satisfies_attrs(...)` no longer resolves this entry. \
                     Authority: §D.2 Table 3 pp.28-30 + §H.8 p145 FD&R precedence. \
                     post-closure dissem_us = {:?}, classification = {:?}, rel_to = {:?}",
                    closed.0.dissem_us,
                    closed.0.classification,
                    closed.0.rel_to,
                );
            }
        }
    }

    /// `MarkingClassification::Conflict { us, foreign }` makes
    /// `attrs.us_classification()` return `Some(us)`, so
    /// `TOK_US_COLLATERAL_CLASSIFIED` fires when the US side is
    /// collateral classified. Net effect:
    /// `CLOSURE_RELIDO_US_CLASS` fires on Conflict markings whose
    /// US side is at or above Restricted.
    ///
    /// This is documented behavior — Conflict is a parser-flagged
    /// structural error condition, and the implicit RELIDO addition
    /// is downstream of (orthogonal to) the Conflict diagnostic. A
    /// future tightening of `TOK_US_COLLATERAL_CLASSIFIED` semantics
    /// that excludes Conflict would flip this assertion and require
    /// updating the doc-comment on `RELIDO_US_CLASS_SUPPRESSORS`.
    #[test]
    fn us_class_conflict_variant_pin() {
        let scheme = CapcoScheme::new();
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Conflict {
            us: Classification::Secret,
            foreign: Box::new(ForeignClassification::Nato(NatoClassification::NatoSecret)),
        });
        let m = CapcoMarking::new(a);
        let closed = scheme.closure(m);
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Relido),
            "Conflict variant currently allows `CLOSURE_RELIDO_US_CLASS` to fire \
             (us_classification() returns Some(Secret) for the US side, and \
             `TOK_US_COLLATERAL_CLASSIFIED` is therefore satisfied). dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    /// Idempotence: closing a US-classified marking twice produces
    /// the same result. Constitution Principle II algebraic
    /// contract.
    #[test]
    fn us_class_idempotent() {
        let scheme = CapcoScheme::new();
        let m = us_classified(Classification::Secret);
        let once = scheme.closure(m);
        let twice = scheme.closure(once.clone());
        assert_eq!(
            once, twice,
            "CLOSURE_RELIDO_US_CLASS must be idempotent; once = {once:?}, twice = {twice:?}"
        );
    }

    // -----------------------------------------------------------------
    // Composition end-state pins — US_CLASS × per-marking SCI rows
    // -----------------------------------------------------------------

    /// `(US TS, HCS-O)` final state must contain NOFORN and ORCON,
    /// and must NOT contain RELIDO. HCS-O's per-marking closure row
    /// adds both NOFORN + ORCON directly (§H.4 p64); the
    /// `TOK_HCS_O` suppressor on `CLOSURE_RELIDO_US_CLASS` prevents
    /// premature RELIDO injection, and NOFORN supersedes RELIDO via
    /// the §H.8 p145 overlay regardless. Companion to
    /// `us_class_monotone_under_added_si_g` (which covers SI-G
    /// where ORCON is added but NOFORN comes via the
    /// CAVEATED→NOFORN→supersession chain).
    #[test]
    fn us_class_with_hcs_o_yields_noforn_orcon_no_relido() {
        let scheme = CapcoScheme::new();
        let suppressor = TokenRef::Token(TOK_HCS_O);
        let closed = scheme.closure(us_class_suppression_fixture(&suppressor));
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Nf),
            "(US TS, HCS-O) closure must contain NOFORN (§H.4 p64 per-marking row); \
             dissem_us = {:?}",
            closed.0.dissem_us
        );
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Oc),
            "(US TS, HCS-O) closure must contain ORCON (§H.4 p64 per-marking row); \
             dissem_us = {:?}",
            closed.0.dissem_us
        );
        assert!(
            !closed.0.dissem_us.contains(&DissemControl::Relido),
            "(US TS, HCS-O) closure must NOT contain RELIDO — NOFORN supersedes via \
             §H.8 p145 overlay; dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    /// `(US TS, TK-BLFH)` final state must contain NOFORN and must
    /// NOT contain RELIDO. TK-BLFH's per-marking closure row adds
    /// NOFORN directly (§H.4 p87); the `TOK_TK_BLFH` suppressor on
    /// `CLOSURE_RELIDO_US_CLASS` prevents premature RELIDO injection,
    /// and NOFORN supersedes RELIDO via the §H.8 p145 overlay
    /// regardless.
    #[test]
    fn us_class_with_tk_blfh_yields_noforn_no_relido() {
        let scheme = CapcoScheme::new();
        let suppressor = TokenRef::Token(TOK_TK_BLFH);
        let closed = scheme.closure(us_class_suppression_fixture(&suppressor));
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Nf),
            "(US TS, TK-BLFH) closure must contain NOFORN (§H.4 p87 per-marking row); \
             dissem_us = {:?}",
            closed.0.dissem_us
        );
        assert!(
            !closed.0.dissem_us.contains(&DissemControl::Relido),
            "(US TS, TK-BLFH) closure must NOT contain RELIDO — NOFORN supersedes via \
             §H.8 p145 overlay; dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    // -----------------------------------------------------------------
    // Composition end-state pins — US_CLASS × Trio 1 CAVEATED triggers
    // -----------------------------------------------------------------

    /// `(US S, ORCON)` final state: CAVEATED (Trio 1) injects NOFORN
    /// on ORCON, and NOFORN supersedes RELIDO via the §H.8 p145
    /// overlay. Pins the §B.3 Table 2 p21 "classified + caveated"
    /// semantic via closure composition rather than via an
    /// anti-monotone suppressor (per the redesign documented in
    /// `RELIDO_US_CLASS_SUPPRESSORS`'s doc-comment).
    #[test]
    fn us_class_with_orcon_yields_noforn_no_relido_via_caveated() {
        let scheme = CapcoScheme::new();
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Us(Classification::Secret));
        a.dissem_us = Box::new([DissemControl::Oc]);
        let closed = scheme.closure(CapcoMarking::new(a));
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Nf),
            "(US S, ORCON) closure must contain NOFORN via CAVEATED→NOFORN \
             (§B.3 Table 2 p21 + §B.3 p20 Note); dissem_us = {:?}",
            closed.0.dissem_us
        );
        assert!(
            !closed.0.dissem_us.contains(&DissemControl::Relido),
            "(US S, ORCON) closure must NOT contain RELIDO — NOFORN supersedes \
             via §H.8 p145 overlay; dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    /// `(US S, RD)` final state: RD triggers CAVEATED (Trio 1) which
    /// adds NOFORN; NOFORN supersedes RELIDO via §H.8 p145. Pins the
    /// AEA-side Trio 1 composition that converges with US_CLASS to
    /// the correct §B.3 Table 2 p21 result.
    #[test]
    fn us_class_with_rd_yields_noforn_no_relido_via_caveated_aea() {
        use marque_ism::{AeaMarking, RdBlock};
        let scheme = CapcoScheme::new();
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Us(Classification::Secret));
        a.aea_markings = Box::new([AeaMarking::Rd(RdBlock::default())]);
        let closed = scheme.closure(CapcoMarking::new(a));
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Nf),
            "(US S, RD) closure must contain NOFORN via CAVEATED→NOFORN \
             (§B.3 p20 Note: AEA is in CAVEATED's trigger list); dissem_us = {:?}",
            closed.0.dissem_us
        );
        assert!(
            !closed.0.dissem_us.contains(&DissemControl::Relido),
            "(US S, RD) closure must NOT contain RELIDO — NOFORN supersedes \
             via §H.8 p145 overlay; dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    /// `(US S, SAR)` final state: SAR triggers CAVEATED (Trio 1) via
    /// the `AnyInCategory(CAT_SAR)` arm, which adds NOFORN; NOFORN
    /// supersedes RELIDO via §H.8 p145. Pins the SAR-side Trio 1
    /// composition that converges with US_CLASS to the correct
    /// §B.3 Table 2 p21 result.
    #[test]
    fn us_class_with_sar_yields_noforn_no_relido_via_caveated_sar() {
        use marque_ism::{SarIndicator, SarMarking, SarProgram};
        use smol_str::SmolStr;
        let scheme = CapcoScheme::new();
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Us(Classification::Secret));
        let program = SarProgram::new(SmolStr::new("BP"), Box::new([]));
        a.sar_markings = Some(SarMarking::new(SarIndicator::Abbrev, Box::new([program])));
        let closed = scheme.closure(CapcoMarking::new(a));
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Nf),
            "(US S, SAR-BP) closure must contain NOFORN via CAVEATED→NOFORN \
             (§B.3 p20 Note: SAR is in CAVEATED's trigger list); dissem_us = {:?}",
            closed.0.dissem_us
        );
        assert!(
            !closed.0.dissem_us.contains(&DissemControl::Relido),
            "(US S, SAR-BP) closure must NOT contain RELIDO — NOFORN supersedes \
             via §H.8 p145 overlay; dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    // -----------------------------------------------------------------
    // Monotonicity sanity pins — additional cross-axis scenarios
    // -----------------------------------------------------------------

    /// Monotonicity at the U→R boundary in the classification ladder.
    ///
    /// `x = (U)`. `y = (R)`. In the classification lattice
    /// `U ⊑ R ⊑ C ⊑ S ⊑ TS`, so `x ⊑ y`. The Phase 3 carve-out
    /// applies to U at the trigger level (`TOK_US_COLLATERAL_CLASSIFIED`
    /// does not fire on `Us(Unclassified)`), so `Cl(x).dissem_us`
    /// does NOT contain RELIDO. At Restricted the trigger fires, so
    /// `Cl(y).dissem_us` contains RELIDO. The lattice ordering on
    /// the dissem axis is `{} ⊑ {Relido}`, so the closure preserves
    /// `x ⊑ y`. This pin closes a Copilot-HIGH-adjacent concern: the
    /// carve-out at U doesn't break monotonicity precisely because
    /// `{} ⊑ {Relido}` holds.
    #[test]
    fn us_class_monotone_at_u_to_r_carve_out_boundary() {
        let scheme = CapcoScheme::new();
        let cl_x = scheme.closure(us_classified(Classification::Unclassified));
        let cl_y = scheme.closure(us_classified(Classification::Restricted));
        assert!(
            !cl_x.0.dissem_us.contains(&DissemControl::Relido),
            "Cl(x = bare U) must NOT contain Relido per §H.8 p154 carve-out; \
             dissem_us = {:?}",
            cl_x.0.dissem_us
        );
        assert!(
            cl_y.0.dissem_us.contains(&DissemControl::Relido),
            "Cl(y = bare R) must contain Relido (Restricted is collateral); \
             dissem_us = {:?}",
            cl_y.0.dissem_us
        );
        // Monotonicity: {} ⊑ {Relido} holds in the SupersessionSet
        // dissem lattice, so Cl(x) ⊑ Cl(y) at the dissem axis.
    }

    /// Monotonicity: `(U, FOUO) ⊑ (S, FOUO)`.
    ///
    /// At U the US_CLASS trigger doesn't fire (carve-out), so
    /// `Cl(x).dissem_us` carries `{FOUO}` (no RELIDO). At S the
    /// US_CLASS trigger fires and FOUO is NOT a US_CLASS suppressor,
    /// so `Cl(y).dissem_us` carries `{FOUO, RELIDO, ...}`. The
    /// SupersessionSet dissem lattice ordering preserves the subset
    /// relation `{FOUO} ⊑ {FOUO, RELIDO}` so monotonicity holds.
    /// Pins the carve-out × cross-axis interaction Copilot flagged
    /// in the HIGH review.
    #[test]
    fn us_class_monotone_at_u_fouo_to_s_fouo() {
        let scheme = CapcoScheme::new();
        let mut a_x = CanonicalAttrs::default();
        a_x.classification = Some(MarkingClassification::Us(Classification::Unclassified));
        a_x.dissem_us = Box::new([DissemControl::Fouo]);
        let cl_x = scheme.closure(CapcoMarking::new(a_x));
        let mut a_y = CanonicalAttrs::default();
        a_y.classification = Some(MarkingClassification::Us(Classification::Secret));
        a_y.dissem_us = Box::new([DissemControl::Fouo]);
        let cl_y = scheme.closure(CapcoMarking::new(a_y));
        assert!(
            !cl_x.0.dissem_us.contains(&DissemControl::Relido),
            "Cl(x = U + FOUO) must NOT contain Relido per §H.8 p154 U-carve-out; \
             dissem_us = {:?}",
            cl_x.0.dissem_us
        );
        assert!(
            cl_y.0.dissem_us.contains(&DissemControl::Relido),
            "Cl(y = S + FOUO) must contain Relido (FOUO is not a US_CLASS \
             suppressor in the redesigned slice); dissem_us = {:?}",
            cl_y.0.dissem_us
        );
        // Both must contain FOUO (closure does not strip FOUO at
        // this level).
        assert!(
            cl_x.0.dissem_us.contains(&DissemControl::Fouo),
            "Cl(x = U + FOUO) must still contain FOUO; dissem_us = {:?}",
            cl_x.0.dissem_us
        );
        assert!(
            cl_y.0.dissem_us.contains(&DissemControl::Fouo),
            "Cl(y = S + FOUO) must still contain FOUO; dissem_us = {:?}",
            cl_y.0.dissem_us
        );
    }
}

// ---------------------------------------------------------------------------
// Issue #525 — FISA / RAWFISA / PROPIN as CAVEATED triggers
// ---------------------------------------------------------------------------

/// Issue #525 closure-row pins.
///
/// CAPCO §B.3 p20 Note: a portion carrying any IC dissem control is
/// caveated. PROPIN (§H.8 p148) and FISA (§H.8 p161) are IC dissem
/// controls; RAWFISA is the post-CAPCO-2016 unminimized variant of
/// FISA registered in ODNI `CVEnumISMDissem.xml` (no CAPCO-2016
/// prose section — see the RAWFISA paragraph below the CAVEATED
/// authority table). All three are structurally identical to
/// ORCON / RSEN / IMCON / DSEN already in
/// `CLOSURE_NOFORN_CAVEATED.triggers`. These pins assert each fires
/// the CAVEATED row's NOFORN cone in isolation AND that the
/// concurrent `CLOSURE_RELIDO_US_CLASS` row is suppressed (NOFORN
/// dominates RELIDO via the §H.8 p145 supersession overlay).
///
/// The pre-existing `every_fdr_dominator_suppresses_caveated_noforn_injection`
/// pin covers the suppressor side of the CAVEATED row; these three
/// pins are the trigger-side companions for the new entries.
///
/// Banner-roll-up of `(U//FISA)` as "considered RELIDO for purposes
/// of developing the overall banner line FD&R marking" (CAPCO-2016
/// `crates/capco/docs/CAPCO-2016.md` notional example, verified
/// 2026-05-18) is a PageContext-layer artifact — it is how a single
/// unclassified-FISA portion contributes to a page's FD&R
/// determination when classified portions are also present. It is
/// NOT a per-portion closure semantic and is out of scope for #525.
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod issue_525_caveated_dissem_pin {
    use super::*;
    use marque_ism::{CanonicalAttrs, Classification, DissemControl, MarkingClassification};
    use marque_scheme::MarkingScheme;

    /// Construct a `(S, dissem)` marking — Secret base with one IC
    /// dissem control on the dissem_us axis. Secret is chosen
    /// arbitrarily as a classified collateral level; the CAVEATED
    /// trigger is class-agnostic (the row has no class floor), so any
    /// classification level not in `FDR_DOMINATORS` would work.
    fn secret_with_dissem(d: DissemControl) -> CapcoMarking {
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Us(Classification::Secret));
        a.dissem_us = Box::new([d]);
        CapcoMarking::new(a)
    }

    /// PROPIN is an IC dissem control per §H.8 p148. Caveated per
    /// §B.3 p20 Note → §B.3 Table 2 p21 default → NOFORN absent
    /// FD&R. `TOK_PROPIN` is a `CLOSURE_NOFORN_CAVEATED` trigger.
    /// The `!Relido` postcondition pins the behavioral change
    /// versus the pre-issue-525 state where CAVEATED was silent on
    /// PROPIN and `CLOSURE_RELIDO_US_CLASS` injected `Relido`
    /// instead.
    #[test]
    fn caveated_fires_on_propin() {
        let scheme = CapcoScheme::new();
        let closed = scheme.closure(secret_with_dissem(DissemControl::Pr));
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Nf),
            "PROPIN must trigger CLOSURE_NOFORN_CAVEATED → NOFORN injection. \
             Authority: §H.8 p148 (PROPIN as IC dissem control) + §B.3 p20 \
             Note + §B.3 Table 2 p21. dissem_us = {:?}",
            closed.0.dissem_us
        );
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Pr),
            "PROPIN itself must be retained — CAVEATED's cone is `{{NOFORN}}` \
             only. dissem_us = {:?}",
            closed.0.dissem_us
        );
        assert!(
            !closed.0.dissem_us.contains(&DissemControl::Relido),
            "RELIDO must NOT appear: NOFORN supersedes RELIDO via §H.8 p145 \
             overlay. Pre-issue-525 behavior (CAVEATED silent on PROPIN, \
             CLOSURE_RELIDO_US_CLASS injects Relido) is now retired. \
             dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    /// FISA is an IC dissem control per §H.8 p161. Caveated → NOFORN
    /// absent FD&R. `TOK_FISA` is a `CLOSURE_NOFORN_CAVEATED` trigger.
    #[test]
    fn caveated_fires_on_fisa() {
        let scheme = CapcoScheme::new();
        let closed = scheme.closure(secret_with_dissem(DissemControl::Fisa));
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Nf),
            "FISA must trigger CLOSURE_NOFORN_CAVEATED → NOFORN injection. \
             Authority: §H.8 p161 (FISA as IC dissem control) + §B.3 p20 \
             Note + §B.3 Table 2 p21. dissem_us = {:?}",
            closed.0.dissem_us
        );
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Fisa),
            "FISA itself must be retained — CAVEATED's cone is `{{NOFORN}}` \
             only. dissem_us = {:?}",
            closed.0.dissem_us
        );
        assert!(
            !closed.0.dissem_us.contains(&DissemControl::Relido),
            "RELIDO must NOT appear: NOFORN supersedes RELIDO via §H.8 p145 \
             overlay. dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    /// RAWFISA is the post-CAPCO-2016 unminimized variant of FISA,
    /// registered in ODNI `CVEnumISMDissem.xml`. Same caveated →
    /// NOFORN semantic by §B.3 p20 Note algebraic basis (IC dissem
    /// control). `TOK_RAWFISA` is a `CLOSURE_NOFORN_CAVEATED`
    /// trigger.
    #[test]
    fn caveated_fires_on_rawfisa() {
        let scheme = CapcoScheme::new();
        let closed = scheme.closure(secret_with_dissem(DissemControl::Rawfisa));
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Nf),
            "RAWFISA must trigger CLOSURE_NOFORN_CAVEATED → NOFORN injection. \
             Authority: ODNI `CVEnumISMDissem.xml` (post-CAPCO-2016) + §B.3 \
             p20 Note + §B.3 Table 2 p21. dissem_us = {:?}",
            closed.0.dissem_us
        );
        assert!(
            closed.0.dissem_us.contains(&DissemControl::Rawfisa),
            "RAWFISA itself must be retained — CAVEATED's cone is `{{NOFORN}}` \
             only. dissem_us = {:?}",
            closed.0.dissem_us
        );
        assert!(
            !closed.0.dissem_us.contains(&DissemControl::Relido),
            "RELIDO must NOT appear: NOFORN supersedes RELIDO via §H.8 p145 \
             overlay. dissem_us = {:?}",
            closed.0.dissem_us
        );
    }

    /// Source-of-truth pin: each of the three new triggers fires the
    /// CAVEATED bitmask row. Post-PR-D this asserts the
    /// `CLOSURE_TABLE` Row 0 trigger mask (the bitmask form of the
    /// retired `CLOSURE_NOFORN_CAVEATED.triggers` slice) contains the
    /// `fact_bit::PROPIN` / `fact_bit::FISA` / `fact_bit::RAWFISA`
    /// atom bits per issue #525. The behavioral pins in the
    /// `caveated_fires_on_*` tests above already prove the end-to-end
    /// closure path; this one closes the source-of-truth drift channel
    /// at the catalog level so a row edit dropping a bit is caught
    /// even if the behavioral pin happens to be observationally
    /// satisfied by another row.
    #[test]
    fn each_new_trigger_appears_in_caveated_bitmask_row() {
        use crate::fact_bitmask::fact_bit;
        use crate::scheme::closure_table::CLOSURE_TABLE;
        let row0_trigger_mask = CLOSURE_TABLE[0].trigger_mask;
        let new_trigger_bits = [
            ("PROPIN", fact_bit::PROPIN),
            ("FISA", fact_bit::FISA),
            ("RAWFISA", fact_bit::RAWFISA),
        ];
        for (name, bit) in new_trigger_bits {
            assert!(
                (row0_trigger_mask & (1u128 << bit)) != 0,
                "fact_bit::{name} (bit {bit}) missing from \
                 CLOSURE_TABLE[0].trigger_mask. Issue #525 requires \
                 PROPIN/FISA/RAWFISA in the caveated trigger list per \
                 §B.3 p20 Note (IC dissem controls are caveated). \
                 Authority: §H.8 p148 (PROPIN), §H.8 p161 (FISA + RAWFISA), \
                 §B.3 Table 2 p21 (caveated-default obligation)."
            );
        }
    }

    /// `FDR_DOMINATORS` continues to suppress CAVEATED on the new
    /// triggers — pinning the algebraic identity that PROPIN/FISA/
    /// RAWFISA are structurally identical to the other IC dissem
    /// caveats in the trigger list (closing under the same FD&R
    /// suppressor set).
    ///
    /// **Observability caveat.** NOFORN is the suppressor AND the
    /// CAVEATED row's cone is `{NOFORN}`. When NOFORN is pre-seeded
    /// in `dissem_us`, a broken suppressor injecting `Nf` again would
    /// dedup against the existing fact — `dissem_us.len()` stays
    /// constant either way, making this assertion shape vacuously
    /// true for the NOFORN-suppresses-NOFORN-cone case. The pin is
    /// kept anyway because (a) it documents the algebraic identity
    /// at the new-trigger × suppressor intersection, and (b) the
    /// retention assertion (`contains(Nf)`) would still flag a
    /// regression that strips or transforms the seed `Nf`. The
    /// existing `every_fdr_dominator_suppresses_caveated_noforn_injection`
    /// pin has the same property and the same rationale (see its
    /// `TOK_NOFORN` arm comment).
    #[test]
    fn fdr_dominator_noforn_suppresses_new_triggers() {
        let scheme = CapcoScheme::new();
        for trigger in [
            DissemControl::Pr,
            DissemControl::Fisa,
            DissemControl::Rawfisa,
        ] {
            let mut a = CanonicalAttrs::default();
            a.classification = Some(MarkingClassification::Us(Classification::Secret));
            a.dissem_us = Box::new([trigger, DissemControl::Nf]);
            let before_len = a.dissem_us.len();
            let closed = scheme.closure(CapcoMarking::new(a));
            assert_eq!(
                closed.0.dissem_us.len(),
                before_len,
                "FD&R dominator NOFORN must suppress CAVEATED on trigger \
                 {trigger:?}: closure should add no facts. dissem_us = {:?}",
                closed.0.dissem_us
            );
            assert!(
                closed.0.dissem_us.contains(&DissemControl::Nf),
                "FD&R dominator NOFORN must be retained on trigger \
                 {trigger:?}: closure must not strip the seed Nf fact. \
                 dissem_us = {:?}",
                closed.0.dissem_us
            );
        }
    }
}
