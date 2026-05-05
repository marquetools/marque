// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T096 — FGI / SAR silent-skip regression guard (FR-015 / FR-016 / SC-011).
//!
//! Closure of the four open-vocabulary parser admission sites migrated in
//! PR 2 of the engine-rule refactor (specs/006-engine-rule-refactor):
//!
//! 1. `parse_fgi_marker` → `CountryCode::admits_country_token`
//!    (3-letter Annex B trigraph OR 4-letter Annex A tetragraph OR
//!    2-letter registered exception code per §H.7 p123 + ISMCAT
//!    CVEnumISMCATRelTo)
//! 2. SAR program identifier (abbrev) → `SarProgram::admits_program_id_abbrev`
//! 3. SAR compartment identifier → `SarCompartment::admits_identifier`
//! 4. SAR sub-compartment identifier → `SarCompartment::admits_identifier`
//!
//! Pre-PR-2 these sites used inline `is_ascii_alphanumeric()` byte-class checks
//! (or, in the FGI case, a length-3 + `try_new` shortcut that silently dropped
//! shape-failing tokens). The replacement contract is FR-016: shape failure →
//! `None`, never a degraded `Some` shape (`FgiMarker { countries: [] }` was the
//! specific ghost shape FR-017's `enum FgiMarker { SourceConcealed, Acknowledged
//! { countries: SmallVec<…> } }` discriminant rules out).
//!
//! These tests pin the contract from the **outside** of the parser — they do
//! not import the private `parse_fgi_marker` helper or reach into private
//! parser fields. They drive the public `Parser::parse` entry point with
//! synthetic `MarkingCandidate`s (the same surface `marque-engine` reaches in
//! production) and inspect `IsmAttributes::fgi_marker` / `IsmAttributes::sar_markings`.
//!
//! A regression that silently re-introduces shape-failure-as-`Some` would
//! produce the wrong observable on these inputs and trip every relevant test.
//!
//! # Authority
//!
//! - CAPCO-2016 §H.7 p123 — FGI banner forms (concealed `FGI`, acknowledged
//!   `FGI [LIST]`); per-country list separator and tetragraph rules at §A.6
//!   p16 ("Multiple FGI trigraph country codes or tetragraph codes must be
//!   separated by a single space").
//! - CAPCO-2016 §H.5 pp99–101 — SAR grammar: program identifier shape (abbrev:
//!   2–3 alphanumeric per p101; full: uppercase + spaces per Table 7 p100),
//!   compartment / sub-compartment identifier shape (alphanumeric, p99).
//!
//! # Spec linkage
//!
//! - FR-015 (admission via documented vocabulary surface)
//! - FR-016 (`parse_fgi_marker` returns `None` on shape failure)
//! - FR-017 (`FgiMarker` discriminant: `SourceConcealed` ⊕ `Acknowledged`)
//! - SC-011 (no `FgiMarker { countries: [] }` shape survives in parser output)

use marque_core::Parser;
use marque_ism::attrs::FgiMarker;
use marque_ism::span::{MarkingCandidate, MarkingType, Span};
use marque_ism::token_set::CapcoTokenSet;
use marque_ism::IsmAttributes;

/// Drive `Parser::parse` over `text` interpreted as a banner candidate and
/// return the resulting `IsmAttributes`. Mirrors the engine's banner-path
/// dispatch (`marque-engine` constructs banner candidates the same way for
/// any byte slice the scanner identified as a banner).
///
/// Returns `None` if the parser refused the candidate outright (e.g., invalid
/// UTF-8 — not in scope for these tests). The relevant assertion shape for
/// shape-admission failures is `attrs.fgi_marker.is_none()` /
/// `attrs.sar_markings.is_none()`, NOT a hard parse error: the parser is
/// lenient on individual blocks and surfaces shape failures as missing
/// attribute fields plus `Unknown` token spans.
fn parse_banner_attrs(text: &str) -> IsmAttributes {
    let source = text.as_bytes();
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidate = MarkingCandidate {
        span: Span::new(0, source.len()),
        kind: MarkingType::Banner,
    };
    parser
        .parse(&candidate, source)
        .expect("banner candidate parses (lenient parser; shape failures surface as None fields)")
        .attrs
}

/// Drive `Parser::parse` over `text` interpreted as a portion candidate.
///
/// `text` MUST include the outer parentheses — the parser strips them and
/// rejects un-parenthesized portion text outright (which is the wrong
/// surface for these admission tests).
fn parse_portion_attrs(text: &str) -> IsmAttributes {
    let source = text.as_bytes();
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidate = MarkingCandidate {
        span: Span::new(0, source.len()),
        kind: MarkingType::Portion,
    };
    parser
        .parse(&candidate, source)
        .expect("portion candidate parses (lenient parser; shape failures surface as None fields)")
        .attrs
}

// =============================================================================
// FGI marker — `parse_fgi_marker` four-case enforcement (FR-016 / FR-017)
// =============================================================================

#[test]
fn parse_fgi_marker_bare_fgi_yields_source_concealed() {
    // CAPCO-2016 §H.7 p123: bare `FGI` is the lawful source-concealed banner
    // form ("FOREIGN GOVERNMENT INFORMATION (when country[ies] or
    // organization[s] of origin must be concealed)"). The parser must
    // produce `Some(SourceConcealed)`, not the pre-FR-017 collision shape
    // `Some(FgiMarker { countries: [] })`.
    let attrs = parse_banner_attrs("SECRET//FGI//NOFORN");
    let marker = attrs
        .fgi_marker
        .as_ref()
        .expect("bare FGI must yield Some(SourceConcealed)");
    assert!(
        matches!(marker, FgiMarker::SourceConcealed),
        "bare FGI must be SourceConcealed (CAPCO §H.7 p123), got {marker:?}",
    );
    // FR-017 invariant: the lawful concealed form has no countries.
    assert!(
        marker.countries().is_empty(),
        "SourceConcealed has no countries by definition",
    );
}

#[test]
fn parse_fgi_marker_acknowledged_yields_countries() {
    // CAPCO-2016 §H.7 p123: `FGI [LIST]` is the acknowledged form. The
    // country list is non-empty by construction
    // (`FgiMarker::acknowledged(...)` returns `None` on empty input).
    let attrs = parse_banner_attrs("SECRET//FGI DEU//NOFORN");
    let marker = attrs
        .fgi_marker
        .as_ref()
        .expect("FGI DEU must yield Some(Acknowledged)");
    match marker {
        FgiMarker::Acknowledged { countries, .. } => {
            assert_eq!(countries.len(), 1, "exactly one country recorded");
            assert_eq!(
                countries[0].as_str(),
                "DEU",
                "country trigraph round-trips through admits_country_token",
            );
        }
        FgiMarker::SourceConcealed => panic!("expected Acknowledged variant, got SourceConcealed"),
    }
}

#[test]
fn parse_fgi_marker_lowercase_trigraph_yields_no_marker() {
    // FR-016: post-prefix bytes failing `shape_admits` MUST return `None` —
    // not silently drop the lowercase token and fall back to a degraded
    // `SourceConcealed` (which was the pre-FR-016 surface).
    //
    // CAPCO §H.7 p123 + §A.6 p16 require trigraph or tetragraph country
    // codes; both registries are uppercase-canonical. Lowercase fails
    // `CountryCode::admits_country_token` at the parser admission gate.
    let attrs = parse_banner_attrs("SECRET//FGI deu//NOFORN");
    assert!(
        attrs.fgi_marker.is_none(),
        "lowercase trigraph must yield None at the FGI admission gate \
         (FR-016); got {:?}",
        attrs.fgi_marker,
    );
}

#[test]
fn parse_fgi_marker_mixed_trigraph_tetragraph_yields_acknowledged() {
    // CAPCO-2016 §H.7 p123 spells out the canonical example:
    // `SECRET//FGI GBR JPN NATO//REL TO USA, GBR, JPN, NATO`
    // — `NATO` is a 4-letter Annex A tetragraph admitted in the same
    // FGI list as the 3-letter trigraphs. The PR #311 review caught
    // that the prior `admits_fgi_trigraph`-only gate silently
    // rejected this lawful spec example; this test pins the
    // post-fix `admits_country_token` contract (3 OR 4 ASCII upper
    // admit) so a future narrowing regression is caught here.
    use marque_ism::CountryCode;
    let attrs = parse_banner_attrs("SECRET//FGI USA NATO//NOFORN");
    let marker = attrs
        .fgi_marker
        .expect("mixed trigraph + tetragraph FGI list must admit per §H.7 p123");
    let countries = marker.countries();
    assert_eq!(
        countries.len(),
        2,
        "FGI USA NATO must produce two-country Acknowledged; got {countries:?}",
    );
    assert!(
        countries.iter().any(|c| c == &CountryCode::try_new(b"USA").unwrap()),
        "USA must appear in countries; got {countries:?}",
    );
    assert!(
        countries.iter().any(|c| c == &CountryCode::try_new(b"NATO").unwrap()),
        "NATO must appear in countries; got {countries:?}",
    );
}

#[test]
fn parse_fgi_marker_two_letter_eu_exception_yields_acknowledged() {
    // ODNI ISMCAT `CVEnumISMCATRelTo` ships `EU` as a registered
    // 2-letter exception code; pre-PR-2 admission accepted it via the
    // union TRIGRAPHS table and the new `admits_country_token`
    // surface preserves that. This test pins the EU-as-2-letter
    // contract so a future narrowing regression (e.g., back to
    // 3-or-4-only) is caught here.
    use marque_ism::CountryCode;
    let attrs = parse_banner_attrs("SECRET//FGI EU//NOFORN");
    let marker = attrs
        .fgi_marker
        .expect("FGI EU must admit per ISMCAT CVEnumISMCATRelTo");
    let countries = marker.countries();
    assert_eq!(countries.len(), 1, "FGI EU must produce single-country Acknowledged");
    assert_eq!(countries[0], CountryCode::try_new(b"EU").unwrap());
}

#[test]
fn parse_fgi_marker_5_letter_token_yields_no_marker() {
    // The shape gate accepts 2-4 ASCII upper bytes only; 5+-byte
    // codes (the `AUSTRALIA_GROUP`-class "exception is granted"
    // surface per CAPCO §H.7 p123) are out of scope at this gate.
    // `USAGB` fails because it's 5 bytes; the whole FGI marker
    // rejects per the FR-016 closure ("one bad token taints the
    // list"), not silent partial acceptance.
    let attrs = parse_banner_attrs("SECRET//FGI USAGB//NOFORN");
    assert!(
        attrs.fgi_marker.is_none(),
        "5-byte token must yield None at the country-token shape \
         gate (FR-016); got {:?}",
        attrs.fgi_marker,
    );
}

#[test]
fn parse_fgi_marker_digit_token_yields_no_marker() {
    // Digits in any list-token slot fail `admits_country_token`
    // (which requires uniform ASCII upper letters across 2/3/4
    // bytes). The pre-FR-016 surface would silently drop `U23` and
    // produce `Some(SourceConcealed)` once the country list went
    // empty; FR-016 + FR-017 close that channel.
    let attrs = parse_banner_attrs("SECRET//FGI U23//NOFORN");
    assert!(
        attrs.fgi_marker.is_none(),
        "digit-bearing list-token candidate must yield None \
         (FR-016); got {:?}",
        attrs.fgi_marker,
    );
}

#[test]
fn parse_fgi_marker_one_invalid_token_taints_whole_list() {
    // FR-016 contract: every token in the list must pass admission. One
    // shape-failing token rejects the whole marker — the parser must NOT
    // accept the valid prefix and silently drop the invalid suffix.
    //
    // `USA` is shape-admissible; `xyz` is lowercase and fails
    // `admits_country_token`. The pre-FR-016 surface would produce a
    // single-country `Acknowledged([USA])`; the post-FR-016 surface returns
    // `None`.
    let attrs = parse_banner_attrs("SECRET//FGI USA xyz//NOFORN");
    assert!(
        attrs.fgi_marker.is_none(),
        "one shape-failing token must reject the entire marker (FR-016); \
         got {:?}",
        attrs.fgi_marker,
    );
}

#[test]
fn parse_fgi_marker_lowercase_tetragraph_yields_no_marker() {
    // The country-token shape gate is uniform ASCII upper across
    // ALL admitted lengths — a 4-byte lowercase candidate (e.g.
    // `nato`) fails the same way a 3-byte lowercase candidate does.
    // This test pins the symmetric admission contract; if a future
    // edit accidentally case-folded only the trigraph branch, the
    // tetragraph branch must catch it.
    let attrs = parse_banner_attrs("SECRET//FGI USA nato//NOFORN");
    assert!(
        attrs.fgi_marker.is_none(),
        "lowercase tetragraph candidate must yield None at the \
         country-token shape gate (FR-016); got {:?}",
        attrs.fgi_marker,
    );
}

// =============================================================================
// SAR program identifier — `SarProgram::admits_program_id_abbrev` shape gate
// =============================================================================

#[test]
fn sar_program_id_too_short_yields_no_sar_marking() {
    // CAPCO-2016 §H.5 p101: "A program identifier abbreviation is the two
    // or three-character designator for the program." A length-1 program
    // identifier fails `admits_program_id_abbrev` at the parser site.
    //
    // Pre-FR-015 the inline `is_ascii_alphanumeric()` check would have
    // accepted `X` (length 1, all alnum) and produced a phantom one-char
    // SAR program. FR-015 routes this through the documented predicate,
    // which enforces the 2-3 length bound.
    let attrs = parse_portion_attrs("(TS//SAR-X)");
    assert!(
        attrs.sar_markings.is_none(),
        "length-1 SAR program identifier fails admits_program_id_abbrev \
         (CAPCO §H.5 p101); got {:?}",
        attrs.sar_markings,
    );
}

#[test]
fn sar_program_id_too_long_yields_no_sar_marking() {
    // §H.5 p101: 2–3 char abbreviation. A 4-char program identifier
    // (e.g. `BPCD`) fails the upper bound and must yield no SAR marking.
    // This is the symmetric closure of the length-1 case above; an inline
    // alnum check would silently accept it.
    let attrs = parse_portion_attrs("(TS//SAR-BPCD)");
    assert!(
        attrs.sar_markings.is_none(),
        "4-char SAR program identifier fails admits_program_id_abbrev \
         (CAPCO §H.5 p101); got {:?}",
        attrs.sar_markings,
    );
}

#[test]
fn sar_program_id_with_punctuation_yields_no_sar_marking() {
    // §H.5 p99: "SAR program identifiers are alphanumeric values."
    // A program identifier containing `.` (or any other non-alnum) fails
    // the character class. The first hyphen always marks the program /
    // compartment boundary (§H.5 p100), so the failing character has to be
    // something other than `-` — `.` is the canonical exemplar a
    // character-class regression would silently admit.
    let attrs = parse_portion_attrs("(TS//SAR-B.P)");
    assert!(
        attrs.sar_markings.is_none(),
        "SAR program identifier with `.` fails admits_program_id_abbrev \
         (CAPCO §H.5 p99 alphanumeric requirement); got {:?}",
        attrs.sar_markings,
    );
}

// =============================================================================
// SAR compartment identifier — `SarCompartment::admits_identifier` shape gate
// =============================================================================

#[test]
fn sar_compartment_with_punctuation_yields_no_sar_marking() {
    // §H.5 pp99–100 + Table 7: compartment identifiers are alphanumeric.
    // A `.` in the compartment slot fails `admits_identifier`. The parser
    // returns `None` from `parse_sar_program`, which propagates up through
    // `parse_sar_category` so `attrs.sar_markings` stays `None`.
    //
    // This guards the FR-015 migration of `crates/core/src/parser.rs:1481`
    // from inline `is_ascii_alphanumeric()` to the documented predicate.
    let attrs = parse_portion_attrs("(TS//SAR-BP-foo.bar)");
    assert!(
        attrs.sar_markings.is_none(),
        "SAR compartment with `.` fails admits_identifier (CAPCO §H.5 \
         pp99–100); got {:?}",
        attrs.sar_markings,
    );
}

#[test]
fn sar_compartment_empty_yields_no_sar_marking() {
    // `SAR-BP-` ends with an empty compartment segment. `admits_identifier`
    // rejects the empty byte string (`bytes.is_empty()` short-circuit), and
    // an inline `is_ascii_alphanumeric()` regression would also reject it
    // (vacuous-truth-on-empty-iterator is the trap here, but the parser
    // pre-checks `seg.is_empty()` and returns `None` before the alnum loop
    // runs). Pin both layers.
    let attrs = parse_portion_attrs("(TS//SAR-BP-)");
    assert!(
        attrs.sar_markings.is_none(),
        "SAR with empty trailing compartment segment yields None; got {:?}",
        attrs.sar_markings,
    );
}

// =============================================================================
// SAR sub-compartment identifier — `SarCompartment::admits_identifier`
// =============================================================================

#[test]
fn sar_sub_compartment_with_punctuation_yields_no_sar_marking() {
    // §H.5 pp99–100: same predicate gates compartments and sub-compartments
    // (alphanumeric values, no character-class distinction). A `.` in a
    // sub-compartment slot rejects the entire SAR program — the parser does
    // not retain the well-formed compartment prefix and discard the failing
    // sub-compartment.
    //
    // This guards the FR-015 migration of `crates/core/src/parser.rs:1493`.
    // `SAR-BP-CD foo.bar` parses as program `BP` with compartment `CD` and
    // sub-compartment `foo.bar`; the dot in the sub-compartment fails
    // admission.
    let attrs = parse_portion_attrs("(TS//SAR-BP-CD foo.bar)");
    assert!(
        attrs.sar_markings.is_none(),
        "SAR sub-compartment with `.` fails admits_identifier (CAPCO §H.5 \
         pp99–100); got {:?}",
        attrs.sar_markings,
    );
}

// =============================================================================
// Positive controls — admission gates accept the canonical lawful shapes
// =============================================================================
//
// These are NOT exhaustive happy-path coverage (that lives in the parser's
// in-module tests at `crates/core/src/parser.rs::tests`). The intent here is
// a thin sanity floor: if a future "fix" tightens admission so far that the
// canonical lawful forms also fail, these tests catch it. Their job is to
// confirm the negative tests above are testing the *gate*, not testing
// "all SAR / FGI parsing is broken".

#[test]
fn fgi_acknowledged_canonical_form_round_trips() {
    // §H.7 p123 lawful acknowledged form. If this fails, the negative
    // tests above are uninformative — they could be passing because
    // FGI parsing is broken end-to-end rather than because the admission
    // gate is doing its job.
    let attrs = parse_banner_attrs("SECRET//FGI USA GBR JPN//NOFORN");
    let marker = attrs.fgi_marker.expect("canonical FGI list parses");
    match marker {
        FgiMarker::Acknowledged { countries, .. } => {
            assert_eq!(countries.len(), 3);
            let names: Vec<&str> = countries.iter().map(|c| c.as_str()).collect();
            assert_eq!(names, ["USA", "GBR", "JPN"]);
        }
        FgiMarker::SourceConcealed => panic!("expected Acknowledged"),
    }
}

#[test]
fn sar_canonical_abbrev_form_round_trips() {
    // §H.5 p100 Table 7 example: `SAR-BP` is a 2-char program identifier.
    // If this fails, the SAR negative tests above are testing failure for
    // the wrong reason.
    let attrs = parse_portion_attrs("(TS//SAR-BP)");
    let sar = attrs.sar_markings.expect("canonical SAR-BP parses");
    assert_eq!(sar.programs.len(), 1);
    assert_eq!(sar.programs[0].identifier.as_ref(), "BP");
}
