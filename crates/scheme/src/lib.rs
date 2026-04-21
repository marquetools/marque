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
//!
//! ## Status
//!
//! Phase A scaffolding. The trait and primitives are stable; concrete
//! adapters land as separate PRs (Phase B onwards). `marque-capco`
//! implements `MarkingScheme` as a proof of fit — see
//! `crates/capco/src/scheme.rs`.

pub mod ambiguity;
pub mod builtins;
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
pub use page_rewrite::{CategoryAction, CategoryPredicate, PageRewrite, RewriteId};
pub use projection::{Projection, categories_in_render_order};
pub use recognizer::{DocumentPosition, ParseContext, Recognizer, Zone};
pub use scheme::MarkingScheme;
pub use scope::{DiffInput, DiffRelation, Scope};
pub use template::{CategoryRule, Presence, Template, TokenForm, Wrapping};
pub use vocabulary::{
    Authority, Deprecation, OwnerProducer, OwnerProducerKind, PointOfContact, TokenMetadataFull,
    Vocabulary,
};
