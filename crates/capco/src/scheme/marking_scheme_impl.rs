// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `impl MarkingScheme for CapcoScheme` — the 22-method trait body.
//!
//! Hosts the entire `impl MarkingScheme for CapcoScheme` block lifted
//! from `scheme/mod.rs` per the Stage 2 PR B hub-split (issue #466).
//! Method bodies are byte-identical to the pre-split source — imports
//! adjusted to reach helpers via `super::actions::*` /
//! `super::predicates::*` (the same glob pattern `mod.rs` used pre-
//! split) plus explicit named imports of scheme-internal symbols
//! (`RENDER_TABLE`, `DissemFamilyMembership`, `CAPCO_CLOSURE_RULES`,
//! the CAT_*/TOK_* constants, the `CapcoMarking` / `CapcoScheme` /
//! `CapcoOpenVocabRef` / `CapcoParseError` types) that travel via
//! the parent module's re-exports.

use marque_ism::CanonicalAttrs;
use marque_scheme::{
    ApplyIntentError, Category, CategoryAction, CategoryId, CategoryPredicate, Constraint,
    ConstraintViolation, FactRef, MarkingScheme, PageRewrite, Parsed, ReplacementIntent, Scope,
    Template, TokenId, TokenRef,
};

use super::actions::*;
use super::closure::CAPCO_CLOSURE_RULES;
use super::predicates::*;
use super::*;

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
    type OpenVocabRef = CapcoOpenVocabRef;

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
    ///   intentionally counts both the dissem axis (`dissem_us` and
    ///   `dissem_nato` together, walked via `attrs.dissem_iter()`
    ///   post PR 9b / FR-046 split) AND `rel_to` as dissem-flavored
    ///   presence, matching the historical E015
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

    /// Map a [`FactRef`] to its [`CategoryId`].
    ///
    /// Closed-CVE sentinels in the current constraint catalog get
    /// explicit mappings; open-vocab references route by variant.
    /// Tokens not in the table return `None`, signaling
    /// [`ApplyIntentError::UnknownToken`] when the engine asks
    /// `apply_intent` to route them.
    fn category_of(&self, token: &FactRef<Self>) -> Option<CategoryId> {
        match token {
            FactRef::Cve(id) => capco_token_category(*id),
            FactRef::OpenVocab(r) => Some(match r {
                CapcoOpenVocabRef::Sar(_) => CAT_SAR,
                CapcoOpenVocabRef::SciCompartment(_) | CapcoOpenVocabRef::SciSubCompartment(_) => {
                    CAT_SCI
                }
                CapcoOpenVocabRef::FgiTetragraph(_) => CAT_FGI_MARKER,
                // PR 3c.B Sub-PR 8.D.4 — open-vocab REL TO country codes
                // route to CAT_REL_TO so E014's `FactAdd { CountryCode,
                // Portion }` intents land on the same axis as the
                // closed-CVE `TOK_USA` / `TOK_REL_TO` sentinels used by
                // FactRemove paths in PR 3c.B Sub-PR 8.D.2.
                CapcoOpenVocabRef::CountryCode(_) => CAT_REL_TO,
            }),
        }
    }

    /// Apply a batch of [`ReplacementIntent`]s to a [`CapcoMarking`].
    ///
    /// Clones the input marking, dispatches each intent through the
    /// per-axis category mutators ([`capco_category_clear`] /
    /// [`capco_category_replace`]) for `FactRemove` and an analogous
    /// closed-vocab add path for `FactAdd`. `Recanonicalize` returns
    /// the cloned marking unchanged — the engine renders it via
    /// [`MarkingScheme::render_canonical`] to produce canonical form.
    ///
    /// # Idempotence
    ///
    /// **Per-intent vs batch-level `IntentInapplicable`**: the trait
    /// invariants for `apply_intent` require idempotence and
    /// commutativity *within a batch*. A redundant or already-satisfied
    /// intent (e.g., a second `FactRemove` of the same token, or a
    /// `FactRemove` of a token a prior intent in the same batch
    /// already removed) MUST be treated as a per-intent no-op — it
    /// MUST NOT abort the rest of the batch. Only when EVERY intent
    /// in the batch is inapplicable does this method return
    /// `Err(IntentInapplicable)`, signaling to the engine that the
    /// whole fix is a no-op and should be dropped.
    ///
    /// Other error variants (`UnknownToken`, `IntentRejectsLattice`)
    /// propagate immediately — they're not idempotency cases.
    fn apply_intent(
        &self,
        marking: &Self::Marking,
        intents: &[ReplacementIntent<Self>],
    ) -> Result<Self::Marking, ApplyIntentError> {
        let mut out = marking.clone();
        let mut any_applied = false;
        for intent in intents {
            match apply_intent_to_marking(self, &mut out, intent) {
                Ok(()) => {
                    any_applied = true;
                }
                Err(ApplyIntentError::IntentInapplicable) => {
                    // Per-intent no-op: redundant intent (e.g., a
                    // prior intent in the same batch already produced
                    // the desired state, or two rules emitted the
                    // same FactRemove). Idempotence/commutativity
                    // invariant requires the batch to continue.
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        if any_applied {
            Ok(out)
        } else {
            // Whole-batch no-op: engine drops the fix silently.
            Err(ApplyIntentError::IntentInapplicable)
        }
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
                // PR 4b-D.2 (this commit) flipped the production page
                // projection from the PageContext aggregator to the
                // post-PR-4b-B lattice path. Pipeline ordering per
                // `docs/plans/2026-05-01-lattice-design.md` §4.7.4:
                //
                //     parse → join (lattice) → Cl_supp (closure)
                //                            → PageRewrites
                //                            → render
                //
                // The closure operator is monotone (adds facts only,
                // never removes; preserves the join order). PageRewrites
                // are NOT monotone in the FactSet sense — `Clear` /
                // `FactRemove` actions are anti-monotone, and the
                // `noforn-clears-*` family explicitly removes tokens
                // and country-list entries. The pipeline-termination
                // guarantee on PageRewrites comes from the **topological
                // ordering** the scheduler imposes on `reads` / `writes`
                // axes (Kahn's algorithm at `Engine::new`, see
                // `crates/engine/src/scheduler.rs`) — the rewrite graph
                // is a DAG, every rewrite fires at most once per
                // projection, and there is no fixpoint loop at this
                // layer (unlike `closure`'s Kleene fixpoint). Copilot
                // R2 #6 caught the previous "both monotone" claim.
                //
                // Commit 7 perf: the trait path still pays the
                // `markings.iter().map(|m| m.0.clone()).collect()`
                // clone round because the trait's `markings:
                // &[CapcoMarking]` slice ties us to per-portion
                // CapcoMarking values. The engine's hot path bypasses
                // this via `CapcoScheme::project_from_attrs_slice`
                // (PR 6c successor to `project_from_page_context`),
                // which consumes the engine accumulator's slice
                // directly and delegates to the same shared
                // `project_attrs_pipeline` body — without paying the
                // trait wrap-then-unwrap round. Test fixtures and
                // external tooling continue to use this trait-path
                // entry.
                let raw: Vec<CanonicalAttrs> = markings.iter().map(|m| m.0.clone()).collect();
                let out_attrs = self.project_attrs_pipeline(&raw);
                CapcoMarking::new(out_attrs)
            }
        }
    }

    fn page_rewrites(&self) -> &[PageRewrite<Self>] {
        &self.page_rewrites
    }

    /// Commit 5 — substantive `render_canonical` body driven by the
    /// per-axis dispatch table [`RENDER_TABLE`].
    ///
    /// The dispatch loop walks `RENDER_TABLE` in declaration order
    /// (which matches `Category::ordering_rank` per §A.6 p15-17
    /// Figure 2), inserting `//` between consecutive non-empty axes.
    /// Each per-axis renderer in [`crate::render`] writes ONLY its own
    /// bytes to `out`; the dispatch loop is the sole owner of the
    /// `//` major-category separator (CAPCO-2016 §A.6 p15-16).
    ///
    /// `Scope::Diff` returns `Err(fmt::Error)` because diff is a
    /// rule-context query mode, not a renderer-output scope. See
    /// the trait-method doc comment and `marque-rules`'
    /// `RecanonScope` (which narrows `Scope` to exclude `Diff`).
    ///
    /// # Byte-identity invariant
    ///
    /// `scheme.render_canonical(m, Scope::Portion, &mut s)` and
    /// `scheme.render_portion(m)` MUST produce byte-identical output
    /// for any input the existing `render_portion` override handled
    /// (and similarly for `Page` / `render_banner`). The
    /// `render_canonical_default_chain.rs` integration tests pin this
    /// property.
    fn render_canonical(
        &self,
        m: &Self::Marking,
        scope: Scope,
        out: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        if matches!(scope, Scope::Diff) {
            return Err(core::fmt::Error);
        }

        // Track whether any axis has emitted bytes yet AND the family
        // of the last-emitting row so the major-category separator
        // `//` can be downgraded to within-category `/` when two
        // consecutive emitting rows belong to the same dissem family.
        //
        // Authority: CAPCO-2016 §A.6 p16 "Dissemination Control
        // Markings ... A single forward slash with no interjected
        // space must be used to separate multiple dissemination
        // controls." Per §G.1 Table 4 row 8 the dissem category
        // includes single-token dissems (ORCON, NOFORN, ...),
        // REL TO, and DISPLAY ONLY — all of which must be `/`
        // separated when commingled in the same `//`-delimited
        // dissem slot. Previously this loop unconditionally inserted
        // `//` between every emitting row, producing canonical
        // strings like `//ORCON//REL TO USA, GBR` (wrong) instead
        // of `//ORCON/REL TO USA, GBR` (canonical).
        //
        // Implementation: render each axis to a per-axis scratch
        // buffer; if non-empty, prepend `//` (different family) or
        // `/` (same dissem family) and copy to `out`. Classification
        // is special: for non-US / JOINT classifications it carries
        // its OWN leading `//` (per §A.6 p15-16 — the `//` occludes
        // the absent US position), so this loop does not prepend
        // ANY separator to the very first axis that emits.
        let mut scratch = String::new();
        let mut prev_family: Option<DissemFamilyMembership> = None;
        for row in RENDER_TABLE {
            scratch.clear();
            (row.render)(m, scope, &mut scratch)?;
            if scratch.is_empty() {
                continue;
            }
            let curr_family = dissem_family_of(row.category);
            match prev_family {
                None => {
                    // First emitting row: classification owns its
                    // leading `//`; every other first-emit just
                    // writes its own bytes.
                }
                Some(prev) => {
                    if prev == DissemFamilyMembership::Member
                        && curr_family == DissemFamilyMembership::Member
                    {
                        // Two consecutive dissem-family rows:
                        // within-category `/` separator.
                        out.write_str("/")?;
                    } else {
                        // Cross-category: §A.6 p16 `//`.
                        out.write_str("//")?;
                    }
                }
            }
            out.write_str(&scratch)?;
            prev_family = Some(curr_family);
        }
        Ok(())
    }

    fn render_portion(&self, m: &Self::Marking) -> String {
        // Override retained for the Phase A byte-identity gate
        // (`render_canonical_default_chain.rs`). Commit 5's
        // render_canonical body is the substantive renderer; this
        // override delegates to it through the trait-default String
        // round-trip. Removing the override is a follow-up once the
        // engine call sites move off `render_portion` to
        // `render_canonical` (commit 6+).
        //
        // `Write for String` is infallible, so a `String` write target
        // never produces `fmt::Error`. The only way the discarded
        // `Result` could be `Err` is a contract violation: an impl
        // returning `Err` for `Scope::Portion`. The
        // [`MarkingScheme::render_canonical`] doc comment forbids
        // this. Debug-assert in development; in release, the contract
        // violation produces an empty / partial `String` rather than
        // a panic (matching the trait-default behavior in
        // `MarkingScheme::render_portion`).
        let mut s = String::new();
        let result = self.render_canonical(m, Scope::Portion, &mut s);
        debug_assert!(
            result.is_ok(),
            "MarkingScheme::render_canonical contract violation: Err returned for Scope::Portion. \
             Conforming impls MUST return Ok(()) for Portion / Page / Document — see trait doc."
        );
        s
    }

    fn render_banner(&self, m: &Self::Marking) -> String {
        // See `render_portion`. Override retained for byte-identity
        // gate; the substantive body is `render_canonical`. Same
        // contract-violation invariant: `Write for String` is
        // infallible, so `Err` here would be a conforming-impl bug
        // forbidden by the trait doc.
        let mut s = String::new();
        let result = self.render_canonical(m, Scope::Page, &mut s);
        debug_assert!(
            result.is_ok(),
            "MarkingScheme::render_canonical contract violation: Err returned for Scope::Page. \
             Conforming impls MUST return Ok(()) for Portion / Page / Document — see trait doc."
        );
        s
    }

    /// Map a closed CVE [`TokenId`] to its host [`CategoryId`].
    ///
    /// Used by the closure operator to route cone tokens to the correct
    /// marking axis when adding facts during implicit-fact propagation.
    /// Per `docs/plans/2026-05-13-pr3.7-lattice-resolution-gate-plan.md`
    /// §2 finding F1, this is the scheme-layer hook required because
    /// [`Self::category_of`] is keyed by `FactRef<S>` (a `marque-rules`
    /// type) and unavailable at the scheme layer.
    ///
    /// Delegates to the free function `capco_token_category` which is
    /// also used by `category_of`. Returns `None` for sentinel marker
    /// tokens (e.g., `TOK_IC_DISSEM`, `TOK_FGI_MARKER`) that label
    /// categorical predicates rather than addressable atomic tokens.
    fn token_category(&self, id: TokenId) -> Option<CategoryId> {
        capco_token_category(id)
    }

    /// CAPCO implicit-fact propagation catalog (closure operator).
    ///
    /// Returns the static catalog of [`ClosureRule`] rows. The catalog
    /// contains a single Trio 1 CAVEATED row (covering every caveat
    /// marking whose default release posture is "no foreign disclosure"
    /// absent an explicit FD&R decision) and one Trio 3 NATO
    /// `REL TO USA, NATO` row:
    ///
    /// | Rule key                                            | Triggers                                                                 |
    /// |-----------------------------------------------------|--------------------------------------------------------------------------|
    /// | `capco/noforn-if-caveated`                          | SAR · RD / FRD / TFNI · UCNI (DOE/DOD) · FGI · ORCON / ORCON-USGOV · RSEN / IMCON / DSEN · LIMDIS / LES / NNPI / SBU / SSI |
    /// | `capco/rel-to-usa-nato-if-nato-classification`      | bare NATO classification                                                 |
    ///
    /// The CAVEATED row is the algebraic union of seven previously
    /// separate Trio 1 rows (one per source §-citation). All shared the
    /// same suppressor (`FDR_DOMINATORS`), the same cone (`{NOFORN}`),
    /// and the same default severity (`Severity::Info`); per D18
    /// rationale 2 the rows are interchangeable with a single n-ary
    /// trigger. The universal label cites CAPCO-2016 §B.3 Table 2 p21
    /// (rooted in ICD 403); per-token Section H subsection authorities live in the row
    /// doc-comment's per-trigger authority table at
    /// `crates/capco/src/scheme/closure.rs`.
    ///
    /// Every row is suppressed by `FDR_DOMINATORS` (any present
    /// FD&R-axis fact: NOFORN, RELIDO, REL TO, EYES, DISPLAY ONLY).
    /// All rows ship at [`Severity::Info`] per `decisions.md` D19 B
    /// (closure firings are silent lattice-layer fact propagation,
    /// not byte-level fixes); user-visible byte diffs ride on
    /// independent `Severity::Suggest` text-layer rules (e.g., S007
    /// for the NATO row — see `decisions.md` D20).
    ///
    /// The Trio 2 (implicit RELIDO) and per-marking SCI implication
    /// rows (HCS-O/P[sub] ⇒ {NOFORN, ORCON}; TK-BLFH/KAND/IDIT ⇒
    /// {NOFORN}; SI-G ⇒ {ORCON}) are intentionally absent — they
    /// require per-compartment sentinels (`TOK_HCS_O`, `TOK_SI_G`, etc.)
    /// that do not yet exist; the alternative proxy triggers via
    /// `AnyInCategory(CAT_SCI)` / `AnyInCategory(CAT_CLASSIFICATION)`
    /// would over-fire on any SCI marking, not just the specific
    /// compartments.
    ///
    /// Per `specs/006-engine-rule-refactor/decisions.md` D18, this is a
    /// PUBLIC catalog surface — visible to tooling, scheme-exploration
    /// UIs, and docs generators.
    ///
    /// # Engine wiring
    ///
    /// `CapcoScheme::closure()` (below) makes the catalog data reachable
    /// through the operator. Wiring `Engine::lint` to invoke
    /// `scheme.closure()` on the hot path before banner-validation runs
    /// is a separate change; today the operator runs through direct
    /// `scheme.closure(marking)` calls (tests + `scheme.project(Scope::Page,
    /// ...)` for callers that opt in).
    fn closure_rules(&self) -> &[marque_scheme::ClosureRule<CapcoScheme>] {
        CAPCO_CLOSURE_RULES
    }

    /// CAPCO closure operator — Kleene fixpoint over the two closure
    /// rows in [`CAPCO_CLOSURE_RULES`] (Trio 1 CAVEATED + Trio 3
    /// NATO REL TO).
    ///
    /// Implements the §4.7 implicit-fact propagation per
    /// `docs/plans/2026-05-01-lattice-design.md` §3 (e). Walks the
    /// catalog repeatedly; on each pass, every rule that satisfies
    /// `should_fire` contributes both its static `cone` facts (routed
    /// via the `apply_closure_fact` helper in `actions::intent`) and
    /// its `cone_derived` facts (the D21 open-vocab branch — same
    /// routing). Convergence is
    /// detected by comparing the marking to a per-pass snapshot;
    /// monotone catalogs reach the fixed point in at most
    /// `|fact_universe|` iterations, well within
    /// [`MAX_CLOSURE_ITERATIONS`]'s `N=16` safety cap.
    ///
    /// # Invariants preserved
    ///
    /// 1. **Extensive**: `closure(m) ⊒ m` — only facts are added; the
    ///    underlying `apply_fact_add` path rejects removals.
    /// 2. **Idempotent**: `closure(closure(m)) == closure(m)` — the
    ///    snapshot-equality early-return guarantees stable fixpoints.
    /// 3. **Monotone**: `m1 ⊑ m2 ⟹ closure(m1) ⊑ closure(m2)` — relies
    ///    on every catalog row's suppressors being disjoint from every
    ///    cone (the §4.7.3 table-design property). Catalog regressions
    ///    are pinned by
    ///    `crates/scheme/tests/proptest_closure_rejects_non_monotone.rs`.
    ///
    /// # Routing
    ///
    /// Each cone fact (`TokenRef::Token(id)` from the static `cone`,
    /// or `FactRef` from `cone_derived`) is routed through
    /// [`CapcoScheme::category_of`] to its host category and applied
    /// via the same per-axis [`apply_fact_add`] helper that
    /// [`MarkingScheme::apply_intent`]'s `FactAdd` path uses. Per-fact
    /// `IntentInapplicable` (already-present, idempotence) and
    /// `UnknownToken` (sentinel that doesn't address a category — e.g.,
    /// `TokenRef::AnyInCategory(_)` entries in `cone`) are silent
    /// no-ops at the closure layer: the operator is monotone fact
    /// propagation, so a fact the scheme can't route is, by definition,
    /// not in the closure.
    ///
    /// # Non-convergence
    ///
    /// Per the [`MarkingScheme::closure`] trait contract, exceeding
    /// `MAX_CLOSURE_ITERATIONS` panics. A monotone catalog cannot
    /// reach this branch (the fact universe is bounded by the union of
    /// every category's value set); non-convergence here indicates a
    /// catalog regression — a non-monotone rule whose suppressor
    /// depends on a fact in another rule's cone. The companion
    /// proptest at
    /// `crates/scheme/tests/proptest_closure_rejects_non_monotone.rs`
    /// pins the monotonicity property; this panic is the runtime
    /// guard against unbounded-growth catalog defects that slip past
    /// proptest.
    fn closure(&self, marking: Self::Marking) -> Self::Marking {
        // PR 4b-D.2 Commit 6: cone-trigger short-circuit (architect's
        // R-1 mitigation). If no catalog row's trigger fires on the
        // input marking, no rule can contribute a cone fact — the
        // closure is a no-op. Skip the snapshot+clone+fixpoint loop
        // entirely. This is the typical case for the bench corpus
        // (markings without SAR/RD/UCNI/FGI/ORCON/RSEN/IMCON/DSEN/
        // LIMDIS/LES/SBU/SSI/NATO-class triggers) where the closure
        // has nothing to add.
        //
        // Correctness: the short-circuit is sound because
        // `should_fire = trigger_fires && !is_suppressed`, and
        // `trigger_fires` is the necessary condition for any rule to
        // contribute a fact. If `trigger_fires` is false for every
        // rule, no rule fires, no facts are added, and the fixpoint
        // is the input.
        if !self.any_closure_trigger_fires(&marking) {
            return marking;
        }
        let mut working = marking;
        for _iteration in 0..marque_scheme::MAX_CLOSURE_ITERATIONS {
            let snapshot = working.clone();
            for rule in CAPCO_CLOSURE_RULES {
                if !rule.should_fire(self, &working) {
                    continue;
                }
                // Static cone: walk closed-vocab `TokenRef::Token(id)`
                // entries via the rule's helper iterator; the helper
                // already filters out `TokenRef::AnyInCategory(_)`
                // (which is a category-wildcard predicate, not a
                // cone carrier — see the `ClosureRule::cone` doc
                // contract).
                for token_id in rule.cone_token_ids() {
                    let fact_ref = FactRef::Cve(token_id);
                    apply_closure_fact(self, &mut working, &fact_ref);
                }
                // Derived cone (D21 open-vocab path, e.g. NATO partner
                // list): the function MUST be monotone in the marking
                // (per `ClosureRule::cone_derived` doc contract). The
                // NATO row's derived cone is constant-output (vacuously
                // monotone); future rows with marking-dependent
                // derivations (e.g. JOINT) must re-verify the §4.7.3
                // chain-depth analysis per the cap's doc comment.
                if let Some(derived_fn) = rule.cone_derived {
                    for fact_ref in derived_fn(&working) {
                        apply_closure_fact(self, &mut working, &fact_ref);
                    }
                }
            }
            if working == snapshot {
                return working;
            }
        }
        // Non-convergence: catalog regression. See doc-comment above.
        // Per `MarkingScheme::closure` trait contract: MUST panic.
        panic!(
            "CapcoScheme::closure did not converge in {} iterations; \
             this indicates a non-monotone catalog row (see \
             crates/scheme/tests/proptest_closure_rejects_non_monotone.rs \
             for the property under test)",
            marque_scheme::MAX_CLOSURE_ITERATIONS,
        );
    }

    /// Enumerate all tokens present in `marking`.
    ///
    /// Required by `Constraint::ConflictsWithFamily` evaluation: the
    /// generic evaluator walks every present token and applies the
    /// [`FamilyPredicate`] to each. Without this override, the family
    /// predicate never fires (the default returns an empty iterator).
    ///
    /// This implementation walks each attribute field and emits two
    /// shapes of `TokenRef` per the trait contract on
    /// [`MarkingScheme::iter_present_tokens`]:
    ///
    /// - `TokenRef::Token(id)` for concrete closed-CVE tokens whose
    ///   identity matters (the common case — dissem controls, AEA
    ///   markings, non-IC dissem) **and** for per-variant
    ///   classification sentinels: `TOK_JOINT`, `TOK_NATO_CLASS`,
    ///   `TOK_FGI_CLASS`, and the dual-axis `TOK_FGI_MARKER`.
    /// - `TokenRef::AnyInCategory(cat)` for axis-level open-vocab
    ///   facts only: `CAT_REL_TO` when a REL TO country list is
    ///   present, `CAT_SCI` when any SCI marking is present,
    ///   `CAT_SAR` when any SAR program is present. The
    ///   `AnyInCategory` shape lets family predicates (e.g.
    ///   `is_fdr_dominator`) match against an axis without
    ///   enumerating each open-vocab token (REL TO trigraphs, SAR
    ///   program names, SCI compartments).
    ///
    /// Per-variant classification sentinels (added in #509) replaced
    /// the umbrella `AnyInCategory(CAT_NON_US_CLASSIFICATION)`
    /// emission so family predicates can target NATO / FGI / JOINT
    /// individually without re-walking the classification axis. The
    /// umbrella shape is no longer emitted here; `CAT_NON_US_CLASSIFICATION`
    /// remains live as a `Constraint::Requires` LHS (evaluated via
    /// `satisfies_attrs`, not `collect_present_tokens` — see E015 in
    /// `core_catalog.rs`) and for vocabulary admission, just not as
    /// a `collect_present_tokens` emission target.
    ///
    /// Open-vocab tokens whose **identity** is needed (specific REL TO
    /// trigraphs, individual SAR program names, individual SCI
    /// compartments) are not emitted as `TokenRef::Token` because no
    /// current `ConflictsWithFamily` row needs them on the RHS. If a
    /// future family predicate needs per-token granularity on those
    /// axes, this method's emission set should be extended.
    fn iter_present_tokens<'m>(
        &self,
        marking: &'m Self::Marking,
    ) -> Box<dyn Iterator<Item = TokenRef> + 'm> {
        Box::new(collect_present_tokens(&marking.0).into_iter())
    }
}

impl CapcoScheme {
    /// PR 6c successor to `project_from_page_context` — engine-facing
    /// hot-path entry that consumes a pre-built per-page slice of
    /// portion attributes directly. The engine owns the accumulator
    /// that grows portions across the document; this entry forwards
    /// the slice to the shared [`Self::project_attrs_pipeline`] body,
    /// skipping the trait-path's `Vec<CapcoMarking> → Vec<CanonicalAttrs>`
    /// wrap-then-unwrap round.
    ///
    /// The trait-level [`MarkingScheme::project`] entry handles
    /// `&[Self::Marking]` callers — test fixtures and external
    /// tooling — and pays one `.0.clone()` per portion to bridge into
    /// the same `project_attrs_pipeline`.
    ///
    /// Phase-attribution profiling (`crates/engine/benches/profile_project.rs`)
    /// found the tmp_ctx-build round earlier PRs paid at ~2.8µs / n=50
    /// portions; eliminating it closed ~60-80µs of the lint_10kb
    /// regression on the bench's monotone-growing call sequence
    /// (sum_i=1^50 of the per-call tmp_ctx cost). PR 4b-F retired the
    /// last remnant of that tmp_ctx build at every layer. PR 6c
    /// (T069) flattened the parameter from `&PageContext` to
    /// `&[CanonicalAttrs]` so the caller no longer needs to construct
    /// the intermediate accumulator type.
    ///
    /// ## Same-slice property
    ///
    /// `raw` flows directly to `project_attrs_pipeline`. There is no
    /// parallel slice the inner pipeline could drift from, so the
    /// earlier debug-assert that PR 4b-D.2 carried at the fold-body
    /// boundary became vacuous in PR 4b-F and retired with the
    /// `_with_context` variant. Future maintenance that reintroduces
    /// a parallel derivation path MUST re-add the contract at the new
    /// fork — the invariant lives in this doc-comment, not in a
    /// runtime check.
    pub fn project_from_attrs_slice(&self, raw: &[CanonicalAttrs]) -> CanonicalAttrs {
        self.project_attrs_pipeline(raw)
    }

    /// Transitional shim — engine-side caller in `dispatch_page_finalization` /
    /// `project_page_marking` migrates to [`Self::project_from_attrs_slice`]
    /// in PR 6c commit 3. Retained for one commit so the workspace stays
    /// green across the rename. Deleted at commit 3 once the engine no
    /// longer constructs a `PageContext`.
    pub fn project_from_page_context(
        &self,
        page_context: &marque_ism::PageContext,
    ) -> CanonicalAttrs {
        self.project_from_attrs_slice(page_context.portions())
    }

    /// Shared body of the page-projection pipeline. Both
    /// [`MarkingScheme::project`] (trait entry, after a per-portion
    /// `.0.clone()`) and [`Self::project_from_attrs_slice`] (engine
    /// fast-path) delegate here, so the pipeline-step semantics are
    /// identical across all surfaces. Per PR 4b-D.2 §4.7.4:
    ///
    /// ```text
    /// join_via_lattice → closure → PageRewrites
    /// ```
    ///
    /// PR 4b-F retired the `page_ctx: &PageContext` parameter — the
    /// pipeline consumes only `raw: &[CanonicalAttrs]`. The same-slice
    /// contract that earlier PRs threaded as a debug-assert at this
    /// layer became vacuous once `join_via_lattice_body` no longer
    /// reads a `PageContext`: there is no parallel slice for the inner
    /// body to drift from. PR 6c (T069) retired the `PageContext`
    /// struct entirely; engine callers reach this pipeline via
    /// [`Self::project_from_attrs_slice`].
    fn project_attrs_pipeline(&self, raw: &[CanonicalAttrs]) -> CanonicalAttrs {
        // PR 4b-D.2 D23 (decisions.md): closure-rewrite-application
        // sentinel. Per `docs/plans/2026-05-01-lattice-design.md`
        // §3 (e.1) read-only-attrs invariant, the closure operator
        // MUST NOT mutate the per-portion CanonicalAttrs slice it
        // observes. Snapshot the input pre-closure and assert
        // byte-identity afterward.
        //
        // ## G13 content-ignorance (Constitution V Principle V +
        // Copilot R2 #4)
        //
        // The failure path emits ONLY counts and the §-citation
        // literal — never `raw` / `raw_snapshot` content. `debug_assert_eq!`'s
        // default `{:?}` format would dump full `CanonicalAttrs`
        // (token values, country lists, spans), violating G13. The
        // explicit `if !=` + `panic!` with a count-only message
        // mirrors the `check_portions_unchanged` pattern in
        // `crates/engine/src/engine.rs` (PageFinalization-rule-dispatch
        // sentinel). Both sentinels enforce the same §3 (e.1)
        // read-only-attrs invariant; both must keep audit-content-
        // ignorance on the failure path.
        #[cfg(debug_assertions)]
        let raw_snapshot: Vec<CanonicalAttrs> = raw.to_vec();

        let joined = CapcoMarking::new(CapcoMarking::join_via_lattice(raw));
        let mut out = self.closure(joined);

        #[cfg(debug_assertions)]
        {
            if raw != raw_snapshot.as_slice() {
                panic!(
                    "closure() mutated the per-portion CanonicalAttrs slice \
                     ({} portion(s) before vs {} after) — violates PageRewrite \
                     read-only-attrs invariant \
                     (docs/plans/2026-05-01-lattice-design.md §3 (e.1))",
                    raw_snapshot.len(),
                    raw.len(),
                );
            }
        }

        // Apply declarative page rewrites. PR 4b-D.2 hot-path flip:
        // page rewrites run on the post-closure state, so any cone
        // facts the closure operator added are visible to rewrite
        // triggers. NOFORN-clears-REL-TO and similar absorbing rewrites
        // remain inflationary on the closed state (they remove
        // dominated tokens but the remaining tokens are already members
        // of the closure's fixed point).
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
                        tracing::debug!(
                            rewrite_id = rw.id,
                            action = "Clear",
                            ?category,
                            "PageRewrite fired",
                        );
                        capco_category_clear(&mut out, *category);
                    }
                    CategoryAction::Replace { category, with } => {
                        tracing::debug!(
                            rewrite_id = rw.id,
                            action = "Replace",
                            ?category,
                            "PageRewrite fired",
                        );
                        capco_category_replace(&mut out, *category, with);
                    }
                    CategoryAction::Promote { from, to, .. } => {
                        // Phase 3 T034 declares the JOINT-promotion and
                        // FGI-absorption rewrites for the scheduler +
                        // catalog surface. Post-PR-4b-D.2 the engine
                        // drives page-marking aggregation through
                        // `scheme.project(Scope::Page, ...)`, so this
                        // arm is reachable at runtime. `Promote` stays
                        // a no-op here because the JOINT-promotion and
                        // FGI-absorption rewrites are renderer-canonical
                        // territory — they restate the same fact set in
                        // a different surface form, which is
                        // `render_canonical`'s job, not the projection
                        // lattice's. The renderer-vs-lattice boundary
                        // is documented in
                        // `docs/plans/2026-05-01-lattice-design.md`
                        // §10 row 4 (SCI per-system canonicalization)
                        // + §10 row 5 (SAR ordering). PR 5+ Stage 4
                        // lands the renderer trait surface that picks
                        // these rewrites up.
                        tracing::debug!(
                            rewrite_id = rw.id,
                            action = "Promote",
                            ?from,
                            ?to,
                            "PageRewrite fired (Promote — renderer-territory no-op)",
                        );
                    }
                    CategoryAction::Custom(f) => {
                        tracing::debug!(rewrite_id = rw.id, action = "Custom", "PageRewrite fired",);
                        f(&mut out);
                    }
                    CategoryAction::Intent(intent) => {
                        // Bridge to the existing per-intent helper.
                        // Errors are handled as follows:
                        // - `Ok(())`: rewrite applied, marking mutated.
                        // - `IntentInapplicable`: silent no-op for
                        //   this rewrite (idempotent — the marking was
                        //   already in the post-rewrite state).
                        // - `UnknownToken`: pre-validated for callers
                        //   that go through `Engine::new` (see
                        //   `validate_intent_rewrites` in
                        //   marque-engine). Direct callers of
                        //   `CapcoScheme::project` (e.g., tests,
                        //   scheme-exploration tooling) bypass that
                        //   validation, so this arm IS reachable on
                        //   the project path; it's also reachable if
                        //   the scheme is mutated between Engine
                        //   construction and call.
                        // - `IntentRejectsLattice`: NOT pre-validated
                        //   — it's a runtime condition (lattice
                        //   invariant violation) that
                        //   `validate_intent_rewrites` cannot detect
                        //   without simulating the intent application.
                        // - Future `ReplacementIntent` variants that
                        //   reach the `apply_intent_to_marking` `_`
                        //   arm also land here.
                        // In every error-arm case, log and treat as a
                        // silent no-op rather than panic; `Engine::lint`'s
                        // hot path must not unwind into Tower middleware.
                        // The corpus-parity tests will surface
                        // incorrect projection output.
                        match apply_intent_to_marking(self, &mut out, intent) {
                            Ok(()) => {
                                tracing::debug!(
                                    rewrite_id = rw.id,
                                    action = "Intent",
                                    "PageRewrite fired (CategoryAction::Intent)",
                                );
                            }
                            Err(ApplyIntentError::IntentInapplicable) => {
                                tracing::debug!(
                                    rewrite_id = rw.id,
                                    action = "Intent",
                                    "PageRewrite no-op (intent already satisfied)",
                                );
                            }
                            Err(e) => {
                                tracing::error!(
                                    rewrite_id = rw.id,
                                    error = ?e,
                                    "PageRewrite Intent failed at runtime — expected to be \
                                     caught at Engine::new validation. Treating as no-op.",
                                );
                            }
                        }
                    }
                }
            }
        }
        out.0
    }

    /// PR 4b-D.2 Commit 6 — hot-path short-circuit predicate for
    /// [`MarkingScheme::closure`].
    ///
    /// Returns `true` if ANY catalog row's trigger fires on `marking`.
    /// If this returns `false`, the closure operator is guaranteed to
    /// be a no-op (no rule can contribute a cone fact when its trigger
    /// is unsatisfied), so the caller can skip the snapshot-and-clone
    /// fixpoint loop entirely. This is the architect's R-1
    /// optimization referenced in the PR 4b-D.2 brief: bench
    /// measurement of `lint_10kb` post-hot-path-flip landed at 1.5ms
    /// (+65% over baseline) because the closure ran on every
    /// portion-bounded page-marking cache miss (~20 invocations per
    /// 10KB doc), and the typical bench page has no SAR / RD / UCNI /
    /// FGI / ORCON / RSEN / IMCON / DSEN / LIMDIS / LES / SBU / SSI /
    /// NATO-class trigger — so the closure does an unnecessary
    /// snapshot-and-compare every time. The short-circuit skips that
    /// per-projection cost for the no-trigger case.
    ///
    /// # Correctness
    ///
    /// `should_fire = trigger_fires && !is_suppressed`. The closure
    /// fixpoint only adds facts when `should_fire` is true for at
    /// least one rule; if `trigger_fires` is false for every rule,
    /// the fixpoint is the input. The short-circuit checks the
    /// disjunction over rules — the cheapest possible necessary
    /// condition.
    ///
    /// Suppression is NOT consulted by this predicate. A
    /// trigger-firing-but-suppressed rule still pays the full
    /// fixpoint loop's snapshot-and-compare cost — that's correct:
    /// suppression may flip across iterations if another rule's
    /// cone adds a suppressor fact, so the loop must run to
    /// verify the fixpoint. The short-circuit specifically targets
    /// the "trigger fires for nothing" case which is the bench
    /// majority.
    ///
    /// # Worst-case cost
    ///
    /// O(rules × triggers-per-rule) `satisfies` calls. The current
    /// catalog has 8 rules × ≤3 triggers each = ≤24 satisfies calls
    /// per projection. Each `satisfies` call walks a tiny constant
    /// number of category fields (`attrs.sar_markings.is_some()`,
    /// `attrs.dissem_us.contains(...)`, etc.). The short-circuit
    /// adds ≤24 bool ops to the closure entry path; the closure
    /// itself (when not short-circuited) adds `working.clone()` per
    /// iteration of the fixpoint, which is the cost this exists to
    /// avoid.
    pub(crate) fn any_closure_trigger_fires(&self, marking: &CapcoMarking) -> bool {
        CAPCO_CLOSURE_RULES
            .iter()
            .any(|rule| rule.trigger_fires(self, marking))
    }
}
