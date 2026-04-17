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
