// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CAPCO closure-rule catalog — `FDR_DOMINATORS` + `CLOSURE_NOFORN_CAVEATED`
//! + `CLOSURE_REL_TO_USA_NATO` + the aggregating `CAPCO_CLOSURE_RULES` static.
//!
//! Implements the §4.7 implicit-fact propagation catalog from
//! `docs/plans/2026-05-01-lattice-design.md` §3 (e) and
//! `marque-applied.md` §4.7. The `MarkingScheme::closure_rules()` impl on
//! `CapcoScheme` exposes it as the public catalog surface per
//! `decisions.md` D18.
//!
//! Trio 1 was originally split into seven token-grouped rows (SAR / AEA-RD
//! / UCNI / FGI / ORCON / RSEN-IMCON-DSEN / non-IC-controls) for §-citation
//! locality. Per D18 rationale 2 ("triggers reduce to n-ary OR over
//! `TokenRef`s") those rows are algebraically identical — same suppressor
//! (`FDR_DOMINATORS`), same cone (`{NOFORN}`), same default severity. The
//! Trio 1 catalog is now a single `CLOSURE_NOFORN_CAVEATED` row whose
//! `label` cites the universal §B.3 algebraic basis (ICD 403 → caveated
//! default); per-token Section H subsection authorities live in the row
//! doc-comment's authority table.

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
// `pub(crate)` for symmetry with `FDR_DOMINATORS` and so future
// runtime-pin modules can walk the slice as a source-of-truth.
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

// --- The implicit-default trio (FD&R-suppressed) ---

/// Trio 1: every trigger marking enumerated in the `triggers` list
/// below implies NOFORN unless an explicit FD&R decision (NOFORN,
/// REL TO, RELIDO, DISPLAY ONLY, EYES) is present.
///
/// **Universal IC principle.** Any AEA marking, SAP marking, or
/// dissemination control marking renders information structurally
/// **caveated** per CAPCO-2016 §B.3 p20 Note: "Caveated means bears no
/// FD&R markings, but has one or more AEA markings, SAP markings,
/// and/or dissemination control marking(s)." The §B.3 Table 2 p21 row
/// "Classified, caveated, on/after 28 Jun 2010 → NOFORN" is the
/// algebraic anchor for the classified case; for triggers that exist
/// at UNCLASSIFIED (UCNI/DCNI by §H.6 marking template, non-IC dissem
/// markings under §H.9 that may be applied at any classification
/// level), the per-marking template authority carries the NOFORN
/// implication independently of §B.3 Table 2 p21. The principle is
/// rooted in ICD 403 (Foreign Disclosure and Release): the IC cannot
/// presume releasability or RELIDO-suitability of information governed
/// by policy regimes outside IC marking authority, so implicit NOFORN
/// is the conservative default absent an explicit FD&R decision.
///
/// **The row is intentionally class-agnostic** — it has no
/// classification gate. Every trigger marking carries an implicit
/// NOFORN release posture under its own per-marking authority,
/// regardless of whether the host information is classified or
/// unclassified. This is correct for UCNI (constrained to UNCLASSIFIED
/// per §H.6 pp116-119) and for non-IC dissem markings (which may apply
/// at any classification level per §H.9 marking templates). The
/// per-trigger authority table below names the load-bearing
/// per-marking citation for each arm.
///
/// **Trigger-set scope.** The `triggers` list enumerates the caveated
/// markings *currently in the catalog*. The universal §B.3 p20 Note
/// definition is broader — it covers every AEA / SAP / dissem marking
/// — but several caveated markings are intentionally out of scope of
/// this row:
/// - **ATOMAL** (NATO AEA) — routed through the AEA axis with its own
///   per-marking handling; see `marque-ism` AEA layer.
/// - **FISA / RAWFISA / PROPIN** — class-bivalent (different semantics
///   at classified vs unclassified) so they cannot be unconditional
///   triggers of the CAVEATED row; tracked at issue #526.
/// - Per-compartment SCI implications (HCS-O/P, SI-G, TK-BLFH/KAND/IDIT)
///   require per-compartment sentinels that do not exist yet; tracked
///   at issue #524.
///
/// New markings registered upstream MUST evaluate against this rule's
/// universal basis (§B.3 p20 Note + §B.3 Table 2 p21) and be added to
/// the trigger list unless one of the structural exceptions above
/// applies.
///
/// This row is the algebraic union of seven previously separate Trio 1
/// rows (SAR / AEA-RD / UCNI / FGI / ORCON / RSEN-IMCON-DSEN /
/// non-IC-controls). All shared the same suppressor (`FDR_DOMINATORS`),
/// the same cone (`{NOFORN}`), and the same default severity
/// (`Severity::Info`); per D18 rationale 2 ("triggers reduce to n-ary
/// OR over `TokenRef`s") the rows are algebraically identical. The
/// universal §B.3 citation in `label` reflects the rule's actual
/// algebraic basis; per-token Section H subsection authorities live in the per-trigger
/// authority table below (per-token traceability without per-row
/// duplication of identical operator structure).
///
/// **Per-trigger authority (the `triggers` list, in order):**
///
/// | Trigger                            | Marking                  | Authority           |
/// |------------------------------------|--------------------------|---------------------|
/// | `AnyInCategory(CAT_SAR)`           | any SAR program          | §H.5 pp99-102       |
/// | `Token(TOK_RD)`                    | RESTRICTED DATA          | §H.6 p104           |
/// | `Token(TOK_FRD)`                   | FORMERLY RESTRICTED DATA | §H.6 p111           |
/// | `Token(TOK_TFNI)`                  | TFNI                     | §H.6 p120           |
/// | `Token(TOK_UCNI)`                  | DOE UCNI                 | §H.6 p118           |
/// | `Token(TOK_DCNI)`                  | DOD UCNI                 | §H.6 p116 (#407)    |
/// | `Token(TOK_FGI_MARKER)`            | foreign-classified portion (`//GBR S`, etc.) | §H.7 p122 |
/// | `AnyInCategory(CAT_FGI_MARKER)`    | explicit `FGI` token     | §H.7 p123           |
/// | `Token(TOK_ORCON)`                 | ORCON                    | §H.8 p136           |
/// | `Token(TOK_ORCON_USGOV)`           | ORCON-USGOV              | §H.8 p139           |
/// | `Token(TOK_RSEN)`                  | RSEN                     | §H.8 p132           |
/// | `Token(TOK_IMCON)`                 | IMCON                    | §H.8 p142           |
/// | `Token(TOK_DSEN)`                  | DEA SENSITIVE            | §H.8 p159           |
/// | `Token(TOK_LIMDIS)`                | LIMDIS                   | §H.9 p170           |
/// | `Token(TOK_LES)`                   | LES                      | §H.9 p181           |
/// | `Token(TOK_NNPI)`                  | NNPI                     | ODNI `CVEnumISMNonIC.xml` |
/// | `Token(TOK_SBU)`                   | SBU                      | §H.9 p176           |
/// | `Token(TOK_SSI)`                   | SSI                      | §H.9 p189           |
///
/// Triggers are evaluated as a logical OR — any single trigger firing
/// fires the row. Two notes on the trigger list shape:
/// - **UCNI pair (`TOK_UCNI` + `TOK_DCNI`)** — both sentinels are
///   required to cover DOE and DOD UCNI as disjoint surfaces.
///   `TOK_UCNI` resolves only to `AeaMarking::DoeUcni`; the DOD variant
///   resolves through the distinct `TOK_DCNI` sentinel (issue #407,
///   `predicates::satisfies::aea_marking_to_token`).
/// - **FGI pair (`TOK_FGI_MARKER` + `AnyInCategory(CAT_FGI_MARKER)`)**
///   — kept as both forms for catalog symmetry with the rest of the
///   `AnyInCategory` triggers, but both `TokenRef`s currently resolve
///   to the same composite predicate
///   `attrs.fgi_marker.is_some() || matches!(&attrs.classification, Some(MarkingClassification::Fgi(_)))`
///   (see `crates/capco/src/scheme/predicates/satisfies.rs` —
///   `TOK_FGI_MARKER` arm and `CAT_FGI_MARKER` arm under
///   `category_has_any`). The pair is therefore *redundant*, not
///   complementary — the closure operator's idempotence makes the
///   double-firing harmless. A follow-up could prune one form once
///   `satisfies_attrs` and `category_has_any` semantics are pinned
///   against accidental divergence.
///
/// **NNPI** is registered in ODNI `CVEnumISMNonIC.xml` but does not
/// appear in CAPCO-2016 §H.9; its governing authority (10 USC 7314 /
/// 50 USC 2511 — Naval Nuclear Propulsion Program) lives outside IC
/// marking policy, and the universal caveated-default principle applies.
///
/// **LES-NF / SBU-NF** are intentionally absent from the trigger list,
/// but the rationale is *not* "the closure operator sees NOFORN first."
/// The page-projection pipeline is
/// `join_via_lattice → closure → PageRewrites` per the body of
/// `CapcoScheme::project_attrs_pipeline_with_context` (the shared
/// pipeline helper that `MarkingScheme::project`, the engine fast-path
/// entries, and direct `scheme.project(Scope::Page, ...)` callers all
/// delegate through — see `crates/capco/src/scheme/marking_scheme_impl.rs`).
/// When closure runs, the LES-NF / SBU-NF PageRewrites have not yet
/// added NOFORN. Closure is permitted to over-fire on bare-LES-NF /
/// bare-SBU-NF — the cone fact it would add (`{NOFORN}`) is
/// byte-identical to what the downstream PageRewrite would add anyway,
/// so the over-fire is mathematically harmless. See the maintenance
/// note on `FDR_DOMINATORS` for the full algebraic justification.
///
/// **Row name stability.** `ClosureRule::name` is the documented
/// public key for `[closure_rules]` severity overrides and future audit
/// row-name emission. This PR (#522) consolidates seven previously
/// public row names (`capco/noforn-if-sar`, `…-aea`, `…-ucni`, `…-fgi`,
/// `…-orcon`, `…-rsen-imcon-dsen`, `…-non-ic-controls`) into the single
/// new `capco/noforn-if-caveated` key. Marque is pre-users per project
/// policy (no deprecation phasing, no alias maps), so the previous keys
/// are not retained as aliases. A config keyed to a retired name will
/// silently become a no-op; the broader gap that the config layer does
/// not validate unknown closure-row keys is independent of this
/// renaming and applies to every closure-rule rename.
const CLOSURE_NOFORN_CAVEATED: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/noforn-if-caveated",
    label: "CAPCO-2016 §B.3 Table 2 p21 (rooted in ICD 403)",
    triggers: &[
        TokenRef::AnyInCategory(CAT_SAR),
        TokenRef::Token(TOK_RD),
        TokenRef::Token(TOK_FRD),
        TokenRef::Token(TOK_TFNI),
        TokenRef::Token(TOK_UCNI),
        TokenRef::Token(TOK_DCNI),
        TokenRef::Token(TOK_FGI_MARKER),
        TokenRef::AnyInCategory(CAT_FGI_MARKER),
        TokenRef::Token(TOK_ORCON),
        TokenRef::Token(TOK_ORCON_USGOV),
        TokenRef::Token(TOK_RSEN),
        TokenRef::Token(TOK_IMCON),
        TokenRef::Token(TOK_DSEN),
        TokenRef::Token(TOK_LIMDIS),
        TokenRef::Token(TOK_LES),
        TokenRef::Token(TOK_NNPI),
        TokenRef::Token(TOK_SBU),
        TokenRef::Token(TOK_SSI),
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

// ---------------------------------------------------------------------------
// Per-marking unconditional implications (Issue #524 Phase 2)
// ---------------------------------------------------------------------------
//
// Per `marque-applied.md` §4.7.5 "Per-marking unconditional
// implications": rules that fire regardless of FD&R state. The
// `suppressors` field is `&[]` for every row — these implications
// are an unconditional consequence of the trigger marking's
// per-marking authority (§H.4 marking templates), not a default
// override-able by FD&R presence. Idempotence preserves
// correctness when the cone fact is already present (closure
// re-adding NOFORN to a marking that already carries NOFORN is a
// no-op).
//
// Per-marking authority anchored in CAPCO-2016 §H.4 marking
// templates with the load-bearing Example Banner Line / Notional
// Example Page citations. Each row's doc-comment names the page
// and the example whose form establishes the per-marking
// implication.

/// `HCS-O` implies `NOFORN` and `ORCON`.
///
/// **Authority.** CAPCO-2016 §H.4 p64 (HCS-OPERATIONS marking
/// template):
///
/// - Example Banner Line: `SECRET//HCS-O//ORCON/NOFORN`
/// - Example Portion Mark: `(S//HCS-O//OC/NF)`
/// - Notional Example Page: `SECRET//HCS-O//ORCON/NOFORN` —
///   "contains HCS-O information that is originator controlled,
///   and not releasable to foreign nationals."
///
/// The Example Banner Line is prescriptive form: HCS-O is conveyed
/// alongside ORCON/NOFORN in the dissem-control band. Marque
/// automates the re-marking the manual permits doing by hand (per
/// project memory `remark-on-derivative-use-is-marque-autofix`).
const CLOSURE_HCS_O_IMPLIES_NF_OC: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/hcs-o-implies-noforn-orcon",
    label: "CAPCO-2016 §H.4 p64",
    triggers: &[TokenRef::Token(TOK_HCS_O)],
    suppressors: &[],
    cone: &[TokenRef::Token(TOK_NOFORN), TokenRef::Token(TOK_ORCON)],
    cone_derived: None,
    default_severity: Severity::Info,
};

/// `HCS-P` with at least one sub-compartment implies `NOFORN` and
/// `ORCON`.
///
/// **Authority.** CAPCO-2016 §H.4 p68 (HCS-PRODUCT
/// [SUB-COMPARTMENT] marking template):
///
/// - Example Banner Line: `TOP SECRET//HCS-P JJJ//ORCON/NOFORN`
/// - Example Portion Mark: `(TS//HCS-P JJJ//OC/NF)`
/// - Notional Example Page: `TOP SECRET//HCS-P EFG//ORCON/NOFORN`
///   — "contains HCS-PRODUCT EFG information, is originator
///   controlled, and not releasable to foreign nationals."
///
/// The sub-compartmented form's per-marking semantic differs from
/// bare HCS-P at §H.4 p66 (`SECRET//HCS-P//NOFORN` — NOFORN only,
/// no ORCON). The grammar-shape sentinel `TOK_HCS_P_SUB` discriminates
/// the two cases — see its doc-comment in
/// `crates/capco/src/scheme/mod.rs`.
const CLOSURE_HCS_P_SUB_IMPLIES_NF_OC: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/hcs-p-sub-implies-noforn-orcon",
    label: "CAPCO-2016 §H.4 p68",
    triggers: &[TokenRef::Token(TOK_HCS_P_SUB)],
    suppressors: &[],
    cone: &[TokenRef::Token(TOK_NOFORN), TokenRef::Token(TOK_ORCON)],
    cone_derived: None,
    default_severity: Severity::Info,
};

/// `SI-G` implies `ORCON`.
///
/// **Authority.** CAPCO-2016 §H.4 p80 (SI-GAMMA marking template):
///
/// - Example Banner Line: `TOP SECRET//SI-G//ORCON`
/// - Example Portion Mark: `(TS//SI-G//OC)`
/// - Notional Example Page: `TOP SECRET//SI-G//ORCON/NOFORN` —
///   "contains SI-GAMMA information, is originator controlled,
///   and not releasable to foreign nationals."
///
/// **NOFORN is NOT in the cone.** The Example Banner Line at
/// §H.4 p80 is prescriptive ORCON only; the Notional Example Page
/// adds NOFORN as a use-case-specific FD&R decision, not a
/// per-marking requirement. Per `marque-applied.md` §4.7.5: "If
/// `SI-G`, then `ORCON` must be present → closure fires `ORCON`."
/// SI-G's class floor (TS) is a `Constraint::Requires` concern per
/// §3.4.6, not a closure addition (§H.4 p80 Example Banner Line
/// starts at TOP SECRET).
///
/// **Trio 2 RELIDO suppression (stability optimization).** SI-G's
/// per-marking cone is `{ORCON}` only; NOFORN is not added in
/// iteration 1. Without `TOK_SI_G` in `FDR_OR_RELIDO_INCOMPAT`,
/// `CLOSURE_RELIDO_SCI` would fire in iteration 1 (adding RELIDO),
/// which would then be stripped in iteration 2 when ORCON triggers
/// `CLOSURE_NOFORN_CAVEATED` → NOFORN → `with_noforn_injected` (the
/// §H.8 p145 supersession overlay that strips dominated dissem
/// controls). The fixpoint result is the same either way; including
/// `TOK_SI_G` directly avoids the transient incorrect intermediate
/// state and keeps the in-pass invariant "Trio 2 doesn't fire on
/// SI-G" stable.
const CLOSURE_SI_G_IMPLIES_OC: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/si-g-implies-orcon",
    label: "CAPCO-2016 §H.4 p80",
    triggers: &[TokenRef::Token(TOK_SI_G)],
    suppressors: &[],
    cone: &[TokenRef::Token(TOK_ORCON)],
    cone_derived: None,
    default_severity: Severity::Info,
};

/// `TK-BLFH` implies `NOFORN`.
///
/// **Authority.** CAPCO-2016 §H.4 p87 (TK-BLUEFISH marking
/// template):
///
/// - Example Banner Line: `TOP SECRET//TK-BLFH//NOFORN`
/// - Example Portion Mark: `(TS//TK-BLFH//NF)`
/// - Notional Example Page: `TOP SECRET//TK-BLFH//NOFORN` —
///   "contains TALENT KEYHOLE-BLUEFISH information, and is not
///   releasable to foreign nationals."
///
/// TK-BLFH's class floor (TS) is a `Constraint::Requires` concern
/// per §3.4.6, not a closure addition (§H.4 p87 Example Banner Line
/// starts at TOP SECRET).
const CLOSURE_TK_BLFH_IMPLIES_NF: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/tk-blfh-implies-noforn",
    label: "CAPCO-2016 §H.4 p87",
    triggers: &[TokenRef::Token(TOK_TK_BLFH)],
    suppressors: &[],
    cone: &[TokenRef::Token(TOK_NOFORN)],
    cone_derived: None,
    default_severity: Severity::Info,
};

/// `TK-IDIT` implies `NOFORN`.
///
/// **Authority.** CAPCO-2016 §H.4 p91 (TK-IDITAROD marking
/// template):
///
/// - Example Banner Line: `TOP SECRET//TK-IDIT//NOFORN`
/// - Example Portion Mark: `(TS//TK-IDIT //NF)`
/// - Notional Example Page: `TOP SECRET//TK-IDIT//NOFORN` —
///   "contains TALENT KEYHOLE-IDITAROD information, and is not
///   releasable to foreign nationals."
const CLOSURE_TK_IDIT_IMPLIES_NF: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/tk-idit-implies-noforn",
    label: "CAPCO-2016 §H.4 p91",
    triggers: &[TokenRef::Token(TOK_TK_IDIT)],
    suppressors: &[],
    cone: &[TokenRef::Token(TOK_NOFORN)],
    cone_derived: None,
    default_severity: Severity::Info,
};

/// `TK-KAND` implies `NOFORN`.
///
/// **Authority.** CAPCO-2016 §H.4 p95 (TK-KANDIK marking template).
/// The §H.4 p95 marking template mirrors §H.4 p91 (TK-IDIT) and
/// §H.4 p87 (TK-BLFH) in shape: Example Banner Line at TOP SECRET
/// with NOFORN, Example Portion Mark in parens, Notional Example
/// Page reiterating the not-releasable semantic. The structural
/// uniformity across the three TK sub-compartment families is
/// itself the authority that TK-KAND's per-marking implication
/// matches TK-BLFH and TK-IDIT.
const CLOSURE_TK_KAND_IMPLIES_NF: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/tk-kand-implies-noforn",
    label: "CAPCO-2016 §H.4 p95",
    triggers: &[TokenRef::Token(TOK_TK_KAND)],
    suppressors: &[],
    cone: &[TokenRef::Token(TOK_NOFORN)],
    cone_derived: None,
    default_severity: Severity::Info,
};

// ---------------------------------------------------------------------------
// Trio 2 — implicit RELIDO (FD&R + RELIDO-incompatible-suppressed)
// ---------------------------------------------------------------------------

/// `CLOSURE_RELIDO_SCI` — bare SCI control implies `RELIDO` unless
/// FD&R-marked or RELIDO-incompatible.
///
/// **Trigger semantic.** `AnyInCategory(CAT_SCI)` fires when any
/// SCI marking is present in the page-projection. The Phase 2
/// scope ships only this Trio 2 row; the additional Trio 2 rows
/// (`CLOSURE_RELIDO_US_CLASS` for US unclassified-or-collateral and
/// `CLOSURE_RELIDO_FOUO`) are deferred — `CLOSURE_RELIDO_US_CLASS`
/// requires a more sophisticated "no other dissem" trigger
/// composition than the bare-CAT_CLASSIFICATION proxy that
/// over-fired in the historical placeholder (see the predecessor
/// commit's deferral comment at this slice's old location), and
/// `CLOSURE_RELIDO_FOUO` follows once the US_CLASS encoding lands.
///
/// **Suppressor semantic.** `FDR_OR_RELIDO_INCOMPAT` covers two
/// disjoint cases:
///
/// 1. **FD&R-marked** — explicit FD&R decision present (NOFORN,
///    REL TO, RELIDO, DISPLAY ONLY, EYES). The implicit-RELIDO
///    default is superseded by the explicit decision per
///    `marque-applied.md` §4.7.3.
/// 2. **RELIDO-incompatible** — foreign-equity / origination
///    markings (FGI / JOINT / NATO) or per-marking NOFORN/ORCON-
///    implying SCI compartments (SI-G, HCS-O, HCS-P[sub],
///    TK-BLFH, TK-IDIT, TK-KAND). RELIDO is structurally
///    inapplicable to these markings per `marque-applied.md`
///    §4.7.5 Trio 2 exclusion list.
///
/// **Kleene-fixpoint composition with per-marking rows.** The five
/// per-marking unconditional NOFORN-cone rows (HCS-O, HCS-P[sub],
/// TK-BLFH, TK-IDIT, TK-KAND) precede this row in the catalog
/// order, so within a single closure iteration NOFORN is added to
/// `working` before this row evaluates — NOFORN ∈
/// `FDR_OR_RELIDO_INCOMPAT` then suppresses the RELIDO cone in
/// iteration 1. For SI-G (cone = `{ORCON}` only), the in-pass
/// NOFORN→suppression path does NOT cover iteration 1 because SI-G
/// doesn't add NOFORN immediately. Across multiple iterations,
/// SI-G's ORCON would trigger `CLOSURE_NOFORN_CAVEATED` (ORCON is
/// in its trigger list), adding NOFORN in iteration 2, which would
/// then strip a previously-injected RELIDO via `with_noforn_injected`
/// (the §H.8 p145 supersession overlay) in iteration 3 — the
/// fixpoint is correct without the direct guard. Including
/// `TOK_SI_G` in `FDR_OR_RELIDO_INCOMPAT` directly is a stability
/// optimization that avoids the transient intermediate state and
/// keeps the per-iteration invariant "Trio 2 doesn't fire on SI-G"
/// stable from iteration 1.
///
/// **Severity calibration.** `Severity::Info` matches the other
/// closure rows (Trio 1, Trio 3, per-marking). The text-layer
/// surface (which proposes the actual byte-level RELIDO insertion)
/// is the responsibility of a future rule, not this lattice-layer
/// row. Per D20 layer-separation principle.
const CLOSURE_RELIDO_SCI: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco/relido-if-sci-and-not-incompatible",
    label: "CAPCO-2016 §H.8 p154",
    triggers: &[TokenRef::AnyInCategory(CAT_SCI)],
    suppressors: FDR_OR_RELIDO_INCOMPAT,
    cone: &[TokenRef::Token(TOK_RELIDO)],
    cone_derived: None,
    default_severity: Severity::Info,
};

/// The full static CAPCO closure-rule catalog.
///
/// Rows are grouped by the three-trio framing from `marque-applied.md` §4.7.1:
///   1. Trio 1 — implicit NOFORN (FD&R-suppressed)
///   2. Per-marking unconditional implications (unsuppressed; Issue #524 Phase 2)
///   3. Trio 3 — implicit REL TO USA, NATO (FD&R-suppressed)
///   4. Trio 2 — implicit RELIDO (FD&R + RELIDO-incompatible-suppressed)
///
/// **Catalog order is load-bearing.** The closure operator
/// (`CapcoScheme::closure`) walks this catalog in order within each
/// Kleene iteration, mutating the working marking in place between
/// rules. Per-marking unconditional rows precede the Trio 2 RELIDO
/// row so that NOFORN added by HCS-O / HCS-P[sub] / TK-BLFH /
/// TK-IDIT / TK-KAND is visible to Trio 2's suppressor check in
/// the same iteration. SI-G adds ORCON only (no NOFORN), so its
/// suppression of Trio 2 routes through `TOK_SI_G ∈
/// FDR_OR_RELIDO_INCOMPAT` directly rather than via Kleene chain.
/// See `CLOSURE_RELIDO_SCI`'s doc-comment for the full ordering
/// rationale.
///
/// Per-row monotonicity attestation (§4.7.3 table-design property, case 2):
/// Every suppressor fact either contains the cone's intent or makes it
/// redundant. For Trio 1/3 (FDR_DOMINATORS): the suppressor is always a
/// manifest FD&R decision that supersedes the implicit default. For Trio 2
/// (FDR_OR_RELIDO_INCOMPAT): same, plus RELIDO-incompatible tokens make the
/// RELIDO cone inapplicable by definition. Unconditional rows have no
/// suppressor — monotonicity is trivial (empty suppressor → no case 2).
///
/// # Deferred Trio 2 rows
///
/// `CLOSURE_RELIDO_US_CLASS` (US unclassified-or-collateral implies
/// RELIDO) and `CLOSURE_RELIDO_FOUO` (FOUO implies RELIDO) are
/// deferred to a follow-up. The US_CLASS row needs a more
/// sophisticated "no other dissem" trigger composition than the
/// bare-CAT_CLASSIFICATION proxy that over-fired in the historical
/// placeholder; the FOUO row follows once US_CLASS lands.
pub(super) static CAPCO_CLOSURE_RULES: &[ClosureRule<CapcoScheme>] = &[
    // Trio 1: implicit NOFORN — single CAVEATED row whose triggers union
    // every caveat marking per §B.3 p20 Note (SAR / AEA / dissem controls /
    // non-IC dissem). Same suppressor (FDR_DOMINATORS) and same cone
    // ({NOFORN}) collapse the seven historical rows into one per D18
    // rationale 2.
    CLOSURE_NOFORN_CAVEATED,
    // Per-marking unconditional implications (Issue #524 Phase 2). Ordered
    // before the Trio 2 RELIDO row so the NOFORN/ORCON cones populate
    // `working` before `CLOSURE_RELIDO_SCI`'s suppressor check runs in the
    // same Kleene iteration.
    CLOSURE_HCS_O_IMPLIES_NF_OC,
    CLOSURE_HCS_P_SUB_IMPLIES_NF_OC,
    CLOSURE_SI_G_IMPLIES_OC,
    CLOSURE_TK_BLFH_IMPLIES_NF,
    CLOSURE_TK_IDIT_IMPLIES_NF,
    CLOSURE_TK_KAND_IMPLIES_NF,
    // Trio 3: implicit `REL TO USA, NATO` for bare NATO classification.
    // Fires at `Severity::Info` (silent lattice-layer fact propagation);
    // S007 owns the text-layer `Severity::Suggest` byte-diff per D20.
    // NATO routes via `cone_derived` (open-vocab `CountryCode::NATO`),
    // USA via the static cone (`TOK_USA` → `CountryCode::USA` through
    // `apply_fact_add`'s CAT_REL_TO arm).
    CLOSURE_REL_TO_USA_NATO,
    // Trio 2: implicit RELIDO for bare SCI controls. Ordered last so
    // the per-marking NOFORN cones above are visible in `working`
    // for the FDR_OR_RELIDO_INCOMPAT suppressor check.
    CLOSURE_RELIDO_SCI,
];

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
