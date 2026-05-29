// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use marque_capco::CapcoScheme;
use marque_rules::{Diagnostic, FixSource, RuleId};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// JSON serialization types — duplicated from CLI render.rs for byte-identical
// NDJSON parity. The native parity test catches any divergence.
// ---------------------------------------------------------------------------

/// JSON projection of a `Diagnostic` conforming to `contracts/diagnostic.json`.
///
/// Wire shape of the `message` and `citation` fields:
/// - `message` is a structured object `{ "template": "..." }`. It
///   carries the template label only; the closed `MessageArgs` payload
///   is intentionally not serialized today and will be added when audit
///   renderers need the structured field set. See [`MessageJson`] below
///   for the per-shape rationale.
/// - `citation` is the [`Display`] form of typed [`Citation`]
///   — `§<L>.<sub> p<page>` for CAPCO sources, `[config]` /
///   `[engine-internal]` for sentinel sources.
#[derive(Debug, Serialize)]
pub(crate) struct DiagnosticJson<'a> {
    /// 2-tuple `RuleId` shape. Mirrors
    /// [`marque::render::DiagnosticJson`] for CLI/WASM NDJSON byte-
    /// identity. See [`RuleIdJson`].
    rule: RuleIdJson<'a>,
    severity: &'a str,
    span: SpanJson,
    message: MessageJson<'a>,
    citation: String,
    fix: Option<FixJson<'a>>,
    /// Decoder-recognized canonical form (issue #699). Mirrors
    /// `marque::render::DiagnosticJson::recognized_canonical` for
    /// CLI/WASM NDJSON byte-identity. Audit-side WASM NDJSON does NOT
    /// mirror this field — Constitution V Principle V (audit
    /// content-ignorance).
    #[serde(skip_serializing_if = "Option::is_none")]
    recognized_canonical: Option<&'a str>,
}

/// JSON projection of a [`RuleId`] as a `{scheme, predicate_id}` 2-tuple
/// object. Mirrors `marque::render::RuleIdJson` for byte-identical
/// NDJSON parity. The two crates carry parallel type definitions; a
/// shared `marque-audit-render` crate is a future consolidation.
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
/// Wire shape: `{ "template": "..." }` only. `template` is the
/// [`MessageTemplate::as_str`] canonical label.
///
/// `args` is intentionally NOT serialized — the closed `MessageArgs`
/// payload (typed `TokenId` / `CategoryId` / `Span` / `Blake3Hash` /
/// `Recognition` / `FeatureId` / `RuleId`) requires a per-template
/// arg-flattening serializer that downstream consumers don't yet need.
/// Add the `args` field when audit renderers demand the structured
/// field set.
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
    // Mirrors `marque::render::diagnostic_to_json` for byte-identical
    // NDJSON parity. Defensive `from_utf8` guard (the engine
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
// `marque-3.1` audit-record JSON projection
//
// The audit-record projection is owned by the engine
// (`crates/engine/src/audit_render.rs`) and re-exported as
// `marque_engine::audit_line_to_json_v1_0` / `audit_line_to_ndjson`. WASM,
// the CLI, and the server all route through that single copy, so there is
// no WASM-private struct definition to drift from the CLI. The parity tests
// (`crates/wasm/tests/audit_v3_0_parity.rs`,
// `crates/wasm/tests/native_parity.rs`) still guard the CLI-vs-WASM
// byte-identity at the integration boundary. What
// stays WASM-local is the `RawValue` wrapper below, which `fix()` embeds
// verbatim into its response.
// ---------------------------------------------------------------------------

/// Serialize a single `AuditLine` to the `marque-3.1` NDJSON wire form,
/// wrapped as a `RawValue` so `fix()` can embed it verbatim. Delegates to
/// the engine's canonical projection; the byte-identity unit is the
/// per-line JSON object (no trailing newline).
pub(crate) fn serialize_audit_line_v1_0(
    scheme: &CapcoScheme,
    line: &marque_rules::audit::AuditLine<CapcoScheme>,
) -> Result<Box<serde_json::value::RawValue>, String> {
    // Single accepted schema (`marque-3.2`) so dispatch is a no-op
    // today; the const lookup is kept so a future schema bump can
    // land via the same dispatch shape without restructuring callers.
    let _ = marque_engine::AUDIT_SCHEMA_IS_V3_2;
    let json = marque_engine::audit_line_to_ndjson(scheme, line);
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
    /// stream.
    pub(crate) r002_fired: bool,
    /// Session-level audit metadata record (issue #399): engine /
    /// lattice / decoder versions, integrity seal, interface (`"W"`),
    /// resolved classifier identity, and optional carry-only signature.
    /// `None` when the fix produced no audit records (preserving the
    /// "no fixes → no audit output" contract). When present it is the
    /// FIRST line folded into `session_root`, so the seal and identity
    /// are tamper-evident under the root. Byte-identical to the CLI /
    /// server `session_metadata` record.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) session_metadata: Option<Box<serde_json::value::RawValue>>,
    /// Session-end BLAKE3 Merkle root over `session_metadata` (when
    /// present) followed by the `applied` audit records (issue #184 /
    /// #399), rendered `blake3:<hex>`. A caller can recompute the root
    /// over those bytes in order and compare. Reproducible under a
    /// fixed clock; computed per `fix()` call (per document).
    pub(crate) session_root: String,
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
/// diagnostics + counts.
#[derive(Serialize)]
pub(crate) struct DeadlineExceededBodyJson<'a> {
    pub(crate) truncated_by: &'static str,
    pub(crate) error_count: usize,
    pub(crate) warn_count: usize,
    pub(crate) fix_count: usize,
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
        error_count: partial_lint.error_count(),
        warn_count: partial_lint.warn_count(),
        fix_count: partial_lint.fix_count(),
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
            serde_json::to_string(&fallback).unwrap_or_else(|_| match truncated_by {
                "lint" => {
                    r#"{"truncated_by":"lint","error":"deadline-exceeded payload serialization failed"}"#
                        .to_owned()
                }
                _ => {
                    r#"{"truncated_by":"fix","error":"deadline-exceeded payload serialization failed"}"#
                        .to_owned()
                }
            })
        }
    }
}
