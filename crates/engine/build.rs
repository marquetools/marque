// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Build script for `marque-engine`.
//!
//! Selects the audit-record schema version emitted by `Engine::fix`.
//! Post T044 (atomic cutover) the accept-list is a single value:
//! `["marque-2.0"]`. The pre-T044 `marque-1.0` shape carried the
//! 1-tuple `RuleId(&'static str)` form; T044 reshapes `RuleId` to a
//! `(scheme, predicate_id)` 2-tuple and structures the audit-record
//! `"rule"` JSON field accordingly (object form, never a flattened
//! string). Pre-cutover `marque-1.0` records are not interoperable
//! with post-cutover binaries per FR-037 (clean break, no
//! marque-audit-reader crate). The earlier `mvp-1` / `mvp-2` /
//! `mvp-3` shapes retired in PR 3c.2.D atomically with the v2
//! `AppliedFix` reshape and BLAKE3 digesting. A single build emits
//! exactly one schema version per FR-014.
//!
//! The value is surfaced to downstream code via
//! `env!("MARQUE_AUDIT_SCHEMA")`. Rebuilds are triggered when the
//! env var changes.

fn main() {
    // Accepted values. The accept-list is a closed contract; adding
    // or removing a value MUST coordinate with audit-emit paths.
    // `crates/engine/tests/audit_schema_accept_list.rs` regression-
    // pins this verbatim.
    const ACCEPTED: &[&str] = &["marque-2.0"];
    const DEFAULT: &str = "marque-2.0";

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
