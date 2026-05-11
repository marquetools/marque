// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! FGI marker axis renderer (Foreign Government Information).
//!
//! # Authority
//!
//! - CAPCO-2016 §A.6 p16 — FGI formatting: "Multiple FGI trigraph
//!   country codes or tetragraph codes must be separated by a single
//!   space. A tetragraph is a four-letter code ... used to represent
//!   an international organization, alliance, or coalition. Trigraph
//!   codes used with the FGI marking must be listed first in ascending
//!   alphabetic sort order, followed by tetragraph codes listed in
//!   ascending alphabetic sort order. An example may appear as:
//!   `SECRET//FGI GBR JPN NATO//REL TO USA, GBR, JPN, NATO`."
//! - CAPCO-2016 §H.7 p123 — FGI marker variants: source-acknowledged
//!   (`FGI [LIST]`) vs source-concealed (`FGI` with no country list).
//!   "If a document contains any source-concealed FGI portions
//!   alongside source-acknowledged FGI portions, the banner must use
//!   `FGI` without country codes — revealing the country list would
//!   compromise the concealed source."
//!
//! # Canonical form
//!
//! - Source-concealed: `FGI`
//! - Source-acknowledged: `FGI GBR JPN NATO` (trigraphs alpha first,
//!   then tetragraphs alpha)
//!
//! Banner and portion forms are identical for the FGI marker (per
//! §A.6 p15 portion uses the same separators as banner).
//!
//! This axis is only emitted when an FGI marker is **present** as a
//! page-level rollup. The classification axis emits FGI when the
//! marking's primary classification system is FGI (`//GBR S`); this
//! axis emits the FGI marker that appears in addition to (or as an
//! alternative to) the classification (e.g., `SECRET//FGI GBR//
//! REL TO USA, GBR`). The two are populated by different parser
//! paths and the renderer treats them independently.

use core::fmt;

use marque_ism::FgiMarker;
use marque_scheme::Scope;

use crate::scheme::CapcoMarking;

/// Render the FGI-marker axis to `out`.
pub(crate) fn render_fgi(m: &CapcoMarking, _scope: Scope, out: &mut dyn fmt::Write) -> fmt::Result {
    let Some(marker) = &m.0.fgi_marker else {
        return Ok(());
    };

    out.write_str("FGI")?;
    match marker {
        FgiMarker::SourceConcealed => Ok(()),
        FgiMarker::Acknowledged { countries, .. } => {
            // Trigraphs first (alpha), then tetragraphs (alpha) per
            // §A.6 p16. A trigraph is a 3-character code; a
            // tetragraph is a 4-character code.
            let mut trigraphs: Vec<&str> = Vec::new();
            let mut tetragraphs: Vec<&str> = Vec::new();
            for c in countries {
                let s = c.as_str();
                if s.len() == 3 {
                    trigraphs.push(s);
                } else {
                    // 4 chars (tetragraph) per §A.6 p16; preserve
                    // anything else as-is in the tetragraph bucket
                    // (defensive — the parser already validates the
                    // shape).
                    tetragraphs.push(s);
                }
            }
            trigraphs.sort_unstable();
            tetragraphs.sort_unstable();
            for code in trigraphs.into_iter().chain(tetragraphs) {
                out.write_char(' ')?;
                out.write_str(code)?;
            }
            Ok(())
        }
    }
}
