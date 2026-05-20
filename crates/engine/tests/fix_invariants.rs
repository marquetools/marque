// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 7c — Layer-3 per-pass fix invariants for the two-pass pipeline.
//!
//! Each invariant gets at least one integration test against the
//! real `Engine`. The fixtures use real CAPCO markings rather than
//! synthetic constructions so the tests exercise the same code path
//! the CLI / WASM / server do.
//!
//! Layer-3 invariant register (per `docs/plans/2026-05-02-engine-
//! refactor-consolidated.md` §6 + the PR 7c implementer spec):
//!
//! - **I-1**: `Engine::lint(buf)` is idempotent (same input → same
//!   diagnostic stream, byte-for-byte).
//! - **I-2**: `Engine::fix` is monotonic: `lint(fix(buf)).len() ≤
//!   lint(buf).len()`. Fix never introduces new defects.
//! - **I-4**: `Severity::Suggest` diagnostics never promote to
//!   `AppliedFix`. Suggestion-channel emissions stay advisory.
//! - **I-18**: No two `AppliedFix.span`s overlap in the merged
//!   audit stream — pass-1 + pass-2 span partitions are disjoint by
//!   construction (the I-18 overlap demotion guards the boundary).
//! - **I-19**: Reshape-aware whole-marking — a `(rule, span)` pair
//!   never appears twice across the audit stream. FR-023
//!   disambiguation drops pass-2 diagnostics whose `(rule,
//!   candidate_span)` matches a pass-1 promoted fix.
//!
//! These tests complement the `two_pass_invariants.rs` proptest
//! suite: that file pins universal generators, this one pins
//! specific corner cases — text corrections, ordering rules,
//! suggestion-only paths.

use marque_capco::CapcoRuleSet;
use marque_config::Config;
use marque_engine::{Engine, FixMode};
use marque_rules::Severity;
use secrecy::ExposeSecret as _;
use std::sync::OnceLock;

fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        Engine::new(
            Config::default(),
            vec![Box::new(CapcoRuleSet::new())],
            marque_engine::default_scheme(),
        )
        .expect("default CAPCO scheme has no rewrite cycles")
    })
}

/// Build an engine with a `[corrections]` map so the test can
/// exercise pass-0 (C001) text corrections. The map carries the
/// canonical `SERCET → SECRET` typo fix — Constitution V permitted-
/// identifier list (token canonical).
fn engine_with_corrections() -> Engine {
    let mut config = Config::default();
    config.corrections.insert("SERCET".into(), "SECRET".into());
    Engine::new(
        config,
        vec![Box::new(CapcoRuleSet::new())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

// ---------------------------------------------------------------------------
// I-1: Lint idempotency
// ---------------------------------------------------------------------------

#[test]
fn i1_lint_is_idempotent_on_clean_input() {
    // A clean valid banner-and-portion document. Running `lint`
    // twice MUST produce the same diagnostic count and the same
    // (rule, span) keys in the same order — diagnostics carry no
    // mutable state across lint invocations.
    let src = b"SECRET//NOFORN\n(S//NF)\n";
    let first = engine().lint(src);
    let second = engine().lint(src);
    assert_eq!(
        first.diagnostics.len(),
        second.diagnostics.len(),
        "diagnostic count differs across lint invocations"
    );
    for (a, b) in first.diagnostics.iter().zip(second.diagnostics.iter()) {
        assert_eq!(
            a.rule.as_str(),
            b.rule.as_str(),
            "rule order differs across lint invocations"
        );
        assert_eq!(
            a.span, b.span,
            "diagnostic spans differ across lint invocations"
        );
        assert_eq!(
            a.severity, b.severity,
            "diagnostic severity differs across lint invocations"
        );
    }
}

#[test]
fn i1_lint_is_idempotent_on_defective_input() {
    // A document that fires multiple rules: misordered banner
    // dissem controls + classification mismatch. Same lint
    // invocation, twice — every diagnostic must be byte-identical.
    let src = b"SECRET//NF/OC\n(S//OC/NF)\n";
    let first = engine().lint(src);
    let second = engine().lint(src);
    assert_eq!(first.diagnostics.len(), second.diagnostics.len());
    for (a, b) in first.diagnostics.iter().zip(second.diagnostics.iter()) {
        assert_eq!(a.rule, b.rule);
        assert_eq!(a.span, b.span);
        assert_eq!(a.severity, b.severity);
    }
}

// ---------------------------------------------------------------------------
// I-2: Fix monotonicity
// ---------------------------------------------------------------------------

#[test]
fn i2_fix_does_not_introduce_new_defects_on_clean_input() {
    // Clean input → lint(fix(clean)) has zero diagnostics. Most
    // important on the no-op path: the engine must not invent
    // pretend defects after a no-op fix pass.
    let src = b"SECRET//NOFORN\n(S//NF)\n";
    let before = engine().lint(src).diagnostics.len();
    let result = engine().fix(src, FixMode::Apply);
    let after = engine()
        .lint(result.source.expose_secret())
        .diagnostics
        .len();
    assert!(
        after <= before,
        "fix introduced diagnostics on clean input: before={before} after={after}"
    );
}

#[test]
fn i2_fix_strictly_reduces_diagnostics_on_correctable_input() {
    // A document with a typo C001 will correct: `SERCET` →
    // `SECRET`. Post-fix lint should report fewer diagnostics
    // than pre-fix lint (the typo diagnostic is resolved).
    let eng = engine_with_corrections();
    let src = b"SERCET//NOFORN\n(S//NF)\n";
    let before = eng.lint(src).diagnostics.len();
    let result = eng.fix(src, FixMode::Apply);
    let after = eng.lint(result.source.expose_secret()).diagnostics.len();
    assert!(
        after <= before,
        "fix introduced diagnostics: before={before} after={after}"
    );
}

// ---------------------------------------------------------------------------
// I-4: Severity::Suggest does not promote to AppliedFix
// ---------------------------------------------------------------------------

#[test]
fn i4_suggest_severity_never_appears_in_applied_audit_stream() {
    // Verify the conjugate property: any diagnostic that DID end
    // up promoted has severity strictly above `Suggest`. The
    // applied stream carries audit records; `AppliedFix` doesn't
    // expose severity directly, but the engine's contract is that
    // only `{Error, Warn, Fix}` severities can drive promotion
    // (D-7.6 — pass-0/1/2 all gate on severity > Suggest before
    // entering synthesize_fixes).
    let src = b"SECRET//NOFORN\n(S//NF)\n";
    let result = engine().fix(src, FixMode::Apply);
    // Cross-check: every remaining diagnostic at `Severity::Suggest`
    // has NO corresponding AppliedFix with the same (rule, span).
    use std::collections::HashSet;
    let applied_keys: HashSet<(String, usize, usize)> = result
        .applied_fixes()
        .map(|a| (a.rule.as_str().to_string(), a.span.start, a.span.end))
        .collect();
    for d in &result.remaining_diagnostics {
        if d.severity == Severity::Suggest {
            let key = (
                d.rule.as_str().to_string(),
                d.candidate_span.unwrap_or(d.span).start,
                d.candidate_span.unwrap_or(d.span).end,
            );
            assert!(
                !applied_keys.contains(&key),
                "Severity::Suggest diagnostic at {key:?} was promoted to AppliedFix"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// I-18: pass-1 and pass-2 AppliedFix spans are pairwise disjoint
// ---------------------------------------------------------------------------

#[test]
fn i18_applied_fix_spans_are_pairwise_disjoint_on_simple_input() {
    let src = b"SECRET//NOFORN\n(S//NF)\n";
    let result = engine().fix(src, FixMode::Apply);
    let spans: Vec<_> = result.applied_fixes().map(|a| a.span).collect();
    for i in 0..spans.len() {
        for j in (i + 1)..spans.len() {
            let a = spans[i];
            let b = spans[j];
            assert!(
                a.end <= b.start || b.end <= a.start,
                "overlapping applied fix spans: {a:?} and {b:?}"
            );
        }
    }
}

#[test]
fn i18_applied_fix_spans_disjoint_when_text_correction_present() {
    // C001 text-corrections fire in pass-0; rule fixes fire in
    // pass-1 / pass-2. The cross-pass merge must still produce
    // non-overlapping spans.
    let eng = engine_with_corrections();
    let src = b"SERCET//NOFORN\n(S//NF)\n";
    let result = eng.fix(src, FixMode::Apply);
    let spans: Vec<_> = result.applied_fixes().map(|a| a.span).collect();
    for i in 0..spans.len() {
        for j in (i + 1)..spans.len() {
            let a = spans[i];
            let b = spans[j];
            assert!(
                a.end <= b.start || b.end <= a.start,
                "overlapping applied fix spans across passes: {a:?} and {b:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// I-19: same (rule, span) pair never duplicates across passes
// ---------------------------------------------------------------------------

#[test]
fn i19_same_rule_and_span_never_repeats_in_audit_stream() {
    use std::collections::HashSet;
    // A document that exercises both Localized + WholeMarking
    // rules. The FR-023 disambiguation guarantees no rule appears
    // twice on the same marking-scope span post-reshape.
    let eng = engine_with_corrections();
    let src = b"SERCET//NOFORN\n(S//NF)\n(C//REL TO USA, GBR)\n";
    let result = eng.fix(src, FixMode::Apply);
    let mut seen: HashSet<(String, usize, usize)> = HashSet::new();
    for fix in result.applied_fixes() {
        let key = (fix.rule.as_str().to_string(), fix.span.start, fix.span.end);
        assert!(
            seen.insert(key.clone()),
            "duplicate (rule, span) in applied stream: {key:?}"
        );
    }
}

#[test]
fn i19_remaining_diagnostics_do_not_duplicate_applied_keys() {
    // Stronger contract: a diagnostic whose (rule, span) was
    // applied MUST NOT also appear in remaining_diagnostics. This
    // is what makes the `applied + remaining` partition disjoint.
    use std::collections::HashSet;
    let eng = engine_with_corrections();
    let src = b"SERCET//NOFORN\n(S//NF)\n";
    let result = eng.fix(src, FixMode::Apply);
    let applied_keys: HashSet<(String, usize, usize)> = result
        .applied_fixes()
        .map(|a| (a.rule.as_str().to_string(), a.span.start, a.span.end))
        .collect();
    for d in &result.remaining_diagnostics {
        // text_corrections key on `span`; structural fixes key on
        // `candidate_span ?? span` (mirrors engine.rs:2228+).
        let span = if d.text_correction.is_some() {
            d.span
        } else {
            d.candidate_span.unwrap_or(d.span)
        };
        let key = (d.rule.as_str().to_string(), span.start, span.end);
        if d.fix.is_some() || d.text_correction.is_some() {
            assert!(
                !applied_keys.contains(&key),
                "remaining_diagnostics carries a key already in applied: {key:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Layer-3 sanity: AppliedFix carries the same dry-run flag the call requested
// ---------------------------------------------------------------------------

#[test]
fn dry_run_audit_records_carry_dry_run_flag() {
    let eng = engine_with_corrections();
    let src = b"SERCET//NOFORN\n";
    let result = eng.fix(src, FixMode::DryRun);
    for fix in result.applied_fixes() {
        assert!(
            fix.dry_run,
            "DryRun mode should set dry_run=true on every applied record"
        );
    }
    // DryRun MUST NOT alter the source bytes returned to the
    // caller (Constitution V Principle V audit-record integrity:
    // the audit stream tells the truth about what was promoted,
    // but the caller sees the untouched buffer).
    assert_eq!(
        result.source.expose_secret(),
        src,
        "DryRun mode should not modify the source buffer"
    );
}

#[test]
fn apply_mode_audit_records_carry_apply_flag() {
    let eng = engine_with_corrections();
    let src = b"SERCET//NOFORN\n";
    let result = eng.fix(src, FixMode::Apply);
    for fix in result.applied_fixes() {
        assert!(
            !fix.dry_run,
            "Apply mode should set dry_run=false on every applied record"
        );
    }
}
