// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Engine-side audit-record NDJSON projection (`marque-3.1`).
//!
//! This is the projection the **server** (`marque-server`'s `/v1/fix`)
//! uses to serialize each [`AuditLine`] into the audit-log wire form and
//! to compute the session-end [`crate::SessionRoot`] over those exact
//! bytes (issue #184). The CLI (`marque/src/render.rs`) and WASM
//! (`crates/wasm/src/types.rs`) carry their own copies of this
//! projection; the byte-identity contract between *those two* is pinned
//! by `crates/wasm/tests/audit_v3_0_parity.rs`. Consolidating all three
//! onto this engine copy is tracked as a follow-up — for now the server
//! reuses one self-consistent serializer (it emits the `audit_log` and
//! computes the root with the same function, so a verifier re-hashing
//! the `audit_log` strings always reproduces the published `session_root`).
//!
//! Per Constitution V Principle V every emitted field is a permitted
//! identifier (closed-enum labels, token canonicals, span offsets,
//! BLAKE3 digests, posterior scalars) — no document content reaches the
//! wire. The `crates/engine/tests/audit_g13_canary.rs` corpus sweep is
//! the empirical guard.

use crate::AUDIT_SCHEMA_VERSION;
use marque_capco::CapcoScheme;
use marque_rules::RuleId;
use marque_rules::audit::{AppliedTextCorrection, AuditLine, discriminant_from_source};
use marque_scheme::{Canonical, CategoryId, TokenSource, Vocabulary};
use serde::Serialize;

/// 2-tuple `RuleId` projection (`{ scheme, predicate_id }`).
#[derive(Debug, Serialize)]
pub struct RuleIdJson<'a> {
    pub scheme: &'a str,
    pub predicate_id: &'a str,
}

impl<'a> From<&'a RuleId> for RuleIdJson<'a> {
    fn from(r: &'a RuleId) -> Self {
        Self {
            scheme: r.scheme(),
            predicate_id: r.predicate_id(),
        }
    }
}

/// Byte-offset span projection.
#[derive(Debug, Serialize)]
pub struct SpanJson {
    pub start: usize,
    pub end: usize,
}

/// `AppliedFix<CapcoScheme>` projection.
#[derive(Debug, Serialize)]
pub struct AuditRecordJsonV1_0<'a> {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub schema: &'static str,
    pub rule: RuleIdJson<'a>,
    pub severity: &'static str,
    pub span: SpanJson,
    pub fix: AuditFixJson<'a>,
    pub message: AuditMessageJson<'a>,
    pub timestamp: String,
    pub classifier_id: Option<&'a str>,
    pub dry_run: bool,
    pub input: Option<&'a str>,
}

/// `AppliedFixDetail<S>` projection (the `fix` sub-object).
#[derive(Debug, Serialize)]
pub struct AuditFixJson<'a> {
    pub replacement: AuditReplacementJson<'a>,
    pub original_span: SpanJson,
    pub original_digest: String,
}

/// `AppliedReplacement<S>` projection (the `fix.replacement` sub-object).
#[derive(Debug, Serialize)]
pub struct AuditReplacementJson<'a> {
    pub discriminant: &'static str,
    pub canonical: AuditCanonicalJson<'a>,
    pub confidence: AuditConfidenceJson<'a>,
}

/// `Canonical<S>` projection.
#[derive(Debug, Serialize)]
pub struct AuditCanonicalJson<'a> {
    pub source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_id: Option<std::borrow::Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render_call_site: Option<String>,
    pub bytes_digest: String,
}

/// `Recognition` projection (single-axis `marque-3.0`/`marque-3.1` shape).
#[derive(Debug, Serialize)]
pub struct AuditConfidenceJson<'a> {
    pub recognition: f32,
    pub runner_up_ratio: Option<f32>,
    pub features: Vec<AuditFeatureJson<'a>>,
}

/// `FeatureContribution` projection (closed-set `FeatureId` labels).
#[derive(Debug, Serialize)]
pub struct AuditFeatureJson<'a> {
    pub id: &'a str,
    pub delta: f32,
}

/// `Message` projection.
#[derive(Debug, Serialize)]
pub struct AuditMessageJson<'a> {
    pub template: &'static str,
    pub args: serde_json::Map<String, serde_json::Value>,
    #[serde(skip)]
    pub _marker: std::marker::PhantomData<&'a ()>,
}

/// `AppliedTextCorrection` projection.
#[derive(Debug, Serialize)]
pub struct TextCorrectionRecordJsonV1_0<'a> {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub schema: &'static str,
    pub rule: RuleIdJson<'a>,
    pub severity: &'static str,
    pub span: SpanJson,
    pub original_digest: String,
    pub replacement: &'a str,
    pub source: &'static str,
    pub confidence: AuditConfidenceJson<'a>,
    pub migration_ref: Option<&'a str>,
    pub message: AuditMessageJson<'a>,
    pub timestamp: String,
    pub classifier_id: Option<&'a str>,
    pub dry_run: bool,
    pub input: Option<&'a str>,
}

fn fix_source_str(source: marque_rules::FixSource) -> &'static str {
    match source {
        marque_rules::FixSource::BuiltinRule => "BuiltinRule",
        marque_rules::FixSource::CorrectionsMap => "CorrectionsMap",
        marque_rules::FixSource::MigrationTable => "MigrationTable",
        marque_rules::FixSource::DecoderPosterior => "DecoderPosterior",
        marque_rules::FixSource::DecoderClassificationHeuristic => "DecoderClassificationHeuristic",
    }
}

/// Wire form: `"blake3:<64-hex>"`.
fn blake3_audit_string(hash: &blake3::Hash) -> String {
    format!("blake3:{}", hash.to_hex())
}

fn format_timestamp(ts: std::time::SystemTime) -> String {
    humantime::format_rfc3339(ts).to_string()
}

fn category_label(scheme: &CapcoScheme, category_id: CategoryId) -> &'static str {
    use marque_scheme::MarkingScheme;
    if category_id == CategoryId::MARKING {
        return "Marking";
    }
    scheme
        .categories()
        .iter()
        .find(|c| c.id == category_id)
        .map(|c| c.name)
        .unwrap_or("unknown")
}

fn project_canonical_to_json<'a>(
    scheme: &'a CapcoScheme,
    canonical: &Canonical<CapcoScheme>,
    precomputed_bytes_digest: &blake3::Hash,
) -> AuditCanonicalJson<'a> {
    let digest = blake3_audit_string(precomputed_bytes_digest);
    match canonical.source() {
        TokenSource::Cve(token_id) => {
            let label =
                <CapcoScheme as Vocabulary<CapcoScheme>>::qualified_token_label(scheme, token_id);
            AuditCanonicalJson {
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
        } => AuditCanonicalJson {
            source: "open_vocab",
            token_id: None,
            category: Some(category_label(scheme, *category)),
            render_call_site: Some(format!(
                "{}:{}",
                render_call_site.file(),
                render_call_site.line(),
            )),
            bytes_digest: digest,
        },
    }
}

fn project_confidence_to_json(confidence: &marque_rules::Recognition) -> AuditConfidenceJson<'_> {
    AuditConfidenceJson {
        recognition: confidence.recognition,
        runner_up_ratio: confidence.runner_up_ratio,
        features: confidence
            .features
            .iter()
            .map(|f| AuditFeatureJson {
                id: f.id.as_str(),
                delta: f.delta,
            })
            .collect(),
    }
}

fn project_message_to_json<'a>(
    scheme: &'a CapcoScheme,
    message: &marque_rules::Message,
) -> AuditMessageJson<'a> {
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
            serde_json::Value::String(category_label(scheme, category_id).to_owned()),
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
            serde_json::Value::String(blake3_audit_string(&digest)),
        );
    }
    if let Some(ref confidence) = m.confidence {
        args.insert(
            "confidence".to_owned(),
            serde_json::to_value(project_confidence_to_json(confidence))
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
                    .map(|r| serde_json::Value::String(r.to_string()))
                    .collect(),
            ),
        );
    }
    AuditMessageJson {
        template: message.template().as_str(),
        args,
        _marker: std::marker::PhantomData,
    }
}

/// Project an `AppliedFix<CapcoScheme>` into the audit JSON shape.
pub fn applied_fix_to_audit_json_v1_0<'a>(
    scheme: &'a CapcoScheme,
    fix: &'a marque_rules::audit::AppliedFix<CapcoScheme>,
) -> AuditRecordJsonV1_0<'a> {
    let replacement = AuditReplacementJson {
        discriminant: discriminant_from_source(fix.source).as_str(),
        canonical: project_canonical_to_json(
            scheme,
            &fix.fix.replacement.canonical,
            &fix.fix.replacement.bytes_digest,
        ),
        confidence: project_confidence_to_json(&fix.fix.replacement.confidence),
    };
    let fix_detail = AuditFixJson {
        replacement,
        original_span: SpanJson {
            start: fix.fix.original_span.start,
            end: fix.fix.original_span.end,
        },
        original_digest: blake3_audit_string(&fix.fix.original_digest),
    };
    AuditRecordJsonV1_0 {
        kind: "applied_fix",
        schema: AUDIT_SCHEMA_VERSION,
        rule: (&fix.rule).into(),
        severity: fix.severity.as_str(),
        span: SpanJson {
            start: fix.span.start,
            end: fix.span.end,
        },
        fix: fix_detail,
        message: project_message_to_json(scheme, &fix.message),
        timestamp: format_timestamp(fix.timestamp),
        classifier_id: fix.classifier_id.as_deref(),
        dry_run: fix.dry_run,
        input: fix.input.as_deref(),
    }
}

/// Project an `AppliedTextCorrection` into the text-correction JSON shape.
pub fn text_correction_to_audit_json_v1_0<'a>(
    scheme: &'a CapcoScheme,
    tc: &'a AppliedTextCorrection,
) -> TextCorrectionRecordJsonV1_0<'a> {
    TextCorrectionRecordJsonV1_0 {
        kind: "text_correction",
        schema: AUDIT_SCHEMA_VERSION,
        rule: (&tc.rule).into(),
        severity: tc.severity.as_str(),
        span: SpanJson {
            start: tc.span.start,
            end: tc.span.end,
        },
        original_digest: blake3_audit_string(&tc.original_digest),
        replacement: tc.replacement.as_str(),
        source: fix_source_str(tc.source),
        confidence: project_confidence_to_json(&tc.confidence),
        migration_ref: tc.migration_ref,
        message: project_message_to_json(scheme, &tc.message),
        timestamp: format_timestamp(tc.timestamp),
        classifier_id: tc.classifier_id.as_deref(),
        dry_run: tc.dry_run,
        input: tc.input.as_deref(),
    }
}

/// Dispatch an [`AuditLine`] to its `serde_json::Value` projection.
pub fn audit_line_to_json_v1_0(
    scheme: &CapcoScheme,
    line: &AuditLine<CapcoScheme>,
) -> serde_json::Value {
    match line {
        AuditLine::AppliedFix(fix) => {
            serde_json::to_value(applied_fix_to_audit_json_v1_0(scheme, fix))
                .unwrap_or(serde_json::Value::Null)
        }
        AuditLine::TextCorrection(tc) => {
            serde_json::to_value(text_correction_to_audit_json_v1_0(scheme, tc))
                .unwrap_or(serde_json::Value::Null)
        }
        // `AuditLine` is `#[non_exhaustive]`. A future variant lands as
        // `Null` until an arm is added here (in lockstep with the CLI +
        // WASM renderers and the g13 canary's projection).
        _ => serde_json::Value::Null,
    }
}

/// Serialize an [`AuditLine`] to a single canonical NDJSON record string
/// (no trailing newline).
///
/// This is the unit the session-end [`crate::SessionRoot`] hashes: the
/// server emits each record via this function AND computes the root over
/// the same strings, so a verifier re-hashing the emitted `audit_log`
/// always reproduces the published root.
pub fn audit_line_to_ndjson(scheme: &CapcoScheme, line: &AuditLine<CapcoScheme>) -> String {
    serde_json::to_string(&audit_line_to_json_v1_0(scheme, line)).unwrap_or_default()
}
