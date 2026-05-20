// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoScheme` action helpers. Lifted from the monolithic `scheme.rs`
//! per the issue #466 split plan
//! (`claudedocs/refactor-466/split_proposal.md`).
//!
//! Covers `ReplacementIntent` application, category-level adders/removers,
//! page-context-to-attrs projection, foreign-source extraction, and the
//! Pattern-C strip helpers + companion emitters.
//!
//! Stage 2 PR A (issue #466) sub-split this leaf into focused files
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`). Public-API names
//! continue to resolve at `super::actions::NAME` via the re-exports
//! below; sub-module boundaries are an internal organization detail.

mod category_ops;
mod companions;
mod fgi;
mod intent;
// PR 4b-E: `mod page_context` retired alongside the
// `page_context_to_attrs` helper. The lattice-path residue migration
// in `crates/capco/src/scheme/marking.rs` (the `join_via_lattice` body)
// retired the helper's last `#[allow(dead_code)]` consumer.
mod strip;

// `pub(crate)` re-exports — names that need to be reachable at the
// historical `super::actions::NAME` path either through the
// `use self::actions::*;` glob in `scheme/mod.rs` or via explicit
// `super::actions::NAME` paths from sibling leaves (notably
// `rewrites.rs` and `constraints.rs`).
pub(crate) use self::category_ops::{
    capco_axis_mask, capco_category_clear, capco_category_contains, capco_category_has_values,
    capco_category_replace,
};
pub(crate) use self::companions::{
    emit_companion_required, emit_hcs_o_companions, emit_hcs_p_sub_companions, emit_si_g_companions,
};
pub(crate) use self::fgi::{extract_foreign_sources, merge_fgi_markers};
pub(crate) use self::intent::{apply_closure_fact, apply_intent_to_marking};
// PR 4b-E: `page_context_to_attrs` retired with the
// `scheme/actions/page_context.rs` file deletion.
pub(crate) use self::strip::{noop_action, strip_dod_ucni_action, strip_doe_ucni_action};
