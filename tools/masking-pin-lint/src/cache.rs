// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Cache read/write for the `cache-with-fallback` strategy (D11).
//!
//! Cache files live at `<cache_dir>/<owner>__<repo>__<NNN>.json` and follow
//! the schema documented at `cache/SCHEMA.md`.
//!
//! Reads return `Option<CachedIssueState>` (`None` on missing file). Writes
//! are atomic via temp-file + rename.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Pinned schema identifier. Future schema bumps require coordinated CI rollout.
pub const CACHE_SCHEMA: &str = "marque-masking-pin-cache-1.0";

/// On-disk cached issue state. Mirrors the JSON schema 1:1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CachedIssueState {
    /// Pinned to `marque-masking-pin-cache-1.0`.
    pub schema: String,
    /// `<owner>/<name>`.
    pub repo: String,
    /// The starting issue number (the one referenced by the pin marker).
    pub issue_number: u32,
    /// `"open"` or `"closed"`.
    pub state: String,
    /// ISO-8601 timestamp of terminal close, or `None` if open.
    pub closed_at: Option<DateTime<Utc>>,
    /// Issue number this duplicates, if any.
    pub closed_as_duplicate_of: Option<u32>,
    /// Time of last fresh API write.
    pub refreshed_at: DateTime<Utc>,
    /// Ordered chain of `closed_as_duplicate_of` traversal, starting with
    /// `issue_number`.
    pub chain: Vec<u32>,
}

/// Build the canonical cache path for a `(owner, repo, issue)` triple.
#[must_use]
pub fn cache_path(cache_dir: &Path, owner: &str, repo: &str, issue: u32) -> PathBuf {
    cache_dir.join(format!("{owner}__{repo}__{issue}.json"))
}

/// Read a cache entry; return `None` on missing file.
///
/// Returns `Err` only on filesystem read failure or malformed JSON.
pub fn read_cache(
    cache_dir: &Path,
    owner: &str,
    repo: &str,
    issue: u32,
) -> Result<Option<CachedIssueState>> {
    let path = cache_path(cache_dir, owner, repo, issue);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path)
        .with_context(|| format!("reading cache file {}", path.display()))?;
    let parsed: CachedIssueState = serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing cache file {}", path.display()))?;
    if parsed.schema != CACHE_SCHEMA {
        anyhow::bail!(
            "cache file {} has schema {:?}, expected {:?}",
            path.display(),
            parsed.schema,
            CACHE_SCHEMA
        );
    }
    Ok(Some(parsed))
}

/// Write a cache entry atomically (temp file + rename).
pub fn write_cache(cache_dir: &Path, entry: &CachedIssueState) -> Result<()> {
    fs::create_dir_all(cache_dir)
        .with_context(|| format!("creating cache dir {}", cache_dir.display()))?;
    let final_path = cache_path(
        cache_dir,
        entry.repo.split('/').next().unwrap_or(""),
        entry.repo.split('/').nth(1).unwrap_or(""),
        entry.issue_number,
    );
    let temp_path = final_path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(entry)
        .context("serializing cache entry")?;
    {
        let mut f = fs::File::create(&temp_path)
            .with_context(|| format!("creating temp cache file {}", temp_path.display()))?;
        f.write_all(&json)
            .with_context(|| format!("writing temp cache file {}", temp_path.display()))?;
        f.sync_all().ok();
    }
    fs::rename(&temp_path, &final_path)
        .with_context(|| format!("renaming temp cache to {}", final_path.display()))?;
    Ok(())
}

/// Time elapsed since a cache entry was last refreshed.
#[must_use]
pub fn cache_age(entry: &CachedIssueState) -> chrono::Duration {
    Utc::now().signed_duration_since(entry.refreshed_at)
}

/// Whether the cache age exceeds the 24-hour staleness threshold.
#[must_use]
pub fn is_stale(entry: &CachedIssueState) -> bool {
    cache_age(entry) >= chrono::Duration::hours(24)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fixture(issue: u32) -> CachedIssueState {
        CachedIssueState {
            schema: CACHE_SCHEMA.to_string(),
            repo: "marquetools/marque".to_string(),
            issue_number: issue,
            state: "open".to_string(),
            closed_at: None,
            closed_as_duplicate_of: None,
            refreshed_at: Utc::now(),
            chain: vec![issue],
        }
    }

    #[test]
    fn read_returns_none_on_missing() {
        let dir = TempDir::new().unwrap();
        let result = read_cache(dir.path(), "marquetools", "marque", 999).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn write_then_read_roundtrip() {
        let dir = TempDir::new().unwrap();
        let entry = fixture(257);
        write_cache(dir.path(), &entry).unwrap();
        let got = read_cache(dir.path(), "marquetools", "marque", 257)
            .unwrap()
            .expect("cache entry present");
        assert_eq!(got.issue_number, 257);
        assert_eq!(got.state, "open");
        assert_eq!(got.schema, CACHE_SCHEMA);
    }

    #[test]
    fn read_rejects_unknown_schema() {
        let dir = TempDir::new().unwrap();
        let mut entry = fixture(257);
        entry.schema = "not-the-real-schema".to_string();
        // Write directly bypassing the canonical writer.
        let path = cache_path(dir.path(), "marquetools", "marque", 257);
        fs::write(&path, serde_json::to_vec(&entry).unwrap()).unwrap();
        let result = read_cache(dir.path(), "marquetools", "marque", 257);
        assert!(result.is_err());
    }

    #[test]
    fn fresh_cache_not_stale() {
        let entry = fixture(1);
        assert!(!is_stale(&entry));
    }

    #[test]
    fn old_cache_is_stale() {
        let mut entry = fixture(1);
        entry.refreshed_at = Utc::now() - chrono::Duration::hours(25);
        assert!(is_stale(&entry));
    }
}
