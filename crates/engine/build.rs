// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Build script for `marque-engine`.
//!
//! Selects the audit-record schema version emitted by `Engine::fix`.
//! The default is `marque-mvp-2` (the Phase-D schema — see
//! `specs/004-constraints-decoder-vocab/contracts/audit-record-v2.md`);
//! a build may downgrade to `marque-mvp-1` by exporting
//! `MARQUE_AUDIT_SCHEMA=marque-mvp-1` before building. A single build
//! emits exactly one schema version per FR-014 and R4 — mixing v1 and
//! v2 records in the same output stream is a downstream-parser hazard,
//! not a feature.
//!
//! The value is surfaced to downstream code via
//! `env!("MARQUE_AUDIT_SCHEMA")`. Rebuilds are triggered when the
//! env var changes.

fn main() {
    // Accepted values. A future schema bump adds its identifier here
    // and lands alongside any emitter / parser updates; an unknown
    // value is a build error so typos don't silently produce v2 output
    // under a v3-named label.
    const ACCEPTED: &[&str] = &["marque-mvp-1", "marque-mvp-2"];
    const DEFAULT: &str = "marque-mvp-2";

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
