use super::*;

#[test]
fn page_banner_span_resets_across_lint_calls() {
    // Two consecutive `engine.lint()` calls: the first source has a
    // banner, the second source does not. The second call's
    // PageFinalization observation MUST see `None`, NOT the first
    // call's leftover banner span. This pins the cross-call reset
    // property — `page_banner_span` is a `lint_inner` stack-local,
    // so cross-call leakage is structurally impossible today, but
    // an explicit test makes the invariant survive future engine
    // refactors that might pull state into `self`.
    use marque_ism::MarkingType;
    let observations = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let rule = ContextRecorderRule {
        observations: std::sync::Arc::clone(&observations),
    };
    let set: Box<dyn RuleSet<CapcoScheme>> = Box::new(RecorderSet(vec![Box::new(rule)]));
    let engine = Engine::with_clock(
        Config::default(),
        vec![set],
        marque_capco::scheme::CapcoScheme::new(),
        Box::new(FixedClock::new(
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    let src_with_banner: &[u8] = b"(SECRET//NF) p1\nSECRET//NOFORN\n";
    let src_no_banner: &[u8] = b"(SECRET//NF) just a portion\n";
    let _ = engine.lint(src_with_banner);
    let _ = engine.lint(src_no_banner);

    let obs = observations.lock().unwrap();
    let page_final_spans: Vec<Option<marque_scheme::Span>> = obs
        .iter()
        .filter(|(kind, _, _)| *kind == MarkingType::PageFinalization)
        .map(|(_, _, span)| *span)
        .collect();
    assert_eq!(
        page_final_spans.len(),
        2,
        "expected 2 PageFinalization fires (one per lint call); got: {obs:?}"
    );
    assert!(
        page_final_spans[0].is_some(),
        "first lint call: banner span MUST be Some (the source has a banner): {:?}",
        page_final_spans[0]
    );
    assert!(
        page_final_spans[1].is_none(),
        "second lint call: banner span MUST be None (the source has no banner; \
             the first call's span MUST NOT leak into the second). Got: {:?}",
        page_final_spans[1]
    );
}

#[test]
fn page_banner_span_resets_across_form_feed() {
    // Two pages: page-1 has a banner; page-2 has only a portion.
    // Verifies the banner-span accumulator resets at the `\f`
    // boundary — page-2's PageFinalization MUST see `None`, NOT
    // page-1's leftover banner span. This is the F.1 reset-
    // semantics gate for issue #663.
    use marque_ism::MarkingType;
    let observations = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let rule = ContextRecorderRule {
        observations: std::sync::Arc::clone(&observations),
    };
    let set: Box<dyn RuleSet<CapcoScheme>> = Box::new(RecorderSet(vec![Box::new(rule)]));
    let engine = Engine::with_clock(
        Config::default(),
        vec![set],
        marque_capco::scheme::CapcoScheme::new(),
        Box::new(FixedClock::new(
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    let src: &[u8] = b"(SECRET//NF) p1\nSECRET//NOFORN\n\x0c(CONFIDENTIAL//NF) p2, no banner.\n";
    let _ = engine.lint(src);

    let obs = observations.lock().unwrap();
    let page_final_spans: Vec<Option<marque_scheme::Span>> = obs
        .iter()
        .filter(|(kind, _, _)| *kind == MarkingType::PageFinalization)
        .map(|(_, _, span)| *span)
        .collect();
    assert_eq!(
        page_final_spans.len(),
        2,
        "expected 2 PageFinalization fires (page-1 \\f boundary + EOD), got: {obs:?}"
    );
    assert!(
        page_final_spans[0].is_some(),
        "page-1 finalization should carry the banner span: {:?}",
        page_final_spans[0]
    );
    assert!(
        page_final_spans[1].is_none(),
        "page-2 finalization MUST see None — the form feed must clear the banner accumulator. \
             Got: {:?}",
        page_final_spans[1]
    );
}

#[test]
fn page_banner_span_distinct_at_both_dispatch_sites() {
    // Issue #680 — multi-page document where BOTH `dispatch_page_finalization`
    // call sites fire AND each page has its own banner. The PageBreak-branch
    // dispatch (page-1 closes at `\f`) and the EOD-flush dispatch (page-2
    // closes at end-of-document) must each surface their own page's banner
    // span, not the other page's and not `None`.
    //
    // This is the behavior-level pin for the `PageFinalizationContext`
    // parameter-bundling refactor: the struct's `banner_span` field is
    // constructed inline at each call site in `lint_inner`, and a future
    // accidental swap (e.g., reading from the wrong stack-local at the
    // EOD site) would land here as a span mismatch rather than a silent
    // semantic regression. The `_resets_across_form_feed` sibling test
    // pins the `\f` reset for a no-banner-on-page-2 case; this test pins
    // the both-pages-have-banners case so a refactor cannot trade one
    // dispatch's correctness for the other's.
    use marque_ism::MarkingType;
    let observations = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let rule = ContextRecorderRule {
        observations: std::sync::Arc::clone(&observations),
    };
    let set: Box<dyn RuleSet<CapcoScheme>> = Box::new(RecorderSet(vec![Box::new(rule)]));
    let engine = Engine::with_clock(
        Config::default(),
        vec![set],
        marque_capco::scheme::CapcoScheme::new(),
        Box::new(FixedClock::new(
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    // Page-1 layout: portion + banner; `\f` separates the pages so the
    // PageBreak branch fires. Page-2 layout: portion + banner; EOD
    // flush fires at `source.len()`. Both banners carry the same text
    // ("SECRET//NOFORN") but at distinct offsets — we assert on offsets,
    // not content, so the test distinguishes the two even when the
    // banner text is identical.
    let page1: &[u8] = b"(SECRET//NF) p1\nSECRET//NOFORN\n";
    let page_break: &[u8] = b"\x0c";
    let page2: &[u8] = b"(SECRET//NF) p2\nSECRET//NOFORN\n";
    let page2_start: usize = page1.len() + page_break.len();
    let mut src = Vec::new();
    src.extend_from_slice(page1);
    src.extend_from_slice(page_break);
    src.extend_from_slice(page2);
    let _ = engine.lint(&src);

    let obs = observations.lock().unwrap();
    let page_final_spans: Vec<marque_scheme::Span> = obs
        .iter()
        .filter(|(kind, _, _)| *kind == MarkingType::PageFinalization)
        .map(|(_, _, span)| span.expect("both pages have banners; span MUST be Some"))
        .collect();
    assert_eq!(
        page_final_spans.len(),
        2,
        "expected 2 PageFinalization fires (PageBreak + EOD); got: {obs:?}"
    );

    // Page-1 banner: "SECRET//NOFORN" lives at offset 16..30 inside
    // `page1` (the portion `(SECRET//NF) p1\n` is 16 bytes; the banner
    // content is 14 bytes; the trailing `\n` is excluded from the span
    // per the scanner contract pinned in
    // `page_banner_span_populated_at_page_finalization`).
    let page1_portion_len: usize = b"(SECRET//NF) p1\n".len();
    let banner_content_len: usize = b"SECRET//NOFORN".len();
    assert_eq!(
        (page_final_spans[0].start, page_final_spans[0].end),
        (page1_portion_len, page1_portion_len + banner_content_len),
        "page-1 (PageBreak dispatch) MUST see page-1's banner span, \
             not page-2's; got={:?}",
        page_final_spans[0]
    );

    // Page-2 banner: same content, but the offsets are shifted by
    // `page2_start` (the byte position where page-2 begins inside the
    // composed source). The EOD dispatch MUST see page-2's banner span,
    // not page-1's.
    let page2_banner_start: usize = page2_start + page1_portion_len;
    assert_eq!(
        (page_final_spans[1].start, page_final_spans[1].end),
        (page2_banner_start, page2_banner_start + banner_content_len),
        "page-2 (EOD dispatch) MUST see page-2's banner span, not \
             page-1's leftover; got={:?}",
        page_final_spans[1]
    );

    // Cross-pin: page-1 and page-2 spans MUST NOT be equal. A future
    // refactor that accidentally reads from a single shared local at
    // both dispatch sites would trip here even if both spans happened
    // to be `Some`.
    assert_ne!(
        page_final_spans[0], page_final_spans[1],
        "page-1 and page-2 banner spans MUST differ — distinct byte ranges",
    );
}

#[test]
fn parsed_markings_cache_persists_across_page_breaks() {
    // CA-1 guard: page-break handling resets the per-page projection
    // accumulators, but must NOT reset the per-document
    // `parsed_markings` cache used by fix synthesis.
    struct ParsedCacheIntentRule;
    impl Rule<CapcoScheme> for ParsedCacheIntentRule {
        fn id(&self) -> RuleId {
            RuleId::new("test", "synthetic.parsed-cache-test")
        }
        fn name(&self) -> &'static str {
            "parsed-cache-test"
        }
        fn default_severity(&self) -> Severity {
            Severity::Fix
        }
        fn check(
            &self,
            _attrs: &CanonicalAttrs,
            ctx: &RuleContext<'_, CapcoScheme>,
        ) -> Vec<Diagnostic<CapcoScheme>> {
            if ctx.marking_type != marque_ism::MarkingType::Portion {
                return vec![];
            }
            vec![Diagnostic::with_fix_at_span(
                self.id(),
                self.default_severity(),
                ctx.candidate_span,
                ctx.candidate_span,
                stub_message(),
                stub_citation(),
                FixIntent {
                    replacement: ReplacementIntent::Recanonicalize {
                        scope: RecanonScope::Portion,
                        prior: None,
                    },
                    confidence: Recognition::strict(),
                    feature_ids: SmallVec::new(),
                    message: Message::new(
                        // Test-fixture FixIntent.message must agree with
                        // the Diagnostic-side `stub_message()` template
                        // (`UnrecognizedToken`) so the audit-record
                        // contract `Diagnostic.message.template ==
                        // AppliedFix.message.template` (issue #709)
                        // holds.
                        MessageTemplate::UnrecognizedToken,
                        MessageArgs::default(),
                    ),
                    source: FixSource::BuiltinRule,
                    migration_ref: None,
                },
            )]
        }
    }

    let set: Box<dyn RuleSet<CapcoScheme>> =
        Box::new(RecorderSet(vec![Box::new(ParsedCacheIntentRule)]));
    let engine = Engine::with_clock(
        Config::default(),
        vec![set],
        marque_capco::scheme::CapcoScheme::new(),
        Box::new(FixedClock::new(
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
    .with_strict_recognizer();

    // Two portions split by a form-feed page break. The test rule
    // emits a FixIntent on each portion, so both candidates should
    // populate `parsed_markings`.
    let src = b"(S)\n\x0c(S)\n";
    let (lint, parsed_markings) = engine.lint_with_options_internal(src, &LintOptions::default());

    assert!(!lint.truncated, "test fixture should not hit lint deadline");
    assert_eq!(
        parsed_markings.len(),
        2,
        "parsed_markings must retain entries from both pages; a page-break reset here would drop the first page entry"
    );
    assert!(
        parsed_markings[0].0.start < parsed_markings[1].0.start,
        "cache order must stay scanner-order sorted by Span.start"
    );
}

// Fix-ordering tiebreaker — same span, different rule IDs.
// The sort is (span.end DESC, span.start DESC, rule_id ASC, replacement ASC).
// When two fixes target the exact same span, rule_id ASC breaks the tie,
// and C-1 drops the second (overlapping) fix.
#[test]
fn fr016_same_span_different_rule_ids_picks_lower_rule_id() {
    // Two proposals for span 0..6 with different rule IDs.
    // "C001" < "E001" lexicographically, so C001 is kept and E001 dropped.
    let engine = engine_with(vec![
        proposal("E001", 0, 6, "BB"),
        proposal("C001", 0, 6, "AA"),
    ]);
    let result = engine.fix(TEST_SRC, FixMode::Apply);
    // Text-correction fixes flow through `AuditLine::TextCorrection`
    // post-cutover.
    let text_corrections = applied_text_corrections(&result);
    assert_eq!(text_corrections.len(), 1);
    // Stub rule: `proposal("C001", ...)` →
    // `RuleId::new("test", "C001")`; predicate_id is the raw input.
    assert_eq!(text_corrections[0].rule.predicate_id(), "C001");
    assert_eq!(text_corrections[0].replacement.as_str(), "AA");
}

// Fix-ordering tiebreaker — same span, same rule ID, different replacements.
#[test]
fn fr016_same_span_same_rule_picks_lower_replacement() {
    let engine = engine_with(vec![
        proposal("E001", 0, 6, "ZZZ"),
        proposal("E001", 0, 6, "AAA"),
    ]);
    let result = engine.fix(TEST_SRC, FixMode::Apply);
    let text_corrections = applied_text_corrections(&result);
    assert_eq!(text_corrections.len(), 1);
    assert_eq!(text_corrections[0].replacement.as_str(), "AAA");
}

// -----------------------------------------------------------------------
// Per-emitted-id severity-override propagation
// -----------------------------------------------------------------------
//
// The walker collapse changed the engine's configured-severity override
// to key on each emitted diagnostic's `rule` ID (`d.rule.as_str()`)
// instead of the registered rule's `id()`. The byte-equivalence claim
// for non-walker rules holds when each rule's `default_severity()`
// matches what `check()` emits — true for every existing CAPCO rule
// by convention. These tests pin the post-change correctness of the
// resolution path against a real `CapcoRuleSet`-driven engine, so a
// future regression that quietly stops honoring per-emitted-id
// overrides is caught at the engine layer (not only at the
// walker-specific test surface).

/// Triggers the SAR row of `BannerMatchesProjectedRule` (E031): a
/// portion introduces SAR-CD; the banner has only SAR-BP. The
/// walker emits one diagnostic with `Diagnostic.rule == "E031"`.
/// Same fixture shape as the `crates/capco/tests/banner_rollup_walker.rs`
/// behavior tests so a baseline drift on this string is caught here
/// too.
const SAR_BANNER_MISSING_PROGRAM: &[u8] =
    b"(S//SAR-BP//NF)\n(S//SAR-CD//NF)\nSECRET//SAR-BP//NOFORN";

/// Triggers the SCI row of `BannerMatchesProjectedRule` (E035): a
/// portion carries SI-G; the banner has bare SI. §H.4 enforces
/// hierarchy roll-up (no §H.5-style optional carve-out), so the
/// walker emits one diagnostic with `Diagnostic.rule == "E035"`.
const SCI_BANNER_MISSING_COMPARTMENT: &[u8] = b"(TS//SI-G//NF)\nTOP SECRET//SI//NOFORN";

fn capco_engine_with_overrides(pairs: &[(&str, &str)]) -> Engine {
    let mut config = Config::default();
    for (k, v) in pairs {
        config
            .rules
            .overrides
            .insert((*k).to_owned(), (*v).to_owned());
    }
    Engine::new(
        config,
        vec![Box::new(marque_capco::CapcoRuleSet::new())],
        marque_capco::scheme::CapcoScheme::new(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

#[test]
fn lint_propagates_warn_override_to_walker_emitted_e031_diagnostic() {
    // E031 is emitted by `BannerMatchesProjectedRule`'s SAR catalog
    // row. The walker registers under the bookkeeping ID `E031` and
    // emits diagnostics with the per-row ID `E031`. With
    // `E031 = "warn"` configured, the engine's per-emitted-id
    // override path must rewrite the diagnostic's severity from
    // its emitted value (Fix → demoted to Suggest by the post-pass)
    // to Warn.
    //
    // A future regression that quietly re-keys the override on the
    // registered rule's `id()` would still pass for non-walker rules
    // (where registered ID equals emitted ID) but would either lose the
    // per-row override or silently apply the walker's
    // `default_severity()` to the SCI / FGI roll-up rows — both of which
    // are the failure modes this test exists to prevent.
    //
    // Users type the wire-string config key
    // `"capco:banner.banner-rollup.sar-portions-roll-up"`. The
    // diagnostic's predicate id is the SAR roll-up tuple.
    let engine =
        capco_engine_with_overrides(&[("capco:banner.banner-rollup.sar-portions-roll-up", "warn")]);
    let diagnostics = engine.lint(SAR_BANNER_MISSING_PROGRAM).diagnostics;

    let e031: Vec<&Diagnostic<CapcoScheme>> = diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "banner.banner-rollup.sar-portions-roll-up")
        .collect();
    assert_eq!(
        e031.len(),
        1,
        "exactly one SAR-roll-up diagnostic; got {} from full diag list: \
             {diagnostics:?}",
        e031.len(),
    );
    assert_eq!(
        e031[0].severity,
        Severity::Warn,
        "config `capco:banner.banner-rollup.sar-portions-roll-up = \"warn\"` must propagate to the walker-\
             emitted SAR-roll-up diagnostic; got severity {:?}",
        e031[0].severity,
    );
}

#[test]
fn lint_propagates_warn_override_to_walker_emitted_e035_diagnostic() {
    // Parallel test for the SCI roll-up row of the walker. That row is
    // NOT a registered rule ID (the walker registers under the SAR
    // roll-up row only); a configured override on it can therefore ONLY
    // take effect through the per-emitted-id override path. An engine
    // that keyed overrides on the registered rule's `id()` would never
    // see the SCI row and would apply the walker's `default_severity()`
    // (Error) to the diagnostic. The per-emitted-id path keys on
    // `d.rule.as_str()`, finds the override, and rewrites to Warn.
    //
    // Users type the wire-string config key
    // `"capco:banner.banner-rollup.sci-portions-roll-up"`.
    let engine =
        capco_engine_with_overrides(&[("capco:banner.banner-rollup.sci-portions-roll-up", "warn")]);
    let diagnostics = engine.lint(SCI_BANNER_MISSING_COMPARTMENT).diagnostics;

    let e035: Vec<&Diagnostic<CapcoScheme>> = diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "banner.banner-rollup.sci-portions-roll-up")
        .collect();
    assert_eq!(
        e035.len(),
        1,
        "exactly one E035 diagnostic; got {} from full diag list: \
             {diagnostics:?}",
        e035.len(),
    );
    assert_eq!(
        e035[0].severity,
        Severity::Warn,
        "config `E035 = \"warn\"` must propagate to the walker-\
             emitted E035 diagnostic via the per-emitted-id override \
             path; got severity {:?}",
        e035[0].severity,
    );
}

#[test]
fn lint_off_override_skips_non_walker_rule_via_fast_path() {
    // Non-walker rule fast path: a rule with empty
    // `additional_emitted_ids()` (i.e., every CAPCO rule except
    // `BannerMatchesProjectedRule`) emits diagnostics only under
    // its registered ID. Configuring that ID to `Off` must skip
    // the rule's `check()` body before invocation — the engine's
    // pre-check fast-path skip.
    //
    // This exercises the fast-path skip on the missing-USA-trigraph
    // rule, a non-walker rule that fires deterministically on
    // `SECRET//REL TO GBR`. With the rule configured `off`, the engine
    // must produce zero diagnostics from it via the fast-path skip.
    // Users type the wire-string config key
    // `"capco:portion.dissem.rel-to-missing-usa"`.
    let engine = capco_engine_with_overrides(&[("capco:portion.dissem.rel-to-missing-usa", "off")]);
    let diagnostics = engine.lint(b"SECRET//REL TO GBR").diagnostics;
    let e002: Vec<&Diagnostic<CapcoScheme>> = diagnostics
        .iter()
        .filter(|d| d.rule.predicate_id() == "portion.dissem.rel-to-missing-usa")
        .collect();
    assert!(
        e002.is_empty(),
        "config `E002 = \"off\"` must produce zero E002 \
             diagnostics via the fast-path pre-check skip; got: \
             {e002:?} (full diag list: {diagnostics:?})",
    );

    // Sanity check: without the Off override, E002 fires on the
    // same input.
    let engine_default = capco_engine_with_overrides(&[]);
    let baseline = engine_default.lint(b"SECRET//REL TO GBR").diagnostics;
    let baseline_e002: Vec<&Diagnostic<CapcoScheme>> = baseline
        .iter()
        .filter(|d| d.rule.predicate_id() == "portion.dissem.rel-to-missing-usa")
        .collect();
    assert!(
        !baseline_e002.is_empty(),
        "fixture sanity check: without Off override, E002 must \
             fire on `SECRET//REL TO GBR`; got: {baseline:?}",
    );
}

// -----------------------------------------------------------------------
// `build_severity_tables` — construction-time severity hoist
// -----------------------------------------------------------------------
//
// These tests pin the population semantics of the two pre-resolved
// tables that drive the lint hot-loop's Sites A/B/C/D:
//
//   - `fast_path_severities` — indexed by (set_idx, rule_idx),
//     resolves to `default_severity` when no override exists.
//   - `emitted_id_overrides` — sparse, only populated when an
//     override is present AND parses to a valid severity.
//
// Walker rules (those with non-empty `additional_emitted_ids()`)
// get a `fast_path_severities` entry too (Site A's guard means
// it's read-but-unused for walkers), but catalog-ID overrides
// (e.g., `E035` on `BannerMatchesProjectedRule`) only ever land
// in `emitted_id_overrides` — they do NOT affect the walker
// rule's `fast_path_severities` entry.

#[test]
fn build_severity_tables_empty_overrides_returns_defaults() {
    // No overrides — every rule's fast-path entry must equal its
    // `default_severity()` and `emitted_id_overrides` must be
    // empty. This pins the "absence preserves default" semantics
    // that Site A's `unwrap_or(rule.default_severity())` arm
    // relied on pre-hoist.
    let engine = capco_engine_with_overrides(&[]);
    assert!(
        engine.emitted_id_overrides.is_empty(),
        "no overrides means emitted_id_overrides empty; got: {:?}",
        engine.emitted_id_overrides,
    );
    assert_eq!(
        engine.fast_path_severities.len(),
        engine.rule_sets.len(),
        "fast_path_severities outer len must match rule_sets len",
    );
    for (set_idx, rule_set) in engine.rule_sets.iter().enumerate() {
        let set_table = &engine.fast_path_severities[set_idx];
        assert_eq!(
            set_table.len(),
            rule_set.rules().len(),
            "fast_path_severities[{set_idx}] inner len must match rule count",
        );
        for (rule_idx, rule) in rule_set.rules().iter().enumerate() {
            assert_eq!(
                set_table[rule_idx],
                rule.default_severity(),
                "fast_path_severities[{set_idx}][{rule_idx}] for rule {} \
                     must equal default_severity with no override; got {:?} \
                     vs default {:?}",
                rule.id(),
                set_table[rule_idx],
                rule.default_severity(),
            );
        }
    }
}

#[test]
fn build_severity_tables_registered_id_override_applies() {
    // Single registered-ID override: `E002 = "off"`. E002
    // (`missing-usa-trigraph`) is a non-walker rule registered
    // in `CapcoRuleSet::new()`. The fast-path table entry for
    // E002 must become `Off`; every other rule's entry must
    // stay at its default; `emitted_id_overrides` must contain
    // exactly `{"portion.dissem.rel-to-missing-usa": Off}`.
    let engine = capco_engine_with_overrides(&[("capco:portion.dissem.rel-to-missing-usa", "off")]);

    // Find the (set_idx, rule_idx) for E002 (predicate id
    // `portion.dissem.rel-to-missing-usa`).
    let mut e002_loc: Option<(usize, usize)> = None;
    for (set_idx, rule_set) in engine.rule_sets.iter().enumerate() {
        for (rule_idx, rule) in rule_set.rules().iter().enumerate() {
            if rule.id().predicate_id() == "portion.dissem.rel-to-missing-usa" {
                e002_loc = Some((set_idx, rule_idx));
                break;
            }
        }
    }
    let (set_idx, rule_idx) = e002_loc.expect("E002 must be registered in CapcoRuleSet");

    assert_eq!(
        engine.fast_path_severities[set_idx][rule_idx],
        Severity::Off,
        "fast_path_severities for E002 must reflect the `off` override",
    );

    // Every other registered rule's entry must equal its default.
    for (s, rule_set) in engine.rule_sets.iter().enumerate() {
        for (r, rule) in rule_set.rules().iter().enumerate() {
            if (s, r) == (set_idx, rule_idx) {
                continue;
            }
            assert_eq!(
                engine.fast_path_severities[s][r],
                rule.default_severity(),
                "fast_path_severities[{s}][{r}] for rule {} must \
                     stay at default when only E002 is overridden",
                rule.id(),
            );
        }
    }

    // `emitted_id_overrides` populated with exactly one entry.
    assert_eq!(
        engine.emitted_id_overrides.len(),
        1,
        "exactly one emitted_id_overrides entry; got: {:?}",
        engine.emitted_id_overrides,
    );
    assert_eq!(
        engine
            .emitted_id_overrides
            .get("portion.dissem.rel-to-missing-usa")
            .copied(),
        Some(Severity::Off),
        "emitted_id_overrides[portion.dissem.rel-to-missing-usa] must be Off",
    );
}

#[test]
fn build_severity_tables_catalog_id_override_lands_in_emitted_only() {
    // E035 (`sci-banner-rollup`) is a per-row catalog ID on
    // `BannerMatchesProjectedRule` — emitted by the walker but
    // NOT a registered rule ID (the walker registers under
    // E031). A `[rules] E035 = "warn"` override must:
    //
    //   1. Land in `emitted_id_overrides` so Site B's
    //      per-diagnostic `retain_mut` can rewrite the
    //      diagnostic's severity from its emitted Error to Warn.
    //   2. NOT change the walker's own `fast_path_severities`
    //      entry — Site A only consults that entry when the
    //      rule's `additional_emitted_ids().is_empty()`, which
    //      is false for walker rules, so the entry is unread;
    //      but pinning it here also catches an inverted
    //      population (a future bug that conflated registered
    //      and catalog ID lookups).
    // Wire-string config key
    // `"capco:banner.banner-rollup.sci-portions-roll-up"`.
    let engine =
        capco_engine_with_overrides(&[("capco:banner.banner-rollup.sci-portions-roll-up", "warn")]);

    // Find the walker rule (registered with the SAR roll-up
    // predicate id — the first `BannerCategoryRow.rule_id`).
    let mut walker_loc: Option<(usize, usize, Severity)> = None;
    for (set_idx, rule_set) in engine.rule_sets.iter().enumerate() {
        for (rule_idx, rule) in rule_set.rules().iter().enumerate() {
            if rule.id().predicate_id() == "banner.banner-rollup.sar-portions-roll-up" {
                walker_loc = Some((set_idx, rule_idx, rule.default_severity()));
                break;
            }
        }
    }
    let (set_idx, rule_idx, walker_default) =
        walker_loc.expect("BannerMatchesProjectedRule must be registered");

    assert_eq!(
        engine.fast_path_severities[set_idx][rule_idx], walker_default,
        "fast_path_severities[E031] must stay at the walker's \
             default_severity — an `E035 = warn` override is a \
             catalog-ID override that affects the per-emitted-id \
             path, not the registered-ID fast-path table",
    );

    // SCI-roll-up predicate (NOT SAR-roll-up) must be in
    // `emitted_id_overrides`.
    assert_eq!(
        engine
            .emitted_id_overrides
            .get("banner.banner-rollup.sci-portions-roll-up")
            .copied(),
        Some(Severity::Warn),
        "emitted_id_overrides[banner.banner-rollup.sci-portions-roll-up] must be Warn",
    );
    assert!(
        !engine
            .emitted_id_overrides
            .contains_key("banner.banner-rollup.sar-portions-roll-up"),
        "the override targets the SCI roll-up; the SAR roll-up (walker's registered id) must NOT appear in emitted_id_overrides",
    );
    assert_eq!(
        engine.emitted_id_overrides.len(),
        1,
        "exactly one emitted_id_overrides entry; got: {:?}",
        engine.emitted_id_overrides,
    );
}

#[test]
fn build_severity_tables_skips_unparsable_severity() {
    // The canonicalizer accepts arbitrary severity strings (it
    // only validates the rule-key side), so a malformed
    // severity like `"borked"` survives to
    // `build_severity_tables`. The pre-hoist code used
    // `.and_then(parse_config)` which returned `None` on a
    // malformed string and fell through to
    // `unwrap_or(default_severity)`. Preserve that exactly: the
    // missing-USA-trigraph rule's fast-path entry stays at its default
    // and `emitted_id_overrides` does NOT contain its predicate id.
    // Wire-string config key.
    let engine =
        capco_engine_with_overrides(&[("capco:portion.dissem.rel-to-missing-usa", "borked")]);

    // Find E002's location.
    let mut e002_loc: Option<(usize, usize, Severity)> = None;
    for (set_idx, rule_set) in engine.rule_sets.iter().enumerate() {
        for (rule_idx, rule) in rule_set.rules().iter().enumerate() {
            if rule.id().predicate_id() == "portion.dissem.rel-to-missing-usa" {
                e002_loc = Some((set_idx, rule_idx, rule.default_severity()));
                break;
            }
        }
    }
    let (set_idx, rule_idx, e002_default) =
        e002_loc.expect("E002 must be registered in CapcoRuleSet");

    assert_eq!(
        engine.fast_path_severities[set_idx][rule_idx], e002_default,
        "unparseable severity must fall through to default — \
             fast_path_severities[E002] expected {:?}, got {:?}",
        e002_default, engine.fast_path_severities[set_idx][rule_idx],
    );
    assert!(
        !engine
            .emitted_id_overrides
            .contains_key("portion.dissem.rel-to-missing-usa"),
        "unparseable severity must NOT populate \
             emitted_id_overrides; got: {:?}",
        engine.emitted_id_overrides,
    );
}

// -----------------------------------------------------------------------
// Task #49 — rule-alias canonicalization + fail-loud on unknown keys
// -----------------------------------------------------------------------
