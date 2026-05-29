// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::types::{
    FixResultJson, deadline_exceeded_payload, diagnostic_to_json, serialize_audit_line_v1_0,
};
use crate::{build_engine_config, parse_wasm_config, stamp_deadline, with_engine};
use marque_engine::{EngineError, FixMode, FixOptions, InterfaceCode, SessionRoot};
use secrecy::ExposeSecret as _;

/// Fix text, returning a JSON object with `fixed_text`, `applied` audit records,
/// and `remaining` diagnostics.
///
/// The `threshold` parameter always takes precedence over any `confidence_threshold`
/// in `config_json`. This matches the CLI's Layer 4 (CLI flag) override behavior.
///
/// When `config_json` carries `deadline_ms` and the
/// deadline expires during the lint or fix-application pass, this
/// function returns `Err(...)` carrying a JSON-serialized
/// `DeadlineExceededBody` (identical shape to the server's 504
/// response — `truncated_by`, `diagnostics`, `error_count`,
/// `warn_count`, `fix_count`, `candidates_processed`,
/// `candidates_total`). JS callers `try`/`catch` and parse the
/// message body to render the partial-lint diagnostics. No partial
/// `FixResult` is ever returned (Constitution V Principle V).
pub fn fix_native(
    text: &str,
    threshold: f32,
    config_json: Option<String>,
) -> Result<String, String> {
    let (wasm_cfg, deadline_duration, cache_key) = parse_wasm_config(&config_json)?;
    let deadline = stamp_deadline(deadline_duration)?;
    // Capture the carry-only signature before `wasm_cfg` is moved into
    // the engine-config builder (the classifier identity rides on the
    // cached engine's config; the signature is a per-call FixOptions
    // field and never affects the engine cache key).
    let signature = wasm_cfg.signature.clone();
    with_engine(
        &cache_key,
        move || build_engine_config(wasm_cfg),
        |engine| {
            let mut fix_opts = FixOptions::default();
            fix_opts.threshold_override = Some(threshold);
            fix_opts.deadline = deadline;
            // issue #399: declare the WASM interface + carry the
            // signature. Identity (classifier_id /
            // classification_authority) rides on the engine config, so
            // it is left as the FixOptions fallback (`None`) and the
            // engine resolves it from config.
            fix_opts.interface = InterfaceCode::Wasm;
            fix_opts.signature = signature;
            let result = match engine.fix_with_options(text.as_bytes(), FixMode::Apply, &fix_opts) {
                Ok(r) => r,
                Err(EngineError::DeadlineExceeded { partial_lint }) => {
                    return Err(deadline_exceeded_payload(&partial_lint));
                }
                Err(e) => return Err(e.to_string()),
            };

            let fixed_text = String::from_utf8(result.source.expose_secret().to_vec())
                .map_err(|e| format!("invalid UTF-8 in fix output: {e}"))?;

            // Emit reads from `result.audit_lines`. Must produce
            // byte-identical NDJSON to the CLI's render path
            // (`marque/src/render.rs::render_audit_line`) — the parity
            // test at `crates/wasm/tests/audit_v3_0_parity.rs` pins this.
            let scheme = engine.scheme();
            let applied: Vec<Box<serde_json::value::RawValue>> = result
                .audit_lines
                .iter()
                .map(|line| serialize_audit_line_v1_0(scheme, line))
                .collect::<Result<_, _>>()?;

            // Remaining diagnostics as pre-serialized raw JSON. Each diagnostic
            // is serialized once into a byte buffer and wrapped as RawValue so
            // the parent FixResultJson serialization embeds it verbatim — no
            // intermediate serde_json::Value tree, no double serialization.
            let remaining: Vec<Box<serde_json::value::RawValue>> = result
                .remaining_diagnostics
                .iter()
                .map(|d| {
                    let mut buf = Vec::with_capacity(256);
                    serde_json::to_writer(&mut buf, &diagnostic_to_json(d))
                        .map_err(|e| e.to_string())?;
                    let json = String::from_utf8(buf).map_err(|e| e.to_string())?;
                    serde_json::value::RawValue::from_string(json).map_err(|e| e.to_string())
                })
                .collect::<Result<_, _>>()?;

            // issue #399: the session-level metadata record (versions /
            // seal / interface / identity / signature). Emitted only
            // when the fix produced audit records, preserving the "no
            // fixes → no audit output" contract. It is the FIRST line
            // folded into the session root so the seal and identity are
            // covered by it.
            let session_metadata: Option<Box<serde_json::value::RawValue>> = if result
                .audit_lines
                .is_empty()
            {
                None
            } else {
                Some(
                    serde_json::value::RawValue::from_string(result.session_metadata.to_ndjson())
                        .map_err(|e| e.to_string())?,
                )
            };

            // issue #184 / #399: session-end Merkle root over the
            // session-metadata record (when present) followed by the
            // emitted audit records, in that order. Computed over the
            // exact bytes the caller receives (each RawValue's JSON
            // text), so the caller can recompute and verify. Per-call
            // (per-document), imposing no sequentiality on callers that
            // batch multiple fix() calls.
            let mut root_lines: Vec<&str> = Vec::with_capacity(applied.len() + 1);
            if let Some(meta) = session_metadata.as_ref() {
                root_lines.push(meta.get());
            }
            root_lines.extend(applied.iter().map(|r| r.get()));
            let session_root = format!("blake3:{}", SessionRoot::compute(&root_lines).root_hex());

            let fix_result = FixResultJson {
                fixed_text,
                applied,
                remaining,
                r002_fired: result.r002_fired,
                session_metadata,
                session_root,
            };

            // Serialize directly into a byte buffer to avoid serde_json::to_string's
            // intermediate String allocation.
            let mut buf = Vec::with_capacity(1024);
            serde_json::to_writer(&mut buf, &fix_result).map_err(|e| e.to_string())?;
            String::from_utf8(buf).map_err(|e| e.to_string())
        },
    )
}
