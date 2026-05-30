// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Linear throughput scaling across input sizes.
//!
//! Sweeps input size across at least one order of magnitude (1KB -> 100KB)
//! and measures throughput at each size. Criterion's HTML report
//! (`target/criterion/`) visualizes the throughput curve for linearity
//! verification. The ≤2ms regression threshold (based on Criterion's
//! CI upper bound) is enforced by `scripts/bench-check.sh`; this
//! benchmark provides the scaling data.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use marque_config::Config;
use marque_engine::Engine;
use std::hint::black_box;

/// Build an input of approximately `target_bytes` by repeating a representative
/// marking block with mixed valid/invalid content.
fn build_input(target_bytes: usize) -> Vec<u8> {
    let block = concat!(
        "TOP SECRET//SCI//NOFORN\n",
        "\n",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit.\n",
        "\n",
        "(S//NF) Abbreviated dissem in portion marking.\n",
        "\n",
        "CONFIDENTIAL//REL TO USA, AUS\n",
        "\n",
        "Sed do eiusmod tempor incididunt ut labore.\n",
        "\n",
    );

    let block_bytes = block.as_bytes();
    let mut input = Vec::with_capacity(target_bytes + block_bytes.len());
    while input.len() < target_bytes {
        input.extend_from_slice(block_bytes);
    }
    // Truncate to a block-aligned boundary to avoid splitting mid-token.
    let complete_blocks = target_bytes / block_bytes.len();
    input.truncate(complete_blocks.max(1) * block_bytes.len());
    input
}

fn linear_scaling_benchmark(c: &mut Criterion) {
    let sizes: &[usize] = &[1_000, 2_000, 5_000, 10_000, 20_000, 50_000, 100_000];
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    let mut group = c.benchmark_group("lint_scaling");
    for &size in sizes {
        let input = build_input(size);
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(input.len()),
            &input,
            |b, input| {
                b.iter(|| engine.lint(black_box(input)));
            },
        );
    }
    group.finish();
}

criterion_group!(benches, linear_scaling_benchmark);
criterion_main!(benches);
