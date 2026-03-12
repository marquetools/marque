//! marque-wasm — WASM target for browser and web worker use.
//!
//! Compiled with `wasm-pack build --target web` (or `--target bundler`).
//! Exposes two functions: `lint` and `fix`, both operating on pre-extracted text.
//!
//! Format extraction is the caller's responsibility in WASM context.
//! Use a web worker to avoid blocking the main thread.

use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use marque_engine::Engine;
use marque_config::Config;
use marque_capco::capco_rules;

/// Initialize the WASM module. Call once before using lint/fix.
/// Sets up panic hook for better error messages in browser console.
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Lint a text string for classification marking violations.
///
/// # Arguments
/// - `text`: UTF-8 text to lint
/// - `config_json`: optional JSON config (see `WasmConfig`)
///
/// # Returns
/// JSON-serialized `WasmLintResult`
#[wasm_bindgen]
pub fn lint(text: &str, config_json: Option<String>) -> Result<String, JsValue> {
    let config = parse_config(config_json)?;
    let engine = build_engine(config);
    let result = engine.lint(text.as_bytes());

    let wasm_result = WasmLintResult {
        diagnostics: result.diagnostics.iter().map(|d| WasmDiagnostic {
            rule_id: d.rule.to_string(),
            severity: format!("{:?}", d.severity),
            message: d.message.clone(),
            start: d.span.start,
            end: d.span.end,
            fix: d.fix.as_ref().map(|f| WasmFix {
                replacement: f.replacement.clone(),
                confidence: f.confidence,
                migration_ref: f.migration_ref.map(str::to_owned),
            }),
        }).collect(),
        error_count: result.error_count(),
        warn_count: result.warn_count(),
        fix_count: result.fix_count(),
    };

    serde_json::to_string(&wasm_result)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Lint and apply fixes to a text string.
///
/// # Returns
/// JSON-serialized `WasmFixResult`
#[wasm_bindgen]
pub fn fix(text: &str, config_json: Option<String>) -> Result<String, JsValue> {
    let config = parse_config(config_json)?;
    let engine = build_engine(config);
    let result = engine.fix(text.as_bytes());

    let wasm_result = WasmFixResult {
        fixed_text: String::from_utf8(result.source)
            .map_err(|e| JsValue::from_str(&e.to_string()))?,
        applied_count: result.applied.len(),
        remaining_diagnostics: result.remaining_diagnostics.len(),
    };

    serde_json::to_string(&wasm_result)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

// ---------------------------------------------------------------------------
// WASM-side types (JSON-serialisable)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct WasmLintResult {
    diagnostics: Vec<WasmDiagnostic>,
    error_count: usize,
    warn_count: usize,
    fix_count: usize,
}

#[derive(Serialize, Deserialize)]
struct WasmDiagnostic {
    rule_id: String,
    severity: String,
    message: String,
    start: usize,
    end: usize,
    fix: Option<WasmFix>,
}

#[derive(Serialize, Deserialize)]
struct WasmFix {
    replacement: String,
    confidence: f32,
    migration_ref: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct WasmFixResult {
    fixed_text: String,
    applied_count: usize,
    remaining_diagnostics: usize,
}

/// Partial config accepted from JS callers.
#[derive(Deserialize, Default)]
struct WasmConfig {
    classifier_id: Option<String>,
}

fn parse_config(json: Option<String>) -> Result<Config, JsValue> {
    let wasm_cfg: WasmConfig = match json {
        Some(s) => serde_json::from_str(&s)
            .map_err(|e| JsValue::from_str(&e.to_string()))?,
        None => WasmConfig::default(),
    };

    let mut config = Config::default();
    config.user.classifier_id = wasm_cfg.classifier_id;
    Ok(config)
}

fn build_engine(config: Config) -> Engine {
    Engine::new(config, vec![Box::new(capco_rules())])
}
