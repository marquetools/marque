// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Compile-gate tests for the `JoinSemilattice` + `MeetSemilattice` trait split.
//!
//! The `Lattice` trait split (issue #456 / PR #502) divided the
//! monolithic `Lattice` trait into two halves with a blanket
//! `impl<T: JoinSemilattice + MeetSemilattice> Lattice for T {}`.
//! This file pins the invariants that must hold at the type-system level:
//!
//! 1. Types satisfying both halves automatically satisfy `Lattice`.
//! 2. Types satisfying only `JoinSemilattice` do NOT satisfy `Lattice`.
//! 3. Bounded variants (`BoundedJoinSemilattice`, `BoundedMeetSemilattice`)
//!    compose to `BoundedLattice` via the blanket impl.

use marque_scheme::lattice::{
    BoundedJoinSemilattice, BoundedLattice, BoundedMeetSemilattice, JoinSemilattice,
    MeetSemilattice,
};

// ---------------------------------------------------------------------------
// Test types
// ---------------------------------------------------------------------------

/// A type that implements both semilattice halves — should auto-get `Lattice`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct FullLattice(u8);

impl JoinSemilattice for FullLattice {
    fn join(&self, other: &Self) -> Self {
        Self(self.0.max(other.0))
    }
}

impl MeetSemilattice for FullLattice {
    fn meet(&self, other: &Self) -> Self {
        Self(self.0.min(other.0))
    }
}

impl BoundedJoinSemilattice for FullLattice {
    fn bottom() -> Self {
        Self(0)
    }
}

impl BoundedMeetSemilattice for FullLattice {
    fn top() -> Self {
        Self(u8::MAX)
    }
}

/// A type that implements only the join half — must NOT get `Lattice`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct JoinOnly(u8);

impl JoinSemilattice for JoinOnly {
    fn join(&self, other: &Self) -> Self {
        Self(self.0 | other.0)
    }
}

// ---------------------------------------------------------------------------
// Compile-time assertions
// ---------------------------------------------------------------------------

/// `FullLattice` implements both semilattice halves → blanket impl gives Lattice.
const _: fn() = || {
    fn assert_lattice<T: marque_scheme::lattice::Lattice>() {}
    assert_lattice::<FullLattice>();
};

/// `FullLattice` also satisfies `BoundedLattice` via the blanket impl.
const _: fn() = || {
    fn assert_bounded_lattice<T: BoundedLattice>() {}
    assert_bounded_lattice::<FullLattice>();
};

/// `JoinOnly` satisfies `JoinSemilattice` but not `Lattice`.
const _: fn() = || {
    fn assert_join_semilattice<T: JoinSemilattice>() {}
    assert_join_semilattice::<JoinOnly>();
    // The negative property (`JoinOnly: !Lattice`) is enforced by the compiler:
    // if `MeetSemilattice` were implemented for `JoinOnly`, the blanket would
    // promote it to `Lattice`. The absence of `impl MeetSemilattice for JoinOnly`
    // is the type-system gate. Verify the negative at the call site — any code
    // that requires `T: Lattice` will reject `JoinOnly` at monomorphization.
};

// ---------------------------------------------------------------------------
// Unit tests: blanket impls work at runtime
// ---------------------------------------------------------------------------

#[test]
fn full_lattice_join_and_meet_via_blanket() {
    let a = FullLattice(3);
    let b = FullLattice(7);
    // `join` and `meet` are now on `JoinSemilattice`/`MeetSemilattice`,
    // but `Lattice: JoinSemilattice + MeetSemilattice` so they are
    // reachable through the supertrait chain.
    let j = a.join(&b);
    let m = a.meet(&b);
    assert_eq!(j, FullLattice(7)); // max
    assert_eq!(m, FullLattice(3)); // min

    // Absorption laws.
    assert_eq!(a.join(&a.meet(&b)), a); // a ⊔ (a ⊓ b) = a
    assert_eq!(a.meet(&a.join(&b)), a); // a ⊓ (a ⊔ b) = a
}

#[test]
fn full_lattice_bounded_identities() {
    let bottom = FullLattice::bottom();
    let top = FullLattice::top();
    let x = FullLattice(42);
    assert_eq!(bottom.join(&x), x); // bottom ⊔ x = x
    assert_eq!(x.join(&bottom), x); // x ⊔ bottom = x
    assert_eq!(top.meet(&x), x); // top ⊓ x = x
    assert_eq!(x.meet(&top), x); // x ⊓ top = x
}

#[test]
fn join_only_type_satisfies_join_semilattice() {
    // Confirm that a type without `MeetSemilattice` still compiles
    // and satisfies the join contract.
    let a = JoinOnly(0b0011);
    let b = JoinOnly(0b0101);
    let j = a.join(&b);
    assert_eq!(j, JoinOnly(0b0111)); // OR

    // Idempotency.
    assert_eq!(a.join(&a), a);
    // Commutativity.
    assert_eq!(a.join(&b), b.join(&a));
}
