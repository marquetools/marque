// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Library entry point for the `masking-pin-lint` binary.
//!
//! The crate ships as a binary; this library exposes the same modules to
//! integration tests so they can drive the scanner without spawning a process.
//!
//! See the binary entry at `src/main.rs` and the design notes in `README.md`.

#![deny(rust_2018_idioms)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

pub mod cache;
pub mod github;
pub mod pin;
pub mod scanner;

pub use cache::{cache_age, cache_path, is_stale, read_cache, write_cache, CachedIssueState};
pub use github::{check_pin, ApiError, IssueState, TerminalState};
pub use pin::{LintDiagnostic, Pin, PinKind, Severity};
pub use scanner::scan_workspace;
