// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! W004 joint-disunity-collapse rule + JointSet lattice integration tests.
//!
//! Authority (verified 2026-05-15 against CAPCO-2016.md):
//! - §H.3 p56 (JOINT classification grammar).
//! - §H.3 pp55-59 (JOINT worked examples).
//! - §H.3 p57 ("JOINT not carried forward to banner in US documents").
//! - §H.7 p123 (FGI source-acknowledged form for disunity-collapse migration).
//!
//! PR 4b-B Commit 5 (006 T112).

use marque_capco::JointSet;
use marque_ism::{
    CanonicalAttrs, Classification, CountryCode, JointClassification, MarkingClassification,
};

fn cc(s: &str) -> CountryCode {
    CountryCode::try_new(s.as_bytes()).expect("valid trigraph")
}

fn joint_portion(level: Classification, producers: &[&str]) -> CanonicalAttrs {
    let countries: Box<[CountryCode]> = producers
        .iter()
        .map(|s| cc(s))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let mut a = CanonicalAttrs::default();
    a.classification = Some(MarkingClassification::Joint(JointClassification {
        level,
        countries,
    }));
    a
}

#[test]
fn joint_unanimous_two_portions_same_producers_passes_through() {
    // §H.3 worked example: (//JOINT S USA GBR) (//JOINT S USA GBR)
    // → banner //JOINT SECRET USA, GBR.
    let portions = [
        joint_portion(Classification::Secret, &["USA", "GBR"]),
        joint_portion(Classification::Secret, &["USA", "GBR"]),
    ];
    let s = JointSet::from_attrs_iter(&portions);
    match s.to_marking_classification() {
        Some(MarkingClassification::Joint(j)) => {
            assert_eq!(j.level, Classification::Secret);
            assert_eq!(j.countries.len(), 2);
            let codes: Vec<&str> = j.countries.iter().map(|c| c.as_str()).collect();
            assert!(codes.contains(&"USA"));
            assert!(codes.contains(&"GBR"));
        }
        other => panic!("expected Joint marking, got {other:?}"),
    }
}

#[test]
fn joint_unanimous_three_portions_different_levels() {
    // (//JOINT C USA GBR) (//JOINT TS USA GBR) (//JOINT S USA GBR)
    // → banner //JOINT TOP SECRET USA, GBR (OrdMax on level).
    let portions = [
        joint_portion(Classification::Confidential, &["USA", "GBR"]),
        joint_portion(Classification::TopSecret, &["USA", "GBR"]),
        joint_portion(Classification::Secret, &["USA", "GBR"]),
    ];
    let s = JointSet::from_attrs_iter(&portions);
    assert_eq!(s.highest_level(), Some(Classification::TopSecret));
}

#[test]
fn joint_disunity_two_portions_different_producers_collapses_to_fgi() {
    // (//JOINT S USA GBR) (//JOINT S USA CAN)
    // → banner SECRET//FGI CAN GBR + W004 Warn diagnostic.
    let portions = [
        joint_portion(Classification::Secret, &["USA", "GBR"]),
        joint_portion(Classification::Secret, &["USA", "CAN"]),
    ];
    let s = JointSet::from_attrs_iter(&portions);
    assert!(s.is_disunity_collapse());
    let non_us = s.disunity_collapse_non_us_producers().expect("disunity");
    assert!(non_us.contains(&cc("GBR")));
    assert!(non_us.contains(&cc("CAN")));
    // USA is excluded from the non-US producer set; banner FGI block
    // gets only the non-US producers (the US side becomes the
    // primary banner level).
    assert!(!non_us.contains(&cc("USA")));

    // The collapse renders the banner classification as US (highest
    // level observed), not Joint.
    match s.to_marking_classification() {
        Some(MarkingClassification::Us(c)) => {
            assert_eq!(c, Classification::Secret);
        }
        other => panic!("expected Us(Secret), got {other:?}"),
    }
}

#[test]
fn joint_mixed_with_us_portions_no_w004_fires() {
    // §H.3 p57: mixed (JOINT + US) → JOINT does not roll
    // up. Lattice returns Bottom; the existing PageContext path
    // handles FGI migration.
    let mut us = CanonicalAttrs::default();
    us.classification = Some(MarkingClassification::Us(Classification::Secret));
    let portions = [joint_portion(Classification::Secret, &["USA", "GBR"]), us];
    let s = JointSet::from_attrs_iter(&portions);
    assert!(matches!(s, JointSet::Bottom));
}

#[test]
fn joint_disunity_warn_diagnostic_carries_no_document_text() {
    // Constitution V Principle V G13: the W004 diagnostic message
    // must not contain document text. We verify this indirectly by
    // confirming that the JointSet's `disunity_collapse_non_us_producers`
    // returns only canonical CountryCode vocabulary atoms (3-byte
    // trigraphs). The rule emits these as canonical strings, never
    // as input bytes.
    let portions = [
        joint_portion(Classification::Secret, &["USA", "GBR"]),
        joint_portion(Classification::Secret, &["USA", "CAN", "FRA"]),
    ];
    let s = JointSet::from_attrs_iter(&portions);
    let non_us = s.disunity_collapse_non_us_producers().expect("disunity");

    // Every producer must be a valid canonical trigraph (3 bytes
    // ASCII uppercase). The CountryCode type enforces this at the
    // type level (any try_new failure would have caught it in the
    // setup), so this assertion is structural — if it ever fails,
    // CountryCode invariants have eroded.
    for c in non_us {
        let s = c.as_str();
        assert_eq!(s.len(), 3, "non-trigraph CountryCode: {s:?}");
        assert!(s.bytes().all(|b| b.is_ascii_uppercase()));
    }
}

#[test]
fn joint_disunity_union_excludes_usa() {
    // §H.7 p123: FGI is foreign-source; USA never appears in the FGI
    // [LIST] regardless of where it appeared on the source side. The
    // JointSet's `union_non_us_producers` excludes USA by
    // construction.
    let portions = [
        joint_portion(Classification::Secret, &["USA", "GBR"]),
        joint_portion(Classification::Secret, &["USA", "FRA", "DEU"]),
    ];
    let s = JointSet::from_attrs_iter(&portions);
    let non_us = s.disunity_collapse_non_us_producers().unwrap();
    assert!(!non_us.contains(&cc("USA")));
    assert!(non_us.contains(&cc("GBR")));
    assert!(non_us.contains(&cc("FRA")));
    assert!(non_us.contains(&cc("DEU")));
    assert_eq!(non_us.len(), 3);
}
