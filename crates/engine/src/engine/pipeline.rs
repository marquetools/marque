use super::bridge::apply_constraint_bridge_for_marking;
use super::lint_helpers::{
    dispatch_rules_for_marking, handle_page_break_candidate, recognize_marking_candidate,
};
use super::page_context::{PageFinalizationContext, dispatch_page_finalization};
use super::*;

impl Engine {
    /// Lint a UTF-8 text buffer. Returns diagnostics without modifying input.
    ///
    /// Back-compat shim over [`Engine::lint_with_options`] — calling
    /// `lint(src)` is equivalent to
    /// `lint_with_options(src, &LintOptions::default())`. New code that
    /// needs a deadline (spec 005 §R3) should call the `_with_options`
    /// variant directly.
    pub fn lint(&self, source: &[u8]) -> LintResult {
        self.lint_with_options(source, &LintOptions::default())
    }

    /// Lint with per-call options (spec 005 §R2).
    pub fn lint_with_options(&self, source: &[u8], opts: &LintOptions) -> LintResult {
        self.lint_with_options_internal(source, opts).0
    }

    /// Internal lint entrypoint that returns the parsed-markings cache
    /// alongside the public `LintResult`.
    pub(super) fn lint_with_options_internal(
        &self,
        source: &[u8],
        opts: &LintOptions,
    ) -> (LintResult, Vec<(Span, marque_capco::CapcoMarking)>) {
        self.lint_with_options_internal_with_cache(source, opts, None)
    }

    pub(super) fn lint_with_options_internal_with_cache(
        &self,
        source: &[u8],
        opts: &LintOptions,
        pre_pass_1_cache: Option<&[(Span, marque_ism::CanonicalAttrs)]>,
    ) -> (LintResult, Vec<(Span, marque_capco::CapcoMarking)>) {
        use marque_core::Scanner;
        use marque_ism::MarkingType;

        if deadline_expired(opts.deadline) {
            return (
                LintResult {
                    truncated: true,
                    ..Default::default()
                },
                Vec::new(),
            );
        }

        let candidates = Scanner::scan(source);
        let candidates_total = candidates.len();
        let mut candidates_processed: usize = 0;
        let mut recognized_marking_count: usize = 0;
        let mut parsed_markings: Vec<(Span, marque_capco::CapcoMarking)> = Vec::new();
        let corrections_arc = self.corrections_arc.clone();
        let mut diagnostics = Vec::new();
        let mut page_portions: Vec<marque_ism::CanonicalAttrs> = fresh_page_portions_accumulator();
        let mut page_portions_arc: Option<Arc<Box<[marque_ism::CanonicalAttrs]>>> = None;
        let mut page_marking_arc: Option<Arc<marque_ism::ProjectedMarking>> = None;
        let mut page_join_acc: marque_ism::CanonicalAttrs = marque_ism::CanonicalAttrs::default();
        let mut page_banner_span: Option<Span> = None;
        let mut classification_floor: Option<u8> = None;
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
                &mut page_banner_span,
                &mut classification_floor,
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
                    );
                }
            }

            let Some(recognized) = recognize_marking_candidate(
                self,
                source,
                candidate,
                &mut diagnostics,
                &mut recognized_marking_count,
                &mut classification_floor,
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
                    page_join_acc = marque_capco::CapcoMarking::join_via_lattice(&[
                        std::mem::take(&mut page_join_acc),
                        recognized.attrs.clone(),
                    ]);
                }
                if intent_emitted {
                    parsed_markings.push((
                        candidate.span,
                        marque_capco::CapcoMarking(recognized.attrs.clone(), recognized.provenance),
                    ));
                    page_portions.push(recognized.attrs);
                } else {
                    page_portions.push(recognized.attrs);
                }
                page_portions_arc = None;
                page_marking_arc = None;
            } else if intent_emitted {
                parsed_markings.push((
                    candidate.span,
                    marque_capco::CapcoMarking(recognized.attrs, recognized.provenance),
                ));
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
            );
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
                            RuleId::new("capco", "marking.correction.token-typo"),
                            c001_severity,
                            span,
                            marque_rules::Message::new(
                                marque_rules::MessageTemplate::CorrectionsApplied,
                                marque_rules::MessageArgs::default(),
                            ),
                            CORRECTIONS_MAP_CITATION,
                            value.as_ref(),
                            FixSource::CorrectionsMap,
                            marque_rules::Confidence::strict(1.0),
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
        )
    }
}
