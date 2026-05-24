// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Closure rules and the closure operator.
//!
//! A [`ClosureRule`] is a PUBLIC catalog primitive declaring an implication:
//! when `triggers` are present and `suppressors` are absent, add `cone`
//! facts to the marking.
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
//! ## Default `closure()` behavior
//!
//! The default [`MarkingScheme::closure()`] implementation is a **no-op**:
//! it returns the input marking unchanged. The trait default cannot
//! generically apply a closure rule's `cone` to `Self::Marking` without a
//! scheme-specific singleton-construction hook, so a scheme that wants
//! closure semantics MUST override `closure()` itself.
//!
//! [`MAX_CLOSURE_ITERATIONS`] is the iteration cap a scheme's
//! `closure()` override SHOULD respect for Kleene-fixpoint walks — see
//! the constant's doc comment.
//!
//! ## Architecture note
//!
//! Closure rules live in `marque-scheme` (the workspace graph leaf), not
//! in `marque-rules` or `marque-engine`. This preserves Constitution VII's
//! crate discipline: the closure operator runs at the scheme layer, before
//! the engine's rule-evaluation loop. The engine wires the call site at
//! `Engine::project`.
//!
//! ## Public catalog vs. private `ImplTable<S>`
//!
//! `ClosureRule` is a PUBLIC catalog primitive — visible to tooling,
//! scheme-exploration UIs, and docs generators — not a private
//! engine-implementation detail. Closure rules are first-class catalog
//! data, parallel to [`crate::constraint::Constraint`] declarations.

use crate::category::TokenId;
use crate::citation::Citation;
use crate::constraint::TokenRef;
use crate::severity::Severity;

/// Type alias for [`ClosureRule::cone_derived`] — silences `clippy::type_complexity`.
///
/// Returns a `SmallVec` of [`FactRef<S>`] values; the executor routes each
/// fact to its host category via [`MarkingScheme::category_of`]. See
/// [`ClosureRule::cone_derived`] for the contract on monotonicity and the
/// closed-vs-open-vocab rationale.
///
/// The `?Sized` bound matches [`FactRef<S>`]'s bound at
/// [`crate::fix_intent::FactRef`] so the alias is well-formed for
/// `ClosureRule<S: MarkingScheme + ?Sized>`.
///
/// [`FactRef<S>`]: crate::fix_intent::FactRef
/// [`MarkingScheme::category_of`]: crate::scheme::MarkingScheme::category_of
#[allow(type_alias_bounds)]
pub type ConeDerivedFn<S: crate::scheme::MarkingScheme + ?Sized> =
    fn(
        &<S as crate::scheme::MarkingScheme>::Marking,
    ) -> smallvec::SmallVec<[crate::fix_intent::FactRef<S>; 2]>;

/// Scheme-agnostic closure-rule inventory metadata.
///
/// This surface is intended for discovery/config consumers that need stable
/// row identity + presentation data without executable trigger/suppressor/cone
/// bodies.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ClosureRuleMetadata {
    /// Stable scheme-unique row identifier.
    pub name: &'static str,
    /// Human-readable row label.
    pub label: &'static str,
    /// Optional authoritative citation payload. `None` for rows without
    /// a structured §-citation.
    pub citation: Option<Citation>,
    /// Catalog default severity intent.
    pub default_severity: Severity,
}

/// A declarative closure rule: when `triggers` are present and `suppressors`
/// are absent, add `cone` facts to the marking.
///
/// Closure rules implement implicit-fact propagation. They propagate facts
/// that CAPCO doesn't require to be written explicitly — for example, that
/// a marking with no explicit FD&R control implies NOFORN as the effective
/// release restriction.
///
/// The operator is monotone, extensive, and idempotent over the joint fact
/// lattice. This is a PUBLIC catalog primitive — not a private
/// `ImplTable<S>` — so tooling can inspect declared closure rules alongside
/// [`crate::constraint::Constraint`] declarations.
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
/// Each entry in `cone` is a [`TokenRef::Token`]; the current CAPCO catalog
/// uses `TokenRef::Token` exclusively. [`TokenRef::AnyInCategory`] is a
/// category-wildcard *predicate* — useful in `triggers` and `suppressors` to
/// match "any token in this category" — and is NOT the carrier for open-
/// vocab cone facts. Open-vocab facts (REL TO country codes, FGI
/// tetragraphs, SAR program identifiers, etc.) are emitted through
/// `cone_derived` returning [`FactRef::OpenVocab`] values; see the
/// `cone_derived` field below for the rationale and contract. The scheme's
/// [`MarkingScheme::token_category()`] lookup routes each `TokenRef::Token`
/// entry to its host category for addition.
///
/// ## Severity
///
/// `default_severity` expresses the catalog author's intent. The typical
/// default is [`Severity::Info`]. [`Severity::Fix`] is rejected at config
/// load (`ConfigError::InvalidClosureRuleSeverity`) — closure firings
/// propagate facts, they are not byte-level fixes.
///
/// [`MarkingScheme::satisfies`]: crate::scheme::MarkingScheme::satisfies
/// [`MarkingScheme::token_category()`]: crate::scheme::MarkingScheme::token_category
pub struct ClosureRule<S: crate::scheme::MarkingScheme + ?Sized> {
    /// Stable scheme-unique identifier in the wire-string form
    /// (e.g., `"capco:closure.dissem.noforn-if-caveated"`).
    ///
    /// Used as the catalog row key for `[closure_rules]` config overrides
    /// and `AuditNote.row_name` emission. Values containing `:` or `.`
    /// MUST be written as quoted TOML keys (e.g.,
    /// `"capco:closure.dissem.noforn-if-caveated" = "warn"`) because
    /// unquoted TOML keys do not allow those characters.
    ///
    /// Every `ClosureRule` in a scheme's catalog MUST have a distinct `name`.
    pub name: &'static str,

    /// Human-readable display label for inventory surfaces.
    pub display_label: &'static str,

    /// Typed authoritative-source citation (e.g., `capco(SectionLetter::H, 8, 145)`).
    ///
    /// Per Constitution VIII, citations MUST refer to a real passage in the
    /// authoritative source, accurately reflect what that passage says, and
    /// be re-verifiable by any reviewer with the source in hand. Carried
    /// through to `AuditNote.citation` when the rule fires.
    pub label: Citation,

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
    /// Each entry is a [`TokenRef::Token`]; the current CAPCO catalog uses
    /// `TokenRef::Token` exclusively. [`TokenRef::AnyInCategory`] is a
    /// category-wildcard predicate (intended for `triggers` / `suppressors`
    /// matching "any token in this category") and is NOT the carrier for
    /// open-vocab cone facts. Open-vocab facts go through [`Self::cone_derived`]
    /// returning [`FactRef::OpenVocab`] values — see that field's docs for
    /// the rationale. Each token in `cone` is routed to its host category
    /// via [`MarkingScheme::token_category`].
    ///
    /// [`FactRef::OpenVocab`]: crate::fix_intent::FactRef::OpenVocab
    /// [`MarkingScheme::token_category`]: crate::scheme::MarkingScheme::token_category
    pub cone: &'static [TokenRef],

    /// Optional marking-derived cone facts — supplements the static `cone` field.
    ///
    /// When `Some(f)`, the closure executor evaluates `f(marking)` after the
    /// static `cone` facts and adds each [`FactRef<S>`] as an additional fact
    /// in the scheme's marking. Each returned `FactRef` is routed to its host
    /// category via [`MarkingScheme::category_of`] — the same dispatch the
    /// engine uses for any other `FactRef` mutation — so the derived path is
    /// symmetric with the static path: both ask the scheme to route, neither
    /// pre-binds a `CategoryId`.
    ///
    /// # Why `FactRef<S>` and not `TokenRef`
    ///
    /// [`TokenRef`] carries only closed `TokenId` values and an axis-level
    /// `AnyInCategory` predicate; it cannot express open-vocabulary facts
    /// like REL TO country codes, FGI tetragraphs, or SAR program identifiers.
    /// The motivating JOINT use case — `REL TO USA, GBR, JPN`
    /// partner-list cone — needs `FactRef::OpenVocab(CapcoOpenVocabRef::CountryCode(_))`,
    /// which is the established open-vocab carrier in CAPCO
    /// (`CapcoOpenVocabRef::CountryCode`). [`FactRef<S>`]'s
    /// `Cve` / `OpenVocab` split covers both closed and open vocab uniformly,
    /// and the scheme's `category_of` impl owns the routing.
    ///
    /// # Contracts the function MUST satisfy
    ///
    /// **Monotonicity**: if `m1 ⊑ m2` in the marking lattice, then folding
    /// `f(m1)` into `m1` via the host categories' joins produces a result
    /// `⊑` folding `f(m2)` into `m2` via the same joins. Set inclusion of
    /// the returned [`FactRef<S>`] list (`f(m1) ⊆ f(m2)` as sets of
    /// `FactRef<S>` values) is sufficient when every host category's join is
    /// set-union — the static catalog's case. Categories whose join
    /// transmutes variants — notably `JointSet` — need the property stated
    /// on the join, not on the emitted set: equal emitted sets can join to
    /// distinct markings, so `⊆` on the raw output is necessary but not
    /// sufficient. Without monotonicity on the join, the closure operator
    /// loses monotonicity globally and the fixpoint iteration's
    /// correctness guarantee fails. For static rows the cone-producing
    /// function is constant — vacuously monotone in `m` — but the
    /// rule-as-a-whole still requires the suppressor/redundancy
    /// monotonicity attestation: adding facts to a marking MUST NOT
    /// unmask a suppressor and silence a row that previously fired.
    /// Derived rows owe both attestations: the cone-producing function
    /// itself must be monotone in `S::Marking`'s join order, AND the
    /// same suppressor monotonicity property must hold. The
    /// `NonMonotoneScheme` fixture in
    /// `crates/scheme/tests/proptest_closure_rejects_non_monotone.rs`
    /// shows the failure mode for a static rule whose suppressor flips
    /// from inactive to active as facts accrete — observationally
    /// non-monotone at the rule level even though the static cone
    /// itself is constant.
    ///
    /// # See also: JOINT JointSet hazard
    ///
    /// `JointSet::join` collapses `UnanimousProducers{A} ⊔ UnanimousProducers{B}`
    /// (where `A ≠ B`) into `DisunityCollapse{union_non_us_producers}` — a
    /// *different variant* that strips USA. A future JOINT `cone_derived` that
    /// reads partner countries directly from `JointSet` and emits one fact per
    /// country can produce a *smaller* country set on `m2 ⊒ m1` than on `m1`
    /// (because the variant change drops USA from the underlying set). A
    /// JOINT-row author should design around this — likely by reading
    /// the post-join normalized form, not the raw producer list.
    ///
    /// # SmallVec inline cap
    ///
    /// The inline-2 cap matches the `ReplacementIntent::FactRemove::facts`
    /// precedent (issue #348). JOINT partner lists of 1-2 countries fit
    /// inline; lists of 3+ (which §H.3 worked examples do include) spill to
    /// the heap. The cap is intentionally aligned with the existing FactRemove
    /// precedent rather than widened speculatively — bump to inline-4 or
    /// inline-8 once a concrete row demonstrates the spill is hot.
    ///
    /// [`FactRef<S>`]: crate::fix_intent::FactRef
    /// [`MarkingScheme::category_of`]: crate::scheme::MarkingScheme::category_of
    pub cone_derived: Option<ConeDerivedFn<S>>,

    /// Catalog-author severity intent.
    ///
    /// The typical value is [`Severity::Info`]. [`Severity::Fix`] is
    /// rejected at config load — closure firings propagate facts, they are
    /// not byte-level fixes.
    pub default_severity: Severity,
}

// Manual `Debug` and `Clone` impls — mirror the `FactRef<S>` pattern at
// `crate::fix_intent::FactRef`'s `impl Debug` / `impl Clone` so the trait
// bounds resolve through the struct's fields without over-constraining on
// `S: Debug` or `S: Clone`. `CapcoScheme: !Clone`, so a `#[derive(Clone)]`
// here would silently prevent any `ClosureRule<CapcoScheme>` from being
// cloned even though every concrete field is `Copy`.
impl<S: crate::scheme::MarkingScheme + ?Sized> core::fmt::Debug for ClosureRule<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ClosureRule")
            .field("name", &self.name)
            .field("display_label", &self.display_label)
            .field("label", &self.label)
            .field("triggers", &self.triggers)
            .field("suppressors", &self.suppressors)
            .field("cone", &self.cone)
            .field("cone_derived", &self.cone_derived.map(|_| "<fn>"))
            .field("default_severity", &self.default_severity)
            .finish()
    }
}

impl<S: crate::scheme::MarkingScheme + ?Sized> Clone for ClosureRule<S> {
    fn clone(&self) -> Self {
        Self {
            name: self.name,
            display_label: self.display_label,
            label: self.label,
            triggers: self.triggers,
            suppressors: self.suppressors,
            cone: self.cone,
            cone_derived: self.cone_derived,
            default_severity: self.default_severity,
        }
    }
}

impl<S: crate::scheme::MarkingScheme + ?Sized> From<&ClosureRule<S>> for ClosureRuleMetadata {
    fn from(rule: &ClosureRule<S>) -> Self {
        Self {
            name: rule.name,
            // Inventory metadata uses the dedicated display label for UX, while
            // carrying the authoritative-source citation separately.
            label: rule.display_label,
            citation: Some(rule.label),
            default_severity: rule.default_severity,
        }
    }
}

impl<S: crate::scheme::MarkingScheme + ?Sized> ClosureRule<S> {
    /// Returns `true` if the trigger condition is met for the given marking.
    ///
    /// The trigger condition is N-ary OR: true when ANY trigger in
    /// `self.triggers` satisfies the marking, OR when `triggers` is empty
    /// (unconditional firing).
    #[inline]
    pub fn trigger_fires(&self, scheme: &S, marking: &S::Marking) -> bool {
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
    pub fn is_suppressed(&self, scheme: &S, marking: &S::Marking) -> bool {
        self.suppressors
            .iter()
            .any(|s| scheme.satisfies(marking, s))
    }

    /// Returns `true` if this rule's structural firing condition is met:
    /// the trigger fires AND no suppressor matches.
    ///
    /// **Does NOT consult severity.** Severity is **runtime-resolved**: a
    /// closure-operator implementation reads the `[closure_rules]` config
    /// table first (per-row overrides from `.marque.toml` /
    /// `MARQUE_CLOSURE_RULES_*` env vars) and falls back to
    /// `ClosureRule.default_severity` only when no override exists. A row
    /// declared `default_severity: Severity::Off` in the catalog can still
    /// be re-enabled by user configuration; baking the catalog default into
    /// `should_fire` would make user override impossible.
    ///
    /// Callers integrating with the engine should evaluate the runtime-
    /// resolved severity separately before applying the cone. The
    /// trait-level `MarkingScheme::closure()` impl is where the
    /// severity-gating policy lives (`Engine::project` runs a config-aware
    /// closure pass).
    #[inline]
    pub fn should_fire(&self, scheme: &S, marking: &S::Marking) -> bool {
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
/// The maximum observed chain depth in the CAPCO catalog is 3 (per-marking
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
///
/// # Derived-cone catalogs require per-scheme chain-depth re-verification
///
/// The `N=16` bound is calibrated against the CAPCO catalog's static
/// cones (chain depth 3, 5× safety padding). [`ClosureRule::cone_derived`]
/// permits marking-derived facts whose per-firing fact-count and inter-rule
/// chaining behavior are scheme-specific. A scheme that wires a
/// `cone_derived` row whose firing produces facts that re-trigger other
/// rows MUST re-do the chain-depth analysis against its own catalog before
/// relying on the `N=16` bound; the existing cap was calibrated against
/// static catalogs only.
pub const MAX_CLOSURE_ITERATIONS: usize = 16;
