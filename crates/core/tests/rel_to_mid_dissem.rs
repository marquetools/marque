// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! REL TO as a same-category sub-token after another dissem token.
//!
//! CAPCO-2016 §H.8 p150-151 places REL TO in the dissem category.
//! §A.6 p16 specifies the within-category separator as a single `/`
//! (with `//` reserved for between-category). A portion mark like
//! `(TS//SI-G ABCD//OC/REL TO USA, NOR)` therefore carries one
//! dissem-category block `OC/REL TO USA, NOR` containing ORCON and
//! REL TO, separated by the within-category `/`.
//!
//! Pre-fix behavior: the multi-token block handler in `parser.rs`
//! recognized `OC` as a dissem sub-token but had no recognizer for
//! the REL TO sub-token, so the entire `REL TO USA, NOR` span was
//! committed as `TokenKind::Unknown` and E008 fired in the rule
//! layer. The documents corpus showed 106 such hits across four
//! valid trigraph/tetragraph variants (NOR/NATO/EST/FVEY).
//!
//! This test pins the post-fix contract: the dissem axis carries
//! ORCON, the rel_to axis carries the parsed country codes, no
//! token is left in `TokenKind::Unknown`, and the within-category
//! separator span is emitted between OC and REL TO.

use marque_core::Parser;
use marque_ism::attrs::TokenKind;
use marque_ism::span::{MarkingCandidate, MarkingType, Span};
use marque_ism::token_set::CapcoTokenSet;

fn parse(text: &str) -> marque_ism::ParsedAttrs<'_> {
    let source = text.as_bytes();
    let tokens = CapcoTokenSet;
    let parser = Parser::new(&tokens);
    let candidate = MarkingCandidate {
        span: Span::new(0, source.len()),
        kind: MarkingType::Portion,
    };
    parser
        .parse(&candidate, source)
        .expect("parse should succeed")
        .attrs
}

#[test]
fn rel_to_after_orcon_same_category_no_unknown() {
    let src = "(TS//SI-G ABCD//OC/REL TO USA, NOR)";
    let attrs = parse(src);

    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(
        unknown.is_empty(),
        "no Unknown spans expected, got: {:?}",
        unknown
            .iter()
            .map(|t| (&*t.text, t.span.start, t.span.end))
            .collect::<Vec<_>>()
    );

    let rel_to_codes: Vec<&str> = attrs.rel_to.iter().map(|e| e.bytes).collect();
    assert_eq!(
        rel_to_codes,
        vec!["USA", "NOR"],
        "rel_to should carry both USA and NOR"
    );

    let dissem_tokens: Vec<&str> = attrs.dissem_us.iter().map(|d| d.bytes).collect();
    assert!(
        dissem_tokens.contains(&"OC"),
        "OC should be on the dissem axis; got {dissem_tokens:?}"
    );

    let block_spans: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::RelToBlock)
        .collect();
    assert_eq!(
        block_spans.len(),
        1,
        "exactly one RelToBlock span expected; got {:?}",
        block_spans
            .iter()
            .map(|t| (&*t.text, t.span.start, t.span.end))
            .collect::<Vec<_>>()
    );
    assert_eq!(&*block_spans[0].text, "REL TO USA, NOR");

    let trigraph_spans: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::RelToTrigraph)
        .collect();
    assert_eq!(
        trigraph_spans.len(),
        2,
        "two RelToTrigraph spans expected (USA + NOR); got {trigraph_spans:?}"
    );

    let within_seps: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Separator && &*t.text == "/")
        .collect();
    assert_eq!(
        within_seps.len(),
        1,
        "one within-category `/` separator expected between OC and REL TO; got {within_seps:?}"
    );
}

#[test]
fn rel_to_after_orcon_simple_class() {
    // Same shape, without the SCI block: `(S//OC/REL TO USA, GBR)`.
    // Tetragraph FVEY and trigraph EST are exercised by the corpus;
    // here we pin the simpler shape so the test fails cleanly on the
    // base parser path without dragging SCI grammar into the repro.
    let src = "(S//OC/REL TO USA, GBR)";
    let attrs = parse(src);

    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(unknown.is_empty(), "no Unknown spans expected");

    let rel_to_codes: Vec<&str> = attrs.rel_to.iter().map(|e| e.bytes).collect();
    assert_eq!(rel_to_codes, vec!["USA", "GBR"]);
}

#[test]
fn rel_to_with_tetragraph_after_orcon() {
    // FVEY is one of the 106 corpus hits — exercise the tetragraph
    // branch of `parse_rel_to_with_spans` from inside the multi-token
    // dissem block.
    let src = "(S//OC/REL TO USA, FVEY)";
    let attrs = parse(src);
    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(unknown.is_empty(), "no Unknown spans expected");
    let rel_to_codes: Vec<&str> = attrs.rel_to.iter().map(|e| e.bytes).collect();
    assert_eq!(rel_to_codes, vec!["USA", "FVEY"]);
}

#[test]
fn rel_to_with_trailing_dissem_after_orcon() {
    // `OC/REL TO USA, NOR/NF` — the new commit branch routes through
    // `parse_rel_to_with_spans`, which splits the trailing `/NF` off
    // the last comma entry as a `trailing_dissem` control. The
    // commit-branch code then extends the outer `dissem` vec from
    // that result. This test pins the wiring; without it a refactor
    // that drops the `dissem.extend(parsed.trailing_dissem)` call
    // would silently lose the trailing NOFORN.
    //
    // Authority: CAPCO-2016 §H.8 p150-151 (REL TO syntax allows
    // trailing same-category controls separated by `/`).
    let src = "(S//OC/REL TO USA, NOR/NF)";
    let attrs = parse(src);
    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(
        unknown.is_empty(),
        "no Unknown spans expected; got {:?}",
        unknown
            .iter()
            .map(|t| (&*t.text, t.span.start, t.span.end))
            .collect::<Vec<_>>()
    );

    let rel_to_codes: Vec<&str> = attrs.rel_to.iter().map(|e| e.bytes).collect();
    assert_eq!(rel_to_codes, vec!["USA", "NOR"]);

    let dissem_tokens: Vec<&str> = attrs.dissem_us.iter().map(|d| d.bytes).collect();
    assert!(
        dissem_tokens.contains(&"OC"),
        "OC should be on the dissem axis; got {dissem_tokens:?}"
    );
    assert!(
        dissem_tokens.contains(&"NF"),
        "NF (from trailing_dissem absorption) should be on the dissem axis; got {dissem_tokens:?}"
    );
}

#[test]
fn rel_lookalike_mangled_token_does_not_match() {
    // The new sub-token recognizer uses `starts_with("REL TO ")` —
    // not `starts_with("REL ")` — specifically to reject mangled
    // non-REL-TO tokens that would otherwise route to
    // `parse_rel_to_with_spans` and silently succeed with zero
    // countries. `REL IDO` (a typo of `RELIDO`) is the canonical
    // example: it would match a `"REL "` prefix but is not a valid
    // REL TO block. Verify it falls through to Unknown so the rule
    // layer (E008) can surface it.
    let src = "(S//OC/REL IDO)";
    let attrs = parse(src);
    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(
        !unknown.is_empty(),
        "mangled `REL IDO` must remain Unknown so E008 can surface it; \
         got spans: {:?}",
        attrs
            .token_spans
            .iter()
            .map(|t| (t.kind, &*t.text))
            .collect::<Vec<_>>()
    );
    assert!(
        attrs.rel_to.is_empty(),
        "mangled token must not have routed through parse_rel_to_with_spans"
    );
}

#[test]
fn baseline_bare_rel_to_unchanged() {
    // The pre-existing fast path `(S//REL TO USA, GBR)` (no dissem
    // prefix) must keep working: the early `trimmed.starts_with("REL TO")`
    // branch in `parser.rs` is unchanged.
    let src = "(S//REL TO USA, GBR)";
    let attrs = parse(src);
    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(unknown.is_empty(), "no Unknown spans expected");
    let rel_to_codes: Vec<&str> = attrs.rel_to.iter().map(|e| e.bytes).collect();
    assert_eq!(rel_to_codes, vec!["USA", "GBR"]);
}
