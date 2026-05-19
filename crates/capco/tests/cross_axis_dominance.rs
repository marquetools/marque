// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Cross-axis dominance tests for the per-category lattice impls.
//!
//! Each test pins a specific cross-axis rule from CAPCO-2016 to the
//! corresponding lattice / constraint / page-rewrite behavior. PR 4b-A
//! lands the AEA-axis coverage; subsequent PRs add the other
//! categories per `docs/plans/2026-05-01-lattice-design.md` §§2-8.
//!
//! # AEA category coverage (PR 4b-A)
//!
//! Four cross-axis cases:
//!
//! 1. **RD evicts FRD / TFNI** — §H.6 p104 (RD precedence over
//!    FRD/TFNI in banner roll-up). Tested via lattice join.
//! 2. **SIGMA coalesces under RD** — §H.6 p108-109 (all SIGMA
//!    numbers list under RD-SIGMA regardless of source). Tested via
//!    lattice join.
//! 3. **UCNI strips when classified** — §H.6 p116 / p118 (UCNI
//!    suppressed from banner when class > U + NOFORN promoted).
//!    Tested as the documented predicate; the runtime rewrite
//!    lands in PR 4b-C.
//! 4. **ATOMAL routes to AEA, not classification** — §H.7 p122 +
//!    §G.2 Table 5 p40 (ATOMAL is a registered standalone control
//!    marking that travels in the AEA category alongside RD/FRD/TFNI).

use marque_capco::lattice::{AeaPrimary, AeaSet, UcniKind};
use marque_capco::scheme::{CapcoMarking, CapcoScheme};
use marque_ism::{
    AeaMarking, AtomalBlock, CanonicalAttrs, Classification, CountryCode, DissemControl,
    FgiClassification, FgiMarker, FrdBlock, MarkingClassification, NonIcDissem, RdBlock,
    SciCompartment, SciControlBare, SciControlSystem, SciMarking,
};
use marque_scheme::{JoinSemilattice, MarkingScheme as _, Scope};
use smol_str::SmolStr;

// ===========================================================================
// AEA: RD evicts FRD / TFNI (§H.6 p104)
// ===========================================================================

/// CAPCO-2016 §H.6 p104 (RESTRICTED DATA, Precedence Rules for Banner
/// Line Guidance): "If RD, FRD, and TFNI portions are in a document,
/// the RD takes precedence and is conveyed in the banner line. In
/// this case, use only the RD warning statement."
///
/// Tested as the lattice join over three AeaSets — one with each
/// primary marking — to verify `Lattice::join` returns the
/// supersession max under `Tfni ⊏ Frd ⊏ Rd`.
#[test]
fn aea_rd_evicts_frd_tfni() {
    let rd_set = AeaSet::from_markings(&[AeaMarking::Rd(RdBlock::default())]);
    let frd_set = AeaSet::from_markings(&[AeaMarking::Frd(FrdBlock::default())]);
    let tfni_set = AeaSet::from_markings(&[AeaMarking::Tfni]);

    // RD ⊔ FRD = RD.
    let joined = rd_set.join(&frd_set);
    assert_eq!(joined.primary(), Some(AeaPrimary::Rd));

    // RD ⊔ TFNI = RD.
    let joined = rd_set.join(&tfni_set);
    assert_eq!(joined.primary(), Some(AeaPrimary::Rd));

    // RD ⊔ FRD ⊔ TFNI = RD.
    let joined = rd_set.join(&frd_set).join(&tfni_set);
    assert_eq!(joined.primary(), Some(AeaPrimary::Rd));

    // FRD ⊔ TFNI = FRD (the §H.6 p111 "FRD takes precedence over
    // TFNI" implication of the §H.6 p120 TFNI banner-eviction rule).
    let joined = frd_set.join(&tfni_set);
    assert_eq!(joined.primary(), Some(AeaPrimary::Frd));

    // Render the RD-wins case to the boxed AeaMarking output to
    // confirm the rendered form omits the dominated atoms.
    let combined = rd_set.join(&frd_set).join(&tfni_set);
    let rendered = combined.to_markings();
    assert_eq!(rendered.len(), 1);
    assert!(matches!(&rendered[0], AeaMarking::Rd(_)));
}

// ===========================================================================
// AEA: SIGMA coalesces under RD (§H.6 p108-109 + §H.6 p113)
// ===========================================================================

/// CAPCO-2016 §H.6 p108-109 (RD-SIGMA Precedence Rules, top-of-page
/// continuation at p109): "If both RD and FRD SIGMA [#] portions are
/// in a document, the RD-SIGMA [#] marking takes precedence over the
/// FRD-SIGMA [#] marking in the banner line and all SIGMA numbers are
/// listed in the RD-SIGMA [#] marking in the banner line, regardless
/// of whether the information was RD or FRD."
///
/// Mutually cited at §H.6 p113 from the FRD-SIGMA subsection's
/// vantage. The two passages are mutual references.
///
/// Tested by joining one RD-SIGMA portion with one FRD-SIGMA portion
/// and verifying the rendered output carries all SIGMA numbers under
/// the surviving RD-SIGMA atom.
#[test]
fn aea_sigma_coalesces_under_rd() {
    // (S//RD/SIGMA 14)
    let rd_sigma_14 = AeaSet::from_markings(&[AeaMarking::Rd(RdBlock {
        cnwdi: false,
        sigma: Box::new([14]),
    })]);
    // (S//FRD/SIGMA 18)
    let frd_sigma_18 = AeaSet::from_markings(&[AeaMarking::Frd(FrdBlock {
        sigma: Box::new([18]),
    })]);

    // Join: primary = Rd; sigmas = {14, 18}; banner renders RD-SIGMA.
    let combined = rd_sigma_14.join(&frd_sigma_18);
    assert_eq!(combined.primary(), Some(AeaPrimary::Rd));
    let sigmas: Vec<u8> = combined.sigmas().iter().copied().collect();
    assert_eq!(sigmas, vec![14, 18]);

    // Render confirms the §H.6 p109 canonical form:
    // `RD-SIGMA 14 18` (ascending order per §H.6 p108
    // "Multiple SIGMA numbers must be listed in numerical order").
    let rendered = combined.to_markings();
    assert_eq!(rendered.len(), 1);
    match &rendered[0] {
        AeaMarking::Rd(rd) => {
            let nums: Vec<u8> = rd.sigma.to_vec();
            assert_eq!(nums, vec![14, 18]);
            assert!(!rd.cnwdi);
        }
        other => panic!("expected RD atom, got {other:?}"),
    }
}

// ===========================================================================
// AEA: UCNI strips when classified (§H.6 p116 / p118) — documented predicate
// ===========================================================================

/// CAPCO-2016 §H.6 p116 (DOD UCNI Precedence Rules for Banner Line
/// Guidance): "Classified documents: DOD UCNI does not appear in the
/// banner line; however, NOFORN must be applied if a less restrictive
/// FD&R marking would otherwise be conveyed with the classified
/// information."
///
/// Symmetric rule at §H.6 p118 for DOE UCNI.
///
/// PR 4b-A documents the predicate as a cross-axis rule. The actual
/// runtime rewrite (strip UCNI + promote NOFORN) lands in PR 4b-C,
/// alongside the §3 (b) FOUO eviction matrix — same algebraic shape
/// (cross-axis strip on classification ascent + NOFORN promotion).
///
/// This test verifies the **AeaSet lattice's contribution** to the
/// rule: the UCNI atom is present in the joined fact set (because
/// the lattice is permissive — strip happens at the post-projection
/// rewrite, not at lattice-join time).
///
/// **PR 4b-C WILL INVERT THE FINAL ASSERTION** — once the
/// post-projection rewrite lands, `combined.to_markings()` after
/// the cross-axis pass should drop UCNI when classification ⊐ U.
/// This test stays here as the lattice-contribution pin; the new
/// assertion against the post-rewrite state goes in a separate
/// test alongside the FOUO eviction matrix in PR 4b-C, NOT by
/// editing this one (so the regression catch for "lattice
/// preserves UCNI" stays intact).
#[test]
fn aea_ucni_strips_when_classified() {
    // (U//DOD UCNI//FOUO) — UCNI carrier portion.
    let ucni_portion = AeaSet::from_markings(&[AeaMarking::DodUcni]);
    // (S) — classified peer with no AEA content.
    let class_portion = AeaSet::empty();

    // AeaSet join leaves UCNI in place. The classification axis is
    // handled by `MarkingClassification::OrdMax` separately; that
    // join lifts the banner to S.
    let combined = ucni_portion.join(&class_portion);
    assert!(combined.ucni().contains(&UcniKind::DodUcni));
    assert!(combined.primary().is_none());

    // The actual eviction is a post-projection cross-axis rewrite
    // (PR 4b-C). Until then, the lattice contributes the
    // `{DodUcni}` atom faithfully; the §H.6 p116 strip-plus-promote
    // semantics ride on the cross-axis rewrite layer above.
    //
    // The lattice render of the post-join state intentionally
    // emits the UCNI atom (the `to_markings` path is
    // classification-agnostic — strip is a cross-axis decision):
    let rendered = combined.to_markings();
    let has_dod_ucni = rendered.iter().any(|m| matches!(m, AeaMarking::DodUcni));
    assert!(
        has_dod_ucni,
        "AeaSet rendered form preserves UCNI atom; banner-strip is post-projection (PR 4b-C)"
    );
}

// ===========================================================================
// AEA: ATOMAL routes to AEA, not classification (§G.2 Table 5 p40 +
// §H.7 p122 FGI-section worked example; ATOMAL has no dedicated §H
// entry — its registration lives in §G.2 Table 5, and the §H.7
// worked example illustrates the AEA-axis placement).
// ===========================================================================

/// CAPCO-2016 §H.7 p122 (Foreign Government Information Markings,
/// banner-roll-up worked example): "*SECRET//RD/ATOMAL//FGI NATO
/// //NOFORN, where ATOMAL is a NATO Atomic Energy Act marking that
/// follows the registered US Atomic Energy Act marking RD.*"
///
/// Confirmed by §G.2 Table 5 p40 (Conceptual Access Rights and
/// Handling by Registered Marking): ATOMAL is a registered standalone
/// control marking under "Non-US Protective Markings" with an
/// "ATOMAL read-in" handling requirement — the marking's *handling
/// classification* is non-US but its *category placement* on a
/// US-classified document is AEA (per §H.7 p122 routing).
///
/// This is the PR 9c.1 T134 routing decision (ATOMAL → AEA, not
/// NATO classification suffix). The test pins it at the `AeaSet`
/// boundary: a portion carrying RD + ATOMAL composes into an
/// `AeaSet` with both `primary=Some(Rd)` and `atomal=Some(_)`,
/// confirming ATOMAL lives in the AEA axis.
#[test]
fn aea_atomal_routes_to_aea_not_nato_class() {
    // (//CTS//RD/ATOMAL//FGI NATO//NOFORN) — the §H.7 p122 worked
    // example, AEA-axis content (RD + ATOMAL).
    let portion = AeaSet::from_markings(&[
        AeaMarking::Rd(RdBlock::default()),
        AeaMarking::Atomal(AtomalBlock),
    ]);

    // Both `Rd` and `Atomal` survive on the AeaSet.
    assert_eq!(portion.primary(), Some(AeaPrimary::Rd));
    assert_eq!(portion.atomal(), Some(AtomalBlock));

    // Render produces RD followed by ATOMAL in §G.1 Table 4
    // cat-6 register order (RD → ATOMAL):
    let rendered = portion.to_markings();
    assert_eq!(rendered.len(), 2);
    assert!(matches!(&rendered[0], AeaMarking::Rd(_)));
    assert!(matches!(&rendered[1], AeaMarking::Atomal(_)));

    // Joining with a second portion carrying only ATOMAL is
    // idempotent on the ATOMAL axis (per the OptionalSingleton
    // join law).
    let atomal_only = AeaSet::from_markings(&[AeaMarking::Atomal(AtomalBlock)]);
    let combined = portion.join(&atomal_only);
    assert_eq!(combined.atomal(), Some(AtomalBlock));
    assert_eq!(combined.primary(), Some(AeaPrimary::Rd));
}

// ===========================================================================
// PR-4 test closeout (006 T117) — additional cross-axis dominance fixtures.
//
// The 4 tests below cover the non-AEA fixture classes per PM doc D-2 of
// `docs/plans/2026-05-19-pr4-tests-closeout-pm-decisions.md`. Each test
// drives 2-3 hand-built `CanonicalAttrs` portions through
// `CapcoScheme::project(Scope::Page, &markings)` and asserts on the
// resulting `CanonicalAttrs` payload. Mirrors the
// `crates/capco/tests/lattice_vs_scheme_parity.rs:114-118` idiom for
// the full-pipeline projection path.
// ===========================================================================

// --- Helpers (parallel to `lattice_vs_scheme_parity.rs:179-213`) -----------

fn cc(s: &str) -> CountryCode {
    CountryCode::try_new(s.as_bytes()).expect("valid trigraph")
}

/// Build a single US-classification portion at `level`.
fn portion_us(level: Classification) -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(level));
    a
}

/// Drive a list of `CanonicalAttrs` portions through
/// `CapcoScheme::project(Scope::Page, ...)` and return the resulting
/// `CanonicalAttrs` (CapcoMarking's `.0`).
fn project_via_scheme(portions: &[CanonicalAttrs]) -> CanonicalAttrs {
    let scheme = CapcoScheme::new();
    let markings: Vec<CapcoMarking> = portions.iter().cloned().map(CapcoMarking::new).collect();
    scheme.project(Scope::Page, &markings).0
}

// ===========================================================================
// FOUO: classified-document sub-clause (§H.8 p134)
// ===========================================================================

/// CAPCO-2016 §H.8 p134 (FOUO Precedence Rules for Banner Line Guidance,
/// classified-document sub-clause): "When a classified document contains
/// portions of FOUO information, the FOUO marking is not used in the
/// banner line."
///
/// Cross-axis: classification × dissem. The classification-ascent path
/// from `Unclassified` (the FOUO carrier portion) to `Secret` (the
/// classified peer) triggers the `capco/classification-evicts-fouo`
/// PageRewrite (PR 4b-C Pattern-B Commit 4). This is the lattice-design
/// §3 Example 3 worked case and parallels the `fouo-eviction-class.txt`
/// fixture under `tests/corpus/lattice/`.
///
/// Citation re-verified against `crates/capco/docs/CAPCO-2016.md` at
/// authorship 2026-05-19 (§H.8 p134, classified-document sub-clause).
#[test]
fn class_evicts_fouo_via_classification_ascent() {
    // (U//FOUO) + (S) — FOUO carrier + classified peer.
    let mut fouo_portion = portion_us(Classification::Unclassified);
    fouo_portion.dissem_us = vec![DissemControl::Fouo].into_boxed_slice();
    let class_portion = portion_us(Classification::Secret);
    let portions = [fouo_portion, class_portion];

    let projected = project_via_scheme(&portions);

    // Classification rolls up to Secret per OrdMax over the classification
    // axis (§D.2 p28 "Take the maximum classification level across all
    // portions").
    assert_eq!(
        projected.classification,
        Some(MarkingClassification::Us(Classification::Secret)),
        "classification must roll up to Secret"
    );
    // FOUO is stripped from the projected `dissem_us` set by the
    // `capco/classification-evicts-fouo` PageRewrite (§H.8 p134
    // classified-document sub-clause).
    assert!(
        !projected.dissem_us.contains(&DissemControl::Fouo),
        "FOUO must be stripped from classified-document banner, got {:?}",
        projected.dissem_us
    );
    // CLOSURE injection: post-FOUO-strip the page is classified +
    // uncaveated + post-28-Jun-2010 → mark as RELIDO per §B.3 Table 2
    // p21 row 1. Driven by `CLOSURE_RELIDO_US_CLASS` at
    // `crates/capco/src/scheme/closure.rs`. The runner output for the
    // parallel `fouo-eviction-class.txt` fixture confirms the
    // post-closure banner is `SECRET//RELIDO`. Citation re-verified
    // against `crates/capco/docs/CAPCO-2016.md` at authorship 2026-05-19.
    assert!(
        projected.dissem_us.contains(&DissemControl::Relido),
        "CLOSURE_RELIDO_US_CLASS must inject RELIDO post-FOUO-strip on \
         classified document per §B.3 Table 2 p21 (classified + uncaveated \
         + on/after 28 June 2010 → mark as RELIDO), got {:?}",
        projected.dissem_us
    );
}

// ===========================================================================
// FOUO: UNCLASSIFIED-with-other-control sub-clause (§H.8 p134)
// ===========================================================================

/// CAPCO-2016 §H.8 p134 (FOUO Precedence Rules for Banner Line Guidance,
/// UNCLASSIFIED-with-other-control sub-clause): "FOUO is not conveyed
/// in the banner line if the document is UNCLASSIFIED with FOUO and
/// other dissemination control markings, excluding any FD&R markings."
///
/// Within-axis (per the lattice-design taxonomy: this fires on the
/// IC dissem axis alone; LES is non-IC dissem, the catalog row's
/// trigger sees the LES presence and strips FOUO without needing
/// classification ascent). LES is intentionally non-FD&R per the
/// FDR_DOMINATORS slice at `crates/capco/src/scheme/closure.rs` —
/// FD&R is NOFORN / RELIDO / REL TO / DISPLAY ONLY / EYES only;
/// LES is a non-IC dissem control per §H.9 p181.
///
/// This is the lattice-design §3 Example 4 worked case and parallels
/// the `fouo-eviction-non-fdr.txt` fixture under
/// `tests/corpus/lattice/`. The `capco/non-fdr-control-evicts-fouo`
/// PageRewrite is the operative declarative row (PR 4b-C Pattern-B
/// Commit 4).
///
/// Citation re-verified against `crates/capco/docs/CAPCO-2016.md` at
/// authorship 2026-05-19 (§H.8 p134, UNCLASSIFIED-with-other-control
/// sub-clause). LES non-IC-dissem classification verified against
/// §H.9 p181.
#[test]
fn non_fdr_control_evicts_fouo() {
    // (U//LES//FOUO) — single portion at U with non-IC LES + FOUO.
    let mut portion = portion_us(Classification::Unclassified);
    portion.dissem_us = vec![DissemControl::Fouo].into_boxed_slice();
    portion.non_ic_dissem = vec![NonIcDissem::Les].into_boxed_slice();
    let portions = [portion];

    let projected = project_via_scheme(&portions);

    // Classification stays Unclassified (no classified portions to lift).
    assert_eq!(
        projected.classification,
        Some(MarkingClassification::Us(Classification::Unclassified)),
        "classification stays Unclassified"
    );
    // LES survives in non_ic_dissem per §G.1 Table 4 p38 category order.
    assert!(
        projected.non_ic_dissem.contains(&NonIcDissem::Les),
        "LES must survive in non_ic_dissem, got {:?}",
        projected.non_ic_dissem
    );
    // FOUO is stripped from `dissem_us` by the
    // `capco/non-fdr-control-evicts-fouo` PageRewrite — LES is non-IC,
    // non-FD&R, so the catalog row's trigger fires.
    assert!(
        !projected.dissem_us.contains(&DissemControl::Fouo),
        "FOUO must be stripped when non-FD&R dissem is present, got {:?}",
        projected.dissem_us
    );
    // CLOSURE injection: post-strip the page carries a non-IC dissem
    // (LES) with no FD&R dominator, so `CLOSURE_NOFORN_CAVEATED`
    // (`crates/capco/src/scheme/closure.rs:348`, the consolidated
    // §B.3 Table 2 p21 row that absorbed the per-token rows in PR
    // #522 / decisions.md D18) fires and injects NOFORN. This pins
    // the universal IC principle that non-IC dissem implies NOFORN
    // absent FD&R — LES is non-IC per §H.9 p181. The runner output
    // for this fixture confirms the post-closure banner is
    // `UNCLASSIFIED//NOFORN//LES`. Citation re-verified against
    // `crates/capco/docs/CAPCO-2016.md` at authorship 2026-05-19.
    assert!(
        projected.dissem_us.contains(&DissemControl::Nf),
        "CLOSURE_NOFORN_CAVEATED must inject NOFORN because non-IC dissem \
         present and no FD&R dominator per §B.3 Table 2 p21, got {:?}",
        projected.dissem_us
    );
}

// ===========================================================================
// FGI: banner roll-up retains FGI marker on cross-classified pages
// (§H.7 pp123-125 + §H.7 p129)
// ===========================================================================

/// CAPCO-2016 §H.7 pp123-125 (FGI banner roll-up plus reciprocal
/// classification) and §H.7 p129 worked example: `(//DEU TS//NF)`
/// portion plus US-classified portion → banner
/// `TOP SECRET//FGI [LIST]//NOFORN`. Reciprocal classification
/// (§H.7 pp123-125) lifts the page to the foreign-equivalent level
/// (TS); the FGI marker captures the foreign provenance; REL TO is
/// cleared by the `capco/noforn-clears-rel-to` PageRewrite per §H.8
/// p145.
///
/// This is the lattice-design §2 Example 2 + §6 Example 2 + §4.8.5
/// worked case and parallels the `fgi-banner-rollup.txt` fixture under
/// `tests/corpus/lattice/`. Issue #276 fixture
/// `tests/corpus/foreign/mixed_us_foreign_rollup.expected.json` is
/// the corpus-level analogue.
///
/// Citation re-verified against `crates/capco/docs/CAPCO-2016.md` at
/// authorship 2026-05-19 (§H.7 pp123-129; p129 worked example).
#[test]
fn fgi_banner_rollup_retains_marker_on_cross_classified_page() {
    // Portion 1: (C//NF) — US Confidential + NOFORN.
    let mut us_portion = portion_us(Classification::Confidential);
    us_portion.dissem_us = vec![DissemControl::Nf].into_boxed_slice();

    // Portion 2: (//GBR TS//REL TO USA, GBR) — UK Top Secret FGI.
    let mut fgi_portion = CanonicalAttrs::default();
    fgi_portion.classification = Some(MarkingClassification::Fgi(FgiClassification {
        level: Classification::TopSecret,
        countries: Box::new([cc("GBR")]),
    }));
    fgi_portion.rel_to = vec![cc("USA"), cc("GBR")].into_boxed_slice();

    let portions = [us_portion, fgi_portion];
    let projected = project_via_scheme(&portions);

    // Reciprocal classification raises to TopSecret per §H.7 pp123-125
    // (the foreign-side TS converts to equivalent-US TS; the page is
    // NOT solely-non-US because the US portion participates).
    assert_eq!(
        projected.classification,
        Some(MarkingClassification::Us(Classification::TopSecret)),
        "reciprocal classification must lift to Us(TopSecret), got {:?}",
        projected.classification
    );

    // FGI marker is acknowledged with GBR (foreign provenance captured).
    match &projected.fgi_marker {
        Some(FgiMarker::Acknowledged { countries, .. }) => {
            let set: std::collections::BTreeSet<CountryCode> = countries.iter().copied().collect();
            assert!(
                set.contains(&cc("GBR")),
                "FGI marker must contain GBR, got {set:?}"
            );
        }
        other => panic!("expected Acknowledged FGI marker, got {other:?}"),
    }

    // NOFORN survives on the page (carried over from the US portion).
    assert!(
        projected.dissem_us.contains(&DissemControl::Nf),
        "NOFORN must survive on page, got {:?}",
        projected.dissem_us
    );

    // REL TO is cleared by the `capco/noforn-clears-rel-to` PageRewrite
    // per §H.8 p145 (NOFORN dominates and the supersession overlay
    // strips dominated REL TO).
    assert!(
        projected.rel_to.is_empty(),
        "REL TO must be cleared by capco/noforn-clears-rel-to, got {:?}",
        projected.rel_to
    );
}

// ===========================================================================
// SCI: cross-system canonicalization with `/` separator
// (§H.4 p61 + §A.6 pp15-17)
// ===========================================================================

/// CAPCO-2016 §H.4 p61 (SCI grammar): "Multiple SCI control system
/// markings must be listed in ascending sort order with numbered values
/// first followed by alphabetic values separated by a single forward
/// slash with no interjected space ('/'). ... Multiple compartments
/// within an SCI control system must be listed in ascending sort order
/// ... separated by a hyphen ('-'). ... Multiple sub-compartments must
/// be listed in ascending sort order ... separated by a space."
///
/// Plus §A.6 pp15-17 (general separator alphabet).
///
/// This is the lattice-design §4 Example 3 worked case and parallels
/// the `sci-cross-system.txt` fixture under `tests/corpus/lattice/`.
/// The `SciSet::from_markings` path composes both portions' SCI atoms
/// into a single `SciSet` whose `to_markings` rendering reflects the
/// canonical numeric-then-alpha order (SI before TK alpha tiebreak).
///
/// Citation re-verified against `crates/capco/docs/CAPCO-2016.md` at
/// authorship 2026-05-19 (§H.4 p61 grammar prose; §A.6 pp15-17
/// general separator alphabet).
#[test]
fn sci_cross_system_canonicalization() {
    // Portion 1: (TS//SI-G ABCD) — SI control system with G compartment +
    // ABCD sub-compartment.
    let mut p1 = portion_us(Classification::TopSecret);
    let si_g_abcd = SciMarking::new(
        SciControlSystem::Published(SciControlBare::Si),
        Box::new([SciCompartment::new(
            SmolStr::from("G"),
            Box::new([SmolStr::from("ABCD")]),
        )]),
        None,
    );
    p1.sci_markings = vec![si_g_abcd].into_boxed_slice();

    // Portion 2: (TS//TK-BLFH XYZW) — TK control system with BLFH
    // compartment + XYZW sub-compartment.
    let mut p2 = portion_us(Classification::TopSecret);
    let tk_blfh_xyzw = SciMarking::new(
        SciControlSystem::Published(SciControlBare::Tk),
        Box::new([SciCompartment::new(
            SmolStr::from("BLFH"),
            Box::new([SmolStr::from("XYZW")]),
        )]),
        None,
    );
    p2.sci_markings = vec![tk_blfh_xyzw].into_boxed_slice();

    let portions = [p1, p2];
    let projected = project_via_scheme(&portions);

    // Classification rolls up to TopSecret (both portions equal).
    assert_eq!(
        projected.classification,
        Some(MarkingClassification::Us(Classification::TopSecret)),
    );

    // SCI markings: BOTH atoms must survive (different control systems
    // do not absorb each other per §H.4 p61 grammar — they compose with
    // `/` separator).
    let systems: Vec<&SciControlSystem> =
        projected.sci_markings.iter().map(|m| &m.system).collect();
    let has_si = systems
        .iter()
        .any(|s| matches!(s, SciControlSystem::Published(SciControlBare::Si)));
    let has_tk = systems
        .iter()
        .any(|s| matches!(s, SciControlSystem::Published(SciControlBare::Tk)));
    assert!(
        has_si && has_tk,
        "both SI and TK systems must survive cross-system join, got {systems:?}",
    );

    // SCI ordering: SI before TK in the projected `sci_markings` per the
    // numeric-then-alpha sort prescribed by §H.4 p61.
    let si_idx = projected
        .sci_markings
        .iter()
        .position(|m| matches!(m.system, SciControlSystem::Published(SciControlBare::Si)));
    let tk_idx = projected
        .sci_markings
        .iter()
        .position(|m| matches!(m.system, SciControlSystem::Published(SciControlBare::Tk)));
    assert!(
        si_idx.is_some() && tk_idx.is_some() && si_idx.unwrap() < tk_idx.unwrap(),
        "SI must sort before TK (alpha tiebreak per §H.4 p61), got si_idx={si_idx:?}, tk_idx={tk_idx:?}",
    );

    // CLOSURE injections per the §H.4 per-system implications:
    //
    // - SI-G implies ORCON per §H.4 p80 (GAMMA Example Banner Line
    //   `TOP SECRET//SI-G//ORCON` and the Relationship/Other-Markings
    //   prose). Closure rule `CLOSURE_SI_G_IMPLIES_OC`.
    // - TK-BLFH implies NOFORN per §H.4 p87 ("Requires NOFORN" at
    //   the BLUEFISH entry's Relationship(s) to Other Markings
    //   bullet list). Closure rule `CLOSURE_TK_BLFH_IMPLIES_NF`.
    //
    // The runner output for the parallel `sci-cross-system.txt`
    // fixture confirms the post-closure banner is
    // `TOP SECRET//SI-G ABCD/TK-BLFH XYZW//ORCON/NOFORN`. Citations
    // re-verified against `crates/capco/docs/CAPCO-2016.md` at
    // authorship 2026-05-19.
    assert!(
        projected.dissem_us.contains(&DissemControl::Oc),
        "CLOSURE_SI_G_IMPLIES_OC must inject ORCON per §H.4 p80 \
         (GAMMA Example Banner Line `TOP SECRET//SI-G//ORCON`), got {:?}",
        projected.dissem_us
    );
    assert!(
        projected.dissem_us.contains(&DissemControl::Nf),
        "CLOSURE_TK_BLFH_IMPLIES_NF must inject NOFORN per §H.4 p87 \
         (BLUEFISH 'Requires NOFORN'), got {:?}",
        projected.dissem_us
    );
}

// ===========================================================================
// PR-4 test closeout (006 T117a) — US reciprocates equivalent protection
// for foreign portions (§H.7 pp123-129)
// ===========================================================================

/// CAPCO-2016 §H.7 pp123-129 + §H.7 p129 worked example (line ~3168):
/// `(S//REL TO USA, AUS) (//CAN S//REL TO USA, AUS, CAN, GBR) (//DEU TS//NF) →
/// TOP SECRET//FGI CAN DEU//NOFORN`.
///
/// The US-reciprocates-equivalent-protection property: when the US
/// classification is below the foreign-side equivalent, the page-level
/// banner raises to the foreign-equivalent US level. The FGI marker
/// captures the foreign provenance from the FGI-marker / foreign-
/// classification axis.
///
/// This is the property-test analogue of
/// `tests/corpus/foreign/mixed_us_foreign_rollup.expected.json`
/// (issue #276 corpus ground truth). The fixture's worked example is
/// `(S//NF) (//DEU TS//REL TO USA, DEU)`; this test uses the same
/// shape with `DEU` to mirror the corpus exactly.
///
/// Citation re-verified against `crates/capco/docs/CAPCO-2016.md` at
/// authorship 2026-05-19 (§H.7 pp123-129; p129 worked example).
#[test]
fn us_reciprocates_equivalent_protection_for_foreign_portion() {
    // Portion 1: (S//NF) — US Secret + NOFORN (the lower-classification
    // US side; the worked example's "US portion").
    let mut us_portion = portion_us(Classification::Secret);
    us_portion.dissem_us = vec![DissemControl::Nf].into_boxed_slice();

    // Portion 2: (//DEU TS//REL TO USA, DEU) — German Top Secret FGI
    // with REL TO. Mirrors the #276 fixture verbatim.
    let mut fgi_portion = CanonicalAttrs::default();
    fgi_portion.classification = Some(MarkingClassification::Fgi(FgiClassification {
        level: Classification::TopSecret,
        countries: Box::new([cc("DEU")]),
    }));
    fgi_portion.rel_to = vec![cc("USA"), cc("DEU")].into_boxed_slice();

    let portions = [us_portion, fgi_portion];
    let projected = project_via_scheme(&portions);

    // US reciprocates equivalent protection: classification raises to
    // TopSecret (the foreign-side equivalent-US level per §H.7 pp123-125).
    assert_eq!(
        projected.classification,
        Some(MarkingClassification::Us(Classification::TopSecret)),
        "US must reciprocate equivalent protection: classification raises \
         to Us(TopSecret), got {:?}",
        projected.classification
    );

    // FGI marker captures DEU as the foreign provenance.
    match &projected.fgi_marker {
        Some(FgiMarker::Acknowledged { countries, .. }) => {
            let set: std::collections::BTreeSet<CountryCode> = countries.iter().copied().collect();
            assert!(
                set.contains(&cc("DEU")),
                "FGI marker must contain DEU, got {set:?}"
            );
        }
        other => panic!("expected Acknowledged FGI marker, got {other:?}"),
    }

    // NOFORN survives.
    assert!(
        projected.dissem_us.contains(&DissemControl::Nf),
        "NOFORN must survive on page, got {:?}",
        projected.dissem_us
    );

    // REL TO is cleared by the `capco/noforn-clears-rel-to` PageRewrite
    // per §H.8 p145.
    assert!(
        projected.rel_to.is_empty(),
        "REL TO must be cleared by capco/noforn-clears-rel-to, got {:?}",
        projected.rel_to
    );
}
