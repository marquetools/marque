// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Issue #493 — engine-level dispatch pin for the strict-parse
//! rejection paths introduced in #280.
//!
//! #280 tightened two predicates in `marque-ism` so the strict
//! parser stops silently accepting case-permissive input in
//! open-vocabulary categories:
//!
//! - `SarProgram::admits_program_id_abbrev` /
//!   `SarCompartment::admits_identifier` reject lowercase shapes
//!   (`fk`, `j12`).
//! - `CountryCode::admits_fgi_ownership_token` admits only 2- or
//!   3-byte registered country codes (sovereign trigraphs + `EU`) and
//!   the literal `NATO`. FVEY, CFIUS, and other coalition tetragraphs
//!   are rejected (right shape, wrong semantic category).
//!
//! The #280 issue body claimed *"Both changes route lowercase through
//! the existing R001 decoder path, which already produces correct
//! canonical uppercase fixes — no new rule needed."* That claim was
//! asserted but never tested end-to-end at the engine. Tests in
//! `crates/core/tests/fgi_silent_skip_guard.rs` confirm strict-parse
//! rejection (parser returns `None` / empty attrs); they do NOT
//! confirm engine-level decoder dispatch fires and produces the
//! expected diagnostic.
//!
//! This test family pins the **actual** engine-level behavior, which
//! diverges from the #280 issue-body claim in two places:
//!
//! 1. SAR lowercase inputs DO reach the decoder and DO emit R001 with
//!    `FixSource::DecoderPosterior` — but at `Severity::Suggest`, not
//!    `Severity::Fix`. `Suggest` is a human-judgment channel and does
//!    NOT auto-apply, so `engine.fix(...)` returns the original bytes
//!    unchanged. The #280 claim of "auto-fix" overstates today's
//!    behavior; pinning this here so a future decoder-severity
//!    revision is a visible, considered change rather than silent
//!    drift.
//! 2. FGI category-mismatch tetragraphs (`FVEY`, `DEUX`) do NOT
//!    reach the decoder under the default dispatcher — the strict
//!    parser surfaces enough partial structure (the literal `FGI`
//!    marker) that the dispatcher treats the strict result as
//!    non-trivial and skips the decoder fallback. The user-visible
//!    diagnostic is `E008` ("unrecognized token inside marking") at
//!    `Severity::Error`. This is the correct end-user signal — "this
//!    isn't a valid FGI ownership token" — even though it doesn't
//!    route through R001.
//!
//! The lowercase-trigraph case (`(S//FGI deu)`) does work end-to-end:
//! decoder fires R001 at `Severity::Fix`, auto-applies, and the
//! output is the byte-canonical `(S//FGI DEU)` form. This is the only
//! #280 case where the issue-body claim is fully realized today.
//!
//! Authority: this file is a regression-guard, not a primary-source
//! grammar test. The underlying §-citations live on the rules and
//! predicates these tests exercise — see #280's parent commits and
//! `crates/core/tests/fgi_silent_skip_guard.rs` for the strict-parse
//! anchors.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::{Engine, FixMode};
use marque_rules::{Diagnostic, FixSource, RuleSet, Severity};
use secrecy::ExposeSecret as _;

fn build_engine() -> Engine {
    let rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![Box::new(CapcoRuleSet::new())];
    Engine::new(Config::default(), rule_sets, CapcoScheme::new())
        .expect("default CAPCO scheme constructs without rewrite cycles")
}

/// Build an engine with `confidence_threshold = 0.0`. The engine
/// demotes any `Severity::Fix` diagnostic whose `combined()`
/// confidence falls below the threshold to `Severity::Suggest`, and
/// only `Severity::Fix` diagnostics survive the apply-gate filter
/// (`Severity::Suggest` is a hard exclusion regardless of
/// confidence — see `Engine::fix_inner`). Lowering the threshold to
/// zero keeps SAR-shape decoder fixes at `Severity::Fix` long enough
/// to actually land, so the test can read the canonical bytes back
/// out of `engine.fix(...).source`.
fn build_engine_threshold_zero() -> Engine {
    let mut config = Config::default();
    config
        .set_confidence_threshold(0.0)
        .expect("0.0 is a valid threshold");
    let rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![Box::new(CapcoRuleSet::new())];
    Engine::new(config, rule_sets, CapcoScheme::new())
        .expect("CAPCO scheme constructs without rewrite cycles")
}

fn diags_summary(diags: &[Diagnostic<CapcoScheme>]) -> Vec<(String, Severity)> {
    diags
        .iter()
        .map(|d| (d.rule.predicate_id().to_owned(), d.severity))
        .collect()
}

/// Locate the first R001 diagnostic carrying a `DecoderPosterior`
/// fix source. Returns `None` if R001 didn't fire or didn't carry the
/// decoder fix-source — either of which is a substantive failure mode
/// for these tests.
fn find_r001(diags: &[Diagnostic<CapcoScheme>]) -> Option<&Diagnostic<CapcoScheme>> {
    diags.iter().find(|d| {
        d.rule.predicate_id() == "R001"
            && d.fix
                .as_ref()
                .is_some_and(|f| matches!(f.source, FixSource::DecoderPosterior))
    })
}

// ============================================================================
// SAR fixtures — strict parser rejects (post-#280); dispatcher falls through
// to decoder; decoder emits R001 with DecoderPosterior at Severity::Suggest.
//
// Pinning today's actual behavior: R001 present, FixSource::DecoderPosterior,
// Severity::Suggest, NO auto-fix. The #280 issue body asserts "the decoder
// path already produces correct canonical uppercase fixes" — partially true
// (R001 fires) and partially false (severity is Suggest, not Fix; no
// `AppliedFix` lands).
// ============================================================================

#[test]
fn sar_lowercase_program_id_emits_r001_fix() {
    // `(TS//SAR-fk)` — lowercase program identifier. Pre-#280 the
    // strict parser silently accepted this as `SarProgram { id: "fk" }`.
    // Post-#280 the strict path rejects the shape; the dispatcher's
    // decoder fallback recognizes the canonical `SAR-FK` form.
    //
    // NOTE: pre-#472 the decoder's posterior on this SAR-shape
    // recovery fell below the default `confidence_threshold = 0.95`,
    // so the engine demoted R001 to `Severity::Suggest`. Post-#472
    // the null hypothesis is now summed over the **observed** bytes
    // (`TS`, `SAR`, `FK`) instead of the canonical token set (just
    // `TS` — SAR program identifiers aren't part of
    // `for_each_canonical_token`'s walk). The observed-bytes null is
    // materially more negative, recognition_runner_up drops, and the
    // resulting recognition_score clears the 0.95 threshold —
    // promoting R001 from `Severity::Suggest` to `Severity::Fix`.
    // This is the audit-worthy severity escalation the pre-#472
    // assertion explicitly tracked; updated to pin post-#472
    // behavior. #280's "auto-fix via R001" claim is now structurally
    // accurate.
    let engine = build_engine();
    let input = b"(TS//SAR-fk)";
    let lint = engine.lint(input);

    let r001 = find_r001(&lint.diagnostics).unwrap_or_else(|| {
        panic!(
            "expected R001 with FixSource::DecoderPosterior on SAR \
             lowercase program id; diagnostics = {:?}",
            diags_summary(&lint.diagnostics),
        );
    });
    assert_eq!(
        r001.severity,
        Severity::Fix,
        "post-#472 the decoder emits R001 for SAR-shape recognition at \
         Severity::Fix. The observed-bytes null hypothesis correctly \
         reports SAR identifiers as low-prose-mass (they are not in \
         the prose priors table), lowering the recognition runner-up \
         and pushing the candidate above the 0.95 threshold.",
    );

    // Severity::Fix → engine.fix DOES auto-apply.
    let fix = engine.fix(input, FixMode::Apply);
    assert_ne!(
        fix.source.expose_secret(),
        input,
        "Fix severity must auto-apply; output bytes must differ from input",
    );
    assert!(
        fix.applied_fixes().next().is_some(),
        "AppliedFix must land for SAR lowercase under Fix severity",
    );
}

#[test]
fn sar_mixed_case_program_id_emits_r001_fix() {
    // `(TS//SAR-Fk)` — title-case program identifier. Same
    // shape-recognition path as the all-lowercase fixture; same
    // post-#472 severity escalation.
    let engine = build_engine();
    let input = b"(TS//SAR-Fk)";
    let lint = engine.lint(input);

    let r001 = find_r001(&lint.diagnostics).unwrap_or_else(|| {
        panic!(
            "expected R001 with FixSource::DecoderPosterior on SAR \
             mixed-case program id; diagnostics = {:?}",
            diags_summary(&lint.diagnostics),
        );
    });
    assert_eq!(r001.severity, Severity::Fix);

    let fix = engine.fix(input, FixMode::Apply);
    assert_ne!(fix.source.expose_secret(), input);
    assert!(fix.applied_fixes().next().is_some());
}

#[test]
fn sar_lowercase_compartment_emits_r001_fix() {
    // `(TS//SAR-FK-blue42)` — uppercase program, lowercase
    // compartment. Tests the second SAR open-vocab tightening site
    // (`SarCompartment::admits_identifier`). Same post-#472
    // escalation.
    let engine = build_engine();
    let input = b"(TS//SAR-FK-blue42)";
    let lint = engine.lint(input);

    let r001 = find_r001(&lint.diagnostics).unwrap_or_else(|| {
        panic!(
            "expected R001 with FixSource::DecoderPosterior on SAR \
             lowercase compartment; diagnostics = {:?}",
            diags_summary(&lint.diagnostics),
        );
    });
    assert_eq!(r001.severity, Severity::Fix);

    let fix = engine.fix(input, FixMode::Apply);
    assert_ne!(fix.source.expose_secret(), input);
    assert!(fix.applied_fixes().next().is_some());
}

#[test]
fn sar_lowercase_sub_compartment_emits_r001_fix() {
    // `(TS//SAR-FK-BLUE 42a)` — uppercase program + compartment,
    // lowercase sub-compartment trailing letter. Tests the
    // SAR sub-compartment open-vocab tightening site. Same post-#472
    // escalation.
    let engine = build_engine();
    let input = b"(TS//SAR-FK-BLUE 42a)";
    let lint = engine.lint(input);

    let r001 = find_r001(&lint.diagnostics).unwrap_or_else(|| {
        panic!(
            "expected R001 with FixSource::DecoderPosterior on SAR \
             lowercase sub-compartment; diagnostics = {:?}",
            diags_summary(&lint.diagnostics),
        );
    });
    assert_eq!(r001.severity, Severity::Fix);

    let fix = engine.fix(input, FixMode::Apply);
    assert_ne!(fix.source.expose_secret(), input);
    assert!(fix.applied_fixes().next().is_some());
}

#[test]
fn sar_lowercase_inputs_canonicalize_to_uppercase_under_zero_threshold() {
    // Companion to the four `*_emits_r001_suggest` tests above that
    // pins the actual canonical bytes the decoder produces for every
    // SAR fixture, not just that R001 fires. With
    // `confidence_threshold = 0.0` the engine's severity-demotion
    // pass leaves R001 at `Severity::Fix`, so the fix auto-applies
    // and the canonical bytes flow out via `engine.fix(...).source`.
    //
    // Spot-check requested by rust-reviewer (#493 PR review) and
    // extended to every SAR shape per Copilot review on PR #500:
    // without canonical-bytes pinning a decoder regression that
    // uppercased noise bytes into a syntactically plausible but
    // semantically wrong canonical form (e.g., `(TS//SAR-FK-blue42)`
    // rendering to `(TS//SAR-FK-BLU 42)` instead of
    // `(TS//SAR-FK-BLUE42)`) would slip past the default-threshold
    // tests above. The default-threshold tests pin the dispatch +
    // severity contract; this loop pins what the decoder actually
    // writes.
    //
    // The `applied.iter().filter(...).count() == 1` form (rather
    // than `.any(...)`) is load-bearing: it catches the case where
    // additional decoder fixes also land — extra fixes would
    // indicate the engine over-applied recovery or that the
    // dispatcher reached the decoder twice on the same input.
    //
    // Audit-content-ignorance (Constitution V Principle V) is
    // preserved: the canonical bytes are read from `fix.source`
    // (the fixed document buffer), not from a diagnostic message
    // field.
    let engine = build_engine_threshold_zero();
    let cases: &[(&[u8], &str)] = &[
        // (input, expected_canonical_output)
        (b"(TS//SAR-fk)", "(TS//SAR-FK)"),
        (b"(TS//SAR-Fk)", "(TS//SAR-FK)"),
        (b"(TS//SAR-FK-blue42)", "(TS//SAR-FK-BLUE42)"),
        (b"(TS//SAR-FK-BLUE 42a)", "(TS//SAR-FK-BLUE 42A)"),
    ];
    for (input, expected) in cases {
        let display = std::str::from_utf8(input).unwrap_or("<bytes>");
        let fix = engine.fix(input, FixMode::Apply);
        // PR 3c.2.D fixup F-3: `applied_fixes()` is `impl Iterator`; collect
        // once for filter + Debug-render in the assertion message.
        let applied: Vec<_> = fix.applied_fixes().collect();
        let r001_decoder_count = applied
            .iter()
            .filter(|a| {
                a.rule.predicate_id() == "R001" && matches!(a.source, FixSource::DecoderPosterior)
            })
            .count();
        assert_eq!(
            r001_decoder_count,
            1,
            "input {display:?} must produce exactly one R001 DecoderPosterior \
             AppliedFix under the zero-threshold engine; applied = {:?}",
            applied
                .iter()
                .map(|a| (a.rule.predicate_id(), a.source))
                .collect::<Vec<_>>(),
        );
        let output = String::from_utf8(fix.source.expose_secret().to_vec()).expect("UTF-8 output");
        assert_eq!(
            output, *expected,
            "decoder must canonicalize {display:?} to {expected:?}; \
             a different canonical form indicates the decoder is \
             writing the wrong canonical bytes",
        );
    }
}

// ============================================================================
// FGI lowercase trigraph — the only #280 case that fully realizes the
// issue-body claim. Strict parser rejects `deu` (post-#280); dispatcher
// falls through; decoder emits R001 at Severity::Fix; auto-applies.
// ============================================================================

#[test]
fn fgi_lowercase_trigraph_decodes_and_fixes_to_canonical() {
    // `(S//FGI deu)` — well-formed portion shape with leading
    // classification and a lowercase ownership trigraph. Pre-#280 the
    // strict parser silently accepted this as
    // `FgiMarker { countries: [] }` — source-concealed FGI per
    // CAPCO-2016 §H.7 p123 (the FGI "Authorized Portion Mark (when
    // source must be concealed and segregated from US): FGI [Non-US
    // Classification Portion Mark]" enumeration). Post-#280 the
    // strict path rejects `deu`; the decoder recovers the canonical
    // `DEU` form and the fix auto-applies.
    let engine = build_engine();
    let input = b"(S//FGI deu)";
    let lint = engine.lint(input);

    let r001 = find_r001(&lint.diagnostics).unwrap_or_else(|| {
        panic!(
            "expected R001 with FixSource::DecoderPosterior on FGI \
             lowercase trigraph; diagnostics = {:?}",
            diags_summary(&lint.diagnostics),
        );
    });
    assert_eq!(
        r001.severity,
        Severity::Fix,
        "FGI trigraph case-fold is the well-trodden decoder path and \
         emits at Severity::Fix (auto-applies)",
    );

    let fix = engine.fix(input, FixMode::Apply);
    assert_eq!(
        String::from_utf8(fix.source.expose_secret().to_vec()).expect("UTF-8 output"),
        "(S//FGI DEU)",
        "decoder must canonicalize lowercase `deu` to uppercase `DEU` \
         and write the fixed output byte-equal to the canonical form",
    );
    // PR 3c.2.D fixup F-3: `applied_fixes()` is `impl Iterator`; collect
    // once for `.len()` + indexed read.
    let applied: Vec<_> = fix.applied_fixes().collect();
    assert_eq!(
        applied.len(),
        1,
        "exactly one AppliedFix should land (the R001 decoder fix)",
    );
    assert_eq!(applied[0].rule.predicate_id(), "R001");
    assert!(matches!(applied[0].source, FixSource::DecoderPosterior));
}

// ============================================================================
// FGI category-mismatch — wrong-shape ownership tokens that the dispatcher
// does NOT route to the decoder. Pinning E008 ("unrecognized token") as the
// observed end-user signal so a future decoder-routing change is a visible,
// considered shift.
// ============================================================================

#[test]
fn fgi_fvey_ownership_token_emits_e008_no_decoder_route() {
    // `(S//FGI FVEY)` — FVEY is a valid REL TO tetragraph (members
    // = USA/GBR/CAN/AUS/NZL) but is semantically wrong as an FGI
    // ownership token. CountryCode::admits_fgi_ownership_token rejects
    // it post-#280.
    //
    // Observed: the dispatcher does NOT route to the decoder. The
    // user-visible diagnostic is E008 at Severity::Error. This is the
    // correct end-user signal ("FVEY is not a valid FGI ownership
    // token"), even though it lands through the rule pipeline rather
    // than through R001. If a future PR routes this through the
    // decoder, the change should produce a different category-specific
    // diagnostic (e.g., "FGI ownership tokens must be sovereign or
    // NATO") rather than a generic R001 canonicalization.
    let engine = build_engine();
    let input = b"(S//FGI FVEY)";
    let lint = engine.lint(input);

    assert!(
        find_r001(&lint.diagnostics).is_none(),
        "no R001 expected — dispatcher does not route FGI category- \
         mismatch tetragraphs to the decoder today; got {:?}",
        diags_summary(&lint.diagnostics),
    );
    assert!(
        lint.diagnostics
            .iter()
            .any(|d| d.rule.predicate_id() == "E008" && d.severity == Severity::Error),
        "expected E008 (unrecognized token) at Error severity; got {:?}",
        diags_summary(&lint.diagnostics),
    );
}

#[test]
fn fgi_deux_unknown_tetragraph_emits_e008_no_decoder_route() {
    // `(S//FGI DEUX)` — 4-byte uppercase token that's not a registered
    // CountryCode tetragraph (`DEUX` is intentionally not FVEY/ACGU/
    // NATO/AUSTRALIA_GROUP/…). Same dispatcher path as FVEY: E008
    // surfaces; the decoder is not invoked.
    let engine = build_engine();
    let input = b"(S//FGI DEUX)";
    let lint = engine.lint(input);

    assert!(
        find_r001(&lint.diagnostics).is_none(),
        "no R001 expected on DEUX (unknown 4-byte token); dispatcher \
         keeps the strict-path result; got {:?}",
        diags_summary(&lint.diagnostics),
    );
    assert!(
        lint.diagnostics
            .iter()
            .any(|d| d.rule.predicate_id() == "E008" && d.severity == Severity::Error),
        "expected E008 (unrecognized token) at Error severity; got {:?}",
        diags_summary(&lint.diagnostics),
    );
}

// ============================================================================
// Negative controls — canonical inputs stay clean of R001.
// ============================================================================

#[test]
fn canonical_sar_portion_emits_no_decoder_diagnostic() {
    // `(TS//SAR-FK)` — already canonical. The strict path resolves
    // unambiguously; the dispatcher does not call the decoder; no
    // R001 should appear.
    let engine = build_engine();
    let lint = engine.lint(b"(TS//SAR-FK)");
    assert!(
        find_r001(&lint.diagnostics).is_none(),
        "canonical SAR portion must not trip the decoder; got {:?}",
        diags_summary(&lint.diagnostics),
    );
}

#[test]
fn canonical_fgi_portion_emits_no_decoder_diagnostic() {
    // `(S//FGI DEU)` — already canonical with a valid sovereign
    // trigraph as the ownership token. The strict path admits DEU
    // and R001 does not fire. (W002, the commingling-warning rule
    // that used to fire on this shape, was retired in the PR
    // closing #470; the canonical US-CLASS + FGI [LIST] shape is
    // §H.7 p123-authorized and no longer triggers any rule.)
    let engine = build_engine();
    let lint = engine.lint(b"(S//FGI DEU)");
    assert!(
        find_r001(&lint.diagnostics).is_none(),
        "canonical FGI portion must not trip the decoder; got {:?}",
        diags_summary(&lint.diagnostics),
    );
}
