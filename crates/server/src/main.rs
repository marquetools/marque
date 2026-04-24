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
use marque_server::{AppState, build_app};
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
    let state = AppState {
        engine: Arc::new(engine),
    };

    let app = build_app(state);

    // Security Convention: Default to binding to the local loopback interface
    // (127.0.0.1) instead of all interfaces (0.0.0.0) to prevent unintentional external exposure.
    let addr = std::env::var("MARQUE_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_owned());

    tracing::info!("marque-server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind MARQUE_ADDR — is the port already in use?");
    axum::serve(listener, app)
        .await
        .expect("server exited with error");
}
