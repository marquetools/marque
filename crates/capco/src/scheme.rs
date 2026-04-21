// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoScheme` — CAPCO's implementation of the `MarkingScheme` trait.
//!
//! This is the Phase A proof that CAPCO's hand-written aggregation in
//! [`PageContext`] falls out of the generic `marque-scheme` abstraction.
//! The adapter wraps `IsmAttributes` as `CapcoMarking`, implements
//! [`Lattice`] by delegating the join to `PageContext`'s existing
//! rollup, and exposes a minimal three-constraint sample to validate
//! that declarative constraints can reproduce existing rule behavior.
//!
//! The bulk of the migration — moving every CAPCO rule and replacing
//! `PageContext`'s internals — is Phase B/C work. The design doc
//! `docs/plans/2026-04-17-marking-scheme-lattice-design.md` sequences
//! the full migration.
//!
//! # Category identifiers
//!
//! CAPCO's categories are assigned small stable ids here. The specific
//! numbers are opaque — the engine only compares them for equality.
//! They're kept as constants so tests can reference them.

use marque_ism::{Classification, IsmAttributes, PageContext, Trigraph};
use marque_scheme::{
    AggregationOp, Cardinality, Category, CategoryAction, CategoryId, CategoryPredicate,
    Constraint, ConstraintViolation, IntraOrdering, Lattice, MarkingScheme, PageRewrite, Parsed,
    Scope, Template, TokenId, TokenRef,
};

// ---------------------------------------------------------------------------
// Category ids
// ---------------------------------------------------------------------------

pub const CAT_CLASSIFICATION: CategoryId = CategoryId(1);
pub const CAT_NON_US_CLASSIFICATION: CategoryId = CategoryId(2);
pub const CAT_JOINT_CLASSIFICATION: CategoryId = CategoryId(3);
pub const CAT_SCI: CategoryId = CategoryId(4);
pub const CAT_SAR: CategoryId = CategoryId(5);
pub const CAT_AEA: CategoryId = CategoryId(6);
pub const CAT_FGI_MARKER: CategoryId = CategoryId(7);
pub const CAT_DISSEM: CategoryId = CategoryId(8);
pub const CAT_REL_TO: CategoryId = CategoryId(9);
pub const CAT_DECLASSIFY_ON: CategoryId = CategoryId(10);

// ---------------------------------------------------------------------------
// Sentinel token ids for constraint expressions
// ---------------------------------------------------------------------------
//
// Phase C will replace these with generated ids pointing to specific
// CVE tokens. For Phase A we only need enough ids to express the three
// sample constraints that the equivalence tests exercise.

pub const TOK_NOFORN: TokenId = TokenId(100);
pub const TOK_JOINT: TokenId = TokenId(103);
pub const TOK_USA: TokenId = TokenId(104);

// Sentinel token ids for the Phase 3 declarative constraint catalog
// (T033). These identify specific tokens referenced by
// `Constraint::{Conflicts, Requires, Supersedes}` entries in the
// 12-rule migration set. Phase 4 replaces them with generated
// per-CVE-value ids; Phase 3 uses sentinels because the engine's
// `lint` path still consults hand-written rule impls as the
// authoritative diagnostic source, and the declarative constraint
// data here exists for scheme-exploration + Phase 4 decoder
// consumption — not (yet) for runtime evaluation.

pub const TOK_RESTRICTED: TokenId = TokenId(110);
pub const TOK_RD: TokenId = TokenId(111);
pub const TOK_FRD: TokenId = TokenId(112);
pub const TOK_TFNI: TokenId = TokenId(113);
pub const TOK_CNWDI: TokenId = TokenId(114);
pub const TOK_UCNI: TokenId = TokenId(115);
pub const TOK_HCS: TokenId = TokenId(116);
pub const TOK_FGI_MARKER: TokenId = TokenId(117);
pub const TOK_US_CLASSIFIED: TokenId = TokenId(118);
pub const TOK_IC_DISSEM: TokenId = TokenId(119);
pub const TOK_NON_IC_DISSEM: TokenId = TokenId(120);
pub const TOK_NON_US_CLASSIFICATION: TokenId = TokenId(121);

// ---------------------------------------------------------------------------
// CapcoMarking — newtype over IsmAttributes implementing Lattice
// ---------------------------------------------------------------------------

/// CAPCO marking as viewed through the `marque-scheme` lens. A thin
/// newtype around [`IsmAttributes`] so we can hang trait impls on it
/// without orphan-rule problems.
///
/// # ⚠️ Phase A scaffolding — do not use in production
///
/// `CapcoMarking` is exported publicly so the Phase A equivalence
/// tests can construct it, but it **does not uphold the [`Lattice`]
/// contract** on every input (see the caveat block on the `Lattice`
/// impl below). Downstream consumers must not rely on `Lattice::join`
/// / `Lattice::meet` of `CapcoMarking` producing law-abiding results
/// until Phase B replaces the impl with a proper product-lattice
/// aggregator. Use [`crate::capco_rules`] and `marque-core` directly
/// for production paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapcoMarking(pub IsmAttributes);

impl From<IsmAttributes> for CapcoMarking {
    #[inline]
    fn from(attrs: IsmAttributes) -> Self {
        Self(attrs)
    }
}

// Phase A caveat on the `Lattice` impl
// -----------------------------------
//
// The `Lattice` contract (idempotency, associativity, commutativity,
// absorption) is NOT fully guaranteed by this Phase A impl:
//
// - `join` delegates to [`PageContext`], which applies non-invertible
//   normalization (DSEN overrides FOUO in classified docs; OC-USGOV
//   drops when not present on every OC-carrying portion; UCNI drops
//   in classified docs; NOFORN clears REL TO). These rules are
//   correct CAPCO semantics but they're the *projection*, not a pure
//   component-wise product-lattice join. Markings that touch those
//   normalizations can violate absorption.
// - `meet` is a partial component-wise implementation on
//   classification + SCI + dissem (enough to satisfy the trait bound
//   and pass the narrow test inputs); all other fields reset to their
//   `Default`, so `meet` is not useful outside tests and is not
//   law-consistent with `join` in edge cases.
//
// Phase A's equivalence tests exercise the narrow, non-normalizing
// subset of inputs where the laws do hold. Phase B replaces this impl
// with a pure product-lattice `join` (component-wise aggregation of
// each category's `AggregationOp`), leaving CAPCO's normalizing
// projection in `project_banner` where it belongs. At that point
// `meet` becomes well-defined across every category.
//
// Downstream code should treat `CapcoMarking`'s `Lattice` impl as an
// expedient for Phase A tests — not a stable API surface.
impl Lattice for CapcoMarking {
    /// Join = banner-aggregate both portions via `PageContext`.
    ///
    /// Delegates to [`PageContext`] so the scheme's join is
    /// definitionally equivalent to the existing hand-written
    /// aggregation on the inputs exercised by Phase A's tests. Phase B
    /// inverts this dependency — `PageContext` will be implemented in
    /// terms of component-wise aggregation, and this method will stop
    /// applying the projection's non-invertible normalizations.
    ///
    /// See the module-level "Phase A caveat" note above for the
    /// specific laws this impl does not satisfy.
    #[inline]
    fn join(&self, other: &Self) -> Self {
        let mut ctx = PageContext::new();
        ctx.add_portion(self.0.clone());
        ctx.add_portion(other.0.clone());
        CapcoMarking(page_context_to_attrs(&ctx))
    }

    /// Meet = partial component-wise minimum.
    ///
    /// Implemented only on classification, SCI, and dissem — enough to
    /// satisfy the trait bound and serve Phase A's test inputs. All
    /// other fields reset to `Default`. This is not a full
    /// product-lattice meet; see the module-level "Phase A caveat"
    /// note above. Phase B replaces it with a proper component-wise
    /// meet across every category.
    #[inline]
    fn meet(&self, other: &Self) -> Self {
        let a = &self.0;
        let b = &other.0;

        let classification = match (&a.classification, &b.classification) {
            (Some(x), Some(y)) => {
                let min = x.effective_level().min(y.effective_level());
                Some(marque_ism::MarkingClassification::Us(min))
            }
            _ => None,
        };

        let sci: Vec<_> = a
            .sci_controls
            .iter()
            .filter(|t| b.sci_controls.contains(t))
            .copied()
            .collect();
        let dissem: Vec<_> = a
            .dissem_controls
            .iter()
            .filter(|t| b.dissem_controls.contains(t))
            .copied()
            .collect();

        let mut out = IsmAttributes::default();
        out.classification = classification;
        out.sci_controls = sci.into_boxed_slice();
        out.dissem_controls = dissem.into_boxed_slice();
        CapcoMarking(out)
    }
}

// ---------------------------------------------------------------------------
// Category-predicate / category-action dispatch (for PageRewrite)
// ---------------------------------------------------------------------------
//
// These helpers implement the trigger and action variants of a
// `PageRewrite` against CAPCO's `CapcoMarking`. They're here rather
// than in `marque-scheme` because the variant payloads reference
// `S::Token` and `S::Marking` and each scheme has to project those
// onto its concrete storage. The `CategoryPredicate::Custom` /
// `CategoryAction::Custom` variants still skip this dispatch and let
// the rewrite author supply the closure directly, but cross-category
// rewrites such as CAPCO's NOFORN rule are also supported in
// declarative form here.

/// `CategoryPredicate::Contains { category, token }` evaluator.
///
/// Phase B supports the sample constraint set. Unhandled `(category,
/// token)` pairs return `false` — a safe conservative answer that
/// effectively disables the rewrite rather than silently misfiring.
/// Phase C expands coverage as more rewrites move to the declarative
/// form.
fn capco_category_contains(m: &CapcoMarking, category: CategoryId, token: TokenId) -> bool {
    let attrs = &m.0;
    if category == CAT_DISSEM && token == TOK_NOFORN {
        return attrs
            .dissem_controls
            .iter()
            .any(|d| matches!(d, marque_ism::DissemControl::Nf));
    }
    false
}

/// `CategoryPredicate::Empty { category }` evaluator.
///
/// Unhandled categories return `true` (treated as "non-empty / unknown")
/// so an `Empty` predicate on an unknown category **does not fire**
/// and a rewrite conditioned on it stays inert. This matches
/// [`capco_category_contains`]'s conservative-false stance and avoids
/// misfiring rewrites on categories Phase B doesn't yet inspect.
/// Phase C expands the match arms as more rewrites move into the
/// declarative form.
fn capco_category_has_values(m: &CapcoMarking, category: CategoryId) -> bool {
    let attrs = &m.0;
    match category {
        CAT_REL_TO => !attrs.rel_to.is_empty(),
        CAT_DISSEM => !attrs.dissem_controls.is_empty(),
        CAT_SCI => !attrs.sci_controls.is_empty() || !attrs.sci_markings.is_empty(),
        _ => true,
    }
}

/// `CategoryAction::Clear { category }` evaluator.
fn capco_category_clear(m: &mut CapcoMarking, category: CategoryId) {
    let attrs = &mut m.0;
    if category == CAT_REL_TO {
        attrs.rel_to = Box::new([]);
    } else if category == CAT_DISSEM {
        attrs.dissem_controls = Box::new([]);
    }
    // Other categories: no-op. Phase C expands coverage.
}

/// `CategoryAction::Replace { category, with }` evaluator. The `with`
/// argument supplies a full marking; Phase B copies only the named
/// category's storage out.
fn capco_category_replace(m: &mut CapcoMarking, category: CategoryId, with: &CapcoMarking) {
    let attrs = &mut m.0;
    if category == CAT_REL_TO {
        attrs.rel_to = with.0.rel_to.clone();
    } else if category == CAT_DISSEM {
        attrs.dissem_controls = with.0.dissem_controls.clone();
    }
}

/// Identity `transform` for [`CategoryAction::Promote`] declarations
/// that do not (yet) need a body.
///
/// Phase 3 declares JOINT-promotion and FGI-absorption in the scheme's
/// rewrite table so the scheduler can sort them by their `reads` /
/// `writes` axes (T031–T032), but runtime dispatch remains with the
/// existing [`PageContext`] aggregator. The `identity_promote` fn is
/// a marker value — it is never called at runtime because the
/// `Promote` arm in `CapcoScheme::project` returns the marking
/// unchanged for Phase 3. Swapping in a real transform is a Phase D /
/// Phase E follow-up.
fn identity_promote(m: &CapcoMarking) -> CapcoMarking {
    m.clone()
}

/// Always-false [`CategoryPredicate::Custom`] body used by Phase 3's
/// JOINT-promotion and FGI-absorption declarations.
///
/// The rewrite's `reads` / `writes` axes are what the Kahn scheduler
/// consumes (T031–T032). Its trigger body does not participate in
/// Phase 3 runtime dispatch because `Engine::lint` does not route
/// aggregation through `scheme.project(Scope::Page, …)` — the
/// hand-coded [`PageContext`] aggregator handles roll-up. Pinning the
/// trigger to `false` makes that no-op explicit: any test or tool
/// that calls `scheme.project()` on today's `CapcoScheme` will see
/// these two rewrites declare but never fire.
fn never_fires(_: &CapcoMarking) -> bool {
    false
}

/// Build an `IsmAttributes` banner projection from the `expected_*`
/// accessors on `PageContext`. Intentionally narrow: only fills the
/// fields exercised by Phase A's equivalence tests. Other fields land
/// at their defaults, which matches Phase B's goal of handing
/// everything off to scheme-driven aggregation.
#[inline]
fn page_context_to_attrs(ctx: &PageContext) -> IsmAttributes {
    let mut out = IsmAttributes::default();

    out.classification = ctx
        .expected_classification()
        .map(marque_ism::MarkingClassification::Us);
    out.sci_controls = ctx.expected_sci_controls().into_boxed_slice();
    out.sci_markings = ctx.expected_sci_markings();
    out.sar_markings = ctx.expected_sar_marking();
    out.aea_markings = ctx.expected_aea_markings().into_boxed_slice();
    out.fgi_marker = ctx.expected_fgi_marker();
    out.dissem_controls = ctx.expected_dissem_controls().into_boxed_slice();
    out.rel_to = ctx.expected_rel_to().into_boxed_slice();
    out.declassify_on = ctx.expected_declassify_on().map(Into::into);
    out.declass_exemption = ctx.expected_declass_exemption();
    let (non_ic, _needs_nf) = ctx.expected_non_ic_dissem();
    out.non_ic_dissem = non_ic.into_boxed_slice();

    out
}

// ---------------------------------------------------------------------------
// CapcoScheme — the trait implementation
// ---------------------------------------------------------------------------

/// CAPCO's implementation of `MarkingScheme`.
///
/// Stateless; construct with `CapcoScheme::new()` and pass into the
/// engine. Phase A's engine doesn't consume the trait yet — this impl
/// exists so the equivalence tests can run.
pub struct CapcoScheme {
    categories: Vec<Category>,
    constraints: Vec<Constraint>,
    templates: Vec<Template>,
    page_rewrites: Vec<PageRewrite<CapcoScheme>>,
}

impl Default for CapcoScheme {
    fn default() -> Self {
        Self::new()
    }
}

impl CapcoScheme {
    pub fn new() -> Self {
        Self {
            categories: Self::build_categories(),
            constraints: Self::build_constraints(),
            templates: Vec::new(), // Phase A does not model templates yet
            page_rewrites: Self::build_page_rewrites(),
        }
    }

    /// Construct CAPCO's `PageRewrite` table per §7a of the Phase B
    /// design doc and Phase 3 T034.
    ///
    /// Three declarative rewrites:
    ///
    /// 1. **`capco/noforn-clears-rel-to`** — when NOFORN is present in
    ///    the aggregated dissem category, the REL TO category clears.
    ///    Cite §F.2 p43.
    /// 2. **`capco/joint-promotion`** — JOINT-country lists promote
    ///    into REL TO. Subsumes the E014 `JointRelToRule` requirement
    ///    logic. Cite §K.2.
    /// 3. **`capco/fgi-absorption`** — FGI tokens roll up from
    ///    portions into the banner-level FGI category. Cite §K p61.
    ///
    /// Actions are expressed declaratively
    /// ([`CategoryAction::Clear`] / [`Promote`]). Two of the three
    /// rewrites currently use a `CategoryPredicate::Custom(never_fires)`
    /// trigger as a placeholder — Phase 3 does not drive page roll-up
    /// through `scheme.project()`, so pinning the trigger to `false`
    /// keeps those rewrites from mutating markings via the
    /// placeholder `identity_promote` transform. Scheme-exploration
    /// tools can still inspect `reads` / `writes` and the action
    /// variant without running the trigger body; Phase D / Phase E
    /// replaces the `Custom` triggers with real presence predicates.
    ///
    /// Phase 3 note: [`Engine::lint`] does not drive portion
    /// aggregation through `scheme.project(Scope::Page, …)` yet — the
    /// engine still consults [`PageContext`]'s hand-coded aggregator
    /// directly, so these rewrites are *declarative data* for the
    /// scheduler + catalog surface. The scheduler uses `reads` /
    /// `writes` to validate dataflow ordering (T031–T032); runtime
    /// dispatch is a Phase D / Phase E follow-up.
    ///
    /// [`CategoryPredicate::Contains`]: marque_scheme::CategoryPredicate::Contains
    /// [`Empty`]: marque_scheme::CategoryPredicate::Empty
    /// [`CategoryAction::Clear`]: marque_scheme::CategoryAction::Clear
    /// [`Promote`]: marque_scheme::CategoryAction::Promote
    /// [`Engine::lint`]: marque_engine::Engine::lint
    fn build_page_rewrites() -> Vec<PageRewrite<CapcoScheme>> {
        // `capco/noforn-clears-rel-to` reads `CAT_DISSEM` to look for
        // NOFORN and writes `CAT_REL_TO` to clear it. It also reads
        // `CAT_REL_TO` explicitly so the scheduler orders it AFTER
        // any rewrite that writes REL TO — e.g.,
        // `capco/joint-promotion` promotes JOINT country lists into
        // REL TO, and the NOFORN clear must see those countries
        // before deciding whether to drop them. Without this
        // read-edge, JOINT-promotion could run after the clearer
        // and reintroduce REL TO entries that should have been
        // dropped.
        //
        // (REL TO appearing as its own category — rather than as a
        // dissem-control subtype — is an artifact of `IsmAttributes`
        // modeling country-list resolution separately; the rewrite
        // semantics treat it as a first-class category that
        // producers can write.)
        const NF_READS: &[marque_scheme::CategoryId] = &[CAT_DISSEM, CAT_REL_TO];
        const NF_WRITES: &[marque_scheme::CategoryId] = &[CAT_REL_TO];

        const JP_READS: &[marque_scheme::CategoryId] = &[CAT_JOINT_CLASSIFICATION];
        const JP_WRITES: &[marque_scheme::CategoryId] = &[CAT_REL_TO];

        const FA_READS: &[marque_scheme::CategoryId] = &[CAT_FGI_MARKER];
        const FA_WRITES: &[marque_scheme::CategoryId] = &[CAT_FGI_MARKER];

        vec![
            PageRewrite::declarative(
                "capco/noforn-clears-rel-to",
                "CAPCO-2016 §F.2 p43",
                CategoryPredicate::Contains {
                    category: CAT_DISSEM,
                    token: TOK_NOFORN,
                },
                CategoryAction::Clear {
                    category: CAT_REL_TO,
                },
                NF_READS,
                NF_WRITES,
            ),
            // JOINT-promotion: JOINT countries promote into REL TO.
            // The trigger is a `Custom` `never_fires` predicate for
            // Phase 3 — runtime dispatch stays with [`PageContext`],
            // and we do not want scheme.project() to accidentally
            // mutate a marking via the placeholder `identity_promote`
            // transform. `reads` / `writes` are the scheduler-visible
            // dataflow annotations; they are what the Kahn sort
            // consumes. Phase D / Phase E replaces `never_fires` with
            // a real presence predicate and supplies a real transform.
            PageRewrite::custom(
                "capco/joint-promotion",
                "CAPCO-2016 §K.2",
                CategoryPredicate::Custom(never_fires),
                CategoryAction::Promote {
                    from: CAT_JOINT_CLASSIFICATION,
                    to: CAT_REL_TO,
                    transform: identity_promote,
                },
                JP_READS,
                JP_WRITES,
            ),
            // FGI-absorption: FGI tokens roll up from portions into
            // the banner-level FGI category. Self-read / self-write —
            // the scheduler sees the intra-category dataflow. Trigger
            // is `never_fires` for the same reason as joint-promotion.
            PageRewrite::custom(
                "capco/fgi-absorption",
                "CAPCO-2016 §K p61",
                CategoryPredicate::Custom(never_fires),
                CategoryAction::Promote {
                    from: CAT_FGI_MARKER,
                    to: CAT_FGI_MARKER,
                    transform: identity_promote,
                },
                FA_READS,
                FA_WRITES,
            ),
        ]
    }

    /// Build the scheme's category table.
    ///
    /// (U) The IC marking system has nine categories of classification and control markings:
    /// 1. US Classification Markings
    /// 2. Non-US Protective Markings
    /// 3. Joint Classification Markings
    /// 4. Sensitive Compartmented Information (SCI) Control System Markings – used by the IC to identify information that has special access requirements not met by classification level, alone
    /// 5. Special Access Program (SAP) Markings – used primarily by non-IC departments and agencies to identify information that has special access requirements not met by classification level, alone
    /// 6. Atomic Energy Act (AEA) Information Markings – used to identify information regarding nuclear matters
    /// 7. Foreign Government Information (FGI) Markings – used to identify information from a foreign source
    /// 8. Dissemination Control Marking – IC markings used to identify the expansion or limitation on distribution
    /// 9. Non-Intelligence Community Dissemination Control Markings – non-IC markings used to identify the expansion or limitation on further distribution
    fn build_categories() -> Vec<Category> {
        vec![
            // US classifications are a core category with a well-defined hierarchy, so `Max` is the natural aggregation.
            // NOTE: `Classification` includes 3 distinct categories that cannot co-occur in the same portion or banner:
            //  - U.S. classification level (e.g. CONFIDENTIAL, SECRET, TOP SECRET) or UNCLASSIFIED (if no classification)
            //  - Non-U.S. classification (e.g. //GBR SECRET, //CAN CONFIDENTIAL, //NATO UNCLASSIFIED etc.).  Non-U.S. classification may also be `RESTRICTED`, between UNCLASSIFIED and CONFIDENTIAL.
            //  - JOINT classification (e.g. //JOINT USA CAN SECRET, //JOINT USA DEU FRA CONFIDENTIAL, etc.) JOINT must always include a REL TO dissemination control that minimally includes the JOINT members (e.g. //JOINT USA CAN SECRET must have at least USA and CAN in REL TO) resulting in: `//JOINT USA CAN SECRET//REL TO USA, CAN` or as a portion `(//JOINT USA CAN S//REL TO USA, CAN)`
            //
            // **A marking can only include one of these three categories** -- they are mutually exclusive.
            //
            // In banner rollup (and rarely in portions), if any portion carries a U.S. classification, the non-U.S. JOINT members and non-U.S. origin countries are moved to the FGI category in the banner as a flat union (with a caveat, see FGI)
            //
            // A simple way to think about non-U.S. and JOINT classifications beginning with `//` is that it indicates the separation of the occluded U.S. classification category
            // It's the category separator that is still required to separate from the 'invisible' U.S. classification category that precedes it.
            Category {
                id: CAT_CLASSIFICATION,
                name: "classification",
                ordering_rank: 0,
                cardinality: Cardinality::One,
                aggregation: AggregationOp::Max,
                intra_ordering: IntraOrdering::AsWritten,
                expansion: None,
            },
            // Non-US classification
            // NATO information falls into this category but has its own tokens
            //   (e.g. //NATO COSMIC TOP SECRET, (//CTS), //NATO SECRET, (//NS), etc.)
            Category {
                id: CAT_NON_US_CLASSIFICATION,
                name: "non_us_classification",
                ordering_rank: 5,
                cardinality: Cardinality::One,
                aggregation: AggregationOp::Max,
                intra_ordering: IntraOrdering::AsWritten,
                expansion: None,
            },
            // JOINT classification connotes that each partner produced the information jointly and has a stake in its protection.
            Category {
                id: CAT_JOINT_CLASSIFICATION,
                name: "joint_classification",
                ordering_rank: 6,
                cardinality: Cardinality::One,
                aggregation: AggregationOp::Max,
                intra_ordering: IntraOrdering::AsWritten,
                expansion: None,
            },
            // SCI is plain union. It can be complicated by compartments
            // and subcompartments. There can be multiple of both compartments and subcompartments.
            // The relationships are hierarchical (i.e. SCI Control -> Compartment --> Subcompartment), and the rollup
            // preserves that hierarchy.
            // CAPCO names several Controls, some compartments and subcompartments. These are the most common ones,
            // but all three levels can have agency or program specific extensions that the scheme must support without requiring code changes.
            // There are some rules to these extensions:
            //  - Controls in their most-common abbreviated form are never more than 3 characters (e.g. HCS, SI, TK, etc.)
            Category {
                id: CAT_SCI,
                name: "sci",
                ordering_rank: 10,
                cardinality: Cardinality::Many,
                aggregation: AggregationOp::Union,
                intra_ordering: IntraOrdering::NumericThenAlpha,
                expansion: None,
            },
            Category {
                id: CAT_SAR,
                name: "sar",
                ordering_rank: 20,
                cardinality: Cardinality::Optional,
                // SAR rollup is structural (programs carry
                // compartments, compartments carry sub-compartments per
                // §H.5) and not expressible as a flat token union. Flag
                // as `Custom` so Phase B leaves
                // `PageContext::expected_sar_marking` in place rather
                // than substituting a naive union reducer.
                aggregation: AggregationOp::Custom,
                intra_ordering: IntraOrdering::NumericThenAlpha,
                expansion: None,
            },
            Category {
                id: CAT_AEA,
                name: "aea",
                ordering_rank: 30,
                cardinality: Cardinality::Many,
                // AEA rollup is not a plain union: RD precedes FRD and
                // TFNI (RD absorbs FRD when both are present), SIGMA
                // compartments merge numerically across RD blocks, and
                // UCNI drops in classified documents. Flag as `Custom`
                // so Phase B does not silently replace
                // `PageContext::expected_aea_markings` with a naive
                // union reducer.
                aggregation: AggregationOp::Custom,
                intra_ordering: IntraOrdering::AsWritten,
                expansion: None,
            },
            Category {
                id: CAT_FGI_MARKER,
                name: "fgi_marker",
                ordering_rank: 40,
                cardinality: Cardinality::Optional,
                // FGI rollup has non-trivial semantics: source-concealed
                // FGI supersedes source-acknowledged FGI (revealing the
                // country list would compromise the concealed source),
                // and the marker changes shape when multiple origin
                // countries contribute. `AggregationOp::Custom` flags
                // this for Phase B so the engine does not silently
                // replace `PageContext::expected_fgi_marker` with a
                // plain union.
                //
                // When multiple source-acknowledged FGIs combine, they
                // are a space delimited union in alphabetical order.
                // When a JOINT marker is superseded by a U.S. classification
                // The non-U.S. JOINT members are moved to the FGI marker.
                //
                // NOTE: The FGI category indicates *origin* and says nothing
                // about *releasability*. FGI should still propagate with NOFORN
                // and some FGI *originates* as NOFORN. Meaning the country
                // requested the information *not* get shared back to them
                // (i.e. to another part of their government)
                aggregation: AggregationOp::Custom,
                intra_ordering: IntraOrdering::Alphabetical,
                expansion: None,
            },
            Category {
                id: CAT_DISSEM,
                name: "dissem",
                ordering_rank: 50,
                cardinality: Cardinality::Many,
                // Plain union at category granularity. NOFORN ⊐ REL TO
                // is a *cross*-category supersession — NOFORN lives in
                // dissem, REL TO in `rel_to` — and
                // `UnionWithSupersession` is only expressive within a
                // single category's token set. The cross-category
                // supersession is enforced today by
                // `PageContext::expected_rel_to()` (which clears REL TO
                // when any NOFORN is present) and by the
                // `Constraint::Conflicts(NOFORN, REL_TO)` check below.
                // Phase C will model cross-category supersession
                // explicitly (e.g. as a new `Constraint::Supersedes`
                // variant that spans categories).
                aggregation: AggregationOp::Union,
                intra_ordering: IntraOrdering::Alphabetical,
                expansion: None,
            },
            // NOTE: REL TO is not its own category; it's a dissemination control.
            // IsmAttributes models it as a separate field because it's a list of countries that must be compared as a set for supersession and conflict rules.
            // The list is comma delimited and may consist of country trigraphs or organizational/operational tetragraphs (e.g. FVEY, NATO).
            // USA **must** always be present and first, other entries are alphabetical.
            Category {
                id: CAT_REL_TO,
                name: "rel_to",
                ordering_rank: 60,
                cardinality: Cardinality::Many,
                aggregation: AggregationOp::Intersect,
                intra_ordering: IntraOrdering::FixedFirst {
                    first: TOK_USA,
                    rest: Box::new(IntraOrdering::Alphabetical),
                },
                // Phase A leaves the expansion table empty; Phase B
                // wires the FVEY/NATO/ACGU → {USA, GBR, ...} map in.
                expansion: None,
            },
            Category {
                id: CAT_DECLASSIFY_ON,
                name: "declassify_on",
                ordering_rank: 70,
                cardinality: Cardinality::Optional,
                aggregation: AggregationOp::MaxDate,
                intra_ordering: IntraOrdering::AsWritten,
                expansion: None,
            },
        ]
    }

    fn build_constraints() -> Vec<Constraint> {
        // The Phase 3 constraint catalog lists every CAPCO
        // declarative invariant visible to tooling. Constraints here
        // are *data for inspection* — the engine's diagnostic stream
        // still comes from hand-written rules in `crate::rules` so
        // that Phase 3 preserves byte-identical corpus output. Phase
        // 4 / Phase E wires runtime evaluation through these
        // declarations via the generic evaluator
        // ([`marque_scheme::constraint::evaluate`]).
        //
        // Each entry carries a `label` pointing at the authoritative
        // CAPCO-2016 passage; citations are re-verified at commit
        // time per Constitution VIII.
        vec![
            // E010 — bare HCS is legacy; HCS requires a qualifying
            // variant (HCS-P or HCS-O) plus the subsystem rules in
            // §4 p62. Dispatched via `Constraint::Custom` because the
            // set is n-ary (bare-HCS detection + O/P rules + ORCON
            // pairing + classification floor).
            Constraint::Custom {
                name: "E010/HCS-system-constraints",
                label: "CAPCO-2016 §4 p62",
            },
            // E012 — a US classification cannot co-occur with a
            // concurrent foreign classification (FGI / NATO / JOINT)
            // on the same marking.
            Constraint::Conflicts {
                name: "E012/us-conflicts-non-us-classification",
                left: TokenRef::Token(TOK_US_CLASSIFIED),
                right: TokenRef::AnyInCategory(CAT_NON_US_CLASSIFICATION),
                label: "CAPCO-2016 §I.3",
            },
            // E015 — non-US classifications (FGI / NATO / JOINT) must
            // carry a dissemination control (REL TO or an IC-approved
            // dissem marker).
            Constraint::Requires {
                name: "E015/non-us-requires-dissem",
                left: TokenRef::AnyInCategory(CAT_NON_US_CLASSIFICATION),
                right: TokenRef::AnyInCategory(CAT_DISSEM),
                label: "CAPCO-2016 §K p61",
            },
            // E016 — JOINT classifications cannot co-occur with
            // RESTRICTED (JOINT has its own classification floor that
            // RESTRICTED is below).
            Constraint::Conflicts {
                name: "E016/joint-conflicts-restricted",
                left: TokenRef::Token(TOK_JOINT),
                right: TokenRef::Token(TOK_RESTRICTED),
                label: "CAPCO-2016 §K.2",
            },
            // E017 — JOINT cannot co-occur with an explicit FGI
            // marker (JOINT already enumerates origin; FGI would
            // double-mark).
            Constraint::Conflicts {
                name: "E017/joint-conflicts-fgi",
                left: TokenRef::Token(TOK_JOINT),
                right: TokenRef::Token(TOK_FGI_MARKER),
                label: "CAPCO-2016 §K.2",
            },
            // E018 — JOINT conflicts with IC dissemination controls
            // other than REL TO (which is required by §K.2).
            Constraint::Conflicts {
                name: "E018/joint-conflicts-ic-dissem",
                left: TokenRef::Token(TOK_JOINT),
                right: TokenRef::Token(TOK_IC_DISSEM),
                label: "CAPCO-2016 §K.2 p66",
            },
            // E019 — JOINT conflicts with non-IC dissemination
            // controls (LIMDIS / SBU / etc.).
            Constraint::Conflicts {
                name: "E019/joint-conflicts-non-ic-dissem",
                left: TokenRef::Token(TOK_JOINT),
                right: TokenRef::Token(TOK_NON_IC_DISSEM),
                label: "CAPCO-2016 §K.2 p66",
            },
            // E021 — AEA tokens (RD / FRD) require NOFORN by default
            // per §H.1.
            Constraint::Requires {
                name: "E021/aea-requires-noforn",
                left: TokenRef::AnyInCategory(CAT_AEA),
                right: TokenRef::Token(TOK_NOFORN),
                label: "CAPCO-2016 §H.1",
            },
            // E022 — CNWDI has a classification floor of TS or S
            // (implication expressed as a Custom constraint because
            // the floor is an enum-range rather than a single token).
            Constraint::Custom {
                name: "E022/CNWDI-classification-floor",
                label: "CAPCO-2016 §H.1",
            },
            // E024 — RD supersedes FRD and TFNI when both appear in
            // the AEA set. Declared as two `Supersedes` entries so
            // catalog consumers see the full supersession relation.
            Constraint::Supersedes {
                name: "E024/rd-supersedes-frd",
                left: TokenRef::Token(TOK_RD),
                right: TokenRef::Token(TOK_FRD),
                label: "CAPCO-2016 §H.1",
            },
            Constraint::Supersedes {
                name: "E024/rd-supersedes-tfni",
                left: TokenRef::Token(TOK_RD),
                right: TokenRef::Token(TOK_TFNI),
                label: "CAPCO-2016 §H.1",
            },
            // E025 — UCNI conflicts with classified markings (it is
            // unclassified-but-controlled and cannot coexist with a
            // classification level).
            Constraint::Conflicts {
                name: "E025/ucni-conflicts-classification",
                left: TokenRef::Token(TOK_UCNI),
                right: TokenRef::AnyInCategory(CAT_CLASSIFICATION),
                label: "CAPCO-2016 §H.1",
            },
            // W002 — a US classification alongside an FGI marker is
            // a warning-level co-mingling event (§K.2 documents the
            // legal commingling rules; `CominglingWarningRule`
            // surfaces the warning).
            Constraint::Conflicts {
                name: "W002/us-commingled-with-fgi",
                left: TokenRef::Token(TOK_US_CLASSIFIED),
                right: TokenRef::Token(TOK_FGI_MARKER),
                label: "CAPCO-2016 §K.2",
            },
            // Existing Phase B sample constraints — retained so the
            // scheme_equivalence tests that read the catalog keep
            // their anchors. The three constraints below are
            // **already implemented** as declarative + Custom entries;
            // Phase 3 added the 12 above for catalog completeness.

            // NOFORN ∥ REL TO — portion-level exclusion (§A.4).
            Constraint::Conflicts {
                name: "capco/noforn-conflicts-rel-to",
                left: TokenRef::Token(TOK_NOFORN),
                right: TokenRef::AnyInCategory(CAT_REL_TO),
                label: "CAPCO-2016 §A.4",
            },
            // JOINT ⇒ USA — JOINT classifications must list USA in
            // both the country list and REL TO.
            Constraint::Requires {
                name: "capco/joint-requires-usa",
                left: TokenRef::Token(TOK_JOINT),
                right: TokenRef::Token(TOK_USA),
                label: "CAPCO-2016 §H.3",
            },
        ]
    }
}

/// Parse errors surfaced by `CapcoScheme::parse`.
///
/// Phase A does not actually parse through the trait — callers continue
/// to use `marque_core::Parser` directly — so `parse()` unconditionally
/// returns [`CapcoParseError::NotImplemented`]. Phase B/E will wrap
/// `marque-core`'s `CoreError` here once parsing is routed through the
/// scheme trait (and the `(C)` ambiguity surface lands).
#[derive(Debug)]
pub enum CapcoParseError {
    /// `CapcoScheme::parse` is intentionally unimplemented in Phase A.
    /// Use `marque_core::Parser` for actual parsing until Phase B/E
    /// routes it through the scheme trait.
    NotImplemented,
}

// Phase 3 drift-hazard note (prerequisite for T035 / Phase D-E):
//
// `satisfies` and `evaluate_custom` are deliberately left at their
// trait defaults on `CapcoScheme` — the declarative constraint
// catalog in `build_constraints()` is **data-only** until T035 or
// Phase D/E rewires the engine to drive diagnostics through
// `marque_scheme::constraint::evaluate(scheme, marking)`. The live
// HCS / dyadic predicates still fire through `CapcoScheme::validate`'s
// hand-coded `match` below (which bypasses the evaluator) plus the
// hand-written `Rule` impls in `crate::rules`, which is why byte-
// identity with the pre-branch corpus holds in this phase.
//
// BEFORE retiring any rule impl (T035) or switching the engine to
// `scheme.validate()` (Phase D/E), override `satisfies` to resolve
// `TokenRef::Token` / `TokenRef::AnyInCategory` against `CapcoMarking`'s
// concrete storage and override `evaluate_custom` to route
// `"HCS-system-constraints"` / `"CNWDI-classification-floor"` to their
// scheme-specific predicates. Until then, calling
// `marque_scheme::constraint::evaluate(&CapcoScheme::new(), &m)`
// returns an empty vec — every dyadic constraint no-ops and every
// `Custom` entry drops silently. Leaving the defaults unoverridden
// would have been a bug if Phase 3 also flipped the engine; the two
// changes must land together.
impl MarkingScheme for CapcoScheme {
    type Token = marque_scheme::TokenId;
    type Marking = CapcoMarking;
    type ParseError = CapcoParseError;

    fn name(&self) -> &str {
        "CAPCO-ISM"
    }

    fn schema_version(&self) -> &str {
        crate::SCHEMA_VERSION
    }

    fn categories(&self) -> &[Category] {
        &self.categories
    }

    fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }

    fn templates(&self) -> &[Template] {
        &self.templates
    }

    fn parse(&self, _input: &str) -> Result<Parsed<Self::Marking>, Self::ParseError> {
        // Phase A: the trait impl exists to validate the abstraction's
        // shape against CAPCO. Callers continue to use
        // `marque_core::Parser` directly. Phase B/E tie parse() into
        // the engine once the ambiguity resolver lands.
        Err(CapcoParseError::NotImplemented)
    }

    fn validate(&self, m: &Self::Marking) -> Vec<ConstraintViolation> {
        let mut out = Vec::new();
        let attrs = &m.0;

        // Walk the declarative constraints. Token-id lookups are
        // sentinel-based for Phase A — only the three sample
        // constraints are exercised.
        for c in &self.constraints {
            match c {
                Constraint::Conflicts {
                    left: TokenRef::Token(a),
                    right: TokenRef::AnyInCategory(cat),
                    label,
                    ..
                } if *a == TOK_NOFORN && *cat == CAT_REL_TO => {
                    let has_nf = attrs
                        .dissem_controls
                        .iter()
                        .any(|d| matches!(d, marque_ism::DissemControl::Nf));
                    let has_rel_to = !attrs.rel_to.is_empty();
                    if has_nf && has_rel_to {
                        out.push(ConstraintViolation {
                            constraint_label: "NOFORN∥REL TO",
                            message: "NOFORN and REL TO cannot co-occur in one portion marking"
                                .to_owned(),
                            citation: label,
                        });
                    }
                }
                Constraint::Custom { name, label }
                    if *name == "E010/HCS-system-constraints"
                        || *name == "HCS-system-constraints" =>
                {
                    out.extend(hcs_system_constraints(attrs, label));
                }
                Constraint::Requires {
                    left: TokenRef::Token(a),
                    right: TokenRef::Token(b),
                    label,
                    ..
                } if *a == TOK_JOINT && *b == TOK_USA => {
                    if let Some(marque_ism::MarkingClassification::Joint(ref j)) =
                        attrs.classification
                    {
                        let has_usa_reltop = attrs.rel_to.contains(&Trigraph::USA);
                        let joint_includes_usa = j.countries.contains(&Trigraph::USA);
                        if !has_usa_reltop || !joint_includes_usa {
                            out.push(ConstraintViolation {
                                constraint_label: "JOINT⇒USA",
                                message: "JOINT classifications must list USA in both the \
                                          classification countries and REL TO"
                                    .to_owned(),
                                citation: label,
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        out
    }

    fn project(&self, scope: Scope, markings: &[Self::Marking]) -> Self::Marking {
        match scope {
            Scope::Portion => {
                // Identity under portion scope: if the caller passed a
                // single marking we return it; empty → bottom.
                markings
                    .first()
                    .cloned()
                    .unwrap_or_else(|| CapcoMarking(IsmAttributes::default()))
            }
            Scope::Page | Scope::Document | Scope::Diff => {
                // Page / Document rollup: drive through the existing
                // `PageContext` aggregator (which is already
                // category-component-wise), then apply page rewrites.
                //
                // Byte-identical equivalence with `PageContext` is the
                // Phase B verification gate — see the
                // `scheme_equivalence.rs` tests. When CAPCO's categories
                // move to individual `impl Lattice` types in their own
                // right (Phase C continuation), this implementation
                // swaps in the category-wise composition directly
                // without changing the outward contract.
                let mut ctx = PageContext::new();
                for p in markings {
                    ctx.add_portion(p.0.clone());
                }
                let mut out = CapcoMarking(page_context_to_attrs(&ctx));
                // Apply declarative page rewrites. `PageContext`
                // already applies NOFORN-clears-REL-TO internally, so
                // the rewrite is effectively a no-op on today's
                // storage — but declaring it here makes the semantic
                // inspectable per §7a.
                for rw in &self.page_rewrites {
                    let fires = match &rw.trigger {
                        CategoryPredicate::Contains { category, token } => {
                            capco_category_contains(&out, *category, *token)
                        }
                        CategoryPredicate::Empty { category } => {
                            !capco_category_has_values(&out, *category)
                        }
                        CategoryPredicate::Custom(f) => f(&out),
                    };
                    if fires {
                        match &rw.action {
                            CategoryAction::Clear { category } => {
                                capco_category_clear(&mut out, *category);
                            }
                            CategoryAction::Replace { category, with } => {
                                capco_category_replace(&mut out, *category, with);
                            }
                            CategoryAction::Promote { from: _, to: _, .. } => {
                                // Phase 3 T034 declares the JOINT-
                                // promotion and FGI-absorption rewrites
                                // for the scheduler + catalog surface,
                                // but runtime dispatch stays with
                                // [`PageContext`] (engine.lint does not
                                // drive aggregation through project()
                                // yet — see the note on
                                // `build_page_rewrites`). Treat
                                // `Promote` as a no-op for now; full
                                // transform-driven dispatch lands in
                                // Phase D / Phase E when the engine
                                // switches to scheme-driven roll-up.
                            }
                            CategoryAction::Custom(f) => f(&mut out),
                        }
                    }
                }
                out
            }
        }
    }

    fn page_rewrites(&self) -> &[PageRewrite<Self>] {
        &self.page_rewrites
    }

    fn render_portion(&self, m: &Self::Marking) -> String {
        // Phase A: render only the classification level — enough to
        // exercise the trait method. Full renderer is Phase B.
        match &m.0.classification {
            Some(c) => c.effective_level().portion_str().to_owned(),
            None => String::new(),
        }
    }

    fn render_banner(&self, m: &Self::Marking) -> String {
        match &m.0.classification {
            Some(c) => c.effective_level().banner_str().to_owned(),
            None => String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// HCS constraint handler (CAPCO 2016 §4 p62)
// ---------------------------------------------------------------------------

/// Evaluate the `Constraint::Custom("HCS-system-constraints")` sample.
///
/// CAPCO 2016 §4 (p62) defines four interlocking HCS rules:
///
/// 1. **Bare `HCS` (no compartment)** is a legacy form. It must be
///    remarked to `HCS-P`, `HCS-O`, or `HCS-O-P`, which requires
///    document-level analysis (the correct variant depends on whether
///    the content is HUMINT product, operations, or both). Legacy
///    `C//HCS` (CONFIDENTIAL with bare HCS -- no compartment) must additionally be
///    identified to the originator for correction.
/// 2. **`HCS-O`** requires ORCON and must **not** include ORCON-USGOV (banner would drop -USGOV).
/// 3. **`HCS-P`** requires **either** ORCON or ORCON-USGOV.
/// 4. **`HCS-O` / `HCS-P`** are only authorized for SECRET and TOP
///    SECRET classifications.
///
/// This helper inspects both `sci_controls` (the CVE-projection for
/// legacy-shape bare HCS tokens) and `sci_markings` (the structural
/// view that carries compartment identifiers). Emits one
/// `ConstraintViolation` per failing rule per offending HCS entry.
///
/// By far the most common HCS compartment is `HCS-P` (Product).
/// HCS-O (Operations) is rarely encountered outside of CIA's walls.
/// But for users in that environment, they may encounter all three variants routinely.
fn hcs_system_constraints(
    attrs: &marque_ism::IsmAttributes,
    citation: &'static str,
) -> Vec<marque_scheme::ConstraintViolation> {
    use marque_ism::{DissemControl, SciControl, SciControlBare, SciControlSystem};

    let mut out = Vec::new();

    let classification = attrs.us_classification();
    let has_orcon = attrs.dissem_controls.contains(&DissemControl::Oc);
    let has_orcon_usgov = attrs.dissem_controls.contains(&DissemControl::OcUsgov);
    let high_enough = matches!(
        classification,
        Some(Classification::Secret) | Some(Classification::TopSecret)
    );

    // Walk structural sci_markings for HCS systems. This is the
    // authoritative source for the compartment identifier.
    for marking in attrs.sci_markings.iter() {
        let is_hcs = matches!(
            marking.system,
            SciControlSystem::Published(SciControlBare::Hcs)
        );
        if !is_hcs {
            continue;
        }

        if marking.compartments.is_empty() {
            // Bare HCS — legacy per CAPCO 2016 §4 p62.
            out.push(marque_scheme::ConstraintViolation {
                constraint_label: "HCS-legacy-bare",
                message:
                    "Bare HCS is legacy; remark to HCS-P, HCS-O, or HCS-O-P per CAPCO 2016 §4 \
                     p62 (requires document-level analysis)."
                        .to_owned(),
                citation,
            });
            if classification == Some(Classification::Confidential) {
                out.push(marque_scheme::ConstraintViolation {
                    constraint_label: "HCS-legacy-confidential",
                    message: "Legacy CONFIDENTIAL//HCS: identify to originator for correction \
                              per CAPCO 2016 §4."
                        .to_owned(),
                    citation,
                });
            }
            continue;
        }

        // For each HCS-{first compartment} variant, apply the O/P
        // specific rules and the SECRET / TOP SECRET floor.
        for comp in marking.compartments.iter() {
            let id = comp.identifier.as_ref();
            match id {
                "O" => {
                    if !high_enough {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-O-classification-floor",
                            message: "HCS-O is only authorized for SECRET and TOP SECRET per \
                                      CAPCO 2016 §4."
                                .to_owned(),
                            citation,
                        });
                    }
                    if !has_orcon {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-O-requires-ORCON",
                            message: "HCS-O requires ORCON per CAPCO 2016 §4.".to_owned(),
                            citation,
                        });
                    }
                    if has_orcon_usgov {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-O-forbids-ORCON-USGOV",
                            message: "HCS-O must not be used with ORCON-USGOV per CAPCO \
                                      2016 §4."
                                .to_owned(),
                            citation,
                        });
                    }
                }
                "P" => {
                    if !high_enough {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-P-classification-floor",
                            message: "HCS-P is only authorized for SECRET and TOP SECRET per \
                                      CAPCO 2016 §4."
                                .to_owned(),
                            citation,
                        });
                    }
                    if !has_orcon && !has_orcon_usgov {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-P-requires-ORCON-or-ORCON-USGOV",
                            message: "HCS-P requires either ORCON or ORCON-USGOV per CAPCO \
                                      2016 §4."
                                .to_owned(),
                            citation,
                        });
                    }
                }
                _ => {
                    // Other HCS compartments (e.g., agency-specific
                    // extensions not yet in this sample) fall through.
                }
            }
        }
    }

    // Back-compat: a portion may carry `SciControl::Hcs` (the CVE
    // projection for bare HCS) without producing a `sci_markings`
    // entry in every test path. Treat a bare `SciControl::Hcs` in the
    // projection but no corresponding `sci_markings` entry as legacy
    // bare HCS too. This keeps the handler robust to the two-path
    // storage (CVE enum vs structural) that `IsmAttributes` carries
    // for back-compat — see crate-level docs on the hybrid SCI model.
    let structural_has_hcs = attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs)));
    let projection_has_bare_hcs = attrs
        .sci_controls
        .iter()
        .any(|s| matches!(s, SciControl::Hcs));
    if projection_has_bare_hcs && !structural_has_hcs {
        out.push(marque_scheme::ConstraintViolation {
            constraint_label: "HCS-legacy-bare",
            // suggested fix should be HCS-P but we should expose a default override path for users in the HCS-O environment
            message: "HCS requires a compartment (O or P); remark to HCS-P, HCS-O, or HCS-O-P per CAPCO 2016 §4 \
                 p62 (requires document-level analysis)."
                .to_owned(),
            citation,
        });
        if classification == Some(Classification::Confidential) {
            out.push(marque_scheme::ConstraintViolation {
                constraint_label: "HCS-legacy-confidential",
                message: "Legacy CONFIDENTIAL//HCS: identify to originator for correction per \
                          CAPCO 2016 §4."
                    .to_owned(),
                citation,
            });
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Convenience: expose the classification level for test assertions
// ---------------------------------------------------------------------------

impl CapcoMarking {
    /// The effective US classification level, if any. Thin shim over
    /// `IsmAttributes::us_classification` for test readability.
    #[inline]
    pub fn classification(&self) -> Option<Classification> {
        self.0.us_classification()
    }
}

#[cfg(test)]
impl CapcoScheme {
    /// Test-only constructor that lets tests install arbitrary
    /// `PageRewrite` entries, exercising the declarative dispatch
    /// path (`CategoryPredicate::Contains` / `Empty`,
    /// `CategoryAction::Clear` / `Replace`) with test-provided
    /// rewrites.
    pub(crate) fn with_rewrites(rewrites: Vec<PageRewrite<CapcoScheme>>) -> Self {
        Self {
            categories: Self::build_categories(),
            constraints: Self::build_constraints(),
            templates: Vec::new(),
            page_rewrites: rewrites,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use marque_ism::{DissemControl, IsmAttributes, MarkingClassification, Trigraph};

    fn mk_attrs() -> IsmAttributes {
        let mut a = IsmAttributes::default();
        a.classification = Some(MarkingClassification::Us(Classification::Secret));
        a
    }

    // capco_category_contains — all branches

    #[test]
    fn category_contains_detects_noforn_in_dissem() {
        let mut a = mk_attrs();
        a.dissem_controls = vec![DissemControl::Nf].into();
        let m = CapcoMarking(a);
        assert!(capco_category_contains(&m, CAT_DISSEM, TOK_NOFORN));
    }

    #[test]
    fn category_contains_returns_false_on_absent_token() {
        let a = mk_attrs();
        let m = CapcoMarking(a);
        assert!(!capco_category_contains(&m, CAT_DISSEM, TOK_NOFORN));
    }

    #[test]
    fn category_contains_returns_false_for_unhandled_pair() {
        let a = mk_attrs();
        let m = CapcoMarking(a);
        // An unhandled (category, token) pair — should be false.
        assert!(!capco_category_contains(&m, CAT_REL_TO, TOK_NOFORN));
        assert!(!capco_category_contains(&m, CAT_DISSEM, TOK_USA));
        assert!(!capco_category_contains(&m, CAT_SCI, TOK_NOFORN));
    }

    // capco_category_has_values — all branches

    #[test]
    fn category_has_values_rel_to_populated() {
        let mut a = mk_attrs();
        a.rel_to = vec![Trigraph::USA].into();
        let m = CapcoMarking(a);
        assert!(capco_category_has_values(&m, CAT_REL_TO));
    }

    #[test]
    fn category_has_values_rel_to_empty() {
        let a = mk_attrs();
        let m = CapcoMarking(a);
        assert!(!capco_category_has_values(&m, CAT_REL_TO));
    }

    #[test]
    fn category_has_values_dissem_populated() {
        let mut a = mk_attrs();
        a.dissem_controls = vec![DissemControl::Nf].into();
        let m = CapcoMarking(a);
        assert!(capco_category_has_values(&m, CAT_DISSEM));
    }

    #[test]
    fn category_has_values_dissem_empty() {
        let m = CapcoMarking(mk_attrs());
        assert!(!capco_category_has_values(&m, CAT_DISSEM));
    }

    #[test]
    fn category_has_values_sci_populated_via_sci_controls() {
        let mut a = mk_attrs();
        a.sci_controls = vec![marque_ism::SciControl::Si].into();
        let m = CapcoMarking(a);
        assert!(capco_category_has_values(&m, CAT_SCI));
    }

    #[test]
    fn category_has_values_sci_empty() {
        let m = CapcoMarking(mk_attrs());
        assert!(!capco_category_has_values(&m, CAT_SCI));
    }

    #[test]
    fn category_has_values_unhandled_returns_true() {
        // Unhandled categories default to true ("non-empty / unknown")
        // so `Empty` predicates on them stay inert.
        let m = CapcoMarking(mk_attrs());
        assert!(capco_category_has_values(&m, CAT_SAR));
        assert!(capco_category_has_values(&m, CAT_AEA));
        assert!(capco_category_has_values(&m, CAT_FGI_MARKER));
    }

    // capco_category_clear — all branches

    #[test]
    fn category_clear_empties_rel_to() {
        let mut a = mk_attrs();
        a.rel_to = vec![Trigraph::USA].into();
        let mut m = CapcoMarking(a);
        capco_category_clear(&mut m, CAT_REL_TO);
        assert!(m.0.rel_to.is_empty());
    }

    #[test]
    fn category_clear_empties_dissem() {
        let mut a = mk_attrs();
        a.dissem_controls = vec![DissemControl::Nf].into();
        let mut m = CapcoMarking(a);
        capco_category_clear(&mut m, CAT_DISSEM);
        assert!(m.0.dissem_controls.is_empty());
    }

    #[test]
    fn category_clear_unhandled_is_noop() {
        let mut a = mk_attrs();
        a.rel_to = vec![Trigraph::USA].into();
        let mut m = CapcoMarking(a);
        capco_category_clear(&mut m, CAT_SCI);
        // REL TO untouched — other-category clear was a no-op.
        assert_eq!(m.0.rel_to.len(), 1);
    }

    // capco_category_replace — all branches

    #[test]
    fn category_replace_rel_to_copies_from_source() {
        let mut src_attrs = IsmAttributes::default();
        src_attrs.rel_to = vec![Trigraph::USA, Trigraph::try_new(*b"GBR").unwrap()].into();
        let src = CapcoMarking(src_attrs);

        let mut dst = CapcoMarking(mk_attrs());
        capco_category_replace(&mut dst, CAT_REL_TO, &src);
        assert_eq!(dst.0.rel_to.len(), 2);
    }

    #[test]
    fn category_replace_dissem_copies_from_source() {
        let mut src_attrs = IsmAttributes::default();
        src_attrs.dissem_controls = vec![DissemControl::Nf].into();
        let src = CapcoMarking(src_attrs);

        let mut dst = CapcoMarking(mk_attrs());
        capco_category_replace(&mut dst, CAT_DISSEM, &src);
        assert_eq!(dst.0.dissem_controls.as_ref(), &[DissemControl::Nf]);
    }

    #[test]
    fn category_replace_unhandled_is_noop() {
        let src = CapcoMarking(mk_attrs());
        let mut dst = CapcoMarking(mk_attrs());
        let before = dst.clone();
        capco_category_replace(&mut dst, CAT_SCI, &src);
        assert_eq!(dst, before);
    }

    // Declarative rewrite dispatch — exercise the Contains / Empty /
    // Clear / Replace match arms inside `project`.

    #[test]
    fn project_applies_declarative_contains_then_clear() {
        // Construct a scheme with a declarative Contains-trigger +
        // Clear-action rewrite (instead of the default Custom
        // closures). That way the engine hits the Contains and Clear
        // match arms in the project() dispatch.
        let rewrites = vec![PageRewrite {
            id: "test/nf-clears-rel-to",
            citation: "test",
            trigger: CategoryPredicate::Contains {
                category: CAT_DISSEM,
                token: TOK_NOFORN,
            },
            action: CategoryAction::Clear {
                category: CAT_REL_TO,
            },
            reads: &[CAT_DISSEM],
            writes: &[CAT_REL_TO],
        }];
        let scheme = CapcoScheme::with_rewrites(rewrites);

        // Two portions: one with NOFORN, one with REL TO.
        let mut p1 = mk_attrs();
        p1.dissem_controls = vec![DissemControl::Nf].into();
        let mut p2 = mk_attrs();
        p2.rel_to = vec![Trigraph::USA, Trigraph::try_new(*b"GBR").unwrap()].into();

        let out = marque_scheme::MarkingScheme::project(
            &scheme,
            marque_scheme::Scope::Page,
            &[CapcoMarking(p1), CapcoMarking(p2)],
        );
        // Rewrite should have fired — REL TO cleared.
        assert!(out.0.rel_to.is_empty());
    }

    #[test]
    fn project_applies_declarative_empty_then_replace() {
        // An Empty trigger on an unhandled category (returns false, so
        // rewrite does NOT fire). Verify a Replace action is reachable
        // via a trigger that DOES fire.
        let mut replacement = IsmAttributes::default();
        replacement.dissem_controls = vec![DissemControl::Nf].into();

        let rewrites = vec![PageRewrite {
            id: "test/empty-rel-to-triggers-replace-dissem",
            citation: "test",
            trigger: CategoryPredicate::Empty {
                category: CAT_REL_TO,
            },
            action: CategoryAction::Replace {
                category: CAT_DISSEM,
                with: CapcoMarking(replacement),
            },
            reads: &[CAT_REL_TO],
            writes: &[CAT_DISSEM],
        }];
        let scheme = CapcoScheme::with_rewrites(rewrites);

        // Portion with no REL TO — trigger fires → dissem replaced.
        let p = mk_attrs();
        let out = marque_scheme::MarkingScheme::project(
            &scheme,
            marque_scheme::Scope::Page,
            &[CapcoMarking(p)],
        );
        assert!(out.0.dissem_controls.contains(&DissemControl::Nf));
    }
}
