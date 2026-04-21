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
/// Every variant carries a `label: &'static str` citation pointing at
/// the authoritative-source passage that defines the constraint
/// (see the module-level docs).
#[derive(Debug, Clone)]
pub enum Constraint {
    /// Two tokens cannot co-occur in one marking. Example: NOFORN and
    /// REL TO are mutually exclusive at the portion level.
    Conflicts {
        left: TokenRef,
        right: TokenRef,
        label: &'static str,
    },
    /// If the left is present, the right must also be present.
    /// Example: HCS requires NOFORN.
    Requires {
        left: TokenRef,
        right: TokenRef,
        label: &'static str,
    },
    /// If the left is present, the right is implied (safe to omit).
    /// The engine uses this to avoid false "missing X" diagnostics.
    Implies {
        left: TokenRef,
        right: TokenRef,
        label: &'static str,
    },
    /// The left supersedes the right during banner roll-up: the right
    /// drops out of the banner if the left is present. Example:
    /// NOFORN ⊐ REL TO at banner scope.
    Supersedes {
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
#[derive(Debug, Clone)]
pub struct ConstraintViolation {
    pub constraint_label: &'static str,
    pub message: String,
    pub citation: &'static str,
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
            Constraint::Conflicts { left, right, label } => {
                if scheme.satisfies(marking, left) && scheme.satisfies(marking, right) {
                    out.push(ConstraintViolation {
                        constraint_label: "conflicts",
                        message: format!("conflicting tokens: {left:?} and {right:?}"),
                        citation: label,
                    });
                }
            }
            Constraint::Requires { left, right, label } => {
                if scheme.satisfies(marking, left) && !scheme.satisfies(marking, right) {
                    out.push(ConstraintViolation {
                        constraint_label: "requires",
                        message: format!("token {left:?} requires {right:?} but it is missing"),
                        citation: label,
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
                // `ConstraintViolation.citation` is the triggering
                // constraint's authoritative-source `label` verbatim.
                // `evaluate_custom` is free to build scheme-specific
                // per-violation messages and sub-labels, but the
                // citation surface must resolve to the same string
                // the catalog publishes — we override after the call
                // so the scheme can't accidentally drift from it.
                out.extend(scheme.evaluate_custom(name, marking).into_iter().map(
                    |mut v| {
                        v.citation = label;
                        v
                    },
                ));
            }
        }
    }
    out
}
