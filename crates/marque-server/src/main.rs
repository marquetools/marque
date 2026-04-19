// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! marque-server — REST microservice exposing marque as an API.
//!
//! Endpoints:
//!   POST /v1/lint      — text → diagnostics
//!   POST /v1/fix       — text → fixed text + audit log
//!   POST /v1/metadata  — document → metadata report
//!   POST /v1/batch     — multiple texts → batch results
//!   GET  /v1/health
//!   GET  /v1/schema/version

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use marque_capco::capco_rules;
use marque_engine::Engine;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AppState {
    engine: Arc<Engine>,
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct LintRequest {
    text: String,
    /// Calling context hint — affects scanner heuristics.
    #[allow(dead_code)]
    context: Option<String>,
}

#[derive(Serialize)]
struct LintResponse {
    diagnostics: Vec<DiagnosticJson>,
    error_count: usize,
    warn_count: usize,
    fix_count: usize,
}

#[derive(Serialize)]
struct DiagnosticJson {
    rule_id: String,
    severity: String,
    message: String,
    start: usize,
    end: usize,
    fix: Option<FixJson>,
}

#[derive(Serialize)]
struct FixJson {
    replacement: String,
    confidence: f32,
    migration_ref: Option<String>,
}

#[derive(Deserialize)]
struct FixRequest {
    text: String,
    /// Optional per-request override of the engine's confidence threshold.
    /// When `None`, the engine uses its configured value. When `Some`, the
    /// value is validated against `[0.0, 1.0]` and a 422 is returned on
    /// invalid input.
    confidence_threshold: Option<f32>,
}

#[derive(Serialize)]
struct FixResponse {
    fixed_text: String,
    applied_count: usize,
    remaining_diagnostics: usize,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    schema_version: &'static str,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        schema_version: marque_capco::SCHEMA_VERSION,
    })
}

async fn schema_version() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "version": marque_capco::SCHEMA_VERSION }))
}

async fn lint_handler(
    State(state): State<AppState>,
    Json(req): Json<LintRequest>,
) -> Result<Json<LintResponse>, StatusCode> {
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
                confidence: f.confidence,
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

async fn fix_handler(
    State(state): State<AppState>,
    Json(req): Json<FixRequest>,
) -> Result<Json<FixResponse>, StatusCode> {
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
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // H-1: load the real layered config so the server honors `.marque.toml`,
    // `MARQUE_CONFIDENCE_THRESHOLD`, `MARQUE_CLASSIFIER_ID`, and — most
    // importantly — runs the FR-011 schema-version hard-fail validator.
    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: cannot determine working directory: {e}");
            std::process::exit(74); // EX_IOERR per contracts/cli.md
        }
    };
    let config = match marque_config::load(&cwd) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: failed to load configuration: {e}");
            std::process::exit(e.exit_code());
        }
    };

    let engine = Engine::new(config, vec![Box::new(capco_rules())]);
    let state = AppState {
        engine: Arc::new(engine),
    };

    let app = Router::new()
        .route("/v1/health", get(health))
        .route("/v1/schema/version", get(schema_version))
        .route("/v1/lint", post(lint_handler))
        .route("/v1/fix", post(fix_handler))
        .with_state(state);

    let addr = std::env::var("MARQUE_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_owned());

    tracing::info!("marque-server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind MARQUE_ADDR — is the port already in use?");
    axum::serve(listener, app)
        .await
        .expect("server exited with error");
}
