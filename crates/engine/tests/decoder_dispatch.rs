// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! End-to-end tests for the engine's strict↔decoder dispatch.
//!
//! Unit-level decoder correctness tests live in
//! `crates/engine/src/decoder.rs::tests`; these tests exercise the
//! dispatch layer — the default `StrictOrDecoderRecognizer` installed
//! by [`Engine::new`], the explicit-strict opt-out via
//! [`Engine::with_recognizer`], and the FR-015 zero-candidate path
//! through the full `Engine::lint` pipeline.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::{DecoderRecognizer, Engine, StrictOrDecoderRecognizer, StrictRecognizer};
use marque_rules::RuleSet;
use marque_scheme::recognizer::Recognizer;
use std::sync::Arc;

fn build_engine() -> Engine {
    let config = Config::default();
    let rule_sets: Vec<Box<dyn RuleSet>> = vec![Box::new(CapcoRuleSet::new())];
    Engine::new(config, rule_sets, CapcoScheme::new())
        .expect("engine construction should succeed on a stock CAPCO scheme")
}

fn build_strict_engine() -> Engine {
    build_engine().with_recognizer(Arc::new(StrictRecognizer::new()))
}

#[test]
fn strict_recognizer_alone_is_send_sync() {
    fn assert_send_sync<T: Send + Sync + ?Sized>() {}
    assert_send_sync::<StrictRecognizer>();
    assert_send_sync::<DecoderRecognizer>();
    assert_send_sync::<StrictOrDecoderRecognizer>();
    assert_send_sync::<std::sync::Arc<dyn Recognizer<CapcoScheme>>>();
}

// ---------------------------------------------------------------------------
// Strict-only opt-out: explicit StrictRecognizer suppresses decoder.
// ---------------------------------------------------------------------------

#[test]
fn explicit_strict_recognizer_never_invokes_the_decoder() {
    // `(SERCET//NOFORN)` is a scanner-detectable portion candidate
    // (leading `(` triggers the portion path) carrying the classic
    // SERCET typo. Under the default dispatcher the decoder resolves
    // it to `SECRET//NOFORN` and auto-applies a
    // `FixSource::DecoderPosterior` fix; with an explicit
    // [`StrictRecognizer`] installed via [`Engine::with_recognizer`]
    // we must NOT see such a fix.
    //
    // Note: a bare `SERCET//NOFORN` banner is NOT scanner-detectable
    // because `marque_core::Scanner::scan_banners` gates on known
    // classification prefixes (TOP SECRET, SECRET, …) or a leading
    // `//`. Using an undetected shape would let this assertion pass
    // vacuously because the engine never reaches the recognizer at
    // all.
    let engine = build_strict_engine();
    let input = b"(SERCET//NOFORN)";
    let result = engine.lint(input);

    let any_decoder_source = result.diagnostics.iter().any(|d| {
        d.fix
            .as_ref()
            .is_some_and(|f| matches!(f.source, marque_rules::FixSource::DecoderPosterior))
    });
    assert!(
        !any_decoder_source,
        "explicit StrictRecognizer must never emit DecoderPosterior fixes; \
         diagnostics = {:?}",
        result.diagnostics,
    );
}

// ---------------------------------------------------------------------------
// FR-015 zero-candidate signal surfaces as diagnostics only, no fix.
// ---------------------------------------------------------------------------

#[test]
fn default_engine_on_unrecognized_bytes_emits_no_decoder_fix() {
    // `//FROBNITZ WIBBLE` is a scanner-detectable banner candidate
    // (leading `//` matches the scanner's prefix list) whose tokens
    // have no vocabulary overlap with any CAPCO CVE. The dispatcher
    // runs; the decoder's candidate set stays empty (FR-015:
    // zero-candidate is the "we see signal, can't resolve" signal).
    // No `DecoderPosterior` fix should appear.
    let engine = build_engine();
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
// Mangled-input regression guard.
// ---------------------------------------------------------------------------

#[test]
fn default_engine_dispatcher_actually_reaches_the_decoder_on_mangled_input() {
    // The critical regression-guard for the strict→decoder fallback:
    // the `marque_core::Parser` is lenient enough to return
    // `Ok(empty IsmAttributes)` for shapes like `(SERCET//NOFORN)`
    // where no CVE tokens are recognized. The dispatcher must treat
    // such a trivial strict result as equivalent to zero-candidate
    // and fall through to the decoder — otherwise the engine would
    // install a dormant decoder that never runs on exactly the
    // inputs it exists to recover.
    //
    // Scanner-detectable shape: leading `(` triggers the portion
    // scanner unconditionally, so the recognizer is actually reached.
    let default_engine = build_engine();
    let strict_engine = build_strict_engine();

    let input = b"(SERCET//NOFORN)";
    let default_result = default_engine.lint(input);
    let strict_result = strict_engine.lint(input);

    // On the strict-only engine the decoder never runs, so the
    // diagnostic set is whatever the lenient strict parser + rules
    // produce. On the default engine the decoder should ADD
    // information: at minimum, the two diagnostic sets should not
    // be identical, or the default engine should surface a
    // decoder-sourced marking somewhere. If both engines produce
    // the same output, the dispatcher is dormant.
    let strict_ids: Vec<&str> = strict_result
        .diagnostics
        .iter()
        .map(|d| d.rule.as_str())
        .collect();
    let default_ids: Vec<&str> = default_result
        .diagnostics
        .iter()
        .map(|d| d.rule.as_str())
        .collect();
    assert!(
        strict_ids != default_ids
            || strict_result.diagnostics.len() != default_result.diagnostics.len(),
        "default engine dispatcher should produce observably different \
         output from strict-only on `(SERCET//NOFORN)`; if both produce \
         the same diagnostics the dispatcher is dormant. \
         strict_ids = {strict_ids:?}, default_ids = {default_ids:?}"
    );
}

#[test]
fn default_engine_does_not_change_canonical_input_diagnostics() {
    // A canonical portion marking hits the strict path unambiguously.
    // The decoder fallback must not alter the diagnostic set on such
    // input — the strict result takes precedence over any decoder
    // output because `StrictOrDecoderRecognizer::recognize` returns
    // the strict result on `Parsed::Unambiguous` without calling the
    // decoder.
    let strict_engine = build_strict_engine();
    let default_engine = build_engine();

    let input = b"(S//NF)";
    let strict_diag = strict_engine.lint(input);
    let default_diag = default_engine.lint(input);

    let strict_ids: Vec<_> = strict_diag
        .diagnostics
        .iter()
        .map(|d| d.rule.as_str().to_owned())
        .collect();
    let default_ids: Vec<_> = default_diag
        .diagnostics
        .iter()
        .map(|d| d.rule.as_str().to_owned())
        .collect();
    assert_eq!(
        strict_ids, default_ids,
        "canonical input must produce identical diagnostics under \
         the default dispatcher and an explicit StrictRecognizer"
    );
}
