// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Spec 005 Phase 3c — T044: byte-identical NDJSON parity between
//! `lint_native(text, deadline_ms)` (the WASM-target entry point
//! exercised here on native) and the equivalent native engine call
//! `Engine::lint_with_options(text, &LintOptions { deadline: Some(_) })`.
//!
//! The non-deadline parity is already pinned at full corpus scope by
//! `tests/native_parity.rs` (SC-008). This file extends that contract
//! to the deadline path: a configured `deadline_ms` MUST NOT alter the
//! NDJSON shape relative to a deadline-free call on the same fixture
//! when the deadline does not trip, and a pre-expired deadline MUST
//! produce byte-equal empty NDJSON on both sides.
//!
//! Tested deterministically:
//!
//! 1. Generous deadline (60 s) on a small fixture — neither path
//!    truncates, both produce the same full NDJSON, and that NDJSON
//!    matches the deadline-free baseline byte-for-byte.
//! 2. Zero-millisecond deadline — both paths trip the per-pass
//!    deadline check on entry, both produce empty NDJSON.
//!
//! Mid-pass truncation parity is intentionally NOT tested here:
//! native and WASM each stamp `Instant::now()` independently, so the
//! exact cutoff candidate varies between the two paths. The parity
//! invariant we care about is "same shape, same vocabulary, same
//! schema" — which the two cases above pin without depending on
//! identical clock reads.

#![cfg(not(target_arch = "wasm32"))]

use marque_config::Config;
use marque_engine::{Engine, LintOptions};
use marque_rules::Diagnostic;
use serde::Serialize;
use std::time::{Duration, Instant};

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

// Native-side projection used to render the NDJSON expected from the
// engine — matches the shape `marque-wasm` and `marque/src/render.rs`
// produce, so any divergence between the three surfaces SC-008 fails
// here (mirrors `tests/native_parity.rs`).

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

fn diag_to_json(d: &Diagnostic) -> DiagnosticJson<'_> {
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
            source: match f.source {
                marque_rules::FixSource::BuiltinRule => "BuiltinRule",
                marque_rules::FixSource::CorrectionsMap => "CorrectionsMap",
                marque_rules::FixSource::MigrationTable => "MigrationTable",
                marque_rules::FixSource::DecoderPosterior => "DecoderPosterior",
                marque_rules::FixSource::DecoderClassificationHeuristic => {
                    "DecoderClassificationHeuristic"
                }
            },
            replacement: f.replacement.as_ref(),
            confidence: f.confidence.combined(),
            migration_ref: f.migration_ref,
        }),
    }
}

fn render_ndjson(diagnostics: &[Diagnostic]) -> String {
    let mut buf = Vec::with_capacity(diagnostics.len() * 256);
    for d in diagnostics {
        serde_json::to_writer(&mut buf, &diag_to_json(d)).expect("infallible: writing to Vec");
        buf.push(b'\n');
    }
    String::from_utf8(buf).expect("serde_json output is valid UTF-8")
}

#[test]
fn wasm_deadline_ms_generous_matches_native_full_lint() {
    // 60 000 ms is well above any real lint runtime on the small
    // fixture — neither path will truncate. The deadline path MUST
    // produce the same NDJSON as the deadline-free baseline.
    let fixture = "(S//NF) Sentence one. (TS//NF) Sentence two. (U) Sentence three.\n";

    // Native-side baseline: no deadline.
    let baseline_native = {
        let e = engine();
        let result = e.lint(fixture.as_bytes());
        render_ndjson(&result.diagnostics)
    };

    // Native-side with generous deadline.
    let deadline_native = {
        let e = engine();
        let mut opts = LintOptions::default();
        opts.deadline = Some(Instant::now() + Duration::from_secs(60));
        let result = e.lint_with_options(fixture.as_bytes(), &opts);
        assert!(
            !result.truncated,
            "60-second deadline must not truncate the small fixture"
        );
        render_ndjson(&result.diagnostics)
    };

    // WASM-target entry point with the same generous deadline via JSON config.
    let wasm_with_deadline = {
        let cfg = r#"{"deadline_ms": 60000}"#;
        marque_wasm::lint_native(fixture, Some(cfg.to_owned())).expect("lint_native")
    };

    // WASM-target entry point without any deadline (sanity baseline).
    let wasm_no_deadline = marque_wasm::lint_native(fixture, None).expect("lint_native");

    // All four paths must produce the same NDJSON byte-for-byte.
    assert_eq!(
        baseline_native, deadline_native,
        "generous deadline must not perturb native NDJSON"
    );
    assert_eq!(
        wasm_no_deadline, wasm_with_deadline,
        "generous deadline_ms must not perturb WASM NDJSON"
    );
    assert_eq!(
        baseline_native, wasm_with_deadline,
        "WASM lint_native(deadline_ms=60000) must equal native engine.lint() byte-for-byte (SC-008)"
    );
}

#[test]
fn wasm_deadline_ms_zero_yields_empty_ndjson_byte_identical_to_native() {
    // `deadline_ms: 0` stamps `Instant::now() + 0ms`. By the time the
    // engine reads `Instant::now()` for the pre-pass check, more than
    // 0 ns has elapsed — deadline is in the past. Both native and
    // WASM trip the pre-pass check on entry, return
    // `truncated: true` with an empty diagnostic vector, and render
    // the empty NDJSON. Byte-identical "" on both sides.
    let fixture = "(S//NF) Banner that would normally fire E-something.\n";

    let native_empty = {
        let e = engine();
        let mut opts = LintOptions::default();
        opts.deadline = Some(Instant::now() + Duration::from_millis(0));
        let result = e.lint_with_options(fixture.as_bytes(), &opts);
        // Note: the deadline may not trip on the pre-pass check
        // exactly — `Instant::now()` after stamping might still be
        // before the deadline if no time elapsed. In that case the
        // per-candidate check trips on the first candidate, which
        // also produces zero diagnostics for this small fixture.
        // Either way the NDJSON is empty.
        render_ndjson(&result.diagnostics)
    };

    let wasm_empty = {
        let cfg = r#"{"deadline_ms": 0}"#;
        marque_wasm::lint_native(fixture, Some(cfg.to_owned())).expect("lint_native")
    };

    assert_eq!(
        native_empty, wasm_empty,
        "WASM and native must produce byte-identical empty NDJSON on a zero-ms deadline"
    );
    assert!(
        native_empty.is_empty(),
        "zero-ms deadline must produce empty NDJSON; got: {native_empty:?}"
    );
}

#[test]
fn wasm_corrections_cache_key_is_stable_across_calls() {
    // The cache key MUST be byte-stable for byte-equal corrections
    // content regardless of HashMap iteration order — otherwise the
    // engine cache invalidates on every call. We can't directly
    // observe the cache-key string from the public surface, but we
    // can pin the load-bearing post-condition: sequential calls
    // with identical-content `corrections` produce byte-identical
    // NDJSON, exercising the BTreeMap projection in
    // `build_cache_key`. If the cache key were HashMap-order
    // sensitive, calls would alternate between cache hit and cache
    // miss; the cache-miss path is functionally correct (rebuilds
    // the engine) but slower — this test pins *correctness*, the
    // perf invariant is documented in the `build_cache_key` doc
    // comment.
    let cfg = r#"{"corrections":{"NF":"NOFORN","SI":"SPECIAL INTELLIGENCE","TS":"TOP SECRET"}}"#;
    let fixture = "(S//NF) Sentence one.\n";

    let first = marque_wasm::lint_native(fixture, Some(cfg.to_owned())).expect("lint first");
    let second = marque_wasm::lint_native(fixture, Some(cfg.to_owned())).expect("lint second");
    let third = marque_wasm::lint_native(fixture, Some(cfg.to_owned())).expect("lint third");

    assert_eq!(
        first, second,
        "consecutive identical calls must produce identical NDJSON"
    );
    assert_eq!(
        first, third,
        "third identical call must produce identical NDJSON"
    );
}

#[test]
fn wasm_empty_corrections_hits_default_cache_slot() {
    // `Some({})` for corrections must be treated equivalently to
    // `None` for cache-key purposes — otherwise a caller passing
    // `{"corrections": {}}` gets a separate cache slot from a
    // caller passing nothing, doubling engine construction cost
    // for no observable benefit. The `build_cache_key` doc comment
    // pins this invariant.
    let no_config = marque_wasm::lint_native("(U)\n", None).expect("lint no config");
    let empty_corrections =
        marque_wasm::lint_native("(U)\n", Some(r#"{"corrections":{}}"#.to_owned()))
            .expect("lint empty corrections");
    assert_eq!(
        no_config, empty_corrections,
        "empty corrections map must produce the same NDJSON as no config — \
         both should hit the default-cache slot"
    );
}

#[test]
fn wasm_deadline_ms_does_not_invalidate_engine_cache() {
    // The cache key produced by `parse_wasm_config` MUST exclude
    // `deadline_ms` so a caller varying the per-call budget does not
    // pay the AhoCorasick / ruleset / scheme rebuild cost on every
    // call.
    //
    // We cannot directly observe cache hits from the public surface,
    // but we can observe the post-condition: NDJSON output is
    // byte-identical across calls that should hit the same cached
    // engine. A regression that invalidated the cache on every
    // `deadline_ms` change would still produce identical NDJSON
    // (engine behavior doesn't depend on the cache), so this test
    // is a partial pin: it verifies behavior is preserved, not that
    // the cache is hot. The cache-key contract itself is enforced by
    // the unit-level shape of `build_cache_key` (deadline_ms field
    // is absent from `WasmConfigCacheKey`).
    marque_wasm::configure_native(None).expect("pre-warm");

    let fixture = "(S//NF) Sentence one. (TS//SI) Sentence two.\n";
    let with_deadline_a =
        marque_wasm::lint_native(fixture, Some(r#"{"deadline_ms": 1000}"#.to_owned()))
            .expect("lint with deadline_ms=1000");
    let with_deadline_b =
        marque_wasm::lint_native(fixture, Some(r#"{"deadline_ms": 2000}"#.to_owned()))
            .expect("lint with deadline_ms=2000");
    let no_deadline = marque_wasm::lint_native(fixture, None).expect("lint without deadline");

    assert_eq!(
        with_deadline_a, with_deadline_b,
        "varying deadline_ms across two generous-budget calls must produce identical NDJSON"
    );
    assert_eq!(
        with_deadline_a, no_deadline,
        "a generous deadline_ms must produce the same NDJSON as no deadline at all"
    );
}

#[test]
fn wasm_rejects_negative_deadline_ms() {
    // T041 validation — negative `deadline_ms` is rejected before any
    // engine work. JS callers should never construct this; rejecting
    // catches a serialization or transformation bug.
    let cfg = r#"{"deadline_ms": -1}"#;
    let err = marque_wasm::lint_native("(S//NF)", Some(cfg.to_owned()))
        .expect_err("negative deadline_ms must be rejected");
    assert!(
        err.contains("non-negative"),
        "error must explain the violation, got: {err}"
    );
}

#[test]
fn wasm_rejects_non_finite_deadline_ms() {
    // Defense in depth for T041 — non-finite `deadline_ms` values are
    // rejected before reaching the engine. There are two layers:
    //
    // 1. `serde_json` itself rejects `1e500` (overflows to f64::INFINITY)
    //    with "number out of range" at JSON-parse time. Plain JSON has no
    //    `NaN` / `Infinity` literals, so this is the most realistic
    //    attack surface.
    // 2. `parse_deadline_ms` rechecks `is_finite()` for any non-finite
    //    value that did somehow slip past serde (e.g., a future
    //    permissive-numbers feature flag, or a transformation layer
    //    constructing the f64 directly). Whichever layer trips first,
    //    the public surface returns `Err` and the engine is never
    //    invoked.
    //
    // The test asserts the rejection happens, not which layer rejects —
    // the contract from the JS caller's perspective is "non-finite
    // input → error string", regardless of where in the pipeline the
    // rejection lands.
    let cfg = r#"{"deadline_ms": 1e500}"#;
    let err = marque_wasm::lint_native("(S//NF)", Some(cfg.to_owned()))
        .expect_err("non-finite deadline_ms must be rejected");
    assert!(
        !err.is_empty(),
        "rejection must produce a non-empty error message"
    );
}
