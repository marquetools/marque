use super::dispatch::{
    build_severity_tables, canonicalize_rule_overrides, partition_rules_by_phase,
};
use super::*;

// Generic construction core. Unlike the CAPCO-default conveniences
// below, this block is generic over the scheme `S` and recognizer `R`:
// it stores the user-supplied `scheme` and `recognizer` directly — no
// discard, no fresh `CapcoScheme::new()`. The heavy construction body
// monomorphizes once per `(scheme, recognizer)` pair actually built
// (the CAPCO path, plus any test stub), not per call site.
impl<S, R> Engine<S, R>
where
    S: MarkingScheme + ConstraintBridge,
    R: Recognizer<S>,
{
    /// Create an engine over an arbitrary scheme and recognizer with a
    /// custom clock — the generic construction core.
    ///
    /// Runs the page-rewrite scheduler (Kahn's algorithm over the
    /// scheme's declared `reads` / `writes` axes) once at construction
    /// time. Cycles and unannotated `Custom` rewrites fail closed with
    /// [`EngineConstructionError`] rather than degrading at lint time.
    /// The passed `scheme` and `recognizer` become the engine's stored
    /// scheme and recognizer.
    ///
    /// [`Engine::new`] / [`Engine::with_clock`] are the CAPCO-default
    /// conveniences over this: they fix `S = CapcoScheme`,
    /// `R = EngineRecognizer`, and supply [`EngineRecognizer::default`]
    /// and [`SystemClock`] so existing call sites stay source-unchanged.
    pub fn with_clock_and_recognizer(
        mut config: Config,
        rule_sets: Vec<Box<dyn RuleSet<S>>>,
        scheme: S,
        recognizer: R,
        clock: Box<dyn Clock>,
    ) -> Result<Self, EngineConstructionError> {
        // Canonicalize [rules] overrides against the registered rule
        // set: accept the wire-string rule ID, its predicate-id half,
        // or the rule's descriptive name, resolve all to the canonical
        // predicate id before the engine stores the map, and hard-fail
        // on any unknown key. Consults `scheme.bridge_emitted_rule_ids()`
        // (a `ConstraintBridge` method) for IDs the engine emits without
        // a registered `Rule`. See `canonicalize_rule_overrides`.
        canonicalize_rule_overrides(&mut config, &rule_sets, &scheme)?;

        // Validate every `CategoryAction::Intent` payload BEFORE
        // scheduling. Reordering
        // matters when a rewrite table contains both an unroutable
        // Intent token AND a topological cycle: validate-first surfaces
        // the per-rewrite-id error (more actionable) instead of the
        // graph-shaped cycle error, and avoids wasting the scheduler
        // pass on a scheme that can't construct anyway. Walks each
        // intent's `FactRef`s and confirms the scheme can route each
        // one via `category_of`; a scheme-authoring bug surfaces here
        // at engine-construction time instead of silently no-opping on
        // the first page that triggers the rewrite.
        validate_intent_rewrites(&scheme, scheme.page_rewrites())?;
        let scheduled_rewrites = schedule_rewrites(scheme.page_rewrites())?;

        // Take ownership of the corrections map instead of cloning —
        // nothing reads config.corrections after construction.
        let corrections_arc = if config.corrections.is_empty() {
            None
        } else {
            Some(Arc::new(std::mem::take(&mut config.corrections)))
        };

        // Pre-build the AhoCorasick automaton for pre-scanner text corrections.
        // This is O(total pattern bytes) and done once, not per-lint call.
        let corrections_ac = corrections_arc.as_ref().and_then(|corrections| {
            // Sort by key for deterministic pattern ordering — HashMap
            // iteration order is random (hash seed varies per process),
            // and AhoCorasick pattern IDs depend on insertion order.
            let mut active: Vec<(Box<str>, Box<str>)> = corrections
                .iter()
                .filter(|(k, v)| k != v && k.as_str() != "//")
                .map(|(k, v)| (k.as_str().into(), v.as_str().into()))
                .collect();
            active.sort_by(|(a, _), (b, _)| a.cmp(b));
            if active.is_empty() {
                return None;
            }
            let patterns: Vec<&str> = active.iter().map(|(k, _)| k.as_ref()).collect();
            match AhoCorasick::new(&patterns) {
                Ok(ac) => Some(CachedAhoCorasick { ac, active }),
                Err(e) => {
                    tracing::warn!(
                        "failed to build AhoCorasick automaton for corrections map \
                         ({} patterns): {e}; pre-scanner text corrections disabled",
                        patterns.len()
                    );
                    None
                }
            }
        });

        // Phase-partition walk. Read every registered rule's declared
        // `Phase` and partition the rule set into a
        // pass-1 (Localized) list, a pass-2 (WholeMarking) list, and
        // (issue #461) a pass-finalization (PageFinalization) list,
        // each indexed by `(rule_set_index, rule_index_within_set)`.
        // The walk runs once at construction time; per-document
        // dispatch reads the cached partition. Phase partition stored
        // but unused in 7a; 7b restructures `fix_inner` to dispatch on
        // it; the issue #461 third bucket is read by
        // `dispatch_page_finalization` at every PageBreak boundary
        // and at EOD.
        let (pass1_rule_indices, pass2_rule_indices, pass_finalization_rule_indices) =
            partition_rules_by_phase(&rule_sets);

        // Pre-resolve all rule severity overrides into indexed lookup
        // tables consumed by the lint hot loop. Drops the per-candidate
        // `config.rules.overrides` HashMap probes + per-candidate
        // `Severity::parse_config` parses from Sites A/B/C/D in
        // `lint_inner` (a perf-only refactor — semantics preserved
        // byte-for-byte at the audit boundary).
        let (fast_path_severities, emitted_id_overrides) = build_severity_tables(
            &rule_sets,
            &config.rules.overrides,
            scheme.bridge_emitted_rule_ids(),
        );

        Ok(Self {
            config,
            rule_sets,
            scheme,
            clock,
            corrections_arc,
            corrections_ac,
            scheduled_rewrites,
            recognizer,
            #[cfg(feature = "corpus-override")]
            corpus_override: None,
            pass1_rule_indices,
            pass2_rule_indices,
            pass_finalization_rule_indices,
            fast_path_severities,
            emitted_id_overrides,
            // Default to `NoopSink`. Constitution Principle I — the
            // sink is `#[cfg(feature = "decision-tracing")]`-only, so
            // the OFF-feature build has no field, no Mutex lock, and
            // no allocation; SC-001 is preserved by construction.
            // ON-feature builds default to the ZST `NoopSink` boxed
            // behind `Mutex<Box<dyn SyncDecisionSink>>`; the dispatch
            // is necessarily through the vtable (not monomorphized
            // away), but `NoopSink::record` is `#[inline(always)]`
            // with an empty body, so every `emit()` call pays the
            // step-counter `fetch_add` + `Mutex::lock` + vtable call
            // and then immediately returns. Callers that want real
            // instrumentation call [`Engine::with_decision_sink`] to
            // install a non-Noop sink.
            #[cfg(feature = "decision-tracing")]
            sink: std::sync::Mutex::new(Box::new(marque_scheme::NoopSink)),
            // No observer until `with_decision_sink` installs one. Gates
            // `Engine::emit` and the scheme-side projection routing onto
            // the lock-free path so the default `NoopSink` costs nothing
            // on the hot path.
            #[cfg(feature = "decision-tracing")]
            tracing_active: false,
            #[cfg(feature = "decision-tracing")]
            next_step: std::sync::atomic::AtomicU32::new(0),
        })
    }
}

// CAPCO-default conveniences and recognizer/sink builders. Pinned to the
// default `Engine<CapcoScheme, EngineRecognizer>` because the `new` /
// `with_clock` conveniences supply `EngineRecognizer::default()`, and the
// `with_recognizer` / `with_strict_recognizer` builders mutate that same
// `EngineRecognizer`. A type-param default does not apply in impl
// position, so the instantiation is spelled out explicitly. Generic
// construction over an arbitrary scheme / recognizer goes through
// [`Engine::with_clock_and_recognizer`] above.
impl Engine<CapcoScheme, EngineRecognizer> {
    /// Create a new engine with the given configuration, rule sets, and
    /// CAPCO marking scheme. Installs the default [`EngineRecognizer`]
    /// (strict-first with a decoder fallback).
    ///
    /// Runs the page-rewrite scheduler (Kahn's algorithm over the
    /// scheme's declared `reads` / `writes` axes) once at construction
    /// time. Cycles and unannotated `Custom` rewrites fail closed with
    /// [`EngineConstructionError`] rather than degrading at lint time.
    ///
    /// Use [`Engine::with_clock`] for deterministic-timestamp testing,
    /// or [`Engine::with_clock_and_recognizer`] to drive a non-CAPCO
    /// scheme / custom recognizer.
    pub fn new(
        config: Config,
        rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>>,
        scheme: CapcoScheme,
    ) -> Result<Self, EngineConstructionError> {
        Self::with_clock(config, rule_sets, scheme, Box::new(SystemClock))
    }

    /// Create a CAPCO engine with a custom clock (for deterministic
    /// tests). Installs the default [`EngineRecognizer`].
    pub fn with_clock(
        config: Config,
        rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>>,
        scheme: CapcoScheme,
        clock: Box<dyn Clock>,
    ) -> Result<Self, EngineConstructionError> {
        Self::with_clock_and_recognizer(
            config,
            rule_sets,
            scheme,
            EngineRecognizer::default(),
            clock,
        )
    }

    /// Override the engine's recognizer. The default installed by
    /// [`Engine::new`] is [`StrictOrDecoderRecognizer`] (strict-first,
    /// decoder fallback). Callers that need to install a custom
    /// recognizer implementation can do so here. For strict-only dispatch
    /// without trait-object dispatch, prefer
    /// [`Engine::with_strict_recognizer`].
    ///
    /// Returns the engine by value so callers can chain:
    ///
    /// ```ignore
    /// let engine = Engine::new(config, rules, scheme)?
    ///     .with_recognizer(Arc::new(MyCustomRecognizer::new()));
    /// ```
    #[must_use = "with_recognizer returns a new Engine; the returned value must be bound for the override to take effect"]
    pub fn with_recognizer(mut self, recognizer: Arc<dyn Recognizer<CapcoScheme>>) -> Self {
        self.recognizer = EngineRecognizer::dynamic(recognizer);
        self
    }

    /// Override the engine recognizer with the strict parser path
    /// without introducing trait-object dispatch.
    ///
    /// Prefer this helper in latency-sensitive strict-only paths (for
    /// example interactive-latency benchmark setups). Use
    /// [`Engine::with_recognizer`] when installing a custom recognizer
    /// implementation.
    ///
    /// ```ignore
    /// let engine = Engine::new(config, rules, scheme)?
    ///     .with_strict_recognizer();
    /// ```
    #[must_use = "with_strict_recognizer returns a new Engine; the returned value must be bound for the override to take effect"]
    pub fn with_strict_recognizer(mut self) -> Self {
        self.recognizer = EngineRecognizer::strict();
        self
    }

    /// Install a [`DecisionSink`](marque_scheme::DecisionSink) on the
    /// engine. Every instrumented decision point (per-rule dispatch,
    /// constraint firing, banner roll-up, scheme-side
    /// `project_with_sink` / `closure_with_sink`) emits one
    /// [`DecisionEvent`](marque_scheme::DecisionEvent) through this
    /// sink during a subsequent [`Engine::lint`] call.
    ///
    /// Only available when the engine is built with the
    /// `decision-tracing` Cargo feature. With the feature off the
    /// method does not exist and the engine carries no sink field —
    /// Constitution Principle I (SC-001 p95 ≤ 2 ms) is preserved by
    /// the absence of any per-call-site branch on the hot path.
    ///
    /// Returns the engine by value so callers can chain:
    ///
    /// ```ignore
    /// let sink = marque_scheme::RecordingSink::new();
    /// let engine = Engine::new(config, rules, scheme)?
    ///     .with_decision_sink(sink);
    /// ```
    ///
    /// Replacing the sink resets the per-document step counter to
    /// zero — events recorded after this call start a fresh cascade
    /// graph.
    #[cfg(feature = "decision-tracing")]
    #[must_use = "with_decision_sink returns a new Engine; the returned value must be bound for the sink to take effect"]
    pub fn with_decision_sink<S>(mut self, sink: S) -> Self
    where
        S: SyncDecisionSink + 'static,
    {
        self.sink = std::sync::Mutex::new(Box::new(sink));
        self.tracing_active = true;
        self.next_step = std::sync::atomic::AtomicU32::new(0);
        self
    }

    /// Install a CLI-supplied corpus override. Only available when
    /// the engine is built with the `corpus-override` Cargo feature
    /// (CLI-only — `marque-server` rejects override input on every
    /// channel, and the WASM crate cannot enable the feature at all).
    ///
    /// Today the engine retains the override for audit-annotation
    /// purposes only. Every subsequent decoder-path fix produced by
    /// [`Engine::lint`] gets a [`FeatureId::CorpusOverrideInEffect`]
    /// feature contribution appended to its `Recognition.features` so an
    /// auditor can identify fixes produced under organizational
    /// overrides vs. stock priors. Substituting the override priors into
    /// the decoder's prior-table lookup is not yet implemented.
    #[cfg(feature = "corpus-override")]
    #[must_use = "with_corpus_override returns a new Engine; the result must be bound to take effect — `engine.with_corpus_override(o)` alone leaves the engine without an override installed"]
    pub fn with_corpus_override(
        mut self,
        override_data: std::sync::Arc<marque_config::corpus_override::CorpusOverride>,
    ) -> Self {
        self.corpus_override = Some(override_data);
        self
    }
}

// Recognizer-agnostic accessors. These read fields that do not depend on
// the recognizer type, so they generalize over `R` and stay callable from
// the `R`-generic lint pipeline (e.g. `lint_helpers` reads
// `corpus_override_active`). Kept separate from the `EngineRecognizer`-
// pinned constructor block so generic-`R` call sites resolve.
impl<S: MarkingScheme, R: Recognizer<S>> Engine<S, R> {
    /// The topologically-sorted rewrite order computed by the scheduler
    /// at construction time.
    ///
    /// Exposed for diagnostic / test inspection. Per-document lint does
    /// not re-sort; this slice is the canonical order every page roll-up
    /// walks.
    pub fn scheduled_rewrites(&self) -> &[RewriteId] {
        &self.scheduled_rewrites
    }

    /// Whether a corpus override is in effect for this engine.
    ///
    /// Returns `false` unconditionally when the `corpus-override`
    /// Cargo feature is not compiled in — the WASM and server
    /// builds therefore cannot observe a `true` here regardless of
    /// what any caller passes through other surfaces. Callers that
    /// need to thread the flag into audit-record construction (the
    /// `build_decoder_diagnostic` helper in marque-capco, reached by the
    /// recognition path through
    /// [`ConstraintBridge::recognition_outcome`]
    /// should go through this method rather than poking at the field
    /// directly.
    #[inline]
    pub fn corpus_override_active(&self) -> bool {
        #[cfg(feature = "corpus-override")]
        {
            self.corpus_override.is_some()
        }
        #[cfg(not(feature = "corpus-override"))]
        {
            false
        }
    }

    /// Borrow the engine's active marking scheme.
    ///
    /// Used by the CLI / WASM audit-record renderers to project
    /// `AuditLine<S>` values through the scheme's
    /// [`Vocabulary`](marque_scheme::Vocabulary) and
    /// [`MarkingScheme::categories`](marque_scheme::MarkingScheme::categories)
    /// surfaces for the audit JSON shape. Off the lint/scan
    /// hot path — purely a wire-format projection helper.
    pub fn scheme(&self) -> &S {
        &self.scheme
    }
}
