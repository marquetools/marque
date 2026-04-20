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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ambiguity::Parsed;
    use crate::category::{Category, TokenId};
    use crate::constraint::{Constraint, ConstraintViolation};
    use crate::lattice::Lattice;
    use crate::scope::Scope;
    use crate::template::Template;

    // Minimal scheme used purely to instantiate the generic types in
    // tests. Not exported; exists only so `PageRewrite<_>` has a
    // concrete `S` to test Debug impls and variant construction
    // against.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct FakeMarking(u32);

    impl Lattice for FakeMarking {
        fn join(&self, other: &Self) -> Self {
            Self(self.0.max(other.0))
        }
        fn meet(&self, other: &Self) -> Self {
            Self(self.0.min(other.0))
        }
    }

    struct FakeScheme;

    impl MarkingScheme for FakeScheme {
        type Token = TokenId;
        type Marking = FakeMarking;
        type ParseError = ();

        fn name(&self) -> &str {
            "fake"
        }
        fn schema_version(&self) -> &str {
            "v0"
        }
        fn categories(&self) -> &[Category] {
            &[]
        }
        fn constraints(&self) -> &[Constraint] {
            &[]
        }
        fn templates(&self) -> &[Template] {
            &[]
        }
        fn parse(&self, _: &str) -> Result<Parsed<Self::Marking>, Self::ParseError> {
            Err(())
        }
        fn validate(&self, _: &Self::Marking) -> Vec<ConstraintViolation> {
            vec![]
        }
        fn project(&self, _: Scope, _: &[Self::Marking]) -> Self::Marking {
            FakeMarking(0)
        }
        fn render_portion(&self, _: &Self::Marking) -> String {
            String::new()
        }
        fn render_banner(&self, _: &Self::Marking) -> String {
            String::new()
        }
    }

    #[test]
    fn debug_category_predicate_contains() {
        let p: CategoryPredicate<FakeScheme> = CategoryPredicate::Contains {
            category: crate::category::CategoryId(1),
            token: TokenId(42),
        };
        let s = format!("{p:?}");
        assert!(s.contains("Contains"));
        assert!(s.contains("category"));
        assert!(s.contains("token"));
    }

    #[test]
    fn debug_category_predicate_empty() {
        let p: CategoryPredicate<FakeScheme> = CategoryPredicate::Empty {
            category: crate::category::CategoryId(2),
        };
        let s = format!("{p:?}");
        assert!(s.contains("Empty"));
        assert!(s.contains("category"));
    }

    #[test]
    fn debug_category_predicate_custom() {
        let p: CategoryPredicate<FakeScheme> = CategoryPredicate::Custom(|_: &FakeMarking| true);
        let s = format!("{p:?}");
        assert_eq!(s, "Custom(<fn>)");
    }

    #[test]
    fn debug_category_action_clear() {
        let a: CategoryAction<FakeScheme> = CategoryAction::Clear {
            category: crate::category::CategoryId(3),
        };
        let s = format!("{a:?}");
        assert!(s.contains("Clear"));
    }

    #[test]
    fn debug_category_action_replace() {
        let a: CategoryAction<FakeScheme> = CategoryAction::Replace {
            category: crate::category::CategoryId(4),
            with: FakeMarking(99),
        };
        let s = format!("{a:?}");
        assert!(s.contains("Replace"));
        assert!(s.contains("99"));
    }

    #[test]
    fn debug_category_action_custom() {
        let a: CategoryAction<FakeScheme> = CategoryAction::Custom(|_: &mut FakeMarking| {});
        let s = format!("{a:?}");
        assert_eq!(s, "Custom(<fn>)");
    }

    #[test]
    fn page_rewrite_struct_fields_accessible() {
        // Exercise the PageRewrite struct itself — store, read back.
        let rw: PageRewrite<FakeScheme> = PageRewrite {
            id: "test/r1",
            citation: "doc §1",
            trigger: CategoryPredicate::Empty {
                category: crate::category::CategoryId(1),
            },
            action: CategoryAction::Clear {
                category: crate::category::CategoryId(1),
            },
        };
        assert_eq!(rw.id, "test/r1");
        assert_eq!(rw.citation, "doc §1");
        // Trigger / action reachable through pattern match.
        match rw.trigger {
            CategoryPredicate::Empty { category } => {
                assert_eq!(category, crate::category::CategoryId(1));
            }
            _ => panic!("wrong variant"),
        }
        match rw.action {
            CategoryAction::Clear { category } => {
                assert_eq!(category, crate::category::CategoryId(1));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn category_predicate_custom_fn_dispatch() {
        // Execute the Custom predicate to exercise the fn-pointer call.
        let p: CategoryPredicate<FakeScheme> = CategoryPredicate::Custom(|m: &FakeMarking| m.0 > 0);
        if let CategoryPredicate::Custom(f) = p {
            assert!(!f(&FakeMarking(0)));
            assert!(f(&FakeMarking(7)));
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn category_action_custom_fn_dispatch() {
        let a: CategoryAction<FakeScheme> = CategoryAction::Custom(|m: &mut FakeMarking| {
            m.0 = 42;
        });
        if let CategoryAction::Custom(f) = a {
            let mut m = FakeMarking(0);
            f(&mut m);
            assert_eq!(m.0, 42);
        } else {
            panic!("wrong variant");
        }
    }

    // Exercise the FakeMarking / FakeScheme impls directly so the
    // helper code used to host the PageRewrite tests doesn't show up
    // as "uncovered" — these are internal-test support code, but
    // coverage tools count them.

    #[test]
    fn fake_marking_lattice_ops() {
        let a = FakeMarking(3);
        let b = FakeMarking(7);
        assert_eq!(a.join(&b), FakeMarking(7));
        assert_eq!(a.meet(&b), FakeMarking(3));
    }

    #[test]
    fn fake_scheme_getters_return_expected_values() {
        let s = FakeScheme;
        assert_eq!(s.name(), "fake");
        assert_eq!(s.schema_version(), "v0");
        assert!(s.categories().is_empty());
        assert!(s.constraints().is_empty());
        assert!(s.templates().is_empty());
    }

    #[test]
    fn fake_scheme_parse_returns_err() {
        let s = FakeScheme;
        assert!(s.parse("anything").is_err());
    }

    #[test]
    fn fake_scheme_validate_returns_empty() {
        let s = FakeScheme;
        assert!(s.validate(&FakeMarking(0)).is_empty());
    }

    #[test]
    fn fake_scheme_project_returns_fake_zero() {
        let s = FakeScheme;
        assert_eq!(s.project(Scope::Page, &[FakeMarking(5)]), FakeMarking(0));
    }

    #[test]
    fn fake_scheme_render_returns_empty() {
        let s = FakeScheme;
        assert_eq!(s.render_portion(&FakeMarking(0)), "");
        assert_eq!(s.render_banner(&FakeMarking(0)), "");
    }

    #[test]
    fn fake_scheme_page_rewrites_default_is_empty() {
        // Exercise the trait-level default `page_rewrites()` impl —
        // FakeScheme doesn't override it.
        let s = FakeScheme;
        assert!(s.page_rewrites().is_empty());
    }

    #[test]
    fn fake_scheme_project_banner_shim_delegates_to_project() {
        // Exercise the `project_banner` default shim.
        let s = FakeScheme;
        assert_eq!(s.project_banner(&[FakeMarking(1)]), FakeMarking(0));
    }
}
