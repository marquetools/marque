// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! marque-server — REST microservice exposing marque as an API.
//!
//! Thin wrapper over the `marque_server` library. All routing,
//! request/response types, and handler logic live in `lib.rs` so
//! integration tests can exercise them without a live listener.
//!
//! Endpoints:
//!   POST /v1/lint      — text → diagnostics
//!   POST /v1/fix       — text → fixed text + audit log
//!   POST /v1/metadata  — document → metadata report
//!   POST /v1/batch     — multiple texts → batch results
//!   GET  /v1/health
//!   GET  /v1/schema/version

use marque_capco::capco_rules;
use marque_engine::Engine;
use marque_server::{AppState, build_app_with_limit, resolve_body_limit, resolve_deadline_cap};
use std::sync::Arc;

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

    let engine = match Engine::new(
        config,
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    ) {
        Ok(e) => e,
        Err(err) => {
            eprintln!("error: failed to construct engine: {err}");
            std::process::exit(69);
        }
    };
    // Spec 005 §10.2 — per-request deadline cap (default 60 s,
    // override via `MARQUE_MAX_DEADLINE`). Resolved here so an
    // unparseable / out-of-range value fails startup loudly instead
    // of silently degrading a per-request safety control.
    let deadline_cap = match resolve_deadline_cap() {
        Ok(d) => d,
        Err(msg) => {
            eprintln!("error: {msg}");
            std::process::exit(64); // EX_USAGE per contracts/cli.md
        }
    };

    let state = AppState {
        engine: Arc::new(engine),
        deadline_cap,
    };

    // Whitepaper §10.2 / gap register #6 — explicit body-size cap
    // (default 10 MiB, override via `MARQUE_MAX_BODY_BYTES`). Earlier
    // builds inherited axum's 2 MB default by accident; the operator
    // decision now lives in `lib.rs::DEFAULT_BODY_LIMIT_BYTES` and is
    // recorded on every startup line below.
    let body_limit_bytes = match resolve_body_limit() {
        Ok(n) => n,
        Err(msg) => {
            eprintln!("error: {msg}");
            std::process::exit(64); // EX_USAGE per contracts/cli.md
        }
    };

    let app = build_app_with_limit(state, body_limit_bytes);

    // Security Convention: Default to binding to the local loopback interface
    // (127.0.0.1) instead of all interfaces (0.0.0.0) to prevent unintentional external exposure.
    let addr = std::env::var("MARQUE_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_owned());

    tracing::info!(
        addr = %addr,
        body_limit_bytes,
        deadline_cap_ms = deadline_cap.as_millis() as u64,
        "marque-server listening"
    );

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind MARQUE_ADDR — is the port already in use?");

    // Whitepaper §10.2 — Landlock process sandbox.
    // Applied after bind (so the listening socket is already open) and before
    // the first request is accepted.  Graceful: logs a warning and continues
    // if the kernel does not support Landlock.
    let sandbox_status = marque_server::sandbox::apply(&cwd);
    tracing::info!(sandbox = ?sandbox_status, "process sandbox status");

    axum::serve(listener, app)
        .await
        .expect("server exited with error");
}
