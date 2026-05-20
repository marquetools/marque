// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Round-trip proptests for `marque_capco::fact_bitmask`'s `derive_bits`
//! and `apply_closed_bits_to` projection helpers.
//!
//! PR-B scope: this file asserts the projection's algebraic properties
//! before PR-C lands the `CLOSURE_TABLE` and PR-D rewires
//! `CapcoScheme::closure` to use it. The closure-table laws
//! (idempotence, extensivity, monotonicity, convergence-bound — plan
//! §6 P1–P4) live in `proptest_closure_table.rs` in PR-C; the
//! cross-path parity gate against `scheme.closure(marking)` (plan §6
//! P5) lives in PR-D.
//!
//! The properties covered here are the load-bearing PR-B invariants:
//!
//! - **Deterministic projection.** `derive_bits(attrs) ==
//!   derive_bits(attrs.clone())`.
//! - **Zero-delta is a no-op.** Calling `apply_closed_bits_to` with
//!   `closed == input` leaves `attrs` byte-identical.
//! - **Apply is idempotent.** Calling `apply_closed_bits_to` twice
//!   with the same `(closed, input)` is equivalent to calling it
//!   once.
//! - **Ineligible-bit immunity.** `apply_closed_bits_to` is a no-op
//!   on `(closed - input)` bits outside `APPLY_ELIGIBLE_MASK`.
//! - **Eligible-bit additivity.** When the delta is exactly one
//!   eligible bit, the corresponding atom appears on its axis after
//!   `apply_closed_bits_to`.
//! - **Closed-vocab fields untouched on apply.** Axes not driven by
//!   the cone (`non_ic_dissem`, `aea_markings`, `sci_markings`,
//!   `sar_markings`, `classification`) are pristine after a closure-
//!   cone write-back.
//!
//! `proptest` is configured with a small case count (256) — the
//! generators are narrow strategies over CAPCO atoms, not full
//! `CanonicalAttrs` shapes, and 256 cases give >99% probability of
//! catching a single-atom regression.

use marque_capco::fact_bitmask::{apply_closed_bits_to, derive_bits, fact_bit};
use marque_ism::{
    AeaMarking, Classification, CountryCode, DissemControl, MarkingClassification, NonIcDissem,
    canonical::CanonicalAttrs,
};
use marque_scheme::FactBitmask;
use proptest::prelude::*;

/// Local mirror of `marque_capco::fact_bitmask::APPLY_ELIGIBLE_MASK`,
/// computed from the public `fact_bit::*` constants. Kept in sync by
/// the unit test in `fact_bitmask.rs` —
/// `mask_constants_are_disjoint_subsets_of_inventory` exercises the
/// same shape on the production constant.
const APPLY_ELIGIBLE_MASK: u128 = (1u128 << fact_bit::NOFORN)
    | (1u128 << fact_bit::ORCON)
    | (1u128 << fact_bit::RELIDO)
    | (1u128 << fact_bit::REL_TO_USA);

// ---------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------

fn arb_classification() -> impl Strategy<Value = Option<MarkingClassification>> {
    let us_arm = prop::sample::select(vec![
        Classification::Unclassified,
        Classification::Restricted,
        Classification::Confidential,
        Classification::Secret,
        Classification::TopSecret,
    ])
    .prop_map(|c| Some(MarkingClassification::Us(c)));

    let fgi_arm = prop::sample::select(vec![
        Classification::Restricted,
        Classification::Confidential,
        Classification::Secret,
        Classification::TopSecret,
    ])
    .prop_map(|level| {
        // Source-concealed FGI per §H.7 p122 (empty countries).
        Some(MarkingClassification::Fgi(marque_ism::FgiClassification {
            countries: Box::new([]),
            level,
        }))
    });

    prop_oneof![Just(None), us_arm, fgi_arm,]
}

fn arb_dissem_us() -> impl Strategy<Value = Vec<DissemControl>> {
    // Sample a small subset of CAPCO IC dissem atoms — closure-relevant
    // tokens only, in canonical order.
    prop::sample::subsequence(
        vec![
            DissemControl::Oc,
            DissemControl::OcUsgov,
            DissemControl::Nf,
            DissemControl::Relido,
            DissemControl::Displayonly,
            DissemControl::Eyes,
            DissemControl::Fouo,
            DissemControl::Pr,
            DissemControl::Rs,
            DissemControl::Imc,
        ],
        0..=4,
    )
}

fn arb_non_ic_dissem() -> impl Strategy<Value = Vec<NonIcDissem>> {
    prop::sample::subsequence(
        vec![
            NonIcDissem::Sbu,
            NonIcDissem::SbuNf,
            NonIcDissem::Les,
            NonIcDissem::LesNf,
            NonIcDissem::Limdis,
            NonIcDissem::Nodis,
            NonIcDissem::Exdis,
            NonIcDissem::Ssi,
            NonIcDissem::Nnpi,
        ],
        0..=3,
    )
}

fn arb_aea() -> impl Strategy<Value = Vec<AeaMarking>> {
    prop::sample::subsequence(
        vec![
            AeaMarking::Tfni,
            AeaMarking::DodUcni,
            AeaMarking::DoeUcni,
            // RD / FRD carry compound blocks; the bitmask only reads
            // the variant tag, so a default block is sufficient.
            AeaMarking::Rd(marque_ism::RdBlock::default()),
            AeaMarking::Frd(marque_ism::FrdBlock::default()),
        ],
        0..=3,
    )
}

fn arb_rel_to() -> impl Strategy<Value = Vec<CountryCode>> {
    prop::sample::subsequence(
        vec![
            CountryCode::USA,
            CountryCode::GBR,
            CountryCode::CAN,
            CountryCode::AUS,
            CountryCode::NZL,
        ],
        0..=4,
    )
}

/// A focused strategy generating realistic `CanonicalAttrs` shapes
/// from the closed-vocab axes the bitmask reads. Open-vocab axes
/// (SCI compartments, SAR programs, custom country codes) are not
/// exercised here — their presence is captured as sentinels
/// (`SCI_PRESENT`, `SAR_PRESENT`) which PR-C's closure-table tests
/// will cover end-to-end.
fn arb_attrs() -> impl Strategy<Value = CanonicalAttrs> {
    (
        arb_classification(),
        arb_dissem_us(),
        arb_non_ic_dissem(),
        arb_aea(),
        arb_rel_to(),
    )
        .prop_map(|(cls, dus, nid, aea, rel)| {
            let mut a = CanonicalAttrs::default();
            a.classification = cls;
            a.dissem_us = dus.into();
            a.non_ic_dissem = nid.into();
            a.aea_markings = aea.into();
            a.rel_to = rel.into();
            a
        })
}

fn arb_eligible_bit() -> impl Strategy<Value = u32> {
    prop::sample::select(vec![
        fact_bit::NOFORN,
        fact_bit::ORCON,
        fact_bit::RELIDO,
        fact_bit::REL_TO_USA,
    ])
}

// ---------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        ..ProptestConfig::default()
    })]

    /// `derive_bits` is deterministic — projection over a cloned
    /// `CanonicalAttrs` produces the same bitmask.
    #[test]
    fn derive_is_deterministic(attrs in arb_attrs()) {
        let a = derive_bits(&attrs);
        let b = derive_bits(&attrs.clone());
        prop_assert_eq!(a, b);
    }

    /// `apply_closed_bits_to(attrs, derive_bits(attrs), derive_bits(attrs))`
    /// — a zero-delta call — leaves `attrs` byte-identical.
    #[test]
    fn apply_zero_delta_is_noop(attrs in arb_attrs()) {
        let mut a = attrs.clone();
        let bits = derive_bits(&a);
        apply_closed_bits_to(&mut a, bits, bits);
        prop_assert_eq!(a, attrs);
    }

    /// Calling `apply_closed_bits_to` twice with identical `(closed,
    /// input)` arguments is the same as calling it once
    /// (algebraic idempotence). The second call sees the same
    /// `(closed, input)` pair the first call saw — NOT a re-derived
    /// input from the mutated `attrs`. This is the load-bearing
    /// property: it catches a double-append regression in the
    /// in-axis presence check that a re-derive-then-apply pattern
    /// would silently mask (because the re-derived input would have
    /// already absorbed the first call's effect).
    #[test]
    fn apply_is_idempotent(attrs in arb_attrs(), bit in arb_eligible_bit()) {
        let input = derive_bits(&attrs);
        let closed = input.with_bit(bit);

        let mut a = attrs.clone();
        apply_closed_bits_to(&mut a, closed, input);
        let after_once = a.clone();

        // Second call with the SAME (closed, input) pair. The
        // presence guards in `apply_closed_bits_to` must short-
        // circuit so no second append happens.
        apply_closed_bits_to(&mut a, closed, input);
        prop_assert_eq!(a, after_once);
    }

    /// Setting a single eligible bit in `closed` adds the
    /// corresponding atom to its axis (or no-ops if already present).
    #[test]
    fn apply_eligible_bit_adds_atom(attrs in arb_attrs(), bit in arb_eligible_bit()) {
        let input = derive_bits(&attrs);
        let closed = input.with_bit(bit);
        let mut a = attrs.clone();
        apply_closed_bits_to(&mut a, closed, input);
        let after = derive_bits(&a);
        // The post-apply bitmask must have the eligible bit set.
        prop_assert!(
            after.is_set(bit),
            "post-apply bitmask missing bit {}",
            bit,
        );
    }

    /// `apply_closed_bits_to` does not modify axes that the
    /// closure-cone outputs cannot reach: `non_ic_dissem`,
    /// `aea_markings`, `sci_markings`, `sar_markings`, and
    /// `classification` are preserved verbatim.
    #[test]
    fn apply_preserves_non_cone_axes(attrs in arb_attrs(), bit in arb_eligible_bit()) {
        let input = derive_bits(&attrs);
        let closed = input.with_bit(bit);
        let mut a = attrs.clone();
        apply_closed_bits_to(&mut a, closed, input);
        prop_assert_eq!(&a.non_ic_dissem, &attrs.non_ic_dissem);
        prop_assert_eq!(&a.aea_markings, &attrs.aea_markings);
        prop_assert_eq!(&a.sci_markings, &attrs.sci_markings);
        prop_assert_eq!(&a.sar_markings, &attrs.sar_markings);
        prop_assert_eq!(&a.classification, &attrs.classification);
    }

    /// Ineligible delta bits (outside `APPLY_ELIGIBLE_MASK`) are
    /// silently ignored — the closed-vocab fields stay pristine.
    #[test]
    fn apply_ignores_ineligible_delta_bits(
        attrs in arb_attrs(),
        ineligible_bits in any::<u128>(),
    ) {
        let input = derive_bits(&attrs);
        // Mask out the eligible cone-output bits so we test only
        // the ineligible portion of the delta.
        let closed_raw = input.bits() | (ineligible_bits & !APPLY_ELIGIBLE_MASK);
        let closed = FactBitmask::from_bits(closed_raw);

        let mut a = attrs.clone();
        apply_closed_bits_to(&mut a, closed, input);
        prop_assert_eq!(a, attrs);
    }

    /// Eligible bits already in `input` — i.e. atoms already present
    /// on `attrs` — are NOT a delta and cause no mutation.
    #[test]
    fn apply_no_delta_on_already_present(attrs in arb_attrs()) {
        let mut a = attrs.clone();
        let bits = derive_bits(&a);
        // closed == input → delta empty.
        apply_closed_bits_to(&mut a, bits, bits);
        // Now spike `closed` with any eligible bit; if the atom was
        // already present (input.is_set(bit) was true), no mutation.
        for &bit in &[
            fact_bit::NOFORN,
            fact_bit::ORCON,
            fact_bit::RELIDO,
            fact_bit::REL_TO_USA,
        ] {
            if bits.is_set(bit) {
                let mut a2 = attrs.clone();
                apply_closed_bits_to(&mut a2, bits.with_bit(bit), bits);
                prop_assert_eq!(
                    &a2,
                    &attrs,
                    "spurious mutation for already-present bit {}",
                    bit,
                );
            }
        }
    }

    /// `derive_bits ∘ apply_closed_bits_to ⊑` is a Galois connection
    /// fragment: every bit in `(closed & APPLY_ELIGIBLE_MASK)` shows
    /// up in `derive_bits(apply(...))`, and no eligible bit not in
    /// `closed` appears that wasn't already in `input`.
    #[test]
    fn apply_then_derive_recovers_eligible_bits(
        attrs in arb_attrs(),
        cone_bits in any::<u128>(),
    ) {
        let input = derive_bits(&attrs);
        let closed = FactBitmask::from_bits(input.bits() | (cone_bits & APPLY_ELIGIBLE_MASK));
        let mut a = attrs.clone();
        apply_closed_bits_to(&mut a, closed, input);
        let after = derive_bits(&a);

        // Every eligible bit in `closed` must be set in `after`.
        let recovered_eligible = after.bits() & APPLY_ELIGIBLE_MASK;
        let expected_eligible = closed.bits() & APPLY_ELIGIBLE_MASK;
        prop_assert_eq!(
            recovered_eligible & expected_eligible,
            expected_eligible,
            "lost an eligible bit on round-trip",
        );
    }

    /// FGI projection invariant: any `MarkingClassification::Fgi(_)`
    /// in `arb_attrs`'s output lights `FGI_PRESENT`. Catches the
    /// drift class where a future `MarkingClassification` FGI
    /// variant is added without an `is_set(FGI_PRESENT)` update;
    /// `MASK_FDR_OR_RELIDO_INCOMPAT`'s correctness depends on this.
    #[test]
    fn fgi_classification_axis_sets_fgi_present(attrs in arb_attrs()) {
        if matches!(attrs.classification, Some(MarkingClassification::Fgi(_))) {
            prop_assert!(
                derive_bits(&attrs).is_set(fact_bit::FGI_PRESENT),
                "FGI classification did not light FGI_PRESENT",
            );
        }
    }
}
