// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Decoder REL TO USA-injection latency benchmark (issue #234 PR-B).
//!
//! Measures the per-document cost of the new
//! [`try_rel_to_usa_injection_candidates`] path against a realistic-
//! shaped 10KB document where one REL TO block has a 2-char first
//! entry (`SA` instead of `USA`) — the canonical fixture
//! `tests/fixtures/mangled/typo/ad2bcfe3ac0b0765.json`.
//!
//! Constitution Principle I — perf bench mandatory: a structural
//! decoder path that lands without a committed bench couldn't claim
//! its candidate-expansion is bounded against the decoder-path latency
//! envelope (p95 ≤ 18 ms). Mirrors the `lint_latency.rs` and
//! `decoder_trigraph_priors.rs` shape (10KB document with one
//! mangled REL TO region) so the three benches share the same
//! synthetic-input model and any latency delta is attributable to
//! the new code path rather than fixture differences.
//!
//! Reference baseline: x86_64 ≥ 3.0 GHz single-thread, warm cache,
//! `--release` build, no tracing subscriber.

use criterion::{Criterion, criterion_group, criterion_main};
use marque_config::Config;
use marque_engine::Engine;
use std::hint::black_box;

/// Build a ~10KB document with exactly ONE REL TO block whose first
/// entry is the 2-char `SA` (a truncated USA). The remainder is the
/// same valid-marking + prose mix as `lint_latency::build_decoder_input`
/// and `decoder_trigraph_priors::build_trigraph_typo_input` so the
/// strict-path cost stays identical between the three benches and the
/// measured delta isolates the USA-injection path.
fn build_usa_injection_input(target_bytes: usize) -> Vec<u8> {
    // Same block shape as the sister benches for cross-comparison.
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

    // The mangled banner: `SA` is below `MIN_FUZZY_LEN = 3` so the
    // standard fuzzy matcher and PR-A's trigraph path both skip it.
    // The §H.8 p151 USA-first invariant in PR-B's
    // `try_rel_to_usa_injection_candidates` is what carries the
    // recovery — fixture `ad2bcfe3ac0b0765.json` round-trips through
    // this path.
    let mangled = "SECRET//REL TO SA, AUS, GBR\n\n";

    let block_bytes = block.as_bytes();
    let mangled_bytes = mangled.as_bytes();

    let mut input = Vec::with_capacity(target_bytes + block_bytes.len() + mangled_bytes.len());
    input.extend_from_slice(mangled_bytes);
    while input.len() < target_bytes {
        input.extend_from_slice(block_bytes);
    }
    let complete_blocks = (target_bytes.saturating_sub(mangled_bytes.len())) / block_bytes.len();
    let truncated_len = mangled_bytes.len() + complete_blocks.max(1) * block_bytes.len();
    input.truncate(truncated_len);
    // Pad to exactly `target_bytes` so the bench name reflects the
    // 10KB document size honestly.
    input.resize(target_bytes, b' ');
    input
}

fn rel_to_invariant_benchmark(c: &mut Criterion) {
    let input = build_usa_injection_input(10_000);
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    c.bench_function("decoder_10kb_rel_to_invariant", |b| {
        b.iter(|| engine.lint(black_box(&input)));
    });
}

criterion_group!(benches, rel_to_invariant_benchmark);
criterion_main!(benches);
