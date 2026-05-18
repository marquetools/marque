// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Sub-PR 8.F-engine-gap — Pattern A NODIS/EXDIS REL-TO clear.
//!
//! Originally this file pinned a dual-route alignment invariant between
//! `PageContext::expected_rel_to()` (engine-side read API) and the
//! `scheme.project(Scope::Page, ...)` (PageRewrite-driven) route. PR 4b-E
//! retired the engine-side accessor surface entirely; the dual-route
//! claim collapses to a single-route assertion against
//! `scheme.project(Scope::Page, ...)`, which is now the production path
//! the architect plan §6 names.
//!
//! # What this file pins post-PR-4b-E
//!
//! The §H.9 p172 (EXDIS) / p174 (NODIS) authoritative semantic — "REL TO
//! is not authorized in the banner line if any portion contains [NODIS /
//! EXDIS] information. In this case, NOFORN would convey in the banner
//! line." — must produce an empty banner REL TO via the declarative
//! PageRewrite catalog (`capco/{nodis,exdis}-implies-noforn` +
//! `capco/noforn-clears-rel-to`). Test 3 is the negative case: when no
//! NODIS/EXDIS/NOFORN/{SBU,LES}-NF triggers are present, REL TO
//! intersects normally.
//!
//! # File naming
//!
//! The file name is retained for git-blame continuity even though
//! "page_context_alignment" no longer applies post-PR-4b-E. A future
//! cleanup may rename.

use marque_capco::{CapcoMarking, CapcoScheme};
use marque_ism::{CanonicalAttrs, Classification, CountryCode, MarkingClassification, NonIcDissem};
use marque_scheme::{MarkingScheme, Scope};

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

fn gbr() -> CountryCode {
    CountryCode::try_new(b"GBR")
        .expect("GBR is a valid 3-char CAPCO trigraph per CVEnumISMCATRelTo.xsd")
}

fn fra() -> CountryCode {
    CountryCode::try_new(b"FRA")
        .expect("FRA is a valid 3-char CAPCO trigraph per CVEnumISMCATRelTo.xsd")
}

fn portion_with_non_ic(c: Classification, non_ic: &[NonIcDissem]) -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(c));
    a.non_ic_dissem = non_ic.to_vec().into_boxed_slice();
    a
}

fn portion_with_rel_to(c: Classification, countries: &[CountryCode]) -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(c));
    a.rel_to = countries.to_vec().into_boxed_slice();
    a
}

fn lift(a: CanonicalAttrs) -> CapcoMarking {
    CapcoMarking::new(a)
}

// ---------------------------------------------------------------------------
// Test 1 — NODIS clears REL TO via scheme.project (Pattern A row)
// ---------------------------------------------------------------------------

/// `(S//NODIS)` portion paired with `(S//REL TO USA, GBR)`: the
/// `capco/nodis-implies-noforn` PageRewrite fires, adding NOFORN to page
/// CAT_DISSEM; `capco/noforn-clears-rel-to` then clears CAT_REL_TO.
///
/// Authority: CAPCO-2016 §H.9 p174 verbatim — "REL TO is not authorized
/// in the banner line if any portion contains NODIS information. In this
/// case, NOFORN would convey in the banner line."
#[test]
fn nodis_portion_clears_rel_to_via_page_rewrite() {
    let scheme = CapcoScheme::new();

    let nodis_portion = portion_with_non_ic(Classification::Secret, &[NonIcDissem::Nodis]);
    let rel_to_portion = portion_with_rel_to(Classification::Secret, &[CountryCode::USA, gbr()]);

    let projected = scheme.project(
        Scope::Page,
        &[lift(nodis_portion), lift(rel_to_portion)],
    );

    assert!(
        projected.0.rel_to.is_empty(),
        "scheme.project(Scope::Page) must clear REL TO when any portion \
         has NODIS (capco/nodis-implies-noforn → capco/noforn-clears-rel-to \
         per §H.9 p174); got rel_to = {:?}",
        projected.0.rel_to,
    );
}

// ---------------------------------------------------------------------------
// Test 2 — EXDIS clears REL TO via scheme.project
// ---------------------------------------------------------------------------

/// Symmetric to test 1 for EXDIS.
///
/// Authority: CAPCO-2016 §H.9 p172 verbatim — "REL TO is not authorized
/// in the banner line if any portion contains EXDIS information. In this
/// case, NOFORN would convey in the banner line."
#[test]
fn exdis_portion_clears_rel_to_via_page_rewrite() {
    let scheme = CapcoScheme::new();

    let exdis_portion = portion_with_non_ic(Classification::Secret, &[NonIcDissem::Exdis]);
    let rel_to_portion = portion_with_rel_to(Classification::Secret, &[CountryCode::USA, gbr()]);

    let projected = scheme.project(
        Scope::Page,
        &[lift(exdis_portion), lift(rel_to_portion)],
    );

    assert!(
        projected.0.rel_to.is_empty(),
        "scheme.project(Scope::Page) must clear REL TO when any portion \
         has EXDIS (capco/exdis-implies-noforn → capco/noforn-clears-rel-to \
         per §H.9 p172); got rel_to = {:?}",
        projected.0.rel_to,
    );
}

// ---------------------------------------------------------------------------
// Test 3 — Inverse / negative case (rust-reviewer MEDIUM finding)
// ---------------------------------------------------------------------------

/// Pin that the §H.9 NODIS/EXDIS short-circuit does NOT over-fire when
/// neither trigger is present.
///
/// Fixture: two REL TO portions with overlapping country sets (USA, GBR)
/// and one with USA, GBR, FRA. No NODIS, no EXDIS, no NOFORN, no SBU-NF,
/// no LES-NF — none of the four `needs_nf` triggers are in play.
///
/// Expected: REL TO intersects normally to {USA, GBR}.
///
/// Why this matters: a too-broad short-circuit (e.g., one that fires on
/// ANY non-empty `non_ic_dissem` regardless of content) would silently
/// break country-rollup for documents containing unrelated non-IC tokens
/// (FOUO, SBU without NOFORN, LES without NOFORN). This test guards
/// against that regression.
#[test]
fn rel_to_intersection_preserved_when_no_nodis_or_exdis_present() {
    let scheme = CapcoScheme::new();

    let p1 = portion_with_rel_to(Classification::Secret, &[CountryCode::USA, gbr()]);
    let p2 = portion_with_rel_to(Classification::Secret, &[CountryCode::USA, gbr(), fra()]);

    let projected = scheme.project(Scope::Page, &[lift(p1), lift(p2)]);

    let projected_set: std::collections::BTreeSet<&str> =
        projected.0.rel_to.iter().map(|c| c.as_str()).collect();
    let expected_set: std::collections::BTreeSet<&str> = ["USA", "GBR"].into_iter().collect();
    assert_eq!(
        projected_set, expected_set,
        "scheme.project(Scope::Page) must compute the REL TO intersection \
         normally when no NODIS/EXDIS/NOFORN/{{SBU,LES}}-NF triggers are \
         present; got rel_to = {:?}",
        projected.0.rel_to,
    );
}
