//! Declarative constraints between tokens.
//!
//! A constraint is a predicate over a marking. Most CAPCO constraints
//! are dyadic (NOFORN ∦ REL TO; HCS ⇒ NOFORN; RD ⇒ NOFORN by default);
//! a handful are not (SIGMA compartments must appear in numeric order;
//! CNWDI requires classification ≥ S). The dyadic ones land in the
//! enumerated variants; the rest use `Custom`.
//!
//! The engine iterates `MarkingScheme::constraints()` after parsing /
//! joining and produces one `ConstraintViolation` per failing predicate.

use crate::category::TokenId;

/// Reference to a token or category in a constraint. Kept as a small
/// enum rather than a bare `TokenId` because some constraints are
/// expressed at category granularity (e.g., "no IC dissem with JOINT").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenRef {
    /// A specific token id.
    Token(TokenId),
    /// Any token in the named category.
    AnyInCategory(crate::category::CategoryId),
}

/// A declarative invariant the scheme enforces.
#[derive(Debug, Clone)]
pub enum Constraint {
    /// Two tokens cannot co-occur in one marking. Example: NOFORN and
    /// REL TO are mutually exclusive at the portion level.
    Conflicts(TokenRef, TokenRef),
    /// If the left is present, the right must also be present.
    /// Example: HCS requires NOFORN.
    Requires(TokenRef, TokenRef),
    /// If the left is present, the right is implied (safe to omit).
    /// The engine uses this to avoid false "missing X" diagnostics.
    Implies(TokenRef, TokenRef),
    /// The left supersedes the right during banner roll-up: the right
    /// drops out of the banner if the left is present. Example:
    /// NOFORN ⊐ REL TO at banner scope.
    Supersedes(TokenRef, TokenRef),
    /// Escape hatch for constraints that can't be expressed dyadically.
    /// Phase A does not use this variant; future work moves n-ary rules
    /// (SIGMA ordering, CNWDI classification floor, JOINT participant
    /// membership) to `Custom` predicates parameterised by the scheme.
    Custom(&'static str),
}

/// A constraint that fired against a marking.
#[derive(Debug, Clone)]
pub struct ConstraintViolation {
    pub constraint_label: &'static str,
    pub message: String,
}
