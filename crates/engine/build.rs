// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Build script for `marque-engine`.
//!
//! Selects the audit-record schema version emitted by `Engine::fix`.
//! Post PR 3c.B Commit 10 the accept-list is a single value:
//! `["marque-mvp-3"]`. The legacy `mvp-1` / `mvp-2` shapes retired
//! atomically with the `FixProposal` cleanup; their structural
//! envelope (top-level `original` / `replacement` fields) is no
//! longer representable. A single build emits exactly one schema
//! version per FR-014 and R4.
//!
//! The value is surfaced to downstream code via
//! `env!("MARQUE_AUDIT_SCHEMA")`. Rebuilds are triggered when the
//! env var changes.

fn main() {
    // Accepted values. The accept-list is a closed contract; adding
    // or removing a value MUST coordinate with audit-emit paths.
    // `crates/engine/tests/audit_schema_accept_list.rs` regression-
    // pins this verbatim.
    const ACCEPTED: &[&str] = &["marque-mvp-3"];
    const DEFAULT: &str = "marque-mvp-3";

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
