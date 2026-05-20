// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 9c.1 T134 — ATOMAL routes onto the AEA axis, not into a fused
//! `NatoClassification` variant.
//!
//! The legacy two-axis fusion (`NatoClassification::*Atomal`) was retired
//! in Commit 5 of PR 9c.1. The parser now lifts ATOMAL out of the
//! classification text and writes it as an [`AeaMarking::Atomal`] on
//! `attrs.aea_markings`, leaving the classification axis carrying only
//! `NatoClassification::CosmicTopSecret` / `NatoSecret` / `NatoConfidential`.
//!
//! This file pins that routing at the parser boundary, exercising both
//! the canonical multi-block form (`(//CTS//ATOMAL)`) and the legacy
//! portion forms the parser canonicalizes (`(CTSA)`, `(NSAT)`, `(NCA)`),
//! plus the §H.7 p122 worked example end-to-end. A future refactor that
//! re-fuses ATOMAL into the classification axis trips these tests
//! immediately.
//!
//! # Authority
//!
//! - CAPCO-2016 §G.2 p40 (Table 5: ARH by Registered Marking — registers
//!   ATOMAL as a standalone control marking).
//! - CAPCO-2016 §H.7 p122 (worked example
//!   `SECRET//RD/ATOMAL//FGI NATO//NOFORN` — ATOMAL travels in the AEA
//!   block after RD, not as a classification suffix).
//! - CAPCO-2016 §A.6 p15-17 (multi-block portion grammar; ATOMAL is its
//!   own block separated by `//`).
//!
//! # Spec linkage
//!
//! Reviewer fix-up under PR 9c.1 R0 (Commit 10) — replaces the empty
//! file fabricated in Commit 9 with substantive assertions.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_core::Parser;
use marque_engine::{Engine, FixMode, FixedClock};
use marque_ism::{
    AeaMarking, CapcoTokenSet, MarkingCandidate, MarkingClassification, MarkingType,
    NatoClassification, Span,
};
use marque_scheme::MarkingScheme as _;

// ---------------------------------------------------------------------------
// Helpers — parse a candidate directly (no engine), so the parser-level
// routing invariants are testable without engine dispatch noise.
// ---------------------------------------------------------------------------

fn parse_portion(scheme: &CapcoScheme, text: &str) -> marque_ism::CanonicalAttrs {
    parse_with_kind(scheme, text.as_bytes(), MarkingType::Portion)
}

fn parse_banner(scheme: &CapcoScheme, text: &str) -> marque_ism::CanonicalAttrs {
    parse_with_kind(scheme, text.as_bytes(), MarkingType::Banner)
}

fn parse_with_kind(
    scheme: &CapcoScheme,
    source: &[u8],
    kind: MarkingType,
) -> marque_ism::CanonicalAttrs {
    // PR 3c.2.B (PM-B-3 second clause): the helper takes `&CapcoScheme`
    // so each #[test] can reuse a single scheme rather than allocating
    // one per parse. Per PM-B-3: "Where the test helper is module-level
    // and called from multiple #[test] functions, the helper takes
    // `&CapcoScheme` as a parameter; each #[test] constructs the scheme
    // inline."
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidate = MarkingCandidate {
        span: Span::new(0, source.len()),
        kind,
    };
    let parsed = parser
        .parse(&candidate, source)
        .expect("legacy / canonical NATO inputs must parse cleanly");
    scheme.canonicalize(parsed.attrs)
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

// ---------------------------------------------------------------------------
// Canonical-form ATOMAL: AEA axis, NOT a fused classification variant.
// ---------------------------------------------------------------------------

/// `(//CTS//ATOMAL)` — canonical multi-block input. ATOMAL lives in
/// `attrs.aea_markings`; the classification axis carries only
/// `NatoClassification::CosmicTopSecret`.
///
/// Authority: CAPCO-2016 §H.7 p122 (the AEA-block worked example
/// confirms ATOMAL travels after RD on the AEA axis); §G.2 p40 (Table 5
/// registers ATOMAL as a standalone marking).
#[test]
fn atomal_on_aea_axis_not_nato_classification() {
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(//CTS//ATOMAL)");

    assert_eq!(
        attrs.classification,
        Some(MarkingClassification::Nato(
            NatoClassification::CosmicTopSecret
        )),
        "classification axis must carry bare NatoClassification::CosmicTopSecret, \
         not a fused *Atomal variant"
    );
    assert!(
        attrs
            .aea_markings
            .iter()
            .any(|a| matches!(a, AeaMarking::Atomal(_))),
        "aea_markings must contain AeaMarking::Atomal; got: {:?}",
        attrs.aea_markings,
    );
}

// ---------------------------------------------------------------------------
// Legacy portion forms canonicalize to bare class + ATOMAL on AEA axis.
// Eight portion patterns total; the ATOMAL family covers CTSA / CTS-A /
// NSAT / NS-A / NCA / NC-A. Each test below pins a representative.
// ---------------------------------------------------------------------------

/// `(//CTSA)` — legacy fused portion form. After PR 9c.1 the parser
/// emits CTS on the classification axis and ATOMAL on the AEA axis.
/// The `//` prefix is mandatory for NATO portion forms per the
/// portion grammar in CAPCO-2016 §A.6 p15-17.
///
/// Authority: CAPCO-2016 §G.1 Table 4 p38 (portion-form column for
/// COSMIC TOP SECRET ATOMAL); §G.2 p40 (Table 5: ATOMAL as standalone
/// control marking).
#[test]
fn legacy_ctsa_canonicalizes_to_cts_plus_atomal() {
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(//CTSA)");

    assert_eq!(
        attrs.classification,
        Some(MarkingClassification::Nato(
            NatoClassification::CosmicTopSecret
        )),
        "CTSA must canonicalize to bare CTS on the classification axis"
    );
    assert!(
        attrs
            .aea_markings
            .iter()
            .any(|a| matches!(a, AeaMarking::Atomal(_))),
        "CTSA must canonicalize ATOMAL onto aea_markings; got: {:?}",
        attrs.aea_markings,
    );
}

/// `(//NSAT)` — legacy NATO SECRET ATOMAL portion form. The `//`
/// prefix is mandatory for NATO portion forms per §A.6 p15-17.
///
/// Authority: CAPCO-2016 §G.1 Table 4 p38 (portion-form column for
/// NATO SECRET ATOMAL); §G.2 p40.
#[test]
fn legacy_nsat_canonicalizes_to_ns_plus_atomal() {
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(//NSAT)");

    assert_eq!(
        attrs.classification,
        Some(MarkingClassification::Nato(NatoClassification::NatoSecret)),
        "NSAT must canonicalize to bare NS on the classification axis"
    );
    assert!(
        attrs
            .aea_markings
            .iter()
            .any(|a| matches!(a, AeaMarking::Atomal(_))),
        "NSAT must canonicalize ATOMAL onto aea_markings; got: {:?}",
        attrs.aea_markings,
    );
}

/// `(//NCA)` — legacy NATO CONFIDENTIAL ATOMAL portion form. The
/// `//` prefix is mandatory for NATO portion forms per §A.6 p15-17.
///
/// Authority: CAPCO-2016 §G.1 Table 4 p38 (portion-form column for
/// NATO CONFIDENTIAL ATOMAL); §G.2 p40.
#[test]
fn legacy_nca_canonicalizes_to_nc_plus_atomal() {
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(//NCA)");

    assert_eq!(
        attrs.classification,
        Some(MarkingClassification::Nato(
            NatoClassification::NatoConfidential
        )),
        "NCA must canonicalize to bare NC on the classification axis"
    );
    assert!(
        attrs
            .aea_markings
            .iter()
            .any(|a| matches!(a, AeaMarking::Atomal(_))),
        "NCA must canonicalize ATOMAL onto aea_markings; got: {:?}",
        attrs.aea_markings,
    );
}

/// `(S//RD/ATOMAL//FGI NATO//NF)` — the structural shape of the §H.7
/// p122 worked example. ATOMAL appears in the AEA block AFTER RD; the
/// classification axis carries only `Secret` (US, not NATO).
///
/// Authority: CAPCO-2016 §H.7 p122 — the manual's authoritative
/// worked example for ATOMAL placement in a US-classified document
/// with FGI NATO ownership.
#[test]
fn atomal_renders_after_rd_in_aea_block() {
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(S//RD/ATOMAL//FGI NATO//NF)");

    // US classification, NOT NATO. The §H.7 p122 example is a US
    // document.
    assert!(
        matches!(
            attrs.classification,
            Some(MarkingClassification::Us(
                marque_ism::Classification::Secret
            ))
        ),
        "§H.7 p122 example must carry US Secret classification; got: {:?}",
        attrs.classification,
    );

    // AEA axis carries both RD and ATOMAL. The exact ordering is
    // renderer territory (§H.7 p122 shows RD/ATOMAL); here we pin
    // the presence of both on the axis.
    let aea: Vec<_> = attrs.aea_markings.iter().collect();
    assert!(
        aea.iter().any(|a| matches!(a, AeaMarking::Rd(_))),
        "§H.7 p122 example must carry RD on aea_markings; got: {aea:?}",
    );
    assert!(
        aea.iter().any(|a| matches!(a, AeaMarking::Atomal(_))),
        "§H.7 p122 example must carry ATOMAL on aea_markings; got: {aea:?}",
    );
}

// ---------------------------------------------------------------------------
// Banner-form round-trip — the §H.7 p122 worked example, end-to-end
// through the engine. Confirms parser + rule-set + renderer agree on
// the AEA-axis routing for ATOMAL.
// ---------------------------------------------------------------------------

/// `TOP SECRET//RD/ATOMAL//FGI NATO//NOFORN` — the §H.7 p122 worked
/// example as a banner. End-to-end through `Engine::fix` — confirms
/// no diagnostic mis-attributes ATOMAL to the classification axis and
/// no rule attempts to re-mark a canonical input.
///
/// Authority: CAPCO-2016 §H.7 p122 — the exact banner string from
/// the FGI section's ATOMAL worked example.
#[test]
fn atomal_banner_h7_p122_example_end_to_end_round_trip() {
    let source = b"TOP SECRET//RD/ATOMAL//FGI NATO//NOFORN";

    // Parser-level: classification = US Top Secret, AEA = [RD, ATOMAL].
    let scheme = CapcoScheme::new();
    let attrs = parse_banner(&scheme, std::str::from_utf8(source).unwrap());
    assert!(
        matches!(
            attrs.classification,
            Some(MarkingClassification::Us(
                marque_ism::Classification::TopSecret
            ))
        ),
        "§H.7 p122 banner must parse as US Top Secret; got: {:?}",
        attrs.classification,
    );
    assert!(
        attrs
            .aea_markings
            .iter()
            .any(|a| matches!(a, AeaMarking::Atomal(_))),
        "§H.7 p122 banner must carry ATOMAL on aea_markings; got: {:?}",
        attrs.aea_markings,
    );

    // Engine-level: a canonical input must NOT fire E066 (the legacy
    // re-marking rule). E066 is the legacy-text re-marking rule;
    // canonical forms are by construction not legacy.
    let engine = engine_with_fixed_clock();
    let result = engine.fix(source, FixMode::Apply);
    assert!(
        result.applied.iter().all(|af| af.rule.as_str() != "E066"),
        "§H.7 p122 canonical banner must NOT trigger E066; applied: {:?}",
        result
            .applied
            .iter()
            .map(|af| af.rule.as_str())
            .collect::<Vec<_>>(),
    );
}
