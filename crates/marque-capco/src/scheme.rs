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
    AggregationOp, Cardinality, Category, CategoryId, Constraint, ConstraintViolation,
    IntraOrdering, Lattice, MarkingScheme, Parsed, Template, TokenId, TokenRef,
};

// ---------------------------------------------------------------------------
// Category ids
// ---------------------------------------------------------------------------

pub const CAT_CLASSIFICATION: CategoryId = CategoryId(1);
pub const CAT_SCI: CategoryId = CategoryId(2);
pub const CAT_SAR: CategoryId = CategoryId(3);
pub const CAT_AEA: CategoryId = CategoryId(4);
pub const CAT_FGI_MARKER: CategoryId = CategoryId(5);
pub const CAT_DISSEM: CategoryId = CategoryId(6);
pub const CAT_REL_TO: CategoryId = CategoryId(7);
pub const CAT_DECLASSIFY_ON: CategoryId = CategoryId(8);

// ---------------------------------------------------------------------------
// Sentinel token ids for constraint expressions
// ---------------------------------------------------------------------------
//
// Phase C will replace these with generated ids pointing to specific
// CVE tokens. For Phase A we only need enough ids to express the three
// sample constraints that the equivalence tests exercise.

pub const TOK_NOFORN: TokenId = TokenId(100);
pub const TOK_HCS: TokenId = TokenId(101);
pub const TOK_REL_TO_ANY: TokenId = TokenId(102);
pub const TOK_JOINT: TokenId = TokenId(103);
pub const TOK_USA: TokenId = TokenId(104);

// ---------------------------------------------------------------------------
// CapcoMarking — newtype over IsmAttributes implementing Lattice
// ---------------------------------------------------------------------------

/// CAPCO marking as viewed through the `marque-scheme` lens. A thin
/// newtype around [`IsmAttributes`] so we can hang trait impls on it
/// without orphan-rule problems.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapcoMarking(pub IsmAttributes);

impl From<IsmAttributes> for CapcoMarking {
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

/// Build an `IsmAttributes` banner projection from the `expected_*`
/// accessors on `PageContext`. Intentionally narrow: only fills the
/// fields exercised by Phase A's equivalence tests. Other fields land
/// at their defaults, which matches Phase B's goal of handing
/// everything off to scheme-driven aggregation.
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
        }
    }

    fn build_categories() -> Vec<Category> {
        vec![
            Category {
                id: CAT_CLASSIFICATION,
                name: "classification",
                ordering_rank: 0,
                cardinality: Cardinality::One,
                aggregation: AggregationOp::Max,
                intra_ordering: IntraOrdering::AsWritten,
                expansion: None,
            },
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
            // Sample constraint 2: HCS requires NOFORN.
            Constraint::Requires(TokenRef::Token(TOK_HCS), TokenRef::Token(TOK_NOFORN)),
            // Sample constraint 3: JOINT requires USA in REL TO.
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
                Constraint::Requires(TokenRef::Token(a), TokenRef::Token(b))
                    if *a == TOK_HCS && *b == TOK_NOFORN =>
                {
                    let has_hcs = attrs
                        .sci_controls
                        .iter()
                        .any(|s| matches!(s, marque_ism::SciControl::Hcs));
                    let has_nf = attrs
                        .dissem_controls
                        .iter()
                        .any(|d| matches!(d, marque_ism::DissemControl::Nf));
                    if has_hcs && !has_nf {
                        out.push(ConstraintViolation {
                            constraint_label: "HCS⇒NOFORN",
                            message: "HCS must be accompanied by NOFORN".to_owned(),
                        });
                    }
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

    fn project_banner(&self, portions: &[Self::Marking]) -> Self::Marking {
        let mut ctx = PageContext::new();
        for p in portions {
            ctx.add_portion(p.0.clone());
        }
        CapcoMarking(page_context_to_attrs(&ctx))
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
// Convenience: expose the classification level for test assertions
// ---------------------------------------------------------------------------

impl CapcoMarking {
    /// The effective US classification level, if any. Thin shim over
    /// `IsmAttributes::us_classification` for test readability.
    pub fn classification(&self) -> Option<Classification> {
        self.0.us_classification()
    }
}
