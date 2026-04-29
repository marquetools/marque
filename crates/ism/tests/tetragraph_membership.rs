// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Integration tests for the build-time canonical tetragraph
//! membership table.
//!
//! Pin the contract that `marque-ism::TETRAGRAPH_MEMBERS` is the
//! single source of truth for tetragraph → constituent-trigraph
//! mappings, consumed by `marque-ism::page_context` and
//! `marque-capco::vocab`. Issue #183 PR-B established the
//! consolidation; issue #208 swapped the source from a hand-curated
//! `BUILTIN_TETRAGRAPH_MEMBERS` slice to the ODNI ISMCAT V2022-NOV
//! Tetragraph Taxonomy parsed at build time.
//!
//! These tests assume the empty-default `country_extensions.toml`
//! that ships in upstream. Org forks that add extensions will see
//! the per-extension assertions here pass with their additions
//! visible in `lookup_tetragraph_members` — those tests live in the
//! org fork.

use marque_ism::{TETRAGRAPH_MEMBERS, lookup_tetragraph_members};

#[test]
fn table_is_sorted_for_binary_search() {
    // `lookup_tetragraph_members` uses `binary_search_by_key` over
    // the slice. If a future build-rs change emits unsorted rows,
    // lookups would silently miss — pin the invariant here so a
    // regression fails loudly rather than at the rule boundary.
    for window in TETRAGRAPH_MEMBERS.windows(2) {
        assert!(
            window[0].0 < window[1].0,
            "TETRAGRAPH_MEMBERS not strictly sorted: {:?} >= {:?}",
            window[0].0,
            window[1].0,
        );
    }
}

#[test]
fn fvey_canonical_membership() {
    // Members are sorted ASCII-alphabetical at emit time. The ODNI
    // taxonomy XML lists them in publication order (`AUS, CAN, NZL,
    // GBR, USA`), but constituent order carries no semantic weight —
    // FVEY in REL TO is a single token, and banner roll-up re-sorts
    // per CAPCO §H.8 regardless. Alphabetical wins on developer
    // ergonomics and diff stability.
    let fvey = lookup_tetragraph_members("FVEY").expect("FVEY is canonical");
    assert_eq!(fvey, &["AUS", "CAN", "GBR", "NZL", "USA"]);
}

#[test]
fn acgu_canonical_membership() {
    let acgu = lookup_tetragraph_members("ACGU").expect("ACGU is canonical");
    assert_eq!(acgu, &["AUS", "CAN", "GBR", "USA"]);
}

#[test]
fn nato_has_taxonomy_membership() {
    // Issue #208: NATO is decomposable="Yes" in the ISMCAT V2022-NOV
    // taxonomy with a materialized 30-trigraph member list. The
    // pre-issue-208 "opaque NATO" behavior was a hand-curated gap,
    // not a deliberate design — closing it is the whole point of
    // sourcing from ODNI's taxonomy.
    let nato = lookup_tetragraph_members("NATO").expect("NATO is decomposable in ISMCAT V2022-NOV");
    assert!(!nato.is_empty(), "NATO members must not be empty");
    // USA is one of the 30 NATO members; smoke-check.
    assert!(nato.contains(&"USA"), "NATO should contain USA");
}

#[test]
fn deprecated_tetragraphs_are_opaque() {
    // Operation-specific codes that are decomposable="NA" in V2022-NOV
    // (deprecated; membership suppressed or OCA-deferred) are still
    // present in the CVE recognition surface but absent from the
    // members table. They compose as opaque atoms in intersection.
    for code in ["RSMA", "ISAF", "MCFI", "SFOR"] {
        assert!(
            lookup_tetragraph_members(code).is_none(),
            "{code} must be opaque (NA-deprecated; no materialized membership)",
        );
    }
}

#[test]
fn no_decomposable_tetragraphs_are_opaque() {
    // decomposable="No" codes (atom by authority) are also opaque
    // for tetragraph expansion — the code IS the recipient.
    for code in ["EU", "GCCH", "KFOR", "MNTF"] {
        assert!(
            lookup_tetragraph_members(code).is_none(),
            "{code} must be opaque (decomposable=\"No\" — atom by authority)",
        );
    }
}

#[test]
fn trigraph_lookup_returns_none() {
    // `lookup_tetragraph_members("USA")` is `None` because
    // trigraphs have no expansion. Composition treats them as
    // atoms in their own right.
    assert!(lookup_tetragraph_members("USA").is_none());
    assert!(lookup_tetragraph_members("GBR").is_none());
}

#[test]
fn unknown_code_returns_none() {
    assert!(lookup_tetragraph_members("XYZW").is_none());
    assert!(lookup_tetragraph_members("").is_none());
}

#[test]
fn empty_extensions_file_produces_baseline_membership_count() {
    // The shipped `country_extensions.toml` is empty (header
    // comments only, no `[[code]]` entries), so the membership
    // table holds only the taxonomy-sourced rows. If an org fork
    // adds extensions WITH `members`, this count grows — that's
    // the expected build-time signal for the fork to update
    // their own tests.
    //
    // The invariant pinned here is: an empty extensions file
    // produces a build with TETRAGRAPH_MEMBERS.len() == 24 — the
    // 24 decomposable="Yes" entries with materialized non-recursive
    // <Country> lists in ISMCAT V2022-NOV. A future taxonomy
    // revision that adds or removes Yes-decomposable entries will
    // bump this count and force the test to be updated alongside
    // the version pin in [package.metadata.marque].
    assert_eq!(
        TETRAGRAPH_MEMBERS.len(),
        24,
        "upstream baseline: 24 decomposable=\"Yes\" entries with \
         materialized members in ISMCAT V2022-NOV. A non-baseline \
         count means either country_extensions.toml has entries with \
         `members`, or a taxonomy revision changed the Yes-decomposable \
         set without updating this test (issue #208).",
    );
}
