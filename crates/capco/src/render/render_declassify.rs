// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Declassify-on axis renderer.
//!
//! # Authority
//!
//! - CAPCO-2016 §E.1 (Original Classification Authority) — the
//!   Classification Authority Block (CAB) line "Declassify On". The
//!   CAB is a separate block from the banner line and portion mark;
//!   it carries the declassification date / event / exemption.
//! - CAPCO-2016 §E.3 (Multiple Sources and the Declassify On Line
//!   Hierarchy) — only one value per CAB. When multiple sources
//!   contribute, use the longest-duration value. The lattice form
//!   `DeclassifyOnLattice` (in `crates/capco/src/lattice.rs`)
//!   composes per-portion values via the `MaxDate` projection
//!   declared on `Category::aggregation = AggregationOp::MaxDate`
//!   in `CapcoScheme::build_categories`.
//!
//! # Canonical form (banner / portion line)
//!
//! **The CAB is not rendered inline with the banner / portion line.**
//! Per CAPCO-2016 §E.1, the banner line is
//! `CLASSIFICATION//SCI//SAR//AEA//FGI//DISSEM//NON-IC` — the CAB
//! ("Classified By", "Derived From", "Declassify On") lives on its
//! own block elsewhere on the page (typically the bottom of the
//! cover page).
//!
//! This renderer therefore emits **nothing** for `Scope::Portion |
//! Page | Document`. The CAB block's renderer is a separate concern
//! (not yet implemented in the workspace). When a future commit adds
//! CAB rendering, it dispatches through a separate `render_cab`
//! function (potentially via a separate `Scope::Cab` variant or a
//! parallel `render_canonical_cab` method); this axis function stays
//! a no-op for the banner axis.

use core::fmt;

use marque_scheme::Scope;

use crate::scheme::CapcoMarking;

/// Render the declassify-on axis to `out`.
///
/// **No-op for the banner / portion line** — the Declassify On
/// value is part of the CAB, not the banner. See module doc.
pub(crate) fn render_declassify(
    _m: &CapcoMarking,
    _scope: Scope,
    _out: &mut dyn fmt::Write,
) -> fmt::Result {
    Ok(())
}
