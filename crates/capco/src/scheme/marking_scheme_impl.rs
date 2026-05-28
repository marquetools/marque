// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `impl MarkingScheme for CapcoScheme` — the 22-method trait body.
//!
//! Reaches helpers via `super::actions::*` / `super::predicates::*`
//! plus explicit named imports of scheme-internal symbols
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

// HOT-1 axis-flag scaffolding retired in the FactBitmask
// refactor (issue #371). The structural `ClosureAxisFlags` snapshot
// + `working_has_caveated_dissem_trigger` + `capco_rule_axis_present`
// per-rule guard were replaced by the single `ALL_TRIGGER_MASK`
// short-circuit on the bitmask projection at the top of
// `CapcoScheme::closure` below — branchless, no per-axis fan-out, no
// per-rule guard table to keep in sync with `CAPCO_CLOSURE_RULES`
// catalog order.

// `satisfies` and `evaluate_custom` are implemented on `CapcoScheme`,
// so calling `marque_scheme::constraint::evaluate(&CapcoScheme::new(),
// &m)` (or equivalently `scheme.validate(&m)` via the trait default)
// fires every dyadic and Custom constraint in the catalog.
//
// Declarative rules dispatch through the engine's scheme-adapter bridge
// (`crate::scheme::adapter`), which synthesizes fixes and messages via
// `CapcoScheme::fix_intent_by_name` / `message_by_name` (not the
// trait-path `validate`) for byte-identical message/span/fix output.
impl MarkingScheme for CapcoScheme {
    type Token = marque_scheme::TokenId;
    type Marking = CapcoMarking;
    type ParseError = CapcoParseError;
    type OpenVocabRef = CapcoOpenVocabRef;

    type Parsed<'src> = ParsedAttrs<'src>;
    type Canonical = CanonicalAttrs;

    /// CAPCO/ISM canonicalization — collapse the borrowed
    /// `ParsedAttrs<'src>` produced by `marque-core`'s strict parser
    /// to the owned `CanonicalAttrs` form rules consume.
    ///
    /// **Structural rename only.** Every field is moved across without
    /// transformation — no case folding, no deprecated-token migration,
    /// no canonicalization. This is the only shape CapcoScheme needs;
    /// future schemes (CUI / NATO) that fold or migrate fields override
    /// this method with their own logic.
    ///
    /// `&self` is unused today, but the trait signature reserves it
    /// for stateful future schemes.
    ///
    /// **Sole-path invariant**: this is the only public
    /// `ParsedAttrs → CanonicalAttrs` route for production code.
    /// `ParsedAttrs` and `CanonicalAttrs` are not `#[non_exhaustive]`,
    /// so destructure-and-literal construction outside `marque-ism` lets
    /// this body live in the scheme adapter, where it belongs.
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
            // Preserve the parser-side attribution. The attribution
            // function lives on the `ParsedAttrs` side; this canonicalize
            // impl is a pure structural rename and must not re-run it.
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

        // Invariant insurance. `attribute_dissems` is the single source
        // of truth; this debug-only assertion catches a future bug where
        // attribution is skipped or the canonical adapter is fed a
        // hand-built `ParsedAttrs` with both fields
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
        // The trait impl exists to validate the abstraction's shape
        // against CAPCO; callers use `marque_core::Parser` directly. The
        // engine ties parse() in once the ambiguity resolver lands.
        Err(CapcoParseError::NotImplemented)
    }

    /// Resolve a [`TokenRef`] against a `CapcoMarking`'s concrete
    /// storage. Drives the dyadic-variant arms of
    /// [`marque_scheme::constraint::evaluate`].
    ///
    /// **Token-presence semantics**:
    /// - [`TokenRef::Token(id)`] returns true when the marking carries
    ///   the named token *anywhere* relevant — `TOK_USA` ⇒ "USA in
    ///   REL TO" (the dissemination context), `TOK_RD` ⇒ "RD anywhere
    ///   in `aea_markings`", etc. The mapping is per-sentinel and
    ///   documented inline below.
    /// - [`TokenRef::AnyInCategory(cat)`] returns true when the
    ///   category has at least one populated value. `CAT_DISSEM`
    ///   intentionally counts both the dissem axis (`dissem_us` and
    ///   `dissem_nato` together, walked via `attrs.dissem_iter()`) AND
    ///   `rel_to` as dissem-flavored presence ("non-US classification
    ///   needs SOME dissem").
    ///
    /// Sentinel `TokenId`s not used by the current catalog
    /// (`TOK_IC_DISSEM`, `TOK_NON_IC_DISSEM`) fall through to `false`
    /// — they remain declared for future consumption. Categories not
    /// listed (none today) likewise fall through.
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
                // Open-vocab REL TO country codes route to CAT_REL_TO so
                // the REL TO `FactAdd { CountryCode, Portion }` intents
                // land on the same axis as the closed-CVE `TOK_USA` /
                // `TOK_REL_TO` sentinels used by FactRemove paths.
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
    /// name→helper router shared with the engine-bridge fast path.
    fn evaluate_custom(
        &self,
        name: &'static str,
        marking: &Self::Marking,
        bits: marque_scheme::FactBitmask,
    ) -> Vec<ConstraintViolation> {
        evaluate_custom_by_attrs(&marking.0, bits, name)
    }

    /// Project with decision-tracing instrumentation.
    ///
    /// Overrides the [`MarkingScheme::project_with_sink`] default to
    /// emit per-stage [`marque_scheme::DecisionEvent`]s as the
    /// [`Self::project_attrs_pipeline`] traverses its stages
    /// (`close` → `apply_default_fill` → `apply_supersession_overlays`
    /// → page rewrites). The default delegation would call
    /// [`Self::project`] and emit nothing; this override delegates to
    /// the instrumented variant [`Self::project_attrs_pipeline_with_sink`].
    ///
    /// # Step coordination
    ///
    /// The trait signature receives `&mut dyn DecisionSink` directly
    /// (the engine's per-document step counter is not visible here).
    /// Events emitted within this call use a **local step counter**
    /// starting at `0`; the engine wraps the sink it passes in a
    /// `StepRemappingSink` adapter (`Engine::with_remapping_sink`)
    /// that mints a fresh global step for each scheme-emitted event
    /// and rewrites `triggered_by` references through a per-call
    /// `local → global` map, so cascade chains across the
    /// scheme/engine boundary stay sound after merging into the
    /// engine's stream.
    ///
    /// Only compiled under the `decision-tracing` feature; without it,
    /// the default trait impl (delegating to [`Self::project`]) is
    /// active and OFF-mode behavior is byte-identical to a build that
    /// never declared the feature.
    #[cfg(feature = "decision-tracing")]
    fn project_with_sink(
        &self,
        scope: Scope,
        markings: &[Self::Marking],
        sink: &mut dyn marque_scheme::DecisionSink,
    ) -> Self::Marking {
        match scope {
            Scope::Portion => self.project(scope, markings),
            Scope::Page | Scope::Document | Scope::Diff => {
                let raw: Vec<CanonicalAttrs> = markings.iter().map(|m| m.0.clone()).collect();
                let out_attrs = self.project_attrs_pipeline_with_sink(&raw, sink);
                CapcoMarking::new(out_attrs)
            }
        }
    }

    /// Closure with decision-tracing instrumentation.
    ///
    /// Overrides the [`MarkingScheme::closure_with_sink`] default to
    /// diff the pre-close vs. post-close bitmask and emit one
    /// [`marque_scheme::DecisionEvent`] per cone bit added by
    /// `close()`. Events carry
    /// [`marque_scheme::DecisionSource::Closure(row_name)`] where
    /// `row_name` comes from [`crate::scheme::closure_table::bit_to_row_name`]
    /// (first-match-by-catalog-order attribution).
    ///
    /// See [`Self::project_with_sink`] for step-coordination notes —
    /// the same local-counter semantics apply.
    ///
    /// Only compiled under the `decision-tracing` feature; without it,
    /// the default trait impl (delegating to [`Self::closure`]) is
    /// active.
    #[cfg(feature = "decision-tracing")]
    fn closure_with_sink(
        &self,
        marking: Self::Marking,
        sink: &mut dyn marque_scheme::DecisionSink,
    ) -> Self::Marking {
        use crate::fact_bitmask::derive_bits;
        use crate::scheme::closure_table::bit_to_row_name;

        let input_bits = derive_bits(&marking.0).bits();
        let closed_marking = self.closure(marking);
        let closed_bits = derive_bits(&closed_marking.0).bits();

        // Emit one ClosureFired event per 0→1 flipped cone bit.
        // Local step counter (see method doc comment).
        let mut local_step: u32 = 0;
        let delta = closed_bits & !input_bits;
        if delta != 0 {
            let mut remaining = delta;
            while remaining != 0 {
                let bit_index = remaining.trailing_zeros();
                remaining &= remaining - 1;
                if let Some(row_name) = bit_to_row_name(bit_index) {
                    let step = local_step;
                    local_step = local_step.saturating_add(1);
                    sink.record(marque_scheme::DecisionEvent {
                        step,
                        site: marque_scheme::DecisionSite::Page(0),
                        category: marque_scheme::CategoryId::MARKING,
                        kind: marque_scheme::DecisionKind::ClosureFired,
                        source: marque_scheme::DecisionSource::Closure(row_name),
                        triggered_by: None,
                    });
                }
            }
        }

        closed_marking
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
                // The production page projection runs the lattice
                // pipeline:
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
                // layer (unlike `closure`'s Kleene fixpoint).
                //
                // The trait path pays the
                // `markings.iter().map(|m| m.0.clone()).collect()`
                // clone round because the trait's `markings:
                // &[CapcoMarking]` slice ties us to per-portion
                // CapcoMarking values. The engine's hot path bypasses
                // this via `CapcoScheme::project_from_attrs_slice`,
                // which consumes the engine accumulator's slice directly
                // and delegates to the same shared
                // `project_attrs_pipeline` body — without paying the
                // trait wrap-then-unwrap round. Test fixtures and
                // external tooling continue to use this trait-path entry.
                let raw: Vec<CanonicalAttrs> = markings.iter().map(|m| m.0.clone()).collect();
                let out_attrs = self.project_attrs_pipeline(&raw);
                CapcoMarking::new(out_attrs)
            }
        }
    }

    fn page_rewrites(&self) -> &[PageRewrite<Self>] {
        &self.page_rewrites
    }

    /// Substantive `render_canonical` body driven by the per-axis
    /// dispatch table [`RENDER_TABLE`].
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
        // The body dispatches on `ctx.scope`. `ctx.emission_form` and
        // `ctx.schema_version` are plumbed through but NOT yet consumed
        // by the per-axis renderers (a future change will land the
        // §G.1 Table 4 dispatch body). Corpus regression is the
        // byte-identity gate.
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
        // Override retained for the byte-identity gate
        // (`render_canonical_default_chain.rs`). `render_canonical` is
        // the substantive renderer; this override delegates to it
        // through the trait-default String round-trip. Removing the
        // override is a follow-up once the engine call sites move off
        // `render_portion` to `render_canonical`.
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
        // Construct an `Auto + MarqueMvp3` RenderContext; a future
        // change will land the §G.1 Table 4 dispatch body.
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
        // Construct an `Auto + MarqueMvp3` RenderContext.
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
    /// This is the scheme-layer hook required because
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
    /// Post-#704, CAPCO's closure operator executes as a bitwise
    /// Kleene fixpoint over
    /// [`CLOSURE_TABLE`](super::closure_table::CLOSURE_TABLE) — a
    /// 6-row `(trigger_mask, cone_mask)` catalog over a `u128`
    /// atom bitmask. The six rows are the per-marking unconditional
    /// implications from §H.4 marking templates (HCS-O / HCS-P[sub]
    /// → NOFORN + ORCON per §H.4 p64 / p68; SI-G → ORCON per §H.4
    /// p80; TK-{BLFH, IDIT, KAND} → NOFORN per §H.4 p87 / p91 /
    /// p95). All six fire unconditionally with no `suppressor_mask`
    /// gating (the pre-#704 `suppressor_mask` field retired in
    /// issue #704 — see the `closure_table.rs` module doc-comment
    /// for the algebraic-monotonicity rationale).
    ///
    /// **Post-#704: the fn-pointer slice this trait method returns
    /// is empty (`&[]`)**. The pre-#704 residual fn-pointer rule
    /// `CLOSURE_REL_TO_USA_NATO` (Row 7's open-vocab NATO
    /// tetragraph tail) retired alongside Rows 0/7/8/9 to
    /// [`crate::scheme::default_fill`] (`row7_should_fill` and
    /// sibling predicates). The fn-pointer surface is reserved as
    /// a stable seam for any future CAPCO closure rule that ships
    /// an open-vocab cone which cannot project onto a closed-vocab
    /// bit; today there are no such rules.
    ///
    /// Why Rows 0/7/8/9 retired: those four are "default if absent"
    /// rules per §B.3 paragraph b p19's "NOT MARKED PREVIOUSLY"
    /// gate — inherently non-monotone by §-spec design and
    /// therefore unable to live in a closure operator that honors
    /// the [`MarkingScheme::closure`] monotone contract. The §H.8
    /// p145 NOFORN-dominates / §B.3.a p19 FD&R supersession
    /// semantics they previously encoded now split two ways:
    /// the "default if absent" half moved to
    /// [`crate::scheme::default_fill::apply_default_fill`]; the
    /// "post-close re-apply per-axis supersession" half moved to
    /// [`CapcoScheme::apply_supersession_overlays`]. The
    /// [`super::closure::FDR_DOMINATORS`] slice remains the
    /// canonical FD&R enumeration consumed by
    /// `Vocabulary::is_fdr_dissem` and the supersession overlays.
    ///
    /// All bitmask rows ship at [`Severity::Info`]: closure firings are
    /// silent lattice-layer fact propagation, not byte-level fixes.
    /// User-visible byte diffs ride on independent `Severity::Suggest`
    /// text-layer rules (e.g., `BareNatoRequiresRelToRule` for the NATO
    /// row).
    ///
    /// Scheme-agnostic discovery should use
    /// [`Self::closure_inventory()`], which projects the bitmask
    /// `CLOSURE_TABLE` rows onto `ClosureRuleMetadata` for unified
    /// downstream consumption.
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
                    // `display_label: &'static str` is human-facing UI
                    // text; `label: Citation` is the typed
                    // authoritative-source anchor. Metadata reads from
                    // each field directly — no string-form display of the
                    // citation, which would need runtime formatting.
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
    /// [`CLOSURE_TABLE`](super::closure_table::CLOSURE_TABLE).
    ///
    /// Implements implicit-fact propagation (the algebraic closure
    /// operator).
    ///
    /// # Algorithm
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
    ///    is set at iteration 0. Post-#704 the trigger mask covers
    ///    the six SCI per-marking sentinel bits only; non-SCI inputs
    ///    hit this short-circuit.
    /// 3. **Kleene fixpoint**: [`close`] runs a bitwise loop over
    ///    the 6-row [`CLOSURE_TABLE`] — for each row, if
    ///    `(next & trigger_mask) != 0`, OR `cone_mask` into `next`.
    ///    Post-#704 the operator is purely additive — no per-row
    ///    suppressor gate — so each row's firing predicate is the
    ///    upward-closed presence check on its trigger atom. Iterate
    ///    until stable or panic at [`MAX_CLOSURE_ITERATIONS`] (= 16).
    ///    Post-#704 the catalog's longest causal chain is depth 1
    ///    (each Row 1-6 fires at most once on its trigger atom; the
    ///    pre-#704 SI-G → ORCON → Trio-1-NOFORN depth-2 chain
    ///    crosses the close()/default_fill boundary in the full
    ///    pipeline). Typical inputs converge in 1 iteration.
    /// 4. **Write-back**: [`apply_closed_bits_to`] materializes every
    ///    new bit in `closed_bits & !input_bits` to the corresponding
    ///    `CanonicalAttrs` axis (`dissem_us` push, etc.). Post-#704
    ///    only the dissem-axis writeback fires — the surviving rows
    ///    emit `NOFORN` and `ORCON` cone bits, both of which route
    ///    through the dissem-axis arm.
    ///
    /// (Post-#704: there is no step 5. The pre-#704 Row 7 open-vocab
    /// NATO tetragraph tail — which invoked the retired fn-pointer
    /// rule `CLOSURE_REL_TO_USA_NATO`'s `cone_derived` via
    /// `apply_closure_fact` — relocated to
    /// [`crate::scheme::default_fill::apply_default_fill`] alongside
    /// Row 7's trigger. `closure()` no longer touches `rel_to`;
    /// `apply_default_fill` writes `CountryCode::USA` and
    /// `CountryCode::NATO` directly when its Row-7 predicate fires.
    /// `apply_default_fill` is called separately by
    /// `project_attrs_pipeline`, NOT by `closure()`.)
    ///
    /// # Invariants preserved
    ///
    /// 1. **Extensive**: `closure(m) ⊒ m` — `close` only OR-s cone
    ///    bits; `apply_closed_bits_to` is a pure-additive projector.
    /// 2. **Idempotent**: `closure(closure(m)) == closure(m)` — the
    ///    bitmask Kleene loop runs to fixpoint, and `apply_closed_bits_to`
    ///    is a no-op for bits already present in the input.
    /// 3. **Monotone**: `m1 ⊑ m2 ⟹ closure(m1) ⊑ closure(m2)` —
    ///    post-#704 every row fires by a pure presence check
    ///    `(working & trigger_mask) != 0` and writes only via `|=`
    ///    on `cone_mask`. No row has a suppressor; the firing
    ///    predicate is upward-closed in `working` and the body is
    ///    purely additive. Bitmask regressions are pinned by
    ///    `proptest_closure_table.rs` (P1–P4 algebraic properties)
    ///    and the
    ///    [`CLOSURE_TABLE`](super::closure_table::CLOSURE_TABLE)
    ///    positional pin in `post_4b_lattice_inventory_pin.rs`.
    ///
    /// # Non-convergence
    ///
    /// Per the [`MarkingScheme::closure`] trait contract, exceeding
    /// `MAX_CLOSURE_ITERATIONS` panics. Post-#704 the 6-row catalog
    /// has max causal depth 1 (no row's cone is in any other row's
    /// trigger mask); non-convergence is unreachable on the current
    /// catalog. Reaching the panic branch means a future row added
    /// a cycle — a catalog regression. [`close`] panics
    /// unconditionally on non-convergence (release builds included)
    /// per the documented contract.
    fn closure(&self, marking: Self::Marking) -> Self::Marking {
        // Bitmask Kleene fast path (issue #371, PR-D; issue #704 refinement).
        //
        // Post-#704: CLOSURE_TABLE carries the 6 per-marking unconditional
        // implication rows (HCS-O / HCS-P[sub] / SI-G / TK-{BLFH,IDIT,KAND})
        // from §H.4 marking templates. The 4 "default if absent" rules
        // that pre-#704 occupied Rows 0/7/8/9 (caveated→NOFORN,
        // NATO→REL TO USA, SCI→RELIDO, US-class→RELIDO) relocated to
        // `crate::scheme::default_fill::apply_default_fill`, which runs
        // in `project_attrs_pipeline` AFTER this closure() converges.
        // Splitting the monotone Rows 1-6 from the non-monotone Rows
        // 0/7/8/9 lets close() honor the `MarkingScheme::closure` trait
        // contract's algebraic monotonicity property.
        //
        // # Cost shape
        //
        // - HOT-1: `derive_bits` is single-pass + branchless; the
        //   `ALL_TRIGGER_MASK` short-circuit gates everything else.
        //   The post-#704 mask covers the six SCI sentinel bits only.
        // - Kleene loop: bitwise AND/OR/cmp on a `u128` per row per
        //   iteration, capped at `MAX_CLOSURE_ITERATIONS` (= 16).
        //   Post-#704 the longest causal chain is depth 1 (each Row
        //   1-6 fires at most once on its trigger atom).
        // - `apply_closed_bits_to`: O(set bits in delta). Per-marking
        //   cones touch 1-2 atoms (NOFORN, ORCON).
        //
        // # Trait contract
        //
        // The bitmask `close()` panics unconditionally on non-convergence
        // per the `MarkingScheme::closure` trait contract — see
        // `closure_table::close` doc-comment for the panic semantics.

        use crate::fact_bitmask::{apply_closed_bits_to, derive_bits};
        use crate::scheme::closure_table::{ALL_TRIGGER_MASK, close};

        let input_bits = derive_bits(&marking.0);

        // HOT-1: pre-Kleene short-circuit. If no trigger fires on the
        // input, no row can fire across any iteration (close is extensive,
        // bits are only added). Return the input verbatim.
        if (input_bits.bits() & ALL_TRIGGER_MASK) == 0 {
            return marking;
        }

        let closed_bits = close(input_bits);

        let mut working = marking;
        apply_closed_bits_to(&mut working.0, closed_bits, input_bits);

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
    /// row still returns `true` here, matching the fn-pointer
    /// walker's behavior.
    ///
    /// [`ALL_TRIGGER_MASK`]: crate::scheme::closure_table::ALL_TRIGGER_MASK
    #[cfg(test)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub(crate) fn any_closure_trigger_fires(&self, marking: &CapcoMarking) -> bool {
        use crate::fact_bitmask::derive_bits;
        use crate::scheme::closure_table::ALL_TRIGGER_MASK;
        let _ = self;
        (derive_bits(&marking.0).bits() & ALL_TRIGGER_MASK) != 0
    }
    /// Engine-facing hot-path entry that consumes a pre-built per-page
    /// slice of portion attributes directly. The engine owns the
    /// accumulator that grows portions across the document; this entry
    /// forwards the slice to the shared [`Self::project_attrs_pipeline`]
    /// body, skipping the trait-path's
    /// `Vec<CapcoMarking> → Vec<CanonicalAttrs>` wrap-then-unwrap round.
    ///
    /// The trait-level [`MarkingScheme::project`] entry handles
    /// `&[Self::Marking]` callers — test fixtures and external
    /// tooling — and pays one `.0.clone()` per portion to bridge into
    /// the same `project_attrs_pipeline`. The `&[CanonicalAttrs]`
    /// parameter lets the engine caller avoid constructing an
    /// intermediate accumulator type on the hot path.
    ///
    /// ## Same-slice property
    ///
    /// `raw` flows directly to `project_attrs_pipeline`. There is no
    /// parallel slice the inner pipeline could drift from. Future
    /// maintenance that reintroduces a parallel derivation path MUST
    /// re-add the contract at the new fork — the invariant lives in this
    /// doc-comment, not in a runtime check.
    pub fn project_from_attrs_slice(&self, raw: &[CanonicalAttrs]) -> CanonicalAttrs {
        self.project_attrs_pipeline(raw)
    }

    /// Sink-aware variant of [`Self::project_from_attrs_slice`] used
    /// by the engine when the `decision-tracing` feature is on.
    ///
    /// Delegates to [`Self::project_attrs_pipeline_with_sink`] which
    /// emits per-stage [`marque_scheme::DecisionEvent`]s for closure
    /// fires, default-fill fires, supersession-overlay mutations,
    /// and page-rewrite fan-outs. Engine callers reach this entry
    /// when the engine's per-document sink is non-`NoopSink`; the
    /// off-feature build never compiles this surface (the engine
    /// gates the call site under `#[cfg(feature = "decision-tracing")]`).
    #[cfg(feature = "decision-tracing")]
    pub fn project_from_attrs_slice_with_sink(
        &self,
        raw: &[CanonicalAttrs],
        sink: &mut dyn marque_scheme::DecisionSink,
    ) -> CanonicalAttrs {
        self.project_attrs_pipeline_with_sink(raw, sink)
    }

    /// Issue #704 — re-apply per-axis supersession overlays to the
    /// post-`close()` / post-`apply_default_fill` CanonicalAttrs.
    ///
    /// Runs as part of [`Self::project_attrs_pipeline`] between
    /// `apply_default_fill` and the declarative `PageRewrites`, in two
    /// steps:
    ///
    /// 1. **Dissem-axis overlay re-application (unconditional)** —
    ///    calls
    ///    [`crate::lattice::dissem::DissemSet::with_all_overlays_reapplied`]
    ///    which runs all three `DissemSet` overlays:
    ///    - Overlay 1: OC > OC-USGOV supersession (§H.8 p140 +
    ///      §H.8 p136). NOFORN-independent.
    ///    - Overlay 2: RELIDO observed-unanimity (§H.8 pp155-156).
    ///      NOFORN-independent.
    ///    - Overlay 3: NOFORN-dominates supersession (§H.8 p145 +
    ///      §B.3.a p19 + §D.2 Table 3 rows 1-2). NOFORN-dependent.
    ///
    ///    This step was formerly gated on `has_noforn` because it
    ///    was framed as just the §H.8 p145 strip. The misframing
    ///    silently disabled Overlays 1 + 2 for inputs whose
    ///    close() / default_fill outputs would have triggered them.
    ///    The step now runs unconditionally; the
    ///    `has_noforn` gate moved to Step 2 (the only NOFORN-
    ///    dependent action).
    ///
    /// 2. **REL TO + DISPLAY ONLY country-list clear (NOFORN-gated)** —
    ///    §H.8 p145 country-list strip: if `Nf` ended up in
    ///    `attrs.dissem_us` after Step 1, clear `attrs.rel_to` and
    ///    `attrs.display_only_to`. §H.8 p145 is symmetric across
    ///    the dissem-axis tokens and the country-list axes — both
    ///    sides of NOFORN's mutual exclusion list must be evicted.
    ///    Mirrors
    ///    [`crate::lattice::rel_to::RelToBlock::with_nato_implicit_stripped`]
    ///    at the CanonicalAttrs boundary.
    ///
    /// Idempotent: rerunning observes the post-overlay state.
    /// Step 1 is idempotent because each overlay strips strictly
    /// (a second pass finds nothing to strip); Step 2 is
    /// idempotent because cleared axes stay cleared.
    ///
    /// Authority: §H.8 p140 + §H.8 p136 (OC > OC-USGOV — Overlay 1);
    /// §H.8 pp155-156 (RELIDO observed-unanimity — Overlay 2);
    /// §H.8 p145 (NOFORN: "Cannot be used with REL TO, RELIDO, EYES
    /// ONLY, or DISPLAY ONLY") + §B.3.a p19 (FD&R dominator
    /// enumeration) + §D.2 Table 3 rows 1-2 (NOFORN dominates
    /// dominated FD&R at banner roll-up) — Overlay 3 + Step 2.
    fn apply_supersession_overlays(attrs: &mut CanonicalAttrs) {
        use crate::lattice::DissemSet;
        use marque_ism::DissemControl;

        // Step 1 — dissem-axis overlay re-application.
        //
        // **Runs unconditionally** (#704). This step was formerly
        // gated on `has_noforn` because it was framed as
        // "the §H.8 p145 NOFORN-dominates strip." Structural review
        // caught the misframing: `DissemSet`
        // carries THREE overlays (`apply_overlays` →
        // `with_all_overlays_reapplied`), only one of which is
        // NOFORN-dependent:
        //
        // - Overlay 1: OC > OC-USGOV supersession (§H.8 p140 +
        //   §H.8 p136). NOFORN-independent: fires when both `Oc`
        //   and `OcUsgov` are present in the post-close /
        //   post-default-fill dissem state, regardless of NOFORN.
        // - Overlay 2: RELIDO observed-unanimity (§H.8 pp155-156).
        //   NOFORN-independent: drops `Relido` when not all
        //   contributing portions carried it.
        // - Overlay 3: NOFORN-dominates (§H.8 p145 + §B.3.a p19
        //   + §D.2 Table 3 rows 1-2). NOFORN-dependent: only
        //   fires when `Nf` is in the set.
        //
        // The former `has_noforn` gate prevented Overlays 1 + 2
        // from re-running when close() / default_fill added an
        // Overlay-1- or Overlay-2-relevant bit that interacts with
        // input bits. No production CAPCO row exposes the gap
        // today (the gap is structural, not observable on the
        // current 6-row CLOSURE_TABLE + 4-row default-fill
        // catalog), but the defensive guard prevents future
        // catalog edits from regressing the §H.8 p140 / pp155-156
        // contracts.
        //
        // The unconditional rebuild cost is bounded by
        // `attrs.dissem_us.len()` (typically 1-3 tokens) plus the
        // `from_attrs_iter` BTreeSet round-trip — cheap enough
        // that the defensive guard pays for itself even on the
        // common (no-Nf) case.
        let view = DissemSet::from_attrs_iter(std::slice::from_ref(attrs));
        let rebuilt = view.with_all_overlays_reapplied();
        let next = rebuilt.into_boxed_slice();
        if next[..] != attrs.dissem_us[..] {
            attrs.dissem_us = next;
        }

        // Step 2 — country-list clear. NOFORN-conditional per
        // §H.8 p145: "NOFORN cannot be used with REL TO, RELIDO,
        // EYES ONLY, or DISPLAY ONLY". When NOFORN is present in
        // the post-overlay dissem state, `rel_to` and
        // `display_only_to` MUST be cleared. This is the only step
        // that gates on `has_noforn` — Overlays 1 + 2 in Step 1
        // are NOFORN-independent and ran unconditionally.
        //
        // Re-read `has_noforn` AFTER Step 1 because Step 1's
        // overlay rebuild does not remove NOFORN (it only removes
        // DOMINATED controls; NOFORN itself is the dominator that
        // survives). Re-read is defensive: a future overlay edit
        // that did remove NOFORN under some condition would be
        // observable here.
        let has_noforn = attrs.dissem_us.contains(&DissemControl::Nf);
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
    /// identical across all surfaces. Pipeline:
    ///
    /// ```text
    /// join_via_lattice → closure → PageRewrites
    /// ```
    ///
    /// The pipeline consumes only `raw: &[CanonicalAttrs]` — there is no
    /// `PageContext` parameter and no parallel slice for the inner body
    /// to drift from. Engine callers reach this pipeline via
    /// [`Self::project_from_attrs_slice`].
    fn project_attrs_pipeline(&self, raw: &[CanonicalAttrs]) -> CanonicalAttrs {
        // Closure-rewrite-application sentinel: the closure operator
        // MUST NOT mutate the per-portion CanonicalAttrs slice it
        // observes (read-only-attrs invariant). Snapshot the input
        // pre-closure and assert byte-identity afterward.
        //
        // ## Audit content-ignorance (Constitution V)
        //
        // The failure path emits ONLY counts and the §-citation
        // literal — never `raw` / `raw_snapshot` content.
        // `debug_assert_eq!`'s default `{:?}` format would dump full
        // `CanonicalAttrs` (token values, country lists, spans), leaking
        // document content. The explicit `if !=` + `panic!` with a
        // count-only message mirrors the `check_portions_unchanged`
        // pattern in `crates/engine/src/engine.rs`. Both sentinels
        // enforce the same read-only-attrs invariant; both must keep
        // audit content-ignorance on the failure path.
        #[cfg(debug_assertions)]
        let raw_snapshot: Vec<CanonicalAttrs> = raw.to_vec();

        let joined = CapcoMarking::new(CapcoMarking::join_via_lattice(raw));
        let mut out = self.closure(joined);

        // Issue #704 — pre-supersession default-fill stage.
        //
        // The `CapcoScheme::closure` operator is purely additive (Kleene
        // fixpoint over `CLOSURE_TABLE`; the `suppressor_mask` gate that
        // previously prevented Trio 1 / Trio 2 / Trio 3 cones from firing
        // when an FD&R dominator was already present was retired because
        // it broke the closure operator's algebraic monotonicity property
        // `a ⊑ b ⟹ Cl(a) ⊑ Cl(b)`).
        //
        // The §B.3 paragraph b p19 "NOT MARKED PREVIOUSLY" / §B.3
        // Table 2 p21 "default if absent" / §H.7 p127 NATO-default /
        // §H.8 p154 RELIDO-default rules that the suppressors encoded
        // are **inherently non-monotone** — they fire only when the
        // input lacks explicit FD&R. They live in their own post-close
        // stage (`apply_default_fill`) so close()'s monotone contract
        // is honored while the default-if-absent semantics are
        // faithfully reproduced.
        //
        // Pipeline order:
        //
        //   1. join_via_lattice (existing per-axis overlays)
        //   2. closure (purely additive Kleene fixpoint over Rows 1-6)
        //   3. apply_default_fill (Rows 0/7/8/9 — §B.3.b/§H.7/§H.8
        //      "default if absent" rules; non-monotone by §-design)
        //   4. apply_supersession_overlays (§H.8 p145 strip for
        //      input-explicit-NOFORN-vs-REL-TO contradictions)
        //   5. PageRewrites (declarative catalog below)
        //
        // Default-fill MUST run BEFORE the supersession overlay: when
        // input has `{S, ORCON}`, default-fill Row 0 adds NOFORN; the
        // supersession overlay then has no work to do (NOFORN-vs-REL-TO
        // contradiction only arises when input has BOTH explicit).
        // When input has `{S, NOFORN, REL_TO_USA}` (user-explicit
        // contradiction), default-fill skips Row 9 (REL_TO_PRESENT in
        // gate); the supersession overlay then strips REL TO per §H.8
        // p145.
        crate::scheme::default_fill::apply_default_fill(&mut out.0);

        // Issue #704 — FD&R supersession overlay (post-default-fill).
        //
        // Narrowed scope post-#704 refinement: the overlay now ONLY
        // handles input-explicit FD&R contradictions per §H.8 p145
        // ("NOFORN ... Cannot be used with REL TO, RELIDO, EYES ONLY,
        // or DISPLAY ONLY"). The default-if-absent semantics moved to
        // `apply_default_fill` above; the overlay strips dominated
        // controls when NOFORN ended up coexisting with REL TO /
        // DISPLAY ONLY / EYES / RELIDO on the marking (input-explicit
        // case the closure operator cannot prevent without violating
        // monotonicity).
        //
        // `apply_closed_bits_to` already strips dominated controls
        // when NOFORN is in the Kleene delta (closure-added NOFORN
        // via Rows 1-6 — HCS-O / HCS-P[sub] / TK-BLFH/IDIT/KAND, all
        // of which carry NOFORN in their cone). The overlay closes
        // the gap when NOFORN was in the input.
        Self::apply_supersession_overlays(&mut out.0);

        #[cfg(debug_assertions)]
        {
            if raw != raw_snapshot.as_slice() {
                panic!(
                    "closure() mutated the per-portion CanonicalAttrs slice \
                     ({} portion(s) before vs {} after) — violates the \
                     PageRewrite read-only-attrs invariant",
                    raw_snapshot.len(),
                    raw.len(),
                );
            }
        }

        // Apply declarative page rewrites. Page rewrites run on the
        // post-closure state, so any cone facts the closure operator
        // added are visible to rewrite triggers. NOFORN-clears-REL-TO
        // and similar absorbing rewrites
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
                        // The JOINT-promotion and FGI-absorption rewrites
                        // are declared for the scheduler + catalog
                        // surface, and the engine drives page-marking
                        // aggregation through `scheme.project(Scope::Page,
                        // ...)`, so this arm is reachable at runtime.
                        // `Promote` stays a no-op here because those
                        // rewrites are renderer-canonical territory —
                        // they restate the same fact set in a different
                        // surface form, which is `render_canonical`'s
                        // job, not the projection lattice's. A future
                        // renderer trait surface picks these rewrites up.
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

    /// Sink-aware variant of [`Self::project_attrs_pipeline`] that
    /// emits per-stage [`marque_scheme::DecisionEvent`]s as the
    /// projection pipeline traverses its stages
    /// (`close` → `apply_default_fill` → `apply_supersession_overlays`
    /// → page rewrites).
    ///
    /// # Step coordination
    ///
    /// Uses a **local step counter** starting at `0`. Events emitted
    /// here are correctly ordered + `triggered_by`-linkable WITHIN this
    /// call, but their step IDs do NOT correlate with engine-emitted
    /// events for the same document. This is a Phase D simplification:
    /// the trait signature receives `&mut dyn DecisionSink` directly
    /// (the engine's `next_step` counter is not visible across the
    /// crate boundary), and adding a step-counter parameter would
    /// require revising the Phase B trait surface. Cross-boundary
    /// cascade chains are a Phase D refinement deferral; for the
    /// page-rewrite fan-out demo asset, intra-call linkage between
    /// `RewriteScheduled` and `RewriteApplied` is the load-bearing
    /// property and is preserved by this local counter.
    ///
    /// # Stage events
    ///
    /// 1. **Closure** — diff input vs. post-`close()` bitmask; emit
    ///    one [`marque_scheme::DecisionKind::ClosureFired`] per
    ///    flipped cone bit with
    ///    [`marque_scheme::DecisionSource::Closure(row_name)`] from
    ///    [`crate::scheme::closure_table::bit_to_row_name`].
    /// 2. **Default-fill** — diff post-close vs. post-default-fill
    ///    bitmask; emit one
    ///    [`marque_scheme::DecisionKind::ClosureFired`] per row that
    ///    fired with [`marque_scheme::DecisionSource::DefaultFill(name)`].
    /// 3. **Supersession overlays** — diff dissem/REL-TO axes; emit
    ///    [`marque_scheme::DecisionKind::Mutated`] with
    ///    [`marque_scheme::DecisionSource::Supersession(name)`].
    /// 4. **Page rewrites** — one `RewriteScheduled` per fire and one
    ///    `RewriteApplied` with `triggered_by` pointing at the parent.
    #[cfg(feature = "decision-tracing")]
    fn project_attrs_pipeline_with_sink(
        &self,
        raw: &[CanonicalAttrs],
        sink: &mut dyn marque_scheme::DecisionSink,
    ) -> CanonicalAttrs {
        use crate::fact_bitmask::derive_bits;
        use crate::scheme::closure_table::bit_to_row_name;

        // Local step counter — see method doc-comment for the
        // step-coordination caveat. Within the call,
        // `triggered_by` references resolve into the events emitted
        // here; across the engine boundary they do not correlate.
        let mut local_step: u32 = 0;
        let mut next_step = || {
            let s = local_step;
            local_step = local_step.saturating_add(1);
            s
        };

        #[cfg(debug_assertions)]
        let raw_snapshot: Vec<CanonicalAttrs> = raw.to_vec();

        // Stage 1 — join_via_lattice (no events; the lattice fold is
        // a single algebraic op, not a multi-rule cascade).
        let joined = CapcoMarking::new(CapcoMarking::join_via_lattice(raw));

        // Stage 2 — closure. Capture pre/post bitmasks for diff.
        let pre_close_bits = derive_bits(&joined.0).bits();
        let mut out = self.closure(joined);
        let post_close_bits = derive_bits(&out.0).bits();
        let close_delta = post_close_bits & !pre_close_bits;
        if close_delta != 0 {
            let mut remaining = close_delta;
            while remaining != 0 {
                let bit_index = remaining.trailing_zeros();
                remaining &= remaining - 1;
                if let Some(row_name) = bit_to_row_name(bit_index) {
                    let step = next_step();
                    sink.record(marque_scheme::DecisionEvent {
                        step,
                        site: marque_scheme::DecisionSite::Page(0),
                        category: marque_scheme::CategoryId::MARKING,
                        kind: marque_scheme::DecisionKind::ClosureFired,
                        source: marque_scheme::DecisionSource::Closure(row_name),
                        triggered_by: None,
                    });
                }
            }
        }

        // Stage 3 — apply_default_fill. Diff against post-close
        // bitmask; the four default-fill rows (Row 0 caveated→NOFORN,
        // Row 7 NATO→REL TO USA NATO, Row 8 SCI→RELIDO, Row 9
        // US-class→RELIDO) each map to a specific bit. The mapping
        // is hardcoded because `apply_default_fill` is internal to
        // CAPCO and its 4-row inventory is stable post-#704.
        crate::scheme::default_fill::apply_default_fill(&mut out.0);
        let post_default_fill_bits = derive_bits(&out.0).bits();
        let default_fill_delta = post_default_fill_bits & !post_close_bits;
        if default_fill_delta != 0 {
            // The four default-fill rows that can flip bits in the
            // post-close delta. Attribution is bit-driven (NOFORN
            // came from Row 0; REL_TO_USA came from Row 7; RELIDO
            // came from either Row 8 or Row 9 — discriminated by the
            // post-close bitmask below).
            //
            // Kind taxonomy: default-fill emissions use
            // `DecisionKind::Mutated` rather than `ClosureFired`.
            // The `ClosureFired` variant is reserved for
            // `ClosureRule` firings (the Kleene fixpoint inside
            // `close()`); default-fill is a distinct grammar stage
            // that adds bits when an axis was absent — semantically
            // a mutation, not a closure firing. Per-row attribution
            // still flows through `DecisionSource::DefaultFill(...)`.
            //
            // The `capco:default-fill.*` strings here and the
            // `capco:supersession.*` string further down are
            // **trace identifiers**, not `RuleId` wire strings.
            // They share the `capco:` scheme prefix for grep
            // ergonomics in `marque trace` consumers but they do
            // NOT live in the rule-checker catalog and a §-citation
            // auditor will not find them there — they label
            // implicit grammar stages (`apply_default_fill`,
            // `apply_supersession_overlays`) rather than checker
            // rules. The `DecisionSource::DefaultFill` /
            // `DecisionSource::Supersession` enum variants
            // discriminate them from `RuleId`-shaped sources at
            // runtime.
            use crate::fact_bitmask::fact_bit;
            if (default_fill_delta & (1u128 << fact_bit::NOFORN)) != 0 {
                let step = next_step();
                sink.record(marque_scheme::DecisionEvent {
                    step,
                    site: marque_scheme::DecisionSite::Page(0),
                    category: marque_scheme::CategoryId::MARKING,
                    kind: marque_scheme::DecisionKind::Mutated,
                    source: marque_scheme::DecisionSource::DefaultFill(
                        "capco:default-fill.dissem.caveated-implies-noforn",
                    ),
                    triggered_by: None,
                });
            }
            if (default_fill_delta & (1u128 << fact_bit::REL_TO_USA)) != 0 {
                let step = next_step();
                sink.record(marque_scheme::DecisionEvent {
                    step,
                    site: marque_scheme::DecisionSite::Page(0),
                    category: marque_scheme::CategoryId::MARKING,
                    kind: marque_scheme::DecisionKind::Mutated,
                    source: marque_scheme::DecisionSource::DefaultFill(
                        "capco:default-fill.rel-to.nato-implies-rel-to-usa-nato",
                    ),
                    triggered_by: None,
                });
            }
            if (default_fill_delta & (1u128 << fact_bit::RELIDO)) != 0 {
                // RELIDO can be flipped by Row 8 (SCI_PRESENT trigger)
                // or Row 9 (US_COLLATERAL_CLASSIFIED trigger; no SCI
                // required). Discriminate by checking the post-close
                // bitmask: if SCI was present at close-time, Row 8
                // fires (and Row 9 is suppressed by Row 8 having
                // already added RELIDO); otherwise Row 9 fires.
                let row8_fired = (post_close_bits & (1u128 << fact_bit::SCI_PRESENT)) != 0;
                let row_name = if row8_fired {
                    "capco:default-fill.dissem.sci-implies-relido"
                } else {
                    "capco:default-fill.dissem.us-class-implies-relido"
                };
                let step = next_step();
                sink.record(marque_scheme::DecisionEvent {
                    step,
                    site: marque_scheme::DecisionSite::Page(0),
                    category: marque_scheme::CategoryId::MARKING,
                    kind: marque_scheme::DecisionKind::Mutated,
                    source: marque_scheme::DecisionSource::DefaultFill(row_name),
                    triggered_by: None,
                });
            }
        }

        // Stage 4 — apply_supersession_overlays. The overlay clears
        // dominated tokens (§H.8 p145 NOFORN-dominates) and runs OC >
        // OC-USGOV / RELIDO observed-unanimity. Diff the dissem and
        // REL-TO axes by length comparison to detect a mutation; the
        // overlay does not add bits, so the delta is purely
        // subtractive (bits that were set before are clear after).
        let pre_overlay_dissem_len = out.0.dissem_us.len();
        let pre_overlay_rel_to_len = out.0.rel_to.len();
        let pre_overlay_display_only_len = out.0.display_only_to.len();
        Self::apply_supersession_overlays(&mut out.0);
        let post_overlay_dissem_len = out.0.dissem_us.len();
        let post_overlay_rel_to_len = out.0.rel_to.len();
        let post_overlay_display_only_len = out.0.display_only_to.len();
        if post_overlay_dissem_len != pre_overlay_dissem_len
            || post_overlay_rel_to_len != pre_overlay_rel_to_len
            || post_overlay_display_only_len != pre_overlay_display_only_len
        {
            let step = next_step();
            sink.record(marque_scheme::DecisionEvent {
                step,
                site: marque_scheme::DecisionSite::Page(0),
                category: marque_scheme::CategoryId::MARKING,
                kind: marque_scheme::DecisionKind::Mutated,
                source: marque_scheme::DecisionSource::Supersession(
                    "capco:supersession.dissem.h8-p145-overlays",
                ),
                triggered_by: None,
            });
        }

        #[cfg(debug_assertions)]
        {
            if raw != raw_snapshot.as_slice() {
                panic!(
                    "closure() mutated the per-portion CanonicalAttrs slice \
                     ({} portion(s) before vs {} after) — violates the \
                     PageRewrite read-only-attrs invariant",
                    raw_snapshot.len(),
                    raw.len(),
                );
            }
        }

        // Stage 5 — page rewrites. Emit one `RewriteScheduled` parent
        // event per fire and one `RewriteApplied` child with
        // `triggered_by` pointing at the parent.
        let mut page_mask = capco_axis_mask(&out);
        for rw in &self.page_rewrites {
            let eligible = match &rw.trigger {
                CategoryPredicate::Contains { category, .. } => {
                    page_mask & (1u64 << category.0) != 0
                }
                CategoryPredicate::Empty { category } => page_mask & (1u64 << category.0) == 0,
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
                let scheduled_step = next_step();
                sink.record(marque_scheme::DecisionEvent {
                    step: scheduled_step,
                    site: marque_scheme::DecisionSite::Page(0),
                    category: marque_scheme::CategoryId::MARKING,
                    kind: marque_scheme::DecisionKind::RewriteScheduled,
                    source: marque_scheme::DecisionSource::PageRewrite(rw.id),
                    triggered_by: None,
                });

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

                // Child event: one `RewriteApplied` linked to the
                // parent `RewriteScheduled` via `triggered_by`. We
                // emit one applied event per scheduled fire (not one
                // per affected portion); per-portion granularity is a
                // Phase D refinement deferral — the current `out`
                // accumulator stores the rolled-up marking, not the
                // per-portion writeback.
                let applied_step = next_step();
                sink.record(marque_scheme::DecisionEvent {
                    step: applied_step,
                    site: marque_scheme::DecisionSite::Page(0),
                    category: marque_scheme::CategoryId::MARKING,
                    kind: marque_scheme::DecisionKind::RewriteApplied,
                    source: marque_scheme::DecisionSource::PageRewrite(rw.id),
                    triggered_by: Some(scheduled_step),
                });
            }
        }
        out.0
    }
}
