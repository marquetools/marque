// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! E071 — FGI with explicit trigraph when concealment intended or
//! acknowledgment contradicted per CAPCO-2016 §H.7 p124.
//!
//! "Do not include country codes within the portion marks where the
//! specific government(s) must be concealed."
//!
//! Four-case behavioral spec:
//!
//! - Case A (countries ⊆ REL TO): Error + fix (drop "FGI " prefix).
//!   `(//FGI DEU R//REL TO USA, DEU)` → `(//DEU R//REL TO USA, DEU)`
//!
//! - Case B (bare FGI, no trigraph): Valid — no diagnostic.
//!   `(//FGI S)` is canonical unacknowledged-source form.
//!
//! - Case C (countries ∩ REL TO = ∅): Warn (drop trigraphs)
//!   + Suggest (drop FGI prefix) + optional NF Suggest.
//!   `(//FGI DEU R)` → primary fix `(//FGI R)`, alt Suggest `(//DEU R)`.
//!
//! - Case D (partial REL TO overlap): Error (no fix) + Suggest ack-all
//!   + Suggest conceal-all + optional NF Suggest.
//!   `(//FGI DEU GBR R//REL TO USA, DEU)` — DEU acknowledged, GBR not.
//!
//! Authority: CAPCO-2016 §H.7 p124. Verified against
//! `crates/capco/docs/CAPCO-2016.md` at the time of authorship.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::Severity;

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

fn lint_e071(source: &[u8]) -> Vec<marque_rules::Diagnostic<marque_capco::CapcoScheme>> {
    engine()
        .lint(source)
        .diagnostics
        .into_iter()
        .filter(|d| d.rule.as_str() == "E071")
        .collect()
}

// ---------------------------------------------------------------------------
// Case A — countries fully ⊆ REL TO: acknowledged source; FGI prefix wrong
// ---------------------------------------------------------------------------

#[test]
fn case_a_fires_error_with_drop_prefix_fix() {
    // `(//FGI DEU R//REL TO USA, DEU)` — DEU is in REL TO → Full
    // containment → acknowledged source → FGI prefix is wrong.
    // Expected: exactly 1 E071 Error with text_correction "DEU R".
    let diags = lint_e071(b"(//FGI DEU R//REL TO USA, DEU)\n");
    assert_eq!(
        diags.len(),
        1,
        "Case A must emit exactly one E071 diagnostic; got {diags:?}",
    );
    let d = &diags[0];
    assert_eq!(
        d.severity,
        Severity::Error,
        "Case A E071 must be Error severity; got {:?}",
        d.severity,
    );
    let tc = d
        .text_correction
        .as_ref()
        .expect("Case A E071 must carry a text_correction");
    assert_eq!(
        tc.replacement.as_str(),
        "DEU R",
        "Case A fix must drop 'FGI ' prefix: 'FGI DEU R' → 'DEU R'; \
         got {:?}",
        tc.replacement,
    );
    assert!(
        format!("{}", d.citation).contains("§H.7 p124"),
        "Case A citation must cite §H.7 p124; got {:?}",
        d.citation,
    );
}

#[test]
fn case_a_multi_country_fix_drops_prefix() {
    // `(//FGI DEU GBR R//REL TO USA, DEU, GBR)` — all FGI countries
    // are in REL TO → Full containment. Fix drops "FGI " prefix only.
    let diags = lint_e071(b"(//FGI DEU GBR R//REL TO USA, DEU, GBR)\n");
    assert_eq!(
        diags.len(),
        1,
        "Case A multi-country must emit one E071 diagnostic; got {diags:?}",
    );
    let tc = diags[0]
        .text_correction
        .as_ref()
        .expect("Case A multi-country must carry text_correction");
    assert_eq!(
        tc.replacement.as_str(),
        "DEU GBR R",
        "Case A multi-country fix drops 'FGI ' prefix; got {:?}",
        tc.replacement,
    );
}

// ---------------------------------------------------------------------------
// Case B — bare FGI (no trigraph): canonical unacknowledged form, valid
// ---------------------------------------------------------------------------

#[test]
fn case_b_silent_on_bare_fgi() {
    // `(//FGI S)` — canonical unacknowledged FGI, no trigraph.
    // Gate 5 (`fgi.countries.is_empty()`) returns early; no E071.
    let diags = lint_e071(b"(//FGI S)\n");
    assert!(
        diags.is_empty(),
        "Case B: bare `(//FGI S)` must not fire E071; got {diags:?}",
    );
}

#[test]
fn case_b_silent_on_bare_fgi_with_noforn() {
    // `(//FGI R//NF)` — unacknowledged FGI with NOFORN, no trigraph.
    // Valid canonical form; no E071.
    let diags = lint_e071(b"(//FGI R//NF)\n");
    assert!(
        diags.is_empty(),
        "Case B: `(//FGI R//NF)` must not fire E071; got {diags:?}",
    );
}

#[test]
fn case_b_silent_on_acknowledged_fgi_without_fgi_prefix() {
    // `(//DEU R//REL TO USA, DEU)` — acknowledged foreign without "FGI "
    // prefix. Gate 4 (`starts_with("FGI ")`) returns early; no E071.
    let diags = lint_e071(b"(//DEU R//REL TO USA, DEU)\n");
    assert!(
        diags.is_empty(),
        "Case B: acknowledged FGI without 'FGI ' prefix must not fire E071; \
         got {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// Case C — countries ∩ REL TO = ∅: concealment required; trigraph wrong
// ---------------------------------------------------------------------------

#[test]
fn case_c_fires_warn_plus_suggest_without_rel_to() {
    // `(//FGI DEU R)` — FGI + DEU trigraph, no REL TO at all.
    // Empty containment → Warn (conceal form "FGI R") + Suggest (ack
    // form "DEU R") + NF Suggest (no NOFORN present). 3 diagnostics.
    let diags = lint_e071(b"(//FGI DEU R)\n");
    assert_eq!(
        diags.len(),
        3,
        "Case C without NOFORN must emit 3 E071 diagnostics \
         (Warn + Suggest alt + Suggest NF); got {diags:?}",
    );

    // Primary: Warn with conceal-form replacement "FGI R".
    let warn = diags
        .iter()
        .find(|d| d.severity == Severity::Warn)
        .expect("Case C must have a Warn diagnostic");
    let tc = warn
        .text_correction
        .as_ref()
        .expect("Case C Warn must carry text_correction");
    assert_eq!(
        tc.replacement.as_str(),
        "FGI R",
        "Case C primary fix must be the concealed form 'FGI R'; \
         got {:?}",
        tc.replacement,
    );

    // Alternate Suggest: text_correction replacement "DEU R".
    let has_ack_suggest = diags.iter().any(|d| {
        d.severity == Severity::Suggest
            && d.text_correction
                .as_ref()
                .map(|tc| tc.replacement.as_str() == "DEU R")
                .unwrap_or(false)
    });
    assert!(
        has_ack_suggest,
        "Case C must include a Suggest with acknowledged form 'DEU R'; \
         got {diags:?}",
    );

    // NF Suggest: has fix (FactAdd intent), no text_correction.
    let nf_suggest = diags
        .iter()
        .find(|d| d.severity == Severity::Suggest && d.fix.is_some())
        .expect(
            "Case C must include a Suggest with a FixIntent (NF companion); \
             got {diags:?}",
        );
    assert!(
        nf_suggest.text_correction.is_none(),
        "NF companion Suggest must not carry text_correction; \
         got {:?}",
        nf_suggest.text_correction,
    );
}

#[test]
fn case_c_nf_companion_suppressed_when_noforn_present() {
    // `(//FGI DEU R//NF)` — NOFORN already present.
    // `if !noforn_present` gate suppresses the NF companion.
    // Expected: 2 diagnostics (Warn + Suggest alt).
    let diags = lint_e071(b"(//FGI DEU R//NF)\n");
    assert_eq!(
        diags.len(),
        2,
        "Case C with NOFORN must emit 2 E071 diagnostics \
         (Warn + Suggest alt, no NF Suggest); got {diags:?}",
    );
    assert!(
        diags.iter().any(|d| d.severity == Severity::Warn),
        "Case C with NOFORN must still emit the Warn diagnostic",
    );
    assert!(
        !diags.iter().any(|d| d.fix.is_some()),
        "Case C with NOFORN must not emit the NF FixIntent Suggest; \
         got {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// Case D — partial REL TO overlap: ambiguous intent; no auto-fix
// ---------------------------------------------------------------------------

#[test]
fn case_d_fires_error_no_fix_on_partial_overlap() {
    // `(//FGI DEU GBR R//REL TO USA, DEU)` — DEU in REL TO, GBR not.
    // Partial containment → Error (no fix) + Suggest ack-all +
    // Suggest conceal-all + NF Suggest. 4 diagnostics.
    let diags = lint_e071(b"(//FGI DEU GBR R//REL TO USA, DEU)\n");
    assert_eq!(
        diags.len(),
        4,
        "Case D without NOFORN must emit 4 E071 diagnostics \
         (Error + Suggest ack-all + Suggest conceal + Suggest NF); \
         got {diags:?}",
    );

    // Error diagnostic: no text_correction, no fix.
    let err = diags
        .iter()
        .find(|d| d.severity == Severity::Error)
        .expect("Case D must have an Error diagnostic");
    assert!(
        err.text_correction.is_none(),
        "Case D Error must carry no text_correction (no auto-fix); \
         got {:?}",
        err.text_correction,
    );
    assert!(
        err.fix.is_none(),
        "Case D Error must carry no FixIntent (no auto-fix); \
         got {:?}",
        err.fix,
    );

    // Suggest: acknowledge-all form "DEU GBR R" (sorted alphabetically).
    let has_ack_all = diags.iter().any(|d| {
        d.severity == Severity::Suggest
            && d.text_correction
                .as_ref()
                .map(|tc| tc.replacement.as_str() == "DEU GBR R")
                .unwrap_or(false)
    });
    assert!(
        has_ack_all,
        "Case D must include a Suggest with ack-all form 'DEU GBR R'; \
         got {diags:?}",
    );

    // Suggest: conceal-all form "FGI R".
    let has_conceal_all = diags.iter().any(|d| {
        d.severity == Severity::Suggest
            && d.text_correction
                .as_ref()
                .map(|tc| tc.replacement.as_str() == "FGI R")
                .unwrap_or(false)
    });
    assert!(
        has_conceal_all,
        "Case D must include a Suggest with conceal-all form 'FGI R'; \
         got {diags:?}",
    );
}

#[test]
fn case_d_nf_companion_suppressed_when_noforn_present() {
    // `(//FGI DEU GBR R//REL TO USA, DEU//NF)` — NOFORN present;
    // NF companion gate fires. Expected: 3 diagnostics (Error + 2 Suggests).
    let diags = lint_e071(b"(//FGI DEU GBR R//REL TO USA, DEU//NF)\n");
    assert_eq!(
        diags.len(),
        3,
        "Case D with NOFORN must emit 3 E071 diagnostics \
         (Error + Suggest ack-all + Suggest conceal-all); got {diags:?}",
    );
    assert!(
        !diags.iter().any(|d| d.fix.is_some()),
        "Case D with NOFORN must not emit the NF FixIntent Suggest; \
         got {diags:?}",
    );
}

// ---------------------------------------------------------------------------
// Gate tests — E071 must NOT fire on non-FGI inputs
// ---------------------------------------------------------------------------

#[test]
fn gate2_silent_on_us_classified_portion() {
    // `(S//NF)` — US classified. Gate 2 (`MarkingClassification::Fgi`)
    // returns early; no E071.
    let diags = lint_e071(b"(S//NF)\n");
    assert!(
        diags.is_empty(),
        "Gate 2: US classified portion must not fire E071; got {diags:?}",
    );
}

#[test]
fn gate2_silent_on_unclassified_portion() {
    // `(U)` — Unclassified. Gate 2 returns early; no E071.
    let diags = lint_e071(b"(U)\n");
    assert!(
        diags.is_empty(),
        "Gate 2: Unclassified portion must not fire E071; got {diags:?}",
    );
}

#[test]
fn gate1_silent_on_banner_marking() {
    // E071 fires only on portions (Gate 1: `MarkingType::Portion`).
    // A US classified banner line does not trigger E071 regardless of
    // what FGI portions appear on the same page.
    //
    // Verify: the `SECRET` banner does not contribute to E071 count.
    // The portion `(//FGI DEU R)` fires 3 E071 diagnostics (Case C);
    // the banner fires zero.
    let result = engine().lint(b"SECRET\n(//FGI DEU R)\n");
    let e071_on_banner = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "E071")
        .filter(|d| {
            // Banner span starts at byte 0 ("SECRET" = bytes 0-5).
            // Any E071 at span.start < 7 is on the banner, not the
            // portion.
            d.span.start < 7
        })
        .count();
    assert_eq!(
        e071_on_banner, 0,
        "Gate 1: E071 must not fire on the banner line; \
         got {e071_on_banner} E071 diagnostics with span in banner range",
    );
}
