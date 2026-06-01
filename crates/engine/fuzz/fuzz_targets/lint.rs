// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Fuzz target driving `Engine::lint` on arbitrary `&[u8]`.
//!
//! Assertions:
//!   (a) `lint` never panics
//!   (b) every emitted `Span` is within input bounds and satisfies `start <= end`
//!   (c) `fix` is idempotent: `fix(fix(x)).source == fix(x).source`
//!   (d) `fix`-then-`lint` produces valid spans
//!
//! Run: `cargo +nightly fuzz run lint -- -max_total_time=60`
//! Not CI-gated in MVP; runs on nightly cron once infrastructure lands.

#![no_main]

use libfuzzer_sys::fuzz_target;
use marque_config::Config;
use marque_engine::{CapcoEngine, FixMode, default_ruleset};
use std::sync::OnceLock;

static ENGINE: OnceLock<CapcoEngine> = OnceLock::new();

fn get_engine() -> &'static CapcoEngine {
    ENGINE.get_or_init(|| {
        CapcoEngine::new(
            Config::default(),
            default_ruleset(),
            marque_engine::default_scheme(),
        )
        .expect("default CAPCO scheme has no rewrite cycles")
    })
}

fuzz_target!(|data: &[u8]| {
    // Bound input to 64KB to prevent OOM on pathological inputs.
    if data.len() > 65_536 {
        return;
    }

    let engine = get_engine();

    // (a) lint never panics
    let result = engine.lint(data);

    // (b) every Span is within input bounds and start <= end
    for d in &result.diagnostics {
        assert!(
            d.span.start <= d.span.end,
            "span start ({}) > end ({})",
            d.span.start,
            d.span.end
        );
        assert!(
            d.span.end <= data.len(),
            "span end ({}) exceeds input length ({})",
            d.span.end,
            data.len()
        );
    }

    // (c) fix idempotency: applying fix twice yields same output as once
    let fixed = engine.fix(data, FixMode::Apply);
    let fixed2 = engine.fix(&fixed.source, FixMode::Apply);
    assert_eq!(
        fixed.source, fixed2.source,
        "fix is not idempotent: second application changed output"
    );

    // (d) fix-then-lint produces valid spans
    let relint = engine.lint(&fixed.source);
    for d in &relint.diagnostics {
        assert!(
            d.span.start <= d.span.end,
            "post-fix span start ({}) > end ({})",
            d.span.start,
            d.span.end
        );
        assert!(
            d.span.end <= fixed.source.len(),
            "post-fix span end ({}) exceeds fixed source length ({})",
            d.span.end,
            fixed.source.len()
        );
    }
});
