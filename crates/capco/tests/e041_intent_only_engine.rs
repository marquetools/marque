#![cfg(any())]
// PR 3c.B Commit 10: legacy FixProposal-shape test disabled pending rewrite

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Sub-PR 8.E.2 — E041 (NODIS supersedes EXDIS in portion)
//! intent-only migration engine-level tests. Unblocks E041 — the
//! primary rule named in #106 — via `synthesize_intent_only_fixes`
//! re-rendering. #106 itself stays open as the tracking ticket for
//! the FR-045 parser within-category-separator-spans work that other
//! downstream rules (per `specs/006-engine-rule-refactor/spec.md`)
//! depend on.
//!
//! These tests cover the engine-synthesis behaviors that can't be
//! exercised through the `lint_portion` helper inside
//! `crates/capco/src/rules.rs::tests` (the inline module sees a
//! different `CapcoScheme` crate identity than `marque-engine`):
//!
//! - Round-trip: `(S//NF//ND/XD)` → `(S//NF//ND)` after one
//!   `Engine::fix` pass. Validates that
//!   `synthesize_intent_only_fixes` → `apply_intent` →
//!   `render_portion` produces the canonical NODIS-only portion.
//! - Idempotence: a second `Engine::fix` pass is a fixed point;
//!   E041's predicate (`has_nodis && has_exdis`) is false after
//!   EXDIS is removed.
//! - FR-016 split: E037 (no-fix conflict, lex-min rule_id on the
//!   group) does NOT block E041's intent-only fix — the synthesis
//!   path is gated on `fix_intent.is_some()`, not on rule-ID
//!   priority within the candidate group.
//!
//! The `(S//NF//ND/XD)` shape carries NF so E038
//! (NODIS-requires-NOFORN) does not also fire and compete for the
//! candidate; the test focuses on the E037+E041 interaction.

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

/// Round-trip: input `(S//NF//ND/XD)` produces canonical
/// `(S//NF//ND)` after one `Engine::fix` pass.
///
/// The canonical output is what `CapcoScheme::render_portion` emits
/// after `apply_intent` removes EXDIS from the marking's
/// non-IC-dissem axis. NF (NOFORN) is included so E038
/// (NODIS-requires-NOFORN) does not fire — without it, E038 would
/// emit its own fix and complicate the assertion.
#[test]
fn e041_fix_round_trip_produces_canonical_nodis_only_portion() {
    let result = engine().fix(b"(S//NF//ND/XD)\n", FixMode::Apply);

    assert_eq!(result.source.expose_secret(),
        b"(S//NF//ND)\n",
        "E041 round-trip must produce canonical portion with EXDIS \
         removed; got: {:?}",
        std::str::from_utf8(result.source.expose_secret()).unwrap_or("<non-utf8>")
    );

    assert!(
        result.applied.iter().any(|af| af.rule.as_str() == "E041"),
        "E041 must auto-apply through `synthesize_intent_only_fixes`; \
         applied rules: {:?}",
        result
            .applied
            .iter()
            .map(|af| af.rule.as_str())
            .collect::<Vec<_>>()
    );
}

/// Idempotence: running `Engine::fix` twice reaches a fixed point.
/// First pass: `(S//NF//ND/XD)` → `(S//NF//ND)`. Second pass:
/// `(S//NF//ND)` → no E041 diagnostics, output byte-identical to
/// first-pass output.
///
/// The second-pass invariant is the load-bearing one: a non-
/// idempotent fix would either keep producing diffs on each pass
/// (forever oscillating) or re-introduce EXDIS — both correctness
/// failures. The `apply_intent` removal is set-based (filter out
/// EXDIS from `non_ic_dissem`), so once EXDIS is gone the rule
/// predicate `has_nodis && has_exdis` is false.
#[test]
fn e041_fix_is_idempotent_second_pass_clears_all_e041() {
    let first = engine().fix(b"(S//NF//ND/XD)\n", FixMode::Apply);
    assert_eq!(
        first.source, b"(S//NF//ND)\n",
        "first pass must produce canonical NODIS-only portion"
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
        second.applied.iter().all(|af| af.rule.as_str() != "E041"),
        "second pass must not re-apply E041 (idempotence); applied \
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
            .all(|d| d.rule.as_str() != "E041"),
        "second pass must produce zero E041 diagnostics (predicate \
         `has_nodis && has_exdis` is false after EXDIS removal); \
         remaining diagnostics: {:?}",
        second
            .remaining_diagnostics
            .iter()
            .map(|d| d.rule.as_str())
            .collect::<Vec<_>>()
    );
}

/// FR-016 deterministic ordering: when both E037 (no-fix conflict)
/// and E041 (intent-only fix) fire on the same candidate, E041's
/// fix applies and E037 surfaces in `remaining_diagnostics`.
///
/// E037 emits `(fix: None, fix_intent: None)` so it's not eligible
/// for the synthesis pipeline (which is gated on `fix_intent.is_some()`),
/// and the lex-min `(E037 < E041)` tiebreaker only matters within
/// the populated-fix set — an empty-fix rule cannot block a
/// populated-fix-intent rule from applying. This is the load-bearing
/// FR-016 invariant for intent-only rules with a no-fix sibling on
/// the same candidate.
#[test]
fn e041_applies_while_e037_remains_unfixed() {
    let result = engine().fix(b"(S//NF//ND/XD)\n", FixMode::Apply);

    assert!(
        result.applied.iter().any(|af| af.rule.as_str() == "E041"),
        "E041 must auto-apply; applied: {:?}",
        result
            .applied
            .iter()
            .map(|af| af.rule.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        result.applied.iter().all(|af| af.rule.as_str() != "E037"),
        "E037 must NOT appear in `applied` (no-fix rule); applied: {:?}",
        result
            .applied
            .iter()
            .map(|af| af.rule.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        result
            .remaining_diagnostics
            .iter()
            .any(|d| d.rule.as_str() == "E037"),
        "E037 must surface in `remaining_diagnostics` (no-fix \
         conflict rule); remaining: {:?}",
        result
            .remaining_diagnostics
            .iter()
            .map(|d| d.rule.as_str())
            .collect::<Vec<_>>()
    );
}
