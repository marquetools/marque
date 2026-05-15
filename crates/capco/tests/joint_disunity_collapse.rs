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

use marque_capco::scheme::CapcoScheme;
use marque_capco::{CapcoRuleSet, JointSet};
use marque_config::Config;
use marque_engine::{Engine, FixedClock};
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
    // up. Lattice returns `Mixed` (PR 4b-B follow-up C-3: was
    // `Bottom` before but that conflated identity with absorption,
    // breaking associativity under grouped joins); the existing
    // PageContext path handles FGI migration.
    let mut us = CanonicalAttrs::default();
    us.classification = Some(MarkingClassification::Us(Classification::Secret));
    let portions = [joint_portion(Classification::Secret, &["USA", "GBR"]), us];
    let s = JointSet::from_attrs_iter(&portions);
    assert!(matches!(s, JointSet::Mixed));
    assert!(s.is_mixed());
    // W004 only fires on DisunityCollapse; Mixed should not fire it.
    assert!(!s.is_disunity_collapse());
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

// ---------------------------------------------------------------------------
// H-2 PR 4b-B follow-up — engine-level W004 tests
// ---------------------------------------------------------------------------
//
// The JointSet unit tests above exercise the lattice type directly.
// The tests below run W004 (`JointDisunityCollapseRule`) through the
// rule/engine path — they verify the diagnostic actually fires,
// carries the expected severity / rule ID, surfaces only canonical
// CountryCode trigraph identifiers in its message (Constitution V
// Principle V G13), and is correctly suppressed on negative cases
// (mixed JOINT+US per §H.3 p57; pure-US pages; pure-JOINT-unanimous
// pages).

fn engine_with_fixed_clock() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(std::time::UNIX_EPOCH)),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

#[test]
fn w004_fires_on_joint_disunity_banner() {
    // Two JOINT-classified portions with disagreeing producer lists
    // appear BEFORE the banner candidate so the engine's
    // PageContext has accumulated them by the time the banner is
    // evaluated. W004 must fire on the banner with rule = "W004",
    // Warn severity, and reference the §H.3 p56 + §H.7 p123 citation.
    let engine = engine_with_fixed_clock();
    let source = b"(//JOINT S USA GBR) first portion.\n\
                   (//JOINT S USA CAN) second portion.\n\
                   SECRET//FGI CAN GBR//NOFORN\n";

    let lint = engine.lint(source);
    let w004 = lint.diagnostics.iter().find(|d| d.rule.as_str() == "W004");
    assert!(
        w004.is_some(),
        "W004 must fire on JOINT-disunity page; diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.as_str())
            .collect::<Vec<_>>()
    );
    let w004 = w004.unwrap();
    assert_eq!(w004.severity, marque_rules::Severity::Warn);
    // H-6 (PR 4b-B follow-up): the assertion message claims "must
    // reference both §H.3 p56 and §H.7 p123" but the pre-fix
    // condition used `||` — passing if either substring was present.
    // The W004 rule's emitted citation is literally
    // `"CAPCO-2016 §H.3 p56 + §H.7 p123"` (see rules.rs:5027) so
    // both substrings must be present; switch to `&&` to enforce
    // what the message already promises. §H.3 p56 (JOINT roll-up)
    // and §H.7 p123 (FGI source-acknowledged transmutation) both
    // ground the W004 transformation; dropping either would break
    // the cross-axis story Constitution VIII expects to survive
    // review.
    assert!(
        w004.citation.contains("§H.3 p56") && w004.citation.contains("§H.7 p123"),
        "W004 citation must reference both §H.3 p56 AND §H.7 p123: {:?}",
        w004.citation
    );
}

#[test]
fn w004_message_contains_only_canonical_trigraphs() {
    // Constitution V Principle V G13: the W004 diagnostic message
    // MUST NOT contain document bytes. The message interpolates only
    // canonical CountryCode trigraphs (vocabulary atoms) and the
    // §-citation literal. Verify by greppping for prose-shape
    // artifacts that would only appear if input bytes leaked.
    let engine = engine_with_fixed_clock();
    // Use a distinctive surrounding prose sentinel that should NEVER
    // appear in any diagnostic message regardless of rule.
    let prose_sentinel = "PROSE_SENTINEL_LEAKED_INTO_DIAGNOSTIC";
    let source = format!(
        "{prose_sentinel} (//JOINT TS USA GBR) first portion.\n\
         {prose_sentinel} (//JOINT TS USA CAN) second portion.\n\
         TOP SECRET//FGI CAN GBR//NOFORN\n"
    );

    let lint = engine.lint(source.as_bytes());
    let w004 = lint
        .diagnostics
        .iter()
        .find(|d| d.rule.as_str() == "W004")
        .expect("W004 must fire on disunity page");
    assert!(
        !w004.message.contains(prose_sentinel),
        "G13 violation: W004 message leaked prose sentinel: {:?}",
        w004.message
    );
    // The message should mention the producer trigraphs (canonical
    // vocabulary atoms — these are 3-letter uppercase codes the
    // CountryCode type guarantees).
    assert!(
        w004.message.contains("CAN") || w004.message.contains("GBR"),
        "W004 message should reference the non-US producer trigraphs: {:?}",
        w004.message
    );
}

#[test]
fn w004_does_not_fire_on_pure_us_page() {
    // No JOINT portions → JointSet::Bottom → W004 must NOT fire.
    let engine = engine_with_fixed_clock();
    let source = b"(S) plain portion one.\n\
                   (S) plain portion two.\n\
                   SECRET\n";
    let lint = engine.lint(source);
    assert!(
        lint.diagnostics.iter().all(|d| d.rule.as_str() != "W004"),
        "W004 must NOT fire on pure-US page; diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn w004_does_not_fire_on_mixed_joint_plus_us() {
    // §H.3 p57: JOINT does not roll up in US documents. The
    // JointSet returns `Mixed` (post-C-3 PR 4b-B follow-up; was
    // Bottom pre-split). W004 must NOT fire — the FGI migration
    // for the JOINT non-US producers rides through the existing
    // PageContext-resident `expected_fgi_marker` path, not through
    // W004's lattice signal.
    let engine = engine_with_fixed_clock();
    let source = b"(//JOINT S USA GBR) joint-classified portion.\n\
                   (S) plain-us-classified portion.\n\
                   SECRET//FGI GBR//NOFORN\n";
    let lint = engine.lint(source);
    assert!(
        lint.diagnostics.iter().all(|d| d.rule.as_str() != "W004"),
        "W004 must NOT fire on mixed JOINT+US page per §H.3 p57; diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn w004_does_not_fire_on_pure_joint_unanimous() {
    // All portions JOINT with identical producer lists → JointSet::
    // UnanimousProducers → W004 must NOT fire (no disunity to
    // surface). The banner shows `//JOINT [class] [LIST]` per §H.3 p56.
    let engine = engine_with_fixed_clock();
    let source = b"(//JOINT S USA GBR) first portion.\n\
                   (//JOINT S USA GBR) second portion.\n\
                   //JOINT SECRET USA, GBR\n";
    let lint = engine.lint(source);
    assert!(
        lint.diagnostics.iter().all(|d| d.rule.as_str() != "W004"),
        "W004 must NOT fire on unanimous-JOINT page; diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.as_str())
            .collect::<Vec<_>>()
    );
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
