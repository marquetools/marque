// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! NNPI (Naval Nuclear Propulsion Information) non-IC dissem recognition.
//!
//! ODNI registers `NNPI` as a non-IC dissemination control in
//! `CVEnumISMNonIC.xml` (value `NNPI`, description
//! `NAVAL NUCLEAR PROPULSION INFORMATION`). CAPCO-2016 §G.1 Table 4
//! (Register of Authorized Markings) and §H.9 (Non-IC Dissemination
//! Control Markings) do not enumerate NNPI, because the marking is
//! governed by separate statutory authority (10 USC 7314 / 50 USC
//! §2511; DOE / Naval Nuclear Propulsion Program) rather than IC
//! marking policy. Per Constitution VIII, an ODNI-registered token
//! whose authority lives outside CAPCO is admissible via the ODNI
//! schema citation when the normative CAPCO sections cited above
//! are silent on the marking itself.
//!
//! The pre-fix `NonIcDissem` enum in `marque-ism::attrs` was missing
//! the `Nnpi` variant, so the strict parser dispatched
//! `(U//NNPI)` → `parse_non_ic_full_form("NNPI")` → `None` → Unknown
//! span, and the rule layer surfaced 37 spurious E008 errors across
//! 13 documents in the marked corpus.
//!
//! This test pins the contract: NNPI is recognized in both portion
//! and banner long forms, no Unknown spans, no E008.

use marque_core::Parser;
use marque_ism::attrs::{NonIcDissem, TokenKind};
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
fn nnpi_portion_form_recognized() {
    let attrs = parse_portion("(U//NNPI)");
    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(
        unknown.is_empty(),
        "(U//NNPI) must parse with no Unknown spans; got {:?}",
        unknown
            .iter()
            .map(|t| (&*t.text, t.span.start, t.span.end))
            .collect::<Vec<_>>()
    );

    let non_ic: Vec<NonIcDissem> = attrs.non_ic_dissem.iter().map(|n| n.value).collect();
    assert_eq!(non_ic, vec![NonIcDissem::Nnpi]);
}

#[test]
fn nnpi_portion_after_classification() {
    let attrs = parse_portion("(C//NNPI)");
    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(unknown.is_empty(), "no Unknown spans expected");
    let non_ic: Vec<NonIcDissem> = attrs.non_ic_dissem.iter().map(|n| n.value).collect();
    assert_eq!(non_ic, vec![NonIcDissem::Nnpi]);
}

#[test]
fn nnpi_banner_long_form_recognized() {
    // The banner long-form `NAVAL NUCLEAR PROPULSION INFORMATION`
    // is in `MARKING_FORMS` (banner ↔ portion ↔ title), so
    // `parse_non_ic_full_form` resolves it via `title_to_portion`
    // → `NNPI` → `NonIcDissem::Nnpi`. This test pins that the
    // banner-form path works once the enum variant is registered.
    let attrs = parse_banner("SECRET//NAVAL NUCLEAR PROPULSION INFORMATION");
    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(
        unknown.is_empty(),
        "banner long-form must parse with no Unknown spans; got {:?}",
        unknown
            .iter()
            .map(|t| (&*t.text, t.span.start, t.span.end))
            .collect::<Vec<_>>()
    );
    let non_ic: Vec<NonIcDissem> = attrs.non_ic_dissem.iter().map(|n| n.value).collect();
    assert_eq!(non_ic, vec![NonIcDissem::Nnpi]);
}

#[test]
fn nnpi_banner_portion_form_recognized() {
    // Banner with the abbreviated `NNPI` (same form for banner and
    // portion per MARKING_FORMS row).
    let attrs = parse_banner("SECRET//NNPI");
    let unknown: Vec<_> = attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(unknown.is_empty(), "no Unknown spans expected");
    let non_ic: Vec<NonIcDissem> = attrs.non_ic_dissem.iter().map(|n| n.value).collect();
    assert_eq!(non_ic, vec![NonIcDissem::Nnpi]);
}

#[test]
fn nnpi_does_not_carry_noforn() {
    assert!(
        !NonIcDissem::Nnpi.carries_noforn(),
        "NNPI is an information-subject-matter marking, not a NOFORN-bearing variant \
         like SBU-NF or LES-NF"
    );
}

#[test]
fn nnpi_propagates_to_classified_banner() {
    // Locks the propagation decision: NNPI sits with EXDIS / NODIS /
    // LES / LES-NF / SSI in the propagating set, not with LIMDIS /
    // SBU / SBU-NF (the unclassified-only cluster per §H.9 p170 /
    // p176 / p178). Authoritative basis lives in the doc-comment
    // table on `NonIcDissem::propagates_to_classified_banner`; this
    // test pins the bit to prevent silent regression if a future
    // refactor reshuffles the match arms.
    assert!(
        NonIcDissem::Nnpi.propagates_to_classified_banner(),
        "NNPI must propagate to classified banner — it is a subject-matter \
         marking that requires banner visibility regardless of classification level"
    );
}
