// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Issue #183 PR-B consolidation parity: pin the contract that
//! `marque_capco::vocab::expand_tetragraph` and the generated
//! `marque_ism::lookup_tetragraph_members` table return identical
//! results for every code.
//!
//! Pre-PR-B these two paths were independent copies of the
//! FVEY/ACGU table that could (and did) drift. PR-B routes both
//! through one source; this test fails loudly if a future change
//! re-introduces drift.

use marque_capco::vocab::expand_tetragraph;
use marque_ism::{TETRAGRAPH_MEMBERS, lookup_tetragraph_members};

#[test]
fn capco_expand_tetragraph_matches_canonical_table_for_every_row() {
    // Iterate every row in the canonical generated table; each
    // must be reachable through the capco/vocab wrapper with
    // identical members.
    for (code, members) in TETRAGRAPH_MEMBERS {
        assert_eq!(
            expand_tetragraph(code),
            Some(*members),
            "vocab::expand_tetragraph({code:?}) drifted from \
             marque_ism::lookup_tetragraph_members — issue #183 \
             PR-B consolidation invariant violated"
        );
    }
}

#[test]
fn capco_expand_tetragraph_matches_canonical_lookup_for_known_negatives() {
    // Codes intentionally absent from the canonical table must
    // also be absent through the capco/vocab wrapper.
    for code in ["NATO", "RSMA", "ISAF", "KFOR", "USA", "GBR", "XYZW"] {
        assert_eq!(
            expand_tetragraph(code),
            lookup_tetragraph_members(code),
            "vocab::expand_tetragraph({code:?}) and \
             lookup_tetragraph_members({code:?}) must agree on \
             absence — they share one source of truth post-PR-B"
        );
    }
}

#[test]
fn capco_constants_match_canonical_table() {
    // The `pub const FVEY` and `pub const ACGU` re-exports in
    // capco/vocab are convenience constants, but they must
    // continue to match the generated table — if the canonical
    // membership ever changes, these constants need to update too.
    assert_eq!(
        marque_capco::vocab::FVEY,
        lookup_tetragraph_members("FVEY").unwrap(),
    );
    assert_eq!(
        marque_capco::vocab::ACGU,
        lookup_tetragraph_members("ACGU").unwrap(),
    );
}
