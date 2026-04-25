// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `CoreError` content-isolation regression test (whitepaper §5.3 / gap
//! register #20).
//!
//! `marque-core::CoreError` has variants that embed input bytes verbatim:
//!
//! - `MalformedMarking(String)` — Display output carries the malformed
//!   marking text via `{0:?}`.
//! - `UnrecognizedToken { token: String, offset: usize }` — Display
//!   output carries the unrecognized token via `{token:?}`.
//!
//! By design these are internal — the engine catches `CoreError` inside
//! `Parser::parse` (and the recognizer wrapper) and never propagates the
//! error value to a public return type. `LintResult` carries no error
//! channel at all; `FixResult` carries `RemainingDiagnostic` and
//! `AppliedFix` whose text fields are token-canonical, not error-Display
//! output.
//!
//! Whitepaper §5.3 calls the no-leak property out as a *convention* —
//! the type system permits a future caller to surface `CoreError` via
//! `.to_string()` and route it into an audit record or server response.
//! This file converts the convention into a runtime-asserted invariant:
//! a canary embedded inside adversarial input bytes does not appear in
//! any serialized output the engine produces.
//!
//! ## What this test does NOT prove
//!
//! It does not prove that no future code path will surface `CoreError`.
//! Nothing short of making `CoreError` `pub(crate)` (which is a
//! breaking change to the existing `marque_core::CoreError` re-export)
//! prevents that at the type level. What this test does prove:
//!
//! 1. The current engine swallows `CoreError`-causing input rather than
//!    surfacing it. A regression that started routing `CoreError::Display`
//!    into a `Diagnostic.message` or `AppliedFix.proposal.original` field
//!    would be caught here.
//! 2. The canary infrastructure works — the self-test asserts that
//!    `CoreError::MalformedMarking(canary).to_string()` actually contains
//!    the canary, so a future change that nulled out the leak vector
//!    accidentally would be visible.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_core::CoreError;
use marque_engine::{Engine, FixMode};

/// A high-entropy ASCII run that cannot occur in any valid CAPCO/ISM
/// marking: lowercase letters, digits, and hyphens combined into a
/// shape that matches no token canonical, no compartment grammar, and
/// no CAB line. If this string appears in any serialized engine output
/// it can have come from only one place — the bytes the test fed in.
const CANARY: &str = "leak-canary-x9z7q3-content-bytes";

fn test_engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
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

/// Input bytes designed to trip every `CoreError` construction site
/// in `crates/core/src/parser.rs`:
///
/// - A bare-portion candidate that survives the scanner but fails the
///   `parse_marking_string` token-extract → `MalformedMarking`.
/// - A banner-shaped candidate that is well-formed at the scanner
///   level but contains the canary as a free-form token →
///   recognizer rejects, scanner skips.
///
/// Each variant interleaves the canary with real marking delimiters
/// so that the scanner finds candidates and routes them to the
/// recognizer. The recognizer returns `Parsed::Ambiguous` on every
/// candidate, the engine drops them, and the canary stays bottled
/// inside `marque-core` where it belongs.
fn adversarial_inputs() -> Vec<Vec<u8>> {
    vec![
        // Portion-shaped candidate carrying the canary as the
        // marking content. The portion parser will try to slice off
        // the parens, then `parse_marking_string` will fail to
        // recognize the canary as any token and return
        // `MalformedMarking(text)`.
        format!("({CANARY})").into_bytes(),
        // Banner-shaped candidate carrying the canary between
        // delimiters. The banner parser sees the canary as
        // unrecognized tokens; if any path Display'd the
        // `UnrecognizedToken` variant the canary would leak.
        format!("TOP SECRET//{CANARY}//NOFORN").into_bytes(),
        // Mixed: real marking followed by a malformed canary
        // portion. Tests that a partial parse over real content
        // does not pull the canary into a downstream field.
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
            assert!(
                !diag.message.contains(CANARY),
                "Diagnostic.message leaked CoreError-bearing input: \
                 {msg:?} (input was {input:?})",
                msg = diag.message,
                input = String::from_utf8_lossy(&input),
            );
            if let Some(fix) = diag.fix.as_ref() {
                assert!(
                    !fix.original.contains(CANARY),
                    "FixProposal.original leaked CoreError-bearing input"
                );
                assert!(
                    !fix.replacement.contains(CANARY),
                    "FixProposal.replacement leaked CoreError-bearing input"
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
            assert!(
                !applied.proposal.original.contains(CANARY),
                "AppliedFix.proposal.original leaked CoreError-bearing input"
            );
            assert!(
                !applied.proposal.replacement.contains(CANARY),
                "AppliedFix.proposal.replacement leaked CoreError-bearing input"
            );
        }

        // Remaining diagnostics — what `marque check` and the lint
        // re-run after fix would emit. Same content-ignorance contract.
        for diag in &result.remaining_diagnostics {
            assert!(
                !diag.message.contains(CANARY),
                "RemainingDiagnostic.message leaked CoreError-bearing input: \
                 {msg:?}",
                msg = diag.message,
            );
        }

        // The post-fix source bytes can legitimately contain the
        // canary — that's the original input, untouched by any rule.
        // We don't assert against `result.source` here.
    }
}
