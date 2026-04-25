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
use marque_server::{AppState, DEFAULT_BODY_LIMIT_BYTES, build_app, build_app_with_limit};
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

// ---------------------------------------------------------------------------
// Body-size cap (whitepaper §10.2 / gap register #6)
// ---------------------------------------------------------------------------
//
// `build_app` applies an axum `DefaultBodyLimit` Tower layer at
// `DEFAULT_BODY_LIMIT_BYTES`. Oversize requests must surface as
// `413 Payload Too Large` before reaching the handler.
//
// Two angles:
//
// 1. **Default-config gate.** A test-sized cap (4 KiB) is plenty to
//    drive the 413 path without committing 10 MiB of bytes to the
//    test binary; uses `build_app_with_limit` directly.
// 2. **Production-config sanity.** A request just under
//    `DEFAULT_BODY_LIMIT_BYTES` against the default `build_app` returns
//    something other than 413, so the cap is set above realistic
//    traffic.

fn app_with_limit(limit_bytes: usize) -> axum::Router {
    let engine = Engine::new(
        Config::default(),
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    )
    .expect("default CAPCO scheme has no rewrite cycles");
    build_app_with_limit(
        AppState {
            engine: Arc::new(engine),
        },
        limit_bytes,
    )
}

#[tokio::test]
async fn body_above_explicit_limit_is_rejected_with_413() {
    let limit = 4096usize;
    // Construct a JSON request whose `text` field alone exceeds the
    // limit. `"a"` repeated `limit + 1` times sits inside a `{"text":"..."}`
    // wrapper, so the wire body is `>= limit + 12 bytes`.
    let big = "a".repeat(limit + 1);
    let body = format!(r#"{{"text":"{big}"}}"#);
    assert!(body.len() > limit, "test setup: body must exceed limit");

    let resp = app_with_limit(limit)
        .oneshot(post_json("/v1/lint", &body))
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "body of {} bytes against a {limit}-byte limit must return 413; got {}",
        body.len(),
        resp.status()
    );
}

#[tokio::test]
async fn body_at_or_below_explicit_limit_is_processed() {
    let limit = 4096usize;
    // Stay well under the cap so the handler runs and returns 200.
    let small = r#"{"text":"SECRET//NF\n"}"#;
    assert!(small.len() < limit, "test setup: body must fit in limit");

    let resp = app_with_limit(limit)
        .oneshot(post_json("/v1/lint", small))
        .await
        .unwrap();

    assert_ne!(
        resp.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "body of {} bytes against a {limit}-byte limit must NOT return 413",
        small.len()
    );
}

#[tokio::test]
async fn fix_endpoint_honors_body_limit() {
    // Same gate must apply to `/v1/fix` — there's no per-route override
    // on the limit, but a regression that wired the layer below
    // routing instead of above it would break this test.
    let limit = 4096usize;
    let big = "a".repeat(limit + 1);
    let body = format!(r#"{{"text":"{big}"}}"#);

    let resp = app_with_limit(limit)
        .oneshot(post_json("/v1/fix", &body))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn default_limit_admits_realistic_traffic() {
    // Documents up to a few hundred KB pass through `lint` cleanly under
    // the production default. We don't materialize a 10 MiB body in the
    // test binary; instead we send a 256 KiB request and assert 200, which
    // proves the cap is set above the realistic-traffic floor and is not
    // accidentally below it (e.g. by a future change that lowered
    // `DEFAULT_BODY_LIMIT_BYTES` to a misconfigured value).
    let realistic = "a".repeat(256 * 1024);
    let body = format!(r#"{{"text":"{realistic}"}}"#);
    assert!(
        body.len() < DEFAULT_BODY_LIMIT_BYTES,
        "test setup: 256 KiB must fit under the production default"
    );

    let resp = app().oneshot(post_json("/v1/lint", &body)).await.unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "default-config request of {} bytes must return 200, not {}",
        body.len(),
        resp.status()
    );
}
