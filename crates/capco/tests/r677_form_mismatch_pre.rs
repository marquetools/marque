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
            pid == "banner.metadata.uses-portion-form" || pid == "portion.metadata.uses-banner-form"
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
// re-reading of §D.1 p27.
//
// SCI compartment broad-scope coverage (SI-EU / SI-NK) is deferred to
// the follow-up that closes issue #701 (MARKING_FORMS SI-EU/SI-NK data
// bug — `marque-ism` records the Marking Title in the `banner` field
// instead of the Authorized Banner Abbreviation `SI-EU` / `SI-NK` per
// §H.4 p78 / p83). Until #701 lands, SI-EU broad-scope canonical-no-fire
// fixtures would assert behavior the data layer does not yet support.
// Non-IC dissem and NATO classification form-pair tests below provide
// the broad-scope authority verification in the meantime — both
// categories have legitimate `banner != portion` rows in MARKING_FORMS
// today.

// ---------------------------------------------------------------------------
// Non-IC dissem form pairs (LIMDIS↔DS, EXDIS↔XD, NODIS↔ND) — broad-scope
// coverage that the rule reaches beyond IC dissem into §H.9. Verified
// against `crates/ism/src/marking_forms.rs` rows at L488-503 (each row
// has `banner != portion` so `portion_to_banner` / `banner_to_portion`
// returns `Some`). Authority: CAPCO-2016 §H.9 pp170-174 + §G.1 Table 4
// p38 (Register closed-set).
// ---------------------------------------------------------------------------

#[test]
fn banner_with_ds_fires_form_mismatch() {
    // DS is the §H.9 p170 portion form for LIMDIS; banner form is
    // `LIMDIS`. A bare `DS` in a banner line is a form-mismatch.
    let diags = lint("UNCLASSIFIED//DS");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "UNCLASSIFIED//DS must produce at least one form-mismatch diagnostic; got {diags:?}",
    );
}

#[test]
fn portion_with_limdis_fires_form_mismatch() {
    // LIMDIS is the §H.9 p170 banner form; portion form is `DS`. A
    // `LIMDIS` in a portion mark is a form-mismatch.
    let diags = lint("(U//LIMDIS)");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "(U//LIMDIS) must produce at least one form-mismatch diagnostic; got {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// NATO classification form pairs — broad-scope coverage that the rule
// reaches into §H.7. Verified against `crates/ism/src/marking_forms.rs`
// L244-277 (each NATO classification row has `banner != portion`).
// Authority: CAPCO-2016 §G.1 Table 4 p36 + §H.7 p123. Note: a NATO
// banner line is `//<NATO class>` per §D.1 p27 line 552-554, so the
// banner test uses a bare `//NS` shape rather than the US-class-prefixed
// `SECRET//...` form.
// ---------------------------------------------------------------------------

#[test]
fn banner_with_nato_portion_form_fires_form_mismatch() {
    // `NS` is the §G.1 Table 4 p36 NATO SECRET portion form; banner
    // form is `NATO SECRET`. A bare `NS` in NATO banner position is
    // a form-mismatch.
    let diags = lint("//NS");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "//NS must produce at least one form-mismatch diagnostic; got {diags:?}",
    );
}

#[test]
fn portion_with_nato_secret_banner_form_fires_form_mismatch() {
    // `NATO SECRET` is the §G.1 Table 4 p36 banner form; portion form
    // is `NS`. A `NATO SECRET` in NATO portion position is a
    // form-mismatch.
    let diags = lint("(//NATO SECRET)");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "(//NATO SECRET) must produce at least one form-mismatch diagnostic; got {diags:?}",
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

// ---------------------------------------------------------------------------
// Marking Title in portion position — Copilot R2 Finding B coverage. The
// `find_banner_form_in_portion` walker checks both the Authorized
// Banner Abbreviation column (`MARKING_FORMS.banner`) via
// `banner_to_portion` AND the Marking Title column (`MARKING_FORMS.title`)
// via `title_to_portion`. SCI rows like TALENT KEYHOLE have
// `title="TALENT KEYHOLE"` ≠ `banner="TK"` == `portion="TK"` — the
// `banner_to_portion("TALENT KEYHOLE")` lookup misses (gated on
// `banner != portion`, which fails here), but `title_to_portion`
// catches it. Authority: CAPCO-2016 §C.1 p25 (portion mark is the
// Register Portion Mark column) + §G.1 Table 4 p38 (Register
// closed-set governs all three columns). Re-verified at authorship
// per Constitution VIII.
// ---------------------------------------------------------------------------

#[test]
fn portion_with_sci_title_fires_form_mismatch() {
    // TALENT KEYHOLE is the Marking Title (§H.4 p85) for TK; the
    // Register Portion Mark is `TK`. Pre-fix this case silently
    // passed because `banner_to_portion("TALENT KEYHOLE")` returns
    // `None` (the row has `banner == portion == "TK"`); the
    // `title_to_portion` fallback catches it post-fix.
    let diags = lint("(TS//TALENT KEYHOLE)");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "(TS//TALENT KEYHOLE) must produce at least one form-mismatch diagnostic; got {diags:?}",
    );
}

#[test]
fn portion_with_orcon_title_fires_form_mismatch() {
    // ORIGINATOR CONTROLLED is the Marking Title (§H.8 p136) for
    // ORCON; the Register Portion Mark is `OC`. ORCON has
    // `title != banner` AND `banner != portion`, so the original
    // `banner_to_portion` lookup ALSO catches `(S//ORCON)`. This
    // test asserts the title-form fallback catches the long-title
    // variant cleanly. Authority: §H.8 p136 + §G.1 Table 4 p38.
    let diags = lint("(S//ORIGINATOR CONTROLLED)");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "(S//ORIGINATOR CONTROLLED) must produce at least one form-mismatch \
         diagnostic; got {diags:?}",
    );
}

#[test]
fn portion_with_noforn_title_fires_form_mismatch() {
    // NOT RELEASABLE TO FOREIGN NATIONALS is the Marking Title
    // (§H.8 p145) for NOFORN; the Register Portion Mark is `NF`.
    // Authority: §H.8 p145 + §G.1 Table 4 p38.
    let diags = lint("(S//NOT RELEASABLE TO FOREIGN NATIONALS)");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "(S//NOT RELEASABLE TO FOREIGN NATIONALS) must produce at least one \
         form-mismatch diagnostic; got {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// Abbreviated US classification in a Conflict banner — Copilot R2
// Finding D coverage. `MarkingClassification::Conflict { us, foreign }`
// is what the parser emits for compound banners that carry both a US
// classification and a NATO classification (e.g.,
// `SECRET//NATO SECRET//NOFORN`). §D.1 p27 line 555 ("The
// classification level must be in English without abbreviation")
// applies to the US side regardless of the foreign companion — so an
// abbreviated US class token (`S` rather than `SECRET`) in a
// Conflict banner is still a form mismatch. The pre-fix branch read
// `MarkingClassification::Us(_)` only and silently passed Conflict;
// post-fix the branch reads via `CanonicalAttrs::us_classification()`
// which covers both variants per its accessor doc.
// ---------------------------------------------------------------------------

#[test]
fn banner_with_us_abbrev_in_conflict_fires_form_mismatch() {
    // `S//NATO SECRET//NOFORN` parses as `Conflict { us: Secret,
    // foreign: NatoSecret }` (verified by
    // `crates/core/src/parser.rs::conflict_us_and_nato`). The US
    // token `S` is a portion-form classification in banner position
    // — must fire form-mismatch even though the classification
    // variant is `Conflict`, not `Us`. Authority: §D.1 p27 line 555.
    let diags = lint("S//NATO SECRET//NOFORN");
    let hits = form_mismatch_diags(&diags);
    assert!(
        !hits.is_empty(),
        "S//NATO SECRET//NOFORN must produce at least one form-mismatch \
         diagnostic (Conflict variant); got {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// One-diagnostic-per-marking regression guard — dual-defective banner.
// ---------------------------------------------------------------------------

#[test]
fn dual_defective_banner_emits_exactly_one_diagnostic() {
    // `S//NF` has BOTH a classification abbreviation (`S`) AND a
    // portion-form dissem (`NF`) in banner position — two distinct
    // defective tokens within one marking. The rule's classification
    // branch short-circuits via `return Some(token.span)` after
    // detecting `S` (`crates/capco/src/rules.rs`), so exactly ONE
    // `Recanonicalize { Page }` diagnostic is emitted per marking —
    // not one per defective token. Regression-protects the
    // one-diagnostic-per-marking guarantee documented at the
    // `PortionFormInBannerRule` module-header doc comment.
    let diags = lint("S//NF");
    let hits = form_mismatch_diags(&diags);
    assert_eq!(
        hits.len(),
        1,
        "S//NF has two defective tokens (S=classification-abbrev, NF=portion-form) \
         but must emit exactly one form-mismatch diagnostic per marking; got {hits:?}",
    );
}
