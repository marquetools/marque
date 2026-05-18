// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Golden tests derived from the ODNI ISM-Rollup XSpec test suite.
//!
//! Each test translates an authoritative XSpec scenario into a Rust test.
//! Uses Default::default() + field mutation since CanonicalAttrs is #[non_exhaustive].
//!
//! # PR 4b-E migration note
//!
//! Pre-PR-4b-E these tests read `PageContext::expected_*` accessors
//! directly. Post-PR-4b-E those accessors retired in favor of the
//! lattice-native helpers in `marque-capco::lattice` and the
//! scheme-level `render_canonical(Scope::Page, ...)` renderer. The
//! tests now construct `CanonicalAttrs` portions and exercise the
//! same axes via:
//!
//! - `ClassificationLattice::from_attrs_iter(&[CanonicalAttrs])` for
//!   the max-classification roll-up.
//! - `AeaSet::from_markings` / `AeaSet::to_markings` for AEA union.
//! - `DissemSet::from_attrs_iter` (US-attributed dissem) and the
//!   `with_noforn_injected` overlay for OC-USGOV / FOUO / NOFORN.
//! - `NonIcDissemSet::from_attrs_iter` for SBU-NF / LES-NF split +
//!   NODIS / EXDIS NF-injection (returns `(set, needs_nf)`).
//! - `RelToBlock::from_attrs_iter` for REL TO intersection.
//! - `FgiSet::from_attrs_iter` for source-concealed-dominates +
//!   acknowledged-union FGI.
//! - `sci_controls_from_markings(&[CanonicalAttrs])` for the flat
//!   SCI CVE projection union.
//! - `CapcoScheme::render_banner(scheme.project(Scope::Page, &markings))`
//!   for the banner-string roll-up (replaces `render_expected_banner`).
//!
//! These golden tests live in `marque-ism` for the historical reason
//! that PageContext lived there. Post-PR-4b-E the lattice helpers
//! live in `marque-capco`; the tests are now ism-side dev-dep
//! consumers of the capco helpers. Test signatures and intent are
//! preserved bit-for-bit so the XSpec correspondence stays auditable.

use marque_capco::CapcoMarking;
use marque_capco::lattice::{
    AeaSet, ClassificationLattice, DissemSet, FgiSet, NonIcDissemSet, RelToBlock,
    sci_controls_from_markings,
};
use marque_capco::scheme::CapcoScheme;
use marque_ism::attrs::*;
use marque_ism::CanonicalAttrs;
use marque_scheme::{MarkingScheme as _, Scope};

fn portion(c: Classification) -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(c));
    a
}

fn render_banner_from_portions(portions: &[CanonicalAttrs]) -> String {
    let scheme = CapcoScheme::new();
    let markings: Vec<CapcoMarking> =
        portions.iter().cloned().map(CapcoMarking::new).collect();
    let projected = scheme.project(Scope::Page, &markings);
    scheme.render_banner(&projected)
}

// =========================================================================
// AEA Rollup
// =========================================================================

/// XSpec: "AEARollup testMultipleSigma+RDPass"
#[test]
fn aea_multiple_sigma_aggregated() {
    let mut p1 = portion(Classification::Secret);
    p1.aea_markings = vec![AeaMarking::Rd(RdBlock {
        cnwdi: false,
        sigma: vec![14, 15, 20].into(),
    })]
    .into();

    let mut p2 = portion(Classification::TopSecret);
    p2.aea_markings = vec![AeaMarking::Rd(RdBlock::default())].into();

    let mut p3 = portion(Classification::Secret);
    p3.aea_markings = vec![AeaMarking::Rd(RdBlock {
        cnwdi: false,
        sigma: vec![18].into(),
    })]
    .into();

    let portions = [p1, p2, p3];

    let class = ClassificationLattice::from_attrs_iter(&portions)
        .into_inner()
        .map(|c| c.effective_level());
    assert_eq!(class, Some(Classification::TopSecret));

    let aea_concat: Vec<AeaMarking> = portions
        .iter()
        .flat_map(|p| p.aea_markings.iter().cloned())
        .collect();
    let aea = AeaSet::from_markings(&aea_concat).to_markings();
    assert_eq!(aea.len(), 1);
    match &aea[0] {
        AeaMarking::Rd(rd) => assert_eq!(&*rd.sigma, &[14, 15, 18, 20]),
        other => panic!("expected Rd, got: {other:?}"),
    }
}

/// XSpec: "AEARollup Ensure AEA U marks drop in classified doc."
///
/// PR 4b-E (and PR 4b-C Commit 5 / PR 4b-D.2 history): the §H.6 p116 /
/// p118 UCNI strip lives in the declarative PageRewrite catalog on
/// `CapcoScheme` (`capco/dod-ucni-evicted-by-classified` +
/// `capco/doe-ucni-evicted-by-classified` + the two NOFORN-promotion
/// siblings). The per-axis `AeaSet::from_markings(...).to_markings()`
/// helper does NOT apply the strip (the strip is a cross-axis rewrite
/// dependent on the classification axis); only the full
/// `scheme.project(Scope::Page, ...)` path runs the rewrite catalog
/// and produces the strip-plus-NOFORN-promotion output.
///
/// verified 2026-05-18 against `crates/capco/docs/CAPCO-2016.md`
/// §H.6 DOD UCNI p116-117 + DOE UCNI p118-119.
#[test]
fn aea_ucni_strip_on_classified_via_scheme_project() {
    let mut p1 = portion(Classification::Unclassified);
    p1.aea_markings = vec![AeaMarking::DodUcni].into();

    let mut p2 = portion(Classification::Unclassified);
    p2.aea_markings = vec![AeaMarking::DoeUcni].into();

    let p3 = portion(Classification::Secret);

    let portions = [p1, p2, p3];

    // Per-axis helper keeps UCNI (no cross-axis classification gate).
    let aea_concat: Vec<AeaMarking> = portions
        .iter()
        .flat_map(|p| p.aea_markings.iter().cloned())
        .collect();
    let aea = AeaSet::from_markings(&aea_concat).to_markings();
    assert!(
        aea.iter().any(|m| matches!(m, AeaMarking::DodUcni)),
        "per-axis AeaSet helper keeps DOD UCNI; aea = {aea:?}"
    );

    // Scheme path applies the declarative strip rows on classified pages.
    let scheme = CapcoScheme::new();
    let markings: Vec<CapcoMarking> =
        portions.iter().cloned().map(CapcoMarking::new).collect();
    let projected = scheme.project(Scope::Page, &markings);
    assert!(
        !projected
            .0
            .aea_markings
            .iter()
            .any(|m| matches!(m, AeaMarking::DodUcni | AeaMarking::DoeUcni)),
        "scheme.project strips UCNI on classified pages; \
         projected.aea_markings = {:?}",
        projected.0.aea_markings,
    );
    // Cross-axis NOFORN promotion fires alongside the strip.
    assert!(
        projected.0.dissem_us.contains(&DissemControl::Nf),
        "scheme.project promotes NOFORN on UCNI strip; \
         projected.dissem_us = {:?}",
        projected.0.dissem_us,
    );
}

/// XSpec: "AEARollup Ensure AEA U marks don't drop in a U doc."
#[test]
fn aea_ucni_kept_in_unclassified() {
    let mut p1 = portion(Classification::Unclassified);
    p1.aea_markings = vec![AeaMarking::DodUcni].into();

    let mut p2 = portion(Classification::Unclassified);
    p2.aea_markings = vec![AeaMarking::DoeUcni].into();

    let portions = [p1, p2];
    let aea_concat: Vec<AeaMarking> = portions
        .iter()
        .flat_map(|p| p.aea_markings.iter().cloned())
        .collect();
    let aea = AeaSet::from_markings(&aea_concat).to_markings();
    assert_eq!(aea.len(), 2);
}

// =========================================================================
// Non-IC Rollup
// =========================================================================

/// XSpec: "NonICRollup Drop SBU-NF classified doc."
#[test]
fn non_ic_sbu_nf_splits_in_classified() {
    let mut p1 = portion(Classification::Unclassified);
    p1.non_ic_dissem = vec![NonIcDissem::SbuNf].into();
    let p2 = portion(Classification::Secret);

    let portions = [p1, p2];
    let non_ic_set = NonIcDissemSet::from_attrs_iter(&portions);
    let needs_nf = non_ic_set.needs_nf();
    let non_ic = non_ic_set.into_boxed_slice();
    assert!(non_ic.contains(&NonIcDissem::Sbu));
    assert!(!non_ic.contains(&NonIcDissem::SbuNf));
    assert!(needs_nf);

    // NF injection at the scheme layer.
    let scheme = CapcoScheme::new();
    let markings: Vec<CapcoMarking> =
        portions.iter().cloned().map(CapcoMarking::new).collect();
    let projected = scheme.project(Scope::Page, &markings);
    assert!(projected.0.dissem_us.contains(&DissemControl::Nf));
}

/// XSpec: "NonICRollup Keep SBU-NF Unclass doc."
#[test]
fn non_ic_sbu_nf_kept_in_unclassified() {
    let mut p1 = portion(Classification::Unclassified);
    p1.non_ic_dissem = vec![NonIcDissem::SbuNf].into();

    let portions = [p1];
    let non_ic_set = NonIcDissemSet::from_attrs_iter(&portions);
    let needs_nf = non_ic_set.needs_nf();
    let non_ic = non_ic_set.into_boxed_slice();
    assert!(non_ic.contains(&NonIcDissem::SbuNf));
    assert!(!needs_nf);
}

// =========================================================================
// Dissem Control Rollup
// =========================================================================

/// XSpec: OC-USGOV drops if not on all OC portions
#[test]
fn dissem_oc_usgov_drops_partial() {
    let mut p1 = portion(Classification::Secret);
    p1.dissem_us = vec![DissemControl::Oc, DissemControl::OcUsgov].into();
    let mut p2 = portion(Classification::Secret);
    p2.dissem_us = vec![DissemControl::Oc, DissemControl::OcUsgov].into();
    let mut p3 = portion(Classification::Secret);
    p3.dissem_us = vec![DissemControl::Oc].into();

    let portions = [p1, p2, p3];
    let dissem = DissemSet::from_attrs_iter(&portions).into_boxed_slice();
    assert!(dissem.contains(&DissemControl::Oc));
    assert!(!dissem.contains(&DissemControl::OcUsgov));
}

/// OC-USGOV drops under §H.8 p136 supersession when ORCON is present
/// anywhere — including when both ORCON and ORCON-USGOV are on every
/// portion. PR 4b-B (006 T112) Commit 2 retired the pre-fix unanimity
/// semantic (which would have kept OC-USGOV here) in favor of the
/// authoritative supersession rule.
#[test]
fn dissem_oc_usgov_drops_under_supersession_even_when_unanimous() {
    let mut p1 = portion(Classification::Secret);
    p1.dissem_us = vec![DissemControl::Oc, DissemControl::OcUsgov].into();
    let mut p2 = portion(Classification::Secret);
    p2.dissem_us = vec![DissemControl::Oc, DissemControl::OcUsgov].into();

    let portions = [p1, p2];
    let dissem = DissemSet::from_attrs_iter(&portions).into_boxed_slice();
    assert!(
        dissem.contains(&DissemControl::Oc),
        "ORCON should be in the banner (it dominates): {dissem:?}"
    );
    assert!(
        !dissem.contains(&DissemControl::OcUsgov),
        "ORCON-USGOV should drop under §H.8 p136 supersession when \
         ORCON is anywhere on the page (PR 4b-B Commit 2): {dissem:?}"
    );
}

/// OC-USGOV rolls up when no ORCON portion exists — supersession
/// triggers only when ORCON is present. §H.8 p140.
#[test]
fn dissem_oc_usgov_rolls_up_when_no_orcon() {
    let mut p1 = portion(Classification::Secret);
    p1.dissem_us = vec![DissemControl::OcUsgov].into();
    let mut p2 = portion(Classification::Secret);
    p2.dissem_us = vec![DissemControl::OcUsgov].into();

    let portions = [p1, p2];
    let dissem = DissemSet::from_attrs_iter(&portions).into_boxed_slice();
    assert!(
        dissem.contains(&DissemControl::OcUsgov),
        "ORCON-USGOV should roll up when no ORCON portion exists: {dissem:?}"
    );
    assert!(!dissem.contains(&DissemControl::Oc));
}

/// FOUO strips on classified pages via the §H.8 p134 PageRewrite catalog
/// (`capco/classification-evicts-fouo` + `capco/fouo-evicted-by-classified`).
/// The per-axis `DissemSet::from_attrs_iter` does NOT apply this
/// classification-gated strip; only `scheme.project(Scope::Page, ...)`
/// runs the rewrite catalog.
///
/// verified 2026-05-18 against `crates/capco/docs/CAPCO-2016.md`
/// §H.8 p134 FOUO Precedence Rules for Banner Line Guidance.
#[test]
fn dissem_fouo_strip_classified_via_scheme_project() {
    let mut p1 = portion(Classification::Unclassified);
    p1.dissem_us = vec![DissemControl::Fouo].into();
    let p2 = portion(Classification::Secret);

    let portions = [p1, p2];

    // Per-axis helper keeps FOUO.
    let dissem_axis = DissemSet::from_attrs_iter(&portions).into_boxed_slice();
    assert!(
        dissem_axis.contains(&DissemControl::Fouo),
        "per-axis DissemSet helper keeps FOUO: {dissem_axis:?}"
    );

    // Scheme path strips FOUO on classified pages.
    let scheme = CapcoScheme::new();
    let markings: Vec<CapcoMarking> =
        portions.iter().cloned().map(CapcoMarking::new).collect();
    let projected = scheme.project(Scope::Page, &markings);
    assert!(
        !projected.0.dissem_us.contains(&DissemControl::Fouo),
        "scheme.project strips FOUO on classified pages: {:?}",
        projected.0.dissem_us,
    );
}

/// FOUO kept in unclassified
#[test]
fn dissem_fouo_kept_unclassified() {
    let mut p1 = portion(Classification::Unclassified);
    p1.dissem_us = vec![DissemControl::Fouo].into();
    let portions = [p1];
    let dissem = DissemSet::from_attrs_iter(&portions).into_boxed_slice();
    assert!(dissem.contains(&DissemControl::Fouo));
}

// =========================================================================
// Country Rollup
// =========================================================================

/// REL TO intersection: USA AUS CAN ∩ USA AUS = USA AUS
#[test]
fn country_rel_intersection() {
    let mut p1 = portion(Classification::Secret);
    p1.dissem_us = vec![DissemControl::Rel].into();
    p1.rel_to = vec![
        CountryCode::USA,
        CountryCode::try_new(b"AUS").unwrap(),
        CountryCode::try_new(b"CAN").unwrap(),
    ]
    .into();

    let mut p2 = portion(Classification::Secret);
    p2.dissem_us = vec![DissemControl::Rel].into();
    p2.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"AUS").unwrap()].into();

    let portions = [p1, p2];
    let rel = RelToBlock::from_attrs_iter(&portions).into_boxed_slice();
    assert_eq!(rel.len(), 2);
    assert_eq!(rel[0], CountryCode::USA);
    assert_eq!(rel[1].as_str(), "AUS");
}

/// NOFORN supersedes REL TO
#[test]
fn country_noforn_supersedes_rel() {
    let mut p1 = portion(Classification::Secret);
    p1.dissem_us = vec![DissemControl::Rel].into();
    p1.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();

    let mut p2 = portion(Classification::Secret);
    p2.dissem_us = vec![DissemControl::Nf].into();

    let portions = [p1, p2];
    let rel = RelToBlock::from_attrs_iter(&portions).into_boxed_slice();
    assert!(rel.is_empty());
    let dissem = DissemSet::from_attrs_iter(&portions)
        .with_noforn_injected()
        .into_boxed_slice();
    assert!(dissem.contains(&DissemControl::Nf));
}

/// PR 3c.B-8F-engine-gap: NODIS in any portion injects NOFORN into the
/// rendered banner per CAPCO-2016 §H.9 p174 verbatim — "REL TO is not
/// authorized in the banner line if any portion contains NODIS information.
/// In this case, NOFORN would convey in the banner line."
///
/// Pins the end-to-end flow: `NonIcDissemSet`'s `needs_nf` propagates
/// through `scheme.project(Scope::Page, ...)` and `render_canonical`
/// produces a banner string that includes `NOFORN` and excludes the
/// REL TO block the portions would otherwise contribute.
#[test]
fn banner_renders_noforn_when_portion_has_nodis() {
    let mut p1 = portion(Classification::Secret);
    p1.dissem_us = vec![DissemControl::Rel].into();
    p1.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();

    let mut p2 = portion(Classification::Secret);
    p2.non_ic_dissem = vec![NonIcDissem::Nodis].into();

    let banner = render_banner_from_portions(&[p1, p2]);

    assert!(
        banner.contains("NOFORN"),
        "banner must inject NOFORN when any portion has NODIS \
         (§H.9 p174): {banner}"
    );
    assert!(
        !banner.contains("REL TO"),
        "banner must NOT carry REL TO when any portion has NODIS \
         (§H.9 p174): {banner}"
    );
    assert!(
        banner.contains("NODIS"),
        "NODIS itself must still roll up to the banner: {banner}"
    );
}

/// PR 3c.B-8F-engine-gap: EXDIS in any portion injects NOFORN into the
/// rendered banner per CAPCO-2016 §H.9 p172 verbatim — "REL TO is not
/// authorized in the banner line if any portion contains EXDIS information.
/// In this case, NOFORN would convey in the banner line."
///
/// Symmetric to the NODIS test above.
#[test]
fn banner_renders_noforn_when_portion_has_exdis() {
    let mut p1 = portion(Classification::Secret);
    p1.dissem_us = vec![DissemControl::Rel].into();
    p1.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();

    let mut p2 = portion(Classification::Secret);
    p2.non_ic_dissem = vec![NonIcDissem::Exdis].into();

    let banner = render_banner_from_portions(&[p1, p2]);

    assert!(
        banner.contains("NOFORN"),
        "banner must inject NOFORN when any portion has EXDIS \
         (§H.9 p172): {banner}"
    );
    assert!(
        !banner.contains("REL TO"),
        "banner must NOT carry REL TO when any portion has EXDIS \
         (§H.9 p172): {banner}"
    );
    assert!(
        banner.contains("EXDIS"),
        "EXDIS itself must still roll up to the banner: {banner}"
    );
}

// =========================================================================
// FGI Rollup
// =========================================================================

/// Source-concealed supersedes open
#[test]
fn fgi_concealed_supersedes_open() {
    let mut p1 = CanonicalAttrs::default();
    p1.fgi_marker = FgiMarker::acknowledged([CountryCode::try_new(b"DEU").unwrap()]);

    let mut p2 = CanonicalAttrs::default();
    p2.fgi_marker = Some(FgiMarker::SourceConcealed);

    let portions = [p1, p2];
    let fgi = FgiSet::from_attrs_iter(&portions).to_marker().unwrap();
    assert!(matches!(fgi, FgiMarker::SourceConcealed));
}

/// FGI open countries union
#[test]
fn fgi_open_union() {
    let mut p1 = CanonicalAttrs::default();
    p1.fgi_marker = FgiMarker::acknowledged([CountryCode::try_new(b"GBR").unwrap()]);

    let mut p2 = CanonicalAttrs::default();
    p2.fgi_marker = FgiMarker::acknowledged([CountryCode::try_new(b"DEU").unwrap()]);

    let portions = [p1, p2];
    let fgi = FgiSet::from_attrs_iter(&portions).to_marker().unwrap();
    match fgi {
        FgiMarker::Acknowledged { countries, .. } => assert_eq!(countries.len(), 2),
        FgiMarker::SourceConcealed => panic!("expected acknowledged variant"),
    }
}

// =========================================================================
// Classification + SCI
// =========================================================================

/// Max classification across portions
#[test]
fn classification_max() {
    let portions = [
        portion(Classification::Unclassified),
        portion(Classification::Secret),
        portion(Classification::Confidential),
    ];
    let class = ClassificationLattice::from_attrs_iter(&portions)
        .into_inner()
        .map(|c| c.effective_level());
    assert_eq!(class, Some(Classification::Secret));
}

/// SCI union
#[test]
fn sci_union() {
    let mut p1 = CanonicalAttrs::default();
    p1.sci_controls = vec![SciControl::Si].into();

    let mut p2 = CanonicalAttrs::default();
    p2.sci_controls = vec![SciControl::Tk, SciControl::HcsP].into();

    let portions = [p1, p2];
    let sci = sci_controls_from_markings(&portions);
    assert_eq!(sci.len(), 3);
    assert!(sci.contains(&SciControl::Si));
    assert!(sci.contains(&SciControl::Tk));
    assert!(sci.contains(&SciControl::HcsP));
}
