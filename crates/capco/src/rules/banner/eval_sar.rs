// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! SAR banner-roll-up evaluator.

use std::collections::HashSet;

use marque_ism::{CanonicalAttrs, Span, sar_sort_key};
use marque_rules::{Diagnostic, FixSource, Message, MessageArgs, MessageTemplate, Severity};
use marque_scheme::{Citation, SectionLetter, capco};

use crate::rules::helpers::{FixDiagnosticParams, make_fix_diagnostic, sar_block_span};
use crate::scheme::CapcoScheme;

use super::BannerCategoryRow;

/// SAR banner roll-up evaluator. Verbatim move of the body of
/// `SarBannerRollupRule::check`, parameterized over the page projection
/// and the catalog row (so the row supplies the rule ID + severity).
///
/// PR 9b (T133): reads `&ProjectedMarking` — the field
/// `sar_markings` carries the union of portion-contributed SAR
/// programs in the same shape `PageContext::expected_sar_marking`
/// used to compute.
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
            // PR 3c.2.C C4 / G13: drop the runtime program list from
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
            // fix at the end of the block so it does not overlap
            // with E028 (program-order, whole-block span) or E029
            // (compartment-order, last program's span) when they
            // fire on the same marking.
            //
            // Why insertion and not a whole-block rewrite: the
            // engine's C-1 overlap guard (FR-016 + `span.end <=
            // boundary`) drops overlapping fixes. If E031's fix
            // were a whole-block rewrite covering the same
            // `sar_block_span` as E028, the lexicographic rule-id
            // tiebreaker would favor E028, silently dropping the
            // missing-program addition. A zero-width span at the
            // block's end byte has `span.start == block_end`, so
            // it sorts FIRST under FR-016 (`span.start DESC`) and
            // its `span.start` becomes the boundary; E028's
            // subsequent `span.end == block_end` still satisfies
            // `<= boundary` and is kept. Both fixes apply.
            //
            // Single-apply convergence: when E028 and E031 both
            // fire, the first apply pass produces
            // `<observed-sorted>/<missing-sorted>` which may not
            // be fully canonical (the inserted missing programs
            // are suffix-appended, not merge-sorted). A second
            // `marque fix` pass will detect and repair that via
            // E028 alone. Net: never loses missing programs,
            // never overflows into E028/E029 territory, and
            // converges in ≤2 passes. The prior whole-block
            // fix dropped silently in the overlap case and
            // required 2 passes anyway — this is strictly
            // better.
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
                confidence: 0.9,
                migration_ref: None,
            })]
        }
        None => {
            // No SAR block in the banner at all. Byte-positioning a new
            // block between SCI and AEA from rule context alone is
            // unsafe — report at Error severity with no fix and let a
            // human place the block.
            //
            // G13: the typed `Message` identifies the banner-rollup
            // mismatch class with category=CAT_SAR. Per-program detail
            // would require coordinated `MARQUE_AUDIT_SCHEMA` bump
            // (out of C scope per PM-C-6).
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
