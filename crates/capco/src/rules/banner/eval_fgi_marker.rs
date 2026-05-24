// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Banner FGI marker mismatch evaluator (E069).

use marque_ism::{CanonicalAttrs, Span};
use marque_rules::{Diagnostic, Message, MessageArgs, MessageTemplate};
use marque_scheme::{Citation, SectionLetter, capco};

use crate::scheme::CapcoScheme;

use super::BannerCategoryRow;

/// E069 evaluator: banner FGI marker disagrees with the projected
/// page state. Pure no-fix Error.
///
/// Detection cases (Constitution V audit content-ignorance: no country code values
/// interpolated into the message):
///
/// 1. Banner has no FGI marker but page projects one
///    (`(None, Some(_))`). Covers the §H.7 p129 worked example
///    case (`TOP SECRET//FGI CAN DEU//NOFORN`) where the portions
///    carry FGI provenance but the banner omits it.
/// 2. Banner has an FGI marker but page projects none
///    (`(Some(_), None)`). Banner over-claims foreign provenance.
/// 3. Banner FGI variant disagrees with projection — concealed vs
///    acknowledged. Covers the §H.7 p124
///    source-concealed-dominates rule: if any portion is
///    source-concealed, the banner MUST use bare `FGI` without a
///    trigraph list.
/// 4. Banner is acknowledged and projection is acknowledged but the
///    country sets differ. Covers the §H.7 p126 worked example
///    (`TOP SECRET//FGI CAN DEU//REL TO USA, CAN, DEU`) where the
///    union of portion-contributed FGI countries must appear in
///    the banner list.
///
/// **Authority**: CAPCO-2016 §H.7 p124 — *"Use FGI + Register, Annex
/// B trigraph country code(s) ... in the banner line, unless the
/// very fact that the information is foreign government information
/// must be concealed."* Plus the source-concealed-dominates rule on
/// the same page. The §H.7 p126 (`TOP SECRET//FGI CAN DEU//REL TO
/// USA, CAN, DEU`) and §H.7 p129 (`TOP SECRET//FGI CAN DEU//NOFORN`)
/// worked examples anchor the projection.
///
/// **Constitution VII (scheme-adoption boundary)**: scheme-internal;
/// no engine-crate touch.
pub(super) fn evaluate_fgi_marker_banner_rollup(
    attrs: &CanonicalAttrs,
    page: &marque_ism::ProjectedMarking,
    row: &BannerCategoryRow,
) -> Vec<Diagnostic<CapcoScheme>> {
    use marque_ism::FgiMarker;

    // Discriminator helper for FgiMarker variant comparison without
    // touching the country lists (Constitution V audit content-ignorance).
    fn fgi_variant_kind(m: &FgiMarker) -> u8 {
        match m {
            FgiMarker::SourceConcealed => 0,
            FgiMarker::Acknowledged { .. } => 1,
        }
    }

    // Audit content-ignorance: 4 narrative reasons collapse to the typed
    // `MessageTemplate::BannerRollupMismatch` with category =
    // `CAT_FGI_MARKER`. The narrative distinction lives in the rule
    // doc comment.
    let has_mismatch = match (attrs.fgi_marker.as_ref(), page.fgi_marker.as_ref()) {
        (None, None) => false,
        (None, Some(_)) | (Some(_), None) => true,
        (Some(observed), Some(projected)) => {
            if fgi_variant_kind(observed) != fgi_variant_kind(projected) {
                true
            } else {
                // Compare country lists as SETS, not slices. The
                // observed side comes from the parser in textual
                // order (`parse_fgi_marker` pushes tokens left-to-
                // right); the projected side comes from
                // `FgiSet::to_marker()` which iterates a
                // `BTreeSet<CountryCode>` (sorted). Slice equality
                // would false-positive on non-canonically-ordered
                // (but otherwise-equivalent) banner input — e.g.,
                // `FGI NZL GBR` vs projected `[GBR, NZL]`. Ordering
                // is the renderer's concern (canonical form); E069
                // is supposed to fire on a missing or wrong country,
                // not on a valid-but-non-canonically-ordered country
                // list. The `BTreeSet` allocation only runs in this
                // branch, which is per-banner-candidate (O(pages),
                // not O(tokens)).
                use std::collections::BTreeSet;
                let observed_set: BTreeSet<_> = observed.countries().iter().copied().collect();
                let projected_set: BTreeSet<_> = projected.countries().iter().copied().collect();
                observed_set != projected_set
            }
        }
    };

    if !has_mismatch {
        return vec![];
    }

    let span = attrs
        .token_spans
        .first()
        .map(|t| t.span)
        .unwrap_or(Span::new(0, 0));

    // Typed Citation anchors at §H.7 p124 (banner-line FGI roll-up
    // rule + source-concealed-dominates); worked examples §H.7 p126
    // and §H.7 p129 cross-referenced in the rule doc.
    const CITATION: Citation = capco(SectionLetter::H, 7, 124);

    vec![Diagnostic::new(
        row.rule_id,
        row.severity,
        span,
        Message::new(
            MessageTemplate::BannerRollupMismatch,
            MessageArgs {
                category: Some(crate::scheme::CAT_FGI_MARKER),
                ..MessageArgs::default()
            },
        ),
        CITATION,
        None,
    )]
}
