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
//!
//! ## Constitution III analysis: `deadline_ms` (spec 005)
//!
//! `WasmConfig` carries a `deadline_ms` field that JS callers may set to bound
//! per-call wall-clock work. This analysis confirms the field is permissible
//! under the Constitution III rule that the WASM target "MUST NOT accept
//! runtime configuration that expands the engine's semantic surface."
//!
//! The relevant Constitution III property is *not* "no decoder runs in the
//! WASM build" — the decoder fallback is the engine default for every
//! target (see [`marque_engine::Engine::new`], which installs
//! `StrictOrDecoderRecognizer`), and the WASM `lint` / `fix` entry points
//! exercise the same strict-first / decoder-fallback dispatch as the CLI
//! and server. The property is "no *caller-controlled* configuration
//! switches the recognizer codepath or alters recognizer posteriors at
//! runtime." The decoder, the corpus priors it consumes, and the
//! recognizer choice are all compile-time decisions.
//!
//! - **No new recognizer codepath.** `deadline_ms` translates into
//!   `LintOptions { deadline: Some(Instant) }` / `FixOptions { deadline: ... }`,
//!   the same data the engine already consults whenever the per-document
//!   deadline check fires. The recognizer trait object
//!   ([`marque_engine::StrictOrDecoderRecognizer`] by default,
//!   [`marque_engine::StrictRecognizer`] when a caller in CLI/server context
//!   pins via `Engine::with_recognizer`) is fixed at engine construction —
//!   `deadline_ms` does not flip strict ↔ decoder, and the WASM target does
//!   not expose `with_recognizer` to JS callers.
//! - **No posterior change.** The deadline check is a `bool` early-return at
//!   candidate boundaries; it gates whether the next candidate is processed,
//!   not how it is scored. A truncated lint produces a *subset* of the
//!   diagnostics the same input would produce without a deadline; every
//!   diagnostic that does fire has identical `Span`, `Severity`, and
//!   `FixProposal` values to the non-truncated equivalent. Decoder priors
//!   are read from compile-time-baked tables in `marque-capco::priors`; a
//!   WASM caller cannot redirect, override, or tamper with those priors
//!   at runtime (the `corpus-override` Cargo feature is gated out of the
//!   WASM artifact — see Gate 1 / Gate 2 in `cli-server-wasm-gates.md`).
//! - **No vocabulary surface change.** The CVE token set, severity table,
//!   and corrections map are unchanged. `deadline_ms` does not introduce a
//!   new way for a caller to influence which tokens the engine recognizes
//!   or how it labels them.
//! - **Permitted under the "data already present in the strict-path codepath"
//!   carve-out.** Constitution III explicitly allows runtime config that
//!   shares the strict-path data shape — severity overrides, corrections
//!   maps. `deadline_ms` is the same kind of object: a runtime budget cap
//!   that constrains *how much* work the engine does, not *what* work.

// TalcLock is tuned for multi-threaded workloads (i.e. server-side)
// if we implement TalcCell, we can use `core::Allocator` on nightly builds and `allocator_api2::Allocator` on stable

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

// T067 / T3 enforcement (Constitution III + FR-013 + whitepaper §10.3 +
// `specs/004-constraints-decoder-vocab/contracts/cli-server-wasm-gates.md`
// Gate 1). The `corpus-override` feature MUST NOT reach the WASM
// artifact. Primary defense is the absence of a `corpus-override`
// declaration in `Cargo.toml [features]`; this guard is the secondary
// defense against a future commit that inadvertently re-introduces it
// or propagates it transitively from a dependency. Companion
// compile-fail check lives at `crates/wasm/tests/no_corpus_override.rs`
// (T051). The `corpus-override` cfg name is declared at the workspace
// level (`Cargo.toml` workspace.lints.rust check-cfg) so this probe
// does not trip `unexpected_cfgs` despite the feature being locally
// undeclared.
#[cfg(all(target_arch = "wasm32", feature = "corpus-override"))]
compile_error!(
    "marque-wasm must not be built with the `corpus-override` feature on wasm32. \
     T3 enforcement per docs/security/WHITEPAPER.md §10.3 and \
     specs/004-constraints-decoder-vocab/contracts/cli-server-wasm-gates.md Gate 1. \
     WASM uses build-time-baked priors only; runtime override is by design \
     unreachable in the WASM artifact."
);

// T-atomics guard: `multi-threading` and `talc_debug` both activate TalcLock with
// spinning_top::RawSpinlock, which requires the WebAssembly atomics proposal
// (`-C target-feature=+atomics`). Building without that flag produces a binary
// that may panic or miscompile at runtime on any runtime that doesn't expose
// SharedArrayBuffer. Catch this at compile time instead.
//
// To build a threaded WASM binary, add to .cargo/config.toml:
//
//   [target.wasm32-unknown-unknown]
//   rustflags = [
//     "-C", "target-feature=+simd128,+atomics,+bulk-memory,+mutable-globals",
//   ]
//
// Note: +bulk-memory and +mutable-globals are also required by wasm-bindgen
// for the SharedArrayBuffer/threading model. The serving page MUST send:
//   Cross-Origin-Opener-Policy: same-origin
//   Cross-Origin-Embedder-Policy: require-corp
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

use marque_capco::CapcoScheme;
use marque_config::Config;
use marque_engine::{Clock, Engine, EngineError, FixMode, FixOptions, Instant, LintOptions};
use marque_rules::{AppliedFix, Diagnostic, FixSource};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
// Single-threaded allocator: WasmDynamicTalc grows WASM memory via memory.grow and
// carries no spinlock overhead. Active when `talc_alloc` is set without
// `multi-threading` or `talc_debug`.
#[cfg(all(
    target_arch = "wasm32",
    feature = "talc_alloc",
    not(feature = "multi-threading"),
    not(feature = "talc_debug"),
))]
#[global_allocator]
static ALLOCATOR: talc::wasm::WasmDynamicTalc = talc::wasm::new_wasm_dynamic_allocator();

// Multi-threaded / debug allocator: TalcLock with a static seed heap. Active when
// `multi-threading` (SharedArrayBuffer builds) or `talc_debug` is set. When both
// `talc_alloc` and `multi-threading` are active (e.g., via `cloud_talc`), this
// declaration wins because `ALLOCATOR` above is gated on `not(feature = "multi-threading")`.
#[cfg(all(
    target_arch = "wasm32",
    any(feature = "multi-threading", feature = "talc_debug"),
))]
use talc::{source::Claim, *};
use wasm_bindgen::prelude::*;

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
    intent_kind: &'static str,
    replacement: Option<&'a str>,
    confidence: f32,
    migration_ref: Option<&'a str>,
}

fn fix_source_str(source: FixSource) -> &'static str {
    match source {
        FixSource::BuiltinRule => "BuiltinRule",
        FixSource::CorrectionsMap => "CorrectionsMap",
        FixSource::MigrationTable => "MigrationTable",
        FixSource::DecoderPosterior => "DecoderPosterior",
        FixSource::DecoderClassificationHeuristic => "DecoderClassificationHeuristic",
    }
}

/// Schema-pinned JSON projection of `CapcoOpenVocabRef`. Mirrors the
/// CLI's `open_vocab_ref_to_json` in `marque/src/render.rs` — CLI and
/// WASM must emit byte-identical JSON for SC-008 parity.
fn open_vocab_ref_to_json(r: &marque_capco::CapcoOpenVocabRef) -> serde_json::Value {
    match r {
        marque_capco::CapcoOpenVocabRef::Sar(name) => serde_json::json!({
            "kind": "Sar",
            "name": name.as_ref(),
        }),
        marque_capco::CapcoOpenVocabRef::SciCompartment(name) => serde_json::json!({
            "kind": "SciCompartment",
            "name": name.as_ref(),
        }),
        marque_capco::CapcoOpenVocabRef::SciSubCompartment(name) => serde_json::json!({
            "kind": "SciSubCompartment",
            "name": name.as_ref(),
        }),
        marque_capco::CapcoOpenVocabRef::FgiTetragraph(code) => serde_json::json!({
            "kind": "FgiTetragraph",
            "code": code.as_ref(),
        }),
        marque_capco::CapcoOpenVocabRef::CountryCode(c) => serde_json::json!({
            "kind": "CountryCode",
            "code": c.as_str(),
        }),
    }
}

/// JSON projection of a `FactRef<CapcoScheme>`. Constitution V
/// Principle V permits CVE token IDs and open-vocab canonical refs
/// in audit output (token canonicals + category IDs are on the
/// permitted-identifier list).
fn fact_ref_to_json(fact: &marque_scheme::FactRef<CapcoScheme>) -> serde_json::Value {
    match fact {
        marque_scheme::FactRef::Cve(token_id) => serde_json::json!({
            "kind": "Cve",
            "token_id": token_id.0,
        }),
        marque_scheme::FactRef::OpenVocab(r) => serde_json::json!({
            "kind": "OpenVocab",
            "ref": open_vocab_ref_to_json(r),
        }),
    }
}

/// Schema-pinned string projection of `Scope`. Used in the audit JSON
/// `proposal.intent.scope` field — `Debug` would not be a stable wire
/// format.
fn scope_str(scope: marque_scheme::Scope) -> &'static str {
    match scope {
        marque_scheme::Scope::Portion => "Portion",
        marque_scheme::Scope::Page => "Page",
        marque_scheme::Scope::Document => "Document",
        marque_scheme::Scope::Diff => "Diff",
    }
}

/// Schema-pinned string projection of `RecanonScope`. Same rationale
/// as [`scope_str`].
fn recanon_scope_str(scope: marque_scheme::fix_intent::RecanonScope) -> &'static str {
    match scope {
        marque_scheme::fix_intent::RecanonScope::Portion => "Portion",
        marque_scheme::fix_intent::RecanonScope::Page => "Page",
        marque_scheme::fix_intent::RecanonScope::Document => "Document",
    }
}

fn diagnostic_to_json(d: &Diagnostic<CapcoScheme>) -> DiagnosticJson<'_> {
    DiagnosticJson {
        rule: d.rule.as_str(),
        severity: d.severity.as_str(),
        span: SpanJson {
            start: d.span.start,
            end: d.span.end,
        },
        message: d.message.as_ref(),
        citation: d.citation,
        fix: match (d.fix.as_ref(), d.text_correction.as_ref()) {
            (Some(f), _) => Some(FixJson {
                source: fix_source_str(f.source),
                intent_kind: match &f.replacement {
                    marque_scheme::ReplacementIntent::FactAdd { .. } => "FactAdd",
                    marque_scheme::ReplacementIntent::FactRemove { .. } => "FactRemove",
                    marque_scheme::ReplacementIntent::Recanonicalize { .. } => "Recanonicalize",
                    _ => "Unknown",
                },
                replacement: None,
                confidence: f.confidence.combined(),
                migration_ref: f.migration_ref,
            }),
            (None, Some(tc)) => Some(FixJson {
                source: fix_source_str(tc.source),
                intent_kind: "TextCorrection",
                replacement: Some(tc.replacement.as_ref()),
                confidence: tc.confidence.combined(),
                migration_ref: tc.migration_ref,
            }),
            (None, None) => None,
        },
    }
}

// ---------------------------------------------------------------------------
// Audit record JSON (marque-mvp-3)
//
// Mirrors the CLI emitter (`marque/src/render.rs`) for SC-008 parity.
// Single accepted schema post PR 3c.B Commit 10.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(tag = "kind")]
enum ProposalJson<'a> {
    FixIntent { intent: serde_json::Value },
    TextCorrection { replacement: &'a str },
}

#[derive(Debug, Serialize)]
struct AuditRecordJsonV3<'a> {
    schema: &'static str,
    rule: &'a str,
    source: &'static str,
    span: SpanJson,
    proposal: ProposalJson<'a>,
    confidence: f32,
    migration_ref: Option<&'a str>,
    timestamp: String,
    classifier_id: Option<&'a str>,
    dry_run: bool,
    input: Option<&'a str>,
    recognition: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    runner_up_ratio: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    features: Vec<FeatureJson>,
}

#[derive(Debug, Serialize)]
struct FeatureJson {
    id: &'static str,
    delta: f32,
}

fn proposal_to_json<'a>(
    proposal: &'a marque_rules::AppliedFixProposal<CapcoScheme>,
) -> ProposalJson<'a> {
    match proposal {
        marque_rules::AppliedFixProposal::FixIntent(intent) => {
            let inner: serde_json::Value = match &intent.replacement {
                marque_scheme::ReplacementIntent::FactAdd { token, scope } => serde_json::json!({
                    "kind": "FactAdd",
                    "scope": scope_str(*scope),
                    "token": fact_ref_to_json(token),
                }),
                marque_scheme::ReplacementIntent::FactRemove { scope, facts } => {
                    let facts_json: Vec<serde_json::Value> =
                        facts.iter().map(fact_ref_to_json).collect();
                    serde_json::json!({
                        "kind": "FactRemove",
                        "scope": scope_str(*scope),
                        "facts": facts_json,
                    })
                }
                marque_scheme::ReplacementIntent::Recanonicalize { scope } => {
                    serde_json::json!({
                        "kind": "Recanonicalize",
                        "scope": recanon_scope_str(*scope),
                    })
                }
                _ => serde_json::json!({ "kind": "Unknown" }),
            };
            ProposalJson::FixIntent { intent: inner }
        }
        marque_rules::AppliedFixProposal::TextCorrection { replacement } => {
            ProposalJson::TextCorrection { replacement }
        }
    }
}

fn applied_fix_to_audit_json_v3(fix: &AppliedFix<CapcoScheme>) -> AuditRecordJsonV3<'_> {
    let c = &fix.confidence;
    AuditRecordJsonV3 {
        schema: marque_engine::AUDIT_SCHEMA_VERSION,
        rule: fix.rule.as_str(),
        source: fix_source_str(fix.source),
        span: SpanJson {
            start: fix.span.start,
            end: fix.span.end,
        },
        proposal: proposal_to_json(&fix.proposal),
        confidence: c.combined(),
        migration_ref: fix.migration_ref,
        timestamp: humantime::format_rfc3339(fix.timestamp).to_string(),
        classifier_id: fix.classifier_id.as_deref(),
        dry_run: fix.dry_run,
        input: fix.input.as_deref(),
        recognition: c.recognition,
        runner_up_ratio: c.runner_up_ratio,
        features: c
            .features
            .iter()
            .map(|f| FeatureJson {
                id: f.id.as_str(),
                delta: f.delta,
            })
            .collect(),
    }
}

fn serialize_applied_fix(
    fix: &AppliedFix<CapcoScheme>,
) -> Result<Box<serde_json::value::RawValue>, String> {
    let _ = marque_engine::AUDIT_SCHEMA_IS_V3;
    let json =
        serde_json::to_string(&applied_fix_to_audit_json_v3(fix)).map_err(|e| e.to_string())?;
    serde_json::value::RawValue::from_string(json).map_err(|e| e.to_string())
}

/// Wrapper for `fix()` output.
#[derive(Debug, Serialize)]
struct FixResultJson {
    fixed_text: String,
    applied: Vec<Box<serde_json::value::RawValue>>,
    remaining: Vec<Box<serde_json::value::RawValue>>,
    /// Mirrors [`marque_engine::FixResult::r002_fired`]. Serialized
    /// at the top level of the JS-object so callers can branch on a
    /// single field read without parsing the NDJSON `remaining`
    /// stream (PR 7b D1 binding constraint).
    r002_fired: bool,
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
    /// Per-call wall-clock budget in milliseconds (spec 005).
    /// `None` / absent → no deadline. Values must satisfy
    /// `is_finite() && >= 0.0`; negative / NaN / Inf are rejected
    /// at parse time. See `parse_deadline_ms` for the validation
    /// rules and the Constitution III analysis at the top of this
    /// file for why a runtime budget cap is permitted in WASM.
    #[serde(default)]
    deadline_ms: Option<f64>,
}

/// Parse the JS-side `config_json` into a [`WasmConfig`], a per-call
/// deadline `Duration`, and a normalized cache key for
/// [`with_engine`].
///
/// Returns `WasmConfig` (not yet a built `Config`) so the engine
/// cache hit path can avoid building a full `Config` (and the
/// associated HashMap moves) when the cached engine is reusable.
/// `Config` is constructed lazily inside [`with_engine`] via the
/// caller-supplied `build_config` closure on cache miss only.
///
/// The third return value is the **engine cache key**, constructed
/// by serializing only the engine-relevant fields (`classifier_id`,
/// `confidence_threshold`, `corrections`) and deliberately excluding
/// `deadline_ms`. This means a caller varying `deadline_ms` per call
/// does not trigger an `Engine` rebuild. Constitution III analysis
/// at the top of this file explains why `deadline_ms` is
/// non-semantic and therefore safe to drop from the cache key.
///
/// `corrections` is serialized via `BTreeMap<&str, &str>` (sorted by
/// key) so the cache-key string is stable across calls — `HashMap`
/// iteration order is non-deterministic, which would otherwise
/// produce different cache-key strings for byte-equal corrections
/// content and force unnecessary engine rebuilds.
///
/// Returns `Ok(None)` for the cache key when no cache-relevant field
/// is set (default config OR an empty corrections map); a
/// deadline-only invocation hits the same default-config cache slot.
fn parse_wasm_config(
    json: &Option<String>,
) -> Result<(WasmConfig, Option<Duration>, Option<String>), String> {
    let wasm_cfg: WasmConfig = match json {
        Some(s) => serde_json::from_str(s).map_err(|e| e.to_string())?,
        None => WasmConfig::default(),
    };
    let deadline_duration = parse_deadline_ms(wasm_cfg.deadline_ms)?;
    let cache_key = build_cache_key(&wasm_cfg)?;
    Ok((wasm_cfg, deadline_duration, cache_key))
}

/// Build an engine-level [`Config`] from a parsed [`WasmConfig`].
///
/// Consumes `wasm_cfg` so `classifier_id` and `corrections` move
/// into the resulting `Config` rather than being cloned —
/// non-trivial savings when a caller passes a large corrections
/// map. Called only on engine cache miss inside [`with_engine`];
/// cache-hit calls drop `wasm_cfg` (and its `corrections` HashMap)
/// without ever invoking this function.
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

/// Cache-relevant projection of [`WasmConfig`]. Serialized to build
/// the engine cache key — `deadline_ms` is excluded; `corrections`
/// is projected through a `BTreeMap` so iteration order is stable
/// (HashMap iteration is non-deterministic and would produce
/// different cache-key strings for byte-equal content).
#[derive(Serialize)]
struct WasmConfigCacheKey<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    classifier_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence_threshold: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    corrections: Option<BTreeMap<&'a str, &'a str>>,
}

/// Build the engine cache key for a parsed [`WasmConfig`].
///
/// Returns `Ok(None)` when no cache-relevant field is set — this
/// includes both `WasmConfig::default()` and configurations whose
/// only signal is an empty `corrections` map (`Some({})` → treated
/// as `None` so callers don't get a separate cache slot for
/// "default with empty corrections" vs. "absent corrections"). A
/// deadline-only invocation hits the same cache slot as a no-config
/// invocation.
fn build_cache_key(cfg: &WasmConfig) -> Result<Option<String>, String> {
    let corrections_present = cfg.corrections.as_ref().is_some_and(|c| !c.is_empty());
    if cfg.classifier_id.is_none() && cfg.confidence_threshold.is_none() && !corrections_present {
        return Ok(None);
    }
    let projection = WasmConfigCacheKey {
        classifier_id: cfg.classifier_id.as_deref(),
        confidence_threshold: cfg.confidence_threshold,
        corrections: cfg.corrections.as_ref().filter(|c| !c.is_empty()).map(|c| {
            // Project HashMap → BTreeMap for stable iteration order.
            // Borrowed (&str, &str) pairs avoid String allocations.
            c.iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect::<BTreeMap<_, _>>()
        }),
    };
    serde_json::to_string(&projection)
        .map(Some)
        .map_err(|e| e.to_string())
}

/// Validate a JS-side `deadline_ms` value and convert to `Duration`.
///
/// Rules (T041):
/// - `None` → `Ok(None)`. No deadline.
/// - Negative, NaN, or Inf → `Err`. JS callers should never construct
///   these; rejecting them loudly catches a serialization or
///   transformation bug before it reaches the engine.
/// - Otherwise → `Ok(Some(Duration::from_millis(value as u64)))`.
///   The `f64 as u64` cast truncates the fractional component
///   (`1.7` → `1`) and saturates above `u64::MAX` to `u64::MAX`. We
///   accept fractional millisecond inputs (rounding toward zero) so
///   JS callers building from `Date.now() / 4` style arithmetic
///   don't have to round before passing; if a future tighter
///   contract requires whole-millisecond inputs only, add an
///   `ms.fract() != 0.0` rejection here. The saturated `u64::MAX`
///   case is handled at the call site by `Instant::now().checked_add(d)`,
///   which surfaces the overflow as a JS error rather than panicking.
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
/// to a structured error (matches the CLI's `stamp_deadline` helper).
/// The user-controlled `deadline_ms` could in principle be a value
/// that, when added to the current Instant, overflows the platform
/// monotonic clock — `Instant::add` panics on overflow, so we use
/// `checked_add` and surface the failure as a JS error instead.
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
///
/// The hot path (cache hit) is an `Option<String>` comparison and a
/// `RefCell` borrow — no JSON parse beyond the upfront one in
/// `parse_wasm_config`, no `Config` construction, no AhoCorasick
/// rebuild. `build_config` is invoked only on cache miss; on cache
/// hit the closure is dropped without being called, releasing any
/// owned `corrections` HashMap without a move.
///
/// `cache_key` is the normalized projection produced by
/// [`build_cache_key`] — engine-relevant fields only, `deadline_ms`
/// excluded — so a caller varying `deadline_ms` per call does not
/// invalidate the cache.
///
/// Uses `try_borrow_mut` to recover gracefully if a prior WASM trap
/// left the RefCell in a borrowed state (WASM traps don't unwind, so
/// `borrow_mut` guards are never released on panic).
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

// ---------------------------------------------------------------------------
// Native-callable entry points (for parity tests — no JsValue dependency).
// ---------------------------------------------------------------------------

/// Pre-warm the engine cache (native entry point for tests).
pub fn configure_native(config_json: Option<String>) -> Result<(), String> {
    let (wasm_cfg, _, cache_key) = parse_wasm_config(&config_json)?;
    with_engine(
        &cache_key,
        move || build_engine_config(wasm_cfg),
        |_| Ok(()),
    )
}

/// Lint text, returning NDJSON conforming to `contracts/diagnostic.json`.
/// One diagnostic per line, newline-terminated. Byte-identical to the CLI's
/// `--format json` output (SC-008) — the truncation case (deadline tripped
/// mid-pass) returns whatever partial NDJSON the engine produced before
/// abort, exactly matching the CLI's stdout shape on the same condition.
///
/// Spec 005 §R3 / Constitution III analysis (T043): when `config_json`
/// carries `deadline_ms`, the engine's per-candidate deadline check
/// activates and the lint pass cooperatively aborts on expiry. This is
/// a *runtime budget cap*, not a vocabulary or scoring change — the
/// same recognizer codepath runs whether `deadline_ms` is set or not,
/// posteriors are identical, the CVE token set is unchanged. Permitted
/// under the Constitution III "data already present in the strict-path
/// codepath" carve-out.
pub fn lint_native(text: &str, config_json: Option<String>) -> Result<String, String> {
    // Parse upfront to fail fast on a bad `deadline_ms` (NaN / Inf /
    // negative) before any engine work, regardless of whether the
    // engine cache is warm. The cache key strips `deadline_ms` so a
    // caller varying the budget per call hits the warm cache.
    let (wasm_cfg, deadline_duration, cache_key) = parse_wasm_config(&config_json)?;
    let deadline = stamp_deadline(deadline_duration)?;
    with_engine(
        &cache_key,
        move || build_engine_config(wasm_cfg),
        |engine| {
            let mut lint_opts = LintOptions::default();
            lint_opts.deadline = deadline;
            let result = engine.lint_with_options(text.as_bytes(), &lint_opts);

            // Write NDJSON directly into a byte buffer — avoids the intermediate
            // String allocation that serde_json::to_string produces per diagnostic.
            let mut buf = Vec::with_capacity(result.diagnostics.len() * 256);
            for d in &result.diagnostics {
                serde_json::to_writer(&mut buf, &diagnostic_to_json(d))
                    .map_err(|e| e.to_string())?;
                buf.push(b'\n');
            }
            // serde_json always produces valid UTF-8.
            String::from_utf8(buf).map_err(|e| e.to_string())
        },
    )
}

/// Fix text, returning a JSON object with `fixed_text`, `applied` audit records,
/// and `remaining` diagnostics.
///
/// The `threshold` parameter always takes precedence over any `confidence_threshold`
/// in `config_json`. This matches the CLI's Layer 4 (CLI flag) override behavior.
///
/// Spec 005 §R4: when `config_json` carries `deadline_ms` and the
/// deadline expires during the lint or fix-application pass, this
/// function returns `Err(...)` carrying a JSON-serialized
/// `DeadlineExceededBody` (identical shape to the server's 504
/// response — `truncated_by`, `diagnostics`, `candidates_processed`,
/// `candidates_total`). JS callers `try`/`catch` and parse the
/// message body to render the partial-lint diagnostics. No partial
/// `FixResult` is ever returned (Constitution V Principle V).
pub fn fix_native(
    text: &str,
    threshold: f32,
    config_json: Option<String>,
) -> Result<String, String> {
    let (wasm_cfg, deadline_duration, cache_key) = parse_wasm_config(&config_json)?;
    let deadline = stamp_deadline(deadline_duration)?;
    with_engine(
        &cache_key,
        move || build_engine_config(wasm_cfg),
        |engine| {
            let mut fix_opts = FixOptions::default();
            fix_opts.threshold_override = Some(threshold);
            fix_opts.deadline = deadline;
            let result = match engine.fix_with_options(text.as_bytes(), FixMode::Apply, &fix_opts) {
                Ok(r) => r,
                Err(EngineError::DeadlineExceeded { partial_lint }) => {
                    return Err(deadline_exceeded_payload(&partial_lint));
                }
                Err(e) => return Err(e.to_string()),
            };

            let fixed_text = String::from_utf8(result.source)
                .map_err(|e| format!("invalid UTF-8 in fix output: {e}"))?;

            let applied: Vec<Box<serde_json::value::RawValue>> = result
                .applied
                .iter()
                .map(serialize_applied_fix)
                .collect::<Result<_, _>>()?;

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
                r002_fired: result.r002_fired,
            };

            // Serialize directly into a byte buffer to avoid serde_json::to_string's
            // intermediate String allocation.
            let mut buf = Vec::with_capacity(1024);
            serde_json::to_writer(&mut buf, &fix_result).map_err(|e| e.to_string())?;
            String::from_utf8(buf).map_err(|e| e.to_string())
        },
    )
}

/// Body shape for a deadline-exceeded fix error (mirrors the
/// `marque-server::DeadlineExceededBody` 504 response). Embedded as a
/// JSON string in the `Err` arm of `fix_native` so JS callers can
/// `JSON.parse(error.message)` to recover the partial-lint
/// diagnostics + candidate counts.
#[derive(Serialize)]
struct DeadlineExceededBodyJson<'a> {
    truncated_by: &'static str,
    candidates_processed: usize,
    candidates_total: usize,
    diagnostics: Vec<DiagnosticJson<'a>>,
}

/// Fallback payload when the primary serialization fails. Carries
/// only the `truncated_by` discriminator and an `error` message —
/// no diagnostics, no counts. Serialized via `serde_json::to_string`
/// so the `error` field is correctly JSON-escaped if the inner
/// message happens to contain quotes or backslashes (e.g., a
/// `serde_json::Error` formatted with a path that includes those
/// characters).
#[derive(Serialize)]
struct DeadlineExceededFallback<'a> {
    truncated_by: &'static str,
    error: &'a str,
}

fn deadline_exceeded_payload(partial_lint: &marque_engine::LintResult) -> String {
    let truncated_by = if partial_lint.truncated {
        "lint"
    } else {
        "fix"
    };
    let body = DeadlineExceededBodyJson {
        truncated_by,
        candidates_processed: partial_lint.candidates_processed,
        candidates_total: partial_lint.candidates_total,
        diagnostics: partial_lint
            .diagnostics
            .iter()
            .map(diagnostic_to_json)
            .collect(),
    };
    // The primary path serializes a struct of basic types; serde_json
    // failure here would imply a fundamental serializer bug. The
    // fallback exists for defense-in-depth — and crucially, it
    // round-trips through `serde_json::to_string` so the `error`
    // field is properly JSON-escaped. A `format!(r#"..."{e}"..."#)`
    // would produce invalid JSON if `e` contained a quote or
    // backslash; JS callers parsing the message as JSON would then
    // see a parse error instead of the structured shape we promised.
    match serde_json::to_string(&body) {
        Ok(s) => s,
        Err(primary_err) => {
            let fallback = DeadlineExceededFallback {
                truncated_by,
                error: &primary_err.to_string(),
            };
            // If even this micro-payload fails to serialize, return a
            // hand-built constant — no interpolation, no escaping
            // hazards. We accept losing the original error message in
            // this terminal-case-of-a-terminal-case path.
            serde_json::to_string(&fallback).unwrap_or_else(|_| {
                r#"{"truncated_by":"fix","error":"deadline-exceeded payload serialization failed"}"#
                    .to_owned()
            })
        }
    }
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
    let (wasm_cfg, _, cache_key) = parse_wasm_config(&config_json)?;

    with_engine(
        &cache_key,
        move || build_engine_config(wasm_cfg),
        |engine| {
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
                            serde_json::value::RawValue::from_string(json)
                                .map_err(|e| e.to_string())
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
        },
    )
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
    configure_native(config_json).map_err(|e| JsValue::from_str(&e))
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
            // PR-3a transitional adapter: parser produces ParsedAttrs<'src>;
            // PageContext stores CanonicalAttrs. This site is a known
            // exception to the "engine-owned adapter" principle (the
            // function is documented as "Does NOT run the rules engine"
            // and predates the keystone window — callers reach for it
            // when they want banner roll-up without rule dispatch).
            // PR 3c retires `from_parsed_unchecked` in favor of
            // `MarkingScheme::canonicalize`; this call migrates then.
            // FR-040 lint whitelists the call site.
            page_context.add_portion(marque_ism::from_parsed_unchecked(parsed.attrs));
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
///    EO 13526, section 1.5(a) (the default duration of original
///    classification when no other instruction is present, restated in
///    CAPCO-2016 §E.1 p31).
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
            // PR-3a transitional adapter: parser produces ParsedAttrs<'src>;
            // downstream consumers want CanonicalAttrs. This site is a
            // known exception to the "engine-owned adapter" principle —
            // CAB-line generation predates the keystone window and runs
            // outside the rules engine on purpose. PR 3c retires
            // `from_parsed_unchecked` in favor of
            // `MarkingScheme::canonicalize`; this call migrates then.
            // FR-040 lint whitelists the call site.
            let attrs = marque_ism::from_parsed_unchecked(parsed.attrs);
            if found_declass_date.is_none() {
                if let Some(date) = &attrs.declassify_on {
                    // `to_maxdate_str()` always returns 8-digit YYYYMMDD:
                    // Year(y) → "{y}1231", YearMonth(y,m) → last day of month,
                    // Date / DateHourMin / DateTime → YYYYMMDD of the date component.
                    // This is the format expected on a CAB "Declassify On:" line.
                    found_declass_date = Some(date.to_maxdate_str().into());
                }
            }
            if found_declass_exemption.is_none() {
                if let Some(ex) = attrs.declass_exemption {
                    found_declass_exemption = Some(ex.as_str().to_owned());
                }
            }
            if candidate.kind == MarkingType::Portion {
                page_context.add_portion(attrs);
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

#[cfg(test)]
mod tests {
    use super::{WasmConfig, build_cache_key, current_year, parse_deadline_ms};
    use std::time::Duration;

    // -----------------------------------------------------------------------
    // parse_deadline_ms
    // -----------------------------------------------------------------------

    #[test]
    fn parse_deadline_ms_none_yields_none() {
        assert_eq!(parse_deadline_ms(None).unwrap(), None);
    }

    #[test]
    fn parse_deadline_ms_zero_yields_zero_duration() {
        assert_eq!(parse_deadline_ms(Some(0.0)).unwrap(), Some(Duration::ZERO));
    }

    #[test]
    fn parse_deadline_ms_positive_rounds_down() {
        // 1.7 ms → truncated to 1 ms (f64 as u64 truncates toward zero).
        assert_eq!(
            parse_deadline_ms(Some(1.7)).unwrap(),
            Some(Duration::from_millis(1))
        );
    }

    #[test]
    fn parse_deadline_ms_negative_returns_error() {
        let err = parse_deadline_ms(Some(-1.0)).unwrap_err();
        assert!(
            err.contains("non-negative"),
            "error must mention non-negative constraint, got: {err}"
        );
    }

    #[test]
    fn parse_deadline_ms_nan_returns_error() {
        let err = parse_deadline_ms(Some(f64::NAN)).unwrap_err();
        assert!(
            !err.is_empty(),
            "NaN deadline must produce a non-empty error"
        );
    }

    #[test]
    fn parse_deadline_ms_positive_infinity_returns_error() {
        let err = parse_deadline_ms(Some(f64::INFINITY)).unwrap_err();
        assert!(
            !err.is_empty(),
            "+Inf deadline must produce a non-empty error"
        );
    }

    #[test]
    fn parse_deadline_ms_negative_infinity_returns_error() {
        let err = parse_deadline_ms(Some(f64::NEG_INFINITY)).unwrap_err();
        assert!(
            !err.is_empty(),
            "-Inf deadline must produce a non-empty error"
        );
    }

    #[test]
    fn parse_deadline_ms_large_value_saturates_without_panic() {
        // A very large finite f64 saturates to u64::MAX; checked_add in
        // stamp_deadline handles the overflow. The important thing here is
        // that parse_deadline_ms itself does not panic.
        let result = parse_deadline_ms(Some(f64::MAX));
        assert!(result.is_ok(), "very large deadline must not panic");
    }

    // -----------------------------------------------------------------------
    // build_cache_key
    // -----------------------------------------------------------------------

    fn default_wasm_config() -> WasmConfig {
        WasmConfig {
            classifier_id: None,
            confidence_threshold: None,
            corrections: None,
            deadline_ms: None,
        }
    }

    #[test]
    fn build_cache_key_is_none_for_default_config() {
        let cfg = default_wasm_config();
        assert_eq!(
            build_cache_key(&cfg).unwrap(),
            None,
            "default config must produce None cache key (uses the default engine slot)"
        );
    }

    #[test]
    fn build_cache_key_is_none_for_empty_corrections() {
        let cfg = WasmConfig {
            corrections: Some(Default::default()),
            ..default_wasm_config()
        };
        assert_eq!(
            build_cache_key(&cfg).unwrap(),
            None,
            "empty corrections map must produce None cache key (same slot as default)"
        );
    }

    #[test]
    fn build_cache_key_is_none_for_deadline_only() {
        // deadline_ms is intentionally excluded from the cache key — a caller
        // varying the per-call budget must not cause an engine rebuild.
        let cfg = WasmConfig {
            deadline_ms: Some(5000.0),
            ..default_wasm_config()
        };
        assert_eq!(
            build_cache_key(&cfg).unwrap(),
            None,
            "deadline_ms alone must NOT produce a distinct cache key"
        );
    }

    #[test]
    fn build_cache_key_is_some_for_classifier_id() {
        let cfg = WasmConfig {
            classifier_id: Some("TEST-WASM-42".to_owned()),
            ..default_wasm_config()
        };
        assert!(
            build_cache_key(&cfg).unwrap().is_some(),
            "classifier_id must produce a non-None cache key"
        );
    }

    #[test]
    fn build_cache_key_is_some_for_confidence_threshold() {
        let cfg = WasmConfig {
            confidence_threshold: Some(0.75),
            ..default_wasm_config()
        };
        assert!(
            build_cache_key(&cfg).unwrap().is_some(),
            "confidence_threshold must produce a non-None cache key"
        );
    }

    #[test]
    fn build_cache_key_is_some_for_nonempty_corrections() {
        let cfg = WasmConfig {
            corrections: Some(
                [("NF".to_owned(), "NOFORN".to_owned())]
                    .into_iter()
                    .collect(),
            ),
            ..default_wasm_config()
        };
        assert!(
            build_cache_key(&cfg).unwrap().is_some(),
            "non-empty corrections map must produce a non-None cache key"
        );
    }

    #[test]
    fn build_cache_key_is_stable_for_equal_corrections() {
        // Two configs with the same corrections content must produce the same
        // cache-key string regardless of HashMap insertion order.
        use std::collections::HashMap;
        let mut m1: HashMap<String, String> = HashMap::new();
        m1.insert("NF".to_owned(), "NOFORN".to_owned());
        m1.insert("SI".to_owned(), "SPECIAL INTELLIGENCE".to_owned());

        let mut m2: HashMap<String, String> = HashMap::new();
        m2.insert("SI".to_owned(), "SPECIAL INTELLIGENCE".to_owned());
        m2.insert("NF".to_owned(), "NOFORN".to_owned());

        let k1 = build_cache_key(&WasmConfig {
            corrections: Some(m1),
            ..default_wasm_config()
        })
        .unwrap();

        let k2 = build_cache_key(&WasmConfig {
            corrections: Some(m2),
            ..default_wasm_config()
        })
        .unwrap();

        assert_eq!(
            k1, k2,
            "byte-equal corrections content must produce identical cache keys \
             regardless of HashMap iteration order (BTreeMap projection)"
        );
    }

    #[test]
    fn build_cache_key_differs_for_different_classifier_ids() {
        let k1 = build_cache_key(&WasmConfig {
            classifier_id: Some("TEST-WASM-42".to_owned()),
            ..default_wasm_config()
        })
        .unwrap();

        let k2 = build_cache_key(&WasmConfig {
            classifier_id: Some("TEST-CLASSIFIER-42".to_owned()),
            ..default_wasm_config()
        })
        .unwrap();

        assert_ne!(
            k1, k2,
            "different classifier_ids must produce different cache keys"
        );
    }

    // -----------------------------------------------------------------------
    // current_year and SECONDS_PER_JULIAN_YEAR
    // -----------------------------------------------------------------------

    #[test]
    fn seconds_per_julian_year_constant_is_correct() {
        // 365.25 days × 24 h × 3600 s = 31,557,600 s
        let expected = (365.25_f64 * 24.0 * 3600.0) as u64;
        assert_eq!(
            super::SECONDS_PER_JULIAN_YEAR,
            expected,
            "SECONDS_PER_JULIAN_YEAR must equal 365.25 × 24 × 3600"
        );
    }

    #[test]
    fn current_year_is_plausible() {
        let year = current_year();
        assert!(
            year >= 2026,
            "current_year must be ≥ 2026 (codebase inception year), got {year}"
        );
        assert!(
            year <= 2100,
            "current_year must be ≤ 2100 (sanity upper bound), got {year}"
        );
    }
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
