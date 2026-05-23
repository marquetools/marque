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

// `FDR_OR_RELIDO_INCOMPAT` — retired in issue #704 along with the
// `phase2_closure_pin` test module that was its sole consumer. The
// suppressor architecture it backstopped (`CLOSURE_TABLE` Row 8 +
// `MASK_FDR_OR_RELIDO_INCOMPAT` projection) violated the closure
// operator's algebraic monotonicity property; the §H.8 p145 / §B.3.a
// p19 FD&R supersession semantics moved to
// `CapcoScheme::apply_supersession_overlays`. See the post-#704
// architecture note in `closure_table.rs`'s module doc-comment for
// the full rationale.

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
/// unless an FD&R dominator strips it post-closure.
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
/// **No suppressors (issue #704)**: pre-#704 this row carried
/// `FDR_DOMINATORS` as its suppressor set. The trait-level
/// `suppressors` mechanism is anti-monotone in the closure operator
/// (adding bits can activate a suppressor and strictly lose cone
/// bits from `Cl(b)` vs `Cl(a)`), which violated the operator's
/// algebraic monotonicity contract and produced the failing
/// `proptest_closure_table::p3_monotonicity_realistic` seed. The
/// §H.8 p145 NOFORN-dominates / §B.3.a p19 FD&R supersession
/// semantics moved to [`CapcoScheme::apply_supersession_overlays`],
/// which runs post-closure and observes the post-Kleene state. The
/// `CLOSURE_TABLE` bitmask Row 7 mirrors this row and is also
/// suppressor-free; the bitmask path makes Row 7's firing decision
/// (`row7_fired` in `CapcoScheme::closure`) from `closed_bits`
/// without re-evaluating the trait `suppressors` slice, so the
/// `&[]` here is honest with runtime behavior.
///
/// **Cone shape**: USA via the static `cone` (`TOK_USA`, which
/// `apply_fact_add` routes to `CountryCode::USA` on CAT_REL_TO); NATO
/// via `cone_derived` returning `FactRef::OpenVocab(CountryCode::NATO)`
/// because `CountryCode::NATO` has no closed-vocab `TokenId`. Both facts
/// route to CAT_REL_TO via `CapcoScheme::category_of`.
pub(super) const CLOSURE_REL_TO_USA_NATO: ClosureRule<CapcoScheme> = ClosureRule {
    name: "capco:closure.nato.rel-to-usa-nato-if-nato-classification",
    display_label: "Bare NATO classification implies REL TO USA, NATO",
    label: capco(SectionLetter::H, 7, 127),
    triggers: &[TokenRef::Token(TOK_NATO_CLASS)],
    suppressors: &[],
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
// Trio 2 RELIDO suppressor slices — retired in issue #704
// ---------------------------------------------------------------------------
//
// The two Trio 2 closure rules (`CLOSURE_RELIDO_SCI` +
// `CLOSURE_RELIDO_US_CLASS`) were retired in PR-D of the FactBitmask
// refactor (issue #371) into `CLOSURE_TABLE` Rows 8-9.
//
// The `RELIDO_US_CLASS_SUPPRESSORS` `TokenRef` slice that backstopped
// the bitmask `MASK_RELIDO_US_CLASS_SUPPRESSORS` projection retired
// with the `phase3_closure_pin` test module in issue #704 — both the
// slice and its sole consumer encoded the pre-#704 suppressor
// architecture that violated the closure operator's algebraic
// monotonicity property. The §H.8 p145 / §B.3.a p19 FD&R supersession
// semantics moved to `CapcoScheme::apply_supersession_overlays`. See
// the post-#704 architecture note in `closure_table.rs`'s module
// doc-comment for the full rationale.

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

    /// Post-#704 FD&R supersession on the issue #525 trigger arms:
    /// `project(Page)` with PROPIN/FISA/RAWFISA + NOFORN converges to
    /// `{Trigger, NOFORN}` with no RELIDO. The closure() layer
    /// produces `{Trigger, NOFORN, RELIDO}` (Row 0 dedups NOFORN; Row
    /// 9 adds RELIDO unconditionally — closure is purely additive
    /// post-#704); the supersession overlay strips RELIDO per §H.8
    /// p145 at the project() boundary.
    ///
    /// Pre-#704 the test asserted `dissem_us.len()` stable at the
    /// closure() layer (the `MASK_FDR_DOMINATORS` suppressor on
    /// Row 0 + Row 9 prevented both implicit defaults from firing).
    /// The post-#704 reading is observationally equivalent at the
    /// project() level: input `{Trigger, NF}` → output `{Trigger,
    /// NF}`, length 2 → 2. The §-citations are preserved verbatim.
    ///
    /// Authority: §B.3 Table 2 p21 (caveated-default obligation
    /// drives Row 0 firing); §H.8 p145 (NOFORN-dominates supersession
    /// overlay strips RELIDO); §H.8 p148 (PROPIN), §H.8 p161 (FISA +
    /// RAWFISA) per the issue #525 trigger registration.
    #[test]
    fn project_resolves_new_trigger_plus_noforn_dominates_relido() {
        use marque_scheme::Scope;
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
            let m = CapcoMarking::new(a);
            let out = scheme.project(Scope::Page, &[m]);
            assert_eq!(
                out.0.dissem_us.len(),
                before_len,
                "project must converge to length-stable `{{Trigger, NF}}` on \
                 {trigger:?} + NOFORN (closure adds RELIDO; overlay strips \
                 it per §H.8 p145); dissem_us = {:?}",
                out.0.dissem_us
            );
            assert!(
                out.0.dissem_us.contains(&DissemControl::Nf),
                "NOFORN must be retained on trigger {trigger:?}: project must \
                 not strip the seed NOFORN fact; dissem_us = {:?}",
                out.0.dissem_us
            );
            assert!(
                !out.0.dissem_us.contains(&DissemControl::Relido),
                "RELIDO must be stripped by §H.8 p145 overlay on \
                 {trigger:?} + NOFORN; dissem_us = {:?}",
                out.0.dissem_us
            );
        }
    }
}
