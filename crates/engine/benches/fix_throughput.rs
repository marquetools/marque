// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Linear-throughput fix benchmark: `Engine::fix` with `FixMode::Apply`
//! must scale linearly in input size when fix density is proportional to
//! document size.
//!
//! Input shape: mixed prose + valid markings with one `SECRET//NOFORN` banner
//! per ~10.9 KB section. Zero fixes fire — the bench measures the fix-path
//! overhead (recognition, page-context accumulation, fix-apply scaffolding)
//! without actual splice work.
//! Fix density proportional to document size is the shape that exposed the
//! O(N²) page-context blowup fixed in issue #306.
//!
//! The benchmark sweeps from 1 MB to 100 MB and reports throughput (MB/s) at
//! each size. Linearity is enforced by `scripts/bench-check.sh` via the
//! `fix_throughput` R² gate in `benches/baseline.json` (R² ≥ 0.9 across the
//! size sweep), mirroring the `lint_scaling` linear-throughput gate. The
//! `fix_throughput/100000000` data point specifically guards the
//! `Vec::splice`-per-fix regression described in the
//! `perf(engine): fix-apply path is quadratic in input size` issue.
//!
//! Benchmark IDs are the actual byte length of the generated input (e.g.
//! `fix_throughput/10912000`), matching the `lint_scaling/<bytes>` convention
//! so `bench-check.sh` can use the raw byte count as the regression x-axis
//! without integer-MB rounding artifacts.
//!
//! **Expected behavior after the fix**: throughput at 100 MB should be in the
//! same MB/s ballpark as at 1 MB (within 2×). Before the fix the 100 MB case
//! did not complete within 20 minutes of single-threaded CPU.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use marque_config::Config;
use marque_engine::{Engine, FixMode};
use secrecy::ExposeSecret as _;
use std::hint::black_box;

/// Build an input of approximately `target_bytes` containing one
/// `SECRET//NOFORN` banner per ~10.9 KB prose section. The banner density
/// scales linearly with document size — exactly the shape that exposed the
/// quadratic blowup in page-context accumulation.
fn build_fix_input(target_bytes: usize) -> Vec<u8> {
    // Each block is ~10.9 KB: one valid `SECRET//NOFORN` banner followed by
    // ~10.9 KB of valid markings and prose. Zero fixes fire; the scaling
    // signal comes from the page-context accumulation path, which was O(N²)
    // before issue #306.
    let banner = "SECRET//NOFORN\n\n";

    // ~220-byte prose + marking block repeated to fill the section.
    let prose_block = concat!(
        "TOP SECRET//SCI//NOFORN\n",
        "\n",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do\n",
        "eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim\n",
        "ad minim veniam, quis nostrud exercitation ullamco laboris nisi.\n",
        "\n",
        "(S//NF) Portion mark with abbreviated dissem — valid portion form.\n",
        "\n",
        "CONFIDENTIAL//REL TO USA, GBR\n",
        "\n",
        "Duis aute irure dolor in reprehenderit in voluptate velit esse cillum\n",
        "dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat.\n",
        "\n",
    );

    // Target section size: banner + enough prose to reach ~10.9 KB.
    let section_target = 10_900usize;
    let banner_bytes = banner.as_bytes();
    let prose_bytes = prose_block.as_bytes();

    // Build one section.
    let mut section = Vec::with_capacity(section_target + prose_bytes.len());
    section.extend_from_slice(banner_bytes);
    while section.len() < section_target {
        section.extend_from_slice(prose_bytes);
    }
    // Trim to a block-aligned boundary so we never split mid-token.
    let prose_reps = (section_target.saturating_sub(banner_bytes.len())) / prose_bytes.len();
    section.truncate(banner_bytes.len() + prose_reps.max(1) * prose_bytes.len());

    // Tile sections to reach target_bytes.
    let mut input = Vec::with_capacity(target_bytes + section.len());
    while input.len() < target_bytes {
        input.extend_from_slice(&section);
    }
    let complete_sections = target_bytes / section.len();
    input.truncate(complete_sections.max(1) * section.len());
    input
}

fn fix_throughput_benchmark(c: &mut Criterion) {
    let sizes: &[usize] = &[
        1_000_000,   // 1 MB
        5_000_000,   // 5 MB
        10_000_000,  // 10 MB
        50_000_000,  // 50 MB
        100_000_000, // 100 MB  ← the regression guard from the issue
    ];
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    let mut group = c.benchmark_group("fix_throughput");
    // Criterion's default sample count (100) is too expensive at 100 MB;
    // 10 samples still give a stable mean and keep CI runtime bounded.
    group.sample_size(10);
    for &size in sizes {
        let input = build_fix_input(size);
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(input.len()),
            &input,
            |b, input| {
                b.iter(|| {
                    let result = engine.fix(black_box(input), FixMode::Apply);
                    // Prevent the compiler from eliding the call: consume the
                    // output length so the fix actually runs.
                    black_box(result.source.expose_secret().len())
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, fix_throughput_benchmark);
criterion_main!(benches);
