// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! SCI banner-roll-up evaluator.

use marque_ism::{CanonicalAttrs, Span, TokenKind, TokenSpan};
use marque_rules::{Diagnostic, FixSource, Message, MessageArgs, MessageTemplate, Severity};
use marque_scheme::{Citation, SectionLetter, capco};

use crate::rules::helpers::{FixDiagnosticParams, make_fix_diagnostic};
use crate::rules_legacy::{render_sci_block, sci_system_text};
use crate::scheme::CapcoScheme;

use super::BannerCategoryRow;

/// Typed Citation anchors at §H.4 p61 (the cross-system SCI banner-rollup
/// authority). The per-system "Precedence Rules for Banner Line Guidance"
/// passages (HCS p62, SI p74, TK p85, …) reiterate the same invariant
/// per system; §H.4 p61 is the operative cross-system citation per
/// T026a D13 single-citation discipline.
const CITATION: Citation = capco(SectionLetter::H, 4, 61);

/// SCI banner roll-up evaluator. Verbatim move of the body of
/// `SciBannerRollupRule::check`, parameterized over the page projection
/// and the catalog row.
///
/// **Operative authority**: CAPCO-2016 §H.4 per-system "Precedence
/// Rules for Banner Line Guidance" template (HCS p62, SI p74, TK p85,
/// …) — *"All unique SCI markings contained in the portion marks must
/// always appear in the banner line."* Unlike SAR (§H.5 p101), SCI
/// has no hierarchy-optional carve-out: compartments and
/// sub-compartments roll up too.
///
/// **Background**: §D.2 p28 (CAPCO-2016 lines 577–579) restates the
/// same banner/portion consistency invariant in general-algorithm
/// prose. Per T026a D13 single-citation discipline (and
/// `specs/006-engine-rule-refactor/tasks.md` T026a wording — *"§D.2
/// is general-algorithm prose (per-category citations are tighter and
/// verifiable per Constitution VIII)"*), §D.2 is a background pointer
/// only and is deliberately NOT included in [`CITATION`]. The
/// per-category §H.4 reference is the row's primary citation.
pub(super) fn evaluate_sci_banner_rollup(
    attrs: &CanonicalAttrs,
    page: &marque_ism::ProjectedMarking,
    row: &BannerCategoryRow,
) -> Vec<Diagnostic<CapcoScheme>> {
    // PR 9b (T133): SCI rollup reads `page.sci_markings` directly.
    // `ProjectedMarking` carries the union with §A.6 ordering already
    // applied by `PageContext::expected_sci_markings`.
    let expected: Vec<marque_ism::SciMarking> = page.sci_markings.to_vec();
    if expected.is_empty() {
        // Either P4 has not landed yet (helper returns empty) or no
        // portions have been accumulated. Either way, nothing to check.
        return vec![];
    }

    let mut missing: Vec<String> = Vec::new();
    for exp in expected.iter() {
        let exp_key = sci_system_text(&exp.system);
        let observed = attrs
            .sci_markings
            .iter()
            .find(|m| sci_system_text(&m.system) == exp_key);
        match observed {
            None => {
                missing.push(format!("{} (system missing from banner)", exp_key));
            }
            Some(obs) => {
                // Compartment check: every expected compartment must
                // appear in the observed marking.
                for exp_comp in exp.compartments.iter() {
                    let obs_comp = obs
                        .compartments
                        .iter()
                        .find(|c| c.identifier == exp_comp.identifier);
                    match obs_comp {
                        None => {
                            missing.push(format!(
                                "{}-{} (compartment missing from banner)",
                                exp_key,
                                exp_comp.identifier.as_str()
                            ));
                        }
                        Some(oc) => {
                            for exp_sub in exp_comp.sub_compartments.iter() {
                                if !oc.sub_compartments.iter().any(|s| s == exp_sub) {
                                    missing.push(format!(
                                        "{}-{} {} (sub-compartment missing from banner)",
                                        exp_key,
                                        exp_comp.identifier.as_str(),
                                        exp_sub.as_str()
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if missing.is_empty() {
        return vec![];
    }

    // Fix: replace the observed SCI block with the fully-rolled-up
    // form. The fix span covers every SciControl block token in order.
    let chunk_spans: Vec<&TokenSpan> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::SciControl)
        .collect();

    if chunk_spans.is_empty() {
        // Banner has no SCI block at all. Byte-positioning a new
        // block between classification and the next category from
        // rule context alone is unsafe (requires knowing the
        // separator offsets and the downstream block boundaries).
        // Escalate severity and emit a diagnostic without a fix
        // so the author inserts the block by hand.
        // G13: per-system detail dropped from the typed `Message`;
        // category=CAT_SCI identifies the axis.
        return vec![Diagnostic::new(
            row.rule_id,
            Severity::Error,
            Span::new(0, 0),
            Message::new(
                MessageTemplate::BannerRollupMismatch,
                MessageArgs {
                    category: Some(crate::scheme::CAT_SCI),
                    ..MessageArgs::default()
                },
            ),
            CITATION,
            None,
        )];
    }

    let fix_start = chunk_spans.first().unwrap().span.start;
    let fix_end = chunk_spans.last().unwrap().span.end;
    let original: String = chunk_spans
        .iter()
        .map(|t| t.text.as_ref())
        .collect::<Vec<_>>()
        .join("/");
    let fix_span = Span::new(fix_start, fix_end);
    let replacement = render_sci_block(&expected);

    // G13 (PM-C-6): drop the per-system `missing` list from the typed
    // `Message`. `MessageArgs.category = Some(CAT_SCI)` identifies the
    // axis that disagreed; per-system detail does NOT belong on the
    // audit record (would require a `MessageArgs.feature_ids`
    // population that needs a coordinated `MARQUE_AUDIT_SCHEMA` bump per
    // PM-C-6). The canonical replacement still rides on
    // `Diagnostic.text_correction.replacement` for the engine's apply path.
    vec![make_fix_diagnostic(FixDiagnosticParams {
        rule: row.rule_id,
        severity: row.severity,
        source: FixSource::BuiltinRule,
        span: fix_span,
        message: Message::new(
            MessageTemplate::BannerRollupMismatch,
            MessageArgs {
                category: Some(crate::scheme::CAT_SCI),
                ..MessageArgs::default()
            },
        ),
        citation: CITATION,
        original,
        replacement,
        confidence: 0.9,
        migration_ref: None,
    })]
}
