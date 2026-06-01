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
//! Canonical inputs trivially produce zero form-mismatch diagnostics.
//! These tests catch any future regression where the rule's
//! `Some(_)`-gated detection accidentally returns true on canonical
//! input.
//!
//! Authority: CAPCO-2016 §D.1 p27 line 560 (Marking Title OR Authorized
//! Abbreviation permitted in banner); §C.1 p25 line 503 (Portion Mark
//! per Register entry); §G.1 Table 4 p38 (Register-closed-set).
//! Re-verified against `crates/capco/docs/CAPCO-2016.md` at authorship
//! per Constitution VIII.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::CapcoEngine;
use marque_rules::Diagnostic;

fn engine() -> CapcoEngine {
    CapcoEngine::new(
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
            pid == "banner.metadata.uses-portion-form" || pid == "portion.metadata.uses-banner-form"
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

// ---------------------------------------------------------------------------
// Non-IC dissem canonical companions for the broad-scope coverage tests
// in `r677_form_mismatch_pre.rs`. Authority: CAPCO-2016 §H.9 p170 +
// §G.1 Table 4 p36 (LIMDIS banner, DS portion).
// ---------------------------------------------------------------------------

#[test]
fn canonical_banner_with_limdis() {
    // LIMDIS is the §H.9 p170 banner form for LIMITED DISTRIBUTION.
    assert_no_form_mismatch("UNCLASSIFIED//LIMDIS");
}

#[test]
fn canonical_portion_with_ds() {
    // DS is the §H.9 p170 portion form for LIMITED DISTRIBUTION.
    assert_no_form_mismatch("(U//DS)");
}

// ---------------------------------------------------------------------------
// NATO classification canonical companions for the broad-scope coverage
// tests in `r677_form_mismatch_pre.rs`. Authority: CAPCO-2016 §G.1 Table
// 4 p36 + §H.7 p123. A NATO banner line is `//<NATO class>` per §D.1 p27
// line 552-554, so the canonical banner uses a bare `//NATO SECRET`
// shape rather than the US-class-prefixed form.
// ---------------------------------------------------------------------------

#[test]
fn canonical_banner_with_nato_secret() {
    // NATO SECRET is the §G.1 Table 4 p36 banner form.
    assert_no_form_mismatch("//NATO SECRET");
}

#[test]
fn canonical_portion_with_ns() {
    // NS is the §G.1 Table 4 p36 NATO SECRET portion form.
    assert_no_form_mismatch("(//NS)");
}

// ---------------------------------------------------------------------------
// Title-form fallback canonical-no-fire regression. With the
// `find_banner_form_in_portion` walker's `title_to_portion` fallback
// for Marking Title forms (e.g.,
// `TALENT KEYHOLE` in portion → fix to `TK`), the canonical SCI portion
// form `TK` must NOT regress: `banner_to_portion("TK")` returns `None`
// (row has `banner == portion`); `title_to_portion("TK")` returns `None`
// (no MARKING_FORMS row has `title == "TK"`). Both paths return `None`,
// so the walker correctly produces no diagnostic. Authority: §H.4 p85.
// ---------------------------------------------------------------------------

#[test]
fn canonical_portion_with_tk_no_title_fallback_regression() {
    // `(TS//TK)` — canonical SCI portion form. Must not regress
    // when the title-form fallback is wired up.
    assert_no_form_mismatch("(TS//TK)");
}

#[test]
fn canonical_portion_with_oc_no_title_fallback_regression() {
    // `(S//OC)` — canonical IC dissem portion form. Must not regress
    // when the title-form fallback is wired up;
    // `title_to_portion("OC")` returns `None` (no row has
    // `title == "OC"`).
    assert_no_form_mismatch("(S//OC)");
}

#[test]
fn canonical_portion_with_nf_no_title_fallback_regression() {
    // `(S//NF)` — canonical FD&R portion form. Must not regress
    // when the title-form fallback is wired up.
    assert_no_form_mismatch("(S//NF)");
}

// ---------------------------------------------------------------------------
// EYES title-form non-emit regression. The
// `title_to_portion("EYES ONLY")` lookup returns `None` because the
// EYES row has `title == banner == "EYES ONLY"` (the title==banner
// gate inside `title_to_portion` short-circuits). The banner-direction
// `find_portion_form_in_banner` walker keeps its `text == "EYES"` /
// `text == "EYES ONLY"` suppression (E064 owns the FVEY conversion).
// The portion-direction walker has no EYES suppression by design
// (§H.8 p158 says the trigraph list must be carried forward from the
// source banner — Marque cannot synthesize one from a portion alone),
// so `(S//EYES ONLY)` MAY fire form-mismatch via the original
// `banner_to_portion` branch — but the title-form fallback must NOT
// be an additional source of EYES double-emit on the portion side.
// This test pins the bottom-line behavior: the canonical portion form
// `EYES` produces zero form-mismatch diagnostics.
// ---------------------------------------------------------------------------

#[test]
fn canonical_portion_with_eyes_no_title_fallback_regression() {
    // `(S//EYES)` — canonical Five Eyes portion form (§H.8 p157).
    // Both `banner_to_portion("EYES")` and `title_to_portion("EYES")`
    // return `None`; no diagnostic.
    assert_no_form_mismatch("(S//EYES)");
}

// ---------------------------------------------------------------------------
// EYES suppression negative test — banner direction.
// ---------------------------------------------------------------------------

#[test]
fn eyes_only_in_banner_silent_e064_owns_this_axis() {
    // E064 (EyesOnlyConvertToRelToRule) owns the §H.8 p157 + p158
    // cross-axis conversion of `EYES ONLY` in banner position to
    // `REL TO USA, AUS, CAN, GBR, NZL` (FVEY) on derivative use.
    // PortionFormInBannerRule suppresses bare `EYES` / `EYES ONLY`
    // in banner position to keep E064's richer cross-axis intent
    // reachable for the engine's C-1 overlap guard.
    //
    // Negative-test contract: these inputs must remain 0
    // form-mismatch diagnostics even though `portion_to_banner("EYES")`
    // returns `Some("EYES ONLY")` — the suppression is architecturally
    // load-bearing. A future code change that removes the EYES guard
    // at the `PortionFormInBannerRule` token walker would silently
    // regress without this assertion.
    let diags = lint("SECRET//EYES ONLY");
    let hits = form_mismatch_diags(&diags);
    assert!(
        hits.is_empty(),
        "SECRET//EYES ONLY must NOT fire form-mismatch (E064 owns this axis); got {hits:?}",
    );

    let diags = lint("SECRET//EYES");
    let hits = form_mismatch_diags(&diags);
    assert!(
        hits.is_empty(),
        "SECRET//EYES must NOT fire form-mismatch (E064 owns this axis); got {hits:?}",
    );
}
