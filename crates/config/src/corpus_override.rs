// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Corpus-override parsing.
//!
//! Operators with their own non-public corpora can supply a JSON file
//! that **annotates** decoder fixes with the
//! [`marque_rules::FeatureId::CorpusOverrideInEffect`] audit marker. The override
//! surface is opt-in (Cargo feature `corpus-override`), CLI-only
//! (the server rejects it on every channel; WASM cannot enable the
//! feature), and gated behind a security envelope summarized in
//! `docs/security/WHITEPAPER.md` §10.3.
//!
//! ## Why this lives in `marque-config`, not `marque-capco`
//!
//! `marque-capco` is a WASM-safe crate (Constitution III). Putting the
//! parser here keeps the wasm-shipping crate set free of an opt-in
//! security surface that the WASM target will never legitimately use.
//! The engine pulls this module in only when its own
//! `corpus-override` feature is enabled, which itself is unreachable
//! from the WASM crate.
//!
//! ## File shape (`schema_version: "marque-corpus-override-1"`)
//!
//! ```json
//! {
//!   "schema_version": "marque-corpus-override-1",
//!   "token_overrides": {
//!     "SECRET": { "log_prior": -2.5 },
//!     "NOFORN": { "log_prior": -3.0 }
//!   },
//!   "template_overrides": {
//!     "classification//dissem": { "log_prior": -1.8 }
//!   },
//!   "strict_context_overrides": {
//!     "confidential_floor": 0.95,
//!     "secret_floor":       0.98,
//!     "top_secret_floor":   0.99
//!   }
//! }
//! ```
//!
//! All three override sections are optional; an empty object (only
//! `schema_version`) is a legal "audit-marker only" override that
//! produces no scoring change but stamps every decoder fix with
//! `CorpusOverrideInEffect`. Counts (`count` in the build-time
//! `priors.json`) are intentionally omitted — operators do not
//! override raw counts; they only override log-priors. Anything
//! beyond the documented fields is rejected so a typo (e.g.
//! `token_override` singular) does not silently no-op.
//!
//! ## What this module is NOT (PR-5 minimal-scope)
//!
//! Loading + validating + handing the parsed value to the engine is
//! the entire surface today. The engine stores it, exposes
//! `Engine::corpus_override_active()`, and stamps the audit feature
//! contribution — but **does not yet substitute the override priors
//! into decoder scoring**. Wiring the override priors into the
//! decoder's prior-table lookup is a follow-up; the security envelope
//! is the load-bearing piece of PR-5.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::ConfigError;

/// On-the-wire schema version. Bump when the shape changes; the
/// loader refuses any other value rather than silently misparsing a
/// future shape.
pub const SCHEMA_VERSION: &str = "marque-corpus-override-1";

/// Parsed, validated corpus override.
///
/// Construction goes through [`load_corpus_override`] (or
/// [`parse_corpus_override`] for tests / in-memory input). All log-prior
/// values are guaranteed finite at this point. Cloning is cheap — the
/// inner maps are not large in practice (operator overrides target a
/// handful of tokens or templates, not the full vocabulary).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CorpusOverride {
    /// Per-token log-prior overrides. Token keys are canonical CAPCO
    /// forms (e.g., `"SECRET"`, `"NOFORN"`).
    pub token_overrides: BTreeMap<String, f32>,
    /// Per-template log-prior overrides. Template keys match the
    /// `GrammarTemplate` shape identifiers the decoder consumes (e.g.,
    /// `"classification//dissem"`).
    pub template_overrides: BTreeMap<String, f32>,
    /// Strict-context floor overrides for candidate filtering.
    /// Any subset of the three may be supplied.
    pub strict_context_overrides: StrictContextOverrides,
}

impl CorpusOverride {
    /// Returns `true` when the override carries no scoring data —
    /// audit-marker-only overrides (only `schema_version` set) are a
    /// legal shape and useful for "stamp every fix as override-derived
    /// without changing scores."
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.token_overrides.is_empty()
            && self.template_overrides.is_empty()
            && self.strict_context_overrides.is_empty()
    }
}

/// Strict-context floor overrides. Each field is `Some(_)` when the
/// override file supplied that floor; `None` otherwise — the decoder
/// falls back to the baked floor from `priors.json` for any axis the
/// override left unset.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StrictContextOverrides {
    pub confidential_floor: Option<f32>,
    pub secret_floor: Option<f32>,
    pub top_secret_floor: Option<f32>,
}

impl StrictContextOverrides {
    #[inline]
    fn is_empty(&self) -> bool {
        self.confidential_floor.is_none()
            && self.secret_floor.is_none()
            && self.top_secret_floor.is_none()
    }
}

// ---------------------------------------------------------------------------
// Wire format (private)
// ---------------------------------------------------------------------------
//
// `deny_unknown_fields` on every type — a typo like `token_override`
// (singular) MUST surface a parse error rather than silently no-op
// every override the operator wrote. This is a security-relevant
// surface: an operator who thinks they reduced the prior on `SECRET`
// but actually wrote nothing because of a typo would believe the
// override is in effect when it is not.

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OverrideFile {
    schema_version: String,
    #[serde(default)]
    token_overrides: BTreeMap<String, TokenOverrideEntry>,
    #[serde(default)]
    template_overrides: BTreeMap<String, TemplateOverrideEntry>,
    #[serde(default)]
    strict_context_overrides: Option<StrictContextOverridesFile>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TokenOverrideEntry {
    /// Log-prior in `f64` on the wire; downcast to `f32` once parsed
    /// (matches the build-time priors handling in
    /// `crates/capco/build.rs`).
    log_prior: f64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TemplateOverrideEntry {
    log_prior: f64,
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
struct StrictContextOverridesFile {
    #[serde(default)]
    confidential_floor: Option<f64>,
    #[serde(default)]
    secret_floor: Option<f64>,
    #[serde(default)]
    top_secret_floor: Option<f64>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Load and validate a corpus override from disk.
///
/// Surface a [`ConfigError`] for I/O failures (missing file, permission
/// errors → `EX_IOERR`) and for parse / validation failures (bad JSON,
/// unknown fields, non-finite log-prior, schema-version mismatch →
/// `EX_DATAERR`).
pub fn load_corpus_override(path: &Path) -> Result<CorpusOverride, ConfigError> {
    let raw = std::fs::read_to_string(path).map_err(|e| ConfigError::ReadError {
        path: path.to_path_buf(),
        source: e,
    })?;
    parse_corpus_override(&raw, path.to_path_buf())
}

/// Parse a corpus override from an in-memory string. Exposed for
/// integration tests; production callers should prefer
/// [`load_corpus_override`].
pub fn parse_corpus_override(
    raw: &str,
    source_path: PathBuf,
) -> Result<CorpusOverride, ConfigError> {
    let file: OverrideFile =
        serde_json::from_str(raw).map_err(|e| ConfigError::CorpusOverrideParse {
            path: source_path.clone(),
            reason: e.to_string(),
        })?;

    if file.schema_version != SCHEMA_VERSION {
        return Err(ConfigError::CorpusOverrideSchemaMismatch {
            path: source_path,
            file_version: file.schema_version,
            expected: SCHEMA_VERSION,
        });
    }

    // `f64 → f32` downcast happens here; we validate finiteness on the
    // f32 side because that is the precision the decoder consumes (the
    // f64 wire form may round to an f32 subnormal or to ±∞ on extreme
    // values, and we want the audit boundary to catch that here, not
    // at scoring time).
    let mut token_overrides = BTreeMap::new();
    for (token, entry) in file.token_overrides {
        let lp = entry.log_prior as f32;
        validate_log_prior(&source_path, "token_overrides", &token, lp)?;
        token_overrides.insert(token, lp);
    }

    let mut template_overrides = BTreeMap::new();
    for (template, entry) in file.template_overrides {
        let lp = entry.log_prior as f32;
        validate_log_prior(&source_path, "template_overrides", &template, lp)?;
        template_overrides.insert(template, lp);
    }

    let strict_context_overrides = match file.strict_context_overrides {
        None => StrictContextOverrides::default(),
        Some(s) => {
            let mut out = StrictContextOverrides::default();
            if let Some(v) = s.confidential_floor {
                let v32 = v as f32;
                validate_floor(&source_path, "confidential_floor", v32)?;
                out.confidential_floor = Some(v32);
            }
            if let Some(v) = s.secret_floor {
                let v32 = v as f32;
                validate_floor(&source_path, "secret_floor", v32)?;
                out.secret_floor = Some(v32);
            }
            if let Some(v) = s.top_secret_floor {
                let v32 = v as f32;
                validate_floor(&source_path, "top_secret_floor", v32)?;
                out.top_secret_floor = Some(v32);
            }
            out
        }
    };

    Ok(CorpusOverride {
        token_overrides,
        template_overrides,
        strict_context_overrides,
    })
}

/// Validate an override-supplied log-prior.
///
/// Policy: reject `NaN`, `+Inf`, and `-Inf`. `-Inf` is mathematically
/// the log of `0.0`, which an operator might intend as a hard
/// "infinite penalty / dead token" claim — a legitimate concept —
/// but allowing it as wire-format input is a footgun:
///
/// - A regenerator emitting `-Inf` accidentally (e.g., `log(0)` from
///   an empty corpus bucket) silently kills a candidate forever
///   with no diagnostic at validation time.
/// - The decoder's hot-path scoring uses `f32` log-posterior
///   addition; an `-Inf` summand contaminates downstream
///   arithmetic (`-Inf + finite = -Inf`, `-Inf - -Inf = NaN`),
///   which the L3 NaN-filter hardens against but does not need to
///   absorb additionally for operator-introduced inputs.
/// - Operators who want "very rare in this context" can write a
///   finite very-negative number (e.g., `-50.0`) which has the
///   same practical effect on candidate ranking without the
///   silent-deletion footgun.
///
/// If a future scoring change makes infinite penalties first-class,
/// this function can be relaxed; until then, the contract is
/// "log_priors must be finite."
fn validate_log_prior(
    path: &Path,
    section: &'static str,
    key: &str,
    value: f32,
) -> Result<(), ConfigError> {
    if !value.is_finite() {
        return Err(ConfigError::CorpusOverrideInvalidValue {
            path: path.to_path_buf(),
            section,
            key: key.to_owned(),
            reason: "log_prior must be finite — `-Inf` (`log(0)`) is rejected as a regenerator footgun; \
                     express 'very rare' with a finite very-negative number (e.g., -50.0) instead",
        });
    }
    // Log-priors are non-positive in well-formed corpora (probabilities
    // are in (0, 1], so log_prior ≤ 0). Accept slight numerical slop
    // (≤ 1e-3) so a hand-written override with `log_prior: 0.0` rounds
    // through cleanly; reject anything appreciably positive.
    if value > 1e-3 {
        return Err(ConfigError::CorpusOverrideInvalidValue {
            path: path.to_path_buf(),
            section,
            key: key.to_owned(),
            reason: "log_prior must be ≤ 0 (probabilities ≤ 1)",
        });
    }
    Ok(())
}

/// Validate a runtime override-supplied strict-context floor.
///
/// Mirrors the build-time policy in
/// `crates/capco/build.rs::require_probability`. Accepts `(0.0,
/// 1.0]`; rejects `0.0` because a `0.0` floor silently makes the
/// strict-context rule a no-op (the feature contribution becomes
/// algebraically identity), defeating candidate filtering with no
/// diagnostic at load time. Operators who want "very permissive" should
/// write a finite small positive (e.g., `0.01`).
fn validate_floor(path: &Path, key: &'static str, value: f32) -> Result<(), ConfigError> {
    if !(value.is_finite() && value > 0.0 && value <= 1.0) {
        return Err(ConfigError::CorpusOverrideInvalidValue {
            path: path.to_path_buf(),
            section: "strict_context_overrides",
            key: key.to_owned(),
            reason: "floor must be in (0.0, 1.0] and finite — `0.0` is rejected because it silently \
                     makes the strict-context rule a no-op; write a finite small positive (e.g., 0.01) \
                     for a permissive floor",
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    fn p() -> PathBuf {
        PathBuf::from("test.json")
    }

    #[test]
    fn parses_minimal_audit_marker_only_override() {
        // The smallest legal override: no scoring data, just the
        // schema version. Useful for "stamp every fix as override-
        // derived without changing scores."
        let raw = r#"{"schema_version": "marque-corpus-override-1"}"#;
        let parsed = parse_corpus_override(raw, p()).unwrap();
        assert!(parsed.is_empty());
        assert!(parsed.token_overrides.is_empty());
        assert!(parsed.template_overrides.is_empty());
        assert_eq!(
            parsed.strict_context_overrides,
            StrictContextOverrides::default()
        );
    }

    #[test]
    fn parses_full_override() {
        let raw = r#"{
            "schema_version": "marque-corpus-override-1",
            "token_overrides": {
                "SECRET": { "log_prior": -2.5 },
                "NOFORN": { "log_prior": -3.0 }
            },
            "template_overrides": {
                "classification//dissem": { "log_prior": -1.8 }
            },
            "strict_context_overrides": {
                "confidential_floor": 0.95,
                "secret_floor": 0.98,
                "top_secret_floor": 0.99
            }
        }"#;
        let parsed = parse_corpus_override(raw, p()).unwrap();
        assert!(!parsed.is_empty());
        assert_eq!(parsed.token_overrides.len(), 2);
        assert!((parsed.token_overrides["SECRET"] - (-2.5)).abs() < 1e-5);
        assert!((parsed.token_overrides["NOFORN"] - (-3.0)).abs() < 1e-5);
        assert_eq!(parsed.template_overrides.len(), 1);
        assert!((parsed.template_overrides["classification//dissem"] - (-1.8)).abs() < 1e-5);
        assert_eq!(
            parsed.strict_context_overrides.confidential_floor,
            Some(0.95)
        );
        assert_eq!(parsed.strict_context_overrides.secret_floor, Some(0.98));
        assert_eq!(parsed.strict_context_overrides.top_secret_floor, Some(0.99));
    }

    #[test]
    fn parses_partial_strict_context_overrides() {
        // Only one floor specified — the other two stay None so the
        // engine falls back to baked priors for those.
        let raw = r#"{
            "schema_version": "marque-corpus-override-1",
            "strict_context_overrides": { "secret_floor": 0.97 }
        }"#;
        let parsed = parse_corpus_override(raw, p()).unwrap();
        assert_eq!(parsed.strict_context_overrides.confidential_floor, None);
        assert_eq!(parsed.strict_context_overrides.secret_floor, Some(0.97));
        assert_eq!(parsed.strict_context_overrides.top_secret_floor, None);
    }

    #[test]
    fn rejects_unknown_schema_version() {
        let raw = r#"{"schema_version": "marque-corpus-override-99"}"#;
        let err = parse_corpus_override(raw, p()).unwrap_err();
        match err {
            ConfigError::CorpusOverrideSchemaMismatch {
                file_version,
                expected,
                ..
            } => {
                assert_eq!(file_version, "marque-corpus-override-99");
                assert_eq!(expected, SCHEMA_VERSION);
            }
            other => panic!("expected SchemaMismatch, got {other:?}"),
        }
    }

    #[test]
    fn rejects_missing_schema_version() {
        let raw = r#"{}"#;
        // No `schema_version` field → JSON parse failure surfaces as
        // CorpusOverrideParse, not SchemaMismatch (the schema-version
        // check only runs after a successful structural parse).
        assert!(matches!(
            parse_corpus_override(raw, p()),
            Err(ConfigError::CorpusOverrideParse { .. })
        ));
    }

    #[test]
    fn rejects_unknown_top_level_field() {
        // `token_override` is the canonical operator typo (singular).
        // Without `deny_unknown_fields` the entire override would
        // silently no-op while the operator believes it took effect.
        let raw = r#"{
            "schema_version": "marque-corpus-override-1",
            "token_override": { "SECRET": { "log_prior": -2.5 } }
        }"#;
        assert!(matches!(
            parse_corpus_override(raw, p()),
            Err(ConfigError::CorpusOverrideParse { .. })
        ));
    }

    #[test]
    fn rejects_unknown_token_entry_field() {
        let raw = r#"{
            "schema_version": "marque-corpus-override-1",
            "token_overrides": {
                "SECRET": { "log_prior": -2.5, "weight": 0.5 }
            }
        }"#;
        assert!(matches!(
            parse_corpus_override(raw, p()),
            Err(ConfigError::CorpusOverrideParse { .. })
        ));
    }

    #[test]
    fn rejects_non_finite_log_prior() {
        // serde_json rejects bare NaN/Infinity in JSON, but the cast
        // path can also produce ±∞ from f64 → f32 narrowing on extreme
        // values. Easier to test: positive log_prior crosses the
        // probability ≤ 1 invariant.
        let raw = r#"{
            "schema_version": "marque-corpus-override-1",
            "token_overrides": { "SECRET": { "log_prior": 5.0 } }
        }"#;
        match parse_corpus_override(raw, p()).unwrap_err() {
            ConfigError::CorpusOverrideInvalidValue { section, key, .. } => {
                assert_eq!(section, "token_overrides");
                assert_eq!(key, "SECRET");
            }
            other => panic!("expected InvalidValue, got {other:?}"),
        }
    }

    #[test]
    fn rejects_floor_outside_unit_interval() {
        let raw = r#"{
            "schema_version": "marque-corpus-override-1",
            "strict_context_overrides": { "secret_floor": 1.5 }
        }"#;
        match parse_corpus_override(raw, p()).unwrap_err() {
            ConfigError::CorpusOverrideInvalidValue { section, key, .. } => {
                assert_eq!(section, "strict_context_overrides");
                assert_eq!(key, "secret_floor");
            }
            other => panic!("expected InvalidValue, got {other:?}"),
        }
    }

    #[test]
    fn accepts_log_prior_zero_with_slop() {
        // A hand-written override with exactly 0.0 is the
        // "probability = 1" extreme; it's legal, and our >1e-3
        // tolerance must not reject it.
        let raw = r#"{
            "schema_version": "marque-corpus-override-1",
            "token_overrides": { "SECRET": { "log_prior": 0.0 } }
        }"#;
        let parsed = parse_corpus_override(raw, p()).unwrap();
        assert_eq!(parsed.token_overrides["SECRET"], 0.0);
    }

    #[test]
    fn load_corpus_override_returns_read_error_for_missing_file() {
        // Construct a guaranteed-missing path inside a freshly-created
        // tempdir rather than hardcoding `/nonexistent/...` — the latter
        // is non-portable (Windows path semantics differ) and on Unix
        // can collide with a real path under unusual sandboxes. The
        // tempdir itself exists (we just made it); the file inside it
        // does not, which is exactly the ReadError-triggering condition
        // we want to exercise.
        let tmp = tempfile::tempdir().unwrap();
        let bad = tmp.path().join("missing-override.json");
        match load_corpus_override(&bad).unwrap_err() {
            ConfigError::ReadError { path, .. } => assert_eq!(path, bad),
            other => panic!("expected ReadError, got {other:?}"),
        }
    }
}
