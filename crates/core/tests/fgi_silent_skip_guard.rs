// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T096 — FGI / SAR silent-skip regression guard (FR-015 / FR-016 / SC-011).
//!
//! Closure of the four open-vocabulary parser admission sites migrated in
//! PR 2 of the engine-rule refactor (specs/006-engine-rule-refactor):
//!
//! 1. `parse_fgi_marker` → `CountryCode::admits_fgi_ownership_token` (#280)
//! 2. SAR program identifier (abbrev) → `SarProgram::admits_program_id_abbrev`
//! 3. SAR compartment identifier → `SarCompartment::admits_identifier`
//! 4. SAR sub-compartment identifier → `SarCompartment::admits_identifier`
//!
//! Item 1 site detail: post-#280 the FGI parser admits any 2- or
//! 3-byte ASCII-upper token OR the literal `NATO` tetragraph.
//! Distribution-list tetragraphs like `FVEY` / `CFIUS` / `ACGU` /
//! `ISAF` reject at this gate because they don't carry ownership
//! semantic per §H.7 p122. The 2- and 3-byte branches are shape-
//! only — unregistered uppercase tokens admit at the parser;
//! registry validation is the rule layer's job (S004, E008) per
//! the project's parser/rule split. EU motivates the 2-byte
//! admission branch (its own classification system per Council
//! Decision 2013/488/EU; registered in ISMCAT CVEnumISMCATRelTo).
//! Pre-#280 this site routed through the broader
//! `admits_country_token` predicate (which also admitted 4-byte
//! distribution-list tetragraphs).
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
//! production) and inspect `CanonicalAttrs::fgi_marker` / `CanonicalAttrs::sar_markings`.
//!
//! A regression that silently re-introduces shape-failure-as-`Some` would
//! produce the wrong observable on these inputs and trip every relevant test.
//!
//! # Authority
//!
//! - CAPCO-2016 §H.7 p122 — FGI banner forms (concealed `FGI`, acknowledged
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
use marque_core::parser::ParsedMarking;
use marque_ism::CanonicalAttrs;
use marque_ism::attrs::FgiMarker;
use marque_ism::span::{MarkingCandidate, MarkingType};
use marque_ism::token_set::CapcoTokenSet;
use marque_scheme::Span;

/// Drive `Parser::parse` over `text` interpreted as a banner candidate and
/// return the resulting `CanonicalAttrs`. Mirrors the engine's banner-path
/// dispatch (`marque-engine` constructs banner candidates the same way for
/// any byte slice the scanner identified as a banner).
///
/// Returns `None` if the parser refused the candidate outright (e.g., invalid
/// UTF-8 — not in scope for these tests). The relevant assertion shape for
/// shape-admission failures is `attrs.fgi_marker.is_none()` /
/// `attrs.sar_markings.is_none()`, NOT a hard parse error: the parser is
/// lenient on individual blocks and surfaces shape failures as missing
/// attribute fields plus `Unknown` token spans.
fn parse_banner_attrs(text: &str) -> CanonicalAttrs {
    let source = text.as_bytes();
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidate = MarkingCandidate {
        span: Span::new(0, source.len()),
        kind: MarkingType::Banner,
    };
    let parsed = parser
        .parse(&candidate, source)
        .expect("banner candidate parses (lenient parser; shape failures surface as None fields)");
    // Test-fixture carve-out per Constitution V Principle V — the
    // structural rename lives inline (via `parsed_marking_to_canonical`
    // below) because `marque-core` cannot dev-depend on `marque-capco`
    // (Constitution VII), so the trait route
    // `CapcoScheme::canonicalize` is unreachable from here. The helper
    // mirrors the override's field mapping and emits the same
    // `CanonicalAttrs` output for every input, plus the §H.7 p41 /
    // PR 9b T132 debug-assert that the override carries. The helper
    // takes `ParsedMarking` (not `ParsedAttrs`) so FR-040 PRC100's
    // `(ParsedAttrs) -> CanonicalAttrs` signature shape never appears
    // in test code; the lint's sole-path invariant is unweakened.
    parsed_marking_to_canonical(parsed)
}

/// Drive `Parser::parse` over `text` interpreted as a portion candidate.
///
/// `text` MUST include the outer parentheses — the parser strips them and
/// rejects un-parenthesized portion text outright (which is the wrong
/// surface for these admission tests).
fn parse_portion_attrs(text: &str) -> CanonicalAttrs {
    let source = text.as_bytes();
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidate = MarkingCandidate {
        span: Span::new(0, source.len()),
        kind: MarkingType::Portion,
    };
    let parsed = parser
        .parse(&candidate, source)
        .expect("portion candidate parses (lenient parser; shape failures surface as None fields)");
    // See `parse_banner_attrs` above for the Constitution V / VII
    // carve-out rationale.
    parsed_marking_to_canonical(parsed)
}

/// Structural rename — `ParsedMarking<'_>::attrs` → `CanonicalAttrs`.
///
/// Mirrors the `CapcoScheme::canonicalize` override's field mapping
/// and output semantics, including the §H.7 p41 / PR 9b T132 debug-
/// assert that no `ParsedAttrs` reaches canonicalization with both
/// `dissem_nato` populated AND a US classification axis (which would
/// mean `attribute_dissems` was skipped). The local helper's
/// control flow and locals are not a literal copy of the override —
/// it returns the assembled `CanonicalAttrs` directly rather than
/// binding to a `let out = ...;` first — but the input/output
/// relationship is identical.
///
/// Lives in `marque-core/tests/` because Constitution VII forbids
/// `marque-core ←── marque-capco` (the trait route would need that
/// dev-dep edge). The helper takes `ParsedMarking` (parser output
/// wrapper) rather than `ParsedAttrs` directly so the FR-040 PRC100
/// signature shape `(ParsedAttrs) -> CanonicalAttrs` does not appear
/// in test code — keeping the sole-path lint at full strength while
/// honoring the Constitution V Principle V test-fixture carve-out.
///
/// Lifted from `marque_ism::from_parsed_unchecked` in PR 3c.2.E.
#[allow(clippy::needless_pass_by_value)]
fn parsed_marking_to_canonical(parsed: ParsedMarking<'_>) -> CanonicalAttrs {
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
    } = parsed.attrs;
    let out = CanonicalAttrs {
        classification: classification.map(|c| c.value),
        sci_controls,
        sci_markings: Vec::from(sci_markings)
            .into_iter()
            .map(|p| p.value)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        sar_markings: sar_markings.map(|p| p.value),
        aea_markings: Vec::from(aea_markings)
            .into_iter()
            .map(|p| p.value)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        fgi_marker: fgi_marker.map(|p| p.value),
        dissem_us: Vec::from(dissem_us)
            .into_iter()
            .map(|p| p.value)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        dissem_nato: Vec::from(dissem_nato)
            .into_iter()
            .map(|p| p.value)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        non_ic_dissem: Vec::from(non_ic_dissem)
            .into_iter()
            .map(|p| p.value)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        rel_to: Vec::from(rel_to)
            .into_iter()
            .map(|p| p.value)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        display_only_to: Vec::from(display_only_to)
            .into_iter()
            .map(|p| p.value)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        declassify_on: declassify_on.map(|p| p.value),
        classified_by: classified_by.map(Box::<str>::from),
        derived_from: derived_from.map(Box::<str>::from),
        declass_exemption,
        token_spans,
    };

    // Mirror the PR 9b (T132) invariant guard carried by
    // `CapcoScheme::canonicalize`. `attribute_dissems` is the single
    // source of truth; this debug-only assertion catches a future
    // bug where attribution is skipped or a hand-built `ParsedAttrs`
    // is fed in with both fields populated.
    #[cfg(debug_assertions)]
    {
        debug_assert!(
            out.dissem_nato.is_empty() || out.us_classification().is_none(),
            "dissem_nato populated alongside US classification — \
             attribute_dissems was skipped or bypassed. CAPCO-2016 p41 \
             reciprocity rule violated."
        );
    }

    out
}

// =============================================================================
// FGI marker — `parse_fgi_marker` four-case enforcement (FR-016 / FR-017)
// =============================================================================

#[test]
fn parse_fgi_marker_bare_fgi_yields_source_concealed() {
    // CAPCO-2016 §H.7 p122: bare `FGI` is the lawful source-concealed banner
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
        "bare FGI must be SourceConcealed (CAPCO §H.7 p122), got {marker:?}",
    );
    // FR-017 invariant: the lawful concealed form has no countries.
    assert!(
        marker.countries().is_empty(),
        "SourceConcealed has no countries by definition",
    );
}

#[test]
fn parse_fgi_marker_acknowledged_yields_countries() {
    // CAPCO-2016 §H.7 p122: `FGI [LIST]` is the acknowledged form. The
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
    // CAPCO §H.7 p122 + §A.6 p16 require trigraph or tetragraph country
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
    // CAPCO-2016 §H.7 p122 spells out the canonical example:
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
        .expect("mixed trigraph + tetragraph FGI list must admit per §H.7 p122");
    let countries = marker.countries();
    assert_eq!(
        countries.len(),
        2,
        "FGI USA NATO must produce two-country Acknowledged; got {countries:?}",
    );
    assert!(
        countries
            .iter()
            .any(|c| c == &CountryCode::try_new(b"USA").unwrap()),
        "USA must appear in countries; got {countries:?}",
    );
    assert!(
        countries
            .iter()
            .any(|c| c == &CountryCode::try_new(b"NATO").unwrap()),
        "NATO must appear in countries; got {countries:?}",
    );
}

#[test]
fn parse_fgi_marker_two_letter_eu_exception_yields_acknowledged() {
    // Issue #280 (widening amendment): the EU 2-letter exception
    // code admits at the FGI ownership gate. EU has its own
    // classification system (EU CONFIDENTIAL / EU SECRET / EU TOP
    // SECRET, per Council Decision 2013/488/EU and successors) used
    // by EU institutions and member states; registered in ODNI
    // ISMCAT `CVEnumISMCATRelTo`. EU motivates the 2-byte admission
    // branch.
    //
    // The 2-byte branch is shape-only (admits any uppercase 2-byte
    // token, not just EU) — see
    // `parse_fgi_marker_two_letter_unregistered_admits_shape_only_280_496`
    // for the shape-only contract pin.
    use marque_ism::CountryCode;
    let attrs = parse_banner_attrs("SECRET//FGI EU//NOFORN");
    let marker = attrs
        .fgi_marker
        .expect("FGI EU must admit at the FGI ownership gate (#280)");
    let countries = marker.countries();
    assert_eq!(
        countries.len(),
        1,
        "FGI EU must produce single-country Acknowledged"
    );
    assert_eq!(countries[0], CountryCode::try_new(b"EU").unwrap());
}

#[test]
fn parse_fgi_marker_two_letter_lowercase_rejects() {
    // Negative control for the EU widening (#280 amendment): the
    // FGI ownership predicate routes 2-byte tokens through
    // `admits_country_token`, which enforces uniform ASCII upper.
    // Lowercase `eu` exercises that the length-2 branch is shape-
    // gated (uniform ASCII upper), not a "any 2-byte sequence"
    // wildcard. If a future regression strips the
    // `admits_country_token` call, this test catches it. Note:
    // unregistered uppercase 2-byte tokens (e.g., `XX`) DO admit —
    // see the shape-only contract pin below.
    let attrs = parse_banner_attrs("SECRET//FGI eu//NOFORN");
    assert!(
        attrs.fgi_marker.is_none(),
        "lowercase 2-byte token must reject at the FGI ownership \
         gate via admits_country_token's uniform-upper rule; got {:?}",
        attrs.fgi_marker,
    );
}

#[test]
fn parse_fgi_marker_two_letter_digit_bearing_rejects() {
    // Negative control mirroring the lowercase case: a 2-byte
    // token with a digit (`E1`) fails `admits_country_token`'s
    // uniform-upper rule. Pins that the length-2 admission is
    // shape-gated on character class (uniform ASCII upper), not on
    // registry membership.
    let attrs = parse_banner_attrs("SECRET//FGI E1//NOFORN");
    assert!(
        attrs.fgi_marker.is_none(),
        "digit-bearing 2-byte token must reject at the FGI ownership \
         gate via admits_country_token's uniform-upper rule; got {:?}",
        attrs.fgi_marker,
    );
}

#[test]
fn parse_fgi_marker_two_letter_unregistered_admits_shape_only_280_496() {
    // Shape-only admission per #494 design decision: `XX` admits
    // at the FGI ownership gate even though it is not a registered
    // CountryCode. Downstream rules (S004 trigraph-suggest, E008
    // unknown-token) carry the registry-validation responsibility
    // per the project's parser/rule split. This matches the
    // established convention (see project memory
    // `project_long_form_trigraphs`).
    //
    // Driven through the banner form (`SECRET//FGI XX//NOFORN`)
    // because that's the surface `parse_fgi_marker` gates; the
    // portion form `(//FGI XX)` reaches a different parse path
    // (`parse_fgi_classification`, which expects `<country>
    // <level>` shape, not `FGI <country>`).
    //
    // TODO(#496): once decoder FGI-context investigation lands,
    // may need a coordinated update if the decoder grows registry-
    // aware matching for unregistered uppercase tokens in FGI
    // ownership context. The parser-side shape-only contract here
    // is independent of and composable with that future decoder
    // behavior.
    let attrs = parse_banner_attrs("SECRET//FGI XX//NOFORN");
    let marker = attrs.fgi_marker.expect("XX admits at the shape-only gate");
    match marker {
        FgiMarker::Acknowledged { countries, .. } => {
            assert_eq!(countries.len(), 1, "single token admitted");
            assert_eq!(
                countries[0].as_str(),
                "XX",
                "the unregistered token round-trips verbatim",
            );
        }
        FgiMarker::SourceConcealed => {
            panic!("XX should land in Acknowledged, not SourceConcealed")
        }
    }
}

#[test]
fn parse_fgi_marker_three_letter_unregistered_admits_shape_only_280_496() {
    // Shape-only contract for length-3 unregistered uppercase
    // tokens. `ZZZ` is not a sovereign trigraph; admits at the
    // FGI ownership gate. Same architectural rationale as the
    // 2-byte case above — downstream rules (S004 / E008) catch the
    // registry miss with actionable diagnostics, which is better UX
    // than silent parser-level rejection. See
    // `project_long_form_trigraphs` memory; see #496 for the
    // decoder-side follow-up.
    //
    // Driven through the banner form for the same reason as the
    // 2-byte test — `parse_fgi_marker` is the FGI ownership gate;
    // portion `(//FGI ZZZ)` reaches `parse_fgi_classification`,
    // which is a different surface.
    let attrs = parse_banner_attrs("SECRET//FGI ZZZ//NOFORN");
    let marker = attrs.fgi_marker.expect("ZZZ admits at the shape-only gate");
    match marker {
        FgiMarker::Acknowledged { countries, .. } => {
            assert_eq!(countries.len(), 1, "single token admitted");
            assert_eq!(
                countries[0].as_str(),
                "ZZZ",
                "the unregistered token round-trips verbatim",
            );
        }
        FgiMarker::SourceConcealed => {
            panic!("ZZZ should land in Acknowledged, not SourceConcealed")
        }
    }
}

#[test]
fn parse_fgi_marker_5_letter_token_yields_no_marker() {
    // The shape gate accepts 2-4 ASCII upper bytes only; 5+-byte
    // codes (the `AUSTRALIA_GROUP`-class "exception is granted"
    // surface per CAPCO §H.7 p122) are out of scope at this gate.
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
    // §H.7 p122 lawful acknowledged form. If this fails, the negative
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
    assert_eq!(sar.programs[0].identifier.as_str(), "BP");
}

// =============================================================================
// Issue #280 — SAR open-vocab case tightening (lowercase / mixed-case reject)
// =============================================================================
//
// SAR has no CVE registry (`CVEnumISMSAR.xml` intentionally empty per ODNI
// policy). With no registry to validate against, the shape gate IS the
// validation. Per CAPCO-2016 §A.6 p15 + §G.1 p36, all banner-line and
// portion-mark Register entries are uppercase, so SAR identifiers must
// conform. Pre-#280 the shape predicates used `is_ascii_alphanumeric()`,
// silently admitting lowercase. The lenient-admit path bypassed the
// `DecoderRecognizer` that handles demangling: once strict parse "succeeds"
// the decoder fallback (R001) does not run, so no diagnostic ever fires on
// the case error. The fix tightens both `SarProgram::admits_program_id_abbrev`
// and `SarCompartment::admits_identifier` to uppercase-or-digit; lowercase
// inputs now fail strict parse and route to the decoder.
//
// TODO(#493): These tests verify strict-parse rejection (parser returns
// None) but not the engine-level decoder dispatch that produces R001 with
// the canonical fix. Cross-crate test can't live here (no access to
// marque-engine). Follow-up #493 tracks the engine-level integration tests
// pinning the decoder-dispatch contract for both SAR and FGI rejection
// paths.

#[test]
fn sar_program_id_lowercase_rejects_issue_280() {
    // Pre-#280: `SAR-fk` produced `SarProgram { id: "fk" }` silently.
    // Post-#280: parser rejects; decoder fallback handles demangling.
    let attrs = parse_portion_attrs("(TS//SAR-fk)");
    assert!(
        attrs.sar_markings.is_none(),
        "lowercase SAR program id must reject at the strict gate \
         (CAPCO §A.6 p15 + §G.1 p36, issue #280); got {:?}",
        attrs.sar_markings,
    );
}

#[test]
fn sar_program_id_mixed_case_rejects_issue_280() {
    // Pre-#280: `SAR-Fk` produced `SarProgram { id: "Fk" }` silently.
    // Mixed case is the most subtle leak — looks "almost right" but
    // still fails Register uppercase rule.
    let attrs = parse_portion_attrs("(TS//SAR-Fk)");
    assert!(
        attrs.sar_markings.is_none(),
        "mixed-case SAR program id must reject at the strict gate \
         (CAPCO §A.6 p15 + §G.1 p36, issue #280); got {:?}",
        attrs.sar_markings,
    );
}

#[test]
fn sar_compartment_lowercase_rejects_issue_280() {
    // Pre-#280: `SAR-FK-blue42` produced `SarCompartment { id: "blue42" }`
    // silently. The compartment identifier predicate carries the same
    // Register-uppercase rule as the program identifier.
    let attrs = parse_portion_attrs("(TS//SAR-FK-blue42)");
    assert!(
        attrs.sar_markings.is_none(),
        "lowercase SAR compartment must reject at the strict gate \
         (CAPCO §A.6 p15 + §G.1 p36, issue #280); got {:?}",
        attrs.sar_markings,
    );
}

#[test]
fn sar_sub_compartment_lowercase_rejects_issue_280() {
    // Pre-#280: `SAR-FK-BLUE 42a` produced
    // `SarSubCompartment { id: "42a" }` silently. The sub-compartment
    // slot uses the same predicate as the compartment slot per
    // CAPCO-2016 §H.5 pp99-100.
    let attrs = parse_portion_attrs("(TS//SAR-FK-BLUE 42a)");
    assert!(
        attrs.sar_markings.is_none(),
        "lowercase SAR sub-compartment must reject at the strict gate \
         (CAPCO §A.6 p15 + §G.1 p36, issue #280); got {:?}",
        attrs.sar_markings,
    );
}

#[test]
fn sar_canonical_compartment_form_round_trips() {
    // Positive control for the #280 negative tests. `SAR-FK-BLUE` is
    // a §H.5 p100 Table 7-shape canonical example: 2-char program +
    // 4-char compartment, all uppercase. If this fails, the negative
    // tests are testing failure for the wrong reason.
    let attrs = parse_portion_attrs("(TS//SAR-FK-BLUE)");
    let sar = attrs.sar_markings.expect("canonical SAR-FK-BLUE parses");
    assert_eq!(sar.programs.len(), 1);
    assert_eq!(sar.programs[0].identifier.as_str(), "FK");
    assert_eq!(sar.programs[0].compartments.len(), 1);
    assert_eq!(sar.programs[0].compartments[0].identifier.as_str(), "BLUE",);
}

#[test]
fn sar_canonical_sub_compartment_form_round_trips() {
    // Positive control: §H.5 p100 Table 7-shape canonical example with
    // a sub-compartment. `SAR-FK-BLUE 42` has program `FK`, compartment
    // `BLUE`, sub-compartment `42`. Pins the all-uppercase round-trip.
    let attrs = parse_portion_attrs("(TS//SAR-FK-BLUE 42)");
    let sar = attrs.sar_markings.expect("canonical SAR-FK-BLUE 42 parses");
    assert_eq!(sar.programs.len(), 1);
    assert_eq!(sar.programs[0].identifier.as_str(), "FK");
    let comp = &sar.programs[0].compartments[0];
    assert_eq!(comp.identifier.as_str(), "BLUE");
    assert_eq!(comp.sub_compartments.len(), 1);
    assert_eq!(&*comp.sub_compartments[0], "42");
}

// =============================================================================
// Issue #280 — FGI ownership-token narrowing (NATO admits; FVEY/CFIUS etc. reject)
// =============================================================================
//
// Pre-#280: `parse_fgi_marker` accepted any 2-4 char uppercase token via
// `CountryCode::admits_country_token` (a REL TO list-token shape). FGI is
// fundamentally an OWNERSHIP marking per CAPCO-2016 §H.7 p122 ("Foreign
// Government Information" — information that an entity originated). NATO is
// the only alliance tetragraph CAPCO treats as an ownership identifier in
// this slot; distribution-list tetragraphs (`FVEY`, `CFIUS`, `ACGU`, `ISAF`)
// describe who may receive a marking, not who owns it. The fix narrows the
// FGI parser-site predicate to `CountryCode::admits_fgi_ownership_token`
// (3-byte trigraph OR literal `NATO`). REL TO list slots continue to admit
// the broader surface.

#[test]
fn fgi_ownership_nato_tetragraph_admits_issue_280() {
    // Positive control: `NATO` is the named ownership tetragraph per
    // §H.7 p122 canonical example. The narrower predicate keeps it.
    use marque_ism::CountryCode;
    let attrs = parse_banner_attrs("SECRET//FGI NATO//NOFORN");
    let marker = attrs
        .fgi_marker
        .expect("FGI NATO must admit per §H.7 p122 (#280)");
    match marker {
        FgiMarker::Acknowledged { countries, .. } => {
            assert_eq!(countries.len(), 1);
            assert_eq!(countries[0], CountryCode::try_new(b"NATO").unwrap());
        }
        FgiMarker::SourceConcealed => {
            panic!("expected Acknowledged([NATO]); got SourceConcealed")
        }
    }
}

#[test]
fn fgi_ownership_distribution_list_tetragraph_rejects_issue_280() {
    // `FVEY` is a Five Eyes distribution-list tetragraph — lawful in
    // REL TO slots but not an ownership identifier per §H.7. The
    // narrowed parser predicate rejects it; decoder handles the
    // routing-to-REL-TO suggestion.
    let attrs = parse_banner_attrs("SECRET//FGI FVEY//NOFORN");
    assert!(
        attrs.fgi_marker.is_none(),
        "FVEY (distribution-list tetragraph) must reject at FGI \
         ownership gate (#280); got {:?}",
        attrs.fgi_marker,
    );
}

#[test]
fn fgi_ownership_unregistered_non_nato_tetragraph_rejects_issue_280() {
    // `DEUX` is a 4-char non-`NATO` candidate. The narrowed predicate
    // pins the rule to "3-byte trigraph OR literal NATO"; any other
    // 4-byte sequence rejects irrespective of registry membership.
    let attrs = parse_banner_attrs("SECRET//FGI DEUX//NOFORN");
    assert!(
        attrs.fgi_marker.is_none(),
        "non-NATO 4-char tetragraph must reject at FGI ownership \
         gate (#280); got {:?}",
        attrs.fgi_marker,
    );
}

#[test]
fn fgi_ownership_arbitrary_non_nato_tetragraph_rejects_issue_280() {
    // `BLAH` — same as `DEUX` above but a distinct example pinning
    // the predicate is "rule-based on shape + NATO literal", not a
    // narrow CVE allow-list.
    let attrs = parse_banner_attrs("SECRET//FGI BLAH//NOFORN");
    assert!(
        attrs.fgi_marker.is_none(),
        "non-NATO 4-char tetragraph must reject at FGI ownership \
         gate (#280); got {:?}",
        attrs.fgi_marker,
    );
}
