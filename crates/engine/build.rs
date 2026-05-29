// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Build script for `marque-engine`.
//!
//! Selects the audit-record schema version emitted by `Engine::fix`.
//! The accept-list is a single value: `["marque-3.1"]`. Under that
//! schema, the `Confidence` two-axis payload was retired in favor of a
//! single `Recognition` axis (PR B); the audit-record `"confidence"`
//! sub-object on the wire drops the `rule` and `region` fields. The
//! 2-tuple `RuleId` shape (introduced at `marque-2.0`) is unchanged.
//! Older record shapes are not interoperable with current binaries
//! (clean break — there is no audit-reader crate). A single build
//! emits exactly one schema version.
//!
//! The value is surfaced to downstream code via
//! `env!("MARQUE_AUDIT_SCHEMA")`. Rebuilds are triggered when the
//! env var changes.

fn main() {
    // Accepted values. The accept-list is a closed contract; adding
    // or removing a value MUST coordinate with audit-emit paths.
    // `crates/engine/tests/audit_schema_accept_list.rs` regression-
    // pins this verbatim.
    const ACCEPTED: &[&str] = &["marque-3.1"];
    const DEFAULT: &str = "marque-3.1";

    let schema = std::env::var("MARQUE_AUDIT_SCHEMA").unwrap_or_else(|_| DEFAULT.to_string());

    if !ACCEPTED.contains(&schema.as_str()) {
        panic!(
            "MARQUE_AUDIT_SCHEMA={schema:?} is not a recognized schema. \
             Accepted: {ACCEPTED:?}."
        );
    }

    println!("cargo:rustc-env=MARQUE_AUDIT_SCHEMA={schema}");
    println!("cargo:rerun-if-env-changed=MARQUE_AUDIT_SCHEMA");
}
