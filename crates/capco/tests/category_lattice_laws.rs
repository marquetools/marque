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
use marque_scheme::Lattice;
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
    use marque_scheme::{BoundedLattice, Lattice};

    fn lvl(c: Classification) -> ClassificationLattice {
        ClassificationLattice::new(Some(MarkingClassification::Us(c)))
    }

    const ALL: [Classification; 4] = [
        Classification::Unclassified,
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
}

// ===========================================================================
// PR 4b-B Commit 3 — NatoClassLattice
// ===========================================================================
// CAPCO-2016 §H.2 p55. Verified 2026-05-15.

mod nato_class_lattice {
    use marque_capco::NatoClassLattice;
    use marque_ism::NatoClassification;
    use marque_scheme::{BoundedLattice, Lattice};

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
                    assert_eq!(
                        la.join(&lb).join(&lc),
                        la.join(&lb.join(&lc)),
                        "assoc"
                    );
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
    use marque_scheme::Lattice;

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
}
