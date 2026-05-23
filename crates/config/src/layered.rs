// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::corrections;
use crate::env;
use crate::local::{self, UserConfigFile};
use crate::severity;
use crate::{Config, ConfigError};
use marque_ism::UtcOffset;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Serialize, Default)]
pub(crate) struct ConfigFile {
    #[serde(default)]
    user: Option<UserConfigFile>,
    #[serde(default)]
    pub(crate) rules: HashMap<String, String>,
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
    pub(crate) capco: CapcoConfigFile,
    #[serde(default)]
    confidence_threshold: Option<f32>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub(crate) struct CapcoConfigFile {
    pub(crate) version: Option<String>,
    /// UTC offset string for floating times. Accepted: `"Z"`, `"+HH:MM"`, `"-HH:MM"`.
    pub(crate) default_timezone: Option<String>,
}

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
///    naming convention documented at [`crate::env::env_var_to_closure_rule_name`]).
///
/// Hard-fail validators run after merging all layers.
pub fn load(start: &Path) -> Result<Config, ConfigError> {
    let mut config = Config::default();

    if let Some(project_dir) = discover_project_dir(start) {
        let project_config = project_dir.join(".marque.toml");
        let raw = std::fs::read_to_string(&project_config).map_err(|e| ConfigError::ReadError {
            path: project_config.clone(),
            source: e,
        })?;
        let file: ConfigFile = toml::from_str(&raw)?;

        if file.user.is_some() {
            return Err(ConfigError::UserSectionInCommitted {
                path: project_config,
            });
        }

        merge_project_into(&mut config, file)?;

        let local_config = project_dir.join(".marque.local.toml");
        if local_config.exists() {
            let raw =
                std::fs::read_to_string(&local_config).map_err(|e| ConfigError::ReadError {
                    path: local_config.clone(),
                    source: e,
                })?;
            let file: ConfigFile = toml::from_str(&raw)?;
            local::merge_user_into(&mut config, file.user);
        }
    }

    env::apply_env(&mut config)?;
    validate_schema_version(&config)?;
    Ok(config)
}

/// Load configuration from an explicit `.marque.toml` path, bypassing the
/// upward walk. Used by `--config <PATH>` per `contracts/cli.md`:
/// "short-circuits the walk and uses the specified path as the project
/// config; the local-config search still applies, only in the directory
/// containing the supplied path."
pub fn load_with_explicit_config(project_config: &Path) -> Result<Config, ConfigError> {
    let mut config = Config::default();

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

    if let Some(parent) = project_config.parent() {
        let local_config = parent.join(".marque.local.toml");
        if local_config.exists() {
            let raw =
                std::fs::read_to_string(&local_config).map_err(|e| ConfigError::ReadError {
                    path: local_config.clone(),
                    source: e,
                })?;
            let file: ConfigFile = toml::from_str(&raw)?;
            local::merge_user_into(&mut config, file.user);
        }
    }

    env::apply_env(&mut config)?;
    validate_schema_version(&config)?;
    Ok(config)
}

pub(crate) fn discover_project_dir(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".marque.toml").is_file() {
            return Some(current);
        }
        if current.join(".git").exists() {
            return None;
        }
        if !current.pop() {
            return None;
        }
    }
}

pub(crate) fn merge_project_into(config: &mut Config, file: ConfigFile) -> Result<(), ConfigError> {
    severity::validate_rule_overrides(&file.rules)?;
    config.rules.overrides.extend(file.rules);

    severity::validate_closure_rule_overrides(&file.closure_rules)?;
    config.closure_rules.overrides.extend(file.closure_rules);

    corrections::merge_into(config, file.corrections);
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
