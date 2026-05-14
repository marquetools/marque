// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T131 — within-category separator span emission (issue #106).
//!
//! After PR 9a Commit 1, the parser emits `TokenKind::Separator` spans for
//! both between-category `//` and within-category `/` byte sequences. The
//! `text` field discriminates: `"//"` for between-category, `"/"` for
//! within-category. No new `TokenKind` variant was introduced.
//!
//! Authority for the within-category `/`:
//! - CAPCO-2016 §A.6 p16, Figure 2 p17 — separator alphabet for the IC
//!   marking categories. The `/` byte is the within-category separator
//!   (e.g., `OC/NF`, `SAR-A/B/C`, `NODIS/EXDIS`); `//` is the
//!   between-category separator.
//!
//! Engineering tolerance note: CAPCO-2016 §A.6 p16 forbids interjected
//! whitespace between within-category `/` separators for SAP (line 328),
//! AEA (line 330), dissem (line 334), and non-IC dissem (line 336)
//! alike, with substantively identical wording. The parser adopts an
//! engineering relaxation that consumes adjacent ASCII whitespace into
//! the Separator span when an author writes `OC / NF` instead of the
//! mandated `OC/NF`, so downstream rules see one token spanning the
//! inter-token byte range rather than failing recognition. This is a
//! Marque tolerance, NOT a §A.6-permitted variant. This test pins that
//! relaxation behavior. The SAR variant produces a strict 1-byte
//! separator span (parser.rs ~2103-2107) — no relaxation needed
//! because the corpus never demands it.

use marque_core::Parser;
use marque_ism::attrs::{TokenKind, TokenSpan};
use marque_ism::span::{MarkingCandidate, MarkingType, Span};
use marque_ism::token_set::CapcoTokenSet;

// -----------------------------------------------------------------------
// Test plumbing — mirrors the in-crate test helpers in `parser.rs`.
// -----------------------------------------------------------------------

fn parse_portion(text: &str) -> Vec<TokenSpan> {
    let source = text.as_bytes();
    let tokens = CapcoTokenSet;
    let parser = Parser::new(&tokens);
    let candidate = MarkingCandidate {
        span: Span::new(0, source.len()),
        kind: MarkingType::Portion,
    };
    let parsed = parser
        .parse(&candidate, source)
        .expect("parse should succeed");
    parsed.attrs.token_spans.to_vec()
}

fn parse_banner(text: &str) -> Vec<TokenSpan> {
    let source = text.as_bytes();
    let tokens = CapcoTokenSet;
    let parser = Parser::new(&tokens);
    let candidate = MarkingCandidate {
        span: Span::new(0, source.len()),
        kind: MarkingType::Banner,
    };
    let parsed = parser
        .parse(&candidate, source)
        .expect("parse should succeed");
    parsed.attrs.token_spans.to_vec()
}

fn separators(spans: &[TokenSpan]) -> Vec<&TokenSpan> {
    spans
        .iter()
        .filter(|t| t.kind == TokenKind::Separator)
        .collect()
}

// -----------------------------------------------------------------------
// Within-category `/` separator emission — the core T131 contract.
// -----------------------------------------------------------------------

#[test]
fn within_category_separator_emitted_for_nodis_exdis_block() {
    // §H.9 p172-174: NODIS / EXDIS are both Non-IC dissem; combining them
    // in a single block (`ND/XD`) puts both on the within-category axis,
    // separated by `/`. The parser must now emit exactly one Separator
    // span for the `/` byte between the two tokens.
    let src = "(S//ND/XD//NF)";
    let spans = parse_portion(src);
    let seps = separators(&spans);

    // Expected separators: two `//` (between cats) + one `/` (within
    // Non-IC dissem block).
    let slash_seps: Vec<&&TokenSpan> = seps.iter().filter(|s| &*s.text == "/").collect();
    assert_eq!(
        slash_seps.len(),
        1,
        "exactly one within-category `/` separator expected; got {:?}",
        seps.iter().map(|s| &*s.text).collect::<Vec<_>>()
    );

    // The `/` byte sits between `ND` and `XD`. `(S//ND/XD//NF)` →
    // byte 6 is the `/`.
    assert_eq!(slash_seps[0].span.start, 6);
    assert_eq!(slash_seps[0].span.end, 7);
    assert_eq!(slash_seps[0].span.as_str(src.as_bytes()).unwrap(), "/");
}

#[test]
fn between_category_separator_unchanged() {
    // Pure between-category markings still produce only `//` Separators —
    // the within-category emission does not perturb the existing path.
    let src = "(TS//SI//NF)";
    let spans = parse_portion(src);
    let seps = separators(&spans);
    assert_eq!(seps.len(), 2, "two `//` separators expected");
    for s in &seps {
        assert_eq!(&*s.text, "//", "all separators must be `//` here");
    }
}

#[test]
fn mixed_within_and_between_separators_in_one_portion() {
    // `(TS//OC/NF//RELIDO)`: two `//` separators (TS//OC, NF//RELIDO)
    // plus one within-category `/` between OC and NF (both dissem).
    let src = "(TS//OC/NF//RELIDO)";
    let spans = parse_portion(src);
    let seps = separators(&spans);
    let double: Vec<&&TokenSpan> = seps.iter().filter(|s| &*s.text == "//").collect();
    let single: Vec<&&TokenSpan> = seps.iter().filter(|s| &*s.text == "/").collect();
    assert_eq!(double.len(), 2, "two `//` separators expected");
    assert_eq!(single.len(), 1, "one within-category `/` expected");

    // The `/` byte in `OC/NF` sits at byte 7 of `(TS//OC/NF//RELIDO)`.
    assert_eq!(single[0].span.start, 7);
    assert_eq!(single[0].span.end, 8);
}

#[test]
fn sar_program_separators_emitted() {
    // §A.6 p16: SAP programs separated by `/` within the SAR block.
    // `SAR-A12/B23/C34` carries three programs; two `/` separators.
    // §A.6 p16 explicitly forbids interjected whitespace in SAP-`/`,
    // so each separator span is exactly 1 byte (no whitespace
    // tolerance like the dissem/SCI multi-token path).
    let src = "(TS//SAR-A12/B23/C34)";
    let spans = parse_portion(src);
    let seps = separators(&spans);
    let single: Vec<&&TokenSpan> = seps.iter().filter(|s| &*s.text == "/").collect();
    assert_eq!(
        single.len(),
        2,
        "two within-category `/` separators expected inside SAR block; got {:?}",
        seps.iter().map(|s| &*s.text).collect::<Vec<_>>()
    );

    // SAR `/` separators must be 1 byte exactly (strict §A.6 p16).
    for s in &single {
        assert_eq!(
            s.span.end - s.span.start,
            1,
            "SAR separator span must be exactly 1 byte (no whitespace tolerance)"
        );
    }

    // First `/` between A12 and B23 → byte 12 of `(TS//SAR-A12/B23/C34)`.
    // Second `/` between B23 and C34 → byte 16.
    assert_eq!(single[0].span.start, 12);
    assert_eq!(single[1].span.start, 16);
}

#[test]
fn whitespace_adjacent_to_within_category_slash_extends_span() {
    // §A.6 p16 is silent on dissem/SCI `/` whitespace tolerance — the
    // manual forbids whitespace only for SAP-`/`. Marque includes
    // trailing ASCII whitespace in the Separator span as engineering
    // tolerance for author drift; the manual itself does NOT require
    // this. Documented at the emission site in `parser.rs`.
    let src = "(S//OC/ NF)";
    let spans = parse_portion(src);
    let seps = separators(&spans);
    let single: Vec<&&TokenSpan> = seps.iter().filter(|s| &*s.text == "/").collect();
    assert_eq!(single.len(), 1, "one within-category `/` expected");
    // `/` at byte 6, then space at byte 7. Separator span covers both.
    assert_eq!(single[0].span.start, 6);
    assert_eq!(single[0].span.end, 8);
}

#[test]
fn banner_within_category_separator_emitted() {
    // Banner-form equivalence: same emission contract holds for
    // banner-form markings. `SECRET//ORCON/NOFORN` carries one
    // within-category `/` between two dissem-axis tokens.
    let src = "SECRET//ORCON/NOFORN";
    let spans = parse_banner(src);
    let seps = separators(&spans);
    let single: Vec<&&TokenSpan> = seps.iter().filter(|s| &*s.text == "/").collect();
    assert_eq!(single.len(), 1, "one within-category `/` expected");
    // `SECRET//ORCON/NOFORN` → `/` at byte 13.
    assert_eq!(single[0].span.start, 13);
    assert_eq!(single[0].span.end, 14);
}

#[test]
fn mixed_category_slash_block_emits_no_within_separator() {
    // §A.6 p16: `/` separates entries within a single category; using it
    // between categories (e.g., SCI + dissem in `SI/NF`) is a structural
    // error. The parser keeps the pre-T131 behavior here — emit the
    // whole block as Unknown so E004 (missing `//`) can fire — and does
    // NOT emit within-category Separator spans for the bogus `/`.
    let src = "(S//SI/NF)";
    let spans = parse_portion(src);
    let seps = separators(&spans);
    let single: Vec<&&TokenSpan> = seps.iter().filter(|s| &*s.text == "/").collect();
    assert_eq!(
        single.len(),
        0,
        "no within-category `/` Separator must be emitted for mixed-category blocks; \
         the `/` is structurally invalid and E004 surface it as a missing `//`"
    );
}
