// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Post-PR-3c.B-Commit-7.4 registration pin (PR 9a updates: 31 → 36).
//!
//! Asserts the **exact set** of 36 registered `Rule::id()` values in
//! `CapcoRuleSet::new()` after PR 9a (T135a + E061/E062/E063/E064/E065
//! additions) lands. The umbrella's structural commitment: the per-commit tests
//! pin per-rule behavior; this pins the closed set the workspace
//! delivers.
//!
//! # Why a separate test from the count pin
//!
//! `crates/capco/tests/corpus_parity.rs` already pins
//! `rule_set.rules().len() == 31`. That count pin catches "a rule was
//! added or removed." It does **not** catch:
//!
//!  * a rule renamed at the same count (e.g., E007 → E061)
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
//! §"Commit 7" + `specs/006-engine-rule-refactor/decisions/06-commit-7-subdivision.md`;
//! per-commit running-count math in `crates/capco/tests/corpus_parity.rs`.

use marque_capco::CapcoRuleSet;
use marque_rules::RuleSet;
use std::collections::BTreeSet;

/// The closed set of 36 registered `Rule::id()` strings post-PR-9a.
///
/// Derivation: PR 3b umbrella closed at 47. PR 3c.B Commit 6 retired 13
/// form rules + the E060 walker (47 → 33). PR 3c.B Commit 7.3 retires
/// `DeclarativeClassFloorRule` (E058) into the engine's constraint-
/// catalog bridge (33 → 32). PR 3c.B Commit 7.4 retires
/// `DeclarativeSciPerSystemRule` (E059) into the bridge's direct
/// `bridge_sci_per_system_diagnostics` path (32 → 31). PR 9a T135a
/// adds `DeprecatedSciLongFormRule` (E065) — deprecated SCI long-form
/// canonicalization walker per CAPCO-2016 §H.4 pp 61, 62, 74, 76, 78,
/// 85 (31 → 32). PR 9a (issue #307) adds three class-specific
/// bare-HCS / bare-RSV rules (E061 / E062 / E063) per §H.4 pp 62, 70
/// (32 → 35). PR 9a Commit 5 adds `EyesOnlyConvertToRelToRule` (E064)
/// — EYES / EYES ONLY → REL TO conversion per §H.8 p157 + p158
/// (35 → 36). The 27 class-floor + 5 SCI per-system catalog rows
/// still fire; they emit through the bridge as
/// `Diagnostic.rule = "E058"` and `Diagnostic.rule = "E059"`
/// respectively (audit-stream + `[rules] E058 = "off"` /
/// `[rules] E059 = "off"` config-override continuity) but are no
/// longer counted as registered `Rule` impls.
const EXPECTED_RULE_IDS: &[&str] = &[
    "C001", "E002", "E005", "E006", "E007", "E008", "E010", "E012", "E014", "E015", "E016", "E021",
    "E024", "E031", "E036", "E037", "E038", "E039", "E041", "E053", "E054", "E055", "E056", "E057",
    "E061", "E062", "E063", "E064", "E065",
    // PR 9c.1 T134: legacy NATO compound text re-marking per
    // CAPCO-2016 §G.2 p41 (Table 5 — ATOMAL/BOHEMIA/BALK as
    // standalone registered control markings) + §H.7 p123 (ATOMAL
    // → AEA worked example) + §H.7 p127 (BALK/BOHEMIA → SCI
    // worked example).
    "E066", "S003", "S004", "S005", "S006", "W002", "W003", "W034",
];

#[test]
fn post_pr_9a_registers_exact_37_rule_ids() {
    let rule_set = CapcoRuleSet::new();

    // Raw-slice cardinality — independently catches duplicate
    // registration (`Box::new(SomeRule)` appearing twice). The
    // BTreeSet collapses duplicates by ID, so the deduplicated
    // assertion below cannot distinguish "37 unique IDs from 37
    // registrations" from "37 unique IDs from 38 registrations
    // where one ID is duplicated." Belt-and-suspenders with
    // `corpus_parity.rs::rule_count_reflects_registration_changes`.
    //
    // PR 9c.1 T134 (this PR) added E066 — bumped from 36 to 37.
    let raw_len = rule_set.rules().len();
    assert_eq!(
        raw_len, 37,
        "post-PR-9c.1 raw rule slice length drifted from 37 \
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
        37,
        "EXPECTED_RULE_IDS does not contain 37 unique entries: {expected:?}",
    );

    // Cardinality check — fast-fails before the more expensive set
    // diff, and matches the existing count pin in corpus_parity.rs.
    assert_eq!(
        actual.len(),
        37,
        "post-PR-9c.1 registered rule count drifted from 37: actual={actual:?}",
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
        "post-PR-9a registered rule-ID set drifted. \
         Missing (expected but not registered): {missing:?}. \
         Unexpected (registered but not expected): {unexpected:?}. \
         Bumping this test requires intentional review; do not \
         silently edit EXPECTED_RULE_IDS to make CI green.",
    );
}
