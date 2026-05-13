// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Regression pin on the audit-schema accept-list and the active
//! schema constant.
//!
//! Post PR 3c.B Commit 10 the accept-list contracts to a single
//! value: `["marque-mvp-3"]`. The legacy `mvp-1` / `mvp-2` shapes
//! (top-level `original` / `replacement` byte fields) retired
//! alongside `FixProposal` to close the G13 audit-content-ignorance
//! channel.
//!
//! These tests pin both surfaces:
//!
//!   1. The active const exports the expected schema version and
//!      the `AUDIT_SCHEMA_IS_V3` discriminant is `true`.
//!   2. The build script's `ACCEPTED` literal matches the expected
//!      shape — adding or removing a value must coordinate with
//!      audit-emit paths, and a silent drift would weaken the
//!      single-schema-per-build invariant (FR-014).

#[test]
fn audit_schema_version_is_mvp3_by_default() {
    assert_eq!(marque_engine::AUDIT_SCHEMA_VERSION, "marque-mvp-3");
}

#[test]
#[allow(clippy::assertions_on_constants)] // Drift-gate: the const value IS the contract; the assert verifies the build.rs codepath produced the expected true.
fn audit_schema_is_v3_const_matches_version() {
    assert!(marque_engine::AUDIT_SCHEMA_IS_V3);
}

#[test]
fn build_rs_accept_list_pinned() {
    // Read the build script verbatim and assert the ACCEPTED line
    // matches the expected post-Commit-10 shape. Pinned verbatim
    // because the accept-list IS the contract; any edit forces a
    // coordinated test update.
    let build_rs = include_str!("../build.rs");
    assert!(
        build_rs.contains(r#"const ACCEPTED: &[&str] = &["marque-mvp-3"];"#),
        "accept-list drifted from `[\"marque-mvp-3\"]`; \
         coordinate with audit-emit paths before editing build.rs",
    );
    assert!(
        build_rs.contains(r#"const DEFAULT: &str = "marque-mvp-3";"#),
        "DEFAULT drifted from `\"marque-mvp-3\"`; coordinate with \
         audit-emit paths before editing build.rs",
    );
}
