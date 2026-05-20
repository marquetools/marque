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

use marque_capco::scheme::{CAT_DISSEM, CAT_JOINT_CLASSIFICATION, CAT_NON_IC_DISSEM};
use marque_capco::{CapcoRuleSet, CapcoScheme};
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
    // (PR 3c.2.C C7 carved out `class-floor/*` + `sci-per-system/*` from
    // this rule; those are now bridge-resolved via row lookup. E012 stays
    // a predicate-body Custom row and returns None.)
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
// PR 3c.2.C C7 (R-C1): class-floor + sci-per-system bridge coverage
// ---------------------------------------------------------------------------

/// Class-floor catalog rows (27 rows; `class-floor/*` and `E058/*`
/// prefixes) must resolve to `MessageTemplate::ClassificationFloorViolated`
/// via the bridge's prefix dispatch. The category arg reflects the row's
/// `primary_kind` — SciSystem rows route to `CAT_SCI`, DissemControl rows
/// to `CAT_DISSEM`, etc.
#[test]
fn message_by_name_class_floor_hcs_comp_sub_returns_some() {
    let scheme = CapcoScheme::new();
    let msg = scheme.message_by_name(
        "class-floor/HCS-comp-sub",
        &empty_attrs(),
        MarkingType::Portion,
    );
    let msg = msg.expect("class-floor/HCS-comp-sub must return Some(...)");
    assert_eq!(
        msg.template(),
        MessageTemplate::ClassificationFloorViolated,
        "class-floor rows map to ClassificationFloorViolated; got {:?}",
        msg.template(),
    );
    // HCS-comp-sub's primary_kind is SciSystem → CAT_SCI.
    assert_eq!(
        msg.args().category,
        Some(marque_capco::scheme::CAT_SCI),
        "class-floor/HCS-comp-sub must identify the SCI axis (primary_kind=SciSystem); got {:?}",
        msg.args().category,
    );
}

/// E058-prefixed class-floor row (legacy-rule-replacement form) must
/// resolve identically to the `class-floor/`-prefixed form.
#[test]
fn message_by_name_class_floor_e058_cnwdi_returns_some() {
    let scheme = CapcoScheme::new();
    let msg = scheme.message_by_name(
        "E058/CNWDI-classification-floor",
        &empty_attrs(),
        MarkingType::Portion,
    );
    let msg = msg.expect("E058/CNWDI-classification-floor must return Some(...)");
    assert_eq!(msg.template(), MessageTemplate::ClassificationFloorViolated);
    // CNWDI's primary_kind is AeaMarking → CAT_AEA.
    assert_eq!(
        msg.args().category,
        Some(marque_capco::scheme::CAT_AEA),
        "E058/CNWDI must identify the AEA axis (primary_kind=AeaMarking); got {:?}",
        msg.args().category,
    );
}

/// SCI per-system catalog rows (5 rows; `sci-per-system/*` prefix)
/// must resolve to `MessageTemplate::RequiredByPresence` with
/// `CAT_SCI`.
#[test]
fn message_by_name_sci_per_system_hcs_o_returns_some() {
    let scheme = CapcoScheme::new();
    let msg = scheme.message_by_name(
        "sci-per-system/HCS-O-companions",
        &empty_attrs(),
        MarkingType::Portion,
    );
    let msg = msg.expect("sci-per-system/HCS-O-companions must return Some(...)");
    assert_eq!(
        msg.template(),
        MessageTemplate::RequiredByPresence,
        "sci-per-system rows map to RequiredByPresence; got {:?}",
        msg.template(),
    );
    assert_eq!(
        msg.args().category,
        Some(marque_capco::scheme::CAT_SCI),
        "sci-per-system/* must identify the SCI axis; got {:?}",
        msg.args().category,
    );
}

/// Unknown `class-floor/`-prefixed names fall through to `None` (no
/// catalog row match). The bridge's `message_by_name` only resolves
/// rows that actually exist in `CLASS_FLOOR_CATALOG`; a typo-class
/// name doesn't get a free template.
#[test]
fn message_by_name_class_floor_unknown_label_returns_none() {
    let scheme = CapcoScheme::new();
    assert!(
        scheme
            .message_by_name(
                "class-floor/no-such-row",
                &empty_attrs(),
                MarkingType::Portion
            )
            .is_none(),
        "unknown class-floor/* label must return None (no catalog match)"
    );
}

/// Class-floor citation_by_name dispatch (parallel to message_by_name)
/// must return the per-row `citation_typed`, not the
/// `[engine-internal]` sentinel.
#[test]
fn citation_by_name_class_floor_hcs_comp_sub_returns_typed_citation() {
    use marque_rules::{AuthoritativeSource, SectionLetter, capco};
    let scheme = CapcoScheme::new();
    let cite = scheme
        .citation_by_name("class-floor/HCS-comp-sub")
        .expect("class-floor/HCS-comp-sub must return Some(citation)");
    // HCS-comp-sub anchors at §H.4 p60 (SCI section start).
    assert_eq!(
        cite,
        capco(SectionLetter::H, 4, 60),
        "class-floor/HCS-comp-sub citation must be §H.4 p60; got {:?}",
        cite,
    );
    assert_eq!(cite.document, AuthoritativeSource::Capco2016);
}

/// SCI per-system citation_by_name dispatch must return the per-row
/// `citation_typed`.
#[test]
fn citation_by_name_sci_per_system_hcs_o_returns_typed_citation() {
    use marque_rules::{AuthoritativeSource, SectionLetter, capco};
    let scheme = CapcoScheme::new();
    let cite = scheme
        .citation_by_name("sci-per-system/HCS-O-companions")
        .expect("sci-per-system/HCS-O-companions must return Some(citation)");
    // HCS-O companions anchor at §H.4 p64.
    assert_eq!(
        cite,
        capco(SectionLetter::H, 4, 64),
        "sci-per-system/HCS-O-companions citation must be §H.4 p64; got {:?}",
        cite,
    );
    assert_eq!(cite.document, AuthoritativeSource::Capco2016);
}

/// Passthrough class-floor rows route to `AuthoritativeSource::EngineInternal`
/// (they reference marque-applied.md, not CAPCO-2016).
#[test]
fn citation_by_name_class_floor_passthrough_routes_to_engine_internal() {
    use marque_rules::AuthoritativeSource;
    let scheme = CapcoScheme::new();
    let cite = scheme
        .citation_by_name("class-floor/passthrough-BUR")
        .expect("class-floor/passthrough-BUR must return Some(citation)");
    // Passthrough rows reference marque-applied.md, not CAPCO. The
    // citation routes through AuthoritativeSource::EngineInternal so
    // Display renders `[engine-internal]`.
    assert_eq!(
        cite.document,
        AuthoritativeSource::EngineInternal,
        "passthrough rows must route to EngineInternal; got {:?}",
        cite.document,
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
