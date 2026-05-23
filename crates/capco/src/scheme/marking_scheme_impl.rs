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

use marque_ism::{CanonicalAttrs, ParsedAttrs};
use marque_scheme::{
    ApplyIntentError, Category, CategoryAction, CategoryId, CategoryPredicate, ClosureRuleMetadata,
    Constraint, ConstraintViolation, FactRef, MarkingScheme, PageRewrite, Parsed, RenderContext,
    ReplacementIntent, Scope, Span, Template, TokenId, TokenRef,
};

use super::actions::*;
use super::closure::CAPCO_CLOSURE_RULES;
use super::predicates::*;
use super::*;

// HOT-1 axis-flag scaffolding retired in PR-D of the FactBitmask
// refactor (issue #371). The structural `ClosureAxisFlags` snapshot
// + `working_has_caveated_dissem_trigger` + `capco_rule_axis_present`
// per-rule guard were replaced by the single `ALL_TRIGGER_MASK`
// short-circuit on the bitmask projection at the top of
// `CapcoScheme::closure` below — branchless, no per-axis fan-out, no
// per-rule guard table to keep in sync with `CAPCO_CLOSURE_RULES`
// catalog order.

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

    // GAT + plain associated type bindings introduced in PR 3c.2.A
    // per `docs/plans/2026-05-19-pr3c2-a-pm-decisions.md` PM-1.
    type Parsed<'src> = ParsedAttrs<'src>;
    type Canonical = CanonicalAttrs;

    /// CAPCO/ISM canonicalization — collapse the borrowed
    /// `ParsedAttrs<'src>` produced by `marque-core`'s strict parser
    /// to the owned `CanonicalAttrs` form rules consume.
    ///
    /// **Structural rename only.** Every field is moved across without
    /// transformation — no case folding, no deprecated-token migration,
    /// no canonicalization. Phase A: this is the only shape CapcoScheme
    /// needs; future schemes (CUI / NATO) that fold or migrate fields
    /// override this method with their own logic.
    ///
    /// `&self` is unused today, but the trait signature reserves it
    /// for stateful future schemes.
    ///
    /// **FR-043 sole-path invariant**: this is the only public
    /// `ParsedAttrs → CanonicalAttrs` route for production code.
    /// The `marque_ism::from_parsed_unchecked` adapter that PR 3a–3c
    /// kept in `crates/ism/src/canonical.rs` retired in PR 3c.2.E
    /// once `ParsedAttrs` and `CanonicalAttrs` lost their
    /// `#[non_exhaustive]` attributes — destructure and literal
    /// construction outside `marque-ism` is the path that lets this
    /// body live in the scheme adapter, where it belongs.
    fn canonicalize<'src>(&self, parsed: Self::Parsed<'src>) -> Self::Canonical {
        let ParsedAttrs {
            classification,
            sci_markings,
            sci_controls,
            sar_markings,
            aea_markings,
            fgi_marker,
            dissem_us,
            dissem_nato,
            non_ic_dissem,
            rel_to,
            display_only_to,
            declassify_on,
            classified_by,
            derived_from,
            declass_exemption,
            token_spans,
            source_bytes_origin: _, // discarded; not on CanonicalAttrs
        } = parsed;

        let out = CanonicalAttrs {
            classification: classification.map(|c| c.value),
            sci_controls,
            sci_markings: Vec::from(sci_markings)
                .into_iter()
                .map(|p| p.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            sar_markings: sar_markings.map(|p| p.value),
            aea_markings: Vec::from(aea_markings)
                .into_iter()
                .map(|p| p.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            fgi_marker: fgi_marker.map(|p| p.value),
            // PR 9b (T132): preserve the parser-side attribution. The
            // attribution function lives on the `ParsedAttrs` side; this
            // canonicalize impl is a pure structural rename and must not
            // re-run it.
            dissem_us: Vec::from(dissem_us)
                .into_iter()
                .map(|p| p.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            dissem_nato: Vec::from(dissem_nato)
                .into_iter()
                .map(|p| p.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            non_ic_dissem: Vec::from(non_ic_dissem)
                .into_iter()
                .map(|p| p.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            rel_to: Vec::from(rel_to)
                .into_iter()
                .map(|p| p.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            display_only_to: Vec::from(display_only_to)
                .into_iter()
                .map(|p| p.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            declassify_on: declassify_on.map(|p| p.value),
            classified_by: classified_by.map(Box::<str>::from),
            derived_from: derived_from.map(Box::<str>::from),
            declass_exemption,
            token_spans,
        };

        // PR 9b (T132) invariant insurance. `attribute_dissems` is the
        // single source of truth; this debug-only assertion catches a
        // future bug where attribution is skipped or the canonical
        // adapter is fed a hand-built `ParsedAttrs` with both fields
        // populated.
        #[cfg(debug_assertions)]
        {
            debug_assert!(
                out.dissem_nato.is_empty() || out.us_classification().is_none(),
                "dissem_nato populated alongside US classification — \
                 attribute_dissems was skipped or bypassed. CAPCO-2016 p41 \
                 reciprocity rule violated."
            );
        }

        out
    }

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
    ///
    /// The free-function `satisfies_attrs` below is the authoritative
    /// implementation; this trait method is a thin forwarder.
    fn satisfies(&self, marking: &Self::Marking, token_ref: &TokenRef) -> bool {
        satisfies_attrs(&marking.0, token_ref)
    }

    fn token_span(&self, marking: &Self::Marking, token_ref: &TokenRef) -> Option<Span> {
        token_span_attrs(&marking.0, token_ref)
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

    /// Pre-compute the [`FactBitmask`] projection of `marking` once per
    /// constraint-evaluation pass. Forwarded to every
    /// [`Self::evaluate_custom`] call by
    /// [`marque_scheme::constraint::evaluate`] so tier-1/2/3 catalog
    /// rows share a single `derive_bits` call instead of recomputing
    /// it per row.
    fn precompute_bits(&self, marking: &Self::Marking) -> marque_scheme::FactBitmask {
        crate::fact_bitmask::derive_bits(&marking.0)
    }

    /// Dispatch a [`Constraint::Custom`] entry to its scheme-private
    /// predicate body. Delegates to `evaluate_custom_by_attrs`, the
    /// name→helper router that the fast-path
    /// [`Self::evaluate_named_constraint`] uses.
    fn evaluate_custom(
        &self,
        name: &'static str,
        marking: &Self::Marking,
        bits: marque_scheme::FactBitmask,
    ) -> Vec<ConstraintViolation> {
        evaluate_custom_by_attrs(&marking.0, bits, name)
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
        ctx: &RenderContext,
        out: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        // PR 3c.2.A: signature migrated from bare `scope: Scope` to
        // `ctx: &RenderContext` per
        // `docs/plans/2026-05-19-pr3c2-a-pm-decisions.md` PM-1. The
        // body continues to dispatch on `ctx.scope` exactly as it did
        // pre-3c.2 — `ctx.emission_form` and `ctx.schema_version` are
        // plumbed through but NOT yet consumed by the per-axis
        // renderers (a future PR will land the §G.1 Table 4 dispatch
        // body). T056 corpus regression is the byte-identity gate.
        if matches!(ctx.scope, Scope::Diff) {
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
            // Per-axis renderers still take a bare `Scope` at PR
            // 3c.2.A (PM-1 forbids changing row signatures in A; the
            // emission-form aware row dispatch lands at 3c.2.B).
            (row.render)(m, ctx.scope, &mut scratch)?;
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
        //
        // PR 3c.2.A: construct an `Auto + MarqueMvp3` RenderContext;
        // a future PR will land the §G.1 Table 4 dispatch body.
        let mut s = String::new();
        let ctx = RenderContext::new(
            Scope::Portion,
            marque_scheme::EmissionForm::Auto,
            marque_scheme::SchemaVersionId::MarqueMvp3,
        );
        let result = self.render_canonical(m, &ctx, &mut s);
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
        //
        // PR 3c.2.A: construct an `Auto + MarqueMvp3` RenderContext.
        let mut s = String::new();
        let ctx = RenderContext::new(
            Scope::Page,
            marque_scheme::EmissionForm::Auto,
            marque_scheme::SchemaVersionId::MarqueMvp3,
        );
        let result = self.render_canonical(m, &ctx, &mut s);
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

    /// Residual fn-pointer closure-rule catalog (PR-D — issue #371).
    ///
    /// Post-PR-D of the FactBitmask refactor, CAPCO's closure operator
    /// executes as a bitwise Kleene fixpoint over
    /// [`CLOSURE_TABLE`](super::closure_table::CLOSURE_TABLE) — a 10-row
    /// `(trigger_mask, suppressor_mask, cone_mask)` catalog over a
    /// `u128` atom bitmask. The fn-pointer slice this trait method
    /// returns is the **residual** catalog: rows whose cone cannot be
    /// expressed as a static closed-vocab bit and therefore retain
    /// fn-pointer form. Today that is exactly one row:
    ///
    /// | Rule key                                            | Why fn-pointer                                                                                                       |
    /// |-----------------------------------------------------|----------------------------------------------------------------------------------------------------------------------|
    /// | `capco/rel-to-usa-nato-if-nato-classification`      | Open-vocab `cone_derived` injects `CountryCode::NATO`, which has no closed-vocab `TokenId`. Bitmask cannot represent it. |
    ///
    /// The static `cone` half of this row (`TOK_USA`) IS in the
    /// bitmask (`closure_table::CONE_REL_TO_USA`); only the open-vocab
    /// NATO leg rides this fn-pointer surface, as a single post-Kleene
    /// tail invoked from [`Self::closure`] when the bitmask Row 7 has
    /// fired (i.e., the `REL_TO_USA` cone bit appears in the
    /// closed_bits delta).
    ///
    /// All other pre-PR-D rows — Trio 1 CAVEATED (the 20-trigger
    /// algebraic union covering SAR · RD/FRD/TFNI · UCNI · FGI · ORCON
    /// / ORCON-USGOV · RSEN / IMCON / PROPIN / DSEN / FISA / RAWFISA ·
    /// LIMDIS / LES / NNPI / SBU / SSI), the per-marking SCI
    /// implications (HCS-O / HCS-P[sub] ⇒ {NOFORN, ORCON}; SI-G ⇒
    /// {ORCON} with NOFORN supplied transitively via Trio 1;
    /// TK-{BLFH, IDIT, KAND} ⇒ {NOFORN}), and the two Trio 2 RELIDO
    /// rows — live exclusively in [`CLOSURE_TABLE`] now. Their
    /// §-citations are preserved verbatim on the bitmask row `label`
    /// fields and the per-row doc-comments in
    /// [`super::closure_table`](super::closure_table).
    ///
    /// Every surviving row is suppressed by `FDR_DOMINATORS` (any
    /// present FD&R-axis fact: NOFORN, RELIDO, REL TO, EYES,
    /// DISPLAY ONLY) — see [`super::closure::FDR_DOMINATORS`] and
    /// its bitmask projection
    /// [`crate::fact_bitmask::MASK_FDR_DOMINATORS`]. All rows ship at
    /// [`Severity::Info`] per `decisions.md` D19 B (closure firings
    /// are silent lattice-layer fact propagation, not byte-level
    /// fixes); user-visible byte diffs ride on independent
    /// `Severity::Suggest` text-layer rules (e.g., S007 for the NATO
    /// row — see `decisions.md` D20).
    ///
    /// `closure_rules()` intentionally remains the residual executable
    /// fn-pointer catalog (1 row post-PR-D). Scheme-agnostic discovery
    /// should use [`Self::closure_inventory()`], which unifies metadata
    /// across this residual catalog and the 10-row bitmask
    /// [`CLOSURE_TABLE`](super::closure_table::CLOSURE_TABLE).
    fn closure_rules(&self) -> &[marque_scheme::ClosureRule<CapcoScheme>] {
        CAPCO_CLOSURE_RULES
    }

    fn closure_inventory(&self) -> Box<dyn Iterator<Item = ClosureRuleMetadata> + '_> {
        use super::closure_table::CLOSURE_TABLE;

        let residual_rules = self.closure_rules();
        let mut inventory = Vec::with_capacity(CLOSURE_TABLE.len() + residual_rules.len());

        // Canonical registry order comes from the bitmask table (10 rows).
        // Prefer fn-pointer metadata when a residual row with the same name
        // exists, so row 7 keeps the surviving fn-pointer source-of-truth.
        for row in CLOSURE_TABLE {
            if let Some(rule) = residual_rules.iter().find(|rule| rule.name == row.name) {
                inventory.push(ClosureRuleMetadata::from(rule));
            } else {
                inventory.push(ClosureRuleMetadata {
                    name: row.name,
                    // PR 10.A.1 split the pre-existing `label: &'static str`
                    // into `display_label: &'static str` (human-facing UI
                    // text) + `label: Citation` (typed authoritative-source
                    // anchor). Metadata reads from each field directly —
                    // no string-form display of the citation, which would
                    // need runtime formatting.
                    label: row.display_label,
                    citation: Some(row.label),
                    default_severity: row.default_severity,
                });
            }
        }

        // Any fn-pointer rows not represented in the bitmask table are appended
        // in fn-pointer declaration order.
        for rule in residual_rules {
            if !CLOSURE_TABLE.iter().any(|row| row.name == rule.name) {
                inventory.push(ClosureRuleMetadata::from(rule));
            }
        }

        Box::new(inventory.into_iter())
    }

    /// CAPCO closure operator — bitwise Kleene fixpoint over
    /// [`CLOSURE_TABLE`](super::closure_table::CLOSURE_TABLE) +
    /// post-Kleene Row 7 NATO open-vocab tail.
    ///
    /// Implements the section 4.7 implicit-fact propagation per
    /// `docs/plans/2026-05-01-lattice-design.md` section 3 (e).
    ///
    /// # Algorithm (post-PR-D, issue #371)
    ///
    /// 1. **Project** the input `CanonicalAttrs` to a `u128`
    ///    `FactBitmask` via [`derive_bits`]. Closed-vocab atoms (SAR
    ///    presence, AEA family bits, FGI marker, IC/non-IC dissem
    ///    tokens, SCI compartments, NATO classification, US collateral
    ///    classification, REL_TO_USA / REL_TO_PRESENT sentinels)
    ///    project to one bit each; open-vocab axes are NOT in the
    ///    bitmask.
    /// 2. **HOT-1 short-circuit**: if no trigger atom is set, return
    ///    the input verbatim. `close` is extensive (bits are only
    ///    added) — no row can fire across any iteration if no trigger
    ///    is set at iteration 0.
    /// 3. **Kleene fixpoint**: [`close`] runs a bitwise loop over
    ///    [`CLOSURE_TABLE`] — for each row, if
    ///    `(next & trigger_mask) != 0 && (next & suppressor_mask) == 0`,
    ///    OR `cone_mask` into `next`. Iterate until stable or panic at
    ///    [`MAX_CLOSURE_ITERATIONS`] (= 16). The CAPCO catalog's
    ///    longest causal chain is depth 2; typical inputs converge in
    ///    1–3 iterations.
    /// 4. **Write-back**: [`apply_closed_bits_to`] materializes every
    ///    new bit in `closed_bits & !input_bits` to the corresponding
    ///    `CanonicalAttrs` axis (`dissem_us` push, `rel_to` insert,
    ///    etc.).
    /// 5. **Row 7 open-vocab tail**: `CountryCode::NATO` has no
    ///    closed-vocab `TokenId`, so it cannot ride the bitmask. If
    ///    the bitmask Row 7 fired (observable as
    ///    `(closed_bits & !input_bits) & CONE_REL_TO_USA != 0`), call
    ///    [`super::closure::CLOSURE_REL_TO_USA_NATO`]'s `cone_derived`
    ///    once and route the resulting `FactRef::OpenVocab(NATO)`
    ///    through `apply_closure_fact`.
    ///
    /// # Invariants preserved
    ///
    /// 1. **Extensive**: `closure(m) ⊒ m` — `close` only OR-s cone
    ///    bits; `apply_closed_bits_to` is a pure-additive projector.
    /// 2. **Idempotent**: `closure(closure(m)) == closure(m)` — the
    ///    bitmask Kleene loop runs to fixpoint, and `apply_closed_bits_to`
    ///    is a no-op for bits already present in the input.
    /// 3. **Monotone**: `m1 ⊑ m2 ⟹ closure(m1) ⊑ closure(m2)` — every
    ///    catalog row's suppressors are disjoint from every cone (the
    ///    section 4.7.3 table-design property). Bitmask regressions
    ///    are pinned by `proptest_closure_table.rs` (P1–P4 algebraic
    ///    properties) and the
    ///    [`CLOSURE_TABLE`](super::closure_table::CLOSURE_TABLE)
    ///    positional pin in `post_4b_lattice_inventory_pin.rs`.
    ///
    /// # Non-convergence
    ///
    /// Per the [`MarkingScheme::closure`] trait contract, exceeding
    /// `MAX_CLOSURE_ITERATIONS` panics. A monotone catalog cannot
    /// reach this branch (the fact universe is bounded by the union
    /// of every category's value set); non-convergence here indicates
    /// a catalog regression — a non-monotone row whose suppressor
    /// depends on a bit in another row's cone. [`close`] panics
    /// unconditionally on non-convergence (release builds included)
    /// per the documented contract.
    fn closure(&self, marking: Self::Marking) -> Self::Marking {
        // Bitmask Kleene fast path (issue #371, PR-D).
        //
        // PR-C landed `CLOSURE_TABLE` + `close()`: a 10-row bitmask catalog
        // and Kleene fixpoint loop covering the closed-vocab atoms of every
        // CAPCO closure rule. PR-D wires it into production. The bitmask
        // path replaces the previous fn-pointer walk of `CAPCO_CLOSURE_RULES`
        // for every closed-vocab cone — only Row 7's `cone_derived` open-
        // vocab NATO tetragraph survives outside the bitmask (see below).
        //
        // # Cost shape
        //
        // - HOT-1: `derive_bits` is single-pass + branchless; the
        //   `ALL_TRIGGER_MASK` short-circuit gates everything else.
        // - Kleene loop: bitwise AND/OR/cmp on a `u128` per row per
        //   iteration, capped at `MAX_CLOSURE_ITERATIONS` (= 16). The CAPCO
        //   catalog's longest causal chain is depth 2; typical inputs
        //   converge in 1–3 iterations.
        // - `apply_closed_bits_to`: O(set bits in delta). Most closures
        //   touch 1–3 atoms.
        //
        // # Row 7 open-vocab tail
        //
        // `CLOSURE_REL_TO_USA_NATO` carries an open-vocab `cone_derived`
        // that injects `CountryCode::NATO` into `rel_to`. `CountryCode::NATO`
        // has no closed-vocab `TokenId`, so it routes via
        // `FactRef::OpenVocab(_)`. The bitmask handles the static `TOK_USA`
        // cone via the `REL_TO_USA` bit; the open-vocab NATO injection
        // happens here, AFTER the Kleene fixpoint, as a single post-pass.
        //
        // The Row 7 trigger / suppressor decision is NOT re-evaluated
        // against the fn-pointer rule's predicates here — it was already
        // made INSIDE [`close`] when CLOSURE_TABLE Row 7's
        // `trigger_mask` / `suppressor_mask` decided whether to OR the
        // `REL_TO_USA` cone bit into the accumulator. The post-Kleene
        // tail observes that decision via the bit-delta gate
        // `row7_fired` (computed just below) and runs the open-vocab
        // injection iff Row 7 actually contributed its closed-vocab
        // cone. See the doc-comment on `row7_fired` for why the delta
        // (`closed & !input`) is required rather than a naive
        // re-evaluation of the fn-pointer predicates against post-
        // closure `working`.
        //
        // # Trait contract
        //
        // The bitmask `close()` panics unconditionally on non-convergence
        // per the `MarkingScheme::closure` trait contract — see
        // `closure_table::close` doc-comment for the panic semantics.

        use crate::fact_bitmask::{apply_closed_bits_to, derive_bits};
        use crate::scheme::closure::CLOSURE_REL_TO_USA_NATO;
        use crate::scheme::closure_table::{ALL_TRIGGER_MASK, CONE_REL_TO_USA, close};

        let input_bits = derive_bits(&marking.0);

        // HOT-1: pre-Kleene short-circuit. If no trigger fires on the
        // input, no row can fire across any iteration (close is extensive,
        // bits are only added). Return the input verbatim, skipping both
        // the Kleene loop AND the Row 7 open-vocab tail (which requires
        // NATO classification — a trigger atom).
        if (input_bits.bits() & ALL_TRIGGER_MASK) == 0 {
            return marking;
        }

        let closed_bits = close(input_bits);

        // Row 7 open-vocab tail decision: did the bitmask Row 7 fire?
        // Equivalent to `(trigger ∧ ¬suppressor)` over `input_bits` for
        // Row 7's masks — observable as "the bitmask added the
        // `REL_TO_USA` cone bit to the accumulator". Computed BEFORE
        // `apply_closed_bits_to` writes back to `CanonicalAttrs` so
        // there is no `rel_to` non-empty ambiguity: at this point
        // `working.0.rel_to` is still the input marking's value, and
        // the bitmask path's decision is the load-bearing one.
        //
        // # Why not re-evaluate against post-`apply_closed_bits_to` state
        //
        // The fn-pointer walker that PR-D retired called
        // `cone_derived(&working)` inside the same row-dispatch
        // iteration where the static cone (`TOK_USA`, applied via
        // `apply_fact_add`'s CAT_REL_TO arm) had just run — meaning
        // `working.rel_to` was already non-empty when `cone_derived`
        // returned NATO, and the suppressor `AnyInCategory(CAT_REL_TO)`
        // would have stripped Row 7 in iteration 2 (after USA injection
        // made `rel_to` non-empty). The fn-pointer fixpoint still
        // contained NATO because iteration 1 ran the derived-cone pass
        // before iteration 2 even started.
        //
        // The bitmask path collapses every iteration into a single
        // `close()` call. To preserve the fn-pointer fixpoint, the
        // open-vocab tail must observe Row 7's "did it ever fire?"
        // decision — not "does it still fire after the bitmask
        // converged?". The bitmask's `REL_TO_USA` cone bit IS that
        // decision: it is set in `closed_bits` iff Row 7 fired in some
        // Kleene iteration. (The bitmask `MASK_FDR_DOMINATORS` does
        // NOT contain `REL_TO_USA` — only `REL_TO_PRESENT`, which
        // remains an input-only sentinel by construction in
        // `derive_bits` — so adding `REL_TO_USA` mid-Kleene cannot
        // retroactively suppress Row 7. Match that here by gating on
        // `closed_bits.is_set(REL_TO_USA)`.)
        // Use the bit-delta `(closed & !input)`, not the post-state, so
        // an input marking that already carries USA in `rel_to`
        // (REL_TO_USA already set, REL_TO_PRESENT also set ⇒ FD&R
        // dominator suppresses Row 7 in the bitmask Kleene → no cone
        // delta) is correctly distinguished from one where Row 7's
        // cone ran and added USA.
        let row7_fired = (closed_bits.bits() & !input_bits.bits()) & CONE_REL_TO_USA != 0;

        let mut working = marking;
        apply_closed_bits_to(&mut working.0, closed_bits, input_bits);

        if row7_fired && let Some(derived_fn) = CLOSURE_REL_TO_USA_NATO.cone_derived {
            for fact_ref in derived_fn(&working) {
                // The return value (changed-bit) is consumed by the
                // fn-pointer walker that retired in PR-D; in the
                // bitmask path the Row 7 NATO post-pass runs exactly
                // once after the Kleene fixpoint converges, so we
                // intentionally discard the bool.
                let _ = apply_closure_fact(self, &mut working, &fact_ref);
            }
        }

        working
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
    /// Check whether any closure rule's trigger fires on `marking`.
    ///
    /// Returns `true` iff the bitmask projection of `marking` intersects
    /// the union of every `CLOSURE_TABLE` row's `trigger_mask`
    /// ([`ALL_TRIGGER_MASK`]) — the same short-circuit gate
    /// [`CapcoScheme::closure`] uses to skip the Kleene fixpoint on
    /// trigger-free inputs.
    ///
    /// Retained as a test-only helper for assertions that need to
    /// inspect trigger-firing without running the full closure pipeline.
    /// Suppression is NOT consulted: a trigger-firing-but-suppressed
    /// row still returns `true` here, matching the pre-PR-D fn-pointer
    /// walker's behavior.
    ///
    /// [`ALL_TRIGGER_MASK`]: crate::scheme::closure_table::ALL_TRIGGER_MASK
    #[cfg(test)]
    pub(crate) fn any_closure_trigger_fires(&self, marking: &CapcoMarking) -> bool {
        use crate::fact_bitmask::derive_bits;
        use crate::scheme::closure_table::ALL_TRIGGER_MASK;
        let _ = self;
        (derive_bits(&marking.0).bits() & ALL_TRIGGER_MASK) != 0
    }
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

    /// Issue #704 — apply §H.8 p145 FD&R supersession to the
    /// post-closure CanonicalAttrs.
    ///
    /// Runs unconditionally as part of [`Self::project_attrs_pipeline`]
    /// between [`MarkingScheme::closure`] and the declarative
    /// `PageRewrites`. The overlay is the lattice-layer answer to the
    /// closure operator being purely additive post-#704: when an
    /// explicit FD&R decision exists on the post-closure state, this
    /// step strips the closure-added implicit defaults that conflict
    /// with §H.8 p145.
    ///
    /// Two strips happen in order (the second consumes the first's
    /// output):
    ///
    /// 1. **Dissem axis strip**
    ///    ([`crate::lattice::dissem::DissemSet::with_fdr_dominance_stripped`]):
    ///    if `Nf` is present in `attrs.dissem_us`, remove every
    ///    dominated control (`Rel`, `Relido`, `Displayonly`, `Eyes`)
    ///    per §H.8 p145.
    ///
    /// 2. **REL TO + DISPLAY ONLY country-list clear**
    ///    ([`crate::lattice::rel_to::RelToBlock::with_nato_implicit_stripped`]):
    ///    if `Nf` ended up in `attrs.dissem_us` (read after step 1),
    ///    clear `attrs.rel_to` and `attrs.display_only_to`. §H.8 p145
    ///    is symmetric across the dissem-axis tokens and the country-
    ///    list axes — both sides of NOFORN's mutual exclusion list
    ///    must be evicted.
    ///
    /// Idempotent: rerunning observes the post-strip state and finds
    /// nothing to do. Read-only with respect to inputs that don't
    /// carry an FD&R dominator.
    ///
    /// Authority: §H.8 p145 (NOFORN: "Cannot be used with REL TO,
    /// RELIDO, EYES ONLY, or DISPLAY ONLY"); §B.3.a p19 (FD&R
    /// dominator enumeration); §D.2 Table 3 rows 1-2 (NOFORN
    /// dominates dominated FD&R at banner roll-up).
    fn apply_supersession_overlays(attrs: &mut CanonicalAttrs) {
        use crate::lattice::DissemSet;
        use marque_ism::DissemControl;

        // Step 1 — dissem-axis strip. Only run the BTreeSet
        // round-trip when `Nf` is actually present; the common case
        // (no NOFORN in the post-closure state) is a single
        // `iter().any(...)` test that costs no allocations.
        let has_noforn = attrs.dissem_us.iter().any(|d| *d == DissemControl::Nf);
        if has_noforn {
            // Build a DissemSet view, run the overlay, write back if
            // changed. The DissemSet round-trip preserves the natural
            // BTreeSet order — same shape `apply_closed_bits_to`
            // produces on a Kleene-delta strip, so re-running this
            // overlay over an already-stripped attrs is a no-op.
            let before_len = attrs.dissem_us.len();
            let view = DissemSet::from_attrs_iter(std::slice::from_ref(attrs));
            // `from_attrs_iter` already applies the supersession
            // overlay during construction, but it operates on a fresh
            // BTreeSet built from per-portion `dissem_us` — its
            // strip is byte-equivalent to `with_fdr_dominance_stripped`
            // for single-portion input. Calling the overlay
            // explicitly is the documented entry point for this
            // strip (issue #704); it remains a no-op when the
            // BTreeSet construction already stripped dominated
            // tokens.
            let stripped = view.with_fdr_dominance_stripped();
            let next = stripped.into_boxed_slice();
            if next.len() != before_len {
                attrs.dissem_us = next;
            }
        }

        // Step 2 — country-list clear. Re-check NOFORN presence; the
        // dissem strip cannot have removed NOFORN (the strip only
        // removes DOMINATED controls; NOFORN itself is the dominator
        // and survives), so this re-check is the same boolean. Kept
        // as a fresh inspection for clarity — the §H.8 p145 country-
        // list clearing rule reads exactly "if NOFORN is present,
        // strip rel_to and display_only_to."
        if has_noforn {
            if !attrs.rel_to.is_empty() {
                attrs.rel_to = Box::new([]);
            }
            if !attrs.display_only_to.is_empty() {
                attrs.display_only_to = Box::new([]);
            }
        }
        // Note: the `RelToBlock::with_nato_implicit_stripped` lattice
        // method is the typed surface for this strip. The CanonicalAttrs
        // path above is the production write-back since project()
        // operates on attrs after the lattice round-trip in
        // `join_via_lattice`. The method exists for callers that
        // want the typed overlay (test fixtures, future refactors
        // that operate on RelToBlock directly).
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

        // Issue #704 — FD&R supersession overlay (post-closure).
        //
        // The `CapcoScheme::closure` operator is purely additive (Kleene
        // fixpoint over `CLOSURE_TABLE`; the `suppressor_mask` gate that
        // previously prevented Trio 1 / Trio 2 / Trio 3 cones from firing
        // when an FD&R dominator was already present was retired in
        // issue #704 because it broke the closure operator's algebraic
        // monotonicity property `a ⊑ b ⟹ Cl(a) ⊑ Cl(b)`).
        //
        // The §H.8 p145 NOFORN-dominates / §B.3.a p19 FD&R supersession
        // semantics that the suppressors encoded move HERE — a per-axis
        // supersession overlay that runs AFTER closure converges and
        // resolves the conflict between (a) the closure's purely-additive
        // implicit defaults and (b) explicit FD&R decisions on the
        // post-closure state. Per
        // `docs/plans/2026-05-01-lattice-design.md` §3 (e) supersession
        // is a separate layer that runs after closure converges, not a
        // suppressor inside the closure loop.
        //
        // Pipeline order post-#704:
        //
        //   1. join_via_lattice (existing per-axis overlays)
        //   2. closure (purely additive Kleene fixpoint)
        //   3. apply_supersession_overlays (THIS step — §H.8 p145 strip)
        //   4. PageRewrites (declarative catalog below)
        //
        // The overlay is idempotent; `apply_closed_bits_to` already
        // strips dominated controls when NOFORN is in the Kleene delta
        // (closure added NOFORN), but does nothing when NOFORN was in
        // the input and closure added e.g. RELIDO or REL_TO_USA. The
        // overlay closes that gap unconditionally and is a no-op when
        // there is nothing to strip.
        Self::apply_supersession_overlays(&mut out.0);

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
        //
        // Per-page eligibility mask (CO-2): `page_mask` is a bitmask
        // where bit `1 << cat.0` is set when category `cat` has at
        // least one value in `out`. Before evaluating each row's trigger
        // we check whether the trigger's required category is present;
        // if not, the trigger can't fire and we skip the row entirely.
        //
        // Mask update policy:
        // - `Contains(X, _)` / `Custom` eligibility: bits are OR-ed in
        //   for each fired rewrite's declared write axes, so downstream
        //   rows that depend on newly-written axes are correctly eligible
        //   even if the category was empty at mask-init time.
        // - `Empty(X)` eligibility: after a `Clear` action fires on
        //   category X, the bit for X is cleared if the category is now
        //   empty per `capco_category_has_values`. Without this step a
        //   downstream `Empty { category: X }` trigger would be skipped
        //   because the bit still reads as set (monotone-only policy
        //   would miss cleared categories), so the trigger could never
        //   fire even though the category is now empty.
        let mut page_mask = capco_axis_mask(&out);
        for rw in &self.page_rewrites {
            // Eligibility pre-check: skip rows whose trigger category is
            // absent from the page mask to avoid unnecessary predicate
            // evaluations.
            let eligible = match &rw.trigger {
                // `Contains(X, _)` fires only if X is non-empty.
                CategoryPredicate::Contains { category, .. } => {
                    page_mask & (1u64 << category.0) != 0
                }
                // `Empty(X)` fires only if X IS empty (bit clear).
                CategoryPredicate::Empty { category } => page_mask & (1u64 << category.0) == 0,
                // `Custom(f)` — use the declared reads ∪ writes axes as
                // a conservative proxy. Skip only when every declared
                // axis is absent: if all inputs and outputs are empty,
                // the predicate cannot observe or produce relevant state.
                // `Custom` rewrites with empty reads ∪ writes are
                // scheduler-rejected (`UnannotatedCustomAxes`), so the
                // empty-intersection case always corresponds to a page
                // where none of the declared axes are present.
                CategoryPredicate::Custom(_) => rw
                    .reads
                    .iter()
                    .chain(rw.writes.iter())
                    .any(|c| page_mask & (1u64 << c.0) != 0),
            };
            if !eligible {
                continue;
            }
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
                // Update the mask monotonically: OR in declared write axes
                // so downstream rows that read these axes are correctly
                // eligible even if the category was empty at mask-init time.
                for w in rw.writes {
                    page_mask |= 1u64 << w.0;
                }
                match &rw.action {
                    CategoryAction::Clear { category } => {
                        tracing::debug!(
                            rewrite_id = rw.id,
                            action = "Clear",
                            ?category,
                            "PageRewrite fired",
                        );
                        capco_category_clear(&mut out, *category);
                        // After clearing, update the mask: if the category
                        // is now empty, clear its bit so that downstream
                        // `Empty { category }` triggers become eligible.
                        if !capco_category_has_values(&out, *category) {
                            page_mask &= !(1u64 << category.0);
                        }
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
}
