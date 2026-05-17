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

// =========================================================================
// Country-code registry gate — PR 9a Copilot R3 Fix 2 (PR #416).
//
// The shape gate alone (`[A-Z]{3}`) accepts arbitrary uppercase triples
// like `AAA` or `XYZ`. Without registry validation, E064's canonical
// autofix would build `REL TO USA, AAA, XYZ` — silent fabrication of
// trigraphs in audit output. Per Constitution Principle VIII
// (Authoritative Source Fidelity), canonical output MUST reference
// real CAPCO registry entries. The recognizer now validates each
// trigraph via `TokenSet::is_trigraph` + `CountryCode::try_new`,
// mirroring the registry check in `parse_rel_to_with_spans`
// (~parser.rs:1870-1871). An unregistered trigraph rejects the whole
// block — the recognizer is all-or-nothing; the block falls through
// to Unknown rather than producing fabricated REL TO output.
// =========================================================================

#[test]
fn unregistered_trigraph_falls_through_no_e064() {
    // `(S//AAA/XYZ EYES ONLY)` — neither `AAA` nor `XYZ` is a
    // CAPCO-registered country code. The recognizer rejects the block;
    // E064 does NOT fire. Critically, the autofix MUST NOT produce
    // `REL TO USA, AAA, XYZ` — that would be fabricating trigraphs in
    // canonical output (Constitution Principle VIII violation).
    let source = b"(S//AAA/XYZ EYES ONLY)";
    let diags = lint_e064(source);
    assert!(
        diags.is_empty(),
        "E064 must NOT fire when the EYES block contains unregistered \
         trigraphs; got {} diagnostic(s)",
        diags.len(),
    );

    // The fix path must leave the input unchanged (or at least not
    // emit a REL TO that contains the fake trigraphs). The block
    // falls through to Unknown; no E064 splice runs.
    let fixed = fix_once(source);
    assert!(
        !fixed.contains("AAA") || !fixed.contains("REL TO"),
        "fabricated trigraph must not appear in canonical REL TO output; \
         got {:?}",
        fixed,
    );
    assert!(
        !fixed.contains("XYZ") || !fixed.contains("REL TO"),
        "fabricated trigraph must not appear in canonical REL TO output; \
         got {:?}",
        fixed,
    );
}

#[test]
fn single_unregistered_trigraph_falls_through() {
    // `(S//XYZ EYES ONLY)` — a lone unregistered trigraph also rejects.
    let source = b"(S//XYZ EYES ONLY)";
    let diags = lint_e064(source);
    assert!(
        diags.is_empty(),
        "E064 must NOT fire on a single unregistered trigraph; \
         got {} diagnostic(s)",
        diags.len(),
    );
    let fixed = fix_once(source);
    assert!(
        !fixed.contains("REL TO USA, XYZ"),
        "fabricated single-trigraph REL TO must not appear; got {:?}",
        fixed,
    );
}

#[test]
fn mixed_registered_and_unregistered_falls_through() {
    // `(S//USA/XYZ EYES ONLY)` — `USA` is registered, `XYZ` is not.
    // The recognizer's per-segment registry loop is all-or-nothing:
    // even one unregistered trigraph rejects the whole block. Without
    // this gate the autofix would emit `REL TO USA, XYZ` —
    // half-canonical, half-fabricated; the rule is built to either
    // produce a 100% canonical conversion or none.
    let source = b"(S//USA/XYZ EYES ONLY)";
    let diags = lint_e064(source);
    assert!(
        diags.is_empty(),
        "E064 must reject the whole block when ANY trigraph is \
         unregistered; got {} diagnostic(s)",
        diags.len(),
    );
    let fixed = fix_once(source);
    assert!(
        !fixed.contains("REL TO USA, XYZ"),
        "half-canonical REL TO must not appear when mixed registered/\
         unregistered trigraphs are present; got {:?}",
        fixed,
    );
}

#[test]
fn regression_registered_trigraphs_still_recognized() {
    // Lock-in non-regression: `(S//USA/GBR EYES ONLY)` has two
    // registered Five Eyes trigraphs. E064 fires and produces the
    // canonical `(S//REL TO USA, GBR)` conversion exactly as before
    // the R3 fix.
    let source = b"(S//USA/GBR EYES ONLY)";
    let diags = lint_e064(source);
    assert_eq!(
        diags.len(),
        1,
        "two registered trigraphs must still trigger E064"
    );
    let fixed = fix_once(source);
    assert_eq!(fixed, "(S//REL TO USA, GBR)");
}

#[test]
fn regression_full_form_eyes_only() {
    // Full Five Eyes membership — all five trigraphs registered.
    // §H.8 p157 worked example.
    let source = b"(S//USA/GBR/CAN/AUS/NZL EYES ONLY)";
    let diags = lint_e064(source);
    assert_eq!(
        diags.len(),
        1,
        "five registered trigraphs must trigger E064"
    );
    let fixed = fix_once(source);
    assert_eq!(fixed, "(S//REL TO USA, AUS, CAN, GBR, NZL)");
}

// =========================================================================
// Banner-form EYES ONLY tests (issue: EYES ONLY banner-form lexer)
// =========================================================================
//
// The banner forms tested here cover:
//   1. Compound form `SECRET//USA/GBR EYES ONLY` — trigraph list present;
//      recognized by `recognize_eyes_only_block` (PR 9a / T135a Commit 5).
//   2. Bare form `SECRET//EYES ONLY` — no country list; maps to
//      `DissemControl::Eyes` via the MARKING_FORMS entry; E064 fires with
//      the FVEY implied list per §H.8 p157.
//   3. Bare form `SECRET//EYES` — the CVE-value form in a banner; same
//      FVEY treatment as (2).
//
// Authority: CAPCO-2016 §H.8 p157 + p158.
// =========================================================================

#[test]
fn banner_with_trigraph_list_fires_e064() {
    // Compound banner form — trigraph list present. The compound block
    // `USA/CAN/GBR EYES ONLY` appears as the dissem block in a banner
    // line (`SECRET//USA/CAN/GBR EYES ONLY`). E064 fires and replaces it
    // with the canonical REL TO form.
    // Authority: CAPCO-2016 §H.8 p158 "carry forward the trigraph codes".
    let source = b"SECRET//USA/CAN/GBR EYES ONLY";
    let diags = lint_e064(source);
    assert_eq!(
        diags.len(),
        1,
        "banner compound EYES ONLY must fire E064; got {diags:?}"
    );
    let fixed = fix_once(source);
    assert_eq!(
        fixed, "SECRET//REL TO USA, CAN, GBR",
        "banner EYES ONLY with list must convert to REL TO USA-first alpha-sorted"
    );
}

#[test]
fn banner_bare_eyes_only_fires_e064_with_fvey() {
    // Bare banner form — `EYES ONLY` with no country list. Per §H.8 p157
    // a bare EYES ONLY banner without a list implies Five Eyes (FVEY)
    // membership. E064 fires and supplies the FVEY REL TO replacement.
    // Authority: CAPCO-2016 §H.8 p157.
    let source = b"SECRET//EYES ONLY";
    let diags = lint_e064(source);
    assert_eq!(
        diags.len(),
        1,
        "bare EYES ONLY in a banner must fire E064 with FVEY; got {diags:?}"
    );
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(
        diags[0].citation.contains("§H.8 p157"),
        "citation must cite §H.8 p157; got {:?}",
        diags[0].citation
    );
    let fixed = fix_once(source);
    assert_eq!(
        fixed, "SECRET//REL TO USA, AUS, CAN, GBR, NZL",
        "bare EYES ONLY banner must convert to full FVEY REL TO"
    );
}

#[test]
fn banner_bare_eyes_cve_form_fires_e064_with_fvey() {
    // Bare `EYES` (CVE-value form, no `ONLY`) in a banner. Same FVEY
    // treatment as bare `EYES ONLY` — the markings waiver covered both
    // forms per §H.8 p157. E064 fires with FVEY.
    // Authority: CAPCO-2016 §H.8 p157.
    let source = b"SECRET//EYES";
    let diags = lint_e064(source);
    assert_eq!(
        diags.len(),
        1,
        "bare EYES (CVE form) in a banner must fire E064 with FVEY; got {diags:?}"
    );
    let fixed = fix_once(source);
    assert_eq!(
        fixed, "SECRET//REL TO USA, AUS, CAN, GBR, NZL",
        "bare EYES (CVE form) banner must convert to full FVEY REL TO"
    );
}

#[test]
fn bare_eyes_in_portion_still_does_not_fire_e064() {
    // Regression: bare `(S//EYES)` in a portion remains out of E064's
    // scope even after the banner-form extension. A bare portion EYES
    // may be intentional when the page banner has the full country list;
    // Marque must not synthesize the list without banner context.
    // Authority: CAPCO-2016 §H.8 p158 ("carry forward the trigraph codes
    // listed in the source document banner line").
    let source = b"(S//EYES)";
    let diags = lint_e064(source);
    assert!(
        diags.is_empty(),
        "bare EYES in a portion must remain E064-out-of-scope after \
         banner-form extension; got {diags:?}"
    );
}

#[test]
fn banner_eyes_only_fix_is_idempotent() {
    // Second pass on the fixed output `SECRET//REL TO USA, AUS, CAN, GBR, NZL`
    // must be a no-op (fixed-point / idempotent).
    let source = b"SECRET//EYES ONLY";
    let pass1 = fix_once(source);
    assert_eq!(pass1, "SECRET//REL TO USA, AUS, CAN, GBR, NZL");
    let pass2 = fix_once(pass1.as_bytes());
    assert_eq!(
        pass1, pass2,
        "second fix pass on banner EYES ONLY output must be a no-op"
    );
}

#[test]
fn banner_compound_eyes_only_unordered_list_converts_correctly() {
    // EYES block with country list in non-USA-first order. The output
    // must be USA-first with the remaining codes alpha-sorted per §H.3
    // and §H.8 p150-151.
    // Authority: CAPCO-2016 §H.8 p150-151 (REL TO USA-first order).
    let source = b"SECRET//GBR/NZL/USA/AUS/CAN EYES ONLY";
    let diags = lint_e064(source);
    assert_eq!(
        diags.len(),
        1,
        "unordered trigraph list must still fire E064; got {diags:?}"
    );
    let fixed = fix_once(source);
    assert_eq!(
        fixed, "SECRET//REL TO USA, AUS, CAN, GBR, NZL",
        "REL TO output must be USA-first with remainder alpha-sorted \
         regardless of input order"
    );
}
