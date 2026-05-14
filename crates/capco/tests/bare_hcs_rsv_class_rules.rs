// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 9a — bare HCS / bare RSV class-specific rules (E061 / E062 / E063).
//!
//! Authority: CAPCO-2016 §H.4 pp 62, 70.
//!
//! - **E061** (`hcs-bare-at-confidential-legacy-remark`): bare HCS at
//!   CONFIDENTIAL — legacy guidance per §H.4 p62: "When legacy
//!   information at the CONFIDENTIAL//HCS level is discovered, contact
//!   the originator for guidance prior to reusing the information."
//!   Warn severity, no fix.
//!
//! - **E062** (`hcs-bare-suggest-subcompartment`): bare HCS at
//!   SECRET / TOP SECRET — per §H.4 p62 re-mark guidance. Emits 3
//!   per-candidate Suggest diagnostics (HCS-O / HCS-P / HCS-O-P) so
//!   editors can offer one-click substitution. The classifier picks
//!   the right one based on content (Operations vs Product). Warn
//!   severity at the rule level.
//!
//! - **E063** (`rsv-bare-requires-compartment`): bare RSV — per §H.4
//!   p70 "the RSV marking may not be used alone and requires the
//!   associated compartment". Error severity, no fix (the compartment
//!   identifier is org-private content beyond Marque's vocabulary).

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{Engine, FixedClock};
use marque_rules::Severity;
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

fn lint(rule: &str, source: &[u8]) -> Vec<marque_rules::Diagnostic<marque_capco::CapcoScheme>> {
    let result = engine().lint(source);
    result
        .diagnostics
        .into_iter()
        .filter(|d| d.rule.as_str() == rule)
        .collect()
}

// =========================================================================
// E061 — bare HCS at CONFIDENTIAL
// =========================================================================

#[test]
fn e061_fires_on_bare_hcs_at_confidential() {
    let source = b"(CONFIDENTIAL//HCS//NOFORN)";
    let diags = lint("E061", source);
    assert_eq!(diags.len(), 1, "exactly one E061 diagnostic expected");
    assert_eq!(diags[0].severity, Severity::Warn);
    assert!(
        diags[0].citation.contains("§H.4 p62"),
        "citation must cite §H.4 p62; got {:?}",
        diags[0].citation
    );
    assert!(
        diags[0].message.contains("contact the originator"),
        "diagnostic must mirror §H.4 p62 'contact the originator' guidance; got {:?}",
        diags[0].message
    );
}

#[test]
fn e061_does_not_fire_at_secret() {
    // E061 is class-specific to CONFIDENTIAL; bare HCS at SECRET is
    // E062's domain.
    let source = b"(SECRET//HCS//NOFORN)";
    let diags = lint("E061", source);
    assert!(diags.is_empty(), "E061 must not fire outside CONFIDENTIAL");
}

#[test]
fn e061_does_not_fire_at_top_secret() {
    let source = b"(TOP SECRET//HCS//NOFORN)";
    let diags = lint("E061", source);
    assert!(diags.is_empty());
}

#[test]
fn e061_does_not_fire_when_hcs_has_compartment() {
    // Bare HCS only — compound HCS-O / HCS-P / HCS-O-P forms are not
    // legacy.
    let source = b"(CONFIDENTIAL//HCS-O//NOFORN)";
    let diags = lint("E061", source);
    assert!(
        diags.is_empty(),
        "E061 must not fire when HCS carries a compartment"
    );
}

// =========================================================================
// E062 — bare HCS at SECRET / TOP SECRET
// =========================================================================

#[test]
fn e062_emits_three_candidates_at_secret() {
    // §H.4 p62 — re-mark to HCS-O / HCS-P / HCS-O-P templates.
    let source = b"(SECRET//HCS//NOFORN)";
    let diags = lint("E062", source);
    assert_eq!(
        diags.len(),
        3,
        "exactly 3 per-candidate diagnostics expected (HCS-O, HCS-P, HCS-O-P)"
    );

    let replacements: Vec<String> = diags
        .iter()
        .filter_map(|d| {
            d.text_correction
                .as_ref()
                .map(|t| t.replacement.to_string())
        })
        .collect();
    assert!(replacements.contains(&"HCS-O".to_owned()));
    assert!(replacements.contains(&"HCS-P".to_owned()));
    assert!(replacements.contains(&"HCS-O-P".to_owned()));

    // Per-diagnostic severity is the rule's emitted Suggest — the
    // engine only overwrites severity from `config.rules.overrides`
    // (the user's `.marque.toml`), not from `default_severity()`.
    // The rule emits Severity::Suggest per-candidate so the engine's
    // auto-apply gate never promotes them; the user picks the right
    // one via UI.
    for d in &diags {
        assert_eq!(
            d.severity,
            Severity::Suggest,
            "per-candidate diagnostics emit at Suggest severity so engine never auto-applies"
        );
    }
}

#[test]
fn e062_emits_three_candidates_at_top_secret() {
    let source = b"(TOP SECRET//HCS//NOFORN)";
    let diags = lint("E062", source);
    assert_eq!(diags.len(), 3);
}

#[test]
fn e062_does_not_fire_at_confidential() {
    // E062 is class-specific to S/TS; bare HCS at C is E061's domain.
    let source = b"(CONFIDENTIAL//HCS//NOFORN)";
    let diags = lint("E062", source);
    assert!(diags.is_empty());
}

#[test]
fn e062_does_not_fire_when_hcs_has_compartment() {
    let source = b"(SECRET//HCS-O//NOFORN)";
    let diags = lint("E062", source);
    assert!(diags.is_empty());
}

// =========================================================================
// E063 — bare RSV requires compartment
// =========================================================================

#[test]
fn e063_fires_on_bare_rsv() {
    // §H.4 p70: "the RSV marking may not be used alone and requires
    // the associated compartment".
    let source = b"(TOP SECRET//RSV//NOFORN)";
    let diags = lint("E063", source);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(
        diags[0].citation.contains("§H.4 p70"),
        "citation must cite §H.4 p70; got {:?}",
        diags[0].citation
    );
    assert!(
        diags[0].message.contains("may not be used alone"),
        "diagnostic must mirror §H.4 p70 wording; got {:?}",
        diags[0].message
    );
    // No fix proposed (compartment identifier is org-private).
    assert!(diags[0].text_correction.is_none());
}

#[test]
fn e063_does_not_fire_when_rsv_has_compartment() {
    // `RSV-XYZ` (compartment present) — E063 must not fire.
    let source = b"(TOP SECRET//RSV-XYZ//NOFORN)";
    let diags = lint("E063", source);
    assert!(
        diags.is_empty(),
        "E063 must not fire when RSV carries a compartment"
    );
}

#[test]
fn e063_does_not_fire_on_non_rsv_sci() {
    let source = b"(TOP SECRET//SI//NOFORN)";
    let diags = lint("E063", source);
    assert!(diags.is_empty());
}
