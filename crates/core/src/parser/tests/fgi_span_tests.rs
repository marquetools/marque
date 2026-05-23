use super::*;

// Issue #545: per-country span emission. `parse_fgi_marker_with_spans`
// emits one `TokenKind::FgiOwnershipTrigraph` per shape-admitted
// country token in the ownership list, alongside the block-level
// `TokenKind::FgiMarker` span the block-walker pushes on its own.
// These tests pin the contract `FgiOwnershipTrigraphSuggestRule`
// (capco:portion.fgi.ownership-trigraph-suggest) depends on.

/// Helper: drive `parse_fgi_marker_with_spans` with a fresh
/// `token_spans` buffer at offset zero and return both the
/// `FgiMarker` result and the emitted span list. Mirrors the
/// pattern in `parse_fgi_marker` (the test wrapper) but exposes
/// the spans for assertion.
fn parse_fgi_marker_capturing_spans(
    s: &str,
    base: usize,
) -> (Option<FgiMarker>, SmallVec<[TokenSpan; 16]>) {
    let mut spans: SmallVec<[TokenSpan; 16]> = SmallVec::new();
    let marker = parse_fgi_marker_with_spans(s, base, &mut spans);
    (marker, spans)
}

#[test]
fn fgi_marker_emits_per_country_ownership_spans_abbrev_prefix() {
    // Single country: `FGI USA` at block_offset 100.
    // `"FGI "` is 4 bytes, so `USA` starts at absolute offset
    // 100 + 4 = 104 and ends at 107.
    let (marker, spans) = parse_fgi_marker_capturing_spans("FGI USA", 100);
    assert!(matches!(marker, Some(FgiMarker::Acknowledged { .. })));
    let ownership: Vec<&TokenSpan> = spans
        .iter()
        .filter(|t| t.kind == TokenKind::FgiOwnershipTrigraph)
        .collect();
    assert_eq!(ownership.len(), 1, "expected exactly one ownership span");
    assert_eq!(ownership[0].text.as_str(), "USA");
    assert_eq!(ownership[0].span.start, 104);
    assert_eq!(ownership[0].span.end, 107);
}

#[test]
fn fgi_marker_emits_per_country_ownership_spans_multi_country() {
    // Multiple countries: `FGI USA GBR NATO` at block_offset 0.
    // Offsets (within "FGI USA GBR NATO"):
    //   `USA` at 4..7
    //   `GBR` at 8..11
    //   `NATO` at 12..16
    let (marker, spans) = parse_fgi_marker_capturing_spans("FGI USA GBR NATO", 0);
    assert!(matches!(marker, Some(FgiMarker::Acknowledged { .. })));
    let ownership: Vec<&TokenSpan> = spans
        .iter()
        .filter(|t| t.kind == TokenKind::FgiOwnershipTrigraph)
        .collect();
    assert_eq!(ownership.len(), 3, "expected three ownership spans");
    assert_eq!(ownership[0].text.as_str(), "USA");
    assert_eq!(ownership[0].span.start, 4);
    assert_eq!(ownership[0].span.end, 7);
    assert_eq!(ownership[1].text.as_str(), "GBR");
    assert_eq!(ownership[1].span.start, 8);
    assert_eq!(ownership[1].span.end, 11);
    assert_eq!(ownership[2].text.as_str(), "NATO");
    assert_eq!(ownership[2].span.start, 12);
    assert_eq!(ownership[2].span.end, 16);
}

#[test]
fn fgi_marker_emits_per_country_ownership_spans_long_form_prefix() {
    // Long-form prefix is 31 bytes (`"FOREIGN GOVERNMENT INFORMATION "`).
    // Verify offset arithmetic on the long-form path.
    let input = "FOREIGN GOVERNMENT INFORMATION USA";
    let (marker, spans) = parse_fgi_marker_capturing_spans(input, 0);
    assert!(matches!(marker, Some(FgiMarker::Acknowledged { .. })));
    let ownership: Vec<&TokenSpan> = spans
        .iter()
        .filter(|t| t.kind == TokenKind::FgiOwnershipTrigraph)
        .collect();
    assert_eq!(ownership.len(), 1);
    assert_eq!(ownership[0].text.as_str(), "USA");
    // 31-byte prefix + 0-byte offset of "USA" within the rest
    // = absolute offset 31.
    assert_eq!(ownership[0].span.start, 31);
    assert_eq!(ownership[0].span.end, 34);
}

#[test]
fn fgi_marker_emits_per_country_ownership_spans_for_unregistered_tokens() {
    // The whole point of issue #545: `XX` and `ZZZ` are shape-
    // admitted-but-unregistered ownership tokens. The parser
    // produces `FgiOwnershipTrigraph` spans for them; the
    // rule layer (S004-style suggest) handles registry validation.
    let (marker, spans) = parse_fgi_marker_capturing_spans("FGI XX", 0);
    assert!(matches!(marker, Some(FgiMarker::Acknowledged { .. })));
    let ownership: Vec<&TokenSpan> = spans
        .iter()
        .filter(|t| t.kind == TokenKind::FgiOwnershipTrigraph)
        .collect();
    assert_eq!(ownership.len(), 1);
    assert_eq!(ownership[0].text.as_str(), "XX");

    let (marker, spans) = parse_fgi_marker_capturing_spans("FGI ZZZ", 0);
    assert!(matches!(marker, Some(FgiMarker::Acknowledged { .. })));
    let ownership: Vec<&TokenSpan> = spans
        .iter()
        .filter(|t| t.kind == TokenKind::FgiOwnershipTrigraph)
        .collect();
    assert_eq!(ownership.len(), 1);
    assert_eq!(ownership[0].text.as_str(), "ZZZ");
}

#[test]
fn fgi_marker_concealed_emits_no_ownership_spans() {
    // Bare `FGI` (concealed) has no ownership list → no spans.
    let (marker, spans) = parse_fgi_marker_capturing_spans("FGI", 0);
    assert!(matches!(marker, Some(FgiMarker::SourceConcealed)));
    assert!(
        !spans
            .iter()
            .any(|t| t.kind == TokenKind::FgiOwnershipTrigraph),
        "concealed FGI must not emit any FgiOwnershipTrigraph spans"
    );
}

#[test]
fn fgi_marker_parse_failure_emits_no_ownership_spans() {
    // FR-016 closure: a parse failure must NOT leak per-country
    // spans into the AST. `FGI FVEY` parses to `None` because
    // distribution-list tetragraphs reject in the ownership slot.
    // The `pending` staging in `parse_fgi_marker_with_spans`
    // guarantees the failure path is span-clean.
    let (marker, spans) = parse_fgi_marker_capturing_spans("FGI FVEY", 0);
    assert!(
        marker.is_none(),
        "FGI FVEY must fail to parse (distribution tetragraph in ownership slot)"
    );
    assert!(
        !spans
            .iter()
            .any(|t| t.kind == TokenKind::FgiOwnershipTrigraph),
        "failed FGI parse must not leak FgiOwnershipTrigraph spans"
    );
}

#[test]
fn conflict_us_and_nato() {
    let parsed = parse_banner("SECRET//NATO SECRET//NOFORN");
    match &parsed.attrs.classification {
        Some(MarkingClassification::Conflict { us, foreign }) => {
            assert_eq!(*us, Classification::Secret);
            assert!(matches!(
                foreign.as_ref(),
                ForeignClassification::Nato(NatoClassification::NatoSecret)
            ));
        }
        other => panic!("expected Conflict, got: {other:?}"),
    }
}

#[test]
fn conflict_level_escalation() {
    // SECRET + COSMIC TOP SECRET → US escalates to TopSecret
    let parsed = parse_banner("SECRET//COSMIC TOP SECRET//NOFORN");
    match &parsed.attrs.classification {
        Some(MarkingClassification::Conflict { us, foreign }) => {
            assert_eq!(*us, Classification::TopSecret);
            assert!(matches!(
                foreign.as_ref(),
                ForeignClassification::Nato(NatoClassification::CosmicTopSecret)
            ));
        }
        other => panic!("expected Conflict with escalation, got: {other:?}"),
    }
}

#[test]
fn restricted_classification_parses() {
    let parsed = parse_banner("RESTRICTED//NF");
    assert_eq!(
        parsed.attrs.classification,
        Some(MarkingClassification::Us(Classification::Restricted)),
    );
}

#[test]
fn restricted_portion_parses() {
    let parsed = parse_portion("(R//NF)");
    assert_eq!(
        parsed.attrs.classification,
        Some(MarkingClassification::Us(Classification::Restricted)),
    );
}
