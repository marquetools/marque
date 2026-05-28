// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! CLI integration tests for `marque trace`.
//!
//! Gated on `#[cfg(feature = "decision-tracing")]` — without the
//! feature the subcommand exists in the CLI's clap surface but exits
//! `EX_USAGE` immediately. The Phase E asserts (event-count > 0,
//! content-ignorance) are only meaningful when the engine is actually
//! threading a sink, so the tests only run under the feature.

#![cfg(feature = "decision-tracing")]

use assert_cmd::Command;

fn marque() -> Command {
    Command::cargo_bin("marque").expect("marque binary")
}

/// `--format=summary` returns exit 0 and stdout mentions "decisions".
#[test]
fn trace_summary_smoke() {
    let assert = marque()
        .arg("trace")
        .arg("-")
        .arg("--format=summary")
        .write_stdin("(S//NF)\n")
        .assert()
        .success();
    let out = assert.get_output();
    let stdout = std::str::from_utf8(&out.stdout).expect("utf-8 stdout");
    assert!(
        stdout.contains("decisions"),
        "summary output should mention 'decisions'; got: {stdout}"
    );
    assert!(
        stdout.contains("total cascade chains"),
        "summary output should mention 'total cascade chains'; got: {stdout}"
    );
}

/// `--format=ndjson` produces > 0 NDJSON lines.
#[test]
fn trace_ndjson_emits_events() {
    let assert = marque()
        .arg("trace")
        .arg("-")
        .arg("--format=ndjson")
        .write_stdin("(S//NF)\n")
        .assert()
        .success();
    let out = assert.get_output();
    let stdout = std::str::from_utf8(&out.stdout).expect("utf-8 stdout");
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert!(
        !lines.is_empty(),
        "ndjson output should produce at least 1 event line; got 0 lines, full stdout: {stdout}"
    );
    // Each line should be valid JSON.
    for line in &lines {
        let parsed: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("ndjson line is not valid JSON: {line:?} ({e})"));
        assert!(
            parsed.get("step").is_some(),
            "ndjson event missing `step` field: {line:?}"
        );
        assert!(
            parsed.get("kind").is_some(),
            "ndjson event missing `kind` field: {line:?}"
        );
    }
}

/// `--format=narrate` must not contain any unique substring from the
/// input bytes. Content-ignorance check (Constitution V Principle V).
///
/// The fixture pairs a sentinel literal `"WIDGETIPHRASE"` with the
/// marking text so that if the narrate path ever leaks document content
/// into its output, the sentinel will surface and fail the test.
#[test]
fn trace_narrate_is_content_ignorant() {
    let sentinel = "WIDGETIPHRASE";
    let input = format!("(S//NF) {sentinel} body text\n");
    let assert = marque()
        .arg("trace")
        .arg("-")
        .arg("--format=narrate")
        .write_stdin(input)
        .assert()
        .success();
    let out = assert.get_output();
    let stdout = std::str::from_utf8(&out.stdout).expect("utf-8 stdout");
    assert!(
        !stdout.contains(sentinel),
        "narrate output leaked the input sentinel {sentinel:?}; full stdout: {stdout}"
    );
    // Sanity: narration ran and produced cascade chain output.
    assert!(
        stdout.contains("Cascade chains:") || stdout.contains("Chain"),
        "narrate output missing cascade-chain section; got: {stdout}"
    );
}
