// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![cfg(feature = "count-allocs")]

//! Zero-allocation regression gate for `Vocabulary<CapcoScheme>`
//! accessors (Phase 5 PR-2, task T077).
//!
//! The Vocabulary trait contract is "every accessor returns
//! `&'static` data — no runtime allocation" (FR-016 +
//! `marque-scheme::vocabulary` invariants). The metadata table is
//! heap-initialized once via `LazyLock` (a single allocation
//! outside the measurement window); subsequent lookups dereference
//! the boxed data directly and must not allocate.
//!
//! ## Why this is a separate integration file
//!
//! `count-allocs` installs a process-wide `#[global_allocator]`
//! (the counting allocator), so it counts allocations from every
//! test in the same binary. Sharing a binary with the rest of
//! `tests/vocabulary.rs` would mean those tests' allocations land
//! inside this test's measurement window when the runner schedules
//! them concurrently — even with a `MEASURE_LOCK` guard, the
//! noise floor is unstable.
//!
//! Splitting the count-allocs test into its own integration file
//! makes it the only test in the binary, which removes the shared-
//! counter contention entirely. Mirrors the file-level
//! `#![cfg(feature = "count-allocs")]` discipline used by
//! `crates/core/tests/alloc_budget.rs` (gap register #15).
//!
//! ## How to run
//!
//! ```text
//! cargo test -p marque-capco --features count-allocs \
//!     --test vocabulary_zero_alloc
//! ```
//!
//! Because the file is gated, the default `cargo test` run (and
//! every other `--test` invocation) compiles it out completely —
//! no global allocator side effect.

use marque_capco::CapcoScheme;
use marque_capco::scheme::{
    TOK_CNWDI, TOK_EXDIS, TOK_FRD, TOK_HCS, TOK_NODIS, TOK_NOFORN, TOK_RD, TOK_RESTRICTED,
    TOK_TFNI, TOK_UCNI,
};
use marque_scheme::{TokenId, Vocabulary};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: forwarded `layout` matches the caller's; we hand
        // back the System allocator's pointer verbatim. Same
        // contract as `crates/core/tests/alloc_budget.rs`.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarded; `ptr` came from our `alloc`.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: forwarded.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

fn allocs_now() -> usize {
    ALLOCATIONS.load(Ordering::Relaxed)
}

/// All sentinels mapped to canonical CVE values in
/// `crates/capco/src/vocabulary.rs::SENTINEL_TO_CANONICAL`. Kept in
/// sync with the production list — adding a sentinel there means
/// adding it here, otherwise the gate stops covering the new entry.
fn active_sentinels() -> &'static [TokenId] {
    &[
        TOK_NOFORN,
        TOK_RD,
        TOK_FRD,
        TOK_TFNI,
        TOK_CNWDI,
        TOK_UCNI,
        TOK_HCS,
        TOK_RESTRICTED,
        TOK_NODIS,
        TOK_EXDIS,
    ]
}

#[test]
fn metadata_query_is_zero_alloc() {
    let scheme = CapcoScheme::new();
    // Force one-time initialization of every `LazyLock`-backed
    // table (CVE_FILE_DERIVED + TOKEN_DERIVED) outside the
    // measurement window. The first access allocates the boxed
    // records; subsequent accesses must not.
    for token in active_sentinels() {
        let _warmup_meta = scheme.metadata(token);
        let _warmup_auth = scheme.authority(token);
        let _warmup_owner = scheme.owner_producer(token);
        let _warmup_poc = scheme.point_of_contact(token);
    }

    let before = allocs_now();
    for token in active_sentinels() {
        let m = scheme.metadata(token);
        std::hint::black_box(m);
        let a = scheme.authority(token);
        std::hint::black_box(a);
        let o = scheme.owner_producer(token);
        std::hint::black_box(o);
        let p = scheme.point_of_contact(token);
        std::hint::black_box(p);
        let pf = scheme.portion_form(token);
        std::hint::black_box(pf);
        let bf = scheme.banner_form(token);
        std::hint::black_box(bf);
        let ba = scheme.banner_abbreviation(token);
        std::hint::black_box(ba);
        let d = scheme.deprecation(token);
        std::hint::black_box(d);
    }
    let allocs = allocs_now() - before;

    assert_eq!(
        allocs, 0,
        "Vocabulary accessors allocated {allocs} time(s) after warmup; \
         expected 0. Every accessor must return `&'static` data \
         (FR-016 / `marque-scheme::vocabulary` contract).",
    );
}
