// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T061 — Native-vs-WASM parity test (SC-008).
//!
//! Drives the same inputs through the native `Engine::lint` API and the WASM
//! crate's `lint_native()` wrapper, then asserts byte-equal NDJSON output.
//! Gated to native only — cannot run inside wasm32.

#![cfg(not(target_arch = "wasm32"))]

use marque_config::Config;
use marque_engine::Engine;
use marque_rules::Diagnostic;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Shared engine instance — avoids reconstructing per-fixture (M-3 review fix).
/// Uses `default_ruleset()` to stay synchronized with what `lint_native` uses (M-7).
fn shared_engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| Engine::new(Config::default(), marque_engine::default_ruleset()))
}

// ---------------------------------------------------------------------------
// DiagnosticJson — duplicated from the WASM crate and CLI render.rs.
// This is intentional: the test must independently produce the same shape
// as both the CLI and the WASM crate. If any of the three diverge, SC-008
// parity fails and this test catches it.
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

fn engine_lint_to_ndjson(source: &[u8]) -> String {
    let engine = shared_engine();
    let result = engine.lint(source);
    let mut out = String::new();
    for d in &result.diagnostics {
        let json = serde_json::to_string(&diagnostic_to_json(d)).expect("serialize diagnostic");
        out.push_str(&json);
        out.push('\n');
    }
    out
}

fn corpus_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("../../tests/corpus")
}

fn load_fixture(path: &std::path::Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

fn txt_files_in(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "txt"))
        .map(|e| e.path())
        .collect();
    files.sort();
    files
}

// ---------------------------------------------------------------------------
// Parity: lint
// ---------------------------------------------------------------------------

#[test]
fn lint_parity_invalid_fixtures() {
    let txt_files = txt_files_in(&corpus_dir().join("invalid"));

    assert!(
        !txt_files.is_empty(),
        "T070 requires corpus fixtures, found none"
    );

    for path in &txt_files {
        let source = load_fixture(path);
        let text = std::str::from_utf8(&source)
            .unwrap_or_else(|_| panic!("non-UTF-8 fixture: {}", path.display()));

        let native_ndjson = engine_lint_to_ndjson(&source);
        let wasm_ndjson = marque_wasm::lint_native(text, None)
            .unwrap_or_else(|e| panic!("lint_native failed on {}: {e}", path.display()));

        assert_eq!(
            native_ndjson,
            wasm_ndjson,
            "SC-008 lint parity failure on {}",
            path.file_name().unwrap().to_string_lossy()
        );
    }
}

#[test]
fn lint_parity_valid_fixtures() {
    let txt_files = txt_files_in(&corpus_dir().join("valid"));
    assert!(!txt_files.is_empty(), "T070 requires valid corpus fixtures");

    for path in &txt_files {
        let source = load_fixture(path);
        let text = std::str::from_utf8(&source)
            .unwrap_or_else(|_| panic!("non-UTF-8 fixture: {}", path.display()));

        let native_ndjson = engine_lint_to_ndjson(&source);
        let wasm_ndjson = marque_wasm::lint_native(text, None)
            .unwrap_or_else(|e| panic!("lint_native failed on {}: {e}", path.display()));

        assert_eq!(
            native_ndjson,
            wasm_ndjson,
            "SC-008 lint parity failure on {}",
            path.file_name().unwrap().to_string_lossy()
        );
    }
}

// ---------------------------------------------------------------------------
// Parity: fix (all invalid fixtures, not just 10)
// ---------------------------------------------------------------------------

#[test]
fn fix_parity_invalid_fixtures() {
    let txt_files = txt_files_in(&corpus_dir().join("invalid"));
    assert!(
        !txt_files.is_empty(),
        "T070 requires invalid corpus fixtures"
    );
    let default_threshold = Config::default().confidence_threshold();

    for path in &txt_files {
        let source = load_fixture(path);
        let text = std::str::from_utf8(&source)
            .unwrap_or_else(|_| panic!("non-UTF-8 fixture: {}", path.display()));

        // Run fix through both paths with the same threshold.
        let engine = shared_engine();
        let native_result = engine.fix(source.as_slice(), marque_engine::FixMode::Apply);
        let native_fixed =
            String::from_utf8(native_result.source).expect("native fix produced non-UTF-8");

        let wasm_json = marque_wasm::fix_native(text, default_threshold, None)
            .unwrap_or_else(|e| panic!("fix_native failed on {}: {e}", path.display()));

        let wasm_result: serde_json::Value =
            serde_json::from_str(&wasm_json).expect("fix_native returned invalid JSON");

        let wasm_fixed = wasm_result["fixed_text"]
            .as_str()
            .expect("missing fixed_text in fix output");

        assert_eq!(
            native_fixed,
            wasm_fixed,
            "SC-008 fix parity failure on {}",
            path.file_name().unwrap().to_string_lossy()
        );
    }
}

// ---------------------------------------------------------------------------
// T070: Parity on prose corpus
// ---------------------------------------------------------------------------

#[test]
fn lint_parity_prose_fixtures() {
    let prose_dir = corpus_dir().join("prose");
    assert!(
        prose_dir.is_dir(),
        "tests/corpus/prose/ missing — required for SC-003a parity"
    );
    let txt_files = txt_files_in(&prose_dir);
    assert!(!txt_files.is_empty(), "T070 requires prose corpus fixtures");

    for path in &txt_files {
        let source = load_fixture(path);
        let text = std::str::from_utf8(&source)
            .unwrap_or_else(|_| panic!("non-UTF-8 fixture: {}", path.display()));

        let native_ndjson = engine_lint_to_ndjson(&source);
        let wasm_ndjson = marque_wasm::lint_native(text, None)
            .unwrap_or_else(|e| panic!("lint_native failed on {}: {e}", path.display()));

        assert_eq!(
            native_ndjson,
            wasm_ndjson,
            "SC-008 lint parity failure on prose {}",
            path.file_name().unwrap().to_string_lossy()
        );
    }
}

#[test]
fn fix_parity_valid_fixtures() {
    let txt_files = txt_files_in(&corpus_dir().join("valid"));
    assert!(!txt_files.is_empty(), "T070 requires valid corpus fixtures");
    let default_threshold = Config::default().confidence_threshold();

    for path in &txt_files {
        let source = load_fixture(path);
        let text = std::str::from_utf8(&source)
            .unwrap_or_else(|_| panic!("non-UTF-8 fixture: {}", path.display()));

        let engine = shared_engine();
        let native_result = engine.fix(source.as_slice(), marque_engine::FixMode::Apply);
        let native_fixed =
            String::from_utf8(native_result.source).expect("native fix produced non-UTF-8");

        let wasm_json = marque_wasm::fix_native(text, default_threshold, None)
            .unwrap_or_else(|e| panic!("fix_native failed on {}: {e}", path.display()));

        let wasm_result: serde_json::Value =
            serde_json::from_str(&wasm_json).expect("fix_native returned invalid JSON");

        let wasm_fixed = wasm_result["fixed_text"]
            .as_str()
            .expect("missing fixed_text in fix output");

        assert_eq!(
            native_fixed,
            wasm_fixed,
            "SC-008 fix parity failure on valid {}",
            path.file_name().unwrap().to_string_lossy()
        );
    }
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn lint_empty_input() {
    let result = marque_wasm::lint_native("", None).expect("lint empty");
    assert_eq!(result, "", "empty input should produce no diagnostics");
}

#[test]
fn lint_clean_input() {
    let result = marque_wasm::lint_native("SECRET//NOFORN\n", None).expect("lint clean");
    assert_eq!(result, "", "clean marking should produce no diagnostics");
}

#[test]
fn fix_clean_input_unchanged() {
    let threshold = Config::default().confidence_threshold();
    let result = marque_wasm::fix_native("SECRET//NOFORN\n", threshold, None).expect("fix clean");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(
        parsed["fixed_text"].as_str().unwrap(),
        "SECRET//NOFORN\n",
        "clean input should be unchanged after fix"
    );
}

// ---------------------------------------------------------------------------
// Config passthrough (M-5: test corrections, classifier_id, error cases)
// ---------------------------------------------------------------------------

#[test]
fn lint_with_corrections_config() {
    // Corrections map NF→NOFORN should produce C001 diagnostic.
    let config = r#"{"corrections":{"NF":"NOFORN"}}"#;
    let result = marque_wasm::lint_native("SECRET//NF\n", Some(config.to_owned()))
        .expect("lint with corrections");
    assert!(
        result.contains("\"rule\":\"C001\""),
        "corrections map should trigger C001, got: {result}"
    );
}

#[test]
fn fix_with_corrections_config() {
    let config = r#"{"corrections":{"NF":"NOFORN"}}"#;
    let result = marque_wasm::fix_native("SECRET//NF\n", 0.95, Some(config.to_owned()))
        .expect("fix with corrections");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(
        parsed["fixed_text"].as_str().unwrap(),
        "SECRET//NOFORN\n",
        "corrections should fix NF→NOFORN"
    );
}

#[test]
fn invalid_config_json_returns_error() {
    let result = marque_wasm::lint_native("SECRET//NF\n", Some("not json".to_owned()));
    assert!(result.is_err(), "invalid JSON config should return error");
}

#[test]
fn config_with_invalid_threshold_returns_error() {
    let result = marque_wasm::fix_native("SECRET//NF\n", -0.5, None);
    assert!(result.is_err(), "negative threshold should return error");
}

#[test]
fn config_with_classifier_id() {
    let config = r#"{"classifier_id":"TEST-WASM-42"}"#;
    let result = marque_wasm::fix_native("SECRET//NF\n", 0.95, Some(config.to_owned()))
        .expect("fix with classifier_id");
    assert!(
        result.contains("TEST-WASM-42"),
        "classifier_id should appear in audit records, got: {result}"
    );
}

// ---------------------------------------------------------------------------
// lint_batch
// ---------------------------------------------------------------------------

#[test]
fn lint_batch_returns_results_for_each_entry() {
    let entries = r#"[
        {"id": "a", "text": "SECRET//NF\n"},
        {"id": "b", "text": "SECRET//NOFORN\n"}
    ]"#;
    let result = marque_wasm::lint_batch_native(entries, None).expect("lint_batch");
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap();

    assert_eq!(parsed.len(), 2, "should return one result per entry");
    assert_eq!(parsed[0]["id"], "a");
    assert_eq!(parsed[1]["id"], "b");
    // "a" has NF (abbreviated) → should have diagnostics
    assert!(
        !parsed[0]["diagnostics"].as_array().unwrap().is_empty(),
        "SECRET//NF should produce diagnostics"
    );
    // "b" is clean → empty diagnostics
    assert!(
        parsed[1]["diagnostics"].as_array().unwrap().is_empty(),
        "SECRET//NOFORN should be clean"
    );
}

#[test]
fn lint_batch_empty_array() {
    let result = marque_wasm::lint_batch_native("[]", None).expect("lint_batch empty");
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap();
    assert!(parsed.is_empty());
}

#[test]
fn lint_batch_parity_with_single_lint() {
    // Each batch entry should produce the same diagnostics as a standalone lint call.
    let texts = [
        ("inv1", "SECRET//NF\n"),
        ("inv2", "TOP SECRET//SI//NF\n"),
        ("clean", "SECRET//NOFORN\n"),
    ];

    let entries_json = serde_json::to_string(
        &texts
            .iter()
            .map(|(id, text)| serde_json::json!({"id": id, "text": text}))
            .collect::<Vec<_>>(),
    )
    .unwrap();

    let batch_result = marque_wasm::lint_batch_native(&entries_json, None).expect("batch");
    let batch_parsed: Vec<serde_json::Value> = serde_json::from_str(&batch_result).unwrap();

    for (i, (id, text)) in texts.iter().enumerate() {
        let single_ndjson = marque_wasm::lint_native(text, None).expect("single lint");
        // Parse NDJSON lines into a JSON array for comparison.
        let single_diags: Vec<serde_json::Value> = single_ndjson
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();

        let batch_diags: Vec<serde_json::Value> =
            batch_parsed[i]["diagnostics"].as_array().unwrap().to_vec();

        assert_eq!(
            batch_parsed[i]["id"], *id,
            "batch result {i} should have id={id}"
        );
        assert_eq!(
            single_diags, batch_diags,
            "batch diagnostics for {id} should match single lint"
        );
    }
}

#[test]
fn lint_batch_invalid_json_returns_error() {
    let result = marque_wasm::lint_batch_native("not json", None);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// generate_cab
// ---------------------------------------------------------------------------

#[test]
fn test_generate_cab_basic() {
    let text = "(S//NF) This is secret.\n(TS//SI//REL TO USA, GBR) This is top secret.";
    let cab = marque_wasm::generate_cab_native(text, None, None).expect("generate_cab failed");
    assert!(cab.contains("Classified By: Derivative Classifier"));
    assert!(cab.contains("Derived From: Multiple Sources"));
    assert!(cab.contains("Declassify On:"));
    // Since TS is present, it's definitely classified.
}

#[test]
fn test_generate_cab_with_explicit_declass() {
    let text = "(S//NF//20401231) Portion 1";
    let cab = marque_wasm::generate_cab_native(text, None, None).expect("generate_cab failed");
    assert!(cab.contains("Declassify On: 20401231"));
}

#[test]
fn test_generate_cab_unclassified_empty() {
    let text = "(U) Unclassified portion";
    let cab = marque_wasm::generate_cab_native(text, None, None).expect("generate_cab failed");
    assert_eq!(cab, "");
}

// ---------------------------------------------------------------------------
// compute_banner
// ---------------------------------------------------------------------------

#[test]
fn test_compute_banner_basic() {
    let text = "(S//NF) Portion 1\n(TS//SI//NF) Portion 2";
    let banner = marque_wasm::compute_banner_native(text).expect("compute_banner failed");
    assert_eq!(banner, "TOP SECRET//SI//NOFORN");
}
