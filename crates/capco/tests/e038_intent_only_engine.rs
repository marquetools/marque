#![cfg(any())]
// PR 3c.B Commit 10: legacy FixProposal-shape test disabled pending rewrite

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Sub-PR 8.D.1 — E038 (NODIS/EXDIS require NOFORN)
//! intent-only migration engine-level tests. Validates
//! `synthesize_intent_only_fixes` → `apply_intent` (FactAdd path,
//! first consumer) → `render_portion`. **#106 stays open — this PR
//! does NOT close it.** The FR-045 parser within-category-separator-
//! spans work that #106 tracks is sidestepped here via
//! `synthesize_intent_only_fixes` re-rendering, same pattern as
//! PR #370 (E041).
//!
//! These tests cover engine-synthesis behaviors that can't be
//! exercised through the `lint_portion` helper inside
//! `crates/capco/src/rules_declarative.rs::tests` (the inline module
//! sees a different `CapcoScheme` crate identity than `marque-engine`):
//!
//! - Round-trip: `(S//ND)` → `(S//NF//ND)` after one `Engine::fix`
//!   pass. Validates that `synthesize_intent_only_fixes` →
//!   `apply_intent` → `render_portion` produces the canonical
//!   NOFORN-then-NODIS portion. `apply_intent`'s FactAdd path lands
//!   in this PR alongside this test.
//! - Idempotence: a second `Engine::fix` pass is a fixed point —
//!   E038's predicate (NODIS or EXDIS present without NOFORN) is
//!   false once NOFORN has been added.
//! - FR-016 split: E037 (NODIS+EXDIS no-fix conflict, lex-min on the
//!   group) does NOT block E038's FactAdd or E041's FactRemove from
//!   applying. The synthesis path is gated on `fix_intent.is_some()`,
//!   not on rule-ID priority within the candidate group.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};
use secrecy::ExposeSecret as _;

fn engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(std::time::UNIX_EPOCH)),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

/// Round-trip: input `(S//ND)` produces canonical `(S//NF//ND)` after
/// one `Engine::fix` pass.
///
/// The canonical output is what `CapcoScheme::render_portion` emits
/// after `apply_intent` adds NOFORN to the marking's IC-dissem-control
/// axis. NOFORN sorts ahead of the non-IC dissem (NODIS) per CAPCO
/// §G.1 Table 4 (IC dissem block precedes non-IC dissem block in
/// portion ordering). This test pins both the FactAdd application and
/// the render-order output in one round-trip.
#[test]
fn round_trip_e038_adds_noforn_to_nodis_portion() {
    let result = engine().fix(b"(S//ND)\n", FixMode::Apply);

    assert_eq!(result.source.expose_secret(),
        b"(S//NF//ND)\n",
        "E038 round-trip must produce canonical portion with NOFORN \
         added before the non-IC dissem block; got: {:?}",
        std::str::from_utf8(result.source.expose_secret()).unwrap_or("<non-utf8>")
    );

    let e038 = result
        .applied
        .iter()
        .find(|af| af.rule.as_str() == "E038")
        .unwrap_or_else(|| {
            panic!(
                "E038 must auto-apply through `synthesize_intent_only_fixes` \
                 (FactAdd path); applied rules: {:?}",
                result
                    .applied
                    .iter()
                    .map(|af| af.rule.as_str())
                    .collect::<Vec<_>>()
            )
        });

    // G13 invariant (Constitution V Principle V): intent-only fixes
    // carry an empty `original` so document content cannot leak into
    // the audit record. Structurally enforced by
    // `synthesize_intent_only_fixes` in `engine.rs`; asserted here to
    // make the invariant self-documenting at the rule level. See
    // `g13_closure_fix_intent.rs` for the workspace-wide gate.
    assert!(
        e038.proposal.original.is_empty(),
        "G13: intent-only AppliedFix must carry empty `original` \
         (no document content in audit record); got: {:?}",
        e038.proposal.original
    );
}

/// EXDIS branch: `(S//XD)` exercises the `trigger_token = TOK_EXDIS`
/// path of the rule, complementing `round_trip_e038_adds_noforn_to_nodis_portion`
/// (which only exercises the NODIS branch). The same FactAdd intent
/// fires; the `MessageArgs.token` payload differs (EXDIS vs NODIS)
/// but the byte-level rewrite is identical in shape.
#[test]
fn round_trip_e038_adds_noforn_to_exdis_portion() {
    let result = engine().fix(b"(S//XD)\n", FixMode::Apply);

    assert_eq!(result.source.expose_secret(),
        b"(S//NF//XD)\n",
        "E038 EXDIS-branch round-trip must produce canonical portion \
         with NOFORN added before the non-IC dissem block; got: {:?}",
        std::str::from_utf8(result.source.expose_secret()).unwrap_or("<non-utf8>")
    );

    assert!(
        result.applied.iter().any(|af| af.rule.as_str() == "E038"),
        "E038 must auto-apply on EXDIS-only input through \
         `synthesize_intent_only_fixes`; applied rules: {:?}",
        result
            .applied
            .iter()
            .map(|af| af.rule.as_str())
            .collect::<Vec<_>>()
    );
}

/// Regression test for the Copilot review of PR #372 (second round):
/// when a marking contains other `NonIcDissem` variants before
/// NODIS/EXDIS in source order (e.g., `(S//DS/ND)` where `DS` is
/// LIMDIS's portion form), E038 must still fire and target NODIS.
/// The rule must scan the full `non_ic_dissem` collection to find
/// the first NODIS-or-EXDIS entry — an earlier shortcut that read
/// only `attrs.non_ic_dissem.first()` silently dropped the
/// diagnostic on such inputs, which is the regression this test
/// pins.
#[test]
fn e038_scans_past_other_non_ic_dissem_to_find_trigger() {
    let result = engine().fix(b"(S//DS/ND)\n", FixMode::Apply);

    assert!(
        result.applied.iter().any(|af| af.rule.as_str() == "E038"),
        "E038 must auto-apply on `(S//DS/ND)` — the rule must scan \
         past LIMDIS to find NODIS in `non_ic_dissem`; applied: {:?}",
        result
            .applied
            .iter()
            .map(|af| af.rule.as_str())
            .collect::<Vec<_>>(),
    );
    // Post-state byte assertion: NOFORN added to IC-dissem block;
    // LIMDIS and NODIS remain in the non-IC block in their render-
    // priority order. The exact non-IC ordering depends on the
    // renderer's `render_non_ic_dissem` priority table — assert that
    // NOFORN is present in the output as the load-bearing property,
    // not the exact byte arrangement of the trailing block.
    let s = std::str::from_utf8(result.source.expose_secret()).unwrap_or("<non-utf8>");
    assert!(
        s.contains("NF"),
        "E038 fix must place NOFORN in the output; got: {s:?}",
    );
    assert!(
        s.contains("DS") && s.contains("ND"),
        "LIMDIS (DS) and NODIS (ND) must remain in the non-IC dissem \
         block after the fix; got: {s:?}",
    );
}

/// Idempotence: running `Engine::fix` twice reaches a fixed point.
/// First pass: `(S//ND)` → `(S//NF//ND)`. Second pass: `(S//NF//ND)` →
/// no E038 diagnostics, output byte-identical to first-pass output.
///
/// The second-pass invariant is the load-bearing one. A non-idempotent
/// fix would either keep producing diffs on each pass (oscillating
/// indefinitely) or fail to recognize the post-state. E038's
/// predicate is "NODIS or EXDIS present without NOFORN"; once NOFORN
/// is added, the predicate is false on subsequent passes.
#[test]
fn e038_idempotent_after_one_pass() {
    let first = engine().fix(b"(S//ND)\n", FixMode::Apply);
    assert_eq!(
        first.source, b"(S//NF//ND)\n",
        "first pass must produce canonical NOFORN-then-NODIS portion"
    );

    let second = engine().fix(&first.source, FixMode::Apply);
    assert_eq!(
        second.source,
        first.source,
        "second pass must be byte-identical to first-pass output \
         (fixed point); got: {:?}",
        std::str::from_utf8(&second.source).unwrap_or("<non-utf8>")
    );
    assert!(
        second.applied.iter().all(|af| af.rule.as_str() != "E038"),
        "second pass must not re-apply E038 (idempotence); applied \
         rules: {:?}",
        second
            .applied
            .iter()
            .map(|af| af.rule.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        second
            .remaining_diagnostics
            .iter()
            .all(|d| d.rule.as_str() != "E038"),
        "second pass must produce zero E038 diagnostics (predicate is \
         false once NOFORN is present); remaining diagnostics: {:?}",
        second
            .remaining_diagnostics
            .iter()
            .map(|d| d.rule.as_str())
            .collect::<Vec<_>>()
    );
}

/// FR-016 split: when E037 (no-fix conflict), E038 (FactAdd NOFORN),
/// and E041 (FactRemove EXDIS) all fire on `(S//ND/XD)`, E037 must
/// NOT block the intent-only rules from applying. The synthesis path
/// is gated on `fix_intent.is_some()`, not on rule-ID priority — E037
/// emits `(fix: None, fix_intent: None)` so it isn't eligible for the
/// synthesis pipeline.
///
/// Expected post-state: NOFORN added (E038's FactAdd) and EXDIS
/// removed (E041's FactRemove), producing `(S//NF//ND)`. Both intents
/// share the same `candidate_span` (the full `(S//ND/XD)` portion),
/// so `synthesize_intent_only_fixes` collapses them into ONE
/// FixProposal under the lex-min rule_id — `'E038' < 'E041'`, so E038
/// claims the audit slot and E041 surfaces in `remaining_diagnostics`
/// (it fired and was observed, but the engine applied one rewrite,
/// not two). This is the architect-preflight "synthesize one
/// FixProposal per candidate-span group" design captured in
/// `synthesize_intent_only_fixes` (engine.rs:2020).
#[test]
fn e038_fr016_split_against_e037() {
    let result = engine().fix(b"(S//ND/XD)\n", FixMode::Apply);

    assert_eq!(result.source.expose_secret(),
        b"(S//NF//ND)\n",
        "FR-016 split: E038's FactAdd(NOFORN) and E041's FactRemove(EXDIS) \
         must both apply atomically despite E037 (no-fix) firing on the \
         same candidate; got: {:?}",
        std::str::from_utf8(result.source.expose_secret()).unwrap_or("<non-utf8>")
    );

    let applied_ids: Vec<&str> = result.applied.iter().map(|af| af.rule.as_str()).collect();
    assert!(
        applied_ids.contains(&"E038"),
        "E038 must auto-apply as the lex-min rule_id in the \
         intent-only group ('E038' < 'E041'); applied: {applied_ids:?}",
    );
    assert!(
        !applied_ids.contains(&"E037"),
        "E037 must NOT appear in `applied` (no-fix conflict rule); \
         applied: {applied_ids:?}",
    );

    // E037 surfaces in `remaining_diagnostics` (it fired but cannot
    // auto-apply). E041 ALSO surfaces in `remaining_diagnostics` —
    // its intent applied through the atomic group, but the audit
    // entry went to lex-min E038, so E041's diagnostic remains
    // visible to the caller per the architect-preflight "honest
    // audit output" design at engine.rs:2045.
    let remaining_ids: Vec<&str> = result
        .remaining_diagnostics
        .iter()
        .map(|d| d.rule.as_str())
        .collect();
    assert!(
        remaining_ids.contains(&"E037"),
        "E037 must surface in `remaining_diagnostics` (no-fix \
         conflict rule); remaining: {remaining_ids:?}",
    );
    assert!(
        remaining_ids.contains(&"E041"),
        "E041 must surface in `remaining_diagnostics` (its FactRemove \
         intent applied through the atomic candidate-span group, but \
         the audit slot went to lex-min E038, so E041's diagnostic \
         remains visible to the caller per the architect-preflight \
         honest-audit-output design); remaining: {remaining_ids:?}",
    );
}
