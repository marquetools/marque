// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase 3 — engine lint pipeline integration test.
//!
//! Covers the FR-001/FR-002/FR-003 happy path and the spec edge cases:
//! empty document, whitespace-only, mid-sentence `(S)` body prose, and
//! unknown tokens (FR-012).

use marque_capco::CapcoRuleSet;
use marque_config::Config;
use marque_engine::Engine;

fn engine() -> Engine {
    Engine::new(Config::default(), vec![Box::new(CapcoRuleSet::new())])
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
fn banner_abbreviation_fires_e001() {
    let result = engine().lint(b"TOP SECRET//SI//NF\n");
    assert!(result.diagnostics.iter().any(|d| d.rule.as_str() == "E001"));
}

#[test]
fn unknown_token_inside_marking_fires_e008() {
    let result = engine().lint(b"SECRET//XYZZY//NOFORN\n");
    assert!(
        result.diagnostics.iter().any(|d| d.rule.as_str() == "E008"),
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
    assert!(result.diagnostics.iter().any(|d| d.rule.as_str() == "E005"));
}

#[test]
fn missing_usa_in_rel_to_fires_e002() {
    let result = engine().lint(b"SECRET//REL TO GBR, AUS\n");
    assert!(result.diagnostics.iter().any(|d| d.rule.as_str() == "E002"));
}

// Note: a previous test asserted that `SECRET//FOUO` fires E006 because
// FOUO was treated as deprecated via a FOUO→CUI migration. That migration
// was factually incorrect — FOUO remains valid in CAPCO ISM (see
// DissemControl::FOUO) and CUI is a separate marking system under NARA
// jurisdiction. The migration was removed per Phase E of the
// recursive-lattice plan (docs/plans/2026-04-19-recursive-lattice-and-decoder.md §14).
//
// The "FOUO in a classified banner is a policy violation" case is real
// and is handled today at the rollup layer (PageContext drops FOUO from
// classified banners). A dedicated validation rule for direct author
// input like `SECRET//FOUO` lands in Phase C as a declarative
// `Constraint::Conflicts(FOUO, Classified)` entry.

#[test]
fn x_shorthand_declass_fires_e007() {
    // The deprecated `25X1-` form (with trailing dash) lands as Unknown,
    // and E007 walks Unknown tokens for migration-table hits.
    let result = engine().lint(b"SECRET//25X1-//NOFORN\n");
    assert!(
        result.diagnostics.iter().any(|d| d.rule.as_str() == "E007"),
        "expected E007 on 25X1-, got: {:?}",
        result.diagnostics
    );
}

#[test]
fn diagnostic_carries_citation() {
    let result = engine().lint(b"TOP SECRET//SI//NF\n");
    let e001 = result
        .diagnostics
        .iter()
        .find(|d| d.rule.as_str() == "E001")
        .expect("E001 must fire");
    assert!(
        !e001.citation.is_empty(),
        "FR-003: every diagnostic must carry a citation"
    );
    assert!(e001.citation.contains("CAPCO"));
}

#[test]
fn diagnostic_span_is_byte_precise() {
    // FR-002: every diagnostic must carry a span pointing into the original
    // source. Phase 3 replaced the Phase 2 Span::new(0, 0) placeholders.
    let src = b"TOP SECRET//SI//NF\n";
    let result = engine().lint(src);
    let e001 = result
        .diagnostics
        .iter()
        .find(|d| d.rule.as_str() == "E001")
        .expect("E001 must fire");
    assert!(e001.span.start > 0, "span must not be a placeholder");
    assert!(e001.span.end > e001.span.start);
    // The span must point at the literal "NF" bytes.
    assert_eq!(e001.span.as_str(src).unwrap(), "NF");
}

// ---------------------------------------------------------------------------
// Snapshot test — pin the JSON diagnostic shape against contracts/diagnostic.json
// ---------------------------------------------------------------------------

/// Project a Diagnostic into the contract shape from contracts/diagnostic.json.
/// Phase 3 mirrors this structure here so the snapshot guards both the
/// contract format and the engine's ability to populate it.
fn diagnostic_to_contract_json(d: &marque_rules::Diagnostic) -> serde_json::Value {
    let fix = d.fix.as_ref().map(|f| {
        serde_json::json!({
            "source": format!("{:?}", f.source),
            "replacement": f.replacement.as_ref(),
            "confidence": f.confidence,
            "migration_ref": f.migration_ref,
        })
    });
    serde_json::json!({
        "rule": d.rule.as_str(),
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
fn diagnostic_json_shape_is_stable_e001() {
    // Pin the canonical E001 fixture's JSON shape. A future contract drift —
    // for example, removing `severity` or renaming `confidence` — flips this
    // snapshot loud.
    let result = engine().lint(b"TOP SECRET//SI//NF\n");
    let json: Vec<_> = result
        .diagnostics
        .iter()
        .map(diagnostic_to_contract_json)
        .collect();
    insta::assert_json_snapshot!("e001_diagnostic_json", json);
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
