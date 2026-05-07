// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CapcoScheme` — CAPCO's implementation of the `MarkingScheme` trait.
//!
//! This is the Phase A proof that CAPCO's hand-written aggregation in
//! [`PageContext`] falls out of the generic `marque-scheme` abstraction.
//! The adapter wraps `CanonicalAttrs` as `CapcoMarking`, implements
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

use marque_ism::{CanonicalAttrs, Classification, CountryCode, PageContext};
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

// T035c-21: NODIS / EXDIS sentinels for E037 (Conflicts) + E038
// (Requires NOFORN). Resolved via `satisfies_attrs` against
// `attrs.non_ic_dissem`, where the `NonIcDissem::Nodis` and
// `NonIcDissem::Exdis` variants live.
pub const TOK_NODIS: TokenId = TokenId(122);
pub const TOK_EXDIS: TokenId = TokenId(123);

// ---------------------------------------------------------------------------
// CapcoMarking — newtype over CanonicalAttrs implementing Lattice
// ---------------------------------------------------------------------------

/// CAPCO marking as viewed through the `marque-scheme` lens. A thin
/// newtype around [`CanonicalAttrs`] so we can hang trait impls on it
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
///
/// # Decoder provenance side channel (Phase 4 PR-4b)
///
/// Tuple-position 1 is an optional [`DecoderProvenance`] populated by
/// the Phase D probabilistic recognizer. Strict-path recognizers leave
/// it `None`. The engine reads `provenance.is_some()` to detect "this
/// recognition went through the decoder fallback" and emits a
/// synthetic `R001 decoder-recognition` diagnostic with
/// [`FixSource::DecoderPosterior`](marque_rules::FixSource::DecoderPosterior).
/// See [`crate::provenance`] for the side-channel contract.
///
/// `PartialEq` / `Eq` ignore tuple-position 1 — provenance is metadata,
/// not identity. Two markings with identical attrs but different
/// provenance traces compare equal.
#[derive(Debug, Clone)]
pub struct CapcoMarking(
    pub CanonicalAttrs,
    pub Option<crate::provenance::DecoderProvenance>,
);

impl PartialEq for CapcoMarking {
    /// Identity is the parsed attributes only — decoder provenance is
    /// audit metadata that does not participate in marking equality
    /// (see the type-level doc comment).
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for CapcoMarking {}

impl From<CanonicalAttrs> for CapcoMarking {
    #[inline]
    fn from(attrs: CanonicalAttrs) -> Self {
        Self(attrs, None)
    }
}

impl CapcoMarking {
    /// Construct a strict-path `CapcoMarking` (no decoder provenance).
    ///
    /// Convenience constructor that mirrors the pre-PR-4b tuple-struct
    /// literal `CapcoMarking(attrs)`. Use this in tests and
    /// strict-path recognizers; the decoder constructs the marking by
    /// setting tuple-position 1 directly when it has provenance to
    /// attach.
    #[inline]
    pub fn new(attrs: CanonicalAttrs) -> Self {
        Self(attrs, None)
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
        CapcoMarking::new(page_context_to_attrs(&ctx))
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

        let mut out = CanonicalAttrs::default();
        out.classification = classification;
        out.sci_controls = sci.into_boxed_slice();
        out.dissem_controls = dissem.into_boxed_slice();
        CapcoMarking::new(out)
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

/// Always-false [`CategoryPredicate::Custom`] body used by every
/// Phase-3 stub `PageRewrite` row.
///
/// The rewrite's `reads` / `writes` axes are what the Kahn scheduler
/// consumes (T031–T032). Its trigger body does not participate in
/// Phase 3 runtime dispatch because `Engine::lint` does not route
/// aggregation through `scheme.project(Scope::Page, …)` — the
/// hand-coded [`PageContext`] aggregator handles roll-up. Pinning the
/// trigger to `false` makes that no-op explicit: any test or tool
/// that calls `scheme.project()` on today's `CapcoScheme` will see
/// these rewrites declare but never fire.
fn never_fires(_: &CapcoMarking) -> bool {
    false
}

/// No-op [`CategoryAction::Custom`] body for Phase-3 stub
/// `PageRewrite` rows whose action would otherwise need a multi-axis
/// or within-axis transform that the Phase-3 declarative surface
/// can't express cleanly (e.g., the §3.4.1 transmutations).
///
/// Runtime page-rewrite dispatch stays in [`PageContext`] until
/// Phase D / Phase E lands real rewrite bodies; until then the
/// action body is a no-op and only the row's `reads` / `writes`
/// axis annotations are consumed (by the engine's topological
/// scheduler, T031–T032). Pairs with [`never_fires`] for triggers.
fn noop_action(_marking: &mut CapcoMarking) {}

/// Build a `CanonicalAttrs` banner projection from the `expected_*`
/// accessors on `PageContext`. Intentionally narrow: only fills the
/// fields exercised by Phase A's equivalence tests. Other fields land
/// at their defaults, which matches Phase B's goal of handing
/// everything off to scheme-driven aggregation.
#[inline]
fn page_context_to_attrs(ctx: &PageContext) -> CanonicalAttrs {
    let mut out = CanonicalAttrs::default();

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
    out.declassify_on = ctx.expected_declassify_on().cloned();
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

    /// Construct CAPCO's `PageRewrite` table.
    ///
    /// Nine rewrites, in two groups:
    ///
    /// - **Active (1):** `capco/noforn-clears-rel-to` — the only row
    ///   wired to a real `Contains` predicate + `Clear` action; cited
    ///   at §D.2 Table 3 + §H.8 p145.
    /// - **Phase-3 stubs (8):** the §3.4.1 / §3.4.3 transmutation
    ///   roster from `marque-applied.md` (consultant Entry 6 split
    ///   into 6a + 6b for D13 single-citation discipline). Each
    ///   declares a `Custom(never_fires)` trigger and a
    ///   `Custom(noop_action)` body — Phase 3 does not drive page
    ///   roll-up through `scheme.project()`, so the trigger pins to
    ///   `false` and the action body is empty. The `reads` / `writes`
    ///   annotations are what the Kahn scheduler consumes (T031–T032)
    ///   to validate dataflow ordering; the runtime semantics still
    ///   live in the hand-coded [`PageContext`] aggregator. Phase D /
    ///   Phase E replaces the `Custom` bodies with real predicates
    ///   and transforms.
    ///
    /// # `reads` semantics — narrow form
    ///
    /// `reads` declares **true dataflow dependencies only**: axes
    /// whose post-rewrite state this rewrite consumes from another
    /// rewrite. Axes the trigger only pattern-matches against
    /// (predicate-scan reads) are documented in the per-entry
    /// doc-comment but excluded from the `reads` slice. Inflating
    /// `reads` with predicate-scan axes manufactures false cycles in
    /// the scheduler's dependency graph: the engine scheduler at
    /// `crates/engine/src/scheduler.rs:78-95` only skips
    /// *same-rewrite* self-edges (`producer_idx == idx`), so two
    /// independent rewrites that each read AND write the same axis
    /// produce a mutual edge in both directions and abort
    /// `Engine::new` with `RewriteCycle`. Predicate-scan axes go in
    /// the doc-comment with the explicit phrase "predicate scans X
    /// for Y"; if Phase D/E discovers a real dataflow dependency on
    /// a documented predicate-scan axis, the corresponding `reads`
    /// annotation can be re-introduced and the scheduler's DAG will
    /// reflect it.
    ///
    /// The eight Phase-3 stubs (in topological order):
    ///
    /// 1. `capco/frd-sigma-consolidates-into-rd-sigma` (§H.6 p113) —
    ///    AEA-only, independent.
    /// 2. `capco/fgi-rollup-on-us-contact` (§H.7 p123) — bare-FGI
    ///    rollup on US-class contact.
    /// 3. `capco/fgi-restricted-rollup-on-us-contact` (§H.7 p123) —
    ///    bare-FGI-R contact rolls FGI list (class lift is
    ///    parser-side per §3.4.1 Note (i)).
    /// 4. `capco/joint-cross-class-rollup` (§H.3 p57) — JOINT [list]
    ///    on non-US-class contact rolls FGI [non-US JOINT members].
    /// 5. `capco/us-presence-promotes-bare-fgi-attribution`
    ///    (§H.7 p123) — idempotent FGI cleanup; runs after entries
    ///    1–3 (consumes their FGI_MARKER output, the one structural
    ///    FGI_MARKER read in the table).
    /// 6. `capco/orcon-nato-to-us-orcon-on-us-contact` (§H.8 p136) —
    ///    ORCON-NATO transmutes to US ORCON on US-class contact.
    /// 7. `capco/sbu-nf-transmutes-on-classified-contact`
    ///    (§H.9 p178) — SBU-NF transmutes on classified contact.
    /// 8. `capco/les-nf-transmutes-on-classified-contact`
    ///    (§H.9 p185) — LES-NF transmutes on classified contact.
    ///
    /// Source: `marque-applied.md` §3.4.1 + §3.4.3. Declaration order
    /// here matches the topological order from
    /// `docs/plans/2026-05-07-pr3b-B-transmutations-plan.md` §4 to
    /// keep the file readable; the scheduler topologically sorts at
    /// `Engine::new` regardless of declaration order.
    ///
    /// [`CategoryPredicate::Contains`]: marque_scheme::CategoryPredicate::Contains
    /// [`CategoryAction::Clear`]: marque_scheme::CategoryAction::Clear
    /// [`Engine::lint`]: marque_engine::Engine::lint
    fn build_page_rewrites() -> Vec<PageRewrite<CapcoScheme>> {
        // `capco/noforn-clears-rel-to` reads `CAT_DISSEM` to look for
        // NOFORN and writes `CAT_REL_TO` to clear it. The CAT_DISSEM
        // read is a real dataflow dependency on entries 5/6a/6b,
        // which write CAT_DISSEM (ORCON-NATO → ORCON, SBU-NF/LES-NF
        // transmutations) — the scheduler must order this rewrite
        // AFTER those entries so the clearer sees the post-
        // transmutation NOFORN state. The CAT_REL_TO read is a
        // self-edge (skipped by the scheduler at
        // `crates/engine/src/scheduler.rs:84-87`), retained as
        // defensive ordering for future REL-TO writers.
        //
        // (REL TO appearing as its own category — rather than as a
        // dissem-control subtype — is an artifact of `CanonicalAttrs`
        // modeling country-list resolution separately; the rewrite
        // semantics treat it as a first-class category that
        // producers can write.)
        const NF_READS: &[marque_scheme::CategoryId] = &[CAT_DISSEM, CAT_REL_TO];
        const NF_WRITES: &[marque_scheme::CategoryId] = &[CAT_REL_TO];

        // Entry 4 (consultant §3.4.1 #4): FRD-SIGMA consolidates into
        // RD-SIGMA. Within-axis transform on CAT_AEA — reads and
        // writes the same axis (self-edge skipped per
        // `crates/engine/src/scheduler.rs:84-87`). Topologically
        // independent of every other entry.
        const E4_READS: &[marque_scheme::CategoryId] = &[CAT_AEA];
        const E4_WRITES: &[marque_scheme::CategoryId] = &[CAT_AEA];

        // Entry 1 (consultant §3.4.1 #1): bare-FGI rollup on US
        // contact. Narrow-form reads: CLASS only. Predicate-scan of
        // CAT_FGI_MARKER (for bare-FGI atoms) is documented in the
        // per-entry doc-comment, not in `reads`; declaring it would
        // cycle against entries 2 and 3 (each writes FGI_MARKER and
        // would read it through their own predicate-scan). Reciprocal
        // class raise is parser-side per §3.4.1 Note (i), so CLASS is
        // not in `writes`.
        const E1_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
        const E1_WRITES: &[marque_scheme::CategoryId] = &[CAT_FGI_MARKER];

        // Entry 2 (consultant §3.4.1 #2): bare-FGI-R rollup on US
        // contact. Narrow-form reads: CLASS only (see Entry 1 note
        // on predicate-scan vs dataflow reads). Class lift to ≥ C is
        // parser-side per §3.4.1 Note (i).
        const E2_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
        const E2_WRITES: &[marque_scheme::CategoryId] = &[CAT_FGI_MARKER];

        // Entry 3 (consultant §3.4.1 #3): JOINT cross-class rollup.
        // Reads CLASS plus JOINT_CLASSIFICATION (the trigger
        // axis — the JOINT scan IS the read, no predicate-scan
        // doc-comment needed). Writes FGI_MARKER only — §H.3 p57
        // is explicit that JOINT does NOT carry forward to the
        // banner line in US documents, so this rewrite consumes
        // JOINT state without writing it back; class lift is
        // parser-side per §3.4.1 Note (i).
        const E3_READS: &[marque_scheme::CategoryId] =
            &[CAT_CLASSIFICATION, CAT_JOINT_CLASSIFICATION];
        const E3_WRITES: &[marque_scheme::CategoryId] = &[CAT_FGI_MARKER];

        // Entry 7 (consultant §3.4.1 #7): US-presence promotes bare
        // FGI attribution. The CAT_FGI_MARKER read IS structural
        // here — entry 7 consumes the post-rewrite FGI state
        // produced by entries 1, 2, 3 and idempotently promotes any
        // remaining `bare(_, C, _)` to `⊤(C)`. This is the one
        // entry whose FGI_MARKER read is a real dataflow dep, not a
        // predicate-scan artifact, so it stays in `reads`.
        const E7_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION, CAT_FGI_MARKER];
        const E7_WRITES: &[marque_scheme::CategoryId] = &[CAT_FGI_MARKER];

        // Entry 5 (consultant §3.4.1 #5): ORCON-NATO transmutes to
        // US ORCON on US-class contact. Narrow-form reads: CLASS
        // only. Predicate-scan of CAT_DISSEM (for ORCON-NATO) is
        // doc-comment only; declaring it would cycle against
        // entries 6a/6b (each writes DISSEM and would read it
        // through their own predicate-scan).
        const E5_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
        const E5_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

        // Entry 6a (consultant §3.4.1 #6, split per D13): SBU-NF
        // transmutes on classified contact. Narrow-form reads:
        // CLASS only (see Entry 5 note on predicate-scan vs
        // dataflow reads — predicate also scans `non_ic_dissem`
        // field for SBU-NF). Per Phase-3 pragmatic mapping
        // (plan §8 Q1), the non-IC dissem axis is folded into
        // CAT_DISSEM until Phase D/E exposes a separate
        // `CAT_NON_IC_DISSEM`.
        const E6A_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
        const E6A_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

        // Entry 6b (consultant §3.4.1 #6, split per D13): LES-NF
        // transmutes on classified contact. Same narrow-form +
        // axis-mapping pragmatism as Entry 6a. Cited at §H.9 p185
        // (LES-NF is its own §H.9 subsection p185–186, distinct
        // from SBU-NF p178).
        const E6B_READS: &[marque_scheme::CategoryId] = &[CAT_CLASSIFICATION];
        const E6B_WRITES: &[marque_scheme::CategoryId] = &[CAT_DISSEM];

        vec![
            // §D.2 Table 3 (FD&R Markings Precedence Rules for Banner
            // Line Roll-Up) Rule #2 specifies that NOFORN supersedes
            // REL TO at banner scope; the §H.8 NOFORN entry (p145)
            // back-references this table via "Refer to Section D.2.,
            // Table 3 FD&R Markings Precedence Rules for Banner Line
            // Roll-Up for guidance" in its Precedence Rules section.
            PageRewrite::declarative(
                "capco/noforn-clears-rel-to",
                "CAPCO-2016 §D.2 Table 3 + §H.8 p145",
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
            // Entry 4 — `capco/frd-sigma-consolidates-into-rd-sigma`.
            // §H.6 p113 (FRD-SIGMA Precedence Rules for Banner Line
            // Guidance): "If both RD and FRD SIGMA [#] portions are
            // in a document, the RD-SIGMA [#] marking takes
            // precedence over the FRD-SIGMA [#] marking in the
            // banner line and all SIGMA numbers are listed in the
            // banner line RD-SIGMA [#] marking, regardless of whether
            // the information was RD or FRD." Within-axis transform
            // — drops FRD-SIGMA atoms from CAT_AEA and folds their
            // numbers into the surviving RD-SIGMA atom.
            //
            // Monotonicity: shrinking on CAT_AEA (FRD-SIGMA atoms
            // dropped). Sound under fixed topological order.
            //
            // Phase-3 stub: trigger is `never_fires` and action is
            // `noop_action` because runtime dispatch stays in
            // `PageContext` until Phase D/E. Only the
            // `reads` / `writes` annotations are consumed (by the
            // scheduler). Topologically independent of every other
            // entry: the AEA axis is otherwise un-written.
            PageRewrite::custom(
                "capco/frd-sigma-consolidates-into-rd-sigma",
                "CAPCO-2016 §H.6 p113",
                CategoryPredicate::Custom(never_fires),
                CategoryAction::Custom(noop_action),
                E4_READS,
                E4_WRITES,
            ),
            // Entry 1 — `capco/fgi-rollup-on-us-contact`.
            // §H.7 p123 (Precedence Rules for Banner Line Guidance):
            // "If any document contains portions of both source-
            // concealed FGI ... and source-acknowledged FGI ..., then
            // only the 'FGI' marking without the source
            // trigraph(s)/tetragraph(s) must appear in the banner
            // line." Trigger surface is bare-FGI portion contacting
            // US-class; effect is FGI banner rollup. Reciprocal
            // class raise is performed at portion-parse-time per
            // `marque-applied.md` §3.4.1 Note (i), NOT as a rewrite
            // transform — CLASS is not in `writes`.
            //
            // Monotonicity: monotone-additive on FGI axis (concealed
            // wins over acknowledged; acknowledged unions). CLASS
            // not mutated by this rewrite.
            //
            // Predicate scans `CAT_FGI_MARKER` for bare-FGI atoms.
            // The scan axis is documented here, not in `reads`:
            // entries 1, 2, 3 each trigger on disjoint portion-level
            // patterns and each writes `CAT_FGI_MARKER`; declaring
            // FGI_MARKER as a read here would manufacture a
            // false-cycle against entries 2 and 3. The scheduler's
            // coarse "writes determines order" model is sufficient
            // because the three rewrites' FGI outputs are
            // commutative shape-modifications. If Phase D/E
            // discovers a real dataflow dep on the FGI state, add
            // FGI_MARKER to `reads` then.
            //
            // Shared §-citation with Entry 7 is admissible under
            // D13: this entry is the rollup TRIGGER (bare-FGI
            // contacts US-class); Entry 7 is the idempotent
            // generalization that runs after 1–3 settle.
            //
            // Phase-3 stub: see Entry 4 doc-comment.
            PageRewrite::custom(
                "capco/fgi-rollup-on-us-contact",
                "CAPCO-2016 §H.7 p123",
                CategoryPredicate::Custom(never_fires),
                CategoryAction::Custom(noop_action),
                E1_READS,
                E1_WRITES,
            ),
            // Entry 2 — `capco/fgi-restricted-rollup-on-us-contact`.
            // §H.7 p123 (Relationship(s) to Other Markings): FGI
            // "may be used with TOP SECRET, SECRET, CONFIDENTIAL,
            // RESTRICTED, UNCLASSIFIED, and other designators ...
            // applied by the non-US originator". Combined with the
            // p123 rollup contract (quoted under Entry 1), bare-
            // FGI-R contacting US-class rolls FGI attribution to
            // `[list]`. Class lift to ≥ C (RESTRICTED is not an
            // authorized US classification, so the reciprocal raise
            // floors at C) is parser-side per
            // `marque-applied.md` §3.4.1 Note (i), NOT a rewrite
            // transform — CLASS is not in `writes`.
            //
            // Monotonicity: monotone-additive on FGI axis
            // (R-classified countries union into the trigraph list).
            // Class lift is parser-side and monotone (R → C is
            // upward only).
            //
            // Predicate scans `CAT_FGI_MARKER` for bare-FGI-R atoms.
            // Same predicate-scan-vs-dataflow convention as Entry 1
            // (see Entry 1 doc-comment); FGI_MARKER excluded from
            // `reads` to avoid manufactured cycles against entries
            // 1 and 3.
            //
            // Phase-3 stub: see Entry 4 doc-comment.
            PageRewrite::custom(
                "capco/fgi-restricted-rollup-on-us-contact",
                "CAPCO-2016 §H.7 p123",
                CategoryPredicate::Custom(never_fires),
                CategoryAction::Custom(noop_action),
                E2_READS,
                E2_WRITES,
            ),
            // Entry 3 — `capco/joint-cross-class-rollup`.
            // §H.3 p57 (Derivative Use, banner-line construction):
            // "Highest classification level of all portions,
            // expressed as a US classification marking. ... The
            // FGI marking including all trigraph/tetragraph codes
            // identified in the JOINT portion(s). REL TO, including
            // all common non-US country trigraph/tetragraph codes
            // identified in the JOINT portions, unless a portion is
            // marked NOFORN, in which case the NOFORN marking must
            // appear in the banner line." JOINT [list] contacting a
            // non-US-class portion rolls FGI attribution to list
            // the non-US JOINT members; banner class is the
            // highest-US-class of all portions, established
            // parser-side per §H.3 p57 + `marque-applied.md`
            // §3.4.1 Note (i) — JOINT does NOT carry forward to the
            // banner line in US documents, so this rewrite consumes
            // JOINT state without writing it back, and CLASS is not
            // in `writes`.
            //
            // Monotonicity: monotone-additive on FGI axis (non-US
            // JOINT members union in). Class lift is parser-side
            // and monotone.
            //
            // No predicate-scan note: the `JOINT_CLASSIFICATION`
            // read IS the trigger axis (§H.3 p57 names JOINT
            // explicitly), so it stays in `reads` as a real
            // dataflow read of the page-level JOINT state.
            //
            // Phase-3 stub: see Entry 4 doc-comment.
            PageRewrite::custom(
                "capco/joint-cross-class-rollup",
                "CAPCO-2016 §H.3 p57",
                CategoryPredicate::Custom(never_fires),
                CategoryAction::Custom(noop_action),
                E3_READS,
                E3_WRITES,
            ),
            // Entry 7 — `capco/us-presence-promotes-bare-fgi-attribution`.
            // §H.7 p123 (Precedence Rules for Banner Line Guidance,
            // quoted under Entry 1) establishes both the trigger and
            // the post-rollup-cleanup contracts. This entry is the
            // idempotent generalization: after entries 1–3 consolidate
            // FGI state, any remaining `bare(_, C, _)` FGI attribution
            // is promoted to a fully-rolled-up `⊤(C)` form.
            //
            // Monotonicity: monotone-additive. `bare(_, C, _) → ⊤(C)`
            // is a join-monotone `FgiSet` promotion; idempotent on
            // already-promoted state.
            //
            // No predicate-scan note: the `CAT_FGI_MARKER` read here
            // IS a real dataflow dependency on entries 1, 2, 3 —
            // entry 7 consumes their post-rewrite FGI state and
            // promotes any remaining `bare(_, C, _)` attribution.
            // This is the one entry in the table whose FGI_MARKER
            // read is structural, not a predicate-scan artifact, so
            // it stays in `reads` and the scheduler orders entry 7
            // after 1, 2, 3.
            //
            // Shared §-citation with Entry 1 is admissible under
            // D13: Entry 1 is the trigger (bare-FGI contacts
            // US-class); this entry is the idempotent cleanup that
            // runs after 1–3 settle.
            //
            // Phase-3 stub: see Entry 4 doc-comment.
            PageRewrite::custom(
                "capco/us-presence-promotes-bare-fgi-attribution",
                "CAPCO-2016 §H.7 p123",
                CategoryPredicate::Custom(never_fires),
                CategoryAction::Custom(noop_action),
                E7_READS,
                E7_WRITES,
            ),
            // Entry 5 — `capco/orcon-nato-to-us-orcon-on-us-contact`.
            // §H.8 p136 (ORCON Precedence Rules for Banner Line
            // Guidance): "If ORCON and ORCON-USGOV portions are in a
            // document, ORCON takes precedence and is conveyed in
            // the banner line." ORCON-NATO (CAPCO-2016 line 895,
            // Appendix B: "ORCON (NATO dissemination control
            // marking) ... See US ORCON ARH requirements") maps onto
            // the same precedence surface — ORCON-NATO contacting
            // US-class transmutes to US ORCON in the page dissem
            // axis. Per D13, the §H.8 p136 cite is the primary
            // anchor; the Appendix B mapping (line 895) is the
            // supplementary reference for ORCON-NATO ↔ US ORCON
            // equivalence.
            //
            // Monotonicity: mixed — drops ORCON-NATO (shrinking) and
            // adds ORCON (additive). Sound under fixed topological
            // order.
            //
            // Predicate scans `CAT_DISSEM` for ORCON-NATO. The scan
            // axis is documented here, not in `reads`: entries 5,
            // 6a, 6b each trigger on disjoint dissem-token patterns
            // and each writes `CAT_DISSEM`; declaring DISSEM as a
            // read here would manufacture a false-cycle against
            // 6a and 6b. The DISSEM-writers are commutative
            // shape-modifications on the page dissem set, so the
            // scheduler's "writes determines order" model is
            // sufficient.
            //
            // Phase-3 stub: see Entry 4 doc-comment.
            PageRewrite::custom(
                "capco/orcon-nato-to-us-orcon-on-us-contact",
                "CAPCO-2016 §H.8 p136",
                CategoryPredicate::Custom(never_fires),
                CategoryAction::Custom(noop_action),
                E5_READS,
                E5_WRITES,
            ),
            // Entry 6a — `capco/sbu-nf-transmutes-on-classified-contact`.
            // §H.9 p178 (SBU-NF Commingling Rule(s) Within a
            // Portion): "The SBU-NF marking is conveyed in the
            // portion mark only if the commingled portion is
            // unclassified and there is no other NOFORN information
            // included in the portion. If there is other NOFORN
            // information in the commingled portion, the 'SBU'
            // marking is used and a NOFORN marking is added, e.g.,
            // (U//NF//SBU)." Class > U drops SBU-NF entirely; class
            // = U replaces SBU-NF with NOFORN + SBU.
            //
            // Monotonicity: mixed — shrinking on class > U;
            // mostly-additive on class = U. Sound under fixed
            // topological order.
            //
            // Predicate scans `CAT_DISSEM` (and the
            // `CanonicalAttrs.non_ic_dissem` field) for SBU-NF.
            // Same predicate-scan-vs-dataflow convention as
            // Entry 5 (see Entry 5 doc-comment); DISSEM excluded
            // from `reads` to avoid manufactured cycles against
            // entries 5 and 6b.
            //
            // Phase-3 axis-mapping pragmatic (plan §8 Q1): SBU/SBU-NF
            // live in `CanonicalAttrs.non_ic_dissem` but no
            // `CAT_NON_IC_DISSEM` CategoryId is exposed yet, so the
            // write axis is `CAT_DISSEM`. Phase D/E may add the
            // separate axis.
            //
            // Phase-3 stub: see Entry 4 doc-comment.
            PageRewrite::custom(
                "capco/sbu-nf-transmutes-on-classified-contact",
                "CAPCO-2016 §H.9 p178",
                CategoryPredicate::Custom(never_fires),
                CategoryAction::Custom(noop_action),
                E6A_READS,
                E6A_WRITES,
            ),
            // Entry 6b — `capco/les-nf-transmutes-on-classified-contact`.
            // §H.9 p185 (LES-NF Precedence Rules for Banner Line
            // Guidance): "When a
            // classified document contains portions of U//LES-NF,
            // the 'LES' marking is used in the banner line and the
            // NOFORN marking is applied as a Dissemination Control
            // Marking. For example: SECRET//NOFORN//LES." LES-NF
            // transmutes to NOFORN + LES; banner consolidates as
            // `[class]//NOFORN//LES`.
            //
            // Monotonicity: monotone-additive on the dissem axis
            // (NOFORN and LES both added; LES-NF dropped is the
            // input-side projection of the transmutation, not a
            // separate axis shrink). Sound under fixed topological
            // order.
            //
            // Predicate scans `CAT_DISSEM` (and the
            // `CanonicalAttrs.non_ic_dissem` field) for LES-NF.
            // Same predicate-scan-vs-dataflow convention as
            // Entry 5 / 6a; DISSEM excluded from `reads` to avoid
            // manufactured cycles against entries 5 and 6a.
            //
            // Phase-3 axis-mapping pragmatic (plan §8 Q1): same
            // CAT_DISSEM fold as Entry 6a.
            //
            // Phase-3 stub: see Entry 4 doc-comment.
            PageRewrite::custom(
                "capco/les-nf-transmutes-on-classified-contact",
                "CAPCO-2016 §H.9 p185",
                CategoryPredicate::Custom(never_fires),
                CategoryAction::Custom(noop_action),
                E6B_READS,
                E6B_WRITES,
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
            // CanonicalAttrs models it as a separate field because it's a list of countries that must be compared as a set for supersession and conflict rules.
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
        // The CAPCO declarative constraint catalog. Every entry's
        // `label` cites a verified passage in
        // `crates/capco/docs/CAPCO-2016.md`; non-normative sections
        // (§I-K — history, examples, acronym list) are NOT valid
        // citation targets. See Constitution VIII and the project
        // memory entry "CAPCO doc structure".
        //
        // T035 (2026-04-21) wired runtime evaluation through this
        // catalog: dyadic variants dispatch via the generic evaluator
        // (`crate::constraint::evaluate`) using
        // [`Self::satisfies`]; `Custom` variants dispatch through
        // [`Self::evaluate_custom`] to scheme-private predicate
        // helpers below. The hand-written `Rule` impls in
        // `crate::rules` that previously enforced these invariants
        // are retired in the same PR; `crate::rules_declarative`
        // hosts thin wrappers that call `scheme.validate()` and
        // construct `Diagnostic` values with byte-identical
        // message/span/fix output.
        //
        // T035b audit (2026-04-21): E017, E018, and E019 were
        // retired as over-restrictive relative to CAPCO-2016 §H.3
        // pp 56–57:
        //
        // - §H.3 p57 lists "FGI, IC and Non-IC dissemination
        //   control markings (excluding NOFORN)" among markings
        //   JOINT "may be used with, as appropriate"
        // - §H.3 p57 names only two explicit exclusions:
        //   HCS markings and NOFORN markings
        // - §H.3 p57 cross-references §H.7 for FGI content marker
        //   syntax on JOINT documents — FGI marker presence is a
        //   content indicator, not a competing classification type
        //
        // The JOINT+NOFORN exclusion is caught indirectly: E014
        // requires JOINT to carry REL TO, and
        // `capco/noforn-conflicts-rel-to` fires when NOFORN and REL
        // TO co-occur. The JOINT+HCS exclusion has no such indirect
        // coverage, so it gets its own catalog entry below as E036.
        vec![
            // ---- E010: HCS subsystem rules (CAPCO-2016 §H.4) -----
            //
            // Bare HCS is legacy; HCS-O requires ORCON; HCS-P
            // requires ORCON or ORCON-USGOV; HCS-O/P require S or
            // TS. The full sub-rule set lives in
            // `hcs_system_constraints` because the predicate is
            // n-ary and emits multiple violations per offending
            // marking (one per failing sub-rule).
            Constraint::Custom {
                name: "E010/HCS-system-constraints",
                label: "CAPCO-2016 §H.4 p61-62",
            },
            // ---- E012: dual classification (CAPCO-2016 §H.3 p55) -
            //
            // §H.3 p55: "The US, non-US, and JOINT
            // classification markings are mutually exclusive – a
            // banner line or portion mark may contain only one type
            // and value for the classification marking."
            //
            // Custom (not Conflicts) because the predicate inspects
            // a single field — `MarkingClassification::Conflict {
            // us, foreign }` — that the parser populates when it
            // encounters two systems in one marking.
            Constraint::Custom {
                name: "E012/dual-classification",
                label: "CAPCO-2016 §H.3 p55",
            },
            // ---- E014: JOINT requires REL TO coverage (§H.3 p57) -
            //
            // §H.3 p57 (Relationship(s) to Other Markings): "Requires
            // REL TO USA, LIST". Every JOINT participant MUST also
            // appear in the marking's REL TO list. Custom (not
            // Requires) because the check is iterative across all
            // JOINT countries.
            Constraint::Custom {
                name: "E014/joint-requires-rel-to-coverage",
                label: "CAPCO-2016 §H.3 p57",
            },
            // ---- E015: non-US requires dissem (§H.7 + §B.3) ------
            //
            // FGI markings require explicit foreign release per
            // §H.7 pp 122–123 (FGI marking template + sharing-
            // agreement basis) and §B.3 p20 paragraph d (FD&R
            // markings on FGI in IC DAPs); JOINT requires REL TO
            // per §H.3 p57. The simplified dyadic predicate
            // "non-US classification + empty dissem" captures the
            // common-case violation. The narrower per-system
            // requirements (FGI-specific, JOINT-specific) are
            // separately enforced by E014 and by the existing
            // hand-written rules.
            Constraint::Requires {
                name: "E015/non-us-requires-dissem",
                left: TokenRef::AnyInCategory(CAT_NON_US_CLASSIFICATION),
                right: TokenRef::AnyInCategory(CAT_DISSEM),
                label: "CAPCO-2016 §H.7 p122 + §B.3 p20",
            },
            // ---- E016: JOINT conflicts RESTRICTED (§H.3 p56) -----
            //
            // §H.3 p56 (Relationship(s) to Other Markings): "May not
            // be used with RESTRICTED. (Note: the US is always a
            // JOINT marking owner/producer; and RESTRICTED is not an
            // authorized US classification marking.)"
            Constraint::Conflicts {
                name: "E016/joint-conflicts-restricted",
                left: TokenRef::Token(TOK_JOINT),
                right: TokenRef::Token(TOK_RESTRICTED),
                label: "CAPCO-2016 §H.3 p56",
            },
            // ---- E036: JOINT conflicts HCS markings (§H.3 p57) ---
            //
            // §H.3 p57 (Relationship(s) to Other Markings): "May not
            // be used with the HCS markings or NOFORN markings."
            // Same page reinforces: JOINT may use "SCI (excluding HCS
            // markings), SAP, AEA, FGI, IC and Non-IC dissemination
            // control markings (excluding NOFORN)".
            //
            // The JOINT-NOFORN exclusion is already caught indirectly
            // by `capco/noforn-conflicts-rel-to` + E014's REL TO
            // requirement (NOFORN in a JOINT document either conflicts
            // with the required REL TO or leaves REL TO empty). The
            // HCS exclusion has no such indirect coverage, so it
            // gets its own catalog entry.
            //
            // Supersedes the retired E017/E018/E019 which over-
            // restricted JOINT against FGI content markers, arbitrary
            // IC dissem, and non-IC dissem respectively. Those rules
            // forbade combinations §H.3 p57 explicitly permits.
            // See T035b retirement commit and project memory
            // `feedback_audit_predicates_against_source.md`.
            Constraint::Conflicts {
                name: "E036/joint-conflicts-hcs",
                left: TokenRef::Token(TOK_JOINT),
                right: TokenRef::Token(TOK_HCS),
                label: "CAPCO-2016 §H.3 p57",
            },
            // ---- E021: AEA requires NOFORN (§H.6 p104) -----------
            //
            // §H.6 RD entry p104: "Is always used with NOFORN
            // unless a sharing agreement has been established per
            // the Atomic Energy Act. (Ref. Sections 123 and 144 of
            // the Atomic Energy Act, and DoD Instruction 5030.14.)".
            // The "always used with NOFORN" requirement applies to
            // RD, FRD (§H.6 p111), and TFNI (§H.6 p120) — not UCNI
            // (DOD UCNI §H.6 p116, DOE UCNI §H.6 p118 carry no such
            // requirement) and not to any future AEA entry added to
            // the category.
            // Custom (not `Requires { left: AnyInCategory(CAT_AEA) }`)
            // because that dyadic shape would sweep UCNI in: a valid
            // `U//UCNI` marking would incorrectly require NOFORN.
            Constraint::Custom {
                name: "E021/aea-requires-noforn",
                label: "CAPCO-2016 §H.6 p104",
            },
            // ---- E022: CNWDI classification floor (§H.6 p106) ----
            //
            // §H.6 CNWDI entry p106: "Applicable only to
            // Top Secret or Secret RD information" / "May only be
            // used with TOP SECRET RD or SECRET RD." Custom because
            // the floor predicate ("classification ≥ S") is a level
            // comparison, not a single-token check.
            Constraint::Custom {
                name: "E022/CNWDI-classification-floor",
                label: "CAPCO-2016 §H.6 p106",
            },
            // ---- E024: RD precedence (§H.6 p104) -----------------
            //
            // §H.6 RD entry p104: "If RD, FRD, and TFNI
            // portions are in a document, the RD takes precedence
            // and is conveyed in the banner line." Custom (not
            // Supersedes) because Supersedes is a banner-rollup
            // hint that doesn't fire diagnostics; the per-portion
            // commingling violation is what E024 reports. The
            // banner-rollup Supersedes entries are intentionally
            // deferred until Phase E wires them through
            // `project(Scope::Page, ...)`.
            Constraint::Custom {
                name: "E024/rd-precedence",
                label: "CAPCO-2016 §H.6 p104",
            },
            // ---- E025: UCNI conflicts classification (§H.6 p116) -
            //
            // §H.6 DOD UCNI entry p116: "Applicable only
            // to unclassified information" / "May only be used with
            // UNCLASSIFIED." Custom (not Conflicts) because the
            // predicate distinguishes UNCLASSIFIED (allowed) from
            // C/S/TS (forbidden) — a level comparison rather than
            // mere presence/absence.
            Constraint::Custom {
                name: "E025/ucni-conflicts-classification",
                label: "CAPCO-2016 §H.6 p116",
            },
            // ---- W002: US + FGI commingling (§H.7 p124) ----------
            //
            // §H.7 p124: documents not marked per ICD 206
            // "must segregate the FGI from US portions." Custom (not
            // Conflicts) because the rule is portion-only — the
            // wrapper filters by `RuleContext::marking_type` after
            // the predicate fires.
            Constraint::Custom {
                name: "W002/us-commingled-with-fgi",
                label: "CAPCO-2016 §H.7 p124",
            },
            // ---- capco/noforn-conflicts-rel-to (§H.8 p145) -------
            //
            // §H.8 NOFORN entry p145: "Cannot be used with
            // REL TO, RELIDO, EYES ONLY, or DISPLAY ONLY." This is
            // the portion-level exclusion; the page-rewrite that
            // clears REL TO when NOFORN is present at page scope is
            // declared separately in `build_page_rewrites`.
            Constraint::Conflicts {
                name: "capco/noforn-conflicts-rel-to",
                left: TokenRef::Token(TOK_NOFORN),
                right: TokenRef::AnyInCategory(CAT_REL_TO),
                label: "CAPCO-2016 §H.8 p145",
            },
            // ---- capco/joint-requires-usa (§H.3 p55) -------------
            //
            // §H.3 p55: "USA is always included in the
            // JOINT marking [LIST], as USA is always a
            // co-owner/producer." Plus REL TO must include USA per
            // §H.3 p57 (REL TO USA, LIST requirement). Custom (not Requires) because USA
            // must appear in BOTH `joint.countries` AND `rel_to` —
            // a coupled predicate that doesn't decompose cleanly
            // into a single TokenRef pair.
            Constraint::Custom {
                name: "capco/joint-requires-usa",
                label: "CAPCO-2016 §H.3 p55",
            },
            // ---- E037: NODIS ⊥ EXDIS (§H.9 p172 + p174) ----------
            //
            // §H.9 EXDIS entry (p172) and NODIS entry (p174) both
            // state the same mutual-exclusion invariant: NODIS and
            // EXDIS MUST NOT coexist on the same information ("EXDIS
            // and NODIS markings cannot be used together" / "NODIS
            // and EXDIS markings cannot be used together"). A portion
            // (or banner) carrying both is malformed.
            //
            // Modeled as a dyadic `Conflicts` constraint — the
            // symmetric shape fits built-in Conflicts exactly, no
            // cross-category coupling, no level comparison.
            Constraint::Conflicts {
                name: "E037/nodis-conflicts-exdis",
                left: TokenRef::Token(TOK_NODIS),
                right: TokenRef::Token(TOK_EXDIS),
                label: "CAPCO-2016 §H.9 p172 + p174",
            },
            // ---- E038: NODIS / EXDIS require NOFORN (§H.9) -------
            //
            // §H.9 EXDIS entry (p172) and NODIS entry (p174) both
            // state "Requires NOFORN" in their Relationship(s) to
            // Other Markings. A marking carrying NODIS or EXDIS
            // without NOFORN is a violation of both template entries.
            //
            // Custom (not two separate `Requires` constraints)
            // because the rule emits a SINGLE diagnostic ID — E038 —
            // and the dispatch layer in `rules_declarative.rs`
            // works by filtering violations by constraint `name`.
            // Splitting into two `Requires` constraints would create
            // two distinct violation names for one rule ID and force
            // the wrapper to OR them. Folding the disjunction into a
            // single Custom predicate keeps the wrapper trivial.
            Constraint::Custom {
                name: "E038/nodis-or-exdis-requires-noforn",
                label: "CAPCO-2016 §H.9 p172 + p174",
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

// ---------------------------------------------------------------------------
// Predicate implementations (free functions — trait impls delegate here)
// ---------------------------------------------------------------------------
//
// `satisfies_attrs` and `evaluate_custom_by_attrs` are the source of
// truth for CAPCO's constraint semantics. They take `&CanonicalAttrs`
// directly to avoid forcing callers on the fast path to wrap in
// `CapcoMarking` (which would require cloning the attributes). The
// trait impls on `CapcoScheme` delegate to them, and the fast-path
// inherent method `CapcoScheme::evaluate_named_constraint` uses them
// directly to dispatch a single named constraint without walking
// the whole catalog.

/// Resolve a [`TokenRef`] against raw [`marque_ism::CanonicalAttrs`].
///
/// **Token-presence semantics** (T035):
/// - [`TokenRef::Token(id)`] returns true when the marking carries
///   the named token *anywhere* relevant — `TOK_USA` ⇒ "USA in
///   REL TO" (the dissemination context), `TOK_RD` ⇒ "RD anywhere in
///   `aea_markings`", etc.
/// - [`TokenRef::AnyInCategory(cat)`] returns true when the category
///   has at least one populated value. `CAT_DISSEM` intentionally
///   counts both `dissem_controls` AND `rel_to` as dissem-flavored
///   presence, matching the historical E015 predicate.
///
/// `MarkingClassification::Conflict` is deliberately excluded from
/// `TOK_NON_US_CLASSIFICATION` / `CAT_NON_US_CLASSIFICATION` — that
/// state is E012's concern, not E015's.
///
/// Sentinel `TokenId`s not used by the current catalog
/// (`TOK_IC_DISSEM`, `TOK_NON_IC_DISSEM`) fall through to `false`;
/// they are declared for future T035b consumption.
fn satisfies_attrs(attrs: &marque_ism::CanonicalAttrs, token_ref: &TokenRef) -> bool {
    use marque_ism::{
        AeaMarking, DissemControl, MarkingClassification, SciControl, SciControlBare,
        SciControlSystem,
    };
    match token_ref {
        TokenRef::Token(id) => match *id {
            TOK_NOFORN => attrs
                .dissem_controls
                .iter()
                .any(|d| matches!(d, DissemControl::Nf)),
            TOK_USA => attrs.rel_to.contains(&CountryCode::USA),
            TOK_JOINT => {
                matches!(&attrs.classification, Some(MarkingClassification::Joint(_)))
            }
            TOK_RESTRICTED => matches!(
                &attrs.classification,
                Some(c) if c.effective_level() == Classification::Restricted
            ),
            TOK_RD => attrs
                .aea_markings
                .iter()
                .any(|a| matches!(a, AeaMarking::Rd(_))),
            TOK_FRD => attrs
                .aea_markings
                .iter()
                .any(|a| matches!(a, AeaMarking::Frd(_))),
            TOK_TFNI => attrs
                .aea_markings
                .iter()
                .any(|a| matches!(a, AeaMarking::Tfni)),
            TOK_CNWDI => attrs
                .aea_markings
                .iter()
                .any(|a| matches!(a, AeaMarking::Rd(rd) if rd.cnwdi)),
            TOK_UCNI => attrs
                .aea_markings
                .iter()
                .any(|a| matches!(a, AeaMarking::DodUcni | AeaMarking::DoeUcni)),
            // "HCS markings" is plural in CAPCO §H.3 p57 — it covers
            // the bare `HCS` token AND the compound forms `HCS-O` /
            // `HCS-P` / `HCS-O-P`. CVE-projection variants `Hcs`,
            // `HcsO`, `HcsP` are all matched explicitly; the
            // structural path via `sci_markings` covers any compound
            // anchored on `SciControlBare::Hcs` regardless of the
            // specific compartments attached.
            TOK_HCS => {
                attrs
                    .sci_controls
                    .iter()
                    .any(|s| matches!(s, SciControl::Hcs | SciControl::HcsO | SciControl::HcsP))
                    || attrs.sci_markings.iter().any(|m| {
                        matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
                    })
            }
            TOK_FGI_MARKER => attrs.fgi_marker.is_some(),
            TOK_US_CLASSIFIED => attrs.us_classification().is_some(),
            // `Conflict` deliberately excluded — see fn doc.
            TOK_NON_US_CLASSIFICATION => matches!(
                &attrs.classification,
                Some(
                    MarkingClassification::Fgi(_)
                        | MarkingClassification::Nato(_)
                        | MarkingClassification::Joint(_)
                )
            ),
            // `TOK_IC_DISSEM` and `TOK_NON_IC_DISSEM` have no live
            // consumers — the legacy E018/E019 constraints that
            // would have used them were retired in T035b as
            // over-restrictive. Kept as declared sentinels so any
            // future narrowly-scoped IC/non-IC dissem invariant
            // can dispatch against them without re-adding a
            // `TokenId` constant.
            TOK_IC_DISSEM | TOK_NON_IC_DISSEM => false,
            // T035c-21 PR-A: NODIS / EXDIS live in `non_ic_dissem`.
            // Both are DoS non-IC dissem controls per §H.9 (NODIS p174;
            // EXDIS p172).
            TOK_NODIS => attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Nodis)),
            TOK_EXDIS => attrs
                .non_ic_dissem
                .iter()
                .any(|d| matches!(d, marque_ism::NonIcDissem::Exdis)),
            _ => false,
        },
        TokenRef::AnyInCategory(cat) => match *cat {
            CAT_CLASSIFICATION => attrs.classification.is_some(),
            // `Conflict` deliberately excluded — see fn doc.
            CAT_NON_US_CLASSIFICATION => matches!(
                &attrs.classification,
                Some(
                    MarkingClassification::Fgi(_)
                        | MarkingClassification::Nato(_)
                        | MarkingClassification::Joint(_)
                )
            ),
            CAT_JOINT_CLASSIFICATION => {
                matches!(&attrs.classification, Some(MarkingClassification::Joint(_)))
            }
            CAT_SCI => !attrs.sci_controls.is_empty() || !attrs.sci_markings.is_empty(),
            CAT_SAR => attrs.sar_markings.is_some(),
            CAT_AEA => !attrs.aea_markings.is_empty(),
            CAT_FGI_MARKER => attrs.fgi_marker.is_some(),
            CAT_DISSEM => !attrs.dissem_controls.is_empty() || !attrs.rel_to.is_empty(),
            CAT_REL_TO => !attrs.rel_to.is_empty(),
            CAT_DECLASSIFY_ON => attrs.declassify_on.is_some(),
            _ => false,
        },
    }
}

/// Route a `Constraint::Custom` by name to its scheme-private
/// predicate helper. Returns an empty `Vec` for unknown names
/// (forward-compat with future catalog entries).
fn evaluate_custom_by_attrs(
    attrs: &marque_ism::CanonicalAttrs,
    name: &'static str,
) -> Vec<ConstraintViolation> {
    match name {
        "E010/HCS-system-constraints" => hcs_system_constraints(attrs, "CAPCO-2016 §H.4 p61-62"),
        "E012/dual-classification" => e012_dual_classification(attrs),
        "E014/joint-requires-rel-to-coverage" => e014_joint_rel_to_coverage(attrs),
        "E021/aea-requires-noforn" => e021_aea_requires_noforn(attrs),
        "E022/CNWDI-classification-floor" => e022_cnwdi_floor(attrs),
        "E024/rd-precedence" => e024_rd_precedence(attrs),
        "E025/ucni-conflicts-classification" => e025_ucni_classification(attrs),
        "W002/us-commingled-with-fgi" => w002_us_commingled_with_fgi(attrs),
        "capco/joint-requires-usa" => joint_requires_usa(attrs),
        "E038/nodis-or-exdis-requires-noforn" => e038_dos_dissem_requires_noforn(attrs),
        _ => Vec::new(),
    }
}

impl CapcoScheme {
    /// Evaluate a single constraint by `name` against raw
    /// `CanonicalAttrs`. Fast path for rule wrappers that want "did
    /// this specific predicate fire?" without the overhead of a
    /// full `MarkingScheme::validate()` call.
    ///
    /// Compared to `scheme.validate(&CapcoMarking::new(attrs.clone()))`:
    /// - **No `CanonicalAttrs` clone** — works on the borrow directly
    /// - **No full catalog walk** — linear `find` by `name` over the
    ///   ~13 catalog entries, then single dispatch. O(1) effectively;
    ///   the filter step that the wrappers previously did after
    ///   `validate()` is eliminated.
    /// - **No `CapcoMarking` wrap** — delegates straight to the
    ///   free-function predicates (`satisfies_attrs`,
    ///   `evaluate_custom_by_attrs`), which is also what the trait
    ///   impls use.
    ///
    /// Contract: the emitted `ConstraintViolation.constraint_label`
    /// and `.citation` are populated from the catalog entry's
    /// declared `name` and `label`, matching the normalization that
    /// `marque_scheme::constraint::evaluate` performs in its
    /// `Custom` arm. Dyadic-variant violations carry a generic
    /// "conflicting tokens" / "token X requires Y" message — same
    /// as the generic evaluator — because the wrapper layer is
    /// responsible for constructing the user-visible diagnostic
    /// text, not the scheme.
    pub(crate) fn evaluate_named_constraint(
        &self,
        attrs: &marque_ism::CanonicalAttrs,
        name: &'static str,
    ) -> Vec<ConstraintViolation> {
        let Some(c) = self.constraints.iter().find(|c| c.name() == name) else {
            return Vec::new();
        };
        let label = c.label();
        match c {
            Constraint::Conflicts { left, right, .. } => {
                if satisfies_attrs(attrs, left) && satisfies_attrs(attrs, right) {
                    vec![ConstraintViolation {
                        constraint_label: name,
                        message: format!("conflicting tokens: {left:?} and {right:?}"),
                        citation: label,
                    }]
                } else {
                    Vec::new()
                }
            }
            Constraint::Requires { left, right, .. } => {
                if satisfies_attrs(attrs, left) && !satisfies_attrs(attrs, right) {
                    vec![ConstraintViolation {
                        constraint_label: name,
                        message: format!("token {left:?} requires {right:?} but it is missing"),
                        citation: label,
                    }]
                } else {
                    Vec::new()
                }
            }
            // `Implies` is informational; `Supersedes` is a lattice
            // hint. Neither emits diagnostics — matches the behavior
            // of `marque_scheme::constraint::evaluate`.
            Constraint::Implies { .. } | Constraint::Supersedes { .. } => Vec::new(),
            Constraint::Custom { .. } => evaluate_custom_by_attrs(attrs, name)
                .into_iter()
                .map(|mut v| {
                    v.constraint_label = name;
                    v.citation = label;
                    v
                })
                .collect(),
        }
    }
}

// T035 (2026-04-21): `satisfies` and `evaluate_custom` are now
// implemented on `CapcoScheme`, so calling
// `marque_scheme::constraint::evaluate(&CapcoScheme::new(), &m)`
// (or equivalently `scheme.validate(&m)` via the trait default)
// fires every dyadic and Custom constraint in the catalog.
//
// The 11 hand-written rule impls retired by T035 dispatch through
// `crate::rules_declarative`, which uses the inherent fast-path
// method `CapcoScheme::evaluate_named_constraint` above (not the
// trait-path `validate`) and constructs `Diagnostic` values
// locally for byte-identical message/span/fix output. E018 / E019
// remain hand-written pending the T035b predicate audit.
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

    /// Resolve a [`TokenRef`] against a `CapcoMarking`'s concrete
    /// storage. Drives the dyadic-variant arms of
    /// [`marque_scheme::constraint::evaluate`].
    ///
    /// **Token-presence semantics** (T035):
    /// - [`TokenRef::Token(id)`] returns true when the marking carries
    ///   the named token *anywhere* relevant — `TOK_USA` ⇒ "USA in
    ///   REL TO" (the dissemination context), `TOK_RD` ⇒ "RD anywhere
    ///   in `aea_markings`", etc. The mapping is per-sentinel and
    ///   documented inline below.
    /// - [`TokenRef::AnyInCategory(cat)`] returns true when the
    ///   category has at least one populated value. `CAT_DISSEM`
    ///   intentionally counts both `dissem_controls` AND `rel_to` as
    ///   dissem-flavored presence, matching the historical E015
    ///   predicate ("non-US classification needs SOME dissem").
    ///
    /// Sentinel `TokenId`s not used by the current catalog
    /// (`TOK_IC_DISSEM`, `TOK_NON_IC_DISSEM`) fall through to `false`
    /// — they remain declared for future T035b consumption when the
    /// E018/E019 catalog entries are added back with corrected
    /// predicates. Categories not listed (none today) likewise fall
    /// through.
    /// Resolve a [`TokenRef`] against a `CapcoMarking`'s concrete
    /// storage. Drives the dyadic-variant arms of
    /// [`marque_scheme::constraint::evaluate`] when callers go through
    /// the trait path; the free-function `satisfies_attrs` below is
    /// the authoritative implementation.
    ///
    /// See `satisfies_attrs` for the full sentinel-to-predicate
    /// table.
    fn satisfies(&self, marking: &Self::Marking, token_ref: &TokenRef) -> bool {
        satisfies_attrs(&marking.0, token_ref)
    }

    /// Dispatch a [`Constraint::Custom`] entry to its scheme-private
    /// predicate body. Delegates to `evaluate_custom_by_attrs`, the
    /// name→helper router that the fast-path
    /// [`Self::evaluate_named_constraint`] uses.
    fn evaluate_custom(
        &self,
        name: &'static str,
        marking: &Self::Marking,
    ) -> Vec<ConstraintViolation> {
        evaluate_custom_by_attrs(&marking.0, name)
    }

    fn project(&self, scope: Scope, markings: &[Self::Marking]) -> Self::Marking {
        match scope {
            Scope::Portion => {
                // Identity under portion scope: if the caller passed a
                // single marking we return it; empty → bottom.
                markings
                    .first()
                    .cloned()
                    .unwrap_or_else(|| CapcoMarking::new(CanonicalAttrs::default()))
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
                let mut out = CapcoMarking::new(page_context_to_attrs(&ctx));
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
                            CategoryAction::Promote { .. } => {
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
// T035 Custom-constraint helpers
// ---------------------------------------------------------------------------
//
// Each helper is the predicate body for a `Constraint::Custom` entry in
// `build_constraints`. The helpers do NOT reference `RuleContext` — only
// `CanonicalAttrs`. Per-context filtering (e.g., W002 portion-only) lives in
// the wrapper layer (`crate::rules_declarative`); the catalog represents
// "this marking is structurally inconsistent" without regard to where the
// marking appears.
//
// The returned `ConstraintViolation` populates `message` with text that the
// wrapper inspects when constructing the user-facing `Diagnostic`. The
// `constraint_label` and `citation` fields are overwritten by the caller
// (`marque_scheme::constraint::evaluate`'s `Custom` arm) so any placeholder
// values are fine — using the catalog name + label keeps the helpers
// self-documenting in isolation.

/// E012 — `MarkingClassification::Conflict` indicates the parser saw a US
/// classification AND a foreign classification in the same marking. CAPCO
/// §H.3 p55 forbids this ("The US, non-US, and JOINT classification
/// markings are mutually exclusive").
fn e012_dual_classification(attrs: &marque_ism::CanonicalAttrs) -> Vec<ConstraintViolation> {
    if let Some(marque_ism::MarkingClassification::Conflict { us, foreign }) = &attrs.classification
    {
        let foreign_desc = match foreign.as_ref() {
            marque_ism::ForeignClassification::Nato(n) => format!("NATO ({})", n.banner_str()),
            marque_ism::ForeignClassification::Fgi(f) => {
                let countries: Vec<&str> = f.countries.iter().map(|c| c.as_str()).collect();
                if countries.is_empty() {
                    "FGI".to_owned()
                } else {
                    format!("FGI {}", countries.join(" "))
                }
            }
            marque_ism::ForeignClassification::Joint(j) => {
                let countries: Vec<&str> = j.countries.iter().map(|c| c.as_str()).collect();
                format!("JOINT {}", countries.join(" "))
            }
        };
        vec![ConstraintViolation {
            constraint_label: "E012/dual-classification",
            // The wrapper rebuilds the user-visible message from attrs;
            // the message here exists for catalog-level inspection and
            // tests. We surface `us` + `foreign_desc` so a test can
            // confirm both systems were observed.
            message: format!(
                "marking has both US ({}) and foreign ({}) classification",
                us.banner_str(),
                foreign_desc
            ),
            citation: "CAPCO-2016 §H.3 p55",
        }]
    } else {
        Vec::new()
    }
}

/// Returns `true` if `trigraph` is directly in `rel_to` or is a member of any
/// tetragraph in `rel_to` (e.g., GBR is covered when FVEY appears in REL TO).
pub(crate) fn rel_to_covers(rel_to: &[marque_ism::CountryCode], trigraph: &str) -> bool {
    rel_to.iter().any(|r| {
        r.as_str() == trigraph
            || crate::vocab::expand_tetragraph(r.as_str())
                .is_some_and(|members| members.contains(&trigraph))
    })
}

/// E014 — every JOINT participant must appear in the marking's REL TO list.
/// CAPCO §H.3 p57 ("Requires REL TO USA, LIST" relationship statement).
/// Tetragraphs in REL TO expand to their constituent trigraphs: a participant
/// covered by a tetragraph (e.g., GBR via FVEY) is considered present.
fn e014_joint_rel_to_coverage(attrs: &marque_ism::CanonicalAttrs) -> Vec<ConstraintViolation> {
    let joint = match &attrs.classification {
        Some(marque_ism::MarkingClassification::Joint(j)) => j,
        _ => return Vec::new(),
    };
    let missing: Vec<&str> = joint
        .countries
        .iter()
        .filter(|c| !rel_to_covers(&attrs.rel_to, c.as_str()))
        .map(|c| c.as_str())
        .collect();
    if missing.is_empty() {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "E014/joint-requires-rel-to-coverage",
        message: format!(
            "JOINT participants [{}] must appear in REL TO list",
            missing.join(", ")
        ),
        citation: "CAPCO-2016 §H.3 p57",
    }]
}

/// E021 — RD, FRD, or TFNI requires NOFORN (unless a sharing agreement under
/// Atomic Energy Act section 123 or 144 applies). CAPCO §H.6 p104.
/// Intentionally narrower than `AnyInCategory(CAT_AEA)` — UCNI variants
/// do not carry the NOFORN requirement (CAPCO §H.6 p116 / p118).
fn e021_aea_requires_noforn(attrs: &marque_ism::CanonicalAttrs) -> Vec<ConstraintViolation> {
    let has_rd_frd_tfni = attrs.aea_markings.iter().any(|a| {
        matches!(
            a,
            marque_ism::AeaMarking::Rd(_)
                | marque_ism::AeaMarking::Frd(_)
                | marque_ism::AeaMarking::Tfni
        )
    });
    if !has_rd_frd_tfni {
        return Vec::new();
    }
    let has_noforn = attrs
        .dissem_controls
        .iter()
        .any(|d| matches!(d, marque_ism::DissemControl::Nf));
    if has_noforn {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "E021/aea-requires-noforn",
        message: "RD/FRD/TFNI requires NOFORN unless a sharing agreement exists \
                  per the Atomic Energy Act"
            .to_owned(),
        citation: "CAPCO-2016 §H.6 p104",
    }]
}

/// E038 — NODIS / EXDIS require NOFORN. CAPCO-2016 §H.9 p172
/// (EXDIS: "Requires NOFORN") and p174 (NODIS: "Requires NOFORN").
/// Emits a single ConstraintViolation when the marking carries NODIS
/// or EXDIS without NOFORN present.
fn e038_dos_dissem_requires_noforn(attrs: &marque_ism::CanonicalAttrs) -> Vec<ConstraintViolation> {
    let has_nodis_or_exdis = attrs.non_ic_dissem.iter().any(|d| {
        matches!(
            d,
            marque_ism::NonIcDissem::Nodis | marque_ism::NonIcDissem::Exdis
        )
    });
    if !has_nodis_or_exdis {
        return Vec::new();
    }
    let has_noforn = attrs
        .dissem_controls
        .iter()
        .any(|d| matches!(d, marque_ism::DissemControl::Nf));
    if has_noforn {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "E038/nodis-or-exdis-requires-noforn",
        message: "NODIS and EXDIS may be used only with NOFORN information".to_owned(),
        citation: "CAPCO-2016 §H.9 p172 + p174",
    }]
}

/// E022 — CNWDI requires TS or S classification. CAPCO §H.6 p106.
fn e022_cnwdi_floor(attrs: &marque_ism::CanonicalAttrs) -> Vec<ConstraintViolation> {
    let has_cnwdi = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Rd(rd) if rd.cnwdi));
    if !has_cnwdi {
        return Vec::new();
    }
    let level = attrs.us_classification();
    let valid = matches!(
        level,
        Some(Classification::TopSecret | Classification::Secret)
    );
    if valid {
        return Vec::new();
    }
    let level_str = level.map(|c| c.banner_str()).unwrap_or("unknown");
    vec![ConstraintViolation {
        constraint_label: "E022/CNWDI-classification-floor",
        message: format!(
            "CNWDI may only be used with TOP SECRET or SECRET RD; \
             current classification is {level_str}"
        ),
        citation: "CAPCO-2016 §H.6 p106",
    }]
}

/// E024 — RD takes precedence over FRD/TFNI. Fires when RD AND any of
/// (FRD, TFNI) are present. The wrapper enumerates per-element to emit one
/// `Diagnostic` per offending marking with byte-precise spans; this helper
/// emits ONE `ConstraintViolation` whose presence signals the wrapper to do
/// that work. CAPCO §H.6 p104.
fn e024_rd_precedence(attrs: &marque_ism::CanonicalAttrs) -> Vec<ConstraintViolation> {
    let has_rd = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Rd(_)));
    if !has_rd {
        return Vec::new();
    }
    let has_superseded = attrs.aea_markings.iter().any(|a| {
        matches!(
            a,
            marque_ism::AeaMarking::Frd(_) | marque_ism::AeaMarking::Tfni
        )
    });
    if !has_superseded {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "E024/rd-precedence",
        message: "RD takes precedence over FRD/TFNI; FRD/TFNI should not appear alongside RD"
            .to_owned(),
        citation: "CAPCO-2016 §H.6 p104",
    }]
}

/// E025 — UCNI may only be used with UNCLASSIFIED. Fires when DOD/DOE UCNI
/// is present AND the classification level is above UNCLASSIFIED.
/// CAPCO §H.6 p116 (DOD UCNI) / p118 (DOE UCNI).
///
/// Note on T035 refactor: the Phase 3 catalog entry was
/// `Conflicts { left: TOK_UCNI, right: AnyInCategory(CAT_CLASSIFICATION) }`.
/// That shape would fire on `classification.is_some()` — including
/// `Some(Unclassified)` — because `satisfies(AnyInCategory(CAT_CLASSIFICATION))`
/// is `true` whenever a classification field is populated at all. The
/// hand-written legacy rule fires only when `classification >
/// Unclassified`. Converting to `Custom` closed that semantic gap: a
/// valid `U//UCNI` marking (CAPCO §H.6) would have tripped the
/// `Conflicts` variant but passes the hand-written predicate. This
/// helper matches the hand-written predicate exactly (early-return
/// on `Some(Unclassified)`).
fn e025_ucni_classification(attrs: &marque_ism::CanonicalAttrs) -> Vec<ConstraintViolation> {
    let has_ucni = attrs.aea_markings.iter().any(|a| {
        matches!(
            a,
            marque_ism::AeaMarking::DodUcni | marque_ism::AeaMarking::DoeUcni
        )
    });
    if !has_ucni {
        return Vec::new();
    }
    let is_unclassified = attrs
        .us_classification()
        .is_some_and(|c| c == Classification::Unclassified);
    if is_unclassified {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "E025/ucni-conflicts-classification",
        message: "DOD/DOE UCNI may only be used with UNCLASSIFIED information".to_owned(),
        citation: "CAPCO-2016 §H.6 p116",
    }]
}

/// W002 — US classification + FGI marker is commingling. Always fires when
/// both are present; the wrapper filters by `RuleContext::marking_type ==
/// Portion`. CAPCO §H.7 lines 8254-8268.
fn w002_us_commingled_with_fgi(attrs: &marque_ism::CanonicalAttrs) -> Vec<ConstraintViolation> {
    if attrs.us_classification().is_none() || attrs.fgi_marker.is_none() {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "W002/us-commingled-with-fgi",
        message: "portion mark comingles US classification with FGI; \
                  consider splitting into separate US and foreign paragraphs"
            .to_owned(),
        citation: "CAPCO-2016 §H.7 p124",
    }]
}

/// `capco/joint-requires-usa` — JOINT classifications must list USA in BOTH
/// `joint.countries` AND `rel_to`. CAPCO §H.3 p55 (USA always included in
/// JOINT [LIST]) + §H.3 p57 (Requires REL TO USA, LIST).
fn joint_requires_usa(attrs: &marque_ism::CanonicalAttrs) -> Vec<ConstraintViolation> {
    let joint = match &attrs.classification {
        Some(marque_ism::MarkingClassification::Joint(j)) => j,
        _ => return Vec::new(),
    };
    let has_usa_in_rel_to = attrs.rel_to.contains(&CountryCode::USA);
    let joint_includes_usa = joint.countries.contains(&CountryCode::USA);
    if has_usa_in_rel_to && joint_includes_usa {
        return Vec::new();
    }
    vec![ConstraintViolation {
        constraint_label: "capco/joint-requires-usa",
        message: "JOINT classifications must list USA in both the \
                  classification countries and REL TO"
            .to_owned(),
        citation: "CAPCO-2016 §H.3 pp 55–57",
    }]
}

// ---------------------------------------------------------------------------
// HCS constraint handler (CAPCO-2016 §H.4 pp 62–66)
// ---------------------------------------------------------------------------

/// Evaluate the `Constraint::Custom("HCS-system-constraints")` sample.
///
/// CAPCO-2016 §H.4 (pp 62–66) defines the interlocking HCS rules:
///
/// 1. **Bare `HCS` (no compartment)** is a legacy form (§H.4 p62). It
///    must be remarked to `HCS-P`, `HCS-O`, or `HCS-O-P`, which requires
///    document-level analysis (the correct variant depends on whether
///    the content is HUMINT product, operations, or both). Legacy
///    `C//HCS` (CONFIDENTIAL with bare HCS -- no compartment) must
///    additionally be identified to the originator for correction.
/// 2. **`HCS-O`** (§H.4 p64) **requires ORCON and NOFORN** and must
///    **not** include ORCON-USGOV (banner would drop -USGOV).
/// 3. **`HCS-P`** (§H.4 p66) **requires NOFORN**; ORCON or ORCON-USGOV
///    **may** be used (permitted, not required).
/// 4. **`HCS-O` / `HCS-P`** are only authorized for SECRET and TOP
///    SECRET classifications (§H.4 p64 / p66).
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
    attrs: &marque_ism::CanonicalAttrs,
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
            // Bare HCS — legacy per CAPCO-2016 §H.4 p62.
            out.push(marque_scheme::ConstraintViolation {
                constraint_label: "HCS-legacy-bare",
                message: "Bare HCS is legacy; remark to HCS-P, HCS-O, or HCS-O-P per CAPCO-2016 \
                     §H.4 p62 (requires document-level analysis)."
                    .to_owned(),
                citation,
            });
            if classification == Some(Classification::Confidential) {
                out.push(marque_scheme::ConstraintViolation {
                    constraint_label: "HCS-legacy-confidential",
                    message: "Legacy CONFIDENTIAL//HCS: identify to originator for correction \
                              per CAPCO-2016 §H.4 p62."
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
                                      CAPCO-2016 §H.4 p64."
                                .to_owned(),
                            citation,
                        });
                    }
                    if !has_orcon {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-O-requires-ORCON",
                            message: "HCS-O requires ORCON per CAPCO-2016 §H.4 p64.".to_owned(),
                            citation,
                        });
                    }
                    if has_orcon_usgov {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-O-forbids-ORCON-USGOV",
                            message: "HCS-O must not be used with ORCON-USGOV per CAPCO-2016 \
                                      §H.4 p64."
                                .to_owned(),
                            citation,
                        });
                    }
                    // HCS-O requires NOFORN per CAPCO-2016 §H.4 p64
                    // ("Relationship(s) to Other Markings: ... Requires
                    // ORCON and NOFORN"). The ORCON side is enforced
                    // above; NOFORN is the second mandatory side. Same
                    // shape as the HCS-P NOFORN-required predicate
                    // below; tracked-and-resolved per #304.
                    let has_noforn = attrs.dissem_controls.contains(&DissemControl::Nf);
                    if !has_noforn {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-O-requires-NOFORN",
                            message: "HCS-O requires NOFORN per CAPCO-2016 §H.4 p64.".to_owned(),
                            citation,
                        });
                    }
                }
                "P" => {
                    if !high_enough {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-P-classification-floor",
                            message: "HCS-P is only authorized for SECRET and TOP SECRET per \
                                      CAPCO-2016 §H.4 p66."
                                .to_owned(),
                            citation,
                        });
                    }
                    // HCS-P requires NOFORN per CAPCO-2016 §H.4 p66
                    // ("Relationship(s) to Other Markings: ... Requires
                    // NOFORN"). ORCON / ORCON-USGOV are permitted but
                    // not required ("ORCON or ORCON-USGOV may be
                    // used."), so the ORCON-required predicate that
                    // previously fired here was over-strict; it is
                    // dropped in favor of the actually-required
                    // NOFORN predicate.
                    let has_noforn = attrs.dissem_controls.contains(&DissemControl::Nf);
                    if !has_noforn {
                        out.push(marque_scheme::ConstraintViolation {
                            constraint_label: "HCS-P-requires-NOFORN",
                            message: "HCS-P requires NOFORN per CAPCO-2016 §H.4 p66.".to_owned(),
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
    // storage (CVE enum vs structural) that `CanonicalAttrs` carries
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
            message: "HCS requires a compartment (O or P); remark to HCS-P, HCS-O, or HCS-O-P \
                 per CAPCO-2016 §H.4 p62 (requires document-level analysis)."
                .to_owned(),
            citation,
        });
        if classification == Some(Classification::Confidential) {
            out.push(marque_scheme::ConstraintViolation {
                constraint_label: "HCS-legacy-confidential",
                message: "Legacy CONFIDENTIAL//HCS: identify to originator for correction per \
                          CAPCO-2016 §H.4 p62."
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
    /// `CanonicalAttrs::us_classification` for test readability.
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
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use marque_ism::{CanonicalAttrs, CountryCode, DissemControl, MarkingClassification};

    fn mk_attrs() -> CanonicalAttrs {
        let mut a = CanonicalAttrs::default();
        a.classification = Some(MarkingClassification::Us(Classification::Secret));
        a
    }

    // capco_category_contains — all branches

    #[test]
    fn category_contains_detects_noforn_in_dissem() {
        let mut a = mk_attrs();
        a.dissem_controls = vec![DissemControl::Nf].into();
        let m = CapcoMarking::new(a);
        assert!(capco_category_contains(&m, CAT_DISSEM, TOK_NOFORN));
    }

    #[test]
    fn category_contains_returns_false_on_absent_token() {
        let a = mk_attrs();
        let m = CapcoMarking::new(a);
        assert!(!capco_category_contains(&m, CAT_DISSEM, TOK_NOFORN));
    }

    #[test]
    fn satisfies_tok_usa_reads_rel_to_for_country_code_usa() {
        // Pin the `TokenRef::Token(TOK_USA)` predicate path
        // touched by issue #183 PR-A's `Trigraph::USA` →
        // `CountryCode::USA` rename. No constraint in the current
        // catalog dispatches `TokenRef::Token(TOK_USA)` (USA-in-
        // REL-TO is read directly by the rule layer), but the
        // `satisfies_attrs` arm exists for future T035b consumption
        // and must read `rel_to` correctly.
        let scheme = CapcoScheme::new();
        let mut a = mk_attrs();
        a.rel_to = vec![CountryCode::USA].into();
        let m = CapcoMarking::new(a);
        assert!(scheme.satisfies(&m, &TokenRef::Token(TOK_USA)));

        let m_empty = CapcoMarking::new(mk_attrs());
        assert!(!scheme.satisfies(&m_empty, &TokenRef::Token(TOK_USA)));
    }

    #[test]
    fn category_contains_returns_false_for_unhandled_pair() {
        let a = mk_attrs();
        let m = CapcoMarking::new(a);
        // An unhandled (category, token) pair — should be false.
        assert!(!capco_category_contains(&m, CAT_REL_TO, TOK_NOFORN));
        assert!(!capco_category_contains(&m, CAT_DISSEM, TOK_USA));
        assert!(!capco_category_contains(&m, CAT_SCI, TOK_NOFORN));
    }

    // capco_category_has_values — all branches

    #[test]
    fn category_has_values_rel_to_populated() {
        let mut a = mk_attrs();
        a.rel_to = vec![CountryCode::USA].into();
        let m = CapcoMarking::new(a);
        assert!(capco_category_has_values(&m, CAT_REL_TO));
    }

    #[test]
    fn category_has_values_rel_to_empty() {
        let a = mk_attrs();
        let m = CapcoMarking::new(a);
        assert!(!capco_category_has_values(&m, CAT_REL_TO));
    }

    #[test]
    fn category_has_values_dissem_populated() {
        let mut a = mk_attrs();
        a.dissem_controls = vec![DissemControl::Nf].into();
        let m = CapcoMarking::new(a);
        assert!(capco_category_has_values(&m, CAT_DISSEM));
    }

    #[test]
    fn category_has_values_dissem_empty() {
        let m = CapcoMarking::new(mk_attrs());
        assert!(!capco_category_has_values(&m, CAT_DISSEM));
    }

    #[test]
    fn category_has_values_sci_populated_via_sci_controls() {
        let mut a = mk_attrs();
        a.sci_controls = vec![marque_ism::SciControl::Si].into();
        let m = CapcoMarking::new(a);
        assert!(capco_category_has_values(&m, CAT_SCI));
    }

    #[test]
    fn category_has_values_sci_empty() {
        let m = CapcoMarking::new(mk_attrs());
        assert!(!capco_category_has_values(&m, CAT_SCI));
    }

    #[test]
    fn category_has_values_unhandled_returns_true() {
        // Unhandled categories default to true ("non-empty / unknown")
        // so `Empty` predicates on them stay inert.
        let m = CapcoMarking::new(mk_attrs());
        assert!(capco_category_has_values(&m, CAT_SAR));
        assert!(capco_category_has_values(&m, CAT_AEA));
        assert!(capco_category_has_values(&m, CAT_FGI_MARKER));
    }

    // capco_category_clear — all branches

    #[test]
    fn category_clear_empties_rel_to() {
        let mut a = mk_attrs();
        a.rel_to = vec![CountryCode::USA].into();
        let mut m = CapcoMarking::new(a);
        capco_category_clear(&mut m, CAT_REL_TO);
        assert!(m.0.rel_to.is_empty());
    }

    #[test]
    fn category_clear_empties_dissem() {
        let mut a = mk_attrs();
        a.dissem_controls = vec![DissemControl::Nf].into();
        let mut m = CapcoMarking::new(a);
        capco_category_clear(&mut m, CAT_DISSEM);
        assert!(m.0.dissem_controls.is_empty());
    }

    #[test]
    fn category_clear_unhandled_is_noop() {
        let mut a = mk_attrs();
        a.rel_to = vec![CountryCode::USA].into();
        let mut m = CapcoMarking::new(a);
        capco_category_clear(&mut m, CAT_SCI);
        // REL TO untouched — other-category clear was a no-op.
        assert_eq!(m.0.rel_to.len(), 1);
    }

    // capco_category_replace — all branches

    #[test]
    fn category_replace_rel_to_copies_from_source() {
        let mut src_attrs = CanonicalAttrs::default();
        src_attrs.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();
        let src = CapcoMarking::new(src_attrs);

        let mut dst = CapcoMarking::new(mk_attrs());
        capco_category_replace(&mut dst, CAT_REL_TO, &src);
        assert_eq!(dst.0.rel_to.len(), 2);
    }

    #[test]
    fn category_replace_dissem_copies_from_source() {
        let mut src_attrs = CanonicalAttrs::default();
        src_attrs.dissem_controls = vec![DissemControl::Nf].into();
        let src = CapcoMarking::new(src_attrs);

        let mut dst = CapcoMarking::new(mk_attrs());
        capco_category_replace(&mut dst, CAT_DISSEM, &src);
        assert_eq!(dst.0.dissem_controls.as_ref(), &[DissemControl::Nf]);
    }

    #[test]
    fn category_replace_unhandled_is_noop() {
        let src = CapcoMarking::new(mk_attrs());
        let mut dst = CapcoMarking::new(mk_attrs());
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
        p2.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();

        let out = marque_scheme::MarkingScheme::project(
            &scheme,
            marque_scheme::Scope::Page,
            &[CapcoMarking::new(p1), CapcoMarking::new(p2)],
        );
        // Rewrite should have fired — REL TO cleared.
        assert!(out.0.rel_to.is_empty());
    }

    #[test]
    fn project_applies_declarative_empty_then_replace() {
        // An Empty trigger on an unhandled category (returns false, so
        // rewrite does NOT fire). Verify a Replace action is reachable
        // via a trigger that DOES fire.
        let mut replacement = CanonicalAttrs::default();
        replacement.dissem_controls = vec![DissemControl::Nf].into();

        let rewrites = vec![PageRewrite {
            id: "test/empty-rel-to-triggers-replace-dissem",
            citation: "test",
            trigger: CategoryPredicate::Empty {
                category: CAT_REL_TO,
            },
            action: CategoryAction::Replace {
                category: CAT_DISSEM,
                with: CapcoMarking::new(replacement),
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
            &[CapcoMarking::new(p)],
        );
        assert!(out.0.dissem_controls.contains(&DissemControl::Nf));
    }
}
