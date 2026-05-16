// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Banner long-form recognition for SCI controls and FGI markers.
//!
//! CAPCO-2016 §D.1 p27: "Any control markings in the banner line may
//! be spelled out per the 'Marking Title' (e.g., TALENT KEYHOLE) or
//! abbreviated as per the 'Authorized Abbreviation' (e.g., TK) in
//! accordance with the Register."
//!
//! Pre-fix, the strict parser called `SciControl::parse(trimmed)` and
//! `parse_fgi_marker(trimmed)` with no long-form fallback, so banner
//! input like `TOP SECRET//SI/TALENT KEYHOLE//FOREIGN GOVERNMENT
//! INFORMATION GBR NZL//NOFORN` surfaced as `Unknown` spans and the
//! rule layer emitted spurious E008 errors. Real-world IC content in
//! `tests/corpus/documents/marked/CIA-RDP09T00207R001000100002-2.md`
//! exercised this gap (two E008 hits on a single banner line).
//!
//! This test pins the long-form admission contract:
//!
//! - `TALENT KEYHOLE` → `SciControl::Tk` (CAPCO-2016 §H.4 p85)
//! - `MARVEL` → `SciControl::Mvl` (ODNI ISM `CVEnumISMSCIControls.xml`
//!   Description; post-CAPCO-2016)
//! - `KLAMATH` → `SciControl::Klm` (ODNI ISM Description; post-CAPCO)
//! - `FOREIGN GOVERNMENT INFORMATION` → `FgiMarker::SourceConcealed`
//!   (CAPCO-2016 §H.7 p123)
//! - `FOREIGN GOVERNMENT INFORMATION [LIST]` →
//!   `FgiMarker::Acknowledged` (CAPCO-2016 §H.7 p123)

use marque_core::Parser;
use marque_ism::attrs::{FgiMarker, SciControl, TokenKind};
use marque_ism::span::{MarkingCandidate, MarkingType, Span};
use marque_ism::token_set::CapcoTokenSet;

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

fn unknown_spans(attrs: &marque_ism::ParsedAttrs<'_>) -> Vec<(String, usize, usize)> {
    attrs
        .token_spans
        .iter()
        .filter(|t| t.kind == TokenKind::Unknown)
        .map(|t| (t.text.to_string(), t.span.start, t.span.end))
        .collect()
}

// ---------------------------------------------------------------------------
// SCI long-form bare controls (§D.1 p27 + §H.4 p85)
// ---------------------------------------------------------------------------

#[test]
fn talent_keyhole_in_sci_block_resolves_to_tk() {
    // CAPCO-2016 §H.4 p85: "TALENT KEYHOLE — Authorized Banner Line
    // Marking Title: TALENT KEYHOLE". `SI/TALENT KEYHOLE` exercises
    // the sub-block dispatch path inside `parse_sci_block` where SCI
    // entries are split on `/` within a `//`-delimited block.
    let attrs = parse_banner("TOP SECRET//SI/TALENT KEYHOLE//NOFORN");
    let unknown = unknown_spans(&attrs);
    assert!(
        unknown.is_empty(),
        "TALENT KEYHOLE must parse with no Unknown spans; got {unknown:?}"
    );
    let sci: Vec<SciControl> = attrs.sci_controls.to_vec();
    assert!(sci.contains(&SciControl::Si), "missing SI in {sci:?}");
    assert!(sci.contains(&SciControl::Tk), "missing TK in {sci:?}");
}

#[test]
fn talent_keyhole_at_top_level_resolves_to_tk() {
    // Top-level dispatch path: `//TALENT KEYHOLE//` (no preceding SCI
    // control like `SI/`), exercising the primary block-level
    // `SciControl::parse(...).or_else(parse_sci_control_full_form)`
    // arm rather than the sub-block path covered above.
    let attrs = parse_banner("TOP SECRET//TALENT KEYHOLE//NOFORN");
    let unknown = unknown_spans(&attrs);
    assert!(
        unknown.is_empty(),
        "top-level TALENT KEYHOLE must parse with no Unknown spans; got {unknown:?}"
    );
    let sci: Vec<SciControl> = attrs.sci_controls.to_vec();
    assert_eq!(sci, vec![SciControl::Tk]);
}

#[test]
fn marvel_bare_long_form_resolves_to_mvl() {
    // ODNI ISM `CVEnumISMSCIControls.xml`: `MVL` Value, `MARVEL`
    // Description. Post-CAPCO-2016; admitted via the ODNI schema
    // (project memory `project_ism_data_external`).
    let attrs = parse_banner("SECRET//MARVEL//NOFORN");
    let unknown = unknown_spans(&attrs);
    assert!(
        unknown.is_empty(),
        "MARVEL must parse with no Unknown spans; got {unknown:?}"
    );
    let sci: Vec<SciControl> = attrs.sci_controls.to_vec();
    assert_eq!(sci, vec![SciControl::Mvl]);
}

#[test]
fn klamath_bare_long_form_resolves_to_klm() {
    // ODNI ISM `CVEnumISMSCIControls.xml`: `KLM` Value, `KLAMATH`
    // Description.
    let attrs = parse_banner("SECRET//KLAMATH//NOFORN");
    let unknown = unknown_spans(&attrs);
    assert!(
        unknown.is_empty(),
        "KLAMATH must parse with no Unknown spans; got {unknown:?}"
    );
    let sci: Vec<SciControl> = attrs.sci_controls.to_vec();
    assert_eq!(sci, vec![SciControl::Klm]);
}

// ---------------------------------------------------------------------------
// FGI long-form (§H.7 p123)
// ---------------------------------------------------------------------------

#[test]
fn fgi_long_form_concealed_resolves_to_source_concealed() {
    // CAPCO-2016 §H.7 p123: "Authorized Banner Line Marking Title
    // (when source must be concealed): FOREIGN GOVERNMENT INFORMATION".
    let attrs = parse_banner("SECRET//FOREIGN GOVERNMENT INFORMATION//NOFORN");
    let unknown = unknown_spans(&attrs);
    assert!(
        unknown.is_empty(),
        "FOREIGN GOVERNMENT INFORMATION (concealed) must parse with no Unknown spans; \
         got {unknown:?}"
    );
    let marker = attrs.fgi_marker.expect("FGI marker must be set");
    assert!(matches!(marker.value, FgiMarker::SourceConcealed));
}

#[test]
fn fgi_long_form_acknowledged_with_list_resolves_to_acknowledged() {
    // CAPCO-2016 §H.7 p123: "Authorized Banner Line Marking Title
    // (when source is acknowledged): FOREIGN GOVERNMENT INFORMATION
    // [LIST]". Real-world fixture:
    // tests/corpus/documents/marked/CIA-RDP09T00207R001000100002-2.md
    // banner uses `FOREIGN GOVERNMENT INFORMATION GBR NZL`.
    let attrs = parse_banner("SECRET//FOREIGN GOVERNMENT INFORMATION GBR NZL//NOFORN");
    let unknown = unknown_spans(&attrs);
    assert!(
        unknown.is_empty(),
        "FOREIGN GOVERNMENT INFORMATION [LIST] must parse with no Unknown spans; \
         got {unknown:?}"
    );
    let marker = attrs.fgi_marker.expect("FGI marker must be set");
    let countries = marker.value.countries();
    assert_eq!(countries.len(), 2, "expected GBR + NZL: {countries:?}");
    let codes: Vec<&str> = countries.iter().map(|c| c.as_str()).collect();
    assert_eq!(codes, vec!["GBR", "NZL"]);
}

#[test]
fn fgi_long_form_single_country() {
    // §H.7 p123: shape `FOREIGN GOVERNMENT INFORMATION [LIST]` with
    // exactly one trigraph in [LIST]. Smaller variant of the
    // acknowledged-form test above; pins that a one-element list
    // doesn't accidentally fall through to source-concealed.
    let attrs = parse_banner("SECRET//FOREIGN GOVERNMENT INFORMATION GBR//NOFORN");
    let unknown = unknown_spans(&attrs);
    assert!(
        unknown.is_empty(),
        "single-country FGI long-form must parse cleanly; got {unknown:?}"
    );
    let marker = attrs.fgi_marker.expect("FGI marker must be set");
    let codes: Vec<&str> = marker
        .value
        .countries()
        .iter()
        .map(|c| c.as_str())
        .collect();
    assert_eq!(codes, vec!["GBR"]);
}

// ---------------------------------------------------------------------------
// Gate lock-step coverage — `starts_with_fgi_prefix` MUST admit every
// prefix that `parse_fgi_marker` accepts. If a new long-form is added
// to `parse_fgi_marker` without updating the gate, the block-dispatch
// arm would silently skip the call. This test fails loudly in that
// scenario.
// ---------------------------------------------------------------------------

#[test]
fn fgi_gate_lock_steps_with_parser() {
    // Every input that `parse_fgi_marker` should accept MUST first
    // pass through the `starts_with_fgi_prefix` gate (the gate is
    // checked before `parse_fgi_marker` is called in the block-walking
    // parser). We exercise the contract end-to-end: if any of these
    // inputs lands as Unknown, either the gate or the parser drifted.
    let lockstep_inputs = [
        "SECRET//FGI",
        "SECRET//FGI GBR",
        "SECRET//FGI GBR NZL",
        "SECRET//FOREIGN GOVERNMENT INFORMATION",
        "SECRET//FOREIGN GOVERNMENT INFORMATION GBR",
        "SECRET//FOREIGN GOVERNMENT INFORMATION GBR NZL",
    ];
    for input in lockstep_inputs {
        let attrs = parse_banner(input);
        assert!(
            attrs.fgi_marker.is_some(),
            "gate/parser drift: {input:?} reached the parser via the FGI gate \
             but produced no marker"
        );
    }
}

// ---------------------------------------------------------------------------
// Negative tests — partial / malformed inputs must NOT silently parse
// ---------------------------------------------------------------------------

#[test]
fn fgi_prefix_without_canonical_separator_does_not_match() {
    // Inputs that don't carry the canonical `FGI ` / `FOREIGN
    // GOVERNMENT INFORMATION ` separator must fall through to the next
    // dispatch arm (per the Case-3 short-circuit in
    // `parse_fgi_marker`). `FOREIGN GOVERNMENT INFORMATIONGBR` (no
    // space) must not parse as an acknowledged FGI marker.
    let attrs = parse_banner("SECRET//FOREIGN GOVERNMENT INFORMATIONGBR");
    assert!(
        attrs.fgi_marker.is_none(),
        "missing canonical separator must NOT match the long-form prefix"
    );
}

#[test]
fn talent_keyhole_does_not_match_partial_prefix() {
    // `TALENT` alone is not a registered marking; the long-form
    // fallback must only fire on the full canonical title.
    let attrs = parse_banner("TOP SECRET//SI/TALENT//NOFORN");
    let unknown = unknown_spans(&attrs);
    // The bare "TALENT" sub-block should land as Unknown — the
    // fallback isn't a fuzzy match.
    assert!(
        unknown.iter().any(|(t, _, _)| t == "TALENT"),
        "partial 'TALENT' must remain Unknown (not silently match TK); got {unknown:?}"
    );
}

#[test]
fn fgi_short_form_with_lowercase_country_emits_unknown() {
    // `starts_with_fgi_prefix("FGI deu")` passes the gate, but the
    // parser rejects lowercase per `admits_country_token` (ASCII
    // upper). The block must not silently drop — emit Unknown so
    // E008 can flag the malformed list.
    let attrs = parse_banner("SECRET//FGI deu//NOFORN");
    let unknown = unknown_spans(&attrs);
    assert!(
        unknown.iter().any(|(t, _, _)| t == "FGI deu"),
        "malformed `FGI deu` must surface as Unknown; got {unknown:?}"
    );
    assert!(
        attrs.fgi_marker.is_none(),
        "malformed input must not produce an FgiMarker"
    );
}

#[test]
fn fgi_long_form_with_invalid_country_token_emits_unknown() {
    // The long-form gate widened the prefix set; a numeric-only
    // country token like `99` passes the gate (long-form prefix
    // matches) but fails the parser. Must surface as Unknown.
    let attrs = parse_banner("SECRET//FOREIGN GOVERNMENT INFORMATION 99//NOFORN");
    let unknown = unknown_spans(&attrs);
    assert!(
        unknown
            .iter()
            .any(|(t, _, _)| t == "FOREIGN GOVERNMENT INFORMATION 99"),
        "malformed long-form FGI with invalid country token must surface as Unknown; got {unknown:?}"
    );
    assert!(
        attrs.fgi_marker.is_none(),
        "malformed long-form input must not produce an FgiMarker"
    );
}

#[test]
fn fgi_long_form_with_lowercase_country_emits_unknown() {
    // Lowercase country in the long-form path — same silent-drop
    // class as `FGI deu` but via the 30-byte prefix. Must surface
    // as Unknown.
    let attrs = parse_banner("SECRET//FOREIGN GOVERNMENT INFORMATION deu//NOFORN");
    let unknown = unknown_spans(&attrs);
    assert!(
        unknown
            .iter()
            .any(|(t, _, _)| t == "FOREIGN GOVERNMENT INFORMATION deu"),
        "malformed long-form FGI with lowercase country must surface as Unknown; got {unknown:?}"
    );
    assert!(
        attrs.fgi_marker.is_none(),
        "malformed long-form input must not produce an FgiMarker"
    );
}
