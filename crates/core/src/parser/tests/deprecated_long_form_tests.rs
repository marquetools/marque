use super::*;

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
    // route it to SciControlBare::Tk + canonical compartment "BLFH".
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
    assert_eq!(&*marking.compartments[0].identifier, "BLFH");
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
    assert_eq!(&*marking.compartments[0].identifier, "IDIT");
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
