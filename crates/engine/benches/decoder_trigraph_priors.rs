// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Decoder REL TO trigraph fuzzy-priors latency benchmark (issue #233).
//!
//! Measures the per-document cost of the trigraph fuzzy-priors
//! recovery path against a realistic-shaped 10KB document where one
//! REL TO block contains an ambiguous trigraph typo (`USB → USA`) that
//! exercises the new candidate-expansion code path:
//!
//! 1. Strict parser emits `TokenKind::Unknown` for `USB` (issue #233
//!    change in `parse_rel_to_with_spans`).
//! 2. Step-3a in `DecoderRecognizer::recognize` rejects the
//!    drop-USB candidate.
//! 3. `try_rel_to_fuzzy_trigraph_candidates` walks the REL TO block,
//!    finds USB is not a valid trigraph, asks the trigraph fuzzy
//!    matcher for all candidates within `MAX_EDIT_DISTANCE`, and
//!    emits one canonical-byte alternate per candidate.
//! 4. `score_candidate` sums `trigraph_log_prior` over each
//!    candidate's `rel_to` slice; the popular-vs-rare delta breaks
//!    the tie.
//!
//! The bench is the gate the issue's "Constitution Principle I —
//! perf bench mandatory" requirement names: a trigraph-priors PR
//! that landed without a committed bench couldn't claim the new
//! candidate-expansion path is bounded.
//!
//! Reference baseline: x86_64 ≥ 3.0 GHz single-thread, warm cache,
//! `--release` build, no tracing subscriber. Mirror of
//! `lint_latency.rs` SC-002 shape so the two benches share the same
//! 10KB-with-one-mangled-region model.

use criterion::{Criterion, criterion_group, criterion_main};
use marque_config::Config;
use marque_engine::Engine;
use std::hint::black_box;

/// Build a ~10KB document with exactly ONE REL TO trigraph typo
/// (`USB`) injected near the front. The remainder is the same valid-
/// marking + prose mix as `lint_latency::build_decoder_input` so the
/// strict-path cost stays identical between the two benches and the
/// measured delta isolates the trigraph fuzzy-priors path.
fn build_trigraph_typo_input(target_bytes: usize) -> Vec<u8> {
    // Same block shape as `lint_latency.rs` for cross-comparison.
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

    // The mangled banner: `USB` is a 1-edit typo with two
    // equidistant trigraph candidates (USA, UZB). Without trigraph
    // priors the standard fuzzy path returns no correction (ambiguous
    // tie), the strict parser drops USB silently, and the document
    // loses a country code. With the issue #233 change the decoder
    // emits one alternate per candidate and the corpus-weighted
    // log-prior breaks the tie at score time.
    let mangled = "SECRET//REL TO USB, AUS, GBR\n\n";

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

fn trigraph_priors_benchmark(c: &mut Criterion) {
    let input = build_trigraph_typo_input(10_000);
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
    .with_deep_scan();

    c.bench_function("decoder_10kb_trigraph_typo", |b| {
        b.iter(|| engine.lint(black_box(&input)));
    });
}

criterion_group!(benches, trigraph_priors_benchmark);
criterion_main!(benches);
