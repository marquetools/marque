// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! E073 — FGI invalid ownership token (category-specific diagnostic).
//!
//! Issue #501: replace the generic E008 ("unrecognized token") Error
//! on FGI-marker spans whose ownership-list tail contains a token
//! that fails the strict-parser shape gate
//! [`marque_ism::CountryCode::admits_fgi_ownership_token`]. The FGI
//! ownership slot admits sovereign trigraphs, the 2-byte `EU`
//! exception, and the literal `NATO` tetragraph; distribution-list
//! tetragraphs (`FVEY`, `ACGU`, `ISAF`, `CFIUS`) describe who may
//! receive a marking, not who owns it.
//!
//! E073 emits at `Severity::Error` with no fix (no single right
//! replacement: `FVEY` is a 5-country coalition tetragraph, `DEUX` is
//! shape-wrong rather than a typo for `DEU`). Diagnostics carry the
//! `CAT_FGI_MARKER` category arg so audit consumers can distinguish
//! E073 from generic E008.
//!
//! The E008 emission path suppresses co-firing on the same FGI-marker
//! span via `is_fgi_invalid_ownership_token` so users see only the
//! actionable category-specific diagnostic.
//!
//! Authority: CAPCO-2016 §H.7 p123. Verified against
//! `crates/capco/docs/CAPCO-2016.md` (lines 3043-3053) at authorship
//! per Constitution VIII.

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

fn diagnostics_for(source: &[u8]) -> Vec<marque_rules::Diagnostic<marque_capco::CapcoScheme>> {
    engine().lint(source).diagnostics.into_iter().collect()
}

fn e073_diags(
    diags: &[marque_rules::Diagnostic<marque_capco::CapcoScheme>],
) -> Vec<&marque_rules::Diagnostic<marque_capco::CapcoScheme>> {
    diags.iter().filter(|d| d.rule.as_str() == "E073").collect()
}

fn e008_diags(
    diags: &[marque_rules::Diagnostic<marque_capco::CapcoScheme>],
) -> Vec<&marque_rules::Diagnostic<marque_capco::CapcoScheme>> {
    diags.iter().filter(|d| d.rule.as_str() == "E008").collect()
}

fn citation_contains(
    d: &marque_rules::Diagnostic<marque_capco::CapcoScheme>,
    needle: &str,
) -> bool {
    format!("{}", d.citation).contains(needle)
}

// ---------------------------------------------------------------------------
// Trigger cases — single invalid token → exactly one E073 Error
// ---------------------------------------------------------------------------

#[test]
fn e073_fires_on_fvey_ownership_token() {
    let diags = diagnostics_for(b"(S//FGI FVEY)");
    let e073 = e073_diags(&diags);
    assert_eq!(
        e073.len(),
        1,
        "(S//FGI FVEY) must emit exactly one E073 diagnostic; got diagnostics={:?}",
        diags
            .iter()
            .map(|d| (d.rule.as_str(), d.severity))
            .collect::<Vec<_>>(),
    );
    let d = e073[0];
    assert_eq!(
        d.severity,
        Severity::Error,
        "E073 must default to Severity::Error"
    );
    assert!(
        citation_contains(d, "§H.7 p123"),
        "E073 must cite §H.7 p123; got {:?}",
        d.citation,
    );
    assert!(
        d.text_correction.is_none() && d.fix.is_none(),
        "E073 must not offer a fix (no single right replacement)"
    );
    assert!(
        e008_diags(&diags).is_empty(),
        "E008 must not co-fire on the same FGI-marker span; got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.as_str(), d.severity))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn e073_fires_on_deux_ownership_token() {
    let diags = diagnostics_for(b"(S//FGI DEUX)");
    let e073 = e073_diags(&diags);
    assert_eq!(e073.len(), 1, "DEUX must emit one E073");
    assert_eq!(e073[0].severity, Severity::Error);
    assert!(citation_contains(e073[0], "§H.7 p123"));
    assert!(
        e008_diags(&diags).is_empty(),
        "E008 must not co-fire on DEUX FGI marker"
    );
}

#[test]
fn e073_fires_on_acgu_ownership_token() {
    // ACGU is a registered REL TO tetragraph (USA/CAN/GBR/AUS) — lawful in
    // REL TO list slots, not in FGI ownership slots per §H.7.
    let diags = diagnostics_for(b"(S//FGI ACGU)");
    let e073 = e073_diags(&diags);
    assert_eq!(
        e073.len(),
        1,
        "ACGU in FGI ownership slot must emit one E073"
    );
    assert_eq!(e073[0].severity, Severity::Error);
    assert!(citation_contains(e073[0], "§H.7 p123"));
}

#[test]
fn e073_fires_on_isaf_ownership_token() {
    let diags = diagnostics_for(b"(S//FGI ISAF)");
    let e073 = e073_diags(&diags);
    assert_eq!(
        e073.len(),
        1,
        "ISAF in FGI ownership slot must emit one E073"
    );
    assert_eq!(e073[0].severity, Severity::Error);
}

// ---------------------------------------------------------------------------
// Source-segregated portion form — `(//FGI X)` instead of `(S//FGI X)`
// ---------------------------------------------------------------------------

#[test]
fn e073_fires_on_source_segregated_portion_form() {
    // `(//FGI FVEY)` — segregated portion form per §H.7 p123 Example
    // Portion Mark "when source must be concealed and segregated from
    // US". E073 still fires because FVEY fails the ownership shape
    // gate regardless of segregation status.
    let diags = diagnostics_for(b"(//FGI FVEY)");
    let e073 = e073_diags(&diags);
    assert_eq!(
        e073.len(),
        1,
        "source-segregated form must still emit E073 on invalid token; got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.as_str(), d.severity))
            .collect::<Vec<_>>(),
    );
    assert_eq!(e073[0].severity, Severity::Error);
}

// ---------------------------------------------------------------------------
// Mixed-valid-and-invalid tokens — one E073 per invalid token
// ---------------------------------------------------------------------------

#[test]
fn e073_fires_only_on_invalid_token_in_mixed_list() {
    // `(S//FGI DEU FVEY)` — DEU is a valid sovereign trigraph; FVEY is
    // the distribution-list tetragraph. E073 must fire ONCE on FVEY's
    // span, not on DEU's. Multi-source FGI lists are §A.6 p16-authorized
    // ("Multiple FGI trigraph country codes or tetragraph codes must be
    // separated by a single space"); the parser rejects the whole marker
    // because one token fails the shape gate, so the rule layer takes
    // over and emits a per-token diagnostic.
    let diags = diagnostics_for(b"(S//FGI DEU FVEY)");
    let e073 = e073_diags(&diags);
    assert_eq!(
        e073.len(),
        1,
        "mixed valid+invalid list must emit exactly one E073 (on FVEY); got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.as_str(), d.severity))
            .collect::<Vec<_>>(),
    );
    assert_eq!(e073[0].severity, Severity::Error);
}

// ---------------------------------------------------------------------------
// Valid cases — no E073 fires
// ---------------------------------------------------------------------------

#[test]
fn e073_does_not_fire_on_nato_ownership_token() {
    // NATO is the one valid tetragraph in the FGI ownership slot per
    // §H.7 (canonical alliance ownership identifier).
    let diags = diagnostics_for(b"(S//FGI NATO)");
    assert!(
        e073_diags(&diags).is_empty(),
        "valid NATO ownership token must not emit E073; got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.as_str(), d.severity))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn e073_does_not_fire_on_eu_ownership_token() {
    // EU is the 2-byte exception per Council Decision 2013/488/EU,
    // registered in ISMCAT `CVEnumISMCATRelTo`.
    let diags = diagnostics_for(b"(S//FGI EU)");
    assert!(
        e073_diags(&diags).is_empty(),
        "valid EU ownership token must not emit E073; got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.as_str(), d.severity))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn e073_does_not_fire_on_canonical_sovereign_trigraph() {
    // DEU is the canonical sovereign trigraph for Germany; §H.7 p123
    // canonical FGI ownership form.
    let diags = diagnostics_for(b"(S//FGI DEU)");
    assert!(
        e073_diags(&diags).is_empty(),
        "valid sovereign trigraph DEU must not emit E073; got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.as_str(), d.severity))
            .collect::<Vec<_>>(),
    );
}

// ---------------------------------------------------------------------------
// Decoder coordination — lowercase routes through R001 decoder (existing
// behavior pinned in decoder_dispatch_post_280.rs); E073 must NOT co-fire so
// the user sees only the actionable R001 case-fold fix.
// ---------------------------------------------------------------------------

#[test]
fn e073_does_not_co_fire_on_lowercase_trigraph_decoder_route() {
    // `(S//FGI deu)` — strict parser rejects, dispatcher falls through
    // to the R001 decoder which emits a `Severity::Fix` case-fold
    // canonicalization. E073 must NOT co-fire — the lowercase token is
    // shape-rejected (uppercase-only admission) but the user-actionable
    // signal is the R001 canonicalization, not a category-specific
    // "wrong ownership token" diagnostic.
    let diags = diagnostics_for(b"(S//FGI deu)");
    assert!(
        e073_diags(&diags).is_empty(),
        "E073 must not co-fire on the lowercase-trigraph decoder route; \
         R001 owns this surface. got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.as_str(), d.severity))
            .collect::<Vec<_>>(),
    );
}

// ---------------------------------------------------------------------------
// Banner form — `SECRET//FGI FVEY//NOFORN`
// ---------------------------------------------------------------------------

#[test]
fn e073_fires_on_banner_form_with_invalid_ownership_token() {
    // Banner form ("SECRET//FGI FVEY//NOFORN") — the parser dispatches
    // `parse_fgi_marker` via `starts_with_fgi_prefix` for banners too.
    // E073 must fire on the FVEY token regardless of marking type.
    let diags = diagnostics_for(b"SECRET//FGI FVEY//NOFORN");
    let e073 = e073_diags(&diags);
    assert!(
        e073.iter().any(|d| d.severity == Severity::Error),
        "banner form with invalid FGI ownership token must emit E073; got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.as_str(), d.severity))
            .collect::<Vec<_>>(),
    );
}

// ---------------------------------------------------------------------------
// Long-form marker — "FOREIGN GOVERNMENT INFORMATION FVEY"
// ---------------------------------------------------------------------------

#[test]
fn e073_fires_on_long_form_marker_with_invalid_ownership_token() {
    // The long-form `FOREIGN GOVERNMENT INFORMATION` banner-title
    // prefix is the §H.7 p123 Authorized Banner Line Marking Title.
    // `parse_fgi_marker` strips both prefixes uniformly; E073 must
    // recognize the same shape.
    let diags = diagnostics_for(b"SECRET//FOREIGN GOVERNMENT INFORMATION FVEY//NOFORN");
    let e073 = e073_diags(&diags);
    assert!(
        e073.iter().any(|d| d.severity == Severity::Error),
        "long-form FGI marker with invalid token must emit E073; got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.as_str(), d.severity))
            .collect::<Vec<_>>(),
    );
}

// ---------------------------------------------------------------------------
// Idempotence — engine.lint twice produces byte-identical diagnostics
// ---------------------------------------------------------------------------

#[test]
fn e073_lint_is_idempotent() {
    let input = b"(S//FGI FVEY)";
    let eng = engine();
    let first = eng.lint(input);
    let second = eng.lint(input);
    let first_ids: Vec<(String, Severity)> = first
        .diagnostics
        .iter()
        .map(|d| (d.rule.as_str().to_owned(), d.severity))
        .collect();
    let second_ids: Vec<(String, Severity)> = second
        .diagnostics
        .iter()
        .map(|d| (d.rule.as_str().to_owned(), d.severity))
        .collect();
    assert_eq!(
        first_ids, second_ids,
        "E073 lint must be idempotent — two passes on the same input \
         must produce identical diagnostic streams. Marque invariant."
    );
}
