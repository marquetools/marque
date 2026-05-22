// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Regression pin on the audit-schema accept-list and the active
//! schema constant.
//!
//! Post T044 (atomic cutover) the accept-list contracts to a single
//! value: `["marque-2.0"]`. T044 reshaped `RuleId` from a 1-tuple
//! `&'static str` opaque newtype into a `(scheme, predicate_id)`
//! 2-tuple and structured the audit-record `"rule"` JSON field
//! accordingly; the pre-cutover `marque-1.0` shape is not
//! interoperable with post-cutover binaries (clean break per
//! FR-037). The earlier `mvp-1` / `mvp-2` / `mvp-3` shapes retired
//! in PR 3c.2.D alongside the v2 `AppliedFix` reshape, BLAKE3
//! digesting, closed `MessageTemplate` JSON serialization, and
//! `Canonical<S>` provenance wiring, to close the G13 audit-
//! content-ignorance channel structurally.
//!
//! These tests pin both surfaces:
//!
//!   1. The active const exports the expected schema version and
//!      the `AUDIT_SCHEMA_IS_V2_0` discriminant is `true`.
//!   2. The build script's `ACCEPTED` literal matches the expected
//!      shape — adding or removing a value must coordinate with
//!      audit-emit paths, and a silent drift would weaken the
//!      single-schema-per-build invariant (FR-014).

#[test]
fn audit_schema_version_is_v2_0_by_default() {
    assert_eq!(marque_engine::AUDIT_SCHEMA_VERSION, "marque-2.0");
}

#[test]
#[allow(clippy::assertions_on_constants)] // Drift-gate: the const value IS the contract; the assert verifies the build.rs codepath produced the expected true.
fn audit_schema_is_v2_0_const_matches_version() {
    assert!(marque_engine::AUDIT_SCHEMA_IS_V2_0);
}

#[test]
fn build_rs_accept_list_pinned() {
    // Read the build script verbatim and assert the ACCEPTED line
    // matches the expected post-cutover shape. Pinned verbatim
    // because the accept-list IS the contract; any edit forces a
    // coordinated test update.
    let build_rs = include_str!("../build.rs");
    assert!(
        build_rs.contains(r#"const ACCEPTED: &[&str] = &["marque-2.0"];"#),
        "accept-list drifted from `[\"marque-2.0\"]`; \
         coordinate with audit-emit paths before editing build.rs",
    );
    assert!(
        build_rs.contains(r#"const DEFAULT: &str = "marque-2.0";"#),
        "DEFAULT drifted from `\"marque-2.0\"`; coordinate with \
         audit-emit paths before editing build.rs",
    );
}
