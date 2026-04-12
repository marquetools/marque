//! marque — classification marking linter, formatter, and fixer.
//!
//! Phase 3 brings the `check` subcommand fully into compliance with
//! `contracts/cli.md`: stdin sentinel, `--config`, `--confidence-threshold`,
//! `--format human|json`, `--no-color` (with NO_COLOR/TERM=dumb honored),
//! `-q`/`-v`, `--explain-config` (mutually exclusive with paths and `fix`),
//! and exit codes per the contract.

mod render;

use clap::{Args, Parser, Subcommand, ValueEnum};
use marque_capco::capco_rules;
use marque_engine::Engine;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process;

const EX_OK: i32 = 0;
const EX_DIAG_ERROR: i32 = 1;
const EX_DIAG_WARN: i32 = 2;
const EX_USAGE: i32 = 64;
const EX_DATAERR: i32 = 65;
const EX_UNAVAILABLE: i32 = 69;
const EX_IOERR: i32 = 74;

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
        #[command(flatten)]
        common: CommonOptions,

        /// Files to lint. Use `-` to read from stdin. If no PATH is given,
        /// reads from stdin.
        #[arg(value_name = "PATH")]
        paths: Vec<PathBuf>,
    },

    /// Lint and apply fixes. Writes fixed files in-place.
    Fix {
        #[command(flatten)]
        common: CommonOptions,

        #[arg(value_name = "PATH", required = true)]
        files: Vec<PathBuf>,

        /// Show what would be fixed without writing.
        #[arg(long)]
        dry_run: bool,
    },

    /// Report document metadata issues. Currently a stub.
    Metadata {
        #[arg(value_name = "FILE", required = true)]
        files: Vec<PathBuf>,
        #[arg(long)]
        strip: bool,
    },
}

#[derive(Args, Debug, Clone)]
struct CommonOptions {
    /// Override the project config search path.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Minimum confidence for a fix to be auto-applied (0.0..=1.0).
    #[arg(long, value_name = "FLOAT")]
    confidence_threshold: Option<f32>,

    /// Output format. Defaults to `human` for TTY, `json` otherwise.
    #[arg(long, value_enum)]
    format: Option<FormatArg>,

    /// Suppress ANSI color in human format.
    #[arg(long)]
    no_color: bool,

    /// Suppress non-diagnostic stderr narration.
    #[arg(short, long)]
    quiet: bool,

    /// Increase log verbosity.
    #[arg(short, long)]
    verbose: bool,

    /// Dump the merged Configuration as JSON and exit 0. Mutually exclusive
    /// with input paths and with `fix`.
    #[arg(long)]
    explain_config: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum FormatArg {
    Human,
    Json,
}

impl From<FormatArg> for render::Format {
    fn from(value: FormatArg) -> Self {
        match value {
            FormatArg::Human => render::Format::Human,
            FormatArg::Json => render::Format::Json,
        }
    }
}

#[tokio::main]
async fn main() {
    // Parse the CLI BEFORE initializing the tracing subscriber so the
    // `-v` flag can promote the default filter to `marque=debug` per
    // `contracts/cli.md` ("-v equivalent to MARQUE_LOG=marque=debug").
    //
    // Precedence chain (matches FR-007): CLI flag > env var > default.
    // If the user passes `-v`, `marque=debug` wins regardless of
    // MARQUE_LOG. Otherwise MARQUE_LOG wins if set, else `marque=info`.
    let cli = Cli::parse();

    let verbose = match &cli.command {
        Command::Check { common, .. } | Command::Fix { common, .. } => common.verbose,
        Command::Metadata { .. } => false,
    };
    let env_filter = if verbose {
        "marque=debug".to_owned()
    } else {
        std::env::var("MARQUE_LOG").unwrap_or_else(|_| "marque=info".to_owned())
    };
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: cannot determine working directory: {e}");
            process::exit(EX_IOERR);
        }
    };

    // Resolve config: --config wins, otherwise upward walk from cwd.
    let exit_code = match cli.command {
        Command::Check { common, paths } => run_check(&cwd, common, paths),
        Command::Fix {
            common,
            files,
            dry_run,
        } => run_fix(&cwd, common, files, dry_run),
        Command::Metadata { files, strip } => run_metadata(&files, strip).await,
    };

    process::exit(exit_code);
}

fn load_config(
    cwd: &std::path::Path,
    common: &CommonOptions,
) -> Result<marque_config::Config, i32> {
    // Per contracts/cli.md: --config <PATH> short-circuits the upward walk
    // and uses the specified path as the project config (with local-config
    // search in the same directory). Without --config, the walk starts
    // from cwd and stops at the first .marque.toml, .git/, or filesystem
    // root.
    let result = match &common.config {
        Some(path) => marque_config::load_with_explicit_config(path),
        None => marque_config::load(cwd),
    };
    match result {
        Ok(c) => Ok(c),
        Err(e) => {
            eprintln!("error: {e}");
            Err(e.exit_code())
        }
    }
}

fn run_check(cwd: &std::path::Path, common: CommonOptions, paths: Vec<PathBuf>) -> i32 {
    // `-q` / `--quiet` suppresses non-diagnostic stderr narration per
    // contracts/cli.md. The `check` subcommand currently emits NO operator
    // narration at all — only diagnostics on stdout. So `-q` is a no-op
    // for `check` today and the `common.quiet` field is intentionally
    // unread. If a future change adds a file-header narration line or a
    // summary, guard it with `if !common.quiet { eprintln!(...) }`.
    let _ = common.quiet;

    // --explain-config is mutually exclusive with input paths.
    if common.explain_config && !paths.is_empty() {
        eprintln!("error: --explain-config is mutually exclusive with input paths");
        return EX_USAGE;
    }

    let config = match load_config(cwd, &common) {
        Ok(c) => c,
        Err(code) => return code,
    };

    if common.explain_config {
        return run_explain_config(&config);
    }

    let engine = Engine::new(config, vec![Box::new(capco_rules())]);
    let format: render::Format = common
        .format
        .map(Into::into)
        .unwrap_or_else(render::default_format);
    let color = render::use_color(common.no_color);

    // No paths → read from stdin.
    let inputs: Vec<(Option<PathBuf>, Vec<u8>)> = if paths.is_empty() {
        match read_stdin() {
            Ok(buf) => vec![(None, buf)],
            Err(e) => {
                eprintln!("error reading stdin: {e}");
                return EX_IOERR;
            }
        }
    } else {
        let mut out = Vec::with_capacity(paths.len());
        for p in paths {
            let label = p.display().to_string();
            // `-` is the stdin sentinel.
            if p.as_os_str() == "-" {
                match read_stdin() {
                    Ok(buf) => out.push((None, buf)),
                    Err(e) => {
                        eprintln!("error reading stdin: {e}");
                        return EX_IOERR;
                    }
                }
            } else {
                match std::fs::read(&p) {
                    Ok(buf) => out.push((Some(p), buf)),
                    Err(e) => {
                        eprintln!("error: {label}: {e}");
                        return EX_IOERR;
                    }
                }
            }
        }
        out
    };

    let mut overall_errors = false;
    let mut overall_warns = false;
    let stdout = std::io::stdout();
    let mut stdout_lock = stdout.lock();

    for (path, source) in &inputs {
        let result = engine.lint(source);
        // Fix-severity diagnostics are still violations — they just have a
        // fix proposal attached. Treat them as errors for the exit-code
        // gate so `marque check` is usable as a CI block.
        if result.error_count() > 0 || result.fix_count() > 0 {
            overall_errors = true;
        } else if result.warn_count() > 0 {
            overall_warns = true;
        }
        let label = render::label_for(path.as_deref());
        let render_result = match format {
            render::Format::Json => render::render_ndjson(&mut stdout_lock, &result),
            render::Format::Human => {
                render::render_human_result(&mut stdout_lock, &label, source, &result, color)
            }
        };
        if let Err(e) = render_result {
            eprintln!("error writing diagnostics: {e}");
            return EX_IOERR;
        }
    }

    if overall_errors {
        EX_DIAG_ERROR
    } else if overall_warns {
        EX_DIAG_WARN
    } else {
        EX_OK
    }
}

/// Phase 3 stub of `marque fix`. The full implementation lives in Phase 4
/// (US2 — auto-fix with audit trail) and lands via tasks T047–T051a. This
/// function keeps the Phase 2 minimal body so the `fix` subcommand still
/// works for simple cases, but two required behaviors are explicitly
/// deferred:
///
/// - **TODO(phase-4: T049)** — Emit an NDJSON audit record to stderr for
///   every `AppliedFix` in `result.applied`, conforming to
///   `contracts/audit-record.json`. Phase 3 only prints a summary line;
///   FR-005a mandates the full audit stream. See the plan's "NOT Building"
///   section for the Phase 3 scope carve-out.
///
/// - **TODO(phase-4: T048)** — Replace `std::fs::write` with an atomic
///   temp-file-and-rename sequence so a crash mid-write cannot leave a
///   partially-written file on disk. `contracts/cli.md` §"Input handling"
///   mandates atomicity for `--in-place` writes.
fn run_fix(
    cwd: &std::path::Path,
    common: CommonOptions,
    files: Vec<PathBuf>,
    dry_run: bool,
) -> i32 {
    if common.explain_config {
        eprintln!("error: --explain-config is mutually exclusive with `fix`");
        return EX_USAGE;
    }
    let config = match load_config(cwd, &common) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let engine = Engine::new(config, vec![Box::new(capco_rules())]);
    let mode = if dry_run {
        marque_engine::FixMode::DryRun
    } else {
        marque_engine::FixMode::Apply
    };

    let mut exit_code = EX_OK;
    for path in files {
        let source = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("error: {}: {e}", path.display());
                exit_code = EX_IOERR;
                continue;
            }
        };

        let result = match engine.fix_with_threshold(&source, mode, common.confidence_threshold) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error: {e}");
                return EX_DATAERR;
            }
        };
        let applied = result.applied.len();
        // TODO(phase-4: T049) — Emit an NDJSON audit record per
        // AppliedFix here, writing to stderr and conforming to
        // `contracts/audit-record.json` (schema version `marque-mvp-1`).
        // Each record must be serialized to an in-memory buffer and
        // flushed with a single `write_all` ending in `\n` (per FR-005a
        // atomic-emission contract). `-q` must NOT suppress audit lines.
        if dry_run {
            if !common.quiet {
                eprintln!("{}: would apply {} fix(es)", path.display(), applied);
            }
        } else {
            // TODO(phase-4: T048) — Replace with an atomic temp-file
            // rename so a crash mid-write cannot leave a partially-
            // written file on disk. Contract: `contracts/cli.md`
            // §"Input handling".
            if let Err(e) = std::fs::write(&path, &result.source) {
                eprintln!("error writing {}: {e}", path.display());
                exit_code = EX_IOERR;
                continue;
            }
            if !common.quiet {
                eprintln!("{}: applied {} fix(es)", path.display(), applied);
            }
        }
        if !result.remaining_diagnostics.is_empty() {
            if !common.quiet {
                eprintln!(
                    "{}: {} issue(s) require manual review",
                    path.display(),
                    result.remaining_diagnostics.len()
                );
            }
            // Only escalate to EX_DIAG_ERROR when no higher-priority code
            // (e.g. EX_IOERR from an earlier file) has already been
            // recorded. Priority order: EX_IOERR > EX_DIAG_ERROR >
            // EX_DIAG_WARN > EX_OK.
            //
            // `EX_DIAG_WARN` is included in the match for defensive
            // parallelism with `run_check`'s exit-code logic — `run_fix`
            // does not currently emit warnings (it only surfaces errors
            // via `remaining_diagnostics`), but keeping the guard
            // consistent across both subcommands means a future addition
            // of `run_fix` warning support will not need to revisit this.
            if matches!(exit_code, EX_OK | EX_DIAG_WARN) {
                exit_code = EX_DIAG_ERROR;
            }
        }
    }
    exit_code
}

async fn run_metadata(_files: &[PathBuf], _strip: bool) -> i32 {
    eprintln!("metadata command: Kreuzberg integration pending (TODO)");
    EX_UNAVAILABLE
}

fn read_stdin() -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    std::io::stdin().lock().read_to_end(&mut buf)?;
    Ok(buf)
}

/// `--explain-config` JSON dump per `contracts/cli.md`.
///
/// Emits the merged Configuration as JSON to stdout, then exits 0.
///
/// Per the contract: "rule severities, corrections-map keys, confidence
/// threshold, schema version, classifier-id presence as a boolean, *not*
/// the value." The `corrections` field is the sorted list of keys (not a
/// count), and the classifier_id value is NEVER included in the output —
/// only a boolean `classifier_id_present` flag.
fn run_explain_config(config: &marque_config::Config) -> i32 {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();

    // Sort the corrections keys so the output is deterministic across
    // HashMap iteration orders — important for CI-golden consumers.
    let mut corrections_keys: Vec<&String> = config.corrections.keys().collect();
    corrections_keys.sort();

    let json = serde_json::json!({
        "rules": config.rules.overrides,
        "corrections": corrections_keys,
        "confidence_threshold": config.confidence_threshold(),
        "schema_version": config.capco.version,
        "classifier_id_present": config.user.classifier_id.is_some(),
    });
    let s = match serde_json::to_string_pretty(&json) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to serialize config: {e}");
            return EX_DATAERR;
        }
    };
    if writeln!(lock, "{s}").is_err() {
        return EX_IOERR;
    }
    EX_OK
}
