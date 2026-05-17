// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Issue #471 — engine-level gate against rule dispatch on sub-threshold
//! decoder-suggested parses.
//! Issue #472 — observed-token null-hypothesis for prose parenthetical
//! acronyms (`(CMS)`, `(CTs)`, …).
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
//! Before #472: the decoder's null hypothesis was summed over the
//! *canonical* tokens (post-fuzzy-correction), so an observed `(CMS)`
//! whose decoder chose canonical `(CTS)` had its prose null computed
//! against `CTS` (rare in prose) — biasing the comparison whenever
//! fuzzy correction shifted between vocabularies of different prose
//! mass. #472 walks the original `bytes` to produce observed tokens
//! and sums prose priors over those, restoring the symmetric
//! marking-vs-prose comparison. The portion-shape null filter was
//! also generalized from the pre-#472 single-letter gate to the
//! `!has_double_slash && !is_bare_classification_shape` gate so
//! multi-letter prose acronyms participate in the filter.
//!
//! Concrete repro:
//! - `(CTs)` (prose parenthetical for "Career Trainees") and `(CMS)`
//!   ("Career Management Staff") both weakly decoded to NATO `CTS`.
//! - The synthetic `MarkingClassification::Nato(_)` then triggered
//!   `E015 non-us-missing-dissem` at `span = 0..0` (the rule's
//!   fallback when no Classification token span exists).
//!
//! This test family pins the post-#471/#472 behavior:
//! 1. Prose parenthetical acronyms emit ZERO diagnostics — the
//!    decoder's observed-bytes null hypothesis suppresses the
//!    candidate at the portion-shape filter, so no R001 is minted
//!    and no downstream rules fire on a synthetic parse.
//! 2. A strict-path parse still drives the full rule pipeline.
//! 3. The CIA-RDP96-00289R000200030004-1 corpus document — the
//!    motivating fixture — emits zero E015 firings end-to-end.
//! 4. Bare-classification portions on the
//!    `is_bare_classification_shape` whitelist (`(C)`, `(S)`, `(TS)`,
//!    …) bypass the null gate but rely on the engine's
//!    no-op-rewrite filter in `build_decoder_diagnostic` to
//!    suppress synthetic R001 when observed bytes equal canonical
//!    bytes. Issue #511 layered-confidence territory; the
//!    end-to-end zero-diagnostic invariant is pinned here so a
//!    no-op-rewrite refactor that changes the contract is caught.

use std::path::{Path, PathBuf};

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::{FixSource, RuleSet};

fn build_engine() -> Engine {
    let rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![Box::new(CapcoRuleSet::new())];
    Engine::new(Config::default(), rule_sets, CapcoScheme::new()).expect("CAPCO engine constructs")
}

/// Workspace-root-anchored corpus path so the fixture loads identically
/// from `cargo test` (workspace cwd) and `cargo test -p marque-engine`
/// (crate cwd). Walking up from `CARGO_MANIFEST_DIR` (`crates/engine`)
/// by two levels lands at the workspace root. Mirrors the
/// `fixtures_root()` helper in `decoder_accuracy.rs`.
fn corpus_documents_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map(|root| {
            root.join("tests")
                .join("corpus")
                .join("documents")
                .join("marked")
        })
        .expect("CARGO_MANIFEST_DIR has a workspace-root grandparent")
}

/// `(CTs)` is "Career Trainees" — a common prose acronym. Post-#472
/// the decoder's observed-bytes null-hypothesis suppresses the
/// candidate entirely; no R001 is minted and no downstream rules
/// (E015 in particular) fire on a synthetic parse.
///
/// Pre-#471: this minted `E015 non-us-missing-dissem` because the
/// rule pipeline saw a synthetic `MarkingClassification::Nato(_)`
/// without a companion dissemination control.
/// Pre-#472 / post-#471: R001 was minted at `Severity::Suggest` (the
/// decoder still produced a synthetic parse but its confidence fell
/// below the threshold).
/// Post-#472: zero diagnostics — the null-hypothesis filter
/// suppresses the candidate before R001 is emitted.
#[test]
fn prose_parenthetical_cts_emits_no_diagnostics() {
    let engine = build_engine();
    let result = engine.lint(b"text (CTs) text");

    let rules: Vec<_> = result
        .diagnostics
        .iter()
        .map(|d| (d.rule.as_str().to_owned(), d.severity))
        .collect();

    assert!(
        result.diagnostics.is_empty(),
        "prose parenthetical `(CTs)` must emit zero diagnostics post-#472 \
         (observed-token null hypothesis suppresses the decoder \
         candidate before R001 is minted). Got: {rules:?}",
    );
}

/// `(CMS)` is "Career Management Staff" — another prose acronym.
/// Same post-#472 expectation as `(CTs)`: observed-bytes null
/// hypothesis suppresses the candidate, zero diagnostics emitted.
#[test]
fn prose_parenthetical_cms_emits_no_diagnostics() {
    let engine = build_engine();
    let result = engine.lint(b"text (CMS) text");
    let rules: Vec<_> = result
        .diagnostics
        .iter()
        .map(|d| (d.rule.as_str().to_owned(), d.severity))
        .collect();
    assert!(
        result.diagnostics.is_empty(),
        "prose parenthetical `(CMS)` must emit zero diagnostics post-#472. \
         Got: {rules:?}",
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
    let path = corpus_documents_root().join("CIA-RDP96-00289R000200030004-1.md");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("fixture {} must be readable: {e}", path.display()));
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

/// `(C)` mid-prose must emit zero diagnostics end-to-end (Copilot #2
/// follow-up; #472 + #511).
///
/// Pins the **two-mechanism interaction** that suppresses `(C)`
/// mid-prose without relying on the corpus fixture
/// `c_mid_prose.txt`:
///
/// 1. **Whitelist bypass**: `(C)` is on
///    [`is_bare_classification_shape`](`marque_engine::decoder` —
///    private) because it is the only grammar form for a
///    CONFIDENTIAL portion. The post-#472 null-hypothesis filter
///    therefore does NOT suppress it at the decoder layer; the
///    candidate reaches `Parsed::Unambiguous(Us(Confidential))`
///    with a recorded `LinePositionPenalty` feature.
/// 2. **No-op-rewrite filter**: at the engine layer,
///    `build_decoder_diagnostic` checks `original == replacement`
///    and returns `None` when the observed bytes equal the
///    canonical bytes. For `(C)` mid-prose the canonical form is
///    `(C)` (already canonical), so the synthetic R001 is never
///    emitted.
///
/// Two existing decoder unit tests
/// (`decoder::tests::decoder_applies_line_position_penalty_for_mid_line_portion`
/// and
/// `decoder::tests::decoder_records_position_penalty_vs_bullet_bonus_for_bare_classification`)
/// pin the recorded `LinePositionPenalty` feature on the surviving
/// candidate — they do NOT assert end-to-end suppression. Copilot
/// #2 flagged the gap: if a future change relaxes the
/// no-op-rewrite filter (audit-verbosity, FR-014 schema evolution,
/// renderer rewrite), or if `(C)` becomes top-posterior via some
/// unrelated calibration change that flips `observed == canonical`,
/// the false-positive would silently come back and only
/// `c_mid_prose.txt` would catch it.
///
/// This test is the engine-level gate that catches that regression
/// independently of the corpus fixture. Hardcoded inputs, no fixture
/// dependency.
///
/// Tracking: #472 (this PR) for the null-gate bypass + whitelist;
/// #511 for the layered-confidence territory that defers a stronger
/// `(C)` mid-prose discriminator.
#[test]
fn bare_class_whitelist_relies_on_no_op_rewrite_filter() {
    let engine = build_engine();
    let result = engine.lint(b"The (C) section of the report describes the protocol.");

    let rules: Vec<_> = result
        .diagnostics
        .iter()
        .map(|d| (d.rule.as_str().to_owned(), d.severity))
        .collect();

    assert!(
        result.diagnostics.is_empty(),
        "`(C)` mid-prose must emit zero diagnostics end-to-end. The \
         decoder produces a candidate (whitelist bypasses the null \
         gate); the engine's no-op-rewrite filter in \
         `build_decoder_diagnostic` is what eats the synthetic R001. \
         If this assertion fails, audit (a) whether `(C)` is still on \
         the bare-classification whitelist, (b) whether the no-op- \
         rewrite filter still short-circuits when observed == \
         canonical, or (c) whether some calibration change pushed a \
         non-canonical alternative to top-posterior. Got: {rules:?}",
    );
}

/// Companion to `bare_class_whitelist_relies_on_no_op_rewrite_filter`:
/// pins that the four other US-axis bare-classification whitelist
/// entries (`(U)`, `(S)`, `(TS)`, `(R)`) share the same end-to-end
/// zero-diagnostic property as `(C)` when used mid-prose. These are
/// the whitelist entries whose canonical form equals their observed
/// bytes, so the no-op-rewrite filter eats the synthetic R001.
///
/// The NATO-axis entries (`(NU)`, `(NR)`, `(NC)`, `(NS)`, `(CTS)`)
/// are NOT exercised here — the decoder canonicalizes those to the
/// `(//NN)` form (adding a leading `//` per CAPCO §A.6 NATO portion
/// grammar), so `observed != canonical`, the no-op-rewrite filter
/// does NOT short-circuit, and R001 is minted at Severity::Suggest
/// (confidence below threshold). That behavior is correct and
/// orthogonal to this test — the NATO whitelist entries exist so
/// the null gate doesn't suppress them at the decoder layer, not
/// because they round-trip to themselves end-to-end.
#[test]
fn us_axis_bare_class_whitelist_end_to_end_zero_diagnostics() {
    let engine = build_engine();
    // US-axis whitelist entries from `is_bare_classification_shape`
    // in `decoder.rs`. Each canonicalizes to its observed form, so
    // `build_decoder_diagnostic` returns None via the no-op-rewrite
    // filter and zero diagnostics emit end-to-end.
    let cases: &[&[u8]] = &[
        b"prefix (U) suffix",
        b"prefix (C) suffix",
        b"prefix (S) suffix",
        b"prefix (TS) suffix",
        b"prefix (R) suffix",
    ];
    for input in cases {
        let result = engine.lint(input);
        assert!(
            result.diagnostics.is_empty(),
            "US-axis whitelist entry in {:?} must emit zero diagnostics; \
             got {:?}",
            std::str::from_utf8(input).unwrap_or("<bytes>"),
            result
                .diagnostics
                .iter()
                .map(|d| (d.rule.as_str().to_owned(), d.severity))
                .collect::<Vec<_>>(),
        );
    }
}
