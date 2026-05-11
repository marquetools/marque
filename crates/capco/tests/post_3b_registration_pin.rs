// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Post-PR-3c.B-Commit-6 registration pin.
//!
//! Asserts the **exact set** of 33 registered `Rule::id()` values in
//! `CapcoRuleSet::new()` after PR 3c.B Commit 6 (form-bucket migration)
//! lands. The umbrella's structural commitment: the per-commit tests
//! pin per-rule behavior; this pins the closed set the workspace
//! delivers.
//!
//! # Why a separate test from the count pin
//!
//! `crates/capco/tests/corpus_parity.rs` already pins
//! `rule_set.rules().len() == 33`. That count pin catches "a rule was
//! added or removed." It does **not** catch:
//!
//!  * a rule renamed at the same count (e.g., E058 → E061)
//!  * a rule deleted and an unrelated rule added at the same count
//!
//! Both drift patterns are exactly what a refactor regression should
//! catch. The exact-set pin closes that gap.
//!
//! # Drift policy
//!
//! Bumping this test requires intentional review. Do **not** silently
//! edit the expected list to make a CI failure go away.
//!
//! Authority: `docs/plans/2026-05-10-pr3c-consolidated-plan.md`
//! lines 788–862 (form-bucket migration commitment); per-commit
//! running-count math in `crates/capco/tests/corpus_parity.rs`.

use marque_capco::CapcoRuleSet;
use marque_rules::RuleSet;
use std::collections::BTreeSet;

/// The closed set of 33 registered `Rule::id()` strings post-PR-3c.B-Commit-6.
///
/// Derivation: PR 3b umbrella closed at 47. PR 3c.B Commit 6 retires 13
/// form rules (E001 / E003 / E004 / E009 / S001 / S002 / E011 / E013 /
/// E026 / E029 / E030 / E032 / E052) + the E060 walker into
/// `MarkingScheme::render_canonical`. Net delta: -14. Final: 33.
const EXPECTED_RULE_IDS: &[&str] = &[
    "C001", "E002", "E005", "E006", "E007", "E008", "E010", "E012", "E014", "E015", "E016", "E021",
    "E024", "E031", "E036", "E037", "E038", "E039", "E041", "E053", "E054", "E055", "E056", "E057",
    "E058", "E059", "S003", "S004", "S005", "S006", "W002", "W003", "W034",
];

#[test]
fn post_3c_b_commit_6_registers_exact_33_rule_ids() {
    let rule_set = CapcoRuleSet::new();

    // Raw-slice cardinality — independently catches duplicate
    // registration (`Box::new(SomeRule)` appearing twice). The
    // BTreeSet collapses duplicates by ID, so the deduplicated
    // assertion below cannot distinguish "33 unique IDs from 33
    // registrations" from "33 unique IDs from 34 registrations
    // where one ID is duplicated." Belt-and-suspenders with
    // `corpus_parity.rs::rule_count_reflects_registration_changes`.
    let raw_len = rule_set.rules().len();
    assert_eq!(
        raw_len, 33,
        "post-3c.B Commit 6 raw rule slice length drifted from 33 \
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
        33,
        "EXPECTED_RULE_IDS does not contain 33 unique entries: {expected:?}",
    );

    // Cardinality check — fast-fails before the more expensive set
    // diff, and matches the existing count pin in corpus_parity.rs.
    assert_eq!(
        actual.len(),
        33,
        "post-3c.B Commit 6 registered rule count drifted from 33: actual={actual:?}",
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
        "post-3c.B Commit 6 registered rule-ID set drifted. \
         Missing (expected but not registered): {missing:?}. \
         Unexpected (registered but not expected): {unexpected:?}. \
         Bumping this test requires intentional review; do not \
         silently edit EXPECTED_RULE_IDS to make CI green.",
    );
}
