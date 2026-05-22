// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CAPCO structural lattice types.
//!
//! The types in this module are the lattice-form counterparts to the
//! structural types [`marque_ism::SciMarking`], [`marque_ism::SarMarking`],
//! and [`marque_ism::FgiMarker`] — newtype wrappers that implement the
//! [`marque_scheme`] semilattice traits ([`marque_scheme::JoinSemilattice`]
//! universally; [`marque_scheme::MeetSemilattice`] on the full-lattice
//! types only — see the PR #502 / issue #456 split, which made join-only
//! `DissemSet` / `JointSet` / `DisplayOnlyBlock` non-`Lattice`-satisfying
//! by design) so CAPCO's structural categories compose through the
//! generic engine machinery. Post-PR-4b-E (this module's
//! `*::from_attrs_iter` constructors + free helpers like
//! [`sci_controls_from_markings`]) these helpers ARE the production
//! page-roll-up path — the retired `PageContext::expected_*` accessor
//! surface was the pre-PR-4b-E shape.
//!
//! # Policy (§3.3a of the Phase B design doc)
//!
//! Tree intersection is not unique. For SCI, given `SI-G ABCD` on the
//! left and plain `SI` on the right, the meet could reasonably be (a)
//! `SI-G ABCD` (right's "SI" is the broadest ancestor and survives),
//! (b) just `SI` (drop everything the right side doesn't explicitly
//! name), or (c) empty (only identical leaves survive).
//!
//! This module picks **policy (b)**: meet keeps only elements present
//! at the same depth in both operands. That gives
//! `SI ⊓ SI-G ABCD = SI`, the interpretation closest to the plain
//! lattice definition (`x ⊓ y ≤ x` and `x ⊓ y ≤ y`).
//!
//! Callers that need a different interpretation — primarily the Phase
//! C constraint-evaluator asking "do these two portions share any SCI
//! compartment?" — use [`SciSet::overlaps`] and
//! [`SciSet::common_compartments`] rather than
//! [`marque_scheme::MeetSemilattice::meet`].
//!
//! # SCI storage canonicalization
//!
//! Post-Phase-B, [`SciSet`] is the **canonical** page-context storage
//! for SCI. [`marque_ism::CanonicalAttrs::sci_controls`] (the flat CVE
//! enum projection) stays populated for rules that currently read it
//! but is a compatibility view scheduled for removal once no rule
//! references it (Phase C or D). New rules read `sci_markings` /
//! `SciSet`.
//!
//! # Module layout (post-#565 split)
//!
//! Each lattice type lives in its own submodule with its inherent
//! impls, lattice-trait impls, and per-type `#[cfg(test)] mod tests`.
//! Shared infrastructure (the `HierarchicalTreeSet<K>` storage
//! primitive plus the `sort_smolstrs_by_sar` /
//! `cmp_country_code_trigraph_first` / `sorted_compartment_items`
//! comparators) lives in `helpers`; cross-submodule test fixtures
//! live in `test_support`. The public API surface
//! (`marque_capco::lattice::*`) is preserved verbatim via the
//! re-exports below.

mod helpers;
#[cfg(test)]
pub(crate) mod test_support;

mod aea;
mod classification;
mod declass_exemption;
mod declassify_on;
mod display_only;
mod dissem;
mod fgi;
mod joint;
mod nato_class;
mod nato_dissem;
mod non_ic_dissem;
mod rel_to;
mod sar;
mod sci;

pub use aea::{AeaPrimary, AeaSet, UcniKind};
pub use classification::ClassificationLattice;
pub use declass_exemption::DeclassExemptionAccumulator;
pub use declassify_on::DeclassifyOnLattice;
pub use display_only::DisplayOnlyBlock;
pub use dissem::DissemSet;
pub use fgi::FgiSet;
pub use joint::JointSet;
pub use nato_class::NatoClassLattice;
pub use nato_dissem::NatoDissemSet;
pub use non_ic_dissem::NonIcDissemSet;
pub use rel_to::RelToBlock;
pub use sar::SarSet;
pub use sci::{SciSet, sci_controls_from_markings};
