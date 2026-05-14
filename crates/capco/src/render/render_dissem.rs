// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! IC dissemination control axis renderer.
//!
//! # Authority
//!
//! - CAPCO-2016 §A.6 p16 — Dissemination Control formatting: "A single
//!   forward slash with no interjected space must be used to separate
//!   multiple dissemination controls. Multiple dissemination controls
//!   must be listed in the order they appear in the Register."
//! - CAPCO-2016 §H.8 — Dissemination control category. Banner uses
//!   long form (e.g., `NOFORN`); portion uses abbreviation (e.g.,
//!   `NF`). The mapping is given by the Register Table 4 row 8 (p36).
//!
//! # Canonical form
//!
//! - Banner: `ORCON/NOFORN/RELIDO` — long forms, `/`-separated, in
//!   Register order.
//! - Portion: `OC/NF/RELIDO` — short/portion forms, `/`-separated, in
//!   Register order.
//!
//! The Register order (§H.8 Table 4 row 8 p36) is:
//! RSEN, FOUO, ORCON, ORCON-USGOV, IMCON, NOFORN, PROPIN, REL TO,
//! RELIDO, EYES ONLY, DSEN, FISA, DISPLAY ONLY.
//!
//! This axis intentionally does NOT render `REL TO` — that lives in
//! its own axis (`render_rel_to`) so the renderer can sort countries
//! and dispatch the trigraph/tetragraph ordering independently. The
//! bare `REL` token (DissemControl::Rel without a country list) is
//! also dropped at render time when REL TO is non-empty (per
//! `PageContext::render_expected_banner` precedent — emit `REL TO ...`
//! once, not also a bare `REL`).

use core::fmt;

use marque_ism::DissemControl;
use marque_scheme::Scope;
use smallvec::SmallVec;

use crate::scheme::CapcoMarking;

/// Render the IC dissem axis (excluding REL TO, which has its own
/// axis).
pub(crate) fn render_dissem(
    m: &CapcoMarking,
    scope: Scope,
    out: &mut dyn fmt::Write,
) -> fmt::Result {
    let portion = matches!(scope, Scope::Portion);

    // Filter: drop bare `REL` when REL TO is non-empty (the REL TO
    // axis emits `REL TO USA, ...` instead).
    let drop_bare_rel = !m.0.rel_to.is_empty();

    // Sort by Register order (§H.8 Table 4 row 8 p36). The CVE
    // `DissemControl` enum already declares variants in roughly this
    // order; we re-sort defensively to honor the precedent.
    //
    // PR 9b (T132): render walks the unified `dissem_iter` (US-then-
    // NATO) and lets the Register-order sort below merge them. The
    // canonical wire form is namespace-indistinguishable — CAPCO-2016
    // p41 reciprocity means OC/REL TO render the same way regardless
    // of attribution — so the renderer joins the two and emits one
    // sequence per §A.6 ordering.
    //
    // Inline-4 covers the typical dissem set (NF/PR/OC/REL, IMCON, RS);
    // longer compositions spill to heap cleanly.
    let mut sorted: SmallVec<[&DissemControl; 4]> =
        m.0.dissem_iter()
            .filter(|d| !(drop_bare_rel && **d == DissemControl::Rel))
            .collect();
    sorted.sort_by_key(|d| register_rank(d));

    let mut first = true;
    for d in sorted {
        if !first {
            out.write_char('/')?;
        }
        first = false;
        let s = if portion {
            portion_str(d)
        } else {
            banner_str(d)
        };
        out.write_str(s)?;
    }
    Ok(())
}

/// Register order rank per CAPCO-2016 Table 4 row 8 (p36).
fn register_rank(d: &DissemControl) -> u8 {
    match d {
        DissemControl::Rs => 0,
        DissemControl::Fouo => 1,
        DissemControl::Oc => 2,
        DissemControl::OcUsgov => 3,
        DissemControl::Imc => 4,
        DissemControl::Nf => 5,
        DissemControl::Pr => 6,
        DissemControl::Rel => 7,
        DissemControl::Relido => 8,
        DissemControl::Eyes => 9,
        DissemControl::Dsen => 10,
        // RAWFISA is not listed in CAPCO-2016 (neither in the Register
        // table nor §H.8); it appears in the ODNI ISM schema only.
        // Position between DSEN (rank 10) and FISA (rank 12) is an
        // inference from sibling ordering — re-audit when a CAPCO
        // citation becomes available. (Constitution VIII "too new to
        // cite" carve-out applies.)
        DissemControl::Rawfisa => 11,
        DissemControl::Fisa => 12,
        DissemControl::Displayonly => 13,
        DissemControl::ExemptFromIcd501Discovery => 14,
        // Defensive: any future variant lands at the end pending a
        // re-audit against the Register.
        _ => u8::MAX,
    }
}

/// Banner form per CAPCO-2016 Table 4 row 8 (p36). The CVE `as_str()`
/// method returns the portion form, not the banner form, for several
/// entries (`OC` vs `ORCON`, `NF` vs `NOFORN`, etc.) — this mapping
/// is the §A.6 banner form used in `PageContext::render_expected_banner`
/// precedent.
fn banner_str(d: &DissemControl) -> &'static str {
    match d {
        DissemControl::Rs => "RSEN",
        DissemControl::Fouo => "FOUO",
        DissemControl::Oc => "ORCON",
        DissemControl::OcUsgov => "ORCON-USGOV",
        DissemControl::Imc => "IMCON",
        DissemControl::Nf => "NOFORN",
        DissemControl::Pr => "PROPIN",
        DissemControl::Rel => "REL",
        DissemControl::Relido => "RELIDO",
        DissemControl::Eyes => "EYES ONLY",
        DissemControl::Dsen => "DSEN",
        DissemControl::Rawfisa => "RAWFISA",
        DissemControl::Fisa => "FISA",
        DissemControl::Displayonly => "DISPLAY ONLY",
        DissemControl::ExemptFromIcd501Discovery => "EXEMPT FROM ICD501 DISCOVERY",
        _ => d.as_str(),
    }
}

/// Portion form per CAPCO-2016 Table 4 row 8 (p36). For most variants
/// this is `DissemControl::as_str()`, but the explicit mapping keeps
/// the renderer's intent visible.
fn portion_str(d: &DissemControl) -> &'static str {
    d.as_str()
}
