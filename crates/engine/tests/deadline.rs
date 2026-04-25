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
use marque_engine::{Engine, EngineError, FixMode, FixOptions, InvalidThreshold, LintOptions};
use std::time::{Duration, Instant};

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

    // Phase 2 wires the truncation / candidate-count fields. With
    // default options (no deadline) both code paths must produce a
    // non-truncated result and identical, non-zero candidate counts
    // — anything else means the back-compat shim drifted from
    // `_with_options(default)`.
    assert!(!via_shim.truncated);
    assert!(!via_options.truncated);
    assert_eq!(
        via_shim.candidates_processed,
        via_options.candidates_processed
    );
    assert_eq!(via_shim.candidates_total, via_options.candidates_total);
    // A non-trivial fixture must produce at least one processed
    // candidate; a regression that silently zero-counted would
    // otherwise pass the equality check above.
    assert!(via_shim.candidates_processed > 0);
    // No pass should report `processed > total` — that's an
    // accounting bug the shim parity test is well-positioned to
    // catch.
    assert!(via_shim.candidates_processed <= via_shim.candidates_total);
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

// ---------------------------------------------------------------------------
// Phase 2 — cooperative cancellation (T013–T017)
// ---------------------------------------------------------------------------

/// Build a synthetic document with `count` portion candidates so that
/// the candidate loop has enough work to make a per-candidate deadline
/// check observably trip mid-document. Each portion is a real, valid
/// CAPCO marking, so the rule loop runs the same code path as production
/// — the test exercises the deadline plumbing, not a degenerate fast
/// path that would skip rules.
fn many_portions(count: usize) -> Vec<u8> {
    let mut buf = String::with_capacity(count * 32);
    for _ in 0..count {
        buf.push_str("(S//NF) Portion text content here.\n");
    }
    buf.into_bytes()
}

/// Build a document where every banner triggers an E001 fix (`NF`
/// abbreviated dissem in banner — banner form requires `NOFORN`).
/// Each page-break-separated banner is a banner candidate that
/// reliably yields a real `FixProposal`, so the apply loop has
/// concrete work to do — load-bearing for the T017 mid-apply
/// deadline test, where the apply phase must take observable
/// wall-clock time for a per-fix deadline check to fire.
fn many_banners_with_fixes(count: usize) -> Vec<u8> {
    // Three newlines between banners trip the scanner's
    // `\n\n\n+` page-break heuristic, so each banner sits on
    // its own page and the engine treats each as a separate
    // banner candidate.
    let mut buf = String::with_capacity(count * 16);
    for _ in 0..count {
        buf.push_str("SECRET//NF\n\n\n");
    }
    buf.into_bytes()
}

#[test]
fn lint_with_already_expired_deadline_returns_immediately_truncated() {
    // T013: an already-expired deadline trips the pre-pass check
    // (T007). Spec §R3: the engine MUST return immediately with
    // `truncated: true` and both candidate counters at 0 — the
    // scanner does not run, so `candidates_total` stays at its
    // initial 0 even though the source has plenty of candidates.
    let eng = engine();
    let mut opts = LintOptions::default();
    opts.deadline = Some(Instant::now() - Duration::from_secs(1));

    let src = many_portions(50);
    let result = eng.lint_with_options(&src, &opts);

    assert!(result.truncated, "already-expired deadline must truncate");
    assert_eq!(
        result.candidates_processed, 0,
        "pre-pass abort processes zero candidates"
    );
    assert_eq!(
        result.candidates_total, 0,
        "pre-pass abort runs before the scanner — total stays at 0"
    );
    assert!(
        result.diagnostics.is_empty(),
        "no rule loop ran, no diagnostics"
    );
}

#[test]
fn lint_truncates_mid_document_at_deadline_boundary() {
    // T014: a deadline that is valid on entry but expires partway
    // through the candidate loop must cause the engine to break
    // out of the loop with `truncated: true` and a partial count
    // (`processed < total`). The loop check (T008) is the
    // load-bearing assertion — without it the engine would
    // overrun the budget by the full document's worth of rule
    // work.
    //
    // Reliability strategy: `Instant::now()` is the only clock
    // the engine has, so we cannot mock time. A tight budget
    // proportional to the unmetered baseline reliably trips
    // the loop on every machine class without flaking. The
    // critical assertions (`truncated` and `processed <
    // total`) are robust to scheduler noise — a too-tight
    // budget will fire on iteration 1 (still truncated, still
    // processed < total), and a too-generous budget would
    // run to completion and FAIL the truncated assertion
    // (never silently pass).
    let eng = engine();

    // Warm up so the actual measurement is on hot caches.
    let src = many_portions(3_000);
    let _ = eng.lint(&src);

    let baseline_start = Instant::now();
    let _ = eng.lint(&src);
    let baseline = baseline_start.elapsed();

    // 5% of baseline forces an early trip and leaves the loop
    // truncated with a definite `processed < total`. Whether
    // `processed` is 0 or some small positive value depends on
    // how much of the budget the scanner itself consumes
    // before the per-candidate check fires; either is a valid
    // observation of the truncation contract.
    let budget = baseline / 20;
    let mut opts = LintOptions::default();
    opts.deadline = Some(Instant::now() + budget);

    let result = eng.lint_with_options(&src, &opts);

    assert!(
        result.truncated,
        "mid-document deadline must produce truncated: true; \
         got truncated=false processed={} total={}",
        result.candidates_processed, result.candidates_total
    );
    assert!(
        result.candidates_total > 0,
        "scanner must have produced candidates"
    );
    assert!(
        result.candidates_processed < result.candidates_total,
        "loop must break before all candidates are processed; \
         processed={} total={}",
        result.candidates_processed,
        result.candidates_total
    );
}

#[test]
fn lint_with_generous_deadline_runs_to_completion_no_truncation() {
    // T015: a deadline far in the future must NOT truncate. The
    // engine completes the candidate loop normally; `truncated:
    // false` and `processed == total`. This is the load-bearing
    // negative test — it pins the invariant that a deadline
    // wide enough to finish does not introduce spurious
    // truncation.
    let eng = engine();
    let mut opts = LintOptions::default();
    opts.deadline = Some(Instant::now() + Duration::from_secs(3600));

    let src = many_portions(100);
    let result = eng.lint_with_options(&src, &opts);

    assert!(
        !result.truncated,
        "generous deadline must not truncate; got truncated=true"
    );
    assert!(
        result.candidates_total > 0,
        "scanner produced no candidates from a 100-portion fixture; \
         the test fixture is broken"
    );
    assert_eq!(
        result.candidates_processed, result.candidates_total,
        "no truncation means processed == total; got {}/{}",
        result.candidates_processed, result.candidates_total
    );
}

#[test]
fn fix_with_already_expired_deadline_returns_deadline_exceeded() {
    // T016: an already-expired deadline on `fix_with_options`
    // must surface as `Err(EngineError::DeadlineExceeded {
    // partial_lint })` per spec §R4. The asymmetric shape
    // protects audit-record integrity (Constitution V Principle
    // V): no partial `FixResult` is ever constructed, no
    // half-applied bytes leak. `partial_lint` carries the
    // pre-pass-truncated lint, which has zero diagnostics
    // because the lint scanner never ran.
    let eng = engine();
    let mut opts = FixOptions::default();
    opts.deadline = Some(Instant::now() - Duration::from_secs(1));

    let src = many_portions(20);
    let result = eng.fix_with_options(&src, FixMode::DryRun, &opts);

    match result {
        Err(EngineError::DeadlineExceeded { partial_lint }) => {
            assert!(
                partial_lint.truncated,
                "partial_lint must reflect that lint pass was deadline-truncated"
            );
            assert!(
                partial_lint.diagnostics.is_empty(),
                "lint pass returned before scanning, so diagnostics must be empty; \
                 got {}",
                partial_lint.diagnostics.len()
            );
            assert_eq!(partial_lint.candidates_processed, 0);
            assert_eq!(partial_lint.candidates_total, 0);
        }
        Ok(_) => panic!("expected Err(DeadlineExceeded), got Ok(_)"),
        Err(other) => panic!("expected Err(DeadlineExceeded), got Err({other:?})"),
    }
}

#[test]
fn fix_with_deadline_during_apply_loop_returns_deadline_exceeded_with_partial_lint() {
    // T017: a deadline crafted to fall AFTER the lint pass
    // completes but BEFORE the apply loop finishes must surface
    // as `Err(DeadlineExceeded)` with `partial_lint` carrying
    // the FULL set of diagnostics — the lint pass itself was
    // not truncated. The audit-integrity invariant
    // (Constitution V Principle V) holds: the half-applied
    // buffer is dropped on the floor and the caller sees only
    // the lint result.
    //
    // Reliability strategy: use `FixMode::Apply` on a large
    // buffer so the apply loop's `Vec::splice` cost (O(bytes
    // remaining after splice point) per fix) makes the apply
    // phase comfortably observable on the wall clock — in
    // `DryRun` the apply phase does ~no work and any
    // mid-apply deadline trips are unreachable in practice.
    // Then set the deadline to the measured lint time plus a
    // small margin. The margin is consumed by the apply
    // loop's first-few-iterations splices and the per-fix
    // deadline check fires.
    //
    // The acceptance is two-sided: the test passes only if
    // (a) `Err(DeadlineExceeded)` fires AND (b) `partial_lint`
    // is non-truncated and non-empty. A too-tight margin
    // truncates lint and fails loudly; a too-generous margin
    // yields `Ok` and also fails loudly — neither converts
    // into a silent pass. The retry loop absorbs scheduler
    // jitter without weakening that contract.
    let eng = engine();
    // Banner fixture (each banner produces an E001 fix) so the
    // apply loop has concrete work to do. The apply phase's
    // `Vec::splice` cost is O(buffer_size - splice_position)
    // per fix; reverse-byte iteration sums to O(N²) total
    // bytes shifted, which dwarfs lint at sufficient N.
    let src = many_banners_with_fixes(20_000);

    // Warm up caches so the next lint and fix calls are
    // representative of the steady-state cost the deadline
    // budget must respect.
    let _ = eng.fix(&src, FixMode::Apply);

    // Measure lint-only time. The lint pass inside `fix`
    // takes the same wall-clock time on warm caches; this
    // gives the lower bound the deadline must clear so the
    // post-lint check passes and the trip falls inside the
    // apply loop.
    let lint_start = Instant::now();
    let _ = eng.lint(&src);
    let t_lint = lint_start.elapsed();

    // Margins span a wide range so jitter on different
    // hardware classes still finds a working budget. The
    // search ends on the first margin that produces a clean
    // apply-loop trip; "clean" means partial_lint is
    // non-truncated and non-empty. Any other observation is
    // collected for the failure message.
    let margin_attempts = [
        Duration::from_millis(1),
        Duration::from_millis(2),
        Duration::from_millis(5),
        Duration::from_millis(10),
        Duration::from_millis(20),
        Duration::from_millis(50),
    ];

    let mut last_observation: Option<(bool, usize)> = None;
    let mut got_ok = false;
    for margin in margin_attempts {
        let mut opts = FixOptions::default();
        opts.deadline = Some(Instant::now() + t_lint + margin);

        match eng.fix_with_options(&src, FixMode::Apply, &opts) {
            Err(EngineError::DeadlineExceeded { partial_lint }) => {
                let truncated = partial_lint.truncated;
                let diag_count = partial_lint.diagnostics.len();
                last_observation = Some((truncated, diag_count));
                if !truncated && diag_count > 0 {
                    // Apply-loop trip with full lint. Done.
                    return;
                }
                // Lint truncated — margin too tight; the next
                // (larger) margin should clear it.
            }
            Ok(_) => {
                got_ok = true;
                // Margin too generous; a smaller margin would
                // trip, but our list is monotonically larger.
                // We stop searching here — the previous
                // margin already produced a non-clean trip
                // (recorded in `last_observation`) and any
                // larger margin will also pass cleanly.
                break;
            }
            Err(other) => panic!("unexpected error variant: {other:?}"),
        }
    }
    panic!(
        "no margin from {:?} above lint baseline ({:?}) produced an \
         apply-loop deadline trip with a non-truncated partial_lint. \
         last observation: truncated={:?} diag_count={:?}, saw_ok={got_ok}",
        margin_attempts,
        t_lint,
        last_observation.map(|(t, _)| t),
        last_observation.map(|(_, c)| c),
    );
}
