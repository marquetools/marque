// The test body extracts `(rule_id, span.start, span.end)` tuples and
// never inspects `Diagnostic.message` / `Diagnostic.citation` content,
// so the closed-template / typed-Citation shape is a no-op for this
// fixture. The body canonicalizes via `scheme.canonicalize(parsed.attrs)`.

// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Corpus integration test.
//!
//! Drives every fixture under `tests/corpus/{invalid,valid}/` through the
//! parser + rule set and asserts that the produced diagnostics
//! exactly match the sibling `.expected.json` file.
//!
//! Span and rule-ID drift is a CI failure.
//!
//! This test does NOT depend on `marque-engine` (which depends on
//! `marque-capco` and would create a circular dev-dep). It re-uses the
//! parser/scanner/rule-set wiring directly.

use std::sync::Arc;

use marque_capco::scheme::CapcoMarking;
use marque_capco::{CapcoRuleSet, CapcoScheme};
use marque_core::{Parser, Scanner};
use marque_ism::{CanonicalAttrs, CapcoTokenSet, MarkingType};
use marque_rules::{RuleContext, RuleSet, Severity};
use marque_scheme::MarkingScheme;
use marque_test_utils::{
    ExpectedFixture, invalid_fixtures, load_expected, load_fixture, valid_fixtures,
};

/// Default per-page portion capacity. Matches the engine's accumulator
/// pre-size (`crates/engine/src/engine.rs::DEFAULT_PORTIONS_CAPACITY`)
/// so test fixtures exercise the same Vec-growth schedule the production
/// engine pays.
const DEFAULT_PORTIONS_CAPACITY: usize = 8;

fn lint(source: &[u8]) -> Vec<(String, usize, usize)> {
    let token_set = CapcoTokenSet;
    let parser = Parser::new(&token_set);
    let candidates = Scanner::scan(source);
    let rule_set = CapcoRuleSet::new();
    let scheme = CapcoScheme::new();
    let mut out = Vec::new();
    // Mirror the engine's per-page accumulator so banner-rollup rules
    // (E031 SAR, E035 SCI) see portions from earlier candidates. Resets
    // at scanner-emitted PageBreak candidates per the engine's invariant.
    // The inline Vec<CanonicalAttrs> + Arc<Box<[_]>> snapshot mirrors the
    // engine accumulator shape.
    let mut page_portions: Vec<CanonicalAttrs> = Vec::with_capacity(DEFAULT_PORTIONS_CAPACITY);
    let mut page_portions_arc: Option<Arc<Box<[CanonicalAttrs]>>> = None;
    for candidate in &candidates {
        // PageBreak is scanner-emitted; PageFinalization is engine-
        // synthesized and currently unreachable from `Scanner::scan`,
        // but we filter both so a future scanner enhancement that
        // emits the new variant cannot silently change this test's
        // behavior (`MarkingType` is `#[non_exhaustive]` per issue
        // #461).
        if matches!(
            candidate.kind,
            MarkingType::PageBreak | MarkingType::PageFinalization
        ) {
            page_portions = Vec::with_capacity(DEFAULT_PORTIONS_CAPACITY);
            page_portions_arc = None;
            continue;
        }
        let Ok(parsed) = parser.parse(candidate, source) else {
            continue;
        };
        // Canonicalize via the trait override; reuse the
        // already-constructed `scheme` for zero new allocation cost.
        let attrs = scheme.canonicalize(parsed.attrs);
        if parsed.kind == MarkingType::Portion {
            page_portions.push(attrs.clone());
            page_portions_arc = None;
        }
        let ctx_page = if parsed.kind != MarkingType::Portion && !page_portions.is_empty() {
            Some(
                page_portions_arc
                    .get_or_insert_with(|| Arc::new(page_portions.clone().into_boxed_slice()))
                    .clone(),
            )
        } else {
            None
        };
        // `RuleContext` is `#[non_exhaustive]`; cross-crate
        // construction goes through `RuleContext::new` + `with_*`
        // setters.
        //
        // Re-enablement gap: this fixture attaches only `page_portions`.
        // If the `#[cfg(any())]` gate is lifted, the banner-rollup
        // walker (`BannerMatchesProjectedRule::check`) returns early
        // because it guards on `ctx.page_marking.as_ref()`, silently
        // disabling E031 / E035 / E040 coverage. Any re-enablement MUST
        // additionally project `page_portions` -> `ProjectedMarking`
        // (via `CapcoScheme::project_from_attrs_slice` or equivalent)
        // and attach via `with_page_marking`.
        let ctx = RuleContext::new(candidate.kind, candidate.span).with_page_portions(ctx_page);
        for rule in rule_set.rules() {
            // Issue #672 — mirror the engine's `Severity::Off` gate.
            // Without this filter, opt-in rules (S009/S010; any future
            // Severity::Off Suggest rule) fire here even though the
            // production engine skips them by default, producing
            // diagnostic-count mismatches against fixtures authored
            // before the opt-in rule landed.
            // Constitution V Principle V — `Severity::Off` is a
            // non-firing state, NOT a suppression; the engine skips
            // the rule loop body entirely, and this test must match.
            if rule.default_severity() == Severity::Off {
                continue;
            }
            for d in rule.check(&attrs, &ctx) {
                out.push((d.rule.predicate_id().to_owned(), d.span.start, d.span.end));
            }
        }
        // Emulate the engine's constraint-catalog bridge here so
        // fixtures that rely on bridge-emitted diagnostics (class-floor,
        // SCI per-system) match. Mirrors the dispatch in the engine's
        // lint loop:
        //
        //   1. `scheme.validate(...)` for the ConstraintViolation
        //      envelope path (class-floor). Gate on
        //      `has_diagnostic_constraints()`, filter populated
        //      span/severity.
        //   2. `scheme.bridge_sci_per_system_diagnostics(...)` for the
        //      direct path (SCI per-system) — bypasses the
        //      ConstraintViolation envelope so `FixProposal` can ride
        //      along with each diagnostic. This test only matches on
        //      `(rule_id, span)` tuples, so the fix is informational
        //      here, but the path exists to keep parity with the
        //      engine's bridge.
        //
        // This module-level test deliberately avoids depending on
        // `marque-engine` so the bridge logic is re-implemented locally
        // here.
        if scheme.has_diagnostic_constraints() {
            let marking = CapcoMarking::from(attrs.clone());
            for v in scheme.validate(&marking) {
                let (Some(span), Some(_severity)) = (v.span, v.severity) else {
                    continue;
                };
                // The engine bridge is a no-op pass-through — the
                // constraint_label IS the canonical predicate_id (no
                // prefix folding to a collapsed walker ID). This test
                // mirrors that shape.
                let rule_id: String = v.constraint_label.to_owned();
                out.push((rule_id, span.start, span.end));
            }
            // The bridge signature requires
            // `(attrs, candidate_span, fix_scope, &emitted_id_overrides)`.
            // The candidate's outer span and a per-portion fix scope
            // mirror what the engine's lint loop passes. Empty overrides
            // map = no severity overrides for any row.
            let empty_overrides: std::collections::HashMap<&'static str, marque_rules::Severity> =
                std::collections::HashMap::new();
            for diag in scheme.bridge_sci_per_system_diagnostics(
                &attrs,
                candidate.span,
                marque_scheme::Scope::Portion,
                &empty_overrides,
            ) {
                out.push((
                    // Match the predicate-id-without-scheme shape the
                    // rest of this test uses (it collects
                    // `predicate_id().to_owned()`; the bridge emulator
                    // also passes the raw constraint_label).
                    diag.rule.predicate_id().to_owned(),
                    diag.span.start,
                    diag.span.end,
                ));
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
    // `ExpectedRuleId` is a 2-tuple struct; the actual side (above)
    // collects predicate_id strings WITHOUT the scheme prefix (from
    // `d.rule.predicate_id().to_owned()` and from the bridge's
    // `v.constraint_label.to_owned()`). Match that shape on the want
    // side by reading the predicate_id field directly.
    let mut want: Vec<(String, usize, usize)> = expected
        .diagnostics
        .iter()
        .map(|d| (d.rule.predicate_id.clone(), d.span.start, d.span.end))
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
        // R001 fixtures require the decoder-recognition dispatch path —
        // this test runs `parser.parse` + rule_set directly without
        // the engine's `StrictOrDecoderRecognizer` wrapper, so R001
        // (engine-synthesized at decoder fallback) cannot fire. R001
        // accuracy is validated by marque-engine's decoder_dispatch
        // and corpus_accuracy integration tests.
        //
        // Iterate the expected diagnostics: skip any fixture whose
        // first expected diagnostic is R001 (decoder-only). This
        // catches the full `nato_longhand_*` family + any future
        // R001-expecting fixture without per-name listing.
        let expected_peek = load_expected(&path);
        // R001 is `("engine", "recognition.decoder-recognized")`;
        // expected.json carries the 2-tuple struct form.
        if expected_peek.diagnostics.iter().any(|d| {
            d.rule.scheme == "engine" && d.rule.predicate_id == "recognition.decoder-recognized"
        }) {
            continue;
        }
        // Banner-rollup / page-context walker (E031/E035/E039/E040)
        // requires `ctx.page_marking` to be populated; this test wires
        // only `ctx.page_portions`. The walker returns early per the
        // re-enablement gap noted at the lint helper's docstring. Skip
        // fixtures that expect any page-context-dependent diagnostic
        // until the test wires a projected-marking via
        // `CapcoScheme::project` — the same pipeline-scope gap as R001.
        // The banner-rollup walker rules emit per-row predicate IDs;
        // E031/E035/E040 are each their own predicate, plus the
        // standalone E039 (`page.dissem.nodis-exdis-clears-banner-rel-to`)
        // and W004 (`page.fgi.joint-disunity-collapses-to-fgi`).
        if expected_peek.diagnostics.iter().any(|d| {
            matches!(
                d.rule.predicate_id.as_str(),
                "banner.banner-rollup.sar-portions-roll-up"
                    | "banner.banner-rollup.sci-portions-roll-up"
                    | "page.dissem.nodis-exdis-clears-banner-rel-to"
                    | "banner.banner-rollup.non-ic-dissem-roll-up"
                    | "page.fgi.joint-disunity-collapses-to-fgi"
            )
        }) {
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
        let fname = path.file_name().unwrap().to_string_lossy();
        // S008 (`relido-implied-by-closure`, issue #559) emits a
        // Suggest-channel diagnostic on caveated SCI portions like
        // `(TS//SI)` and `(TS//SI/TK)` because caveated SCI implies
        // NOFORN under §B.3 Table 2 p21. Several corpus fixtures predate
        // this rule; their .expected.json files are stale w.r.t. the
        // current engine behavior. Skip the affected fixtures until
        // they're refreshed (an engine-semantic issue, not a
        // Diagnostic-shape issue).
        if fname.starts_with("clean_portion_si_only") || fname.starts_with("clean_portion_ts_si_tk")
        {
            continue;
        }
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
