// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Pattern-C strip-row triggers (FOUO / LIMDIS / SBU / UCNI on
//! classified pages) and the UCNI NOFORN-promotion siblings. Lifted
//! from the monolithic `predicates.rs` per the issue #466 Stage 2
//! PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).

use super::super::*;

// ---------------------------------------------------------------------------
// PR 4b-C Commit 3 — Pattern-C strip-row helpers
// ---------------------------------------------------------------------------
//
// Pattern C (classification-driven strip) and the UCNI NOFORN-promotion
// pair both need predicates that gate on "the page is classified". The
// existing `CategoryPredicate::Contains` shape can't express that gate,
// so the seven rows in this PR use `CategoryPredicate::Custom`. The
// helpers below are top-level `fn` items so the rows can store them as
// `fn` pointers (`CategoryPredicate::Custom(fn(&CapcoMarking) -> bool)`)
// and the `Send + Sync` invariant from Constitution VI holds trivially.
//
// Authority (each verified 2026-05-16 against
// `crates/capco/docs/CAPCO-2016.md`):
// - §H.8 p134 (FOUO Precedence Rules for Banner Line Guidance)
// - §H.6 p116-117 (DOD UCNI / DCNI Precedence Rules)
// - §H.6 p118-119 (DOE UCNI Precedence Rules)
// - §H.9 p170 (LIMDIS Precedence Rules)
// - §H.9 p176 (SBU Precedence Rules)

/// `true` when the marking carries a classification level strictly
/// greater than UNCLASSIFIED.
///
/// Classifications without an effective US-level (`None`) are
/// treated as UNCLASSIFIED — Pattern-C rules fire only when there is
/// affirmative classified state on the page. Matches the §H.8 / §H.6
/// / §H.9 wording "classified document" (which presupposes a positive
/// classification, not a no-classification state).
#[inline]
pub(crate) fn is_classified(m: &CapcoMarking) -> bool {
    m.0.classification
        .as_ref()
        .map(|c| c.effective_level() > marque_ism::Classification::Unclassified)
        .unwrap_or(false)
}

/// `true` when the projected page carries DOD UCNI (DCNI) anywhere
/// on the AEA axis. Used by the Pattern-C
/// `capco/dod-ucni-evicted-by-classified` and
/// `capco/dod-ucni-promotes-noforn-when-classified` predicates.
#[inline]
pub(crate) fn has_dod_ucni(m: &CapcoMarking) -> bool {
    m.0.aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::DodUcni))
}

/// `true` when the projected page carries DOE UCNI anywhere on the
/// AEA axis. Mirrors [`has_dod_ucni`] for the DOE side.
#[inline]
pub(crate) fn has_doe_ucni(m: &CapcoMarking) -> bool {
    m.0.aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::DoeUcni))
}

/// `true` when an FD&R dissem marker is already present on the page.
///
/// §H.6 p116 / p118 verbatim: "NOFORN must be applied if a less
/// restrictive FD&R marking would otherwise be conveyed with the
/// classified information." The promotion is suppressed when an
/// equally- or more-restrictive FD&R marker is already present;
/// NOFORN is the most-restrictive member of the FD&R family
/// (§H.8 p145), so a present NOFORN is its own suppressor (no
/// double-add). The other FD&R-family tokens (REL TO / RELIDO /
/// DISPLAY ONLY / EYES) are "less restrictive" and DO NOT suppress
/// the promotion — they would be cleared by `noforn-clears-rel-to`
/// / `noforn-clears-fdr-family` downstream once the promotion fires.
///
/// Set membership matches the §H.8 p145 NOFORN-dominates family
/// scoped to "would otherwise be conveyed in the banner". We check
/// only `Nf` here so the promotion fires whenever NOFORN is absent
/// from the dissem axis. This is consistent with how the existing
/// `*-implies-noforn` rewrites add NOFORN with no FD&R-suppressor
/// gate (FactAdd of an already-present token is a per-intent no-op
/// per the idempotence policy in `apply_fact_add`).
#[inline]
pub(crate) fn dissem_has_noforn(m: &CapcoMarking) -> bool {
    m.0.dissem_iter()
        .any(|d| matches!(d, marque_ism::DissemControl::Nf))
}

/// Pattern-C trigger: `classification > U ∧ contains FOUO in dissem`.
/// Drives `capco/fouo-evicted-by-classified` (§H.8 p134).
pub(crate) fn fouo_classified_trigger(m: &CapcoMarking) -> bool {
    is_classified(m)
        && m.0
            .dissem_iter()
            .any(|d| matches!(d, marque_ism::DissemControl::Fouo))
}

/// Pattern-C trigger: `classification > U ∧ contains LIMDIS in non_ic`.
/// Drives `capco/limdis-evicted-by-classified` (§H.9 p170).
pub(crate) fn limdis_classified_trigger(m: &CapcoMarking) -> bool {
    is_classified(m)
        && m.0
            .non_ic_dissem
            .iter()
            .any(|d| matches!(d, marque_ism::NonIcDissem::Limdis))
}

/// Pattern-C trigger: `classification > U ∧ contains SBU in non_ic`.
/// Drives `capco/sbu-evicted-by-classified` (§H.9 p176 banner-roll-up
/// rule for bare SBU portions commingled with classified portions).
///
/// Matches the bare `Sbu` variant only; the compound `SbuNf` variant
/// is matched by the parallel [`sbu_nf_classified_trigger`] below
/// (driving the analogous `capco/sbu-nf-evicted-by-classified` strip
/// per §H.9 p178 line 4421 — see that trigger's doc-comment for the
/// §3.5 carve-out rationale). #541.
pub(crate) fn sbu_classified_trigger(m: &CapcoMarking) -> bool {
    is_classified(m)
        && m.0
            .non_ic_dissem
            .iter()
            .any(|d| matches!(d, marque_ism::NonIcDissem::Sbu))
}

/// Pattern-C trigger: `classification > U ∧ contains SBU-NF in non_ic`.
/// Drives `capco/sbu-nf-evicted-by-classified` (§H.9 p178 line 4421).
///
/// §H.9 p178 line 4421 (SBU NOFORN Commingling Rule(s) Within a
/// Portion): *"If the portion is classified, the classification level
/// of the portion adequately protects the SBU information, so SBU is
/// not reflected in the portion mark; however a NOFORN marking must
/// be added to the portion mark, e.g., (C//NF)."*
///
/// # §3.5 compound-NF carve-out for SBU-NF (not LES-NF)
///
/// The earlier §3.5 compound-NF invariant said "Pattern-C strip rows
/// MUST NOT touch SbuNf/LesNf because the parallel implies-noforn
/// rewrites carry NF identity separately." That invariant is correct
/// for **LES-NF** — §H.9 p185 line 4557-4558 explicitly says the LES
/// marking survives classification. It is **wrong** for **SBU-NF** —
/// §H.9 p178 line 4421 explicitly says SBU vanishes on classified
/// portions.
///
/// The asymmetry traces to the regulatory authority each marking
/// carries: SBU is administrative-protection-only and classification
/// subsumes it; LES carries independent law-enforcement legal-process
/// discipline (the §H.9 p182 LES Warning Statement, originator-control
/// per §H.9 p186 Notes, prohibition on legal-proceedings use without
/// originator authorization) that classification does NOT subsume.
/// See `NonIcDissemSet`'s type-level doc-comment for the full
/// rationale.
///
/// Co-fires with the existing Pattern-A `capco/sbu-nf-implies-noforn`
/// rewrite (which adds NOFORN to dissem unconditionally on any
/// SBU-NF presence per §H.9 p178 line 4396 banner form). The two
/// touch different axes — Pattern-A writes CAT_DISSEM (FactAdd
/// NOFORN); this row writes CAT_NON_IC_DISSEM (FactRemove SBU-NF) —
/// so they compose cleanly without scheduling conflict. #541.
pub(crate) fn sbu_nf_classified_trigger(m: &CapcoMarking) -> bool {
    is_classified(m)
        && m.0
            .non_ic_dissem
            .iter()
            .any(|d| matches!(d, marque_ism::NonIcDissem::SbuNf))
}

/// Pattern-C trigger: `classification > U ∧ DOD UCNI on AEA axis`.
/// Drives `capco/dod-ucni-evicted-by-classified` (§H.6 p116).
pub(crate) fn dod_ucni_classified_trigger(m: &CapcoMarking) -> bool {
    is_classified(m) && has_dod_ucni(m)
}

/// Pattern-C trigger: `classification > U ∧ DOE UCNI on AEA axis`.
/// Drives `capco/doe-ucni-evicted-by-classified` (§H.6 p118).
pub(crate) fn doe_ucni_classified_trigger(m: &CapcoMarking) -> bool {
    is_classified(m) && has_doe_ucni(m)
}

/// Pattern-C trigger: `dod-ucni-classified ∧ NOFORN absent from dissem`.
/// Drives `capco/dod-ucni-promotes-noforn-when-classified` (§H.6 p116).
pub(crate) fn dod_ucni_promotes_noforn_trigger(m: &CapcoMarking) -> bool {
    dod_ucni_classified_trigger(m) && !dissem_has_noforn(m)
}

/// Pattern-C trigger: `doe-ucni-classified ∧ NOFORN absent from dissem`.
/// Drives `capco/doe-ucni-promotes-noforn-when-classified` (§H.6 p118).
pub(crate) fn doe_ucni_promotes_noforn_trigger(m: &CapcoMarking) -> bool {
    doe_ucni_classified_trigger(m) && !dissem_has_noforn(m)
}
