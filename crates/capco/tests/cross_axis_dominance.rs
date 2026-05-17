// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Cross-axis dominance tests for the per-category lattice impls.
//!
//! Each test pins a specific cross-axis rule from CAPCO-2016 to the
//! corresponding lattice / constraint / page-rewrite behavior. PR 4b-A
//! lands the AEA-axis coverage; subsequent PRs add the other
//! categories per `docs/plans/2026-05-01-lattice-design.md` §§2-8.
//!
//! # AEA category coverage (PR 4b-A)
//!
//! Four cross-axis cases:
//!
//! 1. **RD evicts FRD / TFNI** — §H.6 p104 (RD precedence over
//!    FRD/TFNI in banner roll-up). Tested via lattice join.
//! 2. **SIGMA coalesces under RD** — §H.6 p108-109 (all SIGMA
//!    numbers list under RD-SIGMA regardless of source). Tested via
//!    lattice join.
//! 3. **UCNI strips when classified** — §H.6 p116 / p118 (UCNI
//!    suppressed from banner when class > U + NOFORN promoted).
//!    Tested as the documented predicate; the runtime rewrite
//!    lands in PR 4b-C.
//! 4. **ATOMAL routes to AEA, not classification** — §H.7 p122 +
//!    §G.2 Table 5 p40 (ATOMAL is a registered standalone control
//!    marking that travels in the AEA category alongside RD/FRD/TFNI).

use marque_capco::lattice::{AeaPrimary, AeaSet, UcniKind};
use marque_ism::{AeaMarking, AtomalBlock, FrdBlock, RdBlock};
use marque_scheme::JoinSemilattice;

// ===========================================================================
// AEA: RD evicts FRD / TFNI (§H.6 p104)
// ===========================================================================

/// CAPCO-2016 §H.6 p104 (RESTRICTED DATA, Precedence Rules for Banner
/// Line Guidance): "If RD, FRD, and TFNI portions are in a document,
/// the RD takes precedence and is conveyed in the banner line. In
/// this case, use only the RD warning statement."
///
/// Tested as the lattice join over three AeaSets — one with each
/// primary marking — to verify `Lattice::join` returns the
/// supersession max under `Tfni ⊏ Frd ⊏ Rd`.
#[test]
fn aea_rd_evicts_frd_tfni() {
    let rd_set = AeaSet::from_markings(&[AeaMarking::Rd(RdBlock::default())]);
    let frd_set = AeaSet::from_markings(&[AeaMarking::Frd(FrdBlock::default())]);
    let tfni_set = AeaSet::from_markings(&[AeaMarking::Tfni]);

    // RD ⊔ FRD = RD.
    let joined = rd_set.join(&frd_set);
    assert_eq!(joined.primary(), Some(AeaPrimary::Rd));

    // RD ⊔ TFNI = RD.
    let joined = rd_set.join(&tfni_set);
    assert_eq!(joined.primary(), Some(AeaPrimary::Rd));

    // RD ⊔ FRD ⊔ TFNI = RD.
    let joined = rd_set.join(&frd_set).join(&tfni_set);
    assert_eq!(joined.primary(), Some(AeaPrimary::Rd));

    // FRD ⊔ TFNI = FRD (the §H.6 p111 "FRD takes precedence over
    // TFNI" implication of the §H.6 p120 TFNI banner-eviction rule).
    let joined = frd_set.join(&tfni_set);
    assert_eq!(joined.primary(), Some(AeaPrimary::Frd));

    // Render the RD-wins case to the boxed AeaMarking output to
    // confirm the rendered form omits the dominated atoms.
    let combined = rd_set.join(&frd_set).join(&tfni_set);
    let rendered = combined.to_markings();
    assert_eq!(rendered.len(), 1);
    assert!(matches!(&rendered[0], AeaMarking::Rd(_)));
}

// ===========================================================================
// AEA: SIGMA coalesces under RD (§H.6 p108-109 + §H.6 p113)
// ===========================================================================

/// CAPCO-2016 §H.6 p108-109 (RD-SIGMA Precedence Rules, top-of-page
/// continuation at p109): "If both RD and FRD SIGMA [#] portions are
/// in a document, the RD-SIGMA [#] marking takes precedence over the
/// FRD-SIGMA [#] marking in the banner line and all SIGMA numbers are
/// listed in the RD-SIGMA [#] marking in the banner line, regardless
/// of whether the information was RD or FRD."
///
/// Mutually cited at §H.6 p113 from the FRD-SIGMA subsection's
/// vantage. The two passages are mutual references.
///
/// Tested by joining one RD-SIGMA portion with one FRD-SIGMA portion
/// and verifying the rendered output carries all SIGMA numbers under
/// the surviving RD-SIGMA atom.
#[test]
fn aea_sigma_coalesces_under_rd() {
    // (S//RD/SIGMA 14)
    let rd_sigma_14 = AeaSet::from_markings(&[AeaMarking::Rd(RdBlock {
        cnwdi: false,
        sigma: Box::new([14]),
    })]);
    // (S//FRD/SIGMA 18)
    let frd_sigma_18 = AeaSet::from_markings(&[AeaMarking::Frd(FrdBlock {
        sigma: Box::new([18]),
    })]);

    // Join: primary = Rd; sigmas = {14, 18}; banner renders RD-SIGMA.
    let combined = rd_sigma_14.join(&frd_sigma_18);
    assert_eq!(combined.primary(), Some(AeaPrimary::Rd));
    let sigmas: Vec<u8> = combined.sigmas().iter().copied().collect();
    assert_eq!(sigmas, vec![14, 18]);

    // Render confirms the §H.6 p109 canonical form:
    // `RD-SIGMA 14 18` (ascending order per §H.6 p108
    // "Multiple SIGMA numbers must be listed in numerical order").
    let rendered = combined.to_markings();
    assert_eq!(rendered.len(), 1);
    match &rendered[0] {
        AeaMarking::Rd(rd) => {
            let nums: Vec<u8> = rd.sigma.to_vec();
            assert_eq!(nums, vec![14, 18]);
            assert!(!rd.cnwdi);
        }
        other => panic!("expected RD atom, got {other:?}"),
    }
}

// ===========================================================================
// AEA: UCNI strips when classified (§H.6 p116 / p118) — documented predicate
// ===========================================================================

/// CAPCO-2016 §H.6 p116 (DOD UCNI Precedence Rules for Banner Line
/// Guidance): "Classified documents: DOD UCNI does not appear in the
/// banner line; however, NOFORN must be applied if a less restrictive
/// FD&R marking would otherwise be conveyed with the classified
/// information."
///
/// Symmetric rule at §H.6 p118 for DOE UCNI.
///
/// PR 4b-A documents the predicate as a cross-axis rule. The actual
/// runtime rewrite (strip UCNI + promote NOFORN) lands in PR 4b-C,
/// alongside the §3 (b) FOUO eviction matrix — same algebraic shape
/// (cross-axis strip on classification ascent + NOFORN promotion).
///
/// This test verifies the **AeaSet lattice's contribution** to the
/// rule: the UCNI atom is present in the joined fact set (because
/// the lattice is permissive — strip happens at the post-projection
/// rewrite, not at lattice-join time).
///
/// **PR 4b-C WILL INVERT THE FINAL ASSERTION** — once the
/// post-projection rewrite lands, `combined.to_markings()` after
/// the cross-axis pass should drop UCNI when classification ⊐ U.
/// This test stays here as the lattice-contribution pin; the new
/// assertion against the post-rewrite state goes in a separate
/// test alongside the FOUO eviction matrix in PR 4b-C, NOT by
/// editing this one (so the regression catch for "lattice
/// preserves UCNI" stays intact).
#[test]
fn aea_ucni_strips_when_classified() {
    // (U//DOD UCNI//FOUO) — UCNI carrier portion.
    let ucni_portion = AeaSet::from_markings(&[AeaMarking::DodUcni]);
    // (S) — classified peer with no AEA content.
    let class_portion = AeaSet::empty();

    // AeaSet join leaves UCNI in place. The classification axis is
    // handled by `MarkingClassification::OrdMax` separately; that
    // join lifts the banner to S.
    let combined = ucni_portion.join(&class_portion);
    assert!(combined.ucni().contains(&UcniKind::DodUcni));
    assert!(combined.primary().is_none());

    // The actual eviction is a post-projection cross-axis rewrite
    // (PR 4b-C). Until then, the lattice contributes the
    // `{DodUcni}` atom faithfully; the §H.6 p116 strip-plus-promote
    // semantics ride on the cross-axis rewrite layer above.
    //
    // The lattice render of the post-join state intentionally
    // emits the UCNI atom (the `to_markings` path is
    // classification-agnostic — strip is a cross-axis decision):
    let rendered = combined.to_markings();
    let has_dod_ucni = rendered.iter().any(|m| matches!(m, AeaMarking::DodUcni));
    assert!(
        has_dod_ucni,
        "AeaSet rendered form preserves UCNI atom; banner-strip is post-projection (PR 4b-C)"
    );
}

// ===========================================================================
// AEA: ATOMAL routes to AEA, not classification (§G.2 Table 5 p40 +
// §H.7 p122 FGI-section worked example; ATOMAL has no dedicated §H
// entry — its registration lives in §G.2 Table 5, and the §H.7
// worked example illustrates the AEA-axis placement).
// ===========================================================================

/// CAPCO-2016 §H.7 p122 (Foreign Government Information Markings,
/// banner-roll-up worked example): "*SECRET//RD/ATOMAL//FGI NATO
/// //NOFORN, where ATOMAL is a NATO Atomic Energy Act marking that
/// follows the registered US Atomic Energy Act marking RD.*"
///
/// Confirmed by §G.2 Table 5 p40 (Conceptual Access Rights and
/// Handling by Registered Marking): ATOMAL is a registered standalone
/// control marking under "Non-US Protective Markings" with an
/// "ATOMAL read-in" handling requirement — the marking's *handling
/// classification* is non-US but its *category placement* on a
/// US-classified document is AEA (per §H.7 p122 routing).
///
/// This is the PR 9c.1 T134 routing decision (ATOMAL → AEA, not
/// NATO classification suffix). The test pins it at the `AeaSet`
/// boundary: a portion carrying RD + ATOMAL composes into an
/// `AeaSet` with both `primary=Some(Rd)` and `atomal=Some(_)`,
/// confirming ATOMAL lives in the AEA axis.
#[test]
fn aea_atomal_routes_to_aea_not_nato_class() {
    // (//CTS//RD/ATOMAL//FGI NATO//NOFORN) — the §H.7 p122 worked
    // example, AEA-axis content (RD + ATOMAL).
    let portion = AeaSet::from_markings(&[
        AeaMarking::Rd(RdBlock::default()),
        AeaMarking::Atomal(AtomalBlock),
    ]);

    // Both `Rd` and `Atomal` survive on the AeaSet.
    assert_eq!(portion.primary(), Some(AeaPrimary::Rd));
    assert_eq!(portion.atomal(), Some(AtomalBlock));

    // Render produces RD followed by ATOMAL in §G.1 Table 4
    // cat-6 register order (RD → ATOMAL):
    let rendered = portion.to_markings();
    assert_eq!(rendered.len(), 2);
    assert!(matches!(&rendered[0], AeaMarking::Rd(_)));
    assert!(matches!(&rendered[1], AeaMarking::Atomal(_)));

    // Joining with a second portion carrying only ATOMAL is
    // idempotent on the ATOMAL axis (per the OptionalSingleton
    // join law).
    let atomal_only = AeaSet::from_markings(&[AeaMarking::Atomal(AtomalBlock)]);
    let combined = portion.join(&atomal_only);
    assert_eq!(combined.atomal(), Some(AtomalBlock));
    assert_eq!(combined.primary(), Some(AeaPrimary::Rd));
}
