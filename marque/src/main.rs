// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! marque — a fast, rule-driven text linter, formatter, and transformer. Ships with CAPCO/ISM classification-marking rules.
//!
//! The `check` subcommand supports: a stdin sentinel, `--config`,
//! `--confidence-threshold`, `--format human|json`, `--no-color` (with
//! NO_COLOR/TERM=dumb honored), `-q`/`-v`, `--explain-config` (mutually
//! exclusive with paths and `fix`), and the documented exit codes.

mod render;
#[cfg(feature = "decision-tracing")]
mod trace;

use clap::{Args, Parser, Subcommand, ValueEnum};
use marque_capco::capco_rules;
use marque_engine::{Engine, EngineError, FixOptions, InterfaceCode, LintOptions};
use secrecy::ExposeSecret as _;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process;
use std::time::{Duration, Instant};

const EX_OK: i32 = 0;
const EX_DIAG_ERROR: i32 = 1;
const EX_DIAG_WARN: i32 = 2;
/// Exit code surfaced when the engine emitted an `R002` synthetic
/// diagnostic — pass-1 fixes applied but the resulting buffer was
/// unparseable, so pass-2 was skipped.
///
/// Numerically `3` sits adjacent to the diagnostic exit codes and
/// distinct from the sysexits-style range starting at `64`.
const EX_R002_PARTIAL: i32 = 3;
const EX_USAGE: i32 = 64;
const EX_DATAERR: i32 = 65;
const EX_UNAVAILABLE: i32 = 69;
const EX_IOERR: i32 = 74;
const EX_TEMPFAIL: i32 = 75;

/// Reduce two exit codes to a single value using the precedence chain.
///
/// Precedence (high → low):
/// `EX_R002_PARTIAL` (3) > `EX_DIAG_ERROR` (1) > `EX_DIAG_WARN` (2) > `EX_OK` (0).
///
/// R002 wins over generic error because R002 is the rare, distinguished,
/// action-changing signal — pass-2 was skipped because pass-1 made the
/// buffer unparseable. A consumer seeing `EX_DIAG_ERROR` thinks
/// "diagnostics found, normal exit"; a consumer seeing `EX_R002_PARTIAL`
/// thinks "something unusual happened, investigate." When both signals
/// are present in the same document or batch, the user needs the R002
/// signal because it changes workflow.
///
/// Numeric `max()` is NOT the right operator: `max(EX_DIAG_ERROR,
/// EX_DIAG_WARN) = max(1, 2) = 2` would silently demote an error to a
/// warning. The constants are not ordered by severity.
///
/// If a future R003-class signal lands, extending the chain is
/// mechanical — adding it ahead of or behind R002 is the policy
/// question for that PR.
fn merge_exit_code(current: i32, new_code: i32) -> i32 {
    match (current, new_code) {
        (EX_R002_PARTIAL, _) | (_, EX_R002_PARTIAL) => EX_R002_PARTIAL,
        (EX_DIAG_ERROR, _) | (_, EX_DIAG_ERROR) => EX_DIAG_ERROR,
        (EX_DIAG_WARN, _) | (_, EX_DIAG_WARN) => EX_DIAG_WARN,
        _ => EX_OK,
    }
}

/// Extended `--version` string that exposes the active audit-record
/// schema name alongside the package version.
///
/// The active audit schema name must be discoverable by external
/// consumers without parsing audit records. Two surfaces provide this:
///
/// 1. **Per-record discoverability** — every audit record includes
///    a `"schema"` field with the active version. Streaming
///    consumers detect schema by reading the first record.
/// 2. **Per-binary discoverability** — `marque --version` exposes
///    `audit_schema: <AUDIT_SCHEMA_VERSION>` on its own line so
///    shell scripts can detect schema-major changes without running
///    the binary against a real document.
///
/// The value is sourced from `marque_engine::AUDIT_SCHEMA_VERSION`
/// (single source of truth — the build.rs accept-list + the const
/// re-export are the only places the schema name appears in the binary).
///
/// Format: two lines, key/value with a colon separator. Grep
/// target is `^audit_schema:`. Initialized via `OnceLock` at first
/// access because clap's `version =` accepts only `&'static str`;
/// a `String` lifetime extension via `OnceLock` is the standard
/// pattern for this case.
fn version_str() -> &'static str {
    use std::sync::OnceLock;
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(|| {
        format!(
            "{}\naudit_schema: {}",
            env!("CARGO_PKG_VERSION"),
            marque_engine::AUDIT_SCHEMA_VERSION,
        )
    })
}

#[derive(Parser)]
#[command(name = "marque", about = "Classification marking linter and fixer")]
#[command(version = version_str(), propagate_version = true)]
#[command(after_help = ENV_HELP)]
#[command(after_long_help = ENV_HELP)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

// Surfaced at the bottom of `marque --help` and `marque <subcommand> --help`
// so operators discover the env-var surface (and its production-safety caveats)
// without having to read the whitepaper. The MARQUE_LOG=trace warning is the
// load-bearing line: trace-level logging is not safe for runs over classified
// content because future changes to engine logging may begin interpolating
// fragments of input on the trace path.
const ENV_HELP: &str = "ENVIRONMENT VARIABLES:
    MARQUE_LOG                       tracing-subscriber filter (e.g. \"marque=debug\").
                                     WARNING: trace-level (\"marque=trace\") is NOT
                                     production-safe for classified content. The
                                     engine treats info/warn/debug as content-free
                                     today, but trace is reserved for future
                                     diagnostic output that may interpolate input
                                     fragments. Use trace only against synthetic
                                     fixtures or unclassified test corpora.
    MARQUE_CLASSIFIER_ID             Identity stamped into audit records.
    MARQUE_CLASSIFICATION_AUTHORITY  Authority string stamped into audit records.
    MARQUE_AUDIT_SCHEMA              Build-time audit schema selector
                                     (accept-list: \"marque-3.1\"; default
                                     \"marque-3.1\"). Read at build time only.
                                     Run \"marque --version\" to discover the
                                     active schema in any binary.
    MARQUE_ALLOW_FIXED_CLOCK         Set to \"1\" to permit `--fixed-timestamp`
                                     (off by default; the fixed-clock seam exists
                                     for deterministic snapshot tests, NOT for
                                     production audit-log generation).
    MARQUE_CONFIDENCE_THRESHOLD      Override the auto-apply confidence floor.
    NO_COLOR / TERM=dumb             Suppress ANSI color in human-format output.";

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

    /// Render a per-document decision trace from the decision-tracing
    /// instrumentation. Three output formats: `summary` (human-readable
    /// totals), `ndjson` (one event per line), `narrate` (English
    /// cascade-chain walk).
    ///
    /// Only useful when this build was compiled with `--features
    /// decision-tracing`; otherwise the subcommand exits EX_USAGE with
    /// a "rebuild with the feature" message.
    Trace {
        /// File to trace. Use `-` to read from stdin. If no PATH is
        /// given, reads from stdin.
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,

        /// Output format.
        #[arg(long, value_enum, default_value_t = TraceFormatArg::Summary)]
        format: TraceFormatArg,
    },
}

/// `--format` value for the `trace` subcommand. Mirrored from
/// [`trace::TraceFormat`] so the clap `ValueEnum` derive does not need
/// to be feature-gated (the variant always exists; the handler is what
/// gets gated out).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum TraceFormatArg {
    Summary,
    Ndjson,
    Narrate,
}

#[cfg(feature = "decision-tracing")]
impl From<TraceFormatArg> for trace::TraceFormat {
    fn from(value: TraceFormatArg) -> Self {
        match value {
            TraceFormatArg::Summary => trace::TraceFormat::Summary,
            TraceFormatArg::Ndjson => trace::TraceFormat::Ndjson,
            TraceFormatArg::Narrate => trace::TraceFormat::Narrate,
        }
    }
}

#[derive(Args, Debug, Clone)]
struct CommonOptions {
    /// Override the project config search path.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Minimum confidence for a fix to be auto-applied (0.0..=1.0).
    #[arg(long, value_name = "FLOAT")]
    confidence_threshold: Option<f32>,

    /// Maximum wall-clock budget for processing each input document.
    /// Format: humantime, e.g. "30s", "2m", "500ms". Zero or unparseable
    /// values are rejected with EX_USAGE (64). On expiry, `check`
    /// returns whatever lint diagnostics were produced before the
    /// abort and the existing diagnostic gate decides the exit code
    /// (`0` clean, `1` errors / fix-severity, `2` warnings); `fix`
    /// returns no FixResult and exits EX_TEMPFAIL (75).
    #[arg(long, value_name = "DURATION")]
    deadline: Option<String>,

    /// Classifier identity stamped into the audit record for this run.
    /// Overrides `MARQUE_CLASSIFIER_ID` / `.marque.local.toml`. Issue
    /// #399 — surfaced in the session-level audit metadata and in each
    /// applied-fix record.
    #[arg(long, value_name = "ID")]
    classifier_id: Option<String>,

    /// Classification authority stamped into the session-level audit
    /// metadata. Overrides `.marque.local.toml`.
    #[arg(long, value_name = "AUTHORITY")]
    classification_authority: Option<String>,

    /// Caller-supplied detached signature (carry-only; marque does not
    /// sign). Stamped into the session-level audit metadata. Required
    /// when the project config sets `require_signature` — otherwise
    /// `fix` exits with a data error.
    #[arg(long, value_name = "SIGNATURE")]
    signature: Option<String>,

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

    /// Install a corpus override (JSON) for the decoder. Available only
    /// when this build of marque was compiled with the `corpus-override`
    /// Cargo feature — the WASM target does not declare the feature
    /// (Constitution III) and `marque-server` rejects override input on
    /// every channel, so the flag is CLI-only by construction. Every
    /// decoder fix produced under override is stamped with
    /// `CorpusOverrideInEffect` in its audit record. Override priors do
    /// not yet substitute into decoder scoring.
    #[cfg(feature = "corpus-override")]
    #[arg(long, value_name = "PATH")]
    corpus_override: Option<PathBuf>,
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
    // Precedence chain: CLI flag > env var > default. If the user
    // passes `-v`, `marque=debug` wins regardless of MARQUE_LOG.
    // Otherwise MARQUE_LOG wins if set, else `marque=info`.
    //
    // Production-safety note (whitepaper §11.4): info/warn/debug emit only
    // structured, content-free fields today. Trace level is reserved for
    // future diagnostic output that may interpolate input fragments and is
    // therefore NOT production-safe for runs over classified content. The
    // CLI help (`--help`) surfaces this warning in the ENV section; if a
    // future change introduces a trace-level statement that touches input
    // bytes, the warning above must be promoted to a runtime stderr notice
    // emitted when the resolved filter contains `trace`.
    let cli = Cli::parse();

    let verbose = match &cli.command {
        Command::Check { common, .. } | Command::Fix { common, .. } => common.verbose,
        Command::Metadata { .. } | Command::Trace { .. } => false,
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
        Command::Trace { path, format } => run_trace(path, format),
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

/// Load and install a `--corpus-override` payload onto `engine`.
///
/// When the feature is compiled in AND `common.corpus_override` is
/// `Some(_)`, parse the JSON via
/// `marque_config::corpus_override::load_corpus_override` and call
/// `engine.with_corpus_override(...)`. Returns `EX_DATAERR` /
/// `EX_IOERR` on parse / IO errors.
///
/// Without the `corpus-override` feature this function is a no-op
/// passthrough.
#[cfg_attr(not(feature = "corpus-override"), allow(unused_variables))]
fn install_corpus_override(
    engine: marque_engine::Engine,
    common: &CommonOptions,
) -> Result<marque_engine::Engine, i32> {
    #[cfg(feature = "corpus-override")]
    {
        let Some(path) = common.corpus_override.as_ref() else {
            return Ok(engine);
        };
        let parsed = match marque_config::corpus_override::load_corpus_override(path) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("error: {e}");
                return Err(e.exit_code());
            }
        };
        Ok(engine.with_corpus_override(std::sync::Arc::new(parsed)))
    }
    #[cfg(not(feature = "corpus-override"))]
    {
        Ok(engine)
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

/// Parse `--deadline` if supplied. Parse failures and zero durations
/// both exit `EX_USAGE` (64). humantime requires a unit
/// suffix, so a bare `--deadline 0` fails at parse time; `--deadline 0s`
/// parses to `Duration::ZERO` which we explicitly reject (a zero budget
/// would always trip the pre-pass deadline check on entry, producing a
/// fully-truncated lint or `Err(DeadlineExceeded)` for fix — not the
/// caller's intent).
fn validate_deadline(common: &CommonOptions) -> Result<Option<Duration>, i32> {
    let Some(raw) = common.deadline.as_deref() else {
        return Ok(None);
    };
    let duration = match humantime::parse_duration(raw) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: --deadline {raw:?}: {e}");
            return Err(EX_USAGE);
        }
    };
    if duration.is_zero() {
        eprintln!("error: --deadline must be greater than zero");
        return Err(EX_USAGE);
    }
    Ok(Some(duration))
}

/// Stamp `Instant::now() + duration` for a single input.
///
/// `Duration` is user-controlled via `--deadline`, so we must use
/// `checked_add`: the platform monotonic clock has a finite range
/// (Linux's `CLOCK_MONOTONIC` measures nanoseconds since boot; a
/// caller passing `--deadline 100000y` overflows it), and the
/// non-checked `Instant + Duration` panics on overflow. We map
/// overflow to `EX_USAGE` (64) — same exit code as a malformed
/// `--deadline` value, since the input that caused it is also
/// malformed in spirit (an unbounded budget is the user's intent
/// failing to round-trip as a finite Instant).
///
/// Returns `Ok(None)` when no deadline was requested, `Ok(Some(_))`
/// for a representable `Instant`, and `Err(EX_USAGE)` on overflow.
fn stamp_deadline(deadline_duration: Option<Duration>) -> Result<Option<Instant>, i32> {
    let Some(d) = deadline_duration else {
        return Ok(None);
    };
    match Instant::now().checked_add(d) {
        Some(instant) => Ok(Some(instant)),
        None => {
            eprintln!(
                "error: --deadline is too large for the platform clock; \
                 pick a value that fits in Instant + Duration"
            );
            Err(EX_USAGE)
        }
    }
}

fn run_check(cwd: &std::path::Path, common: CommonOptions, paths: Vec<PathBuf>) -> i32 {
    if let Err(code) = validate_threshold(&common) {
        return code;
    }
    let deadline_duration = match validate_deadline(&common) {
        Ok(d) => d,
        Err(code) => return code,
    };

    // `-q` / `--quiet` suppresses non-diagnostic stderr narration per
    // contracts/cli.md. The `check` subcommand consumes `common.quiet`
    // below to gate the spec-005 truncation warning; if a future change
    // adds further narration (file-header line, summary), guard it the
    // same way.

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

    let engine = match Engine::new(
        config,
        vec![Box::new(capco_rules())],
        marque_engine::default_scheme(),
    ) {
        Ok(e) => e,
        Err(err) => {
            eprintln!("error: failed to construct engine: {err}");
            return err.exit_code();
        }
    };
    // Install the CLI-supplied corpus override.
    let engine = match install_corpus_override(engine, &common) {
        Ok(e) => e,
        Err(code) => return code,
    };
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

        // Stamp the deadline ONCE per input so a slow earlier file
        // does not consume the budget allotted to a later one
        // (per-document semantics). For stdin / single-doc
        // invocations this is identical to a single boundary stamp.
        // `LintOptions` is `#[non_exhaustive]` so we mutate a default
        // rather than struct-construct across the crate boundary.
        let mut lint_opts = LintOptions::default();
        lint_opts.deadline = match stamp_deadline(deadline_duration) {
            Ok(d) => d,
            Err(code) => return code,
        };
        let result = engine.lint_with_options(source, &lint_opts);
        // The truncation warning is operator narration on stderr, not a
        // diagnostic, so it falls under the `-q / --quiet` contract
        // (`contracts/cli.md` §"Suppress non-diagnostic stderr
        // narration"). Diagnostics on stdout are unchanged either way.
        if result.truncated && !common.quiet {
            eprintln!(
                "{label}: ⚠ deadline exceeded: covered {}/{} candidates",
                result.candidates_processed, result.candidates_total
            );
        }
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

/// Full `marque fix` implementation.
///
/// Applies fixes at or above the configured confidence threshold, emits an
/// NDJSON audit record to stderr for every `AppliedFix`, and computes exit
/// codes from the remaining diagnostics (post-fix re-lint).
///
/// Output routing:
/// - File paths → `--in-place` by default (atomic temp-file rename).
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
    let deadline_duration = match validate_deadline(&common) {
        Ok(d) => d,
        Err(code) => return code,
    };

    let config = match load_config(cwd, &common) {
        Ok(c) => c,
        Err(code) => return code,
    };

    // --fixed-timestamp: gated on MARQUE_ALLOW_FIXED_CLOCK=1.
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
            marque_engine::default_scheme(),
            Box::new(marque_engine::FixedClock::new(ts)),
        )
    } else {
        Engine::new(
            config,
            vec![Box::new(capco_rules())],
            marque_engine::default_scheme(),
        )
    };
    let engine = match engine {
        Ok(e) => e,
        Err(err) => {
            eprintln!("error: failed to construct engine: {err}");
            return err.exit_code();
        }
    };
    // Install the CLI-supplied corpus override.
    let engine = match install_corpus_override(engine, &common) {
        Ok(e) => e,
        Err(code) => return code,
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

        // Stamp the per-document deadline BEFORE the fix call so each
        // input gets its own budget (per-document semantics).
        let mut fix_opts = FixOptions::default();
        fix_opts.threshold_override = common.confidence_threshold;
        fix_opts.deadline = match stamp_deadline(deadline_duration) {
            Ok(d) => d,
            Err(code) => return code,
        };
        // issue #399: per-run identity / signature + interface code.
        // The flags override the resolved config identity; the engine
        // resolves `None` back to the config value.
        fix_opts.interface = InterfaceCode::Cli;
        fix_opts.classifier_id = common.classifier_id.clone();
        fix_opts.classification_authority = common.classification_authority.clone();
        fix_opts.signature = common.signature.clone();
        let result = match engine.fix_with_options(source, engine_mode, &fix_opts) {
            Ok(r) => r,
            Err(EngineError::InvalidThreshold(it)) => {
                eprintln!("error: {it}");
                return EX_DATAERR;
            }
            Err(EngineError::DeadlineExceeded { partial_lint }) => {
                // Constitution V Principle V: no partial FixResult is
                // ever produced, so there is no fixed text to write.
                // Render the partial-lint diagnostics
                // to stderr so the operator can see what the engine
                // had identified before the abort, then exit
                // EX_TEMPFAIL to signal "transient failure, retry
                // with a larger budget."
                //
                // All writes go through the held `stderr_lock`
                // (rather than `eprintln!`) so the diagnostic block
                // and the trailing explanation are emitted as a
                // single contiguous run — `eprintln!` would re-enter
                // the global Stderr handle, which works on the
                // single-threaded CLI but invites interleaving and
                // muddies the output ordering contract that the JSON
                // format relies on.
                let stderr = std::io::stderr();
                let mut stderr_lock = stderr.lock();
                let format: render::Format = common
                    .format
                    .map(Into::into)
                    .unwrap_or_else(render::default_format);
                let color = render::use_color(common.no_color);
                let render_result = match format {
                    render::Format::Json => render::render_ndjson(&mut stderr_lock, &partial_lint),
                    render::Format::Human => render::render_human_result(
                        &mut stderr_lock,
                        &label,
                        source,
                        &partial_lint,
                        color,
                    ),
                };
                if let Err(e) = render_result {
                    // Drop the lock so the error message is emitted
                    // through the standard stderr handle without
                    // contending with the in-progress diagnostic
                    // stream we just failed to write.
                    drop(stderr_lock);
                    eprintln!("error writing partial-lint diagnostics: {e}");
                    return EX_IOERR;
                }
                // The trailing explanation is only emitted in human
                // format. In JSON mode the stderr stream is NDJSON
                // (one diagnostic per line); appending a plain-text
                // narration line would corrupt the format and break
                // pipe consumers (`marque fix --format json | jq …`).
                // JSON consumers learn about the deadline from the
                // exit code (75) and can re-lint with a larger
                // budget if needed.
                if let render::Format::Human = format
                    && let Err(e) = writeln!(
                        stderr_lock,
                        "{label}: ⚠ deadline exceeded after processing {}/{} \
                     candidates; no fixes applied",
                        partial_lint.candidates_processed, partial_lint.candidates_total
                    )
                {
                    drop(stderr_lock);
                    eprintln!("error writing deadline-exceeded explanation: {e}");
                    return EX_IOERR;
                }
                return EX_TEMPFAIL;
            }
            // issue #399: project config sets `require_signature` but no
            // `--signature` was supplied. No fix is applied; surface an
            // actionable message and exit with a data error.
            Err(EngineError::SignatureRequired) => {
                eprintln!(
                    "error: this project requires a signature (require_signature is set in \
                     .marque.toml); re-run with --signature <SIGNATURE>"
                );
                return EX_DATAERR;
            }
            // `EngineError` is `#[non_exhaustive]` so the compiler
            // requires a wildcard. A future variant lands here as a
            // generic data error rather than silently mapping to one
            // of the existing exit codes.
            Err(e) => {
                eprintln!("error: {e}");
                return EX_DATAERR;
            }
        };

        // Audit emission — NEVER suppressed by -q. Each record is
        // atomic: serialize to buffer, single write_all.
        //
        // Emit reads from `result.audit_lines`, which preserves
        // cross-record promotion order across both the marking-fix arm
        // and the text-correction arm.
        let mut audit_exit_code: Option<i32> = None;
        // issue #184: collect each emitted record's canonical NDJSON bytes
        // (byte-identical to what `render_audit_line` writes — both feed the
        // same `audit_line_to_json_v1_0` value to serde_json) so a terminal
        // `session_root` BLAKE3 Merkle record can be emitted at session
        // close. `session_ts` tracks the latest record timestamp for the
        // terminal `ts` field (informational; NOT part of the Merkle input,
        // so the root stays reproducible under a fixed clock).
        let mut session_record_lines: Vec<String> = Vec::with_capacity(result.audit_lines.len());
        let mut session_ts: Option<std::time::SystemTime> = None;
        let scheme = engine.scheme();
        let input_label: std::sync::Arc<str> = match path.as_ref() {
            Some(p) => std::sync::Arc::<str>::from(p.display().to_string()),
            None => std::sync::Arc::from("-"),
        };
        {
            let mut stderr_lock = stderr.lock();
            // issue #399: emit the session-level metadata record FIRST
            // (versions / seal / interface / identity / signature) so
            // the terminal `session_root` Merkle root covers it and any
            // tampering with the seal or identity is detectable. Gated
            // on a non-empty audit stream to preserve the established
            // "no fixes -> no audit output" CLI contract. Pushed to
            // `session_record_lines` (no trailing newline) ahead of the
            // per-record lines so it is the first Merkle leaf.
            if !result.audit_lines.is_empty() {
                let meta_line = result.session_metadata.to_ndjson();
                if let Err(e) = writeln!(stderr_lock, "{meta_line}") {
                    audit_exit_code = Some(if e.kind() == std::io::ErrorKind::Other {
                        EX_DATAERR
                    } else {
                        EX_IOERR
                    });
                } else {
                    session_record_lines.push(meta_line);
                }
            }
            for line in &result.audit_lines {
                if audit_exit_code.is_some() {
                    break;
                }
                // Set the caller-supplied input identifier on the
                // audit record. The engine leaves `input` as None;
                // the CLI fills it in at the boundary per the
                // architecture contract. Stdin is represented as
                // "-" per contracts/audit-record.json.
                //
                // `AuditLine` is `#[non_exhaustive]`; the two
                // current arms (AppliedFix / TextCorrection) clone
                // through the per-arm `input` field rebind. A
                // future variant falls through to the wildcard
                // arm and is rendered as-is (without the CLI's
                // `input_label` patched in) so the NDJSON stream
                // does not silently drop the record. The audit
                // content-ignorance canary catches the projection
                // surface change at the same time — together they
                // make the addition of a new variant a loud failure,
                // not a silent omission from `--audit-out`.
                let line_with_input: std::borrow::Cow<'_, _> = match line {
                    marque_rules::AuditLine::AppliedFix(fix) => {
                        let mut cloned = fix.clone();
                        cloned.input = Some(input_label.clone());
                        std::borrow::Cow::Owned(marque_rules::AuditLine::AppliedFix(cloned))
                    }
                    marque_rules::AuditLine::TextCorrection(tc) => {
                        let mut cloned = tc.clone();
                        cloned.input = Some(input_label.clone());
                        std::borrow::Cow::Owned(marque_rules::AuditLine::TextCorrection(cloned))
                    }
                    _ => std::borrow::Cow::Borrowed(line),
                };
                // issue #184: capture timestamp + canonical bytes for the
                // session-root Merkle computation before emitting the record.
                match line {
                    marque_rules::AuditLine::AppliedFix(f) => {
                        session_ts = session_ts.max(Some(f.timestamp));
                    }
                    marque_rules::AuditLine::TextCorrection(tc) => {
                        session_ts = session_ts.max(Some(tc.timestamp));
                    }
                    _ => {}
                }
                session_record_lines.push(
                    serde_json::to_string(&render::audit_line_to_json_v1_0(
                        scheme,
                        &line_with_input,
                    ))
                    .unwrap_or_default(),
                );
                if let Err(e) =
                    render::render_audit_line(&mut stderr_lock, scheme, &line_with_input)
                {
                    // Do NOT write a plain-text error line here — the audit
                    // stream must contain only valid NDJSON objects (FR-005a).
                    // render_audit_line already emitted a JSON error frame
                    // on the serialization-failure path.
                    //
                    // ErrorKind::Other is set by render_audit_line for
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
            // Audit emission failure → nonzero exit.
            return code;
        }

        // issue #184: terminal `session_root` record — a BLAKE3 Merkle root
        // over the emitted audit records (excluding itself). Emitted on both
        // apply and `--dry-run` (the audit stream is identical in both modes).
        // Gated on a non-empty audit stream so the established "no fixes →
        // no audit output" CLI contract is preserved (a clean document
        // produces no records and therefore no terminal record). The
        // empty-session marker root remains a verifiable library primitive
        // (`marque_engine::SessionRoot::compute(&[])`) for embedders that
        // want one. The `ts` is the latest record timestamp (deterministic
        // under a fixed clock).
        if !session_record_lines.is_empty() {
            let root = marque_engine::SessionRoot::compute(&session_record_lines);
            let ts = session_ts.unwrap_or_else(std::time::SystemTime::now);
            let ts_str = humantime::format_rfc3339(ts).to_string();
            let terminal = root.to_ndjson(marque_engine::AUDIT_SCHEMA_VERSION, &ts_str);
            // Gate the stderr write on a non-empty audit stream so the
            // established "no fixes -> no audit output" CLI contract holds
            // (a clean / empty document emits nothing). The empty-marker
            // root stays a library primitive for embedders that want one.
            let write_res = if session_record_lines.is_empty() {
                Ok(())
            } else {
                let mut stderr_lock = stderr.lock();
                writeln!(stderr_lock, "{terminal}")
            };
            if let Err(e) = write_res {
                eprintln!("error writing session-root audit record: {e}");
                return EX_IOERR;
            }
        }

        // Output routing.
        let is_stdin_input = path.is_none();
        let should_write_file = !dry_run && !is_stdin_input && !write_stdout;
        let should_write_stdout = !dry_run && (is_stdin_input || write_stdout);

        if should_write_file {
            // Atomic temp-file rename for --in-place writes.
            // IO errors on write are fatal — return immediately rather than
            // continuing to the next file with a partially-processed batch.
            if let Some(file_path) = path {
                let dir = file_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."));
                match tempfile::NamedTempFile::new_in(dir) {
                    Ok(mut tmp) => {
                        if let Err(e) =
                            std::io::Write::write_all(&mut tmp, result.source.expose_secret())
                        {
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
            if let Err(e) =
                std::io::Write::write_all(&mut stdout_lock, result.source.expose_secret())
            {
                eprintln!("error writing to stdout: {e}");
                return EX_IOERR;
            }
        }

        // Narration (suppressible with -q) — AFTER audit records.
        let applied_count = result.audit_lines.len();
        if !common.quiet && applied_count > 0 {
            if dry_run {
                eprintln!("{label}: would apply {applied_count} fix(es)");
            } else {
                eprintln!("{label}: applied {applied_count} fix(es)");
            }
        }

        // Post-fix re-lint for exit code.
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
        // actual post-fix text for re-lint. We reuse the same `fix_opts`
        // (same `Instant` deadline + same threshold) so the per-document
        // budget covers BOTH the primary call and this replay — without
        // this, `--deadline` could be exceeded by the dry-run replay
        // running unbounded after the primary call succeeded. If the
        // replay itself trips the deadline, we fall back to the original
        // source for the re-lint baseline (worst case: exit code reflects
        // pre-fix diagnostics).
        let relint_source: Vec<u8> = if dry_run {
            match engine.fix_with_options(source, marque_engine::FixMode::Apply, &fix_opts) {
                Ok(r) => r.source.expose_secret().to_vec(),
                Err(_) => source.to_vec(),
            }
        } else {
            result.source.expose_secret().to_vec()
        };
        // The re-lint pass for exit-code accounting also runs under the
        // same per-document deadline. If it trips, we fall back to a
        // deadline-free shape so the exit code still reflects whatever
        // diagnostics the truncated re-lint did surface — accuracy degrades
        // gracefully rather than blocking termination.
        let mut relint_opts = LintOptions::default();
        relint_opts.deadline = fix_opts.deadline;
        let relint = engine.lint_with_options(&relint_source, &relint_opts);
        let has_errors = relint.error_count() > 0 || relint.fix_count() > 0;
        let has_warns = relint.warn_count() > 0;

        // R002 takes priority over every other diagnostic signal.
        // Test it BEFORE the error/warn branch so a document with
        // both R002 and ordinary errors surfaces R002
        // — the partial-application story is what the operator
        // needs to act on.
        let row_code = if result.r002_fired {
            EX_R002_PARTIAL
        } else if has_errors {
            EX_DIAG_ERROR
        } else if has_warns {
            EX_DIAG_WARN
        } else {
            EX_OK
        };
        exit_code = merge_exit_code(exit_code, row_code);

        // Suggest-channel diagnostics are advisory — they don't
        // "require manual review", they offer optional alternatives.
        // Filter them out of the narration count so a Suggest-only
        // document produces no "require manual review" line. The
        // suggestions themselves still appear in the diagnostic
        // stream above; this only affects the summary tally.
        if !common.quiet {
            let review_count = result
                .remaining_diagnostics
                .iter()
                .filter(|d| d.severity != marque_rules::Severity::Suggest)
                .count();
            if review_count > 0 {
                eprintln!("{label}: {review_count} issue(s) require manual review");
            }
        }
    }
    exit_code
}

async fn run_metadata(_files: &[PathBuf], _strip: bool) -> i32 {
    eprintln!("metadata command: Kreuzberg integration pending (TODO)");
    EX_UNAVAILABLE
}

/// Dispatch the `trace` subcommand. With `decision-tracing` enabled,
/// forwards to [`trace::run_trace`]. Without the feature, returns
/// `EX_USAGE` with a message instructing the operator to rebuild.
#[cfg(feature = "decision-tracing")]
fn run_trace(path: Option<PathBuf>, format: TraceFormatArg) -> i32 {
    trace::run_trace(path, format.into())
}

#[cfg(not(feature = "decision-tracing"))]
fn run_trace(_path: Option<PathBuf>, _format: TraceFormatArg) -> i32 {
    eprintln!(
        "error: this build of marque does not have decision-tracing enabled; \
         rebuild with `cargo build -p marque --features decision-tracing`"
    );
    EX_USAGE
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

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod exit_code_tests {
    //! `merge_exit_code` precedence-chain locks.
    //!
    //! These tests pin the rule that R002 wins over generic error and
    //! that the reduction is NOT numeric `max()`. The reduction is
    //! used both per-document (in `run_fix`) and per-row (when batch
    //! support lands); changing the precedence is a policy decision
    //! that this test bank exists to surface in code review.

    use super::*;

    #[test]
    fn ok_with_ok_is_ok() {
        assert_eq!(merge_exit_code(EX_OK, EX_OK), EX_OK);
    }

    #[test]
    fn warn_beats_ok() {
        assert_eq!(merge_exit_code(EX_OK, EX_DIAG_WARN), EX_DIAG_WARN);
        assert_eq!(merge_exit_code(EX_DIAG_WARN, EX_OK), EX_DIAG_WARN);
    }

    #[test]
    fn error_beats_warn() {
        // numeric `max(1, 2) = 2` would be WRONG: error must win.
        assert_eq!(merge_exit_code(EX_DIAG_ERROR, EX_DIAG_WARN), EX_DIAG_ERROR);
        assert_eq!(merge_exit_code(EX_DIAG_WARN, EX_DIAG_ERROR), EX_DIAG_ERROR);
    }

    #[test]
    fn r002_beats_error() {
        // R002 is the rare, distinguished, action-changing signal.
        assert_eq!(
            merge_exit_code(EX_R002_PARTIAL, EX_DIAG_ERROR),
            EX_R002_PARTIAL
        );
        assert_eq!(
            merge_exit_code(EX_DIAG_ERROR, EX_R002_PARTIAL),
            EX_R002_PARTIAL
        );
    }

    #[test]
    fn r002_beats_warn() {
        assert_eq!(
            merge_exit_code(EX_R002_PARTIAL, EX_DIAG_WARN),
            EX_R002_PARTIAL
        );
    }

    #[test]
    fn r002_beats_ok() {
        assert_eq!(merge_exit_code(EX_OK, EX_R002_PARTIAL), EX_R002_PARTIAL);
    }

    #[test]
    fn reduction_is_associative_on_three_codes() {
        // Batch fold property: `(a |> b) |> c` must equal `a |> (b |> c)`
        // so per-row order does not change the batch exit code.
        let codes = [EX_OK, EX_DIAG_WARN, EX_DIAG_ERROR, EX_R002_PARTIAL];
        for &a in &codes {
            for &b in &codes {
                for &c in &codes {
                    let left = merge_exit_code(merge_exit_code(a, b), c);
                    let right = merge_exit_code(a, merge_exit_code(b, c));
                    assert_eq!(
                        left, right,
                        "merge_exit_code must be associative; \
                         got {left} vs {right} for ({a}, {b}, {c})"
                    );
                }
            }
        }
    }

    #[test]
    fn reduction_is_commutative() {
        let codes = [EX_OK, EX_DIAG_WARN, EX_DIAG_ERROR, EX_R002_PARTIAL];
        for &a in &codes {
            for &b in &codes {
                assert_eq!(
                    merge_exit_code(a, b),
                    merge_exit_code(b, a),
                    "merge_exit_code must be commutative; got mismatch for ({a}, {b})"
                );
            }
        }
    }
}
