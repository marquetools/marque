// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Property-based lattice law verification for CAPCO structural lattice types.
//!
//! Complements `lattice_laws.rs` (fixed-sample exhaustive cross-product) by
//! covering the same algebraic laws over generated inputs, exercising the much
//! larger space of compartment-tree combinations that the fixed samples can't
//! reach.

use marque_capco::lattice::{DisplayOnlyBlock, FgiSet, RelToBlock, SarSet, SciSet};
use marque_ism::{
    CanonicalAttrs, CountryCode, FgiMarker, MarkingClassification, SarCompartment, SarIndicator,
    SarMarking, SarProgram, SciCompartment, SciControlBare, SciControlSystem, SciMarking,
};
use marque_scheme::{JoinSemilattice, MeetSemilattice};
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

    // If join of two sets is concealed, meet of those same two sets must NOT
    // produce Present{concealed:false}. With P-9-1's fix, meet(concealed, x)
    // returns x (the acknowledged side), so if a is concealed and b is
    // acknowledged, meet(a,b) = b (acknowledged) — that is correct because we
    // are meeting the inputs (a,b), not the join result. The concealment-
    // monotone property we check here is: if join(a,b) is concealed, then
    // neither a nor b can produce a false-un-conceal via meet.
    #[test]
    fn fgi_concealment_monotone(a in arb_fgi_set(), b in arb_fgi_set()) {
        let joined = a.join(&b);
        let is_concealed = matches!(&joined, FgiSet::Present { concealed: true, .. });
        if is_concealed {
            // At least one of a or b carries the concealed flag. The meet of
            // the two inputs must not produce Present{concealed:false} when
            // BOTH inputs are concealed (top ⊓ top = top). When exactly one
            // is concealed (a = top), meet(a,b) = b — which is acknowledged,
            // not false-un-concealing. So the property we assert: meet of two
            // inputs whose join is concealed should not claim concealed=false
            // when both inputs themselves were concealed.
            let a_concealed = matches!(&a, FgiSet::Present { concealed: true, .. });
            let b_concealed = matches!(&b, FgiSet::Present { concealed: true, .. });
            if a_concealed && b_concealed {
                let met = a.meet(&b);
                prop_assert!(
                    matches!(met, FgiSet::Present { concealed: true, .. }),
                    "meet of two concealed FgiSets must remain concealed: a={a:?}, b={b:?}, met={met:?}",
                );
            }
        }
    }

    #[test]
    fn fgi_meet_associative(a in arb_fgi_set(), b in arb_fgi_set(), c in arb_fgi_set()) {
        prop_assert_eq!(a.meet(&b).meet(&c), a.meet(&b.meet(&c)));
    }

    // P-9-1 (9th-pass): BOTH absorption laws now hold for `FgiSet`.
    //
    // Join-over-meet: a ⊔ (a ⊓ b) = a (holds unconditionally, same as before).
    //
    // Meet-over-join: a ⊓ (a ⊔ b) = a — this also holds after P-9-1 fixed
    // `FgiSet::meet` to treat the source-concealed form as lattice TOP. The
    // prior comment said meet-over-join was "an intentional deviation"; that was
    // written before P-1 (8th-pass) made concealed dominate on join. Once
    // concealed is join-top, the dual absorption law requires meet(x, top) = x,
    // which P-9-1 implements. Both absorption laws now hold over the full state
    // space. Authority: §H.7 p128. Verified 2026-05-16.
    #[test]
    fn fgi_join_over_meet_absorption(a in arb_fgi_set(), b in arb_fgi_set()) {
        prop_assert_eq!(a.join(&a.meet(&b)), a);
    }

    // Meet-over-join absorption: a ⊓ (a ⊔ b) = a (holds after P-9-1 fix).
    #[test]
    fn fgi_meet_over_join_absorption(a in arb_fgi_set(), b in arb_fgi_set()) {
        prop_assert_eq!(a.meet(&a.join(&b)), a);
    }
}

// ---------------------------------------------------------------------------
// RelToBlock laws (PR 4b-D.2 Copilot R1 / D24)
//
// The Copilot R1 review (decisions.md D24) flagged that `CapcoMarking`'s
// prior `JoinSemilattice` impl violated structural-`Eq` idempotence
// whenever `RelToBlock`'s tetragraph expansion fired. The lattice
// consultant verdict: `RelToBlock` IS a sound lattice on its native
// post-expansion domain (BTreeSet over trigraphs); the unsoundness was
// the cross-axis fold (`CapcoMarking`) claiming the law on a
// representation-finer structural `Eq`. PR 4b-D.2 drops the
// cross-axis claim and pins the per-axis claim with these proptests —
// which had no `RelToBlock` coverage before this PR.
//
// Strategy: build `Lattice { countries }` over a small CountryCode
// pool, plus `Bottom`, `Empty`, and `NofornSuperseded` as absorbing /
// identity states. Tetragraph atoms are NOT generated as inputs
// because `RelToBlock` lives on the expanded domain; tetragraph
// expansion happens at `from_attrs_iter` time before the lattice
// state is built. The strategy reflects what the lattice actually
// sees.
// ---------------------------------------------------------------------------

fn arb_rel_to_block() -> impl Strategy<Value = RelToBlock> {
    prop_oneof![
        Just(RelToBlock::Bottom),
        Just(RelToBlock::Empty),
        Just(RelToBlock::NofornSuperseded),
        proptest::collection::vec(arb_country_code(), 1..=4).prop_map(|countries| {
            let set: std::collections::BTreeSet<CountryCode> = countries.into_iter().collect();
            RelToBlock::Lattice { countries: set }
        }),
    ]
}

proptest! {
    // Join laws.
    #[test]
    fn rel_to_join_idempotent(a in arb_rel_to_block()) {
        prop_assert_eq!(a.join(&a), a);
    }

    #[test]
    fn rel_to_join_commutative(a in arb_rel_to_block(), b in arb_rel_to_block()) {
        prop_assert_eq!(a.join(&b), b.join(&a));
    }

    #[test]
    fn rel_to_join_associative(
        a in arb_rel_to_block(),
        b in arb_rel_to_block(),
        c in arb_rel_to_block(),
    ) {
        prop_assert_eq!(a.join(&b).join(&c), a.join(&b.join(&c)));
    }

    #[test]
    fn rel_to_join_bottom_identity(a in arb_rel_to_block()) {
        let bottom = RelToBlock::Bottom;
        prop_assert_eq!(a.join(&bottom), a.clone());
        prop_assert_eq!(bottom.join(&a), a);
    }

    // Meet laws.
    #[test]
    fn rel_to_meet_idempotent(a in arb_rel_to_block()) {
        prop_assert_eq!(a.meet(&a), a);
    }

    #[test]
    fn rel_to_meet_commutative(a in arb_rel_to_block(), b in arb_rel_to_block()) {
        prop_assert_eq!(a.meet(&b), b.meet(&a));
    }

    #[test]
    fn rel_to_meet_associative(
        a in arb_rel_to_block(),
        b in arb_rel_to_block(),
        c in arb_rel_to_block(),
    ) {
        prop_assert_eq!(a.meet(&b).meet(&c), a.meet(&b.meet(&c)));
    }

    // Absorption laws (both directions hold on the meet-bottom +
    // join-top setup: `Bottom` is meet-absorbing and join-identity;
    // `NofornSuperseded` is join-top per the doc comment + 11th-pass
    // fix at lattice.rs:3095-3125).
    #[test]
    fn rel_to_join_over_meet_absorption(a in arb_rel_to_block(), b in arb_rel_to_block()) {
        prop_assert_eq!(a.join(&a.meet(&b)), a);
    }

    #[test]
    fn rel_to_meet_over_join_absorption(a in arb_rel_to_block(), b in arb_rel_to_block()) {
        prop_assert_eq!(a.meet(&a.join(&b)), a);
    }
}

// ---------------------------------------------------------------------------
// DisplayOnlyBlock laws (PR 4b-E review fix-up — rust-reviewer H-2 +
// lattice-consultant L-5)
//
// `DisplayOnlyBlock` is a new `JoinSemilattice` implementor introduced
// in PR 4b-E. The inline `#[cfg(test)]` suite in `lattice.rs` covers
// associativity, identity-with-bottom, empty-absorbs, and
// NofornSuperseded-absorbs at fixed samples; the proptest suite below
// pins commutativity, idempotence, and associativity over arbitrary
// generated inputs.
//
// The strategy mirrors `RelToBlock`'s 4-variant shape (Bottom / Empty /
// NofornSuperseded / Lattice{countries}) so the same absorbing-element
// structure is exercised. Tetragraph expansion is NOT generated as a
// strategy input — `DisplayOnlyBlock` lives on the post-expansion
// trigraph domain; expansion happens at `from_attrs_iter` time before
// the lattice state is built (same precedent as `RelToBlock`).
// ---------------------------------------------------------------------------

fn arb_display_only_block() -> impl Strategy<Value = DisplayOnlyBlock> {
    prop_oneof![
        Just(DisplayOnlyBlock::Bottom),
        Just(DisplayOnlyBlock::Empty),
        Just(DisplayOnlyBlock::NofornSuperseded),
        proptest::collection::vec(arb_country_code(), 1..=4).prop_map(|countries| {
            let set: std::collections::BTreeSet<CountryCode> = countries.into_iter().collect();
            DisplayOnlyBlock::Lattice { countries: set }
        }),
    ]
}

proptest! {
    // Join laws — commutativity, idempotence, associativity, identity-
    // with-bottom. The H-1 root-cause (non-commutative `join` body)
    // would surface immediately under `display_only_block_join_commutative`
    // if it ever recurs.
    #[test]
    fn display_only_block_join_idempotent(a in arb_display_only_block()) {
        prop_assert_eq!(a.join(&a), a);
    }

    #[test]
    fn display_only_block_join_commutative(
        a in arb_display_only_block(),
        b in arb_display_only_block(),
    ) {
        prop_assert_eq!(a.join(&b), b.join(&a));
    }

    #[test]
    fn display_only_block_join_associative(
        a in arb_display_only_block(),
        b in arb_display_only_block(),
        c in arb_display_only_block(),
    ) {
        prop_assert_eq!(a.join(&b).join(&c), a.join(&b.join(&c)));
    }

    #[test]
    fn display_only_block_join_bottom_identity(a in arb_display_only_block()) {
        let bottom = DisplayOnlyBlock::Bottom;
        prop_assert_eq!(a.join(&bottom), a.clone());
        prop_assert_eq!(bottom.join(&a), a);
    }
}

// ---------------------------------------------------------------------------
// FgiSet::from_attrs_iter — proptest coverage of the new constructor
// (PR 4b-E review fix-up — rust-reviewer H-2)
//
// The existing `fgi_join_*` proptests use `FgiSet::from_marker` and
// only exercise the `FgiSet` algebra in isolation. PR 4b-E introduced
// `FgiSet::from_attrs_iter` as the production page-rollup constructor.
// The proptests below pin (a) "bulk construction agrees with iterated
// per-portion construction" and (b) "concealment dominates" over
// arbitrary multi-portion inputs — extending the `FgiSet` coverage to
// the actual production entry point.
// ---------------------------------------------------------------------------

fn arb_portion_with_fgi_marker() -> impl Strategy<Value = CanonicalAttrs> {
    // Two-branch strategy: portions carrying explicit `FgiMarker`
    // values, and bare portions with no FGI axis at all. The
    // `from_attrs_iter` semantic differs across these branches
    // (acknowledged-vs-concealed-dominates, contribution-vs-no-op).
    prop_oneof![
        Just({
            let mut p = CanonicalAttrs::default();
            p.fgi_marker = Some(FgiMarker::SourceConcealed);
            p
        }),
        proptest::collection::vec(arb_country_code(), 1..=3).prop_map(|countries| {
            let mut p = CanonicalAttrs::default();
            // `acknowledged()` returns `Option<FgiMarker>` — only
            // populated when the country list is non-empty. The
            // 1..=3 size bound above guarantees Some.
            p.fgi_marker = FgiMarker::acknowledged(countries);
            p
        }),
        // Bare portion with classification-derived contribution: a
        // JOINT classification with at least one non-US producer.
        // Exercises the `Classification::Joint` branch of
        // `from_attrs_iter` (which strips USA and contributes the
        // remaining producers).
        proptest::collection::vec(arb_country_code(), 1..=3).prop_map(|countries| {
            let mut p = CanonicalAttrs::default();
            p.classification = Some(MarkingClassification::Joint(
                marque_ism::JointClassification {
                    level: marque_ism::Classification::Secret,
                    countries: countries.into_boxed_slice(),
                },
            ));
            p
        }),
        // Portion with no FGI axis at all — contributes nothing.
        Just(CanonicalAttrs::default()),
    ]
}

proptest! {
    // Concealed-dominates invariant — §H.7 p128. If any portion in
    // the input carries `FgiMarker::SourceConcealed`, the resulting
    // `FgiSet` must be `Present { concealed: true, .. }` regardless
    // of the other portions.
    #[test]
    fn fgi_set_from_attrs_iter_concealed_dominates(
        portions in proptest::collection::vec(arb_portion_with_fgi_marker(), 1..=4),
    ) {
        let any_concealed = portions.iter().any(|p| {
            matches!(p.fgi_marker, Some(FgiMarker::SourceConcealed))
        });
        let result = FgiSet::from_attrs_iter(&portions);
        if any_concealed {
            prop_assert!(
                matches!(
                    result,
                    FgiSet::Present { concealed: true, .. }
                ),
                "concealed portion in input must yield concealed result: \
                 result={result:?}",
            );
        }
    }

    // Bulk construction agrees with iterated join. The
    // `from_attrs_iter` constructor walks portions in document order
    // and unions per-portion contributions; assembling per-portion
    // singletons via repeated `FgiSet::join` should produce the same
    // `FgiSet` value. This pins the "constructor is a fold over
    // join" property at the production entry point.
    #[test]
    fn fgi_set_from_attrs_iter_agrees_with_iterated_join(
        portions in proptest::collection::vec(arb_portion_with_fgi_marker(), 1..=4),
    ) {
        let bulk = FgiSet::from_attrs_iter(&portions);
        let stepped = portions
            .iter()
            .map(|p| FgiSet::from_attrs_iter(std::slice::from_ref(p)))
            .fold(FgiSet::empty(), |acc, x| acc.join(&x));
        prop_assert_eq!(bulk, stepped);
    }
}
