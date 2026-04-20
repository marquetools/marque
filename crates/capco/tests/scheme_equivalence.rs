// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase A equivalence tests: `CapcoScheme::project_banner` agrees with
//! `PageContext::expected_*` on the same inputs, and declarative
//! constraints agree with hand-written rule behavior for the three
//! sample constraints wired in `scheme.rs`.
//!
//! These are the acceptance criterion for the abstraction: if CAPCO's
//! existing behavior falls out of the trait unchanged, the abstraction
//! is the right shape. Phase B (replacing `PageContext` internals) and
//! Phase C (moving rules to declarative constraints) build on this
//! foundation.

use marque_capco::{CapcoMarking, CapcoScheme};
use marque_ism::{
    Classification, DissemControl, IsmAttributes, JointClassification, MarkingClassification,
    PageContext, SciControl, Trigraph,
};
use marque_scheme::MarkingScheme;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn portion(c: Classification) -> IsmAttributes {
    let mut a = IsmAttributes::default();
    a.classification = Some(MarkingClassification::Us(c));
    a
}

fn wrap(attrs: IsmAttributes) -> CapcoMarking {
    CapcoMarking(attrs)
}

// ---------------------------------------------------------------------------
// project_banner equivalence
// ---------------------------------------------------------------------------

#[test]
fn project_banner_classification_matches_max() {
    // Three portions at C, TS, S → banner must be TS (max).
    let portions = vec![
        wrap(portion(Classification::Confidential)),
        wrap(portion(Classification::TopSecret)),
        wrap(portion(Classification::Secret)),
    ];

    // Reference: existing PageContext path.
    let mut ctx = PageContext::new();
    for p in &portions {
        ctx.add_portion(p.0.clone());
    }
    let expected = ctx.expected_classification();

    // Scheme path.
    let scheme = CapcoScheme::new();
    let banner = scheme.project_banner(&portions);

    assert_eq!(expected, Some(Classification::TopSecret));
    assert_eq!(banner.classification(), Some(Classification::TopSecret));
    assert_eq!(banner.classification(), expected);
}

#[test]
fn project_banner_sci_union_matches_pagecontext() {
    // Two portions: SI+TK, and SI+HCS → union SI,TK,HCS.
    let mut p1 = portion(Classification::Secret);
    p1.sci_controls = vec![SciControl::Si, SciControl::Tk].into();
    let mut p2 = portion(Classification::Secret);
    p2.sci_controls = vec![SciControl::Si, SciControl::Hcs].into();

    let portions = vec![wrap(p1), wrap(p2)];

    let mut ctx = PageContext::new();
    for p in &portions {
        ctx.add_portion(p.0.clone());
    }
    let expected: std::collections::BTreeSet<_> = ctx.expected_sci_controls().into_iter().collect();

    let scheme = CapcoScheme::new();
    let banner = scheme.project_banner(&portions);
    let actual: std::collections::BTreeSet<_> = banner.0.sci_controls.iter().copied().collect();

    assert_eq!(actual, expected);
    assert!(actual.contains(&SciControl::Si));
    assert!(actual.contains(&SciControl::Tk));
    assert!(actual.contains(&SciControl::Hcs));
}

#[test]
fn project_banner_rel_to_intersection_matches_pagecontext() {
    // p1: REL TO USA, GBR, CAN
    // p2: REL TO USA, GBR, DEU
    // p3: REL TO USA, GBR
    // Intersection = {USA, GBR}, USA first.
    let mut p1 = portion(Classification::Secret);
    p1.rel_to = vec![
        Trigraph::USA,
        Trigraph::try_new(*b"GBR").unwrap(),
        Trigraph::try_new(*b"CAN").unwrap(),
    ]
    .into();
    let mut p2 = portion(Classification::Secret);
    p2.rel_to = vec![
        Trigraph::USA,
        Trigraph::try_new(*b"GBR").unwrap(),
        Trigraph::try_new(*b"DEU").unwrap(),
    ]
    .into();
    let mut p3 = portion(Classification::Secret);
    p3.rel_to = vec![Trigraph::USA, Trigraph::try_new(*b"GBR").unwrap()].into();

    let portions = vec![wrap(p1), wrap(p2), wrap(p3)];

    let mut ctx = PageContext::new();
    for p in &portions {
        ctx.add_portion(p.0.clone());
    }
    let expected = ctx.expected_rel_to();

    let scheme = CapcoScheme::new();
    let banner = scheme.project_banner(&portions);
    assert_eq!(banner.0.rel_to.as_ref(), expected.as_slice());
    // And the specific shape: USA first, GBR second, nothing else.
    assert_eq!(expected.len(), 2);
    assert_eq!(expected[0], Trigraph::USA);
}

#[test]
fn project_banner_noforn_supersedes_rel_to() {
    // p1: REL TO USA, GBR
    // p2: NOFORN
    // Banner: REL TO is superseded; dissem contains NF.
    let mut p1 = portion(Classification::Secret);
    p1.rel_to = vec![Trigraph::USA, Trigraph::try_new(*b"GBR").unwrap()].into();
    let mut p2 = portion(Classification::Secret);
    p2.dissem_controls = vec![DissemControl::Nf].into();

    let portions = vec![wrap(p1), wrap(p2)];

    let mut ctx = PageContext::new();
    for p in &portions {
        ctx.add_portion(p.0.clone());
    }
    let expected_rel_to = ctx.expected_rel_to();
    let expected_dissem = ctx.expected_dissem_controls();

    let scheme = CapcoScheme::new();
    let banner = scheme.project_banner(&portions);

    // REL TO is wiped by the NOFORN supersession.
    assert!(
        expected_rel_to.is_empty(),
        "reference PageContext expected REL TO to be empty"
    );
    assert!(
        banner.0.rel_to.is_empty(),
        "scheme banner should also have empty REL TO"
    );
    // NF appears in both dissem lists.
    assert!(expected_dissem.contains(&DissemControl::Nf));
    assert!(banner.0.dissem_controls.contains(&DissemControl::Nf));
}

// ---------------------------------------------------------------------------
// Lattice join equivalence: join(a, b) agrees with project_banner([a, b])
// ---------------------------------------------------------------------------

#[test]
fn lattice_join_agrees_with_project_banner_pairwise() {
    use marque_scheme::Lattice;

    let mut p1 = portion(Classification::Confidential);
    p1.sci_controls = vec![SciControl::Si].into();
    let mut p2 = portion(Classification::TopSecret);
    p2.sci_controls = vec![SciControl::Tk].into();

    let a = wrap(p1);
    let b = wrap(p2);

    let scheme = CapcoScheme::new();
    let projected = scheme.project_banner(&[a.clone(), b.clone()]);
    let joined = a.join(&b);

    assert_eq!(projected.classification(), joined.classification());
    let p_sci: std::collections::BTreeSet<_> = projected.0.sci_controls.iter().copied().collect();
    let j_sci: std::collections::BTreeSet<_> = joined.0.sci_controls.iter().copied().collect();
    assert_eq!(p_sci, j_sci);
}

// ---------------------------------------------------------------------------
// Constraint equivalence: declarative constraints produce the expected
// violations for the three sample constraints wired into CapcoScheme.
// ---------------------------------------------------------------------------

#[test]
fn constraint_noforn_rel_to_conflict_fires() {
    // Build a marking that has BOTH NOFORN and a REL TO list.
    let mut attrs = portion(Classification::Secret);
    attrs.dissem_controls = vec![DissemControl::Nf].into();
    attrs.rel_to = vec![Trigraph::USA, Trigraph::try_new(*b"GBR").unwrap()].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "NOFORN∥REL TO"),
        "expected NOFORN∥REL TO violation, got: {violations:?}"
    );
}

#[test]
fn constraint_noforn_rel_to_conflict_is_silent_when_separate() {
    // NOFORN only — no REL TO → no conflict.
    let mut attrs = portion(Classification::Secret);
    attrs.dissem_controls = vec![DissemControl::Nf].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking(attrs));
    assert!(
        !violations
            .iter()
            .any(|v| v.constraint_label == "NOFORN∥REL TO"),
        "no conflict expected when only NOFORN is present: {violations:?}"
    );
}

// ---------------------------------------------------------------------------
// HCS constraint tests (CAPCO 2016 §4 p62)
// ---------------------------------------------------------------------------
//
// The HCS sample constraint is `Constraint::Custom("HCS-system-constraints")`,
// dispatched inside `CapcoScheme::validate`. These tests pin each rule in
// the handler:
//
//   - bare HCS (no compartment) is legacy; requires remarking.
//   - CONFIDENTIAL//HCS additionally requires originator correction.
//   - HCS-O requires ORCON and must not include ORCON-USGOV.
//   - HCS-P requires ORCON or ORCON-USGOV.
//   - HCS-O / HCS-P are only authorized for SECRET and TOP SECRET.
//
// Helper: build an IsmAttributes with a single structural SCI marking
// `HCS-{compartment}` at the requested classification.
fn hcs_structural(level: Classification, compartment: Option<&str>) -> IsmAttributes {
    use marque_ism::{SciCompartment, SciControlBare, SciControlSystem, SciMarking};

    let mut attrs = portion(level);
    let compartments: Box<[SciCompartment]> = match compartment {
        Some(id) => vec![SciCompartment::new(
            id.to_owned().into_boxed_str(),
            Box::new([]),
        )]
        .into_boxed_slice(),
        None => Box::new([]),
    };
    attrs.sci_markings = vec![SciMarking::new(
        SciControlSystem::Published(SciControlBare::Hcs),
        compartments,
        None,
    )]
    .into_boxed_slice();
    attrs
}

#[test]
fn hcs_bare_is_flagged_as_legacy() {
    // Bare HCS without compartment: CAPCO 2016 §4 p62 requires
    // remarking to HCS-P / HCS-O / HCS-O-P.
    let attrs = hcs_structural(Classification::TopSecret, None);

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "HCS-legacy-bare"),
        "expected HCS-legacy-bare, got: {violations:?}"
    );
}

#[test]
fn hcs_legacy_confidential_flags_originator_correction() {
    // Legacy `C//HCS`: per CAPCO 2016 §4, identify to originator.
    let attrs = hcs_structural(Classification::Confidential, None);

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "HCS-legacy-bare"),
        "expected HCS-legacy-bare alongside the confidential flag: {violations:?}"
    );
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "HCS-legacy-confidential"),
        "expected HCS-legacy-confidential: {violations:?}"
    );
}

#[test]
fn hcs_projection_only_bare_still_fires_legacy() {
    // Back-compat path: a portion carrying `SciControl::Hcs` in the
    // projection but no structural entry still gets flagged.
    let mut attrs = portion(Classification::TopSecret);
    attrs.sci_controls = vec![SciControl::Hcs].into();
    // sci_markings intentionally empty.

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "HCS-legacy-bare"),
        "expected HCS-legacy-bare from projection-only path: {violations:?}"
    );
}

#[test]
fn hcs_o_without_orcon_fires() {
    // HCS-O on TS without ORCON — ORCON is required.
    let attrs = hcs_structural(Classification::TopSecret, Some("O"));

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "HCS-O-requires-ORCON"),
        "expected HCS-O-requires-ORCON: {violations:?}"
    );
}

#[test]
fn hcs_o_with_orcon_usgov_fires() {
    // HCS-O with ORCON-USGOV is forbidden — must be ORCON only.
    let mut attrs = hcs_structural(Classification::TopSecret, Some("O"));
    attrs.dissem_controls = vec![DissemControl::Oc, DissemControl::OcUsgov].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "HCS-O-forbids-ORCON-USGOV"),
        "expected HCS-O-forbids-ORCON-USGOV: {violations:?}"
    );
}

#[test]
fn hcs_o_on_confidential_fires_classification_floor() {
    // HCS-O requires SECRET or TOP SECRET.
    let mut attrs = hcs_structural(Classification::Confidential, Some("O"));
    attrs.dissem_controls = vec![DissemControl::Oc].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "HCS-O-classification-floor"),
        "expected HCS-O-classification-floor: {violations:?}"
    );
}

#[test]
fn hcs_o_with_orcon_on_top_secret_is_silent() {
    // All HCS-O rules satisfied: TS classification, ORCON present, no
    // ORCON-USGOV.
    let mut attrs = hcs_structural(Classification::TopSecret, Some("O"));
    attrs.dissem_controls = vec![DissemControl::Oc].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking(attrs));
    let hcs_violations: Vec<_> = violations
        .iter()
        .filter(|v| v.constraint_label.starts_with("HCS-"))
        .collect();
    assert!(
        hcs_violations.is_empty(),
        "no HCS violations expected: {hcs_violations:?}"
    );
}

#[test]
fn hcs_p_without_orcon_or_orcon_usgov_fires() {
    // HCS-P requires at least one of ORCON / ORCON-USGOV.
    let attrs = hcs_structural(Classification::Secret, Some("P"));

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "HCS-P-requires-ORCON-or-ORCON-USGOV"),
        "expected HCS-P-requires-ORCON-or-ORCON-USGOV: {violations:?}"
    );
}

#[test]
fn hcs_p_with_orcon_is_silent() {
    // HCS-P with plain ORCON is valid.
    let mut attrs = hcs_structural(Classification::Secret, Some("P"));
    attrs.dissem_controls = vec![DissemControl::Oc].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking(attrs));
    let hcs_violations: Vec<_> = violations
        .iter()
        .filter(|v| v.constraint_label.starts_with("HCS-"))
        .collect();
    assert!(
        hcs_violations.is_empty(),
        "no HCS violations expected: {hcs_violations:?}"
    );
}

#[test]
fn hcs_p_with_orcon_usgov_is_silent() {
    // HCS-P with ORCON-USGOV (no plain ORCON) is valid per CAPCO 2016 §4.
    let mut attrs = hcs_structural(Classification::TopSecret, Some("P"));
    attrs.dissem_controls = vec![DissemControl::OcUsgov].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking(attrs));
    let hcs_violations: Vec<_> = violations
        .iter()
        .filter(|v| v.constraint_label.starts_with("HCS-"))
        .collect();
    assert!(
        hcs_violations.is_empty(),
        "no HCS violations expected: {hcs_violations:?}"
    );
}

#[test]
fn hcs_p_on_confidential_fires_classification_floor() {
    // HCS-P requires SECRET or TOP SECRET.
    let mut attrs = hcs_structural(Classification::Confidential, Some("P"));
    attrs.dissem_controls = vec![DissemControl::Oc].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "HCS-P-classification-floor"),
        "expected HCS-P-classification-floor: {violations:?}"
    );
}

#[test]
fn constraint_joint_requires_usa_fires_when_usa_missing_from_rel_to() {
    // JOINT classification with USA in its country list but REL TO
    // missing USA (contrived — the parser disallows this at grammar
    // level, but the constraint still has teeth for programmatically
    // constructed markings).
    let mut attrs = IsmAttributes::default();
    attrs.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: vec![Trigraph::USA, Trigraph::try_new(*b"GBR").unwrap()].into(),
    }));
    attrs.rel_to = vec![Trigraph::try_new(*b"GBR").unwrap()].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking(attrs));
    assert!(
        violations.iter().any(|v| v.constraint_label == "JOINT⇒USA"),
        "expected JOINT⇒USA violation, got: {violations:?}"
    );
}

#[test]
fn constraint_joint_requires_usa_silent_when_usa_present_everywhere() {
    let mut attrs = IsmAttributes::default();
    attrs.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: vec![Trigraph::USA, Trigraph::try_new(*b"GBR").unwrap()].into(),
    }));
    attrs.rel_to = vec![Trigraph::USA, Trigraph::try_new(*b"GBR").unwrap()].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking(attrs));
    assert!(
        !violations.iter().any(|v| v.constraint_label == "JOINT⇒USA"),
        "no JOINT⇒USA violation expected: {violations:?}"
    );
}

// ---------------------------------------------------------------------------
// Shape sanity: the scheme's category list is internally consistent.
// ---------------------------------------------------------------------------

#[test]
fn scheme_categories_have_distinct_ids_and_ordered_ranks() {
    let scheme = CapcoScheme::new();
    let mut ids: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for cat in scheme.categories() {
        assert!(ids.insert(cat.id.0), "duplicate category id: {:?}", cat.id);
    }

    // Ranks are strictly increasing across our eight categories (so no
    // two categories collide at the same render position).
    let mut ranks: Vec<u16> = scheme
        .categories()
        .iter()
        .map(|c| c.ordering_rank)
        .collect();
    ranks.sort();
    for w in ranks.windows(2) {
        assert!(w[0] < w[1], "ordering_rank values collide: {ranks:?}");
    }
}

#[test]
fn scheme_identity_fields_plausible() {
    let scheme = CapcoScheme::new();
    assert_eq!(scheme.name(), "CAPCO-ISM");
    // Schema version is the one baked into marque-ism. Just check it
    // isn't empty — the exact value bumps with ODNI releases.
    assert!(!scheme.schema_version().is_empty());
}

// ---------------------------------------------------------------------------
// Phase B: Scope-parameterized projection + PageRewrite declaration
// ---------------------------------------------------------------------------

#[test]
fn project_page_scope_equivalent_to_project_banner() {
    use marque_scheme::Scope;

    // project_banner is a Phase A shim that delegates to
    // project(Scope::Page, ...). Both should produce byte-identical
    // results on the same inputs.
    let mut p1 = portion(Classification::Confidential);
    p1.sci_controls = vec![SciControl::Si].into();
    let mut p2 = portion(Classification::TopSecret);
    p2.sci_controls = vec![SciControl::Tk].into();

    let portions = vec![wrap(p1), wrap(p2)];
    let scheme = CapcoScheme::new();
    let banner_from_shim = scheme.project_banner(&portions);
    let banner_from_scope = scheme.project(Scope::Page, &portions);

    assert_eq!(banner_from_shim, banner_from_scope);
}

#[test]
fn project_portion_scope_is_identity() {
    use marque_scheme::Scope;

    let scheme = CapcoScheme::new();
    let only = wrap(portion(Classification::Secret));
    let out = scheme.project(Scope::Portion, std::slice::from_ref(&only));
    assert_eq!(out, only);
}

#[test]
fn project_portion_scope_empty_returns_bottom() {
    use marque_scheme::Scope;

    let scheme = CapcoScheme::new();
    let out = scheme.project(Scope::Portion, &[]);
    // Bottom is the default `IsmAttributes`.
    assert_eq!(out.0, marque_ism::IsmAttributes::default());
}

#[test]
fn scheme_declares_noforn_clears_rel_to_rewrite() {
    let scheme = CapcoScheme::new();
    let rewrites = scheme.page_rewrites();
    assert_eq!(rewrites.len(), 1);
    assert_eq!(rewrites[0].id, "capco/noforn-clears-rel-to");
    assert_eq!(rewrites[0].citation, "CAPCO-2016-§H.2");
}

#[test]
fn page_rewrite_noforn_clears_rel_to_produces_same_banner() {
    // Semantic smoke test: the declarative rewrite should give the
    // same observable result as PageContext's existing
    // expected_rel_to (which applies the supersession internally).
    use marque_scheme::Scope;

    let mut p1 = portion(Classification::Secret);
    p1.rel_to = vec![Trigraph::USA, Trigraph::try_new(*b"GBR").unwrap()].into();
    let mut p2 = portion(Classification::Secret);
    p2.dissem_controls = vec![DissemControl::Nf].into();

    let portions = vec![wrap(p1), wrap(p2)];
    let scheme = CapcoScheme::new();
    let banner = scheme.project(Scope::Page, &portions);

    // After the page rewrite, REL TO should be empty; NF should
    // appear in dissem.
    assert!(banner.0.rel_to.is_empty());
    assert!(banner.0.dissem_controls.contains(&DissemControl::Nf));
}

// ---------------------------------------------------------------------------
// Phase B: SciSet lattice round-trip with PageContext::expected_sci_markings
// ---------------------------------------------------------------------------

#[test]
fn sci_set_from_to_roundtrip_agrees_with_page_context() {
    use marque_capco::SciSet;
    use marque_ism::{SciCompartment, SciControlBare, SciControlSystem, SciMarking};

    // Build two portions, both with SI-G plus sub-compartments; the
    // rollup should union them.
    let sci1 = vec![SciMarking::new(
        SciControlSystem::Published(SciControlBare::Si),
        vec![SciCompartment::new(
            "G".to_string().into_boxed_str(),
            vec!["ABCD".to_string().into_boxed_str()].into_boxed_slice(),
        )]
        .into_boxed_slice(),
        None,
    )]
    .into_boxed_slice();
    let sci2 = vec![SciMarking::new(
        SciControlSystem::Published(SciControlBare::Si),
        vec![SciCompartment::new(
            "G".to_string().into_boxed_str(),
            vec!["DEFG".to_string().into_boxed_str()].into_boxed_slice(),
        )]
        .into_boxed_slice(),
        None,
    )]
    .into_boxed_slice();

    let mut p1 = portion(Classification::Secret);
    p1.sci_markings = sci1.clone();
    let mut p2 = portion(Classification::Secret);
    p2.sci_markings = sci2.clone();

    // Lattice path.
    let set1 = SciSet::from_markings(&p1.sci_markings);
    let set2 = SciSet::from_markings(&p2.sci_markings);
    let joined = marque_scheme::Lattice::join(&set1, &set2);
    let from_lattice = joined.to_markings();

    // PageContext path.
    let mut ctx = marque_ism::PageContext::new();
    ctx.add_portion(p1);
    ctx.add_portion(p2);
    let from_pagectx = ctx.expected_sci_markings();

    assert_eq!(from_lattice, from_pagectx);
}

// ---------------------------------------------------------------------------
// Phase B: Category::shape() returns the expected descriptors
// ---------------------------------------------------------------------------

#[test]
fn category_shapes_are_inspectable() {
    use marque_capco::scheme::{
        CAT_CLASSIFICATION, CAT_DECLASSIFY_ON, CAT_DISSEM, CAT_REL_TO, CAT_SAR,
    };
    use marque_scheme::CategoryShape;

    let scheme = CapcoScheme::new();
    let cats = scheme.categories();

    let by_id = |id| cats.iter().find(|c| c.id == id).unwrap();

    assert_eq!(by_id(CAT_CLASSIFICATION).shape(), CategoryShape::Ordinal);
    assert_eq!(by_id(CAT_DISSEM).shape(), CategoryShape::FlatSet);
    assert_eq!(by_id(CAT_REL_TO).shape(), CategoryShape::IntersectSet);
    assert_eq!(by_id(CAT_DECLASSIFY_ON).shape(), CategoryShape::Date);
    // SAR is a structural category with a bespoke lattice (SarSet).
    assert_eq!(by_id(CAT_SAR).shape(), CategoryShape::Custom);
}
