// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Lattice primitives.
//!
//! A **lattice** is a partially ordered set in which every pair of elements
//! has a unique least upper bound (*join*, `⊔`) and a unique greatest lower
//! bound (*meet*, `⊓`). "Bounded" means there is a top (⊤) and bottom (⊥)
//! element. The full contract requires:
//!
//! - `join` and `meet` are commutative, associative, and idempotent.
//! - Absorption in both directions: `a ⊔ (a ⊓ b) = a` and `a ⊓ (a ⊔ b) = a`.
//! - `top ⊔ a = top`, `bottom ⊔ a = a` (and duals for `meet`).
//!
//! See Denning, *A Lattice Model of Secure Information Flow* (1976).
//!
//! # Join-semilattices: when meet does not apply
//!
//! Some types satisfy only the join half of the lattice laws. Specifically,
//! types that carry **join-side aggregation state** — a flag or variant that
//! records observed page composition across a sequence of `join` calls —
//! have no natural `meet` reading for that state, and therefore cannot
//! satisfy the dual absorption law `a ⊓ (a ⊔ b) = a` over the full type.
//!
//! Three in-tree examples:
//!
//! - `DissemSet` carries `relido_observed_unanimous: bool`, a join-side
//!   observation of RELIDO unanimity. Meet has no natural semantics for this
//!   flag (see `docs/plans/2026-05-01-lattice-design.md` section 4.6).
//! - `JointSet` carries `Mixed` / `DisunityCollapse` variants that record
//!   observed producer-list disagreement. Meet has no natural semantics for
//!   these variants beyond `Bottom` (see section 4.9 in that plan).
//! - `SupersessionSet` applies a post-join supersession overlay whose
//!   post-filter makes the meet direction non-idempotent on inputs that
//!   contain both a dominated token and its dominator.
//!
//! These types implement [`JoinSemilattice`] only. The trait split was
//! introduced in issue #456 (PR #502); see
//! `docs/plans/2026-05-01-lattice-design.md` section 4.10 for the
//! algebraic rationale.
//!
//! # Trait hierarchy
//!
//! ```text
//! JoinSemilattice ──────────────────────────────┐
//!                                               ├── Lattice (blanket impl)
//! MeetSemilattice ──────────────────────────────┘
//!
//! BoundedJoinSemilattice : JoinSemilattice
//! BoundedMeetSemilattice : MeetSemilattice
//!
//! BoundedJoinSemilattice + BoundedMeetSemilattice ──── BoundedLattice (blanket impl)
//! ```

/// A join-semilattice: a type with a `join` (least upper bound) operation.
///
/// Implementors must satisfy the three join laws: commutativity, associativity,
/// and idempotency. A type that also satisfies the meet laws and both absorption
/// identities should implement [`MeetSemilattice`] too; the blanket impl
/// promotes the pair to [`Lattice`] automatically.
pub trait JoinSemilattice: Sized + Clone + Eq {
    /// Least upper bound: the smallest element that dominates both inputs.
    fn join(&self, other: &Self) -> Self;
}

/// A meet-semilattice: a type with a `meet` (greatest lower bound) operation.
///
/// Implementors must satisfy the three meet laws: commutativity, associativity,
/// and idempotency. A type that also satisfies the join laws and both absorption
/// identities should implement [`JoinSemilattice`] too; the blanket impl
/// promotes the pair to [`Lattice`] automatically.
pub trait MeetSemilattice: Sized + Clone + Eq {
    /// Greatest lower bound: the largest element dominated by both inputs.
    fn meet(&self, other: &Self) -> Self;
}

/// A lattice: a type with both `join` (least upper bound) and `meet`
/// (greatest lower bound), satisfying all eight lattice laws including
/// absorption in both directions.
///
/// Implemented automatically for any type that satisfies both
/// [`JoinSemilattice`] and [`MeetSemilattice`] — no manual `impl Lattice`
/// is needed.
///
/// # The blanket impl rejects join-only types
///
/// A type that implements only [`JoinSemilattice`] does not satisfy
/// `Lattice`; the type system rejects it at any call site that requires
/// the full trait. The following example does not compile:
///
/// ```compile_fail
/// use marque_scheme::lattice::{JoinSemilattice, Lattice};
///
/// #[derive(Clone, PartialEq, Eq)]
/// struct JoinOnly(u32);
///
/// impl JoinSemilattice for JoinOnly {
///     fn join(&self, other: &Self) -> Self { JoinOnly(self.0.max(other.0)) }
/// }
///
/// fn requires_full_lattice<T: Lattice>() {}
/// requires_full_lattice::<JoinOnly>();
/// // ERROR: the trait bound `JoinOnly: MeetSemilattice` is not satisfied
/// ```
pub trait Lattice: JoinSemilattice + MeetSemilattice {}

/// Blanket impl: any type satisfying both halves is automatically a full lattice.
impl<T: JoinSemilattice + MeetSemilattice> Lattice for T {}

/// A join-semilattice with an explicit bottom (⊥) element.
///
/// `bottom` must be the identity for `join`: `bottom ⊔ a = a` for every `a`.
pub trait BoundedJoinSemilattice: JoinSemilattice {
    /// The bottom (⊥) element: is dominated by every other element under join.
    fn bottom() -> Self;
}

/// A meet-semilattice with an explicit top (⊤) element.
///
/// `top` must be the identity for `meet`: `top ⊓ a = a` for every `a`.
pub trait BoundedMeetSemilattice: MeetSemilattice {
    /// The top (⊤) element: dominates every other element under meet.
    fn top() -> Self;
}

/// A bounded lattice: top and bottom elements with a full lattice.
///
/// Implemented automatically for any type that satisfies [`Lattice`],
/// [`BoundedJoinSemilattice`], and [`BoundedMeetSemilattice`] — no manual
/// `impl BoundedLattice` is needed.
pub trait BoundedLattice: Lattice + BoundedJoinSemilattice + BoundedMeetSemilattice {}

/// Blanket impl: any doubly-bounded lattice is automatically `BoundedLattice`.
impl<T: Lattice + BoundedJoinSemilattice + BoundedMeetSemilattice> BoundedLattice for T {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    /// A tiny four-element classification lattice, modeling the US
    /// classification ladder `U < C < S < TS`. Used throughout this
    /// module to verify lattice laws without pulling in `marque-ism`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum Level {
        U,
        C,
        S,
        TS,
    }

    impl JoinSemilattice for Level {
        fn join(&self, other: &Self) -> Self {
            *self.max(other)
        }
    }

    impl MeetSemilattice for Level {
        fn meet(&self, other: &Self) -> Self {
            *self.min(other)
        }
    }

    // `Level` gets `Lattice` automatically via the blanket impl.

    impl BoundedJoinSemilattice for Level {
        fn bottom() -> Self {
            Self::U
        }
    }

    impl BoundedMeetSemilattice for Level {
        fn top() -> Self {
            Self::TS
        }
    }

    // `Level` gets `BoundedLattice` automatically via the blanket impl.

    const ALL: [Level; 4] = [Level::U, Level::C, Level::S, Level::TS];

    // -------------------------------------------------------------------------
    // JoinSemilattice laws
    // -------------------------------------------------------------------------

    #[test]
    fn join_is_commutative() {
        for a in ALL {
            for b in ALL {
                assert_eq!(a.join(&b), b.join(&a));
            }
        }
    }

    #[test]
    fn join_is_associative() {
        for a in ALL {
            for b in ALL {
                for c in ALL {
                    assert_eq!(a.join(&b).join(&c), a.join(&b.join(&c)));
                }
            }
        }
    }

    #[test]
    fn join_is_idempotent() {
        for a in ALL {
            assert_eq!(a.join(&a), a);
        }
    }

    // -------------------------------------------------------------------------
    // MeetSemilattice laws
    // -------------------------------------------------------------------------

    #[test]
    fn meet_is_commutative() {
        for a in ALL {
            for b in ALL {
                assert_eq!(a.meet(&b), b.meet(&a));
            }
        }
    }

    #[test]
    fn meet_is_associative() {
        for a in ALL {
            for b in ALL {
                for c in ALL {
                    assert_eq!(a.meet(&b).meet(&c), a.meet(&b.meet(&c)));
                }
            }
        }
    }

    #[test]
    fn meet_is_idempotent() {
        for a in ALL {
            assert_eq!(a.meet(&a), a);
        }
    }

    // -------------------------------------------------------------------------
    // Full Lattice — absorption laws
    // -------------------------------------------------------------------------

    #[test]
    fn absorption_join_over_meet() {
        for a in ALL {
            for b in ALL {
                assert_eq!(a.join(&a.meet(&b)), a);
            }
        }
    }

    #[test]
    fn absorption_meet_over_join() {
        for a in ALL {
            for b in ALL {
                assert_eq!(a.meet(&a.join(&b)), a);
            }
        }
    }

    // -------------------------------------------------------------------------
    // BoundedLattice identities
    // -------------------------------------------------------------------------

    #[test]
    fn bounded_identities() {
        for a in ALL {
            assert_eq!(Level::top().join(&a), Level::top());
            assert_eq!(Level::bottom().join(&a), a);
            assert_eq!(Level::top().meet(&a), a);
            assert_eq!(Level::bottom().meet(&a), Level::bottom());
        }
    }

    #[test]
    fn classification_ladder_join_picks_higher() {
        assert_eq!(Level::U.join(&Level::TS), Level::TS);
        assert_eq!(Level::C.join(&Level::S), Level::S);
    }

    // -------------------------------------------------------------------------
    // Blanket-impl gate: Level satisfies Lattice and BoundedLattice
    // automatically via the blanket impls.
    // -------------------------------------------------------------------------

    #[test]
    fn blanket_impl_promotes_level_to_lattice() {
        fn _assert_lattice<T: Lattice>() {}
        fn _assert_bounded<T: BoundedLattice>() {}
        _assert_lattice::<Level>();
        _assert_bounded::<Level>();
    }

    // -------------------------------------------------------------------------
    // Join-only type: confirm JoinSemilattice can exist without MeetSemilattice
    // -------------------------------------------------------------------------

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct JoinOnly(u32);

    impl JoinSemilattice for JoinOnly {
        fn join(&self, other: &Self) -> Self {
            Self(self.0.max(other.0))
        }
    }

    #[test]
    fn join_only_type_satisfies_join_semilattice() {
        fn _assert_join<T: JoinSemilattice>() {}
        _assert_join::<JoinOnly>();
        let a = JoinOnly(3);
        let b = JoinOnly(7);
        assert_eq!(a.join(&b), JoinOnly(7));
        assert_eq!(a.join(&a), a);
    }
}
