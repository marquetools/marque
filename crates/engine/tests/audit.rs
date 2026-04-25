// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T056 — audit-stream content-ignorance tests.
//!
//! Enforces Constitution V + the G13 invariant (Phase 004 §spec,
//! `docs/security/WHITEPAPER.md` §3.1): no document content, metadata
//! field values, or subject-claim free-form text appears in the
//! text-bearing fields of an `AppliedFix` that the audit stream
//! serializes, nor in `Diagnostic.message`.
//!
//! ## Scope of the check
//!
//! The check greps for distinctive prose sentinels in:
//!
//! - `AppliedFix.proposal.original` — the bytes replaced at the fix
//!   span. Should equal the marking span, nothing more.
//! - `AppliedFix.proposal.replacement` — the canonical marking the
//!   fix writes back. Should contain only marking tokens.
//! - `Diagnostic.message` — the human-readable diagnostic. Should
//!   interpolate token canonicals, never prose.
//!
//! These are the only caller-content-bearing string fields that reach
//! the audit or lint output streams. The remaining fields in the
//! serialized audit record are enum-typed (`rule`, `source`,
//! `severity`), numeric (`confidence`, span offsets), process-supplied
//! opaque identifiers (`classifier_id`, `input`, `timestamp`),
//! `&'static str` references (`migration_ref`), or enum-typed feature
//! labels (`FeatureId` — landed via FR-012). None carry document-
//! derived text by type, so they are not greppable targets for this
//! invariant. The test intentionally does not re-invoke the CLI's
//! NDJSON serializer (`marque::render::applied_fix_to_audit_json`),
//! since serialization is a pure projection of these fields — if the
//! source fields are clean, the NDJSON line they produce is clean.
//!
//! ## Strategy
//!
//! 1. **Corpus sweep.** Run `Engine::fix` over every fixture under
//!    `tests/corpus/{invalid,valid,prose}/`. For every `AppliedFix`,
//!    grep `proposal.{original, replacement}` against a sentinel list.
//!
//! 2. **Marking-in-prose composites.** Synthesize documents by
//!    concatenating ~2 KB of prose bytes, an invalid fixture, and
//!    ~2 KB of prose bytes. Exercises the realistic case where a
//!    marking lives inside a larger document — a rule whose span
//!    expands past the marking boundary is caught here even when
//!    stand-alone fixtures miss it. Concatenation is byte-level,
//!    matching the engine's `&[u8]` API contract; UTF-8 validity of
//!    individual marking candidates is handled by the engine's own
//!    parser (non-UTF-8 candidates yield `CoreError::InvalidUtf8`
//!    and are skipped there, not by the test).
//!
//! 3. **Companion: diagnostic messages.** Not T056 proper (T056 is
//!    scoped to the audit stream), but the same invariant applies to
//!    `Diagnostic.message` since that field is emitted on the lint
//!    NDJSON stream. Kept here because the sentinel infrastructure is
//!    identical.
//!
//! 4. **Self-test.** A synthetic `AppliedFix` with a known leak is
//!    passed through the sentinel checker and must panic. Proves the
//!    check is load-bearing — a future refactor that nulled out the
//!    sentinel list would fail this test immediately instead of
//!    silently weakening the real corpus sweep.
//!
//! ## Sentinel selection
//!
//! Sentinels are phrases from `tests/corpus/prose/article.txt`
//! (public-domain Federalist Papers prose). Each sentinel is long
//! and distinctive enough that it cannot appear in any CAPCO/ISM
//! marking by construction — no classification level, compartment,
//! dissem control, or trigraph contains English words with spaces.

use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixResult, FixedClock};
use marque_ism::Span;
use marque_rules::{AppliedFix, Confidence, FixProposal, FixSource, RuleId};
use marque_test_utils::{invalid_fixtures, load_fixture, prose_fixtures, valid_fixtures};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

const FIXED_TS: u64 = 1_700_000_000;

/// Prose sentinels drawn from `tests/corpus/prose/article.txt`.
///
/// Each phrase is a multi-word English fragment that cannot appear in
/// any valid CAPCO/ISM marking. A match in any audit field means the
/// engine leaked document content into the compliance output stream.
const PROSE_SENTINELS: &[&str] = &[
    "republic has over a democracy",
    "numerous advantages promised",
    "Liberty is to faction what air",
    "insuperable obstacle to a uniformity",
    "early prevalence of these sentiments",
    "distinct interests in society",
    "various and interfering interests",
    "adjust these clashing interests",
    "protection of these faculties",
    "principal task of modern legislation",
    "judge in his own cause",
    "enlightened statesmen",
];

fn test_engine() -> Engine {
    Engine::with_clock(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

fn run_fix(engine: &Engine, source: &[u8]) -> FixResult {
    engine.fix(source, FixMode::Apply)
}

/// Panic if any prose sentinel appears in the given string.
///
/// Panic includes the rule ID and span so a failure points directly at
/// the offending proposal without requiring test re-runs to find it.
fn assert_clean(proposal: &FixProposal, field_name: &str, value: &str, context: &str) {
    for sentinel in PROSE_SENTINELS {
        if value.contains(sentinel) {
            panic!(
                "G13 violation: prose sentinel {sentinel:?} leaked into \
                 AppliedFix.proposal.{field_name} \
                 (rule: {rule}, span: {start}..{end}, context: {context})\n\n\
                 field value: {value:?}",
                rule = proposal.rule.as_str(),
                start = proposal.span.start,
                end = proposal.span.end,
            );
        }
    }
}

/// Check every AppliedFix in `applied` for sentinel leaks.
fn check_fixes_clean(applied: &[AppliedFix], context: &str) {
    for fix in applied {
        let p = &fix.proposal;
        assert_clean(p, "original", p.original.as_ref(), context);
        assert_clean(p, "replacement", p.replacement.as_ref(), context);
    }
}

// ---------------------------------------------------------------------------
// Corpus sweep — every fixture must not leak into AppliedFix fields.
// ---------------------------------------------------------------------------

#[test]
fn no_document_text_leaks_from_invalid_corpus() {
    let engine = test_engine();
    let fixtures = invalid_fixtures();
    assert!(
        !fixtures.is_empty(),
        "no invalid fixtures found — cannot validate G13"
    );
    for path in &fixtures {
        let source = load_fixture(path);
        let result = run_fix(&engine, &source);
        check_fixes_clean(&result.applied, &path.display().to_string());
    }
}

#[test]
fn no_document_text_leaks_from_valid_corpus() {
    let engine = test_engine();
    let fixtures = valid_fixtures();
    assert!(
        !fixtures.is_empty(),
        "no valid fixtures found — cannot validate G13"
    );
    for path in &fixtures {
        let source = load_fixture(path);
        let result = run_fix(&engine, &source);
        check_fixes_clean(&result.applied, &path.display().to_string());
    }
}

#[test]
fn no_document_text_leaks_from_prose_corpus() {
    let engine = test_engine();
    let fixtures = prose_fixtures();
    assert!(
        !fixtures.is_empty(),
        "no prose fixtures found — cannot validate G13"
    );
    for path in &fixtures {
        let source = load_fixture(path);
        let result = run_fix(&engine, &source);
        check_fixes_clean(&result.applied, &path.display().to_string());
    }
}

// ---------------------------------------------------------------------------
// Marking-in-prose composites — the realistic "embedded marking" case.
// ---------------------------------------------------------------------------

#[test]
fn no_document_text_leaks_when_markings_are_embedded_in_prose() {
    // Wrap every invalid fixture with prose from article.txt. If any
    // rule expands its span past the marking boundary (e.g., captures
    // surrounding prose into `original` or extrapolates prose into
    // `replacement`), this test catches it even when the stand-alone
    // fixture does not.
    let engine = test_engine();
    let prose_paths = prose_fixtures();
    let prose_path = prose_paths
        .first()
        .expect("need at least one prose fixture to synthesize composites");
    let prose_bytes = load_fixture(prose_path);

    // Byte-level slicing: the engine's input API is `&[u8]`, but its
    // parser requires UTF-8 for individual marking-candidate spans
    // (non-UTF-8 candidates yield `CoreError::InvalidUtf8` and are
    // skipped by the engine itself). Concatenating at the byte level
    // means the test does not layer an extra UTF-8 skip on top of
    // that contract — every fixture's bytes reach the engine, and
    // the engine applies its own UTF-8 requirement only to the
    // candidate spans it attempts to parse.
    let head_end = prose_bytes.len().min(2048);
    let head: &[u8] = &prose_bytes[..head_end];
    let tail_start = prose_bytes.len().saturating_sub(2048);
    let tail: &[u8] = &prose_bytes[tail_start..];

    let fixtures = invalid_fixtures();
    assert!(
        !fixtures.is_empty(),
        "need invalid fixtures to synthesize composites"
    );

    // Vacuity guard: the test is meaningful only if composite documents
    // actually produce fixes. A future refactor that made all invalid
    // fixtures produce zero fixes (e.g., by changing the default
    // confidence threshold) would otherwise silently turn this into a
    // tautology.
    let mut fixes_examined = 0usize;

    for path in &fixtures {
        let fixture_bytes = load_fixture(path);
        let mut composite =
            Vec::with_capacity(head.len() + 2 + fixture_bytes.len() + 2 + tail.len());
        composite.extend_from_slice(head);
        composite.extend_from_slice(b"\n\n");
        composite.extend_from_slice(&fixture_bytes);
        composite.extend_from_slice(b"\n\n");
        composite.extend_from_slice(tail);

        let result = run_fix(&engine, &composite);
        let label = format!("wrapped:{}", path.display());
        check_fixes_clean(&result.applied, &label);
        fixes_examined += result.applied.len();
    }

    assert!(
        fixes_examined > 0,
        "composite sweep produced zero applied fixes — \
         either the corpus is empty or the engine is not firing \
         (vacuous-pass guard)"
    );
}

// ---------------------------------------------------------------------------
// Companion: same invariant applied to Diagnostic.message.
// ---------------------------------------------------------------------------

#[test]
fn no_document_text_leaks_into_diagnostic_messages() {
    let engine = test_engine();

    let mut sources: Vec<(String, Vec<u8>)> = Vec::new();
    for path in invalid_fixtures() {
        sources.push((path.display().to_string(), load_fixture(&path)));
    }
    for path in valid_fixtures() {
        sources.push((path.display().to_string(), load_fixture(&path)));
    }
    for path in prose_fixtures() {
        sources.push((path.display().to_string(), load_fixture(&path)));
    }

    // Vacuity guard: if the corpus root were mislocated or all three
    // fixture directories were empty, the loop below would trivially
    // succeed. Fail loud instead.
    assert!(
        !sources.is_empty(),
        "no fixtures found across invalid/valid/prose — \
         cannot validate G13 against diagnostic messages \
         (vacuous-pass guard)"
    );

    for (label, source) in &sources {
        let result = engine.lint(source);
        for d in &result.diagnostics {
            for sentinel in PROSE_SENTINELS {
                assert!(
                    !d.message.contains(sentinel),
                    "G13 violation: prose sentinel {sentinel:?} leaked into \
                     Diagnostic.message (rule: {}, fixture: {label})\n\n\
                     message: {:?}",
                    d.rule.as_str(),
                    d.message
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Self-test — the sentinel check actually catches leaks.
// ---------------------------------------------------------------------------

/// Fabricate an `AppliedFix` whose `original` contains a known sentinel.
///
/// This uses `AppliedFix::__engine_promote`, which is the documented
/// test-only exception to the engine-only promotion contract (see
/// `marque/src/render.rs` for the production counterpart and the
/// whitepaper §3.4 / §6.2 for the invariant).
fn fabricate_leaky_fix() -> AppliedFix {
    // A deliberately leaky `original`: a literal prose sentinel. In
    // production this could never happen because every proposal's
    // `original` is a byte-exact slice of the marking span, not of
    // surrounding prose. This is a synthetic leak to prove the check
    // is load-bearing.
    let leaky_original = "enlightened statesmen";
    let proposal = FixProposal::new(
        RuleId::new("E001"),
        FixSource::BuiltinRule,
        Span::new(0, leaky_original.len()),
        leaky_original,
        "SECRET",
        Confidence::strict(1.0),
        None,
    );
    AppliedFix::__engine_promote(
        proposal,
        UNIX_EPOCH + Duration::from_secs(FIXED_TS),
        Some(Arc::<str>::from("test-classifier")),
        /* dry_run */ false,
        Some(Arc::<str>::from("-")),
    )
}

#[test]
#[should_panic(expected = "G13 violation")]
fn sentinel_check_panics_on_synthetic_leak() {
    // Guard against future regressions of the checker itself: a
    // refactor that emptied `PROSE_SENTINELS` or disabled
    // `assert_clean` would cause this `#[should_panic]` test to fail
    // loudly, not silently weaken the real corpus sweep.
    let leaky = fabricate_leaky_fix();
    check_fixes_clean(&[leaky], "synthetic self-test");
}

// ---------------------------------------------------------------------------
// T052 — audit v2 strict-path record invariants.
// ---------------------------------------------------------------------------
//
// The strict path is the engine's default: `Engine::new(...)` without
// `with_deep_scan()` installs `StrictRecognizer` and only ever produces
// fixes from rules / corrections / migrations — never from
// `FixSource::DecoderPosterior`. The v2 audit contract pins four
// per-record shape invariants on every fix that comes out of that
// path:
//
// 1. `confidence.recognition == 1.0_f32` — the strict grammar matched
//    unambiguously by definition.
// 2. `confidence.runner_up_ratio == None` — no candidate set exists,
//    so there is no runner-up.
// 3. `confidence.features.is_empty()` — no decoder feature graph was
//    traversed.
// 4. `proposal.source ∈ { BuiltinRule, CorrectionsMap, MigrationTable }`
//    — the four-way `FixSource` enum minus `DecoderPosterior`.
//
// The invariants are pinned at the data layer by `Confidence::strict`
// (`crates/rules/src/confidence.rs`), so the test below is a
// regression guard: it sweeps the engine's strict-path output over
// the invalid fixture corpus and asserts the four invariants hold for
// every produced `AppliedFix`. A future refactor that, e.g., starts
// emitting `DecoderPosterior` fixes through the strict path, or
// stuffs feature contributions into a strict-path `Confidence`,
// trips this test immediately.
//
// **Companion checks:** the v2 NDJSON envelope is now driven by
// `marque_engine::AUDIT_SCHEMA_VERSION` (PR-4 closed whitepaper §980
// P0-1). T054 (v1-record back-compat parse) lives below; T055
// (stream-level single-schema invariant) lives in
// `marque/tests/cli_fix.rs::audit_stream_uses_only_one_schema_version`
// because the stream emitter is at the CLI layer. T053 (decoder-path
// record shape) waits on PR-4b — `Engine::fix_inner` does not yet
// produce `FixSource::DecoderPosterior` records.

#[test]
fn audit_v2_strict_path_invariants() {
    let engine = test_engine();
    let fixtures = invalid_fixtures();
    assert!(
        !fixtures.is_empty(),
        "no invalid fixtures found — cannot validate T052 strict-path invariants"
    );

    // Vacuity guard: the test is meaningful only if the engine's
    // strict path actually fires fixes. Zero fixes across the entire
    // invalid corpus would silently pass the assertions below.
    let mut total_fixes_examined = 0usize;

    for path in &fixtures {
        let source = load_fixture(path);
        let result = run_fix(&engine, &source);
        for fix in &result.applied {
            let p = &fix.proposal;
            let c = &p.confidence;
            let context = format!(
                "rule {} at {}..{} ({})",
                p.rule.as_str(),
                p.span.start,
                p.span.end,
                path.display()
            );

            assert_eq!(
                c.recognition, 1.0_f32,
                "strict-path Confidence.recognition must be 1.0; got {} for {context}",
                c.recognition,
            );
            assert!(
                c.runner_up_ratio.is_none(),
                "strict-path Confidence.runner_up_ratio must be None; \
                 got {:?} for {context}",
                c.runner_up_ratio,
            );
            assert!(
                c.features.is_empty(),
                "strict-path Confidence.features must be empty; \
                 got {} feature(s) for {context}: {:?}",
                c.features.len(),
                c.features,
            );
            assert!(
                matches!(
                    p.source,
                    FixSource::BuiltinRule | FixSource::CorrectionsMap | FixSource::MigrationTable
                ),
                "strict-path FixSource must be BuiltinRule | CorrectionsMap | \
                 MigrationTable; got {:?} for {context}",
                p.source,
            );
        }
        total_fixes_examined += result.applied.len();
    }

    assert!(
        total_fixes_examined > 0,
        "T052 sweep produced zero applied fixes — \
         either the invalid corpus is empty or the engine's strict path \
         is not firing (vacuous-pass guard)"
    );
}

// ---------------------------------------------------------------------------
// T054 — v1 audit records parse in a v2-aware consumer.
// ---------------------------------------------------------------------------
//
// The Phase-D contract (`contracts/audit-record-v2.md`) defines v2 as a
// strict superset of v1: every v1 field is preserved, then `recognition`
// / `runner_up_ratio` / `features` are added. A v2 consumer MUST be able
// to deserialize a v1 record emitted by a pre-Phase-D engine without
// error — the new fields are simply absent and default to "no decoder
// evidence" (recognition=1.0, runner_up_ratio=None, features=[]).
//
// This test pins the back-compat property at the schema level: a known
// v1-shape JSON fixture (the canonical 12-field record from
// `contracts/audit-record.json`) is deserialized into a struct that
// mirrors the v2 schema. Success means the v2 deserializer is tolerant
// of missing v2 fields; failure means a v2 consumer would reject v1
// records, breaking the back-compat contract (FR-014, SC-006).

/// v2 deserializer for back-compat testing.
///
/// Mirrors the v2 audit-record JSON shape (`contracts/audit-record-v2.md`)
/// with `#[serde(default)]` on every v2-only field so a v1 record (which
/// lacks them) deserializes cleanly. The struct is local to this test —
/// production code uses serializer types in `marque/src/render.rs` and
/// `crates/wasm/src/lib.rs`; this is the test-side mirror of the
/// downstream-consumer contract.
#[derive(Debug, serde::Deserialize, PartialEq)]
#[allow(dead_code)] // fields exercised through serde, not direct access
struct AuditRecordV2Deserializer {
    schema: String,
    rule: String,
    source: String,
    span: SpanDeserializer,
    original: String,
    replacement: String,
    confidence: f32,
    migration_ref: Option<String>,
    timestamp: String,
    classifier_id: Option<String>,
    dry_run: bool,
    input: Option<String>,
    // v2-only fields — defaulted so v1 records parse.
    #[serde(default = "default_recognition")]
    recognition: f32,
    #[serde(default)]
    runner_up_ratio: Option<f32>,
    #[serde(default)]
    features: Vec<FeatureDeserializer>,
}

fn default_recognition() -> f32 {
    // A v1 record has no `recognition` field; the v2-tolerant
    // interpretation is that a pre-Phase-D fix was strict-path by
    // construction, so its recognition is implicitly 1.0.
    1.0
}

#[derive(Debug, serde::Deserialize, PartialEq)]
#[allow(dead_code)]
struct SpanDeserializer {
    start: usize,
    end: usize,
}

#[derive(Debug, serde::Deserialize, PartialEq)]
#[allow(dead_code)]
struct FeatureDeserializer {
    id: String,
    delta: f32,
}

#[test]
fn v1_records_parse_in_v2_consumer() {
    // Canonical v1 fixture — the 12-field shape `marque-mvp-1` consumers
    // emit. Pulled from the snapshot of a pre-Phase-D engine build to
    // ensure parity with what's actually on the wire.
    const V1_RECORD: &str = r#"{
        "schema": "marque-mvp-1",
        "rule": "E001",
        "source": "BuiltinRule",
        "span": {"start": 8, "end": 10},
        "original": "NF",
        "replacement": "NOFORN",
        "confidence": 1.0,
        "migration_ref": null,
        "timestamp": "2023-11-14T22:13:20Z",
        "classifier_id": null,
        "dry_run": false,
        "input": null
    }"#;

    let parsed: AuditRecordV2Deserializer = serde_json::from_str(V1_RECORD)
        .expect("v1 record must deserialize cleanly in a v2-aware consumer");

    assert_eq!(parsed.schema, "marque-mvp-1");
    assert_eq!(parsed.rule, "E001");
    assert_eq!(parsed.source, "BuiltinRule");
    assert_eq!(parsed.span.start, 8);
    assert_eq!(parsed.span.end, 10);
    assert_eq!(parsed.original, "NF");
    assert_eq!(parsed.replacement, "NOFORN");
    assert_eq!(parsed.confidence, 1.0);

    // v2 fields default to "no decoder evidence" — the back-compat
    // contract's interpretation of a v1 record.
    assert_eq!(parsed.recognition, 1.0);
    assert!(
        parsed.runner_up_ratio.is_none(),
        "v1 record must default runner_up_ratio to None"
    );
    assert!(
        parsed.features.is_empty(),
        "v1 record must default features to empty"
    );
}

// ---------------------------------------------------------------------------
// T053 — decoder-path audit-record shape.
// ---------------------------------------------------------------------------
//
// Strict-path invariants are pinned by `audit_v2_strict_path_invariants`
// (T052): every applied fix must have `recognition == 1.0`,
// `runner_up_ratio == None`, `features == []`, and a
// non-`DecoderPosterior` source. The decoder-path counterpart asserts
// the dual: when the engine is in deep-scan mode AND the recognizer
// goes through the decoder fallback for a mangled candidate, the
// resulting `AppliedFix` carries:
//
// - `confidence.recognition < 1.0_f32` — strictly below the strict-path
//   sentinel so audit consumers can distinguish strict from decoder
//   provenance via a single field comparison.
// - `confidence.features` non-empty with every entry's `id` typed as
//   `FeatureId` (free-form strings rejected by the type system per
//   FR-012).
// - `source == FixSource::DecoderPosterior`.
// - `confidence.runner_up_ratio` is either `None` (decoder's K-truncated
//   set collapsed to a single candidate, per `decoder.rs`'s K=1 branch)
//   or `Some(r)` with finite `r`. Both shapes are legal; the audit-shape
//   invariant T053 pins is "if `Some`, the value is finite" — never
//   `NaN` / `±∞`.
//
// Vacuity guard: ≥ 1 decoder fix examined. A pass with zero fixes
// would indicate the deep-scan dispatcher never invoked the decoder
// at all — silently weakening the assertion.

fn deep_scan_engine() -> Engine {
    let engine = Engine::with_clock(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles");
    engine.with_deep_scan()
}

#[test]
fn decoder_path_record_shape() {
    use marque_rules::FeatureId;

    let engine = deep_scan_engine();

    // Mangled portion candidate: leading `(` makes the scanner emit
    // a portion candidate; SERCET inside is edit-distance-1 from
    // SECRET; NF is the canonical portion-form NOFORN abbreviation
    // and survives fuzzy correction unchanged so the decoder
    // produces a clean canonical rewrite `(SECRET//NF)`. The strict
    // parser leaves `classification = None` for the original
    // SERCET-bearing input (lenient parse), so the dispatcher falls
    // through to the decoder per `strict_parse_is_complete`.
    let source: &[u8] = b"(SERCET//NF)";

    let result = run_fix(&engine, source);

    let mut decoder_fixes_examined = 0usize;
    for fix in &result.applied {
        // Identify the decoder-path fix and assert its shape. Other
        // fixes (e.g., a strict-path E001 against the canonical attrs)
        // may also appear in the same audit set; they remain
        // strict-shape and are skipped here.
        if fix.source != FixSource::DecoderPosterior {
            continue;
        }
        decoder_fixes_examined += 1;

        let c = &fix.confidence;
        assert!(
            c.recognition < 1.0_f32,
            "decoder-path Confidence.recognition must be strictly < 1.0; \
             got {} (rule {}, span {}..{})",
            c.recognition,
            fix.proposal.rule.as_str(),
            fix.proposal.span.start,
            fix.proposal.span.end,
        );
        assert!(
            !c.features.is_empty(),
            "decoder-path Confidence.features must be non-empty (FR-009); \
             got 0 features for rule {} at {}..{}",
            fix.proposal.rule.as_str(),
            fix.proposal.span.start,
            fix.proposal.span.end,
        );
        // Every feature carries a `FeatureId` enum — by type, not by
        // string. Iterating exercises the field; pattern-matching is
        // exhaustiveness-checked at compile time, so a future variant
        // addition without a coordinated audit-schema bump fails CI
        // here at the same gate as the `FeatureId::as_str` table.
        for feature in &c.features {
            match feature.id {
                FeatureId::EditDistance1
                | FeatureId::EditDistance2
                | FeatureId::TokenReorder
                | FeatureId::SupersededToken
                | FeatureId::BaseRateCommonMarking
                | FeatureId::StrictContextClassification
                | FeatureId::CorpusOverrideInEffect => {}
            }
            assert!(
                feature.delta.is_finite(),
                "feature delta must be finite, got {}",
                feature.delta
            );
        }

        // `runner_up_ratio` is `Some(r)` when the decoder's K-truncated
        // candidate set had ≥ 2 survivors so a runner-up exists, and
        // `None` when only one candidate cleared strict-parse + the
        // FR-011 floor + the non-trivial filter (decoder.rs's K=1 branch
        // where `runner_up_score.is_finite()` is `false`). Both shapes
        // are legal for a decoder-path fix per the `Confidence` contract
        // in `crates/rules/src/confidence.rs`. The audit-shape invariant
        // T053 pins is "if Some, the value is finite" — `runner_up_ratio`
        // never carries `NaN` / `±∞` at the audit boundary, since
        // `Confidence::validate` rejects non-finite ratios. Whether a
        // particular input produces K=1 or K≥2 is decoder-implementation
        // territory (PR-3) and not what this audit-shape test gates.
        if let Some(r) = c.runner_up_ratio {
            assert!(
                r.is_finite(),
                "decoder-path runner_up_ratio must be finite when Some, got {r}"
            );
        }
    }

    assert!(
        decoder_fixes_examined > 0,
        "T053 vacuity guard: zero decoder-path fixes were produced for \
         the mangled fixture {:?}. Either the deep-scan dispatcher never \
         reached the decoder, or the decoder declined to canonicalize. \
         Without ≥1 fix examined the per-fix shape assertions above pass \
         vacuously.",
        std::str::from_utf8(source).unwrap_or("<non-utf8>"),
    );
}
