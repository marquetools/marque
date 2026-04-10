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
