// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Regression coverage for issue #677.
//!
//! Canonical-form fixtures: each input is already in its authoritative
//! CAPCO-2016 form (banner uses Marking Title or Authorized
//! Abbreviation; portion uses the Register's Portion Mark). The new
//! form-mismatch rules introduced for #677 MUST NOT fire on any of
//! these inputs.
//!
//! These tests pass on the pre-fix staging tree (no form-mismatch
//! rules exist, so canonical inputs trivially produce zero such
//! diagnostics). After Commit 3 lands, they MUST continue to pass —
//! catching any future regression where the rule's `Some(_)`-gated
//! detection accidentally returns true on canonical input.
//!
//! Authority: CAPCO-2016 §D.1 p27 line 560 (Marking Title OR Authorized
//! Abbreviation permitted in banner); §C.1 p25 line 503 (Portion Mark
//! per Register entry); §G.1 Table 4 p38 (Register-closed-set).
//! Re-verified against `crates/capco/docs/CAPCO-2016.md` at authorship
//! per Constitution VIII.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::Diagnostic;

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

fn form_mismatch_diags(diags: &[Diagnostic<CapcoScheme>]) -> Vec<&Diagnostic<CapcoScheme>> {
    diags
        .iter()
        .filter(|d| {
            let pid = d.rule.predicate_id();
            pid == "banner.metadata.uses-portion-form"
                || pid == "portion.metadata.uses-banner-form"
        })
        .collect()
}

fn assert_no_form_mismatch(input: &str) {
    let diags = lint(input);
    let hits = form_mismatch_diags(&diags);
    assert!(
        hits.is_empty(),
        "canonical input {input:?} must not produce any form-mismatch \
         diagnostics; got {hits:?}",
    );
}

// ---------------------------------------------------------------------------
// Banner-direction canonical: full Marking Title or Banner Abbreviation in
// a banner line. Per §D.1 p27 line 560 both are valid.
// ---------------------------------------------------------------------------

#[test]
fn canonical_banner_with_noforn() {
    assert_no_form_mismatch("SECRET//NOFORN");
}

#[test]
fn canonical_banner_with_orcon() {
    assert_no_form_mismatch("SECRET//ORCON");
}

#[test]
fn canonical_banner_with_imcon() {
    assert_no_form_mismatch("SECRET//IMCON");
}

#[test]
fn canonical_banner_with_dea_sensitive() {
    // §G.1 Table 4 p36 lists DEA SENSITIVE with no distinct
    // banner abbreviation — the title `DEA SENSITIVE` is the
    // authoritative banner form. Portion form is `DSEN`.
    assert_no_form_mismatch("SECRET//DEA SENSITIVE");
}

#[test]
fn canonical_banner_with_propin() {
    assert_no_form_mismatch("SECRET//PROPIN");
}

// ---------------------------------------------------------------------------
// Portion-direction canonical: Register Portion Mark in a portion mark.
// ---------------------------------------------------------------------------

#[test]
fn canonical_portion_with_nf() {
    assert_no_form_mismatch("(S//NF)");
}

#[test]
fn canonical_portion_with_oc() {
    assert_no_form_mismatch("(S//OC)");
}

#[test]
fn canonical_portion_with_imc() {
    assert_no_form_mismatch("(S//IMC)");
}

#[test]
fn canonical_portion_with_dsen() {
    assert_no_form_mismatch("(S//DSEN)");
}

#[test]
fn canonical_portion_with_pr() {
    assert_no_form_mismatch("(S//PR)");
}

// ---------------------------------------------------------------------------
// Multi-token markings — every token canonical.
// ---------------------------------------------------------------------------

#[test]
fn canonical_banner_multi_token() {
    // RD is a same-form entry (title `RESTRICTED DATA`, banner `RD`,
    // portion `RD`); NOFORN is banner form. Both canonical for banner.
    assert_no_form_mismatch("SECRET//RD/NOFORN");
}

#[test]
fn canonical_portion_multi_token() {
    // RD same-form; NF portion form. Both canonical for portion.
    assert_no_form_mismatch("(S//RD/NF)");
}

// ---------------------------------------------------------------------------
// SCI canonical: both Authorized Abbreviation (TK) and Marking Title
// (TALENT KEYHOLE) are permitted in banner per §D.1 p27 line 560.
// ---------------------------------------------------------------------------

#[test]
fn canonical_banner_with_tk_abbrev() {
    // TK in banner — the Authorized Abbreviation. CAPCO-2016 §H.4 p85.
    assert_no_form_mismatch("TOP SECRET//TK");
}

#[test]
fn canonical_banner_with_talent_keyhole_title() {
    // TALENT KEYHOLE in banner — the Marking Title. CAPCO-2016 §H.4 p85.
    // banner==portion for TK in MARKING_FORMS (same-form-with-distinct-title);
    // S001 owns title-form-in-banner detection, not the new rules.
    assert_no_form_mismatch("TOP SECRET//TALENT KEYHOLE");
}

#[test]
fn canonical_portion_with_tk() {
    // TK in portion — the Portion Mark. CAPCO-2016 §H.4 p85.
    assert_no_form_mismatch("(TS//TK)");
}

// ---------------------------------------------------------------------------
// Same-form entries (title == banner == portion) — must not fire even
// though MARKING_FORMS has rows for them. The helpers gate on
// `banner != portion`, so these correctly return `None`.
// ---------------------------------------------------------------------------

#[test]
fn canonical_banner_with_fouo() {
    // FOUO is same-form-with-distinct-title; banner==portion.
    assert_no_form_mismatch("UNCLASSIFIED//FOUO");
}

#[test]
fn canonical_portion_with_fouo() {
    assert_no_form_mismatch("(U//FOUO)");
}

#[test]
fn canonical_banner_with_relido() {
    assert_no_form_mismatch("SECRET//RELIDO");
}

#[test]
fn canonical_portion_with_relido() {
    assert_no_form_mismatch("(S//RELIDO)");
}
