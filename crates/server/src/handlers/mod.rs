// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

mod fix;
mod health;
mod lint;

pub use fix::fix_handler;
pub use health::{health, schema_version};
pub use lint::lint_handler;

use crate::{
    MIN_DEADLINE_MS,
    types::{DiagnosticJson, FixJson},
};
use axum::http::{HeaderMap, StatusCode, Uri};
use std::time::{Duration, Instant};

pub(super) const DEFAULT_ENDPOINT_DEADLINE_MS: u64 = 30_000;
const CORPUS_OVERRIDE_HEADER: &str = "x-marque-corpus-override";
pub(super) const DEADLINE_HEADER: &str = "x-marque-deadline";
/// Wire-level signal that the lint pass aborted because the deadline
/// expired. Pairs with the `truncated` body field; older clients that
/// only inspect headers can still detect the partial response.
pub(super) const TRUNCATED_HEADER: &str = "marque-truncated";

/// Detect a corpus-override param in the query string by decoded name.
///
/// Exact-matches param *names* (case-insensitive) after
/// percent-decoding so `?corpus_override=1`, `?a=b&corpus_override&c=d`,
/// `?corpus-override=file:...`, and percent-encoded variants like
/// `?corpus%5Foverride=1` (where `%5F` → `_`) are all caught. Values
/// are never examined.
///
/// Kept `pub(crate)` so crate-root unit tests can exercise this helper
/// directly without duplicating the parser logic.
pub(crate) fn query_carries_corpus_override(query: &str) -> bool {
    form_urlencoded::parse(query.as_bytes()).any(|(name, _value)| {
        name.eq_ignore_ascii_case("corpus_override") || name.eq_ignore_ascii_case("corpus-override")
    })
}

/// Reject a request if its **header** or **query** carries a T3
/// corpus-override claim.
///
/// This function deliberately does NOT inspect the body — at this
/// stage the body has not yet been deserialized, and the wire-level
/// check must run before deserialization so a malformed body still
/// fails with `400 Bad Request` (rather than axum's default `422
/// Unprocessable Entity` from the JSON extractor) when either of
/// these channels carries an override claim.
///
/// The body channel is handled by [`reject_if_body_carries_corpus_override`]
/// after `serde_json::from_slice` succeeds. The two-pass split is
/// intentional: callers always pair both functions, and each one
/// owns exactly one stage of the request lifecycle.
///
/// Visibility is `pub(crate)`: the documented pairing is enforceable
/// only inside this crate, and the only call sites are the lint and
/// fix handlers.
///
/// Returns `Err(StatusCode::BAD_REQUEST)` on any positive signal.
/// Logs the channel but never the payload contents (Constitution V
/// G13: audit-stream content-ignorance).
pub(super) fn reject_if_corpus_override(
    endpoint: &str,
    uri: &Uri,
    headers: &HeaderMap,
) -> Result<(), StatusCode> {
    if headers.contains_key(CORPUS_OVERRIDE_HEADER) {
        tracing::warn!(
            target: "marque_server::t3",
            endpoint,
            channel = "header",
            "rejected X-Marque-Corpus-Override header"
        );
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(q) = uri.query()
        && query_carries_corpus_override(q)
    {
        tracing::warn!(
            target: "marque_server::t3",
            endpoint,
            channel = "query",
            "rejected corpus_override in query string"
        );
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

/// Reject a deserialized request if its body carries a T3
/// corpus-override claim. Pairs with [`reject_if_corpus_override`]
/// (which handles header + query before deserialization).
///
/// Runs after `serde_json::from_slice` so request DTOs can record
/// whether the `corpus_override` JSON key was present without
/// deserializing or examining the payload.
pub(super) fn reject_if_body_carries_corpus_override(
    endpoint: &str,
    has_corpus_override: bool,
) -> Result<(), StatusCode> {
    if has_corpus_override {
        tracing::warn!(
            target: "marque_server::t3",
            endpoint,
            channel = "body",
            "rejected corpus_override in request body"
        );
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

/// Resolve the per-request deadline.
///
/// Returns `Ok(Duration)` with the effective deadline budget. If the
/// header is absent — or present but empty — this is the per-endpoint
/// default; otherwise it's the caller-supplied value (validated and
/// in range).
///
/// Validation rules per spec 005 §10.2:
///
/// - Header absent → `Ok(default)` using the per-endpoint default.
/// - Header present but empty (or whitespace-only) → same as absent.
/// - Header present and parseable as `u64` milliseconds, in the
///   range `[MIN_DEADLINE_MS, deadline_cap]` → `Ok(parsed_duration)`.
/// - Anything else (non-UTF-8, non-numeric, negative, overflow,
///   below floor, above cap) → `Err(BAD_REQUEST)`.
pub(super) fn resolve_request_deadline(
    headers: &HeaderMap,
    deadline_cap: Duration,
    default_ms: u64,
) -> Result<Duration, StatusCode> {
    // HTTP allows duplicate headers; intermediaries (proxies, CDNs,
    // service meshes) may merge or reorder them in ways that would
    // change which value `headers.get()` returns. For a safety
    // control like `X-Marque-Deadline`, the only safe answer is to
    // refuse the ambiguity: a single value is honored, two or more
    // is `400 Bad Request`.
    let mut iter = headers.get_all(DEADLINE_HEADER).iter();
    let raw = match iter.next() {
        Some(value) => value,
        None => return Ok(Duration::from_millis(default_ms)),
    };
    if iter.next().is_some() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let s = raw.to_str().map_err(|_| StatusCode::BAD_REQUEST)?;
    if s.trim().is_empty() {
        return Ok(Duration::from_millis(default_ms));
    }
    let ms: u64 = s.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    if ms < MIN_DEADLINE_MS {
        return Err(StatusCode::BAD_REQUEST);
    }
    let cap_ms = deadline_cap.as_millis().min(u64::MAX as u128) as u64;
    if ms > cap_ms {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(Duration::from_millis(ms))
}

/// Stamp `Instant::now() + duration`, mapping platform-clock overflow
/// to `500 Internal Server Error`.
///
/// Client values are range-checked against the server cap before this
/// point; overflow here indicates server-side misconfiguration rather
/// than malformed client input, so this maps to 500 rather than 400.
pub(super) fn stamp_request_deadline(duration: Duration) -> Result<Instant, StatusCode> {
    Instant::now()
        .checked_add(duration)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)
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

pub(super) fn diagnostics_to_json(result: &marque_engine::LintResult) -> Vec<DiagnosticJson> {
    result
        .diagnostics
        .iter()
        .map(|d| DiagnosticJson {
            rule_id: d.rule.to_string(),
            severity: d.severity.to_string(),
            // PR 3c.2.C C5: `Message` has no Display impl by design.
            // Render the closed-template label; consumers expand args
            // from the structured form via the public Message API.
            message: d.message.template().as_str().to_owned(),
            start: d.span.start,
            end: d.span.end,
            fix: match (d.fix.as_ref(), d.text_correction.as_ref()) {
                (Some(f), _) => Some(FixJson {
                    source: fix_source_str(f.source),
                    intent_kind: match &f.replacement {
                        marque_scheme::ReplacementIntent::FactAdd { .. } => "FactAdd",
                        marque_scheme::ReplacementIntent::FactRemove { .. } => "FactRemove",
                        marque_scheme::ReplacementIntent::Recanonicalize { .. } => "Recanonicalize",
                        _ => "Unknown",
                    },
                    // Structural rule fix — replacement bytes are
                    // engine-rendered at promotion time. The server
                    // response carries only the structural commitment;
                    // callers needing materialized bytes call the fix
                    // endpoint and read the corrected text.
                    replacement: None,
                    confidence: f.confidence.combined(),
                    migration_ref: f.migration_ref.map(str::to_owned),
                }),
                (None, Some(tc)) => Some(FixJson {
                    source: fix_source_str(tc.source),
                    intent_kind: "TextCorrection",
                    replacement: Some(tc.replacement.to_string()),
                    confidence: tc.confidence.combined(),
                    migration_ref: tc.migration_ref.map(str::to_owned),
                }),
                (None, None) => None,
            },
        })
        .collect()
}
