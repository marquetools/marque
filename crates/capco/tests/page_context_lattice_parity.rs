// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 4b-B Commit 8 — PageContext-vs-lattice parity gate (006 T112).
//!
//! Synthetic-fixture parity matrix comparing the post-fix PageContext
//! path (`PageContext::add_portion` + `page_context_to_attrs`) against
//! the new lattice path (`CapcoMarking::join_via_lattice`).
//!
//! The two paths MUST produce byte-identical `CanonicalAttrs` on every
//! axis EXCEPT the deliberate divergences documented inline below.
//! Each divergence carries a `§X.Y pNN` citation re-verified
//! 2026-05-15 against `crates/capco/docs/CAPCO-2016.md`.
//!
//! ## Why synthetic instead of corpus fixtures
//!
//! The corpus fixtures in `tests/corpus/valid/` are designed to
//! exercise the strict-recognizer + rule pipeline end-to-end. The
//! parity gate's job is to compare TWO projection paths from
//! pre-parsed per-portion `CanonicalAttrs` values. Synthetic
//! `CanonicalAttrs` fixtures hand-built in this file cover the
//! specific axes PR 4b-B touches with full control over the input
//! shape; the strict-recognizer is orthogonal to the parity claim.
//!
//! PR 4b-D will widen this gate to corpus fixtures when
//! `CapcoScheme::project(Scope::Page, ...)` flips to use the lattice
//! path.

use marque_capco::CapcoMarking;
use marque_ism::{
    AeaMarking, CanonicalAttrs, Classification, CountryCode, DissemControl, FgiClassification,
    FgiMarker, JointClassification, MarkingClassification, NatoClassification, NonIcDissem,
    PageContext,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn cc(s: &str) -> CountryCode {
    CountryCode::try_new(s.as_bytes()).expect("valid trigraph")
}

/// Build the PageContext-path projection.
fn project_via_page_context(portions: &[CanonicalAttrs]) -> CanonicalAttrs {
    let mut ctx = PageContext::new();
    for p in portions {
        ctx.add_portion(p.clone());
    }
    let mut out = CanonicalAttrs::default();
    out.classification = ctx.expected_classification().map(MarkingClassification::Us);
    out.sci_controls = ctx.expected_sci_controls().into_boxed_slice();
    out.sci_markings = ctx.expected_sci_markings();
    out.sar_markings = ctx.expected_sar_marking();
    out.aea_markings = ctx.expected_aea_markings().into_boxed_slice();
    out.fgi_marker = ctx.expected_fgi_marker();
    out.dissem_us = ctx.expected_dissem_us().into_boxed_slice();
    out.dissem_nato = ctx.expected_dissem_nato().into_boxed_slice();
    out.rel_to = ctx.expected_rel_to().into_boxed_slice();
    out.declassify_on = ctx.expected_declassify_on().cloned();
    out.declass_exemption = ctx.expected_declass_exemption();
    let (non_ic, _needs_nf) = ctx.expected_non_ic_dissem();
    out.non_ic_dissem = non_ic.into_boxed_slice();
    out
}

fn project_via_lattice(portions: &[CanonicalAttrs]) -> CanonicalAttrs {
    CapcoMarking::join_via_lattice(portions)
}

/// Assert byte-identity on every axis present on both sides except
/// the named divergence list.
fn assert_byte_identity(
    fixture_id: &str,
    pc: &CanonicalAttrs,
    lat: &CanonicalAttrs,
    expected_divergences: &[&str],
) {
    let mut diffs = Vec::new();

    macro_rules! check_eq {
        ($field:ident, $name:literal) => {
            if pc.$field != lat.$field {
                diffs.push(($name, format!("pc={:?}, lat={:?}", pc.$field, lat.$field)));
            }
        };
    }

    check_eq!(classification, "classification");
    check_eq!(sci_controls, "sci_controls");
    check_eq!(sci_markings, "sci_markings");
    check_eq!(sar_markings, "sar_markings");
    check_eq!(aea_markings, "aea_markings");
    check_eq!(fgi_marker, "fgi_marker");
    check_eq!(dissem_us, "dissem_us");
    check_eq!(dissem_nato, "dissem_nato");
    check_eq!(non_ic_dissem, "non_ic_dissem");
    check_eq!(rel_to, "rel_to");
    check_eq!(declassify_on, "declassify_on");
    check_eq!(declass_exemption, "declass_exemption");

    let unexpected: Vec<&(&str, String)> = diffs
        .iter()
        .filter(|(name, _)| !expected_divergences.contains(name))
        .collect();
    let missing: Vec<&&str> = expected_divergences
        .iter()
        .filter(|name| !diffs.iter().any(|(n, _)| n == *name))
        .collect();

    assert!(
        unexpected.is_empty(),
        "fixture {fixture_id}: unexpected divergences: {unexpected:?}"
    );
    assert!(
        missing.is_empty(),
        "fixture {fixture_id}: expected divergences did not occur: {missing:?}"
    );
}

fn portion_us(level: Classification) -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(level));
    a
}

fn portion_with_dissem_us(level: Classification, dissem: &[DissemControl]) -> CanonicalAttrs {
    let mut a = portion_us(level);
    a.dissem_us = dissem.to_vec().into_boxed_slice();
    a
}

fn portion_with_rel_to(level: Classification, rel: &[&str]) -> CanonicalAttrs {
    let mut a = portion_us(level);
    a.rel_to = rel
        .iter()
        .map(|s| cc(s))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    a
}

fn portion_joint(level: Classification, producers: &[&str]) -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    let countries: Box<[CountryCode]> = producers
        .iter()
        .map(|s| cc(s))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    a.classification = Some(MarkingClassification::Joint(JointClassification {
        level,
        countries,
    }));
    a
}

// ===========================================================================
// OC-USGOV — 6 cases
// ===========================================================================

#[test]
fn oc_usgov_one_orcon_many_usgov() {
    // §H.8 p136 + p140: ORCON dominates ORCON-USGOV.
    let portions = [
        portion_with_dissem_us(Classification::Secret, &[DissemControl::Oc]),
        portion_with_dissem_us(
            Classification::Secret,
            &[DissemControl::Oc, DissemControl::OcUsgov],
        ),
        portion_with_dissem_us(
            Classification::Secret,
            &[DissemControl::Oc, DissemControl::OcUsgov],
        ),
    ];
    assert_byte_identity(
        "oc_usgov_one_orcon_many_usgov",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn oc_usgov_many_orcon_one_usgov() {
    let portions = [
        portion_with_dissem_us(
            Classification::Secret,
            &[DissemControl::Oc, DissemControl::OcUsgov],
        ),
        portion_with_dissem_us(Classification::Secret, &[DissemControl::Oc]),
        portion_with_dissem_us(Classification::Secret, &[DissemControl::Oc]),
    ];
    assert_byte_identity(
        "oc_usgov_many_orcon_one_usgov",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn oc_usgov_pure_oc() {
    let portions = [
        portion_with_dissem_us(Classification::Secret, &[DissemControl::Oc]),
        portion_with_dissem_us(Classification::Secret, &[DissemControl::Oc]),
    ];
    assert_byte_identity(
        "oc_usgov_pure_oc",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn oc_usgov_pure_usgov() {
    let portions = [
        portion_with_dissem_us(Classification::Secret, &[DissemControl::OcUsgov]),
        portion_with_dissem_us(Classification::Secret, &[DissemControl::OcUsgov]),
    ];
    assert_byte_identity(
        "oc_usgov_pure_usgov",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn oc_usgov_no_oc_no_usgov() {
    let portions = [portion_us(Classification::Secret)];
    assert_byte_identity(
        "oc_usgov_no_oc_no_usgov",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn oc_usgov_mix() {
    let portions = [
        portion_with_dissem_us(Classification::Secret, &[DissemControl::Oc]),
        portion_with_dissem_us(Classification::Secret, &[DissemControl::OcUsgov]),
    ];
    assert_byte_identity(
        "oc_usgov_mix",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

// ===========================================================================
// RELIDO observed-unanimity — 4 cases
// ===========================================================================

#[test]
fn relido_unanimous_all_portions() {
    // §H.8 pp155-156: RELIDO unanimous → banner gets RELIDO.
    let portions = [
        portion_with_dissem_us(Classification::Secret, &[DissemControl::Relido]),
        portion_with_dissem_us(Classification::Secret, &[DissemControl::Relido]),
        portion_with_dissem_us(Classification::Secret, &[DissemControl::Relido]),
    ];
    assert_byte_identity(
        "relido_unanimous_all_portions",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn relido_mixed_drops() {
    // §H.8 pp155-156: 2 of 3 portions → drop RELIDO from banner.
    let portions = [
        portion_with_dissem_us(Classification::Secret, &[DissemControl::Relido]),
        portion_with_dissem_us(Classification::Secret, &[DissemControl::Relido]),
        portion_us(Classification::Secret),
    ];
    assert_byte_identity(
        "relido_mixed_drops",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn relido_single_portion_with_relido_drops() {
    // Single non-RELIDO portion + single RELIDO portion. Not
    // unanimous (the bare portion doesn't carry RELIDO).
    let portions = [
        portion_with_dissem_us(Classification::Secret, &[DissemControl::Relido]),
        portion_us(Classification::Secret),
    ];
    assert_byte_identity(
        "relido_single_portion_with_relido_drops",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn relido_plus_nf_noforn_dominates_documented_divergence() {
    // DIVERGENCE: §D.2 Table 3 row 2 + §H.8 p145: "NOFORN cannot be
    // used with REL TO / RELIDO / DISPLAY ONLY." The lattice path
    // correctly drops RELIDO when NOFORN is present (DissemSet
    // overlay 3 — NOFORN dominates). The PageContext path keeps
    // both — its `expected_dissem_us` does a plain union without
    // the supersession overlay.
    //
    // This is a deliberate parity divergence the lattice path
    // CORRECTS. PageContext is bug-shaped here per §H.8 p145; the
    // fix migrates to the lattice path when PR 4b-D flips
    // `CapcoScheme::project(Scope::Page, ...)`.
    //
    // Citation: §D.2 Table 3 rows 1-2 + §H.8 p145 (verified
    // 2026-05-15 against CAPCO-2016.md).
    let portions = [
        portion_with_dissem_us(
            Classification::Secret,
            &[DissemControl::Nf, DissemControl::Relido],
        ),
        portion_with_dissem_us(
            Classification::Secret,
            &[DissemControl::Nf, DissemControl::Relido],
        ),
    ];
    let pc = project_via_page_context(&portions);
    let lat = project_via_lattice(&portions);
    // PageContext keeps both NF + Relido (incorrect per §H.8 p145).
    assert!(pc.dissem_us.contains(&DissemControl::Nf));
    assert!(pc.dissem_us.contains(&DissemControl::Relido));
    // Lattice path correctly drops Relido (per §H.8 p145 supersession).
    assert!(lat.dissem_us.contains(&DissemControl::Nf));
    assert!(
        !lat.dissem_us.contains(&DissemControl::Relido),
        "lattice path must drop Relido when NOFORN present per §H.8 p145"
    );
}

// ===========================================================================
// REL TO trigraph intersection + tetragraph + USA-first — 4 cases
// ===========================================================================

#[test]
fn rel_to_intersect_common() {
    // §H.8 p152 worked example.
    let portions = [
        portion_with_rel_to(Classification::Secret, &["USA", "GBR", "CAN"]),
        portion_with_rel_to(Classification::Secret, &["USA", "GBR", "AUS"]),
    ];
    assert_byte_identity(
        "rel_to_intersect_common",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn rel_to_intersect_empty() {
    // §D.2 Table 3 row 9: no-common-LIST → NOFORN (post-projection
    // PageRewrite). Both paths produce empty rel_to. The §D.2 Table
    // 3 row 9 NOFORN injection is a project() concern, not a
    // per-axis lattice concern — both paths leave dissem alone here.
    //
    // G-7 (PR 4b-B follow-up): strengthen to full byte-identity
    // rather than only checking the `rel_to` axis; a regression
    // touching `dissem_us` / classification / FGI would have slipped
    // through the prior partial assertion.
    let portions = [
        portion_with_rel_to(Classification::Secret, &["GBR", "CAN"]),
        portion_with_rel_to(Classification::Secret, &["FRA", "DEU"]),
    ];
    assert_byte_identity(
        "rel_to_intersect_empty",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn rel_to_tetragraph_fvey() {
    let portions = [
        portion_with_rel_to(Classification::Secret, &["FVEY"]),
        portion_with_rel_to(Classification::Secret, &["USA", "GBR", "CAN"]),
    ];
    assert_byte_identity(
        "rel_to_tetragraph_fvey",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn rel_to_usa_first_sort() {
    // §H.8 p151: USA first, rest alphabetical.
    let portions = [
        portion_with_rel_to(Classification::Secret, &["GBR", "USA", "CAN"]),
        portion_with_rel_to(Classification::Secret, &["GBR", "USA", "CAN"]),
    ];
    assert_byte_identity(
        "rel_to_usa_first_sort",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

// ===========================================================================
// JOINT — 5 cases (with documented divergences for disunity collapse)
// ===========================================================================

#[test]
fn joint_unanimous_two_portions() {
    let portions = [
        portion_joint(Classification::Secret, &["USA", "GBR"]),
        portion_joint(Classification::Secret, &["USA", "GBR"]),
    ];
    // The PageContext path uses expected_classification (Us-only) and
    // expected_fgi_marker (folds JOINT non-US producers into FGI).
    // The lattice path uses JointSet::UnanimousProducers to emit a
    // proper Joint(_) classification. This is a deliberate divergence
    // that fixes a §H.3 p56 banner-fidelity gap on the PageContext
    // path. Once the renderer trait surface lands (PR 5+ Stage 4),
    // both paths will produce //JOINT SECRET USA, GBR.
    let pc = project_via_page_context(&portions);
    let lat = project_via_lattice(&portions);
    // PageContext path: classification = Us(Secret); fgi_marker
    // = Some(Acknowledged{GBR}).
    // Lattice path: classification = Joint(S, [USA, GBR]); fgi_marker
    // = None or PageContext fallback.
    // We assert the lattice path produces the §H.3 p56-correct shape:
    assert!(
        matches!(lat.classification, Some(MarkingClassification::Joint(_))),
        "lattice should produce Joint classification on unanimous JOINT"
    );
    // PageContext path's known behavior — Us classification at
    // banner per §H.3 p57 ("JOINT not carried forward in
    // US documents") — applies here because PageContext doesn't
    // distinguish pure-JOINT-page from JOINT-with-US-page. The
    // §H.3 p56 "JOINT [class] [LIST]" banner form is a
    // pure-JOINT-page concern that PR 4b-B's JointSet correctly
    // models. Divergence acceptable; the §H.3 p56 + §H.3 p57 line
    // 1288 citations document the asymmetry.
    assert!(matches!(
        pc.classification,
        Some(MarkingClassification::Us(_))
    ));
}

#[test]
fn joint_mixed_with_us_returns_bottom() {
    // §H.3 p57: mixed (JOINT + US) → JOINT does not roll
    // up. Both paths should produce a US-classification banner.
    let portions = [
        portion_joint(Classification::Secret, &["USA", "GBR"]),
        portion_us(Classification::Secret),
    ];
    assert_byte_identity(
        "joint_mixed_with_us_returns_bottom",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn joint_disunity_two_portions_different_producers() {
    // §H.3 p56 + §H.7 p123: disunity → JOINT drops, non-US producers
    // migrate to FGI. The W004 Warn rule (registered in Commit 9)
    // surfaces this transformation.
    //
    // G-7 (PR 4b-B follow-up): strengthen to assert the EXACT
    // producer set on both `fgi_marker` values rather than just
    // "some FGI marker present" — a regression that lost a producer
    // would have slipped through.
    //
    // Both paths produce Us(Secret) classification and FGI marker
    // carrying { GBR, CAN } (the union of non-US producers across
    // disunity-collapse portions).
    let portions = [
        portion_joint(Classification::Secret, &["USA", "GBR"]),
        portion_joint(Classification::Secret, &["USA", "CAN"]),
    ];
    let pc = project_via_page_context(&portions);
    let lat = project_via_lattice(&portions);
    assert_eq!(
        pc.classification,
        Some(MarkingClassification::Us(Classification::Secret))
    );
    assert_eq!(
        lat.classification,
        Some(MarkingClassification::Us(Classification::Secret))
    );
    let expected_producers = {
        let mut s = std::collections::BTreeSet::new();
        s.insert(cc("CAN"));
        s.insert(cc("GBR"));
        s
    };
    let extract_producers = |m: &Option<marque_ism::FgiMarker>| {
        use marque_ism::FgiMarker;
        match m {
            Some(FgiMarker::Acknowledged { countries, .. }) => countries
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(
            ),
            Some(FgiMarker::SourceConcealed) => std::collections::BTreeSet::new(),
            None => std::collections::BTreeSet::new(),
        }
    };
    assert_eq!(
        extract_producers(&pc.fgi_marker),
        expected_producers,
        "PageContext FGI producer set must be {{CAN, GBR}}"
    );
    assert_eq!(
        extract_producers(&lat.fgi_marker),
        expected_producers,
        "Lattice FGI producer set must be {{CAN, GBR}}"
    );
}

#[test]
fn joint_classification_pure_us_no_joint() {
    let portions = [
        portion_us(Classification::Secret),
        portion_us(Classification::TopSecret),
    ];
    assert_byte_identity(
        "joint_classification_pure_us_no_joint",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn joint_single_portion_no_us() {
    // A solitary JOINT portion (pure-JOINT page). The lattice
    // path's JointSet::UnanimousProducers fires and produces
    // Joint(_) classification. The PageContext path produces
    // Us(_) per its existing semantic. Documented divergence per
    // §H.3 p56.
    let portions = [portion_joint(Classification::Secret, &["USA", "GBR"])];
    let pc = project_via_page_context(&portions);
    let lat = project_via_lattice(&portions);
    assert!(matches!(
        pc.classification,
        Some(MarkingClassification::Us(_))
    ));
    assert!(matches!(
        lat.classification,
        Some(MarkingClassification::Joint(_))
    ));
}

// ===========================================================================
// Classification + level promotion — 2 cases
// ===========================================================================

#[test]
fn classification_max_promotes() {
    let portions = [
        portion_us(Classification::Confidential),
        portion_us(Classification::Secret),
        portion_us(Classification::TopSecret),
    ];
    assert_byte_identity(
        "classification_max_promotes",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn classification_single_portion() {
    let portions = [portion_us(Classification::Secret)];
    assert_byte_identity(
        "classification_single_portion",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

// ===========================================================================
// NOFORN + REL TO supersession — 1 case
// ===========================================================================

#[test]
fn noforn_clears_rel_to() {
    // §H.8 p145: NOFORN clears REL TO.
    let mut nf_portion = portion_us(Classification::Secret);
    nf_portion.dissem_us = vec![DissemControl::Nf].into_boxed_slice();
    let portions = [
        portion_with_rel_to(Classification::Secret, &["USA", "GBR"]),
        nf_portion,
    ];
    assert_byte_identity(
        "noforn_clears_rel_to",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

// ===========================================================================
// NODIS clears REL TO — 1 case
// ===========================================================================

#[test]
fn nodis_clears_rel_to() {
    // §H.9 p174: NODIS clears REL TO.
    let mut nodis_portion = portion_us(Classification::Secret);
    nodis_portion.non_ic_dissem = vec![NonIcDissem::Nodis].into_boxed_slice();
    let portions = [
        portion_with_rel_to(Classification::Secret, &["USA", "GBR"]),
        nodis_portion,
    ];
    let pc = project_via_page_context(&portions);
    let lat = project_via_lattice(&portions);
    // Both paths: rel_to is cleared by the NODIS supersession.
    assert!(pc.rel_to.is_empty());
    assert!(lat.rel_to.is_empty());
    // PageContext path: needs_nf is true → NOFORN injected into
    // expected_dissem_us. Lattice path: NofornSuperseded fires →
    // NOFORN injected into the lattice's dissem_us output. Both
    // should contain NOFORN.
    assert!(pc.dissem_us.contains(&DissemControl::Nf));
    assert!(lat.dissem_us.contains(&DissemControl::Nf));
}

// ===========================================================================
// PR 4b-B follow-up parity divergences — documented w/ citation
// ===========================================================================

#[test]
fn fouo_classified_lattice_vs_pagecontext_diverges() {
    // G-1 (PR 4b-B follow-up) documented divergence: a classified
    // page with a FOUO portion. PageContext::expected_dissem_us
    // drops FOUO (§H.8 p134 — FOUO is U-only); the lattice path's
    // DissemSet does NOT apply the cross-axis FOUO eviction (that
    // logic stays on PageContext under the "Constraint::Custom
    // capco/fouo-eviction" migration target — see the §3.3 plan
    // text and project memory `project_noforn_supremacy_composition.md`
    // Pattern B). This divergence is RESOLVED in PR 4b-C, not here.
    //
    // Citation: §H.8 p134 (FOUO classification gate) +
    // project memory `project_noforn_supremacy_composition.md`
    // Pattern B (PR 4b-C scope).
    let portions = [portion_with_dissem_us(
        Classification::Secret,
        &[DissemControl::Fouo],
    )];
    let pc = project_via_page_context(&portions);
    let lat = project_via_lattice(&portions);
    assert!(
        !pc.dissem_us.contains(&DissemControl::Fouo),
        "PageContext drops FOUO on classified page per §H.8 p134"
    );
    assert!(
        lat.dissem_us.contains(&DissemControl::Fouo),
        "Lattice path keeps FOUO until PR 4b-C ships the cross-axis \
         FOUO-eviction rewrite per §H.8 p134"
    );
}

#[test]
fn aea_ucni_classified_lattice_vs_pagecontext_diverges() {
    // G-2 (PR 4b-B follow-up) documented divergence: a classified
    // page with DOD UCNI. PageContext::expected_aea_markings strips
    // UCNI (§H.6 p116 + p118 — UCNI is U-only); the lattice path's
    // `AeaSet::to_markings` does NOT apply the classification gate
    // (Pattern C in `project_noforn_supremacy_composition.md`; PR
    // 4b-C migration target).
    //
    // Citation: §H.6 p116 (DOD UCNI) + §H.6 p118 (DOE UCNI).
    let mut p = portion_us(Classification::Secret);
    p.aea_markings = vec![AeaMarking::DodUcni].into_boxed_slice();
    let portions = [p];
    let pc = project_via_page_context(&portions);
    let lat = project_via_lattice(&portions);
    assert!(
        !pc.aea_markings
            .iter()
            .any(|m| matches!(m, AeaMarking::DodUcni)),
        "PageContext strips DOD UCNI on classified page per §H.6 p116"
    );
    assert!(
        lat.aea_markings
            .iter()
            .any(|m| matches!(m, AeaMarking::DodUcni)),
        "Lattice path keeps DOD UCNI until PR 4b-C ships the cross-axis \
         classification-gate rewrite per §H.6 p116"
    );
}

#[test]
fn pure_nato_lattice_vs_pagecontext_diverges() {
    // G-3 (PR 4b-B follow-up) documented divergence: a SOLELY-NATO
    // page (no US portion). PageContext::expected_classification
    // always returns `Us(_)` (it flattens variants); the lattice
    // path preserves the `Nato(_)` variant per §H.7 pp123-125
    // reciprocal normalization, which is "Us-equivalent at portion-
    // parse time when ANY US portion is present, but the non-US
    // variant survives at banner when the page has no US
    // contribution."
    //
    // G-3 sharper framing: when a page has even one US portion in
    // scope, the lattice path now flattens NATO/FGI to
    // `Us(effective_level)` (matching PageContext), so the
    // divergence is scoped to truly solely-non-US pages.
    //
    // Citation: §H.7 pp123-125 (reciprocal-raise rule).
    let mut nato_portion = CanonicalAttrs::default();
    nato_portion.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
    let portions = [nato_portion];
    let pc = project_via_page_context(&portions);
    let lat = project_via_lattice(&portions);
    assert!(
        matches!(pc.classification, Some(MarkingClassification::Us(_))),
        "PageContext flattens non-US to Us(_) at banner"
    );
    assert!(
        matches!(lat.classification, Some(MarkingClassification::Nato(_))),
        "Lattice preserves Nato variant on solely-NATO page per §H.7 pp123-125"
    );
}

#[test]
fn mixed_us_plus_nato_lattice_flattens_to_us() {
    // G-3 sharper-framing follow-up: mixed US+NATO must flatten the
    // NATO variant to Us(effective_level) on both paths. This is
    // the §H.7 reciprocal-raise rule applied correctly — when any
    // US portion is in scope, the banner classification surface is
    // US, not NATO.
    let mut nato_portion = CanonicalAttrs::default();
    nato_portion.classification = Some(MarkingClassification::Nato(
        NatoClassification::CosmicTopSecret,
    ));
    let portions = [portion_us(Classification::Secret), nato_portion];
    assert_byte_identity(
        "mixed_us_plus_nato_lattice_flattens_to_us",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn classified_sbu_nf_injects_noforn_and_clears_rel_to() {
    // G-6 (PR 4b-B follow-up): classified page with a portion
    // carrying SBU-NF (or LES-NF) and another carrying REL TO must
    // produce a banner with NOFORN in dissem_us AND empty rel_to.
    // Pre-fix the lattice path discarded the `needs_nf` flag from
    // `expected_non_ic_dissem` and kept REL TO + missed NOFORN.
    //
    // Citation: §H.9 p178 (SBU-NF on classified pages) +
    // §H.9 p185 (LES-NF on classified pages) — both inject NOFORN
    // and supersede REL TO when commingled.
    let mut sbunf = portion_us(Classification::Secret);
    sbunf.non_ic_dissem = vec![NonIcDissem::SbuNf].into_boxed_slice();
    let portions = [
        portion_with_rel_to(Classification::Secret, &["USA", "GBR"]),
        sbunf,
    ];
    let pc = project_via_page_context(&portions);
    let lat = project_via_lattice(&portions);
    // Both paths inject NOFORN.
    assert!(
        pc.dissem_us.contains(&DissemControl::Nf),
        "PageContext injects NF"
    );
    assert!(
        lat.dissem_us.contains(&DissemControl::Nf),
        "Lattice injects NF (G-6)"
    );
    // Both paths clear REL TO.
    assert!(pc.rel_to.is_empty(), "PageContext clears REL TO");
    assert!(lat.rel_to.is_empty(), "Lattice clears REL TO (G-6)");
}

#[test]
fn joint_unanimous_does_not_double_mark_with_fgi() {
    // G-4 (PR 4b-B follow-up): JointSet::UnanimousProducers carries
    // the producer list in the JOINT classification itself. The
    // lattice path must NOT additionally FGI-mark those same
    // producers, because §H.3 p56 + §H.7 p123 say JOINT subsumes
    // the FGI marker for the JOINT producer list.
    //
    // Citation: §H.3 p56 (JOINT grammar — producer list is on the
    // JOINT marking) + §H.7 p123 (JOINT subsumes FGI for the same
    // producers).
    let portions = [
        portion_joint(Classification::Secret, &["USA", "GBR"]),
        portion_joint(Classification::Secret, &["USA", "GBR"]),
    ];
    let lat = project_via_lattice(&portions);
    assert!(
        matches!(lat.classification, Some(MarkingClassification::Joint(_))),
        "lattice should produce Joint classification on unanimous JOINT"
    );
    assert!(
        lat.fgi_marker.is_none(),
        "lattice must NOT double-mark JOINT producers as FGI per §H.3 p56 + §H.7 p123"
    );
}

#[test]
fn explicit_fgi_marker_merges_with_classification_derived_producers() {
    // G-5 (PR 4b-B follow-up): when both an explicit FGI marker AND
    // classification-derived FGI producers are present, both sets
    // must appear in the final FGI marker. Pre-fix, the
    // `(Some(_), _) => fgi_acc.to_marker()` match arm preferred the
    // explicit marker wholesale and lost the classification-derived
    // producers.
    //
    // Citation: §H.7 p123 (FGI source-acknowledged form unions all
    // foreign sources observed on the page).
    let mut p1 = CanonicalAttrs::default();
    p1.classification = Some(MarkingClassification::Us(Classification::Secret));
    p1.fgi_marker = Some(FgiMarker::acknowledged([cc("FRA")]).unwrap());
    // Second portion carries an Fgi classification, which the
    // PageContext path folds into the FGI axis as a producer.
    let mut p2 = CanonicalAttrs::default();
    p2.classification = Some(MarkingClassification::Fgi(FgiClassification {
        level: Classification::Secret,
        countries: Box::new([cc("DEU")]),
    }));
    let portions = [p1, p2];
    let lat = project_via_lattice(&portions);
    // Lattice must union FRA (explicit) + DEU (classification-derived).
    if let Some(FgiMarker::Acknowledged { countries, .. }) = lat.fgi_marker {
        let set: std::collections::BTreeSet<CountryCode> = countries.iter().copied().collect();
        assert!(
            set.contains(&cc("FRA")),
            "lattice must include explicit FGI producer FRA"
        );
        assert!(
            set.contains(&cc("DEU")),
            "lattice must include classification-derived FGI producer DEU (G-5)"
        );
    } else {
        panic!("expected Acknowledged FGI marker, got {:?}", lat.fgi_marker);
    }
}
