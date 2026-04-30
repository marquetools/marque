// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `Engine::fix` latency benchmarks. Three functions live here:
//!
//! - **`fix_single_e001_apply`** — `engine.fix(SECRET//NF, FixMode::Apply)`
//!   on a single-marking input that produces exactly one E001 fix at
//!   confidence 1.0. End-to-end per-fix latency: scanner + parser + rule
//!   evaluation + promotion + apply + audit. Criterion amortizes over
//!   thousands of iterations, so the reported time IS the per-fix cost
//!   for an interactive caller fixing one marking at a time.
//! - **`fix_single_e001_dry_run`** — same input, `FixMode::DryRun`. Drops
//!   the source-rewrite cost; isolates the cost of generating the audit
//!   record without applying the rewrite to a fresh `Vec<u8>`.
//! - **`lint_single_e001_baseline`** — `engine.lint` on the same input.
//!   The delta `fix_single_e001_apply - lint_single_e001_baseline` is the
//!   marginal cost of promotion + apply + audit on top of detection.
//!
//! Marketing-facing number is `fix_single_e001_apply`: total wall-clock
//! time to detect, promote, apply, and audit one fix on a one-marking
//! input. The lint baseline contextualizes how much of that is detection
//! vs the fix-specific work.
//!
//! Not gated by `scripts/bench-check.sh` — there is no SC-target for
//! per-fix latency yet. Wired in advisory mode so the numbers print
//! alongside the gated benches.

use criterion::{Criterion, criterion_group, criterion_main};
use marque_config::Config;
use marque_engine::{Engine, FixMode};
use std::hint::black_box;

/// Single-marking input that produces exactly one E001 fix at confidence
/// 1.0 (`NF` → `NOFORN` in a banner). Mirrors the
/// `fix_pipeline.rs::mixed_confidence_source` fixture stripped down to
/// just the high-confidence path so the bench measures one fix per call,
/// not a mix.
const SINGLE_FIX_INPUT: &[u8] = b"SECRET//NF\n";

fn build_engine() -> Engine {
    Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

fn fix_apply_benchmark(c: &mut Criterion) {
    let engine = build_engine();
    c.bench_function("fix_single_e001_apply", |b| {
        b.iter(|| engine.fix(black_box(SINGLE_FIX_INPUT), FixMode::Apply));
    });
}

fn fix_dry_run_benchmark(c: &mut Criterion) {
    let engine = build_engine();
    c.bench_function("fix_single_e001_dry_run", |b| {
        b.iter(|| engine.fix(black_box(SINGLE_FIX_INPUT), FixMode::DryRun));
    });
}

fn lint_baseline_benchmark(c: &mut Criterion) {
    let engine = build_engine();
    c.bench_function("lint_single_e001_baseline", |b| {
        b.iter(|| engine.lint(black_box(SINGLE_FIX_INPUT)));
    });
}

criterion_group!(
    benches,
    fix_apply_benchmark,
    fix_dry_run_benchmark,
    lint_baseline_benchmark
);
criterion_main!(benches);
