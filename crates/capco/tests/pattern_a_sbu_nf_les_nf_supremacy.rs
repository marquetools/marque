#![cfg(any())]
// PR 3c.B Commit 10: legacy FixProposal-shape test disabled pending rewrite

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Sub-PR 8.F.2 — Pattern A NOFORN-supremacy (SBU-NF + LES-NF)
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
//! Test #9 (`pattern_a_sbu_nf_les_nf_rewrites_emit_no_applied_fix`)
//! exercises the G13 content-ignorance invariant through `Engine::fix`
//! with E002 (missing USA trigraph) as the positive control.
//!
//! # Citation anchor
//!
//! - SBU-NF NOFORN implication — CAPCO-2016 §H.9 p178. The page does
//!   NOT contain a "Requires NOFORN." sentence; the implication is
//!   derived from (a) the banner-form heading at `:4388-4398` which
//!   names the marking "SENSITIVE BUT UNCLASSIFIED NOFORN", (b) the
//!   Commingling Rule at `:4420-4421` confirming NOFORN persists after
//!   transmutation strips the SBU half, and (c) §D.2 Table 3 rows 3-5
//!   at `:590-595` listing NOFORN as the FD&R banner consequence.
//! - LES-NF NOFORN implication — CAPCO-2016 §H.9 p185. Same structural
//!   pattern: (a) banner-form heading at `:4532-4542` naming the
//!   marking "LAW ENFORCEMENT SENSITIVE NOFORN", (b) Precedence Rule
//!   at `:4558` "When a classified document contains portions of
//!   U//LES- NF, the 'LES' marking is used in the banner line and the
//!   NOFORN marking is applied as a Dissemination Control Marking. For
//!   example: SECRET//NOFORN//LES." (note: source has whitespace OCR
//!   artifact "LES- NF" rendered with a space; canonical token is
//!   LES-NF), and (c) §D.2 Table 3 rows 6-8 at `:590-595`.
//!
//! Both citations pre-verified by the spec architect against the
//! vendored source at `crates/capco/docs/CAPCO-2016.md`.
//!
//! # Runtime execution gap (preserved as TODO)
//!
//! When the Phase D/E engine wiring lands and `Engine::lint` /
//! `Engine::fix` routes banner-validation through `scheme.project`,
//! test #9 must flip its assertion: instead of asserting no
//! `AppliedFix` carries the new rewrite IDs, it must assert
//! `applied.proposal.original == ""` (G13 content-ignorance). See the
//! TODO comment on that test.
//!
//! # Test inventory (design spec §8)
//!
//! 1. `sbu_nf_portion_projects_noforn_to_page_dissem_unclassified`
//! 2. `sbu_nf_malformed_classified_still_injects_noforn`
//! 3. `les_nf_portion_projects_noforn_to_page_dissem_unclassified`
//! 4. `les_nf_portion_projects_noforn_to_page_dissem_classified`
//! 5. `sbu_nf_portion_composes_with_noforn_clears_rel_to`
//! 6. `les_nf_portion_composes_with_noforn_clears_rel_to`
//! 7. `portion_without_sbu_nf_or_les_nf_does_not_inject_noforn`
//! 8. `sbu_nf_portion_with_noforn_already_present_is_idempotent`
//! 9. `pattern_a_sbu_nf_les_nf_rewrites_emit_no_applied_fix`

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
    // PR 9b T132 / FR-046: field renamed from `dissem_controls` to
    // `dissem_us` (US-classified fixtures route here per CAPCO-2016
    // §G.2 Table 5).
    a.dissem_us = dissem.to_vec().into_boxed_slice();
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
// Test 1 — `sbu_nf_portion_projects_noforn_to_page_dissem_unclassified`
// ---------------------------------------------------------------------------

/// `(U//SBU-NF)` portion: `scheme.project(Scope::Page, ...)` must produce
/// a page-level marking whose `dissem_controls` contains NOFORN.
///
/// This is the load-bearing unclassified-stratum test. §H.9 p178 at
/// `CAPCO-2016.md:4410` says SBU-NF "May only be used with UNCLASSIFIED"
/// — `(U//SBU-NF)` is the canonical valid form for the marking, and is
/// the example portion mark explicitly given on the §H.9 p178 entry
/// (banner-form heading at `:4398`: "Example Portion Mark: (U//SBU-NF)").
///
/// `PageContext::expected_non_ic_dissem`'s split logic only fires in
/// classified docs (`page_context.rs:726`'s `if classified` gate), so
/// the unclassified stratum is where Pattern A's scheme-projection-layer
/// invariant is load-bearing — Pattern A is the only NF-injection path
/// for `(U//SBU-NF)` portions.
///
/// Exercises the `capco/sbu-nf-implies-noforn` PageRewrite:
/// `Contains(CAT_NON_IC_DISSEM, TOK_SBU_NF)` fires → `FactAdd(NOFORN,
/// Scope::Page)` adds NOFORN to the page dissem axis.
///
/// Authority: CAPCO-2016 §H.9 p178 (banner-form heading "SENSITIVE BUT
/// UNCLASSIFIED NOFORN" at `:4388-4398` + Commingling Rule at
/// `:4420-4421` + §D.2 Table 3 rows 3-5 at `:590-595`).
#[test]
fn sbu_nf_portion_projects_noforn_to_page_dissem_unclassified() {
    let scheme = CapcoScheme::new();
    let portion = portion_with_non_ic(Classification::Unclassified, &[NonIcDissem::SbuNf]);

    let projected = scheme.project(Scope::Page, &[portion]);

    assert!(
        projected.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "capco/sbu-nf-implies-noforn rewrite must add NOFORN to page dissem \
         when a portion contains SBU-NF (CAPCO-2016 §H.9 p178 banner-form \
         heading 'SENSITIVE BUT UNCLASSIFIED NOFORN'); \
         got dissem_controls = {:?}",
        projected.0.dissem_controls,
    );
}

// ---------------------------------------------------------------------------
// Test 2 — `sbu_nf_malformed_classified_still_injects_noforn`
// ---------------------------------------------------------------------------

/// `(C//SBU-NF)` portion: `scheme.project(Scope::Page, ...)` must
/// produce a page-level marking whose `dissem_controls` contains NOFORN.
///
/// **`(C//SBU-NF)` is a malformed pre-transmutation input** — §H.9 p178
/// at `CAPCO-2016.md:4410` says SBU-NF "May only be used with
/// UNCLASSIFIED." The Commingling Rule at `:4420-4421` describes that
/// the canonical CORRECTED form for classified portions is `(C//NF)`
/// (SBU dropped, NF added — `(U//NF//SBU)` is the §H.9 example with U
/// classification, NOT `(C//SBU-NF)`). This test verifies Pattern A
/// fires **defensively** on the malformed input; the eventual
/// classified-strips-sbu rule (Pattern C, not in 8.F.2 scope) will
/// transmute the portion to `(C//NF)`.
///
/// Mechanically, Pattern A's predicate is classification-agnostic — it
/// scans `non_ic_dissem` for `SbuNf` regardless of classification — so
/// the assertion holds even on malformed input. This is the same
/// defensive shape exhibited by 8.F's NODIS/EXDIS Pattern A entries
/// (`unclassified_nodis_and_exdis_portions_still_inject_noforn` in
/// `pattern_a_noforn_supremacy.rs`).
///
/// Authority: CAPCO-2016 §H.9 p178 (Commingling Rule + structural
/// banner-form heading).
#[test]
fn sbu_nf_malformed_classified_still_injects_noforn() {
    let scheme = CapcoScheme::new();
    let portion = portion_with_non_ic(Classification::Confidential, &[NonIcDissem::SbuNf]);

    let projected = scheme.project(Scope::Page, &[portion]);

    assert!(
        projected.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "capco/sbu-nf-implies-noforn rewrite must fire even on malformed \
         classified SBU-NF input (CAPCO-2016 §H.9 p178 at `:4410` says SBU-NF \
         'May only be used with UNCLASSIFIED'; the Pattern A predicate is \
         classification-agnostic and fires defensively until Pattern C \
         classified-strips-sbu transmutes the portion); \
         got dissem_controls = {:?}",
        projected.0.dissem_controls,
    );
}

// ---------------------------------------------------------------------------
// Test 3 — `les_nf_portion_projects_noforn_to_page_dissem_unclassified`
// ---------------------------------------------------------------------------

/// `(U//LES-NF)` portion: `scheme.project(Scope::Page, ...)` must
/// produce a page-level marking whose `dissem_controls` contains NOFORN.
///
/// `(U//LES-NF)` is the example portion mark explicitly given on the
/// §H.9 p185 entry (banner-form heading at `CAPCO-2016.md:4542`:
/// "Example Portion Mark: (U//LES-NF)"). The unclassified stratum is
/// covered by Pattern A's scheme-projection-layer invariant —
/// `PageContext`'s classified-doc split logic does not fire.
///
/// Exercises the `capco/les-nf-implies-noforn` PageRewrite:
/// `Contains(CAT_NON_IC_DISSEM, TOK_LES_NF)` fires → `FactAdd(NOFORN,
/// Scope::Page)` adds NOFORN to the page dissem axis.
///
/// Authority: CAPCO-2016 §H.9 p185 (banner-form heading "LAW ENFORCEMENT
/// SENSITIVE NOFORN" at `:4532-4542` + Precedence Rule at `:4558` +
/// §D.2 Table 3 rows 6-8 at `:590-595`).
#[test]
fn les_nf_portion_projects_noforn_to_page_dissem_unclassified() {
    let scheme = CapcoScheme::new();
    let portion = portion_with_non_ic(Classification::Unclassified, &[NonIcDissem::LesNf]);

    let projected = scheme.project(Scope::Page, &[portion]);

    assert!(
        projected.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "capco/les-nf-implies-noforn rewrite must add NOFORN to page dissem \
         when a portion contains LES-NF (CAPCO-2016 §H.9 p185 banner-form \
         heading 'LAW ENFORCEMENT SENSITIVE NOFORN'); \
         got dissem_controls = {:?}",
        projected.0.dissem_controls,
    );
}

// ---------------------------------------------------------------------------
// Test 4 — `les_nf_portion_projects_noforn_to_page_dissem_classified`
// ---------------------------------------------------------------------------

/// `(S//LES-NF)` portion: `scheme.project(Scope::Page, ...)` must
/// produce a page-level marking whose `dissem_controls` contains NOFORN.
///
/// Unlike SBU-NF, LES-NF is a **valid form** in classified portions.
/// §H.9 p185 at `CAPCO-2016.md:4554` (Relationship(s) field) says
/// LES-NF "May be used with TOP SECRET, SECRET, CONFIDENTIAL, or
/// UNCLASSIFIED." The Precedence Rule at `:4558` further specifies:
/// "When a classified document contains portions of U//LES- NF, the
/// 'LES' marking is used in the banner line and the NOFORN marking is
/// applied as a Dissemination Control Marking. For example:
/// SECRET//NOFORN//LES." (note: source has whitespace OCR artifact
/// "LES- NF" rendered with a space; canonical token is LES-NF.)
///
/// **Source-doc internal contradiction.** The same §H.9 p185 entry at
/// `:4552` (Additional Marking Instructions field) reads "Applicable
/// only to unclassified information" — which appears to conflict with
/// the Relationship(s) enumeration at `:4554` and with the Precedence
/// Rule at `:4558`. The Relationship(s) field governs behavioral scope
/// (it explicitly enumerates the permitted classification levels) and
/// is the authority for §H.9 entries. The `:4552` line appears to be a
/// vestigial paste from the sibling LES entry (`:4471`, where LES IS
/// unclassified-only). `NonIcDissem::LesNf`'s implementation makes the
/// same `:4554`-governs determination. Test #4 defers to `:4554` per
/// the precedent in scheme.rs's LES-NF derivation block.
///
/// Pattern A confirms NOFORN materializes on the projected page dissem
/// axis under the classified stratum, matching the `SECRET//NOFORN//LES`
/// banner-form expectation. Authority: CAPCO-2016 §H.9 p185 at `:4554`
/// (classification scope) and `:4558` (Precedence Rule).
#[test]
fn les_nf_portion_projects_noforn_to_page_dissem_classified() {
    let scheme = CapcoScheme::new();
    let portion = portion_with_non_ic(Classification::Secret, &[NonIcDissem::LesNf]);

    let projected = scheme.project(Scope::Page, &[portion]);

    assert!(
        projected.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "capco/les-nf-implies-noforn rewrite must fire on classified \
         LES-NF portions (CAPCO-2016 §H.9 p185 at `:4554`: LES-NF 'May be \
         used with TOP SECRET, SECRET, CONFIDENTIAL, or UNCLASSIFIED'; \
         §H.9 p185 at `:4558` Precedence Rule: SECRET//NOFORN//LES); \
         got dissem_controls = {:?}",
        projected.0.dissem_controls,
    );
}

// ---------------------------------------------------------------------------
// Test 5 — `sbu_nf_portion_composes_with_noforn_clears_rel_to`
// ---------------------------------------------------------------------------

/// `(U//SBU-NF)` portion paired with a synthetic prior REL TO: the
/// composition `capco/sbu-nf-implies-noforn` → `capco/noforn-clears-rel-to`
/// produces a projected page with NOFORN in `dissem_controls` AND an empty
/// `rel_to`.
///
/// This is the load-bearing composition test for SBU-NF — it verifies
/// the *declaration order* of rewrites in `CapcoScheme::build_page_rewrites`
/// places the SBU-NF DISSEM-writing rewrite before the DISSEM-reading
/// `noforn-clears-rel-to`. `CapcoScheme::project` applies rewrites in
/// declaration order (`for rw in &self.page_rewrites`), NOT in the
/// scheduler's topological order — so the test pins the *declaration*
/// invariant.
///
/// The scheduler's topological ordering invariant (DISSEM writers
/// precede `capco/noforn-clears-rel-to`) is covered separately by
/// `phase_3_noforn_clearer_runs_after_dissem_transmutations` in
/// `crates/capco/tests/corpus_parity.rs`, which asserts position
/// inequalities on `Engine::scheduled_rewrites()` for all 7 DISSEM
/// writers including the four Pattern A entries. The two layers
/// complement: declaration order is the runtime guarantee today;
/// scheduler order is the contract for the future Phase D/E execution
/// loop that will iterate `scheduled_rewrites` instead of declaration
/// order.
///
/// The "prior REL TO" is injected by including a second portion that
/// carries REL TO but no SBU-NF — simulating a document where one
/// portion is `(S//REL TO USA, GBR)` and another is `(U//SBU-NF)`.
///
/// Expected result: NOFORN present, REL TO cleared (the `noforn-clears-
/// rel-to` rewrite fired AFTER the `sbu-nf-implies-noforn` rewrite added
/// NOFORN to the projected page state).
///
/// Authority: CAPCO-2016 §H.9 p178 + §D.2 Table 3 / §H.8 p145.
#[test]
fn sbu_nf_portion_composes_with_noforn_clears_rel_to() {
    let scheme = CapcoScheme::new();

    // Portion A: carries SBU-NF, no REL TO.
    let sbu_nf_portion = portion_with_non_ic(Classification::Unclassified, &[NonIcDissem::SbuNf]);

    // Portion B: carries REL TO (USA, GBR), no SBU-NF/LES-NF. This
    // populates the page's REL TO axis before the rewrites fire.
    let rel_to_portion = portion_with_rel_to(Classification::Secret, &[CountryCode::USA, gbr()]);

    let projected = scheme.project(Scope::Page, &[sbu_nf_portion, rel_to_portion]);

    assert!(
        projected.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NOFORN must be present in page dissem after sbu-nf-implies-noforn fires; \
         got dissem_controls = {:?}",
        projected.0.dissem_controls,
    );

    assert!(
        projected.0.rel_to.is_empty(),
        "REL TO must be cleared after noforn-clears-rel-to fires downstream \
         of sbu-nf-implies-noforn (declaration order in `build_page_rewrites` \
         places SBU-NF writer before NOFORN clearer); got rel_to = {:?}",
        projected.0.rel_to,
    );
}

// ---------------------------------------------------------------------------
// Test 6 — `les_nf_portion_composes_with_noforn_clears_rel_to`
// ---------------------------------------------------------------------------

/// Mirror of test 5 for LES-NF. `(U//LES-NF)` portion paired with a
/// synthetic prior REL TO: declaration order places `les-nf-implies-noforn`
/// before `noforn-clears-rel-to`, so the projected page has NOFORN and
/// empty REL TO. Two tests (not one combined) preserve per-token
/// observability — same convention as 8.F's `nodis_portion_composes…`
/// and `exdis_portion_composes…` pair in `pattern_a_noforn_supremacy.rs`.
/// (Scheduler-order coverage is in
/// `phase_3_noforn_clearer_runs_after_dissem_transmutations` in
/// `corpus_parity.rs`.)
///
/// Authority: CAPCO-2016 §H.9 p185 + §D.2 Table 3 / §H.8 p145.
#[test]
fn les_nf_portion_composes_with_noforn_clears_rel_to() {
    let scheme = CapcoScheme::new();

    let les_nf_portion = portion_with_non_ic(Classification::Unclassified, &[NonIcDissem::LesNf]);
    let rel_to_portion = portion_with_rel_to(Classification::Secret, &[CountryCode::USA, gbr()]);

    let projected = scheme.project(Scope::Page, &[les_nf_portion, rel_to_portion]);

    assert!(
        projected.0.dissem_iter().any(|d| d == &DissemControl::Nf),
        "NOFORN must be present in page dissem after les-nf-implies-noforn fires; \
         got dissem_controls = {:?}",
        projected.0.dissem_controls,
    );

    assert!(
        projected.0.rel_to.is_empty(),
        "REL TO must be cleared after noforn-clears-rel-to fires downstream \
         of les-nf-implies-noforn; got rel_to = {:?}",
        projected.0.rel_to,
    );
}

// ---------------------------------------------------------------------------
// Test 7 — `portion_without_sbu_nf_or_les_nf_does_not_inject_noforn`
// ---------------------------------------------------------------------------

/// Negative test: a `(U//SBU)` portion (plain SBU, not SBU-NF) plus a
/// `(U//LES)` portion (plain LES, not LES-NF) must NOT receive NOFORN
/// in the projected page marking. The new rewrites are triggered only
/// by `Contains(CAT_NON_IC_DISSEM, TOK_SBU_NF)` and
/// `Contains(CAT_NON_IC_DISSEM, TOK_LES_NF)`; a portion with the plain
/// variants must not fire either rewrite.
///
/// `NonIcDissem::Sbu` / `NonIcDissem::SbuNf` and `NonIcDissem::Les` /
/// `NonIcDissem::LesNf` are distinct enum variants at
/// `crates/ism/src/attrs.rs:1160`/`:1163` and `:1165`/`:1168`. Catches
/// an over-eager predicate in `capco_category_contains` that would scan
/// for any `non_ic_dissem` entry rather than specifically `SbuNf`/`LesNf`.
#[test]
fn portion_without_sbu_nf_or_les_nf_does_not_inject_noforn() {
    let scheme = CapcoScheme::new();

    // Plain `(U//SBU)` — plain SBU, NOT SBU-NF.
    let sbu = portion_with_non_ic(Classification::Unclassified, &[NonIcDissem::Sbu]);
    let projected_sbu = scheme.project(Scope::Page, &[sbu]);
    assert!(
        !projected_sbu
            .0
            .dissem_iter()
            .any(|d| d == &DissemControl::Nf),
        "A plain (U//SBU) portion must not have NOFORN injected by the \
         SBU-NF/LES-NF-implies-noforn rewrites; got dissem_controls = {:?}",
        projected_sbu.0.dissem_controls,
    );

    // Plain `(U//LES)` — plain LES, NOT LES-NF.
    let les = portion_with_non_ic(Classification::Unclassified, &[NonIcDissem::Les]);
    let projected_les = scheme.project(Scope::Page, &[les]);
    assert!(
        !projected_les
            .0
            .dissem_iter()
            .any(|d| d == &DissemControl::Nf),
        "A plain (U//LES) portion must not have NOFORN injected by the \
         SBU-NF/LES-NF-implies-noforn rewrites; got dissem_controls = {:?}",
        projected_les.0.dissem_controls,
    );

    // Empty portion — sanity-check: a plain `(S)` portion also must not
    // gain NOFORN. (Stronger version of the above two negatives.)
    let plain = portion_classified(Classification::Secret);
    let projected_plain = scheme.project(Scope::Page, &[plain]);
    assert!(
        !projected_plain
            .0
            .dissem_controls
            .contains(&DissemControl::Nf),
        "A plain (S) portion must not have NOFORN injected by the \
         SBU-NF/LES-NF-implies-noforn rewrites; got dissem_controls = {:?}",
        projected_plain.0.dissem_controls,
    );
}

// ---------------------------------------------------------------------------
// Test 8 — `sbu_nf_portion_with_noforn_already_present_is_idempotent`
// ---------------------------------------------------------------------------

/// `(C//SBU-NF/NF)` (synthetic): `non_ic_dissem` has `SbuNf` AND
/// `dissem_controls` has `Nf`. The `capco/sbu-nf-implies-noforn` rewrite
/// fires but `apply_fact_add` returns `IntentInapplicable` (silent
/// per-intent no-op). The projected marking must contain NOFORN
/// exactly once — no panic, no double-add.
///
/// **Idempotence path lives at the `CAT_DISSEM` arm in
/// `apply_fact_add` (the `if category == CAT_DISSEM` block), NOT the
/// unmatched-arm fallthrough at the bottom of `apply_fact_add`.** The
/// Pattern A action emits `FactAdd(FactRef::Cve(TOK_NOFORN), Scope::Page)`.
/// `TOK_NOFORN` maps to `CAT_DISSEM` via `capco_token_category`. The
/// action target is `CAT_DISSEM`, so the second FactAdd's idempotence
/// check runs against the `dissem_controls` axis (the
/// `if attrs.dissem_iter().any(|d| d == &target)` check returns
/// `IntentInapplicable`). The unmatched-arm fallthrough is forward-
/// compatibility only — it exists so Pattern C
/// `classified-strips-{sbu,les}` rewrites can land later without silent
/// fall-through, but it is NOT on the 8.F.2 execution path. Line
/// numbers omitted because they drift with refactors; grep
/// `apply_fact_add` in `crates/capco/src/scheme.rs` to find the current
/// location.
///
/// (`(C//SBU-NF)` is the same malformed pre-transmutation input as
/// test #2 — used here because the idempotence behavior is identical
/// regardless of malformedness, and the test isolates a single
/// invariant: the second FactAdd must be a silent no-op.)
#[test]
fn sbu_nf_portion_with_noforn_already_present_is_idempotent() {
    let scheme = CapcoScheme::new();

    // `(C//SBU-NF/NF)` — NOFORN already in dissem_controls; SBU-NF in
    // non_ic_dissem. Synthetic test fixture to isolate the idempotence
    // path — the form does not need to be a real CAPCO portion.
    let portion = portion_with_non_ic_and_dissem(
        Classification::Confidential,
        &[NonIcDissem::SbuNf],
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
        "NOFORN must appear exactly once after sbu-nf-implies-noforn fires on \
         a portion that already has NOFORN — the FactAdd idempotence path \
         (`apply_fact_add → IntentInapplicable` silent no-op at the \
         CAT_DISSEM arm in `apply_fact_add`, NOT the unmatched-arm \
         fallthrough) must not double-add; got dissem_controls = {:?}",
        projected.0.dissem_controls,
    );

    // Mirror: LES-NF with NOFORN already present.
    let les_nf_portion = portion_with_non_ic_and_dissem(
        Classification::Secret,
        &[NonIcDissem::LesNf],
        &[DissemControl::Nf],
    );

    let les_nf_projected = scheme.project(Scope::Page, &[les_nf_portion]);

    let les_nf_noforn_count = les_nf_projected
        .0
        .dissem_controls
        .iter()
        .filter(|d| matches!(d, DissemControl::Nf))
        .count();

    assert_eq!(
        les_nf_noforn_count, 1,
        "NOFORN must appear exactly once after les-nf-implies-noforn fires on \
         a portion that already has NOFORN — same idempotence invariant \
         (CAT_DISSEM arm); got dissem_controls = {:?}",
        les_nf_projected.0.dissem_controls,
    );
}

// ---------------------------------------------------------------------------
// Test 9 — `pattern_a_sbu_nf_les_nf_rewrites_emit_no_applied_fix`
// ---------------------------------------------------------------------------

/// G13 content-ignorance gate for the new rewrites.
///
/// The two Pattern A rewrites (`capco/sbu-nf-implies-noforn`,
/// `capco/les-nf-implies-noforn`) are **scheduler-validated but
/// execution-deferred**: `Engine::new` validates their intent payloads
/// and topological ordering at construction time, but `Engine::lint` /
/// `Engine::fix` does not currently iterate scheduled rewrites
/// (the engine's runtime banner-validation path drives through
/// `marque_ism::PageContext` directly — see design spec §5).
///
/// Therefore, `Engine::fix` on a SBU-NF / LES-NF / REL-TO input must
/// produce NO `AppliedFix` records whose `proposal.rule` matches the
/// new rewrite IDs. The rewrites exist in the scheme's declarative
/// table; they produce no engine-promoted fixes until the Phase D/E
/// execution loop lands.
///
/// **Positive control: E002**. Unlike 8.F (which used E038
/// NODIS/EXDIS-requires-NOFORN), §H.9 p178 / p185 do NOT have a
/// "Requires NOFORN." sentence and no rule fires on `(U//SBU-NF)` /
/// `(U//LES-NF)` saying "NOFORN is missing" (NOFORN is structurally
/// part of the marking). The fixture pins a second portion
/// `(S//REL TO GBR)` which deterministically triggers E002
/// (`missing-usa-trigraph`, `Severity::Fix`) as the positive control
/// proving the fix pipeline executed.
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
///     if applied.rule.as_str() == "capco/sbu-nf-implies-noforn"
///         || applied.rule.as_str() == "capco/les-nf-implies-noforn"
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
/// See PR 3c.B Sub-PR 8.F.2 design spec §8 test #9 for the complete
/// Phase D/E flip description.
#[test]
fn pattern_a_sbu_nf_les_nf_rewrites_emit_no_applied_fix() {
    // Verify `Engine::new` accepts the new rewrites (no RewriteCycle /
    // InvalidIntentInPageRewrite errors).
    let e = engine();

    // Pinned fixture (design spec §8 Test #9 positive-control):
    // `(U//SBU-NF)` covers the SBU-NF Pattern A surface;
    // `(S//REL TO GBR)` deterministically triggers E002
    // (missing-usa-trigraph, `Severity::Fix`) as the positive control
    // proving the fix pipeline executed end-to-end.
    let result = e.fix(b"(U//SBU-NF)\n(S//REL TO GBR)\n", FixMode::Apply);

    // Collect all applied rule IDs for the assertion message.
    let applied_ids: Vec<&str> = result.applied.iter().map(|af| af.rule.as_str()).collect();

    // Positive control: E002 must fire on the second portion (missing USA
    // trigraph in REL TO). This is the load-bearing assertion proving the
    // fix pipeline executed; without it, the assertions below could pass
    // vacuously if Engine::fix silently failed.
    assert!(
        applied_ids.contains(&"E002"),
        "E002 must appear in audit stream for input `(U//SBU-NF)\\n(S//REL TO GBR)\\n` \
         (second portion missing USA trigraph in REL TO). Without E002 firing, \
         the `no Pattern A AppliedFix` assertions below risk passing vacuously. \
         applied rules: {applied_ids:?}",
    );
    assert!(
        !applied_ids.contains(&"capco/sbu-nf-implies-noforn"),
        "Pattern A `capco/sbu-nf-implies-noforn` MUST NOT appear in audit stream \
         under current Engine::lint execution-deferred posture (PageRewrites are \
         scheduler-validated but not iterated at lint time). \
         TODO(Phase D/E): flip this assertion to require AppliedFix entries with \
         `proposal.original == \"\"` once banner-validation drives through scheme.project."
    );
    assert!(
        !applied_ids.contains(&"capco/les-nf-implies-noforn"),
        "Pattern A `capco/les-nf-implies-noforn` MUST NOT appear (same TODO)."
    );
}
