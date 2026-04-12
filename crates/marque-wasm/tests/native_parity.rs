//! T061 — Native-vs-WASM parity test (SC-008).
//!
//! Drives the same inputs through the native `Engine::lint` API and the WASM
//! crate's `lint_native()` wrapper, then asserts byte-equal NDJSON output.
//! Gated to native only — cannot run inside wasm32.

#![cfg(not(target_arch = "wasm32"))]

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::Diagnostic;
use serde::Serialize;
use std::path::PathBuf;

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
            confidence: f.confidence,
            migration_ref: f.migration_ref,
        }),
    }
}

fn engine_lint_to_ndjson(source: &[u8]) -> String {
    let engine = Engine::new(Config::default(), vec![Box::new(capco_rules())]);
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

// ---------------------------------------------------------------------------
// Parity: lint
// ---------------------------------------------------------------------------

#[test]
fn lint_parity_invalid_fixtures() {
    let invalid_dir = corpus_dir().join("invalid");
    let mut txt_files: Vec<_> = std::fs::read_dir(&invalid_dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", invalid_dir.display()))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "txt"))
        .map(|e| e.path())
        .collect();
    txt_files.sort();

    assert!(
        txt_files.len() >= 10,
        "T061 requires ≥10 corpus fixtures, found {}",
        txt_files.len()
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
    let valid_dir = corpus_dir().join("valid");
    let mut txt_files: Vec<_> = std::fs::read_dir(&valid_dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", valid_dir.display()))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "txt"))
        .map(|e| e.path())
        .collect();
    txt_files.sort();

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
// Parity: fix
// ---------------------------------------------------------------------------

#[test]
fn fix_parity_invalid_fixtures() {
    let invalid_dir = corpus_dir().join("invalid");
    let mut txt_files: Vec<_> = std::fs::read_dir(&invalid_dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", invalid_dir.display()))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "txt"))
        .map(|e| e.path())
        .collect();
    txt_files.sort();

    for path in txt_files.iter().take(10) {
        let source = load_fixture(path);
        let text = std::str::from_utf8(&source)
            .unwrap_or_else(|_| panic!("non-UTF-8 fixture: {}", path.display()));

        // Run fix through both paths.
        let engine = Engine::new(Config::default(), vec![Box::new(capco_rules())]);
        let native_result = engine.fix(source.as_slice(), marque_engine::FixMode::Apply);
        let native_fixed =
            String::from_utf8(native_result.source).expect("native fix produced non-UTF-8");

        let wasm_json = marque_wasm::fix_native(text, 0.95, None)
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
    let result = marque_wasm::fix_native("SECRET//NOFORN\n", 0.95, None).expect("fix clean");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(
        parsed["fixed_text"].as_str().unwrap(),
        "SECRET//NOFORN\n",
        "clean input should be unchanged after fix"
    );
}
