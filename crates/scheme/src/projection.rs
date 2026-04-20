// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Portion → banner projection.
//!
//! A banner is a **lossy compression** of the portions: max on
//! classification, union on controls, intersection on REL TO, max-date
//! on declassify-on. Different schemes may choose different operators
//! per category (a medical scheme that unions on everything; a
//! corporate scheme that picks the mode). The projection is explicit
//! and swappable — it's the design move that makes "marking = encoding"
//! operational without committing to specific machinery.
//!
//! This module is intentionally thin: each `MarkingScheme`
//! implementation provides its own `project_banner` that knows how to
//! read the concrete `Marking` type's fields. The `Projection` trait
//! here is a documentation contract and a future extension point for
//! pluggable projections.

use crate::category::Category;

/// A projection from a set of portion markings to a banner marking.
///
/// Kept generic over `M` so a scheme can express its projection as a
/// stand-alone type (useful for tests) without needing the full
/// `MarkingScheme` context.
pub trait Projection<M> {
    fn project(&self, portions: &[M]) -> M;
}

/// Helper: categories sorted by `ordering_rank`, returning a new Vec of
/// references. Used by the default banner renderer to emit categories
/// in canonical left-to-right order.
pub fn categories_in_render_order(categories: &[Category]) -> Vec<&Category> {
    let mut sorted: Vec<&Category> = categories.iter().collect();
    sorted.sort_by_key(|c| c.ordering_rank);
    sorted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::category::{AggregationOp, Cardinality, CategoryId, IntraOrdering};

    fn cat(id: u32, rank: u16) -> Category {
        Category {
            id: CategoryId(id),
            name: "c",
            ordering_rank: rank,
            cardinality: Cardinality::One,
            aggregation: AggregationOp::Max,
            intra_ordering: IntraOrdering::AsWritten,
            expansion: None,
        }
    }

    #[test]
    fn categories_in_render_order_sorts_by_rank() {
        let cats = vec![cat(1, 10), cat(2, 5), cat(3, 7)];
        let sorted = categories_in_render_order(&cats);
        let ranks: Vec<u16> = sorted.iter().map(|c| c.ordering_rank).collect();
        assert_eq!(ranks, vec![5, 7, 10]);
    }

    #[test]
    fn categories_in_render_order_empty_returns_empty() {
        let cats: Vec<Category> = vec![];
        assert!(categories_in_render_order(&cats).is_empty());
    }

    #[test]
    fn categories_in_render_order_single_returns_same() {
        let cats = vec![cat(1, 0)];
        let sorted = categories_in_render_order(&cats);
        assert_eq!(sorted.len(), 1);
        assert_eq!(sorted[0].id, CategoryId(1));
    }

    // Dummy `Projection` impl to exercise the trait surface.
    struct MaxProjection;

    impl Projection<u32> for MaxProjection {
        fn project(&self, portions: &[u32]) -> u32 {
            portions.iter().copied().max().unwrap_or(0)
        }
    }

    #[test]
    fn projection_trait_can_be_implemented() {
        let p = MaxProjection;
        assert_eq!(p.project(&[1, 5, 3]), 5);
        assert_eq!(p.project(&[]), 0);
    }
}
