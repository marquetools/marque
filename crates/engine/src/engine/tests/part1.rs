use super::*;

#[test]
fn fix_applies_disjoint_fixes_in_reverse_order() {
    // Two non-overlapping fixes; the sort orders by span.end DESC so the
    // later one is applied first, preserving the earlier span's offsets.
    let engine = engine_with(vec![
        proposal("E001", 0, 6, "AA"),  // "SECRET" → "AA"
        proposal("E002", 8, 14, "BB"), // "NOFORN" → "BB"
    ]);
    let result = engine.fix(TEST_SRC, FixMode::Apply);
    let out = std::str::from_utf8(result.source.expose_secret()).unwrap();
    assert!(out.starts_with("AA//BB"), "got: {out:?}");
    // StubRule emits text-correction diagnostics; the marque-1.0
    // audit stream surfaces them on the `TextCorrection` arm.
    assert_eq!(applied_text_corrections(&result).len(), 2);
}

#[test]
fn overlap_guard_drops_overlapping_fix() {
    // Two fixes whose spans collide. C-1: keep one, drop the other.
    let engine = engine_with(vec![
        proposal("E001", 0, 6, "AA"),
        proposal("E002", 3, 10, "BB"), // overlaps E001
    ]);
    let result = engine.fix(TEST_SRC, FixMode::Apply);
    // Exactly one fix should be applied, the other should remain in
    // `remaining_diagnostics` so callers can see it was not silently
    // dropped.
    let applied = applied_text_corrections(&result);
    assert_eq!(applied.len(), 1, "applied: {applied:?}");
    assert_eq!(
        result.remaining_diagnostics.len(),
        1,
        "remaining: {:?}",
        result.remaining_diagnostics
    );
}

#[test]
fn dry_run_returns_original_source_but_records_applied() {
    let engine = engine_with(vec![proposal("E001", 0, 6, "AA")]);
    let result = engine.fix(TEST_SRC, FixMode::DryRun);
    assert_eq!(
        result.source.expose_secret(),
        TEST_SRC,
        "dry-run must not mutate source"
    );
    let text_corrections = applied_text_corrections(&result);
    assert_eq!(text_corrections.len(), 1);
    assert!(text_corrections[0].dry_run, "dry_run flag must be set");
}

#[test]
fn fix_with_threshold_rejects_nan() {
    let engine = engine_with(vec![]);
    assert!(matches!(
        engine.fix_with_threshold(TEST_SRC, FixMode::Apply, Some(f32::NAN)),
        Err(InvalidThreshold(_))
    ));
}

#[test]
fn fix_with_threshold_rejects_out_of_range() {
    let engine = engine_with(vec![]);
    assert!(matches!(
        engine.fix_with_threshold(TEST_SRC, FixMode::Apply, Some(-0.1)),
        Err(InvalidThreshold(_))
    ));
    assert!(matches!(
        engine.fix_with_threshold(TEST_SRC, FixMode::Apply, Some(1.1)),
        Err(InvalidThreshold(_))
    ));
}

#[test]
fn fix_with_threshold_accepts_boundaries() {
    let engine = engine_with(vec![]);
    assert!(
        engine
            .fix_with_threshold(TEST_SRC, FixMode::Apply, Some(0.0))
            .is_ok()
    );
    assert!(
        engine
            .fix_with_threshold(TEST_SRC, FixMode::Apply, Some(1.0))
            .is_ok()
    );
}

#[test]
fn fixed_clock_yields_deterministic_timestamps() {
    let engine = engine_with(vec![proposal("E001", 0, 6, "AA")]);
    let r1 = engine.fix(TEST_SRC, FixMode::Apply);
    let r2 = engine.fix(TEST_SRC, FixMode::Apply);
    // StubRule emits text-correction diagnostics; timestamps live
    // on `AppliedTextCorrection`.
    assert_eq!(
        applied_text_corrections(&r1)[0].timestamp,
        applied_text_corrections(&r2)[0].timestamp
    );
}

// H-3: fix_with_threshold must reject non-finite overrides in all
// directions, not just NaN. INFINITY and NEG_INFINITY are both caught
// by the range check; this test pins that behavior so a future refactor
// that uses e.g. `is_finite` instead of `contains + is_nan` cannot
// silently regress.
#[test]
fn fix_with_threshold_rejects_infinity() {
    let engine = engine_with(vec![]);
    assert!(matches!(
        engine.fix_with_threshold(TEST_SRC, FixMode::Apply, Some(f32::INFINITY)),
        Err(InvalidThreshold(_))
    ));
    assert!(matches!(
        engine.fix_with_threshold(TEST_SRC, FixMode::Apply, Some(f32::NEG_INFINITY)),
        Err(InvalidThreshold(_))
    ));
}

// M-4: the confidence filter at `f.confidence.combined() >= threshold`
// is on the hot path of Engine::fix. These two tests pin the `>=`
// semantics so a future refactor that flips it to `>` (or vice versa)
// is caught. "Confidence" here is the scalar `Confidence::combined()`
// (= recognition, post-PR-B); `runner_up_ratio` and feature
// contributions are audit-provenance metadata and do not participate
// in the threshold gate.
#[test]
fn confidence_below_default_threshold_is_excluded() {
    // Config::default().confidence_threshold == 0.95. A fix at 0.94
    // must not be applied.
    let engine = engine_with(vec![proposal_with_confidence("E001", 0, 6, "AA", 0.94)]);
    let result = engine.fix(TEST_SRC, FixMode::Apply);
    assert_eq!(applied_fixes(&result).len(), 0);
    // The below-threshold fix is a suggestion — it survives in
    // remaining_diagnostics so the caller can surface it.
    assert_eq!(result.remaining_diagnostics.len(), 1);
}

#[test]
fn lint_rewrites_below_threshold_fix_severity_to_suggest() {
    // Issue #235 / #186 PR-3: the lint post-pass turns a Fix-severity
    // diagnostic carrying a sub-threshold proposal into a Suggest-
    // severity diagnostic, preserving the fix payload so the renderer
    // can show "did you mean?" instead of silently dropping the
    // candidate at the threshold gate.
    let engine = engine_with(vec![proposal_with_confidence("E001", 0, 6, "AA", 0.5)]);
    let lint = engine.lint(TEST_SRC);
    assert_eq!(lint.diagnostics.len(), 1);
    assert_eq!(lint.diagnostics[0].severity, Severity::Suggest);
    assert!(
        lint.diagnostics[0].fix.is_some(),
        "the candidate fix must stay attached so the renderer can surface it"
    );
    assert_eq!(lint.suggest_count(), 1);
    // Confirm the engine still excludes Suggest from auto-apply.
    let fix_result = engine.fix(TEST_SRC, FixMode::Apply);
    assert_eq!(applied_fixes(&fix_result).len(), 0);
}

#[test]
fn lint_does_not_rewrite_at_threshold_boundary() {
    // A fix at exactly the threshold (0.95) must NOT be rewritten
    // — it is auto-apply territory, not Suggest territory. This
    // pins the boundary semantics: the rewrite predicate is
    // strictly less-than, matching the engine's `>= threshold`
    // application gate.
    let engine = engine_with(vec![proposal_with_confidence("E001", 0, 6, "AA", 0.95)]);
    let lint = engine.lint(TEST_SRC);
    assert_eq!(lint.diagnostics.len(), 1);
    assert_eq!(lint.diagnostics[0].severity, Severity::Fix);
}

#[test]
fn lint_post_pass_leaves_fix_severity_with_no_fix_payload_alone() {
    // The post-pass guard order matters: even though `Fix`-severity
    // diagnostics are the only ones eligible for the rewrite, a
    // diagnostic that doesn't carry a `FixProposal` (rare in
    // practice — `Fix`-severity rules normally always attach one
    // — but representable in the type) must be skipped by the
    // `let Some(fix) = d.fix.as_ref() else { continue }` arm and
    // keep its `Fix` severity. This pins the behavior so a future
    // refactor that hoists the threshold check above the fix-
    // presence check (and might rewrite to Suggest unconditionally)
    // is caught.
    struct FixWithoutProposalRule;
    impl Rule<CapcoScheme> for FixWithoutProposalRule {
        fn id(&self) -> RuleId {
            RuleId::new("test", "synthetic.e997-fixture")
        }
        fn name(&self) -> &'static str {
            "stub-fix-no-proposal"
        }
        fn default_severity(&self) -> Severity {
            Severity::Fix
        }
        fn check(
            &self,
            _attrs: &CanonicalAttrs,
            _ctx: &RuleContext,
        ) -> Vec<Diagnostic<CapcoScheme>> {
            vec![Diagnostic::info(
                RuleId::new("test", "synthetic.e997-fixture"),
                Severity::Fix,
                Span::new(0, 6),
                stub_message(),
                stub_citation(),
            )]
        }
    }

    let set: Box<dyn RuleSet<CapcoScheme>> =
        Box::new(StubSet(vec![Box::new(FixWithoutProposalRule)]));
    let engine = Engine::with_clock(
        Config::default(),
        vec![set],
        marque_capco::scheme::CapcoScheme::new(),
        Box::new(FixedClock::new(
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    let lint = engine.lint(TEST_SRC);
    assert_eq!(lint.diagnostics.len(), 1);
    assert_eq!(
        lint.diagnostics[0].severity,
        Severity::Fix,
        "Fix-severity diagnostic with no fix payload must NOT be rewritten to Suggest",
    );
    assert!(lint.diagnostics[0].fix.is_none());
}

#[test]
fn fix_excludes_explicit_suggest_severity_from_auto_apply() {
    // Issue #235 / #186 PR-3: a rule that emits at Severity::Suggest
    // directly with confidence ≥ threshold must STILL be excluded
    // from auto-apply by construction. The Suggest channel is a
    // hard "do not apply" signal regardless of the confidence
    // axis. This is the explicit-Suggest invariant; the StubRule
    // emits Fix-severity by default so we route through a custom
    // rule that emits Suggest directly.
    struct SuggestRule;
    impl Rule<CapcoScheme> for SuggestRule {
        fn id(&self) -> RuleId {
            RuleId::new("test", "synthetic.s999-fixture")
        }
        fn name(&self) -> &'static str {
            "stub-suggest"
        }
        fn default_severity(&self) -> Severity {
            Severity::Suggest
        }
        fn check(
            &self,
            _attrs: &CanonicalAttrs,
            _ctx: &RuleContext,
        ) -> Vec<Diagnostic<CapcoScheme>> {
            let intent = FixIntent::<CapcoScheme> {
                replacement: ReplacementIntent::Recanonicalize {
                    scope: RecanonScope::Portion,
                },
                confidence: marque_rules::Confidence::strict(),
                feature_ids: SmallVec::new(),
                message: Message::new(
                    // Test-fixture FixIntent.message must agree with the
                    // Diagnostic-side template (`stub_message()` =
                    // `UnrecognizedToken`) so the audit-record contract
                    // `Diagnostic.message.template == AppliedFix.message.template`
                    // (issue #709) holds.
                    MessageTemplate::UnrecognizedToken,
                    MessageArgs::default(),
                ),
                source: FixSource::BuiltinRule,
                migration_ref: None,
            };
            vec![Diagnostic::with_fix(
                RuleId::new("test", "synthetic.s999-fixture"),
                Severity::Suggest,
                Span::new(0, 6),
                stub_message(),
                stub_citation(),
                Some(intent),
            )]
        }
    }

    let set: Box<dyn RuleSet<CapcoScheme>> = Box::new(StubSet(vec![Box::new(SuggestRule)]));
    let engine = Engine::with_clock(
        Config::default(),
        vec![set],
        marque_capco::scheme::CapcoScheme::new(),
        Box::new(FixedClock::new(
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    let lint = engine.lint(TEST_SRC);
    assert_eq!(lint.diagnostics.len(), 1);
    // Severity stays Suggest (post-pass leaves explicit Suggest alone).
    assert_eq!(lint.diagnostics[0].severity, Severity::Suggest);
    // Even at confidence 1.0, a Suggest-severity fix must not auto-apply.
    let fix_result = engine.fix(TEST_SRC, FixMode::Apply);
    assert_eq!(
        applied_fixes(&fix_result).len(),
        0,
        "explicit Suggest-severity fix must not auto-apply regardless of confidence"
    );
}

#[test]
fn confidence_at_default_threshold_is_included() {
    // A fix at exactly 0.95 must be applied (inclusive threshold).
    let engine = engine_with(vec![proposal_with_confidence("E001", 0, 6, "AA", 0.95)]);
    let result = engine.fix(TEST_SRC, FixMode::Apply);
    assert_eq!(applied_text_corrections(&result).len(), 1);
}

// M-5: the zero-length-span filter (`!f.span.is_empty()`) in fix_inner
// is what masked the Phase 2 Span::new(0, 0) placeholders from the
// C-1 overlap guard. This test pins that guard explicitly so a future
// refactor that drops the filter is caught.
#[test]
fn zero_length_span_fix_is_filtered_before_sort() {
    let engine = engine_with(vec![proposal("E001", 5, 5, "X")]);
    let result = engine.fix(TEST_SRC, FixMode::Apply);
    assert_eq!(applied_text_corrections(&result).len(), 0);
    // Source unchanged: no splice was attempted.
    assert_eq!(result.source.expose_secret(), TEST_SRC);
}

// L-4: all the other threshold tests go through fix_with_threshold
// (override path). This exercises the Config-supplied path explicitly
// so both branches of `fix_with_threshold_inner`'s threshold selection
// are covered.
#[test]
fn config_supplied_threshold_filters_proposals() {
    let mut config = Config::default();
    config.set_confidence_threshold(0.5).unwrap();
    let engine = engine_with_config(
        config,
        vec![
            proposal_with_confidence("E001", 0, 6, "AA", 0.4), // below
            proposal_with_confidence("E002", 8, 14, "BB", 0.6), // above
        ],
    );
    let result = engine.fix(TEST_SRC, FixMode::Apply);
    // Only the 0.6 fix is applied. StubRule emits text-corrections.
    let text_corrections = applied_text_corrections(&result);
    assert_eq!(text_corrections.len(), 1);
    // Stub rule emits via `proposal("E002", ...)` which constructs
    // `RuleId::new("test", "E002")`; the predicate_id is the raw
    // input string.
    assert_eq!(text_corrections[0].rule.predicate_id(), "E002");
    // The 0.4 fix surfaces as a remaining diagnostic.
    assert_eq!(result.remaining_diagnostics.len(), 1);
}

// Phase 3 Task 2: PageBreak candidates must reset the engine's
// PageContext accumulator. Without this, banner-validation rules on
// the second page would see portions from the first page, producing
// over-restrictive expected aggregates.
#[test]
fn lint_handles_multi_page_document_with_form_feed() {
    let src: &[u8] = b"(SECRET//NOFORN) page 1 body.\nSECRET//NOFORN\n\x0c(CONFIDENTIAL) page 2 body.\nCONFIDENTIAL\n";
    let engine = engine_with(vec![]);
    let result = engine.lint(src);
    // Stub rule with no proposals: clean lint, no panic, no parser
    // error from the page-break candidate (which is filtered before
    // parser.parse is called).
    assert!(result.is_clean());
}

// F.1: per-page accumulator reset semantics are observable.
//
// ContextRecorderRule captures the live `ctx.page_portions` length
// and (issue #663) the `ctx.page_banner_span` every time it's
// invoked. By running the engine over a multi-page document and
// inspecting the captured triples at each PageFinalization fire,
// we prove that the engine resets the per-page accumulators at the
// page break instead of accumulating across pages.
//
// The triple is `(marking_type, page_portions.len(),
// page_banner_span)` — one row per rule invocation. The shape lives
// in a type alias because the nested `Arc<Mutex<Vec<(...)>>>`
// surface trips `clippy::type_complexity` and the recorder is
// shared across four test fixtures.
type RecorderObservation = (marque_ism::MarkingType, usize, Option<marque_scheme::Span>);
type RecorderObservations = std::sync::Arc<std::sync::Mutex<Vec<RecorderObservation>>>;

#[derive(Clone)]
struct ContextRecorderRule {
    observations: RecorderObservations,
}

impl Rule<CapcoScheme> for ContextRecorderRule {
    fn id(&self) -> RuleId {
        RuleId::new("test", "synthetic.record-fixture")
    }
    fn name(&self) -> &'static str {
        "page-portions-recorder"
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    // Issue #306 / PR #674 made `ctx.page_portions` only populated
    // for `Phase::PageFinalization` rules — the main candidate loop
    // now passes `None` unconditionally (see `engine.rs:1378`). The
    // recorder MUST declare PageFinalization so it fires from
    // `dispatch_page_finalization` at scanner-emitted page-break
    // boundaries and EOD, where the accumulator is force-populated
    // and observable. Issue #663 added `ctx.page_banner_span` under
    // the same visibility contract.
    fn phase(&self) -> marque_rules::Phase {
        marque_rules::Phase::PageFinalization
    }
    fn check(&self, _attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        let count = ctx
            .page_portions
            .as_ref()
            .map(|pp| pp.as_ref().len())
            .unwrap_or(0);
        self.observations
            .lock()
            .unwrap()
            .push((ctx.marking_type, count, ctx.page_banner_span));
        vec![]
    }
}

struct RecorderSet(Vec<Box<dyn Rule<CapcoScheme>>>);
impl RuleSet<CapcoScheme> for RecorderSet {
    fn rules(&self) -> &[Box<dyn Rule<CapcoScheme>>] {
        &self.0
    }
    fn schema_version(&self) -> &'static str {
        "TEST"
    }
}

#[test]
fn page_portions_reset_observably_across_form_feed() {
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

    // Two pages, separated by a form feed:
    //   Page 1: one portion + one banner
    //   Page break (\f)
    //   Page 2: one portion + one banner
    //
    // The recorder declares `Phase::PageFinalization`, so it
    // fires from `dispatch_page_finalization` — once at the
    // scanner-emitted page-break boundary (\f) and once at EOD.
    // Each fire observes the accumulated `ctx.page_portions` for
    // the just-completed page; if the form feed correctly resets
    // the accumulator, both fires see exactly 1 portion (NOT 2).
    let src: &[u8] =
        b"(SECRET//NF) p1 text\nSECRET//NOFORN\n\x0c(CONFIDENTIAL//NF) p2\nCONFIDENTIAL//NOFORN\n";
    let _ = engine.lint(src);

    let obs = observations.lock().unwrap();
    // PageFinalization fires carry `MarkingType::PageFinalization`
    // (set by `dispatch_page_finalization`'s synthetic
    // RuleContext at the boundary anchor — see `engine.rs:4763`).
    // Filter to those and check the per-page accumulator size at
    // each boundary.
    let page_final_counts: Vec<usize> = obs
        .iter()
        .filter(|(kind, _, _)| *kind == MarkingType::PageFinalization)
        .map(|(_, count, _)| *count)
        .collect();
    assert_eq!(
        page_final_counts.len(),
        2,
        "expected 2 PageFinalization observations \
             (page-1 form-feed boundary + EOD), got: {obs:?}"
    );
    assert_eq!(
        page_final_counts[0], 1,
        "page-1 finalization should see 1 accumulated portion"
    );
    assert_eq!(
        page_final_counts[1], 1,
        "page-2 finalization (EOD) should see 1 accumulated portion \
             (the page-1 portion must be cleared by the form feed)"
    );
}

#[test]
fn page_portions_lint_starts_fresh_on_each_call() {
    // Calling Engine::lint twice on the same engine must produce a
    // fresh per-page accumulator for the second call — no cross-call
    // accumulation.
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
    let src: &[u8] = b"(SECRET//NF) text\nSECRET//NOFORN\n";
    let _ = engine.lint(src);
    let _ = engine.lint(src);

    let obs = observations.lock().unwrap();
    // Each `engine.lint()` call fires `dispatch_page_finalization`
    // once (at EOD — no `\f` in this source, so no mid-doc
    // boundary). Both calls should see identical observations; if
    // the second call leaked the first call's portion into its
    // accumulator, the second PageFinalization count would be 2.
    let page_final_counts: Vec<usize> = obs
        .iter()
        .filter(|(kind, _, _)| *kind == MarkingType::PageFinalization)
        .map(|(_, count, _)| *count)
        .collect();
    assert_eq!(
        page_final_counts.len(),
        2,
        "two lint calls should produce two PageFinalization observations"
    );
    assert_eq!(page_final_counts, vec![1, 1]);
}

// Issue #663: `ctx.page_banner_span` exposes the most recent banner
// candidate's span to `Phase::PageFinalization` rules and resets at
// every scanner-emitted page-break boundary. The three properties
// verified here are the load-bearing contract for downstream
// S010/E072 RELIDO resolution wire-up (CAPCO §H.8 pp150-156):
//
//   1. `Some(span)` at PageFinalization when the page had a banner.
//   2. The captured span points at the banner candidate's full
//      byte range (start matches the banner's "SECRET" prefix
//      offset; end matches the banner-line terminator).
//   3. The accumulator resets on `\f` (the captured span on a
//      banner-less subsequent page is `None`, NOT the previous
//      page's banner span).
//
// The recorder declares `Phase::PageFinalization` so it fires from
// `dispatch_page_finalization` per the visibility contract on the
// `RuleContext::page_banner_span` field.
#[test]
fn page_banner_span_populated_at_page_finalization() {
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

    // Single page: one portion + one banner + EOD.
    // The banner candidate's span starts at the "SECRET" prefix and
    // ends at the newline-terminator boundary (excluding the `\n`
    // itself — the scanner's banner span ends one-past the last
    // printable byte, not one-past the terminator). The assertion
    // below pins both endpoints to the byte offsets in this
    // fixture.
    let src: &[u8] = b"(SECRET//NF) body text\nSECRET//NOFORN\n";
    let _ = engine.lint(src);

    let obs = observations.lock().unwrap();
    let page_final_obs: Vec<&RecorderObservation> = obs
        .iter()
        .filter(|(kind, _, _)| *kind == MarkingType::PageFinalization)
        .collect();
    assert_eq!(
        page_final_obs.len(),
        1,
        "expected 1 PageFinalization observation at EOD, got: {obs:?}"
    );
    let banner_span = page_final_obs[0]
        .2
        .expect("page_banner_span MUST be Some when the page had a banner");
    // Source layout (byte offsets):
    //   0-21: "(SECRET//NF) body text"   (22 bytes — the portion + body)
    //     22: "\n"                       (portion-terminator)
    //  23-36: "SECRET//NOFORN"           (14 bytes — the banner content)
    //     37: "\n"                       (banner-terminator; EXCLUDED
    //                                     from the banner span — the
    //                                     scanner ends the candidate
    //                                     one-past the last printable
    //                                     byte, NOT one-past the `\n`)
    //     38: end of source              (src.len() == 38)
    //
    // The banner candidate's `Span` is the half-open range
    // `start..end` covering the banner's printable bytes only.
    // Empirically: `start = 23` (first byte of "SECRET"),
    // `end = 37` (the `\n` at byte 37 is the terminator that ends
    // the candidate but is not itself part of the span). If a
    // future scanner pass changes the trailing-newline behavior
    // (either including the `\n` so `end = 38`, or tightening to
    // exclude trailing whitespace), this assertion is the canary
    // — update both halves intentionally.
    assert_eq!(
        (banner_span.start, banner_span.end),
        (23, 37),
        "banner span MUST cover bytes 23..37 (SECRET//NOFORN, excluding the trailing \
             `\\n` at byte 37); got span={banner_span:?} from source: {:?}",
        std::str::from_utf8(src).unwrap_or("<non-utf8>")
    );
}

#[test]
fn page_banner_span_is_none_when_page_has_no_banner() {
    // A page fragment with only a portion (no banner line) at EOD.
    // The PageFinalization fire MUST see `page_banner_span = None`.
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

    let src: &[u8] = b"(SECRET//NF) just a portion, no banner.\n";
    let _ = engine.lint(src);

    let obs = observations.lock().unwrap();
    let page_final_obs: Vec<&RecorderObservation> = obs
        .iter()
        .filter(|(kind, _, _)| *kind == MarkingType::PageFinalization)
        .collect();
    assert_eq!(
        page_final_obs.len(),
        1,
        "expected 1 PageFinalization observation at EOD"
    );
    assert!(
        page_final_obs[0].2.is_none(),
        "page_banner_span MUST be None when the page had no banner; got: {:?}",
        page_final_obs[0].2
    );
}

#[test]
fn page_banner_span_holds_most_recent_on_multi_banner_page() {
    // Pathological-but-legal layout: TWO banner candidates on a
    // single page (header banner before the portion + footer banner
    // after, with no intervening `\f`). The doc-comment on
    // `RuleContext::page_banner_span` defines the field as carrying
    // the MOST RECENT banner span observed on the page; this test
    // pins that semantic so a future refactor that changes the
    // accumulator to "first-seen-wins" (or any other ordering)
    // trips the assertion.
    //
    // S010/E072 wire-up will rely on this: when a user has both a
    // header and footer banner, the rule's fix needs to target the
    // footer (the one most likely to be the user's intended
    // canonical banner — header banners are often legacy
    // artifacts).
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

    // Header banner at offset 0, portion in the middle, footer
    // banner at offset `header.len() + portion.len()` (= 33 with
    // the current literals: 15-byte header + 18-byte portion). The
    // footer banner's start offset is the assertion target; the
    // computation is parameterized off the actual literal lengths
    // so a future fixture edit doesn't desync the comment from the
    // arithmetic.
    let header: &[u8] = b"SECRET//NOFORN\n";
    let portion: &[u8] = b"(SECRET//NF) body\n";
    let footer: &[u8] = b"SECRET//NOFORN\n";
    let footer_start: usize = header.len() + portion.len();
    let mut src = Vec::new();
    src.extend_from_slice(header);
    src.extend_from_slice(portion);
    src.extend_from_slice(footer);
    let _ = engine.lint(&src);

    let obs = observations.lock().unwrap();
    let page_final_obs: Vec<&RecorderObservation> = obs
        .iter()
        .filter(|(kind, _, _)| *kind == MarkingType::PageFinalization)
        .collect();
    assert_eq!(
        page_final_obs.len(),
        1,
        "expected 1 PageFinalization observation at EOD; got: {obs:?}"
    );
    let banner_span = page_final_obs[0]
        .2
        .expect("page_banner_span MUST be Some on a multi-banner page");
    // Footer banner content is "SECRET//NOFORN" (14 bytes); the
    // span excludes the trailing `\n` per the scanner contract
    // pinned in `page_banner_span_populated_at_page_finalization`.
    // So `end = footer_start + 14`.
    let footer_content_len: usize =
            footer.len() - 1 /* trailing \n excluded from span */;
    assert_eq!(
        (banner_span.start, banner_span.end),
        (footer_start, footer_start + footer_content_len),
        "page_banner_span MUST hold the MOST RECENT banner span (footer at \
             {footer_start}..{}, NOT the header at 0..{header_end}); got span={banner_span:?}",
        footer_start + footer_content_len,
        header_end = header.len() - 1
    );
}
