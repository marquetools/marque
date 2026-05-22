// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! TDD RED-state tests for issue #677.
//!
//! PR 3c.B Commit 6 retired E001 (`PortionMarkInBannerRule`) and E009
//! (banner→portion form normalization) on the premise that
//! `MarkingScheme::render_canonical` would absorb their fix paths. The
//! renderer's fix path IS in place — but no rule emits the
//! `Recanonicalize` `FixIntent` that would trigger it, so the bug
//! manifests as silent acceptance: `SECRET//NF`, `(S//NOFORN)`,
//! `SECRET//OC`, etc. all produce zero diagnostics.
//!
//! These tests were authored on the pre-fix tree (commit `ed9c3fe1` of
//! `staging`) and confirmed RED — every assertion below FAILS on
//! pre-fix. After Commit 3 of the fix lands, every assertion MUST pass.
//!
//! Authority: CAPCO-2016 §D.1 p27 line 560 (banner-line syntax —
//! controls in Marking Title or Authorized Abbreviation form only),
//! §C.1 p25 line 503 (portion mark — "An authorized portion mark is
//! listed for each classification and control marking entry in the
//! Register"), §G.1 Table 4 p38 (Register-closed-set authority).
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

/// Filter diagnostics down to the new form-mismatch rules.
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

// ---------------------------------------------------------------------------
// Five abbreviated dissem pairs (NF/NOFORN, OC/ORCON, IMC/IMCON, DSEN/DEA
// SENSITIVE, PR/PROPIN) — each tested in both directions: portion form in
// banner, banner form in portion.
// ---------------------------------------------------------------------------

#[test]
fn banner_with_nf_fires_form_mismatch() {
    let diags = lint("SECRET//NF");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "SECRET//NF must produce at least one form-mismatch diagnostic; got {diags:?}",
    );
}

#[test]
fn portion_with_noforn_fires_form_mismatch() {
    let diags = lint("(S//NOFORN)");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "(S//NOFORN) must produce at least one form-mismatch diagnostic; got {diags:?}",
    );
}

#[test]
fn banner_with_oc_fires_form_mismatch() {
    let diags = lint("SECRET//OC");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "SECRET//OC must produce at least one form-mismatch diagnostic; got {diags:?}",
    );
}

#[test]
fn portion_with_orcon_fires_form_mismatch() {
    let diags = lint("(S//ORCON)");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "(S//ORCON) must produce at least one form-mismatch diagnostic; got {diags:?}",
    );
}

#[test]
fn banner_with_imc_fires_form_mismatch() {
    let diags = lint("SECRET//IMC");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "SECRET//IMC must produce at least one form-mismatch diagnostic; got {diags:?}",
    );
}

#[test]
fn portion_with_imcon_fires_form_mismatch() {
    let diags = lint("(S//IMCON)");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "(S//IMCON) must produce at least one form-mismatch diagnostic; got {diags:?}",
    );
}

#[test]
fn banner_with_dsen_fires_form_mismatch() {
    let diags = lint("SECRET//DSEN");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "SECRET//DSEN must produce at least one form-mismatch diagnostic; got {diags:?}",
    );
}

#[test]
fn portion_with_dea_sensitive_fires_form_mismatch() {
    let diags = lint("(S//DEA SENSITIVE)");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "(S//DEA SENSITIVE) must produce at least one form-mismatch diagnostic; got {diags:?}",
    );
}

#[test]
fn banner_with_pr_fires_form_mismatch() {
    let diags = lint("SECRET//PR");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "SECRET//PR must produce at least one form-mismatch diagnostic; got {diags:?}",
    );
}

#[test]
fn portion_with_propin_fires_form_mismatch() {
    let diags = lint("(S//PROPIN)");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "(S//PROPIN) must produce at least one form-mismatch diagnostic; got {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// Broad-scope verification — confirms the rule reaches beyond dissem.
// ---------------------------------------------------------------------------
//
// Note: §D.1 p27 line 560 explicitly permits BOTH the Marking Title
// (e.g., TALENT KEYHOLE) AND the Authorized Abbreviation (e.g., TK) in
// the banner line. So `SECRET//TALENT KEYHOLE` is VALID, not a defect —
// the synthesis-brief's draft test for that case was incorrect under
// re-reading of §D.1 p27. The broad-scope SCI test is the portion-
// direction case: `(TS//TALENT KEYHOLE)`, where the long Marking Title
// appears in a portion mark instead of the short portion form `TK`.
// `banner_to_portion("TALENT KEYHOLE")` returns `None` because the
// title-form lookup is owned by `title_to_portion`; we instead reach
// SI-ECRU / SI-NONBOOK for broad-scope SCI coverage where the helpers
// directly map. For TALENT KEYHOLE specifically, the portion-form
// "TK" already equals its banner form (same-form row in
// `MARKING_FORMS`), so it does not fire. We swap in SI-ECRU/SI-EU as
// the SCI broad-scope case.

#[test]
fn banner_with_si_eu_fires_form_mismatch() {
    // SI-EU is the portion form; SI-ECRU is the banner Marking Title.
    // CAPCO-2016 §H.4 p78. Banner appearing as `SI-EU` should flag.
    let diags = lint("TOP SECRET//SI-EU");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "TOP SECRET//SI-EU must produce at least one form-mismatch diagnostic; got {diags:?}",
    );
}

#[test]
fn portion_with_si_ecru_fires_form_mismatch() {
    // SI-ECRU is the banner Marking Title; SI-EU is the portion form.
    // CAPCO-2016 §H.4 p78. Portion appearing as `SI-ECRU` should flag.
    let diags = lint("(TS//SI-ECRU)");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "(TS//SI-ECRU) must produce at least one form-mismatch diagnostic; got {diags:?}",
    );
}

#[test]
fn banner_with_classification_abbrev_fires_form_mismatch() {
    // Classifier abbreviation `S` in a banner — `portion_to_banner("S")`
    // returns `Some("SECRET")` so the broad-scope walker catches it.
    // This is the PM-4 sister bug subsumed by broad scope (no separate
    // issue needed). Banner line requires the spelled-out classification
    // per §D.1 p27 line 555 ("The classification level must be in
    // English without abbreviation").
    let diags = lint("S//NOFORN");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "S//NOFORN must produce at least one form-mismatch diagnostic for the \
         classification abbreviation in banner position; got {diags:?}",
    );
}
