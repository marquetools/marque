// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 9c.1 T134 — BOHEMIA / BALK route onto the SCI axis as
//! [`SciControlSystem::NatoSap`], not into a fused
//! `NatoClassification::CosmicTopSecretBohemia` / `CosmicTopSecretBalk`
//! variant.
//!
//! The legacy two-axis fusion (`NatoClassification::CosmicTopSecretBohemia`,
//! `NatoClassification::CosmicTopSecretBalk`) was retired in Commit 5
//! of PR 9c.1. The parser's `parse_nato_classification` (in
//! `crates/core/src/parser.rs`) now lifts BOHEMIA / BALK out of the
//! legacy fused classification text (`CTS-B` / `CTS-BALK`) and writes
//! them as `SciControlSystem::NatoSap(NatoSap::Bohemia)` /
//! `SciControlSystem::NatoSap(NatoSap::Balk)` entries on
//! `attrs.sci_markings`, leaving the classification axis carrying only
//! `NatoClassification::CosmicTopSecret`.
//!
//! BALK sorts before BOHEMIA alphabetically — the renderer follows the
//! lexicographic order required by §A.6 p15-16 (numeric-then-alpha
//! within an SCI category) and confirmed by the §H.7 p127 worked
//! example.
//!
//! # Coverage scope (R0 fix-up — what these tests pin)
//!
//! The post-PR-9c.1 invariants this file pins are:
//!
//! - The legacy compound text forms (`CTS-B`, `CTS-BALK`) canonicalize
//!   to bare CTS on the classification axis with `NatoSap::Bohemia` /
//!   `NatoSap::Balk` written onto `sci_markings`.
//! - The retired `NatoClassification::CosmicTopSecretBohemia` and
//!   `NatoClassification::CosmicTopSecretBalk` variants do not
//!   reappear (a future regression that re-fuses them would fail
//!   the legacy-form assertions immediately).
//! - `NatoSap` derives `Ord` such that `Balk < Bohemia` — the
//!   §H.7 p127 worked-example sort key the renderer drives off.
//!
//! # Out of scope (tracked-gap note for the PM)
//!
//! The canonical multi-block forms `(//CTS//BOHEMIA)` and
//! `(//CTS//BALK)` do NOT yet route BOHEMIA / BALK onto `sci_markings`
//! as `SciControlSystem::NatoSap`. The parser's structural SCI block
//! parser (`parse_sci_block`) recognizes only CVE-bare controls and
//! 2–5-char custom controls — BOHEMIA (7 chars) and BALK (4 chars)
//! fall outside the CVE registry (NATO SAPs have no ODNI CVE entry per
//! §G.2 p40), and BOHEMIA exceeds the 5-char custom-control length cap
//! in `is_valid_custom_control` (`crates/core/src/parser.rs`).
//!
//! Extending the SCI block parser to recognize BOHEMIA / BALK as
//! `NatoSap` is the natural follow-up: it closes the asymmetry where
//! legacy fused forms canonicalize correctly but the canonical form
//! parses to an empty SCI block. Tracking lives in the PR 9c.1
//! reviewer-fix-up commit message; the open-gap tests below carry
//! `#[ignore]` annotations so the gap stays visible without blocking CI.
//!
//! # Authority
//!
//! - CAPCO-2016 §G.2 p40 (Table 5: ARH by Registered Marking — registers
//!   BOHEMIA / BALK as standalone control markings, not classification
//!   suffixes).
//! - CAPCO-2016 §H.7 p127 (worked example
//!   `TOP SECRET//BOHEMIA//FGI AUS CAN DEU NATO//NOFORN` and the
//!   `(//CTS//BOHEMIA//REL TO USA, NATO)` portion — BOHEMIA in the SCI
//!   block position).
//! - CAPCO-2016 §A.6 p15-17 (multi-block portion grammar; BOHEMIA/BALK
//!   travel in the SCI block separated by `//`).
//!
//! # Spec linkage
//!
//! Reviewer fix-up under PR 9c.1 R0 (Commit 10) — replaces the empty
//! file fabricated in Commit 9 with substantive assertions.

use marque_capco::CapcoRuleSet;
use marque_capco::scheme::CapcoScheme;
use marque_config::Config;
use marque_core::Parser;
use marque_engine::{Engine, FixMode, FixedClock};
use marque_ism::{
    CapcoTokenSet, MarkingCandidate, MarkingClassification, MarkingType, NatoClassification,
    NatoSap, SciControlSystem, Span,
};

// ---------------------------------------------------------------------------
// Helpers — parse a candidate directly (no engine), so the parser-level
// routing invariants are testable without engine dispatch noise.
// ---------------------------------------------------------------------------

fn parse_portion(text: &str) -> marque_ism::CanonicalAttrs {
    parse_with_kind(text.as_bytes(), MarkingType::Portion)
}

fn parse_banner(text: &str) -> marque_ism::CanonicalAttrs {
    parse_with_kind(text.as_bytes(), MarkingType::Banner)
}

fn parse_with_kind(source: &[u8], kind: MarkingType) -> marque_ism::CanonicalAttrs {
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidate = MarkingCandidate {
        span: Span::new(0, source.len()),
        kind,
    };
    let parsed = parser
        .parse(&candidate, source)
        .expect("legacy / canonical NATO inputs must parse cleanly");
    // Test-fixture carve-out per Constitution V Principle V — wrap the
    // parser's borrowed output through the PR-3a transitional adapter
    // so tests can read the CanonicalAttrs surface the rules crate
    // consumes.
    marque_ism::from_parsed_unchecked(parsed.attrs)
}

fn engine_with_fixed_clock() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
        Box::new(FixedClock::new(std::time::UNIX_EPOCH)),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

/// Returns `true` when `attrs.sci_markings` contains a NATO SAP
/// matching `expected`. Iterates rather than indexing because the
/// renderer may interleave with other SCI markings on the same axis.
fn has_nato_sap(attrs: &marque_ism::CanonicalAttrs, expected: NatoSap) -> bool {
    attrs.sci_markings.iter().any(|m| {
        matches!(
            m.system,
            SciControlSystem::NatoSap(sap) if sap == expected
        )
    })
}

// ---------------------------------------------------------------------------
// Legacy portion forms canonicalize to bare class + SCI-axis NatoSap.
// These are the LOAD-BEARING post-PR-9c.1 invariants — the parser's
// `parse_nato_classification` was extended in Commit 3 to route
// CTS-B / CTS-BALK through `NatoCompanion::Sci(NatoSap)`.
// ---------------------------------------------------------------------------

/// `(//CTS-B)` — legacy fused portion form for COSMIC TOP SECRET
/// BOHEMIA. After PR 9c.1 the parser emits CTS on the classification
/// axis and `NatoSap::Bohemia` on the SCI axis.
///
/// Authority: CAPCO-2016 §G.1 Table 4 p38 (portion-form column for
/// COSMIC TOP SECRET BOHEMIA); §G.2 p40 (Table 5 registers BOHEMIA);
/// §H.7 p127 (BOHEMIA worked example in SCI block position).
#[test]
fn legacy_cts_b_canonicalizes_to_cts_plus_bohemia() {
    let attrs = parse_portion("(//CTS-B)");

    assert_eq!(
        attrs.classification,
        Some(MarkingClassification::Nato(
            NatoClassification::CosmicTopSecret
        )),
        "CTS-B must canonicalize to bare CTS on the classification axis"
    );
    assert!(
        has_nato_sap(&attrs, NatoSap::Bohemia),
        "CTS-B must canonicalize BOHEMIA onto sci_markings as NatoSap; got: {:?}",
        attrs.sci_markings,
    );
}

/// `(//CTS-BALK)` — legacy fused portion form for COSMIC TOP SECRET
/// BALK. After PR 9c.1 the parser emits CTS on the classification
/// axis and `NatoSap::Balk` on the SCI axis.
///
/// Authority: CAPCO-2016 §G.1 Table 4 p38 (portion-form column for
/// COSMIC TOP SECRET BALK); §G.2 p40 (Table 5 registers BALK).
#[test]
fn legacy_cts_balk_canonicalizes_to_cts_plus_balk() {
    let attrs = parse_portion("(//CTS-BALK)");

    assert_eq!(
        attrs.classification,
        Some(MarkingClassification::Nato(
            NatoClassification::CosmicTopSecret
        )),
        "CTS-BALK must canonicalize to bare CTS on the classification axis"
    );
    assert!(
        has_nato_sap(&attrs, NatoSap::Balk),
        "CTS-BALK must canonicalize BALK onto sci_markings as NatoSap; got: {:?}",
        attrs.sci_markings,
    );
}

// ---------------------------------------------------------------------------
// Banner legacy forms (the five banner-level patterns the parser's
// `parse_nato_classification` also accepts).
// ---------------------------------------------------------------------------

/// `//COSMIC TOP SECRET-BOHEMIA` — legacy banner form for the same
/// composite. Parser canonicalizes the same way the portion form does.
///
/// Authority: CAPCO-2016 §G.1 Table 4 p38 (banner-title column for
/// COSMIC TOP SECRET BOHEMIA); §G.2 p40.
#[test]
fn legacy_banner_cosmic_top_secret_bohemia_canonicalizes() {
    let attrs = parse_banner("//COSMIC TOP SECRET-BOHEMIA");

    assert_eq!(
        attrs.classification,
        Some(MarkingClassification::Nato(
            NatoClassification::CosmicTopSecret
        )),
        "COSMIC TOP SECRET-BOHEMIA must canonicalize to bare CTS on the \
         classification axis"
    );
    assert!(
        has_nato_sap(&attrs, NatoSap::Bohemia),
        "COSMIC TOP SECRET-BOHEMIA must canonicalize BOHEMIA onto sci_markings; \
         got: {:?}",
        attrs.sci_markings,
    );
}

/// `//COSMIC TOP SECRET-BALK` — legacy banner form. Same shape.
///
/// Authority: CAPCO-2016 §G.1 Table 4 p38 (banner-title column for
/// COSMIC TOP SECRET BALK); §G.2 p40.
#[test]
fn legacy_banner_cosmic_top_secret_balk_canonicalizes() {
    let attrs = parse_banner("//COSMIC TOP SECRET-BALK");

    assert_eq!(
        attrs.classification,
        Some(MarkingClassification::Nato(
            NatoClassification::CosmicTopSecret
        )),
        "COSMIC TOP SECRET-BALK must canonicalize to bare CTS on the \
         classification axis"
    );
    assert!(
        has_nato_sap(&attrs, NatoSap::Balk),
        "COSMIC TOP SECRET-BALK must canonicalize BALK onto sci_markings; \
         got: {:?}",
        attrs.sci_markings,
    );
}

// ---------------------------------------------------------------------------
// Order pin — BALK before BOHEMIA in NatoSap's derived Ord, matching
// `as_str()` lexicographic order. This is the post-PR-9c.1 invariant
// the renderer relies on for §H.5 numeric-then-alpha ordering.
// ---------------------------------------------------------------------------

/// `NatoSap::Balk < NatoSap::Bohemia` under the derived `Ord`,
/// matching the alphabetic ordering of `as_str()` (`"BALK" < "BOHEMIA"`).
/// The renderer drives §H.7 p127's BALK-before-BOHEMIA ordering off
/// this comparison.
///
/// Authority: CAPCO-2016 §H.7 p127 worked example renders multi-SAP
/// combinations alphabetically; §A.6 p15-16 (numeric-then-alpha within
/// an SCI category).
#[test]
fn balk_before_bohemia_in_sci_render() {
    assert!(
        NatoSap::Balk < NatoSap::Bohemia,
        "NatoSap derived Ord must place Balk < Bohemia (alphabetic on as_str()); \
         got: {:?}",
        (NatoSap::Balk, NatoSap::Bohemia),
    );
    assert_eq!(NatoSap::Balk.as_str(), "BALK");
    assert_eq!(NatoSap::Bohemia.as_str(), "BOHEMIA");
    assert!(
        NatoSap::Balk.as_str() < NatoSap::Bohemia.as_str(),
        "as_str() lexicographic order must agree with derived Ord"
    );
}

// ---------------------------------------------------------------------------
// Negative: bare CTS (no companion) must NOT route any NatoSap.
// Guards the regression class where a future refactor accidentally
// fires NatoSap inference off the bare classification.
// ---------------------------------------------------------------------------

/// `(//CTS)` — bare COSMIC TOP SECRET, no compound suffix. The parser
/// must NOT manufacture a `NatoSap` entry on the SCI axis. This pins
/// the negative direction of the canonicalization path.
///
/// Authority: CAPCO-2016 §A.6 p15-17 (multi-block portion grammar —
/// SCI markings travel in their own block, separated by `//`);
/// implementation: `parse_nato_classification` returns
/// `NatoCompanion::Bare` for bare-class inputs (no companion write).
#[test]
fn bare_cts_does_not_manufacture_nato_sap() {
    let attrs = parse_portion("(//CTS)");

    assert_eq!(
        attrs.classification,
        Some(MarkingClassification::Nato(
            NatoClassification::CosmicTopSecret
        )),
    );
    assert!(
        attrs.sci_markings.is_empty(),
        "bare CTS must NOT populate sci_markings; got: {:?}",
        attrs.sci_markings,
    );
}

// ---------------------------------------------------------------------------
// E066 round-trip — the legacy-form re-marking rule fires on the
// legacy compound text and not on bare CTS.
// ---------------------------------------------------------------------------

/// `(//CTS-B)` — legacy form triggers E066 (legacy NATO compound text
/// re-marking) at confidence 1.0; the engine auto-applies the
/// Recanonicalize fix.
///
/// Authority: CAPCO-2016 §G.2 p40 + §H.7 p127.
#[test]
fn e066_fires_on_legacy_cts_b_portion() {
    let engine = engine_with_fixed_clock();
    let source = b"(//CTS-B)";
    let lint = engine.lint(source);

    assert!(
        lint.diagnostics.iter().any(|d| d.rule.as_str() == "E066"),
        "E066 must fire on (//CTS-B) legacy portion; diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.as_str())
            .collect::<Vec<_>>(),
    );
}

/// `(//CTS)` — bare canonical form does NOT trigger E066. The rule's
/// predicate gates on the legacy fused text + a companion AEA/SCI
/// write, which only happens for the eight portion / five banner
/// legacy patterns. Bare canonical inputs are by construction not
/// legacy.
///
/// Authority: CAPCO-2016 §A.6 p15-17 — canonical multi-block forms
/// are the destination state E066 re-marks toward.
#[test]
fn e066_does_not_fire_on_bare_cts_portion() {
    let engine = engine_with_fixed_clock();
    let source = b"(//CTS)";
    let lint = engine.lint(source);

    assert!(
        lint.diagnostics.iter().all(|d| d.rule.as_str() != "E066"),
        "E066 must NOT fire on bare canonical (//CTS); diagnostics: {:?}",
        lint.diagnostics
            .iter()
            .map(|d| d.rule.as_str())
            .collect::<Vec<_>>(),
    );
}

// ---------------------------------------------------------------------------
// Tracked-gap tests — canonical multi-block forms `(//CTS//BOHEMIA)`,
// `(//CTS//BALK)`, and the §H.7 p127 worked example do NOT yet route
// the bare BOHEMIA / BALK token onto `sci_markings` as
// `SciControlSystem::NatoSap`. See file-level docstring for the gap
// analysis. These are `#[ignore]`d so the gap remains visible without
// blocking CI; flipping them to live tests is the natural follow-up.
// ---------------------------------------------------------------------------

/// IGNORED — tracks the parser gap. `(//CTS//BOHEMIA)` should route
/// BOHEMIA onto `sci_markings` as `SciControlSystem::NatoSap(Bohemia)`,
/// but `parse_sci_block` currently rejects BOHEMIA (7 chars exceeds
/// the 5-char custom-control length cap and BOHEMIA has no CVE
/// registration per §G.2 p40).
///
/// Authority: CAPCO-2016 §H.7 p127 — `(//CTS//BOHEMIA//REL TO USA, NATO)`
/// is the worked-example shape this canonical input mirrors.
#[test]
#[ignore = "PR 9c.1 R0: parser does not yet route canonical BOHEMIA to NatoSap; tracked follow-up"]
fn canonical_form_bohemia_on_sci_axis_not_yet_routed() {
    let attrs = parse_portion("(//CTS//BOHEMIA)");
    assert_eq!(
        attrs.classification,
        Some(MarkingClassification::Nato(
            NatoClassification::CosmicTopSecret
        )),
    );
    assert!(
        has_nato_sap(&attrs, NatoSap::Bohemia),
        "canonical form must route BOHEMIA onto sci_markings; got: {:?}",
        attrs.sci_markings,
    );
}

/// IGNORED — companion gap test for canonical BALK.
///
/// Authority: CAPCO-2016 §G.2 p40 (BALK as standalone control marking).
#[test]
#[ignore = "PR 9c.1 R0: parser does not yet route canonical BALK to NatoSap; tracked follow-up"]
fn canonical_form_balk_on_sci_axis_not_yet_routed() {
    let attrs = parse_portion("(//CTS//BALK)");
    assert_eq!(
        attrs.classification,
        Some(MarkingClassification::Nato(
            NatoClassification::CosmicTopSecret
        )),
    );
    assert!(
        has_nato_sap(&attrs, NatoSap::Balk),
        "canonical form must route BALK onto sci_markings; got: {:?}",
        attrs.sci_markings,
    );
}

/// IGNORED — §H.7 p127 worked-example end-to-end.
///
/// `TOP SECRET//BOHEMIA//FGI AUS CAN DEU NATO//NOFORN` is the
/// authoritative worked example from the CAPCO manual. End-to-end
/// recognition pins both parser routing (BOHEMIA → SCI axis) and
/// engine behavior (no E066 fire on canonical input).
///
/// Authority: CAPCO-2016 §H.7 p127 — verbatim banner from the FGI
/// section's BOHEMIA worked example.
#[test]
#[ignore = "PR 9c.1 R0: parser does not yet route canonical BOHEMIA to NatoSap; tracked follow-up"]
fn h7_p127_worked_example_round_trip_not_yet_supported() {
    let source = b"TOP SECRET//BOHEMIA//FGI AUS CAN DEU NATO//NOFORN";

    let attrs = parse_banner(std::str::from_utf8(source).unwrap());
    assert!(
        matches!(
            attrs.classification,
            Some(MarkingClassification::Us(
                marque_ism::Classification::TopSecret
            ))
        ),
        "§H.7 p127 banner must parse as US Top Secret; got: {:?}",
        attrs.classification,
    );
    assert!(
        has_nato_sap(&attrs, NatoSap::Bohemia),
        "§H.7 p127 banner must carry BOHEMIA on sci_markings; got: {:?}",
        attrs.sci_markings,
    );

    let engine = engine_with_fixed_clock();
    let result = engine.fix(source, FixMode::Apply);
    assert!(
        result.applied.iter().all(|af| af.rule.as_str() != "E066"),
        "§H.7 p127 canonical banner must NOT trigger E066; applied: {:?}",
        result
            .applied
            .iter()
            .map(|af| af.rule.as_str())
            .collect::<Vec<_>>(),
    );
}
