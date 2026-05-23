// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Built-in lattice constructors.
//!
//! A small family of generic lattice types that cover the bulk of
//! category shapes across CAPCO, CUI, and NATO. A scheme author
//! picks a constructor appropriate to their category rather than writing
//! `impl JoinSemilattice` / `impl MeetSemilattice` by hand every time.
//!
//! | Constructor            | Shape                                      | CAPCO example           | Laws                |
//! |------------------------|--------------------------------------------|-------------------------|---------------------|
//! | [`OrdMax`]             | total order, join = `max`                  | classification ladder   | Full lattice        |
//! | [`OrdMin`]             | total order, join = `min`                  | "most specific" picks   | Full lattice        |
//! | [`FlatSet`]            | powerset, join = union, meet = intersect   | SCI / SAR / dissem      | Full lattice        |
//! | [`IntersectSet`]       | inverted powerset, join = intersect        | REL TO (pre-expansion)  | Full lattice        |
//! | [`SupersessionSet`]    | union, then drop superseded tokens         | NOFORN ⊐ REL TO (intra) | **Join-only**       |
//! | [`ModeSet`]            | multiset, join = most-frequent             | corporate sensitivity   | Full lattice        |
//! | [`MaxDate`]            | dates, join = later, bottom = absent       | declassify-on           | Full lattice        |
//! | [`OptionalSingleton`]  | lifts any `JoinSemilattice` to `Option<L>` | optional single fields  | Mirrors inner type  |
//! | [`Product`]            | tuple product of two semilattices          | composed sub-lattices   | Mirrors inner types |
//!
//! `SupersessionSet` implements only [`JoinSemilattice`] — the supersession
//! overlay is a join-side post-filter and the meet direction is
//! non-idempotent on inputs that contain both a dominated token and its
//! dominator. See the type-level doc for the counterexample.
//!
//! `OptionalSingleton<L>` and `Product<A, B>` mirror their inner type(s):
//! if the inner type(s) are full lattices, the wrapper is a full lattice
//! (via the blanket impl); if the inner type(s) are join-only, the wrapper
//! is join-only.
//!
//! # Contract
//!
//! The usual lattice laws (commutative, associative, idempotent join
//! and meet; absorption) are verified by unit tests in this module on
//! small example instances. The property tests in `marque-capco` extend
//! the checks to the CAPCO structural lattices that consume these
//! primitives.

mod date;
mod optional;
mod ord;
mod product;
mod set;
mod supersession;

pub use date::MaxDate;
pub use optional::{ModeSet, OptionalSingleton};
pub use ord::{OrdMax, OrdMin};
pub use product::Product;
pub use set::{FlatSet, IntersectSet};
pub use supersession::SupersessionSet;

/// Merge two sorted, de-duplicated slices into a sorted union.
///
/// # Preconditions
///
/// `left` and `right` must each already be sorted ascending and free of
/// duplicates. If callers violate those invariants, the output ordering
/// and uniqueness guarantees no longer hold.
fn merge_sorted_union<T: Ord + Clone>(left: &[T], right: &[T]) -> Vec<T> {
    let mut out: Vec<T> = Vec::with_capacity(left.len() + right.len());
    let (mut i, mut j) = (0, 0);
    while i < left.len() && j < right.len() {
        match left[i].cmp(&right[j]) {
            std::cmp::Ordering::Less => {
                out.push(left[i].clone());
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                out.push(right[j].clone());
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                out.push(left[i].clone());
                i += 1;
                j += 1;
            }
        }
    }
    out.extend_from_slice(&left[i..]);
    out.extend_from_slice(&right[j..]);
    out
}
