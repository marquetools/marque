// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! E035 (`capco:banner.banner-rollup.sci-portions-roll-up`) — SCI
//! banner-rollup walker hierarchy-mandatory pins ported from
//! `crates/capco/src/_disabled_tests.rs` per issue #722.
//!
//! E035 is a per-row catalog ID emitted by `BannerMatchesProjectedRule`
//! (the E031 walker). Unlike the SAR per-row (E031 SAR row), §H.4
//! contains no hierarchy-optional carve-out for SCI, so the SCI row
//! enforces full hierarchy roll-up (system + compartment +
//! sub-compartment must all appear in the banner). This file pins
//! the SCI-vs-SAR asymmetry — flipping the SCI rows to "hierarchy
//! optional" would break the source-level semantic distinction
//! between §H.4 and §H.5 p101.
//!
//! # Source tests ported
//!
//! - `e035_fires_on_missing_compartment_sci_asymmetry_with_sar` —
//!   portion has `SI-G`, banner has bare `SI`; E035 fires (the SAR
//!   equivalent — portion `SAR-BP-J12` with banner `SAR-BP` — does
//!   NOT fire E031).
//! - `e035_fires_on_missing_sub_compartment_sci_asymmetry_with_sar`
//!   — portion has `SI-G ABCD`, banner has `SI-G`; E035 fires for
//!   the missing sub-compartment.
//! - `e035_cites_h4_p61` — citation lockdown pin (the SCI per-system
//!   "Precedence Rules for Banner Line Guidance" anchor, a typed
//!   `Citation`).
//! - `e035_message_wording_covers_all_hierarchy_levels` — the
//!   message is a closed-template carrier
//!   (`MessageTemplate::BannerRollupMismatch` + category-only
//!   args); the per-message wording is no longer reachable from
//!   `Message`. The structural property the legacy test guarded
//!   ("message describes hierarchy-level breadth") is preserved at
//!   the type level — the closed `MessageTemplate` precludes the
//!   "missing compartments" prose drift. This test pins the
//!   template variant + category and documents the structural
//!   subsumption.
//!
//! # Authority
//!
//! CAPCO-2016 §H.4 p61 (SCI per-system "Precedence Rules for Banner
//! Line Guidance"; the operative banner-roll-up rule under
//! single-citation discipline). §H.5 p101 (SAR hierarchy-optional carve-out
//! — the asymmetry anchor). Each citation re-verified against
//! `crates/capco/docs/CAPCO-2016.md` at authorship per Constitution
//! VIII.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::{Diagnostic, MessageTemplate};
use marque_scheme::{SectionLetter, capco};

const E035_PREDICATE: &str = "banner.banner-rollup.sci-portions-roll-up";

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

fn e035_diags(source: &str) -> Vec<Diagnostic<CapcoScheme>> {
    lint(source)
        .into_iter()
        .filter(|d| d.rule.predicate_id() == E035_PREDICATE)
        .collect()
}

// ---------------------------------------------------------------------------
// SCI hierarchy is mandatory (unlike SAR per §H.5 p101)
// ---------------------------------------------------------------------------

/// SCI/SAR asymmetry lockdown: portion has `SI-G` (system SI,
/// compartment G); banner has bare `SI` (no compartment). E035 MUST
/// fire — this is the exact shape that the E031 SAR row deliberately
/// does NOT fire on (per §H.5 p101 hierarchy-optional carve-out).
/// §H.4 contains no equivalent carve-out for SCI, so E035 enforces
/// full hierarchy roll-up. Flipping this test would break the
/// source-level semantic distinction between §H.4 and §H.5 p101.
///
/// Authority: CAPCO-2016 §H.4 p61 (SCI per-system precedence —
/// mandatory hierarchy). Re-verified against
/// `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
#[test]
fn e035_fires_on_missing_compartment_sci_asymmetry_with_sar() {
    let src = "(TS//SI-G//NF)\nTOP SECRET//SI//NOFORN";
    let diags = e035_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "E035 MUST fire when banner omits compartment G that appears \
         in a portion — SCI has no hierarchy-optional carve-out: \
         {diags:?}",
    );
}

/// Sibling asymmetry test: portion has `SI-G ABCD` (sub-comp ABCD
/// under compartment G); banner has `SI-G` (no sub-compartment).
/// E035 MUST fire for the missing sub-compartment; the SAR
/// equivalent (portion `SAR-BP-J12 K15`, banner `SAR-BP-J12`) does
/// NOT fire E031 per the hierarchy-optional carve-out.
#[test]
fn e035_fires_on_missing_sub_compartment_sci_asymmetry_with_sar() {
    let src = "(TS//SI-G ABCD//NF)\nTOP SECRET//SI-G//NOFORN";
    let diags = e035_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "E035 MUST fire when banner omits sub-compartment ABCD present \
         in a portion: {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// Citation lockdown
// ---------------------------------------------------------------------------

/// The typed `Citation` pins §H.4 p61 — the SCI per-system
/// "Precedence Rules for Banner Line Guidance" anchor (a parallel
/// instance lives at every §H.4 per-system subsection; single-citation
/// discipline). §D.2 p28 restates the same
/// invariant in general-algorithm prose; that cross-reference
/// lives in `evaluate_sci_banner_rollup`'s doc comment, NOT in the
/// typed Citation.
///
/// Authority: CAPCO-2016 §H.4 p61. Re-verified against
/// `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
#[test]
fn e035_cites_h4_p61() {
    let src = "(TS//SI-G//NF)\nTOP SECRET//SI//NOFORN";
    let diags = e035_diags(src);
    assert_eq!(diags.len(), 1, "E035 must fire: {diags:?}");
    assert_eq!(
        diags[0].citation,
        capco(SectionLetter::H, 4, 61),
        "E035 citation must pin §H.4 p61 (SCI per-system Precedence \
         Rules for Banner Line Guidance); got: {:?}",
        diags[0].citation,
    );
}

// ---------------------------------------------------------------------------
// Closed-template message (audit content-ignorance)
// ---------------------------------------------------------------------------

/// The legacy `e035_message_wording_covers_all_hierarchy_levels`
/// test pinned the corrected wording from PR #102 review by
/// asserting `message.contains("systems, compartments, and/or
/// sub-compartments")` and `message.contains("system missing from
/// banner")`. Post-PR-3c.2.C C5 the message is a closed-template
/// carrier (`MessageTemplate` enum + `MessageArgs` struct) — the
/// per-level prose is no longer reachable from `Message` because
/// it was never representable in the closed-args shape (which
/// carries only `TokenId` / `CategoryId` / `Span` / `Confidence` /
/// `FeatureId` — no `String` field).
///
/// The structural property the legacy test guarded — "the message
/// describes hierarchy-level breadth accurately" — is preserved
/// by construction: the closed `MessageTemplate` precludes the
/// "missing compartments" prose drift that PR #102 review caught.
/// This test pins the template variant + category as the structural
/// successor.
///
/// Scenario: portion carries `TK` (entire system); banner carries
/// only `SI`. TK is missing as an ENTIRE SYSTEM — the
/// closed-template path identifies the violation class without
/// formatting per-level prose.
#[test]
fn e035_emits_closed_template_with_sci_category() {
    let src = "(TS//SI/TK//NF)\nTOP SECRET//SI//NOFORN";
    let diags = e035_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "E035 must fire on missing-TK system: {diags:?}"
    );
    assert_eq!(
        diags[0].message.template(),
        MessageTemplate::BannerRollupMismatch,
        "E035 must emit MessageTemplate::BannerRollupMismatch; got: {:?}",
        diags[0].message.template(),
    );
    // The SCI category is the load-bearing axis identifier on the
    // args; the per-hierarchy-level prose is not representable under
    // the closed-template shape.
    let args = diags[0].message.args();
    assert!(
        args.category.is_some(),
        "E035 message args must populate `category` to identify the SCI \
         axis; got: {args:?}",
    );
}
