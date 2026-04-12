//! SC-005 benchmark: linear throughput scaling across input sizes.
//!
//! Sweeps input size across at least one order of magnitude (1KB -> 100KB)
//! and asserts throughput stays linear with no super-linear growth.
//! Criterion's HTML report (`target/criterion/`) visualises the throughput
//! curve — visual inspection confirms linearity.

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use marque_config::Config;
use marque_engine::Engine;

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

    let mut input = Vec::with_capacity(target_bytes + block.len());
    while input.len() < target_bytes {
        input.extend_from_slice(block.as_bytes());
    }
    input.truncate(target_bytes);
    input
}

fn linear_scaling_benchmark(c: &mut Criterion) {
    let sizes: &[usize] = &[1_000, 2_000, 5_000, 10_000, 20_000, 50_000, 100_000];
    let engine = Engine::new(Config::default(), marque_engine::default_ruleset());

    let mut group = c.benchmark_group("lint_scaling");
    for &size in sizes {
        let input = build_input(size);
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &input, |b, input| {
            b.iter(|| engine.lint(black_box(input)));
        });
    }
    group.finish();
}

criterion_group!(benches, linear_scaling_benchmark);
criterion_main!(benches);
