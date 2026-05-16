// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Span lookups + companion-form inference used by SCI per-system
//! emit helpers and the action layer ([`first_sci_span`],
//! [`last_dissem_span`], [`dissem_token_span`], [`us_level`],
//! [`infer_companion_form`], [`dissem_token_id_for_form`]). Lifted
//! from the monolithic `predicates.rs` per the issue #466 Stage 2
//! PR A leaf split
//! (`claudedocs/refactor-466/stage2_leaves_plan.md`).

use marque_ism::{Classification, TokenKind};
use marque_scheme::TokenId;

use super::super::*;

/// Find the first SCI-system/SCI-control token span in document order.
/// Used as the diagnostic anchor when the rule fires on a portion's SCI
/// block.
pub(crate) fn first_sci_span(attrs: &marque_ism::CanonicalAttrs) -> Option<marque_ism::Span> {
    attrs
        .token_spans
        .iter()
        .find(|t| {
            matches!(
                t.kind,
                TokenKind::SciSystem
                    | TokenKind::SciControl
                    | TokenKind::SciCompartment
                    | TokenKind::SciSubCompartment
            )
        })
        .map(|t| t.span)
}

/// Observed US classification level, if any. Returns `None` for pure
/// foreign classifications (FGI/NATO/JOINT) — SCI-on-foreign is out of
/// §H.4's scope and handled by the foreign-classification rule cluster.
pub(crate) fn us_level(attrs: &marque_ism::CanonicalAttrs) -> Option<Classification> {
    use marque_ism::MarkingClassification;
    match attrs.classification {
        Some(MarkingClassification::Us(c)) => Some(c),
        Some(MarkingClassification::Conflict { us, .. }) => Some(us),
        _ => None,
    }
}

/// Last token span of the IC dissem block (anchors zero-width insertions).
/// Returns `None` when no IC dissem token exists.
pub(crate) fn last_dissem_span(attrs: &marque_ism::CanonicalAttrs) -> Option<marque_ism::Span> {
    attrs
        .token_spans
        .iter()
        .rev()
        .find(|t| t.kind == TokenKind::DissemControl)
        .map(|t| t.span)
}

/// Find the span (and current text) of a specific `DissemControl` token —
/// used when a rule needs to replace e.g. `OC-USGOV` with `OC`.
///
/// PR 9b (T132): walks the unified [`dissem_iter`](marque_ism::CanonicalAttrs::dissem_iter)
/// — which visits `dissem_us` first, then `dissem_nato` — and
/// correlates against the `token_spans` `DissemControl`-kind sequence
/// in document order. The parser emits dissem tokens to
/// `token_spans` once per source occurrence, irrespective of
/// post-parse attribution, so the iteration order through
/// `dissem_iter()` MUST match `token_spans` document order. This
/// holds because `attribute_dissems` partitions but does not
/// re-order: all `dissem_us` tokens come first by construction
/// (every non-NATO classification routes here), and `dissem_nato`
/// is non-empty only on pure-NATO portions where `dissem_us` is
/// empty by spec.
pub(crate) fn dissem_token_span(
    attrs: &marque_ism::CanonicalAttrs,
    target: marque_ism::DissemControl,
) -> Option<(marque_ism::Span, &str)> {
    for (dissem_idx, d) in attrs.dissem_iter().enumerate() {
        if *d == target {
            // Walk token_spans to find the Nth DissemControl.
            let tok = attrs
                .token_spans
                .iter()
                .filter(|t| t.kind == TokenKind::DissemControl)
                .nth(dissem_idx)?;
            return Some((tok.span, tok.text.as_ref()));
        }
    }
    None
}

/// Banner-form vs portion-form companion representation, given the
/// current dissem block. The parser preserves user-written text verbatim
/// in `TokenSpan::text`, so inserting in matching form avoids surprise
/// mixed-form output.
pub(crate) fn infer_companion_form(attrs: &marque_ism::CanonicalAttrs) -> CompanionForm {
    let first = attrs
        .token_spans
        .iter()
        .find(|t| t.kind == TokenKind::DissemControl);
    match first.map(|t| t.text.as_ref()) {
        Some("NF") | Some("OC") | Some("OC-USGOV") => CompanionForm::Abbreviated,
        _ => CompanionForm::Full,
    }
}

/// Map a dissem-control surface form (`"NF"` / `"NOFORN"` / `"OC"` /
/// `"ORCON"` / `"OC-USGOV"` / `"ORCON-USGOV"`) to its CVE `TokenId`.
/// Surface-form distinction (banner abbrev vs portion abbrev vs full)
/// collapses at the canonical layer; the engine's `render_canonical`
/// decides emission form from the inferred companion form at the
/// insertion site. Returns `None` for unrecognized forms — the
/// caller routes those to the no-fix `Severity::Error` path rather
/// than silently substituting NOFORN. In normal flow the catalog
/// rows only ever pass `form.noforn()` or `form.orcon()` which
/// return one of the six recognized surface forms; an unrecognized
/// input represents a programming error (e.g., a new surface form
/// added without updating this lookup), and failing loudly is the
/// correct behavior.
#[inline]
pub(crate) fn dissem_token_id_for_form(token: &str) -> Option<TokenId> {
    match token {
        "NF" | "NOFORN" => Some(TOK_NOFORN),
        "OC" | "ORCON" => Some(TOK_ORCON),
        "OC-USGOV" | "ORCON-USGOV" => Some(TOK_ORCON_USGOV),
        _ => None,
    }
}
