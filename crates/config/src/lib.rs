// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! marque-config — layered configuration loading.
//!
//! Precedence (highest wins): CLI flags → env vars → `.marque.local.toml` → `.marque.toml`
//!
//! # Hard-fail validators
//!
//! The loader refuses to produce a `Config` if any of these conditions hold:
//! - `.marque.toml` contains a `[user]` section → exit 65
//! - `[capco] version` mismatches `marque_ism::SCHEMA_VERSION` → exit 65
//! - `confidence_threshold` outside `[0.0, 1.0]` → exit 65

use marque_ism::UtcOffset;
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

#[cfg(feature = "corpus-override")]
pub mod corpus_override;
#[cfg(feature = "toml-loader")]
mod corrections;
#[cfg(feature = "toml-loader")]
mod env;
#[cfg(feature = "toml-loader")]
mod layered;
#[cfg(feature = "toml-loader")]
mod local;
#[cfg(feature = "toml-loader")]
mod severity;

#[cfg(feature = "toml-loader")]
pub use layered::{load, load_with_explicit_config};

/// Exit code 65 (`EX_DATAERR`) per `contracts/cli.md`.
pub const EX_DATAERR: i32 = 65;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file {path}: {source}")]
    ReadError {
        path: PathBuf,
        source: std::io::Error,
    },

    /// `.marque.toml` / `.marque.local.toml` failed to parse. Only present
    /// when the `toml-loader` feature is on; the WASM artifact ships with
    /// the feature off and receives config from JS callers as a JSON-set
    /// `Config`, so a TOML parse error variant is unreachable there.
    #[cfg(feature = "toml-loader")]
    #[error("failed to parse config: {0}")]
    ParseError(#[from] toml::de::Error),

    /// `.marque.toml` contains a `[user]` section.
    #[error(
        "committed config file {path} contains a [user] section — classifier identity \
         must live only in .marque.local.toml or env vars (FR-010)"
    )]
    UserSectionInCommitted { path: PathBuf },

    /// Schema version in config doesn't match compiled schema.
    #[error(
        "schema version mismatch: config says {config_version:?} but marque was compiled \
         against {compiled_version:?} (FR-011). Update [capco] version in .marque.toml."
    )]
    SchemaVersionMismatch {
        config_version: String,
        compiled_version: &'static str,
    },

    /// Confidence threshold out of range.
    #[error("confidence_threshold {value} is outside [0.0, 1.0]")]
    ThresholdOutOfRange { value: f32 },

    /// Environment variable could not be parsed into the expected type.
    #[error("environment variable {var} has invalid value {raw:?}: {reason}")]
    InvalidEnvVar {
        var: &'static str,
        raw: String,
        reason: &'static str,
    },

    /// Rule severity string in config is not one of the recognized values.
    #[error(
        "rule {rule:?} has unrecognized severity {value:?} — expected one of \
         \"off\", \"suggest\", \"info\", \"warn\", \"error\", \"fix\""
    )]
    UnknownSeverity { rule: String, value: String },

    /// Closure-rule severity string in `[closure_rules]` config is not one
    /// of the recognized values. Differs from `UnknownSeverity` because
    /// closure rules do not accept `"fix"` (closure firings propagate
    /// facts, not byte-level fixes), so the user-facing error message
    /// should not list `"fix"` as an expected value.
    /// Per Copilot PR 3.7 review #3 — Constitution VIII fidelity for
    /// the diagnostic surface (don't tell the user `"fix"` is acceptable
    /// for closure rules then reject `"fix"` on the next code path).
    #[error(
        "closure rule {rule:?} has unrecognized severity {value:?} — expected one of \
         \"off\", \"suggest\", \"info\", \"warn\", \"error\" (note: closure rules \
         do not accept \"fix\")"
    )]
    UnknownClosureRuleSeverity { rule: String, value: String },

    /// Timezone offset string is not a recognized ISO 8601 UTC offset form.
    #[error("invalid timezone offset {value:?} — expected \"Z\", \"+HH:MM\", or \"-HH:MM\"")]
    InvalidTimezone { value: String },

    /// Corpus-override file did not parse as JSON, or violated the
    /// `deny_unknown_fields` contract on any wire-format struct.
    #[error("failed to parse corpus override {path}: {reason}")]
    CorpusOverrideParse { path: PathBuf, reason: String },

    /// Corpus-override file's `schema_version` is not the value
    /// supported by this build of marque.
    #[error(
        "corpus override {path} has schema_version {file_version:?} but this build of marque \
         supports {expected:?}"
    )]
    CorpusOverrideSchemaMismatch {
        path: PathBuf,
        file_version: String,
        expected: &'static str,
    },

    /// Corpus-override file contained a value that failed range /
    /// finiteness validation. `section` and `key` localize the
    /// violation so an operator can find and correct the offending
    /// entry without grepping the whole file.
    #[error("corpus override {path}: invalid {section}.{key}: {reason}")]
    CorpusOverrideInvalidValue {
        path: PathBuf,
        section: &'static str,
        key: String,
        reason: &'static str,
    },

    /// Closure rule severity override uses "fix", which is rejected per
    /// `decisions.md` D19 B. Closure firings propagate facts, not byte-level
    /// edits, so the only valid severities for closure rules are
    /// `off / suggest / info / warn / error`.
    #[error(
        "closure rule {rule:?} cannot use 'fix' severity; \
        use 'warn' or 'error' (or 'off'/'info'/'suggest' for non-blocking surfaces)"
    )]
    InvalidClosureRuleSeverity {
        rule: String,
        /// Hint string for downstream tooling (e.g., the CLI surface) — not
        /// used in the Display impl but available for richer surfaces.
        hint: &'static str,
    },
}

impl ConfigError {
    /// Returns the exit code for this error per `contracts/cli.md`.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::ReadError { .. } => 74, // EX_IOERR
            #[cfg(feature = "toml-loader")]
            Self::ParseError(_) => EX_DATAERR,
            Self::UserSectionInCommitted { .. } => EX_DATAERR,
            Self::SchemaVersionMismatch { .. } => EX_DATAERR,
            Self::ThresholdOutOfRange { .. } => EX_DATAERR,
            Self::InvalidEnvVar { .. } => EX_DATAERR,
            Self::UnknownSeverity { .. } => EX_DATAERR,
            Self::UnknownClosureRuleSeverity { .. } => EX_DATAERR,
            Self::InvalidTimezone { .. } => EX_DATAERR,
            Self::CorpusOverrideParse { .. } => EX_DATAERR,
            Self::CorpusOverrideSchemaMismatch { .. } => EX_DATAERR,
            Self::CorpusOverrideInvalidValue { .. } => EX_DATAERR,
            Self::InvalidClosureRuleSeverity { .. } => EX_DATAERR,
        }
    }
}

/// Resolved, merged configuration ready for engine use.
#[derive(Debug, Clone)]
pub struct Config {
    pub user: UserConfig,
    pub rules: RuleConfig,
    /// Per-closure-rule severity overrides from `[closure_rules]` in `.marque.toml`.
    ///
    /// Keyed by closure rule name in the wire-string form
    /// (e.g., `"capco:closure.dissem.noforn-if-caveated"`).
    /// `Severity::Fix` is rejected at config load — closure firings propagate
    /// facts, not byte-level edits.
    pub closure_rules: ClosureRuleConfig,
    /// Organization-specific typo corrections from `[corrections]` in `.marque.toml`.
    ///
    /// **Do not mutate after passing to `Engine::new`** — the engine caches
    /// this as an `Arc<HashMap>` at construction time. Post-construction
    /// mutation leaves the cached copy stale.
    pub corrections: HashMap<String, String>,
    pub capco: CapcoConfig,
    /// Fix confidence threshold. Fixes with confidence >= this value are auto-applied.
    /// Default: 0.95 per spec.
    confidence_threshold: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            user: UserConfig::default(),
            rules: RuleConfig::default(),
            closure_rules: ClosureRuleConfig::default(),
            corrections: HashMap::new(),
            capco: CapcoConfig::default(),
            confidence_threshold: 0.95,
        }
    }
}

impl Config {
    /// Returns the confidence threshold for auto-applying fixes.
    pub fn confidence_threshold(&self) -> f32 {
        self.confidence_threshold
    }

    /// Set confidence threshold (validated at load time).
    pub fn set_confidence_threshold(&mut self, value: f32) -> Result<(), ConfigError> {
        if !(0.0..=1.0).contains(&value) || value.is_nan() {
            return Err(ConfigError::ThresholdOutOfRange { value });
        }
        self.confidence_threshold = value;
        Ok(())
    }
}

/// User identity — always from local config, never committed.
#[derive(Debug, Clone, Default)]
pub struct UserConfig {
    pub classifier_id: Option<String>,
    pub classification_authority: Option<String>,
    pub default_reason: Option<String>,
    pub derived_from_default: Option<String>,
}

/// Per-rule severity overrides.
#[derive(Debug, Clone, Default)]
pub struct RuleConfig {
    /// Map of rule ID → configured severity string ("fix", "warn", "error", "off").
    pub overrides: HashMap<String, String>,
}

/// Per-closure-rule severity overrides.
///
/// Section-isolated from `[rules]`.
/// Keyed by `ClosureRule.name` in the wire-string form
/// (e.g. `"capco:closure.dissem.noforn-if-caveated"`).
/// `Severity::Fix` is rejected at config load because closure firings
/// are not byte-level fixes — see `ConfigError::InvalidClosureRuleSeverity`.
#[derive(Debug, Clone, Default)]
pub struct ClosureRuleConfig {
    /// Map of closure-rule name → configured severity string
    /// ("off", "suggest", "info", "warn", "error"). "fix" is rejected.
    pub overrides: HashMap<String, String>,
}

/// CAPCO-specific configuration.
#[derive(Debug, Clone)]
pub struct CapcoConfig {
    /// Pinned ISM schema version. Must match the compiled marque-ism version.
    pub version: String,

    /// Default UTC offset applied to floating (offset-naive) `DateHourMin` and
    /// `DateTime` values encountered during document processing.
    ///
    /// In national-security documents, times without an explicit offset are
    /// conventionally Zulu (UTC). Set this to a different offset only when
    /// processing documents from an organization that consistently marks times
    /// with a local civil offset without recording it explicitly in the marking.
    ///
    /// Configurable via `[capco] default_timezone = "Z"` in `.marque.toml` or
    /// the `MARQUE_DEFAULT_TIMEZONE` environment variable.
    /// Accepted forms: `"Z"`, `"+HH:MM"`, `"-HH:MM"`. Defaults to UTC.
    pub default_timezone: UtcOffset,
}

impl Default for CapcoConfig {
    fn default() -> Self {
        Self {
            version: marque_ism::generated::values::SCHEMA_VERSION.to_owned(),
            default_timezone: UtcOffset::UTC,
        }
    }
}

#[cfg(all(feature = "toml-loader", test))]
use layered::ConfigFile;
#[cfg(all(feature = "toml-loader", test))]
use layered::{discover_project_dir, merge_project_into};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[cfg(feature = "toml-loader")]
    fn config_file_with_rules(rules: &[(&str, &str)]) -> ConfigFile {
        let mut file = ConfigFile::default();
        for (k, v) in rules {
            file.rules.insert((*k).to_owned(), (*v).to_owned());
        }
        file
    }

    #[test]
    fn set_confidence_threshold_accepts_boundaries() {
        let mut c = Config::default();
        assert!(c.set_confidence_threshold(0.0).is_ok());
        assert!(c.set_confidence_threshold(1.0).is_ok());
        assert!(c.set_confidence_threshold(0.5).is_ok());
    }

    #[test]
    fn set_confidence_threshold_rejects_out_of_range() {
        let mut c = Config::default();
        assert!(matches!(
            c.set_confidence_threshold(-0.1),
            Err(ConfigError::ThresholdOutOfRange { .. })
        ));
        assert!(matches!(
            c.set_confidence_threshold(1.1),
            Err(ConfigError::ThresholdOutOfRange { .. })
        ));
    }

    #[test]
    fn set_confidence_threshold_rejects_nan() {
        let mut c = Config::default();
        assert!(matches!(
            c.set_confidence_threshold(f32::NAN),
            Err(ConfigError::ThresholdOutOfRange { .. })
        ));
    }

    #[cfg(feature = "toml-loader")]
    #[test]
    fn merge_project_accepts_valid_severity_strings() {
        let mut c = Config::default();
        let file = config_file_with_rules(&[
            ("E001", "fix"),
            ("E002", "warn"),
            ("E003", "error"),
            ("E004", "off"),
            ("E005", "info"),
            ("S004", "suggest"),
        ]);
        assert!(merge_project_into(&mut c, file).is_ok());
        assert_eq!(c.rules.overrides.len(), 6);
    }

    #[cfg(feature = "toml-loader")]
    #[test]
    fn merge_project_accepts_suggest_severity() {
        // Issue #235 / #186 PR-3: the suggest-don't-fix channel must be
        // a config-valid severity string. Validates the loader pipes
        // through `Severity::parse_config("suggest")`.
        let mut c = Config::default();
        let file = config_file_with_rules(&[("S004", "suggest")]);
        assert!(merge_project_into(&mut c, file).is_ok());
        assert_eq!(
            c.rules.overrides.get("S004").map(String::as_str),
            Some("suggest")
        );
    }

    #[cfg(feature = "toml-loader")]
    #[test]
    fn merge_project_rejects_unknown_severity() {
        let mut c = Config::default();
        let file = config_file_with_rules(&[("E001", "err")]);
        let err = merge_project_into(&mut c, file).unwrap_err();
        match err {
            ConfigError::UnknownSeverity { rule, value } => {
                assert_eq!(rule, "E001");
                assert_eq!(value, "err");
            }
            other => panic!("expected UnknownSeverity, got {other:?}"),
        }
    }

    #[cfg(feature = "toml-loader")]
    #[test]
    fn merge_project_rejects_severity_is_case_sensitive() {
        // Severity::parse_config is case-sensitive by design — uppercase must fail.
        let mut c = Config::default();
        let file = config_file_with_rules(&[("E001", "FIX")]);
        assert!(matches!(
            merge_project_into(&mut c, file),
            Err(ConfigError::UnknownSeverity { .. })
        ));
    }

    #[cfg(feature = "toml-loader")]
    #[test]
    fn merge_project_rejects_empty_severity() {
        let mut c = Config::default();
        let file = config_file_with_rules(&[("E001", "")]);
        assert!(matches!(
            merge_project_into(&mut c, file),
            Err(ConfigError::UnknownSeverity { .. })
        ));
    }

    #[test]
    fn exit_code_matches_contract() {
        assert_eq!(
            ConfigError::ThresholdOutOfRange { value: 2.0 }.exit_code(),
            EX_DATAERR
        );
        assert_eq!(
            ConfigError::UnknownSeverity {
                rule: "E001".into(),
                value: "err".into(),
            }
            .exit_code(),
            EX_DATAERR
        );
        assert_eq!(
            ConfigError::InvalidEnvVar {
                var: "MARQUE_CONFIDENCE_THRESHOLD",
                raw: "bananas".into(),
                reason: "not a float",
            }
            .exit_code(),
            EX_DATAERR
        );
    }

    // ---------------------------------------------------------------------
    // D.1: discover_project_dir upward-walk semantics
    //
    // All discover_* / load_* tests below exercise the `toml-loader` codepath
    // (file IO + TOML parsing). They are gated as a block; the WASM build
    // (default-features = false on the workspace pin) excludes them.
    // ---------------------------------------------------------------------

    #[cfg(feature = "toml-loader")]
    use std::fs;
    #[cfg(feature = "toml-loader")]
    use std::path::PathBuf;

    #[cfg(feature = "toml-loader")]
    fn make_tmpdir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("marque-config-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create tmpdir");
        dir
    }

    #[cfg(feature = "toml-loader")]
    #[test]
    fn discover_finds_marque_toml_in_start_dir() {
        let dir = make_tmpdir("discover-here");
        fs::write(dir.join(".marque.toml"), b"").unwrap();
        assert_eq!(super::discover_project_dir(&dir), Some(dir.clone()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(feature = "toml-loader")]
    #[test]
    fn discover_walks_upward_for_marque_toml() {
        // tmp/root/.marque.toml; start from tmp/root/sub/deeper.
        let root = make_tmpdir("discover-walk");
        fs::write(root.join(".marque.toml"), b"").unwrap();
        let sub = root.join("sub").join("deeper");
        fs::create_dir_all(&sub).unwrap();
        assert_eq!(super::discover_project_dir(&sub), Some(root.clone()));
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(feature = "toml-loader")]
    #[test]
    fn discover_stops_at_git_root_without_marque_toml() {
        // tmp/root/.git/ + tmp/root/sub/ — start from sub, walk should hit
        // .git in root and return None (no project config above this point).
        let root = make_tmpdir("discover-git-stop");
        fs::create_dir_all(root.join(".git")).unwrap();
        let sub = root.join("sub");
        fs::create_dir_all(&sub).unwrap();
        assert_eq!(super::discover_project_dir(&sub), None);
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(feature = "toml-loader")]
    #[test]
    fn discover_returns_marque_toml_at_git_root_when_both_present() {
        // The common case: a repo whose root has both .git and .marque.toml.
        // The walk must NOT stop at .git before checking .marque.toml.
        let root = make_tmpdir("discover-both");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".marque.toml"), b"").unwrap();
        let sub = root.join("crates").join("foo");
        fs::create_dir_all(&sub).unwrap();
        assert_eq!(super::discover_project_dir(&sub), Some(root.clone()));
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(feature = "toml-loader")]
    #[test]
    fn load_walks_upward_to_find_project_config() {
        // tmp/root/.marque.toml + tmp/root/sub/, load from sub.
        let root = make_tmpdir("load-walk");
        fs::write(
            root.join(".marque.toml"),
            br#"
[rules]
E001 = "warn"
"#,
        )
        .unwrap();
        let sub = root.join("sub");
        fs::create_dir_all(&sub).unwrap();
        let config = super::load(&sub).expect("load should succeed");
        assert_eq!(config.rules.overrides.get("E001"), Some(&"warn".to_owned()));
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(feature = "toml-loader")]
    #[test]
    fn load_returns_defaults_when_walk_finds_no_marque_toml() {
        // tmp/root/.git but no .marque.toml — load returns defaults.
        let root = make_tmpdir("load-defaults");
        fs::create_dir_all(root.join(".git")).unwrap();
        let sub = root.join("sub");
        fs::create_dir_all(&sub).unwrap();
        let config = super::load(&sub).expect("load should succeed with defaults");
        assert!(config.rules.overrides.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(feature = "toml-loader")]
    #[test]
    fn load_local_config_only_in_same_dir_as_marque_toml() {
        // tmp/root/.marque.toml + tmp/root/.marque.local.toml
        // tmp/root/sub/.marque.local.toml (should NOT be loaded)
        let root = make_tmpdir("load-local-same-dir");
        fs::write(
            root.join(".marque.toml"),
            br#"
[capco]
"#,
        )
        .unwrap();
        fs::write(
            root.join(".marque.local.toml"),
            br#"
[user]
classifier_id = "from-root"
"#,
        )
        .unwrap();
        let sub = root.join("sub");
        fs::create_dir_all(&sub).unwrap();
        // A stray local config in `sub` should NOT be loaded — the local
        // search is anchored to the directory of the project config.
        fs::write(
            sub.join(".marque.local.toml"),
            br#"
[user]
classifier_id = "from-sub"
"#,
        )
        .unwrap();
        let config = super::load(&sub).expect("load should succeed");
        assert_eq!(
            config.user.classifier_id.as_deref(),
            Some("from-root"),
            "local config must be the one alongside .marque.toml, not in sub"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(feature = "toml-loader")]
    #[test]
    #[cfg(unix)]
    fn load_returns_read_error_for_unreadable_project_config() {
        use std::os::unix::fs::PermissionsExt;
        let root = make_tmpdir("load-err-proj");
        let project_config = root.join(".marque.toml");
        fs::write(&project_config, b"").unwrap();

        let mut perms = fs::metadata(&project_config).unwrap().permissions();
        perms.set_mode(0o000); // remove read permission
        fs::set_permissions(&project_config, perms).unwrap();

        let err = super::load(&root).unwrap_err();
        assert!(matches!(err, ConfigError::ReadError { .. }));

        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(feature = "toml-loader")]
    #[test]
    #[cfg(unix)]
    fn load_returns_read_error_for_unreadable_local_config() {
        use std::os::unix::fs::PermissionsExt;
        let root = make_tmpdir("load-err-local");
        fs::write(root.join(".marque.toml"), b"").unwrap();

        let local_config = root.join(".marque.local.toml");
        fs::write(&local_config, b"").unwrap();

        let mut perms = fs::metadata(&local_config).unwrap().permissions();
        perms.set_mode(0o000); // remove read permission
        fs::set_permissions(&local_config, perms).unwrap();

        let err = super::load(&root).unwrap_err();
        assert!(matches!(err, ConfigError::ReadError { .. }));

        let _ = fs::remove_dir_all(&root);
    }

    // ---------------------------------------------------------------------
    // TZ-1: timezone config
    // ---------------------------------------------------------------------

    #[test]
    fn capco_default_timezone_defaults_to_utc() {
        let c = Config::default();
        assert_eq!(c.capco.default_timezone, UtcOffset::UTC);
    }

    #[cfg(feature = "toml-loader")]
    #[test]
    fn merge_project_accepts_valid_timezone_offsets() {
        for tz in ["Z", "+05:30", "-05:00", "+00:00", "+23:59"] {
            let mut c = Config::default();
            let mut file = ConfigFile::default();
            file.capco.default_timezone = Some(tz.to_owned());
            assert!(
                merge_project_into(&mut c, file).is_ok(),
                "should accept timezone {tz:?}"
            );
        }
    }

    #[cfg(feature = "toml-loader")]
    #[test]
    fn merge_project_timezone_sets_correct_offset() {
        let mut c = Config::default();
        let mut file = ConfigFile::default();
        file.capco.default_timezone = Some("+05:30".to_owned());
        merge_project_into(&mut c, file).unwrap();
        assert_eq!(
            c.capco.default_timezone,
            UtcOffset::from_hhmm(1, 5, 30).unwrap()
        );
    }

    #[cfg(feature = "toml-loader")]
    #[test]
    fn merge_project_rejects_invalid_timezone() {
        for bad in ["EST", "UTC", "utc", "+0530", "+05-30", "05:30"] {
            let mut c = Config::default();
            let mut file = ConfigFile::default();
            file.capco.default_timezone = Some(bad.to_owned());
            assert!(
                matches!(
                    merge_project_into(&mut c, file),
                    Err(ConfigError::InvalidTimezone { .. })
                ),
                "should reject timezone {bad:?}"
            );
        }
    }

    #[test]
    fn utc_offset_from_str_z_is_utc() {
        // Exercising UtcOffset::from_str through the config-layer parse path.
        assert_eq!("Z".parse::<UtcOffset>().unwrap(), UtcOffset::UTC);
    }

    #[test]
    fn utc_offset_from_str_wrong_separator_is_err() {
        // `+05-30` has `-` instead of `:` at index 3.
        assert!("+05-30".parse::<UtcOffset>().is_err());
    }

    #[test]
    fn utc_offset_from_str_out_of_range_is_err() {
        // Hours > 23 must be rejected.
        assert!("+24:00".parse::<UtcOffset>().is_err());
    }
}
