// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Sub-PR 8.F — Pattern A NOFORN-supremacy (NODIS + EXDIS)
//! behavioral tests.
//!
//! These tests drive `scheme.project(Scope::Page, &[portion_attrs])`
//! directly — NOT `Engine::lint` — because the engine's runtime
//! banner-validation path does not currently iterate scheduled rewrites
//! (see design spec §5 "Runtime execution gap" and §8 "Test plan
//! implication"). `Engine::new` validates the rewrites' intent payloads
//! and topological ordering at construction time; the behavioral effect
//! is visible through `scheme.project(Scope::Page, ...)`.
//!
//! # Citation anchor
//!
//! - NODIS "Requires NOFORN." — CAPCO-2016 §H.9 p174, line 4296
//! - EXDIS "Requires NOFORN." — CAPCO-2016 §H.9 p172, line 4236
//!
//! Both citations pre-verified by the spec architect against the
//! vendored source at `crates/capco/docs/CAPCO-2016.md`.
//!
//! # Runtime execution gap (preserved as TODO)
//!
//! When the Phase D/E engine wiring lands and `Engine::lint` /
//! `Engine::fix` routes banner-validation through `scheme.project`,
//! test #9 (`pattern_a_rewrites_emit_no_applied_fix`) must flip its
//! assertion: instead of asserting no `AppliedFix` carries the new
//! rewrite IDs, it must assert `applied.proposal.original == ""`
//! (G13 content-ignorance). See the TODO comment on that test.
//!
//! # Test inventory (design spec §8)
//!
//! 1. `nodis_portion_projects_noforn_to_page_dissem`
//! 2. `exdis_portion_projects_noforn_to_page_dissem`
//! 3. `nodis_portion_composes_with_noforn_clears_rel_to`
//! 4. `exdis_portion_composes_with_noforn_clears_rel_to`
//! 5. `portion_without_nodis_or_exdis_does_not_inject_noforn`
//! 6. `nodis_portion_with_noforn_already_present_is_idempotent`
//! 7. `unclassified_nodis_and_exdis_portions_still_inject_noforn`
//! 8. `portion_with_both_nodis_and_exdis_is_safe`
//! 9. `pattern_a_rewrites_emit_no_applied_fix`

use marque_capco::{CapcoMarking, CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};
use marque_ism::{
    CanonicalAttrs, Classification, CountryCode, DissemControl, MarkingClassification, NonIcDissem,
};
use marque_scheme::{MarkingScheme, Scope};

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

/// Build a minimal `CanonicalAttrs` with only a US classification set.
fn portion_classified(c: Classification) -> CapcoMarking {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(c));
    CapcoMarking::new(a)
}

/// Build a `CapcoMarking` with a US classification and non-IC dissem controls.
fn portion_with_non_ic(c: Classification, non_ic: &[NonIcDissem]) -> CapcoMarking {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(c));
    a.non_ic_dissem = non_ic.to_vec().into_boxed_slice();
    CapcoMarking::new(a)
}

/// Build a `CapcoMarking` with a classification, non-IC dissem, and
/// IC dissem controls (for testing NOFORN already-present cases).
fn portion_with_non_ic_and_dissem(
    c: Classification,
    non_ic: &[NonIcDissem],
    dissem: &[DissemControl],
) -> CapcoMarking {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(c));
    a.non_ic_dissem = non_ic.to_vec().into_boxed_slice();
    a.dissem_controls = dissem.to_vec().into_boxed_slice();
    CapcoMarking::new(a)
}

/// Build a `CapcoMarking` with a classification and REL TO country codes.
fn portion_with_rel_to(c: Classification, countries: &[CountryCode]) -> CapcoMarking {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(c));
    a.rel_to = countries.to_vec().into_boxed_slice();
    CapcoMarking::new(a)
}

/// `GBR` country code for use in test fixtures.
/// Constructed at runtime (only `CountryCode::USA` is a pre-built const).
fn gbr() -> CountryCode {
    CountryCode::try_new(b"GBR")
        .expect("GBR is a valid 3-char CAPCO trigraph per CVEnumISMCATRelTo.xsd")
}

/// Standard engine fixture for the G13 test (test #9).
fn engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(std::time::UNIX_EPOCH)),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

// ---------------------------------------------------------------------------
// Test 1 — `nodis_portion_projects_noforn_to_page_dissem`
// ---------------------------------------------------------------------------

/// `(S//ND)` portion: `scheme.project(Scope::Page, ...)` must produce
/// a page-level marking whose `dissem_controls` contains NOFORN.
///
/// Exercises the `capco/nodis-implies-noforn` PageRewrite:
/// `Contains(CAT_NON_IC_DISSEM, TOK_NODIS)` fires → `FactAdd(NOFORN,
/// Scope::Page)` adds NOFORN to the page dissem axis.
///
/// Authority: CAPCO-2016 §H.9 p174 "Requires NOFORN."
#[test]
fn nodis_portion_projects_noforn_to_page_dissem() {
    let scheme = CapcoScheme::new();
    let portion = portion_with_non_ic(Classification::Secret, &[NonIcDissem::Nodis]);

    let projected = scheme.project(Scope::Page, &[portion]);

    assert!(
        projected.0.dissem_controls.contains(&DissemControl::Nf),
        "capco/nodis-implies-noforn rewrite must add NOFORN to page dissem \
         when a portion contains NODIS (CAPCO-2016 §H.9 p174 'Requires NOFORN.'); \
         got dissem_controls = {:?}",
        projected.0.dissem_controls,
    );
}

// ---------------------------------------------------------------------------
// Test 2 — `exdis_portion_projects_noforn_to_page_dissem`
// ---------------------------------------------------------------------------

/// `(S//XD)` portion: `scheme.project(Scope::Page, ...)` must produce
/// a page-level marking whose `dissem_controls` contains NOFORN.
///
/// Exercises the `capco/exdis-implies-noforn` PageRewrite:
/// `Contains(CAT_NON_IC_DISSEM, TOK_EXDIS)` fires → `FactAdd(NOFORN,
/// Scope::Page)` adds NOFORN to the page dissem axis.
///
/// Authority: CAPCO-2016 §H.9 p172 "Requires NOFORN."
#[test]
fn exdis_portion_projects_noforn_to_page_dissem() {
    let scheme = CapcoScheme::new();
    let portion = portion_with_non_ic(Classification::Secret, &[NonIcDissem::Exdis]);

    let projected = scheme.project(Scope::Page, &[portion]);

    assert!(
        projected.0.dissem_controls.contains(&DissemControl::Nf),
        "capco/exdis-implies-noforn rewrite must add NOFORN to page dissem \
         when a portion contains EXDIS (CAPCO-2016 §H.9 p172 'Requires NOFORN.'); \
         got dissem_controls = {:?}",
        projected.0.dissem_controls,
    );
}

// ---------------------------------------------------------------------------
// Test 3 — `nodis_portion_composes_with_noforn_clears_rel_to`
// ---------------------------------------------------------------------------

/// `(S//ND)` portion paired with a synthetic prior REL TO: the scheduler
/// runs `capco/nodis-implies-noforn` BEFORE `capco/noforn-clears-rel-to`.
/// The projected page must have NOFORN in `dissem_controls` AND an empty
/// `rel_to`.
///
/// This is the load-bearing composition test — it verifies the scheduler's
/// topological ordering places the two DISSEM-writing rewrites (nodis-/
/// exdis-implies-noforn) before the DISSEM-reading `noforn-clears-rel-to`.
///
/// The "prior REL TO" is injected by including a second portion that
/// carries REL TO but no NODIS/EXDIS — simulating a document where one
/// portion is `(S//REL TO USA, GBR)` and another is `(S//ND)`.
///
/// Expected result: NOFORN present, REL TO cleared (the `noforn-clears-
/// rel-to` rewrite fired AFTER the `nodis-implies-noforn` rewrite added
/// NOFORN to the projected page state).
///
/// Authority: CAPCO-2016 §H.9 p174 + §D.2 Table 3 / §H.8 p145.
#[test]
fn nodis_portion_composes_with_noforn_clears_rel_to() {
    let scheme = CapcoScheme::new();

    // Portion A: carries NODIS, no REL TO.
    let nodis_portion = portion_with_non_ic(Classification::Secret, &[NonIcDissem::Nodis]);

    // Portion B: carries REL TO (USA, GBR), no NODIS/EXDIS. This
    // populates the page's REL TO axis before the rewrites fire.
    let rel_to_portion = portion_with_rel_to(
        Classification::Secret,
        &[CountryCode::USA, gbr()],
    );

    let projected = scheme.project(Scope::Page, &[nodis_portion, rel_to_portion]);

    assert!(
        projected.0.dissem_controls.contains(&DissemControl::Nf),
        "NOFORN must be present in page dissem after nodis-implies-noforn fires; \
         got dissem_controls = {:?}",
        projected.0.dissem_controls,
    );

    assert!(
        projected.0.rel_to.is_empty(),
        "REL TO must be cleared after noforn-clears-rel-to fires downstream \
         of nodis-implies-noforn (scheduler ordering guarantees CAT_DISSEM \
         writers precede CAT_DISSEM readers); got rel_to = {:?}",
        projected.0.rel_to,
    );
}

// ---------------------------------------------------------------------------
// Test 4 — `exdis_portion_composes_with_noforn_clears_rel_to`
// ---------------------------------------------------------------------------

/// Mirror of test 3 for EXDIS. `(S//XD)` portion paired with a synthetic
/// prior REL TO: the scheduler orders `exdis-implies-noforn` before
/// `noforn-clears-rel-to`, so the projected page has NOFORN and empty
/// REL TO.
///
/// Authority: CAPCO-2016 §H.9 p172 + §D.2 Table 3 / §H.8 p145.
#[test]
fn exdis_portion_composes_with_noforn_clears_rel_to() {
    let scheme = CapcoScheme::new();

    let exdis_portion = portion_with_non_ic(Classification::Secret, &[NonIcDissem::Exdis]);
    let rel_to_portion = portion_with_rel_to(
        Classification::Secret,
        &[CountryCode::USA, gbr()],
    );

    let projected = scheme.project(Scope::Page, &[exdis_portion, rel_to_portion]);

    assert!(
        projected.0.dissem_controls.contains(&DissemControl::Nf),
        "NOFORN must be present in page dissem after exdis-implies-noforn fires; \
         got dissem_controls = {:?}",
        projected.0.dissem_controls,
    );

    assert!(
        projected.0.rel_to.is_empty(),
        "REL TO must be cleared after noforn-clears-rel-to fires downstream \
         of exdis-implies-noforn; got rel_to = {:?}",
        projected.0.rel_to,
    );
}

// ---------------------------------------------------------------------------
// Test 5 — `portion_without_nodis_or_exdis_does_not_inject_noforn`
// ---------------------------------------------------------------------------

/// Negative test: a plain `(S)` portion must NOT receive NOFORN in the
/// projected page marking. The new rewrites are triggered only by
/// `Contains(CAT_NON_IC_DISSEM, TOK_NODIS)` and
/// `Contains(CAT_NON_IC_DISSEM, TOK_EXDIS)`; a portion with neither
/// token must not fire either rewrite.
///
/// Catches an over-eager predicate in `capco_category_contains`.
#[test]
fn portion_without_nodis_or_exdis_does_not_inject_noforn() {
    let scheme = CapcoScheme::new();

    // Plain classified portion: no dissem controls, no non-IC dissem.
    let plain = portion_classified(Classification::Secret);

    let projected = scheme.project(Scope::Page, &[plain]);

    assert!(
        !projected.0.dissem_controls.contains(&DissemControl::Nf),
        "A plain (S) portion must not have NOFORN injected by the \
         NODIS/EXDIS-implies-noforn rewrites; got dissem_controls = {:?}",
        projected.0.dissem_controls,
    );
}

// ---------------------------------------------------------------------------
// Test 6 — `nodis_portion_with_noforn_already_present_is_idempotent`
// ---------------------------------------------------------------------------

/// `(S//NF//ND)` portion: NOFORN already present. The
/// `capco/nodis-implies-noforn` rewrite fires but `apply_fact_add` returns
/// `IntentInapplicable` (silent per-intent no-op per the idempotence
/// policy at `crates/capco/src/scheme.rs:624-639`). The projected marking
/// must contain NOFORN exactly once — no panic, no double-add.
///
/// Also verifies EXDIS is handled analogously via a `(S//NF//XD)` input.
#[test]
fn nodis_portion_with_noforn_already_present_is_idempotent() {
    let scheme = CapcoScheme::new();

    // `(S//NF//ND)` — NOFORN already in dissem_controls; NODIS in non_ic_dissem.
    let portion = portion_with_non_ic_and_dissem(
        Classification::Secret,
        &[NonIcDissem::Nodis],
        &[DissemControl::Nf],
    );

    let projected = scheme.project(Scope::Page, &[portion]);

    let noforn_count = projected
        .0
        .dissem_controls
        .iter()
        .filter(|d| matches!(d, DissemControl::Nf))
        .count();

    assert_eq!(
        noforn_count, 1,
        "NOFORN must appear exactly once after nodis-implies-noforn fires on \
         a portion that already has NOFORN — the FactAdd idempotence path \
         (`apply_fact_add → IntentInapplicable` silent no-op) must not \
         double-add; got dissem_controls = {:?}",
        projected.0.dissem_controls,
    );

    // Mirror: EXDIS with NOFORN already present.
    let exdis_portion = portion_with_non_ic_and_dissem(
        Classification::Secret,
        &[NonIcDissem::Exdis],
        &[DissemControl::Nf],
    );

    let exdis_projected = scheme.project(Scope::Page, &[exdis_portion]);

    let exdis_noforn_count = exdis_projected
        .0
        .dissem_controls
        .iter()
        .filter(|d| matches!(d, DissemControl::Nf))
        .count();

    assert_eq!(
        exdis_noforn_count, 1,
        "NOFORN must appear exactly once after exdis-implies-noforn fires on \
         a portion that already has NOFORN — same idempotence invariant; \
         got dissem_controls = {:?}",
        exdis_projected.0.dissem_controls,
    );
}

// ---------------------------------------------------------------------------
// Test 7 — `unclassified_nodis_and_exdis_portions_still_inject_noforn`
// ---------------------------------------------------------------------------

/// `(U//ND)` and `(U//XD)` portions: CAPCO-2016 §H.9 p174 (NODIS) and
/// p172 (EXDIS) both say "May be used with TOP SECRET, SECRET, CONFIDENTIAL,
/// or UNCLASSIFIED." The `*-implies-noforn` rewrites must fire regardless
/// of classification level — the `Contains(CAT_NON_IC_DISSEM, TOK_NODIS)`
/// trigger predicate is classification-agnostic.
///
/// Authority: CAPCO-2016 §H.9 p174 / p172 Relationship(s) stanza.
#[test]
fn unclassified_nodis_and_exdis_portions_still_inject_noforn() {
    let scheme = CapcoScheme::new();

    // `(U//ND)` — unclassified NODIS portion.
    let nodis_u = portion_with_non_ic(Classification::Unclassified, &[NonIcDissem::Nodis]);
    let projected_nodis = scheme.project(Scope::Page, &[nodis_u]);

    assert!(
        projected_nodis.0.dissem_controls.contains(&DissemControl::Nf),
        "capco/nodis-implies-noforn must fire on UNCLASSIFIED NODIS portions \
         (CAPCO-2016 §H.9 p174: 'May be used with ... UNCLASSIFIED. Requires \
         NOFORN.'); got dissem_controls = {:?}",
        projected_nodis.0.dissem_controls,
    );

    // `(U//XD)` — unclassified EXDIS portion.
    let exdis_u = portion_with_non_ic(Classification::Unclassified, &[NonIcDissem::Exdis]);
    let projected_exdis = scheme.project(Scope::Page, &[exdis_u]);

    assert!(
        projected_exdis.0.dissem_controls.contains(&DissemControl::Nf),
        "capco/exdis-implies-noforn must fire on UNCLASSIFIED EXDIS portions \
         (CAPCO-2016 §H.9 p172: 'May be used with ... UNCLASSIFIED. Requires \
         NOFORN.'); got dissem_controls = {:?}",
        projected_exdis.0.dissem_controls,
    );
}

// ---------------------------------------------------------------------------
// Test 8 — `portion_with_both_nodis_and_exdis_is_safe`
// ---------------------------------------------------------------------------

/// `(S//ND//XD)` portion: semantically forbidden by E037 (NODIS/EXDIS mutex
/// per CAPCO-2016 §H.9 p172/p174 "EXDIS and NODIS markings cannot be used
/// together"), but the PageRewrite path must remain safe under accidental
/// concurrent firing. Both `nodis-implies-noforn` and `exdis-implies-noforn`
/// rewrites trigger; both attempt `FactAdd(NOFORN, Page)`. The second
/// attempt hits `apply_fact_add → IntentInapplicable` (NOFORN already
/// present, idempotent no-op). Result: exactly one NOFORN in projected
/// dissem, no panic.
///
/// Master pattern doc (`project_noforn_supremacy_composition.md:17`):
/// "NODIS and EXDIS markings cannot be used together."
#[test]
fn portion_with_both_nodis_and_exdis_is_safe() {
    let scheme = CapcoScheme::new();

    // `(S//ND//XD)` — both NODIS and EXDIS in non_ic_dissem (forbidden
    // by E037 but reachable in malformed input; the PageRewrite path
    // must be safe regardless).
    let portion = portion_with_non_ic(
        Classification::Secret,
        &[NonIcDissem::Nodis, NonIcDissem::Exdis],
    );

    let projected = scheme.project(Scope::Page, &[portion]);

    let noforn_count = projected
        .0
        .dissem_controls
        .iter()
        .filter(|d| matches!(d, DissemControl::Nf))
        .count();

    assert_eq!(
        noforn_count, 1,
        "Both NODIS and EXDIS rewrites fire, but NOFORN must appear exactly \
         once — the second FactAdd hits the idempotence path \
         (IntentInapplicable silent no-op); no panic expected; \
         got dissem_controls = {:?}",
        projected.0.dissem_controls,
    );
}

// ---------------------------------------------------------------------------
// Test 9 — `pattern_a_rewrites_emit_no_applied_fix`
// ---------------------------------------------------------------------------

/// G13 content-ignorance gate for the new rewrites.
///
/// The two Pattern A rewrites (`capco/nodis-implies-noforn`,
/// `capco/exdis-implies-noforn`) are **scheduler-validated but
/// execution-deferred**: `Engine::new` validates their intent payloads
/// and topological ordering at construction time, but `Engine::lint` /
/// `Engine::fix` does not currently iterate scheduled rewrites
/// (the engine's runtime banner-validation path drives through
/// `marque_ism::PageContext` directly — see design spec §5).
///
/// Therefore, `Engine::fix` on `(S//ND)\n` must produce NO `AppliedFix`
/// records whose `proposal.rule` matches the new rewrite IDs. The rewrites
/// exist in the scheme's declarative table; they produce no engine-promoted
/// fixes until the Phase D/E execution loop lands.
///
/// # TODO — Phase D/E flip
///
/// When the engine's banner-validation path switches to `scheme.project`-
/// driven semantics (Phase D or Phase E), this test MUST flip its assertion.
/// At that point, the new rewrites will materialize as `AppliedFix` records
/// on the audit stream, and the assertion should become:
///
/// ```rust
/// // Phase D/E flip: rewrites now produce AppliedFix records.
/// // G13 invariant: content must not appear in audit output.
/// for applied in &result.applied {
///     if applied.proposal.rule.as_str() == "capco/nodis-implies-noforn"
///         || applied.proposal.rule.as_str() == "capco/exdis-implies-noforn"
///     {
///         assert!(
///             applied.proposal.original.is_empty(),
///             "G13: page-rewrite AppliedFix must carry empty `original` \
///              (no document content in audit record); got: {:?}",
///             applied.proposal.original,
///         );
///     }
/// }
/// ```
///
/// See PR 3c.B Sub-PR 8.F design spec §8 test #9 for the complete
/// Phase D/E flip description.
#[test]
fn pattern_a_rewrites_emit_no_applied_fix() {
    // Verify `Engine::new` accepts the new rewrites (no RewriteCycle /
    // InvalidIntentInPageRewrite errors).
    let e = engine();

    // Run `Engine::fix` on a corpus that would trigger both rewrites
    // if they were execution-active. The execution-deferred posture means
    // they produce no AppliedFix records today.
    let result = e.fix(b"(S//ND)\n(S//XD)\n", FixMode::Apply);

    // Collect all applied rule IDs for the assertion message.
    let applied_ids: Vec<&str> = result
        .applied
        .iter()
        .map(|af| af.proposal.rule.as_str())
        .collect();

    // Assert: neither new rewrite ID appears in the audit stream.
    // These rewrites are scheduler-validated but execution-deferred —
    // they live in the scheme's declarative table and their intent
    // payloads are validated at `Engine::new` time, but `Engine::fix`
    // does not currently drive banner-validation through `scheme.project`.
    assert!(
        !applied_ids.contains(&"capco/nodis-implies-noforn"),
        "capco/nodis-implies-noforn must not appear as an AppliedFix rule \
         under the current execution-deferred posture (design spec §5); \
         applied rules: {applied_ids:?}",
    );

    assert!(
        !applied_ids.contains(&"capco/exdis-implies-noforn"),
        "capco/exdis-implies-noforn must not appear as an AppliedFix rule \
         under the current execution-deferred posture (design spec §5); \
         applied rules: {applied_ids:?}",
    );

    // TODO (Phase D/E): when the engine's banner-validation path switches
    // to `scheme.project`-driven semantics, replace the two `assert!`
    // blocks above with a G13 content-ignorance walk over the new rewrite
    // records (see the doc comment on this test for the replacement code).
    // Reference: PR 3c.B Sub-PR 8.F design spec §8 test #9.
}
