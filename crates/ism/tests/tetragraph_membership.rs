// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Integration tests for the build-time canonical tetragraph
//! membership table (issue #183 PR-B).
//!
//! Pin the contract that `marque-ism::TETRAGRAPH_MEMBERS` is the
//! single source of truth for tetragraph → constituent-trigraph
//! mappings, consumed by `marque-ism::page_context` and
//! `marque-capco::vocab` after the PR-B consolidation.
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
    let fvey = lookup_tetragraph_members("FVEY").expect("FVEY is canonical");
    assert_eq!(fvey, &["AUS", "CAN", "GBR", "NZL", "USA"]);
}

#[test]
fn acgu_canonical_membership() {
    let acgu = lookup_tetragraph_members("ACGU").expect("ACGU is canonical");
    assert_eq!(acgu, &["AUS", "CAN", "GBR", "USA"]);
}

#[test]
fn nato_is_opaque_not_in_table() {
    // NATO membership is treaty-driven and omitted from the
    // generated table until the Phase F NATO scheme adapter lands.
    // Until then, NATO must compose as an opaque atom in
    // intersection — `lookup_tetragraph_members("NATO") == None`
    // is what guarantees that.
    assert!(lookup_tetragraph_members("NATO").is_none());
}

#[test]
fn operation_specific_tetragraphs_are_opaque() {
    // Operation-specific codes (RSMA / ISAF / KFOR / SFOR / MNTF / …)
    // are recognized by `is_trigraph` (they're in CVE) but the
    // generated table omits them — they have no membership data.
    for code in ["RSMA", "ISAF", "KFOR", "SFOR", "MNTF"] {
        assert!(
            lookup_tetragraph_members(code).is_none(),
            "{code} must be opaque (no membership)",
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
    // table holds only the two built-in rows. If an org fork
    // adds extensions WITH `members`, this count grows — that's
    // the expected build-time signal for the fork to update
    // their own tests.
    //
    // The invariant pinned here is: an empty extensions file
    // produces a build with TETRAGRAPH_MEMBERS.len() == 2
    // (FVEY + ACGU built-ins, no extensions contributing).
    assert_eq!(
        TETRAGRAPH_MEMBERS.len(),
        2,
        "upstream baseline: only FVEY and ACGU built-ins. \
         A non-baseline count means either the country_extensions.toml \
         shipped in this build has entries with `members`, or a new \
         built-in tetragraph membership row was added without updating \
         this test.",
    );
}
