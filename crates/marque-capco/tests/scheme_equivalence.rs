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
