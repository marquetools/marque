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
use crate::constraint::{Constraint, ConstraintViolation, TokenRef};
use crate::lattice::Lattice;
use crate::page_rewrite::PageRewrite;
use crate::scope::Scope;
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

    /// Resolve a [`TokenRef`] against a concrete marking.
    ///
    /// The declarative constraint evaluator (see
    /// [`crate::constraint::evaluate`]) asks the scheme this question
    /// for every dyadic-variant predicate it needs to fire. Schemes
    /// map `TokenRef::Token(id)` to "the marking carries that token
    /// somewhere" and `TokenRef::AnyInCategory(cat)` to "any token in
    /// that category is present in the marking."
    ///
    /// The default implementation returns `false` so a scheme that
    /// does not declare dyadic constraints in Phase 3 is still
    /// well-formed — only the variants the scheme actually uses need
    /// coverage.
    fn satisfies(&self, _marking: &Self::Marking, _token_ref: &TokenRef) -> bool {
        false
    }

    /// Evaluate a [`Constraint::Custom`] by name. Returns one
    /// [`ConstraintViolation`] per failing check.
    ///
    /// Custom constraints are n-ary or context-dependent predicates
    /// that cannot be expressed as a pair of token references (SIGMA
    /// numeric-ordering, CNWDI's classification floor, HCS's
    /// sub-compartment rules). The scheme's evaluator owns the
    /// predicate body; [`crate::constraint::evaluate`] simply calls
    /// this method and pipes the results through.
    ///
    /// Default: no violations.
    fn evaluate_custom(
        &self,
        _name: &'static str,
        _marking: &Self::Marking,
    ) -> Vec<ConstraintViolation> {
        Vec::new()
    }

    /// Check all declarative constraints against `m`. Returns one
    /// violation per failing predicate.
    ///
    /// Default: delegates to [`crate::constraint::evaluate`] so
    /// schemes get the declarative-evaluator behavior automatically.
    /// Schemes override when they need to prepend / append
    /// scheme-specific non-constraint checks that live outside the
    /// declarative catalog (e.g., structural validations tied to
    /// token ordering within a category).
    fn validate(&self, m: &Self::Marking) -> Vec<ConstraintViolation> {
        crate::constraint::evaluate(self, m)
    }

    /// Project a set of markings into a single marking under the given
    /// scope.
    ///
    /// - `Scope::Portion` — identity; returns the first marking (or
    ///   the scheme's bottom if empty).
    /// - `Scope::Page` — per-page banner roll-up. This is the
    ///   operation CAPCO's `PageContext::expected_*` accessors
    ///   historically performed. Implementations should apply
    ///   component-wise category joins first, then run
    ///   [`Self::page_rewrites`] in declaration order.
    /// - `Scope::Document` — document-level roll-up. On single-page
    ///   documents this typically agrees with `Scope::Page`.
    /// - `Scope::Diff` — callers should use a dedicated diff entry
    ///   point carrying a [`crate::scope::DiffInput`]; this default
    ///   mirrors `Page` so a bare `project` call on diff scope is
    ///   still well-defined.
    fn project(&self, scope: Scope, markings: &[Self::Marking]) -> Self::Marking;

    /// Back-compat shim: project at page scope. Default implementation
    /// calls `project(Scope::Page, portions)`. Kept so existing callers
    /// (Phase A / Phase B tests, current CAPCO rules) don't churn.
    #[inline]
    fn project_banner(&self, portions: &[Self::Marking]) -> Self::Marking {
        self.project(Scope::Page, portions)
    }

    /// Cross-category rewrites applied after component-wise
    /// page-scope projection. CAPCO's canonical entry is
    /// NOFORN-clears-REL-TO — see §7a of the Phase B design doc.
    ///
    /// Default: no rewrites. Schemes override to declare their table.
    fn page_rewrites(&self) -> &[PageRewrite<Self>] {
        &[]
    }

    /// Render a marking in portion form (abbreviated).
    fn render_portion(&self, m: &Self::Marking) -> String;

    /// Render a marking in banner form (expanded).
    fn render_banner(&self, m: &Self::Marking) -> String;
}
