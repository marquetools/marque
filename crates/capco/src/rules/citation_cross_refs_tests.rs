// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

// ---------------------------------------------------------------------------
// Citation cross-reference pins
// ---------------------------------------------------------------------------
//
// Live `#[cfg(test)]` module carrying the cross-reference
// secondary-passage guards: each rule's typed `Citation` on its emitted
// diagnostic carries one passage, so these pins assert the
// cross-reference constants still include the secondary passages.
//
// # Why the consts live adjacent to each rule, not centralized
//
// The cross-references are rule-authoritative metadata. Living adjacent
// to each rule's struct (or, for the declarative NODIS/EXDIS family,
// adjacent to the corresponding `NodisExdisClearsBannerRelToRule`) makes
// "where is this rule's cross-reference?" answerable by reading the
// rule's source file alone.
//
// # CAPCO §-citation verification
//
// Every literal §-reference asserted below was verified against
// `crates/capco/docs/CAPCO-2016.md` (Constitution VIII). See the
// per-const doc comments on `DECLASSIFY_MISPLACED_CROSS_REFS` /
// `JOINT_USA_FIRST_CROSS_REFS` / `NODIS_EXDIS_MUTEX_CROSS_REFS` /
// `NODIS_EXDIS_REQUIRES_NOFORN_CROSS_REFS` /
// `NODIS_EXDIS_CLEARS_REL_TO_CROSS_REFS` for the source passages.

use super::joint::JOINT_USA_FIRST_CROSS_REFS;
use super::nodis_exdis::{
    NODIS_EXDIS_CLEARS_REL_TO_CROSS_REFS, NODIS_EXDIS_MUTEX_CROSS_REFS,
    NODIS_EXDIS_REQUIRES_NOFORN_CROSS_REFS,
};
use super::text_handling::DECLASSIFY_MISPLACED_CROSS_REFS;
use marque_scheme::{Citation, SectionLetter, capco};

/// declassify-misplaced rule: secondary §D.1 p27 (banner categories
/// exclude declassification — negative-inference complement to the
/// primary §E.1 p31).
#[test]
fn e005_cross_refs_pin_section_d_1_p27() {
    let expected: Citation = capco(SectionLetter::D, 1, 27);
    assert!(
        DECLASSIFY_MISPLACED_CROSS_REFS.contains(&expected),
        "DECLASSIFY_MISPLACED_CROSS_REFS must include §D.1 p27; got: {:?}",
        DECLASSIFY_MISPLACED_CROSS_REFS,
    );
}

/// joint-usa-first rule: secondary §H.8 p150 (REL TO USA-first
/// convention — the IC-convention analogue this rule ports to JOINT
/// classifications).
#[test]
fn s003_cross_refs_pin_section_h_8_p150() {
    let expected: Citation = capco(SectionLetter::H, 8, 150);
    assert!(
        JOINT_USA_FIRST_CROSS_REFS.contains(&expected),
        "JOINT_USA_FIRST_CROSS_REFS must include §H.8 p150; got: {:?}",
        JOINT_USA_FIRST_CROSS_REFS,
    );
}

/// NODIS/EXDIS mutual-exclusion: secondary §H.9 p174 (NODIS clause —
/// mirror of the EXDIS clause at p172).
#[test]
fn e037_cross_refs_pin_section_h_9_p174() {
    let expected: Citation = capco(SectionLetter::H, 9, 174);
    assert!(
        NODIS_EXDIS_MUTEX_CROSS_REFS.contains(&expected),
        "NODIS_EXDIS_MUTEX_CROSS_REFS must include §H.9 p174; got: {:?}",
        NODIS_EXDIS_MUTEX_CROSS_REFS,
    );
}

/// NODIS/EXDIS requires-NOFORN: secondary §H.9 p174 (NODIS "Requires
/// NOFORN" — mirror of the EXDIS clause at p172).
#[test]
fn e038_cross_refs_pin_section_h_9_p174() {
    let expected: Citation = capco(SectionLetter::H, 9, 174);
    assert!(
        NODIS_EXDIS_REQUIRES_NOFORN_CROSS_REFS.contains(&expected),
        "NODIS_EXDIS_REQUIRES_NOFORN_CROSS_REFS must include §H.9 p174; got: {:?}",
        NODIS_EXDIS_REQUIRES_NOFORN_CROSS_REFS,
    );
}

/// NODIS/EXDIS clears banner REL TO: secondary §H.9 p174 (NODIS
/// authority for the REL-TO-not-authorized rule — mirror of the EXDIS
/// clause at p172).
#[test]
fn e039_cross_refs_pin_section_h_9_p174() {
    let expected: Citation = capco(SectionLetter::H, 9, 174);
    assert!(
        NODIS_EXDIS_CLEARS_REL_TO_CROSS_REFS.contains(&expected),
        "NODIS_EXDIS_CLEARS_REL_TO_CROSS_REFS must include §H.9 p174; got: {:?}",
        NODIS_EXDIS_CLEARS_REL_TO_CROSS_REFS,
    );
}
