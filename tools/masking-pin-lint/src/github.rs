// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! GitHub-API check + `closed_as_duplicate_of` chain following.
//!
//! Per FR-039 rule 4, the lint follows the duplicate-of chain until terminal
//! close. Cycles are detected via a visited-set and reported as errors.
//!
//! Network errors (timeout / rate-limit / I/O) are returned as
//! [`ApiError::Unavailable`]; the caller falls back to the cache layer.

use std::collections::HashSet;
use std::time::Duration;

use chrono::{DateTime, Utc};
use thiserror::Error;
use tokio::time::timeout;

/// Per-call API timeout per D11.
pub const API_TIMEOUT: Duration = Duration::from_secs(5);

/// Resolved state of an issue chain.
#[derive(Debug, Clone)]
pub struct IssueState {
    /// Final issue number after following any `closed_as_duplicate_of` chain.
    pub final_issue: u32,
    /// Terminal-state classification.
    pub terminal_state: TerminalState,
    /// Ordered chain of issue numbers visited.
    pub chain: Vec<u32>,
    /// True if the terminal issue title looks like a tracking/meta issue
    /// (FR-039 rule 4 cascade-close-via-meta-issue flag).
    pub meta_issue_warning: bool,
    /// ISO-8601 close timestamp for the terminal issue, if closed.
    pub closed_at: Option<DateTime<Utc>>,
    /// Issue number the terminal issue duplicates, if any (informational —
    /// the authoritative path is `chain`).
    pub closed_as_duplicate_of: Option<u32>,
}

/// Classification of the chain's terminal state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalState {
    /// The terminal issue is open.
    Open,
    /// Closed not as a duplicate (the typical `state_reason: "completed"`).
    ClosedNotDuplicate,
    /// Closed with a duplicate-of pointer that itself is also closed and not
    /// a duplicate.
    ClosedAsCompleted,
    /// `closed_as_duplicate_of` chain revisited a previously-seen issue.
    Cycle,
}

/// Errors from the GitHub API path.
#[derive(Debug, Error)]
pub enum ApiError {
    /// API call failed for a reason that the cache layer should compensate for
    /// (timeout, 5xx, network, rate-limit). Caller falls back to the cache.
    #[error("github API unavailable: {0}")]
    Unavailable(String),
    /// API returned a definitive 404 (issue does not exist).
    #[error("issue #{0} not found in {1}")]
    NotFound(u32, String),
    /// Catch-all for unexpected API errors that are not recoverable via cache.
    #[error("github API error: {0}")]
    Other(String),
}

/// Walk the `closed_as_duplicate_of` chain from `issue` until a terminal state.
///
/// Wraps each API call in a 5-second timeout; on timeout/rate-limit/network
/// error returns [`ApiError::Unavailable`] so the caller can fall back to the
/// cache.
pub async fn check_pin(
    octo: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    issue: u32,
) -> Result<IssueState, ApiError> {
    let mut visited: HashSet<u32> = HashSet::new();
    let mut chain: Vec<u32> = Vec::new();
    let mut current = issue;
    loop {
        if !visited.insert(current) {
            // Cycle.
            return Ok(IssueState {
                final_issue: current,
                terminal_state: TerminalState::Cycle,
                chain,
                meta_issue_warning: false,
                closed_at: None,
                closed_as_duplicate_of: None,
            });
        }
        chain.push(current);
        let issue_data = fetch_issue(octo, owner, repo, current).await?;
        let title_meta = title_looks_like_meta(&issue_data.title);
        match (issue_data.state.as_str(), issue_data.closed_as_duplicate_of) {
            ("open", _) => {
                return Ok(IssueState {
                    final_issue: current,
                    terminal_state: TerminalState::Open,
                    chain,
                    meta_issue_warning: title_meta,
                    closed_at: None,
                    closed_as_duplicate_of: None,
                });
            }
            ("closed", Some(next)) => {
                // Continue the chain.
                current = next;
            }
            ("closed", None) => {
                let terminal = if issue_data.state_reason.as_deref() == Some("completed") {
                    TerminalState::ClosedAsCompleted
                } else {
                    TerminalState::ClosedNotDuplicate
                };
                return Ok(IssueState {
                    final_issue: current,
                    terminal_state: terminal,
                    chain,
                    meta_issue_warning: title_meta,
                    closed_at: issue_data.closed_at,
                    closed_as_duplicate_of: None,
                });
            }
            (other, _) => {
                return Err(ApiError::Other(format!(
                    "unknown issue state {other:?} for #{current}"
                )));
            }
        }
    }
}

/// Internal projection of the fields the lint cares about.
#[derive(Debug, Clone)]
struct IssueProjection {
    title: String,
    state: String,
    state_reason: Option<String>,
    closed_at: Option<DateTime<Utc>>,
    closed_as_duplicate_of: Option<u32>,
}

async fn fetch_issue(
    octo: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    issue: u32,
) -> Result<IssueProjection, ApiError> {
    let issues = octo.issues(owner, repo);
    let fut = issues.get(u64::from(issue));
    let outcome = timeout(API_TIMEOUT, fut).await.map_err(|_| {
        ApiError::Unavailable(format!(
            "timeout after {}s fetching issue #{issue}",
            API_TIMEOUT.as_secs()
        ))
    })?;
    let issue_data = match outcome {
        Ok(i) => i,
        Err(err) => {
            // Map octocrab errors. 404 is definitive; other status codes
            // (rate-limit, 5xx, network) are recoverable via cache.
            let msg = err.to_string();
            if msg.contains("404") || msg.contains("Not Found") {
                return Err(ApiError::NotFound(issue, format!("{owner}/{repo}")));
            }
            return Err(ApiError::Unavailable(msg));
        }
    };
    let state = format!("{:?}", issue_data.state).to_lowercase();
    // Extract `closed_as_duplicate_of` from the timeline events. The
    // octocrab `Issue` struct does not directly expose the duplicate target
    // on stable, so fetch the timeline as a generic JSON value.
    //
    // Skip the timeline call when the issue is open: the `closed_as_duplicate_of`
    // field is only meaningful for closed issues, and a transient timeout
    // or rate-limit on the secondary call would otherwise convert a
    // perfectly clean "issue is open" probe into a cache-fallback or hard
    // failure for no semantic gain. Open issues cannot be duplicates by
    // GitHub's own data model.
    let dup_target = if state == "open" {
        None
    } else {
        fetch_duplicate_target(octo, owner, repo, issue).await?
    };
    Ok(IssueProjection {
        title: issue_data.title,
        state,
        state_reason: issue_data.state_reason.map(|r| format!("{r:?}").to_lowercase()),
        closed_at: issue_data.closed_at,
        closed_as_duplicate_of: dup_target,
    })
}

/// Look at the issue's timeline events for a `marked_as_duplicate` event whose
/// canonical issue is reachable. Returns `None` if no such event exists.
///
/// This call is wrapped in the same 5-second timeout as the issue fetch.
async fn fetch_duplicate_target(
    octo: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    issue: u32,
) -> Result<Option<u32>, ApiError> {
    let route = format!("/repos/{owner}/{repo}/issues/{issue}/timeline");
    let fut = octo.get::<serde_json::Value, _, ()>(route, None);
    let outcome = timeout(API_TIMEOUT, fut).await.map_err(|_| {
        ApiError::Unavailable(format!(
            "timeout after {}s fetching timeline for #{issue}",
            API_TIMEOUT.as_secs()
        ))
    })?;
    let events = match outcome {
        Ok(v) => v,
        Err(err) => {
            // If the timeline endpoint is unavailable, treat as "no duplicate
            // event found" rather than escalating; the issue's `state` already
            // tells us closed/open.
            return Err(ApiError::Unavailable(err.to_string()));
        }
    };
    let Some(arr) = events.as_array() else {
        return Ok(None);
    };
    for ev in arr {
        let event = ev.get("event").and_then(|v| v.as_str()).unwrap_or("");
        if event == "marked_as_duplicate" || event == "duplicated_by" {
            // The canonical issue's number lives at
            // `dupe.canonical.issue.number` for `marked_as_duplicate` events.
            if let Some(num) = ev
                .pointer("/dupe/canonical/issue/number")
                .or_else(|| ev.pointer("/canonical/number"))
                .and_then(serde_json::Value::as_u64)
            {
                return Ok(u32::try_from(num).ok());
            }
        }
    }
    Ok(None)
}

/// Heuristic for FR-039 rule 4 cascade-close-via-meta-issue: a title prefixed
/// `[meta]` or containing the word "tracking" (case-insensitive).
fn title_looks_like_meta(title: &str) -> bool {
    let lower = title.to_lowercase();
    lower.starts_with("[meta]") || lower.contains("tracking")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_title_brackets() {
        assert!(title_looks_like_meta("[meta] roll-up issue"));
        assert!(title_looks_like_meta("[META] case-insensitive"));
    }

    #[test]
    fn meta_title_tracking_keyword() {
        assert!(title_looks_like_meta("Tracking issue for refactor"));
        assert!(title_looks_like_meta("TRACKING: phase-D rollout"));
    }

    #[test]
    fn ordinary_title_not_meta() {
        assert!(!title_looks_like_meta("Decoder canonicalization leaks input bytes"));
        assert!(!title_looks_like_meta("E001 false positive on edge case"));
    }
}
