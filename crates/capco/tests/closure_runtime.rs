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
    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
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

    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
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

    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
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

/// Post-#704 (refined per redirect brief): project(Page) preserves
/// the user-explicit `(S, ORCON, RELIDO)` input — RELIDO is an
/// explicit FD&R decision per §B.3.a p19, so the §B.3 paragraph b
/// p19 "NOT MARKED PREVIOUSLY" gate forbids the implicit
/// caveated-default NOFORN from firing. The pre-#704
/// `MASK_FDR_DOMINATORS` suppressor encoded exactly this
/// "default if absent" semantic; post-#704 it lives in
/// `default_fill::row0_should_fill`'s `(post_close ∩ MASK_FDR_DOMINATORS == 0)`
/// gate. End-to-end behavior is preserved.
///
/// Authority: §B.3 paragraph b p19 ("not marked previously by the
/// originator"); §B.3.a p19 (FD&R dominator enumeration — RELIDO
/// is canonical); §H.8 p154 (RELIDO grammar — defaulting marking
/// applies absent explicit FD&R).
#[test]
fn project_preserves_orcon_plus_relido_input() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_us = vec![DissemControl::Oc, DissemControl::Relido].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let out = scheme.project(Scope::Page, &[m]);
    assert!(
        !dissem_us_contains(&out, DissemControl::Nf),
        "default-fill Row 0 must NOT add NOFORN when input carries \
         RELIDO (§B.3 paragraph b p19 'NOT MARKED PREVIOUSLY' gate; \
         RELIDO is in MASK_FDR_DOMINATORS); dissem_us = {:?}",
        out.0.dissem_us
    );
    assert!(
        dissem_us_contains(&out, DissemControl::Relido),
        "RELIDO must survive (it is the explicit FD&R decision the \
         default-fill defers to); dissem_us = {:?}",
        out.0.dissem_us
    );
    assert!(
        dissem_us_contains(&out, DissemControl::Oc),
        "ORCON must survive; dissem_us = {:?}",
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

    let out = scheme.project(Scope::Page, std::slice::from_ref(&m));
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

/// Post-#704 (refined per redirect brief): project(Page) preserves
/// the user-explicit `(S, ORCON, REL TO USA, GBR)` input — REL TO
/// is an explicit FD&R decision per §B.3.a p19, so the §B.3
/// paragraph b p19 "NOT MARKED PREVIOUSLY" gate forbids the
/// implicit caveated-default NOFORN from firing. §H.7 p124's
/// "FGI may include US FD&R as circumstances warrant" generalizes
/// the same reading across markings; the explicit REL TO is the
/// author's release decision and survives.
///
/// Authority: §B.3 paragraph b p19 ("not marked previously by the
/// originator"); §B.3.a p19 (REL TO is canonical FD&R); §H.8 p150
/// (REL TO marking template).
#[test]
fn project_preserves_orcon_plus_rel_to_input() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_us = vec![DissemControl::Oc].into_boxed_slice();
    a.rel_to = vec![CountryCode::USA, CountryCode::GBR].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let out = scheme.project(Scope::Page, &[m]);
    assert!(
        !dissem_us_contains(&out, DissemControl::Nf),
        "default-fill Row 0 must NOT add NOFORN when input carries \
         REL TO (§B.3 paragraph b p19 'NOT MARKED PREVIOUSLY' gate; \
         REL_TO_PRESENT is in MASK_FDR_DOMINATORS); dissem_us = {:?}",
        out.0.dissem_us
    );
    assert!(
        rel_to_contains(&out, CountryCode::USA) && rel_to_contains(&out, CountryCode::GBR),
        "rel_to must survive — explicit REL TO is the author's release \
         decision; rel_to = {:?}",
        out.0.rel_to
    );
    assert!(
        dissem_us_contains(&out, DissemControl::Oc),
        "ORCON must survive; dissem_us = {:?}",
        out.0.dissem_us
    );
}

/// Post-#704 (refined): project(Page) preserves the user-explicit
/// `(S, ORCON, DISPLAY ONLY)` input. DISPLAY ONLY is in
/// MASK_FDR_DOMINATORS per §B.3.a p19; default-fill Row 0 skips.
///
/// Authority: §B.3 paragraph b p19; §H.8 p163 (DISPLAY ONLY
/// marking template).
#[test]
fn project_preserves_orcon_plus_displayonly_input() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_us = vec![DissemControl::Oc, DissemControl::Displayonly].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let out = scheme.project(Scope::Page, &[m]);
    assert!(
        !dissem_us_contains(&out, DissemControl::Nf),
        "default-fill must NOT add NOFORN when input carries DISPLAY \
         ONLY (§B.3 paragraph b p19); dissem_us = {:?}",
        out.0.dissem_us
    );
    assert!(
        dissem_us_contains(&out, DissemControl::Displayonly),
        "DISPLAY ONLY must survive (explicit FD&R per §B.3.a p19); \
         dissem_us = {:?}",
        out.0.dissem_us
    );
}

/// Post-#704 FD&R supersession: project(Page) with ORCON + EYES
/// converges to `{ORCON, NOFORN}` with EYES stripped.
///
/// Authority: §H.8 p145 (NOFORN-dominates supersession overlay,
/// including EYES); §H.8 p157 (EYES marking template / FD&R
/// designation, deprecated 2017-10-01).
/// Post-#704 (refined): project(Page) preserves the user-explicit
/// `(S, ORCON, EYES)` input. EYES is in MASK_FDR_DOMINATORS per
/// §H.8 p157 (designated FD&R marking, deprecated 2017-10-01 but
/// still recognized); default-fill Row 0 skips.
///
/// Authority: §B.3 paragraph b p19; §H.8 p157 (EYES marking
/// template / FD&R designation).
#[test]
fn project_preserves_orcon_plus_eyes_input() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.dissem_us = vec![DissemControl::Oc, DissemControl::Eyes].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let out = scheme.project(Scope::Page, &[m]);
    assert!(
        !dissem_us_contains(&out, DissemControl::Nf),
        "default-fill must NOT add NOFORN when input carries EYES \
         (§H.8 p157 FD&R designation); dissem_us = {:?}",
        out.0.dissem_us
    );
    assert!(
        dissem_us_contains(&out, DissemControl::Eyes),
        "EYES must survive (explicit FD&R per §H.8 p157); dissem_us = {:?}",
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
    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));

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

/// Post-#704 (refined): project(Page) with bare NATO + NOFORN does
/// NOT inject the implicit REL TO USA, NATO default — NOFORN is in
/// MASK_FDR_DOMINATORS so default-fill Row 7's
/// `(post_close ∩ MASK_FDR_DOMINATORS == 0)` gate fails. The
/// user-explicit NOFORN survives; rel_to stays empty.
///
/// This is the §H.7 paragraph d (§B.3.d p20) reading: "REL TO USA,
/// [LIST] MUST be used [absent] FD&R marking(s)." When the
/// originating country (or in this case the user) has already
/// prohibited further sharing via NOFORN, the implicit REL TO USA
/// default does NOT apply.
///
/// Authority: §B.3 paragraph b p19 ("not marked previously"); §B.3.d
/// p20 (FGI/NATO REL TO MUST when allowed, NOFORN otherwise); §H.8
/// p145 (NOFORN cannot coexist with REL TO — separately enforced
/// by `apply_supersession_overlays` if the two ever did coexist via
/// some other path).
#[test]
fn project_default_fill_skips_nato_implicit_when_noforn_present() {
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
// Issue #704 end-to-end observability — the four scenarios from the
// PM implementation brief's "Test strategy" section #3. These pin
// the project()-level outcome for the post-#704 architecture
// (purely-additive closure + supersession overlay).
// ---------------------------------------------------------------------------

/// Brief scenario #3: bare SCI portion (no FD&R, no caveat) projects
/// to a marking carrying RELIDO via closure Row 8
/// (`relido-if-sci-and-not-incompatible`).
///
/// Pipeline trace: closure() Row 8 fires (`SCI_PRESENT` trigger;
/// post-#704 no suppressor on the bitmask row). RELIDO added to
/// `dissem_us` via `apply_closed_bits_to`. The supersession overlay
/// observes no NOFORN → no strip. PageRewrites: none of the
/// RELIDO-eviction rewrites trigger (no DISPLAY ONLY, no ORCON,
/// no ORCON-USGOV in the marking).
///
/// Authority: §H.8 p154 (RELIDO grammar — defaulting marking for
/// SCI content absent FD&R); CAPCO-2016 §H.4 (SCI control system
/// grammar).
#[test]
fn project_sci_alone_adds_relido() {
    use marque_ism::{SciCompartment, SciControlBare, SciControlSystem, SciMarking};
    use smol_str::SmolStr;

    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
    // Use a synthetic SI compartment `Z9` that doesn't match any
    // per-marking SCI sentinel (SI-G / HCS-O / HCS-P[sub] / TK-*) so
    // only Row 8 (SCI_PRESENT default) fires. SI itself is admitted
    // per §H.4 p74 (SI grammar).
    let comp = SciCompartment::new(SmolStr::new("Z9"), Box::new([]));
    a.sci_markings = Box::new([SciMarking::new(
        SciControlSystem::Published(SciControlBare::Si),
        Box::new([comp]),
        None,
    )]);
    let m = CapcoMarking::new(a);

    let out = scheme.project(Scope::Page, &[m]);
    assert!(
        dissem_us_contains(&out, DissemControl::Relido),
        "project must add RELIDO on bare SCI (Trio 2 defaulting per \
         §H.8 p154); dissem_us = {:?}",
        out.0.dissem_us
    );
    assert!(
        !dissem_us_contains(&out, DissemControl::Nf),
        "no caveat trigger present — NOFORN must NOT be added; \
         dissem_us = {:?}",
        out.0.dissem_us
    );
}

/// Brief scenario #3: bare SCI + NOFORN. RELIDO is closure-added
/// via Row 8 (post-#704 unsuppressed) and immediately stripped by
/// the §H.8 p145 supersession overlay (NOFORN dominates RELIDO).
/// Net: dissem_us contains NOFORN only on the SCI axis.
///
/// Pipeline trace: closure() Row 8 fires → RELIDO in delta;
/// `apply_closed_bits_to` skips the RELIDO add per `apply_closed_bits_to`'s
/// existing NOFORN-in-input dedup logic (NOFORN was input). The
/// supersession overlay confirms — RELIDO would also have been
/// stripped by the post-closure §H.8 p145 strip if it had been
/// added.
///
/// Authority: §H.8 p145 (NOFORN dominates RELIDO); §H.8 p154
/// (RELIDO grammar — defaulting marking yields when an explicit
/// dominator is present).
#[test]
fn project_sci_plus_noforn_resolves_to_noforn_only() {
    use marque_ism::{SciCompartment, SciControlBare, SciControlSystem, SciMarking};
    use smol_str::SmolStr;

    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::TopSecret));
    a.dissem_us = vec![DissemControl::Nf].into_boxed_slice();
    let comp = SciCompartment::new(SmolStr::new("Z9"), Box::new([]));
    a.sci_markings = Box::new([SciMarking::new(
        SciControlSystem::Published(SciControlBare::Si),
        Box::new([comp]),
        None,
    )]);
    let m = CapcoMarking::new(a);

    let out = scheme.project(Scope::Page, &[m]);
    assert!(
        dissem_us_contains(&out, DissemControl::Nf),
        "NOFORN must survive (it is the dominator); dissem_us = {:?}",
        out.0.dissem_us
    );
    assert!(
        !dissem_us_contains(&out, DissemControl::Relido),
        "post-#704 supersession overlay strips RELIDO when NOFORN is \
         present per §H.8 p145; dissem_us = {:?}",
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

    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
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

    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on DOD UCNI (§B.3 Table 2 p21 + \
         §H.6 p116); dissem_us = {:?}",
        closed.0.dissem_us
    );
}

/// Post-#704: project(Page) on `(S, DOD UCNI, RELIDO)` ends at
/// `dissem_us = [Nf]` — the §H.6 p116 PageRewrite
/// `capco/dod-ucni-promotes-noforn-when-classified` fires because
/// RELIDO is "less restrictive" than NOFORN per §H.8 supersession,
/// and §H.6 p116 prescribes NOFORN promotion in that case. The
/// PageRewrite's FactAdd(NOFORN) then strips the dominated RELIDO
/// via the §H.8 p145 strip in `apply_fact_add`.
///
/// This is end-to-end behavior IDENTICAL to pre-#704. Pre-#704 the
/// closure() suppressor blocked the Row 0 / Row 9 caveated-defaults,
/// but the §H.6 p116 PageRewrite ran independently — same final
/// state. The pre-#704 `closure_dod_ucni_suppressed_by_relido_dominator`
/// test asserted the closure()-layer-only state which is no longer
/// observable through `project()` (post-#704 close() is a no-op on
/// AEA-only inputs).
///
/// Authority: §H.6 p116 (DOD UCNI banner-line guidance: "NOFORN
/// must be applied if a less restrictive FD&R marking would
/// otherwise be conveyed"); §H.8 p145 (NOFORN-dominates RELIDO);
/// §B.3.a p19 (FD&R-set membership).
#[test]
fn project_dod_ucni_plus_relido_promotes_to_noforn_per_h6_p116() {
    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.aea_markings = vec![AeaMarking::DodUcni].into_boxed_slice();
    a.dissem_us = vec![DissemControl::Relido].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let out = scheme.project(Scope::Page, &[m]);
    // §H.6 p116 PageRewrite promotes RELIDO → NOFORN on
    // classified-with-UCNI inputs. Independent of default-fill;
    // identical end-to-end behavior to pre-#704.
    assert!(
        dissem_us_contains(&out, DissemControl::Nf),
        "§H.6 p116 PageRewrite must promote RELIDO → NOFORN on \
         classified DOD UCNI; dissem_us = {:?}",
        out.0.dissem_us
    );
    assert!(
        !dissem_us_contains(&out, DissemControl::Relido),
        "RELIDO must be stripped by §H.8 p145 NOFORN-dominates \
         within apply_fact_add (NOFORN added by PageRewrite); \
         dissem_us = {:?}",
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

    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
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

    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
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

    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
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

    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
    assert!(
        dissem_us_contains(&closed, DissemControl::Nf),
        "closure should inject NOFORN on classified + NNPI (§B.3 Table 2 \
         p21 + ODNI CVEnumISMNonIC); dissem_us = {:?}",
        closed.0.dissem_us
    );
}

/// Post-#704 (refined): project(Page) preserves the user-explicit
/// `(S, NNPI, RELIDO)` input. RELIDO is in MASK_FDR_DOMINATORS per
/// §B.3.a p19; default-fill Row 0's gate fails on the caveated NNPI
/// arm. The explicit RELIDO survives.
///
/// Authority: §B.3 paragraph b p19 ("not marked previously"); §B.3.a
/// p19 (RELIDO is canonical FD&R); ODNI `CVEnumISMNonIC.xml` (NNPI
/// registration).
#[test]
fn project_preserves_nnpi_plus_relido_input() {
    use marque_ism::NonIcDissem;

    let scheme = CapcoScheme::new();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(Classification::Secret));
    a.non_ic_dissem = vec![NonIcDissem::Nnpi].into_boxed_slice();
    a.dissem_us = vec![DissemControl::Relido].into_boxed_slice();
    let m = CapcoMarking::new(a);

    let out = scheme.project(Scope::Page, &[m]);
    assert!(
        !dissem_us_contains(&out, DissemControl::Nf),
        "default-fill must NOT add NOFORN on NNPI when input carries \
         RELIDO (§B.3 paragraph b p19 'NOT MARKED PREVIOUSLY' gate); \
         dissem_us = {:?}",
        out.0.dissem_us
    );
    assert!(
        dissem_us_contains(&out, DissemControl::Relido),
        "RELIDO must survive — the explicit FD&R decision the \
         default-fill defers to; dissem_us = {:?}",
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

    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
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

    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
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
    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
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
    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
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
    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
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
    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
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
    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
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
    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
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
    let twice = scheme.project(Scope::Page, std::slice::from_ref(&once));

    assert_eq!(
        once, twice,
        "project must be idempotent: project(project(m)) == project(m)"
    );
}

/// Post-#704: close() is idempotent on bare-NATO inputs because
/// NATO_CLASS is no longer a close() trigger (Row 7 relocated to
/// `default_fill::row7_should_fill`). close() leaves bare-NATO
/// unchanged; idempotence is trivially preserved.
///
/// Note: `project()` is NOT idempotent across multiple invocations
/// on bare-NATO because `RelToBlock::from_attrs_iter` expands
/// tetragraphs (NATO → 30 constituent country trigraphs) on every
/// join_via_lattice pass. The project() non-idempotence predates
/// #704 and is documented in `crates/capco/src/scheme/marking.rs`'s
/// "PR 4b-D.2 Copilot R1 / decisions.md D24" block; it's why
/// `CapcoMarking` does NOT implement `JoinSemilattice`.
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

    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));

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
    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));

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

    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));

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
    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
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
    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
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
    let closed = scheme.project(Scope::Page, std::slice::from_ref(&m));
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
    let out = scheme.project(Scope::Page, std::slice::from_ref(&m));
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
