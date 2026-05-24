// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `promote-callsite-lint` CLI entry point.
//!
//! See the crate-level rustdoc on [`promote_callsite_lint`] (the
//! library half) for the lint semantics. This binary aggregates
//! the two passes and exits non-zero on any error-severity
//! diagnostic.

#![warn(clippy::pedantic)]

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;
use promote_callsite_lint::{Diagnostic, Severity, callsite, signature};

/// AST-based CI lint enforcing the engine-only promote-callsite
/// discipline plus the signature-shape discipline for marque.
#[derive(Debug, Parser)]
#[command(name = "promote-callsite-lint", version, about)]
struct Cli {
    /// Workspace root directory (the directory containing the
    /// `crates/` subtree to lint).
    #[arg(long, value_name = "PATH")]
    workspace_dir: PathBuf,

    /// Run only the call-site origin pass (PRC001 / PRC002).
    /// Mutually exclusive with `--signature-only` and `--all`.
    #[arg(long, conflicts_with_all = ["signature_only", "all"])]
    callsite_only: bool,

    /// Run only the D12 signature-shape pass (PRC100).
    /// Mutually exclusive with `--callsite-only` and `--all`.
    #[arg(long, conflicts_with_all = ["callsite_only", "all"])]
    signature_only: bool,

    /// Run both passes (default).
    #[arg(long, conflicts_with_all = ["callsite_only", "signature_only"])]
    all: bool,
}

impl Cli {
    fn run_callsite_pass(&self) -> bool {
        // Run when explicitly selected, when `--all` is set, or when
        // no flag is given (default behavior is "run both passes").
        self.all || self.callsite_only || !self.signature_only
    }

    fn run_signature_pass(&self) -> bool {
        self.all || self.signature_only || !self.callsite_only
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(failed) => {
            if failed {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(err) => {
            eprintln!("promote-callsite-lint: {err:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<bool> {
    let cli = Cli::parse();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    if cli.run_callsite_pass() {
        let mut found = callsite::scan_workspace(&cli.workspace_dir)?;
        diagnostics.append(&mut found);
    }
    if cli.run_signature_pass() {
        let mut found = signature::scan_workspace(&cli.workspace_dir)?;
        diagnostics.append(&mut found);
    }

    let mut any_error = false;
    for diag in &diagnostics {
        if diag.severity == Severity::Error {
            any_error = true;
        }
        eprintln!("{diag}");
    }
    if !any_error {
        eprintln!("promote-callsite-lint: no findings");
    }
    Ok(any_error)
}
