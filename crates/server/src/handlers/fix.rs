// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use super::{
    DEFAULT_ENDPOINT_DEADLINE_MS, diagnostics_to_json, reject_if_body_carries_corpus_override,
    reject_if_corpus_override, resolve_request_deadline, stamp_request_deadline,
};
use crate::{
    state::AppState,
    types::{DeadlineExceededBody, FixRequest, FixResponse},
};
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Json, Response},
};
use marque_engine::{EngineError, FixMode, FixOptions, InterfaceCode};
use secrecy::ExposeSecret as _;

pub async fn fix_handler(
    State(state): State<AppState>,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    // Wire-level checks (header + query) run BEFORE body deserialization.
    reject_if_corpus_override("/v1/fix", &uri, &headers)?;

    // Validate the deadline header BEFORE body
    // deserialization so 400 (bad header) takes precedence over 422
    // (malformed JSON). Same `Instant::now()` deferral as `lint_handler`.
    let deadline_duration =
        resolve_request_deadline(&headers, state.deadline_cap, DEFAULT_ENDPOINT_DEADLINE_MS)?;

    let req: FixRequest =
        serde_json::from_slice(&body).map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;

    // Body-field check after successful deserialization.
    reject_if_body_carries_corpus_override("/v1/fix", req.carries_corpus_override())?;

    let mut fix_opts = FixOptions::default();
    fix_opts.threshold_override = req.confidence_threshold;
    fix_opts.deadline = Some(stamp_request_deadline(deadline_duration)?);
    // Per-request identity / signature (issue #399). The server
    // declares its interface so the session-level audit metadata
    // records which surface applied the fix.
    fix_opts.interface = InterfaceCode::Server;
    fix_opts.classifier_id = req.classifier_id.clone();
    fix_opts.classification_authority = req.classification_authority.clone();
    fix_opts.signature = req.signature.clone();
    // #176 / SC-010: thread the trusted-caller recognition input-source
    // into the fix path so `input_source: "structured_field"` applies
    // the assertive recovery on the server fix endpoint, matching the
    // lint endpoint. Absent / unknown → DocumentContent (the safe path).
    fix_opts.input_source = req.resolved_input_source();

    match state
        .engine
        .fix_with_options(req.text.as_bytes(), FixMode::Apply, &fix_opts)
    {
        Ok(result) => {
            let fixed = String::from_utf8(result.source.expose_secret().to_vec())
                .map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;
            // `applied_count` must reflect edit-applying records
            // only. `AuditLine` is `#[non_exhaustive]`; using
            // `audit_lines.len()` would silently inflate the count
            // if a non-edit variant is added later. The two iterator
            // accessors restrict the count to the two known edit
            // arms (AppliedFix + TextCorrection), so adding a new
            // variant becomes an explicit decision at this site.
            let applied_count =
                result.applied_fixes().count() + result.applied_text_corrections().count();

            // issue #184 / #399: serialize each audit record to its
            // canonical NDJSON line and compute a session-end Merkle
            // root over those exact bytes. The session-level
            // `session_metadata` record (versions / seal / interface /
            // identity / signature) is the FIRST line when the stream
            // is non-empty, so the seal and identity are covered by the
            // root (tampering breaks it). Emitting the lines AND hashing
            // them keeps the response self-verifiable (a caller
            // re-hashes `audit_log` → `session_root`).
            let scheme = state.engine.scheme();
            let mut audit_log: Vec<String> = Vec::with_capacity(result.audit_lines.len() + 1);
            if !result.audit_lines.is_empty() {
                audit_log.push(result.session_metadata.to_ndjson());
            }
            audit_log.extend(
                result
                    .audit_lines
                    .iter()
                    .map(|line| marque_engine::audit_line_to_ndjson(scheme, line)),
            );
            let session_root = format!(
                "blake3:{}",
                marque_engine::SessionRoot::compute(&audit_log).root_hex()
            );

            Ok(Json(FixResponse {
                fixed_text: fixed,
                applied_count,
                remaining_diagnostics: result.remaining_diagnostics.len(),
                audit_log,
                session_root,
            })
            .into_response())
        }
        Err(EngineError::DeadlineExceeded { partial_lint }) => {
            // Constitution V Principle V: no partial FixResult is
            // ever produced. The 504 body carries the
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
        // issue #399: the deployment requires a signature
        // (`require_signature`) but the request carried none. This is
        // a client-side omission given operator policy, so surface it
        // as 403 Forbidden rather than a 4xx that implies malformed
        // input or a 5xx that implies a server fault.
        Err(EngineError::SignatureRequired) => Err(StatusCode::FORBIDDEN),
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
