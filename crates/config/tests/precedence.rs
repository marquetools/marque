// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase 5 — Config precedence chain and hard-fail validator tests (T052, T053).
//!
//! Tests the four-layer precedence chain (FR-007):
//!   committed `.marque.toml` → `.marque.local.toml` → env vars → CLI flags
//!
//! And the three hard-fail scenarios from `contracts/cli.md`:
//!   1. `[user]` section in committed config (FR-010, SC-006) → exit 65
//!   2. Schema version mismatch (FR-011) → exit 65
//!   3. Confidence threshold out of range → exit 65

use marque_config::ConfigError;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

/// Create a unique tempdir with a process-id + test-name discriminator.
fn make_tmpdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("marque-prec-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create tmpdir");
    dir
}

/// The compiled schema version — config files must use this to pass FR-011.
const SCHEMA_VERSION: &str = marque_ism::generated::values::SCHEMA_VERSION;

/// Global mutex serializing all env-var access in this test binary.
///
/// Environment variables are process-global state. Tests within the same
/// integration-test binary can run in parallel, so without serialization
/// one test's `set_var` can race with another test's `load()` call.
/// Every test that calls `marque_config::load()` must hold this lock —
/// not just tests that set env vars — because `load()` reads env vars
/// internally (`MARQUE_CLASSIFIER_ID`, `MARQUE_CONFIDENCE_THRESHOLD`).
///
/// **Scope**: this mutex serializes threads within this test binary only.
/// Different integration-test binaries are separate OS processes, each
/// with their own copy of this static. Cross-binary races are impossible
/// because each process has its own environment. If a future test file
/// in this crate also touches env vars, it needs its own mutex or must
/// be merged into this file.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// RAII guard: saves the previous value of `var`, sets it to `value`,
/// and restores the original on drop. Caller must hold `ENV_MUTEX`.
struct EnvGuard {
    var: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn set(var: &'static str, value: &str) -> Self {
        let previous = std::env::var(var).ok();
        // SAFETY: single-threaded access is ensured by the caller holding ENV_MUTEX.
        unsafe { std::env::set_var(var, value) };
        Self { var, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: single-threaded access is ensured by the caller holding ENV_MUTEX.
        unsafe {
            match &self.previous {
                Some(v) => std::env::set_var(self.var, v),
                None => std::env::remove_var(self.var),
            }
        }
    }
}

// -----------------------------------------------------------------------
// T052: Four-layer precedence chain
// -----------------------------------------------------------------------

#[test]
fn layer1_project_config_sets_rule_severity() {
    let dir = make_tmpdir("l1-severity");
    fs::write(
        dir.join(".marque.toml"),
        format!(
            r#"
[capco]
version = "{SCHEMA_VERSION}"

[rules]
E001 = "warn"
"#
        ),
    )
    .unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let config = marque_config::load(&dir).expect("load should succeed");
    assert_eq!(
        config.rules.overrides.get("E001"),
        Some(&"warn".to_owned()),
        "project config should set E001 to warn"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn layer1_project_config_sets_corrections_map() {
    let dir = make_tmpdir("l1-corrections");
    fs::write(
        dir.join(".marque.toml"),
        format!(
            r#"
[capco]
version = "{SCHEMA_VERSION}"

[corrections]
SERCET = "SECRET"
SECERT = "SECRET"
"#
        ),
    )
    .unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let config = marque_config::load(&dir).expect("load should succeed");
    assert_eq!(config.corrections.get("SERCET"), Some(&"SECRET".to_owned()));
    assert_eq!(config.corrections.get("SECERT"), Some(&"SECRET".to_owned()));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn layer1_project_config_sets_confidence_threshold() {
    let dir = make_tmpdir("l1-threshold");
    // confidence_threshold is a top-level key, must appear BEFORE [table] sections.
    fs::write(
        dir.join(".marque.toml"),
        format!("confidence_threshold = 0.8\n\n[capco]\nversion = \"{SCHEMA_VERSION}\"\n"),
    )
    .unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let config = marque_config::load(&dir).expect("load should succeed");
    assert!((config.confidence_threshold() - 0.8).abs() < f32::EPSILON);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn layer2_local_config_sets_classifier_id() {
    let dir = make_tmpdir("l2-classifier");
    fs::write(
        dir.join(".marque.toml"),
        format!(
            r#"
[capco]
version = "{SCHEMA_VERSION}"
"#
        ),
    )
    .unwrap();
    fs::write(
        dir.join(".marque.local.toml"),
        r#"
[user]
classifier_id = "LOCAL-42"
"#,
    )
    .unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let config = marque_config::load(&dir).expect("load should succeed");
    assert_eq!(config.user.classifier_id.as_deref(), Some("LOCAL-42"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn layer3_env_overrides_local_config_classifier_id() {
    let dir = make_tmpdir("l3-env-classifier");
    fs::write(
        dir.join(".marque.toml"),
        format!(
            r#"
[capco]
version = "{SCHEMA_VERSION}"
"#
        ),
    )
    .unwrap();
    fs::write(
        dir.join(".marque.local.toml"),
        r#"
[user]
classifier_id = "LOCAL-42"
"#,
    )
    .unwrap();

    // Serialize env-var access so parallel test threads don't race.
    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _env = EnvGuard::set("MARQUE_CLASSIFIER_ID", "ENV-99");
    let config = marque_config::load(&dir);

    let config = config.expect("load should succeed");
    assert_eq!(
        config.user.classifier_id.as_deref(),
        Some("ENV-99"),
        "env var should override local config"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn layer3_env_overrides_project_config_threshold() {
    let dir = make_tmpdir("l3-env-threshold");
    fs::write(
        dir.join(".marque.toml"),
        format!("confidence_threshold = 0.8\n\n[capco]\nversion = \"{SCHEMA_VERSION}\"\n"),
    )
    .unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _env = EnvGuard::set("MARQUE_CONFIDENCE_THRESHOLD", "0.5");
    let config = marque_config::load(&dir);

    let config = config.expect("load should succeed");
    assert!(
        (config.confidence_threshold() - 0.5).abs() < f32::EPSILON,
        "env var threshold should override project config"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn defaults_when_no_config_files() {
    // A dir with .git/ but no .marque.toml → defaults.
    let dir = make_tmpdir("defaults");
    fs::create_dir_all(dir.join(".git")).unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let config = marque_config::load(&dir).expect("load should succeed with defaults");
    assert!(config.rules.overrides.is_empty());
    assert!(config.corrections.is_empty());
    assert!((config.confidence_threshold() - 0.95).abs() < f32::EPSILON);
    assert!(config.user.classifier_id.is_none());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn empty_classifier_id_treated_as_not_set() {
    // L-2: an empty string is semantically "not set".
    let dir = make_tmpdir("empty-classifier");
    fs::write(
        dir.join(".marque.toml"),
        format!(
            r#"
[capco]
version = "{SCHEMA_VERSION}"
"#
        ),
    )
    .unwrap();
    fs::write(
        dir.join(".marque.local.toml"),
        r#"
[user]
classifier_id = ""
"#,
    )
    .unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let config = marque_config::load(&dir).expect("load should succeed");
    assert!(
        config.user.classifier_id.is_none(),
        "empty classifier_id should be treated as not set"
    );
    let _ = fs::remove_dir_all(&dir);
}

// F-07: local config intentionally carries only user identity, not rules.
#[test]
fn layer2_local_config_does_not_override_rule_severities() {
    let dir = make_tmpdir("l2-rules-ignored");
    fs::write(
        dir.join(".marque.toml"),
        format!(
            r#"
[capco]
version = "{SCHEMA_VERSION}"

[rules]
E001 = "error"
"#
        ),
    )
    .unwrap();
    // .marque.local.toml may contain a [rules] section, but merge_user_into
    // only picks up [user] fields — rule overrides from local config are
    // silently ignored. This is by design: local config is for user identity,
    // not project policy.
    fs::write(
        dir.join(".marque.local.toml"),
        r#"
[rules]
E001 = "off"
"#,
    )
    .unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let config = marque_config::load(&dir).expect("load should succeed");
    assert_eq!(
        config.rules.overrides.get("E001"),
        Some(&"error".to_owned()),
        "local config must not override project rule severity"
    );
    let _ = fs::remove_dir_all(&dir);
}

// -----------------------------------------------------------------------
// T053: Hard-fail scenarios
// -----------------------------------------------------------------------

#[test]
fn hard_fail_user_section_in_committed_config() {
    let dir = make_tmpdir("hf-user");
    fs::write(
        dir.join(".marque.toml"),
        format!(
            r#"
[capco]
version = "{SCHEMA_VERSION}"

[user]
classifier_id = "LEAKED-42"
"#
        ),
    )
    .unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let err = marque_config::load(&dir).unwrap_err();
    assert!(
        matches!(err, ConfigError::UserSectionInCommitted { .. }),
        "expected UserSectionInCommitted, got: {err:?}"
    );
    assert_eq!(err.exit_code(), 65, "exit code must be 65 (EX_DATAERR)");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn hard_fail_schema_version_mismatch() {
    let dir = make_tmpdir("hf-schema");
    fs::write(
        dir.join(".marque.toml"),
        r#"
[capco]
version = "ISM-v1999-WRONG"
"#,
    )
    .unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let err = marque_config::load(&dir).unwrap_err();
    assert!(
        matches!(err, ConfigError::SchemaVersionMismatch { .. }),
        "expected SchemaVersionMismatch, got: {err:?}"
    );
    assert_eq!(err.exit_code(), 65);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn hard_fail_threshold_out_of_range() {
    let dir = make_tmpdir("hf-threshold");
    fs::write(
        dir.join(".marque.toml"),
        format!("confidence_threshold = 2.0\n\n[capco]\nversion = \"{SCHEMA_VERSION}\"\n"),
    )
    .unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let err = marque_config::load(&dir).unwrap_err();
    assert!(
        matches!(err, ConfigError::ThresholdOutOfRange { .. }),
        "expected ThresholdOutOfRange, got: {err:?}"
    );
    assert_eq!(err.exit_code(), 65);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn hard_fail_unknown_severity_string() {
    let dir = make_tmpdir("hf-severity");
    fs::write(
        dir.join(".marque.toml"),
        format!(
            r#"
[capco]
version = "{SCHEMA_VERSION}"

[rules]
E001 = "err"
"#
        ),
    )
    .unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let err = marque_config::load(&dir).unwrap_err();
    assert!(
        matches!(err, ConfigError::UnknownSeverity { .. }),
        "expected UnknownSeverity, got: {err:?}"
    );
    assert_eq!(err.exit_code(), 65);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn hard_fail_env_threshold_not_a_float() {
    let dir = make_tmpdir("hf-env-parse");
    fs::create_dir_all(dir.join(".git")).unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _env = EnvGuard::set("MARQUE_CONFIDENCE_THRESHOLD", "bananas");
    let result = marque_config::load(&dir);

    let err = result.unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidEnvVar { .. }),
        "expected InvalidEnvVar, got: {err:?}"
    );
    assert_eq!(err.exit_code(), 65);
    let _ = fs::remove_dir_all(&dir);
}

// F-11: NaN parses as f32 but must be rejected by set_confidence_threshold.
#[test]
fn hard_fail_env_threshold_nan() {
    let dir = make_tmpdir("hf-env-nan");
    fs::create_dir_all(dir.join(".git")).unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _env = EnvGuard::set("MARQUE_CONFIDENCE_THRESHOLD", "NaN");
    let result = marque_config::load(&dir);

    let err = result.unwrap_err();
    assert!(
        matches!(err, ConfigError::ThresholdOutOfRange { .. }),
        "NaN threshold via env var should be rejected, got: {err:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

// F-02: empty MARQUE_CLASSIFIER_ID must not overwrite a populated local value.
#[test]
fn empty_env_classifier_id_does_not_overwrite_local() {
    let dir = make_tmpdir("env-empty-classifier");
    fs::write(
        dir.join(".marque.toml"),
        format!(
            r#"
[capco]
version = "{SCHEMA_VERSION}"
"#
        ),
    )
    .unwrap();
    fs::write(
        dir.join(".marque.local.toml"),
        r#"
[user]
classifier_id = "LOCAL-42"
"#,
    )
    .unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _env = EnvGuard::set("MARQUE_CLASSIFIER_ID", "");
    let config = marque_config::load(&dir);

    let config = config.expect("load should succeed");
    assert_eq!(
        config.user.classifier_id.as_deref(),
        Some("LOCAL-42"),
        "empty env var must not overwrite populated local classifier_id"
    );
    let _ = fs::remove_dir_all(&dir);
}

// -----------------------------------------------------------------------
// C-3: MARQUE_DEFAULT_TIMEZONE env-var path (IsmDate / PR #229)
// -----------------------------------------------------------------------

/// C-3a: a valid ISO 8601 UTC offset in MARQUE_DEFAULT_TIMEZONE is applied.
#[test]
fn env_default_timezone_sets_offset() {
    let dir = make_tmpdir("tz-env-set");
    fs::create_dir_all(dir.join(".git")).unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _tz = EnvGuard::set("MARQUE_DEFAULT_TIMEZONE", "+05:30");
    let config = marque_config::load(&dir).expect("load should succeed");

    let expected = marque_ism::date::UtcOffset::from_hhmm(1, 5, 30).unwrap();
    assert_eq!(
        config.capco.default_timezone, expected,
        "MARQUE_DEFAULT_TIMEZONE=+05:30 should set default_timezone to +05:30"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// C-3b: MARQUE_DEFAULT_TIMEZONE=Z sets UTC.
#[test]
fn env_default_timezone_z_is_utc() {
    let dir = make_tmpdir("tz-env-z");
    fs::create_dir_all(dir.join(".git")).unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _tz = EnvGuard::set("MARQUE_DEFAULT_TIMEZONE", "Z");
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(
        config.capco.default_timezone,
        marque_ism::date::UtcOffset::UTC,
        "MARQUE_DEFAULT_TIMEZONE=Z should set default_timezone to UTC"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// C-3c: an invalid MARQUE_DEFAULT_TIMEZONE value must return InvalidEnvVar.
#[test]
fn env_default_timezone_invalid_value_errors() {
    let dir = make_tmpdir("tz-env-bad");
    fs::create_dir_all(dir.join(".git")).unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _tz = EnvGuard::set("MARQUE_DEFAULT_TIMEZONE", "EST");
    let err = marque_config::load(&dir).unwrap_err();

    assert!(
        matches!(
            err,
            ConfigError::InvalidEnvVar {
                var: "MARQUE_DEFAULT_TIMEZONE",
                ..
            }
        ),
        "invalid timezone env var should produce InvalidEnvVar, got: {err:?}"
    );
    assert_eq!(err.exit_code(), 65, "exit code must be 65 (EX_DATAERR)");
    let _ = fs::remove_dir_all(&dir);
}

/// C-3d: a whitespace-only MARQUE_DEFAULT_TIMEZONE must be treated as not-set.
#[test]
fn env_default_timezone_empty_string_is_ignored() {
    let dir = make_tmpdir("tz-env-empty");
    fs::create_dir_all(dir.join(".git")).unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _tz = EnvGuard::set("MARQUE_DEFAULT_TIMEZONE", "");
    let config = marque_config::load(&dir).expect("load should succeed");

    // Empty string is treated as not-set → default remains UTC.
    assert_eq!(
        config.capco.default_timezone,
        marque_ism::date::UtcOffset::UTC,
        "empty MARQUE_DEFAULT_TIMEZONE should leave default_timezone as UTC"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// C-3e: MARQUE_DEFAULT_TIMEZONE overrides a project-file value.
#[test]
fn env_default_timezone_overrides_project_file() {
    let dir = make_tmpdir("tz-env-override");
    fs::write(
        dir.join(".marque.toml"),
        format!(
            r#"
[capco]
version = "{SCHEMA_VERSION}"
default_timezone = "-05:00"
"#
        ),
    )
    .unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _tz = EnvGuard::set("MARQUE_DEFAULT_TIMEZONE", "+09:00");
    let config = marque_config::load(&dir).expect("load should succeed");

    let expected = marque_ism::date::UtcOffset::from_hhmm(1, 9, 0).unwrap();
    assert_eq!(
        config.capco.default_timezone, expected,
        "env var +09:00 should override project-file -05:00"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// C-3f: project-file default_timezone is applied when no env var is set.
#[test]
fn project_file_default_timezone_is_applied() {
    let dir = make_tmpdir("tz-project-file");
    fs::write(
        dir.join(".marque.toml"),
        format!(
            r#"
[capco]
version = "{SCHEMA_VERSION}"
default_timezone = "+05:30"
"#
        ),
    )
    .unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    // Ensure env var is absent.
    let _tz = EnvGuard::set("MARQUE_DEFAULT_TIMEZONE", "");
    let config = marque_config::load(&dir).expect("load should succeed");

    let expected = marque_ism::date::UtcOffset::from_hhmm(1, 5, 30).unwrap();
    assert_eq!(
        config.capco.default_timezone, expected,
        "project-file default_timezone=+05:30 should be applied"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// C-3g: invalid default_timezone in project config file produces InvalidTimezone.
#[test]
fn project_file_invalid_timezone_errors() {
    let dir = make_tmpdir("tz-project-bad");
    fs::write(
        dir.join(".marque.toml"),
        format!(
            r#"
[capco]
version = "{SCHEMA_VERSION}"
default_timezone = "PST"
"#
        ),
    )
    .unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let err = marque_config::load(&dir).unwrap_err();

    assert!(
        matches!(err, ConfigError::InvalidTimezone { .. }),
        "invalid project-file timezone should produce InvalidTimezone, got: {err:?}"
    );
    assert_eq!(err.exit_code(), 65);
    let _ = fs::remove_dir_all(&dir);
}
