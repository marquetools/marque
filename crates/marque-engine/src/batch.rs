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
//!     println!("{id}: {} diagnostics", result.diagnostics.len());
//! }
//! # }
//! ```

use std::sync::Arc;

use futures::{Stream, StreamExt, stream};
use recoco_utils::concur_control::{ConcurrencyController, Options as ConcurOptions};

use crate::{Engine, FixResult, LintResult};

/// Concurrency limits for batch processing.
///
/// Both limits are optional and independent. When both are set the more
/// restrictive one governs at any given moment.
pub struct BatchOptions {
    /// Maximum documents in-flight simultaneously.
    ///
    /// Also used as the `buffer_unordered` cap, so this controls how many
    /// futures are created ahead of being polled. Defaults to 32.
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

    /// Lint many documents concurrently; yields `(id, LintResult)` in completion order.
    pub fn lint_many(
        &self,
        docs: impl IntoIterator<Item = (String, Vec<u8>)>,
    ) -> impl Stream<Item = (String, LintResult)> {
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
                        .expect("lint task panicked");
                    (id, result)
                }
            })
            .buffer_unordered(concurrent)
    }

    /// Fix many documents concurrently; yields `(id, FixResult)` in completion order.
    pub fn fix_many(
        &self,
        docs: impl IntoIterator<Item = (String, Vec<u8>)>,
    ) -> impl Stream<Item = (String, FixResult)> {
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
                    let result = tokio::task::spawn_blocking(move || engine.fix(&data))
                        .await
                        .expect("fix task panicked");
                    (id, result)
                }
            })
            .buffer_unordered(concurrent)
    }
}
