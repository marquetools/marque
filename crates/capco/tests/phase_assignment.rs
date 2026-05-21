// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 7a phase-assignment drift backstop (FR-021).
//!
//! Walks every registered `Rule::id()` in `CapcoRuleSet::new()` and
//! asserts the rule's declared `Phase` matches a hand-maintained
//! allowlist. The allowlist is the design's drift backstop: adding a
//! new rule without thinking about phase forces an allowlist edit,
//! and renaming or repurposing an existing rule without reconsidering
//! phase fails the test loudly.
//!
//! # Why this test exists (PM decision D-7.2)
//!
//! The `Rule::phase()` trait method defaults to `Phase::WholeMarking`
//! (`docs/refactor-006/pr-7-pm-decisions.md` D-7.2). The default is
//! the safer dispatch — a whole-marking rule mistakenly running in
//! pass-1 violates the span-shape constraint and trips the PR 7b
//! first-fire check, whereas a localized rule mistakenly running in
//! pass-2 is conservative (no I-19 false positive).
//!
//! The trade-off the default buys (no 27-line `fn phase() -> Phase
//! { Phase::WholeMarking }` boilerplate across the whole-marking
//! ruleset) needs a guard against the silent-acceptance failure
//! mode: a rule author adds a new `Phase::Localized` rule but
//! forgets to override `phase()`. Pass-2 will then dispatch the rule
//! against post-pass-1 attrs (since its declared phase silently
//! defaulted to `WholeMarking`) and skip the pass-1 splice the rule
//! intended — its fix never lands, and a localized defect goes
//! unfixed. The error mode is silent, not unsafe — but it's exactly
//! the kind of silent failure the corpus regression won't catch if
//! the rule was newly added. This test catches it by pinning every
//! registered rule's phase against an audited allowlist.
//!
//! # Drift policy
//!
//! Bumping this test requires intentional review. Do **not** silently
//! edit the expected list to make a CI failure go away. When the rule
//! set changes:
//!
//! 1. If a new rule is registered, add a row to `EXPECTED_PHASES`
//!    with the reviewed phase.
//! 2. If an existing rule's phase changes (rare), update its row and
//!    re-justify the change in the PR description.
//! 3. If a rule is retired, remove its row.
//!
//! Authority: `specs/006-engine-rule-refactor/spec.md` FR-021;
//! `docs/refactor-006/pr-7-pm-decisions.md` D-7.2;
//! `docs/plans/2026-05-02-engine-refactor-consolidated.md` §9.1.

use marque_capco::CapcoRuleSet;
use marque_rules::{Phase, RuleSet};
use std::collections::BTreeMap;

/// The closed allowlist of `(rule_id, phase)` pairs every
/// `CapcoRuleSet::new()` rule MUST match. Ordered by phase then by
/// rule ID for review readability — phase changes the dispatch
/// path, so grouping makes the localized-rule subset (currently four
/// rules) visible at a glance.
///
/// Phase rationale for each row is documented at the rule's
/// `impl Rule<CapcoScheme> for X` block via the doc comment on
/// `fn phase(&self) -> Phase`. This table is the audit-controlled
/// reflection of those per-rule declarations.
const EXPECTED_PHASES: &[(&str, Phase)] = &[
    // ----- Phase::Localized (4 rules + E064/E065/E067 declared inline
    // below) ----------------------------------------------------------
    // Each fix is a single-token rewrite (typo, migration, suggest).
    ("C001", Phase::Localized),
    ("E006", Phase::Localized),
    ("E007", Phase::Localized),
    // S004 stays a registered walker after PR #578 (its candidate
    // replacement is corpus-derived during evaluation and cannot
    // be reproduced from `(name, attrs)` via the bridge's
    // `fix_intent_by_name` shape).
    ("S004", Phase::Localized),
    // ----- Phase::WholeMarking ---------------------------------------
    // Banner roll-up walkers, cross-axis decisions, intent-only
    // FactAdd / FactRemove / Recanonicalize emissions, and no-fix
    // advisories whose span coverage is per-marking.
    //
    // PR #578: the following 15 IDs retired as registered `Rule`
    // impls — they now fire via the engine's constraint-catalog
    // bridge (`severity` + `span_anchor` live on `Constraint::Conflicts`
    // / `Constraint::Requires`; `FixIntent` synthesized by
    // `CapcoScheme::fix_intent_by_name`):
    //   E010 E012 E014 E015 E016 E021 E024 E036 E037 E038
    //   E053 E054 E055 E056 E057
    ("E002", Phase::WholeMarking),
    ("E005", Phase::WholeMarking),
    ("E008", Phase::WholeMarking),
    ("E031", Phase::WholeMarking),
    ("E039", Phase::WholeMarking),
    ("E041", Phase::WholeMarking),
    // PR 9a (issue #307): class-specific bare-HCS / bare-RSV rules per
    // CAPCO-2016 §H.4. Phase::WholeMarking because each rule's trigger
    // is a cross-token condition (classification level + SCI marking
    // shape) — the diagnostic spans a single token but the predicate
    // needs the whole marking's attrs.
    ("E061", Phase::WholeMarking),
    ("E062", Phase::WholeMarking),
    ("E063", Phase::WholeMarking),
    // PR 9a Commit 5 (issue #307): EYES / EYES ONLY → REL TO
    // conversion per §H.8 p157 + p158. Phase::Localized — the
    // text_correction span covers a single TokenKind::DissemControl
    // block (the EYES compound token).
    ("E064", Phase::Localized),
    // PR 9a T135a (issue #307 Group D): deprecated SCI long-form
    // canonicalization walker. Phase::Localized because every emitted
    // diagnostic carries a span that covers a single TokenSpan (the
    // deprecated long-form token); text-correction replacements are
    // byte-precise single-token splices.
    ("E065", Phase::Localized),
    // PR 9c.1 T134: legacy NATO compound text re-marking. Whole-marking
    // because the canonical re-rendering needs to span the full
    // candidate — the classification block AND the appended AEA/SCI
    // companion block need to land together (e.g.,
    // `(//CTSA)` → `(//CTS//ATOMAL)`).
    ("E066", Phase::WholeMarking),
    // Issue #407: bare-canonical-compound rewriter (CNWDI → RD-CNWDI,
    // NK → SI-NK, EU → SI-EU). Phase::Localized — the text_correction
    // span covers a single `TokenKind::Unknown` token (the bare-form
    // text); replacements are byte-precise single-token splices.
    ("E067", Phase::Localized),
    ("S003", Phase::WholeMarking),
    // PR 9c.2 / FR-048: S007 emits text_correction at the
    // classification token's span; the augmentation branch can also
    // emit at a RelToBlock token's span (a different token than the
    // classification block — crosses a token boundary). Phase::Localized's
    // single-token-span contract would fail the augmentation branch.
    ("S007", Phase::WholeMarking),
    // #559 close-out C1 (2026-05-19): S008 byte-surfacing twin of
    // `CLOSURE_RELIDO_SCI` / `CLOSURE_RELIDO_US_CLASS`. Emits a
    // `FactAdd(RELIDO, Scope::Portion)` intent; the engine re-renders
    // the full marking from canonical attrs at promotion time, so
    // the splice spans the candidate. Phase::WholeMarking covers
    // the marking-scope re-render even though the intent itself is
    // single-fact.
    ("S008", Phase::WholeMarking),
    // W002 retired in the PR closing #470 (CAPCO §H.7 p123
    // authorized the shape the rule warned on).
    ("W003", Phase::WholeMarking),
    ("W034", Phase::WholeMarking),
    // Issue #261: FGI with explicit trigraph (concealment contradiction).
    // Phase::WholeMarking because the optional NF companion emits a
    // `FactAdd(NOFORN, Scope::Portion)` intent that targets the whole
    // marking candidate span — a single-token splice at the classification
    // position cannot also add NOFORN to the dissem axis.
    ("E071", Phase::WholeMarking),
    // Issue #250: S009 prefer-tetragraph-collapse. Phase::WholeMarking
    // because the rule rewrites the entire RelToBlock span (multi-token
    // replacement: explicit member trigraphs → compact tetragraph form).
    // Default Off — tetragraph vs. explicit-member form is an org style
    // choice. Authority: CAPCO-2016 §H.8 p150.
    ("S009", Phase::WholeMarking),
    // ----- Phase::PageFinalization (4 rules, issues #461 + #488 + #251) ----
    // PR #488 (issue #488): S005 rel-to-opaque-uncertain-reduction
    // migrated from `Phase::WholeMarking` (Banner/CAB-gated firing)
    // to `Phase::PageFinalization`. Same dispatch shape as W004 —
    // page-level fixpoint snapshot, fires once per page at every
    // scanner-emitted `MarkingType::PageBreak` BEFORE the
    // PageContext reset plus once at end-of-document. The pre-#488
    // banner/CAB gate produced a documented false-negative on
    // banner-first / banner-less layouts (no banner candidate ⇒ no
    // firing surface). PR #488 also collapsed the historical
    // S005/S006 Suggest/Info split into a single Suggest-severity
    // rule — the split was an engine-workaround (per-rule severity
    // overwrite), NOT §-grounded; §H.8 + §D.2 Table 3 rule 21 apply
    // uniformly. Authority: CAPCO-2016 §H.8 (REL TO grammar) +
    // ODNI ISMCAT V[`marque_ism::ISMCAT_TETRA_VERSION`] Tetragraph
    // Taxonomy. Re-verified 2026-05-17 against
    // `crates/capco/docs/CAPCO-2016.md`.
    ("S005", Phase::PageFinalization),
    // PR refactor-006-pr-pagefinalization (issue #461): W004
    // joint-disunity-collapse migrated from `Phase::WholeMarking`
    // (Banner-only firing) to `Phase::PageFinalization`. The engine
    // dispatches PageFinalization rules once per page on the
    // page-level fixpoint snapshot — at every scanner-emitted
    // `MarkingType::PageBreak` BEFORE the PageContext reset, plus
    // once at end-of-document. This closes the pre-#461
    // banner-first false-negative (no closing banner → no firing
    // surface) without re-introducing the 6th-pass Mixed-page
    // false-positive (intermediate snapshot misread as
    // DisunityCollapse). Authority: §H.3 p57 (Derivative Use
    // bullets) + §H.7 p123 (FGI grammar). Re-verified 2026-05-16
    // against `crates/capco/docs/CAPCO-2016.md`.
    ("W004", Phase::PageFinalization),
    // Issue #251: S010 collapse-uniform-rel-portions. Phase::PageFinalization
    // because the rule reads `ctx.page_portions` to compare each portion's
    // explicit REL TO list against the projected page-level banner list —
    // the cross-portion comparison requires a page-level snapshot, not
    // per-marking dispatch. Default Off — compact `REL` vs. explicit
    // `REL TO <list>` is a style choice when all portions agree.
    // Authority: CAPCO-2016 §H.8 p150.
    ("S010", Phase::PageFinalization),
    // Issue #251: E072 bare-rel-portion-divergence. Phase::PageFinalization
    // because detecting the coexistence of bare-REL portions and explicit
    // REL TO portions with a divergent list requires a page-level snapshot
    // of all portions — per-marking dispatch cannot observe the cross-
    // portion relationship. Default Warn.
    // Authority: CAPCO-2016 §H.8 p150-151.
    ("E072", Phase::PageFinalization),
];

#[test]
fn every_registered_rule_declares_expected_phase() {
    let rule_set = CapcoRuleSet::new();

    // Build the actual `rule_id → phase` map from the registered set.
    // BTreeMap keys for deterministic diff output on failure.
    let actual: BTreeMap<String, Phase> = rule_set
        .rules()
        .iter()
        .map(|r| (r.id().as_str().to_owned(), r.phase()))
        .collect();

    // Build the expected map from the allowlist. A duplicate rule ID
    // in `EXPECTED_PHASES` is a test-data defect, not a ruleset drift,
    // and would be visible only if BTreeMap silently collapsed it.
    let mut expected: BTreeMap<&str, Phase> = BTreeMap::new();
    for (rule_id, phase) in EXPECTED_PHASES {
        let prior = expected.insert(*rule_id, *phase);
        assert!(
            prior.is_none(),
            "EXPECTED_PHASES contains a duplicate row for {rule_id:?} \
             — test data drift, fix EXPECTED_PHASES before re-running",
        );
    }

    // Cardinality check fast-fails on count drift before the
    // (slower) per-rule diff. Complements
    // `crates/capco/tests/post_3b_registration_pin.rs`, which pins
    // the registered rule-ID set; this test pins the per-rule phase
    // assignment over that same set.
    assert_eq!(
        actual.len(),
        expected.len(),
        "registered rule count ({actual_count}) does not match the \
         allowlist count ({expected_count}); add or remove rows in \
         EXPECTED_PHASES to match the current ruleset. \
         actual={actual:?}, expected={expected:?}",
        actual_count = actual.len(),
        expected_count = expected.len(),
    );

    // Per-rule diff. Collected first into typed lists so the failure
    // message can show both directions (missing from registration,
    // missing from allowlist, phase mismatch) in one shot.
    let mut missing_from_registration: Vec<&str> = Vec::new();
    let mut missing_from_allowlist: Vec<String> = Vec::new();
    let mut phase_mismatches: Vec<(String, Phase, Phase)> = Vec::new();

    for (expected_id, expected_phase) in &expected {
        match actual.get(*expected_id) {
            None => missing_from_registration.push(*expected_id),
            Some(actual_phase) if actual_phase != expected_phase => {
                phase_mismatches.push(((*expected_id).to_owned(), *actual_phase, *expected_phase));
            }
            Some(_) => {}
        }
    }

    for actual_id in actual.keys() {
        if !expected.contains_key(actual_id.as_str()) {
            missing_from_allowlist.push(actual_id.clone());
        }
    }

    assert!(
        missing_from_registration.is_empty()
            && missing_from_allowlist.is_empty()
            && phase_mismatches.is_empty(),
        "phase-assignment drift detected. \
         Bumping this test requires intentional review per the \
         module-level drift policy; do NOT silently edit \
         EXPECTED_PHASES to make CI green.\n\
         \n\
         Missing from registration (in allowlist but no rule registered): {missing_from_registration:?}\n\
         Missing from allowlist (registered but not in allowlist; consider phase carefully): {missing_from_allowlist:?}\n\
         Phase mismatches (rule_id, actual, expected): {phase_mismatches:?}",
    );
}

#[test]
fn allowlist_partitions_match_engine_partition_arithmetic() {
    // Independent counting check: the allowlist's per-phase partition
    // matches the registered ruleset's partition across all three
    // phases (Localized, WholeMarking, PageFinalization — the third
    // bucket landed in PR refactor-006-pr-pagefinalization for issue
    // #461). This catches the case where a rule's `phase()` body is
    // changed atomically with an EXPECTED_PHASES edit but the new
    // total accidentally double-counts (e.g., a row added to both
    // sub-sections in a hand-merge). The primary
    // `every_registered_rule_declares_expected_phase` test would
    // already catch that via the duplicate-row guard, but this
    // second view makes the count math explicit at the test surface.
    let rule_set = CapcoRuleSet::new();
    let localized_actual = rule_set
        .rules()
        .iter()
        .filter(|r| r.phase() == Phase::Localized)
        .count();
    let whole_marking_actual = rule_set
        .rules()
        .iter()
        .filter(|r| r.phase() == Phase::WholeMarking)
        .count();
    let page_finalization_actual = rule_set
        .rules()
        .iter()
        .filter(|r| r.phase() == Phase::PageFinalization)
        .count();
    let localized_expected = EXPECTED_PHASES
        .iter()
        .filter(|(_, p)| *p == Phase::Localized)
        .count();
    let whole_marking_expected = EXPECTED_PHASES
        .iter()
        .filter(|(_, p)| *p == Phase::WholeMarking)
        .count();
    let page_finalization_expected = EXPECTED_PHASES
        .iter()
        .filter(|(_, p)| *p == Phase::PageFinalization)
        .count();

    assert_eq!(
        localized_actual, localized_expected,
        "Localized count drift: registered={localized_actual}, allowlist={localized_expected}",
    );
    assert_eq!(
        whole_marking_actual, whole_marking_expected,
        "WholeMarking count drift: registered={whole_marking_actual}, \
         allowlist={whole_marking_expected}",
    );
    assert_eq!(
        page_finalization_actual, page_finalization_expected,
        "PageFinalization count drift: registered={page_finalization_actual}, \
         allowlist={page_finalization_expected}",
    );
    assert_eq!(
        localized_actual + whole_marking_actual + page_finalization_actual,
        rule_set.rules().len(),
        "Phase partition does not cover every registered rule \
         (Localized + WholeMarking + PageFinalization should sum to rules().len())",
    );
}
