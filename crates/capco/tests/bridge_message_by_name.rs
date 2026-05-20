// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Tests for `CapcoScheme::message_by_name` — the engine-bridge
//! message hook.
//!
//! PR 3c.2.C C5 reshape: `message_by_name` now returns a closed
//! `Message` (template + args), not a free-form string. The bridge
//! invariant that this file pins is structurally enforced by the
//! closed-args / closed-template invariants in `crates/rules/src/
//! message.rs`:
//!
//! - **No `TokenId` debug leakage** — `MessageArgs` carries
//!   `Option<TokenId>` and `Option<CategoryId>` only; raw bytes /
//!   debug strings are unrepresentable by construction.
//! - **No free-form prose** — `MessageTemplate` is a closed enum;
//!   the engine emits the variant label, never a `format!`-built
//!   sentence.
//!
//! Two test layers:
//!
//! 1. **Unit tests** (`message_by_name_*`) — call the inherent method
//!    directly on `CapcoScheme` and assert (a) each known dyadic
//!    constraint name returns `Some(message)` with the expected
//!    template + category and (b) unknown names return `None`.
//!
//! 2. **Integration tests** (`bridge_emits_typed_message_*`) — run
//!    `Engine::lint` on a triggering input and assert that the
//!    emitted `Diagnostic.message` carries the expected closed-set
//!    identification (`MessageTemplate` + `MessageArgs.category`)
//!    rather than a generic fallback.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_capco::scheme::{
    CAT_DISSEM, CAT_JOINT_CLASSIFICATION, CAT_NON_IC_DISSEM,
};
use marque_config::Config;
use marque_engine::{Engine, FixedClock};
use marque_ism::{CanonicalAttrs, MarkingType};
use marque_rules::MessageTemplate;

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
// Unit tests — message_by_name returns Some(Message) with expected shape
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
    let msg = msg.expect("E015 must return Some(...)");
    assert_eq!(
        msg.template(),
        MessageTemplate::RequiredByPresence,
        "E015 maps to the RequiredByPresence template; got {:?}",
        msg.template(),
    );
    assert_eq!(
        msg.args().category,
        Some(CAT_DISSEM),
        "E015 must identify the dissem axis; got {:?}",
        msg.args().category,
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
    let msg = msg.expect("E016 must return Some(...)");
    assert_eq!(
        msg.template(),
        MessageTemplate::ConflictsWith,
        "E016 maps to the ConflictsWith template; got {:?}",
        msg.template(),
    );
    assert_eq!(
        msg.args().category,
        Some(CAT_JOINT_CLASSIFICATION),
        "E016 must identify the JOINT classification axis; got {:?}",
        msg.args().category,
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
    let msg = msg.expect("E036 must return Some(...)");
    assert_eq!(
        msg.template(),
        MessageTemplate::ConflictsWith,
        "E036 maps to the ConflictsWith template; got {:?}",
        msg.template(),
    );
    assert_eq!(
        msg.args().category,
        Some(CAT_JOINT_CLASSIFICATION),
        "E036 must identify the JOINT classification axis; got {:?}",
        msg.args().category,
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
    let msg = msg.expect("capco/noforn-conflicts-rel-to must return Some(...)");
    assert_eq!(
        msg.template(),
        MessageTemplate::ConflictsWith,
        "noforn-conflicts-rel-to maps to the ConflictsWith template; got {:?}",
        msg.template(),
    );
    assert_eq!(
        msg.args().category,
        Some(CAT_DISSEM),
        "noforn-conflicts-rel-to must identify the dissem axis; got {:?}",
        msg.args().category,
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
    let msg = msg.expect("E037 must return Some(...)");
    assert_eq!(
        msg.template(),
        MessageTemplate::ConflictsWith,
        "E037 maps to the ConflictsWith template; got {:?}",
        msg.template(),
    );
    assert_eq!(
        msg.args().category,
        Some(CAT_NON_IC_DISSEM),
        "E037 must identify the non-IC dissem axis; got {:?}",
        msg.args().category,
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
    let msg = msg.expect("E054 must return Some(...)");
    assert_eq!(
        msg.template(),
        MessageTemplate::ConflictsWith,
        "E054 maps to the ConflictsWith template; got {:?}",
        msg.template(),
    );
    assert_eq!(
        msg.args().category,
        Some(CAT_DISSEM),
        "E054 must identify the dissem axis; got {:?}",
        msg.args().category,
    );
}

/// Unknown constraint names must return None so the bridge falls back to the
/// engine's generic-template path (which still emits a closed Message —
/// the engine's fallback uses `MessageTemplate::ConflictsWith` with empty
/// args per `Engine::bridge_constraint_diagnostic`, but the per-row
/// identification is lost).
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
// Integration tests — engine bridge emits typed Message with expected shape
// ---------------------------------------------------------------------------

/// E037 (NODIS ⊥ EXDIS): the diagnostic must carry the
/// `ConflictsWith` template + `CAT_NON_IC_DISSEM` category from
/// `message_by_name`.
///
/// Input `(S//NF//ND/XD)` carries both NODIS and EXDIS alongside NOFORN
/// (so E038 does not also fire for "no NOFORN"). The bridge must emit
/// E037 with the typed message supplied by `message_by_name`.
#[test]
fn bridge_emits_typed_message_for_e037() {
    let result = engine().lint(b"(S//NF//ND/XD)\n");
    let e037 = result
        .diagnostics
        .iter()
        .find(|d| d.rule.as_str() == "E037")
        .expect("E037 must fire on (S//NF//ND/XD)");

    assert_eq!(
        e037.message.template(),
        MessageTemplate::ConflictsWith,
        "E037 must carry the ConflictsWith template after the message_by_name hook; \
         got: {:?}",
        e037.message.template(),
    );
    assert_eq!(
        e037.message.args().category,
        Some(CAT_NON_IC_DISSEM),
        "E037 must identify the non-IC dissem axis; got: {:?}",
        e037.message.args().category,
    );
}

/// E054 (RELIDO ⊥ NOFORN): the diagnostic must carry the
/// `ConflictsWith` template + `CAT_DISSEM` category.
///
/// Input `(S//NF/RELIDO)` carries both NOFORN and RELIDO together.
/// The bridge must emit E054 with the typed message from
/// `message_by_name`.
#[test]
fn bridge_emits_typed_message_for_e054() {
    let result = engine().lint(b"(S//NF/RELIDO)\n");
    let e054 = result
        .diagnostics
        .iter()
        .find(|d| d.rule.as_str() == "E054")
        .expect("E054 must fire on (S//NF/RELIDO)");

    assert_eq!(
        e054.message.template(),
        MessageTemplate::ConflictsWith,
        "E054 must carry the ConflictsWith template after the message_by_name hook; \
         got: {:?}",
        e054.message.template(),
    );
    assert_eq!(
        e054.message.args().category,
        Some(CAT_DISSEM),
        "E054 must identify the dissem axis; got: {:?}",
        e054.message.args().category,
    );
}
