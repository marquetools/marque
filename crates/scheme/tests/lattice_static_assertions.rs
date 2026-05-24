// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Compile-time lattice trait-impl pin for `marque-scheme` built-in
//! constructors (refactor 006 follow-up T146, issue #665).
//!
//! Locks the [`SupersessionSet`] trait-impl shape — [`JoinSemilattice`]
//! only, no [`MeetSemilattice`], no `Bounded*` halves — at build time.
//! Catches the drift class where a trait impl is silently added or
//! removed on a built-in lattice constructor, before any test binary
//! runs. Complements the `marque-capco` pin at
//! `crates/capco/tests/lattice_static_assertions.rs`, which covers
//! the per-axis domain lattice types built on top of these
//! constructors.
//!
//! ## Why join-only
//!
//! The `Lattice` trait split — issue #456 / implementing PR #502 —
//! separated [`JoinSemilattice`] from [`MeetSemilattice`] precisely
//! because [`SupersessionSet`] cannot satisfy the dual absorption law
//! `a ⊓ (a ⊔ b) = a`. Its post-join supersession overlay drops
//! dominated tokens, so under the natural set-intersection candidate
//! for `meet`, `{R} ⊓ ({R} ⊔ {N}) = {R} ⊓ {N} = {} ≠ {R}` whenever
//! the supersession table contains `(N → R)`. The counterexample is
//! hypothetical — no `meet` is defined today — and any other
//! plausible `meet` candidate (e.g., supersession-aware
//! intersection) also fails dual absorption. The type-level
//! discussion lives in the [`SupersessionSet`] doc-comment in
//! `crates/scheme/src/builtins.rs`.
//!
//! PR #538 then audited the remaining `JoinSemilattice` claims on
//! the three join-only types (`DissemSet`, `JointSet`,
//! `SupersessionSet`) and confirmed the join half holds —
//! associativity, commutativity, idempotence, identity-with-bottom
//! — for all three. Adding `MeetSemilattice` later would silently
//! re-introduce the dual-absorption failure PR #502 lifted into the
//! type system, so this pin keeps the build failing at the first
//! sign of regression.
//!
//! ## Why only `SupersessionSet`
//!
//! The other eight constructors in `marque-scheme::builtins` at the
//! time of this pin are either full lattices by construction
//! (`OrdMax`, `OrdMin`, `FlatSet`, `IntersectSet`, `ModeSet`,
//! `MaxDate`) or conditional wrappers that transparently inherit
//! the inner type's lattice status (`OptionalSingleton<L>`,
//! `Product<A, B>`). None of them sits in the "join-only-by-audit"
//! category that motivated the trait split, so locking their shape
//! is not required for the #665 invariant. A future constructor
//! added to `builtins` would need its own audit; broadening the
//! positive coverage of this pin to enforce that is tracked as a
//! follow-up rather than handled here.

use marque_scheme::{
    BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice,
    SupersessionSet,
};
use static_assertions::{assert_impl_all, assert_not_impl_any};

// --- Positive lock: `SupersessionSet` implements `JoinSemilattice` ---
//
// `u8` matches the in-tree precedent in the
// `supersession_is_join_semilattice_only` unit test in
// `crates/scheme/src/builtins.rs`, which instantiates
// `SupersessionSet<u8>` to exercise the trait-impl shape. The choice
// of element type is incidental; the pin locks the impl, not the
// element semantics.

assert_impl_all!(SupersessionSet<u8>: JoinSemilattice);

// --- Negative lock: no `MeetSemilattice` (the load-bearing claim) ---
//
// Issue #456 / PR #502 + PR #538 establish that `SupersessionSet`
// cannot lawfully implement `MeetSemilattice`. An accidental
// `impl MeetSemilattice for SupersessionSet<T>` would compile
// cleanly without this assertion and silently re-introduce the
// dual-absorption failure that motivated the trait split.

assert_not_impl_any!(SupersessionSet<u8>: MeetSemilattice);

// --- Negative lock: no bounded halves (symmetry safeguard) ---
//
// Per the Copilot R2 finding addressed in PR #557 and applied in
// the post-PR-557 negative-lock block in
// `crates/capco/tests/lattice_static_assertions.rs`,
// `SupersessionSet` has no lawful finite top: the universe of `T`
// is open-vocabulary by design (see the type-level doc on
// [`SupersessionSet`] in `crates/scheme/src/builtins.rs`). Lock the
// unbounded shape so a future sentinel `top()`/`bottom()` addition
// fails the build instead of silently weakening the invariant.

assert_not_impl_any!(SupersessionSet<u8>: BoundedJoinSemilattice, BoundedMeetSemilattice);
