use super::dispatch::{
    build_severity_tables, canonicalize_rule_overrides, partition_rules_by_phase,
};
use super::*;

impl Engine {
    /// Create a new engine with the given configuration, rule sets, and
    /// marking scheme.
    ///
    /// Runs the page-rewrite scheduler (Kahn's algorithm over the
    /// scheme's declared `reads` / `writes` axes) once at construction
    /// time. Cycles and unannotated `Custom` rewrites fail closed with
    /// [`EngineConstructionError`] rather than degrading at lint time.
    ///
    /// Use [`Engine::with_clock`] for deterministic-timestamp testing.
    pub fn new<S: MarkingScheme>(
        config: Config,
        rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>>,
        scheme: S,
    ) -> Result<Self, EngineConstructionError> {
        Self::with_clock(config, rule_sets, scheme, Box::new(SystemClock))
    }

    /// Create an engine with a custom clock (for deterministic tests).
    pub fn with_clock<S: MarkingScheme>(
        mut config: Config,
        rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>>,
        scheme: S,
        clock: Box<dyn Clock>,
    ) -> Result<Self, EngineConstructionError> {
        // Instantiate the constraint-catalog bridge's `CapcoScheme`
        // up front so the override canonicalizer can consult its
        // `bridge_emitted_rule_ids()` for IDs the engine emits without
        // a registered `Rule`. The user-supplied generic `scheme: S`
        // is `drop()`-ped below the corrections-map setup and
        // `bridge_scheme` becomes the engine's stored scheme (the
        // `let scheme = bridge_scheme;` step) — see PR 3c.B Commit
        // 7.2's silent-drop note inside that block for the broader
        // design rationale.
        let bridge_scheme = CapcoScheme::new();

        // Canonicalize [rules] overrides against the registered rule
        // set: accept either the rule ID (e.g. "E001") or the rule
        // name (e.g. "portion-mark-in-banner"), resolve both to the
        // canonical ID before the engine stores the map, and hard-fail
        // on any unknown key. See `canonicalize_rule_overrides`.
        canonicalize_rule_overrides(&mut config, &rule_sets, &bridge_scheme)?;

        // PR 3c.B Sub-PR 8.F engine-prereq: validate every
        // `CategoryAction::Intent` payload BEFORE scheduling. Reordering
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
        // Drop the user-supplied scheme after page-rewrite extraction;
        // the constraint-catalog bridge in `lint_inner` uses a fresh
        // `CapcoScheme::new()` (see the `scheme` field doc above for
        // the design rationale).
        //
        // CAUTION (review-pass HIGH): this discard is SILENT. A caller
        // that passes a configured `CapcoScheme` (custom catalog,
        // runtime-amended constraint rows, alternative rewrite axis
        // beyond what we already extracted) loses every customization
        // here. No compile-time guard — the `S: MarkingScheme` bound
        // permits any scheme because the scheduler test
        // (`crates/engine/tests/scheduler.rs:106`) deliberately
        // exercises that flexibility with a `StubScheme`. Every
        // production call site today passes `CapcoScheme::new()` (the
        // default), so the discard is currently lossless; a future
        // refactor that makes `Engine<S>` truly generic over the
        // scheme will close this. The `tracing::debug!` below makes
        // the silent drop observable to a developer running with
        // `MARQUE_LOG=marque=debug` (off by default in
        // production).
        tracing::debug!(
            target: "marque_engine::scheme_discard",
            "user-supplied scheme dropped; constraint-catalog bridge uses default \
             CapcoScheme::new() (a future Engine<S> generic-cleanup PR closes this)"
        );
        drop(scheme);
        Self::with_clock_prepared(config, rule_sets, clock, bridge_scheme, scheduled_rewrites)
    }

    /// Non-generic tail of [`Engine::with_clock`].
    ///
    /// Keeping the heavy construction path behind a concrete signature
    /// avoids monomorphizing the full constructor body for every `S`
    /// used at call sites; only the rewrite-validation/scheduling front
    /// edge remains generic.
    fn with_clock_prepared(
        mut config: Config,
        rule_sets: Vec<Box<dyn RuleSet<CapcoScheme>>>,
        clock: Box<dyn Clock>,
        bridge_scheme: CapcoScheme,
        scheduled_rewrites: Box<[RewriteId]>,
    ) -> Result<Self, EngineConstructionError> {
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

        let scheme = bridge_scheme;

        // PR 7a phase-partition walk (FR-021). Read every registered
        // rule's declared `Phase` and partition the rule set into a
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
            recognizer: EngineRecognizer::default(),
            #[cfg(feature = "corpus-override")]
            corpus_override: None,
            pass1_rule_indices,
            pass2_rule_indices,
            pass_finalization_rule_indices,
            fast_path_severities,
            emitted_id_overrides,
        })
    }

    /// The topologically-sorted rewrite order computed by the scheduler
    /// at construction time.
    ///
    /// Exposed for diagnostic / test inspection. Per-document lint does
    /// not re-sort; this slice is the canonical order every page roll-up
    /// walks.
    pub fn scheduled_rewrites(&self) -> &[RewriteId] {
        &self.scheduled_rewrites
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
        self.recognizer = EngineRecognizer::Dyn(recognizer);
        self
    }

    /// Override the engine recognizer with the strict parser path
    /// without introducing trait-object dispatch.
    ///
    /// Prefer this helper in latency-sensitive strict-only paths (for
    /// example SC-001 benchmark setups). Use [`Engine::with_recognizer`]
    /// when installing a custom recognizer implementation.
    ///
    /// ```ignore
    /// let engine = Engine::new(config, rules, scheme)?
    ///     .with_strict_recognizer();
    /// ```
    #[must_use = "with_strict_recognizer returns a new Engine; the returned value must be bound for the override to take effect"]
    pub fn with_strict_recognizer(mut self) -> Self {
        self.recognizer = EngineRecognizer::Strict(StrictRecognizer::new());
        self
    }

    /// Install a CLI-supplied corpus override. Only available when
    /// the engine is built with the `corpus-override` Cargo feature
    /// (CLI-only — `marque-server` rejects override input on every
    /// channel per T066, and the WASM crate cannot enable the feature
    /// at all per T067).
    ///
    /// Phase 4 PR-5 minimal scope: the engine retains the override
    /// for audit-annotation purposes only. Every subsequent decoder-
    /// path fix produced by [`Engine::lint`] gets a
    /// [`FeatureId::CorpusOverrideInEffect`] feature contribution
    /// appended to its `Confidence.features` so an auditor can
    /// identify fixes produced under organizational overrides vs.
    /// stock priors. Substituting the override priors into the
    /// decoder's prior-table lookup is the next-PR step.
    #[cfg(feature = "corpus-override")]
    #[must_use = "with_corpus_override returns a new Engine; the result must be bound to take effect — `engine.with_corpus_override(o)` alone leaves the engine without an override installed"]
    pub fn with_corpus_override(
        mut self,
        override_data: std::sync::Arc<marque_config::corpus_override::CorpusOverride>,
    ) -> Self {
        self.corpus_override = Some(override_data);
        self
    }

    /// Whether a corpus override is in effect for this engine.
    ///
    /// Returns `false` unconditionally when the `corpus-override`
    /// Cargo feature is not compiled in — the WASM and server
    /// builds therefore cannot observe a `true` here regardless of
    /// what any caller passes through other surfaces. Callers that
    /// need to thread the flag into audit-record construction (the
    /// private `build_decoder_diagnostic` helper inside this module)
    /// should go through this method rather than poking at the
    /// field directly.
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
    /// Used by the CLI / WASM audit-record renderers (PR 3c.2.D / D4)
    /// to project [`AuditLine<CapcoScheme>`] values through the
    /// scheme's [`Vocabulary`](marque_scheme::Vocabulary) and
    /// [`MarkingScheme::categories`](marque_scheme::MarkingScheme::categories)
    /// surfaces for the `marque-1.0` JSON shape. Off the lint/scan
    /// hot path — purely a wire-format projection helper.
    pub fn scheme(&self) -> &CapcoScheme {
        &self.scheme
    }
}
