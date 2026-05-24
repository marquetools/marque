// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `citation-lint` — CI lint enforcing citation fidelity.
//!
//! See `README.md` for design and `--help` for invocation.

#![deny(rust_2018_idioms)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;

use citation_lint::catalog::write_catalog;
use citation_lint::diagnostic::render_stderr;
use citation_lint::lint_workspace;

/// CLI arguments.
///
/// Accepts the workspace root as a positional argument so the
/// invocation matches the established `tools/<name>-lint/` pattern:
/// `cargo run --manifest-path tools/citation-lint/Cargo.toml -- <workspace-dir>`.
#[derive(Debug, Parser)]
#[command(
    name = "citation-lint",
    about = "AST-based lint enforcing citation fidelity",
    version
)]
struct Cli {
    /// Path to the marque workspace root (the directory containing
    /// `crates/`, `tools/`, and `docs/`).
    workspace_dir: PathBuf,

    /// Override the catalog output path. Defaults to
    /// `<workspace>/docs/refactor-006/citation-defect-catalog.md`.
    #[arg(long)]
    catalog_path: Option<PathBuf>,

    /// Skip writing the catalog file. Useful for local diagnostic
    /// runs that don't want to touch the committed catalog.
    #[arg(long)]
    no_catalog: bool,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    let workspace_dir = cli
        .workspace_dir
        .canonicalize()
        .with_context(|| format!("resolving workspace dir {}", cli.workspace_dir.display()))?;
    let defects = lint_workspace(&workspace_dir)?;
    eprintln!(
        "citation-lint: scanned crates/*/src; found {} defect(s)",
        defects.len()
    );
    for d in &defects {
        eprintln!("{}", render_stderr(d));
    }
    if !cli.no_catalog {
        let catalog_path = cli.catalog_path.unwrap_or_else(|| {
            workspace_dir
                .join("docs")
                .join("refactor-006")
                .join("citation-defect-catalog.md")
        });
        write_catalog(&catalog_path, &defects, &workspace_dir)?;
        eprintln!("citation-lint: wrote catalog to {}", catalog_path.display());
    }
    if defects.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}
