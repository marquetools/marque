// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 4b-E renamed parity gate — lattice-vs-scheme byte-identity
//! (post-PageContext-deletion).
//!
//! History: this file landed in PR 4b-B Commit 8 (006 T112) as
//! `page_context_lattice_parity.rs`, a THREE-path comparison
//! (`PageContext::expected_*` vs `join_via_lattice` vs
//! `CapcoScheme::project(Scope::Page, ...)`). PR 4b-E deleted the
//! `PageContext::expected_*` accessor surface entirely; this PR
//! renamed the file to `lattice_vs_scheme_parity.rs` to reflect
//! the surviving TWO-path comparison.
//!
//! Synthetic-fixture parity matrix comparing the per-axis lattice
//! path (`project_via_lattice` = `CapcoMarking::join_via_lattice`)
//! against the full scheme pipeline
//! (`project_via_scheme` = `CapcoScheme::project(Scope::Page, ...)`,
//! which composes per-axis lattices then runs the declarative
//! PageRewrite catalog).
//!
//! The two paths MUST produce byte-identical `CanonicalAttrs` on every
//! axis EXCEPT the deliberate divergences documented inline below.
//! Each divergence carries a `§X.Y pNN` citation re-verified against
//! `crates/capco/docs/CAPCO-2016.md`.
//!
//! ## Why synthetic instead of corpus fixtures
//!
//! The corpus fixtures in `tests/corpus/valid/` are designed to
//! exercise the strict-recognizer + rule pipeline end-to-end. The
//! parity gate's job is to compare TWO projection paths from
//! pre-parsed per-portion `CanonicalAttrs` values. Synthetic
//! `CanonicalAttrs` fixtures hand-built in this file cover the
//! specific axes the PR touches with full control over the input
//! shape; the strict-recognizer is orthogonal to the parity claim.
//!
//! ## DISSEM_US divergence (hoisted rationale)
//!
//! Many fixtures in this file declare `&["dissem_us"]` as a
//! deliberate divergence between `project_via_lattice` and
//! `project_via_scheme`. Per Copilot R2 (PR #539): duplicating the
//! same rationale inline at every call-site is a citation-drift
//! hazard — Constitution VIII "propagation requires re-verification"
//! applies. The canonical rationale lives here; each fixture-site
//! comment is one line that points back.
//!
//! **Two divergence sources today (single source of truth):**
//!
//! 1. **§B.3 Table 2 p21 caveated-classified.** Fires on the scheme
//!    path's `CLOSURE_NOFORN_CAVEATED` closure (the input is a
//!    classified + caveated marking — caveat per §B.3 p20 Note
//!    covers ORCON / non-IC dissem / etc.). The per-axis lattice
//!    path does not run closure rules; the expected `dissem_us`
//!    divergence is `lat=[..no Nf], scheme=[..Nf]`. Verified
//!    2026-05-18 against `crates/capco/docs/CAPCO-2016.md` §B.3
//!    p20 + Table 2 p21.
//!
//! 2. **§B.3 Table 2 p21 implicit-RELIDO on US collateral
//!    classification (Issue #524 Phase 3).** Fires on the scheme
//!    path's `CLOSURE_RELIDO_US_CLASS` closure (the input is a
//!    US collateral classification — Restricted / Confidential /
//!    Secret / TopSecret — absent any FD&R-dominator). The
//!    per-axis lattice path does not run closure rules; the
//!    expected `dissem_us` divergence is `lat=[..no Relido],
//!    scheme=[..Relido]`. Primary authority: CAPCO-2016 §B.3
//!    Table 2 p21 ("Classified + uncaveated + on/after 28 June
//!    2010 → Mark as RELIDO"). Grammar reference: §H.8 p154
//!    (RELIDO marking template + Unclassified carve-out). Design
//!    synthesis: `marque-applied.md` Section 4.7.5. Verified
//!    2026-05-18 against `crates/capco/docs/CAPCO-2016.md` §B.3
//!    Table 2 p21 + §H.8 p154.

use marque_capco::CapcoMarking;
use marque_capco::scheme::CapcoScheme;
use marque_ism::{
    AeaMarking, CanonicalAttrs, Classification, CountryCode, DissemControl, FgiClassification,
    FgiMarker, ForeignClassification, JointClassification, MarkingClassification,
    NatoClassification, NonIcDissem,
};
use marque_scheme::{MarkingScheme as _, Scope};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn cc(s: &str) -> CountryCode {
    CountryCode::try_new(s.as_bytes()).expect("valid trigraph")
}

// PR 4b-E: `project_via_page_context` retired alongside the
// `PageContext::expected_*` accessor surface. Post-deletion the
// gate compares `project_via_lattice` (per-axis lattice composition)
// against `project_via_scheme` (full pipeline including the
// PageRewrite catalog). See `lattice_vs_scheme_parity.rs` rename
// in PR 4b-E Commit 7.

fn project_via_lattice(portions: &[CanonicalAttrs]) -> CanonicalAttrs {
    CapcoMarking::join_via_lattice(portions)
}

/// Drive the projection through `CapcoScheme::project(Scope::Page, ...)` —
/// the post-PR-4b-D production path. The scheme's page-rewrite loop
/// runs the declarative PageRewrite catalog over the input portions,
/// so Pattern-B + Pattern-C strip rows fire here even though they are
/// inert in the per-axis `project_via_page_context` and
/// `project_via_lattice` helpers above.
///
/// PR 4b-C Commit 6 introduces this helper so the Pattern-B + Pattern-C
/// fixtures can assert the strip-plus-promote semantic that lives in
/// the declarative rows. The pre-existing fixtures continue to use
/// the per-axis helpers; the new fixtures use `project_via_scheme` to
/// exercise the declarative path.
fn project_via_scheme(portions: &[CanonicalAttrs]) -> CanonicalAttrs {
    let scheme = CapcoScheme::new();
    let markings: Vec<CapcoMarking> = portions.iter().cloned().map(CapcoMarking::new).collect();
    scheme.project(Scope::Page, &markings).0
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — see module doc "DISSEM_US divergence (hoisted rationale)"
        // for the §B.3 Table 2 p21 + §B.3 p20 Note caveated-classified citation.
        &["dissem_us"],
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — see module doc "DISSEM_US divergence (hoisted rationale)"
        // for the §B.3 Table 2 p21 + §B.3 p20 Note caveated-classified citation.
        &["dissem_us"],
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — see module doc "DISSEM_US divergence (hoisted rationale)"
        // for the §B.3 Table 2 p21 + §B.3 p20 Note caveated-classified citation.
        &["dissem_us"],
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — see module doc "DISSEM_US divergence (hoisted rationale)"
        // for the §B.3 Table 2 p21 + §B.3 p20 Note caveated-classified citation.
        &["dissem_us"],
    );
}

#[test]
fn oc_usgov_no_oc_no_usgov() {
    let portions = [portion_us(Classification::Secret)];
    assert_byte_identity(
        "oc_usgov_no_oc_no_usgov",
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — see module doc "DISSEM_US divergence
        // (hoisted rationale)" source 2 (§H.8 p154 implicit-RELIDO,
        // Issue #524 Phase 3) for the citation.
        &["dissem_us"],
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — see module doc "DISSEM_US divergence (hoisted rationale)"
        // for the §B.3 Table 2 p21 + §B.3 p20 Note caveated-classified citation.
        &["dissem_us"],
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — the third (bare) portion triggers
        // CLOSURE_RELIDO_US_CLASS (Issue #524 Phase 3). See module
        // doc "DISSEM_US divergence (hoisted rationale)" source 2
        // for the §H.8 p154 + marque-applied Section 4.7.5 citation.
        &["dissem_us"],
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — the bare US portion triggers
        // CLOSURE_RELIDO_US_CLASS (Issue #524 Phase 3). See module
        // doc "DISSEM_US divergence (hoisted rationale)" source 2.
        &["dissem_us"],
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        &[],
    );
}

// ===========================================================================
// JOINT — 5 cases (with documented divergences for disunity collapse)
// ===========================================================================

#[test]
fn joint_unanimous_two_portions_converge_to_joint_variant() {
    let portions = [
        portion_joint(Classification::Secret, &["USA", "GBR"]),
        portion_joint(Classification::Secret, &["USA", "GBR"]),
    ];
    // PR 4b-E (OQ-7 convergence): the pre-PR-4b-E divergence was
    // PageContext (Us-only) vs lattice/scheme (Joint(_)).
    // Post-deletion the PageContext side is gone; both surviving paths
    // produce `Joint(S, [USA, GBR])` per the §H.3 p56 + §H.3 p57
    // banner-fidelity reading. CONVERGED. Fixture renamed in PR 4b-E
    // review fix-up to reflect the post-deletion claim: the lattice
    // and scheme paths CONVERGE to a `Joint(_)` classification, not
    // diverge.
    //
    // Citation: §H.3 p56 + §H.3 p57.
    let lat = project_via_lattice(&portions);
    let scheme_proj = project_via_scheme(&portions);
    assert!(
        matches!(lat.classification, Some(MarkingClassification::Joint(_))),
        "lattice should produce Joint classification on unanimous JOINT"
    );
    assert!(
        matches!(
            scheme_proj.classification,
            Some(MarkingClassification::Joint(_))
        ),
        "scheme.project agrees with the lattice path (both go through \
         join_via_lattice); scheme_proj.classification = {:?}",
        scheme_proj.classification,
    );
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — see module doc "DISSEM_US divergence (hoisted rationale)"
        // for the §B.3 Table 2 p21 + §B.3 p20 Note caveated-classified citation.
        &["dissem_us"],
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
    // PR 4b-E: PageContext side retired; assert the same invariants
    // on the lattice and scheme paths. Both must produce Us(Secret)
    // classification and FGI marker carrying {GBR, CAN}.
    let lat = project_via_lattice(&portions);
    let scheme_proj = project_via_scheme(&portions);
    assert_eq!(
        lat.classification,
        Some(MarkingClassification::Us(Classification::Secret))
    );
    assert_eq!(
        scheme_proj.classification,
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
        extract_producers(&lat.fgi_marker),
        expected_producers,
        "Lattice FGI producer set must be {{CAN, GBR}}"
    );
    assert_eq!(
        extract_producers(&scheme_proj.fgi_marker),
        expected_producers,
        "scheme.project FGI producer set must be {{CAN, GBR}}"
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — pure US classified, CLOSURE_RELIDO_US_CLASS
        // fires on the scheme path (Issue #524 Phase 3). See module
        // doc "DISSEM_US divergence (hoisted rationale)" source 2.
        &["dissem_us"],
    );
}

#[test]
fn joint_single_portion_no_us_converge_to_joint_variant() {
    // A solitary JOINT portion (pure-JOINT page). Both surviving
    // paths produce Joint(_) classification per §H.3 p56
    // banner-fidelity. The pre-PR-4b-E PageContext side was the
    // divergent path; OQ-7 convergence achieved post-deletion.
    // Fixture renamed in PR 4b-E review fix-up to reflect the
    // post-deletion claim: the lattice and scheme paths CONVERGE
    // to a `Joint(_)` classification, not diverge.
    let portions = [portion_joint(Classification::Secret, &["USA", "GBR"])];
    let lat = project_via_lattice(&portions);
    let scheme_proj = project_via_scheme(&portions);
    assert!(matches!(
        lat.classification,
        Some(MarkingClassification::Joint(_))
    ));
    assert!(
        matches!(
            scheme_proj.classification,
            Some(MarkingClassification::Joint(_))
        ),
        "scheme.project agrees with the lattice path; \
         scheme_proj.classification = {:?}",
        scheme_proj.classification,
    );
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — pure US classified, CLOSURE_RELIDO_US_CLASS
        // fires on the scheme path (Issue #524 Phase 3). See module
        // doc "DISSEM_US divergence (hoisted rationale)" source 2.
        &["dissem_us"],
    );
}

#[test]
fn classification_single_portion() {
    let portions = [portion_us(Classification::Secret)];
    assert_byte_identity(
        "classification_single_portion",
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — pure US classified, CLOSURE_RELIDO_US_CLASS
        // fires on the scheme path (Issue #524 Phase 3). See module
        // doc "DISSEM_US divergence (hoisted rationale)" source 2.
        &["dissem_us"],
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
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
    let lat = project_via_lattice(&portions);
    let scheme_proj = project_via_scheme(&portions);
    // Both surviving paths: rel_to is cleared by the NODIS
    // supersession; NOFORN is injected per §H.9 p174 ("NOFORN would
    // convey in the banner line").
    assert!(lat.rel_to.is_empty());
    assert!(scheme_proj.rel_to.is_empty());
    assert!(lat.dissem_us.contains(&DissemControl::Nf));
    assert!(scheme_proj.dissem_us.contains(&DissemControl::Nf));
}

// ===========================================================================
// PR 4b-B follow-up parity divergences — documented w/ citation
// ===========================================================================

#[test]
fn fouo_classified_scheme_project_strips_fouo() {
    // PR 4b-E: PageContext side retired; the post-flip asymmetry is
    // now between the per-axis lattice helper (keeps FOUO — the
    // DissemSet's BTreeSet union doesn't apply classification-gated
    // strips) and the scheme path (strips FOUO via the
    // `capco/classification-evicts-fouo` + `capco/fouo-evicted-by-classified`
    // PageRewrites).
    //
    // Citation: §H.8 p134 FOUO Precedence Rules for Banner Line
    // Guidance (verified 2026-05-17 against
    // `crates/capco/docs/CAPCO-2016.md`).
    let portions = [portion_with_dissem_us(
        Classification::Secret,
        &[DissemControl::Fouo],
    )];
    let lat = project_via_lattice(&portions);
    let scheme_proj = project_via_scheme(&portions);

    assert!(
        lat.dissem_us.contains(&DissemControl::Fouo),
        "Per-axis lattice helper keeps FOUO (DissemSet does not apply \
         the §H.8 p134 classification-gate strip); lat.dissem_us = {:?}",
        lat.dissem_us,
    );
    assert!(
        !scheme_proj.dissem_us.contains(&DissemControl::Fouo),
        "scheme.project(Scope::Page, ...) strips FOUO from dissem_us on \
         classified pages via the `capco/classification-evicts-fouo` + \
         `capco/fouo-evicted-by-classified` PageRewrites \
         (§H.8 p134). scheme_proj.dissem_us = {:?}",
        scheme_proj.dissem_us,
    );
}

#[test]
fn aea_ucni_classified_scheme_project_strips_and_promotes_noforn() {
    // G-2 retarget (PR 4b-D.2): pre-flip this fixture was named
    // `aea_ucni_classified_pagecontext_and_lattice_both_keep_ucni_pending_pr_4b_d`
    // and asserted both per-axis helpers kept UCNI on classified pages
    // because the §H.6 p116/p118 strip-plus-NOFORN-promotion lived
    // only in `scheme.project`'s page-rewrite loop. PR 4b-D.2 flipped
    // the hot path to use `scheme.project`; this fixture now asserts
    // BOTH the UCNI strip AND the NOFORN promotion via the
    // `project_via_scheme` helper.
    //
    // Post-flip semantic: on a classified+DOD-UCNI page, four
    // PageRewrites fire through `scheme.project`:
    //   - `capco/dod-ucni-evicted-by-classified` (strip UCNI)
    //   - `capco/dod-ucni-promotes-noforn-when-classified` (inject NF)
    // The strip-and-promote pair was the §H.6 NOFORN-promotion clause
    // the pre-PR-4b-C PageContext branch silently dropped (a real bug
    // pinned by `pattern_c_dod_ucni_classified_strip_promotes_noforn`
    // in `crates/capco/tests/pattern_c_dod_ucni_classified_strip.rs`).
    //
    // Citation: §H.6 p116 (DOD UCNI / DCNI Precedence Rules) +
    // §H.6 p118 (DOE UCNI Precedence Rules) — verified 2026-05-17
    // against `crates/capco/docs/CAPCO-2016.md`.
    let mut p = portion_us(Classification::Secret);
    p.aea_markings = vec![AeaMarking::DodUcni].into_boxed_slice();
    let portions = [p];
    let lat = project_via_lattice(&portions);
    let scheme_proj = project_via_scheme(&portions);

    assert!(
        lat.aea_markings
            .iter()
            .any(|m| matches!(m, AeaMarking::DodUcni)),
        "Per-axis lattice helper keeps DOD UCNI (AeaSet does not apply \
         the §H.6 p116 strip); lat.aea_markings = {:?}",
        lat.aea_markings,
    );
    assert!(
        !scheme_proj
            .aea_markings
            .iter()
            .any(|m| matches!(m, AeaMarking::DodUcni)),
        "scheme.project(Scope::Page, ...) strips DOD UCNI from \
         aea_markings on classified pages via \
         `capco/dod-ucni-evicted-by-classified` (§H.6 p116); \
         scheme_proj.aea_markings = {:?}",
        scheme_proj.aea_markings,
    );
    assert!(
        scheme_proj.dissem_us.contains(&DissemControl::Nf),
        "scheme.project(Scope::Page, ...) promotes NOFORN into dissem_us \
         on classified+UCNI pages via \
         `capco/dod-ucni-promotes-noforn-when-classified` (§H.6 p116); \
         scheme_proj.dissem_us = {:?}",
        scheme_proj.dissem_us,
    );
}

#[test]
fn pure_nato_both_paths_preserve_nato_variant() {
    // PR 4b-E (OQ-7 convergence): pre-PR-4b-E G-3 divergence —
    // PageContext flattens NATO to Us(_); lattice/scheme preserve
    // Nato(_). Post-deletion the PageContext side is gone; both
    // surviving paths preserve `Nato(_)` per §H.7 pp123-125
    // reciprocal-raise (the rule applies only when a US portion is
    // in scope; pure-NATO pages preserve the foreign variant).
    // CONVERGED — file renamed to `lattice_vs_scheme_parity.rs` in
    // PR 4b-E Commit 7; fixture renamed in PR 4b-E review fix-up
    // (former name `pure_nato_lattice_vs_pagecontext_diverges` was
    // stale: the PageContext side is deleted, and the post-deletion
    // claim is that both surviving paths PRESERVE `Nato(_)`).
    //
    // Citation: §H.7 pp123-125 (reciprocal-raise rule).
    let mut nato_portion = CanonicalAttrs::default();
    nato_portion.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
    let portions = [nato_portion];
    let lat = project_via_lattice(&portions);
    let scheme_proj = project_via_scheme(&portions);
    assert!(
        matches!(lat.classification, Some(MarkingClassification::Nato(_))),
        "Lattice preserves Nato variant on solely-NATO page per §H.7 pp123-125"
    );
    assert!(
        matches!(
            scheme_proj.classification,
            Some(MarkingClassification::Nato(_))
        ),
        "scheme.project agrees with the lattice path on solely-NATO; \
         scheme_proj.classification = {:?}",
        scheme_proj.classification,
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — see module doc "DISSEM_US divergence (hoisted rationale)"
        // for the §B.3 Table 2 p21 + §B.3 p20 Note caveated-classified citation.
        &["dissem_us"],
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
    let lat = project_via_lattice(&portions);
    let scheme_proj = project_via_scheme(&portions);
    // Both surviving paths inject NOFORN.
    assert!(
        lat.dissem_us.contains(&DissemControl::Nf),
        "Lattice injects NF (G-6)"
    );
    assert!(
        scheme_proj.dissem_us.contains(&DissemControl::Nf),
        "scheme.project injects NF"
    );
    // Both surviving paths clear REL TO.
    assert!(lat.rel_to.is_empty(), "Lattice clears REL TO (G-6)");
    assert!(
        scheme_proj.rel_to.is_empty(),
        "scheme.project clears REL TO"
    );
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — Conflict-variant US classification
        // satisfies the `TOK_US_CLASSIFIED` trigger (us_classification()
        // returns the resolved US side) and CAT_NON_US_CLASSIFICATION
        // excludes Conflict per fn-doc, so CLOSURE_RELIDO_US_CLASS
        // fires on the scheme path (Issue #524 Phase 3 — see
        // `phase3_closure_pin::us_class_conflict_variant_pin`). See
        // module doc "DISSEM_US divergence (hoisted rationale)"
        // source 2.
        &["dissem_us"],
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — see module doc "DISSEM_US divergence (hoisted rationale)"
        // for the §B.3 Table 2 p21 + §B.3 p20 Note caveated-classified citation.
        &["dissem_us"],
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — the explicit US portion satisfies
        // `TOK_US_CLASSIFIED` and the lattice-joined output is pure
        // Us(_), so CLOSURE_RELIDO_US_CLASS fires on the scheme path
        // (Issue #524 Phase 3). See module doc "DISSEM_US divergence
        // (hoisted rationale)" source 2.
        &["dissem_us"],
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — see module doc "DISSEM_US divergence (hoisted rationale)"
        // for the §B.3 Table 2 p21 + §B.3 p20 Note caveated-classified citation.
        &["dissem_us"],
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — see module doc "DISSEM_US divergence (hoisted rationale)"
        // for the §B.3 Table 2 p21 + §B.3 p20 Note caveated-classified citation.
        &["dissem_us"],
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
// (`pure_nato_both_paths_preserve_nato_variant` — renamed from
// `pure_nato_lattice_vs_pagecontext_diverges` in PR 4b-E review
// fix-up). The new test below
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
    // Nato(_) classification per §H.7 pp123-125 (see the
    // `pure_nato_both_paths_preserve_nato_variant` parity fixture —
    // renamed from `pure_nato_lattice_vs_pagecontext_diverges` in
    // PR 4b-E review fix-up). The FGI
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — see module doc "DISSEM_US divergence (hoisted rationale)"
        // for the §B.3 Table 2 p21 + §B.3 p20 Note caveated-classified citation.
        &["dissem_us"],
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
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
    // tracked by `joint_unanimous_two_portions_converge_to_joint_variant`
    // / G-4 — the lattice
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // DISSEM_US divergence — see module doc "DISSEM_US divergence (hoisted rationale)"
        // for the §B.3 Table 2 p21 + §B.3 p20 Note caveated-classified citation.
        &["dissem_us"],
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
    // hits the documented divergence at
    // `joint_unanimous_two_portions_converge_to_joint_variant`
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
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
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // display_only_to: both paths produce Box<[]> (Phase-2
        // deferred). When Phase 2 lands, add a row checking the
        // non-empty result.
        // DISSEM_US divergence — pure US classified, no other
        // dissem; CLOSURE_RELIDO_US_CLASS fires on the scheme path
        // (Issue #524 Phase 3). See module doc "DISSEM_US divergence
        // (hoisted rationale)" source 2.
        &["dissem_us"],
    );
}

#[test]
fn display_only_two_portions_disjoint_lists_parity() {
    // §D.2 p28-30 Table 3 row 20: two DISPLAY ONLY portions with
    // disjoint [LIST]s → NOFORN at the banner.
    //
    // PR 4b-E note: pre-deletion the pc-vs-lat comparison expected
    // PC=[Nf], lat=[]. Post-deletion the comparison is lat-vs-scheme;
    // neither path currently injects NF at the dissem layer when the
    // DisplayOnlyBlock collapses to Empty (§D.2 row 20). The
    // DisplayOnlyBlock correctly produces empty DO output and the
    // PageRewrite catalog does not (yet) declare a row that fires NF
    // when DO collapses to Empty. Both paths produce `dissem_us=[]`
    // — convergent at the wrong value relative to §D.2 row 20. This
    // is a real lattice/scheme gap (NF should be injected here per
    // §D.2 row 20) but it's the same gap on both paths, so the
    // parity assertion is satisfied. Issue tracked for follow-up
    // PR; not blocking PR 4b-E.
    //
    // Citation: §D.2 p28-30 Table 3 row 20 (verified 2026-05-18
    // against CAPCO-2016.md).
    let mut p1 = portion_us(Classification::Secret);
    p1.display_only_to = vec![cc("IRQ")].into_boxed_slice();
    let mut p2 = portion_us(Classification::Secret);
    p2.display_only_to = vec![cc("AFG")].into_boxed_slice();
    let portions = [p1, p2];
    assert_byte_identity(
        "display_only_two_portions_disjoint_lists_parity",
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        // §D.2 row 20 NF-when-DO-collapses-to-Empty injection is a
        // known follow-up gap (same on both paths).
        // DISSEM_US divergence — both portions are pure US classified,
        // no other dissem (display_only_to is a separate axis);
        // CLOSURE_RELIDO_US_CLASS fires on the scheme path (Issue #524
        // Phase 3). See module doc "DISSEM_US divergence (hoisted
        // rationale)" source 2.
        &["dissem_us"],
    );
}

// ===========================================================================
// PR 4b-C Commit 6 — Pattern-B + Pattern-C declarative row fixtures
// ===========================================================================
//
// These fixtures exercise the declarative `CapcoScheme` PageRewrite
// catalog via `project_via_scheme`. They DO NOT compare PageContext
// against the lattice path — the parity gate above stays focused on
// per-axis projection equivalence. The fixtures here pin the §H.6 /
// §H.8 / §H.9 strip semantics that PR 4b-C delivered as declarative
// rows.
//
// verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md` per
// Constitution VIII propagation discipline.

#[test]
fn pattern_c_fouo_classified_strip() {
    // CAPCO-2016 §H.8 p134 (FOUO Precedence Rules for Banner Line
    // Guidance, classified-document sub-clause): "FOUO in a classified
    // document: When a classified document contains portions of FOUO
    // information, the FOUO marking is not used in the banner line."
    //
    // Pattern-C row `capco/fouo-evicted-by-classified` + Pattern-B row
    // `capco/classification-evicts-fouo` both fire on this input;
    // their FactRemove[TOK_FOUO] payloads are idempotent.
    let portions = [
        portion_with_dissem_us(Classification::Unclassified, &[DissemControl::Fouo]),
        portion_us(Classification::Secret),
    ];
    let banner = project_via_scheme(&portions);
    assert!(
        !banner.dissem_us.contains(&DissemControl::Fouo),
        "Pattern-C row `capco/fouo-evicted-by-classified` (§H.8 p134) \
         must strip FOUO from the banner dissem axis. banner.dissem_us = {:?}",
        banner.dissem_us,
    );
}

#[test]
fn pattern_c_fouo_unclassified_keeps_when_alone() {
    // §H.8 p134: "FOUO must convey in the banner line if the document
    // is UNCLASSIFIED with FOUO marked information and no other
    // dissemination control markings." FOUO alone keeps; the
    // Pattern-B trigger requires another non-FD&R control.
    let portions = [portion_with_dissem_us(
        Classification::Unclassified,
        &[DissemControl::Fouo],
    )];
    let banner = project_via_scheme(&portions);
    assert!(
        banner.dissem_us.contains(&DissemControl::Fouo),
        "§H.8 p134 unclassified-alone: FOUO must stay in the banner. \
         banner.dissem_us = {:?}",
        banner.dissem_us,
    );
}

#[test]
fn pattern_c_limdis_classified_strip() {
    // CAPCO-2016 §H.9 p170 (LIMITED DISTRIBUTION, Precedence Rules for
    // Banner Line Guidance): "When a document contains LIMDIS and
    // classified portions, LIMDIS is not used in the banner line."
    let mut p_limdis = portion_us(Classification::Unclassified);
    p_limdis.non_ic_dissem = vec![NonIcDissem::Limdis].into_boxed_slice();
    let portions = [p_limdis, portion_us(Classification::Secret)];
    let banner = project_via_scheme(&portions);
    assert!(
        !banner.non_ic_dissem.contains(&NonIcDissem::Limdis),
        "Pattern-C row `capco/limdis-evicted-by-classified` (§H.9 p170) \
         must strip LIMDIS from the banner non_ic axis. \
         banner.non_ic_dissem = {:?}",
        banner.non_ic_dissem,
    );
}

#[test]
fn pattern_c_sbu_classified_strip() {
    // CAPCO-2016 §H.9 p176 (SENSITIVE BUT UNCLASSIFIED, Precedence
    // Rules for Banner Line Guidance): "When a document contains SBU
    // and classified portions, SBU is not used in the banner line."
    let mut p_sbu = portion_us(Classification::Unclassified);
    p_sbu.non_ic_dissem = vec![NonIcDissem::Sbu].into_boxed_slice();
    let portions = [p_sbu, portion_us(Classification::Secret)];
    let banner = project_via_scheme(&portions);
    assert!(
        !banner.non_ic_dissem.contains(&NonIcDissem::Sbu),
        "Pattern-C row `capco/sbu-evicted-by-classified` (§H.9 p176) \
         must strip SBU from the banner non_ic axis. \
         banner.non_ic_dissem = {:?}",
        banner.non_ic_dissem,
    );
}

#[test]
fn pattern_c_dod_ucni_classified_strip_promotes_noforn() {
    // CAPCO-2016 §H.6 p116 (DOD UCNI / DCNI, Precedence Rules for
    // Banner Line Guidance): "Classified documents: DOD UCNI does not
    // appear in the banner line; however, NOFORN must be applied if a
    // less restrictive FD&R marking would otherwise be conveyed with
    // the classified information."
    //
    // This is the load-bearing post-fix test for Commit 2's pre-fix
    // bug — the §H.6 NOFORN-promotion clause was missing from the
    // pre-PR-4b-C PageContext UCNI strip. The declarative
    // `capco/dod-ucni-evicted-by-classified` + `capco/dod-ucni-promotes-noforn-when-classified`
    // pair fixes it.
    let mut p_ucni = portion_us(Classification::Unclassified);
    p_ucni.aea_markings = vec![AeaMarking::DodUcni].into_boxed_slice();
    let portions = [p_ucni, portion_us(Classification::Secret)];
    let banner = project_via_scheme(&portions);
    assert!(
        !banner
            .aea_markings
            .iter()
            .any(|m| matches!(m, AeaMarking::DodUcni)),
        "Pattern-C row `capco/dod-ucni-evicted-by-classified` (§H.6 p116) \
         must strip DOD UCNI from the banner AEA axis. \
         banner.aea_markings = {:?}",
        banner.aea_markings,
    );
    assert!(
        banner.dissem_us.contains(&DissemControl::Nf),
        "Pattern-C row `capco/dod-ucni-promotes-noforn-when-classified` \
         (§H.6 p116) must promote NOFORN onto the banner dissem axis \
         (fixes the pre-PR-4b-C silent strip bug pinned by Commit 2). \
         banner.dissem_us = {:?}",
        banner.dissem_us,
    );
}

#[test]
fn pattern_c_doe_ucni_classified_strip_promotes_noforn() {
    // CAPCO-2016 §H.6 p118 (DOE UCNI, Precedence Rules for Banner
    // Line Guidance): mirrors §H.6 p116 (DOD UCNI) verbatim.
    let mut p_ucni = portion_us(Classification::Unclassified);
    p_ucni.aea_markings = vec![AeaMarking::DoeUcni].into_boxed_slice();
    let portions = [p_ucni, portion_us(Classification::Secret)];
    let banner = project_via_scheme(&portions);
    assert!(
        !banner
            .aea_markings
            .iter()
            .any(|m| matches!(m, AeaMarking::DoeUcni)),
        "Pattern-C row `capco/doe-ucni-evicted-by-classified` (§H.6 p118) \
         must strip DOE UCNI from the banner AEA axis. \
         banner.aea_markings = {:?}",
        banner.aea_markings,
    );
    assert!(
        banner.dissem_us.contains(&DissemControl::Nf),
        "Pattern-C row `capco/doe-ucni-promotes-noforn-when-classified` \
         (§H.6 p118) must promote NOFORN onto the banner dissem axis. \
         banner.dissem_us = {:?}",
        banner.dissem_us,
    );
}

#[test]
fn pattern_c_dod_ucni_classified_with_explicit_noforn_no_double_inject() {
    // §H.6 p116: NOFORN promotion fires "if a less restrictive FD&R
    // marking would otherwise be conveyed". When NOFORN is already
    // present on the dissem axis, the promote-row's predicate
    // (`dod_ucni_promotes_noforn_trigger`'s `!dissem_has_noforn`
    // check) suppresses the FactAdd. The strip-row still fires.
    let mut p_ucni = portion_us(Classification::Unclassified);
    p_ucni.aea_markings = vec![AeaMarking::DodUcni].into_boxed_slice();
    let mut p_class = portion_us(Classification::Secret);
    p_class.dissem_us = vec![DissemControl::Nf].into_boxed_slice();
    let portions = [p_ucni, p_class];
    let banner = project_via_scheme(&portions);
    let nf_count = banner
        .dissem_us
        .iter()
        .filter(|d| matches!(d, DissemControl::Nf))
        .count();
    assert!(
        !banner
            .aea_markings
            .iter()
            .any(|m| matches!(m, AeaMarking::DodUcni)),
        "DOD UCNI must still be stripped on a classified page even when \
         NOFORN is already present. banner.aea_markings = {:?}",
        banner.aea_markings,
    );
    assert_eq!(
        nf_count, 1,
        "NOFORN must appear exactly once on the banner dissem axis — \
         the existing portion-NOFORN dedupes via the lattice union and \
         the promote-row's predicate suppresses double-inject. \
         banner.dissem_us = {:?}",
        banner.dissem_us,
    );
}

#[test]
fn pattern_c_sbu_nf_classified_strip_promotes_noforn() {
    // #541 — §H.9 p178 line 4421 (SBU NOFORN Commingling Rule(s)
    // Within a Portion): "If the portion is classified, the
    // classification level of the portion adequately protects the
    // SBU information, so SBU is not reflected in the portion mark;
    // however a NOFORN marking must be added to the portion mark,
    // e.g., (C//NF)."
    //
    // This fixture pins the post-#541 semantic on the scheme path:
    // SBU-NF + classified page → both `capco/sbu-nf-evicted-by-
    // classified` (Pattern-C) and `capco/sbu-nf-implies-noforn`
    // (Pattern-A) fire together, producing `non_ic_dissem = []` +
    // `dissem_us = [Nf]`. The pre-#541 behavior was wrong: SbuNf
    // persisted in non_ic_dissem because Pattern-C's bare-Sbu
    // trigger didn't match the compound variant and the
    // transmutation_stubs Entry 6a was a `never_fires`/`noop_action`
    // stub.
    let mut p_sbu_nf = portion_us(Classification::Unclassified);
    p_sbu_nf.non_ic_dissem = vec![NonIcDissem::SbuNf].into_boxed_slice();
    let portions = [p_sbu_nf, portion_us(Classification::Secret)];
    let banner = project_via_scheme(&portions);
    assert!(
        !banner.non_ic_dissem.contains(&NonIcDissem::SbuNf),
        "§H.9 p178 line 4421: Pattern-C row \
         `capco/sbu-nf-evicted-by-classified` must strip SBU-NF \
         from the banner non_ic axis. banner.non_ic_dissem = {:?}",
        banner.non_ic_dissem,
    );
    assert!(
        !banner.non_ic_dissem.contains(&NonIcDissem::Sbu),
        "§H.9 p178 line 4421: bare SBU must not be introduced \
         either — the §3.5 carve-out for SBU-NF is a pure removal, \
         not a transmutation. banner.non_ic_dissem = {:?}",
        banner.non_ic_dissem,
    );
    assert!(
        banner.dissem_us.contains(&DissemControl::Nf),
        "§H.9 p178 line 4421: Pattern-A row \
         `capco/sbu-nf-implies-noforn` must promote NOFORN onto \
         the banner dissem axis. banner.dissem_us = {:?}",
        banner.dissem_us,
    );
}

#[test]
fn parity_classified_sbu_nf_lattice_and_scheme_both_drop_sbu_nf() {
    // #541 — convergence gate: post-fix the lattice path (via
    // `NonIcDissemSet::from_attrs_iter`'s classified branch) and
    // the scheme path (via the new `capco/sbu-nf-evicted-by-
    // classified` Pattern-C row + the existing `capco/sbu-nf-
    // implies-noforn` Pattern-A row) MUST produce byte-identical
    // CanonicalAttrs on every axis. Pre-#541 both paths were wrong
    // in different ways (lattice kept bare Sbu; scheme kept compound
    // SbuNf) — divergence was masked because no fixture asserted
    // byte-identity on this input.
    //
    // The `dissem_us` divergence carve-out in the file's hoisted
    // rationale (§B.3 Table 2 p21 caveated-classified closure)
    // applies to caveated content; SBU-NF is non-IC dissem, not
    // ORCON/IMCON/etc., and the input isn't on the caveated
    // closure path. Expect byte-identity on every axis.
    let mut p_sbu_nf = portion_us(Classification::Unclassified);
    p_sbu_nf.non_ic_dissem = vec![NonIcDissem::SbuNf].into_boxed_slice();
    let portions = [p_sbu_nf, portion_us(Classification::Secret)];
    assert_byte_identity(
        "parity_classified_sbu_nf_lattice_and_scheme_both_drop_sbu_nf",
        &project_via_lattice(&portions),
        &project_via_scheme(&portions),
        &[],
    );
}

#[test]
fn parity_classified_les_nf_lattice_and_scheme_both_retain_les() {
    // #541 — LES asymmetry pin: per §H.9 p185 line 4557-4558 LES
    // survives classification (unlike SBU per §H.9 p178 line 4421).
    // Both projection paths MUST retain `Les` in `non_ic_dissem`
    // and inject NOFORN into `dissem_us` — the canonical worked
    // example from §H.9 p185 is `SECRET//NOFORN//LES`.
    //
    // This is the negative-regression gate against a future
    // change-of-mind that mistakenly treats LES-NF as symmetric
    // with SBU-NF. The asymmetry traces to LES carrying independent
    // law-enforcement legal-process discipline (warning statements,
    // originator-control, prohibition on legal-proceedings use per
    // §H.9 p184) that classification doesn't subsume; SBU is purely
    // admin-protection that classification does subsume. See
    // `NonIcDissemSet`'s type-level doc-comment in
    // `crates/capco/src/lattice.rs` for the full rationale.
    let mut p_les_nf = portion_us(Classification::Unclassified);
    p_les_nf.non_ic_dissem = vec![NonIcDissem::LesNf].into_boxed_slice();
    let portions = [p_les_nf, portion_us(Classification::Secret)];
    let lat = project_via_lattice(&portions);
    let scheme = project_via_scheme(&portions);
    // Whole-attrs byte-identity: every axis must agree (no expected
    // divergence). This is the authoritative gate against silent
    // divergence on axes the LES-specific field checks below do
    // not inspect (sci_controls, sar_markings, fgi_marker,
    // declass_exemption, etc.).
    assert_byte_identity(
        "parity_classified_les_nf_lattice_and_scheme_both_retain_les",
        &lat,
        &scheme,
        &[],
    );
    // Field-level documentation of the load-bearing properties: LES
    // survives, NOFORN is injected, LES-NF is transformed away.
    // These assertions also serve as faster failure-mode triage when
    // the test trips — `assert_byte_identity`'s diff message lists
    // raw values; these messages cite §H.9 p185 directly.
    assert!(
        lat.non_ic_dissem.contains(&NonIcDissem::Les),
        "§H.9 p185: lattice must retain Les on classified pages. \
         lat.non_ic_dissem = {:?}",
        lat.non_ic_dissem,
    );
    assert!(
        scheme.non_ic_dissem.contains(&NonIcDissem::Les),
        "§H.9 p185: scheme must retain Les on classified pages. \
         scheme.non_ic_dissem = {:?}",
        scheme.non_ic_dissem,
    );
    assert!(
        lat.dissem_us.contains(&DissemControl::Nf),
        "§H.9 p185: lattice must inject NOFORN. \
         lat.dissem_us = {:?}",
        lat.dissem_us,
    );
    assert!(
        scheme.dissem_us.contains(&DissemControl::Nf),
        "§H.9 p185: scheme must inject NOFORN. \
         scheme.dissem_us = {:?}",
        scheme.dissem_us,
    );
    assert!(
        !lat.non_ic_dissem.contains(&NonIcDissem::LesNf),
        "LES-NF must be transformed away (Les + NOFORN). \
         lat.non_ic_dissem = {:?}",
        lat.non_ic_dissem,
    );
    assert!(
        !scheme.non_ic_dissem.contains(&NonIcDissem::LesNf),
        "LES-NF must be transformed away (Les + NOFORN). \
         scheme.non_ic_dissem = {:?}",
        scheme.non_ic_dissem,
    );
}

// ===========================================================================
// #552 — same-axis compound-supersedes-bare supersession (4 cases)
// ===========================================================================
//
// §H.9 p178 (SBU NOFORN Precedence Rules for Banner Line Guidance):
// "When a document contains both SBU-NF and SBU portions, SBU NOFORN
// supersedes SBU in the banner line."
// §H.9 p185 derivation (banner-form heading + Notional Example Page 1):
// `(U//LES-NF)` rolls up to banner `UNCLASSIFIED//LES NOFORN`; LES-NF
// compound carries the LES family marker on the unclassified banner so
// bare LES is redundant on co-presence.
//
// Behavior matrix (composed with the existing #541 classified gate):
// | Input            | Unclassified         | Classified                   |
// |------------------|----------------------|------------------------------|
// | {Sbu, SbuNf}     | {SbuNf}              | {} + NF (banner SECRET//NF)  |
// | {Les, LesNf}     | {LesNf}              | {Les} + NF (S//NF//LES)      |
//
// The scheme path's `non_ic_dissem` projection flows through the same
// `NonIcDissemSet::from_attrs_iter` helper as the lattice path
// (`marking.rs:587`), so the #552 fix lands on both paths
// automatically. The four parity fixtures gate against future drift
// in the shared helper or in the new
// `capco/{sbu-nf,les-nf}-supersedes-*` scheme-side rewrites.

#[test]
fn parity_unclassified_sbu_co_present_lattice_and_scheme_both_drop_bare_sbu() {
    // §H.9 p178: bare SBU dropped on co-presence with SBU-NF.
    // Unclassified output: `{SbuNf}` only; banner
    // `UNCLASSIFIED//SBU NOFORN`. #552.
    //
    // Post-#554: byte-identity on every axis (no `dissem_us`
    // divergence). The Pattern-A `capco/sbu-nf-implies-noforn` row
    // now gates on `is_classified` per `sbu_nf_classified_trigger`,
    // so the unclassified compound's intrinsic NOFORN identity is
    // not double-counted onto the dissem axis. Both projection
    // paths agree: `non_ic_dissem = {SbuNf}`, `dissem_us = {}`,
    // banner `UNCLASSIFIED//SBU NOFORN` per the §H.9 p178 Example
    // Banner Line.
    let mut p_sbu = portion_us(Classification::Unclassified);
    p_sbu.non_ic_dissem = vec![NonIcDissem::Sbu].into_boxed_slice();
    let mut p_sbu_nf = portion_us(Classification::Unclassified);
    p_sbu_nf.non_ic_dissem = vec![NonIcDissem::SbuNf].into_boxed_slice();
    let portions = [p_sbu, p_sbu_nf];
    let lat = project_via_lattice(&portions);
    let scheme = project_via_scheme(&portions);
    assert_byte_identity(
        "parity_unclassified_sbu_co_present_lattice_and_scheme_both_drop_bare_sbu",
        &lat,
        &scheme,
        &[],
    );
    // Both paths agree on the load-bearing #552 axis: non_ic_dissem
    // drops bare Sbu and retains SbuNf.
    assert!(
        !lat.non_ic_dissem.contains(&NonIcDissem::Sbu),
        "§H.9 p178: lattice must drop bare Sbu on co-presence with SbuNf. \
         lat.non_ic_dissem = {:?}",
        lat.non_ic_dissem,
    );
    assert!(
        !scheme.non_ic_dissem.contains(&NonIcDissem::Sbu),
        "§H.9 p178: scheme must drop bare Sbu on co-presence with SbuNf. \
         scheme.non_ic_dissem = {:?}",
        scheme.non_ic_dissem,
    );
    assert!(
        lat.non_ic_dissem.contains(&NonIcDissem::SbuNf),
        "§H.9 p178: compound SbuNf must survive on unclassified pages. \
         lat.non_ic_dissem = {:?}",
        lat.non_ic_dissem,
    );
    assert!(
        scheme.non_ic_dissem.contains(&NonIcDissem::SbuNf),
        "§H.9 p178: scheme must retain compound SbuNf on unclassified pages. \
         scheme.non_ic_dissem = {:?}",
        scheme.non_ic_dissem,
    );
}

#[test]
fn parity_unclassified_les_co_present_lattice_and_scheme_both_drop_bare_les() {
    // §H.9 p185: bare LES dropped on co-presence with LES-NF.
    // Unclassified output: `{LesNf}` only; banner
    // `UNCLASSIFIED//LES NOFORN`. #552.
    //
    // Post-#554: byte-identity on every axis (no `dissem_us`
    // divergence). The Pattern-A `capco/les-nf-implies-noforn` row
    // now gates on `is_classified` per `les_nf_classified_trigger`,
    // so the unclassified compound's intrinsic NOFORN identity is
    // not double-counted onto the dissem axis. Both projection
    // paths agree: `non_ic_dissem = {LesNf}`, `dissem_us = {}`,
    // banner `UNCLASSIFIED//LES NOFORN` per the §H.9 p185 Example
    // Banner Line + Notional Example Page 1.
    let mut p_les = portion_us(Classification::Unclassified);
    p_les.non_ic_dissem = vec![NonIcDissem::Les].into_boxed_slice();
    let mut p_les_nf = portion_us(Classification::Unclassified);
    p_les_nf.non_ic_dissem = vec![NonIcDissem::LesNf].into_boxed_slice();
    let portions = [p_les, p_les_nf];
    let lat = project_via_lattice(&portions);
    let scheme = project_via_scheme(&portions);
    assert_byte_identity(
        "parity_unclassified_les_co_present_lattice_and_scheme_both_drop_bare_les",
        &lat,
        &scheme,
        &[],
    );
    assert!(
        !lat.non_ic_dissem.contains(&NonIcDissem::Les),
        "§H.9 p185: lattice must drop bare Les on co-presence with LesNf. \
         lat.non_ic_dissem = {:?}",
        lat.non_ic_dissem,
    );
    assert!(
        !scheme.non_ic_dissem.contains(&NonIcDissem::Les),
        "§H.9 p185: scheme must drop bare Les on co-presence with LesNf. \
         scheme.non_ic_dissem = {:?}",
        scheme.non_ic_dissem,
    );
    assert!(
        lat.non_ic_dissem.contains(&NonIcDissem::LesNf),
        "§H.9 p185: compound LesNf must survive on unclassified pages. \
         lat.non_ic_dissem = {:?}",
        lat.non_ic_dissem,
    );
    assert!(
        scheme.non_ic_dissem.contains(&NonIcDissem::LesNf),
        "§H.9 p185: scheme must retain compound LesNf on unclassified pages. \
         scheme.non_ic_dissem = {:?}",
        scheme.non_ic_dissem,
    );
}

#[test]
fn parity_classified_sbu_co_present_lattice_and_scheme_both_strip_to_empty() {
    // #552 + #541 interaction on classified pages with both bare SBU
    // and compound SBU-NF: #552 supersession drops bare SBU →
    // `{SbuNf}`; #541 classified gate strips SbuNf → `{}` + NOFORN
    // injection. Net banner `SECRET//NOFORN`. §H.9 p178.
    let mut p_sbu = portion_us(Classification::Secret);
    p_sbu.non_ic_dissem = vec![NonIcDissem::Sbu].into_boxed_slice();
    let mut p_sbu_nf = portion_us(Classification::Secret);
    p_sbu_nf.non_ic_dissem = vec![NonIcDissem::SbuNf].into_boxed_slice();
    let portions = [p_sbu, p_sbu_nf];
    let lat = project_via_lattice(&portions);
    let scheme = project_via_scheme(&portions);
    assert_byte_identity(
        "parity_classified_sbu_co_present_lattice_and_scheme_both_strip_to_empty",
        &lat,
        &scheme,
        &[],
    );
    assert!(
        lat.non_ic_dissem.is_empty(),
        "§H.9 p178: classified strip after #552 supersession must \
         leave the non-IC set empty. lat.non_ic_dissem = {:?}",
        lat.non_ic_dissem,
    );
    assert!(
        lat.dissem_us.contains(&DissemControl::Nf),
        "§H.9 p178: NOFORN must be injected on classified SBU-NF strip. \
         lat.dissem_us = {:?}",
        lat.dissem_us,
    );
}

#[test]
fn parity_classified_les_co_present_lattice_and_scheme_both_split_to_bare_les() {
    // #552 + #541 interaction on classified pages with both bare LES
    // and compound LES-NF: #552 supersession drops bare LES →
    // `{LesNf}`; #541 classified gate splits LesNf into bare Les +
    // NOFORN injection. Net banner `SECRET//NOFORN//LES`. §H.9 p185.
    let mut p_les = portion_us(Classification::Secret);
    p_les.non_ic_dissem = vec![NonIcDissem::Les].into_boxed_slice();
    let mut p_les_nf = portion_us(Classification::Secret);
    p_les_nf.non_ic_dissem = vec![NonIcDissem::LesNf].into_boxed_slice();
    let portions = [p_les, p_les_nf];
    let lat = project_via_lattice(&portions);
    let scheme = project_via_scheme(&portions);
    assert_byte_identity(
        "parity_classified_les_co_present_lattice_and_scheme_both_split_to_bare_les",
        &lat,
        &scheme,
        &[],
    );
    assert!(
        lat.non_ic_dissem.contains(&NonIcDissem::Les),
        "§H.9 p185: classified split after #552 supersession must \
         leave bare Les in the set. lat.non_ic_dissem = {:?}",
        lat.non_ic_dissem,
    );
    assert!(
        !lat.non_ic_dissem.contains(&NonIcDissem::LesNf),
        "§H.9 p185: LesNf must be transformed away (Les + NOFORN) on \
         classified pages. lat.non_ic_dissem = {:?}",
        lat.non_ic_dissem,
    );
    assert!(
        lat.dissem_us.contains(&DissemControl::Nf),
        "§H.9 p185: NOFORN must be injected on classified LES-NF split. \
         lat.dissem_us = {:?}",
        lat.dissem_us,
    );
}

#[test]
fn pattern_c_les_in_classified_propagates_to_banner() {
    // CAPCO-2016 §H.9 p181 (LAW ENFORCEMENT SENSITIVE, Precedence
    // Rules for Banner Line Guidance): "The LES marking always appears
    // in the banner line if LES information ... is contained in the
    // document, regardless of the document's classification level."
    //
    // Pattern-C explicitly EXCLUDES LES (the PM-confirmed §H.9 p181
    // exception). This fixture is the regression-gate against a
    // future "LES classified strip" accidentally being added.
    let mut p_les = portion_us(Classification::Unclassified);
    p_les.non_ic_dissem = vec![NonIcDissem::Les].into_boxed_slice();
    let portions = [p_les, portion_us(Classification::Secret)];
    let banner = project_via_scheme(&portions);
    assert!(
        banner.non_ic_dissem.contains(&NonIcDissem::Les),
        "§H.9 p181: LES propagates to the banner regardless of \
         classification level — Pattern-C must NOT strip LES. \
         banner.non_ic_dissem = {:?}",
        banner.non_ic_dissem,
    );
}

#[test]
fn pattern_b_fouo_with_dsen_unclassified_strip() {
    // §H.8 p134: "FOUO is not conveyed in the banner line if the
    // document is UNCLASSIFIED with FOUO and other dissemination
    // control markings, excluding any FD&R markings."
    //
    // DSEN is a non-FD&R IC dissem control (§H.8 p159), so the
    // Pattern-B `capco/non-fdr-control-evicts-fouo` row fires.
    let portions = [portion_with_dissem_us(
        Classification::Unclassified,
        &[DissemControl::Fouo, DissemControl::Dsen],
    )];
    let banner = project_via_scheme(&portions);
    assert!(
        !banner.dissem_us.contains(&DissemControl::Fouo),
        "Pattern-B row `capco/non-fdr-control-evicts-fouo` (§H.8 p134) \
         must strip FOUO when DSEN is present on an UNCLASSIFIED page. \
         banner.dissem_us = {:?}",
        banner.dissem_us,
    );
    assert!(
        banner.dissem_us.contains(&DissemControl::Dsen),
        "DSEN must be retained — Pattern-B strips FOUO, not the trigger. \
         banner.dissem_us = {:?}",
        banner.dissem_us,
    );
}

#[test]
fn pattern_b_fouo_with_orcon_unclassified_strip() {
    // §H.8 p134 + §H.8 p136 (ORCON): ORCON is a non-FD&R IC dissem
    // control, so the Pattern-B trigger fires on a UNCLASSIFIED page
    // carrying FOUO + ORCON.
    let portions = [portion_with_dissem_us(
        Classification::Unclassified,
        &[DissemControl::Fouo, DissemControl::Oc],
    )];
    let banner = project_via_scheme(&portions);
    assert!(
        !banner.dissem_us.contains(&DissemControl::Fouo),
        "Pattern-B row `capco/non-fdr-control-evicts-fouo` (§H.8 p134 + \
         §H.8 p136) must strip FOUO when ORCON is present on an \
         UNCLASSIFIED page. banner.dissem_us = {:?}",
        banner.dissem_us,
    );
    assert!(
        banner.dissem_us.contains(&DissemControl::Oc),
        "ORCON must be retained — Pattern-B strips FOUO, not the trigger. \
         banner.dissem_us = {:?}",
        banner.dissem_us,
    );
}

#[test]
fn pattern_b_fouo_with_relido_unclassified_keeps_fouo() {
    // §H.8 p134: "excluding any FD&R markings". RELIDO is FD&R-set
    // membership (§B.3.a p19; `FDR_DOMINATORS` includes RELIDO), so
    // the Pattern-B `capco/non-fdr-control-evicts-fouo` predicate
    // does NOT fire on RELIDO+FOUO. FOUO stays.
    //
    // This is the load-bearing test for the
    // `is_fdr_dissem_token` helper's broad-membership semantic
    // (matches `Vocabulary::is_fdr_dissem`, INCLUDES RELIDO; NOT
    // `is_fdr_dominator` which EXCLUDES RELIDO).
    let portions = [portion_with_dissem_us(
        Classification::Unclassified,
        &[DissemControl::Fouo, DissemControl::Relido],
    )];
    let banner = project_via_scheme(&portions);
    assert!(
        banner.dissem_us.contains(&DissemControl::Fouo),
        "§H.8 p134: FOUO must STAY in the banner when only FD&R \
         markings accompany it (RELIDO is FD&R per §B.3.a p19; \
         `FDR_DOMINATORS` includes RELIDO). banner.dissem_us = {:?}",
        banner.dissem_us,
    );
}

#[test]
fn pattern_b_fouo_with_noforn_unclassified_keeps_fouo() {
    // §H.8 p134: "excluding any FD&R markings". NOFORN is the
    // canonical FD&R member; the Pattern-B trigger excludes it. FOUO
    // stays.
    let portions = [portion_with_dissem_us(
        Classification::Unclassified,
        &[DissemControl::Fouo, DissemControl::Nf],
    )];
    let banner = project_via_scheme(&portions);
    assert!(
        banner.dissem_us.contains(&DissemControl::Fouo),
        "§H.8 p134: FOUO must STAY in the banner when only NOFORN \
         accompanies it (FD&R-only context). banner.dissem_us = {:?}",
        banner.dissem_us,
    );
}

#[test]
fn pattern_b_fouo_with_aea_unclassified_strip() {
    // §H.8 p134 + the Pattern-B four-axis "other control" reading:
    // AEA markings (RD / FRD / TFNI / UCNI / ATOMAL) are atomic-
    // energy controls, not FD&R markings. The Pattern-B trigger's
    // `!attrs.aea_markings.is_empty()` clause fires.
    let mut p = portion_us(Classification::Unclassified);
    p.dissem_us = vec![DissemControl::Fouo].into_boxed_slice();
    p.aea_markings = vec![AeaMarking::DodUcni].into_boxed_slice();
    let portions = [p];
    let banner = project_via_scheme(&portions);
    assert!(
        !banner.dissem_us.contains(&DissemControl::Fouo),
        "Pattern-B row `capco/non-fdr-control-evicts-fouo` (§H.8 p134) \
         must strip FOUO when an AEA marking (UCNI here) is present. \
         banner.dissem_us = {:?}",
        banner.dissem_us,
    );
}

#[test]
fn pattern_b_fouo_with_non_ic_unclassified_strip() {
    // §H.8 p134 + the Pattern-B four-axis reading: non-IC dissem
    // tokens (SSI / LIMDIS / LES / SBU / NODIS / EXDIS / NNPI /
    // SbuNf / LesNf) are non-FD&R by construction; the Pattern-B
    // trigger's `!attrs.non_ic_dissem.is_empty()` clause fires.
    let mut p = portion_us(Classification::Unclassified);
    p.dissem_us = vec![DissemControl::Fouo].into_boxed_slice();
    p.non_ic_dissem = vec![NonIcDissem::Ssi].into_boxed_slice();
    let portions = [p];
    let banner = project_via_scheme(&portions);
    assert!(
        !banner.dissem_us.contains(&DissemControl::Fouo),
        "Pattern-B row `capco/non-fdr-control-evicts-fouo` (§H.8 p134 + \
         §H.9 p189 SSI) must strip FOUO when a non-IC dissem control \
         (SSI here) is present. banner.dissem_us = {:?}",
        banner.dissem_us,
    );
}

#[test]
fn pattern_b_fouo_classified_alone_strips_via_classification_row() {
    // §H.8 p134 classified-document sub-clause: FOUO alone on a
    // classified page is stripped via the Pattern-B
    // `capco/classification-evicts-fouo` row (and equivalently via
    // Pattern-C `capco/fouo-evicted-by-classified` — both rows are
    // scheduler-siblings producing the same FactRemove[TOK_FOUO]
    // payload, idempotently).
    let portions = [portion_with_dissem_us(
        Classification::Secret,
        &[DissemControl::Fouo],
    )];
    let banner = project_via_scheme(&portions);
    assert!(
        !banner.dissem_us.contains(&DissemControl::Fouo),
        "Pattern-B row `capco/classification-evicts-fouo` (§H.8 p134) + \
         Pattern-C row `capco/fouo-evicted-by-classified` (§H.8 p134) \
         must both strip FOUO on a classified single-portion page. \
         banner.dissem_us = {:?}",
        banner.dissem_us,
    );
}

#[test]
fn pattern_b_fouo_with_propin_unclassified_strip() {
    // §H.8 p134 + PROPIN as a non-FD&R IC dissem control. The
    // Pattern-B `capco/non-fdr-control-evicts-fouo` row's
    // `dissem_has_non_fdr_other_than_fouo` predicate scan finds
    // DissemControl::Pr → TOK_PROPIN (TOK_PROPIN=143 sentinel added
    // in PR 4b-C Commit 1) and the broad-membership
    // `is_fdr_dissem_token` helper correctly returns false for
    // PROPIN (it's a control marking, not FD&R-set).
    //
    // verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`
    // §H.8 p134 + §H.8 PROPIN entry.
    let portions = [portion_with_dissem_us(
        Classification::Unclassified,
        &[DissemControl::Fouo, DissemControl::Pr],
    )];
    let banner = project_via_scheme(&portions);
    assert!(
        !banner.dissem_us.contains(&DissemControl::Fouo),
        "Pattern-B row `capco/non-fdr-control-evicts-fouo` (§H.8 p134) \
         must strip FOUO when PROPIN is present on an UNCLASSIFIED \
         page. banner.dissem_us = {:?}",
        banner.dissem_us,
    );
    assert!(
        banner.dissem_us.contains(&DissemControl::Pr),
        "PROPIN must be retained — Pattern-B strips FOUO, not the \
         trigger. banner.dissem_us = {:?}",
        banner.dissem_us,
    );
}

#[test]
fn pattern_b_fouo_with_fisa_unclassified_strip() {
    // §H.8 p134 + FISA as a non-FD&R IC dissem control. The
    // Pattern-B predicate scan finds DissemControl::Fisa →
    // TOK_FISA (TOK_FISA=144 sentinel added in PR 4b-C Commit 1).
    // FISA is not FD&R-set; `is_fdr_dissem_token` returns false.
    //
    // verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`
    // §H.8 p134 + §H.8 FISA entry.
    let portions = [portion_with_dissem_us(
        Classification::Unclassified,
        &[DissemControl::Fouo, DissemControl::Fisa],
    )];
    let banner = project_via_scheme(&portions);
    assert!(
        !banner.dissem_us.contains(&DissemControl::Fouo),
        "Pattern-B row `capco/non-fdr-control-evicts-fouo` (§H.8 p134) \
         must strip FOUO when FISA is present on an UNCLASSIFIED \
         page. banner.dissem_us = {:?}",
        banner.dissem_us,
    );
    assert!(
        banner.dissem_us.contains(&DissemControl::Fisa),
        "FISA must be retained — Pattern-B strips FOUO, not the \
         trigger. banner.dissem_us = {:?}",
        banner.dissem_us,
    );
}

#[test]
fn pattern_b_fouo_with_rawfisa_unclassified_strip() {
    // §H.8 p134 + RAW FISA as a non-FD&R IC dissem control. The
    // Pattern-B predicate scan finds DissemControl::Rawfisa →
    // TOK_RAWFISA (TOK_RAWFISA=145 sentinel added in PR 4b-C
    // Commit 1). RAW FISA is not FD&R-set; `is_fdr_dissem_token`
    // returns false.
    //
    // verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`
    // §H.8 p134 + §H.8 RAW FISA entry.
    let portions = [portion_with_dissem_us(
        Classification::Unclassified,
        &[DissemControl::Fouo, DissemControl::Rawfisa],
    )];
    let banner = project_via_scheme(&portions);
    assert!(
        !banner.dissem_us.contains(&DissemControl::Fouo),
        "Pattern-B row `capco/non-fdr-control-evicts-fouo` (§H.8 p134) \
         must strip FOUO when RAW FISA is present on an UNCLASSIFIED \
         page. banner.dissem_us = {:?}",
        banner.dissem_us,
    );
    assert!(
        banner.dissem_us.contains(&DissemControl::Rawfisa),
        "RAW FISA must be retained — Pattern-B strips FOUO, not the \
         trigger. banner.dissem_us = {:?}",
        banner.dissem_us,
    );
}

// ===========================================================================
// PR 4b-C consolidation — declaration-order pin tests
// ===========================================================================
//
// These tests pin two declaration-order invariants the runtime
// evaluator relies on, both documented inline at the relevant row
// doc-comments in `crates/capco/src/scheme.rs`. They are deliberately
// position-pin tests, not lattice-output tests — the runtime semantics
// are exercised by the Pattern-C fixtures above; these guard against
// a future refactor that quietly reorders the catalog into a
// scheduler-legal but runtime-broken state.

#[test]
fn pin_ucni_promote_before_strip_declaration_order() {
    // Order-dependency: the promote rows' predicates read
    // `attrs.aea_markings` (via `has_dod_ucni` / `has_doe_ucni`) and
    // would observe an empty axis if the strip rows had already
    // fired. The Kahn scheduler accepts either order (promote writes
    // CAT_DISSEM, strip writes CAT_AEA — both independent of the
    // other's writes), so the runtime correctness comes from the
    // position of the rows in the declaration `Vec`, not from the
    // scheduler's topological resolution.
    //
    // See `scheme.rs` doc-comment on
    // `capco/dod-ucni-promotes-noforn-when-classified` for the full
    // rationale.
    let scheme = CapcoScheme::new();
    let rewrites = scheme.page_rewrites();

    let pos = |id: &str| {
        rewrites
            .iter()
            .position(|r| r.id == id)
            .unwrap_or_else(|| panic!("rewrite {id} not declared"))
    };

    let dod_promote = pos("capco/dod-ucni-promotes-noforn-when-classified");
    let dod_strip = pos("capco/dod-ucni-evicted-by-classified");
    assert!(
        dod_promote < dod_strip,
        "DOD UCNI promote must be declared before DOD UCNI strip — \
         promote predicate reads aea_markings which strip would clear \
         (§H.6 p116). promote_pos={dod_promote}, strip_pos={dod_strip}"
    );

    let doe_promote = pos("capco/doe-ucni-promotes-noforn-when-classified");
    let doe_strip = pos("capco/doe-ucni-evicted-by-classified");
    assert!(
        doe_promote < doe_strip,
        "DOE UCNI promote must be declared before DOE UCNI strip — \
         promote predicate reads aea_markings which strip would clear \
         (§H.6 p118). promote_pos={doe_promote}, strip_pos={doe_strip}"
    );
}

#[test]
fn pin_pattern_b_row_2_before_noforn_clears_fdr_family() {
    // Cycle-workaround invariant: Pattern-B row 2
    // `capco/non-fdr-control-evicts-fouo` omits CAT_DISSEM from its
    // `reads` annotation (the predicate scans it but declaring it
    // would create a 2-row cycle with `capco/noforn-clears-fdr-family`,
    // which reads + writes CAT_DISSEM as a 1-row self-edge the
    // scheduler accepts). Correctness then requires Pattern-B row 2
    // to fire BEFORE NOFORN-injecting rewrites; the scheduler cannot
    // enforce this through the omitted edge, so we pin the
    // declaration position instead.
    //
    // Additionally, `FDR_DOMINATORS` membership (consumed by the
    // broad-set `is_fdr_dissem_token` helper) must stay complete —
    // a missing FD&R variant would let Pattern-B row 2 fire on a
    // pure FD&R+FOUO page and incorrectly strip FOUO. The
    // `pattern_b_fouo_with_{relido,noforn}_unclassified_keeps_fouo`
    // fixtures above pin that side.
    //
    // See `scheme.rs::PATTERN_B_NON_FDR_READS` doc-comment + plan
    // §3.4 risk #4 for the full rationale.
    let scheme = CapcoScheme::new();
    let rewrites = scheme.page_rewrites();

    let pos = |id: &str| {
        rewrites
            .iter()
            .position(|r| r.id == id)
            .unwrap_or_else(|| panic!("rewrite {id} not declared"))
    };

    let pattern_b_row_2 = pos("capco/non-fdr-control-evicts-fouo");
    let noforn_clears_fdr = pos("capco/noforn-clears-fdr-family");
    assert!(
        pattern_b_row_2 < noforn_clears_fdr,
        "Pattern-B row 2 must be declared before `noforn-clears-fdr-family` \
         — cycle-workaround invariant (CAT_DISSEM omitted from reads, \
         predicate-scan only, runtime order pinned via declaration \
         position). pattern_b_row_2_pos={pattern_b_row_2}, \
         noforn_clears_fdr_pos={noforn_clears_fdr}"
    );
}
