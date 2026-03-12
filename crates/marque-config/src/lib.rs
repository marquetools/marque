//! marque-config — layered configuration loading.
//!
//! Precedence (highest wins): CLI flags → env vars → `.marque.local.toml` → `.marque.toml`

use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file {path}: {source}")]
    ReadError { path: PathBuf, source: std::io::Error },

    #[error("failed to parse config: {0}")]
    ParseError(#[from] toml::de::Error),
}

/// Resolved, merged configuration ready for engine use.
#[derive(Debug, Clone, Default)]
pub struct Config {
    pub user: UserConfig,
    pub rules: RuleConfig,
    pub corrections: HashMap<String, String>,
    pub capco: CapcoConfig,
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

/// CAPCO-specific configuration.
#[derive(Debug, Clone)]
pub struct CapcoConfig {
    /// Pinned ISM schema version. Must match the compiled marque-capco version.
    pub version: String,
}

impl Default for CapcoConfig {
    fn default() -> Self {
        Self { version: "2022-DEC".to_owned() }
    }
}

// ---------------------------------------------------------------------------
// TOML-deserialisable file format
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Default)]
struct ConfigFile {
    #[serde(default)]
    user: UserConfigFile,
    #[serde(default)]
    rules: HashMap<String, String>,
    #[serde(default)]
    corrections: HashMap<String, String>,
    #[serde(default)]
    capco: CapcoConfigFile,
}

#[derive(Debug, Deserialize, Serialize, Default)]
struct UserConfigFile {
    classifier_id: Option<String>,
    classification_authority: Option<String>,
    default_reason: Option<String>,
    derived_from_default: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
struct CapcoConfigFile {
    version: Option<String>,
}

// ---------------------------------------------------------------------------
// Config loading
// ---------------------------------------------------------------------------

/// Load and merge configuration from standard locations.
///
/// Search order (first found wins for each layer):
/// 1. `.marque.toml` in current directory or any parent
/// 2. `.marque.local.toml` alongside the project config (user-specific, gitignored)
pub fn load(project_root: &std::path::Path) -> Result<Config, ConfigError> {
    let mut config = Config::default();

    // Layer 1: project config
    let project_config = project_root.join(".marque.toml");
    if project_config.exists() {
        let raw = std::fs::read_to_string(&project_config)
            .map_err(|e| ConfigError::ReadError { path: project_config.clone(), source: e })?;
        let file: ConfigFile = toml::from_str(&raw)?;
        merge_file_into(&mut config, file);
    }

    // Layer 2: user-local config (gitignored)
    let local_config = project_root.join(".marque.local.toml");
    if local_config.exists() {
        let raw = std::fs::read_to_string(&local_config)
            .map_err(|e| ConfigError::ReadError { path: local_config.clone(), source: e })?;
        let file: ConfigFile = toml::from_str(&raw)?;
        merge_user_into(&mut config, file);
    }

    // Layer 3: environment variables
    apply_env(&mut config);

    Ok(config)
}

fn merge_file_into(config: &mut Config, file: ConfigFile) {
    config.rules.overrides.extend(file.rules);
    config.corrections.extend(file.corrections);
    if let Some(v) = file.capco.version {
        config.capco.version = v;
    }
}

fn merge_user_into(config: &mut Config, file: ConfigFile) {
    config.user.classifier_id = file.user.classifier_id.or(config.user.classifier_id.take());
    config.user.classification_authority =
        file.user.classification_authority.or(config.user.classification_authority.take());
    config.user.default_reason = file.user.default_reason.or(config.user.default_reason.take());
    config.user.derived_from_default =
        file.user.derived_from_default.or(config.user.derived_from_default.take());
}

fn apply_env(config: &mut Config) {
    if let Ok(id) = std::env::var("MARQUE_CLASSIFIER_ID") {
        config.user.classifier_id = Some(id);
    }
}
