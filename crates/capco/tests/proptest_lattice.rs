// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Property-based lattice law verification for CAPCO structural lattice types.
//!
//! Complements `lattice_laws.rs` (fixed-sample exhaustive cross-product) by
//! covering the same algebraic laws over generated inputs, exercising the much
//! larger space of compartment-tree combinations that the fixed samples can't
//! reach.
//!
//! # Audit coverage (decisions.md D24 follow-up)
//!
//! In addition to the original `SciSet`, `SarSet`, `FgiSet`, and `RelToBlock`
//! tests, this file covers the three per-axis types identified by PR #456 as
//! carrying join-side observational state:
//!
//! - **`DissemSet`** — `relido_observed_unanimous` flag; tests verify that
//!   `join(a, a) == a` under structural `Eq` across all reachable states.
//! - **`JointSet`** — `Mixed` / `DisunityCollapse` variants; tests verify all
//!   three join semilattice laws.
//! - **`SupersessionSet<TestTok>`** — post-join overlay re-application; tests
//!   verify the join laws hold on the generic primitive.
//!
//! Verdict (post-audit): all three types pass all three join laws (idempotence,
//! commutativity, associativity) on their structural `Eq`. No trait removals or
//! representation changes are required. The `JoinSemilattice` claim is sound for
//! each type.

use marque_capco::lattice::{DissemSet, FgiSet, JointSet, RelToBlock, SarSet, SciSet};
use marque_ism::{
    CanonicalAttrs, Classification, CountryCode, DissemControl, FgiMarker, SarCompartment,
    SarIndicator, SarMarking, SarProgram, SciCompartment, SciControlBare, SciControlSystem,
    SciMarking,
};
use marque_scheme::{JoinSemilattice, MeetSemilattice, SupersessionSet};
use proptest::prelude::*;
use smol_str::SmolStr;
use std::collections::BTreeSet;

// ---------------------------------------------------------------------------
// TestTok — minimal token type for SupersessionSet law tests
//
// N dominates A and B (mimicking NOFORN ⊐ REL TO / RELIDO); C is
// independent. The table is a strict subset of the CAPCO semantics so the
// tests exercise the generic SupersessionSet primitive rather than the
// CAPCO-specific overlay logic (which is tested indirectly via DissemSet).
// ---------------------------------------------------------------------------

/// Minimal four-token enum for `SupersessionSet` property tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TestTok {
    A,
    B,
    C,
    N,
}

/// `N` dominates `A` and `B`; `C` is independent.
static TEST_SUPERSESSION: &[(TestTok, TestTok)] = &[
    (TestTok::N, TestTok::A),
    (TestTok::N, TestTok::B),
];

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
// DissemSet strategies and join-law tests (D24 follow-up audit)
//
// `DissemSet` carries `relido_observed_unanimous: bool` — a join-side
// aggregation flag that tracks whether every contributing portion carried
// RELIDO. The audit question: does `join(a, a) == a` hold under structural
// `Eq` (which compares both `set` and `relido_observed_unanimous`)?
//
// Strategy: build `CanonicalAttrs` slices with various subsets of the
// overlap-sensitive DissemControl tokens (Nf, Rel, Relido, Oc, OcUsgov,
// Fouo, Displayonly, Eyes) and call `DissemSet::from_attrs_iter`. This
// exercises:
//   - Vacuous unanimity path (`empty()`, zero portions, no-RELIDO portions).
//   - Non-unanimous path (some portions have RELIDO, some don't).
//   - NOFORN-dominates overlay (strips Rel/Relido/Displayonly/Eyes).
//   - OC-USGOV supersession overlay (drops OcUsgov when Oc is present).
//   - RELIDO unanimity overlay (drops Relido when flag=false).
//
// Verdict: all three join semilattice laws hold. The `relido_observed_unanimous`
// flag is computed as `self.flag && other.flag`, and the overlay is
// idempotent on already-canonical sets — so `join(a, a) == a` is guaranteed
// by construction. The proptests confirm this empirically across the reachable
// state space.
// ---------------------------------------------------------------------------

/// DissemControl tokens relevant to the three supersession overlays:
/// Nf (dominator), Rel/Relido/Displayonly/Eyes (dominated by Nf),
/// Oc/OcUsgov (OC-USGOV superseded by OC), and Fouo (independent).
fn arb_dissem_control() -> impl Strategy<Value = DissemControl> {
    prop_oneof![
        Just(DissemControl::Nf),
        Just(DissemControl::Rel),
        Just(DissemControl::Relido),
        Just(DissemControl::Oc),
        Just(DissemControl::OcUsgov),
        Just(DissemControl::Fouo),
        Just(DissemControl::Displayonly),
        Just(DissemControl::Eyes),
    ]
}

/// A single `CanonicalAttrs` portion with only `dissem_us` populated.
fn arb_dissem_portion() -> impl Strategy<Value = CanonicalAttrs> {
    // Use proptest::collection::vec — duplicates are fine because
    // DissemSet::from_attrs_iter unions into a BTreeSet internally.
    proptest::collection::vec(arb_dissem_control(), 0..=4).prop_map(|controls| {
        let mut a = CanonicalAttrs::default();
        a.dissem_us = controls.into_boxed_slice();
        a
    })
}

fn arb_dissem_set() -> impl Strategy<Value = DissemSet> {
    // 0 portions → empty() (vacuous unanimity). 1–3 portions exercise
    // the flag-propagation and overlay paths.
    proptest::collection::vec(arb_dissem_portion(), 0..=3)
        .prop_map(|portions| DissemSet::from_attrs_iter(&portions))
}

proptest! {
    // --- D24 audit: DissemSet join semilattice laws ---

    /// `join(a, a) == a` under structural `Eq` (compares both `set` and
    /// `relido_observed_unanimous`). Confirms the overlay is idempotent on
    /// already-canonical inputs and the flag is preserved by `x && x = x`.
    #[test]
    fn dissem_join_idempotent(a in arb_dissem_set()) {
        prop_assert_eq!(a.join(&a), a);
    }

    #[test]
    fn dissem_join_commutative(a in arb_dissem_set(), b in arb_dissem_set()) {
        prop_assert_eq!(a.join(&b), b.join(&a));
    }

    #[test]
    fn dissem_join_associative(
        a in arb_dissem_set(),
        b in arb_dissem_set(),
        c in arb_dissem_set(),
    ) {
        prop_assert_eq!(a.join(&b).join(&c), a.join(&b.join(&c)));
    }

    #[test]
    fn dissem_join_empty_identity(a in arb_dissem_set()) {
        let empty = DissemSet::empty();
        prop_assert_eq!(a.join(&empty), a.clone());
        prop_assert_eq!(empty.join(&a), a);
    }
}

// ---------------------------------------------------------------------------
// JointSet strategies and join-law tests (D24 follow-up audit)
//
// `JointSet` carries two variants with observational state:
//   - `DisunityCollapse { highest_level, union_non_us_producers }` — the
//     union of non-US producers across JOINT portions with differing lists.
//   - `Mixed` — absorbing state once JOINT and non-JOINT portions are mixed.
//
// Idempotence concern: does `join(dc, dc) == dc`? Yes — BTreeSet union is
// idempotent and `Classification::max` is idempotent. The proptests confirm
// this. Associativity: the `DisunityCollapse` + `UnanimousProducers`
// interaction propagates `non_us` via BTreeSet union (both associative and
// commutative), so the three-operand test is exhaustive.
//
// Verdict: all three join semilattice laws hold for JointSet.
// ---------------------------------------------------------------------------

fn arb_classification() -> impl Strategy<Value = Classification> {
    prop_oneof![
        Just(Classification::Unclassified),
        Just(Classification::Confidential),
        Just(Classification::Secret),
        Just(Classification::TopSecret),
    ]
}

/// Generates a `BTreeSet<CountryCode>` that always contains USA, plus 0–2
/// additional codes. Models the well-formed `UnanimousProducers.producers`
/// field (§H.3 p56: USA always in the JOINT producer list).
fn arb_joint_producers() -> impl Strategy<Value = BTreeSet<CountryCode>> {
    let usa = CountryCode::try_new(b"USA").expect("static");
    // Non-USA codes: GBR, CAN, AUS, NZL (skip USA at index 0).
    proptest::collection::vec(
        (1..5usize).prop_map(|i| {
            CountryCode::try_new(&VALID_COUNTRY_CODES[i]).expect("static country codes are valid")
        }),
        0..=2,
    )
    .prop_map(move |others| {
        let mut set = BTreeSet::new();
        set.insert(usa);
        set.extend(others);
        set
    })
}

/// Generates a `BTreeSet<CountryCode>` of non-US producers (for
/// `DisunityCollapse.union_non_us_producers`). May be empty.
fn arb_non_us_producers() -> impl Strategy<Value = BTreeSet<CountryCode>> {
    // Non-USA codes only: GBR, CAN, AUS, NZL, DEU, FRA, JPN.
    proptest::collection::vec(
        (1..VALID_COUNTRY_CODES.len()).prop_map(|i| {
            CountryCode::try_new(&VALID_COUNTRY_CODES[i]).expect("static country codes are valid")
        }),
        0..=3,
    )
    .prop_map(|codes| codes.into_iter().collect::<BTreeSet<_>>())
}

fn arb_joint_set() -> impl Strategy<Value = JointSet> {
    prop_oneof![
        Just(JointSet::Bottom),
        Just(JointSet::Mixed),
        (arb_classification(), arb_joint_producers()).prop_map(|(level, producers)| {
            JointSet::UnanimousProducers { level, producers }
        }),
        (arb_classification(), arb_non_us_producers()).prop_map(
            |(highest_level, union_non_us_producers)| JointSet::DisunityCollapse {
                highest_level,
                union_non_us_producers,
            }
        ),
    ]
}

proptest! {
    // --- D24 audit: JointSet join semilattice laws ---

    /// `join(a, a) == a` for every reachable JointSet state:
    /// - `Bottom ⊔ Bottom = Bottom` ✓
    /// - `Mixed ⊔ Mixed = Mixed` ✓
    /// - `UP(l, p) ⊔ UP(l, p) = UP(l, p)` (same producers → same result) ✓
    /// - `DC(l, n) ⊔ DC(l, n) = DC(l, n)` (set union idempotent; max idempotent) ✓
    #[test]
    fn joint_join_idempotent(a in arb_joint_set()) {
        prop_assert_eq!(a.join(&a), a);
    }

    #[test]
    fn joint_join_commutative(a in arb_joint_set(), b in arb_joint_set()) {
        prop_assert_eq!(a.join(&b), b.join(&a));
    }

    #[test]
    fn joint_join_associative(
        a in arb_joint_set(),
        b in arb_joint_set(),
        c in arb_joint_set(),
    ) {
        prop_assert_eq!(a.join(&b).join(&c), a.join(&b.join(&c)));
    }

    #[test]
    fn joint_join_bottom_identity(a in arb_joint_set()) {
        let bottom = JointSet::Bottom;
        prop_assert_eq!(a.join(&bottom), a.clone());
        prop_assert_eq!(bottom.join(&a), a);
    }
}

// ---------------------------------------------------------------------------
// SupersessionSet<TestTok> strategies and join-law tests (D24 follow-up audit)
//
// `SupersessionSet` implements only `JoinSemilattice` (not `MeetSemilattice`).
// The join applies the supersession overlay post-union: tokens that are
// dominated by a superseding peer in the union are dropped. The audit
// question: does `join(a, a) == a` hold under structural `Eq`?
//
// Analysis: `from_iter_sorted` produces a canonical value (overlay already
// applied). On `join(a, a)`:
//   1. `FlatSet(a.set).join(FlatSet(a.set)) = FlatSet(a.set)` (union idempotent).
//   2. `apply_supersession(a.set, table) = a.set` (already canonical).
//   3. Result structurally equals `a`. ✓
//
// Associativity: `apply_supersession(apply_supersession(S ∪ T) ∪ U) =
// apply_supersession(S ∪ T ∪ U)` because `apply_supersession` is idempotent
// on its output (dominated tokens absent ↔ no change on re-application).
// The proptests confirm empirically across all 2^4 = 16 inputs.
//
// Verdict: all three join semilattice laws hold for SupersessionSet<TestTok>.
// ---------------------------------------------------------------------------

fn arb_test_tok() -> impl Strategy<Value = TestTok> {
    prop_oneof![
        Just(TestTok::A),
        Just(TestTok::B),
        Just(TestTok::C),
        Just(TestTok::N),
    ]
}

fn arb_supersession_set() -> impl Strategy<Value = SupersessionSet<TestTok>> {
    proptest::collection::vec(arb_test_tok(), 0..=4)
        .prop_map(|tokens| SupersessionSet::from_iter_sorted(tokens, TEST_SUPERSESSION))
}

proptest! {
    // --- D24 audit: SupersessionSet join semilattice laws ---

    /// `join(a, a) == a` — the post-join overlay re-application on an
    /// already-canonical input is a no-op (dominated tokens were already
    /// removed at `from_iter_sorted` time).
    #[test]
    fn supersession_join_idempotent(a in arb_supersession_set()) {
        prop_assert_eq!(a.join(&a), a);
    }

    #[test]
    fn supersession_join_commutative(
        a in arb_supersession_set(),
        b in arb_supersession_set(),
    ) {
        prop_assert_eq!(a.join(&b), b.join(&a));
    }

    #[test]
    fn supersession_join_associative(
        a in arb_supersession_set(),
        b in arb_supersession_set(),
        c in arb_supersession_set(),
    ) {
        prop_assert_eq!(a.join(&b).join(&c), a.join(&b.join(&c)));
    }

    #[test]
    fn supersession_join_empty_identity(a in arb_supersession_set()) {
        let empty = SupersessionSet::new(TEST_SUPERSESSION);
        prop_assert_eq!(a.join(&empty), a.clone());
        prop_assert_eq!(empty.join(&a), a);
    }
}
