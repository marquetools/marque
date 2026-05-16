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
    FgiMarker, ForeignClassification, JointClassification, MarkingClassification,
    NatoClassification, NonIcDissem, PageContext,
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
    // DISPLAY ONLY axis: both projection paths produce Box<[]> today
    // because PageContext deliberately defers the §D.2 Table 3 row 25-27
    // intersection roll-up to Phase 2 (see `page_context.rs:246-252`).
    // This check gates against future drift — if either path starts
    // emitting non-empty `display_only_to` the gate catches the
    // divergence. §H.8 p163 (DISPLAY ONLY template) + §D.2 p28-30
    // Table 3 rows 25-27 (roll-up rules). Verified 2026-05-16 against
    // `crates/capco/docs/CAPCO-2016.md`.
    check_eq!(display_only_to, "display_only_to");
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
fn relido_plus_nf_noforn_dominates_parity() {
    // PARITY (post-staging-convergence): §D.2 Table 3 row 2 + §H.8
    // p145: "NOFORN cannot be used with REL TO / RELIDO / DISPLAY
    // ONLY." Both paths now correctly drop RELIDO when NOFORN is
    // present.
    //
    // Pre-staging-convergence this was a documented divergence —
    // PageContext kept Relido via a plain union without the
    // supersession overlay, while the lattice path's `DissemSet`
    // overlay 3 dropped it per §H.8 p145. Staging's PageContext
    // changes (DISPLAY ONLY Phase 2 / page-rewrite
    // `capco/noforn-clears-fdr-family`) implemented the same
    // supersession on the PageContext side, restoring parity.
    //
    // Citation: §D.2 Table 3 rows 1-2 + §H.8 p145 (verified
    // 2026-05-16 against CAPCO-2016.md).
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
    assert_byte_identity(
        "relido_plus_nf_noforn_dominates_parity",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
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
    // PARITY (post-staging-convergence): §D.2 Table 3 row 9 states
    // "REL TO [USA, LIST] | REL TO [USA, LIST] (with no common [LIST]
    // value(s)) | NOFORN". When two REL TO portions share no common
    // country list, the result is NOFORN on the dissem_us axis.
    //
    // Pre-staging-convergence this was a documented divergence — the
    // lattice path (via RelToBlock::Empty → is_empty_intersection()
    // check in scheme.rs project()) injected NOFORN, while the
    // PageContext path did not. Staging's PageContext changes
    // (DISPLAY ONLY Phase 2 / page-rewrite `capco/noforn-clears-
    // fdr-family`) implemented the same NF-on-empty-REL-TO injection
    // path, restoring parity.
    //
    // Both paths now produce empty rel_to (correct — no common
    // members) AND dissem_us = [Nf] per §D.2 row 9.
    //
    // Citation: §D.2 p28-30 Table 3 row 9 (verified 2026-05-16 against
    // CAPCO-2016.md).
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
fn joint_mixed_with_us_returns_mixed() {
    // §H.3 p57: mixed (JOINT + US) → JOINT does not roll up. Both
    // paths should produce a US-classification banner. The JointSet
    // lattice returns the `Mixed` variant per C-3 (PR 4b-B follow-up
    // — split out of `Bottom` so the absorbing JOINT+non-JOINT state
    // keeps `join` associative).
    //
    // Rename history: `joint_mixed_with_us_returns_bottom` (pre-C-3,
    // when Mixed and Bottom were conflated) → M-17 PR 4b-B 6th-pass
    // renamed to `joint_mixed_with_us_returns_us_class_no_w004` to
    // spell out the banner-side assertion → M-21 PR 4b-B 7th-pass
    // shortens to `returns_mixed` to name the lattice variant
    // directly. The banner-side guarantee still rides on
    // `assert_byte_identity` and the W004-side guarantee on the
    // separate W004 rule tests in `joint_disunity_collapse.rs`.
    //
    // Mixed silences W004 by design (FGI migration rides through
    // `expected_fgi_marker`, not through W004's lattice signal).
    let portions = [
        portion_joint(Classification::Secret, &["USA", "GBR"]),
        portion_us(Classification::Secret),
    ];
    assert_byte_identity(
        "joint_mixed_with_us_returns_mixed",
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
fn lattice_classified_sbu_nf_with_displayonly_supersedes_displayonly() {
    // G-8 (PR 4b-B follow-up): a classified page with SBU-NF on one
    // portion AND DISPLAY ONLY on another must end up with NOFORN in
    // dissem_us, NOT DISPLAY ONLY. NOFORN dominates DISPLAY ONLY per
    // §H.8 p145 (NOFORN: "Cannot be used with REL TO / RELIDO /
    // EYES ONLY / DISPLAY ONLY") + §D.2 Table 3 rows 1-2.
    //
    // Pre-fix, G-6 injected `Nf` directly into `out.dissem_us` after
    // the supersession overlay had already run, so the `Nf` addition
    // never triggered `DissemSet`'s NOFORN-dominates step. The
    // classified page's lattice output wound up with `Nf + Displayonly`
    // together — invalid per §H.8 p145.
    //
    // This test asserts only the lattice path's correctness (the
    // PageContext path lacks a NOFORN-dominates step in
    // `expected_dissem_us` at all — that's a separate pre-existing
    // bug tracked outside this PR-4b-B follow-up scope).
    //
    // Citation: §H.8 p145 (NOFORN: "Cannot be used with REL TO /
    // RELIDO / EYES ONLY / DISPLAY ONLY") + §H.8 p163 (DISPLAY ONLY:
    // "Not with NOFORN") + §D.2 Table 3 rows 1-2 (NOFORN dominates).
    let mut sbunf = portion_us(Classification::Secret);
    sbunf.non_ic_dissem = vec![NonIcDissem::SbuNf].into_boxed_slice();
    let mut display_portion = portion_us(Classification::Secret);
    display_portion.dissem_us = vec![DissemControl::Displayonly].into_boxed_slice();
    let portions = [display_portion, sbunf];
    let lat = project_via_lattice(&portions);
    assert!(
        lat.dissem_us.contains(&DissemControl::Nf),
        "Lattice injects NF (G-8)"
    );
    assert!(
        !lat.dissem_us.contains(&DissemControl::Displayonly),
        "Lattice supersedes DISPLAY ONLY via NOFORN overlay (G-8): dissem_us = {:?}",
        lat.dissem_us
    );
}

#[test]
fn lattice_noforn_clears_rel_to_supersedes_displayonly() {
    // G-8 (PR 4b-B follow-up): when `rel_to_was_noforn_superseded`
    // fires (a portion carries NODIS or EXDIS) AND another portion
    // carries DISPLAY ONLY, the injected NOFORN must dominate
    // DISPLAY ONLY too. Pre-fix this same code path (line 510-521)
    // injected `Nf` without re-running the supersession overlay.
    //
    // Citation: §H.9 p172 (NODIS) + §H.9 p174 (EXDIS) inject NOFORN;
    // §H.8 p145 + §H.8 p163 (NOFORN dominates DISPLAY ONLY).
    let mut nodis_portion = portion_us(Classification::Secret);
    nodis_portion.non_ic_dissem = vec![NonIcDissem::Nodis].into_boxed_slice();
    let mut display_portion = portion_us(Classification::Secret);
    display_portion.dissem_us = vec![DissemControl::Displayonly].into_boxed_slice();
    let portions = [display_portion, nodis_portion];
    let lat = project_via_lattice(&portions);
    assert!(
        lat.dissem_us.contains(&DissemControl::Nf),
        "Lattice injects NF on NODIS (G-8)"
    );
    assert!(
        !lat.dissem_us.contains(&DissemControl::Displayonly),
        "Lattice supersedes DISPLAY ONLY via NOFORN overlay on NODIS (G-8): dissem_us = {:?}",
        lat.dissem_us
    );
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

// ===========================================================================
// G-9 (PR 4b-B follow-up) — Conflict participates in the solely-non-US gate
// ===========================================================================
//
// `MarkingClassification::Conflict { us, foreign }` carries an implicit US
// classification (`us: Classification`). `PageContext::expected_classification`
// uses `effective_level()` over every variant — including Conflict — and
// wraps the result in `Us(_)`. The lattice path's `join_via_lattice`
// gate-check at scheme.rs:334 originally only counted explicit
// `MarkingClassification::Us(_)` portions as US-bearing, so a page with
// a Conflict portion (or Conflict mixed with NATO/FGI) skipped the
// `solely_non_us = false` branch and the §H.7 pp123-125 reciprocal-raise
// to `Us(level)`. The lattice returned `Conflict(...)`, PageContext
// returned `Us(level)` → parity broke for Conflict inputs.
//
// G-9 closes the gap by treating Conflict as US-bearing in the
// `has_us_class` accumulation at scheme.rs:334.
//
// Citation: §H.7 pp123-125 (reciprocal-classification rule — same
// authority that motivated G-3 for explicit US+NATO/FGI mixes). The
// Conflict variant exists because of the same rule: it's the parser's
// way of recording "the source had two classification systems; the
// US side wins, upgraded to the greater level" (see
// `MarkingClassification::Conflict` doc comment in
// `crates/ism/src/attrs.rs:521-526`).
// ===========================================================================

fn portion_conflict(us_level: Classification, foreign: ForeignClassification) -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Conflict {
        us: us_level,
        foreign: Box::new(foreign),
    });
    a
}

#[test]
fn conflict_classification_flattens_to_us() {
    // G-9: a page with a single Conflict portion. PageContext returns
    // `Us(effective_level)`; the lattice path must do the same.
    let portions = [portion_conflict(
        Classification::TopSecret,
        ForeignClassification::Nato(NatoClassification::CosmicTopSecret),
    )];
    assert_byte_identity(
        "conflict_classification_flattens_to_us",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn conflict_plus_nato_flattens_to_us() {
    // G-9: Conflict + NATO. Conflict carries implicit US, so the page
    // is NOT solely-non-US; both the NATO portion and the Conflict
    // portion must reciprocal-raise to Us(effective_level).
    let mut nato_portion = CanonicalAttrs::default();
    nato_portion.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
    let portions = [
        portion_conflict(
            Classification::Secret,
            ForeignClassification::Nato(NatoClassification::NatoSecret),
        ),
        nato_portion,
    ];
    assert_byte_identity(
        "conflict_plus_nato_flattens_to_us",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn conflict_plus_us_flattens_to_us() {
    // G-9 baseline: Conflict + explicit Us. Both paths produce
    // Us(max_level). This was already passing pre-G-9 because
    // `has_us_class = true` from the explicit Us portion regardless of
    // Conflict; the test pins the established behavior so a future
    // regression on the mixed case names itself.
    let portions = [
        portion_conflict(
            Classification::Confidential,
            ForeignClassification::Fgi(FgiClassification {
                level: Classification::Confidential,
                countries: Box::new([cc("GBR")]),
            }),
        ),
        portion_us(Classification::Secret),
    ];
    assert_byte_identity(
        "conflict_plus_us_flattens_to_us",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

// ===========================================================================
// G-9b (PR 4b-B 7th-pass follow-up) — Joint participates in solely-non-US gate
// ===========================================================================
//
// Continuation of G-9. The original G-9 fix added `Conflict` to the
// `has_us_class` branch but missed `Joint`. JOINT classifications are
// US co-owned by definition: §H.3 p56 requires USA to be in the producer
// list. A JOINT portion is therefore US-bearing for the purposes of the
// `solely_non_us` gate. Without this, a mixed page like
// `JOINT S USA GBR + NATO C` keeps `solely_non_us=true` and the NATO
// classification survives into the lattice output as `Nato(_)` rather
// than reciprocal-raising to `Us(_)` per §H.7 pp123-125.
//
// This case reaches the gate because `JointSet::Mixed` returns `None`
// from `to_marking_classification`, which falls through to the gate
// branch at scheme.rs.
//
// Citation: §H.3 p56 (JOINT requires USA in producer list) +
// §H.7 pp123-125 (reciprocal-classification rule for mixed US +
// non-US pages).
// ===========================================================================

#[test]
fn joint_plus_nato_same_level_flattens_to_us() {
    // G-9b: JOINT S USA GBR + NATO S (NatoSecret).
    // Both portions at the same effective level (Secret), so the
    // ClassificationLattice's OrdMax does NOT pick a winner via level —
    // it falls into the same-level variant-rank tiebreak. Without the
    // gate fix, `solely_non_us=true` (Joint not counted as US-bearing),
    // so the NATO portion is NOT flattened to Us(Secret) and survives
    // as Nato(NatoSecret). The variant-rank tiebreak then prefers the
    // lower-rank variant — Us is rank 0, Joint flattens to Us(Secret)
    // → Us wins, but the OUTPUT classification is Us(Secret) only if
    // the variant-rank picks Us. Without the gate fix, the lattice
    // variant-rank tiebreak would pick Us anyway because we DID
    // flatten Joint → Us in the per-portion loop. So this case still
    // passes... unless we make the JOINT portion's level lower than
    // NATO. Let's invert: JOINT C + NATO S — JOINT is at lower level
    // so its flattened Us(C) loses to Nato(NS) on level (NatoSecret's
    // us_equivalent is Secret). With gate fix, Nato is reciprocal-
    // raised to Us(Secret); lattice picks Us(Secret). Without gate
    // fix, Nato stays as Nato(NS); lattice picks Nato(NS). PageContext
    // returns Us(Secret) regardless. Divergence.
    let mut nato_portion = CanonicalAttrs::default();
    nato_portion.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
    let portions = [
        portion_joint(Classification::Confidential, &["USA", "GBR"]),
        nato_portion,
    ];
    assert_byte_identity(
        "joint_plus_nato_same_level_flattens_to_us",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn joint_plus_fgi_same_level_flattens_to_us() {
    // G-9b: JOINT C USA GBR + FGI S [FRA]. JOINT flattens to Us(C);
    // FGI at higher level. Without gate fix, FGI stays as
    // Fgi(S, [FRA]); lattice picks Fgi(S, [FRA]) via OrdMax (FGI
    // higher level wins). PageContext returns Us(Secret). Divergence.
    let mut fgi_portion = CanonicalAttrs::default();
    fgi_portion.classification = Some(MarkingClassification::Fgi(FgiClassification {
        level: Classification::Secret,
        countries: Box::new([cc("FRA")]),
    }));
    let portions = [
        portion_joint(Classification::Confidential, &["USA", "GBR"]),
        fgi_portion,
    ];
    assert_byte_identity(
        "joint_plus_fgi_same_level_flattens_to_us",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

// ===========================================================================
// G-4b (PR 4b-B 7th-pass follow-up) — FGI suppression for solely-non-US
// classifications carrying their own foreign-source info
// ===========================================================================
//
// Continuation of G-4. G-4 suppressed `expected_fgi_marker()` only when
// `joint_set == UnanimousProducers`. But solely-non-US pages classified
// with `Nato(_)` or `Fgi(_)` ALSO carry foreign-source info on the
// classification axis itself, and the lattice path's
// `expected_fgi_marker()` fallback derives the SAME producers from the
// classification — producing a doubled FGI marker that doesn't match
// PageContext's behavior on solely-NATO / solely-FGI pages.
//
// The fix: when the lattice is preserving the non-US variant intact
// (i.e., `solely_non_us=true` AND the classification is Nato or Fgi),
// suppress the `expected_fgi_marker()` fallback so the FGI axis is
// purely sourced from per-portion `fgi_marker` fields (via FgiSet) and
// is NOT duplicated from the classification.
//
// PageContext path on a solely-NATO page: `expected_classification`
// returns `Us(level)` (always wraps in Us regardless of source), and
// `expected_fgi_marker` returns `Some(Acknowledged{[NATO]})`. The
// lattice path on a solely-NATO page returns `Nato(_)` AND
// `Some(Acknowledged{[NATO]})` — the latter is double-marking.
//
// This is a documented divergence in the existing parity gate
// (`pure_nato_lattice_vs_pagecontext_diverges`). The new test below
// asserts the lattice does NOT double-mark — the solo-non-US case
// keeps the foreign source on the classification axis and leaves
// `fgi_marker` empty on solely-non-US pages where FGI semantics
// already ride on the classification.
//
// Citation: §H.7 p123 (FGI source-acknowledged form — the foreign
// source is recorded ONCE per portion; for non-US classifications
// the source is the classification itself).
// ===========================================================================

#[test]
fn solely_nato_does_not_double_mark_fgi() {
    // G-4b: pure-NATO page. Lattice path correctly preserves the
    // Nato(_) classification per §H.7 pp123-125 (documented divergence
    // — see `pure_nato_lattice_vs_pagecontext_diverges`). The FGI
    // axis must NOT additionally receive an Acknowledged marker for
    // the NATO source — that information is already on the
    // classification axis.
    let mut nato_portion = CanonicalAttrs::default();
    nato_portion.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
    let portions = [nato_portion];
    let lat = project_via_lattice(&portions);
    assert!(
        matches!(lat.classification, Some(MarkingClassification::Nato(_))),
        "lattice preserves Nato classification on solely-NATO page (G-3 divergence)"
    );
    assert!(
        lat.fgi_marker.is_none(),
        "G-4b: lattice must NOT double-mark NATO as FGI on a solely-non-US page; \
         the foreign source is already on the classification axis. \
         fgi_marker = {:?}",
        lat.fgi_marker
    );
}

#[test]
fn solely_fgi_does_not_double_mark_fgi() {
    // G-4b: pure-FGI page. Same shape as the NATO case but for the
    // Fgi(_) variant. The FGI countries are on the classification
    // axis; the dissem-axis fgi_marker should not duplicate them.
    let mut fgi_portion = CanonicalAttrs::default();
    fgi_portion.classification = Some(MarkingClassification::Fgi(FgiClassification {
        level: Classification::Secret,
        countries: Box::new([cc("GBR")]),
    }));
    let portions = [fgi_portion];
    let lat = project_via_lattice(&portions);
    assert!(
        matches!(lat.classification, Some(MarkingClassification::Fgi(_))),
        "lattice preserves Fgi classification on solely-FGI page"
    );
    assert!(
        lat.fgi_marker.is_none(),
        "G-4b: lattice must NOT double-mark FGI classification as fgi_marker. \
         fgi_marker = {:?}",
        lat.fgi_marker
    );
}

// ===========================================================================
// G-4c (PR 4b-B 9th-pass follow-up) — FGI suppression source-loss guard
// ===========================================================================
//
// G-4b suppressed `expected_fgi_marker()` on solely-non-US pages on the
// theory that the foreign source is already recorded on the
// classification axis. That assumption holds when the winning
// classification's payload is a SUPERSET of all foreign sources
// contributed by all non-US classification portions.
//
// **It does NOT hold when classification-axis OrdMax picks a winner
// whose foreign payload is a strict subset of all observed sources**:
//
//   Inputs:  Fgi(Confidential, [GBR]), Fgi(Secret, [CAN])
//   Winner:  Fgi(Secret, [CAN])      -- OrdMax: Secret > Confidential
//   Source loss: GBR is dropped from the FGI axis silently.
//
// PageContext does not have this problem because its
// `expected_classification` always wraps in `Us(level)` and its
// `expected_fgi_marker` unions every foreign source from every
// non-US classification portion (NATO-emitted "NATO" + Fgi-emitted
// countries + Joint-emitted non-USA countries) regardless of which
// portion's classification level "won."
//
// **Fix shape**: when the lattice's solely-non-US suppression path
// fires, gather the union of foreign sources from all non-US
// classification portions; compare to the winner's foreign sources
// (extracted via `extract_foreign_sources`). If the winner is a
// strict subset, the missing sources MUST be merged into the
// FGI marker so they are preserved on the dissem axis. If the sets
// are equal, suppression is safe (no source loss).
//
// Citation: §H.7 p124 (source-concealed-dominance precedence rules
// at the banner-line guidance block) + §H.7 pp123-125 reciprocal-
// normalization (FGI source must be preserved across the projection).
// Verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`.
// ===========================================================================

#[test]
fn fgi_mixed_level_different_countries_preserves_all_sources() {
    // G-4c: solely-FGI page with two portions at different
    // classification levels and disjoint country sets. The
    // ClassificationLattice winner is the higher level (Secret) which
    // carries only CAN; without the source-loss guard, GBR is silently
    // dropped from the FGI axis. The fix merges GBR into the FGI
    // marker so both producers are preserved.
    let mut p_low = CanonicalAttrs::default();
    p_low.classification = Some(MarkingClassification::Fgi(FgiClassification {
        level: Classification::Confidential,
        countries: Box::new([cc("GBR")]),
    }));
    let mut p_high = CanonicalAttrs::default();
    p_high.classification = Some(MarkingClassification::Fgi(FgiClassification {
        level: Classification::Secret,
        countries: Box::new([cc("CAN")]),
    }));
    let portions = [p_low, p_high];
    let lat = project_via_lattice(&portions);

    // Winner: Fgi(Secret, [CAN]) per OrdMax.
    match &lat.classification {
        Some(MarkingClassification::Fgi(f)) => {
            assert_eq!(f.level, Classification::Secret, "winner is Secret level");
            let winner_set: std::collections::BTreeSet<CountryCode> =
                f.countries.iter().copied().collect();
            assert!(
                winner_set.contains(&cc("CAN")),
                "winner classification carries CAN: {:?}",
                f.countries
            );
            // GBR is NOT on the winning classification axis (would
            // require classification-payload merging across levels,
            // which OrdMax does not do).
            assert!(
                !winner_set.contains(&cc("GBR")),
                "winning classification does not carry GBR (it was on the \
                 lower-level Confidential portion that OrdMax discarded): {:?}",
                f.countries
            );
        }
        other => panic!("expected Fgi classification winner, got {other:?}"),
    }

    // G-4c invariant: GBR MUST appear on the FGI axis (either via
    // marker merge from classification-derived sources, or via some
    // other preservation channel). Source loss is the bug.
    let fgi_set: std::collections::BTreeSet<CountryCode> = match &lat.fgi_marker {
        Some(FgiMarker::Acknowledged { countries, .. }) => countries.iter().copied().collect(),
        Some(FgiMarker::SourceConcealed) => std::collections::BTreeSet::new(),
        None => std::collections::BTreeSet::new(),
    };
    assert!(
        fgi_set.contains(&cc("GBR")),
        "G-4c: GBR was on the lower-level FGI portion's classification \
         and must be preserved on the FGI axis when OrdMax picks a \
         winner that does not carry it. fgi_marker = {:?}",
        lat.fgi_marker
    );
}

#[test]
fn fgi_same_level_different_countries_merges_via_union() {
    // G-4c sibling case: same-level FGI portions with disjoint country
    // sets. ClassificationLattice's `classification_join_same_variant`
    // already unions the country payloads (per C-7 PR 4b-B follow-up),
    // so the winner carries BOTH GBR and CAN. The FGI axis is
    // suppression-safe in this case because no source is lost.
    //
    // This pins the safe-suppression branch of the G-4c fix: when the
    // winner's foreign payload IS the union of all observed sources,
    // suppression is correct.
    let mut p1 = CanonicalAttrs::default();
    p1.classification = Some(MarkingClassification::Fgi(FgiClassification {
        level: Classification::Secret,
        countries: Box::new([cc("GBR")]),
    }));
    let mut p2 = CanonicalAttrs::default();
    p2.classification = Some(MarkingClassification::Fgi(FgiClassification {
        level: Classification::Secret,
        countries: Box::new([cc("CAN")]),
    }));
    let portions = [p1, p2];
    let lat = project_via_lattice(&portions);

    // Winner classification: Fgi(Secret, [CAN, GBR]) per C-7 union
    // tiebreaker. Both producers ride on the classification axis.
    match &lat.classification {
        Some(MarkingClassification::Fgi(f)) => {
            let winner_set: std::collections::BTreeSet<CountryCode> =
                f.countries.iter().copied().collect();
            assert!(
                winner_set.contains(&cc("GBR")) && winner_set.contains(&cc("CAN")),
                "C-7 same-level union: winner should carry both GBR and CAN: {:?}",
                f.countries
            );
        }
        other => panic!("expected Fgi classification winner, got {other:?}"),
    }

    // Suppression is safe — the winning classification carries every
    // observed foreign source, so the FGI axis can stay empty.
    assert!(
        lat.fgi_marker.is_none(),
        "G-4c safe-branch: when the winning classification's payload \
         already contains all foreign sources, the FGI axis stays \
         suppressed (no double-marking). fgi_marker = {:?}",
        lat.fgi_marker
    );
}

// ===========================================================================
// JointSet empty-producer defensive normalization (PR 4b-B 7th-pass)
// ===========================================================================
//
// `JointSet::from_attrs_iter` had a defensive shape that returned
// `Bottom` only when ALL JOINT portions shared an empty producer set.
// If portion 1 had an empty producer list (malformed per §H.3 p56,
// which requires `[LIST]` non-empty) and portion 2 had a non-empty
// list, the unanimity check failed (sets differ), the code fell into
// the disunity branch, and emitted a fake `DisunityCollapse` whose
// "union of non-US producers" was just portion 2's set.
//
// Fix: drop empty-producer JOINT portions from the calculation
// entirely. Per §H.3 p56 they're malformed and shouldn't contribute.
// If all portions are dropped → `Bottom`. If only some are dropped →
// the remaining portions drive the lattice state per the standard
// rules.
// ===========================================================================

#[test]
fn joint_with_empty_producer_portion_does_not_emit_fake_disunity() {
    // Portion 1: malformed JOINT with empty producer list.
    // Portion 2: well-formed JOINT with USA+GBR.
    // Pre-fix: unanimity check failed (∅ != {USA,GBR}) → fell into
    // disunity branch → emitted fake DisunityCollapse{non_us={GBR}}.
    // Post-fix: portion 1 is dropped, portion 2 is the only JOINT
    // portion, and the result is UnanimousProducers{level=S,
    // producers={USA,GBR}}.
    use marque_capco::JointSet;
    let mut malformed = CanonicalAttrs::default();
    malformed.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: Box::new([]), // empty producer list — malformed per §H.3 p56
    }));
    let well_formed = portion_joint(Classification::Secret, &["USA", "GBR"]);
    let portions = [malformed, well_formed];
    let s = JointSet::from_attrs_iter(&portions);
    // Must not be DisunityCollapse — the empty-producer portion was
    // malformed, not a real disagreement signal.
    assert!(
        !s.is_disunity_collapse(),
        "JointSet must not emit fake DisunityCollapse from malformed empty-producer portion: {s:?}"
    );
    // Should be UnanimousProducers with USA+GBR (the remaining
    // well-formed portion's producer list).
    match s {
        JointSet::UnanimousProducers { level, producers } => {
            assert_eq!(level, Classification::Secret);
            assert!(producers.contains(&cc("USA")));
            assert!(producers.contains(&cc("GBR")));
        }
        other => {
            panic!("expected UnanimousProducers after dropping malformed portion, got {other:?}")
        }
    }
}

#[test]
fn joint_with_only_empty_producer_portions_returns_bottom() {
    // All JOINT portions have empty producer lists → all are dropped
    // → result is Bottom (no JOINT portions to consider).
    use marque_capco::JointSet;
    let mut malformed1 = CanonicalAttrs::default();
    malformed1.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: Box::new([]),
    }));
    let mut malformed2 = CanonicalAttrs::default();
    malformed2.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Confidential,
        countries: Box::new([]),
    }));
    let portions = [malformed1, malformed2];
    let s = JointSet::from_attrs_iter(&portions);
    assert!(
        matches!(s, JointSet::Bottom),
        "All-malformed-empty-producer JOINT portions must collapse to Bottom: {s:?}"
    );
}

#[test]
fn joint_two_well_formed_with_one_empty_disunity_drops_empty() {
    // Three portions:
    //   - JOINT S USA GBR (well-formed)
    //   - JOINT S [] (malformed empty)
    //   - JOINT S USA CAN (well-formed, disagrees with first)
    // After dropping the malformed portion, the remaining two are
    // genuine disunity → DisunityCollapse{non_us={GBR, CAN}}.
    use marque_capco::JointSet;
    let mut malformed = CanonicalAttrs::default();
    malformed.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: Box::new([]),
    }));
    let portions = [
        portion_joint(Classification::Secret, &["USA", "GBR"]),
        malformed,
        portion_joint(Classification::Secret, &["USA", "CAN"]),
    ];
    let s = JointSet::from_attrs_iter(&portions);
    assert!(
        s.is_disunity_collapse(),
        "Two well-formed JOINT portions with disagreeing producers must yield DisunityCollapse: {s:?}"
    );
    let non_us = s.disunity_collapse_non_us_producers().expect("disunity");
    assert!(non_us.contains(&cc("GBR")));
    assert!(non_us.contains(&cc("CAN")));
    assert!(!non_us.contains(&cc("USA")));
}

// ===========================================================================
// JointSet USA invariant (PR 4b-B 9th-pass follow-up)
// ===========================================================================
//
// Per CAPCO-2016 §H.3 p56: "JOINT marking ... USA always appears as
// the OWNER/PRODUCER." A `JointClassification` constructed without
// USA in the producer list is malformed input — pre-fix, the lattice
// treated `JointClassification { countries: [GBR] }` (no USA) as a
// well-formed JOINT portion and emitted a JOINT banner without USA,
// which is unrepresentable in the §H.3 grammar.
//
// Pre-fix, `JointSet::from_attrs_iter` only filtered empty-producer
// JOINT portions (the previous defensive shape from
// `joint_with_empty_producer_portion_does_not_emit_fake_disunity`).
// A JOINT portion with one or more non-USA countries but no USA was
// pushed to `joint_portions` and contributed to the unanimity /
// disunity decision, producing a malformed JOINT banner.
//
// **Fix**: extend the empty-producer drop predicate to also drop
// JOINT portions whose producer list does not contain USA. The
// portion is treated as invisible to the JOINT axis (matching the
// existing empty-producer treatment); other well-formed JOINT
// portions on the page still drive the unanimity / disunity branch.
//
// Authority: §H.3 p56 ("USA always appears as the OWNER/PRODUCER").
// Verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`.
// ===========================================================================

#[test]
fn joint_missing_usa_dropped_from_joint_calculation() {
    // Single JOINT portion with no USA in producer list → treated
    // as malformed, dropped → JointSet::Bottom (no JOINT seen).
    use marque_capco::JointSet;
    let portions = [portion_joint(Classification::Secret, &["GBR"])];
    let s = JointSet::from_attrs_iter(&portions);
    assert!(
        matches!(s, JointSet::Bottom),
        "JOINT portion missing USA must be dropped (treated as malformed) \
         per §H.3 p56; got {s:?}"
    );
}

#[test]
fn joint_two_portions_one_missing_usa_other_valid() {
    // Two JOINT portions:
    //   - JOINT S GBR CAN (malformed: no USA)
    //   - JOINT S USA GBR (well-formed)
    // Pre-fix: malformed portion contributes producer set {GBR, CAN};
    // valid portion contributes {USA, GBR}; unanimity check fails →
    // fake DisunityCollapse{non_us={CAN, GBR}} (CAN is from the
    // malformed portion).
    // Post-fix: malformed portion dropped → only valid portion
    // remains → UnanimousProducers{S, {USA, GBR}}.
    use marque_capco::JointSet;
    let portions = [
        portion_joint(Classification::Secret, &["GBR", "CAN"]),
        portion_joint(Classification::Secret, &["USA", "GBR"]),
    ];
    let s = JointSet::from_attrs_iter(&portions);
    match &s {
        JointSet::UnanimousProducers { level, producers } => {
            assert_eq!(*level, Classification::Secret);
            assert!(producers.contains(&cc("USA")));
            assert!(producers.contains(&cc("GBR")));
            assert!(
                !producers.contains(&cc("CAN")),
                "CAN was on the malformed portion; must NOT appear in the \
                 unanimous-producers set after the malformed drop: {producers:?}"
            );
        }
        other => {
            panic!("Expected UnanimousProducers after dropping no-USA JOINT portion, got {other:?}")
        }
    }
}

#[test]
fn joint_all_portions_missing_usa_returns_bottom() {
    // All JOINT portions missing USA → all dropped → Bottom.
    use marque_capco::JointSet;
    let portions = [
        portion_joint(Classification::Secret, &["GBR"]),
        portion_joint(Classification::Confidential, &["CAN", "AUS"]),
    ];
    let s = JointSet::from_attrs_iter(&portions);
    assert!(
        matches!(s, JointSet::Bottom),
        "All-malformed (no-USA) JOINT portions must collapse to Bottom \
         per §H.3 p56; got {s:?}"
    );
}

#[test]
fn joint_missing_usa_parity_with_pagecontext() {
    // End-to-end parity: a malformed JOINT portion (no USA) must
    // produce the same `CanonicalAttrs` shape on both projection
    // paths. PageContext's `expected_classification` returns the
    // max `effective_level()` (Secret) and `expected_fgi_marker`
    // skips USA when iterating JOINT countries — so PageContext
    // surfaces GBR via the FGI axis, not via a JOINT banner.
    //
    // Post-fix lattice path: the JOINT portion is dropped from
    // `JointSet`, the classification falls through to the
    // ClassificationLattice non-JOINT branch (where Joint flattens
    // to Us(level)), and `expected_fgi_marker` (called when
    // `solely_non_us = false` since Joint counts as US-bearing per
    // G-9b) surfaces GBR identically.
    //
    // Both paths must agree on the banner shape: `Us(Secret)` +
    // `FGI [GBR]`. This pins the fix-both-paths stance: PageContext
    // does not have the JOINT-no-USA bug because it never
    // reconstructs the JOINT banner from per-portion data; the
    // lattice-only fix achieves parity.
    let portions = [portion_joint(Classification::Secret, &["GBR"])];
    assert_byte_identity(
        "joint_missing_usa_parity_with_pagecontext",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

// ===========================================================================
// CV-6 (PR 4b-B 8th-pass follow-up) — edge-case parity fixtures
//
// Three deterministic fixtures locking in design assumptions surfaced
// by the independent CAPCO-domain review:
//
// - Gap B: empty-portion-list (banner candidate but no portions
//   accumulated) — proves no panic, sane lattice-bottom defaults on
//   every axis, byte-identity with PageContext's empty-portion path.
// - Gap C: JOINT + explicit FGI marker on the same portion. CAPCO
//   §H.3 p57 line 1271 explicitly permits this composition ("If FGI
//   information is used in a JOINT classified document, refer to
//   Section H.7"); §H.3 Notional Example Page 3 p59 shows
//   `(//JOINT S GBR USA//FGI NZL//REL TO USA, FVEY)`. The lattice
//   handles this correctly via independent JointSet and FgiSet
//   construction; this fixture proves coexistence.
// - Gap D: JOINT + NOFORN on the SAME portion. CAPCO §H.3 p57
//   line 1271 PROHIBITS this composition ("May not be used with the
//   HCS markings or NOFORN markings"). The lattice does NOT catch
//   this directly — the constraint is caught INDIRECTLY via E014
//   (JOINT requires REL TO coverage) + `capco/noforn-conflicts-rel-to`
//   (NOFORN conflicts REL TO in the dissem axis). This fixture
//   pins the parity-gate's INDIRECT-coverage stance: both projection
//   paths produce the same banner shape because the indirect
//   constraint catch happens at a layer above the projection.
//
// Each fixture below is byte-identity-asserting (no documented
// divergences). If a future refactor breaks one of these
// composition stances silently, the parity gate fires immediately
// and names the fixture that anchors the assumption.
// ===========================================================================

#[test]
fn cv6_gap_b_empty_portion_list_yields_lattice_bottom() {
    // Banner candidate with no portions accumulated. Both projection
    // paths must produce a default-sane `CanonicalAttrs` — every
    // axis at its lattice bottom, no panic.
    //
    // The lattice path's `CapcoMarking::join_via_lattice(&[])` must
    // be a total function returning `CanonicalAttrs::default()`-
    // shaped output: empty boxed slices, `None` for option-shaped
    // axes, `Some(Bottom)` for any state-machine axis that named
    // a default bottom in its enum.
    //
    // PageContext's `add_portion` loop over an empty slice produces
    // the same shape. Pinning byte-identity here closes the
    // edge-case the parity gate didn't previously cover.
    let portions: [CanonicalAttrs; 0] = [];
    assert_byte_identity(
        "cv6_gap_b_empty_portion_list",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn cv6_gap_c_joint_with_explicit_fgi_marker_coexist_mixed_us_page() {
    // CAPCO §H.3 p57 explicitly permits JOINT + FGI on the same
    // portion ("If FGI information is used in a JOINT classified
    // document, refer to Section H.7"); §H.3 Notional Example
    // Page 3 p59 shows `(//JOINT S GBR USA//FGI NZL//REL TO USA,
    // FVEY)`. The lattice handles this correctly because JointSet
    // reads `MarkingClassification::Joint` and FgiSet reads the
    // independent `fgi_marker` field — neither projection step
    // drops the other axis.
    //
    // The fixture: one JOINT (USA + GBR) portion that ALSO carries
    // an explicit FGI marker for NZL, COMBINED WITH a plain US
    // portion. The mixed JOINT+US page sends `JointSet` into the
    // `Mixed` state (§H.3 p57 "JOINT marking is not carried forward
    // to the banner line in US documents"); both paths flatten the
    // classification axis to `Us(Secret)`, and both paths migrate
    // the JOINT non-US producers (GBR) into the FGI axis alongside
    // the explicit FGI marker (NZL). The byte-identity assertion
    // proves the parity-gate baseline covers this composition:
    //   - classification = Us(Secret) on both paths.
    //   - fgi_marker = Acknowledged{countries: {GBR, NZL}} on both
    //     paths — explicit FGI marker AND JOINT-migrated producer
    //     coexist via the union.
    //
    // Note: a SOLELY-JOINT page with an explicit FGI marker
    // (single-portion variant) is the documented divergence shape
    // tracked by `joint_unanimous_two_portions` / G-4 — the lattice
    // intentionally preserves the JOINT classification on the
    // classification axis there, while PageContext flattens and
    // double-marks producers as FGI. The Mixed-page fixture here
    // sidesteps that divergence by hitting the §H.3 p57 mixed-page
    // path where both projections agree.
    let mut joint_with_fgi = CanonicalAttrs::default();
    joint_with_fgi.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: Box::new([cc("USA"), cc("GBR")]),
    }));
    joint_with_fgi.fgi_marker = FgiMarker::acknowledged([cc("NZL")]);
    let portions = [joint_with_fgi, portion_us(Classification::Secret)];
    assert_byte_identity(
        "cv6_gap_c_joint_with_explicit_fgi_marker_coexist_mixed_us_page",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

#[test]
fn cv6_gap_d_joint_with_noforn_parity_indirect_catch_mixed_us_page() {
    // CAPCO §H.3 p57 line 1271 PROHIBITS JOINT + NOFORN ("May not
    // be used with the HCS markings or NOFORN markings"). The
    // lattice does NOT catch this directly — the constraint is
    // caught INDIRECTLY via:
    //   - E014/joint-requires-rel-to-coverage (CAPCO §H.3 p57:
    //     "Requires REL TO USA, LIST") at the constraint layer
    //     above the projection, AND
    //   - the `capco/noforn-conflicts-rel-to` rewrite at the
    //     PageRewrite scheduler layer (NOFORN clears REL TO).
    //
    // The constraint and rewrite layers are BOTH outside the
    // per-axis projection that `join_via_lattice` and
    // `expected_*` produce. The parity gate's job is to assert
    // that both projection paths agree on the BANNER SHAPE for
    // this constraint-prohibited-but-axis-permissible input —
    // catching the prohibited composition is the constraint
    // layer's responsibility, not the projection's.
    //
    // The fixture: one JOINT (USA + GBR) portion that ALSO carries
    // NOFORN, COMBINED WITH a plain US portion. The mixed JOINT+US
    // shape sends JointSet into the `Mixed` state per §H.3 p57 so
    // both projections flatten the classification axis to
    // `Us(Secret)`, and both keep `Nf` in `dissem_us`. The byte-
    // identity assertion documents the indirect-catch stance: the
    // projection paths agree on the shape; the prohibited-
    // composition guard fires at the constraint layer (E014 +
    // `capco/noforn-conflicts-rel-to`).
    //
    // Note: a SOLELY-JOINT page with NOFORN (single-portion variant)
    // hits the documented divergence at `joint_unanimous_two_portions`
    // / G-4 — the lattice preserves the JOINT classification, and
    // PageContext flattens to Us + migrates JOINT producers to FGI.
    // Mixed-page fixture sidesteps that.
    //
    // A future refactor that pushes the JOINT+NOFORN check INTO the
    // projection would change the byte-identity output AND need to
    // update this fixture, surfacing the architectural shift in
    // code review.
    let mut joint_with_nf = CanonicalAttrs::default();
    joint_with_nf.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: Box::new([cc("USA"), cc("GBR")]),
    }));
    joint_with_nf.dissem_us = vec![DissemControl::Nf].into_boxed_slice();
    let portions = [joint_with_nf, portion_us(Classification::Secret)];
    assert_byte_identity(
        "cv6_gap_d_joint_with_noforn_parity_indirect_catch_mixed_us_page",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        &[],
    );
}

// ===========================================================================
// DISPLAY ONLY axis — 2 parity fixtures (N-9-1 PR 437 10th-pass)
// ===========================================================================
//
// Both paths produce `display_only_to = Box<[]>` today because
// PageContext explicitly defers the §D.2 Table 3 row 25-27
// intersection roll-up to Phase 2 (see `page_context.rs:246-252`).
// These fixtures gate against future divergence: if either projection
// path starts emitting non-empty `display_only_to`, the parity helper's
// `check_eq!(display_only_to, ...)` invocation will catch it.
//
// Authority: §H.8 p163 (DISPLAY ONLY template, authorized banner
// marking and portion form) + §D.2 p28-30 Table 3 rows 25-27 (roll-up
// rules: row 25 = DO[LIST] ∩ DO[LIST] common-element, row 26 =
// DO[LIST] ∩ REL TO common-element, row 27 = combined REL TO/DO).
// Both citations verified 2026-05-16 against
// `crates/capco/docs/CAPCO-2016.md`.

#[test]
fn display_only_single_portion_parity() {
    // §H.8 p163: single DISPLAY ONLY [IRQ] portion.
    // Both projection paths should produce `display_only_to = Box<[]>`
    // (Phase-2 deferred). This fixture gates that both paths agree —
    // if Phase 2 wires the axis in one path before the other, this
    // fixture will catch the divergence first.
    let mut p = portion_us(Classification::Secret);
    p.display_only_to = vec![cc("IRQ")].into_boxed_slice();
    let portions = [p];
    assert_byte_identity(
        "display_only_single_portion_parity",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        // Phase-2 deferred: both produce Box<[]>; no divergence today.
        // When Phase 2 lands, remove the empty divergence list and add
        // a row checking the non-empty result.
        &[],
    );
}

#[test]
fn display_only_two_portions_disjoint_lists_parity() {
    // DOCUMENTED DIVERGENCE (post-DISPLAY-ONLY-Phase-2):
    // §D.2 p28-30 Table 3 row 20: two DISPLAY ONLY portions with
    // disjoint [LIST]s → NOFORN at the banner.
    //
    // Pre-DISPLAY-ONLY-Phase-2 both paths produced empty
    // `display_only_to` and no NF injection (deferred). Staging
    // landed PR #449 (DISPLAY ONLY Phase 2 banner roll-up) which
    // adds the §D.2 row 20 NF injection to PageContext via the
    // `capco/noforn-clears-fdr-family` page-rewrite. The lattice
    // path (`CapcoMarking::join_via_lattice`) does NOT yet implement
    // DISPLAY ONLY axis aggregation, so it skips the corresponding
    // NF injection — the divergence is `pc=[Nf], lat=[]` on
    // `dissem_us`.
    //
    // This is a TEMPORARY divergence — lattice path catches up when
    // PR 4b-C/4b-D adds the DISPLAY ONLY axis aggregator + the
    // mirrored NF injection. Tracked alongside issue #461
    // (Phase::PageFinalization scope).
    //
    // Citation: §D.2 p28-30 Table 3 row 20 (verified 2026-05-16
    // against CAPCO-2016.md).
    let mut p1 = portion_us(Classification::Secret);
    p1.display_only_to = vec![cc("IRQ")].into_boxed_slice();
    let mut p2 = portion_us(Classification::Secret);
    p2.display_only_to = vec![cc("AFG")].into_boxed_slice();
    let portions = [p1, p2];
    assert_byte_identity(
        "display_only_two_portions_disjoint_lists_parity",
        &project_via_page_context(&portions),
        &project_via_lattice(&portions),
        // Lattice path lags PageContext for DISPLAY ONLY §D.2 row 20
        // NF injection — see fixture doc above.
        &["dissem_us"],
    );
}
