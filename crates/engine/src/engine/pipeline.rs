use super::bridge::apply_constraint_bridge_for_marking;
use super::lint_helpers::{
    dispatch_rules_for_marking, handle_page_break_candidate, recognize_marking_candidate,
};
use super::page_context::{PageFinalizationContext, dispatch_page_finalization};
use super::*;

// The lint pipeline drives the scheme through the canonical-space trait
// surface: `canonical_from_marking` (per recognized candidate),
// `canonical_page_join` (per portion push), `project_canonical` (per page),
// and the `ConstraintBridge` hooks. Those `MarkingScheme` methods carry
// `unimplemented!()` defaults, so a scheme that reaches this pipeline MUST
// override them — today only `CapcoScheme` does, and it is the only scheme
// the (still scheme-pinned) constructors can produce. A future scheme made
// constructible here without those overrides would panic on the first
// recognized candidate, not miscompile.
impl<S, R> Engine<S, R>
where
    S: MarkingScheme + ConstraintBridge,
    S::Canonical: Clone + Default + PartialEq,
    R: Recognizer<S>,
{
    /// Lint a UTF-8 text buffer. Returns diagnostics without modifying input.
    ///
    /// Back-compat shim over [`Engine::lint_with_options`] — calling
    /// `lint(src)` is equivalent to
    /// `lint_with_options(src, &LintOptions::default())`. New code that
    /// needs a deadline (spec 005 §R3) should call the `_with_options`
    /// variant directly.
    pub fn lint(&self, source: &[u8]) -> LintResult<S> {
        self.lint_with_options(source, &LintOptions::default())
    }

    /// Lint with per-call options (spec 005 §R2).
    pub fn lint_with_options(&self, source: &[u8], opts: &LintOptions) -> LintResult<S> {
        self.lint_with_options_internal(source, opts).0
    }

    /// Lint, routing recognition by the input boundary's
    /// [`InputContext::source`](marque_scheme::InputContext) (#176 /
    /// #643, T013). Trusted callers (CLI `--input-source`, server
    /// per-request) reach this; the WASM target never does (it pins
    /// `DocumentContent`, Constitution III).
    ///
    /// Routing table:
    ///
    /// | `InputSource`     | branch                                            |
    /// |-------------------|---------------------------------------------------|
    /// | `DocumentContent` | existing raw-text pipeline, **byte-identical**    |
    /// | `StructuredField` | same scanner/recognizer/parser path, but the      |
    /// |                   | per-candidate `ParseContext.input_source` is set  |
    /// |                   | to `StructuredField` so the decoder's lone-case   |
    /// |                   | heuristic fires assertively (#176 / SC-010)       |
    /// | `SchemaDocument`  | adapter-owned — `InputAdapter::adapt` produces     |
    /// |                   | `S::Canonical` directly, bypassing this text       |
    /// |                   | pipeline. The CapcoScheme engine ships no schema   |
    /// |                   | adapter yet, so this entry treats it as the        |
    /// |                   | conservative text path (no adapter to dispatch);   |
    /// |                   | a schema adapter wires in via a later phase.       |
    ///
    /// The `DocumentContent` branch delegates verbatim to
    /// [`Self::lint_with_options`] to guarantee byte-identity.
    pub fn lint_with_input_context(
        &self,
        source: &[u8],
        opts: &LintOptions,
        input_cx: &marque_scheme::InputContext<'_>,
    ) -> LintResult<S> {
        match input_cx.source {
            // Byte-identical to the pre-#176 path.
            marque_scheme::InputSource::DocumentContent => self.lint_with_options(source, opts),
            // Same pipeline, recognition-provenance lifted to the
            // structured-field tier so the decoder's lone-case heuristic
            // recovers assertively (SC-010).
            marque_scheme::InputSource::StructuredField => {
                self.lint_with_options_internal_with_source(
                    source,
                    opts,
                    None,
                    marque_scheme::InputSource::StructuredField,
                )
                .0
            }
            // SchemaDocument is the adapter mechanism (InputAdapter::adapt
            // → S::Canonical, no recognizer). The CapcoScheme text engine
            // ships no schema adapter, so there is nothing to dispatch
            // here; fall through to the conservative text path rather than
            // fabricate canonicals. Wiring a real adapter is later-phase
            // work (the trait surface is WASM-safe; concrete adapters are
            // native).
            marque_scheme::InputSource::SchemaDocument => self.lint_with_options(source, opts),
            // `InputSource` is `#[non_exhaustive]`; a future variant lands
            // on the conservative text path until it is wired explicitly.
            _ => self.lint_with_options(source, opts),
        }
    }

    /// Internal lint entrypoint that returns the parsed-markings cache
    /// alongside the public `LintResult`.
    pub(super) fn lint_with_options_internal(
        &self,
        source: &[u8],
        opts: &LintOptions,
    ) -> (LintResult<S>, Vec<(Span, S::Marking)>) {
        self.lint_with_options_internal_with_cache(source, opts, None)
    }

    pub(super) fn lint_with_options_internal_with_cache(
        &self,
        source: &[u8],
        opts: &LintOptions,
        pre_pass_1_cache: Option<&[(Span, S::Canonical)]>,
    ) -> (LintResult<S>, Vec<(Span, S::Marking)>) {
        // Existing callers (the byte-identical raw-text path) recognize
        // as DocumentContent. The document-scope rollup is not consumed on
        // this path yet (#799), so the third element is discarded here.
        let (r, m, _) = self.lint_with_options_internal_with_source(
            source,
            opts,
            pre_pass_1_cache,
            marque_scheme::InputSource::DocumentContent,
        );
        (r, m)
    }

    // The 3-tuple return (lint result + parsed-markings cache +
    // document-scope rollup, #799) trips `type_complexity`; the shape is
    // the natural multi-output of one pipeline pass, and a type alias would
    // obscure it more than it clarifies — matching the existing
    // `synthesis.rs` precedent.
    #[allow(clippy::type_complexity)]
    pub(super) fn lint_with_options_internal_with_source(
        &self,
        source: &[u8],
        opts: &LintOptions,
        pre_pass_1_cache: Option<&[(Span, S::Canonical)]>,
        input_source: marque_scheme::InputSource,
    ) -> (LintResult<S>, Vec<(Span, S::Marking)>, S::Canonical) {
        use marque_core::Scanner;
        use marque_ism::MarkingType;

        // Decision-tracing: zero step IDs at the document boundary so
        // a long-lived engine doesn't leak step IDs across documents.
        // `triggered_by` references resolve into the current document's
        // event stream only.
        #[cfg(feature = "decision-tracing")]
        self.reset_decision_step_counter();

        if deadline_expired(opts.deadline) {
            // Pre-init guard: `doc_join_acc` does not exist yet, so the
            // partial rollup is the canonical bottom.
            return (
                LintResult {
                    truncated: true,
                    ..Default::default()
                },
                Vec::new(),
                <S::Canonical>::default(),
            );
        }

        let candidates = Scanner::scan(source);
        let candidates_total = candidates.len();
        let mut candidates_processed: usize = 0;
        let mut recognized_marking_count: usize = 0;
        let mut parsed_markings: Vec<(Span, S::Marking)> = Vec::new();
        let corrections_arc = self.corrections_arc.clone();
        let mut diagnostics = Vec::new();
        let mut page_portions: Vec<S::Canonical> = fresh_page_portions_accumulator::<S>();
        let mut page_portions_arc: Option<Arc<Box<[S::Canonical]>>> = None;
        let mut page_marking_arc: Option<Arc<S::Projected>> = None;
        let mut page_join_acc: S::Canonical = <S::Canonical>::default();
        // Document-scope rollup accumulator (#799): a running canonical
        // folded from each closing page's `page_join_acc` at every page
        // boundary and at end-of-document. Fresh `Default` per call is the
        // fresh-per-input guarantee (Constitution VI).
        let mut doc_join_acc: S::Canonical = <S::Canonical>::default();
        let mut page_banner_span: Option<Span> = None;
        let mut rank_floor: Option<u8> = None;
        let mut render_scratch = String::new();

        for candidate in &candidates {
            if deadline_expired(opts.deadline) {
                return (
                    LintResult {
                        diagnostics,
                        truncated: true,
                        candidates_processed,
                        candidates_total,
                        recognized_marking_count,
                        ..Default::default()
                    },
                    parsed_markings,
                    std::mem::take(&mut doc_join_acc),
                );
            }
            candidates_processed += 1;

            match handle_page_break_candidate(
                self,
                candidate,
                &corrections_arc,
                opts.deadline,
                &mut diagnostics,
                &mut page_portions,
                &mut page_portions_arc,
                &mut page_marking_arc,
                &mut page_join_acc,
                &mut doc_join_acc,
                &mut page_banner_span,
                &mut rank_floor,
                &mut render_scratch,
            ) {
                Ok(true) => continue,
                Ok(false) => {}
                Err(()) => {
                    return (
                        LintResult {
                            diagnostics,
                            truncated: true,
                            candidates_processed,
                            candidates_total,
                            recognized_marking_count,
                            ..Default::default()
                        },
                        parsed_markings,
                        std::mem::take(&mut doc_join_acc),
                    );
                }
            }

            let Some(recognized) = recognize_marking_candidate(
                self,
                source,
                candidate,
                &mut diagnostics,
                &mut recognized_marking_count,
                &mut rank_floor,
                input_source,
            ) else {
                continue;
            };

            dispatch_rules_for_marking(
                self,
                candidate,
                &recognized.attrs,
                &corrections_arc,
                pre_pass_1_cache,
                &page_portions,
                &mut page_marking_arc,
                &page_join_acc,
                &mut page_banner_span,
                &mut diagnostics,
            );
            apply_constraint_bridge_for_marking(
                self,
                candidate,
                &recognized.attrs,
                &page_portions,
                &mut diagnostics,
            );

            let intent_emitted = diagnostics[recognized.diagnostics_pre_candidate..]
                .iter()
                .any(|d| d.fix.is_some());
            debug_assert!(
                parsed_markings
                    .last()
                    .is_none_or(|(prev, _)| prev.start < candidate.span.start),
                "parsed_markings push violated strictly-increasing-start invariant: \
                 prev.start={:?} candidate.span.start={}",
                parsed_markings.last().map(|(s, _)| s.start),
                candidate.span.start
            );
            if candidate.kind == MarkingType::Portion {
                if page_portions.is_empty() {
                    page_join_acc = recognized.attrs.clone();
                } else {
                    page_join_acc = self.scheme.canonical_page_join(&[
                        std::mem::take(&mut page_join_acc),
                        recognized.attrs.clone(),
                    ]);
                }
                // The whole marking carries into the lint→fix cache (it
                // preserves the recognizer's full output); the separately
                // extracted canonical feeds the page accumulator.
                if intent_emitted {
                    parsed_markings.push((candidate.span, recognized.marking));
                    page_portions.push(recognized.attrs);
                } else {
                    page_portions.push(recognized.attrs);
                }
                page_portions_arc = None;
                page_marking_arc = None;
            } else if intent_emitted {
                parsed_markings.push((candidate.span, recognized.marking));
            }
        }

        // Phase C decision-tracing — `Evaluated` event at the
        // end-of-document banner roll-up boundary. Paired with
        // the per-PageBreak emission in `handle_page_break_candidate`
        // so every page boundary (PageBreak or EOD) is observed.
        // Low frequency (one per document), so this relies on
        // `Engine::emit`'s `tracing_active` early-return rather than a
        // call-site guard — unlike the per-rule×candidate blocks in
        // `lint_helpers.rs`, which gate at the call site.
        #[cfg(feature = "decision-tracing")]
        {
            if !page_portions.is_empty() {
                self.emit(|step| marque_scheme::DecisionEvent {
                    step,
                    site: marque_scheme::DecisionSite::Banner,
                    category: marque_scheme::CategoryId::MARKING,
                    kind: marque_scheme::DecisionKind::Evaluated,
                    source: marque_scheme::DecisionSource::BannerRollup,
                    triggered_by: None,
                });
            }
        }
        // Phase D decision-tracing — pre-init `page_marking_arc`
        // through the sink-aware projection so the engine's sink
        // observes per-stage projection events at end-of-document.
        // The subsequent `dispatch_page_finalization`
        // `get_or_insert_with` becomes a no-op (cell already populated)
        // and the OFF-feature build is byte-identical.
        #[cfg(feature = "decision-tracing")]
        {
            if self.tracing_active() && !page_portions.is_empty() && page_marking_arc.is_none() {
                // Use the step-remapping adapter so scheme-side local
                // step IDs translate into the engine's global step
                // space — see `Engine::with_remapping_sink`. Only pre-init
                // through the sink-aware path when an observer is
                // installed; otherwise `dispatch_page_finalization`
                // populates the cell lazily via the plain projection,
                // matching the OFF-feature path.
                page_marking_arc = Some(std::sync::Arc::new(self.with_remapping_sink(|sink| {
                    super::page_context::project_page_marking_with_sink(
                        &self.scheme,
                        &page_join_acc,
                        sink,
                    )
                })));
            }
        }
        if !page_portions.is_empty()
            && dispatch_page_finalization(
                &self.scheme,
                &self.rule_sets,
                &self.pass_finalization_rule_indices,
                &self.fast_path_severities,
                &self.emitted_id_overrides,
                PageFinalizationContext {
                    portions: &page_portions,
                    portions_arc: &mut page_portions_arc,
                    marking_arc: &mut page_marking_arc,
                    join_acc: &page_join_acc,
                    banner_span: page_banner_span,
                    boundary_offset: source.len(),
                },
                &corrections_arc,
                opts.deadline,
                &mut diagnostics,
            )
            .is_err()
        {
            return (
                LintResult {
                    diagnostics,
                    truncated: true,
                    candidates_processed,
                    candidates_total,
                    recognized_marking_count,
                    ..Default::default()
                },
                parsed_markings,
                std::mem::take(&mut doc_join_acc),
            );
        }

        // Fold the final (un-page-broken) page rollup into the document
        // accumulator — catches trailing portions that never reached a
        // PageBreak boundary. Guarded on non-empty portions exactly as the
        // EOD finalization dispatch above is. `page_join_acc` is not reset
        // at EOD (the document is ending), so the clone is the only way to
        // feed the fold (#799).
        if !page_portions.is_empty() {
            doc_join_acc = self
                .scheme
                .canonical_document_join(&[std::mem::take(&mut doc_join_acc), page_join_acc.clone()]);
        }

        if let Some(cached) = &self.corrections_ac {
            let c001_severity = self
                .emitted_id_overrides
                .get("marking.correction.token-typo")
                .copied()
                .unwrap_or(Severity::Fix);
            if c001_severity != Severity::Off {
                let existing_c001_spans: std::collections::HashSet<Span> = diagnostics
                    .iter()
                    .filter(|d| d.rule.predicate_id() == "marking.correction.token-typo")
                    .map(|d| d.span)
                    .collect();
                for mat in cached.ac.find_iter(source) {
                    let span = Span::new(mat.start(), mat.end());
                    let (ref key, ref value) = cached.active[mat.pattern().as_usize()];
                    if !existing_c001_spans.contains(&span) {
                        let _ = key;
                        diagnostics.push(Diagnostic::text_correction(
                            RuleId::new(self.scheme.scheme_id(), "marking.correction.token-typo"),
                            c001_severity,
                            span,
                            marque_rules::Message::new(
                                marque_rules::MessageTemplate::CorrectionsApplied,
                                marque_rules::MessageArgs::default(),
                            ),
                            CORRECTIONS_MAP_CITATION,
                            value.as_ref(),
                            FixSource::CorrectionsMap,
                            marque_rules::Recognition::strict(),
                            None,
                        ));
                    }
                }
            }
        }

        let threshold = self.config.confidence_threshold();
        for d in &mut diagnostics {
            if d.severity != Severity::Fix {
                continue;
            }
            let combined = match d.fix.as_ref() {
                Some(fix) => fix.confidence.combined(),
                None => continue,
            };
            if combined < threshold {
                d.severity = Severity::Suggest;
            }
        }

        (
            LintResult {
                diagnostics,
                truncated: false,
                candidates_processed,
                candidates_total,
                recognized_marking_count,
                ..Default::default()
            },
            parsed_markings,
            doc_join_acc,
        )
    }
}
