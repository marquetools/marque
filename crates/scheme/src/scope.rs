// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Scope-parameterized projection.
//!
//! A `Scope` tells [`crate::MarkingScheme::project`] which reduction
//! semantic to apply:
//!
//! - `Portion` — identity on a single marking.
//! - `Page` — per-page banner roll-up (the CAPCO "expected banner").
//! - `Document` — document-level roll-up (identical to `Page` on
//!   single-page documents; may diverge for multi-page).
//! - `Diff` — marker variant; diff rules consume a [`DiffInput`] on a
//!   separate entry point. Kept here for enum completeness so code
//!   walking `Scope` variants sees the full surface.
//!
//! `DiffInput` is a dedicated input type (rather than a `Scope`
//! variant carrying references) so the 99% of scope values that never
//! carry a second marking don't have to thread lifetimes through their
//! call sites.

use crate::ambiguity::Parsed;

/// Where a projection is being evaluated.
///
/// The enum is `#[non_exhaustive]`-free because the variant set is
/// fixed by the design doc: scheme authors don't introduce new scopes.
/// If a future scheme needs a scope not represented here, the variant
/// is added to the enum via a minor-version bump — callers who match
/// exhaustively will see the addition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    /// Individual portion marking. Identity under projection.
    Portion,
    /// Page-level rollup (banner / CAB). Corresponds to CAPCO's
    /// per-page `expected_*` aggregate.
    Page,
    /// Document-level rollup. Usually agrees with `Page` on
    /// single-page documents.
    Document,
    /// Diff-rule context; the caller supplies a [`DiffInput`] rather
    /// than a slice of markings. See crate docs.
    Diff,
}

/// Input to a diff rule: two markings and the relation between them.
///
/// Kept separate from the `Scope` enum so that a `Scope` value never
/// needs to carry a lifetime for the 99% of call sites that don't care
/// about diffs. Callers (CLI batch mode, server diff endpoints, email-
/// thread walkers) construct the `DiffInput` explicitly; the engine
/// does not fetch second markings.
///
/// `DiffInput` carries a `from`/`to` pair of `Parsed<M>` values and a
/// [`DiffRelation`] tag. The type holds no lattice machinery itself —
/// its fields are inspected by diff rules, which compose per-axis
/// lattice operations on the inner `M::Marking` if they need to.
///
/// PR 4b-D.2 (2026-05-18) dropped the prior `M: JoinSemilattice` bound
/// in lock-step with the `MarkingScheme::Marking` bound relaxation
/// (D24). The bound was purely declarative — `DiffInput` itself never
/// called `.join` on `M` — and keeping it would force every consumer
/// to satisfy a trait the cross-axis fold cannot keep idempotently.
/// See `MarkingScheme::Marking` doc comment for the full rationale.
#[derive(Debug, Clone)]
pub struct DiffInput<M> {
    pub from: Parsed<M>,
    pub to: Parsed<M>,
    pub relation: DiffRelation,
}

/// The semantic relationship between the two markings in a
/// [`DiffInput`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffRelation {
    /// One document — banner compared against its portions.
    BannerOverPortions,
    /// Email thread — reply compared against parent.
    ReplyOverParent,
    /// CUI re-disclosure — derivative compared against original.
    DisclosureOverOriginal,
    /// Current marking compared against its historical equivalent.
    Historical,
    /// Scheme-specific relation identified by a stable label.
    Custom(&'static str),
}
