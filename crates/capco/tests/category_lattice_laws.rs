// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Per-category lattice law property tests.
//!
//! Verifies the algebraic laws (associativity, commutativity,
//! idempotency, identity-with-bottom) for each CAPCO category's
//! lattice impl, across all seven CAPCO categories.
//!
//! # AEA category coverage
//!
//! The AEA `Product` decomposes into five sub-axes; each axis carries
//! its own algebraic shape and gets its own law-suite below. The
//! composite `AeaSet` laws ride on top.

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
// These tests pin the `AeaSet::meet` algebra component-wise per sub-axis
// plus the Product-level laws.

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
// ClassificationLattice
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

    // The five-level chain (U < R < C < S < TS) is exercised
    // end-to-end. `Restricted` is the US equivalent of NATO `NR` per
    // `NatoClassification::us_equivalent()`.
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
    // Commutativity tiebreak across equal-level variants. Returning the
    // left operand on equal level would break commutativity. This suite
    // exhausts the cross-product of `{Us, Fgi, Nato, Joint}` at every
    // level and asserts `a.join(b) == b.join(a)`.
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
        // (U / R / C / S / TS).
        //
        // Include multiple distinct payloads at the same
        // variant-rank/same-level so commutativity is exercised on the
        // payload tiebreaker as well as the variant tiebreaker — a join
        // that fell through `ra <= rb` would return the left operand on
        // same-variant/same-level, which is non-commutative.
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
            // level; these must join commutatively.
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
        // At each effective level, every pair of distinct-variant
        // classifications must commute under join.
        for level in ALL {
            let variants = arb_classification_variant(level);
            for a in &variants {
                for b in &variants {
                    assert_eq!(
                        a.join(b),
                        b.join(a),
                        "join not commutative at level {level:?}: \
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
                        "meet not commutative at level {level:?}: \
                         {a:?} vs {b:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn classification_us_wins_equal_level_tiebreak() {
        // US is the canonical variant per §H.7 reciprocal
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
    // Absorption laws across the cross-product of equal-level variants
    // AND across the partial order on payloads.
    //
    // Absorption pair: `a ⊔ (a ⊓ b) = a` and `a ⊓ (a ⊔ b) = a`.
    //
    // `meet` must NOT return the union of same-variant payloads —
    // otherwise `a.join(a.meet(b)) = union(a, b) ≠ a` for the operand
    // whose payload was a strict subset. The meet rule (in
    // `crates/capco/src/lattice/classification.rs::ClassificationLattice::meet`):
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
    // The same asymmetry inside `meet_foreign_classification` for
    // `Conflict`-`Conflict` cross-variant inner foreign payloads
    // likewise returns the higher-rank inner. See the
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
            // Conflict variants with cross-variant inner foreign
            // payloads exercise the dual-absorption behavior on
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
                    "a ⊔ (a ⊓ b) ≠ a for a={a:?}, b={b:?}, \
                     a⊓b={a_meet_b:?}"
                );
                assert_eq!(
                    a.meet(&a_join_b),
                    *a,
                    "a ⊓ (a ⊔ b) ≠ a for a={a:?}, b={b:?}, \
                     a⊔b={a_join_b:?}"
                );
            }
        }
    }

    // Spot-checks: counterexamples pinned individually so a regression
    // names them in test output.
    //
    // At same level, different variants are NOT incomparable — they
    // are linearly
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
            "meet at cross-variant same-level returns dominated"
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
            "meet of same-variant disjoint payloads must be bottom"
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
        assert_eq!(meet, fgi_gbr, "meet picks smaller payload on subset");
        // Symmetric: order shouldn't matter.
        assert_eq!(fgi_gbr.meet(&fgi_both), fgi_gbr);
        // Absorption.
        assert_eq!(fgi_both.join(&meet), fgi_both);
        assert_eq!(fgi_gbr.join(&meet), fgi_gbr);
    }

    // -----------------------------------------------------------------------
    // Conflict cross-variant inner foreign-classification absorption.
    //
    // Two `Conflict` values at the same outer level with different
    // `foreign` inner variants (e.g. one with `Nato(NS)` inner, another
    // with `Fgi(S, [GBR])` inner) trigger the `Conflict-Conflict` arm of
    // `classification_join_same_variant` /
    // `classification_meet_same_variant`. Those arms delegate to
    // `merge_foreign_classification` / `meet_foreign_classification`,
    // whose tiebreaks must align:
    //
    //   - `merge_foreign_classification` cross-variant: returns the
    //     lower-rank variant (Fgi=1 < Nato=2 < Joint=3).
    //   - `meet_foreign_classification` cross-variant: returned `None`,
    //     which the outer `classification_meet_same_variant` translated
    //     to the lattice bottom.
    //
    // A `None` return from `meet_foreign_classification` would break the
    // dual absorption law `a ⊓ (a ⊔ b) = a` for the operand whose inner
    // `foreign` was the LOWER-rank one. Instead,
    // `meet_foreign_classification` cross-variant aligns with
    // `merge_foreign_classification`'s tiebreak — returning the
    // HIGHER-rank operand (the dominated, lower-≤ side; the GLB dual) —
    // so the inner foreign axis is its own linear-ordered tiebreak that
    // satisfies absorption, mirroring the outer classification level.
    //
    // Authority: §H.7 pp123-125 reciprocal-normalization (variant-rank
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
        // - meet_foreign_classification(Nato, Fgi): returns the
        //   higher-rank inner (the lower-≤ side; GLB dual) = Nato.
        //   So `a.meet(b) = Conflict{foreign: Nato}` = a.
        //
        // Absorption checks:
        //   - a ⊔ (a ⊓ b) = a ⊔ a = a ✓
        //   - a ⊓ (a ⊔ b) = a ⊓ b = a ✓
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
        assert_eq!(a_join_b, b_join_a, "join must be commutative");

        let a_meet_b = a.meet(&b);
        let b_meet_a = b.meet(&a);
        assert_eq!(a_meet_b, b_meet_a, "meet must be commutative");

        // Absorption — both directions.
        assert_eq!(a.join(&a_meet_b), a, "a ⊔ (a ⊓ b) = a");
        assert_eq!(a.meet(&a_join_b), a, "a ⊓ (a ⊔ b) = a");
        assert_eq!(b.join(&b_meet_a), b, "b ⊔ (b ⊓ a) = b");
        assert_eq!(b.meet(&b_join_a), b, "b ⊓ (b ⊔ a) = b");
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
        assert_eq!(a.join(&a_meet_b), a, "a ⊔ (a ⊓ b) = a");
        assert_eq!(a.meet(&a_join_b), a, "a ⊓ (a ⊔ b) = a");
        assert_eq!(b.join(&b.meet(&a)), b, "b ⊔ (b ⊓ a) = b");
        assert_eq!(b.meet(&b.join(&a)), b, "b ⊓ (b ⊔ a) = b");
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
                assert_eq!(a.join(b), b.join(a), "join commutativity");
                assert_eq!(a.meet(b), b.meet(a), "meet commutativity");
                assert_eq!(
                    a.join(&a.meet(b)),
                    *a,
                    "a ⊔ (a ⊓ b) ≠ a for a={a:?}, b={b:?}"
                );
                assert_eq!(
                    a.meet(&a.join(b)),
                    *a,
                    "a ⊓ (a ⊔ b) ≠ a for a={a:?}, b={b:?}"
                );
            }
        }
    }
}

// ===========================================================================
// NatoClassLattice
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
// DeclassifyOnLattice
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
// DissemSet
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

        // NOTE (`Lattice` trait split, issue #456 / PR #502): `DissemSet` is join-only
        // (`JoinSemilattice` but NOT `MeetSemilattice`). The meet-side
        // absorption test `a ⊔ (a ⊓ b) = a` is enforced at the type
        // level — `DissemSet::meet` does not exist. The join-only
        // laws (commutativity, idempotency, associativity, identity)
        // are fully covered by `dissem_set_lattice_laws_idempotent_associative`.
    }

    #[test]
    fn dissem_set_all_empty_constructors_agree() {
        // `from_attrs_iter(&[])` must return the same value as
        // `DissemSet::empty()`. If `from_attrs_iter(&[])` set
        // `relido_observed_unanimous = false` while `empty()` uses the
        // vacuous `true`, the two bottom states would not be
        // `PartialEq`, and joining with the wrong bottom would drop
        // RELIDO under the overlay.
        let from_empty = DissemSet::from_attrs_iter(&[]);
        let empty = DissemSet::empty();
        assert_eq!(from_empty, empty);
        assert!(from_empty.relido_unanimous());
        assert!(from_empty.as_set().is_empty());

        // `DissemSet::default()` MUST also agree with
        // `DissemSet::empty()`. If `#[derive(Default)]` produced
        // `relido_observed_unanimous = false` (bool's Default) while
        // `empty()` uses `true` (vacuous-truth over empty portion
        // list), the two bottom states would be `PartialEq`-different,
        // and joining a `Default::default()` operand into a
        // unanimous-RELIDO set would drop RELIDO under the
        // unanimity-AND-propagation rule.
        let default = DissemSet::default();
        assert_eq!(default, empty, "Default == empty()");
        assert!(default.relido_unanimous(), "Default is vacuously unanimous");
        assert!(default.as_set().is_empty(), "Default is the empty bag");
    }

    #[test]
    fn dissem_set_default_does_not_drop_relido_when_joined() {
        // Joining a unanimous-RELIDO set with `DissemSet::default()`
        // MUST preserve RELIDO. A derived `Default` setting
        // `relido_observed_unanimous = false` would let the
        // AND-propagation in `join` flip the flag, and the overlay
        // would then drop RELIDO from the set.
        let unanimous_relido = DissemSet::from_attrs_iter(&[portion(&[DissemControl::Relido])]);
        let default = DissemSet::default();
        let joined_left = unanimous_relido.join(&default);
        let joined_right = default.join(&unanimous_relido);
        assert!(
            joined_left.as_set().contains(&DissemControl::Relido),
            "RELIDO preserved across join with Default (left)"
        );
        assert!(
            joined_right.as_set().contains(&DissemControl::Relido),
            "RELIDO preserved across join with Default (right)"
        );
        assert!(
            joined_left.relido_unanimous(),
            "unanimity preserved across join with Default (left)"
        );
        assert!(
            joined_right.relido_unanimous(),
            "unanimity preserved across join with Default (right)"
        );
    }

    // NOTE (`Lattice` trait split, issue #456 / PR #502): `DissemSet::absorption_specific_relido_case`
    // was removed because `DissemSet` no longer implements `MeetSemilattice`.
    // Unanimous-RELIDO join-absorption is now enforced structurally by
    // the trait split: callers cannot call `.meet()` on
    // `DissemSet`, eliminating the class of bugs the test was guarding.
    // The RELIDO-unanimity preservation regression is covered by
    // `dissem_set_default_does_not_drop_relido_when_joined` above.
}

// ===========================================================================
// NatoDissemSet
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
// JointSet
// ===========================================================================
// CAPCO-2016 §H.3 p56 (JOINT grammar) + §H.7 p123 (FGI source-acknowledged
// form for disunity-collapse migration) + §H.3 p57 (mixed-US case
// `Mixed`; the `Mixed` variant is split out of `Bottom` so the
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
        // The constructor returns `Mixed` (a distinct, absorbing
        // state), not `Bottom` — `join` treats `Bottom` as the
        // identity, which would break associativity under grouped
        // folds. No W004 fires on `Mixed`; the JOINT non-US producers
        // ride to FgiSet.
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
        // representatives that exercise each transition. The `Mixed`
        // variant is distinct from `Bottom`; without it,
        // `(Mixed + Bottom).join(Unanimous)` would resurrect an
        // `UnanimousProducers` value, breaking associativity.
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

    // NOTE (`Lattice` trait split, issue #456 / PR #502): `JointSet` is join-only (`JoinSemilattice`
    // but NOT `MeetSemilattice`). The meet-side absorption test and the
    // `meet_identical_producers` test are removed because `JointSet::meet`
    // no longer exists. The join-absorption law `a ⊔ (a ⊓ b) = a` is now
    // enforced at the type level — callers that would have called `.meet()`
    // on `JointSet` are rejected at compile time. The join-side laws
    // (commutativity, idempotency, identity, DisunityCollapse absorbing)
    // are fully covered by `joint_set_join_laws_*` tests above.

    #[test]
    fn joint_set_mixed_absorbs_unanimous_under_grouped_join() {
        // If `Mixed` were conflated with `Bottom`, grouped joins could
        // resurrect `UnanimousProducers` from a page that should have
        // collapsed to mixed JOINT+US:
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
    // DisunityCollapse / Mixed) via FIXED-ORDERING fixtures. The
    // `JointSet` lattice runs on the production hot path; the
    // byte-identity parity fixtures in `lattice_vs_scheme_parity.rs`
    // also use fixed orderings by construction. Any associativity,
    // commutativity, or
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
        /// change the resulting `JointSet`. The constructor runs on the
        /// production hot path; an ordering-dependent constructor would
        /// silently produce different banners depending on the
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

        // NOTE (`Lattice` trait split, issue #456 / PR #502): `joint_set_proptest_join_side_absorption`
        // removed. `JointSet` no longer implements `MeetSemilattice`, so the
        // absorption expression `a ⊔ (a ⊓ b) = a` is not expressible. The
        // join-only laws (commutativity, associativity, idempotency) in
        // `joint_set_join_laws_commutative_associative_idempotent` above
        // fully cover the `JoinSemilattice` contract.
    }
}

// ===========================================================================
// RelToBlock
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
        // Disjoint intersection must NOT return `Bottom`: that would
        // conflate the "no portions observed" identity with the
        // "intersected to empty" absorbing state and break join
        // associativity (see
        // `rel_to_block_associative_under_empty_intersection` below).
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
        // If disjoint intersection collapsed to `Bottom` (which `join`
        // treats as the identity), the associativity check below would
        // fail:
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
// FgiSet concealed-top meet absorption
// ===========================================================================
//
// CAPCO-2016 §H.7 p128: "A document containing portions of both
// source-concealed FGI and source-acknowledged FGI must have only the
// 'FGI' marking without source trigraph(s)/tetragraph(s) in the banner
// line, as it is the most restrictive form of the marking." The
// source-concealed form is therefore the lattice TOP in the FGI
// source-disclosure dimension.
//
// `FgiSet::meet` must NOT intersect country sets when one operand is
// concealed (empty countries). Since the join treats concealed as top,
// intersecting would break the dual absorption law `a ⊓ (a ⊔ b) = a`:
// `acknowledged.meet(acknowledged.join(concealed))` =
// `acknowledged.meet(concealed)` = intersect({GBR,CAN}, {}) = {} → None.
// Instead, meet treats concealed as top (meet with top = other operand).
// These tests exercise the four cases.
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
    // Join produces concealed (top); meet with the concealed top must
    // return the other operand (acknowledged), not intersect to None.
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
        // a ⊓ (a ⊔ b) = a  (meet-over-join absorption)
        assert_eq!(
            a.meet(&joined),
            a,
            "a ⊓ (a ⊔ b) must equal a when b is concealed"
        );
    }

    // Dual absorption: `a ⊔ (a ⊓ b) = a` (was always correct; guard regression).
    #[test]
    fn absorption_acknowledged_join_meet_concealed() {
        let a = acknowledged([gbr()]);
        let b = concealed();
        // a ⊓ b = a (meet with top = other operand)
        let met = a.meet(&b);
        assert_eq!(met, a, "acknowledged ⊓ concealed should equal acknowledged");
        // a ⊔ (a ⊓ b) = a ⊔ a = a
        assert_eq!(a.join(&met), a, "a ⊔ (a ⊓ b) must equal a");
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

    // -----------------------------------------------------------------------
    // FgiSet join-side extension.
    //
    // The existing tests above pin meet-side dominance (concealed acts as
    // lattice TOP on the meet per §H.7 p128). The 3 tests below pin the
    // join-side dual: concealed dominates acknowledged under join — i.e.
    // any source-concealed entry on any portion forces a source-concealed
    // banner per §H.7 p128 ("the most restrictive form of the marking").
    //
    // Citation re-verified against `crates/capco/docs/CAPCO-2016.md` at
    // authorship 2026-05-19 (§H.7 p128: "A document containing portions
    // of both source-concealed FGI and source-acknowledged FGI must have
    // only the 'FGI' marking without source trigraph(s)/tetragraph(s) in
    // the banner line, as it is the most restrictive form of the marking").
    // -----------------------------------------------------------------------

    /// §H.7 p128 join-side: `Concealed ⊔ Acknowledged{...} = Concealed`.
    /// A source-concealed portion on the page forces the banner to the
    /// concealed form regardless of co-occurring acknowledged producers.
    #[test]
    fn join_concealed_with_acknowledged_is_concealed() {
        let c = concealed();
        let a = acknowledged([gbr(), can()]);
        assert_eq!(
            c.join(&a),
            concealed(),
            "concealed ⊔ acknowledged should be concealed (§H.7 p128)"
        );
        // Commutativity check.
        assert_eq!(
            a.join(&c),
            concealed(),
            "acknowledged ⊔ concealed should also be concealed (§H.7 p128)"
        );
    }

    /// Concealed is idempotent under join (top of the join semilattice
    /// on the concealment axis).
    #[test]
    fn join_concealed_concealed_is_concealed() {
        let c = concealed();
        assert_eq!(c.join(&c), concealed());
    }

    /// `FgiSet::None` is the join identity: `None ⊔ x = x`.
    #[test]
    fn join_none_is_identity() {
        let bottom = FgiSet::None;
        let c = concealed();
        let a = acknowledged([gbr()]);
        assert_eq!(bottom.join(&c), c);
        assert_eq!(c.join(&bottom), c);
        assert_eq!(bottom.join(&a), a);
        assert_eq!(a.join(&bottom), a);
        // Identity composes with itself.
        assert_eq!(bottom.join(&bottom), bottom);
    }
}

// ===========================================================================
// SciSet lattice laws (proptest)
//
// SciSet's vocabulary spans three Published bare control systems (HCS,
// SI, TK), two registered NATO SAPs (BOHEMIA, BALK per §G.2 p40), and
// agency-allocated Custom identifiers (§A.6 p15 `[A-Z0-9]{2,5}`). This
// module's proptest harness exercises the algebraic laws over **the 3
// Published bare systems only**, with the bounded compartment / sub-
// compartment strategies below — ≤3 markings per SciSet, ≤3
// compartments per marking, ≤3 sub-compartments per compartment (cap
// is per-axis per PM doc D-2 to keep runtime <5s under default
// `proptest::Config`).
//
// Note: `proptest_lattice.rs` (sci_join_* / sci_meet_*) carries
// parallel SciSet proptest coverage with similar Published-only bounds.
// The duplication is deliberate — consolidating algebraic-law coverage
// in `category_lattice_laws.rs` makes that file the canonical
// single-source-of-truth for per-category lattice laws, with smaller
// proptest cycles and uniform §-citation discipline.
//
// Gap (both harnesses): `Custom(...)` and `NatoSap(...)` variants are
// not exercised in either file. Closing the gap is a straightforward
// additive follow-up (extend the `prop_oneof!` to sample both
// variants); architecturally distinct from the bounded `Published`
// strategy used here.
//
// Citation re-verified against `crates/capco/docs/CAPCO-2016.md` at
// authorship 2026-05-19 (§A.6 p15 custom-control grammar; §G.2 p40
// NATO SAP registration; §H.4 p61 SCI grammar).
// ===========================================================================

mod sci_set {
    use marque_capco::lattice::SciSet;
    use marque_ism::{SciCompartment, SciControlBare, SciControlSystem, SciMarking};
    use marque_scheme::{JoinSemilattice, MeetSemilattice};
    use proptest::prelude::*;
    use smol_str::SmolStr;

    /// Open-vocab control-system strategy: 3 published bare systems
    /// (HCS, SI, TK) so the resulting SciSet has compartment structure
    /// to exercise.
    fn arb_system() -> impl Strategy<Value = SciControlSystem> {
        prop_oneof![
            Just(SciControlSystem::Published(SciControlBare::Hcs)),
            Just(SciControlSystem::Published(SciControlBare::Si)),
            Just(SciControlSystem::Published(SciControlBare::Tk)),
        ]
    }

    /// Compartment identifier strategy: 1-3 chars from a small alpha
    /// alphabet `[A-G]`. Keeps state space bounded. CAPCO §A.6 + §H.4
    /// admit single-letter compartments (`G`, `P`) and longer forms
    /// (`MMM`); 1-3 covers both shapes.
    fn arb_compartment_id() -> impl Strategy<Value = String> {
        "[A-G]{1,3}"
    }

    /// Sub-compartment identifier strategy: 4 chars from a small alpha
    /// alphabet. Per §A.6 + §H.4 sub-compartments are 4-6 alnum; we use
    /// 4 alpha for proptest stability.
    fn arb_sub_compartment_id() -> impl Strategy<Value = String> {
        "[A-Z]{4,4}"
    }

    /// One `SciCompartment` with 0-3 sub-compartments.
    fn arb_compartment() -> impl Strategy<Value = SciCompartment> {
        (
            arb_compartment_id(),
            prop::collection::vec(arb_sub_compartment_id(), 0..=3),
        )
            .prop_map(|(cid, subs)| {
                let sub_boxes: Box<[SmolStr]> = subs
                    .into_iter()
                    .map(SmolStr::from)
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                SciCompartment::new(cid, sub_boxes)
            })
    }

    /// One `SciMarking` with 0-3 compartments under a single system.
    fn arb_marking() -> impl Strategy<Value = SciMarking> {
        (
            arb_system(),
            prop::collection::vec(arb_compartment(), 0..=3),
        )
            .prop_map(|(sys, comps)| SciMarking::new(sys, comps.into_boxed_slice(), None))
    }

    /// One `SciSet` from 0-3 markings (at most 3 systems per the
    /// PM doc D-2 ≤3×3×3 cap).
    fn arb_sci_set() -> impl Strategy<Value = SciSet> {
        prop::collection::vec(arb_marking(), 0..=3)
            .prop_map(|markings| SciSet::from_markings(&markings))
    }

    proptest! {
        /// SciSet join is a prefix-closed union.
        #[test]
        fn sci_set_assoc_comm_idem_proptest(
            a in arb_sci_set(),
            b in arb_sci_set(),
            c in arb_sci_set(),
        ) {
            // Commutativity.
            prop_assert_eq!(a.join(&b), b.join(&a));
            // Associativity.
            prop_assert_eq!(a.join(&b).join(&c), a.join(&b.join(&c)));
            // Idempotency.
            prop_assert_eq!(a.join(&a), a.clone());
        }

        /// Empty is the join identity. SciSet does NOT implement
        /// `BoundedJoinSemilattice` — open-vocab `Custom` controls
        /// have no lawful finite top — so the identity property is
        /// the strongest claim available.
        #[test]
        fn sci_set_identity_with_bottom(a in arb_sci_set()) {
            let empty = SciSet::empty();
            prop_assert_eq!(a.join(&empty), a.clone());
            prop_assert_eq!(empty.join(&a), a);
        }

        /// Meet idempotency under proptest. §3.3a equal-depth meet
        /// is commutative + idempotent on the SciSet open-vocab space
        /// (verified at the brute-force level in `lattice_laws.rs`;
        /// re-pinned here for proptest cycle coverage).
        #[test]
        fn sci_set_meet_idempotent_proptest(a in arb_sci_set()) {
            prop_assert_eq!(a.meet(&a), a);
        }
    }
}

// ===========================================================================
// SarSet lattice laws (proptest)
//
// SarSet's program identifiers are agency-assigned codewords with no
// central registry (CVEnumISMSAR.xml is intentionally empty per ODNI
// convention). The proptest harness here samples 1-3 programs × 0-2
// compartments × 0-2 sub-compartments — the same cap as SciSet for
// runtime parity.
//
// SarSet has no `from_attrs_iter` shortcut; constructors use
// `SarMarking` literals via `SarSet::from_marking`.
//
// Citation re-verified against `crates/capco/docs/CAPCO-2016.md` at
// authorship 2026-05-19 (§H.5 p99 SAR grammar).
// ===========================================================================

mod sar_set {
    use marque_capco::lattice::SarSet;
    use marque_ism::{SarCompartment, SarIndicator, SarMarking, SarProgram};
    use marque_scheme::{JoinSemilattice, MeetSemilattice};
    use proptest::prelude::*;
    use smol_str::SmolStr;

    /// Program identifier: 2-3 alpha (mirrors §H.5 p99-100 grammar:
    /// "Program identifier is 2-3 char abbreviation").
    fn arb_program_id() -> impl Strategy<Value = String> {
        "[A-Z]{2,3}"
    }

    /// Compartment identifier: 2-3 alnum.
    fn arb_compartment_id() -> impl Strategy<Value = String> {
        "[A-Z][A-Z0-9]{1,2}"
    }

    /// Sub-compartment identifier: 2-3 alnum.
    fn arb_sub_compartment_id() -> impl Strategy<Value = String> {
        "[A-Z][A-Z0-9]{1,2}"
    }

    fn arb_compartment() -> impl Strategy<Value = SarCompartment> {
        (
            arb_compartment_id(),
            prop::collection::vec(arb_sub_compartment_id(), 0..=2),
        )
            .prop_map(|(cid, subs)| {
                let sub_boxes: Box<[SmolStr]> = subs
                    .into_iter()
                    .map(SmolStr::from)
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                SarCompartment::new(cid, sub_boxes)
            })
    }

    fn arb_program() -> impl Strategy<Value = SarProgram> {
        (
            arb_program_id(),
            prop::collection::vec(arb_compartment(), 0..=2),
        )
            .prop_map(|(pid, comps)| SarProgram::new(pid, comps.into_boxed_slice()))
    }

    fn arb_marking() -> impl Strategy<Value = SarMarking> {
        prop::collection::vec(arb_program(), 1..=3)
            .prop_map(|progs| SarMarking::new(SarIndicator::Abbrev, progs.into_boxed_slice()))
    }

    /// SarSet from at most one `SarMarking` (since `from_marking` takes
    /// a single marker). The "no SAR" case is `SarSet::empty()`.
    fn arb_sar_set() -> impl Strategy<Value = SarSet> {
        prop_oneof![
            Just(SarSet::empty()),
            arb_marking().prop_map(|m| SarSet::from_marking(Some(&m))),
        ]
    }

    proptest! {
        /// SarSet join is a prefix-closed union over the program hierarchy.
        #[test]
        fn sar_set_assoc_comm_idem_proptest(
            a in arb_sar_set(),
            b in arb_sar_set(),
            c in arb_sar_set(),
        ) {
            // Commutativity.
            prop_assert_eq!(a.join(&b), b.join(&a));
            // Associativity.
            prop_assert_eq!(a.join(&b).join(&c), a.join(&b.join(&c)));
            // Idempotency.
            prop_assert_eq!(a.join(&a), a.clone());
        }

        /// Empty is the join identity. SarSet does NOT implement
        /// `BoundedJoinSemilattice` — open-vocab SAR programs have
        /// no lawful finite top.
        #[test]
        fn sar_set_identity_with_bottom(a in arb_sar_set()) {
            let empty = SarSet::empty();
            prop_assert_eq!(a.join(&empty), a.clone());
            prop_assert_eq!(empty.join(&a), a);
        }

        /// Meet idempotency under proptest.
        #[test]
        fn sar_set_meet_idempotent_proptest(a in arb_sar_set()) {
            prop_assert_eq!(a.meet(&a), a);
        }
    }
}

// ===========================================================================
// NonIcDissemSet compositional invariance
//
// `NonIcDissemSet` does NOT implement `JoinSemilattice` — it is an
// accumulator-style type constructed via
// `from_attrs_iter(&[CanonicalAttrs])` with classification-gated
// SBU-NF / LES-NF split and NODIS / EXDIS NF-injection (per §H.9
// p178 / p185 / p172 / p174). The "lattice law" reframing for this
// type is **input-order invariance**: `from_attrs_iter(&[a, b])`
// must equal `from_attrs_iter(&[b, a])`. This is the property the
// production hot path relies on (page-traversal order must not
// affect the projected non-IC dissem state).
//
// Brute-force enumeration over the closed `NonIcDissem` variants
// (LIMDIS / EXDIS / NODIS / SBU / SBU-NF / LES / LES-NF / SSI) plus
// classification gating gives full coverage in a small state space.
//
// Citation re-verified against `crates/capco/docs/CAPCO-2016.md` at
// authorship 2026-05-19 (§H.9 pp169-191 non-IC dissem family;
// §H.9 p178 SBU-NF + p185 LES-NF classified split).
// ===========================================================================

mod non_ic_dissem_set {
    use marque_capco::lattice::NonIcDissemSet;
    use marque_ism::{CanonicalAttrs, Classification, MarkingClassification, NonIcDissem};

    fn portion_with_non_ic(level: Classification, non_ic: &[NonIcDissem]) -> CanonicalAttrs {
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Us(level));
        a.non_ic_dissem = non_ic.to_vec().into_boxed_slice();
        a
    }

    /// Empty input → empty `NonIcDissemSet` (identity).
    #[test]
    fn non_ic_dissem_set_empty_input_is_bottom() {
        let s = NonIcDissemSet::from_attrs_iter(&[]);
        assert_eq!(s, NonIcDissemSet::empty());
        assert_eq!(s, NonIcDissemSet::default());
        assert!(s.as_set().is_empty());
        assert!(!s.needs_nf());
    }

    /// `from_attrs_iter` is order-invariant: `[a, b]` and `[b, a]`
    /// yield the same `NonIcDissemSet`. This is the
    /// compositional-lattice analogue of commutativity for the
    /// accumulator type. The production hot path
    /// (`CapcoScheme::project`) relies on this property — page
    /// traversal order MUST NOT affect the projected non-IC state.
    #[test]
    fn non_ic_dissem_set_from_attrs_order_invariant() {
        let p_limdis = portion_with_non_ic(Classification::Unclassified, &[NonIcDissem::Limdis]);
        let p_exdis = portion_with_non_ic(Classification::Secret, &[NonIcDissem::Exdis]);
        let p_sbu = portion_with_non_ic(Classification::Unclassified, &[NonIcDissem::Sbu]);
        let p_les = portion_with_non_ic(Classification::Unclassified, &[NonIcDissem::Les]);

        let s_forward = NonIcDissemSet::from_attrs_iter(&[
            p_limdis.clone(),
            p_exdis.clone(),
            p_sbu.clone(),
            p_les.clone(),
        ]);
        let s_reversed = NonIcDissemSet::from_attrs_iter(&[p_les, p_sbu, p_exdis, p_limdis]);

        assert_eq!(
            s_forward, s_reversed,
            "NonIcDissemSet::from_attrs_iter must be order-invariant"
        );
    }

    /// `from_attrs_iter` is idempotent on repeated portions: passing
    /// the same portion twice produces the same set as passing it
    /// once. This pins the BTreeSet-dedup semantics + classification-
    /// gate stability under repetition.
    #[test]
    fn non_ic_dissem_set_from_attrs_idempotent_on_repeats() {
        let p = portion_with_non_ic(Classification::Unclassified, &[NonIcDissem::Les]);
        let single = NonIcDissemSet::from_attrs_iter(std::slice::from_ref(&p));
        let doubled = NonIcDissemSet::from_attrs_iter(&[p.clone(), p]);
        assert_eq!(
            single, doubled,
            "NonIcDissemSet must dedup repeated portions"
        );
    }

    /// Classification gate (§H.9 p185 LES-NF Precedence Rules for
    /// Banner Line Guidance): "The LES marking always appears in the
    /// banner line if LES information (either LES or LES NOFORN) is
    /// contained in the document, regardless of the document's
    /// classification level. When a classified document contains
    /// portions of U//LES-NF, the 'LES' marking is used in the banner
    /// line and the NOFORN marking is applied as a Dissemination
    /// Control Marking. For example: SECRET//NOFORN//LES."
    ///
    /// Pins that the LES-NF → LES + needs-NF classification-gate
    /// transformation (`crates/capco/src/lattice/non_ic_dissem.rs::NonIcDissemSet::from_attrs_iter`
    /// LES-NF branch) participates in the order-invariance property —
    /// adding a classified peer to a U/LES-NF carrier produces the
    /// same final set whether the classified peer appears first or
    /// second. Using bare `Les` here would bypass the gate (which
    /// fires only on `LesNf` / `SbuNf` variants), making this test
    /// trivially pass without exercising the property it claims to.
    ///
    /// Citation re-verified against `crates/capco/docs/CAPCO-2016.md`
    /// at authorship 2026-05-19 (§H.9 p185 LES-NF banner-precedence).
    #[test]
    fn non_ic_dissem_set_classification_gate_order_invariant() {
        let p_les_nf = portion_with_non_ic(Classification::Unclassified, &[NonIcDissem::LesNf]);
        let p_class = portion_with_non_ic(Classification::Secret, &[]);
        let s_forward = NonIcDissemSet::from_attrs_iter(&[p_les_nf.clone(), p_class.clone()]);
        let s_reverse = NonIcDissemSet::from_attrs_iter(&[p_class, p_les_nf]);
        assert_eq!(
            s_forward, s_reverse,
            "classification gate must participate in order invariance: \
             LES-NF + classified peer must yield the same set regardless \
             of portion order"
        );
        // The gate produces LES (not LES-NF) post-transform and sets
        // needs_nf — both sides must agree on both observable axes.
        assert!(
            s_forward.as_set().contains(&NonIcDissem::Les),
            "post-gate set must contain bare LES (LES-NF → LES under classification gate)"
        );
        assert!(
            !s_forward.as_set().contains(&NonIcDissem::LesNf),
            "post-gate set must NOT contain LES-NF (the gate consumes it)"
        );
        assert!(
            s_forward.needs_nf(),
            "classification gate must request NOFORN injection per §H.9 p185"
        );
    }
}

// ===========================================================================
// DisplayOnlyBlock lattice laws
//
// DisplayOnlyBlock implements `JoinSemilattice` with a 4-variant
// absorbing-element pattern: `NofornSuperseded > Empty > Lattice{·} >
// Bottom`. The structure mirrors `RelToBlock` (see
// `category_lattice_laws.rs::rel_to_block` above) — same absorbing-
// element shape; same proof obligations.
//
// Tests use hybrid brute-force enumeration over the 3 enum-only
// variants (Bottom / Empty / NofornSuperseded) plus a hand-picked
// suite of `Lattice{countries}` payloads. The `Lattice` payload is
// open-vocab in `CountryCode`, so a small proptest seam over the
// 4 enum classes × 3 hand-picked country sets gives 12 sample
// states for the assoc/comm/idem walk.
//
// Citation re-verified against `crates/capco/docs/CAPCO-2016.md` at
// authorship 2026-05-19 (§H.8 pp149-150 DISPLAY ONLY template;
// §D.2 Table 3 rows 25-27 DISPLAY ONLY roll-up).
// ===========================================================================

mod display_only_block {
    use std::collections::BTreeSet;

    use marque_capco::lattice::DisplayOnlyBlock;
    use marque_ism::CountryCode;
    use marque_scheme::JoinSemilattice;

    fn cc(s: &str) -> CountryCode {
        CountryCode::try_new(s.as_bytes()).expect("valid trigraph")
    }

    fn lattice(codes: &[&str]) -> DisplayOnlyBlock {
        let countries: BTreeSet<CountryCode> = codes.iter().map(|s| cc(s)).collect();
        DisplayOnlyBlock::Lattice { countries }
    }

    /// Six-state representative sample exercising every join transition:
    ///
    /// - `Bottom` — identity
    /// - `Lattice{GBR, CAN}` — partial-intersection input
    /// - `Lattice{CAN}` — narrowing-intersection input
    /// - `Lattice{FRA}` — disjoint-intersection input (collapses to Empty)
    /// - `Empty` — absorbing-empty state
    /// - `NofornSuperseded` — top absorbing state
    fn samples() -> Vec<DisplayOnlyBlock> {
        vec![
            DisplayOnlyBlock::Bottom,
            lattice(&["GBR", "CAN"]),
            lattice(&["CAN"]),
            lattice(&["FRA"]),
            DisplayOnlyBlock::Empty,
            DisplayOnlyBlock::NofornSuperseded,
        ]
    }

    /// Brute-force walk over all 6×6×6 = 216 triples confirms join
    /// laws. State space is small; brute-force gives full coverage
    /// without proptest cycle overhead.
    #[test]
    fn display_only_block_join_laws_assoc_comm_idem() {
        let states = samples();
        for a in &states {
            // Idempotency.
            assert_eq!(a.join(a), *a, "idem fail: {a:?}");
            for b in &states {
                // Commutativity.
                assert_eq!(a.join(b), b.join(a), "comm fail: {a:?} vs {b:?}");
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
    }

    /// Bottom is the join identity per the 4-variant absorbing-element
    /// pattern at `crates/capco/src/lattice/display_only.rs::DisplayOnlyBlock::join`.
    #[test]
    fn display_only_block_bottom_is_join_identity() {
        let bottom = DisplayOnlyBlock::Bottom;
        for s in samples() {
            assert_eq!(bottom.join(&s), s.clone());
            assert_eq!(s.join(&bottom), s.clone());
        }
        // Identity composes with itself.
        assert_eq!(bottom.join(&bottom), bottom);
    }

    /// NofornSuperseded is the top absorbing element: `NofornSuperseded
    /// ⊔ x = NofornSuperseded` for every x. Confirms the 4-variant
    /// supersession chain `NofornSuperseded > Empty > Lattice{·} >
    /// Bottom`.
    #[test]
    fn display_only_block_noforn_superseded_absorbs() {
        let top = DisplayOnlyBlock::NofornSuperseded;
        for s in samples() {
            assert_eq!(top.join(&s), top);
            assert_eq!(s.join(&top), top);
        }
    }

    /// Empty absorbs every non-NofornSuperseded state — `Empty ⊔
    /// Lattice{·} = Empty`, `Empty ⊔ Bottom = Empty`. Only
    /// NofornSuperseded escapes the Empty absorbing trap.
    #[test]
    fn display_only_block_empty_absorbs_below_noforn() {
        let empty = DisplayOnlyBlock::Empty;
        let bottom_or_lattice = vec![
            DisplayOnlyBlock::Bottom,
            lattice(&["GBR"]),
            DisplayOnlyBlock::Empty,
        ];
        for s in bottom_or_lattice {
            assert_eq!(empty.join(&s), DisplayOnlyBlock::Empty);
            assert_eq!(s.join(&empty), DisplayOnlyBlock::Empty);
        }
        // NofornSuperseded escapes Empty.
        assert_eq!(
            empty.join(&DisplayOnlyBlock::NofornSuperseded),
            DisplayOnlyBlock::NofornSuperseded
        );
    }
}
