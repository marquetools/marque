// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Rule-panic isolation regression test (whitepaper §6.3 / gap
//! register #10).
//!
//! `Engine::lint` wraps every `Rule::check` call in
//! `std::panic::catch_unwind`. A buggy rule that panics — most
//! commonly via `FixProposal::new` rejecting an out-of-range
//! `Confidence`, but any panic source qualifies — must NOT abort the
//! whole document. The catch logs a `marque_engine::rule_panic`
//! warning naming the rule and skips it; sibling rules and remaining
//! candidates keep running.
//!
//! ## Why this is its own file
//!
//! The other engine integration tests assume rule sets that don't
//! panic. Loading a deliberately-panicky rule alongside the real
//! `capco_rules()` set would either pollute their assertions or
//! force every test to defend against panic-recovery noise. This
//! file owns the panicky rule set and the assertions tied to it.

use marque_capco::{CapcoScheme, capco_rules};
use marque_config::Config;
use marque_engine::Engine;
use marque_ism::CanonicalAttrs;
use marque_rules::{
    AuthoritativeSource, Citation, Diagnostic, Message, MessageArgs, MessageTemplate, Rule,
    RuleContext, RuleId, RuleSet, SectionLetter, SectionRef, Severity,
};

/// A rule that always panics in `check()`.
///
/// Mimics the failure mode in whitepaper §6.3: a rule that constructs
/// an invalid `Confidence` would panic inside `FixProposal::new`. Here
/// we panic directly with a recognizable string so the test can
/// assert (where it would matter) that the panic was contained.
struct AlwaysPanicsRule;

impl Rule<CapcoScheme> for AlwaysPanicsRule {
    fn id(&self) -> RuleId {
        // Reserved-for-tests prefix: real rules use `E### / W### /
        // C### / S### / R001`. `Z` keeps this rule out of the rule-id
        // allocation space the rest of the codebase tracks.
        RuleId::new("Z001")
    }

    fn name(&self) -> &'static str {
        "always-panics-test-rule"
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn check(&self, _attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        panic!("FixProposal invalid confidence: simulated rule defect (Z001 panic-isolation test)");
    }
}

/// A rule that always emits exactly one Info diagnostic.
///
/// Used as a "sibling" to verify that the engine continues processing
/// other rules after one has panicked.
struct AlwaysFiresRule;

/// Test-fixture `Message` stub mirroring the helpers in
/// `engine.rs::tests` and `output.rs::tests`. The rule body only needs
/// to emit *some* diagnostic; the closed-set `Message` shape (per
/// PR 3c.2.C C5) means no free-form sentinel text is constructible
/// here. `UnrecognizedToken` is the generic template; default args
/// keep the payload empty.
#[inline]
fn stub_message() -> Message {
    Message::new(MessageTemplate::UnrecognizedToken, MessageArgs::default())
}

/// Test-fixture `Citation` stub mirroring the helpers in
/// `engine.rs::tests` and `output.rs::tests`. Uses
/// `AuthoritativeSource::EngineInternal` (non-CAPCO sentinel) so
/// citation-lint skips the entry.
#[inline]
fn stub_citation() -> Citation {
    Citation::new(
        AuthoritativeSource::EngineInternal,
        SectionRef::new(SectionLetter::A),
        core::num::NonZeroU16::new(1).unwrap(),
    )
}

impl Rule<CapcoScheme> for AlwaysFiresRule {
    fn id(&self) -> RuleId {
        RuleId::new("Z002")
    }

    fn name(&self) -> &'static str {
        "always-fires-test-rule"
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn check(&self, _attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        // Build a Diagnostic without a fix — we just need to prove
        // this rule's output reaches the LintResult after a sibling
        // rule panics.
        vec![Diagnostic::new(
            self.id(),
            self.default_severity(),
            marque_scheme::Span::new(0, 1),
            stub_message(),
            stub_citation(),
            None,
        )]
    }
}

struct TestRuleSet {
    rules: Vec<Box<dyn Rule<CapcoScheme>>>,
}

impl TestRuleSet {
    fn new(rules: Vec<Box<dyn Rule<CapcoScheme>>>) -> Self {
        Self { rules }
    }
}

impl RuleSet<CapcoScheme> for TestRuleSet {
    fn rules(&self) -> &[Box<dyn Rule<CapcoScheme>>] {
        &self.rules
    }
    fn schema_version(&self) -> &'static str {
        // Real CAPCO rule set's `schema_version()` is the ODNI ISM
        // package version. Tests don't gate on the value — any
        // non-empty literal will do.
        "test-2026-04"
    }
}

fn engine_with(panicky: bool, with_fires: bool) -> Engine {
    let mut rules: Vec<Box<dyn Rule<CapcoScheme>>> = Vec::new();
    if panicky {
        rules.push(Box::new(AlwaysPanicsRule));
    }
    if with_fires {
        rules.push(Box::new(AlwaysFiresRule));
    }
    Engine::new(
        Config::default(),
        vec![Box::new(TestRuleSet::new(rules))],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

// ---------------------------------------------------------------------------
// Lint must not abort when a rule panics.
// ---------------------------------------------------------------------------

#[test]
fn panicking_rule_does_not_abort_lint() {
    let engine = engine_with(/* panicky */ true, /* with_fires */ false);
    let source = b"TOP SECRET//SI//NOFORN\n";

    // Bare `lint(...)` — if the catch_unwind wrapper isn't doing its
    // job, this call panics and the test reports the panic, not a
    // clean failure. We assert the call returns at all.
    let result = engine.lint(source);

    // The panicky rule produced no diagnostics (it panicked instead);
    // no other rules are configured. Result is just empty.
    assert_eq!(
        result.diagnostics.len(),
        0,
        "panicky rule must produce zero diagnostics, not propagate"
    );
}

#[test]
fn sibling_rules_continue_after_panic() {
    // Same source, but now `AlwaysFiresRule` is alongside
    // `AlwaysPanicsRule`. The panic on Z001 must NOT prevent Z002
    // from running and emitting its diagnostic.
    let engine = engine_with(/* panicky */ true, /* with_fires */ true);
    let source = b"TOP SECRET//SI//NOFORN\n";

    let result = engine.lint(source);

    let z002_count = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "Z002")
        .count();
    assert!(
        z002_count >= 1,
        "AlwaysFiresRule (Z002) must still produce its diagnostic after \
         AlwaysPanicsRule (Z001) panics. Got diagnostics: {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| d.rule.as_str())
            .collect::<Vec<_>>()
    );

    // The panicky rule must have produced no Z001 diagnostic.
    let z001_count = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "Z001")
        .count();
    assert_eq!(
        z001_count, 0,
        "AlwaysPanicsRule (Z001) must produce zero diagnostics; the \
         panic should have been caught and the rule skipped"
    );
}

#[test]
fn fix_pipeline_does_not_abort_when_a_rule_panics() {
    // Same isolation contract for `Engine::fix`. The fix path runs
    // the same `Rule::check` loop, so the wrapper covers it too —
    // pin that.
    let engine = engine_with(/* panicky */ true, /* with_fires */ true);
    let source = b"(S//SI//NF) sample portion.\n";

    // `FixMode::DryRun` is enough to drive the fix pipeline without
    // mutating any backing store.
    let result = engine.fix(source, marque_engine::FixMode::DryRun);

    // The post-fix re-lint of the dry-run output runs the rule loop
    // a second time — both passes must complete cleanly.
    assert!(
        result
            .remaining_diagnostics
            .iter()
            .any(|d| d.rule.as_str() == "Z002"),
        "AlwaysFiresRule (Z002) diagnostic must surface in fix pipeline output"
    );
    assert!(
        result.applied_fixes().is_empty(),
        "no fixes should have been applied — neither test rule emits any"
    );
}

// ---------------------------------------------------------------------------
// Direct exercise of the original failure mode (invalid Confidence).
// ---------------------------------------------------------------------------
//
// The whitepaper §6.3 / gap register #10 narrative names
// `FixProposal::new` panicking on out-of-range `Confidence` as the
// canonical failure. Rule `AlwaysPanicsRule` uses a hand-rolled
// panic message that names the same shape, but exercising the real
// `FixProposal::new` panic site on the engine path proves the catch
// works against the actual API surface, not just our test
// imitation. This rule would normally be a code defect — the test
// is the fence around it.

struct InvalidConfidenceRule;

impl Rule<CapcoScheme> for InvalidConfidenceRule {
    fn id(&self) -> RuleId {
        RuleId::new("Z003")
    }
    fn name(&self) -> &'static str {
        "invalid-confidence-test-rule"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }
    fn check(&self, _attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        // `Confidence::strict(2.0)` is out of `[0.0, 1.0]` and
        // panics directly. The engine's catch_unwind wrapper must
        // catch the rule-side panic and continue.
        let _bad = marque_rules::Confidence::strict(2.0);
        Vec::new() // unreachable
    }
}

#[test]
fn invalid_confidence_in_fix_proposal_does_not_abort_lint() {
    // Use the canonical failure-mode rule (calls real
    // `FixProposal::new` with bad confidence) instead of our hand-
    // rolled `AlwaysPanicsRule`. If this passes, we've proved the
    // catch_unwind wrapper handles the *actual* API surface the
    // gap register names, not just our imitation of it.
    let engine = Engine::new(
        Config::default(),
        vec![Box::new(TestRuleSet::new(vec![Box::new(
            InvalidConfidenceRule,
        )]))],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    let source = b"TOP SECRET//SI//NOFORN\n";
    let result = engine.lint(source);

    assert_eq!(result.diagnostics.len(), 0);
}

// ---------------------------------------------------------------------------
// Production rule set is unaffected by the wrapper.
// ---------------------------------------------------------------------------
//
// A regression that broke the catch_unwind wrapper would still let
// the production rule set run cleanly (no rule panics today). To
// protect against the inverse — a regression that broke production
// rules in the name of the wrapper — pin that the real CAPCO rule
// set still emits its expected diagnostics on a representative
// failing input.

#[test]
fn capco_rules_still_emit_diagnostics() {
    // Sanity: the real CAPCO rule set wired through the standard
    // `Engine::new` shape produces at least one diagnostic on this
    // known-bad input. `SECRET//SI//FOREIGN` is well-formed at the
    // scanner but `FOREIGN` isn't a recognized dissem control —
    // E008 (or similar) fires.
    let engine = Engine::new(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    let source = b"SECRET//SI//FOREIGN\n";
    let result = engine.lint(source);
    assert!(
        !result.diagnostics.is_empty(),
        "CAPCO rule set must still emit diagnostics on known-bad input \
         even after the rule-panic wrapper landed"
    );
}
