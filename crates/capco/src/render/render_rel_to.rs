// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! REL TO axis renderer (Authorized for Release To country list).
//!
//! # Authority
//!
//! - CAPCO-2016 §A.6 p16 — REL TO formatting: "Multiple REL TO
//!   countries and/or international organizations must be separated by
//!   commas with an interjected space. The `USA` trigraph code must be
//!   listed first, followed by trigraph codes listed in ascending
//!   alphabetic sort order, then tetragraph codes listed in ascending
//!   alphabetic sort order, e.g., `SECRET//REL TO USA, GBR, JPN, ISAF,
//!   NATO`."
//! - CAPCO-2016 §H.8 p150-151 — `REL TO` (Authorized for Release To):
//!   USA must be present and first; remainder ascending alpha within
//!   the trigraph and tetragraph buckets.
//!
//! # Canonical form
//!
//! `REL TO USA, GBR, JPN, ISAF, NATO`
//!
//! - Always prefixed with `REL TO ` (banner) / `REL TO ` (portion —
//!   the `REL TO` token form is identical in both per §A.6 p15: "the
//!   same order and separators ... used for the banner line").
//! - Country codes are comma-space separated.
//! - USA is first.
//! - Trigraphs (3 chars) sort before tetragraphs (4 chars), each
//!   ascending alpha.
//! - Duplicate trigraphs are deduped (the lattice already ensures
//!   set semantics; the renderer dedupes defensively for resilience
//!   against partial/corrupted projections).
//!
//! Banner and portion forms are identical for REL TO (§A.6 p15).
//!
//! This axis emits nothing when `rel_to` is empty. The IC dissem axis
//! handles the case where a bare `REL` token appears without a
//! country list (the existing `render_dissem` drops that token when
//! REL TO is non-empty; otherwise it emits `REL` alone).

use core::fmt;

use marque_scheme::Scope;
use smallvec::SmallVec;

use crate::scheme::CapcoMarking;

/// Render the REL TO axis to `out`. USA-first, trigraphs alpha,
/// tetragraphs alpha, comma-space separated.
pub(crate) fn render_rel_to(
    m: &CapcoMarking,
    _scope: Scope,
    out: &mut dyn fmt::Write,
) -> fmt::Result {
    if m.0.rel_to.is_empty() {
        return Ok(());
    }

    // §H.8 p151: "`REL TO USA` or `REL USA` (i.e., there is not at
    // least one country trigraph code or tetragraph code following
    // the USA code), is not an authorized marking and is not allowed
    // on US intelligence information." Upstream consumers (parser
    // + lattice projection + rule fixers) are responsible for never
    // producing a single-USA REL TO marking. This debug_assert
    // surfaces upstream invariant violations in dev builds without
    // crashing production renders — if a malformed marking does
    // reach this renderer in release, it emits the unauthorized
    // form rather than panicking, leaving downstream lint rules to
    // catch the violation.
    debug_assert!(
        m.0.rel_to.iter().any(|c| c.as_str() != "USA"),
        "REL TO must contain at least one non-USA trigraph/tetragraph code per §H.8 p151 \
         (CAPCO-2016); got USA-only rel_to (unauthorized form)"
    );

    out.write_str("REL TO ")?;

    // Bucket trigraphs vs tetragraphs (§A.6 p16: "Trigraph codes
    // ... listed first ... then tetragraph codes"). USA always first.
    //
    // Inline-8 / inline-4 keeps the typical REL TO list (≤8 trigraphs,
    // ≤3 tetragraphs in real CAPCO) heap-free on every banner render.
    let mut has_usa = false;
    let mut trigraphs: SmallVec<[&str; 8]> = SmallVec::new();
    let mut tetragraphs: SmallVec<[&str; 4]> = SmallVec::new();
    for c in &m.0.rel_to {
        let s = c.as_str();
        if s == "USA" {
            has_usa = true;
        } else if s.len() == 3 {
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

    if has_usa {
        emit("USA", out, &mut first)?;
    }
    for code in trigraphs {
        emit(code, out, &mut first)?;
    }
    for code in tetragraphs {
        emit(code, out, &mut first)?;
    }
    Ok(())
}
