use super::*;

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
    let src = b"(TS//SI/TK//NF)";
    assert!(
        parsed
            .attrs
            .token_spans
            .iter()
            .any(|t| t.kind == TokenKind::Separator && t.span.as_str(src) == Ok("/")),
        "SCI within-category slash must emit a TokenKind::Separator span"
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
