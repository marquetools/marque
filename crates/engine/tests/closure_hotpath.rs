// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 4b-D.2 closure-on-the-hot-path integration tests.
//!
//! Exercises [`CapcoScheme::project(Scope::Page, ...)`] post-flip
//! (PR 4b-D.2 Commit 3). The closure operator now runs between the
//! per-axis lattice join and the declarative `PageRewrite` catalog;
//! these tests pin that each of the seven `CLOSURE_NOFORN_*` rules
//! and `CLOSURE_REL_TO_USA_NATO` fires through the production page
//! projection, that the operator is idempotent and monotone in the
//! marking, and that NOFORN-injection at the closure layer
//! correctly composes with the `DissemSet` supersession overlay
//! (§H.8 p145 NOFORN-dominates).
//!
//! Authority: `docs/plans/2026-05-01-lattice-design.md` §3 (e) +
//! §4.7.4 pipeline ordering. Per-row §-citations on the
//! `CLOSURE_NOFORN_*` constants in `crates/capco/src/scheme/closure.rs`.

use marque_capco::scheme::{CapcoMarking, CapcoScheme};
use marque_ism::{
    AeaMarking, CanonicalAttrs, Classification, CountryCode, DissemControl, FgiMarker,
    MarkingClassification, NatoClassification, NonIcDissem, RdBlock, SarIndicator, SarMarking,
    SarProgram, SciControl, SciControlSystem, SciMarking,
};
use marque_scheme::{MarkingScheme, Scope};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn classified_us(level: Classification) -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(level));
    a
}

fn classified_with_dissem(level: Classification, dissem: DissemControl) -> CanonicalAttrs {
    let mut a = classified_us(level);
    a.dissem_us = vec![dissem].into_boxed_slice();
    a
}

fn project_page(portions: &[CanonicalAttrs]) -> CapcoMarking {
    let scheme = CapcoScheme::new();
    let markings: Vec<CapcoMarking> = portions.iter().cloned().map(CapcoMarking::new).collect();
    scheme.project(Scope::Page, &markings)
}

fn dissem_contains(m: &CapcoMarking, target: DissemControl) -> bool {
    m.0.dissem_us.iter().any(|d| d == &target)
}

fn rel_to_contains(m: &CapcoMarking, target: CountryCode) -> bool {
    m.0.rel_to.iter().any(|c| c == &target)
}

// ---------------------------------------------------------------------------
// Trio 1 — implicit NOFORN closure rules (one fixture each)
//
// Per `crates/capco/src/scheme/closure.rs::CAPCO_CLOSURE_RULES`. Each
// CLOSURE_NOFORN_* row fires on `scheme.project(Scope::Page, ...)`
// when its trigger is observed and no FD&R dominator is present.
// ---------------------------------------------------------------------------

/// CLOSURE_NOFORN_SAR (§H.5 p101 + §B.3 Table 2 p21): any SAR program
/// triggers implicit NOFORN through the page projection.
#[test]
fn closure_noforn_sar_fires_on_hotpath() {
    let program = SarProgram::new("EXP", Box::new([]));
    let sar = SarMarking::new(SarIndicator::Abbrev, Box::new([program]));
    let mut portion = classified_us(Classification::Secret);
    portion.sar_markings = Some(sar);
    let projected = project_page(&[portion]);
    assert!(
        dissem_contains(&projected, DissemControl::Nf),
        "closure should inject NOFORN on SAR without FD&R \
         (§H.5 p101 + §B.3 Table 2 p21); dissem_us = {:?}",
        projected.0.dissem_us,
    );
}

/// CLOSURE_NOFORN_AEA_RD (§H.6 p104 + §B.3 Table 2 p21): RD / FRD /
/// TFNI trigger implicit NOFORN.
#[test]
fn closure_noforn_aea_rd_fires_on_hotpath() {
    let mut portion = classified_us(Classification::Secret);
    portion.aea_markings = vec![AeaMarking::Rd(RdBlock::default())].into_boxed_slice();
    let projected = project_page(&[portion]);
    assert!(
        dissem_contains(&projected, DissemControl::Nf),
        "closure should inject NOFORN on RD without FD&R \
         (§H.6 p104 + §B.3 Table 2 p21); dissem_us = {:?}",
        projected.0.dissem_us,
    );
}

/// CLOSURE_NOFORN_UCNI (§H.6 p118 + §B.3 Table 2 p21): DOE UCNI
/// triggers implicit NOFORN through the page projection.
#[test]
fn closure_noforn_ucni_fires_on_hotpath() {
    let mut portion = classified_us(Classification::Unclassified);
    portion.aea_markings = vec![AeaMarking::DoeUcni].into_boxed_slice();
    let projected = project_page(&[portion]);
    assert!(
        dissem_contains(&projected, DissemControl::Nf),
        "closure should inject NOFORN on DOE UCNI without FD&R \
         (§H.6 p118 + §B.3 Table 2 p21); dissem_us = {:?}",
        projected.0.dissem_us,
    );
}

/// CLOSURE_NOFORN_FGI (§H.7 p122 + §B.3 Table 2 p21): FGI marker
/// triggers implicit NOFORN.
#[test]
fn closure_noforn_fgi_fires_on_hotpath() {
    let gbr = CountryCode::try_new(b"GBR").expect("trigraph");
    let mut portion = classified_us(Classification::Secret);
    portion.fgi_marker = FgiMarker::acknowledged([gbr]);
    let projected = project_page(&[portion]);
    assert!(
        dissem_contains(&projected, DissemControl::Nf),
        "closure should inject NOFORN on FGI without FD&R \
         (§H.7 p122 + §B.3 Table 2 p21); dissem_us = {:?}",
        projected.0.dissem_us,
    );
}

/// CLOSURE_NOFORN_ORCON (§H.8 p136 + §B.3 Table 2 p21): ORCON triggers
/// implicit NOFORN. The post-closure supersession overlay does not
/// strip ORCON itself (ORCON and NOFORN coexist per §H.8 p145 — only
/// REL TO / RELIDO / EYES ONLY / DISPLAY ONLY are dominated).
#[test]
fn closure_noforn_orcon_fires_on_hotpath() {
    let portion = classified_with_dissem(Classification::Secret, DissemControl::Oc);
    let projected = project_page(&[portion]);
    assert!(
        dissem_contains(&projected, DissemControl::Nf),
        "closure should inject NOFORN on ORCON without FD&R \
         (§H.8 p136 + §B.3 Table 2 p21); dissem_us = {:?}",
        projected.0.dissem_us,
    );
    // Extensive: ORCON survives the closure.
    assert!(
        dissem_contains(&projected, DissemControl::Oc),
        "closure is extensive — ORCON must survive; dissem_us = {:?}",
        projected.0.dissem_us,
    );
}

/// CLOSURE_NOFORN_RSEN_IMCON_DSEN (§H.8 p132 + §B.3 Table 2 p21):
/// RSEN / IMCON / DSEN trigger implicit NOFORN. Test with RSEN —
/// the same row covers IMCON and DSEN.
#[test]
fn closure_noforn_rsen_fires_on_hotpath() {
    let portion = classified_with_dissem(Classification::Secret, DissemControl::Rs);
    let projected = project_page(&[portion]);
    assert!(
        dissem_contains(&projected, DissemControl::Nf),
        "closure should inject NOFORN on RSEN without FD&R \
         (§H.8 p132 + §B.3 Table 2 p21); dissem_us = {:?}",
        projected.0.dissem_us,
    );
}

/// CLOSURE_NOFORN_NONICCONTROLS (§H.9 p170 + §B.3 Table 2 p21):
/// LIMDIS / LES / SBU / SSI trigger implicit NOFORN. Test with
/// LIMDIS — the same row covers the other three.
#[test]
fn closure_noforn_nonic_limdis_fires_on_hotpath() {
    let mut portion = classified_us(Classification::Unclassified);
    portion.non_ic_dissem = vec![NonIcDissem::Limdis].into_boxed_slice();
    let projected = project_page(&[portion]);
    assert!(
        dissem_contains(&projected, DissemControl::Nf),
        "closure should inject NOFORN on LIMDIS without FD&R \
         (§H.9 p170 + §B.3 Table 2 p21); dissem_us = {:?}",
        projected.0.dissem_us,
    );
}

/// CLOSURE_REL_TO_USA_NATO (§H.7 p127 + §G.2 Table 5 p40): bare NATO
/// classification triggers implicit REL TO USA, NATO via the
/// open-vocab `cone_derived` branch.
#[test]
fn closure_rel_to_usa_nato_fires_on_hotpath() {
    let usa = CountryCode::USA;
    let nato = CountryCode::try_new(b"NATO").expect("tetragraph");
    let mut portion = CanonicalAttrs::default();
    portion.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
    let projected = project_page(&[portion]);
    assert!(
        rel_to_contains(&projected, usa),
        "closure should inject USA into rel_to on bare NATO \
         (§H.7 p127 + §G.2 Table 5 p40); rel_to = {:?}",
        projected.0.rel_to,
    );
    assert!(
        rel_to_contains(&projected, nato),
        "closure should inject NATO into rel_to on bare NATO \
         (§H.7 p127 + §G.2 Table 5 p40); rel_to = {:?}",
        projected.0.rel_to,
    );
}

// ---------------------------------------------------------------------------
// Operator laws — idempotence + monotonicity (sanity) on the hot path
// ---------------------------------------------------------------------------

/// Pipeline idempotence on the ORCON-derived closed state:
/// `project(project(m).into()) == project(m)`. The closure operator
/// is monotone-extensive-idempotent per `marque-applied.md` §4.7.3;
/// the production hot path inherits the property.
///
/// Note that the second projection's input is NOT the original
/// ORCON-only portion — it's the post-pass-1 closed state, which
/// already has NOFORN injected by closure. The test verifies that
/// running the whole `join → closure → page_rewrites` pipeline on
/// that closed state is a fixed point (no further facts added, no
/// facts removed). The "_on_orcon_derived_closed_state" suffix names
/// what the assertion actually shows; the original
/// "_on_classified_orcon" name suggested the second pass also ran on
/// ORCON-only state, which it does not.
#[test]
fn project_pipeline_is_idempotent_on_orcon_derived_closed_state() {
    let portion = classified_with_dissem(Classification::Secret, DissemControl::Oc);
    let pass1 = project_page(&[portion]);
    let pass2 = project_page(std::slice::from_ref(&pass1.0));
    assert_eq!(
        pass1.0, pass2.0,
        "scheme.project must be idempotent on the closed state; \
         pass1 = {:?}, pass2 = {:?}",
        pass1.0, pass2.0,
    );
}

/// Idempotence on bare NATO at the closure layer: re-running closure
/// on the projection result does not add facts. This tests the
/// closure operator's idempotence property directly without paying
/// the second `RelToBlock::from_attrs_iter` tetragraph-expansion pass
/// (which is part of the join, not the closure — re-projecting expands
/// the `NATO` tetragraph closure injected into its constituent
/// trigraphs because that's the join-time semantic of REL TO
/// tetragraphs; the closure operator alone is idempotent).
#[test]
fn closure_is_idempotent_on_bare_nato_at_closure_layer() {
    let scheme = CapcoScheme::new();
    let mut portion = CanonicalAttrs::default();
    portion.classification = Some(MarkingClassification::Nato(NatoClassification::NatoSecret));
    let m = CapcoMarking::new(portion);
    // Apply join then closure manually (matches scheme.project's
    // inner pipeline up to PageRewrites).
    let joined = CapcoMarking::new(CapcoMarking::join_via_lattice(std::slice::from_ref(&m.0)));
    let closed = scheme.closure(joined);
    let closed_twice = scheme.closure(closed.clone());
    assert_eq!(
        closed.0, closed_twice.0,
        "closure operator must be idempotent on bare NATO; \
         closed = {:?}, closed_twice = {:?}",
        closed.0, closed_twice.0,
    );
}

/// Monotonicity sanity: a portion with strictly more facts produces
/// a projected marking with at least the same facts. We compare two
/// single-portion pages where the second page's portion has every
/// fact of the first plus one more.
#[test]
fn project_is_monotone_on_extending_facts() {
    let small = classified_with_dissem(Classification::Secret, DissemControl::Oc);
    let mut big = classified_us(Classification::Secret);
    big.dissem_us = vec![DissemControl::Oc, DissemControl::Rs].into_boxed_slice();
    let small_proj = project_page(&[small]);
    let big_proj = project_page(&[big]);
    // The big projection's dissem_us must contain every dissem from
    // the small projection (closure is monotone in the marking).
    for token in small_proj.0.dissem_us.iter() {
        assert!(
            big_proj.0.dissem_us.contains(token),
            "monotonicity violation: small_proj produced {:?} but big_proj \
             dropped it; big_proj.dissem_us = {:?}",
            token,
            big_proj.0.dissem_us,
        );
    }
}

// ---------------------------------------------------------------------------
// NOFORN-dominates-after-closure regression (§H.8 p145 + §B.3 Table 2 p21)
// ---------------------------------------------------------------------------

/// Direct test of the `apply_fact_add` NOFORN-supersession routing
/// landed in PR 4b-D.2 (decisions.md D22). A synthetic PageRewrite
/// emits `FactAdd { Cve(TOK_NOFORN), Scope::Page }` on a marking
/// that carries DISPLAY ONLY; the post-fix path MUST strip
/// DISPLAY ONLY via the §H.8 p145 supersession overlay at the
/// injection site.
///
/// Pre-fix the apply_fact_add path appended `Nf` to `dissem_us`
/// without re-applying overlays, leaving the marking with
/// `{Nf, Displayonly}` — invalid per §H.8 p145. Post-fix the
/// `DissemSet::with_noforn_injected` routing strips dominated
/// controls automatically.
///
/// Authority: §H.8 p145 (NOFORN: "Cannot be used with REL TO,
/// RELIDO, EYES ONLY, or DISPLAY ONLY") + §D.2 Table 3 rows 1-2.
/// Trigger predicate for the supersession test: fires when DISPLAY
/// ONLY is observed in the marking's dissem axis. Cannot use
/// `CategoryPredicate::Contains` here because
/// `capco_category_contains` does not currently dispatch on
/// `(CAT_DISSEM, TOK_DISPLAY_ONLY)` — the existing dispatch arms
/// cover NOFORN, NODIS, EXDIS, SBU-NF, LES-NF.
fn displayonly_present(m: &CapcoMarking) -> bool {
    m.0.dissem_us
        .iter()
        .any(|d| matches!(d, DissemControl::Displayonly))
}

#[test]
fn apply_fact_add_noforn_strips_displayonly_via_supersession() {
    use marque_capco::scheme::{CAT_DISSEM, TOK_NOFORN};
    use marque_scheme::{
        CategoryAction, CategoryPredicate, FactRef, PageRewrite, ReplacementIntent,
    };

    // Synthetic rewrite: when DISPLAY ONLY is present, inject NOFORN.
    // The post-PR-4b-D.2 apply_fact_add NOFORN branch routes through
    // DissemSet::with_noforn_injected, which applies the §H.8 p145
    // overlay and strips DISPLAY ONLY at the injection site.
    let rewrite = PageRewrite {
        id: "test/displayonly-triggers-noforn-overlay",
        citation: "§H.8 p145 + PR 4b-D.2 D22 test fixture",
        trigger: CategoryPredicate::Custom(displayonly_present),
        action: CategoryAction::Intent(ReplacementIntent::FactAdd {
            token: FactRef::Cve(TOK_NOFORN),
            scope: Scope::Page,
        }),
        reads: &[CAT_DISSEM],
        writes: &[CAT_DISSEM],
    };
    let scheme = CapcoScheme::with_rewrites(vec![rewrite]);

    let usa = CountryCode::USA;
    let gbr = CountryCode::try_new(b"GBR").expect("trigraph");
    let mut portion = classified_us(Classification::Secret);
    portion.dissem_us = vec![DissemControl::Displayonly].into_boxed_slice();
    portion.display_only_to = vec![usa, gbr].into_boxed_slice();

    let projected = scheme.project(Scope::Page, &[CapcoMarking::new(portion)]);

    assert!(
        projected.0.dissem_us.iter().any(|d| d == &DissemControl::Nf),
        "FactAdd routing must inject NOFORN; dissem_us = {:?}",
        projected.0.dissem_us,
    );
    assert!(
        !projected
            .0
            .dissem_us
            .iter()
            .any(|d| d == &DissemControl::Displayonly),
        "§H.8 p145: NOFORN dominates DISPLAY ONLY — the supersession \
         overlay at injection time must strip Displayonly; \
         dissem_us = {:?}",
        projected.0.dissem_us,
    );
}

/// Idempotence of the NOFORN-injection supersession: re-inserting
/// NOFORN into a marking that already has NOFORN must produce the
/// same marking (no-op via `IntentInapplicable`).
#[test]
fn apply_fact_add_noforn_is_idempotent() {
    use marque_capco::scheme::{CAT_DISSEM, TOK_NOFORN};
    use marque_scheme::{
        CategoryAction, CategoryPredicate, FactRef, PageRewrite, ReplacementIntent,
    };

    let rewrite = PageRewrite {
        id: "test/noforn-already-present",
        citation: "§H.8 p145 idempotence",
        trigger: CategoryPredicate::Contains {
            category: CAT_DISSEM,
            token: TOK_NOFORN,
        },
        action: CategoryAction::Intent(ReplacementIntent::FactAdd {
            token: FactRef::Cve(TOK_NOFORN),
            scope: Scope::Page,
        }),
        reads: &[CAT_DISSEM],
        writes: &[CAT_DISSEM],
    };
    let scheme = CapcoScheme::with_rewrites(vec![rewrite]);

    let portion = classified_with_dissem(Classification::Secret, DissemControl::Nf);
    let projected = scheme.project(Scope::Page, &[CapcoMarking::new(portion)]);

    // Re-injection is a no-op: NOFORN stays present exactly once.
    let nf_count = projected
        .0
        .dissem_us
        .iter()
        .filter(|d| **d == DissemControl::Nf)
        .count();
    assert_eq!(
        nf_count, 1,
        "FactAdd of NOFORN onto a marking that already has NOFORN \
         must be idempotent; dissem_us = {:?}",
        projected.0.dissem_us,
    );
}

// ---------------------------------------------------------------------------
// Compile-time anchor — keep the helpers attached to the public surface.
// If `SciControl` / `SciControlSystem` / `SciMarking` move under
// `marque-ism`'s reorganization, the import block at the top breaks
// before the test failure — fail-fast for a downstream rename.
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn _compile_time_anchor() -> SciMarking {
    SciMarking::new(
        SciControlSystem::Published(marque_ism::SciControlBare::Hcs),
        Box::new([]),
        Some(SciControl::Hcs),
    )
}
