// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 4b-D.2 closure-on-the-hot-path integration tests.
//!
//! Exercises [`CapcoScheme::project(Scope::Page, ...)`] post-flip
//! (PR 4b-D.2 Commit 3). The closure operator now runs between the
//! per-axis lattice join and the declarative `PageRewrite` catalog;
//! these tests pin that the unified `CLOSURE_NOFORN_CAVEATED` Trio 1
//! row (post-PR-#522 collapse of the seven historical `CLOSURE_NOFORN_*`
//! rows) and `CLOSURE_REL_TO_USA_NATO` fire through the production
//! page projection, that the operator is idempotent and monotone in
//! the marking, and that NOFORN-injection at the closure layer
//! correctly composes with the `DissemSet` supersession overlay
//! (§H.8 p145 NOFORN-dominates).
//!
//! Authority: `docs/plans/2026-05-01-lattice-design.md` §3 (e) +
//! §4.7.4 pipeline ordering. Per-trigger §-citations on the
//! `CLOSURE_NOFORN_CAVEATED` row doc-comment at
//! `crates/capco/src/scheme/closure.rs`.

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
// Trio 1 — implicit NOFORN closure rule (representative fixtures per
// trigger family)
//
// Per `crates/capco/src/scheme/closure.rs::CAPCO_CLOSURE_RULES`. The
// single `CLOSURE_NOFORN_CAVEATED` row fires on
// `scheme.project(Scope::Page, ...)` when any of its 18 triggers is
// observed and no FD&R dominator is present. The fixtures below
// exercise one representative arm per trigger family (SAR / AEA / UCNI
// / FGI / ORCON / RSEN-IMCON-DSEN / non-IC-controls); the per-arm
// algebraic-firing parity is pinned by `closure_runtime.rs` in
// `marque-capco` (which exercises every individual `TokenRef` in the
// trigger list).
// ---------------------------------------------------------------------------

/// SAR arm of `CLOSURE_NOFORN_CAVEATED` (§H.5 p101 + §B.3 Table 2 p21):
/// any SAR program triggers implicit NOFORN through the page projection.
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

/// AEA arm of `CLOSURE_NOFORN_CAVEATED` (§H.6 p104 + §B.3 Table 2 p21):
/// RD / FRD / TFNI trigger implicit NOFORN.
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

/// UCNI arm of `CLOSURE_NOFORN_CAVEATED` (§H.6 p118 + §B.3 Table 2 p21):
/// DOE UCNI triggers implicit NOFORN through the page projection.
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

/// FGI arm of `CLOSURE_NOFORN_CAVEATED` (§H.7 p122 + §B.3 Table 2 p21):
/// FGI marker triggers implicit NOFORN.
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

/// ORCON arm of `CLOSURE_NOFORN_CAVEATED` (§H.8 p136 + §B.3 Table 2 p21):
/// ORCON triggers implicit NOFORN. The post-closure supersession overlay
/// does not strip ORCON itself (ORCON and NOFORN coexist per §H.8 p145
/// — only REL TO / RELIDO / EYES ONLY / DISPLAY ONLY are dominated).
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

/// RSEN / IMCON / DSEN arms of `CLOSURE_NOFORN_CAVEATED` (§H.8 p132
/// and §B.3 Table 2 p21): RSEN / IMCON / DSEN trigger implicit NOFORN.
/// Test with RSEN — the same row covers IMCON and DSEN.
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

/// Non-IC-controls arm of `CLOSURE_NOFORN_CAVEATED` (§H.9 p170 +
/// §B.3 Table 2 p21): LIMDIS / LES / SBU / SSI / NNPI trigger implicit
/// NOFORN. Test with LIMDIS — the same row covers the others.
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
        projected
            .0
            .dissem_us
            .iter()
            .any(|d| d == &DissemControl::Nf),
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
// PR 4b-D.2 Copilot R1 #2 + R2 #2 — §H.8 p145 banner invariant on
// the `display_only_to` country axis
// ---------------------------------------------------------------------------
//
// Two layers maintain the §H.8 p145 invariant ("NOFORN ... Cannot be
// used with DISPLAY ONLY") on the `attrs.display_only_to` country
// axis:
//
//   1. `apply_fact_add` (post-PR-4b-D.2 Copilot R2 #1): every direct
//      FactAdd of NOFORN clears `display_only_to` at the injection
//      site. Covers E021 / E038 / closure-driven injection paths.
//      Pinned by tests in `crates/capco/tests/category_action_intent.rs`.
//   2. `capco/noforn-clears-display-only-to` PageRewrite (PR 4b-D.2
//      Copilot R1 #2): a defense-in-depth `Clear { CAT_DISPLAY_ONLY_TO }`
//      action that fires whenever NOFORN ends up in `dissem_us` at
//      the projection's PageRewrite phase. Pinned by the integration
//      test below.
//
// Copilot R2 #2 surfaced that the prior `noforn_clears_display_only_to_via_cross_portion_join`
// test didn't exercise layer #2: `expected_display_only` in PageContext
// short-circuits to empty whenever ANY portion has NOFORN
// (`crates/ism/src/page_context.rs:881-896`). When portion 1 was
// NOFORN-bearing, `out.display_only_to` was empty at join time —
// before the rewrite ran. The test passed whether or not the rewrite
// existed.
//
// Post-Copilot-R2 the test pivots to a UCNI scenario where:
//   - Both portions have non-empty `display_only_to` (passes the row-19
//     all-or-nothing gate in `expected_display_only`).
//   - Neither portion has NOFORN at join time (passes the `any_noforn`
//     short-circuit at line 894).
//   - The Pattern-C `capco/dod-ucni-promotes-noforn-when-classified`
//     rewrite injects NOFORN AFTER `display_only_to` has been
//     populated.
//
// With Item 1 (apply_fact_add clears country axes) in place, the
// Pattern-C rewrite's FactAdd ALREADY clears `display_only_to` —
// making the noforn-clears-display-only-to rewrite functionally
// idempotent on this path. The integration test verifies BOTH layers
// converge to the same correct output: post-`scheme.project`, NOFORN
// is present and `display_only_to` is empty.
//
// The rewrite is retained as defense-in-depth — a future refactor
// that bypasses `apply_fact_add` or changes its clearing semantics
// will be caught by the PageRewrite layer.
//
// Authority: CAPCO-2016 §H.8 p145 ("NOFORN ... Cannot be used with
// REL TO / RELIDO / EYES ONLY / DISPLAY ONLY") + §D.2 Table 3 rows
// 1-2 (NOFORN dominates the FD&R family) + §H.6 p116 (DOD UCNI
// strip-and-promote on classified). All re-verified 2026-05-18
// against `crates/capco/docs/CAPCO-2016.md`.

/// Cross-axis integration: a classified page where the Pattern-C
/// `capco/dod-ucni-promotes-noforn-when-classified` rewrite injects
/// NOFORN AFTER `display_only_to` has been populated by per-portion
/// union. Verifies the §H.8 p145 invariant on the country axis holds
/// through the full `scheme.project` pipeline: NOFORN present and
/// `display_only_to` empty post-projection.
#[test]
fn noforn_clears_display_only_to_via_ucni_promote() {
    use marque_ism::AeaMarking;

    let usa = CountryCode::USA;
    let gbr = CountryCode::try_new(b"GBR").expect("trigraph");

    // Portion 1: classified with DOD UCNI AND a DISPLAY ONLY list.
    // The DISPLAY ONLY here is required so portion 1 passes the
    // row-19 all-or-nothing gate in `expected_display_only`. Both
    // portions thus contribute display-permission, the gate doesn't
    // short-circuit, and `out.display_only_to` is populated at join
    // time. The Pattern-C `capco/dod-ucni-promotes-noforn-when-classified`
    // rewrite then injects NOFORN, which clears `display_only_to`
    // via the apply_fact_add Item-1 cleanup AND/OR the
    // noforn-clears-display-only-to rewrite (defense-in-depth).
    let mut ucni_portion = classified_us(Classification::Secret);
    ucni_portion.aea_markings = vec![AeaMarking::DodUcni].into_boxed_slice();
    ucni_portion.dissem_us = vec![DissemControl::Displayonly].into_boxed_slice();
    ucni_portion.display_only_to = vec![usa, gbr].into_boxed_slice();

    let mut do_portion = classified_us(Classification::Secret);
    do_portion.dissem_us = vec![DissemControl::Displayonly].into_boxed_slice();
    do_portion.display_only_to = vec![usa].into_boxed_slice();

    let projected = project_page(&[ucni_portion, do_portion]);

    // NOFORN was injected by the Pattern-C UCNI-promote rewrite.
    assert!(
        dissem_contains(&projected, DissemControl::Nf),
        "Pattern-C `capco/dod-ucni-promotes-noforn-when-classified` \
         must inject NOFORN on classified UCNI (§H.6 p116); \
         dissem_us = {:?}",
        projected.0.dissem_us,
    );

    // `attrs.display_only_to` was cleared post-NOFORN-injection.
    // Either the apply_fact_add Item-1 cleanup (during the
    // Pattern-C rewrite's FactAdd invocation) or the
    // `capco/noforn-clears-display-only-to` rewrite (running later
    // in the pipeline as defense-in-depth) — both paths produce the
    // same correct output. The assertion captures the §H.8 p145
    // banner invariant regardless of which layer cleared.
    assert!(
        projected.0.display_only_to.is_empty(),
        "§H.8 p145: NOFORN must clear `display_only_to` when present; \
         display_only_to = {:?}",
        projected.0.display_only_to,
    );

    // DISPLAY ONLY token was stripped from `dissem_us` by
    // `capco/noforn-clears-fdr-family`. Companion check — the token
    // axis and the country axis must BOTH be clear for §H.8 p145
    // to hold; either alone is incomplete.
    assert!(
        !dissem_contains(&projected, DissemControl::Displayonly),
        "§H.8 p145: NOFORN must strip Displayonly token from dissem_us; \
         dissem_us = {:?}",
        projected.0.dissem_us,
    );
}

/// Defense-in-depth gate: when NOFORN is already present at join
/// time but `display_only_to` is empty (the common case), the
/// rewrite is a no-op. Confirms the `Clear` action's idempotence on
/// empty input — equivalent to the `noforn-clears-rel-to`
/// idempotency precedent.
#[test]
fn noforn_clears_display_only_to_is_idempotent_on_empty_field() {
    // Portion with NOFORN but no DISPLAY ONLY country list.
    let portion = classified_with_dissem(Classification::Secret, DissemControl::Nf);
    let projected = project_page(&[portion]);
    assert!(dissem_contains(&projected, DissemControl::Nf));
    assert!(
        projected.0.display_only_to.is_empty(),
        "rewrite must be idempotent on empty input; \
         display_only_to = {:?}",
        projected.0.display_only_to,
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
