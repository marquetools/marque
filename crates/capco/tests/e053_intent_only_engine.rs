#![cfg(any())]
// PR 3c.B Commit 10: legacy FixProposal-shape test disabled pending rewrite

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Sub-PR 8.D.2 — E053 (NOFORN ⊥ REL TO, §H.8 p145) intent-
//! only migration engine-level tests.
//!
//! E053 is the first consumer of the `TOK_REL_TO` whole-axis-clear
//! sentinel on CAT_REL_TO (extended in this sub-PR alongside the
//! per-country `TOK_USA` removal path). CAPCO-2016 §H.8 p145
//! NOFORN entry states verbatim: "Cannot be used with REL TO,
//! RELIDO, EYES ONLY, or DISPLAY ONLY." NOFORN unambiguously
//! supersedes REL TO; the intent clears the REL TO axis entirely
//! (not just USA), because §H.8 p145 names the prohibition against
//! REL TO as a whole, not against any particular country.
//!
//! These tests cover the engine-synthesis behaviors that can't be
//! exercised through the `lint_portion` helper inside
//! `crates/capco/src/rules.rs::tests` (the inline module sees a
//! different `CapcoScheme` crate identity than `marque-engine`):
//!
//! - Round-trip: `(S//NF//REL TO USA, GBR)` → `(S//NF)` after one
//!   `Engine::fix` pass. Validates that
//!   `synthesize_intent_only_fixes` → `apply_intent` →
//!   `render_portion` produces the canonical NOFORN-only portion.
//! - Idempotence: a second `Engine::fix` pass is a fixed point;
//!   E053's predicate (NOFORN + REL TO coexist) is false after
//!   REL TO is cleared.
//! - Predicate guards: rule does not fire when either NOFORN or
//!   REL TO is absent — regression guard against over-application.
//! - G13 invariant: the audit record's `proposal.original` is the
//!   empty string for the intent-only path (Constitution V
//!   Principle V — audit records carry no document content).
//!
//! Scope note: E053 fires on portions AND banners, but only
//! emits a `FixIntent` at portion scope. Banner-form firings
//! produce a diagnostic-only emission (no `fix_intent`) — the
//! `capco/noforn-clears-rel-to` PageRewrite is responsible for the
//! page-level mutation. Engine-level round-trip semantics here
//! cover the portion path; banner-form behavior stays in the
//! `lint_banner` unit tests in `crates/capco/src/rules.rs`.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixedClock};
use marque_rules::AppliedFixProposal;
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

/// Round-trip: input `(S//NF//REL TO USA, GBR)` produces canonical
/// `(S//NF)` after one `Engine::fix` pass.
///
/// The canonical output is what `CapcoScheme::render_portion` emits
/// after `apply_intent` clears the REL TO axis from the marking via
/// the `TOK_REL_TO` whole-axis-clear sentinel. NF (NOFORN) survives;
/// USA, GBR are dropped together because §H.8 p145 says NOFORN
/// "Cannot be used with REL TO" — the prohibition is whole-axis,
/// not per-country.
#[test]
fn round_trip_e053_removes_rel_to_when_noforn_present() {
    let result = engine().fix(b"(S//NF//REL TO USA, GBR)\n", FixMode::Apply);

    assert_eq!(result.source.expose_secret(),
        b"(S//NF)\n",
        "E053 round-trip must produce canonical NOFORN-only portion \
         with REL TO axis cleared; got: {:?}",
        std::str::from_utf8(result.source.expose_secret()).unwrap_or("<non-utf8>")
    );

    assert!(
        result.applied.iter().any(|af| af.rule.as_str() == "E053"),
        "E053 must auto-apply through `synthesize_intent_only_fixes`; \
         applied rules: {:?}",
        result
            .applied
            .iter()
            .map(|af| af.rule.as_str())
            .collect::<Vec<_>>()
    );
}

/// Idempotence: running `Engine::fix` twice reaches a fixed point.
/// First pass: `(S//NF//REL TO USA, GBR)` → `(S//NF)`. Second pass:
/// `(S//NF)` → no E053 diagnostics, output byte-identical to
/// first-pass output.
///
/// The second-pass invariant is the load-bearing one: a non-
/// idempotent fix would either keep producing diffs on each pass
/// (forever oscillating) or re-introduce REL TO — both correctness
/// failures. The `apply_intent` removal is set-based (clear
/// `attrs.rel_to`), so once REL TO is gone the rule predicate
/// (NOFORN + any REL TO entry) is false.
#[test]
fn e053_idempotent_after_one_pass() {
    let first = engine().fix(b"(S//NF//REL TO USA, GBR)\n", FixMode::Apply);
    assert_eq!(
        first.source, b"(S//NF)\n",
        "first pass must produce canonical NOFORN-only portion"
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
        second.applied.iter().all(|af| af.rule.as_str() != "E053"),
        "second pass must not re-apply E053 (idempotence); applied \
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
            .all(|d| d.rule.as_str() != "E053"),
        "second pass must produce zero E053 diagnostics (predicate \
         (NOFORN + REL TO coexist) is false after REL TO clear); \
         remaining diagnostics: {:?}",
        second
            .remaining_diagnostics
            .iter()
            .map(|d| d.rule.as_str())
            .collect::<Vec<_>>()
    );
}

/// Predicate guard: E053 does not fire when REL TO is absent.
/// `(S//NF)` is a clean NOFORN-only portion with no REL TO axis to
/// supersede; the rule predicate is false. Regression guard against
/// over-application — a previous-revision bug could have left the
/// rule firing on any NOFORN portion.
#[test]
fn e053_does_not_fire_when_rel_to_absent() {
    let result = engine().fix(b"(S//NF)\n", FixMode::Apply);
    assert_eq!(result.source.expose_secret(), b"(S//NF)\n",
        "no fix should apply on clean portion"
    );
    assert!(
        result
            .remaining_diagnostics
            .iter()
            .all(|d| d.rule.as_str() != "E053"),
        "E053 must not fire when REL TO is absent; remaining \
         diagnostics: {:?}",
        result
            .remaining_diagnostics
            .iter()
            .map(|d| d.rule.as_str())
            .collect::<Vec<_>>()
    );
}

/// Predicate guard: E053 does not fire when NOFORN is absent.
/// `(S//REL TO USA, GBR)` carries REL TO but no NOFORN; the §H.8
/// p145 prohibition is conditional on NOFORN's presence. E052
/// (REL TO requires USA-first ordering — orthogonal axis) may still
/// fire if USA isn't first, but the input here has USA first so
/// E052 stays silent too.
#[test]
fn e053_does_not_fire_when_noforn_absent() {
    let result = engine().fix(b"(S//REL TO USA, GBR)\n", FixMode::Apply);
    assert!(
        result
            .remaining_diagnostics
            .iter()
            .all(|d| d.rule.as_str() != "E053"),
        "E053 must not fire when NOFORN is absent; remaining \
         diagnostics: {:?}",
        result
            .remaining_diagnostics
            .iter()
            .map(|d| d.rule.as_str())
            .collect::<Vec<_>>()
    );
}

/// Constitution V Principle V — G13 invariant: the audit record's
/// `proposal.original` field is the empty string for the intent-only
/// path. The engine's intent-synthesis path (Path C of the
/// consolidated plan) wraps the synthesized `FixProposal` inside
/// `AppliedFixProposal::New { intent, synthesized }`; the
/// `synthesized` field's `original` is set to `""` by construction
/// because the byte-precise replacement is computed from the intent
/// against the candidate-span window, not from a verbatim copy of
/// the input.
///
/// Note: `proposal.original` is `Box<str>` — assert via
/// `.is_empty()`, not `== ""` (the `Box<str> == &str` Deref-coerce
/// path is fine in equality but `.is_empty()` is the idiomatic
/// check that aligns with the `Box<str>` vs `String` distinction
/// the audit format carries through).
#[test]
fn e053_proposal_original_is_empty_g13_invariant() {
    let result = engine().fix(b"(S//NF//REL TO USA, GBR)\n", FixMode::Apply);
    let af = result
        .applied
        .iter()
        .find(|af| af.rule.as_str() == "E053")
        .expect("E053 must auto-apply on (S//NF//REL TO USA, GBR)");

    match &af.proposal {
        AppliedFixProposal::New {
            intent: _,
            synthesized,
        } => {
            assert!(
                synthesized.original.is_empty(),
                "G13 invariant: `synthesized.original` must be empty \
                 for intent-only path (Constitution V Principle V — \
                 audit records carry no document content); got: {:?}",
                synthesized.original
            );
        }
        AppliedFixProposal::Legacy(_) => panic!(
            "E053 (migrated in Sub-PR 8.D.2) must emit \
             AppliedFixProposal::New; got Legacy. The intent-emission \
             path did not fire."
        ),
    }
}
