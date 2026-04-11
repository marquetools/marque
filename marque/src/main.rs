//! marque — classification marking linter, formatter, and fixer.
//!
//! Usage:
//!   marque check [files...]       lint, exit 1 if errors
//!   marque fix [files...]         lint and apply fixes
//!   marque fix --dry-run [files...] show what would be fixed
//!   marque metadata [files...]    report document metadata issues

use clap::{Parser, Subcommand};
use marque_capco::capco_rules;
use marque_engine::Engine;
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "marque", about = "Classification marking linter and fixer")]
#[command(version, propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Lint files for classification marking violations.
    Check {
        #[arg(value_name = "FILE", required = true)]
        files: Vec<PathBuf>,

        /// Output format: text (default) or json.
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Lint and apply fixes. Writes fixed files in-place.
    Fix {
        #[arg(value_name = "FILE", required = true)]
        files: Vec<PathBuf>,

        /// Show what would be fixed without writing.
        #[arg(long)]
        dry_run: bool,

        /// Minimum confidence threshold for auto-fix (0.0–1.0).
        ///
        /// When omitted, the engine uses the value from `.marque.toml`
        /// (or `MARQUE_CONFIDENCE_THRESHOLD`, default 0.95). When set,
        /// this overrides the config for this invocation only.
        #[arg(long)]
        confidence: Option<f32>,
    },

    /// Report document metadata issues (sensitive fields, EXIF, revision history).
    Metadata {
        #[arg(value_name = "FILE", required = true)]
        files: Vec<PathBuf>,

        /// Strip metadata from documents (writes sanitized copies).
        #[arg(long)]
        strip: bool,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("MARQUE_LOG").unwrap_or_else(|_| "marque=info".to_owned()))
        .init();

    let cli = Cli::parse();

    // H-4: handle working-directory lookup failure explicitly. `current_dir`
    // can fail under chroot/sandbox, or when the cwd has been deleted.
    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: cannot determine working directory: {e}");
            process::exit(74); // EX_IOERR per contracts/cli.md
        }
    };
    // The config loader hard-fails on FR-010 (committed [user] section),
    // FR-011 (schema version mismatch), and threshold/severity validation.
    // These are intentional safety gates — do not silently fall back to
    // `Config::default()` on error.
    let config = match marque_config::load(&cwd) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(e.exit_code());
        }
    };

    let engine = Engine::new(config, vec![Box::new(capco_rules())]);

    let exit_code = match cli.command {
        Command::Check { files, format } => run_check(&engine, &files, &format),
        Command::Fix {
            files,
            dry_run,
            confidence,
        } => run_fix(&engine, &files, dry_run, confidence),
        Command::Metadata { files, strip } => run_metadata(&files, strip).await,
    };

    process::exit(exit_code);
}

fn run_check(engine: &Engine, files: &[PathBuf], format: &str) -> i32 {
    let mut exit_code = 0;

    for path in files {
        let source = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("error: {}: {e}", path.display());
                exit_code = 1;
                continue;
            }
        };

        let result = engine.lint(&source);

        if !result.is_clean() {
            exit_code = 1;
        }

        match format {
            "json" => print_json_diagnostics(path, &result),
            _ => print_text_diagnostics(path, &result),
        }
    }

    exit_code
}

fn run_fix(engine: &Engine, files: &[PathBuf], dry_run: bool, confidence: Option<f32>) -> i32 {
    let mut exit_code = 0;
    let mode = if dry_run {
        marque_engine::FixMode::DryRun
    } else {
        marque_engine::FixMode::Apply
    };

    for path in files {
        let source = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("error: {}: {e}", path.display());
                exit_code = 1;
                continue;
            }
        };

        let result = match engine.fix_with_threshold(&source, mode, confidence) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error: {e}");
                return 65; // EX_DATAERR per contracts/cli.md
            }
        };
        let applied = result.applied.len();

        if dry_run {
            println!("{}: would apply {} fix(es)", path.display(), applied);
        } else {
            if let Err(e) = std::fs::write(path, &result.source) {
                eprintln!("error writing {}: {e}", path.display());
                exit_code = 1;
                continue;
            }
            println!("{}: applied {} fix(es)", path.display(), applied);
        }

        if !result.remaining_diagnostics.is_empty() {
            println!(
                "{}: {} issue(s) require manual review",
                path.display(),
                result.remaining_diagnostics.len()
            );
            exit_code = 1;
        }
    }

    exit_code
}

async fn run_metadata(_files: &[PathBuf], _strip: bool) -> i32 {
    eprintln!("metadata command: Kreuzberg integration pending (TODO)");
    // Exit 69 (EX_UNAVAILABLE) so callers do not interpret silence as
    // success — the command is wired up but the implementation is a stub.
    69
}

fn print_text_diagnostics(path: &std::path::Path, result: &marque_engine::LintResult) {
    for diag in &result.diagnostics {
        println!(
            "{}:{}:{} [{}] {}",
            path.display(),
            diag.span.start,
            diag.span.end,
            diag.rule,
            diag.message,
        );
        if let Some(fix) = &diag.fix {
            println!(
                "  → fix (confidence {:.0}%): {:?}",
                fix.confidence * 100.0,
                fix.replacement,
            );
        }
    }
}

fn print_json_diagnostics(path: &std::path::Path, result: &marque_engine::LintResult) {
    let json = serde_json::json!({
        "file": path.display().to_string(),
        "diagnostics": result.diagnostics.iter().map(|d| serde_json::json!({
            "rule": d.rule.to_string(),
            "severity": d.severity.to_string(),
            "message": d.message,
            "start": d.span.start,
            "end": d.span.end,
        })).collect::<Vec<_>>(),
    });
    match serde_json::to_string_pretty(&json) {
        Ok(s) => println!("{s}"),
        Err(e) => eprintln!("error: failed to serialize diagnostics: {e}"),
    }
}
