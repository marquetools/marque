// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Closure-operator runtime tests for `CapcoScheme`.
//!
//! Exercises `<CapcoScheme as MarkingScheme>::closure(...)` end-to-end
//! AND the post-closure FD&R supersession overlay via
//! `<CapcoScheme as MarkingScheme>::project(Scope::Page, ...)`. These
//! tests are the production-side companion to the synthetic-scheme
//! proptests in `crates/scheme/tests/proptest_closure*.rs`:
//! the proptests pin the algebraic properties (extensive / idempotent /
//! monotone) against a bitset `BitMarking`; this file pins the
//! observable cone effects against `CapcoMarking` — the single Trio-1
//! `CLOSURE_NOFORN_CAVEATED` row (union of every caveat trigger) and
//! the NATO row `capco/rel-to-usa-nato-if-nato-classification`.
//!
//! # Post-#704 architecture note
//!
//! Issue #704 retired the `CLOSURE_TABLE.suppressor_mask` gating that
//! prevented Trio 1 / Trio 2 / Trio 3 cones from firing alongside an
//! existing FD&R dominator — the gating violated the closure
//! operator's algebraic monotonicity property
//! (`a ⊑ b ⟹ Cl(a) ⊑ Cl(b)`). The §H.8 p145 NOFORN-dominates /
//! §B.3.a p19 FD&R supersession semantics moved to
//! `CapcoScheme::apply_supersession_overlays`, which runs after
//! `closure()` converges and is invoked from `project()`. Tests
//! that pin "FD&R dominator on input prevents closure-added implicit
//! defaults from coexisting" therefore call `scheme.project(Scope::Page,
//! &[marking])` (single-portion page) to exercise the full pipeline;
//! tests that pin "closure adds the cone fact" call `scheme.closure(...)`
//! directly. Both surfaces are public API for `MarkingScheme`.
//!
//! # Citation anchors
//!
//! - Trio 1 NOFORN rows — CAPCO-2016 §B.3 Table 2 p21 ("Classified,
//!   caveated, on/after 28 Jun 2010 → NOFORN") + §B.3 p20 Note
//!   (caveated/uncaveated structural definitions).
//! - NOFORN supersession (NOFORN dominates REL TO / RELIDO / DISPLAY
//!   ONLY / EYES) — CAPCO-2016 §H.8 p145.
//! - NATO row — CAPCO-2016 §H.7 p127 Notional Example Page 2
//!   `(//CTS//BOHEMIA//REL TO USA, NATO)` (example-derived inference,
//!   see `CLOSURE_REL_TO_USA_NATO` doc comment in `closure.rs`).
//!
//! All citations re-verified against `crates/capco/docs/CAPCO-2016.md`
//! at authorship per Constitution VIII.

use marque_capco::scheme::{CapcoMarking, CapcoScheme};
use marque_ism::{
    AeaMarking, CanonicalAttrs, Classification, CountryCode, DissemControl, MarkingClassification,
    NatoClassification, RdBlock,
};
use marque_scheme::{MarkingScheme, Scope};

// ---------------------------------------------------------------------------
// Marking-construction helpers (test-fixture only)
// ---------------------------------------------------------------------------

/// Classified US portion with no dissem and no FD&R.
fn classified_no_dissem(c: Classification) -> CapcoMarking {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(c));
    CapcoMarking::new(a)
}

/// Classified US portion with a single IC dissem control.
fn classified_with_dissem(c: Classification, dissem: DissemControl) -> CapcoMarking {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(c));
    a.dissem_us = vec![dissem].into_boxed_slice();
    CapcoMarking::new(a)
}

/// Bare NATO portion (no US classification axis populated).
///
/// `NS` (NATO SECRET, portion mark abbreviation `NS`) per CAPCO-2016
/// §G.1 Table 4 p38 — registers the five NATO classification levels
/// (CTS / NS / NC / NR / NU) with the pointer "NATO Protective
/// Markings, refer to Appendix B" for the full grammar.
fn bare_nato_secret() -> CapcoMarking {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
    CapcoMarking::new(a)
}

fn dissem_us_contains(marking: &CapcoMarking, target: DissemControl) -> bool {
    marking.0.dissem_us.contains(&target)
}

fn rel_to_contains(marking: &CapcoMarking, target: CountryCode) -> bool {
    marking.0.rel_to.contains(&target)
}

fn nato_country() -> CountryCode {
    CountryCode::NATO
}

// ---------------------------------------------------------------------------
// Trio 1 (implicit NOFORN) — load-bearing closure firing
// ---------------------------------------------------------------------------

/// Trio 1 fires when a caveat marking (here: `ORCON`) is present without
/// any FD&R decision: closure adds NOFORN.
///
/// Authority: CAPCO-2016 §B.3 Table 2 p21 (classified + caveated +
/// post-28-Jun-2010 → NOFORN); §H.8 p136 (ORCON is a caveat).
#[test]
fn closure_fires_noforn_on_classified_with_orcon() {
    let scheme = CapcoScheme::new();
    let m = classified_with_dissem(Classification::Secret, DissemControl::Oc);

    assert!(
        !dissem_us_contains(&m, DissemControl::Nf),
        "test setup: NOFORN must be absent from the input"
    );
    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on classified + ORCON without FD&R \
         (§B.3 Table 2 p21); dissem_us = {:?}",
        closed.0.dissem_us
    );
    // Extensive: ORCON must survive — closure can only add.
    assert!(
        dissem_us_contains(&closed, DissemControl::Oc),
        "closure must not remove existing facts (extensive property)"
    );
}

/// Trio 1 fires on AEA RD via the AEA arm of `capco/noforn-if-caveated`:
/// an RD marking without any FD&R decision implies NOFORN.
///
/// Authority: CAPCO-2016 §H.6 p104 (RD); §B.3 Table 2 p21.
#[test]
fn closure_fires_noforn_on_classified_with_rd() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.aea_markings = vec![AeaMarking::Rd(RdBlock::default())].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on RD without FD&R (§B.3 Table 2 p21); \
         dissem_us = {:?}",
        closed.0.dissem_us
    );
}

/// Trio 1 row 6: RSEN triggers the implicit-NOFORN closure alongside
/// IMCON / DSEN.
///
/// Authority: §H.8 p132 (RSEN marking template) plus §B.3 Table 2 p21
/// and §B.3 p20 Note (caveated structural definition — "bears no FD&R
/// markings, but has one or more AEA markings, SAP markings, and/or
/// dissemination control marking(s)"). RSEN is a dissemination control
/// per §G.1 Table 4 row 8 p38.
#[test]
fn closure_fires_noforn_on_classified_with_rsen() {
    let scheme = CapcoScheme::new();
    let m = classified_with_dissem(Classification::Secret, DissemControl::Rs);

    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on RSEN without FD&R \
         (caveat per §B.3 p20 Note + §B.3 Table 2 p21); dissem_us = {:?}",
        closed.0.dissem_us
    );
    assert!(
        dissem_us_contains(&closed, DissemControl::Rs),
        "closure must not remove existing facts (extensive property)"
    );
}

/// Post-#704 FD&R supersession: project(Page) with ORCON + RELIDO
/// converges to `{ORCON, NOFORN}` with RELIDO stripped.
///
/// Pipeline trace: closure() adds NOFORN (Row 0 fires on ORCON) and
/// keeps RELIDO (purely-additive Kleene fixpoint); `apply_closed_bits_to`
/// sees NOFORN in the Kleene delta and strips RELIDO per §H.8 p145
/// at the writeback. The supersession overlay (`apply_supersession_overlays`)
/// is then idempotent on this input — no work remains. The §H.8 p145
/// "NOFORN dominates RELIDO" semantic that the previous
/// `MASK_FDR_DOMINATORS` suppressor encoded by *blocking* the cone is
/// now applied by *stripping* the dominated peer after the cone fires.
///
/// Authority: §H.8 p145 (NOFORN: "Cannot be used with REL TO, RELIDO,
/// EYES ONLY, or DISPLAY ONLY"); §B.3 Table 2 p21 (classified +
/// caveated obligation drives the NOFORN addition); §B.3.a p19 (FD&R
/// dominator enumeration).
#[test]
fn project_resolves_orcon_plus_relido_to_noforn() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_us = vec![DissemControl::Oc, DissemControl::Relido].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let out = scheme.project(Scope::Page, &[m]);
    assert!(
        dissem_us_contains(&out, DissemControl::Nf),
        "project must add NOFORN (Trio 1 caveated-default on ORCON \
         per §B.3 Table 2 p21); dissem_us = {:?}",
        out.0.dissem_us
    );
    assert!(
        !dissem_us_contains(&out, DissemControl::Relido),
        "project must strip RELIDO when NOFORN is present \
         (§H.8 p145 NOFORN-dominates supersession overlay); \
         dissem_us = {:?}",
        out.0.dissem_us
    );
    assert!(
        dissem_us_contains(&out, DissemControl::Oc),
        "ORCON must survive (not dominated by NOFORN); dissem_us = {:?}",
        out.0.dissem_us
    );
}

/// FD&R-dominator parity: ORCON + NOFORN in project(Page) is
/// idempotent — closure dedups NOFORN, overlay finds nothing to
/// strip (no dominated peer present).
///
/// Authority: §B.3.a p19 (FD&R-set membership; NOFORN is the most
/// restrictive FD&R marking); §H.8 p145 (NOFORN supersession overlay).
#[test]
fn project_orcon_plus_noforn_is_idempotent() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_us = vec![DissemControl::Oc, DissemControl::Nf].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let out = scheme.project(Scope::Page, &[m.clone()]);
    assert_eq!(
        out.0.dissem_us.len(),
        m.0.dissem_us.len(),
        "project must dedup NOFORN (Trio 1 caveated-default observes \
         NOFORN already present; apply_closed_bits_to skips the add); \
         dissem_us = {:?}",
        out.0.dissem_us
    );
    assert!(dissem_us_contains(&out, DissemControl::Nf));
    assert!(dissem_us_contains(&out, DissemControl::Oc));
}

/// Post-#704 FD&R supersession: project(Page) with ORCON + REL TO USA,
/// GBR converges to `{ORCON, NOFORN}` with `rel_to` cleared.
///
/// Pipeline trace: closure() fires Row 0 (ORCON caveated trigger →
/// NOFORN) and Row 9 (`US_COLLATERAL_CLASSIFIED` → RELIDO); both cone
/// bits land in the Kleene delta; `apply_closed_bits_to`'s NOFORN-in-
/// delta path strips RELIDO from `dissem_us` and clears `rel_to` /
/// `display_only_to` per §H.8 p145. The supersession overlay
/// re-inspects the post-closure state and finds no remaining FD&R
/// peer to strip — idempotent. The previous `MASK_FDR_DOMINATORS`
/// suppressor blocked the cone; the post-#704 overlay strips the
/// dominated peer after the cone fires, which is the §H.8 p145
/// canonical reading ("NOFORN cannot be used with REL TO").
///
/// Authority: §H.8 p145 (NOFORN-dominates supersession overlay);
/// §H.8 p150 (REL TO marking template); §B.3.a p19 (FD&R-set
/// membership).
#[test]
fn project_resolves_orcon_plus_rel_to_to_noforn() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_us = vec![DissemControl::Oc].into_boxed_slice();
    a.rel_to = vec![CountryCode::USA, CountryCode::GBR].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let out = scheme.project(Scope::Page, &[m]);
    assert!(
        dissem_us_contains(&out, DissemControl::Nf),
        "project must add NOFORN (Trio 1 caveated-default on ORCON); \
         dissem_us = {:?}",
        out.0.dissem_us
    );
    assert!(
        !rel_to_contains(&out, CountryCode::USA)
            && !rel_to_contains(&out, CountryCode::GBR),
        "project must clear rel_to when NOFORN is present \
         (§H.8 p145 NOFORN-dominates: REL TO cannot coexist with \
         NOFORN); rel_to = {:?}",
        out.0.rel_to
    );
    assert!(
        dissem_us_contains(&out, DissemControl::Oc),
        "ORCON must survive; dissem_us = {:?}",
        out.0.dissem_us
    );
}

/// Post-#704 FD&R supersession: project(Page) with ORCON + DISPLAY
/// ONLY converges to `{ORCON, NOFORN}` with DISPLAY ONLY stripped.
///
/// Same pipeline shape as the RELIDO case: closure() adds NOFORN;
/// `apply_closed_bits_to` strips DISPLAY ONLY per §H.8 p145.
///
/// Authority: §H.8 p145 (NOFORN-dominates supersession overlay);
/// §H.8 p163 (DISPLAY ONLY marking template); §B.3.a p19.
#[test]
fn project_resolves_orcon_plus_displayonly_to_noforn() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_us = vec![DissemControl::Oc, DissemControl::Displayonly].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let out = scheme.project(Scope::Page, &[m]);
    assert!(
        dissem_us_contains(&out, DissemControl::Nf),
        "project must add NOFORN; dissem_us = {:?}",
        out.0.dissem_us
    );
    assert!(
        !dissem_us_contains(&out, DissemControl::Displayonly),
        "project must strip DISPLAY ONLY when NOFORN is present \
         (§H.8 p145); dissem_us = {:?}",
        out.0.dissem_us
    );
}

/// Post-#704 FD&R supersession: project(Page) with ORCON + EYES
/// converges to `{ORCON, NOFORN}` with EYES stripped.
///
/// Authority: §H.8 p145 (NOFORN-dominates supersession overlay,
/// including EYES); §H.8 p157 (EYES marking template / FD&R
/// designation, deprecated 2017-10-01).
#[test]
fn project_resolves_orcon_plus_eyes_to_noforn() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_us = vec![DissemControl::Oc, DissemControl::Eyes].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let out = scheme.project(Scope::Page, &[m]);
    assert!(
        dissem_us_contains(&out, DissemControl::Nf),
        "project must add NOFORN; dissem_us = {:?}",
        out.0.dissem_us
    );
    assert!(
        !dissem_us_contains(&out, DissemControl::Eyes),
        "project must strip EYES when NOFORN is present \
         (§H.8 p145, EYES is in the dominated FD&R set); \
         dissem_us = {:?}",
        out.0.dissem_us
    );
}

// ---------------------------------------------------------------------------
// NATO row (CLOSURE_REL_TO_USA_NATO)
// ---------------------------------------------------------------------------

/// Bare NATO classification fires the NATO closure row: closure
/// injects both `USA` and `NATO` into the REL TO axis.
///
/// Authority: CAPCO-2016 §H.7 p127 Notional Example Page 2 worked
/// example `(//CTS//BOHEMIA//REL TO USA, NATO)` (example-derived
/// inference per the `CLOSURE_REL_TO_USA_NATO` doc comment) +
/// §G.2 Table 5 p40 alliance-reciprocity ARH grounding. Per D20:
/// the closure row fires at `Severity::Info` (silent lattice-layer
/// fact propagation); S007 owns the text-layer `Severity::Suggest`
/// byte diff. This test reads the post-closure marking state, not
/// any audit output.
#[test]
fn closure_rel_to_usa_nato_fires_on_bare_nato() {
    let scheme = CapcoScheme::new();
    let m = bare_nato_secret();

    assert!(
        m.0.rel_to.is_empty(),
        "test setup: rel_to must be empty in the input"
    );
    let closed = scheme.closure(m);

    assert!(
        rel_to_contains(&closed, CountryCode::USA),
        "closure should inject USA into rel_to on bare-NATO classification \
         (§H.7 p127 + §G.2 Table 5 p40); rel_to = {:?}",
        closed.0.rel_to
    );
    assert!(
        rel_to_contains(&closed, nato_country()),
        "closure should inject NATO into rel_to on bare-NATO classification \
         (§H.7 p127, open-vocab path via cone_derived); rel_to = {:?}",
        closed.0.rel_to
    );
}

/// Post-#704 FD&R supersession: project(Page) with bare NATO + NOFORN
/// strips the closure-added REL TO USA, NATO injection — NOFORN wins
/// the §H.8 p145 conflict.
///
/// Pipeline trace: closure() Row 7 fires (NATO_CLASS trigger, no
/// suppressor) and adds REL_TO_USA bit; the `cone_derived` post-Kleene
/// tail injects `CountryCode::NATO`. NOFORN was in the input, so
/// `apply_closed_bits_to`'s NOFORN-in-delta path doesn't fire (no NOFORN
/// in delta — it was input). The supersession overlay
/// (`apply_supersession_overlays`) sees NOFORN in post-closure
/// `dissem_us` and clears `rel_to` per §H.8 p145, transitioning the
/// (conceptual) RelToBlock to `NofornSuperseded`. Net: rel_to is empty,
/// NOFORN survives.
///
/// Authority: CAPCO-2016 §H.8 p145 (NOFORN dominates REL TO / RELIDO
/// / DISPLAY ONLY / EYES); §H.7 p127 (the NATO row's primary authority
/// whose §H.8 p145 conflict this overlay resolves).
#[test]
fn project_strips_rel_to_usa_nato_when_noforn_present() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
    a.dissem_us = vec![DissemControl::Nf].into_boxed_slice();
    let m = CapcoMarking::new(a);

    assert!(
        m.0.rel_to.is_empty(),
        "test setup: rel_to must be empty in the input"
    );
    let out = scheme.project(Scope::Page, &[m]);

    assert!(
        !rel_to_contains(&out, CountryCode::USA),
        "project must clear rel_to when NOFORN is present (overlay \
         strips the closure-added USA per §H.8 p145); rel_to = {:?}",
        out.0.rel_to
    );
    assert!(
        !rel_to_contains(&out, nato_country()),
        "project must clear rel_to when NOFORN is present (overlay \
         strips the closure-added NATO per §H.8 p145); rel_to = {:?}",
        out.0.rel_to
    );
    assert!(
        dissem_us_contains(&out, DissemControl::Nf),
        "NOFORN must survive (it is the dominator, not the dominated); \
         dissem_us = {:?}",
        out.0.dissem_us
    );
}

// ---------------------------------------------------------------------------
// Trio 1 per-row positive firing
// ---------------------------------------------------------------------------

/// Trio 1 SAR arm of `capco/noforn-if-caveated`: a SAR program
/// triggers implicit-NOFORN closure.
///
/// Authority: §B.3 Table 2 p21 (classified + caveated + post-28-Jun-2010
/// → NOFORN) + §H.5 p101 (SAR is a Special Access Program — a caveat per
/// §B.3 p20 Note).
#[test]
fn closure_fires_noforn_on_sar_marking() {
    use marque_ism::{SarIndicator, SarMarking, SarProgram};

    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.sar_markings = Some(SarMarking::new(
        SarIndicator::Abbrev,
        vec![SarProgram::new("BP", Box::new([]))].into_boxed_slice(),
    ));
    let m = CapcoMarking::new(a);

    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on classified + SAR (§B.3 Table 2 p21 \
         + §H.5 p101); dissem_us = {:?}",
        closed.0.dissem_us
    );
}

/// Trio 1 DOD-UCNI arm of `capco/noforn-if-caveated`: a DOD UCNI
/// marking triggers implicit-NOFORN closure.
///
/// Authority: §B.3 Table 2 p21 + §H.6 p116 (DOD UNCLASSIFIED CONTROLLED
/// NUCLEAR INFORMATION). The DOD variant resolves through `TOK_DCNI`
/// per issue #407; the CAVEATED row's trigger list includes both
/// `TOK_UCNI` (DOE) and `TOK_DCNI` (DOD) because the §B.3 Table 2 p21
/// algebra is grammar-agnostic over which sentinel surfaces the UCNI
/// marking. Closes #518.
#[test]
fn closure_fires_noforn_on_dod_ucni_marking() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.aea_markings = vec![AeaMarking::DodUcni].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on DOD UCNI (§B.3 Table 2 p21 + \
         §H.6 p116); dissem_us = {:?}",
        closed.0.dissem_us
    );
}

/// Post-#704 FD&R supersession parity for DOD UCNI: project(Page) with
/// DOD UCNI + RELIDO converges to `{NOFORN, ...}` with RELIDO stripped
/// per §H.8 p145. Same shape as the ORCON case; pinned per-arm so a
/// regression in the DOD UCNI caveated arm cannot hide behind the
/// shared dominator semantic.
///
/// Authority: §B.3 Table 2 p21 (caveated default fires on DOD UCNI);
/// §H.8 p145 (NOFORN-dominates supersession overlay strips RELIDO);
/// §H.6 p116 (DOD UCNI marking template).
#[test]
fn project_resolves_dod_ucni_plus_relido_to_noforn() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.aea_markings = vec![AeaMarking::DodUcni].into_boxed_slice();
    a.dissem_us = vec![DissemControl::Relido].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let out = scheme.project(Scope::Page, &[m]);
    assert!(
        dissem_us_contains(&out, DissemControl::Nf),
        "project must add NOFORN on DOD UCNI (Trio 1 caveated-default \
         per §B.3 Table 2 p21 + §H.6 p116); dissem_us = {:?}",
        out.0.dissem_us
    );
    assert!(
        !dissem_us_contains(&out, DissemControl::Relido),
        "project must strip RELIDO when NOFORN is present \
         (§H.8 p145 NOFORN-dominates overlay); dissem_us = {:?}",
        out.0.dissem_us
    );
}

/// Trio 1 DOE-UCNI arm of `capco/noforn-if-caveated`: a DOE UCNI
/// marking triggers implicit-NOFORN closure.
///
/// Authority: §B.3 Table 2 p21 + §H.6 p118 (DOE UNCLASSIFIED CONTROLLED
/// NUCLEAR INFORMATION).
#[test]
fn closure_fires_noforn_on_doe_ucni_marking() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.aea_markings = vec![AeaMarking::DoeUcni].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on DOE UCNI (§B.3 Table 2 p21 + \
         §H.6 p118); dissem_us = {:?}",
        closed.0.dissem_us
    );
}

/// Trio 1 FGI arm of `capco/noforn-if-caveated`: an acknowledged FGI
/// marker triggers implicit-NOFORN closure (covers the
/// `AnyInCategory(CAT_FGI_MARKER)` trigger).
///
/// Authority: §B.3 Table 2 p21 + §H.7 p123 (FGI marking template — FGI
/// information has a foreign-government equity and so carries the
/// implicit NOFORN posture absent an explicit FD&R decision).
#[test]
fn closure_fires_noforn_on_fgi_marking() {
    use marque_ism::FgiMarker;

    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.fgi_marker = FgiMarker::acknowledged([CountryCode::GBR]);
    let m = CapcoMarking::new(a);

    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on classified + FGI (§B.3 Table 2 \
         p21 + §H.7 p123); dissem_us = {:?}",
        closed.0.dissem_us
    );
}

/// Trio 1 LIMDIS arm of `capco/noforn-if-caveated`: a LIMDIS non-IC
/// dissem control triggers implicit-NOFORN closure.
///
/// Authority: §B.3 Table 2 p21 + §H.9 p170 (LIMITED DISTRIBUTION).
#[test]
fn closure_fires_noforn_on_limdis_marking() {
    use marque_ism::NonIcDissem;

    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.non_ic_dissem = vec![NonIcDissem::Limdis].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on classified + LIMDIS (§B.3 Table 2 \
         p21 + §H.9 p170); dissem_us = {:?}",
        closed.0.dissem_us
    );
}

/// Trio 1 NNPI arm of `capco/noforn-if-caveated`: an NNPI non-IC
/// dissem control triggers implicit-NOFORN closure.
///
/// NNPI is an ODNI-registered non-IC dissem control whose governing
/// authority (10 USC 7314 / 50 USC 2511 — Naval Nuclear Propulsion
/// Program) lives outside IC marking policy; CAPCO-2016 §G.1 Table 4
/// and §H.9 do not enumerate it. The closure fires by the universal
/// non-IC-dissem principle: the IC cannot presume releasability or
/// RELIDO-suitability of information governed by policy regimes
/// outside IC marking authority, so absent an explicit FD&R decision
/// implicit NOFORN is the conservative default.
///
/// Authority: §B.3 Table 2 p21, §B.3 p20 Note (caveated structural
/// definition), and ODNI `CVEnumISMNonIC.xml` (NNPI registration).
#[test]
fn closure_fires_noforn_on_nnpi_marking() {
    use marque_ism::NonIcDissem;

    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.non_ic_dissem = vec![NonIcDissem::Nnpi].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on classified + NNPI (§B.3 Table 2 \
         p21 + ODNI CVEnumISMNonIC); dissem_us = {:?}",
        closed.0.dissem_us
    );
}

/// Post-#704 FD&R supersession parity for NNPI: project(Page) with NNPI
/// + RELIDO converges to `{NOFORN, ...}` with RELIDO stripped per
/// §H.8 p145.
///
/// Authority: §B.3 Table 2 p21 (caveated default fires on NNPI per the
/// non-IC-dissem-implies-NOFORN principle); §H.8 p145 (NOFORN-dominates
/// supersession overlay strips RELIDO); ODNI `CVEnumISMNonIC.xml` (NNPI
/// registration).
#[test]
fn project_resolves_nnpi_plus_relido_to_noforn() {
    use marque_ism::NonIcDissem;

    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.non_ic_dissem = vec![NonIcDissem::Nnpi].into_boxed_slice();
    a.dissem_us = vec![DissemControl::Relido].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let out = scheme.project(Scope::Page, &[m]);
    assert!(
        dissem_us_contains(&out, DissemControl::Nf),
        "project must add NOFORN on NNPI (Trio 1 caveated-default per \
         §B.3 Table 2 p21); dissem_us = {:?}",
        out.0.dissem_us
    );
    assert!(
        !dissem_us_contains(&out, DissemControl::Relido),
        "project must strip RELIDO when NOFORN is present \
         (§H.8 p145 NOFORN-dominates overlay); dissem_us = {:?}",
        out.0.dissem_us
    );
}

/// Trio 1 IMCON arm of `capco/noforn-if-caveated`: an IMCON dissem
/// triggers implicit-NOFORN closure (separately from the RSEN arm
/// covered above).
///
/// Authority: §B.3 Table 2 p21 + §H.8 p142 (CONTROLLED IMAGERY — IMCON
/// is a caveat per §B.3 p20 Note).
#[test]
fn closure_fires_noforn_on_classified_with_imcon() {
    let scheme = CapcoScheme::new();
    let m = classified_with_dissem(Classification::Secret, DissemControl::Imc);

    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on IMCON without FD&R (§B.3 Table 2 \
         p21 + §H.8 p142); dissem_us = {:?}",
        closed.0.dissem_us
    );
    assert!(
        dissem_us_contains(&closed, DissemControl::Imc),
        "closure must not remove existing facts (extensive property)"
    );
}

/// Trio 1 DSEN arm of `capco/noforn-if-caveated`: a DSEN dissem
/// triggers implicit-NOFORN closure (separately from the RSEN and
/// IMCON arms).
///
/// Authority: §B.3 Table 2 p21 + §H.8 p159 (DEA SENSITIVE — DSEN is a
/// caveat per §B.3 p20 Note).
#[test]
fn closure_fires_noforn_on_classified_with_dsen() {
    let scheme = CapcoScheme::new();
    let m = classified_with_dissem(Classification::Secret, DissemControl::Dsen);

    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on DSEN without FD&R (§B.3 Table 2 \
         p21 + §H.8 p159); dissem_us = {:?}",
        closed.0.dissem_us
    );
    assert!(
        dissem_us_contains(&closed, DissemControl::Dsen),
        "closure must not remove existing facts (extensive property)"
    );
}

// ---------------------------------------------------------------------------
// Per-arm parity — every individual `TokenRef` in the unified
// CAVEATED row's trigger list fires the closure (issue #522 follow-up).
//
// The compact fixtures below close the per-arm coverage gap noted in
// the PR #529 review: the historical per-row tests skipped FRD, TFNI,
// ORCON-USGOV, LES, SBU, and SSI even though every one of them is a
// distinct entry in the CAVEATED row's trigger list. Without these,
// a future edit that silently drops an arm from the trigger list
// could pass the existing closure_runtime suite. Each arm pins its
// per-token §-citation (re-verified against `crates/capco/docs/CAPCO-2016.md`).
// ---------------------------------------------------------------------------

/// FRD arm — Formerly Restricted Data implies NOFORN.
/// Authority: §H.6 p111 (FRD marking template) + §B.3 Table 2 p21.
#[test]
fn closure_fires_noforn_on_frd_marking() {
    use marque_ism::FrdBlock;
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.aea_markings = vec![AeaMarking::Frd(FrdBlock::default())].into_boxed_slice();
    let m = CapcoMarking::new(a);
    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on FRD (§H.6 p111 + §B.3 Table 2 p21); \
         dissem_us = {:?}",
        closed.0.dissem_us
    );
}

/// TFNI arm — Transclassified Foreign Nuclear Information implies NOFORN.
/// Authority: §H.6 p120 (TFNI marking template) + §B.3 Table 2 p21.
#[test]
fn closure_fires_noforn_on_tfni_marking() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.aea_markings = vec![AeaMarking::Tfni].into_boxed_slice();
    let m = CapcoMarking::new(a);
    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on TFNI (§H.6 p120 + §B.3 Table 2 p21); \
         dissem_us = {:?}",
        closed.0.dissem_us
    );
}

/// ORCON-USGOV arm — distinct from ORCON, must fire its own trigger.
/// Authority: §H.8 p139 (ORCON-USGOV marking template) + §B.3 Table 2 p21.
#[test]
fn closure_fires_noforn_on_orcon_usgov_marking() {
    let scheme = CapcoScheme::new();
    let m = classified_with_dissem(Classification::Secret, DissemControl::OcUsgov);
    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on ORCON-USGOV (§H.8 p139 + §B.3 Table 2 p21); \
         dissem_us = {:?}",
        closed.0.dissem_us
    );
    assert!(
        dissem_us_contains(&closed, DissemControl::OcUsgov),
        "closure must not remove existing facts (extensive property)"
    );
}

/// LES arm — Law Enforcement Sensitive non-IC dissem.
/// Authority: §H.9 p181 (LES marking template) + §B.3 Table 2 p21.
#[test]
fn closure_fires_noforn_on_les_marking() {
    use marque_ism::NonIcDissem;
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.non_ic_dissem = vec![NonIcDissem::Les].into_boxed_slice();
    let m = CapcoMarking::new(a);
    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on LES (§H.9 p181 + §B.3 Table 2 p21); \
         dissem_us = {:?}",
        closed.0.dissem_us
    );
}

/// SBU arm — Sensitive But Unclassified non-IC dissem (bare SBU, not
/// SBU-NF which carries its own PageRewrite).
/// Authority: §H.9 p176 (SBU marking template) + §B.3 Table 2 p21.
#[test]
fn closure_fires_noforn_on_sbu_marking() {
    use marque_ism::NonIcDissem;
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.non_ic_dissem = vec![NonIcDissem::Sbu].into_boxed_slice();
    let m = CapcoMarking::new(a);
    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on SBU (§H.9 p176 + §B.3 Table 2 p21); \
         dissem_us = {:?}",
        closed.0.dissem_us
    );
}

/// SSI arm — Sensitive Security Information non-IC dissem.
/// Authority: §H.9 p189 (SSI marking template) + §B.3 Table 2 p21.
#[test]
fn closure_fires_noforn_on_ssi_marking() {
    use marque_ism::NonIcDissem;
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.non_ic_dissem = vec![NonIcDissem::Ssi].into_boxed_slice();
    let m = CapcoMarking::new(a);
    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on SSI (§H.9 p189 + §B.3 Table 2 p21); \
         dissem_us = {:?}",
        closed.0.dissem_us
    );
}

// ---------------------------------------------------------------------------
// Algebraic properties — extensive, idempotent (operator-level)
// ---------------------------------------------------------------------------

/// Idempotence at the project() layer: `project(project(m)) == project(m)`.
///
/// Algebraic obligation per `marque-applied.md` §4.7.3. Post-#704
/// `closure()` itself is purely additive but not fully idempotent
/// at the CanonicalAttrs layer — `apply_closed_bits_to`'s NOFORN-in-
/// delta strip path depends on whether NOFORN was an input bit or a
/// closure-added bit. The supersession overlay
/// (`CapcoScheme::apply_supersession_overlays`) at the project()
/// boundary closes this gap: the overlay observes the post-closure
/// state and applies the §H.8 p145 strip unconditionally, so
/// `project()` is fully idempotent end-to-end.
#[test]
fn project_is_idempotent_on_orcon_marking() {
    let scheme = CapcoScheme::new();
    let m = classified_with_dissem(Classification::Secret, DissemControl::Oc);

    let once = scheme.project(Scope::Page, &[m]);
    let twice = scheme.project(Scope::Page, &[once.clone()]);

    assert_eq!(
        once, twice,
        "project must be idempotent: project(project(m)) == project(m)"
    );
}

/// Idempotence (NATO row): the NATO closure row converges on the
/// first iteration's fixpoint.
#[test]
fn closure_is_idempotent_on_bare_nato() {
    let scheme = CapcoScheme::new();
    let m = bare_nato_secret();

    let once = scheme.closure(m);
    let twice = scheme.closure(once.clone());

    assert_eq!(
        once, twice,
        "closure must be idempotent on bare-NATO inputs"
    );
}

/// Extensive: `closure(m) ⊒ m` over the dissem axis. Every fact
/// present in the input is present in the closure output — including
/// the firing case where a Trio-1 row adds NOFORN alongside the
/// existing ORCON.
///
/// Fixture: classified + ORCON, no FD&R. Closure fires the ORCON
/// Trio-1 row (§B.3 Table 2 p21 caveated → NOFORN), adding NOFORN;
/// the input ORCON must also survive.
#[test]
fn closure_is_extensive_on_dissem_axis() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_us = vec![DissemControl::Oc].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let closed = scheme.closure(m.clone());

    // Every input dissem must survive in the output (extensive).
    for input_dissem in m.0.dissem_us.iter() {
        assert!(
            dissem_us_contains(&closed, *input_dissem),
            "closure must be extensive: input dissem {:?} missing from \
             closure output {:?}",
            input_dissem,
            closed.0.dissem_us
        );
    }
    // NOFORN must be added by the ORCON Trio-1 row firing.
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "ORCON Trio-1 row should add NOFORN to the extensive closure \
         output; dissem_us = {:?}",
        closed.0.dissem_us
    );
}

// ---------------------------------------------------------------------------
// Convergence — the operator must reach a fixed point well within
// MAX_CLOSURE_ITERATIONS=16.
// ---------------------------------------------------------------------------

/// Multiple catalog rows applied to a single classified marking with
/// triggers for AEA-RD, ORCON, RSEN+IMCON, and non-IC-controls (LIMDIS):
/// closure must converge within `MAX_CLOSURE_ITERATIONS`.
///
/// We can't assert iteration count directly (it's encapsulated in the
/// operator), but a panic from the `MAX_CLOSURE_ITERATIONS` cap would
/// fail this test — that's the convergence signal we care about.
///
/// Every trigger should fire its row, and all Trio-1 rows that fire
/// contribute NOFORN; idempotence ensures it appears exactly once on
/// the dissem axis.
#[test]
fn closure_converges_within_max_iterations_on_multi_trigger_marking() {
    use marque_ism::NonIcDissem;

    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.aea_markings = vec![AeaMarking::Rd(RdBlock::default())].into_boxed_slice();
    a.dissem_us = vec![DissemControl::Oc, DissemControl::Rs, DissemControl::Imc].into_boxed_slice();
    a.non_ic_dissem = vec![NonIcDissem::Limdis].into_boxed_slice();
    let m = CapcoMarking::new(a);

    // If closure() reaches MAX_CLOSURE_ITERATIONS without converging,
    // it panics — failing this test. A clean return is the convergence
    // signal.
    let closed = scheme.closure(m);

    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "multi-trigger marking should converge with NOFORN injected \
         (all Trio-1 rows that fire contribute NOFORN; idempotence \
         ensures it appears exactly once); dissem_us = {:?}",
        closed.0.dissem_us
    );
}

// ---------------------------------------------------------------------------
// Negative / safety — closure does not over-fire.
// ---------------------------------------------------------------------------

/// A bare US classification with no caveat receives the Trio 2
/// implicit-RELIDO injection but no NOFORN (the §B.3 Table 2 p21
/// FD&R consequence for uncaveated classified is RELIDO, not
/// NOFORN). This was a no-op pre-Issue #524 Phase 3 (the Trio 2
/// `CLOSURE_RELIDO_US_CLASS` row did not exist; the pre-Phase-3
/// comment explicitly noted "the per-marking sentinels its triggers
/// require do not yet exist"). Phase 3 wires
/// `TOK_US_CLASSIFIED` as the trigger and the closure now adds
/// RELIDO per `marque-applied.md` Section 4.7.5.
///
/// The "does not overfire" property is preserved in its actual
/// load-bearing form: NOFORN must NOT be added (only RELIDO).
#[test]
fn closure_adds_relido_but_not_noforn_on_uncaveated_classified() {
    let scheme = CapcoScheme::new();
    let m = classified_no_dissem(Classification::Secret);

    let closed = scheme.closure(m);

    assert!(
        dissem_us_contains(&closed, DissemControl::Relido),
        "Phase 3 CLOSURE_RELIDO_US_CLASS must add RELIDO to bare US (S); \
         dissem_us = {:?}",
        closed.0.dissem_us,
    );
    assert!(
        !dissem_us_contains(&closed, DissemControl::Nf),
        "uncaveated (S) must NOT receive implicit NOFORN (Trio 1 \
         requires a caveat trigger; bare classification alone is not \
         a caveat per §B.3 p20); dissem_us = {:?}",
        closed.0.dissem_us,
    );
}

// ---------------------------------------------------------------------------
// PR 4b-D.2 Commit 6 — cone-trigger short-circuit (behavioral)
// ---------------------------------------------------------------------------
//
// These tests assert the *observable* behavior of the short-circuit
// (closure is a no-op when no triggers fire; closure still contributes
// when a trigger fires). The *predicate-direct* tests for
// `any_closure_trigger_fires` live in
// `crates/capco/src/scheme/tests.rs` (in-crate so they can call the
// `pub(crate)` predicate directly without forcing it to `pub`).

/// Closure is a no-op on a bare unclassified portion — no SAR / AEA /
/// FGI / ORCON / RSEN / IMCON / DSEN / LIMDIS / LES / SBU / SSI /
/// NATO-class trigger is present. The short-circuit returns the input
/// unchanged without entering the fixpoint loop.
///
/// This is the architect's R-1 mitigation: bench corpus's typical
/// portion has no closure-rule trigger, so the short-circuit skips
/// the fixpoint loop on the common case.
#[test]
fn closure_short_circuits_on_bare_unclassified() {
    let scheme = CapcoScheme::new();
    let m = CapcoMarking::new(CanonicalAttrs::default());
    let closed = scheme.closure(m.clone());
    assert_eq!(m, closed, "closure must be a no-op when no triggers fire");
}

/// Closure is NOT a no-op on a classified-but-uncaveated portion
/// (`(S)`) as of Issue #524 Phase 3: the Trio 2
/// `CLOSURE_RELIDO_US_CLASS` row fires on `TOK_US_CLASSIFIED` and
/// injects RELIDO. The short-circuit predicate correctly reports
/// `true` here (the trigger fires) so the fixpoint runs and
/// produces the new fact. Pre-Phase-3 this case short-circuited
/// to a no-op; the flip is intentional.
#[test]
fn closure_runs_fixpoint_on_uncaveated_classified() {
    let scheme = CapcoScheme::new();
    let m = classified_no_dissem(Classification::Secret);
    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Relido),
        "CLOSURE_RELIDO_US_CLASS must add RELIDO to bare US (S) \
         per Phase 3 (marque-applied Section 4.7.5); dissem_us = {:?}",
        closed.0.dissem_us,
    );
}

/// Closure still contributes when a trigger fires. `(S//OC)` carries
/// ORCON; the short-circuit does NOT skip; the fixpoint runs and
/// NOFORN is injected.
#[test]
fn closure_does_not_short_circuit_when_trigger_fires() {
    let scheme = CapcoScheme::new();
    let m = classified_with_dissem(Classification::Secret, DissemControl::Oc);
    let closed = scheme.closure(m);
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure must inject NOFORN when ORCON is present (the \
         short-circuit must not skip a productive fixpoint); \
         closed.dissem_us = {:?}",
        closed.0.dissem_us,
    );
}

/// Project converges to `{ORCON, NOFORN}` on `(S//OC//NF)` —
/// triggers fire (ORCON triggers Row 0; US_COLLATERAL_CLASSIFIED
/// triggers Row 9) but the supersession overlay strips the
/// closure-added RELIDO per §H.8 p145. Post-#704: the closure()
/// layer is purely additive (it now adds RELIDO unconditionally
/// on US-classified inputs), and the project() boundary's overlay
/// resolves the §H.8 p145 conflict.
///
/// Authority: §H.8 p145 (NOFORN-dominates supersession); §B.3 Table 2
/// p21 (caveated-default obligation on ORCON).
#[test]
fn project_resolves_orcon_plus_noforn_no_relido() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_us = vec![DissemControl::Oc, DissemControl::Nf].into_boxed_slice();
    let m = CapcoMarking::new(a);
    let out = scheme.project(Scope::Page, &[m.clone()]);
    assert!(dissem_us_contains(&out, DissemControl::Oc));
    assert!(dissem_us_contains(&out, DissemControl::Nf));
    assert!(
        !dissem_us_contains(&out, DissemControl::Relido),
        "project must strip RELIDO when NOFORN is present (§H.8 p145 \
         supersession overlay); dissem_us = {:?}",
        out.0.dissem_us
    );
    // Length stable end-to-end: input {OC, NF}, output {OC, NF}.
    assert_eq!(out.0.dissem_us.len(), m.0.dissem_us.len());
}
