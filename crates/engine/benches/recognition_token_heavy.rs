// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Token-heavy recognition micro-benchmark for issue #431.
//!
//! Advisory only — not gated by `scripts/bench-check.sh` and not
//! pinned in `baseline.json`. The two functions here exercise the
//! hot path most affected by the `shift_token_spans` post-pass
//! elimination: a single very large marking whose `token_spans` array
//! is long enough that any per-token post-pass cost shows up as a
//! measurable delta.
//!
//! Two fixtures, both ~10KB single-document inputs dominated by one
//! large portion:
//!
//! - **`recognition_long_rel_to`** — a portion carrying a maxed-out
//!   `REL TO` country list. The trigraph token stream is the longest
//!   linear `token_spans` array a portion can produce under CAPCO-2016
//!   §H.8.
//! - **`recognition_sci_heavy`** — a portion carrying a saturated SCI
//!   compartment + sub-compartment block under §A.6 (`SI-G ABCD EFGH
//!   IJKL MNOP-TK XYZW`). The grammar packs more distinct tokens per
//!   byte than `REL TO` does, so the per-token bookkeeping cost is
//!   the dominant factor.
//!
//! Both benchmarks pin [`StrictRecognizer`] explicitly: the dispatcher
//! default would interleave decoder-fallback overhead on any
//! borderline candidate and obscure the pure strict-path measurement.

use criterion::{Criterion, criterion_group, criterion_main};
use marque_config::Config;
use marque_engine::{Engine, StrictRecognizer};
use std::hint::black_box;
use std::sync::Arc;

/// Build a 10KB input where one portion carries a long REL TO list.
/// The rest is short benign prose so the scanner has exactly one
/// recognition-heavy candidate to chew through.
fn build_long_rel_to_input(target_bytes: usize) -> Vec<u8> {
    // 10 country codes in canonical USA-first alpha order. Token
    // stream is long enough that any per-token post-pass on
    // `token_spans` is the dominant cost inside `recognize()`.
    // Codes are valid ODNI ISMCAT trigraphs to keep the strict parser
    // happy.
    let heavy = "(S//REL TO USA, AUS, BEL, CAN, DEU, ESP, FRA, GBR, ITA, NLD)\n\n";
    let filler = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do\n\
                  eiusmod tempor incididunt ut labore et dolore magna aliqua.\n\n";
    let heavy_bytes = heavy.as_bytes();
    let filler_bytes = filler.as_bytes();

    let mut input = Vec::with_capacity(target_bytes + heavy_bytes.len() + filler_bytes.len());
    input.extend_from_slice(heavy_bytes);
    while input.len() < target_bytes {
        input.extend_from_slice(filler_bytes);
    }
    input.truncate(target_bytes);
    input
}

/// Build a 10KB input where one portion carries a saturated SCI block
/// per CAPCO-2016 §A.6 grammar.
fn build_sci_heavy_input(target_bytes: usize) -> Vec<u8> {
    // SCI compositional grammar: CONTROL-COMP (SPACE SUB-COMP)*
    // (-COMP (SPACE SUB-COMP)*)*. The portion below saturates two
    // controls (SI, TK) with multiple compartments + sub-compartments
    // each, exercising the structural SCI subparser's per-token
    // bookkeeping more heavily than the §H.8 REL TO fixture.
    let heavy = "(TS//SI-G ABCD EFGH IJKL MNOP-TK XYZW)\n\n";
    let filler = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do\n\
                  eiusmod tempor incididunt ut labore et dolore magna aliqua.\n\n";
    let heavy_bytes = heavy.as_bytes();
    let filler_bytes = filler.as_bytes();

    let mut input = Vec::with_capacity(target_bytes + heavy_bytes.len() + filler_bytes.len());
    input.extend_from_slice(heavy_bytes);
    while input.len() < target_bytes {
        input.extend_from_slice(filler_bytes);
    }
    input.truncate(target_bytes);
    input
}

fn recognition_long_rel_to_benchmark(c: &mut Criterion) {
    let input = build_long_rel_to_input(10_000);
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
    .with_recognizer(Arc::new(StrictRecognizer::new()));

    c.bench_function("recognition_long_rel_to", |b| {
        b.iter(|| engine.lint(black_box(&input)));
    });
}

fn recognition_sci_heavy_benchmark(c: &mut Criterion) {
    let input = build_sci_heavy_input(10_000);
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
    .with_recognizer(Arc::new(StrictRecognizer::new()));

    c.bench_function("recognition_sci_heavy", |b| {
        b.iter(|| engine.lint(black_box(&input)));
    });
}

criterion_group!(
    benches,
    recognition_long_rel_to_benchmark,
    recognition_sci_heavy_benchmark
);
criterion_main!(benches);
