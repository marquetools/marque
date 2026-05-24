// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Shared diagnostic type for both lint passes.
//!
//! Diagnostics use rustc-style `<severity>: <code>: <message> at
//! <file>:<line>:<col>` formatting so IDEs that parse compiler
//! output can hyperlink directly to the offending site.

use serde::Serialize;
use std::fmt;
use std::path::PathBuf;

/// Severity of a lint finding.
///
/// Only [`Severity::Error`] influences the binary's exit status.
/// [`Severity::Warning`] is reserved for advisory output that does
/// not block CI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Failing diagnostic: the run exits non-zero.
    Error,
    /// Advisory diagnostic: surfaced to the operator but non-fatal.
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => f.write_str("error"),
            Self::Warning => f.write_str("warning"),
        }
    }
}

/// A single lint finding.
///
/// Construction is performed by the individual lint passes; the
/// `main.rs` entry point aggregates a `Vec<Diagnostic>` from all
/// passes and prints them in the order they were produced.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    /// Path to the file containing the offending construct.
    pub file: PathBuf,
    /// 1-indexed line number.
    pub line: usize,
    /// 1-indexed column number (byte offset, syn convention).
    pub column: usize,
    /// Severity of the finding.
    pub severity: Severity,
    /// Stable error code (e.g., `PRC001`, `PRC100`).
    pub code: &'static str,
    /// Human-readable message, single-line.
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}: {} at {}:{}:{}",
            self.severity,
            self.code,
            self.message,
            self.file.display(),
            self.line,
            self.column,
        )
    }
}
