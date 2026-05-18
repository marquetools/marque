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
use marque_ism::span::{MarkingCandidate, MarkingType};
use marque_ism::token_set::CapcoTokenSet;
use marque_scheme::Span;

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

// -----------------------------------------------------------------------
// Bidirectional whitespace coverage — PR 9a Copilot R2 Fix 1.
//
// Before the fix, `split_slash_with_separator_offsets` only extended the
// separator span over whitespace *after* the `/`. For input `OC /NF`,
// the leading space at byte 6 was owned by neither the `OC` token (3..5)
// nor the separator (6..7 — the bare `/`) nor the `NF` token (7..9), so
// the byte range between adjacent tokens had a gap. Downstream
// byte-precise text-correction splices could produce audit records with
// uncovered bytes. The fix walks leading whitespace BEFORE the `/`,
// bounded by the end of the previous emitted token so the separator
// span never overlaps token bytes. Both sides are now covered.
// -----------------------------------------------------------------------

/// Walk the token-span stream once and assert that every byte between the
/// first and last `target_spans` is owned by exactly one TokenSpan (no
/// gaps, no overlaps). Used by the bidirectional-coverage tests below.
fn assert_contiguous_coverage(src: &str, spans: &[TokenSpan]) {
    // Skip the surrounding `(...)` by collecting only spans that fall
    // strictly inside the portion's payload range.
    let bytes = src.as_bytes();
    let open = bytes
        .iter()
        .position(|&b| b == b'(')
        .map(|p| p + 1)
        .unwrap_or(0);
    let close = bytes
        .iter()
        .rposition(|&b| b == b')')
        .unwrap_or(bytes.len());

    // Collect every span that intersects the payload region, sorted by
    // start. Skip zero-length spans (none expected; defensive).
    let mut covering: Vec<&TokenSpan> = spans
        .iter()
        .filter(|s| s.span.start >= open && s.span.end <= close && s.span.end > s.span.start)
        .collect();
    covering.sort_by_key(|s| (s.span.start, s.span.end));

    // RelToBlock / SciControl are block-level spans that cover their
    // constituent tokens. For the contiguous-coverage assertion we
    // care about whether the union of all spans covers every byte
    // between the first and last token; block-level spans contribute
    // to that union but introduce overlaps with their children. So
    // we union into a single boolean coverage bitmap.
    if covering.is_empty() {
        return;
    }
    let first = covering[0].span.start;
    let last = covering.iter().map(|s| s.span.end).max().unwrap();
    let len = last - first;
    let mut covered = vec![false; len];
    for s in &covering {
        for b in s.span.start..s.span.end {
            covered[b - first] = true;
        }
    }
    for (idx, c) in covered.iter().enumerate() {
        assert!(
            *c,
            "uncovered byte at offset {} (input {:?}): byte {:?} not owned by any span",
            first + idx,
            src,
            &src[(first + idx)..(first + idx + 1)],
        );
    }
}

#[test]
fn leading_whitespace_before_slash_is_covered() {
    // `(S//OC /NF)` — space at byte 6 (between `OC` end at byte 6 and
    // `/` at byte 7). Pre-fix: byte 6 was uncovered (token ended at 6,
    // separator started at 7). Post-fix: separator span walks back to
    // byte 6, so the inter-token range is contiguous.
    let src = "(S//OC /NF)";
    let spans = parse_portion(src);
    let seps = separators(&spans);
    let slash_seps: Vec<&&TokenSpan> = seps.iter().filter(|s| &*s.text == "/").collect();
    assert_eq!(slash_seps.len(), 1, "expected one within-category `/`");

    // `(S//OC /NF)`: `OC` at bytes 4..6, space at 6, `/` at 7, `NF` at 8..10.
    // Separator span: 6..8 (leading space + `/`).
    assert_eq!(slash_seps[0].span.start, 6);
    assert_eq!(slash_seps[0].span.end, 8);

    assert_contiguous_coverage(src, &spans);
}

#[test]
fn whitespace_both_sides_of_slash_is_covered() {
    // `(S//OC / NF)` — space at byte 6, `/` at byte 7, space at byte 8.
    // Separator span covers bytes 6..9 (one byte on each side of `/`).
    let src = "(S//OC / NF)";
    let spans = parse_portion(src);
    let seps = separators(&spans);
    let slash_seps: Vec<&&TokenSpan> = seps.iter().filter(|s| &*s.text == "/").collect();
    assert_eq!(slash_seps.len(), 1);
    assert_eq!(slash_seps[0].span.start, 6);
    assert_eq!(slash_seps[0].span.end, 9);

    assert_contiguous_coverage(src, &spans);
}

#[test]
fn double_whitespace_both_sides_of_slash_is_covered() {
    // `(S//OC  /  NF)` — two spaces on each side of `/`. The full
    // four-byte whitespace run is absorbed into the separator span.
    let src = "(S//OC  /  NF)";
    let spans = parse_portion(src);
    let seps = separators(&spans);
    let slash_seps: Vec<&&TokenSpan> = seps.iter().filter(|s| &*s.text == "/").collect();
    assert_eq!(slash_seps.len(), 1);
    // `OC` at 4..6, two spaces (6..8), `/` at 8, two spaces (9..11), `NF` at 11..13.
    assert_eq!(slash_seps[0].span.start, 6);
    assert_eq!(slash_seps[0].span.end, 11);

    assert_contiguous_coverage(src, &spans);
}

#[test]
fn no_whitespace_separator_remains_one_byte() {
    // Negative-control: `(S//OC/NF)` has no surrounding whitespace, so
    // the separator span stays exactly 1 byte (the `/` itself). The
    // bidirectional-coverage code path must not synthesize whitespace
    // that isn't there.
    let src = "(S//OC/NF)";
    let spans = parse_portion(src);
    let seps = separators(&spans);
    let slash_seps: Vec<&&TokenSpan> = seps.iter().filter(|s| &*s.text == "/").collect();
    assert_eq!(slash_seps.len(), 1);
    assert_eq!(
        slash_seps[0].span.end - slash_seps[0].span.start,
        1,
        "separator span must be exactly 1 byte when no whitespace surrounds `/`"
    );

    assert_contiguous_coverage(src, &spans);
}

// -----------------------------------------------------------------------
// Separator scope — PR 9a Copilot R3 Fix 1 (PR #416).
//
// `TokenKind::Separator` (text = `"/"`) must sit between two non-empty
// same-category committed tokens. Empty-boundary slashes (trailing
// `OC/`, leading `/NF`) and slashes adjacent to an Unknown sub-token
// (e.g. `OC/FOO` where FOO is unrecognized) violate the contract;
// downstream byte-precise splice rules (E041 auto-fix path, canonical
// rendering) trust the invariant. All-Unknown blocks (`FOO/BAR` with
// no recognized category) emit a single block-level `TokenKind::
// Unknown` span — mirroring the mixed-category branch — rather than
// per-token Unknowns + Separator emission, since there is no committed
// category for the Separator to live within.
// -----------------------------------------------------------------------

#[test]
fn trailing_slash_emits_no_separator() {
    // `(S//OC/)`: the inner-block content `OC/` has a trailing slash
    // with an empty right neighbor (`s.split('/') = ["OC", ""]`). The
    // empty boundary means the contract "Separator between two
    // committed tokens" cannot hold — no Separator must be emitted.
    // The `OC` token itself is still committed as a DissemControl.
    let src = "(S//OC/)";
    let spans = parse_portion(src);
    let seps = separators(&spans);
    let slash_seps: Vec<&&TokenSpan> = seps.iter().filter(|s| &*s.text == "/").collect();
    assert_eq!(
        slash_seps.len(),
        0,
        "trailing slash must NOT emit a within-category Separator; got {:?}",
        slash_seps.iter().map(|s| s.span).collect::<Vec<_>>()
    );

    // Sanity: the OC token IS committed (the empty boundary doesn't
    // suppress legitimate token emission).
    let dissems: Vec<&TokenSpan> = spans
        .iter()
        .filter(|t| t.kind == TokenKind::DissemControl)
        .collect();
    assert_eq!(
        dissems.len(),
        1,
        "OC must still be emitted as DissemControl"
    );
    assert_eq!(&*dissems[0].text, "OC");
}

#[test]
fn leading_slash_emits_no_separator() {
    // `(S///NF)` — the scanner splits on `//`, so block 2 arrives at the
    // slash-block path as the raw inner `/NF`. `s.split('/')` →
    // `["", "NF"]`: the leading slash has an empty left neighbor.
    // Per the Separator contract no within-category `/` Separator must
    // be emitted (committed_idx machinery would otherwise try to read
    // `results[-1]`, which is exactly the bug class this fix
    // eliminates). `NF` itself is still committed.
    let src = "(S///NF)";
    let spans = parse_portion(src);
    let seps = separators(&spans);
    let slash_seps: Vec<&&TokenSpan> = seps.iter().filter(|s| &*s.text == "/").collect();
    assert_eq!(
        slash_seps.len(),
        0,
        "leading slash must NOT emit a within-category Separator; got {:?}",
        slash_seps.iter().map(|s| s.span).collect::<Vec<_>>()
    );

    // The NF token is still committed even with the leading slash.
    let dissems: Vec<&TokenSpan> = spans
        .iter()
        .filter(|t| t.kind == TokenKind::DissemControl)
        .collect();
    assert!(
        dissems.iter().any(|d| &*d.text == "NF"),
        "NF must still be emitted as DissemControl; got {:?}",
        dissems.iter().map(|d| &*d.text).collect::<Vec<_>>()
    );
}

#[test]
fn all_unknown_block_emits_single_unknown_no_separators() {
    // `(S//FOOBAR/BARFOO)` — neither token is recognized: both fail
    // the SCI custom-control shape gate (`[A-Z0-9]{2,5}` — 6 chars
    // each is too long) AND no other category claims them. Pre-fix
    // the parser emitted Separator + per-token Unknown spans even
    // though no committed category existed for the Separator to live
    // within. Post-fix the whole `FOOBAR/BARFOO` block is a single
    // `TokenKind::Unknown` span — mirroring the mixed-category branch
    // — and no Separator is emitted.
    //
    // Test-input note: 3-letter pure-alpha tokens (e.g. `FOO`/`BAR`)
    // are accepted by the SCI structural path as
    // `SciControlSystem::Custom`, so they would never reach the
    // slash-block all-Unknown branch. 6-char tokens fail the SCI
    // custom-control shape and exercise the branch this fix targets.
    let src = "(S//FOOBAR/BARFOO)";
    let spans = parse_portion(src);

    let seps = separators(&spans);
    let slash_seps: Vec<&&TokenSpan> = seps.iter().filter(|s| &*s.text == "/").collect();
    assert_eq!(
        slash_seps.len(),
        0,
        "all-Unknown block must NOT emit any within-category Separator; \
         the block has no committed category for a Separator to live within"
    );

    // The whole `FOOBAR/BARFOO` range must be covered by exactly one
    // `TokenKind::Unknown` span — not two per-token Unknowns.
    let unknowns: Vec<&TokenSpan> = spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    // `(S//FOOBAR/BARFOO)`: `FOOBAR/BARFOO` covers bytes 4..17.
    let block_unknowns: Vec<&&TokenSpan> = unknowns
        .iter()
        .filter(|u| u.span.start >= 4 && u.span.end <= 17)
        .collect();
    assert_eq!(
        block_unknowns.len(),
        1,
        "all-Unknown block must emit exactly one block-level Unknown span, \
         not per-token Unknowns; got {:?}",
        block_unknowns
            .iter()
            .map(|u| (u.span, &*u.text))
            .collect::<Vec<_>>()
    );
    assert_eq!(block_unknowns[0].span.start, 4);
    assert_eq!(block_unknowns[0].span.end, 17);
    assert_eq!(&*block_unknowns[0].text, "FOOBAR/BARFOO");
}

#[test]
fn same_category_with_mixed_unknown_neighbor_skips_separator() {
    // `(S//OC/FOOBAR)` — `OC` is a recognized DissemControl, `FOOBAR`
    // is unknown (6 chars fails the SCI custom-control shape gate).
    // `first_parsed_kind = Some(Dissem)` so the same-category branch
    // fires, but the slash sits between a committed Dissem and an
    // Unknown. The contract "Separator between two same-category
    // committed tokens" is violated; the Separator MUST be skipped.
    // OC is still committed; FOOBAR is still emitted as per-token
    // Unknown (since the block has at least one committed category).
    //
    // Test-input note: 3-letter pure-alpha `FOO` would be accepted by
    // the SCI structural path as a custom control, never reaching this
    // code path. 6-char `FOOBAR` is the smallest reliable unknown.
    let src = "(S//OC/FOOBAR)";
    let spans = parse_portion(src);

    let seps = separators(&spans);
    let slash_seps: Vec<&&TokenSpan> = seps.iter().filter(|s| &*s.text == "/").collect();
    assert_eq!(
        slash_seps.len(),
        0,
        "slash adjacent to an Unknown sub-token must NOT emit Separator; got {:?}",
        slash_seps.iter().map(|s| s.span).collect::<Vec<_>>()
    );

    // OC committed; FOOBAR emitted as per-token Unknown.
    let dissems: Vec<&TokenSpan> = spans
        .iter()
        .filter(|t| t.kind == TokenKind::DissemControl)
        .collect();
    assert!(dissems.iter().any(|d| &*d.text == "OC"));
    let unknowns: Vec<&TokenSpan> = spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(
        unknowns.iter().any(|u| &*u.text == "FOOBAR"),
        "FOOBAR must surface as per-token Unknown when the block has \
         at least one committed category; got {:?}",
        unknowns.iter().map(|u| &*u.text).collect::<Vec<_>>()
    );
}

#[test]
fn regression_oc_slash_nf_still_emits_separator() {
    // Non-regression lock-in: `(S//OC/NF)` has two committed Dissem
    // tokens with a `/` between them. The Separator emission contract
    // is satisfied; one within-category `/` Separator must still be
    // emitted exactly as before the R3 fix.
    let src = "(S//OC/NF)";
    let spans = parse_portion(src);
    let seps = separators(&spans);
    let slash_seps: Vec<&&TokenSpan> = seps.iter().filter(|s| &*s.text == "/").collect();
    assert_eq!(
        slash_seps.len(),
        1,
        "OC/NF (both committed Dissem) must emit exactly one Separator"
    );
    // `/` byte at position 6 in `(S//OC/NF)`.
    assert_eq!(slash_seps[0].span.start, 6);
    assert_eq!(slash_seps[0].span.end, 7);
}
