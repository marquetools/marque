// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 7c — `Engine::fix` latency on a 10 KB document under two-pass dispatch.
//!
//! Two bench functions per D-7.11:
//!
//! - **`fix_10kb_pass2_only`**: 10 KB input with only `Phase::WholeMarking`-
//!   triggering content (no C001 corrections, no E006 deprecations). The
//!   short-circuit path through pass-1 (empty `pass1_applied`) exercises
//!   the no-reshape branch — the pre-pass-1 attrs cache is empty, the
//!   re-parse arm short-circuits, and pass-2 dispatches directly against
//!   the post-pass-0 buffer.
//! - **`fix_10kb_two_pass`**: 10 KB input with BOTH `Phase::Localized`
//!   triggers (C001 typos that the corrections map fixes) AND
//!   `Phase::WholeMarking` triggers. Exercises the full two-pass path:
//!   pre-pass-1 cache population, post-pass-1 re-lint, FR-023
//!   disambiguation, I-18 overlap demotion, `PrecedingFixPenalty`
//!   application at the threshold gate.
//!
//! Both benches construct the engine ONCE outside the `b.iter` loop —
//! constructing the AhoCorasick automaton on every iteration would
//! dominate the measurement (~1 ms / construction; well above the
//! per-call SC-001 budget). The constant-time engine construction is a
//! native-call concern, not a per-fix concern.
//!
//! # Gating
//!
//! - **Absolute (SC-001 / FR-030)**: p95 ≤ 16 ms on 10 KB inputs.
//! - **Delta (D-7.11 / FR-033)**: p99 ≤ baseline.p99 × 1.05 against the
//!   pre-PR-7c baseline. Capture: `cargo bench --bench fix_10kb -- \
//!   --save-baseline pre-pr7c` on `origin/staging`; then `--save-baseline pr7c`
//!   on this branch. `scripts/bench-check.sh` enforces the delta.

use criterion::{Criterion, criterion_group, criterion_main};
use marque_config::Config;
use marque_engine::{Engine, FixMode};
use std::hint::black_box;

/// Build a ~10 KB input with only `Phase::WholeMarking`-eligible
/// content — valid markings the engine canonicalizes via the
/// scheme's apply_intent path. No typos / no deprecated markings,
/// so pass-1 produces zero applied fixes; the pre-pass-1 cache
/// stays empty.
fn build_pass2_only_input(target_bytes: usize) -> Vec<u8> {
    // A block with valid markings the WholeMarking rules canonicalize
    // (banner / portion dissem ordering, banner roll-up validation).
    // No typos, no E006 deprecations. ~250 B per block.
    let block = concat!(
        "TOP SECRET//NOFORN\n",
        "\n",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do\n",
        "eiusmod tempor incididunt ut labore et dolore magna aliqua.\n",
        "\n",
        "(TS//NF) Valid portion marking.\n",
        "(S//NF) Lower-classification portion.\n",
        "\n",
        "Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris\n",
        "nisi ut aliquip ex ea commodo consequat.\n",
        "\n",
        "(TS//REL TO USA, GBR) A portion releasing to USA, GBR.\n",
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

/// Build a ~10 KB input with BOTH `Phase::Localized` triggers (the
/// C001 `SERCET → SECRET` typo) AND `Phase::WholeMarking` triggers.
/// Each block carries one typo so pass-1 splices fire, then pass-2
/// dispatches against the corrected buffer.
fn build_two_pass_input(target_bytes: usize) -> Vec<u8> {
    let block = concat!(
        "SERCET//NOFORN\n", // C001 trigger: SERCET → SECRET
        "\n",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do\n",
        "eiusmod tempor incididunt ut labore et dolore magna aliqua.\n",
        "\n",
        "(S//NF) Portion marking that survives correction.\n",
        "(C//NF) Lower-classification portion.\n",
        "\n",
        "TOP SECRET//NOFORN\n",
        "\n",
        "Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris\n",
        "nisi ut aliquip ex ea commodo consequat.\n",
        "\n",
        "(TS//REL TO USA, GBR) Portion releasing to a coalition.\n",
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

fn build_engine_default() -> Engine {
    Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

fn build_engine_with_corrections() -> Engine {
    let mut config = Config::default();
    config.corrections.insert("SERCET".into(), "SECRET".into());
    Engine::new(
        config,
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

fn fix_10kb_pass2_only(c: &mut Criterion) {
    let input = build_pass2_only_input(10_000);
    let engine = build_engine_default();
    // Sanity-check: pass-1 produces zero applied fixes on this
    // fixture, so the two-pass orchestrator short-circuits the
    // re-parse arm and dispatches pass-2 directly against the
    // post-pass-0 buffer.
    let result = engine.fix(&input, FixMode::Apply);
    debug_assert!(
        result.applied.iter().all(|a| a.rule.as_str() != "C001"),
        "fix_10kb_pass2_only fixture leaked a C001 trigger"
    );

    c.bench_function("fix_10kb_pass2_only", |b| {
        b.iter(|| engine.fix(black_box(&input), FixMode::Apply));
    });
}

fn fix_10kb_two_pass(c: &mut Criterion) {
    let input = build_two_pass_input(10_000);
    let engine = build_engine_with_corrections();
    // Sanity-check: pass-0 / pass-1 produces ≥1 applied fix (a C001
    // text correction), driving the engine through the post-pass-1
    // re-parse arm and exercising the FR-023 / I-18 / PrecedingFixPenalty
    // codepaths.
    let result = engine.fix(&input, FixMode::Apply);
    debug_assert!(
        !result.applied.is_empty(),
        "fix_10kb_two_pass fixture should fire at least one applied fix"
    );

    c.bench_function("fix_10kb_two_pass", |b| {
        b.iter(|| engine.fix(black_box(&input), FixMode::Apply));
    });
}

criterion_group!(benches, fix_10kb_pass2_only, fix_10kb_two_pass);
criterion_main!(benches);
