//! marque-wasm — WASM target for browser and web worker use.
//!
//! Compiled with `wasm-pack build --target web` (or `--target bundler`).
//! Exposes two functions: `lint` and `fix`, both operating on pre-extracted text.
//!
//! Format extraction is the caller's responsibility in WASM context.
//! Use a web worker to avoid blocking the main thread.
//!
//! ## Output Contract
//!
//! `lint()` returns NDJSON conforming to `contracts/diagnostic.json` — one record
//! per line. This is byte-identical to the CLI's `--format json` output (SC-008).
//!
//! `fix()` returns a JSON object with `fixed_text`, `applied` (audit records
//! per `contracts/audit-record.json`), and `remaining` (diagnostics per
//! `contracts/diagnostic.json`).

use marque_config::Config;
use marque_engine::{Engine, FixMode};
use marque_rules::{AppliedFix, Diagnostic, FixSource};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// JSON serialization types — duplicated from CLI render.rs for SC-008 parity.
// The parity test (T061) catches any divergence.
// ---------------------------------------------------------------------------

/// JSON projection of a `Diagnostic` conforming to `contracts/diagnostic.json`.
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

fn fix_source_str(source: FixSource) -> &'static str {
    match source {
        FixSource::BuiltinRule => "BuiltinRule",
        FixSource::CorrectionsMap => "CorrectionsMap",
        FixSource::MigrationTable => "MigrationTable",
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

/// JSON projection of an `AppliedFix` conforming to `contracts/audit-record.json`.
#[derive(Debug, Serialize)]
struct AuditRecordJson {
    schema: &'static str,
    rule: String,
    source: &'static str,
    span: SpanJson,
    original: String,
    replacement: String,
    confidence: f32,
    migration_ref: Option<String>,
    timestamp: String,
    classifier_id: Option<String>,
    dry_run: bool,
    input: Option<String>,
}

const AUDIT_SCHEMA_VERSION: &str = "marque-mvp-1";

fn applied_fix_to_audit_json(fix: &AppliedFix) -> AuditRecordJson {
    AuditRecordJson {
        schema: AUDIT_SCHEMA_VERSION,
        rule: fix.proposal.rule.as_str().to_owned(),
        source: fix_source_str(fix.proposal.source),
        span: SpanJson {
            start: fix.proposal.span.start,
            end: fix.proposal.span.end,
        },
        original: fix.proposal.original.to_string(),
        replacement: fix.proposal.replacement.to_string(),
        confidence: fix.proposal.confidence,
        migration_ref: fix.proposal.migration_ref.map(|s| s.to_owned()),
        timestamp: humantime::format_rfc3339(fix.timestamp).to_string(),
        classifier_id: fix.classifier_id.as_ref().map(|s| s.to_string()),
        dry_run: fix.dry_run,
        input: fix.input.as_ref().map(|s| s.to_string()),
    }
}

/// Wrapper for `fix()` output.
#[derive(Debug, Serialize)]
struct FixResultJson {
    fixed_text: String,
    applied: Vec<AuditRecordJson>,
    remaining: Vec<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Partial config accepted from JS callers.
// ---------------------------------------------------------------------------

#[derive(Deserialize, Default)]
struct WasmConfig {
    #[serde(default)]
    classifier_id: Option<String>,
    #[serde(default)]
    confidence_threshold: Option<f32>,
    #[serde(default)]
    corrections: Option<HashMap<String, String>>,
}

fn parse_config(json: Option<String>) -> Result<Config, String> {
    let wasm_cfg: WasmConfig = match json {
        Some(s) => serde_json::from_str(&s).map_err(|e| e.to_string())?,
        None => WasmConfig::default(),
    };

    let mut config = Config::default();
    config.user.classifier_id = wasm_cfg.classifier_id;
    if let Some(threshold) = wasm_cfg.confidence_threshold {
        config
            .set_confidence_threshold(threshold)
            .map_err(|e| e.to_string())?;
    }
    if let Some(corrections) = wasm_cfg.corrections {
        config.corrections = corrections;
    }
    Ok(config)
}

fn build_engine(config: Config) -> Engine {
    Engine::new(config, marque_engine::default_ruleset())
}

// ---------------------------------------------------------------------------
// Native-callable entry points (for parity tests — no JsValue dependency).
// ---------------------------------------------------------------------------

/// Lint text, returning NDJSON conforming to `contracts/diagnostic.json`.
/// One diagnostic per line, newline-terminated. Byte-identical to the CLI's
/// `--format json` output (SC-008).
pub fn lint_native(text: &str, config_json: Option<String>) -> Result<String, String> {
    let config = parse_config(config_json)?;
    let engine = build_engine(config);
    let result = engine.lint(text.as_bytes());

    let mut out = String::new();
    for d in &result.diagnostics {
        let json = serde_json::to_string(&diagnostic_to_json(d)).map_err(|e| e.to_string())?;
        out.push_str(&json);
        out.push('\n');
    }
    Ok(out)
}

/// Fix text, returning a JSON object with `fixed_text`, `applied` audit records,
/// and `remaining` diagnostics.
///
/// The `threshold` parameter always takes precedence over any `confidence_threshold`
/// in `config_json`. This matches the CLI's Layer 4 (CLI flag) override behavior.
pub fn fix_native(
    text: &str,
    threshold: f32,
    config_json: Option<String>,
) -> Result<String, String> {
    let mut config = parse_config(config_json)?;
    config
        .set_confidence_threshold(threshold)
        .map_err(|e| e.to_string())?;
    let engine = build_engine(config);
    let result = engine.fix(text.as_bytes(), FixMode::Apply);

    let fixed_text = String::from_utf8(result.source)
        .map_err(|e| format!("invalid UTF-8 in fix output: {e}"))?;

    let applied: Vec<AuditRecordJson> = result
        .applied
        .iter()
        .map(applied_fix_to_audit_json)
        .collect();

    // Remaining diagnostics as contract-conformant JSON values.
    let remaining: Vec<serde_json::Value> = result
        .remaining_diagnostics
        .iter()
        .map(|d| serde_json::to_value(diagnostic_to_json(d)).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;

    let fix_result = FixResultJson {
        fixed_text,
        applied,
        remaining,
    };

    serde_json::to_string(&fix_result).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// wasm-bindgen exports
// ---------------------------------------------------------------------------

/// Initialize the WASM module. Call once before using lint/fix.
/// Sets up panic hook for better error messages in browser console.
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Lint a text string for classification marking violations.
///
/// Returns NDJSON conforming to `contracts/diagnostic.json` — one record per
/// line. Byte-identical to the native CLI's `--format json` output (SC-008).
///
/// # Arguments
/// - `text`: UTF-8 text to lint
/// - `config_json`: optional JSON config `{"classifier_id":"...","corrections":{"NF":"NOFORN"}}`
#[wasm_bindgen]
pub fn lint(text: &str, config_json: Option<String>) -> Result<String, JsValue> {
    lint_native(text, config_json).map_err(|e| JsValue::from_str(&e))
}

/// Lint and apply fixes to a text string.
///
/// Returns a JSON object:
/// ```json
/// {
///   "fixed_text": "SECRET//NOFORN\n",
///   "applied": [ /* audit records per contracts/audit-record.json */ ],
///   "remaining": [ /* diagnostics per contracts/diagnostic.json */ ]
/// }
/// ```
///
/// # Arguments
/// - `text`: UTF-8 text to lint and fix
/// - `threshold`: confidence threshold (0.0–1.0); fixes below this are suggestions only
/// - `config_json`: optional JSON config
#[wasm_bindgen]
pub fn fix(text: &str, threshold: f32, config_json: Option<String>) -> Result<String, JsValue> {
    fix_native(text, threshold, config_json).map_err(|e| JsValue::from_str(&e))
}
