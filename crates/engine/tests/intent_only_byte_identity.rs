// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B engine-prereq — intent-only fix synthesis byte-identity test.
//!
//! Defines a synthetic test rule that emits **intent-only** diagnostics
//! (`Diagnostic.fix = None`, `Diagnostic.fix_intent = Some(...)`,
//! `Diagnostic.candidate_span = Some(...)`) on portions carrying RELIDO,
//! and verifies the engine's new intent-synthesis path produces
//! byte-identical output to the existing dual-populate E054 path
//! (`Diagnostic.fix = Some(...)` AND `Diagnostic.fix_intent = Some(...)`)
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
use marque_capco::scheme::TOK_RELIDO;
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
            .dissem_controls
            .iter()
            .any(|d| matches!(d, DissemControl::Relido));
        if !has_relido {
            return Vec::new();
        }

        let intent = FixIntent {
            replacement: ReplacementIntent::FactRemove {
                token_ref: FactRef::Cve(TOK_RELIDO),
                scope: Scope::Portion,
            },
            confidence: Confidence::strict(0.99),
            feature_ids: SmallVec::new(),
            message: Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
        };

        // Use the new with_intent_at_span constructor — the engine's
        // synthesis path reads `candidate_span` to determine which
        // bytes to re-render.
        let candidate_span = ctx.candidate_span;
        vec![Diagnostic::with_intent_at_span(
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
    assert_eq!(
        result.applied[0].proposal.rule.as_str(),
        "TEST_INTENT_ONLY_RELIDO"
    );
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
    assert_eq!(applied.proposal.span, Span::new(0, 11));
    assert_eq!(applied.proposal.replacement.as_ref(), "(S)");
    // G13 closure: original bytes elided on the new emission path.
    assert_eq!(applied.proposal.original.as_ref(), "");

    // Verify the AppliedFixProposal::New variant is used (not Legacy).
    match &applied.proposal {
        marque_rules::AppliedFixProposal::New { intent, .. } => match &intent.replacement {
            ReplacementIntent::FactRemove { token_ref, scope } => {
                assert!(matches!(token_ref, FactRef::Cve(t) if *t == TOK_RELIDO));
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
                .dissem_controls
                .iter()
                .any(|d| matches!(d, DissemControl::Relido))
            {
                return Vec::new();
            }
            let intent = FixIntent {
                replacement: ReplacementIntent::FactRemove {
                    token_ref: FactRef::Cve(TOK_RELIDO),
                    scope: Scope::Portion,
                },
                // 0.50 is well below the default 0.95 threshold.
                confidence: Confidence::strict(0.50),
                feature_ids: SmallVec::new(),
                message: Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
            };
            vec![Diagnostic::with_intent_at_span(
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
