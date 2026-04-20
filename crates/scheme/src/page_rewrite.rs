// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Cross-category page rewrites.
//!
//! Some CAPCO aggregation rules can't be expressed as a single-category
//! lattice join because the trigger and the effect live in different
//! categories. The canonical example is `NOFORN ⊐ REL TO`: NOFORN is a
//! dissem token, REL TO is its own category, and when NOFORN is
//! present in the page-level dissem roll-up, REL TO clears entirely.
//!
//! The engine runs category-wise projection first (each category's
//! `Lattice::join` composed over all portions), then applies the
//! scheme's `page_rewrites()` in declaration order, then validates
//! constraints. Declaring the rewrite as *data* — rather than hiding it
//! inside `PageContext::expected_rel_to` — means tooling (constraint
//! catalog, scheme-exploration UI, docs generator) can render the
//! scheme's full aggregation semantics without executing scheme code.
//!
//! See §7a of the Phase B design doc.

use crate::category::CategoryId;
use crate::scheme::MarkingScheme;

/// One post-aggregation page-level rewrite.
///
/// A rewrite is a pair of (`trigger`, `action`) — if the trigger fires
/// on the page-aggregated marking, the action mutates that marking
/// before constraint validation. The `id` and `citation` fields exist
/// for diagnostic / audit purposes: the engine records which rewrites
/// ran during a given `project(Scope::Page, ...)` invocation so a
/// reviewer can see exactly which rewrites shaped the final banner.
///
/// Generic over `S: MarkingScheme` because predicate and action need
/// to reference the scheme's concrete marking type.
pub struct PageRewrite<S: MarkingScheme + ?Sized> {
    /// Stable identifier for this rewrite. Surfaced in diagnostics and
    /// audit records. Convention: `"scheme/snake-case-description"`
    /// (e.g., `"capco/noforn-clears-rel-to"`).
    pub id: &'static str,
    /// The rewrite's CAPCO / CUI / other-spec citation.
    pub citation: &'static str,
    /// When this rewrite fires.
    pub trigger: CategoryPredicate<S>,
    /// What to do when it fires.
    pub action: CategoryAction<S>,
}

/// Trigger for a [`PageRewrite`].
///
/// `Custom` carries a function pointer for predicates that can't be
/// expressed as "token present in category" — e.g., FGI concealment
/// transitions that depend on the presence of multiple markers.
pub enum CategoryPredicate<S: MarkingScheme + ?Sized> {
    /// The marking contains the given token in the given category.
    Contains {
        category: CategoryId,
        token: S::Token,
    },
    /// The named category is empty in the aggregated marking.
    Empty { category: CategoryId },
    /// Scheme-specific predicate.
    Custom(fn(&S::Marking) -> bool),
}

/// Action performed by a [`PageRewrite`].
pub enum CategoryAction<S: MarkingScheme + ?Sized> {
    /// Drop every value from the named category.
    Clear { category: CategoryId },
    /// Replace the named category with a supplied marking.
    Replace {
        category: CategoryId,
        with: S::Marking,
    },
    /// Scheme-specific mutation.
    Custom(fn(&mut S::Marking)),
}

// Manual Debug impls — function pointers don't auto-derive well across
// all Rust versions, and the marking types rarely implement Debug in a
// uniform way. Keep these minimal so tooling can still inspect the
// data-form variants.
impl<S: MarkingScheme + ?Sized> std::fmt::Debug for CategoryPredicate<S>
where
    S::Token: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Contains { category, token } => f
                .debug_struct("Contains")
                .field("category", category)
                .field("token", token)
                .finish(),
            Self::Empty { category } => {
                f.debug_struct("Empty").field("category", category).finish()
            }
            Self::Custom(_) => f.write_str("Custom(<fn>)"),
        }
    }
}

impl<S: MarkingScheme + ?Sized> std::fmt::Debug for CategoryAction<S>
where
    S::Marking: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Clear { category } => {
                f.debug_struct("Clear").field("category", category).finish()
            }
            Self::Replace { category, with } => f
                .debug_struct("Replace")
                .field("category", category)
                .field("with", with)
                .finish(),
            Self::Custom(_) => f.write_str("Custom(<fn>)"),
        }
    }
}
