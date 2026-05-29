// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Build script for `marque-engine`.
//!
//! Selects the audit-record schema version emitted by `Engine::fix`.
//! The accept-list is a single value: `["marque-3.2"]`. `marque-3.2`
//! (issue #399) is purely additive over `marque-3.1`: it introduces a
//! session-level `session_metadata` record (engine/lattice/decoder
//! versions, an integrity `seal`, the applying interface, classifier
//! identity, and an optional carry-only signature) emitted as the
//! first line of a non-empty audit stream. The per-record
//! `AppliedFix` / `text_correction` shapes are byte-identical to
//! `marque-3.1`. As under earlier schemas, the `Recognition`
//! confidence sub-object and the 2-tuple `RuleId` shape are unchanged.
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
    const ACCEPTED: &[&str] = &["marque-3.2"];
    const DEFAULT: &str = "marque-3.2";

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
