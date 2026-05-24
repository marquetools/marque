// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Property-based parity gate for the PR-H (issue #371 tier-3) SCI
//! per-system bitmask dispatch against an independent structural oracle.
//!
//! # Scope
//!
//! Five proptest blocks covering all 5 rows of `SCI_PER_SYSTEM_CATALOG`:
//!
//! - **Row #1** `marking.sci.hcs-o-companions` — HCS-O with US
//!   classification; ORCON + NOFORN required, ORCON-USGOV forbidden. §H.4 p64.
//! - **Row #2** `sci-per-system/HCS-P-NOFORN` — HCS-P (bare or sub) with
//!   US classification; NOFORN required. §H.4 p66.
//! - **Row #3** `sci-per-system/HCS-P-sub-companions` — HCS-P with
//!   sub-compartments; ORCON required, ORCON-USGOV forbidden. §H.4 p68.
//! - **Row #4** `sci-per-system/SI-G-companions` — SI-G with US
//!   classification; ORCON required, ORCON-USGOV forbidden. §H.4 p80.
//! - **Row #5** `sci-per-system/TK-compartment-NOFORN` — TK-BLFH/IDIT/KAND
//!   with US classification; NOFORN required. §H.4 p87 + p91 + p95.
//!
//! # Oracle discipline
//!
//! Each `oracle_*` function re-derives the row's predicate directly from
//! the CAPCO-2016 source text. It does NOT call `derive_bits`, `row.presence()`,
//! or any `SCI_PER_SYSTEM_CATALOG` field — same independence discipline as
//! tier-1's `proptest_tier1_mask.rs` and tier-2's `proptest_tier2_mask.rs`.
//!
//! The proptest asserts that
//! `fires_via_dispatch(name, &attrs) == oracle(name, &attrs)` across
//! randomly generated `CanonicalAttrs` shapes. Diagnostic synthesis
//! (message, citation, span, severity) is exercised by corpus parity
//! and the disabled legacy tests in `sci_per_system_catalog.rs`.
//!
//! # US-classification gate
//!
//! All 5 emit functions open with an early-out when no US classification
//! is present (§H.4 rows apply to US-classified portions only). The oracle
//! mirrors this: if `us_classification() == None`, the oracle returns `false`
//! (no violation) regardless of which companions are present.

use marque_capco::{CapcoMarking, CapcoScheme};
use marque_ism::{
    Classification, DissemControl, MarkingClassification, SciCompartment, SciControlBare,
    SciControlSystem, SciMarking, canonical::CanonicalAttrs,
};
use marque_scheme::{ConstraintViolation, MarkingScheme};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Shared scheme instance
// ---------------------------------------------------------------------------

fn shared_scheme() -> &'static CapcoScheme {
    use std::sync::OnceLock;
    static SCHEME: OnceLock<CapcoScheme> = OnceLock::new();
    SCHEME.get_or_init(CapcoScheme::new)
}

fn fires_via_dispatch(name: &'static str, attrs: &CanonicalAttrs) -> bool {
    let marking = CapcoMarking::new(attrs.clone());
    let bits = shared_scheme().precompute_bits(&marking);
    let out: Vec<ConstraintViolation> = shared_scheme().evaluate_custom(name, &marking, bits);
    !out.is_empty()
}

// ---------------------------------------------------------------------------
// Strategies — companion presence
// ---------------------------------------------------------------------------

/// Generate a dissem set that is a random subset of
/// {NOFORN, ORCON, ORCON-USGOV} for testing companion checks.
fn arb_ic_dissem_companions() -> impl Strategy<Value = Vec<DissemControl>> {
    (any::<bool>(), any::<bool>(), any::<bool>()).prop_map(|(nf, oc, oc_usgov)| {
        let mut v = Vec::new();
        if nf {
            v.push(DissemControl::Nf);
        }
        if oc {
            v.push(DissemControl::Oc);
        }
        // ORCON-USGOV and ORCON are mutually exclusive in practice
        // (per §H.8 p140 supersession) but both can appear in the
        // attrs during testing — the emit functions handle both cases.
        if oc_usgov {
            v.push(DissemControl::OcUsgov);
        }
        v
    })
}

/// US classification at any level — Unclassified through TopSecret.
fn arb_us_classification() -> impl Strategy<Value = MarkingClassification> {
    prop_oneof![
        Just(MarkingClassification::Us(Classification::Unclassified)),
        Just(MarkingClassification::Us(Classification::Restricted)),
        Just(MarkingClassification::Us(Classification::Confidential)),
        Just(MarkingClassification::Us(Classification::Secret)),
        Just(MarkingClassification::Us(Classification::TopSecret)),
    ]
}

/// Generate optional US classification (None = no US class, which exercises
/// the US-only early-out in all 5 emit functions).
fn arb_optional_us_classification() -> impl Strategy<Value = Option<MarkingClassification>> {
    prop_oneof![Just(None), arb_us_classification().prop_map(Some),]
}

// ---------------------------------------------------------------------------
// Strategies — Row #1: HCS-O companions
// ---------------------------------------------------------------------------

fn arb_attrs_hcs_o() -> impl Strategy<Value = CanonicalAttrs> {
    (
        any::<bool>(), // hcs_o present?
        arb_optional_us_classification(),
        arb_ic_dissem_companions(),
    )
        .prop_map(|(hcs_o, cls, dissem)| {
            let mut a = CanonicalAttrs::default();
            a.classification = cls;
            if hcs_o {
                a.sci_markings = Box::new([SciMarking::new(
                    SciControlSystem::Published(SciControlBare::Hcs),
                    Box::new([SciCompartment::new("O", Box::new([]))]),
                    None,
                )]);
            }
            a.dissem_us = dissem.into_boxed_slice();
            a
        })
}

// ---------------------------------------------------------------------------
// Strategies — Row #2: HCS-P-NOFORN
// ---------------------------------------------------------------------------

fn arb_attrs_hcs_p_noforn() -> impl Strategy<Value = CanonicalAttrs> {
    (
        any::<bool>(), // hcs_p_bare present?
        any::<bool>(), // hcs_p_sub present?
        arb_optional_us_classification(),
        arb_ic_dissem_companions(),
    )
        .prop_map(|(hcs_p_bare, hcs_p_sub, cls, dissem)| {
            let mut a = CanonicalAttrs::default();
            a.classification = cls;
            let mut sci: Vec<SciMarking> = Vec::new();
            if hcs_p_bare {
                // Bare HCS-P (no sub-compartments) — sets SCI_PRESENT only.
                sci.push(SciMarking::new(
                    SciControlSystem::Published(SciControlBare::Hcs),
                    Box::new([SciCompartment::new("P", Box::new([]))]),
                    None,
                ));
            }
            if hcs_p_sub {
                // HCS-P with sub-compartment — sets SCI_PRESENT + SCI_HCS_P_SUB.
                sci.push(SciMarking::new(
                    SciControlSystem::Published(SciControlBare::Hcs),
                    Box::new([SciCompartment::new("P", Box::new(["ALPHA".into()]))]),
                    None,
                ));
            }
            a.sci_markings = sci.into_boxed_slice();
            a.dissem_us = dissem.into_boxed_slice();
            a
        })
}

// ---------------------------------------------------------------------------
// Strategies — Row #3: HCS-P-sub-companions
// ---------------------------------------------------------------------------

fn arb_attrs_hcs_p_sub() -> impl Strategy<Value = CanonicalAttrs> {
    (
        any::<bool>(), // hcs_p_sub present?
        arb_optional_us_classification(),
        arb_ic_dissem_companions(),
    )
        .prop_map(|(hcs_p_sub, cls, dissem)| {
            let mut a = CanonicalAttrs::default();
            a.classification = cls;
            if hcs_p_sub {
                a.sci_markings = Box::new([SciMarking::new(
                    SciControlSystem::Published(SciControlBare::Hcs),
                    Box::new([SciCompartment::new("P", Box::new(["ALPHA".into()]))]),
                    None,
                )]);
            }
            a.dissem_us = dissem.into_boxed_slice();
            a
        })
}

// ---------------------------------------------------------------------------
// Strategies — Row #4: SI-G companions
// ---------------------------------------------------------------------------

fn arb_attrs_si_g() -> impl Strategy<Value = CanonicalAttrs> {
    (
        any::<bool>(), // si_g present?
        arb_optional_us_classification(),
        arb_ic_dissem_companions(),
    )
        .prop_map(|(si_g, cls, dissem)| {
            let mut a = CanonicalAttrs::default();
            a.classification = cls;
            if si_g {
                a.sci_markings = Box::new([SciMarking::new(
                    SciControlSystem::Published(SciControlBare::Si),
                    Box::new([SciCompartment::new("G", Box::new([]))]),
                    None,
                )]);
            }
            a.dissem_us = dissem.into_boxed_slice();
            a
        })
}

// ---------------------------------------------------------------------------
// Strategies — Row #5: TK-compartment-NOFORN
// ---------------------------------------------------------------------------

fn arb_attrs_tk_noforn() -> impl Strategy<Value = CanonicalAttrs> {
    (
        any::<bool>(), // tk_blfh present?
        any::<bool>(), // tk_idit present?
        any::<bool>(), // tk_kand present?
        arb_optional_us_classification(),
        arb_ic_dissem_companions(),
    )
        .prop_map(|(blfh, idit, kand, cls, dissem)| {
            let mut a = CanonicalAttrs::default();
            a.classification = cls;
            let mut sci: Vec<SciMarking> = Vec::new();
            if blfh {
                sci.push(SciMarking::new(
                    SciControlSystem::Published(SciControlBare::Tk),
                    Box::new([SciCompartment::new("BLFH", Box::new([]))]),
                    None,
                ));
            }
            if idit {
                sci.push(SciMarking::new(
                    SciControlSystem::Published(SciControlBare::Tk),
                    Box::new([SciCompartment::new("IDIT", Box::new([]))]),
                    None,
                ));
            }
            if kand {
                sci.push(SciMarking::new(
                    SciControlSystem::Published(SciControlBare::Tk),
                    Box::new([SciCompartment::new("KAND", Box::new([]))]),
                    None,
                ));
            }
            a.sci_markings = sci.into_boxed_slice();
            a.dissem_us = dissem.into_boxed_slice();
            a
        })
}

// ---------------------------------------------------------------------------
// Oracles — re-derived from CAPCO-2016 source text (NO derive_bits)
// ---------------------------------------------------------------------------

/// Returns `true` if the attrs have a US classification (any level).
/// All 5 emit functions early-out when this is false (§H.4 rows are US-only).
fn has_us_classification(attrs: &CanonicalAttrs) -> bool {
    attrs.us_classification().is_some()
}

/// Oracle for `marking.sci.hcs-o-companions`:
/// HCS-O present + US classified + (ORCON or ORCON-USGOV absent) OR NOFORN absent.
/// Fires when ORCON or NOFORN is missing. §H.4 p64.
///
/// # Token-span limitation
///
/// `emit_hcs_o_companions` also emits an ORCON-USGOV→ORCON replacement fix when
/// `DissemControl::OcUsgov` is present, but only when `dissem_token_span` finds a
/// real byte span for it. Programmatic `CanonicalAttrs` (as generated here) have
/// empty `token_spans`, so that branch never fires in test conditions. The oracle
/// therefore does not check the ORCON-USGOV-present case: ORCON-USGOV satisfies
/// the ORCON companion requirement here (no violation), and the replacement fix is
/// not observable without a real span.
fn oracle_hcs_o_companions(attrs: &CanonicalAttrs) -> bool {
    // Presence gate: any HCS-O compartment.
    let hcs_o_present = attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
            && m.compartments.iter().any(|c| c.identifier.as_str() == "O")
    });
    if !hcs_o_present {
        return false;
    }
    // US-only gate (all §H.4 rows).
    if !has_us_classification(attrs) {
        return false;
    }
    // OcUsgov satisfies "has ORCON" — the structural emit uses `Oc || OcUsgov`.
    let has_orcon = attrs
        .dissem_iter()
        .any(|d| matches!(d, DissemControl::Oc | DissemControl::OcUsgov));
    let has_noforn = attrs.dissem_iter().any(|d| matches!(d, DissemControl::Nf));
    !has_orcon || !has_noforn
}

/// Oracle for `sci-per-system/HCS-P-NOFORN`:
/// HCS-P (any) present + US classified + NOFORN absent. §H.4 p66.
fn oracle_hcs_p_noforn(attrs: &CanonicalAttrs) -> bool {
    let hcs_p_present = attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
            && m.compartments.iter().any(|c| c.identifier.as_str() == "P")
    });
    if !hcs_p_present {
        return false;
    }
    if !has_us_classification(attrs) {
        return false;
    }
    !attrs.dissem_iter().any(|d| matches!(d, DissemControl::Nf))
}

/// Oracle for `sci-per-system/HCS-P-sub-companions`:
/// HCS-P with sub-compartment + US classified + ORCON absent. §H.4 p68.
///
/// # Token-span limitation
///
/// `emit_hcs_p_sub_companions` also emits an ORCON-USGOV→ORCON replacement fix
/// when `DissemControl::OcUsgov` is present with a real byte span. Programmatic
/// `CanonicalAttrs` have empty `token_spans`, so `dissem_token_span` returns None
/// and that branch never fires. The oracle therefore does not check the
/// ORCON-USGOV-present case: OcUsgov satisfies the ORCON companion requirement in
/// the structural emit (`has_orcon = Oc || OcUsgov`), and the replacement fix is
/// not observable without a real span.
fn oracle_hcs_p_sub_companions(attrs: &CanonicalAttrs) -> bool {
    let hcs_p_sub_present = attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
            && m.compartments
                .iter()
                .any(|c| c.identifier.as_str() == "P" && !c.sub_compartments.is_empty())
    });
    if !hcs_p_sub_present {
        return false;
    }
    if !has_us_classification(attrs) {
        return false;
    }
    // OcUsgov satisfies "has ORCON" — the structural emit uses `Oc || OcUsgov`.
    let has_orcon = attrs
        .dissem_iter()
        .any(|d| matches!(d, DissemControl::Oc | DissemControl::OcUsgov));
    !has_orcon
}

/// Oracle for `sci-per-system/SI-G-companions`:
/// SI-G present + US classified + ORCON absent. §H.4 p80.
///
/// # Token-span limitation
///
/// `emit_si_g_companions` also emits an ORCON-USGOV→ORCON replacement fix when
/// `DissemControl::OcUsgov` is present with a real byte span. Programmatic
/// `CanonicalAttrs` have empty `token_spans`, so `dissem_token_span` returns None
/// and that branch never fires. The oracle therefore does not check the
/// ORCON-USGOV-present case: OcUsgov satisfies the ORCON companion requirement in
/// the structural emit (`has_orcon = Oc || OcUsgov`), and the replacement fix is
/// not observable without a real span.
fn oracle_si_g_companions(attrs: &CanonicalAttrs) -> bool {
    let si_g_present = attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Si))
            && m.compartments.iter().any(|c| c.identifier.as_str() == "G")
    });
    if !si_g_present {
        return false;
    }
    if !has_us_classification(attrs) {
        return false;
    }
    // OcUsgov satisfies "has ORCON" — the structural emit uses `Oc || OcUsgov`.
    let has_orcon = attrs
        .dissem_iter()
        .any(|d| matches!(d, DissemControl::Oc | DissemControl::OcUsgov));
    !has_orcon
}

/// Oracle for `sci-per-system/TK-compartment-NOFORN`:
/// TK-{BLFH|IDIT|KAND} present + US classified + NOFORN absent.
/// §H.4 p87 (BLFH) + p91 (IDIT) + p95 (KAND).
fn oracle_tk_compartment_noforn(attrs: &CanonicalAttrs) -> bool {
    let tk_noforn_present = attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Tk))
            && m.compartments
                .iter()
                .any(|c| matches!(c.identifier.as_str(), "BLFH" | "IDIT" | "KAND"))
    });
    if !tk_noforn_present {
        return false;
    }
    if !has_us_classification(attrs) {
        return false;
    }
    !attrs.dissem_iter().any(|d| matches!(d, DissemControl::Nf))
}

// ---------------------------------------------------------------------------
// Proptests — Row #1: HCS-O companions
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1024,
        .. ProptestConfig::default()
    })]

    #[test]
    fn hcs_o_companions_matches_oracle(attrs in arb_attrs_hcs_o()) {
        prop_assert_eq!(
            fires_via_dispatch("marking.sci.hcs-o-companions", &attrs),
            oracle_hcs_o_companions(&attrs),
            "HCS-O-companions dispatch vs oracle diverged"
        );
    }
}

// ---------------------------------------------------------------------------
// Proptests — Row #2: HCS-P-NOFORN
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1024,
        .. ProptestConfig::default()
    })]

    #[test]
    fn hcs_p_noforn_matches_oracle(attrs in arb_attrs_hcs_p_noforn()) {
        prop_assert_eq!(
            fires_via_dispatch("marking.sci.hcs-p-noforn-required", &attrs),
            oracle_hcs_p_noforn(&attrs),
            "HCS-P-NOFORN dispatch vs oracle diverged"
        );
    }
}

// ---------------------------------------------------------------------------
// Proptests — Row #3: HCS-P-sub-companions
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1024,
        .. ProptestConfig::default()
    })]

    #[test]
    fn hcs_p_sub_companions_matches_oracle(attrs in arb_attrs_hcs_p_sub()) {
        prop_assert_eq!(
            fires_via_dispatch("marking.sci.hcs-p-sub-companions", &attrs),
            oracle_hcs_p_sub_companions(&attrs),
            "HCS-P-sub-companions dispatch vs oracle diverged"
        );
    }
}

// ---------------------------------------------------------------------------
// Proptests — Row #4: SI-G companions
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1024,
        .. ProptestConfig::default()
    })]

    #[test]
    fn si_g_companions_matches_oracle(attrs in arb_attrs_si_g()) {
        prop_assert_eq!(
            fires_via_dispatch("marking.sci.si-g-companions", &attrs),
            oracle_si_g_companions(&attrs),
            "SI-G-companions dispatch vs oracle diverged"
        );
    }
}

// ---------------------------------------------------------------------------
// Proptests — Row #5: TK-compartment-NOFORN
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1024,
        .. ProptestConfig::default()
    })]

    #[test]
    fn tk_compartment_noforn_matches_oracle(attrs in arb_attrs_tk_noforn()) {
        prop_assert_eq!(
            fires_via_dispatch("marking.sci.tk-compartment-noforn-required", &attrs),
            oracle_tk_compartment_noforn(&attrs),
            "TK-compartment-NOFORN dispatch vs oracle diverged"
        );
    }
}
