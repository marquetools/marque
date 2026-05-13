// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Golden tests derived from the ODNI ISM-Rollup XSpec test suite.
//!
//! Each test translates an authoritative XSpec scenario into a Rust test.
//! Uses Default::default() + field mutation since CanonicalAttrs is #[non_exhaustive].

use marque_ism::CanonicalAttrs;
use marque_ism::attrs::*;
use marque_ism::page_context::PageContext;

fn portion(c: Classification) -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(c));
    a
}

// =========================================================================
// AEA Rollup
// =========================================================================

/// XSpec: "AEARollup testMultipleSigma+RDPass"
#[test]
fn aea_multiple_sigma_aggregated() {
    let mut ctx = PageContext::new();

    let mut p1 = portion(Classification::Secret);
    p1.aea_markings = vec![AeaMarking::Rd(RdBlock {
        cnwdi: false,
        sigma: vec![14, 15, 20].into(),
    })]
    .into();
    ctx.add_portion(p1);

    let mut p2 = portion(Classification::TopSecret);
    p2.aea_markings = vec![AeaMarking::Rd(RdBlock::default())].into();
    ctx.add_portion(p2);

    let mut p3 = portion(Classification::Secret);
    p3.aea_markings = vec![AeaMarking::Rd(RdBlock {
        cnwdi: false,
        sigma: vec![18].into(),
    })]
    .into();
    ctx.add_portion(p3);

    assert_eq!(
        ctx.expected_classification(),
        Some(Classification::TopSecret)
    );
    let aea = ctx.expected_aea_markings();
    assert_eq!(aea.len(), 1);
    match &aea[0] {
        AeaMarking::Rd(rd) => assert_eq!(&*rd.sigma, &[14, 15, 18, 20]),
        other => panic!("expected Rd, got: {other:?}"),
    }
}

/// XSpec: "AEARollup Ensure AEA U marks drop in classified doc."
#[test]
fn aea_ucni_drops_in_classified() {
    let mut ctx = PageContext::new();

    let mut p1 = portion(Classification::Unclassified);
    p1.aea_markings = vec![AeaMarking::DodUcni].into();
    ctx.add_portion(p1);

    let mut p2 = portion(Classification::Unclassified);
    p2.aea_markings = vec![AeaMarking::DoeUcni].into();
    ctx.add_portion(p2);

    ctx.add_portion(portion(Classification::Secret));

    assert_eq!(ctx.expected_classification(), Some(Classification::Secret));
    assert!(ctx.expected_aea_markings().is_empty());
}

/// XSpec: "AEARollup Ensure AEA U marks don't drop in a U doc."
#[test]
fn aea_ucni_kept_in_unclassified() {
    let mut ctx = PageContext::new();

    let mut p1 = portion(Classification::Unclassified);
    p1.aea_markings = vec![AeaMarking::DodUcni].into();
    ctx.add_portion(p1);

    let mut p2 = portion(Classification::Unclassified);
    p2.aea_markings = vec![AeaMarking::DoeUcni].into();
    ctx.add_portion(p2);

    assert_eq!(ctx.expected_aea_markings().len(), 2);
}

// =========================================================================
// Non-IC Rollup
// =========================================================================

/// XSpec: "NonICRollup Drop SBU-NF classified doc."
#[test]
fn non_ic_sbu_nf_splits_in_classified() {
    let mut ctx = PageContext::new();

    let mut p1 = portion(Classification::Unclassified);
    p1.non_ic_dissem = vec![NonIcDissem::SbuNf].into();
    ctx.add_portion(p1);

    ctx.add_portion(portion(Classification::Secret));

    let (non_ic, needs_nf) = ctx.expected_non_ic_dissem();
    assert!(non_ic.contains(&NonIcDissem::Sbu));
    assert!(!non_ic.contains(&NonIcDissem::SbuNf));
    assert!(needs_nf);

    let dissem = ctx.expected_dissem_controls();
    assert!(dissem.contains(&DissemControl::Nf));
}

/// XSpec: "NonICRollup Keep SBU-NF Unclass doc."
#[test]
fn non_ic_sbu_nf_kept_in_unclassified() {
    let mut ctx = PageContext::new();

    let mut p1 = portion(Classification::Unclassified);
    p1.non_ic_dissem = vec![NonIcDissem::SbuNf].into();
    ctx.add_portion(p1);

    let (non_ic, needs_nf) = ctx.expected_non_ic_dissem();
    assert!(non_ic.contains(&NonIcDissem::SbuNf));
    assert!(!needs_nf);
}

// =========================================================================
// Dissem Control Rollup
// =========================================================================

/// XSpec: OC-USGOV drops if not on all OC portions
#[test]
fn dissem_oc_usgov_drops_partial() {
    let mut ctx = PageContext::new();

    let mut p1 = portion(Classification::Secret);
    p1.dissem_controls = vec![DissemControl::Oc, DissemControl::OcUsgov].into();
    ctx.add_portion(p1);

    let mut p2 = portion(Classification::Secret);
    p2.dissem_controls = vec![DissemControl::Oc, DissemControl::OcUsgov].into();
    ctx.add_portion(p2);

    let mut p3 = portion(Classification::Secret);
    p3.dissem_controls = vec![DissemControl::Oc].into();
    ctx.add_portion(p3);

    let dissem = ctx.expected_dissem_controls();
    assert!(dissem.contains(&DissemControl::Oc));
    assert!(!dissem.contains(&DissemControl::OcUsgov));
}

/// OC-USGOV kept when on all OC portions
#[test]
fn dissem_oc_usgov_kept_when_all() {
    let mut ctx = PageContext::new();

    let mut p1 = portion(Classification::Secret);
    p1.dissem_controls = vec![DissemControl::Oc, DissemControl::OcUsgov].into();
    ctx.add_portion(p1);

    let mut p2 = portion(Classification::Secret);
    p2.dissem_controls = vec![DissemControl::Oc, DissemControl::OcUsgov].into();
    ctx.add_portion(p2);

    let dissem = ctx.expected_dissem_controls();
    assert!(dissem.contains(&DissemControl::OcUsgov));
}

/// FOUO drops in classified
#[test]
fn dissem_fouo_drops_classified() {
    let mut ctx = PageContext::new();

    let mut p1 = portion(Classification::Unclassified);
    p1.dissem_controls = vec![DissemControl::Fouo].into();
    ctx.add_portion(p1);

    ctx.add_portion(portion(Classification::Secret));

    let dissem = ctx.expected_dissem_controls();
    assert!(!dissem.contains(&DissemControl::Fouo));
}

/// FOUO kept in unclassified
#[test]
fn dissem_fouo_kept_unclassified() {
    let mut ctx = PageContext::new();

    let mut p1 = portion(Classification::Unclassified);
    p1.dissem_controls = vec![DissemControl::Fouo].into();
    ctx.add_portion(p1);

    let dissem = ctx.expected_dissem_controls();
    assert!(dissem.contains(&DissemControl::Fouo));
}

// =========================================================================
// Country Rollup
// =========================================================================

/// REL TO intersection: USA AUS CAN ∩ USA AUS = USA AUS
#[test]
fn country_rel_intersection() {
    let mut ctx = PageContext::new();

    let mut p1 = portion(Classification::Secret);
    p1.dissem_controls = vec![DissemControl::Rel].into();
    p1.rel_to = vec![
        CountryCode::USA,
        CountryCode::try_new(b"AUS").unwrap(),
        CountryCode::try_new(b"CAN").unwrap(),
    ]
    .into();
    ctx.add_portion(p1);

    let mut p2 = portion(Classification::Secret);
    p2.dissem_controls = vec![DissemControl::Rel].into();
    p2.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"AUS").unwrap()].into();
    ctx.add_portion(p2);

    let rel = ctx.expected_rel_to();
    assert_eq!(rel.len(), 2);
    assert_eq!(rel[0], CountryCode::USA);
    assert_eq!(rel[1].as_str(), "AUS");
}

/// NOFORN supersedes REL TO
#[test]
fn country_noforn_supersedes_rel() {
    let mut ctx = PageContext::new();

    let mut p1 = portion(Classification::Secret);
    p1.dissem_controls = vec![DissemControl::Rel].into();
    p1.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();
    ctx.add_portion(p1);

    let mut p2 = portion(Classification::Secret);
    p2.dissem_controls = vec![DissemControl::Nf].into();
    ctx.add_portion(p2);

    assert!(ctx.expected_rel_to().is_empty());
    assert!(ctx.expected_dissem_controls().contains(&DissemControl::Nf));
}

/// PR 3c.B-8F-engine-gap: NODIS in any portion injects NOFORN into the
/// rendered banner per CAPCO-2016 §H.9 p174 verbatim — "REL TO is not
/// authorized in the banner line if any portion contains NODIS information.
/// In this case, NOFORN would convey in the banner line."
///
/// Pins the end-to-end flow: `needs_nf` propagates from
/// `expected_non_ic_dissem` into `render_expected_banner` and produces a
/// banner string that includes `//NOFORN` and excludes the REL TO block
/// the portions would otherwise contribute.
#[test]
fn banner_renders_noforn_when_portion_has_nodis() {
    let mut ctx = PageContext::new();

    let mut p1 = portion(Classification::Secret);
    p1.dissem_controls = vec![DissemControl::Rel].into();
    p1.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();
    ctx.add_portion(p1);

    let mut p2 = portion(Classification::Secret);
    p2.non_ic_dissem = vec![NonIcDissem::Nodis].into();
    ctx.add_portion(p2);

    let banner = ctx
        .render_expected_banner()
        .expect("portions present, banner must render");

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
    let mut ctx = PageContext::new();

    let mut p1 = portion(Classification::Secret);
    p1.dissem_controls = vec![DissemControl::Rel].into();
    p1.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();
    ctx.add_portion(p1);

    let mut p2 = portion(Classification::Secret);
    p2.non_ic_dissem = vec![NonIcDissem::Exdis].into();
    ctx.add_portion(p2);

    let banner = ctx
        .render_expected_banner()
        .expect("portions present, banner must render");

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
    let mut ctx = PageContext::new();

    let mut p1 = CanonicalAttrs::default();
    p1.fgi_marker = FgiMarker::acknowledged([CountryCode::try_new(b"DEU").unwrap()]);
    ctx.add_portion(p1);

    let mut p2 = CanonicalAttrs::default();
    p2.fgi_marker = Some(FgiMarker::SourceConcealed);
    ctx.add_portion(p2);

    let fgi = ctx.expected_fgi_marker().unwrap();
    assert!(matches!(fgi, FgiMarker::SourceConcealed));
}

/// FGI open countries union
#[test]
fn fgi_open_union() {
    let mut ctx = PageContext::new();

    let mut p1 = CanonicalAttrs::default();
    p1.fgi_marker = FgiMarker::acknowledged([CountryCode::try_new(b"GBR").unwrap()]);
    ctx.add_portion(p1);

    let mut p2 = CanonicalAttrs::default();
    p2.fgi_marker = FgiMarker::acknowledged([CountryCode::try_new(b"DEU").unwrap()]);
    ctx.add_portion(p2);

    let fgi = ctx.expected_fgi_marker().unwrap();
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
    let mut ctx = PageContext::new();
    ctx.add_portion(portion(Classification::Unclassified));
    ctx.add_portion(portion(Classification::Secret));
    ctx.add_portion(portion(Classification::Confidential));
    assert_eq!(ctx.expected_classification(), Some(Classification::Secret));
}

/// SCI union
#[test]
fn sci_union() {
    let mut ctx = PageContext::new();

    let mut p1 = CanonicalAttrs::default();
    p1.sci_controls = vec![SciControl::Si].into();
    ctx.add_portion(p1);

    let mut p2 = CanonicalAttrs::default();
    p2.sci_controls = vec![SciControl::Tk, SciControl::HcsP].into();
    ctx.add_portion(p2);

    let sci = ctx.expected_sci_controls();
    assert_eq!(sci.len(), 3);
    assert!(sci.contains(&SciControl::Si));
    assert!(sci.contains(&SciControl::Tk));
    assert!(sci.contains(&SciControl::HcsP));
}
