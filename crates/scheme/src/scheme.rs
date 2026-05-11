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
use crate::category::{Category, CategoryId};
use crate::constraint::{Constraint, ConstraintViolation, TokenRef};
use crate::fix_intent::FactRef;
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
    /// Default implementation panics with `unimplemented!()` so schemes
    /// that do not yet support intent-based fix application still
    /// compile — but the panic surfaces at engine-fix time if a rule
    /// emits a [`FixIntent`] against the scheme, which is the correct
    /// fail-loud behavior pre-migration.
    ///
    /// [`ReplacementIntent`]: crate::ReplacementIntent
    /// [`FixIntent`]: crate::ReplacementIntent
    /// [`ApplyIntentError`]: ApplyIntentError
    fn category_of(&self, _token: &FactRef<Self>) -> Option<CategoryId>
    where
        Self: Sized,
    {
        unimplemented!(
            "category_of not supported by this scheme (PR 3c.B engine-prereq); \
             a rule emitted a FixIntent but the scheme has no token-to-category \
             routing table — implement category_of for this scheme."
        )
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
    /// - [`ApplyIntentError::IntentInapplicable`] — the intent doesn't
    ///   apply: `FactRemove` of an absent token, or `FactAdd` of a
    ///   present-and-already-canonical token. Engine drops the fix
    ///   silently — the marking is already consistent.
    /// - [`ApplyIntentError::UnknownToken`] — a [`FactRef::Cve`]'s
    ///   [`crate::TokenId`] doesn't map to any category. Programmer
    ///   error in the rule; engine logs and skips the fix.
    /// - [`ApplyIntentError::IntentRejectsLattice`] — applying would
    ///   produce a marking that violates a structural invariant the
    ///   scheme can't repair through fact-set delta alone. Engine
    ///   surfaces as a diagnostic.
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
    /// [`FixIntent`]: crate::ReplacementIntent
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
    /// Two markings that compare equal under [`Lattice`] equality
    /// MUST render to byte-identical output for the same `scope`.
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
