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
    dissem_token_id_for_form, dissem_token_span, first_sci_span, infer_companion_form,
    last_dissem_span, us_level,
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
/// Falls back to `Severity::Error` no-fix when no dissem block exists
/// — inserting a whole new dissem category from rule context is
/// unsafe (the structural addition has no existing block to compose
/// with for canonical re-rendering). Same policy as E040.
//
// 8 args is the irreducible carrying capacity: id/severity for the
// catalog row, anchor_span/candidate_span for the diagnostic-vs-fix
// span split, last_dissem for the anchor lookup, token/message/citation
// for the emission. Folding into a struct would shift the count
// without reducing it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_companion_insert(
    rule: marque_rules::RuleId,
    severity: marque_rules::Severity,
    anchor_span: marque_ism::Span,
    candidate_span: marque_ism::Span,
    fix_scope: marque_scheme::Scope,
    last_dissem: Option<marque_ism::Span>,
    token: &str,
    message: String,
    citation: &'static str,
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
    match last_dissem {
        Some(_dissem_span) => {
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
        None => {
            // No dissem block — escalate to Error with no fix.
            Diagnostic::info(rule, Severity::Error, anchor_span, message, citation)
        }
    }
}

// ---------------------------------------------------------------------------
// Per-row Custom-kind emit closures (rows #1, #3, #4)
// ---------------------------------------------------------------------------

/// Row #1 — HCS-O companions: requires ORCON + NOFORN, forbids
/// ORCON-USGOV. §H.4 p64.
pub(crate) fn emit_hcs_o_companions(
    attrs: &marque_ism::CanonicalAttrs,
    candidate_span: marque_ism::Span,
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
    let last_dissem = last_dissem_span(attrs);
    let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));

    if !has_orcon {
        out.push(emit_companion_insert(
            RULE_E059,
            row.severity,
            sci_span,
            candidate_span,
            fix_scope,
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
            candidate_span,
            fix_scope,
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
pub(crate) fn emit_hcs_p_sub_companions(
    attrs: &marque_ism::CanonicalAttrs,
    candidate_span: marque_ism::Span,
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
    let last_dissem = last_dissem_span(attrs);
    let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));

    if !has_orcon {
        out.push(emit_companion_insert(
            RULE_E059,
            row.severity,
            sci_span,
            candidate_span,
            fix_scope,
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
pub(crate) fn emit_si_g_companions(
    attrs: &marque_ism::CanonicalAttrs,
    candidate_span: marque_ism::Span,
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
    let last_dissem = last_dissem_span(attrs);
    let sci_span = first_sci_span(attrs).unwrap_or(Span::new(0, 0));

    if !has_orcon {
        out.push(emit_companion_insert(
            RULE_E059,
            row.severity,
            sci_span,
            candidate_span,
            fix_scope,
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
pub(crate) fn emit_companion_required(
    attrs: &marque_ism::CanonicalAttrs,
    candidate_span: marque_ism::Span,
    fix_scope: marque_scheme::Scope,
    row: &SciPerSystemRow,
    dissem: marque_ism::DissemControl,
    token_name: &'static str,
) -> Vec<marque_rules::Diagnostic<CapcoScheme>> {
    use marque_ism::Span;

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
        candidate_span,
        fix_scope,
        last_dissem,
        companion_text,
        message,
        row.citation,
    )]
}
