// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Issue #407 — `BareCanonicalCompoundRule` (E067) integration tests.
//!
//! Authority:
//! - CNWDI bare form → `RD-CNWDI`: CAPCO-2016 §H.6 p106
//!   (`(U) Example Portion Mark: (S//RD-CNWDI)`).
//! - NK bare form → `SI-NK`: CAPCO-2016 §H.4 p83
//!   (`(U) Authorized Portion Mark: SI-NK`,
//!    `(U) Example Portion Mark: (TS//SI-NK)`).
//! - EU bare form (in SCI position) → `SI-EU`: CAPCO-2016 §H.4 p78
//!   (`(U) Authorized Portion Mark: SI-EU`,
//!    `(U) Example Portion Mark: (TS//SI-EU)`).
//!
//! The walker filters `TokenKind::Unknown` spans and emits one
//! `Diagnostic::text_correction` per bare-form match. Hardcoded
//! static-literal replacements satisfy Constitution V (audit
//! content-ignorance: no document content flows into the
//! `text_correction.replacement` string).
//!
//! ## EU disambiguation
//!
//! `EU` is a registered 2-char `CountryCode` (per
//! `crates/ism/src/attrs.rs::CountryCode::try_new`). In REL TO position
//! the parser tags it as `TokenKind::RelToTrigraph`; in FGI position the
//! parser routes it through the FGI grammar (different `TokenKind`
//! again). Only the SCI-position bare `EU` lands as `TokenKind::Unknown`
//! — so filtering by `Unknown` alone is sufficient as the category gate
//! and no positional logic is needed.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};
use marque_rules::{AppliedFixProposal, Severity};
use std::time::{Duration, UNIX_EPOCH};

const FIXED_TS: u64 = 1_700_000_000;

fn engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme constructs without rewrite cycles")
}

fn lint_e067(source: &[u8]) -> Vec<marque_rules::Diagnostic<marque_capco::CapcoScheme>> {
    let result = engine().lint(source);
    result
        .diagnostics
        .into_iter()
        .filter(|d| d.rule.as_str() == "E067")
        .collect()
}

fn fix_once(source: &[u8]) -> String {
    let result = engine().fix(source, FixMode::Apply);
    String::from_utf8(result.source).expect("engine output is valid UTF-8")
}

// =========================================================================
// CNWDI → RD-CNWDI (§H.6 p106)
// =========================================================================

#[test]
fn bare_cnwdi_rewrites_to_rd_cnwdi() {
    let source = b"(S//CNWDI//NF)";
    let diags = lint_e067(source);
    assert_eq!(
        diags.len(),
        1,
        "exactly one E067 diagnostic expected for bare CNWDI; got {diags:?}"
    );
    let d = &diags[0];
    assert_eq!(d.severity, Severity::Fix);
    assert!(
        d.citation.contains("§H.6 p106"),
        "citation must cite §H.6 p106; got {:?}",
        d.citation
    );

    // Replacement is a hardcoded static literal — assert via the
    // text_correction handle directly.
    let tc = d
        .text_correction
        .as_ref()
        .expect("E067 emits text_correction");
    assert_eq!(tc.replacement.as_str(), "RD-CNWDI");

    let fixed = fix_once(source);
    assert_eq!(fixed, "(S//RD-CNWDI//NF)");
}

#[test]
fn compound_rd_cnwdi_does_not_fire_e067() {
    // The canonical `(S//RD-CNWDI//NF)` should pass cleanly — E067
    // only fires on the bare `CNWDI` token that the parser tags as
    // `Unknown`.
    let source = b"(S//RD-CNWDI//NF)";
    let diags = lint_e067(source);
    assert!(
        diags.is_empty(),
        "canonical RD-CNWDI must not fire E067; got {diags:?}"
    );
}

// =========================================================================
// NK → SI-NK (§H.4 p83)
// =========================================================================

#[test]
fn bare_nk_rewrites_to_si_nk() {
    let source = b"(TS//NK)";
    let diags = lint_e067(source);
    assert_eq!(
        diags.len(),
        1,
        "exactly one E067 diagnostic expected for bare NK; got {diags:?}"
    );
    let d = &diags[0];
    assert_eq!(d.severity, Severity::Fix);
    assert!(
        d.citation.contains("§H.4 p83"),
        "citation must cite §H.4 p83; got {:?}",
        d.citation
    );
    let tc = d
        .text_correction
        .as_ref()
        .expect("E067 emits text_correction");
    assert_eq!(tc.replacement.as_str(), "SI-NK");

    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//SI-NK)");
}

#[test]
fn compound_si_nk_does_not_fire_e067() {
    let source = b"(TS//SI-NK)";
    let diags = lint_e067(source);
    assert!(
        diags.is_empty(),
        "canonical SI-NK must not fire E067; got {diags:?}"
    );
}

// =========================================================================
// EU → SI-EU (§H.4 p78) — SCI position only
// =========================================================================

#[test]
fn bare_eu_in_sci_position_rewrites_to_si_eu() {
    let source = b"(TS//EU)";
    let diags = lint_e067(source);
    assert_eq!(
        diags.len(),
        1,
        "exactly one E067 diagnostic expected for bare EU in SCI position; got {diags:?}"
    );
    let d = &diags[0];
    assert_eq!(d.severity, Severity::Fix);
    assert!(
        d.citation.contains("§H.4 p78"),
        "citation must cite §H.4 p78; got {:?}",
        d.citation
    );
    let tc = d
        .text_correction
        .as_ref()
        .expect("E067 emits text_correction");
    assert_eq!(tc.replacement.as_str(), "SI-EU");

    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//SI-EU)");
}

#[test]
fn eu_in_rel_to_position_does_not_fire_e067() {
    // `EU` in REL TO position parses as `TokenKind::RelToTrigraph`,
    // not `Unknown`, so the walker's category gate filters it out.
    let source = b"(S//REL TO USA, EU)";
    let diags = lint_e067(source);
    assert!(
        diags.is_empty(),
        "EU in REL TO position is RelToTrigraph and must not fire E067; got {diags:?}"
    );
}

#[test]
fn eu_in_fgi_position_does_not_fire_e067() {
    // `EU` in FGI position routes through the FGI grammar — also not
    // `Unknown`. Defensive coverage that the SCI-position gate is the
    // only path that fires E067.
    let source = b"(//FGI EU//NF)";
    let diags = lint_e067(source);
    assert!(
        diags.is_empty(),
        "EU in FGI position must not fire E067; got {diags:?}"
    );
}

#[test]
fn compound_si_eu_does_not_fire_e067() {
    let source = b"(TS//SI-EU//REL TO USA, CAN)";
    let diags = lint_e067(source);
    assert!(
        diags.is_empty(),
        "canonical SI-EU must not fire E067; got {diags:?}"
    );
}

// =========================================================================
// End-to-end audit-stream coverage
// =========================================================================

#[test]
fn e067_applies_in_fix_pass() {
    // Drive through Engine::fix; assert the AppliedFix audit record
    // carries the rule ID and the hardcoded canonical replacement.
    let source = b"(S//CNWDI//NF)";
    let result = engine().fix(source, FixMode::Apply);

    let e067_applied: Vec<_> = result
        .applied
        .iter()
        .filter(|a| a.rule.as_str() == "E067")
        .collect();
    assert_eq!(
        e067_applied.len(),
        1,
        "expected exactly one E067 AppliedFix audit record; got {} \
         (full audit set: {:?})",
        e067_applied.len(),
        result
            .applied
            .iter()
            .map(|a| a.rule.as_str())
            .collect::<Vec<_>>(),
    );
    let applied = e067_applied[0];
    match &applied.proposal {
        AppliedFixProposal::TextCorrection { replacement } => {
            assert_eq!(
                replacement.as_str(),
                "RD-CNWDI",
                "E067 must promote a TextCorrection whose replacement is the \
                 hardcoded canonical `RD-CNWDI`; got {replacement:?}"
            );
        }
        AppliedFixProposal::FixIntent(_) => {
            panic!("E067 must promote a TextCorrection, not a FixIntent");
        }
    }
}
