// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Spec 005 deadline-overhead benchmark.
//!
//! Compares the unbounded `Engine::lint` path against the
//! deadline-bounded `Engine::lint_with_options` path running with a
//! generous deadline (1 hour) — i.e., the deadline is checked but
//! never trips. The delta isolates the per-candidate `Instant::now()`
//! cost the cooperative-cancellation wiring adds.
//!
//! **Spec 005 SC**: mean overhead MUST be ≤ 2% on the standard 10 KB
//! corpus document. ("Mean" not "median" — Criterion's `time:
//! [lower mean upper]` triple is a confidence interval around the
//! mean point estimate, not a sample percentile, and the gate parses
//! the middle value of that triple.) The bench emits both paths under
//! names `deadline_overhead_baseline` and
//! `deadline_overhead_with_deadline`; the regression gate
//! (`scripts/bench-check.sh::check_deadline_overhead` +
//! `benches/baseline.json`) compares the two and fails the build if
//! the with-deadline mean exceeds the threshold ratio over the
//! baseline mean.
//!
//! Reference baseline: x86_64 ≥ 3.0 GHz single-thread, warm cache,
//! `--release` build, no tracing subscriber. Same shape as
//! `lint_latency.rs` so a regression gate against it composes cleanly
//! with the existing SC-001 / SC-002 gates.

use criterion::{Criterion, criterion_group, criterion_main};
use marque_config::Config;
use marque_engine::{Engine, LintOptions};
use std::hint::black_box;
use std::time::{Duration, Instant};

/// Build the same 10 KB representative input the SC-001 bench uses,
/// so the deadline-overhead measurement composes against the same
/// input shape (mixed valid markings + prose). Sharing the fixture
/// definition between benches would create a cross-bench dependency;
/// duplicating ~30 lines is cheaper than the build-graph entanglement.
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

fn deadline_overhead_baseline_benchmark(c: &mut Criterion) {
    let input = build_representative_input(10_000);
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    c.bench_function("deadline_overhead_baseline", |b| {
        b.iter(|| engine.lint(black_box(&input)));
    });
}

fn deadline_overhead_with_deadline_benchmark(c: &mut Criterion) {
    let input = build_representative_input(10_000);
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    // Construct the deadline ONCE outside the iter loop. The
    // bench measures the steady-state per-call cost of the
    // engine's deadline-aware path; pulling the
    // `Instant::now() + Duration` arithmetic out keeps the
    // per-iteration measurement honest (otherwise the
    // construction cost itself shows up as part of the delta
    // against the baseline bench, which it isn't). The 1-hour
    // budget is far larger than any reasonable bench duration,
    // so the deadline is checked but never trips — exactly the
    // path callers exercise in production for "lint with a
    // timeout that almost certainly won't fire."
    let mut opts = LintOptions::default();
    opts.deadline = Some(Instant::now() + Duration::from_secs(3600));

    c.bench_function("deadline_overhead_with_deadline", |b| {
        b.iter(|| engine.lint_with_options(black_box(&input), &opts));
    });
}

// Larger sample size + longer measurement window so the 2% ratio
// gate (`scripts/bench-check.sh::check_deadline_overhead`) has
// enough signal to clear the bench-runner noise floor on WSL2-class
// CI hardware. The default 100-sample window with the standard
// 5-second `measurement_time` produces ±5% iteration-to-iteration
// jitter on this bench, which would convert a real 1–2% overhead
// into a flaky gate. Bumping sample size to 500 narrows the
// confidence interval ~2.2× and brings false-positive risk under
// 1% empirically. The tradeoff is bench wall-clock — each function
// now takes ~10–15s instead of ~5s, totaling ~25–30s for the pair
// (still under the per-job timeout in CI).
criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(500)
        .measurement_time(Duration::from_secs(10));
    targets =
        deadline_overhead_baseline_benchmark,
        deadline_overhead_with_deadline_benchmark
}
criterion_main!(benches);
