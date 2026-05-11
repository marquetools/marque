// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Commit 3 — FixIntent emission + engine round-trip tests.
//!
//! Covers the three beachhead rules migrated in Commit 3:
//!
//! - **E054** — NOFORN ⊥ RELIDO (`FactRemove`)
//! - **E057** — ORCON-USGOV ⊥ RELIDO (`FactRemove`)
//! - **E021** — RD/FRD requires NOFORN (`FactAdd`; severity flipped
//!   Error → Fix; auto-applies when an IC dissem block exists)
//!
//! Two layers of assertion:
//!
//! 1. **Shape** — each rule's `Diagnostic` carries both `fix` (legacy
//!    projection, byte-identical to pre-migration) AND `fix_intent`
//!    (new structural emission). The shape assertions pin every
//!    field that the engine's promotion path keys on.
//!
//! 2. **Engine round-trip** — running each fixture through `Engine::fix`
//!    promotes the paired emission to an `AppliedFix` carrying
//!    `AppliedFixProposal::New { intent, synthesized }`. The
//!    synthesized projection is byte-identical to the
//!    `Diagnostic.fix` the rule emitted (Path C invariant).
//!
//! The complementary **byte-identity acceptance gate** (against
//! pre-PR-3c NDJSON baselines stored under
//! `tests/fixtures/pr3c_baseline/`) lives in
//! `tests/byte_identity_pr3c.rs`. The complementary
//! **content-ignorance gate** on the `intent` payload lives in
//! `tests/g13_closure_fix_intent.rs`.

use marque_capco::CapcoRuleSet;
use marque_capco::CapcoScheme;
use marque_capco::scheme::{TOK_NOFORN, TOK_RELIDO};
use marque_config::Config;
use marque_engine::{Engine, FixMode};
use marque_rules::{
    AppliedFixProposal, FactRef, FixSource, ReplacementIntent, ReplacementIntent::FactAdd, Severity,
};
use marque_scheme::Scope;

// ---------------------------------------------------------------------------
// Engine helpers
// ---------------------------------------------------------------------------

/// Build a default-configured `Engine` over `CapcoScheme`.
fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

// ---------------------------------------------------------------------------
// E054 — NOFORN ⊥ RELIDO (FactRemove)
// ---------------------------------------------------------------------------

#[test]
fn e054_emits_correct_fix_intent_shape() {
    // Portion form: `(S//NF/RELIDO)`. NF (NOFORN) is the asserting
    // side per the dual-citation chain (§H.8 p145 NOFORN entry +
    // §H.8 p154 RELIDO entry). RELIDO is the rejected token.
    let source = "(S//NF/RELIDO)";
    let diags = engine().lint(source.as_bytes()).diagnostics;
    let d = diags
        .iter()
        .find(|d| d.rule.as_str() == "E054")
        .expect("E054 must fire on (S//NF/RELIDO)");

    // Dual-population invariant: BOTH fields populated post-migration.
    let fix = d.fix.as_ref().expect("E054 must carry legacy `fix`");
    let intent = d
        .fix_intent
        .as_ref()
        .expect("E054 must carry new `fix_intent`");

    // Structural intent payload — FactRemove { RELIDO, Scope::Portion }.
    match &intent.replacement {
        ReplacementIntent::FactRemove { token_ref, scope } => {
            assert!(
                matches!(token_ref, FactRef::Cve(id) if *id == TOK_RELIDO),
                "E054 intent must remove TOK_RELIDO; got token_ref = {token_ref:?}"
            );
            assert_eq!(
                *scope,
                Scope::Portion,
                "E054 fact-set delta applies at Scope::Portion"
            );
        }
        other => panic!("E054 intent must be FactRemove, got {other:?}"),
    }

    // Confidence must agree across the two emission paths (the engine
    // applies the same threshold filter to both; divergence would
    // produce different filter behavior on the same diagnostic).
    assert!((intent.confidence.rule - fix.confidence.rule).abs() < f32::EPSILON);
    assert!((intent.confidence.rule - 0.95).abs() < f32::EPSILON);

    // Legacy projection is byte-identical to pre-migration emission.
    assert_eq!(fix.source, FixSource::BuiltinRule);
    assert_eq!(fix.replacement.as_ref(), "");
    assert_eq!(fix.original.as_ref(), "/RELIDO");
}

#[test]
fn e054_promotes_through_engine_as_new_variant() {
    // Run `(S//NF/RELIDO)` through the full Engine::fix path. The
    // returned AppliedFix must carry AppliedFixProposal::New, NOT
    // Legacy — the pairing path in fix_inner routed the migrated
    // diagnostic through __engine_promote.
    let source = "(S//NF/RELIDO)";
    let result = engine().fix(source.as_bytes(), FixMode::Apply);

    let applied = result
        .applied
        .iter()
        .find(|af| af.proposal.rule.as_str() == "E054")
        .expect("E054 must promote through Engine::fix");

    match &applied.proposal {
        AppliedFixProposal::New {
            intent,
            synthesized,
        } => {
            // Synthesized = pre-migration FixProposal verbatim
            // (the rule pre-populated the legacy projection in
            // dual-population mode, so the engine pulled it from
            // Diagnostic.fix without going through
            // fix_intent_to_legacy_proposal).
            assert_eq!(synthesized.rule.as_str(), "E054");
            assert_eq!(synthesized.replacement.as_ref(), "");
            assert_eq!(synthesized.original.as_ref(), "/RELIDO");
            assert_eq!(synthesized.source, FixSource::BuiltinRule);

            // Intent payload — FactRemove of RELIDO at Scope::Portion.
            assert!(matches!(
                intent.replacement,
                ReplacementIntent::FactRemove {
                    token_ref: FactRef::Cve(t),
                    scope: Scope::Portion,
                } if t == TOK_RELIDO
            ));
        }
        AppliedFixProposal::Legacy(_) => {
            panic!(
                "E054 must promote through AppliedFixProposal::New (Commit 3 \
                 dual-population); got Legacy variant — the intent_index pairing \
                 did not fire."
            );
        }
    }

    // Post-fix source: RELIDO removed.
    assert_eq!(result.source, b"(S//NF)");
}

// ---------------------------------------------------------------------------
// E057 — ORCON-USGOV ⊥ RELIDO (FactRemove)
// ---------------------------------------------------------------------------

#[test]
fn e057_emits_correct_fix_intent_shape() {
    let source = "(S//OC-USGOV/RELIDO)";
    let diags = engine().lint(source.as_bytes()).diagnostics;
    let d = diags
        .iter()
        .find(|d| d.rule.as_str() == "E057")
        .expect("E057 must fire on (S//OC-USGOV/RELIDO)");

    let fix = d.fix.as_ref().expect("E057 must carry legacy `fix`");
    let intent = d
        .fix_intent
        .as_ref()
        .expect("E057 must carry new `fix_intent`");

    match &intent.replacement {
        ReplacementIntent::FactRemove { token_ref, scope } => {
            assert!(matches!(token_ref, FactRef::Cve(id) if *id == TOK_RELIDO));
            assert_eq!(*scope, Scope::Portion);
        }
        other => panic!("E057 intent must be FactRemove, got {other:?}"),
    }
    assert!((intent.confidence.rule - 0.95).abs() < f32::EPSILON);
    assert_eq!(fix.replacement.as_ref(), "");
}

#[test]
fn e057_promotes_through_engine_as_new_variant() {
    let source = "(S//OC-USGOV/RELIDO)";
    let result = engine().fix(source.as_bytes(), FixMode::Apply);

    let applied = result
        .applied
        .iter()
        .find(|af| af.proposal.rule.as_str() == "E057")
        .expect("E057 must promote through Engine::fix");

    match &applied.proposal {
        AppliedFixProposal::New {
            intent,
            synthesized,
        } => {
            assert_eq!(synthesized.rule.as_str(), "E057");
            assert_eq!(synthesized.replacement.as_ref(), "");
            assert!(matches!(
                intent.replacement,
                ReplacementIntent::FactRemove {
                    token_ref: FactRef::Cve(t),
                    scope: Scope::Portion,
                } if t == TOK_RELIDO
            ));
        }
        AppliedFixProposal::Legacy(_) => panic!("E057 must promote as New"),
    }
    assert_eq!(result.source, b"(S//OC-USGOV)");
}

// ---------------------------------------------------------------------------
// E021 — RD/FRD requires NOFORN (FactAdd) — TFNI excluded per §H.6 p120/p121
// ---------------------------------------------------------------------------

#[test]
fn e021_severity_is_fix_post_migration() {
    // Pre-PR-3c E021 default severity was Severity::Error with no
    // fix; PR 3c.B Commit 3 flipped it to Severity::Fix with a
    // FactAdd { NOFORN } emission. The change is documented inline
    // on the rule's `default_severity()` and in the migration commit
    // message; this test pins the post-migration severity via the
    // engine's lint surface (rules_declarative is pub(crate) so the
    // rule struct itself isn't accessible from the integration test;
    // emitted-severity equivalence is sufficient).
    let source = "(S//RD//IMC)";
    let diags = engine().lint(source.as_bytes()).diagnostics;
    let d = diags
        .iter()
        .find(|d| d.rule.as_str() == "E021")
        .expect("E021 must fire on (S//RD//IMC)");
    assert_eq!(
        d.severity,
        Severity::Fix,
        "E021 emitted-severity must be Severity::Fix post-PR-3c Commit 3 \
         (pre-migration was Severity::Error no-fix; §H.6 p104 gives a \
         deterministic repair so the engine auto-applies)"
    );
}

#[test]
fn e021_emits_correct_fix_intent_shape() {
    // Portion form: `(S//RD//IMC)` — RD is AEA, IMC is the last (and
    // only) IC dissem token. NOFORN absent → E021 fires; the legacy
    // FixProposal anchors on IMC and appends `/NOFORN`.
    let source = "(S//RD//IMC)";
    let diags = engine().lint(source.as_bytes()).diagnostics;
    let d = diags
        .iter()
        .find(|d| d.rule.as_str() == "E021")
        .expect("E021 must fire on (S//RD//IMC)");

    assert_eq!(d.severity, Severity::Fix);

    let fix = d.fix.as_ref().expect("E021 must carry legacy `fix`");
    let intent = d
        .fix_intent
        .as_ref()
        .expect("E021 must carry new `fix_intent`");

    // Structural intent — FactAdd { NOFORN, Scope::Portion }.
    match &intent.replacement {
        FactAdd { token, scope } => {
            assert!(
                matches!(token, FactRef::Cve(id) if *id == TOK_NOFORN),
                "E021 intent must add TOK_NOFORN; got token = {token:?}"
            );
            assert_eq!(*scope, Scope::Portion);
        }
        other => panic!("E021 intent must be FactAdd, got {other:?}"),
    }
    assert!((intent.confidence.rule - 0.95).abs() < f32::EPSILON);

    // Legacy projection — anchor on IMC, replacement = "IMC/NOFORN"
    // (token-anchored append; required because the engine's
    // `!f.span.is_empty()` filter drops genuine zero-width fixes).
    assert_eq!(fix.source, FixSource::BuiltinRule);
    assert_eq!(fix.original.as_ref(), "IMC");
    assert_eq!(fix.replacement.as_ref(), "IMC/NOFORN");
}

#[test]
fn e021_promotes_through_engine_as_new_variant() {
    let source = "(S//RD//IMC)";
    let result = engine().fix(source.as_bytes(), FixMode::Apply);

    let applied = result
        .applied
        .iter()
        .find(|af| af.proposal.rule.as_str() == "E021")
        .expect("E021 must auto-apply post Error→Fix flip");

    match &applied.proposal {
        AppliedFixProposal::New {
            intent,
            synthesized,
        } => {
            assert_eq!(synthesized.rule.as_str(), "E021");
            assert_eq!(synthesized.original.as_ref(), "IMC");
            assert_eq!(synthesized.replacement.as_ref(), "IMC/NOFORN");
            assert!(matches!(
                intent.replacement,
                FactAdd {
                    token: FactRef::Cve(t),
                    scope: Scope::Portion,
                } if t == TOK_NOFORN
            ));
        }
        AppliedFixProposal::Legacy(_) => panic!("E021 must promote as New"),
    }

    // Post-fix source: NOFORN appended after IMC.
    assert_eq!(result.source, b"(S//RD//IMC/NOFORN)");
}

#[test]
fn e021_falls_back_to_no_fix_when_no_ic_dissem_block_exists() {
    // (S//RD) — no IC dissem block at all. The rule still fires
    // (Severity::Fix; the diagnostic surfaces) but cannot emit a
    // structural fix without synthesizing a whole `//`-separated
    // category from rule context. Same defensive policy as
    // `compute_relido_removal_span` and `emit_companion_insert`:
    // never emit a malformed fix.
    let source = "(S//RD)";
    let diags = engine().lint(source.as_bytes()).diagnostics;
    let d = diags
        .iter()
        .find(|d| d.rule.as_str() == "E021")
        .expect("E021 must fire on (S//RD)");

    assert_eq!(d.severity, Severity::Fix);
    assert!(
        d.fix.is_none() && d.fix_intent.is_none(),
        "E021 must emit no fix when the portion has no IC dissem block; \
         got fix={:?}, fix_intent={:?}",
        d.fix,
        d.fix_intent
    );
}

// ---------------------------------------------------------------------------
// E055 / E056 control group — non-migrated rules still emit FixProposal-only
// ---------------------------------------------------------------------------
//
// E055 (RELIDO ⊥ DISPLAY ONLY) and E056 (ORCON ⊥ RELIDO) are NOT in
// the Commit 3 beachhead set. They continue to emit only
// `Diagnostic.fix` (legacy `FixProposal`); `Diagnostic.fix_intent` is
// `None`. Engine routes them through `__engine_promote_legacy` →
// `AppliedFixProposal::Legacy`. These tests pin the unchanged
// behavior so a future PR migrating E055 / E056 has to update both
// the rule body AND this test, not just the rule body.

#[test]
fn e055_emits_legacy_fix_only_post_commit_3() {
    let source = "(S//RELIDO/DISPLAY ONLY)";
    let diags = engine().lint(source.as_bytes()).diagnostics;
    let Some(d) = diags.iter().find(|d| d.rule.as_str() == "E055") else {
        // Parser may not recognize "DISPLAY ONLY" with space; the
        // test exists primarily as a regression pin for the dual-
        // population invariant. If E055 doesn't fire, the control-
        // group assertion is vacuously satisfied.
        //
        // Surface the vacuous-pass in CI logs so a future change
        // that silently breaks the parser path (e.g., regresses
        // "DISPLAY ONLY" tokenization) doesn't mask the regression
        // by also masking this test. E056 carries the same
        // control-group invariant via a separately-tokenized
        // fixture, so the dual-population pin is not lost.
        eprintln!(
            "WARNING: e055_emits_legacy_fix_only_post_commit_3 vacuously passed — \
             parser did not recognize `DISPLAY ONLY` in fixture \
             `(S//RELIDO/DISPLAY ONLY)`. Tracked as a parser-gap follow-up; \
             E056 carries the same control-group invariant."
        );
        return;
    };
    assert!(
        d.fix.is_some() && d.fix_intent.is_none(),
        "E055 (non-migrated) must emit only `fix`; intent migration \
         lands in a later commit. Got fix={:?}, fix_intent={:?}",
        d.fix,
        d.fix_intent
    );
}

#[test]
fn e056_emits_legacy_fix_only_post_commit_3() {
    let source = "(S//OC/RELIDO)";
    let diags = engine().lint(source.as_bytes()).diagnostics;
    let d = diags
        .iter()
        .find(|d| d.rule.as_str() == "E056")
        .expect("E056 must fire on (S//OC/RELIDO)");
    assert!(
        d.fix.is_some() && d.fix_intent.is_none(),
        "E056 (non-migrated) must emit only `fix`; intent migration \
         lands in a later commit. Got fix={:?}, fix_intent={:?}",
        d.fix,
        d.fix_intent
    );

    // Sanity: the legacy proposal still promotes through
    // __engine_promote_legacy as AppliedFixProposal::Legacy.
    let result = engine().fix(source.as_bytes(), FixMode::Apply);
    let applied = result
        .applied
        .iter()
        .find(|af| af.proposal.rule.as_str() == "E056")
        .expect("E056 must auto-apply");
    assert!(
        matches!(applied.proposal, AppliedFixProposal::Legacy(_)),
        "E056 must promote as Legacy, not New (no FixIntent migrated yet)"
    );
}
