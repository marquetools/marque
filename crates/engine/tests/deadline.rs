// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Spec 005 (engine deadlines) — Phase 1 tests.
//!
//! Phase 1 lands the type surface (`LintOptions`, `FixOptions`,
//! `EngineError::DeadlineExceeded`) and the `_with_options`
//! signatures with zero behavior change. These tests pin both
//! invariants:
//!
//! 1. Default options yield no deadline / no threshold override.
//! 2. The back-compat shims (`Engine::lint`, `Engine::fix`,
//!    `Engine::fix_with_threshold`) produce identical results to
//!    their `_with_options` counterparts when called with the
//!    default options.
//!
//! Phase 2 will add the actual deadline-driven cancellation tests
//! (T013–T017).

use marque_capco::CapcoRuleSet;
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixOptions, InvalidThreshold, LintOptions};

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

// A fixture document that produces real diagnostics — a banner with
// a missing-USA-trigraph and a corrections-map candidate. Picking a
// non-trivial input ensures the back-compat parity tests actually
// exercise the rule loop, not just the empty-document path.
const TEST_SRC: &[u8] = b"SECRET//NOFORN\n\n(S//NF) Sample portion that triggers rules.\n";

// ---------------------------------------------------------------------------
// T005 — default options carry no deadline / no threshold override
// ---------------------------------------------------------------------------

#[test]
fn lint_options_default_yields_no_deadline() {
    let opts = LintOptions::default();
    assert!(opts.deadline.is_none());
}

#[test]
fn fix_options_default_yields_no_deadline_and_no_threshold_override() {
    let opts = FixOptions::default();
    assert!(opts.deadline.is_none());
    assert!(opts.threshold_override.is_none());
}

// ---------------------------------------------------------------------------
// T006 — back-compat shims produce identical results to `_with_options`
// ---------------------------------------------------------------------------

#[test]
fn lint_shim_matches_lint_with_options_default() {
    // The two paths must produce structurally identical results when
    // the options are at default. Using diagnostic count + per-message
    // equality avoids depending on `LintResult: PartialEq` (which the
    // type does not derive).
    let eng = engine();
    let via_shim = eng.lint(TEST_SRC);
    let via_options = eng.lint_with_options(TEST_SRC, &LintOptions::default());

    assert_eq!(
        via_shim.diagnostics.len(),
        via_options.diagnostics.len(),
        "shim vs _with_options diagnostic count differs"
    );
    for (a, b) in via_shim
        .diagnostics
        .iter()
        .zip(via_options.diagnostics.iter())
    {
        assert_eq!(a.rule, b.rule);
        assert_eq!(a.severity, b.severity);
        assert_eq!(a.span, b.span);
        assert_eq!(a.message, b.message);
        assert_eq!(a.citation, b.citation);
    }

    // Phase 1 keeps the new fields at their default zero values for
    // both code paths — Phase 2 wires real values in.
    assert!(!via_shim.truncated);
    assert!(!via_options.truncated);
    assert_eq!(via_shim.candidates_processed, 0);
    assert_eq!(via_options.candidates_processed, 0);
    assert_eq!(via_shim.candidates_total, 0);
    assert_eq!(via_options.candidates_total, 0);
}

#[test]
fn fix_with_threshold_ok_matches_fix_with_options_threshold_override() {
    // The Ok branch: a valid threshold override produces the same
    // FixResult through both entry points.
    let eng = engine();

    let via_threshold = eng
        .fix_with_threshold(TEST_SRC, FixMode::DryRun, Some(0.5))
        .expect("0.5 is in range");
    // `#[non_exhaustive]` forbids struct-literal construction across
    // crate boundaries, so build via Default + field assignment.
    let mut opts = FixOptions::default();
    opts.threshold_override = Some(0.5);
    let via_options = eng
        .fix_with_options(TEST_SRC, FixMode::DryRun, &opts)
        .expect("0.5 is in range");

    assert_eq!(via_threshold.source, via_options.source);
    assert_eq!(via_threshold.applied.len(), via_options.applied.len());
    assert_eq!(
        via_threshold.remaining_diagnostics.len(),
        via_options.remaining_diagnostics.len()
    );
}

#[test]
fn fix_with_threshold_invalid_threshold_path_matches_fix_with_options() {
    // The error branch: both entry points reject NaN as
    // `InvalidThreshold(_)`. `fix_with_threshold` returns the bare
    // `InvalidThreshold` (its public signature); `fix_with_options`
    // wraps it in `EngineError::InvalidThreshold`.
    let eng = engine();
    let nan = f32::NAN;

    let via_threshold = eng.fix_with_threshold(TEST_SRC, FixMode::DryRun, Some(nan));
    assert!(matches!(via_threshold, Err(InvalidThreshold(_))));

    let mut opts = FixOptions::default();
    opts.threshold_override = Some(nan);
    let via_options = eng.fix_with_options(TEST_SRC, FixMode::DryRun, &opts);
    assert!(matches!(
        via_options,
        Err(marque_engine::EngineError::InvalidThreshold(_))
    ));
}

#[test]
fn fix_shim_matches_fix_with_options_default() {
    // The plain `fix` shim feeds default options through; the result
    // must match `fix_with_options(.., &FixOptions::default())`.
    let eng = engine();
    let via_shim = eng.fix(TEST_SRC, FixMode::DryRun);
    let via_options = eng
        .fix_with_options(TEST_SRC, FixMode::DryRun, &FixOptions::default())
        .expect("default options cannot fail");

    assert_eq!(via_shim.source, via_options.source);
    assert_eq!(via_shim.applied.len(), via_options.applied.len());
    assert_eq!(
        via_shim.remaining_diagnostics.len(),
        via_options.remaining_diagnostics.len()
    );
}
