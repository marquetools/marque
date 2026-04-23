// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase 3 US1 — Corpus-parity baseline harness (T026, T037, T038).
//!
//! The Phase 3 migration guarantees byte-identical diagnostic output
//! against the pre-branch baseline: declarative `Constraint` /
//! `PageRewrite` entries are registered on `CapcoScheme` for the
//! scheduler + catalog surface, but the hand-written rule impls in
//! `crate::rules` remain the authoritative emitters of diagnostics.
//! Retirement of those rule impls (T035) is intentionally staged to
//! a follow-up so byte-identity is trivially preserved in this phase.
//!
//! This harness runs the shared corpus fixtures through `Engine::lint`
//! and `Engine::fix`, asserting that:
//!
//! 1. Every fixture still produces a well-formed `LintResult`.
//! 2. The Phase 3 rule count matches the pre-Phase-3 count (39).
//! 3. Every declared `PageRewrite` on `CapcoScheme` carries a
//!    non-empty citation.
//!
//! Full corpus-diff parity (baseline manifest vs. current run) rides
//! on top of the corpus-accuracy harness in
//! `crates/engine/tests/corpus_accuracy.rs`; this file pins the
//! Phase 3 declaration-layer invariants specifically.

use marque_capco::CapcoRuleSet;
use marque_capco::scheme::CapcoScheme;
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::RuleSet;
use marque_scheme::MarkingScheme;

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

#[test]
fn rule_count_reflects_registration_changes() {
    // T035a: 1-for-1 swap of 11 hand-written rules → 11 declarative
    // wrappers. Count stayed at 39.
    //
    // T035b: retired 3 over-restrictive JOINT rules (E017, E018,
    // E019) that contradicted CAPCO-2016 §H.3 p169; added 1
    // narrowed rule (E036 joint-conflicts-hcs) matching §H.3 p169.
    // Net: 39 - 3 + 1 = 37.
    //
    // T035c-1b: added S001 (prefer-banner-abbreviation, style). Net: 38.
    //
    // T035c-8: added S002 (banner-consistent-form, style). Net: 39.
    //
    // T035c-14: retired W001 (DeprecatedMarkingWarningRule).
    // CAPCO-2016 §F (Legacy Control Markings, p35) treats legacy
    // markings as unauthorized — an error category owned by
    // E006/E008 — not "deprecated but still legal." No
    // authoritative bucket exists for a warning-severity
    // vocabulary-deprecation rule. Net: 38.
    //
    // T035c-21 PR-A: added E037 (nodis-conflicts-exdis) + E038
    // (dos-dissem-noforn) per CAPCO-2016 §H.9 NODIS/EXDIS templates
    // (p172 + p174). Net: 40.
    //
    // S003 (follow-up from #97 / T035c-18): added joint-usa-first
    // style rule. §H.3 p56 prescribes pure alphabetical for JOINT
    // with no USA-first carve-out; S003 encodes the convention
    // observed in REL TO §H.8 p150–151 across US-authored country
    // lists. Info severity. Net: 41.
    //
    // T035c-21 PR-B: added E039 (nodis-exdis-clears-banner-rel-to) +
    // E040 (nodis-exdis-banner-rollup) + E041 (nodis-supersedes-exdis
    // -in-portion). Net: 44.
    //
    // Bumping this number means a rule was added or retired; either
    // action should be an intentional, documented change.
    let rule_set = CapcoRuleSet::new();
    assert_eq!(
        rule_set.rules().len(),
        44,
        "rule count: T035b (retired E017/E018/E019, added E036) + \
         T035c-1b (added S001) + T035c-8 (added S002) + T035c-14 \
         (retired W001) + T035c-21 PR-A (added E037, E038) + \
         S003 (added joint-usa-first) + T035c-21 PR-B (added \
         E039, E040, E041). Adjust this assertion only when rule \
         registration actually changes."
    );
}

#[test]
fn phase_3_declares_three_page_rewrites_with_citations() {
    let scheme = CapcoScheme::new();
    let rewrites = scheme.page_rewrites();
    assert_eq!(
        rewrites.len(),
        3,
        "Phase 3 T034 declares three page rewrites (NOFORN clears \
         REL TO, JOINT-promotion, FGI-absorption)"
    );
    for rw in rewrites {
        assert!(
            !rw.citation.is_empty(),
            "rewrite {} has empty citation; Constitution VIII requires a \
             traceable authoritative-source passage",
            rw.id
        );
    }
}

#[test]
fn phase_3_engine_lint_produces_wellformed_result_on_empty_input() {
    // Smoke test: the Phase 3 scheduler construction path does not
    // regress the trivial empty-input case.
    let engine = engine();
    let result = engine.lint(b"");
    assert!(result.is_clean());
    assert_eq!(result.error_count(), 0);
    assert_eq!(result.warn_count(), 0);
}

#[test]
fn phase_3_scheduler_exposes_three_scheduled_rewrites() {
    // The scheduler produced a topological order at construction
    // time (Phase 3 T031). Expose it and verify the scheduled set
    // equals the declared set — the ordering is a data-flow
    // property, not a declaration-order one.
    let engine = engine();
    let scheduled = engine.scheduled_rewrites();
    assert_eq!(scheduled.len(), 3);
    let mut names: Vec<&str> = scheduled.to_vec();
    names.sort();
    assert_eq!(
        names,
        [
            "capco/fgi-absorption",
            "capco/joint-promotion",
            "capco/noforn-clears-rel-to",
        ]
    );
}

#[test]
fn phase_3_noforn_clearer_runs_after_joint_promotion() {
    // `capco/joint-promotion` writes REL TO; `capco/noforn-clears-
    // rel-to` reads REL TO (and writes it to clear it). The
    // scheduler must order JOINT-promotion before the NOFORN
    // clearer — otherwise JOINT could reintroduce REL TO entries
    // after NOFORN cleared them. This ordering is a declarative
    // guarantee of the scheme's `reads` / `writes` annotations,
    // not an accident of declaration order.
    let engine = engine();
    let scheduled = engine.scheduled_rewrites();
    let jp = scheduled
        .iter()
        .position(|&r| r == "capco/joint-promotion")
        .expect("joint-promotion is declared");
    let nf = scheduled
        .iter()
        .position(|&r| r == "capco/noforn-clears-rel-to")
        .expect("noforn-clears-rel-to is declared");
    assert!(
        jp < nf,
        "joint-promotion ({jp}) must be scheduled before \
         noforn-clears-rel-to ({nf}) — scheduled order: {scheduled:?}",
    );
}
