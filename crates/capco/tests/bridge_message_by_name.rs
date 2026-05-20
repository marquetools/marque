// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Tests for `CapcoScheme::message_by_name` — the engine-bridge
//! message hook that replaces opaque Debug-formatted `TokenId(N)`
//! strings with user-friendly prose.
//!
//! Two test layers:
//!
//! 1. **Unit tests** (`message_by_name_*`) — call the inherent method
//!    directly on `CapcoScheme` and assert (a) each known dyadic
//!    constraint name returns `Some(friendly_text)` and (b) unknown
//!    names return `None`.
//!
//! 2. **Integration tests** (`bridge_emits_friendly_message_*`) — run
//!    `Engine::lint` on a triggering input and assert that the emitted
//!    `Diagnostic.message` no longer contains `"TokenId"` (the
//!    tell-tale sign of the generic evaluator fallback).

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::{Engine, FixedClock};
use marque_ism::{CanonicalAttrs, MarkingType};

// ---------------------------------------------------------------------------
// Helpers
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

/// Empty `CanonicalAttrs` used for unit-level `message_by_name` calls where
/// the method body does not inspect `attrs`.
fn empty_attrs() -> CanonicalAttrs {
    CanonicalAttrs::default()
}

// ---------------------------------------------------------------------------
// Unit tests — message_by_name returns Some for known names
// ---------------------------------------------------------------------------

/// E015 dyadic Requires row: non-US classification requires a dissem control.
#[test]
fn message_by_name_e015_returns_some() {
    let scheme = CapcoScheme::new();
    let msg = scheme.message_by_name(
        "E015/non-us-requires-dissem",
        &empty_attrs(),
        MarkingType::Portion,
    );
    assert!(msg.is_some(), "E015 must return Some(...)");
    let text = msg.unwrap();
    assert!(
        !text.contains("TokenId"),
        "E015 message must not contain 'TokenId'; got: {text:?}"
    );
    assert!(
        text.contains("§H.7"),
        "E015 message must cite §H.7; got: {text:?}"
    );
}

/// E016 dyadic Conflicts row: JOINT ⊥ RESTRICTED.
#[test]
fn message_by_name_e016_returns_some() {
    let scheme = CapcoScheme::new();
    let msg = scheme.message_by_name(
        "E016/joint-conflicts-restricted",
        &empty_attrs(),
        MarkingType::Portion,
    );
    assert!(msg.is_some(), "E016 must return Some(...)");
    let text = msg.unwrap();
    assert!(
        !text.contains("TokenId"),
        "E016 message must not contain 'TokenId'; got: {text:?}"
    );
    assert!(
        text.contains("JOINT") && text.contains("RESTRICTED"),
        "E016 message must mention JOINT and RESTRICTED; got: {text:?}"
    );
}

/// E036 dyadic Conflicts row: JOINT ⊥ HCS.
#[test]
fn message_by_name_e036_returns_some() {
    let scheme = CapcoScheme::new();
    let msg = scheme.message_by_name(
        "E036/joint-conflicts-hcs",
        &empty_attrs(),
        MarkingType::Portion,
    );
    assert!(msg.is_some(), "E036 must return Some(...)");
    let text = msg.unwrap();
    assert!(
        !text.contains("TokenId"),
        "E036 message must not contain 'TokenId'; got: {text:?}"
    );
    assert!(
        text.contains("JOINT") && text.contains("HCS"),
        "E036 message must mention JOINT and HCS; got: {text:?}"
    );
}

/// capco/noforn-conflicts-rel-to dyadic Conflicts row (→ E053): NOFORN ⊥ REL TO.
#[test]
fn message_by_name_noforn_conflicts_rel_to_returns_some() {
    let scheme = CapcoScheme::new();
    let msg = scheme.message_by_name(
        "capco/noforn-conflicts-rel-to",
        &empty_attrs(),
        MarkingType::Portion,
    );
    assert!(
        msg.is_some(),
        "capco/noforn-conflicts-rel-to must return Some(...)"
    );
    let text = msg.unwrap();
    assert!(
        !text.contains("TokenId"),
        "noforn-conflicts-rel-to message must not contain 'TokenId'; got: {text:?}"
    );
    assert!(
        text.contains("NOFORN") && text.contains("REL TO"),
        "noforn-conflicts-rel-to message must mention NOFORN and REL TO; got: {text:?}"
    );
}

/// E037 dyadic Conflicts row: NODIS ⊥ EXDIS.
#[test]
fn message_by_name_e037_returns_some() {
    let scheme = CapcoScheme::new();
    let msg = scheme.message_by_name(
        "E037/nodis-conflicts-exdis",
        &empty_attrs(),
        MarkingType::Portion,
    );
    assert!(msg.is_some(), "E037 must return Some(...)");
    let text = msg.unwrap();
    assert!(
        !text.contains("TokenId"),
        "E037 message must not contain 'TokenId'; got: {text:?}"
    );
    assert!(
        text.contains("NODIS") && text.contains("EXDIS"),
        "E037 message must mention NODIS and EXDIS; got: {text:?}"
    );
}

/// E054 dyadic Conflicts row: RELIDO ⊥ NOFORN.
#[test]
fn message_by_name_e054_returns_some() {
    let scheme = CapcoScheme::new();
    let msg = scheme.message_by_name(
        "E054/relido-conflicts-noforn",
        &empty_attrs(),
        MarkingType::Portion,
    );
    assert!(msg.is_some(), "E054 must return Some(...)");
    let text = msg.unwrap();
    assert!(
        !text.contains("TokenId"),
        "E054 message must not contain 'TokenId'; got: {text:?}"
    );
    assert!(
        text.contains("RELIDO") && text.contains("NOFORN"),
        "E054 message must mention RELIDO and NOFORN; got: {text:?}"
    );
}

/// Unknown constraint names must return None so the bridge falls back to the
/// evaluator-generated message (which at least contains TokenId-free text for
/// Custom-arm constraints).
#[test]
fn message_by_name_returns_none_for_unknown_name() {
    let scheme = CapcoScheme::new();
    assert!(
        scheme
            .message_by_name("no-such-constraint", &empty_attrs(), MarkingType::Portion)
            .is_none(),
        "unknown name must return None"
    );
    // Custom-arm constraint names should also return None — they have
    // their own well-formed messages from the predicate body helpers.
    assert!(
        scheme
            .message_by_name(
                "E012/dual-classification",
                &empty_attrs(),
                MarkingType::Portion
            )
            .is_none(),
        "Custom-arm constraint E012 must return None (message lives in the predicate body)"
    );
}

// ---------------------------------------------------------------------------
// Integration tests — engine bridge emits friendly messages
// ---------------------------------------------------------------------------

/// E037 (NODIS ⊥ EXDIS): the diagnostic message must not contain `"TokenId"`.
///
/// Input `(S//NF//ND/XD)` carries both NODIS and EXDIS alongside NOFORN
/// (so E038 does not also fire for "no NOFORN"). The bridge must emit the
/// E037 diagnostic with the friendly message supplied by `message_by_name`.
#[test]
fn bridge_emits_friendly_message_for_e037() {
    let result = engine().lint(b"(S//NF//ND/XD)\n");
    let e037 = result
        .diagnostics
        .iter()
        .find(|d| d.rule.as_str() == "E037")
        .expect("E037 must fire on (S//NF//ND/XD)");

    assert!(
        !e037.message.contains("TokenId"),
        "E037 message must not contain 'TokenId' after message_by_name hook; \
         got: {:?}",
        e037.message
    );
    assert!(
        e037.message.contains("NODIS") && e037.message.contains("EXDIS"),
        "E037 message must mention NODIS and EXDIS; got: {:?}",
        e037.message
    );
}

/// E054 (RELIDO ⊥ NOFORN): the diagnostic message must not contain `"TokenId"`.
///
/// Input `(S//NF/RELIDO)` carries both NOFORN and RELIDO together.
/// The bridge must emit E054 with the friendly message from `message_by_name`.
#[test]
fn bridge_emits_friendly_message_for_e054() {
    let result = engine().lint(b"(S//NF/RELIDO)\n");
    let e054 = result
        .diagnostics
        .iter()
        .find(|d| d.rule.as_str() == "E054")
        .expect("E054 must fire on (S//NF/RELIDO)");

    assert!(
        !e054.message.contains("TokenId"),
        "E054 message must not contain 'TokenId' after message_by_name hook; \
         got: {:?}",
        e054.message
    );
    assert!(
        e054.message.contains("RELIDO") && e054.message.contains("NOFORN"),
        "E054 message must mention RELIDO and NOFORN; got: {:?}",
        e054.message
    );
}
