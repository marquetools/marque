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
    // `(SERCET//NOFORN)` is a scanner-detectable portion candidate
    // (leading `(` triggers the portion path) carrying the classic
    // SERCET typo. With deep-scan on the decoder would resolve it
    // to `SECRET//NOFORN` and auto-apply a `FixSource::DecoderPosterior`
    // fix; without deep-scan we must NOT see such a fix.
    //
    // Note: a bare `SERCET//NOFORN` banner is NOT scanner-detectable
    // because `marque_core::Scanner::scan_banners` gates on known
    // classification prefixes (TOP SECRET, SECRET, …) or a leading
    // `//`. Using an undetected shape would let this assertion pass
    // vacuously because the engine never reaches the recognizer at
    // all.
    let engine = build_engine(false);
    let input = b"(SERCET//NOFORN)";
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
    // `//FROBNITZ WIBBLE` is a scanner-detectable banner candidate
    // (leading `//` matches the scanner's prefix list) whose tokens
    // have no vocabulary overlap with any CAPCO CVE. Deep-scan
    // dispatch runs; the decoder's candidate set stays empty
    // (FR-015: zero-candidate is the "we see signal, can't resolve"
    // signal). No `DecoderPosterior` fix should appear.
    let engine = build_engine(true);
    let result = engine.lint(b"//FROBNITZ WIBBLE");

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
fn deep_scan_dispatcher_actually_reaches_the_decoder_on_mangled_input() {
    // The critical regression-guard for the fix to the strict→decoder
    // fallback: the `marque_core::Parser` is lenient enough to return
    // `Ok(empty IsmAttributes)` for shapes like `(SERCET//NOFORN)`
    // where no CVE tokens are recognized. The dispatcher must treat
    // such a trivial strict result as equivalent to zero-candidate
    // and fall through to the decoder — otherwise `with_deep_scan`
    // would install a dormant decoder that never runs on exactly the
    // inputs it exists to recover.
    //
    // Scanner-detectable shape: leading `(` triggers the portion
    // scanner unconditionally, so the recognizer is actually reached.
    let deep_engine = build_engine(true);
    let strict_engine = build_engine(false);

    let input = b"(SERCET//NOFORN)";
    let deep_result = deep_engine.lint(input);
    let strict_result = strict_engine.lint(input);

    // On the strict-only engine the decoder never runs, so the
    // diagnostic set is whatever the lenient strict parser + rules
    // produce. On the deep-scan engine the decoder should ADD
    // information: at minimum, the two diagnostic sets should not
    // be identical, or the deep-scan engine should surface a
    // decoder-sourced marking somewhere. If both engines produce
    // the same output, the dispatcher is dormant and the reviewer's
    // concern has resurfaced.
    //
    // This test is intentionally loose on "what exactly changed"
    // because the end-to-end audit-v2 emission (FixSource::DecoderPosterior
    // on fixes) lands in PR-4. For PR-3 it's sufficient to prove
    // the two modes produce observably different outputs on the
    // canonical mangled input.
    let strict_ids: Vec<&str> = strict_result
        .diagnostics
        .iter()
        .map(|d| d.rule.as_str())
        .collect();
    let deep_ids: Vec<&str> = deep_result
        .diagnostics
        .iter()
        .map(|d| d.rule.as_str())
        .collect();
    assert!(
        strict_ids != deep_ids || strict_result.diagnostics.len() != deep_result.diagnostics.len(),
        "deep-scan dispatcher should produce observably different output \
         from strict-only on `(SERCET//NOFORN)`; if both produce the \
         same diagnostics the dispatcher is dormant (see PR #114 review). \
         strict_ids = {strict_ids:?}, deep_ids = {deep_ids:?}"
    );
}

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
