// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 7c remediation — integration-level guard for the
//! `PrecedingFixPenalty` audit-provenance path.
//!
//! Background (Rust panel HIGH-1, Code panel MEDIUM-1):
//!
//! When a pass-1 `Phase::Localized` rule produces a `Diagnostic.fix`
//! (a structural `FixIntent`, not a `text_correction`), the engine
//! promotes it via the pass-1 rule channel, the pre-pass-1 attrs
//! cache populates, and pass-2 diagnostics whose `candidate_span`
//! overlaps the cached marking span get `PrecedingFixPenalty`
//! attached. The HIGH-1 finding: the within-group scaling block in
//! `synthesize_fixes` was overwriting the penalty-reduced `rule`
//! axis on multi-diagnostic groups, leaving the audit record
//! claiming a -0.10 delta that the `rule` field no longer
//! reflected (Constitution V Principle V violation).
//!
//! Reachability today:
//!
//! All four currently-registered `Phase::Localized` rules
//! (C001 / E006 / E007 / S004) emit via `Diagnostic::text_correction`,
//! which routes through pass-0 `apply_text_corrections` rather than
//! the pass-1 rule channel. None populate `pass1.applied`, and
//! therefore the pre-pass-1 cache stays empty for every real CAPCO
//! input today. The penalty path is reserved for future
//! `Phase::Localized` rules that emit `Diagnostic.fix` (a
//! structural reshape via `FixIntent`).
//!
//! What this file pins:
//!
//! 1. The negation — no current real input produces a
//!    `PrecedingFixPenalty` entry, AND when it eventually does the
//!    `FeatureContribution.delta` and `Confidence.rule` axis stay
//!    consistent.
//! 2. The reachability watchdog — when a future PR adds a
//!    `Phase::Localized` rule with a `Diagnostic.fix` payload, the
//!    test will see the penalty fire and verify the axis invariant
//!    automatically (no separate "remember to update the test" step).
//!
//! Unit-level coverage of the penalty + scaling math lives inside
//! `engine.rs`'s `#[cfg(test)]` module, where `synthesize_fixes` is
//! directly callable with hand-built multi-diagnostic input. This
//! file provides the integration-level reachability check.

use marque_capco::CapcoRuleSet;
use marque_config::Config;
use marque_engine::{Engine, FixMode};
use marque_rules::confidence::FeatureId;

fn engine_default() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

fn engine_with_corrections() -> Engine {
    let mut config = Config::default();
    // The corrections map drives pre-pass-0 text corrections. These
    // do NOT populate the pre-pass-1 cache (text-correction channel
    // is independent of pass-1's rule channel), so a corrections-
    // driven input is a useful negation case for "the penalty must
    // NOT fire on pass-0 reshapes."
    config.corrections.insert("SERCET".into(), "SECRET".into());
    Engine::new(
        config,
        vec![Box::new(CapcoRuleSet::new())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

fn has_preceding_fix_penalty(features: &[marque_rules::confidence::FeatureContribution]) -> bool {
    features
        .iter()
        .any(|f| matches!(f.id, FeatureId::PrecedingFixPenalty))
}

// ---------------------------------------------------------------------------
// Negation: clean input produces no PrecedingFixPenalty entries
// ---------------------------------------------------------------------------

#[test]
fn no_penalty_appears_in_audit_when_no_pass1_reshape_occurred() {
    // Clean valid banner + portion. Pass-1 produces no applied
    // fixes, so the pre-pass-1 cache is empty and no diagnostic
    // can be marked penalty-eligible. Every `AppliedFix` in this
    // path must have `features` free of `PrecedingFixPenalty`.
    let eng = engine_default();
    let src = b"SECRET//NOFORN\n(S//NF)\n";
    let result = eng.fix(src, FixMode::Apply);
    for fix in &result.applied {
        assert!(
            !has_preceding_fix_penalty(&fix.confidence.features),
            "AppliedFix on clean input for rule {:?} carries an unexpected \
             PrecedingFixPenalty contribution",
            fix.rule.as_str()
        );
    }
}

#[test]
fn no_penalty_appears_in_audit_when_only_text_corrections_reshaped() {
    // Text-corrections route through pass-0 `apply_text_corrections`,
    // NOT through the pass-1 rule channel. The pre-pass-1 cache
    // populates from `pass1.applied` only — so a `SERCET → SECRET`
    // pre-pass-0 reshape must NOT cause downstream pass-2
    // diagnostics to be penalty-tagged. This pins the channel
    // separation.
    let eng = engine_with_corrections();
    let src = b"SERCET//NOFORN\n(S//NF)\n";
    let result = eng.fix(src, FixMode::Apply);
    for fix in &result.applied {
        assert!(
            !has_preceding_fix_penalty(&fix.confidence.features),
            "AppliedFix from a text-correction-driven input ({:?}) carries an \
             unexpected PrecedingFixPenalty contribution; the pass-0 text-
             correction channel must not flow through the pre-pass-1 cache",
            fix.rule.as_str()
        );
    }
}

// ---------------------------------------------------------------------------
// Reachability watchdog + HIGH-1 invariant
// ---------------------------------------------------------------------------

/// Watchdog test: when a future `Phase::Localized` rule starts
/// emitting `Diagnostic.fix` and the penalty path becomes reachable
/// from real CAPCO inputs, this test will start observing
/// `PrecedingFixPenalty` audit records and verify the HIGH-1
/// invariant automatically.
///
/// The sweep input set covers the multi-token banners that are
/// likely candidates for a future structural pass-1 fix
/// (deprecated dissem, X-shorthand date, ordering rewrites). Each
/// `AppliedFix` observed with `PrecedingFixPenalty` in `features`
/// must satisfy:
///
/// - `delta == -0.10` exactly — the engine's
///   `PRECEDING_FIX_PENALTY_DELTA` constant.
/// - `Confidence::rule` axis is in `[0.0, 0.9]` — the penalty is
///   multiplicative `* 0.9` AFTER any within-group scaling step,
///   so the upper bound is `0.9 * 1.0 = 0.9`. The HIGH-1 bug
///   (penalty before scaling) would have left the `rule` axis
///   above 0.9 on the multi-diagnostic group case; the post-fix
///   re-order produces a `rule` axis that always reflects the
///   penalty.
///
/// The test passes trivially today (the loop body never enters
/// because the penalty path is unreachable from real CAPCO rules)
/// but is the only path-coverage guard that activates the moment a
/// pass-1 `Diagnostic.fix` rule lands.
#[test]
fn preceding_fix_penalty_invariant_holds_whenever_path_reachable() {
    let eng = engine_default();
    let inputs: &[&[u8]] = &[
        b"SECRET//REL TO GBR//25X1-\n",
        b"SECRET//REL TO GBR, CAN//25X1-\n",
        b"SECRET//OC/NF\n",
        b"TOP SECRET//REL TO GBR//25X1-\n",
        b"SECRET//50X1-//REL TO GBR\n",
        b"(S//OC/NF)\n",
    ];

    for src in inputs {
        let result = eng.fix(src, FixMode::Apply);
        for fix in &result.applied {
            if !has_preceding_fix_penalty(&fix.confidence.features) {
                continue;
            }
            // Invariant 1: delta is exactly the engine's constant.
            let penalty_entry = fix
                .confidence
                .features
                .iter()
                .find(|f| matches!(f.id, FeatureId::PrecedingFixPenalty))
                .expect("penalty entry filtered above by has_preceding_fix_penalty");
            assert!(
                (penalty_entry.delta - (-0.10_f32)).abs() < 1e-6,
                "PrecedingFixPenalty delta diverged from -0.10: got {} \
                 on fix rule={} span=({},{}) input={:?}",
                penalty_entry.delta,
                fix.rule.as_str(),
                fix.span.start,
                fix.span.end,
                std::str::from_utf8(src).unwrap_or("<non-utf8>")
            );
            // Invariant 2: rule axis ≤ 0.9 — HIGH-1 regression catch.
            // Before the penalty-after-scaling fix, the
            // multi-diagnostic-group path could leave the `rule`
            // axis above 0.9 (scaling overwriting the penalty).
            // The post-fix re-order guarantees the penalty is the
            // last operation, so `rule` is always ≤ 0.9 when the
            // penalty fired.
            assert!(
                fix.confidence.rule >= 0.0 && fix.confidence.rule <= 0.9 + 1e-6,
                "PrecedingFixPenalty fix has rule axis outside [0.0, 0.9]: \
                 rule={} on fix rule={} span=({},{}) input={:?}",
                fix.confidence.rule,
                fix.rule.as_str(),
                fix.span.start,
                fix.span.end,
                std::str::from_utf8(src).unwrap_or("<non-utf8>")
            );
        }
    }
}
