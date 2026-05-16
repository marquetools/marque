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

pub mod sandbox;

use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode, Uri, header},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
};
use marque_engine::{Engine, EngineError, FixOptions, LintOptions};
use serde::{Deserialize, Deserializer, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};

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

/// Default per-endpoint deadline (milliseconds) when the caller omits the
/// `X-Marque-Deadline` header.
///
/// 30 s. Generous enough that any reasonable single-document call
/// completes before tripping; tight enough that a stuck request does
/// not pin a worker indefinitely. Both `/v1/lint` and `/v1/fix` use
/// the same default in MVP — split if/when a future endpoint has a
/// materially different latency profile.
pub const DEFAULT_ENDPOINT_DEADLINE_MS: u64 = 30_000;
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
pub fn resolve_deadline_cap() -> Result<Duration, String> {
    classify_deadline_cap_var(std::env::var("MARQUE_MAX_DEADLINE")).map(Duration::from_millis)
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
    /// Upper bound for a caller-supplied `X-Marque-Deadline` header
    /// (spec 005 §10.2). When the caller omits the header, each
    /// endpoint applies its own default —
    /// [`DEFAULT_ENDPOINT_DEADLINE_MS`] for lint and fix in MVP — so
    /// this field is only consulted when the header is present and
    /// must be range-checked.
    pub deadline_cap: Duration,
}

impl AppState {
    /// Construct an `AppState` with the default deadline cap. Tests
    /// and embedders that want to control the cap should use
    /// `AppState { engine, deadline_cap: ... }` directly.
    pub fn new(engine: Arc<Engine>) -> Self {
        Self {
            engine,
            deadline_cap: Duration::from_millis(DEFAULT_DEADLINE_CAP_MS),
        }
    }
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
    /// Spec 005 §R3 — `true` when the engine aborted the lint pass
    /// because the per-request deadline expired. Older clients that
    /// do not deserialize unknown fields will silently ignore this;
    /// new clients should pair it with the `Marque-Truncated`
    /// response header (set on the wire-level shell).
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
    /// Number of candidate spans whose rule pass started before the
    /// deadline tripped. On a non-truncated response, equals
    /// `candidates_total`.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub candidates_processed: usize,
    /// Total candidate spans the scanner produced for this document.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub candidates_total: usize,
}

fn is_false(b: &bool) -> bool {
    !*b
}
fn is_zero_usize(n: &usize) -> bool {
    *n == 0
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
    /// Provenance of the fix — `"BuiltinRule" | "CorrectionsMap" |
    /// "MigrationTable" | "DecoderPosterior" |
    /// "DecoderClassificationHeuristic"`. Mirrors the CLI/WASM
    /// `source` field.
    pub source: &'static str,
    /// The kind of fix payload — `"FactAdd" | "FactRemove" |
    /// "Recanonicalize"` for structural rule fixes, `"TextCorrection"`
    /// for byte-substitution fixes (the corrections-map / migration
    /// channel). Mirrors the CLI and WASM diagnostic JSON shape.
    pub intent_kind: &'static str,
    /// Replacement bytes, present only for `TextCorrection` payloads.
    /// `None` for structural-intent fixes (the engine synthesizes the
    /// canonical bytes at fix-application time via `apply_intent` +
    /// `render_canonical`; the server response carries only the
    /// structural commitment, not the materialized bytes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
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

/// Reject a request if its **header** or **query** carries a T3
/// corpus-override claim.
///
/// This function deliberately does NOT inspect the body — at this
/// stage the body has not yet been deserialized, and the wire-level
/// check must run before deserialization so a malformed body still
/// fails with `400 Bad Request` (rather than axum's default `422
/// Unprocessable Entity` from the JSON extractor) when either of
/// these channels carries an override claim.
///
/// The body channel is handled by [`reject_if_body_carries_corpus_override`]
/// after `serde_json::from_slice` succeeds. The two-pass split is
/// intentional: callers always pair both functions, and each one
/// owns exactly one stage of the request lifecycle.
///
/// Visibility is `pub(crate)`: the documented pairing is enforceable
/// only inside this crate (the body-side helper takes a private
/// `PresenceMarker` that we do not want on the public API surface),
/// and the only call sites are the lint and fix handlers in this
/// module. External integrators that need T3 rejection should call
/// the lint/fix handlers themselves rather than reusing this guard
/// directly — the guard's contract is "rejects header + query
/// before body deserialization", not a general-purpose request
/// inspector.
///
/// Returns `Err(StatusCode::BAD_REQUEST)` on any positive signal.
/// Logs the channel but never the payload contents (Constitution V
/// G13: audit-stream content-ignorance).
pub(crate) fn reject_if_corpus_override(
    endpoint: &str,
    uri: &Uri,
    headers: &HeaderMap,
) -> Result<(), StatusCode> {
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

/// Reject a deserialized request if its body carries a T3
/// corpus-override claim. Pairs with [`reject_if_corpus_override`]
/// (which handles header + query before deserialization).
///
/// Runs after `serde_json::from_slice` so `PresenceMarker` records
/// whether the `corpus_override` JSON key was present without
/// deserializing or examining the payload — see the
/// `PresenceMarker` doc comment for the absent-vs-`null`
/// distinction.
fn reject_if_body_carries_corpus_override(
    endpoint: &str,
    presence: &PresenceMarker,
) -> Result<(), StatusCode> {
    if presence.is_present() {
        tracing::warn!(
            target: "marque_server::t3",
            endpoint,
            channel = "body",
            "rejected corpus_override in request body"
        );
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Deadline header parsing (spec 005 §10.2)
// ---------------------------------------------------------------------------

const DEADLINE_HEADER: &str = "x-marque-deadline";
/// Wire-level signal that the lint pass aborted because the deadline
/// expired. Pairs with the `truncated` body field; older clients that
/// only inspect headers can still detect the partial response.
const TRUNCATED_HEADER: &str = "marque-truncated";

/// Resolve the per-request deadline.
///
/// Returns `Ok(Duration)` with the effective deadline budget. If the
/// header is absent — or present but empty — this is the per-endpoint
/// default; otherwise it's the caller-supplied value (validated and
/// in range). The function does NOT return an `Option`: every code
/// path produces a budget the caller can stamp into an `Instant`,
/// because spec 005 §10.2 requires every request to have a deadline.
///
/// Validation rules per spec 005 §10.2:
///
/// - Header absent → `Ok(default)` using the per-endpoint default
///   (typically [`DEFAULT_ENDPOINT_DEADLINE_MS`]).
/// - Header present but empty (or whitespace-only) → same as absent.
///   This matches the "empty == default" convention HTTP libraries
///   use elsewhere and avoids a confusing 400 for a caller that sets
///   the header from a possibly-unset variable.
/// - Header present and parseable as `u64` milliseconds, in the
///   range `[MIN_DEADLINE_MS, deadline_cap]` → `Ok(parsed_duration)`.
/// - Anything else (non-UTF-8, non-numeric, negative, overflow,
///   below floor, above cap) → `Err(BAD_REQUEST)`.
///
/// The `deadline_cap` argument is read from `AppState` so an
/// embedder running in a tightly bounded environment (a CI job, a
/// pre-deploy validator) can dial it down without a recompile.
fn resolve_request_deadline(
    headers: &HeaderMap,
    deadline_cap: Duration,
    default_ms: u64,
) -> Result<Duration, StatusCode> {
    // HTTP allows duplicate headers; intermediaries (proxies, CDNs,
    // service meshes) may merge or reorder them in ways that would
    // change which value `headers.get()` returns. For a safety
    // control like `X-Marque-Deadline`, the only safe answer is to
    // refuse the ambiguity: a single value is honored, two or more
    // is `400 Bad Request`. Same defensive shape that
    // `MARQUE_MAX_DEADLINE` env-var resolution applies at startup.
    let mut iter = headers.get_all(DEADLINE_HEADER).iter();
    let raw = match iter.next() {
        Some(value) => value,
        None => return Ok(Duration::from_millis(default_ms)),
    };
    if iter.next().is_some() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let s = raw.to_str().map_err(|_| StatusCode::BAD_REQUEST)?;
    // Empty / whitespace-only header → fall back to the per-endpoint
    // default, matching the contract for an absent header. A caller
    // building the header from a possibly-unset env var or template
    // variable should not get a 400 just because the substitution
    // emitted nothing.
    if s.trim().is_empty() {
        return Ok(Duration::from_millis(default_ms));
    }
    let ms: u64 = s.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    if ms < MIN_DEADLINE_MS {
        return Err(StatusCode::BAD_REQUEST);
    }
    let cap_ms = deadline_cap.as_millis().min(u64::MAX as u128) as u64;
    if ms > cap_ms {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(Duration::from_millis(ms))
}

/// Stamp `Instant::now() + duration`, mapping platform-clock overflow
/// to `500 Internal Server Error`. The `MAX_DEADLINE_CAP_MS` ceiling
/// (10 min) keeps this comfortably inside any monotonic clock on
/// real hardware, but an embedder constructing
/// `AppState { deadline_cap: ... }` directly with a value larger
/// than the ceiling could in principle hand us a duration that
/// overflows. `Instant::add` panics on overflow, so we use
/// `checked_add` and surface the failure as 500 — the request was
/// well-formed (client sent a value the server's own cap accepted),
/// the misconfiguration is server-side. Mapping this to `400` would
/// blame the client for the operator's typo.
fn stamp_request_deadline(duration: Duration) -> Result<Instant, StatusCode> {
    Instant::now()
        .checked_add(duration)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}

/// JSON body for a 504 deadline-exceeded fix response.
///
/// `truncated_by` distinguishes which phase tripped the deadline:
/// `"lint"` if the lint pass itself aborted (the engine never
/// reached the fix loop), `"fix"` if the lint pass completed and the
/// fix-application loop was the one that ran out of time.
#[derive(Serialize)]
pub struct DeadlineExceededBody {
    pub truncated_by: &'static str,
    pub diagnostics: Vec<DiagnosticJson>,
    pub error_count: usize,
    pub warn_count: usize,
    pub fix_count: usize,
    pub candidates_processed: usize,
    pub candidates_total: usize,
}

fn fix_source_str(source: marque_rules::FixSource) -> &'static str {
    match source {
        marque_rules::FixSource::BuiltinRule => "BuiltinRule",
        marque_rules::FixSource::CorrectionsMap => "CorrectionsMap",
        marque_rules::FixSource::MigrationTable => "MigrationTable",
        marque_rules::FixSource::DecoderPosterior => "DecoderPosterior",
        marque_rules::FixSource::DecoderClassificationHeuristic => "DecoderClassificationHeuristic",
    }
}

fn diagnostics_to_json(result: &marque_engine::LintResult) -> Vec<DiagnosticJson> {
    result
        .diagnostics
        .iter()
        .map(|d| DiagnosticJson {
            rule_id: d.rule.to_string(),
            severity: d.severity.to_string(),
            message: d.message.to_string(),
            start: d.span.start,
            end: d.span.end,
            fix: match (d.fix.as_ref(), d.text_correction.as_ref()) {
                (Some(f), _) => Some(FixJson {
                    source: fix_source_str(f.source),
                    intent_kind: match &f.replacement {
                        marque_scheme::ReplacementIntent::FactAdd { .. } => "FactAdd",
                        marque_scheme::ReplacementIntent::FactRemove { .. } => "FactRemove",
                        marque_scheme::ReplacementIntent::Recanonicalize { .. } => "Recanonicalize",
                        _ => "Unknown",
                    },
                    // Structural rule fix — replacement bytes are
                    // engine-rendered at promotion time. The server
                    // response carries only the structural commitment;
                    // callers needing materialized bytes call the fix
                    // endpoint and read the corrected text.
                    replacement: None,
                    confidence: f.confidence.combined(),
                    migration_ref: f.migration_ref.map(str::to_owned),
                }),
                (None, Some(tc)) => Some(FixJson {
                    source: fix_source_str(tc.source),
                    intent_kind: "TextCorrection",
                    replacement: Some(tc.replacement.to_string()),
                    confidence: tc.confidence.combined(),
                    migration_ref: tc.migration_ref.map(str::to_owned),
                }),
                (None, None) => None,
            },
        })
        .collect()
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
) -> Result<Response, StatusCode> {
    // Wire-level checks (header + query) run BEFORE body deserialization so
    // a request with a malformed body is still rejected with 400 when either
    // of those channels carries an override claim (axum's Json extractor
    // would otherwise short-circuit with 422 before this handler ran).
    reject_if_corpus_override("/v1/lint", &uri, &headers)?;

    // Spec 005 §R3 — validate `X-Marque-Deadline` BEFORE body
    // deserialization so an out-of-range header surfaces as 400, not
    // as 422 (which axum's Json extractor would emit on a malformed
    // body). The `Instant::now()` stamp is deferred until just
    // before the engine call so JSON parse time isn't billed against
    // the caller's budget.
    let deadline_duration =
        resolve_request_deadline(&headers, state.deadline_cap, DEFAULT_ENDPOINT_DEADLINE_MS)?;

    let req: LintRequest =
        serde_json::from_slice(&body).map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;

    // Body-field check after successful deserialization.
    reject_if_body_carries_corpus_override("/v1/lint", &req._corpus_override)?;

    let mut lint_opts = LintOptions::default();
    lint_opts.deadline = Some(stamp_request_deadline(deadline_duration)?);

    let result = state
        .engine
        .lint_with_options(req.text.as_bytes(), &lint_opts);
    let truncated = result.truncated;
    let candidates_processed = result.candidates_processed;
    let candidates_total = result.candidates_total;

    let body = LintResponse {
        error_count: result.error_count(),
        warn_count: result.warn_count(),
        fix_count: result.fix_count(),
        diagnostics: diagnostics_to_json(&result),
        truncated,
        candidates_processed,
        candidates_total,
    };

    if truncated {
        // Spec 005 §10.2 — surface partial-pass status on the wire
        // shell as well as in the body. Status remains 200 because
        // the lint pass produced a usable (if incomplete) result;
        // the asymmetric 504-on-deadline shape is reserved for
        // `fix`, where Constitution V Principle V forbids a partial
        // FixResult.
        Ok(([(TRUNCATED_HEADER, "true")], Json(body)).into_response())
    } else {
        Ok(Json(body).into_response())
    }
}

pub async fn fix_handler(
    State(state): State<AppState>,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    // Wire-level checks (header + query) run BEFORE body deserialization.
    reject_if_corpus_override("/v1/fix", &uri, &headers)?;

    // Spec 005 §R3 — validate the deadline header BEFORE body
    // deserialization so 400 (bad header) takes precedence over 422
    // (malformed JSON). Same `Instant::now()` deferral as `lint_handler`.
    let deadline_duration =
        resolve_request_deadline(&headers, state.deadline_cap, DEFAULT_ENDPOINT_DEADLINE_MS)?;

    let req: FixRequest =
        serde_json::from_slice(&body).map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;

    // Body-field check after successful deserialization.
    reject_if_body_carries_corpus_override("/v1/fix", &req._corpus_override)?;

    let mut fix_opts = FixOptions::default();
    fix_opts.threshold_override = req.confidence_threshold;
    fix_opts.deadline = Some(stamp_request_deadline(deadline_duration)?);

    match state.engine.fix_with_options(
        req.text.as_bytes(),
        marque_engine::FixMode::Apply,
        &fix_opts,
    ) {
        Ok(result) => {
            let fixed =
                String::from_utf8(result.source).map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;
            Ok(Json(FixResponse {
                fixed_text: fixed,
                applied_count: result.applied.len(),
                remaining_diagnostics: result.remaining_diagnostics.len(),
            })
            .into_response())
        }
        Err(EngineError::DeadlineExceeded { partial_lint }) => {
            // Spec 005 §R4 / Constitution V Principle V: no partial
            // FixResult is ever produced. The 504 body carries the
            // partial-lint diagnostics so the caller can render
            // them (matching the CLI's stderr behavior). The
            // `truncated_by` discriminator distinguishes a lint-
            // phase trip ("lint pass aborted before reaching the
            // fix loop") from a fix-phase trip ("lint pass
            // completed; fix application timed out").
            let truncated_by = if partial_lint.truncated {
                "lint"
            } else {
                "fix"
            };
            let body = DeadlineExceededBody {
                truncated_by,
                error_count: partial_lint.error_count(),
                warn_count: partial_lint.warn_count(),
                fix_count: partial_lint.fix_count(),
                diagnostics: diagnostics_to_json(&partial_lint),
                candidates_processed: partial_lint.candidates_processed,
                candidates_total: partial_lint.candidates_total,
            };
            Ok((StatusCode::GATEWAY_TIMEOUT, Json(body)).into_response())
        }
        Err(EngineError::InvalidThreshold(_)) => Err(StatusCode::UNPROCESSABLE_ENTITY),
        // `EngineError` is `#[non_exhaustive]`. A future variant
        // is a server-side condition by default — it represents an
        // engine-internal failure mode that the current handler can't
        // classify as a client input error. Map to 500 so a future
        // engine change doesn't silently start telling clients
        // "your request was malformed" for a server-side issue. If
        // a new variant is genuinely client-driven, the fix is to
        // add an explicit arm that returns 4xx.
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
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
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/schema/version", get(schema_version))
        .route("/v1/lint", post(lint_handler))
        .route("/v1/fix", post(fix_handler))
        .layer(DefaultBodyLimit::max(body_limit_bytes))
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            header::HeaderValue::from_static("nosniff"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            header::X_FRAME_OPTIONS,
            header::HeaderValue::from_static("DENY"),
        ))
        .with_state(state)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
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
