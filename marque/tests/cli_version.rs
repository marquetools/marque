// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `marque --version` schema-discoverability pin.
//!
//! The active audit-record schema name must be discoverable by
//! external consumers without parsing audit records. The CLI
//! surfaces it via `marque --version`:
//!
//! ```text
//! marque 0.3.0
//! audit_schema: marque-3.1
//! ```
//!
//! Shell scripts grep for `^audit_schema:` to detect schema-major
//! changes between binaries; the test pins the grep target so a
//! silent `--version` reformat surfaces as a CI failure.
//!
//! The schema value is sourced from
//! `marque_engine::AUDIT_SCHEMA_VERSION` (single source of truth) at
//! compile time — the const flows from `crates/engine/build.rs`'s
//! accept-list through the `env!("MARQUE_AUDIT_SCHEMA")` re-export.

use std::process::Command;

#[test]
fn version_exposes_audit_schema_name() {
    let bin = env!("CARGO_BIN_EXE_marque");
    let output = Command::new(bin)
        .arg("--version")
        .output()
        .expect("failed to invoke marque --version");
    let stdout = String::from_utf8(output.stdout).expect("stdout was not valid UTF-8");
    let schema_line = stdout
        .lines()
        .find(|l| l.starts_with("audit_schema:"))
        .unwrap_or_else(|| {
            panic!(
                "`marque --version` must expose an `audit_schema:` line per \
                 contracts/audit-record.md §\"Schema discoverability (D3)\". \
                 Stdout was:\n{stdout}"
            )
        });
    // The value comes from marque_engine::AUDIT_SCHEMA_VERSION at
    // build time. The test imports the same const so a coordinated
    // future schema bump (e.g., marque-1.1) updates the assertion
    // alongside the build.rs accept-list automatically.
    let expected = format!("audit_schema: {}", marque_engine::AUDIT_SCHEMA_VERSION);
    assert_eq!(
        schema_line, expected,
        "schema-discoverability line drift detected. Expected {expected:?}, \
         got {schema_line:?}. Shell scripts grepping for ^audit_schema: rely \
         on this exact shape."
    );
}

#[test]
fn version_starts_with_package_version() {
    let bin = env!("CARGO_BIN_EXE_marque");
    let output = Command::new(bin)
        .arg("--version")
        .output()
        .expect("failed to invoke marque --version");
    let stdout = String::from_utf8(output.stdout).expect("stdout was not valid UTF-8");
    // First line is the package version per clap's default
    // version formatter. Pinned so an accidental version-string
    // reorder (e.g., audit_schema line first) doesn't slip
    // through; the package version must lead so consumers reading
    // line 1 see what they expect.
    let first_line = stdout.lines().next().expect("--version output is empty");
    assert!(
        first_line.starts_with("marque "),
        "first line of --version output must start with 'marque ', got {first_line:?}"
    );
}
