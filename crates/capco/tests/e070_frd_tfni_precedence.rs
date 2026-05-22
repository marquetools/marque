// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! E070 — FRD takes precedence over TFNI. #559 close-out PM decision
//! 2026-05-19: E024 covers RD>FRD AND RD>TFNI; this row carries the
//! FRD>TFNI leg as a distinct catalog entry.
//!
//! Predicate-level + bridge-emission pinning. Post-#661 the helper
//! populates both `span` (anchored on TFNI via an inline
//! `text == "TFNI"` walk over `token_spans`, falling back to the
//! shared `token_span_attrs(TokenRef::Token(TOK_TFNI))` arm) and
//! `severity: Some(Severity::Fix)`, producing a user-visible
//! Diagnostic through `bridge_constraint_diagnostic`. The
//! pre-#661 advisory `(None, None)` shape was structurally
//! suppressed by the bridge's both-fields-required guard at
//! `crates/engine/src/engine.rs` and never surfaced to consumers.
//!
//! These tests exercise the predicate independently of the engine
//! bridge — see `crates/capco/src/scheme/predicates/tier1_mask.rs`
//! unit tests for the in-helper span/severity shape pinning, and
//! `tests/corpus/invalid/e070_frd_tfni_precedence.{txt,expected.json}`
//! for the end-to-end engine emission contract.
//!
//! Authority: CAPCO-2016 §H.6 p120 (TFNI subsection precedence rules
//! plus commingling rules). Verified against
//! `crates/capco/docs/CAPCO-2016.md` at the time of authorship.

use marque_capco::scheme::{CapcoMarking, CapcoScheme};
use marque_ism::{CanonicalAttrs, CapcoTokenSet, MarkingCandidate, MarkingType, Span};
use marque_scheme::MarkingScheme;

/// Parse a portion text into `CanonicalAttrs` via the canonical
/// parser path — same shape used by the other CAPCO integration
/// tests (e.g., `dissem_nato_pure_nato_portion.rs`).
fn parse_portion(scheme: &CapcoScheme, text: &str) -> CanonicalAttrs {
    // PR 3c.2.B (PM-B-3 second clause): the helper takes `&CapcoScheme`
    // so each #[test] can reuse the scheme it already constructs for
    // `fires_e070(&scheme, ...)`.
    let tokens = CapcoTokenSet;
    let parser = marque_core::Parser::new(&tokens);
    let cand = MarkingCandidate {
        span: Span::new(0, text.len()),
        kind: MarkingType::Portion,
    };
    let parsed = parser
        .parse(&cand, text.as_bytes())
        .expect("E070 test inputs must parse cleanly");
    scheme.canonicalize(parsed.attrs)
}

/// Did `scheme.validate(marking)` produce a `ConstraintViolation`
/// whose `constraint_label` matches the FRD/TFNI precedence
/// predicate (post-T044: `portion.aea.frd-tfni-precedence`)?
fn fires_e070(scheme: &CapcoScheme, attrs: CanonicalAttrs) -> bool {
    let marking = CapcoMarking::new(attrs);
    scheme
        .validate(&marking)
        .iter()
        .any(|v| v.constraint_label == "portion.aea.frd-tfni-precedence")
}

#[test]
fn fires_on_frd_and_tfni_together() {
    // `(TS//FRD//TFNI//NF)` — the canonical commingling case from
    // §H.6 p120 ("If TFNI is commingled with RD or FRD within a
    // portion, the RD or FRD takes precedence").
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(TS//FRD//TFNI//NF)");

    // Sanity: parser must have placed BOTH FRD and TFNI in the AEA
    // axis. If this fires, the parser dropped one and the test no
    // longer exercises the E070 predicate.
    let has_frd = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Frd(_)));
    let has_tfni = attrs
        .aea_markings
        .iter()
        .any(|a| matches!(a, marque_ism::AeaMarking::Tfni));
    assert!(
        has_frd && has_tfni,
        "test fixture `(TS//FRD//TFNI//NF)` must parse with both \
         AeaMarking::Frd and AeaMarking::Tfni; got aea_markings = {:?}",
        attrs.aea_markings,
    );

    assert!(
        fires_e070(&scheme, attrs),
        "E070 must fire when FRD and TFNI are commingled in one portion \
         per CAPCO-2016 §H.6 p120",
    );
}

#[test]
fn silent_on_frd_alone() {
    // `(TS//FRD//NF)` — FRD only, no TFNI. E070 must NOT fire.
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(TS//FRD//NF)");

    assert!(
        !fires_e070(&scheme, attrs),
        "E070 must NOT fire when FRD is present without TFNI",
    );
}

#[test]
fn silent_on_tfni_alone() {
    // `(TS//TFNI//NF)` — TFNI only, no FRD. E070 must NOT fire.
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(TS//TFNI//NF)");

    assert!(
        !fires_e070(&scheme, attrs),
        "E070 must NOT fire when TFNI is present without FRD",
    );
}

#[test]
fn fires_when_rd_also_present() {
    // `(TS//RD//FRD//TFNI//NF)` — RD AND FRD AND TFNI all present.
    // Both E024 (RD>FRD/TFNI) and E070 (FRD>TFNI) hold simultaneously.
    // The catalog entries are independent policy decisions per
    // Constitution V Principle V; E070 must fire regardless of RD
    // presence.
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(TS//RD//FRD//TFNI//NF)");

    assert!(
        fires_e070(&scheme, attrs),
        "E070 must fire when FRD and TFNI co-occur, regardless of RD \
         presence — the FRD>TFNI relationship is independent of \
         RD>FRD/TFNI (E024). Constitution V Principle V (one policy → \
         one audit lineage).",
    );
}

#[test]
fn silent_when_no_aea() {
    // `(TS//NF)` — no AEA markings at all. E070 must NOT fire.
    let scheme = CapcoScheme::new();
    let attrs = parse_portion(&scheme, "(TS//NF)");

    assert!(
        !fires_e070(&scheme, attrs),
        "E070 must NOT fire on a portion with no AEA markings",
    );
}
