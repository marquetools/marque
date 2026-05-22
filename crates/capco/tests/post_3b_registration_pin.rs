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
//! contribution on the walker — post-T044 `.marque.toml` configurations
//! like `[rules] "capco:banner.classification.mismatch-vs-projected" =
//! "warn"` and `[rules] "capco:banner.fgi.marker-mismatch-vs-projected"
//! = "warn"` are recognized via the canonicalization path. Per-row
//! diagnostics carry the respective predicate IDs for audit-stream
//! traceability without inflating the registered count.
//!
//! Asserts the **exact set** of 28 registered `Rule::id()` values.
//!
//! # Why a separate test from the count pin
//!
//! `crates/capco/tests/corpus_parity.rs` already pins
//! `rule_set.rules().len() == 28` (post-issue-#261 + post-PR-5;
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

/// The closed set of 28 registered `Rule::id()` strings post-issue-#251.
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
/// emit through the bridge using the catalog row's `name` as the
/// predicate ID (post-T044 the bridge became a no-op pass-through —
/// e.g., `class-floor/HCS-comp-sub` row emits with predicate
/// `banner.classification.floor-hcs-comp-sub`). They are no longer
/// counted as registered `Rule` impls.
/// Issue #261 adds `FgiExplicitWithTrigraphRule` (E071) — FGI with
/// explicit trigraph when concealment is intended or acknowledgment is
/// contradicted per §H.7 p124 (24 → 25). Issue #250 adds
/// `PreferTetragraphCollapseRule` (S009) — suggest replacing explicit
/// member trigraph lists with a compact tetragraph when all members are
/// present per §H.8 p150 (25 → 26). Issue #251 adds
/// `CollapseUniformRelPortionsRule` (S010) and
/// `BareRelPortionDivergenceRule` (E072) — REL TO / bare-REL portion
/// consistency rules per §H.8 p150-151 (26 → 28).
/// The 28 registered rule IDs in wire-string form
/// (`"<scheme>:<predicate_id>"`).
///
/// Post-T044 the legacy E### / W### / C### / S### / R### flat-string IDs
/// became the 2-tuple `(scheme, predicate_id)` shape. The wire-string
/// form here is what `RuleId::Display` produces (`scheme:predicate_id`)
/// — also what users type in `.marque.toml` `[rules]` keys per PM
/// decision OD-7, and what the comparison logic builds via
/// `r.id().to_string()`. Each entry's legacy-ID + CAPCO citation is
/// recorded in `docs/refactor-006/legacy-rule-id-map.md` §1.
const EXPECTED_RULE_IDS: &[&str] = &[
    // PR #578 retires the following 15 IDs as registered `Rule` impls
    // (they remain emittable via the engine's constraint-catalog bridge,
    // tracked separately in `CapcoScheme::bridge_emitted_rule_ids`):
    //   E010 E012 E014 E015 E016 E021 E024 E036 E037 E038
    //   E053 E054 E055 E056 E057
    //
    // S004 stays a registered walker (see top-of-file header).
    "capco:marking.correction.token-typo",                  // C001
    "capco:portion.dissem.rel-to-missing-usa",              // E002
    "capco:portion.declassification.declassify-on-misplaced", // E005
    "capco:marking.deprecation.deprecated-dissem-control",  // E006
    "capco:portion.metadata.x-shorthand-date-pattern",      // E007
    "capco:marking.metadata.unrecognized-token",            // E008
    "capco:banner.banner-rollup.sar-portions-roll-up",      // E031 (SAR row;
    // walker registration. The walker emits 4 additional rule IDs
    // (E035/E040/E068/E069) per `additional_emitted_ids`; those are
    // NOT separately registered — see file header.
    "capco:page.dissem.nodis-exdis-clears-banner-rel-to",   // E039
    "capco:portion.dissem.nodis-supersedes-exdis-in-portion", // E041
    "capco:portion.sci.hcs-bare-at-confidential-legacy-remark", // E061
    "capco:portion.sci.hcs-bare-suggest-subcompartment",    // E062
    "capco:portion.sci.rsv-bare-requires-compartment",      // E063
    "capco:portion.dissem.eyes-only-convert-to-rel-to",     // E064
    "capco:portion.sci.deprecated-long-form",               // E065
    // PR 9c.1 T134: legacy NATO compound text re-marking per
    // CAPCO-2016 §G.2 p40 (Table 5 — ATOMAL/BOHEMIA/BALK as
    // standalone registered control markings) + §H.7 p122 (ATOMAL
    // → AEA worked example) + §H.7 p127 (BALK/BOHEMIA → SCI
    // worked example).
    "capco:marking.recanonicalize.legacy-nato-compound",    // E066
    // Issue #407 / PR #491: bare-canonical-compound rewriter. Three
    // legacy short-forms (bare CNWDI / NK / EU in SCI position) carry
    // CAPCO-2016 canonical compound portion marks (RD-CNWDI per
    // §H.6 p106; SI-NK per §H.4 p83; SI-EU per §H.4 p78).
    "capco:marking.recanonicalize.bare-canonical-compound", // E067
    "capco:portion.classification.joint-usa-first-style",   // S003
    "capco:portion.dissem.rel-to-trigraph-suggest",         // S004
    // PR #488 (issue #488): S006 retired; S005 is the sole survivor
    // of the historical Suggest/Info split. See the header for the
    // collapse rationale.
    "capco:page.dissem.rel-to-uncertain-reduction",         // S005
    // PR 9c.2 / FR-048: bare NATO classification in a US-classified
    // document should carry `REL TO USA, NATO` per §H.7 p127 Notional
    // Example 2 worked example `(//CTS//BOHEMIA//REL TO USA, NATO)`.
    "capco:portion.nato.bare-nato-requires-rel-to-usa-nato", // S007
    // #559 close-out C1 (2026-05-19): RELIDO byte-surfacing twin of
    // the `CLOSURE_RELIDO_SCI` / `CLOSURE_RELIDO_US_CLASS` lattice-
    // layer closures. Severity::Suggest at confidence 0.85 — matches
    // S007's text-layer pattern. Authority: CAPCO-2016 §H.8 p154 +
    // §D.2 Table 3 rule 17.
    "capco:portion.dissem.relido-implied-by-closure",       // S008
    // Issue #250: suggest replacing explicit member trigraph lists with
    // a compact tetragraph when all members are present. Default Off —
    // tetragraph vs. explicit-member form is an org style choice.
    // Authority: CAPCO-2016 §H.8 p150.
    "capco:page.dissem.prefer-tetragraph-collapse",         // S009
    // Issue #251: suggest bare REL when all portions carry the same
    // REL TO list as the banner. Default Off. Authority: §H.8 p150.
    "capco:page.dissem.collapse-uniform-rel-portions",      // S010
    // Issue #251: warn when bare-REL and explicit-REL-TO portions with
    // a divergent list coexist. Default Warn. Authority: §H.8 p150-151.
    "capco:page.dissem.bare-rel-portion-divergence",        // E072
    // W002 retired in the PR closing #470 — CAPCO §H.7 p123
    // authorized the shape the rule was warning on. See
    // `crates/capco/src/rules.rs` module header for the rationale.
    "capco:page.dissem.non-ic-dissem-in-classified-banner", // W003
    // PR 4b-B Commit 9 (006 T112): joint-disunity-collapse-to-FGI per
    // CAPCO-2016 §H.3 p57 + §H.7 p123 (CV-4 PR 4b-B 8th-pass updated
    // from §H.3 p56). Surfaces the cross-axis transformation when
    // all-JOINT portions disagree on producer lists and
    // JointSet::DisunityCollapse fires.
    "capco:page.fgi.joint-disunity-collapses-to-fgi",       // W004
    "capco:portion.sci.unpublished-custom-control",         // W034
    // Issue #261: FGI with explicit trigraph when concealment intended
    // or acknowledgment contradicted per CAPCO-2016 §H.7 p124. Four-case
    // behavioral spec (Full/Empty/Partial REL TO overlap + Case B valid).
    "capco:portion.fgi.fgi-explicit-with-trigraph",         // E071
];

#[test]
fn post_pr_578_registers_exact_28_rule_ids() {
    let rule_set = CapcoRuleSet::new();

    // Raw-slice cardinality — independently catches duplicate
    // registration (`Box::new(SomeRule)` appearing twice). The
    // BTreeSet collapses duplicates by ID, so the deduplicated
    // assertion below cannot distinguish "28 unique IDs from 28
    // registrations" from "28 unique IDs from 29 registrations
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
    // (23 → 24). Issue #261: added E071 `FgiExplicitWithTrigraphRule`
    // (24 → 25). Issue #250: added S009 `PreferTetragraphCollapseRule`
    // (25 → 26). Issue #251: added S010 + E072 (26 → 28).
    let raw_len = rule_set.rules().len();
    assert_eq!(
        raw_len, 28,
        "post-#251 raw rule slice length drifted from 28 \
         (duplicate or missing registration in CapcoRuleSet::new()): \
         raw_len={raw_len}",
    );

    // T044: `RuleId` reshaped to the 2-tuple `(scheme, predicate_id)`.
    // `RuleId::Display` produces the wire-string form
    // `"<scheme>:<predicate_id>"` (per `crates/rules/src/lib.rs` Display
    // impl), which is also what users type in `.marque.toml [rules]`
    // keys per PM decision OD-7. `EXPECTED_RULE_IDS` carries the wire
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
        28,
        "EXPECTED_RULE_IDS does not contain 28 unique entries: {expected:?}",
    );

    // Cardinality check — fast-fails before the more expensive set
    // diff, and matches the existing count pin in corpus_parity.rs.
    assert_eq!(
        actual.len(),
        28,
        "post-#251 registered rule count drifted from 28: actual={actual:?}",
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
        "post-issue-#251 registered rule-ID set drifted. \
         Missing (expected but not registered): {missing:?}. \
         Unexpected (registered but not expected): {unexpected:?}. \
         Bumping this test requires intentional review; do not \
         silently edit EXPECTED_RULE_IDS to make CI green.",
    );
}
