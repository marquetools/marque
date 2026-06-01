// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Erasure-dispatch overhead smoke bench (Phase B4.2, task T028b).
//!
//! Captures the cost of routing a lint through the object-safe
//! [`marque_engine::MultiGrammarEngine`] (one vtable dispatch per registered
//! grammar + the `LintResult<S>` → `ErasedLintResult` projection) relative to
//! the typed `Engine::lint` hot path. Three measurements:
//!
//! - `multi_grammar_dispatch_single` — one CAPCO grammar behind the registry.
//!   The delta against `lint_10kb` is the erasure overhead (vtable + project).
//! - `multi_grammar_dispatch_two` — CAPCO + a no-op `StubScheme` grammar. The
//!   delta against the single-grammar number is the marginal per-grammar cost.
//! - `erased_lint_single` — a bare `Box<dyn ErasedEngine>` lint, isolating the
//!   erasure projection from the registry iteration.
//!
//! **Informational only — not wired into `scripts/bench-check.sh` and not in
//! `benches/baseline.json`.** The typed `Engine<S>` hot path is untouched by
//! erasure (Phase B4.2 §7 invariant 4); these numbers exist to record the
//! opt-in dispatch cost, not to gate it.

use criterion::{Criterion, criterion_group, criterion_main};
use marque_config::Config;
use marque_engine::{
    CapcoEngine, Engine, ErasedEngine, InputContext, InputSource, MultiGrammarEngine, SystemClock,
};
use marque_rules::RuleSet;
use marque_test_utils::stub_scheme::{StubRecognizer, StubScheme};
use std::hint::black_box;

/// ~10KB representative input: valid banners/portions + one common violation,
/// interleaved with prose (mirrors `lint_latency.rs::build_representative_input`).
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

fn capco_engine() -> CapcoEngine {
    Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

fn stub_engine() -> Engine<StubScheme, StubRecognizer> {
    Engine::with_clock_and_recognizer(
        Config::default(),
        Vec::<Box<dyn RuleSet<StubScheme>>>::new(),
        StubScheme::new(),
        StubRecognizer,
        Box::new(SystemClock),
    )
    .expect("StubScheme declares no rewrites, so scheduling cannot fail")
}

fn multi_grammar_dispatch_single_benchmark(c: &mut Criterion) {
    let input = build_representative_input(10_000);
    let ctx = InputContext::new(InputSource::DocumentContent);
    let mut registry = MultiGrammarEngine::new();
    registry.register(Box::new(capco_engine()));

    c.bench_function("multi_grammar_dispatch_single", |b| {
        b.iter(|| registry.lint(black_box(&input), black_box(&ctx)));
    });
}

fn multi_grammar_dispatch_two_benchmark(c: &mut Criterion) {
    let input = build_representative_input(10_000);
    let ctx = InputContext::new(InputSource::DocumentContent);
    let mut registry = MultiGrammarEngine::new();
    registry.register(Box::new(capco_engine()));
    registry.register(Box::new(stub_engine()));

    c.bench_function("multi_grammar_dispatch_two", |b| {
        b.iter(|| registry.lint(black_box(&input), black_box(&ctx)));
    });
}

fn erased_lint_single_benchmark(c: &mut Criterion) {
    let input = build_representative_input(10_000);
    let ctx = InputContext::new(InputSource::DocumentContent);
    let engine: Box<dyn ErasedEngine> = Box::new(capco_engine());

    c.bench_function("erased_lint_single", |b| {
        b.iter(|| engine.lint_erased(black_box(&input), black_box(&ctx)));
    });
}

criterion_group!(
    benches,
    multi_grammar_dispatch_single_benchmark,
    multi_grammar_dispatch_two_benchmark,
    erased_lint_single_benchmark,
);
criterion_main!(benches);
