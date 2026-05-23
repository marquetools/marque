// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! S003 (`capco:portion.classification.joint-usa-first-style`)
//! citation lockdown pin ported from
//! `crates/capco/src/_disabled_tests.rs` per issue #722.
//!
//! # Source test ported
//!
//! - `s003_citation_frames_as_convention_not_mandate` — citation
//!   lockdown pin: the diagnostic's typed `Citation` MUST anchor at
//!   §H.3 p56 (pure-alpha JOINT ordering). S003's whole design is
//!   to surface the IC-convention USA-first preference at `Info`
//!   severity without conflicting with §H.3's pure-alpha mandate;
//!   pinning the §H.3 p56 anchor keeps the framing
//!   "convention, not mandate" verifiable.
//!
//! # Authority
//!
//! CAPCO-2016 §H.3 p56 (JOINT classification grammar — pure
//! alphabetical ordering of producer country trigraphs). Re-verified
//! against `crates/capco/docs/CAPCO-2016.md` at authorship per
//! Constitution VIII.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::Diagnostic;
use marque_scheme::{SectionLetter, capco};

const S003_PREDICATE: &str = "portion.classification.joint-usa-first-style";

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

fn lint(source: &str) -> Vec<Diagnostic<CapcoScheme>> {
    engine().lint(source.as_bytes()).diagnostics
}

/// Lock down the S003 citation anchor at §H.3 p56. S003 surfaces the
/// IC convention "USA first" at `Severity::Info` — the §H.3
/// prescription is pure-alpha, so framing the suggestion as
/// "convention, not mandate" is what keeps S003 from violating §H.3
/// p56. Re-pinning the citation prevents a citation drift that
/// would silently turn S003 into a CAPCO-mandate enforcer.
///
/// Authority: CAPCO-2016 §H.3 p56 (JOINT classification grammar —
/// alphabetical producer ordering). Re-verified against
/// `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
#[test]
fn s003_citation_anchors_at_h3_p56() {
    // JOINT banner with USA not in the first producer slot triggers
    // S003 (USA present but pure-alpha sort placed it last). S003 is
    // banner-only per the rule's `ctx.marking_type != Banner` guard;
    // the disabled-test fixture used the bare banner form `//JOINT S
    // AUS GBR USA`.
    let src = "//JOINT S AUS GBR USA";
    let diags: Vec<_> = lint(src)
        .into_iter()
        .filter(|d| d.rule.predicate_id() == S003_PREDICATE)
        .collect();
    assert!(
        !diags.is_empty(),
        "S003 must fire on JOINT banner producer list without USA first: \
         {diags:?}",
    );
    assert_eq!(
        diags[0].citation,
        capco(SectionLetter::H, 3, 56),
        "S003 citation must anchor at §H.3 p56 (pure-alpha JOINT \
         ordering — S003 surfaces the IC convention without violating \
         §H.3's mandate); got: {:?}",
        diags[0].citation,
    );
}
