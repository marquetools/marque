// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Decision-tracing overhead benchmark.
//!
//! Pins the constitutional invariant the `decision-tracing` feature
//! exists to satisfy: enabling the feature MUST NOT push the engine off
//! the SC-001 interactive-latency floor, whether or not an observer is
//! installed. The gate asserts both arms of this binary clear the 16 ms
//! ceiling absolutely (see "How the gate is checked" below).
//!
//! [`marque_scheme::NoopSink`] is a ZST with an `#[inline(always)]`
//! empty body, boxed behind `Mutex<Box<dyn SyncDecisionSink>>` on the
//! engine field. Before the `tracing_active` short-circuit, every
//! `emit()` call on the default-NoopSink path still paid three residual
//! operations — `AtomicU32::fetch_add`, `Mutex::lock`, and one vtable
//! call to the empty `record` body — ×~300/10KB doc. `Engine::emit` and
//! the per-rule emission blocks now early-return when
//! `tracing_active == false` (no observer installed), so the
//! default-NoopSink path skips all three and the per-page projection
//! takes the plain (non-sink-aware) path identical to the OFF-feature
//! build.
//!
//! ## Bench pair
//!
//! Both functions in this binary are gated behind
//! `required-features = ["decision-tracing"]` (declared in
//! `crates/engine/Cargo.toml`), so they run only when the feature is
//! enabled.
//!
//! - `decision_tracing_overhead_baseline` — `Engine::lint(...)` with
//!   no `with_decision_sink` call. The engine carries its default
//!   `NoopSink` behind the `Mutex<Box<dyn SyncDecisionSink>>` field.
//!   This is the production hot path when the feature is enabled but
//!   no observer is installed.
//!
//! - `decision_tracing_overhead_with_recording_sink` —
//!   `Engine::lint(...)` with [`marque_scheme::RecordingSink`]
//!   installed. The recording sink allocates and pushes every event.
//!   This is the worst-case per-call cost of a real observer; its
//!   mean is informational, not gated.
//!
//! ## How the gate is checked
//!
//! `scripts/bench-check.sh::check_decision_tracing_overhead` runs this
//! binary ONCE with `--features decision-tracing` (no per-bench filter,
//! so both functions run in one invocation under identical machine
//! conditions) and asserts the upper-CI of BOTH
//! `decision_tracing_overhead_baseline` and
//! `decision_tracing_overhead_with_recording_sink` is at or under the
//! SC-001 16 ms ceiling (`benches/baseline.json`
//! `decision_tracing_overhead.target_upper_ci_us = 16000`).
//!
//! This replaced a prior ≤2% ratio of `_baseline` against the
//! feature-OFF `lint_latency::lint_10kb`. "Feature compiled in vs
//! compiled out" has no same-binary analogue, so that comparison was
//! irreducibly cross-binary AND cross-invocation — on WSL2-class CI the
//! run-to-run noise was ±~9% (the feature-ON/NoopSink path measured
//! faster than no-feature on 2 of 3 identical runs), so a 2% gate flaked
//! regardless of code. The shared hot path stays tightly regression-
//! gated by `lint_10kb`; this gate guards that the feature-ON paths stay
//! interactive. The two arms still pin `.with_strict_recognizer()` to
//! keep the measurement on the strict path (PR #811 first-run measured a
//! 36 % default-recognizer vs strict-pinned gap on the same input).
//!
//! ## Bench config
//!
//! Sample size and measurement time mirror `deadline_overhead.rs`
//! exactly (sample_size=500, measurement_time=10s) — that config keeps
//! the per-arm confidence interval tight and the measurement comparable
//! to the sibling latency benches. The gate is absolute (each arm vs the
//! 16 ms ceiling), so it does not depend on cross-run noise the way the
//! retired 2% ratio did, but a narrow CI still makes the upper-CI bound
//! the parser reads a stable number.
//!
//! Reference baseline: x86_64 ≥ 3.0 GHz single-thread, warm cache,
//! `--release` build, no tracing subscriber.

use criterion::{Criterion, criterion_group, criterion_main};
use marque_config::Config;
use marque_engine::Engine;
use marque_scheme::RecordingSink;
use std::hint::black_box;
use std::time::Duration;

/// Build the same 10 KB representative input the deadline-overhead and
/// interactive-latency benches use. Duplicated rather than shared
/// across bench crates because Criterion bench files don't have a
/// shared-module discipline; the cost is ~30 lines for measurement
/// shape parity.
fn build_representative_input(target_bytes: usize) -> Vec<u8> {
    let block = concat!(
        "TOP SECRET//SCI//NOFORN\n",
        "\n",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do\n",
        "eiusmod tempor incididunt ut labore et dolore magna aliqua.\n",
        "\n",
        "(S//NF) This portion contains abbreviated dissemination controls.\n",
        "\n",
        "SECRET//NOFORN//REL TO USA, GBR\n",
        "\n",
        "Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.\n",
        "\n",
        "(TS//SI) Another portion with SCI controls and valid formatting.\n",
        "\n",
    );

    let block_bytes = block.as_bytes();
    let mut input = Vec::with_capacity(target_bytes + block_bytes.len());
    while input.len() < target_bytes {
        input.extend_from_slice(block_bytes);
    }
    let complete_blocks = target_bytes / block_bytes.len();
    input.truncate(complete_blocks.max(1) * block_bytes.len());
    input.resize(target_bytes, b' ');
    input
}

fn decision_tracing_overhead_baseline(c: &mut Criterion) {
    let input = build_representative_input(10_000);
    // Pin the strict recognizer to match `lint_latency::lint_10kb`'s
    // engine configuration. The ratio gate in
    // `scripts/bench-check.sh::check_decision_tracing_overhead` compares
    // this bench's mean against `lint_10kb`'s mean to isolate the
    // decision-tracing feature's per-call cost. Both engines MUST use
    // the same recognizer or the ratio conflates recognizer-dispatch
    // overhead (~36 % delta observed in CI on PR #811 first run) with
    // decision-tracing overhead.
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
    .with_strict_recognizer();

    c.bench_function("decision_tracing_overhead_baseline", |b| {
        b.iter(|| engine.lint(black_box(&input)));
    });
}

fn decision_tracing_overhead_with_recording_sink(c: &mut Criterion) {
    let input = build_representative_input(10_000);
    // Same strict-recognizer pinning rationale as
    // `decision_tracing_overhead_baseline` above — keeps both functions
    // in this bench file on identical engine config so the
    // recording-sink/noop-sink delta is the only variable.
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
    .with_strict_recognizer()
    .with_decision_sink(RecordingSink::new());

    c.bench_function("decision_tracing_overhead_with_recording_sink", |b| {
        b.iter(|| engine.lint(black_box(&input)));
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(500)
        .measurement_time(Duration::from_secs(10));
    targets =
        decision_tracing_overhead_baseline,
        decision_tracing_overhead_with_recording_sink
}
criterion_main!(benches);
