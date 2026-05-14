// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Stage F — `[closure_rules]` config section tests (PR 3.7 T108f).
//!
//! Verifies per-closure-rule severity overrides, section isolation from
//! `[rules]`, `Severity::Fix` rejection, TOML quoted-key parsing, and
//! the `MARQUE_CLOSURE_RULES_*` env-var namespace.
//!
//! See `decisions.md` D19 B and `docs/plans/2026-05-13-pr3.7-lattice-resolution-gate-plan.md`
//! Stage F for the binding spec.

use marque_config::ConfigError;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Create a unique tempdir with a process-id + test-name discriminator.
fn make_tmpdir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("marque-closure-test-{name}-{}", std::process::id()));
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
/// internally via `apply_env`.
///
/// Scope: this mutex serializes threads within this test binary only.
/// Different integration-test binaries are separate OS processes with
/// their own environment copies.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// RAII guard: saves the previous value of `var`, sets it to `value`,
/// and restores the original on drop. Caller must hold `ENV_MUTEX`.
struct EnvGuard {
    var: String,
    previous: Option<String>,
}

impl EnvGuard {
    fn set(var: impl Into<String>, value: &str) -> Self {
        let var = var.into();
        let previous = std::env::var(&var).ok();
        // SAFETY: single-threaded access is ensured by the caller holding ENV_MUTEX.
        unsafe { std::env::set_var(&var, value) };
        Self { var, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: single-threaded access is ensured by the caller holding ENV_MUTEX.
        unsafe {
            match &self.previous {
                Some(v) => std::env::set_var(&self.var, v),
                None => std::env::remove_var(&self.var),
            }
        }
    }
}

/// Write a standard `.marque.toml` with the compiled schema version and
/// an optional `[closure_rules]` payload appended.
fn write_project_config(dir: &Path, closure_rules_section: &str) {
    let content = format!("[capco]\nversion = \"{SCHEMA_VERSION}\"\n\n{closure_rules_section}");
    fs::write(dir.join(".marque.toml"), content).unwrap();
}

// ---------------------------------------------------------------------------
// Category 1: Default behavior — empty [closure_rules] section
// ---------------------------------------------------------------------------

/// An empty `[closure_rules]` section (or absent section) must produce no
/// overrides in `config.closure_rules.overrides`.
#[test]
fn default_empty_closure_rules() {
    let dir = make_tmpdir("closure-defaults");
    write_project_config(&dir, "");

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let config = marque_config::load(&dir).expect("load should succeed");

    assert!(
        config.closure_rules.overrides.is_empty(),
        "absent [closure_rules] section must produce no overrides"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// An explicit but empty `[closure_rules]` section must produce no overrides.
#[test]
fn explicit_empty_closure_rules_section() {
    let dir = make_tmpdir("closure-explicit-empty");
    write_project_config(&dir, "[closure_rules]\n");

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let config = marque_config::load(&dir).expect("load should succeed");

    assert!(
        config.closure_rules.overrides.is_empty(),
        "explicit empty [closure_rules] section must produce no overrides"
    );
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Category 2: Per-row overrides for each valid severity
// ---------------------------------------------------------------------------

/// `[closure_rules] "capco/foo" = "off"` must round-trip as "off".
#[test]
fn closure_rules_severity_off() {
    let dir = make_tmpdir("closure-off");
    write_project_config(
        &dir,
        "[closure_rules]\n\"capco/noforn-if-no-fdr\" = \"off\"\n",
    );

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco/noforn-if-no-fdr")
            .map(String::as_str),
        Some("off"),
        "closure rule severity 'off' must round-trip"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// `[closure_rules] "capco/foo" = "suggest"` must round-trip as "suggest".
#[test]
fn closure_rules_severity_suggest() {
    let dir = make_tmpdir("closure-suggest");
    write_project_config(
        &dir,
        "[closure_rules]\n\"capco/noforn-if-no-fdr\" = \"suggest\"\n",
    );

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco/noforn-if-no-fdr")
            .map(String::as_str),
        Some("suggest"),
        "closure rule severity 'suggest' must round-trip"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// `[closure_rules] "capco/foo" = "info"` must round-trip as "info".
#[test]
fn closure_rules_severity_info() {
    let dir = make_tmpdir("closure-info");
    write_project_config(
        &dir,
        "[closure_rules]\n\"capco/noforn-if-no-fdr\" = \"info\"\n",
    );

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco/noforn-if-no-fdr")
            .map(String::as_str),
        Some("info"),
        "closure rule severity 'info' must round-trip"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// `[closure_rules] "capco/foo" = "warn"` must round-trip as "warn".
#[test]
fn closure_rules_severity_warn() {
    let dir = make_tmpdir("closure-warn");
    write_project_config(
        &dir,
        "[closure_rules]\n\"capco/noforn-if-no-fdr\" = \"warn\"\n",
    );

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco/noforn-if-no-fdr")
            .map(String::as_str),
        Some("warn"),
        "closure rule severity 'warn' must round-trip"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// `[closure_rules] "capco/foo" = "error"` must round-trip as "error".
#[test]
fn closure_rules_severity_error() {
    let dir = make_tmpdir("closure-error");
    write_project_config(
        &dir,
        "[closure_rules]\n\"capco/noforn-if-no-fdr\" = \"error\"\n",
    );

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco/noforn-if-no-fdr")
            .map(String::as_str),
        Some("error"),
        "closure rule severity 'error' must round-trip"
    );
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Category 3: Fix rejection at load
// ---------------------------------------------------------------------------

/// `[closure_rules] "capco/foo" = "fix"` must fail with
/// `ConfigError::InvalidClosureRuleSeverity` at config load.
///
/// Closure firings propagate facts, not byte-level edits — "fix" severity
/// is meaningless and rejected. See decisions.md D19 B.
#[test]
fn closure_rules_fix_severity_rejected() {
    let dir = make_tmpdir("closure-fix-rejected");
    write_project_config(
        &dir,
        "[closure_rules]\n\"capco/noforn-if-no-fdr\" = \"fix\"\n",
    );

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let err = marque_config::load(&dir).unwrap_err();

    match &err {
        ConfigError::InvalidClosureRuleSeverity { rule, hint } => {
            assert_eq!(rule, "capco/noforn-if-no-fdr");
            // The hint must mention an alternative severity to guide the user.
            assert!(
                hint.contains("fix") || hint.contains("fact") || hint.contains("byte"),
                "hint should explain why 'fix' is rejected; got: {hint:?}"
            );
        }
        other => panic!("expected InvalidClosureRuleSeverity, got: {other:?}"),
    }

    // The error message must include both the rule name and useful guidance.
    let msg = err.to_string();
    assert!(
        msg.contains("capco/noforn-if-no-fdr"),
        "error message must include the rule name; got: {msg:?}"
    );
    // The message must suggest a valid alternative (per plan §1.5b / preflight F4).
    assert!(
        msg.contains("warn") || msg.contains("error"),
        "error message must suggest 'warn' or 'error'; got: {msg:?}"
    );

    assert_eq!(
        err.exit_code(),
        65,
        "InvalidClosureRuleSeverity must exit with EX_DATAERR (65)"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Unknown severity string in `[closure_rules]` must fail with `UnknownSeverity`
/// (not `InvalidClosureRuleSeverity`).
#[test]
fn closure_rules_unknown_severity_rejected() {
    let dir = make_tmpdir("closure-unknown-severity");
    write_project_config(
        &dir,
        "[closure_rules]\n\"capco/noforn-if-no-fdr\" = \"err\"\n",
    );

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let err = marque_config::load(&dir).unwrap_err();

    assert!(
        matches!(err, ConfigError::UnknownSeverity { .. }),
        "unknown severity string must produce UnknownSeverity, got: {err:?}"
    );
    assert_eq!(err.exit_code(), 65);
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Category 4: Section isolation
// ---------------------------------------------------------------------------

/// A key present in both `[rules]` and `[closure_rules]` must resolve
/// independently in each section — they must NOT cross-talk.
///
/// This test uses the same string `"capco/foo"` as a key in both sections
/// (both are `HashMap<String, String>` at the file level) and asserts the
/// two overrides land in their respective config fields without interference.
#[test]
fn rules_and_closure_rules_are_section_isolated() {
    let dir = make_tmpdir("closure-isolation");
    let content = format!(
        "[capco]\nversion = \"{SCHEMA_VERSION}\"\n\n\
        [rules]\n\
        \"capco/foo\" = \"warn\"\n\n\
        [closure_rules]\n\
        \"capco/foo\" = \"error\"\n"
    );
    fs::write(dir.join(".marque.toml"), content).unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let config = marque_config::load(&dir).expect("load should succeed");

    // [rules] → config.rules.overrides
    assert_eq!(
        config.rules.overrides.get("capco/foo").map(String::as_str),
        Some("warn"),
        "[rules] capco/foo must be 'warn'"
    );

    // [closure_rules] → config.closure_rules.overrides
    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco/foo")
            .map(String::as_str),
        Some("error"),
        "[closure_rules] capco/foo must be 'error'"
    );

    // Cross-talk: [rules] must not see the closure value and vice versa.
    assert_ne!(
        config.rules.overrides.get("capco/foo").map(String::as_str),
        config
            .closure_rules
            .overrides
            .get("capco/foo")
            .map(String::as_str),
        "rules and closure_rules must NOT cross-talk for the same key"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// A plain `[rules]` entry with a slash-containing ID (valid per the `RuleId`
/// doc) must not bleed into `[closure_rules]`.
#[test]
fn rules_section_does_not_populate_closure_rules() {
    let dir = make_tmpdir("closure-no-bleed");
    write_project_config(&dir, "[rules]\n\"E001\" = \"warn\"\n");

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let config = marque_config::load(&dir).expect("load should succeed");

    assert!(
        config.closure_rules.overrides.is_empty(),
        "[closure_rules] must be empty when only [rules] is populated"
    );
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Category 5: TOML quoted-key form
// ---------------------------------------------------------------------------

/// Closure-rule names containing `/` require TOML quoted-key form.
/// Verify that the full example from plan §1.5 / rust-preflight B4 parses
/// correctly.
#[test]
fn closure_rules_quoted_key_with_slash_parses() {
    let dir = make_tmpdir("closure-quoted-key");
    // Both keys use the slash-containing form that TOML requires quoting for.
    let content = format!(
        "[capco]\nversion = \"{SCHEMA_VERSION}\"\n\n\
        [closure_rules]\n\
        \"capco/noforn-if-no-fdr\" = \"warn\"\n\
        \"capco/relido-if-no-fdr\" = \"off\"\n"
    );
    fs::write(dir.join(".marque.toml"), content).unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco/noforn-if-no-fdr")
            .map(String::as_str),
        Some("warn"),
        "capco/noforn-if-no-fdr should be 'warn'"
    );
    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco/relido-if-no-fdr")
            .map(String::as_str),
        Some("off"),
        "capco/relido-if-no-fdr should be 'off'"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Multiple entries in `[closure_rules]` must all be loaded.
#[test]
fn closure_rules_multiple_entries_all_loaded() {
    let dir = make_tmpdir("closure-multi");
    let content = format!(
        "[capco]\nversion = \"{SCHEMA_VERSION}\"\n\n\
        [closure_rules]\n\
        \"capco/noforn-if-no-fdr\" = \"warn\"\n\
        \"capco/relido-if-no-fdr\" = \"off\"\n\
        \"capco/sci-requires-ts\" = \"error\"\n"
    );
    fs::write(dir.join(".marque.toml"), content).unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(config.closure_rules.overrides.len(), 3);
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Category 6: Env-var override (MARQUE_CLOSURE_RULES_*)
// ---------------------------------------------------------------------------

/// `MARQUE_CLOSURE_RULES_CAPCO__NOFORN_IF_NO_FDR=warn` must override a
/// file-level value for `"capco/noforn-if-no-fdr"`.
///
/// Naming convention: `__` → `/`, entire suffix lowercased.
/// e.g. `CAPCO__NOFORN_IF_NO_FDR` → `capco/noforn-if-no-fdr`
#[test]
fn env_var_overrides_closure_rule_file_value() {
    let dir = make_tmpdir("closure-env-override");
    // Project config sets "info"; env var overrides to "warn".
    write_project_config(
        &dir,
        "[closure_rules]\n\"capco/noforn-if-no-fdr\" = \"info\"\n",
    );

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _env = EnvGuard::set("MARQUE_CLOSURE_RULES_CAPCO__NOFORN_IF_NO_FDR", "warn");
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco/noforn-if-no-fdr")
            .map(String::as_str),
        Some("warn"),
        "MARQUE_CLOSURE_RULES_* env var must override file-level value"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// `MARQUE_CLOSURE_RULES_CAPCO__NOFORN_IF_NO_FDR=info` without a file-level
/// entry must add the override to `config.closure_rules.overrides`.
#[test]
fn env_var_adds_closure_rule_when_absent_in_file() {
    let dir = make_tmpdir("closure-env-add");
    write_project_config(&dir, "");

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _env = EnvGuard::set("MARQUE_CLOSURE_RULES_CAPCO__RELIDO_IF_NO_FDR", "error");
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco/relido-if-no-fdr")
            .map(String::as_str),
        Some("error"),
        "MARQUE_CLOSURE_RULES_* env var must add entry absent in file"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// `MARQUE_CLOSURE_RULES_CAPCO__FOO=fix` must be rejected with
/// `ConfigError::InvalidClosureRuleSeverity` — the same Fix-rejection
/// logic as the file-level path.
#[test]
fn env_var_fix_severity_rejected() {
    let dir = make_tmpdir("closure-env-fix-rejected");
    write_project_config(&dir, "");

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _env = EnvGuard::set("MARQUE_CLOSURE_RULES_CAPCO__FOO", "fix");
    let err = marque_config::load(&dir).unwrap_err();

    assert!(
        matches!(err, ConfigError::InvalidClosureRuleSeverity { .. }),
        "MARQUE_CLOSURE_RULES_* with 'fix' must produce InvalidClosureRuleSeverity, got: {err:?}"
    );
    assert_eq!(err.exit_code(), 65);
    let _ = fs::remove_dir_all(&dir);
}

/// `MARQUE_CLOSURE_RULES_CAPCO__FOO=err` (unknown) must be rejected with
/// `ConfigError::UnknownSeverity`.
#[test]
fn env_var_unknown_severity_rejected() {
    let dir = make_tmpdir("closure-env-unknown");
    write_project_config(&dir, "");

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _env = EnvGuard::set("MARQUE_CLOSURE_RULES_CAPCO__FOO", "err");
    let err = marque_config::load(&dir).unwrap_err();

    assert!(
        matches!(err, ConfigError::UnknownSeverity { .. }),
        "MARQUE_CLOSURE_RULES_* with unknown severity must produce UnknownSeverity, got: {err:?}"
    );
    assert_eq!(err.exit_code(), 65);
    let _ = fs::remove_dir_all(&dir);
}

/// Non-`MARQUE_CLOSURE_RULES_*` env vars must not affect `closure_rules`.
#[test]
fn unrelated_env_vars_do_not_affect_closure_rules() {
    let dir = make_tmpdir("closure-env-unrelated");
    write_project_config(&dir, "");

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    // MARQUE_RULES_* does not exist yet; MARQUE_CLASSIFIER_ID should not bleed in.
    let _env1 = EnvGuard::set("MARQUE_CLASSIFIER_ID", "test-classifier");
    let config = marque_config::load(&dir).expect("load should succeed");

    assert!(
        config.closure_rules.overrides.is_empty(),
        "unrelated env vars must not populate closure_rules.overrides"
    );
    let _ = fs::remove_dir_all(&dir);
}
