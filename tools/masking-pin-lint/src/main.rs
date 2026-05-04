// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! `masking-pin-lint` — CI lint enforcing FR-039 masking-pin discipline.
//!
//! See `README.md` for design and `--help` for invocation.

#![deny(rust_2018_idioms)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Parser, ValueEnum};

use masking_pin_lint::cache::{
    cache_path, is_stale, read_cache, write_cache, CachedIssueState, CACHE_SCHEMA,
};
use masking_pin_lint::github::{check_pin, ApiError, IssueState, TerminalState};
use masking_pin_lint::pin::{LintDiagnostic, Pin, PinKind, Severity};
use masking_pin_lint::scanner::scan_workspace;

#[derive(Debug, Parser)]
#[command(
    name = "masking-pin-lint",
    about = "AST-based lint enforcing FR-039 masking-pin discipline",
    version
)]
struct Cli {
    /// Path to the marque workspace root.
    #[arg(long)]
    workspace_dir: PathBuf,

    /// Operating mode.
    #[arg(long, value_enum, default_value_t = Mode::Ci)]
    mode: Mode,

    /// Cache directory. Defaults to `<workspace>/tools/masking-pin-lint/cache`.
    #[arg(long)]
    cache_dir: Option<PathBuf>,

    /// GitHub personal access token. Falls back to `GITHUB_TOKEN` env var.
    #[arg(long, env = "GITHUB_TOKEN")]
    github_token: Option<String>,

    /// Repo to query, in `<owner>/<name>` form.
    #[arg(long, default_value = "marquetools/marque")]
    repo: String,
}

/// Operating mode.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum Mode {
    /// Production CI mode: scan, validate markers, query API w/ cache fallback,
    /// fail on any error.
    Ci,
    /// Refresh-cache mode: query API for every tracked issue and write cache
    /// entries; exits zero unless the API itself is fully unreachable.
    RefreshCache,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    // `Box::pin` keeps the entry future off the (small) main task stack;
    // the run-loop builds an octocrab client whose state plus per-pin
    // futures push the inline future size just over clippy's 16 KB
    // pedantic threshold. Heap-allocating the entry once is free in a
    // CLI binary and avoids a noisy lint allow.
    match Box::pin(run(cli)).await {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(2)
        }
    }
}

// `run` is a long top-level orchestrator: it sets up paths, resolves
// unique tracked issues (the round-8 dedup pre-pass), iterates the
// scanned pin sites for per-pin diagnostic emission, and maps the
// final diagnostic severity vector to an exit code. Splitting the
// body further would require threading the same set of refs (octo,
// owner, repo, cache_dir, refresh_outcome, diagnostics) through 3-4
// helper signatures and would obscure the linear control flow more
// than it clarifies it. The function is one logical unit.
#[allow(clippy::too_many_lines)]
async fn run(cli: Cli) -> Result<ExitCode> {
    let workspace_dir = cli.workspace_dir.canonicalize().with_context(|| {
        format!(
            "resolving workspace dir {}",
            cli.workspace_dir.display()
        )
    })?;
    let cache_dir = cli.cache_dir.unwrap_or_else(|| {
        workspace_dir
            .join("tools")
            .join("masking-pin-lint")
            .join("cache")
    });
    let (owner, repo) = parse_repo(&cli.repo)?;

    let pins = scan_workspace(&workspace_dir).context("scanning workspace for pin sites")?;
    eprintln!("masking-pin-lint: scanned, found {} pin site(s)", pins.len());

    let octo = build_octocrab(cli.github_token.as_deref())?;

    let mut diagnostics: Vec<LintDiagnostic> = Vec::new();
    let mut refresh_outcome = RefreshOutcome::default();

    // Pre-resolve every unique tracked issue ONCE before the per-pin
    // dispatch loop. Multiple pins pointing at the same issue number
    // (e.g. two test files both masking #258) would otherwise hit the
    // GitHub API once per pin site and write the same cache file
    // repeatedly, burning rate-limit budget and flushing the same
    // state through the atomic-rename pipeline N times. Memoizing
    // here keeps API/cache work proportional to the number of UNIQUE
    // issues, not the number of pin sites.
    let mut unique_issues: Vec<u32> = pins
        .iter()
        .filter_map(|p| match &p.kind {
            PinKind::Masking { issue, .. } => Some(*issue),
            _ => None,
        })
        .collect();
    unique_issues.sort_unstable();
    unique_issues.dedup();

    let mut resolutions: std::collections::HashMap<u32, MemoOutcome> =
        std::collections::HashMap::new();
    for issue in unique_issues {
        let memo = resolve_issue(
            cli.mode,
            issue,
            &octo,
            &owner,
            &repo,
            &cache_dir,
            &mut refresh_outcome,
            &mut diagnostics,
        )
        .await;
        resolutions.insert(issue, memo);
    }

    for pin in &pins {
        match (&pin.kind, cli.mode) {
            (PinKind::Unmarked, _) => diagnostics.push(LintDiagnostic {
                severity: Severity::Error,
                message: format!(
                    "error: masking pin at {}:{}:{} requires either '// MASKING-PIN: tracks #NNN' or '// INTENTIONAL-STRICT: <reason>' within 5 lines (FR-039)",
                    pin.file.display(),
                    pin.line,
                    pin.column
                ),
            }),
            (PinKind::BothMarkers, _) => diagnostics.push(LintDiagnostic {
                severity: Severity::Error,
                message: format!(
                    "error: masking pin at {}:{}:{} carries both MASKING-PIN and INTENTIONAL-STRICT markers — pick one",
                    pin.file.display(),
                    pin.line,
                    pin.column
                ),
            }),
            (PinKind::BadFormat(line), _) => diagnostics.push(LintDiagnostic {
                severity: Severity::Error,
                message: format!(
                    "error: masking pin at {}:{}:{} has malformed marker comment: {}",
                    pin.file.display(),
                    pin.line,
                    pin.column,
                    line.trim()
                ),
            }),
            (PinKind::IntentionalStrict { .. }, _) => {
                // No API check required; marker syntax was validated by the scanner.
            }
            (PinKind::Masking { issue, .. }, Mode::Ci) => {
                let memo = resolutions
                    .get(issue)
                    .expect("unique-issue pre-pass populates every Masking issue");
                emit_ci_diagnostics_for_pin(pin, *issue, memo, &mut diagnostics);
            }
            (PinKind::Masking { issue, .. }, Mode::RefreshCache) => {
                let memo = resolutions
                    .get(issue)
                    .expect("unique-issue pre-pass populates every Masking issue");
                emit_refresh_diagnostics_for_pin(pin, *issue, memo, &mut diagnostics);
            }
        }
    }

    // Total-outage escalation for refresh-cache mode: if there were
    // tracked-issue pins to refresh but every one failed, the daily
    // workflow would otherwise look healthy while the cache silently
    // goes stale. Push an error diagnostic so the run fails loudly.
    if matches!(cli.mode, Mode::RefreshCache)
        && refresh_outcome.failures > 0
        && refresh_outcome.successes == 0
    {
        diagnostics.push(LintDiagnostic {
            severity: Severity::Error,
            message: format!(
                "error: refresh-cache: total outage — {} pin(s) attempted, 0 succeeded; cache will go stale (D11)",
                refresh_outcome.failures
            ),
        });
    }

    let mut had_error = false;
    for d in &diagnostics {
        eprintln!("{}", d.message);
        if d.severity == Severity::Error {
            had_error = true;
        }
    }

    // CI mode and RefreshCache mode use the same exit-code policy: a
    // hard error fails the run, anything else (warnings, silent success)
    // exits zero. RefreshCache routes API errors to warnings rather than
    // hard errors via `refresh_masking_pin`, so the policy correctly
    // distinguishes the two modes by which diagnostics get pushed,
    // not by how the codes are mapped here.
    Ok(if had_error { ExitCode::from(1) } else { ExitCode::SUCCESS })
}

/// Build the octocrab client. Authenticated calls when a token is provided.
fn build_octocrab(token: Option<&str>) -> Result<octocrab::Octocrab> {
    let mut builder = octocrab::Octocrab::builder();
    if let Some(t) = token {
        if !t.is_empty() {
            builder = builder.personal_token(t.to_string());
        }
    }
    builder.build().context("building octocrab client")
}

fn parse_repo(spec: &str) -> Result<(String, String)> {
    let mut parts = spec.splitn(2, '/');
    let owner = parts
        .next()
        .filter(|s| !s.is_empty())
        .with_context(|| format!("repo {spec:?} missing owner"))?;
    let repo = parts
        .next()
        .filter(|s| !s.is_empty())
        .with_context(|| format!("repo {spec:?} missing name"))?;
    Ok((owner.to_string(), repo.to_string()))
}

/// Resolution outcome for a single tracked issue, computed ONCE per
/// unique issue regardless of how many pin sites point at it. Multiple
/// pins sharing an issue number look up the same memo entry and emit
/// per-pin diagnostics from it without re-hitting the API or
/// re-persisting the cache.
enum MemoOutcome {
    /// API succeeded; cache-write outcome (Updated/Unchanged) and any
    /// cache-write error were already handled in `resolve_issue` at
    /// resolution time. The `IssueState` is shared by all pins that
    /// point at this issue.
    Resolved { state: IssueState },
    /// API returned 404 — the issue doesn't exist. Per-pin emission
    /// is a hard error.
    NotFound { full_repo: String },
    /// API failed with a non-404 error. CI mode falls back to cache;
    /// refresh-cache mode treats it as a per-pin warning. The
    /// fallback happens at resolution time so we read the cache
    /// once, not once per pin.
    ApiUnavailable {
        err: String,
        /// Cache fallback resolved at resolution time:
        /// - `Ok(Some(entry, stale))`: cache hit; emit per-pin
        ///   evaluation against the entry, plus a stale warning if
        ///   `stale`.
        /// - `Ok(None)`: cache miss; per-pin error.
        /// - `Err(cache_read_err)`: cache I/O failed; per-pin error.
        fallback: Result<Option<(CachedIssueState, bool)>, String>,
    },
}

/// Resolve a single tracked issue ONCE for the whole run.
///
/// `Mode::Ci`: API call, persist on success, fall back to cache on error.
/// `Mode::RefreshCache`: API call, persist on success, no fallback (the
/// purpose of this mode IS to refresh the cache, so a cache fallback
/// would be circular).
///
/// Run-level `refresh_outcome` counters and the cache-bootstrap
/// per-issue eprintln logs are updated here at resolution time —
/// they're per-issue concerns, not per-pin, and emitting them at the
/// per-pin level would also produce N copies for N pins sharing an
/// issue.
#[allow(clippy::too_many_arguments, clippy::large_futures)]
async fn resolve_issue(
    mode: Mode,
    issue: u32,
    octo: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    cache_dir: &std::path::Path,
    refresh_outcome: &mut RefreshOutcome,
    diagnostics: &mut Vec<LintDiagnostic>,
) -> MemoOutcome {
    match check_pin(octo, owner, repo, issue).await {
        Ok(state) => {
            // Persist (once per issue). `persist` is a no-op when the
            // cached state-of-record fields match the live state, so
            // refresh-cache runs that confirm an unchanged issue
            // produce no `git diff` and the daily workflow can no-op.
            // Per-mode side effects (eprintln logs, refresh-outcome
            // counters, refresh-mode persist-error diagnostics)
            // happen ONCE per issue here, not per pin site.
            match (mode, persist(state.clone(), &format!("{owner}/{repo}"), issue, cache_dir)) {
                (Mode::Ci, Err(err)) => {
                    eprintln!("warning: failed to persist cache for #{issue}: {err:#}");
                }
                (Mode::RefreshCache, Err(err)) => {
                    refresh_outcome.failures += 1;
                    diagnostics.push(LintDiagnostic {
                        severity: Severity::Error,
                        message: format!(
                            "error: refresh-cache: failed to persist #{issue}: {err:#}"
                        ),
                    });
                }
                (Mode::RefreshCache, Ok(WriteOutcome::Updated)) => {
                    refresh_outcome.successes += 1;
                    eprintln!("refresh-cache: #{issue} refreshed");
                }
                (Mode::RefreshCache, Ok(WriteOutcome::Unchanged)) => {
                    refresh_outcome.successes += 1;
                    eprintln!("refresh-cache: #{issue} unchanged");
                }
                (Mode::Ci, Ok(_)) => {}
            }
            MemoOutcome::Resolved { state }
        }
        Err(ApiError::NotFound(_, full_repo)) => {
            MemoOutcome::NotFound { full_repo }
        }
        Err(api_err) => {
            let err_str = format!("{api_err}");
            // CI mode falls back to cache; refresh-cache mode does NOT
            // (refresh's purpose IS the API call).
            let fallback = match mode {
                Mode::Ci => match read_cache(cache_dir, owner, repo, issue) {
                    Ok(Some(entry)) => {
                        let stale = is_stale(&entry);
                        Ok(Some((entry, stale)))
                    }
                    Ok(None) => Ok(None),
                    Err(err) => Err(format!("{err:#}")),
                },
                Mode::RefreshCache => {
                    refresh_outcome.failures += 1;
                    diagnostics.push(LintDiagnostic {
                        severity: Severity::Warning,
                        message: format!(
                            "warning: refresh-cache: API unavailable for #{issue}: {api_err}"
                        ),
                    });
                    // No fallback in refresh-cache mode; the per-pin
                    // emission shouldn't try to read cache.
                    Ok(None)
                }
            };
            MemoOutcome::ApiUnavailable {
                err: err_str,
                fallback,
            }
        }
    }
}

/// Per-pin CI-mode emission given a pre-resolved issue memo. Called
/// once per pin site (so a file/line is reported per pin), but the
/// underlying API/cache work happened ONCE in `resolve_issue`.
fn emit_ci_diagnostics_for_pin(
    pin: &Pin,
    issue: u32,
    memo: &MemoOutcome,
    diagnostics: &mut Vec<LintDiagnostic>,
) {
    match memo {
        MemoOutcome::Resolved { state } => {
            evaluate_issue_state(pin, issue, state, diagnostics);
        }
        MemoOutcome::NotFound { full_repo } => {
            diagnostics.push(LintDiagnostic {
                severity: Severity::Error,
                message: format!(
                    "error: MASKING-PIN at {}:{} tracks #{issue} which does not exist in {full_repo}",
                    pin.file.display(),
                    pin.line
                ),
            });
        }
        MemoOutcome::ApiUnavailable { err, fallback } => match fallback {
            Ok(Some((entry, stale))) => {
                if *stale {
                    diagnostics.push(LintDiagnostic {
                        severity: Severity::Warning,
                        message: format!(
                            "warning: GitHub API unavailable ({err}); falling back to cache for #{issue} (cache age >24h — refresh recommended)"
                        ),
                    });
                }
                evaluate_cached_state(pin, issue, entry, diagnostics);
            }
            Ok(None) => {
                diagnostics.push(LintDiagnostic {
                    severity: Severity::Error,
                    message: format!(
                        "error: no cached state for #{issue} and API unavailable ({err}); run --mode refresh-cache locally"
                    ),
                });
            }
            Err(cache_err) => {
                diagnostics.push(LintDiagnostic {
                    severity: Severity::Error,
                    message: format!(
                        "error: API unavailable ({err}) and cache read failed for #{issue}: {cache_err}"
                    ),
                });
            }
        },
    }
}

/// Per-pin refresh-cache-mode emission given a pre-resolved issue
/// memo. Run-level success/failure counts and per-issue eprintln
/// logs are NOT emitted here — they're per-issue concerns and were
/// already handled in `resolve_issue`. This function only emits
/// per-pin diagnostics that should appear once per pin site.
///
/// Currently a no-op: all refresh-cache diagnostics are per-issue.
/// Defined explicitly (rather than inlined as a no-op in the caller)
/// so a future per-pin diagnostic can be added without restructuring
/// the dispatch site.
fn emit_refresh_diagnostics_for_pin(
    _pin: &Pin,
    _issue: u32,
    _memo: &MemoOutcome,
    _diagnostics: &mut [LintDiagnostic],
) {
}

/// Per-run counters for the `RefreshCache` mode so the caller can
/// distinguish "individual pin failed, others succeeded" (warning)
/// from "everything failed" (error: cache will go stale).
#[derive(Debug, Default)]
struct RefreshOutcome {
    successes: u32,
    failures: u32,
}

fn evaluate_issue_state(
    pin: &Pin,
    issue: u32,
    state: &IssueState,
    diagnostics: &mut Vec<LintDiagnostic>,
) {
    match state.terminal_state {
        TerminalState::Open => {
            // Silent success.
        }
        TerminalState::Cycle => diagnostics.push(LintDiagnostic {
            severity: Severity::Error,
            message: format!(
                "error: cycle in closed_as_duplicate_of chain at #{} — manual review required (chain: {:?})",
                state.final_issue, state.chain
            ),
        }),
        TerminalState::ClosedAsCompleted | TerminalState::ClosedNotDuplicate => {
            let date = state
                .closed_at
                .map_or_else(|| "<unknown>".to_string(), |t| t.to_rfc3339());
            diagnostics.push(LintDiagnostic {
                severity: Severity::Error,
                message: format!(
                    "error: MASKING-PIN at {}:{} tracks #{issue} which closed at {date}; per source plan §6 rule 5, the pin must be removed in the issue-closing PR (chain: {:?})",
                    pin.file.display(),
                    pin.line,
                    state.chain
                ),
            });
        }
    }
    if state.meta_issue_warning {
        diagnostics.push(LintDiagnostic {
            severity: Severity::Warning,
            message: format!(
                "warning: MASKING-PIN at {}:{} chain terminates at meta/tracking issue #{} — reviewer should confirm cascade-close is appropriate (FR-039 rule 4)",
                pin.file.display(),
                pin.line,
                state.final_issue
            ),
        });
    }
}

fn evaluate_cached_state(
    pin: &Pin,
    issue: u32,
    entry: &CachedIssueState,
    diagnostics: &mut Vec<LintDiagnostic>,
) {
    // Schema 1.1 preserves the structured `TerminalState` + `meta_issue_warning`
    // alongside the coarse `state` string, so the cache-fallback path
    // produces byte-identical diagnostics to the API-available path:
    // a cycle-classified issue surfaces the cycle warning, a
    // chain-of-meta-issues surfaces the cascade-close warning, and so
    // on. The pre-1.1 fallback collapsed every non-open state into
    // "issue is closed" with no distinction.
    match entry.terminal_state.as_str() {
        "Open" => {
            // Nothing to flag for the open case directly. The
            // chain-meta-issue warning still applies if any issue in
            // the duplicate chain looked like a meta-issue.
        }
        "Cycle" => {
            diagnostics.push(LintDiagnostic {
                severity: Severity::Error,
                message: format!(
                    "error: cycle in closed_as_duplicate_of chain at #{issue} — manual review required (cached; chain: {:?})",
                    entry.chain
                ),
            });
        }
        "ClosedAsCompleted" | "ClosedNotDuplicate" => {
            let date = entry
                .closed_at
                .map_or_else(|| "<unknown>".to_string(), |t| t.to_rfc3339());
            diagnostics.push(LintDiagnostic {
                severity: Severity::Error,
                message: format!(
                    "error: MASKING-PIN at {}:{} tracks #{issue} which closed at {date} (cached); per source plan §6 rule 5, the pin must be removed in the issue-closing PR (chain: {:?})",
                    pin.file.display(),
                    pin.line,
                    entry.chain
                ),
            });
        }
        other => {
            // Unknown terminal_state value (forward-compat: a future
            // schema bump might add new variants). Fail loudly rather
            // than silently misclassifying as open.
            diagnostics.push(LintDiagnostic {
                severity: Severity::Error,
                message: format!(
                    "error: MASKING-PIN at {}:{} tracks #{issue} with unknown cached terminal_state {other:?}; refresh the cache",
                    pin.file.display(),
                    pin.line
                ),
            });
        }
    }
    if entry.meta_issue_warning {
        diagnostics.push(LintDiagnostic {
            severity: Severity::Warning,
            message: format!(
                "warning: MASKING-PIN at {}:{} chain visited a meta/tracking issue (cached) — reviewer should confirm cascade-close is appropriate (FR-039 rule 4)",
                pin.file.display(),
                pin.line
            ),
        });
    }
}

/// Persist `state` for `starting_issue` to the cache directory.
///
/// **No-op if the persisted state-of-record fields are unchanged**.
/// The `refreshed_at` timestamp is intentionally NOT part of the
/// state-of-record — only `state`, `closed_at`, `closed_as_duplicate_of`,
/// and `chain` are. If the daily refresh observes that GitHub still
/// reports the same terminal state for an issue, we leave the
/// existing cache file alone so the scheduled-refresh workflow's
/// `git diff` produces no churn and no PR is opened.
///
/// On state change, the function writes the new entry with a fresh
/// `refreshed_at` and returns `WriteOutcome::Updated`. On no-op, it
/// returns `WriteOutcome::Unchanged`.
fn persist(
    state: IssueState,
    repo: &str,
    starting_issue: u32,
    cache_dir: &std::path::Path,
) -> Result<WriteOutcome> {
    let new_state_str = match state.terminal_state {
        TerminalState::Open => "open".to_string(),
        _ => "closed".to_string(),
    };
    // Structured terminal-state string mirrors the enum's Debug form
    // so the cache file's `terminal_state` field can be matched
    // unambiguously in `evaluate_cached_state` even on API outage.
    let new_terminal_state_str = match state.terminal_state {
        TerminalState::Open => "Open",
        TerminalState::ClosedAsCompleted => "ClosedAsCompleted",
        TerminalState::ClosedNotDuplicate => "ClosedNotDuplicate",
        TerminalState::Cycle => "Cycle",
    }
    .to_string();

    let parts: Vec<&str> = repo.splitn(2, '/').collect();
    if parts.len() == 2 {
        // Compare against the existing entry (if any) to suppress
        // `refreshed_at`-only churn. The state-of-record fields now
        // include `terminal_state` and `meta_issue_warning` (schema
        // 1.1) so a no-op write requires the structured terminal
        // classification AND the chain-meta-issue flag to also match.
        if let Ok(Some(existing)) =
            read_cache(cache_dir, parts[0], parts[1], starting_issue)
        {
            if existing.repo == repo
                && existing.issue_number == starting_issue
                && existing.state == new_state_str
                && existing.terminal_state == new_terminal_state_str
                && existing.meta_issue_warning == state.meta_issue_warning
                && existing.closed_at == state.closed_at
                && existing.closed_as_duplicate_of == state.closed_as_duplicate_of
                && existing.chain == state.chain
            {
                return Ok(WriteOutcome::Unchanged);
            }
        }
    }

    let entry = CachedIssueState {
        schema: CACHE_SCHEMA.to_string(),
        repo: repo.to_string(),
        issue_number: starting_issue,
        state: new_state_str,
        terminal_state: new_terminal_state_str,
        meta_issue_warning: state.meta_issue_warning,
        closed_at: state.closed_at,
        closed_as_duplicate_of: state.closed_as_duplicate_of,
        refreshed_at: Utc::now(),
        chain: state.chain,
    };
    write_cache(cache_dir, &entry)?;
    // Touch path lookup as a sanity check (also surfaces atomic-rename failures).
    let parts: Vec<&str> = repo.splitn(2, '/').collect();
    if parts.len() == 2 {
        let _ = cache_path(cache_dir, parts[0], parts[1], starting_issue);
    }
    Ok(WriteOutcome::Updated)
}

/// Outcome of a `persist` call. `Unchanged` means the cache file's
/// state-of-record fields already matched the freshly-fetched state,
/// so no write was performed (suppresses daily-refresh PR churn).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WriteOutcome {
    Updated,
    Unchanged,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_repo_ok() {
        let (o, r) = parse_repo("marquetools/marque").unwrap();
        assert_eq!(o, "marquetools");
        assert_eq!(r, "marque");
    }

    #[test]
    fn parse_repo_missing_owner() {
        assert!(parse_repo("/marque").is_err());
    }

    #[test]
    fn parse_repo_missing_name() {
        assert!(parse_repo("marquetools/").is_err());
    }
}
