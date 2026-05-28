// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! SAR banner-roll-up evaluator.

use std::collections::HashSet;

use marque_ism::{CanonicalAttrs, Span, sar_sort_key};
use marque_rules::{Diagnostic, FixSource, Message, MessageArgs, MessageTemplate, Severity};
use marque_scheme::{Citation, SectionLetter, capco};

use super::super::helpers::{FixDiagnosticParams, make_fix_diagnostic, sar_block_span};
use crate::scheme::CapcoScheme;

use super::BannerCategoryRow;

/// SAR banner roll-up evaluator, parameterized over the page projection
/// and the catalog row (so the row supplies the rule ID + severity).
///
/// Reads `&ProjectedMarking` — the field `sar_markings` carries the
/// union of portion-contributed SAR programs in the same shape
/// `PageContext::expected_sar_marking` used to compute.
///
/// Authority: CAPCO-2016 §H.5 p101 (Unique SAPs contained in portion
/// marks must always appear in the banner line; hierarchy depiction
/// optional per §H.5 p101 + p99).
pub(super) fn evaluate_sar_banner_rollup(
    attrs: &CanonicalAttrs,
    page: &marque_ism::ProjectedMarking,
    row: &BannerCategoryRow,
) -> Vec<Diagnostic<CapcoScheme>> {
    let Some(expected) = page.sar_markings.as_ref() else {
        return vec![];
    };
    if expected.programs.is_empty() {
        return vec![];
    }

    // Compute the identifiers of programs missing from the
    // observed banner. Hierarchy (compartments / sub-compartments)
    // is deliberately NOT compared — §H.5 p101 makes
    // banner hierarchy depth optional even when portions carry
    // hierarchy. See the `sar_missing_programs` helper doc for
    // the authority trail.
    let missing_ids: Vec<&str> = sar_missing_programs(attrs.sar_markings.as_ref(), expected);
    if missing_ids.is_empty() {
        return vec![];
    }

    // Typed Citation anchors at §H.5 p101 (the SAR per-system banner
    // rule); the hierarchy-optional note at §H.5 p99 is cross-
    // referenced in the rule doc comment, not in the Citation field.
    const CITATION: Citation = capco(SectionLetter::H, 5, 101);

    // Sort missing identifiers per §H.5 p99 (ascending,
    // numeric first, then alpha) so the fix output is
    // deterministic and self-canonical for the new tail.
    let mut sorted_missing = missing_ids.clone();
    sorted_missing.sort_by(|a, b| sar_sort_key(a).cmp(&sar_sort_key(b)));

    match attrs.sar_markings.as_ref() {
        Some(_observed) => {
            // Audit content-ignorance: the runtime program list is not in
            // the typed `Message`. `MessageArgs.category =
            // Some(CAT_SAR)` identifies the axis. The canonical
            // replacement still rides on `Diagnostic.text_correction.replacement`.
            let message = Message::new(
                MessageTemplate::BannerRollupMismatch,
                MessageArgs {
                    category: Some(crate::scheme::CAT_SAR),
                    ..MessageArgs::default()
                },
            );
            // Banner has a SAR block. Emit a RIGHT-ALIGNED INSERTION
            // fix at the end of the block so it does not overlap with
            // the SAR program-order rule (whole-block span) or
            // compartment-order rule (last program's span) when they
            // fire on the same marking.
            //
            // Why insertion and not a whole-block rewrite: the engine's
            // overlap guard (`span.end <= boundary`) drops overlapping
            // fixes. A whole-block rewrite covering the same
            // `sar_block_span` as the program-order rule would lose the
            // lexicographic rule-id tiebreaker, silently dropping the
            // missing-program addition. A zero-width span at the block's
            // end byte has `span.start == block_end`, so it sorts FIRST
            // (`span.start DESC`) and its `span.start` becomes the
            // boundary; the program-order rule's subsequent
            // `span.end == block_end` still satisfies `<= boundary` and
            // is kept. Both fixes apply.
            //
            // Single-apply convergence: when both fire, the first apply
            // pass produces `<observed-sorted>/<missing-sorted>` which
            // may not be fully canonical (the inserted missing programs
            // are suffix-appended, not merge-sorted). A second
            // `marque fix` pass repairs that via the program-order rule
            // alone. Net: never loses missing programs, never overflows
            // into the other rules' territory, and converges in ≤2
            // passes.
            let Some(block) = sar_block_span(attrs) else {
                return vec![];
            };
            let insertion_span = Span::new(block.end, block.end);
            // Replacement: `/PROG1/PROG2` — leading slash separates
            // the inserted run from the last existing program
            // per §H.5 p100 bullet 4 (`/` between
            // program identifiers, no interjected spaces).
            let replacement = format!("/{}", sorted_missing.join("/"));

            vec![make_fix_diagnostic(FixDiagnosticParams {
                rule: row.rule_id,
                severity: row.severity,
                source: FixSource::BuiltinRule,
                span: insertion_span,
                message,
                citation: CITATION,
                // Zero-width insertion: `original` is empty to match
                // `span.start..span.end` being a zero-length slice.
                original: String::new(),
                replacement,
                migration_ref: None,
            })]
        }
        None => {
            // No SAR block in the banner at all. Byte-positioning a new
            // block between SCI and AEA from rule context alone is
            // unsafe — report at Error severity with no fix and let a
            // human place the block.
            //
            // Audit content-ignorance: the typed `Message` identifies
            // the banner-rollup mismatch class with category=CAT_SAR.
            // Per-program detail would require a coordinated
            // `MARQUE_AUDIT_SCHEMA` bump.
            let _ = sorted_missing;
            let span = attrs
                .token_spans
                .first()
                .map(|t| t.span)
                .unwrap_or(Span::new(0, 0));
            vec![Diagnostic::new(
                row.rule_id,
                Severity::Error,
                span,
                Message::new(
                    MessageTemplate::BannerRollupMismatch,
                    MessageArgs {
                        category: Some(crate::scheme::CAT_SAR),
                        ..MessageArgs::default()
                    },
                ),
                CITATION,
                None,
            )]
        }
    }
}

/// Collect program identifiers that appear in `expected` but not in
/// `observed`.
///
/// Compares by program identifier only. Compartments and sub-compartments
/// are deliberately NOT compared — per CAPCO-2016 §H.5 p101
/// and §H.5 p99, banner hierarchy depiction below the program
/// level is optional even when portions carry hierarchy. A banner showing
/// `SAR-BP` when a portion shows `SAR-BP-J12` is therefore compliant and
/// must not be flagged.
///
/// Returns borrowed `&str` views into `expected.programs[i].identifier`.
/// The caller uses these only for (a) the diagnostic message and (b)
/// the insertion-fix replacement string; neither path needs the
/// expected-side compartment/sub-compartment hierarchy, so returning
/// owned `SarProgram` clones would be unnecessary allocation.
fn sar_missing_programs<'a>(
    observed: Option<&marque_ism::SarMarking>,
    expected: &'a marque_ism::SarMarking,
) -> Vec<&'a str> {
    let observed_ids: HashSet<&str> = match observed {
        Some(obs) => obs.programs.iter().map(|p| p.identifier.as_str()).collect(),
        None => HashSet::new(),
    };

    expected
        .programs
        .iter()
        .filter(|p| !observed_ids.contains(p.identifier.as_str()))
        .map(|p| p.identifier.as_str())
        .collect()
}
