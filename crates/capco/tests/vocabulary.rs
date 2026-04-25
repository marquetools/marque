// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Trait-level tests for `impl Vocabulary<CapcoScheme> for CapcoScheme`
//! (Phase 5 PR-2, tasks T071 / T072 / T073 / T074 / T076 / T077).
//!
//! `marque-ism` ships the raw per-token tables (Phase 5 PR-1, tested
//! in `crates/ism/tests/vocabulary_tables.rs`); this file exercises
//! the `marque-scheme::Vocabulary<S>` adapter that composes them
//! into the trait surface.
//!
//! ## Token coverage scope
//!
//! `CapcoScheme::Token = TokenId` and the active sentinel set today
//! is ~14 hand-assigned ids in `crates/capco/src/scheme.rs` — the
//! only TokenIds CAPCO's catalog actually references. Of those, the
//! ones with a corresponding canonical CVE value in the JSON-derived
//! `TOKEN_METADATA` table form the "active CAPCO vocabulary set"
//! that this file iterates. Aggregate sentinels (`TOK_US_CLASSIFIED`,
//! `TOK_NON_US_CLASSIFICATION`, `TOK_IC_DISSEM`, `TOK_NON_IC_DISSEM`),
//! trigraph sentinels (`TOK_USA` — trigraphs come from XSD, not
//! JSON), and grammar-shape sentinels (`TOK_JOINT`, `TOK_FGI_MARKER`)
//! are explicitly out of scope: they have no single CVE value to
//! attach metadata to. The Vocabulary impl panics if asked about
//! one of those, surfacing the misuse loudly.
//!
//! Phase C extends both the sentinel set and the canonical-string
//! mapping to cover the full CVE-published token vocabulary; until
//! then the per-sentinel set is what "active" means here.

use marque_capco::CapcoScheme;
use marque_capco::scheme::{
    TOK_CNWDI, TOK_EXDIS, TOK_FRD, TOK_HCS, TOK_NODIS, TOK_NOFORN, TOK_RD, TOK_RESTRICTED,
    TOK_TFNI, TOK_UCNI,
};
use marque_scheme::{TokenId, Vocabulary};

/// The set of sentinel TokenIds with a corresponding canonical CVE
/// value in `marque-ism::generated::vocabulary::TOKEN_METADATA`. Used
/// by every "iterate the active set" test below.
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

// ---------------------------------------------------------------------------
// T071 — every active token has authority + owner/producer + POC + forms.
// ---------------------------------------------------------------------------

#[test]
fn every_active_token_has_authority() {
    let scheme = CapcoScheme::new();
    for token in active_sentinels() {
        let authority = scheme.authority(token);
        assert!(
            !authority.source_name.is_empty(),
            "authority({token:?}).source_name is empty"
        );
        assert!(
            !authority.urn.is_empty(),
            "authority({token:?}).urn is empty"
        );
        assert!(
            !authority.schema_version.is_empty(),
            "authority({token:?}).schema_version is empty"
        );

        let owner = scheme.owner_producer(token);
        assert!(
            !owner.code.is_empty(),
            "owner_producer({token:?}).code is empty"
        );

        let poc = scheme.point_of_contact(token);
        assert!(
            !poc.name.is_empty(),
            "point_of_contact({token:?}).name is empty"
        );
        assert!(
            !poc.email.is_empty(),
            "point_of_contact({token:?}).email is empty"
        );
        assert!(
            poc.email.contains('@'),
            "point_of_contact({token:?}).email is not an email"
        );

        assert!(
            !scheme.portion_form(token).is_empty(),
            "portion_form({token:?}) is empty"
        );
        assert!(
            !scheme.banner_form(token).is_empty(),
            "banner_form({token:?}) is empty"
        );
    }
}

// ---------------------------------------------------------------------------
// T072 — authority traces back to ODNI for every ISM token.
// ---------------------------------------------------------------------------

#[test]
fn authority_points_to_odni_for_ism_tokens() {
    let scheme = CapcoScheme::new();
    for token in active_sentinels() {
        let authority = scheme.authority(token);
        assert!(
            authority.urn.starts_with("urn:us:gov:ic:cvenum:"),
            "authority({token:?}).urn = {:?} does not start with the ODNI prefix",
            authority.urn,
        );
        assert_eq!(
            authority.schema_version, "ISM-v2022-DEC",
            "authority({token:?}).schema_version is not the pinned package version",
        );
    }
}

// ---------------------------------------------------------------------------
// T073 — deprecated tokens carry deprecation metadata.
// ---------------------------------------------------------------------------

/// No active sentinel today is a deprecated marking — every entry in
/// `active_sentinels()` is a current, valid CAPCO token. This test
/// asserts the *negative* case: an active token returns `None` from
/// `deprecation()`. The complementary positive case (T074) lives
/// below; both arms of the `Option` are exercised between the two.
///
/// When Phase C extends the sentinel set to include deprecated tokens
/// (e.g., adding a sentinel for `25X1-` from the `MIGRATIONS` table),
/// this test should split into "active → None" and "deprecated → Some"
/// assertions over each subset.
#[test]
fn deprecated_tokens_carry_deprecation() {
    let scheme = CapcoScheme::new();
    for token in active_sentinels() {
        assert!(
            scheme.deprecation(token).is_none(),
            "deprecation({token:?}) returned Some for an active token — \
             active tokens must not carry deprecation metadata",
        );
    }
}

// ---------------------------------------------------------------------------
// T074 — replacement is populated when a deprecation maps to a known token.
// ---------------------------------------------------------------------------

/// Pin-down test for the deprecation-replacement contract.
///
/// `Deprecation { since, replacement: Option<S::Token> }` carries
/// `replacement = Some(_)` when the deprecated token has a known
/// canonical successor in the active vocabulary, and `None` when
/// "no known replacement" (FR-017 — silence is informative; do not
/// rewrite into a token that does not exist).
///
/// The `MIGRATIONS` table in `marque-ism::generated::migrations`
/// today has two entries (`25X1-` → `25X1`, `50X1-` → `50X1-HUM`),
/// neither of which corresponds to a `CapcoScheme` sentinel — so no
/// active sentinel currently has a `Some(replacement)` deprecation.
/// This test asserts the structural property: every replacement that
/// IS populated must be a TokenId that resolves cleanly in the same
/// vocabulary (no dangling pointers). Phase C will extend the
/// sentinel set to include real deprecations and this test will
/// gain genuine `Some(_)` coverage.
#[test]
fn deprecation_replacement_when_known() {
    let scheme = CapcoScheme::new();
    for token in active_sentinels() {
        if let Some(deprecation) = scheme.deprecation(token) {
            assert!(
                !deprecation.since.is_empty(),
                "deprecation({token:?}).since is empty",
            );
            if let Some(replacement) = &deprecation.replacement {
                // Self-consistency: if a replacement is named, the
                // vocabulary must be able to resolve it.
                let _ = scheme.authority(replacement);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// T076 — FOUO remains an active dissem control after the FOUO→CUI removal.
// ---------------------------------------------------------------------------

/// Constructs a FOUO-bearing document and runs it through `Engine::lint`,
/// asserting no diagnostic suggests migrating FOUO to CUI. The legacy
/// `FOUO → CUI` migration entry was retired in Phase E
/// (`crates/ism/build.rs` doc-comment under `MIGRATIONS`); FOUO remains
/// enumerated in `CVEnumISMDissem.json` and is a current, valid CAPCO
/// dissem control. Any rule that proposes a CUI replacement on FOUO is
/// a regression of FR-020.
///
/// The test phrases "no CUI suggestion" structurally rather than by
/// counting all diagnostics: a stylistic E001/E009-style fix may still
/// fire on the input, and we don't want to false-fail on those.
#[test]
fn fouo_remains_active_dissem_control() {
    use marque_capco::CapcoRuleSet;
    use marque_config::Config;
    use marque_engine::Engine;

    let engine = Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme has no rewrite cycles");
    let source = b"(U//FOUO) Test paragraph with for-official-use-only marking.";
    let result = engine.lint(source);

    for diag in &result.diagnostics {
        let message = diag.message.to_lowercase();
        assert!(
            !message.contains("cui") && !message.contains("controlled unclassified"),
            "diagnostic suggests CUI migration on FOUO input: {diag:?}",
        );
        if let Some(fix) = &diag.fix {
            let replacement_lower = fix.replacement.to_lowercase();
            assert!(
                !replacement_lower.contains("cui"),
                "fix replacement migrates FOUO to CUI: {fix:?}",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// T077 — repeated `metadata()` calls are zero-allocation.
// ---------------------------------------------------------------------------
//
// The trait contract is "every accessor returns `&'static` data — no
// runtime allocation" (vocabulary.rs invariants). The metadata table
// is heap-initialized once via `LazyLock` (a single allocation outside
// the measurement window); subsequent lookups dereference the boxed
// data directly and must not allocate.
//
// This test mirrors the count-allocs harness shape from
// `crates/core/tests/alloc_budget.rs` (gap register #15). It is gated
// behind the `count-allocs` feature so installing the global allocator
// does not pollute the default `cargo test` run.

#[cfg(feature = "count-allocs")]
mod zero_alloc {
    use super::*;
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::Mutex;
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

    static MEASURE_LOCK: Mutex<()> = Mutex::new(());

    fn count_allocs<F: FnOnce()>(body: F) -> usize {
        let _guard = MEASURE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let before = ALLOCATIONS.load(Ordering::Relaxed);
        body();
        ALLOCATIONS.load(Ordering::Relaxed) - before
    }

    #[test]
    fn metadata_query_is_zero_alloc() {
        let scheme = CapcoScheme::new();
        // Force one-time initialization of the LazyLock-backed metadata
        // table outside the measurement window. The first access
        // allocates the boxed records; subsequent accesses must not.
        let _warmup = scheme.metadata(&TOK_NOFORN);
        let _warmup = scheme.metadata(&TOK_HCS);

        let allocs = count_allocs(|| {
            for token in active_sentinels() {
                let m = scheme.metadata(token);
                std::hint::black_box(m);
                let a = scheme.authority(token);
                std::hint::black_box(a);
                let p = scheme.portion_form(token);
                std::hint::black_box(p);
            }
        });
        assert_eq!(
            allocs, 0,
            "Vocabulary accessors allocated {allocs} time(s) after warmup; \
             expected 0 (every method must return `&'static` data)",
        );
    }
}
