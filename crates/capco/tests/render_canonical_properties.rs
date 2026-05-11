// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 3c.B Commit 5 — `render_canonical` property tests.
//!
//! Two properties pin the renderer's structural commitments:
//!
//! 1. **`round_trip_idempotent`** —
//!    `render(parse(render(parse(x)))) == render(parse(x))` for every
//!    fixture in the strict-path corpus. One round of canonicalization
//!    stabilizes; subsequent rounds are no-ops.
//! 2. **`lattice_equal_renders_byte_identical`** — pairs of inputs
//!    that are lattice-equal but differ only by form (delimiter,
//!    sort order, abbreviation) MUST render to byte-identical output.
//!    This is the load-bearing property the architecture spec calls
//!    "form is not shape" (§3.0.a).
//!
//! # Authority
//!
//! - `specs/006-engine-rule-refactor/architecture.md` §3.0.a "form is
//!   not shape" + "What this commits us to" (renderer is the single
//!   source of canonical form).
//! - CAPCO-2016 §H — per-axis canonical-form definitions cited in
//!   each pair below.

use marque_capco::scheme::{CapcoMarking, CapcoScheme};
use marque_core::{Parser, Scanner};
use marque_ism::span::{MarkingCandidate, MarkingType, Span};
use marque_ism::token_set::CapcoTokenSet;
use marque_ism::{CanonicalAttrs, MarkingClassification};
use marque_scheme::MarkingScheme;
use marque_test_utils::{load_fixture, valid_fixtures};

// ---------------------------------------------------------------------------
// Parse helpers — mirror parse_render_roundtrip.rs
// ---------------------------------------------------------------------------

fn parse_with_kind(source: &[u8], kind: MarkingType) -> Option<CanonicalAttrs> {
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidate = MarkingCandidate {
        span: Span::new(0, source.len()),
        kind,
    };
    parser
        .parse(&candidate, source)
        .ok()
        // Test-fixture carve-out per Constitution V Principle V.
        .map(|p| marque_ism::from_parsed_unchecked(p.attrs))
}

fn parse_banner(text: &str) -> CanonicalAttrs {
    parse_with_kind(text.as_bytes(), MarkingType::Banner)
        .expect("banner candidate from valid corpus must parse")
}

fn parse_portion(text: &str) -> CanonicalAttrs {
    parse_with_kind(text.as_bytes(), MarkingType::Portion)
        .expect("portion candidate from valid corpus must parse")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Portion,
    Banner,
    Other,
}

fn detect_kind(source: &[u8]) -> Kind {
    let candidates = Scanner::scan(source);
    let mut markings: Vec<&MarkingCandidate> = candidates
        .iter()
        .filter(|c| !matches!(c.kind, MarkingType::PageBreak))
        .collect();
    if markings.len() != 1 {
        return Kind::Other;
    }
    match markings.pop().unwrap().kind {
        MarkingType::Portion => Kind::Portion,
        MarkingType::Banner => Kind::Banner,
        _ => Kind::Other,
    }
}

fn fixture_text(bytes: &[u8]) -> String {
    let s = std::str::from_utf8(bytes).expect("fixture must be UTF-8");
    s.lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .to_owned()
}

// ---------------------------------------------------------------------------
// Property 1 — round_trip_idempotent
// ---------------------------------------------------------------------------

#[test]
fn round_trip_idempotent() {
    // For every fixture in the strict-path corpus, two consecutive
    // render-of-parse rounds must produce byte-identical output. The
    // first round canonicalizes form; the second must be a no-op.
    let scheme = CapcoScheme::new();
    let fixtures = valid_fixtures();
    assert!(
        !fixtures.is_empty(),
        "valid corpus is empty; check tests/corpus/valid/ scaffold",
    );

    let mut exercised = 0;

    for path in &fixtures {
        let bytes = load_fixture(path);
        let text = fixture_text(&bytes);
        let kind = detect_kind(text.as_bytes());

        let first = match kind {
            Kind::Portion => {
                let attrs = parse_portion(&text);
                let inner = scheme.render_portion(&CapcoMarking::from(attrs));
                format!("({inner})")
            }
            Kind::Banner => {
                let attrs = parse_banner(&text);
                scheme.render_banner(&CapcoMarking::from(attrs))
            }
            Kind::Other => continue,
        };

        let second = match kind {
            Kind::Portion => {
                let attrs = parse_portion(&first);
                let inner = scheme.render_portion(&CapcoMarking::from(attrs));
                format!("({inner})")
            }
            Kind::Banner => {
                let attrs = parse_banner(&first);
                scheme.render_banner(&CapcoMarking::from(attrs))
            }
            Kind::Other => unreachable!(),
        };

        assert_eq!(
            first,
            second,
            "round_trip_idempotent failed on fixture {} \
             (input {text:?} → first {first:?} → second {second:?})",
            path.display(),
        );
        exercised += 1;
    }

    assert!(
        exercised > 0,
        "round_trip_idempotent exercised no fixtures (corpus filter too aggressive?)",
    );
}

// ---------------------------------------------------------------------------
// Property 2 — lattice_equal_renders_byte_identical
// ---------------------------------------------------------------------------
//
// Hand-curated pairs covering the canonicalization classes from
// CAPCO-2016 §H. Each pair is two distinct input forms that MUST
// canonicalize to the same output bytes. Each entry carries an inline
// §-citation justifying which form is canonical.

/// A pair-test row: two banner inputs that the renderer must canonicalize
/// to the same bytes.
struct CanonPair {
    /// Diagnostic name (used in assertion messages).
    name: &'static str,
    /// First banner input form.
    a: &'static str,
    /// Second banner input form.
    b: &'static str,
    /// Expected canonical bytes the renderer should produce for both.
    canonical: &'static str,
    /// Authoritative CAPCO-2016 citation for the canonical form.
    citation: &'static str,
}

static BANNER_PAIRS: &[CanonPair] = &[
    // §H.8 p150-151 + §A.6 p16: USA-first, then trigraphs alpha,
    // tetragraphs alpha. Comma-space separated.
    CanonPair {
        name: "rel-to/usa-first-then-alpha",
        a: "SECRET//REL TO GBR, USA, JPN",
        b: "SECRET//REL TO USA, JPN, GBR",
        canonical: "SECRET//REL TO USA, GBR, JPN",
        citation: "CAPCO-2016 §H.8 p150-151 + §A.6 p16",
    },
    // §H.8 p150-151: trigraphs alpha first, then tetragraphs alpha.
    CanonPair {
        name: "rel-to/trigraphs-then-tetragraphs",
        a: "SECRET//REL TO USA, NATO, GBR, FVEY",
        b: "SECRET//REL TO USA, GBR, FVEY, NATO",
        canonical: "SECRET//REL TO USA, GBR, FVEY, NATO",
        citation: "CAPCO-2016 §H.8 p150-151",
    },
    // §A.6 p16 + §H.4 p61: SCI compartments numeric-then-alpha sort.
    CanonPair {
        name: "sci/compartment-numeric-then-alpha",
        a: "TOP SECRET//SI-G ABCD DEFG",
        b: "TOP SECRET//SI-G DEFG ABCD",
        canonical: "TOP SECRET//SI-G ABCD DEFG",
        citation: "CAPCO-2016 §A.6 p15-16 + §H.4 p61",
    },
    // §A.6 p16 + §H.5 p99-100: SAR programs ascending alpha
    // (numeric first), `/`-separated.
    CanonPair {
        name: "sar/program-ascending-alpha",
        a: "SECRET//SAR-XYZ/ABC",
        b: "SECRET//SAR-ABC/XYZ",
        canonical: "SECRET//SAR-ABC/XYZ",
        citation: "CAPCO-2016 §A.6 p16 + §H.5 p99-100",
    },
    // §H.6 + Table 4 §6 p36: SIGMA numbers ascending numerical sort.
    CanonPair {
        name: "aea/sigma-numeric-ascending",
        a: "SECRET//RD-SIGMA 18 14//NOFORN",
        b: "SECRET//RD-SIGMA 14 18//NOFORN",
        canonical: "SECRET//RD-SIGMA 14 18//NOFORN",
        citation: "CAPCO-2016 §H.6 + Table 4 §6 p36",
    },
    // §A.6 p16 + Table 4 §6 p36: AEA Register order RD < FRD.
    CanonPair {
        name: "aea/register-order-rd-before-frd",
        a: "SECRET//FRD/RD//NOFORN",
        b: "SECRET//RD/FRD//NOFORN",
        canonical: "SECRET//RD/FRD//NOFORN",
        citation: "CAPCO-2016 §A.6 p16 + §H.6 Table 4 §6 p36",
    },
    // §A.6 p16 + Table 4 §8 p36: IC dissem in Register order
    // (NOFORN < ORCON … reorder via the RELIDO + NOFORN flip below).
    // NB: The pair shows two valid Register-order inputs producing
    // the same canonical form. (A "reverse-order" probe of
    // `RELIDO/NOFORN` would also test renderer canonicalization, but
    // the current parser rejects out-of-Register-order dissem inputs
    // — that's a parser limitation outside this commit's scope.
    // Tracked as an open question for the parser owner.)
    CanonPair {
        name: "dissem/register-order-noforn-before-relido",
        a: "SECRET//NOFORN/RELIDO",
        b: "SECRET//NOFORN/RELIDO",
        canonical: "SECRET//NOFORN/RELIDO",
        citation: "CAPCO-2016 §A.6 p16 + §H.8 Table 4 §8 p36",
    },
    // §A.6 p16: Multiple SCI control systems `/`-separated, sorted
    // numeric-then-alpha. SI < TK alphabetically.
    CanonPair {
        name: "sci/multiple-systems-alpha-sort",
        a: "TOP SECRET//TK/SI//NOFORN",
        b: "TOP SECRET//SI/TK//NOFORN",
        canonical: "TOP SECRET//SI/TK//NOFORN",
        citation: "CAPCO-2016 §A.6 p15-16",
    },
    // §H.3 p56 + §A.6 p15-16: JOINT [LIST] is alphabetical (trigraphs
    // first, then tetragraphs, each alpha-sorted). USA appears in
    // alphabetical position, NOT pulled to the front. Canonical
    // examples on §H.3 p56 ("//JOINT TOP SECRET CAN ISR USA"), §H.3
    // p58 ("//JOINT SECRET CAN GBR USA"), and §H.3 p59
    // ("//JOINT SECRET GBR USA") all show USA in its alphabetical
    // slot. The USA-first rule is REL TO-axis only, NOT JOINT-axis
    // (caught in pre-flight review as a Constitution VIII defect).
    CanonPair {
        name: "joint/alphabetical",
        a: "//JOINT SECRET GBR USA AUS",
        b: "//JOINT SECRET USA AUS GBR",
        canonical: "//JOINT SECRET AUS GBR USA",
        citation: "CAPCO-2016 §A.6 p15-16 + §H.3 p56",
    },
    // §A.6 p16 + §H.7 p123: FGI marker — trigraphs alpha first,
    // then tetragraphs alpha, space-separated. (NB: the FGI content
    // marker; not the FGI classification system.)
    CanonPair {
        name: "fgi-marker/trigraphs-then-tetragraphs",
        a: "SECRET//FGI NATO GBR JPN//REL TO USA, GBR, JPN, NATO",
        b: "SECRET//FGI JPN GBR NATO//REL TO USA, GBR, JPN, NATO",
        canonical: "SECRET//FGI GBR JPN NATO//REL TO USA, GBR, JPN, NATO",
        citation: "CAPCO-2016 §A.6 p16 + §H.7 p123",
    },
];

#[test]
fn lattice_equal_renders_byte_identical() {
    let scheme = CapcoScheme::new();

    for pair in BANNER_PAIRS {
        let attrs_a = parse_banner(pair.a);
        let attrs_b = parse_banner(pair.b);

        let render_a = scheme.render_banner(&CapcoMarking::from(attrs_a));
        let render_b = scheme.render_banner(&CapcoMarking::from(attrs_b));

        assert_eq!(
            render_a, pair.canonical,
            "{}: input A {:?} did not produce canonical {:?}; got {:?}. \
             Authority: {}.",
            pair.name, pair.a, pair.canonical, render_a, pair.citation,
        );
        assert_eq!(
            render_b, pair.canonical,
            "{}: input B {:?} did not produce canonical {:?}; got {:?}. \
             Authority: {}.",
            pair.name, pair.b, pair.canonical, render_b, pair.citation,
        );
        assert_eq!(
            render_a, render_b,
            "{}: lattice-equal inputs A {:?} and B {:?} rendered \
             differently — A→{:?}, B→{:?}. \
             Authority: {}.",
            pair.name, pair.a, pair.b, render_a, render_b, pair.citation,
        );
    }
}

// ---------------------------------------------------------------------------
// Sanity check — `MarkingClassification` import not unused
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn _force_classification_import(_c: MarkingClassification) {}
