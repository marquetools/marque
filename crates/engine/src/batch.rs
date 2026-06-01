// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Concurrent batch processing over many documents.
//!
//! `BatchEngine` wraps `Engine` behind an `Arc` and uses `ConcurrencyController`
//! from `marque-utils` to enforce row and byte limits on in-flight work.
//!
//! CPU-bound lint/fix work is dispatched to tokio's blocking thread pool via
//! `spawn_blocking`, keeping the async executor free for I/O-bound coordination.
//!
//! Results stream out in **completion order** (fastest documents first), not
//! submission order. Callers correlate results by the `id` field echoed back
//! alongside each result.
//!
//! # Example
//!
//! ```rust,no_run
//! use marque_engine::{CapcoEngine, batch::{BatchEngine, BatchOptions}};
//! use futures::StreamExt;
//! use std::time::Duration;
//!
//! # async fn example(engine: CapcoEngine) {
//! // `BatchOptions` is `#[non_exhaustive]`, so construct via
//! // `Default::default()` + field assignment.
//! let mut options = BatchOptions::default();
//! options.max_concurrent_docs = Some(16);
//! options.max_inflight_bytes = Some(256 * 1024 * 1024); // 256 MiB
//! options.per_doc_deadline = Some(Duration::from_secs(5));
//! let batch = BatchEngine::new(engine, options);
//!
//! let docs = vec![
//!     ("doc1".to_owned(), b"TOP SECRET//SI".to_vec()),
//!     ("doc2".to_owned(), b"SECRET//NOFORN".to_vec()),
//! ];
//!
//! let mut results = batch.lint_many(docs);
//! while let Some((id, result)) = results.next().await {
//!     match result {
//!         Ok(lint) => println!("{id}: {} diagnostics", lint.diagnostics.len()),
//!         Err(e) => eprintln!("{id}: failed: {e}"),
//!     }
//! }
//! # }
//! ```

use std::sync::Arc;
// Batch processing uses `std::time::Instant`. `BatchEngine` depends
// on tokio (gated behind the `batch` Cargo feature), and tokio
// itself does not target `wasm32-unknown-unknown`, so this module
// never reaches the WASM clock-polyfill question — std's `Instant`
// is sufficient.
use std::time::{Duration, Instant};

use futures::{Stream, StreamExt, stream};
use marque_utils::concur_control::{ConcurrencyController, Options as ConcurOptions};

use crate::CapcoEngine;

use crate::{EngineError, FixOptions, FixResult, LintOptions, LintResult};

/// Error returned when a single document in a batch fails to process.
///
/// Batch APIs surface this per-document so a panic, cancellation, or
/// graceful shutdown of the underlying concurrency controller does not
/// abort the entire batch run.
///
/// `#[non_exhaustive]` because future infrastructure-level errors
/// (deadline expired, cache write-through failed, queue overflow,
/// etc.) will land as new variants alongside the existing two. A
/// downstream `match` should always carry a wildcard arm; without
/// `non_exhaustive` every new variant would be a semver-breaking
/// change for consumers, which would either pin them to a stale
/// version or pressure us to never grow the surface.
#[derive(Debug)]
#[non_exhaustive]
pub enum BatchError {
    /// The blocking lint/fix task panicked or was cancelled.
    TaskFailed(tokio::task::JoinError),
    /// The `ConcurrencyController` semaphore was closed while this
    /// document was waiting for a permit. Indicates the runtime is in
    /// shutdown — the caller has no work to do beyond observing the
    /// error and ending its loop.
    ///
    /// Whitepaper §9.4 / gap register #8 carved this out as a separate
    /// variant so deployment supervisors can distinguish it from a
    /// real worker-task panic. `is_panic()` returns `false` for this
    /// variant; `is_shutdown()` returns `true`.
    ShutdownInProgress,
    /// `fix_many` aborted this document's fix pass because the
    /// per-document deadline (set on `BatchOptions::per_doc_deadline`)
    /// expired. Spec 005 §R4 / Constitution V Principle V — no partial
    /// `FixResult` is ever produced; the caller receives the partial
    /// `LintResult` so it can render whatever diagnostics the engine
    /// surfaced before the abort.
    ///
    /// `is_deadline_exceeded()` returns `true` for this variant only.
    /// `is_panic()` and `is_shutdown()` return `false` — a deadline
    /// trip is a routine operational signal, not a worker bug or
    /// runtime shutdown.
    ///
    /// Note: only the **fix** path produces this variant. `lint_many`
    /// surfaces a deadline-truncated lint as `Ok(LintResult { truncated:
    /// true, .. })` so the partial diagnostics flow through the same
    /// success channel — there is no asymmetric response shape on the
    /// lint side because no audit-stream invariant is at risk.
    DocumentDeadlineExceeded {
        /// The lint pass produced before the deadline tripped. May
        /// itself be truncated (`partial_lint.truncated`) if the
        /// deadline expired during the lint phase rather than the
        /// fix-application phase.
        partial_lint: LintResult,
    },
    /// The engine's `require_signature` policy is set, but `BatchEngine`
    /// supplies no per-document signature (issue #399). Batch fixing is
    /// bulk, non-interactive processing — there is no per-call channel
    /// to attach a classifier signature — so under a `require_signature`
    /// deployment every document fails this gate. A future full-signing
    /// design (the #399 follow-up) can thread per-document signatures
    /// through `BatchOptions`; until then this variant reports the
    /// policy refusal explicitly rather than silently dropping work.
    ///
    /// `is_panic()` / `is_cancelled()` / `is_shutdown()` /
    /// `is_deadline_exceeded()` all return `false` — it is a
    /// configuration/policy condition, not a runtime failure.
    SignatureRequired,
}

impl BatchError {
    /// Returns `true` if the error was caused by a panic in the worker task.
    ///
    /// CI pipelines and supervisors should treat this as an application bug
    /// that warrants investigation (not a transient infrastructure issue).
    pub fn is_panic(&self) -> bool {
        match self {
            Self::TaskFailed(e) => e.is_panic(),
            Self::ShutdownInProgress => false,
            Self::DocumentDeadlineExceeded { .. } => false,
            Self::SignatureRequired => false,
        }
    }

    /// Returns `true` if the error was caused by task cancellation (e.g.,
    /// runtime shutdown, explicit abort).
    ///
    /// Cancellation is an expected operational event — callers that see
    /// this during a graceful shutdown should typically log-and-continue,
    /// not alert.
    pub fn is_cancelled(&self) -> bool {
        match self {
            Self::TaskFailed(e) => e.is_cancelled(),
            Self::ShutdownInProgress => false,
            Self::DocumentDeadlineExceeded { .. } => false,
            Self::SignatureRequired => false,
        }
    }

    /// Returns `true` if the error was caused by the `ConcurrencyController`
    /// semaphore being closed while this document was awaiting a permit.
    ///
    /// Distinct from `is_cancelled()` (which fires when a worker task is
    /// aborted mid-execution) and from `is_panic()` (which fires on a real
    /// bug). Shutdown is the routine end-of-life signal — supervisors
    /// should drain any remaining items in the result stream and exit.
    pub fn is_shutdown(&self) -> bool {
        matches!(self, Self::ShutdownInProgress)
    }

    /// Returns `true` if this error was caused by the per-document
    /// deadline expiring during a `fix_many` call.
    ///
    /// Routine operational signal — the document took longer to
    /// process than its budget allowed. Callers should render the
    /// embedded `partial_lint` diagnostics and either skip the
    /// document or retry with a larger budget. Distinct from
    /// `is_panic()` (worker bug) and `is_shutdown()` (runtime
    /// end-of-life).
    pub fn is_deadline_exceeded(&self) -> bool {
        matches!(self, Self::DocumentDeadlineExceeded { .. })
    }
}

impl std::fmt::Display for BatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TaskFailed(e) => {
                let kind = if e.is_panic() {
                    "panicked"
                } else if e.is_cancelled() {
                    "was cancelled"
                } else {
                    "failed"
                };
                write!(f, "batch task {kind}: {e}")
            }
            Self::ShutdownInProgress => {
                f.write_str("ConcurrencyController semaphore closed (shutdown in progress)")
            }
            Self::DocumentDeadlineExceeded { partial_lint } => write!(
                f,
                "document deadline exceeded after {}/{} candidates",
                partial_lint.candidates_processed, partial_lint.candidates_total
            ),
            Self::SignatureRequired => f.write_str(
                "fix requires a signature (require_signature is set) but BatchEngine supplies none",
            ),
        }
    }
}

impl std::error::Error for BatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TaskFailed(e) => Some(e),
            Self::ShutdownInProgress => None,
            // Like `EngineError::DeadlineExceeded`, the deadline trip
            // is not caused by an inner error — it reports a runtime
            // condition (the deadline elapsed) with no underlying
            // failure to chain.
            Self::DocumentDeadlineExceeded { .. } => None,
            Self::SignatureRequired => None,
        }
    }
}

impl From<tokio::task::JoinError> for BatchError {
    fn from(e: tokio::task::JoinError) -> Self {
        Self::TaskFailed(e)
    }
}

impl From<tokio::sync::AcquireError> for BatchError {
    fn from(_: tokio::sync::AcquireError) -> Self {
        // `AcquireError` carries no information beyond "semaphore was
        // closed" — and the semaphore here is owned by `BatchEngine`,
        // so closure means the engine is shutting down. Surface that
        // intent explicitly rather than re-exporting tokio's type.
        Self::ShutdownInProgress
    }
}

/// Concurrency limits and per-document budgets for batch processing.
///
/// All fields are optional and independent. When both concurrency limits
/// are set the more restrictive one governs at any given moment;
/// `per_doc_deadline` is orthogonal and applies separately to each
/// document's permit-acquired execution slice.
///
/// # Breaking change in this release
///
/// This struct gained `#[non_exhaustive]` and a new `per_doc_deadline`
/// field in spec 005 Phase 3d. **Downstream code that previously
/// constructed `BatchOptions` with a struct literal**
/// (`BatchOptions { max_concurrent_docs, max_inflight_bytes }`) **will
/// no longer compile** — `#[non_exhaustive]` blocks cross-crate
/// struct-literal construction unconditionally, even when every
/// existing field is supplied. Switch to
/// `Default::default()` + public field assignment, shown below. (The
/// CHANGELOG / release notes for this version surface this explicitly.)
///
/// `#[non_exhaustive]` was added so future per-doc concerns (memory
/// budgets, per-rule deadlines, cancellation tokens) can join without
/// a further breaking-change cycle for downstream callers using the
/// recommended construction pattern.
///
/// ```rust,no_run
/// use marque_engine::BatchOptions;
/// use std::time::Duration;
///
/// let mut opts = BatchOptions::default();
/// opts.per_doc_deadline = Some(Duration::from_secs(5));
/// ```
#[non_exhaustive]
pub struct BatchOptions {
    /// Maximum documents in-flight simultaneously.
    ///
    /// This field drives **two** independent limits that both happen to
    /// share this value:
    ///
    /// 1. `ConcurrencyController::max_inflight_rows` — the semaphore that
    ///    rate-limits how many documents can hold permits at the same time.
    /// 2. `buffer_unordered` cap — how many per-document futures are
    ///    created and polled ahead of readiness.
    ///
    /// In practice they are always set together: the effective maximum is
    /// the minimum of whichever blocks first for a given workload.
    /// Defaults to 32.
    pub max_concurrent_docs: Option<usize>,

    /// Maximum total bytes of document content in-flight simultaneously.
    ///
    /// Useful for memory-bounded batch runs over large corpora. `None` means
    /// unlimited (byte accounting is still tracked for observability).
    pub max_inflight_bytes: Option<usize>,

    /// Per-document wall-clock budget (spec 005 §R2). When `Some(d)`,
    /// each document's lint/fix call gets its own deadline of
    /// `Instant::now() + d` stamped **after** the document acquires
    /// its concurrency permit — `ConcurrencyController` wait time
    /// does not consume the budget. A slow document does not borrow
    /// from a fast document's slice.
    ///
    /// On expiry: lint returns `Ok(LintResult { truncated: true, .. })`
    /// (partial diagnostics matter to the caller). Fix returns
    /// `Err(BatchError::DocumentDeadlineExceeded { partial_lint })`
    /// per Constitution V Principle V — no partial `FixResult` is
    /// ever produced.
    ///
    /// `None` (default) means no per-document deadline.
    pub per_doc_deadline: Option<Duration>,
}

impl Default for BatchOptions {
    fn default() -> Self {
        Self {
            max_concurrent_docs: Some(32),
            max_inflight_bytes: None,
            per_doc_deadline: None,
        }
    }
}

/// Wraps `Engine` for concurrent multi-document processing with backpressure.
///
/// The underlying `Engine` is shared via `Arc`; cloning `BatchEngine` is cheap.
pub struct BatchEngine {
    engine: Arc<CapcoEngine>,
    controller: Arc<ConcurrencyController>,
    /// Buffer cap forwarded to `buffer_unordered`.
    concurrent: usize,
    /// Default per-document deadline (spec 005 §R2). Stamped into an
    /// `Instant` after each document acquires its concurrency permit
    /// — so a slow earlier document does not consume budget allotted
    /// to a later one, and `ConcurrencyController` wait time does
    /// not count against the engine's slice. `None` means no
    /// deadline; the construction-time default flows through
    /// `lint_many` / `fix_many`. Per-call `_with_options` variants
    /// can override.
    per_doc_deadline: Option<Duration>,
}

impl BatchEngine {
    pub fn new(engine: CapcoEngine, options: BatchOptions) -> Self {
        let concurrent = options.max_concurrent_docs.unwrap_or(32);
        let controller = ConcurrencyController::new(&ConcurOptions {
            max_inflight_rows: options.max_concurrent_docs,
            max_inflight_bytes: options.max_inflight_bytes,
        });
        Self {
            engine: Arc::new(engine),
            controller: Arc::new(controller),
            concurrent,
            per_doc_deadline: options.per_doc_deadline,
        }
    }

    /// Lint many documents concurrently. Yields `(id, Result)` in
    /// completion order; an `Err` indicates the per-document task
    /// panicked, was cancelled, or could not start because shutdown
    /// is in progress (the `ConcurrencyController` semaphore was
    /// closed) — it does not abort the batch.
    ///
    /// Honors `BatchOptions::per_doc_deadline` from construction time
    /// (spec 005 §R2). A deadline-truncated lint surfaces as
    /// `Ok(LintResult { truncated: true, .. })` — the partial
    /// diagnostics are useful, so they flow through the success
    /// channel rather than `Err`.
    pub fn lint_many(
        &self,
        docs: impl IntoIterator<Item = (String, Vec<u8>)>,
    ) -> impl Stream<Item = (String, Result<LintResult, BatchError>)> {
        self.lint_many_inner(docs, self.per_doc_deadline)
    }

    /// Same as [`lint_many`] but reads `per_doc_deadline` from the
    /// supplied [`BatchOptions`] instead of the construction-time
    /// default. Other fields on `opts` are reserved for future
    /// per-call overrides; in MVP only `per_doc_deadline` is honored.
    ///
    /// [`lint_many`]: BatchEngine::lint_many
    pub fn lint_many_with_options(
        &self,
        docs: impl IntoIterator<Item = (String, Vec<u8>)>,
        opts: &BatchOptions,
    ) -> impl Stream<Item = (String, Result<LintResult, BatchError>)> {
        self.lint_many_inner(docs, opts.per_doc_deadline)
    }

    fn lint_many_inner(
        &self,
        docs: impl IntoIterator<Item = (String, Vec<u8>)>,
        per_doc_deadline: Option<Duration>,
    ) -> impl Stream<Item = (String, Result<LintResult, BatchError>)> {
        let engine = Arc::clone(&self.engine);
        let controller = Arc::clone(&self.controller);
        let concurrent = self.concurrent;

        stream::iter(docs)
            .map(move |(id, data)| {
                let engine = Arc::clone(&engine);
                let controller = Arc::clone(&controller);
                async move {
                    let byte_len = data.len();
                    // Whitepaper §9.4 / gap register #8: surface a closed
                    // controller as `BatchError::ShutdownInProgress` rather
                    // than `.expect()`-panicking. The `From<AcquireError>`
                    // impl above maps the only possible error.
                    let _permit = match controller.acquire(Some(|| byte_len)).await {
                        Ok(p) => p,
                        Err(e) => return (id, Err(BatchError::from(e))),
                    };
                    // Spec 005 §R2: the deadline is stamped AFTER permit
                    // acquisition so slow `ConcurrencyController` waits
                    // (a backed-up batch) don't consume the document's
                    // engine budget.
                    let result = tokio::task::spawn_blocking(move || {
                        // `checked_add` overflow must not silently drop
                        // the deadline (which would let an unbounded
                        // pass run after the operator explicitly
                        // configured a budget). Treat overflow as
                        // `deadline = now`, which the engine's pre-pass
                        // check (`now >= deadline`) treats as expired
                        // and aborts on entry.
                        let deadline = per_doc_deadline.map(|d| {
                            let now = Instant::now();
                            now.checked_add(d).unwrap_or(now)
                        });
                        // In-crate construction may use struct-update
                        // syntax across `#[non_exhaustive]` — only the
                        // outside-the-defining-crate boundary is restricted.
                        let opts = LintOptions {
                            deadline,
                            ..LintOptions::default()
                        };
                        engine.lint_with_options(&data, &opts)
                    })
                    .await
                    .map_err(BatchError::from);
                    (id, result)
                }
            })
            .buffer_unordered(concurrent)
    }

    /// Fix many documents concurrently. Yields `(id, Result)` in
    /// completion order; an `Err` indicates the per-document task
    /// panicked, was cancelled, hit the per-document deadline, or the
    /// runtime is shutting down — it does not abort the batch.
    ///
    /// Honors `BatchOptions::per_doc_deadline` from construction
    /// time. A deadline trip on the fix path returns
    /// `Err(BatchError::DocumentDeadlineExceeded { partial_lint })`
    /// per Constitution V Principle V — no partial `FixResult` is
    /// ever produced. Match on `is_deadline_exceeded()` to
    /// distinguish from worker bugs (`is_panic()`) or shutdown
    /// (`is_shutdown()`).
    pub fn fix_many(
        &self,
        docs: impl IntoIterator<Item = (String, Vec<u8>)>,
    ) -> impl Stream<Item = (String, Result<FixResult, BatchError>)> {
        self.fix_many_inner(docs, self.per_doc_deadline)
    }

    /// Same as [`fix_many`] but reads `per_doc_deadline` from the
    /// supplied [`BatchOptions`] instead of the construction-time
    /// default. Other fields on `opts` are reserved for future
    /// per-call overrides; in MVP only `per_doc_deadline` is honored.
    ///
    /// [`fix_many`]: BatchEngine::fix_many
    pub fn fix_many_with_options(
        &self,
        docs: impl IntoIterator<Item = (String, Vec<u8>)>,
        opts: &BatchOptions,
    ) -> impl Stream<Item = (String, Result<FixResult, BatchError>)> {
        self.fix_many_inner(docs, opts.per_doc_deadline)
    }

    fn fix_many_inner(
        &self,
        docs: impl IntoIterator<Item = (String, Vec<u8>)>,
        per_doc_deadline: Option<Duration>,
    ) -> impl Stream<Item = (String, Result<FixResult, BatchError>)> {
        let engine = Arc::clone(&self.engine);
        let controller = Arc::clone(&self.controller);
        let concurrent = self.concurrent;

        stream::iter(docs)
            .map(move |(id, data)| {
                let engine = Arc::clone(&engine);
                let controller = Arc::clone(&controller);
                async move {
                    let byte_len = data.len();
                    let _permit = match controller.acquire(Some(|| byte_len)).await {
                        Ok(p) => p,
                        Err(e) => return (id, Err(BatchError::from(e))),
                    };
                    // Spec 005 §R2 — same per-permit stamping as
                    // `lint_many`. Spec 005 §R4: a deadline trip on the
                    // fix path returns `Err(EngineError::DeadlineExceeded
                    // { partial_lint })`, which we re-shape into
                    // `BatchError::DocumentDeadlineExceeded` so callers
                    // matching on `BatchError` see the deadline-trip
                    // signal at the same level as panic / shutdown.
                    let result = tokio::task::spawn_blocking(move || {
                        // Same overflow semantics as `lint_many_inner` —
                        // overflow folds to `deadline = now` (which the
                        // engine treats as already expired) so the
                        // operator-configured deadline is never silently
                        // disabled.
                        let deadline = per_doc_deadline.map(|d| {
                            let now = Instant::now();
                            now.checked_add(d).unwrap_or(now)
                        });
                        let opts = FixOptions {
                            deadline,
                            ..FixOptions::default()
                        };
                        engine.fix_with_options(&data, crate::FixMode::Apply, &opts)
                    })
                    .await;
                    let mapped = match result {
                        Ok(Ok(fix_result)) => Ok(fix_result),
                        Ok(Err(EngineError::DeadlineExceeded { partial_lint })) => {
                            Err(BatchError::DocumentDeadlineExceeded { partial_lint })
                        }
                        // `EngineError::InvalidThreshold` cannot fire here
                        // because `FixOptions` carries no `threshold_override`
                        // (default is `None`, falling back to the engine's
                        // pre-validated config threshold). A future addition
                        // of per-doc threshold overrides on `BatchOptions`
                        // would need to thread `EngineError::InvalidThreshold`
                        // into a new `BatchError` variant; until then the
                        // arm is `unreachable!` so a silent breakage is
                        // visible at the next test run.
                        Ok(Err(EngineError::InvalidThreshold(_))) => unreachable!(
                            "BatchEngine does not set FixOptions::threshold_override; \
                             InvalidThreshold cannot fire"
                        ),
                        // `BatchEngine` supplies no per-document
                        // signature, so under a `require_signature`
                        // deployment every document trips the gate. Map
                        // it to the dedicated BatchError variant rather
                        // than dropping the document silently.
                        Ok(Err(EngineError::SignatureRequired)) => {
                            Err(BatchError::SignatureRequired)
                        }
                        // `EngineError` is `#[non_exhaustive]` for crate
                        // outsiders, but inside `marque-engine` we see all
                        // variants — adding a future variant will produce
                        // a non-exhaustive-match error here, forcing an
                        // explicit `BatchError` mapping decision rather
                        // than a silently-eaten wildcard.
                        Err(join_error) => Err(BatchError::from(join_error)),
                    };
                    (id, mapped)
                }
            })
            .buffer_unordered(concurrent)
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn shutdown_error_is_not_panic_or_cancellation() {
        let e = BatchError::ShutdownInProgress;
        assert!(!e.is_panic());
        assert!(!e.is_cancelled());
        assert!(e.is_shutdown());
    }

    #[test]
    fn signature_required_is_a_policy_condition_not_a_runtime_failure() {
        // issue #399: BatchEngine carries no per-document signature, so
        // under a require_signature deployment each document maps to
        // this variant. It is a configuration/policy condition — none
        // of the runtime-failure classifiers should claim it.
        let e = BatchError::SignatureRequired;
        assert!(!e.is_panic());
        assert!(!e.is_cancelled());
        assert!(!e.is_shutdown());
        assert!(!e.is_deadline_exceeded());
        let s = e.to_string();
        assert!(
            s.contains("signature"),
            "Display should mention the missing signature, got: {s}"
        );
        assert!(std::error::Error::source(&e).is_none());
    }

    #[test]
    fn shutdown_error_display_names_the_state() {
        let e = BatchError::ShutdownInProgress;
        let s = e.to_string();
        // The Display string must convey "shutdown" cleanly so a log
        // grep on operator dashboards picks it up. We don't assert
        // exact wording — only the discriminating substrings.
        assert!(
            s.contains("shutdown"),
            "ShutdownInProgress Display should name the state explicitly: got {s:?}"
        );
        assert!(
            s.contains("closed"),
            "Display should name the underlying signal (semaphore closed): got {s:?}"
        );
    }

    #[test]
    fn shutdown_error_has_no_source() {
        // Whitepaper §9.4: `ShutdownInProgress` is a terminal signal,
        // not a wrapped error. Anything pretending to be a `source()`
        // here would be misleading — the underlying `AcquireError`
        // carries no information beyond "closed".
        let e = BatchError::ShutdownInProgress;
        assert!(
            std::error::Error::source(&e).is_none(),
            "ShutdownInProgress must not chain to a source"
        );
    }

    #[test]
    fn from_acquire_error_yields_shutdown_variant() {
        // Drive the conversion path the runtime uses. Closing a
        // semaphore and acquiring against it produces `AcquireError`,
        // which `BatchError::from` must convert to `ShutdownInProgress`.
        let sem = tokio::sync::Semaphore::new(1);
        sem.close();
        // `try_acquire` on a closed semaphore returns
        // `TryAcquireError::Closed`, not `AcquireError`. The async
        // `acquire().await` returns `AcquireError`. Run a tiny
        // single-thread runtime to drive the right path.
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("current_thread runtime builds");
        let acquire_err = rt.block_on(async { sem.acquire().await }).unwrap_err();
        let batch_err: BatchError = acquire_err.into();
        assert!(batch_err.is_shutdown());
        assert!(!batch_err.is_panic());
    }
}
