// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! End-to-end insurance fixture for PR 9b (T132 / FR-046).
//!
//! Pins the pure-NATO portion → `dissem_nato` attribution from
//! parser → canonical → page projection, so that a future regression
//! that silently re-attributes NATO dissems to `dissem_us` (a
//! violation of the CAPCO-2016 §G.2 Table 5 NATO-dissem ARH rule)
//! trips the test suite before it reaches users.
//!
//! ATOMAL recognition lands in PR 9c, so the fixture below uses a
//! plain `COSMIC TOP SECRET` classification with `OC/REL TO USA,
//! NATO` dissems — no ATOMAL token required.
//!
//! Authority: CAPCO-2016 §H.7 p122 (FGI / NATO grammar) and §G.2
//! Table 5 (pp 40-45), which enumerates the two NATO dissemination
//! control markings (ORCON, REL TO / [LIST] ONLY) and directs their
//! ARH to "See US X ARH requirements" — no NATO-specific dissem form
//! exists, so a US-classified marking carrying these tokens routes
//! them to `dissem_us`; the NATO namespace populates only on
//! pure-NATO portions.

use marque_capco::scheme::{CapcoMarking, CapcoScheme};
use marque_ism::{
    CanonicalAttrs, CapcoTokenSet, Classification, DissemControl, MarkingCandidate,
    MarkingClassification, MarkingType, Span,
};
use marque_scheme::{MarkingScheme, Scope};

/// Construct the canonical-attrs form of a `(//CTS//OC/REL TO USA, NATO)`
/// portion via the marque-core parser, exercising the post-parse
/// `attribute_dissems` pass.
///
/// PR 3c.2.B (PM-B-3 second clause): the helper takes `&CapcoScheme`
/// so the page-rollup test that already constructs a scheme for
/// `scheme.project(...)` can reuse it.
fn parse_pure_nato_portion(scheme: &CapcoScheme) -> CanonicalAttrs {
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
    scheme.canonicalize(parsed.attrs)
}

#[test]
fn pure_nato_portion_attributes_dissem_to_dissem_nato() {
    let scheme = CapcoScheme::new();
    let attrs = parse_pure_nato_portion(&scheme);

    // The portion's classification is NATO with no US axis — confirm
    // before asserting on dissem.
    let is_nato_only = matches!(
        attrs.classification,
        Some(marque_ism::MarkingClassification::Nato(_))
    );

    // This is an *insurance* fixture: its entire purpose is to fail
    // loud if the pure-NATO attribution path breaks. Soft-skipping
    // when the parser doesn't recognize CTS as NATO would forfeit
    // that purpose. The CTS classification is pinned at
    // crates/core/src/parser.rs:1681; if this assertion fires the
    // pin has drifted and the load-bearing FR-046 path needs review.
    if !is_nato_only {
        panic!(
            "PR 9b insurance fixture: parser did NOT produce \
             MarkingClassification::Nato for portion `(//CTS//OC/REL TO USA, NATO)`. \
             Got: {:?}. This is a load-bearing test of the FR-046 pure-NATO \
             attribution path; soft-failing forfeits its purpose. CTS \
             classification is pinned at parser.rs:1681.",
            attrs.classification,
        );
    }

    // Reciprocity: NATO classification → dissems in dissem_nato.
    assert!(
        attrs.dissem_us.is_empty(),
        "pure-NATO portion must NOT populate dissem_us (CAPCO-2016 \
         §G.2 Table 5 NATO-dissem ARH rule); got dissem_us = {:?}",
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
    let scheme = CapcoScheme::new();
    let attrs = parse_pure_nato_portion(&scheme);

    if !matches!(
        attrs.classification,
        Some(marque_ism::MarkingClassification::Nato(_))
    ) {
        panic!(
            "PR 9b insurance fixture: parser did NOT produce \
             MarkingClassification::Nato for portion `(//CTS//OC/REL TO USA, NATO)`. \
             Got: {:?}. This rollup test is load-bearing for the FR-046 \
             pure-NATO page-projection path; soft-failing forfeits its \
             purpose. CTS classification is pinned at parser.rs:1681.",
            attrs.classification,
        );
    }

    let portion = CapcoMarking::new(attrs);
    let projected = scheme.project(Scope::Page, &[portion]);

    // ORCON in `dissem_nato` survives the projection (page rollup
    // composes namespaces independently).
    assert!(
        projected
            .0
            .dissem_nato
            .iter()
            .any(|d| d == &DissemControl::Oc),
        "ORCON must survive page rollup in dissem_nato; got {:?}",
        projected.0.dissem_nato,
    );

    // Post-#704: the closure() Row 0 (caveated-default NOFORN) fires
    // on ORCON regardless of which namespace it lives in
    // (`derive_bits` reads `dissem_iter()` across both `dissem_us`
    // and `dissem_nato`). The pre-#704 suppressor that blocked the
    // injection when REL_TO_PRESENT was in the input was retired
    // along with the rest of the `MASK_FDR_DOMINATORS` gate; the
    // §H.8 p145 NOFORN-dominates supersession overlay then clears
    // `rel_to` and strips dominated dissem at the project()
    // boundary. Net effect on this pure-NATO portion: `dissem_us`
    // ends up `[Nf]` (closure-injected, written to `dissem_us` per
    // `apply_closed_bits_to`'s ALL-IC-dissem-axis-is-US convention),
    // `dissem_nato` retains the input ORCON, `rel_to` is cleared.
    //
    // This is a deliberate behavioral change introduced by issue
    // #704: pre-#704 the explicit `REL TO USA, NATO` in the input
    // suppressed the implicit NOFORN default; post-#704 the §H.8
    // p145 strict reading ("NOFORN cannot be used with REL TO")
    // governs. Authority: §H.8 p145 (NOFORN dominates REL TO).
    assert!(
        projected.0.dissem_us.iter().any(|d| d == &DissemControl::Nf),
        "post-#704 §H.8 p145 strict reading: closure injects NOFORN \
         on the ORCON caveat trigger and the overlay does not strip \
         NOFORN itself; got dissem_us = {:?}",
        projected.0.dissem_us,
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

/// PR 9b R2 (Copilot inline review at `render_dissem.rs:74`): dissem
/// render path MUST dedup across namespaces.
///
/// `dissem_iter()` chains `dissem_us` and `dissem_nato`, so a page
/// rollup that contributes the same control from both namespaces
/// (e.g., a US-classified portion with ORCON and a pure-NATO portion
/// with ORCON) would otherwise emit `ORCON/ORCON` — an invalid
/// repeated token.
///
/// Authority: CAPCO-2016 §G.2 Table 5 pp 40-45. Table 5 directs NATO
/// ORCON to "See US ORCON ARH requirements", i.e. they render to the
/// same canonical token regardless of attribution. The banner must
/// carry one `ORCON`, not two.
///
/// The test constructs a `CapcoMarking` directly with the duplicate
/// namespace state that a real page rollup would produce — bypassing
/// the engine for unit-test focus — and asserts that
/// `render_canonical` collapses the duplicate.
#[test]
fn render_dissem_dedups_same_control_across_namespaces() {
    let scheme = CapcoScheme::new();

    // Construct the post-rollup state: SECRET classification, ORCON
    // in both `dissem_us` (from a hypothetical US-classified portion)
    // and `dissem_nato` (from a hypothetical pure-NATO portion). This
    // is exactly what `CapcoScheme::project(Scope::Page, ...)` would
    // produce for that fixture pair.
    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Us(Classification::Secret));
    attrs.dissem_us = vec![DissemControl::Oc].into();
    attrs.dissem_nato = vec![DissemControl::Oc].into();
    let marking = CapcoMarking::new(attrs);

    // Render as a page-scope banner. The dissem axis must emit a
    // single `ORCON`.
    let mut banner = String::new();
    let page_ctx = marque_scheme::RenderContext::new(
        Scope::Page,
        marque_scheme::EmissionForm::Auto,
        marque_scheme::SchemaVersionId::MarqueMvp3,
    );
    scheme
        .render_canonical(&marking, &page_ctx, &mut banner)
        .expect("render_canonical(Scope::Page) must succeed");

    let orcon_count = banner.matches("ORCON").count();
    assert_eq!(
        orcon_count, 1,
        "banner must carry one ORCON, not duplicates from cross-namespace rollup; got banner = {banner:?}",
    );
    // Negative-form sanity: the broken renderer would emit
    // `ORCON/ORCON` literally. Pin that exact substring is absent.
    assert!(
        !banner.contains("ORCON/ORCON"),
        "banner must NOT contain `ORCON/ORCON`; got banner = {banner:?}",
    );

    // Same property at portion scope (portion form uses `OC`).
    let mut portion = String::new();
    let portion_ctx = marque_scheme::RenderContext::new(
        Scope::Portion,
        marque_scheme::EmissionForm::Auto,
        marque_scheme::SchemaVersionId::MarqueMvp3,
    );
    scheme
        .render_canonical(&marking, &portion_ctx, &mut portion)
        .expect("render_canonical(Scope::Portion) must succeed");
    let oc_count = portion.matches("OC").count();
    assert_eq!(
        oc_count, 1,
        "portion must carry one OC, not duplicates; got portion = {portion:?}",
    );
    assert!(
        !portion.contains("OC/OC"),
        "portion must NOT contain `OC/OC`; got portion = {portion:?}",
    );
}
