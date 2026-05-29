// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! End-to-end test for the terminal `session_root` audit record
//! (issue #184, `marque-3.1`).
//!
//! Runs the real `marque fix` binary, locates the terminal
//! `session_root` NDJSON record on stderr, and proves that the BLAKE3
//! Merkle `root` it carries recomputes over exactly the preceding audit
//! records — and that mutating any record invalidates it. This exercises
//! the actual emitted bytes (not a stand-in serialization), closing the
//! producer/verifier byte-identity contract end-to-end.

use marque_engine::SessionRoot;
use std::io::Write as _;
use std::process::{Command, Stdio};

/// Run `marque fix --write-stdout -` over `input` (stdin) and return the
/// stderr text (the NDJSON audit stream). Uses plain `std::process` with
/// the `CARGO_BIN_EXE_marque` path that Cargo provides to integration
/// tests, so the test does not depend on any test-harness stdin API.
///
/// Asserts the CLI exited successfully (exit 0): without this, a binary
/// that crashed before emitting a terminal record would produce stderr
/// with no `session_root` line, and `split_audit` would return
/// `(vec![], None)` — silently turning a crash into a "passing" no-audit
/// assertion. Checking the exit status makes the no-audit contract test
/// prove what it claims. (`fix` exits 0 only when the post-fix re-lint is
/// clean; every fixture here is clean-after-fix or fix-free, so 0 is the
/// expected status.)
fn fix_stderr(input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_marque"))
        .arg("fix")
        .arg("--write-stdout")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn marque fix");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(input.as_bytes())
        .expect("write to child stdin");
    let output = child.wait_with_output().expect("failed to run marque fix");
    let stderr = String::from_utf8(output.stderr).expect("stderr was not valid UTF-8");
    assert!(
        output.status.success(),
        "`marque fix` must exit 0 for these fixtures; got {:?}.\nstderr:\n{stderr}",
        output.status.code()
    );
    stderr
}

/// Split the audit stream into (preceding record lines, terminal
/// `session_root` line). Narration lines after the terminal record are
/// ignored. Returns `None` for the terminal line when no `session_root`
/// record was emitted (a clean, zero-fix document).
fn split_audit(stderr: &str) -> (Vec<String>, Option<String>) {
    let lines: Vec<&str> = stderr.lines().collect();
    match lines
        .iter()
        .position(|l| l.contains("\"type\":\"session_root\""))
    {
        Some(idx) => (
            lines[..idx].iter().map(|s| s.to_string()).collect(),
            Some(lines[idx].to_string()),
        ),
        None => (Vec::new(), None),
    }
}

/// Extract `record_count` and the bare hex root (the `blake3:` prefix
/// stripped) from a terminal `session_root` line.
fn parse_terminal(line: &str) -> (usize, String) {
    let v: serde_json::Value = serde_json::from_str(line).expect("terminal record is valid JSON");
    assert_eq!(v["type"], "session_root");
    assert_eq!(
        v["schema"],
        marque_engine::AUDIT_SCHEMA_VERSION,
        "terminal record schema must match the per-record schema constant"
    );
    let count = v["record_count"]
        .as_u64()
        .expect("record_count is an integer") as usize;
    let root = v["root"].as_str().expect("root is a string");
    let hex = root
        .strip_prefix("blake3:")
        .expect("root is rendered as blake3:<hex>")
        .to_string();
    (count, hex)
}

#[test]
fn session_root_recomputes_over_preceding_records() {
    // `SECRET//REL TO GBR` triggers at least one fix (USA trigraph
    // insertion), so the audit stream carries records to hash.
    let stderr = fix_stderr("SECRET//REL TO GBR\n");
    let (records, terminal) = split_audit(&stderr);
    let terminal = terminal.expect("a fixing session must emit a session_root record");
    let (count, hex) = parse_terminal(&terminal);

    assert!(
        !records.is_empty(),
        "fixture should produce at least one audit record; stderr was:\n{stderr}"
    );
    assert_eq!(
        count,
        records.len(),
        "terminal record_count must equal the number of preceding records"
    );

    // The CLI hashes exactly the bytes it emits (each record's
    // canonical NDJSON line, no trailing newline). Recomputing over the
    // captured lines must reproduce the published root.
    let recomputed = SessionRoot::compute(&records);
    assert_eq!(
        recomputed.root_hex(),
        hex,
        "recomputed Merkle root must match the published session_root"
    );
}

#[test]
fn mutating_a_record_invalidates_the_session_root() {
    let stderr = fix_stderr("SECRET//REL TO GBR\n");
    let (records, terminal) = split_audit(&stderr);
    let terminal = terminal.expect("a fixing session must emit a session_root record");
    let (_, hex) = parse_terminal(&terminal);
    let published = SessionRoot::compute(&records);
    assert_eq!(published.root_hex(), hex);

    // Flip one byte of the first record; the root must change.
    let mut tampered = records.clone();
    tampered[0].push(' ');
    assert!(
        !SessionRoot::verify(&tampered, &published.root),
        "a mutated record must fail verification against the published root"
    );
}

#[test]
fn zero_fix_session_emits_no_audit_and_no_terminal_record() {
    // Plain prose with no markings yields no fixes. The CLI preserves its
    // established "no fixes → no audit output" contract: no per-record
    // lines AND no terminal session_root record. (The empty-marker root
    // remains a library primitive, covered by the engine merkle unit
    // test `empty_session_has_well_defined_root`.)
    let stderr = fix_stderr("nothing to mark here.\n");
    let (records, terminal) = split_audit(&stderr);
    assert!(
        records.is_empty(),
        "a clean input produces zero audit records"
    );
    assert!(
        terminal.is_none(),
        "a zero-fix session emits no terminal session_root record (CLI \
         contract); stderr was:\n{stderr}"
    );
}
