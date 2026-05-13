// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 7b — C001 dual-path idempotency lock.
//!
//! Closes the L-1 forward obligation from `docs/refactor-006/pr-7a-rust-review.md`:
//! C001 is declared `Phase::Localized` (T074), and the engine's
//! pass-1 dispatch loop (PR 7b) would naively re-run every Localized
//! rule against the post-pass-0 buffer. The risk: a double-application
//! of C001 (once as pass-0 text-correction, once as pass-1 FixIntent)
//! would either splice the same bytes twice (corrupting the audit
//! log) or split a successful pass-0 correction into two audit
//! entries (inflating the apparent fix count).
//!
//! The mitigating property comes from C001's check body
//! (`crates/capco/src/rules.rs`'s `CorrectionsMapRule::check`): it
//! walks `attrs.token_spans`, looks up each token's text in
//! `ctx.corrections`, and emits a diagnostic ONLY when
//! `replacement != text` (the M2 no-op guard). After pass-0 rewrites
//! `SERCET → SECRET`, the re-lint produces token spans whose `.text`
//! reads `"SECRET"` — the lookup against the same corrections map
//! either misses entirely (typical) or hits with `replacement == text`
//! and the M2 guard drops it. Pass-1 dispatch of C001 produces zero
//! diagnostics by construction.
//!
//! This test pins that property at the engine boundary so a future
//! refactor that removes the M2 guard, or changes the corrections-map
//! key semantics, would fail visibly here.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};
use std::collections::HashMap;
use std::time::{Duration, UNIX_EPOCH};

const FIXED_TS: u64 = 1_700_000_000;

fn make_engine_with_correction(from: &str, to: &str) -> Engine {
    let mut corrections: HashMap<String, String> = HashMap::new();
    corrections.insert(from.to_owned(), to.to_owned());
    let mut config = Config::default();
    config.corrections = corrections;
    Engine::with_clock(
        config,
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

#[test]
fn c001_pass1_dispatch_noop_after_pass0() {
    // The load-bearing fixture: `(TS//SERCET//NF)` with a corrections
    // entry `"SERCET" → "SECRET"`. Pass-0 rewrites `SERCET` to
    // `SECRET`; pass-1 sees the corrected buffer, re-lints, and
    // C001 fires exactly ZERO additional times. The total C001
    // count in `result.applied` is exactly 1 — the pass-0 promotion.
    let engine = make_engine_with_correction("SERCET", "SECRET");
    let source = b"(TS//SERCET//NF)";
    let result = engine.fix(source, FixMode::Apply);

    let c001_count = result
        .applied
        .iter()
        .filter(|f| f.rule.as_str() == "C001")
        .count();
    assert_eq!(
        c001_count,
        1,
        "C001 fires exactly once (pass-0 only); pass-1 dispatch \
         is a no-op after pass-0 rewrote the source. Applied: {:?}",
        result
            .applied
            .iter()
            .map(|f| f.rule.as_str())
            .collect::<Vec<_>>()
    );
    // Sanity: the output buffer contains the corrected token.
    let out = String::from_utf8(result.source).unwrap();
    assert!(
        out.contains("SECRET"),
        "expected SECRET in corrected output, got: {out:?}"
    );
    assert!(
        !out.contains("SERCET"),
        "SERCET should be gone from corrected output, got: {out:?}"
    );
}

#[test]
fn c001_self_correction_filtered_at_pass0() {
    // Corrections entry where key == value (a no-op `"SECRET" →
    // "SECRET"`) is filtered out at engine construction time
    // (`CachedAhoCorasick` filter excludes `k == v` patterns). The
    // pass-0 path therefore never sees a self-correction to apply,
    // and there is no pass-1 dispatch issue to test here either.
    let engine = make_engine_with_correction("SECRET", "SECRET");
    let source = b"(TS//SECRET//NF)";
    let result = engine.fix(source, FixMode::Apply);

    let c001_count = result
        .applied
        .iter()
        .filter(|f| f.rule.as_str() == "C001")
        .count();
    assert_eq!(
        c001_count, 0,
        "self-correction must produce zero C001 applied fixes"
    );
}
