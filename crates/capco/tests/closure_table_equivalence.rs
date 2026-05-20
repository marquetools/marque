// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Equivalence cross-check between the existing fn-pointer closure
//! catalog (`CAPCO_CLOSURE_RULES`, walked by `CapcoScheme::closure`)
//! and the bitmask-form catalog `CLOSURE_TABLE` (walked by `close`).
//!
//! PR-C ships the bitmask table unused on the production path; this
//! integration test is the load-bearing transitional gate that
//! verifies the two catalogs converge to the same closed-vocab fact
//! set across a curated fixture set. PR-D's parity gate (P5 in plan
//! §6) lifts this to a proptest over `arb_closed_vocab_attrs`; PR-C
//! covers the hand-built worked examples drawn from
//! `phase2_closure_pin` + `fdr_dominators_runtime_pin` + the §H.8 /
//! §H.4 marking templates.
//!
//! # What "equivalence" means here
//!
//! The two paths produce different surface shapes:
//!
//! - **fn-pointer path** (`scheme.closure(marking)`) — mutates a
//!   `CapcoMarking` directly. Adds NOFORN/ORCON/RELIDO via dissem-
//!   axis writes; adds USA (closed-vocab) AND NATO (open-vocab) to
//!   `rel_to`.
//! - **bitmask path** (`close(derive_bits(attrs))` +
//!   `apply_closed_bits_to`) — mutates a `CanonicalAttrs` clone.
//!   Adds NOFORN/ORCON/RELIDO via the dissem rebuild; adds USA to
//!   `rel_to` (closed-vocab static cone). **Does NOT add NATO** —
//!   that's the `cone_derived` open-vocab tail PR-D wires outside
//!   the bitmask loop.
//!
//! The comparison therefore checks: the dissem-axis cone-output
//! atoms (NOFORN / ORCON / RELIDO / EYES / Displayonly / Rel / Relido
//! presence in `dissem_us`) and the USA-in-`rel_to` status. The NATO
//! tetragraph in the fn-pointer path's `rel_to` is intentionally
//! ignored in the comparison — PR-D's equivalence proptest covers
//! the combined closed + open-vocab path.

use marque_capco::closure_table::close;
use marque_capco::fact_bitmask::{apply_closed_bits_to, derive_bits};
use marque_capco::scheme::{CapcoMarking, CapcoScheme};
use marque_ism::{
    AeaMarking, CanonicalAttrs, Classification, CountryCode, DissemControl, FgiClassification,
    FgiMarker, JointClassification, MarkingClassification, RdBlock, SarIndicator, SarMarking,
    SarProgram, SciCompartment, SciControlBare, SciControlSystem, SciMarking,
};
use marque_scheme::MarkingScheme;
use smol_str::SmolStr;

// ---------------------------------------------------------------------------
// Comparison shape
// ---------------------------------------------------------------------------

/// The closed-vocab cone-output atoms the two paths must agree on.
///
/// **Cone outputs** (NOFORN / ORCON / RELIDO / REL_TO_USA) are the
/// four atoms in `APPLY_ELIGIBLE_MASK`; both paths actively write
/// these. Divergence here would indicate a row's trigger or
/// suppressor drift between the bitmask and fn-pointer catalogs.
///
/// **Defensive observation fields** (`has_eyes`, `has_displayonly`,
/// `has_rel`, `display_only_to_is_empty`) — no closure row in
/// `CLOSURE_TABLE` emits EYES, DISPLAYONLY, or REL as cone outputs,
/// and the apply-time §H.8 p145 supersession overlay clears
/// `display_only_to` only when NOFORN is added. These fields are
/// included so a future cone-row addition that introduces one of
/// these atoms (e.g., a hypothetical "FGI ⇒ EYES" row) immediately
/// exercises both paths through the same comparison shape — they
/// are not currently load-bearing for equivalence but close the
/// "silent divergence on new cone atom" drift class.
///
/// NATO presence in `rel_to` is intentionally elided per the file
/// doc comment (open-vocab tail handled by PR-D outside the bitmask
/// loop).
#[derive(Debug, PartialEq, Eq)]
struct ClosedVocabView {
    has_noforn: bool,
    has_orcon: bool,
    has_relido: bool,
    has_eyes: bool,
    has_displayonly: bool,
    has_rel: bool,
    rel_to_has_usa: bool,
    display_only_to_is_empty: bool,
}

impl ClosedVocabView {
    fn from_attrs(attrs: &CanonicalAttrs) -> Self {
        Self {
            has_noforn: attrs.dissem_iter().any(|d| *d == DissemControl::Nf),
            has_orcon: attrs.dissem_iter().any(|d| *d == DissemControl::Oc),
            has_relido: attrs.dissem_iter().any(|d| *d == DissemControl::Relido),
            has_eyes: attrs.dissem_iter().any(|d| *d == DissemControl::Eyes),
            has_displayonly: attrs
                .dissem_iter()
                .any(|d| *d == DissemControl::Displayonly),
            has_rel: attrs.dissem_iter().any(|d| *d == DissemControl::Rel),
            rel_to_has_usa: attrs.rel_to.contains(&CountryCode::USA),
            display_only_to_is_empty: attrs.display_only_to.is_empty(),
        }
    }
}

/// Run both closure paths on the input marking and assert the
/// closed-vocab cone-output views match. The error message names the
/// fixture and the divergence so failure points to the row that
/// drifted.
fn assert_equivalent(fixture_label: &str, marking: CapcoMarking) {
    let scheme = CapcoScheme::new();

    // Path A — fn-pointer catalog via CapcoScheme::closure.
    let fn_pointer_closed = scheme.closure(marking.clone());
    let view_fn_pointer = ClosedVocabView::from_attrs(&fn_pointer_closed.0);

    // Path B — bitmask catalog via close + apply_closed_bits_to.
    let mut via_bitmask = marking.0.clone();
    let input_bits = derive_bits(&via_bitmask);
    let closed_bits = close(input_bits);
    apply_closed_bits_to(&mut via_bitmask, closed_bits, input_bits);
    let view_bitmask = ClosedVocabView::from_attrs(&via_bitmask);

    assert_eq!(
        view_fn_pointer,
        view_bitmask,
        "{fixture_label}: fn-pointer / bitmask closure paths diverge \
         on closed-vocab cone outputs.\n  \
         fn-pointer view: {view_fn_pointer:#?}\n  \
         bitmask view:    {view_bitmask:#?}\n  \
         fn-pointer dissem_us: {:?}\n  \
         bitmask    dissem_us: {:?}\n  \
         fn-pointer rel_to:    {:?}\n  \
         bitmask    rel_to:    {:?}",
        fn_pointer_closed.0.dissem_us,
        via_bitmask.dissem_us,
        fn_pointer_closed.0.rel_to,
        via_bitmask.rel_to,
    );
}

// ---------------------------------------------------------------------------
// Fixture builders
// ---------------------------------------------------------------------------

fn us_classified(c: Classification) -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(c));
    a
}

fn sci_compartment(system: SciControlBare, ident: &str, subs: &[&str]) -> SciMarking {
    let comp = SciCompartment::new(
        SmolStr::new(ident),
        subs.iter()
            .map(|s| SmolStr::new(*s))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    SciMarking::new(SciControlSystem::Published(system), Box::new([comp]), None)
}

fn sar_marking_simple(program_id: &str) -> SarMarking {
    SarMarking::new(
        SarIndicator::Abbrev,
        Box::new([SarProgram::new(program_id, Box::new([]))]),
    )
}

// ---------------------------------------------------------------------------
// Worked-example fixtures — Trio 1 (caveated → NOFORN)
// ---------------------------------------------------------------------------

#[test]
fn equivalent_bare_orcon_secret() {
    // ORCON on US Secret. Row 0 fires +NOFORN. Row 9 then sees NOFORN
    // in RELIDO_US_CLASS_SUPPRESSORS → suppressed.
    let mut a = us_classified(Classification::Secret);
    a.dissem_us = vec![DissemControl::Oc].into();
    assert_equivalent("bare-orcon-secret", CapcoMarking::new(a));
}

#[test]
fn equivalent_bare_sar_secret() {
    // SAR program present (Trio 1 SAR trigger) on US Secret. Row 0
    // fires +NOFORN; Row 9 suppressed (NOFORN dominator).
    let mut a = us_classified(Classification::Secret);
    a.sar_markings = Some(sar_marking_simple("BP"));
    assert_equivalent("bare-sar-secret", CapcoMarking::new(a));
}

#[test]
fn equivalent_bare_rd_secret() {
    let mut a = us_classified(Classification::Secret);
    a.aea_markings = Box::new([AeaMarking::Rd(RdBlock::default())]);
    assert_equivalent("bare-rd-secret", CapcoMarking::new(a));
}

#[test]
fn equivalent_bare_doe_ucni_unclassified() {
    // DOE UCNI at UNCLASSIFIED — Trio 1 caveated default applies at
    // any classification level per §H.6 p118. Row 0 fires +NOFORN;
    // Row 9 not triggered (US_COLLATERAL_CLASSIFIED requires
    // Restricted+).
    let mut a = us_classified(Classification::Unclassified);
    a.aea_markings = Box::new([AeaMarking::DoeUcni]);
    assert_equivalent("bare-doe-ucni-unclassified", CapcoMarking::new(a));
}

#[test]
fn equivalent_fgi_acknowledged_secret() {
    // §H.7 p123 FGI acknowledged form — `fgi_marker` populated. Row 0
    // fires +NOFORN via FGI_PRESENT.
    let mut a = us_classified(Classification::Secret);
    a.fgi_marker = FgiMarker::acknowledged([CountryCode::GBR]);
    assert_equivalent("fgi-acknowledged-secret", CapcoMarking::new(a));
}

#[test]
fn equivalent_joint_us_gbr_secret() {
    // JOINT classification (USA + GBR co-owners at SECRET). Per §H.3
    // p56 + §H.8 p154, JOINT_PRESENT is in MASK_FDR_OR_RELIDO_INCOMPAT
    // (Row 8 suppressor) — Row 8 RELIDO_SCI suppressed. Row 9
    // RELIDO_US_CLASS gates on US_COLLATERAL_CLASSIFIED, which is NOT
    // set for a JOINT classification (the bitmask treats JOINT as a
    // distinct branch on the classification axis; `derive_bits` sets
    // `JOINT_PRESENT` instead of the US chain). So Row 9 is also not
    // triggered, and the fixpoint adds nothing — matching the
    // fn-pointer path's behavior (TOK_JOINT is in
    // FDR_OR_RELIDO_INCOMPAT for the SCI Trio 2 row, and JOINT
    // markings don't satisfy TOK_US_COLLATERAL_CLASSIFIED). Stable.
    //
    // This fixture closes the JOINT-axis equivalence-coverage gap
    // flagged by the rust-reviewer for PR-C: bitmask Row 8's
    // MASK_FDR_OR_RELIDO_INCOMPAT includes JOINT_PRESENT (bit 39),
    // but no other fixture exercises the path. If `derive_bits` ever
    // failed to set `JOINT_PRESENT` for a JOINT form, this test
    // would diverge.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: vec![CountryCode::USA, CountryCode::GBR].into_boxed_slice(),
    }));
    assert_equivalent("joint-us-gbr-secret", CapcoMarking::new(a));
}

#[test]
fn equivalent_joint_us_gbr_secret_with_sci_present() {
    // JOINT + bare SCI (SI-Z9). Row 8 (RELIDO_SCI) triggered by
    // SCI_PRESENT — but suppressed by JOINT_PRESENT in
    // MASK_FDR_OR_RELIDO_INCOMPAT. End state: no RELIDO added on
    // either path.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: vec![CountryCode::USA, CountryCode::GBR].into_boxed_slice(),
    }));
    a.sci_markings = Box::new([sci_compartment(SciControlBare::Si, "Z9", &[])]);
    assert_equivalent("joint-us-gbr+sci-z9", CapcoMarking::new(a));
}

#[test]
fn equivalent_fgi_classification_secret() {
    // §H.7 p122 FGI classification axis form. Same FGI_PRESENT path.
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Fgi(FgiClassification {
        countries: vec![CountryCode::GBR].into_boxed_slice(),
        level: Classification::Secret,
    }));
    assert_equivalent("fgi-classification-secret", CapcoMarking::new(a));
}

#[test]
fn equivalent_bare_propin_secret() {
    let mut a = us_classified(Classification::Secret);
    a.dissem_us = vec![DissemControl::Pr].into();
    assert_equivalent("bare-propin-secret", CapcoMarking::new(a));
}

#[test]
fn equivalent_bare_fisa_secret() {
    let mut a = us_classified(Classification::Secret);
    a.dissem_us = vec![DissemControl::Fisa].into();
    assert_equivalent("bare-fisa-secret", CapcoMarking::new(a));
}

// ---------------------------------------------------------------------------
// FD&R-suppression fixtures — Row 0 must NOT fire
// ---------------------------------------------------------------------------

#[test]
fn equivalent_orcon_with_noforn_already_present() {
    // ORCON + NOFORN. Row 0's NOFORN cone is already present; Row 0
    // sees NOFORN in MASK_FDR_DOMINATORS suppressor → stable fixpoint
    // (no change).
    let mut a = us_classified(Classification::Secret);
    a.dissem_us = vec![DissemControl::Oc, DissemControl::Nf].into();
    assert_equivalent("orcon-with-existing-noforn", CapcoMarking::new(a));
}

#[test]
fn equivalent_orcon_with_relido_suppresses_row0() {
    // RELIDO is in FDR_DOMINATORS — Row 0 suppressed even though
    // ORCON is in its trigger. Stable fixpoint.
    let mut a = us_classified(Classification::Secret);
    a.dissem_us = vec![DissemControl::Oc, DissemControl::Relido].into();
    assert_equivalent("orcon-with-relido", CapcoMarking::new(a));
}

#[test]
fn equivalent_orcon_with_rel_to_suppresses_row0() {
    // REL TO populated → REL_TO_PRESENT in FDR_DOMINATORS suppresses
    // Row 0.
    let mut a = us_classified(Classification::Secret);
    a.dissem_us = vec![DissemControl::Oc].into();
    a.rel_to = vec![CountryCode::USA, CountryCode::GBR].into();
    assert_equivalent("orcon-with-rel-to", CapcoMarking::new(a));
}

#[test]
fn equivalent_orcon_with_display_only_suppresses_row0() {
    // DISPLAY ONLY USA — country-list axis populates DISPLAY_ONLY bit
    // (mirroring satisfies_attrs(TOK_DISPLAY_ONLY)). Row 0 suppressed.
    let mut a = us_classified(Classification::Secret);
    a.dissem_us = vec![DissemControl::Oc].into();
    a.display_only_to = vec![CountryCode::USA].into();
    assert_equivalent("orcon-with-display-only-list", CapcoMarking::new(a));
}

// ---------------------------------------------------------------------------
// Per-marking unconditional fixtures — Rows 1-6
// ---------------------------------------------------------------------------

#[test]
fn equivalent_hcs_o() {
    let mut a = us_classified(Classification::TopSecret);
    a.sci_markings = Box::new([sci_compartment(SciControlBare::Hcs, "O", &[])]);
    assert_equivalent("hcs-o-bare", CapcoMarking::new(a));
}

#[test]
fn equivalent_hcs_p_sub() {
    let mut a = us_classified(Classification::TopSecret);
    a.sci_markings = Box::new([sci_compartment(SciControlBare::Hcs, "P", &["JJJ"])]);
    assert_equivalent("hcs-p-sub", CapcoMarking::new(a));
}

#[test]
fn equivalent_hcs_p_bare_does_not_imply_orcon() {
    // Bare HCS-P (no sub) — Row 2 doesn't fire. Row 8 (RELIDO_SCI)
    // does fire on SCI_PRESENT since HCS-P is not in
    // MASK_FDR_OR_RELIDO_INCOMPAT. End state: +RELIDO.
    let mut a = us_classified(Classification::TopSecret);
    a.sci_markings = Box::new([sci_compartment(SciControlBare::Hcs, "P", &[])]);
    assert_equivalent("hcs-p-bare", CapcoMarking::new(a));
}

#[test]
fn equivalent_si_g() {
    let mut a = us_classified(Classification::TopSecret);
    a.sci_markings = Box::new([sci_compartment(SciControlBare::Si, "G", &[])]);
    assert_equivalent("si-g", CapcoMarking::new(a));
}

#[test]
fn equivalent_tk_blfh() {
    let mut a = us_classified(Classification::TopSecret);
    a.sci_markings = Box::new([sci_compartment(SciControlBare::Tk, "BLFH", &[])]);
    assert_equivalent("tk-blfh", CapcoMarking::new(a));
}

#[test]
fn equivalent_tk_idit() {
    let mut a = us_classified(Classification::TopSecret);
    a.sci_markings = Box::new([sci_compartment(SciControlBare::Tk, "IDIT", &[])]);
    assert_equivalent("tk-idit", CapcoMarking::new(a));
}

#[test]
fn equivalent_tk_kand() {
    let mut a = us_classified(Classification::TopSecret);
    a.sci_markings = Box::new([sci_compartment(SciControlBare::Tk, "KAND", &[])]);
    assert_equivalent("tk-kand", CapcoMarking::new(a));
}

// ---------------------------------------------------------------------------
// Trio 2 fixtures — Rows 8-9
// ---------------------------------------------------------------------------

#[test]
fn equivalent_bare_us_secret_no_dissem() {
    // No caveat, no SCI. Row 9 (RELIDO_US_CLASS) fires +RELIDO. Row 0
    // would fire on RELIDO trigger? — wait, RELIDO is in
    // FDR_DOMINATORS so Row 0 is suppressed in the iteration after
    // Row 9 adds RELIDO. Stable fixpoint: +RELIDO only.
    let a = us_classified(Classification::Secret);
    assert_equivalent("bare-us-secret", CapcoMarking::new(a));
}

#[test]
fn equivalent_bare_us_top_secret() {
    let a = us_classified(Classification::TopSecret);
    assert_equivalent("bare-us-top-secret", CapcoMarking::new(a));
}

#[test]
fn equivalent_bare_us_restricted() {
    let a = us_classified(Classification::Restricted);
    assert_equivalent("bare-us-restricted", CapcoMarking::new(a));
}

#[test]
fn equivalent_bare_us_unclassified_does_not_relido() {
    // §H.8 p154 carve-out: Unclassified does NOT trigger RELIDO_US_CLASS.
    let a = us_classified(Classification::Unclassified);
    assert_equivalent("bare-us-unclassified", CapcoMarking::new(a));
}

#[test]
fn equivalent_bare_si_z9_secret() {
    // Bare SCI control SI-Z9 (synthetic compartment that matches no
    // per-compartment sentinel) on US Secret. Row 8 fires +RELIDO via
    // SCI_PRESENT. Row 9 also gates on US_COLLATERAL_CLASSIFIED but
    // intra-iteration mutation means whichever fires first sees the
    // other's RELIDO in its suppressor.
    let mut a = us_classified(Classification::Secret);
    a.sci_markings = Box::new([sci_compartment(SciControlBare::Si, "Z9", &[])]);
    assert_equivalent("bare-si-z9-secret", CapcoMarking::new(a));
}

// ---------------------------------------------------------------------------
// Coexistence fixtures — multiple rows firing in one iteration
// ---------------------------------------------------------------------------

#[test]
fn equivalent_hcs_p_sub_and_tk_blfh() {
    // §H.4 commingled portion — two per-marking rows fire, both add
    // NOFORN (idempotent). Trio 2 rows suppressed by both direct token
    // (HCS_P_SUB, TK_BLFH in FDR_OR_RELIDO_INCOMPAT) AND Kleene chain
    // (NOFORN once added).
    let mut a = us_classified(Classification::TopSecret);
    a.sci_markings = Box::new([
        sci_compartment(SciControlBare::Hcs, "P", &["JJJ"]),
        sci_compartment(SciControlBare::Tk, "BLFH", &[]),
    ]);
    assert_equivalent("hcs-p-sub+tk-blfh", CapcoMarking::new(a));
}

#[test]
fn equivalent_orcon_with_already_dominated_state() {
    // ORCON + NOFORN + RELIDO (which is semantically incoherent
    // pre-closure but exercises stability). NOFORN supersedes RELIDO,
    // but PR-C's bitmask path only handles closure additions — pre-
    // existing RELIDO would survive unless the inverse projection's
    // NOFORN supersession overlay strips it. derive_bits sees both
    // NOFORN and RELIDO; close() produces the same; delta is empty;
    // apply_closed_bits_to is a no-op. The fn-pointer path's existing
    // call into `with_noforn_injected` runs at projection time, not
    // closure time, so it also leaves the marking unchanged. Stable.
    let mut a = us_classified(Classification::Secret);
    a.dissem_us = vec![DissemControl::Oc, DissemControl::Nf, DissemControl::Relido].into();
    assert_equivalent("orcon-with-noforn-relido-stable", CapcoMarking::new(a));
}

// ---------------------------------------------------------------------------
// Empty fixture — both paths must be no-ops
// ---------------------------------------------------------------------------

#[test]
fn equivalent_empty_canonical_attrs() {
    assert_equivalent(
        "empty-canonical-attrs",
        CapcoMarking::new(CanonicalAttrs::default()),
    );
}

#[test]
fn equivalent_unclassified_no_dissem() {
    // UNCLASSIFIED with no other state — no row's trigger fires. The
    // HOT-1 early-exit path PR-D installs would skip the whole loop.
    let a = us_classified(Classification::Unclassified);
    assert_equivalent("unclassified-no-dissem", CapcoMarking::new(a));
}
