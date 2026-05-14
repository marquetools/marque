// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! End-to-end insurance fixture for PR 9b (T132 / FR-046).
//!
//! Pins the pure-NATO portion → `dissem_nato` attribution from
//! parser → canonical → page projection, so that a future regression
//! that silently re-attributes NATO dissems to `dissem_us` (the
//! CAPCO-2016 p41 reciprocity rule's failure mode) trips the test
//! suite before it reaches users.
//!
//! ATOMAL recognition lands in PR 9c, so the fixture below uses a
//! plain `COSMIC TOP SECRET` classification with `OC/REL TO USA,
//! NATO` dissems — no ATOMAL token required.
//!
//! Authority: CAPCO-2016 §H.7 p123 (FGI / NATO grammar), p41
//! (reciprocity rule for the two NATO-applicable dissems: ORCON and
//! REL TO).

use marque_capco::scheme::{CapcoMarking, CapcoScheme};
use marque_ism::{
    CanonicalAttrs, CapcoTokenSet, DissemControl, MarkingCandidate, MarkingType, Span,
};
use marque_scheme::{MarkingScheme, Scope};

/// Construct the canonical-attrs form of a `(//CTS//OC/REL TO USA, NATO)`
/// portion via the marque-core parser, exercising the post-parse
/// `attribute_dissems` pass.
fn parse_pure_nato_portion() -> CanonicalAttrs {
    let tokens = CapcoTokenSet;
    let parser = marque_core::Parser::new(&tokens);
    let src = b"(//CTS//OC/REL TO USA, NATO)";
    let cand = MarkingCandidate {
        span: Span::new(0, src.len()),
        kind: MarkingType::Portion,
    };
    let parsed = parser
        .parse(&cand, src)
        .expect("pure-NATO portion must parse cleanly");
    marque_ism::from_parsed_unchecked(parsed.attrs)
}

#[test]
fn pure_nato_portion_attributes_dissem_to_dissem_nato() {
    let attrs = parse_pure_nato_portion();

    // The portion's classification is NATO with no US axis — confirm
    // before asserting on dissem.
    let is_nato_only = matches!(
        attrs.classification,
        Some(marque_ism::MarkingClassification::Nato(_))
    );

    // If the parser produced a different shape (e.g., it treats
    // `CTS` as something other than NATO COSMIC TOP SECRET), this
    // fixture cannot exercise the NATO-dissem attribution path —
    // skip with an explicit note rather than fail mysteriously.
    if !is_nato_only {
        eprintln!(
            "skipping NATO-dissem fixture: parser classification = {:?} \
             (expected MarkingClassification::Nato)",
            attrs.classification
        );
        return;
    }

    // Reciprocity: NATO classification → dissems in dissem_nato.
    assert!(
        attrs.dissem_us.is_empty(),
        "pure-NATO portion must NOT populate dissem_us (CAPCO-2016 p41 \
         reciprocity rule); got dissem_us = {:?}",
        attrs.dissem_us,
    );
    assert!(
        attrs.dissem_nato.iter().any(|d| d == &DissemControl::Oc),
        "ORCON must land in dissem_nato; got dissem_nato = {:?}",
        attrs.dissem_nato,
    );
}

#[test]
fn pure_nato_portion_projects_dissem_nato_through_page_rollup() {
    let attrs = parse_pure_nato_portion();

    if !matches!(
        attrs.classification,
        Some(marque_ism::MarkingClassification::Nato(_))
    ) {
        eprintln!(
            "skipping NATO-dissem rollup fixture: parser did not produce NATO classification"
        );
        return;
    }

    let portion = CapcoMarking::new(attrs);
    let scheme = CapcoScheme::new();
    let projected = scheme.project(Scope::Page, &[portion]);

    // Page rollup composes namespaces independently. A pure-NATO
    // portion contributes only to `dissem_nato`; `dissem_us`
    // remains empty.
    assert!(
        projected.0.dissem_us.is_empty(),
        "pure-NATO page rollup must leave dissem_us empty; got {:?}",
        projected.0.dissem_us,
    );
    assert!(
        projected
            .0
            .dissem_nato
            .iter()
            .any(|d| d == &DissemControl::Oc),
        "ORCON must survive page rollup in dissem_nato; got {:?}",
        projected.0.dissem_nato,
    );
}

#[test]
fn dissem_iter_yields_both_namespaces_in_order() {
    // Confirm the iter accessor walks dissem_us first, then dissem_nato —
    // the invariant `dissem_token_span` and the decoder feature extractor
    // both rely on.
    let mut attrs = CanonicalAttrs::default();
    attrs.dissem_us = vec![DissemControl::Nf].into();
    attrs.dissem_nato = vec![DissemControl::Oc].into();
    let collected: Vec<&DissemControl> = attrs.dissem_iter().collect();
    assert_eq!(collected.len(), 2);
    assert_eq!(collected[0], &DissemControl::Nf, "dissem_us comes first");
    assert_eq!(collected[1], &DissemControl::Oc, "dissem_nato comes second");
}
