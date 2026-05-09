// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! # marque-scheme
//!
//! Domain-neutral trait surface for structured marking schemes.
//!
//! A marking system is a **typed algebra over a bounded lattice**,
//! with a constraint predicate and a lossy projection operator, plus
//! local probabilistic disambiguation at a small number of decision
//! points. Concrete schemes (CAPCO, CUI, NATO, ...) implement
//! [`MarkingScheme`] against their own marking type; the engine
//! operates on the trait.
//!
//! See `docs/plans/2026-04-17-marking-scheme-lattice-design.md` in
//! the workspace root for the consolidated design document.
//!
//! ## Module layout
//!
//! - [`lattice`] — `Lattice`, `BoundedLattice` traits.
//! - [`category`] — `Category`, `AggregationOp`, `Cardinality`,
//!   `IntraOrdering`, and generic reducers keyed by `AggregationOp`.
//! - [`constraint`] — declarative `Constraint` invariants.
//! - [`template`] — structural templates for portion / banner / CAB.
//! - [`projection`] — `Projection` trait and render-order helpers.
//! - [`ambiguity`] — `Parsed<M>`, `Candidate`, `EvidenceFeature`.
//! - [`scheme`] — the `MarkingScheme` trait.
//! - [`page_rewrite`] — declarative `PageRewrite`, `CategoryAction`,
//!   `CategoryPredicate` (Phase B).
//! - [`scope`] — `Scope` enum for projection contexts (Phase B).
//! - [`builtins`] — built-in lattice constructors `OrdMax`, `OrdMin`,
//!   `FlatSet`, `IntersectSet`, `SupersessionSet`, `ModeSet`,
//!   `MaxDate`, `OptionalSingleton`, `Product` (Phase B).
//! - [`recognizer`] — `Recognizer<S>` trait + `ParseContext`
//!   (Phase D / decoder dispatch).
//! - [`vocabulary`] — `Vocabulary<S>` trait + `TokenMetadataFull`,
//!   `Authority`, `OwnerProducer`, `PointOfContact`, `Deprecation`
//!   (Phase E).
//! - [`codec`] — `Codec<S>` trait + `CodecError` surface — pinned
//!   for Phase G to implement against (Phase E).
//!
//! ## Status
//!
//! Phase E trait surface is complete. The lattice, projection,
//! recognizer, vocabulary, and codec surfaces are pinned and
//! consumed by `marque-capco` (the in-tree adapter). A second
//! scheme can land in its own crate without touching this one —
//! see `crates/scheme/tests/adoption_readiness.rs` for the
//! SC-010 pre-verification (`StubScheme` builds against
//! `marque_scheme::*` and `std::*` only).

pub mod ambiguity;
pub mod builtins;
pub mod canonical;
pub mod category;
pub mod codec;
pub mod constraint;
pub mod lattice;
pub mod page_rewrite;
pub mod projection;
pub mod recognizer;
pub mod scheme;
pub mod scope;
pub mod template;
pub mod vocabulary;

pub use ambiguity::{Candidate, EvidenceFeature, Parsed};
pub use canonical::{Canonical, CanonicalConstructor, EngineConstructor, TokenSource};
pub use builtins::{
    FlatSet, IntersectSet, MaxDate, ModeSet, OptionalSingleton, OrdMax, OrdMin, Product,
    SupersessionSet,
};
pub use category::{
    AggregationOp, Cardinality, Category, CategoryId, CategoryShape, ExpansionFn, IntraOrdering,
    TokenId, reduce_intersect, reduce_max, reduce_union, reduce_union_with_supersession,
};
pub use codec::{Codec, CodecError};
pub use constraint::{Constraint, ConstraintViolation, TokenRef};
pub use lattice::{BoundedLattice, Lattice};
pub use page_rewrite::{
    CategoryAction, CategoryPredicate, PageRewrite, PageRewriteAxisError, RewriteId,
};
pub use projection::{Projection, categories_in_render_order};
pub use recognizer::{DocumentPosition, ParseContext, Recognizer, Zone};
pub use scheme::MarkingScheme;
pub use scope::{DiffInput, DiffRelation, Scope};
pub use template::{CategoryRule, Presence, Template, TokenForm, Wrapping};
pub use vocabulary::{
    Authority, Deprecation, OwnerProducer, OwnerProducerKind, PointOfContact, TokenMetadataFull,
    Vocabulary,
};
