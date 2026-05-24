// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

// Whole file gated on `toml-loader`: every test invokes `marque_config::load()`
// which is itself gated on the same feature (issue #454, WASM size).
#![cfg(feature = "toml-loader")]

//! `[closure_rules]` config section tests.
//!
//! Verifies per-closure-rule severity overrides, section isolation from
//! `[rules]`, `Severity::Fix` rejection, TOML quoted-key parsing, and
//! the `MARQUE_CLOSURE_RULES_*` env-var namespace.
//!
//! # Wire-string convention
//!
//! Closure-rule keys take the wire-string form
//! `<scheme>:closure.<category>.<predicate>` — e.g.,
//! `"capco:closure.dissem.noforn-if-caveated"` (matching the
//! `closure_table.rs::CLOSURE_TABLE` row inventory in
//! `crates/capco/src/scheme/closure_table.rs`).
//!
//! The engine does not yet consume `closure_rules.overrides` on the
//! hot path — the config map is a forward-looking surface for the
//! eventual closure-rule severity dispatch. Tests here pin the
//! wire-string convention so whoever wires the config-to-engine path
//! later starts with the right key shape.
//!
//! The `MARQUE_CLOSURE_RULES_*` env-var encoding in
//! `crates/config/src/lib.rs::env_var_to_closure_rule_name` uses the
//! same wire-string form. The encoder splits the env-var suffix on `__` into
//! N segments: the first becomes the scheme (joined with `:` to the
//! predicate), subsequent segments join with `.`; single `_` within
//! a segment becomes `-`. Example:
//! `MARQUE_CLOSURE_RULES_CAPCO__CLOSURE__DISSEM__NOFORN_IF_CAVEATED`
//! → `"capco:closure.dissem.noforn-if-caveated"` — matching the
//! `.marque.toml [closure_rules]` key shape verbatim, so env-var
//! and file-level overrides converge on the same map key.

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

/// The compiled schema version — config files must use this to pass
/// the schema-version validator.
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

/// RAII guard: removes every ambient `MARQUE_CLOSURE_RULES_*` env var
/// from the process environment for the duration of the test, then
/// restores them on drop. Tests that assert an EMPTY `closure_rules`
/// overrides map must use this guard to avoid false failures from
/// ambient env vars in the developer/CI shell.
///
/// Per Copilot PR 3.7 review #4: `load()` imports every variable
/// matching the `MARQUE_CLOSURE_RULES_*` pattern, so a stray env var
/// in the test environment would cause `closure_rules.overrides` to
/// be non-empty even for tests that explicitly write an empty
/// `[closure_rules]` section. Caller must hold `ENV_MUTEX`.
struct AmbientClosureEnvCleanGuard {
    saved: Vec<(String, String)>,
}

impl AmbientClosureEnvCleanGuard {
    fn new() -> Self {
        let saved: Vec<(String, String)> = std::env::vars()
            .filter(|(k, _)| k.starts_with("MARQUE_CLOSURE_RULES_"))
            .collect();
        // SAFETY: single-threaded access ensured by caller holding ENV_MUTEX.
        unsafe {
            for (k, _) in &saved {
                std::env::remove_var(k);
            }
        }
        Self { saved }
    }
}

impl Drop for AmbientClosureEnvCleanGuard {
    fn drop(&mut self) {
        // SAFETY: single-threaded access ensured by caller holding ENV_MUTEX.
        unsafe {
            for (k, v) in &self.saved {
                std::env::set_var(k, v);
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
    // Clear ambient MARQUE_CLOSURE_RULES_* env vars to prevent
    // developer/CI shell pollution from causing a false failure.
    let _ambient = AmbientClosureEnvCleanGuard::new();
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
    let _ambient = AmbientClosureEnvCleanGuard::new();
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

/// `[closure_rules] "capco:closure.dissem.noforn-if-caveated" = "off"` must round-trip as "off".
#[test]
fn closure_rules_severity_off() {
    let dir = make_tmpdir("closure-off");
    write_project_config(
        &dir,
        "[closure_rules]\n\"capco:closure.dissem.noforn-if-caveated\" = \"off\"\n",
    );

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _ambient = AmbientClosureEnvCleanGuard::new();
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco:closure.dissem.noforn-if-caveated")
            .map(String::as_str),
        Some("off"),
        "closure rule severity 'off' must round-trip"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// `[closure_rules] "capco:closure.dissem.noforn-if-caveated" = "suggest"` must round-trip as "suggest".
#[test]
fn closure_rules_severity_suggest() {
    let dir = make_tmpdir("closure-suggest");
    write_project_config(
        &dir,
        "[closure_rules]\n\"capco:closure.dissem.noforn-if-caveated\" = \"suggest\"\n",
    );

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _ambient = AmbientClosureEnvCleanGuard::new();
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco:closure.dissem.noforn-if-caveated")
            .map(String::as_str),
        Some("suggest"),
        "closure rule severity 'suggest' must round-trip"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// `[closure_rules] "capco:closure.dissem.noforn-if-caveated" = "info"` must round-trip as "info".
#[test]
fn closure_rules_severity_info() {
    let dir = make_tmpdir("closure-info");
    write_project_config(
        &dir,
        "[closure_rules]\n\"capco:closure.dissem.noforn-if-caveated\" = \"info\"\n",
    );

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _ambient = AmbientClosureEnvCleanGuard::new();
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco:closure.dissem.noforn-if-caveated")
            .map(String::as_str),
        Some("info"),
        "closure rule severity 'info' must round-trip"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// `[closure_rules] "capco:closure.dissem.noforn-if-caveated" = "warn"` must round-trip as "warn".
#[test]
fn closure_rules_severity_warn() {
    let dir = make_tmpdir("closure-warn");
    write_project_config(
        &dir,
        "[closure_rules]\n\"capco:closure.dissem.noforn-if-caveated\" = \"warn\"\n",
    );

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _ambient = AmbientClosureEnvCleanGuard::new();
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco:closure.dissem.noforn-if-caveated")
            .map(String::as_str),
        Some("warn"),
        "closure rule severity 'warn' must round-trip"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// `[closure_rules] "capco:closure.dissem.noforn-if-caveated" = "error"` must round-trip as "error".
#[test]
fn closure_rules_severity_error() {
    let dir = make_tmpdir("closure-error");
    write_project_config(
        &dir,
        "[closure_rules]\n\"capco:closure.dissem.noforn-if-caveated\" = \"error\"\n",
    );

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _ambient = AmbientClosureEnvCleanGuard::new();
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco:closure.dissem.noforn-if-caveated")
            .map(String::as_str),
        Some("error"),
        "closure rule severity 'error' must round-trip"
    );
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Category 3: Fix rejection at load
// ---------------------------------------------------------------------------

/// `[closure_rules] "capco:closure.dissem.noforn-if-caveated" = "fix"` must fail with
/// `ConfigError::InvalidClosureRuleSeverity` at config load.
///
/// Closure firings propagate facts, not byte-level edits — "fix" severity
/// is meaningless and rejected.
#[test]
fn closure_rules_fix_severity_rejected() {
    let dir = make_tmpdir("closure-fix-rejected");
    write_project_config(
        &dir,
        "[closure_rules]\n\"capco:closure.dissem.noforn-if-caveated\" = \"fix\"\n",
    );

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _ambient = AmbientClosureEnvCleanGuard::new();
    let err = marque_config::load(&dir).unwrap_err();

    match &err {
        ConfigError::InvalidClosureRuleSeverity { rule, hint } => {
            assert_eq!(rule, "capco:closure.dissem.noforn-if-caveated");
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
        msg.contains("capco:closure.dissem.noforn-if-caveated"),
        "error message must include the rule name; got: {msg:?}"
    );
    // The message must suggest a valid alternative.
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

/// Unknown severity string in `[closure_rules]` must fail with
/// `UnknownClosureRuleSeverity` (NOT `UnknownSeverity` and NOT
/// `InvalidClosureRuleSeverity`). The closure-rule variant's error
/// message omits "fix" from the expected-value list because closure
/// rules reject "fix" on the next code path — listing "fix" as
/// "expected" would mislead the user. Per Copilot PR 3.7 review #3.
#[test]
fn closure_rules_unknown_severity_rejected() {
    let dir = make_tmpdir("closure-unknown-severity");
    write_project_config(
        &dir,
        "[closure_rules]\n\"capco:closure.dissem.noforn-if-caveated\" = \"err\"\n",
    );

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _ambient = AmbientClosureEnvCleanGuard::new();
    let err = marque_config::load(&dir).unwrap_err();

    assert!(
        matches!(err, ConfigError::UnknownClosureRuleSeverity { .. }),
        "unknown severity string in [closure_rules] must produce \
         UnknownClosureRuleSeverity, got: {err:?}"
    );
    // The error message must NOT list `"fix"` in the expected-value list.
    // The pattern is "expected one of \"off\", ..., \"error\"" — `"fix"`
    // must not appear in that comma-separated list. The message MAY mention
    // "fix" elsewhere (e.g., a parenthetical pedagogical note saying
    // "closure rules do not accept fix"); only the expected-list is
    // forbidden from including it. Per Copilot PR 3.7 review #3.
    let err_text = format!("{err}");
    // Extract the "expected one of ..." substring and assert "fix" is
    // not inside it.
    let expected_list = err_text
        .split("expected one of ")
        .nth(1)
        .map(|tail| tail.split(" (").next().unwrap_or(tail))
        .expect("UnknownClosureRuleSeverity message should contain an expected-list");
    assert!(
        !expected_list.contains("\"fix\""),
        "UnknownClosureRuleSeverity expected-value list must not include \"fix\" \
         (closure rules reject \"fix\"); expected list was: {expected_list}"
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
/// This test uses the same string `"capco:closure.dissem.noforn-if-caveated"` as a key in both sections
/// (both are `HashMap<String, String>` at the file level) and asserts the
/// two overrides land in their respective config fields without interference.
#[test]
fn rules_and_closure_rules_are_section_isolated() {
    let dir = make_tmpdir("closure-isolation");
    let content = format!(
        "[capco]\nversion = \"{SCHEMA_VERSION}\"\n\n\
        [rules]\n\
        \"capco:closure.dissem.noforn-if-caveated\" = \"warn\"\n\n\
        [closure_rules]\n\
        \"capco:closure.dissem.noforn-if-caveated\" = \"error\"\n"
    );
    fs::write(dir.join(".marque.toml"), content).unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    // Clear ambient MARQUE_CLOSURE_RULES_* env vars — a stray override
    // on the same key would silently flip the [closure_rules] expectation
    // away from "error" and make this test fail nondeterministically.
    let _ambient = AmbientClosureEnvCleanGuard::new();
    let config = marque_config::load(&dir).expect("load should succeed");

    // [rules] → config.rules.overrides
    assert_eq!(
        config
            .rules
            .overrides
            .get("capco:closure.dissem.noforn-if-caveated")
            .map(String::as_str),
        Some("warn"),
        "[rules] capco:closure.dissem.noforn-if-caveated must be 'warn'"
    );

    // [closure_rules] → config.closure_rules.overrides
    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco:closure.dissem.noforn-if-caveated")
            .map(String::as_str),
        Some("error"),
        "[closure_rules] capco:closure.dissem.noforn-if-caveated must be 'error'"
    );

    // Cross-talk: [rules] must not see the closure value and vice versa.
    assert_ne!(
        config
            .rules
            .overrides
            .get("capco:closure.dissem.noforn-if-caveated")
            .map(String::as_str),
        config
            .closure_rules
            .overrides
            .get("capco:closure.dissem.noforn-if-caveated")
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
    let _ambient = AmbientClosureEnvCleanGuard::new();
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

/// Closure-rule names contain `:` and `.` (the wire-string
/// `<scheme>:closure.<category>.<predicate>` form), which TOML requires
/// to be quoted. Verify that the canonical wire-string keys parse
/// correctly.
#[test]
fn closure_rules_quoted_key_with_wire_string_parses() {
    let dir = make_tmpdir("closure-quoted-key");
    // Both keys use the wire-string form that TOML requires quoting for
    // (colon and dot are special outside string keys).
    let content = format!(
        "[capco]\nversion = \"{SCHEMA_VERSION}\"\n\n\
        [closure_rules]\n\
        \"capco:closure.dissem.noforn-if-caveated\" = \"warn\"\n\
        \"capco:closure.dissem.relido-if-sci-and-not-incompatible\" = \"off\"\n"
    );
    fs::write(dir.join(".marque.toml"), content).unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _ambient = AmbientClosureEnvCleanGuard::new();
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco:closure.dissem.noforn-if-caveated")
            .map(String::as_str),
        Some("warn"),
        "capco:closure.dissem.noforn-if-caveated should be 'warn'"
    );
    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco:closure.dissem.relido-if-sci-and-not-incompatible")
            .map(String::as_str),
        Some("off"),
        "capco:closure.dissem.relido-if-sci-and-not-incompatible should be 'off'"
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
        \"capco:closure.dissem.noforn-if-caveated\" = \"warn\"\n\
        \"capco:closure.dissem.relido-if-sci-and-not-incompatible\" = \"off\"\n\
        \"capco:closure.dissem.si-g-implies-orcon\" = \"error\"\n"
    );
    fs::write(dir.join(".marque.toml"), content).unwrap();

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    // Clear ambient MARQUE_CLOSURE_RULES_* env vars to ensure the
    // count assertion measures the file-derived overrides only.
    let _ambient = AmbientClosureEnvCleanGuard::new();
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(config.closure_rules.overrides.len(), 3);
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Category 6: Env-var override (MARQUE_CLOSURE_RULES_*)
// ---------------------------------------------------------------------------

/// `MARQUE_CLOSURE_RULES_CAPCO__CLOSURE__DISSEM__NOFORN_IF_CAVEATED=warn`
/// is decoded by the env-var encoder to the wire-string
/// form `"capco:closure.dissem.noforn-if-caveated"` (per
/// `config/src/lib.rs::env_var_to_closure_rule_name`). Encoder
/// convention: `__` between segments (first occurrence becomes `:`,
/// subsequent become `.`); `_` within a segment becomes `-`. This
/// matches the `.marque.toml [closure_rules]` key shape.
///
/// The env-var override path is keyed by whatever the encoder
/// produces; the test pins that file-level and env-var key shapes
/// CONVERGE — both write `capco:closure.dissem.noforn-if-caveated`
/// into `closure_rules.overrides` so the env var actually overrides
/// the file value.
#[test]
fn env_var_overrides_closure_rule_file_value() {
    let dir = make_tmpdir("closure-env-override");
    write_project_config(
        &dir,
        "[closure_rules]\n\"capco:closure.dissem.noforn-if-caveated\" = \"info\"\n",
    );

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _ambient = AmbientClosureEnvCleanGuard::new();
    let _env = EnvGuard::set(
        "MARQUE_CLOSURE_RULES_CAPCO__CLOSURE__DISSEM__NOFORN_IF_CAVEATED",
        "warn",
    );
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco:closure.dissem.noforn-if-caveated")
            .map(String::as_str),
        Some("warn"),
        "MARQUE_CLOSURE_RULES_* env var must override file-level value \
         (wire-string convergence)"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// `MARQUE_CLOSURE_RULES_CAPCO__CLOSURE__NATO__REL_TO_USA_NATO_IF_NATO_CLASSIFICATION=error`
/// without a file-level entry must add the override to
/// `config.closure_rules.overrides` (the env var is the sole source
/// of the row's severity). Encoder produces the wire-string
/// form — see the note on the preceding test for the encoding convention.
#[test]
fn env_var_adds_closure_rule_when_absent_in_file() {
    let dir = make_tmpdir("closure-env-add");
    write_project_config(&dir, "");

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _ambient = AmbientClosureEnvCleanGuard::new();
    let _env = EnvGuard::set(
        "MARQUE_CLOSURE_RULES_CAPCO__CLOSURE__NATO__REL_TO_USA_NATO_IF_NATO_CLASSIFICATION",
        "error",
    );
    let config = marque_config::load(&dir).expect("load should succeed");

    assert_eq!(
        config
            .closure_rules
            .overrides
            .get("capco:closure.nato.rel-to-usa-nato-if-nato-classification")
            .map(String::as_str),
        Some("error"),
        "MARQUE_CLOSURE_RULES_* env var must add entry absent in file \
         (wire-string convergence)"
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
    let _ambient = AmbientClosureEnvCleanGuard::new();
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
/// `ConfigError::UnknownClosureRuleSeverity` (closure-rule-specific variant,
/// per Copilot PR 3.7 review #3).
#[test]
fn env_var_unknown_severity_rejected() {
    let dir = make_tmpdir("closure-env-unknown");
    write_project_config(&dir, "");

    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _ambient = AmbientClosureEnvCleanGuard::new();
    let _env = EnvGuard::set("MARQUE_CLOSURE_RULES_CAPCO__FOO", "err");
    let err = marque_config::load(&dir).unwrap_err();

    assert!(
        matches!(err, ConfigError::UnknownClosureRuleSeverity { .. }),
        "MARQUE_CLOSURE_RULES_* with unknown severity must produce UnknownClosureRuleSeverity, got: {err:?}"
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
    // Clear ambient MARQUE_CLOSURE_RULES_* env vars first so the
    // assertion below tests the actual property (unrelated env vars
    // don't bleed in) rather than incidentally passing/failing on
    // developer-shell pollution.
    let _ambient = AmbientClosureEnvCleanGuard::new();
    // MARQUE_RULES_* does not exist yet; MARQUE_CLASSIFIER_ID should not bleed in.
    let _env1 = EnvGuard::set("MARQUE_CLASSIFIER_ID", "test-classifier");
    let config = marque_config::load(&dir).expect("load should succeed");

    assert!(
        config.closure_rules.overrides.is_empty(),
        "unrelated env vars must not populate closure_rules.overrides"
    );
    let _ = fs::remove_dir_all(&dir);
}
