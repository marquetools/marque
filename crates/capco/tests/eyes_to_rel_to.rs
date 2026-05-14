// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T135a Commit 5 (issue #307) — E064 EYES / EYES ONLY → REL TO conversion.
//!
//! Authority: CAPCO-2016 §H.8 p157 + §H.8 p158.
//!
//! §H.8 p157: EYES ONLY is NSA-only and deprecated; the markings waiver
//! expired 1 Oct 2017 (after manual publication).
//!
//! §H.8 p158: "When extracting EYES ONLY portions from SIGINT
//! reporting, convert the EYES ONLY portion marks to REL TO" and
//! "carry forward the trigraph/tetragraph codes listed in the source
//! document banner line to the new portion mark."
//!
//! The fix is a `text_correction` covering the compound EYES block
//! span; the replacement string follows the §H.3 USA-first + alpha
//! sort and §A.6 p16 `, ` separator conventions for REL TO.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};
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

fn lint_e064(source: &[u8]) -> Vec<marque_rules::Diagnostic<marque_capco::CapcoScheme>> {
    let result = engine().lint(source);
    result
        .diagnostics
        .into_iter()
        .filter(|d| d.rule.as_str() == "E064")
        .collect()
}

fn fix_once(source: &[u8]) -> String {
    let result = engine().fix(source, FixMode::Apply);
    String::from_utf8(result.source).expect("engine output is valid UTF-8")
}

// =========================================================================
// E064 — EYES / EYES ONLY conversions
// =========================================================================

#[test]
fn eyes_only_with_fiveeyes_list_converts_to_rel_to() {
    // §H.8 p158 — "carry forward the trigraph codes"; §H.3 — USA first
    // in country lists. Input `USA/GBR/CAN EYES ONLY` becomes REL TO
    // with USA prepended and the remaining codes alphabetically sorted.
    let source = b"(S//USA/GBR/CAN EYES ONLY)";
    let diags = lint_e064(source);
    assert_eq!(diags.len(), 1, "exactly one E064 diagnostic expected");
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(
        diags[0].citation.contains("§H.8 p157"),
        "citation must cite §H.8 p157; got {:?}",
        diags[0].citation
    );

    let fixed = fix_once(source);
    assert_eq!(
        fixed, "(S//REL TO USA, CAN, GBR)",
        "EYES ONLY must convert to REL TO with USA first, remaining alpha-sorted"
    );
}

#[test]
fn eyes_short_form_converts_to_rel_to() {
    // `EYES` without `ONLY` is also covered per §H.8 p157 (the
    // markings waiver applies to both forms).
    let source = b"(S//USA/GBR EYES)";
    let fixed = fix_once(source);
    assert_eq!(fixed, "(S//REL TO USA, GBR)");
}

#[test]
fn eyes_only_without_usa_in_list_prepends_usa() {
    // §H.3: REL TO mandates USA-first. If the EYES block omitted USA
    // from the trigraph list, the canonical REL TO replacement still
    // prepends USA (REL TO is always to USA + the listed countries).
    let source = b"(S//GBR/CAN EYES ONLY)";
    let fixed = fix_once(source);
    assert_eq!(fixed, "(S//REL TO USA, CAN, GBR)");
}

#[test]
fn eyes_only_with_only_usa_converts_to_rel_to_usa_only() {
    // Edge case: the trigraph list contains only USA. The conversion
    // produces `REL TO USA`.
    let source = b"(S//USA EYES ONLY)";
    let fixed = fix_once(source);
    assert_eq!(fixed, "(S//REL TO USA)");
}

#[test]
fn eyes_only_with_duplicate_trigraphs_dedups() {
    // Defensive: a malformed `USA/USA EYES` collapses to a single USA
    // in the output. The §H.8 conversion is not the place to surface
    // duplicates — E052 (REL TO no duplicates) is the dedicated rule
    // for that case.
    let source = b"(S//USA/USA EYES ONLY)";
    let fixed = fix_once(source);
    assert_eq!(fixed, "(S//REL TO USA)");
}

#[test]
fn bare_eyes_without_trigraphs_does_not_trigger_e064() {
    // E064 requires the compound `<trigraphs> EYES` form. Bare `EYES`
    // (no preceding trigraph list) is out of E064's scope — §H.8
    // p158's "carry forward the trigraph codes" guidance does not
    // apply when no codes were given. The user wrote a bare EYES
    // marker; Marque cannot synthesize a country list.
    let source = b"(S//EYES)";
    let diags = lint_e064(source);
    assert!(
        diags.is_empty(),
        "bare EYES (no trigraph list) is E064-out-of-scope"
    );
}

#[test]
fn fix_round_trip_idempotent() {
    let source = b"(S//USA/GBR/CAN EYES ONLY)";
    let pass1 = fix_once(source);
    assert_eq!(pass1, "(S//REL TO USA, CAN, GBR)");
    let pass2 = fix_once(pass1.as_bytes());
    assert_eq!(pass1, pass2, "second fix pass must be a no-op fixed point");
}
