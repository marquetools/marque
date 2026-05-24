// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Live regression coverage for the W002 retirement (closes #470).
//!
//! The pre-retirement `DeclarativeCominglingWarningRule` fired on
//! every US-classified portion that carried an FGI marker, including
//! the §H.7 p123 canonical "Example Portion Mark (when sources are
//! acknowledged, but not segregated from US): `(S//FGI AUS GBR)`"
//! shape. The §H.7 p124 segregation rule the predicate was modeling
//! is conditioned on ICD-206 status — a document-level property the
//! engine has no portion-local way to derive — so the diagnostic
//! produced noise without a useful action on every authorized
//! portion in the corpus (11 firings across 3 documents).
//!
//! These tests pin the post-retirement behavior:
//! 1. The §H.7 p123 single-FGI-source + REL TO canonical shape
//!    emits no W002.
//! 2. The corpus repro shape (US-class + FGI [LIST] + NF) emits
//!    no W002.
//! 3. The ID `"W002"` is no longer in the registered rule set.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::RuleSet;
use marque_scheme::{Citation, SectionLetter, capco};

/// The CAPCO §-citation W002 was modeling — §H.7 p124 (segregation of
/// commingled FGI when ICD-206 status does not apply). W002's
/// retirement (#470) was justified by the engine's inability to
/// derive ICD-206 status from a portion alone; any future rule
/// attempting to fire on that §-citation effectively re-introduces
/// W002 regardless of its predicate-id name. Citation-based assertions
/// replace `"W002"` string comparisons, which would always pass
/// vacuously because no rule's `predicate_id()` is "W002".
const W002_RETIRED_CITATION: Citation = capco(SectionLetter::H, 7, 124);

fn engine() -> Engine {
    let rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![Box::new(CapcoRuleSet::new())];
    Engine::new(Config::default(), rule_sets, CapcoScheme::new()).expect("default CAPCO engine")
}

/// CAPCO-2016 §H.7 p123 "Example Portion Mark (when sources are
/// acknowledged, but not segregated from US): (S//FGI AUS GBR)" —
/// the canonical commingled-with-US-classification form. Pre-#470
/// retirement this minted W002; post-retirement no W002 fires.
#[test]
fn canonical_us_plus_fgi_portion_emits_no_w002() {
    let result = engine().lint(b"(S//FGI DEU//REL TO USA, DEU)");
    // Behavioral assertion: no diagnostic on this §H.7 p123 canonical
    // shape may cite §H.7 p124 — that's the
    // segregation §-citation W002 was modeling and the rationale
    // for its retirement (engine cannot derive ICD-206 status from
    // a portion). Any future rule that re-introduces the W002
    // behavior under a different predicate name would emit a
    // diagnostic citing §H.7 p124 and trip this check.
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.citation != W002_RETIRED_CITATION),
        "W002 retired (closes #470) — canonical (S//FGI [COUNTRY]\
         //REL TO USA, [COUNTRY]) per §H.7 p123 must not produce a \
         diagnostic citing §H.7 p124 (the segregation rule W002 was \
         modeling). Got: {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| (d.rule.predicate_id(), d.citation))
            .collect::<Vec<_>>()
    );
}

/// The corpus shape that drove #470: US-class + FGI [LIST] + NF.
/// Matches the fixture cases `(S//FGI GBR NZL//NF)` and
/// `(S//FGI JPN KOR//NF)` structurally.
#[test]
fn fgi_country_list_portion_emits_no_w002() {
    let result = engine().lint(b"(S//FGI GBR NZL//NF)");
    // Behavioral assertion mirrors `canonical_us_plus_fgi_portion_emits_no_w002`
    // (same retirement rationale per §H.7 p124).
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.citation != W002_RETIRED_CITATION),
        "W002 retired (closes #470) — corpus repro shape \
         (US-class + FGI [LIST] + NF) must not produce a diagnostic \
         citing §H.7 p124 (the segregation rule W002 was modeling). \
         Got: {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| (d.rule.predicate_id(), d.citation))
            .collect::<Vec<_>>()
    );
}

// A `w002_is_not_a_registered_rule_id` registration check is omitted
// for two reasons:
//
// 1. Vacuous on the predicate-id model — `RuleId::new(scheme,
//    predicate_id)` with `predicate_id == "W002"` is not valid by
//    convention (predicate IDs are descriptive English-with-hyphens,
//    not legacy E###/W### codes). The check would always pass.
// 2. The natural tightening — "no registered rule cites §H.7 p124" —
//    is too broad: §H.7 p124 is a legitimate secondary authority for
//    other valid rules (e.g., the banner-rollup walker's SAR rows
//    cite §H.7 p124 alongside their primary §H.5 p101 authority).
//
// The behavioral fixture-based checks above (firing on the two
// canonical W002 repro shapes and asserting no diagnostic has
// `citation == §H.7 p124`) are non-vacuous: they catch
// re-registration of W002 (or any equivalent rule) regardless of
// predicate-id naming, scoped to where W002 used to fire. The
// `post_3b_registration_pin.rs` registered-rule-ID set additionally
// catches the cardinality/identity drift class.
