// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Issue #545: Constitution V audit-content-ignorance invariant for
//! `capco:portion.fgi.ownership-trigraph-suggest`.
//!
//! The rule must NOT splice document content into any of its emitted
//! surfaces (diagnostic message, fix replacement, citation). Permitted
//! identifiers are token canonicals (sovereign trigraphs from the
//! CAPCO Annex B vocabulary), category IDs (the typed
//! `CAT_FGI_MARKER`), and span offsets — none of which are document
//! text.
//!
//! # Structural closure
//!
//! Post PR 3c.2.C C5 (the closed-template `Message` shape + typed
//! `Citation` struct), the message- and citation-leak channels are
//! STRUCTURALLY closed:
//!
//! - `Diagnostic.message: Message` carries `MessageTemplate` (closed
//!   enum) + `MessageArgs` (closed struct of typed identifiers);
//!   raw bytes are unrepresentable.
//! - `Diagnostic.citation: Citation` is a typed struct; the Display
//!   impl emits a fixed `§X.Y pN` shape or a `[<source>]` sentinel,
//!   never a free-form string.
//!
//! The test purpose verifies the closed-set type discipline holds.
//! `TextCorrection.replacement` is the one bytes-carrying surface;
//! we assert it is a bare 3-letter trigraph from the CAPCO Annex B
//! vocabulary (length-3 enforced).
//!
//! Test-fixture carve-out per Constitution V Principle V — this
//! file reimplements the engine's lint loop locally to drive the
//! rule against a synthesized fixture. The `Severity::Off` gate
//! mirror at line 88 reproduces the production engine behavior
//! (per the PR #695 / issue #672 fix on `rules_us1.rs`'s twin
//! lint helper) so an opt-in rule cannot pollute the post-collection
//! filter window and mask a real audit-content leak.

use std::sync::Arc;

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_core::{Parser, Scanner};
use marque_ism::{CanonicalAttrs, CapcoTokenSet, MarkingType};
use marque_rules::{Diagnostic, RuleContext, RuleSet, Severity};
use marque_scheme::MarkingScheme;

const RULE_PREDICATE: &str = "portion.fgi.ownership-trigraph-suggest";

/// Default per-page portion capacity. Matches the engine's accumulator
/// pre-size (`crates/engine/src/engine.rs::DEFAULT_PORTIONS_CAPACITY`)
/// so test fixtures exercise the same Vec-growth schedule the production
/// engine pays.
const DEFAULT_PORTIONS_CAPACITY: usize = 8;

fn lint(source: &[u8]) -> Vec<Diagnostic<CapcoScheme>> {
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidates = Scanner::scan(source);
    let rule_set = CapcoRuleSet::new();
    let scheme = CapcoScheme::new();
    let mut out = Vec::new();
    let mut page_portions: Vec<CanonicalAttrs> = Vec::with_capacity(DEFAULT_PORTIONS_CAPACITY);
    let mut page_portions_arc: Option<Arc<Box<[CanonicalAttrs]>>> = None;
    for candidate in &candidates {
        if matches!(
            candidate.kind,
            MarkingType::PageBreak | MarkingType::PageFinalization
        ) {
            page_portions = Vec::with_capacity(DEFAULT_PORTIONS_CAPACITY);
            page_portions_arc = None;
            continue;
        }
        let Ok(parsed) = parser.parse(candidate, source) else {
            continue;
        };
        let attrs = scheme.canonicalize(parsed.attrs);
        if parsed.kind == MarkingType::Portion {
            page_portions.push(attrs.clone());
            page_portions_arc = None;
        }
        let ctx_page = if parsed.kind != MarkingType::Portion && !page_portions.is_empty() {
            Some(
                page_portions_arc
                    .get_or_insert_with(|| Arc::new(page_portions.clone().into_boxed_slice()))
                    .clone(),
            )
        } else {
            None
        };
        let ctx = RuleContext::new(candidate.kind, candidate.span).with_page_portions(ctx_page);
        for rule in rule_set.rules() {
            // Issue #672 — mirror the engine's `Severity::Off` gate
            // (see `crates/engine/src/engine.rs` lint loop). Same
            // defensive filter as the parallel S004 test at
            // `s004_audit_content_ignorance.rs:100-114` — an opt-in
            // rule (e.g., a future Off-severity Suggest co-firing on
            // this rule's trigger span) cannot pollute the
            // post-collection filter window and mask a real
            // audit-content leak. Constitution V Principle V —
            // `Severity::Off` is a non-firing state, NOT a
            // suppression; the engine skips the rule loop body
            // entirely, and this test must match.
            if rule.default_severity() == Severity::Off {
                continue;
            }
            out.extend(rule.check(&attrs, &ctx));
        }
    }
    out
}

/// Fixture: an FGI portion with the unregistered shape-admitted
/// trigraph `XXZ` (3 bytes, no corpus-prior neighbor within margin
/// — observed empirically during implementation). Drives the
/// rule's emit path while keeping the fixture content-rich so any
/// audit-content leak is detectable.
///
/// `(C//FGI XXZ)` — Confidential US classification + FGI ownership
/// list containing an unregistered 3-letter shape-admitted trigraph.
/// The classification level is irrelevant to the rule's predicate;
/// CONFIDENTIAL keeps the fixture minimal.
///
/// The fixture wraps the portion in "sensitive" non-marking prose;
/// the test asserts that prose never appears in any audit-bound
/// output.
const FIXTURE: &[u8] = b"\
(C//FGI XXZ)

operation kingfisher details: contact alpha-team-lead, target site \
charlie-7, withdrawal route bravo-29, secret-handshake-codeword \
poseidon. coordinate with foxtrot-actual on encrypted channel.
";

/// Phrases from the document body that would be a violation if any
/// emitted surface contained them. The list covers natural-language
/// fragments and keywords that look like markings (e.g.,
/// `kingfisher`, `poseidon`) but are document content.
const FORBIDDEN_PHRASES: &[&str] = &[
    "operation",
    "kingfisher",
    "alpha-team-lead",
    "target site",
    "charlie-7",
    "withdrawal route",
    "bravo-29",
    "secret-handshake",
    "poseidon",
    "foxtrot-actual",
    "encrypted channel",
];

#[test]
fn diagnostic_message_carries_only_closed_template() {
    // The message channel is structurally closed by `MessageTemplate`
    // + `MessageArgs`. The runtime check verifies the template label
    // is a fixed `&'static str` from the closed enum (no document
    // bytes can flow through).
    let diags = lint(FIXTURE);
    let hits: Vec<_> = diags
        .iter()
        .filter(|d| d.rule.predicate_id() == RULE_PREDICATE)
        .collect();
    assert!(
        !hits.is_empty(),
        "the rule must fire on the XXZ trigraph in the fixture"
    );

    for d in &hits {
        let template_label = d.message.template().as_str();
        for phrase in FORBIDDEN_PHRASES {
            assert!(
                !template_label.contains(phrase),
                "template label {template_label:?} contains document text {phrase:?}"
            );
        }
        let _ = d.message.args();
    }
}

#[test]
fn text_correction_replacement_contains_no_document_content() {
    // `TextCorrection.replacement: SmolStr` is the one bytes-carrying
    // surface on the diagnostic; Constitution V requires it to be a
    // permitted identifier from the closed CAPCO vocabulary (an
    // Annex B trigraph the corpus-prior + edit-distance machinery
    // chose as the canonical replacement).
    //
    // Some emitted diagnostics carry no `text_correction` (the
    // no-fix branch when no neighbor clears the margin); for those,
    // the test is vacuously satisfied.
    let diags = lint(FIXTURE);
    let hits: Vec<_> = diags
        .iter()
        .filter(|d| d.rule.predicate_id() == RULE_PREDICATE)
        .collect();
    assert!(!hits.is_empty(), "the rule must fire");

    for d in &hits {
        let Some(tc) = d.text_correction.as_ref() else {
            continue;
        };
        let replacement = tc.replacement.as_str();
        for phrase in FORBIDDEN_PHRASES {
            assert!(
                !replacement.contains(phrase),
                "text_correction replacement {replacement:?} contains document text {phrase:?}"
            );
        }
        // The replacement is a bare 3-letter trigraph from the closed
        // CAPCO Annex B vocabulary. Anything else is a content-leak.
        // The calibration table currently includes only 3-letter
        // trigraphs as candidates; EU (2-byte) is a registered FGI
        // ownership code but has no calibrated near-neighbor in the
        // current table. If the prior table is ever extended to
        // include 2-byte candidates, this assertion will need to be
        // relaxed.
        assert_eq!(
            replacement.len(),
            3,
            "replacement must be a bare trigraph (3 ASCII chars), got: {replacement:?}"
        );
        assert!(
            replacement
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()),
            "replacement must be ASCII upper/digit (CAPCO Annex B \
             vocabulary), got: {replacement:?}"
        );
    }
}

#[test]
fn citation_carries_only_typed_capco_anchor() {
    // `Diagnostic.citation` is a typed `Citation` struct; the Display
    // impl emits a fixed `§<L>.<sub> p<N>` shape for CAPCO sources
    // (no free-form prose). The runtime check sweeps the rendered
    // form for any document-content leakage.
    let diags = lint(FIXTURE);
    let hits: Vec<_> = diags
        .iter()
        .filter(|d| d.rule.predicate_id() == RULE_PREDICATE)
        .collect();
    assert!(!hits.is_empty(), "the rule must fire");

    for d in &hits {
        let rendered = format!("{}", d.citation);
        for phrase in FORBIDDEN_PHRASES {
            assert!(
                !rendered.contains(phrase),
                "citation render {rendered:?} contains document text {phrase:?}"
            );
        }
        // Pin the §-citation anchor: §H.7 p122 (FGI ownership
        // grammar) per `FgiOwnershipTrigraphSuggestRule`'s emit body.
        assert!(
            rendered.starts_with("§"),
            "citation must render as a typed CAPCO §-reference, got: {rendered}"
        );
        assert!(
            rendered.contains("§H.7 p122"),
            "citation must anchor at §H.7 p122 (FGI ownership grammar); got: {rendered}"
        );
    }
}

#[test]
fn diagnostic_span_offsets_are_byte_locators_not_content_payloads() {
    // Span offsets are byte locators into the source buffer, not
    // content payloads. The G13 invariant is satisfied as long as
    // the diagnostic surface carries no document-content bytes
    // — which the three tests above already verify. This test
    // pins the structural property: the span field is `Span`
    // (`{start, end}`-typed integer pair), not a `&str` or
    // `Vec<u8>`. Compile-time type discipline carries this;
    // the runtime check is informational.
    let diags = lint(FIXTURE);
    let hits: Vec<_> = diags
        .iter()
        .filter(|d| d.rule.predicate_id() == RULE_PREDICATE)
        .collect();
    assert!(!hits.is_empty(), "the rule must fire");
    for d in &hits {
        // Sanity: span is a valid range over the source buffer.
        assert!(
            d.span.end > d.span.start,
            "diagnostic span must be non-empty, got {}..{}",
            d.span.start,
            d.span.end,
        );
        assert!(
            d.span.end <= FIXTURE.len(),
            "diagnostic span must lie within the fixture; got end={}, fixture_len={}",
            d.span.end,
            FIXTURE.len(),
        );
    }
}
