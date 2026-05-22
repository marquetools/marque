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
    assert!(
        result.diagnostics.iter().all(|d| d.rule.predicate_id() != "W002"),
        "W002 retired (closes #470) — canonical (S//FGI [COUNTRY]\
         //REL TO USA, [COUNTRY]) per §H.7 p123 must not produce a \
         W002 firing. Got: {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

/// The corpus shape that drove #470: US-class + FGI [LIST] + NF.
/// Matches the fixture cases `(S//FGI GBR NZL//NF)` and
/// `(S//FGI JPN KOR//NF)` structurally.
#[test]
fn fgi_country_list_portion_emits_no_w002() {
    let result = engine().lint(b"(S//FGI GBR NZL//NF)");
    assert!(
        result.diagnostics.iter().all(|d| d.rule.predicate_id() != "W002"),
        "W002 retired (closes #470) — corpus repro shape \
         (US-class + FGI [LIST] + NF) must not produce a W002 \
         firing. Got: {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| d.rule.predicate_id())
            .collect::<Vec<_>>()
    );
}

/// The W002 rule ID is no longer registered. Catches a future
/// re-registration regression independent of the count pin in
/// `tests/post_3b_registration_pin.rs`.
#[test]
fn w002_is_not_a_registered_rule_id() {
    let rule_set = CapcoRuleSet::new();
    let registered: Vec<&str> = rule_set.rules().iter().map(|r| r.id().predicate_id()).collect();
    assert!(
        !registered.contains(&"W002"),
        "W002 was retired in the PR closing #470 — re-registering \
         it requires removing the §H.7 p123 authorization rationale \
         from the catalog row tombstone (`scheme/constraints/core_catalog.rs`) \
         and the helper-module header (`scheme/constraints/helpers.rs`). \
         Got: {registered:?}"
    );
}
