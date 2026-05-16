// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Family-presence predicates (one per class-floor catalog row) plus
//! the §3.7 passthrough-family predicates and the SCI helper
//! primitives ([`anchors_on`] / [`has_compartment`] /
//! [`compartment_has_sub`] / [`is_tk_noforn_compartment`]). Lifted
//! from the monolithic `predicates.rs` per the issue #466 Stage 2
//! PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).

// No items from `super::super::*` are needed here — every predicate
// reaches its types via the `marque_ism::` and `marque_scheme::`
// paths directly. The original `predicates.rs` carried the glob
// import for adjacent predicates in the same file; after the Stage-2
// PR A split (`claudedocs/refactor-466/stage2_leaves_plan.md`), the
// only items presence-style predicates need live in `marque_ism`.

// ---------------------------------------------------------------------------
// Family-presence predicates (one per catalog row)
// ---------------------------------------------------------------------------
//
// Each predicate iterates the relevant axis (`attrs.sci_markings`,
// `attrs.aea_markings`, `attrs.dissem_iter()` over the namespace
// split, etc.) looking for any token matching the family pattern.
// Family granularity is the §3.4.6
// author's choice — the predicates pattern-match across all marking-
// template-level leaves that belong to the family.

/// HCS-[comp][sub] — any HCS-anchored marking carrying a compartment
/// that has at least one sub-compartment. Family covers HCS-P [SUB] and
/// any future HCS sub-compartmented variants.
pub(crate) fn presence_hcs_comp_sub(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
            && m.compartments
                .iter()
                .any(|c| !c.sub_compartments.is_empty())
    })
}

/// HCS-[comp] — any HCS-anchored marking carrying a compartment but no
/// sub-compartment (HCS-O, HCS-P bare). Family does NOT include HCS-X
/// (passthrough — see `presence_passthrough_hcs_x`) or bare HCS (legacy,
/// covered by E006/E008).
pub(crate) fn presence_hcs_comp_only(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
            && !m.compartments.is_empty()
            && m.compartments.iter().all(|c| c.sub_compartments.is_empty())
            // Exclude HCS-X: it's a passthrough family with its own row.
            && !m.compartments.iter().any(|c| c.identifier.as_str() == "X")
    })
}

/// SI-[comp] — any SI-anchored marking carrying at least one
/// compartment. Family covers SI-G, SI-G [SUB], SI-ECRU, SI-NONBOOK, and
/// any agency SI compartment per CAPCO-2016 §H.4 p76 (TS-only).
pub(crate) fn presence_si_comp(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Si))
            && !m.compartments.is_empty()
    })
}

/// SI (bare) — any SI-anchored marking with NO compartment. Family is
/// the bare SI control system per §H.4 p74 (C-or-above floor).
pub(crate) fn presence_si_bare(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Si))
            && m.compartments.is_empty()
    })
}

/// TK-BLFH — any TK-anchored marking carrying a BLFH compartment (with
/// or without sub-compartments). §H.4 p87 / p89 — TS-only.
pub(crate) fn presence_tk_blfh(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Tk))
            && m.compartments
                .iter()
                .any(|c| c.identifier.as_str() == "BLFH")
    })
}

/// TK family at the S floor — TK bare, TK-IDIT (with/without sub-comp),
/// TK-KAND (with/without sub-comp). Excludes TK-BLFH (covered by
/// `presence_tk_blfh` at TS-only). §H.4 p85 / p91 / p95.
pub(crate) fn presence_tk_family(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        if !matches!(m.system, SciControlSystem::Published(SciControlBare::Tk)) {
            return false;
        }
        // Exclude markings whose compartment set includes BLFH — those
        // are §2.1 row TK-BLFH (TS-only), not §2.2 row TK (S floor).
        let has_blfh = m
            .compartments
            .iter()
            .any(|c| c.identifier.as_str() == "BLFH");
        !has_blfh
    })
}

/// RSV-[comp] — any RSV-anchored marking carrying a compartment.
/// CAPCO §H.4 p72.
pub(crate) fn presence_rsv_comp(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Rsv))
            && !m.compartments.is_empty()
    })
}

/// RD bare — RD without CNWDI and without SIGMA. CAPCO §H.6 p104 floor C.
pub(crate) fn presence_rd_bare(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs.aea_markings.iter().any(|a| {
        matches!(
            a,
            marque_ism::AeaMarking::Rd(rd) if !rd.cnwdi && rd.sigma.is_empty()
        )
    })
}

/// RD-CNWDI — any RD block with `cnwdi == true`. Replaces retired E022.
/// CAPCO §H.6 p104 (TS-or-S RD); matches the catalog row's
/// authoritative §3.4.6 citation
/// (`E058/CNWDI-classification-floor` → `CAPCO-2016 §H.6 p104`).
pub(crate) fn presence_rd_cnwdi(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Rd(rd) if rd.cnwdi))
}

/// RD-SIGMA — any RD block carrying at least one SIGMA number.
/// CAPCO §H.6 p108 / p113 (RD-SIGMA TS-or-S).
pub(crate) fn presence_rd_sigma(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Rd(rd) if !rd.sigma.is_empty()))
}

/// FRD bare — FRD without SIGMA. CAPCO §H.6 p111 floor C.
pub(crate) fn presence_frd_bare(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs.aea_markings.iter().any(|a| {
        matches!(
            a,
            marque_ism::AeaMarking::Frd(frd) if frd.sigma.is_empty()
        )
    })
}

/// FRD-SIGMA — any FRD block carrying at least one SIGMA number.
/// CAPCO §H.6 p113 (FRD-SIGMA TS-or-S).
pub(crate) fn presence_frd_sigma(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Frd(frd) if !frd.sigma.is_empty()))
}

/// TFNI present. CAPCO §H.6 p120 floor C.
pub(crate) fn presence_tfni(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Tfni))
}

/// DOD UCNI present. Replaces half of retired E025.
pub(crate) fn presence_dod_ucni(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::DodUcni))
}

/// DOE UCNI present. Replaces half of retired E025.
pub(crate) fn presence_doe_ucni(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::DoeUcni))
}

/// SAR markings present. Replaces retired E027.
pub(crate) fn presence_sar(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs.sar_markings.is_some()
}

/// RSEN dissem control present. CAPCO §H.8 p132 (operative §H.8 p149
/// per §3.4.6 author). RSEN's CVE form is `RS`
/// (the portion-mark abbreviation; banner form is `RSEN`).
pub(crate) fn presence_rsen(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::DissemControl;
    attrs.dissem_iter().any(|d| matches!(d, DissemControl::Rs))
}

/// IMCON dissem control present.
pub(crate) fn presence_imcon(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::DissemControl;
    attrs.dissem_iter().any(|d| matches!(d, DissemControl::Imc))
}

/// ORCON family — ORCON or ORCON-USGOV. The §3.4.6 single family entry
/// covers both because §H.8 p136 (ORCON) and p139 (ORCON-USGOV) both
/// require classification ≥ C and the §3.4.6 author groups them.
pub(crate) fn presence_orcon_family(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::DissemControl;
    attrs
        .dissem_iter()
        .any(|d| matches!(d, DissemControl::Oc | DissemControl::OcUsgov))
}

/// EYES ONLY portion mark / banner form. CAPCO §H.8 p157 (operative
/// §H.8 p152 per `marque-applied.md` Section 3.4.6 author).
pub(crate) fn presence_eyes_only(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::DissemControl;
    attrs
        .dissem_iter()
        .any(|d| matches!(d, DissemControl::Eyes))
}

/// BALK / BOHEMIA / ATOMAL — NATO control markings (not NATO
/// classifications) per CAPCO-2016 §G.2 p40 Table 5 (ARH by
/// Registered Marking).
///
/// PR 9c.1 T134 corrected the structural model:
///   - ATOMAL is an AEA-axis marking (CAPCO-2016 §H.7 p122 worked
///     example `SECRET//RD/ATOMAL//FGI NATO//NOFORN`), shared with
///     NATO+UK under §123/§144 sharing agreements.
///   - BALK / BOHEMIA are NATO SAPs in the SCI category position
///     (§G.2 p40 + §H.7 p127), rendered standalone with no `SAR-`
///     prefix.
///
/// The presence predicates read the corresponding canonical axes:
/// `aea_markings` for ATOMAL, `sci_markings` (via
/// `SciControlSystem::NatoSap`) for BALK / BOHEMIA. Legacy text
/// (`CTSA`, `CTS-B`, `CTS-BALK`, …) canonicalizes through the parser
/// (PR 9c.1 Commit 3), so this predicate fires on both well-formed
/// canonical input and on parsed legacy text.
pub(crate) fn presence_balk(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{NatoSap, SciControlSystem};
    attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::NatoSap(NatoSap::Balk)))
}

pub(crate) fn presence_bohemia(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{NatoSap, SciControlSystem};
    attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::NatoSap(NatoSap::Bohemia)))
}

pub(crate) fn presence_atomal(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::AeaMarking;
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, AeaMarking::Atomal(_)))
}

// ---------------------------------------------------------------------------
// Passthrough family predicates — §3.7 unknown-floor passthrough policy
// ---------------------------------------------------------------------------

/// BUR family — `BUR`, `BUR-BLG`, `BUR-DTP`, `BUR-WRG`. ISM-known SCI
/// control system; specific floor not enumerated in CAPCO-2016.
pub(crate) fn presence_passthrough_bur(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControl, SciControlBare, SciControlSystem};
    let has_via_markings = attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::Published(SciControlBare::Bur)));
    let has_via_controls = attrs.sci_controls.iter().any(|s| {
        matches!(
            s,
            SciControl::Bur | SciControl::BurBlg | SciControl::BurDtp | SciControl::BurWrg
        )
    });
    has_via_markings || has_via_controls
}

/// HCS-X — ISM-known HCS variant; specific floor not enumerated.
pub(crate) fn presence_passthrough_hcs_x(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControl, SciControlBare, SciControlSystem};
    let has_via_markings = attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
            && m.compartments.iter().any(|c| c.identifier.as_str() == "X")
    });
    let has_via_controls = attrs
        .sci_controls
        .iter()
        .any(|s| matches!(s, SciControl::HcsX));
    has_via_markings || has_via_controls
}

/// KLM family — `KLM` / `KLAMATH`, `KLM-R`. ISM-known SCI control system.
pub(crate) fn presence_passthrough_klm(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControl, SciControlBare, SciControlSystem};
    let has_via_markings = attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::Published(SciControlBare::Klm)));
    let has_via_controls = attrs
        .sci_controls
        .iter()
        .any(|s| matches!(s, SciControl::Klm | SciControl::KlmR));
    has_via_markings || has_via_controls
}

/// MVL family — `MVL` / `MARVEL`. ISM-known SCI control system.
pub(crate) fn presence_passthrough_mvl(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControl, SciControlBare, SciControlSystem};
    let has_via_markings = attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::Published(SciControlBare::Mvl)));
    let has_via_controls = attrs
        .sci_controls
        .iter()
        .any(|s| matches!(s, SciControl::Mvl));
    has_via_markings || has_via_controls
}

// ---------------------------------------------------------------------------
// SCI per-system helpers — moved verbatim from rules_sci_per_system.rs
// (helper-relocation Option A per planning doc §4.1)
// ---------------------------------------------------------------------------

/// Is this `SciMarking` anchored on the given published bare system?
pub(crate) fn anchors_on(m: &marque_ism::SciMarking, system: marque_ism::SciControlBare) -> bool {
    use marque_ism::SciControlSystem;
    matches!(&m.system, SciControlSystem::Published(s) if *s == system)
}

/// Does any compartment under this marking carry the given identifier?
pub(crate) fn has_compartment(m: &marque_ism::SciMarking, id: &str) -> bool {
    m.compartments.iter().any(|c| c.identifier.as_str() == id)
}

/// Does the specific compartment carry at least one sub-compartment?
pub(crate) fn compartment_has_sub(m: &marque_ism::SciMarking, comp_id: &str) -> bool {
    m.compartments
        .iter()
        .any(|c| c.identifier.as_str() == comp_id && !c.sub_compartments.is_empty())
}

/// Is this a TK-BLFH, TK-IDIT, or TK-KAND marking (the three TK
/// compartments that require NOFORN per §H.4 p87 / p91 / p95)?
pub(crate) fn is_tk_noforn_compartment(m: &marque_ism::SciMarking) -> bool {
    use marque_ism::SciControlBare;
    anchors_on(m, SciControlBare::Tk)
        && m.compartments
            .iter()
            .any(|c| matches!(c.identifier.as_str(), "BLFH" | "IDIT" | "KAND"))
}
