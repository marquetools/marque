// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! BatchEngine per-document deadline tests.
//!
//! Pins the contract that `BatchOptions::per_doc_deadline`:
//! - Stamps a fresh `Instant` after each document acquires its
//!   concurrency permit (so a slow earlier doc does not consume
//!   later docs' budgets, and `ConcurrencyController` wait time is
//!   excluded).
//! - On expiry surfaces `BatchError::DocumentDeadlineExceeded`
//!   for the fix path (Constitution V Principle V — no partial
//!   `FixResult`).
//! - Is matchable, displays informatively, and the predicates
//!   distinguish it from `is_panic()` / `is_shutdown()`.

#![cfg(feature = "batch")]

use futures::StreamExt;
use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{BatchEngine, BatchError, BatchOptions, CapcoEngine, FixResult, LintResult};
use std::time::Duration;

fn engine() -> CapcoEngine {
    CapcoEngine::new(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

/// Synthesize a document large enough that even a generous
/// per-doc deadline (50 ms) will trip on it. 20 000 banners is
/// the same shape the engine deadline-overhead bench uses,
/// scaled up so engine work reliably exceeds the budget on any
/// plausible host.
fn slow_doc() -> Vec<u8> {
    "SECRET//NF\n\n\n".repeat(20_000).into_bytes()
}

#[tokio::test]
async fn batch_per_doc_deadline_isolates_one_slow_doc_from_rest() {
    // Per-doc deadline, three docs. The slow doc must trip its own
    // budget; the fast docs must complete on theirs. With
    // `max_concurrent_docs: Some(4)` all three run in parallel,
    // so the slow doc cannot block the fast ones, and each doc's
    // deadline is stamped independently after its own permit
    // acquisition.
    let mut opts = BatchOptions::default();
    opts.max_concurrent_docs = Some(4);
    opts.per_doc_deadline = Some(Duration::from_millis(50));

    let batch = BatchEngine::new(engine(), opts);
    let docs: Vec<(String, Vec<u8>)> = vec![
        ("slow".to_owned(), slow_doc()),
        ("fast1".to_owned(), b"(U) Plain text.\n".to_vec()),
        ("fast2".to_owned(), b"(U) Also plain.\n".to_vec()),
    ];

    let mut results: Vec<(String, Result<FixResult, BatchError>)> =
        batch.fix_many(docs).collect().await;
    // Results stream in completion order; sort by id so the
    // assertions are deterministic regardless of which doc
    // finished first.
    results.sort_by(|a, b| a.0.cmp(&b.0));

    assert_eq!(results.len(), 3);
    assert_eq!(results[0].0, "fast1");
    assert_eq!(results[1].0, "fast2");
    assert_eq!(results[2].0, "slow");

    // Slow doc must trip the deadline.
    let slow_err = results[2]
        .1
        .as_ref()
        .expect_err("slow doc must produce an Err on a 50ms deadline");
    assert!(
        slow_err.is_deadline_exceeded(),
        "slow doc must report deadline exceeded; got: {slow_err:?}"
    );

    // Fast docs must succeed — they have a few microseconds of
    // engine work each, well under 50 ms.
    for (id, result) in &results[..2] {
        assert!(
            result.is_ok(),
            "fast doc {id} must succeed within the 50 ms budget; got: {result:?}"
        );
    }
}

#[tokio::test]
async fn batch_lint_many_truncates_slow_doc_without_erroring() {
    // The lint side surfaces a deadline trip as
    // `Ok(LintResult { truncated: true })` — partial diagnostics
    // are useful, so they flow through the success channel.
    let mut opts = BatchOptions::default();
    opts.max_concurrent_docs = Some(4);
    opts.per_doc_deadline = Some(Duration::from_millis(50));

    let batch = BatchEngine::new(engine(), opts);
    let docs: Vec<(String, Vec<u8>)> = vec![
        ("slow".to_owned(), slow_doc()),
        ("fast".to_owned(), b"(U) Plain.\n".to_vec()),
    ];

    let mut results: Vec<(String, Result<LintResult, BatchError>)> =
        batch.lint_many(docs).collect().await;
    results.sort_by(|a, b| a.0.cmp(&b.0));

    let (fast_id, fast_result) = &results[0];
    assert_eq!(fast_id, "fast");
    let fast_lint = fast_result.as_ref().expect("fast doc must succeed");
    assert!(
        !fast_lint.truncated,
        "fast doc must not be truncated; got: {fast_lint:?}"
    );

    let (slow_id, slow_result) = &results[1];
    assert_eq!(slow_id, "slow");
    let slow_lint = slow_result
        .as_ref()
        .expect("lint_many must NOT produce Err on deadline trip — partial Ok is the contract");
    assert!(
        slow_lint.truncated,
        "slow doc lint must be truncated; got: {slow_lint:?}"
    );
    assert!(
        slow_lint.candidates_processed < slow_lint.candidates_total,
        "truncated lint must show partial progress; got processed={}, total={}",
        slow_lint.candidates_processed,
        slow_lint.candidates_total
    );
}

#[tokio::test]
async fn batch_per_doc_deadline_overflow_does_not_silently_disable_budget() {
    // `Duration::MAX` is ~584 billion years; adding it to any
    // realistic `Instant` overflows. An earlier implementation used
    // `and_then(checked_add)` which silently returned `None` and
    // dropped the deadline — letting an operator-configured budget
    // disappear because of a clock-arithmetic edge case. The
    // current implementation maps overflow to "already expired" so
    // the engine's pre-pass deadline check trips immediately. This
    // test pins that behavior so a future regression can't quietly
    // re-introduce silent-drop semantics.
    let mut opts = BatchOptions::default();
    opts.max_concurrent_docs = Some(2);
    opts.per_doc_deadline = Some(Duration::MAX);

    let batch = BatchEngine::new(engine(), opts);
    let docs: Vec<(String, Vec<u8>)> = vec![
        ("doc1".to_owned(), b"(S//NF) Some text.\n".to_vec()),
        ("doc2".to_owned(), b"(U) More text.\n".to_vec()),
    ];

    let results: Vec<(String, Result<FixResult, BatchError>)> =
        batch.fix_many(docs).collect().await;
    // Both docs must report deadline exceeded, NOT succeed (which
    // would be the bug — overflow silently dropping the deadline).
    for (id, result) in &results {
        let err = result
            .as_ref()
            .expect_err("overflow must NOT disable the deadline");
        assert!(
            err.is_deadline_exceeded(),
            "doc {id} must report deadline exceeded; got: {err:?}"
        );
    }
}

#[tokio::test]
async fn batch_no_deadline_runs_all_docs_to_completion() {
    // Sanity: with `per_doc_deadline: None`, even the slow doc
    // completes. Confirms the deadline plumbing is opt-in and
    // doesn't accidentally trip when unset.
    let mut opts = BatchOptions::default();
    opts.max_concurrent_docs = Some(4);
    // per_doc_deadline = None (default)

    let batch = BatchEngine::new(engine(), opts);
    let docs: Vec<(String, Vec<u8>)> = vec![
        // Smaller "slow" doc here — without a deadline cap we
        // don't want to slow the test suite to a crawl on slow
        // CI runners. 1 000 banners is ~5 ms of engine work,
        // plenty to demonstrate completion without truncation.
        (
            "a".to_owned(),
            "SECRET//NF\n\n\n".repeat(1_000).into_bytes(),
        ),
        ("b".to_owned(), b"(U) Plain.\n".to_vec()),
    ];

    let results: Vec<(String, Result<FixResult, BatchError>)> =
        batch.fix_many(docs).collect().await;
    for (id, result) in &results {
        assert!(
            result.is_ok(),
            "doc {id} must succeed without a deadline; got: {result:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// `BatchError::DocumentDeadlineExceeded` shape and predicates.
// ---------------------------------------------------------------------------

#[test]
fn document_deadline_exceeded_is_matchable_and_distinguishable() {
    // Fabricate the variant directly. Construction across the
    // crate boundary works because `LintResult` is
    // `#[non_exhaustive]` but we use `Default::default()` plus
    // field assignment.
    let mut partial_lint = LintResult::default();
    partial_lint.truncated = true;
    partial_lint.candidates_processed = 17;
    partial_lint.candidates_total = 100;
    let err = BatchError::DocumentDeadlineExceeded { partial_lint };

    // Predicate: the deadline-trip is positively identified by
    // `is_deadline_exceeded()` and NOT by any of the other
    // predicates. A supervisor matching on `is_panic()` to alert
    // must not also fire on a routine deadline trip.
    assert!(err.is_deadline_exceeded());
    assert!(!err.is_panic());
    assert!(!err.is_shutdown());
    assert!(!err.is_cancelled());

    // Match arms work — pin the API shape so a destructuring
    // call site keeps compiling.
    match &err {
        BatchError::DocumentDeadlineExceeded { partial_lint } => {
            assert_eq!(partial_lint.candidates_processed, 17);
            assert_eq!(partial_lint.candidates_total, 100);
            assert!(partial_lint.truncated);
        }
        other => panic!("expected DocumentDeadlineExceeded, got: {other:?}"),
    }
}

#[test]
fn document_deadline_exceeded_display_carries_counts() {
    let mut partial_lint = LintResult::default();
    partial_lint.candidates_processed = 17;
    partial_lint.candidates_total = 100;
    let err = BatchError::DocumentDeadlineExceeded { partial_lint };
    let s = err.to_string();
    assert!(
        s.contains("17"),
        "Display should carry candidates_processed; got: {s:?}"
    );
    assert!(
        s.contains("100"),
        "Display should carry candidates_total; got: {s:?}"
    );
    assert!(
        s.contains("deadline"),
        "Display should name the failure mode; got: {s:?}"
    );
}

#[test]
fn document_deadline_exceeded_has_no_source() {
    // Like `EngineError::DeadlineExceeded`, the deadline trip is
    // a runtime condition with no underlying error to chain.
    // `source()` returns `None`.
    let err = BatchError::DocumentDeadlineExceeded {
        partial_lint: LintResult::default(),
    };
    assert!(
        std::error::Error::source(&err).is_none(),
        "DocumentDeadlineExceeded must not chain to a source"
    );
}

#[test]
fn other_batch_errors_do_not_report_deadline_exceeded() {
    // Negative cross-check: the `is_deadline_exceeded()` predicate
    // must fire ONLY for `DocumentDeadlineExceeded`. Construct
    // `ShutdownInProgress` directly and confirm.
    let err = BatchError::ShutdownInProgress;
    assert!(!err.is_deadline_exceeded());
    assert!(err.is_shutdown());
}
