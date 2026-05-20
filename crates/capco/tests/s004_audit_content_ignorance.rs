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
use marque_ism::{CanonicalAttrs, CapcoTokenSet, MarkingType};
use marque_rules::{Diagnostic, RuleContext, RuleSet};

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
    let mut out = Vec::new();
    // PR 6c (T069): inline Vec<CanonicalAttrs> + Arc<Box<[_]>> snapshot
    // mirrors the post-retirement engine accumulator shape.
    let mut page_portions: Vec<CanonicalAttrs> = Vec::with_capacity(DEFAULT_PORTIONS_CAPACITY);
    let mut page_portions_arc: Option<Arc<Box<[CanonicalAttrs]>>> = None;
    for candidate in &candidates {
        // PageBreak is scanner-emitted; PageFinalization is
        // engine-synthesized and currently unreachable from
        // `Scanner::scan`, but we filter both so a future
        // scanner enhancement that emits the new variant cannot
        // silently change this test's behavior (`MarkingType` is
        // `#[non_exhaustive]` per issue #461).
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
        // TODO(3c.2.C): migrate when test rewrite per Diagnostic-shape
        // lands. The file is `#![cfg(any())]`-disabled (line 1) pending
        // the Diagnostic-shape rewrite at PR 3c.2.C; PM-B-7 commits to
        // migrating this `from_parsed_unchecked` call as part of that
        // rewrite, not separately in 3c.2.B. Test-fixture carve-out
        // per Constitution V Principle V.
        let attrs = marque_ism::from_parsed_unchecked(parsed.attrs);
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
        // PR 4b-B 9th-pass follow-up: `RuleContext` is
        // `#[non_exhaustive]`; cross-crate construction goes through
        // `RuleContext::new` + `with_*` setters.
        let ctx = RuleContext::new(candidate.kind, candidate.span).with_page_portions(ctx_page);
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
