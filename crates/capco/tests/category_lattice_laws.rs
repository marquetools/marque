// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Per-category lattice law property tests.
//!
//! Verifies the algebraic laws (associativity, commutativity,
//! idempotency, identity-with-bottom) for each CAPCO category's
//! lattice impl. Lands in PR 4b-A for the AEA category; subsequent
//! PRs add coverage for the other six categories per
//! `docs/plans/2026-05-01-lattice-design.md` §§2-8.
//!
//! # AEA category coverage (PR 4b-A)
//!
//! The AEA `Product` decomposes into five sub-axes (per §7.5 of the
//! design doc); each axis carries its own algebraic shape and gets
//! its own law-suite below. The composite `AeaSet` laws ride on top.
//!
//! Test names are referenced by the design doc §7.5 acceptance
//! checklist — adding / renaming a test here MUST update the design
//! doc reference for traceability.

use std::collections::BTreeSet;

use marque_capco::lattice::{AeaPrimary, AeaSet, UcniKind};
use marque_ism::{AeaMarking, AtomalBlock, FrdBlock, RdBlock};
use marque_scheme::{JoinSemilattice, MeetSemilattice};
use proptest::prelude::*;

// ===========================================================================
// AEA: Axis 1 — `SupersessionSet<AeaPrimary>` (RD ⊐ FRD ⊐ TFNI)
// ===========================================================================

/// CAPCO-2016 §H.6 p104 + p111 + p120: the primary axis is a
/// total-order supersession chain `Tfni ⊏ Frd ⊏ Rd`. The lattice
/// laws follow trivially from `max` over a total order — this test
/// pins them so a future refactor that, say, accidentally turns the
/// chain into a partial order trips the proptest harness.
#[test]
fn aea_primary_supersession_assoc_comm_idem() {
    // Enumerate the 4-element set of primary states ({None, Tfni,
    // Frd, Rd}) and check all triples by exhaustion. The state space
    // is small enough that brute-force is cleaner than proptest here.
    let states: [Option<AeaPrimary>; 4] = [
        None,
        Some(AeaPrimary::Tfni),
        Some(AeaPrimary::Frd),
        Some(AeaPrimary::Rd),
    ];
    let lift = |p: Option<AeaPrimary>| {
        let mut s = AeaSet::empty();
        if let Some(primary) = p {
            // We can't construct `AeaSet` with only the primary set;
            // use `from_markings` with the appropriate atom.
            let m = match primary {
                AeaPrimary::Tfni => AeaMarking::Tfni,
                AeaPrimary::Frd => AeaMarking::Frd(FrdBlock::default()),
                AeaPrimary::Rd => AeaMarking::Rd(RdBlock::default()),
            };
            s = AeaSet::from_markings(&[m]);
        }
        s
    };
    for a in states {
        for b in states {
            let la = lift(a);
            let lb = lift(b);
            // Commutativity.
            assert_eq!(la.join(&lb).primary(), lb.join(&la).primary(), "comm");
            // Idempotency.
            assert_eq!(la.join(&la).primary(), la.primary(), "idem");
            for c in states {
                let lc = lift(c);
                // Associativity.
                assert_eq!(
                    la.join(&lb).join(&lc).primary(),
                    la.join(&lb.join(&lc)).primary(),
                    "assoc"
                );
            }
        }
    }
    // Identity with bottom (`None` is bottom).
    let bottom = AeaSet::empty();
    for s in states {
        let ls = lift(s);
        assert_eq!(bottom.join(&ls).primary(), ls.primary());
        assert_eq!(ls.join(&bottom).primary(), ls.primary());
    }
}

// ===========================================================================
// AEA: Axis 2 — `FlatSet<CnwdiPresence>` (closed singleton on `bool`)
// ===========================================================================

/// CAPCO-2016 §H.6 p106: CNWDI presence propagates as a boolean
/// OR-monotone — once any RD portion carries CNWDI, the banner
/// RD-block carries it. Tested over the closed `{false, true}` set.
#[test]
fn aea_cnwdi_flatset_assoc_comm_idem() {
    let mk = |cnwdi: bool| {
        if cnwdi {
            AeaSet::from_markings(&[AeaMarking::Rd(RdBlock {
                cnwdi: true,
                sigma: Box::new([]),
            })])
        } else {
            AeaSet::from_markings(&[AeaMarking::Rd(RdBlock::default())])
        }
    };
    let a_false = mk(false);
    let a_true = mk(true);
    // Commutativity.
    assert_eq!(a_false.join(&a_true).cnwdi(), a_true.join(&a_false).cnwdi());
    // Idempotency.
    assert!(!a_false.join(&a_false).cnwdi());
    assert!(a_true.join(&a_true).cnwdi());
    // Associativity (degenerate but worth pinning).
    assert_eq!(
        a_false.join(&a_true).join(&a_true).cnwdi(),
        a_false.join(&a_true.join(&a_true)).cnwdi()
    );
    // OR-monotonicity.
    assert!(a_false.join(&a_true).cnwdi());
}

// ===========================================================================
// AEA: Axis 3 — `FlatSet<SigmaNumber>` (open-vocab `u8`)
// ===========================================================================

/// Helper: build an AeaSet carrying only the given SIGMA numbers
/// under RD-SIGMA. Used by the axis-3 proptest below to avoid
/// re-running parser-side logic.
fn mk_aea_sigmas(sigmas: &[u8]) -> AeaSet {
    AeaSet::from_markings(&[AeaMarking::Rd(RdBlock {
        cnwdi: false,
        sigma: sigmas.to_vec().into_boxed_slice(),
    })])
}

proptest! {
    /// CAPCO-2016 §H.6 p108: SIGMA numbers compose as set-union; the
    /// canonical render is ascending sort. Tested via proptest over
    /// arbitrary u8 sets (which over-covers the §H.6 14/15/18/20
    /// vocabulary but exercises the algebraic shape).
    #[test]
    fn aea_sigma_flatset_assoc_comm_idem(
        a in proptest::collection::btree_set(0u8..255, 0..6),
        b in proptest::collection::btree_set(0u8..255, 0..6),
        c in proptest::collection::btree_set(0u8..255, 0..6),
    ) {
        let av: Vec<u8> = a.iter().copied().collect();
        let bv: Vec<u8> = b.iter().copied().collect();
        let cv: Vec<u8> = c.iter().copied().collect();
        let la = mk_aea_sigmas(&av);
        let lb = mk_aea_sigmas(&bv);
        let lc = mk_aea_sigmas(&cv);

        // Commutativity.
        prop_assert_eq!(la.join(&lb).sigmas().clone(), lb.join(&la).sigmas().clone());
        // Associativity.
        prop_assert_eq!(
            la.join(&lb).join(&lc).sigmas().clone(),
            la.join(&lb.join(&lc)).sigmas().clone()
        );
        // Idempotency.
        prop_assert_eq!(la.join(&la).sigmas().clone(), la.sigmas().clone());
    }
}

// ===========================================================================
// AEA: Axis 4 — `FlatSet<UcniKind>` (closed `{DodUcni, DoeUcni}`)
// ===========================================================================

/// CAPCO-2016 §H.6 p116-117 + p118-119: UCNI variants compose as
/// set-union over the closed two-element vocabulary. Tested by
/// exhausting the four-element power-set.
#[test]
fn aea_ucni_flatset_assoc_comm_idem() {
    let mk = |dod: bool, doe: bool| {
        let mut v = Vec::new();
        if dod {
            v.push(AeaMarking::DodUcni);
        }
        if doe {
            v.push(AeaMarking::DoeUcni);
        }
        AeaSet::from_markings(&v)
    };
    let states = [
        mk(false, false),
        mk(true, false),
        mk(false, true),
        mk(true, true),
    ];
    for a in &states {
        for b in &states {
            assert_eq!(a.join(b).ucni().clone(), b.join(a).ucni().clone(), "comm");
            assert_eq!(a.join(a).ucni().clone(), a.ucni().clone(), "idem");
            for c in &states {
                assert_eq!(
                    a.join(b).join(c).ucni().clone(),
                    a.join(&b.join(c)).ucni().clone(),
                    "assoc"
                );
            }
        }
    }
}

// ===========================================================================
// AEA: Axis 5 — `OptionalSingleton<AtomalBlock>`
// ===========================================================================

/// CAPCO-2016 §H.7 p122 + §G.2 Table 5 p40: ATOMAL composes as
/// `OptionalSingleton::join = a.or(b)`. Tested by exhausting the
/// four-element pair-state.
#[test]
fn aea_atomal_optional_singleton_identity() {
    let none_set = AeaSet::empty();
    let atomal_set = AeaSet::from_markings(&[AeaMarking::Atomal(AtomalBlock)]);

    // Identity with bottom.
    assert_eq!(none_set.join(&none_set).atomal(), None);
    assert_eq!(none_set.join(&atomal_set).atomal(), Some(AtomalBlock));
    assert_eq!(atomal_set.join(&none_set).atomal(), Some(AtomalBlock));
    // Idempotency.
    assert_eq!(atomal_set.join(&atomal_set).atomal(), Some(AtomalBlock));
    // Commutativity.
    assert_eq!(
        none_set.join(&atomal_set).atomal(),
        atomal_set.join(&none_set).atomal()
    );
    // Associativity.
    assert_eq!(
        none_set.join(&atomal_set).join(&none_set).atomal(),
        none_set.join(&atomal_set.join(&none_set)).atomal()
    );
}

// ===========================================================================
// AEA: Product composition (`AeaSet` overall)
// ===========================================================================

/// Build an `AeaSet` from a flat parameter tuple — used by the
/// proptest below to exercise the Product composition under random
/// inputs.
fn mk_aea(
    primary: Option<AeaPrimary>,
    cnwdi: bool,
    sigmas: &BTreeSet<u8>,
    ucni: &BTreeSet<UcniKind>,
    atomal: bool,
) -> AeaSet {
    let mut v: Vec<AeaMarking> = Vec::new();
    let sigma_slice: Box<[u8]> = sigmas
        .iter()
        .copied()
        .collect::<Vec<_>>()
        .into_boxed_slice();
    match primary {
        Some(AeaPrimary::Rd) => v.push(AeaMarking::Rd(RdBlock {
            cnwdi,
            sigma: sigma_slice,
        })),
        Some(AeaPrimary::Frd) => v.push(AeaMarking::Frd(FrdBlock { sigma: sigma_slice })),
        Some(AeaPrimary::Tfni) => v.push(AeaMarking::Tfni),
        None => {
            // CNWDI / SIGMA without a primary anchor is malformed
            // input (caught by E067 / future SIGMA-requires-RD).
            // Construct an `AeaSet` that carries those fields by
            // anchoring with a synthetic RD then stripping the
            // primary — but we can't strip in the public surface.
            // Instead, skip those fields when primary is None;
            // the law tests stay valid because `None`-primary
            // states with cnwdi/sigmas don't arise in practice
            // and the lattice is total over the reachable inputs.
            let _ = cnwdi;
        }
    }
    if ucni.contains(&UcniKind::DodUcni) {
        v.push(AeaMarking::DodUcni);
    }
    if ucni.contains(&UcniKind::DoeUcni) {
        v.push(AeaMarking::DoeUcni);
    }
    if atomal {
        v.push(AeaMarking::Atomal(AtomalBlock));
    }
    AeaSet::from_markings(&v)
}

fn arb_primary() -> impl Strategy<Value = Option<AeaPrimary>> {
    prop_oneof![
        Just(None),
        Just(Some(AeaPrimary::Tfni)),
        Just(Some(AeaPrimary::Frd)),
        Just(Some(AeaPrimary::Rd)),
    ]
}

fn arb_ucni() -> impl Strategy<Value = BTreeSet<UcniKind>> {
    proptest::collection::btree_set(
        prop_oneof![Just(UcniKind::DodUcni), Just(UcniKind::DoeUcni)],
        0..3,
    )
}

proptest! {
    /// Componentwise `Lattice` laws on the full `AeaSet` `Product`.
    /// If every sub-axis is a lattice, the Product is a lattice
    /// (standard universal-algebra fact); this test pins it for the
    /// CAPCO `AeaSet` so a future refactor that breaks one sub-axis
    /// trips the harness.
    #[test]
    fn aea_set_join_assoc_comm_idem(
        primary_a in arb_primary(),
        cnwdi_a in any::<bool>(),
        sigmas_a in proptest::collection::btree_set(14u8..=20, 0..4),
        ucni_a in arb_ucni(),
        atomal_a in any::<bool>(),
        primary_b in arb_primary(),
        cnwdi_b in any::<bool>(),
        sigmas_b in proptest::collection::btree_set(14u8..=20, 0..4),
        ucni_b in arb_ucni(),
        atomal_b in any::<bool>(),
        primary_c in arb_primary(),
        cnwdi_c in any::<bool>(),
        sigmas_c in proptest::collection::btree_set(14u8..=20, 0..4),
        ucni_c in arb_ucni(),
        atomal_c in any::<bool>(),
    ) {
        let a = mk_aea(primary_a, cnwdi_a, &sigmas_a, &ucni_a, atomal_a);
        let b = mk_aea(primary_b, cnwdi_b, &sigmas_b, &ucni_b, atomal_b);
        let c = mk_aea(primary_c, cnwdi_c, &sigmas_c, &ucni_c, atomal_c);

        // Commutativity.
        prop_assert_eq!(a.join(&b), b.join(&a));
        // Associativity.
        prop_assert_eq!(a.join(&b).join(&c), a.join(&b.join(&c)));
        // Idempotency.
        prop_assert_eq!(a.join(&a), a.clone());
    }
}

/// `AeaSet::default()` is the identity for `join` — the
/// `BoundedLattice::bottom` law applied to the unbounded `AeaSet`.
#[test]
fn aea_set_identity_with_default() {
    let bottom = AeaSet::default();
    let rich = AeaSet::from_markings(&[
        AeaMarking::Rd(RdBlock {
            cnwdi: true,
            sigma: Box::new([14, 18]),
        }),
        AeaMarking::DodUcni,
        AeaMarking::Atomal(AtomalBlock),
    ]);
    assert_eq!(bottom.join(&rich), rich);
    assert_eq!(rich.join(&bottom), rich);
    assert!(bottom.is_empty());
    assert!(!rich.is_empty());
}

// ===========================================================================
// AeaSet — `Lattice::meet` laws
// ===========================================================================
//
// `Lattice` requires meet to be commutative, associative, idempotent, and to
// absorb against join: `a.meet(a.join(&b)) == a` and `a.join(a.meet(&b)) == a`.
// The PR 4b-A `AeaSet::meet` impl was not exercised by the original test
// suite; Copilot review (PR #426) flagged the gap. These tests pin the meet
// algebra component-wise per sub-axis + the Product-level laws.

/// `AeaSet::meet` is commutative and idempotent over the primary axis
/// (SupersessionSet meet is the *min* under `Tfni ⊏ Frd ⊏ Rd`).
#[test]
fn aea_primary_supersession_meet_assoc_comm_idem() {
    let rd = AeaSet::from_markings(&[AeaMarking::Rd(RdBlock::default())]);
    let frd = AeaSet::from_markings(&[AeaMarking::Frd(FrdBlock::default())]);
    let tfni = AeaSet::from_markings(&[AeaMarking::Tfni]);
    let bottom = AeaSet::default();

    // Commutativity.
    assert_eq!(rd.meet(&frd), frd.meet(&rd));
    assert_eq!(frd.meet(&tfni), tfni.meet(&frd));
    assert_eq!(rd.meet(&tfni), tfni.meet(&rd));

    // Associativity.
    assert_eq!(rd.meet(&frd).meet(&tfni), rd.meet(&frd.meet(&tfni)));

    // Idempotency.
    assert_eq!(rd.meet(&rd), rd);
    assert_eq!(frd.meet(&frd), frd);
    assert_eq!(tfni.meet(&tfni), tfni);

    // Meet with bottom is bottom (bottom is the meet absorber).
    assert_eq!(rd.meet(&bottom), bottom);
    assert_eq!(bottom.meet(&frd), bottom);

    // Meet matches the §H.6 p104 supersession-min: RD ⊓ FRD = FRD;
    // RD ⊓ TFNI = TFNI; FRD ⊓ TFNI = TFNI.
    assert_eq!(rd.meet(&frd).primary(), Some(AeaPrimary::Frd));
    assert_eq!(rd.meet(&tfni).primary(), Some(AeaPrimary::Tfni));
    assert_eq!(frd.meet(&tfni).primary(), Some(AeaPrimary::Tfni));
}

/// `AeaSet::meet` on the SIGMA FlatSet axis is set-intersection.
#[test]
fn aea_sigma_flatset_meet_intersect() {
    let s14_18 = AeaSet::from_markings(&[AeaMarking::Rd(RdBlock {
        cnwdi: false,
        sigma: Box::new([14, 18]),
    })]);
    let s18_20 = AeaSet::from_markings(&[AeaMarking::Rd(RdBlock {
        cnwdi: false,
        sigma: Box::new([18, 20]),
    })]);

    let intersect: Vec<u8> = s14_18.meet(&s18_20).sigmas().iter().copied().collect();
    assert_eq!(intersect, vec![18]);

    // Commutativity.
    assert_eq!(s14_18.meet(&s18_20), s18_20.meet(&s14_18));

    // Idempotency.
    assert_eq!(s14_18.meet(&s14_18), s14_18);
}

/// `AeaSet::meet` on the UCNI FlatSet axis is set-intersection.
#[test]
fn aea_ucni_flatset_meet_intersect() {
    let dod = AeaSet::from_markings(&[AeaMarking::DodUcni]);
    let doe = AeaSet::from_markings(&[AeaMarking::DoeUcni]);
    let both = AeaSet::from_markings(&[AeaMarking::DodUcni, AeaMarking::DoeUcni]);

    // Disjoint single-element sets meet to empty.
    assert!(dod.meet(&doe).ucni().is_empty());

    // {Dod} ⊓ {Dod, Doe} = {Dod}.
    assert_eq!(dod.meet(&both).ucni().len(), 1);
    assert!(dod.meet(&both).ucni().contains(&UcniKind::DodUcni));

    // Commutativity.
    assert_eq!(dod.meet(&doe), doe.meet(&dod));

    // Idempotency.
    assert_eq!(both.meet(&both), both);
}

/// `AeaSet::meet` on the ATOMAL OptionalSingleton axis is `Option::and`.
#[test]
fn aea_atomal_optional_singleton_meet() {
    let atomal = AeaSet::from_markings(&[AeaMarking::Atomal(AtomalBlock)]);
    let bottom = AeaSet::default();

    // Some ⊓ Some = Some (AtomalBlock is unit; all Some are equal).
    assert!(atomal.meet(&atomal).atomal().is_some());

    // Some ⊓ None = None.
    assert!(atomal.meet(&bottom).atomal().is_none());
    assert!(bottom.meet(&atomal).atomal().is_none());

    // Commutativity.
    assert_eq!(atomal.meet(&bottom), bottom.meet(&atomal));
}

/// Product-level meet absorption against join: `a ⊓ (a ⊔ b) = a` and
/// `a ⊔ (a ⊓ b) = a`. These are the two absorption laws every lattice must
/// satisfy.
#[test]
fn aea_set_meet_join_absorption() {
    let a = AeaSet::from_markings(&[AeaMarking::Rd(RdBlock {
        cnwdi: true,
        sigma: Box::new([14]),
    })]);
    let b = AeaSet::from_markings(&[
        AeaMarking::Frd(FrdBlock {
            sigma: Box::new([18]),
        }),
        AeaMarking::DodUcni,
    ]);

    // Absorption: a ⊓ (a ⊔ b) = a.
    let a_join_b = a.join(&b);
    assert_eq!(a.meet(&a_join_b), a);

    // Absorption: a ⊔ (a ⊓ b) = a.
    let a_meet_b = a.meet(&b);
    assert_eq!(a.join(&a_meet_b), a);
}

// ===========================================================================
// PR 4b-B Commit 3 — ClassificationLattice
// ===========================================================================
// CAPCO-2016 §H.1 pp47-54 (US class chain) + §H.7 pp123-125 (reciprocal-
// classification rule). Verified 2026-05-15 against CAPCO-2016.md.

mod classification_lattice {
    use marque_capco::ClassificationLattice;
    use marque_ism::{Classification, MarkingClassification};
    use marque_scheme::{BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice};

    fn lvl(c: Classification) -> ClassificationLattice {
        ClassificationLattice::new(Some(MarkingClassification::Us(c)))
    }

    // H-5 (PR 4b-B follow-up): include `Restricted` so the
    // five-level chain (U < R < C < S < TS) is exercised end-to-end.
    // `Restricted` is the US equivalent of NATO `NR` per
    // `NatoClassification::us_equivalent()` and was previously
    // omitted from the test sweep.
    const ALL: [Classification; 5] = [
        Classification::Unclassified,
        Classification::Restricted,
        Classification::Confidential,
        Classification::Secret,
        Classification::TopSecret,
    ];

    #[test]
    fn classification_chain_assoc_comm_idem() {
        let bottom = ClassificationLattice::empty();
        for a in ALL {
            for b in ALL {
                let la = lvl(a);
                let lb = lvl(b);
                // Commutativity.
                assert_eq!(la.join(&lb), lb.join(&la), "comm: {a:?} vs {b:?}");
                // Idempotency.
                assert_eq!(la.join(&la), la, "idem");
                for c in ALL {
                    let lc = lvl(c);
                    // Associativity.
                    assert_eq!(
                        la.join(&lb).join(&lc),
                        la.join(&lb.join(&lc)),
                        "assoc: ({a:?},{b:?},{c:?})"
                    );
                }
            }
            // Identity with bottom.
            let la = lvl(a);
            assert_eq!(bottom.join(&la), la);
            assert_eq!(la.join(&bottom), la);
        }
    }

    #[test]
    fn classification_top_absorbs() {
        let top = ClassificationLattice::top();
        for a in ALL {
            let la = lvl(a);
            assert_eq!(top.join(&la), top, "top absorbs join");
            assert_eq!(la.meet(&top), la, "top is meet-identity");
        }
    }

    #[test]
    fn classification_join_picks_higher_us_chain() {
        assert_eq!(
            lvl(Classification::Confidential).join(&lvl(Classification::TopSecret)),
            lvl(Classification::TopSecret)
        );
        assert_eq!(
            lvl(Classification::Secret).join(&lvl(Classification::Unclassified)),
            lvl(Classification::Secret)
        );
    }

    #[test]
    fn classification_preserves_nato_variant_when_higher() {
        // NATO CTS ≥ US TS in the §H.7 reciprocal lattice; join
        // should keep the NATO variant if it's at the higher level.
        // (Reality: NATO classifications get reciprocal-normalized at
        // portion-parse time, so this is a defense-in-depth check on
        // the lattice itself when fed un-normalized inputs.)
        let us_secret = lvl(Classification::Secret);
        let nato_cts = ClassificationLattice::new(Some(MarkingClassification::Nato(
            marque_ism::NatoClassification::CosmicTopSecret,
        )));
        let joined = us_secret.join(&nato_cts);
        // CTS effective_level == TopSecret > Secret, so NATO variant
        // wins. Variant preservation is the key property here.
        assert_eq!(joined, nato_cts);
    }

    // -----------------------------------------------------------------------
    // PR 4b-B follow-up — C-1 commutativity tiebreak across equal-level
    // variants. The original PR 4b-B impl always returned the left
    // operand on equal level, breaking commutativity. This regression
    // suite exhausts the cross-product of `{Us, Fgi, Nato, Joint}` at
    // every level and asserts `a.join(b) == b.join(a)`.
    // -----------------------------------------------------------------------
    fn arb_classification_variant(level: Classification) -> Vec<ClassificationLattice> {
        use marque_ism::{CountryCode, FgiClassification, JointClassification, NatoClassification};
        let usa = CountryCode::try_new(b"USA").expect("USA");
        let gbr = CountryCode::try_new(b"GBR").expect("GBR");
        let can = CountryCode::try_new(b"CAN").expect("CAN");
        let fra = CountryCode::try_new(b"FRA").expect("FRA");
        // Pair a few representative variants at the same effective
        // level. NATO uses `us_equivalent`; pick the variant whose
        // us_equivalent matches `level` for the five-level chain
        // (U / R / C / S / TS — H-5 PR 4b-B follow-up adds R).
        //
        // C-7 / H-7 (PR 4b-B follow-up): include multiple distinct
        // payloads at the same variant-rank/same-level so commutativity
        // is exercised on the payload tiebreaker as well as the
        // variant tiebreaker. Pre-C-7 the join fell through `ra <= rb`
        // returning the left operand on same-variant/same-level —
        // non-commutative.
        let nato = match level {
            Classification::TopSecret => Some(NatoClassification::CosmicTopSecret),
            Classification::Secret => Some(NatoClassification::NatoSecret),
            Classification::Confidential => Some(NatoClassification::NatoConfidential),
            Classification::Restricted => Some(NatoClassification::NatoRestricted),
            Classification::Unclassified => Some(NatoClassification::NatoUnclassified),
        };
        let mut out = vec![
            // Us — only one payload (the level itself).
            ClassificationLattice::new(Some(MarkingClassification::Us(level))),
            // Fgi — two payloads with different country lists at same
            // level. Pre-C-7 these joined non-commutatively.
            ClassificationLattice::new(Some(MarkingClassification::Fgi(FgiClassification {
                level,
                countries: Box::new([gbr]),
            }))),
            ClassificationLattice::new(Some(MarkingClassification::Fgi(FgiClassification {
                level,
                countries: Box::new([can]),
            }))),
            ClassificationLattice::new(Some(MarkingClassification::Fgi(FgiClassification {
                level,
                countries: Box::new([can, gbr]),
            }))),
            // Joint — two payloads with different co-owner lists at
            // same level.
            ClassificationLattice::new(Some(MarkingClassification::Joint(JointClassification {
                level,
                countries: Box::new([usa, gbr]),
            }))),
            ClassificationLattice::new(Some(MarkingClassification::Joint(JointClassification {
                level,
                countries: Box::new([usa, can]),
            }))),
            ClassificationLattice::new(Some(MarkingClassification::Joint(JointClassification {
                level,
                countries: Box::new([usa, can, fra]),
            }))),
        ];
        if let Some(n) = nato {
            out.push(ClassificationLattice::new(Some(
                MarkingClassification::Nato(n),
            )));
        }
        out
    }

    #[test]
    fn classification_join_commutative_across_variants() {
        // C-1 regression: at each effective level, every pair of
        // distinct-variant classifications must commute under join.
        for level in ALL {
            let variants = arb_classification_variant(level);
            for a in &variants {
                for b in &variants {
                    assert_eq!(
                        a.join(b),
                        b.join(a),
                        "C-1: join not commutative at level {level:?}: \
                         {a:?} vs {b:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn classification_meet_commutative_across_variants() {
        // Companion check: the same tiebreak applies to `meet` so
        // both ops stay commutative and consistent.
        for level in ALL {
            let variants = arb_classification_variant(level);
            for a in &variants {
                for b in &variants {
                    assert_eq!(
                        a.meet(b),
                        b.meet(a),
                        "C-1: meet not commutative at level {level:?}: \
                         {a:?} vs {b:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn classification_us_wins_equal_level_tiebreak() {
        // C-1: US is the canonical variant per §H.7 reciprocal
        // normalization. At equal effective level, joining Us with
        // any other variant produces Us.
        use marque_ism::{CountryCode, FgiClassification, JointClassification, NatoClassification};
        let gbr = CountryCode::try_new(b"GBR").expect("GBR");
        let usa = CountryCode::try_new(b"USA").expect("USA");
        let us = lvl(Classification::Secret);
        let fgi = ClassificationLattice::new(Some(MarkingClassification::Fgi(FgiClassification {
            level: Classification::Secret,
            countries: Box::new([gbr]),
        })));
        let nato = ClassificationLattice::new(Some(MarkingClassification::Nato(
            NatoClassification::NatoSecret,
        )));
        let joint =
            ClassificationLattice::new(Some(MarkingClassification::Joint(JointClassification {
                level: Classification::Secret,
                countries: Box::new([usa, gbr]),
            })));
        assert_eq!(us.join(&fgi), us);
        assert_eq!(fgi.join(&us), us);
        assert_eq!(us.join(&nato), us);
        assert_eq!(nato.join(&us), us);
        assert_eq!(us.join(&joint), us);
        assert_eq!(joint.join(&us), us);
    }

    // -----------------------------------------------------------------------
    // C-9 (PR 4b-B follow-up) — absorption laws across the cross-product
    // of equal-level variants AND across the partial order on payloads.
    //
    // Absorption pair: `a ⊔ (a ⊓ b) = a` and `a ⊓ (a ⊔ b) = a`.
    //
    // **Background (pre-C-9 history; M-24 PR 4b-B 7th-pass — wording
    // amended to remove the misleading "different variants are
    // incomparable" framing).** The C-7 fix introduced a same-
    // variant-payload union tiebreaker on both `join` and `meet`.
    // That broke absorption at equal level on same-variant payload
    // diffs: `meet` should NOT return the UNION — otherwise
    // `a.join(a.meet(b)) = union(a, b) ≠ a` for the operand whose
    // payload was a strict subset.
    //
    // The C-9 fix (verified against `lattice.rs::ClassificationLattice::meet`):
    // - Different variants at same level are NOT incomparable — they
    //   are linearly ordered by `classification_variant_rank`
    //   (`Us < Fgi < Nato < Joint < Conflict`); `meet` returns the
    //   HIGHER-rank operand (the dominated, lower-≤ side; the GLB
    //   dual of `join`'s "lower variant rank wins" tiebreaker).
    //   §H.7 pp123-125 reciprocal-normalization grounds the rank
    //   ordering. Returning `bottom` here would break the dual
    //   absorption law for the higher-rank operand — see
    //   `classification_meet_different_variants_returns_dominated`
    //   below for the spot-check.
    // - Same variant, payload subset (one operand's countries ⊆ the
    //   other's) → `meet` returns the smaller payload (the GLB on the
    //   country-list partial order).
    // - Same variant, disjoint payloads → `meet` returns the lattice
    //   bottom (`Self(None)`; no common subset). This is the ONLY
    //   path that returns `empty()` from `meet` at same-level inputs.
    //
    // **C-9b extension (PR 4b-B 7th-pass).** The same asymmetry
    // existed inside `meet_foreign_classification` for `Conflict`-
    // `Conflict` cross-variant inner foreign payloads — fixed to
    // return the higher-rank inner. See the
    // `classification_conflict_cross_variant_inner_*` tests below.
    //
    // §-authority (verified 2026-05-15 against CAPCO-2016.md):
    // §H.7 pp123-125 (reciprocal normalization, variant-rank order) +
    // §H.1 pp47-54 (US class chain).
    // -----------------------------------------------------------------------
    #[test]
    fn classification_lattice_absorption_across_variants_and_payloads() {
        // Build a small set of representative inputs at one level so
        // the cross-product stays tractable but still exercises:
        //   - different variants at same level
        //   - same variant, payload subset
        //   - same variant, disjoint payloads
        //   - bottom and top against everything
        //
        // Country payloads are pre-sorted (alphabetical) because the
        // `classification_join_same_variant` helper canonicalizes via
        // BTreeSet on cross-payload merges; absorption holds at the
        // structural-equality level only when the inputs are already
        // in the canonical order. Production code (parser, page-
        // context roll-up) emits sorted lists by §H.8 p150-151 / §H.3
        // p56 (REL TO / JOINT alphabetical-with-USA-first).
        use marque_ism::{
            CountryCode, FgiClassification, ForeignClassification, JointClassification,
            NatoClassification,
        };
        let usa = CountryCode::try_new(b"USA").expect("USA");
        let gbr = CountryCode::try_new(b"GBR").expect("GBR");
        let can = CountryCode::try_new(b"CAN").expect("CAN");
        let bottom = ClassificationLattice::empty();
        let top = ClassificationLattice::top();
        let inputs: Vec<ClassificationLattice> = vec![
            bottom.clone(),
            top.clone(),
            // US at multiple levels.
            lvl(Classification::Unclassified),
            lvl(Classification::Confidential),
            lvl(Classification::Secret),
            // FGI same level, different payloads (subset + disjoint).
            ClassificationLattice::new(Some(MarkingClassification::Fgi(FgiClassification {
                level: Classification::Secret,
                countries: Box::new([gbr]),
            }))),
            ClassificationLattice::new(Some(MarkingClassification::Fgi(FgiClassification {
                level: Classification::Secret,
                countries: Box::new([can]),
            }))),
            ClassificationLattice::new(Some(MarkingClassification::Fgi(FgiClassification {
                level: Classification::Secret,
                countries: Box::new([can, gbr]),
            }))),
            // NATO same level (variant-only mismatch with FGI/US).
            ClassificationLattice::new(Some(MarkingClassification::Nato(
                NatoClassification::NatoSecret,
            ))),
            // JOINT same level, different payloads. Sorted alphabetical.
            ClassificationLattice::new(Some(MarkingClassification::Joint(JointClassification {
                level: Classification::Secret,
                countries: Box::new([gbr, usa]),
            }))),
            ClassificationLattice::new(Some(MarkingClassification::Joint(JointClassification {
                level: Classification::Secret,
                countries: Box::new([can, usa]),
            }))),
            // C-9b: Conflict variants with cross-variant inner foreign
            // payloads exercise the C-9b dual-absorption fix on
            // `meet_foreign_classification`.
            ClassificationLattice::new(Some(MarkingClassification::Conflict {
                us: Classification::Secret,
                foreign: Box::new(ForeignClassification::Fgi(FgiClassification {
                    level: Classification::Secret,
                    countries: Box::new([gbr]),
                })),
            })),
            ClassificationLattice::new(Some(MarkingClassification::Conflict {
                us: Classification::Secret,
                foreign: Box::new(ForeignClassification::Nato(NatoClassification::NatoSecret)),
            })),
            ClassificationLattice::new(Some(MarkingClassification::Conflict {
                us: Classification::Secret,
                foreign: Box::new(ForeignClassification::Joint(JointClassification {
                    level: Classification::Secret,
                    countries: Box::new([can, usa]),
                })),
            })),
        ];
        for a in &inputs {
            for b in &inputs {
                let a_meet_b = a.meet(b);
                let a_join_b = a.join(b);
                assert_eq!(
                    a.join(&a_meet_b),
                    *a,
                    "C-9: a ⊔ (a ⊓ b) ≠ a for a={a:?}, b={b:?}, \
                     a⊓b={a_meet_b:?}"
                );
                assert_eq!(
                    a.meet(&a_join_b),
                    *a,
                    "C-9: a ⊓ (a ⊔ b) ≠ a for a={a:?}, b={b:?}, \
                     a⊔b={a_join_b:?}"
                );
            }
        }
    }

    // C-9 spot-checks: the user-cited counterexamples from the triage,
    // pinned individually so a regression names them in test output.
    //
    // Analysis correction vs. the triage description: at same level,
    // different variants are NOT incomparable — they are linearly
    // ordered by the variant-rank join policy (Us < Fgi < Nato <
    // Joint < Conflict, where lower rank wins join → lower rank is
    // GREATER in the ≤ order). The meet of Fgi(S,[GBR]) and Us(S) is
    // therefore Fgi(S,[GBR]) (the dominated, lower-≤ variant), NOT
    // bottom. Returning bottom would break the dual absorption law
    // `a.meet(a.join(b)) = a` for the higher-rank operand. §H.7
    // pp123-125 reciprocal-normalization implicitly defines this
    // order on variant precedence.
    #[test]
    fn classification_meet_different_variants_returns_dominated() {
        // Fgi(S, [GBR]) ⊓ Us(S) → Fgi(S, [GBR]) (the dominated variant).
        use marque_ism::{CountryCode, FgiClassification};
        let gbr = CountryCode::try_new(b"GBR").expect("GBR");
        let fgi_gbr =
            ClassificationLattice::new(Some(MarkingClassification::Fgi(FgiClassification {
                level: Classification::Secret,
                countries: Box::new([gbr]),
            })));
        let us_s = lvl(Classification::Secret);
        let meet = fgi_gbr.meet(&us_s);
        assert_eq!(
            meet, fgi_gbr,
            "C-9: meet at cross-variant same-level returns dominated"
        );
        // Symmetric: order shouldn't matter.
        assert_eq!(us_s.meet(&fgi_gbr), fgi_gbr);
        // Absorption: a ⊔ (a ⊓ b) = a.
        assert_eq!(fgi_gbr.join(&meet), fgi_gbr);
        // Dual: a ⊓ (a ⊔ b) = a.
        assert_eq!(fgi_gbr.meet(&fgi_gbr.join(&us_s)), fgi_gbr);
    }

    #[test]
    fn classification_meet_same_variant_disjoint_payloads_returns_bottom() {
        // Fgi(S, [GBR]) ⊓ Fgi(S, [CAN]) → bottom (no common subset).
        use marque_ism::{CountryCode, FgiClassification};
        let gbr = CountryCode::try_new(b"GBR").expect("GBR");
        let can = CountryCode::try_new(b"CAN").expect("CAN");
        let fgi_gbr =
            ClassificationLattice::new(Some(MarkingClassification::Fgi(FgiClassification {
                level: Classification::Secret,
                countries: Box::new([gbr]),
            })));
        let fgi_can =
            ClassificationLattice::new(Some(MarkingClassification::Fgi(FgiClassification {
                level: Classification::Secret,
                countries: Box::new([can]),
            })));
        let meet = fgi_gbr.meet(&fgi_can);
        assert_eq!(
            meet,
            ClassificationLattice::empty(),
            "C-9: meet of same-variant disjoint payloads must be bottom"
        );
        assert_eq!(fgi_gbr.join(&meet), fgi_gbr);
        assert_eq!(fgi_can.join(&meet), fgi_can);
    }

    #[test]
    fn classification_meet_same_variant_payload_subset_returns_smaller() {
        // Fgi(S, [GBR, CAN]) ⊓ Fgi(S, [GBR]) → Fgi(S, [GBR])
        // (the smaller set is the GLB on the country partial order).
        use marque_ism::{CountryCode, FgiClassification};
        let gbr = CountryCode::try_new(b"GBR").expect("GBR");
        let can = CountryCode::try_new(b"CAN").expect("CAN");
        let fgi_both =
            ClassificationLattice::new(Some(MarkingClassification::Fgi(FgiClassification {
                level: Classification::Secret,
                countries: Box::new([can, gbr]),
            })));
        let fgi_gbr =
            ClassificationLattice::new(Some(MarkingClassification::Fgi(FgiClassification {
                level: Classification::Secret,
                countries: Box::new([gbr]),
            })));
        let meet = fgi_both.meet(&fgi_gbr);
        assert_eq!(meet, fgi_gbr, "C-9: meet picks smaller payload on subset");
        // Symmetric: order shouldn't matter.
        assert_eq!(fgi_gbr.meet(&fgi_both), fgi_gbr);
        // Absorption.
        assert_eq!(fgi_both.join(&meet), fgi_both);
        assert_eq!(fgi_gbr.join(&meet), fgi_gbr);
    }

    // -----------------------------------------------------------------------
    // C-9b (PR 4b-B 7th-pass follow-up) — Conflict cross-variant inner
    // foreign-classification absorption.
    //
    // Continuation of C-9. Two `Conflict` values at the same outer level
    // with different `foreign` inner variants (e.g. one with
    // `Nato(NS)` inner, another with `Fgi(S, [GBR])` inner) trigger the
    // `Conflict-Conflict` arm of `classification_join_same_variant` /
    // `classification_meet_same_variant`. Those arms delegate to
    // `merge_foreign_classification` / `meet_foreign_classification`,
    // which were ASYMMETRIC pre-C-9b:
    //
    //   - `merge_foreign_classification` cross-variant: returns the
    //     lower-rank variant (Fgi=1 < Nato=2 < Joint=3).
    //   - `meet_foreign_classification` cross-variant: returned `None`,
    //     which the outer `classification_meet_same_variant` translated
    //     to the lattice bottom.
    //
    // That asymmetry broke the dual absorption law `a ⊓ (a ⊔ b) = a` for
    // the operand whose inner `foreign` was the LOWER-rank one (Fgi
    // wins join → Fgi = `a ⊔ b`; then `a.meet(b) = bottom`; so the join
    // direction gives `b`, but the meet direction gives bottom, which
    // joined back with `a` gives `a` ✓ — but `a.meet(a.join(b))` =
    // `a.meet(b)` (since `a ⊔ b = b` for the higher-rank operand),
    // which = bottom, NOT `a`).
    //
    // Fix: align `meet_foreign_classification` cross-variant with
    // `merge_foreign_classification`'s tiebreak — return the HIGHER-rank
    // operand (the dominated, lower-≤ side; the GLB dual). This makes
    // the inner foreign axis its own linear-ordered tiebreak that
    // satisfies absorption, mirroring the C-9 fix at the outer
    // classification level.
    //
    // §-authority: §H.7 pp123-125 reciprocal-normalization (variant-rank
    // order). Verified 2026-05-15 against CAPCO-2016.md.
    // -----------------------------------------------------------------------

    fn conflict(
        level: Classification,
        foreign: marque_ism::ForeignClassification,
    ) -> ClassificationLattice {
        ClassificationLattice::new(Some(MarkingClassification::Conflict {
            us: level,
            foreign: Box::new(foreign),
        }))
    }

    #[test]
    fn classification_conflict_cross_variant_inner_absorption() {
        // a = Conflict{us=S, foreign: Nato(NS)}
        // b = Conflict{us=S, foreign: Fgi(S, [GBR])}
        // Same outer level. Inner variant ranks: Fgi=1, Nato=2.
        //
        // - merge_foreign_classification(Nato, Fgi): rank(Nato)=2,
        //   rank(Fgi)=1; rank(a)<=rank(b) is `2<=1` = false → returns
        //   Fgi. So `a.join(b) = Conflict{foreign: Fgi}` = b.
        // - meet_foreign_classification(Nato, Fgi) post-C-9b: returns
        //   the higher-rank inner (the lower-≤ side; GLB dual) = Nato.
        //   So `a.meet(b) = Conflict{foreign: Nato}` = a.
        //
        // Absorption checks:
        //   - a ⊔ (a ⊓ b) = a ⊔ a = a ✓
        //   - a ⊓ (a ⊔ b) = a ⊓ b = a ✓ (this is the C-9b fix)
        //   - b ⊔ (b ⊓ a) = b ⊔ a = b ✓
        //   - b ⊓ (b ⊔ a) = b ⊓ b = b ✓
        use marque_ism::{
            CountryCode, FgiClassification, ForeignClassification, NatoClassification,
        };
        let gbr = CountryCode::try_new(b"GBR").expect("GBR");
        let a = conflict(
            Classification::Secret,
            ForeignClassification::Nato(NatoClassification::NatoSecret),
        );
        let b = conflict(
            Classification::Secret,
            ForeignClassification::Fgi(FgiClassification {
                level: Classification::Secret,
                countries: Box::new([gbr]),
            }),
        );

        // Sanity: a and b are at the same outer level so the
        // same-level tiebreak path activates.
        let a_join_b = a.join(&b);
        let b_join_a = b.join(&a);
        assert_eq!(a_join_b, b_join_a, "C-9b: join must be commutative");

        let a_meet_b = a.meet(&b);
        let b_meet_a = b.meet(&a);
        assert_eq!(a_meet_b, b_meet_a, "C-9b: meet must be commutative");

        // Absorption — both directions.
        assert_eq!(a.join(&a_meet_b), a, "C-9b: a ⊔ (a ⊓ b) = a");
        assert_eq!(a.meet(&a_join_b), a, "C-9b: a ⊓ (a ⊔ b) = a");
        assert_eq!(b.join(&b_meet_a), b, "C-9b: b ⊔ (b ⊓ a) = b");
        assert_eq!(b.meet(&b_join_a), b, "C-9b: b ⊓ (b ⊔ a) = b");
    }

    #[test]
    fn classification_conflict_cross_variant_inner_with_joint() {
        // a = Conflict{us=S, foreign: Joint(S, [USA, CAN])}
        // b = Conflict{us=S, foreign: Fgi(S, [GBR])}
        // Inner ranks: Fgi=1, Joint=3. Same shape as Nato/Fgi case.
        use marque_ism::{
            CountryCode, FgiClassification, ForeignClassification, JointClassification,
        };
        let usa = CountryCode::try_new(b"USA").expect("USA");
        let can = CountryCode::try_new(b"CAN").expect("CAN");
        let gbr = CountryCode::try_new(b"GBR").expect("GBR");
        let a = conflict(
            Classification::Secret,
            ForeignClassification::Joint(JointClassification {
                level: Classification::Secret,
                countries: Box::new([can, usa]),
            }),
        );
        let b = conflict(
            Classification::Secret,
            ForeignClassification::Fgi(FgiClassification {
                level: Classification::Secret,
                countries: Box::new([gbr]),
            }),
        );

        let a_join_b = a.join(&b);
        let a_meet_b = a.meet(&b);
        assert_eq!(b.join(&a), a_join_b, "comm join");
        assert_eq!(b.meet(&a), a_meet_b, "comm meet");
        // Absorption.
        assert_eq!(a.join(&a_meet_b), a, "C-9b: a ⊔ (a ⊓ b) = a");
        assert_eq!(a.meet(&a_join_b), a, "C-9b: a ⊓ (a ⊔ b) = a");
        assert_eq!(b.join(&b.meet(&a)), b, "C-9b: b ⊔ (b ⊓ a) = b");
        assert_eq!(b.meet(&b.join(&a)), b, "C-9b: b ⊓ (b ⊔ a) = b");
    }

    #[test]
    fn classification_conflict_cross_variant_inner_full_cube() {
        // Exhaustive cross-product over the three inner-foreign variants.
        // Every pair must satisfy commutativity + dual absorption.
        use marque_ism::{
            CountryCode, FgiClassification, ForeignClassification, JointClassification,
            NatoClassification,
        };
        let usa = CountryCode::try_new(b"USA").expect("USA");
        let can = CountryCode::try_new(b"CAN").expect("CAN");
        let gbr = CountryCode::try_new(b"GBR").expect("GBR");
        let inputs: Vec<ClassificationLattice> = vec![
            conflict(
                Classification::Secret,
                ForeignClassification::Fgi(FgiClassification {
                    level: Classification::Secret,
                    countries: Box::new([gbr]),
                }),
            ),
            conflict(
                Classification::Secret,
                ForeignClassification::Fgi(FgiClassification {
                    level: Classification::Secret,
                    countries: Box::new([can]),
                }),
            ),
            conflict(
                Classification::Secret,
                ForeignClassification::Nato(NatoClassification::NatoSecret),
            ),
            conflict(
                Classification::Secret,
                ForeignClassification::Joint(JointClassification {
                    level: Classification::Secret,
                    countries: Box::new([can, usa]),
                }),
            ),
        ];
        for a in &inputs {
            for b in &inputs {
                assert_eq!(a.join(b), b.join(a), "C-9b: join commutativity");
                assert_eq!(a.meet(b), b.meet(a), "C-9b: meet commutativity");
                assert_eq!(
                    a.join(&a.meet(b)),
                    *a,
                    "C-9b: a ⊔ (a ⊓ b) ≠ a for a={a:?}, b={b:?}"
                );
                assert_eq!(
                    a.meet(&a.join(b)),
                    *a,
                    "C-9b: a ⊓ (a ⊔ b) ≠ a for a={a:?}, b={b:?}"
                );
            }
        }
    }
}

// ===========================================================================
// PR 4b-B Commit 3 — NatoClassLattice
// ===========================================================================
// CAPCO-2016 §H.2 p55. Verified 2026-05-15.

mod nato_class_lattice {
    use marque_capco::NatoClassLattice;
    use marque_ism::NatoClassification;
    use marque_scheme::{BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice};

    const ALL: [NatoClassification; 5] = [
        NatoClassification::NatoUnclassified,
        NatoClassification::NatoRestricted,
        NatoClassification::NatoConfidential,
        NatoClassification::NatoSecret,
        NatoClassification::CosmicTopSecret,
    ];

    fn n(c: NatoClassification) -> NatoClassLattice {
        NatoClassLattice::new(Some(c))
    }

    #[test]
    fn nato_chain_assoc_comm_idem() {
        let bottom = NatoClassLattice::empty();
        for a in ALL {
            for b in ALL {
                let la = n(a);
                let lb = n(b);
                assert_eq!(la.join(&lb), lb.join(&la), "comm");
                assert_eq!(la.join(&la), la, "idem");
                for c in ALL {
                    let lc = n(c);
                    assert_eq!(la.join(&lb).join(&lc), la.join(&lb.join(&lc)), "assoc");
                }
            }
            assert_eq!(bottom.join(&n(a)), n(a));
            assert_eq!(n(a).join(&bottom), n(a));
        }
    }

    #[test]
    fn nato_top_absorbs() {
        let top = NatoClassLattice::top();
        for a in ALL {
            assert_eq!(top.join(&n(a)), top);
            assert_eq!(n(a).meet(&top), n(a));
        }
    }

    #[test]
    fn nato_absorption() {
        for a in ALL {
            for b in ALL {
                let la = n(a);
                let lb = n(b);
                assert_eq!(la.meet(&la.join(&lb)), la, "a ⊓ (a ⊔ b) = a");
                assert_eq!(la.join(&la.meet(&lb)), la, "a ⊔ (a ⊓ b) = a");
            }
        }
    }
}

// ===========================================================================
// PR 4b-B Commit 3 — DeclassifyOnLattice
// ===========================================================================
// CAPCO-2016 §H.6 p104 (most-restrictive date wins). Verified 2026-05-15.

mod declassify_on_lattice {
    use marque_capco::DeclassifyOnLattice;
    use marque_ism::IsmDate;
    use marque_scheme::{JoinSemilattice, MeetSemilattice};

    fn d(y: i32, m: u8, day: u8) -> DeclassifyOnLattice {
        DeclassifyOnLattice::new(Some(IsmDate::Date(y, m, day)))
    }
    fn y(year: i32) -> DeclassifyOnLattice {
        DeclassifyOnLattice::new(Some(IsmDate::Year(year)))
    }
    fn bottom() -> DeclassifyOnLattice {
        DeclassifyOnLattice::empty()
    }

    #[test]
    fn declassify_on_max_assoc_comm_idem() {
        let a = d(2030, 6, 15);
        let b = d(2030, 12, 1);
        let c = y(2031);
        assert_eq!(a.join(&b), b.join(&a), "comm");
        assert_eq!(a.join(&b).join(&c), a.join(&b.join(&c)), "assoc");
        assert_eq!(a.join(&a), a, "idem");
        assert_eq!(bottom().join(&a), a, "bottom-identity");
        assert_eq!(a.join(&bottom()), a, "bottom-identity (right)");
    }

    #[test]
    fn declassify_on_join_picks_furthest_out() {
        let earlier = d(2025, 1, 1);
        let later = d(2030, 1, 1);
        assert_eq!(earlier.join(&later), later);
        // Year (2025) spans through Dec 31; that's later than
        // Date(2025-06-15)'s end-of-span.
        let year_2025 = y(2025);
        let mid_2025 = d(2025, 6, 15);
        // Year 2025 ends Dec 31; mid-2025 date ends June 15; year wins.
        assert_eq!(year_2025.join(&mid_2025), year_2025);
    }

    #[test]
    fn declassify_on_absorption() {
        // Absorption pin: 3-state cube over a total-order semilattice.
        // For a total order, both laws hold by algebra: a⊔(a⊓b)=a and
        // a⊓(a⊔b)=a. No current defect here; pin guards future regressions.
        // §H.6 p104 (most-restrictive declassification date wins = max).
        // Verified 2026-05-16 against CAPCO-2016.md.
        let all_states = [bottom(), d(2025, 6, 15), y(2030)];
        for x in &all_states {
            for yy in &all_states {
                let x_join_yy = x.join(yy);
                let x_meet_yy = x.meet(yy);
                assert_eq!(
                    x.join(&x_meet_yy),
                    x.clone(),
                    "absorption x ⊔ (x ⊓ y) = x failed for x={x:?}, y={yy:?}"
                );
                assert_eq!(
                    x.meet(&x_join_yy),
                    x.clone(),
                    "absorption x ⊓ (x ⊔ y) = x failed for x={x:?}, y={yy:?}"
                );
            }
        }
    }
}

// ===========================================================================
// PR 4b-B Commit 4 — DissemSet
// ===========================================================================
// CAPCO-2016 §H.8 p136/p140 (OC-USGOV supersession), §H.8 pp155-156
// (RELIDO unanimity), §D.2 Table 3 + §H.8 p145 (NOFORN dominates).
// Verified 2026-05-15 against CAPCO-2016.md.

mod dissem_set {
    use marque_capco::DissemSet;
    use marque_ism::{CanonicalAttrs, DissemControl};
    use marque_scheme::JoinSemilattice;
    use proptest::prelude::*;

    fn portion(controls: &[DissemControl]) -> CanonicalAttrs {
        let mut a = CanonicalAttrs::default();
        a.dissem_us = controls.to_vec().into_boxed_slice();
        a
    }

    #[test]
    fn dissem_basic_union() {
        // Plain union for the non-supersession-managed tokens.
        let portions = [
            portion(&[DissemControl::Imc]),
            portion(&[DissemControl::Pr]),
        ];
        let s = DissemSet::from_attrs_iter(&portions);
        assert!(s.as_set().contains(&DissemControl::Imc));
        assert!(s.as_set().contains(&DissemControl::Pr));
    }

    #[test]
    fn dissem_oc_usgov_supersession_mirrors_pagecontext() {
        // OC + OC-USGOV in joined set → drop OC-USGOV.
        // §H.8 p136 + p140.
        let portions = [
            portion(&[DissemControl::Oc, DissemControl::OcUsgov]),
            portion(&[DissemControl::Oc]),
        ];
        let s = DissemSet::from_attrs_iter(&portions);
        assert!(s.as_set().contains(&DissemControl::Oc));
        assert!(!s.as_set().contains(&DissemControl::OcUsgov));
    }

    #[test]
    fn dissem_oc_usgov_kept_when_no_orcon() {
        // Pure OC-USGOV across portions → kept (no supersession trigger).
        let portions = [
            portion(&[DissemControl::OcUsgov]),
            portion(&[DissemControl::OcUsgov]),
        ];
        let s = DissemSet::from_attrs_iter(&portions);
        assert!(s.as_set().contains(&DissemControl::OcUsgov));
        assert!(!s.as_set().contains(&DissemControl::Oc));
    }

    #[test]
    fn dissem_relido_observed_unanimity_pass() {
        // Every portion has RELIDO → kept and unanimous=true.
        let portions = [
            portion(&[DissemControl::Relido]),
            portion(&[DissemControl::Relido]),
            portion(&[DissemControl::Relido]),
        ];
        let s = DissemSet::from_attrs_iter(&portions);
        assert!(s.as_set().contains(&DissemControl::Relido));
        assert!(s.relido_unanimous());
    }

    #[test]
    fn dissem_relido_observed_unanimity_fail() {
        // 2 of 3 portions have RELIDO → dropped and unanimous=false.
        // §H.8 pp155-156.
        let portions = [
            portion(&[DissemControl::Relido]),
            portion(&[DissemControl::Relido]),
            portion(&[]),
        ];
        let s = DissemSet::from_attrs_iter(&portions);
        assert!(!s.as_set().contains(&DissemControl::Relido));
        assert!(!s.relido_unanimous());
    }

    #[test]
    fn dissem_relido_layer1_does_not_infer() {
        // 1-portion uncaveated classified, no RELIDO in portion → no
        // RELIDO in DissemSet. Layer 2 FD&R inference defers to PR
        // 4b-D.
        let portions = [portion(&[])];
        let s = DissemSet::from_attrs_iter(&portions);
        assert!(!s.as_set().contains(&DissemControl::Relido));
    }

    #[test]
    fn dissem_noforn_clears_rel_relido_displayonly() {
        // NOFORN + REL TO + RELIDO + DISPLAY ONLY → only NOFORN
        // survives. §D.2 Table 3 + §H.8 p145.
        let portions = [
            portion(&[DissemControl::Nf]),
            portion(&[DissemControl::Rel]),
            portion(&[DissemControl::Relido]),
            portion(&[DissemControl::Displayonly]),
        ];
        let s = DissemSet::from_attrs_iter(&portions);
        assert!(s.as_set().contains(&DissemControl::Nf));
        assert!(!s.as_set().contains(&DissemControl::Rel));
        assert!(!s.as_set().contains(&DissemControl::Relido));
        assert!(!s.as_set().contains(&DissemControl::Displayonly));
    }

    // Proptest: assoc/comm/idem on DissemSet over arbitrary
    // dissem-control bag operands. The state space is small enough
    // (proptest collects up to 8 tokens per side) that the test runs
    // in microseconds.
    fn arb_controls() -> impl Strategy<Value = Vec<DissemControl>> {
        // Restrict to a tractable subset of representative variants.
        let single = prop_oneof![
            Just(DissemControl::Oc),
            Just(DissemControl::OcUsgov),
            Just(DissemControl::Nf),
            Just(DissemControl::Rel),
            Just(DissemControl::Relido),
            Just(DissemControl::Displayonly),
            Just(DissemControl::Imc),
            Just(DissemControl::Pr),
            Just(DissemControl::Fouo),
            Just(DissemControl::Dsen),
        ];
        prop::collection::vec(single, 0..=4)
    }

    fn arb_portions() -> impl Strategy<Value = Vec<CanonicalAttrs>> {
        prop::collection::vec(arb_controls().prop_map(|v| portion(&v)), 0..=4)
    }

    proptest! {
        #[test]
        fn dissem_set_lattice_laws_idempotent_associative(
            p1 in arb_portions(),
            p2 in arb_portions(),
            p3 in arb_portions(),
        ) {
            let s1 = DissemSet::from_attrs_iter(&p1);
            let s2 = DissemSet::from_attrs_iter(&p2);
            let s3 = DissemSet::from_attrs_iter(&p3);
            // Commutativity.
            prop_assert_eq!(s1.join(&s2), s2.join(&s1));
            // Idempotency.
            prop_assert_eq!(s1.join(&s1), s1.clone());
            // Associativity.
            prop_assert_eq!(
                s1.join(&s2).join(&s3),
                s1.join(&s2.join(&s3))
            );
            // Identity with bottom.
            let bottom = DissemSet::empty();
            prop_assert_eq!(bottom.join(&s1), s1.clone());
            prop_assert_eq!(s1.join(&bottom), s1.clone());
        }

        // NOTE (PR #456 trait split): `DissemSet` is join-only
        // (`JoinSemilattice` but NOT `MeetSemilattice`). The C-4 meet-
        // side absorption test `a ⊔ (a ⊓ b) = a` is enforced at the
        // type level — `DissemSet::meet` does not exist. The join-only
        // laws (commutativity, idempotency, associativity, identity)
        // are fully covered by `dissem_set_lattice_laws_idempotent_associative`.
    }

    #[test]
    fn dissem_set_all_empty_constructors_agree() {
        // C-5 (PR 4b-B follow-up): `from_attrs_iter(&[])` must
        // return the same value as `DissemSet::empty()`. Pre-fix,
        // `from_attrs_iter(&[])` set `relido_observed_unanimous =
        // false` (universal-over-empty short-circuit failure) while
        // `empty()` documented the vacuous `true` value. The two
        // bottom states were not `PartialEq`, and joining with the
        // wrong bottom dropped RELIDO under the overlay.
        let from_empty = DissemSet::from_attrs_iter(&[]);
        let empty = DissemSet::empty();
        assert_eq!(from_empty, empty);
        assert!(from_empty.relido_unanimous());
        assert!(from_empty.as_set().is_empty());

        // C-8 (PR 4b-B follow-up): `DissemSet::default()` MUST also
        // agree with `DissemSet::empty()`. Pre-fix, `#[derive(Default)]`
        // produced `relido_observed_unanimous = false` (bool's Default)
        // while `empty()` uses `true` (vacuous-truth over empty
        // portion list). The two bottom states were `PartialEq`-
        // different, and joining a `Default::default()` operand into
        // a unanimous-RELIDO set dropped RELIDO under the
        // unanimity-AND-propagation rule.
        let default = DissemSet::default();
        assert_eq!(default, empty, "C-8: Default == empty()");
        assert!(
            default.relido_unanimous(),
            "C-8: Default is vacuously unanimous"
        );
        assert!(default.as_set().is_empty(), "C-8: Default is the empty bag");
    }

    #[test]
    fn dissem_set_default_does_not_drop_relido_when_joined() {
        // C-8 (PR 4b-B follow-up): concrete regression — joining a
        // unanimous-RELIDO set with `DissemSet::default()` MUST
        // preserve RELIDO. Pre-fix, the derived `Default` set
        // `relido_observed_unanimous = false`, so the AND-propagation
        // in `join` flipped the flag, and the overlay then dropped
        // RELIDO from the set.
        let unanimous_relido = DissemSet::from_attrs_iter(&[portion(&[DissemControl::Relido])]);
        let default = DissemSet::default();
        let joined_left = unanimous_relido.join(&default);
        let joined_right = default.join(&unanimous_relido);
        assert!(
            joined_left.as_set().contains(&DissemControl::Relido),
            "C-8: RELIDO preserved across join with Default (left)"
        );
        assert!(
            joined_right.as_set().contains(&DissemControl::Relido),
            "C-8: RELIDO preserved across join with Default (right)"
        );
        assert!(
            joined_left.relido_unanimous(),
            "C-8: unanimity preserved across join with Default (left)"
        );
        assert!(
            joined_right.relido_unanimous(),
            "C-8: unanimity preserved across join with Default (right)"
        );
    }

    // NOTE (PR #456 trait split): `DissemSet::absorption_specific_relido_case`
    // was removed because `DissemSet` no longer implements `MeetSemilattice`.
    // The C-4 correction (unanimous-RELIDO join-absorption) is now enforced
    // structurally by the trait split: callers cannot call `.meet()` on
    // `DissemSet`, eliminating the class of bugs the test was guarding.
    // The RELIDO-unanimity preservation regression is covered by
    // `dissem_set_default_does_not_drop_relido_when_joined` above.
}

// ===========================================================================
// PR 4b-B Commit 4 — NatoDissemSet
// ===========================================================================
// CAPCO-2016 p41 (NATO reciprocity table). Verified 2026-05-15.

mod nato_dissem_set {
    use marque_capco::NatoDissemSet;
    use marque_ism::{CanonicalAttrs, DissemControl};
    use marque_scheme::{JoinSemilattice, MeetSemilattice};

    fn portion(controls: &[DissemControl]) -> CanonicalAttrs {
        let mut a = CanonicalAttrs::default();
        a.dissem_nato = controls.to_vec().into_boxed_slice();
        a
    }

    #[test]
    fn nato_dissem_set_plain_union() {
        // NATO contributes only ORCON-NATO and REL TO (CAPCO-2016 p41);
        // plain BTreeSet union, no supersession overlays.
        let portions = [
            portion(&[DissemControl::Oc]), // ORCON in NATO namespace
            portion(&[DissemControl::Rel]),
        ];
        let s = NatoDissemSet::from_attrs_iter(&portions);
        assert!(s.as_set().contains(&DissemControl::Oc));
        assert!(s.as_set().contains(&DissemControl::Rel));
    }

    #[test]
    fn nato_dissem_set_lattice_laws() {
        let p1 = portion(&[DissemControl::Oc]);
        let p2 = portion(&[DissemControl::Rel]);
        let p3 = portion(&[]);
        let s1 = NatoDissemSet::from_attrs_iter(&[p1]);
        let s2 = NatoDissemSet::from_attrs_iter(&[p2]);
        let s3 = NatoDissemSet::from_attrs_iter(&[p3]);
        assert_eq!(s1.join(&s2), s2.join(&s1), "comm");
        assert_eq!(s1.join(&s1), s1, "idem");
        assert_eq!(s1.join(&s2).join(&s3), s1.join(&s2.join(&s3)), "assoc");
        let bottom = NatoDissemSet::empty();
        assert_eq!(bottom.join(&s1), s1);
    }

    #[test]
    fn nato_dissem_set_absorption() {
        // Absorption pin: 4-state cube over a powerset lattice.
        // For a powerset lattice, A∪(A∩B)=A and A∩(A∪B)=A hold by
        // set algebra. No current defect; pin guards future regressions.
        // CAPCO-2016 p41 (NATO reciprocity; trivial union).
        // Verified 2026-05-16 against CAPCO-2016.md.
        let bottom = NatoDissemSet::empty();
        let p_oc = portion(&[DissemControl::Oc]);
        let p_rel = portion(&[DissemControl::Rel]);
        let p_both = portion(&[DissemControl::Oc, DissemControl::Rel]);
        let s_oc = NatoDissemSet::from_attrs_iter(&[p_oc]);
        let s_rel = NatoDissemSet::from_attrs_iter(&[p_rel]);
        let s_both = NatoDissemSet::from_attrs_iter(&[p_both]);
        let all_states = [bottom, s_oc, s_rel, s_both];
        for x in &all_states {
            for yy in &all_states {
                let x_join_yy = x.join(yy);
                let x_meet_yy = x.meet(yy);
                assert_eq!(
                    x.join(&x_meet_yy),
                    x.clone(),
                    "absorption x ⊔ (x ⊓ y) = x failed for x={x:?}, y={yy:?}"
                );
                assert_eq!(
                    x.meet(&x_join_yy),
                    x.clone(),
                    "absorption x ⊓ (x ⊔ y) = x failed for x={x:?}, y={yy:?}"
                );
            }
        }
    }
}

// ===========================================================================
// PR 4b-B Commit 5 — JointSet
// ===========================================================================
// CAPCO-2016 §H.3 p56 (JOINT grammar) + §H.7 p123 (FGI source-acknowledged
// form for disunity-collapse migration) + §H.3 p57 (mixed-US case
// `Mixed`; M-23 PR 4b-B 7th-pass — pre-C-3 wording said `bottom` but
// PR 4b-B follow-up C-3 split `Mixed` out of `Bottom` so the
// absorbing JOINT+non-JOINT state keeps `join` associative). Verified
// 2026-05-15 against CAPCO-2016.md.

mod joint_set {
    use marque_capco::JointSet;
    use marque_ism::{
        CanonicalAttrs, Classification, CountryCode, JointClassification, MarkingClassification,
    };
    use marque_scheme::JoinSemilattice;
    use proptest::prelude::*;

    fn cc(s: &str) -> CountryCode {
        CountryCode::try_new(s.as_bytes()).expect("valid trigraph")
    }

    fn joint_portion(level: Classification, producers: &[&str]) -> CanonicalAttrs {
        let countries: Box<[CountryCode]> = producers
            .iter()
            .map(|s| cc(s))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Joint(JointClassification {
            level,
            countries,
        }));
        a
    }

    fn us_portion(level: Classification) -> CanonicalAttrs {
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Us(level));
        a
    }

    #[test]
    fn joint_unanimous_two_portions_same_producers_passes_through() {
        let portions = [
            joint_portion(Classification::Secret, &["USA", "GBR"]),
            joint_portion(Classification::Secret, &["USA", "GBR"]),
        ];
        let s = JointSet::from_attrs_iter(&portions);
        assert!(
            matches!(&s, JointSet::UnanimousProducers { level, producers }
                if *level == Classification::Secret
                    && producers.len() == 2),
            "expected UnanimousProducers {{S, [USA, GBR]}}, got {s:?}",
        );
    }

    #[test]
    fn joint_unanimous_three_portions_different_levels_picks_highest() {
        let portions = [
            joint_portion(Classification::Confidential, &["USA", "GBR"]),
            joint_portion(Classification::TopSecret, &["USA", "GBR"]),
            joint_portion(Classification::Secret, &["USA", "GBR"]),
        ];
        let s = JointSet::from_attrs_iter(&portions);
        assert_eq!(s.highest_level(), Some(Classification::TopSecret));
    }

    #[test]
    fn joint_disunity_two_portions_different_producers_collapses_to_fgi() {
        let portions = [
            joint_portion(Classification::Secret, &["USA", "GBR"]),
            joint_portion(Classification::Secret, &["USA", "CAN"]),
        ];
        let s = JointSet::from_attrs_iter(&portions);
        assert!(
            s.is_disunity_collapse(),
            "expected DisunityCollapse, got {s:?}"
        );
        let non_us = s.disunity_collapse_non_us_producers().unwrap();
        assert!(non_us.contains(&cc("GBR")));
        assert!(non_us.contains(&cc("CAN")));
        assert_eq!(non_us.len(), 2);
    }

    #[test]
    fn joint_mixed_with_us_portions_returns_mixed_no_w004() {
        // §H.3 p57: JOINT does not roll up in US documents.
        // PR 4b-B follow-up C-3: the constructor returns `Mixed`
        // (a distinct, absorbing state) — pre-fix it returned
        // `Bottom`, which `join` treats as the identity, breaking
        // associativity under grouped folds. No W004 fires on
        // `Mixed`; the JOINT non-US producers ride to FgiSet via
        // the existing PageContext path.
        let portions = [
            joint_portion(Classification::Secret, &["USA", "GBR"]),
            us_portion(Classification::Secret),
        ];
        let s = JointSet::from_attrs_iter(&portions);
        assert!(matches!(s, JointSet::Mixed), "expected Mixed, got {s:?}");
        assert!(s.is_mixed());
        assert!(!s.is_disunity_collapse());
    }

    #[test]
    fn joint_empty_producers_normalizes_to_bottom() {
        // Defensive shape: JOINT requires USA + at least one
        // co-owner. An empty producer list is malformed. The
        // constructor should return Bottom rather than constructing
        // an UnanimousProducers { producers: ∅ }.
        let portions = [joint_portion(Classification::Secret, &[])];
        let s = JointSet::from_attrs_iter(&portions);
        assert!(matches!(s, JointSet::Bottom));
    }

    #[test]
    fn joint_set_lattice_laws_assoc_comm_idem() {
        // Four-variant state space exhausted as 4×4×4 over
        // representatives that exercise each transition. C-3
        // (PR 4b-B follow-up) added the `Mixed` variant; without
        // it, `(Mixed + Bottom).join(Unanimous)` would have
        // resurrected an `UnanimousProducers` value, breaking
        // associativity.
        let bottom = JointSet::Bottom;
        let mixed = JointSet::Mixed;
        let unanim =
            JointSet::from_attrs_iter(&[joint_portion(Classification::Secret, &["USA", "GBR"])]);
        let disunity = JointSet::from_attrs_iter(&[
            joint_portion(Classification::Secret, &["USA", "GBR"]),
            joint_portion(Classification::Secret, &["USA", "CAN"]),
        ]);
        let states = [bottom, mixed, unanim, disunity];
        for a in &states {
            for b in &states {
                // Commutativity.
                assert_eq!(a.join(b), b.join(a), "comm fail: {a:?} vs {b:?}");
                // Idempotency.
                assert_eq!(a.join(a), *a, "idem fail: {a:?}");
                for c in &states {
                    // Associativity.
                    assert_eq!(
                        a.join(b).join(c),
                        a.join(&b.join(c)),
                        "assoc fail: ({a:?}, {b:?}, {c:?})"
                    );
                }
            }
        }
        // Identity with bottom (separate assertion for clarity).
        let bottom = JointSet::Bottom;
        for s in &states {
            assert_eq!(bottom.join(s), s.clone());
            assert_eq!(s.join(&bottom), s.clone());
        }
    }

    // NOTE (PR #456 trait split): `JointSet` is join-only (`JoinSemilattice`
    // but NOT `MeetSemilattice`). The C-6 meet-side absorption test and the
    // `meet_identical_producers` test are removed because `JointSet::meet`
    // no longer exists. The join-absorption law `a ⊔ (a ⊓ b) = a` is now
    // enforced at the type level — callers that would have called `.meet()`
    // on `JointSet` are rejected at compile time. The join-side laws
    // (commutativity, idempotency, identity, DisunityCollapse absorbing)
    // are fully covered by `joint_set_join_laws_*` tests above.

    #[test]
    fn joint_set_mixed_absorbs_unanimous_under_grouped_join() {
        // C-3 (PR 4b-B follow-up) regression case: with the pre-fix
        // 3-variant state space, `Mixed` was conflated with `Bottom`.
        // Grouped joins could resurrect `UnanimousProducers` from a
        // page that should have collapsed to mixed JOINT+US:
        //
        //   [JOINT, US] becomes `Bottom` (under the pre-fix);
        //   `Bottom.join(JointSet::from([JOINT])) = JointSet::from([JOINT])`
        //   → JOINT banner on a mixed page (wrong per §H.3 p57).
        //
        // With `Mixed` as a distinct absorbing state, the same fold
        // stays at `Mixed`, preserving the §H.3 p57 decision.
        let mixed = JointSet::Mixed;
        let unanim =
            JointSet::from_attrs_iter(&[joint_portion(Classification::Secret, &["USA", "GBR"])]);
        assert_eq!(mixed.join(&unanim), JointSet::Mixed);
        assert_eq!(unanim.join(&mixed), JointSet::Mixed);
    }

    // -----------------------------------------------------------------------
    // GitHub issue #489 — proptest shuffling for `JointSet`
    // -----------------------------------------------------------------------
    // The hand-written `joint_set_lattice_laws_assoc_comm_idem` test above
    // covers the 4-state representation (Bottom / UnanimousProducers /
    // DisunityCollapse / Mixed) via FIXED-ORDERING fixtures. PR 4b-D
    // activates the `JointSet` lattice on the production hot path; the
    // 51 byte-identity parity fixtures in
    // `page_context_lattice_parity.rs` also use fixed orderings by
    // construction. Any associativity, commutativity, or
    // order-invariance defect that depends on the order of producers
    // within a portion OR the order of portions across the page would
    // therefore survive both the existing law test and the parity gate,
    // and only surface after the hot-path flip.
    //
    // The three proptests below shuffle both axes:
    //
    //   1. Producer-list membership order within each generated portion
    //      (via `BTreeSet` collection at the strategy level, which
    //      naturally dedups and canonicalizes ordering).
    //   2. Portion order across the `[CanonicalAttrs]` slice fed to
    //      `from_attrs_iter` (via `prop_shuffle()` on the generated
    //      page, drawing a uniform sample from S_n rather than the
    //      single non-identity orbit a reverse would test).
    //
    // Every generated JOINT portion has USA auto-injected, so the
    // §H.3 p56 malformed-drop path (empty producer list OR missing USA)
    // is exercised by the existing hand-written
    // `joint_empty_producers_normalizes_to_bottom` test — proptest cycles
    // stay on the well-formed grammar.
    //
    // CAPCO authority (re-verified against `crates/capco/docs/
    // CAPCO-2016.md` at authorship per Constitution VIII):
    //   - §H.3 p56 (JOINT grammar: USA in producer list).
    //   - §H.3 p57 (disunity → FGI migration + JOINT-doesn't-roll-up).

    /// Strategy: produce a single JOINT portion at `level` with a
    /// randomized producer list. USA is always injected so the §H.3 p56
    /// malformed-drop path is not exercised here. Non-USA producers
    /// are drawn from a closed 7-trigraph alphabet (GBR/CAN/AUS/NZL/
    /// FRA/DEU/JPN). A `BTreeSet` at the strategy level dedups and
    /// gives a canonical ordering; the slice we eventually feed back
    /// into `JointClassification::countries` is therefore
    /// shuffle-invariant by construction, which is the property under
    /// test.
    fn arb_joint_portion() -> impl Strategy<Value = CanonicalAttrs> {
        let level = prop_oneof![
            Just(Classification::Confidential),
            Just(Classification::Secret),
            Just(Classification::TopSecret),
        ];
        let non_us = prop_oneof![
            Just("GBR"),
            Just("CAN"),
            Just("AUS"),
            Just("NZL"),
            Just("FRA"),
            Just("DEU"),
            Just("JPN"),
        ];
        let producers = prop::collection::btree_set(non_us, 1..=4);
        (level, producers).prop_map(|(lvl, set)| {
            let mut names: Vec<&'static str> = vec!["USA"];
            names.extend(set);
            joint_portion(lvl, &names)
        })
    }

    /// Strategy: a sequence of 1..=4 JOINT portions, each itself a
    /// randomized `arb_joint_portion`. The order of this `Vec` is the
    /// "portion order" axis tested by
    /// `joint_set_from_attrs_iter_portion_order_invariant`.
    fn arb_joint_page() -> impl Strategy<Value = Vec<CanonicalAttrs>> {
        prop::collection::vec(arb_joint_portion(), 1..=4)
    }

    proptest! {
        /// Issue #489 load-bearing test: shuffling the
        /// `[CanonicalAttrs]` slice fed to `from_attrs_iter` MUST NOT
        /// change the resulting `JointSet`. PR 4b-D wires this into
        /// the production hot path; an ordering-dependent constructor
        /// would silently produce different banners depending on the
        /// page-traversal order.
        ///
        /// We pair each generated page with a uniform sample from its
        /// permutation orbit via `prop_shuffle()` and assert that the
        /// derived `JointSet` is equal under permutation. A reverse-
        /// only probe would test exactly one of `n!` permutations and
        /// would be vacuous at length 1; `prop_shuffle()` draws a
        /// fresh permutation per case so each of 256 cases samples a
        /// different orbit element.
        #[test]
        fn joint_set_from_attrs_iter_portion_order_invariant(
            (page, shuffled) in arb_joint_page().prop_flat_map(|page| {
                let copy = page.clone();
                (Just(page), Just(copy).prop_shuffle())
            }),
        ) {
            let s_forward = JointSet::from_attrs_iter(&page);
            let s_shuffled = JointSet::from_attrs_iter(&shuffled);
            prop_assert_eq!(s_forward, s_shuffled);
        }

        /// Associativity, commutativity, idempotency under proptest-
        /// shuffled operands. Each operand is independently drawn
        /// from `arb_joint_page`, exercising the full state-space
        /// transitions across the 4-variant decision tree.
        #[test]
        fn joint_set_proptest_assoc_comm_idem(
            page_a in arb_joint_page(),
            page_b in arb_joint_page(),
            page_c in arb_joint_page(),
        ) {
            let a = JointSet::from_attrs_iter(&page_a);
            let b = JointSet::from_attrs_iter(&page_b);
            let c = JointSet::from_attrs_iter(&page_c);

            // Commutativity.
            prop_assert_eq!(a.join(&b), b.join(&a));
            // Associativity.
            prop_assert_eq!(a.join(&b).join(&c), a.join(&b.join(&c)));
            // Idempotency.
            prop_assert_eq!(a.join(&a), a.clone());
        }

        // NOTE (PR #456 trait split): `joint_set_proptest_join_side_absorption`
        // removed. `JointSet` no longer implements `MeetSemilattice`, so the
        // absorption expression `a ⊔ (a ⊓ b) = a` is not expressible. The
        // join-only laws (commutativity, associativity, idempotency) in
        // `joint_set_join_laws_commutative_associative_idempotent` above
        // fully cover the `JoinSemilattice` contract.
    }
}

// ===========================================================================
// PR 4b-B Commit 6 — RelToBlock
// ===========================================================================
// CAPCO-2016 §H.8 pp150-151 (REL TO grammar) + §D.2 Table 3 rows 9-13
// (REL TO supersession) + §H.9 p172 + p174 (NODIS/EXDIS clear REL TO).
// Verified 2026-05-15 against CAPCO-2016.md.

mod rel_to_block {
    use marque_capco::RelToBlock;
    use marque_ism::{CanonicalAttrs, CountryCode, DissemControl, NonIcDissem};
    use marque_scheme::{JoinSemilattice, MeetSemilattice};

    fn cc(s: &str) -> CountryCode {
        CountryCode::try_new(s.as_bytes()).expect("valid trigraph")
    }

    fn rel_portion(rel_to: &[&str]) -> CanonicalAttrs {
        let mut a = CanonicalAttrs::default();
        a.rel_to = rel_to
            .iter()
            .map(|s| cc(s))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        a
    }

    fn nf_portion() -> CanonicalAttrs {
        let mut a = CanonicalAttrs::default();
        a.dissem_us = vec![DissemControl::Nf].into_boxed_slice();
        a
    }

    fn nodis_portion() -> CanonicalAttrs {
        let mut a = CanonicalAttrs::default();
        a.non_ic_dissem = vec![NonIcDissem::Nodis].into_boxed_slice();
        a
    }

    #[test]
    fn rel_to_block_intersection_common_list() {
        // §H.8 p152 worked example: two portions, common LIST →
        // banner gets the intersection.
        let portions = [
            rel_portion(&["USA", "GBR", "CAN"]),
            rel_portion(&["USA", "GBR", "AUS"]),
        ];
        let b = RelToBlock::from_attrs_iter(&portions);
        match &b {
            RelToBlock::Lattice { countries } => {
                assert!(countries.contains(&cc("USA")));
                assert!(countries.contains(&cc("GBR")));
                assert!(!countries.contains(&cc("CAN")));
                assert!(!countries.contains(&cc("AUS")));
                assert_eq!(countries.len(), 2);
            }
            other => panic!("expected Lattice, got {other:?}"),
        }
    }

    #[test]
    fn rel_to_block_noforn_supersedes() {
        // §D.2 Table 3 + §H.8 p145: NOFORN in any portion → empty
        // REL TO; lattice returns NofornSuperseded.
        let portions = [rel_portion(&["USA", "GBR"]), nf_portion()];
        let b = RelToBlock::from_attrs_iter(&portions);
        assert!(b.is_noforn_superseded());
        assert!(b.into_boxed_slice().is_empty());
    }

    #[test]
    fn rel_to_block_nodis_supersedes() {
        // §H.9 p174: NODIS clears REL TO.
        let portions = [rel_portion(&["USA", "GBR"]), nodis_portion()];
        let b = RelToBlock::from_attrs_iter(&portions);
        assert!(b.is_noforn_superseded());
    }

    #[test]
    fn rel_to_block_empty_intersection_returns_empty() {
        // §D.2 Table 3 row 9: no-common-LIST → NOFORN. The lattice
        // produces `Empty` (a distinct state from `Bottom`); the
        // post-projection PageRewrite injects NF into DissemSet.
        //
        // C-2 regression (PR 4b-B follow-up): before this fix,
        // disjoint intersection returned `Bottom`, conflating the
        // "no portions observed" identity with the "intersected
        // to empty" absorbing state. That broke join associativity
        // (see `rel_to_block_associative_under_empty_intersection`
        // below).
        let portions = [rel_portion(&["USA", "GBR"]), rel_portion(&["USA", "FRA"])];
        let b = RelToBlock::from_attrs_iter(&portions);
        match b {
            RelToBlock::Lattice { countries } => {
                // USA survives — both portions have USA.
                assert!(countries.contains(&cc("USA")));
                assert_eq!(countries.len(), 1);
            }
            _ => panic!("expected non-empty intersection (USA common)"),
        }

        // Now a truly disjoint case.
        let portions = [rel_portion(&["GBR", "CAN"]), rel_portion(&["FRA", "DEU"])];
        let b = RelToBlock::from_attrs_iter(&portions);
        assert!(matches!(b, RelToBlock::Empty));
        assert!(!b.is_noforn_superseded());
        assert!(b.is_empty_intersection());
        // Both `Empty` and `Bottom` render to an empty REL TO list.
        assert!(b.to_vec().is_empty());
    }

    #[test]
    fn rel_to_block_tetragraph_expansion_fvey() {
        // FVEY expands to {AUS, CAN, GBR, NZL, USA}.
        let portions = [rel_portion(&["FVEY"]), rel_portion(&["USA", "GBR", "CAN"])];
        let b = RelToBlock::from_attrs_iter(&portions);
        match b {
            RelToBlock::Lattice { countries } => {
                assert!(countries.contains(&cc("USA")));
                assert!(countries.contains(&cc("GBR")));
                assert!(countries.contains(&cc("CAN")));
                // AUS / NZL drop because the second portion didn't list them.
                assert!(!countries.contains(&cc("AUS")));
                assert!(!countries.contains(&cc("NZL")));
            }
            other => panic!("expected Lattice, got {other:?}"),
        }
    }

    #[test]
    fn rel_to_block_usa_first_ordering() {
        // §H.8 p151: USA first, rest alphabetical.
        let portions = [
            rel_portion(&["GBR", "CAN", "USA", "AUS"]),
            rel_portion(&["GBR", "CAN", "USA", "AUS"]),
        ];
        let b = RelToBlock::from_attrs_iter(&portions);
        let codes = b.to_vec();
        assert_eq!(codes[0], cc("USA"));
        // The rest are alphabetical: AUS, CAN, GBR.
        assert_eq!(codes[1], cc("AUS"));
        assert_eq!(codes[2], cc("CAN"));
        assert_eq!(codes[3], cc("GBR"));
    }

    #[test]
    fn rel_to_block_lattice_laws() {
        let bottom = RelToBlock::Bottom;
        let nf = RelToBlock::NofornSuperseded;
        let empty = RelToBlock::Empty;
        let a = RelToBlock::from_attrs_iter(&[rel_portion(&["USA", "GBR", "CAN"])]);
        let b = RelToBlock::from_attrs_iter(&[rel_portion(&["USA", "GBR"])]);
        let c = RelToBlock::from_attrs_iter(&[rel_portion(&["USA", "CAN"])]);
        let states = [bottom, nf, empty, a, b, c];

        for s1 in &states {
            for s2 in &states {
                assert_eq!(s1.join(s2), s2.join(s1), "comm: {s1:?} vs {s2:?}");
                assert_eq!(s1.join(s1), s1.clone(), "idem");
                for s3 in &states {
                    assert_eq!(
                        s1.join(s2).join(s3),
                        s1.join(&s2.join(s3)),
                        "assoc: ({s1:?}, {s2:?}, {s3:?})"
                    );
                }
            }
        }
    }

    #[test]
    fn rel_to_block_associative_under_empty_intersection() {
        // C-2 (PR 4b-B follow-up) regression: pre-fix, disjoint
        // intersection collapsed to `Bottom`, which `join` treats as
        // the identity. The associativity check below would have
        // failed:
        //
        //   ({GBR} ⊔ {FRA}) ⊔ {FRA}  →  Bottom ⊔ {FRA}  =  {FRA}
        //   {GBR} ⊔ ({FRA} ⊔ {FRA})  →  {GBR} ⊔ {FRA}   =  Bottom
        //
        // With the new `Empty` variant the left side reaches `Empty`
        // (absorbing), which stays `Empty` after joining `{FRA}`.
        let gbr_only = RelToBlock::from_attrs_iter(&[rel_portion(&["GBR"])]);
        let fra_only = RelToBlock::from_attrs_iter(&[rel_portion(&["FRA"])]);
        let lhs = gbr_only.join(&fra_only).join(&fra_only);
        let rhs = gbr_only.join(&fra_only.join(&fra_only));
        assert_eq!(lhs, rhs, "assoc: ({{GBR}} ⊔ {{FRA}}) ⊔ {{FRA}}");
        assert_eq!(lhs, RelToBlock::Empty);
    }

    #[test]
    fn rel_to_block_empty_absorbs_lattice_in_join() {
        let empty = RelToBlock::Empty;
        let gbr = RelToBlock::from_attrs_iter(&[rel_portion(&["GBR"])]);
        assert_eq!(empty.join(&gbr), RelToBlock::Empty);
        assert_eq!(gbr.join(&empty), RelToBlock::Empty);
    }

    #[test]
    fn rel_to_block_noforn_superseded_dominates_empty() {
        // NofornSuperseded > Empty in the join lattice — NOFORN is
        // an explicit "do not release" signal; an empty-intersection
        // is the §D.2 Table 3 row 9 path that requires post-
        // projection NF injection. Both are absorbing; their join
        // resolves to the more conservative outcome.
        let empty = RelToBlock::Empty;
        let nf = RelToBlock::NofornSuperseded;
        assert_eq!(empty.join(&nf), RelToBlock::NofornSuperseded);
        assert_eq!(nf.join(&empty), RelToBlock::NofornSuperseded);
    }

    #[test]
    fn rel_to_block_absorption_laws() {
        // 11th-pass consultant HIGH defect pin: absorption laws over the
        // full 6-state cube (same states as `rel_to_block_lattice_laws`).
        //
        // Pre-fix, `meet(NofornSuperseded, x) = NofornSuperseded` violated
        // dual absorption — `a ⊓ (a ⊔ N) = a ⊓ N = N ≠ a` for a ≠ N.
        // Fixed by treating NofornSuperseded as join-top: `meet(N, x) = x`.
        //
        // Authority: §H.8 p145 (NOFORN cannot be used with REL TO —
        // semantically NofornSuperseded is the join-absorbing outcome);
        // §D.2 Table 3 row 9 (disjoint-LIST → NOFORN).
        // Verified 2026-05-16 against CAPCO-2016.md.
        let bottom = RelToBlock::Bottom;
        let nf = RelToBlock::NofornSuperseded;
        let empty = RelToBlock::Empty;
        let a = RelToBlock::from_attrs_iter(&[rel_portion(&["USA", "GBR", "CAN"])]);
        let b = RelToBlock::from_attrs_iter(&[rel_portion(&["USA", "GBR"])]);
        let c = RelToBlock::from_attrs_iter(&[rel_portion(&["USA", "CAN"])]);
        let all_states = [bottom, nf, empty, a, b, c];

        for x in &all_states {
            for y in &all_states {
                let x_join_y = x.join(y);
                let x_meet_y = x.meet(y);
                assert_eq!(
                    x.join(&x_meet_y),
                    x.clone(),
                    "absorption x ⊔ (x ⊓ y) = x failed for x={x:?}, y={y:?}, x⊓y={x_meet_y:?}"
                );
                assert_eq!(
                    x.meet(&x_join_y),
                    x.clone(),
                    "absorption x ⊓ (x ⊔ y) = x failed for x={x:?}, y={y:?}, x⊔y={x_join_y:?}"
                );
            }
        }
    }
}

// ===========================================================================
// P-9-1 (PR 4b-B 9th-pass) — FgiSet concealed-top meet absorption
// ===========================================================================
//
// CAPCO-2016 §H.7 p128: "A document containing portions of both
// source-concealed FGI and source-acknowledged FGI must have only the
// 'FGI' marking without source trigraph(s)/tetragraph(s) in the banner
// line, as it is the most restrictive form of the marking." The
// source-concealed form is therefore the lattice TOP in the FGI
// source-disclosure dimension.
//
// Pre-P-9-1, `FgiSet::meet` performed country-set intersection even when
// one operand was concealed (empty countries). After P-1 (8th-pass) made
// the join treat concealed as top, the dual absorption law
// `a ⊓ (a ⊔ b) = a` broke: `acknowledged.meet(acknowledged.join(concealed))`
// = `acknowledged.meet(concealed)` = intersect({GBR,CAN}, {}) = {} → None.
//
// P-9-1 fixes meet to treat concealed as top (meet with top = other operand).
// These tests exercise the four cases in the fix.
//
// Verified 2026-05-16 against crates/capco/docs/CAPCO-2016.md.

mod fgi_set_concealed_top {
    use marque_capco::lattice::FgiSet;
    use marque_ism::{CountryCode, FgiMarker};
    use marque_scheme::{JoinSemilattice, MeetSemilattice};

    fn gbr() -> CountryCode {
        CountryCode::try_new(b"GBR").expect("GBR")
    }
    fn can() -> CountryCode {
        CountryCode::try_new(b"CAN").expect("CAN")
    }
    fn acknowledged(codes: impl IntoIterator<Item = CountryCode>) -> FgiSet {
        let marker = FgiMarker::acknowledged(codes).expect("non-empty acknowledged");
        FgiSet::from_marker(Some(&marker))
    }
    fn concealed() -> FgiSet {
        FgiSet::from_marker(Some(&FgiMarker::SourceConcealed))
    }

    // Case (a): both concealed — top ⊓ top = top.
    #[test]
    fn meet_concealed_concealed_is_concealed() {
        let c = concealed();
        assert_eq!(c.meet(&c), concealed());
    }

    // Case (b): concealed ⊓ acknowledged = acknowledged (meet with top).
    #[test]
    fn meet_concealed_acknowledged_is_acknowledged() {
        let c = concealed();
        let a = acknowledged([gbr(), can()]);
        // Both orderings must give the same result (commutativity).
        assert_eq!(
            c.meet(&a),
            a,
            "concealed ⊓ acknowledged should be acknowledged"
        );
        assert_eq!(
            a.meet(&c),
            a,
            "acknowledged ⊓ concealed should be acknowledged"
        );
    }

    // Core absorption: `acknowledged.meet(acknowledged.join(concealed)) = acknowledged`.
    // Pre-P-9-1, join produced concealed (top), then meet intersected with empty
    // countries → None. Post-P-9-1, meet(acknowledged, concealed-top) = acknowledged.
    #[test]
    fn absorption_acknowledged_meet_join_concealed() {
        let a = acknowledged([gbr(), can()]);
        let b = concealed();
        // a ⊔ b = concealed (P-1 join)
        let joined = a.join(&b);
        assert_eq!(
            joined,
            concealed(),
            "join with concealed must produce concealed"
        );
        // a ⊓ (a ⊔ b) = a  (P-9-1 meet-over-join absorption)
        assert_eq!(
            a.meet(&joined),
            a,
            "P-9-1: a ⊓ (a ⊔ b) must equal a when b is concealed"
        );
    }

    // Dual absorption: `a ⊔ (a ⊓ b) = a` (was always correct; guard regression).
    #[test]
    fn absorption_acknowledged_join_meet_concealed() {
        let a = acknowledged([gbr()]);
        let b = concealed();
        // a ⊓ b = a (P-9-1: meet with top = other operand)
        let met = a.meet(&b);
        assert_eq!(met, a, "acknowledged ⊓ concealed should equal acknowledged");
        // a ⊔ (a ⊓ b) = a ⊔ a = a
        assert_eq!(a.join(&met), a, "P-9-1: a ⊔ (a ⊓ b) must equal a");
    }

    // Both acknowledged, disjoint countries → None (existing behavior, regression guard).
    #[test]
    fn meet_disjoint_acknowledged_is_none() {
        let a = acknowledged([gbr()]);
        let b = acknowledged([can()]);
        assert_eq!(
            a.meet(&b),
            FgiSet::None,
            "disjoint country sets must collapse to None"
        );
    }

    // Both acknowledged, overlapping countries → intersection.
    #[test]
    fn meet_overlapping_acknowledged_is_intersection() {
        let a = acknowledged([gbr(), can()]);
        let b = acknowledged([gbr()]);
        let expected = acknowledged([gbr()]);
        assert_eq!(
            a.meet(&b),
            expected,
            "overlapping acknowledged sets must produce the intersection"
        );
    }
}
