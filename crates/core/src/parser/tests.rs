use super::*;
use marque_ism::CanonicalAttrs;
use marque_ism::span::{MarkingCandidate, MarkingType};
use marque_ism::token_set::CapcoTokenSet;
use marque_scheme::Span;

/// Test-helper output: a [`ParsedMarking`] post-canonicalization,
/// so existing assertions on the typed `attrs.classification` /
/// `attrs.dissem_us` / `attrs.dissem_nato` shape continue to work
/// without per-test edits.
///
/// Test-fixture carve-out per Constitution V Principle V — the
/// structural rename is inlined here only to construct test inputs
/// whose shape mirrors the engine's post-recognition view.
/// `marque-core` cannot dev-depend on `marque-capco` (Constitution
/// VII), so the trait route `CapcoScheme::canonicalize` is
/// unreachable from here. The inlined body mirrors the override's
/// field mapping and output semantics — including the §G.2 p41 /
/// PR 9b T132 debug-assert — but is not a literal byte-for-byte
/// copy (control flow + locals differ, `From::from` returns
/// `Self` rather than the override's `CanonicalAttrs`).
/// FR-040 PRC100 stays satisfied because the enclosing
/// `From::from` signature is `(ParsedMarking) -> Self`, not
/// `(ParsedAttrs) -> CanonicalAttrs`.
pub(super) struct CanonicalParsed {
    pub attrs: CanonicalAttrs,
    #[allow(dead_code)] // tests inspect attrs only; kept for parity
    pub source_span: Span,
    #[allow(dead_code)]
    pub kind: MarkingType,
}

impl<'src> From<ParsedMarking<'src>> for CanonicalParsed {
    fn from(p: ParsedMarking<'src>) -> Self {
        let marque_ism::ParsedAttrs {
            classification,
            sci_markings,
            sci_controls,
            sar_markings,
            aea_markings,
            fgi_marker,
            dissem_us,
            dissem_nato,
            non_ic_dissem,
            rel_to,
            display_only_to,
            declassify_on,
            classified_by,
            derived_from,
            declass_exemption,
            token_spans,
            source_bytes_origin: _,
        } = p.attrs;
        let attrs = CanonicalAttrs {
            classification: classification.map(|c| c.value),
            sci_controls,
            sci_markings: Vec::from(sci_markings)
                .into_iter()
                .map(|q| q.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            sar_markings: sar_markings.map(|q| q.value),
            aea_markings: Vec::from(aea_markings)
                .into_iter()
                .map(|q| q.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            fgi_marker: fgi_marker.map(|q| q.value),
            dissem_us: Vec::from(dissem_us)
                .into_iter()
                .map(|q| q.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            dissem_nato: Vec::from(dissem_nato)
                .into_iter()
                .map(|q| q.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            non_ic_dissem: Vec::from(non_ic_dissem)
                .into_iter()
                .map(|q| q.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            rel_to: Vec::from(rel_to)
                .into_iter()
                .map(|q| q.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            display_only_to: Vec::from(display_only_to)
                .into_iter()
                .map(|q| q.value)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            declassify_on: declassify_on.map(|q| q.value),
            classified_by: classified_by.map(Box::<str>::from),
            derived_from: derived_from.map(Box::<str>::from),
            declass_exemption,
            token_spans,
        };

        // Mirror the PR 9b (T132) invariant guard carried by
        // `CapcoScheme::canonicalize`. `attribute_dissems` is the
        // single source of truth; this debug-only assertion catches
        // a future bug where attribution is skipped or a hand-built
        // `ParsedAttrs` is fed in with both fields populated.
        #[cfg(debug_assertions)]
        {
            debug_assert!(
                attrs.dissem_nato.is_empty() || attrs.us_classification().is_none(),
                "dissem_nato populated alongside US classification — \
                 attribute_dissems was skipped or bypassed. CAPCO-2016 p41 \
                 reciprocity rule violated."
            );
        }

        Self {
            attrs,
            source_span: p.source_span,
            kind: p.kind,
        }
    }
}

fn make_candidate(text: &[u8], kind: MarkingType, offset: usize) -> MarkingCandidate {
    MarkingCandidate {
        span: Span::new(offset, offset + text.len()),
        kind,
    }
}

fn parse_banner(text: &str) -> CanonicalParsed {
    let source = text.as_bytes();
    let tokens = CapcoTokenSet;
    let parser = Parser::new(&tokens);
    let candidate = make_candidate(source, MarkingType::Banner, 0);
    parser
        .parse(&candidate, source)
        .expect("parse should succeed")
        .into()
}

fn parse_portion(text: &str) -> CanonicalParsed {
    let source = text.as_bytes();
    let tokens = CapcoTokenSet;
    let parser = Parser::new(&tokens);
    let candidate = make_candidate(source, MarkingType::Portion, 0);
    parser
        .parse(&candidate, source)
        .expect("parse should succeed")
        .into()
}

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
fn rel_to_drops_unrecognized_token_silently() {
    // Defensive: tokens outside the CVE recognition set
    // (`is_trigraph` is false) are still skipped — we widened
    // recognition, not the gate. `XYZQ` is a 4-char string not
    // in the CVE TRIGRAPHS list.
    let parsed = parse_banner("SECRET//REL TO USA, XYZQ, GBR");
    let codes: Vec<&str> = parsed.attrs.rel_to.iter().map(|c| c.as_str()).collect();
    assert_eq!(codes, vec!["USA", "GBR"]);
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

// -----------------------------------------------------------------------
// Non-US classification parsing
// -----------------------------------------------------------------------

/// What companion writes a NATO block should produce alongside
/// the bare classification. Mirrors the parser's `NatoCompanion`
/// enum without re-exporting it from the parser module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedCompanion {
    None,
    Atomal,
    Balk,
    Bohemia,
}

fn assert_nato_companion(parsed: &CanonicalParsed, expected: ExpectedCompanion) {
    use marque_ism::{AeaMarking, NatoSap, SciControlSystem};
    let aea_atomal = parsed
        .attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, AeaMarking::Atomal(_)));
    let sci_balk = parsed
        .attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::NatoSap(NatoSap::Balk)));
    let sci_bohemia = parsed
        .attrs
        .sci_markings
        .iter()
        .any(|m| matches!(m.system, SciControlSystem::NatoSap(NatoSap::Bohemia)));
    match expected {
        ExpectedCompanion::None => {
            assert!(!aea_atomal, "did not expect AEA ATOMAL companion");
            assert!(!sci_balk, "did not expect SCI BALK companion");
            assert!(!sci_bohemia, "did not expect SCI BOHEMIA companion");
        }
        ExpectedCompanion::Atomal => {
            assert!(aea_atomal, "expected AEA ATOMAL companion to be written");
        }
        ExpectedCompanion::Balk => {
            assert!(sci_balk, "expected SCI BALK companion to be written");
        }
        ExpectedCompanion::Bohemia => {
            assert!(sci_bohemia, "expected SCI BOHEMIA companion to be written");
        }
    }
}

/// PR 9c.1 T134: legacy compound NATO classification text (e.g.,
/// `COSMIC TOP SECRET ATOMAL`) is canonicalized at parse time to
/// bare class + companion AEA/SCI write per CAPCO-2016 §H.7 p122 +
/// §G.2 p40 + §H.7 p127. The class axis carries only the bare
/// `NatoClassification::CosmicTopSecret` / `NatoSecret` /
/// `NatoConfidential` form; the marking-specific concern (ATOMAL,
/// BALK, BOHEMIA) lives on its grammar-correct axis.
#[test]
fn nato_banner_parses_all_variants() {
    for (input, expected_class, expected_companion) in [
        (
            "//NATO UNCLASSIFIED",
            NatoClassification::NatoUnclassified,
            ExpectedCompanion::None,
        ),
        (
            "//NATO RESTRICTED",
            NatoClassification::NatoRestricted,
            ExpectedCompanion::None,
        ),
        (
            "//NATO CONFIDENTIAL",
            NatoClassification::NatoConfidential,
            ExpectedCompanion::None,
        ),
        (
            "//NATO CONFIDENTIAL ATOMAL",
            NatoClassification::NatoConfidential,
            ExpectedCompanion::Atomal,
        ),
        (
            "//NATO SECRET",
            NatoClassification::NatoSecret,
            ExpectedCompanion::None,
        ),
        (
            "//NATO SECRET ATOMAL",
            NatoClassification::NatoSecret,
            ExpectedCompanion::Atomal,
        ),
        (
            "//COSMIC TOP SECRET",
            NatoClassification::CosmicTopSecret,
            ExpectedCompanion::None,
        ),
        (
            "//COSMIC TOP SECRET ATOMAL",
            NatoClassification::CosmicTopSecret,
            ExpectedCompanion::Atomal,
        ),
        (
            "//COSMIC TOP SECRET-BOHEMIA",
            NatoClassification::CosmicTopSecret,
            ExpectedCompanion::Bohemia,
        ),
        (
            "//COSMIC TOP SECRET-BALK",
            NatoClassification::CosmicTopSecret,
            ExpectedCompanion::Balk,
        ),
    ] {
        let parsed = parse_banner(input);
        assert_eq!(
            parsed.attrs.classification,
            Some(MarkingClassification::Nato(expected_class)),
            "failed bare-class for banner: {input}"
        );
        assert_nato_companion(&parsed, expected_companion);
    }
}

/// PR 9c.1 T134: portion-form legacy compounds (`CTSA`, `CTS-A`,
/// `CTS-B`, `CTS-BALK`, `NSAT`, `NS-A`, `NCA`, `NC-A`) canonicalize
/// to bare class + companion AEA/SCI per CAPCO-2016 §G.1 Table 4
/// p38 (portion-form column) + §G.2 p40 (Table 5: ARH by Registered
/// Marking — ATOMAL/BOHEMIA/BALK as registered control markings) +
/// §H.7 p122 (ATOMAL worked example) + §H.7 p127 (BOHEMIA worked
/// example in SCI block position).
///
/// Marque's E066 autofix rule emits the canonical multi-block form
/// as a Recanonicalize fix; the parser canonicalizes the data shape
/// so rules consuming `attrs.{classification,aea_markings,sci_markings}`
/// see the structural truth.
#[test]
fn nato_portion_parses_all_variants() {
    for (input, expected_class, expected_companion) in [
        (
            "(//NU)",
            NatoClassification::NatoUnclassified,
            ExpectedCompanion::None,
        ),
        (
            "(//NR)",
            NatoClassification::NatoRestricted,
            ExpectedCompanion::None,
        ),
        (
            "(//NC)",
            NatoClassification::NatoConfidential,
            ExpectedCompanion::None,
        ),
        (
            "(//NCA)",
            NatoClassification::NatoConfidential,
            ExpectedCompanion::Atomal,
        ),
        (
            "(//NC-A)",
            NatoClassification::NatoConfidential,
            ExpectedCompanion::Atomal,
        ),
        (
            "(//NS)",
            NatoClassification::NatoSecret,
            ExpectedCompanion::None,
        ),
        (
            "(//NSAT)",
            NatoClassification::NatoSecret,
            ExpectedCompanion::Atomal,
        ),
        (
            "(//NS-A)",
            NatoClassification::NatoSecret,
            ExpectedCompanion::Atomal,
        ),
        (
            "(//CTS)",
            NatoClassification::CosmicTopSecret,
            ExpectedCompanion::None,
        ),
        (
            "(//CTSA)",
            NatoClassification::CosmicTopSecret,
            ExpectedCompanion::Atomal,
        ),
        (
            "(//CTS-A)",
            NatoClassification::CosmicTopSecret,
            ExpectedCompanion::Atomal,
        ),
        (
            "(//CTS-B)",
            NatoClassification::CosmicTopSecret,
            ExpectedCompanion::Bohemia,
        ),
        (
            "(//CTS-BALK)",
            NatoClassification::CosmicTopSecret,
            ExpectedCompanion::Balk,
        ),
    ] {
        let parsed = parse_portion(input);
        assert_eq!(
            parsed.attrs.classification,
            Some(MarkingClassification::Nato(expected_class)),
            "failed bare-class for portion: {input}"
        );
        assert_nato_companion(&parsed, expected_companion);
    }
}

#[test]
fn nato_banner_with_rel_to() {
    let parsed = parse_banner("//NATO SECRET//REL TO USA, GBR");
    assert_eq!(
        parsed.attrs.classification,
        Some(MarkingClassification::Nato(NatoClassification::NatoSecret)),
    );
    assert_eq!(parsed.attrs.rel_to.len(), 2);
    assert_eq!(parsed.attrs.rel_to[0], CountryCode::USA);
}

#[test]
fn joint_banner_parses_correctly() {
    let parsed = parse_banner("//JOINT S USA GBR");
    match &parsed.attrs.classification {
        Some(MarkingClassification::Joint(j)) => {
            assert_eq!(j.level, Classification::Secret);
            assert_eq!(j.countries.len(), 2);
            assert_eq!(j.countries[0], CountryCode::USA);
            assert_eq!(j.countries[1].as_str(), "GBR");
        }
        other => panic!("expected Joint, got: {other:?}"),
    }
}

#[test]
fn joint_banner_parses_top_secret_multi_word_level() {
    // The JOINT parser has a separate two-token path for the
    // multi-word `TOP SECRET` level (vs. the single-token `S` /
    // `TS` / `C` / `U` abbreviations). Exercises lines 905-907
    // and 909 of `parse_joint_classification`.
    let parsed = parse_banner("//JOINT TOP SECRET USA GBR");
    match &parsed.attrs.classification {
        Some(MarkingClassification::Joint(j)) => {
            assert_eq!(j.level, Classification::TopSecret);
            assert_eq!(j.countries.len(), 2);
            assert_eq!(j.countries[0], CountryCode::USA);
            assert_eq!(j.countries[1].as_str(), "GBR");
        }
        other => panic!("expected Joint(TopSecret), got: {other:?}"),
    }
}

#[test]
fn joint_banner_rejects_bare_top_without_secret() {
    // `TOP` alone is not a valid classification level — the
    // JOINT parser must return None and let the parent path
    // try other foreign-classification shapes. Exercises the
    // `else { return None; }` branch of the TOP-SECRET path.
    let parsed = parse_banner("//JOINT TOP USA GBR");
    assert!(
        !matches!(
            parsed.attrs.classification,
            Some(MarkingClassification::Joint(_))
        ),
        "bare TOP must not parse as a JOINT classification"
    );
}

#[test]
fn joint_portion_with_rel_to() {
    let parsed = parse_portion("(//JOINT TS USA AUS GBR//REL TO USA, AUS, GBR)");
    match &parsed.attrs.classification {
        Some(MarkingClassification::Joint(j)) => {
            assert_eq!(j.level, Classification::TopSecret);
            assert_eq!(j.countries.len(), 3);
        }
        other => panic!("expected Joint, got: {other:?}"),
    }
    assert_eq!(parsed.attrs.rel_to.len(), 3);
}

#[test]
fn fgi_single_country_parses() {
    let parsed = parse_portion("(//GBR S//NF)");
    match &parsed.attrs.classification {
        Some(MarkingClassification::Fgi(f)) => {
            assert_eq!(f.level, Classification::Secret);
            assert_eq!(f.countries.len(), 1);
            assert_eq!(f.countries[0].as_str(), "GBR");
        }
        other => panic!("expected Fgi, got: {other:?}"),
    }
}

#[test]
fn fgi_multiple_countries_parses() {
    let parsed = parse_banner("//GBR DEU TS//NF");
    match &parsed.attrs.classification {
        Some(MarkingClassification::Fgi(f)) => {
            assert_eq!(f.level, Classification::TopSecret);
            assert_eq!(f.countries.len(), 2);
        }
        other => panic!("expected Fgi, got: {other:?}"),
    }
}

#[test]
fn fgi_placeholder_country_parses() {
    // FGI as placeholder for unknown country + level
    let parsed = parse_portion("(//FGI S//NF)");
    match &parsed.attrs.classification {
        Some(MarkingClassification::Fgi(f)) => {
            assert_eq!(f.level, Classification::Secret);
            assert!(
                f.countries.is_empty(),
                "FGI placeholder should have no countries"
            );
        }
        other => panic!("expected Fgi, got: {other:?}"),
    }
}

#[test]
fn fgi_non_uppercase_trigraph_rejected() {
    // `CountryCode::try_new` accepts ASCII uppercase letter,
    // ASCII digit, or underscore (issue #183 widened the byte
    // set to cover `AX2`/`AX3` and `AUSTRALIA_GROUP`). A 3-byte
    // token containing a lowercase letter still fails that
    // check and trips the `CountryCode::try_new(...)?` rejection
    // path in `parse_fgi_classification`.
    let parsed = parse_banner("//Gbr S//NF");
    assert!(
        !matches!(
            parsed.attrs.classification,
            Some(MarkingClassification::Fgi(_))
        ),
        "Gbr should not parse as a valid FGI classification: {:?}",
        parsed.attrs.classification,
    );
}

#[test]
fn fgi_no_level_is_error() {
    // //FGI// with no classification level — classification should be None
    let parsed = parse_banner("//FGI//NF");
    assert!(
        parsed.attrs.classification.is_none()
            || matches!(
                parsed.attrs.classification,
                Some(MarkingClassification::Us(_))
            ),
        "bare FGI with no level should not produce a valid non-US classification: {:?}",
        parsed.attrs.classification,
    );
}

#[test]
fn fgi_marker_in_us_marking() {
    let parsed = parse_banner("SECRET//FGI DEU//NOFORN");
    assert_eq!(
        parsed.attrs.classification,
        Some(MarkingClassification::Us(Classification::Secret)),
    );
    let marker = parsed
        .attrs
        .fgi_marker
        .as_ref()
        .expect("should have FGI marker");
    match marker {
        FgiMarker::Acknowledged { countries, .. } => {
            assert_eq!(countries.len(), 1);
            assert_eq!(countries[0].as_str(), "DEU");
        }
        FgiMarker::SourceConcealed => panic!("expected acknowledged variant"),
    }
}

#[test]
fn fgi_marker_bare_is_source_concealed() {
    let parsed = parse_banner("SECRET//FGI//NOFORN");
    assert_eq!(
        parsed.attrs.classification,
        Some(MarkingClassification::Us(Classification::Secret)),
    );
    let marker = parsed
        .attrs
        .fgi_marker
        .as_ref()
        .expect("should have FGI marker");
    // CAPCO §H.7 p122: bare `FGI` is the lawful source-concealed
    // banner form, distinct from a parser failure.
    assert!(matches!(marker, FgiMarker::SourceConcealed));
}

// ---- T088 + T093: FR-015 / FR-016 closure for parse_fgi_marker ----
//
// GH #280 retired the transitional `unwrap_or(SourceConcealed)`
// fallback. These tests pin the three lawful cases per CAPCO-2016
// §H.7 p122 (the only two banner forms — concealed `FGI` and
// acknowledged `FGI [LIST]`) plus the negative cases that map to
// `None`. The parser is invoked via `parse_banner` here to
// exercise the same call site (`crates/core/src/parser.rs:345`)
// that the engine reaches in production; the public surface of
// `parse_fgi_marker` itself is private to this module.

#[test]
fn fgi_marker_multi_country_acknowledged() {
    // Three-country list: tests that the SmallVec inline path
    // (4 codes) covers the typical case without heap allocation,
    // and that the parser admits each token through
    // `CountryCode::admits_fgi_trigraph`.
    let parsed = parse_banner("SECRET//FGI USA GBR JPN//NOFORN");
    let marker = parsed
        .attrs
        .fgi_marker
        .as_ref()
        .expect("should have FGI marker");
    match marker {
        FgiMarker::Acknowledged { countries, .. } => {
            assert_eq!(countries.len(), 3);
            assert_eq!(countries[0].as_str(), "USA");
            assert_eq!(countries[1].as_str(), "GBR");
            assert_eq!(countries[2].as_str(), "JPN");
        }
        FgiMarker::SourceConcealed => panic!("expected Acknowledged variant"),
    }
}

#[test]
fn fgi_marker_lowercase_token_no_marker() {
    // Lowercase fails `admits_fgi_trigraph`; the previous
    // transitional behavior would have silently dropped the
    // token, producing an empty country list and falling back to
    // `SourceConcealed`. Post-T088: the parser returns `None`,
    // so `attrs.fgi_marker` is unset (CAPCO §H.7 p122 disallows
    // a degraded lawful form on shape failure).
    let parsed = parse_banner("SECRET//FGI deu//NOFORN");
    assert!(
        parsed.attrs.fgi_marker.is_none(),
        "lowercase trigraph must fail FGI marker shape gate (got {:?})",
        parsed.attrs.fgi_marker,
    );
}

#[test]
fn fgi_marker_tetragraph_admits_per_capco_h7() {
    // CAPCO-2016 §H.7 p122 spells out the canonical example:
    // `SECRET//FGI GBR JPN NATO//REL TO USA, GBR, JPN, NATO`
    // — `NATO` is a 4-letter Annex A tetragraph admitted in the
    // same FGI list as the trigraphs. The PR #311 review caught
    // a regression where the parser narrowed admission to
    // `admits_fgi_trigraph` (3-only); the post-fix
    // `admits_country_token` widens to 2/3/4 ASCII upper,
    // matching the §H.7 grammar.
    let parsed = parse_banner("SECRET//FGI USA NATO//NOFORN");
    let marker = parsed
        .attrs
        .fgi_marker
        .as_ref()
        .expect("FGI USA NATO admits per §H.7 p122");
    match marker {
        FgiMarker::Acknowledged { countries, .. } => {
            assert_eq!(countries.len(), 2);
            let names: Vec<&str> = countries.iter().map(|c| c.as_str()).collect();
            assert!(names.contains(&"USA"), "USA must appear; got {names:?}");
            assert!(names.contains(&"NATO"), "NATO must appear; got {names:?}");
        }
        FgiMarker::SourceConcealed => panic!("expected Acknowledged([USA, NATO])"),
    }
}

#[test]
fn fgi_marker_unregistered_trigraph_shape_admits_but_marker_records_it() {
    // `XYZ` is shape-admissible (3 ASCII upper) — `admits_fgi_trigraph`
    // is a *shape* gate, not a registry-membership gate. The CVE
    // table check (`is_trigraph` against the GENC trigraph
    // registry) lives in the rule layer (S### / E###), not the
    // parser. So `XYZ` parses as an Acknowledged country code
    // and a downstream rule flags the unknown trigraph.
    //
    // This pins the boundary: shape vs. registry. The earlier
    // (pre-T088) implementation also accepted `XYZ` because
    // `CountryCode::try_new` succeeds on 3 ASCII upper, so this
    // is not a regression — it's a confirmation that the gate's
    // semantics are scoped correctly.
    let parsed = parse_banner("SECRET//FGI XYZ//NOFORN");
    let marker = parsed
        .attrs
        .fgi_marker
        .as_ref()
        .expect("XYZ is shape-admissible; rule layer flags registry membership");
    match marker {
        FgiMarker::Acknowledged { countries, .. } => {
            assert_eq!(countries.len(), 1);
            assert_eq!(countries[0].as_str(), "XYZ");
        }
        FgiMarker::SourceConcealed => panic!("expected Acknowledged variant"),
    }
}

#[test]
fn fgi_marker_direct_three_cases() {
    // Direct exercise of `parse_fgi_marker` at the same module
    // level, covering the three lawful return cases without the
    // banner wrapper. Pins behavior the public `parse_banner`
    // tests above route through indirectly.

    // Case 1: bare "FGI" → Some(SourceConcealed)
    assert!(matches!(
        parse_fgi_marker("FGI"),
        Some(FgiMarker::SourceConcealed),
    ));

    // Case 2: "FGI <trigraph>" → Some(Acknowledged)
    match parse_fgi_marker("FGI USA") {
        Some(FgiMarker::Acknowledged { countries, .. }) => {
            assert_eq!(countries.len(), 1);
            assert_eq!(countries[0].as_str(), "USA");
        }
        other => panic!("expected Acknowledged([USA]), got {other:?}"),
    }

    // Case 2 (multi): up to and beyond SmallVec inline capacity
    match parse_fgi_marker("FGI USA GBR DEU JPN FRA") {
        Some(FgiMarker::Acknowledged { countries, .. }) => {
            assert_eq!(countries.len(), 5);
            let names: Vec<&str> = countries.iter().map(|c| c.as_str()).collect();
            assert_eq!(names, ["USA", "GBR", "DEU", "JPN", "FRA"]);
        }
        other => panic!("expected 5-country Acknowledged, got {other:?}"),
    }

    // Case 3: empty input → None
    assert!(parse_fgi_marker("").is_none());

    // Case 3: lowercase token → None (FR-016 closure)
    assert!(parse_fgi_marker("FGI deu").is_none());

    // Case 2 (mixed shapes per §H.7 p122): trigraph + tetragraph
    // → Some(Acknowledged) with both countries. PR #311 review
    // caught the prior trigraph-only narrowing; post-fix
    // admission accepts the spec-canonical example.
    match parse_fgi_marker("FGI GBR JPN NATO") {
        Some(FgiMarker::Acknowledged { countries, .. }) => {
            assert_eq!(countries.len(), 3);
            let names: Vec<&str> = countries.iter().map(|c| c.as_str()).collect();
            assert_eq!(names, ["GBR", "JPN", "NATO"]);
        }
        other => panic!("expected 3-country Acknowledged([GBR, JPN, NATO]), got {other:?}"),
    }

    // Case 2 (#280 widening): the EU 2-letter exception code
    // admits at the FGI ownership gate. EU is the only
    // supranational sub-NATO entity with its own classification
    // system (Council Decision 2013/488/EU); registered in
    // ISMCAT `CVEnumISMCATRelTo`. Distinct from the broader
    // `admits_country_token` surface used at REL TO list slots.
    match parse_fgi_marker("FGI EU") {
        Some(FgiMarker::Acknowledged { countries, .. }) => {
            assert_eq!(countries.len(), 1);
            assert_eq!(countries[0].as_str(), "EU");
        }
        other => panic!("expected Acknowledged([EU]), got {other:?}"),
    }

    // Case 3 (#280): non-`NATO` tetragraphs reject in the FGI
    // ownership slot. `ABCD` (and `FVEY`, `CFIUS`, `ACGU`, `ISAF`)
    // are lawful at the broader REL TO list-token surface but
    // distribution-list markers don't carry FGI's ownership
    // semantic per §H.7.
    assert!(
        parse_fgi_marker("FGI ABCD").is_none(),
        "non-NATO 4-char tetragraph rejects in FGI ownership \
         context (#280)",
    );
    assert!(
        parse_fgi_marker("FGI FVEY").is_none(),
        "distribution-list tetragraph FVEY rejects in FGI ownership \
         context (#280)",
    );

    // Case 3: empty input → None
    assert!(parse_fgi_marker("").is_none());

    // Case 3: lowercase token → None (FR-016 closure)
    assert!(parse_fgi_marker("FGI deu").is_none());
    assert!(parse_fgi_marker("FGI nato").is_none());

    // Case 3: 5+-byte token rejects (out-of-scope of
    // `admits_fgi_ownership_token`; the §H.7 "exception is
    // granted" surface for AUSTRALIA_GROUP-class codes is not
    // handled at this gate).
    assert!(parse_fgi_marker("FGI USAGB").is_none());
    assert!(parse_fgi_marker("FGI AUSTRALIA_GROUP").is_none());

    // Case 3: trailing whitespace with no tokens → None
    assert!(parse_fgi_marker("FGI ").is_none());

    // Case 3: malformed prefix → None
    assert!(parse_fgi_marker("foo FGI USA").is_none());
    assert!(parse_fgi_marker("FGIDEU").is_none()); // no separator

    // Case 3: digits in any list-token slot → None
    assert!(parse_fgi_marker("FGI US1").is_none());
    assert!(parse_fgi_marker("FGI 123").is_none());
    assert!(parse_fgi_marker("FGI NAT0").is_none()); // 0 not O
}

#[test]
fn fgi_marker_double_space_tolerated() {
    // CAPCO §A.6 p16 specifies "single space" as the canonical
    // separator, but `split_whitespace` in the parser
    // tolerates multi-space and tab between tokens. A separate
    // style rule (S###) can flag the non-canonical separator
    // if the project ever wants one; the parser's job is
    // admission, not style enforcement. Pin the tolerance so
    // a future split-on-single-space rewrite is forced to
    // notice this contract.
    match parse_fgi_marker("FGI  USA") {
        Some(FgiMarker::Acknowledged { countries, .. }) => {
            assert_eq!(countries.len(), 1);
            assert_eq!(countries[0].as_str(), "USA");
        }
        other => panic!("expected Acknowledged([USA]) for double-space input, got {other:?}"),
    }
}

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

// -----------------------------------------------------------------------
// Non-IC dissemination controls
// -----------------------------------------------------------------------

#[test]
fn non_ic_dissem_limdis_banner_form() {
    let parsed = parse_banner("UNCLASSIFIED//LIMDIS");
    assert_eq!(parsed.attrs.non_ic_dissem.len(), 1);
    assert_eq!(parsed.attrs.non_ic_dissem[0], NonIcDissem::Limdis,);
}

#[test]
fn non_ic_dissem_ds_portion_form() {
    let parsed = parse_portion("(U//DS)");
    assert_eq!(parsed.attrs.non_ic_dissem.len(), 1);
    assert_eq!(parsed.attrs.non_ic_dissem[0], NonIcDissem::Limdis);
}

#[test]
fn non_ic_dissem_les_nf() {
    let parsed = parse_portion("(U//LES-NF)");
    assert_eq!(parsed.attrs.non_ic_dissem.len(), 1);
    assert_eq!(parsed.attrs.non_ic_dissem[0], NonIcDissem::LesNf);
    assert!(parsed.attrs.non_ic_dissem[0].carries_noforn());
}

#[test]
fn non_ic_dissem_sbu_nf_banner() {
    let parsed = parse_banner("UNCLASSIFIED//SBU NOFORN");
    assert_eq!(parsed.attrs.non_ic_dissem.len(), 1);
    assert_eq!(parsed.attrs.non_ic_dissem[0], NonIcDissem::SbuNf);
}

#[test]
fn non_ic_dissem_not_confused_with_ic_dissem() {
    // SSI should be non-IC, not IC.
    let parsed = parse_portion("(U//SSI)");
    assert_eq!(parsed.attrs.dissem_iter().count(), 0);
    assert_eq!(parsed.attrs.non_ic_dissem.len(), 1);
    assert_eq!(parsed.attrs.non_ic_dissem[0], NonIcDissem::Ssi);
}

#[test]
fn non_ic_dissem_alongside_ic_dissem() {
    // Classified portion with both IC and non-IC dissem.
    let parsed = parse_portion("(C//NF//DS)");
    assert_eq!(parsed.attrs.dissem_iter().count(), 1); // NF
    assert_eq!(parsed.attrs.non_ic_dissem.len(), 1); // DS = LIMDIS
}

// -----------------------------------------------------------------------
// Atomic Energy Act markings
// -----------------------------------------------------------------------

#[test]
fn aea_rd_parses() {
    let parsed = parse_banner("TOP SECRET//RD//NOFORN");
    assert_eq!(parsed.attrs.aea_markings.len(), 1);
    assert_eq!(
        parsed.attrs.aea_markings[0],
        AeaMarking::Rd(marque_ism::RdBlock::default()),
    );
}

#[test]
fn aea_rd_cnwdi_compound() {
    // CNWDI is a hyphen-modifier of RD, not a separate // block.
    let parsed = parse_banner("SECRET//RD-CNWDI//NOFORN");
    assert_eq!(parsed.attrs.aea_markings.len(), 1);
    match &parsed.attrs.aea_markings[0] {
        AeaMarking::Rd(rd) => {
            assert!(rd.cnwdi);
            assert!(rd.sigma.is_empty());
        }
        other => panic!("expected Rd with CNWDI, got: {other:?}"),
    }
}

#[test]
fn aea_rd_sigma_compound() {
    // SIGMA is a hyphen-modifier: RD-SIGMA 20
    let parsed = parse_banner("SECRET//RD-SIGMA 20//NOFORN");
    assert_eq!(parsed.attrs.aea_markings.len(), 1);
    match &parsed.attrs.aea_markings[0] {
        AeaMarking::Rd(rd) => {
            assert!(!rd.cnwdi);
            assert_eq!(&*rd.sigma, &[20]);
        }
        other => panic!("expected Rd with SIGMA, got: {other:?}"),
    }
}

#[test]
fn aea_rd_cnwdi_sigma_compound() {
    let parsed = parse_banner("SECRET//RD-CNWDI-SIGMA 18 20//NOFORN");
    assert_eq!(parsed.attrs.aea_markings.len(), 1);
    match &parsed.attrs.aea_markings[0] {
        AeaMarking::Rd(rd) => {
            assert!(rd.cnwdi);
            assert_eq!(&*rd.sigma, &[18, 20]);
        }
        other => panic!("expected Rd with CNWDI+SIGMA, got: {other:?}"),
    }
}

#[test]
fn aea_rd_sigma_portion() {
    // Portion form uses SG instead of SIGMA.
    let parsed = parse_portion("(TS//RD-SG 14//NF)");
    assert_eq!(parsed.attrs.aea_markings.len(), 1);
    match &parsed.attrs.aea_markings[0] {
        AeaMarking::Rd(rd) => {
            assert_eq!(&*rd.sigma, &[14]);
        }
        other => panic!("expected Rd with SG, got: {other:?}"),
    }
}

#[test]
fn aea_frd_parses() {
    let parsed = parse_portion("(S//FRD//NF)");
    assert_eq!(parsed.attrs.aea_markings.len(), 1);
    assert_eq!(
        parsed.attrs.aea_markings[0],
        AeaMarking::Frd(marque_ism::FrdBlock::default()),
    );
}

#[test]
fn aea_frd_sigma_compound() {
    let parsed = parse_banner("SECRET//FRD-SIGMA 14//NOFORN");
    assert_eq!(parsed.attrs.aea_markings.len(), 1);
    match &parsed.attrs.aea_markings[0] {
        AeaMarking::Frd(frd) => {
            assert_eq!(&*frd.sigma, &[14]);
        }
        other => panic!("expected Frd with SIGMA, got: {other:?}"),
    }
}

#[test]
fn aea_dod_ucni_parses() {
    let parsed = parse_banner("UNCLASSIFIED//DOD UCNI");
    assert_eq!(parsed.attrs.aea_markings.len(), 1);
    assert_eq!(parsed.attrs.aea_markings[0], AeaMarking::DodUcni);
}

#[test]
fn aea_dcni_portion_parses() {
    let parsed = parse_portion("(U//DCNI)");
    assert_eq!(parsed.attrs.aea_markings.len(), 1);
    assert_eq!(parsed.attrs.aea_markings[0], AeaMarking::DodUcni);
}

#[test]
fn aea_tfni_parses() {
    let parsed = parse_banner("SECRET//TFNI//NOFORN");
    assert_eq!(parsed.attrs.aea_markings.len(), 1);
    assert_eq!(parsed.attrs.aea_markings[0], AeaMarking::Tfni);
}

#[test]
fn aea_rd_n_shorthand() {
    // DoD shorthand: RD-N means RD-CNWDI
    let parsed = parse_portion("(S//RD-N//NF)");
    assert_eq!(parsed.attrs.aea_markings.len(), 1);
    match &parsed.attrs.aea_markings[0] {
        AeaMarking::Rd(rd) => assert!(rd.cnwdi),
        other => panic!("expected Rd with CNWDI from RD-N, got: {other:?}"),
    }
}

// --- CAPCO §D.1 intra-block `/` separator ---

#[test]
fn slash_separated_sci_in_single_block_parses() {
    // CAPCO §D.1: multiple SCI controls in one block, `/`-separated.
    // "(TS//SI/TK//NF)" must produce sci_controls: [Si, Tk], NOT Unknown.
    use marque_ism::SciControl;
    let parsed = parse_portion("(TS//SI/TK//NF)");
    assert_eq!(
        parsed.attrs.sci_controls.as_ref(),
        &[SciControl::Si, SciControl::Tk],
        "SI/TK block must yield two SCI controls"
    );
    // No Unknown token spans
    assert!(
        parsed
            .attrs
            .token_spans
            .iter()
            .all(|t| t.kind != TokenKind::Unknown),
        "no Unknown spans expected: {:?}",
        parsed.attrs.token_spans
    );
}

#[test]
fn slash_separated_sci_banner_parses() {
    // Same rule applies to banner markings.
    use marque_ism::SciControl;
    let parsed = parse_banner("TOP SECRET//SI/TK//NOFORN");
    assert_eq!(
        parsed.attrs.sci_controls.as_ref(),
        &[SciControl::Si, SciControl::Tk],
    );
}

#[test]
fn slash_separated_dissem_in_single_block_parses() {
    // Dissem controls can also share a block: "NF/RD" in one // block.
    use marque_ism::DissemControl;
    let parsed = parse_banner("SECRET//SI//NF/RELIDO");
    // US-classified marking → all dissems attributed to dissem_us
    // per CAPCO-2016 p41 reciprocity (PR 9b / FR-046).
    let dissem: Vec<DissemControl> = parsed.attrs.dissem_iter().copied().collect();
    assert!(dissem.contains(&DissemControl::Nf), "must contain NF");
    assert!(
        dissem.contains(&DissemControl::Relido),
        "must contain RELIDO"
    );
}

#[test]
fn unrecognized_slash_token_emits_unknown() {
    // An unknown token like "XYZZY" in a slash block → Unknown span.
    let parsed = parse_portion("(S//XYZZY)");
    assert!(
        parsed
            .attrs
            .token_spans
            .iter()
            .any(|t| t.kind == TokenKind::Unknown),
        "XYZZY must produce Unknown span"
    );
}

// -----------------------------------------------------------------------
// SCI structural subparser (spec 003-sci-compartments §R2 / P2)
// -----------------------------------------------------------------------

#[test]
fn sci_bare_single_still_parses_via_structural_path() {
    // Regression: `(U//SI//NF)` existing happy path. Structural parser
    // claims `SI` (bare CVE) and projects to sci_controls for
    // back-compat with E010/E011.
    use marque_ism::{SciControl, SciControlBare, SciControlSystem};
    let parsed = parse_portion("(U//SI//NF)");
    assert_eq!(parsed.attrs.sci_controls.as_ref(), &[SciControl::Si]);
    assert_eq!(parsed.attrs.sci_markings.len(), 1);
    let m = &parsed.attrs.sci_markings[0];
    assert_eq!(m.system, SciControlSystem::Published(SciControlBare::Si));
    assert!(m.compartments.is_empty());
    assert_eq!(m.canonical_enum, Some(SciControl::Si));
}

#[test]
fn sci_published_compound_si_g_parses() {
    // `SI-G` is a pre-registered CVE composite; canonical_enum must be Some(SiG).
    use marque_ism::{SciControl, SciControlBare, SciControlSystem};
    let parsed = parse_banner("SECRET//SI-G//NOFORN");
    let m = &parsed.attrs.sci_markings[0];
    assert_eq!(m.system, SciControlSystem::Published(SciControlBare::Si));
    assert_eq!(m.compartments.len(), 1);
    assert_eq!(m.compartments[0].identifier.as_str(), "G");
    assert!(m.compartments[0].sub_compartments.is_empty());
    assert_eq!(m.canonical_enum, Some(SciControl::SiG));
    assert_eq!(parsed.attrs.sci_controls.as_ref(), &[SciControl::SiG]);
}

#[test]
fn sci_published_compound_hcs_p_parses() {
    use marque_ism::{SciControl, SciControlBare, SciControlSystem};
    let parsed = parse_banner("TOP SECRET//HCS-P//NOFORN");
    let m = &parsed.attrs.sci_markings[0];
    assert_eq!(m.system, SciControlSystem::Published(SciControlBare::Hcs));
    assert_eq!(m.compartments[0].identifier.as_str(), "P");
    assert_eq!(m.canonical_enum, Some(SciControl::HcsP));
}

#[test]
fn sci_bare_tk_parses() {
    use marque_ism::{SciControl, SciControlBare, SciControlSystem};
    let parsed = parse_banner("SECRET//TK//NOFORN");
    let m = &parsed.attrs.sci_markings[0];
    assert_eq!(m.system, SciControlSystem::Published(SciControlBare::Tk));
    assert!(m.compartments.is_empty());
    assert_eq!(m.canonical_enum, Some(SciControl::Tk));
}

#[test]
fn sci_multi_system_si_tk_parses() {
    // `SI/TK` — two bare systems in one SCI block. Existing behavior.
    use marque_ism::SciControl;
    let parsed = parse_portion("(TS//SI/TK//NF)");
    assert_eq!(
        parsed.attrs.sci_controls.as_ref(),
        &[SciControl::Si, SciControl::Tk]
    );
    assert_eq!(parsed.attrs.sci_markings.len(), 2);
}

#[test]
fn sci_compound_with_sub_compartment_sets_canonical_none() {
    // `SI-G ABCD`: published system SI with compartment G and sub-comp
    // ABCD. Because the first compartment has sub-comps, canonical_enum
    // is None (the compound is a structural anchor, not an atomic CVE).
    use marque_ism::{SciControlBare, SciControlSystem};
    let parsed = parse_banner("SECRET//SI-G ABCD//NOFORN");
    assert_eq!(parsed.attrs.sci_markings.len(), 1);
    let m = &parsed.attrs.sci_markings[0];
    assert_eq!(m.system, SciControlSystem::Published(SciControlBare::Si));
    assert_eq!(m.compartments.len(), 1);
    assert_eq!(m.compartments[0].identifier.as_str(), "G");
    assert_eq!(m.compartments[0].sub_compartments.len(), 1);
    assert_eq!(m.compartments[0].sub_compartments[0].as_str(), "ABCD");
    assert_eq!(m.canonical_enum, None);
    // sci_controls projection: no canonical_enum → no entry
    assert!(parsed.attrs.sci_controls.is_empty());
}

#[test]
fn sci_capco_canonical_example_parses() {
    // CAPCO-2016 §A.6 p16 canonical example:
    //   TOP SECRET//123/SI-G ABCD DEFG-MMM AACD//ORCON/NOFORN
    use marque_ism::{SciControlBare, SciControlSystem};
    let parsed = parse_banner("TOP SECRET//123/SI-G ABCD DEFG-MMM AACD//ORCON/NOFORN");
    assert_eq!(parsed.attrs.sci_markings.len(), 2);
    // Marking 0: Custom("123"), no compartments.
    let m0 = &parsed.attrs.sci_markings[0];
    assert!(matches!(&m0.system, SciControlSystem::Custom(s) if s.as_str() == "123"));
    assert!(m0.compartments.is_empty());
    assert_eq!(m0.canonical_enum, None);
    // Marking 1: Published(SI) with compartments G[ABCD, DEFG] and MMM[AACD].
    let m1 = &parsed.attrs.sci_markings[1];
    assert_eq!(m1.system, SciControlSystem::Published(SciControlBare::Si));
    assert_eq!(m1.compartments.len(), 2);
    assert_eq!(m1.compartments[0].identifier.as_str(), "G");
    assert_eq!(m1.compartments[0].sub_compartments.len(), 2);
    assert_eq!(m1.compartments[0].sub_compartments[0].as_str(), "ABCD");
    assert_eq!(m1.compartments[0].sub_compartments[1].as_str(), "DEFG");
    assert_eq!(m1.compartments[1].identifier.as_str(), "MMM");
    assert_eq!(m1.compartments[1].sub_compartments.len(), 1);
    assert_eq!(m1.compartments[1].sub_compartments[0].as_str(), "AACD");
    // First compartment has sub-comps → canonical_enum is None.
    assert_eq!(m1.canonical_enum, None);
    // No Unknown spans in the SCI block.
    let sci_block_has_unknown = parsed
        .attrs
        .token_spans
        .iter()
        .any(|t| t.kind == TokenKind::Unknown);
    assert!(
        !sci_block_has_unknown,
        "canonical example must not produce Unknown tokens; got: {:?}",
        parsed.attrs.token_spans
    );
}

#[test]
fn sci_custom_numeric_99_direct_parse() {
    // Direct unit test of parse_sci_block: `99` → Custom("99").
    // In dispatch, `99` alone wouldn't pass the containment gate; this
    // exercises the parser's custom-only happy path.
    use marque_ism::SciControlSystem;
    let mut tokens = SmallVec::new();
    let result = parse_sci_block("99", 0, &mut tokens).expect("99 must parse");
    assert_eq!(result.len(), 1);
    assert!(matches!(&result[0].system, SciControlSystem::Custom(s) if s.as_str() == "99"));
    assert!(result[0].compartments.is_empty());
    assert_eq!(result[0].canonical_enum, None);
}

#[test]
fn sci_structural_rejections_return_none() {
    // Dangling hyphen.
    let mut tokens = SmallVec::new();
    assert!(parse_sci_block("SI-", 0, &mut tokens).is_none());
    // Leading hyphen.
    let mut tokens = SmallVec::new();
    assert!(parse_sci_block("-SI", 0, &mut tokens).is_none());
    // Empty.
    let mut tokens = SmallVec::new();
    assert!(parse_sci_block("", 0, &mut tokens).is_none());
    // Lowercase.
    let mut tokens = SmallVec::new();
    assert!(parse_sci_block("si-g", 0, &mut tokens).is_none());
    // Consecutive hyphens.
    let mut tokens = SmallVec::new();
    assert!(parse_sci_block("SI--G", 0, &mut tokens).is_none());
    // Empty slash chunk.
    let mut tokens = SmallVec::new();
    assert!(parse_sci_block("SI/", 0, &mut tokens).is_none());
}

#[test]
fn sci_mixed_category_slash_block_falls_through() {
    // `SI/NF` has `/` and gate passes, but parse_sci_block must reject
    // because NF is a known dissem control — otherwise E004's
    // stray-slash detection would stop working.
    let parsed = parse_banner("SECRET//SI/NF");
    // The SI/NF block should NOT be claimed by structural SCI; it must
    // fall through to the existing intra-block `/` splitter which in
    // turn flags the mixed-category slash as Unknown.
    let has_unknown_block = parsed
        .attrs
        .token_spans
        .iter()
        .any(|t| t.kind == TokenKind::Unknown);
    assert!(
        has_unknown_block,
        "SI/NF must surface as Unknown for E004; got: {:?}",
        parsed.attrs.token_spans
    );
}

#[test]
fn sci_weird_sub_compartment_parses() {
    // `SI-G WEIRD FOO` — WEIRD and FOO both match [A-Z0-9]+ so the
    // grammar treats them as sub-compartments of G.
    use marque_ism::{SciControlBare, SciControlSystem};
    let parsed = parse_banner("SECRET//SI-G WEIRD FOO//NOFORN");
    let m = &parsed.attrs.sci_markings[0];
    assert_eq!(m.system, SciControlSystem::Published(SciControlBare::Si));
    assert_eq!(m.compartments.len(), 1);
    assert_eq!(m.compartments[0].identifier.as_str(), "G");
    assert_eq!(m.compartments[0].sub_compartments.len(), 2);
    assert_eq!(m.compartments[0].sub_compartments[0].as_str(), "WEIRD");
    assert_eq!(m.compartments[0].sub_compartments[1].as_str(), "FOO");
}

// -----------------------------------------------------------------------
// CAB date parsing (parse_cab Declassify On: path)
// -----------------------------------------------------------------------

fn parse_cab_text(text: &str) -> CanonicalParsed {
    let source = text.as_bytes();
    let tokens = CapcoTokenSet;
    let parser = Parser::new(&tokens);
    let candidate = make_candidate(source, MarkingType::Cab, 0);
    parser
        .parse(&candidate, source)
        .expect("CAB parse should succeed")
        .into()
}

#[test]
fn cab_declassify_on_yyyymmdd_populates_declassify_on() {
    let text = "Classified By: Jane Doe\nDeclassify On: 20301231";
    let parsed = parse_cab_text(text);
    assert_eq!(
        parsed.attrs.declassify_on,
        Some(marque_ism::IsmDate::Date(2030, 12, 31)),
        "YYYYMMDD in CAB should set declassify_on to Date"
    );
    assert!(parsed.attrs.declass_exemption.is_none());
}

#[test]
fn cab_declassify_on_yyyy_populates_declassify_on() {
    let text = "Declassify On: 2035";
    let parsed = parse_cab_text(text);
    assert_eq!(
        parsed.attrs.declassify_on,
        Some(marque_ism::IsmDate::Year(2035)),
        "YYYY in CAB should set declassify_on to Year"
    );
}

#[test]
fn cab_declassify_on_iso_date_populates_declassify_on() {
    // ISO hyphenated YYYY-MM-DD form is valid for the CAB "Declassify On:" line.
    let text = "Declassify On: 2030-12-31";
    let parsed = parse_cab_text(text);
    assert_eq!(
        parsed.attrs.declassify_on,
        Some(marque_ism::IsmDate::Date(2030, 12, 31)),
        "YYYY-MM-DD in CAB should set declassify_on to Date"
    );
}

#[test]
fn cab_declassify_on_exemption_sets_exemption_not_date() {
    // A declassification exemption code must not be stored in declassify_on.
    let text = "Declassify On: 50X1-HUM";
    let parsed = parse_cab_text(text);
    assert!(
        parsed.attrs.declassify_on.is_none(),
        "exemption code must not set declassify_on"
    );
    assert!(
        parsed.attrs.declass_exemption.is_some(),
        "exemption code must set declass_exemption"
    );
}

#[test]
fn cab_declassify_on_invalid_date_silently_ignored() {
    // Unrecognized strings are silently dropped — no panic, declassify_on stays None.
    let text = "Declassify On: UNRECOGNIZED";
    let parsed = parse_cab_text(text);
    assert!(
        parsed.attrs.declassify_on.is_none(),
        "unrecognized Declassify On value should leave declassify_on as None"
    );
    assert!(parsed.attrs.declass_exemption.is_none());
}

#[test]
fn cab_classified_by_and_derived_from_populated() {
    let text = "Classified By: Jane Doe\nDerived From: SCG-2024\nDeclassify On: 20301231";
    let parsed = parse_cab_text(text);
    assert_eq!(
        parsed.attrs.classified_by.as_deref(),
        Some("Jane Doe"),
        "classified_by should be populated"
    );
    assert_eq!(
        parsed.attrs.derived_from.as_deref(),
        Some("SCG-2024"),
        "derived_from should be populated"
    );
    assert_eq!(
        parsed.attrs.declassify_on,
        Some(marque_ism::IsmDate::Date(2030, 12, 31))
    );
}

#[test]
fn cab_without_declassify_on_leaves_both_none() {
    let text = "Classified By: Jane Doe\nDerived From: SCG-2024";
    let parsed = parse_cab_text(text);
    assert!(parsed.attrs.declassify_on.is_none());
    assert!(parsed.attrs.declass_exemption.is_none());
}

// -----------------------------------------------------------------------
// Portion declass date (is_declass_date path in parse_marking_string)
// -----------------------------------------------------------------------

#[test]
fn portion_with_yyyymmdd_sets_declassify_on() {
    // A portion that (erroneously) contains an inline declass date; the
    // parser must populate declassify_on so E005 can fire.
    let parsed = parse_portion("(SECRET//20301231//NOFORN)");
    assert_eq!(
        parsed.attrs.declassify_on,
        Some(marque_ism::IsmDate::Date(2030, 12, 31)),
        "YYYYMMDD in portion should set declassify_on"
    );
}

#[test]
fn portion_with_yyyy_sets_declassify_on() {
    let parsed = parse_portion("(SECRET//2035)");
    assert_eq!(
        parsed.attrs.declassify_on,
        Some(marque_ism::IsmDate::Year(2035)),
        "YYYY in portion should set declassify_on"
    );
}

#[test]
fn is_declass_date_rejects_leap_day_non_leap_year() {
    // 2003 is not a leap year; Feb 29 is impossible.
    assert!(!is_declass_date("20030229"));
}

#[test]
fn is_declass_date_accepts_leap_day_in_leap_year() {
    assert!(is_declass_date("20040229")); // 2004 is a leap year
    assert!(is_declass_date("20000229")); // 2000 is a leap year
}

#[test]
fn is_declass_date_rejects_day_zero() {
    assert!(!is_declass_date("20030100")); // day 0 is impossible
}

// -------------------------------------------------------------------
// T135a — deprecated SCI long-form recognition (issue #307 Group D).
//
// The recognizer accepts the deprecated long forms (HUMINT, COMINT,
// SPECIAL INTELLIGENCE, ECI <COMP>, EL <COMP>, KDK-<COMP>,
// KLONDIKE-<COMP>, etc.) as their canonical SCI category internally
// while preserving source bytes verbatim in `TokenSpan.text`. The
// Commit 3 walker rule (E065) consumes the preserved text to emit
// canonicalization fixes.
//
// Authority: CAPCO-2016 §H.4 pp 61, 62, 74, 76, 78, 85.
// -------------------------------------------------------------------

#[test]
fn humint_bare_recognized_as_hcs_with_source_preserved() {
    // §H.4 p62 — HUMINT is the legacy long form for HCS.
    let parsed = parse_banner("TOP SECRET//HUMINT//NOFORN");
    assert!(
        parsed.attrs.sci_controls.contains(&SciControl::Hcs),
        "HUMINT must map to SciControl::Hcs internally; sci_controls = {:?}",
        parsed.attrs.sci_controls
    );
    let humint_span = parsed
        .attrs
        .token_spans
        .iter()
        .find(|t| &*t.text == "HUMINT")
        .expect("source bytes must be preserved verbatim in a TokenSpan");
    assert_eq!(humint_span.kind, TokenKind::SciControl);
}

#[test]
fn humint_control_system_recognized_as_hcs() {
    // §H.4 p62 — HUMINT CONTROL SYSTEM is the spelled-out form.
    let parsed = parse_banner("TOP SECRET//HUMINT CONTROL SYSTEM//NOFORN");
    assert!(parsed.attrs.sci_controls.contains(&SciControl::Hcs));
    let span = parsed
        .attrs
        .token_spans
        .iter()
        .find(|t| &*t.text == "HUMINT CONTROL SYSTEM")
        .expect("multi-word phrase preserved verbatim");
    assert_eq!(span.kind, TokenKind::SciControl);
}

#[test]
fn comint_recognized_as_si() {
    // §H.4 p74 — "The COMINT title for the Special Intelligence (SI)
    // control system is no longer valid".
    let parsed = parse_banner("TOP SECRET//COMINT//NOFORN");
    assert!(parsed.attrs.sci_controls.contains(&SciControl::Si));
    let span = parsed
        .attrs
        .token_spans
        .iter()
        .find(|t| &*t.text == "COMINT")
        .expect("COMINT preserved verbatim");
    assert_eq!(span.kind, TokenKind::SciControl);
}

#[test]
fn special_intelligence_recognized_as_si() {
    let parsed = parse_banner("TOP SECRET//SPECIAL INTELLIGENCE//NOFORN");
    assert!(parsed.attrs.sci_controls.contains(&SciControl::Si));
    let span = parsed
        .attrs
        .token_spans
        .iter()
        .find(|t| &*t.text == "SPECIAL INTELLIGENCE")
        .expect("SPECIAL INTELLIGENCE preserved verbatim");
    assert_eq!(span.kind, TokenKind::SciControl);
}

#[test]
fn eci_with_compartment_maps_to_si_compartment() {
    // §H.4 p76 — "information formerly marked TS//SI-ECI ABC must
    // now be marked TS//SI-ABC".
    let parsed = parse_banner("TOP SECRET//ECI ABC//NOFORN");
    // Compound form: canonical_enum may or may not resolve depending
    // on whether `SI-ABC` is a published CVE entry. The structural
    // SciMarking must carry the compartment regardless.
    let marking = parsed
        .attrs
        .sci_markings
        .iter()
        .next()
        .expect("SciMarking emitted for ECI ABC");
    assert_eq!(
        marking.system,
        SciControlSystem::Published(SciControlBare::Si)
    );
    assert_eq!(marking.compartments.len(), 1);
    assert_eq!(&*marking.compartments[0].identifier, "ABC");
    // Source bytes preserved.
    let span = parsed
        .attrs
        .token_spans
        .iter()
        .find(|t| &*t.text == "ECI ABC")
        .expect("ECI compound form preserved verbatim");
    assert_eq!(span.kind, TokenKind::SciControl);
}

#[test]
fn bare_eci_recognized_as_si_no_compartment() {
    // Bare ECI (no compartment) is recognized so the walker can
    // emit a suggest-only diagnostic asking the author to contact
    // the originator for the compartment context.
    let parsed = parse_banner("TOP SECRET//ECI//NOFORN");
    let marking = parsed
        .attrs
        .sci_markings
        .iter()
        .next()
        .expect("SciMarking emitted for bare ECI");
    assert_eq!(
        marking.system,
        SciControlSystem::Published(SciControlBare::Si)
    );
    assert_eq!(marking.compartments.len(), 0);
}

#[test]
fn el_ecru_maps_to_si_with_ecru_compartment() {
    // §H.4 p78 — "the EL control system is being retired and all
    // associated compartments moved to the SI control system".
    // The structural form is SI + compartment ECRU. The textual
    // CAPCO canonical is `SI-ECRU` per §H.4 p78 prose, but the
    // ODNI CVE catalog publishes only `SI-EU` (the 5-char abbreviated
    // banner-abbreviation form per §H.4 p78), so `canonical_enum`
    // resolves to None here — the walker emits the fix using the
    // textual canonical (`SI-ECRU`), not the CVE abbreviation.
    let parsed = parse_banner("TOP SECRET//EL ECRU//NOFORN");
    let marking = parsed
        .attrs
        .sci_markings
        .iter()
        .next()
        .expect("SciMarking emitted for EL ECRU");
    assert_eq!(
        marking.system,
        SciControlSystem::Published(SciControlBare::Si)
    );
    assert_eq!(&*marking.compartments[0].identifier, "ECRU");
}

#[test]
fn endseal_with_compartment_maps_to_si() {
    let parsed = parse_banner("TOP SECRET//ENDSEAL ECRU//NOFORN");
    let marking = parsed
        .attrs
        .sci_markings
        .iter()
        .next()
        .expect("SciMarking emitted for ENDSEAL ECRU");
    assert_eq!(
        marking.system,
        SciControlSystem::Published(SciControlBare::Si)
    );
    assert_eq!(&*marking.compartments[0].identifier, "ECRU");
}

#[test]
fn kdk_bluefish_maps_to_tk_blfh() {
    // §H.4 p85 (NSG PM 3802 closure) — "re-mark the new document
    // and associated portions according to the instructions in the
    // TK-BLFH, TK-IDIT, and TK-KAND marking templates".
    //
    // CRITICAL: pre-T135a, KDK-BLUEFISH would have routed through
    // `parse_sci_block` as `SciControlSystem::Custom("KDK")` with
    // compartment `BLUEFISH` because `KDK` is a 3-letter custom-
    // control shape match. The new recognizer must fire FIRST to
    // route it to SciControlBare::Tk + compartment "BLUEFISH".
    let parsed = parse_banner("TOP SECRET//KDK-BLUEFISH//NOFORN");
    let marking = parsed
        .attrs
        .sci_markings
        .iter()
        .next()
        .expect("SciMarking emitted for KDK-BLUEFISH");
    assert_eq!(
        marking.system,
        SciControlSystem::Published(SciControlBare::Tk),
        "KDK-BLUEFISH must map to TK, not Custom(\"KDK\")"
    );
    assert_eq!(&*marking.compartments[0].identifier, "BLUEFISH");
}

#[test]
fn klondike_iditarod_maps_to_tk() {
    let parsed = parse_banner("TOP SECRET//KLONDIKE-IDITAROD//NOFORN");
    let marking = parsed
        .attrs
        .sci_markings
        .iter()
        .next()
        .expect("SciMarking emitted for KLONDIKE-IDITAROD");
    assert_eq!(
        marking.system,
        SciControlSystem::Published(SciControlBare::Tk)
    );
    assert_eq!(&*marking.compartments[0].identifier, "IDITAROD");
}

#[test]
fn bare_kdk_recognized_as_tk_no_compartment() {
    // Bare KDK (compartment context missing) is recognized so the
    // walker can emit a suggest-only diagnostic.
    let parsed = parse_banner("TOP SECRET//KDK//NOFORN");
    let marking = parsed
        .attrs
        .sci_markings
        .iter()
        .next()
        .expect("SciMarking emitted for bare KDK");
    assert_eq!(
        marking.system,
        SciControlSystem::Published(SciControlBare::Tk)
    );
    assert_eq!(marking.compartments.len(), 0);
}

// -------------------------------------------------------------------
// T135a Commit 5 — EYES / EYES ONLY compound block recognition
// (issue #307). CAPCO-2016 §H.8 p157.
//
// The recognizer accepts `<TRIGRAPH>(/<TRIGRAPH>)* EYES [ONLY]` as
// a single block-token in the dissem axis, populating
// `DissemControl::Eyes` and preserving source bytes verbatim in
// `TokenSpan.text`. Without this recognizer the multi-token block
// handler splits on `/` and emits each trigraph as Unknown.
// -------------------------------------------------------------------

#[test]
fn eyes_only_compound_block_recognized() {
    // §H.8 p157 — EYES ONLY block with Five Eyes trigraph list.
    // Pre-T135a Commit 5 this would be a 3-token Unknown soup;
    // now it lands as a single DissemControl::Eyes block with
    // source bytes preserved verbatim.
    let parsed = parse_portion("(S//USA/GBR/CAN EYES ONLY)");
    let eyes_tokens: Vec<&TokenSpan> = parsed
        .attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::DissemControl && &*t.text == "USA/GBR/CAN EYES ONLY")
        .collect();
    assert_eq!(
        eyes_tokens.len(),
        1,
        "exactly one DissemControl token preserving the source-bytes block expected"
    );

    // Dissem axis carries EYES. US-classified portion attributes
    // dissem to dissem_us per CAPCO-2016 p41 (PR 9b / FR-046).
    assert!(
        parsed
            .attrs
            .dissem_iter()
            .any(|d| d == &marque_ism::DissemControl::Eyes),
        "DissemControl::Eyes must be populated"
    );

    // No Unknown tokens — the recognizer succeeded.
    let unknowns: Vec<&TokenSpan> = parsed
        .attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .collect();
    assert!(
        unknowns.is_empty(),
        "no Unknown tokens expected; got {:?}",
        unknowns.iter().map(|t| &*t.text).collect::<Vec<_>>()
    );
}

#[test]
fn eyes_only_short_form_recognized() {
    // `EYES` without `ONLY` is also accepted per §H.8 p157.
    let parsed = parse_portion("(S//USA/GBR EYES)");
    let eyes_tokens: Vec<&TokenSpan> = parsed
        .attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::DissemControl && &*t.text == "USA/GBR EYES")
        .collect();
    assert_eq!(eyes_tokens.len(), 1);
    assert!(
        parsed
            .attrs
            .dissem_iter()
            .any(|d| d == &marque_ism::DissemControl::Eyes)
    );
}

#[test]
fn eyes_only_bare_block_unaffected() {
    // `EYES` alone (no trigraph list) is the bare CVE dissem-token
    // case; it flows through `DissemControl::parse` unchanged.
    // This regression guard catches a stray recognizer match.
    let parsed = parse_portion("(S//EYES)");
    assert!(
        parsed
            .attrs
            .dissem_iter()
            .any(|d| d == &marque_ism::DissemControl::Eyes),
        "bare EYES must parse as DissemControl::Eyes via the CVE path"
    );
}

#[test]
fn source_bytes_preserved_for_all_long_forms() {
    // Regression guard: the parser must NEVER rewrite the user's
    // input. Every recognized deprecated long form must carry its
    // original source bytes verbatim in `TokenSpan.text`. The
    // walker rule (Commit 3) uses these bytes to emit the
    // canonicalization fix.
    for input in [
        "TOP SECRET//HUMINT//NOFORN",
        "TOP SECRET//HUMINT CONTROL SYSTEM//NOFORN",
        "TOP SECRET//COMINT//NOFORN",
        "TOP SECRET//SPECIAL INTELLIGENCE//NOFORN",
        "TOP SECRET//ECI ABC//NOFORN",
        "TOP SECRET//EXCEPTIONALLY CONTROLLED INFORMATION ABC//NOFORN",
        "TOP SECRET//EL ECRU//NOFORN",
        "TOP SECRET//ENDSEAL ECRU//NOFORN",
        "TOP SECRET//KDK-BLUEFISH//NOFORN",
        "TOP SECRET//KLONDIKE-IDITAROD//NOFORN",
    ] {
        let parsed = parse_banner(input);
        // The first `//` after TOP SECRET, then the long-form block.
        let want = input
            .strip_prefix("TOP SECRET//")
            .and_then(|s| s.strip_suffix("//NOFORN"))
            .unwrap();
        assert!(
            parsed.attrs.token_spans.iter().any(|t| &*t.text == want),
            "{input:?}: source bytes {want:?} must appear verbatim in token_spans"
        );
    }
}
