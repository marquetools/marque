use super::*;

pub(super) fn apply_constraint_bridge_for_marking<S, R>(
    engine: &Engine<S, R>,
    candidate: &marque_ism::MarkingCandidate,
    attrs: &S::Canonical,
    page_portions: &[S::Canonical],
    diagnostics: &mut Vec<Diagnostic<S>>,
) where
    S: MarkingScheme + ConstraintBridge,
    S::Canonical: Clone,
    R: Recognizer<S>,
{
    if !engine.scheme.has_diagnostic_constraints() {
        return;
    }

    // Decision-tracing site discriminator (Copilot-flagged correctness
    // fix): `DecisionSite::Portion` is documented as a portion ordinal,
    // not a byte offset. Earlier wiring populated it with
    // `candidate.span.start`, which would have caused per-portion
    // aggregations to allocate vectors sized to the largest byte offset
    // seen. Portions use their per-page ordinal
    // (`page_portions.len()` — the candidate hasn't been pushed yet);
    // banner/CAB candidates route to `DecisionSite::Banner`.
    #[cfg(feature = "decision-tracing")]
    let decision_site = match candidate.kind {
        marque_ism::MarkingType::Portion => {
            marque_scheme::DecisionSite::Portion(page_portions.len().min(u32::MAX as usize) as u32)
        }
        marque_ism::MarkingType::Banner | marque_ism::MarkingType::Cab => {
            marque_scheme::DecisionSite::Banner
        }
        // `MarkingType` is `#[non_exhaustive]`; PageBreak doesn't
        // reach here (handled by `handle_page_break_candidate`) and
        // any future kind falls back to a document-scope event.
        _ => marque_scheme::DecisionSite::Document,
    };
    #[cfg(not(feature = "decision-tracing"))]
    let _ = page_portions;

    let marking = engine.scheme.marking_from_canonical(attrs.clone());
    for v in engine.scheme.validate(&marking) {
        // Phase C decision-tracing — `ConstraintFired` event per
        // emitted `ConstraintViolation`. The non-firing path
        // (constraint evaluated but did NOT fire) is the noisy
        // case and is deferred per
        // `plans/i-see-this-as-jiggly-lobster.md` insertion-point
        // table. `constraint_label` is the catalog-row stable
        // identifier and IS the predicate id (see
        // `Engine::bridge_constraint_diagnostic` doc-comment).
        // Low frequency (only fires per emitted violation), so this
        // relies on `Engine::emit`'s `tracing_active` early-return
        // rather than a call-site guard — unlike the per-rule×candidate
        // blocks in `lint_helpers.rs`, which gate at the call site.
        #[cfg(feature = "decision-tracing")]
        {
            let label: &'static str = v.constraint_label;
            engine.emit(|step| marque_scheme::DecisionEvent {
                step,
                site: decision_site,
                category: marque_scheme::CategoryId::MARKING,
                kind: marque_scheme::DecisionKind::ConstraintFired,
                source: marque_scheme::DecisionSource::Constraint(label),
                triggered_by: None,
            });
        }
        if let Some(diag) = engine.bridge_constraint_diagnostic(&v, attrs, candidate) {
            diagnostics.push(diag);
        }
    }

    let fix_scope = match candidate.kind {
        marque_ism::MarkingType::Portion => marque_scheme::Scope::Portion,
        _ => marque_scheme::Scope::Page,
    };
    diagnostics.extend(engine.scheme.bridge_sci_per_system_diagnostics(
        attrs,
        candidate.span,
        fix_scope,
        &engine.emitted_id_overrides,
    ));
}

impl<S, R> Engine<S, R>
where
    S: MarkingScheme + ConstraintBridge,
    R: Recognizer<S>,
{
    /// Translate a scheme-emitted [`ConstraintViolation`] into an
    /// engine-side [`Diagnostic`].
    ///
    /// Returns `None` for advisory violations — entries whose `span`
    /// or `severity` is `None` are tooling-only signals that never
    /// surface to users. Returns `None` for severity-`Off` overrides
    /// (`Off`-severity diagnostics are unrepresentable).
    ///
    /// For qualifying violations the bridge:
    ///
    /// 1. Constructs the `RuleId` 2-tuple `("capco",
    ///    constraint_label)` — the catalog row's `constraint_label`
    ///    IS the predicate id (no string folding, no legacy-id lookup
    ///    table).
    /// 2. Applies the user-configured severity override
    ///    (`emitted_id_overrides`) keyed on the resolved RuleId's
    ///    predicate id.
    /// 3. Synthesizes the optional [`FixIntent`] via
    ///    [`CapcoScheme::fix_intent_by_name`] from the row name +
    ///    `attrs` + candidate `MarkingType`.
    /// 4. Resolves the user-facing message via
    ///    [`CapcoScheme::message_by_name`]; falls back to the generic
    ///    evaluator text from `ConstraintViolation.message` when the
    ///    scheme returns `None` for the row name.
    /// 5. Builds the [`Diagnostic`] with the resolved `message`
    ///    and `citation` carried through verbatim, and stamps the
    ///    candidate's outer span as `candidate_span`.
    ///
    /// [`ConstraintViolation`]: marque_scheme::ConstraintViolation
    /// [`Diagnostic`]: marque_rules::Diagnostic
    /// [`FixIntent`]: marque_rules::FixIntent
    /// [`CapcoScheme::fix_intent_by_name`]: CapcoScheme::fix_intent_by_name
    /// [`CapcoScheme::message_by_name`]: CapcoScheme::message_by_name
    pub(super) fn bridge_constraint_diagnostic(
        &self,
        v: &marque_scheme::ConstraintViolation,
        attrs: &S::Canonical,
        candidate: &marque_ism::MarkingCandidate,
    ) -> Option<marque_rules::Diagnostic<S>> {
        use marque_rules::{Diagnostic, RuleId, Severity};

        let span = match v.span {
            Some(s) => s,
            None => {
                tracing::trace!(
                    target: "marque_engine::constraint_bridge",
                    constraint = v.constraint_label,
                    "advisory constraint violation (no span); not surfaced as Diagnostic"
                );
                return None;
            }
        };

        let severity = match v.severity {
            Some(s) => s,
            None => {
                tracing::trace!(
                    target: "marque_engine::constraint_bridge",
                    constraint = v.constraint_label,
                    "advisory constraint violation (no severity); not surfaced as Diagnostic"
                );
                return None;
            }
        };

        // The bridge is a no-op pass-through. The catalog row's
        // `constraint_label` IS the predicate id, so we construct
        // `RuleId::new("capco", constraint_label)` directly — no prefix
        // recovery, no legacy-id lookup table. There is no translation
        // layer between label and rule id, so the "label says one
        // thing, rule id says another" drift surface does not exist.
        let rule_id = RuleId::new("capco", v.constraint_label);

        let final_severity = self
            .emitted_id_overrides
            .get(rule_id.predicate_id())
            .copied()
            .unwrap_or(severity);

        if final_severity == Severity::Off {
            return None;
        }

        let fix_intent = self
            .scheme
            .fix_intent_by_name(v.constraint_label, attrs, candidate.kind);

        // Convert the carrier-string `ConstraintViolation.message:
        // String` to a typed `Diagnostic.message: Message`. The citation
        // channel is not bridged — `Constraint.label: Citation` is typed
        // at declaration, and `ConstraintViolation.citation: Citation`
        // flows verbatim through the evaluator.
        //
        // The message lookup still falls back to a generic sentinel
        // when the constraint_label is not in the explicit mapping;
        // the fallback shape preserves audit-content-ignorance (no
        // `v.message` raw bytes flow through).
        let message = self
            .scheme
            .message_by_name(v.constraint_label, attrs, candidate.kind)
            .unwrap_or_else(|| {
                // Unknown constraint label — emit a generic
                // `ConflictsWith` template with no args so the audit
                // record is still closed-template. The original String
                // message is dropped (audit content-ignorance). Future
                // labels SHOULD be added to `message_by_name` explicitly.
                tracing::trace!(
                    target: "marque_engine::constraint_bridge",
                    constraint = v.constraint_label,
                    "no typed Message mapping for constraint_label; using generic fallback",
                );
                marque_rules::Message::new(
                    marque_rules::MessageTemplate::ConflictsWith,
                    marque_rules::MessageArgs::default(),
                )
            });

        // Catalog-row citations are typed end-to-end. The
        // `ConstraintViolation.citation: Citation` value flows verbatim
        // from the constraint's `label: Citation` declaration via
        // `marque_scheme::constraint::evaluate`, so the bridge is a
        // direct copy.
        let citation = v.citation;

        let mut diag =
            Diagnostic::with_fix(rule_id, final_severity, span, message, citation, fix_intent);
        diag.candidate_span = Some(candidate.span);
        Some(diag)
    }
}
