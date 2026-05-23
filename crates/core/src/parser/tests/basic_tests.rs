use super::*;

// --- declass exemption in banner (E005 detection) ---

#[test]
fn banner_with_declass_exemption_populates_attrs() {
    // A banner string that (incorrectly) contains a declass exemption code.
    // parse_marking_string must populate declass_exemption so E005 can fire.
    let parsed = parse_banner("SECRET//25X1//NOFORN");
    assert!(
        parsed.attrs.declass_exemption.is_some(),
        "declass_exemption should be populated when 25X1 appears in banner"
    );
    use marque_ism::DeclassExemption;
    assert_eq!(
        parsed.attrs.declass_exemption,
        Some(DeclassExemption::X25x1)
    );
}

#[test]
fn portion_with_declass_exemption_populates_attrs() {
    let parsed = parse_portion("(SECRET//50X1-HUM)");
    assert!(parsed.attrs.declass_exemption.is_some());
}

// --- declass date in banner (E005 detection) ---

#[test]
fn banner_with_declass_date_populates_attrs() {
    let parsed = parse_banner("SECRET//20301231//NOFORN");
    assert_eq!(
        parsed.attrs.declassify_on,
        Some(marque_ism::IsmDate::Date(2030, 12, 31)),
        "declassify_on should be populated when YYYYMMDD appears in banner"
    );
}

#[test]
fn banner_with_four_digit_year_populates_attrs() {
    let parsed = parse_banner("SECRET//2035");
    assert_eq!(
        parsed.attrs.declassify_on,
        Some(marque_ism::IsmDate::Year(2035))
    );
}

// --- normal banner (no declass tokens) ---

#[test]
fn banner_without_declass_leaves_fields_none() {
    let parsed = parse_banner("TOP SECRET//SI//NOFORN");
    assert!(parsed.attrs.declassify_on.is_none());
    assert!(parsed.attrs.declass_exemption.is_none());
}

// --- is_declass_date helper ---

#[test]
fn is_declass_date_accepts_yyyymmdd() {
    assert!(is_declass_date("20301231"));
}

#[test]
fn is_declass_date_accepts_yyyy() {
    assert!(is_declass_date("2035"));
}

#[test]
fn is_declass_date_rejects_non_digit() {
    assert!(!is_declass_date("2030X231"));
    assert!(!is_declass_date("YYYYMMDD"));
}

#[test]
fn is_declass_date_rejects_wrong_length() {
    assert!(!is_declass_date("203012"));
    assert!(!is_declass_date("203012311"));
}

#[test]
fn is_declass_date_rejects_impossible_calendar_dates() {
    // Month 13 is impossible.
    assert!(!is_declass_date("20301340"));
    // Day 0 is impossible.
    assert!(!is_declass_date("20300100"));
    // 2003-02-31 doesn't exist (February has at most 29 days).
    assert!(!is_declass_date("20030231"));
    // 2003-04-31 doesn't exist (April has 30 days).
    assert!(!is_declass_date("20030431"));
}

// --- token spans ---

#[test]
fn token_spans_track_offsets_in_banner() {
    let parsed = parse_banner("TOP SECRET//SI//NF");
    let kinds: Vec<TokenKind> = parsed.attrs.token_spans.iter().map(|t| t.kind).collect();
    // Two separators + classification + sci + dissem.
    assert!(kinds.contains(&TokenKind::Separator));
    assert!(kinds.contains(&TokenKind::Classification));
    assert!(kinds.contains(&TokenKind::SciControl));
    assert!(kinds.contains(&TokenKind::DissemControl));

    // Find each by kind and verify the byte slice matches.
    let src = b"TOP SECRET//SI//NF";
    let cls = parsed
        .attrs
        .token_spans
        .iter()
        .find(|t| t.kind == TokenKind::Classification)
        .unwrap();
    assert_eq!(cls.span.as_str(src).unwrap(), "TOP SECRET");

    let sci = parsed
        .attrs
        .token_spans
        .iter()
        .find(|t| t.kind == TokenKind::SciControl)
        .unwrap();
    assert_eq!(sci.span.as_str(src).unwrap(), "SI");

    let dissem = parsed
        .attrs
        .token_spans
        .iter()
        .find(|t| t.kind == TokenKind::DissemControl)
        .unwrap();
    assert_eq!(dissem.span.as_str(src).unwrap(), "NF");
}

#[test]
fn token_spans_strip_paren_in_portion() {
    let parsed = parse_portion("(SECRET//NF)");
    let src = b"(SECRET//NF)";
    let cls = parsed
        .attrs
        .token_spans
        .iter()
        .find(|t| t.kind == TokenKind::Classification)
        .unwrap();
    // SECRET starts at byte 1 (after the open paren), runs to byte 7.
    assert_eq!(cls.span.start, 1);
    assert_eq!(cls.span.end, 7);
    assert_eq!(cls.span.as_str(src).unwrap(), "SECRET");

    let dissem = parsed
        .attrs
        .token_spans
        .iter()
        .find(|t| t.kind == TokenKind::DissemControl)
        .unwrap();
    // NF starts at byte 9 (after `SECRET//`).
    assert_eq!(dissem.span.start, 9);
    assert_eq!(dissem.span.end, 11);
}

#[test]
fn token_spans_record_unknown_token() {
    let parsed = parse_banner("SECRET//XYZZY//NOFORN");
    let unknowns: Vec<&TokenSpan> = parsed
        .attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert_eq!(unknowns.len(), 1);
    assert_eq!(
        unknowns[0].span.as_str(b"SECRET//XYZZY//NOFORN").unwrap(),
        "XYZZY"
    );
}

#[test]
fn token_spans_record_rel_to_trigraphs() {
    let parsed = parse_banner("SECRET//REL TO USA, GBR, AUS");
    let trigraphs: Vec<&TokenSpan> = parsed
        .attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::RelToTrigraph)
        .collect();
    assert_eq!(trigraphs.len(), 3);
    let src = b"SECRET//REL TO USA, GBR, AUS";
    assert_eq!(trigraphs[0].span.as_str(src).unwrap(), "USA");
    assert_eq!(trigraphs[1].span.as_str(src).unwrap(), "GBR");
    assert_eq!(trigraphs[2].span.as_str(src).unwrap(), "AUS");
}

// -----------------------------------------------------------------------
// Issue #183 PR-A — country-code widening: REL TO must preserve
// tetragraphs (FVEY, NATO, ACGU, …), `EU`, and `AUSTRALIA_GROUP`.
// Pre-PR-A, every non-3-byte token was silently dropped at the
// `b.len() != 3` gate in `parse_rel_to_with_spans`, so a marking
// like `(S//REL TO USA, FVEY, GBR)` arrived at the rule layer as
// `rel_to: [USA, GBR]` — FVEY gone with no diagnostic.
// -----------------------------------------------------------------------

#[test]
fn rel_to_preserves_tetragraph_fvey() {
    let parsed = parse_banner("SECRET//REL TO USA, FVEY, GBR");
    let codes: Vec<&str> = parsed.attrs.rel_to.iter().map(|c| c.as_str()).collect();
    assert_eq!(
        codes,
        vec!["USA", "FVEY", "GBR"],
        "FVEY tetragraph must land in rel_to (issue #183 silent-drop fix)"
    );
}

#[test]
fn rel_to_preserves_opaque_tetragraph_nato() {
    let parsed = parse_banner("SECRET//REL TO USA, NATO, GBR");
    let codes: Vec<&str> = parsed.attrs.rel_to.iter().map(|c| c.as_str()).collect();
    assert_eq!(
        codes,
        vec!["USA", "NATO", "GBR"],
        "NATO is in CVE TRIGRAPHS recognition set; rel_to must preserve it \
         even though membership expansion is deferred to Phase F"
    );
}

#[test]
fn rel_to_preserves_two_byte_eu() {
    let parsed = parse_banner("SECRET//REL TO USA, EU");
    let codes: Vec<&str> = parsed.attrs.rel_to.iter().map(|c| c.as_str()).collect();
    assert_eq!(
        codes,
        vec!["USA", "EU"],
        "EU (2-byte CVE entry) must round-trip through the parser"
    );
}

#[test]
fn rel_to_preserves_long_australia_group() {
    let parsed = parse_banner("SECRET//REL TO USA, AUSTRALIA_GROUP");
    let codes: Vec<&str> = parsed.attrs.rel_to.iter().map(|c| c.as_str()).collect();
    assert_eq!(
        codes,
        vec!["USA", "AUSTRALIA_GROUP"],
        "AUSTRALIA_GROUP (15-byte CVE entry, contains underscore) \
         must round-trip through the parser"
    );
}

#[test]
fn rel_to_token_span_widens_to_actual_code_length() {
    // Pre-PR-A the RelToTrigraph TokenSpan was hardcoded to 3
    // bytes (`Span::new(abs_start, abs_start + 3)`). Widening
    // matters because consumers — the E002 fix splice and
    // diagnostic underlines — read `span.as_str()` to anchor
    // their replacement / message at the exact source bytes.
    let parsed = parse_banner("SECRET//REL TO USA, FVEY, AUSTRALIA_GROUP");
    let trigraph_spans: Vec<&TokenSpan> = parsed
        .attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::RelToTrigraph)
        .collect();
    let src = b"SECRET//REL TO USA, FVEY, AUSTRALIA_GROUP";
    assert_eq!(trigraph_spans[0].span.as_str(src).unwrap(), "USA");
    assert_eq!(trigraph_spans[1].span.as_str(src).unwrap(), "FVEY");
    assert_eq!(
        trigraph_spans[2].span.as_str(src).unwrap(),
        "AUSTRALIA_GROUP"
    );
}

#[test]
fn rel_to_drops_unrecognized_token_from_rel_to_but_keeps_unknown_span() {
    // Defensive: tokens outside the CVE recognition set
    // (`is_trigraph` is false) are skipped from `attrs.rel_to` —
    // we widened recognition, not the gate. `XYZQ` is a 4-char
    // string not in the CVE TRIGRAPHS list.
    let parsed = parse_banner("SECRET//REL TO USA, XYZQ, GBR");
    let codes: Vec<&str> = parsed.attrs.rel_to.iter().map(|c| c.as_str()).collect();
    assert_eq!(codes, vec!["USA", "GBR"]);
    let src = b"SECRET//REL TO USA, XYZQ, GBR";
    assert!(
        parsed
            .attrs
            .token_spans
            .iter()
            .any(|t| { t.kind == TokenKind::Unknown && t.span.as_str(src).ok() == Some("XYZQ") }),
        "unrecognized REL TO token should still be recorded as Unknown span"
    );
}

#[test]
fn token_spans_record_separators() {
    let parsed = parse_banner("SECRET//NF");
    let seps: Vec<&TokenSpan> = parsed
        .attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Separator)
        .collect();
    assert_eq!(seps.len(), 1);
    let src = b"SECRET//NF";
    assert_eq!(seps[0].span.as_str(src).unwrap(), "//");
}
