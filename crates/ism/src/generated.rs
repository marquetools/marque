// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Generated code wrappers — `include!()` bridges to `OUT_DIR/` build.rs output.
//!
//! The actual generated files (`values.rs`, `validators.rs`, `migrations.rs`)
//! are produced by `build.rs` from ODNI ISM schema files and written to `OUT_DIR`.
//! They are never checked into version control.

/// CVE enumeration values: SCI controls, dissem controls, country trigraphs, etc.
pub mod values {
    include!(concat!(env!("OUT_DIR"), "/values.rs"));
}

/// Schematron-derived validation predicates.
pub mod validators {
    include!(concat!(env!("OUT_DIR"), "/validators.rs"));
}

/// Deprecated marking → replacement migration table.
pub mod migrations {
    include!(concat!(env!("OUT_DIR"), "/migrations.rs"));
}

/// Per-token metadata derived from the ODNI ISM JSON sidecars
/// (`ism::package_root() / CVE/ISM/CVEnum*.json`, vendored via the
/// `ism` build-dependency). Provides the publishing authority (URN,
/// source, point of contact, schema version) and long-form description
/// for every token in the active CVE vocabulary.
///
/// The composition of these raw records into the
/// `marque-scheme::Vocabulary<S>` trait surface lives in
/// `marque-capco` (Phase 5 PR-2 task T084). `marque-ism` cannot
/// reference the scheme types directly per Constitution VII —
/// it sits below `marque-scheme` in the dependency graph.
pub mod vocabulary {
    include!(concat!(env!("OUT_DIR"), "/vocabulary.rs"));
}
