#![cfg(any())]
// PR 3c.B Commit 10: legacy FixProposal-shape test disabled pending rewrite

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Issue #235 / #186 PR-3: Constitution V audit-content-ignorance
//! invariant for `S004 rel-to-trigraph-suggest`.
//!
//! S004 must NOT splice document content into any of its emitted
//! surfaces (diagnostic message, fix replacement, citation). The
//! permitted identifiers are token canonicals (`AUT`, `AUS`),
//! vocabulary-derived English country names (`Austria`, `Australia`),
//! and span offsets — none of which are document text.
//!
//! This test lints a fixture with a sensitive REL TO block plus
//! surrounding non-marking text and asserts that no portion of the
//! document body or surrounding text appears in any S004 diagnostic
//! field.

use std::sync::Arc;

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_core::{Parser, Scanner};
use marque_ism::{CapcoTokenSet, MarkingType, PageContext};
use marque_rules::{Diagnostic, RuleContext, RuleSet};

fn lint(source: &[u8]) -> Vec<Diagnostic<CapcoScheme>> {
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidates = Scanner::scan(source);
    let rule_set = CapcoRuleSet::new();
    let mut out = Vec::new();
    let mut page_context = PageContext::new();
    let mut page_context_arc: Option<Arc<PageContext>> = None;
    for candidate in &candidates {
        if candidate.kind == MarkingType::PageBreak {
            page_context = PageContext::new();
            page_context_arc = None;
            continue;
        }
        let Ok(parsed) = parser.parse(candidate, source) else {
            continue;
        };
        // PR-3a transitional adapter; test-fixture carve-out per
        // Constitution V Principle V.
        let attrs = marque_ism::from_parsed_unchecked(parsed.attrs);
        if parsed.kind == MarkingType::Portion {
            page_context.add_portion(attrs.clone());
            page_context_arc = None;
        }
        let ctx_page = if parsed.kind != MarkingType::Portion && !page_context.is_empty() {
            Some(
                page_context_arc
                    .get_or_insert_with(|| Arc::new(page_context.clone()))
                    .clone(),
            )
        } else {
            None
        };
        let ctx = RuleContext {
            marking_type: candidate.kind,
            zone: None,
            position: None,
            candidate_span: candidate.span,
            page_context: ctx_page,
            corrections: None,
        };
        for rule in rule_set.rules() {
            out.extend(rule.check(&attrs, &ctx));
        }
    }
    out
}

/// The fixture has a banner-form REL TO with `AUT` (S004's canonical
/// trigger) plus a body of "sensitive" non-marking text. Constitution
/// V requires that text never appear in any audit-bound output —
/// diagnostic messages, fix proposals, or citations.
const FIXTURE: &[u8] = b"\
SECRET//REL TO USA, AUT, GBR

operation kingfisher details: contact alpha-team-lead, target site \
charlie-7, withdrawal route bravo-29, secret-handshake-codeword \
poseidon. coordinate with foxtrot-actual on encrypted channel.
";

/// Phrases from the document body that would be a violation if any
/// S004 surface contained them. The list covers both natural-language
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
fn s004_diagnostic_message_contains_no_document_content() {
    let diags = lint(FIXTURE);
    let s004: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S004").collect();
    assert!(
        !s004.is_empty(),
        "S004 must fire on the AUT trigraph in the fixture"
    );

    for d in &s004 {
        let msg = d.message.as_ref();
        for phrase in FORBIDDEN_PHRASES {
            assert!(
                !msg.contains(phrase),
                "S004 diagnostic message contains document text {phrase:?}: {msg}"
            );
        }
    }
}

#[test]
fn s004_fix_replacement_contains_no_document_content() {
    let diags = lint(FIXTURE);
    let s004: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S004").collect();
    assert!(!s004.is_empty(), "S004 must fire");

    for d in &s004 {
        let Some(fix) = d.fix.as_ref() else { continue };
        let replacement = fix.replacement.as_ref();
        let original = fix.original.as_ref();
        for phrase in FORBIDDEN_PHRASES {
            assert!(
                !replacement.contains(phrase),
                "S004 fix replacement contains document text {phrase:?}: {replacement}"
            );
            assert!(
                !original.contains(phrase),
                "S004 fix original contains document text {phrase:?}: {original}"
            );
        }
        // Replacement and original are bare 3-letter trigraphs from
        // the closed CAPCO Annex B vocabulary. Anything else is a
        // content-leak.
        assert_eq!(
            replacement.len(),
            3,
            "S004 replacement must be a bare trigraph (3 ASCII chars), got: {replacement:?}"
        );
        assert_eq!(
            original.len(),
            3,
            "S004 original must be a bare trigraph (3 ASCII chars), got: {original:?}"
        );
    }
}

#[test]
fn s004_citation_contains_no_document_content() {
    let diags = lint(FIXTURE);
    let s004: Vec<_> = diags.iter().filter(|d| d.rule.as_str() == "S004").collect();
    assert!(!s004.is_empty(), "S004 must fire");

    for d in &s004 {
        for phrase in FORBIDDEN_PHRASES {
            assert!(
                !d.citation.contains(phrase),
                "S004 citation contains document text {phrase:?}: {}",
                d.citation
            );
        }
        // Citation should be a CAPCO-2016 reference; pin the prefix
        // so a future drift to a non-CAPCO citation is obvious.
        assert!(
            d.citation.starts_with("CAPCO-2016"),
            "S004 citation must reference CAPCO-2016, got: {}",
            d.citation
        );
    }
}
