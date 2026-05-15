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
    // INTENTIONAL-STRICT: SC-001 interactive-latency bench pins the strict recognizer to measure the latency floor; the dispatcher's decoder fallback is benchmarked separately in decoder_10kb_rel_to_invariant.rs
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

// ---------------------------------------------------------------------------
// Severity-override-hoist benchmarks (perf/engine-severity-override-hoisting)
// ---------------------------------------------------------------------------
//
// These two variants pin the speedup expected from pre-resolving rule
// severity overrides at engine construction time. Both run on the same
// `build_representative_input(10_000)` fixture as `lint_10kb` and pin
// the `StrictRecognizer` for the same reason (isolate strict-path
// latency from the decoder-dispatcher's fallback overhead).
//
//   - `lint_default_config`   — empty `Config::default()` (no overrides).
//                               Baseline; the hoist removes per-candidate
//                               HashMap probes + per-diagnostic parse_config
//                               calls that previously fired in the hot loop
//                               even with an empty override map.
//   - `lint_off_heavy_config` — `OFF_RULES` (below) set to `"off"` in
//                               `config.rules.overrides`. This is the
//                               configuration where the hoist matters
//                               most: pre-hoist, every (candidate × rule)
//                               pair did an `overrides.get + parse_config`
//                               just to decide whether to skip the rule;
//                               post-hoist, each pair is one indexed
//                               array load.

/// Rules disabled in the `lint_off_heavy_config` bench. Chosen to maximize
/// the number of pre-resolved Off entries the lint hot loop's Site A
/// short-circuits past, while avoiding the rules the bench fixture is
/// known to exercise (E031, the banner-roll-up walker; E002, the
/// missing-USA-trigraph rule that doesn't actually fire on this fixture
/// since `REL TO USA, GBR` already contains USA).
///
/// Source set: the 38 registered rule IDs pinned by
/// `crates/capco/tests/post_3b_registration_pin.rs::EXPECTED_RULE_IDS`.
/// The picks below cover the long-tail of rules the bench fixture
/// doesn't trigger — every PR 9a-era addition (E061-E065), the
/// PR 9c.1 / 9c.2 NATO additions (E066, S007), the warning suite, and
/// the rare-fire dissem / SCI rules. Together they exercise the
/// Site A fast-path Off-skip on every candidate (~10 per 10KB).
const OFF_RULES: &[&str] = &[
    // PR 9a additions (very rare in clean fixture).
    "E061", "E062", "E063", "E064", "E065", // PR 9c.1 / 9c.2 NATO additions.
    "E066", "S007", // PR 9c.1 / 9c.2 NATO additions (don't fire in this fixture).
    "W002", "W003", "W034", // Style suggestions.
    "S003", "S004", "S005", "S006",
    // Dissem / SCI / SAR per-axis rules outside the fixture's coverage.
    "E005", "E006", "E007", "E008", "E010", "E012", "E014", "E015", "E016",
    // Misc rare-fire rules.
    "E021", "E024", "E036",
];

fn lint_default_config_benchmark(c: &mut Criterion) {
    let input = build_representative_input(10_000);
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
    // INTENTIONAL-STRICT: matches lint_10kb's recognizer pin so the
    // severity-hoist delta is measured against a pure strict-path
    // baseline. Same rationale as the SC-001 bench.
    .with_recognizer(Arc::new(StrictRecognizer::new()));

    c.bench_function("lint_default_config", |b| {
        b.iter(|| engine.lint(black_box(&input)));
    });
}

fn lint_off_heavy_config_benchmark(c: &mut Criterion) {
    let input = build_representative_input(10_000);
    let mut config = Config::default();
    for rule_id in OFF_RULES {
        config
            .rules
            .overrides
            .insert((*rule_id).to_owned(), "off".to_owned());
    }
    let engine = Engine::new(
        config,
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
    // INTENTIONAL-STRICT: same recognizer pin as the baseline so the
    // measured delta isolates the per-rule override resolution cost.
    .with_recognizer(Arc::new(StrictRecognizer::new()));

    c.bench_function("lint_off_heavy_config", |b| {
        b.iter(|| engine.lint(black_box(&input)));
    });
}

// ---------------------------------------------------------------------------
// Prose-heavy advisory bench (perf/scanner-memchr-page-breaks)
// ---------------------------------------------------------------------------
//
// `lint_prose_heavy` measures the scanner-pass cost on pure-prose input
// (newline-sparse text, no marking tokens). This is the input shape that
// most exercises `Scanner::scan_page_breaks` and the other newline-driven
// sub-passes; it isolates the perf delta from a SIMD-driven newline
// stride against the previous byte-by-byte iter loop.
//
// Advisory bench — no entry in `benches/baseline.json`, same pattern as
// `lint_default_config` / `lint_off_heavy_config`. Report the number in
// PRs that touch the scanner; don't gate on it.

/// Build a ~10KB pure-prose input. Lorem-ipsum-style sentences with `\n`
/// line breaks and `\n\n` paragraph breaks. Explicitly NO marking tokens
/// (no `(U)`, no `//`, no banners) and NO `\n\n\n+` runs that would
/// trigger soft page breaks — the bench measures scanner cost on the
/// happy-path prose case, not page-break emission.
fn build_prose_input(target_bytes: usize) -> Vec<u8> {
    let block = concat!(
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do\n",
        "eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut\n",
        "enim ad minim veniam, quis nostrud exercitation ullamco laboris\n",
        "nisi ut aliquip ex ea commodo consequat.\n",
        "\n",
        "Duis aute irure dolor in reprehenderit in voluptate velit esse\n",
        "cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat\n",
        "cupidatat non proident, sunt in culpa qui officia deserunt\n",
        "mollit anim id est laborum.\n",
        "\n",
        "Sed ut perspiciatis unde omnis iste natus error sit voluptatem\n",
        "accusantium doloremque laudantium, totam rem aperiam, eaque ipsa\n",
        "quae ab illo inventore veritatis et quasi architecto beatae vitae\n",
        "dicta sunt explicabo.\n",
        "\n",
    );

    let block_bytes = block.as_bytes();
    let mut input = Vec::with_capacity(target_bytes + block_bytes.len());
    while input.len() < target_bytes {
        input.extend_from_slice(block_bytes);
    }
    // Truncate to a block-aligned boundary so we don't split mid-sentence.
    let complete_blocks = target_bytes / block_bytes.len();
    input.truncate(complete_blocks.max(1) * block_bytes.len());
    // Pad with spaces to reach exactly `target_bytes` so the bench name
    // (`lint_prose_heavy`) corresponds to a true 10KB input. Trailing
    // whitespace does not change scanner output.
    input.resize(target_bytes, b' ');
    input
}

fn lint_prose_heavy_benchmark(c: &mut Criterion) {
    let input = build_prose_input(10_000);
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
    // INTENTIONAL-STRICT: pure-prose bench pins the strict recognizer
    // for the same reason `lint_10kb` does — isolate scanner cost from
    // the dispatcher's decoder fallback. The prose input contains no
    // tokens the decoder would fire on, but pinning matches the
    // sibling benches and keeps the measurement deterministic.
    .with_recognizer(Arc::new(StrictRecognizer::new()));

    c.bench_function("lint_prose_heavy", |b| {
        b.iter(|| engine.lint(black_box(&input)));
    });
}

// ---------------------------------------------------------------------------
// Portion-dense advisory bench (perf/scanner-presize-allocators, issue #430)
// ---------------------------------------------------------------------------
//
// `lint_portion_dense` measures the scanner-output-SmallVec + PageContext
// portions-Vec allocator cost on a portion-rich input (20+ portion markings
// in a 10KB doc). This is the input shape that most exercises the pre-sized
// floors. Advisory bench — no entry in `benches/baseline.json`, same pattern
// as `lint_prose_heavy`. Report the number in PRs that touch the scanner or
// PageContext; don't gate on it.

fn build_portion_dense_input(target_bytes: usize) -> Vec<u8> {
    let block = concat!(
        "(U) Background paragraph one with unclassified portion marking.\n",
        "(C) Confidential portion follows. (S//NF) And a secret one.\n",
        "(U) Another unclassified portion in the running text.\n",
        "(S//REL TO USA, GBR) Secret releasable portion here.\n",
        "(U) Closing unclassified portion for this block.\n",
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

fn lint_portion_dense_benchmark(c: &mut Criterion) {
    let input = build_portion_dense_input(10_000);
    let engine = Engine::new(
        Config::default(),
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
    // INTENTIONAL-STRICT: portion-dense bench pins the strict
    // recognizer to isolate scanner + PageContext allocator cost
    // from the dispatcher's decoder fallback. Issue #430.
    .with_recognizer(Arc::new(StrictRecognizer::new()));

    c.bench_function("lint_portion_dense", |b| {
        b.iter(|| engine.lint(black_box(&input)));
    });
}

criterion_group!(
    benches,
    lint_latency_benchmark,
    decoder_latency_benchmark,
    lint_default_config_benchmark,
    lint_off_heavy_config_benchmark,
    lint_prose_heavy_benchmark,
    lint_portion_dense_benchmark,
);
criterion_main!(benches);
