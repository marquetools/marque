// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Engine::lint latency benchmarks. Two functions live here:
//!
//! - **SC-001 strict-path**: `lint_10kb` — `Engine::lint` on a 10KB
//!   representative input with [`StrictRecognizer`] explicitly
//!   installed. Target p95 <= 16ms. Pinning the strict recognizer
//!   directly (rather than relying on the engine default, which is the
//!   strict-then-decoder dispatcher) keeps SC-001 measuring a pure
//!   strict-path number even if the dispatcher's overhead grows.
//! - **SC-002 decoder-path**: `decoder_10kb_one_mangled_region` —
//!   `Engine::lint` on a 10KB representative input where exactly one
//!   region contains a mangled marking that forces the decoder to fire.
//!   Target p95 <= 18ms. The gap (18 - 16 = 2ms) is the per-document
//!   budget the decoder gets for fuzzy correction + canonical generation
//!   on a single mangled region; corpus-wide accuracy is gated separately
//!   by SC-004 in `tests/decoder_accuracy.rs`.
//!
//! Both targets are enforced by `scripts/bench-check.sh`, not by this
//! benchmark file. Run `./scripts/bench-check.sh` to gate.
//!
//! Reference baseline: x86_64 >= 3.0 GHz single-thread (e.g. modern laptop-class CPU),
//! warm cache, `--release` build, no tracing subscriber.

use criterion::{Criterion, criterion_group, criterion_main};
use marque_config::Config;
use marque_engine::{Engine, StrictRecognizer};
use std::hint::black_box;
use std::sync::Arc;

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
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
    .with_recognizer(Arc::new(StrictRecognizer::new()));

    c.bench_function("lint_10kb", |b| {
        b.iter(|| engine.lint(black_box(&input)));
    });
}

/// Build a ~10KB representative input where exactly ONE region contains
/// a mangled marking that forces the decoder to fire. The rest is the
/// same valid-marking + prose mix as `build_representative_input` so the
/// strict-path cost is identical and the measured delta isolates the
/// decoder's fuzzy-correction + canonical-generation cost.
///
/// SC-002 measures the *worst-case* decoder cost on a single document:
/// one region triggers the slow path while the rest stays on the fast
/// strict path. A document with many mangled regions amortizes the
/// per-token fuzzy work over more matches; a single mangled region in
/// otherwise clean text is the load-bearing case for interactive use
/// (an editor cursor sitting on a single typo'd marking).
fn build_decoder_input(target_bytes: usize) -> Vec<u8> {
    // Same block as `build_representative_input` so the strict-path
    // cost stays identical. Differences in measurement isolate to the
    // injected mangled region below.
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
    // The mangled portion: `SERCET` is edit-distance-1 from `SECRET`
    // and `NF` is the canonical portion-form NOFORN abbreviation. The
    // strict parser leaves classification = None on this input
    // (lenient parse), so the deep-scan dispatcher falls through to the
    // decoder. Mirrors the fixture used in
    // `tests/audit.rs::decoder_path_record_shape` so the bench
    // exercises the same decoder code path as the audit-shape regression.
    let mangled =
        "(SERCET//NF) Decoder fixture — single mangled portion in otherwise clean text.\n\n";

    let block_bytes = block.as_bytes();
    let mangled_bytes = mangled.as_bytes();

    let mut input = Vec::with_capacity(target_bytes + block_bytes.len() + mangled_bytes.len());
    // Inject the mangled region exactly once, near the front of the
    // document so the scanner reaches it before the byte budget is
    // exhausted. Then fill the rest with the strict-path block.
    input.extend_from_slice(mangled_bytes);
    while input.len() < target_bytes {
        input.extend_from_slice(block_bytes);
    }
    let complete_blocks = (target_bytes.saturating_sub(mangled_bytes.len())) / block_bytes.len();
    let truncated_len = mangled_bytes.len() + complete_blocks.max(1) * block_bytes.len();
    input.truncate(truncated_len);
    // Pad with spaces to reach exactly `target_bytes` so the bench
    // name (`decoder_10kb_one_mangled_region`) and the SC-002 gate are
    // measured against a true 10KB input.
    input.resize(target_bytes, b' ');
    input
}

fn decoder_latency_benchmark(c: &mut Criterion) {
    let input = build_decoder_input(10_000);
    // The decoder fallback is the engine default (`Engine::new` installs
    // `StrictOrDecoderRecognizer`), so this bench exercises the same
    // dispatcher every CLI / WASM caller runs against. The mangled
    // portion in `build_decoder_input` forces the strict path to leave
    // `classification = None`, which trips the dispatcher's fallback
    // into the decoder.
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    c.bench_function("decoder_10kb_one_mangled_region", |b| {
        b.iter(|| engine.lint(black_box(&input)));
    });
}

criterion_group!(benches, lint_latency_benchmark, decoder_latency_benchmark);
criterion_main!(benches);
