// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use marque_capco::CapcoScheme;
use marque_rules::{Diagnostic, FixSource, RuleId};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// JSON serialization types — duplicated from CLI render.rs for SC-008 parity.
// The parity test (T061) catches any divergence.
// ---------------------------------------------------------------------------

/// JSON projection of a `Diagnostic` conforming to `contracts/diagnostic.json`.
///
/// PR 3c.2.C C5 changed the `message` and `citation` fields' wire
/// shape per PM-C-7:
/// - `message` is now a structured object `{ "template": "..." }`
///   (was a free-form string). Phase-1 carries the template label
///   only; the closed `MessageArgs` payload is intentionally not
///   serialized today and will be added when audit renderers need
///   the structured field set. See [`MessageJson`] below for the
///   per-shape rationale.
/// - `citation` is now the [`Display`] form of typed [`Citation`]
///   — `§<L>.<sub> p<page>` for CAPCO sources, `[config]` /
///   `[engine-internal]` for sentinel sources.
///
/// Documented in PR 3c.2.C PR description.
#[derive(Debug, Serialize)]
pub(crate) struct DiagnosticJson<'a> {
    /// 2-tuple `RuleId` shape per T044 PM OD-2. Mirrors
    /// [`marque::render::DiagnosticJson`] for CLI/WASM NDJSON byte-
    /// identity (SC-008). See [`RuleIdJson`].
    rule: RuleIdJson<'a>,
    severity: &'a str,
    span: SpanJson,
    message: MessageJson<'a>,
    citation: String,
    fix: Option<FixJson<'a>>,
    /// Decoder-recognized canonical form (issue #699). Mirrors
    /// `marque::render::DiagnosticJson::recognized_canonical` for
    /// CLI/WASM NDJSON byte-identity (SC-008). Audit-side WASM NDJSON
    /// does NOT mirror this field — Constitution V Principle V / G13.
    #[serde(skip_serializing_if = "Option::is_none")]
    recognized_canonical: Option<&'a str>,
}

/// JSON projection of a [`RuleId`] as a `{scheme, predicate_id}` 2-tuple
/// object (T044 PM OD-2). Mirrors `marque::render::RuleIdJson` for
/// byte-identical NDJSON parity (SC-008). The two crates carry parallel
/// type definitions per architect D-D-1 (shared `marque-audit-render`
/// crate deferred to post-PR-10).
#[derive(Debug, Serialize)]
struct RuleIdJson<'a> {
    scheme: &'a str,
    predicate_id: &'a str,
}

impl<'a> From<&'a RuleId> for RuleIdJson<'a> {
    fn from(r: &'a RuleId) -> Self {
        Self {
            scheme: r.scheme(),
            predicate_id: r.predicate_id(),
        }
    }
}

/// Structured JSON projection of a [`Message`].
///
/// Phase-1 wire shape (PR 3c.2.C): `{ "template": "..." }` only.
/// `template` is the [`MessageTemplate::as_str`] canonical label.
///
/// `args` is intentionally NOT serialized in phase 1 — the closed
/// `MessageArgs` payload (typed `TokenId` / `CategoryId` / `Span` /
/// `Blake3Hash` / `Confidence` / `FeatureId` / `RuleId`) requires a
/// per-template arg-flattening serializer that downstream consumers
/// don't yet need. A future PR will add the `args` field when audit
/// renderers demand the structured field set.
#[derive(Debug, Serialize)]
struct MessageJson<'a> {
    template: &'a str,
}

#[derive(Debug, Serialize)]
struct SpanJson {
    start: usize,
    end: usize,
}

#[derive(Debug, Serialize)]
struct FixJson<'a> {
    source: &'static str,
    intent_kind: &'static str,
    replacement: Option<&'a str>,
    confidence: f32,
    migration_ref: Option<&'a str>,
}

fn fix_source_str(source: FixSource) -> &'static str {
    match source {
        FixSource::BuiltinRule => "BuiltinRule",
        FixSource::CorrectionsMap => "CorrectionsMap",
        FixSource::MigrationTable => "MigrationTable",
        FixSource::DecoderPosterior => "DecoderPosterior",
        FixSource::DecoderClassificationHeuristic => "DecoderClassificationHeuristic",
    }
}

pub(crate) fn diagnostic_to_json(d: &Diagnostic<CapcoScheme>) -> DiagnosticJson<'_> {
    // Principle II readout — projecting the decoder-recognized
    // canonical bytes into the WASM-side NDJSON surface (issue #699).
    // Mirrors `marque::render::diagnostic_to_json` for SC-008 byte-
    // identical NDJSON parity. Defensive `from_utf8` guard (the engine
    // validates UTF-8 before populating `recognized_canonical`).
    let recognized_canonical = d
        .recognized_canonical
        .as_ref()
        .and_then(|sb| std::str::from_utf8(secrecy::ExposeSecret::expose_secret(sb)).ok());
    DiagnosticJson {
        rule: (&d.rule).into(),
        severity: d.severity.as_str(),
        span: SpanJson {
            start: d.span.start,
            end: d.span.end,
        },
        message: MessageJson {
            template: d.message.template().as_str(),
        },
        citation: d.citation.to_string(),
        fix: match (d.fix.as_ref(), d.text_correction.as_ref()) {
            (Some(f), _) => Some(FixJson {
                source: fix_source_str(f.source),
                intent_kind: match &f.replacement {
                    marque_scheme::ReplacementIntent::FactAdd { .. } => "FactAdd",
                    marque_scheme::ReplacementIntent::FactRemove { .. } => "FactRemove",
                    marque_scheme::ReplacementIntent::Recanonicalize { .. } => "Recanonicalize",
                    _ => "Unknown",
                },
                replacement: None,
                confidence: f.confidence.combined(),
                migration_ref: f.migration_ref,
            }),
            (None, Some(tc)) => Some(FixJson {
                source: fix_source_str(tc.source),
                intent_kind: "TextCorrection",
                replacement: Some(tc.replacement.as_ref()),
                confidence: tc.confidence.combined(),
                migration_ref: tc.migration_ref,
            }),
            (None, None) => None,
        },
        recognized_canonical,
    }
}

// ---------------------------------------------------------------------------
// `marque-1.0` audit-record JSON projection (PR 3c.2.D / D5)
//
// Mirrors the CLI's `marque/src/render.rs` v1.0 surface — CLI and WASM
// emit byte-identical NDJSON for SC-008 parity. The struct shapes are
// duplicated verbatim per architect D-D-1 (shared `marque-audit-render`
// crate deferred to post-PR-10); `crates/wasm/tests/audit_v1_0_parity.rs`
// pins the byte-identity at integration-test time (PM-D-16 / R-D-1).
// ---------------------------------------------------------------------------

/// Mirrors `marque::render::AuditRecordJsonV1_0`. Contract §107-178.
#[derive(Debug, Serialize)]
struct AuditRecordJsonV1_0<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    schema: &'static str,
    /// 2-tuple `RuleId` per T044 PM OD-2. See [`RuleIdJson`].
    rule: RuleIdJson<'a>,
    severity: &'static str,
    span: SpanJson,
    fix: AuditFixJsonV1_0<'a>,
    message: AuditMessageJsonV1_0<'a>,
    timestamp: String,
    classifier_id: Option<&'a str>,
    dry_run: bool,
    input: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct AuditFixJsonV1_0<'a> {
    replacement: AuditReplacementJsonV1_0<'a>,
    original_span: SpanJson,
    original_digest: String,
}

#[derive(Debug, Serialize)]
struct AuditReplacementJsonV1_0<'a> {
    discriminant: &'static str,
    canonical: AuditCanonicalJsonV1_0<'a>,
    confidence: AuditConfidenceJsonV1_0<'a>,
}

#[derive(Debug, Serialize)]
struct AuditCanonicalJsonV1_0<'a> {
    source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    token_id: Option<std::borrow::Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    render_call_site: Option<String>,
    bytes_digest: String,
}

#[derive(Debug, Serialize)]
struct AuditConfidenceJsonV1_0<'a> {
    recognition: f32,
    rule: f32,
    combined: f32,
    region: Option<f32>,
    runner_up_ratio: Option<f32>,
    features: Vec<AuditFeatureJsonV1_0<'a>>,
}

#[derive(Debug, Serialize)]
struct AuditFeatureJsonV1_0<'a> {
    id: &'a str,
    delta: f32,
}

#[derive(Debug, Serialize)]
struct AuditMessageJsonV1_0<'a> {
    template: &'static str,
    args: serde_json::Map<String, serde_json::Value>,
    #[serde(skip)]
    _marker: std::marker::PhantomData<&'a ()>,
}

/// Mirrors `marque::render::TextCorrectionRecordJsonV1_0`. Contract §388-402.
#[derive(Debug, Serialize)]
struct TextCorrectionRecordJsonV1_0<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    schema: &'static str,
    /// 2-tuple `RuleId` per T044 PM OD-2. See [`RuleIdJson`].
    rule: RuleIdJson<'a>,
    severity: &'static str,
    span: SpanJson,
    original_digest: String,
    replacement: &'a str,
    source: &'static str,
    confidence: AuditConfidenceJsonV1_0<'a>,
    migration_ref: Option<&'a str>,
    message: AuditMessageJsonV1_0<'a>,
    timestamp: String,
    classifier_id: Option<&'a str>,
    dry_run: bool,
    input: Option<&'a str>,
}

fn blake3_audit_string_v1_0(hash: &blake3::Hash) -> String {
    format!("blake3:{}", hash.to_hex())
}

fn format_timestamp_v1_0(ts: std::time::SystemTime) -> String {
    humantime::format_rfc3339(ts).to_string()
}

/// Resolve a `CategoryId` to its lowercase scheme-name label. Mirrors
/// the CLI's `category_label`. `CategoryId::MARKING` → `"Marking"`;
/// scheme-registered categories project through their `Category.name`.
fn category_label_v1_0(
    scheme: &CapcoScheme,
    category_id: marque_scheme::CategoryId,
) -> &'static str {
    use marque_scheme::MarkingScheme;
    if category_id == marque_scheme::CategoryId::MARKING {
        return "Marking";
    }
    scheme
        .categories()
        .iter()
        .find(|c| c.id == category_id)
        .map(|c| c.name)
        .unwrap_or("unknown")
}

fn project_canonical_to_json_v1_0<'a>(
    scheme: &'a CapcoScheme,
    canonical: &marque_scheme::Canonical<CapcoScheme>,
    precomputed_bytes_digest: &blake3::Hash,
) -> AuditCanonicalJsonV1_0<'a> {
    use marque_scheme::Vocabulary;
    use marque_scheme::canonical::TokenSource;
    let digest = blake3_audit_string_v1_0(precomputed_bytes_digest);
    match canonical.source() {
        TokenSource::Cve(token_id) => {
            let label =
                <CapcoScheme as Vocabulary<CapcoScheme>>::qualified_token_label(scheme, token_id);
            AuditCanonicalJsonV1_0 {
                source: "cve",
                token_id: Some(label),
                category: None,
                render_call_site: None,
                bytes_digest: digest,
            }
        }
        TokenSource::OpenVocab {
            category,
            render_call_site,
        } => AuditCanonicalJsonV1_0 {
            source: "open_vocab",
            token_id: None,
            category: Some(category_label_v1_0(scheme, *category)),
            render_call_site: Some(format!(
                "{}:{}",
                render_call_site.file(),
                render_call_site.line(),
            )),
            bytes_digest: digest,
        },
    }
}

fn project_confidence_to_json_v1_0(
    confidence: &marque_rules::Confidence,
) -> AuditConfidenceJsonV1_0<'_> {
    AuditConfidenceJsonV1_0 {
        recognition: confidence.recognition,
        rule: confidence.rule,
        combined: confidence.combined(),
        region: confidence.region,
        runner_up_ratio: confidence.runner_up_ratio,
        features: confidence
            .features
            .iter()
            .map(|f| AuditFeatureJsonV1_0 {
                id: f.id.as_str(),
                delta: f.delta,
            })
            .collect(),
    }
}

fn project_message_to_json_v1_0<'a>(
    scheme: &'a CapcoScheme,
    message: &marque_rules::Message,
) -> AuditMessageJsonV1_0<'a> {
    use marque_scheme::Vocabulary;
    let mut args = serde_json::Map::new();
    let m = message.args();
    if let Some(token_id) = m.token {
        let label =
            <CapcoScheme as Vocabulary<CapcoScheme>>::qualified_token_label(scheme, &token_id);
        args.insert(
            "token".to_owned(),
            serde_json::Value::String(label.into_owned()),
        );
    }
    if let Some(category_id) = m.category {
        args.insert(
            "category".to_owned(),
            serde_json::Value::String(category_label_v1_0(scheme, category_id).to_owned()),
        );
    }
    if let Some(span) = m.span {
        args.insert(
            "span".to_owned(),
            serde_json::json!({ "start": span.start, "end": span.end }),
        );
    }
    if let Some(digest) = m.digest {
        args.insert(
            "digest".to_owned(),
            serde_json::Value::String(blake3_audit_string_v1_0(&digest)),
        );
    }
    if let Some(ref confidence) = m.confidence {
        args.insert(
            "confidence".to_owned(),
            serde_json::to_value(project_confidence_to_json_v1_0(confidence))
                .unwrap_or(serde_json::Value::Null),
        );
    }
    if let Some(expected_token) = m.expected_token {
        let label = <CapcoScheme as Vocabulary<CapcoScheme>>::qualified_token_label(
            scheme,
            &expected_token,
        );
        args.insert(
            "expected_token".to_owned(),
            serde_json::Value::String(label.into_owned()),
        );
    }
    if let Some(actual_token) = m.actual_token {
        let label =
            <CapcoScheme as Vocabulary<CapcoScheme>>::qualified_token_label(scheme, &actual_token);
        args.insert(
            "actual_token".to_owned(),
            serde_json::Value::String(label.into_owned()),
        );
    }
    if !m.feature_ids.is_empty() {
        args.insert(
            "feature_ids".to_owned(),
            serde_json::Value::Array(
                m.feature_ids
                    .iter()
                    .map(|f| serde_json::Value::String(f.as_str().to_owned()))
                    .collect(),
            ),
        );
    }
    if !m.contributing_rule_ids.is_empty() {
        args.insert(
            "contributing_rule_ids".to_owned(),
            serde_json::Value::Array(
                m.contributing_rule_ids
                    .iter()
                    // T044: `RuleId.as_str()` is removed; render via the
                    // `Display` impl as the wire-string form
                    // `"<scheme>:<predicate_id>"`. Matches the CLI
                    // emitter for SC-008 NDJSON byte-identity.
                    .map(|r| serde_json::Value::String(r.to_string()))
                    .collect(),
            ),
        );
    }
    AuditMessageJsonV1_0 {
        template: message.template().as_str(),
        args,
        _marker: std::marker::PhantomData,
    }
}

fn applied_fix_to_audit_json_v1_0<'a>(
    scheme: &'a CapcoScheme,
    fix: &'a marque_rules::audit::AppliedFix<CapcoScheme>,
) -> AuditRecordJsonV1_0<'a> {
    use marque_rules::audit::discriminant_from_source;
    let replacement = AuditReplacementJsonV1_0 {
        discriminant: discriminant_from_source(fix.source).as_str(),
        canonical: project_canonical_to_json_v1_0(
            scheme,
            &fix.fix.replacement.canonical,
            &fix.fix.replacement.bytes_digest,
        ),
        confidence: project_confidence_to_json_v1_0(&fix.fix.replacement.confidence),
    };
    let fix_detail = AuditFixJsonV1_0 {
        replacement,
        original_span: SpanJson {
            start: fix.fix.original_span.start,
            end: fix.fix.original_span.end,
        },
        original_digest: blake3_audit_string_v1_0(&fix.fix.original_digest),
    };
    AuditRecordJsonV1_0 {
        kind: "applied_fix",
        schema: marque_engine::AUDIT_SCHEMA_VERSION,
        rule: (&fix.rule).into(),
        severity: fix.severity.as_str(),
        span: SpanJson {
            start: fix.span.start,
            end: fix.span.end,
        },
        fix: fix_detail,
        message: project_message_to_json_v1_0(scheme, &fix.message),
        timestamp: format_timestamp_v1_0(fix.timestamp),
        classifier_id: fix.classifier_id.as_deref(),
        dry_run: fix.dry_run,
        input: fix.input.as_deref(),
    }
}

fn text_correction_to_audit_json_v1_0<'a>(
    scheme: &'a CapcoScheme,
    tc: &'a marque_rules::audit::AppliedTextCorrection,
) -> TextCorrectionRecordJsonV1_0<'a> {
    TextCorrectionRecordJsonV1_0 {
        kind: "text_correction",
        schema: marque_engine::AUDIT_SCHEMA_VERSION,
        rule: (&tc.rule).into(),
        severity: tc.severity.as_str(),
        span: SpanJson {
            start: tc.span.start,
            end: tc.span.end,
        },
        original_digest: blake3_audit_string_v1_0(&tc.original_digest),
        replacement: tc.replacement.as_str(),
        source: fix_source_str(tc.source),
        confidence: project_confidence_to_json_v1_0(&tc.confidence),
        migration_ref: tc.migration_ref,
        message: project_message_to_json_v1_0(scheme, &tc.message),
        timestamp: format_timestamp_v1_0(tc.timestamp),
        classifier_id: tc.classifier_id.as_deref(),
        dry_run: tc.dry_run,
        input: tc.input.as_deref(),
    }
}

/// Dispatch an [`AuditLine<CapcoScheme>`] to its v1.0 JSON projection.
/// Mirrors the CLI's `audit_line_to_json_v1_0`.
///
/// `pub(crate)` so the SC-008 parity test at `tests/audit_v1_0_parity.rs`
/// can compare byte-identity against the CLI's projection without
/// reimplementing the helper in the test harness.
pub fn audit_line_to_json_v1_0(
    scheme: &CapcoScheme,
    line: &marque_rules::audit::AuditLine<CapcoScheme>,
) -> serde_json::Value {
    use marque_rules::audit::AuditLine;
    match line {
        AuditLine::AppliedFix(fix) => {
            serde_json::to_value(applied_fix_to_audit_json_v1_0(scheme, fix))
                .unwrap_or(serde_json::Value::Null)
        }
        AuditLine::TextCorrection(tc) => {
            serde_json::to_value(text_correction_to_audit_json_v1_0(scheme, tc))
                .unwrap_or(serde_json::Value::Null)
        }
        // **Parallel-update requirement** (PR 3c.2.D fixup F-10).
        // When a new `AuditLine` variant lands in
        // `marque-rules::audit`, three call sites MUST add a
        // corresponding arm in lockstep: the CLI renderer at
        // `marque/src/render.rs::audit_line_to_json_v1_0`, this
        // WASM renderer, and the canary's
        // `render_audit_line_to_json` at
        // `crates/engine/tests/audit_g13_canary.rs`. A silent
        // `Value::Null` here would defeat both the SC-008 byte-
        // identity parity test (the CLI and WASM emit shapes would
        // diverge silently) AND the G13 content-ignorance canary
        // (a future leak channel would emit nothing for the canary
        // to scan and pass the sweep vacuously).
        _ => serde_json::Value::Null,
    }
}

/// Serialize a single `AuditLine` to the v1.0 NDJSON wire form. WASM
/// counterpart of the CLI's `render_audit_line`. SC-008 binding
/// constraint: this function MUST produce byte-identical output to
/// the CLI's `render_audit_line` (modulo the trailing newline, which
/// the renderer appends and the in-memory serialization here omits —
/// the per-line JSON object is the byte-identity unit).
pub(crate) fn serialize_audit_line_v1_0(
    scheme: &CapcoScheme,
    line: &marque_rules::audit::AuditLine<CapcoScheme>,
) -> Result<Box<serde_json::value::RawValue>, String> {
    // Single accepted schema (`marque-2.0`) so dispatch is a no-op
    // today; the const lookup is kept so a future schema bump can
    // land via the same dispatch shape without restructuring callers.
    let _ = marque_engine::AUDIT_SCHEMA_IS_V2_0;
    let json =
        serde_json::to_string(&audit_line_to_json_v1_0(scheme, line)).map_err(|e| e.to_string())?;
    serde_json::value::RawValue::from_string(json).map_err(|e| e.to_string())
}

/// Wrapper for `fix()` output.
#[derive(Debug, Serialize)]
pub(crate) struct FixResultJson {
    pub(crate) fixed_text: String,
    pub(crate) applied: Vec<Box<serde_json::value::RawValue>>,
    pub(crate) remaining: Vec<Box<serde_json::value::RawValue>>,
    /// Mirrors [`marque_engine::FixResult::r002_fired`]. Serialized
    /// at the top level of the JS-object so callers can branch on a
    /// single field read without parsing the NDJSON `remaining`
    /// stream (PR 7b D1 binding constraint).
    pub(crate) r002_fired: bool,
}

// ---------------------------------------------------------------------------
// Batch types — lint_batch accepts an array of {id, text} entries and returns
// an array of {id, diagnostics} results in a single WASM boundary crossing.
// ---------------------------------------------------------------------------

/// One entry in a `lint_batch` request.
#[derive(Deserialize)]
pub(crate) struct BatchEntry {
    pub(crate) id: String,
    pub(crate) text: String,
}

/// One result in a `lint_batch` response.
#[derive(Serialize)]
pub(crate) struct BatchResultEntry<'a> {
    pub(crate) id: &'a str,
    pub(crate) diagnostics: Vec<Box<serde_json::value::RawValue>>,
}

/// Body shape for a deadline-exceeded fix error (mirrors the
/// `marque-server::DeadlineExceededBody` 504 response). Embedded as a
/// JSON string in the `Err` arm of `fix_native` so JS callers can
/// `JSON.parse(error.message)` to recover the partial-lint
/// diagnostics + candidate counts.
#[derive(Serialize)]
pub(crate) struct DeadlineExceededBodyJson<'a> {
    pub(crate) truncated_by: &'static str,
    pub(crate) candidates_processed: usize,
    pub(crate) candidates_total: usize,
    pub(crate) diagnostics: Vec<DiagnosticJson<'a>>,
}

/// Fallback payload when the primary serialization fails. Carries
/// only the `truncated_by` discriminator and an `error` message —
/// no diagnostics, no counts. Serialized via `serde_json::to_string`
/// so the `error` field is correctly JSON-escaped if the inner
/// message happens to contain quotes or backslashes (e.g., a
/// `serde_json::Error` formatted with a path that includes those
/// characters).
#[derive(Serialize)]
pub(crate) struct DeadlineExceededFallback<'a> {
    pub(crate) truncated_by: &'static str,
    pub(crate) error: &'a str,
}

pub(crate) fn deadline_exceeded_payload(partial_lint: &marque_engine::LintResult) -> String {
    let truncated_by = if partial_lint.truncated {
        "lint"
    } else {
        "fix"
    };
    let body = DeadlineExceededBodyJson {
        truncated_by,
        candidates_processed: partial_lint.candidates_processed,
        candidates_total: partial_lint.candidates_total,
        diagnostics: partial_lint
            .diagnostics
            .iter()
            .map(diagnostic_to_json)
            .collect(),
    };
    // The primary path serializes a struct of basic types; serde_json
    // failure here would imply a fundamental serializer bug. The
    // fallback exists for defense-in-depth — and crucially, it
    // round-trips through `serde_json::to_string` so the `error`
    // field is properly JSON-escaped. A `format!(r#"..."{e}"..."#)`
    // would produce invalid JSON if `e` contained a quote or
    // backslash; JS callers parsing the message as JSON would then
    // see a parse error instead of the structured shape we promised.
    match serde_json::to_string(&body) {
        Ok(s) => s,
        Err(primary_err) => {
            let fallback = DeadlineExceededFallback {
                truncated_by,
                error: &primary_err.to_string(),
            };
            // If even this micro-payload fails to serialize, return a
            // hand-built constant — no interpolation, no escaping
            // hazards. We accept losing the original error message in
            // this terminal-case-of-a-terminal-case path.
            serde_json::to_string(&fallback).unwrap_or_else(|_| {
                r#"{"truncated_by":"fix","error":"deadline-exceeded payload serialization failed"}"#
                    .to_owned()
            })
        }
    }
}
