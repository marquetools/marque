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
use marque_ism::span::{MarkingCandidate, MarkingType};
use marque_ism::token_set::CapcoTokenSet;
use marque_scheme::Span;

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
fn display_only_multi_token_commingling_after_orcon_and_rel_to() {
    // CAPCO-2016 §H.8 p164 commingling rule: DISPLAY ONLY may be
    // commingled with another same-category dissem control under
    // defined disclosure-review conditions. The multi-token shape
    // `(S//OC/REL TO USA, IRQ/DISPLAY ONLY AFG)` puts three
    // dissem-family sub-tokens in the same `//`-block:
    //   - `OC` (ORCON)
    //   - `REL TO USA, IRQ`
    //   - `DISPLAY ONLY AFG`
    //
    // `split_slash_with_separator_offsets` yields three sub-tokens.
    // The multi-token speculative loop must recognize the DISPLAY
    // ONLY sub-token (parallel to the REL TO sub-token recognizer
    // added in PR #440) and route it through
    // `parse_display_only_with_spans` at commit. Without this
    // arm the `DISPLAY ONLY AFG` token would fall to
    // `SubKind::Unknown` and emit E008 (Copilot review on PR #445
    // caught this).
    let attrs = parse_portion("(S//OC/REL TO USA, IRQ/DISPLAY ONLY AFG)");
    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(
        unknown.is_empty(),
        "no Unknown spans expected for three-way same-category commingling; got {:?}",
        unknown
            .iter()
            .map(|t| (&*t.text, t.span.start, t.span.end))
            .collect::<Vec<_>>()
    );

    // All three sub-tokens land on their respective axes.
    let dissem: Vec<&str> = attrs.dissem_us.iter().map(|d| d.bytes).collect();
    assert!(
        dissem.contains(&"OC"),
        "ORCON should be on the dissem axis; got {dissem:?}"
    );
    let rel: Vec<&str> = attrs.rel_to.iter().map(|e| e.bytes).collect();
    assert_eq!(rel, vec!["USA", "IRQ"]);
    let dox: Vec<&str> = attrs.display_only_to.iter().map(|e| e.bytes).collect();
    assert_eq!(dox, vec!["AFG"]);

    // Block-level spans confirm each axis was recognized at its
    // canonical token boundary.
    let do_blocks = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::DisplayOnlyBlock)
        .count();
    let rel_blocks = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::RelToBlock)
        .count();
    assert_eq!(do_blocks, 1);
    assert_eq!(rel_blocks, 1);
}

#[test]
fn display_only_accepts_full_country_code_width_surface() {
    // ODNI ISMCAT's CVEnumISMCATRelTo.xsd admits country codes at
    // four widths (340 entries total, generated into
    // `values::TRIGRAPHS` by build.rs):
    //   - 2-char registered exception (`EU`)
    //   - 3-char trigraphs (`AFG`, `USA`, ...)
    //   - 4-char tetragraphs (`NATO`, `FVEY`, `ACGU`, `KFOR`, ...)
    //   - 15-char (`AUSTRALIA_GROUP`)
    //
    // CAPCO-2016 §H.8 p164 admits all of these in the
    // DISPLAY ONLY list. The same `TokenSet::is_trigraph` predicate
    // used by REL TO drives DISPLAY ONLY recognition (issue #444
    // tracks the misnomer rename), so the surface should be
    // identical. This test pins each width on its own line.
    for src in [
        "(U//DISPLAY ONLY EU)",
        "(U//DISPLAY ONLY AFG)",
        "(U//DISPLAY ONLY NATO)",
        "(U//DISPLAY ONLY FVEY)",
        "(U//DISPLAY ONLY ACGU)",
        "(U//DISPLAY ONLY AUSTRALIA_GROUP)",
        // Mixed-width list — explicit alphabetical ordering per
        // §H.8 p164 (trigraphs alphabetically then tetragraphs).
        "(U//DISPLAY ONLY AFG, EU, ACGU, FVEY, NATO, AUSTRALIA_GROUP)",
    ] {
        let attrs = parse_portion(src);
        let unknown: Vec<_> = attrs
            .token_spans
            .iter()
            .filter(|t| t.kind == TokenKind::Unknown)
            .collect();
        assert!(
            unknown.is_empty(),
            "{src:?}: no Unknown spans expected; got {:?}",
            unknown
                .iter()
                .map(|t| (&*t.text, t.span.start, t.span.end))
                .collect::<Vec<_>>()
        );
        assert!(
            !attrs.display_only_to.is_empty(),
            "{src:?}: display_only_to must be populated"
        );
    }
}

#[test]
fn display_only_invalid_country_in_list_emits_unknown_only_for_that_entry() {
    // `XYZQ` is not in `values::TRIGRAPHS`. The DISPLAY ONLY list
    // parser should emit `TokenKind::Unknown` for that single
    // entry while still recognizing the valid `AFG` and `IRQ`
    // entries on either side. Mirrors REL TO's behavior on
    // unknown trigraphs (parser.rs comments cite issue #233 for
    // the rationale: decoder dispatcher consults Unknown spans
    // to surface fuzzy-trigraph alternates).
    let attrs = parse_portion("(U//DISPLAY ONLY AFG, XYZQ, IRQ)");
    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert_eq!(
        unknown.len(),
        1,
        "exactly one Unknown span expected (for XYZQ); got {:?}",
        unknown
            .iter()
            .map(|t| (&*t.text, t.span.start, t.span.end))
            .collect::<Vec<_>>()
    );
    assert_eq!(&*unknown[0].text, "XYZQ");

    // The known entries still populate the axis.
    let codes: Vec<&str> = attrs.display_only_to.iter().map(|e| e.bytes).collect();
    assert_eq!(codes, vec!["AFG", "IRQ"]);
}

#[test]
fn display_only_byte_precise_per_trigraph_spans() {
    // Lock the per-trigraph span offsets. Downstream rules (E054
    // / E055 + future DISPLAY ONLY constraint rules) anchor
    // diagnostics on these spans. Drift would silently
    // mis-position diagnostics by even one byte.
    let src = "(C//DISPLAY ONLY AFG, IRQ)";
    let attrs = parse_portion(src);

    let trigraphs: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::DisplayOnlyTrigraph)
        .collect();
    assert_eq!(trigraphs.len(), 2);

    let afg = src.find("AFG").unwrap();
    let irq = src.find("IRQ").unwrap();
    // Trigraphs may be emitted out of document order before the
    // final `sort_unstable_by_key` pass; check span starts as a
    // set, not as a sequence.
    let starts: Vec<usize> = trigraphs.iter().map(|t| t.span.start).collect();
    assert!(
        starts.contains(&afg) && starts.contains(&irq),
        "trigraph spans must land on `A` of AFG and `I` of IRQ; got starts={starts:?}"
    );
    // Width assertions: AFG is 3 bytes, IRQ is 3 bytes.
    for t in &trigraphs {
        assert_eq!(
            t.span.end - t.span.start,
            3,
            "trigraph span should cover exactly 3 bytes; got {:?}",
            t
        );
    }
}

#[test]
fn display_only_block_span_precedes_trigraph_spans_in_document_order() {
    // The block-level span starts at `D` of `DISPLAY ONLY`; the
    // per-trigraph spans start strictly later (after the
    // `DISPLAY ONLY ` prefix). The parser's final
    // `token_spans.sort_unstable_by_key(|ts| ts.span.start)` pass
    // (parser.rs `parse_inner`) sorts spans by start offset, so
    // the block span MUST precede the trigraph spans in the
    // emitted slice. This invariant is what lets downstream rules
    // anchor a diagnostic on the whole block before walking the
    // constituent trigraphs.
    let attrs = parse_portion("(S//DISPLAY ONLY AFG, IRQ)");
    let positions: Vec<(TokenKind, usize)> = attrs
        .token_spans
        .iter()
        .filter(|t| {
            matches!(
                t.kind,
                TokenKind::DisplayOnlyBlock | TokenKind::DisplayOnlyTrigraph
            )
        })
        .map(|t| (t.kind, t.span.start))
        .collect();
    assert!(
        positions.len() >= 3,
        "expected 1 block + 2 trigraph spans; got {positions:?}"
    );
    // First entry must be the block; subsequent entries are the
    // trigraphs in document order.
    assert_eq!(positions[0].0, TokenKind::DisplayOnlyBlock);
    for window in positions.windows(2) {
        assert!(
            window[0].1 <= window[1].1,
            "token_spans must be sorted by span.start; got {positions:?}"
        );
    }
}

#[test]
fn display_only_does_not_false_positive_on_truncated_keyword() {
    // The recognition gate `starts_with("DISPLAY ONLY ") ||
    // == "DISPLAY ONLY"` is tight: it requires the literal
    // keyword `DISPLAY ONLY` followed by either a trailing space
    // (with a list) or end-of-token (bare). Sibling shapes that
    // would have matched a looser guard MUST fall through to the
    // existing parsers:
    //
    //   - `DISPLAY FOO` (no `ONLY` keyword) — author typo /
    //     truncation; falls to `SubKind::Unknown` so E008 surfaces
    //   - `DISPLAYFOO`  (no space, no ONLY) — falls similarly
    //
    // CAPCO-2016 §H.8 p163 admits NO abbreviated form for
    // DISPLAY ONLY (the Authorized Banner Line Abbreviation is
    // explicitly `None` in the manual), so the guard intentionally
    // does NOT admit a `DISPLAY ` prefix.
    let attrs = parse_portion("(U//DISPLAY FOO)");
    assert!(
        attrs.display_only_to.is_empty(),
        "`DISPLAY FOO` must not populate display_only_to"
    );
    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(
        !unknown.is_empty(),
        "`DISPLAY FOO` must surface as Unknown so E008 can flag it"
    );
}

#[test]
fn display_only_populates_parsed_axis() {
    // The `display_only_to` field on `ParsedAttrs` (parser output)
    // is the data rules consume after canonicalization. Without
    // it the new axis is invisible to the rule layer.
    //
    // Pre PR 3c.2.E this test invoked `marque_ism::from_parsed_unchecked`
    // to round-trip through `CanonicalAttrs`. PR 3c.2.E retired the
    // adapter; the corresponding canonicalize path now lives in
    // `CapcoScheme::canonicalize`, which `marque-core` cannot reach
    // (Constitution VII forbids `marque-core ←── marque-capco`).
    // Asserting directly on the `ParsedAttrs` side is sufficient
    // — the structural rename to `CanonicalAttrs.display_only_to`
    // is a 1:1 `entry.value` copy exercised by the scheme-level
    // tests in `crates/capco/tests/`.
    let src = "(S//DISPLAY ONLY AFG, IRQ)";
    let attrs = parse_portion(src);
    let codes: Vec<String> = attrs
        .display_only_to
        .iter()
        .map(|e| e.value.as_str().to_string())
        .collect();
    assert_eq!(codes, vec!["AFG", "IRQ"]);
}

#[test]
fn display_only_multi_country_after_rel_to_commingling() {
    // Regression: Copilot review on PR #445 caught that the
    // entry-by-entry slash-tail handler in
    // `parse_rel_to_with_spans` misclassified the second-and-later
    // countries of a multi-country DISPLAY ONLY list commingled
    // with REL TO.
    //
    // For `(S//REL TO USA, IRQ/DISPLAY ONLY AFG, NATO)` the outer
    // `after_rel.split(',')` had already chopped `NATO` into a
    // separate REL TO entry by the time the inner handler saw the
    // `/DISPLAY ONLY AFG` slash-tail — so `NATO` landed on `rel_to`
    // instead of `display_only_to`. The fix introduces a
    // `find_display_only_slash_boundary` pre-scan that detects the
    // `/DISPLAY ONLY` boundary before the comma split, restricts
    // the REL TO scope to bytes before it, and parses the entire
    // remainder (including all its commas) via
    // `parse_display_only_with_spans` after the loop.
    let attrs = parse_portion("(S//REL TO USA, IRQ/DISPLAY ONLY AFG, NATO)");
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

    let rel: Vec<&str> = attrs.rel_to.iter().map(|e| e.bytes).collect();
    assert_eq!(
        rel,
        vec!["USA", "IRQ"],
        "rel_to must NOT include NATO — NATO belongs to the DISPLAY ONLY list"
    );
    let dox: Vec<&str> = attrs.display_only_to.iter().map(|e| e.bytes).collect();
    assert_eq!(
        dox,
        vec!["AFG", "NATO"],
        "display_only_to must include BOTH AFG and NATO"
    );

    // Block-level spans must still emit exactly once each.
    let do_blocks = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::DisplayOnlyBlock)
        .count();
    let rel_blocks = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::RelToBlock)
        .count();
    assert_eq!(do_blocks, 1);
    assert_eq!(rel_blocks, 1);
}

#[test]
fn display_only_after_rel_to_with_trailing_dissem_before_commingling() {
    // §H.8 p164 admits commingling REL TO + simple dissem +
    // DISPLAY ONLY in one block. The `/NF/DISPLAY ONLY` shape
    // exercises the pre-scan's tolerance for multi-slash tails:
    // `find_display_only_slash_boundary` must find the SECOND
    // slash (the one before `DISPLAY ONLY`), not the first.
    let attrs = parse_portion("(S//REL TO USA, GBR/NF/DISPLAY ONLY AFG, IRQ)");
    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(unknown.is_empty(), "no Unknown spans expected");

    let rel: Vec<&str> = attrs.rel_to.iter().map(|e| e.bytes).collect();
    assert_eq!(rel, vec!["USA", "GBR"]);
    let dissem: Vec<&str> = attrs.dissem_us.iter().map(|d| d.bytes).collect();
    assert!(
        dissem.contains(&"NF"),
        "trailing NF must land on dissem axis; got {dissem:?}"
    );
    let dox: Vec<&str> = attrs.display_only_to.iter().map(|e| e.bytes).collect();
    assert_eq!(dox, vec!["AFG", "IRQ"]);
}

#[test]
fn display_only_boundary_tolerates_whitespace_after_slash() {
    // Whitespace tolerance: `/ DISPLAY ONLY AFG, NATO` (space
    // after the slash before the keyword). The boundary detector
    // skips ASCII whitespace after the slash to match the
    // parser's existing within-category-separator relaxation in
    // `split_slash_with_separator_offsets` (CAPCO §A.6 p16
    // forbids interjected whitespace but the corpus occasionally
    // drifts).
    let attrs = parse_portion("(S//REL TO USA, IRQ/ DISPLAY ONLY AFG, NATO)");
    let rel: Vec<&str> = attrs.rel_to.iter().map(|e| e.bytes).collect();
    assert_eq!(rel, vec!["USA", "IRQ"]);
    let dox: Vec<&str> = attrs.display_only_to.iter().map(|e| e.bytes).collect();
    assert_eq!(dox, vec!["AFG", "NATO"]);
}

#[test]
fn display_only_boundary_tolerates_tab_after_slash() {
    // Copilot review on PR #445 caught that the boundary
    // detector originally checked only literal `b' '` spaces,
    // inconsistent with the rest of the parser's
    // `is_ascii_whitespace`-based tolerance. After the fix, a
    // tab between `/` and `DISPLAY ONLY` must also be tolerated.
    let attrs = parse_portion("(S//REL TO USA, IRQ/\tDISPLAY ONLY AFG)");
    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(
        unknown.is_empty(),
        "tab after `/` should not block boundary detection; got Unknown spans: {:?}",
        unknown
            .iter()
            .map(|t| (&*t.text, t.span.start, t.span.end))
            .collect::<Vec<_>>()
    );
    let rel: Vec<&str> = attrs.rel_to.iter().map(|e| e.bytes).collect();
    assert_eq!(rel, vec!["USA", "IRQ"]);
    let dox: Vec<&str> = attrs.display_only_to.iter().map(|e| e.bytes).collect();
    assert_eq!(dox, vec!["AFG"]);
}

#[test]
fn display_only_boundary_word_boundary_check_rejects_prefix_match() {
    // The boundary detector requires a word boundary AFTER the
    // `DISPLAY ONLY` keyword (whitespace, `/`, or end-of-string).
    // A hypothetical token like `DISPLAYONLYNESS` must NOT trip
    // the boundary detection — it would have routed through this
    // path in a naive substring search.
    //
    // CAPCO has no such token; this test guards against future
    // false-positive expansions of the detector. We use a
    // non-CAPCO token that exercises the word-boundary gate.
    let src = "(S//REL TO USA, IRQ/DISPLAY ONLYNESS)";
    let attrs = parse_portion(src);
    // The fake token `DISPLAY ONLYNESS` should NOT be admitted
    // as a DISPLAY ONLY block. The parser falls through to
    // existing tail handling (Unknown for the unrecognized
    // token); the REL TO list remains `[USA, IRQ]`.
    let rel: Vec<&str> = attrs.rel_to.iter().map(|e| e.bytes).collect();
    assert_eq!(rel, vec!["USA", "IRQ"]);
    assert!(
        attrs.display_only_to.is_empty(),
        "DISPLAY ONLYNESS must NOT trip the DISPLAY ONLY boundary detector"
    );
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
    use marque_core::{Parser, Scanner};
    use marque_ism::span::MarkingType;
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
