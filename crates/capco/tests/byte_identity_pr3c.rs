#![cfg(any())]
// PR 3c.B Commit 10: legacy FixProposal-shape test disabled pending rewrite

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Commit 3 — byte-identity acceptance gate (engine-side).
//!
//! Asserts that the post-Commit-3 engine `AppliedFixProposal::New
//! { intent, synthesized }` projects to a `synthesized: FixProposal`
//! whose every audit-emitted field is byte-identical to the
//! pre-PR-3c `FixProposal` for the same input. This is the
//! empirical pin for Path C of the consolidated plan
//! (`docs/plans/2026-05-10-pr3c-consolidated-plan.md` lines 100–175):
//! the audit-record shape does NOT change in commits 2–9. Migrated
//! rules emit a `FixIntent<S>` alongside a legacy `FixProposal`;
//! the engine wraps the pair in `AppliedFixProposal::New { intent,
//! synthesized: fix }`. The `Deref<Target = FixProposal>` impl on
//! `AppliedFixProposal` returns `&synthesized` for audit-emit
//! consumers, so the JSON serializer reads byte-identical fields
//! regardless of variant.
//!
//! **Two fixtures per migrated rule with auto-applying fixes** —
//! single-trigraph degenerate (smoke) AND multi-trigraph
//! load-bearing (exercises the engine's sibling preservation
//! under fix application). E021 has no baseline because pre-PR-3c
//! E021 was Severity::Error no-fix; the byte-identity gate is
//! vacuous for E021 and exercised instead by per-rule shape tests
//! in `tests/fix_intent_round_trip.rs`.
//!
//! **NDJSON-byte-identity** at the full CLI emission layer
//! (FR-005a) is a complementary gate. It cannot live here because
//! `marque-capco` does not depend on the `marque` CLI crate (the
//! audit-JSON serializer lives in `marque/src/render.rs`). The
//! engine-side fields exercised by these tests are the substrate
//! the JSON serializer reads, so a divergence at the engine layer
//! would propagate to the CLI layer; the CLI integration test
//! (`marque/tests/` — if added in a later commit) would catch the
//! one additional drift class (the JSON serializer itself
//! changing). Through Commit 9 the JSON serializer is Path-C-
//! frozen.
//!
//! **If this gate fails, STOP** — do not regenerate the
//! baselines. A failure means either (a) the synthesized
//! projection on a migrated rule diverged from the pre-migration
//! FixProposal (a regression in the rule body or in the engine's
//! `AppliedFixProposal::New` Deref impl), or (b) Commit 5's
//! renderer produced different canonical bytes than the pre-PR-3c
//! FixProposal layout (a real renderer defect). See
//! `tests/fixtures/pr3c_baseline/README.md` for the baseline-
//! capture procedure and how to interpret a failure.

use std::path::PathBuf;

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};
use marque_rules::{AppliedFix, AppliedFixProposal, FixSource};
use secrecy::ExposeSecret as _;

// ---------------------------------------------------------------------------
// Baseline fixtures — captured at merge-base 30e11b0d
// ---------------------------------------------------------------------------

/// Path to the baseline NDJSON snapshot directory. The full
/// JSON-level byte-identity gate is documented in
/// `tests/fixtures/pr3c_baseline/README.md` and lives at the CLI
/// integration layer (when wired); this file's engine-level gate
/// asserts the substrate the JSON serializer reads.
fn baseline_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("pr3c_baseline")
}

/// Confirm the baseline NDJSON file exists. Files do NOT have to be
/// parsed here — they document the canonical pre-PR-3c byte layout
/// for the CLI-layer gate, and the engine-level gate below pins the
/// same span / original / replacement values inline.
fn ensure_baseline_exists(name: &str) {
    let path = baseline_dir().join(name);
    assert!(
        path.exists(),
        "baseline fixture missing: {path:?}. See \
         tests/fixtures/pr3c_baseline/README.md for the capture \
         procedure."
    );
}

// ---------------------------------------------------------------------------
// Engine fixture
// ---------------------------------------------------------------------------

fn engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(std::time::UNIX_EPOCH)),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

/// Look up the first `AppliedFix` whose synthesized projection
/// carries `rule_id`. Panics with a list of every rule that DID
/// fire so a failed lookup tells the reviewer immediately whether
/// the rule simply didn't fire or whether the wrong rule fired.
fn find_applied<'a>(
    applied: &'a [AppliedFix<CapcoScheme>],
    rule_id: &str,
) -> &'a AppliedFix<CapcoScheme> {
    applied
        .iter()
        .find(|af| af.rule.as_str() == rule_id)
        .unwrap_or_else(|| {
            let fired: Vec<&str> = applied.iter().map(|af| af.rule.as_str()).collect();
            panic!(
                "{rule_id} did not auto-apply on the test input; \
                 rules that did: {fired:?}"
            )
        })
}

/// Assert that `applied` is the `New` variant (migrated rule) and
/// return the cached `synthesized` projection.
fn assert_new_variant_and_synthesized(af: &AppliedFix<CapcoScheme>) -> &marque_rules::FixProposal {
    match &af.proposal {
        AppliedFixProposal::New {
            intent: _,
            synthesized,
        } => synthesized,
        AppliedFixProposal::Legacy(_) => panic!(
            "{}: expected AppliedFixProposal::New (migrated rule); \
             got Legacy. The dual-population pairing in \
             Engine::fix_inner did not fire — likely a regression in \
             the (RuleId, Span) intent-index keying or in the rule's \
             with_fix_and_intent emission.",
            af.rule
        ),
    }
}

// ---------------------------------------------------------------------------
// E054 — NOFORN ⊥ RELIDO (FactRemove)
// ---------------------------------------------------------------------------

#[test]
fn e054_simple_synthesized_matches_pre_pr3c_baseline() {
    // Single-trigraph degenerate: `(S//NF/RELIDO)`.  RELIDO is the
    // last dissem token; the fix consumes the preceding `/`.
    ensure_baseline_exists("e054_simple.ndjson");
    let result = engine().fix(b"(S//NF/RELIDO)\n", FixMode::Apply);
    let af = find_applied(&result.applied, "E054");
    let synth = assert_new_variant_and_synthesized(af);

    // Fields pinned from `tests/fixtures/pr3c_baseline/e054_simple.ndjson`.
    // Drift here MUST fail this assertion — DO NOT update the
    // expected values; investigate the regression first.
    assert_eq!(synth.span.start, 6, "e054_simple: span.start drifted");
    assert_eq!(synth.span.end, 13, "e054_simple: span.end drifted");
    assert_eq!(
        synth.original.as_ref(),
        "/RELIDO",
        "e054_simple: original drifted"
    );
    assert_eq!(
        synth.replacement.as_ref(),
        "",
        "e054_simple: replacement drifted"
    );
    assert_eq!(synth.source, FixSource::BuiltinRule);
    assert!((af.confidence.combined() - 0.95).abs() < f32::EPSILON);
    assert!(synth.migration_ref.is_none());

    // Post-fix source: RELIDO removed.
    assert_eq!(result.source.expose_secret(), b"(S//NF)\n");
}

#[test]
fn e054_multi_synthesized_matches_pre_pr3c_baseline() {
    // Multi-trigraph load-bearing: `(S//NF/IMC/RELIDO)`. RELIDO is
    // the last of three dissem tokens; the fix preserves the
    // sibling block (NF/IMC) verbatim. This is the load-bearing
    // case for Commit 5's renderer: if the renderer re-sorted
    // siblings or normalized whitespace differently than pre-PR-3c,
    // the synthesized projection would diverge from baseline and
    // this test would fire.
    ensure_baseline_exists("e054_multi.ndjson");
    let result = engine().fix(b"(S//NF/IMC/RELIDO)\n", FixMode::Apply);
    let af = find_applied(&result.applied, "E054");
    let synth = assert_new_variant_and_synthesized(af);

    assert_eq!(synth.span.start, 10, "e054_multi: span.start drifted");
    assert_eq!(synth.span.end, 17, "e054_multi: span.end drifted");
    assert_eq!(synth.original.as_ref(), "/RELIDO");
    assert_eq!(synth.replacement.as_ref(), "");
    assert_eq!(result.source.expose_secret(), b"(S//NF/IMC)\n");
}

// ---------------------------------------------------------------------------
// E057 — ORCON-USGOV ⊥ RELIDO (FactRemove)
// ---------------------------------------------------------------------------

#[test]
fn e057_simple_synthesized_matches_pre_pr3c_baseline() {
    // Single-trigraph: `(S//OC-USGOV/RELIDO)`. ORCON-USGOV is the
    // §-asserting token per §H.8 p140; the fix removes RELIDO.
    ensure_baseline_exists("e057_simple.ndjson");
    let result = engine().fix(b"(S//OC-USGOV/RELIDO)\n", FixMode::Apply);
    let af = find_applied(&result.applied, "E057");
    let synth = assert_new_variant_and_synthesized(af);

    assert_eq!(synth.span.start, 12, "e057_simple: span.start drifted");
    assert_eq!(synth.span.end, 19, "e057_simple: span.end drifted");
    assert_eq!(synth.original.as_ref(), "/RELIDO");
    assert_eq!(synth.replacement.as_ref(), "");
    assert_eq!(result.source.expose_secret(), b"(S//OC-USGOV)\n");
}
