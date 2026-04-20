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
// `CategoryAction::Custom` variants skip this dispatch and the
// rewrite author supplies the closure directly — §7a's NOFORN rewrite
// takes that path since it's cross-category.

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
    /// design doc. Phase B declares one rewrite:
    ///
    /// - `capco/noforn-clears-rel-to` — when NOFORN is present in the
    ///   aggregated dissem category, the REL TO category clears. This
    ///   is the same behavior [`PageContext::expected_rel_to`]
    ///   implements today; the declarative form is inspectable at
    ///   runtime (tooling can render "this page will have NOFORN⇒
    ///   REL-TO-cleared applied" without calling scheme code).
    fn build_page_rewrites() -> Vec<PageRewrite<CapcoScheme>> {
        vec![PageRewrite {
            id: "capco/noforn-clears-rel-to",
            citation: "CAPCO-2016-§H.2",
            trigger: CategoryPredicate::Custom(|m: &CapcoMarking| {
                m.0.dissem_controls
                    .iter()
                    .any(|d| matches!(d, marque_ism::DissemControl::Nf))
            }),
            action: CategoryAction::Custom(|m: &mut CapcoMarking| {
                m.0.rel_to = Box::new([]);
            }),
        }]
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
        vec![
            // Sample constraint 1: NOFORN and REL TO cannot co-occur.
            Constraint::Conflicts(
                TokenRef::Token(TOK_NOFORN),
                TokenRef::AnyInCategory(CAT_REL_TO),
            ),
            // Sample constraint 2: HCS handling per CAPCO 2016 §4
            // (p62). Expressed as `Custom` because the rule set is
            // n-ary — bare HCS is legacy and requires document-level
            // remarking; HCS-O requires ORCON and forbids ORCON-USGOV;
            // HCS-P requires ORCON or ORCON-USGOV; and HCS-O / HCS-P
            // are only authorized for SECRET and TOP SECRET. The
            // scheme's `validate` matches the label and dispatches the
            // scheme-specific predicate (see
            // [`CapcoScheme::validate`]).
            Constraint::Custom("HCS-system-constraints"),
            // Sample constraint 3: JOINT always requires USA in both
            // the JOINT classification country list and the REL TO
            // list. The declarative `Requires(JOINT, USA)` above is
            // shorthand — the scheme's `validate` dispatches it to a
            // predicate that checks both positions. Phase C will
            // express this with a richer `TokenRef` that can target a
            // specific category so the constraint is fully
            // declarative.
            Constraint::Requires(TokenRef::Token(TOK_JOINT), TokenRef::Token(TOK_USA)),
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
                Constraint::Conflicts(TokenRef::Token(a), TokenRef::AnyInCategory(cat))
                    if *a == TOK_NOFORN && *cat == CAT_REL_TO =>
                {
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
                        });
                    }
                }
                Constraint::Custom(label) if *label == "HCS-system-constraints" => {
                    out.extend(hcs_system_constraints(attrs));
                }
                Constraint::Requires(TokenRef::Token(a), TokenRef::Token(b))
                    if *a == TOK_JOINT && *b == TOK_USA =>
                {
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
            });
            if classification == Some(Classification::Confidential) {
                out.push(marque_scheme::ConstraintViolation {
                    constraint_label: "HCS-legacy-confidential",
                    message: "Legacy CONFIDENTIAL//HCS: identify to originator for correction \
                              per CAPCO 2016 §4."
                        .to_owned(),
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
                        });
                    }
                    if !has_orcon {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-O-requires-ORCON",
                            message: "HCS-O requires ORCON per CAPCO 2016 §4.".to_owned(),
                        });
                    }
                    if has_orcon_usgov {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-O-forbids-ORCON-USGOV",
                            message: "HCS-O must not be used with ORCON-USGOV per CAPCO \
                                      2016 §4."
                                .to_owned(),
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
                        });
                    }
                    if !has_orcon && !has_orcon_usgov {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-P-requires-ORCON-or-ORCON-USGOV",
                            message: "HCS-P requires either ORCON or ORCON-USGOV per CAPCO \
                                      2016 §4."
                                .to_owned(),
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
        });
        if classification == Some(Classification::Confidential) {
            out.push(marque_scheme::ConstraintViolation {
                constraint_label: "HCS-legacy-confidential",
                message: "Legacy CONFIDENTIAL//HCS: identify to originator for correction per \
                          CAPCO 2016 §4."
                    .to_owned(),
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
