// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use super::{
    DEFAULT_ENDPOINT_DEADLINE_MS, TRUNCATED_HEADER, diagnostics_to_json,
    reject_if_body_carries_corpus_override, reject_if_corpus_override, resolve_request_deadline,
    stamp_request_deadline,
};
use crate::{
    state::AppState,
    types::{LintRequest, LintResponse},
};
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Json, Response},
};
use marque_engine::LintOptions;

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

    // Validate `X-Marque-Deadline` BEFORE body
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
    reject_if_body_carries_corpus_override("/v1/lint", req.carries_corpus_override())?;

    let mut lint_opts = LintOptions::default();
    lint_opts.deadline = Some(stamp_request_deadline(deadline_duration)?);

    // #176 / T015: the server is a trusted caller permitted to opt into
    // the recognition input-source axis per request. Absent / unknown →
    // DocumentContent (byte-identical to the pre-#176 path).
    let input_cx = marque_engine::InputContext::new(req.resolved_input_source());
    let result = state
        .engine
        .lint_with_input_context(req.text.as_bytes(), &lint_opts, &input_cx);
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
        // Surface partial-pass status on the wire
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
