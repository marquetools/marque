// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T028 — post-PR-3b umbrella registration pin.
//!
//! Asserts the **exact set** of 47 registered `Rule::id()` values in
//! `CapcoRuleSet::new()` after the PR 3b umbrella (sub-moves 3b.A
//! through 3b.F) lands. This is the umbrella's structural commitment:
//! the per-sub-move tests pin per-walker behavior; this pins
//! "the closed set of 47 rules is what PR-3b umbrella delivered."
//!
//! # Why a separate test from the existing count pin
//!
//! `crates/capco/tests/corpus_parity.rs` already pins
//! `rule_set.rules().len() == 47`. That count pin catches "a rule was
//! added or removed." It does **not** catch:
//!
//!  * a rule was renamed at the same count (e.g., E060 → E061)
//!  * a rule was deleted and an unrelated rule was added at the same
//!    count (e.g., E060 dropped, E099 added)
//!
//! Both drift patterns are exactly what an umbrella regression test
//! should catch. The exact-set pin closes that gap.
//!
//! # Drift policy
//!
//! Bumping this test requires intentional review (PR 3.7 / T108 or
//! later, when the next planned rule-roster change lands). Do **not**
//! silently edit the expected list to make a CI failure go away.
//!
//! Authority for the closed-set claim: D13 attestation in
//! `specs/006-engine-rule-refactor/plan.md` ("PR 3b sub-moves" block);
//! per-sub-move running-count math in
//! `crates/capco/tests/corpus_parity.rs:170-194`.

use marque_capco::CapcoRuleSet;
use marque_rules::RuleSet;
use std::collections::BTreeSet;

/// The closed set of 47 registered `Rule::id()` strings post-PR-3b.
///
/// Source-of-truth derivation per the PR 3b net-delta math:
///
/// | Step                              | Net | Running |
/// | --------------------------------- | --- | ------- |
/// | Pre-3b baseline                   |  —  |   59    |
/// | 3b.A banner roll-up walker        |  −2 |   57    |
/// | 3b.B `PageRewrite` roster (data)  |   0 |   57    |
/// | 3b.C RELIDO `Constraint::Conflicts` | +4 |   61    |
/// | 3b.D class-floor catalog          |  −2 |   59    |
/// | 3b.E SCI per-system catalog       |  −9 |   50    |
/// | 3b.F non-canonical input walker   |  −3 |   47    |
const EXPECTED_RULE_IDS: &[&str] = &[
    "C001", "E001", "E002", "E003", "E004", "E005", "E006", "E007", "E008", "E009", "E010", "E011",
    "E012", "E013", "E014", "E015", "E016", "E021", "E024", "E026", "E029", "E030", "E031", "E032",
    "E036", "E037", "E038", "E039", "E041", "E052", "E053", "E054", "E055", "E056", "E057", "E058",
    "E059", "E060", "S001", "S002", "S003", "S004", "S005", "S006", "W002", "W003", "W034",
];

#[test]
fn post_3b_registers_exact_47_rule_ids() {
    let rule_set = CapcoRuleSet::new();

    // Raw-slice cardinality — independently catches duplicate
    // registration (`Box::new(SomeRule)` appearing twice). The
    // BTreeSet collapses duplicates by ID, so the deduplicated
    // assertion below cannot distinguish "47 unique IDs from 47
    // registrations" from "47 unique IDs from 48 registrations
    // where one ID is duplicated." Belt-and-suspenders with
    // `corpus_parity.rs::rule_count_reflects_pr_3b`.
    let raw_len = rule_set.rules().len();
    assert_eq!(
        raw_len, 47,
        "post-3b raw rule slice length drifted from 47 \
         (duplicate or missing registration in CapcoRuleSet::new()): \
         raw_len={raw_len}",
    );

    let actual: BTreeSet<String> = rule_set
        .rules()
        .iter()
        .map(|r| r.id().as_str().to_owned())
        .collect();
    let expected: BTreeSet<&str> = EXPECTED_RULE_IDS.iter().copied().collect();

    // Sanity: the expected list itself is the right size and has no
    // duplicates. If this fires, the test data has drifted, not the
    // ruleset.
    assert_eq!(
        expected.len(),
        47,
        "EXPECTED_RULE_IDS does not contain 47 unique entries: {expected:?}",
    );

    // Cardinality check — fast-fails before the more expensive set
    // diff, and matches the existing count pin in corpus_parity.rs.
    assert_eq!(
        actual.len(),
        47,
        "post-3b registered rule count drifted from 47: actual={actual:?}",
    );

    // Exact-set check — the load-bearing assertion.
    let missing: Vec<&str> = expected
        .iter()
        .copied()
        .filter(|id| !actual.contains(*id))
        .collect();
    let unexpected: Vec<&str> = actual
        .iter()
        .filter(|id| !expected.contains(id.as_str()))
        .map(|s| s.as_str())
        .collect();
    assert!(
        missing.is_empty() && unexpected.is_empty(),
        "post-3b registered rule-ID set drifted. \
         Missing (expected but not registered): {missing:?}. \
         Unexpected (registered but not expected): {unexpected:?}. \
         Bumping this test requires intentional review (PR 3.7 / T108 \
         or later); do not silently edit EXPECTED_RULE_IDS to make CI \
         green.",
    );
}
