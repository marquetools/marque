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

/// Create a unique tempdir with a process-id + test-name discriminator.
fn make_tmpdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("marque-prec-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create tmpdir");
    dir
}

/// The compiled schema version — config files must use this to pass FR-011.
const SCHEMA_VERSION: &str = marque_ism::generated::values::SCHEMA_VERSION;

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

    // Env vars are process-global, so use a distinguishing value.
    // Safety: env var manipulation in tests is inherently non-thread-safe;
    // cargo test runs integration test binaries in separate processes, so
    // this is fine for integration tests (separate binary per test file).
    unsafe {
        std::env::set_var("MARQUE_CLASSIFIER_ID", "ENV-99");
    }
    let config = marque_config::load(&dir);
    unsafe {
        std::env::remove_var("MARQUE_CLASSIFIER_ID");
    }

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

    unsafe {
        std::env::set_var("MARQUE_CONFIDENCE_THRESHOLD", "0.5");
    }
    let config = marque_config::load(&dir);
    unsafe {
        std::env::remove_var("MARQUE_CONFIDENCE_THRESHOLD");
    }

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

    let config = marque_config::load(&dir).expect("load should succeed");
    assert!(
        config.user.classifier_id.is_none(),
        "empty classifier_id should be treated as not set"
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

    unsafe {
        std::env::set_var("MARQUE_CONFIDENCE_THRESHOLD", "bananas");
    }
    let result = marque_config::load(&dir);
    unsafe {
        std::env::remove_var("MARQUE_CONFIDENCE_THRESHOLD");
    }

    let err = result.unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidEnvVar { .. }),
        "expected InvalidEnvVar, got: {err:?}"
    );
    assert_eq!(err.exit_code(), 65);
    let _ = fs::remove_dir_all(&dir);
}
