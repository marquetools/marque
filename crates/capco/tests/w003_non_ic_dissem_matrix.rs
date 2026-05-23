// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! W003 (`capco:page.dissem.non-ic-dissem-in-classified-banner`)
//! coverage matrix ported from `crates/capco/src/_disabled_tests.rs`
//! per issue #722.
//!
//! # Source tests ported (11 of 12)
//!
//! - `w003_fires_on_sbu_in_classified_banner` — §H.9 p176 (SBU
//!   evicted from classified banner).
//! - `w003_does_not_fire_on_unclassified_banner` — negative case.
//! - `w003_fires_on_limdis_in_classified_banner` — §H.9 p170 (LIMDIS
//!   evicted from classified banner).
//! - `w003_does_not_fire_on_exdis_in_classified_banner` — §H.9 p172
//!   propagation case (EXDIS DOES propagate to classified banners).
//! - `w003_does_not_fire_on_nodis_in_classified_banner` — §H.9 p174
//!   propagation case (NODIS DOES propagate).
//! - `w003_fires_on_sbu_nf_in_classified_banner` — §H.9 p178
//!   (SBU-NF literal banner form non-canonical).
//! - `w003_does_not_fire_on_les_in_classified_banner` — §H.9 p181
//!   propagation case (LES DOES propagate).
//! - `w003_does_not_fire_on_les_nf_in_classified_banner` — §H.9 p185
//!   propagation case.
//! - `w003_does_not_fire_on_ssi_in_classified_banner` — §H.9 p189
//!   propagation case.
//! - `w003_fires_on_sbu_in_nato_classified_banner` — NATO-classified
//!   banner is still classified.
//! - `w003_does_not_fire_on_portion` — banner-only-scope guard.
//!
//! # G13 / Message reshape note (PR 3c.2.C C5)
//!
//! Pre-cutover the tests asserted `w003[0].message.contains("SBU")`
//! to identify which non-IC token triggered the diagnostic. Post-
//! cutover `Diagnostic.message: Message` is a closed-template +
//! closed-args carrier — `.contains(...)` is not a method on `Message`,
//! and the per-token text was deliberately removed from the message
//! body by the G13 closure in
//! `NonIcInClassifiedBannerRule::check` (`crates/capco/src/rules/
//! dissem.rs`): the rule drops the `nic` token text after using it
//! for emit-class dispatch (`let _ = nic;`) because the emit class
//! is known without the runtime value. Token-level identification
//! now flows via the diagnostic's
//! `span.as_str(source)` (the byte slice the diagnostic points at),
//! which IS a permitted-identifier surface per Constitution V Principle
//! V (span offsets are on the permitted-identifier list).
//!
//! # Authority
//!
//! CAPCO-2016 §H.9 pp 170-191 (Non-IC Dissemination Control Markings;
//! per-marking templates + propagation rules). Each citation
//! re-verified against `crates/capco/docs/CAPCO-2016.md` at
//! authorship per Constitution VIII.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::{Diagnostic, MessageTemplate};

const W003_PREDICATE: &str = "page.dissem.non-ic-dissem-in-classified-banner";

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

fn w003_diags(source: &str) -> Vec<Diagnostic<CapcoScheme>> {
    lint(source)
        .into_iter()
        .filter(|d| d.rule.predicate_id() == W003_PREDICATE)
        .collect()
}

/// W003 emits `MessageTemplate::NonIcDissemInClassifiedBanner`
/// uniformly across all triggering tokens; per-token identification
/// is via the diagnostic's `span` slicing into the original source.
fn assert_template(diag: &Diagnostic<CapcoScheme>) {
    assert_eq!(
        diag.message.template(),
        MessageTemplate::NonIcDissemInClassifiedBanner,
        "W003 must emit MessageTemplate::NonIcDissemInClassifiedBanner; \
         got: {:?}",
        diag.message.template(),
    );
}

/// Assert the diagnostic's span points at the given expected token
/// text in the source. Replaces the legacy `message.contains(token)`
/// per-token identification that pre-G13-closure used.
fn assert_span_token(diag: &Diagnostic<CapcoScheme>, source: &str, expected: &str) {
    let span_text = diag
        .span
        .as_str(source.as_bytes())
        .expect("W003 span must be valid UTF-8");
    assert_eq!(
        span_text, expected,
        "W003 span must point at the {expected} token; got: {span_text:?} \
         (full source: {source:?})",
    );
}

// ---------------------------------------------------------------------------
// Eviction cases — W003 fires (§H.9 per-marking eviction)
// ---------------------------------------------------------------------------

/// SBU in a classified banner — W003 fires per CAPCO-2016 §H.9 p176
/// (SBU is `U only`; classified docs do NOT carry SBU in the banner;
/// the class adequately protects the SBU content). Re-verified
/// against `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
#[test]
fn w003_fires_on_sbu_in_classified_banner() {
    let src = "SECRET//SBU";
    let diags = w003_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "W003 must fire on SBU in classified banner: {diags:?}"
    );
    assert_template(&diags[0]);
    assert_span_token(&diags[0], src, "SBU");
}

/// LIMDIS in a classified banner — W003 fires per CAPCO-2016 §H.9
/// p170 (LIMDIS is `U only`; on classified docs LIMDIS NOT in
/// banner). Re-verified against `crates/capco/docs/CAPCO-2016.md`
/// per Constitution VIII.
#[test]
fn w003_fires_on_limdis_in_classified_banner() {
    let src = "SECRET//LIMDIS";
    let diags = w003_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "W003 must fire on LIMDIS in classified banner: {diags:?}"
    );
    assert_template(&diags[0]);
    assert_span_token(&diags[0], src, "LIMDIS");
}

/// SBU NOFORN literal banner form in a classified banner — W003
/// fires. Per CAPCO-2016 §H.9 p178 the banner form is `SBU NOFORN`
/// (space-separated; SBU-NF is the portion form). SBU NOFORN is `U
/// only`; classified-banner placement is non-canonical.
#[test]
fn w003_fires_on_sbu_nf_in_classified_banner() {
    let src = "SECRET//SBU NOFORN";
    let diags = w003_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "W003 must fire on SBU NOFORN in classified banner: {diags:?}"
    );
    assert_template(&diags[0]);
}

/// NATO-classified banner is still classified — W003 fires on `SBU`
/// even when the classification slot carries a NATO marking. Tests
/// the W003 trigger's classification-axis breadth: it gates on
/// "banner classification is any classified", not US-only.
#[test]
fn w003_fires_on_sbu_in_nato_classified_banner() {
    let src = "//NS//SBU";
    let diags = w003_diags(src);
    assert_eq!(
        diags.len(),
        1,
        "W003 must fire on SBU in NATO-classified banner: {diags:?}",
    );
    assert_template(&diags[0]);
}

// ---------------------------------------------------------------------------
// Negative cases — W003 stays silent
// ---------------------------------------------------------------------------

/// Unclassified banner — W003 must NOT fire. SBU is valid in
/// unclassified banners (§H.9 p176 roll-up rules).
#[test]
fn w003_does_not_fire_on_unclassified_banner() {
    let diags = w003_diags("UNCLASSIFIED//SBU");
    assert!(
        diags.is_empty(),
        "W003 must not fire on UNCLASSIFIED banner: {diags:?}",
    );
}

/// EXDIS propagates to classified banners per CAPCO-2016 §H.9 p172
/// — W003 must NOT fire. EXDIS is `TS/S/C/U` and requires NOFORN;
/// the EXDIS-in-classified-banner shape is the canonical form, not
/// the eviction shape.
#[test]
fn w003_does_not_fire_on_exdis_in_classified_banner() {
    let diags = w003_diags("SECRET//NOFORN//EXDIS");
    assert!(
        diags.is_empty(),
        "EXDIS propagates to classified banners per §H.9 p172: {diags:?}",
    );
}

/// NODIS propagates to classified banners per CAPCO-2016 §H.9 p174
/// — W003 must NOT fire. NODIS is `TS/S/C/U` and requires NOFORN.
#[test]
fn w003_does_not_fire_on_nodis_in_classified_banner() {
    let diags = w003_diags("SECRET//NOFORN//NODIS");
    assert!(
        diags.is_empty(),
        "NODIS propagates to classified banners per §H.9 p174: {diags:?}",
    );
}

/// LES propagates to classified banners per CAPCO-2016 §H.9 p181
/// — W003 must NOT fire. LES is `TS/S/C/U`.
#[test]
fn w003_does_not_fire_on_les_in_classified_banner() {
    let diags = w003_diags("SECRET//LES");
    assert!(
        diags.is_empty(),
        "LES propagates to classified banners per §H.9 p181: {diags:?}",
    );
}

/// LES-NF (LES NOFORN literal banner form) propagates to classified
/// banners per CAPCO-2016 §H.9 p185.
#[test]
fn w003_does_not_fire_on_les_nf_in_classified_banner() {
    let diags = w003_diags("SECRET//NOFORN//LES");
    assert!(
        diags.is_empty(),
        "LES-NF propagates to classified banners per §H.9 p185: {diags:?}",
    );
}

/// SSI propagates to classified banners per CAPCO-2016 §H.9 p189
/// — W003 must NOT fire. SSI is `TS/S/C/U` regardless of class.
#[test]
fn w003_does_not_fire_on_ssi_in_classified_banner() {
    let diags = w003_diags("SECRET//SSI");
    assert!(
        diags.is_empty(),
        "SSI propagates to classified banners per §H.9 p189: {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// Scope guard — W003 is banner-only
// ---------------------------------------------------------------------------

/// W003 is banner-only per the rule's doc comment ("W003 is banner-
/// only — a non-IC dissem control in a *portion* marking is a
/// page-rewrite concern, not a W003 concern"). Pin the scope guard
/// so a regression that re-emits on portion candidates is caught.
#[test]
fn w003_does_not_fire_on_portion() {
    let diags = w003_diags("(S//SBU)");
    assert!(
        diags.is_empty(),
        "W003 must not fire on portion context (banner-only): {diags:?}",
    );
}
