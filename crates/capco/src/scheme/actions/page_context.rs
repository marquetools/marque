// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! [`page_context_to_attrs`]: the banner-projection bridge that
//! reads `PageContext::expected_*` accessors and assembles a
//! `CanonicalAttrs` for downstream rule + lattice evaluation.
//! Lifted from the monolithic `actions.rs` per the issue #466 Stage 2
//! PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).

use marque_ism::{CanonicalAttrs, PageContext};

/// Build a `CanonicalAttrs` banner projection from the `expected_*`
/// accessors on `PageContext`. Intentionally narrow: only fills the
/// fields exercised by Phase A's equivalence tests. Other fields land
/// at their defaults, which matches Phase B's goal of handing
/// everything off to scheme-driven aggregation.
///
/// PR 4b-D.2 retired all production call sites. The function survives
/// as the reference shape that the parity-gate helper
/// `project_via_page_context` at
/// `crates/capco/tests/page_context_lattice_parity.rs:48-78` MIRRORS
/// (the parity gate inlines the `expected_*` accessor calls directly
/// rather than calling this function, so the function is not literally
/// "used by" the helper — Copilot R2 #13 caught the prior misnomer).
/// Both call shapes encode the same PageContext-path projection
/// semantics; PR 4b-E retires both alongside the PageContext aggregator.
#[inline]
#[allow(dead_code)]
pub(crate) fn page_context_to_attrs(ctx: &PageContext) -> CanonicalAttrs {
    let mut out = CanonicalAttrs::default();

    // Destructure `expected_non_ic_dissem` up front so both the
    // non-IC dissem assignment below AND the DISPLAY ONLY defensive
    // clear (which fires when a later `*-implies-noforn` rewrite
    // will inject NOFORN at banner) see the same `needs_nf` flag.
    let (non_ic, needs_nf) = ctx.expected_non_ic_dissem();

    out.classification = ctx
        .expected_classification()
        .map(marque_ism::MarkingClassification::Us);
    out.sci_controls = ctx.expected_sci_controls().into_boxed_slice();
    out.sci_markings = ctx.expected_sci_markings();
    out.sar_markings = ctx.expected_sar_marking();
    out.aea_markings = ctx.expected_aea_markings().into_boxed_slice();
    out.fgi_marker = ctx.expected_fgi_marker();
    // PR 9b (T132): page-rollup composes each dissem namespace
    // independently. CAPCO-2016 p41 reciprocity is intrinsic to each
    // portion's attribution; the page-level union preserves it.
    out.dissem_us = ctx.expected_dissem_us().into_boxed_slice();
    out.dissem_nato = ctx.expected_dissem_nato().into_boxed_slice();
    out.rel_to = ctx.expected_rel_to().into_boxed_slice();
    // DISPLAY ONLY axis (Phase 2 / §D.2 Table 3 rows 18-20, 25-27).
    // Cross-axis intersection over (REL TO ∪ DO) with banner-REL-TO
    // and USA subtraction — see `PageContext::expected_display_only`.
    //
    // Belt-and-suspenders defense against deferred NOFORN injection
    // handled by the page-rewrite layer below: per §H.8 p154 + §D.2
    // Table 3 row 2, NOFORN and DISPLAY ONLY cannot coexist in the
    // projected banner. `expected_display_only` already short-
    // circuits to empty when `needs_nf` is true (NODIS/EXDIS/SBU-NF/
    // LES-NF), but this defensive `.clear()` keeps the scheme-layer
    // invariant explicit and survives a future refactor that drops
    // the PageContext-side short-circuit.
    let mut display_only_to = ctx.expected_display_only();
    if needs_nf {
        display_only_to.clear();
    }
    out.display_only_to = display_only_to.into_boxed_slice();
    out.declassify_on = ctx.expected_declassify_on().cloned();
    out.declass_exemption = ctx.expected_declass_exemption();
    // `needs_nf` is also consumed above to suppress DISPLAY ONLY when
    // a later rewrite will inject NOFORN.
    // NOFORN injection into `out.dissem_us` (post PR 9b / FR-046 split;
    // the field was `out.dissem_controls` pre-split) for the non-IC
    // dissem trigger family (SBU-NF/LES-NF classified-context split, and
    // NODIS/EXDIS imply-NF per CAPCO-2016 §H.9 p172 / p174) is handled at
    // the final-projection layer by the PageRewrites
    // `capco/{sbu-nf,les-nf,nodis,exdis}-implies-noforn`
    // (declared in `CapcoScheme::page_rewrites`). Adding a second
    // injection path here would duplicate work the PageRewrites already
    // do and split the "what does the projected page look like?" answer
    // across two code paths. The PageRewrites are authoritative for final
    // mutations on CAT_DISSEM; this function only assembles the
    // intermediate snapshot from raw portion reads. `out.rel_to` (set on
    // the line above) is consistent with the post-rewrite state via the
    // `expected_rel_to` short-circuit that fires whenever `needs_nf` is
    // true.
    out.non_ic_dissem = non_ic.into_boxed_slice();

    out
}
