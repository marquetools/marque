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
//! ## Citations (FR-021, Constitution VIII)
//!
//! Every variant carries a `label: &'static str` holding the
//! authoritative-source passage that defines the constraint
//! (e.g. `"CAPCO-2016 §H.4"`). When a constraint fires,
//! [`ConstraintViolation::citation`] is populated verbatim from that
//! field so the triggering passage travels with the diagnostic.
//!
//! The engine iterates `MarkingScheme::constraints()` after parsing /
//! joining to display the catalog; a full evaluator calls
//! `MarkingScheme::validate`, which dispatches dyadic variants
//! directly and routes `Custom` variants into the scheme's own
//! predicate.

use crate::category::{CategoryId, TokenId};
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

/// A declarative invariant the scheme enforces.
///
/// Every variant carries two `&'static str` identifiers:
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
/// - `label` — the authoritative-source citation passage (e.g.
///   `"CAPCO-2016 §H.4"`). Shared across a catalog is fine — many
///   distinct rules may share a citation.
///
/// See the module-level docs for the full rationale and Constitution
/// VIII for citation discipline.
#[derive(Debug, Clone)]
pub enum Constraint {
    /// Two tokens cannot co-occur in one marking. Example: NOFORN and
    /// REL TO are mutually exclusive at the portion level.
    Conflicts {
        name: &'static str,
        left: TokenRef,
        right: TokenRef,
        label: &'static str,
    },
    /// If the left is present, the right must also be present.
    /// Example: HCS requires NOFORN.
    Requires {
        name: &'static str,
        left: TokenRef,
        right: TokenRef,
        label: &'static str,
    },
    /// If the left is present, the right is implied (safe to omit).
    /// The engine uses this to avoid false "missing X" diagnostics.
    Implies {
        name: &'static str,
        left: TokenRef,
        right: TokenRef,
        label: &'static str,
    },
    /// The left supersedes the right during banner roll-up: the right
    /// drops out of the banner if the left is present. Example:
    /// NOFORN ⊐ REL TO at banner scope.
    Supersedes {
        name: &'static str,
        left: TokenRef,
        right: TokenRef,
        label: &'static str,
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
    Custom {
        name: &'static str,
        label: &'static str,
    },
}

impl Constraint {
    /// The stable short identifier for this constraint — the same
    /// string that lands in [`ConstraintViolation::constraint_label`]
    /// when the constraint fires.
    pub fn name(&self) -> &'static str {
        match self {
            Constraint::Conflicts { name, .. }
            | Constraint::Requires { name, .. }
            | Constraint::Implies { name, .. }
            | Constraint::Supersedes { name, .. }
            | Constraint::Custom { name, .. } => name,
        }
    }

    /// The authoritative-source citation for this constraint (e.g.
    /// `"CAPCO-2016 §H.4"`). Returned unchanged regardless of variant.
    pub fn label(&self) -> &'static str {
        match self {
            Constraint::Conflicts { label, .. }
            | Constraint::Requires { label, .. }
            | Constraint::Implies { label, .. }
            | Constraint::Supersedes { label, .. }
            | Constraint::Custom { label, .. } => label,
        }
    }
}

/// A constraint that fired against a marking.
///
/// `citation` holds the triggering constraint's authoritative-source
/// passage verbatim (FR-021 + Constitution VIII). `constraint_label`
/// remains the short rule identifier used in diagnostic messages and
/// log output.
///
/// # Optional emission fields
///
/// `span` and `severity` are `Option`-typed and were added in PR 3c.B
/// Commit 7.1 to let the scheme-side constraint catalog produce
/// diagnostics that today require a walker rule to decorate. The
/// dyadic-constraint arms in [`evaluate`] (Conflicts/Requires/Implies/
/// Supersedes) have no natural span or severity from the constraint
/// declaration itself and continue to emit `None` for both fields; the
/// engine treats `ConstraintViolation`s with `None` span or `None`
/// severity as advisory and does NOT surface them as
/// [`marque_rules::Diagnostic`]s. Schemes that want their `Custom`
/// constraint catalog to be the end-state emission path (post PR 3c.B
/// Commit 7) populate both fields from a catalog row.
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
    pub citation: &'static str,
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
/// [`ConstraintViolation`] per dyadic variant whose predicate fires
/// against `marking`.
///
/// `evaluate` covers the four dyadic variants (`Conflicts`, `Requires`,
/// `Implies`, `Supersedes`) by asking the scheme to resolve each
/// [`TokenRef`] via [`MarkingScheme::satisfies`]. The `Custom` variant
/// is dispatched through [`MarkingScheme::evaluate_custom`] so the
/// scheme owns its bespoke predicate bodies.
///
/// Contract (FR-007, Phase 3 US1):
/// - **Deterministic**: same input returns identical output on every
///   call, regardless of thread or iteration order.
/// - **Declaration-ordered**: the scheme's declared constraint order is
///   preserved in the returned violation vec.
/// - **Allocation-bounded**: the only heap allocations come from the
///   returned `Vec` and the violation-message strings each variant
///   constructs. The loop itself does not allocate.
pub fn evaluate<S>(scheme: &S, marking: &S::Marking) -> Vec<ConstraintViolation>
where
    S: MarkingScheme + ?Sized,
{
    let mut out = Vec::new();
    for c in scheme.constraints() {
        match c {
            Constraint::Conflicts {
                name,
                left,
                right,
                label,
            } => {
                if scheme.satisfies(marking, left) && scheme.satisfies(marking, right) {
                    out.push(ConstraintViolation {
                        constraint_label: name,
                        message: format!("conflicting tokens: {left:?} and {right:?}"),
                        citation: label,
                        span: None,
                        severity: None,
                    });
                }
            }
            Constraint::Requires {
                name,
                left,
                right,
                label,
            } => {
                if scheme.satisfies(marking, left) && !scheme.satisfies(marking, right) {
                    out.push(ConstraintViolation {
                        constraint_label: name,
                        message: format!("token {left:?} requires {right:?} but it is missing"),
                        citation: label,
                        span: None,
                        severity: None,
                    });
                }
            }
            Constraint::Implies { .. } => {
                // `Implies` is an information statement, not a
                // violation trigger: it tells other engine paths that
                // `right` is safe to omit when `left` is present. No
                // diagnostic emission.
            }
            Constraint::Supersedes { .. } => {
                // `Supersedes` is a rewrite hint for banner roll-up,
                // not a violation trigger — the rewrite itself is
                // applied by `project(Scope::Page, ...)`. No diagnostic
                // emission from the evaluator.
            }
            Constraint::Custom { name, label } => {
                // The module-level invariant is that
                // `ConstraintViolation.constraint_label` is the
                // declared `name` and `.citation` is the `label`
                // verbatim. `evaluate_custom` is free to build
                // scheme-specific per-violation messages, but the
                // identifier surface must resolve uniformly — we
                // override both fields after the call so the scheme
                // can't accidentally drift from the catalog.
                //
                // Sub-rule information (e.g., HCS's "HCS-legacy-bare"
                // vs "HCS-O-requires-ORCON" differentiation) belongs
                // in the violation message, not in `constraint_label`.
                // Schemes that need per-subcheck surfacing must carry
                // that signal through `message` or declare distinct
                // `Constraint::Custom` entries.
                out.extend(
                    scheme
                        .evaluate_custom(name, marking)
                        .into_iter()
                        .map(|mut v| {
                            v.constraint_label = name;
                            v.citation = label;
                            v
                        }),
                );
            }
        }
    }
    out
}
