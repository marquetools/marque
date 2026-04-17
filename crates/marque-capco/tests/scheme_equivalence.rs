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
    let actual: std::collections::BTreeSet<_> =
        banner.0.sci_controls.iter().copied().collect();

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
        violations.iter().any(|v| v.constraint_label == "NOFORN∥REL TO"),
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
        !violations.iter().any(|v| v.constraint_label == "NOFORN∥REL TO"),
        "no conflict expected when only NOFORN is present: {violations:?}"
    );
}

#[test]
fn constraint_hcs_requires_noforn_fires_when_noforn_absent() {
    let mut attrs = portion(Classification::TopSecret);
    attrs.sci_controls = vec![SciControl::Hcs].into();
    // No NOFORN set.

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking(attrs));
    assert!(
        violations.iter().any(|v| v.constraint_label == "HCS⇒NOFORN"),
        "expected HCS⇒NOFORN violation, got: {violations:?}"
    );
}

#[test]
fn constraint_hcs_requires_noforn_silent_when_noforn_present() {
    let mut attrs = portion(Classification::TopSecret);
    attrs.sci_controls = vec![SciControl::Hcs].into();
    attrs.dissem_controls = vec![DissemControl::Nf].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking(attrs));
    assert!(
        !violations.iter().any(|v| v.constraint_label == "HCS⇒NOFORN"),
        "no HCS⇒NOFORN violation expected: {violations:?}"
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
    let mut ranks: Vec<u16> = scheme.categories().iter().map(|c| c.ordering_rank).collect();
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
