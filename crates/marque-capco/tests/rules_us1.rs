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

use std::sync::Arc;

use marque_capco::CapcoRuleSet;
use marque_core::{Parser, Scanner};
use marque_ism::{CapcoTokenSet, MarkingType, PageContext};
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
    // Mirror the engine's PageContext accumulation so banner-rollup rules
    // (E031 SAR, E035 SCI) see portions from earlier candidates. Resets at
    // scanner-emitted PageBreak candidates per the engine's invariant.
    let mut page_context = PageContext::new();
    let mut page_context_arc: Option<Arc<PageContext>> = None;
    for candidate in &candidates {
        if candidate.kind == MarkingType::PageBreak {
            page_context = PageContext::new();
            page_context_arc = None;
            continue;
        }
        let Ok(parsed) = parser.parse(candidate, source) else {
            continue;
        };
        if parsed.kind == MarkingType::Portion {
            page_context.add_portion(parsed.attrs.clone());
            page_context_arc = None;
        }
        let ctx_page = if parsed.kind != MarkingType::Portion && !page_context.is_empty() {
            Some(
                page_context_arc
                    .get_or_insert_with(|| Arc::new(page_context.clone()))
                    .clone(),
            )
        } else {
            None
        };
        let ctx = RuleContext {
            marking_type: candidate.kind,
            zone: None,
            position: None,
            page_context: ctx_page,
            corrections: None,
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
        // C001 fixtures require a corrections config — skip in rule-level tests.
        // C001 accuracy is validated by marque-engine's c001_corrections_map_accuracy test.
        let fname = path.file_name().unwrap().to_string_lossy();
        if fname.starts_with("corrections_map_typo") {
            continue;
        }
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
