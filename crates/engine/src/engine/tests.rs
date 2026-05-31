use super::dispatch::{canonicalize_rule_overrides, levenshtein, suggest_closest};
use super::fix::{TwoPassFixer, engine_promotion_token, partition_diags_by_phase};
use super::fix_impl::Pass1Result;
#[cfg(debug_assertions)]
use super::page_context::check_portions_unchanged;
use super::synthesis::{
    HEURISTIC_RECOGNITION_CAP, build_r002_diagnostic, find_containing_marking, lookup_marking,
    sort_and_c1_dedup, span_is_within_marking, splice_fixes_forward,
};
use super::*;
use crate::clock::FixedClock;
use marque_ism::CanonicalAttrs;
use marque_rules::audit::AppliedFix;
use marque_rules::{
    Diagnostic, FixIntent, FixSource, Message, MessageArgs, MessageTemplate, Rule, RuleContext,
    RuleId, RuleSet, Severity,
};
use marque_scheme::fix_intent::RecanonScope;
use marque_scheme::{AuthoritativeSource, Citation, ReplacementIntent, SectionLetter, SectionRef};
use secrecy::ExposeSecret as _;
use std::time::{Duration, UNIX_EPOCH};

/// Test-fixture `Message` stub for `Diagnostic` constructors that
/// don't exercise message content.
///
/// Uses `UnrecognizedToken` (a generic closed-set template variant)
/// with default args — no `TokenId` lookup needed, no axis-specific
/// payload required. The engine tests that consume this helper
/// assert against `Diagnostic.rule`, `.span`, `.severity`, and
/// fix-attachment shape, never against message content.
#[inline]
fn stub_message() -> Message {
    Message::new(MessageTemplate::UnrecognizedToken, MessageArgs::default())
}

/// Test-fixture [`SessionMetadata`] for direct `TwoPassFixer`
/// construction in unit tests that bypass `Engine::fix_inner` (which
/// is where the real metadata is built). Carries the engine-wide
/// version constants and the `Other` interface; identity / signature
/// are `None` (these tests do not exercise the metadata wire shape —
/// that lives in `session.rs::tests` and the audit canary).
#[inline]
fn test_session_metadata() -> crate::SessionMetadata {
    crate::SessionMetadata {
        marque_version: crate::MARQUE_VERSION,
        audit_schema: crate::AUDIT_SCHEMA_VERSION,
        lattice_version: smol_str::SmolStr::new(marque_capco::LATTICE_VERSION),
        decoder_version: crate::DECODER_VERSION,
        interface: crate::InterfaceCode::Other,
        classifier_id: None,
        classification_authority: None,
        signature: None,
    }
}

/// Filter the marking-side audit lines from a [`FixResult`] into
/// a `Vec<&AppliedFix>` view.
///
/// The engine's sole audit-output channel is
/// `FixResult.audit_lines: Vec<AuditLine<S>>`. This helper exposes a
/// marking-side-only read shape for unit tests that consume only the
/// marking side without pattern-matching the sum type at every
/// assertion site.
/// Text-correction audit lines (`AuditLine::TextCorrection`) are
/// surfaced by [`applied_text_corrections`] below.
#[inline]
fn applied_fixes(result: &FixResult) -> Vec<&AppliedFix<CapcoScheme>> {
    result
        .audit_lines
        .iter()
        .filter_map(|line| match line {
            AuditLine::AppliedFix(f) => Some(f),
            _ => None,
        })
        .collect()
}

/// Filter the text-correction audit lines from a [`FixResult`]
/// into a `Vec<&AppliedTextCorrection>` view.
#[inline]
fn applied_text_corrections(result: &FixResult) -> Vec<&AppliedTextCorrection> {
    result
        .audit_lines
        .iter()
        .filter_map(|line| match line {
            AuditLine::TextCorrection(tc) => Some(tc),
            _ => None,
        })
        .collect()
}

/// Test-fixture `Citation` stub for `Diagnostic` constructors that
/// don't exercise citation content.
///
/// Uses `AuthoritativeSource::EngineInternal` (a non-CAPCO sentinel
/// source per PM-C-4) so the citation-lint scanner skips this entry
/// — these stubs are test fixtures, not real CAPCO citations, and
/// must not trip the §-citation resolver. The `SectionRef` /
/// `PageNumber` carry niche-sentinel values the Display impl
/// deliberately elides for non-CAPCO sources.
#[inline]
fn stub_citation() -> Citation {
    Citation::new(
        AuthoritativeSource::EngineInternal,
        SectionRef::new(SectionLetter::A),
        core::num::NonZeroU16::new(1).unwrap(),
    )
}

/// Pins the issue #430 pre-size contract on the per-page portion
/// accumulator. If `fresh_page_portions_accumulator` ever drifts
/// to `Vec::new()` (or a smaller capacity), every subsequent page
/// on a multi-page document would pay the `Vec` growth sequence
/// (4 → 8 → 16 …) on the first several portion pushes — a silent
/// perf regression no functional test catches. This test fails
/// at compile-after-edit time if the helper body diverges from
/// `DEFAULT_PORTIONS_CAPACITY`.
#[test]
fn fresh_accumulator_uses_default_capacity() {
    let v = fresh_page_portions_accumulator();
    assert!(
        v.is_empty(),
        "fresh accumulator must be empty, got len={}",
        v.len()
    );
    assert_eq!(
        v.capacity(),
        DEFAULT_PORTIONS_CAPACITY,
        "fresh accumulator capacity drifted from DEFAULT_PORTIONS_CAPACITY ({}); \
             multi-page perf regression risk per issue #430",
        DEFAULT_PORTIONS_CAPACITY,
    );
}

/// Pins the rewrite scheduling contract for the generic front edge
/// of `Engine::with_clock`: extracting a non-generic tail must not
/// change the rewrite schedule chosen for the default scheme.
#[test]
fn with_clock_uses_default_rewrite_schedule() {
    let via_new = Engine::new(
        Config::default(),
        crate::default_ruleset(),
        crate::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles");
    let via_with_clock = Engine::with_clock(
        Config::default(),
        crate::default_ruleset(),
        crate::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(0))),
    )
    .expect("default CAPCO scheme has no rewrite cycles");

    assert_eq!(
        via_new.scheduled_rewrites(),
        via_with_clock.scheduled_rewrites(),
        "scheduling regression: with_clock no longer preserves the scheduler output for the default scheme"
    );
}

/// A pure-test proposal carrier: the fields engine tests actually
/// exercise (rule, span, replacement, confidence). The engine pipeline
/// takes `FixIntent<S>` exclusively, so `StubRule` synthesizes a
/// `Recanonicalize` intent + a separate Diagnostic per
/// `StubProposal` and the engine's `synthesize_fixes` path runs
/// the recanonicalization through a stub-scheme override.
///
/// Tests that need byte-precise replacement assertions install a
/// `replacement` here and assert against the engine output after
/// fix application — they don't reach into the audit-record
/// `proposal` shape.
#[derive(Debug, Clone)]
pub(super) struct StubProposal {
    pub rule: RuleId,
    pub span: Span,
    pub replacement: Box<str>,
    pub confidence: Recognition,
    pub source: FixSource,
}

#[test]
fn heuristic_recognition_cap_matches_default_threshold() {
    // Issue #133 PR 4 invariant: the position-aware classification
    // heuristic's `Recognition::recognition` cap (renamed from
    // `HEURISTIC_RULE_AXIS_CAP` in PR B, when the `rule` axis was
    // retired) is pinned at the default `confidence_threshold` (0.95).
    // Solo-candidate heuristic fixes auto-apply at the default
    // threshold; the empirical corpus measurement (see
    // `HEURISTIC_RECOGNITION_CAP` doc and
    // `tools/corpus-analysis/output/heuristic_frequencies.json`)
    // justifies confidence ≥ 99.4% per-trigger, comfortably above
    // the cap.
    //
    // If a future change drops `HEURISTIC_RECOGNITION_CAP` below
    // `Config::default().confidence_threshold()`, that's a
    // behavioral regression: heuristic fixes that previously auto-
    // applied at the default threshold would silently stop
    // applying, and the user-visible "fix-and-warn" surface
    // collapses to "warn-only-without-fix" without an explicit
    // intent recorded in the change.
    //
    // If a future change drops the default `confidence_threshold`
    // below `HEURISTIC_RECOGNITION_CAP`, that's the inverse problem:
    // the heuristic suddenly becomes more aggressive than the
    // governance signal we agreed on. Either way, the equality
    // pin here forces a coordinated decision.
    let default_threshold = Config::default().confidence_threshold();
    assert!(
        (HEURISTIC_RECOGNITION_CAP - default_threshold).abs() < 1e-6,
        "HEURISTIC_RECOGNITION_CAP={HEURISTIC_RECOGNITION_CAP} must equal \
             Config::default().confidence_threshold()={default_threshold}; \
             a divergence requires an intentional governance change recorded \
             in the cap's doc comment"
    );
}

/// A test rule that emits text-correction diagnostics directly
/// (via `Diagnostic::text_correction`). Engine tests use this to
/// exercise the fix-application + audit-promotion path without
/// needing a real CAPCO scheme + `apply_intent` + `render_*`
/// roundtrip. The promotion lands on
/// `AppliedFix::__engine_promote_text_correction` via the engine's
/// `apply_text_corrections` path, which the test's
/// `text_correction`-bearing diagnostic feeds. The resulting
/// `AppliedFixProposal::TextCorrection { replacement }` carries
/// the canonical bytes for assertions.
struct StubRule {
    id: &'static str,
    proposals: Vec<StubProposal>,
}

impl Rule<CapcoScheme> for StubRule {
    fn id(&self) -> RuleId {
        // Every stub rule uses the reserved `"test"` scheme. The
        // `self.id` field carries the predicate id (the per-test
        // discriminant) so call sites like `proposal("E001", ...)`
        // read naturally.
        RuleId::new("test", self.id)
    }
    fn name(&self) -> &'static str {
        "stub"
    }
    fn default_severity(&self) -> Severity {
        Severity::Fix
    }
    fn check(&self, _attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        // Emit text-correction diagnostics: the C001 path is the
        // only fix channel that carries byte-precise replacement
        // bytes the engine actually applies. Engine tests
        // exercise the application + C-1 overlap-guard +
        // remaining-diagnostics path through this channel.
        //
        // For sub-threshold proposals also attach a structural
        // FixIntent so the lint post-pass demotes the severity
        // to Suggest (the demotion gate consults
        // `d.fix.confidence`, not `text_correction`).
        self.proposals
            .iter()
            .map(|p| {
                let mut d = Diagnostic::text_correction(
                    p.rule,
                    Severity::Fix,
                    p.span,
                    stub_message(),
                    stub_citation(),
                    p.replacement.clone(),
                    p.source,
                    p.confidence.clone(),
                    None,
                );
                if p.confidence.combined() < 1.0 {
                    d.fix = Some(FixIntent::<CapcoScheme> {
                        replacement: ReplacementIntent::Recanonicalize {
                            scope: RecanonScope::Portion,
                            prior: None,
                        },
                        confidence: p.confidence.clone(),
                        feature_ids: SmallVec::new(),
                        message: Message::new(
                            // Test-fixture FixIntent.message must agree
                            // with the test's `stub_message()` (Diagnostic-
                            // side) template — both `UnrecognizedToken` —
                            // so the audit-record contract
                            // `Diagnostic.message.template ==
                            // AppliedFix.message.template` (issue #709)
                            // holds in this synthetic fixture's audit
                            // line.
                            MessageTemplate::UnrecognizedToken,
                            MessageArgs::default(),
                        ),
                        source: FixSource::BuiltinRule,
                        migration_ref: None,
                    });
                }
                d
            })
            .collect()
    }
}

struct StubSet(Vec<Box<dyn Rule<CapcoScheme>>>);
impl RuleSet<CapcoScheme> for StubSet {
    fn rules(&self) -> &[Box<dyn Rule<CapcoScheme>>] {
        &self.0
    }
    fn schema_version(&self) -> &'static str {
        "TEST"
    }
}

fn proposal(rule: &'static str, start: usize, end: usize, replacement: &str) -> StubProposal {
    proposal_with_confidence(rule, start, end, replacement, 1.0)
}

fn proposal_with_confidence(
    rule: &'static str,
    start: usize,
    end: usize,
    replacement: &str,
    confidence: f32,
) -> StubProposal {
    StubProposal {
        // Every stub proposal uses the reserved `"test"` scheme; the
        // `rule` arg is the predicate id.
        rule: RuleId::new("test", rule),
        span: Span::new(start, end),
        replacement: replacement.into(),
        // Construct directly so the helper can synthesize sub-1.0
        // recognition values for threshold-gate tests; `Recognition::strict`
        // pins at 1.0 by definition (PR B).
        confidence: marque_rules::Recognition {
            recognition: confidence,
            runner_up_ratio: None,
            features: SmallVec::new(),
        },
        source: FixSource::CorrectionsMap,
    }
}

fn engine_with(proposals: Vec<StubProposal>) -> Engine {
    engine_with_config(Config::default(), proposals)
}

fn engine_with_config(config: Config, proposals: Vec<StubProposal>) -> Engine {
    let stub = StubRule {
        id: "TEST",
        proposals,
    };
    let set: Box<dyn RuleSet<CapcoScheme>> = Box::new(StubSet(vec![Box::new(stub)]));
    Engine::with_clock(
        config,
        vec![set],
        marque_capco::scheme::CapcoScheme::new(),
        Box::new(FixedClock::new(
            UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        )),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

/// A source long enough to span the test fix offsets, AND containing a
/// banner marking so the parser produces a candidate that triggers
/// the rule loop in `Engine::lint`.
const TEST_SRC: &[u8] = b"SECRET//NOFORN                                                ";

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

struct NamedStub {
    id: &'static str,
    name: &'static str,
}

impl Rule<CapcoScheme> for NamedStub {
    fn id(&self) -> RuleId {
        RuleId::new("test", self.id)
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    fn check(&self, _attrs: &CanonicalAttrs, _ctx: &RuleContext) -> Vec<Diagnostic<CapcoScheme>> {
        vec![]
    }
}

fn synth_audit_line(rule: &'static str, start: usize, end: usize) -> AuditLine<CapcoScheme> {
    let intent = FixIntent::<CapcoScheme> {
        replacement: ReplacementIntent::Recanonicalize {
            scope: RecanonScope::Portion,
            prior: None,
        },
        confidence: marque_rules::Recognition::strict(),
        feature_ids: SmallVec::new(),
        message: Message::new(
            // Test-fixture FixIntent.message must agree with the
            // Diagnostic-side template (`stub_message()` =
            // `UnrecognizedToken`) so the audit-record contract
            // `Diagnostic.message.template == AppliedFix.message.template`
            // (issue #709) holds in this synthetic fixture's audit line.
            MessageTemplate::UnrecognizedToken,
            MessageArgs::default(),
        ),
        source: FixSource::BuiltinRule,
        migration_ref: None,
    };
    let span = Span::new(start, end);
    let original_bytes: &[u8] = b"synth";
    // Test-fixture carve-out per Constitution V Principle V: __engine_construct
    // used here solely to build synthetic AppliedFix fixtures in test code.
    let constructor: EngineConstructor<CapcoScheme> =
        EngineConstructor::<CapcoScheme>::__engine_construct();
    let canonical: Canonical<CapcoScheme> = constructor.build_open_vocab(
        CategoryId::MARKING,
        Box::<str>::from("synth"),
        Scope::Portion,
    );
    AuditLine::AppliedFix(
        // Test-fixture carve-out per Constitution V Principle V: __engine_promote
        // used here solely to build synthetic AppliedFix fixtures in test code.
        marque_rules::audit::AppliedFix::<CapcoScheme>::__engine_promote(
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
        ),
    )
}

mod part1;
mod part2;
mod part3;
mod part4;
mod part5;
