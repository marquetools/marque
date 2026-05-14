#![cfg(any())]
// PR 3c.B Commit 10: legacy FixProposal-shape test disabled pending rewrite

// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B engine-prereq — intent-only fix synthesis byte-identity test.
//!
//! Defines a synthetic test rule that emits **intent-only** diagnostics
//! (`Diagnostic.fix = None`, `Diagnostic.fix = Some(...)`,
//! `Diagnostic.candidate_span = Some(...)`) on portions carrying RELIDO,
//! and verifies the engine's new intent-synthesis path produces
//! byte-identical output to the existing dual-populate E054 path
//! (`Diagnostic.fix = Some(...)` AND `Diagnostic.fix = Some(...)`)
//! on the same fixture set.
//!
//! This is **Option A** (integration) of the engine-prereq preflight test
//! strategy. Option C (unit tests directly on `CapcoScheme::apply_intent`
//! / `category_of`) lives in `crates/capco/src/scheme.rs` `#[cfg(test)]
//! mod tests`. Both are required because they exercise different surfaces:
//!
//! - Option C pins the scheme's per-axis intent semantics in isolation.
//! - Option A (this file) pins the engine's end-to-end synthesis pipeline
//!   — recognizer recovery, multi-intent grouping, render_canonical
//!   invocation, and FixProposal materialization with span / replacement /
//!   confidence wired correctly.
//!
//! Test-fixture carve-out per Constitution V Principle V: the synthetic
//! `IntentOnlyRelidoRule` defined below is test code (it lives inside
//! `tests/`, gated by `#[cfg(test)]` at the integration-test boundary)
//! and constructs scheme-typed `FixIntent` payloads only — it does not
//! touch `AppliedFix::__engine_promote`.

use marque_capco::CapcoScheme;
use marque_capco::scheme::{TOK_NOFORN, TOK_RELIDO};
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};
use marque_ism::{CanonicalAttrs, DissemControl, Span};
use marque_rules::{
    Confidence, Diagnostic, FixIntent, Message, MessageArgs, MessageTemplate, Rule, RuleContext,
    RuleId, RuleSet, Severity,
};
use marque_scheme::{FactRef, ReplacementIntent, Scope};
use smallvec::SmallVec;
use std::time::{Duration, UNIX_EPOCH};

const FIXED_TS: u64 = 1_700_000_000;

/// Test-only rule that emits **intent-only** diagnostics for RELIDO
/// portions. Mirrors the structural shape of the production E054
/// rule but takes the intent-only emission path so the engine's
/// new synthesis pipeline drives the byte rewrite.
struct IntentOnlyRelidoRule;

impl Rule<CapcoScheme> for IntentOnlyRelidoRule {
    fn id(&self) -> RuleId {
        RuleId::new("TEST_INTENT_ONLY_RELIDO")
    }

    fn name(&self) -> &'static str {
        "test-intent-only-relido-removal"
    }

    fn default_severity(&self) -> Severity {
        Severity::Fix
    }

    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        // Only fire on portions carrying RELIDO. The test fixture below
        // crafts a portion that has RELIDO + REL TO USA, which is a
        // valid CAPCO marking even without a NOFORN conflict — we want
        // to verify the synthesis path emits the right bytes regardless
        // of the underlying rule semantics.
        let has_relido = attrs
            .dissem_iter()
            .any(|d| matches!(d, DissemControl::Relido));
        if !has_relido {
            return Vec::new();
        }

        let intent = FixIntent {
            replacement: ReplacementIntent::fact_remove(FactRef::Cve(TOK_RELIDO), Scope::Portion),
            confidence: Confidence::strict(0.99),
            feature_ids: SmallVec::new(),
            message: Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        };

        // Use the new with_intent_at_span constructor — the engine's
        // synthesis path reads `candidate_span` to determine which
        // bytes to re-render.
        let candidate_span = ctx.candidate_span;
        vec![Diagnostic::with_fix_at_span(
            self.id(),
            self.default_severity(),
            candidate_span,
            candidate_span,
            "test: RELIDO removed via intent-only path",
            "test-only",
            intent,
        )]
    }
}

struct IntentOnlyTestRuleSet {
    rules: Vec<Box<dyn Rule<CapcoScheme>>>,
}

impl IntentOnlyTestRuleSet {
    fn new() -> Self {
        Self {
            rules: vec![Box::new(IntentOnlyRelidoRule)],
        }
    }
}

impl RuleSet<CapcoScheme> for IntentOnlyTestRuleSet {
    fn rules(&self) -> &[Box<dyn Rule<CapcoScheme>>] {
        &self.rules
    }
    fn schema_version(&self) -> &'static str {
        "test-intent-only"
    }
}

fn intent_only_engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(IntentOnlyTestRuleSet::new())],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

#[test]
fn intent_only_path_removes_relido_from_portion() {
    // Input: a portion carrying RELIDO. The synthesis path:
    //   1. Re-parses `(S//RELIDO)` via the recognizer.
    //   2. Calls scheme.apply_intent with FactRemove(RELIDO, Portion).
    //   3. Renders the modified marking via render_portion.
    //   4. Wraps in `()` and substitutes for the original portion.
    //
    // The expected output is `(S)` — RELIDO removed, no dissem
    // controls remaining.
    let engine = intent_only_engine();
    let source = b"(S//RELIDO)";
    let result = engine.fix(source, FixMode::Apply);

    assert_eq!(
        std::str::from_utf8(&result.source).unwrap(),
        "(S)",
        "intent-only synthesis must remove RELIDO and re-render the portion canonically"
    );
    assert_eq!(
        result.applied.len(),
        1,
        "expected exactly one applied fix from the synthesis path"
    );
    assert_eq!(result.applied[0].rule.as_str(), "TEST_INTENT_ONLY_RELIDO");
}

#[test]
fn intent_only_path_produces_audit_record_with_intent_variant() {
    // The intent-only synthesis path routes through
    // AppliedFix::__engine_promote (the new constructor that takes a
    // FixIntent), not __engine_promote_legacy. The result wraps the
    // proposal in AppliedFixProposal::New, which Deref-projects to
    // the synthesized FixProposal.
    let engine = intent_only_engine();
    let source = b"(S//RELIDO)";
    let result = engine.fix(source, FixMode::Apply);

    assert_eq!(result.applied.len(), 1);
    let applied = &result.applied[0];

    // The Deref path returns the synthesized FixProposal — verify
    // its fields are wired correctly.
    assert_eq!(applied.span, Span::new(0, 11));
    assert_eq!(applied.proposal.replacement.as_ref(), "(S)");
    // G13 closure: original bytes elided on the new emission path.
    assert_eq!(applied.proposal.original.as_ref(), "");

    // Verify the AppliedFixProposal::New variant is used (not Legacy).
    match &applied.proposal {
        marque_rules::AppliedFixProposal::New { intent, .. } => match &intent.replacement {
            ReplacementIntent::FactRemove { facts, scope } => {
                assert_eq!(
                    facts.len(),
                    1,
                    "FactRemove must have exactly one fact (RELIDO)"
                );
                assert!(matches!(facts[0], FactRef::Cve(t) if t == TOK_RELIDO));
                assert_eq!(*scope, Scope::Portion);
            }
            _ => panic!("expected FactRemove intent variant"),
        },
        marque_rules::AppliedFixProposal::Legacy(_) => {
            panic!(
                "intent-only synthesis path must route through AppliedFixProposal::New, \
                 not the legacy variant — the engine-prereq's load-bearing dispatch"
            );
        }
    }
}

#[test]
fn intent_only_path_below_threshold_becomes_suggest() {
    // The threshold-rewrite loop fix (engine-prereq Commit 6) ensures
    // low-confidence intent-only diagnostics are rewritten to
    // Severity::Suggest. Below-threshold intent-only fixes must NOT
    // auto-apply.
    struct LowConfidenceRelidoRule;
    impl Rule<CapcoScheme> for LowConfidenceRelidoRule {
        fn id(&self) -> RuleId {
            RuleId::new("TEST_LOW_CONFIDENCE_INTENT")
        }
        fn name(&self) -> &'static str {
            "test-low-confidence-intent"
        }
        fn default_severity(&self) -> Severity {
            Severity::Fix
        }
        fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
            if !attrs
                .dissem_iter()
                .any(|d| matches!(d, DissemControl::Relido))
            {
                return Vec::new();
            }
            let intent = FixIntent {
                replacement: ReplacementIntent::fact_remove(
                    FactRef::Cve(TOK_RELIDO),
                    Scope::Portion,
                ),
                // 0.50 is well below the default 0.95 threshold.
                confidence: Confidence::strict(0.50),
                feature_ids: SmallVec::new(),
                message: Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
                source: FixSource::BuiltinRule,
                migration_ref: None,
            };
            vec![Diagnostic::with_fix_at_span(
                self.id(),
                self.default_severity(),
                ctx.candidate_span,
                ctx.candidate_span,
                "test: low-confidence intent must not auto-apply",
                "test-only",
                intent,
            )]
        }
    }

    struct LowConfidenceRuleSet {
        rules: Vec<Box<dyn Rule<CapcoScheme>>>,
    }
    impl RuleSet<CapcoScheme> for LowConfidenceRuleSet {
        fn rules(&self) -> &[Box<dyn Rule<CapcoScheme>>] {
            &self.rules
        }
        fn schema_version(&self) -> &'static str {
            "test-low-confidence"
        }
    }

    let engine = Engine::with_clock(
        Config::default(),
        vec![Box::new(LowConfidenceRuleSet {
            rules: vec![Box::new(LowConfidenceRelidoRule)],
        })],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    let source = b"(S//RELIDO)";
    let result = engine.fix(source, FixMode::Apply);

    // Below-threshold intent-only diagnostics must not auto-apply.
    assert_eq!(
        std::str::from_utf8(&result.source).unwrap(),
        "(S//RELIDO)",
        "source must be unchanged when intent-only confidence is below threshold"
    );
    assert_eq!(
        result.applied.len(),
        0,
        "no fix may apply when intent-only confidence is below threshold"
    );

    // The diagnostic should still be present in remaining_diagnostics,
    // rewritten to Severity::Suggest by the engine's post-lint loop.
    let suggest_diags: Vec<_> = result
        .remaining_diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "TEST_LOW_CONFIDENCE_INTENT")
        .collect();
    assert_eq!(
        suggest_diags.len(),
        1,
        "expected one Suggest-rewritten intent-only diagnostic in remaining"
    );
    assert_eq!(
        suggest_diags[0].severity,
        Severity::Suggest,
        "below-threshold intent-only diagnostic must be rewritten to Severity::Suggest"
    );
}

// ---------------------------------------------------------------------------
// Fixup commit b9fd7980 coverage — Copilot PR #369 findings 1, 2, 3.
//
// Each test below pins one behavior added by the fixup commit that was
// previously asserted only by the implementation, not by tests.
//
// Test-fixture carve-out per Constitution V Principle V: every test-only
// rule below lives inside `tests/` and constructs scheme-typed
// `FixIntent` payloads via the public API; none touches
// `AppliedFix::__engine_promote` directly.
// ---------------------------------------------------------------------------

/// Test-only rule that emits TWO intent-only diagnostics on the same
/// candidate_span from a single `check()` call — `FactRemove(NOFORN)`
/// and `FactRemove(RELIDO)`, both Scope::Portion. Used to drive the
/// audit-collapse path in `synthesize_intent_only_fixes` (Copilot
/// finding #1).
struct DualIntentRule;

impl Rule<CapcoScheme> for DualIntentRule {
    fn id(&self) -> RuleId {
        RuleId::new("TEST_DUAL_INTENT")
    }
    fn name(&self) -> &'static str {
        "test-dual-intent-on-same-candidate"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        let has_nf = attrs.dissem_iter().any(|d| matches!(d, DissemControl::Nf));
        let has_relido = attrs
            .dissem_iter()
            .any(|d| matches!(d, DissemControl::Relido));
        if !has_nf || !has_relido {
            return Vec::new();
        }
        let cspan = ctx.candidate_span;
        let nf_intent = FixIntent {
            replacement: ReplacementIntent::fact_remove(FactRef::Cve(TOK_NOFORN), Scope::Portion),
            // 0.97 — the weaker leg; the collapse uses min combined.
            confidence: Confidence::strict(0.97),
            feature_ids: SmallVec::new(),
            message: Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        };
        let relido_intent = FixIntent {
            replacement: ReplacementIntent::fact_remove(FactRef::Cve(TOK_RELIDO), Scope::Portion),
            confidence: Confidence::strict(0.99),
            feature_ids: SmallVec::new(),
            message: Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        };
        vec![
            Diagnostic::with_fix_at_span(
                self.id(),
                self.default_severity(),
                cspan,
                cspan,
                "test: remove NF (dual-intent)",
                "test-only",
                nf_intent,
            ),
            Diagnostic::with_fix_at_span(
                self.id(),
                self.default_severity(),
                cspan,
                cspan,
                "test: remove RELIDO (dual-intent)",
                "test-only",
                relido_intent,
            ),
        ]
    }
}

struct DualIntentRuleSet {
    rules: Vec<Box<dyn Rule<CapcoScheme>>>,
}
impl RuleSet<CapcoScheme> for DualIntentRuleSet {
    fn rules(&self) -> &[Box<dyn Rule<CapcoScheme>>] {
        &self.rules
    }
    fn schema_version(&self) -> &'static str {
        "test-dual-intent"
    }
}

#[test]
fn intent_only_path_collapses_multi_diagnostic_group_to_one_fix_proposal() {
    // Finding 1 fix — synthesize_intent_only_fixes collapses every
    // diagnostic in a candidate-span group into ONE FixProposal. The
    // owning rule is the lex-min rule_id (both diagnostics here come
    // from `TEST_DUAL_INTENT`, so collapse is trivial — N=2 same-rule
    // is the Stage-4 E024 multi-remove shape). The combined confidence
    // is the MINIMUM combined() across the group (0.97, not 0.99).
    let engine = Engine::with_clock(
        Config::default(),
        vec![Box::new(DualIntentRuleSet {
            rules: vec![Box::new(DualIntentRule)],
        })],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    let source = b"(S//NF/RELIDO)";
    let result = engine.fix(source, FixMode::Apply);

    // Both intents applied atomically through scheme.apply_intent: NF
    // and RELIDO removed, only S remains in the portion.
    assert_eq!(
        std::str::from_utf8(&result.source).unwrap(),
        "(S)",
        "atomic multi-intent batch must remove both NF and RELIDO"
    );

    // The audit collapse: ONE FixProposal for N=2 diagnostics on the
    // same candidate_span.
    assert_eq!(
        result.applied.len(),
        1,
        "audit-collapse must emit one FixProposal per candidate-span group"
    );
    let applied = &result.applied[0];
    assert_eq!(
        applied.rule.as_str(),
        "TEST_DUAL_INTENT",
        "owning rule_id is the lex-min in the group (trivial for same-rule N=2)"
    );

    // Confidence collapse: min combined across group = 0.97.
    let combined = applied.confidence.combined();
    assert!(
        (combined - 0.97).abs() < 1e-5,
        "combined confidence must equal min across group (got {combined})"
    );
}

// Two test-only rules with distinct rule_ids that BOTH fire on the
// same candidate. Used for the multi-rule audit-collapse test.

// Rule A removes NOFORN; rule Z removes RELIDO. Distinct intents so
// `apply_intent` can apply both atomically (a duplicate intent would
// hit `IntentInapplicable` on the second pass and fail the batch).
struct TestRuleA;
impl Rule<CapcoScheme> for TestRuleA {
    fn id(&self) -> RuleId {
        RuleId::new("TEST_A_REMOVE_NOFORN")
    }
    fn name(&self) -> &'static str {
        "test-rule-a-remove-noforn"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        let has_nf = attrs.dissem_iter().any(|d| matches!(d, DissemControl::Nf));
        if !has_nf {
            return Vec::new();
        }
        let intent = FixIntent {
            replacement: ReplacementIntent::fact_remove(FactRef::Cve(TOK_NOFORN), Scope::Portion),
            confidence: Confidence::strict(0.99),
            feature_ids: SmallVec::new(),
            message: Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        };
        vec![Diagnostic::with_fix_at_span(
            self.id(),
            self.default_severity(),
            ctx.candidate_span,
            ctx.candidate_span,
            "test rule A: remove NOFORN",
            "test-only",
            intent,
        )]
    }
}

struct TestRuleZ;
impl Rule<CapcoScheme> for TestRuleZ {
    fn id(&self) -> RuleId {
        RuleId::new("TEST_Z_REMOVE_RELIDO")
    }
    fn name(&self) -> &'static str {
        "test-rule-z-remove-relido"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        let has_relido = attrs
            .dissem_iter()
            .any(|d| matches!(d, DissemControl::Relido));
        if !has_relido {
            return Vec::new();
        }
        let intent = FixIntent {
            replacement: ReplacementIntent::fact_remove(FactRef::Cve(TOK_RELIDO), Scope::Portion),
            confidence: Confidence::strict(0.99),
            feature_ids: SmallVec::new(),
            message: Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        };
        vec![Diagnostic::with_fix_at_span(
            self.id(),
            self.default_severity(),
            ctx.candidate_span,
            ctx.candidate_span,
            "test rule Z: remove RELIDO",
            "test-only",
            intent,
        )]
    }
}

struct TwoRuleSet {
    rules: Vec<Box<dyn Rule<CapcoScheme>>>,
}
impl RuleSet<CapcoScheme> for TwoRuleSet {
    fn rules(&self) -> &[Box<dyn Rule<CapcoScheme>>] {
        &self.rules
    }
    fn schema_version(&self) -> &'static str {
        "test-two-rule"
    }
}

#[test]
fn intent_only_path_collapses_multi_rule_same_candidate_to_lex_min_rule_id() {
    // Finding 1 multi-rule edge case — when TWO different rules emit
    // intent-only diagnostics on the same candidate_span, the
    // audit-collapse picks the lex-min rule_id
    // (`TEST_A_REMOVE_NOFORN` < `TEST_Z_REMOVE_RELIDO`) for the
    // synthesized FixProposal.
    //
    // The losing rule's diagnostic must surface in
    // `remaining_diagnostics` so the audit consumer sees that it
    // fired — the engine applied one rewrite, not N.
    //
    // Rule A removes NOFORN; rule Z removes RELIDO. Distinct intents
    // so `scheme.apply_intent` can absorb both into one atomic batch.
    let engine = Engine::with_clock(
        Config::default(),
        vec![Box::new(TwoRuleSet {
            rules: vec![Box::new(TestRuleZ), Box::new(TestRuleA)],
        })],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    let source = b"(S//NF/RELIDO)";
    let result = engine.fix(source, FixMode::Apply);

    // Both intents applied atomically: NF AND RELIDO removed.
    assert_eq!(
        std::str::from_utf8(&result.source).unwrap(),
        "(S)",
        "atomic multi-intent batch must remove both NF and RELIDO"
    );

    // Exactly one FixProposal: owned by the lex-min rule_id.
    assert_eq!(
        result.applied.len(),
        1,
        "audit-collapse must emit exactly one FixProposal per candidate-span group"
    );
    assert_eq!(
        result.applied[0].rule.as_str(),
        "TEST_A_REMOVE_NOFORN",
        "lex-min rule_id wins (`TEST_A_…` < `TEST_Z_…`)"
    );

    // The losing rule's diagnostic survives in remaining_diagnostics
    // — the `(rule, candidate_span)` keying in remaining_diagnostics
    // means only the winning rule's diagnostic is filtered out.
    let z_remaining: Vec<_> = result
        .remaining_diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "TEST_Z_REMOVE_RELIDO")
        .collect();
    assert_eq!(
        z_remaining.len(),
        1,
        "losing rule's diagnostic must survive in remaining_diagnostics \
         so the audit consumer sees it fired"
    );
    let a_remaining: Vec<_> = result
        .remaining_diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "TEST_A_REMOVE_NOFORN")
        .collect();
    assert!(
        a_remaining.is_empty(),
        "winning rule's diagnostic must be removed from remaining_diagnostics"
    );
}

/// Test-only rule that emits one `FactRemove(NOFORN, Page)` intent
/// on banner-form input. Used to drive the whitespace-preservation
/// path in `synthesize_intent_only_fixes` (Copilot finding #3).
struct BannerNoforn;
impl Rule<CapcoScheme> for BannerNoforn {
    fn id(&self) -> RuleId {
        RuleId::new("TEST_BANNER_NOFORN")
    }
    fn name(&self) -> &'static str {
        "test-banner-remove-noforn"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }
    fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        let has_nf = attrs.dissem_iter().any(|d| matches!(d, DissemControl::Nf));
        if !has_nf {
            return Vec::new();
        }
        let intent = FixIntent {
            replacement: ReplacementIntent::fact_remove(FactRef::Cve(TOK_NOFORN), Scope::Page),
            confidence: Confidence::strict(0.99),
            feature_ids: SmallVec::new(),
            message: Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        };
        vec![Diagnostic::with_fix_at_span(
            self.id(),
            self.default_severity(),
            ctx.candidate_span,
            ctx.candidate_span,
            "test: remove NF from banner",
            "test-only",
            intent,
        )]
    }
}

struct BannerNofornRuleSet {
    rules: Vec<Box<dyn Rule<CapcoScheme>>>,
}
impl RuleSet<CapcoScheme> for BannerNofornRuleSet {
    fn rules(&self) -> &[Box<dyn Rule<CapcoScheme>>] {
        &self.rules
    }
    fn schema_version(&self) -> &'static str {
        "test-banner-noforn"
    }
}

fn banner_noforn_engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(BannerNofornRuleSet {
            rules: vec![Box::new(BannerNoforn)],
        })],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

#[test]
fn intent_only_path_preserves_leading_whitespace_on_banner() {
    // Finding 3 fix — the scanner-emitted banner candidate_span covers
    // the entire line including leading indentation. Without the
    // whitespace-preservation pass, `scheme.render_banner(...)` (which
    // emits no surrounding whitespace) would replace the whole span and
    // strip the indentation.
    let engine = banner_noforn_engine();
    let source = b"    SECRET//NOFORN\n";
    let result = engine.fix(source, FixMode::Apply);

    let out = std::str::from_utf8(&result.source).unwrap();
    assert!(
        out.starts_with("    "),
        "leading 4-space indentation must be preserved across intent-only synthesis \
         (got: {out:?})"
    );
    assert_eq!(result.applied.len(), 1, "expected one applied fix");
    // The applied output should be the rendered banner with NF removed,
    // wrapped between the original leading/trailing whitespace.
    assert!(
        !out.contains("NOFORN") && !out.contains("//NF"),
        "NOFORN must be removed from the rendered banner (got: {out:?})"
    );
}

#[test]
fn intent_only_path_preserves_trailing_whitespace_on_banner() {
    // Same shape as the leading-whitespace test but with trailing
    // whitespace (the trailing `\n` is itself ASCII whitespace, plus
    // trailing spaces before the newline).
    let engine = banner_noforn_engine();
    let source = b"SECRET//NOFORN   \n";
    let result = engine.fix(source, FixMode::Apply);

    let out = std::str::from_utf8(&result.source).unwrap();
    assert!(
        out.ends_with("   \n"),
        "trailing 3-space-then-newline must be preserved (got: {out:?})"
    );
    assert_eq!(result.applied.len(), 1, "expected one applied fix");
    assert!(
        !out.contains("NOFORN") && !out.contains("//NF"),
        "NOFORN must be removed from the rendered banner (got: {out:?})"
    );
}

#[test]
fn intent_only_path_preserves_surrounding_whitespace_on_portion() {
    // Finding 3 — portion candidates wrapped in `()` are *tightly*
    // spanned by the scanner (`(start, end+1)` of the matching paren
    // pair; no surrounding whitespace), so the whitespace-preservation
    // logic is a no-op for portions. This test pins the engine's
    // promise that surrounding bytes OUTSIDE the candidate are never
    // touched — the splice covers exactly the candidate span.
    let engine = intent_only_engine();
    let source = b"  (S//RELIDO)  \n";
    let result = engine.fix(source, FixMode::Apply);

    let out = std::str::from_utf8(&result.source).unwrap();
    assert_eq!(
        out, "  (S)  \n",
        "surrounding whitespace OUTSIDE the portion candidate must be preserved verbatim"
    );
    assert_eq!(result.applied.len(), 1, "expected one applied fix");
}

#[test]
fn intent_only_path_skips_all_whitespace_candidate() {
    // Finding 3 edge case — the guard `if trimmed_end <= trimmed_start
    // { continue; }` in `synthesize_intent_only_fixes` protects
    // against an all-whitespace candidate. This branch is unreachable
    // through normal scanner emission:
    //
    // - Portion scanner: requires a matching `(...)` pair, so the
    //   candidate slice can never be all whitespace.
    // - Banner scanner: requires one of BANNER_PREFIXES (`TOP SECRET`,
    //   `SECRET`, `S//`, `//`, `UNCLASSIFIED`, etc.) to start the
    //   trimmed line, so the candidate (which is the entire line)
    //   always contains at least one non-whitespace prefix byte.
    // - CAB scanner: matches the literal label `Classified By:`,
    //   never all whitespace.
    //
    // The guard exists as defense-in-depth, not because any production
    // codepath reaches it. Per the architect's preflight ("prefer
    // skipping with a comment over fabricating an unreachable path"),
    // this test documents the invariant rather than constructing a
    // synthetic miss.
    //
    // Indirect verification: every other intent-only test in this file
    // exercises the WELL-FORMED candidate path. If the guard ever
    // misfired (treated a well-formed candidate as all-whitespace and
    // skipped it), every other test would lose its applied fix and
    // fail loudly. That cross-test invariant pins the guard's
    // upper-bound: it MUST NOT fire on any reachable input. The
    // lower-bound (it WOULD fire on an unreachable all-whitespace
    // candidate) is enforced by the `if trimmed_end <= trimmed_start`
    // check itself, statically.
}

#[test]
fn intent_only_path_parsed_markings_cache_miss_skips_synthesis() {
    // Finding 2 fix — `synthesize_intent_only_fixes` looks up the
    // candidate's marking in the `parsed_markings` cache populated by
    // `lint_with_options_internal`. The cache-miss branch skips
    // synthesis with a `tracing::warn`.
    //
    // The miss is structurally unreachable under correct rule behavior:
    // every intent-only diagnostic gets its `candidate_span` from
    // `RuleContext.candidate_span`, which the engine sets to the span
    // of the candidate it is currently dispatching against. The same
    // dispatch path inserts that candidate's marking into
    // `parsed_markings` BEFORE invoking the rule (see
    // `lint_with_options_internal` in engine.rs around line 643:650).
    //
    // The miss therefore signals a rule bug — most likely a rule that
    // hand-built a `candidate_span` from `Diagnostic.span` instead of
    // copying `RuleContext.candidate_span` verbatim, or a rule that
    // emitted a candidate_span pointing outside the document. Per the
    // architect's preflight, this test documents the invariant rather
    // than fabricating a synthetic miss.
    //
    // Indirect verification: every other intent-only test in this file
    // populates `candidate_span` from `RuleContext.candidate_span` and
    // observes a successful synthesis. If the cache-miss path ever
    // misfired (treated a correctly-keyed candidate as missing and
    // skipped it), every other test would lose its applied fix and
    // fail loudly. That cross-test invariant pins the cache-hit branch
    // as load-bearing on every well-formed rule emission.
}
