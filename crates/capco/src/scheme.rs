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

use marque_ism::{CanonicalAttrs, Classification, CountryCode, PageContext, TokenKind};
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

// PR 3b.C (T026c): RELIDO incompatibility roster sentinels.
// Resolved via `satisfies_attrs` against `attrs.dissem_controls` —
// all four tokens are IC dissem controls living in
// `marque_ism::DissemControl`.
//
// DissemControl variant → CVE string form (from generated values.rs):
//   Relido     → "RELIDO"
//   Displayonly → "DISPLAYONLY"
//   Oc         → "OC"      (ORCON portion abbreviation)
//   OcUsgov    → "OC-USGOV" (ORCON-USGOV portion abbreviation)
pub const TOK_RELIDO: TokenId = TokenId(124);
pub const TOK_DISPLAY_ONLY: TokenId = TokenId(125);
pub const TOK_ORCON: TokenId = TokenId(126);
pub const TOK_ORCON_USGOV: TokenId = TokenId(127);

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
    /// is one valid total ordering of the rewrite vector (it groups
    /// `noforn-clears-rel-to` first as the canonical worked example,
    /// followed by entries 4, 1, 2, 3, 7, 5, 6a, 6b in the order
    /// they appear in the consultant roster). It is **not** the
    /// scheduler's topological order — `noforn-clears-rel-to` reads
    /// `CAT_DISSEM` which entries 5/6a/6b write, so the scheduler
    /// orders it AFTER those entries. `Engine::new` runs Kahn's
    /// algorithm at construction; runtime execution order is
    /// determined by the scheduler, not by this `Vec` order.
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
            // ---- E022 retired in PR 3b.D (T026d) -----------------
            //
            // The CNWDI classification floor moved into the class-
            // floor catalog block below as
            // `E058/CNWDI-classification-floor`. The legacy
            // `E022/CNWDI-classification-floor` entry that previously
            // lived here is removed because (a) the catalog walker
            // emits the diagnostic via `E058/...`, and (b) keeping the
            // `E022/...` entry alongside the `E058/...` entry produced
            // a dead duplicate constraint row that never fires (the
            // dispatch in `evaluate_custom_by_attrs` no longer routes
            // to a predicate for it). Per
            // `feedback_pre_users_no_deprecation_phasing.md`, no
            // alias map is preserved.

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
            // ---- E025 retired in PR 3b.D (T026d) -----------------
            //
            // The UCNI ceiling invariant moved into the class-floor
            // catalog block below as TWO rows
            // (`E058/DOD-UCNI-classification-ceiling` at §H.6 p116 and
            // `E058/DOE-UCNI-classification-ceiling` at §H.6 p118),
            // split per PM decision #1 so each variant carries its
            // own §H.6 sub-page citation. The legacy
            // `E025/ucni-conflicts-classification` aggregated entry
            // that previously lived here is removed for the same
            // reason as the E022 entry above (the dispatch in
            // `evaluate_custom_by_attrs` no longer routes to a
            // predicate for it).

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
            // ---- E054: RELIDO ⊥ NOFORN (§H.8 p154) ------------------
            //
            // §H.8 RELIDO entry p154, Relationship(s) to Other Markings:
            // "Cannot be used with NOFORN or DISPLAY ONLY."
            // Verified against `crates/capco/docs/CAPCO-2016.md` line 3808.
            //
            // Rationale: RELIDO authorizes foreign release under a
            // Secretary of Defense / SFDRA-mediated arrangement;
            // NOFORN is the most restrictive FD&R marking, prohibiting
            // any foreign national access. The two are in direct semantic
            // conflict on the FD&R axis.
            //
            // Reciprocal (doc-comment only — NOT the primary citation
            // under D13 single-citation discipline):
            // §H.8 NOFORN entry p145 line 3585: "Cannot be used with
            // REL TO, RELIDO, EYES ONLY, or DISPLAY ONLY."
            //
            // LHS = asserting token (RELIDO at p154); wrapper span
            // anchors at RELIDO per PM Q1 resolution.
            Constraint::Conflicts {
                name: "E054/relido-conflicts-noforn",
                left: TokenRef::Token(TOK_RELIDO),
                right: TokenRef::Token(TOK_NOFORN),
                label: "CAPCO-2016 §H.8 p154",
            },
            // ---- E055: RELIDO ⊥ DISPLAY ONLY (§H.8 p154) ------------
            //
            // §H.8 RELIDO entry p154, Relationship(s) to Other Markings:
            // "Cannot be used with NOFORN or DISPLAY ONLY."
            // Same cited line as E054 — both NOFORN and DISPLAY ONLY
            // appear in the single prohibition sentence.
            // Verified against `crates/capco/docs/CAPCO-2016.md` line 3808.
            //
            // Rationale: DISPLAY ONLY authorizes viewing but not release
            // or duplication; RELIDO defers release to a
            // Secretary of Defense / SFDRA arrangement. The two FD&R
            // semantics — "view in place" vs. "release deferred pending
            // SFDRA authorization" — are in direct conflict.
            //
            // Reciprocal (doc-comment only — NOT the primary citation):
            // §H.8 DISPLAY ONLY entry p163 line 4050: "Cannot be used
            // with RELIDO or NOFORN."
            //
            // LHS = asserting token (RELIDO at p154); wrapper span
            // anchors at RELIDO per PM Q1 resolution.
            Constraint::Conflicts {
                name: "E055/relido-conflicts-display-only",
                left: TokenRef::Token(TOK_RELIDO),
                right: TokenRef::Token(TOK_DISPLAY_ONLY),
                label: "CAPCO-2016 §H.8 p154",
            },
            // ---- E056: ORCON ⊥ RELIDO (§H.8 p136) -------------------
            //
            // §H.8 ORCON entry p136, Relationship(s) to Other Markings:
            // "May not be used with RELIDO."
            // Full surrounding prose (lines 3361–3363):
            // "May not be used with ORCON-USGOV in a portion mark or
            // banner line. May be used with NOFORN, REL TO, DISPLAY
            // ONLY. May not be used with RELIDO."
            // Verified against `crates/capco/docs/CAPCO-2016.md` line 3363.
            //
            // Citation authority note: the asserting prose lives on the
            // ORCON template (p136), NOT in RELIDO's p154
            // Relationship(s) section. §H.8 p154 does NOT mention ORCON.
            // The directionality is real: this entry carries the ORCON
            // assertion, and the catalog row anchors at the page where
            // that assertion is made.
            //
            // Rationale: ORCON requires originator approval before
            // further dissemination; RELIDO defers release to a SFDRA
            // arrangement that bypasses originator approval. The two
            // control semantics are incompatible.
            //
            // LHS = asserting token (ORCON at p136); wrapper span
            // anchors at ORCON per PM Q1 + Q2 resolution.
            Constraint::Conflicts {
                name: "E056/orcon-conflicts-relido",
                left: TokenRef::Token(TOK_ORCON),
                right: TokenRef::Token(TOK_RELIDO),
                label: "CAPCO-2016 §H.8 p136",
            },
            // ---- E057: ORCON-USGOV ⊥ RELIDO (§H.8 p140) ------------
            //
            // §H.8 ORCON-USGOV entry p140, Relationship(s) to Other
            // Markings: "May not be used with RELIDO."
            // Full surrounding prose (lines 3442–3446):
            // "May not be used with ORCON in a portion mark or banner
            // line. May be used with NOFORN, REL TO, DISPLAY ONLY.
            // May not be used with RELIDO."
            // Verified against `crates/capco/docs/CAPCO-2016.md` line 3444.
            //
            // Citation page note: the ORCON-USGOV template begins p139
            // (line 3407); the Relationship(s) subsection straddles
            // p139–p140. The RELIDO exclusion appears on p140. The
            // catalog primary is p140 because that is where the specific
            // RELIDO prose occurs. Verified against line 3444 in the
            // vendored source.
            //
            // Rationale: ORCON-USGOV is the USGOV-pre-approved variant
            // of ORCON; it carries the same originator-approval semantic
            // conflict with RELIDO's SFDRA-deferred release arrangement.
            //
            // LHS = asserting token (ORCON-USGOV at p140); wrapper span
            // anchors at ORCON-USGOV per PM Q1 + Q2 resolution.
            Constraint::Conflicts {
                name: "E057/orcon-usgov-conflicts-relido",
                left: TokenRef::Token(TOK_ORCON_USGOV),
                right: TokenRef::Token(TOK_RELIDO),
                label: "CAPCO-2016 §H.8 p140",
            },
            // ================================================================
            // PR 3b.D (T026d) — class-floor catalog (§3.4.6)
            // ================================================================
            //
            // Per-marking classification floors per `marque-applied.md`
            // §3.4.6: presence of marking M requires the page's
            // classification level to be at least F(M). This is *not* part
            // of the lattice axis itself (the class chain is
            // `OrdMax(TS > CTS > S > NS > C > NC > R > NR > U > NU)`); it
            // is a *constraint* over the joint fact-set: the page is
            // malformed if M is present and the class level is below F(M).
            //
            // # Why Constraint::Custom (architectural choice — Option A)
            //
            // Class-floor RHS is "classification level ≥ F(M)" — a
            // partial-order threshold over the OrdMax classification
            // chain, not a token-presence assertion. The existing
            // `Constraint::Requires` shape is dyadic token-presence; the
            // class-floor predicate doesn't fit. PR 3.7 (T108b) may
            // revisit and re-classify to a primitive form
            // (e.g., `TokenRef::ClassAtLeast(ClassLevel)` or
            // `Constraint::ClassFloor`) once that primitive lands in
            // marque-scheme. See
            // `docs/plans/2026-05-08-pr3b-D-class-floor-catalog-plan.md`
            // §3 for the architectural rationale.
            //
            // # Why family granularity (~26 rows, not ~38)
            //
            // The §3.4.6 author wrote at family granularity (HCS-[comp][sub],
            // SI-[comp], TK, RD-SG, etc. — pattern-matching family rows,
            // not enumerated per-template rows). Family granularity is
            // deliberate: clean lattice algebra, stable ImplTable shape
            // that survives PR 3.7's closure-operator landing without
            // re-shaping, uniform §-citation discipline. Family-pattern
            // matching is implemented in the predicate body
            // (`class_floor_catalog_eval`) — each predicate iterates the
            // relevant axis (`attrs.sci_markings`, `attrs.aea_markings`,
            // etc.) looking for any token matching the family.
            //
            // # Per-row name and walker rule-ID
            //
            // The single walker `DeclarativeClassFloorRule` (rule ID
            // `E058`) emits all diagnostics. Each catalog row's `name`
            // takes one of two forms:
            //
            //   - `E058/<purpose>` for rows that REPLACE a retired
            //     legacy rule. Specifically:
            //     `E058/CNWDI-classification-floor` (replaces retired
            //     E022), `E058/SAR-classification-floor` (replaces
            //     retired E027), `E058/DOD-UCNI-classification-ceiling`
            //     and `E058/DOE-UCNI-classification-ceiling` (replace
            //     retired E025; split per PM decision so each carries
            //     its own §H.6 sub-page citation).
            //   - `class-floor/<marking>` for rows with no retired-rule
            //     predecessor (e.g., `class-floor/HCS-comp-sub`,
            //     `class-floor/SI-comp`, `class-floor/BALK`,
            //     `class-floor/passthrough-BUR`).
            //
            // Per-row identification flows via the catalog's `name`
            // field into `ConstraintViolation.constraint_label` and is
            // referenced in `Diagnostic.message` for human-readable
            // identification.
            //
            // Severity-config compatibility for the legacy IDs (E022,
            // E025, E027) is intentionally NOT preserved. Per project
            // memory `feedback_pre_users_no_deprecation_phasing.md`:
            // marque is pre-users, so we don't carry alias maps,
            // retained namespaces, or phased deprecation.
            // `.marque.toml` files keying class-floor severity
            // overrides MUST use `E058` (walker-level) — there's no
            // per-row severity-override surface in PR D.
            //
            // # Citation methodology
            //
            // Each row's `label` carries the §3.4.6 author's chosen
            // citation. Some rows cite operative-authority pages
            // (precedence rules, FD&R-supersession anchors, AEA-chain
            // references) rather than the marking-template-body page; the
            // §3.4.6 author's choice is authoritative per
            // `marque-applied.md` line 783-808. The marking-body floor
            // language is verifiable in the H.x section body of each
            // marking; see the planning doc §2 for the verification
            // matrix.
            //
            // ---- §2.1 Floor TS — single classification level (5 rows) -
            Constraint::Custom {
                name: "class-floor/HCS-comp-sub",
                label: "CAPCO-2016 §H.4",
            },
            Constraint::Custom {
                name: "class-floor/SI-comp",
                label: "CAPCO-2016 §H.4",
            },
            Constraint::Custom {
                name: "class-floor/TK-BLFH",
                label: "CAPCO-2016 §H.4",
            },
            Constraint::Custom {
                name: "class-floor/BALK",
                label: "CAPCO-2016 §H.7 Appendix B",
            },
            Constraint::Custom {
                name: "class-floor/BOHEMIA",
                label: "CAPCO-2016 §H.7 Appendix B",
            },
            // ---- §2.2 Floor S — TS-or-S allowed (8 rows) --------------
            Constraint::Custom {
                name: "class-floor/HCS-comp",
                label: "CAPCO-2016 §H.4",
            },
            Constraint::Custom {
                name: "class-floor/RSV-comp",
                label: "CAPCO-2016 §H.4",
            },
            Constraint::Custom {
                name: "class-floor/TK",
                label: "CAPCO-2016 §H.4",
            },
            Constraint::Custom {
                name: "class-floor/RD-SG",
                label: "CAPCO-2016 §H.6 p113",
            },
            Constraint::Custom {
                name: "class-floor/FRD-SG",
                label: "CAPCO-2016 §H.6 p113",
            },
            // CNWDI — replaces retired E022. Per PM directive #5 + the
            // PR 3b.D planning doc §5.2, catalog row names use the
            // walker-prefixed form `E058/<suffix>`. Per
            // `feedback_pre_users_no_deprecation_phasing.md` (marque is
            // pre-users), severity-config back-compat for the retiring
            // E022 rule ID is not preserved — users keying `.marque.toml`
            // at `E022` will need to migrate to `E058`.
            Constraint::Custom {
                name: "E058/CNWDI-classification-floor",
                label: "CAPCO-2016 §H.6 p104",
            },
            Constraint::Custom {
                name: "class-floor/RSEN",
                label: "CAPCO-2016 §H.8 p149",
            },
            Constraint::Custom {
                name: "class-floor/IMCON",
                label: "CAPCO-2016 §H.8 p144",
            },
            // ---- §2.3 Floor C — any classified level (8 rows) --------
            Constraint::Custom {
                name: "class-floor/SI",
                label: "CAPCO-2016 §H.4",
            },
            // SAR — replaces retired E027. Walker-prefixed name per PM
            // directive #5.
            Constraint::Custom {
                name: "E058/SAR-classification-floor",
                label: "CAPCO-2016 §H.5",
            },
            Constraint::Custom {
                name: "class-floor/RD",
                label: "CAPCO-2016 §H.6 p104",
            },
            Constraint::Custom {
                name: "class-floor/FRD",
                label: "CAPCO-2016 §H.6 p104",
            },
            Constraint::Custom {
                name: "class-floor/TFNI",
                label: "CAPCO-2016 §H.6 p107",
            },
            Constraint::Custom {
                name: "class-floor/ATOMAL",
                label: "CAPCO-2016 §H.7 Appendix B",
            },
            Constraint::Custom {
                name: "class-floor/ORCON",
                label: "CAPCO-2016 §H.8 p136",
            },
            Constraint::Custom {
                name: "class-floor/EYES-ONLY",
                label: "CAPCO-2016 §H.8 p152",
            },
            // ---- §2.4 Floor =U — UNCLASSIFIED-only (2 rows; UCNI split) -
            //
            // Replaces retired `DeclarativeUcniClassificationRule` (E025).
            // Split per PM decision into two rows (DOD UCNI and DOE UCNI)
            // so each row carries its own §H.6 sub-page citation. Both
            // use the walker-prefixed name `E058/<suffix>`.
            Constraint::Custom {
                name: "E058/DOD-UCNI-classification-ceiling",
                label: "CAPCO-2016 §H.6 p116",
            },
            Constraint::Custom {
                name: "E058/DOE-UCNI-classification-ceiling",
                label: "CAPCO-2016 §H.6 p118",
            },
            // ---- §2.6 Unknown-floor passthrough (4 rows) -------------
            //
            // Per `marque-applied.md` §3.4.6 unknown-floor sub-catalog +
            // §3.7 passthrough policy. Provisional `F(M) = C` (minimal
            // classified). Severity Warn (per §3.4.6 Q-3.4.6b) — fired by
            // the walker at the per-row severity stored in the catalog
            // table.
            Constraint::Custom {
                name: "class-floor/passthrough-BUR",
                label: "marque-applied.md §3.7 (passthrough); CAPCO-2016 unmapped",
            },
            Constraint::Custom {
                name: "class-floor/passthrough-HCS-X",
                label: "marque-applied.md §3.7 (passthrough); CAPCO-2016 unmapped",
            },
            Constraint::Custom {
                name: "class-floor/passthrough-KLM",
                label: "marque-applied.md §3.7 (passthrough); CAPCO-2016 unmapped",
            },
            Constraint::Custom {
                name: "class-floor/passthrough-MVL",
                label: "marque-applied.md §3.7 (passthrough); CAPCO-2016 unmapped",
            },
            // ================================================================
            // PR 3b.E (T026e) — SCI per-system catalog (§H.4)
            // ================================================================
            //
            // Per-SCI-system companion-required / forbid-companion
            // invariants per CAPCO-2016 §H.4. Five rows at family
            // granularity covering the §H.4 invariants that PR 3b.D's
            // class-floor catalog does NOT already cover (companion-
            // required: ORCON, NOFORN; forbid-companion: ORCON-USGOV).
            // The class-floor portions of the retired E044/E045/E046/
            // E048/E049/E050 rules are absorbed by PR 3b.D's class-floor
            // rows and are not duplicated here.
            //
            // # Why Constraint::Custom (architectural choice)
            //
            // The §H.4 invariants are companion-presence (ORCON, NOFORN)
            // + companion-forbid (ORCON-USGOV) + per-row fix-shape
            // (zero-width insertion at the end of the IC dissem block,
            // or a span replacement on the dominated token) — none of
            // which fit the existing primitive surface. PR 4 (per-
            // category Lattice impls per Stage 3 of plan.md:263) MAY
            // revisit and re-classify to a `CompanionRequired<Set>` /
            // `Forbid<Set>` primitive on `marque-scheme` when those
            // primitives land. The walker stays until that retirement.
            // See `docs/plans/2026-05-08-pr3b-E-sci-per-system-collapse-plan.md`
            // §3 for the rule-by-rule analysis; tasks.md T026e for the
            // walker landing.
            //
            // # Per-row name and walker rule-ID
            //
            // The single walker `DeclarativeSciPerSystemRule` (rule ID
            // `E059`) emits all diagnostics. Each catalog row's `name`
            // takes the `sci-per-system/<purpose>` form. Per project
            // memory `feedback_pre_users_no_deprecation_phasing.md`
            // (marque is pre-users), severity-config back-compat for
            // the retiring E042–E051 rule IDs is not preserved — users
            // keying `.marque.toml` at any of `E042`..`E051` must
            // migrate to `E059`.
            Constraint::Custom {
                name: "sci-per-system/HCS-O-companions",
                label: "CAPCO-2016 §H.4 p64",
            },
            Constraint::Custom {
                name: "sci-per-system/HCS-P-NOFORN",
                label: "CAPCO-2016 §H.4 p66",
            },
            Constraint::Custom {
                name: "sci-per-system/HCS-P-sub-companions",
                label: "CAPCO-2016 §H.4 p68",
            },
            Constraint::Custom {
                name: "sci-per-system/SI-G-companions",
                label: "CAPCO-2016 §H.4 p80",
            },
            Constraint::Custom {
                name: "sci-per-system/TK-compartment-NOFORN",
                label: "CAPCO-2016 §H.4 p87 + p91 + p95",
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
            // PR 3b.C (T026c): RELIDO incompatibility sentinels.
            // Pattern mirrors TOK_NOFORN above — scan `dissem_controls`
            // for the matching DissemControl variant. All four variants
            // exist in the generated values.rs; no new marque-ism edits
            // needed (Constitution VII compliance verified).
            TOK_RELIDO => attrs
                .dissem_controls
                .iter()
                .any(|d| matches!(d, DissemControl::Relido)),
            TOK_DISPLAY_ONLY => attrs
                .dissem_controls
                .iter()
                .any(|d| matches!(d, DissemControl::Displayonly)),
            TOK_ORCON => attrs
                .dissem_controls
                .iter()
                .any(|d| matches!(d, DissemControl::Oc)),
            TOK_ORCON_USGOV => attrs
                .dissem_controls
                .iter()
                .any(|d| matches!(d, DissemControl::OcUsgov)),
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
///
/// PR 3b.D (T026d): catalog-row names with the prefixes
/// `class-floor/` or `E058/` are dispatched to
/// [`class_floor_catalog_eval`] over the static
/// [`CLASS_FLOOR_CATALOG`] table. The retired `e022_cnwdi_floor` /
/// `e025_ucni_classification` helpers were absorbed into the
/// catalog's static-table form; their replacement catalog rows
/// (`E058/CNWDI-classification-floor`,
/// `E058/DOD-UCNI-classification-ceiling`,
/// `E058/DOE-UCNI-classification-ceiling`,
/// `E058/SAR-classification-floor`) reuse the walker's `E058`
/// prefix rather than the legacy E022/E025/E027 IDs. Per project
/// memory `feedback_pre_users_no_deprecation_phasing.md`,
/// severity-config back-compat for the legacy IDs is intentionally
/// not preserved; `.marque.toml` keys must use `E058` (walker-level).
fn evaluate_custom_by_attrs(
    attrs: &marque_ism::CanonicalAttrs,
    name: &'static str,
) -> Vec<ConstraintViolation> {
    if is_class_floor_catalog_name(name) {
        return class_floor_catalog_eval(attrs, name);
    }
    if is_sci_per_system_catalog_name(name) {
        return sci_per_system_catalog_eval(attrs, name);
    }
    match name {
        "E010/HCS-system-constraints" => hcs_system_constraints(attrs, "CAPCO-2016 §H.4 p61-62"),
        "E012/dual-classification" => e012_dual_classification(attrs),
        "E014/joint-requires-rel-to-coverage" => e014_joint_rel_to_coverage(attrs),
        "E021/aea-requires-noforn" => e021_aea_requires_noforn(attrs),
        "E024/rd-precedence" => e024_rd_precedence(attrs),
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

// ===========================================================================
// PR 3b.D (T026d) — Class-floor catalog dispatch (§3.4.6)
// ===========================================================================
//
// `class_floor_catalog_eval` is the static-table dispatcher for the 27
// `Constraint::Custom` rows declared by `build_constraints` under the
// "PR 3b.D (T026d) — class-floor catalog (§3.4.6)" section header.
//
// Each row's predicate has a uniform shape: "if marking M is present in
// `attrs`, the page's classification must satisfy F(M)" where F(M) is
// either a floor (`level >= floor`) or an equality (`level == U`). The
// table stores one entry per row carrying:
//
//   - `name`: catalog row identifier (matches `Constraint::Custom { name }`)
//   - `marking_label`: human-readable marking name for the diagnostic
//   - `presence`: predicate `fn(&CanonicalAttrs) -> bool` checking whether
//      the family pattern is present
//   - `policy`: `ClassFloorPolicy` — either `AtLeast(level)` or `EqualsU`
//   - `severity`: `Severity` — `Error` for enumerated rows, `Warn` for
//      passthrough rows (§3.4.6 Q-3.4.6b)
//   - `citation`: per-row §-citation matching `Constraint::Custom { label }`
//   - `passthrough`: `true` for unknown-floor passthrough rows (drives the
//      diagnostic message variant)
//
// The walker `DeclarativeClassFloorRule` (in `rules_declarative.rs`)
// iterates the table and emits one `Diagnostic` per row whose presence
// predicate fires AND whose floor/equality predicate is violated.
//
// FORWARD LINK to PR 3.7 (T108b): once `TokenRef::ClassAtLeast(ClassLevel)`
// or `Constraint::ClassFloor` lands as a primitive in `marque-scheme`,
// these rows can re-classify from `Constraint::Custom` to the new
// primitive form without changing per-row semantics. See
// `docs/plans/2026-05-08-pr3b-D-class-floor-catalog-plan.md` §3 for the
// architectural rationale.

/// Floor policy for a class-floor catalog row.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ClassFloorPolicy {
    /// Classification level must be ≥ this floor (TS / S / C semantics).
    AtLeast(Classification),
    /// Classification must be exactly UNCLASSIFIED. Used by the UCNI
    /// ceiling rows (§2.4 of the planning doc).
    EqualsU,
}

/// One catalog row. The walker dispatches over the `&[ClassFloorRow]`
/// table; each row owns its presence predicate, floor policy, severity,
/// citation, and human-readable marking label for diagnostic messages.
///
/// # Naming-prefix invariant (PR D R3.2)
///
/// Every row's `name` MUST start with one of two prefixes:
///
///   - **`E058/<purpose>`** — for rows replacing a retired legacy rule
///     (the four E022 / E025 / E027 successors:
///     `E058/CNWDI-classification-floor`,
///     `E058/SAR-classification-floor`,
///     `E058/DOD-UCNI-classification-ceiling`,
///     `E058/DOE-UCNI-classification-ceiling`).
///   - **`class-floor/<marking>`** — for rows with no retired-rule
///     predecessor (e.g., `class-floor/HCS-comp-sub`,
///     `class-floor/SI-comp`, `class-floor/BALK`,
///     `class-floor/passthrough-BUR`).
///
/// The prefix invariant is what makes the
/// [`is_class_floor_catalog_name`] dispatch routing O(1) instead of
/// a linear catalog scan. The
/// `class_floor_catalog_naming_convention` test in
/// `crates/capco/tests/class_floor_catalog.rs` enforces this at
/// build time; adding a row whose name doesn't match either prefix
/// will fail CI.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ClassFloorRow {
    /// Catalog row name — matches the `Constraint::Custom { name }` of
    /// the same logical row. MUST start with `E058/` or
    /// `class-floor/` per the naming-prefix invariant above.
    pub(crate) name: &'static str,
    /// Human-readable marking name for the diagnostic message
    /// (e.g., `"CNWDI"`, `"HCS-P sub-compartment"`, `"BUR family"`).
    pub(crate) marking_label: &'static str,
    /// Marking-presence predicate.
    pub(crate) presence: fn(&marque_ism::CanonicalAttrs) -> bool,
    /// Floor policy.
    pub(crate) policy: ClassFloorPolicy,
    /// Per-row severity (`Error` for enumerated rows, `Warn` for
    /// passthrough rows per §3.4.6 Q-3.4.6b).
    pub(crate) severity: marque_rules::Severity,
    /// Per-row §-citation, matching `Constraint::Custom { label }`.
    pub(crate) citation: &'static str,
    /// True for the unknown-floor passthrough rows. Drives the
    /// diagnostic message variant (passthrough rows quote the §3.7
    /// passthrough-policy framing).
    pub(crate) passthrough: bool,
    /// Diagnostic-span anchor token kind. PR D R2 hot-path optimization
    /// (perf-3): hoisted from the per-diagnostic
    /// `primary_token_kind_for_row` string match in
    /// `rules_declarative.rs`. The walker reads this field directly
    /// when resolving the diagnostic span. `None` means "fall back to
    /// the classification span" (used for NATO rows where the
    /// classification token IS the marking surface).
    pub(crate) primary_kind: Option<marque_ism::TokenKind>,
    /// Coarse axis classifier for the early-out guard. PR D R2 hot-path
    /// optimization (perf-1): the walker reads this once per row and
    /// can skip the entire row when the corresponding axis is empty
    /// in `attrs`. The axis bitfield model is too coarse for the BUR
    /// passthrough family (which dual-reads `sci_controls` AND
    /// `sci_markings`); using a single discriminant per row is
    /// sufficient because the early-out guard only reads
    /// "any-token-present-on-this-axis" flags.
    pub(crate) axis: ClassFloorAxis,
}

/// Coarse axis classifier for a class-floor catalog row's marking
/// presence. Used by the walker's early-out guard to skip rows whose
/// axis is empty in the current `attrs` without invoking the row's
/// presence predicate.
///
/// The classifier is at the marking-axis granularity, NOT the
/// CanonicalAttrs-field granularity — `Sci` covers BOTH `sci_controls`
/// and `sci_markings` because passthrough predicates dual-read; `Aea`
/// covers `aea_markings`; etc. This is a hot-path optimization, not a
/// semantic guard, so coarseness is correct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClassFloorAxis {
    /// SCI markings or SCI controls (covers HCS / SI / TK / RSV +
    /// passthrough BUR / HCS-X / KLM / MVL).
    Sci,
    /// AEA markings (covers RD / FRD / TFNI / CNWDI / SIGMA / UCNI).
    Aea,
    /// SAR markings (covers the SAR floor row).
    Sar,
    /// IC dissemination controls (covers RSEN / IMCON / ORCON / EYES).
    Dissem,
    /// NATO classification system (covers BALK / BOHEMIA / ATOMAL).
    NatoClass,
}

/// Returns true if `name` is a catalog row name dispatched by
/// [`class_floor_catalog_eval`]. Used by `evaluate_custom_by_attrs`
/// to route on the table.
///
/// PR D R3.2 (R3 C1): O(1) prefix check. Every catalog row's `name`
/// follows one of two prefix conventions (see [`ClassFloorRow`]
/// docstring):
///
///   - `E058/<purpose>` for rows replacing a retired legacy rule.
///   - `class-floor/<marking>` for rows with no retired-rule
///     predecessor.
///
/// New catalog rows MUST follow one of these prefixes; the
/// `class_floor_catalog_naming_convention` test in
/// `crates/capco/tests/class_floor_catalog.rs` enforces the
/// invariant at build time so adding a row that doesn't follow the
/// convention fails CI.
fn is_class_floor_catalog_name(name: &str) -> bool {
    name.starts_with("E058/") || name.starts_with("class-floor/")
}

/// Resolve a catalog row by `name`. Returns `None` for unknown
/// names.
///
/// Walked only on the trait/validate path (27-row catalog → linear
/// scan, ≪1 µs) — the walker hot path uses
/// [`class_floor_catalog`] then [`class_floor_eval_row`] directly
/// with no name lookup. A build-time perfect-hash lookup
/// (`phf::Map`) is deferred unless the trait path shows up as a
/// measurable hotspot in profiling.
pub(crate) fn class_floor_row_by_name(name: &str) -> Option<&'static ClassFloorRow> {
    CLASS_FLOOR_CATALOG.iter().find(|row| row.name == name)
}

/// Iterate the full class-floor catalog. Used by the walker
/// `DeclarativeClassFloorRule::check` to dispatch over every row.
pub(crate) fn class_floor_catalog() -> &'static [ClassFloorRow] {
    CLASS_FLOOR_CATALOG
}

/// Single source of truth for the class-floor catalog's
/// presence-check + floor-satisfaction-check + diagnostic message
/// shape. PR D R3.1 (R3 C2): extracted to converge the walker
/// hot-path ([`class_floor_eval_row`]) and the trait/validate path
/// ([`class_floor_catalog_eval`]) on one body so a citation,
/// message-text, or floor-comparison change to one row cannot
/// silently diverge between the two emitters.
///
/// Returns `None` when the row's predicate does not fire (presence
/// false OR floor satisfied). Returns `Some(ConstraintViolation)`
/// when the row fires; the violation carries the row's `name` as
/// `constraint_label`, the formatted diagnostic message, and the
/// row's `citation` verbatim — matching the
/// `marque_scheme::constraint::evaluate` Custom-arm contract.
///
/// The diagnostic message uses the *effective* classification level
/// (reciprocal-raised for NATO / FGI / JOINT classifications via
/// [`marque_ism::MarkingClassification::effective_level`]) so a
/// portion classified `//NATO SECRET//ATOMAL` reports `SECRET` —
/// not `unknown` — even though `attrs.us_classification()` returns
/// `None` for non-US classification kinds. This is the C1 fix from
/// PR #324 R1; see [`class_floor_satisfied`] doc for the AtLeast vs
/// EqualsU split.
fn class_floor_emit(
    attrs: &marque_ism::CanonicalAttrs,
    row: &ClassFloorRow,
) -> Option<ConstraintViolation> {
    if !(row.presence)(attrs) {
        return None;
    }
    if class_floor_satisfied(attrs, row.policy) {
        return None;
    }
    let level_str = attrs
        .classification
        .as_ref()
        .map(|c| c.effective_level().banner_str())
        .unwrap_or("unknown");
    let message = if row.passthrough {
        format!(
            "{} is known from ISM but not enumerated in CAPCO-2016; provisional classification \
             floor is C (classified). Verify against the current ODNI manual; current \
             classification is {level_str}. (See marque-applied.md §3.7 passthrough policy.)",
            row.marking_label
        )
    } else {
        match row.policy {
            ClassFloorPolicy::AtLeast(floor) => format!(
                "{} requires classification ≥ {} ({}); current classification is {level_str}",
                row.marking_label,
                floor.banner_str(),
                row.citation
            ),
            ClassFloorPolicy::EqualsU => format!(
                "{} may only be used with UNCLASSIFIED information ({}); current classification \
                 is {level_str}",
                row.marking_label, row.citation
            ),
        }
    };
    Some(ConstraintViolation {
        constraint_label: row.name,
        message,
        citation: row.citation,
    })
}

/// Direct catalog-row dispatch for the walker's hot path. Skips the
/// `evaluate_custom_by_attrs` → `class_floor_catalog_eval` → name-
/// lookup chain entirely; the walker has the row in hand and calls
/// the predicate fields directly. PR D R2 hot-path optimization
/// (perf-2).
///
/// Returns `None` when the row's predicate does not fire. Returns
/// `Some(message)` when the row fires; the caller pairs that with
/// the row's severity, citation, and span anchor to construct a
/// `Diagnostic`. The caller does not need the
/// [`ConstraintViolation`] envelope — the walker constructs a
/// `Diagnostic` directly from the row's static fields plus the
/// returned message — so this thin wrapper unwraps
/// [`class_floor_emit`]'s return to drop the `constraint_label` /
/// `citation` fields the caller is going to overwrite anyway.
pub(crate) fn class_floor_eval_row(
    attrs: &marque_ism::CanonicalAttrs,
    row: &ClassFloorRow,
) -> Option<String> {
    class_floor_emit(attrs, row).map(|v| v.message)
}

/// Dispatch a single catalog row by name and return at most one
/// `ConstraintViolation`. The trait-path entry point used by
/// [`MarkingScheme::validate`] →
/// [`marque_scheme::constraint::evaluate`] when the catalog row's
/// `Constraint::Custom` arm fires.
///
/// PR D R3.1 (R3 C2): converges through [`class_floor_emit`] so the
/// presence check, floor-satisfaction check, and message-format are
/// not duplicated against the walker's [`class_floor_eval_row`]
/// path.
fn class_floor_catalog_eval(
    attrs: &marque_ism::CanonicalAttrs,
    name: &'static str,
) -> Vec<ConstraintViolation> {
    class_floor_row_by_name(name)
        .and_then(|row| class_floor_emit(attrs, row))
        .map(|v| vec![v])
        .unwrap_or_default()
}

/// Returns true when the classification axis satisfies the floor policy.
///
/// The two policy variants take different views of the classification axis:
///
/// - **`AtLeast(floor)`** uses `MarkingClassification::effective_level`
///   so NATO / FGI / JOINT classifications get reciprocal-raised to
///   their US-equivalent level per `marque-applied.md` §3.4.1 Note (i)
///   (CTS → TS, NS → S, NC → C, NR → R, NU → U). This is the C1 fix
///   from PR #324 R1: before the fix, the NATO catalog rows
///   (BALK / BOHEMIA / ATOMAL) queried `attrs.us_classification()`,
///   which returns `None` for non-US classification kinds, so the
///   reciprocal-raised NATO floors always failed and always emitted a
///   spurious diagnostic — guaranteed false positive on every
///   well-formed NATO portion. The `effective_level()` accessor
///   already lives in `marque-ism` and is the canonical answer to
///   "what's the effective classification level for ordering?";
///   capco-side we just consume it.
///
///   Behavior on a `None` classification (no classification token
///   parsed at all) stays as "fail the floor" — this preserves
///   retired-E022 / retired-E027 semantics where a CNWDI / SAR marking
///   without any classification context is treated as malformed and
///   the floor diagnostic fires.
///
/// - **`EqualsU`** keeps `attrs.us_classification()` semantics. The
///   UCNI ceiling per CAPCO-2016 §H.6 p116 (DOD UCNI) and §H.6 p118
///   (DOE UCNI) is "May only be used with UNCLASSIFIED" — strictly the
///   US-classification system, not reciprocal-raised. A NATO-class
///   portion carrying UCNI is malformed input (UCNI is US AEA,
///   parallel to NATO ATOMAL); other rules catch the malformed shape.
fn class_floor_satisfied(attrs: &marque_ism::CanonicalAttrs, policy: ClassFloorPolicy) -> bool {
    match policy {
        ClassFloorPolicy::AtLeast(floor) => match attrs.classification.as_ref() {
            // Reciprocal-raise via `effective_level()`. NATO / FGI /
            // JOINT classifications return their US-equivalent level
            // for the comparison; US classifications return as-is.
            Some(c) => c.effective_level() >= floor,
            // No classification parsed at all → fail the floor.
            // Preserves retired-E022 / retired-E027 behavior on the
            // "classification is missing" case.
            None => false,
        },
        ClassFloorPolicy::EqualsU => match attrs.us_classification() {
            // Equals-U is the UCNI ceiling. `Some(Unclassified)` is the
            // only allowed state; everything else (including `None` for
            // pure-FGI / NATO / JOINT) fails. Mirrors retired E025
            // semantics: UCNI is US AEA and a non-US classification
            // carrying UCNI is malformed.
            Some(Classification::Unclassified) => true,
            _ => false,
        },
    }
}

// ---------------------------------------------------------------------------
// Family-presence predicates (one per catalog row)
// ---------------------------------------------------------------------------
//
// Each predicate iterates the relevant axis (`attrs.sci_markings`,
// `attrs.aea_markings`, `attrs.dissem_controls`, etc.) looking for any
// token matching the family pattern. Family granularity is the §3.4.6
// author's choice — the predicates pattern-match across all marking-
// template-level leaves that belong to the family.

/// HCS-[comp][sub] — any HCS-anchored marking carrying a compartment
/// that has at least one sub-compartment. Family covers HCS-P [SUB] and
/// any future HCS sub-compartmented variants.
fn presence_hcs_comp_sub(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
            && m.compartments
                .iter()
                .any(|c| !c.sub_compartments.is_empty())
    })
}

/// HCS-[comp] — any HCS-anchored marking carrying a compartment but no
/// sub-compartment (HCS-O, HCS-P bare). Family does NOT include HCS-X
/// (passthrough — see `presence_passthrough_hcs_x`) or bare HCS (legacy,
/// covered by E006/E008).
fn presence_hcs_comp_only(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
            && !m.compartments.is_empty()
            && m.compartments.iter().all(|c| c.sub_compartments.is_empty())
            // Exclude HCS-X: it's a passthrough family with its own row.
            && !m.compartments.iter().any(|c| c.identifier.as_ref() == "X")
    })
}

/// SI-[comp] — any SI-anchored marking carrying at least one
/// compartment. Family covers SI-G, SI-G [SUB], SI-ECRU, SI-NONBOOK, and
/// any agency SI compartment per CAPCO-2016 §H.4 p76 (TS-only).
fn presence_si_comp(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Si))
            && !m.compartments.is_empty()
    })
}

/// SI (bare) — any SI-anchored marking with NO compartment. Family is
/// the bare SI control system per §H.4 p74 (C-or-above floor).
fn presence_si_bare(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Si))
            && m.compartments.is_empty()
    })
}

/// TK-BLFH — any TK-anchored marking carrying a BLFH compartment (with
/// or without sub-compartments). §H.4 p87 / p89 — TS-only.
fn presence_tk_blfh(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Tk))
            && m.compartments
                .iter()
                .any(|c| c.identifier.as_ref() == "BLFH")
    })
}

/// TK family at the S floor — TK bare, TK-IDIT (with/without sub-comp),
/// TK-KAND (with/without sub-comp). Excludes TK-BLFH (covered by
/// `presence_tk_blfh` at TS-only). §H.4 p85 / p91 / p95.
fn presence_tk_family(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        if !matches!(m.system, SciControlSystem::Published(SciControlBare::Tk)) {
            return false;
        }
        // Exclude markings whose compartment set includes BLFH — those
        // are §2.1 row TK-BLFH (TS-only), not §2.2 row TK (S floor).
        let has_blfh = m
            .compartments
            .iter()
            .any(|c| c.identifier.as_ref() == "BLFH");
        !has_blfh
    })
}

/// RSV-[comp] — any RSV-anchored marking carrying a compartment.
/// CAPCO §H.4 p72.
fn presence_rsv_comp(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControlBare, SciControlSystem};
    attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Rsv))
            && !m.compartments.is_empty()
    })
}

/// RD bare — RD without CNWDI and without SIGMA. CAPCO §H.6 p104 floor C.
fn presence_rd_bare(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs.aea_markings.iter().any(|a| {
        matches!(
            a,
            marque_ism::AeaMarking::Rd(rd) if !rd.cnwdi && rd.sigma.is_empty()
        )
    })
}

/// RD-CNWDI — any RD block with `cnwdi == true`. Replaces retired E022.
/// CAPCO §H.6 p104 (TS-or-S RD); matches the catalog row's
/// authoritative §3.4.6 citation
/// (`E058/CNWDI-classification-floor` → `CAPCO-2016 §H.6 p104`).
fn presence_rd_cnwdi(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Rd(rd) if rd.cnwdi))
}

/// RD-SIGMA — any RD block carrying at least one SIGMA number.
/// CAPCO §H.6 p108 / p113 (RD-SIGMA TS-or-S).
fn presence_rd_sigma(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Rd(rd) if !rd.sigma.is_empty()))
}

/// FRD bare — FRD without SIGMA. CAPCO §H.6 p111 floor C.
fn presence_frd_bare(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs.aea_markings.iter().any(|a| {
        matches!(
            a,
            marque_ism::AeaMarking::Frd(frd) if frd.sigma.is_empty()
        )
    })
}

/// FRD-SIGMA — any FRD block carrying at least one SIGMA number.
/// CAPCO §H.6 p113 (FRD-SIGMA TS-or-S).
fn presence_frd_sigma(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Frd(frd) if !frd.sigma.is_empty()))
}

/// TFNI present. CAPCO §H.6 p120 floor C.
fn presence_tfni(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Tfni))
}

/// DOD UCNI present. Replaces half of retired E025.
fn presence_dod_ucni(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::DodUcni))
}

/// DOE UCNI present. Replaces half of retired E025.
fn presence_doe_ucni(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::DoeUcni))
}

/// SAR markings present. Replaces retired E027.
fn presence_sar(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs.sar_markings.is_some()
}

/// RSEN dissem control present. CAPCO §H.8 p132 (operative §H.8 p149
/// per §3.4.6 author). RSEN's CVE form is `RS`
/// (the portion-mark abbreviation; banner form is `RSEN`).
fn presence_rsen(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::DissemControl;
    attrs
        .dissem_controls
        .iter()
        .any(|d| matches!(d, DissemControl::Rs))
}

/// IMCON dissem control present.
fn presence_imcon(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::DissemControl;
    attrs
        .dissem_controls
        .iter()
        .any(|d| matches!(d, DissemControl::Imc))
}

/// ORCON family — ORCON or ORCON-USGOV. The §3.4.6 single family entry
/// covers both because §H.8 p136 (ORCON) and p139 (ORCON-USGOV) both
/// require classification ≥ C and the §3.4.6 author groups them.
fn presence_orcon_family(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::DissemControl;
    attrs
        .dissem_controls
        .iter()
        .any(|d| matches!(d, DissemControl::Oc | DissemControl::OcUsgov))
}

/// EYES ONLY portion mark / banner form. CAPCO §H.8 p157 (operative
/// §H.8 p152 per §3.4.6 author).
fn presence_eyes_only(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::DissemControl;
    attrs
        .dissem_controls
        .iter()
        .any(|d| matches!(d, DissemControl::Eyes))
}

/// BALK / BOHEMIA / ATOMAL — NATO Appendix B markings.
///
/// These appear via the NATO classification system: the `BALK`,
/// `BOHEMIA`, and `ATOMAL` floors fire when the *NATO sub-classification*
/// indicates the corresponding atom AND the page's US-equivalent
/// classification (per `NatoClassification::us_equivalent` and the
/// reciprocal-raise rule) is below the floor.
fn presence_balk(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{MarkingClassification, NatoClassification};
    matches!(
        &attrs.classification,
        Some(MarkingClassification::Nato(
            NatoClassification::CosmicTopSecretBalk
        ))
    )
}

fn presence_bohemia(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{MarkingClassification, NatoClassification};
    matches!(
        &attrs.classification,
        Some(MarkingClassification::Nato(
            NatoClassification::CosmicTopSecretBohemia
        ))
    )
}

fn presence_atomal(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{MarkingClassification, NatoClassification};
    matches!(
        &attrs.classification,
        Some(MarkingClassification::Nato(
            NatoClassification::NatoConfidentialAtomal
                | NatoClassification::NatoSecretAtomal
                | NatoClassification::CosmicTopSecretAtomal
        ))
    )
}

// ---------------------------------------------------------------------------
// Passthrough family predicates — §3.7 unknown-floor passthrough policy
// ---------------------------------------------------------------------------

/// BUR family — `BUR`, `BUR-BLG`, `BUR-DTP`, `BUR-WRG`. ISM-known SCI
/// control system; specific floor not enumerated in CAPCO-2016.
fn presence_passthrough_bur(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControl, SciControlBare, SciControlSystem};
    let has_via_markings = attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::Published(SciControlBare::Bur)));
    let has_via_controls = attrs.sci_controls.iter().any(|s| {
        matches!(
            s,
            SciControl::Bur | SciControl::BurBlg | SciControl::BurDtp | SciControl::BurWrg
        )
    });
    has_via_markings || has_via_controls
}

/// HCS-X — ISM-known HCS variant; specific floor not enumerated.
fn presence_passthrough_hcs_x(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControl, SciControlBare, SciControlSystem};
    let has_via_markings = attrs.sci_markings.iter().any(|m| {
        matches!(m.system, SciControlSystem::Published(SciControlBare::Hcs))
            && m.compartments.iter().any(|c| c.identifier.as_ref() == "X")
    });
    let has_via_controls = attrs
        .sci_controls
        .iter()
        .any(|s| matches!(s, SciControl::HcsX));
    has_via_markings || has_via_controls
}

/// KLM family — `KLM` / `KLAMATH`, `KLM-R`. ISM-known SCI control system.
fn presence_passthrough_klm(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControl, SciControlBare, SciControlSystem};
    let has_via_markings = attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::Published(SciControlBare::Klm)));
    let has_via_controls = attrs
        .sci_controls
        .iter()
        .any(|s| matches!(s, SciControl::Klm | SciControl::KlmR));
    has_via_markings || has_via_controls
}

/// MVL family — `MVL` / `MARVEL`. ISM-known SCI control system.
fn presence_passthrough_mvl(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::{SciControl, SciControlBare, SciControlSystem};
    let has_via_markings = attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::Published(SciControlBare::Mvl)));
    let has_via_controls = attrs
        .sci_controls
        .iter()
        .any(|s| matches!(s, SciControl::Mvl));
    has_via_markings || has_via_controls
}

// ---------------------------------------------------------------------------
// The catalog — 27 rows at §3.4.6 family granularity
// ---------------------------------------------------------------------------

const CLASS_FLOOR_CATALOG: &[ClassFloorRow] = &[
    // ---- §2.1 Floor TS (5 rows) ------------------------------------
    ClassFloorRow {
        name: "class-floor/HCS-comp-sub",
        marking_label: "HCS sub-compartment markings",
        presence: presence_hcs_comp_sub,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        axis: ClassFloorAxis::Sci,
    },
    ClassFloorRow {
        name: "class-floor/SI-comp",
        marking_label: "SI compartments",
        presence: presence_si_comp,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        axis: ClassFloorAxis::Sci,
    },
    ClassFloorRow {
        name: "class-floor/TK-BLFH",
        marking_label: "TK-BLFH (BLUEFISH)",
        presence: presence_tk_blfh,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        axis: ClassFloorAxis::Sci,
    },
    // BALK and BOHEMIA: floor TS via CTS reciprocal-raise per
    // marque-applied.md §3.4.1 Note (i). The presence predicate fires
    // only when the document's NATO classification is exactly
    // `CosmicTopSecretBalk` / `CosmicTopSecretBohemia`. CTS = TS in the
    // OrdMax chain, so an at-least-TS floor is satisfied by the
    // presence itself; the row exists for the case where a portion
    // labeled BALK/BOHEMIA is incorrectly carried with a sub-CTS
    // classification (data-corruption / mangled input).
    ClassFloorRow {
        name: "class-floor/BALK",
        marking_label: "BALK (NATO)",
        presence: presence_balk,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.7 Appendix B",
        passthrough: false,
        primary_kind: None,
        axis: ClassFloorAxis::NatoClass,
    },
    ClassFloorRow {
        name: "class-floor/BOHEMIA",
        marking_label: "BOHEMIA (NATO)",
        presence: presence_bohemia,
        policy: ClassFloorPolicy::AtLeast(Classification::TopSecret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.7 Appendix B",
        passthrough: false,
        primary_kind: None,
        axis: ClassFloorAxis::NatoClass,
    },
    // ---- §2.2 Floor S (8 rows) -------------------------------------
    ClassFloorRow {
        name: "class-floor/HCS-comp",
        marking_label: "HCS-O / HCS-P (compartment, no sub-compartment)",
        presence: presence_hcs_comp_only,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        axis: ClassFloorAxis::Sci,
    },
    ClassFloorRow {
        name: "class-floor/RSV-comp",
        marking_label: "RSV compartment",
        presence: presence_rsv_comp,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        axis: ClassFloorAxis::Sci,
    },
    ClassFloorRow {
        name: "class-floor/TK",
        marking_label: "TK / TK-IDIT / TK-KAND",
        presence: presence_tk_family,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        axis: ClassFloorAxis::Sci,
    },
    ClassFloorRow {
        name: "class-floor/RD-SG",
        marking_label: "RD-SIGMA",
        presence: presence_rd_sigma,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p113",
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        axis: ClassFloorAxis::Aea,
    },
    ClassFloorRow {
        name: "class-floor/FRD-SG",
        marking_label: "FRD-SIGMA",
        presence: presence_frd_sigma,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p113",
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        axis: ClassFloorAxis::Aea,
    },
    // CNWDI — replaces retired E022. Walker-prefixed name per PM
    // directive #5.
    ClassFloorRow {
        name: "E058/CNWDI-classification-floor",
        marking_label: "CNWDI",
        presence: presence_rd_cnwdi,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p104",
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        axis: ClassFloorAxis::Aea,
    },
    ClassFloorRow {
        name: "class-floor/RSEN",
        marking_label: "RSEN",
        presence: presence_rsen,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.8 p149",
        passthrough: false,
        primary_kind: Some(TokenKind::DissemControl),
        axis: ClassFloorAxis::Dissem,
    },
    ClassFloorRow {
        name: "class-floor/IMCON",
        marking_label: "IMCON",
        presence: presence_imcon,
        policy: ClassFloorPolicy::AtLeast(Classification::Secret),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.8 p144",
        passthrough: false,
        primary_kind: Some(TokenKind::DissemControl),
        axis: ClassFloorAxis::Dissem,
    },
    // ---- §2.3 Floor C (8 rows) -------------------------------------
    ClassFloorRow {
        name: "class-floor/SI",
        marking_label: "SI (bare)",
        presence: presence_si_bare,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.4",
        passthrough: false,
        primary_kind: Some(TokenKind::SciSystem),
        axis: ClassFloorAxis::Sci,
    },
    // SAR — replaces retired E027.
    ClassFloorRow {
        name: "E058/SAR-classification-floor",
        marking_label: "SAR",
        presence: presence_sar,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.5",
        passthrough: false,
        primary_kind: Some(TokenKind::SarIndicator),
        axis: ClassFloorAxis::Sar,
    },
    ClassFloorRow {
        name: "class-floor/RD",
        marking_label: "RD",
        presence: presence_rd_bare,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p104",
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        axis: ClassFloorAxis::Aea,
    },
    ClassFloorRow {
        name: "class-floor/FRD",
        marking_label: "FRD",
        presence: presence_frd_bare,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p104",
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        axis: ClassFloorAxis::Aea,
    },
    ClassFloorRow {
        name: "class-floor/TFNI",
        marking_label: "TFNI",
        presence: presence_tfni,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p107",
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        axis: ClassFloorAxis::Aea,
    },
    ClassFloorRow {
        name: "class-floor/ATOMAL",
        marking_label: "ATOMAL (NATO)",
        presence: presence_atomal,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.7 Appendix B",
        passthrough: false,
        primary_kind: None,
        axis: ClassFloorAxis::NatoClass,
    },
    ClassFloorRow {
        name: "class-floor/ORCON",
        marking_label: "ORCON / ORCON-USGOV",
        presence: presence_orcon_family,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.8 p136",
        passthrough: false,
        primary_kind: Some(TokenKind::DissemControl),
        axis: ClassFloorAxis::Dissem,
    },
    ClassFloorRow {
        name: "class-floor/EYES-ONLY",
        marking_label: "EYES ONLY",
        presence: presence_eyes_only,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.8 p152",
        passthrough: false,
        primary_kind: Some(TokenKind::DissemControl),
        axis: ClassFloorAxis::Dissem,
    },
    // ---- §2.4 Floor =U (2 rows; UCNI split per PM decision) ----------
    ClassFloorRow {
        name: "E058/DOD-UCNI-classification-ceiling",
        marking_label: "DOD UCNI",
        presence: presence_dod_ucni,
        policy: ClassFloorPolicy::EqualsU,
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p116",
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        axis: ClassFloorAxis::Aea,
    },
    ClassFloorRow {
        name: "E058/DOE-UCNI-classification-ceiling",
        marking_label: "DOE UCNI",
        presence: presence_doe_ucni,
        policy: ClassFloorPolicy::EqualsU,
        severity: marque_rules::Severity::Error,
        citation: "CAPCO-2016 §H.6 p118",
        passthrough: false,
        primary_kind: Some(TokenKind::AeaMarking),
        axis: ClassFloorAxis::Aea,
    },
    // ---- §2.6 Unknown-floor passthrough (4 rows; Warn) ---------------
    ClassFloorRow {
        name: "class-floor/passthrough-BUR",
        marking_label: "BUR family",
        presence: presence_passthrough_bur,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Warn,
        citation: "marque-applied.md §3.7 (passthrough); CAPCO-2016 unmapped",
        passthrough: true,
        primary_kind: Some(TokenKind::SciSystem),
        axis: ClassFloorAxis::Sci,
    },
    ClassFloorRow {
        name: "class-floor/passthrough-HCS-X",
        marking_label: "HCS-X",
        presence: presence_passthrough_hcs_x,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Warn,
        citation: "marque-applied.md §3.7 (passthrough); CAPCO-2016 unmapped",
        passthrough: true,
        primary_kind: Some(TokenKind::SciSystem),
        axis: ClassFloorAxis::Sci,
    },
    ClassFloorRow {
        name: "class-floor/passthrough-KLM",
        marking_label: "KLM family",
        presence: presence_passthrough_klm,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Warn,
        citation: "marque-applied.md §3.7 (passthrough); CAPCO-2016 unmapped",
        passthrough: true,
        primary_kind: Some(TokenKind::SciSystem),
        axis: ClassFloorAxis::Sci,
    },
    ClassFloorRow {
        name: "class-floor/passthrough-MVL",
        marking_label: "MVL",
        presence: presence_passthrough_mvl,
        policy: ClassFloorPolicy::AtLeast(Classification::Confidential),
        severity: marque_rules::Severity::Warn,
        citation: "marque-applied.md §3.7 (passthrough); CAPCO-2016 unmapped",
        passthrough: true,
        primary_kind: Some(TokenKind::SciSystem),
        axis: ClassFloorAxis::Sci,
    },
];

// ===========================================================================
// PR 3b.E (T026e) — SCI per-system catalog (§H.4)
// ===========================================================================
//
// `sci_per_system_catalog_eval` is the static-table dispatcher for the 5
// `Constraint::Custom` rows declared by `build_constraints` under the
// "PR 3b.E (T026e) — SCI per-system catalog (§H.4)" section header.
//
// Each row's predicate has a uniform shape: "if SCI marking M is present in
// `attrs`, the portion's IC dissem block must satisfy F(M)" where F(M) is
// either a companion-required check (NOFORN must appear) or a multi-branch
// check covering required-and-forbidden companions (ORCON required, ORCON-
// USGOV forbidden, etc.). The table stores one entry per row carrying:
//
//   - `name`: catalog row identifier (matches `Constraint::Custom { name }`,
//      and starts with the `sci-per-system/` prefix)
//   - `marking_label`: human-readable marking name for the diagnostic
//   - `presence`: predicate `fn(&CanonicalAttrs) -> bool` checking whether
//      the family pattern is present
//   - `kind`: dispatch tag — `CompanionRequired` (single dissem-control
//      insertion) or `Custom` (closure for multi-branch emit logic)
//   - `severity`: per-row default `Severity` (typically `Warn`; the emit
//      helper escalates per-branch to `Error` no-fix when no IC dissem
//      block exists)
//   - `citation`: per-row §-citation matching `Constraint::Custom { label }`
//
// Diagnostic-span anchoring is NOT a row field — companion-insertion
// branches anchor the diagnostic at the offending SCI marking token via
// `first_sci_span(attrs)`, while token-replacement branches (e.g., the
// OC-USGOV → OC fix in row #1 / #3 / #4) anchor both the diagnostic and
// the fix at the dissem token's own span so the user sees the offending
// dissem token directly. See the per-emit-fn doc comments for the
// branch-specific anchor.
//
// The walker `DeclarativeSciPerSystemRule` (in `rules_declarative.rs`)
// iterates the table and emits per-row diagnostics.
//
// FORWARD LINK to PR 4 (per-category Lattice impls): once `marque-scheme`
// exposes `Constraint::CompanionRequired<Set>` / `Forbid<Set>` primitives
// (or the equivalent ImplTable / closure-operator machinery from
// `marque-applied.md` §3.4.6), these rows can re-classify from
// `Constraint::Custom` to a primitive form without changing per-row
// semantics. See `docs/plans/2026-05-08-pr3b-E-sci-per-system-collapse-plan.md`
// §1 for the architectural rationale.

/// Companion form (abbreviated vs full) inferred from the dissem-token
/// text observed on a portion. Used to keep the inserted token's surface
/// form consistent with the existing block (so `(S//HCS-O//OC)` inserts
/// `/NF`, not `/NOFORN`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum CompanionForm {
    /// Short form: `OC`, `NF`, `OC-USGOV`. Used when the first observed
    /// dissem token on the portion is a portion/abbrev surface form.
    Abbreviated,
    /// Full form: `ORCON`, `NOFORN`. Used otherwise (banner long-form or
    /// no dissem block yet).
    Full,
}

impl CompanionForm {
    pub(crate) fn orcon(self) -> &'static str {
        match self {
            Self::Abbreviated => "OC",
            Self::Full => "ORCON",
        }
    }

    pub(crate) fn noforn(self) -> &'static str {
        match self {
            Self::Abbreviated => "NF",
            Self::Full => "NOFORN",
        }
    }
}

/// Walker rule ID shared by every SCI per-system catalog emit body.
/// `RuleId::new` is `const fn`, so this is a zero-cost replacement for
/// the four prior inline `RuleId::new("E059")` call sites (one per
/// row-emit helper). Hoisting also makes a future rule-ID change a
/// single edit.
const RULE_E059: marque_rules::RuleId = marque_rules::RuleId::new("E059");

/// Dispatch tag for an SCI per-system catalog row's emit body. Two
/// variants keep the `match row.kind` arm count under the ≤3-branch
/// reviewer-attestation cap (§7(b) of the PR 3b.E plan).
#[derive(Copy, Clone)]
pub(crate) enum SciPerSystemKind {
    /// Single dissem-control insertion. The row encodes "if marking M is
    /// present, dissem control D must appear; if absent, emit a
    /// zero-width insertion fix at the end of the IC dissem block." The
    /// only PR-E rows using this kind are the NOFORN-only rows (#2 and
    /// #5).
    CompanionRequired {
        /// The dissem control whose presence is required.
        dissem: marque_ism::DissemControl,
        /// Component for the diagnostic message (e.g., "NOFORN").
        token_name: &'static str,
    },
    /// Custom multi-branch emit. The row encodes a closure that produces
    /// the full emit list, used by rows whose emit logic spans 2-3 distinct
    /// branches with row-specific text and span logic (rows #1, #3, #4).
    Custom(fn(&marque_ism::CanonicalAttrs, &SciPerSystemRow) -> Vec<marque_rules::Diagnostic>),
}

/// One catalog row. The walker dispatches over `&[SciPerSystemRow]`;
/// each row owns its presence predicate, dispatch kind, severity,
/// citation, and human-readable marking label.
///
/// # Naming-prefix invariant
///
/// Every row's `name` MUST start with `sci-per-system/`. The
/// `sci_per_system_catalog_naming_convention` test in
/// `crates/capco/tests/sci_per_system_catalog.rs` enforces this at build
/// time so adding a row that doesn't follow the convention fails CI.
/// The prefix is what makes [`is_sci_per_system_catalog_name`] dispatch
/// O(1) instead of a linear catalog scan.
#[derive(Copy, Clone)]
pub(crate) struct SciPerSystemRow {
    /// Catalog row name — matches the `Constraint::Custom { name }` of
    /// the same logical row. MUST start with `sci-per-system/`.
    pub(crate) name: &'static str,
    /// Human-readable marking name for the diagnostic message
    /// (e.g., `"HCS-O"`, `"TK-{BLFH|IDIT|KAND}"`).
    pub(crate) marking_label: &'static str,
    /// Marking-presence predicate.
    pub(crate) presence: fn(&marque_ism::CanonicalAttrs) -> bool,
    /// Dispatch kind — `CompanionRequired` (single-token) or `Custom`
    /// (multi-branch closure).
    pub(crate) kind: SciPerSystemKind,
    /// Default severity (typically `Warn`). The emit helper escalates
    /// per-branch to `Error` no-fix when no IC dissem block exists.
    pub(crate) severity: marque_rules::Severity,
    /// Per-row §-citation, matching `Constraint::Custom { label }`.
    pub(crate) citation: &'static str,
}

// ---------------------------------------------------------------------------
// SCI per-system helpers — moved verbatim from rules_sci_per_system.rs
// (helper-relocation Option A per planning doc §4.1)
// ---------------------------------------------------------------------------

/// Is this `SciMarking` anchored on the given published bare system?
pub(crate) fn anchors_on(m: &marque_ism::SciMarking, system: marque_ism::SciControlBare) -> bool {
    use marque_ism::SciControlSystem;
    matches!(&m.system, SciControlSystem::Published(s) if *s == system)
}

/// Does any compartment under this marking carry the given identifier?
pub(crate) fn has_compartment(m: &marque_ism::SciMarking, id: &str) -> bool {
    m.compartments.iter().any(|c| c.identifier.as_ref() == id)
}

/// Does the specific compartment carry at least one sub-compartment?
pub(crate) fn compartment_has_sub(m: &marque_ism::SciMarking, comp_id: &str) -> bool {
    m.compartments
        .iter()
        .any(|c| c.identifier.as_ref() == comp_id && !c.sub_compartments.is_empty())
}

/// Is this a TK-BLFH, TK-IDIT, or TK-KAND marking (the three TK
/// compartments that require NOFORN per §H.4 p87 / p91 / p95)?
pub(crate) fn is_tk_noforn_compartment(m: &marque_ism::SciMarking) -> bool {
    use marque_ism::SciControlBare;
    anchors_on(m, SciControlBare::Tk)
        && m.compartments
            .iter()
            .any(|c| matches!(c.identifier.as_ref(), "BLFH" | "IDIT" | "KAND"))
}

/// Find the first SCI-system/SCI-control token span in document order.
/// Used as the diagnostic anchor when the rule fires on a portion's SCI
/// block.
pub(crate) fn first_sci_span(attrs: &marque_ism::CanonicalAttrs) -> Option<marque_ism::Span> {
    attrs
        .token_spans
        .iter()
        .find(|t| {
            matches!(
                t.kind,
                TokenKind::SciSystem
                    | TokenKind::SciControl
                    | TokenKind::SciCompartment
                    | TokenKind::SciSubCompartment
            )
        })
        .map(|t| t.span)
}

/// Observed US classification level, if any. Returns `None` for pure
/// foreign classifications (FGI/NATO/JOINT) — SCI-on-foreign is out of
/// §H.4's scope and handled by the foreign-classification rule cluster.
pub(crate) fn us_level(attrs: &marque_ism::CanonicalAttrs) -> Option<Classification> {
    use marque_ism::MarkingClassification;
    match attrs.classification {
        Some(MarkingClassification::Us(c)) => Some(c),
        Some(MarkingClassification::Conflict { us, .. }) => Some(us),
        _ => None,
    }
}

/// Last token span of the IC dissem block (anchors zero-width insertions).
/// Returns `None` when no IC dissem token exists.
pub(crate) fn last_dissem_span(attrs: &marque_ism::CanonicalAttrs) -> Option<marque_ism::Span> {
    attrs
        .token_spans
        .iter()
        .rev()
        .find(|t| t.kind == TokenKind::DissemControl)
        .map(|t| t.span)
}

/// Find the span (and current text) of a specific `DissemControl` token —
/// used when a rule needs to replace e.g. `OC-USGOV` with `OC`.
pub(crate) fn dissem_token_span(
    attrs: &marque_ism::CanonicalAttrs,
    target: marque_ism::DissemControl,
) -> Option<(marque_ism::Span, &str)> {
    for (dissem_idx, d) in attrs.dissem_controls.iter().enumerate() {
        if *d == target {
            // Walk token_spans to find the Nth DissemControl.
            let tok = attrs
                .token_spans
                .iter()
                .filter(|t| t.kind == TokenKind::DissemControl)
                .nth(dissem_idx)?;
            return Some((tok.span, tok.text.as_ref()));
        }
    }
    None
}

/// Banner-form vs portion-form companion representation, given the
/// current dissem block. The parser preserves user-written text verbatim
/// in `TokenSpan::text`, so inserting in matching form avoids surprise
/// mixed-form output.
pub(crate) fn infer_companion_form(attrs: &marque_ism::CanonicalAttrs) -> CompanionForm {
    let first = attrs
        .token_spans
        .iter()
        .find(|t| t.kind == TokenKind::DissemControl);
    match first.map(|t| t.text.as_ref()) {
        Some("NF") | Some("OC") | Some("OC-USGOV") => CompanionForm::Abbreviated,
        _ => CompanionForm::Full,
    }
}

/// Build a diagnostic that points at `anchor_span` (the offending SCI
/// token) with a zero-width insertion fix appending `/<token>` at the
/// end of the existing IC dissem block. Diagnostic span and fix span
/// intentionally differ: the user sees the SCI marking that triggered
/// the requirement; the edit applies at the dissem block where the
/// insertion belongs. Same diagnostic-vs-fix-span split used by
/// `SarPortionFormRule` (E026).
///
/// Falls back to `Severity::Error` no-fix when no dissem block exists
/// — inserting a whole `//`-separated category block from rule context
/// is unsafe (no anchor for the `//`). Same policy as E040.
pub(crate) fn emit_companion_insert(
    rule: marque_rules::RuleId,
    severity: marque_rules::Severity,
    anchor_span: marque_ism::Span,
    last_dissem: Option<marque_ism::Span>,
    token: &str,
    message: String,
    citation: &'static str,
) -> marque_rules::Diagnostic {
    use marque_ism::Span;
    use marque_rules::{Confidence, Diagnostic, FixProposal, FixSource, Severity};
    match last_dissem {
        Some(dissem_span) => {
            let insert_at = dissem_span.end;
            let fix = FixProposal::new(
                rule.clone(),
                FixSource::BuiltinRule,
                Span::new(insert_at, insert_at),
                String::new(),
                format!("/{token}"),
                Confidence::strict(0.9),
                None,
            );
            Diagnostic::new(rule, severity, anchor_span, message, citation, Some(fix))
        }
        None => {
            // No dissem block — escalate to Error with no fix.
            Diagnostic::new(rule, Severity::Error, anchor_span, message, citation, None)
        }
    }
}

// ---------------------------------------------------------------------------
// Family-presence predicates (one per PR-E catalog row)
// ---------------------------------------------------------------------------

/// HCS-O — any HCS-anchored marking carrying the "O" compartment.
/// §H.4 p64.
#[inline]
fn presence_hcs_o(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::SciControlBare;
    attrs
        .sci_markings
        .iter()
        .any(|m| anchors_on(m, SciControlBare::Hcs) && has_compartment(m, "O"))
}

/// HCS-P (any) — any HCS-anchored marking carrying the "P" compartment,
/// with or without sub-compartments. §H.4 p66 (and p68 inheriting NOFORN).
#[inline]
fn presence_hcs_p_any(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::SciControlBare;
    attrs
        .sci_markings
        .iter()
        .any(|m| anchors_on(m, SciControlBare::Hcs) && has_compartment(m, "P"))
}

/// HCS-P [SUB] — any HCS-anchored marking carrying a "P" compartment
/// with at least one sub-compartment. §H.4 p68. By §H.4 grammar, P is
/// the only HCS compartment that can carry sub-compartments, so this
/// coincides with `presence_hcs_comp_sub` from the class-floor catalog
/// in practice; we keep a separate predicate here to make the row
/// surface-explicit ("requires ORCON / forbids ORCON-USGOV on
/// sub-compartmented HCS-P").
#[inline]
fn presence_hcs_p_sub(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::SciControlBare;
    attrs
        .sci_markings
        .iter()
        .any(|m| anchors_on(m, SciControlBare::Hcs) && compartment_has_sub(m, "P"))
}

/// SI-G — any SI-anchored marking carrying the "G" compartment, with or
/// without sub-compartments. §H.4 p80 (and p81 inheriting ORCON).
#[inline]
fn presence_si_g(attrs: &marque_ism::CanonicalAttrs) -> bool {
    use marque_ism::SciControlBare;
    attrs
        .sci_markings
        .iter()
        .any(|m| anchors_on(m, SciControlBare::Si) && has_compartment(m, "G"))
}

/// TK with BLFH/IDIT/KAND compartment — any TK-anchored marking carrying
/// at least one of the three NOFORN-required compartments. §H.4 p87 +
/// p91 + p95.
#[inline]
fn presence_tk_compartment_noforn(attrs: &marque_ism::CanonicalAttrs) -> bool {
    attrs.sci_markings.iter().any(is_tk_noforn_compartment)
}

// ---------------------------------------------------------------------------
// Per-row Custom-kind emit closures (rows #1, #3, #4)
// ---------------------------------------------------------------------------

/// Row #1 — HCS-O companions: requires ORCON + NOFORN, forbids
/// ORCON-USGOV. §H.4 p64.
fn emit_hcs_o_companions(
    attrs: &marque_ism::CanonicalAttrs,
    row: &SciPerSystemRow,
) -> Vec<marque_rules::Diagnostic> {
    use crate::rules::{FixDiagnosticParams, make_fix_diagnostic};
    use marque_ism::{DissemControl, Span};
    use marque_rules::FixSource;

    if us_level(attrs).is_none() {
        return Vec::new();
    }
    let has_orcon = attrs.dissem_controls.contains(&DissemControl::Oc)
        || attrs.dissem_controls.contains(&DissemControl::OcUsgov);
    let has_noforn = attrs.dissem_controls.contains(&DissemControl::Nf);
    let usgov_entry = dissem_token_span(attrs, DissemControl::OcUsgov);

    let mut out = Vec::new();
    let form = infer_companion_form(attrs);
    let last_dissem = last_dissem_span(attrs);
    let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));

    if !has_orcon {
        out.push(emit_companion_insert(
            RULE_E059,
            row.severity,
            sci_span,
            last_dissem,
            form.orcon(),
            "HCS-O requires ORCON (§H.4 p64)".to_owned(),
            row.citation,
        ));
    }
    if !has_noforn {
        out.push(emit_companion_insert(
            RULE_E059,
            row.severity,
            sci_span,
            last_dissem,
            form.noforn(),
            "HCS-O requires NOFORN (§H.4 p64)".to_owned(),
            row.citation,
        ));
    }
    if let Some((span, text)) = usgov_entry {
        out.push(make_fix_diagnostic(FixDiagnosticParams {
            rule: RULE_E059,
            severity: row.severity,
            source: FixSource::BuiltinRule,
            span,
            message: "HCS-O forbids ORCON-USGOV (§H.4 p64) — replace with ORCON".to_owned(),
            citation: row.citation,
            original: text.to_owned(),
            replacement: form.orcon().to_owned(),
            confidence: 0.9,
            migration_ref: None,
        }));
    }
    out
}

/// Row #3 — HCS-P sub-compartment companions: requires ORCON, forbids
/// ORCON-USGOV. §H.4 p68. NOFORN is enforced by row #2 (HCS-P NOFORN)
/// which fires on any HCS-P including sub-compartmented variants, so
/// it is not duplicated here.
fn emit_hcs_p_sub_companions(
    attrs: &marque_ism::CanonicalAttrs,
    row: &SciPerSystemRow,
) -> Vec<marque_rules::Diagnostic> {
    use crate::rules::{FixDiagnosticParams, make_fix_diagnostic};
    use marque_ism::{DissemControl, Span};
    use marque_rules::FixSource;

    if us_level(attrs).is_none() {
        return Vec::new();
    }
    let has_orcon = attrs.dissem_controls.contains(&DissemControl::Oc)
        || attrs.dissem_controls.contains(&DissemControl::OcUsgov);
    let usgov_entry = dissem_token_span(attrs, DissemControl::OcUsgov);

    let mut out = Vec::new();
    let form = infer_companion_form(attrs);
    let last_dissem = last_dissem_span(attrs);
    let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));

    if !has_orcon {
        out.push(emit_companion_insert(
            RULE_E059,
            row.severity,
            sci_span,
            last_dissem,
            form.orcon(),
            "HCS-P sub-compartment requires ORCON (§H.4 p68)".to_owned(),
            row.citation,
        ));
    }
    if let Some((span, text)) = usgov_entry {
        out.push(make_fix_diagnostic(FixDiagnosticParams {
            rule: RULE_E059,
            severity: row.severity,
            source: FixSource::BuiltinRule,
            span,
            message: "HCS-P sub-compartment forbids ORCON-USGOV (§H.4 p68) — replace with ORCON"
                .to_owned(),
            citation: row.citation,
            original: text.to_owned(),
            replacement: form.orcon().to_owned(),
            confidence: 0.9,
            migration_ref: None,
        }));
    }
    out
}

/// Row #4 — SI-G companions: requires ORCON, forbids ORCON-USGOV.
/// §H.4 p80.
fn emit_si_g_companions(
    attrs: &marque_ism::CanonicalAttrs,
    row: &SciPerSystemRow,
) -> Vec<marque_rules::Diagnostic> {
    use crate::rules::{FixDiagnosticParams, make_fix_diagnostic};
    use marque_ism::{DissemControl, Span};
    use marque_rules::FixSource;

    if us_level(attrs).is_none() {
        return Vec::new();
    }
    let has_orcon = attrs.dissem_controls.contains(&DissemControl::Oc)
        || attrs.dissem_controls.contains(&DissemControl::OcUsgov);
    let usgov_entry = dissem_token_span(attrs, DissemControl::OcUsgov);

    let mut out = Vec::new();
    let form = infer_companion_form(attrs);
    let last_dissem = last_dissem_span(attrs);
    let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));

    if !has_orcon {
        out.push(emit_companion_insert(
            RULE_E059,
            row.severity,
            sci_span,
            last_dissem,
            form.orcon(),
            "SI-G requires ORCON (§H.4 p80)".to_owned(),
            row.citation,
        ));
    }
    if let Some((span, text)) = usgov_entry {
        out.push(make_fix_diagnostic(FixDiagnosticParams {
            rule: RULE_E059,
            severity: row.severity,
            source: FixSource::BuiltinRule,
            span,
            message: "SI-G forbids ORCON-USGOV (§H.4 p80) — replace with ORCON".to_owned(),
            citation: row.citation,
            original: text.to_owned(),
            replacement: form.orcon().to_owned(),
            confidence: 0.9,
            migration_ref: None,
        }));
    }
    out
}

// ---------------------------------------------------------------------------
// CompanionRequired single-token emit (rows #2, #5)
// ---------------------------------------------------------------------------

/// Single-token companion insertion. Used by `CompanionRequired`-kind
/// rows whose only check is "dissem control X must appear; if missing,
/// emit a zero-width-insertion fix at the end of the IC dissem block."
///
/// # Message format
///
/// Diagnostic message is uniformly `"{marking_label} requires
/// {token_name} ({citation})"`, derived entirely from row metadata
/// (`SciPerSystemRow::marking_label`, the caller-provided `token_name`,
/// and `SciPerSystemRow::citation`). This keeps the catalog as the
/// single source of truth for both message-text and citation: a 6th
/// `CompanionRequired` row added in the future inherits the same
/// shape automatically without a per-row branch. The legacy E043 /
/// E051 messages used a slightly different shape (bare `§H.4 p66`,
/// `§H.4 p87, p91, p95` instead of the full `CAPCO-2016 §H.4 …`
/// citation); pre-users (per project policy) means no fixture-stability
/// constraint, so the format is unified rather than carrying a
/// per-row exception table.
fn emit_companion_required(
    attrs: &marque_ism::CanonicalAttrs,
    row: &SciPerSystemRow,
    dissem: marque_ism::DissemControl,
    token_name: &'static str,
) -> Vec<marque_rules::Diagnostic> {
    use marque_ism::Span;

    if us_level(attrs).is_none() {
        return Vec::new();
    }
    if attrs.dissem_controls.contains(&dissem) {
        return Vec::new();
    }
    // ORCON-USGOV satisfies ORCON-presence checks (the OC-USGOV → OC
    // replacement covers the post-fix state). For PR-E rows #2 and #5
    // (NOFORN-only), this branch never trips because the dissem
    // control is `Nf`, not `Oc`. Guard kept for symmetry with the
    // multi-branch helpers; the explicit `dissem == Oc` check is what
    // makes the guard apply only when relevant.
    if dissem == marque_ism::DissemControl::Oc
        && attrs
            .dissem_controls
            .contains(&marque_ism::DissemControl::OcUsgov)
    {
        return Vec::new();
    }

    let form = infer_companion_form(attrs);
    let last_dissem = last_dissem_span(attrs);
    let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));

    let companion_text = match dissem {
        marque_ism::DissemControl::Nf => form.noforn(),
        marque_ism::DissemControl::Oc => form.orcon(),
        // PR-E rows do not currently use other dissem controls; fall
        // back to the abbreviated CVE form for symmetry.
        _ => dissem.as_str(),
    };

    let message = format!(
        "{label} requires {token_name} ({citation})",
        label = row.marking_label,
        citation = row.citation,
    );

    vec![emit_companion_insert(
        RULE_E059,
        row.severity,
        sci_span,
        last_dissem,
        companion_text,
        message,
        row.citation,
    )]
}

// ---------------------------------------------------------------------------
// Catalog dispatch
// ---------------------------------------------------------------------------

/// Returns true if `name` is a catalog row name dispatched by
/// [`sci_per_system_catalog_eval`]. Used by `evaluate_custom_by_attrs`
/// to route on the table.
///
/// O(1) prefix check — every catalog row's `name` MUST start with
/// `sci-per-system/`. The `sci_per_system_catalog_naming_convention`
/// test in `crates/capco/tests/sci_per_system_catalog.rs` enforces the
/// invariant at build time.
fn is_sci_per_system_catalog_name(name: &str) -> bool {
    name.starts_with("sci-per-system/")
}

/// Resolve a catalog row by `name`. Returns `None` for unknown names.
///
/// Walked only on the trait/validate path (5-row catalog → linear scan,
/// ≪1 µs). The walker hot path uses [`sci_per_system_catalog`] then
/// [`sci_per_system_emit`] directly with no name lookup.
pub(crate) fn sci_per_system_row_by_name(name: &str) -> Option<&'static SciPerSystemRow> {
    SCI_PER_SYSTEM_CATALOG.iter().find(|row| row.name == name)
}

/// Iterate the full SCI per-system catalog. Used by the walker
/// `DeclarativeSciPerSystemRule::check` to dispatch over every row.
pub(crate) fn sci_per_system_catalog() -> &'static [SciPerSystemRow] {
    SCI_PER_SYSTEM_CATALOG
}

/// Test-only accessor returning every catalog row's `name` field
/// directly from the static catalog table. Used by the catalog
/// naming-convention test in
/// `crates/capco/tests/sci_per_system_catalog.rs` to assert the
/// `sci-per-system/` prefix invariant on every row WITHOUT a
/// `contains("sci-per-system")` filter that would silently skip a
/// typo'd-prefix row (e.g., `sai-per-system/...`). The accessor walks
/// `SCI_PER_SYSTEM_CATALOG` so a typo at the row's authoring site is
/// caught at CI time.
#[doc(hidden)]
pub fn sci_per_system_catalog_row_names() -> Vec<&'static str> {
    SCI_PER_SYSTEM_CATALOG.iter().map(|r| r.name).collect()
}

/// Single source of truth for the SCI per-system catalog's emit logic.
/// Both the walker hot path (`DeclarativeSciPerSystemRule::check` calls
/// this directly per row) and the trait/validate path
/// ([`sci_per_system_catalog_eval`]) converge through here, so a
/// citation, message-text, fix-shape, or severity-escalation change to
/// one row cannot silently diverge between the two emitters.
///
/// `#[inline]` because the walker's hot path is the bench-gate-relevant
/// one and the emit dispatch is a 2-arm match on a `Copy` enum field —
/// inlining lets the compiler hoist the row's presence predicate +
/// kind dispatch into the catalog-walk loop.
///
/// Returns an empty `Vec` when the row's presence predicate doesn't fire
/// or when no diagnostic is warranted; otherwise returns one or more
/// `Diagnostic` values per the row's emit logic.
#[inline]
pub(crate) fn sci_per_system_emit(
    attrs: &marque_ism::CanonicalAttrs,
    row: &SciPerSystemRow,
) -> Vec<marque_rules::Diagnostic> {
    if !(row.presence)(attrs) {
        return Vec::new();
    }
    match row.kind {
        SciPerSystemKind::CompanionRequired { dissem, token_name } => {
            emit_companion_required(attrs, row, dissem, token_name)
        }
        SciPerSystemKind::Custom(emit_fn) => emit_fn(attrs, row),
    }
}

/// Dispatch a single catalog row by name and return any
/// `ConstraintViolation`s. Trait-path entry point used by
/// [`MarkingScheme::validate`] →
/// [`marque_scheme::constraint::evaluate`] when the catalog row's
/// `Constraint::Custom` arm fires.
///
/// Note: PR-E rows produce `FixProposal` values on the walker path,
/// but `ConstraintViolation` doesn't carry a fix — the trait/validate
/// path drops the fix (this is the same divergence PR D's class-floor
/// catalog has). The engine path is the only path that produces
/// `AppliedFix` records, and the engine path always uses the walker.
fn sci_per_system_catalog_eval(
    attrs: &marque_ism::CanonicalAttrs,
    name: &'static str,
) -> Vec<ConstraintViolation> {
    let Some(row) = sci_per_system_row_by_name(name) else {
        return Vec::new();
    };
    sci_per_system_emit(attrs, row)
        .into_iter()
        .map(|d| ConstraintViolation {
            constraint_label: row.name,
            message: String::from(d.message),
            citation: row.citation,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The catalog — 5 rows at §H.4 family granularity
// ---------------------------------------------------------------------------

const SCI_PER_SYSTEM_CATALOG: &[SciPerSystemRow] = &[
    // Row #1 — HCS-O companions (ORCON + NOFORN required, ORCON-USGOV
    // forbidden). §H.4 p64.
    SciPerSystemRow {
        name: "sci-per-system/HCS-O-companions",
        marking_label: "HCS-O",
        presence: presence_hcs_o,
        kind: SciPerSystemKind::Custom(emit_hcs_o_companions),
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §H.4 p64",
    },
    // Row #2 — HCS-P NOFORN (NOFORN required). §H.4 p66.
    SciPerSystemRow {
        name: "sci-per-system/HCS-P-NOFORN",
        marking_label: "HCS-P",
        presence: presence_hcs_p_any,
        kind: SciPerSystemKind::CompanionRequired {
            dissem: marque_ism::DissemControl::Nf,
            token_name: "NOFORN",
        },
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §H.4 p66",
    },
    // Row #3 — HCS-P sub-compartment companions (ORCON required,
    // ORCON-USGOV forbidden). §H.4 p68. NOFORN is covered by row #2.
    SciPerSystemRow {
        name: "sci-per-system/HCS-P-sub-companions",
        marking_label: "HCS-P sub-compartment",
        presence: presence_hcs_p_sub,
        kind: SciPerSystemKind::Custom(emit_hcs_p_sub_companions),
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §H.4 p68",
    },
    // Row #4 — SI-G companions (ORCON required, ORCON-USGOV forbidden).
    // §H.4 p80.
    SciPerSystemRow {
        name: "sci-per-system/SI-G-companions",
        marking_label: "SI-G",
        presence: presence_si_g,
        kind: SciPerSystemKind::Custom(emit_si_g_companions),
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §H.4 p80",
    },
    // Row #5 — TK compartment NOFORN (BLFH/IDIT/KAND require NOFORN).
    // §H.4 p87 (TK-BLFH) + p91 (TK-IDIT) + p95 (TK-KAND).
    SciPerSystemRow {
        name: "sci-per-system/TK-compartment-NOFORN",
        marking_label: "TK-{BLFH|IDIT|KAND}",
        presence: presence_tk_compartment_noforn,
        kind: SciPerSystemKind::CompanionRequired {
            dissem: marque_ism::DissemControl::Nf,
            token_name: "NOFORN",
        },
        severity: marque_rules::Severity::Warn,
        citation: "CAPCO-2016 §H.4 p87 + p91 + p95",
    },
];

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
