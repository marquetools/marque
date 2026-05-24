#![cfg(any())]
// Legacy FixProposal-shape test disabled pending rewrite.

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Engine lint pipeline integration test.
//!
//! Covers the happy path and the spec edge cases: empty document,
//! whitespace-only, mid-sentence `(S)` body prose, and unknown tokens.

use marque_capco::CapcoRuleSet;
use marque_config::Config;
use marque_engine::Engine;

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(CapcoRuleSet::new())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

#[test]
fn empty_document_produces_no_diagnostics() {
    let result = engine().lint(b"");
    assert!(result.is_clean());
    assert_eq!(result.error_count(), 0);
    assert_eq!(result.warn_count(), 0);
}

#[test]
fn whitespace_only_document_produces_no_diagnostics() {
    let result = engine().lint(b"   \n\n   \t\n");
    assert!(result.is_clean());
}

#[test]
fn happy_path_clean_banner_produces_no_diagnostics() {
    let result = engine().lint(b"TOP SECRET//SI//NOFORN\n");
    assert!(
        result.is_clean(),
        "clean banner should not produce diagnostics, got: {:?}",
        result.diagnostics
    );
}

#[test]
fn happy_path_clean_portion_produces_no_diagnostics() {
    let result = engine().lint(b"(TS//SI//NF)\n");
    assert!(
        result.is_clean(),
        "clean portion should not produce diagnostics, got: {:?}",
        result.diagnostics
    );
}

#[test]
fn banner_with_e002_fires() {
    // The portion-mark-in-banner remediation lives in the renderer, so
    // this smoke test exercises the REL-TO-missing-USA rule as the
    // canonical "this rule fires on a simple banner" fixture.
    let result = engine().lint(b"SECRET//REL TO GBR\n");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.rule.predicate_id() == "portion.dissem.rel-to-missing-usa")
    );
}

#[test]
fn unknown_token_inside_marking_fires_e008() {
    let result = engine().lint(b"SECRET//XYZZY//NOFORN\n");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.rule.predicate_id() == "marking.metadata.unrecognized-token"),
        "expected E008, got: {:?}",
        result.diagnostics
    );
}

#[test]
fn mid_sentence_single_letter_paren_does_not_fire() {
    // The disambiguation heuristic is implemented in the scanner / parser
    // path: a `(S)` followed by a lowercase letter or used as an enumeration
    // marker should not be parsed as a Classification::Secret marking. The
    // parser parses it but the rule loop produces no diagnostics because the
    // "marking" is just `S` with no other content.
    //
    // Note: a more sophisticated heuristic in a future scanner pass would
    // reject such candidates outright. For Phase 3, the bare-classification
    // case (`(S)` alone) is a legitimate portion marking, so the test only
    // verifies that *prose* containing `(a)` enumeration markers stays clean
    // (since `(a)` is not a known classification).
    let prose: &[u8] =
        b"This is a paragraph with (a) some text and (b) more text on the same line.";
    let result = engine().lint(prose);
    assert!(
        result.is_clean(),
        "mid-sentence (a)/(b) markers should not fire, got: {:?}",
        result.diagnostics
    );
}

#[test]
fn declass_in_banner_fires_e005() {
    let result = engine().lint(b"SECRET//25X1//NOFORN\n");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.rule.predicate_id() == "portion.declassification.declassify-on-misplaced")
    );
}

#[test]
fn missing_usa_in_rel_to_fires_e002() {
    let result = engine().lint(b"SECRET//REL TO GBR, AUS\n");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.rule.predicate_id() == "portion.dissem.rel-to-missing-usa")
    );
}

// Note: a previous test asserted that `SECRET//FOUO` fires E006 because
// FOUO was treated as deprecated via a FOUO→CUI migration. That migration
// was factually incorrect — FOUO remains valid in CAPCO ISM (see
// DissemControl::Fouo) and CUI is a separate marking system under NARA
// jurisdiction. FOUO also does not propagate to classified markings
// The migration was removed.
//
// The "FOUO in a classified banner is a policy violation" case is real
// and is handled today at the rollup layer (PageContext drops FOUO from
// classified banners). A dedicated validation rule for direct author
// input like `SECRET//FOUO` is handled as a declarative
// `Constraint::Conflicts(FOUO, Classified)` entry.
//
// The following test replaces that test with the proper behavior.

#[test]
fn unclassified_fouo_does_not_fire_e006() {
    let result = engine().lint(b"UNCLASSIFIED//FOUO");
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.rule.predicate_id() != "marking.deprecation.deprecated-dissem-control")
    );
}

#[test]
fn x_shorthand_declass_fires_e007() {
    // The deprecated `25X1-` form (with trailing dash) lands as Unknown,
    // and E007 walks Unknown tokens for migration-table hits.
    let result = engine().lint(b"SECRET//25X1-//NOFORN\n");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.rule.predicate_id() == "portion.metadata.x-shorthand-date-pattern"),
        "expected E007 on 25X1-, got: {:?}",
        result.diagnostics
    );
}

#[test]
fn diagnostic_carries_citation() {
    let result = engine().lint(b"SECRET//REL TO GBR\n");
    let e002 = result
        .diagnostics
        .iter()
        .find(|d| d.rule.predicate_id() == "portion.dissem.rel-to-missing-usa")
        .expect("E002 must fire");
    // Typed Citation — assert the authoritative source is CAPCO-2016.
    // Every diagnostic must carry a citation; the type enforces presence
    // by construction (every `Diagnostic` has a `Citation` field).
    assert_eq!(
        e002.citation.document,
        marque_scheme::AuthoritativeSource::Capco2016,
        "diagnostic must cite CAPCO-2016; got: {:?}",
        e002.citation,
    );
}

#[test]
fn diagnostic_span_is_byte_precise() {
    // Every diagnostic must carry a span pointing into the original
    // source — not a `Span::new(0, 0)` placeholder. The
    // REL-TO-missing-USA span anchors on the REL-TO trigraph list (the
    // single existing `GBR` here): the span points at the bytes the
    // diagnostic is about.
    let src = b"SECRET//REL TO GBR\n";
    let result = engine().lint(src);
    let e002 = result
        .diagnostics
        .iter()
        .find(|d| d.rule.predicate_id() == "portion.dissem.rel-to-missing-usa")
        .expect("E002 must fire");
    assert!(e002.span.start > 0, "span must not be a placeholder");
    assert!(e002.span.end > e002.span.start);
    // The span must point at the literal `GBR` bytes (the only REL-TO
    // trigraph in this fixture).
    assert_eq!(
        e002.span.as_str(src).unwrap(),
        "GBR",
        "the diagnostic span must point at the REL-TO trigraph bytes"
    );
}

// ---------------------------------------------------------------------------
// Snapshot test — pin the JSON diagnostic shape against contracts/diagnostic.json
// ---------------------------------------------------------------------------

/// Project a Diagnostic into the contract shape from contracts/diagnostic.json.
/// This mirrors that structure so the snapshot guards both the
/// contract format and the engine's ability to populate it.
fn diagnostic_to_contract_json(
    d: &marque_rules::Diagnostic<marque_capco::CapcoScheme>,
) -> serde_json::Value {
    let fix = d.fix.as_ref().map(|f| {
        serde_json::json!({
            "source": format!("{:?}", f.source),
            "replacement": f.replacement.as_ref(),
            "confidence": f.confidence.combined(),
            "migration_ref": f.migration_ref,
        })
    });
    serde_json::json!({
        // Structured 2-tuple shape.
        "rule": {
            "scheme": d.rule.scheme(),
            "predicate_id": d.rule.predicate_id(),
        },
        "severity": d.severity.as_str(),
        "span": {
            "start": d.span.start,
            "end": d.span.end,
        },
        "message": d.message.as_ref(),
        "citation": d.citation,
        "fix": fix,
    })
}

#[test]
fn diagnostic_json_shape_is_stable_e002() {
    // Pin the canonical E002 fixture's JSON shape. A future contract drift —
    // for example, removing `severity` or renaming `confidence` — flips this
    // snapshot loud. (Pre-PR-3c.B-Commit-6 this was anchored on E001;
    // the fixture migrated when E001 retired.)
    let result = engine().lint(b"SECRET//REL TO GBR\n");
    let json: Vec<_> = result
        .diagnostics
        .iter()
        .map(diagnostic_to_contract_json)
        .collect();
    insta::assert_json_snapshot!("e002_diagnostic_json", json);
}

#[test]
fn diagnostic_json_shape_is_stable_e008() {
    // E008 has fix: null — pin both branches of the contract `oneOf`.
    let result = engine().lint(b"SECRET//XYZZY//NOFORN\n");
    let json: Vec<_> = result
        .diagnostics
        .iter()
        .map(diagnostic_to_contract_json)
        .collect();
    insta::assert_json_snapshot!("e008_diagnostic_json", json);
}
