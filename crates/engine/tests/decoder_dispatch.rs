// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! End-to-end tests for Phase 4 PR-3's strict↔decoder dispatch.
//!
//! Unit-level decoder correctness tests live in
//! `crates/engine/src/decoder.rs::tests`; these tests exercise the
//! dispatch layer — `Engine::with_deep_scan`, the `StrictRecognizer`
//! / `StrictOrDecoderRecognizer` swap, and the FR-015 zero-candidate
//! path through the full `Engine::lint` pipeline.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::{DecoderRecognizer, Engine, StrictOrDecoderRecognizer, StrictRecognizer};
use marque_rules::RuleSet;
use marque_scheme::recognizer::Recognizer;

fn build_engine(deep_scan: bool) -> Engine {
    let config = Config::default();
    let rule_sets: Vec<Box<dyn RuleSet>> = vec![Box::new(CapcoRuleSet::new())];
    let engine = Engine::new(config, rule_sets, CapcoScheme::new())
        .expect("engine construction should succeed on a stock CAPCO scheme");
    if deep_scan {
        engine.with_deep_scan()
    } else {
        engine
    }
}

#[test]
fn engine_default_mode_is_strict_only() {
    let engine = build_engine(false);
    assert!(
        !engine.deep_scan_enabled(),
        "default engine mode must be strict-only (SC-001)"
    );
}

#[test]
fn with_deep_scan_flips_the_mode_flag() {
    let engine = build_engine(true);
    assert!(
        engine.deep_scan_enabled(),
        "with_deep_scan() must flip the mode flag so downstream \
         callers can mirror the engine's mode"
    );
}

#[test]
fn strict_recognizer_alone_is_send_sync() {
    // Companion to PR-2's `StrictRecognizer` Send+Sync check — covers
    // the concrete types PR-3 adds alongside the strict recognizer.
    fn assert_send_sync<T: Send + Sync + ?Sized>() {}
    assert_send_sync::<StrictRecognizer>();
    assert_send_sync::<DecoderRecognizer>();
    assert_send_sync::<StrictOrDecoderRecognizer>();
    assert_send_sync::<std::sync::Arc<dyn Recognizer<CapcoScheme>>>();
}

// ---------------------------------------------------------------------------
// T048 — interactive-authoring latency envelope: no deep-scan opt-in,
// the decoder never fires.
// ---------------------------------------------------------------------------

#[test]
fn lint_without_deep_scan_never_invokes_the_decoder() {
    // `SERCET//NOFORN` is the classic typo input — strict parse
    // reports it as an unknown-token diagnostic, but the decoder
    // (if wired) would resolve it to `SECRET//NOFORN` and
    // auto-apply a `FixSource::DecoderPosterior` fix. Without
    // deep-scan, we must NOT see such a fix: the strict path
    // produces a diagnostic, nothing more.
    let engine = build_engine(false);
    let input = b"SERCET//NOFORN";
    let result = engine.lint(input);

    let any_decoder_source = result.diagnostics.iter().any(|d| {
        d.fix
            .as_ref()
            .is_some_and(|f| matches!(f.source, marque_rules::FixSource::DecoderPosterior))
    });
    assert!(
        !any_decoder_source,
        "non-deep-scan engine must never emit DecoderPosterior fixes; diagnostics = {:?}",
        result.diagnostics,
    );
}

// ---------------------------------------------------------------------------
// T044 — zero-candidate signal surfaces as diagnostics only, no fix.
// ---------------------------------------------------------------------------

#[test]
fn deep_scan_on_unrecognized_bytes_emits_no_decoder_fix() {
    // `FROBNITZ//WIBBLE` has no vocabulary overlap with any CAPCO
    // token — the decoder's candidate set must stay empty
    // (FR-015: zero-candidate is the "we see signal, can't resolve"
    // signal). No `DecoderPosterior` fix should appear.
    let engine = build_engine(true);
    let result = engine.lint(b"FROBNITZ//WIBBLE");

    let any_decoder_fix = result.diagnostics.iter().any(|d| {
        d.fix
            .as_ref()
            .is_some_and(|f| matches!(f.source, marque_rules::FixSource::DecoderPosterior))
    });
    assert!(
        !any_decoder_fix,
        "decoder must not fabricate a fix for unrecognized bytes; \
         diagnostics = {:?}",
        result.diagnostics,
    );
}

// ---------------------------------------------------------------------------
// Canonical-input regression: strict-path behavior is unchanged when
// deep-scan is flipped on but the input is already canonical.
// ---------------------------------------------------------------------------

#[test]
fn deep_scan_does_not_change_canonical_input_diagnostics() {
    // A canonical portion marking hits the strict path unambiguously.
    // Turning deep-scan on must not alter the diagnostic set — the
    // strict result takes precedence over any decoder output
    // because `StrictOrDecoderRecognizer::recognize` returns the
    // strict result on `Parsed::Unambiguous` without calling the
    // decoder.
    let strict_engine = build_engine(false);
    let deep_engine = build_engine(true);

    let input = b"(S//NF)";
    let strict_diag = strict_engine.lint(input);
    let deep_diag = deep_engine.lint(input);

    // Same number of diagnostics, same IDs — the canonical input
    // doesn't surface any decoder-specific output.
    let strict_ids: Vec<_> = strict_diag
        .diagnostics
        .iter()
        .map(|d| d.rule.as_str().to_owned())
        .collect();
    let deep_ids: Vec<_> = deep_diag
        .diagnostics
        .iter()
        .map(|d| d.rule.as_str().to_owned())
        .collect();
    assert_eq!(
        strict_ids, deep_ids,
        "canonical input must produce identical diagnostics in strict \
         and deep-scan modes"
    );
}
