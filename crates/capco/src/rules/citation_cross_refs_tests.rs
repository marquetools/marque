// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

// ---------------------------------------------------------------------------
// PR 10.A.1 Commit 4 — Citation cross-reference pins
// ---------------------------------------------------------------------------
//
// Live `#[cfg(test)]` module carrying the cross-reference
// secondary-passage guards that PR 10.A.1 Commit 2 dropped when
// collapsing the dual-passage `.contains("§...") + .contains("§...")`
// assertions on diagnostic citation strings into single typed-
// `Citation` `assert_eq!`s. The pre-#561 inline `mod tests`
// block that originally carried the `.contains(...)` test bodies
// was quarantined to `_disabled_tests.rs` (`#[cfg(any())]`-gated
// dead code, disposition tracked in issue #722) — the active test
// surface in this file is the cross-ref pins below plus the
// integration tests under `crates/capco/tests/`.
//
// # Why the consts live adjacent to each rule, not centralized
//
// Per the PR brief, the cross-references are rule-authoritative
// metadata. Living adjacent to each rule's struct (or, for the
// declarative E037/E038 family, adjacent to the corresponding rule
// struct at E039) makes "where is this rule's cross-reference?"
// answerable by reading the rule's source file alone. A future PR
// 10.A.2 `Rule::cited_authorities()` trait method (deferred per the
// brief) would migrate these consts to the trait surface.
//
// # CAPCO §-citation verification
//
// Every literal §-reference asserted below was re-verified against
// `crates/capco/docs/CAPCO-2016.md` at PR 10.A.1 Commit 4 authorship
// per Constitution VIII propagation rule. See the per-const doc
// comments on `DECLASSIFY_MISPLACED_CROSS_REFS` / `JOINT_USA_FIRST_CROSS_REFS` / `NODIS_EXDIS_MUTEX_CROSS_REFS`
// / `NODIS_EXDIS_REQUIRES_NOFORN_CROSS_REFS` / `NODIS_EXDIS_CLEARS_REL_TO_CROSS_REFS` for the source passages.

use crate::rules::joint::JOINT_USA_FIRST_CROSS_REFS;
use crate::rules::nodis_exdis::{NODIS_EXDIS_MUTEX_CROSS_REFS, NODIS_EXDIS_REQUIRES_NOFORN_CROSS_REFS, NODIS_EXDIS_CLEARS_REL_TO_CROSS_REFS};
use crate::rules::text_handling::DECLASSIFY_MISPLACED_CROSS_REFS;
use marque_scheme::{Citation, SectionLetter, capco};

/// E005: secondary §D.1 p27 (banner categories exclude
/// declassification — negative-inference complement to the
/// primary §E.1 p31). PR 10.A.1 Commit 2 dropped the
/// `.contains("§D.1 p27")` assertion in
/// `e005_citation_points_at_specific_sections` (now quarantined
/// in `_disabled_tests.rs`, disposition #722).
#[test]
fn e005_cross_refs_pin_section_d_1_p27() {
    let expected: Citation = capco(SectionLetter::D, 1, 27);
    assert!(
        DECLASSIFY_MISPLACED_CROSS_REFS.contains(&expected),
        "DECLASSIFY_MISPLACED_CROSS_REFS must include §D.1 p27; got: {:?}",
        DECLASSIFY_MISPLACED_CROSS_REFS,
    );
}

/// S003: secondary §H.8 p150 (REL TO USA-first convention — the
/// IC-convention analogue S003 ports to JOINT classifications).
/// PR 10.A.1 Commit 2 dropped the `.contains("§H.8 pp 150")`
/// assertion in `s003_citation_frames_as_convention_not_mandate`.
#[test]
fn s003_cross_refs_pin_section_h_8_p150() {
    let expected: Citation = capco(SectionLetter::H, 8, 150);
    assert!(
        JOINT_USA_FIRST_CROSS_REFS.contains(&expected),
        "JOINT_USA_FIRST_CROSS_REFS must include §H.8 p150; got: {:?}",
        JOINT_USA_FIRST_CROSS_REFS,
    );
}

/// E037: secondary §H.9 p174 (NODIS mutual-exclusion clause —
/// mirror of the EXDIS clause at p172). PR 10.A.1 Commit 2
/// dropped the `.contains("p174")` assertion in
/// `e037_fires_when_nodis_and_exdis_coexist`.
#[test]
fn e037_cross_refs_pin_section_h_9_p174() {
    let expected: Citation = capco(SectionLetter::H, 9, 174);
    assert!(
        NODIS_EXDIS_MUTEX_CROSS_REFS.contains(&expected),
        "NODIS_EXDIS_MUTEX_CROSS_REFS must include §H.9 p174; got: {:?}",
        NODIS_EXDIS_MUTEX_CROSS_REFS,
    );
}

/// E038: secondary §H.9 p174 (NODIS "Requires NOFORN" — mirror
/// of the EXDIS clause at p172). PR 10.A.1 Commit 2 dropped the
/// `.contains("p174")` assertion in
/// `e038_fires_on_nodis_without_noforn`.
#[test]
fn e038_cross_refs_pin_section_h_9_p174() {
    let expected: Citation = capco(SectionLetter::H, 9, 174);
    assert!(
        NODIS_EXDIS_REQUIRES_NOFORN_CROSS_REFS.contains(&expected),
        "NODIS_EXDIS_REQUIRES_NOFORN_CROSS_REFS must include §H.9 p174; got: {:?}",
        NODIS_EXDIS_REQUIRES_NOFORN_CROSS_REFS,
    );
}

/// E039: secondary §H.9 p174 (NODIS authority for the
/// REL-TO-not-authorized rule — mirror of the EXDIS clause at
/// p172). PR 10.A.1 Commit 2 dropped the `.contains("p174")`
/// assertion in `e039_fires_on_banner_rel_to_with_nodis_portion`
/// AND the corresponding `e039_still_fires_after_engine_gap_close`
/// regression-pin site (one const covers both sites).
#[test]
fn e039_cross_refs_pin_section_h_9_p174() {
    let expected: Citation = capco(SectionLetter::H, 9, 174);
    assert!(
        NODIS_EXDIS_CLEARS_REL_TO_CROSS_REFS.contains(&expected),
        "NODIS_EXDIS_CLEARS_REL_TO_CROSS_REFS must include §H.9 p174; got: {:?}",
        NODIS_EXDIS_CLEARS_REL_TO_CROSS_REFS,
    );
}
