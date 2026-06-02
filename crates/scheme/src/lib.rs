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
//!   (`marque-capco` for CAPCO) (#371).
//! - [`template`] — structural templates for portion / banner / CAB.
//! - [`projection`] — `Projection` trait and render-order helpers.
//! - [`ambiguity`] — `Parsed<M>`, `Candidate`, `EvidenceFeature`.
//! - [`scheme`] — the `MarkingScheme` trait.
//! - [`page_rewrite`] — declarative `PageRewrite`, `CategoryAction`,
//!   `CategoryPredicate`.
//! - [`scope`] — `Scope` enum for projection contexts.
//! - [`builtins`] — built-in lattice constructors `OrdMax`, `OrdMin`,
//!   `FlatSet`, `IntersectSet`, `SupersessionSet`, `ModeSet`,
//!   `MaxDate`, `OptionalSingleton`, `Product`.
//! - [`recognizer`] — `Recognizer<S>` trait + `ParseContext`
//!   (decoder dispatch).
//! - [`input`] — input boundary (#176 / #643): `InputSource`
//!   (recognition-provenance axis), `InputContext<'a>` (the
//!   `ParseContext` wrapper the engine routes on), and the
//!   `InputAdapter` / `StructuredDocument` schema-document surface.
//! - [`render_context`] — `RenderContext { scope, emission_form,
//!   schema_version }`, `EmissionForm` (Auto / Portion / BannerTitle /
//!   BannerAbbreviation), `SchemaVersionId`. The §G.1 Table 4
//!   emission-form dispatch body is future work.
//! - [`vocabulary`] — `Vocabulary<S>` trait + `TokenMetadataFull`,
//!   `Authority`, `OwnerProducer`, `PointOfContact`, `Deprecation`.
//! - [`codec`] — `Codec<S>` trait + `CodecError` surface — pinned
//!   for concrete XML/JSON impls to implement against.
//! - [`canonical`] — `Canonical<S>`, `TokenSource`,
//!   `CanonicalConstructor<S>` (sealed), `EngineConstructor<S>`.
//!   The provenance-tagged single-token replacement type with a
//!   sealed open-vocab construction path; the engine is the only
//!   crate that can mint open-vocab `Canonical<S>` values, via the
//!   sealed `CanonicalConstructor<S>` trait whose lone impl is
//!   `EngineConstructor<S>`. Closes the audit content-ignorance leak
//!   channel as a type invariant for closed-CVE tokens; the rule-API
//!   surface `FixIntent<S>` lives in `marque-rules`.
//! - [`decision`] — opt-in `DecisionSink` instrumentation surface
//!   (`NoopSink` / `CountingSink` / `RecordingSink`) for counting and
//!   tracing the marking decisions an engine run makes. Off by default;
//!   engine threading is gated on the `decision-tracing` feature.
//! - [`artifact`] — document-scoped artifact node model: `ArtifactKind`,
//!   the five-state `ArtifactState<P>` (status enum, not a lattice), and
//!   `DocumentArtifact<S>` parameterized by the scheme's
//!   `SchemeArtifacts::ArtifactPayload`.
//! - [`document_context`] — `DocumentContext<S>`: the document-scope
//!   rollup (the page→document lattice fold via
//!   `MarkingScheme::canonical_document_join`) plus document-scoped
//!   `DocumentArtifact<S>` nodes.
//! - [`provenance`] — the two orthogonal provenance axes:
//!   `RecognitionProvenance` (adapter axis) and `ValueDerivation`
//!   (DAG-node axis).
//! - [`derivation`] — `DerivationEdge` (inbound derivation relations with
//!   `reads` / `writes` dataflow + a `FiringPredicate`), mirroring
//!   `PageRewrite`'s shape for the Phase C topological scheduler.
//!
//! ## Status
//!
//! The lattice, projection, recognizer, vocabulary, and codec surfaces
//! are pinned and consumed by `marque-capco` (the in-tree adapter). The
//! [`canonical`] module is the sealed-construction surface for rule
//! emission and engine promotion. A second scheme can land in its own
//! crate without touching this one — see
//! `crates/scheme/tests/adoption_readiness.rs` for the
//! `StubScheme` pre-verification (builds against `marque_scheme::*` and
//! `std::*` only).

pub mod ambiguity;
pub mod artifact;
pub mod builtins;
pub mod canonical;
pub mod category;
pub mod citation;
pub mod closure;
pub mod codec;
pub mod constraint;
pub mod decision;
pub mod derivation;
pub mod document_context;
pub mod fact_bitmask;
pub mod fix_intent;
pub mod input;
pub mod lattice;
pub mod page_rewrite;
pub mod projection;
pub mod provenance;
pub mod recognizer;
pub mod render_context;
pub mod scheme;
pub mod scope;
pub mod severity;
pub mod span;
pub mod template;
pub mod vocabulary;

pub use ambiguity::{Candidate, EvidenceFeature, Parsed};
pub use artifact::{ArtifactKind, ArtifactState, DocumentArtifact};
pub use builtins::{
    FlatSet, IntersectSet, MaxDate, ModeSet, OptionalSingleton, OrdMax, OrdMin, Product,
    SupersessionSet,
};
pub use canonical::{Canonical, CanonicalConstructor, EngineConstructor, TokenSource};
pub use category::{
    AggregationOp, Cardinality, Category, CategoryId, CategoryShape, ExpansionFn, IntraOrdering,
    TokenId, reduce_intersect, reduce_max, reduce_union, reduce_union_with_supersession,
};
pub use citation::{
    AuthoritativeSource, Citation, PageNumber, SectionLetter, SectionRef, capco, capco_section,
    capco_table,
};
pub use closure::{ClosureRule, ClosureRuleMetadata, ConeDerivedFn, MAX_CLOSURE_ITERATIONS};
pub use codec::{Codec, CodecError};
pub use constraint::{Constraint, ConstraintViolation, FamilyPredicate, TokenRef};
pub use decision::report::{CascadeChain, DecisionReport};
pub use decision::sinks::{CountingSink, NoopSink, RecordingSink};
pub use decision::{DecisionEvent, DecisionKind, DecisionSink, DecisionSite, DecisionSource};
pub use derivation::{DerivationEdge, DerivationRelation, EdgeId, FiringPredicate};
pub use document_context::DocumentContext;
pub use fact_bitmask::{FactBitmask, WIDTH as FACT_BITMASK_WIDTH};
pub use fix_intent::{
    FactRef, RecanonPriorState, RecanonScope, RelocatePriorState, ReplacementIntent,
};
pub use input::{
    AdaptError, DocumentLayer, DocumentStructure, InputAdapter, InputContext, InputSource,
    RepairKind, StructuredDocument,
};
pub use lattice::{
    BoundedJoinSemilattice, BoundedLattice, BoundedMeetSemilattice, JoinSemilattice, Lattice,
    MeetSemilattice,
};
pub use page_rewrite::{
    CategoryAction, CategoryPredicate, PageRewrite, PageRewriteAxisError, RewriteId,
};
pub use projection::{Projection, categories_in_render_order};
pub use provenance::{RecognitionProvenance, ValueDerivation};
pub use recognizer::{DocumentPosition, ParseContext, Recognizer, Zone};
pub use render_context::{EmissionForm, RenderContext, SchemaVersionId};
pub use scheme::{ApplyIntentError, MarkingScheme, SchemeArtifacts};
pub use scope::{DiffInput, DiffRelation, Scope};
pub use severity::Severity;
pub use span::Span;
pub use template::{CategoryRule, Presence, Template, TokenForm, Wrapping};
pub use vocabulary::{
    Authority, Deprecation, FormKind, FormSet, IcMarkingVocabulary, OwnerProducer,
    OwnerProducerKind, PointOfContact, TokenMetadataFull, Vocabulary,
};
