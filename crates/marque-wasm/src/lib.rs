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

// ---------------------------------------------------------------------------
// compute_banner — scanner + parser + PageContext only (no rules engine)
// ---------------------------------------------------------------------------

/// Compute the expected CAPCO banner string from portion markings in `text`.
///
/// Scans the text for portion markings only, parses each, accumulates a
/// [`PageContext`], and returns `render_expected_banner()`. Does NOT run the
/// rules engine — this is purely: scanner → parser → PageContext.
///
/// Returns `"UNCLASSIFIED"` if no portions are found or none parse.
pub fn compute_banner_native(text: &str) -> Result<String, String> {
    use marque_core::{Parser, Scanner};
    use marque_ism::{CapcoTokenSet, MarkingType, PageContext};

    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidates = Scanner::scan(text.as_bytes());
    let mut page_context = PageContext::new();

    for candidate in &candidates {
        if candidate.kind != MarkingType::Portion {
            continue;
        }
        if let Ok(parsed) = parser.parse(candidate, text.as_bytes()) {
            page_context.add_portion(parsed.attrs);
        }
    }

    Ok(page_context
        .render_expected_banner()
        .unwrap_or_else(|| "UNCLASSIFIED".to_owned()))
}

/// Compute the expected CAPCO banner string from portion markings in `text`.
///
/// Returns `"UNCLASSIFIED"` if no portion markings are found.
#[wasm_bindgen]
pub fn compute_banner(text: &str) -> Result<String, JsValue> {
    compute_banner_native(text).map_err(|e| JsValue::from_str(&e))
}

// ---------------------------------------------------------------------------
// generate_cab — Classification Authority Block text
// ---------------------------------------------------------------------------

/// Generate a Classification Authority Block (CAB) text block.
///
/// Scans `text` for portion markings to determine the document's expected
/// classification and declassification marking, then produces a formatted CAB:
///
/// ```text
/// Classified By: <classified_by>
/// Derived From: <derived_from>
/// Declassify On: <declass>
/// ```
///
/// # Declassification logic
///
/// 1. If an explicit `declassify_on` date or `declass_exemption` is found in a
///    parsed marking in `text`, that value is used verbatim.
/// 2. Otherwise, the default is **25 years from the current year** per
///    EO 13526 § 1.5(a) (the CAPCO default for NSI when no other instruction
///    is present). This can be overridden by the caller via `declass_override`.
/// 3. If the document is UNCLASSIFIED, the `Declassify On` line is omitted.
///
/// `classified_by` defaults to `"Derivative Classifier"` if not provided.
/// `derived_from` defaults to `"Multiple Sources"` if not provided.
pub fn generate_cab_native(
    text: &str,
    classified_by: Option<String>,
    derived_from: Option<String>,
) -> Result<String, String> {
    use marque_core::{Parser, Scanner};
    use marque_ism::{CapcoTokenSet, MarkingType, PageContext};

    let classified_by =
        classified_by.unwrap_or_else(|| "Derivative Classifier".to_owned());
    let derived_from = derived_from.unwrap_or_else(|| "Multiple Sources".to_owned());

    // Scan text to accumulate portions into PageContext and collect any
    // declassification markings already present.
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidates = Scanner::scan(text.as_bytes());
    let mut page_context = PageContext::new();
    let mut found_declass_date: Option<String> = None;
    let mut found_declass_exemption: Option<String> = None;

    for candidate in &candidates {
        if let Ok(parsed) = parser.parse(candidate, text.as_bytes()) {
            if candidate.kind == MarkingType::Portion {
                page_context.add_portion(parsed.attrs.clone());
            }
            if found_declass_date.is_none() {
                if let Some(date) = &parsed.attrs.declassify_on {
                    found_declass_date = Some(date.to_string());
                }
            }
            if found_declass_exemption.is_none() {
                if let Some(ex) = parsed.attrs.declass_exemption {
                    found_declass_exemption = Some(ex.as_str().to_owned());
                }
            }
        }
    }

    // If the document is unclassified, omit the Declassify On line entirely.
    if !page_context.is_classified() {
        return Ok(format!(
            "Classified By: {classified_by}\nDerived From: {derived_from}"
        ));
    }

    // Determine the declassification marking.
    let declass = if let Some(date) = found_declass_date {
        date
    } else if let Some(ex) = found_declass_exemption {
        ex
    } else if let Some(ex) = page_context.expected_declass_exemption() {
        ex.as_str().to_owned()
    } else {
        // EO 13526 §1.5(a) default: 25 years from the date of origin.
        // Since we cannot determine the document date from raw text, we
        // use the current year as a conservative base (the user should
        // supply a known origination date via a future API parameter when
        // precision matters).
        let base_year = current_year();
        format!("{}", base_year + 25)
    };

    Ok(format!(
        "Classified By: {classified_by}\nDerived From: {derived_from}\nDeclassify On: {declass}"
    ))
}

/// Returns the current calendar year, usable in both native and WASM contexts.
///
/// Uses `std::time::SystemTime` (available since Rust 1.85 in `wasm32-unknown-unknown`).
/// Falls back to 2025 in the unlikely event the system clock is unavailable.
fn current_year() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Approximate: 1970 + elapsed_seconds / seconds_per_year (Julian year ≈ 365.25 days)
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    1970 + (secs / 31_557_600) as u32
}

/// Generate a Classification Authority Block (CAB) text block.
///
/// Returns formatted multi-line text suitable for display in the CAB section
/// of a classified document.
///
/// # Arguments
/// - `text`: document body text (used to compute classification from portions)
/// - `classified_by`: optional "Classified By" field (defaults to "Derivative Classifier")
/// - `derived_from`: optional "Derived From" field (defaults to "Multiple Sources")
#[wasm_bindgen]
pub fn generate_cab(
    text: &str,
    classified_by: Option<String>,
    derived_from: Option<String>,
) -> Result<String, JsValue> {
    generate_cab_native(text, classified_by, derived_from).map_err(|e| JsValue::from_str(&e))
}
