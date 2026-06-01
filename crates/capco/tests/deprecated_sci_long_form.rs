// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Deprecated SCI long-form canonicalization (issue #307) —
//! walker integration tests.
//!
//! Exercises the end-to-end engine flow for the deprecated SCI
//! long-form walker (`DeprecatedSciLongFormRule`, wire ID
//! `capco:portion.sci.deprecated-long-form`) per row in the catalog in
//! `marque-capco`'s `rules::sci_deprecated` module.
//!
//! Authority: CAPCO-2016 §H.4 pp 61, 62, 74, 76, 78, 85.
//!
//! Each test exercises a specific catalog row through the production
//! `Engine::fix` / `Engine::lint` path so the parser → walker →
//! text-correction → audit-stream pipeline is verified in one shot.
//! Per-row severity is asserted explicitly so an accidental severity
//! widening (Warn → Error or vice versa) is caught at the test level
//! before it lands in a release.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{CapcoEngine, FixMode, FixedClock};
use marque_rules::Severity;
use secrecy::ExposeSecret as _;
use std::time::{Duration, UNIX_EPOCH};

const FIXED_TS: u64 = 1_700_000_000;

fn engine() -> CapcoEngine {
    CapcoEngine::with_clock(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme constructs without rewrite cycles")
}

/// Helper: collect every E065 diagnostic emitted by `Engine::lint`
/// on `source`.
fn lint_e065(source: &[u8]) -> Vec<marque_rules::Diagnostic<marque_capco::CapcoScheme>> {
    let result = engine().lint(source);
    result
        .diagnostics
        .into_iter()
        .filter(|d| d.rule.predicate_id() == "portion.sci.deprecated-long-form")
        .collect()
}

/// Helper: drive `Engine::fix` once and return the fixed text.
fn fix_once(source: &[u8]) -> String {
    let result = engine().fix(source, FixMode::Apply);
    String::from_utf8(result.source.expose_secret().to_vec()).expect("engine output is valid UTF-8")
}

// =========================================================================
// HCS family — §H.4 p62
// =========================================================================

#[test]
fn humint_bare_rewrites_to_hcs_at_error_severity() {
    let source = b"(TS//HUMINT//NF)";
    let diags = lint_e065(source);
    assert_eq!(diags.len(), 1, "exactly one E065 diagnostic expected");
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(
        format!("{}", diags[0].citation).contains("§H.4 p62"),
        "expected §H.4 p62 citation; got {:?}",
        diags[0].citation
    );

    let fixed = fix_once(source);
    assert_eq!(
        fixed, "(TS//HCS//NF)",
        "HUMINT must be re-marked to HCS per §H.4 p62"
    );
}

#[test]
fn humint_control_system_rewrites_to_hcs() {
    let source = b"(TS//HUMINT CONTROL SYSTEM//NF)";
    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//HCS//NF)");
}

// =========================================================================
// SI family (COMINT / SPECIAL INTELLIGENCE) — §H.4 p74
// =========================================================================

#[test]
fn comint_rewrites_to_si_at_error_severity() {
    let source = b"(TS//COMINT//NF)";
    let diags = lint_e065(source);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(format!("{}", diags[0].citation).contains("§H.4 p74"));

    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//SI//NF)");
}

#[test]
fn special_intelligence_rewrites_to_si() {
    let source = b"(TS//SPECIAL INTELLIGENCE//NF)";
    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//SI//NF)");
}

// =========================================================================
// SI family (ECI / EXCEPTIONALLY CONTROLLED INFORMATION) — §H.4 p61 + p76
// =========================================================================

#[test]
fn eci_with_compartment_rewrites_to_si_compartment() {
    // §H.4 p76: "information formerly marked TS//SI-ECI ABC must now be
    // marked TS//SI-ABC".
    let source = b"(TS//ECI ABC//NF)";
    let diags = lint_e065(source);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
    // The typed `Citation` carries one §-reference per diagnostic. The
    // catalog row anchors at §H.4 p61 (SCI grammar);
    // the row.message documentation still cross-references p76.
    assert!(
        format!("{}", diags[0].citation).contains("§H.4 p61"),
        "expected §H.4 p61 citation; got {:?}",
        diags[0].citation
    );

    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//SI-ABC//NF)");
}

#[test]
fn exceptionally_controlled_information_with_compartment_rewrites() {
    let source = b"(TS//EXCEPTIONALLY CONTROLLED INFORMATION ABC//NF)";
    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//SI-ABC//NF)");
}

#[test]
fn bare_eci_is_suggest_only_no_fix() {
    // Bare ECI cannot be canonicalized — compartment context is required
    // per §H.4 p76. Walker emits Info diagnostic at Error severity, no
    // fix proposal.
    let source = b"(TS//ECI//NF)";
    let diags = lint_e065(source);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(
        diags[0].text_correction.is_none(),
        "bare ECI must NOT carry a text_correction (compartment missing)"
    );

    // Fix is a no-op (no fix proposed).
    let fixed = fix_once(source);
    assert_eq!(
        fixed, "(TS//ECI//NF)",
        "bare ECI input must be unchanged when no fix is proposed"
    );
}

// =========================================================================
// SI family (EL / ENDSEAL) — §H.4 p78 + p83
// =========================================================================

#[test]
fn el_ecru_rewrites_to_si_ecru() {
    // §H.4 p78: "the EL control system is being retired and all
    // associated compartments moved to the SI control system".
    let source = b"(TS//EL ECRU//NF)";
    let diags = lint_e065(source);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(format!("{}", diags[0].citation).contains("§H.4 p78"));

    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//SI-ECRU//NF)");
}

#[test]
fn endseal_with_compartment_rewrites_to_si() {
    let source = b"(TS//ENDSEAL ECRU//NF)";
    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//SI-ECRU//NF)");
}

#[test]
fn bare_endseal_is_error_suggest_only() {
    // bare ENDSEAL = retired control system, Error + suggest-only.
    // The EL/ENDSEAL control system itself was retired (EL/ENDSEAL → SI;
    // transition predates CAPCO-2016, likely in the 2013 manual). Unlike
    // bare HCS / RSV (E061 / E063) where the control system is valid and
    // just needs the compartment, the bare form here has no canonical
    // migration because the source control system is gone. The user MUST
    // contact the originator for the compartment to complete the
    // migration; Marque can't fabricate it.
    let source = b"(TS//ENDSEAL//NF)";
    let diags = lint_e065(source);
    assert_eq!(diags.len(), 1);
    assert_eq!(
        diags[0].severity,
        Severity::Error,
        "bare ENDSEAL is Error — the EL/ENDSEAL control system itself is retired"
    );
    assert!(diags[0].text_correction.is_none());
}

#[test]
fn bare_el_is_error_suggest_only() {
    // bare EL = retired control system, Error + suggest-only. Same
    // reasoning as bare ENDSEAL: the EL control system itself was
    // retired (EL/ENDSEAL → SI). No canonical migration without the
    // compartment context.
    let source = b"(TS//EL//NF)";
    let diags = lint_e065(source);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(diags[0].text_correction.is_none());
}

// =========================================================================
// TK family (KDK / KLONDIKE) — §H.4 p85 (NSG PM 3802)
// =========================================================================

#[test]
fn kdk_bluefish_rewrites_to_tk_blfh() {
    // §H.4 p85 (NSG PM 3802 closure): "re-mark the new document and
    // associated portions according to the instructions in the TK-BLFH,
    // TK-IDIT, and TK-KAND marking templates."
    //
    // §H.4 p87 documents the BLUEFISH → BLFH portion-mark abbreviation;
    // §H.4 p91 documents IDITAROD → IDIT; §H.4 p95 documents KANDIK →
    // KAND. The walker translates the captured legacy compartment via
    // `KDK_COMPARTMENT_MAPPING` to the canonical short form. The CVE
    // vocabulary registers only `TK-BLFH` / `TK-IDIT` / `TK-KAND` — no
    // entries exist for the long forms, so emitting `TK-BLUEFISH` would
    // produce a marking with no CVE backing.
    let source = b"(TS//KDK-BLUEFISH//NF)";
    let diags = lint_e065(source);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(format!("{}", diags[0].citation).contains("§H.4 p85"));

    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//TK-BLFH//NF)");
}

#[test]
fn klondike_iditarod_rewrites_to_tk_idit() {
    // §H.4 p85 + §H.4 p91 (IDITAROD → IDIT portion-mark abbreviation).
    let source = b"(TS//KLONDIKE-IDITAROD//NF)";
    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//TK-IDIT//NF)");
}

#[test]
fn kdk_kandik_rewrites_to_tk_kand() {
    // §H.4 p85 + §H.4 p95 (KANDIK → KAND portion-mark abbreviation).
    let source = b"(TS//KDK-KANDIK//NF)";
    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//TK-KAND//NF)");
}

#[test]
fn kdk_unknown_compartment_emits_warn_no_fix() {
    // KDK-FROBNITZ — an undocumented compartment. The walker cannot
    // fabricate a canonical TK- short form (the BLUEFISH → BLFH
    // pattern is per-compartment, not a general truncation rule), so
    // it emits a Warn-severity diagnostic with no text correction.
    // Producing `TK-FROBNITZ` (an invalid CVE) would be strictly
    // worse than no fix.
    let source = b"(TS//KDK-FROBNITZ//NF)";
    let diags = lint_e065(source);
    assert_eq!(diags.len(), 1);
    assert_eq!(
        diags[0].severity,
        Severity::Warn,
        "unknown KDK compartment must downgrade to Warn (no fabricated fix)"
    );
    assert!(
        diags[0].text_correction.is_none(),
        "unknown KDK compartment must NOT carry a text_correction"
    );
    // `Diagnostic.message` is a closed `Message` (template + args);
    // free-form prose is absent from the message body per Constitution
    // V Principle V (audit content-ignorance). The deprecation class is
    // captured by the
    // `SupersededToken` template; the no-fix signal is captured by
    // the `text_correction.is_none()` assertion above and the
    // unchanged-source assertion below.
    use marque_rules::MessageTemplate;
    assert_eq!(
        diags[0].message.template(),
        MessageTemplate::SupersededToken,
        "deprecated KDK class fires under the SupersededToken template; got {:?}",
        diags[0].message.template(),
    );

    // No-op fix: input unchanged because no text correction was emitted.
    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//KDK-FROBNITZ//NF)");
}

#[test]
fn bare_kdk_is_error_suggest_only() {
    // bare KDK = retired control system, Error + suggest-only. The
    // KDK/KLONDIKE control system itself was retired (KDK/KLONDIKE → TK;
    // transition predates CAPCO-2016, documented in NSG PM 3802). Unlike
    // bare HCS / RSV (E061 / E063) where the control system is valid and
    // just needs the compartment, the bare form here has no canonical
    // migration because the source control system is gone. The user MUST
    // contact the originator for the compartment to complete the
    // migration; Marque can't fabricate it.
    let source = b"(TS//KDK//NF)";
    let diags = lint_e065(source);
    assert_eq!(diags.len(), 1);
    assert_eq!(
        diags[0].severity,
        Severity::Error,
        "bare KDK is Error — the KDK/KLONDIKE control system itself is retired"
    );
    assert!(diags[0].text_correction.is_none());
}

#[test]
fn bare_klondike_is_error_suggest_only() {
    // bare KLONDIKE = retired control system, Error + suggest-only. Same
    // reasoning as bare KDK: the KDK/KLONDIKE control system itself was
    // retired (KDK/KLONDIKE → TK per NSG PM 3802). No canonical
    // migration without the compartment context.
    let source = b"(TS//KLONDIKE//NF)";
    let diags = lint_e065(source);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(diags[0].text_correction.is_none());
}

// =========================================================================
// Regression guards
// =========================================================================

#[test]
fn canonical_hcs_not_flagged() {
    // Negative test: canonical HCS must NOT fire E065. The walker only
    // matches deprecated long-forms, not the canonical short-form HCS.
    let source = b"(TS//HCS//NF)";
    let diags = lint_e065(source);
    assert!(
        diags.is_empty(),
        "canonical HCS must not trigger E065 (walker matches deprecated forms only); \
         got {} diagnostics",
        diags.len()
    );
}

#[test]
fn canonical_si_not_flagged() {
    let source = b"(TS//SI//NF)";
    let diags = lint_e065(source);
    assert!(diags.is_empty());
}

#[test]
fn canonical_tk_not_flagged() {
    let source = b"(TS//TK//NF)";
    let diags = lint_e065(source);
    assert!(diags.is_empty());
}

#[test]
fn fix_round_trip_idempotent() {
    // After one Engine::fix pass, a deprecated long-form is canonicalized.
    // A second pass over the canonical output is a fixed point — no
    // further E065 diagnostics fire.
    let source = b"(TS//HUMINT//NF)";
    let pass1 = fix_once(source);
    assert_eq!(pass1, "(TS//HCS//NF)");
    let pass2 = fix_once(pass1.as_bytes());
    assert_eq!(pass1, pass2, "second fix pass must be a no-op fixed point");
}

// =========================================================================
// Multi-system SCI long-form canonicalization.
//
// Before the fix, the deprecated SCI long-form recognizer
// (`recognize_deprecated_sci_long_form`) only ran at the whole-`//`-block
// level (parser.rs ~lines 407-475). For multi-system SCI inputs like
// `(TS//COMINT/TK//NF)` or `(TS//KDK-BLUEFISH/SI//NF)`, the dispatch goes
// through `parse_sci_block` (because the block contains `/`). Inside
// `parse_sci_block`'s per-chunk loop the recognizer was NOT called, so:
//
// - `COMINT` / `HUMINT` (>5 chars, fail `is_valid_custom_control`) →
//   parse_sci_block returns None → whole block becomes Unknown / E008.
// - `KDK-BLUEFISH` (KDK = 3 chars, fits custom-control shape) → parses
//   as `SciControlSystem::Custom("KDK")` with BLUEFISH compartment. The
//   E065 walker fires (text-prefix match on "KDK-") but the marking is
//   structurally Custom (triggering W034 unpublished-control noise)
//   instead of the canonical `Published(Tk)` with `BLFH` compartment.
// - `ECI ABC` / `EL ECRU` (space inside chunk) → parses neither as
//   CVE-bare nor custom-control; rejects.
//
// The fix calls `recognize_deprecated_sci_long_form` per chunk inside
// `parse_sci_block` BEFORE the structural CVE-bare / custom-control
// checks. The chunk-level path constructs a canonical `SciMarking`
// (Published system + canonical compartment), dual-emits SciControl +
// SciSystem spans with the chunk source bytes verbatim, and preserves
// the per-chunk all-or-nothing parse semantic.
//
// Authority: CAPCO-2016 §H.4 pp 61, 62, 74, 76, 78, 85.
// =========================================================================

#[test]
fn multi_system_comint_tk_canonicalizes_to_si_tk() {
    // `COMINT` is the deprecated long-form of SI (§H.4 p74). Before the
    // fix, this multi-system input rejected `parse_sci_block` (COMINT is
    // 6 chars, fails `is_valid_custom_control`) and the whole block
    // became Unknown — E065 never fired.
    let source = b"(TS//COMINT/TK//NF)";
    let diags = lint_e065(source);
    assert_eq!(
        diags.len(),
        1,
        "exactly one E065 diagnostic expected for COMINT chunk in multi-system block"
    );
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(format!("{}", diags[0].citation).contains("§H.4 p74"));

    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//SI/TK//NF)");
}

#[test]
fn multi_system_kdk_bluefish_si_canonicalizes_to_tk_blfh_si() {
    // `KDK-BLUEFISH` is the deprecated long-form of TK-BLFH (§H.4 p85 +
    // p87). Before the fix, this parsed as `SciControlSystem::Custom("KDK")`
    // + BLUEFISH compartment (and emitted W034 unpublished-control noise);
    // after the fix it parses as `Published(Tk)` + canonical BLFH
    // compartment with no W034 noise.
    let source = b"(TS//KDK-BLUEFISH/SI//NF)";
    let diags = lint_e065(source);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(format!("{}", diags[0].citation).contains("§H.4 p85"));

    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//TK-BLFH/SI//NF)");
}

#[test]
fn multi_system_humint_si_canonicalizes_to_hcs_si() {
    // `HUMINT` is the deprecated long-form of HCS (§H.4 p62). Before the
    // fix, this multi-system input rejected `parse_sci_block` (HUMINT
    // is 6 chars).
    let source = b"(TS//HUMINT/SI//NF)";
    let diags = lint_e065(source);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(format!("{}", diags[0].citation).contains("§H.4 p62"));

    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//HCS/SI//NF)");
}

#[test]
fn multi_system_klondike_iditarod_tk_canonicalizes() {
    // `KLONDIKE-IDITAROD` is the deprecated long-form of TK-IDIT (§H.4
    // p85 + p91). KLONDIKE (8 chars) exceeds the custom-control shape
    // cap, so before the fix this rejected. Note: the canonical output
    // has TK appearing twice (once from the KLONDIKE-IDITAROD → TK-IDIT
    // canonicalization, once from the original /TK chunk). This tests
    // the all-or-nothing parse semantic doesn't reject valid duplicates.
    let source = b"(TS//KLONDIKE-IDITAROD/TK//NF)";
    let diags = lint_e065(source);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(format!("{}", diags[0].citation).contains("§H.4 p85"));

    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//TK-IDIT/TK//NF)");
}

#[test]
fn multi_system_eci_abc_si_canonicalizes() {
    // `ECI ABC` is the deprecated long-form of SI-ABC (§H.4 p61 + p76).
    // The internal space caused the whole chunk to fail both
    // CVE-bare and custom-control recognition before the fix.
    let source = b"(TS//ECI ABC/SI//NF)";
    let diags = lint_e065(source);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(
        format!("{}", diags[0].citation).contains("§H.4 p61"),
        "citation must cite §H.4 p61; got {:?}",
        diags[0].citation
    );

    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//SI-ABC/SI//NF)");
}

#[test]
fn regression_multi_system_canonical_unchanged() {
    // Pure-canonical multi-system input must NOT fire E065. The
    // recognizer-fallback flow only kicks in for deprecated forms;
    // canonical SI/TK passes through the structural path unchanged.
    let source = b"(TS//SI/TK//NF)";
    let diags = lint_e065(source);
    assert!(
        diags.is_empty(),
        "canonical multi-system SI/TK must not trigger E065; got {} diagnostics",
        diags.len()
    );

    let fixed = fix_once(source);
    assert_eq!(fixed, "(TS//SI/TK//NF)");
}

#[test]
fn custom_sci_control_still_recognized() {
    // Negative test: a multi-system block with a custom (non-deprecated)
    // control system in one chunk must still parse via the
    // custom-control fallback. The fix's deprecated-form check is
    // mutually exclusive with the custom-control path; an unrecognized
    // long-form falls through to `is_valid_custom_control`. `AB123` is
    // a 5-char custom control (fits the [A-Z0-9]{2,5} shape gate); TK
    // is canonical CVE.
    let source = b"(TS//AB123/TK//NF)";
    let diags = lint_e065(source);
    assert!(
        diags.is_empty(),
        "custom-control AB123 must not trigger E065 (not a deprecated form); \
         got {} diagnostics",
        diags.len()
    );
    // Note: W034 (unpublished SCI control) fires on `AB123` because it's
    // a custom (non-Published) control — that's the existing
    // unpublished-control behavior, independent of E065.
}
