// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Post-PR-4b compile-time lattice impl pin.
//!
//! Locks the exact set of [`JoinSemilattice`] / [`MeetSemilattice`] /
//! [`BoundedJoinSemilattice`] / [`BoundedMeetSemilattice`] impls on
//! `marque-capco` lattice types. Catches the trait-bound drift class
//! (silently adding or removing a trait impl) at build time, complementing the
//! runtime exact-set pin at
//! `crates/capco/tests/post_4b_lattice_inventory_pin.rs`.
//!
//! ## Why two pins?
//!
//! The compile-time `assert_impl_all!` block fires before any test
//! binary runs. A rename or removal of a `JoinSemilattice` impl on
//! one of the locked types is a build error, not a test failure.
//! The runtime inventory pin catches the orthogonal drift class
//! ("row renamed at the same count" / "row swapped at the same
//! count") that the type system cannot see.
//!
//! ## Join-only invariant (issue #456 / PR #502 split)
//!
//! Three types — [`DissemSet`], [`JointSet`], [`DisplayOnlyBlock`] —
//! implement only [`JoinSemilattice`]. The reasons differ but the
//! shape does not:
//!
//! - [`DissemSet`] carries `relido_observed_unanimous` — a running
//!   join-side observation per CAPCO-2016 §H.8 pp 155-156 that
//!   cannot be derived from the structural meet of two sets. A meet
//!   impl would silently produce wrong banner roll-up.
//! - [`JointSet`] uses the `Mixed` / `DisunityCollapse` variants
//!   to make `join` associative under the absorbing JOINT+non-JOINT
//!   state per §H.3 p57. Meet would
//!   need to define "lowest JOINT state," which §H.3 does not
//!   prescribe.
//! - [`DisplayOnlyBlock`] is a structural union accumulator;
//!   intersecting display-only audiences across portions has no
//!   policy basis in §H.8.
//!
//! The [`assert_not_impl_any!`] blocks below lock the Join-only
//! shape so an accidental `MeetSemilattice` addition is a build
//! error.
//!
//! ## Authority
//!
//! - Lattice-split addendum (issue #456 / PR #502).
//! - The observational-state-lattice audit (memory
//!   `project_pr538_observational_lattice_audit`).
//! - Per-row §-citations live in the lattice type's own doc-comment in
//!   `crates/capco/src/lattice/` (per-type submodule).

use marque_capco::lattice::{
    AeaSet, ClassificationLattice, DeclassifyOnLattice, DisplayOnlyBlock, DissemSet, FgiSet,
    JointSet, NatoClassLattice, NatoDissemSet, RelToBlock, SarSet, SciSet,
};
use marque_scheme::{
    BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice,
};
use static_assertions::{assert_impl_all, assert_not_impl_any};

// --- Types implementing both halves of the Lattice ---

assert_impl_all!(SciSet: JoinSemilattice, MeetSemilattice);
assert_impl_all!(SarSet: JoinSemilattice, MeetSemilattice);
assert_impl_all!(FgiSet: JoinSemilattice, MeetSemilattice);
assert_impl_all!(AeaSet: JoinSemilattice, MeetSemilattice);
assert_impl_all!(NatoDissemSet: JoinSemilattice, MeetSemilattice);
assert_impl_all!(RelToBlock: JoinSemilattice, MeetSemilattice);
assert_impl_all!(DeclassifyOnLattice: JoinSemilattice, MeetSemilattice);

// --- Bounded types ---
//
// `ClassificationLattice` and `NatoClassLattice` are bounded on both
// halves: the five-level classification chain (Unclassified ≤
// Restricted ≤ Confidential ≤ Secret ≤ TopSecret) has lawful top and
// bottom elements.
//
// `DeclassifyOnLattice` is bounded on the JOIN side only. Its bottom is
// absence of any declassification instruction (the join identity), so
// it implements `BoundedJoinSemilattice` (PR-D1 / T043). The §E.3 chain
// has a genuine maximum element — `DeclassInstruction::NaSeeSourceList`,
// which "takes precedence over all other declassification
// instructions" (CAPCO-2016 §E.3 p32 line 663) — but that maximum is a
// property of the `DeclassInstruction` `Ord` (tier rank), NOT a
// `meet`-identity, so the type is deliberately NOT
// `BoundedMeetSemilattice` (the dates/exemptions below the top form an
// open space with no lawful finite meet-top). The remaining lattice
// types (SCI / SAR / FGI / dissem) are unbounded on both halves —
// open-vocab axes with no lawful finite top.

assert_impl_all!(
    ClassificationLattice:
    JoinSemilattice,
    MeetSemilattice,
    BoundedJoinSemilattice,
    BoundedMeetSemilattice
);
assert_impl_all!(
    NatoClassLattice:
    JoinSemilattice,
    MeetSemilattice,
    BoundedJoinSemilattice,
    BoundedMeetSemilattice
);
assert_impl_all!(DeclassifyOnLattice: BoundedJoinSemilattice);
assert_not_impl_any!(DeclassifyOnLattice: BoundedMeetSemilattice);

// Negative locks for the 9 fully-unbounded types (both halves).
// `DeclassifyOnLattice` is locked separately above — it IS
// `BoundedJoinSemilattice` (bottom = absence) but NOT
// `BoundedMeetSemilattice`. Without these locks, accidentally adding
// `BoundedJoinSemilattice` (e.g., declaring a `top()` value for
// `SciSet` that doesn't actually bound the open-vocabulary axis) would
// compile cleanly and silently weaken the "open-vocab has no lawful
// finite top" invariant that grounds the
// observational-state-vs-bounded distinction. The positive
// `assert_impl_all!` block above is asymmetric with the Join-only
// negative locks below; this block closes the symmetry.

assert_not_impl_any!(SciSet: BoundedJoinSemilattice, BoundedMeetSemilattice);
assert_not_impl_any!(SarSet: BoundedJoinSemilattice, BoundedMeetSemilattice);
assert_not_impl_any!(FgiSet: BoundedJoinSemilattice, BoundedMeetSemilattice);
assert_not_impl_any!(AeaSet: BoundedJoinSemilattice, BoundedMeetSemilattice);
assert_not_impl_any!(NatoDissemSet: BoundedJoinSemilattice, BoundedMeetSemilattice);
assert_not_impl_any!(RelToBlock: BoundedJoinSemilattice, BoundedMeetSemilattice);
assert_not_impl_any!(DissemSet: BoundedJoinSemilattice, BoundedMeetSemilattice);
assert_not_impl_any!(JointSet: BoundedJoinSemilattice, BoundedMeetSemilattice);
assert_not_impl_any!(DisplayOnlyBlock: BoundedJoinSemilattice, BoundedMeetSemilattice);

// --- Join-only observational-state types (issue #456 / PR #502 split + PR #538 audit) ---

assert_impl_all!(DissemSet: JoinSemilattice);
assert_not_impl_any!(DissemSet: MeetSemilattice);

assert_impl_all!(JointSet: JoinSemilattice);
assert_not_impl_any!(JointSet: MeetSemilattice);

assert_impl_all!(DisplayOnlyBlock: JoinSemilattice);
assert_not_impl_any!(DisplayOnlyBlock: MeetSemilattice);
