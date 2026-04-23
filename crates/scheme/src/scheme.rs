// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! The `MarkingScheme` trait.
//!
//! A scheme bundles the data that defines a marking system (categories,
//! constraints, templates) with the parse/validate/project/render
//! operations the engine invokes. See the crate-level docs and the
//! design document `docs/plans/2026-04-17-marking-scheme-lattice-
//! design.md` in the workspace root for the conceptual framing.

use crate::ambiguity::Parsed;
use crate::category::Category;
use crate::constraint::{Constraint, ConstraintViolation};
use crate::lattice::Lattice;
use crate::template::Template;

/// A structured marking scheme — CAPCO, CUI, NATO, or a custom
/// corporate/medical scheme.
///
/// Implementors bundle the scheme's data (`categories`, `constraints`,
/// `templates`) with operations the engine invokes. The data-heavy
/// methods are `&self` getters so adapters can return references into
/// `static` tables; the behavioral methods take the concrete
/// `Marking` type.
pub trait MarkingScheme {
    /// The scheme's token type. Kept associated (not parameterized) so
    /// schemes can use their own enum without leaking generics into the
    /// engine's call sites.
    type Token;

    /// The scheme's full-marking type. Must be a lattice: the product
    /// over the scheme's categories.
    type Marking: Lattice;

    /// Parse-level errors produced by `parse`.
    type ParseError;

    /// Human-readable name, e.g., "CAPCO-ISM-v2022-DEC".
    fn name(&self) -> &str;

    /// Schema/version identifier used for cache invalidation and audit
    /// logs.
    fn schema_version(&self) -> &str;

    /// All categories in the scheme, in arbitrary order. Sort by
    /// `ordering_rank` for render order.
    fn categories(&self) -> &[Category];

    /// Declarative invariants checked by `validate`.
    fn constraints(&self) -> &[Constraint];

    /// Structural templates (portion, banner, CAB, ...).
    fn templates(&self) -> &[Template];

    /// Parse an input string into a structured marking.
    ///
    /// Returns `Parsed::Unambiguous(m)` for the normal deterministic
    /// case; returns `Parsed::Ambiguous` only at enumerated decision
    /// points (e.g., the CAPCO `(C)` copyright-vs-CONFIDENTIAL case).
    fn parse(&self, input: &str) -> Result<Parsed<Self::Marking>, Self::ParseError>;

    /// Check all declarative constraints against `m`. Returns one
    /// violation per failing predicate.
    fn validate(&self, m: &Self::Marking) -> Vec<ConstraintViolation>;

    /// Project a set of portion markings into a banner marking. This
    /// is the lossy compression: per-category aggregation (max, union,
    /// intersect, supersession, ...) applied component-wise.
    fn project_banner(&self, portions: &[Self::Marking]) -> Self::Marking;

    /// Render a marking in portion form (abbreviated).
    fn render_portion(&self, m: &Self::Marking) -> String;

    /// Render a marking in banner form (expanded).
    fn render_banner(&self, m: &Self::Marking) -> String;
}
