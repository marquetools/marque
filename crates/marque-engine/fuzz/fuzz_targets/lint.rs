//! T072a — Fuzz target driving `Engine::lint` on arbitrary `&[u8]`.
//!
//! Assertions:
//!   (a) `lint` never panics
//!   (b) every emitted `Span` is within input bounds and satisfies `start <= end`
//!   (c) `fix`-then-`lint` does not panic and produces valid spans
//!
//! Run: `cargo +nightly fuzz run lint -- -max_total_time=60`
//! Not CI-gated in MVP; runs on nightly cron once infrastructure lands.

#![no_main]

use libfuzzer_sys::fuzz_target;
use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{Engine, FixMode};

fuzz_target!(|data: &[u8]| {
    // Bound input to 64KB to prevent OOM on pathological inputs.
    if data.len() > 65_536 {
        return;
    }

    let engine = Engine::new(Config::default(), vec![Box::new(capco_rules())]);

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

    // (c) fix-then-lint idempotency: no panics and valid spans
    let fixed = engine.fix(data, FixMode::Apply);
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
