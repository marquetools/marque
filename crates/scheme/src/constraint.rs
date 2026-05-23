// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Declarative constraints between tokens.
//!
//! A constraint is a declaration the scheme makes about its marking
//! type. Most CAPCO constraints are dyadic (NOFORN ∦ REL TO; HCS ⇒
//! NOFORN; RD ⇒ NOFORN by default) and land in the enumerated
//! variants — those are fully evaluable by a generic engine that only
//! knows how to check token/category presence.
//!
//! Some constraints are not dyadic (SIGMA compartments must appear in
//! numeric order; CNWDI requires classification ≥ S). Those land as
//! [`Constraint::Custom`] — a named, scheme-specific rule that the
//! scheme's own [`crate::MarkingScheme::validate`] implementation is
//! responsible for evaluating. `Constraint` is a `&[…]`-returned value;
//! schemes own the actual predicate logic privately.
//!
//! ## Conflict variant family
//!
//! [`Constraint::Conflicts`] covers exact pair conflicts. For cases where
//! one token conflicts with a whole family of tokens (e.g., RELIDO conflicts
//! with any FD&R dominator), [`Constraint::ConflictsWithFamily`] expresses
//! this as a [`FamilyPredicate`] over the right-hand side. The family form
//! is distributively equivalent to one `Conflicts` row per matching token
//! present in the marking.
//!
//! ## Citations (Constitution VIII)
//!
//! Every variant carries a `label: Citation` holding the typed
//! authoritative-source passage that defines the constraint (e.g.
//! `capco(SectionLetter::H, 4, 61)`). When a constraint fires,
//! [`ConstraintViolation::citation`] is populated by `Copy` from that
//! field so the triggering passage travels with the diagnostic. See
//! `crates/scheme/src/citation.rs` for the closed-template surface.
//!
//! The engine iterates `MarkingScheme::constraints()` after parsing /
//! joining to display the catalog; a full evaluator calls
//! `MarkingScheme::validate`, which dispatches dyadic variants
//! directly and routes `Custom` variants into the scheme's own
//! predicate.

use crate::category::{CategoryId, TokenId};
use crate::citation::Citation;
use crate::scheme::MarkingScheme;
use crate::severity::Severity;
use crate::span::Span;

/// Reference to a token or category in a constraint. Kept as a small
/// enum rather than a bare `TokenId` because some constraints are
/// expressed at category granularity (e.g., "no IC dissem with JOINT").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenRef {
    /// A specific token id.
    Token(TokenId),
    /// Any token in the named category.
    AnyInCategory(CategoryId),
}

/// A predicate over [`TokenRef`] used by [`Constraint::ConflictsWithFamily`]
/// to express family-shaped conflicts without enumerating every right-hand
/// side member.
///
/// Newtype wrapping `fn(&TokenRef) -> bool` per
/// `docs/plans/2026-05-13-pr3.7-lattice-resolution-gate-plan.md` §2
/// finding B2 — the bare `fn`-pointer doesn't implement [`Debug`], so we
/// wrap and manually implement it. `fn`-pointers are `Copy + Send + Sync +
/// 'static` by construction, so `FamilyPredicate` inherits those properties.
///
/// ## Closure captures are NOT supported
///
/// The inner pointer is a named `fn` item, not a closure. Closures with
/// captures would break the `'static` and `Send + Sync` guarantees. All
/// family predicates MUST be defined as named `pub fn` items, not closures.
///
/// ## Distributive expansion property
///
/// The family-predicate form is algebraically equivalent to one
/// [`Constraint::Conflicts`] row per token in the marking that matches the
/// predicate:
///
/// ```text
/// emit(ConflictsWithFamily(LHS, p)) =
///   union_{t in present_tokens(marking), p(t)} emit(Conflicts(LHS, Token(t)))
/// ```
///
/// Property tests in `crates/scheme/tests/proptest_constraint_rhs_family_distributive.rs`
/// verify this equivalence holds for any marking and predicate.
#[derive(Copy, Clone)]
pub struct FamilyPredicate(pub fn(&TokenRef) -> bool);

impl std::fmt::Debug for FamilyPredicate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FamilyPredicate(<fn>)")
    }
}

/// A declarative invariant the scheme enforces.
///
/// Every variant carries a stable `&'static str` `name` identifier
/// plus a typed [`Citation`] `label`:
///
/// - `name` — a **stable, scheme-unique short identifier** for the
///   constraint (e.g. `"capco/joint-conflicts-fgi"`). Surfaced as
///   [`ConstraintViolation::constraint_label`] so a downstream
///   consumer can trace a violation back to the specific declared
///   entry. Two `Conflicts` or `Requires` constraints in the same
///   catalog MUST have different `name`s; the evaluator does not
///   enforce uniqueness because the catalog is author-owned, but a
///   repeated `name` would collapse two logically distinct rules into
///   indistinguishable diagnostics.
/// - `label` — the typed authoritative-source citation (e.g.
///   `capco(SectionLetter::H, 4, 61)`). Shared across a catalog is
///   fine — many distinct rules may share a citation.
///
/// The five active variants are:
///
/// - [`Conflicts`](Self::Conflicts): exact pair conflict (LHS ∦ RHS).
/// - [`ConflictsWithFamily`](Self::ConflictsWithFamily): LHS conflicts
///   with any token in a family, using a [`FamilyPredicate`] for the
///   right-hand side. Distributively equivalent to one `Conflicts` row
///   per matching token in the marking.
/// - [`Requires`](Self::Requires): LHS requires RHS.
/// - [`Supersedes`](Self::Supersedes): LHS supersedes RHS at banner scope.
/// - [`Custom`](Self::Custom): scheme-specific n-ary predicate.
///
/// Note: there is no `Implies` variant. Fact-propagation is handled by the
/// closure operator ([`crate::closure::ClosureRule`] /
/// [`MarkingScheme::closure_rules`]), which runs before constraint
/// validation. After closure, implied facts are present in the marking
/// before `Requires` checks evaluate, so "missing X" false positives
/// disappear automatically.
///
/// See the module-level docs for the full rationale and Constitution
/// VIII for citation discipline.
///
/// [`MarkingScheme::closure_rules`]: crate::scheme::MarkingScheme::closure_rules
#[derive(Debug, Clone)]
pub enum Constraint {
    /// Two tokens cannot co-occur in one marking. Example: NOFORN and
    /// REL TO are mutually exclusive at the portion level.
    Conflicts {
        name: &'static str,
        left: TokenRef,
        right: TokenRef,
        label: Citation,
        /// The diagnostic severity to emit when the conflict fires.
        /// When `None`, the violation is advisory (no diagnostic).
        severity: Option<Severity>,
        /// Which side of the conflict to anchor the diagnostic span
        /// on. When `None`, defaults to the `left` token.
        span_anchor: Option<TokenRef>,
    },
    /// One token conflicts with an entire family of tokens, expressed
    /// via a [`FamilyPredicate`] over the right-hand side.
    ///
    /// This is the distributive-expansion form: at evaluation time,
    /// the evaluator walks the marking's present tokens via
    /// [`MarkingScheme::iter_present_tokens`], applies the predicate
    /// to each, and emits one [`ConstraintViolation`] per matching
    /// token that co-occurs with `left`. This is algebraically
    /// equivalent to one [`Constraint::Conflicts`] row per matching
    /// token (see [`FamilyPredicate`] doc for the formal equivalence).
    ///
    /// Example use case: RELIDO conflicts with any FD&R dominator —
    /// rather than enumerating each dominator as a separate `Conflicts`
    /// row, a single `ConflictsWithFamily` row with an `is_fdr_dominator`
    /// predicate compacts the catalog.
    ///
    /// [`MarkingScheme::iter_present_tokens`]: crate::scheme::MarkingScheme::iter_present_tokens
    ConflictsWithFamily {
        name: &'static str,
        left: TokenRef,
        family: FamilyPredicate,
        label: Citation,
        /// The diagnostic severity to emit when the conflict fires.
        /// When `None`, the violation is advisory (no diagnostic).
        severity: Option<Severity>,
    },
    /// If the left is present, the right must also be present.
    /// Example: HCS requires NOFORN.
    Requires {
        name: &'static str,
        left: TokenRef,
        right: TokenRef,
        label: Citation,
        /// The diagnostic severity to emit when the requirement is
        /// not satisfied. When `None`, the violation is advisory.
        severity: Option<Severity>,
    },
    /// The left supersedes the right during banner roll-up: the right
    /// drops out of the banner if the left is present. Example:
    /// NOFORN ⊐ REL TO at banner scope.
    Supersedes {
        name: &'static str,
        left: TokenRef,
        right: TokenRef,
        label: Citation,
    },
    /// A scheme-specific constraint identified by a stable `name`
    /// (what appears in diagnostics and the declared-constraint
    /// catalog) and backed by an authoritative-source `label`.
    ///
    /// The actual predicate lives inside the scheme's
    /// [`crate::MarkingScheme::validate`] implementation, which matches
    /// on `name` and runs the scheme-specific check. This is the escape
    /// hatch for n-ary rules that can't be expressed as a pair of token
    /// references — SIGMA must sort numerically, CNWDI requires
    /// classification ≥ S, JOINT participants must appear in REL TO,
    /// etc.
    ///
    /// Keeping the predicate out of the variant lets `Constraint` stay
    /// `'static` and returnable as `&[Constraint]`.
    Custom { name: &'static str, label: Citation },
}

impl Constraint {
    /// The stable short identifier for this constraint — the same
    /// string that lands in [`ConstraintViolation::constraint_label`]
    /// when the constraint fires.
    pub fn name(&self) -> &'static str {
        match self {
            Constraint::Conflicts { name, .. }
            | Constraint::ConflictsWithFamily { name, .. }
            | Constraint::Requires { name, .. }
            | Constraint::Supersedes { name, .. }
            | Constraint::Custom { name, .. } => name,
        }
    }

    /// The authoritative-source citation for this constraint (e.g.
    /// `capco(SectionLetter::H, 4, 61)`). Returned by [`Copy`] regardless
    /// of variant.
    pub fn label(&self) -> Citation {
        match self {
            Constraint::Conflicts { label, .. }
            | Constraint::ConflictsWithFamily { label, .. }
            | Constraint::Requires { label, .. }
            | Constraint::Supersedes { label, .. }
            | Constraint::Custom { label, .. } => *label,
        }
    }

    /// The fixed diagnostic severity for this constraint, when defined
    /// in the catalog. Dyadic variants populate this; `Custom`
    /// variants typically return `None` and let the predicate helper
    /// decide.
    pub fn severity(&self) -> Option<Severity> {
        match self {
            Constraint::Conflicts { severity, .. }
            | Constraint::ConflictsWithFamily { severity, .. }
            | Constraint::Requires { severity, .. } => *severity,
            Constraint::Supersedes { .. } | Constraint::Custom { .. } => None,
        }
    }
}

/// A constraint that fired against a marking.
///
/// `citation` holds the triggering constraint's authoritative-source
/// passage verbatim (Constitution VIII). `constraint_label` remains the
/// short rule identifier used in diagnostic messages and log output.
///
/// # Optional emission fields
///
/// `span` and `severity` are `Option`-typed so the scheme-side constraint
/// catalog can produce diagnostics that would otherwise require a walker
/// rule to decorate. The dyadic-constraint arms in [`evaluate`]
/// (Conflicts / ConflictsWithFamily / Requires / Supersedes) have no
/// natural span or severity from the constraint declaration itself and
/// emit `None` for both fields; downstream engine layers treat
/// `ConstraintViolation`s with `None` span or `None` severity as advisory
/// and do NOT surface them as user-facing diagnostics. Schemes that want
/// their `Custom` constraint catalog to be the end-state
/// diagnostic-emission path populate both fields from a catalog row.
///
/// (The `marque-scheme` crate is the workspace graph leaf and does not
/// depend on the higher-layer rule / diagnostic types — the precise
/// engine-side type the populated fields feed into is named in the
/// engine crate's bridge code, not here.)
///
/// This shape was chosen over required `Span` / `Severity` fields
/// specifically because (a) it preserves backwards compatibility with
/// the ~25 in-tree construction sites that emit dyadic violations with
/// no natural span/severity, and (b) it leaves room for a future
/// "advisory ConstraintViolation that doesn't become a Diagnostic"
/// surface (logs, audit-only signals, etc.) without inverting the
/// crate-graph rule that `marque-scheme` is the leaf.
#[derive(Debug, Clone)]
pub struct ConstraintViolation {
    pub constraint_label: &'static str,
    pub message: String,
    pub citation: Citation,
    /// Source-position anchor for the diagnostic, when the violation
    /// has a natural location in the input bytes. `None` when the
    /// violation is a whole-marking fact-set property with no single
    /// blameable token (e.g., the dyadic Conflicts/Requires arms in
    /// [`evaluate`] today).
    pub span: Option<Span>,
    /// Diagnostic severity, when the catalog row commits to a fixed
    /// severity. `None` when the violation is an advisory signal that
    /// the engine should treat as informational only (current default
    /// for the dyadic Conflicts/Requires arms).
    pub severity: Option<Severity>,
}

/// Walk a scheme's declarative constraints and emit one
/// [`ConstraintViolation`] per variant whose predicate fires against `marking`.
///
/// `evaluate` covers the active variants:
/// - [`Constraint::Conflicts`]: exact pair — fires when both tokens are present.
/// - [`Constraint::ConflictsWithFamily`]: family form — fires once per
///   present token that matches the [`FamilyPredicate`], when `left` is
///   present. Distributively equivalent to one `Conflicts` row per matching
///   present token (see [`FamilyPredicate`] and the property tests in
///   `crates/scheme/tests/proptest_constraint_rhs_family_distributive.rs`).
/// - [`Constraint::Requires`]: fires when `left` is present but `right` is
///   absent.
/// - [`Constraint::Supersedes`]: rewrite hint for banner roll-up, not a
///   violation trigger — silently skipped.
/// - [`Constraint::Custom`]: dispatched through
///   [`MarkingScheme::evaluate_custom`] so the scheme owns its bespoke
///   predicate bodies.
///
/// Note: there is no `Implies` variant. Fact-propagation is handled by the
/// closure operator ([`crate::closure::ClosureRule`] /
/// [`MarkingScheme::closure_rules`]), which runs before constraint
/// validation, eliminating false "missing X" positives automatically.
///
/// Contract:
/// - **Deterministic**: same input returns identical output on every
///   call, regardless of thread or iteration order.
/// - **Declaration-ordered**: the scheme's declared constraint order is
///   preserved in the returned violation vec.
/// - **Allocation-bounded** (per-variant): the dyadic arms (Conflicts /
///   Requires / Supersedes) allocate only the violation-message strings
///   they construct plus the returned `Vec`. The `ConflictsWithFamily`
///   arm additionally allocates the iterator returned by
///   `MarkingScheme::iter_present_tokens` (typically a boxed `Vec` of
///   present tokens — see `collect_present_tokens` in
///   `crates/capco/src/scheme/predicates/satisfies.rs`),
///   so its allocation cost is O(|present tokens|) per row. The `Custom`
///   arm delegates to `scheme.evaluate_custom`, whose allocation cost is
///   scheme-defined. The loop body itself does not allocate beyond the
///   returned `Vec`.
///
/// [`MarkingScheme::closure_rules`]: crate::scheme::MarkingScheme::closure_rules
pub fn evaluate<S>(scheme: &S, marking: &S::Marking) -> Vec<ConstraintViolation>
where
    S: MarkingScheme + ?Sized,
{
    let bits = scheme.precompute_bits(marking);
    let mut out = Vec::new();
    for c in scheme.constraints() {
        match c {
            Constraint::Conflicts {
                name,
                left,
                right,
                label,
                severity,
                span_anchor,
            } => {
                if scheme.satisfies(marking, left) && scheme.satisfies(marking, right) {
                    let anchor = span_anchor.as_ref().unwrap_or(left);
                    out.push(ConstraintViolation {
                        constraint_label: name,
                        message: format!("conflicting tokens: {left:?} and {right:?}"),
                        citation: *label,
                        span: scheme.token_span(marking, anchor),
                        severity: *severity,
                    });
                }
            }
            Constraint::ConflictsWithFamily {
                name,
                left,
                family,
                label,
                severity,
            } => {
                if scheme.satisfies(marking, left) {
                    for present_token in scheme.iter_present_tokens(marking) {
                        if family.0(&present_token) {
                            out.push(ConstraintViolation {
                                constraint_label: name,
                                message: format!(
                                    "conflicting tokens: {left:?} and {present_token:?} (family match)"
                                ),
                                citation: *label,
                                span: scheme.token_span(marking, left),
                                severity: *severity,
                            });
                        }
                    }
                }
            }
            Constraint::Requires {
                name,
                left,
                right,
                label,
                severity,
            } => {
                if scheme.satisfies(marking, left) && !scheme.satisfies(marking, right) {
                    out.push(ConstraintViolation {
                        constraint_label: name,
                        message: format!(
                            "missing required token: {right:?} (required by {left:?})"
                        ),
                        citation: *label,
                        span: scheme.token_span(marking, left),
                        severity: *severity,
                    });
                }
            }
            Constraint::Supersedes { .. } => {}
            Constraint::Custom { name, label } => {
                // Route to the scheme-specific predicate. The returned
                // violations carry the name and label of the Custom
                // row.
                for mut v in scheme.evaluate_custom(name, marking, bits) {
                    v.constraint_label = name;
                    v.citation = *label;
                    out.push(v);
                }
            }
        }
    }
    out
}
