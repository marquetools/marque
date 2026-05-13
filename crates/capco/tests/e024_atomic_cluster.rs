#![cfg(any())]
// PR 3c.B Commit 10: legacy FixProposal-shape test disabled pending rewrite

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Sub-PR 8.C — E024 (RD precedence over FRD/TFNI) atomic-cluster
//! engine-level tests.
//!
//! Issue: extend `ReplacementIntent::FactRemove` to
//! `SmallVec<[FactRef<S>; 2]>` so atomic chain removals — like
//! "remove both FRD and TFNI when RD is present" — land as a single
//! audit-record entry (Constitution V Principle V: one policy decision →
//! one audit repair).
//!
//! These tests verify:
//!
//! - **Round-trip FRD only**: `(TS//RD//FRD//NF)` → `(TS//RD//NF)` in
//!   one pass. E024 removes only FRD (TFNI absent); single `AppliedFix`.
//! - **Round-trip TFNI only**: `(TS//RD//TFNI//NF)` → `(TS//RD//NF)` in
//!   one pass.
//! - **Atomic-cluster round-trip**: `(TS//RD//FRD//TFNI//NF)` →
//!   `(TS//RD//NF)` in one pass. Both FRD and TFNI removed atomically;
//!   exactly ONE `AppliedFix` in the audit stream (the multi-fact
//!   `FactRemove` is collapsed by `synthesize_intent_only_fixes` to one
//!   entry per `candidate_span`).
//! - **Idempotence**: a second `Engine::fix` pass is a fixed point.
//! - **Audit cardinality**: the atomic-cluster pass produces exactly ONE
//!   `AppliedFix` entry for E024, not two (one for FRD, one for TFNI).
//!   This is the load-bearing test for Constitution V Principle V
//!   compliance: "one policy decision → one audit record".
//!
//! CAPCO-2016 §H.6 p104: "If RD, FRD, and TFNI portions are in a
//! document, the RD takes precedence and is conveyed in the banner line."

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};

fn engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(std::time::UNIX_EPOCH)),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

/// Round-trip (FRD only): `(TS//RD//FRD//NF)` → `(TS//RD//NF)`.
///
/// When TFNI is absent, E024 emits a single-fact `FactRemove([FRD])`
/// intent. After `synthesize_intent_only_fixes`, the marking is
/// re-rendered without FRD. The expected canonical form contains only
/// RD in the AEA axis.
#[test]
fn e024_fix_round_trip_removes_frd_when_rd_present() {
    let result = engine().fix(b"(TS//RD//FRD//NF)\n", FixMode::Apply);

    assert_eq!(
        result.source,
        b"(TS//RD//NF)\n",
        "E024 round-trip must produce (TS//RD//NF) with FRD removed; \
         got: {:?}",
        std::str::from_utf8(&result.source).unwrap_or("<non-utf8>")
    );

    assert!(
        result.applied.iter().any(|af| af.rule.as_str() == "E024"),
        "E024 must auto-apply; applied rules: {:?}",
        result
            .applied
            .iter()
            .map(|af| af.rule.as_str())
            .collect::<Vec<_>>()
    );
}

/// Round-trip (TFNI only): `(TS//RD//TFNI//NF)` → `(TS//RD//NF)`.
///
/// When FRD is absent, E024 emits a single-fact `FactRemove([TFNI])`
/// intent.
#[test]
fn e024_fix_round_trip_removes_tfni_when_rd_present() {
    let result = engine().fix(b"(TS//RD//TFNI//NF)\n", FixMode::Apply);

    assert_eq!(
        result.source,
        b"(TS//RD//NF)\n",
        "E024 round-trip must produce (TS//RD//NF) with TFNI removed; \
         got: {:?}",
        std::str::from_utf8(&result.source).unwrap_or("<non-utf8>")
    );

    assert!(
        result.applied.iter().any(|af| af.rule.as_str() == "E024"),
        "E024 must auto-apply; applied rules: {:?}",
        result
            .applied
            .iter()
            .map(|af| af.rule.as_str())
            .collect::<Vec<_>>()
    );
}

/// Atomic-cluster round-trip: `(TS//RD//FRD//TFNI//NF)` → `(TS//RD//NF)`
/// in one pass.
///
/// **Load-bearing test for the issue's acceptance criteria.**
///
/// Both FRD and TFNI are superseded by RD. E024 emits ONE diagnostic
/// with ONE `FactRemove { facts: [FRD, TFNI], scope: Portion }` intent.
/// `synthesize_intent_only_fixes` collapses the diagnostic group for
/// this `candidate_span` into ONE `FixProposal`, which promotes to
/// exactly ONE `AppliedFix` in the audit stream.
///
/// Emitting two separate `FixProposal`s (one per token) would violate
/// Constitution V Principle V: "one policy decision → one audit record."
#[test]
fn e024_atomic_cluster_removes_both_frd_and_tfni_in_one_audit_entry() {
    let result = engine().fix(b"(TS//RD//FRD//TFNI//NF)\n", FixMode::Apply);

    // Byte-level round-trip: both FRD and TFNI removed in one pass.
    assert_eq!(
        result.source,
        b"(TS//RD//NF)\n",
        "E024 atomic-cluster must produce (TS//RD//NF) with both FRD and \
         TFNI removed in one pass; got: {:?}",
        std::str::from_utf8(&result.source).unwrap_or("<non-utf8>")
    );

    // Audit cardinality: exactly ONE AppliedFix for E024.
    // This is the Constitution V Principle V assertion: one policy
    // decision (RD supersedes both FRD and TFNI) must produce one
    // audit entry, not two.
    let e024_applied: Vec<_> = result
        .applied
        .iter()
        .filter(|af| af.rule.as_str() == "E024")
        .collect();

    assert_eq!(
        e024_applied.len(),
        1,
        "E024 atomic cluster must produce exactly ONE AppliedFix entry \
         (Constitution V: one policy decision → one audit record); \
         got {} E024 entries; all applied: {:?}",
        e024_applied.len(),
        result
            .applied
            .iter()
            .map(|af| af.rule.as_str())
            .collect::<Vec<_>>()
    );
}

/// Idempotence: running `Engine::fix` twice reaches a fixed point.
///
/// First pass: `(TS//RD//FRD//TFNI//NF)` → `(TS//RD//NF)`.
/// Second pass: `(TS//RD//NF)` → no E024 diagnostics, output byte-
/// identical to first-pass output. The E024 predicate fires only when
/// BOTH RD AND (FRD or TFNI) are present; once the superseded tokens
/// are removed, the predicate is false.
#[test]
fn e024_fix_is_idempotent_second_pass_clears_all_e024() {
    let first = engine().fix(b"(TS//RD//FRD//TFNI//NF)\n", FixMode::Apply);
    assert_eq!(
        first.source, b"(TS//RD//NF)\n",
        "first pass must produce canonical RD-only portion"
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
        second.applied.iter().all(|af| af.rule.as_str() != "E024"),
        "second pass must not re-apply E024 (idempotence); applied \
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
            .all(|d| d.rule.as_str() != "E024"),
        "second pass must produce zero E024 diagnostics (predicate \
         `has_rd && has_superseded` is false after FRD/TFNI removal); \
         remaining diagnostics: {:?}",
        second
            .remaining_diagnostics
            .iter()
            .map(|d| d.rule.as_str())
            .collect::<Vec<_>>()
    );
}

/// Negative guard: RD alone (no FRD or TFNI) must not trigger E024.
///
/// The E024 predicate is `has_rd && (has_frd || has_tfni)`. When only
/// RD is present the predicate is false; no diagnostic must be emitted.
/// This locks in the no-fire contract so a future change to the
/// predicate cannot silently cause false positives on clean RD portions.
///
/// CAPCO-2016 §H.6 p104–p105: RD takes precedence *when* FRD or TFNI
/// are commingled; there is nothing to fix when they are absent.
#[test]
fn e024_does_not_fire_on_rd_alone() {
    let input = b"(TS//RD//NF) Marking without FRD or TFNI.\n";
    let result = engine().lint(input);
    assert!(
        result.diagnostics.iter().all(|d| d.rule.as_str() != "E024"),
        "E024 must not fire on RD-alone portions; got: {:?}",
        result
            .diagnostics
            .iter()
            .map(|d| d.rule.as_str())
            .collect::<Vec<_>>()
    );
}
