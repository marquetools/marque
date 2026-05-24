// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![cfg(feature = "count-allocs")]

//! Zero-allocation regression gate for `Vocabulary<CapcoScheme>`
//! accessors.
//!
//! The Vocabulary trait contract is "every accessor returns
//! `&'static` data — no runtime allocation" (per the
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
use marque_capco::active_sentinel_count;
use marque_capco::scheme::{
    TOK_ATOMAL, TOK_BALK, TOK_BOHEMIA, TOK_CNWDI, TOK_DCNI, TOK_EXDIS, TOK_FISA, TOK_FRD, TOK_HCS,
    TOK_HCS_O, TOK_HCS_P, TOK_NNPI, TOK_NODIS, TOK_NOFORN, TOK_ORCON_USGOV, TOK_RD, TOK_RESTRICTED,
    TOK_SI_G, TOK_SSI, TOK_TFNI, TOK_TK_BLFH, TOK_TK_IDIT, TOK_TK_KAND, TOK_UCNI,
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

/// Every sentinel mapped to a canonical CVE value in
/// `crates/capco/src/vocabulary.rs::SENTINEL_TO_CANONICAL`. The slice
/// MUST equal the full production set so the zero-alloc gate exercises
/// every accessor path; the
/// `active_sentinels_matches_active_sentinel_count` test below pins
/// `active_sentinels().len() == active_sentinel_count()` so a
/// production-side addition without a matching entry here fails the
/// gate immediately rather than weakening the regression coverage.
fn active_sentinels() -> &'static [TokenId] {
    &[
        // Dissem
        TOK_NOFORN,
        // AEA
        TOK_RD,
        TOK_FRD,
        TOK_TFNI,
        TOK_CNWDI,
        TOK_UCNI,
        // SCI — bare control + per-compartment compounds (#524 Phase 1).
        TOK_HCS,
        TOK_HCS_O,
        TOK_HCS_P,
        TOK_SI_G,
        TOK_TK_BLFH,
        TOK_TK_IDIT,
        TOK_TK_KAND,
        // Classification
        TOK_RESTRICTED,
        // Non-IC dissem
        TOK_NODIS,
        TOK_EXDIS,
        // #407 — IC dissem + non-IC dissem additions.
        TOK_ORCON_USGOV,
        TOK_FISA,
        TOK_SSI,
        TOK_NNPI,
        TOK_DCNI,
        // Issue #660 — NATO program markings (CVE canonical
        // `NATO-`-prefixed; `nato_program_form_set` projects to the
        // bare §G.1 Table 4 p37 display form). Static-slice projection
        // still returns `&'static` data, so the zero-alloc invariant
        // holds.
        TOK_ATOMAL,
        TOK_BALK,
        TOK_BOHEMIA,
    ]
}

/// Pin: `active_sentinels()` must enumerate every entry in
/// `SENTINEL_TO_CANONICAL`. Without this, a production-side addition
/// could land without a corresponding probe entry here, silently
/// shrinking the zero-alloc gate's coverage. The runtime check ties
/// the two sources to the same number — drift trips the test on the
/// next CI run.
#[test]
fn active_sentinels_matches_active_sentinel_count() {
    assert_eq!(
        active_sentinels().len(),
        active_sentinel_count(),
        "active_sentinels() length ({}) drifted from \
         SENTINEL_TO_CANONICAL length ({}). When you add a sentinel to \
         crates/capco/src/vocabulary.rs::SENTINEL_TO_CANONICAL you MUST \
         add the matching TokenId to active_sentinels() here so the \
         zero-alloc regression gate keeps exercising every accessor.",
        active_sentinels().len(),
        active_sentinel_count(),
    );
}

#[test]
fn metadata_query_is_zero_alloc() {
    let scheme = CapcoScheme::new();
    // Force one-time initialization of every `LazyLock`-backed
    // table (CVE_FILE_DERIVED + TOKEN_DERIVED) outside the
    // measurement window. The first access allocates the boxed
    // records; subsequent accesses must not.
    let warmup_start = allocs_now();
    for token in active_sentinels() {
        let _warmup_meta = scheme.metadata(token);
        let _warmup_auth = scheme.authority(token);
        let _warmup_owner = scheme.owner_producer(token);
        let _warmup_poc = scheme.point_of_contact(token);
    }
    let warmup_allocs = allocs_now() - warmup_start;

    // Vacuity guard. The whole test is meaningless if the warmup
    // didn't actually trigger `LazyLock` init — a future regression
    // that makes the accessors return cached placeholder data
    // (e.g., a const fallback path that bypasses the `LazyLock`
    // entirely) would produce 0 allocs in BOTH the warmup and the
    // measurement, falsely passing the gate. Pinning a positive
    // floor here makes that failure mode loud. The actual count
    // depends on `LazyLock` internals + the `Vec::collect` for
    // CVE_FILE_DERIVED and TOKEN_DERIVED — each is at least one
    // allocation, so the floor is conservatively 2.
    assert!(
        warmup_allocs >= 2,
        "warmup performed only {warmup_allocs} allocation(s); expected ≥2 \
         (one per LazyLock-backed Vec). The Vocabulary accessors may be \
         bypassing the LazyLock-backed tables — the zero-alloc gate would \
         pass vacuously without exercising the real path.",
    );

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
        let abbr = scheme.banner_abbreviation(token);
        std::hint::black_box(abbr);
        let d = scheme.deprecation(token);
        std::hint::black_box(d);
    }
    let allocs = allocs_now() - before;

    assert_eq!(
        allocs, 0,
        "Vocabulary accessors allocated {allocs} time(s) after warmup; \
         expected 0. Every accessor must return `&'static` data \
         (`marque-scheme::vocabulary` contract).",
    );
}
