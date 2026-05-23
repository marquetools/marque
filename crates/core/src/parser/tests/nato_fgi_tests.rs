use super::*;

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

/// Legacy compound NATO classification text (e.g.,
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

/// Portion-form legacy compounds (`CTSA`, `CTS-A`,
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

// ---- Admission closure for parse_fgi_marker ----
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
    // Lowercase fails `admits_fgi_trigraph`. A degraded fallback
    // would silently drop the token, produce an empty country list,
    // and fall back to `SourceConcealed`; instead the parser returns
    // `None`, so `attrs.fgi_marker` is unset (CAPCO §H.7 p122
    // disallows a degraded lawful form on shape failure).
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
    // This pins the boundary: shape vs. registry. `XYZ` is accepted
    // here because `CountryCode::try_new` succeeds on 3 ASCII upper;
    // the gate's semantics are scoped to shape, not registry membership.
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

    // Case 3: lowercase token → None (closure invariant)
    assert!(parse_fgi_marker("FGI deu").is_none());

    // Case 2 (mixed shapes per §H.7 p122): trigraph + tetragraph
    // → Some(Acknowledged) with both countries. Admission accepts
    // the spec-canonical example rather than narrowing to trigraphs
    // only (issue #311).
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
