// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![cfg(feature = "count-allocs")]

//! Hot-path heap-allocation regression gate (whitepaper §3.2 / Constitution II,
//! gap register #15).
//!
//! Installs a counting global allocator and asserts that `Scanner::scan(...)`
//! does not exceed a small allocation budget on a representative corpus
//! sweep. The Constitution invariant is "zero heap allocations *per candidate
//! span detected*" — the scanner is allowed to allocate the result `Vec`
//! itself (which grows logarithmically as candidates accumulate), but it is
//! NOT allowed to allocate per-character, per-byte, or per-candidate beyond
//! that single buffer.
//!
//! ## Why this is feature-gated
//!
//! Installing a `#[global_allocator]` is intrusive: it overrides the system
//! allocator for the entire test binary, including the test framework's own
//! allocations. We don't want that active in the default `cargo test` run —
//! it would pollute every test's runtime characteristics and complicate
//! debug-output capture. Instead the file is gated behind the
//! `count-allocs` feature, exercised only by the dedicated CI job that
//! invokes
//!
//! ```text
//! cargo test -p marque-core --features count-allocs --test alloc_budget \
//!     -- --test-threads=1
//! ```
//!
//! ## Why `--test-threads=1` is mandatory
//!
//! `ALLOCATIONS` is a process-wide atomic counter. Two tests running in
//! parallel see each other's allocations in their delta windows — Test A's
//! `let x = vec![..]` lands inside Test B's measurement, and Test B's
//! assertion-formatting machinery lands inside Test A's. The on-test
//! `MEASURE_LOCK` mutex narrows the contention surface but cannot eliminate
//! it: the test runner's own per-thread setup / teardown (panic handler
//! installation, stdout capture buffer, name-string interning) allocates
//! between releasing the lock at the end of one test and acquiring it at
//! the start of another. Empirically the parallel-execution noise is on
//! the order of 5-25 allocations per concurrent test pair, easily large
//! enough to blow past every budget below.
//!
//! `--test-threads=1` removes the noise floor entirely. If a contributor
//! runs the gate without it the failures will still be informative — the
//! test names point at the single regression site — but the budgets
//! were calibrated under serial execution and are tight.
//!
//! ## Why a counting allocator and not `dhat` / `allocation-counter`
//!
//! Dependencies have a real cost on a WASM-safe crate (Constitution III,
//! Tech Stack pinning). A 30-line counting allocator wired against
//! `std::alloc::System` discharges the gap without adding a dev-dep that
//! would have to be license-audited and tracked across releases. If the
//! gate evolves into a profiler-style harness (hot-path bytes-per-call,
//! peak resident set, etc.) the cost calculus flips and pulling in a
//! dedicated crate becomes the right move. Today the answer is "≤ K
//! allocations per scan", which a counter handles.

use marque_core::Scanner;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

// =====================================================================
// Counting global allocator.
//
// Wraps `std::alloc::System` and increments an atomic on every alloc /
// realloc. The counter is process-global; tests sample it before and
// after a measured operation and compare the delta to a budget.
//
// `Ordering::Relaxed` is sufficient — we don't read the counter from
// inside the allocator hooks, only from outside the measured window.
// Total-store-order isn't needed; we only need atomicity per increment.
// =====================================================================

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: We forward the caller's `layout` unchanged. The System
        // allocator's preconditions (non-zero size, valid alignment) are
        // the same as ours, so if the caller satisfied ours it satisfies
        // System's. We do not read the returned pointer; we hand it back
        // to the caller verbatim.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from our `alloc` (same allocator instance,
        // same layout) per the GlobalAlloc contract. We forward both
        // unchanged.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: Same forwarding contract as `alloc` — caller-supplied
        // pointer + layout are preserved, returned pointer is handed
        // back verbatim.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

/// Snapshot the current allocation count.
fn allocs_now() -> usize {
    ALLOCATIONS.load(Ordering::Relaxed)
}

/// Serialize concurrent measurements. The global allocator counter is
/// process-wide, so two tests running in parallel inflate each other's
/// deltas (one test's `vec![..]` lands inside another's count window).
/// `cargo test` defaults to `--test-threads = N` where N is the CPU
/// count — without this lock the harness would only be reliable under
/// `--test-threads=1`, which would silently break the CI gate the
/// moment someone forgot to pass it.
///
/// `MeasureLock` is poison-aware: if a test panics inside the
/// measurement window, the next test's `lock()` returns `Err`. The
/// helper recovers via `into_inner` and the next test runs cleanly.
static MEASURE_LOCK: Mutex<()> = Mutex::new(());

/// Run `body`, return the number of allocations that occurred during it.
///
/// Closures themselves don't allocate (closure captures live on the
/// stack), so the delta is exactly the work `body` did. Callers must
/// avoid implicit allocations inside the closure (e.g., `format!`,
/// `String::from`, `vec![..]`) — anything they materialize counts.
fn count_allocs<F: FnOnce()>(body: F) -> usize {
    // Acquire the serialization lock BEFORE sampling `before`, so
    // another test's allocations (made before it released the lock)
    // are flushed by then.
    let _guard = MEASURE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let before = allocs_now();
    body();
    allocs_now() - before
}

// =====================================================================
// Budgets.
//
// The scanner's only legitimate allocation is the result `Vec<MarkingCandidate>`
// growing as candidates accumulate. A `Vec` that pushes N items goes
// through ceil(log2(N + initial_capacity)) - log2(initial_capacity)
// reallocations. The standard library's growth factor is 2× and the
// initial allocation is 4 elements; for 0 ≤ N ≤ 4 we expect 1 alloc, for
// 5 ≤ N ≤ 8 we expect 2, and so on.
//
// The scanner runs four sub-scans (`scan_portions`, `scan_banners`,
// `scan_cab`, `scan_page_breaks`) plus a `sort_unstable_by` pass. None
// of those allocate, so the candidate-Vec growth is the entire budget.
//
// Budgets here are deliberately above the theoretical minimum — we want
// the gate to fire on per-byte / per-candidate allocations (a real
// regression), not on a +1 alloc from a stdlib growth-factor tweak.
// =====================================================================

/// Empty input: zero candidates, zero pushes → zero allocations.
const BUDGET_EMPTY: usize = 0;

/// A single banner-shaped marking: 1 candidate. The Vec's first push
/// triggers the initial allocation; nothing else is allocated.
const BUDGET_SINGLE_MARKING: usize = 2;

/// Up to ~32 candidates: at most 4 Vec growths (0 → 4 → 8 → 16 → 32).
/// The "32" comes from the realistic ceiling on a single-page document
/// with portions and a banner; corpus inspection shows no fixture
/// exceeding ~12 candidates.
const BUDGET_PER_PAGE: usize = 6;

// =====================================================================
// Tests.
// =====================================================================

/// Run a scan once before the measurement window so any first-call
/// initialization (lazy SIMD feature detection inside `memchr`,
/// per-thread output-capture buffer setup inside the test runner,
/// global string-interner setup inside `assert!`-related machinery,
/// etc.) is amortized away. Without this, the first measured scan
/// in the binary picks up a one-time fixed cost that has nothing
/// to do with the scanner itself.
fn warm_up() {
    let _ = Scanner::scan(b"TOP SECRET//SI//NOFORN\n");
    let _ = Scanner::scan(b"(S//NF) sample portion.");
    let _ = Scanner::scan(b"");
}

#[test]
fn scanner_zero_alloc_on_empty_input() {
    warm_up();

    // Tight scope inside `count_allocs`: only the call we're measuring,
    // no fixture loading or formatting helpers. Empty input has no
    // candidates → no Vec push → no allocation.
    let allocs = count_allocs(|| {
        let result = Scanner::scan(b"");
        // Ensure the result is observed so the optimizer doesn't
        // elide the call in release mode.
        std::hint::black_box(result);
    });

    assert_eq!(
        allocs, BUDGET_EMPTY,
        "Scanner::scan(empty) allocated {allocs} time(s); expected exactly \
         {BUDGET_EMPTY}. Constitution II requires zero per-candidate \
         allocation on the hot path; an empty input has no candidates and \
         must therefore allocate nothing."
    );
}

#[test]
fn scanner_single_banner_within_budget() {
    warm_up();
    let input: &[u8] = b"TOP SECRET//SI//NOFORN\n";

    let allocs = count_allocs(|| {
        let result = Scanner::scan(input);
        std::hint::black_box(result);
    });

    assert!(
        allocs <= BUDGET_SINGLE_MARKING,
        "Scanner::scan(single-banner) allocated {allocs} time(s); \
         budget {BUDGET_SINGLE_MARKING}. The candidate-Vec's initial \
         allocation accounts for at most one of these. A regression \
         that allocates per-byte or per-token would push this far over."
    );
}

#[test]
fn scanner_multi_marking_document_within_budget() {
    warm_up();
    // A document with multiple portions + a banner + a CAB. The
    // candidate count is finite (about 6 here); the test asserts
    // the allocation count stays within the per-page budget no
    // matter how the buffer interior changes — the only thing
    // that should drive allocs is the candidate-Vec growth.
    let input: &[u8] = b"\
TOP SECRET//SI//NOFORN

(TS//SI//NF) The quick brown fox jumps over the lazy dog.
(S) Subsequent paragraph at a lower level.
(C) And another.
(U) Public sentence.

Classified By: 12345
Derived From: source-doc-9876
Declassify On: 20420101

(TS//SI//NF) Final classified portion.
TOP SECRET//SI//NOFORN
";

    let allocs = count_allocs(|| {
        let result = Scanner::scan(input);
        std::hint::black_box(result);
    });

    assert!(
        allocs <= BUDGET_PER_PAGE,
        "Scanner::scan(multi-marking) allocated {allocs} time(s); \
         budget {BUDGET_PER_PAGE}. Allocations should grow with the \
         Vec's geometric resize series, not with the number of bytes \
         in the input. A regression that ran allocations per-line or \
         per-candidate would blow past the budget on this 13-line \
         fixture."
    );
}

#[test]
fn scanner_alloc_count_is_buffer_size_independent() {
    warm_up();

    // Scan two buffers with the SAME marking content but very
    // different sizes. The Constitution invariant ("zero allocations
    // per candidate span detected") permits a small fixed overhead
    // for the result-Vec growth, but explicitly rules out per-byte
    // / per-line / per-window allocation. The strict form of that
    // claim is "alloc count for a buffer with one banner is the same
    // whether the buffer is 23 bytes or 4 KB."
    //
    // We assert equality, not just "small": a regression that
    // introduced a per-line `String::from(line)` would scale linearly
    // with the line count and trip this even at 100-line documents.

    let small: &[u8] = b"TOP SECRET//SI//NOFORN\n";

    let mut large = vec![b' '; 4096];
    large.extend_from_slice(b"\nTOP SECRET//SI//NOFORN\n");

    let allocs_small = count_allocs(|| {
        let result = Scanner::scan(small);
        std::hint::black_box(result);
    });
    let allocs_large = count_allocs(|| {
        let result = Scanner::scan(&large);
        std::hint::black_box(result);
    });

    assert_eq!(
        allocs_small, allocs_large,
        "Scanner::scan allocation count must depend on candidate count, \
         not buffer size. small (23 B): {allocs_small} alloc(s); \
         large (4 KB): {allocs_large} alloc(s). The diff implies a \
         per-byte / per-line / per-window allocation in the scanner."
    );
}
