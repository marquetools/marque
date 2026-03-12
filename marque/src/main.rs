//! marque — classification marking linter, formatter, and fixer.
//!
//! Usage:
//!   marque check [files...]       lint, exit 1 if errors
//!   marque fix [files...]         lint and apply fixes
//!   marque fix --dry-run [files...] show what would be fixed
//!   marque metadata [files...]    report document metadata issues

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;
use marque_engine::Engine;
use marque_config::{self, Config};
use marque_capco::capco_rules;

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
        #[arg(long, default_value_t = 0.90)]
        confidence: f32,
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
        .with_env_filter(
            std::env::var("MARQUE_LOG")
                .unwrap_or_else(|_| "marque=info".to_owned())
        )
        .init();

    let cli = Cli::parse();

    let config = marque_config::load(std::env::current_dir().unwrap().as_path())
        .unwrap_or_else(|e| {
            eprintln!("warning: could not load config: {e}");
            Config::default()
        });

    let engine = Engine::new(config, vec![Box::new(capco_rules())]);

    let exit_code = match cli.command {
        Command::Check { files, format } => run_check(&engine, &files, &format),
        Command::Fix { files, dry_run, confidence } => run_fix(&engine, &files, dry_run, confidence),
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
            _      => print_text_diagnostics(path, &result),
        }
    }

    exit_code
}

fn run_fix(engine: &Engine, files: &[PathBuf], dry_run: bool, _confidence: f32) -> i32 {
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

        let result = engine.fix(&source);
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
    0
}

fn print_text_diagnostics(path: &PathBuf, result: &marque_engine::LintResult) {
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

fn print_json_diagnostics(path: &PathBuf, result: &marque_engine::LintResult) {
    let json = serde_json::json!({
        "file": path.display().to_string(),
        "diagnostics": result.diagnostics.iter().map(|d| serde_json::json!({
            "rule": d.rule.to_string(),
            "severity": format!("{:?}", d.severity),
            "message": d.message,
            "start": d.span.start,
            "end": d.span.end,
        })).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&json).unwrap());
}
