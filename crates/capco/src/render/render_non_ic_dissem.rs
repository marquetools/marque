// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Non-IC dissemination control axis renderer.
//!
//! # Authority
//!
//! - CAPCO-2016 §A.6 p16 — Non-IC Dissem formatting: "A single forward
//!   slash with no interjected space must be used to separate multiple
//!   controls in the category. Multiple Non-IC dissemination controls
//!   must be listed in the order they appear in the Register. In the
//!   portion mark for non-IC Dissemination Control Markings, the
//!   marking and its sub-marking must be kept together, connected by a
//!   hyphen, (i.e., the portion mark for `SBU NOFORN` is `SBU-NF`)."
//! - CAPCO-2016 §H.9 — Non-IC dissem (LIMDIS, EXDIS, NODIS, SBU,
//!   SBU NOFORN, LES, LES NOFORN, SSI). Per-marking propagation rules
//!   are encoded on `NonIcDissem::propagates_to_classified_banner`;
//!   the renderer emits whatever the projected page state contains.
//!   See §H.9 pp 169-191 for the per-marking treatment.
//!
//! # Canonical form
//!
//! - Banner: `EXDIS/NODIS` — `/`-separated, Register order, banner
//!   forms (`SBU NOFORN`, `LES NOFORN` with space, no hyphen).
//! - Portion: `XD/ND` — `/`-separated, Register order, portion forms
//!   (`SBU-NF`, `LES-NF` with hyphen — the §A.6 p16 explicit
//!   "marking and its sub-marking must be kept together, connected by
//!   a hyphen" rule).
//!
//! Register order per CAPCO-2016 Table 4 row 9 (p36):
//! LIMDIS, EXDIS, NODIS, SBU, SBU-NF, LES, LES-NF, SSI.

use core::fmt;

use marque_ism::NonIcDissem;
use marque_scheme::Scope;
use smallvec::SmallVec;

use crate::scheme::CapcoMarking;

/// Render the non-IC dissem axis to `out`.
pub(crate) fn render_non_ic_dissem(
    m: &CapcoMarking,
    scope: Scope,
    out: &mut dyn fmt::Write,
) -> fmt::Result {
    if m.0.non_ic_dissem.is_empty() {
        return Ok(());
    }

    let portion = matches!(scope, Scope::Portion);

    // Sort by Register order (§H.9 Table 4 row 9 p36). Inline-4
    // covers FOUO/SBU/LIMDIS/UCNI / DCNI / LES — the practical
    // ceiling on simultaneous non-IC dissem tokens.
    let mut sorted: SmallVec<[&NonIcDissem; 4]> = m.0.non_ic_dissem.iter().collect();
    sorted.sort_by_key(|n| register_rank(n));

    let mut first = true;
    for n in sorted {
        if !first {
            out.write_char('/')?;
        }
        first = false;
        let s = if portion {
            n.portion_str()
        } else {
            n.banner_str()
        };
        out.write_str(s)?;
    }
    Ok(())
}

fn register_rank(n: &NonIcDissem) -> u8 {
    // NNPI placed after SSI (rank 8) for register ordering.
    // Placement is local-policy — CAPCO-2016 §H.9 has no NNPI
    // Register row — but pairing NNPI with SSI is consistent with
    // their shared `propagates_to_classified_banner = true` semantic
    // documented on the `NonIcDissem::Nnpi` variant in
    // `crates/ism/src/attrs.rs`.
    //
    // `NonIcDissem` is `#[non_exhaustive]` (declared upstream in
    // `marque-ism`), so the wildcard arm is required by the
    // compiler — Rust treats out-of-crate `#[non_exhaustive]` as
    // open even when every published variant is named. The wildcard
    // body is intentionally `u8::MAX` (not a panic) so that, if a
    // future ODNI schema bump adds a variant before the
    // capco render rank is updated, the unknown sorts to the
    // tail rather than panicking on the hot path. The drift signal
    // is the missing render-rank entry; the test that catches it
    // lives at the integration boundary (a future render-rank
    // exhaustivity test would assert every public variant has a
    // distinct rank).
    match n {
        NonIcDissem::Limdis => 0,
        NonIcDissem::Exdis => 1,
        NonIcDissem::Nodis => 2,
        NonIcDissem::Sbu => 3,
        NonIcDissem::SbuNf => 4,
        NonIcDissem::Les => 5,
        NonIcDissem::LesNf => 6,
        NonIcDissem::Ssi => 7,
        NonIcDissem::Nnpi => 8,
        _ => u8::MAX,
    }
}
