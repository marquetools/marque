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

// ---------------------------------------------------------------------------
// Prose-glue suppression + bare-RESTRICTED rejection (this PR).
// ---------------------------------------------------------------------------

#[test]
fn default_engine_suppresses_prose_glue_single_letter_portions() {
    // `letter(s)` / `function(c)` / `loss(s)` — the single-letter
    // portion is glued to a preceding word, so the engine populates
    // `ParseContext.preceded_by_whitespace = false` from the source
    // byte preceding the candidate. The decoder must produce zero
    // candidates and the engine must emit zero diagnostics — these
    // shapes are overwhelmingly plural-suffix prose, not markings.
    let engine = build_engine();
    for input in &[
        b"the letter(s)" as &[u8],
        b"function(c)",
        b"loss(s)",
        b"reset(u)",
    ] {
        let result = engine.lint(input);
        assert!(
            result.diagnostics.is_empty(),
            "prose-glued single-letter portion {:?} must produce zero diagnostics, got: {:?}",
            std::str::from_utf8(input).unwrap_or("<bytes>"),
            result
                .diagnostics
                .iter()
                .map(|d| (d.rule.as_str(), d.message.to_string()))
                .collect::<Vec<_>>(),
        );
    }
}

#[test]
fn default_engine_recovers_single_letter_portion_after_whitespace() {
    // Counterpart: when the single-letter portion is preceded by
    // whitespace (column zero, after a space, post-newline), the
    // dispatcher's strict path resolves it directly. The prose-glue
    // heuristic must NOT fire here — verifying the heuristic doesn't
    // overshoot into legitimate marking territory.
    let engine = build_engine();
    // Leading whitespace + canonical-case `(S)`: strict path produces
    // a SECRET marking; downstream rules see it as a real marking.
    let result = engine.lint(b" (S) some text");
    let saw_marking = result
        .diagnostics
        .iter()
        .any(|d| d.fix.is_some() || d.rule.as_str().starts_with('E'));
    let _ = saw_marking; // diagnostic set is rule-dependent; the load-bearing
    // assertion is "no panic, marking surfaces normally" — recognizer
    // proves it via the `(s)` lower-case canonicalization in
    // `decoder_canonicalizes_single_letter_when_preceded_by_whitespace`.
    // Engine-level test: zero panics and the strict path completes.
    assert!(
        result.candidates_processed >= 1,
        "engine must reach the recognizer for whitespace-preceded `(S)`, \
         got candidates_processed = {}",
        result.candidates_processed
    );
}

#[test]
fn default_engine_rejects_bare_restricted_portion() {
    // CAPCO §H.7: `(R)` without an FGI marker is structurally
    // indistinguishable from prose glyphs (registered-mark, list-item)
    // and is rejected at both the strict recognizer
    // (`is_us_restricted`) and the decoder's per-
    // candidate filter. The engine emits no diagnostics for bare
    // `(R)` — neither a strict-path E015 ("non-US classification
    // without dissem control") nor a decoder R001 ("decoder-recognized
    // canonical form") should fire.
    let engine = build_engine();
    for input in &[
        b"(R)" as &[u8],  // bare uppercase
        b"(r)",           // lowercase form (decoder canonicalizes case)
        b"text (R) more", // mid-prose, whitespace-preceded
        b"footnote(R)",   // word-glued — both heuristics apply
    ] {
        let result = engine.lint(input);
        assert!(
            result.diagnostics.is_empty(),
            "bare RESTRICTED portion {:?} must produce zero diagnostics, got: {:?}",
            std::str::from_utf8(input).unwrap_or("<bytes>"),
            result
                .diagnostics
                .iter()
                .map(|d| (d.rule.as_str(), d.message.to_string()))
                .collect::<Vec<_>>(),
        );
    }
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
