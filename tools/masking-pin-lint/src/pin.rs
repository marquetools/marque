// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Pin types: data structures representing the result of scanning a single
//! `with_recognizer(StrictRecognizer)` call site.
//!
//! See `scanner` for the AST walker that produces `Pin` values and `main` for
//! the lint-driver that classifies them into pass/warn/fail outcomes.

use std::path::PathBuf;

/// A `with_recognizer(StrictRecognizer)` call site discovered by the scanner.
///
/// The `kind` field encodes the comment-marker analysis applied to the
/// 5-line window above (and including) the call site. `Unmarked`, `BothMarkers`,
/// and `BadFormat` are lint-failure variants; `Masking` and `IntentionalStrict`
/// are valid markers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    /// Absolute path to the source file.
    pub file: PathBuf,
    /// 1-indexed line number of the `with_recognizer(...)` method-call expression.
    pub line: u32,
    /// 1-indexed column of the method-call expression.
    pub column: u32,
    /// Marker classification.
    pub kind: PinKind,
}

/// Comment-marker classification for a pin site.
///
/// Per FR-039 + source-plan §6:
/// - A valid pin carries exactly one of `Masking` or `IntentionalStrict`.
/// - `Unmarked` and `BothMarkers` always fail the lint.
/// - `BadFormat` indicates a marker prefix was found but did not parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinKind {
    /// `// MASKING-PIN: tracks #NNN — <reason>`. Issue state is verified
    /// against the GitHub API by the lint driver.
    Masking {
        /// Tracked GitHub issue number.
        issue: u32,
        /// Free-form rationale text following the issue number.
        reason: String,
    },
    /// `// INTENTIONAL-STRICT: <reason>`. No API check is performed.
    IntentionalStrict {
        /// Free-form rationale text.
        reason: String,
    },
    /// No marker found within the 5-line lookback window.
    Unmarked,
    /// Both `MASKING-PIN` and `INTENTIONAL-STRICT` markers present — illegal.
    BothMarkers,
    /// A marker prefix was found but did not parse against the canonical regex.
    /// The `String` is the offending source line for inclusion in the diagnostic.
    BadFormat(String),
}

impl PinKind {
    /// Returns true if this pin kind is a lint failure regardless of any
    /// downstream API check (i.e., the marker is structurally invalid).
    #[must_use]
    pub fn is_marker_failure(&self) -> bool {
        matches!(
            self,
            PinKind::Unmarked | PinKind::BothMarkers | PinKind::BadFormat(_)
        )
    }
}

/// Aggregated lint diagnostic. Errors fail the run; warnings emit to stderr
/// but exit zero.
#[derive(Debug, Clone)]
pub struct LintDiagnostic {
    /// Severity controls process exit code.
    pub severity: Severity,
    /// Human-readable message; printed verbatim to stderr.
    pub message: String,
}

/// Diagnostic severity. `Error` returns nonzero exit; `Warning` is informational.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Lint error — exit non-zero.
    Error,
    /// Lint warning — emit to stderr, exit zero.
    Warning,
}
