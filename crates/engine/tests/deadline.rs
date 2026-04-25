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

/// Compare two `FixResult`s field-by-field for shim-parity tests.
///
/// `FixResult` (and the types it contains) does not derive `PartialEq`
/// — `AppliedFix::timestamp` and `Confidence`'s f32 axes can drift in
/// ways that aren't load-bearing. This helper asserts equality on the
/// fields that *should* match between two equivalent code paths
/// (source bytes, audit-record content, remaining diagnostics) so a
/// regression that produces the same counts but different entries
/// fails the test.
fn assert_fix_results_match_byte_for_byte(
    actual: &marque_engine::FixResult,
    expected: &marque_engine::FixResult,
    label: &str,
) {
    assert_eq!(actual.source, expected.source, "{label}: source bytes");
    assert_eq!(
        actual.applied.len(),
        expected.applied.len(),
        "{label}: applied count"
    );
    for (i, (a, e)) in actual
        .applied
        .iter()
        .zip(expected.applied.iter())
        .enumerate()
    {
        assert_eq!(
            a.proposal.rule, e.proposal.rule,
            "{label}: applied[{i}].rule"
        );
        assert_eq!(
            a.proposal.span, e.proposal.span,
            "{label}: applied[{i}].span"
        );
        assert_eq!(
            a.proposal.replacement, e.proposal.replacement,
            "{label}: applied[{i}].replacement"
        );
        assert_eq!(a.source, e.source, "{label}: applied[{i}].source");
        assert_eq!(a.dry_run, e.dry_run, "{label}: applied[{i}].dry_run");
    }
    assert_eq!(
        actual.remaining_diagnostics.len(),
        expected.remaining_diagnostics.len(),
        "{label}: remaining count"
    );
    for (i, (a, e)) in actual
        .remaining_diagnostics
        .iter()
        .zip(expected.remaining_diagnostics.iter())
        .enumerate()
    {
        assert_eq!(a.rule, e.rule, "{label}: remaining[{i}].rule");
        assert_eq!(a.severity, e.severity, "{label}: remaining[{i}].severity");
        assert_eq!(a.span, e.span, "{label}: remaining[{i}].span");
        assert_eq!(a.message, e.message, "{label}: remaining[{i}].message");
    }
}

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
        // Catch divergence in fix proposals — the shim could produce
        // identical metadata but mismatched replacements / spans /
        // sources without this. `FixProposal` does not derive `PartialEq`
        // (the Confidence axes are f32), so compare the load-bearing
        // fields explicitly.
        assert_eq!(
            a.fix.is_some(),
            b.fix.is_some(),
            "shim vs _with_options fix presence differs for {:?}",
            a.rule
        );
        if let (Some(a_fix), Some(b_fix)) = (&a.fix, &b.fix) {
            assert_eq!(a_fix.rule, b_fix.rule);
            assert_eq!(a_fix.span, b_fix.span);
            assert_eq!(a_fix.replacement, b_fix.replacement);
            assert_eq!(a_fix.source, b_fix.source);
        }
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

    assert_fix_results_match_byte_for_byte(
        &via_threshold,
        &via_options,
        "fix_with_threshold vs fix_with_options",
    );
}

#[test]
fn fix_with_threshold_invalid_threshold_path_matches_fix_with_options() {
    // The error branch: both entry points reject NaN as
    // `InvalidThreshold(_)`. `fix_with_threshold` returns the bare
    // `InvalidThreshold` (its public signature); `fix_with_options`
    // wraps it in `EngineError::InvalidThreshold`. Beyond the variant
    // shape, the NaN payload itself MUST survive through both paths
    // — a regression that swallowed the offending value (e.g.,
    // re-mapping to a sentinel like 0.0 in the wrap layer) would
    // change the user-facing diagnostic and the error's `Display`
    // string. Destructuring pins it.
    let eng = engine();
    let nan = f32::NAN;

    let via_threshold = eng.fix_with_threshold(TEST_SRC, FixMode::DryRun, Some(nan));
    match via_threshold {
        Err(InvalidThreshold(value)) => assert!(value.is_nan()),
        other => panic!("expected Err(InvalidThreshold(NaN)), got {other:?}"),
    }

    let mut opts = FixOptions::default();
    opts.threshold_override = Some(nan);
    let via_options = eng.fix_with_options(TEST_SRC, FixMode::DryRun, &opts);
    match via_options {
        Err(marque_engine::EngineError::InvalidThreshold(InvalidThreshold(value))) => {
            assert!(value.is_nan());
        }
        other => panic!(
            "expected Err(EngineError::InvalidThreshold(InvalidThreshold(NaN))), got {other:?}"
        ),
    }
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

    assert_fix_results_match_byte_for_byte(
        &via_shim,
        &via_options,
        "fix shim vs fix_with_options(default)",
    );
}
