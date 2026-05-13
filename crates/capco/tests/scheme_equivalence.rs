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
    CanonicalAttrs, Classification, CountryCode, DissemControl, JointClassification,
    MarkingClassification, PageContext, SciControl,
};
use marque_scheme::MarkingScheme;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn portion(c: Classification) -> CanonicalAttrs {
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Us(c));
    a
}

fn wrap(attrs: CanonicalAttrs) -> CapcoMarking {
    CapcoMarking::new(attrs)
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
        CountryCode::USA,
        CountryCode::try_new(b"GBR").unwrap(),
        CountryCode::try_new(b"CAN").unwrap(),
    ]
    .into();
    let mut p2 = portion(Classification::Secret);
    p2.rel_to = vec![
        CountryCode::USA,
        CountryCode::try_new(b"GBR").unwrap(),
        CountryCode::try_new(b"DEU").unwrap(),
    ]
    .into();
    let mut p3 = portion(Classification::Secret);
    p3.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();

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
    assert_eq!(expected[0], CountryCode::USA);
}

#[test]
fn project_banner_noforn_supersedes_rel_to() {
    // p1: REL TO USA, GBR
    // p2: NOFORN
    // Banner: REL TO is superseded; dissem contains NF.
    let mut p1 = portion(Classification::Secret);
    p1.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();
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
    attrs.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "capco/noforn-conflicts-rel-to"),
        "expected NOFORN∥REL TO violation, got: {violations:?}"
    );
}

#[test]
fn constraint_noforn_rel_to_conflict_is_silent_when_separate() {
    // NOFORN only — no REL TO → no conflict.
    let mut attrs = portion(Classification::Secret);
    attrs.dissem_controls = vec![DissemControl::Nf].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        !violations
            .iter()
            .any(|v| v.constraint_label == "capco/noforn-conflicts-rel-to"),
        "no conflict expected when only NOFORN is present: {violations:?}"
    );
}

// ---------------------------------------------------------------------------
// HCS constraint tests (CAPCO-2016 §H.4 pp 62–66)
// ---------------------------------------------------------------------------
//
// The HCS sample constraint is `Constraint::Custom("HCS-system-constraints")`,
// dispatched inside `CapcoScheme::validate`. These tests pin each rule in
// the handler:
//
//   - bare HCS (no compartment) is legacy; requires remarking (§H.4 p62).
//   - CONFIDENTIAL//HCS additionally requires originator correction.
//   - HCS-O requires ORCON and must not include ORCON-USGOV (§H.4 p64).
//   - HCS-P requires NOFORN; ORCON or ORCON-USGOV may be used (§H.4 p66).
//   - HCS-O / HCS-P are only authorized for SECRET and TOP SECRET.
//
// Helper: build an CanonicalAttrs with a single structural SCI marking
// `HCS-{compartment}` at the requested classification.
fn hcs_structural(level: Classification, compartment: Option<&str>) -> CanonicalAttrs {
    use marque_ism::{SciCompartment, SciControlBare, SciControlSystem, SciMarking};

    let mut attrs = portion(level);
    let compartments: Box<[SciCompartment]> = match compartment {
        Some(id) => vec![SciCompartment::new(id, Box::new([]))].into_boxed_slice(),
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
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    // After T035, all HCS sub-rule violations carry the catalog label
    // "E010/HCS-system-constraints" (the per-Custom evaluator
    // overrides constraint_label). Sub-rule discrimination moves to
    // the message text per the constraint module's documented
    // contract.
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "E010/HCS-system-constraints"
                && v.message.starts_with("Bare HCS is legacy")),
        "expected bare-HCS legacy violation, got: {violations:?}"
    );
}

#[test]
fn hcs_legacy_confidential_flags_originator_correction() {
    // Legacy `C//HCS`: per CAPCO 2016 §H.4, identify to originator.
    let attrs = hcs_structural(Classification::Confidential, None);

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "E010/HCS-system-constraints"
                && v.message.starts_with("Bare HCS is legacy")),
        "expected bare-HCS legacy violation alongside the confidential flag: {violations:?}"
    );
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "E010/HCS-system-constraints"
                && v.message.starts_with("Legacy CONFIDENTIAL//HCS")),
        "expected CONFIDENTIAL//HCS originator-correction violation: {violations:?}"
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
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "E010/HCS-system-constraints"
                && v.message.starts_with("HCS requires a compartment")),
        "expected projection-only bare-HCS violation: {violations:?}"
    );
}

#[test]
fn hcs_o_without_orcon_fires() {
    // HCS-O on TS without ORCON — ORCON is required.
    let attrs = hcs_structural(Classification::TopSecret, Some("O"));

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "E010/HCS-system-constraints"
                && v.message.contains("HCS-O requires ORCON")),
        "expected HCS-O-requires-ORCON: {violations:?}"
    );
}

#[test]
fn hcs_o_with_orcon_usgov_fires() {
    // HCS-O with ORCON-USGOV is forbidden — must be ORCON only.
    let mut attrs = hcs_structural(Classification::TopSecret, Some("O"));
    attrs.dissem_controls = vec![DissemControl::Oc, DissemControl::OcUsgov].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "E010/HCS-system-constraints"
                && v.message
                    .contains("HCS-O must not be used with ORCON-USGOV")),
        "expected HCS-O-forbids-ORCON-USGOV: {violations:?}"
    );
}

#[test]
fn hcs_o_on_confidential_fires_classification_floor() {
    // HCS-O requires SECRET or TOP SECRET.
    let mut attrs = hcs_structural(Classification::Confidential, Some("O"));
    attrs.dissem_controls = vec![DissemControl::Oc].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "E010/HCS-system-constraints"
                && v.message
                    .contains("HCS-O is only authorized for SECRET and TOP SECRET")),
        "expected HCS-O-classification-floor: {violations:?}"
    );
}

#[test]
fn hcs_o_with_orcon_and_noforn_on_top_secret_is_silent() {
    // All HCS-O rules satisfied: TS classification, ORCON present, no
    // ORCON-USGOV, NOFORN present. Per CAPCO-2016 §H.4 p64 HCS-O
    // requires BOTH ORCON and NOFORN.
    let mut attrs = hcs_structural(Classification::TopSecret, Some("O"));
    attrs.dissem_controls = vec![DissemControl::Oc, DissemControl::Nf].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    let hcs_violations: Vec<_> = violations
        .iter()
        .filter(|v| v.constraint_label == "E010/HCS-system-constraints")
        .collect();
    assert!(
        hcs_violations.is_empty(),
        "no HCS violations expected: {hcs_violations:?}"
    );
}

#[test]
fn hcs_o_without_noforn_fires() {
    // HCS-O requires NOFORN per CAPCO-2016 §H.4 p64
    // ("Relationship(s) to Other Markings: ... Requires ORCON and
    // NOFORN"). ORCON-only at TS without NOFORN must fire
    // HCS-O-requires-NOFORN. This was the gap captured by #304 and
    // resolved by this PR.
    let mut attrs = hcs_structural(Classification::TopSecret, Some("O"));
    attrs.dissem_controls = vec![DissemControl::Oc].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "E010/HCS-system-constraints"
                && v.message.contains("HCS-O requires NOFORN")),
        "expected HCS-O-requires-NOFORN: {violations:?}"
    );
}

#[test]
fn hcs_o_with_noforn_only_fires_for_missing_orcon() {
    // HCS-O with NOFORN but no ORCON: must still fire HCS-O-requires-
    // ORCON. Regression guard ensuring the new HCS-O-requires-NOFORN
    // constraint did not silently dilute the pre-existing
    // HCS-O-requires-ORCON predicate.
    let mut attrs = hcs_structural(Classification::TopSecret, Some("O"));
    attrs.dissem_controls = vec![DissemControl::Nf].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "E010/HCS-system-constraints"
                && v.message.contains("HCS-O requires ORCON")),
        "expected HCS-O-requires-ORCON even with NOFORN present: {violations:?}"
    );
}

#[test]
fn hcs_p_without_noforn_fires() {
    // HCS-P requires NOFORN per CAPCO-2016 §H.4 p66
    // ("Relationship(s) to Other Markings: ... Requires NOFORN").
    // Bare HCS-P at SECRET with no dissem controls populated: must
    // fire HCS-P-requires-NOFORN.
    let attrs = hcs_structural(Classification::Secret, Some("P"));

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "E010/HCS-system-constraints"
                && v.message.contains("HCS-P requires NOFORN")),
        "expected HCS-P-requires-NOFORN: {violations:?}"
    );
}

#[test]
fn hcs_p_with_noforn_is_silent() {
    // HCS-P with NOFORN alone is valid per CAPCO-2016 §H.4 p66:
    // "Requires NOFORN. ORCON or ORCON-USGOV may be used."
    // ORCON / ORCON-USGOV are permitted but not required.
    let mut attrs = hcs_structural(Classification::Secret, Some("P"));
    attrs.dissem_controls = vec![DissemControl::Nf].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    let hcs_violations: Vec<_> = violations
        .iter()
        .filter(|v| v.constraint_label == "E010/HCS-system-constraints")
        .collect();
    assert!(
        hcs_violations.is_empty(),
        "no HCS violations expected: {hcs_violations:?}"
    );
}

#[test]
fn hcs_p_with_orcon_and_noforn_is_silent() {
    // HCS-P with ORCON + NOFORN: valid per §H.4 p66 (ORCON permitted,
    // NOFORN required).
    let mut attrs = hcs_structural(Classification::Secret, Some("P"));
    attrs.dissem_controls = vec![DissemControl::Oc, DissemControl::Nf].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    let hcs_violations: Vec<_> = violations
        .iter()
        .filter(|v| v.constraint_label == "E010/HCS-system-constraints")
        .collect();
    assert!(
        hcs_violations.is_empty(),
        "no HCS violations expected: {hcs_violations:?}"
    );
}

#[test]
fn hcs_p_with_orcon_usgov_and_noforn_is_silent() {
    // HCS-P with ORCON-USGOV + NOFORN: valid per CAPCO-2016 §H.4 p66
    // (ORCON-USGOV permitted, NOFORN required).
    let mut attrs = hcs_structural(Classification::TopSecret, Some("P"));
    attrs.dissem_controls = vec![DissemControl::OcUsgov, DissemControl::Nf].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    let hcs_violations: Vec<_> = violations
        .iter()
        .filter(|v| v.constraint_label == "E010/HCS-system-constraints")
        .collect();
    assert!(
        hcs_violations.is_empty(),
        "no HCS violations expected: {hcs_violations:?}"
    );
}

#[test]
fn hcs_p_with_orcon_only_fires_for_missing_noforn() {
    // HCS-P with ORCON but no NOFORN: must still fire (NOFORN is
    // required per §H.4 p66, regardless of ORCON status). This is
    // the regression guard for the prior over-strict / under-strict
    // predicate that demanded ORCON but did not demand NOFORN.
    let mut attrs = hcs_structural(Classification::Secret, Some("P"));
    attrs.dissem_controls = vec![DissemControl::Oc].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "E010/HCS-system-constraints"
                && v.message.contains("HCS-P requires NOFORN")),
        "expected HCS-P-requires-NOFORN even with ORCON present: {violations:?}"
    );
}

#[test]
fn hcs_p_on_confidential_fires_classification_floor() {
    // HCS-P requires SECRET or TOP SECRET. The fixture supplies
    // ORCON + NOFORN so the only HCS-P violation surfaced is the
    // classification-floor one (§H.4 p66).
    let mut attrs = hcs_structural(Classification::Confidential, Some("P"));
    attrs.dissem_controls = vec![DissemControl::Oc, DissemControl::Nf].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "E010/HCS-system-constraints"
                && v.message
                    .contains("HCS-P is only authorized for SECRET and TOP SECRET")),
        "expected HCS-P-classification-floor: {violations:?}"
    );
}

#[test]
fn constraint_joint_requires_usa_fires_when_usa_missing_from_rel_to() {
    // JOINT classification with USA in its country list but REL TO
    // missing USA (contrived — the parser disallows this at grammar
    // level, but the constraint still has teeth for programmatically
    // constructed markings).
    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into(),
    }));
    attrs.rel_to = vec![CountryCode::try_new(b"GBR").unwrap()].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "capco/joint-requires-usa"),
        "expected JOINT⇒USA violation, got: {violations:?}"
    );
}

// ---------------------------------------------------------------------------
// T035a feedback-round-1 regressions
// ---------------------------------------------------------------------------
//
// These tests pin the predicate narrowing applied after the first
// review pass on PR #70. They guard against the two dyadic-shape
// mismatches that would silently double-fire diagnostics or fire
// diagnostics on valid markings:
//
// - `e021_does_not_fire_on_u_ucni` — `E021/aea-requires-noforn` must
//   only fire on RD/FRD, not the broader `AnyInCategory(CAT_AEA)`
//   which sweeps UCNI in. `U//UCNI` is a valid CAPCO §H.6 marking and
//   must not trip E021.
// - `e021_does_not_fire_on_valid_release_authorized_tfni` —
//   `E021/aea-requires-noforn` must NOT fire on
//   `SECRET//TFNI//REL TO USA, ACGU`, the §H.6 p121 Notional Example 2
//   canonical release-authorized TFNI marking. Pre-PR-3c.A-fixup, the
//   predicate incorrectly lumped TFNI with RD/FRD and would auto-fix
//   this valid marking into a NOFORN-bearing form (Constitution VIII
//   defect).
// - `e015_does_not_fire_on_dual_classification` — `Conflict` is a
//   parser-internal dual-classification state handled by E012 alone;
//   E015's `non-US-requires-dissem` must not also emit on it.

#[test]
fn e021_does_not_fire_on_u_ucni() {
    // `U//UCNI` — valid per CAPCO §H.6 lines 7706+ ("Applicable only
    // to unclassified information"). Legacy `AeaNofornRule` did NOT
    // fire on UCNI; the T035 catalog must match that behavior.
    use marque_ism::AeaMarking;

    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Us(Classification::Unclassified));
    attrs.aea_markings = vec![AeaMarking::DoeUcni].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        !violations
            .iter()
            .any(|v| v.constraint_label == "E021/aea-requires-noforn"),
        "E021 must not fire on U//UCNI (only RD/FRD require NOFORN); got: {violations:?}"
    );
}

#[test]
fn e021_fires_on_s_rd_without_noforn() {
    // Positive anchor for the narrowed predicate: `S//RD` with no
    // NOFORN MUST fire E021.
    use marque_ism::{AeaMarking, RdBlock};

    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Us(Classification::Secret));
    attrs.aea_markings = vec![AeaMarking::Rd(RdBlock::default())].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "E021/aea-requires-noforn"),
        "E021 must fire on S//RD without NOFORN; got: {violations:?}"
    );
}

#[test]
fn e021_does_not_fire_on_valid_release_authorized_tfni() {
    // §H.6 p121 Notional Example 2: `SECRET//TFNI//REL TO USA, ACGU`
    // is a canonical release-authorized TFNI marking. §H.6 p120
    // Relationship clause says only "May only be used with TOP
    // SECRET, SECRET, or CONFIDENTIAL" — silent on NOFORN. §H.6 p121
    // Note 4 explicitly authorizes foreign sharing per IC guidance.
    //
    // Pre-PR-3c.A-fixup, the predicate lumped TFNI with RD/FRD and
    // would have auto-rewritten this valid marking into a
    // NOFORN-bearing form (which would itself trip E054
    // NOFORN ⊥ REL TO). This regression test pins that TFNI does
    // NOT fire E021. Constitution VIII fidelity guard.
    use marque_ism::{AeaMarking, CountryCode};

    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Us(Classification::Secret));
    attrs.aea_markings = vec![AeaMarking::Tfni].into();
    attrs.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"ACGU").unwrap()].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        !violations
            .iter()
            .any(|v| v.constraint_label == "E021/aea-requires-noforn"),
        "E021 must NOT fire on TFNI (release-authorized per §H.6 p121); \
         got: {violations:?}"
    );
}

#[test]
fn e015_does_not_fire_on_dual_classification() {
    // `MarkingClassification::Conflict` is a parser-internal
    // dual-classification state. E012 (`dual-classification`) handles
    // it; E015 (`non-us-requires-dissem`) must NOT also fire — the
    // legacy `NonUsMissingDissemRule` excluded `Conflict` from its
    // predicate. Catching the `Conflict` arm in CAT_NON_US_CLASSIFICATION
    // would make E015 double-emit on every dual-classification
    // marking that lacks dissem.
    use marque_ism::ForeignClassification;

    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Conflict {
        us: Classification::Secret,
        foreign: Box::new(ForeignClassification::Joint(JointClassification {
            level: Classification::Secret,
            countries: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into(),
        })),
    });
    // No dissem_controls, no rel_to — would trigger E015 if Conflict
    // were treated as non-US classification presence.

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        !violations
            .iter()
            .any(|v| v.constraint_label == "E015/non-us-requires-dissem"),
        "E015 must not fire on `Conflict` (that's E012's job); got: {violations:?}"
    );
    // Sanity: E012 should still fire.
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "E012/dual-classification"),
        "E012 should fire on Conflict; got: {violations:?}"
    );
}

// ---------------------------------------------------------------------------
// T035b regressions — E017/E018/E019 retirement + E036 addition
// ---------------------------------------------------------------------------
//
// CAPCO §H.3 line 4140 permits JOINT with SCI (excluding HCS), SAP,
// AEA, FGI, IC and non-IC dissem controls (excluding NOFORN). Line
// 4146 names the two hard exclusions: HCS and NOFORN. The legacy
// E017/E018/E019 rules broadly forbade JOINT+FGI, JOINT+IC dissem
// (except REL TO), and JOINT+non-IC dissem — all over-restrictive.
// T035b retired them and added the narrowed E036 for the HCS case.
// JOINT+NOFORN is covered indirectly by `capco/noforn-conflicts-
// rel-to` + E014's REL TO requirement.

#[test]
fn e036_fires_on_joint_with_bare_hcs() {
    use marque_ism::{SciCompartment, SciControlBare, SciControlSystem, SciMarking};

    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into(),
    }));
    attrs.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();
    // Structural HCS marking (bare: no compartments).
    attrs.sci_markings = vec![SciMarking::new(
        SciControlSystem::Published(SciControlBare::Hcs),
        Box::<[SciCompartment]>::from(Vec::new()),
        None,
    )]
    .into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "E036/joint-conflicts-hcs"),
        "E036 must fire on JOINT+HCS per §H.3 line 4146; got: {violations:?}"
    );
}

#[test]
fn e036_fires_on_joint_with_hcs_p() {
    // §H.3 line 4146 "HCS markings" is plural — covers HCS-P/HCS-O
    // too. `TOK_HCS` in satisfies_attrs matches Hcs|HcsO|HcsP.
    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into(),
    }));
    attrs.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();
    attrs.sci_controls = vec![SciControl::HcsP].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        violations
            .iter()
            .any(|v| v.constraint_label == "E036/joint-conflicts-hcs"),
        "E036 must fire on JOINT+HCS-P; got: {violations:?}"
    );
}

#[test]
fn e036_does_not_fire_on_joint_without_hcs() {
    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into(),
    }));
    attrs.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();
    // SI is permitted with JOINT (§H.3 line 4140 says SCI "excluding
    // HCS" is allowed).
    attrs.sci_controls = vec![SciControl::Si].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        !violations
            .iter()
            .any(|v| v.constraint_label == "E036/joint-conflicts-hcs"),
        "E036 must NOT fire on JOINT+SI (SCI sans HCS is permitted); got: {violations:?}"
    );
}

#[test]
fn e036_does_not_fire_on_non_joint_with_hcs() {
    // US TS//HCS-P is valid. E036 is JOINT-specific.
    use marque_ism::{SciCompartment, SciControlBare, SciControlSystem, SciMarking};

    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Us(Classification::TopSecret));
    attrs.sci_markings = vec![SciMarking::new(
        SciControlSystem::Published(SciControlBare::Hcs),
        vec![SciCompartment::new("P", Box::new([]))].into(),
        None,
    )]
    .into();
    attrs.dissem_controls = vec![DissemControl::Oc].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        !violations
            .iter()
            .any(|v| v.constraint_label == "E036/joint-conflicts-hcs"),
        "E036 must not fire on US+HCS (rule is JOINT-specific); got: {violations:?}"
    );
}

#[test]
fn no_legacy_e017_e018_e019_constraints_in_catalog() {
    // Catalog regression: the retired rule IDs must not reappear.
    // If someone re-adds them (e.g. via a revert), this test
    // catches it before byte-identity drift sets in.
    let scheme = CapcoScheme::new();
    let labels: Vec<&str> = scheme.constraints().iter().map(|c| c.name()).collect();
    for retired in ["E017/", "E018/", "E019/"] {
        assert!(
            !labels.iter().any(|l| l.starts_with(retired)),
            "retired rule {retired} must not have a catalog entry; got: {labels:?}"
        );
    }
}

#[test]
fn constraint_joint_requires_usa_silent_when_usa_present_everywhere() {
    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into(),
    }));
    attrs.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();

    let scheme = CapcoScheme::new();
    let violations = scheme.validate(&CapcoMarking::new(attrs));
    assert!(
        !violations
            .iter()
            .any(|v| v.constraint_label == "capco/joint-requires-usa"),
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
    // Bottom is the default `CanonicalAttrs`.
    assert_eq!(out.0, marque_ism::CanonicalAttrs::default());
}

#[test]
fn scheme_declares_phase3_rewrites() {
    // PR 3b.B (T026b) declared nine rewrites — the retained
    // `noforn-clears-rel-to` plus the eight §3.4.1 / §3.4.3
    // transmutation entries (consultant Entry 6 split into 6a + 6b
    // for D13 single-citation discipline). The two earlier Phase-3
    // stubs (`joint-promotion`, `fgi-absorption`) were retired in
    // PR 3b.B because their semantics are subsumed by entries 1, 3,
    // and 7 with finer-grained, properly-cited transmutations.
    //
    // PR 3c.B Sub-PR 8.F adds two Pattern A NOFORN-supremacy rewrites:
    // `capco/nodis-implies-noforn` (§H.9 p174) and
    // `capco/exdis-implies-noforn` (§H.9 p172). Both are declared
    // BEFORE `noforn-clears-rel-to` in the vec so that `scheme.project`'s
    // sequential scan executes them in the correct topological order
    // (DISSEM-writers before the DISSEM-reader).
    //
    // PR 3c.B Sub-PR 8.F.2 adds two more Pattern A rewrites:
    // `capco/sbu-nf-implies-noforn` (§H.9 p178) and
    // `capco/les-nf-implies-noforn` (§H.9 p185). Inserted after the
    // 8.F entries at positions [2] and [3] (append within the
    // `*-implies-noforn` group) per design-spec §10 Q2 resolution.
    // Total: thirteen.
    //
    // Dual-page-citation pattern (INTENTIONAL, do NOT deduplicate):
    // positions [2] and [11] both cite `"CAPCO-2016 §H.9 p178"`
    // (`sbu-nf-implies-noforn` at [2] — Pattern A page-rewrite;
    // `sbu-nf-transmutes-on-classified-contact` at [11] —
    // transmutation rewrite). Positions [3] and [12] both cite
    // `"CAPCO-2016 §H.9 p185"` (`les-nf-implies-noforn` at [3];
    // `les-nf-transmutes-on-classified-contact` at [12]). The §H.9
    // p178 entry covers both the NF implication and the
    // transmutation in the same source page; the same is true for
    // §H.9 p185. A future reviewer should NOT flag this as a
    // copy-paste error.
    let scheme = CapcoScheme::new();
    let rewrites = scheme.page_rewrites();
    assert_eq!(rewrites.len(), 13);

    let ids: Vec<&str> = rewrites.iter().map(|r| r.id).collect();
    assert_eq!(
        ids,
        [
            "capco/nodis-implies-noforn",
            "capco/exdis-implies-noforn",
            "capco/sbu-nf-implies-noforn",
            "capco/les-nf-implies-noforn",
            "capco/noforn-clears-rel-to",
            "capco/frd-sigma-consolidates-into-rd-sigma",
            "capco/fgi-rollup-on-us-contact",
            "capco/fgi-restricted-rollup-on-us-contact",
            "capco/joint-cross-class-rollup",
            "capco/us-presence-promotes-bare-fgi-attribution",
            "capco/orcon-nato-to-us-orcon-on-us-contact",
            "capco/sbu-nf-transmutes-on-classified-contact",
            "capco/les-nf-transmutes-on-classified-contact",
        ],
        "rewrite declaration order is observable; the scheduler (Phase 3 T031) \
         reorders them by read/write edges. The four Pattern A NOFORN-supremacy \
         rewrites (nodis/exdis/sbu-nf/les-nf-implies-noforn) are declared first \
         in the vec so `scheme.project`'s sequential scan executes them \
         before `noforn-clears-rel-to` — matching the topological order. \
         The scheduler's topological order is also computed at `Engine::new` \
         and confirms the same partial order."
    );

    // Citations point at verified normative passages (Constitution
    // VIII; T035 cleanup of T034's drift into §I-K non-normative
    // sections; T089 retired the line-number form per project memory
    // `feedback_citations_use_page_numbers.md`). Each of the thirteen
    // citations is verifiable in the vendored CAPCO-2016 markdown.
    // PR 3c.B Sub-PR 8.F.2 added two entries at positions [2] and [3],
    // shifting the original nine 8.F + transmutation entries to
    // positions [4..12]. The dual-page-citation pattern noted above
    // ([2] / [11] both §H.9 p178; [3] / [12] both §H.9 p185) is
    // intentional — the page in CAPCO-2016 covers both the Pattern A
    // implication and the transmutation rewrite.
    assert_eq!(rewrites[0].citation, "CAPCO-2016 §H.9 p174");
    assert_eq!(rewrites[1].citation, "CAPCO-2016 §H.9 p172");
    assert_eq!(rewrites[2].citation, "CAPCO-2016 §H.9 p178");
    assert_eq!(rewrites[3].citation, "CAPCO-2016 §H.9 p185");
    assert_eq!(rewrites[4].citation, "CAPCO-2016 §D.2 Table 3 + §H.8 p145");
    assert_eq!(rewrites[5].citation, "CAPCO-2016 §H.6 p113");
    assert_eq!(rewrites[6].citation, "CAPCO-2016 §H.7 p123");
    assert_eq!(rewrites[7].citation, "CAPCO-2016 §H.7 p123");
    assert_eq!(rewrites[8].citation, "CAPCO-2016 §H.3 p57");
    assert_eq!(rewrites[9].citation, "CAPCO-2016 §H.7 p123");
    assert_eq!(rewrites[10].citation, "CAPCO-2016 §H.8 p136");
    assert_eq!(rewrites[11].citation, "CAPCO-2016 §H.9 p178");
    assert_eq!(rewrites[12].citation, "CAPCO-2016 §H.9 p185");
}

#[test]
fn page_rewrite_noforn_clears_rel_to_produces_same_banner() {
    // Semantic smoke test: the declarative rewrite should give the
    // same observable result as PageContext's existing
    // expected_rel_to (which applies the supersession internally).
    use marque_scheme::Scope;

    let mut p1 = portion(Classification::Secret);
    p1.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();
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
    use smol_str::SmolStr;
    let sci1 = vec![SciMarking::new(
        SciControlSystem::Published(SciControlBare::Si),
        vec![SciCompartment::new(
            "G",
            Box::new([SmolStr::from("ABCD")]),
        )]
        .into_boxed_slice(),
        None,
    )]
    .into_boxed_slice();
    let sci2 = vec![SciMarking::new(
        SciControlSystem::Published(SciControlBare::Si),
        vec![SciCompartment::new(
            "G",
            Box::new([SmolStr::from("DEFG")]),
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

// ---------------------------------------------------------------------------
// Phase B: coverage — CapcoScheme trait-getter surface, Default, From,
// scheme.rs helpers reached only through page-rewrite dispatch
// ---------------------------------------------------------------------------

#[test]
fn capco_marking_from_ism_attributes() {
    let attrs = portion(Classification::Secret);
    let m: CapcoMarking = attrs.clone().into();
    assert_eq!(m.0, attrs);
}

#[test]
fn capco_scheme_default_equals_new() {
    let d = CapcoScheme::default();
    let n = CapcoScheme::new();
    // Can't compare schemes directly (no PartialEq on Vec<Category>);
    // check the observable surface.
    assert_eq!(d.name(), n.name());
    assert_eq!(d.schema_version(), n.schema_version());
    assert_eq!(d.categories().len(), n.categories().len());
    assert_eq!(d.constraints().len(), n.constraints().len());
    assert_eq!(d.templates().len(), n.templates().len());
    assert_eq!(d.page_rewrites().len(), n.page_rewrites().len());
}

#[test]
fn capco_scheme_parse_returns_not_implemented() {
    use marque_capco::scheme::CapcoParseError;
    let s = CapcoScheme::new();
    match s.parse("anything") {
        Err(CapcoParseError::NotImplemented) => {}
        other => panic!("expected NotImplemented, got {:?}", other),
    }
}

#[test]
fn capco_scheme_templates_slice_returns_empty_in_phase_a() {
    let s = CapcoScheme::new();
    // Phase A does not model templates.
    assert!(s.templates().is_empty());
}

#[test]
fn capco_marking_meet_narrow_components() {
    use marque_scheme::Lattice;

    // Exercise the CapcoMarking::meet impl (Phase A narrow
    // component-wise min on classification, SCI, dissem).
    let mut a = portion(Classification::Secret);
    a.sci_controls = vec![SciControl::Si, SciControl::Tk].into();
    a.dissem_controls = vec![DissemControl::Nf].into();
    let mut b = portion(Classification::TopSecret);
    b.sci_controls = vec![SciControl::Si].into();
    b.dissem_controls = vec![DissemControl::Nf, DissemControl::Oc].into();

    let m = wrap(a).meet(&wrap(b));
    // classification = min(S, TS) = S (effective_level).
    assert_eq!(m.classification(), Some(Classification::Secret));
    // SCI intersection = {Si}
    assert_eq!(m.0.sci_controls.as_ref(), &[SciControl::Si]);
    // Dissem intersection = {Nf}
    assert_eq!(m.0.dissem_controls.as_ref(), &[DissemControl::Nf]);
}

#[test]
fn capco_marking_meet_with_missing_classification_is_none() {
    use marque_scheme::Lattice;

    // One side has no classification → meet.classification = None.
    let a = CanonicalAttrs::default();
    let mut b = portion(Classification::Secret);
    b.sci_controls = vec![SciControl::Si].into();

    let m = wrap(a).meet(&wrap(b));
    assert!(m.0.classification.is_none());
}

#[test]
fn render_portion_and_render_banner_use_classification() {
    let s = CapcoScheme::new();
    let p = wrap(portion(Classification::Secret));
    assert_eq!(s.render_portion(&p), "S");
    assert_eq!(s.render_banner(&p), "SECRET");
}

#[test]
fn render_portion_and_banner_empty_without_classification() {
    let s = CapcoScheme::new();
    let p = wrap(CanonicalAttrs::default());
    // CanonicalAttrs::default has classification = None.
    assert_eq!(s.render_portion(&p), "");
    assert_eq!(s.render_banner(&p), "");
}

#[test]
fn render_banner_with_joint_classification_renders_canonical_joint_form() {
    use marque_ism::MarkingClassification;

    // PR 3c.B Commit 5: the renderer now emits the §A.6 p15-16
    // canonical JOINT form: leading `//` (occluding the absent US
    // position), `JOINT` indicator, level (banner long form), then
    // the participant countries alpha-sorted per CAPCO-2016 §H.3 p56
    // ("Country trigraph codes are listed alphabetically followed by
    // tetragraph codes in alphabetical order"). USA appears in
    // alphabetical position — NOT pulled to the front. Pre-commit-5
    // the renderer was a Phase A stub that fell back to printing only
    // the US-level string; the canonical form is the per-axis renderer
    // body in `crates/capco/src/render/render_classification.rs`.
    //
    // Authority for the canonical form:
    // - CAPCO-2016 §A.6 p15-16 — leading `//` for non-US / JOINT.
    // - CAPCO-2016 §H.3 p56 line 1258 — JOINT [LIST] alphabetical;
    //   examples on §H.3 p56 ("//JOINT TOP SECRET CAN ISR USA") and
    //   §H.3 p58 ("//JOINT SECRET CAN GBR USA") confirm USA in its
    //   alphabetical slot. The USA-first rule is REL TO-axis only
    //   (§H.8 p150-151).
    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into(),
    }));
    let s = CapcoScheme::new();
    let out = s.render_banner(&wrap(attrs));
    assert_eq!(out, "//JOINT SECRET GBR USA");
}

#[test]
fn project_diff_scope_runs_as_page_rollup() {
    use marque_scheme::Scope;

    // `Scope::Diff` shares the Page rollup path in the current impl —
    // exercise it so the match-arm is covered.
    let mut p1 = portion(Classification::Secret);
    p1.sci_controls = vec![SciControl::Si].into();
    let mut p2 = portion(Classification::TopSecret);
    p2.sci_controls = vec![SciControl::Tk].into();

    let s = CapcoScheme::new();
    let diff_out = s.project(Scope::Diff, &[wrap(p1.clone()), wrap(p2.clone())]);
    let page_out = s.project(Scope::Page, &[wrap(p1), wrap(p2)]);
    assert_eq!(diff_out, page_out);
}

#[test]
fn project_document_scope_runs_as_page_rollup() {
    use marque_scheme::Scope;

    let mut p1 = portion(Classification::Secret);
    p1.sci_controls = vec![SciControl::Si].into();
    let s = CapcoScheme::new();
    let doc = s.project(Scope::Document, &[wrap(p1.clone())]);
    let page = s.project(Scope::Page, &[wrap(p1)]);
    assert_eq!(doc, page);
}

#[test]
fn constraint_joint_without_usa_in_reltop_violates() {
    use marque_ism::{JointClassification, MarkingClassification};

    // JOINT ⇒ USA must be in both classification countries and REL TO.
    // Build a Joint marking with USA in the country list but MISSING
    // from REL TO.
    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into(),
    }));
    // Empty REL TO — this should violate JOINT⇒USA.
    let s = CapcoScheme::new();
    let v = s.validate(&CapcoMarking::new(attrs));
    assert!(
        v.iter()
            .any(|c| c.constraint_label == "capco/joint-requires-usa"),
        "expected JOINT⇒USA violation, got: {:?}",
        v
    );
}

#[test]
fn constraint_joint_with_usa_everywhere_is_silent() {
    use marque_ism::{JointClassification, MarkingClassification};

    let mut attrs = CanonicalAttrs::default();
    attrs.classification = Some(MarkingClassification::Joint(JointClassification {
        level: Classification::Secret,
        countries: vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into(),
    }));
    attrs.rel_to = vec![CountryCode::USA, CountryCode::try_new(b"GBR").unwrap()].into();
    let s = CapcoScheme::new();
    let v = s.validate(&CapcoMarking::new(attrs));
    assert!(
        !v.iter()
            .any(|c| c.constraint_label == "capco/joint-requires-usa"),
        "unexpected JOINT⇒USA violation, got: {:?}",
        v
    );
}
