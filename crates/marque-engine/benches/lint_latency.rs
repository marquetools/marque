//! SC-001 benchmark: Engine::lint latency on representative inputs.
//!
//! Target: <= 16ms on inputs <= 10KB of raw text on commodity dev hardware,
//! using Criterion's confidence-interval upper bound as the enforced metric.
//! The threshold is enforced by `scripts/bench-check.sh`, not by this benchmark
//! file. Run `./scripts/bench-check.sh` to gate on the SC-001 target.
//!
//! Reference baseline: x86_64 >= 3.0 GHz single-thread (e.g. modern laptop-class CPU),
//! warm cache, `--release` build, no tracing subscriber.

use criterion::{Criterion, criterion_group, criterion_main};
use marque_config::Config;
use marque_engine::Engine;
use std::hint::black_box;

/// Build a ~10KB representative input by repeating a block of mixed valid and
/// invalid markings interspersed with prose. This mimics a real document with
/// markings scattered through body text.
fn build_representative_input(target_bytes: usize) -> Vec<u8> {
    // A representative block: ~200 bytes containing valid banners, portions,
    // and one common violation (abbreviated dissem in banner).
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
    // Truncate to a block-aligned boundary to avoid splitting mid-token,
    // which would create artificial partial-token diagnostics.
    let complete_blocks = target_bytes / block_bytes.len();
    input.truncate(complete_blocks.max(1) * block_bytes.len());
    // Pad with spaces to reach exactly target_bytes so the benchmark name
    // (`lint_10kb`) and the SC-001 gate are measured against a true 10KB input.
    // Trailing whitespace does not affect any token boundaries.
    input.resize(target_bytes, b' ');
    input
}

fn lint_latency_benchmark(c: &mut Criterion) {
    let input = build_representative_input(10_000);
    let engine = Engine::new(Config::default(), marque_engine::default_ruleset());

    c.bench_function("lint_10kb", |b| {
        b.iter(|| engine.lint(black_box(&input)));
    });
}

criterion_group!(benches, lint_latency_benchmark);
criterion_main!(benches);
