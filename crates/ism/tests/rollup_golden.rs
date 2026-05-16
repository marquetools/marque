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
///
/// PR 4b-C Commit 5 (006 T112): the §H.6 p116 / p118 UCNI strip moved
/// out of `expected_aea_markings` into the declarative PageRewrite
/// catalog on `CapcoScheme` (`capco/dod-ucni-evicted-by-classified` +
/// `capco/doe-ucni-evicted-by-classified` + the two NOFORN-promotion
/// siblings). PageContext is the transitional driver until PR 4b-D
/// wires the lattice path. Until then, UCNI keeps on the PageContext
/// AEA axis on classified pages; `scheme.project(Scope::Page, ...)`
/// produces the correct strip-plus-NOFORN-promotion output.
///
/// verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`
/// §H.6 DOD UCNI p116-117 + DOE UCNI p118-119.
#[test]
fn aea_ucni_kept_in_classified_via_pagecontext_transitional_pending_pr_4b_d() {
    let mut ctx = PageContext::new();

    let mut p1 = portion(Classification::Unclassified);
    p1.aea_markings = vec![AeaMarking::DodUcni].into();
    ctx.add_portion(p1);

    let mut p2 = portion(Classification::Unclassified);
    p2.aea_markings = vec![AeaMarking::DoeUcni].into();
    ctx.add_portion(p2);

    ctx.add_portion(portion(Classification::Secret));

    assert_eq!(ctx.expected_classification(), Some(Classification::Secret));
    let aea = ctx.expected_aea_markings();
    assert!(
        aea.iter().any(|m| matches!(m, AeaMarking::DodUcni)),
        "PR 4b-C post-deletion: DOD UCNI keeps via PageContext on \
         classified pages (declarative strip in `scheme.project`). \
         aea = {aea:?}"
    );
    assert!(
        aea.iter().any(|m| matches!(m, AeaMarking::DoeUcni)),
        "PR 4b-C post-deletion: DOE UCNI keeps via PageContext on \
         classified pages. aea = {aea:?}"
    );
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

    let dissem = ctx.expected_dissem_us();
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
    p1.dissem_us = vec![DissemControl::Oc, DissemControl::OcUsgov].into();
    ctx.add_portion(p1);

    let mut p2 = portion(Classification::Secret);
    p2.dissem_us = vec![DissemControl::Oc, DissemControl::OcUsgov].into();
    ctx.add_portion(p2);

    let mut p3 = portion(Classification::Secret);
    p3.dissem_us = vec![DissemControl::Oc].into();
    ctx.add_portion(p3);

    let dissem = ctx.expected_dissem_us();
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
    let mut ctx = PageContext::new();

    let mut p1 = portion(Classification::Secret);
    p1.dissem_us = vec![DissemControl::Oc, DissemControl::OcUsgov].into();
    ctx.add_portion(p1);

    let mut p2 = portion(Classification::Secret);
    p2.dissem_us = vec![DissemControl::Oc, DissemControl::OcUsgov].into();
    ctx.add_portion(p2);

    let dissem = ctx.expected_dissem_us();
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
    let mut ctx = PageContext::new();

    let mut p1 = portion(Classification::Secret);
    p1.dissem_us = vec![DissemControl::OcUsgov].into();
    ctx.add_portion(p1);

    let mut p2 = portion(Classification::Secret);
    p2.dissem_us = vec![DissemControl::OcUsgov].into();
    ctx.add_portion(p2);

    let dissem = ctx.expected_dissem_us();
    assert!(
        dissem.contains(&DissemControl::OcUsgov),
        "ORCON-USGOV should roll up when no ORCON portion exists: {dissem:?}"
    );
    assert!(!dissem.contains(&DissemControl::Oc));
}

/// FOUO drops in classified (transitional post-PR-4b-C).
///
/// PR 4b-C Commit 5 (006 T112): the §H.8 p134 FOUO classified-strip
/// moved out of `expected_dissem_us` Step 3 into the declarative
/// PageRewrite catalog on `CapcoScheme`
/// (`capco/fouo-evicted-by-classified` +
/// `capco/classification-evicts-fouo`). PageContext is the
/// transitional driver until PR 4b-D wires the lattice path.
/// Until then, FOUO keeps on the PageContext dissem axis on
/// classified pages; `scheme.project(Scope::Page, ...)` produces
/// the correct strip output.
///
/// verified 2026-05-16 against `crates/capco/docs/CAPCO-2016.md`
/// §H.8 p134 FOUO Precedence Rules for Banner Line Guidance.
#[test]
fn dissem_fouo_kept_classified_via_pagecontext_transitional_pending_pr_4b_d() {
    let mut ctx = PageContext::new();

    let mut p1 = portion(Classification::Unclassified);
    p1.dissem_us = vec![DissemControl::Fouo].into();
    ctx.add_portion(p1);

    ctx.add_portion(portion(Classification::Secret));

    let dissem = ctx.expected_dissem_us();
    assert!(
        dissem.contains(&DissemControl::Fouo),
        "PR 4b-C post-deletion: FOUO keeps via PageContext on classified \
         pages (declarative strip in `scheme.project`). dissem = {dissem:?}"
    );
}

/// FOUO kept in unclassified
#[test]
fn dissem_fouo_kept_unclassified() {
    let mut ctx = PageContext::new();

    let mut p1 = portion(Classification::Unclassified);
    p1.dissem_us = vec![DissemControl::Fouo].into();
    ctx.add_portion(p1);

    let dissem = ctx.expected_dissem_us();
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
    p1.dissem_us = vec![DissemControl::Rel].into();
    p1.rel_to = vec![
        CountryCode::USA,
        CountryCode::try_new(b"AUS").unwrap(),
        CountryCode::try_new(b"CAN").unwrap(),
    ]
    .into();
    ctx.add_portion(p1);

    let mut p2 = portion(Classification::Secret);
    p2.dissem_us = vec![DissemControl::Rel].into();
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
    p1.dissem_us = vec![DissemControl::Rel].into();
    p1.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();
    ctx.add_portion(p1);

    let mut p2 = portion(Classification::Secret);
    p2.dissem_us = vec![DissemControl::Nf].into();
    ctx.add_portion(p2);

    assert!(ctx.expected_rel_to().is_empty());
    assert!(ctx.expected_dissem_us().contains(&DissemControl::Nf));
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
    p1.dissem_us = vec![DissemControl::Rel].into();
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
    p1.dissem_us = vec![DissemControl::Rel].into();
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
