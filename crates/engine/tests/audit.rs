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
//!    stand-alone fixtures miss it. Concatenation is byte-level so
//!    non-UTF-8 fixtures are never silently skipped.
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
fn assert_clean(field_name: &str, value: &str, context: &str) {
    for sentinel in PROSE_SENTINELS {
        if value.contains(sentinel) {
            panic!(
                "G13 violation: prose sentinel {sentinel:?} leaked into \
                 AppliedFix.proposal.{field_name} (context: {context})\n\n\
                 field value: {value:?}"
            );
        }
    }
}

/// Check every AppliedFix in `applied` for sentinel leaks.
fn check_fixes_clean(applied: &[AppliedFix], context: &str) {
    for fix in applied {
        assert_clean("original", fix.proposal.original.as_ref(), context);
        assert_clean("replacement", fix.proposal.replacement.as_ref(), context);
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

    // Byte-level slicing: we don't assume fixtures are valid UTF-8
    // (fixtures are `.txt` today, but the engine operates on `&[u8]`
    // and the test must not be weaker than the engine's own contract).
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
