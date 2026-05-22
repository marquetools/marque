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
//! # PR 3c.2.C C5 reshape
//!
//! Under the closed-template `Message` shape and the typed `Citation`
//! struct, the message- and citation-leak channels are now
//! STRUCTURALLY closed:
//!
//! - `Diagnostic.message: Message` carries `MessageTemplate` (closed
//!   enum) + `MessageArgs` (closed struct of typed identifiers); raw
//!   bytes are unrepresentable.
//! - `Diagnostic.citation: Citation` is a typed struct; the Display
//!   impl emits a fixed `§X.Y pN` shape or a `[<source>]` sentinel,
//!   never a free-form string.
//!
//! The test purpose STRENGTHENS: instead of grepping prose for
//! document content, we verify the closed-set type discipline holds.
//! The legacy substring sweep is preserved for `TextCorrection.replacement`
//! (the one remaining bytes-carrying surface) and a transitive check
//! on Display renders.
//!
//! PM-C-3: cfg-gate lifted in PR 3c.2.C C5. Test-fixture carve-out
//! per Constitution V Principle V.

use std::sync::Arc;

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_core::{Parser, Scanner};
use marque_ism::{CanonicalAttrs, CapcoTokenSet, MarkingType};
use marque_rules::{Diagnostic, RuleContext, RuleSet};
use marque_scheme::MarkingScheme;

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
        // PR 3c.2.B B4 (PM-B-1, PM-B-3): canonicalize via the trait
        // override; reuse the already-constructed `scheme` for zero
        // new allocation cost.
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
        // PR 4b-B 9th-pass follow-up: `RuleContext` is
        // `#[non_exhaustive]`; cross-crate construction goes through
        // `RuleContext::new` + `with_*` setters.
        let ctx = RuleContext::new(candidate.kind, candidate.span).with_page_portions(ctx_page);
        for rule in rule_set.rules() {
            out.extend(rule.check(&attrs, &ctx));
        }
        // S004 fires through the rule pipeline (not the constraint-
        // catalog bridge), so no bridge invocation is needed here. The
        // bridge surface is exercised by dedicated tests at
        // `crates/capco/tests/bridge_message_by_name.rs` and
        // `class_floor_catalog.rs`.
    }
    out
}

/// The fixture has a portion-form REL TO with `ASM` (S004's current
/// driving pair per `crates/capco/tests/s004_coverage_exclusion.rs`).
/// The corpus priors shifted across growth so the issue-spec
/// `AUT → AUS` example no longer fires under the current calibration;
/// `ASM → USA` is the active S004 trigger today. Issue #439's
/// coverage-exclusion rule suppresses the suggestion when the
/// candidate (USA) is already in the block, so the fixture omits USA;
/// `DEU` (registered trigraph, edit distance 3 from ASM, not
/// decomposable) is included to anchor the REL TO list without
/// suppressing S004.
///
/// The fixture is wrapped in "sensitive" non-marking prose; the test
/// asserts that prose never appears in any audit-bound output —
/// diagnostic messages, fix proposals, or citations.
const FIXTURE: &[u8] = b"\
(C//REL TO ASM, DEU)

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
fn s004_diagnostic_message_carries_only_closed_template() {
    // PR 3c.2.C C5: G13 closure for `Diagnostic.message` is
    // STRUCTURALLY enforced by `Message` (closed template + closed
    // args). The test verifies the structural property: S004's
    // template label is a fixed `&'static str` from the closed
    // `MessageTemplate` enum, and the args carry only typed
    // identifiers (no raw bytes).
    let diags = lint(FIXTURE);
    let s004: Vec<_> = diags.iter().filter(|d| d.rule.predicate_id() == "portion.dissem.rel-to-trigraph-suggest").collect();
    assert!(
        !s004.is_empty(),
        "S004 must fire on the ASM trigraph in the fixture"
    );

    for d in &s004 {
        // Template label is `&'static str` from a closed enum;
        // no document content can flow through it.
        let template_label = d.message.template().as_str();
        for phrase in FORBIDDEN_PHRASES {
            assert!(
                !template_label.contains(phrase),
                "S004 template label {template_label:?} contains document text {phrase:?}"
            );
        }
        // Args carry only `Option<TokenId>` / `Option<CategoryId>` /
        // `Option<Span>` etc. — no `String` field. The compile-fail
        // doctests in `crates/rules/src/message.rs` pin this; the
        // runtime check here is informational.
        let _ = d.message.args();
    }
}

#[test]
fn s004_text_correction_replacement_contains_no_document_content() {
    // S004 emits a `text_correction` carrying the corpus-derived
    // canonical neighbor trigraph (e.g., `"AUT" → "AUS"`).
    // `TextCorrection.replacement: SmolStr` is the one bytes-carrying
    // surface on the diagnostic; Constitution V requires it to be a
    // permitted identifier from the closed CAPCO vocabulary.
    //
    // PR 3c.2.C C5: the retired `FixProposal.original` field is gone
    // (the engine derives the original bytes from `source[span]` at
    // promotion time per PM-C-5 renderer-responsibility), so the
    // legacy `fix.original.contains` check is dropped — that channel
    // doesn't exist anymore.
    let diags = lint(FIXTURE);
    let s004: Vec<_> = diags.iter().filter(|d| d.rule.predicate_id() == "portion.dissem.rel-to-trigraph-suggest").collect();
    assert!(!s004.is_empty(), "S004 must fire");

    for d in &s004 {
        let Some(tc) = d.text_correction.as_ref() else {
            continue;
        };
        let replacement = tc.replacement.as_str();
        for phrase in FORBIDDEN_PHRASES {
            assert!(
                !replacement.contains(phrase),
                "S004 text_correction replacement {replacement:?} contains document text {phrase:?}"
            );
        }
        // Replacement is a bare 3-letter trigraph from the closed
        // CAPCO Annex B vocabulary. Anything else is a content-leak.
        assert_eq!(
            replacement.len(),
            3,
            "S004 replacement must be a bare trigraph (3 ASCII chars), got: {replacement:?}"
        );
    }
}

#[test]
fn s004_citation_carries_only_typed_capco_anchor() {
    // PR 3c.2.C C5: `Diagnostic.citation` is now a typed `Citation`
    // struct. The Display impl emits a fixed `§<L>.<sub> p<N>` shape
    // for CAPCO sources (no free-form prose). The legacy substring
    // sweep is replaced by a structural check on the rendered form.
    let diags = lint(FIXTURE);
    let s004: Vec<_> = diags.iter().filter(|d| d.rule.predicate_id() == "portion.dissem.rel-to-trigraph-suggest").collect();
    assert!(!s004.is_empty(), "S004 must fire");

    for d in &s004 {
        let rendered = format!("{}", d.citation);
        for phrase in FORBIDDEN_PHRASES {
            assert!(
                !rendered.contains(phrase),
                "S004 citation render {rendered:?} contains document text {phrase:?}"
            );
        }
        // Pin the §-citation anchor: §H.8 p150 (REL TO grammar) per
        // `RelToTrigraphSuggestRule`'s emit body.
        assert!(
            rendered.starts_with("§"),
            "S004 citation must render as a typed CAPCO §-reference, got: {rendered}"
        );
    }
}
