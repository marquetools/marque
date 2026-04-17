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
//! [`Constraint::Custom`] — a label identifying a scheme-specific rule
//! that the scheme's own [`crate::MarkingScheme::validate`]
//! implementation is responsible for evaluating. The enum variant
//! carries only a label because `Constraint` is a `&[…]`-returned
//! value; schemes own the actual predicate logic privately. The label
//! is what surfaces in diagnostics, docs, and the constraint catalog.
//!
//! The engine iterates `MarkingScheme::constraints()` after parsing /
//! joining to display the catalog; a full evaluator calls
//! `MarkingScheme::validate`, which dispatches dyadic variants
//! directly and routes `Custom` variants into the scheme's own
//! predicate.

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
    /// A scheme-specific constraint identified by a stable label.
    ///
    /// The payload is just the label (what appears in diagnostics and
    /// the declared-constraint catalog). The actual predicate lives
    /// inside the scheme's [`crate::MarkingScheme::validate`]
    /// implementation, which matches on the label and runs the
    /// scheme-specific check. This is the escape hatch for n-ary rules
    /// that can't be expressed as a pair of token references — SIGMA
    /// must sort numerically, CNWDI requires classification ≥ S, JOINT
    /// participants must appear in REL TO, etc.
    ///
    /// Keeping the predicate out of the variant lets `Constraint`
    /// stay `'static` and returnable as `&[Constraint]`; a future
    /// variant that carries `Arc<dyn Fn(&Marking) -> ...>` can be
    /// added alongside if an engine-side generic evaluator becomes
    /// valuable.
    Custom(&'static str),
}

/// A constraint that fired against a marking.
#[derive(Debug, Clone)]
pub struct ConstraintViolation {
    pub constraint_label: &'static str,
    pub message: String,
}
