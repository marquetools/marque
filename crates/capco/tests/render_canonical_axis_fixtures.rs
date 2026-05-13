// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Commit 5 — per-axis golden-output fixtures.
//!
//! Each axis renderer in `crates/capco/src/render/` ships ~5 golden
//! fixtures: an in-code `CapcoMarking` constructed by direct lattice
//! literal (NOT via the parser, so the test exercises the renderer
//! isolated from parser behavior) plus the expected canonical bytes
//! the renderer must produce. Each row carries an inline §-citation
//! pointing at the CAPCO-2016 passage that defines the canonical
//! form.
//!
//! # Authority
//!
//! Per Constitution VIII (Authoritative Source Fidelity): every
//! per-axis golden output cites the §H passage that defines the
//! canonical form. The verification oracle is
//! `crates/capco/docs/CAPCO-2016.md`, never retiring rule code.
//!
//! # Coverage
//!
//! 5+ fixtures per axis × 9 axes (classification, SCI, SAR, AEA, FGI,
//! dissem, REL TO, non-IC dissem, declassify) ≈ 50+ fixtures total.
//! The 100-fixture target the plan calls out is a stretch; the 50-row
//! floor is the load-bearing per-axis canonicalization-invariant
//! coverage.

use marque_capco::scheme::{CapcoMarking, CapcoScheme};
use smol_str::SmolStr;
use marque_ism::{
    AeaMarking, CanonicalAttrs, Classification, CountryCode, DissemControl, FgiClassification,
    FgiMarker, FrdBlock, IsmDate, JointClassification, MarkingClassification, NatoClassification,
    NonIcDissem, RdBlock, SarCompartment, SarIndicator, SarMarking, SarProgram, SciCompartment,
    SciControl, SciControlSystem, SciMarking,
};
use marque_scheme::{MarkingScheme, Scope};

// ---------------------------------------------------------------------------
// Helpers — render via render_canonical at the requested scope and
// assert byte-identity against the expected canonical form.
// ---------------------------------------------------------------------------

fn render(attrs: CanonicalAttrs, scope: Scope) -> String {
    let scheme = CapcoScheme::new();
    let marking = CapcoMarking::from(attrs);
    let mut out = String::new();
    scheme
        .render_canonical(&marking, scope, &mut out)
        .expect("render_canonical must succeed for Portion / Page / Document");
    out
}

fn render_banner(attrs: CanonicalAttrs) -> String {
    render(attrs, Scope::Page)
}

fn render_portion(attrs: CanonicalAttrs) -> String {
    render(attrs, Scope::Portion)
}

fn cc(s: &str) -> CountryCode {
    CountryCode::try_new(s.as_bytes()).expect("valid country code in test fixture")
}

// ===========================================================================
// CLASSIFICATION axis (CAPCO-2016 §A.6 p15-16, §H.1 p49, §H.3 p55-58, §H.7 p123)
// ===========================================================================

#[test]
fn classification_us_secret_banner() {
    // Authority: CAPCO-2016 §H.1 p49 — US Secret banner = `SECRET`.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    assert_eq!(render_banner(a), "SECRET");
}

#[test]
fn classification_us_topsecret_portion() {
    // Authority: CAPCO-2016 §H.1 p49 — US TS portion = `TS`.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
    assert_eq!(render_portion(a), "TS");
}

#[test]
fn classification_fgi_acknowledged_single_country_banner() {
    // Authority: CAPCO-2016 §H.7 p123 — FGI as classification system,
    // source-acknowledged single country: `//GBR S`.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Fgi(FgiClassification {
        countries: vec![cc("GBR")].into(),
        level: Classification::Secret,
    }));
    assert_eq!(render_banner(a), "//GBR SECRET");
}

#[test]
fn classification_fgi_concealed_banner() {
    // Authority: CAPCO-2016 §H.7 p123 — source-concealed FGI:
    // `//FGI S` (FGI prefix replaces country list).
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Fgi(FgiClassification {
        countries: vec![].into(),
        level: Classification::Secret,
    }));
    assert_eq!(render_banner(a), "//FGI SECRET");
}

#[test]
fn classification_nato_banner() {
    // Authority: CAPCO-2016 §H.3 + Table 4 §3 p36 — NATO Secret
    // banner = `NATO SECRET`.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
    assert_eq!(render_banner(a), "NATO SECRET");
}

#[test]
fn classification_joint_alphabetical_banner() {
    // Authority: CAPCO-2016 §A.6 p15-16 + §H.3 p56 — JOINT [LIST] is
    // alphabetical (trigraphs first, then tetragraphs, each alpha-
    // sorted). USA appears in alphabetical position, NOT pulled to the
    // front. Canonical examples on §H.3 p56 line "//JOINT TOP SECRET
    // CAN ISR USA" and §H.3 p58 line "//JOINT SECRET CAN GBR USA"
    // both place USA in its alphabetical slot (after C, G, etc.).
    //
    // The USA-first rule is REL TO-axis only (§H.8 p150-151), not
    // JOINT-axis. Conflating the two was a Constitution VIII defect
    // caught in pre-flight review.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: vec![cc("GBR"), cc("USA"), cc("AUS")].into(),
    }));
    assert_eq!(render_banner(a), "//JOINT SECRET AUS GBR USA");
}

// ===========================================================================
// SCI axis (CAPCO-2016 §A.6 p15-16, §H.4 p61)
// ===========================================================================

#[test]
fn sci_single_system_bare() {
    // Authority: CAPCO-2016 §A.6 p15-16 — bare SCI control system.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
    a.sci_markings = vec![SciMarking::new(
        SciControlSystem::Published(marque_ism::SciControlBare::Si),
        Box::new([]),
        None,
    )]
    .into();
    assert_eq!(render_banner(a), "TOP SECRET//SI");
}

#[test]
fn sci_compartment_numeric_then_alpha_sort() {
    // Authority: CAPCO-2016 §A.6 p15-16 + §H.4 p61 — compartments
    // numeric-then-alpha. Input out-of-order; expect canonical sort.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
    a.sci_markings = vec![SciMarking::new(
        SciControlSystem::Published(marque_ism::SciControlBare::Si),
        vec![
            SciCompartment::new("DEFG", Box::new([])),
            SciCompartment::new("ABCD", Box::new([])),
        ]
        .into(),
        None,
    )]
    .into();
    assert_eq!(render_banner(a), "TOP SECRET//SI-ABCD-DEFG");
}

#[test]
fn sci_sub_compartments_space_separated() {
    // Authority: CAPCO-2016 §A.6 p15-16 + §A.6 p16 example
    // `SI-G ABCD DEFG` — sub-compartments space-separated within
    // a compartment.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
    a.sci_markings = vec![SciMarking::new(
        SciControlSystem::Published(marque_ism::SciControlBare::Si),
        vec![SciCompartment::new(
            "G",
            Box::new([SmolStr::from("ABCD"), SmolStr::from("DEFG")]),
        )]
        .into(),
        None,
    )]
    .into();
    assert_eq!(render_banner(a), "TOP SECRET//SI-G ABCD DEFG");
}

#[test]
fn sci_multiple_systems_slash_separated() {
    // Authority: CAPCO-2016 §A.6 p15-16 — multiple SCI control
    // systems `/`-separated, alpha-sorted.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
    a.sci_markings = vec![
        SciMarking::new(
            SciControlSystem::Published(marque_ism::SciControlBare::Tk),
            Box::new([]),
            None,
        ),
        SciMarking::new(
            SciControlSystem::Published(marque_ism::SciControlBare::Si),
            Box::new([]),
            None,
        ),
    ]
    .into();
    assert_eq!(render_banner(a), "TOP SECRET//SI/TK");
}

#[test]
fn sci_numeric_system_sorts_before_alpha() {
    // Authority: CAPCO-2016 §A.6 p15-16 example p16: `123` (numeric)
    // sorts before `SI-G` (alpha). Custom system named `123`.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
    a.sci_markings = vec![
        SciMarking::new(
            SciControlSystem::Published(marque_ism::SciControlBare::Si),
            vec![SciCompartment::new("G", Box::new([]))].into(),
            None,
        ),
        SciMarking::new(SciControlSystem::Custom(SmolStr::from("123")), Box::new([]), None),
    ]
    .into();
    assert_eq!(render_banner(a), "TOP SECRET//123/SI-G");
}

// ===========================================================================
// SAR axis (CAPCO-2016 §A.6 p16, §H.5 p99-100)
// ===========================================================================

#[test]
fn sar_single_program_short_indicator() {
    // Authority: CAPCO-2016 §A.6 p16 + §H.5 p100 — `SAR-` indicator
    // canonical for short program identifiers.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.sar_markings = Some(SarMarking::new(
        SarIndicator::Abbrev,
        vec![SarProgram::new("ABC", Box::new([]))].into(),
    ));
    assert_eq!(render_banner(a), "SECRET//SAR-ABC");
}

#[test]
fn sar_multi_program_alpha_sort() {
    // Authority: CAPCO-2016 §A.6 p16 — multiple SAP program IDs in
    // ascending alpha order, `/`-separated.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.sar_markings = Some(SarMarking::new(
        SarIndicator::Abbrev,
        vec![
            SarProgram::new("XYZ", Box::new([])),
            SarProgram::new("ABC", Box::new([])),
        ]
        .into(),
    ));
    assert_eq!(render_banner(a), "SECRET//SAR-ABC/XYZ");
}

#[test]
fn sar_program_with_compartment() {
    // Authority: CAPCO-2016 §A.6 p16 example
    // `SECRET//SAR-ABC-DEF 123/SDA-121//NOFORN` — compartment
    // hyphen-attached to program.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.sar_markings = Some(SarMarking::new(
        SarIndicator::Abbrev,
        vec![SarProgram::new(
            "ABC",
            vec![SarCompartment::new("DEF", Box::new([]))].into(),
        )]
        .into(),
    ));
    assert_eq!(render_banner(a), "SECRET//SAR-ABC-DEF");
}

#[test]
fn sar_full_indicator_for_multiword_program() {
    // Authority: CAPCO-2016 §A.6 p16 + §H.5 p100 — multi-word
    // program names require the long `SPECIAL ACCESS REQUIRED-`
    // indicator (the abbreviated `SAR-` grammar admits only
    // `[A-Z0-9]{2,3}`).
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
    a.sar_markings = Some(SarMarking::new(
        SarIndicator::Full,
        vec![SarProgram::new("BUTTER POPCORN", Box::new([]))].into(),
    ));
    assert_eq!(
        render_banner(a),
        "TOP SECRET//SPECIAL ACCESS REQUIRED-BUTTER POPCORN"
    );
}

#[test]
fn sar_compartment_with_sub_compartments() {
    // Authority: CAPCO-2016 §A.6 p16 example
    // `SECRET//SAR-ABC-DEF 123/SDA-121` — sub-compartments
    // space-separated under their compartment.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.sar_markings = Some(SarMarking::new(
        SarIndicator::Abbrev,
        vec![SarProgram::new(
            "ABC",
            vec![SarCompartment::new(
                "DEF",
                Box::new([SmolStr::from("123")]),
            )]
            .into(),
        )]
        .into(),
    ));
    assert_eq!(render_banner(a), "SECRET//SAR-ABC-DEF 123");
}

// ===========================================================================
// AEA axis (CAPCO-2016 §A.6 p16, §H.6, Table 4 §6 p36)
// ===========================================================================

#[test]
fn aea_rd_alone_banner() {
    // Authority: CAPCO-2016 §H.6 + Table 4 §6 p36 — bare RD = `RD`.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.aea_markings = vec![AeaMarking::Rd(RdBlock::default())].into();
    assert_eq!(render_banner(a), "SECRET//RD");
}

#[test]
fn aea_rd_with_cnwdi_banner() {
    // Authority: CAPCO-2016 §H.6 — `RD-CNWDI` (CNWDI hyphen-attached
    // after RD).
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.aea_markings = vec![AeaMarking::Rd(RdBlock {
        cnwdi: true,
        sigma: Box::new([]),
    })]
    .into();
    assert_eq!(render_banner(a), "SECRET//RD-CNWDI");
}

#[test]
fn aea_rd_sigma_numeric_ascending() {
    // Authority: CAPCO-2016 §H.6 — SIGMA numbers ascending numeric
    // sort, space-separated.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
    a.aea_markings = vec![AeaMarking::Rd(RdBlock {
        cnwdi: false,
        sigma: vec![18, 14, 20].into(),
    })]
    .into();
    assert_eq!(render_banner(a), "TOP SECRET//RD-SIGMA 14 18 20");
}

#[test]
fn aea_register_order_rd_before_frd() {
    // Authority: CAPCO-2016 §A.6 p16 + §H.6 Table 4 §6 p36 — RD
    // before FRD (Register order).
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.aea_markings = vec![
        AeaMarking::Frd(FrdBlock::default()),
        AeaMarking::Rd(RdBlock::default()),
    ]
    .into();
    assert_eq!(render_banner(a), "SECRET//RD/FRD");
}

#[test]
fn aea_dod_ucni_portion_form() {
    // Authority: CAPCO-2016 §H.6 Table 4 §6 p36 — DOD UCNI banner
    // = `DOD UCNI`, portion = `DCNI`.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Unclassified));
    a.aea_markings = vec![AeaMarking::DodUcni].into();
    assert_eq!(render_banner(a.clone()), "UNCLASSIFIED//DOD UCNI");
    assert_eq!(render_portion(a), "U//DCNI");
}

#[test]
fn aea_tfni_banner() {
    // Authority: CAPCO-2016 §H.6 Table 4 §6 p36 — TFNI standalone.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.aea_markings = vec![AeaMarking::Tfni].into();
    assert_eq!(render_banner(a), "SECRET//TFNI");
}

// ===========================================================================
// FGI marker axis (CAPCO-2016 §A.6 p16, §H.7 p123)
// ===========================================================================

#[test]
fn fgi_marker_concealed() {
    // Authority: CAPCO-2016 §H.7 p123 — source-concealed FGI marker
    // = bare `FGI`.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.fgi_marker = Some(FgiMarker::SourceConcealed);
    assert_eq!(render_banner(a), "SECRET//FGI");
}

#[test]
fn fgi_marker_acknowledged_single_trigraph() {
    // Authority: CAPCO-2016 §H.7 p123 — source-acknowledged FGI
    // marker with one country trigraph.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.fgi_marker = FgiMarker::acknowledged([cc("GBR")]);
    assert_eq!(render_banner(a), "SECRET//FGI GBR");
}

#[test]
fn fgi_marker_acknowledged_trigraphs_alpha_sort() {
    // Authority: CAPCO-2016 §A.6 p16 — trigraphs alpha first.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.fgi_marker = FgiMarker::acknowledged([cc("JPN"), cc("GBR")]);
    assert_eq!(render_banner(a), "SECRET//FGI GBR JPN");
}

#[test]
fn fgi_marker_acknowledged_trigraphs_then_tetragraphs() {
    // Authority: CAPCO-2016 §A.6 p16 — trigraphs alpha first, then
    // tetragraphs alpha.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.fgi_marker = FgiMarker::acknowledged([cc("NATO"), cc("GBR"), cc("JPN")]);
    assert_eq!(render_banner(a), "SECRET//FGI GBR JPN NATO");
}

#[test]
fn fgi_marker_with_rel_to_inline() {
    // Authority: CAPCO-2016 §A.6 p16 example
    // `SECRET//FGI GBR JPN NATO//REL TO USA, GBR, JPN, NATO`.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.fgi_marker = FgiMarker::acknowledged([cc("GBR"), cc("JPN"), cc("NATO")]);
    a.rel_to = vec![cc("USA"), cc("GBR"), cc("JPN"), cc("NATO")].into();
    assert_eq!(
        render_banner(a),
        "SECRET//FGI GBR JPN NATO//REL TO USA, GBR, JPN, NATO"
    );
}

// ===========================================================================
// IC dissem axis (CAPCO-2016 §A.6 p16, §H.8, Table 4 §8 p36)
// ===========================================================================

#[test]
fn dissem_noforn_banner() {
    // Authority: CAPCO-2016 §H.8 Table 4 §8 p36 — `NF` portion
    // form maps to `NOFORN` banner form.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_controls = vec![DissemControl::Nf].into();
    assert_eq!(render_banner(a), "SECRET//NOFORN");
}

#[test]
fn dissem_noforn_portion() {
    // Authority: CAPCO-2016 §H.8 Table 4 §8 p36 — `NF` portion form.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_controls = vec![DissemControl::Nf].into();
    assert_eq!(render_portion(a), "S//NF");
}

#[test]
fn dissem_orcon_banner_form() {
    // Authority: CAPCO-2016 §H.8 Table 4 §8 p36 — `OC` portion ↔
    // `ORCON` banner.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
    a.dissem_controls = vec![DissemControl::Oc].into();
    assert_eq!(render_banner(a), "TOP SECRET//ORCON");
}

#[test]
fn dissem_register_order_orcon_before_noforn() {
    // Authority: CAPCO-2016 §A.6 p16 + §H.8 Table 4 §8 p36 — Register
    // order ORCON < NOFORN < RELIDO.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
    a.dissem_controls = vec![DissemControl::Relido, DissemControl::Nf, DissemControl::Oc].into();
    assert_eq!(render_banner(a), "TOP SECRET//ORCON/NOFORN/RELIDO");
}

#[test]
fn dissem_bare_rel_dropped_when_rel_to_present() {
    // Authority: CAPCO-2016 §H.8 + render_dissem.rs module doc —
    // when REL TO list is non-empty, the bare `REL` token in
    // `dissem_controls` is dropped (the REL TO axis emits
    // `REL TO USA, ...` once).
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_controls = vec![DissemControl::Rel].into();
    a.rel_to = vec![cc("USA"), cc("GBR")].into();
    assert_eq!(render_banner(a), "SECRET//REL TO USA, GBR");
}

// ===========================================================================
// REL TO axis (CAPCO-2016 §A.6 p16, §H.8 p150-151)
// ===========================================================================

#[test]
fn rel_to_usa_first() {
    // Authority: CAPCO-2016 §H.8 p150-151 — USA must be first.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.rel_to = vec![cc("GBR"), cc("USA"), cc("JPN")].into();
    assert_eq!(render_banner(a), "SECRET//REL TO USA, GBR, JPN");
}

#[test]
fn rel_to_trigraphs_alpha() {
    // Authority: CAPCO-2016 §H.8 p150-151 — after USA, trigraphs in
    // ascending alpha order.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.rel_to = vec![cc("USA"), cc("JPN"), cc("GBR"), cc("AUS")].into();
    assert_eq!(render_banner(a), "SECRET//REL TO USA, AUS, GBR, JPN");
}

#[test]
fn rel_to_tetragraphs_after_trigraphs() {
    // Authority: CAPCO-2016 §A.6 p16 + §H.8 p150-151 — trigraphs
    // first (alpha), then tetragraphs (alpha).
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.rel_to = vec![cc("USA"), cc("NATO"), cc("GBR"), cc("FVEY")].into();
    assert_eq!(render_banner(a), "SECRET//REL TO USA, GBR, FVEY, NATO");
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "REL TO must contain at least one non-USA")]
fn rel_to_only_usa_panics_in_debug() {
    // Authority: CAPCO-2016 §H.8 p151 line 3715 — `REL TO USA`
    // alone (no other trigraph or tetragraph) is NOT an authorized
    // marking. The renderer carries a `debug_assert!` guard against
    // this upstream-invariant violation; this test pins that guard
    // by exercising the USA-only path and asserting the assertion
    // fires under cfg(debug_assertions).
    //
    // Gated on `#[cfg(debug_assertions)]` because `debug_assert!`
    // compiles to a no-op under `cfg(not(debug_assertions))` (i.e.,
    // `cargo test --release`); without the gate, the `#[should_panic]`
    // expectation would fail under a release-profile test run with
    // "test did not panic as expected". The release-profile path is
    // covered by `rel_to_only_usa_release_emits_unauthorized_form`
    // below, which exercises the no-op-assert path and asserts the
    // renderer's release-build behavior (emits the unauthorized form
    // rather than panicking) — leaving downstream lint rules to catch
    // the violation.
    //
    // Pre-guard, this test asserted `render_banner == "SECRET//REL
    // TO USA"`, which captured the prior (broken) renderer output.
    // The reframe to a `#[should_panic]` invariant pin preserves
    // the test as a regression guard for the assertion itself
    // rather than for the unauthorized output.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.rel_to = vec![cc("USA")].into();
    let _ = render_banner(a);
}

#[cfg(not(debug_assertions))]
#[test]
fn rel_to_only_usa_release_emits_unauthorized_form() {
    // Release-profile counterpart of `rel_to_only_usa_panics_in_debug`.
    // Under `cfg(not(debug_assertions))` the `debug_assert!` in
    // `render_rel_to` is a no-op, so the renderer emits the
    // unauthorized §H.8 p151 form `SECRET//REL TO USA` rather than
    // panicking. This test pins that release-build behavior so a
    // future change that promotes the assert to a runtime `assert!`
    // (which would crash production renders) trips a regression.
    // Downstream lint rules are responsible for catching the
    // unauthorized form in release builds.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.rel_to = vec![cc("USA")].into();
    assert_eq!(render_banner(a), "SECRET//REL TO USA");
}

#[test]
fn rel_to_dedup_duplicates() {
    // Authority: CAPCO-2016 §H.8 p150-151 — REL TO is set semantics;
    // duplicate trigraphs are deduped at canonicalization.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.rel_to = vec![cc("USA"), cc("GBR"), cc("GBR"), cc("JPN")].into();
    assert_eq!(render_banner(a), "SECRET//REL TO USA, GBR, JPN");
}

// ===========================================================================
// Non-IC dissem axis (CAPCO-2016 §A.6 p16, §H.9, Table 4 §9 p36)
// ===========================================================================

#[test]
fn non_ic_dissem_exdis_banner() {
    // Authority: CAPCO-2016 §H.9 p172 — EXDIS propagates to banner
    // when classified.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.non_ic_dissem = vec![NonIcDissem::Exdis].into();
    assert_eq!(render_banner(a), "SECRET//EXDIS");
}

#[test]
fn non_ic_dissem_exdis_portion_form() {
    // Authority: CAPCO-2016 §H.9 Table 4 §9 p36 — EXDIS portion
    // form = `XD`.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.non_ic_dissem = vec![NonIcDissem::Exdis].into();
    assert_eq!(render_portion(a), "S//XD");
}

#[test]
fn non_ic_dissem_register_order() {
    // Authority: CAPCO-2016 §A.6 p16 + §H.9 Table 4 §9 p36 —
    // Register order EXDIS < NODIS.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.non_ic_dissem = vec![NonIcDissem::Nodis, NonIcDissem::Exdis].into();
    assert_eq!(render_banner(a), "SECRET//EXDIS/NODIS");
}

#[test]
fn non_ic_dissem_sbu_nf_portion_hyphenated() {
    // Authority: CAPCO-2016 §A.6 p16 + §H.9 Table 4 §9 p36 — portion
    // form `SBU-NF` (hyphenated; banner `SBU NOFORN` with space).
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Unclassified));
    a.non_ic_dissem = vec![NonIcDissem::SbuNf].into();
    assert_eq!(render_portion(a), "U//SBU-NF");
}

#[test]
fn non_ic_dissem_les_banner_full_form() {
    // Authority: CAPCO-2016 §H.9 p181 — LES banner form = `LES`,
    // propagates to classified banners.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.non_ic_dissem = vec![NonIcDissem::Les].into();
    assert_eq!(render_banner(a), "SECRET//LES");
}

// ===========================================================================
// Declassify-on axis — banner / portion no-op (CAB is a separate block)
// ===========================================================================

#[test]
fn declassify_no_op_in_banner() {
    // Authority: render_declassify.rs module doc — the Declassify On
    // value lives in the CAB, not the banner / portion line. The
    // axis emits nothing.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.declassify_on = Some(IsmDate::Date(2030, 6, 15));
    // Declassify-on has no effect on the banner output.
    assert_eq!(render_banner(a), "SECRET");
}

#[test]
fn declassify_no_op_in_portion() {
    // Same as above for portion scope.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
    a.declassify_on = Some(IsmDate::Date(2030, 6, 15));
    assert_eq!(render_portion(a), "TS");
}

// ===========================================================================
// Cross-axis composition (CAPCO-2016 §A.6 p15-17 Figure 2 ordering)
// ===========================================================================

#[test]
fn full_composition_class_sci_aea_dissem_relto() {
    // Authority: CAPCO-2016 §A.6 p15-17 Figure 2 — the canonical
    // ordering is: classification → SCI → SAR → AEA → FGI → IC dissem
    // → REL TO → non-IC dissem. This exercises composition across
    // multiple axes in one marking.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
    a.sci_markings = vec![SciMarking::new(
        SciControlSystem::Published(marque_ism::SciControlBare::Si),
        Box::new([]),
        None,
    )]
    .into();
    a.dissem_controls = vec![DissemControl::Oc, DissemControl::Nf].into();
    a.rel_to = vec![cc("USA"), cc("GBR")].into();
    assert_eq!(
        render_banner(a),
        "TOP SECRET//SI//ORCON/NOFORN//REL TO USA, GBR"
    );
}

#[test]
fn full_composition_class_aea_dissem() {
    // Authority: CAPCO-2016 §A.6 p15-17 Figure 2 — class → AEA →
    // dissem ordering.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.aea_markings = vec![AeaMarking::Rd(RdBlock::default())].into();
    a.dissem_controls = vec![DissemControl::Nf].into();
    assert_eq!(render_banner(a), "SECRET//RD//NOFORN");
}

// ---------------------------------------------------------------------------
// Force imports stay live (some used only in cfg-conditional paths or
// in fixtures that test failed-to-canonicalize input).
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn _force_unused_imports(_: SciControl) {}
