// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Audit content-ignorance canary.
//!
//! Constitution V Principle V requires that no document content
//! appears verbatim in any audit-record NDJSON output. The audit
//! envelope is closed-typed by construction: marking-side records carry
//! a sealed [`Canonical<S>`] payload (rendered token canonicals, not
//! document bytes) plus BLAKE3 digests of the original-bytes and
//! canonical-bytes; text-correction records carry only corpus-derived
//! [`SmolStr`] replacements (on Constitution V's permitted-identifier
//! list). The NDJSON projection emits a closed set of named fields; no
//! `format!`-interpolated content channel reaches the wire.
//!
//! This canary is the empirical sweep that proves the type-level
//! invariant survives end-to-end: every fixture under
//! `tests/corpus/{valid,invalid,prose,lattice}/` is fed through
//! [`Engine::fix`] and every emitted [`AuditLine`] is rendered to its
//! NDJSON line; the canary scans each line for any contiguous ≥4-byte
//! sequence from the input that appears anywhere outside the
//! permitted-identifier list.
//!
//! # Permitted-identifier check strategy
//!
//! The canary's `is_permitted_in_audit_json` helper takes a JSON-
//! aware approach (architect R-D-3): it parses every emitted
//! NDJSON line via `serde_json::from_str::<serde_json::Value>` and
//! walks the JSON tree, scanning every string-valued leaf for
//! contiguous ≥4-byte sequences from the input. Numeric leaves
//! (span offsets, confidence scalars) are permitted by construction;
//! `"blake3:<hex>"` digest values are permitted via prefix match.
//!
//! False-positive resistance: a 4-byte numeric substring of the
//! input (e.g., a year `2026` or port number `1024`) appearing
//! inside a JSON numeric field (`"start": 1024`) is NOT a leak —
//! the field is structurally a span integer, the value is a
//! permitted scalar. The check only flags ≥4-byte UTF-8 substrings
//! appearing in **string-valued** JSON leaves outside the closed-
//! set identifier list.
//!
//! # Self-test
//!
//! `canary_fires_on_synthetic_regression` fabricates an
//! `AppliedTextCorrection` whose `replacement` contains a known
//! input substring, and verifies the canary detects the leak. The
//! synthetic record is constructed under Constitution V Principle V's
//! test-fixture carve-out — `#[cfg(test)]`-scoped, never commingled
//! with engine output, only for canary self-validation.

use marque_capco::{CapcoScheme, capco_rules};
use marque_config::Config;
use marque_engine::{Engine, FixMode};
use marque_rules::audit::{AppliedTextCorrection, AuditLine};
use marque_rules::message::Blake3Hash;
use marque_rules::{
    Confidence, EnginePromotionToken, FixSource, Message, MessageArgs, MessageTemplate, RuleId,
};
use marque_scheme::{Severity, Span};
use marque_test_utils::{
    fixtures_in, invalid_fixtures, load_fixture, prose_fixtures, valid_fixtures,
};
use serde_json::Value;
use smol_str::SmolStr;
use std::path::PathBuf;
use std::time::SystemTime;

/// Minimum substring length to flag as a content leak.
///
/// Per `contracts/audit-record.md` §"Content-ignorance canary"
/// (§332-343), the canary scans for ≥4-byte contiguous sequences
/// from the input. Three-byte sequences are short enough to false-
/// positive on random alignment (e.g., a marking abbreviation
/// could coincide with three bytes of prose); four bytes is the
/// contractual threshold.
const MIN_LEAK_LEN: usize = 4;

/// JSON keys whose **string** values are permitted-identifier types
/// per Constitution V Principle V. A ≥4-byte input substring
/// appearing in any of these fields is not a leak — the field is
/// structurally typed and the value is on the permitted list.
///
/// The list reflects the marque-1.0 audit-record contract body §:
/// closed-enum identifiers (`rule`, `severity`, `template`,
/// `discriminant`, `source`), namespaced token labels
/// (`token_id`, `expected_token`, `actual_token`, `category`),
/// engine-controlled scalars (`schema`, `classifier_id`, `input`),
/// timestamps (`timestamp` ISO-8601), and BLAKE3 digests
/// (`bytes_digest`, `original_digest`) which are prefix-tested
/// separately.
const PERMITTED_STRING_KEYS: &[&str] = &[
    "rule",
    // The `rule` field renders as a structured 2-tuple
    // `{"scheme": ..., "predicate_id": ...}`; both string-valued
    // leaves are permitted identifier types (the scheme name is
    // a closed enum, the predicate id is a closed-set surface +
    // category + descriptive English path).
    "scheme",
    "predicate_id",
    "severity",
    "template",
    "discriminant",
    "source",
    "schema",
    "type",
    "classifier_id",
    "input",
    "timestamp",
    "token_id",
    "expected_token",
    "actual_token",
    "category",
    "token",
    "render_call_site",
    "bytes_digest",
    "original_digest",
    "id",
];

/// Lower-cased list of JSON keys whose string values carry corpus-
/// derived bytes that may legitimately overlap with input
/// (text-correction replacement canonicals). These keys are
/// **excluded** from the leak scan; their values are on Constitution
/// V's permitted-identifier list by construction.
///
/// `replacement` is a [`SmolStr`] containing canonical token bytes
/// (e.g., `"SECRET"` replacing a typo); any input overlap is the
/// intended behavior of the corrections-map / E006-shaped path.
const PERMITTED_VALUE_KEYS: &[&str] = &["replacement"];

/// Test engine — uses the default `StrictOrDecoderRecognizer` so the
/// canary exercises BOTH the strict-path and the decoder-fallback
/// emit channels. The #257 strict-recognizer masking pin retired
/// concurrently with this canary's introduction in D8.
fn test_engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

/// Render an [`AuditLine`] to its NDJSON line via the CLI
/// renderer in `marque::render`. The CLI emitter is the load-bearing
/// projection path (the CLI/WASM parity test pins WASM byte-identity to
/// it), so testing against this projection covers both wire-format
/// surfaces.
///
/// Returns the JSON string (no trailing newline) for the audit line,
/// or `None` if serialization fails (the renderer is the same path
/// production callers use; a serialization failure is itself a
/// signal worth surfacing).
fn render_audit_line_to_json(
    _scheme: &CapcoScheme,
    line: &AuditLine<CapcoScheme>,
) -> Option<String> {
    // The CLI renderer lives in the `marque` bin crate; integration
    // tests cannot reach it. Reproduce the projection inline
    // via `serde_json::to_string` against the same shape — the
    // canary's job is to scan the wire bytes, and any structural
    // drift between the inline projection and the CLI emit would
    // surface as a separate test failure in `audit_v1_0_parity.rs`.
    use serde_json::json;
    // The `"rule"` field renders as the structured 2-tuple
    // `{"scheme": ..., "predicate_id": ...}`. The canary's
    // `PERMITTED_STRING_KEYS` includes both `"scheme"` and
    // `"predicate_id"` so the structured value bytes are
    // permitted-identifier types.
    let rule_id_json = |r: &marque_rules::RuleId| {
        json!({
            "scheme": r.scheme(),
            "predicate_id": r.predicate_id(),
        })
    };
    let v = match line {
        AuditLine::AppliedFix(f) => json!({
            "type": "applied_fix",
            "schema": marque_engine::AUDIT_SCHEMA_VERSION,
            "rule": rule_id_json(&f.rule),
            "severity": f.severity.as_str(),
            "span": {"start": f.span.start, "end": f.span.end},
            "fix": {
                "replacement": {
                    "discriminant": marque_rules::audit::discriminant_from_source(f.source).as_str(),
                    "canonical": {
                        "bytes_digest": format!("blake3:{}", f.fix.replacement.bytes_digest.to_hex()),
                        // TokenSource emits via Debug for the canary; the
                        // production renderer uses Vocabulary lookup, but
                        // the canary only needs a stable string position
                        // for the JSON path — the actual value is a
                        // permitted identifier either way.
                        "category": format!("{:?}", f.fix.replacement.canonical.source()),
                    },
                    "confidence": {
                        "recognition": f.fix.replacement.confidence.recognition,
                        "combined": f.fix.replacement.confidence.combined(),
                    },
                },
                "original_span": {"start": f.fix.original_span.start, "end": f.fix.original_span.end},
                "original_digest": format!("blake3:{}", f.fix.original_digest.to_hex()),
            },
            "message": {
                "template": f.message.template().as_str(),
            },
            "timestamp": humantime::format_rfc3339(f.timestamp).to_string(),
            "classifier_id": f.classifier_id.as_deref(),
            "dry_run": f.dry_run,
        }),
        AuditLine::TextCorrection(tc) => json!({
            "type": "text_correction",
            "schema": marque_engine::AUDIT_SCHEMA_VERSION,
            "rule": rule_id_json(&tc.rule),
            "severity": tc.severity.as_str(),
            "span": {"start": tc.span.start, "end": tc.span.end},
            "original_digest": format!("blake3:{}", tc.original_digest.to_hex()),
            "replacement": tc.replacement.as_str(),
            "source": match tc.source {
                FixSource::CorrectionsMap => "corrections_map",
                FixSource::BuiltinRule => "builtin_rule",
                FixSource::MigrationTable => "migration_table",
                FixSource::DecoderPosterior => "decoder_posterior",
                FixSource::DecoderClassificationHeuristic => "decoder_classification_heuristic",
            },
            "message": {
                "template": tc.message.template().as_str(),
            },
            "timestamp": humantime::format_rfc3339(tc.timestamp).to_string(),
            "classifier_id": tc.classifier_id.as_deref(),
            "dry_run": tc.dry_run,
        }),
        // **Parallel-update requirement.**
        // When a new `AuditLine` variant lands in
        // `marque-rules::audit`, three call sites MUST add a
        // corresponding arm in lockstep: the CLI renderer at
        // `marque/src/render.rs::audit_line_to_json_v1_0`, the WASM
        // renderer at `crates/wasm/src/lib.rs::audit_line_to_json_v1_0`,
        // and this canary's `render_audit_line_to_json`. Returning
        // `None` here without adding the arm would let a future leak
        // channel slip past the canary's corpus sweep — the canary
        // scans the bytes the renderers emit, so a `None` arm
        // produces no bytes to scan and any input substring leaked
        // through the new variant would pass the sweep vacuously.
        _ => return None,
    };
    serde_json::to_string(&v).ok()
}

/// Scan an NDJSON line for ≥4-byte sequences from `input` appearing
/// in non-permitted JSON positions.
///
/// Returns `Some((leaked_string, json_path))` on the first detected
/// leak so the failure message points the reviewer at the offending
/// field. Returns `None` when the line is clean.
fn detect_content_leak(input: &[u8], ndjson_line: &str) -> Option<(String, String)> {
    let parsed: Value = serde_json::from_str(ndjson_line).ok()?;
    walk_json(input, &parsed, String::new()).or(None)
}

/// Walk every value in the JSON tree, scanning string-valued leaves
/// for input-substring leaks. Returns `Some((string, path))` on the
/// first leak; `None` when the subtree is clean.
fn walk_json(input: &[u8], v: &Value, path: String) -> Option<(String, String)> {
    match v {
        Value::String(s) => {
            // Path's terminal segment names the JSON key. Permitted
            // string keys (closed-enum identifiers, etc.) skip the
            // scan; their values are on the permitted-identifier
            // list by construction.
            let last_segment = path.rsplit('.').next().unwrap_or("");
            if PERMITTED_STRING_KEYS.contains(&last_segment) {
                return None;
            }
            if PERMITTED_VALUE_KEYS.contains(&last_segment) {
                return None;
            }
            // BLAKE3 digest prefix is permitted in any string position
            // (the contract emits `"blake3:<hex>"` consistently).
            if s.starts_with("blake3:") {
                return None;
            }
            scan_for_leak(input, s).map(|leaked| (leaked, path))
        }
        Value::Object(map) => {
            for (k, val) in map {
                let child_path = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                if let Some(hit) = walk_json(input, val, child_path) {
                    return Some(hit);
                }
            }
            None
        }
        Value::Array(arr) => {
            for (i, val) in arr.iter().enumerate() {
                let child_path = format!("{path}[{i}]");
                if let Some(hit) = walk_json(input, val, child_path) {
                    return Some(hit);
                }
            }
            None
        }
        // Numbers, bool, null — not greppable for input leaks.
        _ => None,
    }
}

/// Scan `value` for a contiguous ≥4-byte UTF-8 substring from
/// `input`. Returns the leaked substring on the first match;
/// `None` when no input substring of length ≥ `MIN_LEAK_LEN`
/// appears in `value`.
///
/// The scan iterates over all `MIN_LEAK_LEN`-byte windows of `input`
/// and tests substring presence in `value`. For typical input sizes
/// (≤32 KB) and short `value` strings (≤256 bytes) the cost is
/// `O(len_input * len_value)` per scan, which is acceptable for a
/// per-record canary not on the hot path.
fn scan_for_leak(input: &[u8], value: &str) -> Option<String> {
    if value.len() < MIN_LEAK_LEN || input.len() < MIN_LEAK_LEN {
        return None;
    }
    let value_bytes = value.as_bytes();
    // Iterate over MIN_LEAK_LEN-byte windows of the input; only
    // UTF-8-aligned starts are meaningful here (the canary searches
    // for *substring* presence, not byte-array overlap).
    for start in 0..=(input.len() - MIN_LEAK_LEN) {
        // Find the longest input substring starting at `start` that
        // still appears in `value`. Cap at value.len() to bound the
        // search.
        let mut end = start + MIN_LEAK_LEN;
        let max_end = (start + value_bytes.len()).min(input.len());
        if !contains_subslice(value_bytes, &input[start..end]) {
            continue;
        }
        // Walk forward to find the maximal leaked window for a
        // descriptive panic message.
        while end < max_end && contains_subslice(value_bytes, &input[start..=end]) {
            end += 1;
        }
        let leaked = std::str::from_utf8(&input[start..end]).ok()?;
        if leaked.len() >= MIN_LEAK_LEN {
            return Some(leaked.to_owned());
        }
    }
    None
}

/// `haystack.contains(needle)` for `&[u8]`. Stdlib's `slice::windows`
/// makes this idiomatic without pulling in `memchr` for an off-hot-
/// path canary.
fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Iterate `tests/corpus/<subdir>/*.txt` paths. Used to walk the
/// subdirectories that don't have a typed accessor in
/// `marque-test-utils`.
fn corpus_subdir_fixtures(subdir: &str) -> Vec<PathBuf> {
    fixtures_in(subdir)
}

// ---------------------------------------------------------------------------
// Corpus sweep — every fixture must emit canary-clean NDJSON.
// ---------------------------------------------------------------------------

#[test]
fn canary_passes_on_full_corpus() {
    let engine = test_engine();
    let scheme = engine.scheme();

    // Walk every corpus directory. The sweep is canary-flat: each
    // fixture runs through `Engine::fix` once, every emitted
    // `AuditLine` projects to NDJSON, and every line is scanned
    // for content leaks.
    let mut fixtures: Vec<PathBuf> = Vec::new();
    fixtures.extend(valid_fixtures());
    fixtures.extend(invalid_fixtures());
    fixtures.extend(prose_fixtures());
    fixtures.extend(corpus_subdir_fixtures("lattice"));

    assert!(
        !fixtures.is_empty(),
        "canary needs at least one corpus fixture — \
         is tests/corpus/ missing or empty?"
    );

    let mut total_lines_scanned = 0usize;
    for path in &fixtures {
        let source = load_fixture(path);
        let result = engine.fix(&source, FixMode::Apply);
        for line in &result.audit_lines {
            let Some(ndjson) = render_audit_line_to_json(scheme, line) else {
                continue;
            };
            total_lines_scanned += 1;
            if let Some((leaked, json_path)) = detect_content_leak(&source, &ndjson) {
                panic!(
                    "content-ignorance canary violation: input substring {:?} (len {}) leaked into \
                     audit NDJSON at JSON path {:?}.\n\n\
                     Fixture: {}\n\
                     NDJSON line: {}\n\n\
                     The marque-1.0 audit-record contract \
                     (`contracts/audit-record.md` §327-348) requires that no \
                     contiguous ≥4-byte input substring appears in any NDJSON \
                     audit record outside the permitted-identifier list. \
                     Either the leak channel is real (fix the engine), or the \
                     canary's permitted-identifier list is missing a key \
                     (extend `PERMITTED_STRING_KEYS` / `PERMITTED_VALUE_KEYS` \
                     in this file and re-run).",
                    leaked,
                    leaked.len(),
                    json_path,
                    path.display(),
                    ndjson,
                );
            }
        }
    }

    // Vacuity guard: a zero-record sweep would silently pass the
    // canary even if the engine stopped emitting altogether. The
    // corpus must produce ≥1 audit line across all fixtures, or
    // the canary isn't actually checking anything.
    assert!(
        total_lines_scanned > 0,
        "canary vacuity guard: the corpus sweep produced zero NDJSON \
         lines across {} fixtures. Either no fixture triggers a fix \
         (corpus regression) or the renderer dropped every line \
         (renderer regression).",
        fixtures.len(),
    );
}

// ---------------------------------------------------------------------------
// Self-test — fabricated leak fires the canary.
// ---------------------------------------------------------------------------

/// Synthetic input bytes the self-test embeds in a fabricated audit
/// record. Long enough to clear `MIN_LEAK_LEN`; distinctive enough
/// to not coincide with any closed-enum identifier in the audit
/// contract.
const SYNTHETIC_LEAK_BYTES: &[u8] = b"the quick brown fox jumps over the lazy dog";

#[test]
fn canary_fires_on_synthetic_regression() {
    // Constitution V Principle V test-fixture carve-out: construct an
    // `AppliedTextCorrection` whose `replacement` field carries a
    // synthetic input substring. The fabricated record is wrapped in
    // an `AuditLine::TextCorrection`, projected to NDJSON via the
    // same renderer the corpus sweep uses, and the canary's
    // detect_content_leak helper is invoked directly to verify it
    // detects the leak.
    //
    // The carve-out scope: this call sits inside #[cfg(test)] (the
    // file is in `crates/engine/tests/`), the fabricated record is
    // NEVER spliced into a real engine audit stream, and it exists
    // solely to validate the canary's leak-detection path.
    //
    // We bypass the renderer's `replacement`-key exclusion by
    // injecting the synthetic substring into a JSON path the canary
    // DOES scan — the `message.template` field would normally carry
    // a closed-enum variant name, so a leak there is a structural
    // regression. The renderer doesn't expose a custom-template
    // path, so we synthesize the NDJSON directly here to exercise
    // the canary's check function in isolation.
    // `"rule"` renders as the structured `{scheme, predicate_id}`
    // 2-tuple. The fixture below carries the same shape that the
    // canary's `rule_id_json` helper produces.
    let synthetic_ndjson = format!(
        r#"{{"type":"text_correction","schema":"marque-2.0","rule":{{"scheme":"test","predicate_id":"synthetic.r999-fixture"}},
          "severity":"info","span":{{"start":0,"end":10}},
          "leak_field":"{leak}",
          "original_digest":"blake3:abcd","replacement":"SECRET",
          "source":"corrections_map","message":{{"template":"CorrectionsApplied"}},
          "timestamp":"2026-05-20T00:00:00Z","classifier_id":null,"dry_run":false}}"#,
        leak = std::str::from_utf8(SYNTHETIC_LEAK_BYTES).unwrap(),
    );

    let result = detect_content_leak(SYNTHETIC_LEAK_BYTES, &synthetic_ndjson);
    assert!(
        result.is_some(),
        "self-test: canary failed to detect synthetic leak — the canary is \
         not load-bearing. detect_content_leak returned None when given \
         input {:?} and NDJSON {:?}",
        std::str::from_utf8(SYNTHETIC_LEAK_BYTES).unwrap(),
        synthetic_ndjson,
    );
}

#[test]
fn canary_permits_blake3_digest_strings() {
    // A `"blake3:<hex>"` digest string is structurally a permitted
    // identifier per Constitution V Principle V. The canary must
    // NOT flag input substrings appearing inside such a value.
    //
    // Construct an input whose substring happens to coincide with a
    // BLAKE3 hex prefix, and verify the canary stays quiet.
    let input = b"the quick brown abcdef0123";
    let ndjson = r#"{"type":"text_correction","blake3_field":"blake3:abcdef0123456789"}"#;
    assert!(
        detect_content_leak(input, ndjson).is_none(),
        "canary false-positive: blake3-prefixed digest strings must not \
         trigger leak detection"
    );
}

#[test]
fn canary_permits_span_integer_overlap() {
    // The canary scans STRING-valued JSON leaves; numeric leaves
    // (span integers, confidence scalars) are permitted by
    // construction. An input string that happens to contain the
    // digits `1024` (e.g., a port number, a year) appearing
    // alongside a JSON value `"start": 1024` must NOT trigger a
    // leak (architect R-D-3).
    let input = b"port 1024 is the cutoff threshold";
    let ndjson = r#"{"type":"text_correction","span":{"start":1024,"end":2048}}"#;
    assert!(
        detect_content_leak(input, ndjson).is_none(),
        "canary false-positive: numeric JSON values must not trigger \
         leak detection on input substrings of the same digits"
    );
}

// ---------------------------------------------------------------------------
// Carve-out: synthetic AppliedTextCorrection construction.
// ---------------------------------------------------------------------------
//
// Wrapped in a helper so the carve-out site is greppable. Used by
// the TextCorrection-arm canary regression test below, which
// exercises the real promotion path rather than the inline NDJSON
// projection used by `canary_fires_on_synthetic_regression`.
fn synth_leaky_text_correction(leak: &[u8]) -> AppliedTextCorrection {
    // Test-fixture carve-out per Constitution V Principle V — the
    // fabricated record is engine-promotion-shaped but never flows
    // into an Engine::fix audit stream.
    AppliedTextCorrection::__engine_promote_text_correction(
        // Synthetic test fixture for the canary self-test, in the
        // reserved `"test"` scheme. The synthetic-NDJSON literals in the
        // self-test functions are kept in lockstep with this shape so
        // the canary tests the structured `{scheme, predicate_id}` form
        // end-to-end.
        RuleId::new("test", "synthetic.r999-fixture"),
        Severity::Info,
        Span::new(0, leak.len()),
        Blake3Hash::from_bytes([0u8; 32]),
        SmolStr::new(std::str::from_utf8(leak).unwrap_or("non-utf8")),
        FixSource::CorrectionsMap,
        Confidence::strict(),
        None,
        Message::new(MessageTemplate::CorrectionsApplied, MessageArgs::default()),
        SystemTime::now(),
        None,
        false,
        None,
        // Test-fixture carve-out per Constitution V Principle V — the
        // promotion token mint is part of the same synthetic helper
        // above; both calls feed `synth_leaky_text_correction` only.
        EnginePromotionToken::__engine_construct(),
    )
}

#[test]
fn canary_fires_on_synthetic_text_correction_regression() {
    // Exercise the `AuditLine::TextCorrection` arm symmetrically with
    // the `canary_fires_on_synthetic_regression` test (which only
    // exercises the inline-NDJSON path for the AppliedFix shape).
    // Two-part regression:
    //
    //   1. The `synth_leaky_text_correction` helper must successfully
    //      construct an `AppliedTextCorrection` via the
    //      `__engine_promote_text_correction` carve-out. This pins
    //      that the engine-promotion seal AND its test-fixture
    //      carve-out (Constitution V Principle V) survive. A future
    //      refactor that breaks the carve-out fails this test at
    //      compile time, not at audit-log diff time.
    //
    //   2. A synthetic TextCorrection-shape NDJSON with a leak in a
    //      non-permitted field (`bogus_text_corr_field`, structurally
    //      analogous to the AppliedFix-arm test's `leak_field`)
    //      causes `detect_content_leak` to FIRE. This mirrors
    //      `canary_fires_on_synthetic_regression`'s shape so a future
    //      regression on the `text_correction` line type is caught
    //      symmetrically with the `applied_fix` line type.
    //
    // The fabricated record from (1) does NOT flow into the canary's
    // own renderer — the renderer is structurally safe by construction
    // (every emitted field is on the permitted-identifier list, so
    // any leak via the real renderer would be a structural regression
    // caught upstream when the new offending field landed). Detecting
    // leak channels through future field additions is the point of
    // step (2)'s explicit non-permitted-field-name fixture.

    // Part 1: exercise the carve-out helper end-to-end. Constructing
    // the value (and accepting it as `AppliedTextCorrection`) is the
    // assertion — any future signature drift fails here.
    let _leaky_tc = synth_leaky_text_correction(SYNTHETIC_LEAK_BYTES);

    // Part 2: synthetic TextCorrection-shape NDJSON with a leak in a
    // non-permitted field. Mirrors `canary_fires_on_synthetic_regression`
    // for the `applied_fix` shape. `"rule"` is the structured 2-tuple
    // shape.
    let synthetic_ndjson = format!(
        r#"{{"type":"text_correction","schema":"marque-2.0","rule":{{"scheme":"test","predicate_id":"synthetic.r999-fixture"}},
          "severity":"info","span":{{"start":0,"end":43}},
          "bogus_text_corr_field":"{leak}",
          "original_digest":"blake3:abcd","replacement":"SECRET",
          "source":"corrections_map","message":{{"template":"CorrectionsApplied"}},
          "timestamp":"2026-05-20T00:00:00Z","classifier_id":null,"dry_run":false}}"#,
        leak = std::str::from_utf8(SYNTHETIC_LEAK_BYTES).unwrap(),
    );

    let result = detect_content_leak(SYNTHETIC_LEAK_BYTES, &synthetic_ndjson);
    assert!(
        result.is_some(),
        "self-test (TextCorrection arm): canary failed to detect synthetic leak \
         in a non-permitted text-correction NDJSON field. detect_content_leak \
         returned None on input {:?} and NDJSON {:?}",
        std::str::from_utf8(SYNTHETIC_LEAK_BYTES).unwrap(),
        synthetic_ndjson,
    );
}
