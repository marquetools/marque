// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Round-trip proptests for `marque_capco::fact_bitmask`'s `derive_bits`
//! and `apply_closed_bits_to` projection helpers.
//!
//! This file asserts the projection's algebraic properties. The
//! closure-table laws (idempotence, extensivity, monotonicity,
//! convergence-bound) live in `proptest_closure_table.rs`; the
//! cross-path parity gate against `scheme.closure(marking)` lives
//! alongside it.
//!
//! The properties covered here:
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

/// Optionally generate a single SCI marking exercising the six
/// compartment sentinels (SI-G, HCS-O, HCS-P[sub], TK-BLFH, TK-IDIT,
/// TK-KAND) plus a "no SCI" arm. Loaded into `CanonicalAttrs::sci_markings`
/// so `apply_preserves_non_cone_axes` exercises the preservation
/// property non-vacuously (a default-empty strategy would make the
/// sci_markings assertion trivially true).
fn arb_sci_markings() -> impl Strategy<Value = Vec<marque_ism::SciMarking>> {
    use marque_ism::{SciCompartment, SciControlBare, SciControlSystem, SciMarking};

    prop_oneof![
        Just(vec![]),
        Just(vec![SciMarking::new(
            SciControlSystem::Published(SciControlBare::Si),
            Box::new([SciCompartment::new("G", Box::new([]))]),
            None,
        )]),
        Just(vec![SciMarking::new(
            SciControlSystem::Published(SciControlBare::Hcs),
            Box::new([SciCompartment::new("O", Box::new([]))]),
            None,
        )]),
        Just(vec![SciMarking::new(
            SciControlSystem::Published(SciControlBare::Tk),
            Box::new([SciCompartment::new("BLFH", Box::new([]))]),
            None,
        )]),
    ]
}

/// Optionally generate a SAR marking — exercises `SAR_PRESENT` and
/// keeps `apply_preserves_non_cone_axes`'s `sar_markings` assertion
/// non-vacuous.
fn arb_sar_markings() -> impl Strategy<Value = Option<marque_ism::SarMarking>> {
    use marque_ism::{SarIndicator, SarMarking, SarProgram};

    prop_oneof![
        Just(None),
        Just(Some(SarMarking::new(
            SarIndicator::Abbrev,
            Box::new([SarProgram::new("BP", Box::new([]))]),
        ))),
    ]
}

/// A focused strategy generating realistic `CanonicalAttrs` shapes
/// from the closed-vocab axes the bitmask reads. The SCI and SAR axes
/// are populated some of the time so
/// `apply_preserves_non_cone_axes`' assertions on
/// `sci_markings` / `sar_markings` are non-vacuous — the strategies
/// produce non-empty values in roughly 3/4 of cases for SCI and
/// 1/2 for SAR.
fn arb_attrs() -> impl Strategy<Value = CanonicalAttrs> {
    (
        arb_classification(),
        arb_dissem_us(),
        arb_non_ic_dissem(),
        arb_aea(),
        arb_rel_to(),
        arb_sci_markings(),
        arb_sar_markings(),
    )
        .prop_map(|(cls, dus, nid, aea, rel, sci, sar)| {
            let mut a = CanonicalAttrs::default();
            a.classification = cls;
            a.dissem_us = dus.into();
            a.non_ic_dissem = nid.into();
            a.aea_markings = aea.into();
            a.rel_to = rel.into();
            a.sci_markings = sci.into();
            a.sar_markings = sar;
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
    /// fragment: every eligible bit in `closed` is set in
    /// `derive_bits(apply(...))`, AND no eligible bit appears that
    /// was not in `closed` either to start with (in `input`) or as
    /// part of the `cone_bits & APPLY_ELIGIBLE_MASK` injection.
    ///
    /// §H.8 p145 caveat: the NOFORN supersession overlay can EVICT
    /// `RELIDO` / `Rel` / `Displayonly` / `Eyes` from `dissem_us`
    /// (the dominated-control strip) AND clear `rel_to` (which
    /// erases `REL_TO_USA` if `USA` was already in the input list).
    /// When NOFORN is in the delta, those bits drop from the
    /// post-apply bitmask. The assertion accounts for that —
    /// `recovered_eligible` may be a strict subset of
    /// `expected_eligible` exactly when NOFORN is in `closed`.
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

        let recovered_eligible = after.bits() & APPLY_ELIGIBLE_MASK;
        let expected_eligible = closed.bits() & APPLY_ELIGIBLE_MASK;

        // Forward direction: every eligible bit in `closed` must be
        // set in `after`, EXCEPT the §H.8 p145 dominated atoms when
        // NOFORN is in the delta. The dominated tokens (RELIDO,
        // REL_TO_USA implicitly via rel_to clear) drop out by design.
        let noforn_in_closed = (closed.bits() & (1u128 << fact_bit::NOFORN)) != 0;
        let evicted_by_noforn = if noforn_in_closed {
            (1u128 << fact_bit::RELIDO) | (1u128 << fact_bit::REL_TO_USA)
        } else {
            0
        };
        let must_be_recovered = expected_eligible & !evicted_by_noforn;
        prop_assert_eq!(
            recovered_eligible & must_be_recovered,
            must_be_recovered,
            "lost a non-dominated eligible bit on round-trip",
        );

        // Reverse direction: no eligible bit appears in `after` that
        // wasn't either in `closed` to begin with. `apply_closed_bits_to`
        // only adds atoms from `APPLY_ELIGIBLE_MASK`, never creates
        // ineligible bits, and strips dominated controls (never adds
        // dominated controls). So the post-apply eligible bits MUST
        // be a subset of `expected_eligible`.
        prop_assert_eq!(
            recovered_eligible & !expected_eligible,
            0,
            "post-apply has an eligible bit that wasn't in closed",
        );
    }

    /// §H.8 p145 NOFORN-dominates invariant: when NOFORN is in the
    /// closed bitmask AND the apply path adds it, the resulting
    /// `attrs` must satisfy the dominance rule — no `Rel`, `Relido`,
    /// `Displayonly`, `Eyes` in `dissem_us`; empty `rel_to`; empty
    /// `display_only_to`. Catches the hole where unconditional
    /// per-marking NOFORN rows (HCS-O / HCS-P[sub] / TK-BLFH/IDIT/KAND)
    /// with no suppressors fire on portions with pre-existing FD&R
    /// tokens.
    ///
    /// Authority: §H.8 p145 + §D.2 Table 3 rows 1-2.
    #[test]
    fn apply_preserves_h8_p145_invariant(attrs in arb_attrs()) {
        let input = derive_bits(&attrs);
        // Force NOFORN into the delta (skip if already present).
        if input.is_set(fact_bit::NOFORN) {
            return Ok(());
        }
        let closed = input.with_bit(fact_bit::NOFORN);
        let mut a = attrs.clone();
        apply_closed_bits_to(&mut a, closed, input);

        // §H.8 p145 must hold post-apply.
        prop_assert!(
            a.dissem_us.contains(&DissemControl::Nf),
            "NOFORN delta did not add Nf to dissem_us",
        );
        for dominated in [
            DissemControl::Rel,
            DissemControl::Relido,
            DissemControl::Displayonly,
            DissemControl::Eyes,
        ] {
            prop_assert!(
                !a.dissem_us.contains(&dominated),
                "§H.8 p145: NOFORN did not evict {} from dissem_us",
                dominated.as_str(),
            );
        }
        prop_assert!(a.rel_to.is_empty(), "§H.8 p145: rel_to not cleared");
        prop_assert!(
            a.display_only_to.is_empty(),
            "§H.8 p145: display_only_to not cleared",
        );
    }

    /// FGI projection invariant: any `MarkingClassification::Fgi(_)`
    /// in `arb_attrs`'s output lights `FGI_PRESENT`. Catches the
    /// drift class where a future `MarkingClassification` FGI
    /// variant is added without an `is_set(FGI_PRESENT)` update.
    /// Pre-#704 `MASK_FDR_OR_RELIDO_INCOMPAT`'s correctness depended
    /// on this; post-#704 the masks retired but the FGI_PRESENT bit
    /// is still load-bearing for `derive_bits` correctness and any
    /// future overlay that observes FGI presence.
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
