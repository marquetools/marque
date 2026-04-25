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
/// Override at runtime by setting `MARQUE_MAX_BODY_BYTES` to any value
/// in `[MIN_BODY_LIMIT_BYTES, MAX_BODY_LIMIT_BYTES]` — values outside
/// that window are rejected at startup (`resolve_body_limit`).
pub const DEFAULT_BODY_LIMIT_BYTES: usize = 10 * 1024 * 1024;

/// Floor for `MARQUE_MAX_BODY_BYTES`.
///
/// 1 KiB. Below this, every realistic request 413s — including
/// the smallest legitimate `/v1/lint` body — so the cap stops being
/// a safety control and starts being a denial-of-service against
/// the operator's own service. Surface that as a startup error.
pub const MIN_BODY_LIMIT_BYTES: usize = 1024;

/// Ceiling for `MARQUE_MAX_BODY_BYTES`.
///
/// 1 GiB. Above this, the per-request memory footprint stops being
/// a useful DoS control: handlers extract the full body into `Bytes`
/// and then convert into `String`, so a single request at the cap
/// drives an O(cap) allocation. A misconfigured `usize::MAX` value
/// would effectively disable the limit. Surface that as a startup
/// error rather than letting "the operator wrote a number" override
/// the safety property the layer exists to provide.
pub const MAX_BODY_LIMIT_BYTES: usize = 1024 * 1024 * 1024;

/// Resolve the body-size cap from `MARQUE_MAX_BODY_BYTES` or fall back
/// to [`DEFAULT_BODY_LIMIT_BYTES`].
///
/// Returns an error string suitable for stderr if the env var is set
/// but unparseable / unreasonable. Caller decides whether to abort
/// (the binary entry point does) or log-and-default (an embedder
/// might).
///
/// `VarError::NotPresent` (unset) silently falls back to the default;
/// `VarError::NotUnicode` (set but contains invalid UTF-8) is treated
/// as a misconfiguration and surfaces as `Err`. A blanket `Err(_) ⇒
/// default` would hide the real bug behind a default that has nothing
/// to do with what the operator wrote.
///
/// The decision logic is factored into [`classify_body_limit_var`] so
/// the parse-fail / below-floor / not-unicode branches are reachable
/// from a unit test without env-var manipulation. The thin wrapper
/// here is the only path that touches `std::env`, and it's a straight
/// pass-through — anything that would break it would also break the
/// classifier, which the tests cover.
pub fn resolve_body_limit() -> Result<usize, String> {
    classify_body_limit_var(std::env::var("MARQUE_MAX_BODY_BYTES"))
}

/// Pure decision logic for [`resolve_body_limit`].
///
/// Takes the raw `Result` shape of `std::env::var(...)` so tests can
/// exercise every branch by constructing the input directly.
fn classify_body_limit_var(var: Result<String, std::env::VarError>) -> Result<usize, String> {
    match var {
        Err(std::env::VarError::NotPresent) => Ok(DEFAULT_BODY_LIMIT_BYTES),
        Err(std::env::VarError::NotUnicode(raw)) => Err(format!(
            "MARQUE_MAX_BODY_BYTES is set but is not valid UTF-8: {raw:?}"
        )),
        Ok(s) => {
            let parsed: usize = s
                .parse()
                .map_err(|_| format!("MARQUE_MAX_BODY_BYTES is not a valid byte count: {s:?}"))?;
            if parsed < MIN_BODY_LIMIT_BYTES {
                return Err(format!(
                    "MARQUE_MAX_BODY_BYTES={parsed} is below the \
                     {MIN_BODY_LIMIT_BYTES}-byte floor; anything smaller \
                     would make every realistic request 413"
                ));
            }
            if parsed > MAX_BODY_LIMIT_BYTES {
                return Err(format!(
                    "MARQUE_MAX_BODY_BYTES={parsed} is above the \
                     {MAX_BODY_LIMIT_BYTES}-byte ceiling; the cap exists \
                     as a per-request memory-footprint control, and a \
                     value this large effectively disables it"
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
/// layer; oversize requests are rejected by the layer with a
/// `413 Payload Too Large` response before the handler is invoked.
/// The limit applies to every route on the returned router,
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

    // -----------------------------------------------------------------
    // `classify_body_limit_var` — pure decision logic for the
    // body-size cap (whitepaper §10.2 / gap register #6). Tested via
    // synthesized `Result<String, VarError>` inputs so every error
    // branch is reachable without env-var manipulation.
    // -----------------------------------------------------------------

    use std::env::VarError;
    use std::ffi::OsString;

    #[test]
    fn classify_body_limit_var_unset_returns_default() {
        assert_eq!(
            classify_body_limit_var(Err(VarError::NotPresent)),
            Ok(DEFAULT_BODY_LIMIT_BYTES)
        );
    }

    #[test]
    fn classify_body_limit_var_valid_value_passes_through() {
        // Just above the floor.
        assert_eq!(classify_body_limit_var(Ok("1024".to_owned())), Ok(1024));
        // Production-realistic.
        assert_eq!(
            classify_body_limit_var(Ok("10485760".to_owned())),
            Ok(10 * 1024 * 1024)
        );
    }

    #[test]
    fn classify_body_limit_var_below_floor_is_rejected() {
        let err = classify_body_limit_var(Ok("512".to_owned()))
            .expect_err("512 must be rejected as below the 1024-byte floor");
        assert!(
            err.contains("1024-byte floor"),
            "error must name the floor: {err}"
        );
        assert!(
            err.contains("512"),
            "error must echo back the offending value: {err}"
        );
    }

    #[test]
    fn classify_body_limit_var_above_ceiling_is_rejected() {
        // `MAX_BODY_LIMIT_BYTES + 1` is the smallest above-ceiling value;
        // catches an off-by-one regression in the boundary check
        // alongside the larger pathological cases.
        let just_above = (MAX_BODY_LIMIT_BYTES + 1).to_string();
        let err = classify_body_limit_var(Ok(just_above.clone()))
            .expect_err("MAX+1 must be rejected as above the ceiling");
        assert!(
            err.contains("ceiling"),
            "error must name the ceiling: {err}"
        );
        assert!(
            err.contains(&just_above),
            "error must echo back the offending value: {err}"
        );

        // A pathological `usize::MAX` value (or any operator typo of
        // many GiB) must also be rejected — without the ceiling, this
        // value would effectively disable the body-cap as a DoS control.
        let pathological = usize::MAX.to_string();
        assert!(
            classify_body_limit_var(Ok(pathological)).is_err(),
            "usize::MAX must trip the ceiling guard"
        );
    }

    #[test]
    fn classify_body_limit_var_at_ceiling_is_accepted() {
        // The ceiling itself is the largest legitimate value; pin
        // that the boundary is inclusive so a future refactor
        // doesn't quietly tighten it.
        assert_eq!(
            classify_body_limit_var(Ok(MAX_BODY_LIMIT_BYTES.to_string())),
            Ok(MAX_BODY_LIMIT_BYTES)
        );
    }

    #[test]
    fn classify_body_limit_var_zero_is_rejected() {
        // Zero is the most pathological case — accepting it would 413
        // every request including health checks. Below-floor branch
        // catches it.
        let err = classify_body_limit_var(Ok("0".to_owned()))
            .expect_err("0 must be rejected as below the 1024-byte floor");
        assert!(err.contains("0"), "error must echo back the value: {err}");
    }

    #[test]
    fn classify_body_limit_var_unparseable_is_rejected() {
        let err = classify_body_limit_var(Ok("not-a-number".to_owned()))
            .expect_err("garbage value must be rejected");
        assert!(
            err.contains("not a valid byte count"),
            "error must name the parse failure: {err}"
        );
        assert!(
            err.contains("not-a-number"),
            "error must echo back the offending value: {err}"
        );
    }

    #[test]
    fn classify_body_limit_var_negative_is_rejected() {
        // `usize::from_str` rejects negatives — this lands on the
        // "not a valid byte count" branch, not the below-floor
        // branch. Either is acceptable; the test just pins the
        // current dispatch.
        let err = classify_body_limit_var(Ok("-1".to_owned()))
            .expect_err("negative value must be rejected");
        assert!(
            err.contains("not a valid byte count"),
            "negative parse failure must use the parse-error branch: {err}"
        );
    }

    #[test]
    fn classify_body_limit_var_not_unicode_is_rejected() {
        // Construct a deliberately non-UTF-8 OsString. On Unix this is
        // straightforward via `OsStringExt::from_vec`; on Windows we'd
        // use `OsStringExt::from_wide` with an unpaired surrogate.
        // Both targets produce the same VarError shape, so the
        // branch logic is testable on either. Gate the construction
        // helper on `cfg(unix)` to avoid a Windows-only fallback.
        #[cfg(unix)]
        let raw: OsString = {
            use std::os::unix::ffi::OsStringExt;
            // 0xFF is not valid as a leading UTF-8 byte.
            OsString::from_vec(vec![0xFF, 0xFE])
        };
        #[cfg(not(unix))]
        let raw: OsString = {
            // Unpaired high surrogate — not valid UTF-16 either,
            // but `OsString` accepts it on Windows. The
            // `VarError::NotUnicode` shape is identical.
            use std::os::windows::ffi::OsStringExt;
            OsString::from_wide(&[0xD800])
        };

        let err = classify_body_limit_var(Err(VarError::NotUnicode(raw)))
            .expect_err("non-UTF-8 env value must be rejected");
        assert!(
            err.contains("not valid UTF-8"),
            "error must name the encoding failure: {err}"
        );
    }
}
