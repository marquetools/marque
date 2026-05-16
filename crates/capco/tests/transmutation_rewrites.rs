// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3b.B (T026b) — declarative `PageRewrite` authoring contract.
//!
//! These tests pin the *authoring contract* of the eight transmutation
//! `PageRewrite` rows added in PR 3b.B (`docs/plans/2026-05-07-pr3b-B-
//! transmutations-plan.md`). For each row they assert:
//!
//! 1. The `id` is the exact stable string downstream tooling will
//!    match against.
//! 2. The `citation` is the exact §-cite the rule's authority is
//!    traceable to (Constitution VIII).
//! 3. The `reads` / `writes` axis annotations are exactly what the
//!    Kahn scheduler will consume to build the topological order
//!    (`crates/engine/src/scheduler.rs`).
//! 4. The `trigger` is `CategoryPredicate::Custom(_)` and the
//!    `action` is `CategoryAction::Custom(_)` — the Phase-3 stub
//!    shape (per plan §3 + §8 Q5 PM resolution).
//!
//! These are authoring-contract tests, not runtime-behavior tests.
//! Phase 3 does not drive page roll-up through `scheme.project()` —
//! the `Custom(never_fires)` triggers are inert until Phase D / E
//! lands real predicate bodies. The contract under test here is what
//! the scheduler, the catalog surface, and the citation-fidelity
//! harness see.
//!
//! Two additional tests cover scheme-construction invariants:
//! - `engine_construction_succeeds_with_full_rewrite_table` — the
//!   topological scheduler accepts the nine-row table without
//!   `RewriteCycle` or `UnannotatedCustomAxes` errors.
//! - `retired_stubs_are_no_longer_in_rewrite_table` — the two stubs
//!   retired in PR 3b.B (`capco/joint-promotion`,
//!   `capco/fgi-absorption`) are absent from the declared table; the
//!   retained `capco/noforn-clears-rel-to` is still present.

use marque_capco::CapcoScheme;
use marque_capco::scheme::{
    CAT_AEA, CAT_CLASSIFICATION, CAT_DISSEM, CAT_FGI_MARKER, CAT_JOINT_CLASSIFICATION,
};
use marque_config::Config;
use marque_engine::Engine;
use marque_scheme::{CategoryAction, CategoryPredicate, MarkingScheme, PageRewrite};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Look up a `PageRewrite` by its stable id. Panics with a clear
/// message naming both the missing id and every id present, so a
/// failed lookup tells the reviewer immediately whether the test
/// expects a typo or the rewrite truly isn't declared.
fn lookup_rewrite<'a>(scheme: &'a CapcoScheme, id: &str) -> &'a PageRewrite<CapcoScheme> {
    scheme
        .page_rewrites()
        .iter()
        .find(|r| r.id == id)
        .unwrap_or_else(|| {
            let declared: Vec<&str> = scheme.page_rewrites().iter().map(|r| r.id).collect();
            panic!("rewrite {id:?} is not declared on CapcoScheme; declared rewrites: {declared:?}")
        })
}

fn assert_predicate_is_custom(rw: &PageRewrite<CapcoScheme>) {
    assert!(
        matches!(rw.trigger, CategoryPredicate::Custom(_)),
        "rewrite {} must use CategoryPredicate::Custom (Phase-3 stub) — \
         the trigger is `never_fires` because runtime dispatch stays in \
         PageContext until Phase D / E. Got: {:?}",
        rw.id,
        rw.trigger,
    );
}

fn assert_action_is_custom(rw: &PageRewrite<CapcoScheme>) {
    assert!(
        matches!(rw.action, CategoryAction::Custom(_)),
        "rewrite {} must use CategoryAction::Custom (Phase-3 stub) — \
         the action body is `noop_action` because runtime dispatch \
         stays in PageContext until Phase D / E. Got: {:?}",
        rw.id,
        rw.action,
    );
}

// ---------------------------------------------------------------------------
// Per-entry authoring-contract tests
// ---------------------------------------------------------------------------

#[test]
fn entry_4_frd_sigma_consolidates_is_correctly_authored() {
    // Arrange
    let scheme = CapcoScheme::new();

    // Act
    let rw = lookup_rewrite(&scheme, "capco/frd-sigma-consolidates-into-rd-sigma");

    // Assert — id, citation, axes, predicate / action shape.
    assert_eq!(rw.id, "capco/frd-sigma-consolidates-into-rd-sigma");
    assert_eq!(rw.citation, "CAPCO-2016 §H.6 p113");
    assert_eq!(rw.reads, &[CAT_AEA]);
    assert_eq!(rw.writes, &[CAT_AEA]);
    assert_predicate_is_custom(rw);
    assert_action_is_custom(rw);
}

#[test]
fn entry_1_fgi_rollup_on_us_contact_is_correctly_authored() {
    // Arrange
    let scheme = CapcoScheme::new();

    // Act
    let rw = lookup_rewrite(&scheme, "capco/fgi-rollup-on-us-contact");

    // Assert — narrow-form reads (CLASS only). FGI_MARKER is a
    // predicate-scan axis (doc-comment only), excluded from `reads`
    // to avoid manufactured cycles against entries 2 and 3 per plan
    // §4. Class lift is parser-side per §3.4.1 Note (i), so CLASS is
    // not in `writes`.
    assert_eq!(rw.id, "capco/fgi-rollup-on-us-contact");
    assert_eq!(rw.citation, "CAPCO-2016 §H.7 p122");
    assert_eq!(rw.reads, &[CAT_CLASSIFICATION]);
    assert_eq!(rw.writes, &[CAT_FGI_MARKER]);
    assert_predicate_is_custom(rw);
    assert_action_is_custom(rw);
}

#[test]
fn entry_2_fgi_restricted_rollup_is_correctly_authored() {
    // Arrange
    let scheme = CapcoScheme::new();

    // Act
    let rw = lookup_rewrite(&scheme, "capco/fgi-restricted-rollup-on-us-contact");

    // Assert — narrow-form reads (CLASS only); see Entry 1 note.
    assert_eq!(rw.id, "capco/fgi-restricted-rollup-on-us-contact");
    assert_eq!(rw.citation, "CAPCO-2016 §H.7 p122");
    assert_eq!(rw.reads, &[CAT_CLASSIFICATION]);
    assert_eq!(rw.writes, &[CAT_FGI_MARKER]);
    assert_predicate_is_custom(rw);
    assert_action_is_custom(rw);
}

#[test]
fn entry_3_joint_cross_class_rollup_is_correctly_authored() {
    // Arrange
    let scheme = CapcoScheme::new();

    // Act
    let rw = lookup_rewrite(&scheme, "capco/joint-cross-class-rollup");

    // Assert — narrow-form: reads CLASS + JOINT_CLASSIFICATION
    // (the JOINT scan IS the trigger read per §H.3 p57); writes
    // FGI_MARKER only (JOINT does not roll up to banner; class
    // lift is parser-side).
    assert_eq!(rw.id, "capco/joint-cross-class-rollup");
    assert_eq!(rw.citation, "CAPCO-2016 §H.3 p57");
    assert_eq!(rw.reads, &[CAT_CLASSIFICATION, CAT_JOINT_CLASSIFICATION]);
    assert_eq!(rw.writes, &[CAT_FGI_MARKER]);
    assert_predicate_is_custom(rw);
    assert_action_is_custom(rw);
}

#[test]
fn entry_7_us_presence_promotes_bare_fgi_is_correctly_authored() {
    // Arrange
    let scheme = CapcoScheme::new();

    // Act
    let rw = lookup_rewrite(&scheme, "capco/us-presence-promotes-bare-fgi-attribution");

    // Assert — entry 7 is the one entry whose FGI_MARKER read is a
    // real dataflow dependency (consumes the post-rewrite FGI
    // state of entries 1, 2, 3), so FGI_MARKER stays in `reads`
    // and the scheduler orders entry 7 after 1, 2, 3.
    assert_eq!(rw.id, "capco/us-presence-promotes-bare-fgi-attribution");
    assert_eq!(rw.citation, "CAPCO-2016 §H.7 p122");
    assert_eq!(rw.reads, &[CAT_CLASSIFICATION, CAT_FGI_MARKER]);
    assert_eq!(rw.writes, &[CAT_FGI_MARKER]);
    assert_predicate_is_custom(rw);
    assert_action_is_custom(rw);
}

#[test]
fn entry_5_orcon_nato_transmutes_to_us_orcon_is_correctly_authored() {
    // Arrange
    let scheme = CapcoScheme::new();

    // Act
    let rw = lookup_rewrite(&scheme, "capco/orcon-nato-to-us-orcon-on-us-contact");

    // Assert — narrow-form reads (CLASS only). DISSEM is a
    // predicate-scan axis, excluded from `reads` to avoid
    // manufactured cycles against 6a / 6b.
    assert_eq!(rw.id, "capco/orcon-nato-to-us-orcon-on-us-contact");
    assert_eq!(rw.citation, "CAPCO-2016 §H.8 p136");
    assert_eq!(rw.reads, &[CAT_CLASSIFICATION]);
    assert_eq!(rw.writes, &[CAT_DISSEM]);
    assert_predicate_is_custom(rw);
    assert_action_is_custom(rw);
}

#[test]
fn entry_6a_sbu_nf_transmutes_is_correctly_authored() {
    // Arrange
    let scheme = CapcoScheme::new();

    // Act
    let rw = lookup_rewrite(&scheme, "capco/sbu-nf-transmutes-on-classified-contact");

    // Assert — narrow-form reads (CLASS only); see Entry 5 note.
    // Phase-3 axis-mapping pragmatic (§8 Q1 PM resolution):
    // CAT_DISSEM stands in for the non-IC dissem axis until
    // Phase D/E.
    assert_eq!(rw.id, "capco/sbu-nf-transmutes-on-classified-contact");
    assert_eq!(rw.citation, "CAPCO-2016 §H.9 p178");
    assert_eq!(rw.reads, &[CAT_CLASSIFICATION]);
    assert_eq!(rw.writes, &[CAT_DISSEM]);
    assert_predicate_is_custom(rw);
    assert_action_is_custom(rw);
}

#[test]
fn entry_6b_les_nf_transmutes_is_correctly_authored() {
    // Arrange
    let scheme = CapcoScheme::new();

    // Act
    let rw = lookup_rewrite(&scheme, "capco/les-nf-transmutes-on-classified-contact");

    // Assert — narrow-form reads (CLASS only); see Entry 5 note.
    // Distinct citation from Entry 6a (D13: the consultant's
    // §3.4.1 Entry 6 is split into 6a/6b so each row has exactly
    // one §-citation).
    assert_eq!(rw.id, "capco/les-nf-transmutes-on-classified-contact");
    assert_eq!(rw.citation, "CAPCO-2016 §H.9 p185");
    assert_eq!(rw.reads, &[CAT_CLASSIFICATION]);
    assert_eq!(rw.writes, &[CAT_DISSEM]);
    assert_predicate_is_custom(rw);
    assert_action_is_custom(rw);
}

// ---------------------------------------------------------------------------
// Scheme-construction invariants
// ---------------------------------------------------------------------------

#[test]
fn engine_construction_succeeds_with_full_rewrite_table() {
    // Arrange — the canonical CAPCO engine construction path. The
    // scheduler runs Kahn's algorithm at `Engine::new` over the
    // eleven-row rewrite table; this test asserts the table is
    // acyclic (no `RewriteCycle`) and that every `Custom`-shape
    // rewrite carries non-empty `reads` / `writes` annotations
    // (no `UnannotatedCustomAxes`).
    use marque_capco::CapcoRuleSet;

    // Act
    let result = Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    );

    // Assert
    let engine = result.expect(
        "Engine::new must succeed with the thirteen-row rewrite table (nine \
         from PR 3b.B + two from PR 3c.B Sub-PR 8.F: nodis-implies-noforn \
         and exdis-implies-noforn + two from PR 3c.B Sub-PR 8.F.2: \
         sbu-nf-implies-noforn and les-nf-implies-noforn) — a failure here \
         indicates either a `RewriteCycle` (a writer/reader dependency loop) \
         or `UnannotatedCustomAxes` (a `Custom` rewrite declared with empty \
         reads/writes). Both are scheme-authoring bugs.",
    );
    // Smoke-check the scheduler exposed the same twenty-one ids it was
    // handed; this prevents a regression where construction silently
    // drops a rewrite.
    // PR 3c.B Sub-PR 8.F added capco/nodis-implies-noforn and
    // capco/exdis-implies-noforn (9 → 11).
    // PR 3c.B Sub-PR 8.F.2 added capco/sbu-nf-implies-noforn and
    // capco/les-nf-implies-noforn (11 → 13).
    // DISPLAY ONLY Phase 2 added capco/noforn-clears-fdr-family
    // (§H.8 p154 + §D.2 Table 3 row 2; 13 → 14).
    // PR 4b-C Commit 3 added 7 Pattern-C rows (limdis/sbu/dod-ucni-
    // strip+promote/doe-ucni-strip+promote/fouo-evicted-by-classified;
    // §H.6 p116-119 + §H.8 p134 + §H.9 p170 + §H.9 p176; 14 → 21).
    // PR 4b-C Commit 4 adds 2 Pattern-B rows (target: 23).
    assert_eq!(engine.scheduled_rewrites().len(), 21);
}

#[test]
fn retired_stubs_are_no_longer_in_rewrite_table() {
    // Arrange
    let scheme = CapcoScheme::new();
    let ids: Vec<&str> = scheme.page_rewrites().iter().map(|r| r.id).collect();

    // Act — no separate act phase; the assertions below inspect the
    // ids vector directly.

    // Assert — both stubs removed in PR 3b.B (semantics subsumed by
    // entries 1, 3, and 7).
    assert!(
        !ids.contains(&"capco/joint-promotion"),
        "`capco/joint-promotion` was retired in PR 3b.B (replaced by \
         entries 1, 3, 7); declared ids: {ids:?}"
    );
    assert!(
        !ids.contains(&"capco/fgi-absorption"),
        "`capco/fgi-absorption` was retired in PR 3b.B (replaced by \
         entries 1, 7); declared ids: {ids:?}"
    );

    // The retained active rewrite is still present.
    assert!(
        ids.contains(&"capco/noforn-clears-rel-to"),
        "`capco/noforn-clears-rel-to` is the only currently-active \
         rewrite and MUST be retained per PR 3b.B; declared ids: {ids:?}"
    );
}
