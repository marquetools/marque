// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![forbid(unsafe_code)]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! marque-config — layered configuration loading.
//!
//! Precedence (highest wins): CLI flags → env vars → `.marque.local.toml` → `.marque.toml`
//!
//! # Hard-fail validators (T023)
//!
//! The loader refuses to produce a `Config` if any of these conditions hold:
//! - `.marque.toml` contains a `[user]` section (FR-010, SC-006) → exit 65
//! - `[capco] version` mismatches `marque_ism::SCHEMA_VERSION` (FR-011) → exit 65
//! - `confidence_threshold` outside `[0.0, 1.0]` → exit 65

use marque_ism::UtcOffset;
#[cfg(feature = "toml-loader")]
use marque_rules::Severity;
#[cfg(feature = "toml-loader")]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

#[cfg(feature = "corpus-override")]
pub mod corpus_override;

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

    /// `.marque.toml` contains a `[user]` section (FR-010, SC-006).
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
    /// Keyed by closure rule name in the post-T044 wire-string form
    /// (e.g., `"capco:closure.dissem.noforn-if-caveated"`).
    /// `Severity::Fix` is rejected at config load — closure firings propagate
    /// facts, not byte-level edits. See `decisions.md` D19 B.
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
/// Per `decisions.md` D19 B + plan §1.5: section-isolated from `[rules]`.
/// Keyed by `ClosureRule.name` in the post-T044 wire-string form
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

// ---------------------------------------------------------------------------
// TOML-deserialisable file format
//
// `ConfigFile` + nested file structs only deserialize from TOML today, so
// they are gated alongside the `toml-loader` feature. If a future codepath
// needs to deserialize the same shape from JSON, the gate can be widened
// without re-deriving the structure.
// ---------------------------------------------------------------------------

#[cfg(feature = "toml-loader")]
#[derive(Debug, Deserialize, Serialize, Default)]
struct ConfigFile {
    #[serde(default)]
    user: Option<UserConfigFile>,
    #[serde(default)]
    rules: HashMap<String, String>,
    /// Closure-rule severity overrides. Keys use the T044 wire-string
    /// form (`<scheme>:closure.<category>.<predicate>`) and must be
    /// quoted in TOML because `:` and `.` are not valid in bare TOML
    /// keys. Example:
    ///
    /// ```toml
    /// [closure_rules]
    /// "capco:closure.dissem.noforn-if-caveated" = "warn"
    /// "capco:closure.dissem.relido-if-sci-and-not-incompatible" = "off"
    /// ```
    #[serde(default)]
    closure_rules: HashMap<String, String>,
    #[serde(default)]
    corrections: HashMap<String, String>,
    #[serde(default)]
    capco: CapcoConfigFile,
    #[serde(default)]
    confidence_threshold: Option<f32>,
}

#[cfg(feature = "toml-loader")]
#[derive(Debug, Deserialize, Serialize, Default)]
struct UserConfigFile {
    classifier_id: Option<String>,
    classification_authority: Option<String>,
    default_reason: Option<String>,
    derived_from_default: Option<String>,
}

#[cfg(feature = "toml-loader")]
#[derive(Debug, Deserialize, Serialize, Default)]
struct CapcoConfigFile {
    version: Option<String>,
    /// UTC offset string for floating times. Accepted: `"Z"`, `"+HH:MM"`, `"-HH:MM"`.
    default_timezone: Option<String>,
}

// ---------------------------------------------------------------------------
// Config loading
// ---------------------------------------------------------------------------

/// Load and merge configuration from standard locations.
///
/// Search order (first found wins for each layer):
/// 1. `.marque.toml` discovered by walking upward from `start` per
///    `contracts/cli.md`. The walk stops at the **first** of:
///    - a directory containing `.marque.toml`
///    - a directory containing `.git/` (git repository root)
///    - the filesystem root
///
///    If the walk finds a `.marque.toml`, that directory is the project root
///    for both Layer 1 (committed) and Layer 2 (local). If the walk finds a
///    git root or filesystem root first, no project config is loaded —
///    Layer 3 (env vars) still runs.
/// 2. `.marque.local.toml` **only in the same directory** as the discovered
///    `.marque.toml`. The local-config search is never independently walked,
///    so a stray `.marque.local.toml` in a parent directory cannot silently
///    attach to a child project's config.
/// 3. Environment variables (`MARQUE_CLASSIFIER_ID`, `MARQUE_CONFIDENCE_THRESHOLD`,
///    `MARQUE_LOG`, `MARQUE_DEFAULT_TIMEZONE`, and the
///    `MARQUE_CLOSURE_RULES_*` parallel namespace for `[closure_rules]`
///    per-row severity overrides — see [`Config::closure_rules`] and the
///    naming convention documented at [`env_var_to_closure_rule_name`]).
///
/// Hard-fail validators run after merging all layers.
#[cfg(feature = "toml-loader")]
pub fn load(start: &std::path::Path) -> Result<Config, ConfigError> {
    let mut config = Config::default();

    // Layer 1+2: walk upward for the project config.
    if let Some(project_dir) = discover_project_dir(start) {
        // Layer 1: project config
        let project_config = project_dir.join(".marque.toml");
        let raw = std::fs::read_to_string(&project_config).map_err(|e| ConfigError::ReadError {
            path: project_config.clone(),
            source: e,
        })?;
        let file: ConfigFile = toml::from_str(&raw)?;

        // T023: refuse [user] section in committed config (FR-010, SC-006)
        if file.user.is_some() {
            return Err(ConfigError::UserSectionInCommitted {
                path: project_config,
            });
        }

        merge_project_into(&mut config, file)?;

        // Layer 2: user-local config in the SAME directory only.
        let local_config = project_dir.join(".marque.local.toml");
        if local_config.exists() {
            let raw =
                std::fs::read_to_string(&local_config).map_err(|e| ConfigError::ReadError {
                    path: local_config.clone(),
                    source: e,
                })?;
            let file: ConfigFile = toml::from_str(&raw)?;
            merge_user_into(&mut config, file);
        }
    }

    // Layer 3: environment variables
    apply_env(&mut config)?;

    // T023: validate schema version (FR-011)
    validate_schema_version(&config)?;

    Ok(config)
}

/// Load configuration from an explicit `.marque.toml` path, bypassing the
/// upward walk. Used by `--config <PATH>` per `contracts/cli.md`:
/// "short-circuits the walk and uses the specified path as the project
/// config; the local-config search still applies, only in the directory
/// containing the supplied path."
#[cfg(feature = "toml-loader")]
pub fn load_with_explicit_config(project_config: &std::path::Path) -> Result<Config, ConfigError> {
    let mut config = Config::default();

    // Layer 1: explicit project config — required to exist.
    let raw = std::fs::read_to_string(project_config).map_err(|e| ConfigError::ReadError {
        path: project_config.to_path_buf(),
        source: e,
    })?;
    let file: ConfigFile = toml::from_str(&raw)?;

    if file.user.is_some() {
        return Err(ConfigError::UserSectionInCommitted {
            path: project_config.to_path_buf(),
        });
    }

    merge_project_into(&mut config, file)?;

    // Layer 2: local config in the same directory as the explicit path.
    if let Some(parent) = project_config.parent() {
        let local_config = parent.join(".marque.local.toml");
        if local_config.exists() {
            let raw =
                std::fs::read_to_string(&local_config).map_err(|e| ConfigError::ReadError {
                    path: local_config.clone(),
                    source: e,
                })?;
            let file: ConfigFile = toml::from_str(&raw)?;
            merge_user_into(&mut config, file);
        }
    }

    apply_env(&mut config)?;
    validate_schema_version(&config)?;
    Ok(config)
}

/// Walk upward from `start` looking for a directory containing `.marque.toml`.
///
/// Returns `Some(dir)` if a `.marque.toml` is found before hitting either a
/// git repository root (a directory containing `.git/`) or the filesystem
/// root. Returns `None` otherwise — falling back to built-in defaults is the
/// caller's responsibility.
///
/// The walk treats `.git` as a hard stop *only when* the directory does not
/// also contain `.marque.toml`. A repo with `.marque.toml` at its root is
/// the common case and must succeed.
#[cfg(feature = "toml-loader")]
fn discover_project_dir(start: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".marque.toml").is_file() {
            return Some(current);
        }
        // Hit a git repo root that did not contain .marque.toml — stop.
        // The check is for `.git` as either a file (git worktree pointer)
        // or a directory (normal repo).
        if current.join(".git").exists() {
            return None;
        }
        if !current.pop() {
            // Filesystem root — nothing more to walk.
            return None;
        }
    }
}

#[cfg(feature = "toml-loader")]
fn merge_project_into(config: &mut Config, file: ConfigFile) -> Result<(), ConfigError> {
    // H-6: validate every severity override at load time. A typo like
    // `E001 = "err"` must fail loudly, not silently fall back to the rule
    // default.
    for (rule, value) in &file.rules {
        if Severity::parse_config(value).is_none() {
            return Err(ConfigError::UnknownSeverity {
                rule: rule.clone(),
                value: value.clone(),
            });
        }
    }
    config.rules.overrides.extend(file.rules);

    // D19 B: validate closure-rule severity overrides. "fix" is rejected because
    // closure firings propagate facts, not byte-level edits. Unknown values
    // route to `UnknownClosureRuleSeverity` (whose error message excludes
    // "fix" from the expected list) rather than the generic `UnknownSeverity`,
    // per Copilot PR 3.7 review #3.
    for (rule, value) in &file.closure_rules {
        match Severity::parse_config(value) {
            None => {
                return Err(ConfigError::UnknownClosureRuleSeverity {
                    rule: rule.clone(),
                    value: value.clone(),
                });
            }
            Some(Severity::Fix) => {
                return Err(ConfigError::InvalidClosureRuleSeverity {
                    rule: rule.clone(),
                    hint: "closure rows propagate facts, not byte-level fixes",
                });
            }
            Some(_) => { /* valid */ }
        }
    }
    config.closure_rules.overrides.extend(file.closure_rules);

    config.corrections.extend(file.corrections);
    if let Some(v) = file.capco.version {
        config.capco.version = v;
    }
    if let Some(ref tz) = file.capco.default_timezone {
        config.capco.default_timezone = tz
            .parse::<UtcOffset>()
            .map_err(|_| ConfigError::InvalidTimezone { value: tz.clone() })?;
    }
    if let Some(threshold) = file.confidence_threshold {
        config.set_confidence_threshold(threshold)?;
    }
    Ok(())
}

#[cfg(feature = "toml-loader")]
fn merge_user_into(config: &mut Config, file: ConfigFile) {
    // L-2: an empty string is semantically equivalent to "not set". Without
    // this guard, a .marque.local.toml entry of `classifier_id = ""` would
    // silently overwrite a populated value from another layer with an empty
    // string. For a security tool where classifier identity ends up in the
    // audit record, that is a meaningful correctness hole.
    fn non_empty(s: Option<String>) -> Option<String> {
        s.filter(|v| !v.trim().is_empty())
    }

    if let Some(user) = file.user {
        if let Some(v) = non_empty(user.classifier_id) {
            config.user.classifier_id = Some(v);
        }
        if let Some(v) = non_empty(user.classification_authority) {
            config.user.classification_authority = Some(v);
        }
        if let Some(v) = non_empty(user.default_reason) {
            config.user.default_reason = Some(v);
        }
        if let Some(v) = non_empty(user.derived_from_default) {
            config.user.derived_from_default = Some(v);
        }
    }
}

#[cfg(feature = "toml-loader")]
fn apply_env(config: &mut Config) -> Result<(), ConfigError> {
    // L-2 parity: apply the same non-empty guard as merge_user_into so that
    // `MARQUE_CLASSIFIER_ID=""` does not silently overwrite a populated
    // local-config value with an empty string.
    if let Ok(id) = std::env::var("MARQUE_CLASSIFIER_ID") {
        if !id.trim().is_empty() {
            config.user.classifier_id = Some(id);
        }
    }
    // C-2: propagate parse failures. `MARQUE_CONFIDENCE_THRESHOLD=0.9o` must
    // hard-fail, not silently apply the default.
    if let Ok(raw) = std::env::var("MARQUE_CONFIDENCE_THRESHOLD") {
        let threshold = raw.parse::<f32>().map_err(|_| ConfigError::InvalidEnvVar {
            var: "MARQUE_CONFIDENCE_THRESHOLD",
            raw: raw.clone(),
            reason: "expected a floating-point number in [0.0, 1.0]",
        })?;
        config.set_confidence_threshold(threshold)?;
    }
    // MARQUE_LOG is handled by the tracing subscriber, not by config loading.
    // C-3: parse MARQUE_DEFAULT_TIMEZONE as an ISO 8601 UTC offset.
    if let Ok(raw) = std::env::var("MARQUE_DEFAULT_TIMEZONE") {
        if !raw.trim().is_empty() {
            config.capco.default_timezone =
                raw.parse::<UtcOffset>()
                    .map_err(|_| ConfigError::InvalidEnvVar {
                        var: "MARQUE_DEFAULT_TIMEZONE",
                        raw: raw.clone(),
                        reason: "expected \"Z\", \"+HH:MM\", or \"-HH:MM\"",
                    })?;
        }
    }
    // D19 B / plan §1.5b: MARQUE_CLOSURE_RULES_* env-var namespace.
    //
    // Naming convention: MARQUE_CLOSURE_RULES_<NAME> where <NAME> is the
    // closure rule name with '/' replaced by '__' and the whole suffix
    // lowercased. Examples:
    //   MARQUE_CLOSURE_RULES_CAPCO__NOFORN_IF_NO_FDR=warn
    //     → "capco/noforn-if-no-fdr"
    //   MARQUE_CLOSURE_RULES_CAPCO__RELIDO_IF_NO_FDR=off
    //     → "capco/relido-if-no-fdr"
    //
    // Note: MARQUE_RULES_* per-rule env-var overrides do not exist for the
    // [rules] section as of this implementation; MARQUE_CLOSURE_RULES_* is
    // the first per-row env-var surface in the config system.
    for (key, value) in std::env::vars() {
        if let Some(rule_name) = env_var_to_closure_rule_name(&key) {
            match Severity::parse_config(&value) {
                None => {
                    return Err(ConfigError::UnknownClosureRuleSeverity {
                        rule: rule_name,
                        value,
                    });
                }
                Some(Severity::Fix) => {
                    return Err(ConfigError::InvalidClosureRuleSeverity {
                        rule: rule_name,
                        hint: "closure rows propagate facts, not byte-level fixes",
                    });
                }
                Some(_) => {
                    config.closure_rules.overrides.insert(rule_name, value);
                }
            }
        }
    }
    Ok(())
}

/// Convert a `MARQUE_CLOSURE_RULES_*` env-var key to a closure rule name.
///
/// Encoding convention (env var → rule name):
/// 1. Strip the `MARQUE_CLOSURE_RULES_` prefix.
/// 2. Replace `__` (double-underscore) with `/` (domain separator).
/// 3. Replace remaining `_` (single-underscore) with `-` (word separator).
/// 4. Lowercase the whole result.
///
/// Examples:
/// - `MARQUE_CLOSURE_RULES_CAPCO__NOFORN_IF_NO_FDR` → `"capco/noforn-if-no-fdr"`
/// - `MARQUE_CLOSURE_RULES_CAPCO__RELIDO_IF_NO_FDR` → `"capco/relido-if-no-fdr"`
///
/// Returns `None` if the key does not have the expected prefix.
///
/// **T044 status:** the encoder currently emits the pre-T044 legacy
/// slash form. The `.marque.toml [closure_rules]` keys use the
/// post-T044 wire-string form (`<scheme>:closure.<category>.<predicate>`).
/// The two surfaces will land on the same shape when the env-var
/// encoder is migrated; until then, env-var-derived overrides and
/// file-derived overrides land in `closure_rules.overrides` under
/// different keys. The engine does not yet consume
/// `closure_rules.overrides` so this divergence is forward-looking
/// only; the migration is its own follow-up.
#[cfg(feature = "toml-loader")]
fn env_var_to_closure_rule_name(env_key: &str) -> Option<String> {
    const PREFIX: &str = "MARQUE_CLOSURE_RULES_";
    let suffix = env_key.strip_prefix(PREFIX)?;
    // Step 1: replace `__` with `/` first so single-underscore pass doesn't
    // corrupt the double-underscore separator.
    // Step 2: replace remaining `_` with `-`.
    // Step 3: lowercase.
    Some(suffix.replace("__", "/").replace('_', "-").to_lowercase())
}

/// T023: validate schema version matches compiled marque-ism (FR-011).
///
/// Exact match required — the config must use the canonical form (e.g., "ISM-v2022-DEC").
#[cfg(feature = "toml-loader")]
fn validate_schema_version(config: &Config) -> Result<(), ConfigError> {
    let compiled = marque_ism::generated::values::SCHEMA_VERSION;
    let config_ver = &config.capco.version;

    if config_ver != compiled {
        return Err(ConfigError::SchemaVersionMismatch {
            config_version: config_ver.clone(),
            compiled_version: compiled,
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
