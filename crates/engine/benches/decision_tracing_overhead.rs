// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Decision-tracing overhead benchmark.
//!
//! Pins the constitutional invariant the `decision-tracing` feature
//! exists to satisfy: when the feature is ON and the engine's default
//! `NoopSink` is in place, the per-call cost MUST be within 2% of the
//! no-feature path. [`marque_scheme::NoopSink`] is a ZST with an
//! `#[inline(always)]` empty body, but it is boxed behind
//! `Mutex<Box<dyn SyncDecisionSink>>` on the engine field, so each
//! `emit()` call still incurs three residual operations even on the
//! Noop path: (1) an `AtomicU32::fetch_add` on the per-document step
//! counter, (2) a `Mutex::lock` on the sink, and (3) one vtable call
//! to the empty `record` body. The 2% ratio gate budgets these
//! against the no-feature baseline, where none of the three happen
//! because the engine field is compiled out entirely.
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
//! ## How the 2% gate is checked
//!
//! The constitutional 2% ratio gate compares this bench's
//! `decision_tracing_overhead_baseline` mean against the no-feature
//! baseline produced by `lint_latency::lint_10kb`. Both benches use the
//! same 10 KB representative input shape (`build_representative_input`
//! copied verbatim from `deadline_overhead.rs`) so the ratio is
//! meaningful. Wiring the comparison into `scripts/bench-check.sh` is
//! a follow-up; for now, the bench reports a Criterion mean.
//!
//! ## Bench config
//!
//! Sample size and measurement time mirror `deadline_overhead.rs`
//! exactly (sample_size=500, measurement_time=10s) — those were tuned
//! to bring the noise floor on WSL2-class CI hardware below the 2%
//! ratio gate, and this bench needs the same headroom to be comparable.
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
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    c.bench_function("decision_tracing_overhead_baseline", |b| {
        b.iter(|| engine.lint(black_box(&input)));
    });
}

fn decision_tracing_overhead_with_recording_sink(c: &mut Criterion) {
    let input = build_representative_input(10_000);
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
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
