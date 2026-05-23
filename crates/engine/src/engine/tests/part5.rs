use super::*;

#[test]
fn assemble_r002_result_carries_pass0_then_pass1_applied_plus_r002_diag() {
    // The assembler concatenates pass-0 then pass-1 applied (audit
    // stream order D-7.6), filters remaining diagnostics by
    // applied_keys, then appends the R002 diagnostic LAST.
    let engine = engine_with(vec![]);
    let fixer = super::TwoPassFixer {
        engine: &engine,
        source: TEST_SRC,
        mode: FixMode::Apply,
        threshold: 0.95,
        deadline: None,
    };

    let pass0_audit_lines = vec![synth_audit_line("C001", 0, 6)];
    let pass1_audit_lines = vec![synth_audit_line("E006", 8, 12)];
    let pass1 = Pass1Result {
        post_buffer: Zeroizing::new(b"POST-PASS-1-BUFFER".to_vec()),
        audit_lines: pass1_audit_lines,
        applied_keys: HashSet::new(),
    };
    let lint = LintResult {
        diagnostics: Vec::new(),
        truncated: false,
        candidates_processed: 0,
        candidates_total: 0,
        recognized_marking_count: 0,
    };
    let r002 = super::build_r002_diagnostic(
        smallvec::smallvec![RuleId::new(
            "capco",
            "marking.deprecation.deprecated-dissem-control"
        )],
        Span::new(0, 18),
    );
    let result =
        fixer.assemble_r002_result(pass0_audit_lines, Vec::new(), pass1, lint, r002.clone());

    // Order: pass0 (C001) then pass1 (E006). Synth records use
    // the `AuditLine::AppliedFix` arm regardless of rule ID (the
    // test-fixture path doesn't route by rule).
    assert_eq!(result.audit_lines.len(), 2);
    let first_rule = match &result.audit_lines[0] {
        AuditLine::AppliedFix(f) => f.rule.predicate_id(),
        AuditLine::TextCorrection(tc) => tc.rule.predicate_id(),
        _ => "unknown",
    };
    let second_rule = match &result.audit_lines[1] {
        AuditLine::AppliedFix(f) => f.rule.predicate_id(),
        AuditLine::TextCorrection(tc) => tc.rule.predicate_id(),
        _ => "unknown",
    };
    assert_eq!(first_rule, "C001");
    assert_eq!(second_rule, "E006");
    // R002 fired flag set.
    assert!(result.r002_fired);
    // R002 diagnostic is the last entry in remaining_diagnostics.
    assert!(!result.remaining_diagnostics.is_empty());
    let last = result.remaining_diagnostics.last().unwrap();
    assert_eq!(last.rule, super::R002_RULE_ID);
    // Apply mode returns the pass-1 buffer.
    assert_eq!(result.source.expose_secret(), b"POST-PASS-1-BUFFER");
}

#[test]
fn assemble_r002_result_dryrun_returns_original_source() {
    // DryRun mode returns the original `self.source`, NOT the
    // pass-1 buffer — even though pass-1's audit records are
    // preserved (D-7.6: "the fixes happened; the audit log is
    // honest about it" doesn't mean the buffer mutates in dry-run).
    let engine = engine_with(vec![]);
    let fixer = super::TwoPassFixer {
        engine: &engine,
        source: TEST_SRC,
        mode: FixMode::DryRun,
        threshold: 0.95,
        deadline: None,
    };

    let pass1 = Pass1Result {
        post_buffer: Zeroizing::new(b"POST-PASS-1-BUFFER".to_vec()),
        audit_lines: vec![synth_audit_line("E006", 8, 12)],
        applied_keys: HashSet::new(),
    };
    let lint = LintResult {
        diagnostics: Vec::new(),
        truncated: false,
        candidates_processed: 0,
        candidates_total: 0,
        recognized_marking_count: 0,
    };
    let r002 = super::build_r002_diagnostic(SmallVec::new(), Span::new(0, 0));
    let result = fixer.assemble_r002_result(Vec::new(), Vec::new(), pass1, lint, r002);
    assert_eq!(result.source.expose_secret(), TEST_SRC);
    assert!(result.r002_fired);
}

#[test]
fn assemble_r002_result_carries_through_pass0_dropped_diagnostics() {
    // Pass-0 dropped diagnostics (C-1 overlap-loss in the text-
    // correction layer) MUST surface via remaining_diagnostics
    // even on the R002 path; the pass-2 lint never runs to re-emit
    // them.
    let engine = engine_with(vec![]);
    let fixer = super::TwoPassFixer {
        engine: &engine,
        source: TEST_SRC,
        mode: FixMode::Apply,
        threshold: 0.95,
        deadline: None,
    };

    let dropped = vec![Diagnostic::<CapcoScheme>::new(
        RuleId::new("capco", "marking.correction.token-typo"),
        Severity::Fix,
        Span::new(20, 24),
        stub_message(),
        stub_citation(),
        None,
    )];
    let pass1 = Pass1Result {
        post_buffer: Zeroizing::new(Vec::new()),
        audit_lines: Vec::new(),
        applied_keys: HashSet::new(),
    };
    let lint = LintResult {
        diagnostics: Vec::new(),
        truncated: false,
        candidates_processed: 0,
        candidates_total: 0,
        recognized_marking_count: 0,
    };
    let r002 = super::build_r002_diagnostic(SmallVec::new(), Span::new(0, 0));
    let result = fixer.assemble_r002_result(Vec::new(), dropped, pass1, lint, r002);
    // Dropped diagnostic + R002 = 2 entries; the dropped one
    // appears before the R002 entry (R002 pushed last).
    assert_eq!(result.remaining_diagnostics.len(), 2);
    assert_eq!(
        result.remaining_diagnostics[0].rule.predicate_id(),
        "marking.correction.token-typo"
    );
    assert_eq!(result.remaining_diagnostics[1].rule, super::R002_RULE_ID);
}

#[test]
fn assemble_r002_result_filters_fixed_diagnostics_from_remaining() {
    // A diagnostic whose fix landed (applied key matches) is
    // filtered out of `remaining_diagnostics`. Locks the
    // applied_keys filter on the R002 path — without this the
    // same diagnostic would surface in both applied and remaining.
    let engine = engine_with(vec![]);
    let fixer = super::TwoPassFixer {
        engine: &engine,
        source: TEST_SRC,
        mode: FixMode::Apply,
        threshold: 0.95,
        deadline: None,
    };

    let intent = FixIntent::<CapcoScheme> {
        replacement: ReplacementIntent::Recanonicalize {
            scope: RecanonScope::Portion,
        },
        confidence: marque_rules::Confidence::strict(1.0),
        feature_ids: SmallVec::new(),
        // Phase-partition filtering test keyed on (rule, span); message
        // templates are irrelevant to what it asserts. Reuse the shared
        // stub on both the diagnostic and the fix so the fixture makes no
        // template-parity claim (issue #709 removed the prior hardcoded
        // `BannerRollupMismatch` here).
        message: stub_message(),
        source: FixSource::BuiltinRule,
        migration_ref: None,
    };
    // Keep the test's `(rule, span)` matching invariant by using a
    // synthetic `"test"`-scheme id on both the diagnostic and the audit
    // line — the assembler's filter key is `(rule, span)` so both ends
    // must agree on rule identity.
    let diag_with_fix = Diagnostic::with_fix(
        RuleId::new("test", "E006"),
        Severity::Error,
        Span::new(8, 14),
        stub_message(),
        stub_citation(),
        Some(intent),
    );
    let pass1_audit_lines = vec![synth_audit_line("E006", 8, 14)];
    let pass1 = Pass1Result {
        post_buffer: Zeroizing::new(Vec::new()),
        audit_lines: pass1_audit_lines,
        applied_keys: HashSet::new(),
    };
    let lint = LintResult {
        diagnostics: vec![diag_with_fix],
        truncated: false,
        candidates_processed: 0,
        candidates_total: 0,
        recognized_marking_count: 0,
    };
    let r002 = super::build_r002_diagnostic(SmallVec::new(), Span::new(0, 0));
    let result = fixer.assemble_r002_result(Vec::new(), Vec::new(), pass1, lint, r002);
    // Pre-r002 entries are 0 (the E006 diag was filtered),
    // then R002 is pushed last.
    assert_eq!(result.remaining_diagnostics.len(), 1);
    assert_eq!(result.remaining_diagnostics[0].rule, super::R002_RULE_ID);
}

// ---------------------------------------------------------------------------
// PR #490 — PageFinalization read-only-attrs sentinel helper tests
// ---------------------------------------------------------------------------
//
// Separate `#[cfg(test)]` module from `mod tests` above because the
// existing module carries `#[cfg_attr(coverage_nightly, coverage(off))]`
// — these sentinel tests need to land in Codecov patch coverage (the
// motivation for extracting `check_portions_unchanged` to a testable
// helper in the first place). Keeping them in a coverage-included
// module makes the comparison + error-message-construction paths
// of `check_portions_unchanged` visible to the coverage tool.

#[cfg(test)]
mod sentinel_tests {
    use super::check_portions_unchanged;
    use marque_ism::{CanonicalAttrs, Classification, MarkingClassification};

    /// Construct a default `CanonicalAttrs`. `CanonicalAttrs` is
    /// `#[non_exhaustive]` so we use `Default::default()` and patch
    /// the field(s) the test needs.
    fn empty_attrs() -> CanonicalAttrs {
        CanonicalAttrs::default()
    }

    /// `CanonicalAttrs` with a SECRET US classification — used as the
    /// "before" snapshot in mismatched-content tests so the
    /// "after" diverges on the classification field.
    ///
    /// `CanonicalAttrs` is `#[non_exhaustive]` so cross-crate
    /// construction goes through `Default::default()` + field
    /// mutation; the struct-expression form is not callable.
    fn secret_attrs() -> CanonicalAttrs {
        let mut attrs = CanonicalAttrs::default();
        attrs.classification = Some(MarkingClassification::Us(Classification::Secret));
        attrs
    }

    /// Test 1 — equality path returns `Ok(())`.
    ///
    /// Exercises the `before == after` branch of
    /// [`check_portions_unchanged`]. Covers both the empty-slice
    /// case (the typical PageFinalization dispatch shape on a
    /// no-portion page) and a single-portion case where the two
    /// sides are independent clones.
    #[test]
    fn check_portions_unchanged_returns_ok_on_equal_slices() {
        // Empty + empty — the typical no-portion dispatch shape.
        assert!(check_portions_unchanged(&[], &[], 0).is_ok());

        // Single portion, cloned — Vec clone proves the comparison
        // is value-equality, not pointer-equality.
        let portions = vec![secret_attrs()];
        let cloned = portions.clone();
        assert!(check_portions_unchanged(&portions, &cloned, 1).is_ok());
    }

    /// Test 2 — mismatched lengths return `Err`, error string
    /// carries counts + rule_count, and no type/field names appear.
    ///
    /// Exercises the `Err` branch and verifies the format-arg
    /// interpolation lands the three `usize` operands in the
    /// rendered string. The G13 negative assertions guard against
    /// a future format-string edit that re-introduces operand
    /// `Debug` representation.
    #[test]
    fn check_portions_unchanged_returns_err_on_mismatched_lengths() {
        let before = vec![secret_attrs()];
        let after: Vec<CanonicalAttrs> = vec![];

        let err = check_portions_unchanged(&before, &after, 7)
            .expect_err("length mismatch must surface as Err");

        // Counts present.
        assert!(
            err.contains("1 portion(s) before vs 0 after"),
            "expected count phrase in error, got: {err}"
        );
        assert!(
            err.contains("7 rule(s) dispatched"),
            "expected rule_count phrase in error, got: {err}"
        );

        // G13 (Constitution V Principle V): no type names that
        // would imply portion content leakage.
        assert!(
            !err.contains("CanonicalAttrs"),
            "G13 violation: type name `CanonicalAttrs` in error: {err}"
        );
        assert!(
            !err.contains("SciControl"),
            "G13 violation: type name `SciControl` in error: {err}"
        );
        assert!(
            !err.contains("Span"),
            "G13 violation: type name `Span` in error: {err}"
        );
        assert!(
            !err.contains("MarkingClassification"),
            "G13 violation: type name `MarkingClassification` in error: {err}"
        );
        assert!(
            !err.contains("Secret"),
            "G13 violation: classification variant `Secret` in error: {err}"
        );
    }

    /// Test 3 — same-length-but-different-content mismatch returns
    /// `Err`. Distinct from test 2 (which exercises a length
    /// mismatch); this one forces the slice `PartialEq` to walk
    /// into element-by-element comparison before returning `false`.
    #[test]
    fn check_portions_unchanged_returns_err_on_mismatched_content() {
        let before = vec![empty_attrs()];
        let after = vec![secret_attrs()];

        let err = check_portions_unchanged(&before, &after, 1)
            .expect_err("content mismatch must surface as Err");

        // Counts: both sides are length-1, so the count phrasing
        // is symmetric. The error message reports the symmetry
        // truthfully ("1 portion(s) before vs 1 after") — that is
        // the documented limitation of the outer-loop sentinel
        // placement (it cannot attribute which portion mutated).
        assert!(
            err.contains("1 portion(s) before vs 1 after"),
            "expected count phrase in error, got: {err}"
        );
        assert!(
            err.contains("1 rule(s) dispatched"),
            "expected rule_count phrase in error, got: {err}"
        );

        // The doc-cross-reference is the audit trail back to the
        // invariant statement — verify it survives format-arg
        // expansion.
        assert!(
            err.contains("section 3 (e.1)"),
            "expected doc-cross-reference in error, got: {err}"
        );
    }

    /// Test 4 — load-bearing G13 invariant test.
    ///
    /// Constructs a `CanonicalAttrs` with distinctive free-text
    /// content (`classified_by`) — the kind of field that a
    /// `debug_assert_eq!` macro would auto-dump via `Debug` formatting
    /// on panic (`core::panicking::assert_failed_inner` formats both
    /// operands as `left: {:?} right: {:?}` regardless of any custom
    /// message). Calls the helper with a mismatch and asserts the
    /// rendered error string does NOT contain the distinctive content.
    ///
    /// This is the redundant-by-design content-ignorance check that
    /// pins the helper's contract independent of the type-name negative
    /// assertions in test 2 — the failure mode
    /// being guarded against is "a future helper edit pipes
    /// element content through `{:?}`", and the sentinel value
    /// here is what makes that regression detectable.
    #[test]
    fn check_portions_unchanged_error_message_is_g13_compliant() {
        // Distinctive sentinel embedded in a free-text field. If
        // any future edit to `check_portions_unchanged` formats
        // a `CanonicalAttrs` field via `Debug` / `Display`, this
        // string will surface in the rendered error.
        const G13_SENTINEL: &str = "MARQUE-PR-490-G13-CANARY-XYZZY-7F3A1B2C";

        let mut attrs_with_canary = CanonicalAttrs::default();
        attrs_with_canary.classified_by = Some(G13_SENTINEL.into());
        let before = vec![attrs_with_canary];
        let after: Vec<CanonicalAttrs> = vec![];

        let err = check_portions_unchanged(&before, &after, 1)
            .expect_err("mismatch must surface as Err for the G13 check");

        // The load-bearing assertion: the distinctive sentinel
        // string MUST NOT appear anywhere in the rendered error.
        // If this fires, the helper has regressed — operand content is
        // leaking through the panic surface.
        assert!(
            !err.contains(G13_SENTINEL),
            "G13 violation: classified_by content leaked into sentinel \
             error message. Sentinel string `{G13_SENTINEL}` found in \
             rendered error: {err}"
        );

        // Sanity: the helper still rendered a non-empty error
        // (i.e., we didn't accidentally pass the assertion by
        // making the helper a no-op).
        assert!(
            !err.is_empty(),
            "G13 test fixture invalid: helper returned empty error \
             string — the negative assertion above is vacuous."
        );
    }
}
