// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Lattice primitives.
//!
//! A lattice is a partially ordered set in which every pair of elements
//! has a unique least upper bound (*join*, `⊔`) and a unique greatest
//! lower bound (*meet*, `⊓`). "Bounded" means there's a top (⊤) and
//! bottom (⊥) element.
//!
//! # Contract for implementors
//!
//! Implementors are **expected** to satisfy the standard lattice laws
//! on their marking type. The crate cannot enforce this from the trait
//! definition alone; the expectation is documented here as the
//! contract every `Lattice` / `BoundedLattice` impl must uphold:
//!
//! - `join` / `meet` are commutative, associative, idempotent.
//! - `top ⊔ a = top`, `bottom ⊔ a = a` (symmetric for `meet`).
//! - Absorption: `a ⊔ (a ⊓ b) = a`.
//!
//! A scheme-specific adapter that applies lossy normalization during
//! `join` (for example, CAPCO's projection which strips FOUO in
//! classified banners) will not satisfy these laws on every input and
//! should either (a) keep normalization out of `join` and put it in a
//! separate `project_banner` method, or (b) document explicitly which
//! laws fail and on which inputs.
//!
//! See Denning, *A Lattice Model of Secure Information Flow* (1976).

/// A lattice: a type with `join` (least upper bound) and `meet`
/// (greatest lower bound).
///
/// Implementors must obey the standard lattice laws. The `tests`
/// module in this file verifies these on an example lattice.
pub trait Lattice: Sized + Clone + Eq {
    /// Least upper bound: the smallest element that dominates both inputs.
    fn join(&self, other: &Self) -> Self;

    /// Greatest lower bound: the largest element dominated by both inputs.
    fn meet(&self, other: &Self) -> Self;
}

/// A lattice with explicit top and bottom elements.
pub trait BoundedLattice: Lattice {
    /// The top (⊤) element: dominates every other element.
    fn top() -> Self;

    /// The bottom (⊥) element: is dominated by every other element.
    fn bottom() -> Self;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
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

    impl Lattice for Level {
        fn join(&self, other: &Self) -> Self {
            *self.max(other)
        }
        fn meet(&self, other: &Self) -> Self {
            *self.min(other)
        }
    }

    impl BoundedLattice for Level {
        fn top() -> Self {
            Self::TS
        }
        fn bottom() -> Self {
            Self::U
        }
    }

    const ALL: [Level; 4] = [Level::U, Level::C, Level::S, Level::TS];

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
    fn absorption() {
        for a in ALL {
            for b in ALL {
                assert_eq!(a.join(&a.meet(&b)), a);
                assert_eq!(a.meet(&a.join(&b)), a);
            }
        }
    }

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
}
