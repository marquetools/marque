// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! marque — a fast, rule-driven text linter, formatter, and transformer. Ships with CAPCO/ISM classification-marking rules.
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

    /// Lint and apply fixes for classification marking violations.
    Fix {
        #[command(flatten)]
        common: CommonOptions,

        /// Files to fix. Use `-` to read from stdin. If no PATH is given,
        /// reads from stdin.
        #[arg(value_name = "PATH")]
        paths: Vec<PathBuf>,

        /// Show what would be fixed without writing.
        #[arg(long)]
        dry_run: bool,

        /// Rewrite files in place (default for file-path inputs).
        /// Mutually exclusive with `--write-stdout`.
        #[arg(long)]
        in_place: bool,

        /// Write fixed content to stdout (default for stdin input).
        /// Mutually exclusive with `--in-place`.
        #[arg(long)]
        write_stdout: bool,

        /// Override the clock for deterministic audit timestamps (RFC 3339).
        /// Requires `MARQUE_ALLOW_FIXED_CLOCK=1` in environment.
        #[arg(long, value_name = "RFC3339")]
        fixed_timestamp: Option<String>,
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
            paths,
            dry_run,
            in_place,
            write_stdout,
            fixed_timestamp,
        } => run_fix(
            &cwd,
            common,
            paths,
            dry_run,
            in_place,
            write_stdout,
            fixed_timestamp,
        ),
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

/// Validate `--confidence-threshold` early per `contracts/cli.md` §"Hard-fail
/// at config load" item 3: outside `[0.0, 1.0]` → `65 EX_DATAERR`.
fn validate_threshold(common: &CommonOptions) -> Result<(), i32> {
    if let Some(t) = common.confidence_threshold {
        if !(0.0..=1.0).contains(&t) || t.is_nan() {
            eprintln!("error: --confidence-threshold {t} is outside [0.0, 1.0]");
            return Err(EX_DATAERR);
        }
    }
    Ok(())
}

fn run_check(cwd: &std::path::Path, common: CommonOptions, paths: Vec<PathBuf>) -> i32 {
    if let Err(code) = validate_threshold(&common) {
        return code;
    }

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
        // contracts/cli.md §"Input handling": non-UTF-8 → 74 EX_IOERR.
        let label = render::label_for(path.as_deref());
        if let Err(code) = validate_utf8(source, &label) {
            return code;
        }

        let result = engine.lint(source);
        // Fix-severity diagnostics are still violations — they just have a
        // fix proposal attached. Treat them as errors for the exit-code
        // gate so `marque check` is usable as a CI block.
        if result.error_count() > 0 || result.fix_count() > 0 {
            overall_errors = true;
        } else if result.warn_count() > 0 {
            overall_warns = true;
        }
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

/// Full `marque fix` implementation (Phase 4, US2 — T047–T051a).
///
/// Applies fixes at or above the configured confidence threshold, emits an
/// NDJSON audit record to stderr for every `AppliedFix` (FR-005a), and
/// computes exit codes from the remaining diagnostics (post-fix re-lint).
///
/// Output routing:
/// - File paths → `--in-place` by default (atomic temp-file rename, T048).
/// - stdin → `--write-stdout` by default.
/// - `--dry-run` → audit records emitted, no file/stdout output.
// Flat CLI argument set mirrors the clap struct exactly; extracting into a
// separate args struct would duplicate all fields without reducing complexity.
#[allow(clippy::too_many_arguments)]
fn run_fix(
    cwd: &std::path::Path,
    common: CommonOptions,
    paths: Vec<PathBuf>,
    dry_run: bool,
    in_place: bool,
    write_stdout: bool,
    fixed_timestamp: Option<String>,
) -> i32 {
    // --explain-config is mutually exclusive with `fix`.
    if common.explain_config {
        eprintln!("error: --explain-config is mutually exclusive with `fix`");
        return EX_USAGE;
    }
    // --dry-run and --in-place are mutually exclusive.
    if dry_run && in_place {
        eprintln!("error: --dry-run and --in-place are mutually exclusive");
        return EX_USAGE;
    }
    // --in-place and --write-stdout are mutually exclusive.
    if in_place && write_stdout {
        eprintln!("error: --in-place and --write-stdout are mutually exclusive");
        return EX_USAGE;
    }
    // --dry-run and --write-stdout are mutually exclusive (dry-run produces
    // no output; --write-stdout has no effect and would confuse the user).
    if dry_run && write_stdout {
        eprintln!("error: --dry-run and --write-stdout are mutually exclusive");
        return EX_USAGE;
    }

    if let Err(code) = validate_threshold(&common) {
        return code;
    }

    let config = match load_config(cwd, &common) {
        Ok(c) => c,
        Err(code) => return code,
    };

    // --fixed-timestamp: gated on MARQUE_ALLOW_FIXED_CLOCK=1 (T051a).
    let engine = if let Some(ref ts_str) = fixed_timestamp {
        if std::env::var("MARQUE_ALLOW_FIXED_CLOCK").as_deref() != Ok("1") {
            eprintln!(
                "error: --fixed-timestamp requires MARQUE_ALLOW_FIXED_CLOCK=1 \
                 in the environment (the fixed-clock seam is off by default \
                 to prevent accidental audit-log falsification)"
            );
            return EX_USAGE;
        }
        let ts = match humantime::parse_rfc3339(ts_str) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("error: invalid RFC 3339 timestamp '{ts_str}': {e}");
                return EX_USAGE;
            }
        };
        Engine::with_clock(
            config,
            vec![Box::new(capco_rules())],
            Box::new(marque_engine::FixedClock::new(ts)),
        )
    } else {
        Engine::new(config, vec![Box::new(capco_rules())])
    };

    let engine_mode = if dry_run {
        marque_engine::FixMode::DryRun
    } else {
        marque_engine::FixMode::Apply
    };

    // Build input list — same pattern as run_check.
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
                        eprintln!("error: {}: {e}", p.display());
                        return EX_IOERR;
                    }
                }
            }
        }
        out
    };

    let stderr = std::io::stderr();
    let stdout = std::io::stdout();
    let mut exit_code = EX_OK;

    for (path, source) in &inputs {
        // contracts/cli.md §"Input handling": non-UTF-8 → 74 EX_IOERR.
        let label = render::label_for(path.as_deref());
        if let Err(code) = validate_utf8(source, &label) {
            return code;
        }

        let result =
            match engine.fix_with_threshold(source, engine_mode, common.confidence_threshold) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("error: {e}");
                    return EX_DATAERR;
                }
            };

        // Audit emission (T049, FR-005a) — NEVER suppressed by -q.
        // Each record is atomic: serialize to buffer, single write_all.
        let mut audit_exit_code: Option<i32> = None;
        {
            let mut stderr_lock = stderr.lock();
            for applied_fix in &result.applied {
                // Set the caller-supplied input identifier on the audit record.
                // The engine leaves `input` as None; the CLI fills it in at the
                // boundary per the architecture contract. Stdin is represented
                // as "-" per contracts/audit-record.json.
                let mut audit_fix = applied_fix.clone();
                audit_fix.input = Some(match path.as_ref() {
                    Some(p) => std::sync::Arc::<str>::from(p.display().to_string()),
                    None => std::sync::Arc::from("-"),
                });
                if let Err(e) = render::render_audit_record(&mut stderr_lock, &audit_fix) {
                    // Do NOT write a plain-text error line here — the audit
                    // stream must contain only valid NDJSON objects (FR-005a).
                    // render_audit_record already emitted a JSON error frame
                    // on the serialization-failure path.
                    //
                    // ErrorKind::Other is set by render_audit_record for
                    // serde_json serialization failures; any other kind is a
                    // true I/O failure (broken pipe, disk full, etc.).
                    let code = if e.kind() == std::io::ErrorKind::Other {
                        EX_DATAERR
                    } else {
                        EX_IOERR
                    };
                    audit_exit_code = Some(code);
                    break;
                }
            }
        }

        if let Some(code) = audit_exit_code {
            // FR-005a: audit emission failure → nonzero exit.
            return code;
        }

        // Output routing.
        let is_stdin_input = path.is_none();
        let should_write_file = !dry_run && !is_stdin_input && !write_stdout;
        let should_write_stdout = !dry_run && (is_stdin_input || write_stdout);

        if should_write_file {
            // T048: atomic temp-file rename for --in-place writes.
            // IO errors on write are fatal — return immediately rather than
            // continuing to the next file with a partially-processed batch.
            if let Some(file_path) = path {
                let dir = file_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."));
                match tempfile::NamedTempFile::new_in(dir) {
                    Ok(mut tmp) => {
                        if let Err(e) = std::io::Write::write_all(&mut tmp, &result.source) {
                            eprintln!("error writing temp file: {e}");
                            return EX_IOERR;
                        }
                        if let Err(e) = tmp.persist(file_path) {
                            eprintln!("error: atomic rename to {}: {e}", file_path.display());
                            return EX_IOERR;
                        }
                    }
                    Err(e) => {
                        eprintln!("error: cannot create temp file in {}: {e}", dir.display());
                        return EX_IOERR;
                    }
                }
            }
        }

        if should_write_stdout {
            let mut stdout_lock = stdout.lock();
            if let Err(e) = std::io::Write::write_all(&mut stdout_lock, &result.source) {
                eprintln!("error writing to stdout: {e}");
                return EX_IOERR;
            }
        }

        // Narration (suppressible with -q) — AFTER audit records.
        let applied_count = result.applied.len();
        if !common.quiet && applied_count > 0 {
            if dry_run {
                eprintln!("{label}: would apply {applied_count} fix(es)");
            } else {
                eprintln!("{label}: applied {applied_count} fix(es)");
            }
        }

        // C1: post-fix re-lint for exit code (T050).
        //
        // Re-lint the post-fix text to catch cascading resolutions (a fix
        // resolved a secondary violation) and introduced violations (unlikely
        // but contractually required). For DryRun mode, the engine returns the
        // original source — to get the "as if applied" text we re-lint the
        // original source with the applied proposals replayed. However, the
        // engine's fix_inner already computes `result.source` as the fixed
        // text for Apply mode. For DryRun, we need the would-be fixed text.
        //
        // Strategy: run a second Apply-mode fix call for DryRun to get the
        // actual post-fix text for re-lint. This is the simplest correct
        // approach — the extra cost is negligible for MVP document sizes.
        let relint_source = if dry_run {
            match engine.fix_with_threshold(
                source,
                marque_engine::FixMode::Apply,
                common.confidence_threshold,
            ) {
                Ok(r) => r.source,
                Err(_) => source.to_vec(), // threshold already validated; unreachable
            }
        } else {
            result.source.clone()
        };
        let relint = engine.lint(&relint_source);
        let has_errors = relint.error_count() > 0 || relint.fix_count() > 0;
        let has_warns = relint.warn_count() > 0;

        if has_errors && matches!(exit_code, EX_OK | EX_DIAG_WARN) {
            exit_code = EX_DIAG_ERROR;
        } else if has_warns && exit_code == EX_OK {
            exit_code = EX_DIAG_WARN;
        }

        if !common.quiet && !result.remaining_diagnostics.is_empty() {
            eprintln!(
                "{label}: {} issue(s) require manual review",
                result.remaining_diagnostics.len()
            );
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

/// Validate that `buf` is valid UTF-8 per `contracts/cli.md` §"Input handling".
/// Returns `EX_IOERR` (74) on non-UTF-8 input.
fn validate_utf8(buf: &[u8], label: &str) -> Result<(), i32> {
    if std::str::from_utf8(buf).is_err() {
        eprintln!("error: {label}: input is not valid UTF-8");
        return Err(EX_IOERR);
    }
    Ok(())
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
