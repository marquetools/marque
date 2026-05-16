// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! DISPLAY ONLY list-aware parsing — Phase 1 of the
//! `display_only_to` axis.
//!
//! Per CAPCO-2016 §H.8 p163 the canonical portion + banner form is
//! `DISPLAY ONLY [LIST]` where `[LIST]` is the country / tetragraph
//! list (comma-separated, alphabetical trigraphs then alphabetical
//! tetragraphs per §H.8 p164). The ODNI ISM CVE value `DISPLAYONLY`
//! (no space) is the machine-readable form; the CAPCO portion/banner
//! form uses the space-separated `DISPLAY ONLY` with a trailing list.
//!
//! Pre-fix the parser had no list-aware path: `DISPLAY ONLY AFG`
//! parsed as one unrecognized blob (E008) because
//! `DissemControl::parse("DISPLAY ONLY")` returns `None` (the CVE
//! token is `DISPLAYONLY`) and no sub-parser handled the
//! `DISPLAY ONLY [LIST]` grammar. The fix adds a sub-parser modeled
//! on `parse_rel_to_with_spans` plus a new `display_only_to` axis
//! on `ParsedAttrs` / `CanonicalAttrs` / `ProjectedMarking`.
//!
//! Tests pin: portion form (single country), portion form (multi),
//! banner form (single + multi), §H.8 p165 notional example pages,
//! and the ODNI CVE form (still works).
//!
//! Out of scope for this test (deferred to follow-up): banner
//! roll-up rules per §D.2 Table 3 rows 25-26 (PageContext aggregation
//! is empty in Phase 1; see `display_only_to: Box::new([])` at
//! `crates/ism/src/page_context.rs::ProjectedMarking::project`).

use marque_core::Parser;
use marque_ism::attrs::TokenKind;
use marque_ism::span::{MarkingCandidate, MarkingType, Span};
use marque_ism::token_set::CapcoTokenSet;

fn parse_portion(text: &str) -> marque_ism::ParsedAttrs<'_> {
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

fn parse_banner(text: &str) -> marque_ism::ParsedAttrs<'_> {
    let source = text.as_bytes();
    let tokens = CapcoTokenSet;
    let parser = Parser::new(&tokens);
    let candidate = MarkingCandidate {
        span: Span::new(0, source.len()),
        kind: MarkingType::Banner,
    };
    parser
        .parse(&candidate, source)
        .expect("parse should succeed")
        .attrs
}

#[test]
fn display_only_single_country_portion() {
    let attrs = parse_portion("(S//DISPLAY ONLY AFG)");
    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(
        unknown.is_empty(),
        "(S//DISPLAY ONLY AFG) must parse with no Unknown spans; got {:?}",
        unknown
            .iter()
            .map(|t| (&*t.text, t.span.start, t.span.end))
            .collect::<Vec<_>>()
    );
    let codes: Vec<&str> = attrs.display_only_to.iter().map(|e| e.bytes).collect();
    assert_eq!(codes, vec!["AFG"]);
}

#[test]
fn display_only_multiple_countries_portion() {
    let attrs = parse_portion("(C//DISPLAY ONLY AFG, IRQ)");
    let codes: Vec<&str> = attrs.display_only_to.iter().map(|e| e.bytes).collect();
    assert_eq!(codes, vec!["AFG", "IRQ"]);
    let block_spans: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::DisplayOnlyBlock)
        .collect();
    assert_eq!(block_spans.len(), 1, "expected one DisplayOnlyBlock span");
    assert_eq!(&*block_spans[0].text, "DISPLAY ONLY AFG, IRQ");

    let trigraph_spans: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::DisplayOnlyTrigraph)
        .collect();
    assert_eq!(
        trigraph_spans.len(),
        2,
        "expected two DisplayOnlyTrigraph spans"
    );
}

#[test]
fn display_only_banner_single_country() {
    // §H.8 p163 example banner line: `SECRET//DISPLAY ONLY IRQ`.
    let attrs = parse_banner("SECRET//DISPLAY ONLY IRQ");
    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(unknown.is_empty(), "no Unknown spans expected");
    let codes: Vec<&str> = attrs.display_only_to.iter().map(|e| e.bytes).collect();
    assert_eq!(codes, vec!["IRQ"]);
}

#[test]
fn display_only_banner_multi_country() {
    // §H.8 p163 example banner with multiple countries.
    let attrs = parse_banner("CONFIDENTIAL//DISPLAY ONLY AFG, IRQ");
    let codes: Vec<&str> = attrs.display_only_to.iter().map(|e| e.bytes).collect();
    assert_eq!(codes, vec!["AFG", "IRQ"]);
}

#[test]
fn display_only_with_tetragraph_in_list() {
    // §H.8 p164: "Country codes are listed alphabetically followed by
    // tetragraph codes in alphabetical order." Exercise the
    // tetragraph branch of the country-code parser inside DISPLAY
    // ONLY context.
    let attrs = parse_portion("(S//DISPLAY ONLY AFG, NATO)");
    let codes: Vec<&str> = attrs.display_only_to.iter().map(|e| e.bytes).collect();
    assert_eq!(codes, vec!["AFG", "NATO"]);
}

#[test]
fn display_only_commingling_in_rel_to_portion() {
    // CAPCO-2016 §H.8 p165 Notional Example Page 5 portion form:
    // `(S//REL TO USA, IRQ/DISPLAY ONLY AFG)` — REL TO and DISPLAY
    // ONLY commingled in the same `//`-block, separated by a single
    // `/`. §H.8 p164 admits this commingling under defined
    // disclosure-review conditions.
    //
    // Before the slash-tail commingling fix, the trailing
    // `DISPLAY ONLY AFG` part fell through to `Unknown` (because
    // `DissemControl::parse("DISPLAY ONLY AFG") == None` — the CVE
    // token is `DISPLAYONLY` with no space). Post-fix, the tail loop
    // detects the `DISPLAY ONLY ` prefix and routes through
    // `parse_display_only_with_spans`, populating both `rel_to` and
    // `display_only_to` axes.
    let attrs = parse_portion("(S//REL TO USA, IRQ/DISPLAY ONLY AFG)");
    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(
        unknown.is_empty(),
        "no Unknown spans expected on §H.8 p165 commingling form; got {:?}",
        unknown
            .iter()
            .map(|t| (&*t.text, t.span.start, t.span.end))
            .collect::<Vec<_>>()
    );

    let rel_to_codes: Vec<&str> = attrs.rel_to.iter().map(|e| e.bytes).collect();
    assert_eq!(rel_to_codes, vec!["USA", "IRQ"]);
    let do_codes: Vec<&str> = attrs.display_only_to.iter().map(|e| e.bytes).collect();
    assert_eq!(do_codes, vec!["AFG"]);

    // Both block-level spans should be emitted (one RelToBlock,
    // one DisplayOnlyBlock) for span anchoring by future rules.
    let rel_blocks = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::RelToBlock)
        .count();
    let do_blocks = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::DisplayOnlyBlock)
        .count();
    assert_eq!(rel_blocks, 1, "expected one RelToBlock span");
    assert_eq!(do_blocks, 1, "expected one DisplayOnlyBlock span");
}

#[test]
fn display_only_commingling_with_whitespace_around_slash() {
    // Regression: Copilot review on PR #445 flagged that the
    // slash-tail handler in `parse_rel_to_with_spans` (and the
    // mirroring loop in `parse_display_only_with_spans`) dropped
    // bytes when whitespace appeared around the `/` separator:
    //   (1) `IRQ/ DISPLAY ONLY AFG` — the `D` span started on the
    //       space (`tail_base` didn't account for leading whitespace
    //       of the raw tail).
    //   (2) `NF / OC` — the `O` span started one byte early
    //       (`tail_cursor` advanced by trimmed length instead of
    //       untrimmed length, dropping the trailing space before
    //       the next `/`).
    //
    // CAPCO §A.6 p16 forbids interjected whitespace within `/`
    // separators, but the existing parser tolerance for whitespace
    // around within-category separators (per
    // `crates/core/tests/separator_spans.rs`) means the spans must
    // still anchor on the canonical token bytes when authors drift.
    //
    // This test pins both (1) and (2): the DisplayOnlyBlock span
    // must start exactly on the `D` of `DISPLAY ONLY` even with
    // intervening whitespace, and the trigraph span must land on
    // `A` of `AFG` regardless of the slash-adjacent whitespace.
    let src = "(S//REL TO USA, IRQ/ DISPLAY ONLY AFG)";
    let attrs = parse_portion(src);
    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(unknown.is_empty(), "no Unknown spans expected");

    let do_block = attrs
        .token_spans
        .iter()
        .find(|t| t.kind == TokenKind::DisplayOnlyBlock)
        .expect("DisplayOnlyBlock span present");
    // `(S//REL TO USA, IRQ/ DISPLAY ONLY AFG)` — byte positions:
    //   ( = 0, S = 1, / = 2,3 = '//', R = 4, ... I = 16, R = 17,
    //   Q = 18, / = 19, ' ' = 20, D = 21, ...
    let expected_d_start = src.find("DISPLAY ONLY AFG").unwrap();
    assert_eq!(
        do_block.span.start, expected_d_start,
        "DisplayOnlyBlock span must start on `D`, not the leading space"
    );
    assert_eq!(&*do_block.text, "DISPLAY ONLY AFG");

    let trigraph = attrs
        .token_spans
        .iter()
        .find(|t| t.kind == TokenKind::DisplayOnlyTrigraph)
        .expect("DisplayOnlyTrigraph span present");
    let expected_a_start = src.find("AFG").unwrap();
    assert_eq!(
        trigraph.span.start, expected_a_start,
        "DisplayOnlyTrigraph span must start on `A`"
    );
    assert_eq!(trigraph.span.end, expected_a_start + 3);
}

#[test]
fn cve_form_displayonly_unchanged() {
    // The pre-fix path `(U//DISPLAYONLY)` (ODNI CVE value, no space)
    // continues to route through the existing `DissemControl::parse`
    // recognizer — the new DISPLAY ONLY axis only handles the
    // space-separated `DISPLAY ONLY [LIST]` form. This test pins the
    // backward-compatibility.
    let attrs = parse_portion("(U//DISPLAYONLY)");
    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(unknown.is_empty(), "no Unknown spans expected");
    assert!(
        attrs.display_only_to.is_empty(),
        "CVE-form DISPLAYONLY routes through DissemControl, not display_only_to"
    );
    // The CVE form lands on the dissem axis via the existing parser path.
    let dissem_tokens: Vec<&str> = attrs.dissem_us.iter().map(|d| d.bytes).collect();
    assert!(dissem_tokens.contains(&"DISPLAYONLY"));
}

#[test]
fn h8_p165_notional_example_page1() {
    // CAPCO-2016 §H.8 p165 Notional Example Page 1: banner top +
    // three identical `(S//DISPLAY ONLY AFG)` portion marks +
    // banner bottom. This test reproduces the full structural shape
    // of the manual's example (all three portions, both banner
    // occurrences) and asserts the canonical zero-Unknown property.
    let src = "SECRET//DISPLAY ONLY AFG\n\n\
               (S//DISPLAY ONLY AFG) This portion is classified SECRET and is authorized for DISPLAY ONLY Afghanistan (AFG). This portion is marked for training purposes only.\n\n\
               (S//DISPLAY ONLY AFG) This portion is classified SECRET and is authorized for DISPLAY ONLY AFG. This portion is marked for training purposes only.\n\n\
               (S//DISPLAY ONLY AFG) This portion is classified SECRET and is authorized for DISPLAY ONLY AFG. This portion is marked for training purposes only.\n\n\
               SECRET//DISPLAY ONLY AFG";
    // Engine-level lint should produce zero E008 over this manual
    // example — the byte-position assertions are in the per-form
    // tests above; here we just want the document-level clean pass.
    use marque_core::{MarkingType, Parser, Scanner};
    let tokens = CapcoTokenSet;
    let parser = Parser::new(&tokens);
    let candidates = Scanner::scan(src.as_bytes());
    let mut unknown_total = 0usize;
    for c in &candidates {
        if !matches!(c.kind, MarkingType::Portion | MarkingType::Banner) {
            continue;
        }
        let Ok(parsed) = parser.parse(c, src.as_bytes()) else {
            continue;
        };
        unknown_total += parsed
            .attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::Unknown)
            .count();
    }
    assert_eq!(
        unknown_total, 0,
        "no Unknown spans expected across the §H.8 p165 notional example"
    );
}
