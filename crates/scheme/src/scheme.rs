// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! The `MarkingScheme` trait.
//!
//! A scheme bundles the data that defines a marking system (categories,
//! constraints, templates) with the parse/validate/project/render
//! operations the engine invokes. See the crate-level docs for the
//! conceptual framing.

use core::fmt;
use core::fmt::Debug;
use core::hash::Hash;

use crate::ambiguity::Parsed;
use crate::artifact::ArtifactKind;
use crate::category::{Category, CategoryId, TokenId};
use crate::closure::{ClosureRule, ClosureRuleMetadata};
use crate::constraint::{Constraint, ConstraintViolation, TokenRef};
use crate::derivation::DerivationEdge;
use crate::fix_intent::FactRef;
use crate::page_rewrite::PageRewrite;
use crate::render_context::RenderContext;
use crate::scope::Scope;
use crate::span::Span;
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

    /// The scheme's full-marking type — a **projection target**, not a
    /// lattice element.
    ///
    /// `Marking` intentionally carries no `JoinSemilattice` bound.
    /// The per-axis lattices (`RelToBlock`, `DissemSet`, `SciSet`,
    /// `SarSet`, etc.) are sound lattices on their native domain
    /// (expanded `2^{Trigraph}` for REL TO, etc.); `Marking` is the
    /// **cross-axis fold** of those lattice values back into a single
    /// structural record. Cross-axis folding is a *projection*, not a
    /// lattice operation. Claiming `JoinSemilattice` on the cross-axis
    /// record-type would promise a law (idempotence on structural `Eq`)
    /// the construction cannot keep: tetragraph expansion in
    /// `RelToBlock::from_attrs_iter` means `m.join(m)` can expand
    /// `m.rel_to = [NATO]` into `{30 expanded trigraphs}`, so structural
    /// `Eq` fails idempotence. Keeping that law would require either
    /// lossy eager canonicalization at construction or a quotient-`Eq`
    /// on every `CanonicalAttrs`-shaped field — both rejected as
    /// blast-radius-too-large. Omitting the bound removes the false
    /// claim instead.
    ///
    /// The per-axis lattices keep their own `JoinSemilattice` impls
    /// (sound on their respective domains). Schemes whose
    /// cross-axis-fold needs a "join-shaped" entry point should expose
    /// it as an inherent method on the scheme's marking type (e.g.
    /// `CapcoMarking::join_via_lattice`) rather than via this trait
    /// bound; the engine's `project_from_attrs_slice` hot path takes
    /// exactly that shape.
    ///
    /// PR #502 (issue #456) introduced the
    /// `JoinSemilattice`/`MeetSemilattice` split. The systematic audit
    /// of per-axis lattices for structural-vs-lattice-`Eq` mismatches
    /// (`DissemSet::relido_observed_unanimous`,
    /// `JointSet::Mixed`/`DisunityCollapse`, etc.) is tracked as a
    /// follow-up issue.
    type Marking;

    /// Parse-level errors produced by `parse`.
    type ParseError;

    /// The scheme's open-vocabulary structural reference type.
    ///
    /// `FactRef<S>` (in `marque-rules`) names tokens in the projected
    /// fact set. Closed-CVE tokens flow through `FactRef::Cve(TokenId)`;
    /// open-vocabulary tokens (SAR program identifiers, SCI compartment
    /// / sub-compartment paths, FGI tetragraphs in CAPCO, and whatever
    /// the equivalent open-vocab carriers are in future schemes) flow
    /// through `FactRef::OpenVocab(S::OpenVocabRef)`.
    ///
    /// The bound set is what `FactRef<S>` / `FixIntent<S>` propagate to
    /// callers — `Debug` and `Clone` because the rule-emission API
    /// derives both; `Eq + Hash` because audit-emitter call paths may
    /// key on the reference (and downstream consumers building lookup
    /// tables benefit); `Send + Sync` because `BatchEngine` schedules
    /// `FixIntent<S>` across worker threads (Constitution VI);
    /// `'static` because open-vocab references must own their data
    /// (a SAR program identifier as a `Box<str>` or an enum, not a
    /// `&'src str` into the input buffer — that would re-introduce an
    /// audit content-ignorance leak channel).
    ///
    /// Schemes with no open-vocab axes bind this to
    /// `std::convert::Infallible`, which carries no runtime values and
    /// makes `FactRef::OpenVocab(...)` statically unreachable for that
    /// scheme.
    type OpenVocabRef: Debug + Clone + Eq + Hash + Send + Sync + 'static;

    /// The scheme's borrowed parsed-attrs type — input to
    /// [`Self::canonicalize`].
    ///
    /// CAPCO binds this to `marque_ism::ParsedAttrs<'src>`; future
    /// schemes (CUI, NATO) bind to their own borrowed parsed shape.
    /// Test stubs that do not exercise the canonicalize path bind
    /// to `()` — the `unimplemented!()` default on
    /// [`Self::canonicalize`] is unreachable from their code paths.
    ///
    /// This is a GAT; GATs stabilized in Rust 1.65 and the workspace
    /// MSRV is 1.85.
    type Parsed<'src>;

    /// The scheme's owned canonical-attrs type — output of
    /// [`Self::canonicalize`].
    ///
    /// CAPCO binds this to `marque_ism::CanonicalAttrs`; future
    /// schemes bind to their own owned canonical shape.
    ///
    /// The bound set is what the generic engine pipeline requires to
    /// thread page state in canonical space: `Clone` (the per-candidate
    /// accumulator clones recognized canonicals), `Default` (the
    /// page-join accumulator seeds from the lattice bottom), `Debug`
    /// (the read-only-attrs `debug_assert` sentinel), and `Send + Sync +
    /// 'static` (`BatchEngine` threads canonical state across workers,
    /// Constitution VI). `marque_ism::CanonicalAttrs` already satisfies
    /// all five; test stubs binding `type Canonical = ()` satisfy them
    /// trivially.
    type Canonical: Clone + Default + core::fmt::Debug + Send + Sync + 'static;

    /// The page-roll-up projection shape banner/CAB rules consume via
    /// `RuleContext::page_marking`. The engine produces it from a
    /// `Scope::Page` projection; rules read it but never construct it.
    /// CAPCO binds this to `marque_ism::ProjectedMarking`.
    type Projected: core::fmt::Debug + Clone + Send + Sync + 'static;

    /// Convert a parsed-attrs value into the scheme's canonical
    /// representation.
    ///
    /// This is the **sole production path** for `Parsed → Canonical`.
    /// The trait method has an `unimplemented!()` default; schemes that
    /// canonicalize (CapcoScheme) override it.
    ///
    /// # Why the default is `unimplemented!()` (not delegation)
    ///
    /// `marque-scheme` is domain-neutral and cannot reference
    /// `marque_ism` types in a default body — Constitution VII
    /// directionality. Schemes that need canonicalization MUST
    /// override; schemes that don't (test-fixture schemes binding
    /// `type Parsed<'src> = ();` and `type Canonical = ();`) inherit
    /// this default safely because their rule paths never call
    /// `canonicalize`.
    fn canonicalize<'src>(&self, _parsed: Self::Parsed<'src>) -> Self::Canonical {
        unimplemented!(
            "MarkingScheme::canonicalize not overridden by this scheme. \
             Schemes that canonicalize (e.g. CapcoScheme) override it; test \
             stub schemes that never call canonicalize() inherit the default \
             safely (the panic is unreachable from their code paths)."
        )
    }

    /// Human-readable name, e.g., "CAPCO-ISM-v2022-DEC".
    fn name(&self) -> &str;

    /// Schema/version identifier used for cache invalidation and audit
    /// logs.
    fn schema_version(&self) -> &str;

    /// Version of the scheme's lattice algebra — the meet/join
    /// semantics and the category set — distinct from
    /// [`Self::schema_version`] (which tracks the upstream CVE package
    /// label). Surfaced into the audit-record session metadata so a
    /// fix can be traced to the exact lattice revision that produced
    /// it.
    ///
    /// Defaults to [`Self::schema_version`] for schemes whose lattice
    /// tracks the schema 1:1; CAPCO overrides it because its lattice
    /// surface (`SciSet` / `SarSet` / `FgiSet`, the §3.3a meet, the
    /// `Lattice` split) evolves independently of the ODNI CVE package.
    fn lattice_version(&self) -> &str {
        self.schema_version()
    }

    /// All categories in the scheme, in arbitrary order. Sort by
    /// `ordering_rank` for render order.
    fn categories(&self) -> &[Category];

    /// Declarative invariants checked by `validate`.
    fn constraints(&self) -> &[Constraint];

    /// The scheme's wire namespace — the `scheme` half of every rule-id
    /// 2-tuple this scheme mints.
    ///
    /// Defaults to `"scheme"`; every real scheme overrides it
    /// (`CapcoScheme` returns `"capco"`). Returned as `&'static str` so
    /// the projection in [`Self::constraint_rule_id`] allocates nothing.
    fn scheme_id(&self) -> &'static str {
        "scheme"
    }

    /// Projects a constraint `label` to the rule-id 2-tuple
    /// `(scheme, predicate_id)` the engine's constraint bridge stamps
    /// onto a `Diagnostic`.
    ///
    /// Constraint labels are authored in the canonical
    /// `<surface>.<category>.<predicate>` predicate form, so the default
    /// is the identity projection `(self.scheme_id(), label)` — the label
    /// is the predicate id verbatim under the scheme's namespace. Returns
    /// the bare 2-tuple rather than a `marque_rules::RuleId` because
    /// `marque-scheme` is the dependency-graph leaf (Constitution VII)
    /// and must not depend on `marque-rules`; the engine constructs
    /// `RuleId::new(scheme, predicate_id)` at the bridge.
    fn constraint_rule_id(&self, label: &'static str) -> (&'static str, &'static str) {
        (self.scheme_id(), label)
    }

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
    /// does not declare dyadic constraints is still well-formed — only
    /// the variants the scheme actually uses need coverage.
    fn satisfies(&self, _marking: &Self::Marking, _token_ref: &TokenRef) -> bool {
        false
    }

    /// Resolve the source byte span for a given token or category
    /// presence in `marking`.
    ///
    /// The default implementation returns `None`. Schemes that want
    /// their declarative constraint catalog to produce user-facing
    /// diagnostics (via the engine bridge) MUST override this method
    /// to provide a valid span for the triggering token.
    fn token_span(&self, _marking: &Self::Marking, _token_ref: &TokenRef) -> Option<Span> {
        None
    }

    /// Resolve a [`FactRef`] to its [`CategoryId`].
    ///
    /// The engine consults this when materializing a [`ReplacementIntent`]
    /// (in [`Self::apply_intent`]) so it knows which category-axis the
    /// fact-set delta targets. For example, `FactRef::Cve(TOK_RELIDO)`
    /// resolves to `CAT_DISSEM` in CAPCO; `FactRef::Cve(TOK_EXDIS)`
    /// resolves to `CAT_NON_IC_DISSEM`.
    ///
    /// Returns `None` when the token is not known to this scheme — a
    /// programmer error in the rule's emission (the engine surfaces
    /// this as [`ApplyIntentError::UnknownToken`]).
    ///
    /// Default implementation returns `None` for every token, treating
    /// the scheme as having no routing table yet. The engine's
    /// construction-time validation (`validate_intent_rewrites`) and
    /// `apply_intent` runtime path both treat `None` as
    /// [`ApplyIntentError::UnknownToken`]; a scheme that has not yet
    /// overridden `category_of` will surface this as a controlled
    /// `Err` rather than a panic, preserving Constitution VI's
    /// non-unwinding guarantee even when the scheme hasn't finished
    /// the intent-based-fix migration.
    ///
    /// [`ReplacementIntent`]: crate::ReplacementIntent
    /// [`ApplyIntentError`]: ApplyIntentError
    fn category_of(&self, _token: &FactRef<Self>) -> Option<CategoryId>
    where
        Self: Sized,
    {
        None
    }

    /// Apply a batch of [`ReplacementIntent`]s to a marking, returning
    /// the modified marking.
    ///
    /// This is the bag-of-tokens fix-synthesis bridge — the engine
    /// receives the rule's structural intent emission, clones the
    /// scheme's current marking, and asks the scheme to apply each
    /// intent atomically. The engine then renders the modified marking
    /// via [`Self::render_canonical`] to produce the final fix bytes.
    ///
    /// # Slice argument
    ///
    /// The `intents` slice carries one or more intents that all target
    /// the same candidate span. This shape supports atomic multi-fact
    /// changes (e.g., E024's RD/FRD/TFNI multi-remove cluster) without
    /// requiring the engine to splice multiple per-token edits into
    /// the same byte range — the scheme applies every intent to the
    /// cloned marking, then the engine renders once.
    ///
    /// # Invariants
    ///
    /// - **Idempotent**: applying the same intent batch twice produces
    ///   the same result. An impl MUST treat `FactAdd` of a token
    ///   already present as a no-op rather than duplicating it.
    /// - **Commutative within a batch**: intent order within a single
    ///   `intents` slice MUST NOT affect output. Implementations may
    ///   sort, deduplicate, or interleave application; callers MUST
    ///   NOT depend on the slice's index order.
    /// - **Stateless**: the scheme MUST NOT mutate any shared state.
    ///   Apply against the `marking` argument only.
    /// - **Output rendering authority**: the engine calls
    ///   [`Self::render_canonical`] on the returned marking. Conforming
    ///   impls return a fact-set-correct marking; canonical-form
    ///   normalization (delimiter spacing, sort order, abbreviation
    ///   form, banner roll-up) is the renderer's job, not
    ///   `apply_intent`'s.
    ///
    /// # Errors
    ///
    /// - [`ApplyIntentError::IntentInapplicable`] — returned ONLY when
    ///   the entire batch is a no-op (no intent in the slice produced
    ///   any change to the marking). Engine drops the fix silently —
    ///   the marking is already consistent. **Per-intent inapplicability
    ///   within a batch is NOT a failure**: an impl MUST silently skip
    ///   the redundant intent and continue applying the rest. This is
    ///   what the idempotence/commutativity invariants require — two
    ///   rules emitting the same `FactRemove`, or one intent in the
    ///   batch removing a token a prior intent already removed, must
    ///   not abort the batch.
    /// - [`ApplyIntentError::UnknownToken`] — a [`FactRef::Cve`]'s
    ///   [`crate::TokenId`] doesn't map to any category. Programmer
    ///   error in the rule; engine logs and skips the fix. Propagates
    ///   immediately, even mid-batch.
    /// - [`ApplyIntentError::IntentRejectsLattice`] — applying would
    ///   produce a marking that violates a structural invariant the
    ///   scheme can't repair through fact-set delta alone. Engine
    ///   surfaces as a diagnostic. Propagates immediately, even mid-batch.
    ///
    /// # Default implementation
    ///
    /// Panics with `unimplemented!()` so schemes that do not yet
    /// support intent-based fix application still compile — but the
    /// panic surfaces at engine-fix time if a rule emits a [`FixIntent`]
    /// against the scheme. Test-fixture schemes (the five stubs in
    /// `crates/scheme/tests/`) inherit the default and never trigger
    /// it because their rule paths emit only `FixProposal`.
    ///
    /// [`ReplacementIntent`]: crate::ReplacementIntent
    /// [`FactRef::Cve`]: crate::FactRef::Cve
    fn apply_intent(
        &self,
        _marking: &Self::Marking,
        _intents: &[crate::fix_intent::ReplacementIntent<Self>],
    ) -> Result<Self::Marking, ApplyIntentError>
    where
        Self: Sized,
    {
        unimplemented!(
            "apply_intent not supported by this scheme; \
             a rule emitted a FixIntent but the scheme has no intent-application \
             routing — implement apply_intent for this scheme."
        )
    }

    /// Pre-compute a [`FactBitmask`] projection of `marking` once per
    /// `evaluate` call so every [`Constraint::Custom`] row shares the
    /// same projection rather than each row recomputing it independently.
    ///
    /// [`crate::constraint::evaluate`] calls this method once before
    /// the constraint loop and forwards the returned bitmask to every
    /// [`Self::evaluate_custom`] call. Schemes that do not use a
    /// bitmask projection (e.g. schemes that haven't implemented
    /// [`crate::FactBitmask`] support yet) return
    /// [`FactBitmask::EMPTY`]; their [`evaluate_custom`] implementations
    /// can safely ignore the `bits` argument.
    ///
    /// Default: [`FactBitmask::EMPTY`] (no-op for schemes without
    /// bitmask support).
    ///
    /// [`FactBitmask::EMPTY`]: crate::FactBitmask::EMPTY
    fn precompute_bits(&self, _marking: &Self::Marking) -> crate::FactBitmask {
        crate::FactBitmask::EMPTY
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
    /// `bits` is the pre-computed [`FactBitmask`] projection of
    /// `marking`, obtained once per evaluation pass via
    /// [`Self::precompute_bits`]. Schemes with bitmask fast-paths use
    /// it to avoid recomputing the projection per row; schemes without
    /// bitmask support ignore it.
    ///
    /// Default: no violations.
    fn evaluate_custom(
        &self,
        _name: &'static str,
        _marking: &Self::Marking,
        _bits: crate::FactBitmask,
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
    /// - `Scope::Page` — per-page banner roll-up. CAPCO composes the
    ///   per-axis lattice constructors (`SciSet::from_markings`,
    ///   `RelToBlock::from_attrs_iter`, etc.) via
    ///   `CapcoMarking::join_via_lattice` then runs them through this
    ///   `project` trait method. Implementations should apply
    ///   component-wise category joins first, then run
    ///   [`Self::page_rewrites`] in declaration order.
    /// - `Scope::Document` — document-level roll-up. On single-page
    ///   documents this typically agrees with `Scope::Page`.
    /// - `Scope::Diff` — callers should use a dedicated diff entry
    ///   point carrying a [`crate::scope::DiffInput`]; this default
    ///   mirrors `Page` so a bare `project` call on diff scope is
    ///   still well-defined.
    fn project(&self, scope: Scope, markings: &[Self::Marking]) -> Self::Marking;

    /// Project a set of markings into a single marking under the given
    /// scope, emitting [`DecisionEvent`]s to the supplied sink as
    /// projection-stage decisions are made (component-wise category
    /// joins, page rewrites, supersession overlays, recanonicalization).
    ///
    /// The default implementation delegates to [`Self::project`] and
    /// emits no events — this is the zero-cost off-mode path that
    /// keeps Constitution Principle I (SC-001 16 ms p95) intact for
    /// schemes that don't opt in to instrumentation.
    ///
    /// Schemes that want to surface per-rewrite cascade events to a
    /// `marque trace` consumer override this method and call
    /// `sink.record(...)` between stages. The engine threads a sink
    /// here only when the user has opted into the `decision-tracing`
    /// feature and configured a non-[`crate::NoopSink`] sink on the
    /// engine.
    ///
    /// See `crates/scheme/src/decision.rs` for the event model and
    /// the [`crate::DecisionSink`] trait.
    ///
    /// [`DecisionEvent`]: crate::DecisionEvent
    fn project_with_sink(
        &self,
        scope: Scope,
        markings: &[Self::Marking],
        _sink: &mut dyn crate::DecisionSink,
    ) -> Self::Marking {
        self.project(scope, markings)
    }

    /// Convenience shim: project at page scope. Default implementation
    /// calls `project(Scope::Page, portions)`.
    #[inline]
    fn project_summary(&self, portions: &[Self::Marking]) -> Self::Marking {
        self.project(Scope::Page, portions)
    }

    /// Join a slice of canonicals into a single canonical, in
    /// **canonical space** (not marking space).
    ///
    /// The engine accumulates a page incrementally in `Self::Canonical`
    /// (the recognizer's per-candidate output), so the page roll-up join
    /// is expressed over canonicals rather than over `Self::Marking`.
    /// This is distinct from [`Self::project`], which operates on
    /// `&[Self::Marking]` and yields a marking.
    ///
    /// The default folds to the last portion (`portions.last().cloned()`,
    /// or the canonical bottom for an empty page) — the correct behavior
    /// for a scheme whose canonical carries no cross-portion composition.
    /// CapcoScheme overrides this with its per-axis lattice join.
    fn canonical_page_join(&self, portions: &[Self::Canonical]) -> Self::Canonical {
        portions.last().cloned().unwrap_or_default()
    }

    /// Project a slice of canonicals into the page projection shape, in
    /// **canonical space** (the non-instrumented hot path).
    ///
    /// This is the canonical-space companion to [`Self::project`] (which
    /// operates on `&[Self::Marking]`). The engine produces its
    /// `RuleContext::page_marking` from this when its page accumulator is
    /// already in canonical space and no decision sink is installed.
    ///
    /// The default is `unimplemented!()` (mirrors [`Self::canonicalize`]):
    /// `marque-scheme` is domain-neutral and cannot synthesize a
    /// `Self::Projected` from `Self::Canonical` generically. Schemes that
    /// project (CapcoScheme) override; test-stub schemes never reach this
    /// path and inherit the default safely.
    fn project_canonical(&self, _portions: &[Self::Canonical]) -> Self::Projected {
        unimplemented!(
            "MarkingScheme::project_canonical not overridden by this scheme. \
             Schemes that project from canonical space (e.g. CapcoScheme) override it; \
             test stub schemes never reach this path and inherit the default safely."
        )
    }

    /// Sink-aware variant of [`Self::project_canonical`], emitting
    /// [`DecisionEvent`]s to the sink as projection-stage decisions are
    /// made. The engine threads a sink here only when an observer is
    /// installed; otherwise it calls the plain [`Self::project_canonical`]
    /// hot path.
    ///
    /// The default delegates to [`Self::project_canonical`] and emits no
    /// events — the zero-cost off-mode path, mirroring how
    /// [`Self::project_with_sink`] defaults to [`Self::project`]. So any
    /// scheme that implements `project_canonical` gets a correct (if
    /// uninstrumented) sink method for free, and calling the public sink
    /// method never panics on it. Schemes that want per-stage projection
    /// events override this (CapcoScheme does, under its `decision-tracing`
    /// gate).
    ///
    /// [`DecisionEvent`]: crate::DecisionEvent
    fn project_canonical_with_sink(
        &self,
        portions: &[Self::Canonical],
        _sink: &mut dyn crate::DecisionSink,
    ) -> Self::Projected {
        self.project_canonical(portions)
    }

    /// Convert an owned canonical into the scheme's marking
    /// representation.
    ///
    /// The engine holds recognized state as `Self::Canonical` but several
    /// trait surfaces (notably [`Self::validate`]) take `&Self::Marking`;
    /// this is the lift the engine uses to cross from canonical space into
    /// marking space.
    ///
    /// The default is `unimplemented!()` (mirrors [`Self::canonicalize`]):
    /// a generic canonical→marking conversion does not exist. Schemes that
    /// canonicalize (CapcoScheme) override; test-stub schemes never reach
    /// this path and inherit the default safely.
    fn marking_from_canonical(&self, _canonical: Self::Canonical) -> Self::Marking {
        unimplemented!(
            "MarkingScheme::marking_from_canonical not overridden by this scheme. \
             Schemes that canonicalize (e.g. CapcoScheme) override it; test stub \
             schemes never reach this path and inherit the default safely."
        )
    }

    /// Cross-category rewrites applied after component-wise
    /// page-scope projection. CAPCO's canonical entry is
    /// NOFORN-clears-REL-TO.
    ///
    /// Default: no rewrites. Schemes override to declare their table.
    fn page_rewrites(&self) -> &[PageRewrite<Self>] {
        &[]
    }

    /// The document-scoped artifact kinds this scheme models (CABs,
    /// declassify instructions, notices, caveat layers, front markings).
    ///
    /// This is a declarative inventory surface, parallel to
    /// [`Self::categories`] / [`Self::page_rewrites`]: tooling and the
    /// engine can read which artifact kinds a scheme recognizes without
    /// executing scheme code.
    ///
    /// Default: empty slice (no document artifacts declared). Returning
    /// the empty slice is behavior-neutral — a scheme that does not model
    /// document artifacts is unchanged.
    fn document_artifacts(&self) -> &[ArtifactKind] {
        &[]
    }

    /// The inbound derivation edges this scheme declares for its
    /// document-scoped artifacts.
    ///
    /// Mirrors [`Self::page_rewrites`]'s `reads` / `writes` shape so the
    /// engine's Kahn scheduler (Phase C) can order derivation edges and
    /// page rewrites in one topological pass. The edge topology is static
    /// (validated at `Engine::new`); per-edge firing is conditional via
    /// each edge's [`crate::FiringPredicate`].
    ///
    /// Default: empty slice (no derivation edges declared).
    fn derivation_edges(&self) -> &[DerivationEdge] {
        &[]
    }

    /// Declared closure rules for this scheme.
    ///
    /// Closure rules implement implicit-fact propagation. They
    /// propagate facts that the marking system doesn't require to be written
    /// explicitly — for example, that a CAPCO marking with no explicit FD&R
    /// control implies NOFORN as the effective release restriction.
    ///
    /// Schemes opt in by overriding this method to return their catalog.
    /// The default [`Self::closure()`] implementation is a **no-op** —
    /// because a truly generic Kleene-fixpoint walker requires a
    /// scheme-level fact-join operation that this trait does not
    /// currently expose, schemes that want runtime cone application
    /// must also override `closure()` with their own implementation
    /// (typically walking the catalog to fixpoint, bounded by
    /// [`crate::closure::MAX_CLOSURE_ITERATIONS`]).
    ///
    /// Returning closure rules without overriding `closure()` is a
    /// supported **catalog-data-only mode** used by tooling, scheme-
    /// exploration UIs, and proptest harnesses that walk
    /// `should_fire` directly without applying the cone. This is a
    /// PUBLIC catalog surface — visible to tooling, scheme-exploration
    /// UIs, and docs generators — not a private engine detail.
    ///
    /// Default: empty slice (no closure rules declared).
    fn closure_rules(&self) -> &[ClosureRule<Self>]
    where
        Self: Sized,
    {
        &[]
    }

    /// Declared closure-rule inventory metadata for this scheme.
    ///
    /// This is the scheme-agnostic discovery/config surface for closure rows.
    /// The default forwards from [`Self::closure_rules()`], preserving current
    /// behavior for schemes that still expose closure rules only in fn-pointer
    /// form.
    fn closure_inventory(&self) -> Box<dyn Iterator<Item = ClosureRuleMetadata> + '_>
    where
        Self: Sized,
    {
        Box::new(self.closure_rules().iter().map(ClosureRuleMetadata::from))
    }

    /// Returns the host [`CategoryId`] for a given [`TokenId`], used by
    /// the closure operator to route cone tokens to their appropriate
    /// category when adding facts.
    ///
    /// This method is the scheme-layer hook required by the closure
    /// operator: `closure()` needs to know which category a cone token
    /// belongs to so it can add the fact to the right axis of the marking.
    /// The existing `category_of` method is keyed by `FactRef<Self>` (a
    /// `marque-rules` type) and is unavailable at the `marque-scheme`
    /// layer; this method provides the same routing over the simpler
    /// `TokenId` key.
    ///
    /// This method exists specifically to resolve the scheme-layer /
    /// rules-layer boundary constraint on the closure operator's token
    /// routing.
    ///
    /// Returns `None` when the token is not known to this scheme. Schemes
    /// that do not implement closure rules do not need to override this
    /// method (the default `closure()` is a no-op).
    ///
    /// Default: returns `None` for every token.
    fn token_category(&self, _id: TokenId) -> Option<CategoryId> {
        None
    }

    /// Enumerate all [`TokenRef::Token`] references that are currently
    /// present in `marking`.
    ///
    /// Used by the [`crate::constraint::evaluate`] function when dispatching
    /// [`crate::constraint::Constraint::ConflictsWithFamily`] rows: it walks
    /// every token present in the marking and applies the
    /// [`crate::constraint::FamilyPredicate`] to each, emitting one
    /// violation per match that co-occurs with the LHS token.
    ///
    /// Schemes MUST override this method if they declare any
    /// `ConflictsWithFamily` constraints — the default returns an empty
    /// iterator, which would silently cause the family predicate to never
    /// fire. Schemes without family constraints do not need to override.
    ///
    /// The iterator yields `TokenRef` values for facts currently present
    /// in the marking. Both variants are permitted:
    ///
    /// - `TokenRef::Token(id)` for concrete present tokens whose
    ///   identity matters (the common case).
    /// - `TokenRef::AnyInCategory(cat)` for facts whose presence is
    ///   axis-level only (e.g., `CAT_REL_TO` for "some REL TO country
    ///   list is present" without enumerating the specific countries).
    ///   Family predicates may match on category granularity rather
    ///   than per-token (e.g., CapcoScheme's `is_fdr_dominator`
    ///   matches `AnyInCategory(CAT_REL_TO)` to capture REL TO as an
    ///   FD&R-chain member without enumerating each country).
    ///
    /// `AnyInCategory` is permitted so the output can align with
    /// CapcoScheme's actual emission (`AnyInCategory(CAT_REL_TO)`,
    /// `CAT_SCI`, `CAT_SAR`, `CAT_NON_US_CLASSIFICATION` for axis-level
    /// facts) and the family-predicate idiom that depends on it.
    ///
    /// Default: empty iterator (no present tokens enumerated).
    fn iter_present_tokens<'m>(
        &self,
        _marking: &'m Self::Marking,
    ) -> Box<dyn Iterator<Item = TokenRef> + 'm> {
        Box::new(core::iter::empty())
    }

    /// Apply the closure operator to `marking`, returning the closed
    /// marking (the smallest superset of `marking` that satisfies all
    /// declared closure rules).
    ///
    /// The closure operator walks [`Self::closure_rules()`] to Kleene
    /// fixpoint: in each iteration, rules whose triggers fire and whose
    /// suppressors are absent add their cone facts to the working set.
    /// Iteration stops when no new facts are added (fixed point reached).
    ///
    /// ## Why the default is a no-op
    ///
    /// A truly generic Kleene-fixpoint implementation would need a
    /// scheme-level operation to join a cone token into `Self::Marking`
    /// (i.e., "add this token to the marking's fact set for its host
    /// category"). `Self::Marking` is an associated type with no
    /// constructor surface on this trait; constructing a singleton marking
    /// for a cone token and joining it requires scheme-internal knowledge.
    ///
    /// Rather than add a generic `singleton_marking(TokenId) -> Self::Marking`
    /// method (which would impose an additional implementation burden on
    /// every scheme), the default `closure()` is a no-op. Schemes that
    /// want **runtime cone application** override this method with their
    /// own fixpoint implementation; schemes that ship `closure_rules()`
    /// purely as inspection surface (catalog-data-only mode — see the
    /// note on [`Self::closure_rules`] above) can leave the default in
    /// place. The default is safe for schemes without closure rules —
    /// those schemes' `closure_rules()` returns `&[]` and no
    /// propagation is needed.
    ///
    /// ## Invariants (schemes that override MUST preserve these)
    ///
    /// 1. **Extensive**: `closure(m) ⊒ m` — the result is a superset of
    ///    the input (only facts are added, never removed).
    /// 2. **Idempotent**: `closure(closure(m)) == closure(m)` — the
    ///    fixed point is stable.
    /// 3. **Monotone**: if `m1 ⊑ m2` then `closure(m1) ⊑ closure(m2)`.
    ///
    /// The override MUST panic if it exceeds
    /// [`crate::closure::MAX_CLOSURE_ITERATIONS`] iterations without
    /// reaching a fixed point. This cap detects **non-convergence**
    /// (a catalog whose fact-set grows unbounded), but it does NOT
    /// detect every non-monotone catalog: a non-monotone catalog with
    /// a suppressor depending on facts in another rule's cone can
    /// converge quickly to a fixed point while still violating
    /// monotonicity 3. Monotonicity violations are caught by the
    /// proptest harness at
    /// `crates/scheme/tests/proptest_closure_rejects_non_monotone.rs`,
    /// not by the iteration cap. The cap exists as a runtime
    /// safeguard against catalog regressions that lead to
    /// non-termination, not as a monotonicity oracle.
    ///
    /// Default: returns `marking` unchanged (no-op).
    fn closure(&self, marking: Self::Marking) -> Self::Marking {
        // Default no-op: schemes without closure rules don't need
        // fact propagation. Schemes with closure rules override this
        // method. The no-op is correct for any scheme where
        // closure_rules() returns &[].
        marking
    }

    /// Apply the closure operator to `marking`, emitting one
    /// [`DecisionEvent`] per closure rule that fires.
    ///
    /// Default: delegates to [`Self::closure`] and emits no events.
    /// Schemes that want to thread events through their closure
    /// walker override this method; `marque-capco` is the planned
    /// override site for the §B.3 Table 2 caveated-implies-NOFORN
    /// rows and the post-#704 bitmask Kleene fixpoint stages, but
    /// at the trait level the default delegation is the contract.
    ///
    /// See `crates/scheme/src/decision.rs` for the event model.
    ///
    /// [`DecisionEvent`]: crate::DecisionEvent
    fn closure_with_sink(
        &self,
        marking: Self::Marking,
        _sink: &mut dyn crate::DecisionSink,
    ) -> Self::Marking {
        self.closure(marking)
    }

    /// Render a marking in canonical form per the given
    /// [`RenderContext`], writing the bytes through `out`.
    ///
    /// This is the **single source of truth for canonical form** in
    /// the scheme:
    ///
    /// > The renderer (`render_canonical`) is the single source of
    /// > canonical form. Form rules retire into it.
    ///
    /// # `RenderContext` parameter
    ///
    /// The `RenderContext` carries the projection scope, the emission
    /// form ([`crate::EmissionForm`] — closes the §G.1 Table 4
    /// four-form ambiguity), and the active schema version
    /// ([`crate::SchemaVersionId`]).
    ///
    /// Currently only `ctx.scope` is actively consumed by every impl
    /// body. Implementations should route on `ctx.scope` today and may
    /// ignore `ctx.emission_form` and `ctx.schema_version` until those
    /// are fully supported (the §G.1 Table 4 dispatch body is future
    /// work; `ctx.schema_version` is reserved for future expansion).
    ///
    /// # Lattice-equal-byte-identical property
    ///
    /// Two markings that compare equal under [`Lattice`](crate::lattice::Lattice)
    /// equality MUST render to byte-identical output for the same
    /// `RenderContext`. This is what makes `Recanonicalize` a sound
    /// fix: the renderer is referentially transparent over
    /// lattice-equivalent inputs, and the engine can therefore
    /// re-render a `ProjectedMarking` without consulting the input
    /// bytes that produced it.
    ///
    /// # Writer-passing contract
    ///
    /// `out` is intended to be a caller-pre-allocated, reusable
    /// buffer. The engine's lint loop holds a per-page scratch
    /// `String` and clears it between calls so that scoring N portions
    /// on a page produces O(1) heap allocations rather than O(N).
    /// Implementations MUST NOT assume `out` is empty on entry — they
    /// MUST append to it and return `Ok(())` on success — and they
    /// MUST NOT clear `out` themselves. The caller owns the buffer's
    /// lifetime.
    ///
    /// # Scope semantics — return-value contract
    ///
    /// Implementations MUST honor the following per-scope contract on
    /// `ctx.scope`. The default impls of [`Self::render_item`] /
    /// [`Self::render_summary`] rely on this contract being upheld;
    /// they `debug_assert!` on contract violation.
    ///
    /// - `Scope::Portion` — canonical portion form. MUST return `Ok(())`.
    /// - `Scope::Page` — canonical banner / CAB roll-up. MUST return `Ok(())`.
    /// - `Scope::Document` — canonical document-level rendering;
    ///   typically agrees with `Page` on single-page documents. MUST
    ///   return `Ok(())`.
    /// - `Scope::Diff` — diff is a *rule-context query mode*, not a
    ///   renderer-output scope. Implementations SHOULD return
    ///   `Err(fmt::Error)`. The architecture spec is explicit that
    ///   `RecanonScope` (in `marque-rules`) narrows `Scope` precisely
    ///   to exclude `Diff` from recanonicalization targets, so this
    ///   `Err` branch is unreachable from the engine's `Recanonicalize`
    ///   dispatch.
    ///
    /// Returning `Err` for `Portion` / `Page` / `Document` is a
    /// contract violation and is undefined behavior at the protocol
    /// level — debug builds panic via the default impls'
    /// `debug_assert!`; release builds fall through to an **empty**
    /// `String` (the default impls explicitly discard any partial
    /// output the violating impl may have written before returning
    /// `Err`, so downstream consumers never see a partial / subtly-
    /// wrong canonical form on contract violation).
    ///
    /// # Engine dispatch contract
    ///
    /// When `Engine::fix_inner` materializes a
    /// `ReplacementIntent::Recanonicalize { scope }`, it consults its
    /// in-scope projection (already computed during `lint` per
    /// Constitution VI's dataflow pipeline) for the named
    /// `RecanonScope`, then calls
    /// `render_canonical(&projection.marking, &RenderContext::new(scope.into(), Auto, schema), &mut writer)`.
    /// Rules NEVER carry the `ProjectedMarking` — the engine is the
    /// authority. See `ReplacementIntent::Recanonicalize` in
    /// `marque-rules` for the intent-side surface.
    fn render_canonical(
        &self,
        m: &Self::Marking,
        ctx: &RenderContext,
        out: &mut dyn fmt::Write,
    ) -> fmt::Result;

    /// Render a marking in portion form (abbreviated).
    ///
    /// Default delegates to [`Self::render_canonical`] with
    /// [`crate::scope::Scope::Portion`]. Implementations that already
    /// have a portion-renderer body MAY override this to avoid the
    /// `String` round-trip, but the override MUST produce
    /// byte-identical output to the default chain — `render_canonical`
    /// is the canonical-form authority.
    fn render_item(&self, m: &Self::Marking) -> String {
        let mut s = String::new();
        // `Write for String` is infallible, so a `String` write target
        // never produces `fmt::Error`. The only way the `Result` could
        // be `Err` is a contract violation: an impl returning `Err`
        // for `Scope::Portion` (the [`Self::render_canonical`] doc
        // comment forbids this). Debug-assert in development; on Err,
        // discard any partial output the violating impl may have
        // written before returning so downstream consumers see an
        // empty `String` rather than a partial / subtly-wrong
        // canonical form (the trait-level "empty on Err" guarantee).
        //
        // Construct an `Auto + MarqueMvp3` RenderContext; the §G.1
        // Table 4 emission-form dispatch body is future work.
        let ctx = RenderContext::new(
            crate::scope::Scope::Portion,
            crate::EmissionForm::Auto,
            crate::SchemaVersionId::MarqueMvp3,
        );
        let result = self.render_canonical(m, &ctx, &mut s);
        debug_assert!(
            result.is_ok(),
            "MarkingScheme::render_canonical contract violation: Err returned for Scope::Portion. \
             Conforming impls MUST return Ok(()) for Portion / Page / Document — see trait doc."
        );
        match result {
            Ok(()) => s,
            Err(_) => String::new(),
        }
    }

    /// Render a marking in banner form (expanded).
    ///
    /// Default delegates to [`Self::render_canonical`] with
    /// [`crate::scope::Scope::Page`]. Same byte-identity contract as
    /// [`Self::render_item`].
    ///
    /// # Note on scope naming
    ///
    /// The argument is [`crate::scope::Scope::Page`], not a hypothetical
    /// `Scope::Banner` — banner roll-up is *defined* as page-scope
    /// rendering in the architecture spec ("banner = lattice join over
    /// the page's portions"). The method name `render_summary` is the
    /// public API surface; the underlying scope is `Page`. Schemes that
    /// override this method MUST honor the same scope semantics.
    fn render_summary(&self, m: &Self::Marking) -> String {
        let mut s = String::new();
        // Construct an `Auto + MarqueMvp3` RenderContext; the §G.1
        // Table 4 emission-form dispatch body is future work.
        let ctx = RenderContext::new(
            crate::scope::Scope::Page,
            crate::EmissionForm::Auto,
            crate::SchemaVersionId::MarqueMvp3,
        );
        let result = self.render_canonical(m, &ctx, &mut s);
        debug_assert!(
            result.is_ok(),
            "MarkingScheme::render_canonical contract violation: Err returned for Scope::Page. \
             Conforming impls MUST return Ok(()) for Portion / Page / Document — see trait doc."
        );
        match result {
            Ok(()) => s,
            Err(_) => String::new(),
        }
    }
}

/// Opt-in extension trait for schemes that model document-scoped
/// artifacts.
///
/// The `ArtifactPayload` associated type — what a
/// [`DocumentArtifact`](crate::artifact::DocumentArtifact) node carries
/// when present — lives here rather than on [`MarkingScheme`] so the frozen
/// `MarkingScheme` surface gains only *defaulted* methods
/// ([`MarkingScheme::document_artifacts`] /
/// [`MarkingScheme::derivation_edges`]) and no new *required* associated
/// type. A scheme that does not model document artifacts simply does not
/// implement `SchemeArtifacts`, and still compiles unchanged — the critical
/// additive-staging property of this phase.
///
/// Schemes that DO model document artifacts implement this trait and bind
/// `ArtifactPayload` to their parsed-artifact type (CAPCO will eventually
/// bind it to a parsed-CAB structural type; in this phase it binds `()` as
/// a placeholder).
///
/// # Constitution V Principle V (audit content-ignorance)
///
/// `ArtifactPayload` is scheme-chosen, but schemes MUST keep it
/// content-ignorant for any audit-adjacent use — a parsed, structural
/// representation of the artifact, never raw document bytes.
pub trait SchemeArtifacts: MarkingScheme {
    /// The payload an artifact node carries when present — e.g. CAPCO's
    /// parsed CAB. Schemes that model document artifacts bind this to their
    /// own structural artifact type.
    type ArtifactPayload: Send + Sync;
}

/// Error variants returned by [`MarkingScheme::apply_intent`].
///
/// The engine dispatches on these so different failure modes get
/// different handling:
/// - [`IntentInapplicable`](Self::IntentInapplicable) — silent drop.
///   The marking is already consistent under the rule's invariant.
/// - [`UnknownToken`](Self::UnknownToken) — log + skip. Programmer
///   error in the rule's emission; the engine cannot route the
///   intent without a category mapping.
/// - [`IntentRejectsLattice`](Self::IntentRejectsLattice) — surface
///   as a diagnostic. The scheme refuses the fix because applying it
///   would violate a structural invariant.
/// - [`IntentNotYetApplicable`](Self::IntentNotYetApplicable) —
///   surface as a diagnostic. The intent is a reserved variant whose
///   apply path is not yet wired (currently `Relocate`; Phase E /
///   #824). Distinct from `IntentRejectsLattice` so a not-yet-wired
///   reserved variant is never silently dropped, silently applied, or
///   mislabeled as a true lattice rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyIntentError {
    /// The intent doesn't apply: `FactRemove` of a token that's
    /// absent, `FactAdd` of a token that's already present, or
    /// `Recanonicalize` on a marking that's already canonical. The
    /// engine drops the fix silently — the rule was pre-emptively
    /// right that the marking is already consistent.
    IntentInapplicable,
    /// A [`FactRef::Cve`](crate::FactRef::Cve)'s
    /// [`TokenId`](crate::TokenId) doesn't map to any category in
    /// this scheme. Programmer error in the rule. The engine logs
    /// and skips the fix.
    UnknownToken,
    /// Applying the intent would produce a marking that violates a
    /// structural invariant the scheme can't repair through fact-set
    /// delta alone. The engine surfaces this as a diagnostic.
    IntentRejectsLattice,
    /// The intent is a reserved [`ReplacementIntent`] variant —
    /// currently [`Relocate`](crate::ReplacementIntent::Relocate) —
    /// whose apply path is not yet wired (Phase E / #824). The engine
    /// surfaces this as a diagnostic rather than silently dropping or
    /// applying it. This is deliberately NOT
    /// [`IntentRejectsLattice`](Self::IntentRejectsLattice): the fix is
    /// not refused because it violates an invariant, it is refused
    /// because the cross-scope move semantics are not implemented yet.
    ///
    /// [`ReplacementIntent`]: crate::ReplacementIntent
    IntentNotYetApplicable,
}

impl fmt::Display for ApplyIntentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApplyIntentError::IntentInapplicable => {
                f.write_str("intent does not apply to this marking (already consistent)")
            }
            ApplyIntentError::UnknownToken => {
                f.write_str("token reference does not map to any category in this scheme")
            }
            ApplyIntentError::IntentRejectsLattice => {
                f.write_str("applying intent would violate a structural invariant of this scheme")
            }
            ApplyIntentError::IntentNotYetApplicable => {
                f.write_str("intent uses a reserved variant whose apply path is not yet wired")
            }
        }
    }
}

impl core::error::Error for ApplyIntentError {}
