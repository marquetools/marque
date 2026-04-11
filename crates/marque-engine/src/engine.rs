//! `Engine` — the configured, ready-to-run pipeline.

use crate::clock::{Clock, SystemClock};
use crate::output::{FixResult, LintResult};
use marque_config::Config;
use marque_ism::Span;
use marque_rules::{AppliedFix, RuleId, RuleSet, Severity};

/// Whether to apply fixes or just simulate (dry-run).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixMode {
    /// Apply fixes to the source text.
    Apply,
    /// Simulate fixes — audit stream is identical but source is unchanged.
    DryRun,
}

/// Error returned when a caller supplies a runtime confidence threshold
/// override that is outside the valid `[0.0, 1.0]` range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InvalidThreshold(pub f32);

impl std::fmt::Display for InvalidThreshold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "confidence threshold {} is outside [0.0, 1.0] or is NaN",
            self.0
        )
    }
}

impl std::error::Error for InvalidThreshold {}

/// A configured engine instance.
pub struct Engine {
    config: Config,
    rule_sets: Vec<Box<dyn RuleSet>>,
    clock: Box<dyn Clock>,
}

impl Engine {
    /// Create a new engine with the given configuration and rule sets.
    pub fn new(config: Config, rule_sets: Vec<Box<dyn RuleSet>>) -> Self {
        Self {
            config,
            rule_sets,
            clock: Box::new(SystemClock),
        }
    }

    /// Create an engine with a custom clock (for deterministic tests).
    pub fn with_clock(
        config: Config,
        rule_sets: Vec<Box<dyn RuleSet>>,
        clock: Box<dyn Clock>,
    ) -> Self {
        Self {
            config,
            rule_sets,
            clock,
        }
    }

    /// Lint a UTF-8 text buffer. Returns diagnostics without modifying input.
    pub fn lint(&self, source: &[u8]) -> LintResult {
        use marque_core::{Parser, Scanner};
        use marque_ism::{CapcoTokenSet, MarkingType, PageContext};
        use marque_rules::RuleContext;
        use std::sync::Arc;

        let token_set = CapcoTokenSet;
        let parser = Parser::new(&token_set);
        let candidates = Scanner::scan(source);

        let mut diagnostics = Vec::new();
        // Build page context by accumulating portion markings in document order.
        // Banner and CAB rules receive this context so they can validate the
        // observed banner against the expected composite.
        // TODO(phase-3): reset the context at page boundaries when the scanner
        // provides page-break candidates.
        let mut page_context = PageContext::new();
        // Cache the current Arc<PageContext> so that consecutive banner/CAB
        // candidates on the same page share a single allocation. The cache is
        // invalidated (set to None) whenever a new portion is accumulated.
        let mut page_context_arc: Option<Arc<PageContext>> = None;

        for candidate in &candidates {
            let Ok(parsed) = parser.parse(candidate, source) else {
                continue;
            };

            // Accumulate portions before running banner/CAB rules so that
            // when we reach a banner candidate the context already reflects
            // all preceding portion data.
            if parsed.kind == MarkingType::Portion {
                page_context.add_portion(parsed.attrs.clone());
                // Invalidate the cached Arc so the next banner/CAB gets a
                // fresh snapshot. We rebuild it lazily below.
                page_context_arc = None;
            }

            // TODO(phase-3): plumb the document zone and position from the
            // scanner. Both are currently hardcoded to `Body`, which is
            // correct for current rules (they only key off `marking_type`)
            // but will silently lie to any future rule that reads them.
            let ctx_page = if parsed.kind != MarkingType::Portion && !page_context.is_empty() {
                // Lazily wrap the accumulated context in an Arc once per
                // page-context snapshot; subsequent banner/CAB candidates on
                // the same page clone only the cheap Arc pointer.
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
                zone: marque_ism::Zone::Body,
                position: marque_ism::DocumentPosition::Body,
                page_context: ctx_page,
            };
            for rule_set in &self.rule_sets {
                for rule in rule_set.rules() {
                    // Skip rules that are configured as Off.
                    let configured_severity = self
                        .config
                        .rules
                        .overrides
                        .get(rule.id().as_str())
                        .and_then(|s| Severity::parse_config(s))
                        .unwrap_or(rule.default_severity());

                    if configured_severity == Severity::Off {
                        continue;
                    }

                    let mut diags = rule.check(&parsed.attrs, &ctx);
                    // Apply configured severity override.
                    for d in &mut diags {
                        d.severity = configured_severity;
                    }
                    diagnostics.extend(diags);
                }
            }
        }

        LintResult { diagnostics }
    }

    /// Lint and apply fixes. Returns fixed source and audit log.
    ///
    /// Fix application order follows FR-016: `(span.end DESC, span.start DESC,
    /// rule_id ASC, replacement ASC)` so reverse-byte application preserves
    /// earlier-span offsets and equal-span ties break deterministically.
    ///
    /// Uses the confidence threshold configured in the engine's `Config`.
    /// To supply a per-call override (e.g., from a `--confidence` CLI flag
    /// or an HTTP request field), use [`Engine::fix_with_threshold`].
    pub fn fix(&self, source: &[u8], mode: FixMode) -> FixResult {
        // The config threshold is pre-validated at load time, so the
        // `Result` branch is unreachable.
        self.fix_with_threshold(source, mode, None)
            .expect("config-supplied confidence threshold is pre-validated")
    }

    /// Lint and apply fixes using an optional per-call confidence threshold.
    ///
    /// When `threshold_override` is `Some`, it replaces the config-level
    /// threshold for this call only and is validated against `[0.0, 1.0]`.
    /// When `None`, the engine falls back to `Config::confidence_threshold`.
    pub fn fix_with_threshold(
        &self,
        source: &[u8],
        mode: FixMode,
        threshold_override: Option<f32>,
    ) -> Result<FixResult, InvalidThreshold> {
        let threshold = match threshold_override {
            Some(value) => {
                if !(0.0..=1.0).contains(&value) || value.is_nan() {
                    return Err(InvalidThreshold(value));
                }
                value
            }
            None => self.config.confidence_threshold(),
        };

        Ok(self.fix_inner(source, mode, threshold))
    }

    fn fix_inner(&self, source: &[u8], mode: FixMode, threshold: f32) -> FixResult {
        use std::collections::HashSet;

        let lint = self.lint(source);

        let mut fixes: Vec<_> = lint
            .diagnostics
            .iter()
            .filter_map(|d| d.fix.as_ref())
            .filter(|f| f.confidence >= threshold)
            .filter(|f| !f.span.is_empty())
            .collect();

        // FR-016: deterministic total-order fix application.
        // Sort by (span.end DESC, span.start DESC, rule_id ASC, replacement ASC).
        fixes.sort_by(|a, b| {
            b.span
                .end
                .cmp(&a.span.end)
                .then(b.span.start.cmp(&a.span.start))
                .then(a.rule.cmp(&b.rule))
                .then(a.replacement.cmp(&b.replacement))
        });

        // C-1: overlap guard. After the FR-016 sort, two fixes can still
        // touch the same byte range if multiple rules emit a fix for the
        // same span (or overlapping spans). Applying both via `splice`
        // would silently corrupt the byte stream. We keep the first fix
        // per span (which under FR-016 ordering is deterministic) and
        // surface the dropped fixes through `remaining_diagnostics`.
        //
        // The walk is over fixes in reverse-end order, so a fix is kept
        // only if its `span.end` is at or below the previous kept fix's
        // `span.start` — i.e., strictly to the left, no overlap.
        let mut kept_fixes: Vec<&marque_rules::FixProposal> = Vec::with_capacity(fixes.len());
        let mut next_window_end: Option<usize> = None;
        for fix in &fixes {
            let fits = match next_window_end {
                Some(boundary) => fix.span.end <= boundary,
                None => true,
            };
            if fits {
                next_window_end = Some(fix.span.start);
                kept_fixes.push(*fix);
            }
        }

        // M-4: hold the classifier id in an `Arc<str>` so cloning into each
        // applied-fix audit record is an O(1) refcount bump rather than a
        // full string copy per fix.
        let classifier_id: Option<std::sync::Arc<str>> = self
            .config
            .user
            .classifier_id
            .as_deref()
            .map(std::sync::Arc::from);
        let dry_run = mode == FixMode::DryRun;
        let now = self.clock.now();

        // H-7: applied-fix lookup is keyed by (RuleId, Span). Use a HashSet
        // so the per-diagnostic filter at the bottom of this function is
        // O(1) per query instead of O(n) over a Vec.
        let mut applied_keys: HashSet<(RuleId, Span)> = HashSet::with_capacity(kept_fixes.len());
        let mut applied: Vec<AppliedFix> = Vec::with_capacity(kept_fixes.len());

        // Only allocate the output buffer when we actually need to mutate it.
        // Dry-run returns the original source verbatim.
        let output = match mode {
            FixMode::Apply => {
                let mut buf = source.to_vec();
                for fix in kept_fixes {
                    buf.splice(fix.span.start..fix.span.end, fix.replacement.bytes());
                    applied_keys.insert((fix.rule.clone(), fix.span));
                    applied.push(AppliedFix::__engine_promote(
                        fix.clone(),
                        now,
                        classifier_id.clone(),
                        dry_run,
                        None, // input identifier set by CLI at the boundary
                    ));
                }
                buf
            }
            FixMode::DryRun => {
                for fix in kept_fixes {
                    applied_keys.insert((fix.rule.clone(), fix.span));
                    applied.push(AppliedFix::__engine_promote(
                        fix.clone(),
                        now,
                        classifier_id.clone(),
                        dry_run,
                        None,
                    ));
                }
                source.to_vec()
            }
        };

        // Remaining diagnostics: those whose fix was not applied.
        // Filter by (rule_id, span) pair — not just rule ID — so that if
        // rule E001 fires on three spans and only one is fixed, the other
        // two remain.
        let remaining_diagnostics = lint
            .diagnostics
            .into_iter()
            .filter(|d| {
                !d.fix
                    .as_ref()
                    .is_some_and(|f| applied_keys.contains(&(f.rule.clone(), f.span)))
            })
            .collect();

        FixResult {
            source: output,
            applied,
            remaining_diagnostics,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::FixedClock;
    use marque_ism::IsmAttributes;
    use marque_rules::{
        Diagnostic, FixProposal, FixSource, Rule, RuleContext, RuleId, RuleSet, Severity,
    };
    use std::time::{Duration, UNIX_EPOCH};

    /// A test rule that emits a fixed list of FixProposals on every check call,
    /// ignoring the parsed attributes. Lets us drive the engine deterministically
    /// without depending on real CAPCO rule output.
    struct StubRule {
        id: &'static str,
        proposals: Vec<FixProposal>,
    }

    impl Rule for StubRule {
        fn id(&self) -> RuleId {
            RuleId::new(self.id)
        }
        fn name(&self) -> &'static str {
            "stub"
        }
        fn default_severity(&self) -> Severity {
            Severity::Fix
        }
        fn check(&self, _attrs: &IsmAttributes, _ctx: &RuleContext) -> Vec<Diagnostic> {
            self.proposals
                .iter()
                .map(|p| {
                    Diagnostic::new(
                        p.rule.clone(),
                        Severity::Fix,
                        p.span,
                        "stub",
                        "TEST",
                        Some(p.clone()),
                    )
                })
                .collect()
        }
    }

    struct StubSet(Vec<Box<dyn Rule>>);
    impl RuleSet for StubSet {
        fn rules(&self) -> &[Box<dyn Rule>] {
            &self.0
        }
        fn schema_version(&self) -> &'static str {
            "TEST"
        }
    }

    fn proposal(rule: &'static str, start: usize, end: usize, replacement: &str) -> FixProposal {
        proposal_with_confidence(rule, start, end, replacement, 1.0)
    }

    fn proposal_with_confidence(
        rule: &'static str,
        start: usize,
        end: usize,
        replacement: &str,
        confidence: f32,
    ) -> FixProposal {
        FixProposal::new(
            RuleId::new(rule),
            FixSource::BuiltinRule,
            Span::new(start, end),
            "x",
            replacement,
            confidence,
            None,
        )
    }

    fn engine_with(proposals: Vec<FixProposal>) -> Engine {
        engine_with_config(Config::default(), proposals)
    }

    fn engine_with_config(config: Config, proposals: Vec<FixProposal>) -> Engine {
        let stub = StubRule {
            id: "TEST",
            proposals,
        };
        let set: Box<dyn RuleSet> = Box::new(StubSet(vec![Box::new(stub)]));
        Engine::with_clock(
            config,
            vec![set],
            Box::new(FixedClock::new(
                UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            )),
        )
    }

    /// A source long enough to span the test fix offsets, AND containing a
    /// banner marking so the parser produces a candidate that triggers
    /// the rule loop in `Engine::lint`.
    const TEST_SRC: &[u8] = b"SECRET//NOFORN                                                ";

    #[test]
    fn fix_applies_disjoint_fixes_in_reverse_order() {
        // Two non-overlapping fixes; FR-016 sorts by span.end DESC so the
        // later one is applied first, preserving the earlier span's offsets.
        let engine = engine_with(vec![
            proposal("E001", 0, 6, "AA"),  // "SECRET" → "AA"
            proposal("E002", 8, 14, "BB"), // "NOFORN" → "BB"
        ]);
        let result = engine.fix(TEST_SRC, FixMode::Apply);
        let out = String::from_utf8(result.source).unwrap();
        assert!(out.starts_with("AA//BB"), "got: {out:?}");
        assert_eq!(result.applied.len(), 2);
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
        assert_eq!(result.applied.len(), 1, "applied: {:?}", result.applied);
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
        assert_eq!(result.source, TEST_SRC, "dry-run must not mutate source");
        assert_eq!(result.applied.len(), 1);
        assert!(result.applied[0].dry_run, "dry_run flag must be set");
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
        assert_eq!(r1.applied[0].timestamp, r2.applied[0].timestamp);
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

    // M-4: the confidence filter at `f.confidence >= threshold` is on the
    // hot path of Engine::fix. These two tests pin the `>=` semantics so a
    // future refactor that flips it to `>` (or vice versa) is caught.
    #[test]
    fn confidence_below_default_threshold_is_excluded() {
        // Config::default().confidence_threshold == 0.95. A fix at 0.94
        // must not be applied.
        let engine = engine_with(vec![proposal_with_confidence("E001", 0, 6, "AA", 0.94)]);
        let result = engine.fix(TEST_SRC, FixMode::Apply);
        assert_eq!(result.applied.len(), 0);
        // The below-threshold fix is a suggestion — it survives in
        // remaining_diagnostics so the caller can surface it.
        assert_eq!(result.remaining_diagnostics.len(), 1);
    }

    #[test]
    fn confidence_at_default_threshold_is_included() {
        // A fix at exactly 0.95 must be applied (inclusive threshold).
        let engine = engine_with(vec![proposal_with_confidence("E001", 0, 6, "AA", 0.95)]);
        let result = engine.fix(TEST_SRC, FixMode::Apply);
        assert_eq!(result.applied.len(), 1);
    }

    // M-5: the zero-length-span filter (`!f.span.is_empty()`) in fix_inner
    // is what masked the Phase 2 Span::new(0, 0) placeholders from the
    // C-1 overlap guard. This test pins that guard explicitly so a future
    // refactor that drops the filter is caught.
    #[test]
    fn zero_length_span_fix_is_filtered_before_sort() {
        let engine = engine_with(vec![proposal("E001", 5, 5, "X")]);
        let result = engine.fix(TEST_SRC, FixMode::Apply);
        assert_eq!(result.applied.len(), 0);
        // Source unchanged: no splice was attempted.
        assert_eq!(result.source, TEST_SRC);
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
        // Only the 0.6 fix is applied.
        assert_eq!(result.applied.len(), 1);
        assert_eq!(result.applied[0].proposal.rule.as_str(), "E002");
        // The 0.4 fix surfaces as a remaining diagnostic.
        assert_eq!(result.remaining_diagnostics.len(), 1);
    }
}
