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
//! - **Request body:** `LintRequest` / `FixRequest` carry a marker field
//!   `_corpus_override: Option<serde::de::IgnoredAny>` renamed to
//!   `corpus_override`. Presence → 400 Bad Request without examining
//!   contents.
//! - **Request header:** `X-Marque-Corpus-Override` (case-insensitive).
//!   Presence → 400.
//! - **Query string:** any param named `corpus_override` or
//!   `corpus-override` (case-insensitive). Presence → 400.
//!
//! In all three cases the rejection emits a `tracing::warn!` entry
//! naming the channel and the endpoint path. The attempted override
//! contents are never examined, deserialized, or logged.

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, Uri},
    response::Json,
    routing::{get, post},
};
use marque_engine::Engine;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    /// T3 guard: if present, the handler rejects with 400. Typed as
    /// `IgnoredAny` so the contents are never deserialized or stored,
    /// only their presence is observable.
    #[serde(default, rename = "corpus_override")]
    _corpus_override: Option<serde::de::IgnoredAny>,
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
    _corpus_override: Option<serde::de::IgnoredAny>,
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

/// Detect a corpus-override param in the query string by name only.
///
/// Substring-matches param *names* so `?corpus_override=1`,
/// `?a=b&corpus_override&c=d`, and `?corpus-override=file:...` are all
/// caught. Values are never examined.
fn query_carries_corpus_override(query: &str) -> bool {
    query.split('&').any(|pair| {
        let name = pair.split('=').next().unwrap_or("");
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
    Json(req): Json<LintRequest>,
) -> Result<Json<LintResponse>, StatusCode> {
    reject_if_corpus_override("/v1/lint", &uri, &headers, req._corpus_override.is_some())?;

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
    Json(req): Json<FixRequest>,
) -> Result<Json<FixResponse>, StatusCode> {
    reject_if_corpus_override("/v1/fix", &uri, &headers, req._corpus_override.is_some())?;

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

/// Build the axum `Router` wiring every endpoint to its handler.
///
/// Factored out of `main()` so integration tests can exercise handlers
/// in-process via `tower::ServiceExt::oneshot` without binding a
/// listener.
pub fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/schema/version", get(schema_version))
        .route("/v1/lint", post(lint_handler))
        .route("/v1/fix", post(fix_handler))
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
    fn query_carries_corpus_override_negatives() {
        assert!(!query_carries_corpus_override(""));
        assert!(!query_carries_corpus_override("text=hi"));
        // Substring-only matches on param VALUE should NOT trigger —
        // we match on param NAME to avoid false positives where a
        // legitimate field value contains the literal string.
        assert!(!query_carries_corpus_override("text=corpus_override"));
        assert!(!query_carries_corpus_override(
            "text=my_corpus_override_is_cool"
        ));
    }
}
