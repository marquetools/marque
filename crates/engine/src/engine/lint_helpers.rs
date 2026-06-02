use super::dispatch::panic_payload_to_string;
use super::fix::pre_pass_1_attrs_for_span;
use super::page_context::project_page_marking;
#[cfg(feature = "decision-tracing")]
use super::page_context::project_page_marking_with_sink;
use super::page_context::{PageFinalizationContext, dispatch_page_finalization};
use super::*;

/// A scanner candidate the recognizer resolved to a marking.
///
/// Holds the recognized `S::Marking` whole (so the lint→fix cache keeps the
/// recognizer's full output, including any scheme-private recognition
/// side-channel) alongside the canonical projection the rule loop and page
/// accumulator consume. `attrs` is `scheme.canonical_from_marking(&marking)` —
/// computed once here (recognition already needs it for the rank floor) and
/// reused by the caller rather than re-derived.
pub(super) struct RecognizedCandidate<S: MarkingScheme> {
    pub(super) marking: S::Marking,
    pub(super) attrs: S::Canonical,
    pub(super) diagnostics_pre_candidate: usize,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_page_break_candidate<S, R>(
    engine: &Engine<S, R>,
    candidate: &marque_ism::MarkingCandidate,
    corrections_arc: &Option<Arc<HashMap<String, String>>>,
    deadline: Option<Instant>,
    diagnostics: &mut Vec<Diagnostic<S>>,
    page_portions: &mut Vec<S::Canonical>,
    page_portions_arc: &mut Option<Arc<Box<[S::Canonical]>>>,
    page_marking_arc: &mut Option<Arc<S::Projected>>,
    page_join_acc: &mut S::Canonical,
    doc_join_acc: &mut S::Canonical,
    page_banner_span: &mut Option<Span>,
    rank_floor: &mut Option<u8>,
    render_scratch: &mut String,
) -> Result<bool, ()>
where
    S: MarkingScheme + ConstraintBridge,
    S::Canonical: Clone + Default + PartialEq,
    R: Recognizer<S>,
{
    use marque_ism::MarkingType;

    if candidate.kind != MarkingType::PageBreak {
        return Ok(false);
    }

    // Phase C decision-tracing — `Evaluated` event at the page
    // boundary marking the banner roll-up dispatch entry. One
    // event per page boundary (PageBreak or EOD); per-axis
    // refinement deferred. Low frequency (one per page), so this
    // relies on `Engine::emit`'s `tracing_active` early-return rather
    // than a call-site guard — unlike the per-rule×candidate blocks
    // below, which gate at the call site.
    #[cfg(feature = "decision-tracing")]
    {
        if !page_portions.is_empty() {
            engine.emit(|step| marque_scheme::DecisionEvent {
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
    // observes per-stage projection events at the page boundary.
    // The subsequent `dispatch_page_finalization`
    // `get_or_insert_with` becomes a no-op (cell already populated)
    // and the OFF-feature build is byte-identical.
    #[cfg(feature = "decision-tracing")]
    {
        if engine.tracing_active() && !page_portions.is_empty() && page_marking_arc.is_none() {
            // Step-remapping wrapper translates scheme-side local
            // step IDs into the engine's global step space; see
            // `Engine::with_remapping_sink`. Only pre-init through the
            // sink-aware path when an observer is installed; otherwise
            // `dispatch_page_finalization` populates the cell lazily via
            // the plain projection, matching the OFF-feature path.
            *page_marking_arc = Some(Arc::new(engine.with_remapping_sink(|sink| {
                project_page_marking_with_sink(&engine.scheme, page_join_acc, sink)
            })));
        }
    }
    if !page_portions.is_empty()
        && dispatch_page_finalization(
            &engine.scheme,
            &engine.rule_sets,
            &engine.pass_finalization_rule_indices,
            &engine.fast_path_severities,
            &engine.emitted_id_overrides,
            PageFinalizationContext {
                portions: page_portions,
                portions_arc: page_portions_arc,
                marking_arc: page_marking_arc,
                join_acc: page_join_acc,
                banner_span: *page_banner_span,
                boundary_offset: candidate.span.start,
            },
            corrections_arc,
            deadline,
            diagnostics,
        )
        .is_err()
    {
        return Err(());
    }

    // Fold the closing page's canonical rollup into the document
    // accumulator before the per-page reset below. The page join is a
    // genuine semilattice join for lattice schemes (CapcoScheme), so the
    // page→document fold is order-independent (research D12 / LV3); the
    // default scheme gets last-page-wins. Sits before the unconditional
    // reset so a malformed page-break that still reached this branch folds
    // page N and resets for N+1 (#799).
    if !page_portions.is_empty() {
        *doc_join_acc = engine
            .scheme
            .canonical_document_join(&[std::mem::take(doc_join_acc), page_join_acc.clone()]);
    }

    *page_portions = fresh_page_portions_accumulator::<S>();
    *page_join_acc = <S::Canonical>::default();
    *page_portions_arc = None;
    *page_banner_span = None;
    *page_marking_arc = None;
    *rank_floor = None;
    render_scratch.clear();
    Ok(true)
}

pub(super) fn recognize_marking_candidate<S, R>(
    engine: &Engine<S, R>,
    source: &[u8],
    candidate: &marque_ism::MarkingCandidate,
    diagnostics: &mut Vec<Diagnostic<S>>,
    recognized_marking_count: &mut usize,
    rank_floor: &mut Option<u8>,
    input_source: marque_scheme::InputSource,
) -> Option<RecognizedCandidate<S>>
where
    S: MarkingScheme + ConstraintBridge,
    R: Recognizer<S>,
{
    let span_start = candidate.span.start.min(source.len());
    let span_end = candidate.span.end.min(source.len());
    let preceded_by_whitespace = match span_start.checked_sub(1) {
        None => true,
        Some(prev_idx) => source
            .get(prev_idx)
            .map(|b| b.is_ascii_whitespace())
            .unwrap_or(true),
    };
    let line_start = source[..span_start]
        .iter()
        .rposition(|&b| b == b'\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    let line_offset = span_start - line_start;
    let line_prefix =
        marque_scheme::recognizer::LinePrefix::from_slice(&source[line_start..span_start]);
    let surrounding_is_lowercase = surrounding_lowercase_majority(source, span_start, span_end);
    // `ParseContext` is `#[non_exhaustive]` (#176 staging step 1), so
    // it is constructed via `default()` + field assignment rather than
    // a record literal across the crate boundary.
    let mut parse_cx = ParseContext::default();
    parse_cx.strict_evidence = false;
    parse_cx.rank_floor = *rank_floor;
    parse_cx.preceded_by_whitespace = preceded_by_whitespace;
    parse_cx.line_offset = Some(line_offset);
    parse_cx.line_prefix = Some(line_prefix);
    parse_cx.surrounding_is_lowercase = surrounding_is_lowercase;
    // #176 / SC-010: thread the recognition-provenance axis into the
    // per-candidate context so the decoder's lone-case heuristic can
    // calibrate. `DocumentContent` (the raw-text path) keeps the
    // conservative guard; `StructuredField` (trusted-caller entry)
    // lifts it.
    parse_cx.input_source = input_source;

    let start = span_start;
    let end = span_end;
    if start >= end {
        return None;
    }
    let bytes = &source[start..end];
    let Parsed::Unambiguous(marking) =
        engine
            .recognizer
            .recognize(bytes, start, &engine.scheme, &parse_cx)
    else {
        return None;
    };
    *recognized_marking_count += 1;
    let diagnostics_pre_candidate = diagnostics.len();

    // Canonical projection of the recognized marking. Computed once here
    // (the rank floor below reads it, and the caller reuses it for the page
    // accumulator) so the marking can be moved whole into the lint→fix
    // cache without re-deriving its canonical.
    let attrs = engine.scheme.canonical_from_marking(&marking);

    // What the recognition tells the engine beyond the marking itself:
    // strict vs. probabilistic path, posterior score, and the optional
    // synthetic recognition diagnostic. For a strict recognizer this is the
    // inert default; the CAPCO decoder fills it from the recognizer's
    // side-channel and synthesizes the diagnostic internally.
    let outcome = engine.scheme.recognition_outcome(
        &marking,
        Span::new(start, end),
        bytes,
        candidate.kind,
        engine.corpus_override_active(),
    );

    // Strict recognitions raise the page rank floor by the scheme's monotone
    // sensitivity rank; probabilistic ones do not (they are bounded by the
    // existing floor).
    if !outcome.is_decoder_path
        && let Some(level) = engine.scheme.canonical_rank(&attrs)
    {
        *rank_floor = Some(match *rank_floor {
            Some(prev) => prev.max(level),
            None => level,
        });
    }

    if let Some(diagnostic) = outcome.diagnostic {
        diagnostics.push(diagnostic);
    }

    // Reject a probabilistic recognition whose posterior is below the
    // configured confidence threshold. The strict path carries no score and
    // is unconditionally accepted.
    if let Some(score) = outcome.recognition_score
        && score < engine.config.confidence_threshold()
    {
        return None;
    }

    Some(RecognizedCandidate {
        marking,
        attrs,
        diagnostics_pre_candidate,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn dispatch_rules_for_marking<S, R>(
    engine: &Engine<S, R>,
    candidate: &marque_ism::MarkingCandidate,
    attrs: &S::Canonical,
    corrections_arc: &Option<Arc<HashMap<String, String>>>,
    pre_pass_1_cache: Option<&[(Span, S::Canonical)]>,
    page_portions: &[S::Canonical],
    page_marking_arc: &mut Option<Arc<S::Projected>>,
    page_join_acc: &S::Canonical,
    page_banner_span: &mut Option<Span>,
    diagnostics: &mut Vec<Diagnostic<S>>,
) where
    S: MarkingScheme + ConstraintBridge,
    R: Recognizer<S>,
{
    use marque_ism::MarkingType;
    use marque_rules::RuleContext;

    // Decision-tracing site discriminator (Copilot-flagged correctness
    // fix): `DecisionSite::Portion(idx)` is documented as a portion
    // ordinal, NOT a byte offset. Earlier wiring populated it with
    // `candidate.span.start`, which would have caused `CountingSink`
    // and `DecisionReport::by_portion` to allocate vectors sized to
    // the largest byte offset seen — pathological on real inputs.
    // Now: portions use their per-page ordinal (`page_portions.len()`
    // is the upcoming index because the candidate hasn't been pushed
    // yet); banner / CAB candidates route to `DecisionSite::Banner`;
    // page-breaks never reach here.
    #[cfg(feature = "decision-tracing")]
    let decision_site = match candidate.kind {
        MarkingType::Portion => {
            marque_scheme::DecisionSite::Portion(page_portions.len().min(u32::MAX as usize) as u32)
        }
        MarkingType::Banner | MarkingType::Cab => marque_scheme::DecisionSite::Banner,
        // `MarkingType` is `#[non_exhaustive]`; PageBreak doesn't
        // reach here (handled by `handle_page_break_candidate`) and
        // any future kind falls back to a document-scope event.
        _ => marque_scheme::DecisionSite::Document,
    };

    let ctx_page_portions: Option<Arc<Box<[S::Canonical]>>> = None;
    let ctx_page_marking = if candidate.kind != MarkingType::Portion && !page_portions.is_empty() {
        Some(
            page_marking_arc
                .get_or_insert_with(|| {
                    // Phase D decision-tracing — route the per-page
                    // projection through the sink-aware variant so the
                    // engine's sink observes per-stage events
                    // (closure / default-fill / supersession / page
                    // rewrites). OFF-feature build keeps the original
                    // signature with zero extra work on the hot path.
                    #[cfg(feature = "decision-tracing")]
                    {
                        if engine.tracing_active() {
                            // Use the step-remapping adapter so scheme-side
                            // local step IDs (0, 1, 2, …) are translated
                            // into the engine's global step space — keeps
                            // cascade reconstruction sound across the
                            // engine/scheme boundary.
                            Arc::new(engine.with_remapping_sink(|sink| {
                                project_page_marking_with_sink(&engine.scheme, page_join_acc, sink)
                            }))
                        } else {
                            // No observer: take the plain projection so the
                            // feature-ON / no-observer path is identical to
                            // the OFF-feature path (no lock, no remapping
                            // HashMap, no per-stage diff events).
                            Arc::new(project_page_marking(&engine.scheme, page_join_acc))
                        }
                    }
                    #[cfg(not(feature = "decision-tracing"))]
                    {
                        Arc::new(project_page_marking(&engine.scheme, page_join_acc))
                    }
                })
                .clone(),
        )
    } else {
        None
    };
    let pre_pass_1_attrs =
        pre_pass_1_cache.and_then(|cache| pre_pass_1_attrs_for_span::<S>(cache, candidate.span));
    if candidate.kind == MarkingType::Banner {
        *page_banner_span = Some(candidate.span);
    }
    let ctx = RuleContext::<S>::new(candidate.kind, candidate.span)
        .with_page_portions(ctx_page_portions)
        .with_page_marking(ctx_page_marking)
        .with_page_banner_span(None)
        .with_corrections(corrections_arc.clone())
        .with_pre_pass_1_attrs(pre_pass_1_attrs);
    for (set_idx, rule_set) in engine.rule_sets.iter().enumerate() {
        for (rule_idx, rule) in rule_set.rules().iter().enumerate() {
            if engine
                .pass_finalization_rule_indices
                .iter()
                .any(|&(s, r)| s == set_idx && r == rule_idx)
            {
                continue;
            }
            if rule.additional_emitted_ids().is_empty() {
                let configured_severity = engine.fast_path_severities[set_idx][rule_idx];
                if configured_severity == Severity::Off {
                    continue;
                }
            }
            let rule_id = rule.id();
            // Phase C decision-tracing emission — `Evaluated`
            // event before the rule body runs. Granularity is
            // per-portion per-rule-check call (NOT per-axis); the
            // `EvaluatedSubstantive` refinement is deferred per
            // `plans/i-see-this-as-jiggly-lobster.md` "Open items"
            // §1. Site comes from the kind-discriminated
            // `decision_site` computed above; portions carry their
            // per-page ordinal, banner/CAB candidates carry
            // `DecisionSite::Banner`.
            #[cfg(feature = "decision-tracing")]
            {
                // Gate the whole emission block on the observer flag, not
                // just `emit()` internally: this fires once per
                // (rule × candidate) — ~300×/10KB doc — so skipping the
                // `predicate_id` read and closure construction when no
                // observer is installed keeps the feature-ON / no-observer
                // path on the no-feature path's latency floor.
                if engine.tracing_active() {
                    let predicate_id: &'static str = rule_id.predicate_id();
                    engine.emit(|step| marque_scheme::DecisionEvent {
                        step,
                        site: decision_site,
                        category: marque_scheme::CategoryId::MARKING,
                        kind: marque_scheme::DecisionKind::Evaluated,
                        source: marque_scheme::DecisionSource::RuleCheck(predicate_id),
                        triggered_by: None,
                    });
                }
            }
            let mut diags = if rule.trusted() {
                rule.check(attrs, &ctx)
            } else {
                match std::panic::catch_unwind(AssertUnwindSafe(|| rule.check(attrs, &ctx))) {
                    Ok(d) => d,
                    Err(payload) => {
                        let msg = panic_payload_to_string(&payload);
                        tracing::warn!(
                            target: "marque_engine::rule_panic",
                            rule = %rule_id,
                            error = %msg,
                            "rule check panicked; skipping this rule for the current candidate"
                        );
                        Vec::new()
                    }
                }
            };
            diags.retain_mut(|d| {
                match engine
                    .emitted_id_overrides
                    .get(d.rule.predicate_id())
                    .copied()
                {
                    Some(Severity::Off) => false,
                    Some(override_severity) => {
                        d.severity = override_severity;
                        true
                    }
                    None => true,
                }
            });
            // Phase C decision-tracing — `Mutated` event when the
            // rule produced a fix-carrying diagnostic that survived
            // severity-override filtering. Emitting after
            // `retain_mut` ensures the event reflects diagnostics
            // the engine will actually surface, not ones suppressed
            // by `Severity::Off`. Diagnostics without a fix are
            // observations, not mutations, and are covered by the
            // pre-check `Evaluated` emission above.
            #[cfg(feature = "decision-tracing")]
            {
                // Same observer gate as the `Evaluated` block above: skip
                // the per-rule `any_fix` diagnostic scan + closure
                // construction entirely when no observer is listening.
                if engine.tracing_active() {
                    let any_fix = diags.iter().any(|d| d.fix.is_some());
                    if any_fix {
                        let predicate_id: &'static str = rule_id.predicate_id();
                        engine.emit(|step| marque_scheme::DecisionEvent {
                            step,
                            site: decision_site,
                            category: marque_scheme::CategoryId::MARKING,
                            kind: marque_scheme::DecisionKind::Mutated,
                            source: marque_scheme::DecisionSource::RuleCheck(predicate_id),
                            triggered_by: None,
                        });
                    }
                }
            }
            diagnostics.extend(diags);
        }
    }
}
