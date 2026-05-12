// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Sub-PR 8.D.3 — E010 (bare HCS requires compartment
//! suffix, §H.4 p62) intent-only migration engine-level tests.
//!
//! E010 migrates to `fix_intent: None` matching the E015/E016
//! conscious-defer pattern landed in Sub-PR 8.D.2 (PR #374). The
//! authoritative source (CAPCO-2016 §H.4 at
//! `crates/capco/docs/CAPCO-2016.md` lines 1393–1395) does NOT
//! mandate HCS-P as the default fill for bare HCS. The
//! Relationship(s) to Other Markings paragraph reads in relevant
//! part:
//!
//!   "When incorporating legacy material marked 'HCS' into a new
//!    product, re-mark the new document and associated portion
//!    according to the instructions in the HCS-O and HCS-P marking
//!    templates. However, legacy information previously marked HCS
//!    and transmitted via machine-to-machine processes may retain
//!    the HCS marking without requiring translation to either HCS-O
//!    or HCS-P."
//!
//! The classifier must read the HCS-O / HCS-P marking templates and
//! decide which applies — operational source (HCS-O) versus
//! analytical product (HCS-P). The decision depends on facts about
//! the underlying intelligence that marque cannot see. The prior
//! byte-surgical fix (`HCS → HCS-P` at 0.95 / 0.5 confidence) was a
//! UX heuristic, not a manual directive; per project memory
//! `feedback_pre_users_no_deprecation_phasing`, we drop the
//! heuristic rather than preserve it at higher confidence.
//!
//! These tests cover the engine-level behaviors that exercise the
//! intent-only path through the real `Engine::fix` surface (the
//! inline `lint_banner` / `lint_portion` helpers inside
//! `crates/capco/src/rules.rs::tests` see a different
//! `CapcoScheme` crate identity than `marque-engine`):
//!
//! - Diagnostic emission on the portion and banner forms with
//!   `fix.is_none()` AND `fix_intent.is_none()` (conscious-defer
//!   shape — neither path produces an auto-applied fix).
//! - Predicate guards: rule does not fire when HCS-P or HCS-O is
//!   present (no bare HCS to flag).
//! - Idempotence on auto-apply: `Engine::fix` produces no
//!   `AppliedFix` entry for E010 because the predicate has no fix
//!   path; the input string is unchanged.
//!
//! Scope note: the Stage-4 target is a `Severity::Suggest`
//! companion diagnostic pair ("did you mean HCS-O?" / "did you mean
//! HCS-P?") — the same Reject-with-suggest pattern named for E015
//! / E036. No auto-applied fix exists for this combination today.

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

/// E010 diagnostic emission on the portion form. `(TS//HCS)` carries
/// a bare HCS with no compartment suffix; the rule fires with
/// `fix.is_none()` and `fix_intent.is_none()` (conscious-defer). The
/// diagnostic message identifies HCS-O, HCS-P, and the combined
/// HCS-O-P form as the candidate compartments so the classifier
/// knows which templates to consult (CAPCO-2016 §H.4 p63 line 1406:
/// "the portion mark must include either HCS-P, HCS-O, or
/// HCS-O-P, if applicable").
#[test]
fn e010_emits_diagnostic_on_bare_hcs_in_portion() {
    let result = engine().lint(b"(TS//HCS)\n");
    let e010: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "E010")
        .collect();

    assert_eq!(
        e010.len(),
        1,
        "E010 must fire exactly once on (TS//HCS); diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        e010[0].fix.is_none(),
        "E010 must not carry a legacy FixProposal post-migration; \
         got: {:?}",
        e010[0].fix
    );
    assert!(
        e010[0].fix_intent.is_none(),
        "E010 must consciously decline to emit a FixIntent \
         (HCS-O vs HCS-P vs HCS-O-P is a classifier decision per \
         §H.4); got: {:?}",
        e010[0].fix_intent
    );
    assert!(
        e010[0].message.contains("HCS-O for"),
        "E010 message must reference HCS-O as a distinct form so \
         the classifier knows which template to consult; got: {:?}",
        e010[0].message
    );
    assert!(
        e010[0].message.contains("HCS-P"),
        "E010 message must reference HCS-P so the classifier knows \
         which template to consult; got: {:?}",
        e010[0].message
    );
    assert!(
        e010[0].message.contains("HCS-O-P"),
        "E010 message must reference HCS-O-P (the combined \
         compartment form) per CAPCO-2016 §H.4 p63 line 1406; \
         got: {:?}",
        e010[0].message
    );
}

/// E010 diagnostic emission on the banner form. Same conscious-
/// defer shape as the portion variant — diagnostic fires, no auto-
/// fix.
#[test]
fn e010_emits_diagnostic_on_bare_hcs_in_banner() {
    let result = engine().lint(b"TOP SECRET//HCS\n");
    let e010: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.as_str() == "E010")
        .collect();

    assert_eq!(
        e010.len(),
        1,
        "E010 must fire exactly once on TOP SECRET//HCS; \
         diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        e010[0].fix.is_none(),
        "E010 must not carry a legacy FixProposal post-migration; \
         got: {:?}",
        e010[0].fix
    );
    assert!(
        e010[0].fix_intent.is_none(),
        "E010 must consciously decline to emit a FixIntent \
         (HCS-O vs HCS-P vs HCS-O-P is a classifier decision per \
         §H.4); got: {:?}",
        e010[0].fix_intent
    );
    assert!(
        e010[0].message.contains("HCS-O for"),
        "E010 message must reference HCS-O as a distinct form so \
         the classifier knows which template to consult; got: {:?}",
        e010[0].message
    );
    assert!(
        e010[0].message.contains("HCS-P"),
        "E010 message must reference HCS-P so the classifier knows \
         which template to consult; got: {:?}",
        e010[0].message
    );
    assert!(
        e010[0].message.contains("HCS-O-P"),
        "E010 message must reference HCS-O-P (the combined \
         compartment form) per CAPCO-2016 §H.4 p63 line 1406; \
         got: {:?}",
        e010[0].message
    );
}

/// Predicate guard: E010 does not fire when HCS-P is present.
/// `(TS//HCS-P)` is a well-formed marking with the analytical-product
/// compartment; no bare HCS to flag.
#[test]
fn e010_does_not_fire_when_hcs_p_present() {
    let result = engine().lint(b"(TS//HCS-P)\n");
    assert!(
        result.diagnostics.iter().all(|d| d.rule.as_str() != "E010"),
        "E010 must not fire when HCS-P is present; diagnostics: {:?}",
        result.diagnostics
    );
}

/// Predicate guard: E010 does not fire when HCS-O is present.
/// `(TS//HCS-O)` is a well-formed marking with the operational-
/// source compartment; no bare HCS to flag.
#[test]
fn e010_does_not_fire_when_hcs_o_present() {
    let result = engine().lint(b"(TS//HCS-O)\n");
    assert!(
        result.diagnostics.iter().all(|d| d.rule.as_str() != "E010"),
        "E010 must not fire when HCS-O is present; diagnostics: {:?}",
        result.diagnostics
    );
}

/// Idempotence on auto-apply: `Engine::fix` produces no `AppliedFix`
/// entry for E010 because the rule has neither `fix` nor `fix_intent`
/// populated. The input string is unchanged. This is the load-bearing
/// invariant of the conscious-defer pattern: a previous-revision bug
/// could have left the auto-pick `HCS → HCS-P` fix in place, corrupt-
/// ing the audit log with a classifier decision the engine cannot
/// make.
#[test]
fn e010_idempotent_no_auto_fix_application() {
    let result = engine().fix(b"(TS//HCS)\n", FixMode::Apply);

    assert_eq!(
        result.source,
        b"(TS//HCS)\n",
        "no fix should apply on bare HCS (conscious-defer); \
         got: {:?}",
        std::str::from_utf8(&result.source).unwrap_or("<non-utf8>")
    );
    assert!(
        result
            .applied
            .iter()
            .all(|af| af.proposal.rule.as_str() != "E010"),
        "E010 must not produce an AppliedFix entry post-migration \
         (no fix path); applied rules: {:?}",
        result
            .applied
            .iter()
            .map(|af| af.proposal.rule.as_str())
            .collect::<Vec<_>>()
    );
}
