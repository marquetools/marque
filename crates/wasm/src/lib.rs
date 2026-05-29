// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
//! marque-wasm — WASM target for browser and web worker use.
//!
//! Compiled with `wasm-pack build --target web` and operating on pre-extracted text.

#![cfg_attr(
    not(all(
        target_arch = "wasm32",
        any(feature = "multi-threading", feature = "talc_debug")
    )),
    forbid(unsafe_code)
)]
// Allocator statics, clock impls, and wasm-bindgen exports are gated on
// `target_arch = "wasm32"` and appear dead to the native host compiler and
// rust-analyzer. They are live on the actual build target, so only relax
// `dead_code` during non-wasm host analysis.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

#[cfg(all(target_arch = "wasm32", feature = "corpus-override"))]
compile_error!(
    "marque-wasm must not be built with the `corpus-override` feature on wasm32. \
     WASM uses build-time-baked priors only; runtime override is by design \
     unreachable in the WASM artifact."
);

#[cfg(all(
    target_arch = "wasm32",
    any(feature = "multi-threading", feature = "talc_debug"),
    not(target_feature = "atomics"),
))]
compile_error!(
    "The `multi-threading` and `talc_debug` features require WebAssembly atomics \
     (`-C target-feature=+atomics`), which is not enabled in the current build. \
     Add to .cargo/config.toml under [target.wasm32-unknown-unknown]:\n\
     \n\
     rustflags = [\"-C\", \
     \"target-feature=+simd128,+atomics,+bulk-memory,+mutable-globals\"]\n\
     \n\
     Note: the serving page must also send COOP/COEP headers for SharedArrayBuffer \
     access. See crates/wasm/src/lib.rs for details."
);

use marque_config::Config;
use marque_engine::{Clock, Engine, Instant};
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
#[cfg(all(
    target_arch = "wasm32",
    feature = "talc_alloc",
    not(feature = "multi-threading"),
    not(feature = "talc_debug"),
))]
#[global_allocator]
static ALLOCATOR: talc::wasm::WasmDynamicTalc = talc::wasm::new_wasm_dynamic_allocator();

#[cfg(all(
    target_arch = "wasm32",
    any(feature = "multi-threading", feature = "talc_debug"),
))]
use talc::{source::Claim, *};
use wasm_bindgen::prelude::*;

mod banner;
mod fix;
mod lint;
mod types;

/// Re-export of the engine's canonical audit-record projection so the
/// byte-identity parity test (`tests/audit_v3_0_parity.rs`) can exercise
/// the exact projection WASM `fix()` emits without reimplementing it. WASM
/// no longer carries a private copy — this routes through the single
/// engine source of truth (`crates/engine/src/audit_render.rs`).
#[doc(hidden)]
pub use marque_engine::audit_line_to_json_v1_0;

#[cfg(all(
    target_arch = "wasm32",
    any(feature = "multi-threading", feature = "talc_debug"),
))]
// Extra headroom beyond Talc's minimum first heap size so typical WASM lint/fix
// workloads do not immediately trigger heap growth. Tune alongside expected
// input sizes and allocator behavior.
const INITIAL_HEAP_EXTRA_BYTES: usize = 100_000;

#[cfg_attr(
    all(
        target_arch = "wasm32",
        any(feature = "multi-threading", feature = "talc_debug"),
    ),
    global_allocator
)]
#[cfg(all(
    target_arch = "wasm32",
    any(feature = "multi-threading", feature = "talc_debug"),
))]
static TALC: TalcLock<spinning_top::RawSpinlock, Claim> = TalcLock::new(
    // SAFETY: `INITIAL_HEAP` is a private static buffer used only to seed the
    // global allocator during this one-time initialization. We pass a raw
    // mutable pointer with `&raw mut`, so no Rust reference is created, and the
    // buffer is handed off to Talc for allocator-managed access. This module
    // does not access `INITIAL_HEAP` anywhere else, so there are no competing
    // aliases to the storage after the claim is created.
    unsafe {
        static mut INITIAL_HEAP: [u8; min_first_heap_size::<DefaultBinning>()
            + INITIAL_HEAP_EXTRA_BYTES] =
            [0; min_first_heap_size::<DefaultBinning>() + INITIAL_HEAP_EXTRA_BYTES];

        Claim::array(&raw mut INITIAL_HEAP)
    },
);

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
            std::time::UNIX_EPOCH + Duration::from_millis(millis)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            SystemTime::now()
        }
    }
}

// ---------------------------------------------------------------------------
// Partial config accepted from JS callers.
// ---------------------------------------------------------------------------

/// Partial config accepted from JS callers — a closed accept-list of
/// four fields (`classifier_id`, `confidence_threshold`, `corrections`,
/// `deadline_ms`). Constitution III preservation: unknown JSON fields
/// are silently ignored; field-level type mismatches are loud errors.
#[derive(Default)]
struct WasmConfig {
    classifier_id: Option<String>,
    confidence_threshold: Option<f32>,
    corrections: Option<HashMap<String, String>>,
    /// Per-call wall-clock budget in milliseconds.
    /// `None` / absent → no deadline. Values must satisfy
    /// `is_finite() && >= 0.0`; negative / NaN / Inf are rejected
    /// at parse time. See `parse_deadline_ms` for the validation
    /// rules and the Constitution III analysis at the top of this
    /// file for why a runtime budget cap is permitted in WASM.
    deadline_ms: Option<f64>,
}

/// Parse the JS-side `config_json` into a [`WasmConfig`], a per-call
/// deadline `Duration`, and a normalized cache key for
/// [`with_engine`].
fn parse_wasm_config(
    json: &Option<String>,
) -> Result<(WasmConfig, Option<Duration>, Option<String>), String> {
    let wasm_cfg = match json {
        None => WasmConfig::default(),
        Some(s) => {
            let value: Value = serde_json::from_str(s).map_err(|e| e.to_string())?;
            wasm_config_from_value(value)?
        }
    };
    let deadline_duration = parse_deadline_ms(wasm_cfg.deadline_ms)?;
    let cache_key = build_cache_key(&wasm_cfg)?;
    Ok((wasm_cfg, deadline_duration, cache_key))
}

/// Extract a [`WasmConfig`] from a parsed JSON value.
fn wasm_config_from_value(value: Value) -> Result<WasmConfig, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "config must be a JSON object".to_owned())?;

    let classifier_id = match obj.get("classifier_id") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(other) => {
            return Err(format!(
                "classifier_id must be a string; got {}",
                value_type_name(other)
            ));
        }
    };

    let confidence_threshold = match obj.get("confidence_threshold") {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => {
            let f = n
                .as_f64()
                .ok_or_else(|| "confidence_threshold must be a finite number".to_owned())?;
            Some(f as f32)
        }
        Some(other) => {
            return Err(format!(
                "confidence_threshold must be a number; got {}",
                value_type_name(other)
            ));
        }
    };

    let corrections = match obj.get("corrections") {
        None | Some(Value::Null) => None,
        Some(Value::Object(map_in)) => {
            let mut map_out = HashMap::with_capacity(map_in.len());
            for (k, v) in map_in {
                let s = match v {
                    Value::String(s) => s.clone(),
                    other => {
                        return Err(format!(
                            "corrections[{k}] must be a string; got {}",
                            value_type_name(other)
                        ));
                    }
                };
                map_out.insert(k.clone(), s);
            }
            Some(map_out)
        }
        Some(other) => {
            return Err(format!(
                "corrections must be a JSON object; got {}",
                value_type_name(other)
            ));
        }
    };

    let deadline_ms = match obj.get("deadline_ms") {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => Some(
            n.as_f64()
                .ok_or_else(|| "deadline_ms must be a finite number".to_owned())?,
        ),
        Some(other) => {
            return Err(format!(
                "deadline_ms must be a number; got {}",
                value_type_name(other)
            ));
        }
    };

    Ok(WasmConfig {
        classifier_id,
        confidence_threshold,
        corrections,
        deadline_ms,
    })
}

/// Human-readable type name for a JSON value, used in field-level
/// type-mismatch error messages.
fn value_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Build an engine-level [`Config`] from a parsed [`WasmConfig`].
fn build_engine_config(wasm_cfg: WasmConfig) -> Result<Config, String> {
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

/// Build the engine cache key for a parsed [`WasmConfig`].
fn build_cache_key(cfg: &WasmConfig) -> Result<Option<String>, String> {
    let corrections_present = cfg.corrections.as_ref().is_some_and(|c| !c.is_empty());
    if cfg.classifier_id.is_none() && cfg.confidence_threshold.is_none() && !corrections_present {
        return Ok(None);
    }

    let mut map = serde_json::Map::new();
    if let Some(id) = cfg.classifier_id.as_deref() {
        map.insert("classifier_id".to_owned(), Value::String(id.to_owned()));
    }
    if let Some(threshold) = cfg.confidence_threshold {
        let formatted = serde_json::to_string(&threshold).map_err(|e| e.to_string())?;
        let parsed: Value = serde_json::from_str(&formatted).map_err(|e| e.to_string())?;
        map.insert("confidence_threshold".to_owned(), parsed);
    }
    if let Some(corrections) = cfg.corrections.as_ref().filter(|c| !c.is_empty()) {
        let mut corrections_map = serde_json::Map::new();
        for (k, v) in corrections {
            corrections_map.insert(k.clone(), Value::String(v.clone()));
        }
        map.insert("corrections".to_owned(), Value::Object(corrections_map));
    }

    serde_json::to_string(&Value::Object(map))
        .map(Some)
        .map_err(|e| e.to_string())
}

/// Validate a JS-side `deadline_ms` value and convert to `Duration`.
fn parse_deadline_ms(value: Option<f64>) -> Result<Option<Duration>, String> {
    let Some(ms) = value else {
        return Ok(None);
    };
    if !ms.is_finite() {
        return Err(format!(
            "deadline_ms must be a finite number; got {ms} (NaN/Inf are rejected)"
        ));
    }
    if ms < 0.0 {
        return Err(format!("deadline_ms must be non-negative; got {ms}"));
    }
    Ok(Some(Duration::from_millis(ms as u64)))
}

/// Stamp `Instant::now() + duration`, mapping platform-clock overflow
/// to a structured error.
fn stamp_deadline(duration: Option<Duration>) -> Result<Option<Instant>, String> {
    let Some(d) = duration else {
        return Ok(None);
    };
    Instant::now()
        .checked_add(d)
        .map(Some)
        .ok_or_else(|| "deadline_ms is too large for the platform clock".to_owned())
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

/// Execute `f` against a cached `Engine`, rebuilding only when the
/// engine-relevant cache key differs from the previously cached
/// configuration.
fn with_engine<T, F>(
    cache_key: &Option<String>,
    build_config: F,
    f: impl FnOnce(&Engine) -> Result<T, String>,
) -> Result<T, String>
where
    F: FnOnce() -> Result<Config, String>,
{
    ENGINE_CACHE.with(|cell| {
        let mut cache = cell.try_borrow_mut().map_err(|_| {
            "engine cache is already mutably borrowed (either re-entrancy in the WASM callsite \
             or a prior WASM trap left the borrow alive — traps don't unwind, so a `RefMut` \
             alive at the trap site is never dropped)"
                .to_string()
        })?;

        let needs_rebuild = match &*cache {
            None => true,
            Some(cached) => cached.config_key.as_deref() != cache_key.as_deref(),
        };

        if needs_rebuild {
            let config = build_config()?;
            let engine = Engine::with_clock(
                config,
                marque_engine::default_ruleset(),
                marque_engine::default_scheme(),
                Box::new(WasmClock),
            )
            .map_err(|e| format!("engine construction failed: {e}"))?;
            *cache = Some(CachedEngine {
                engine,
                config_key: cache_key.clone(),
            });
        }

        f(&cache.as_ref().unwrap().engine)
    })
}

/// Pre-warm the engine cache (native entry point for tests).
pub fn configure_native(config_json: Option<String>) -> Result<(), String> {
    let (wasm_cfg, _, cache_key) = parse_wasm_config(&config_json)?;
    with_engine(
        &cache_key,
        move || build_engine_config(wasm_cfg),
        |_| Ok(()),
    )
}

pub use banner::{compute_banner_native, generate_cab_native};
pub use fix::fix_native;
pub use lint::{lint_batch_native, lint_native};

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

#[wasm_bindgen]
pub fn configure(config_json: Option<String>) -> Result<(), JsValue> {
    configure_native(config_json).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn lint(text: &str, config_json: Option<String>) -> Result<String, JsValue> {
    lint_native(text, config_json).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn fix(text: &str, threshold: f32, config_json: Option<String>) -> Result<String, JsValue> {
    fix_native(text, threshold, config_json).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn lint_batch(entries_json: &str, config_json: Option<String>) -> Result<String, JsValue> {
    lint_batch_native(entries_json, config_json).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn compute_banner(text: &str) -> Result<String, JsValue> {
    compute_banner_native(text).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
pub fn generate_cab(
    text: &str,
    classified_by: Option<String>,
    derived_from: Option<String>,
) -> Result<String, JsValue> {
    generate_cab_native(text, classified_by, derived_from).map_err(|e| JsValue::from_str(&e))
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests;
