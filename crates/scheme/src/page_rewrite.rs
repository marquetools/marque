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

/// Stable identifier for a [`PageRewrite`]. Alias for `&'static str` —
/// exists as a named type so error variants can say
/// `members: Box<[RewriteId]>` and line up with the data-model
/// contract without forcing a newtype wrapper at every call site.
///
/// Convention: `"scheme/snake-case-description"` (e.g.,
/// `"capco/noforn-clears-rel-to"`).
pub type RewriteId = &'static str;

/// One post-aggregation page-level rewrite.
///
/// A rewrite is a pair of (`trigger`, `action`) — if the trigger fires
/// on the page-aggregated marking, the action mutates that marking
/// before constraint validation. The `id` and `citation` fields exist
/// for diagnostic / audit purposes: the engine records which rewrites
/// ran during a given `project(Scope::Page, ...)` invocation so a
/// reviewer can see exactly which rewrites shaped the final banner.
///
/// `reads` / `writes` declare the categories the rewrite depends on
/// and the categories it mutates. They surface two properties the
/// engine relies on:
///
/// 1. **Topological ordering** — the engine sorts rewrites so a
///    rewrite that writes category X runs before any rewrite that
///    reads X (Phase 3 / T031–T032). Cycles fail scheme construction
///    (`EngineConstructionError::RewriteCycle`).
/// 2. **Tooling** — scheme-exploration UIs and the declarative-
///    constraint catalog can render the dataflow graph without
///    executing scheme code.
///
/// For declarative triggers and actions the fields can be derived
/// from the variants themselves (Phase 3 / T029 adds const-fn
/// derivation in [`PageRewrite::declarative`]). For `Custom` triggers
/// and actions the caller MUST supply them explicitly via
/// [`PageRewrite::custom`] because the function-pointer bodies are
/// opaque to the engine.
///
/// Generic over `S: MarkingScheme` because predicate and action need
/// to reference the scheme's concrete marking type.
pub struct PageRewrite<S: MarkingScheme + ?Sized> {
    /// Stable identifier for this rewrite. Surfaced in diagnostics and
    /// audit records. Convention: `"scheme/snake-case-description"`
    /// (e.g., `"capco/noforn-clears-rel-to"`).
    pub id: RewriteId,
    /// The rewrite's CAPCO / CUI / other-spec citation.
    pub citation: &'static str,
    /// When this rewrite fires.
    pub trigger: CategoryPredicate<S>,
    /// What to do when it fires.
    pub action: CategoryAction<S>,
    /// Categories this rewrite inspects. Used by the engine to build
    /// the topological ordering between rewrites.
    pub reads: &'static [CategoryId],
    /// Categories this rewrite mutates. Used by the engine to build
    /// the topological ordering between rewrites.
    pub writes: &'static [CategoryId],
}

impl<S: MarkingScheme + ?Sized> PageRewrite<S> {
    /// Constructor for a declarative rewrite — all-data triggers and
    /// actions (`Contains` / `Empty` + `Clear` / `Replace` / `Promote`).
    ///
    /// The caller still passes `reads` / `writes` explicitly. The
    /// scheduler in `marque-engine` uses them to build the topological
    /// ordering between rewrites (task T031). For a pure-declarative
    /// rewrite the slices should be derivable from the `trigger` and
    /// `action` categories; callers that want a single-category hint
    /// can construct the slices as `const`s at the call site. Deriving
    /// them at construction would require runtime allocation — the
    /// scheme's rewrite table is a `&'static` constant — so this stays
    /// a plain fallible-free `const fn`.
    ///
    /// For `Custom` trigger or action variants the scheduler cannot
    /// derive dataflow from the variant itself, so use
    /// [`PageRewrite::custom`] which fails closed on empty annotations.
    pub const fn declarative(
        id: RewriteId,
        citation: &'static str,
        trigger: CategoryPredicate<S>,
        action: CategoryAction<S>,
        reads: &'static [CategoryId],
        writes: &'static [CategoryId],
    ) -> Self {
        Self {
            id,
            citation,
            trigger,
            action,
            reads,
            writes,
        }
    }

    /// `const` constructor for a rewrite with a `Custom` trigger or
    /// action. Panics when `reads` or `writes` is empty:
    ///
    /// - Called from a `static` / `const` initializer (the common
    ///   case), the panic fires during **`const` evaluation** — the
    ///   build fails at compile time with the `assert!` messages
    ///   below, so the error is visible at the declaration site
    ///   before the crate ever runs.
    /// - Called from a non-`const` call site (runtime), the panic
    ///   fires at runtime as a normal panic.
    ///
    /// Callers MUST annotate `reads` / `writes` explicitly because the
    /// function-pointer bodies are opaque. Empty slices are treated
    /// as "unannotated" and abort construction — a `Custom` rewrite
    /// that reaches the scheduler without axis annotations cannot be
    /// ordered relative to the other rewrites, and silently
    /// degrading would mask the authoring bug.
    ///
    /// For callers holding pre-built `'static` tables that need a
    /// recoverable validation path (rather than a panic), use
    /// [`PageRewrite::try_custom`]. The engine's
    /// `EngineConstructionError::UnannotatedCustomAxes` variant
    /// catches the same invariant at `Engine::new` when a
    /// field-literal rewrite bypasses both constructors — the
    /// invariant is guarded on three surfaces (compile/runtime
    /// construct via this fn, fallible construct via `try_custom`,
    /// and engine-construct).
    pub const fn custom(
        id: RewriteId,
        citation: &'static str,
        trigger: CategoryPredicate<S>,
        action: CategoryAction<S>,
        reads: &'static [CategoryId],
        writes: &'static [CategoryId],
    ) -> Self {
        assert!(
            !reads.is_empty(),
            "PageRewrite::custom: empty `reads` slice; Custom triggers/actions \
             require explicit axis annotation so the scheduler can order them. \
             Use `PageRewrite::try_custom` for runtime-authored rewrites that \
             need a recoverable error path."
        );
        assert!(
            !writes.is_empty(),
            "PageRewrite::custom: empty `writes` slice; Custom triggers/actions \
             require explicit axis annotation so the scheduler can order them. \
             Use `PageRewrite::try_custom` for runtime-authored rewrites that \
             need a recoverable error path."
        );
        Self {
            id,
            citation,
            trigger,
            action,
            reads,
            writes,
        }
    }

    /// Non-panicking runtime constructor for a rewrite with a `Custom`
    /// trigger or action.
    ///
    /// Returns [`PageRewriteAxisError`] when `reads` or `writes` is
    /// empty — the same invariant [`PageRewrite::custom`] enforces via
    /// a `const` panic. Use this constructor when you have pre-built
    /// `&'static` axis slices and want validation to surface as a
    /// `Result` rather than aborting the process (e.g., a rewrite
    /// registry that loads from a baked-in table and wants to report
    /// authoring errors through its own error path).
    ///
    /// `'static` is a load-bearing requirement: both `reads` and
    /// `writes` must be `&'static [CategoryId]` because `PageRewrite`
    /// itself stores them that way and the engine's scheduler walks
    /// them without owning. For truly *dynamic* axes (e.g., parsed
    /// from a user config at runtime), callers must either leak the
    /// slice (`Box::leak(Vec::into_boxed_slice(…))`) during one-time
    /// scheme initialization or design a separate owned-axis rewrite
    /// type. An owned-axis variant is not provided here because every
    /// production scheme in-tree (CAPCO, and the future CUI / NATO
    /// schemes) declares its rewrite table as a `const` at scheme-
    /// construction time.
    pub fn try_custom(
        id: RewriteId,
        citation: &'static str,
        trigger: CategoryPredicate<S>,
        action: CategoryAction<S>,
        reads: &'static [CategoryId],
        writes: &'static [CategoryId],
    ) -> Result<Self, PageRewriteAxisError> {
        if reads.is_empty() {
            return Err(PageRewriteAxisError::EmptyReads { rewrite: id });
        }
        if writes.is_empty() {
            return Err(PageRewriteAxisError::EmptyWrites { rewrite: id });
        }
        Ok(Self {
            id,
            citation,
            trigger,
            action,
            reads,
            writes,
        })
    }
}

/// Error returned by [`PageRewrite::try_custom`] when a `Custom`
/// rewrite is constructed without axis annotations.
///
/// Mirrors the engine-level
/// `EngineConstructionError::UnannotatedCustomAxes` variant. The two
/// types are kept separate because `marque-scheme` is upstream of
/// `marque-engine` in the crate graph (see Constitution VII) and
/// cannot reference the engine's error type. Engine callers that
/// want to fold rewrite-construction errors into the engine-
/// construction error surface can convert at the boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageRewriteAxisError {
    /// `reads` was empty; a `Custom` trigger/action requires at least
    /// one read-axis annotation so the scheduler can order this
    /// rewrite after its producers.
    EmptyReads { rewrite: RewriteId },
    /// `writes` was empty; a `Custom` trigger/action requires at
    /// least one write-axis annotation so the scheduler can order
    /// this rewrite before its consumers.
    EmptyWrites { rewrite: RewriteId },
}

impl std::fmt::Display for PageRewriteAxisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyReads { rewrite } => write!(
                f,
                "page-rewrite {rewrite:?}: empty `reads` on a Custom rewrite"
            ),
            Self::EmptyWrites { rewrite } => write!(
                f,
                "page-rewrite {rewrite:?}: empty `writes` on a Custom rewrite"
            ),
        }
    }
}

impl std::error::Error for PageRewriteAxisError {}

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
    /// Promote content from `from` into `to`, optionally transformed
    /// by a declarative function. Used for rewrites like JOINT →
    /// Classification or FGI absorption: the `from` category drains
    /// and `to` absorbs the transformed content.
    ///
    /// The transform is a plain `fn` pointer (not a closure) so the
    /// rewrite remains data-form — scheme-exploration tooling can
    /// still pattern-match on the variant without executing scheme
    /// code, and the engine can pre-compute `reads` / `writes` from
    /// the variant's categories.
    Promote {
        from: CategoryId,
        to: CategoryId,
        transform: fn(&S::Marking) -> S::Marking,
    },
    /// Scheme-specific mutation.
    Custom(fn(&mut S::Marking)),
    /// Apply a structural fix-intent at page scope.
    ///
    /// Bridges page-level declarative rewrites to the
    /// [`ReplacementIntent`](crate::ReplacementIntent) vocabulary already
    /// used for rule-emitted fixes. The trigger detects a page-scope
    /// precondition (e.g., NODIS present in a dissem-control category)
    /// and the action expresses the rewrite as a `FactAdd` / `FactRemove`
    /// / `Recanonicalize` operation against the projected marking.
    ///
    /// Unlike `Clear` / `Replace` (which operate on an entire category)
    /// and `Custom` (which is opaque to the scheduler), `Intent` lets the
    /// rewrite mutate one or more named facts in a category while
    /// remaining declarative (`FactAdd` carries one fact;
    /// `FactRemove` carries a `SmallVec` of one or more for atomic
    /// multi-fact clusters like the E024 RD/FRD/TFNI removal). The
    /// rewrite author still declares `reads` / `writes` annotations
    /// explicitly via [`PageRewrite::declarative`].
    ///
    /// **Validation**: every `CategoryAction::Intent` is validated at
    /// engine-construction time. The engine walks each rewrite's
    /// `FactRef`s and calls the scheme's category routing to confirm
    /// every token maps to a category. If any token is unroutable, the
    /// engine returns `EngineConstructionError::InvalidIntentInPageRewrite`
    /// at `Engine::new` — the failure surfaces deterministically at
    /// startup, not on the first page that triggers the rewrite.
    ///
    /// **Note on `Recanonicalize`**: the `Recanonicalize` variant of
    /// `ReplacementIntent` is a no-op when used inside a `PageRewrite`
    /// action. Page rewrites mutate the projected marking before the
    /// renderer runs; a re-canonicalization intent has no semantic effect
    /// at this layer. Authoring `CategoryAction::Intent(Recanonicalize {
    /// .. })` is permitted (for round-trip uniformity) but is silently
    /// inert.
    Intent(crate::fix_intent::ReplacementIntent<S>),
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
            Self::Promote { from, to, .. } => f
                .debug_struct("Promote")
                .field("from", from)
                .field("to", to)
                .field("transform", &"<fn>")
                .finish(),
            Self::Custom(_) => f.write_str("Custom(<fn>)"),
            Self::Intent(intent) => f.debug_tuple("Intent").field(intent).finish(),
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
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
        type OpenVocabRef = core::convert::Infallible;

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
        fn render_canonical(
            &self,
            _: &Self::Marking,
            _: Scope,
            _: &mut dyn core::fmt::Write,
        ) -> core::fmt::Result {
            Ok(())
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
    fn debug_category_action_intent_fact_add() {
        let a: CategoryAction<FakeScheme> =
            CategoryAction::Intent(crate::fix_intent::ReplacementIntent::FactAdd {
                token: crate::fix_intent::FactRef::Cve(TokenId(7)),
                scope: Scope::Page,
            });
        let s = format!("{a:?}");
        assert!(s.contains("Intent"), "got: {s}");
        assert!(s.contains("FactAdd"), "got: {s}");
    }

    #[test]
    fn debug_category_action_intent_fact_remove() {
        let a: CategoryAction<FakeScheme> =
            CategoryAction::Intent(crate::fix_intent::ReplacementIntent::fact_remove(
                crate::fix_intent::FactRef::Cve(TokenId(3)),
                Scope::Page,
            ));
        let s = format!("{a:?}");
        assert!(s.contains("Intent"), "got: {s}");
        assert!(s.contains("FactRemove"), "got: {s}");
    }

    #[test]
    fn debug_category_action_intent_recanonicalize() {
        let a: CategoryAction<FakeScheme> =
            CategoryAction::Intent(crate::fix_intent::ReplacementIntent::Recanonicalize {
                scope: crate::fix_intent::RecanonScope::Page,
            });
        let s = format!("{a:?}");
        assert!(s.contains("Intent"), "got: {s}");
        assert!(s.contains("Recanonicalize"), "got: {s}");
    }

    #[test]
    fn try_custom_rejects_empty_reads() {
        let res = PageRewrite::<FakeScheme>::try_custom(
            "bad",
            "test",
            CategoryPredicate::Custom(|_: &FakeMarking| false),
            CategoryAction::Custom(|_: &mut FakeMarking| {}),
            &[],
            &[crate::category::CategoryId(1)],
        );
        let err = match res {
            Ok(_) => panic!("empty reads must fail"),
            Err(e) => e,
        };
        assert_eq!(err, PageRewriteAxisError::EmptyReads { rewrite: "bad" },);
    }

    #[test]
    fn try_custom_rejects_empty_writes() {
        let res = PageRewrite::<FakeScheme>::try_custom(
            "bad",
            "test",
            CategoryPredicate::Custom(|_: &FakeMarking| false),
            CategoryAction::Custom(|_: &mut FakeMarking| {}),
            &[crate::category::CategoryId(1)],
            &[],
        );
        let err = match res {
            Ok(_) => panic!("empty writes must fail"),
            Err(e) => e,
        };
        assert_eq!(err, PageRewriteAxisError::EmptyWrites { rewrite: "bad" },);
    }

    #[test]
    fn try_custom_accepts_non_empty_axes() {
        let ok = PageRewrite::<FakeScheme>::try_custom(
            "ok",
            "test",
            CategoryPredicate::Custom(|_: &FakeMarking| false),
            CategoryAction::Custom(|_: &mut FakeMarking| {}),
            &[crate::category::CategoryId(1)],
            &[crate::category::CategoryId(2)],
        );
        assert!(ok.is_ok());
    }

    #[test]
    fn page_rewrite_axis_error_display_is_informative() {
        let err = PageRewriteAxisError::EmptyReads { rewrite: "r1" };
        let s = format!("{err}");
        assert!(s.contains("r1"));
        assert!(s.contains("reads"));
    }

    #[test]
    fn page_rewrite_struct_fields_accessible() {
        // Exercise the PageRewrite struct itself — store, read back.
        let rw: PageRewrite<FakeScheme> = PageRewrite {
            id: "test/r1",
            citation: "doc test-fixture",
            trigger: CategoryPredicate::Empty {
                category: crate::category::CategoryId(1),
            },
            action: CategoryAction::Clear {
                category: crate::category::CategoryId(1),
            },
            reads: &[crate::category::CategoryId(1)],
            writes: &[crate::category::CategoryId(1)],
        };
        assert_eq!(rw.id, "test/r1");
        assert_eq!(rw.citation, "doc test-fixture");
        assert_eq!(rw.reads, &[crate::category::CategoryId(1)]);
        assert_eq!(rw.writes, &[crate::category::CategoryId(1)]);
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
