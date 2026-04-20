// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
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

// TODO: We should probably implement a custom allocator for cloud deployment, since it's single threaded, using TalcCell
// TalcLock is tuned for multi-threaded workloads (i.e. browser)
// if we implement TalcCell, we can use `core::Allocator` on nightly builds and `allocator_api2::Allocator` on stable
// TODO: implement JavaScript calling instead of serializing to JSON using newer WASM 2.0 features

#![cfg_attr(
    all(target_arch = "wasm32", feature = "talc"),
    feature(allow_internal_unsafe)
)]
#![cfg_attr(
    not(all(target_arch = "wasm32", feature = "talc")),
    forbid(unsafe_code)
)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use marque_config::Config;
use marque_engine::{Clock, Engine, FixMode};
use marque_rules::{AppliedFix, Diagnostic, FixSource};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
#[cfg(target_arch = "wasm32")]
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(all(target_arch = "wasm32", feature = "simd128"))]
// placeholder for future SIMD-optimized code paths; currently unused but ensures the simd128 feature compiles.
#[cfg(all(target_arch = "wasm32", feature = "talc"))]
use talc::{source::Claim, *};
use wasm_bindgen::prelude::*;

#[cfg_attr(all(target_arch = "wasm32", feature = "talc"), global_allocator)]
#[cfg(all(target_arch = "wasm32", feature = "talc"))]
static TALC: TalcLock<spinning_top::RawSpinlock, Claim> = TalcLock::new(unsafe {
    static mut INITIAL_HEAP: [u8; min_first_heap_size::<DefaultBinning>() + 100000] =
        [0; min_first_heap_size::<DefaultBinning>() + 100000];

    Claim::array(&raw mut INITIAL_HEAP)
});

// ---------------------------------------------------------------------------
// WASM-compatible clock — Date.now() via wasm_bindgen extern
// ---------------------------------------------------------------------------

#[wasm_bindgen]
extern "C" {
    /// JavaScript `Date.now()` — returns milliseconds since Unix epoch.
    #[wasm_bindgen(js_namespace = Date, js_name = now)]
    fn date_now_ms() -> f64;
}

/// Clock implementation for WASM that calls JavaScript's `Date.now()`.
///
/// `SystemTime::now()` is not available on `wasm32-unknown-unknown` (panics
/// with "time not implemented on this platform"). This clock converts the
/// JS millisecond timestamp into a `SystemTime` that the engine's audit
/// records expect.
struct WasmClock;

impl Clock for WasmClock {
    fn now(&self) -> SystemTime {
        // date_now_ms() is only available in WASM context. In native test
        // context this struct is never constructed — native tests use
        // Engine::new() which injects SystemClock.
        #[cfg(target_arch = "wasm32")]
        {
            let millis = date_now_ms() as u64;
            UNIX_EPOCH + Duration::from_millis(millis)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            SystemTime::now()
        }
    }
}

/// Returns the current calendar year, usable in both native and WASM contexts.
///
/// In WASM, uses `Date.now()` via wasm_bindgen. In native, uses `SystemTime`.
fn current_year() -> u32 {
    #[cfg(target_arch = "wasm32")]
    {
        let millis = date_now_ms() as u64;
        let secs = millis / 1000;
        1970 + (secs / SECONDS_PER_JULIAN_YEAR) as u32
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        1970 + (secs / SECONDS_PER_JULIAN_YEAR) as u32
    }
}

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
///
/// Borrows from the `AppliedFix` to avoid per-field heap allocations.
/// Only `timestamp` is owned — `humantime::format_rfc3339` returns a
/// temporary that cannot be borrowed across the struct boundary.
#[derive(Debug, Serialize)]
struct AuditRecordJson<'a> {
    schema: &'static str,
    rule: &'a str,
    source: &'static str,
    span: SpanJson,
    original: &'a str,
    replacement: &'a str,
    confidence: f32,
    migration_ref: Option<&'a str>,
    timestamp: String,
    classifier_id: Option<&'a str>,
    dry_run: bool,
    input: Option<&'a str>,
}

const AUDIT_SCHEMA_VERSION: &str = "marque-mvp-1";

fn applied_fix_to_audit_json(fix: &AppliedFix) -> AuditRecordJson<'_> {
    AuditRecordJson {
        schema: AUDIT_SCHEMA_VERSION,
        rule: fix.proposal.rule.as_str(),
        source: fix_source_str(fix.proposal.source),
        span: SpanJson {
            start: fix.proposal.span.start,
            end: fix.proposal.span.end,
        },
        original: &fix.proposal.original,
        replacement: &fix.proposal.replacement,
        confidence: fix.proposal.confidence,
        migration_ref: fix.proposal.migration_ref,
        timestamp: humantime::format_rfc3339(fix.timestamp).to_string(),
        classifier_id: fix.classifier_id.as_deref(),
        dry_run: fix.dry_run,
        input: fix.input.as_deref(),
    }
}

/// Wrapper for `fix()` output.
#[derive(Debug, Serialize)]
struct FixResultJson<'a> {
    fixed_text: String,
    applied: Vec<AuditRecordJson<'a>>,
    remaining: Vec<Box<serde_json::value::RawValue>>,
}

// ---------------------------------------------------------------------------
// Batch types — lint_batch accepts an array of {id, text} entries and returns
// an array of {id, diagnostics} results in a single WASM boundary crossing.
// ---------------------------------------------------------------------------

/// One entry in a `lint_batch` request.
#[derive(Deserialize)]
struct BatchEntry {
    id: String,
    text: String,
}

/// One result in a `lint_batch` response.
#[derive(Serialize)]
struct BatchResultEntry<'a> {
    id: &'a str,
    diagnostics: Vec<Box<serde_json::value::RawValue>>,
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

fn parse_config(json: &Option<String>) -> Result<Config, String> {
    let wasm_cfg: WasmConfig = match json {
        Some(s) => serde_json::from_str(s).map_err(|e| e.to_string())?,
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

// ---------------------------------------------------------------------------
// Engine cache — avoids rebuilding the Engine (AhoCorasick, rule set, config)
// on every lint/fix call. WASM is single-threaded so thread_local + RefCell
// is safe and lock-free.
// ---------------------------------------------------------------------------

struct CachedEngine {
    engine: Engine,
    /// The raw config JSON used to build this engine. Byte-equal comparison
    /// for cache invalidation. `None` = default config.
    config_key: Option<String>,
}

thread_local! {
    static ENGINE_CACHE: RefCell<Option<CachedEngine>> = const { RefCell::new(None) };
}

/// Execute `f` against a cached `Engine`, rebuilding only when `config_json`
/// differs from the previously cached configuration.
///
/// The hot path (same config across calls) is an `Option<String>` comparison
/// and a `RefCell` borrow — no allocations, no AhoCorasick rebuild.
///
/// Uses `try_borrow_mut` to recover gracefully if a prior WASM trap left the
/// RefCell in a borrowed state (WASM traps don't unwind, so `borrow_mut`
/// guards are never released on panic).
fn with_engine<T>(
    config_json: &Option<String>,
    f: impl FnOnce(&Engine) -> Result<T, String>,
) -> Result<T, String> {
    ENGINE_CACHE.with(|cell| {
        let mut cache = cell.try_borrow_mut().map_err(|_| {
            "engine cache is locked (likely a prior WASM panic poisoned the RefCell)".to_string()
        })?;

        let needs_rebuild = match &*cache {
            None => true,
            Some(cached) => cached.config_key.as_deref() != config_json.as_deref(),
        };

        if needs_rebuild {
            let config = parse_config(config_json)?;
            *cache = Some(CachedEngine {
                engine: Engine::with_clock(
                    config,
                    marque_engine::default_ruleset(),
                    Box::new(WasmClock),
                ),
                config_key: config_json.clone(),
            });
        }

        f(&cache.as_ref().unwrap().engine)
    })
}

// ---------------------------------------------------------------------------
// Native-callable entry points (for parity tests — no JsValue dependency).
// ---------------------------------------------------------------------------

/// Pre-warm the engine cache (native entry point for tests).
pub fn configure_native(config_json: Option<String>) -> Result<(), String> {
    with_engine(&config_json, |_| Ok(()))
}

/// Lint text, returning NDJSON conforming to `contracts/diagnostic.json`.
/// One diagnostic per line, newline-terminated. Byte-identical to the CLI's
/// `--format json` output (SC-008).
pub fn lint_native(text: &str, config_json: Option<String>) -> Result<String, String> {
    with_engine(&config_json, |engine| {
        let result = engine.lint(text.as_bytes());

        // Write NDJSON directly into a byte buffer — avoids the intermediate
        // String allocation that serde_json::to_string produces per diagnostic.
        let mut buf = Vec::with_capacity(result.diagnostics.len() * 256);
        for d in &result.diagnostics {
            serde_json::to_writer(&mut buf, &diagnostic_to_json(d)).map_err(|e| e.to_string())?;
            buf.push(b'\n');
        }
        // serde_json always produces valid UTF-8.
        String::from_utf8(buf).map_err(|e| e.to_string())
    })
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
    with_engine(&config_json, |engine| {
        let result = engine
            .fix_with_threshold(text.as_bytes(), FixMode::Apply, Some(threshold))
            .map_err(|e| e.to_string())?;

        let fixed_text = String::from_utf8(result.source)
            .map_err(|e| format!("invalid UTF-8 in fix output: {e}"))?;

        let applied: Vec<AuditRecordJson> = result
            .applied
            .iter()
            .map(applied_fix_to_audit_json)
            .collect();

        // Remaining diagnostics as pre-serialized raw JSON. Each diagnostic
        // is serialized once into a byte buffer and wrapped as RawValue so
        // the parent FixResultJson serialization embeds it verbatim — no
        // intermediate serde_json::Value tree, no double serialization.
        let remaining: Vec<Box<serde_json::value::RawValue>> = result
            .remaining_diagnostics
            .iter()
            .map(|d| {
                let mut buf = Vec::with_capacity(256);
                serde_json::to_writer(&mut buf, &diagnostic_to_json(d))
                    .map_err(|e| e.to_string())?;
                let json = String::from_utf8(buf).map_err(|e| e.to_string())?;
                serde_json::value::RawValue::from_string(json).map_err(|e| e.to_string())
            })
            .collect::<Result<_, _>>()?;

        let fix_result = FixResultJson {
            fixed_text,
            applied,
            remaining,
        };

        // Serialize directly into a byte buffer to avoid serde_json::to_string's
        // intermediate String allocation.
        let mut buf = Vec::with_capacity(1024);
        serde_json::to_writer(&mut buf, &fix_result).map_err(|e| e.to_string())?;
        String::from_utf8(buf).map_err(|e| e.to_string())
    })
}

/// Lint multiple text entries in a single WASM boundary crossing.
///
/// Accepts a JSON array of `{"id": "...", "text": "..."}` objects and returns
/// a JSON array of `{"id": "...", "diagnostics": [...]}` results. All entries
/// are linted against the same cached engine.
///
/// Designed for as-you-type feedback: the JS caller debounces keystrokes,
/// extracts the changed paragraphs or marking regions, and sends them as a
/// batch. One boundary crossing, one engine, N lints.
///
/// ```js
/// const results = lint_batch(JSON.stringify([
///   { id: "para-1", text: "(S//NF) First paragraph..." },
///   { id: "para-2", text: "(TS//SI) Second paragraph..." },
/// ]));
/// ```
pub fn lint_batch_native(
    entries_json: &str,
    config_json: Option<String>,
) -> Result<String, String> {
    let entries: Vec<BatchEntry> = serde_json::from_str(entries_json).map_err(|e| e.to_string())?;

    with_engine(&config_json, |engine| {
        let results: Vec<BatchResultEntry<'_>> = entries
            .iter()
            .map(|entry| {
                let result = engine.lint(entry.text.as_bytes());
                let diagnostics = result
                    .diagnostics
                    .iter()
                    .map(|d| {
                        let mut buf = Vec::with_capacity(256);
                        serde_json::to_writer(&mut buf, &diagnostic_to_json(d))
                            .map_err(|e| e.to_string())?;
                        let json = String::from_utf8(buf).map_err(|e| e.to_string())?;
                        serde_json::value::RawValue::from_string(json).map_err(|e| e.to_string())
                    })
                    .collect::<Result<_, String>>()?;

                Ok(BatchResultEntry {
                    id: &entry.id,
                    diagnostics,
                })
            })
            .collect::<Result<_, String>>()?;

        let mut buf = Vec::with_capacity(results.len() * 512);
        serde_json::to_writer(&mut buf, &results).map_err(|e| e.to_string())?;
        String::from_utf8(buf).map_err(|e| e.to_string())
    })
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

/// Pre-warm the engine cache with the given configuration.
///
/// Optional — the engine is lazily constructed on the first `lint`/`fix` call.
/// Use this from a web worker's `onmessage` init handler to pay the
/// AhoCorasick + rule-set construction cost up front rather than on the
/// first lint request.
///
/// Passing `None` (or omitting the argument from JS) pre-warms with the
/// default configuration.
#[wasm_bindgen]
pub fn configure(config_json: Option<String>) -> Result<(), JsValue> {
    with_engine(&config_json, |_| Ok(())).map_err(|e| JsValue::from_str(&e))
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
///   "applied": [ /* audit records per contracts/audit-record.json *\/ ],
///   "remaining": [ /* diagnostics per contracts/diagnostic.json *\/ ]
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

/// Lint multiple text entries in a single WASM call.
///
/// Accepts a JSON array of `[{"id":"…","text":"…"}, …]` and returns a JSON
/// array of `[{"id":"…","diagnostics":[…]}, …]`.
///
/// Designed for as-you-type feedback: the JS caller debounces input, extracts
/// the changed paragraphs or marking regions, and sends them as one batch.
///
/// # Arguments
/// - `entries_json`: JSON array of `{"id": string, "text": string}` objects
/// - `config_json`: optional JSON config (same as `lint`)
#[wasm_bindgen]
pub fn lint_batch(entries_json: &str, config_json: Option<String>) -> Result<String, JsValue> {
    lint_batch_native(entries_json, config_json).map_err(|e| JsValue::from_str(&e))
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
///    is present).
/// 3. If the document computes as UNCLASSIFIED (with or without dissem
///    controls), returns an **empty string** — no CAB is required for
///    UNCLASSIFIED documents.
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

    let classified_by = classified_by.unwrap_or_else(|| "Derivative Classifier".to_owned());
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
            if candidate.kind == MarkingType::Portion {
                page_context.add_portion(parsed.attrs);
            }
        }
    }

    // If the document is unclassified, there is no CAB at all.
    // CAPCO: a CAB is only required for classified NSI documents; an
    // UNCLASSIFIED banner (with or without dissem controls) carries no
    // "Classified By", "Derived From", or "Declassify On" fields.
    if !page_context.is_classified() {
        return Ok(String::new());
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
        // Format as YYYYMMDD (December 31, conventional end-of-year date).
        let base_year = current_year();
        format!("{}1231", base_year + 25)
    };

    Ok(format!(
        "Classified By: {classified_by}\nDerived From: {derived_from}\nDeclassify On: {declass}"
    ))
}

/// Seconds in a Julian year (365.25 × 24 × 3600), used to approximate the
/// current calendar year from a UNIX timestamp.
const SECONDS_PER_JULIAN_YEAR: u64 = 31_557_600;

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
