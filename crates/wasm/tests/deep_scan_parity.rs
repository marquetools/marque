// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T067b — WASM deep-scan parity test.
//!
//! SC-008 byte-equal-output parity, extended to the decoder path. The
//! same mangled input fed to a native `Engine::with_deep_scan()` and to
//! the WASM crate's [`lint_deep_scan_native`] / [`fix_deep_scan_native`]
//! must produce byte-identical NDJSON. A divergence here means a WASM
//! caller using the deep-scan exports would see different diagnostics
//! / fixes than a CLI caller running the same input through `marque
//! check --deep-scan` / `marque fix --deep-scan`.
//!
//! Native-only; cannot run inside `wasm32`.

#![cfg(not(target_arch = "wasm32"))]

use marque_config::Config;
use marque_engine::{Engine, FixMode};
use marque_rules::{AppliedFix, Diagnostic};
use marque_wasm::{fix_deep_scan_native, lint_deep_scan_native};
use serde::Serialize;
use std::sync::OnceLock;

/// Mangled input used across every deep-scan parity assertion.
///
/// Leading `(` makes the scanner emit a portion candidate; SERCET is
/// edit-distance-1 from SECRET; NF is the canonical portion-form
/// abbreviation that survives fuzzy correction unchanged. The decoder
/// canonicalizes to `(SECRET//NF)`. The scanner-detectability +
/// guaranteed-decoder-fire combination is documented in
/// `crates/engine/tests/decoder_dispatch.rs::deep_scan_dispatcher_actually_reaches_the_decoder_on_mangled_input`.
const MANGLED_INPUT: &[u8] = b"(SERCET//NF)";

fn shared_native_deep_scan_engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        Engine::new(
            Config::default(),
            marque_engine::default_ruleset(),
            marque_engine::default_scheme(),
        )
        .expect("default CAPCO scheme has no rewrite cycles")
        .with_deep_scan()
    })
}

// ---------------------------------------------------------------------------
// JSON projection — duplicated from the WASM crate to keep the parity
// check independent. If the two diverge in shape, the byte-equal
// assertion catches it.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct DiagnosticJson<'a> {
    rule: &'a str,
    severity: &'a str,
    span: SpanJson,
    message: &'a str,
    citation: &'a str,
    fix: Option<FixJson<'a>>,
}

#[derive(Debug, Serialize)]
struct SpanJson {
    start: usize,
    end: usize,
}

#[derive(Debug, Serialize)]
struct FixJson<'a> {
    source: &'static str,
    replacement: &'a str,
    confidence: f32,
    migration_ref: Option<&'a str>,
}

fn fix_source_str(source: marque_rules::FixSource) -> &'static str {
    match source {
        marque_rules::FixSource::BuiltinRule => "BuiltinRule",
        marque_rules::FixSource::CorrectionsMap => "CorrectionsMap",
        marque_rules::FixSource::MigrationTable => "MigrationTable",
        marque_rules::FixSource::DecoderPosterior => "DecoderPosterior",
        marque_rules::FixSource::DecoderClassificationHeuristic => "DecoderClassificationHeuristic",
    }
}

fn diagnostic_to_json(d: &Diagnostic) -> DiagnosticJson<'_> {
    DiagnosticJson {
        rule: d.rule.as_str(),
        severity: d.severity.as_str(),
        span: SpanJson {
            start: d.span.start,
            end: d.span.end,
        },
        message: d.message.as_ref(),
        citation: d.citation,
        fix: d.fix.as_ref().map(|f| FixJson {
            source: fix_source_str(f.source),
            replacement: f.replacement.as_ref(),
            confidence: f.confidence.combined(),
            migration_ref: f.migration_ref,
        }),
    }
}

fn render_lint_ndjson(diagnostics: &[Diagnostic]) -> String {
    let mut buf = Vec::with_capacity(diagnostics.len() * 256);
    for d in diagnostics {
        serde_json::to_writer(&mut buf, &diagnostic_to_json(d))
            .expect("diagnostic JSON serialization is infallible for the test fixture");
        buf.push(b'\n');
    }
    String::from_utf8(buf).expect("serde_json output is always valid UTF-8")
}

#[test]
fn wasm_deep_scan_lint_matches_native() {
    let engine = shared_native_deep_scan_engine();
    let native_result = engine.lint(MANGLED_INPUT);
    let native_ndjson = render_lint_ndjson(&native_result.diagnostics);

    let wasm_ndjson = lint_deep_scan_native(MANGLED_INPUT)
        .expect("lint_deep_scan_native must succeed on a UTF-8 fixture");

    assert_eq!(
        native_ndjson,
        wasm_ndjson,
        "SC-008 parity violated: native deep-scan lint and WASM \
         lint_deep_scan_native produced different NDJSON for {:?}",
        std::str::from_utf8(MANGLED_INPUT).unwrap()
    );
}

#[test]
fn wasm_deep_scan_fix_emits_decoder_audit_record() {
    // T067b's load-bearing direction: the WASM deep-scan path must
    // surface a `DecoderPosterior` audit record on the canonical
    // mangled input. If this fixture stops triggering the decoder,
    // either the dispatcher is dormant or the WASM build's baked
    // priors / vocab have drifted from the native build (FR-013a /
    // Gate 2 enforcement). Compare strict and deep-scan native
    // outputs to surface drift early.
    let engine = shared_native_deep_scan_engine();
    let native_fix = engine.fix(MANGLED_INPUT, FixMode::Apply);
    let saw_decoder_native = native_fix
        .applied
        .iter()
        .any(|f: &AppliedFix| matches!(f.source, marque_rules::FixSource::DecoderPosterior));
    assert!(
        saw_decoder_native,
        "deep-scan native engine produced no DecoderPosterior fix on {:?}; \
         the test fixture or decoder dispatcher has regressed",
        std::str::from_utf8(MANGLED_INPUT).unwrap()
    );

    // WASM path: the JSON envelope's `applied` records must include
    // at least one entry whose `source` is `"DecoderPosterior"`.
    let wasm_json = fix_deep_scan_native(MANGLED_INPUT).expect("fix_deep_scan_native must succeed");
    let parsed: serde_json::Value =
        serde_json::from_str(&wasm_json).expect("fix_deep_scan_native output is valid JSON");
    let applied = parsed
        .get("applied")
        .and_then(|v| v.as_array())
        .expect("fix_deep_scan_native output has an `applied` array");
    let saw_decoder_wasm = applied
        .iter()
        .filter_map(|rec| rec.get("source").and_then(|v| v.as_str()))
        .any(|s| s == "DecoderPosterior");
    assert!(
        saw_decoder_wasm,
        "WASM fix_deep_scan_native produced no DecoderPosterior audit record \
         on {:?}; output: {wasm_json}",
        std::str::from_utf8(MANGLED_INPUT).unwrap()
    );
}
