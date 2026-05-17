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

use core::fmt;
use core::fmt::Debug;
use core::hash::Hash;

use crate::ambiguity::Parsed;
use crate::category::{Category, CategoryId, TokenId};
use crate::closure::ClosureRule;
use crate::constraint::{Constraint, ConstraintViolation, TokenRef};
use crate::fix_intent::FactRef;
use crate::lattice::JoinSemilattice;
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

    /// The scheme's full-marking type. Must be a join-semilattice: the join
    /// is the product over the scheme's categories. Doubly-lawful schemes
    /// (where every category satisfies meet too) automatically satisfy
    /// [`Lattice`](crate::lattice::Lattice) via the blanket impl in the
    /// [`crate::lattice`] module.
    type Marking: JoinSemilattice;

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
    /// `&'src str` into the input buffer — that would re-introduce a
    /// G13 leak channel).
    ///
    /// Schemes with no open-vocab axes bind this to
    /// `std::convert::Infallible`, which carries no runtime values and
    /// makes `FactRef::OpenVocab(...)` statically unreachable for that
    /// scheme.
    type OpenVocabRef: Debug + Clone + Eq + Hash + Send + Sync + 'static;

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
    /// See `specs/006-engine-rule-refactor/architecture.md` "What
    /// fixes are" for the architectural rationale.
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
            "apply_intent not supported by this scheme (PR 3c.B engine-prereq); \
             a rule emitted a FixIntent but the scheme has no intent-application \
             routing — implement apply_intent for this scheme."
        )
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

    /// Declared closure rules for this scheme.
    ///
    /// Closure rules implement the §4.7 implicit-fact propagation operator
    /// from `docs/plans/2026-05-01-lattice-design.md` §3 (e). They
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
    /// `should_fire` directly without applying the cone. This is the
    /// mode `CapcoScheme` ships in PR 3.7: the catalog is published
    /// PUBLIC inspection surface (D18); the `closure()` override that
    /// applies the cone at runtime lands in PR 4 alongside
    /// `Engine::project::closure()` wiring.
    ///
    /// Per `specs/006-engine-rule-refactor/decisions.md` D18, this is a
    /// PUBLIC catalog surface — visible to tooling, scheme-exploration UIs,
    /// and docs generators — not a private engine detail.
    ///
    /// Default: empty slice (no closure rules declared).
    fn closure_rules(&self) -> &[ClosureRule<Self>]
    where
        Self: Sized,
    {
        &[]
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
    /// Per `docs/plans/2026-05-13-pr3.7-lattice-resolution-gate-plan.md`
    /// §2 finding F1: this method was added specifically to resolve the
    /// scheme-layer / rules-layer boundary constraint on the closure
    /// operator's token routing.
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
    /// An earlier rev of this contract restricted the output to
    /// `TokenRef::Token` only; that restriction was lifted in PR 3.7
    /// rev 3 to align with CapcoScheme's actual emission (which
    /// emits `AnyInCategory(CAT_REL_TO)`, `CAT_SCI`, `CAT_SAR`, and
    /// `CAT_NON_US_CLASSIFICATION` for axis-level facts) and the
    /// family-predicate idiom that depends on it. Per Copilot
    /// PR 3.7 review pass 3.
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

    /// Render a marking in canonical form for the given `scope`,
    /// writing the bytes through `out`.
    ///
    /// This is the **single source of truth for canonical form** in
    /// the scheme. Per `specs/006-engine-rule-refactor/architecture.md`
    /// "What this commits us to":
    ///
    /// > The renderer (`render_canonical`) is the single source of
    /// > canonical form. Form rules retire into it.
    ///
    /// # Lattice-equal-byte-identical property
    ///
    /// Two markings that compare equal under [`Lattice`](crate::lattice::Lattice)
    /// equality MUST render to byte-identical output for the same `scope`.
    /// This is what makes `Recanonicalize` a sound fix: the renderer
    /// is referentially transparent over lattice-equivalent inputs,
    /// and the engine can therefore re-render a `ProjectedMarking`
    /// without consulting the input bytes that produced it.
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
    /// Implementations MUST honor the following per-scope contract.
    /// The default impls of [`Self::render_portion`] /
    /// [`Self::render_banner`] rely on this contract being upheld;
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
    /// `render_canonical(&projection.marking, scope.into(), &mut writer)`.
    /// Rules NEVER carry the `ProjectedMarking` — the engine is the
    /// authority. See `ReplacementIntent::Recanonicalize` in
    /// `marque-rules` for the intent-side surface.
    fn render_canonical(
        &self,
        m: &Self::Marking,
        scope: crate::scope::Scope,
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
    fn render_portion(&self, m: &Self::Marking) -> String {
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
        let result = self.render_canonical(m, crate::scope::Scope::Portion, &mut s);
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
    /// [`Self::render_portion`].
    ///
    /// # Note on scope naming
    ///
    /// The argument is [`crate::scope::Scope::Page`], not a hypothetical
    /// `Scope::Banner` — banner roll-up is *defined* as page-scope
    /// rendering in the architecture spec ("banner = lattice join over
    /// the page's portions"). The method name `render_banner` is the
    /// public API surface; the underlying scope is `Page`. Schemes that
    /// override this method MUST honor the same scope semantics.
    fn render_banner(&self, m: &Self::Marking) -> String {
        let mut s = String::new();
        let result = self.render_canonical(m, crate::scope::Scope::Page, &mut s);
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
        }
    }
}

impl core::error::Error for ApplyIntentError {}
