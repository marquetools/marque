// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::{Config, ConfigError};
use marque_ism::UtcOffset;
use marque_rules::Severity;

pub(crate) fn apply_env(config: &mut Config) -> Result<(), ConfigError> {
    if let Ok(id) = std::env::var("MARQUE_CLASSIFIER_ID") {
        if !id.trim().is_empty() {
            config.user.classifier_id = Some(id);
        }
    }
    if let Ok(raw) = std::env::var("MARQUE_CONFIDENCE_THRESHOLD") {
        let threshold = raw.parse::<f32>().map_err(|_| ConfigError::InvalidEnvVar {
            var: "MARQUE_CONFIDENCE_THRESHOLD",
            raw: raw.clone(),
            reason: "expected a floating-point number in [0.0, 1.0]",
        })?;
        config.set_confidence_threshold(threshold)?;
    }
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

/// Convert a `MARQUE_CLOSURE_RULES_*` env var key into a closure-rule
/// wire-string key (`<scheme>:<predicate.path>`).
///
/// Encoding:
/// 1. Strip `MARQUE_CLOSURE_RULES_`
/// 2. Split on `__` (segment boundaries)
/// 3. Convert `_` within each segment to `-`
/// 4. Lowercase
/// 5. Join as `<first>:<rest.join(".")>`
///
/// Returns `None` for non-matching prefixes or suffixes without at least
/// two segments (scheme + one predicate segment).
pub(crate) fn env_var_to_closure_rule_name(env_key: &str) -> Option<String> {
    const PREFIX: &str = "MARQUE_CLOSURE_RULES_";
    let suffix = env_key.strip_prefix(PREFIX)?;
    let segments: Vec<String> = suffix
        .split("__")
        .map(|seg| seg.replace('_', "-").to_lowercase())
        .collect();
    if segments.len() < 2 {
        return None;
    }
    let scheme = &segments[0];
    let predicate = segments[1..].join(".");
    Some(format!("{scheme}:{predicate}"))
}
