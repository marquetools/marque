// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Registration pin: asserts the **exact set** of 32 registered
//! `Rule::id()` values.
//!
//! Some predicate IDs (e.g. the banner-mismatch IDs emitted by
//! `BannerMatchesProjectedRule`) are per-row IDs that a single walker
//! emits via `additional_emitted_ids`; they do NOT count as separate
//! registered `Rule` impls. Likewise the 27 class-floor + 5 SCI
//! per-system catalog rows fire through the engine's constraint-catalog
//! bridge and are not registered `Rule` impls. `S004` stays a
//! registered walker because its replacement is corpus-derived during
//! evaluation and the bridge's `fix_intent_by_name(name, attrs,
//! marking_type)` shape cannot return the candidate without re-running
//! the evaluator.
//!
//! # Why a separate test from the count pin
//!
//! `crates/capco/tests/corpus_parity.rs` pins `rule_set.rules().len()`.
//! That count pin catches "a rule was added or removed." It does
//! **not** catch:
//!
//!  * a rule renamed at the same count
//!  * a rule deleted and an unrelated rule added at the same count
//!
//! The exact-set pin here closes that gap.
//!
//! # Drift policy
//!
//! Bumping this test requires intentional review. Do **not** silently
//! edit the expected list to make a CI failure go away.

use marque_capco::CapcoRuleSet;
use marque_rules::RuleSet;
use std::collections::BTreeSet;

/// The 32 registered rule IDs in wire-string form
/// (`"<scheme>:<predicate_id>"`), the exact closed set.
///
/// The wire-string form is what `RuleId::Display` produces
/// (`scheme:predicate_id`) — also what users type in `.marque.toml`
/// `[rules]` keys, and what the comparison logic builds via
/// `r.id().to_string()`. Each entry's legacy-ID + CAPCO citation is
/// recorded in `docs/refactor-006/legacy-rule-id-map.md`.
///
/// The class-floor and SCI per-system catalog rows fire through the
/// engine's constraint-catalog bridge using the catalog row's `name`
/// as the predicate ID; they are not registered `Rule` impls and do
/// not appear here.
const EXPECTED_RULE_IDS: &[&str] = &[
    // The 15 declarative-wrapper IDs (E010/E012/E014/E015/E016/E021/
    // E024/E036/E037/E038/E053/E054/E055/E056/E057) are emitted via the
    // engine's constraint-catalog bridge, not registered `Rule` impls.
    //
    // S004 stays a registered walker (see top-of-file header).
    "capco:marking.correction.token-typo",     // C001
    "capco:portion.dissem.rel-to-missing-usa", // E002
    "capco:portion.declassification.declassify-on-misplaced", // E005
    "capco:marking.deprecation.deprecated-dissem-control", // E006
    "capco:portion.metadata.x-shorthand-date-pattern", // E007
    "capco:marking.metadata.unrecognized-token", // E008
    "capco:banner.banner-rollup.sar-portions-roll-up", // E031 (SAR row;
    // walker registration. The walker emits 4 additional rule IDs
    // (E035/E040/E068/E069) per `additional_emitted_ids`; those are
    // NOT separately registered — see file header.
    "capco:page.dissem.nodis-exdis-clears-banner-rel-to", // E039
    "capco:portion.dissem.nodis-supersedes-exdis-in-portion", // E041
    "capco:portion.sci.hcs-bare-at-confidential-legacy-remark", // E061
    "capco:portion.sci.hcs-bare-suggest-subcompartment",  // E062
    "capco:portion.sci.rsv-bare-requires-compartment",    // E063
    "capco:portion.dissem.eyes-only-convert-to-rel-to",   // E064
    "capco:portion.sci.deprecated-long-form",             // E065
    // Legacy NATO compound text re-marking per CAPCO-2016 §G.2 p40
    // (Table 5 — ATOMAL/BOHEMIA/BALK as standalone registered control
    // markings) + §H.7 p122 (ATOMAL → AEA worked example) + §H.7 p127
    // (BALK/BOHEMIA → SCI worked example).
    "capco:marking.recanonicalize.legacy-nato-compound", // E066
    // Bare-canonical-compound rewriter. Three legacy short-forms (bare
    // CNWDI / NK / EU in SCI position) carry CAPCO-2016 canonical
    // compound portion marks (RD-CNWDI per §H.6 p106; SI-NK per §H.4
    // p83; SI-EU per §H.4 p78).
    "capco:marking.recanonicalize.bare-canonical-compound", // E067
    "capco:portion.classification.joint-usa-first-style",   // S003
    "capco:portion.dissem.rel-to-trigraph-suggest",         // S004
    // S006 retired; S005 is the sole survivor of the historical
    // Suggest/Info split. See the header for the collapse rationale.
    "capco:page.dissem.rel-to-uncertain-reduction", // S005
    // Bare NATO classification in a US-classified document should carry
    // `REL TO USA, NATO` per §H.7 p127 Notional Example 2 worked
    // example `(//CTS//BOHEMIA//REL TO USA, NATO)`.
    "capco:portion.nato.bare-nato-requires-rel-to-usa-nato", // S007
    // RELIDO byte-surfacing twin of the `CLOSURE_RELIDO_SCI` /
    // `CLOSURE_RELIDO_US_CLASS` lattice-layer closures. Severity::Suggest;
    // per the PR A invariant, emission is `Confidence::strict(1.0)` like
    // every other strict-path rule. Authority: CAPCO-2016 §H.8 p154 +
    // §D.2 Table 3 rule 17.
    "capco:portion.dissem.relido-implied-by-closure", // S008
    // Suggest replacing explicit member trigraph lists with a compact
    // tetragraph when all members are present. Default Off — tetragraph
    // vs. explicit-member form is an org style choice. Authority:
    // CAPCO-2016 §H.8 p150.
    "capco:page.dissem.prefer-tetragraph-collapse", // S009
    // Suggest bare REL when all portions carry the same REL TO list as
    // the banner. Default Off. Authority: §H.8 p150.
    "capco:page.dissem.collapse-uniform-rel-portions", // S010
    // Warn when bare-REL and explicit-REL-TO portions with a divergent
    // list coexist. Default Warn. Authority: §H.8 p150-151.
    "capco:page.dissem.bare-rel-portion-divergence", // E072
    // W002 is retired — CAPCO §H.7 p123 authorized the shape the rule
    // was warning on. See `crates/capco/src/rules.rs` module header for
    // the rationale.
    "capco:page.dissem.non-ic-dissem-in-classified-banner", // W003
    // Joint-disunity-collapse-to-FGI per CAPCO-2016 §H.3 p57 + §H.7
    // p123. Surfaces the cross-axis transformation when all-JOINT
    // portions disagree on producer lists and
    // JointSet::DisunityCollapse fires.
    "capco:page.fgi.joint-disunity-collapses-to-fgi", // W004
    "capco:portion.sci.unpublished-custom-control",   // W034
    // FGI with explicit trigraph when concealment intended or
    // acknowledgment contradicted per CAPCO-2016 §H.7 p124. Four-case
    // behavioral spec (Full/Empty/Partial REL TO overlap + Case B valid).
    "capco:portion.fgi.fgi-explicit-with-trigraph", // E071
    // Invalid FGI ownership tokens — category-specific diagnostic per
    // CAPCO-2016 §H.7 p123. Replaces the generic E008 surface on
    // FGI-marker spans whose ownership-list tail contains a token that
    // fails `CountryCode::admits_fgi_ownership_token` (`FVEY`, `DEUX`,
    // `ACGU`, `ISAF`, …). The E008 emission path suppresses co-firing
    // via `is_fgi_invalid_ownership_token`.
    "capco:marking.fgi.invalid-ownership-token", // E073
    // FGI ownership-trigraph-suggest. Architectural twin of S004 —
    // shape-admitted-but-unregistered FGI ownership tokens (`XX`, `ZZZ`,
    // etc.) trigger a corpus-prior + edit-distance suggestion at the
    // precise `TokenKind::FgiOwnershipTrigraph` byte span. Suggest
    // channel (engine never auto-applies). Authority: CAPCO-2016 §H.7
    // p122 + §A.6 p16.
    "capco:portion.fgi.ownership-trigraph-suggest",
    // Portion-form-in-banner / banner-form-in-portion form-mismatch
    // detection. Walker dispatch through `MARKING_FORMS` (built-in
    // `banner != portion` filter) plus a US-classification branch
    // reading `attrs.classification`. Authority: CAPCO-2016 §D.1 p27
    // (banner-line syntax) + §C.1 p25 (portion-mark syntax) + §G.1
    // Table 4 p38 (Register-closed-set).
    "capco:banner.metadata.uses-portion-form",
    "capco:portion.metadata.uses-banner-form",
];

#[test]
fn post_issue_677_registers_exact_32_rule_ids() {
    let rule_set = CapcoRuleSet::new();

    // Raw-slice cardinality — independently catches duplicate
    // registration (`Box::new(SomeRule)` appearing twice). The
    // BTreeSet collapses duplicates by ID, so the deduplicated
    // assertion below cannot distinguish "32 unique IDs from 32
    // registrations" from "32 unique IDs from 33 registrations where
    // one ID is duplicated." Belt-and-suspenders with
    // `corpus_parity.rs::rule_count_reflects_registration_changes`.
    let raw_len = rule_set.rules().len();
    assert_eq!(
        raw_len, 32,
        "raw rule slice length drifted from 32 \
         (duplicate or missing registration in CapcoRuleSet::new()): \
         raw_len={raw_len}",
    );

    // `RuleId::Display` produces the wire-string form
    // `"<scheme>:<predicate_id>"`, which is also what users type in
    // `.marque.toml [rules]` keys. `EXPECTED_RULE_IDS` carries the wire
    // strings; the comparison uses `r.id().to_string()` so the wire
    // shape is the load-bearing assertion.
    let actual: BTreeSet<String> = rule_set
        .rules()
        .iter()
        .map(|r| r.id().to_string())
        .collect();
    let expected: BTreeSet<&str> = EXPECTED_RULE_IDS.iter().copied().collect();

    // Sanity: the expected list itself is the right size and has no
    // duplicates. If this fires, the test data has drifted, not the
    // ruleset.
    assert_eq!(
        expected.len(),
        32,
        "EXPECTED_RULE_IDS does not contain 32 unique entries: {expected:?}",
    );

    // Cardinality check — fast-fails before the more expensive set
    // diff, and matches the existing count pin in corpus_parity.rs.
    assert_eq!(
        actual.len(),
        32,
        "post-#677 registered rule count drifted from 32: actual={actual:?}",
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
        "post-issue-#677 registered rule-ID set drifted. \
         Missing (expected but not registered): {missing:?}. \
         Unexpected (registered but not expected): {unexpected:?}. \
         Bumping this test requires intentional review; do not \
         silently edit EXPECTED_RULE_IDS to make CI green.",
    );
}
