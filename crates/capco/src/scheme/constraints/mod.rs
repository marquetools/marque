// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoScheme` constraint catalog + `build_categories` /
//! `build_constraints` free constructors.
//!
//! See [`build_constraints`] for the catalog and per-row authority.
//!
//! The catalog body is partitioned by section: `core_catalog` (dyadic
//! Conflicts / Requires / Custom rows), `class_floor_catalog`
//! (per-marking class-floor rows), and `sci_per_system_catalog`
//! (§H.4 per-system rows). `build_constraints` concatenates them in the
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
    class_floor_emit, e012_dual_classification, e014_joint_rel_to_coverage, sci_per_system_emit,
    w005_rel_to_not_in_joint_coverage,
};
// PR-E (#371): tier-1 predicates (`e021_rd_frd_requires_noforn`,
// `e024_rd_precedence`, `e038_dos_dissem_requires_noforn`,
// `e070_frd_tfni_precedence`) moved to
// `crates/capco/src/scheme/predicates/tier1_mask.rs` as
// [`FactBitmask`]-compiled mask-form predicates. The structural slice
// walks were retired in the same commit per project memory
// `feedback_pre_users_no_deprecation_phasing.md` (marque is
// pre-users; alias maps and shim re-exports are not carried).

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
/// 2. [`class_floor_catalog::class_floor_constraints`] — per-marking
///    class-floor rows.
/// 3. [`sci_per_system_catalog::sci_per_system_constraints`] —
///    §H.4 SCI per-system rows.
pub(crate) fn build_constraints() -> Vec<Constraint> {
    let mut out = core_catalog::core_constraints();
    out.extend(class_floor_catalog::class_floor_constraints());
    out.extend(sci_per_system_catalog::sci_per_system_constraints());
    out
}
