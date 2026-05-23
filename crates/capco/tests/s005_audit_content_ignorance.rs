// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! S005 (`capco:page.dissem.rel-to-uncertain-reduction`) Constitution
//! V Principle V (G13) audit-content-ignorance pin ported from
//! `crates/capco/src/_disabled_tests.rs` per issue #722.
//!
//! Pair file to `s004_audit_content_ignorance.rs` — same shape, same
//! discipline, different rule.
//!
//! # Source test ported
//!
//! - `s005_audit_content_ignorance_no_user_content_in_message` —
//!   the legacy test asserted `!s005.message.contains("Operation
//!   Confidential")` to verify the rule did not splice surrounding
//!   document text into the diagnostic message.
//!
//! # PR 3c.2.C C5 reshape
//!
//! Under the closed-template `Message` shape and the typed `Citation`
//! struct, the message- and citation-leak channels are now
//! STRUCTURALLY closed:
//!
//! - `Diagnostic.message: Message` carries `MessageTemplate` (closed
//!   enum) + `MessageArgs` (closed struct of typed identifiers); raw
//!   bytes are unrepresentable in either field.
//! - `Diagnostic.citation: Citation` is a typed struct; the Display
//!   impl emits a fixed `§X.Y pN` shape or a `[<source>]` sentinel,
//!   never a free-form string.
//!
//! S005 specifically emits `MessageTemplate::NonCanonicalOrder` with
//! `category=Some(CAT_REL_TO)` and no other args populated:
//! `analyze_uncertain_reduction` in
//! `crates/capco/src/rules/rel_to_uncertainty.rs` (called by
//! `RelToOpaqueUncertainReductionSuggestRule::check`) drops every
//! runtime value (`let _ = (x, state, expected_str, other_str);`)
//! before the `Message::new` call. The legacy
//! `message.contains("...")` assertion was an attempt to verify
//! the same property under a `Box<str>` message shape; under the
//! closed-template shape the assertion is structurally subsumed —
//! S005's message body cannot mechanically carry document text
//! because the closed-args record has no `String` field.
//!
//! The test purpose strengthens: instead of grepping prose for
//! sentinels that *could have* leaked, we verify the closed-set
//! type discipline holds (template + args structurally precludes
//! document content) AND that no audit-bound rendered surface
//! (message template label, citation render, text_correction
//! replacement) carries the sentinel.
//!
//! # Authority
//!
//! CAPCO-2016 §H.8 + §D.2 Table 3 rule 21 (REL TO atom-semantics
//! intersection — the rule's substantive trigger). Re-verified
//! against `crates/capco/docs/CAPCO-2016.md` at authorship per
//! Constitution VIII.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::{Diagnostic, MessageTemplate};

const S005_PREDICATE: &str = "page.dissem.rel-to-uncertain-reduction";

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

/// Fixture wrapping the S005 trigger (RSMA opaque-uncertain with
/// non-X portions whose atom intersection includes GBR) in
/// distinctive non-marking prose. The prose sentinels are kept in
/// the source so future regressions (an accidental free-form channel
/// added back to `Message` or to any audit-bound rendered surface)
/// would surface via this fixture.
const FIXTURE: &[u8] = b"\
Document subject: \"Operation Confidential\"
(S//REL TO USA, GBR, RSMA)
(S//REL TO USA, AUS, GBR)
SECRET//NOFORN
";

/// Sentinels from the document body that MUST NOT appear in any
/// audit-bound output from S005.
const FORBIDDEN_PHRASES: &[&str] = &[
    "Operation Confidential",
    "Document subject",
    "operation confidential",
    "document subject",
];

fn lint() -> Vec<Diagnostic<CapcoScheme>> {
    engine().lint(FIXTURE).diagnostics
}

/// Verify S005 fires on the fixture (so the audit-content-ignorance
/// assertions below are meaningful — a silent rule wouldn't carry
/// any audit surface to leak through).
#[test]
fn s005_fires_on_audit_content_ignorance_fixture() {
    let diags = lint();
    let s005: Vec<_> = diags
        .iter()
        .filter(|d| d.rule.predicate_id() == S005_PREDICATE)
        .collect();
    assert!(
        !s005.is_empty(),
        "S005 must fire on the RSMA opaque-uncertain fixture so the \
         audit-content-ignorance pins below are meaningful; got: {diags:?}",
    );
}

/// PR 3c.2.C C5: `Diagnostic.message` carries `MessageTemplate`
/// (closed enum) + `MessageArgs` (closed struct of typed
/// identifiers); raw bytes are unrepresentable. Verify the structural
/// property: S005's template label is a fixed `&'static str` from
/// the closed enum, and the rendered label contains no fixture
/// prose. The compile-fail doctests in `crates/rules/src/message.rs`
/// pin the impossibility of a free-form `MessageArgs` field; this
/// runtime check is informational.
#[test]
fn s005_message_template_label_carries_no_document_content() {
    let diags = lint();
    let s005: Vec<_> = diags
        .iter()
        .filter(|d| d.rule.predicate_id() == S005_PREDICATE)
        .collect();
    assert!(!s005.is_empty(), "S005 must fire");

    for d in &s005 {
        let template_label = d.message.template().as_str();
        for phrase in FORBIDDEN_PHRASES {
            assert!(
                !template_label.contains(phrase),
                "S005 template label {template_label:?} contains \
                 document text {phrase:?} — the closed-template \
                 Message shape should make this impossible by \
                 construction",
            );
        }
        // S005 emits NonCanonicalOrder per the rule body. Pin the
        // template variant so a regression that switches to a
        // free-form-prone template would surface here.
        assert_eq!(
            d.message.template(),
            MessageTemplate::NonCanonicalOrder,
            "S005 must emit MessageTemplate::NonCanonicalOrder; got: {:?}",
            d.message.template(),
        );
        // Args carry only `Option<TokenId>` / `Option<CategoryId>` /
        // `Option<Span>` etc. — no `String` field. Inspect to silence
        // dead-code warnings; the compile-fail doctest at
        // `crates/rules/src/message.rs` is the type-level pin.
        let _ = d.message.args();
    }
}

/// PR 3c.2.C C5: `Diagnostic.citation` is a typed `Citation` struct.
/// The Display impl emits a fixed `§<L>.<sub> p<N>` shape for CAPCO
/// sources (no free-form prose) or a `[<source>]` sentinel for
/// non-CAPCO sources. Verify the rendered form contains no fixture
/// prose and starts with the canonical `§` lead-in.
#[test]
fn s005_citation_render_carries_no_document_content() {
    let diags = lint();
    let s005: Vec<_> = diags
        .iter()
        .filter(|d| d.rule.predicate_id() == S005_PREDICATE)
        .collect();
    assert!(!s005.is_empty(), "S005 must fire");

    for d in &s005 {
        let rendered = format!("{}", d.citation);
        for phrase in FORBIDDEN_PHRASES {
            assert!(
                !rendered.contains(phrase),
                "S005 citation render {rendered:?} contains document \
                 text {phrase:?}",
            );
        }
        // Pin the canonical `§…` lead-in (S005 is a CAPCO-sourced
        // rule; the citation must render as a typed §-reference).
        assert!(
            rendered.starts_with("§"),
            "S005 citation must render as a typed CAPCO §-reference, \
             got: {rendered}",
        );
    }
}

/// S005 carries no `text_correction` (the rule consciously declines
/// to emit a fix per the Stage-4 admonition-channel defer; see
/// `crates/capco/tests/s005_pagefinalization.rs::
/// s005_emits_neither_fix_nor_text_correction_pending_stage4_admonition`).
/// The absence of a text-correction payload means the
/// bytes-carrying audit surface (`TextCorrection.replacement`) is
/// unreachable from S005 today; verify the absence holds and the
/// audit surface stays content-clean by construction.
#[test]
fn s005_carries_no_text_correction_so_replacement_channel_is_unreachable() {
    let diags = lint();
    let s005: Vec<_> = diags
        .iter()
        .filter(|d| d.rule.predicate_id() == S005_PREDICATE)
        .collect();
    assert!(!s005.is_empty(), "S005 must fire");

    for d in &s005 {
        assert!(
            d.text_correction.is_none(),
            "S005 must not carry a text_correction (Stage-4 \
             admonition-channel defer); got: {:?}",
            d.text_correction,
        );
    }
}
