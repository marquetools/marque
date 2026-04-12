//! Phase 3 — User Story 1 corpus integration test.
//!
//! Drives every fixture under `tests/corpus/{invalid,valid}/` through the
//! parser + Phase 3 rule set and asserts that the produced diagnostics
//! exactly match the sibling `.expected.json` file.
//!
//! Span and rule-ID drift is a CI failure.
//!
//! This test does NOT depend on `marque-engine` (which depends on
//! `marque-capco` and would create a circular dev-dep). It re-uses the
//! parser/scanner/rule-set wiring directly.

use marque_capco::CapcoRuleSet;
use marque_core::{Parser, Scanner};
use marque_ism::{CapcoTokenSet, MarkingType};
use marque_rules::{RuleContext, RuleSet};
use marque_test_utils::{
    ExpectedFixture, invalid_fixtures, load_expected, load_fixture, valid_fixtures,
};

fn lint(source: &[u8]) -> Vec<(String, usize, usize)> {
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidates = Scanner::scan(source);
    let rule_set = CapcoRuleSet::new();
    let mut out = Vec::new();
    for candidate in &candidates {
        if candidate.kind == MarkingType::PageBreak {
            continue;
        }
        let Ok(parsed) = parser.parse(candidate, source) else {
            continue;
        };
        let ctx = RuleContext {
            marking_type: candidate.kind,
            zone: None,
            position: None,
            page_context: None,
        };
        for rule in rule_set.rules() {
            for d in rule.check(&parsed.attrs, &ctx) {
                out.push((d.rule.as_str().to_owned(), d.span.start, d.span.end));
            }
        }
    }
    // Sort for stable comparison.
    out.sort();
    out
}

fn assert_matches(
    path: &std::path::Path,
    expected: &ExpectedFixture,
    actual: &[(String, usize, usize)],
) {
    let mut want: Vec<(String, usize, usize)> = expected
        .diagnostics
        .iter()
        .map(|d| (d.rule.clone(), d.span.start, d.span.end))
        .collect();
    want.sort();
    assert_eq!(
        actual.len(),
        want.len(),
        "fixture {}: diagnostic count mismatch — expected {}, got {}\n  want: {:?}\n  got:  {:?}",
        path.display(),
        want.len(),
        actual.len(),
        want,
        actual
    );
    for (got, expected) in actual.iter().zip(want.iter()) {
        assert_eq!(
            got,
            expected,
            "fixture {}: diagnostic mismatch — expected {:?}, got {:?}",
            path.display(),
            expected,
            got
        );
    }
}

#[test]
fn invalid_corpus_matches_expected_diagnostics() {
    let fixtures = invalid_fixtures();
    assert!(
        !fixtures.is_empty(),
        "tests/corpus/invalid/ is empty — Phase 3 must populate the corpus"
    );
    for path in fixtures {
        let source = load_fixture(&path);
        let expected = load_expected(&path);
        let actual = lint(&source);
        assert_matches(&path, &expected, &actual);
    }
}

#[test]
fn valid_corpus_produces_no_diagnostics() {
    for path in valid_fixtures() {
        let source = load_fixture(&path);
        let expected = load_expected(&path);
        assert!(
            expected.diagnostics.is_empty(),
            "valid fixture {} must have empty .expected.json",
            path.display()
        );
        let actual = lint(&source);
        assert!(
            actual.is_empty(),
            "valid fixture {} produced unexpected diagnostics: {:?}",
            path.display(),
            actual
        );
    }
}
