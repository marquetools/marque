// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Layer 2 parse-render round-trip property test.
//!
//! Pins the contract: for every well-formed marking `s` in the strict-path
//! corpus,
//!
//! ```text
//!     parse(s) -> attrs1
//!     render(attrs1) -> s'
//!     parse(s') -> attrs2
//!     attrs2 ≡ attrs1 (modulo renderer coverage)
//! ```
//!
//! is idempotent at the AST level. The exact "modulo renderer coverage"
//! qualifier is the load-bearing detail — when the `MarkingScheme::render_*`
//! impl on `CapcoScheme` emits only the classification level (`"SECRET"` for
//! banners, `"S"` for portions), this file pins the *narrowest defensible*
//! round-trip — the classification axis the renderer covers — and gates the
//! full attr-surface round-trip behind `#[ignore]`.
//!
//! Pinning the narrow round-trip catches a real regression class: if a
//! future change breaks `Classification` parsing
//! or breaks the `effective_level → banner_str / portion_str → re-parse`
//! cycle, this test fires immediately. The full-attribute round-trip is
//! useful as a guardrail; the classification round-trip is useful as an
//! immediate alarm.
//!
//! # Authority
//!
//! - CAPCO-2016 §A.6 p15 — banner + portion grammar.
//! - CAPCO-2016 §H.7 p122 — FGI banner/portion forms (lawful concealed +
//!   acknowledged variants).
//!
//! The full round-trip is gated on the full
//! `MarkingScheme::render_canonical` impl landing.

use marque_capco::scheme::{CapcoMarking, CapcoScheme};
use marque_core::{Parser, Scanner};
use marque_ism::span::{MarkingCandidate, MarkingType};
use marque_ism::token_set::CapcoTokenSet;
use marque_ism::{CanonicalAttrs, MarkingClassification};
use marque_scheme::{MarkingScheme, Span};
use marque_test_utils::{load_fixture, valid_fixtures};
use std::path::Path;

// =============================================================================
// Parse helpers — drive `marque_core::Parser` on a typed banner / portion
// candidate and return the produced `CanonicalAttrs`. Mirrors the
// engine's per-candidate dispatch without pulling in `marque-engine` (a
// dev-dep cycle would result for the PR-2 scope).
// =============================================================================

fn parse_with_kind(
    scheme: &CapcoScheme,
    source: &[u8],
    kind: MarkingType,
) -> Option<CanonicalAttrs> {
    // The helper takes `&CapcoScheme` as its first parameter so callers
    // that already construct a scheme for rendering / lattice work can
    // reuse it.
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidate = MarkingCandidate {
        span: Span::new(0, source.len()),
        kind,
    };
    parser
        .parse(&candidate, source)
        .ok()
        .map(|p| scheme.canonicalize(p.attrs))
}

/// Parse a banner string; panics on parser failure (the strict-path corpus
/// is, by construction, parseable).
fn parse_banner(scheme: &CapcoScheme, text: &str) -> CanonicalAttrs {
    parse_with_kind(scheme, text.as_bytes(), MarkingType::Banner)
        .expect("banner candidate from valid corpus must parse")
}

/// Parse a portion string; panics on parser failure. The text must include
/// outer parens — the parser strips them and rejects un-parenthesized
/// portion text outright.
fn parse_portion(scheme: &CapcoScheme, text: &str) -> CanonicalAttrs {
    parse_with_kind(scheme, text.as_bytes(), MarkingType::Portion)
        .expect("portion candidate from valid corpus must parse")
}

// =============================================================================
// Fixture classification — every `tests/corpus/capco/valid/*.txt` file is a
// single well-formed marking on the first non-blank line. The Scanner is
// the source of truth for whether a string is a portion / banner / CAB
// (it is the same component the engine uses); we drive it once per fixture
// to pick the right re-parse kind for the round-trip.
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Portion,
    Banner,
    /// Skipped at the round-trip step. The current renderer does not emit
    /// CAB blocks. Counted in the test summary so a corpus expansion is
    /// visible.
    Cab,
    /// Multiple markings on the line (e.g., a portion glued to prose). Not
    /// in the round-trip set; the per-fixture corpus covers single-marking
    /// inputs only.
    Other,
}

fn detect_kind(source: &[u8]) -> Kind {
    let candidates = Scanner::scan(source);
    // Skip engine-internal boundary candidates (PageBreak +
    // PageFinalization — the scanner only emits PageBreak today,
    // but PageFinalization (issue #461) is engine-synthesized and
    // could never appear in the scanner-emitted candidate stream
    // here anyway; included for forward-compatibility against the
    // `#[non_exhaustive]` enum).
    let real: Vec<_> = candidates
        .iter()
        .filter(|c| {
            !matches!(
                c.kind,
                MarkingType::PageBreak | MarkingType::PageFinalization
            )
        })
        .collect();
    if real.len() != 1 {
        return Kind::Other;
    }
    match real[0].kind {
        MarkingType::Portion => Kind::Portion,
        MarkingType::Banner => Kind::Banner,
        MarkingType::Cab => Kind::Cab,
        MarkingType::PageBreak | MarkingType::PageFinalization => Kind::Other,
        // `MarkingType` is `#[non_exhaustive]` (issue #461). Any
        // future variant the scanner emits is not a known
        // round-trip surface for this test and falls to `Other`.
        _ => Kind::Other,
    }
}

/// Strip trailing whitespace + the trailing newline that every corpus
/// fixture has (the corpus convention is one marking per file with a
/// trailing newline).
fn fixture_text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec())
        .expect("corpus fixtures are UTF-8")
        .trim_end()
        .to_owned()
}

// =============================================================================
// Classification round-trip (current renderer coverage)
//
// The `MarkingScheme::render_item` / `render_summary` impl on `CapcoScheme`
// today emits the classification level only. The narrowest round-trip the
// renderer can satisfy is on `Classification::effective_level()`.
//
// This pins the contract on every banner / portion fixture in the strict-
// path corpus. Failure means either:
//   (a) the renderer broke for a `Classification` variant, or
//   (b) `Classification::banner_str()` / `portion_str()` produces a string
//       that the parser cannot round-trip back to the same level.
//
// Both are real regression classes that this test catches deterministically.
// =============================================================================

fn render_and_reparse_classification(
    scheme: &CapcoScheme,
    fixture: &Path,
    text: &str,
    kind: Kind,
    attrs1: &CanonicalAttrs,
) {
    let marking1 = CapcoMarking::from(attrs1.clone());

    let (rendered, attrs2) = match kind {
        Kind::Portion => {
            // Renderer emits `"S"`; re-parse as a portion needs the outer
            // parens. Wrapping is the inverse of the parser's
            // `strip_prefix('(') / strip_suffix(')')` step — pinning it
            // explicitly here keeps the round-trip end-to-end through the
            // public surface.
            let inner = scheme.render_item(&marking1);
            // The renderer returns an empty string when classification
            // is `None`; the parser would also accept `()` and produce
            // empty attrs, which round-trips trivially. Skip to avoid
            // exercising the empty-classification edge case as if it
            // were the contract under test.
            if inner.is_empty() {
                return;
            }
            let rendered = format!("({inner})");
            let attrs2 = parse_portion(scheme, &rendered);
            (rendered, attrs2)
        }
        Kind::Banner => {
            let rendered = scheme.render_summary(&marking1);
            if rendered.is_empty() {
                return;
            }
            let attrs2 = parse_banner(scheme, &rendered);
            (rendered, attrs2)
        }
        Kind::Cab | Kind::Other => return,
    };

    // The classification axis is the current renderer's coverage. We
    // compare `effective_level()` rather than the full
    // `MarkingClassification` because the renderer drops the originating
    // system (US / NATO / FGI / JOINT) and emits the level only.
    // `effective_level()` collapses every system to its `Classification`
    // rung, which is exactly the data the renderer-then-reparser
    // pipeline preserves.
    let level1 = attrs1
        .classification
        .as_ref()
        .map(MarkingClassification::effective_level);
    let level2 = attrs2
        .classification
        .as_ref()
        .map(MarkingClassification::effective_level);

    assert_eq!(
        level1,
        level2,
        "classification round-trip drift on fixture {} (input {text:?} → \
         rendered {rendered:?}): attrs1 level={level1:?} attrs2 \
         level={level2:?}. \
         Either the renderer or `Classification::{{banner,portion}}_str` \
         no longer round-trips.",
        fixture.display(),
    );
}

#[test]
fn classification_round_trips_across_strict_corpus() {
    // Drive every fixture under `tests/corpus/capco/valid/` through the
    // narrow round-trip. The strict-path corpus is the load-bearing
    // input set; classification round-trip is the parse-render-parse
    // idempotence closure scoped to the renderer's current coverage.
    let scheme = CapcoScheme::new();
    let fixtures = valid_fixtures();
    assert!(
        !fixtures.is_empty(),
        "valid corpus is empty; check tests/corpus/capco/valid/ scaffold",
    );

    let mut counts = [0usize; 4]; // [portion, banner, cab_skipped, other_skipped]
    for path in &fixtures {
        let bytes = load_fixture(path);
        let text = fixture_text(&bytes);
        let kind = detect_kind(text.as_bytes());

        match kind {
            Kind::Portion => {
                let attrs = parse_portion(&scheme, &text);
                render_and_reparse_classification(&scheme, path, &text, kind, &attrs);
                counts[0] += 1;
            }
            Kind::Banner => {
                let attrs = parse_banner(&scheme, &text);
                render_and_reparse_classification(&scheme, path, &text, kind, &attrs);
                counts[1] += 1;
            }
            Kind::Cab => counts[2] += 1,
            Kind::Other => counts[3] += 1,
        }
    }

    // Defense in depth: if a corpus refactor accidentally drops every
    // banner or every portion fixture, the round-trip would silently
    // pass with zero work. Require at least one of each so the gate
    // stays meaningful.
    assert!(
        counts[0] > 0,
        "no portion fixtures exercised — corpus regression likely",
    );
    assert!(
        counts[1] > 0,
        "no banner fixtures exercised — corpus regression likely",
    );
}

// =============================================================================
// Targeted round-trips at the FGI/SAR closure. These are the cases the
// pinning is *for*: bare FGI, FGI with a single trigraph, FGI with
// multiple trigraphs, and SAR program-only / program-with-compartment
// forms. They are synthetic inputs, not corpus-driven; the corpus-wide
// test above provides breadth, these provide depth at the load-bearing
// FGI/SAR closure surface.
//
// Each test verifies that re-parsing the rendered classification produces
// the same level — the narrowest invariant the current renderer can
// guarantee.
// =============================================================================

#[test]
fn fr016_bare_fgi_classification_round_trips() {
    // CAPCO-2016 §H.7 p122 lawful concealed form. The FGI closure pins
    // this to `Some(SourceConcealed)` rather than the degraded
    // `Some(FgiMarker { countries: [] })`. The classification axis
    // ("SECRET") round-trips through the renderer.
    let scheme = CapcoScheme::new();
    let attrs1 = parse_banner(&scheme, "SECRET//FGI//NOFORN");
    let rendered = scheme.render_summary(&CapcoMarking::from(attrs1.clone()));
    let attrs2 = parse_banner(&scheme, &rendered);
    assert_eq!(
        attrs1
            .classification
            .as_ref()
            .map(MarkingClassification::effective_level),
        attrs2
            .classification
            .as_ref()
            .map(MarkingClassification::effective_level),
        "bare FGI banner classification round-trip drift; rendered {rendered:?}",
    );
}

#[test]
fn fr017_acknowledged_fgi_single_country_classification_round_trips() {
    // §H.7 p122 lawful acknowledged form. Single-country list.
    let scheme = CapcoScheme::new();
    let attrs1 = parse_banner(&scheme, "SECRET//FGI DEU//NOFORN");
    let rendered = scheme.render_summary(&CapcoMarking::from(attrs1.clone()));
    let attrs2 = parse_banner(&scheme, &rendered);
    assert_eq!(
        attrs1
            .classification
            .as_ref()
            .map(MarkingClassification::effective_level),
        attrs2
            .classification
            .as_ref()
            .map(MarkingClassification::effective_level),
        "FGI DEU banner classification round-trip drift; rendered {rendered:?}",
    );
}

#[test]
fn fr017_acknowledged_fgi_multi_country_classification_round_trips() {
    // §H.7 p122 + §A.6 p16 multi-trigraph list (sorted).
    let scheme = CapcoScheme::new();
    let attrs1 = parse_banner(&scheme, "SECRET//FGI USA GBR JPN//NOFORN");
    let rendered = scheme.render_summary(&CapcoMarking::from(attrs1.clone()));
    let attrs2 = parse_banner(&scheme, &rendered);
    assert_eq!(
        attrs1
            .classification
            .as_ref()
            .map(MarkingClassification::effective_level),
        attrs2
            .classification
            .as_ref()
            .map(MarkingClassification::effective_level),
        "FGI USA GBR JPN banner classification round-trip drift; \
         rendered {rendered:?}",
    );
}

#[test]
fn fr015_sar_program_only_classification_round_trips() {
    // §H.5 p101 lawful abbreviation form (program identifier, no
    // compartment). Portion side.
    let scheme = CapcoScheme::new();
    let attrs1 = parse_portion(&scheme, "(TS//SAR-BP)");
    let rendered = scheme.render_item(&CapcoMarking::from(attrs1.clone()));
    let attrs2 = parse_portion(&scheme, &format!("({rendered})"));
    assert_eq!(
        attrs1
            .classification
            .as_ref()
            .map(MarkingClassification::effective_level),
        attrs2
            .classification
            .as_ref()
            .map(MarkingClassification::effective_level),
        "SAR-BP portion classification round-trip drift; rendered {rendered:?}",
    );
}

#[test]
fn fr015_sar_program_with_compartment_classification_round_trips() {
    // §H.5 p100 Table 7 canonical form: program + compartment + sub-comp.
    // Portion side.
    let scheme = CapcoScheme::new();
    let attrs1 = parse_portion(&scheme, "(TS//SAR-BP-J12)");
    let rendered = scheme.render_item(&CapcoMarking::from(attrs1.clone()));
    let attrs2 = parse_portion(&scheme, &format!("({rendered})"));
    assert_eq!(
        attrs1
            .classification
            .as_ref()
            .map(MarkingClassification::effective_level),
        attrs2
            .classification
            .as_ref()
            .map(MarkingClassification::effective_level),
        "SAR-BP-J12 portion classification round-trip drift; rendered {rendered:?}",
    );
}

// =============================================================================
// Full-attribute round-trip — IDEMPOTENCE form.
//
// The renderer has substantive per-axis bodies in
// `crates/capco/src/render/`. Per the "form is not shape" principle:
// two markings that differ only in delimiter, sort order, abbreviation,
// or inter-category position are lattice-equal on every axis, and the
// renderer chooses one canonical representative.
//
// The strict-AST round-trip property `attrs1 == attrs2` therefore does
// NOT hold for inputs that differ from canonical form (e.g.,
// `SPECIAL ACCESS REQUIRED-` indicator instead of canonical `SAR-`,
// reordered REL TO trigraphs, etc.) — the renderer canonicalizes the
// form, and the re-parsed AST will reflect that canonicalization.
//
// What DOES hold (and what this test pins) is the IDEMPOTENCE
// property: rendering a parsed-then-rendered fixture twice in a row
// produces byte-identical output. One round of canonicalization is
// sufficient; subsequent rounds are no-ops.
//
//     render(parse(render(parse(x)))) == render(parse(x))
//
// This is the load-bearing property `render_canonical` carries: the
// renderer is referentially transparent over lattice-equivalent
// inputs. See `tests/render_canonical_properties.rs` for the
// dedicated property test that pins this; this test exercises the
// same property across the strict-path corpus rather than hand-
// curated pairs.
//
// The assertion shape is idempotence rather than strict
// `attrs1 == attrs2` because form is not shape.
// =============================================================================

#[test]
fn full_attribute_round_trip_across_strict_corpus() {
    let scheme = CapcoScheme::new();
    let fixtures = valid_fixtures();
    assert!(
        !fixtures.is_empty(),
        "valid corpus is empty; check tests/corpus/capco/valid/ scaffold",
    );

    for path in &fixtures {
        let bytes = load_fixture(path);
        let text = fixture_text(&bytes);
        let kind = detect_kind(text.as_bytes());

        // First round — canonicalize the input.
        let (rendered_1, kind_owned) = match kind {
            Kind::Portion => {
                let attrs = parse_portion(&scheme, &text);
                let inner = scheme.render_item(&CapcoMarking::from(attrs));
                (format!("({inner})"), Kind::Portion)
            }
            Kind::Banner => {
                let attrs = parse_banner(&scheme, &text);
                (
                    scheme.render_summary(&CapcoMarking::from(attrs)),
                    Kind::Banner,
                )
            }
            Kind::Cab | Kind::Other => continue,
        };

        // Second round — render again from the re-parsed canonical
        // form. The output MUST be byte-identical to the first round.
        let rendered_2 = match kind_owned {
            Kind::Portion => {
                let attrs = parse_portion(&scheme, &rendered_1);
                let inner = scheme.render_item(&CapcoMarking::from(attrs));
                format!("({inner})")
            }
            Kind::Banner => {
                let attrs = parse_banner(&scheme, &rendered_1);
                scheme.render_summary(&CapcoMarking::from(attrs))
            }
            Kind::Cab | Kind::Other => unreachable!(),
        };

        assert_eq!(
            rendered_1,
            rendered_2,
            "renderer-canonical-form idempotence drift on fixture {} \
             (input {text:?} → first-render {rendered_1:?} → \
             re-render {rendered_2:?})",
            path.display(),
        );
    }
}
