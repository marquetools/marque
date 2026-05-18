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
//! default); per-token §H.X authorities live in the row doc-comment's
//! authority table.

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
/// algebraic basis; per-token §H.X authorities live in the per-trigger
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
    // Trio 1: implicit NOFORN — single CAVEATED row whose triggers union
    // every caveat marking per §B.3 p20 Note (SAR / AEA / dissem controls /
    // non-IC dissem). Same suppressor (FDR_DOMINATORS) and same cone
    // ({NOFORN}) collapse the seven historical rows into one per D18
    // rationale 2.
    CLOSURE_NOFORN_CAVEATED,
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
