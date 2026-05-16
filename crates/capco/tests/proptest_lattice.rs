// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Property-based lattice law verification for CAPCO structural lattice types.
//!
//! Complements `lattice_laws.rs` (fixed-sample exhaustive cross-product) by
//! covering the same algebraic laws over generated inputs, exercising the much
//! larger space of compartment-tree combinations that the fixed samples can't
//! reach.

use marque_capco::lattice::{FgiSet, SarSet, SciSet};
use marque_ism::{
    CountryCode, FgiMarker, SarCompartment, SarIndicator, SarMarking, SarProgram, SciCompartment,
    SciControlBare, SciControlSystem, SciMarking,
};
use marque_scheme::Lattice;
use proptest::prelude::*;
use smol_str::SmolStr;

// ---------------------------------------------------------------------------
// SciSet strategy
// ---------------------------------------------------------------------------

fn arb_sci_control_bare() -> impl Strategy<Value = SciControlBare> {
    prop_oneof![
        Just(SciControlBare::Bur),
        Just(SciControlBare::Hcs),
        Just(SciControlBare::Klm),
        Just(SciControlBare::Mvl),
        Just(SciControlBare::Rsv),
        Just(SciControlBare::Si),
        Just(SciControlBare::Tk),
    ]
}

fn arb_uppercase_id(min: usize, max: usize) -> impl Strategy<Value = String> {
    proptest::string::string_regex(&format!("[A-Z0-9]{{{},{}}}", min, max)).expect("valid regex")
}

fn arb_sci_compartment() -> impl Strategy<Value = SciCompartment> {
    (
        arb_uppercase_id(1, 4),
        proptest::collection::vec(arb_uppercase_id(1, 4), 0..=3),
    )
        .prop_map(|(id, subs)| {
            let sub_boxes: Box<[SmolStr]> = subs
                .into_iter()
                .map(SmolStr::from)
                .collect::<Vec<_>>()
                .into_boxed_slice();
            SciCompartment::new(id, sub_boxes)
        })
}

fn arb_sci_marking() -> impl Strategy<Value = SciMarking> {
    (
        arb_sci_control_bare(),
        proptest::collection::vec(arb_sci_compartment(), 0..=3),
    )
        .prop_map(|(bare, comps)| {
            SciMarking::new(
                SciControlSystem::Published(bare),
                comps.into_boxed_slice(),
                None,
            )
        })
}

fn arb_sci_set() -> impl Strategy<Value = SciSet> {
    proptest::collection::vec(arb_sci_marking(), 0..=4)
        .prop_map(|markings| SciSet::from_markings(&markings))
}

// ---------------------------------------------------------------------------
// SarSet strategy
// ---------------------------------------------------------------------------

fn arb_sar_program_id() -> impl Strategy<Value = String> {
    proptest::string::string_regex("[A-Z0-9]{2,3}").expect("valid regex")
}

fn arb_sar_compartment() -> impl Strategy<Value = SarCompartment> {
    (
        arb_uppercase_id(1, 4),
        proptest::collection::vec(arb_uppercase_id(1, 4), 0..=2),
    )
        .prop_map(|(id, subs)| {
            let sub_boxes: Box<[SmolStr]> = subs
                .into_iter()
                .map(SmolStr::from)
                .collect::<Vec<_>>()
                .into_boxed_slice();
            SarCompartment::new(id, sub_boxes)
        })
}

fn arb_sar_program() -> impl Strategy<Value = SarProgram> {
    (
        arb_sar_program_id(),
        proptest::collection::vec(arb_sar_compartment(), 0..=2),
    )
        .prop_map(|(id, comps)| SarProgram::new(id, comps.into_boxed_slice()))
}

fn arb_sar_marking() -> impl Strategy<Value = SarMarking> {
    proptest::collection::vec(arb_sar_program(), 1..=3)
        .prop_map(|progs| SarMarking::new(SarIndicator::Abbrev, progs.into_boxed_slice()))
}

fn arb_sar_set() -> impl Strategy<Value = SarSet> {
    prop_oneof![
        Just(SarSet::empty()),
        arb_sar_marking().prop_map(|m| SarSet::from_marking(Some(&m))),
    ]
}

// ---------------------------------------------------------------------------
// FgiSet strategy
// ---------------------------------------------------------------------------

static VALID_COUNTRY_CODES: &[[u8; 3]] = &[
    *b"USA", *b"GBR", *b"CAN", *b"AUS", *b"NZL", *b"DEU", *b"FRA", *b"JPN",
];

fn arb_country_code() -> impl Strategy<Value = CountryCode> {
    (0..VALID_COUNTRY_CODES.len()).prop_map(|i| {
        CountryCode::try_new(&VALID_COUNTRY_CODES[i]).expect("static country codes are valid")
    })
}

fn arb_fgi_set() -> impl Strategy<Value = FgiSet> {
    prop_oneof![
        Just(FgiSet::None),
        Just(FgiSet::empty()),
        proptest::collection::vec(arb_country_code(), 0..=4).prop_map(|countries| {
            // Deduplicate — FgiMarker doesn't require uniqueness but the lattice
            // operates on sets; duplicate codes don't change the semantic.
            let mut deduped = countries;
            deduped.sort_by_key(|c| c.as_str().to_owned());
            deduped.dedup_by_key(|c| c.as_str().to_owned());
            // The 0-length sample is the lawful source-concealed FGI banner
            // form (CAPCO §H.7 p122); non-empty samples are source-
            // acknowledged. Post-FR-017 these are distinct enum variants,
            // not a shared shape, so the lattice strategy reflects that.
            match FgiMarker::acknowledged(deduped) {
                Some(m) => FgiSet::from_marker(Some(&m)),
                None => FgiSet::from_marker(Some(&FgiMarker::SourceConcealed)),
            }
        }),
        Just(FgiSet::Present {
            concealed: true,
            countries: std::collections::BTreeSet::new(),
        }),
    ]
}

// ---------------------------------------------------------------------------
// SciSet laws
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn sci_join_idempotent(a in arb_sci_set()) {
        prop_assert_eq!(a.join(&a), a);
    }

    #[test]
    fn sci_join_commutative(a in arb_sci_set(), b in arb_sci_set()) {
        prop_assert_eq!(a.join(&b), b.join(&a));
    }

    #[test]
    fn sci_join_associative(a in arb_sci_set(), b in arb_sci_set(), c in arb_sci_set()) {
        prop_assert_eq!(a.join(&b).join(&c), a.join(&b.join(&c)));
    }

    #[test]
    fn sci_join_empty_identity(a in arb_sci_set()) {
        let empty = SciSet::empty();
        prop_assert_eq!(a.join(&empty), a.clone());
        prop_assert_eq!(empty.join(&a), a);
    }

    #[test]
    fn sci_meet_idempotent(a in arb_sci_set()) {
        prop_assert_eq!(a.meet(&a), a);
    }

    #[test]
    fn sci_meet_commutative(a in arb_sci_set(), b in arb_sci_set()) {
        prop_assert_eq!(a.meet(&b), b.meet(&a));
    }

    #[test]
    fn sci_meet_associative(a in arb_sci_set(), b in arb_sci_set(), c in arb_sci_set()) {
        prop_assert_eq!(a.meet(&b).meet(&c), a.meet(&b.meet(&c)));
    }

    // empty is the bottom element: meet with empty absorbs to empty.
    #[test]
    fn sci_meet_bottom_absorbs(a in arb_sci_set()) {
        prop_assert_eq!(a.meet(&SciSet::empty()), SciSet::empty());
    }

    // Absorption: a ⊔ (a ⊓ b) = a  and  a ⊓ (a ⊔ b) = a.
    #[test]
    fn sci_absorption(a in arb_sci_set(), b in arb_sci_set()) {
        prop_assert_eq!(a.join(&a.meet(&b)), a.clone(), "join-over-meet absorption failed");
        prop_assert_eq!(a.meet(&a.join(&b)), a, "meet-over-join absorption failed");
    }
}

// ---------------------------------------------------------------------------
// SarSet laws
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn sar_join_idempotent(a in arb_sar_set()) {
        prop_assert_eq!(a.join(&a), a);
    }

    #[test]
    fn sar_join_commutative(a in arb_sar_set(), b in arb_sar_set()) {
        prop_assert_eq!(a.join(&b), b.join(&a));
    }

    #[test]
    fn sar_join_associative(a in arb_sar_set(), b in arb_sar_set(), c in arb_sar_set()) {
        prop_assert_eq!(a.join(&b).join(&c), a.join(&b.join(&c)));
    }

    #[test]
    fn sar_join_empty_identity(a in arb_sar_set()) {
        let empty = SarSet::empty();
        prop_assert_eq!(a.join(&empty), a.clone());
        prop_assert_eq!(empty.join(&a), a);
    }

    #[test]
    fn sar_meet_idempotent(a in arb_sar_set()) {
        prop_assert_eq!(a.meet(&a), a);
    }

    #[test]
    fn sar_meet_commutative(a in arb_sar_set(), b in arb_sar_set()) {
        prop_assert_eq!(a.meet(&b), b.meet(&a));
    }

    #[test]
    fn sar_meet_associative(a in arb_sar_set(), b in arb_sar_set(), c in arb_sar_set()) {
        prop_assert_eq!(a.meet(&b).meet(&c), a.meet(&b.meet(&c)));
    }

    // empty is the bottom element: meet with empty absorbs to empty.
    #[test]
    fn sar_meet_bottom_absorbs(a in arb_sar_set()) {
        prop_assert_eq!(a.meet(&SarSet::empty()), SarSet::empty());
    }

    // Absorption: a ⊔ (a ⊓ b) = a  and  a ⊓ (a ⊔ b) = a.
    #[test]
    fn sar_absorption(a in arb_sar_set(), b in arb_sar_set()) {
        prop_assert_eq!(a.join(&a.meet(&b)), a.clone(), "join-over-meet absorption failed");
        prop_assert_eq!(a.meet(&a.join(&b)), a, "meet-over-join absorption failed");
    }
}

// ---------------------------------------------------------------------------
// FgiSet laws
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn fgi_join_idempotent(a in arb_fgi_set()) {
        prop_assert_eq!(a.join(&a), a);
    }

    #[test]
    fn fgi_join_commutative(a in arb_fgi_set(), b in arb_fgi_set()) {
        prop_assert_eq!(a.join(&b), b.join(&a));
    }

    #[test]
    fn fgi_join_associative(a in arb_fgi_set(), b in arb_fgi_set(), c in arb_fgi_set()) {
        prop_assert_eq!(a.join(&b).join(&c), a.join(&b.join(&c)));
    }

    #[test]
    fn fgi_bottom_is_join_identity(a in arb_fgi_set()) {
        // B-1 (PR 4b-B 8th-pass): FgiSet no longer implements
        // `BoundedLattice` (open-vocab CountryCode axis). Use the
        // `empty()` constructor for the lattice bottom; semantically
        // identical to the retired `BoundedLattice::bottom()` call.
        let bot = FgiSet::empty();
        prop_assert_eq!(a.join(&bot), a.clone());
        prop_assert_eq!(bot.join(&a), a);
    }

    #[test]
    fn fgi_meet_idempotent(a in arb_fgi_set()) {
        prop_assert_eq!(a.meet(&a), a);
    }

    #[test]
    fn fgi_meet_commutative(a in arb_fgi_set(), b in arb_fgi_set()) {
        prop_assert_eq!(a.meet(&b), b.meet(&a));
    }

    #[test]
    fn fgi_bottom_absorbs_meet(a in arb_fgi_set()) {
        // B-1: use `empty()` after `BoundedLattice` retirement.
        prop_assert_eq!(a.meet(&FgiSet::empty()), FgiSet::empty());
    }

    // `fgi_top_propagates_join` retired in B-1 (PR 4b-B 8th-pass).
    // `SourceConcealed` IS a syntactic supersession-top for the join
    // operation, but `FgiSet` no longer implements `BoundedLattice` per
    // the open-vocab `CountryCode` precedent — see the doc comment on
    // `FgiSet` in `crates/capco/src/lattice.rs`. The supersession
    // semantic is still exercised by `fgi_concealment_monotone` below.

    // If the join of two sets has concealed=true, meet must not un-conceal.
    #[test]
    fn fgi_concealment_monotone(a in arb_fgi_set(), b in arb_fgi_set()) {
        let joined = a.join(&b);
        let is_concealed = matches!(&joined, FgiSet::Present { concealed: true, .. });
        if is_concealed {
            let met = a.meet(&b);
            // meet of a concealed-join result must not be Present{concealed:false}
            prop_assert!(
                !matches!(met, FgiSet::Present { concealed: false, .. }),
                "meet un-concealed after concealed join: a={a:?}, b={b:?}",
            );
        }
    }

    #[test]
    fn fgi_meet_associative(a in arb_fgi_set(), b in arb_fgi_set(), c in arb_fgi_set()) {
        prop_assert_eq!(a.meet(&b).meet(&c), a.meet(&b.meet(&c)));
    }

    // Join-over-meet absorption: a ⊔ (a ⊓ b) = a (holds unconditionally).
    //
    // Note: the symmetric meet-over-join direction (`a ⊓ (a ⊔ b) = a`) does NOT
    // hold when `b` carries CAPCO's source-concealment flag, because join with a
    // concealed element produces a concealed result, and the subsequent meet
    // intersects country sets with the empty set, collapsing to `None`. This is
    // an intentional deviation from standard lattice absorption, documented in
    // the `FgiSet` impl and driven by CAPCO §3.3a concealment-supersession
    // policy. The `fgi_concealment_monotone` test above verifies the policy
    // holds; do not add `meet_over_join_absorption` or `top_is_meet_identity`
    // tests for `FgiSet` — they will fail by design.
    #[test]
    fn fgi_join_over_meet_absorption(a in arb_fgi_set(), b in arb_fgi_set()) {
        prop_assert_eq!(a.join(&a.meet(&b)), a);
    }
}
