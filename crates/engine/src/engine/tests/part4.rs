use super::*;

#[test]
fn pass1_localized_fixintent_dryrun_records_applied_without_mutating_source() {
    // Companion to the Apply test above: in DryRun mode the
    // engine MUST return the original source unmodified AND
    // still surface the stub Localized fix as an applied record
    // with `dry_run = true`. Locks the DryRun branch of
    // `apply_kept_fixes` (the second arm of the inner
    // `match self.mode`) which the Apply test does not reach.

    struct LocalizedFixIntentStub;
    impl Rule<CapcoScheme> for LocalizedFixIntentStub {
        fn id(&self) -> RuleId {
            // Test-fixture synthetic id in `"test"` scheme.
            RuleId::new("test", "synthetic.e898-fixture")
        }
        fn name(&self) -> &'static str {
            "stub-localized-fixintent-dryrun"
        }
        fn default_severity(&self) -> Severity {
            Severity::Fix
        }
        fn phase(&self) -> marque_rules::Phase {
            marque_rules::Phase::Localized
        }
        fn check(
            &self,
            _attrs: &CanonicalAttrs,
            ctx: &RuleContext,
        ) -> Vec<Diagnostic<CapcoScheme>> {
            let intent = FixIntent::<CapcoScheme> {
                replacement: ReplacementIntent::Recanonicalize {
                    scope: RecanonScope::Portion,
                },
                confidence: marque_rules::Confidence::strict(1.0),
                feature_ids: SmallVec::new(),
                message: Message::new(
                    // Test-fixture FixIntent.message must agree with the
                    // Diagnostic-side `stub_message()` template
                    // (`UnrecognizedToken`) so the audit-record contract
                    // `Diagnostic.message.template == AppliedFix.message.template`
                    // (issue #709) holds.
                    MessageTemplate::UnrecognizedToken,
                    MessageArgs::default(),
                ),
                source: FixSource::BuiltinRule,
                migration_ref: None,
            };
            vec![Diagnostic::with_fix_at_span(
                RuleId::new("test", "synthetic.e898-fixture"),
                Severity::Fix,
                Span::new(8, 14),
                ctx.candidate_span,
                stub_message(),
                stub_citation(),
                intent,
            )]
        }
    }

    let set: Box<dyn RuleSet<CapcoScheme>> =
        Box::new(StubSet(vec![Box::new(LocalizedFixIntentStub)]));
    let engine = Engine::with_clock(
        Config::default(),
        vec![set],
        marque_capco::scheme::CapcoScheme::new(),
        Box::new(FixedClock::new(
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )),
    )
    .expect("engine constructs cleanly");

    let result = engine.fix(TEST_SRC, FixMode::DryRun);
    assert_eq!(
        result.source.expose_secret(),
        TEST_SRC,
        "DryRun must not mutate source"
    );
    assert!(!result.r002_fired);
    let applied_view = applied_fixes(&result);
    let stub_fix = applied_view
        .iter()
        .find(|f| f.rule.predicate_id() == "synthetic.e898-fixture")
        .expect("stub fix should appear in applied list");
    assert!(
        stub_fix.dry_run,
        "DryRun applied fix must have dry_run=true"
    );
}

#[test]
fn apply_kept_fixes_splices_post_buffer_in_dryrun_mode() {
    // Regression lock: `apply_kept_fixes` must not short-circuit in
    // DryRun mode and return the unspliced source as `post_buffer`.
    // If it did, the outer `TwoPassFixer::run` would re-lint that
    // unspliced buffer and dispatch pass-2 against the WRONG
    // coordinate space — same byte input as Apply but a different
    // pass-2 context, breaking the DryRun-as-preview contract.
    //
    // `apply_kept_fixes` always builds the post-splice
    // buffer in BOTH modes; only the OUTER `FixResult.source`
    // differs between Apply and DryRun (the outer layer in
    // `run_pass2_whole_marking` substitutes `self.source.to_vec()`
    // for DryRun). The intermediate pass-1 → pass-2 buffer must
    // be the spliced output regardless of mode so pass-2 dispatch
    // is mode-invariant.
    //
    // This test pins the structural property directly: it calls
    // `apply_kept_fixes` with the same synthesized fixes in both
    // modes and asserts the returned `post_buffer` is the spliced
    // result in both. A future regression to "skip splicing in
    // DryRun" would flip the DryRun assertion to the unspliced
    // source bytes and fail loudly.
    let engine = engine_with(vec![]);
    let source = b"SECRET//NOFORN";

    // Sorted span.end DESC: the synth helper produces
    // one fix at 8..14 replacing "NOFORN" with "REL TO USA".
    let kept_fixes = vec![synth_fix("E001", 8, 14, "REL TO USA")];
    let expected_post_buffer = b"SECRET//REL TO USA".to_vec();

    // Build a dummy LintResult so `apply_kept_fixes`'s deadline-
    // error branch has something to clone (the test never trips
    // the deadline, but the signature requires it).
    let dummy_lint = LintResult::default();

    // Apply mode — establish the spliced baseline.
    let apply_fixer = super::TwoPassFixer {
        engine: &engine,
        source,
        mode: FixMode::Apply,
        threshold: 0.95,
        deadline: None,
    };
    let (apply_post, _, apply_audit_lines) = apply_fixer
        .apply_kept_fixes(source, kept_fixes.clone(), &dummy_lint)
        .expect("apply_kept_fixes succeeds in Apply mode");
    assert_eq!(
        &*apply_post, &expected_post_buffer,
        "Apply mode: post_buffer must be the spliced result",
    );
    for line in &apply_audit_lines {
        if let AuditLine::AppliedFix(f) = line {
            assert!(!f.dry_run, "Apply: dry_run must be false");
        }
    }

    // DryRun mode — the load-bearing R2-3 assertion. Pre-R2-3,
    // this branch returned `source.to_vec()` (unspliced). After
    // the fix, it returns the spliced buffer just like Apply.
    let dry_run_fixer = super::TwoPassFixer {
        engine: &engine,
        source,
        mode: FixMode::DryRun,
        threshold: 0.95,
        deadline: None,
    };
    let (dry_run_post, _, dry_run_audit_lines) = dry_run_fixer
        .apply_kept_fixes(source, kept_fixes, &dummy_lint)
        .expect("apply_kept_fixes succeeds in DryRun mode");
    assert_eq!(
        &*dry_run_post, &expected_post_buffer,
        "DryRun mode: post_buffer must be the spliced result so pass-2 \
             dispatches against the same coordinate space as Apply (R2-3 lock)",
    );
    // Sanity: post_buffer is NOT the unspliced source — that's
    // the exact pre-R2-3 behavior this test exists to detect.
    assert_ne!(
        dry_run_post.as_slice(),
        source,
        "DryRun post_buffer must differ from the unspliced source — \
             returning the unspliced source is the R2-3 regression"
    );
    for line in &dry_run_audit_lines {
        if let AuditLine::AppliedFix(f) = line {
            assert!(f.dry_run, "DryRun: dry_run must be true");
        }
    }

    // Cross-mode parity at the AppliedFix (rule, span) level:
    // the promotion loop is shared, so the applied set must be
    // identical modulo the dry_run flag.
    let apply_applied: Vec<&AppliedFix<CapcoScheme>> = apply_audit_lines
        .iter()
        .filter_map(|l| {
            if let AuditLine::AppliedFix(f) = l {
                Some(f)
            } else {
                None
            }
        })
        .collect();
    let dry_run_applied: Vec<&AppliedFix<CapcoScheme>> = dry_run_audit_lines
        .iter()
        .filter_map(|l| {
            if let AuditLine::AppliedFix(f) = l {
                Some(f)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        apply_applied.len(),
        dry_run_applied.len(),
        "applied list length must match across modes",
    );
    for (a, d) in apply_applied.iter().zip(dry_run_applied.iter()) {
        assert_eq!(a.rule, d.rule);
        assert_eq!(a.span, d.span);
    }
}

// -------------------------------------------------------------------
// Pure-helper unit tests — sort_and_c1_dedup / splice_fixes_forward /
// span_is_within_marking / find_containing_marking
// -------------------------------------------------------------------
//
// The TwoPassFixer methods invoke these via the engine end-to-end
// path. Direct unit tests pin the algebraic contract of each helper
// independently of the dispatcher, so a future change to the
// dispatcher cannot silently break an invariant of the helper.

/// Build a `SynthesizedFix` for unit tests of the splice / sort
/// helpers. `intent` is filled with a no-op Recanonicalize because
/// the helpers only read `rule`/`span`/`replacement`.
fn synth_fix(rule: &'static str, start: usize, end: usize, replacement: &str) -> SynthesizedFix {
    SynthesizedFix {
        // Synthesized-fix helper uses the reserved `"test"` scheme;
        // the `rule` arg is the predicate id (caller picks a unique
        // discriminant per fix).
        rule: RuleId::new("test", rule),
        severity: Severity::Fix,
        span: Span::new(start, end),
        replacement: replacement.into(),
        scope: Scope::Portion,
        intent: FixIntent::<CapcoScheme> {
            replacement: ReplacementIntent::Recanonicalize {
                scope: RecanonScope::Portion,
            },
            confidence: marque_rules::Confidence::strict(1.0),
            feature_ids: SmallVec::new(),
            message: Message::new(
                // Test-fixture FixIntent.message; the splice/sort
                // helpers under test only read `rule`/`span`/
                // `replacement`. `UnrecognizedToken` agrees with the
                // generic test-fixture `stub_message()` for the
                // audit-record contract `Diagnostic.message.template
                // == AppliedFix.message.template` (issue #709).
                MessageTemplate::UnrecognizedToken,
                MessageArgs::default(),
            ),
            source: FixSource::BuiltinRule,
            migration_ref: None,
        },
    }
}

#[test]
fn sort_and_c1_dedup_orders_descending_by_span_end() {
    // Sort key: span.end DESC, then span.start DESC, then rule ASC,
    // then replacement ASC. Use truly disjoint spans so the C-1 dedup
    // walk keeps all of them.
    let synthesized = vec![
        synth_fix("E001", 0, 2, "AA"),   // span 0..2
        synth_fix("E002", 10, 14, "BB"), // span 10..14
        synth_fix("E003", 4, 8, "CC"),   // span 4..8
    ];
    let sorted = super::sort_and_c1_dedup(synthesized);
    // Disjoint spans, so all three survive. Sort →
    // 10..14, 4..8, 0..2.
    assert_eq!(sorted.len(), 3);
    assert_eq!(sorted[0].span.end, 14);
    assert_eq!(sorted[1].span.end, 8);
    assert_eq!(sorted[2].span.end, 2);
}

#[test]
fn sort_and_c1_dedup_drops_overlapping_fixes() {
    // Two overlapping fixes: keep the lex-min winner per C-1.
    // After the sort, span 4..10 comes first (later end),
    // then 0..8 (earlier end) — but 0..8 overlaps with 4..10,
    // so it is dropped.
    let synthesized = vec![
        synth_fix("E001", 0, 8, "AA"), // overlaps 4..10
        synth_fix("E002", 4, 10, "BB"),
    ];
    let kept = super::sort_and_c1_dedup(synthesized);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].span, Span::new(4, 10));
}

#[test]
fn sort_and_c1_dedup_tiebreaks_lex_min_rule_then_replacement() {
    // Same span (1..5): tie-break by rule ASC, then replacement.
    let synthesized = vec![
        synth_fix("E003", 1, 5, "ZZ"),
        synth_fix("E001", 1, 5, "AA"),
        synth_fix("E002", 1, 5, "BB"),
    ];
    let kept = super::sort_and_c1_dedup(synthesized);
    // C-1 dedup: only one fix survives the overlap walk
    // (lex-min winner). With same span across all three, the
    // first to enter the kept set is the sort head.
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].rule.predicate_id(), "E001");
}

#[test]
fn sort_and_c1_dedup_empty_input_returns_empty() {
    let kept = super::sort_and_c1_dedup(Vec::new());
    assert!(kept.is_empty());
}

#[test]
fn splice_fixes_forward_splices_in_reverse_order() {
    // Source: "SECRET//NOFORN" (14 bytes).
    // Two fixes: 0..6 → "AA", 8..14 → "BB".
    // Sort (span.end DESC) → 8..14 first, then 0..6.
    // forward walk via `iter().rev()` yields 0..6 then 8..14.
    let source = b"SECRET//NOFORN";
    let fixes = super::sort_and_c1_dedup(vec![
        synth_fix("E001", 0, 6, "AA"),
        synth_fix("E002", 8, 14, "BB"),
    ]);
    let out = super::splice_fixes_forward(source, &fixes);
    assert_eq!(out, b"AA//BB");
}

#[test]
fn splice_fixes_forward_with_empty_fixes_returns_source_clone() {
    let source = b"SECRET//NOFORN";
    let out = super::splice_fixes_forward(source, &[]);
    assert_eq!(out, source);
}

#[test]
fn splice_fixes_forward_handles_replacement_growth_and_shrink() {
    // 0..6 → "TOP SECRET" (grow), 8..14 → "X" (shrink).
    let source = b"SECRET//NOFORN";
    let fixes = super::sort_and_c1_dedup(vec![
        synth_fix("E001", 0, 6, "TOP SECRET"),
        synth_fix("E002", 8, 14, "X"),
    ]);
    let out = super::splice_fixes_forward(source, &fixes);
    assert_eq!(out, b"TOP SECRET//X");
}

#[test]
fn span_is_within_marking_inclusive_on_both_endpoints() {
    let marking = Span::new(0, 14);
    // Exact match
    assert!(super::span_is_within_marking(Span::new(0, 14), marking));
    // Sub-span
    assert!(super::span_is_within_marking(Span::new(2, 8), marking));
    // Touching start
    assert!(super::span_is_within_marking(Span::new(0, 5), marking));
    // Touching end
    assert!(super::span_is_within_marking(Span::new(9, 14), marking));
    // Out of bounds on either side
    assert!(!super::span_is_within_marking(Span::new(0, 15), marking));
    assert!(!super::span_is_within_marking(Span::new(15, 20), marking));
}

#[test]
fn find_containing_marking_returns_some_when_span_inside() {
    // Construct a synthetic `parsed_markings` directly. Issue #433
    // made the engine's cache populate lazily (only when a
    // diagnostic with `fix.is_some()` is emitted for the
    // candidate), so a fixture that exercises the cache via
    // `lint_with_options_internal` would need a FixIntent-emitting
    // input. The function under test (`find_containing_marking`)
    // keys on `Span` only — building the slice directly tests the
    // lookup semantics without coupling to engine cache policy.
    // Issue #432: cache type swapped from `HashMap<Span, ...>` to
    // `Vec<(Span, ...)>` sorted by `Span.start`; this fixture has
    // one entry so order is trivial.
    let marking_span = Span::new(0, 13);
    let markings: Vec<(Span, marque_capco::CapcoMarking)> = vec![(
        marking_span,
        marque_capco::CapcoMarking::new(CanonicalAttrs::default()),
    )];
    // A sub-span inside marking_span resolves to marking_span.
    let sub = Span::new(marking_span.start, marking_span.start + 1);
    let found = super::find_containing_marking(&markings, sub);
    assert_eq!(found, Some(marking_span));
}

#[test]
fn find_containing_marking_returns_none_when_no_marking_contains() {
    let markings: Vec<(Span, marque_capco::CapcoMarking)> = vec![(
        Span::new(0, 13),
        marque_capco::CapcoMarking::new(CanonicalAttrs::default()),
    )];
    // Way past the inserted marking span — no marking contains it.
    let far = Span::new(10_000, 10_001);
    let found = super::find_containing_marking(&markings, far);
    assert!(found.is_none());
}

#[test]
fn lookup_marking_finds_exact_span() {
    // Pin the binary-search-by-start lookup semantics — exact
    // `Span` match returns the entry; mismatched end returns None.
    // Issue #432.
    let span_a = Span::new(0, 13);
    let span_b = Span::new(20, 35);
    let markings: Vec<(Span, marque_capco::CapcoMarking)> = vec![
        (
            span_a,
            marque_capco::CapcoMarking::new(CanonicalAttrs::default()),
        ),
        (
            span_b,
            marque_capco::CapcoMarking::new(CanonicalAttrs::default()),
        ),
    ];

    assert!(super::lookup_marking(&markings, span_a).is_some());
    assert!(super::lookup_marking(&markings, span_b).is_some());
    // Same start, different end — does NOT match.
    assert!(super::lookup_marking(&markings, Span::new(0, 12)).is_none());
    // Start not in the table — does NOT match.
    assert!(super::lookup_marking(&markings, Span::new(5, 10)).is_none());
    // Between two entries — binary search lands on an adjacent
    // entry, the equality post-check rejects. Pins the case that
    // would silently regress if the search key changed from
    // `s.start` to something else.
    assert!(super::lookup_marking(&markings, Span::new(14, 19)).is_none());
}

#[test]
fn lookup_marking_walks_duplicate_start_run() {
    // The cache's strictly-increasing-start invariant is enforced
    // at the push site by a `debug_assert!`, but `lookup_marking`
    // is defensive against future regressions: if duplicate-start
    // entries ever sneak in, the binary search may land on the
    // wrong same-start entry. The forward+backward walk over the
    // matching-start run finds the target if it exists. This test
    // builds a deliberately-degenerate slice (bypassing the engine
    // push site) to pin the walk's correctness in isolation.
    // Issue #432 + suppressed-comment follow-up on PR #481.
    let target = Span::new(50, 65);
    let markings: Vec<(Span, marque_capco::CapcoMarking)> = vec![
        (
            Span::new(50, 55),
            marque_capco::CapcoMarking::new(CanonicalAttrs::default()),
        ),
        (
            Span::new(50, 60),
            marque_capco::CapcoMarking::new(CanonicalAttrs::default()),
        ),
        (
            target,
            marque_capco::CapcoMarking::new(CanonicalAttrs::default()),
        ),
        (
            Span::new(50, 70),
            marque_capco::CapcoMarking::new(CanonicalAttrs::default()),
        ),
    ];
    // The walk finds the exact target regardless of which entry
    // the binary search initially landed on (criterion would
    // otherwise be non-deterministic across implementations).
    assert!(super::lookup_marking(&markings, target).is_some());
    // A start-matching but end-mismatching probe across the same
    // run still returns None.
    assert!(super::lookup_marking(&markings, Span::new(50, 80)).is_none());
}

// -------------------------------------------------------------------
// TwoPassFixer method-level tests — contributing_pass1_rule_ids /
// assemble_r002_result
// -------------------------------------------------------------------
//
// R002 is unreachable from production CAPCO rules today (no Localized
// rule emits a FixIntent that collapses marking shape), so the
// assemble_r002_result + contributing_pass1_rule_ids paths cannot be
// exercised end-to-end through the public `Engine::fix`. The unit
// tests below construct a `TwoPassFixer` directly and invoke the two
// methods with synthetic inputs to pin the audit-stream invariant
// (R002 result carries pass-0 + pass-1 fixes in order, R002
// diagnostic appended last).
//
// Synthetic `AuditLine::AppliedFix` records here are constructed
// via `__engine_promote` under the Constitution V Principle V
// test-fixture carve-out — the fabricated fixes never flow into a
// real audit stream; they exist to feed the assembler under test.
fn synth_audit_line(rule: &'static str, start: usize, end: usize) -> AuditLine<CapcoScheme> {
    let intent = FixIntent::<CapcoScheme> {
        replacement: ReplacementIntent::Recanonicalize {
            scope: RecanonScope::Portion,
        },
        confidence: marque_rules::Confidence::strict(1.0),
        feature_ids: SmallVec::new(),
        message: Message::new(
            // Synthetic AuditLine fixture; the R002-assembler tests
            // exercising this only read `rule`/`span`. `UnrecognizedToken`
            // pairs with the test-fixture `stub_message()`-style
            // Diagnostic-side template so the audit-record contract
            // `Diagnostic.message.template == AppliedFix.message.template`
            // (issue #709) holds.
            MessageTemplate::UnrecognizedToken,
            MessageArgs::default(),
        ),
        source: FixSource::BuiltinRule,
        migration_ref: None,
    };
    let span = Span::new(start, end);
    // Original-bytes slice for the synthetic record; the bytes
    // hash inline at construction and are never stored. The
    // EngineConstructor-minted Canonical carries the same
    // synthetic payload.
    let original_bytes: &[u8] = b"synth";
    // Test-fixture carve-out per Constitution V Principle V — the
    // `EngineConstructor` mint here mirrors the `__engine_promote`
    // mint below; both feed `synth_audit_line` and never reach a
    // real audit stream.
    let constructor: EngineConstructor<CapcoScheme> =
        EngineConstructor::<CapcoScheme>::__engine_construct();
    let canonical = constructor.build_open_vocab(
        CategoryId::MARKING,
        Box::from("(S)"),
        marque_scheme::Scope::Portion,
    );
    // Test-fixture carve-out per Constitution V Principle V — this
    // call sits inside #[cfg(test)] and feeds the
    // `assemble_r002_result` / `contributing_pass1_rule_ids` unit
    // tests; the fabricated record is never commingled with engine
    // output.
    let applied = AppliedFix::__engine_promote(
        // Synthetic test fixture uses `"test"` scheme; the `rule` arg
        // is the predicate-id discriminant.
        RuleId::new("test", rule),
        Severity::Fix,
        span,
        intent,
        original_bytes,
        canonical,
        UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        None,
        false,
        None,
        engine_promotion_token(),
    );
    AuditLine::AppliedFix(applied)
}

#[test]
fn contributing_pass1_rule_ids_dedupes_and_sorts() {
    let engine = engine_with(vec![]);
    let fixer = super::TwoPassFixer {
        engine: &engine,
        source: TEST_SRC,
        mode: FixMode::Apply,
        threshold: 0.95,
        deadline: None,
    };
    // Three fixes: two duplicates of E006, one of C001. The helper
    // dedupes and sorts ASC. Result: [C001, E006].
    let applied = vec![
        synth_audit_line("E006", 0, 4),
        synth_audit_line("C001", 4, 8),
        synth_audit_line("E006", 8, 12),
    ];
    let out = fixer.contributing_pass1_rule_ids(&applied);
    let ids: Vec<&str> = out.iter().map(|id| id.predicate_id()).collect();
    assert_eq!(ids, vec!["C001", "E006"]);
}

#[test]
fn contributing_pass1_rule_ids_caps_at_inline_capacity_4() {
    let engine = engine_with(vec![]);
    let fixer = super::TwoPassFixer {
        engine: &engine,
        source: TEST_SRC,
        mode: FixMode::Apply,
        threshold: 0.95,
        deadline: None,
    };
    // Five distinct IDs — only the first 4 (after sort) survive
    // the SmallVec inline cap.
    let applied = vec![
        synth_audit_line("E009", 0, 4),
        synth_audit_line("E008", 4, 8),
        synth_audit_line("E007", 8, 12),
        synth_audit_line("E006", 12, 16),
        synth_audit_line("C001", 16, 20),
    ];
    let out = fixer.contributing_pass1_rule_ids(&applied);
    let ids: Vec<&str> = out.iter().map(|id| id.predicate_id()).collect();
    // ASC-sorted, then take(4) → C001, E006, E007, E008.
    assert_eq!(ids, vec!["C001", "E006", "E007", "E008"]);
}

#[test]
fn contributing_pass1_rule_ids_empty_input_returns_empty() {
    let engine = engine_with(vec![]);
    let fixer = super::TwoPassFixer {
        engine: &engine,
        source: TEST_SRC,
        mode: FixMode::Apply,
        threshold: 0.95,
        deadline: None,
    };
    let out = fixer.contributing_pass1_rule_ids(&[]);
    assert!(out.is_empty());
}
