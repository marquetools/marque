// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Issue #471 — engine-level gate against rule dispatch on sub-threshold
//! decoder-suggested parses.
//!
//! The decoder's `StrictOrDecoderRecognizer` can return
//! `Parsed::Unambiguous(marking)` for an input whose decoder posterior
//! is below the configured `confidence_threshold`. The R001 diagnostic
//! is the user-facing surface for that recognition; the post-emission
//! severity pass demotes it from `Severity::Fix` to `Severity::Suggest`
//! when `combined() < threshold`, so the recognition is a *suggestion*
//! the user has not accepted.
//!
//! Before #471: the engine continued to run downstream rules against
//! `marking.0` (the synthetic attrs the decoder produced) and folded
//! them into `PageContext`. That trusted the unaccepted parse as if it
//! were strict-path canonical, minting false positives keyed on a
//! canonicalization no human had endorsed.
//!
//! Concrete repro:
//! - `(CTs)` (prose parenthetical for "Career Trainees") and `(CMS)`
//!   ("Career Management Staff") both weakly decode to NATO `CTS`
//!   (recognition ≈ 0.86, below the default 0.95 threshold).
//! - The synthetic `MarkingClassification::Nato(_)` then triggered
//!   `E015 non-us-missing-dissem` at `span = 0..0` (the rule's
//!   fallback when no Classification token span exists).
//!
//! This test family pins the post-fix behavior:
//! 1. Sub-threshold decoder recognitions emit R001 (Suggest) only;
//!    no downstream rules fire on the synthetic parse.
//! 2. A strict-path parse still drives the full rule pipeline.
//! 3. The CIA-RDP96-00289R000200030004-1 corpus document — the
//!    motivating fixture — emits zero E015 firings end-to-end.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::{FixSource, RuleSet, Severity};

fn build_engine() -> Engine {
    let rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![Box::new(CapcoRuleSet::new())];
    Engine::new(Config::default(), rule_sets, CapcoScheme::new()).expect("CAPCO engine constructs")
}

/// `(CTs)` is "Career Trainees" — a common prose acronym. The decoder
/// can weakly recognize the parenthesized form as a candidate
/// classification portion `(CTS)` (NATO Cosmic Top Secret) via a
/// case-fold + reorder feature path; the posterior lands around 0.86,
/// well below the default 0.95 threshold.
///
/// Pre-#471: this minted `E015 non-us-missing-dissem` because the
/// rule pipeline saw a synthetic `MarkingClassification::Nato(_)`
/// without a companion dissemination control.
/// Post-#471: only R001 fires, at `Severity::Suggest`.
#[test]
fn prose_parenthetical_cts_emits_only_r001_suggest() {
    let engine = build_engine();
    let result = engine.lint(b"text (CTs) text");

    let rules: Vec<_> = result
        .diagnostics
        .iter()
        .map(|d| (d.rule.as_str().to_owned(), d.severity))
        .collect();

    assert!(
        result.diagnostics.iter().all(|d| d.rule.as_str() != "E015"),
        "E015 must not fire on prose parenthetical `(CTs)` — the \
         decoder's recognition score is below threshold and rules \
         should not run against the unaccepted synthetic parse. \
         Got: {rules:?}",
    );

    let r001s: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "R001")
        .collect();
    assert_eq!(
        r001s.len(),
        1,
        "expected exactly one R001 (the decoder suggestion); got {rules:?}",
    );
    assert_eq!(
        r001s[0].severity,
        Severity::Suggest,
        "sub-threshold decoder R001 must be Suggest (combined() < threshold)",
    );
    assert!(
        r001s[0]
            .fix
            .as_ref()
            .is_some_and(|f| matches!(f.source, FixSource::DecoderPosterior)),
        "R001 must carry FixSource::DecoderPosterior",
    );
}

/// `(CMS)` is "Career Management Staff" — another prose acronym. The
/// decoder weakly recognizes it as `CTS` via a one-character edit
/// distance feature path; posterior ≈ 0.84, also below threshold.
#[test]
fn prose_parenthetical_cms_emits_only_r001_suggest() {
    let engine = build_engine();
    let result = engine.lint(b"text (CMS) text");
    let rules: Vec<_> = result
        .diagnostics
        .iter()
        .map(|d| (d.rule.as_str().to_owned(), d.severity))
        .collect();
    assert!(
        result.diagnostics.iter().all(|d| d.rule.as_str() != "E015"),
        "E015 must not fire on prose parenthetical `(CMS)`; got {rules:?}",
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.rule.as_str() == "R001" && d.severity == Severity::Suggest),
        "expected R001 Suggest; got {rules:?}",
    );
}

/// Strict-path parses (no decoder fallback) must continue to drive the
/// full rule pipeline. Negative control: `SECRET//RD` triggers
/// `E021 aea-requires-noforn` (CAPCO-2016 §H.6 p104 — RD always
/// requires NOFORN absent a sharing agreement). E021 carries
/// `FixSource::BuiltinRule`, distinct from R001's
/// `FixSource::DecoderPosterior`, so a wrong inversion of the gate's
/// `marking.1.is_none()` predicate (which would skip the strict path)
/// would silently drop E021 here.
#[test]
fn strict_path_aea_rd_drives_e021() {
    let engine = build_engine();
    let result = engine.lint(b"SECRET//RD");
    let rules: Vec<_> = result
        .diagnostics
        .iter()
        .map(|d| (d.rule.as_str().to_owned(), d.severity))
        .collect();
    let e021 = result
        .diagnostics
        .iter()
        .find(|d| d.rule.as_str() == "E021");
    let e021 = e021.unwrap_or_else(|| {
        panic!(
            "E021 (RD requires NOFORN) must fire on strict-path \
             `SECRET//RD` — gating sub-threshold decoder parses \
             must not regress the strict-path rule dispatch. \
             Got {rules:?}",
        );
    });
    assert!(
        e021.fix
            .as_ref()
            .is_some_and(|f| matches!(f.source, FixSource::BuiltinRule)),
        "E021 must carry FixSource::BuiltinRule on strict path, not \
         DecoderPosterior — confirms the rule pipeline ran against \
         strict-path attrs, not a decoder-synthesized parse",
    );
}

/// End-to-end regression: the motivating fixture for #471 emits zero
/// E015 firings post-fix. Prevents silent regression at the corpus
/// boundary.
#[test]
fn cia_rdp96_fixture_emits_no_e015() {
    let bytes =
        std::fs::read("../../tests/corpus/documents/marked/CIA-RDP96-00289R000200030004-1.md")
            .expect("fixture exists");
    let engine = build_engine();
    let result = engine.lint(&bytes);
    let e015: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "E015")
        .collect();
    assert_eq!(
        e015.len(),
        0,
        "E015 must not fire on CIA-RDP96-00289R000200030004-1 — \
         document is US TOP SECRET, no non-US classification anywhere; \
         #471 traced this to decoder weakly recognizing prose \
         parentheticals (CTs)/(CMS) as NATO CTS. Got {} E015 firings.",
        e015.len(),
    );
}
