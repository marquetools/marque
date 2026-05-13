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
        assert_eq!(a.rule, e.rule, "{label}: applied[{i}].rule");
        assert_eq!(a.span, e.span, "{label}: applied[{i}].span");
        // Compare the proposal envelope discriminant + carried
        // replacement bytes (TextCorrection variant only — FixIntent
        // payloads compare via their replacement intent discriminant).
        let same = match (&a.proposal, &e.proposal) {
            (
                marque_rules::AppliedFixProposal::TextCorrection { replacement: ra },
                marque_rules::AppliedFixProposal::TextCorrection { replacement: re },
            ) => ra == re,
            (
                marque_rules::AppliedFixProposal::FixIntent(ai),
                marque_rules::AppliedFixProposal::FixIntent(ei),
            ) => std::mem::discriminant(&ai.replacement) == std::mem::discriminant(&ei.replacement),
            _ => false,
        };
        assert!(same, "{label}: applied[{i}].proposal shape differs");
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
            assert_eq!(a_fix.source, b_fix.source);
            // FixIntent does not impl PartialEq (Confidence carries
            // f32). Compare the replacement variant discriminant
            // and confidence-combined() value — the fix's structural
            // identity for the shim parity check.
            assert_eq!(
                std::mem::discriminant(&a_fix.replacement),
                std::mem::discriminant(&b_fix.replacement)
            );
            assert!(
                (a_fix.confidence.combined() - b_fix.confidence.combined()).abs() < f32::EPSILON
            );
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
fn fix_with_deadline_during_fix_call_returns_deadline_exceeded() {
    // T017: a deadline that expires sometime during a `fix_with_options`
    // call MUST surface as `Err(EngineError::DeadlineExceeded { partial_lint })`,
    // never as a partial `FixResult` (Constitution V Principle V — the
    // half-applied buffer is dropped on the floor and the caller sees only
    // the lint result).
    //
    // The spec describes two distinct trip points: the post-lint check
    // (T010) and the per-fix-application check (T011). Both produce the
    // same `Err(DeadlineExceeded)` shape, and both are exercised here by
    // setting a deadline that will trip somewhere inside the call. Which
    // exact path fires is hardware-dependent:
    //
    //   - On hardware where the lint phase dominates `fix` runtime
    //     (typical for slow CI runners, where `lint` can take 100×
    //     `apply` for the same fixture), the deadline trips inside the
    //     candidate loop and we get `truncated: true` with partial
    //     diagnostics. The fix-side path that converts that to
    //     `EngineError::DeadlineExceeded` is the post-lint check (T010).
    //   - On hardware where the apply phase is slow enough to be
    //     observable (fast machines with the FixMode::Apply variant on
    //     a large buffer), the deadline can trip inside the apply
    //     loop, yielding `truncated: false` with full diagnostics.
    //     That exercises the per-fix check (T011).
    //
    // The test accepts either observation because both prove the
    // deadline plumbing works through `fix_with_options`. The previous
    // version asserted specifically the second case but that's not
    // reliably reachable on slow CI hardware where apply takes
    // microseconds — there is simply no margin window where
    // `deadline > lint_time && deadline < lint_time + apply_time`.
    let eng = engine();
    // Bounded fixture size — large enough that `fix` takes long
    // enough on every machine class for a half-baseline deadline
    // to land mid-call, small enough that the test does not bloat
    // the suite runtime in debug/CI. 4_000 banners produces ~50KB
    // of source and ~4_000 fix proposals; on slow CI runners this
    // typically runs in 50–200ms total (warmup + baseline +
    // bounded run), versus the prior 20K-banner version which
    // could spend several seconds in the apply loop alone.
    let src = many_banners_with_fixes(4_000);

    // Warm caches WITHOUT paying the full apply cost up front:
    // the lint pass on the real fixture is what dominates the
    // baseline below, so warming lint is sufficient. A separate
    // tiny `fix(Apply)` warm-up exercises the apply codepath
    // (so the splice and audit-record paths are also warm)
    // without scaling that warm-up cost with the test fixture.
    let _ = eng.lint(&src);
    let warmup_src = many_banners_with_fixes(128);
    let _ = eng.fix(&warmup_src, FixMode::Apply);

    let baseline_start = Instant::now();
    let _ = eng.fix(&src, FixMode::Apply);
    let t_fix = baseline_start.elapsed();

    // Half the warm baseline is reliably mid-`fix` on every machine
    // class: by the time the deadline trips, either lint is in the
    // candidate loop (slow machines where lint dominates) or apply is
    // in its iteration loop (fast machines where lint completes in
    // <50% of the budget). Both paths terminate as
    // `Err(DeadlineExceeded)`, which is the load-bearing behavior the
    // test pins. A budget of 0 would test the pre-pass abort, which
    // T016 already covers.
    let mut opts = FixOptions::default();
    opts.deadline = Some(Instant::now() + t_fix / 2);

    match eng.fix_with_options(&src, FixMode::Apply, &opts) {
        Err(EngineError::DeadlineExceeded { partial_lint }) => {
            // Either path is acceptable. We only sanity-check that the
            // result is internally consistent — `processed` cannot
            // exceed `total`, ever — to catch any future regression
            // that scrambles the count fields when constructing the
            // partial_lint.
            assert!(
                partial_lint.candidates_processed <= partial_lint.candidates_total,
                "partial_lint counts inconsistent: processed={} total={}",
                partial_lint.candidates_processed,
                partial_lint.candidates_total
            );
        }
        Ok(_) => panic!(
            "expected Err(DeadlineExceeded) from a deadline at half the warm fix baseline \
             ({:?}); the deadline plumbing is not converting expiry into Err",
            t_fix / 2
        ),
        Err(other) => panic!("expected Err(DeadlineExceeded), got Err({other:?})"),
    }
}
