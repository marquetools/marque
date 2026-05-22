// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! AEA axis renderer (Atomic Energy Act information markings).
//!
//! # Authority
//!
//! - CAPCO-2016 §A.6 p16 — AEA formatting: "AEA Information Markings
//!   and their subsets must be kept together, connected by a hyphen.
//!   Multiple AEA markings must be listed in the order they appear in
//!   the Register, separated by a single forward slash with no
//!   interjected space. An example may appear as: `SECRET//RD-CNWDI//
//!   REL TO USA, GBR`."
//! - CAPCO-2016 §H.6 p108 — AEA precedence: "If RD, FRD, and TFNI
//!   portions are in a document, the RD takes precedence and is
//!   conveyed in the banner line. In this case, use only the RD
//!   warning statement."
//! - CAPCO-2016 §H.6 Table 4 row 6 (p36 register, expanded p108) —
//!   Register order: RD, CNWDI, SIGMA[#], FRD, SIGMA[#], DOD UCNI,
//!   DOE UCNI, TFNI.
//! - CAPCO-2016 §H.6 — SIGMA compartment numbers must be in numerical
//!   ascending order.
//!
//! # Canonical form
//!
//! Each AEA marking renders as one of (banner / portion):
//! - `RD` / `RD`
//! - `RD-CNWDI` / `RD-CNWDI`
//! - `RD-SIGMA 14 18` / `RD-SG 14 18` (SIGMAs ascending numeric)
//! - `RD-CNWDI-SIGMA 14 18` / `RD-CNWDI-SG 14 18`
//! - `FRD` / `FRD`
//! - `FRD-SIGMA 14` / `FRD-SG 14`
//! - `DOD UCNI` / `DCNI`
//! - `DOE UCNI` / `UCNI`
//! - `TFNI` / `TFNI`
//!
//! Multiple AEA atoms in the same axis are `/`-separated in Register
//! order. Page-level projection — `AeaSet::from_markings` plus the
//! `scheme.project(Scope::Page, …)` PageRewrite catalog — is what
//! enforces RD > FRD > TFNI precedence per §H.6 p104 (a page that
//! sees both RD and FRD projects to RD only). The renderer assumes
//! the projected marking is already canonical with respect to
//! precedence and only handles in-axis sort + form choice.

use core::fmt;

use marque_ism::AeaMarking;
use marque_scheme::Scope;
use smallvec::SmallVec;

use crate::scheme::CapcoMarking;

/// Render the AEA axis to `out`. SIGMA numbers within a single AEA
/// atom are emitted in numerical ascending order; multiple AEA atoms
/// within the same axis are emitted in Register order (RD < FRD <
/// UCNI < TFNI).
pub(crate) fn render_aea(m: &CapcoMarking, scope: Scope, out: &mut dyn fmt::Write) -> fmt::Result {
    if m.0.aea_markings.is_empty() {
        return Ok(());
    }

    // Sort by Register order (§H.6 Table 4 row 6 p36). Inline-4 covers
    // the AEA variants (RD/FRD/TFNI/DCNI/UCNI) on a single marking.
    //
    // Named `fn`-item key adapter (`rank_aea`) for closure-axis
    // monomorphization collapse — R1 WASM-cut per issue #689 and the
    // PR #585 precedent at `crate::lattice::helpers::sort_smolstrs_by_sar`.
    let mut sorted: SmallVec<[&AeaMarking; 4]> = m.0.aea_markings.iter().collect();
    sorted.sort_by_key(rank_aea);

    let portion = matches!(scope, Scope::Portion);

    let mut first = true;
    for aea in sorted {
        if !first {
            out.write_char('/')?;
        }
        first = false;
        write_aea(aea, portion, out)?;
    }
    Ok(())
}

/// Register order per CAPCO-2016 Table 4 row 6 (p36, expanded p108).
/// Lower rank = earlier in register.
///
/// ATOMAL (PR 9c.1 T134) lands at rank 5 — last in the AEA register
/// — per the §H.7 p122 worked example `SECRET//RD/ATOMAL//FGI NATO//
/// NOFORN`. The manual's Register Table 4 row 6 does not enumerate
/// ATOMAL in the AEA sequence (Table 4 lists only US-domestic AEA
/// markings); the §H.7 worked example shows RD before ATOMAL, and
/// the §H.7 prose ("ATOMAL is a NATO Atomic Energy Act marking
/// that follows the registered US Atomic Energy Act marking RD")
/// confirms ATOMAL trails the US-domestic AEA family.
///
/// The wildcard arm covers any future variant added to the
/// `#[non_exhaustive]` `AeaMarking` enum — such a variant lands at
/// the end pending a re-audit against the Register.
fn register_rank(aea: &AeaMarking) -> u8 {
    match aea {
        AeaMarking::Rd(_) => 0,
        AeaMarking::Frd(_) => 1,
        AeaMarking::DodUcni => 2,
        AeaMarking::DoeUcni => 3,
        AeaMarking::Tfni => 4,
        AeaMarking::Atomal(_) => 5,
        _ => u8::MAX,
    }
}

fn write_aea(aea: &AeaMarking, portion: bool, out: &mut dyn fmt::Write) -> fmt::Result {
    // SIGMA portion-form uses `SG`; banner-form uses `SIGMA` per
    // §H.6 Table 4 row 6 p36.
    let sigma_label = if portion { "SG" } else { "SIGMA" };

    match aea {
        AeaMarking::Rd(rd) => {
            out.write_str("RD")?;
            if rd.cnwdi {
                out.write_str("-CNWDI")?;
            }
            write_sigma(&rd.sigma, sigma_label, out)?;
        }
        AeaMarking::Frd(frd) => {
            out.write_str("FRD")?;
            write_sigma(&frd.sigma, sigma_label, out)?;
        }
        AeaMarking::DodUcni => {
            out.write_str(if portion { "DCNI" } else { "DOD UCNI" })?;
        }
        AeaMarking::DoeUcni => {
            out.write_str(if portion { "UCNI" } else { "DOE UCNI" })?;
        }
        AeaMarking::Tfni => {
            out.write_str("TFNI")?;
        }
        // PR 9c.1 T134: ATOMAL renders same-form across banner and
        // portion (CAPCO-2016 §G.1 Table 4 p38 row "ATOMAL" — same
        // canonical name in all three columns). No sub-markings to
        // emit; `AtomalBlock` is the empty carrier.
        AeaMarking::Atomal(_) => {
            out.write_str("ATOMAL")?;
        }
        // `AeaMarking` is `#[non_exhaustive]`. Future variants land
        // here pending a re-audit against §H.6 / §H.7; emit nothing
        // rather than panic.
        _ => {}
    }
    Ok(())
}

/// `sort_by_key` adapter — `slice::sort_by_key` over `&[&AeaMarking]`
/// invokes its key fn with `&&AeaMarking`; [`register_rank`] takes
/// `&AeaMarking`. Named `fn`-item (not closure) for closure-axis
/// monomorphization collapse per R1 / issue #689. The `&&AeaMarking
/// → &AeaMarking` reduction is auto-deref'd inside the adapter body.
fn rank_aea(a: &&AeaMarking) -> u8 {
    register_rank(a)
}

fn write_sigma(sigma: &[u8], label: &str, out: &mut dyn fmt::Write) -> fmt::Result {
    if sigma.is_empty() {
        return Ok(());
    }
    out.write_char('-')?;
    out.write_str(label)?;
    // Numerical ascending sort per §H.6. Inline-8 conservatively covers
    // the four current SIGMA values (14, 15, 18, 20 per CAPCO-2016 §H.6
    // p108 + p113); even counting obsolete SIGMAs (1-5 and 9-13 per
    // §H.6 p109, retired and must not be carried forward to new
    // markings), real-world banners never approach this ceiling.
    let mut numeric: SmallVec<[u8; 8]> = SmallVec::from_slice(sigma);
    numeric.sort_unstable();
    for n in numeric {
        out.write_char(' ')?;
        write!(out, "{n}")?;
    }
    Ok(())
}
