use super::*;

pub(super) fn apply_constraint_bridge_for_marking(
    engine: &Engine,
    candidate: &marque_ism::MarkingCandidate,
    attrs: &marque_ism::CanonicalAttrs,
    diagnostics: &mut Vec<Diagnostic<CapcoScheme>>,
) {
    if !engine.scheme.has_diagnostic_constraints() {
        return;
    }

    let marking = marque_capco::CapcoMarking::from(attrs.clone());
    for v in engine.scheme.validate(&marking) {
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

impl Engine {
    /// Translate a scheme-emitted [`ConstraintViolation`] into an
    /// engine-side [`Diagnostic`].
    ///
    /// Returns `None` for advisory violations — entries whose `span`
    /// or `severity` is `None` are tooling-only signals that never
    /// surface to users. Returns `None` for severity-`Off` overrides
    /// (FR-008: `Off`-severity diagnostics are unrepresentable).
    ///
    /// For qualifying violations the bridge:
    ///
    /// 1. Constructs the `RuleId` 2-tuple `("capco",
    ///    constraint_label)` — the catalog row's `constraint_label`
    ///    IS the predicate id (T044 OD-8.A, no string folding, no
    ///    legacy-id lookup table).
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
        attrs: &marque_ism::CanonicalAttrs,
        candidate: &marque_ism::MarkingCandidate,
    ) -> Option<marque_rules::Diagnostic<CapcoScheme>> {
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

        // T044 (OD-8.A) — the bridge is a no-op pass-through. The
        // catalog row's `constraint_label` IS the predicate id
        // post-T044, so we construct `RuleId::new("capco",
        // constraint_label)` directly — no prefix recovery, no
        // legacy-id lookup table.
        //
        // Pre-T044 the bridge parsed
        // `constraint_label.split('/').next()` to recover a flat
        // `E### / W###` legacy id and applied a 15-row special-case
        // table to remap `capco/...` rows to specific `E0xx`. That
        // translation layer was the source of a class of "label says
        // one thing, rule id says another" drift bugs (CLAUDE.md "PR
        // 3b umbrella closeout" entry). Eliminating the translation
        // table eliminates the drift surface. See
        // `docs/refactor-006/2026-05-22-T044-rule-id-tuple-plan.md`
        // §2.2 + OD-8.A + the rename table in §1.5; the
        // `docs/refactor-006/legacy-rule-id-map.md` records each
        // pre-T044 catalog label's predicate-id successor for
        // archaeology.
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

        // PR 3c.2.C C5 / PM-C-1 / PR 10.A.1 bridge layer: convert the
        // carrier-string `ConstraintViolation.message: String` to a typed
        // `Diagnostic.message: Message`. The citation channel is no
        // longer bridged — PR 10.A.1 made `Constraint.label: Citation`
        // typed at declaration, and `ConstraintViolation.citation:
        // Citation` flows verbatim through the evaluator. The
        // `citation_by_name` lookup and `EngineInternal` sentinel
        // fallback were retired in PR 10.A.1.
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
                // message is dropped (G13). Future labels SHOULD be
                // added to `message_by_name` explicitly.
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

        // PR 10.A.1: catalog-row citations are now typed end-to-end. The
        // `ConstraintViolation.citation: Citation` value flowed verbatim
        // from the constraint's `label: Citation` declaration via
        // `marque_scheme::constraint::evaluate`, so the bridge is a direct
        // copy — the prior `citation_by_name` fallback and
        // `EngineInternal` sentinel are gone.
        let citation = v.citation;

        let mut diag =
            Diagnostic::with_fix(rule_id, final_severity, span, message, citation, fix_intent);
        diag.candidate_span = Some(candidate.span);
        Some(diag)
    }
}
