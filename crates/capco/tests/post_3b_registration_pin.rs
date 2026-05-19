// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Post-PR-#578 registration pin (architecturally consistent with PR 3b.A:
//! E068 + E069 are per-row IDs emitted by `BannerMatchesProjectedRule`,
//! analogous to E035 + E040; they do NOT count as separate registered
//! `Rule` impls. Registered count is 23 after PR #578 retires 15
//! declarative wrappers (E010/E012/E014/E015/E016/E021/E024/E036/E037/
//! E038/E053/E054/E055/E056/E057) into the engine's constraint-catalog
//! bridge — these IDs are still emittable through the bridge but no
//! longer correspond to registered `Rule` impls. S004 stays a
//! registered walker because its replacement is corpus-derived during
//! evaluation and the bridge's `fix_intent_by_name(name, attrs,
//! marking_type)` shape cannot return the candidate without
//! re-running the evaluator.
//!
//! ## PR 5 PM-Addendum-I.6 deviation
//!
//! PM Addendum I.6 specified `38 → 40` for the registered-rule count
//! and proposed adding `"E068"`, `"E069"` to `EXPECTED_RULE_IDS`.
//! Mechanically this is incorrect: `rule_set.rules().len()` counts
//! `Box<dyn Rule>` entries registered via `CapcoRuleSet::new()`. The
//! E068 + E069 catalog rows live inside the existing
//! `BannerMatchesProjectedRule` walker, which is already registered
//! ONCE under `id() = "E031"` (analogous to E035 + E040 — per-row
//! emitted IDs, NOT separate walker registrations). Adding them to
//! `EXPECTED_RULE_IDS` would assert a presence that
//! `rule_set.rules().iter().map(|r| r.id())` does not produce.
//!
//! The intent of PM Addendum I.6 (closing the audit gap for the new
//! E068 + E069 IDs) is preserved by the `additional_emitted_ids`
//! contribution on the walker — `.marque.toml` configurations like
//! `[rules] E068 = "warn"` and `[rules] E069 = "warn"` are
//! recognized via the canonicalization path. Per-row diagnostics
//! carry `Diagnostic.rule = "E068"` / `"E069"` for audit-stream
//! traceability without inflating the registered count.
//!
//! Asserts the **exact set** of 38 registered `Rule::id()` values.
//!
//! # Why a separate test from the count pin
//!
//! `crates/capco/tests/corpus_parity.rs` already pins
//! `rule_set.rules().len() == 38` (post-PR-closing-#470 + post-PR-5;
//! the count rolls forward in lock-step with this test as rules land
//! or retire — see the running-count derivation comment in
//! `corpus_parity.rs::rule_count_reflects_registration_changes`).
//! That count pin catches "a rule was added or removed." It does
//! **not** catch:
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

/// The closed set of 38 registered `Rule::id()` strings post-PR-closing-#470
/// (and unchanged under PR 5, which adds E068 + E069 as per-row IDs of the
/// existing `BannerMatchesProjectedRule` walker — not new `Rule` impls).
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
/// (35 → 36). PR 9c.1 T134 adds `LegacyNatoCompoundRemarkRule` (E066)
/// — legacy NATO compound text re-marking per §G.2 p40 + §H.7 p122 +
/// §H.7 p127 (36 → 37). PR 9c.2 adds `BareNatoRequiresRelToRule` (S007)
/// — bare NATO classification in a US-classified document should carry
/// `REL TO USA, NATO` per §H.7 p127 Notional Example 2 (37 → 38).
/// PR 4b-B (006 T112) adds W004 — joint-disunity-collapse-to-FGI per
/// §H.3 p57 + §H.7 p123 (38 → 39) (CV-4 PR 4b-B 8th-pass updated from
/// §H.3 p56). Issue #407 / PR #491 adds `BareCanonicalCompoundRule`
/// (E067) — bare CNWDI / NK / EU portion-mark short-forms → canonical
/// compound forms per §H.6 p106 (RD-CNWDI), §H.4 p83 (SI-NK), §H.4
/// p78 (SI-EU) (39 → 40). PR #488 (issue #488) retires S006 — the
/// historical S005/S006 Suggest/Info split was an engine-workaround
/// (per-rule severity override is the only way to surface two
/// severities for one trigger), NOT §-grounded; CAPCO-2016 §H.8 +
/// §D.2 Table 3 rule 21 apply uniformly to REL TO atom-semantics.
/// Collapsed to a single Suggest-severity S005 under
/// `Phase::PageFinalization` so the rule also closes the pre-#488
/// banner-less false-negative (40 → 39).
/// The 27 class-floor + 5 SCI per-system catalog rows still fire; they
/// emit through the bridge as `Diagnostic.rule = "E058"` and
/// `Diagnostic.rule = "E059"` respectively (audit-stream +
/// `[rules] E058 = "off"` / `[rules] E059 = "off"` config-override
/// continuity) but are no longer counted as registered `Rule` impls.
const EXPECTED_RULE_IDS: &[&str] = &[
    // PR #578 retires the following 15 IDs as registered `Rule` impls
    // (they remain emittable via the engine's constraint-catalog bridge,
    // tracked separately in `CapcoScheme::bridge_emitted_rule_ids`):
    //   E010 E012 E014 E015 E016 E021 E024 E036 E037 E038
    //   E053 E054 E055 E056 E057
    //
    // S004 stays a registered walker (see top-of-file header).
    "C001", "E002", "E005", "E006", "E007", "E008", "E031", "E039", "E041", "E061", "E062", "E063",
    "E064", "E065",
    // PR 9c.1 T134: legacy NATO compound text re-marking per
    // CAPCO-2016 §G.2 p40 (Table 5 — ATOMAL/BOHEMIA/BALK as
    // standalone registered control markings) + §H.7 p122 (ATOMAL
    // → AEA worked example) + §H.7 p127 (BALK/BOHEMIA → SCI
    // worked example).
    "E066",
    // Issue #407 / PR #491: bare-canonical-compound rewriter. Three
    // legacy short-forms (bare CNWDI / NK / EU in SCI position) carry
    // CAPCO-2016 canonical compound portion marks (RD-CNWDI per
    // §H.6 p106; SI-NK per §H.4 p83; SI-EU per §H.4 p78).
    "E067", "S003", "S004",
    // PR #488 (issue #488): S006 retired; S005 is the sole survivor
    // of the historical Suggest/Info split. See the header for the
    // collapse rationale.
    "S005",
    // PR 9c.2 / FR-048: bare NATO classification in a US-classified
    // document should carry `REL TO USA, NATO` per §H.7 p127 Notional
    // Example 2 worked example `(//CTS//BOHEMIA//REL TO USA, NATO)`.
    "S007",
    // #559 close-out C1 (2026-05-19): RELIDO byte-surfacing twin of
    // the `CLOSURE_RELIDO_SCI` / `CLOSURE_RELIDO_US_CLASS` lattice-
    // layer closures. Severity::Suggest at confidence 0.85 — matches
    // S007's text-layer pattern. Authority: CAPCO-2016 §H.8 p154 +
    // §D.2 Table 3 rule 17.
    "S008",
    // W002 retired in the PR closing #470 — CAPCO §H.7 p123
    // authorized the shape the rule was warning on. See
    // `crates/capco/src/rules.rs` module header for the rationale.
    "W003",
    // PR 4b-B Commit 9 (006 T112): joint-disunity-collapse-to-FGI per
    // CAPCO-2016 §H.3 p57 + §H.7 p123 (CV-4 PR 4b-B 8th-pass updated
    // from §H.3 p56). Surfaces the cross-axis transformation when
    // all-JOINT portions disagree on producer lists and
    // JointSet::DisunityCollapse fires.
    "W004", "W034",
];

#[test]
fn post_pr_578_registers_exact_24_rule_ids() {
    let rule_set = CapcoRuleSet::new();

    // Raw-slice cardinality — independently catches duplicate
    // registration (`Box::new(SomeRule)` appearing twice). The
    // BTreeSet collapses duplicates by ID, so the deduplicated
    // assertion below cannot distinguish "24 unique IDs from 24
    // registrations" from "24 unique IDs from 25 registrations
    // where one ID is duplicated." Belt-and-suspenders with
    // `corpus_parity.rs::rule_count_reflects_registration_changes`.
    //
    // Issue #407 / PR #491: added E067 `BareCanonicalCompoundRule`
    // (39 → 40). PR #488 (issue #488): retired S006 (40 → 39).
    // PR closing #470: retired W002 `DeclarativeCominglingWarningRule`
    // (39 → 38). PR #578: retired 15 declarative wrappers
    // (E010/E012/E014/E015/E016/E021/E024/E036/E037/E038/E053/E054/
    // E055/E056/E057) into the engine's constraint-catalog bridge
    // (38 → 23). S004 stays a registered walker because its
    // candidate replacement is corpus-derived during evaluation.
    // #559 close-out C1 (2026-05-19): added S008
    // `RelidoImpliedByClosureRule` byte-surfacing twin of the
    // `CLOSURE_RELIDO_{SCI,US_CLASS}` lattice-layer closures
    // (23 → 24).
    let raw_len = rule_set.rules().len();
    assert_eq!(
        raw_len, 24,
        "post-#559-C1 raw rule slice length drifted from 24 \
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
        24,
        "EXPECTED_RULE_IDS does not contain 24 unique entries: {expected:?}",
    );

    // Cardinality check — fast-fails before the more expensive set
    // diff, and matches the existing count pin in corpus_parity.rs.
    assert_eq!(
        actual.len(),
        24,
        "post-#559-C1 registered rule count drifted from 24: actual={actual:?}",
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
        "post-PR-#578 registered rule-ID set drifted. \
         Missing (expected but not registered): {missing:?}. \
         Unexpected (registered but not expected): {unexpected:?}. \
         Bumping this test requires intentional review; do not \
         silently edit EXPECTED_RULE_IDS to make CI green.",
    );
}
