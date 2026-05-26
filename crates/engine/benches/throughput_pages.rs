// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Full-machine "pages per minute" throughput harness.
//!
//! Unlike the per-document latency benches (`lint_latency`, `fix_10kb`),
//! which use Criterion to measure single-call latency, this is a custom
//! `harness = false` bench: it drives [`BatchEngine`] across every core on
//! the host, drains the completion-order result stream, and reports
//! sustained throughput as **pages per minute** for both the lint path and
//! the full two-pass fix path. The base document is the shared
//! `SINGLE_PAGE` / `SINGLE_PAGE_TO_FIX` ~3 KB classified memo from
//! `marque_test_utils::fixtures` — the same fixture the single-page latency
//! benches use, so the throughput number is the batch-amortized companion
//! to those latency numbers.
//!
//! Output: machine-parseable `bench-check[pages_per_minute]:` lines (the
//! same convention `report_fix_latency` uses in `scripts/bench-check.sh`),
//! consumed by `.github/workflows/throughput-weekly.yml`. The total figure
//! is hardware-scaled (it depends on core count); the per-core figure is
//! the host-independent companion.
//!
//! # Not a regression gate
//!
//! Advisory only — never wired into `scripts/bench-check.sh`, never fails a
//! build. It records a marketing/reference time series. Throughput is
//! sensitive to runner core count and contention, so a single capture is a
//! point estimate, not a gate.
//!
//! # Known limitation (#807)
//!
//! Per-core throughput on tiny (single-page, ~3 KB) documents is well below
//! `60s / single-page-latency`: `BatchEngine`'s per-document `spawn_blocking`
//! + semaphore coordination is large relative to a ~60 µs lint. By ~10 KB the
//! overhead amortizes (<5%). Investigating coalescing / size-threshold
//! dispatch for the small-doc path is tracked in #807; this bench is the
//! vehicle for the doc-size crossover sweep called for there.
//!
//! # Tuning
//!
//! `MARQUE_THROUGHPUT_PAGES` overrides the batch size (default 20,000
//! pages per phase). Larger batches amortize thread-pool spin-up and
//! allocator warmup at the cost of a longer run and more memory
//! (~`pages × 3 KB` per phase).

use std::time::Instant;

use futures::StreamExt;
use marque_config::Config;
use marque_engine::{BatchEngine, BatchOptions, Engine};
use marque_test_utils::fixtures::{SINGLE_PAGE, SINGLE_PAGE_TO_FIX};

/// Default pages processed per phase. Large enough to amortize tokio
/// blocking-pool spin-up and allocator warmup into a steady-state number,
/// small enough to keep the weekly run to a few seconds and memory bounded
/// (~`PAGES × 3 KB`). Overridable via `MARQUE_THROUGHPUT_PAGES`.
const DEFAULT_BATCH_PAGES: usize = 20_000;

/// Warmup pages discarded before the timed run, so the measured window
/// reflects steady-state throughput rather than first-touch costs (blocking
/// threads spawned lazily, pages first faulted in, caches cold).
const WARMUP_PAGES: usize = 2_000;

fn build_engine() -> Engine {
    // The `SERCET → SECRET` correction lets the fix phase exercise the full
    // two-pass pipeline on `SINGLE_PAGE_TO_FIX` (matching the `fix_single_page`
    // latency bench). The lint phase runs `SINGLE_PAGE`, which contains no
    // `SERCET`, so the correction is inert there — one engine serves both.
    let mut config = Config::default();
    config.corrections.insert("SERCET".into(), "SECRET".into());
    Engine::new(
        config,
        marque_engine::default_ruleset(),
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

/// Build a fresh batch of `count` `(id, bytes)` pairs from `fixture`.
fn build_docs(fixture: &str, count: usize) -> Vec<(String, Vec<u8>)> {
    let bytes = fixture.as_bytes();
    (0..count)
        .map(|i| (format!("p{i}"), bytes.to_vec()))
        .collect()
}

/// Drain a lint batch, panicking on any per-document error, and return the
/// number of documents processed.
async fn drain_lint(batch: &BatchEngine, docs: Vec<(String, Vec<u8>)>) -> usize {
    let mut stream = batch.lint_many(docs);
    let mut n = 0usize;
    while let Some((id, result)) = stream.next().await {
        result.unwrap_or_else(|e| panic!("lint_many failed for {id}: {e}"));
        n += 1;
    }
    n
}

/// Drain a fix batch, panicking on any per-document error, and return the
/// number of documents processed.
async fn drain_fix(batch: &BatchEngine, docs: Vec<(String, Vec<u8>)>) -> usize {
    let mut stream = batch.fix_many(docs);
    let mut n = 0usize;
    while let Some((id, result)) = stream.next().await {
        result.unwrap_or_else(|e| panic!("fix_many failed for {id}: {e}"));
        n += 1;
    }
    n
}

/// Pages per minute, rounded to a whole page.
fn pages_per_minute(pages: usize, elapsed_secs: f64) -> u64 {
    if elapsed_secs <= 0.0 {
        return 0;
    }
    ((pages as f64 / elapsed_secs) * 60.0).round() as u64
}

fn main() {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let batch_pages = std::env::var("MARQUE_THROUGHPUT_PAGES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_BATCH_PAGES);

    // Multi-thread runtime so the async coordination layer is not itself a
    // bottleneck; the CPU-bound lint/fix work runs on tokio's blocking pool.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("multi-thread tokio runtime builds");

    // Allow enough documents in-flight to keep every core busy: the work is
    // dispatched to `spawn_blocking`, so concurrency must exceed the core
    // count for the OS scheduler to saturate all cores. `cores * 4` is a
    // generous, bounded over-subscription.
    let mut options = BatchOptions::default();
    options.max_concurrent_docs = Some(cores.saturating_mul(4).max(4));
    let batch = BatchEngine::new(build_engine(), options);

    // --- Warmup (discarded) ---
    rt.block_on(async {
        drain_lint(&batch, build_docs(SINGLE_PAGE, WARMUP_PAGES)).await;
        drain_fix(&batch, build_docs(SINGLE_PAGE_TO_FIX, WARMUP_PAGES)).await;
    });

    // --- Lint throughput (timed) ---
    let lint_docs = build_docs(SINGLE_PAGE, batch_pages);
    let lint_ppm = rt.block_on(async {
        let t = Instant::now();
        let n = drain_lint(&batch, lint_docs).await;
        let elapsed = t.elapsed().as_secs_f64();
        assert_eq!(n, batch_pages, "lint batch dropped documents");
        pages_per_minute(n, elapsed)
    });

    // --- Fix throughput (timed, full two-pass) ---
    let fix_docs = build_docs(SINGLE_PAGE_TO_FIX, batch_pages);
    let fix_ppm = rt.block_on(async {
        let t = Instant::now();
        let n = drain_fix(&batch, fix_docs).await;
        let elapsed = t.elapsed().as_secs_f64();
        assert_eq!(n, batch_pages, "fix batch dropped documents");
        pages_per_minute(n, elapsed)
    });

    let page_bytes = SINGLE_PAGE.len();
    let lint_per_core = lint_ppm / cores as u64;
    let fix_per_core = fix_ppm / cores as u64;

    // Machine-parseable lines (parsed by throughput-weekly.yml). Keep the
    // `bench-check[pages_per_minute]:` prefix stable.
    println!("bench-check[pages_per_minute]: cores: {cores}");
    println!("bench-check[pages_per_minute]: page_bytes: {page_bytes}");
    println!("bench-check[pages_per_minute]: batch_pages: {batch_pages}");
    println!(
        "bench-check[pages_per_minute]: lint: {lint_ppm} pages/min total; \
         {lint_per_core} pages/min/core"
    );
    println!(
        "bench-check[pages_per_minute]: fix: {fix_ppm} pages/min total; \
         {fix_per_core} pages/min/core"
    );
}
