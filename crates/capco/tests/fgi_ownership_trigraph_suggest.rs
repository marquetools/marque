// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Integration tests for `capco:portion.fgi.ownership-trigraph-suggest`
//! (issue #545).
//!
//! Fires on shape-admitted-but-unregistered FGI ownership tokens like
//! `(S//FGI XX)` and `(S//FGI ZZZ)`. Architectural twin of S004
//! `RelToTrigraphSuggestRule` — reuses the corpus-prior + edit-distance
//! machinery on the FGI ownership axis. Suggest channel: the engine
//! never auto-applies a `Severity::Suggest` diagnostic regardless of
//! confidence.
//!
//! # Coverage
//!
//! - Unregistered shape-admitted tokens emit diagnostics: `XX`, `ZZZ`.
//! - Registered ownership tokens stay silent: `USA`, `NATO`, `EU`.
//! - Banner-form FGI flows through the same parser path: assert
//!   the rule fires equivalently.
//! - Multi-country lists with mixed registered + unregistered: only
//!   the unregistered tokens trigger.
//! - Multiple unregistered tokens in one list emit per-token
//!   diagnostics.
//! - Span precision: the diagnostic span anchors exactly on the
//!   offending token's byte range (not the whole FGI marker block).
//! - Issue #672 / engine `Severity::Off` gate: configuring the rule
//!   `off` in `.marque.toml` suppresses all diagnostics.
//! - E073 disjointness: shape-rejected `FVEY` triggers E073 only;
//!   shape-admitted `XX` triggers this rule only. No co-firing.
//!
//! Authority: CAPCO-2016 §H.7 p122 + §A.6 p16. Both citations
//! re-verified against `crates/capco/docs/CAPCO-2016.md` at authorship
//! per Constitution VIII.

use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_config::Config;
use marque_engine::Engine;
use marque_rules::Severity;

const RULE_PREDICATE: &str = "portion.fgi.ownership-trigraph-suggest";
const RULE_WIRE_STRING: &str = "capco:portion.fgi.ownership-trigraph-suggest";

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("default CAPCO scheme must construct without rewrite cycles")
}

fn engine_with_rule_off() -> Engine {
    let mut config = Config::default();
    config
        .rules
        .overrides
        .insert(RULE_WIRE_STRING.to_owned(), "off".to_owned());
    Engine::new(
        config,
        vec![Box::new(CapcoRuleSet::new())],
        CapcoScheme::new(),
    )
    .expect("engine constructs with rule severity override")
}

fn diagnostics_for(source: &[u8]) -> Vec<marque_rules::Diagnostic<marque_capco::CapcoScheme>> {
    engine().lint(source).diagnostics.into_iter().collect()
}

fn rule_diags(
    diags: &[marque_rules::Diagnostic<marque_capco::CapcoScheme>],
) -> Vec<&marque_rules::Diagnostic<marque_capco::CapcoScheme>> {
    diags
        .iter()
        .filter(|d| d.rule.predicate_id() == RULE_PREDICATE)
        .collect()
}

fn e073_diags(
    diags: &[marque_rules::Diagnostic<marque_capco::CapcoScheme>],
) -> Vec<&marque_rules::Diagnostic<marque_capco::CapcoScheme>> {
    diags
        .iter()
        .filter(|d| d.rule.predicate_id() == "marking.fgi.invalid-ownership-token")
        .collect()
}

fn citation_contains(
    d: &marque_rules::Diagnostic<marque_capco::CapcoScheme>,
    needle: &str,
) -> bool {
    format!("{}", d.citation).contains(needle)
}

// ---------------------------------------------------------------------------
// Trigger cases — single unregistered shape-admitted token
// ---------------------------------------------------------------------------

#[test]
fn fires_on_xx_ownership_token() {
    // `(S//FGI XX)` — `XX` is shape-admitted (2-byte ASCII upper) but
    // unregistered (not in `TRIGRAPHS`). The rule must surface it.
    //
    // Calibration finding (issue #545 implementation): `XX` is a
    // 2-byte token, so the rule's candidate-finding (gated on
    // 3-letter trigraphs + corpus prior) takes the no-fix branch.
    // The diagnostic surfaces without a `text_correction`. 2-byte
    // codes have a different ambiguity profile than 3-letter
    // trigraphs and would need their own calibration table; the
    // diagnostic is the user-actionable signal in this case.
    let diags = diagnostics_for(b"(S//FGI XX)");
    let hits = rule_diags(&diags);
    assert_eq!(
        hits.len(),
        1,
        "(S//FGI XX) must emit exactly one ownership-trigraph-suggest \
         diagnostic; got diagnostics={:?}",
        diags
            .iter()
            .map(|d| (d.rule.predicate_id(), d.severity))
            .collect::<Vec<_>>(),
    );
    let d = hits[0];
    assert_eq!(d.severity, Severity::Suggest, "default severity must be Suggest");
    assert!(
        citation_contains(d, "§H.7 p122"),
        "rule must cite §H.7 p122; got {:?}",
        d.citation,
    );
    assert!(
        d.text_correction.is_none(),
        "2-byte unregistered token has no calibrated neighbor — \
         the rule emits a no-fix diagnostic. got text_correction={:?}",
        d.text_correction,
    );
}

#[test]
fn fires_on_zzz_ownership_token() {
    // `(S//FGI ZZZ)` — shape-admitted but unregistered. `ZZZ` has no
    // close neighbors in the corpus prior table; the rule emits a
    // no-fix diagnostic in this case.
    let diags = diagnostics_for(b"(S//FGI ZZZ)");
    let hits = rule_diags(&diags);
    assert_eq!(
        hits.len(),
        1,
        "(S//FGI ZZZ) must emit exactly one diagnostic; got diagnostics={:?}",
        diags
            .iter()
            .map(|d| (d.rule.predicate_id(), d.severity))
            .collect::<Vec<_>>(),
    );
    let d = hits[0];
    assert_eq!(d.severity, Severity::Suggest);
    // ZZZ has no close-edit-distance neighbor with a high corpus
    // log-prior delta — the no-fix template fires. Pin both
    // (a) the diagnostic surfaces and (b) the no-fix shape.
    assert!(
        d.text_correction.is_none(),
        "ZZZ has no corpus neighbor within margin/edit-distance; \
         the rule emits a no-fix diagnostic. got text_correction={:?}",
        d.text_correction,
    );
}

// ---------------------------------------------------------------------------
// Silent cases — registered ownership tokens
// ---------------------------------------------------------------------------

#[test]
fn does_not_fire_on_usa() {
    // `(S//FGI USA)` — sovereign trigraph, registered. Silent.
    let diags = diagnostics_for(b"(S//FGI USA)");
    assert!(
        rule_diags(&diags).is_empty(),
        "registered USA trigraph must not fire ownership-trigraph-suggest; \
         got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.predicate_id(), d.severity))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn does_not_fire_on_nato() {
    // `(S//FGI NATO)` — the §H.7 alliance ownership tetragraph,
    // registered in `TRIGRAPHS`. Silent.
    let diags = diagnostics_for(b"(S//FGI NATO)");
    assert!(
        rule_diags(&diags).is_empty(),
        "registered NATO tetragraph must not fire ownership-trigraph-suggest; \
         got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.predicate_id(), d.severity))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn does_not_fire_on_eu() {
    // `(S//FGI EU)` — the 2-byte exception per Council Decision
    // 2013/488/EU, registered in `TRIGRAPHS`. Silent.
    let diags = diagnostics_for(b"(S//FGI EU)");
    assert!(
        rule_diags(&diags).is_empty(),
        "registered EU code must not fire ownership-trigraph-suggest; \
         got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.predicate_id(), d.severity))
            .collect::<Vec<_>>(),
    );
}

// ---------------------------------------------------------------------------
// Banner form — `SECRET//FGI XX//NOFORN`
// ---------------------------------------------------------------------------

#[test]
fn fires_on_banner_form_with_unregistered_token() {
    // Banner-form FGI markers flow through the same
    // `parse_fgi_marker_with_spans` path; the rule must fire
    // equivalently on `SECRET//FGI XX//NOFORN`.
    let diags = diagnostics_for(b"SECRET//FGI XX//NOFORN");
    let hits = rule_diags(&diags);
    assert_eq!(
        hits.len(),
        1,
        "banner form `SECRET//FGI XX//NOFORN` must emit exactly one \
         ownership-trigraph-suggest diagnostic on `XX`; got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.predicate_id(), d.severity))
            .collect::<Vec<_>>(),
    );
    assert_eq!(hits[0].severity, Severity::Suggest);
}

// ---------------------------------------------------------------------------
// Mixed registered + unregistered list — fires only on the unregistered
// token
// ---------------------------------------------------------------------------

#[test]
fn fires_only_on_unregistered_token_in_mixed_list() {
    // `(S//FGI USA XX)` — USA registered, XX unregistered. Rule
    // must fire on XX only; USA stays clean.
    let diags = diagnostics_for(b"(S//FGI USA XX)");
    let hits = rule_diags(&diags);
    assert_eq!(
        hits.len(),
        1,
        "mixed registered/unregistered list must emit exactly one \
         diagnostic (on XX); got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.predicate_id(), d.severity))
            .collect::<Vec<_>>(),
    );
    assert_eq!(hits[0].severity, Severity::Suggest);
}

#[test]
fn fires_per_unregistered_token_in_multi_invalid_list() {
    // `(S//FGI XX YY)` — both unregistered. Rule fires twice with
    // distinct spans. Each diagnostic anchors at its own token.
    let diags = diagnostics_for(b"(S//FGI XX YY)");
    let hits = rule_diags(&diags);
    assert_eq!(
        hits.len(),
        2,
        "two unregistered tokens must emit two diagnostics; got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.predicate_id(), d.severity))
            .collect::<Vec<_>>(),
    );
    assert_ne!(
        (hits[0].span.start, hits[0].span.end),
        (hits[1].span.start, hits[1].span.end),
        "each diagnostic must anchor at its own token span, not the \
         whole marker: got {:?} and {:?}",
        (hits[0].span.start, hits[0].span.end),
        (hits[1].span.start, hits[1].span.end),
    );
}

// ---------------------------------------------------------------------------
// Span precision — anchor exactly on the offending token
// ---------------------------------------------------------------------------

#[test]
fn diagnostic_span_anchors_on_unregistered_token_bytes() {
    // `(S//FGI XX)` — verify the diagnostic span covers exactly the
    // `XX` bytes, not the whole `FGI XX` block. The source byte
    // layout is `(S//FGI XX)`:
    //   index: 0123456789
    //   chars: ( S / / F G I   X X )
    //   `XX` lives at byte offsets 8..10.
    let source = b"(S//FGI XX)";
    let diags = diagnostics_for(source);
    let hits = rule_diags(&diags);
    assert_eq!(hits.len(), 1);
    let d = hits[0];
    assert_eq!(
        d.span.start, 8,
        "span must start at the byte offset of `XX` (8); got {}",
        d.span.start,
    );
    assert_eq!(
        d.span.end, 10,
        "span must end at the byte offset after `XX` (10); got {}",
        d.span.end,
    );
    // Sanity-check the bytes the span covers, in case future scanner
    // changes shift candidate-extraction offsets.
    let span_bytes = &source[d.span.start..d.span.end];
    assert_eq!(
        span_bytes, b"XX",
        "span bytes must equal `XX`; got {:?}",
        std::str::from_utf8(span_bytes).unwrap_or("<non-utf8>"),
    );
}

// ---------------------------------------------------------------------------
// Severity::Off gate (Constitution V Principle V / FR-008)
// ---------------------------------------------------------------------------

#[test]
fn severity_off_suppresses_all_diagnostics() {
    // Issue #672 / engine `Severity::Off` is a non-firing state per
    // Constitution V Principle V. Setting
    // `[rules] "capco:portion.fgi.ownership-trigraph-suggest" = "off"`
    // must suppress every diagnostic this rule would otherwise emit.
    let engine = engine_with_rule_off();
    let result = engine.lint(b"(S//FGI XX)");
    let hits: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == RULE_PREDICATE)
        .collect();
    assert!(
        hits.is_empty(),
        "`{RULE_WIRE_STRING} = off` must suppress all diagnostics \
         (FR-008 / Constitution V Principle V); got {hits:?}",
    );
}

// ---------------------------------------------------------------------------
// Disjointness with E073 — shape-rejected vs shape-admitted-unregistered
// ---------------------------------------------------------------------------

#[test]
fn e073_only_fires_on_shape_rejected_token_no_co_fire() {
    // `(S//FGI FVEY)` — FVEY is a distribution-list tetragraph,
    // shape-rejected by `admits_fgi_ownership_token`. The parser
    // fails to construct an `FgiMarker::Acknowledged`, emits a
    // `TokenKind::Unknown` block span, and E073 owns this surface.
    //
    // The new ownership-trigraph-suggest rule reads
    // `attrs.fgi_marker.countries()` (only populated on a successful
    // acknowledged parse) and `TokenKind::FgiOwnershipTrigraph`
    // spans (only emitted on the success path), so it cannot
    // see FVEY at all. Pin the disjoint emission contract.
    let diags = diagnostics_for(b"(S//FGI FVEY)");
    assert!(
        !e073_diags(&diags).is_empty(),
        "FVEY must trigger E073 (shape-reject path); got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.predicate_id(), d.severity))
            .collect::<Vec<_>>(),
    );
    assert!(
        rule_diags(&diags).is_empty(),
        "FVEY (shape-rejected) must NOT trigger ownership-trigraph-suggest \
         (which reads only the success-path attrs); got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.predicate_id(), d.severity))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn ownership_suggest_only_fires_on_shape_admitted_unregistered_no_co_fire() {
    // `(S//FGI XX)` — `XX` is shape-admitted (2-byte ASCII upper).
    // The parser succeeds, emits a `TokenKind::FgiOwnershipTrigraph`
    // span, no `Unknown` span fires. E073 owns the failure path
    // (Unknown spans only) so it cannot see XX. Pin the disjoint
    // emission contract from the other direction.
    let diags = diagnostics_for(b"(S//FGI XX)");
    assert!(
        e073_diags(&diags).is_empty(),
        "XX (shape-admitted-unregistered) must NOT trigger E073 \
         (which reads only the failure-path Unknown spans); got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.predicate_id(), d.severity))
            .collect::<Vec<_>>(),
    );
    assert!(
        !rule_diags(&diags).is_empty(),
        "XX must trigger ownership-trigraph-suggest (success-path \
         registry-check); got {:?}",
        diags
            .iter()
            .map(|d| (d.rule.predicate_id(), d.severity))
            .collect::<Vec<_>>(),
    );
}
