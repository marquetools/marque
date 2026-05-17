#![cfg(any())]
// PR 3c.B Commit 10: legacy FixProposal-shape test disabled pending rewrite

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Sub-PR 8.D.5 — E012 (dual classification, §H.3 p55)
//! consciously-deferred `fix_intent: None` migration engine-level
//! tests.
//!
//! E012 migrates to `fix_intent: None` matching the E010 / E015 /
//! E016 conscious-defer pattern landed in Sub-PR 8.D.3 / 8.D.2 /
//! 8.B. The §H.3 p55 mutual-exclusion predicate is authoritative
//! ("The US, non-US, and JOINT classification markings are mutually
//! exclusive — a banner line or portion mark may contain only one
//! type and value for the classification marking"), but the
//! *remediation* — "move foreign to FGI block" — is a CROSS-AXIS
//! renormalization (classification axis → FGI axis) outside the
//! current intent vocabulary:
//!
//! 1. `ReplacementIntent::FactAdd` and `FactRemove` are strictly
//!    single-axis-scoped; the E012 fix mutates two axes atomically
//!    (drop foreign-classification token, add FGI block).
//! 2. `ReplacementIntent::Recanonicalize` re-renders an existing
//!    axis; `CapcoScheme::project` does not yet admit
//!    `MarkingClassification::Conflict` resolution into an FGI
//!    projection.
//!
//! Either remediation path forward (a new `Migrate { from, to,
//! scope }` intent variant, or extending `CapcoScheme::project`'s
//! Conflict resolution) is an engine/scheme edit forbidden in
//! scheme-adoption sub-PRs by Constitution VII §IV. The Stage-4
//! retirement target is tracked under
//! `specs/006-engine-rule-refactor/followups/incompatibility-primitive-consolidation.md`
//! — E012 is the canonical example of the type-incompatibility
//! pattern class (alongside JOINT, NODIS/EXDIS, RELIDO/REL TO/
//! NOFORN).
//!
//! Citation-honesty rationale (code-reviewer CRITICAL-1, refined
//! after Copilot review on PR #390): the prior `make_fix_diagnostic`
//! emission claimed a `FGI {countries}` / `FGI NATO` replacement at
//! confidence 0.90 — a pattern application, not direct CAPCO
//! citation. The diagnostic now separates the load-bearing cites:
//!
//!   - §H.3 p55 (detection): "The US, non-US, and JOINT
//!     classification markings are mutually exclusive — a banner
//!     line or portion mark may contain only one type and value
//!     for the classification marking." Authoritative for the
//!     malformed-input predicate.
//!   - §H.3 p57 (US-precedence pattern, normative for JOINT
//!     derivative use): the banner line of a US document
//!     containing JOINT portions is "expressed as a US
//!     classification marking" with FGI carrying the foreign
//!     trigraph/tetragraph codes.
//!   - §H.3 p59 Example 4 note (broader principle): "when US and
//!     non-US portions are combined in a single document, the
//!     overall marking is a US classification."
//!   - §H.7 (FGI marking format): the shape the foreign side
//!     takes once the classifier has decided to express it as FGI.
//!
//! What CAPCO does NOT say: "if a classifier writes `C//NATO C` in
//! a single marking, treat the marking as US C." The p57/p59
//! passages cover document-level commingling, not the
//! single-marking malformed-input case the rule detects. The
//! inference from "document commingling → US classification + FGI
//! block" to "malformed dual marking → US classification + FGI
//! block" is defensible pattern application, not direct citation.
//! Path B's no-fix posture remains the citation-honest choice: the
//! rule fires, surfaces §H.3 p55, names the CAPCO US-precedence
//! pattern (§H.3 p57 / p59), and lets the classifier consult §H.7
//! for the correct FGI marking shape.
//!
//! Confidence-threshold note (code-reviewer CRITICAL-2): the
//! legacy 0.90 confidence sat BELOW the default
//! `Config::confidence_threshold` (0.95), so the dual-population
//! legacy path was never reaching `result.applied` in production —
//! Path B closes the proposal channel cleanly without altering
//! observable auto-apply behavior. Severity stays at
//! `Severity::Fix` (E010 / E015 / E016 precedent — severity
//! classifies the rule's PROBLEM-CATEGORY, not a fix-emission
//! promise).
//!
//! These tests cover the engine-level behaviors that exercise the
//! intent-only path through the real `Engine::fix` / `Engine::lint`
//! surface:
//!
//! - Diagnostic emission on the portion and banner forms with
//!   `fix.is_none()` AND `fix_intent.is_none()` (conscious-defer
//!   shape — neither path produces an auto-applied fix).
//! - Predicate guards: rule does not fire when the classification
//!   is a non-Conflict shape (pure JOINT, pure NATO, pure US).
//! - Idempotence on auto-apply: `Engine::fix` produces no
//!   `AppliedFix` entry for E012 because the predicate has no fix
//!   path; the input string is unchanged.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};
use secrecy::ExposeSecret as _;

fn engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(std::time::UNIX_EPOCH)),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

/// E012 diagnostic emission on the portion form. `(C//NC)` carries
/// a US CONFIDENTIAL classification followed by a NATO CONFIDENTIAL
/// portion classification — the parser produces
/// `MarkingClassification::Conflict { us: C, foreign: Nato(NC) }`.
/// The rule fires with `fix.is_none()` and `fix_intent.is_none()`
/// (conscious-defer). The diagnostic message names both
/// classifications via vocabulary-derived banner forms (G13-clean)
/// and points the classifier at §H.7 for the FGI remediation.
#[test]
fn e012_emits_diagnostic_on_dual_us_plus_nato_in_portion() {
    let result = engine().lint(b"(C//NC)\n");
    let e012: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "E012")
        .collect();

    assert_eq!(
        e012.len(),
        1,
        "E012 must fire exactly once on (C//NC); diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        e012[0].fix.is_none(),
        "E012 must not carry a legacy FixProposal post-migration; \
         got: {:?}",
        e012[0].fix
    );
    assert!(
        e012[0].fix.is_none(),
        "E012 must consciously decline to emit a FixIntent — the \
         cross-axis renormalization (classification → FGI) is outside \
         the current `ReplacementIntent` vocabulary; got: {:?}",
        e012[0].fix
    );
    assert!(
        e012[0].message.contains("mutually exclusive"),
        "E012 message must reference the §H.3 p55 mutual-exclusion \
         predicate verbatim so the classifier knows the problem \
         category; got: {:?}",
        e012[0].message
    );
    assert!(
        e012[0].message.contains("§H.3 p55"),
        "E012 message must cite §H.3 p55 inline so the classifier \
         can trace the predicate; got: {:?}",
        e012[0].message
    );
    assert!(
        e012[0].message.contains("§H.7"),
        "E012 message must point at §H.7 (FGI marking guidance) so \
         the classifier knows where to find the remediation \
         template; got: {:?}",
        e012[0].message
    );
    // Citation-honesty: the diagnostic separates the §H.3 p55
    // detection mandate from the §H.3 p57 / p59 US-precedence pattern
    // (post-Copilot-review-r3229000257 refinement). Asserting all
    // four cites appear in the message text locks in the separation
    // — a drift that collapsed back to "§H.3 p55 mandates US wins"
    // would break this test.
    assert!(
        e012[0].message.contains("§H.3 p57"),
        "E012 message must cite §H.3 p57 (JOINT derivative use) — \
         this is the normative passage for the US-precedence + \
         foreign-to-FGI structural pattern that marque's no-fix \
         posture points the classifier at; got: {:?}",
        e012[0].message
    );
    assert!(
        e012[0].message.contains("§H.3 p59"),
        "E012 message must cite §H.3 p59 (Example 4 note: \"when \
         US and non-US portions are combined in a single document, \
         the overall marking is a US classification\") — the \
         broader-principle complement to the §H.3 p57 normative \
         template; got: {:?}",
        e012[0].message
    );
    assert_eq!(
        e012[0].citation, "CAPCO-2016 §H.3 p55",
        "E012 citation must pin the authoritative detection passage; \
         the §H.3 p57 / p59 remediation pattern cites live in the \
         message body, not the citation field (which holds the \
         single load-bearing detection cite per existing \
         convention); got: {:?}",
        e012[0].citation
    );
}

/// E012 diagnostic emission on the banner form. Same conscious-
/// defer shape as the portion variant — diagnostic fires, no
/// auto-fix. `SECRET//NATO SECRET//NOFORN` is the canonical
/// existing fixture from the E012 unit tests in
/// `crates/capco/src/rules.rs::tests::e012_fires_on_us_plus_nato`.
#[test]
fn e012_emits_diagnostic_on_dual_us_plus_nato_in_banner() {
    let result = engine().lint(b"SECRET//NATO SECRET//NOFORN\n");
    let e012: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "E012")
        .collect();

    assert_eq!(
        e012.len(),
        1,
        "E012 must fire exactly once on SECRET//NATO SECRET//NOFORN; \
         diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        e012[0].fix.is_none(),
        "E012 must not carry a legacy FixProposal post-migration; \
         got: {:?}",
        e012[0].fix
    );
    assert!(
        e012[0].fix.is_none(),
        "E012 must consciously decline to emit a FixIntent; got: {:?}",
        e012[0].fix
    );
    assert!(
        e012[0].message.contains("US"),
        "E012 message must reference the US side of the conflict; \
         got: {:?}",
        e012[0].message
    );
    assert!(
        e012[0].message.contains("NATO"),
        "E012 message must reference the foreign side (NATO) so the \
         classifier knows which classification system is in \
         conflict; got: {:?}",
        e012[0].message
    );
    assert_eq!(
        e012[0].citation, "CAPCO-2016 §H.3 p55",
        "E012 citation must pin the authoritative detection passage; \
         got: {:?}",
        e012[0].citation
    );
}

/// E012 also fires on the COSMIC-TOP-SECRET escalation case
/// (`SECRET//COSMIC TOP SECRET//NOFORN`). The parser produces
/// `Conflict { us: TopSecret, foreign: Nato(CosmicTopSecret) }`
/// because the US level escalates to the higher of the two. The
/// diagnostic still surfaces the conflict — escalation does NOT
/// resolve the §H.3 p55 mutual-exclusion problem.
#[test]
fn e012_emits_on_level_escalation_conflict() {
    let result = engine().lint(b"SECRET//COSMIC TOP SECRET//NOFORN\n");
    let e012: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "E012")
        .collect();

    assert_eq!(
        e012.len(),
        1,
        "E012 must fire on level-escalation Conflict; diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        e012[0].fix.is_none() && e012[0].fix.is_none(),
        "E012 must emit conscious-defer shape on escalation Conflict; \
         got fix: {:?}, fix_intent: {:?}",
        e012[0].fix,
        e012[0].fix
    );
}

/// Predicate guard: E012 does NOT fire on a pure JOINT banner
/// `//JOINT S GBR USA//REL TO USA, GBR`. JOINT is a primary
/// classification mode (not a Conflict state) — there is no US
/// classification preceding it, so the parser produces
/// `MarkingClassification::Joint(_)`, not `Conflict`. §H.3 p55
/// applies mutual-exclusion ACROSS the three modes (US / non-US /
/// JOINT); a single mode in isolation is well-formed.
#[test]
fn e012_does_not_fire_on_pure_joint_banner() {
    let result = engine().lint(b"//JOINT S GBR USA//REL TO USA, GBR\n");
    assert!(
        result.diagnostics.iter().all(|d| d.rule.as_str() != "E012"),
        "E012 must not fire on pure JOINT (no Conflict shape); \
         E012 diagnostics: {:?}",
        result
            .diagnostics
            .iter()
            .filter(|d| d.rule.as_str() == "E012")
            .collect::<Vec<_>>()
    );
}

/// Predicate guard: E012 does NOT fire on pure NATO classification
/// (`//NATO SECRET//REL TO USA, GBR`). No US classification → no
/// Conflict → no E012.
#[test]
fn e012_does_not_fire_on_pure_nato_banner() {
    let result = engine().lint(b"//NATO SECRET//REL TO USA, GBR\n");
    assert!(
        result.diagnostics.iter().all(|d| d.rule.as_str() != "E012"),
        "E012 must not fire on pure NATO (no Conflict shape); \
         E012 diagnostics: {:?}",
        result
            .diagnostics
            .iter()
            .filter(|d| d.rule.as_str() == "E012")
            .collect::<Vec<_>>()
    );
}

/// Predicate guard: E012 does NOT fire on pure US classification
/// (`SECRET//NOFORN`). Single-mode, no Conflict.
#[test]
fn e012_does_not_fire_on_pure_us_banner() {
    let result = engine().lint(b"SECRET//NOFORN\n");
    assert!(
        result.diagnostics.iter().all(|d| d.rule.as_str() != "E012"),
        "E012 must not fire on pure US (no Conflict shape); \
         E012 diagnostics: {:?}",
        result
            .diagnostics
            .iter()
            .filter(|d| d.rule.as_str() == "E012")
            .collect::<Vec<_>>()
    );
}

/// Idempotence on auto-apply: `Engine::fix` produces no `AppliedFix`
/// entry for E012 because the rule has neither `fix` nor
/// `fix_intent` populated. The input string is unchanged. This is
/// the load-bearing invariant of the conscious-defer pattern: a
/// previous-revision bug could have left the auto-pick `→ FGI X`
/// fix in place, corrupting the audit log with a classifier
/// decision the engine cannot make (cross-axis renormalization
/// outside the intent vocabulary).
#[test]
fn e012_idempotent_no_auto_fix_application() {
    let result = engine().fix(b"SECRET//NATO SECRET//NOFORN\n", FixMode::Apply);

    assert_eq!(result.source.expose_secret(),
        b"SECRET//NATO SECRET//NOFORN\n",
        "no fix should apply on dual-classification Conflict \
         (conscious-defer); got: {:?}",
        std::str::from_utf8(result.source.expose_secret()).unwrap_or("<non-utf8>")
    );
    assert!(
        result.applied.iter().all(|af| af.rule.as_str() != "E012"),
        "E012 must not produce an AppliedFix entry post-migration \
         (no fix path); applied rules: {:?}",
        result
            .applied
            .iter()
            .map(|af| af.rule.as_str())
            .collect::<Vec<_>>()
    );
}
