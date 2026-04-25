// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! marque-server library — shared Router, handlers, and state.
//!
//! The `marque-server` binary (`src/main.rs`) is a thin wrapper
//! around this library. All REST-surface logic lives here so
//! integration tests can exercise handlers via
//! `axum::Router::oneshot(...)` without spinning up a real TCP
//! listener.
//!
//! ## T3 enforcement (corpus-override gate)
//!
//! Per Constitution III + FR-013 + the Phase-D threat model
//! (`docs/plans/2026-04-19-recursive-lattice-and-decoder.md` §6a) +
//! the contract at
//! `specs/004-constraints-decoder-vocab/contracts/cli-server-wasm-gates.md`,
//! HTTP callers may not supply runtime corpus overrides. Three channels
//! are guarded:
//!
//! - **Request body:** `LintRequest` / `FixRequest` carry a
//!   `PresenceMarker` field renamed to `corpus_override`. The marker
//!   records whether the key was present regardless of the value —
//!   `null`, `{}`, arrays, and arbitrary JSON all count as presence.
//!   Key presence → 400 Bad Request without examining contents.
//! - **Request header:** `X-Marque-Corpus-Override` (case-insensitive).
//!   Presence → 400.
//! - **Query string:** any param named `corpus_override` or
//!   `corpus-override` (case-insensitive, after percent-decoding).
//!   Presence → 400.
//!
//! In all three cases the rejection emits a `tracing::warn!` entry
//! naming the channel and the endpoint path. The attempted override
//! contents are never materialized, stored, or logged.

use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode, Uri},
    response::Json,
    routing::{get, post},
};
use marque_engine::Engine;
use serde::{Deserialize, Deserializer, Serialize};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Body-size cap (whitepaper §10.2 / gap register #6)
// ---------------------------------------------------------------------------

/// Default request-body cap, in bytes.
///
/// 10 MiB. Five orders of magnitude above the largest classified marking
/// any reasonable document would carry; an order of magnitude above the
/// p99 of typical document sizes the corpus has seen. Set explicitly so
/// the operator's choice is recorded in the source rather than inherited
/// from axum's 2 MB default — gap register #6 was the absence of an
/// intentional decision.
///
/// Override at runtime by setting `MARQUE_MAX_BODY_BYTES`. Values below
/// 1 KB are rejected at startup (`resolve_body_limit`) — anything that
/// small would make every realistic request 413.
pub const DEFAULT_BODY_LIMIT_BYTES: usize = 10 * 1024 * 1024;

/// Resolve the body-size cap from `MARQUE_MAX_BODY_BYTES` or fall back
/// to [`DEFAULT_BODY_LIMIT_BYTES`].
///
/// Returns an error string suitable for stderr if the env var is set
/// but unparseable / unreasonable. Caller decides whether to abort
/// (the binary entry point does) or log-and-default (an embedder
/// might).
pub fn resolve_body_limit() -> Result<usize, String> {
    match std::env::var("MARQUE_MAX_BODY_BYTES") {
        Err(_) => Ok(DEFAULT_BODY_LIMIT_BYTES),
        Ok(s) => {
            let parsed: usize = s
                .parse()
                .map_err(|_| format!("MARQUE_MAX_BODY_BYTES is not a valid byte count: {s:?}"))?;
            if parsed < 1024 {
                return Err(format!(
                    "MARQUE_MAX_BODY_BYTES={parsed} is below the 1024-byte floor; \
                     anything smaller would make every realistic request 413"
                ));
            }
            Ok(parsed)
        }
    }
}

// ---------------------------------------------------------------------------
// Presence marker — key-present-regardless-of-value detector
// ---------------------------------------------------------------------------

/// Records whether a JSON key was present, without materializing or
/// examining the value.
///
/// Used by the T3 body-field guard. `#[serde(default)]` on the field
/// means an absent key deserializes as `PresenceMarker(false)`; any
/// present key — including `null`, `{}`, `[]`, numbers, strings —
/// runs the `Deserialize` impl, which consumes the value via
/// `IgnoredAny` (never stored, never logged) and returns
/// `PresenceMarker(true)`.
///
/// This matches the contract wording "Any such field is rejected
/// with 400" more precisely than `Option<IgnoredAny>` would, because
/// the latter cannot distinguish an absent key from an explicit
/// `null` value.
#[derive(Default)]
struct PresenceMarker(bool);

impl PresenceMarker {
    fn is_present(&self) -> bool {
        self.0
    }
}

impl<'de> Deserialize<'de> for PresenceMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Consume the value without storing it. `IgnoredAny` accepts
        // any JSON shape — including `null`, objects, arrays — so
        // presence of the key alone is the observable signal.
        serde::de::IgnoredAny::deserialize(deserializer)?;
        Ok(PresenceMarker(true))
    }
}

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<Engine>,
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct LintRequest {
    pub text: String,
    /// Calling context hint — affects scanner heuristics.
    #[allow(dead_code)]
    pub context: Option<String>,
    /// T3 guard: if the key is present (regardless of value), the
    /// handler rejects with 400. `PresenceMarker` records key presence
    /// without deserializing or storing the payload, so even
    /// `"corpus_override": null` still trips the guard — matching the
    /// contract's "any such field is rejected" wording.
    #[serde(default, rename = "corpus_override")]
    _corpus_override: PresenceMarker,
}

#[derive(Serialize)]
pub struct LintResponse {
    pub diagnostics: Vec<DiagnosticJson>,
    pub error_count: usize,
    pub warn_count: usize,
    pub fix_count: usize,
}

#[derive(Serialize)]
pub struct DiagnosticJson {
    pub rule_id: String,
    pub severity: String,
    pub message: String,
    pub start: usize,
    pub end: usize,
    pub fix: Option<FixJson>,
}

#[derive(Serialize)]
pub struct FixJson {
    pub replacement: String,
    pub confidence: f32,
    pub migration_ref: Option<String>,
}

#[derive(Deserialize)]
pub struct FixRequest {
    pub text: String,
    /// Optional per-request override of the engine's confidence threshold.
    /// When `None`, the engine uses its configured value. When `Some`, the
    /// value is validated against `[0.0, 1.0]` and a 422 is returned on
    /// invalid input.
    pub confidence_threshold: Option<f32>,
    /// T3 guard: see `LintRequest::_corpus_override`.
    #[serde(default, rename = "corpus_override")]
    _corpus_override: PresenceMarker,
}

#[derive(Serialize)]
pub struct FixResponse {
    pub fixed_text: String,
    pub applied_count: usize,
    pub remaining_diagnostics: usize,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub schema_version: &'static str,
}

// ---------------------------------------------------------------------------
// T3 corpus-override rejection (Gate 1)
// ---------------------------------------------------------------------------

const CORPUS_OVERRIDE_HEADER: &str = "x-marque-corpus-override";

/// Detect a corpus-override param in the query string by decoded name.
///
/// Exact-matches param *names* (case-insensitive) after
/// percent-decoding so `?corpus_override=1`, `?a=b&corpus_override&c=d`,
/// `?corpus-override=file:...`, and percent-encoded variants like
/// `?corpus%5Foverride=1` (where `%5F` → `_`) are all caught. Values
/// are never examined.
fn query_carries_corpus_override(query: &str) -> bool {
    form_urlencoded::parse(query.as_bytes()).any(|(name, _value)| {
        name.eq_ignore_ascii_case("corpus_override") || name.eq_ignore_ascii_case("corpus-override")
    })
}

/// Inspect a request for any T3 corpus-override claim.
///
/// Returns `Err(StatusCode::BAD_REQUEST)` on any positive signal. Logs
/// the channel but never the payload contents.
pub fn reject_if_corpus_override(
    endpoint: &str,
    uri: &Uri,
    headers: &HeaderMap,
    body_has_override: bool,
) -> Result<(), StatusCode> {
    if body_has_override {
        tracing::warn!(
            target: "marque_server::t3",
            endpoint,
            channel = "body",
            "rejected corpus_override in request body"
        );
        return Err(StatusCode::BAD_REQUEST);
    }
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

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        schema_version: marque_capco::SCHEMA_VERSION,
    })
}

pub async fn schema_version() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "version": marque_capco::SCHEMA_VERSION }))
}

pub async fn lint_handler(
    State(state): State<AppState>,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<LintResponse>, StatusCode> {
    // Wire-level checks (header + query) run BEFORE body deserialization so
    // a request with a malformed body is still rejected with 400 when either
    // of those channels carries an override claim (axum's Json extractor
    // would otherwise short-circuit with 422 before this handler ran).
    reject_if_corpus_override("/v1/lint", &uri, &headers, false)?;

    let req: LintRequest =
        serde_json::from_slice(&body).map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;

    // Body-field check after successful deserialization.
    if req._corpus_override.is_present() {
        tracing::warn!(
            target: "marque_server::t3",
            endpoint = "/v1/lint",
            channel = "body",
            "rejected corpus_override in request body"
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    let result = state.engine.lint(req.text.as_bytes());

    let diagnostics = result
        .diagnostics
        .iter()
        .map(|d| DiagnosticJson {
            rule_id: d.rule.to_string(),
            severity: d.severity.to_string(),
            message: d.message.to_string(),
            start: d.span.start,
            end: d.span.end,
            fix: d.fix.as_ref().map(|f| FixJson {
                replacement: f.replacement.to_string(),
                confidence: f.confidence.combined(),
                migration_ref: f.migration_ref.map(str::to_owned),
            }),
        })
        .collect();

    Ok(Json(LintResponse {
        error_count: result.error_count(),
        warn_count: result.warn_count(),
        fix_count: result.fix_count(),
        diagnostics,
    }))
}

pub async fn fix_handler(
    State(state): State<AppState>,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<FixResponse>, StatusCode> {
    // Wire-level checks (header + query) run BEFORE body deserialization.
    reject_if_corpus_override("/v1/fix", &uri, &headers, false)?;

    let req: FixRequest =
        serde_json::from_slice(&body).map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;

    // Body-field check after successful deserialization.
    if req._corpus_override.is_present() {
        tracing::warn!(
            target: "marque_server::t3",
            endpoint = "/v1/fix",
            channel = "body",
            "rejected corpus_override in request body"
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    let result = state
        .engine
        .fix_with_threshold(
            req.text.as_bytes(),
            marque_engine::FixMode::Apply,
            req.confidence_threshold,
        )
        .map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;
    let fixed = String::from_utf8(result.source).map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;

    Ok(Json(FixResponse {
        fixed_text: fixed,
        applied_count: result.applied.len(),
        remaining_diagnostics: result.remaining_diagnostics.len(),
    }))
}

// ---------------------------------------------------------------------------
// Router assembly
// ---------------------------------------------------------------------------

/// Build the axum `Router` wiring every endpoint to its handler with
/// the default body-size cap.
///
/// Factored out of `main()` so integration tests can exercise handlers
/// in-process via `tower::ServiceExt::oneshot` without binding a
/// listener. The cap defaults to [`DEFAULT_BODY_LIMIT_BYTES`]; tests
/// that need to exercise a different limit should call
/// [`build_app_with_limit`] directly.
pub fn build_app(state: AppState) -> Router {
    build_app_with_limit(state, DEFAULT_BODY_LIMIT_BYTES)
}

/// Same as [`build_app`] but with an explicit body-size cap in bytes.
///
/// `body_limit_bytes` is applied as an axum `DefaultBodyLimit` Tower
/// layer; oversize requests reach the handler as a `413 Payload Too
/// Large`. The limit applies to every route on the returned router,
/// including the GET endpoints (which carry no body in practice; the
/// cap is harmless there).
pub fn build_app_with_limit(state: AppState, body_limit_bytes: usize) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/schema/version", get(schema_version))
        .route("/v1/lint", post(lint_handler))
        .route("/v1/fix", post(fix_handler))
        .layer(DefaultBodyLimit::max(body_limit_bytes))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_carries_corpus_override_basic() {
        assert!(query_carries_corpus_override("corpus_override=1"));
        assert!(query_carries_corpus_override("a=b&corpus_override=x"));
        assert!(query_carries_corpus_override("corpus-override=file"));
        assert!(query_carries_corpus_override("CORPUS_OVERRIDE=1"));
        // Bare param (no `=value`).
        assert!(query_carries_corpus_override("corpus_override"));
    }

    #[test]
    fn query_carries_corpus_override_percent_encoded() {
        // `%5F` decodes to `_`, `%2D` decodes to `-`. A param name
        // using either encoding must still match after decoding.
        assert!(query_carries_corpus_override("corpus%5Foverride=1"));
        assert!(query_carries_corpus_override("corpus%5foverride=1"));
        assert!(query_carries_corpus_override("corpus%2Doverride=1"));
        assert!(query_carries_corpus_override("a=b&corpus%5Foverride&c=d"));
    }

    #[test]
    fn query_carries_corpus_override_negatives() {
        assert!(!query_carries_corpus_override(""));
        assert!(!query_carries_corpus_override("text=hi"));
        // Substring-only matches on param VALUE must NOT trigger —
        // we match on decoded param NAME to avoid false positives
        // where a legitimate field value contains the literal string.
        assert!(!query_carries_corpus_override("text=corpus_override"));
        assert!(!query_carries_corpus_override(
            "text=my_corpus_override_is_cool"
        ));
        // A percent-encoded form of the name appearing only as a VALUE
        // must also not trigger — only decoded names are checked.
        assert!(!query_carries_corpus_override("text=corpus%5Foverride"));
    }
}
