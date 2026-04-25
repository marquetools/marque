// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Spec 005 Phase 3b — server-side deadline tests (T036–T039).
//!
//! Covers the `X-Marque-Deadline` header surface plus the
//! `Marque-Truncated` response header for partial lint responses
//! and the 504 response shape for fix.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use marque_capco::capco_rules;
use marque_config::Config;
use marque_engine::Engine;
use marque_server::{AppState, DEFAULT_DEADLINE_CAP_MS, build_app};
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

fn engine() -> Engine {
    Engine::new(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles")
}

fn app_with_cap(deadline_cap: Duration) -> axum::Router {
    build_app(AppState {
        engine: Arc::new(engine()),
        deadline_cap,
    })
}

fn app() -> axum::Router {
    build_app(AppState::new(Arc::new(engine())))
}

fn post_json(uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .expect("well-formed request")
}

fn post_with_deadline(uri: &str, body: &str, deadline_ms: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .header("X-Marque-Deadline", deadline_ms)
        .body(Body::from(body.to_owned()))
        .expect("well-formed request")
}

/// Stuff `text` with enough banner candidates to reliably trip the
/// per-candidate deadline at 1 ms on any reasonable host. 4 000 is
/// the same shape the engine deadline-overhead bench uses.
fn many_banners_payload(count: usize) -> String {
    let banners = "SECRET//NF\\n\\n\\n".repeat(count);
    format!(r#"{{"text": "{banners}"}}"#)
}

// ---------------------------------------------------------------------------
// T036 — header truncates lint with 200 + Marque-Truncated header.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn header_driven_deadline_truncates_lint_response() {
    let payload = many_banners_payload(4_000);
    let resp = app()
        .oneshot(post_with_deadline("/v1/lint", &payload, "1"))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "truncated lint must be 200 (the body still carries usable diagnostics)"
    );
    let truncated = resp
        .headers()
        .get("Marque-Truncated")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    assert_eq!(
        truncated.as_deref(),
        Some("true"),
        "truncated lint must set Marque-Truncated: true (got: {truncated:?})"
    );

    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(
        body_json["truncated"],
        serde_json::Value::Bool(true),
        "body must carry truncated:true (got: {body_json})"
    );
    assert!(
        body_json["candidates_total"].as_u64().is_some(),
        "body must carry candidates_total (got: {body_json})"
    );
}

// ---------------------------------------------------------------------------
// T037 — out-of-range / unparseable header → 400.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn deadline_header_zero_returns_400() {
    let resp = app()
        .oneshot(post_with_deadline(
            "/v1/lint",
            r#"{"text":"SECRET//NF\n"}"#,
            "0",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn deadline_header_non_numeric_returns_400() {
    for raw in ["30s", "abc", "1.5", " 100", "100ms"] {
        let resp = app()
            .oneshot(post_with_deadline(
                "/v1/lint",
                r#"{"text":"SECRET//NF\n"}"#,
                raw,
            ))
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "header value {raw:?} must be rejected as 400"
        );
    }
}

#[tokio::test]
async fn deadline_header_negative_returns_400() {
    // u64 parsing rejects negatives — surfaces as 400.
    let resp = app()
        .oneshot(post_with_deadline(
            "/v1/lint",
            r#"{"text":"SECRET//NF\n"}"#,
            "-1",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn deadline_header_above_cap_returns_400() {
    // The default `AppState` cap is `DEFAULT_DEADLINE_CAP_MS` (60 000
    // ms). Pin the boundary at `cap + 1` so this test fails the moment
    // the default cap regresses, and so it doesn't accidentally pin
    // the *ceiling* (`MAX_DEADLINE_CAP_MS`, 600 000 ms) instead — those
    // are different invariants. (`deadline_header_just_above_configured_cap_returns_400`
    // covers the explicit-cap-via-AppState path on the same boundary.)
    let too_big = (DEFAULT_DEADLINE_CAP_MS + 1).to_string();
    let resp = app()
        .oneshot(post_with_deadline(
            "/v1/lint",
            r#"{"text":"SECRET//NF\n"}"#,
            &too_big,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn deadline_header_just_above_configured_cap_returns_400() {
    // App configured with a 100 ms cap; 101 ms must reject. This
    // exercises the AppState.deadline_cap path independently of the
    // env-var resolution — that an embedder dialing the cap down via
    // direct AppState construction is honored.
    let cap = Duration::from_millis(100);
    let resp = app_with_cap(cap)
        .oneshot(post_with_deadline(
            "/v1/lint",
            r#"{"text":"SECRET//NF\n"}"#,
            "101",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn deadline_header_overflow_returns_400() {
    // u64::MAX + 1 — string longer than u64::MAX's width, must fail
    // u64::from_str (parse overflow) → 400.
    let overflow = "999999999999999999999999999";
    let resp = app()
        .oneshot(post_with_deadline(
            "/v1/lint",
            r#"{"text":"SECRET//NF\n"}"#,
            overflow,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn deadline_header_empty_uses_endpoint_default() {
    // An empty header value (header present, value `""`) must behave
    // like the header is absent — same convention HTTP libraries use
    // elsewhere, and matches a caller building the header from a
    // possibly-unset env var or template variable. The contract is
    // documented on `resolve_request_deadline`.
    let resp = app()
        .oneshot(post_with_deadline(
            "/v1/lint",
            r#"{"text":"SECRET//NF\n"}"#,
            "",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        resp.headers().get("Marque-Truncated").is_none(),
        "small fixture should not be truncated when empty deadline falls back to default"
    );
}

#[tokio::test]
async fn deadline_header_whitespace_uses_endpoint_default() {
    // Whitespace-only is the same as empty per
    // `resolve_request_deadline`'s `s.trim().is_empty()` branch — a
    // template substitution that emits `" "` instead of nothing must
    // not 400 the request.
    let resp = app()
        .oneshot(post_with_deadline(
            "/v1/lint",
            r#"{"text":"SECRET//NF\n"}"#,
            "   ",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn deadline_header_validated_before_body_deserialization() {
    // Ordering invariant: an out-of-range deadline header must surface
    // as 400 even when the body is also malformed (axum's Json
    // extractor would otherwise short-circuit with 422). This pins the
    // handler ordering that puts deadline validation before
    // `serde_json::from_slice`.
    let resp = app()
        .oneshot(post_with_deadline("/v1/lint", "this is not json", "0"))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "bad-header takes precedence over malformed-body"
    );
}

// ---------------------------------------------------------------------------
// T038 — header omitted runs to completion.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn lint_without_header_uses_endpoint_default() {
    // 30 s default is generous on a one-banner fixture.
    let resp = app()
        .oneshot(post_json("/v1/lint", r#"{"text":"SECRET//NF\n"}"#))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        resp.headers().get("Marque-Truncated").is_none(),
        "non-truncated lint must NOT set Marque-Truncated"
    );
}

// ---------------------------------------------------------------------------
// T039 — fix deadline-exceeded returns 504 with partial-lint body.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn fix_deadline_exceeded_returns_504_with_partial_lint_body() {
    let payload = many_banners_payload(4_000);
    let resp = app()
        .oneshot(post_with_deadline("/v1/fix", &payload, "1"))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::GATEWAY_TIMEOUT,
        "fix deadline-exceeded must be 504"
    );
    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let truncated_by = body_json["truncated_by"].as_str();
    assert!(
        matches!(truncated_by, Some("lint") | Some("fix")),
        "truncated_by must be \"lint\" or \"fix\" (got: {truncated_by:?})"
    );
    assert!(
        body_json["candidates_total"].as_u64().is_some(),
        "504 body must carry candidates_total (got: {body_json})"
    );
}

#[tokio::test]
async fn fix_with_generous_deadline_runs_to_completion() {
    // Sanity: a normal request without a deadline header completes
    // happily under the default 30 s budget.
    let resp = app()
        .oneshot(post_json("/v1/fix", r#"{"text":"SECRET//NF\n"}"#))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
