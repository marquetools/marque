// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Audit-stream content-ignorance tests.
//!
//! Enforces Constitution V and the audit content-ignorance boundary
//! (`docs/security/WHITEPAPER.md` §3.1): no document content, metadata
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
//! labels (`FeatureId`). None carry document-
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
//! 3. **Companion: diagnostic messages.** Strictly the audit-stream
//!    check is scoped to the audit stream, but the same invariant
//!    applies to
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

use marque_capco::{CapcoScheme, capco_rules};
use marque_config::Config;
use marque_engine::{Engine, FixMode, FixResult, FixedClock};
use marque_rules::audit::{AppliedTextCorrection, AuditLine};
use marque_rules::{
    EnginePromotionToken, FixSource, Message, MessageArgs, MessageTemplate, Recognition, RuleId,
};
use marque_scheme::{Severity, Span};
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
    // `audit_v3_strict_path_invariants` asserts strict-shape
    // invariants on every produced `AppliedFix`. Pin the recognizer to
    // `StrictRecognizer` explicitly — the engine default
    // (`StrictOrDecoderRecognizer`) would still hold the invariant on
    // today's fixture set because no fixture trips the decoder, but a
    // future fixture that does would silently weaken the assertion if
    // we relied on the default.
    Engine::with_clock(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
    // INTENTIONAL-STRICT: audit-trail tests pin the strict recognizer because the audit invariants tested here apply to the strict-path AppliedFix shape; decoder-path differences are exercised in decoder_diagnostic.rs
    .with_recognizer(std::sync::Arc::new(marque_engine::StrictRecognizer::new()))
}

fn run_fix(engine: &Engine, source: &[u8]) -> FixResult {
    engine.fix(source, FixMode::Apply)
}

/// Build an engine with `confidence_threshold = 0.80` so decoder-
/// path fixes land in `result.applied` even when the prose null-
/// hypothesis runner-up has shrunk `recognition_score` below the
/// default 0.95.
///
/// Issue #258 added a per-token prose null hypothesis to the decoder's
/// dispatch — the marking-side score now competes against
/// `log P(tokens|prose)` for the same canonical-token set. For
/// inputs like `(SERCET//NF)` the marking interpretation still wins
/// the dispatch (positive log-margin against null), but the runner-up
/// is no longer `f32::NEG_INFINITY` (no other marking candidate) —
/// it's the prose null-hypothesis score, so the resulting
/// `runner_up_ratio` is finite and `recognition` lands around 0.83
/// for short portion-form fuzzy fixes instead of `SOLO_RECOGNITION =
/// 0.999999`. That's correct decoder behavior — a fuzzy
/// edit-distance-1 fix on a two-portion-token input (`S` + `NF`)
/// carries appropriate uncertainty when `S` has high prose-side
/// frequency — but it pushes the fix below the default
/// `confidence_threshold = 0.95`, and the lint phase's eager
/// `Severity::Fix → Severity::Suggest` downgrade
/// (`crates/engine/src/engine.rs:748`) consults the engine's
/// `Config::confidence_threshold`, NOT the per-call
/// `threshold_override` from `fix_with_threshold`. So the override
/// does not undo the lint downgrade — we have to lower the config
/// threshold itself.
fn deep_scan_engine_relaxed() -> Engine {
    let mut config = Config::default();
    config
        .set_confidence_threshold(0.80)
        .expect("0.80 is a valid confidence threshold");
    Engine::with_clock(
        config,
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

/// Check every AppliedFix in `applied` for sentinel leaks.
///
/// The only audit field that can carry corpus-derived bytes is the
/// `TextCorrection.replacement` payload — and that's a canonical token
/// from the `[corrections]` map, on Constitution V's
/// permitted-identifier list. No raw original-byte field is
/// representable in the audit record.
fn check_fixes_clean(audit_lines: &[AuditLine<CapcoScheme>], context: &str) {
    for line in audit_lines {
        // Marking-side fixes carry a sealed `Canonical<S>` payload —
        // no free-form string surface to scan. Text-correction lines
        // carry a corpus-derived `replacement: SmolStr`; that's where
        // a regressed channel would leak.
        if let AuditLine::TextCorrection(tc) = line {
            assert_text_correction_clean(tc, "replacement", tc.replacement.as_ref(), context);
        }
    }
}

/// Panic if any prose sentinel appears in the given string (v2
/// [`AppliedTextCorrection`] variant).
///
/// Mirrors [`assert_clean`] but reads the v2 audit-record type's
/// fields ([`marque_rules::audit::AppliedTextCorrection::rule`] +
/// `.span` + `.replacement`). Used by the v2 self-test
/// [`sentinel_check_panics_on_synthetic_leak`].
fn assert_text_correction_clean(
    tc: &AppliedTextCorrection,
    field_name: &str,
    value: &str,
    context: &str,
) {
    for sentinel in PROSE_SENTINELS {
        if value.contains(sentinel) {
            panic!(
                "content-ignorance violation: prose sentinel {sentinel:?} leaked into \
                 AppliedTextCorrection.{field_name} \
                 (rule: {rule}, span: {start}..{end}, context: {context})\n\n\
                 field value: {value:?}",
                rule = tc.rule,
                start = tc.span.start,
                end = tc.span.end,
            );
        }
    }
}

/// Check every v2 [`AppliedTextCorrection`] in `corrections` for
/// sentinel leaks.
///
/// The v2 type's only corpus-derived field is `replacement: SmolStr`
/// (the canonical token from the `[corrections]` map). Constitution V
/// Principle V permits the corpus-derived value here because it must
/// be on the permitted-identifier list; this checker exists to catch
/// regressions where prose would leak into that field.
fn check_text_corrections_clean(corrections: &[AppliedTextCorrection], context: &str) {
    for tc in corrections {
        assert_text_correction_clean(tc, "replacement", tc.replacement.as_ref(), context);
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
        "no invalid fixtures found — cannot validate audit content-ignorance"
    );
    for path in &fixtures {
        let source = load_fixture(path);
        let result = run_fix(&engine, &source);
        check_fixes_clean(&result.audit_lines, &path.display().to_string());
    }
}

#[test]
fn no_document_text_leaks_from_valid_corpus() {
    let engine = test_engine();
    let fixtures = valid_fixtures();
    assert!(
        !fixtures.is_empty(),
        "no valid fixtures found — cannot validate audit content-ignorance"
    );
    for path in &fixtures {
        let source = load_fixture(path);
        let result = run_fix(&engine, &source);
        check_fixes_clean(&result.audit_lines, &path.display().to_string());
    }
}

#[test]
fn no_document_text_leaks_from_prose_corpus() {
    let engine = test_engine();
    let fixtures = prose_fixtures();
    assert!(
        !fixtures.is_empty(),
        "no prose fixtures found — cannot validate audit content-ignorance"
    );
    for path in &fixtures {
        let source = load_fixture(path);
        let result = run_fix(&engine, &source);
        check_fixes_clean(&result.audit_lines, &path.display().to_string());
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
        check_fixes_clean(&result.audit_lines, &label);
        fixes_examined += result.applied_fixes().count();
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
         cannot validate audit content-ignorance against diagnostic messages \
         (vacuous-pass guard)"
    );

    for (label, source) in &sources {
        let result = engine.lint(source);
        for d in &result.diagnostics {
            // `Diagnostic.message` is a typed `Message`, not a
            // `Box<str>`. Document text is no longer
            // constructible inside `Diagnostic.message` by type — every
            // field on `MessageArgs` is a closed-set identifier
            // (`TokenId`/`CategoryId`/`Span`/`Blake3Hash`/`Recognition`/
            // `FeatureId`/`RuleId`), and the only string content is the
            // `MessageTemplate` label (a `&'static str` from the closed
            // enum). Scan that label as the load-bearing structural
            // pin: if a future refactor reintroduced a free-form string
            // channel on `Message`, this scan would still catch it.
            let template_label = d.message.template().as_str();
            for sentinel in PROSE_SENTINELS {
                assert!(
                    !template_label.contains(sentinel),
                    "content-ignorance violation: prose sentinel {sentinel:?} leaked into \
                     Diagnostic.message template (rule: {}, fixture: {label})\n\n\
                     template: {template_label}",
                    d.rule,
                );
            }
        }
    }
}

#[test]
fn no_document_text_leaks_into_fix_remaining_diagnostics() {
    // Companion to `no_document_text_leaks_into_diagnostic_messages`.
    //
    // The lint-side sweep covers `LintResult.diagnostics`. A second
    // stream of `Diagnostic` values flows out
    // of the engine: `FixResult.remaining_diagnostics`. R002 — the
    // synthetic post-pass-1 re-parse-failure diagnostic — lands
    // exclusively in this stream, not in `LintResult.diagnostics`.
    // Other diagnostics that the fix path filters in (suggest-only
    // entries below the confidence threshold, diagnostics whose
    // fix did not apply due to C-1 overlap, pass-0 dropped
    // text-correction diagnostics) also reach the caller through
    // this field. The content-ignorance invariant — no document content in any
    // `Diagnostic.message` reaching the audit / consumer boundary
    // — applies identically to both streams; the corpus-level test
    // requirement from Constitution V Principle V is "engine output
    // streams," plural.
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

    assert!(
        !sources.is_empty(),
        "no fixtures found across invalid/valid/prose — \
         cannot validate audit content-ignorance against fix-remaining-diagnostic messages \
         (vacuous-pass guard)"
    );

    // Vacuity guard: the test is meaningful only if at least one
    // fixture produces a remaining diagnostic. The invalid fixture
    // set is the natural producer (sub-threshold suggestions surface
    // in `remaining_diagnostics`), but if a future refactor purged
    // every below-threshold path the assertion below would silently
    // pass. Fail loudly instead.
    let mut diagnostics_examined = 0usize;

    for (label, source) in &sources {
        let result = engine.fix(source, FixMode::Apply);
        diagnostics_examined += result.remaining_diagnostics.len();
        for d in &result.remaining_diagnostics {
            // `Diagnostic.message` is closed-template `Message`, no
            // document text constructible by type. Scan
            // the template label as the structural-pin equivalent of
            // the prior `contains()` byte-substring check (see the
            // companion `no_document_text_leaks_into_diagnostic_messages`
            // explanation above).
            let template_label = d.message.template().as_str();
            for sentinel in PROSE_SENTINELS {
                assert!(
                    !template_label.contains(sentinel),
                    "content-ignorance violation: prose sentinel {sentinel:?} leaked into \
                     FixResult.remaining_diagnostics[].message template \
                     (rule: {}, fixture: {label})\n\n\
                     template: {template_label}",
                    d.rule,
                );
            }
        }
    }

    assert!(
        diagnostics_examined > 0,
        "fix-remaining-diagnostics sweep produced zero diagnostics across the \
         full corpus — either the corpus is empty or the engine's fix path \
         no longer surfaces remaining diagnostics (vacuous-pass guard)"
    );
}

// ---------------------------------------------------------------------------
// Self-test — the sentinel check actually catches leaks.
// ---------------------------------------------------------------------------

/// Fabricate an `AppliedTextCorrection` whose `replacement` contains
/// a known sentinel.
///
/// Test-fixture carve-out per Constitution V Principle V: this
/// fabricated record is the input to
/// [`check_text_corrections_clean`]'s sentinel sweep, exists only
/// inside the `tests/` tree, and is never spliced into a real audit
/// stream. Engine production paths remain the only route to a real
/// [`AppliedTextCorrection`] in `cfg(not(test))` code; see the doc
/// comment on
/// [`marque_rules::audit::AppliedTextCorrection::__engine_promote_text_correction`]
/// for the three-constraint definition of the carve-out.
///
/// The leak channel under test is `replacement: SmolStr` on the
/// [`AppliedTextCorrection`] type.
fn fabricate_leaky_text_correction() -> AppliedTextCorrection {
    // A deliberately leaky text-correction replacement carrying a
    // literal prose sentinel. The only audit field that can carry
    // corpus-derived bytes in the v2 envelope is the
    // `AppliedTextCorrection.replacement` payload; a real
    // corrections-map entry would never contain prose, but this
    // synthetic fixture proves the checker can catch a leak if one
    // were to land.
    let leaky_replacement = "enlightened statesmen";
    let original_digest = blake3::hash(leaky_replacement.as_bytes());
    // Test-fixture carve-out per Constitution V Principle V.
    let token = EnginePromotionToken::__engine_construct();
    AppliedTextCorrection::__engine_promote_text_correction(
        RuleId::new("capco", "marking.correction.token-typo"),
        Severity::Fix,
        Span::new(0, leaky_replacement.len()),
        original_digest,
        leaky_replacement.into(),
        FixSource::CorrectionsMap,
        Recognition::strict(),
        /* migration_ref */ None,
        Message::new(MessageTemplate::CorrectionsApplied, MessageArgs::default()),
        UNIX_EPOCH + Duration::from_secs(FIXED_TS),
        Some(Arc::<str>::from("test-classifier")),
        /* dry_run */ false,
        Some(Arc::<str>::from("-")),
        token,
    )
}

#[test]
#[should_panic(expected = "content-ignorance violation")]
fn sentinel_check_panics_on_synthetic_leak() {
    // Guard against future regressions of the checker itself: a
    // refactor that emptied `PROSE_SENTINELS` or disabled
    // `assert_text_correction_clean` would cause this `#[should_panic]`
    // test to fail loudly, not silently weaken the real corpus sweep.
    let leaky = fabricate_leaky_text_correction();
    check_text_corrections_clean(&[leaky], "synthetic self-test");
}

// ---------------------------------------------------------------------------
// Audit v2 strict-path record invariants.
// ---------------------------------------------------------------------------
//
// The strict path is the explicit-opt-out mode: `Engine::new(...)`
// installs the `StrictOrDecoderRecognizer` dispatcher by default;
// callers that need strict-only behavior install `StrictRecognizer`
// via `Engine::with_recognizer`. The strict path produces fixes only
// from rules / corrections / migrations — never from
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
// The invariants are pinned at the data layer by `Recognition::strict`
// (`crates/rules/src/recognition.rs`), so the test below is a
// regression guard: it sweeps the engine's strict-path output over
// the invalid fixture corpus and asserts the four invariants hold for
// every produced `AppliedFix`. A future refactor that, e.g., starts
// emitting `DecoderPosterior` fixes through the strict path, or
// stuffs feature contributions into a strict-path `Recognition`,
// trips this test immediately.
//
// **Companion checks:** the v2 NDJSON envelope is driven by
// `marque_engine::AUDIT_SCHEMA_VERSION`. The v1-record back-compat
// parse test lives below; the stream-level single-schema invariant
// lives in
// `marque/tests/cli_fix.rs::audit_stream_uses_only_one_schema_version`
// because the stream emitter is at the CLI layer. The decoder-path
// record-shape test lives further below.

#[test]
fn audit_v3_strict_path_invariants() {
    let engine = test_engine();
    let fixtures = invalid_fixtures();
    assert!(
        !fixtures.is_empty(),
        "no invalid fixtures found — cannot validate strict-path invariants"
    );

    // Vacuity guard: the test is meaningful only if the engine's
    // strict path actually fires fixes. Zero fixes across the entire
    // invalid corpus would silently pass the assertions below.
    let mut total_fixes_examined = 0usize;

    for path in &fixtures {
        let source = load_fixture(path);
        let result = run_fix(&engine, &source);
        for line in &result.audit_lines {
            // Marking-side audit-record contract per
            // `contracts/audit-record.md` §107-178: `fix.replacement
            // .confidence` carries the strict invariants below.
            // Text-correction lines have their own `confidence` field
            // at the top level.
            let (rule, span, source_arm, confidence) = match line {
                AuditLine::AppliedFix(f) => {
                    (&f.rule, f.span, f.source, &f.fix.replacement.confidence)
                }
                AuditLine::TextCorrection(tc) => (&tc.rule, tc.span, tc.source, &tc.confidence),
                _ => continue,
            };
            let context = format!(
                "rule {} at {}..{} ({})",
                rule,
                span.start,
                span.end,
                path.display()
            );

            assert_eq!(
                confidence.recognition, 1.0_f32,
                "strict-path Recognition.recognition must be 1.0; got {} for {context}",
                confidence.recognition,
            );
            assert!(
                confidence.runner_up_ratio.is_none(),
                "strict-path Recognition.runner_up_ratio must be None; \
                 got {:?} for {context}",
                confidence.runner_up_ratio,
            );
            assert!(
                confidence.features.is_empty(),
                "strict-path Recognition.features must be empty; \
                 got {} feature(s) for {context}: {:?}",
                confidence.features.len(),
                confidence.features,
            );
            assert!(
                matches!(
                    source_arm,
                    FixSource::BuiltinRule | FixSource::CorrectionsMap | FixSource::MigrationTable
                ),
                "strict-path FixSource must be BuiltinRule | CorrectionsMap | \
                 MigrationTable; got {source_arm:?} for {context}",
            );
        }
        total_fixes_examined += result.audit_lines.len();
    }

    assert!(
        total_fixes_examined > 0,
        "strict-path sweep produced zero applied fixes — \
         either the invalid corpus is empty or the engine's strict path \
         is not firing (vacuous-pass guard)"
    );
}

// ---------------------------------------------------------------------------
// v1 audit records parse in a v2-aware consumer.
// ---------------------------------------------------------------------------
//
// v2 is a strict superset of v1: every v1 field is preserved, then
// `recognition` / `runner_up_ratio` / `features` are added. A v2
// consumer MUST be able to deserialize a v1 record without error — the
// new fields are simply absent and default to "no decoder evidence"
// (recognition=1.0, runner_up_ratio=None, features=[]).
//
// This test pins the back-compat property at the schema level: a known
// v1-shape JSON fixture (the canonical 12-field record) is
// deserialized into a struct that mirrors the v2 schema. Success means
// the v2 deserializer is tolerant of missing v2 fields; failure means a
// v2 consumer would reject v1 records, breaking the back-compat
// contract.

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
    // emit, matching what an older engine put on the wire.
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
// Decoder-path audit-record shape.
// ---------------------------------------------------------------------------
//
// Strict-path invariants are pinned by `audit_v3_strict_path_invariants`:
// every applied fix must have `recognition == 1.0`,
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
//   `FeatureId` (free-form strings rejected by the type system).
// - `source == FixSource::DecoderPosterior`.
// - `confidence.runner_up_ratio` is either `None` (decoder's K-truncated
//   set collapsed to a single candidate, per `decoder.rs`'s K=1 branch)
//   or `Some(r)` with finite `r`. Both shapes are legal; the audit-shape
//   invariant is "if `Some`, the value is finite" — never `NaN` / `±∞`.
//
// Vacuity guard: ≥ 1 decoder fix examined. A pass with zero fixes
// would indicate the deep-scan dispatcher never invoked the decoder
// at all — silently weakening the assertion.

// Default-threshold deep-scan engine helper. The prose null-hypothesis
// runner-up shrinks recognition for short fuzzy fixes below the
// default 0.95 gate, so `decoder_path_record_shape` uses
// `deep_scan_engine_relaxed` (config threshold 0.80) instead. Kept for
// documentation and as a scaffolding handle if a future test needs the
// default threshold; `#[allow(dead_code)]` suppresses the unused-function
// warning rather than deleting the helper outright.
#[allow(dead_code)]
fn deep_scan_engine() -> Engine {
    // The decoder fallback is the engine default (`Engine::new` /
    // `Engine::with_clock` install `StrictOrDecoderRecognizer`); no
    // explicit opt-in is required.
    Engine::with_clock(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

#[test]
fn decoder_path_record_shape() {
    use marque_rules::FeatureId;

    // Use a relaxed-threshold engine — the prose null-hypothesis
    // runner-up (issue #258) shrinks recognition for short fuzzy
    // fixes below the default 0.95 gate. See
    // `deep_scan_engine_relaxed` for the rationale.
    let engine = deep_scan_engine_relaxed();

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
    for fix in result.applied_fixes() {
        // Identify the decoder-path fix and assert its shape. Other
        // fixes (e.g., a strict-path rule firing against the canonical
        // attrs) may also appear in the same audit set; they remain
        // strict-shape and are skipped here.
        if fix.source != FixSource::DecoderPosterior {
            continue;
        }
        decoder_fixes_examined += 1;

        // `fix.replacement.confidence` per the audit-record shape.
        let c = &fix.fix.replacement.confidence;
        assert!(
            c.recognition < 1.0_f32,
            "decoder-path Recognition.recognition must be strictly < 1.0; \
             got {} (rule {}, span {}..{})",
            c.recognition,
            fix.rule,
            fix.span.start,
            fix.span.end,
        );
        assert!(
            !c.features.is_empty(),
            "decoder-path Recognition.features must be non-empty; \
             got 0 features for rule {} at {}..{}",
            fix.rule,
            fix.span.start,
            fix.span.end,
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
                | FeatureId::CorpusOverrideInEffect
                | FeatureId::LinePositionPenalty
                | FeatureId::BulletAnchorBonus
                | FeatureId::LowercaseSurroundingContext => {}
                // `FeatureId` is `#[non_exhaustive]` (T025): the
                // multi-scheme `Grammar { .. }` escape and any future
                // additive variant land here. The test exercises the
                // field type, not the closed CAPCO set — the closed-set
                // pin lives in `recognition.rs::feature_id_as_str_matches_audit_contract`.
                FeatureId::Grammar { .. } => {}
                _ => {}
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
        // classification floor + the non-trivial filter (decoder.rs's
        // K=1 branch where `runner_up_score.is_finite()` is `false`).
        // Both shapes are legal for a decoder-path fix per the
        // `Recognition` contract in `crates/rules/src/recognition.rs`. The
        // audit-shape invariant is "if Some, the value is finite" —
        // `runner_up_ratio` never carries `NaN` / `±∞` at the audit
        // boundary, since `Recognition::validate` rejects non-finite
        // ratios. Whether a particular input produces K=1 or K≥2 is
        // decoder-implementation territory and not what this audit-shape
        // test gates.
        if let Some(r) = c.runner_up_ratio {
            assert!(
                r.is_finite(),
                "decoder-path runner_up_ratio must be finite when Some, got {r}"
            );
        }
    }

    assert!(
        decoder_fixes_examined > 0,
        "vacuity guard: zero decoder-path fixes were produced for \
         the mangled fixture {:?}. Either the deep-scan dispatcher never \
         reached the decoder, or the decoder declined to canonicalize. \
         Without ≥1 fix examined the per-fix shape assertions above pass \
         vacuously.",
        std::str::from_utf8(source).unwrap_or("<non-utf8>"),
    );
}

// ---------------------------------------------------------------------------
// Migration-audit URN provenance.
// ---------------------------------------------------------------------------
//
// The audit-record contract (Constitution V) requires every applied
// fix to be traceable to its publishing authority. For ODNI-published
// markings that means a URN that resolves to the source CVE file in
// the schema package.
//
// `AppliedFix` carries the `original` and `replacement` strings and
// the rule id. The URN provenance does NOT live as a separate field
// on the audit record — that would either bump the audit schema to v3
// or require adding non-back-compat optional fields that bypass the
// `MARQUE_AUDIT_SCHEMA` accept-list. Instead, the URNs are
// *recoverable* from the audit record's strings + `marque-ism`'s
// public lookup tables.
//
// ## Recovery path (string-keyed, what audit consumers use)
//
// Audit consumers receive serialized records — strings, not typed
// `TokenId`s. Their recovery path runs through `marque-ism`'s public
// surface:
//
// - `marque_ism::generated::vocabulary::lookup_token_metadata(value)`:
//   string-keyed lookup over `TOKEN_METADATA`. Returns the entry
//   whose `cve_file.urn` is the source URN.
// - `marque_ism::marking_forms::banner_to_portion(banner)`: maps a
//   banner-form string back to its canonical CVE value, since
//   banner forms are not themselves CVE values (they're publishing
//   conventions per CAPCO-2016 §G.1 Table 4).
//
// Together these compose the recovery: given the audit's `original`
// (canonical) and `replacement` (banner) strings, the consumer
// recovers both URNs without engine internals or `TokenId`-keyed
// access.
//
// ## Cross-check: `Vocabulary<S>` agrees
//
// The `Vocabulary<CapcoScheme>` trait surface
// is the TYPED accessor: `TokenId`-keyed, used by rule code that
// already has a typed token. The cross-check below verifies the typed
// and untyped paths agree — a divergence would indicate either the
// `SENTINEL_TO_CANONICAL` mapping or `marque-ism`'s string-keyed
// table got out of sync.
//
// ## Test shape
//
// 1. Takes the canonical `NF` / `NOFORN` form pair directly.
// 2. Recovers `source_urn` from the canonical value ("NF") via the
//    string-keyed path.
// 3. Recovers `replacement_urn` from the banner form ("NOFORN") via
//    the banner-form round-trip path.
// 4. Asserts both URNs trace to ODNI and are equal (same CVE entry).
// 5. Cross-checks the typed `Vocabulary` accessor agrees with the
//    string-keyed path.

#[test]
fn nf_noforn_form_pair_resolves_to_same_odni_urn() {
    // The contract being tested is the URN provenance property itself
    // — that a canonical CVE value and its banner form resolve to the
    // same ODNI URN via the public lookup tables — independent of any
    // emitting rule. The portion-mark-in-banner remediation now lives
    // in `MarkingScheme::render_canonical` rather than a dedicated
    // rule, so the contract is exercised directly here against the
    // recovery helpers, not via a rule's emitted audit record.
    use marque_capco::scheme::{CapcoScheme, TOK_NOFORN};
    use marque_scheme::Vocabulary;

    let original = "NF";
    let replacement = "NOFORN";

    // Path 1: `original` is "NF", a canonical CVE value. Look it up
    // directly in the per-token metadata table.
    let source_metadata = marque_ism::generated::vocabulary::lookup_token_metadata(original)
        .unwrap_or_else(|| panic!("canonical {original:?} must resolve to TOKEN_METADATA",));
    let source_urn = source_metadata.cve_file.urn;

    // Path 2: `replacement` is "NOFORN", a banner form (not a CVE
    // value). The audit consumer's recovery path is to map back to
    // canonical via `marking_forms::banner_to_portion`, then look up
    // the canonical in TOKEN_METADATA.
    let canonical_for_replacement = marque_ism::marking_forms::banner_to_portion(replacement)
        .unwrap_or_else(|| {
            panic!("banner form {replacement:?} must map back via banner_to_portion",)
        });
    let replacement_metadata =
        marque_ism::generated::vocabulary::lookup_token_metadata(canonical_for_replacement)
            .unwrap_or_else(|| {
                panic!(
                    "canonical {canonical_for_replacement:?} (from banner form \
                     {replacement:?}) must resolve to a TOKEN_METADATA entry",
                )
            });
    let replacement_urn = replacement_metadata.cve_file.urn;

    // Both URNs must trace to ODNI.
    assert!(
        source_urn.starts_with("urn:us:gov:ic:cvenum:"),
        "source URN must trace to ODNI: {source_urn:?}",
    );
    assert!(
        replacement_urn.starts_with("urn:us:gov:ic:cvenum:"),
        "replacement URN must trace to ODNI: {replacement_urn:?}",
    );

    // The canonical/banner-form pair NF/NOFORN are forms of the same
    // CVE entry, so the URNs must match exactly.
    assert_eq!(
        source_urn, replacement_urn,
        "NF (source) and NOFORN (replacement) are forms of the same CVE entry — \
         their URNs must match.",
    );

    // Cross-check against the Vocabulary trait surface — both lookup
    // paths must agree with `Vocabulary<CapcoScheme>::metadata`.
    let scheme = CapcoScheme::new();
    let metadata = scheme.metadata(&TOK_NOFORN);
    assert_eq!(metadata.urn, source_urn);
    assert_eq!(metadata.canonical, "NF");
    assert_eq!(metadata.banner_form, "NOFORN");
    assert_eq!(metadata.authority.urn, source_urn);
    assert_eq!(
        metadata.authority.schema_version,
        marque_ism::SCHEMA_VERSION
    );

    // NOTE: This test exercises the vocabulary lookup tables
    // (`lookup_token_metadata`, `banner_to_portion`, `Vocabulary::for_token`)
    // directly with hardcoded `NF` / `NOFORN`, NOT through a rule-emitted
    // `AppliedFix` — the portion-mark-in-banner remediation lives in the
    // renderer, not a rule. The end-to-end audit round-trip (engine
    // produces fix → audit fields → URN recovery) is no longer separately
    // pinned by this file. A regression in `proposal.original` /
    // `proposal.replacement` field emission would surface in the audit
    // snapshot tests (`fix_pipeline.rs::audit_record_snapshot_*`), not here.
}
