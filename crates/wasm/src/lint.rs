// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::types::{BatchEntry, BatchResultEntry, diagnostic_to_json};
use crate::{build_engine_config, parse_wasm_config, stamp_deadline, with_engine};
use marque_engine::LintOptions;

/// Lint text, returning NDJSON conforming to `contracts/diagnostic.json`.
/// One diagnostic per line, newline-terminated. Byte-identical to the CLI's
/// `--format json` output (SC-008) — the truncation case (deadline tripped
/// mid-pass) returns whatever partial NDJSON the engine produced before
/// abort, exactly matching the CLI's stdout shape on the same condition.
///
/// Spec 005 §R3 / Constitution III analysis (T043): when `config_json`
/// carries `deadline_ms`, the engine's per-candidate deadline check
/// activates and the lint pass cooperatively aborts on expiry. This is
/// a *runtime budget cap*, not a vocabulary or scoring change — the
/// same recognizer codepath runs whether `deadline_ms` is set or not,
/// posteriors are identical, the CVE token set is unchanged. Permitted
/// under the Constitution III "data already present in the strict-path
/// codepath" carve-out.
pub fn lint_native(text: &str, config_json: Option<String>) -> Result<String, String> {
    // Parse upfront to fail fast on a bad `deadline_ms` (NaN / Inf /
    // negative) before any engine work, regardless of whether the
    // engine cache is warm. The cache key strips `deadline_ms` so a
    // caller varying the budget per call hits the warm cache.
    let (wasm_cfg, deadline_duration, cache_key) = parse_wasm_config(&config_json)?;
    let deadline = stamp_deadline(deadline_duration)?;
    with_engine(
        &cache_key,
        move || build_engine_config(wasm_cfg),
        |engine| {
            let mut lint_opts = LintOptions::default();
            lint_opts.deadline = deadline;
            let result = engine.lint_with_options(text.as_bytes(), &lint_opts);

            // Write NDJSON directly into a byte buffer — avoids the intermediate
            // String allocation that serde_json::to_string produces per diagnostic.
            let mut buf = Vec::with_capacity(result.diagnostics.len() * 256);
            for d in &result.diagnostics {
                serde_json::to_writer(&mut buf, &diagnostic_to_json(d))
                    .map_err(|e| e.to_string())?;
                buf.push(b'\n');
            }
            // serde_json always produces valid UTF-8.
            String::from_utf8(buf).map_err(|e| e.to_string())
        },
    )
}

/// Lint multiple text entries in a single WASM boundary crossing.
///
/// Accepts a JSON array of `{"id": "...", "text": "..."}` objects and returns
/// a JSON array of `{"id": "...", "diagnostics": [...]}` results. All entries
/// are linted against the same cached engine.
///
/// Designed for as-you-type feedback: the JS caller debounces keystrokes,
/// extracts the changed paragraphs or marking regions, and sends them as a
/// batch. One boundary crossing, one engine, N lints.
///
/// ```js
/// const results = lint_batch(JSON.stringify([
///   { id: "para-1", text: "(S//NF) First paragraph..." },
///   { id: "para-2", text: "(TS//SI) Second paragraph..." },
/// ]));
/// ```
pub fn lint_batch_native(
    entries_json: &str,
    config_json: Option<String>,
) -> Result<String, String> {
    let entries: Vec<BatchEntry> = serde_json::from_str(entries_json).map_err(|e| e.to_string())?;
    let (wasm_cfg, _, cache_key) = parse_wasm_config(&config_json)?;

    with_engine(
        &cache_key,
        move || build_engine_config(wasm_cfg),
        |engine| {
            let results: Vec<BatchResultEntry<'_>> = entries
                .iter()
                .map(|entry| {
                    let result = engine.lint(entry.text.as_bytes());
                    let diagnostics = result
                        .diagnostics
                        .iter()
                        .map(|d| {
                            let mut buf = Vec::with_capacity(256);
                            serde_json::to_writer(&mut buf, &diagnostic_to_json(d))
                                .map_err(|e| e.to_string())?;
                            let json = String::from_utf8(buf).map_err(|e| e.to_string())?;
                            serde_json::value::RawValue::from_string(json)
                                .map_err(|e| e.to_string())
                        })
                        .collect::<Result<_, String>>()?;

                    Ok(BatchResultEntry {
                        id: &entry.id,
                        diagnostics,
                    })
                })
                .collect::<Result<_, String>>()?;

            let mut buf = Vec::with_capacity(results.len() * 512);
            serde_json::to_writer(&mut buf, &results).map_err(|e| e.to_string())?;
            String::from_utf8(buf).map_err(|e| e.to_string())
        },
    )
}
