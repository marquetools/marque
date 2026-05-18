// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 9a Copilot R2 Fix 3 — SCI long-form span emission invariant.
//!
//! When the parser recognizes a deprecated SCI long-form (HUMINT,
//! COMINT, ECI, EL, ENDSEAL, KDK, KLONDIKE, ...), it emits a
//! `ParsedSciMarking` entry alongside structural-parser markings. The
//! rule layer (E061 / E062 / E063 + any future SCI rule) locates byte
//! anchors by filtering `attrs.token_spans` for
//! `TokenKind::SciSystem` and indexing by position into the
//! `sci_markings` vector. Without a matching `SciSystem` span emission
//! on the long-form parser path, that lookup silently falls through
//! to `Span::new(0, 0)` and the resulting diagnostic / text correction
//! anchors at byte 0..0 of the input — silent audit corruption per
//! Constitution Principle V.
//!
//! Invariant: for every `sci_markings` entry, exactly one
//! `TokenKind::SciSystem` span MUST be present in `attrs.token_spans`.
//!
//! Authority for the rule-layer behavior the invariant supports:
//! CAPCO-2016 §H.4 pp 62, 70.

use marque_core::Parser;
use marque_ism::attrs::{TokenKind, TokenSpan};
use marque_ism::span::{MarkingCandidate, MarkingType};
use marque_ism::token_set::CapcoTokenSet;
use marque_scheme::Span;

/// Parsed-attrs surface needed by the tests. Returned by reference so the
/// caller doesn't have to manage `ParsedAttrs<'src>` lifetimes; we copy
/// out only what we need: token-span vector + sci_markings count.
struct ParsedSnapshot {
    token_spans: Vec<TokenSpan>,
    sci_markings_count: usize,
}

fn parse_portion(text: &str) -> ParsedSnapshot {
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
    ParsedSnapshot {
        token_spans: parsed.attrs.token_spans.to_vec(),
        sci_markings_count: parsed.attrs.sci_markings.len(),
    }
}

/// Count `TokenKind::SciSystem` spans in `token_spans`.
fn count_sci_system_spans(spans: &[TokenSpan]) -> usize {
    spans
        .iter()
        .filter(|t| t.kind == TokenKind::SciSystem)
        .count()
}

#[test]
fn humint_long_form_emits_one_sci_system_span() {
    // `(SECRET//HUMINT//NOFORN)` — the long-form parser path recognizes
    // HUMINT as the deprecated form of HCS. Per the invariant, exactly
    // one SciSystem span must be present so rule-layer span anchoring
    // (E062 looking up `sys_spans[0]`) does not fall through to
    // `Span::new(0, 0)`.
    let snap = parse_portion("(SECRET//HUMINT//NOFORN)");
    assert_eq!(
        snap.sci_markings_count, 1,
        "HUMINT must produce exactly one sci_markings entry"
    );
    assert_eq!(
        count_sci_system_spans(&snap.token_spans),
        1,
        "long-form HUMINT must emit exactly one TokenKind::SciSystem span"
    );

    // The emitted span must cover the source bytes of `HUMINT`, not
    // collapse to byte 0..0. The rule layer reads spans for byte
    // anchoring; a 0..0 span anchors diagnostics at the start of the
    // input.
    let sys_span = snap
        .token_spans
        .iter()
        .find(|t| t.kind == TokenKind::SciSystem)
        .expect("SciSystem span present");
    assert!(
        sys_span.span.start > 0 && sys_span.span.end > sys_span.span.start,
        "SciSystem span must be non-zero-length and non-degenerate; got {:?}",
        sys_span.span
    );

    // The span should cover `HUMINT` in the source — bytes 9..15.
    // `(SECRET//HUMINT//NOFORN)`: `(`=0, `SECRET`=1..7, `//`=7..9,
    // `HUMINT`=9..15, `//`=15..17, `NOFORN`=17..23, `)`=23.
    assert_eq!(sys_span.span.start, 9);
    assert_eq!(sys_span.span.end, 15);
    assert_eq!(
        sys_span.span.as_str(b"(SECRET//HUMINT//NOFORN)").unwrap(),
        "HUMINT"
    );
}

#[test]
fn comint_long_form_emits_one_sci_system_span() {
    // `(SECRET//COMINT//NOFORN)` — COMINT is the deprecated long-form
    // of SI. Same invariant: one SciSystem span per sci_markings
    // entry.
    let snap = parse_portion("(SECRET//COMINT//NOFORN)");
    assert_eq!(snap.sci_markings_count, 1);
    assert_eq!(count_sci_system_spans(&snap.token_spans), 1);

    let sys_span = snap
        .token_spans
        .iter()
        .find(|t| t.kind == TokenKind::SciSystem)
        .expect("SciSystem span present");
    assert!(sys_span.span.start > 0);
    assert!(sys_span.span.end > sys_span.span.start);
}

#[test]
fn structural_parser_baseline_one_sci_system_span() {
    // Baseline: the structural parser (the path the long-form parser
    // path mirrors) already emits one SciSystem span per marking. This
    // test pins the invariant we expect to hold across BOTH recognizer
    // paths — structural and long-form — so a future refactor that
    // alters one path can't drift the other without flipping this
    // test.
    let snap = parse_portion("(SECRET//HCS//NOFORN)");
    assert_eq!(snap.sci_markings_count, 1);
    assert_eq!(count_sci_system_spans(&snap.token_spans), 1);
}

#[test]
fn long_form_and_structural_produce_identical_span_count() {
    // The whole point of the dual-emission fix: a rule that filters
    // `attrs.token_spans` by `TokenKind::SciSystem` and indexes by
    // sci_markings position MUST see the same span count whether the
    // input used the long form or the canonical short form. Without
    // the fix, the long-form path produces 0 SciSystem spans and the
    // structural path produces 1.
    let long = parse_portion("(SECRET//HUMINT//NOFORN)");
    let short = parse_portion("(SECRET//HCS//NOFORN)");
    assert_eq!(
        count_sci_system_spans(&long.token_spans),
        count_sci_system_spans(&short.token_spans),
        "SciSystem span count must match across long-form and short-form inputs"
    );
}
