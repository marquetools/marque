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
//! - [`lattice`] — `JoinSemilattice`, `MeetSemilattice`,
//!   `BoundedJoinSemilattice`, `BoundedMeetSemilattice`, and the
//!   `Lattice` / `BoundedLattice` blanket-impl marker traits.
//! - [`category`] — `Category`, `AggregationOp`, `Cardinality`,
//!   `IntraOrdering`, and generic reducers keyed by `AggregationOp`.
//! - [`constraint`] — declarative `Constraint` invariants.
//! - [`fact_bitmask`] — [`FactBitmask`] + [`FACT_BITMASK_WIDTH`]:
//!   packed Boolean characteristic-vector primitive (`u128`) for
//!   closed-vocab atom sets. Domain-neutral storage shape;
//!   per-scheme atom layouts live in the consuming crate
//!   (`marque-capco` for CAPCO). (#371 PR-A)
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
//! - [`render_context`] — `RenderContext { scope, emission_form,
//!   schema_version }`, `EmissionForm` (Auto / Portion / BannerTitle /
//!   BannerAbbreviation), `SchemaVersionId` (PR 3c.2.A scaffolding;
//!   §G.1 Table 4 dispatch body lands at PR 3c.2.B).
//! - [`vocabulary`] — `Vocabulary<S>` trait + `TokenMetadataFull`,
//!   `Authority`, `OwnerProducer`, `PointOfContact`, `Deprecation`
//!   (Phase E).
//! - [`codec`] — `Codec<S>` trait + `CodecError` surface — pinned
//!   for Phase G to implement against (Phase E).
//! - [`canonical`] — `Canonical<S>`, `TokenSource`,
//!   `CanonicalConstructor<S>` (sealed), `EngineConstructor<S>`.
//!   The provenance-tagged single-token replacement type with a
//!   sealed open-vocab construction path; the engine is the only
//!   crate that can mint open-vocab `Canonical<S>` values, via the
//!   sealed `CanonicalConstructor<S>` trait whose lone impl is
//!   `EngineConstructor<S>`. Closes the G13 leak channel as a type
//!   invariant for closed-CVE tokens (PR 3c.1; rule-API surface
//!   `FixIntent<S>` lives in `marque-rules`). See source plan §8.1.
//!
//! ## Status
//!
//! Phase E trait surface is complete. The lattice, projection,
//! recognizer, vocabulary, and codec surfaces are pinned and
//! consumed by `marque-capco` (the in-tree adapter). PR 3c.1 added
//! the [`canonical`] module — the sealed-construction surface that
//! PR 3c.2 will wire into rule emission and engine promotion. A
//! second scheme can land in its own crate without touching this
//! one — see `crates/scheme/tests/adoption_readiness.rs` for the
//! SC-010 pre-verification (`StubScheme` builds against
//! `marque_scheme::*` and `std::*` only).

pub mod ambiguity;
pub mod builtins;
pub mod canonical;
pub mod category;
pub mod closure;
pub mod codec;
pub mod constraint;
pub mod fact_bitmask;
pub mod fix_intent;
pub mod lattice;
pub mod page_rewrite;
pub mod projection;
pub mod recognizer;
pub mod render_context;
pub mod scheme;
pub mod scope;
pub mod severity;
pub mod span;
pub mod template;
pub mod vocabulary;

pub use ambiguity::{Candidate, EvidenceFeature, Parsed};
pub use builtins::{
    FlatSet, IntersectSet, MaxDate, ModeSet, OptionalSingleton, OrdMax, OrdMin, Product,
    SupersessionSet,
};
pub use canonical::{Canonical, CanonicalConstructor, EngineConstructor, TokenSource};
pub use category::{
    AggregationOp, Cardinality, Category, CategoryId, CategoryShape, ExpansionFn, IntraOrdering,
    TokenId, reduce_intersect, reduce_max, reduce_union, reduce_union_with_supersession,
};
pub use closure::{ClosureRule, ClosureRuleMetadata, ConeDerivedFn, MAX_CLOSURE_ITERATIONS};
pub use codec::{Codec, CodecError};
pub use constraint::{Constraint, ConstraintViolation, FamilyPredicate, TokenRef};
pub use fact_bitmask::{FactBitmask, WIDTH as FACT_BITMASK_WIDTH};
pub use fix_intent::{FactRef, RecanonScope, ReplacementIntent};
pub use lattice::{
    BoundedJoinSemilattice, BoundedLattice, BoundedMeetSemilattice, JoinSemilattice, Lattice,
    MeetSemilattice,
};
pub use page_rewrite::{
    CategoryAction, CategoryPredicate, PageRewrite, PageRewriteAxisError, RewriteId,
};
pub use projection::{Projection, categories_in_render_order};
pub use recognizer::{DocumentPosition, ParseContext, Recognizer, Zone};
pub use render_context::{EmissionForm, RenderContext, SchemaVersionId};
pub use scheme::{ApplyIntentError, MarkingScheme};
pub use scope::{DiffInput, DiffRelation, Scope};
pub use severity::Severity;
pub use span::Span;
pub use template::{CategoryRule, Presence, Template, TokenForm, Wrapping};
pub use vocabulary::{
    Authority, Deprecation, FormKind, FormSet, OwnerProducer, OwnerProducerKind, PointOfContact,
    TokenMetadataFull, Vocabulary,
};
