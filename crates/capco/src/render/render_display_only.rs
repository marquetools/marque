// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! DISPLAY ONLY axis renderer (Authorized for Display Only country
//! list).
//!
//! # Authority
//!
//! - CAPCO-2016 §H.8 p163 — DISPLAY ONLY:
//!     - Authorized Banner Line Marking Title: `DISPLAY ONLY [LIST]`
//!     - Authorized Banner Line Abbreviation: `None`
//!     - Authorized Portion Mark: `DISPLAY ONLY [LIST]`
//!     - Example Banner Line: `SECRET//DISPLAY ONLY IRQ`
//!     - Example Portion Mark: `(S//DISPLAY ONLY IRQ)`
//!     - Example with multiple countries: `CONFIDENTIAL//DISPLAY ONLY AFG, IRQ`
//! - CAPCO-2016 §H.8 p163 — formatting: "`[LIST]` pertains to the
//!   Annex B country trigraph code(s) or Annex A tetragraph code(s),
//!   or Manual, Appendix B NATO/NAC markings used with the DISPLAY
//!   ONLY marking. Country codes are listed alphabetically followed
//!   by tetragraph codes in alphabetical order. Multiple codes must
//!   be separated by commas with an interjected space."
//!
//! # Canonical form
//!
//! `DISPLAY ONLY AFG, IRQ, NATO`
//!
//! - Always prefixed with `DISPLAY ONLY ` — banner and portion forms
//!   are identical (§H.8 p163 lists no abbreviated form; portion mark
//!   is also `DISPLAY ONLY [LIST]`).
//! - Country codes are comma-space separated.
//! - USA is NOT required and NOT prepended — DISPLAY ONLY identifies
//!   the foreign audience permitted to view; release to US recipients
//!   is implicit. Compare REL TO (§H.8 p150-151) which mandates
//!   USA-first because REL TO is a release decision that includes US
//!   release.
//! - Trigraphs (3 chars) sort before tetragraphs (4 chars), each
//!   ascending alpha. The 2-char `EU` and 15-char `AUSTRALIA_GROUP`
//!   registered codes — admitted by `is_country_code` per ODNI ISMCAT
//!   `CVEnumISMCATRelTo.xsd` (340 entries) — bucket alongside
//!   tetragraphs (>=4-char non-3-byte sort group). The §H.8 p163
//!   text says only "country codes ... then tetragraph codes" so the
//!   exact placement of EU and AUSTRALIA_GROUP between the buckets
//!   is mildly under-specified; matching the REL TO renderer's
//!   convention here (length-3 in trigraph bucket, everything else
//!   in tetragraph bucket) keeps the two axes consistent.
//! - Duplicate codes are deduped defensively against partial /
//!   corrupted projections; the lattice already ensures set
//!   semantics upstream.
//!
//! Banner and portion forms are identical (§H.8 p163 — same form
//! for both).
//!
//! This axis emits nothing when `display_only_to` is empty.

use core::fmt;

use marque_scheme::Scope;
use smallvec::SmallVec;

use crate::scheme::CapcoMarking;

/// Render the DISPLAY ONLY axis to `out`. Trigraphs alpha, tetragraphs
/// (and other non-3-char codes) alpha, comma-space separated. USA is
/// NOT prepended (compare REL TO).
pub(crate) fn render_display_only(
    m: &CapcoMarking,
    _scope: Scope,
    out: &mut dyn fmt::Write,
) -> fmt::Result {
    if m.0.display_only_to.is_empty() {
        return Ok(());
    }

    out.write_str("DISPLAY ONLY ")?;

    // Bucket trigraphs (3-byte) vs tetragraphs and other-width codes
    // (everything else). §H.8 p163: "Country codes are listed
    // alphabetically followed by tetragraph codes in alphabetical
    // order." Inline-4 / inline-2 keeps the typical DISPLAY ONLY
    // list (1–3 country codes per the §H.8 p165 worked examples)
    // heap-free on every render.
    let mut trigraphs: SmallVec<[&str; 4]> = SmallVec::new();
    let mut tetragraphs: SmallVec<[&str; 2]> = SmallVec::new();
    for c in &m.0.display_only_to {
        let s = c.as_str();
        if s.len() == 3 {
            trigraphs.push(s);
        } else {
            tetragraphs.push(s);
        }
    }
    trigraphs.sort_unstable();
    trigraphs.dedup();
    tetragraphs.sort_unstable();
    tetragraphs.dedup();

    let mut first = true;
    let emit = |s: &str, out: &mut dyn fmt::Write, first: &mut bool| -> fmt::Result {
        if !*first {
            out.write_str(", ")?;
        }
        *first = false;
        out.write_str(s)
    };

    for code in trigraphs {
        emit(code, out, &mut first)?;
    }
    for code in tetragraphs {
        emit(code, out, &mut first)?;
    }
    Ok(())
}
