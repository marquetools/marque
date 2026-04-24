// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T049 / T050 — HTTP corpus-override rejection tests.
//!
//! Enforces whitepaper §10.2 + Constitution III + FR-013 + the Phase-D
//! threat model T3 (`docs/plans/2026-04-19-recursive-lattice-and-decoder.md`
//! §6a) via the contract at
//! `specs/004-constraints-decoder-vocab/contracts/cli-server-wasm-gates.md`
//! Gate 1:
//!
//! > HTTP callers MUST NOT be able to supply a corpus override. Any
//! > such field is rejected with `400`. Rejection is audit-logged but
//! > does not expose the attempted override contents to downstream
//! > logs.
//!
//! Three channels are tested for each of `/v1/lint` and `/v1/fix`:
//!
//! 1. JSON body field `corpus_override` (T049)
//! 2. Header `X-Marque-Corpus-Override` (T050)
//! 3. Query string parameter `corpus_override=...`
//!
//! A baseline "no override present" test guarantees the rejection path
//! is distinguishable from generic 400s — if the refactor accidentally
//! made every request 400, the baseline would fail before the override
//! tests passed spuriously.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::Engine;
use marque_server::{AppState, build_app};
use std::sync::Arc;
use tower::ServiceExt;

fn app() -> axum::Router {
    let engine = Engine::new(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles");
    build_app(AppState {
        engine: Arc::new(engine),
    })
}

fn post_json(uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .expect("well-formed request")
}

fn post_json_with_header(uri: &str, body: &str, header: &str, value: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .header(header, value)
        .body(Body::from(body.to_owned()))
        .expect("well-formed request")
}

// ---------------------------------------------------------------------------
// Baselines — a valid request with no override channel returns 200.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn baseline_lint_without_override_is_ok() {
    let resp = app()
        .oneshot(post_json("/v1/lint", r#"{"text": "SECRET//NF\n"}"#))
        .await
        .unwrap();
    // This baseline sends valid JSON with the correct content type, so it
    // should exercise the happy path and return 200. Requiring 200 ensures
    // regressions in request deserialization or validation do not slip
    // through while still satisfying a weaker non-400 assertion.
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "baseline lint should return 200"
    );
}

#[tokio::test]
async fn baseline_fix_without_override_is_ok() {
    let resp = app()
        .oneshot(post_json("/v1/fix", r#"{"text": "SECRET//NF\n"}"#))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "baseline fix should return 200"
    );
}

// ---------------------------------------------------------------------------
// T049 — body-field rejection.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rejects_corpus_override_body_on_lint() {
    let body = r#"{"text": "SECRET//NF\n", "corpus_override": {"priors": {"foo": 1}}}"#;
    let resp = app().oneshot(post_json("/v1/lint", body)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rejects_corpus_override_body_on_fix() {
    let body = r#"{"text": "SECRET//NF\n", "corpus_override": {"priors": {"foo": 1}}}"#;
    let resp = app().oneshot(post_json("/v1/fix", body)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

/// The body-field guard keys on **key presence**, not value shape.
/// Per the contract ("Any such field is rejected with 400"), an
/// explicit `"corpus_override": null` still names the claim and must
/// be rejected. `PresenceMarker` records the key regardless of value.
#[tokio::test]
async fn rejects_null_corpus_override_body_field() {
    let body = r#"{"text": "SECRET//NF\n", "corpus_override": null}"#;
    let resp = app().oneshot(post_json("/v1/fix", body)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

/// Same invariant for other "empty-looking" value shapes — `{}`, `[]`,
/// `""`, `0`. Key presence alone is the claim; the value is never
/// examined.
#[tokio::test]
async fn rejects_empty_value_shapes_in_corpus_override_body() {
    for value in ["null", "{}", "[]", r#""""#, "0", "false"] {
        let body = format!(r#"{{"text": "SECRET//NF\n", "corpus_override": {value}}}"#);
        let resp = app().oneshot(post_json("/v1/fix", &body)).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "corpus_override value {value:?} should trip the T3 body guard"
        );
    }
}

// ---------------------------------------------------------------------------
// T050 — header rejection.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rejects_corpus_override_header_on_lint() {
    let resp = app()
        .oneshot(post_json_with_header(
            "/v1/lint",
            r#"{"text": "SECRET//NF\n"}"#,
            "X-Marque-Corpus-Override",
            "anything",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rejects_corpus_override_header_on_fix() {
    let resp = app()
        .oneshot(post_json_with_header(
            "/v1/fix",
            r#"{"text": "SECRET//NF\n"}"#,
            "X-Marque-Corpus-Override",
            "anything",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rejects_corpus_override_header_case_insensitively() {
    // HTTP headers are case-insensitive; verify the guard matches
    // regardless of how the attacker cases the name.
    for name in [
        "x-marque-corpus-override",
        "X-MARQUE-CORPUS-OVERRIDE",
        "X-Marque-Corpus-Override",
    ] {
        let resp = app()
            .oneshot(post_json_with_header(
                "/v1/fix",
                r#"{"text": "SECRET//NF\n"}"#,
                name,
                "anything",
            ))
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "header name {name} should trip the T3 guard"
        );
    }
}

// ---------------------------------------------------------------------------
// Query-string rejection (covers the third channel listed by T066).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rejects_corpus_override_query_on_lint() {
    let resp = app()
        .oneshot(post_json(
            "/v1/lint?corpus_override=1",
            r#"{"text": "SECRET//NF\n"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rejects_corpus_override_query_on_fix() {
    let resp = app()
        .oneshot(post_json(
            "/v1/fix?corpus_override=file.toml",
            r#"{"text": "SECRET//NF\n"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rejects_corpus_override_query_with_hyphen() {
    let resp = app()
        .oneshot(post_json(
            "/v1/fix?corpus-override=1",
            r#"{"text": "SECRET//NF\n"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rejects_corpus_override_query_percent_encoded() {
    // `%5F` decodes to `_` → name becomes `corpus_override`.
    let resp = app()
        .oneshot(post_json(
            "/v1/fix?corpus%5Foverride=1",
            r#"{"text": "SECRET//NF\n"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rejects_corpus_override_query_with_other_params() {
    let resp = app()
        .oneshot(post_json(
            "/v1/fix?foo=bar&corpus_override=1&baz=qux",
            r#"{"text": "SECRET//NF\n"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

/// Legitimate query strings that happen to contain the override name
/// only as a VALUE must NOT trip the guard. The guard keys on param
/// name, not value.
#[tokio::test]
async fn value_containing_override_name_is_not_rejected() {
    // The value of `context` contains the literal string "corpus_override",
    // but the param name is `context`. Must not 400.
    let resp = app()
        .oneshot(post_json(
            "/v1/lint?context=my_corpus_override_context",
            r#"{"text": "SECRET//NF\n"}"#,
        ))
        .await
        .unwrap();
    assert_ne!(resp.status(), StatusCode::BAD_REQUEST);
}
