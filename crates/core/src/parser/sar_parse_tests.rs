use super::*;

// Direct unit tests for [`parse_sar_category`] plus integration-level
// tests that exercise the dispatch from `parse_marking_string`.

use super::tests::CanonicalParsed;
use marque_ism::span::{MarkingCandidate, MarkingType};
use marque_ism::token_set::CapcoTokenSet;
use marque_scheme::Span;

// ---------------------------------------------------------------------
// Direct subparser tests
// ---------------------------------------------------------------------

#[test]
fn single_program_no_compartments() {
    let (marking, spans) = parse_sar_category("SAR-BP", 0).expect("grammar accepts SAR-BP");
    assert_eq!(marking.indicator, SarIndicator::Abbrev);
    assert_eq!(marking.programs.len(), 1);
    assert_eq!(&*marking.programs[0].identifier, "BP");
    assert_eq!(marking.programs[0].compartments.len(), 0);
    // Spans: one indicator + one program.
    assert_eq!(
        spans
            .iter()
            .filter(|s| s.kind == TokenKind::SarIndicator)
            .count(),
        1
    );
    assert_eq!(
        spans
            .iter()
            .filter(|s| s.kind == TokenKind::SarProgram)
            .count(),
        1
    );
}

#[test]
fn three_programs_no_compartments() {
    let (marking, _) =
        parse_sar_category("SAR-BP/CD/XR", 0).expect("grammar accepts three programs");
    assert_eq!(marking.programs.len(), 3);
    let ids: Vec<&str> = marking.programs.iter().map(|p| &*p.identifier).collect();
    assert_eq!(ids, vec!["BP", "CD", "XR"]);
    for p in marking.programs.iter() {
        assert_eq!(p.compartments.len(), 0);
    }
}

#[test]
fn program_with_single_compartment() {
    let (marking, _) = parse_sar_category("SAR-BP-J12", 0).expect("grammar accepts");
    assert_eq!(marking.programs.len(), 1);
    let p = &marking.programs[0];
    assert_eq!(&*p.identifier, "BP");
    assert_eq!(p.compartments.len(), 1);
    assert_eq!(&*p.compartments[0].identifier, "J12");
    assert_eq!(p.compartments[0].sub_compartments.len(), 0);
}

#[test]
fn program_with_compartment_and_sub_compartment() {
    let (marking, _) = parse_sar_category("SAR-BP-J12 J54", 0).expect("grammar accepts");
    let p = &marking.programs[0];
    assert_eq!(p.compartments.len(), 1);
    let c = &p.compartments[0];
    assert_eq!(&*c.identifier, "J12");
    assert_eq!(c.sub_compartments.len(), 1);
    assert_eq!(&*c.sub_compartments[0], "J54");
}

#[test]
fn canonical_h5_p100_multi_program_example() {
    // The §H.5 p100 canonical decomposition:
    //   BP → [J12 (+ J54), K15]
    //   CD → [YYY (+ 456, 689)]
    //   XR → [XRA (+ RB)]
    let block = "SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB";
    let (marking, spans) = parse_sar_category(block, 0).expect("grammar accepts");

    assert_eq!(marking.indicator, SarIndicator::Abbrev);
    assert_eq!(marking.programs.len(), 3);

    // BP
    let bp = &marking.programs[0];
    assert_eq!(&*bp.identifier, "BP");
    assert_eq!(bp.compartments.len(), 2);
    assert_eq!(&*bp.compartments[0].identifier, "J12");
    assert_eq!(
        bp.compartments[0]
            .sub_compartments
            .iter()
            .map(|s| &**s)
            .collect::<Vec<_>>(),
        vec!["J54"]
    );
    assert_eq!(&*bp.compartments[1].identifier, "K15");
    assert_eq!(bp.compartments[1].sub_compartments.len(), 0);

    // CD
    let cd = &marking.programs[1];
    assert_eq!(&*cd.identifier, "CD");
    assert_eq!(cd.compartments.len(), 1);
    assert_eq!(&*cd.compartments[0].identifier, "YYY");
    assert_eq!(
        cd.compartments[0]
            .sub_compartments
            .iter()
            .map(|s| &**s)
            .collect::<Vec<_>>(),
        vec!["456", "689"]
    );

    // XR
    let xr = &marking.programs[2];
    assert_eq!(&*xr.identifier, "XR");
    assert_eq!(xr.compartments.len(), 1);
    assert_eq!(&*xr.compartments[0].identifier, "XRA");
    assert_eq!(
        xr.compartments[0]
            .sub_compartments
            .iter()
            .map(|s| &**s)
            .collect::<Vec<_>>(),
        vec!["RB"]
    );

    // Spot-check span offsets: the indicator is at [0, 4) and the first
    // program "BP" is at [4, 6).
    let indicator = spans
        .iter()
        .find(|s| s.kind == TokenKind::SarIndicator)
        .unwrap();
    assert_eq!(indicator.span, Span::new(0, 4));
    assert_eq!(&*indicator.text, "SAR-");
    let first_prog = spans
        .iter()
        .find(|s| s.kind == TokenKind::SarProgram)
        .unwrap();
    assert_eq!(first_prog.span, Span::new(4, 6));
    assert_eq!(&*first_prog.text, "BP");
}

#[test]
fn full_form_single_program_with_space() {
    // `SPECIAL ACCESS REQUIRED-BUTTER POPCORN` — full form allows spaces
    // inside the nickname. No compartment decomposition at the lexical
    // level (see spec §R2 ambiguity note).
    let (marking, spans) = parse_sar_category("SPECIAL ACCESS REQUIRED-BUTTER POPCORN", 0).unwrap();
    assert_eq!(marking.indicator, SarIndicator::Full);
    assert_eq!(marking.programs.len(), 1);
    assert_eq!(&*marking.programs[0].identifier, "BUTTER POPCORN");
    assert_eq!(marking.programs[0].compartments.len(), 0);

    // Indicator span is 24 bytes: `SPECIAL ACCESS REQUIRED-`.
    let indicator = spans
        .iter()
        .find(|s| s.kind == TokenKind::SarIndicator)
        .unwrap();
    assert_eq!(&*indicator.text, "SPECIAL ACCESS REQUIRED-");
    assert_eq!(indicator.span, Span::new(0, 24));
}

#[test]
fn full_form_with_compartment_and_sub() {
    // The grammar permits compartments under a full-form program
    // identically to the abbreviated form. Program nickname may
    // contain spaces; compartments and sub-compartments are still
    // alphanumeric without spaces.
    let (marking, _spans) = parse_sar_category("SPECIAL ACCESS REQUIRED-BUTTER POPCORN-J12 J54", 0)
        .expect("grammar accepts full form with compartment");
    assert_eq!(marking.indicator, SarIndicator::Full);
    assert_eq!(marking.programs.len(), 1);
    let prog = &marking.programs[0];
    assert_eq!(&*prog.identifier, "BUTTER POPCORN");
    assert_eq!(prog.compartments.len(), 1);
    assert_eq!(&*prog.compartments[0].identifier, "J12");
    assert_eq!(prog.compartments[0].sub_compartments.len(), 1);
    assert_eq!(&*prog.compartments[0].sub_compartments[0], "J54");
}

#[test]
fn full_form_rejects_digits_or_hyphens_in_nickname() {
    // Full-form nickname may only contain uppercase letters and
    // spaces; digits or hyphens inside the nickname are parsed as
    // compartment boundaries (hyphen) or as a shape violation
    // (digits).
    assert!(parse_sar_category("SPECIAL ACCESS REQUIRED-123", 0).is_none());
}

#[test]
fn rejects_double_slash_inside_block() {
    // Defensive: the outer category-block splitter wouldn't hand us
    // `SAR-BP//CD` (it splits on `//` first). But if it somehow did,
    // `parse_sar_category` refuses because `//` is a category separator
    // that should never appear inside a single block. The caller
    // records the text as Unknown so E030 can flag the repeat form.
    assert!(parse_sar_category("SAR-BP//CD", 0).is_none());
}

#[test]
fn rejects_missing_hyphen() {
    assert!(parse_sar_category("SAR", 0).is_none());
}

#[test]
fn rejects_empty_program() {
    assert!(parse_sar_category("SAR-", 0).is_none());
}

#[test]
fn rejects_empty_string() {
    assert!(parse_sar_category("", 0).is_none());
}

#[test]
fn rejects_non_sar_prefix() {
    assert!(parse_sar_category("NOFORN", 0).is_none());
    assert!(parse_sar_category("SI", 0).is_none());
}

#[test]
fn rejects_program_id_out_of_2_3_length() {
    // Single-char program id.
    assert!(parse_sar_category("SAR-B", 0).is_none());
    // Four-char program id.
    assert!(parse_sar_category("SAR-BPCD", 0).is_none());
}

// ---------------------------------------------------------------------
// T089 / T090 / T091: FR-015 closure for parse_sar_program
//
// The parser-side admission for SAR program identifiers,
// compartments, and sub-compartments routes through the
// `marque-ism` predicates `SarProgram::admits_program_id_abbrev`,
// `SarProgram::admits_program_id_full`, and
// `SarCompartment::admits_identifier`. These tests pin the
// accept/reject boundary at the parser dispatch level —
// catching any future drift between the parser and the
// single-source-of-truth predicates in `marque-ism::attrs`.
// The predicates' own accept/reject sets are exhaustively
// tested in `marque_ism::attrs::sar_shape_tests`; these tests
// verify the parser actually calls them.
// ---------------------------------------------------------------------

#[test]
fn t089_program_id_abbrev_length_boundary() {
    // FR-015 / T089 regression. The 2-3 alnum gate is the
    // most observable boundary; if the parser ever falls back
    // to a length-only or class-only check (a pre-T089 bug
    // mode), one of these assertions will fail.

    // Length 1 (below the 2-char minimum) — must reject.
    assert!(
        parse_sar_category("SAR-X", 0).is_none(),
        "single-char program id must reject (below the §H.5 p101 \
         2-3 char bound)",
    );

    // Length 2 (the lower bound) — must accept and produce a
    // single program with the abbreviated identifier.
    let (marking, _spans) = parse_sar_category("SAR-XY", 0)
        .expect("2-char abbrev program id must accept (§H.5 p101 lower bound)");
    assert_eq!(marking.indicator, SarIndicator::Abbrev);
    assert_eq!(marking.programs.len(), 1);
    assert_eq!(&*marking.programs[0].identifier, "XY");

    // Length 3 (the upper bound) — must accept.
    let (marking, _spans) =
        parse_sar_category("SAR-XYZ", 0).expect("3-char abbrev program id must accept");
    assert_eq!(&*marking.programs[0].identifier, "XYZ");

    // Length 4 (above the 3-char maximum) — must reject.
    assert!(
        parse_sar_category("SAR-XYZW", 0).is_none(),
        "4-char program id must reject (above the §H.5 p101 \
         2-3 char bound)",
    );

    // Issue #280: SAR open-vocab tightening — lowercase is
    // rejected (CAPCO §A.6 p15 + §G.1 p36: Register entries are
    // uppercase; SAR has no CVE registry, so the shape gate is
    // the validation). Digits still admit.
    assert!(
        parse_sar_category("SAR-bp", 0).is_none(),
        "lowercase abbrev program id must reject (#280); decoder \
         handles demangling",
    );
    assert!(
        parse_sar_category("SAR-Bp", 0).is_none(),
        "mixed-case abbrev program id must reject (#280)",
    );
    let (marking, _spans) =
        parse_sar_category("SAR-99", 0).expect("digit-only abbrev id must accept");
    assert_eq!(&*marking.programs[0].identifier, "99");
}

#[test]
fn t090_compartment_identifier_admission() {
    // FR-015 / T090 regression. `parse_sar_program` must
    // delegate compartment admission to
    // `SarCompartment::admits_identifier`, not an inline
    // length-and-class check. The accept set is "≥1 ASCII
    // alnum"; the reject set covers the empty-segment and
    // punctuation cases.

    // Empty compartment after the program/compartment hyphen
    // — `SAR-BP-` produces an empty trailing segment that must
    // reject (mirrors `parse_sar_program`'s segment-empty guard
    // even though `admits_identifier(b"")` would also reject).
    assert!(
        parse_sar_category("SAR-BP-", 0).is_none(),
        "trailing hyphen with empty compartment must reject",
    );

    // Single-character compartment — manual silent on lower
    // bound beyond ≥1; marque admits length 1+. Pins the
    // marque interpretation noted in the predicate's doc
    // comment.
    let (marking, _spans) = parse_sar_category("SAR-BP-1", 0)
        .expect("single-char compartment id must accept (marque interpretation of §H.5 p99)");
    assert_eq!(marking.programs.len(), 1);
    assert_eq!(marking.programs[0].compartments.len(), 1);
    assert_eq!(&*marking.programs[0].compartments[0].identifier, "1");

    // Multi-char alnum compartment — Table 7 §H.5 p100 examples.
    let (marking, _spans) =
        parse_sar_category("SAR-BP-J12", 0).expect("alnum compartment id must accept");
    assert_eq!(&*marking.programs[0].compartments[0].identifier, "J12");
}

#[test]
fn t091_sub_compartment_identifier_admission() {
    // FR-015 / T091 regression. Sub-compartment admission goes
    // through the same `SarCompartment::admits_identifier`
    // predicate as the compartment slot — the manual places
    // both grammar positions under one rule
    // (CAPCO-2016 §H.5 pp 99-100).

    // Trailing space with no sub-compartment token — empty
    // sub-compartment must reject. `split_with_offsets(seg, ' ')`
    // produces an empty trailing token; `admits_identifier(b"")`
    // catches it.
    assert!(
        parse_sar_category("SAR-BP-J12 ", 0).is_none(),
        "trailing space with no sub-compartment token must reject",
    );

    // Single-char sub-compartment — admitted by the same
    // length-1+ rule.
    let (marking, _spans) =
        parse_sar_category("SAR-BP-J12 1", 0).expect("single-char sub-compartment id must accept");
    let comp = &marking.programs[0].compartments[0];
    assert_eq!(comp.sub_compartments.len(), 1);
    assert_eq!(&*comp.sub_compartments[0], "1");

    // Multi-char alnum sub-compartment — Table 7 §H.5 p100.
    let (marking, _spans) =
        parse_sar_category("SAR-BP-J12 J54", 0).expect("alnum sub-compartment id must accept");
    let comp = &marking.programs[0].compartments[0];
    assert_eq!(&*comp.sub_compartments[0], "J54");

    // Punctuation in sub-compartment — must reject. The
    // grammar separators `-`, `/`, and ` ` cannot be tested
    // here: `-` and `/` are consumed at the compartment /
    // program level before sub-compartment admission runs,
    // and ` ` is itself the sub-compartment separator. Any
    // other punctuation byte has no role in §H.5 and reaches
    // `admits_identifier`, where it is rejected.
    assert!(
        parse_sar_category("SAR-BP-J12 J.54", 0).is_none(),
        "punctuation (`.`) in sub-compartment must reject",
    );
    assert!(
        parse_sar_category("SAR-BP-J12 J_54", 0).is_none(),
        "punctuation (`_`) in sub-compartment must reject",
    );
}

// ---------------------------------------------------------------------
// Dispatch tests (through `parse_marking_string`)
// ---------------------------------------------------------------------

fn make_banner(text: &str) -> CanonicalParsed {
    let source = text.as_bytes();
    let tokens = CapcoTokenSet;
    let parser = Parser::new(&tokens);
    let candidate = MarkingCandidate {
        span: Span::new(0, source.len()),
        kind: MarkingType::Banner,
    };
    parser
        .parse(&candidate, source)
        .expect("parse succeeds")
        .into()
}

#[test]
fn banner_dispatch_populates_sar_markings() {
    let parsed = make_banner("TOP SECRET//SAR-BP//NOFORN");
    let sar = parsed
        .attrs
        .sar_markings
        .as_ref()
        .expect("SAR block must populate sar_markings");
    assert_eq!(sar.programs.len(), 1);
    assert_eq!(&*sar.programs[0].identifier, "BP");

    // Token-span mix must include both the indicator and program token.
    let kinds: Vec<TokenKind> = parsed.attrs.token_spans.iter().map(|t| t.kind).collect();
    assert!(kinds.contains(&TokenKind::SarIndicator));
    assert!(kinds.contains(&TokenKind::SarProgram));

    // Dissem accumulator still populated: NOFORN is present.
    assert!(
        parsed
            .attrs
            .dissem_iter()
            .any(|d| d == &marque_ism::DissemControl::Nf),
        "NOFORN must still be recognized after the SAR block"
    );
}

#[test]
fn banner_dispatch_multi_program_canonical() {
    // The §H.5 p100 canonical line as a full banner.
    let parsed = make_banner("SECRET//SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB//NOFORN");
    let sar = parsed.attrs.sar_markings.as_ref().expect("sar present");
    assert_eq!(sar.programs.len(), 3);
    let ids: Vec<&str> = sar.programs.iter().map(|p| &*p.identifier).collect();
    assert_eq!(ids, vec!["BP", "CD", "XR"]);

    // Token-span offsets are absolute into the banner string. Find the
    // SarIndicator and verify its byte slice.
    let src = parsed
        .attrs
        .token_spans
        .iter()
        .find(|t| t.kind == TokenKind::SarIndicator)
        .expect("SarIndicator span present");
    assert_eq!(&*src.text, "SAR-");
    // `SECRET//` is 8 bytes, so `SAR-` starts at offset 8.
    assert_eq!(src.span, Span::new(8, 12));
}

#[test]
fn second_sar_block_becomes_unknown() {
    // Two SAR category blocks: the first populates `sar_markings`; the
    // second is left as `Unknown` so rule E030 can flag the repeat.
    let parsed = make_banner("SECRET//SAR-BP//SAR-CD//NOFORN");
    let sar = parsed
        .attrs
        .sar_markings
        .as_ref()
        .expect("first SAR block populates sar_markings");
    assert_eq!(sar.programs.len(), 1);
    assert_eq!(&*sar.programs[0].identifier, "BP");

    // The `SAR-CD` block must appear as an Unknown span.
    let unknown_texts: Vec<&str> = parsed
        .attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .map(|t| &*t.text)
        .collect();
    assert!(
        unknown_texts.contains(&"SAR-CD"),
        "duplicate SAR block must be recorded as Unknown, got: {unknown_texts:?}",
    );
}
