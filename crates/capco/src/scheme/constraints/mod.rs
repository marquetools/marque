// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoScheme` constraint catalog + `build_categories`/`build_constraints`
//! free constructors. Lifted from the monolithic `scheme.rs` per the issue
//! #466 split plan (`claudedocs/refactor-466/split_proposal.md`, Risk 1
//! Option 2).
//!
//! See [`build_constraints`] for the catalog and per-row authority.
//!
//! Stage 2 PR A (issue #466) sub-split this leaf into focused files
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`). The catalog
//! body is partitioned by section: `core_catalog` (dyadic Conflicts /
//! Requires / Custom rows), `class_floor_catalog` (PR 3b.D §3.4.6
//! class-floor rows), and `sci_per_system_catalog` (PR 3b.E §H.4
//! per-system rows). `build_constraints` concatenates them in the
//! original order — row order is load-bearing for the predicate
//! evaluator's tiebreakers.

use marque_scheme::{Category, Constraint};

mod categories;
mod class_floor_catalog;
mod core_catalog;
mod helpers;
mod sci_per_system_catalog;

// `pub(crate)` re-exports — historical `super::constraints::NAME`
// paths used by sibling leaves (`predicates/satisfies.rs`,
// `predicates/class_floor.rs`, `predicates/sci_per_system.rs`)
// continue to resolve.
pub(crate) use self::helpers::{
    class_floor_emit, e012_dual_classification, e014_joint_rel_to_coverage,
    e021_aea_requires_noforn, e024_rd_precedence, e038_dos_dissem_requires_noforn,
    sci_per_system_emit,
};

/// Build the scheme's category table. Lives in
/// [`categories`](self::categories::build_categories).
pub(crate) fn build_categories() -> Vec<Category> {
    categories::build_categories()
}

/// Build the CAPCO declarative constraint catalog by concatenating
/// the three section helpers IN ORDER.
///
/// **Constraint order is load-bearing.** The predicate evaluator
/// may rely on declaration order for tiebreakers; this concatenation
/// preserves the exact ordering present pre-split:
///
/// 1. [`core_catalog::core_constraints`] — dyadic Conflicts /
///    Requires / Custom rows (E010 through E057, plus
///    `capco/joint-requires-usa`).
/// 2. [`class_floor_catalog::class_floor_constraints`] — PR 3b.D
///    §3.4.6 per-marking class-floor rows.
/// 3. [`sci_per_system_catalog::sci_per_system_constraints`] —
///    PR 3b.E §H.4 SCI per-system rows.
pub(crate) fn build_constraints() -> Vec<Constraint> {
    let mut out = core_catalog::core_constraints();
    out.extend(class_floor_catalog::class_floor_constraints());
    out.extend(sci_per_system_catalog::sci_per_system_constraints());
    out
}
