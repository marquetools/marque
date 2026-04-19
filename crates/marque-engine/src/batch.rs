// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Concurrent batch processing over many documents.
//!
//! `BatchEngine` wraps `Engine` behind an `Arc` and uses `ConcurrencyController`
//! from `recoco-utils` to enforce row and byte limits on in-flight work.
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
//! use marque_engine::{Engine, batch::{BatchEngine, BatchOptions}};
//! use futures::StreamExt;
//!
//! # async fn example(engine: Engine) {
//! let batch = BatchEngine::new(engine, BatchOptions {
//!     max_concurrent_docs: Some(16),
//!     max_inflight_bytes: Some(256 * 1024 * 1024), // 256 MiB
//! });
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

use futures::{Stream, StreamExt, stream};
use recoco_utils::concur_control::{ConcurrencyController, Options as ConcurOptions};

use crate::{Engine, FixResult, LintResult};

/// Error returned when a single document in a batch fails to process.
///
/// Batch APIs surface this per-document so a panic or cancellation in one
/// document does not abort the entire batch run.
#[derive(Debug)]
pub enum BatchError {
    /// The blocking lint/fix task panicked or was cancelled.
    TaskFailed(tokio::task::JoinError),
}

impl BatchError {
    /// Returns `true` if the error was caused by a panic in the worker task.
    ///
    /// CI pipelines and supervisors should treat this as an application bug
    /// that warrants investigation (not a transient infrastructure issue).
    pub fn is_panic(&self) -> bool {
        match self {
            Self::TaskFailed(e) => e.is_panic(),
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
        }
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
        }
    }
}

impl std::error::Error for BatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TaskFailed(e) => Some(e),
        }
    }
}

impl From<tokio::task::JoinError> for BatchError {
    fn from(e: tokio::task::JoinError) -> Self {
        Self::TaskFailed(e)
    }
}

/// Concurrency limits for batch processing.
///
/// Both limits are optional and independent. When both are set the more
/// restrictive one governs at any given moment.
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
}

impl Default for BatchOptions {
    fn default() -> Self {
        Self {
            max_concurrent_docs: Some(32),
            max_inflight_bytes: None,
        }
    }
}

/// Wraps `Engine` for concurrent multi-document processing with backpressure.
///
/// The underlying `Engine` is shared via `Arc`; cloning `BatchEngine` is cheap.
pub struct BatchEngine {
    engine: Arc<Engine>,
    controller: Arc<ConcurrencyController>,
    /// Buffer cap forwarded to `buffer_unordered`.
    concurrent: usize,
}

impl BatchEngine {
    pub fn new(engine: Engine, options: BatchOptions) -> Self {
        let concurrent = options.max_concurrent_docs.unwrap_or(32);
        let controller = ConcurrencyController::new(&ConcurOptions {
            max_inflight_rows: options.max_concurrent_docs,
            max_inflight_bytes: options.max_inflight_bytes,
        });
        Self {
            engine: Arc::new(engine),
            controller: Arc::new(controller),
            concurrent,
        }
    }

    /// Lint many documents concurrently. Yields `(id, Result)` in
    /// completion order; an `Err` indicates the per-document task panicked
    /// or was cancelled — it does not abort the batch.
    pub fn lint_many(
        &self,
        docs: impl IntoIterator<Item = (String, Vec<u8>)>,
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
                    let _permit = controller
                        .acquire(Some(|| byte_len))
                        .await
                        .expect("ConcurrencyController semaphore unexpectedly closed");
                    let result = tokio::task::spawn_blocking(move || engine.lint(&data))
                        .await
                        .map_err(BatchError::from);
                    (id, result)
                }
            })
            .buffer_unordered(concurrent)
    }

    /// Fix many documents concurrently. Yields `(id, Result)` in
    /// completion order; an `Err` indicates the per-document task panicked
    /// or was cancelled — it does not abort the batch.
    pub fn fix_many(
        &self,
        docs: impl IntoIterator<Item = (String, Vec<u8>)>,
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
                    let _permit = controller
                        .acquire(Some(|| byte_len))
                        .await
                        .expect("ConcurrencyController semaphore unexpectedly closed");
                    let result = tokio::task::spawn_blocking(move || {
                        engine.fix(&data, crate::FixMode::Apply)
                    })
                    .await
                    .map_err(BatchError::from);
                    (id, result)
                }
            })
            .buffer_unordered(concurrent)
    }
}
