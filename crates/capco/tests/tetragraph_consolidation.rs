// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Issue #183 PR-B / Issue #208 consolidation parity.
//!
//! Pins the contract that `marque_capco::vocab::expand_tetragraph` and
//! the generated `marque_ism::lookup_tetragraph_members` /
//! `marque_ism::is_decomposable` tables return identical results for
//! every code. Pre-PR-B these paths were independent copies of the
//! FVEY/ACGU table that could (and did) drift. Issue #208 added the
//! ODNI ISMCAT V2022-NOV taxonomy as the single source of truth and
//! introduced the three-state decomposability discriminator that S005
//! (#206; post-PR-#488 collapsed from the historical S005/S006 pair)
//! depends on; this test suite covers all three trichotomy branches
//! and the §D Table 3 rule 23 round-trip.

use marque_capco::lattice::RelToBlock;
use marque_capco::vocab::{expand_tetragraph, is_decomposable_tetragraph};
use marque_ism::{
    CanonicalAttrs, Classification, CountryCode, MarkingClassification, TETRAGRAPH_MEMBERS,
    is_decomposable, lookup_tetragraph_members,
};

fn cc(code: &str) -> CountryCode {
    CountryCode::try_new(code.as_bytes())
        .unwrap_or_else(|| panic!("CountryCode::try_new({code:?}) failed in test fixture"))
}

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
    // Codes intentionally absent from the canonical members table:
    // - decomposable="No" atoms (EU, GCCH, KFOR) — atom by authority.
    // - decomposable="NA" deprecated codes (RSMA, ISAF, MCFI).
    // - trigraphs (USA, GBR) — undefined for tetragraph expansion.
    // - codes outside the taxonomy entirely (XYZW).
    //
    // NATO is intentionally NOT in this list — issue #208 added it as
    // a decomposable=Yes entry with 30 trigraph members. Pre-issue-208
    // it was hardcoded as opaque; that gap is now closed.
    for code in [
        "EU", "GCCH", "KFOR", "RSMA", "ISAF", "MCFI", "USA", "GBR", "XYZW",
    ] {
        assert_eq!(
            expand_tetragraph(code),
            lookup_tetragraph_members(code),
            "vocab::expand_tetragraph({code:?}) and \
             lookup_tetragraph_members({code:?}) must agree on \
             absence — they share one source of truth post-issue-208"
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

// -- Issue #208 trichotomy branches --------------------------------

#[test]
fn trichotomy_decomposable_yes_with_members() {
    // Plan §2.8 first branch: decomposable="Yes" with materialized
    // member lists. Issue #208's `is_decomposable` returns Some(true)
    // and `lookup_tetragraph_members` returns Some(non-empty).
    for code in ["FVEY", "ACGU", "TEYE", "NATO", "AUSTRALIA_GROUP", "NSG"] {
        assert_eq!(
            is_decomposable(code),
            Some(true),
            "ISMCAT V2022-NOV decomposable=\"Yes\" code {code:?} \
             must map to is_decomposable == Some(true)",
        );
        let members =
            lookup_tetragraph_members(code).unwrap_or_else(|| panic!("{code} should have members"));
        assert!(
            !members.is_empty(),
            "decomposable=\"Yes\" code {code:?} must have non-empty members",
        );
        // Cross-check the capco/vocab wrapper — same answer.
        assert_eq!(is_decomposable_tetragraph(code), Some(true));
    }
}

#[test]
fn trichotomy_decomposable_no_atom_by_authority() {
    // Plan §2.8 second branch: decomposable="No" entries are atoms
    // by authority — Some(false), no members.
    for code in ["EU", "GCCH", "KFOR"] {
        assert_eq!(
            is_decomposable(code),
            Some(false),
            "ISMCAT V2022-NOV decomposable=\"No\" code {code:?} \
             must map to is_decomposable == Some(false)",
        );
        assert!(
            lookup_tetragraph_members(code).is_none(),
            "decomposable=\"No\" code {code:?} must have no \
             materialized member list — it's an atom",
        );
        assert_eq!(is_decomposable_tetragraph(code), Some(false));
    }
}

#[test]
fn trichotomy_decomposable_na_deprecated_suppressed() {
    // Plan §2.8 third branch — first sub-shape: NA-deprecated with
    // <MembershipSupressed/> sentinel. RSMA / ISAF / MCFI are the
    // canonical suppressed cases.
    for code in ["RSMA", "ISAF", "MCFI"] {
        assert_eq!(
            is_decomposable(code),
            None,
            "ISMCAT V2022-NOV decomposable=\"NA\" suppressed code \
             {code:?} must map to is_decomposable == None",
        );
        assert!(
            lookup_tetragraph_members(code).is_none(),
            "deprecated suppressed code {code:?} must have no \
             materialized member list",
        );
    }
}

#[test]
fn trichotomy_decomposable_na_deprecated_description() {
    // Plan §2.8 third branch — second sub-shape: NA-deprecated with
    // <Description> body (OCA-deferral pointer). EUDA is the
    // canonical case (also MPFL, PGMF — same shape).
    for code in ["EUDA", "MPFL", "PGMF"] {
        assert_eq!(
            is_decomposable(code),
            None,
            "ISMCAT V2022-NOV decomposable=\"NA\" Description code \
             {code:?} must map to is_decomposable == None",
        );
        assert!(
            lookup_tetragraph_members(code).is_none(),
            "deprecated Description code {code:?} must have no \
             materialized member list",
        );
    }
}

#[test]
fn trichotomy_decomposable_na_deprecated_recursive() {
    // Plan §2.8 third branch — third sub-shape: NA-deprecated with
    // <Organization> ref (the BHTF case in V2022-NOV — runtime-inert
    // because NA→None already covers it). Build-guard #4 in build.rs
    // emits cargo:warning= if a future revision lands a non-NA
    // recursive entry; this test pins the V2022-NOV ground truth.
    assert_eq!(is_decomposable("BHTF"), None);
    assert!(lookup_tetragraph_members("BHTF").is_none());
}

#[test]
fn trichotomy_unknown_or_extension_routes_to_none() {
    // Plan §2.8 fourth branch: a synthetic 4-letter code outside
    // both the taxonomy and country_extensions.toml. is_decomposable
    // returns None (taxonomy-absent), and lookup_tetragraph_members
    // also returns None.
    //
    // Per plan §2.3, org-fork extensions deliberately route to None
    // for is_decomposable even when they declare members — extensions
    // don't carry ODNI authority, only the taxonomy does. S005 fires
    // on such codes precisely because the marking depends on
    // org-local data ODNI didn't bless.
    assert_eq!(is_decomposable("XYZW"), None);
    assert!(lookup_tetragraph_members("XYZW").is_none());
}

// -- §D Table 3 rule 23 round-trip (the silent-loss case from #183) --

#[test]
fn rel_to_intersection_d_table_3_rule_23_round_trip() {
    // Plan §2.8 round-trip: expected_rel_to([REL TO USA, FVEY],
    // [REL TO USA, GBR]) → {USA, GBR}.
    //
    // Pre-issue-208 this case worked for FVEY because FVEY was in the
    // hand-curated BUILTIN_TETRAGRAPH_MEMBERS. Issue #208 sources the
    // same row from the ODNI taxonomy; this test pins that the
    // round-trip is preserved across the source change.
    let mut p1 = CanonicalAttrs::default();
    p1.classification = Some(MarkingClassification::Us(Classification::Secret));
    p1.rel_to = vec![cc("USA"), cc("FVEY")].into_boxed_slice();

    let mut p2 = CanonicalAttrs::default();
    p2.classification = Some(MarkingClassification::Us(Classification::Secret));
    p2.rel_to = vec![cc("USA"), cc("GBR")].into_boxed_slice();

    // Uses `RelToBlock::from_attrs_iter` (lattice-native) for tetragraph
    // expansion + intersection semantics — §D Table 3 rule 23 round-trip
    // pinned identically. §-authority unchanged.
    let rel = RelToBlock::from_attrs_iter(&[p1, p2]).into_boxed_slice();
    let codes: std::collections::BTreeSet<String> = rel
        .iter()
        .map(|c| String::from_utf8_lossy(c.as_bytes()).into_owned())
        .collect();
    let expected: std::collections::BTreeSet<String> =
        ["USA", "GBR"].into_iter().map(String::from).collect();
    assert_eq!(
        codes, expected,
        "§D Table 3 rule 23 round-trip: REL TO USA, FVEY ∩ \
         REL TO USA, GBR should reduce to {{USA, GBR}}, got {codes:?}",
    );
}
