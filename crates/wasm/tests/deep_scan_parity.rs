// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Decoder-fallback parity through the WASM native shims.
//!
//! The decoder is the engine default ([`Engine::new`]); the WASM crate's
//! [`lint_native`] / [`fix_native`] therefore exercise the strict-first /
//! decoder-fallback dispatcher transparently. This file pins two
//! invariants on the WASM shim layer:
//!
//! 1. **Lint NDJSON parity**: a mangled input fed to `lint_native`
//!    produces byte-identical NDJSON to the same input fed to a default
//!    native [`Engine`].
//! 2. **DecoderPosterior surfaces through fix**: `fix_native` on a
//!    mangled input produces at least one `AppliedFix` whose `source`
//!    is `DecoderPosterior`, matching the native engine's output. If
//!    this stops holding, either the dispatcher is dormant or the
//!    WASM build's baked priors / vocab have drifted from the native
//!    build (FR-013a / Gate 1 enforcement).
//!
//! Native-only; cannot run inside `wasm32`.

#![cfg(not(target_arch = "wasm32"))]

use marque_config::Config;
use marque_engine::{Engine, FixMode};
use marque_rules::{AppliedFix, Diagnostic};
use marque_wasm::{fix_native, lint_native};
use serde::Serialize;
use std::sync::OnceLock;

/// Mangled input used across every decoder-fallback parity assertion.
///
/// Leading `(` makes the scanner emit a portion candidate; SERCET is
/// edit-distance-1 from SECRET; NF is the canonical portion-form
/// abbreviation that survives fuzzy correction unchanged. The decoder
/// canonicalizes to `(SECRET//NF)`. The scanner-detectability +
/// guaranteed-decoder-fire combination is documented in
/// `crates/engine/tests/decoder_dispatch.rs::default_engine_dispatcher_actually_reaches_the_decoder_on_mangled_input`.
const MANGLED_INPUT: &[u8] = b"(SERCET//NF)";

fn shared_native_engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        Engine::new(
            Config::default(),
            marque_engine::default_ruleset(),
            marque_engine::default_scheme(),
        )
        .expect("default CAPCO scheme has no rewrite cycles")
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
fn wasm_lint_native_matches_native_engine_on_mangled_input() {
    let engine = shared_native_engine();
    let native_result = engine.lint(MANGLED_INPUT);
    let native_ndjson = render_lint_ndjson(&native_result.diagnostics);

    let wasm_ndjson = lint_native(
        std::str::from_utf8(MANGLED_INPUT).expect("MANGLED_INPUT is valid UTF-8"),
        None,
    )
    .expect("lint_native must succeed on a UTF-8 fixture");

    assert_eq!(
        native_ndjson,
        wasm_ndjson,
        "decoder-fallback parity violated: native engine and WASM \
         lint_native produced different NDJSON for {:?}",
        std::str::from_utf8(MANGLED_INPUT).unwrap()
    );
}

#[test]
fn wasm_fix_native_emits_decoder_audit_record_on_mangled_input() {
    // The load-bearing direction: the WASM regular fix path must
    // surface a `DecoderPosterior` audit record on the canonical
    // mangled input. If this fixture stops triggering the decoder,
    // either the dispatcher is dormant or the WASM build's baked
    // priors / vocab have drifted from the native build.
    let engine = shared_native_engine();
    let native_fix = engine.fix(MANGLED_INPUT, FixMode::Apply);
    let saw_decoder_native = native_fix
        .applied
        .iter()
        .any(|f: &AppliedFix| matches!(f.source, marque_rules::FixSource::DecoderPosterior));
    assert!(
        saw_decoder_native,
        "native engine produced no DecoderPosterior fix on {:?}; \
         the test fixture or decoder dispatcher has regressed",
        std::str::from_utf8(MANGLED_INPUT).unwrap()
    );

    // WASM path: the JSON envelope's `applied` records must include
    // at least one entry whose `source` is `"DecoderPosterior"`.
    let default_threshold = Config::default().confidence_threshold();
    let wasm_json = fix_native(
        std::str::from_utf8(MANGLED_INPUT).expect("MANGLED_INPUT is valid UTF-8"),
        default_threshold,
        None,
    )
    .expect("fix_native must succeed");
    let parsed: serde_json::Value =
        serde_json::from_str(&wasm_json).expect("fix_native output is valid JSON");
    let applied = parsed
        .get("applied")
        .and_then(|v| v.as_array())
        .expect("fix_native output has an `applied` array");
    let saw_decoder_wasm = applied
        .iter()
        .filter_map(|rec| rec.get("source").and_then(|v| v.as_str()))
        .any(|s| s == "DecoderPosterior");
    assert!(
        saw_decoder_wasm,
        "WASM fix_native produced no DecoderPosterior audit record \
         on {:?}; output: {wasm_json}",
        std::str::from_utf8(MANGLED_INPUT).unwrap()
    );
}
