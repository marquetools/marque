// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Non-IC dissem (NODIS / EXDIS) banner-roll-up evaluator.

use marque_ism::{CanonicalAttrs, Span, TokenKind};
use marque_rules::{Diagnostic, FixSource, Message, MessageArgs, MessageTemplate, Severity};
use marque_scheme::{Citation, SectionLetter, capco};

use crate::rules::helpers::{FixDiagnosticParams, make_fix_diagnostic};
use crate::scheme::CapcoScheme;

use super::BannerCategoryRow;

/// Non-IC dissem banner roll-up evaluator. Verbatim move of the body of
/// `NodisExdisBannerRollupRule::check`, parameterized over the page
/// projection and the catalog row.
///
/// Authority: CAPCO-2016 §H.9 p174 (NODIS) + §H.9 p172 (EXDIS) — NODIS
/// has priority over EXDIS in the banner; either token, if present in
/// any portion, must roll up. The single operative supersession-and-
/// roll-up rule.
pub(super) fn evaluate_non_ic_dissem_banner_rollup(
    attrs: &CanonicalAttrs,
    page: &marque_ism::ProjectedMarking,
    row: &BannerCategoryRow,
) -> Vec<Diagnostic<CapcoScheme>> {
    use marque_ism::NonIcDissem;

    // PR 9b (T133): the NODIS/EXDIS supersession logic in
    // `PageContext::expected_non_ic_dissem` is preserved inside
    // `PageContext::project` (the `non_ic_dissem` field on the
    // projection comes from `expected_non_ic_dissem().0`). The
    // second tuple element (`needs_nf` injection signal) is
    // intentionally not surfaced here — this evaluator does not
    // consume it. If a future change needs it, plumb it through
    // either a `ProjectionProvenance` extension or a dedicated
    // accessor that returns the pre-projection
    // `(non_ic, needs_nf)` pair.
    let portions_have_nodis = page
        .non_ic_dissem
        .iter()
        .any(|d| matches!(d, NonIcDissem::Nodis));
    let portions_have_exdis = page
        .non_ic_dissem
        .iter()
        .any(|d| matches!(d, NonIcDissem::Exdis));

    // Determine what the banner MUST carry per §H.9. NODIS has
    // priority over EXDIS; if any portion has NODIS, the banner
    // must have NODIS even if other portions have EXDIS.
    let required = if portions_have_nodis {
        NonIcDissem::Nodis
    } else if portions_have_exdis {
        NonIcDissem::Exdis
    } else {
        return vec![];
    };

    let banner_has_required = attrs.non_ic_dissem.contains(&required);
    if banner_has_required {
        return vec![];
    }

    let required_str = required.banner_str();
    // PR 3c.2.C C5: both arms now use the typed `Message` shape.
    // §H.9 p172 (EXDIS) and §H.9 p174 (NODIS) — typed Citation
    // anchors at p172; the p174 cross-reference lives in the rule
    // doc comment.
    const CITATION: Citation = capco(SectionLetter::H, 9, 172);

    // Fix: if banner has at least one Non-IC dissem token, emit a
    // zero-width insertion at the end of that category block
    // appending `/<required>`. Otherwise, no-fix Error.
    let last_non_ic_span = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::NonIcDissem)
        .map(|t| t.span)
        .next_back();

    match last_non_ic_span {
        Some(last_span) => {
            let insertion = Span::new(last_span.end, last_span.end);
            let replacement = format!("/{required_str}");
            vec![make_fix_diagnostic(FixDiagnosticParams {
                rule: row.rule_id,
                severity: row.severity,
                source: FixSource::BuiltinRule,
                span: insertion,
                message: Message::new(
                    MessageTemplate::BannerRollupMismatch,
                    MessageArgs {
                        category: Some(crate::scheme::CAT_NON_IC_DISSEM),
                        ..MessageArgs::default()
                    },
                ),
                citation: CITATION,
                original: String::new(),
                replacement,
                confidence: 0.9,
                migration_ref: None,
            })]
        }
        None => {
            // No Non-IC dissem block in banner at all. Byte-
            // positioning a new block requires separator offsets
            // the rule cannot safely supply. No fix.
            let span = attrs
                .token_spans
                .first()
                .map(|t| t.span)
                .unwrap_or(Span::new(0, 0));
            // G13: drop the runtime `required_str` interpolation.
            let _ = required_str;
            vec![Diagnostic::new(
                row.rule_id,
                Severity::Error,
                span,
                Message::new(
                    MessageTemplate::BannerRollupMismatch,
                    MessageArgs {
                        category: Some(crate::scheme::CAT_NON_IC_DISSEM),
                        ..MessageArgs::default()
                    },
                ),
                CITATION,
                None,
            )]
        }
    }
}
