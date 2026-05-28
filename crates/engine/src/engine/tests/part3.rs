use super::*;

fn named_rule_set(rules: &[(&'static str, &'static str)]) -> Box<dyn RuleSet<CapcoScheme>> {
    let rules: Vec<Box<dyn Rule<CapcoScheme>>> = rules
        .iter()
        .map(|(id, name)| Box::new(NamedStub { id, name }) as Box<dyn Rule<CapcoScheme>>)
        .collect();
    Box::new(StubSet(rules))
}

fn config_with_overrides(pairs: &[(&str, &str)]) -> Config {
    let mut config = Config::default();
    for (k, v) in pairs {
        config
            .rules
            .overrides
            .insert((*k).to_owned(), (*v).to_owned());
    }
    config
}

#[test]
fn canonicalize_accepts_rule_id_form_unchanged() {
    let mut config = config_with_overrides(&[("E001", "warn")]);
    let sets = vec![named_rule_set(&[("E001", "portion-mark-in-banner")])];
    canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new()).expect("should succeed");
    assert_eq!(
        config.rules.overrides.get("E001"),
        Some(&"warn".to_owned()),
        "ID-form override keeps its key"
    );
}

#[test]
fn canonicalize_accepts_rule_name_form_and_resolves_to_id() {
    let mut config = config_with_overrides(&[("portion-mark-in-banner", "error")]);
    let sets = vec![named_rule_set(&[("E001", "portion-mark-in-banner")])];
    canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new()).expect("should succeed");
    assert_eq!(
        config.rules.overrides.get("E001"),
        Some(&"error".to_owned()),
        "name-form override resolves to canonical ID"
    );
    assert!(
        !config
            .rules
            .overrides
            .contains_key("portion-mark-in-banner"),
        "pre-canonicalization name key must not survive"
    );
}

#[test]
fn canonicalize_rejects_unknown_key_with_suggestion_for_near_miss() {
    let mut config = config_with_overrides(&[("E00l", "warn")]); // lowercase-L, not 1
    let sets = vec![named_rule_set(&[("E001", "portion-mark-in-banner")])];
    let err = canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new()).unwrap_err();
    match err {
        EngineConstructionError::UnknownRuleOverride { key, did_you_mean } => {
            assert_eq!(key, "E00l");
            assert_eq!(
                did_you_mean.as_deref(),
                Some("E001"),
                "single-character typo should suggest the canonical ID"
            );
        }
        other => panic!("expected UnknownRuleOverride, got {other:?}"),
    }
}

#[test]
fn canonicalize_rejects_unknown_key_without_suggestion_when_nothing_close() {
    // No candidate is within edit distance 3, so did_you_mean must be None
    // — a nonsense suggestion is worse than no suggestion.
    let mut config = config_with_overrides(&[("totally-made-up-rule-name", "error")]);
    let sets = vec![named_rule_set(&[("E001", "portion-mark-in-banner")])];
    let err = canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new()).unwrap_err();
    match err {
        EngineConstructionError::UnknownRuleOverride { key, did_you_mean } => {
            assert_eq!(key, "totally-made-up-rule-name");
            assert!(
                did_you_mean.is_none(),
                "distant misses must not emit a suggestion; got {did_you_mean:?}"
            );
        }
        other => panic!("expected UnknownRuleOverride, got {other:?}"),
    }
}

#[test]
fn canonicalize_rejects_conflicting_id_and_name_forms_with_different_severity() {
    let mut config =
        config_with_overrides(&[("E001", "warn"), ("portion-mark-in-banner", "error")]);
    let sets = vec![named_rule_set(&[("E001", "portion-mark-in-banner")])];
    let err = canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new()).unwrap_err();
    match err {
        EngineConstructionError::ConflictingRuleOverride {
            rule_id,
            keys,
            severities,
        } => {
            assert_eq!(rule_id, "E001");
            // HashMap iteration order isn't deterministic — verify by set.
            let k: std::collections::HashSet<&str> = keys.iter().map(|s| s.as_str()).collect();
            assert!(k.contains("E001"));
            assert!(k.contains("portion-mark-in-banner"));
            let s: std::collections::HashSet<&str> =
                severities.iter().map(|s| s.as_str()).collect();
            assert!(s.contains("warn"));
            assert!(s.contains("error"));
        }
        other => panic!("expected ConflictingRuleOverride, got {other:?}"),
    }
}

#[test]
fn canonicalize_accepts_duplicate_forms_with_same_severity() {
    // A user who writes both `E001 = "warn"` and `portion-mark-in-banner
    // = "warn"` (e.g., via copy-paste across layers) is unambiguous and
    // should not be punished.
    let mut config = config_with_overrides(&[("E001", "warn"), ("portion-mark-in-banner", "warn")]);
    let sets = vec![named_rule_set(&[("E001", "portion-mark-in-banner")])];
    canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new())
        .expect("duplicate forms with same severity must succeed");
    assert_eq!(config.rules.overrides.len(), 1);
    assert_eq!(config.rules.overrides.get("E001"), Some(&"warn".to_owned()));
}

#[test]
fn canonicalize_accepts_overrides_across_multiple_rule_sets() {
    // Two rule sets registered; aliases from each must resolve.
    let mut config = config_with_overrides(&[
        ("portion-mark-in-banner", "error"), // name from set A
        ("M500", "warn"),                    // ID from set B
    ]);
    let sets = vec![
        named_rule_set(&[("E001", "portion-mark-in-banner")]),
        named_rule_set(&[("M500", "some-other-domain-rule")]),
    ];
    canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new()).expect("should succeed");
    assert_eq!(
        config.rules.overrides.get("E001"),
        Some(&"error".to_owned())
    );
    assert_eq!(config.rules.overrides.get("M500"), Some(&"warn".to_owned()));
}

#[test]
fn canonicalize_empty_overrides_is_noop() {
    let mut config = Config::default();
    let sets = vec![named_rule_set(&[("E001", "portion-mark-in-banner")])];
    canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new())
        .expect("empty overrides must succeed");
    assert!(config.rules.overrides.is_empty());
}

// Bridge-emitted rule IDs (no registered `Rule` impl). The
// canonicalizer consults `CapcoScheme::bridge_emitted_rule_ids()` so
// `.marque.toml` keys referencing the bridged catalog rows or their
// descriptive aliases are accepted rather than failing
// `UnknownRuleOverride`. These tests pin the four key forms +
// canonical-ID resolution so the bridge path can't silently regress.

// `bridge_emitted_rule_ids` returns `(wire_string, descriptive_alias)`
// pairs; each bridged catalog row has its own per-row wire string. The
// tests below pin the four key forms — wire string, predicate-id
// alone, descriptive alias, and a representative class-floor /
// sci-per-system row.
//
// Pick one row from the SCI per-system catalog
// (`marking.sci.hcs-o-companions`) and one from the class-floor
// catalog (`banner.classification.floor-hcs-comp-sub`) as the
// representative entries. Future bridge-row additions should
// re-verify the canonicalize round-trip via this pattern.

#[test]
fn canonicalize_accepts_bridge_emitted_wire_string_form() {
    // Users type the wire-string form
    // `"capco:marking.sci.hcs-o-companions"` in `.marque.toml`;
    // canonicalize reduces to the predicate-id intern.
    let mut config = config_with_overrides(&[("capco:marking.sci.hcs-o-companions", "warn")]);
    let sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![];
    canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new())
        .expect("bridge-emitted wire-string form must be accepted");
    assert_eq!(
        config.rules.overrides.get("marking.sci.hcs-o-companions"),
        Some(&"warn".to_owned()),
        "wire-string config key canonicalizes to predicate-id intern"
    );
}

#[test]
fn canonicalize_accepts_bridge_emitted_predicate_id_form() {
    // The predicate-id alone (no `capco:` prefix) is also a valid
    // config key form — it's what `RuleId::predicate_id()` returns
    // and what shows up in audit-log searches.
    let mut config = config_with_overrides(&[("marking.sci.hcs-o-companions", "off")]);
    let sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![];
    canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new())
        .expect("bridge-emitted predicate-id form must be accepted");
    assert_eq!(
        config.rules.overrides.get("marking.sci.hcs-o-companions"),
        Some(&"off".to_owned()),
        "predicate-id config key resolves to itself"
    );
}

#[test]
fn canonicalize_accepts_bridge_emitted_descriptive_alias() {
    // The descriptive alias (second column of
    // `bridge_emitted_rule_ids`) is a third permissible config-key
    // form. It canonicalizes to the predicate-id intern.
    let mut config = config_with_overrides(&[("sci-per-system-hcs-o-companions", "error")]);
    let sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![];
    canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new())
        .expect("bridge-emitted descriptive alias must be accepted");
    assert_eq!(
        config.rules.overrides.get("marking.sci.hcs-o-companions"),
        Some(&"error".to_owned()),
        "descriptive-alias config key canonicalizes to predicate-id intern"
    );
    assert!(
        !config
            .rules
            .overrides
            .contains_key("sci-per-system-hcs-o-companions"),
        "pre-canonicalization alias key must not survive"
    );
}

#[test]
fn canonicalize_accepts_class_floor_wire_string() {
    // Same round-trip for a class-floor catalog row (per Agent A's
    // rename to wire-string form in `bridge_emitted_rule_ids`).
    let mut config =
        config_with_overrides(&[("capco:banner.classification.floor-hcs-comp-sub", "off")]);
    let sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![];
    canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new())
        .expect("class-floor wire-string form must be accepted");
    assert_eq!(
        config
            .rules
            .overrides
            .get("banner.classification.floor-hcs-comp-sub"),
        Some(&"off".to_owned()),
        "class-floor wire-string canonicalizes to predicate-id intern"
    );
}

#[test]
fn canonicalize_rejects_legacy_walker_id_with_unknown_rule_override() {
    // Regression guard: the retired walker IDs (E022 / E025 / E027
    // for class-floor; E042-E051 for SCI per-system) MUST NOT be
    // silently accepted as aliases for E058 / E059. Per project
    // memory `feedback_pre_users_no_deprecation_phasing.md` marque
    // is pre-users; legacy ID acceptance would be a deprecation-
    // phasing mechanism we don't carry.
    let mut config = config_with_overrides(&[("E022", "warn")]);
    let sets: Vec<Box<dyn RuleSet<CapcoScheme>>> = vec![];
    let err = canonicalize_rule_overrides(&mut config, &sets, &CapcoScheme::new())
        .expect_err("retired legacy ID E022 must NOT be silently aliased to E058");
    match err {
        EngineConstructionError::UnknownRuleOverride { key, .. } => {
            assert_eq!(key, "E022");
        }
        other => panic!("expected UnknownRuleOverride for E022, got {other:?}"),
    }
}

#[test]
fn unknown_rule_override_exit_code_is_dataerr() {
    let err = EngineConstructionError::UnknownRuleOverride {
        key: "E999".into(),
        did_you_mean: None,
    };
    assert_eq!(err.exit_code(), 65, "EX_DATAERR for user-config errors");
}

#[test]
fn conflicting_rule_override_exit_code_is_dataerr() {
    let err = EngineConstructionError::ConflictingRuleOverride {
        rule_id: "E001".into(),
        keys: Box::new(["E001".into(), "portion-mark-in-banner".into()]),
        severities: Box::new(["warn".into(), "error".into()]),
    };
    assert_eq!(err.exit_code(), 65);
}

#[test]
fn rewrite_cycle_exit_code_is_unavailable() {
    // Scheme defects (not user-config errors) stay on EX_UNAVAILABLE.
    use marque_scheme::CategoryId;
    let err = EngineConstructionError::RewriteCycle {
        axis: CategoryId(0),
        members: Box::new(["a", "b"]),
    };
    assert_eq!(err.exit_code(), 69);
}

#[test]
fn levenshtein_matches_reference_values() {
    // Spot-check against hand-computed distances to catch regressions
    // in the DP implementation.
    assert_eq!(super::levenshtein("", ""), 0);
    assert_eq!(super::levenshtein("E001", "E001"), 0);
    assert_eq!(super::levenshtein("E001", "E002"), 1);
    assert_eq!(super::levenshtein("E001", "E00l"), 1);
    assert_eq!(super::levenshtein("kitten", "sitting"), 3);
    assert_eq!(super::levenshtein("", "abc"), 3);
    assert_eq!(super::levenshtein("abc", ""), 3);
}

#[test]
fn suggest_closest_prefers_smaller_distance() {
    let cands = ["E001", "E002", "E010"];
    // "E00l" has dist 1 to E001 and dist 1 to E002 (single substitution),
    // and dist 2 to E010. E001 should win the tie-break because it appears
    // first among the equally close candidates.
    assert_eq!(
        super::suggest_closest("E00l", cands.iter().copied()),
        Some("E001".to_owned())
    );
}

#[test]
fn suggest_closest_returns_none_when_nothing_is_close_enough() {
    let cands = ["portion-mark-in-banner", "missing-usa-trigraph"];
    // Very short needle with no near neighbors — threshold is 1 for
    // length 3, and the closest candidate is many edits away.
    assert!(super::suggest_closest("xyz", cands.iter().copied()).is_none());
}

// -------------------------------------------------------------------
// `build_r002_diagnostic` shape pin
// -------------------------------------------------------------------
//
// R002 is a synthetic diagnostic emitted when the post-pass-1 buffer
// cannot be re-parsed. Constitution V Principle V forbids R002 from
// becoming an `AppliedFix` audit record — R002 carries no replacement
// bytes and represents no action taken. The function-level pin below
// exercises `build_r002_diagnostic` directly so the no-promotion
// contract is enforced at the unit-test layer, not only at the
// integration layer where R002 can fire today (which is never — no
// production `Phase::Localized` rule emits a `FixIntent` that could
// trigger R002 yet). The integration-layer test
// (`audit_completeness.rs::r002_does_not_mint_applied_fix`) becomes
// load-bearing when a future Localized rule lands; this unit test
// makes the shape invariant load-bearing today.

#[test]
fn build_r002_diagnostic_returns_diagnostic_not_appliedfix() {
    use smallvec::smallvec;

    let contributing: SmallVec<[RuleId; 4]> = smallvec![
        RuleId::new("capco", "marking.correction.token-typo"),
        RuleId::new("capco", "marking.deprecation.deprecated-dissem-control"),
    ];
    let failure_span = Span::new(0, 64);
    let diag = super::build_r002_diagnostic(contributing, failure_span);

    // The function returns a `Diagnostic<CapcoScheme>`; the type
    // system already forbids it from being an `AppliedFix`. The
    // assertions below pin the structural shape that complements
    // the type-level guarantee: R002 carries no FixIntent and no
    // TextCorrection — the two channels through which a `Diagnostic`
    // can subsequently be promoted to an `AppliedFix` by the engine.
    assert_eq!(diag.rule, super::R002_RULE_ID);
    assert_eq!(diag.severity, Severity::Error);
    assert_eq!(diag.span, failure_span);
    assert!(diag.fix.is_none(), "R002 must carry no FixIntent");
    assert!(
        diag.text_correction.is_none(),
        "R002 must carry no TextCorrection"
    );
    // The typed `Message` carries the contributing rule IDs
    // structurally via `MessageArgs.contributing_rule_ids` (closed-set
    // permitted type). The args check asserts on a closed type rather
    // than a string substring.
    assert_eq!(diag.message.template(), MessageTemplate::ReparseFailed);
    let contributors = &diag.message.args().contributing_rule_ids;
    assert!(
        contributors
            .iter()
            .any(|id| id.predicate_id() == "marking.correction.token-typo")
    );
    assert!(
        contributors
            .iter()
            .any(|id| id.predicate_id() == "marking.deprecation.deprecated-dissem-control")
    );
}

#[test]
fn build_r002_diagnostic_empty_contributors_uses_generic_message() {
    let contributing: SmallVec<[RuleId; 4]> = SmallVec::new();
    let failure_span = Span::new(0, 0);
    let diag = super::build_r002_diagnostic(contributing, failure_span);

    assert_eq!(diag.rule, super::R002_RULE_ID);
    assert!(diag.fix.is_none());
    assert!(diag.text_correction.is_none());
    // Empty-contributors branch identified by an empty
    // `contributing_rule_ids` SmallVec, not by message substring.
    assert_eq!(diag.message.template(), MessageTemplate::ReparseFailed);
    assert!(diag.message.args().contributing_rule_ids.is_empty());
}

// -------------------------------------------------------------------
// Partition + re-lint data-flow locks
// -------------------------------------------------------------------
//
// The re-parse arm of `TwoPassFixer::run` must re-partition
// `relint.diagnostics` and feed pass-2 the fresh post-pass-1
// WholeMarking slice — NOT the stale pre-pass-1 partition. Tests below
// pin the partition logic in isolation and lock the data-flow contract
// via a stub Phase::Localized
// FixIntent rule that mutates the buffer.

#[test]
fn partition_diags_by_phase_routes_by_localized_id_set() {
    // The partition predicate: rule IDs in `localized_ids` go to
    // pass-1; everything else goes to pass-2. text_correction
    // diagnostics with no `fix` are excluded from BOTH partitions.
    //
    // Synthetic test ids in the `"test"` reserved scheme;
    // `localized_ids` keys on the predicate-id half.
    let localized: HashSet<&'static str> = ["e006", "e007", "c001"].into_iter().collect();

    let pass1_id = Diagnostic::<CapcoScheme>::new(
        RuleId::new("test", "e006"),
        Severity::Error,
        Span::new(0, 4),
        stub_message(),
        stub_citation(),
        None,
    );
    let pass2_id = Diagnostic::<CapcoScheme>::new(
        RuleId::new("test", "e022"),
        Severity::Error,
        Span::new(4, 8),
        stub_message(),
        stub_citation(),
        None,
    );
    let unknown_id = Diagnostic::<CapcoScheme>::new(
        RuleId::new("test", "e999"),
        Severity::Error,
        Span::new(8, 12),
        stub_message(),
        stub_citation(),
        None,
    );
    let text_corr_no_fix = Diagnostic::text_correction(
        RuleId::new("test", "c001"),
        Severity::Fix,
        Span::new(12, 16),
        stub_message(),
        stub_citation(),
        "REPL",
        FixSource::CorrectionsMap,
        marque_rules::Recognition::strict(),
        None,
    );

    let diags = vec![
        pass1_id.clone(),
        pass2_id.clone(),
        unknown_id.clone(),
        text_corr_no_fix.clone(),
    ];
    let (p1, p2) = super::partition_diags_by_phase(&diags, &localized);

    // Pass-1: e006 only (c001's text-correction with no fix is
    // excluded; pass-0 already promoted it or marked it as a sub-
    // threshold suggestion the remaining-diagnostics filter handles).
    assert_eq!(p1.len(), 1);
    assert_eq!(p1[0].rule.predicate_id(), "e006");

    // Pass-2: e022 (declared) + e999 (unknown id ⇒ default to pass-2).
    assert_eq!(p2.len(), 2);
    let p2_ids: Vec<&str> = p2.iter().map(|d| d.rule.predicate_id()).collect();
    assert!(p2_ids.contains(&"e022"));
    assert!(p2_ids.contains(&"e999"));
    // text_correction-no-fix excluded from both:
    for d in p1.iter().chain(p2.iter()) {
        assert_ne!(
            d.rule.predicate_id(),
            "c001",
            "text_correction-no-fix must be excluded from both partitions"
        );
    }
}

#[test]
fn partition_diags_by_phase_returns_references_not_clones() {
    // Regression test: the partition MUST
    // return reference vectors borrowing from `diagnostics`, not
    // cloned owned vectors. The cloning shape allocated O(N)
    // Diagnostic bodies on every call, and `partition_diags_by_phase`
    // is called up to twice per fix on the hot path — that
    // allocation cost is unjustified under Constitution I.
    //
    // Behavioral lock: build an input slice, partition it, and
    // verify each entry in the returned partitions is pointer-
    // equal (via `std::ptr::eq`) to its source entry. Pointer
    // identity is the load-bearing assertion — a clone-based
    // partition would produce structurally-equal but
    // pointer-distinct Diagnostics, failing the test.
    let localized: HashSet<&'static str> = ["e006"].into_iter().collect();

    let diags = vec![
        Diagnostic::<CapcoScheme>::new(
            RuleId::new("test", "e006"),
            Severity::Error,
            Span::new(0, 4),
            stub_message(),
            stub_citation(),
            None,
        ),
        Diagnostic::<CapcoScheme>::new(
            RuleId::new("test", "e022"),
            Severity::Error,
            Span::new(4, 8),
            stub_message(),
            stub_citation(),
            None,
        ),
    ];
    let (p1, p2) = super::partition_diags_by_phase(&diags, &localized);
    assert_eq!(p1.len(), 1);
    assert_eq!(p2.len(), 1);
    assert!(
        std::ptr::eq(p1[0], &diags[0]),
        "pass-1 partition entry must be a reference to the original Diagnostic"
    );
    assert!(
        std::ptr::eq(p2[0], &diags[1]),
        "pass-2 partition entry must be a reference to the original Diagnostic"
    );
}

#[test]
fn partition_diags_by_phase_includes_text_correction_with_fix_in_partition() {
    // A text_correction diagnostic that ALSO carries a `fix` is a
    // sub-threshold suggestion — it stays in the partition routed by
    // its rule id's phase (so the engine's remaining-diagnostics
    // filter can re-surface it as a Suggest).
    let localized: HashSet<&'static str> = ["c001"].into_iter().collect();

    let mut tc = Diagnostic::text_correction(
        RuleId::new("test", "c001"),
        Severity::Fix,
        Span::new(0, 6),
        stub_message(),
        stub_citation(),
        "SECRET",
        FixSource::CorrectionsMap,
        marque_rules::Recognition::strict(),
        None,
    );
    tc.fix = Some(FixIntent::<CapcoScheme> {
        replacement: ReplacementIntent::Recanonicalize {
            scope: RecanonScope::Portion,
        },
        confidence: marque_rules::Recognition::strict(),
        feature_ids: SmallVec::new(),
        // Phase-partition filtering test keyed on rule phase; message
        // templates are irrelevant here. Reuse the shared stub so the
        // fixture makes no template-parity claim (issue #709 removed the
        // prior hardcoded `BannerRollupMismatch`).
        message: stub_message(),
        source: FixSource::BuiltinRule,
        migration_ref: None,
    });

    // Bind the input array to a named local so the reference
    // partition outlives the assertion —
    // `partition_diags_by_phase` returns reference vectors, so the
    // source `[Diagnostic]` must remain live.
    let diags = [tc];
    let (p1, p2) = super::partition_diags_by_phase(&diags, &localized);
    assert_eq!(p1.len(), 1, "text_correction WITH fix → pass-1 (c001)");
    assert_eq!(p2.len(), 0);
}

#[test]
fn pass1_localized_fixintent_run_dispatches_pass2_with_fresh_relint() {
    // Scenario: a stub `Phase::Localized` rule that emits a
    // `FixIntent` whose application changes the buffer in a way the
    // pre-pass-1 lint could not have seen. After pass-1, the engine
    // MUST re-lint the post-pass-1 buffer and partition the FRESH
    // diagnostics for pass-2 — NOT reuse the stale pre-pass-1
    // partition.
    //
    // The behavioral lock: after the fix runs, `FixResult.applied`
    // contains the stub rule's fix at the post-pass-0 span, and
    // pass-2's diagnostic dispatch operates against the post-pass-1
    // buffer's marking shape (verifiable through engine-level
    // outputs — `result.source` after Apply, `remaining_diagnostics`
    // reflecting the post-pass-1 state).
    //
    // Today no production `Phase::Localized` rule emits a FixIntent,
    // so this stub-based test is the load-bearing pin for the
    // re-partition data flow. When a future Localized FixIntent rule
    // lands, integration-level fixtures cover the same path.

    struct LocalizedFixIntentStub;
    impl Rule<CapcoScheme> for LocalizedFixIntentStub {
        fn id(&self) -> RuleId {
            // Test-fixture synthetic id in `"test"` scheme.
            RuleId::new("test", "synthetic.e899-fixture")
        }
        fn name(&self) -> &'static str {
            "stub-localized-fixintent"
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
            // Emit a Recanonicalize FixIntent at the marking's
            // portion span. CapcoScheme will recanonicalize the
            // portion in `apply_intent`, which produces a real
            // byte-level rewrite. The diagnostic's `span` is a
            // sub-region (the NOFORN token within the marking)
            // so the Localized span-shape filter accepts it;
            // `candidate_span` is the full marking span so the
            // synthesize step can look up the parsed marking.
            let intent = FixIntent::<CapcoScheme> {
                replacement: ReplacementIntent::Recanonicalize {
                    scope: RecanonScope::Portion,
                },
                confidence: marque_rules::Recognition::strict(),
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
                RuleId::new("test", "synthetic.e899-fixture"),
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

    let result = engine.fix(TEST_SRC, FixMode::Apply);

    // The engine ran without panicking, returned a coherent
    // FixResult, and did NOT trigger R002 (the post-pass-1
    // buffer still parses). This is the data-flow integration
    // sanity for the re-partition arm — if pass-2 had been fed
    // stale pre-pass-1 diagnostics whose spans no longer matched
    // the post-pass-1 buffer, the engine would panic on the
    // splice slice (or assert in debug). Reaching here cleanly
    // is the lock.
    assert!(
        !result.r002_fired,
        "stub rule's fix does not collapse the marking, so R002 must not fire"
    );
    // Pass-1's fix was applied — at least one `AppliedFix`
    // carries the stub rule's ID.
    let applied_view = applied_fixes(&result);
    let saw_stub_fix = applied_view
        .iter()
        .any(|f| f.rule.predicate_id() == "synthetic.e899-fixture");
    assert!(
        saw_stub_fix,
        "stub localized FixIntent rule's fix must be promoted into AppliedFix; \
             applied: {:?}",
        applied_view
            .iter()
            .map(|f| f.rule.predicate_id())
            .collect::<Vec<_>>(),
    );
}
