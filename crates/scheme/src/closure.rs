// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Closure rules and the §4.7 closure operator.
//!
//! A [`ClosureRule`] is a PUBLIC catalog primitive declaring an implication:
//! when `triggers` are present and `suppressors` are absent, add `cone`
//! facts to the marking. This is the algebraic closure operator from
//! `docs/plans/2026-05-01-lattice-design.md` §3 (e).
//!
//! The operator has three key properties that make it sound to apply
//! before constraint validation:
//!
//! 1. **Monotone**: adding facts never removes other facts — the operator
//!    can only move a marking "up" the lattice.
//! 2. **Extensive**: the output is always a superset of the input — closure
//!    can only add facts, not remove them.
//! 3. **Idempotent**: applying closure twice yields the same result as
//!    applying it once — the fixed point is stable.
//!
//! ## Default `closure()` behavior — PR 3.7 ships catalog only
//!
//! The default [`MarkingScheme::closure()`] implementation is a **no-op**
//! in PR 3.7: it returns the input marking unchanged. The trait default
//! cannot generically apply a closure rule's `cone` to `Self::Marking`
//! without a scheme-specific singleton-construction hook, so a scheme
//! that wants closure semantics MUST override `closure()` itself.
//!
//! [`MAX_CLOSURE_ITERATIONS`] is the iteration cap a scheme's
//! `closure()` override SHOULD respect for Kleene-fixpoint walks — see
//! the constant's doc comment. PR 3.7 ships [`ClosureRule`] data and
//! the trait scaffold; `CapcoScheme::closure()` override + engine
//! call-site wiring at `Engine::project` lands in PR 4 (T112).
//!
//! Until then, calling [`MarkingScheme::closure()`] on a scheme with
//! declared closure rules will silently return the input unchanged.
//! Production code paths in PR 3.7 do NOT call `closure()`; the
//! catalog is exposed as public data via [`MarkingScheme::closure_rules()`]
//! for tooling and proptest exercise only.
//!
//! ## Architecture note
//!
//! Closure rules live in `marque-scheme` (the workspace graph leaf), not
//! in `marque-rules` or `marque-engine`. This preserves Constitution VII's
//! crate discipline: the closure operator runs at the scheme layer, before
//! the engine's rule-evaluation loop. The engine wires the call site at
//! `Engine::project` (PR 4); this PR ships the trait surface and catalog
//! shape only.
//!
//! ## Public catalog vs. private `ImplTable<S>`
//!
//! Per `specs/006-engine-rule-refactor/decisions.md` D18, `ClosureRule` is
//! a PUBLIC catalog primitive — visible to tooling, scheme-exploration UIs,
//! and docs generators — not a private engine-implementation detail. This
//! is the "Option C" choice from the D18 design pass: closure rules are
//! first-class catalog data, parallel to [`crate::constraint::Constraint`]
//! declarations.

use crate::category::TokenId;
use crate::constraint::TokenRef;
use crate::severity::Severity;

/// A declarative closure rule: when `triggers` are present and `suppressors`
/// are absent, add `cone` facts to the marking.
///
/// Closure rules implement the §4.7 implicit-fact propagation operator from
/// `docs/plans/2026-05-01-lattice-design.md` §3 (e). They propagate facts
/// that CAPCO doesn't require to be written explicitly — for example, that
/// a marking with no explicit FD&R control implies NOFORN as the effective
/// release restriction.
///
/// The operator is monotone, extensive, and idempotent over the joint fact
/// lattice. Per `specs/006-engine-rule-refactor/decisions.md` D18, this is a
/// PUBLIC catalog primitive — not a private `ImplTable<S>` — so tooling can
/// inspect declared closure rules alongside [`crate::constraint::Constraint`]
/// declarations.
///
/// ## Trigger and suppressor semantics
///
/// - **Triggers**: N-ary OR over `triggers`. The rule fires when
///   [`MarkingScheme::satisfies`] returns `true` for ANY token in this slice.
///   An empty `triggers` slice means unconditional firing (the rule always
///   applies).
/// - **Suppressors**: N-ary OR over `suppressors`. The rule is suppressed when
///   [`MarkingScheme::satisfies`] returns `true` for ANY token in this slice.
///   An empty `suppressors` slice means no suppression (the rule can never
///   be suppressed).
///
/// ## Cone semantics
///
/// Each entry in `cone` is a [`TokenRef::Token`] or [`TokenRef::AnyInCategory`]
/// (the latter is reserved for future open-vocab cones; the current CAPCO
/// catalog uses `TokenRef::Token` exclusively). The scheme's
/// [`MarkingScheme::token_category()`] lookup routes each token to its host
/// category for addition.
///
/// ## Severity
///
/// `default_severity` expresses the catalog author's intent. Per
/// `decisions.md` D19 B, the typical default is [`Severity::Info`].
/// [`Severity::Fix`] is rejected at config load — closure firings propagate
/// facts, they are not byte-level fixes; see Stage F
/// (`ConfigError::InvalidClosureRuleSeverity`).
///
/// [`MarkingScheme::satisfies`]: crate::scheme::MarkingScheme::satisfies
/// [`MarkingScheme::token_category()`]: crate::scheme::MarkingScheme::token_category
#[derive(Debug, Clone)]
pub struct ClosureRule {
    /// Stable scheme-unique identifier (e.g., `"capco/noforn-if-no-fdr"`).
    ///
    /// Used as the catalog row key for `[closure_rules]` config overrides
    /// and `AuditNote.row_name` emission. Values containing `/` MUST be
    /// written as quoted TOML keys (e.g., `"capco/noforn-if-no-fdr" = "warn"`)
    /// because unquoted TOML keys do not allow slash characters.
    ///
    /// Every `ClosureRule` in a scheme's catalog MUST have a distinct `name`.
    pub name: &'static str,

    /// Authoritative-source citation passage (e.g., `"CAPCO-2016 §H.8 p145"`).
    ///
    /// Per Constitution VIII, citations MUST refer to a real passage in the
    /// authoritative source, accurately reflect what that passage says, and
    /// be re-verifiable by any reviewer with the source in hand. Carried
    /// through to `AuditNote.citation` when the rule fires.
    pub label: &'static str,

    /// N-ary OR over triggers.
    ///
    /// The rule fires when [`MarkingScheme::satisfies`] returns `true` for
    /// ANY trigger in this slice. An empty slice means unconditional firing.
    pub triggers: &'static [TokenRef],

    /// N-ary OR over suppressors.
    ///
    /// The rule is suppressed when [`MarkingScheme::satisfies`] returns `true`
    /// for ANY suppressor in this slice. An empty slice means no suppression.
    pub suppressors: &'static [TokenRef],

    /// Facts added by this rule when `triggers ∧ ¬suppressors`.
    ///
    /// Each entry is a [`TokenRef::Token`] or [`TokenRef::AnyInCategory`]
    /// (the latter reserved for future open-vocab cones; the current CAPCO
    /// catalog uses `TokenRef::Token` exclusively). Each token in `cone` is
    /// routed to its host category via [`MarkingScheme::token_category`].
    ///
    /// [`MarkingScheme::token_category`]: crate::scheme::MarkingScheme::token_category
    pub cone: &'static [TokenRef],

    /// Catalog-author severity intent.
    ///
    /// Per `decisions.md` D19 B, the typical value is [`Severity::Info`].
    /// [`Severity::Fix`] is rejected at config load — closure firings
    /// propagate facts, they are not byte-level fixes.
    pub default_severity: Severity,
}

impl ClosureRule {
    /// Returns `true` if the trigger condition is met for the given marking.
    ///
    /// The trigger condition is N-ary OR: true when ANY trigger in
    /// `self.triggers` satisfies the marking, OR when `triggers` is empty
    /// (unconditional firing).
    #[inline]
    pub fn trigger_fires<S>(&self, scheme: &S, marking: &S::Marking) -> bool
    where
        S: crate::scheme::MarkingScheme,
    {
        if self.triggers.is_empty() {
            return true;
        }
        self.triggers.iter().any(|t| scheme.satisfies(marking, t))
    }

    /// Returns `true` if the suppressor condition is met for the given marking,
    /// meaning this rule should NOT fire.
    ///
    /// The suppressor condition is N-ary OR: true when ANY suppressor in
    /// `self.suppressors` satisfies the marking. False (not suppressed) when
    /// `suppressors` is empty.
    #[inline]
    pub fn is_suppressed<S>(&self, scheme: &S, marking: &S::Marking) -> bool
    where
        S: crate::scheme::MarkingScheme,
    {
        self.suppressors
            .iter()
            .any(|s| scheme.satisfies(marking, s))
    }

    /// Returns `true` if this rule's structural firing condition is met:
    /// the trigger fires AND no suppressor matches.
    ///
    /// **Does NOT consult severity.** Per `decisions.md` D19 B, severity
    /// is **runtime-resolved**: a closure-operator implementation reads
    /// the `[closure_rules]` config table first (per-row overrides from
    /// `.marque.toml` / `MARQUE_CLOSURE_RULES_*` env vars) and falls
    /// back to `ClosureRule.default_severity` only when no override
    /// exists. A row declared `default_severity: Severity::Off` in the
    /// catalog can still be re-enabled by user configuration; baking
    /// the catalog default into `should_fire` would make user override
    /// impossible.
    ///
    /// Callers integrating with the engine should evaluate the runtime-
    /// resolved severity separately before applying the cone. The
    /// trait-level `MarkingScheme::closure()` impl is where the
    /// severity-gating policy lives (PR 4 wires `Engine::project`
    /// through a config-aware closure pass).
    ///
    /// (An earlier PR 3.7 rev briefly added a `default_severity == Off`
    /// short-circuit here to make "dormant placeholder rows" inert.
    /// Copilot PR 3.7 review pass 4 flagged that this contradicted
    /// D19 B's runtime-resolved-severity contract; the short-circuit
    /// was reverted and the placeholder rows were removed from
    /// `CapcoScheme::closure_rules()` entirely.)
    #[inline]
    pub fn should_fire<S>(&self, scheme: &S, marking: &S::Marking) -> bool
    where
        S: crate::scheme::MarkingScheme,
    {
        self.trigger_fires(scheme, marking) && !self.is_suppressed(scheme, marking)
    }

    /// Returns only the `TokenId` entries from `cone` (skipping
    /// `AnyInCategory` entries, which are open-vocab cones handled
    /// separately by the scheme's override).
    pub fn cone_token_ids(&self) -> impl Iterator<Item = TokenId> + '_ {
        self.cone.iter().filter_map(|tr| match tr {
            TokenRef::Token(id) => Some(*id),
            TokenRef::AnyInCategory(_) => None,
        })
    }
}

/// Maximum Kleene-fixpoint iterations for the closure operator.
///
/// Per `docs/plans/2026-05-01-lattice-design.md` §4.7.3 table-design
/// property and the chain-depth walk in
/// `docs/plans/2026-05-13-pr3.7-lattice-resolution-gate-plan.md` §9 OQ #4:
/// the maximum observed chain depth in the CAPCO catalog is 3 (per-marking
/// implication → trio suppressor → trio re-evaluation → fixed point).
/// `N=16` is 5× safety padding over that observed maximum.
///
/// A fixpoint that does NOT converge within `N=16` iterations causes a
/// panic at runtime. Non-termination of the closure operator is a
/// catalog correctness failure — monotone catalogs always reach a
/// fixpoint in at most `|fact_universe|` iterations, which is bounded
/// for any finite scheme.
///
/// The iteration cap is a **non-convergence guard**, not a
/// monotonicity oracle: a non-monotone catalog with a suppressor
/// depending on a fact in another rule's cone can converge quickly to
/// a fixed point while still violating
/// `m1 ⊑ m2 ⇒ closure(m1) ⊑ closure(m2)`. The companion proptest at
/// `crates/scheme/tests/proptest_closure_rejects_non_monotone.rs`
/// exercises that observable violation directly; the cap here only
/// catches the unbounded-growth failure mode.
pub const MAX_CLOSURE_ITERATIONS: usize = 16;
