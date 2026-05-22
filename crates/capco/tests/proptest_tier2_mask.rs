// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Property-based parity gate for the PR-G (#650 tier-2) class-floor
//! bitmask dispatch against an independent structural oracle.
//!
//! # Scope
//!
//! Four proptest blocks covering the 23 bitmask-compiled rows of the
//! 27-row `CLASS_FLOOR_CATALOG` (the 4 passthrough rows require open-vocab
//! markings not representable in a closed proptest strategy; they are
//! exercised by the catalog pin in `tier2_catalog_pin.rs`):
//!
//! - **§2.1 Floor TS** (5 rows): HCS-comp-sub, SI-comp, TK-BLFH, BALK, BOHEMIA
//! - **§2.2 Floor S** (8 rows): HCS-comp, RSV-comp, TK, RD-SG, FRD-SG,
//!   CNWDI, RSEN, IMCON
//! - **§2.3 Floor C** (8 rows): SI-bare, SAR, RD, FRD, TFNI, ATOMAL,
//!   ORCON, EYES-ONLY
//! - **§2.4 Floor =U** (2 rows): DOD-UCNI, DOE-UCNI
//!
//! # Oracle discipline
//!
//! Each `oracle_*` function re-derives the row's predicate directly from
//! the CAPCO-2016 source text. It does NOT call `derive_bits`, `row.presence()`,
//! or `class_floor_satisfied` — same independence discipline as tier-1's
//! `proptest_tier1_mask.rs`. The proptest asserts that
//! `fires_via_dispatch(name, &attrs) == oracle(name, &attrs)` across
//! randomly generated `CanonicalAttrs` shapes.
//!
//! Diagnostic synthesis (message, citation, span, severity) is exercised
//! by the corpus parity gate and per-row unit tests in
//! `tier2_mask::tests`. This file covers the algebraic-predicate check.

use marque_capco::{CapcoMarking, CapcoScheme};
use marque_ism::{
    AeaMarking, AtomalBlock, Classification, DissemControl, FrdBlock, MarkingClassification,
    NatoClassification, NatoSap, RdBlock, SarIndicator, SarMarking, SarProgram, SciCompartment,
    SciControlBare, SciControlSystem, SciMarking, canonical::CanonicalAttrs,
};
use marque_scheme::{ConstraintViolation, MarkingScheme};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Shared scheme instance (amortize catalog build across 5×1024 cases)
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
// Strategies — §2.1 Floor TS (HCS-sub, SI-comp, TK-BLFH, BALK, BOHEMIA)
// ---------------------------------------------------------------------------

/// Generate attrs for the Floor TS group: any combination of the five
/// TS-floor-triggering markers, classified at any US level.
fn arb_attrs_floor_ts() -> impl Strategy<Value = CanonicalAttrs> {
    (
        any::<bool>(), // hcs_p_sub present?
        any::<bool>(), // si_g present?
        any::<bool>(), // tk_blfh present?
        any::<bool>(), // balk present?
        any::<bool>(), // bohemia present?
        arb_us_classification(),
    )
        .prop_map(|(hcs, si, tk, balk, bohemia, cls)| {
            let mut a = CanonicalAttrs::default();
            a.classification = Some(cls);
            let mut sci: Vec<SciMarking> = Vec::new();
            if hcs {
                // HCS-P with sub-compartment — exactly what SCI_HCS_P_SUB (bit 42) gates on.
                sci.push(SciMarking::new(
                    SciControlSystem::Published(SciControlBare::Hcs),
                    Box::new([SciCompartment::new("P", Box::new(["ALPHA".into()]))]),
                    None,
                ));
            }
            if si {
                // SI-G compartment — SCI_SI_G (bit 40).
                sci.push(SciMarking::new(
                    SciControlSystem::Published(SciControlBare::Si),
                    Box::new([SciCompartment::new("G", Box::new([]))]),
                    None,
                ));
            }
            if tk {
                // TK-BLFH — SCI_TK_BLFH (bit 43).
                sci.push(SciMarking::new(
                    SciControlSystem::Published(SciControlBare::Tk),
                    Box::new([SciCompartment::new("BLFH", Box::new([]))]),
                    None,
                ));
            }
            if balk {
                // BALK NATO SAP — AEA_BALK (bit 50).
                sci.push(SciMarking::new(
                    SciControlSystem::NatoSap(NatoSap::Balk),
                    Box::new([]),
                    None,
                ));
            }
            if bohemia {
                // BOHEMIA NATO SAP — AEA_BOHEMIA (bit 49).
                sci.push(SciMarking::new(
                    SciControlSystem::NatoSap(NatoSap::Bohemia),
                    Box::new([]),
                    None,
                ));
            }
            a.sci_markings = sci.into_boxed_slice();
            a
        })
}

// ---------------------------------------------------------------------------
// Strategies — §2.2 Floor S
// ---------------------------------------------------------------------------

fn arb_attrs_floor_s() -> impl Strategy<Value = CanonicalAttrs> {
    (
        any::<bool>(), // hcs_o present?
        any::<bool>(), // hcs_p_bare present? (no sub-compartments — exercises the SCI_PRESENT coarse gate)
        any::<bool>(), // rsv_comp present?
        any::<bool>(), // tk_idit present?
        any::<bool>(), // rd_sigma present?
        any::<bool>(), // frd_sigma present?
        any::<bool>(), // rd_cnwdi present?
        any::<bool>(), // rsen present?
        any::<bool>(), // imcon present?
        arb_us_classification(),
    )
        .prop_map(
            |(hcs_o, hcs_p_bare, rsv, tk_idit, rd_sg, frd_sg, cnwdi, rsen, imcon, cls)| {
                let mut a = CanonicalAttrs::default();
                a.classification = Some(cls);
                let mut sci: Vec<SciMarking> = Vec::new();
                if hcs_o {
                    // HCS-O compartment (bare, no sub-compartments, not X) — SCI_HCS_O (bit 41).
                    sci.push(SciMarking::new(
                        SciControlSystem::Published(SciControlBare::Hcs),
                        Box::new([SciCompartment::new("O", Box::new([]))]),
                        None,
                    ));
                }
                if hcs_p_bare {
                    // HCS-P bare compartment (no sub-compartments) — sets SCI_PRESENT (bit 37)
                    // only; SCI_HCS_P_SUB (bit 42) is NOT set. This is the case the old trigger
                    // `SCI_HCS_O | SCI_HCS_P_SUB` missed: presence_hcs_comp_only fires on bare
                    // HCS-P, but neither sentinel bit is set, so the coarse gate must use
                    // SCI_PRESENT instead. §H.4 p66 example: `(S//HCS-P//NF)`.
                    sci.push(SciMarking::new(
                        SciControlSystem::Published(SciControlBare::Hcs),
                        Box::new([SciCompartment::new("P", Box::new([]))]),
                        None,
                    ));
                }
                if rsv {
                    // RSV with a compartment — SCI_PRESENT (bit 37) coarse gate.
                    sci.push(SciMarking::new(
                        SciControlSystem::Published(SciControlBare::Rsv),
                        Box::new([SciCompartment::new("COMP1", Box::new([]))]),
                        None,
                    ));
                }
                if tk_idit {
                    // TK-IDIT — SCI_TK_IDIT (bit 44).
                    sci.push(SciMarking::new(
                        SciControlSystem::Published(SciControlBare::Tk),
                        Box::new([SciCompartment::new("IDIT", Box::new([]))]),
                        None,
                    ));
                }
                a.sci_markings = sci.into_boxed_slice();
                let mut aea: Vec<AeaMarking> = Vec::new();
                if rd_sg {
                    // RD with sigma — AEA_RD (bit 22) + sigma slice non-empty.
                    aea.push(AeaMarking::Rd(RdBlock {
                        sigma: vec![1u8].into_boxed_slice(),
                        cnwdi: false,
                    }));
                }
                if frd_sg {
                    // FRD with sigma — AEA_FRD (bit 23) + sigma slice non-empty.
                    aea.push(AeaMarking::Frd(FrdBlock {
                        sigma: vec![1u8].into_boxed_slice(),
                    }));
                }
                if cnwdi {
                    // RD-CNWDI (distinct from RD-sigma) — AEA_RD (bit 22) + cnwdi=true.
                    aea.push(AeaMarking::Rd(RdBlock {
                        sigma: Box::new([]),
                        cnwdi: true,
                    }));
                }
                a.aea_markings = aea.into_boxed_slice();
                let mut dissem: Vec<DissemControl> = Vec::new();
                if rsen {
                    dissem.push(DissemControl::Rs);
                }
                if imcon {
                    dissem.push(DissemControl::Imc);
                }
                a.dissem_us = dissem.into_boxed_slice();
                a
            },
        )
}

// ---------------------------------------------------------------------------
// Strategies — §2.3 Floor C
// ---------------------------------------------------------------------------

fn arb_attrs_floor_c() -> impl Strategy<Value = CanonicalAttrs> {
    (
        any::<bool>(), // si_bare present?
        any::<bool>(), // sar present?
        any::<bool>(), // rd_bare present?
        any::<bool>(), // frd_bare present?
        any::<bool>(), // tfni present?
        any::<bool>(), // atomal present?
        any::<bool>(), // orcon present?
        any::<bool>(), // eyes_only present?
        arb_us_classification(),
    )
        .prop_map(
            |(si_bare, sar, rd_bare, frd_bare, tfni, atomal, orcon, eyes, cls)| {
                let mut a = CanonicalAttrs::default();
                a.classification = Some(cls);
                let mut sci: Vec<SciMarking> = Vec::new();
                if si_bare {
                    // SI bare (no compartments) — SCI_PRESENT (bit 37) coarse gate.
                    sci.push(SciMarking::new(
                        SciControlSystem::Published(SciControlBare::Si),
                        Box::new([]),
                        None,
                    ));
                }
                a.sci_markings = sci.into_boxed_slice();
                if sar {
                    // SAR with one program — SAR_PRESENT (bit 36).
                    a.sar_markings = Some(SarMarking::new(
                        SarIndicator::Abbrev,
                        Box::new([SarProgram::new("BP", Box::new([]))]),
                    ));
                }
                let mut aea: Vec<AeaMarking> = Vec::new();
                if rd_bare {
                    // Bare RD (no CNWDI, no sigma) — AEA_RD (bit 22).
                    aea.push(AeaMarking::Rd(RdBlock {
                        sigma: Box::new([]),
                        cnwdi: false,
                    }));
                }
                if frd_bare {
                    // Bare FRD (no sigma) — AEA_FRD (bit 23).
                    aea.push(AeaMarking::Frd(FrdBlock {
                        sigma: Box::new([]),
                    }));
                }
                if tfni {
                    aea.push(AeaMarking::Tfni);
                }
                if atomal {
                    aea.push(AeaMarking::Atomal(AtomalBlock {}));
                }
                a.aea_markings = aea.into_boxed_slice();
                let mut dissem: Vec<DissemControl> = Vec::new();
                if orcon {
                    dissem.push(DissemControl::Oc);
                }
                if eyes {
                    dissem.push(DissemControl::Eyes);
                }
                a.dissem_us = dissem.into_boxed_slice();
                a
            },
        )
}

// ---------------------------------------------------------------------------
// Strategies — §2.4 Floor =U (UCNI ceiling)
// ---------------------------------------------------------------------------

fn arb_attrs_floor_eq_u() -> impl Strategy<Value = CanonicalAttrs> {
    (
        any::<bool>(), // dod_ucni present?
        any::<bool>(), // doe_ucni present?
        arb_us_classification_for_ucni(),
    )
        .prop_map(|(dod_ucni, doe_ucni, cls)| {
            let mut a = CanonicalAttrs::default();
            a.classification = Some(cls);
            let mut aea: Vec<AeaMarking> = Vec::new();
            if dod_ucni {
                aea.push(AeaMarking::DodUcni);
            }
            if doe_ucni {
                aea.push(AeaMarking::DoeUcni);
            }
            a.aea_markings = aea.into_boxed_slice();
            a
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

/// US classification biased toward U and classified for UCNI ceiling tests.
fn arb_us_classification_for_ucni() -> impl Strategy<Value = MarkingClassification> {
    prop_oneof![
        // U: the only allowed state for UCNI ceiling rows
        Just(MarkingClassification::Us(Classification::Unclassified)),
        // Any classified level: ceiling fails
        Just(MarkingClassification::Us(Classification::Confidential)),
        Just(MarkingClassification::Us(Classification::Secret)),
        Just(MarkingClassification::Us(Classification::TopSecret)),
        // NATO classification: US chain is zero → ceiling fails
        Just(MarkingClassification::Nato(NatoClassification::NatoSecret)),
    ]
}

// ---------------------------------------------------------------------------
// Oracles — re-derived from CAPCO-2016 source text (NO `derive_bits`)
// ---------------------------------------------------------------------------

// ---- §2.1 Floor TS oracles ------------------------------------------------

/// Oracle for `banner.classification.floor-hcs-comp-sub`: HCS with a sub-compartment at < TS.
/// §H.4 p68 (HCS-P sub-compartment guidance).
fn oracle_hcs_comp_sub(attrs: &CanonicalAttrs) -> bool {
    let present = attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
            && m.compartments
                .iter()
                .any(|c| !c.sub_compartments.is_empty())
    });
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::TopSecret)
}

/// Oracle for `class-floor/SI-comp`: SI with a compartment at < TS.
/// §H.4 p76 (SI-comp TS floor).
fn oracle_si_comp(attrs: &CanonicalAttrs) -> bool {
    let present = attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Si))
            && !m.compartments.is_empty()
    });
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::TopSecret)
}

/// Oracle for `class-floor/TK-BLFH`: TK-BLFH compartment at < TS.
/// §H.4 p87/p89 (TK-BLFH TS-only).
fn oracle_tk_blfh(attrs: &CanonicalAttrs) -> bool {
    let present = attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Tk))
            && m.compartments
                .iter()
                .any(|c| c.identifier.as_str() == "BLFH")
    });
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::TopSecret)
}

/// Oracle for `class-floor/BALK`: BALK NATO SAP at < TS. §G.2 p40.
fn oracle_balk(attrs: &CanonicalAttrs) -> bool {
    let present = attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::NatoSap(NatoSap::Balk)));
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::TopSecret)
}

/// Oracle for `class-floor/BOHEMIA`: BOHEMIA NATO SAP at < TS. §G.2 p40.
fn oracle_bohemia(attrs: &CanonicalAttrs) -> bool {
    let present = attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::NatoSap(NatoSap::Bohemia)));
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::TopSecret)
}

// ---- §2.2 Floor S oracles -------------------------------------------------

/// Oracle for `class-floor/HCS-comp`: HCS-O or HCS-P (bare comp, no sub) at < S.
/// §H.4 p64/p66/p68.
fn oracle_hcs_comp(attrs: &CanonicalAttrs) -> bool {
    let present = attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
            && !m.compartments.is_empty()
            && m.compartments.iter().all(|c| c.sub_compartments.is_empty())
            && !m.compartments.iter().any(|c| c.identifier.as_str() == "X")
    });
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::Secret)
}

/// Oracle for `class-floor/RSV-comp`: RSV with compartment at < S. §H.4 p72.
fn oracle_rsv_comp(attrs: &CanonicalAttrs) -> bool {
    let present = attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Rsv))
            && !m.compartments.is_empty()
    });
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::Secret)
}

/// Oracle for `class-floor/TK`: TK (non-BLFH) at < S. §H.4 p85/p91/p95.
fn oracle_tk(attrs: &CanonicalAttrs) -> bool {
    let present = attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Tk))
            && !m
                .compartments
                .iter()
                .any(|c| c.identifier.as_str() == "BLFH")
    });
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::Secret)
}

/// Oracle for `class-floor/RD-SG`: RD-SIGMA at < S. §H.6 p113.
fn oracle_rd_sg(attrs: &CanonicalAttrs) -> bool {
    let present = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, AeaMarking::Rd(rd) if !rd.sigma.is_empty()));
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::Secret)
}

/// Oracle for `class-floor/FRD-SG`: FRD-SIGMA at < S. §H.6 p113.
fn oracle_frd_sg(attrs: &CanonicalAttrs) -> bool {
    let present = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, AeaMarking::Frd(frd) if !frd.sigma.is_empty()));
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::Secret)
}

/// Oracle for `banner.aea.floor-cnwdi`: RD-CNWDI at < S. §H.6 p104.
fn oracle_cnwdi(attrs: &CanonicalAttrs) -> bool {
    let present = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, AeaMarking::Rd(rd) if rd.cnwdi));
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::Secret)
}

/// Oracle for `class-floor/RSEN`: RSEN at < S. §H.8 p149.
fn oracle_rsen(attrs: &CanonicalAttrs) -> bool {
    let present = attrs.dissem_iter().any(|d| matches!(d, DissemControl::Rs));
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::Secret)
}

/// Oracle for `class-floor/IMCON`: IMCON at < S. §H.8 p144.
fn oracle_imcon(attrs: &CanonicalAttrs) -> bool {
    let present = attrs.dissem_iter().any(|d| matches!(d, DissemControl::Imc));
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::Secret)
}

// ---- §2.3 Floor C oracles -------------------------------------------------

/// Oracle for `class-floor/SI`: bare SI at < C. §H.4 p74.
fn oracle_si(attrs: &CanonicalAttrs) -> bool {
    let present = attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Si))
            && m.compartments.is_empty()
    });
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::Confidential)
}

/// Oracle for `E058/SAR-classification-floor`: SAR present at < C. §H.5 p99.
fn oracle_sar(attrs: &CanonicalAttrs) -> bool {
    let present = attrs.sar_markings.is_some();
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::Confidential)
}

/// Oracle for `class-floor/RD`: bare RD (no CNWDI, no sigma) at < C. §H.6 p104.
fn oracle_rd(attrs: &CanonicalAttrs) -> bool {
    let present = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, AeaMarking::Rd(rd) if !rd.cnwdi && rd.sigma.is_empty()));
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::Confidential)
}

/// Oracle for `class-floor/FRD`: bare FRD (no sigma) at < C. §H.6 p104.
fn oracle_frd(attrs: &CanonicalAttrs) -> bool {
    let present = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, AeaMarking::Frd(frd) if frd.sigma.is_empty()));
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::Confidential)
}

/// Oracle for `class-floor/TFNI`: TFNI at < C. §H.6 p107.
fn oracle_tfni(attrs: &CanonicalAttrs) -> bool {
    let present = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, AeaMarking::Tfni));
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::Confidential)
}

/// Oracle for `class-floor/ATOMAL`: ATOMAL at < C. §H.7 p122.
fn oracle_atomal(attrs: &CanonicalAttrs) -> bool {
    let present = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, AeaMarking::Atomal(_)));
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::Confidential)
}

/// Oracle for `class-floor/ORCON`: ORCON or ORCON-USGOV at < C.
/// §H.8 p136 (ORCON) + p140 (ORCON-USGOV).
fn oracle_orcon(attrs: &CanonicalAttrs) -> bool {
    let present = attrs
        .dissem_iter()
        .any(|d| matches!(d, DissemControl::Oc | DissemControl::OcUsgov));
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::Confidential)
}

/// Oracle for `class-floor/EYES-ONLY`: EYES ONLY at < C. §H.8 p152.
fn oracle_eyes_only(attrs: &CanonicalAttrs) -> bool {
    let present = attrs
        .dissem_iter()
        .any(|d| matches!(d, DissemControl::Eyes));
    if !present {
        return false;
    }
    !class_is_at_least(attrs, Classification::Confidential)
}

// ---- §2.4 Floor =U oracles ------------------------------------------------

/// Oracle for `E058/DOD-UCNI-classification-ceiling`: DOD UCNI present and
/// US classification != UNCLASSIFIED. §H.6 p116.
///
/// The ceiling fires when UCNI is present AND the US classification is not
/// exactly UNCLASSIFIED. A non-US (NATO) classification has no US-chain
/// field → `us_classification()` returns `None` → ceiling fires.
fn oracle_dod_ucni(attrs: &CanonicalAttrs) -> bool {
    let present = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, AeaMarking::DodUcni));
    if !present {
        return false;
    }
    // Ceiling fails when US classification is not exactly UNCLASSIFIED.
    !matches!(
        attrs.us_classification(),
        Some(Classification::Unclassified)
    )
}

/// Oracle for `E058/DOE-UCNI-classification-ceiling`: DOE UCNI at non-U US
/// classification. §H.6 p118. Mirror of DOD UCNI.
fn oracle_doe_ucni(attrs: &CanonicalAttrs) -> bool {
    let present = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, AeaMarking::DoeUcni));
    if !present {
        return false;
    }
    !matches!(
        attrs.us_classification(),
        Some(Classification::Unclassified)
    )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the effective classification level is ≥ `floor`,
/// following `MarkingClassification::effective_level()` semantics
/// (reciprocal-raise for NATO; FGI/JOINT as-is).
fn class_is_at_least(attrs: &CanonicalAttrs, floor: Classification) -> bool {
    match attrs.classification.as_ref() {
        Some(c) => c.effective_level() >= floor,
        None => false, // no classification → floor not satisfied
    }
}

// ---------------------------------------------------------------------------
// Proptests — §2.1 Floor TS
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1024,
        .. ProptestConfig::default()
    })]

    #[test]
    fn floor_ts_hcs_comp_sub_matches_oracle(attrs in arb_attrs_floor_ts()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.classification.floor-hcs-comp-sub", &attrs),
            oracle_hcs_comp_sub(&attrs),
            "HCS-comp-sub dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_ts_si_comp_matches_oracle(attrs in arb_attrs_floor_ts()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.classification.floor-si-comp", &attrs),
            oracle_si_comp(&attrs),
            "SI-comp dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_ts_tk_blfh_matches_oracle(attrs in arb_attrs_floor_ts()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.classification.floor-tk-blfh", &attrs),
            oracle_tk_blfh(&attrs),
            "TK-BLFH dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_ts_balk_matches_oracle(attrs in arb_attrs_floor_ts()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.classification.floor-balk", &attrs),
            oracle_balk(&attrs),
            "BALK dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_ts_bohemia_matches_oracle(attrs in arb_attrs_floor_ts()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.classification.floor-bohemia", &attrs),
            oracle_bohemia(&attrs),
            "BOHEMIA dispatch vs oracle diverged"
        );
    }
}

// ---------------------------------------------------------------------------
// Proptests — §2.2 Floor S
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1024,
        .. ProptestConfig::default()
    })]

    #[test]
    fn floor_s_hcs_comp_matches_oracle(attrs in arb_attrs_floor_s()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.classification.floor-hcs-comp", &attrs),
            oracle_hcs_comp(&attrs),
            "HCS-comp dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_s_rsv_comp_matches_oracle(attrs in arb_attrs_floor_s()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.classification.floor-rsv-comp", &attrs),
            oracle_rsv_comp(&attrs),
            "RSV-comp dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_s_tk_matches_oracle(attrs in arb_attrs_floor_s()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.classification.floor-tk", &attrs),
            oracle_tk(&attrs),
            "TK dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_s_rd_sg_matches_oracle(attrs in arb_attrs_floor_s()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.aea.floor-rd-sg", &attrs),
            oracle_rd_sg(&attrs),
            "RD-SG dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_s_frd_sg_matches_oracle(attrs in arb_attrs_floor_s()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.aea.floor-frd-sg", &attrs),
            oracle_frd_sg(&attrs),
            "FRD-SG dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_s_cnwdi_matches_oracle(attrs in arb_attrs_floor_s()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.aea.floor-cnwdi", &attrs),
            oracle_cnwdi(&attrs),
            "CNWDI dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_s_rsen_matches_oracle(attrs in arb_attrs_floor_s()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.dissem.floor-rsen", &attrs),
            oracle_rsen(&attrs),
            "RSEN dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_s_imcon_matches_oracle(attrs in arb_attrs_floor_s()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.dissem.floor-imcon", &attrs),
            oracle_imcon(&attrs),
            "IMCON dispatch vs oracle diverged"
        );
    }
}

// ---------------------------------------------------------------------------
// Proptests — §2.3 Floor C
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1024,
        .. ProptestConfig::default()
    })]

    #[test]
    fn floor_c_si_matches_oracle(attrs in arb_attrs_floor_c()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.classification.floor-si", &attrs),
            oracle_si(&attrs),
            "SI-bare dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_c_sar_matches_oracle(attrs in arb_attrs_floor_c()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.classification.floor-sar", &attrs),
            oracle_sar(&attrs),
            "SAR dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_c_rd_matches_oracle(attrs in arb_attrs_floor_c()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.aea.floor-rd", &attrs),
            oracle_rd(&attrs),
            "RD-bare dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_c_frd_matches_oracle(attrs in arb_attrs_floor_c()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.aea.floor-frd", &attrs),
            oracle_frd(&attrs),
            "FRD-bare dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_c_tfni_matches_oracle(attrs in arb_attrs_floor_c()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.aea.floor-tfni", &attrs),
            oracle_tfni(&attrs),
            "TFNI dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_c_atomal_matches_oracle(attrs in arb_attrs_floor_c()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.aea.floor-atomal", &attrs),
            oracle_atomal(&attrs),
            "ATOMAL dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_c_orcon_matches_oracle(attrs in arb_attrs_floor_c()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.dissem.floor-orcon", &attrs),
            oracle_orcon(&attrs),
            "ORCON dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_c_eyes_only_matches_oracle(attrs in arb_attrs_floor_c()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.dissem.floor-eyes-only", &attrs),
            oracle_eyes_only(&attrs),
            "EYES-ONLY dispatch vs oracle diverged"
        );
    }
}

// ---------------------------------------------------------------------------
// Proptests — §2.4 Floor =U (UCNI ceiling)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1024,
        .. ProptestConfig::default()
    })]

    #[test]
    fn floor_eq_u_dod_ucni_matches_oracle(attrs in arb_attrs_floor_eq_u()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.aea.ceiling-dod-ucni", &attrs),
            oracle_dod_ucni(&attrs),
            "DOD-UCNI dispatch vs oracle diverged"
        );
    }

    #[test]
    fn floor_eq_u_doe_ucni_matches_oracle(attrs in arb_attrs_floor_eq_u()) {
        prop_assert_eq!(
            fires_via_dispatch("banner.aea.ceiling-doe-ucni", &attrs),
            oracle_doe_ucni(&attrs),
            "DOE-UCNI dispatch vs oracle diverged"
        );
    }
}
