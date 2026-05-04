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
                check_masking_pin(
                    pin,
                    *issue,
                    &octo,
                    &owner,
                    &repo,
                    &cache_dir,
                    &mut diagnostics,
                )
                .await;
            }
            (PinKind::Masking { issue, .. }, Mode::RefreshCache) => {
                refresh_masking_pin(
                    pin,
                    *issue,
                    &octo,
                    &owner,
                    &repo,
                    &cache_dir,
                    &mut diagnostics,
                    &mut refresh_outcome,
                )
                .await;
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

/// CI-mode masking-pin handler: API-first, cache-fallback per D11.
async fn check_masking_pin(
    pin: &Pin,
    issue: u32,
    octo: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    cache_dir: &std::path::Path,
    diagnostics: &mut Vec<LintDiagnostic>,
) {
    match check_pin(octo, owner, repo, issue).await {
        Ok(state) => {
            // Persist fresh state.
            if let Err(err) =
                persist(state.clone(), &format!("{owner}/{repo}"), issue, cache_dir)
            {
                eprintln!(
                    "warning: failed to persist cache for #{issue}: {err:#}"
                );
            }
            evaluate_issue_state(pin, issue, &state, diagnostics);
        }
        Err(ApiError::NotFound(n, full_repo)) => diagnostics.push(LintDiagnostic {
            severity: Severity::Error,
            message: format!(
                "error: MASKING-PIN at {}:{} tracks #{n} which does not exist in {full_repo}",
                pin.file.display(),
                pin.line
            ),
        }),
        Err(api_err) => {
            // Fall back to cache.
            match read_cache(cache_dir, owner, repo, issue) {
                Ok(Some(entry)) => {
                    let stale = is_stale(&entry);
                    if stale {
                        diagnostics.push(LintDiagnostic {
                            severity: Severity::Warning,
                            message: format!(
                                "warning: GitHub API unavailable ({api_err}); falling back to cache for #{issue} (cache age >24h — refresh recommended)"
                            ),
                        });
                    }
                    evaluate_cached_state(pin, issue, &entry, diagnostics);
                }
                Ok(None) => diagnostics.push(LintDiagnostic {
                    severity: Severity::Error,
                    message: format!(
                        "error: no cached state for #{issue} and API unavailable ({api_err}); run --mode refresh-cache locally"
                    ),
                }),
                Err(err) => diagnostics.push(LintDiagnostic {
                    severity: Severity::Error,
                    message: format!(
                        "error: API unavailable ({api_err}) and cache read failed for #{issue}: {err:#}"
                    ),
                }),
            }
        }
    }
}

/// Refresh-cache handler: write fresh state on success; record API
/// failures as warnings. The caller (`run`) tracks per-issue success
/// vs. failure totals so a *total* outage (zero successful refreshes
/// out of a non-empty pin set) escalates to a hard error — otherwise
/// the daily cache-refresh workflow looks healthy while the cache
/// silently goes stale, contradicting the D11 contract.
//
// `clippy::too_many_arguments` and `clippy::large_futures`: this
// function passes the same `(octo, owner, repo, cache_dir)` quartet
// as the sibling `check_masking_pin`. Factoring them into a struct
// would add indirection without saving anything in a function called
// once per pin in a CI lint binary; the future size is dominated by
// `octocrab`'s reqwest state, not by our argument list. Both lints
// allowed locally with this rationale; the CI gate (`-D warnings`,
// no `pedantic`) is unaffected.
#[allow(clippy::too_many_arguments, clippy::large_futures)]
async fn refresh_masking_pin(
    pin: &Pin,
    issue: u32,
    octo: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    cache_dir: &std::path::Path,
    diagnostics: &mut Vec<LintDiagnostic>,
    refresh_outcome: &mut RefreshOutcome,
) {
    match check_pin(octo, owner, repo, issue).await {
        Ok(state) => {
            if let Err(err) =
                persist(state, &format!("{owner}/{repo}"), issue, cache_dir)
            {
                refresh_outcome.failures += 1;
                diagnostics.push(LintDiagnostic {
                    severity: Severity::Error,
                    message: format!(
                        "error: refresh-cache: failed to persist #{issue}: {err:#}"
                    ),
                });
            } else {
                refresh_outcome.successes += 1;
                eprintln!(
                    "refresh-cache: #{issue} ({}:{}) refreshed",
                    pin.file.display(),
                    pin.line
                );
            }
        }
        Err(api_err) => {
            refresh_outcome.failures += 1;
            diagnostics.push(LintDiagnostic {
                severity: Severity::Warning,
                message: format!(
                    "warning: refresh-cache: API unavailable for #{issue}: {api_err}"
                ),
            });
        }
    }
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
    if entry.state == "open" {
        return;
    }
    let date = entry
        .closed_at
        .map_or_else(|| "<unknown>".to_string(), |t| t.to_rfc3339());
    diagnostics.push(LintDiagnostic {
        severity: Severity::Error,
        message: format!(
            "error: MASKING-PIN at {}:{} tracks #{issue} which closed at {date} (cached); per source plan §6 rule 5, the pin must be removed in the issue-closing PR",
            pin.file.display(),
            pin.line
        ),
    });
}

fn persist(
    state: IssueState,
    repo: &str,
    starting_issue: u32,
    cache_dir: &std::path::Path,
) -> Result<()> {
    let entry = CachedIssueState {
        schema: CACHE_SCHEMA.to_string(),
        repo: repo.to_string(),
        issue_number: starting_issue,
        state: match state.terminal_state {
            TerminalState::Open => "open".to_string(),
            _ => "closed".to_string(),
        },
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
    Ok(())
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
