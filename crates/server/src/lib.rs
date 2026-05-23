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
//! ## Corpus-override gate
//!
//! Per Constitution III (WASM-safety / runtime-config restriction),
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

mod handlers;
mod middleware;
mod state;
mod types;

pub mod sandbox;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};

pub use handlers::{fix_handler, health, lint_handler, schema_version};
pub use state::AppState;
pub use types::{
    DeadlineExceededBody, DiagnosticJson, FixJson, FixRequest, FixResponse, HealthResponse,
    LintRequest, LintResponse,
};

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
// Per-request deadline cap (spec 005 §10.2)
// ---------------------------------------------------------------------------

/// Default ceiling for a caller-supplied `X-Marque-Deadline` header
/// (milliseconds). The header value is rejected with `400 Bad Request`
/// if it exceeds this number — preventing a single misbehaving caller
/// from holding a worker for hours.
///
/// 60 s. Two ratios above the default endpoint deadline so callers
/// retrying with extra headroom (e.g., a long batch document) can
/// nudge upward without operator intervention; bounded so the cap
/// stops being a safety control if someone forgets to set it.
pub const DEFAULT_DEADLINE_CAP_MS: u64 = 60_000;
/// Floor for a caller-supplied / operator-configured deadline
/// (milliseconds). 1 ms — anything below would always trip the
/// pre-pass deadline check on entry, producing a fully-truncated
/// lint or `Err(DeadlineExceeded)` for fix; the operator never
/// intends that, so reject it loudly rather than silently degrading.
pub const MIN_DEADLINE_MS: u64 = 1;
/// Ceiling for `MARQUE_MAX_DEADLINE` (milliseconds). 10 min.
///
/// Beyond this, the cap stops bounding a misbehaving caller — a
/// pathological `u64::MAX` value would effectively disable the
/// deadline subsystem. Surface that as a startup error rather than
/// letting "the operator wrote a number" override the safety
/// property the layer exists to provide.
pub const MAX_DEADLINE_CAP_MS: u64 = 600_000;

/// Resolve the per-request deadline cap from `MARQUE_MAX_DEADLINE` or
/// fall back to [`DEFAULT_DEADLINE_CAP_MS`].
///
/// Mirrors the [`resolve_body_limit`] surface: error string suitable
/// for stderr; pure decision logic factored into
/// [`classify_deadline_cap_var`] for direct unit testing without
/// env-var manipulation. The thin wrapper here is the only path that
/// touches `std::env`.
pub fn resolve_deadline_cap() -> Result<std::time::Duration, String> {
    classify_deadline_cap_var(std::env::var("MARQUE_MAX_DEADLINE"))
        .map(std::time::Duration::from_millis)
}

fn classify_deadline_cap_var(var: Result<String, std::env::VarError>) -> Result<u64, String> {
    match var {
        Err(std::env::VarError::NotPresent) => Ok(DEFAULT_DEADLINE_CAP_MS),
        Err(std::env::VarError::NotUnicode(raw)) => Err(format!(
            "MARQUE_MAX_DEADLINE is set but is not valid UTF-8: {raw:?}"
        )),
        Ok(s) => {
            let parsed: u64 = s.parse().map_err(|_| {
                format!("MARQUE_MAX_DEADLINE is not a valid millisecond count: {s:?}")
            })?;
            if parsed < MIN_DEADLINE_MS {
                return Err(format!(
                    "MARQUE_MAX_DEADLINE={parsed} is below the \
                     {MIN_DEADLINE_MS}-ms floor; a zero budget would \
                     trip the deadline check on entry for every request"
                ));
            }
            if parsed > MAX_DEADLINE_CAP_MS {
                return Err(format!(
                    "MARQUE_MAX_DEADLINE={parsed} is above the \
                     {MAX_DEADLINE_CAP_MS}-ms ceiling; the cap exists \
                     to bound a misbehaving caller, and a value this \
                     large effectively disables it"
                ));
            }
            Ok(parsed)
        }
    }
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
    let app = Router::new()
        .route("/v1/health", get(health))
        .route("/v1/schema/version", get(schema_version))
        .route("/v1/lint", post(lint_handler))
        .route("/v1/fix", post(fix_handler))
        .layer(DefaultBodyLimit::max(body_limit_bytes))
        .with_state(state);

    middleware::apply_default_layers(app)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn query_carries_corpus_override_basic() {
        assert!(crate::handlers::query_carries_corpus_override(
            "corpus_override=1"
        ));
        assert!(crate::handlers::query_carries_corpus_override(
            "a=b&corpus_override=x"
        ));
        assert!(crate::handlers::query_carries_corpus_override(
            "corpus-override=file"
        ));
        assert!(crate::handlers::query_carries_corpus_override(
            "CORPUS_OVERRIDE=1"
        ));
        // Bare param (no `=value`).
        assert!(crate::handlers::query_carries_corpus_override(
            "corpus_override"
        ));
    }

    #[test]
    fn query_carries_corpus_override_percent_encoded() {
        // `%5F` decodes to `_`, `%2D` decodes to `-`. A param name
        // using either encoding must still match after decoding.
        assert!(crate::handlers::query_carries_corpus_override(
            "corpus%5Foverride=1"
        ));
        assert!(crate::handlers::query_carries_corpus_override(
            "corpus%5foverride=1"
        ));
        assert!(crate::handlers::query_carries_corpus_override(
            "corpus%2Doverride=1"
        ));
        assert!(crate::handlers::query_carries_corpus_override(
            "a=b&corpus%5Foverride&c=d"
        ));
    }

    #[test]
    fn query_carries_corpus_override_negatives() {
        assert!(!crate::handlers::query_carries_corpus_override(""));
        assert!(!crate::handlers::query_carries_corpus_override("text=hi"));
        // Substring-only matches on param VALUE must NOT trigger —
        // we match on decoded param NAME to avoid false positives
        // where a legitimate field value contains the literal string.
        assert!(!crate::handlers::query_carries_corpus_override(
            "text=corpus_override"
        ));
        assert!(!crate::handlers::query_carries_corpus_override(
            "text=my_corpus_override_is_cool"
        ));
        // A percent-encoded form of the name appearing only as a VALUE
        // must also not trigger — only decoded names are checked.
        assert!(!crate::handlers::query_carries_corpus_override(
            "text=corpus%5Foverride"
        ));
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
    fn classify_body_limit_var_unparsable_is_rejected() {
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

    // -----------------------------------------------------------------
    // `classify_deadline_cap_var` — pure decision logic for the
    // per-request deadline cap (spec 005 §10.2). Mirrors the
    // body-limit suite above.
    // -----------------------------------------------------------------

    #[test]
    fn classify_deadline_cap_var_unset_returns_default() {
        assert_eq!(
            classify_deadline_cap_var(Err(VarError::NotPresent)),
            Ok(DEFAULT_DEADLINE_CAP_MS)
        );
    }

    #[test]
    fn classify_deadline_cap_var_valid_value_passes_through() {
        // Just above the floor.
        assert_eq!(classify_deadline_cap_var(Ok("1".to_owned())), Ok(1));
        // Production-realistic.
        assert_eq!(
            classify_deadline_cap_var(Ok("30000".to_owned())),
            Ok(30_000)
        );
    }

    #[test]
    fn classify_deadline_cap_var_below_floor_is_rejected() {
        let err = classify_deadline_cap_var(Ok("0".to_owned()))
            .expect_err("0 must be rejected as below the 1-ms floor");
        assert!(
            err.contains("1-ms floor"),
            "error must name the floor: {err}"
        );
        assert!(
            err.contains('0'),
            "error must echo back the offending value: {err}"
        );
    }

    #[test]
    fn classify_deadline_cap_var_above_ceiling_is_rejected() {
        let just_above = (MAX_DEADLINE_CAP_MS + 1).to_string();
        let err = classify_deadline_cap_var(Ok(just_above.clone()))
            .expect_err("MAX+1 must be rejected as above the ceiling");
        assert!(
            err.contains("ceiling"),
            "error must name the ceiling: {err}"
        );
        assert!(
            err.contains(&just_above),
            "error must echo back the offending value: {err}"
        );
    }

    #[test]
    fn classify_deadline_cap_var_at_ceiling_is_accepted() {
        assert_eq!(
            classify_deadline_cap_var(Ok(MAX_DEADLINE_CAP_MS.to_string())),
            Ok(MAX_DEADLINE_CAP_MS)
        );
    }

    #[test]
    fn classify_deadline_cap_var_unparsable_is_rejected() {
        let err = classify_deadline_cap_var(Ok("not-a-number".to_owned()))
            .expect_err("garbage value must be rejected");
        assert!(
            err.contains("not a valid millisecond count"),
            "error must name the parse failure: {err}"
        );
        assert!(
            err.contains("not-a-number"),
            "error must echo back the offending value: {err}"
        );
    }

    #[test]
    fn classify_deadline_cap_var_negative_is_rejected() {
        // u64 parsing rejects negatives — lands on the parse-error
        // branch, matching the body-limit precedent.
        let err = classify_deadline_cap_var(Ok("-1".to_owned()))
            .expect_err("negative value must be rejected");
        assert!(
            err.contains("not a valid millisecond count"),
            "negative parse failure must use the parse-error branch: {err}"
        );
    }

    #[test]
    fn classify_deadline_cap_var_not_unicode_is_rejected() {
        #[cfg(unix)]
        let raw: OsString = {
            use std::os::unix::ffi::OsStringExt;
            OsString::from_vec(vec![0xFF, 0xFE])
        };
        #[cfg(not(unix))]
        let raw: OsString = {
            use std::os::windows::ffi::OsStringExt;
            OsString::from_wide(&[0xD800])
        };
        let err = classify_deadline_cap_var(Err(VarError::NotUnicode(raw)))
            .expect_err("non-UTF-8 env value must be rejected");
        assert!(
            err.contains("not valid UTF-8"),
            "error must name the encoding failure: {err}"
        );
    }
}
