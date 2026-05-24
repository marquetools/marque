// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! SCI per-system companion emitters: `emit_companion_insert`
//! (single-token zero-width insertion), `emit_companion_required`
//! (`CompanionRequired`-kind dispatcher for catalog rows 2/5), and
//! the row-specific bodies for HCS-O / HCS-P-sub / SI-G companions
//! (rows 1/3/4). Lifted from the monolithic `actions.rs` per the
//! issue #466 Stage 2 PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).

use super::super::predicates::{
    dissem_token_id_for_form, dissem_token_span, first_sci_span, infer_companion_form, us_level,
};
use super::super::*;

/// Build a diagnostic that points at `anchor_span` (the offending SCI
/// token) with a structural `FixIntent::FactAdd` fix at the marking
/// scope. Diagnostic span and fix-scope span intentionally differ:
/// the user sees the SCI marking that triggered the requirement; the
/// engine's `synthesize_fixes` path applies the intent to the parsed
/// marking covering `candidate_span` and re-renders the canonical
/// bytes via `apply_intent` + `render_canonical`. Same
/// diagnostic-vs-fix-scope split used by `SarPortionFormRule` (E026).
///
/// When no dissem block exists yet, the same `FactAdd` intent is still
/// emitted; `apply_intent` mutates the parsed marking and
/// `render_canonical` synthesizes the `//` dissem block in canonical
/// form.
//
// 7 args is the irreducible carrying capacity: id/severity for the
// catalog row, anchor_span/candidate_span for the diagnostic-vs-fix
// span split, token/message/citation for the emission. Folding into a struct would shift the count
// without reducing it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_companion_insert(
    rule: marque_rules::RuleId,
    severity: marque_rules::Severity,
    anchor_span: marque_scheme::Span,
    candidate_span: marque_scheme::Span,
    fix_scope: marque_scheme::Scope,
    token: &str,
    message: marque_rules::Message,
    citation: marque_scheme::Citation,
) -> marque_rules::Diagnostic<CapcoScheme> {
    use marque_rules::{
        Confidence, Diagnostic, FixIntent, FixSource, Message, MessageArgs, MessageTemplate,
        Severity,
    };
    use marque_scheme::{FactRef, ReplacementIntent};
    let token_id = match dissem_token_id_for_form(token) {
        Some(id) => id,
        None => {
            // Unrecognized surface form — fail loudly with a no-fix
            // diagnostic rather than silently substituting NOFORN.
            // In normal flow this is unreachable (the catalog rows
            // pass `form.noforn()` / `form.orcon()` which return one
            // of the six recognized forms); reaching this arm means
            // a new surface form was added without updating the
            // lookup, which is a programming error worth surfacing.
            tracing::warn!(
                target: "marque_capco::scheme",
                token = token,
                "emit_companion_insert: unrecognized dissem-control surface form; emitting no-fix Error diagnostic"
            );
            return Diagnostic::info(rule, Severity::Error, anchor_span, message, citation);
        }
    };
    // Insert the companion token via a `FactAdd` intent.
    // `fix_scope` is the caller-derived scope: `Scope::Portion`
    // for portion candidates, `Scope::Page` for banner
    // candidates (the banner roll-up's per-page projection).
    // Both `NF`/`NOFORN` and `OC`/`ORCON`/`OC-USGOV`/
    // `ORCON-USGOV` resolve to the same canonical `TokenId`
    // per CVE — the engine's `render_canonical` decides
    // surface form from the inferred companion form.
    let intent = FixIntent::<CapcoScheme> {
        replacement: ReplacementIntent::FactAdd {
            token: FactRef::Cve(token_id),
            scope: fix_scope,
        },
        confidence: Confidence::strict(0.9),
        feature_ids: Default::default(),
        message: Message::new(MessageTemplate::RequiredByPresence, MessageArgs::default()),
        source: FixSource::BuiltinRule,
        migration_ref: None,
    };
    Diagnostic::with_fix_at_span(
        rule,
        severity,
        anchor_span,
        candidate_span,
        message,
        citation,
        intent,
    )
}

// ---------------------------------------------------------------------------
// Per-row Custom-kind emit closures (rows #1, #3, #4)
// ---------------------------------------------------------------------------

/// Row #1 — HCS-O companions: requires ORCON + NOFORN, forbids
/// ORCON-USGOV. §H.4 p64.
pub(crate) fn emit_hcs_o_companions(
    attrs: &marque_ism::CanonicalAttrs,
    candidate_span: marque_scheme::Span,
    fix_scope: marque_scheme::Scope,
    row: &SciPerSystemRow,
) -> Vec<marque_rules::Diagnostic<CapcoScheme>> {
    use crate::rules::{FixDiagnosticParams, make_fix_diagnostic};
    use marque_ism::{DissemControl, Span};
    use marque_rules::FixSource;

    if us_level(attrs).is_none() {
        return Vec::new();
    }
    let has_orcon = attrs.dissem_iter().any(|d| d == &DissemControl::Oc)
        || attrs.dissem_iter().any(|d| d == &DissemControl::OcUsgov);
    let has_noforn = attrs.dissem_iter().any(|d| d == &DissemControl::Nf);
    let usgov_entry = dissem_token_span(attrs, DissemControl::OcUsgov);

    let mut out = Vec::new();
    let form = infer_companion_form(attrs);
    let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));

    // T044 OD-8.A: the catalog row's `name` IS the predicate ID;
    // construct the audit RuleId from `row.name` directly with no
    // walker-shared `RULE_E059` indirection.
    let rule = marque_rules::RuleId::new("capco", row.name);

    if !has_orcon {
        out.push(emit_companion_insert(
            rule,
            row.severity,
            sci_span,
            candidate_span,
            fix_scope,
            form.orcon(),
            marque_rules::Message::new(
                marque_rules::MessageTemplate::RequiredByPresence,
                marque_rules::MessageArgs::default(),
            ),
            row.citation,
        ));
    }
    if !has_noforn {
        out.push(emit_companion_insert(
            rule,
            row.severity,
            sci_span,
            candidate_span,
            fix_scope,
            form.noforn(),
            marque_rules::Message::new(
                marque_rules::MessageTemplate::RequiredByPresence,
                marque_rules::MessageArgs::default(),
            ),
            row.citation,
        ));
    }
    if let Some((span, text)) = usgov_entry {
        // Audit content-ignorance: drop runtime byte text. Template names
        // the conflict class; `MessageArgs.category` carries the
        // dissem axis identifier.
        out.push(make_fix_diagnostic(FixDiagnosticParams {
            rule,
            severity: row.severity,
            source: FixSource::BuiltinRule,
            span,
            message: marque_rules::Message::new(
                marque_rules::MessageTemplate::ConflictsWith,
                marque_rules::MessageArgs {
                    category: Some(crate::scheme::CAT_DISSEM),
                    ..marque_rules::MessageArgs::default()
                },
            ),
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
pub(crate) fn emit_hcs_p_sub_companions(
    attrs: &marque_ism::CanonicalAttrs,
    candidate_span: marque_scheme::Span,
    fix_scope: marque_scheme::Scope,
    row: &SciPerSystemRow,
) -> Vec<marque_rules::Diagnostic<CapcoScheme>> {
    use crate::rules::{FixDiagnosticParams, make_fix_diagnostic};
    use marque_ism::{DissemControl, Span};
    use marque_rules::FixSource;

    if us_level(attrs).is_none() {
        return Vec::new();
    }
    let has_orcon = attrs.dissem_iter().any(|d| d == &DissemControl::Oc)
        || attrs.dissem_iter().any(|d| d == &DissemControl::OcUsgov);
    let usgov_entry = dissem_token_span(attrs, DissemControl::OcUsgov);

    let mut out = Vec::new();
    let form = infer_companion_form(attrs);
    let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));

    // T044 OD-8.A: row.name IS the predicate ID.
    let rule = marque_rules::RuleId::new("capco", row.name);

    if !has_orcon {
        out.push(emit_companion_insert(
            rule,
            row.severity,
            sci_span,
            candidate_span,
            fix_scope,
            form.orcon(),
            marque_rules::Message::new(
                marque_rules::MessageTemplate::RequiredByPresence,
                marque_rules::MessageArgs::default(),
            ),
            row.citation,
        ));
    }
    if let Some((span, text)) = usgov_entry {
        // Audit content-ignorance: drop runtime byte text. Template names
        // the conflict class; `MessageArgs.category` carries the
        // dissem axis identifier.
        out.push(make_fix_diagnostic(FixDiagnosticParams {
            rule,
            severity: row.severity,
            source: FixSource::BuiltinRule,
            span,
            message: marque_rules::Message::new(
                marque_rules::MessageTemplate::ConflictsWith,
                marque_rules::MessageArgs {
                    category: Some(crate::scheme::CAT_DISSEM),
                    ..marque_rules::MessageArgs::default()
                },
            ),
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
pub(crate) fn emit_si_g_companions(
    attrs: &marque_ism::CanonicalAttrs,
    candidate_span: marque_scheme::Span,
    fix_scope: marque_scheme::Scope,
    row: &SciPerSystemRow,
) -> Vec<marque_rules::Diagnostic<CapcoScheme>> {
    use crate::rules::{FixDiagnosticParams, make_fix_diagnostic};
    use marque_ism::{DissemControl, Span};
    use marque_rules::FixSource;

    if us_level(attrs).is_none() {
        return Vec::new();
    }
    let has_orcon = attrs.dissem_iter().any(|d| d == &DissemControl::Oc)
        || attrs.dissem_iter().any(|d| d == &DissemControl::OcUsgov);
    let usgov_entry = dissem_token_span(attrs, DissemControl::OcUsgov);

    let mut out = Vec::new();
    let form = infer_companion_form(attrs);
    let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));

    // T044 OD-8.A: row.name IS the predicate ID.
    let rule = marque_rules::RuleId::new("capco", row.name);

    if !has_orcon {
        out.push(emit_companion_insert(
            rule,
            row.severity,
            sci_span,
            candidate_span,
            fix_scope,
            form.orcon(),
            marque_rules::Message::new(
                marque_rules::MessageTemplate::RequiredByPresence,
                marque_rules::MessageArgs::default(),
            ),
            row.citation,
        ));
    }
    if let Some((span, text)) = usgov_entry {
        // Audit content-ignorance: drop runtime byte text. Template names
        // the conflict class; `MessageArgs.category` carries the
        // dissem axis identifier.
        out.push(make_fix_diagnostic(FixDiagnosticParams {
            rule,
            severity: row.severity,
            source: FixSource::BuiltinRule,
            span,
            message: marque_rules::Message::new(
                marque_rules::MessageTemplate::ConflictsWith,
                marque_rules::MessageArgs {
                    category: Some(crate::scheme::CAT_DISSEM),
                    ..marque_rules::MessageArgs::default()
                },
            ),
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
pub(crate) fn emit_companion_required(
    attrs: &marque_ism::CanonicalAttrs,
    candidate_span: marque_scheme::Span,
    fix_scope: marque_scheme::Scope,
    row: &SciPerSystemRow,
    dissem: marque_ism::DissemControl,
    token_name: &'static str,
) -> Vec<marque_rules::Diagnostic<CapcoScheme>> {
    use marque_scheme::Span;

    if us_level(attrs).is_none() {
        return Vec::new();
    }
    if attrs.dissem_iter().any(|d| d == &dissem) {
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
            .dissem_iter()
            .any(|d| d == &marque_ism::DissemControl::OcUsgov)
    {
        return Vec::new();
    }

    let form = infer_companion_form(attrs);
    let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));

    let companion_text = match dissem {
        marque_ism::DissemControl::Nf => form.noforn(),
        marque_ism::DissemControl::Oc => form.orcon(),
        // PR-E rows do not currently use other dissem controls; fall
        // back to the abbreviated CVE form for symmetry.
        _ => dissem.as_str(),
    };

    // Audit content-ignorance: drop the runtime interpolation; typed Message identifies
    // the required-by-presence class.
    let _ = (token_name, &row.marking_label);
    let message = marque_rules::Message::new(
        marque_rules::MessageTemplate::RequiredByPresence,
        marque_rules::MessageArgs::default(),
    );

    // T044 OD-8.A: row.name IS the predicate ID.
    let rule = marque_rules::RuleId::new("capco", row.name);

    vec![emit_companion_insert(
        rule,
        row.severity,
        sci_span,
        candidate_span,
        fix_scope,
        companion_text,
        message,
        row.citation,
    )]
}
