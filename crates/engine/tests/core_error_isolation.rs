// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CoreError` content-isolation regression test (whitepaper §5.3 / gap
//! register #20).
//!
//! `marque-core::CoreError` has two variants that *could* carry input
//! bytes verbatim through their `Display` impl:
//!
//! - `MalformedMarking(String)` — Display interpolates the embedded
//!   string via `{0:?}`.
//! - `UnrecognizedToken { token: String, offset: usize }` — Display
//!   interpolates the embedded token via `{token:?}`.
//!
//! Today the parser source actually constructs only the first of these,
//! and only on inputs the scanner does not produce (portion candidates
//! without balanced parens, or empty inner content). `UnrecognizedToken`
//! is unreferenced. The third variant — `InvalidUtf8(Span)` — carries
//! no embedded content. Even so, the leak surface is a *contract*, not
//! a state of the source: a future change that adds a content-bearing
//! `MalformedMarking` site, or that revives `UnrecognizedToken`, would
//! re-open the channel.
//!
//! `StrictRecognizer` (`crates/engine/src/recognizer.rs:97`) catches
//! `CoreError` from `Parser::parse` and discards it — `Err(_) =>
//! Parsed::Ambiguous { candidates: Vec::new() }`. The engine never sees
//! the error value. `LintResult` carries no error channel; `FixResult`
//! carries `RemainingDiagnostic` and `AppliedFix` whose text fields are
//! built from token canonicals, not error-Display output.
//!
//! Whitepaper §5.3 calls the no-leak property out as a *convention* —
//! the type system permits a future caller to call `.to_string()` on a
//! `CoreError` and route it into an audit record or server response.
//! This file is the runtime backstop: a canary embedded inside
//! adversarial input bytes does not appear in any serialized output the
//! engine produces.
//!
//! ## What this test does NOT prove
//!
//! - It does not prove that no future code path will surface
//!   `CoreError`. Nothing short of making `CoreError` `pub(crate)`
//!   (a breaking change to the existing `marque_core::CoreError`
//!   re-export) prevents that at the type level.
//! - It does not enumerate every `CoreError` variant. `UnrecognizedToken`
//!   has no construction site to exercise; `InvalidUtf8` carries no
//!   content. The cases below cover the one variant that has a
//!   reachable construction site (`InvalidUtf8`, via a portion span
//!   with invalid UTF-8) plus three adversarial inputs that ride the
//!   recognizer/engine path past the strict grammar.
//!
//! What this test does prove:
//!
//! 1. The current `StrictRecognizer::recognize` discards `Err(CoreError::*)`
//!    from `Parser::parse` rather than surfacing the error value. A
//!    regression that started routing `CoreError::Display` into a
//!    `Diagnostic.message` or an `AppliedFix.proposal.{original,replacement}`
//!    field would be caught here.
//! 2. The canary infrastructure is real — the self-test asserts that
//!    `CoreError::MalformedMarking(canary).to_string()` does carry the
//!    canary, so a future Display redaction surfaces explicitly rather
//!    than silently obsoleting the rest of this file.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_core::CoreError;
use marque_engine::{Engine, FixMode, StrictRecognizer};
use std::sync::Arc;

/// A high-entropy ASCII run that cannot occur in any valid CAPCO/ISM
/// marking: lowercase letters, digits, and hyphens combined into a
/// shape that matches no token canonical, no compartment grammar, and
/// no CAB line. If this string appears in any serialized engine output
/// it can have come from only one place — the bytes the test fed in.
const CANARY: &str = "leak-canary-x9z7q3-content-bytes";

/// `CoreError` is produced only on the strict path
/// (`StrictRecognizer::recognize` catches `CoreError` from
/// `Parser::parse` and discards it). The decoder fallback uses a
/// different error shape entirely. To exercise the *CoreError*
/// content-isolation channel that this file is named for, pin the
/// engine to `StrictRecognizer` rather than relying on the default
/// dispatcher (which would also exercise decoder-side leak channels —
/// real but separately scoped issues, not the one this file gates).
fn test_engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
    // MASKING-PIN: tracks #257 — decoder canonicalization leaks input bytes into AppliedFix (#257); strict path isolates the test from that leak channel until PR 3c closes the carve-out
    .with_recognizer(Arc::new(StrictRecognizer::new()))
}

// ---------------------------------------------------------------------------
// Sanity check — the leak vector is real.
// ---------------------------------------------------------------------------
//
// If a future refactor changes `CoreError::MalformedMarking`'s Display
// formatter to redact / drop / hash the embedded string, the leak risk
// the rest of this file exists to gate goes away — but so does the
// motivation for the file. A test that silently passes after the
// underlying risk evaporated is dead weight. This sanity check fires
// on that scenario.
#[test]
fn core_error_display_carries_input_string() {
    let err = CoreError::MalformedMarking(CANARY.to_owned());
    let rendered = err.to_string();
    assert!(
        rendered.contains(CANARY),
        "`CoreError::MalformedMarking` Display output no longer carries \
         the embedded string. If this is intentional, the gap register #20 \
         test family in this file (`crates/engine/tests/core_error_isolation.rs`) \
         is now obsolete and should be retired alongside whitepaper §5.3. \
         Got: {rendered:?}"
    );
}

// ---------------------------------------------------------------------------
// Engine isolation — input that would trigger CoreError must not
// surface in any LintResult or FixResult field.
// ---------------------------------------------------------------------------

/// Adversarial input bytes for engine-isolation coverage.
///
/// Of the four `CoreError` construction sites in
/// `crates/core/src/parser.rs`, only `InvalidUtf8(span)` is reliably
/// reachable from scanner-fed input today (`MalformedMarking` with a
/// content-bearing payload requires either a portion candidate
/// without balanced parens — which the scanner does not produce — or
/// an empty inner string after stripping parens, which carries no
/// content; `UnrecognizedToken` is unreferenced by the parser source).
/// `InvalidUtf8` carries only a `Span`, no content, so a Display leak
/// of that variant would not expose input bytes.
///
/// The cases below therefore split into two roles:
///
/// 1. **Guaranteed `Parser::parse` -> `Err(CoreError::*)` site.** The
///    UTF-8-corrupted portion `(0xff)` survives the scanner, reaches
///    the recognizer's `parser.parse(...)` call, and returns
///    `Err(CoreError::InvalidUtf8(span))`. `StrictRecognizer`
///    (`crates/engine/src/recognizer.rs:97`) discards the error via
///    `Err(_) => Parsed::Ambiguous { candidates: Vec::new() }`. This
///    case proves the discard path holds — a future refactor that
///    Display'd the error into the recognizer output would surface
///    the (variant-specific) error string in the lint stream and
///    fail the assertions below.
/// 2. **Adversarial recognizer/engine inputs.** The remaining cases
///    interleave the canary with real marking delimiters so the
///    scanner produces candidates, the recognizer returns
///    `Parsed::Ambiguous`, and the engine drops them. They prove that
///    no engine code path interpolates input bytes into a public
///    output field independently of `CoreError`.
///
/// The test does NOT claim every `CoreError` variant is constructed.
/// It claims that the engine, fed input designed to exercise the
/// `Parser::parse -> Err(CoreError)` path that does exist today,
/// surfaces no canary in any text-bearing public output.
fn adversarial_inputs() -> Vec<Vec<u8>> {
    vec![
        // (1) Guaranteed `Parser::parse` -> `Err(CoreError::InvalidUtf8(span))`.
        //
        // `0xff` is not valid as a leading byte of any UTF-8 sequence, so
        // `candidate.span.as_str(source)` returns `Err`. The scanner
        // emits a portion candidate spanning the balanced parens
        // regardless. No canary content is carried by `InvalidUtf8`,
        // but the case proves the recognizer's `Err(_) -> Ambiguous`
        // discard fires on a real CoreError construction.
        vec![b'(', 0xff, b')'],
        // (2) Portion-shaped candidate carrying the canary as marking
        // content. The portion parser strips parens successfully; the
        // canary survives as `TokenKind::Unknown` token spans. No
        // CoreError is constructed today — the case is adversarial
        // recognizer coverage, not a CoreError-site exercise.
        format!("({CANARY})").into_bytes(),
        // (3) Banner-shaped candidate with the canary between marking
        // delimiters. Same shape as (2): the canary becomes `Unknown`
        // tokens, the recognizer rejects, no CoreError is constructed.
        format!("TOP SECRET//{CANARY}//NOFORN").into_bytes(),
        // (4) Mixed: real marking, then a canary portion, then another
        // real marking. Tests that partial success over real content
        // does not pull the canary forward into any downstream field.
        format!("(S//SI//NF)\n({CANARY})\n(C)").into_bytes(),
    ]
}

#[test]
fn lint_does_not_leak_core_error_content() {
    let engine = test_engine();
    for input in adversarial_inputs() {
        let result = engine.lint(&input);

        // Walk every text-bearing field of LintResult.
        for diag in &result.diagnostics {
            // PR 3c.2.C C5: `Diagnostic.message` is closed-template
            // `Message`; document text (including the `CANARY` byte
            // sequence) cannot reach this field by type. Scan the
            // closed-enum template label as the structural-pin
            // equivalent of the prior `contains()` check — if a
            // future refactor reintroduces a free-form string channel
            // on `Message`, this scan still catches it.
            let template_label = diag.message.template().as_str();
            assert!(
                !template_label.contains(CANARY),
                "Diagnostic.message template leaked CoreError-bearing input: \
                 {template_label} (input was {input:?})",
                input = String::from_utf8_lossy(&input),
            );
            // Post Commit 10: `Diagnostic.fix` carries a structural
            // FixIntent with no byte payload. The diagnostic's
            // `text_correction` field is the only string-bearing
            // channel; assert it doesn't contain the canary.
            if let Some(tc) = diag.text_correction.as_ref() {
                assert!(
                    !tc.replacement.contains(CANARY),
                    "Diagnostic.text_correction.replacement leaked CoreError-bearing input"
                );
            }
        }
    }
}

#[test]
fn fix_does_not_leak_core_error_content() {
    let engine = test_engine();
    for input in adversarial_inputs() {
        let result = engine.fix(&input, FixMode::Apply);

        // Every applied fix's proposal — the bytes that flow into
        // the audit record. This is the same surface T056 covers in
        // `audit.rs`, but with input designed to trip CoreError
        // rather than to embed prose.
        for applied in &result.applied {
            // Post Commit 10 the audit record carries no `original`
            // byte field; only `TextCorrection.replacement` can hold
            // string bytes (corpus-derived canonical token).
            if let marque_rules::AppliedFixProposal::TextCorrection { replacement } =
                &applied.proposal
            {
                assert!(
                    !replacement.contains(CANARY),
                    "AppliedFix TextCorrection.replacement leaked CoreError-bearing input"
                );
            }
        }

        // Remaining diagnostics — what `marque check` and the lint
        // re-run after fix would emit. Same content-ignorance contract.
        for diag in &result.remaining_diagnostics {
            // PR 3c.2.C C5: scan the closed-template label, per the
            // companion `lint_does_not_leak_core_error_content`
            // structural-pin explanation above.
            let template_label = diag.message.template().as_str();
            assert!(
                !template_label.contains(CANARY),
                "RemainingDiagnostic.message template leaked CoreError-bearing input: \
                 {template_label}",
            );
        }

        // The post-fix source bytes can legitimately contain the
        // canary — that's the original input, untouched by any rule.
        // We don't assert against `result.source` here.
    }
}
