// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Per-axis renderer dispatch table — `DissemFamilyMembership` +
//! `AxisRenderRow` + `RENDER_TABLE`.
//!
//! Carved out from `scheme/mod.rs` per the Stage 2 PR B hub-split
//! (issue #466). Module contents are byte-identical to the pre-split
//! source — imports adjusted to pick up the `CAT_*` constants and the
//! `CapcoMarking` type from the parent module via `use super::*;` and
//! the `CategoryId` / `Scope` types directly from `marque_scheme`.

use marque_scheme::{CategoryId, Scope};

use super::*;

// ---------------------------------------------------------------------------
// Per-axis renderer dispatch table (commit 5 populated)
// ---------------------------------------------------------------------------
//
// The dispatch primitive consumed by [`MarkingScheme::render_canonical`].
// One [`AxisRenderRow`] per CAPCO category, in the §A.6 p15-17 Figure 2
// canonical sequence (matches `Category::ordering_rank` declared in
// `build_categories`). The `render_canonical` body walks this table in
// declaration order and inserts the `//` major-category separator
// between consecutive non-empty axis emissions.
//
// The `render` field is a bare function pointer so the table can be
// `const` and shared across `CapcoScheme` instances; per-axis
// renderers cannot capture `&self` or scheme-instance state. All
// inputs come from [`CapcoMarking`] (which wraps
// [`marque_ism::CanonicalAttrs`]) or `&'static` vocabulary tables in
// `crates/capco/src/vocab.rs`.

/// Whether a render row's category is in the §A.6 / §G.1 Table 4
/// row-8 dissem family. Two consecutive emitting rows from the
/// dissem family get a within-category `/` separator instead of
/// the major-category `//` separator (§A.6 p16: "A single forward
/// slash with no interjected space must be used to separate
/// multiple dissemination controls").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DissemFamilyMembership {
    /// Row renders a §H.8 dissem-category axis (single-token
    /// dissems, REL TO, DISPLAY ONLY). Consecutive Members get
    /// `/` between them.
    Member,
    /// Row renders something else (classification, SCI, SAR, AEA,
    /// FGI, non-IC dissem, declassify). Always `//` between this
    /// row and any neighbor that emits.
    Other,
}

/// Per-axis renderer dispatch row.
///
/// `render` writes the axis's canonical bytes for the given `scope`
/// into `out`. Same writer-passing contract as
/// [`MarkingScheme::render_canonical`]: append; do not clear; return
/// `Ok(())` on success.
pub(crate) struct AxisRenderRow {
    /// The category this row renders (e.g., [`CAT_CLASSIFICATION`],
    /// [`CAT_DISSEM`]). Read by the dispatch loop's
    /// [`dissem_family_of`] helper to choose `/` vs `//` between
    /// consecutive emitting rows.
    pub category: CategoryId,
    /// Render the axis's contribution to the canonical form for the
    /// given `scope`, appending bytes to `out`.
    pub render: fn(&CapcoMarking, Scope, &mut dyn core::fmt::Write) -> core::fmt::Result,
}

/// Per-axis renderer dispatch table.
///
/// Order matches `Category::ordering_rank` (CAPCO-2016 §A.6 p15-17
/// Figure 2). The `render_canonical` body walks this table in
/// declaration order; the §A.6 `//` major-category separator is
/// inserted by the dispatch loop, NOT by individual axis renderers.
/// Classification is the sole axis that owns its leading `//` — for
/// non-US / JOINT classifications, the `//` is part of the
/// classification token because it occludes the absent US position
/// (§A.6 p15-16).
pub(crate) const RENDER_TABLE: &[AxisRenderRow] = &[
    AxisRenderRow {
        category: CAT_CLASSIFICATION,
        render: crate::render::render_classification::render_classification,
    },
    AxisRenderRow {
        category: CAT_SCI,
        render: crate::render::render_sci::render_sci,
    },
    AxisRenderRow {
        category: CAT_SAR,
        render: crate::render::render_sar::render_sar,
    },
    AxisRenderRow {
        category: CAT_AEA,
        render: crate::render::render_aea::render_aea,
    },
    AxisRenderRow {
        category: CAT_FGI_MARKER,
        render: crate::render::render_fgi::render_fgi,
    },
    AxisRenderRow {
        category: CAT_DISSEM,
        render: crate::render::render_dissem::render_dissem,
    },
    AxisRenderRow {
        category: CAT_REL_TO,
        render: crate::render::render_rel_to::render_rel_to,
    },
    // DISPLAY ONLY between REL TO and non-IC dissem per CAPCO-2016
    // §G.1 Table 4 row 8 ordering (the IC dissem-category sequence
    // ends with `DISPLAY ONLY [LIST]`). Like REL TO, DISPLAY ONLY
    // carries a country list rather than a single token, so it
    // gets its own renderer (the flat-token `render_dissem` can't
    // emit a list). The category id is reused from `CAT_DISSEM`
    // (DISPLAY ONLY is §H.8 dissem, not §H.9 non-IC) — `category`
    // is informational; dispatch is by declaration order.
    AxisRenderRow {
        category: CAT_DISSEM,
        render: crate::render::render_display_only::render_display_only,
    },
    // Non-IC dissem comes after REL TO in §A.6 sequence (§A.6 p16:
    // "Non-IC Dissemination Control Markings — must follow,
    // Dissemination Controls"). REL TO and DISPLAY ONLY are part
    // of the §H.8 dissem axis; non-IC is its own §H.9 major
    // category. Use the dedicated `CAT_NON_IC_DISSEM` id so the
    // dispatch loop's [`dissem_family_of`] helper correctly emits
    // `//` between this row and the preceding REL TO / DISPLAY
    // ONLY dissem-family rows (not `/`).
    AxisRenderRow {
        category: CAT_NON_IC_DISSEM,
        render: crate::render::render_non_ic_dissem::render_non_ic_dissem,
    },
    // Declassify-on is a no-op in the banner-line dispatch (the CAB
    // is a separate block; see render::render_declassify module doc).
    // Kept in the table so the declassify axis is visible to future
    // CAB-rendering work.
    AxisRenderRow {
        category: CAT_DECLASSIFY_ON,
        render: crate::render::render_declassify::render_declassify,
    },
];
