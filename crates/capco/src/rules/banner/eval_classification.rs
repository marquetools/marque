// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Banner classification mismatch evaluator (E068).

use marque_ism::{CanonicalAttrs, MarkingClassification, Span};
use marque_rules::{Diagnostic, Message, MessageArgs, MessageTemplate};
use marque_scheme::{Citation, SectionLetter, capco};

use crate::scheme::CapcoScheme;

use super::BannerCategoryRow;

/// E068 evaluator: banner classification disagrees with the projected
/// page state. Pure no-fix Error per CAPCO-2016 §H.7 pp123-125
/// reciprocal classification ladder and the worked examples on
/// pp126-129.
///
/// Detection cases (Constitution V audit content-ignorance: no document values
/// interpolated into the message; the message describes the axis
/// only, not the observed or projected values):
///
/// 1. Banner has no classification but page projects one
///    (`(None, Some(_))`). The banner is missing the classification
///    block required by the portions.
/// 2. Banner has a classification but page projects none
///    (`(Some(_), None)`). The banner is over-classified relative to
///    the (empty) projected page state — an odd but possible shape
///    when banner-only candidates appear.
/// 3. Banner classification level disagrees with the projected
///    effective level (`a.effective_level() != b.effective_level()`).
///    Covers the §H.7 p129 worked example case (`TOP SECRET//FGI
///    CAN DEU//NOFORN`) where portions roll up to `TopSecret` but
///    banner observes `Secret`.
/// 4. Banner classification VARIANT disagrees with the projected
///    variant (e.g. `Us(_)` observed when projection is `Fgi(_)` on
///    a pure-foreign page). Compared by [`variant_kind`] via
///    discriminant equality. Covers the §H.7 pp123-125
///    solely-foreign preservation case.
///
/// **Authority**: CAPCO-2016 §H.7 pp123-125 (Precedence Rules for
/// Banner Line Guidance + reciprocal classification grammar). The
/// worked examples on pp126-129 anchor the cross-axis composition.
///
/// **Constitution VII (scheme-adoption boundary)**: this evaluator
/// is scheme-internal (`marque-capco`). No engine-crate touch.
pub(super) fn evaluate_classification_banner_rollup(
    attrs: &CanonicalAttrs,
    page: &marque_ism::ProjectedMarking,
    row: &BannerCategoryRow,
) -> Vec<Diagnostic<CapcoScheme>> {
    // Discriminator helper: distinguish MarkingClassification variants
    // without leaking the contained values. Mirrors
    // `classification_variant_rank` in `marque-capco::lattice` but
    // local to this evaluator (the lattice helper is `pub(crate)`
    // there and we re-derive locally to avoid coupling rule emission
    // to lattice internals).
    fn variant_kind(c: &MarkingClassification) -> u8 {
        match c {
            MarkingClassification::Us(_) => 0,
            MarkingClassification::Fgi(_) => 1,
            MarkingClassification::Nato(_) => 2,
            MarkingClassification::Joint(_) => 3,
            MarkingClassification::Conflict { .. } => 4,
        }
    }

    // Audit content-ignorance: collapse the 4 string-literal reasons into
    // the typed `MessageTemplate::BannerRollupMismatch` with
    // `category=CAT_CLASSIFICATION`. The narrative distinction
    // (missing / over-classified / level-disagrees / variant-disagrees)
    // moves into the rule doc comment; the audit record carries only
    // the closed-set identifier.
    let has_mismatch = match (attrs.classification.as_ref(), page.classification.as_ref()) {
        (None, None) => false,
        (None, Some(_)) | (Some(_), None) => true,
        (Some(observed), Some(projected)) => {
            observed.effective_level() != projected.effective_level()
                || variant_kind(observed) != variant_kind(projected)
        }
    };

    if !has_mismatch {
        return vec![];
    }

    // Span: point at the first token of the banner candidate so the
    // user can locate the offending line. Per Constitution V audit content-ignorance the
    // span is structural metadata, not document content.
    let span = attrs
        .token_spans
        .first()
        .map(|t| t.span)
        .unwrap_or(Span::new(0, 0));

    // Typed Citation anchors at §H.7 p123 (Precedence Rules for
    // Banner Line Guidance + reciprocal classification); worked
    // examples §H.7 pp126-129 cross-referenced in the rule doc.
    const CITATION: Citation = capco(SectionLetter::H, 7, 123);

    vec![Diagnostic::new(
        row.rule_id,
        row.severity,
        span,
        Message::new(
            MessageTemplate::BannerRollupMismatch,
            MessageArgs {
                category: Some(crate::scheme::CAT_CLASSIFICATION),
                ..MessageArgs::default()
            },
        ),
        CITATION,
        None,
    )]
}
