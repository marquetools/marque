// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::{Config, ConfigError};
use marque_ism::UtcOffset;
use marque_rules::Severity;

pub(crate) fn apply_env(config: &mut Config) -> Result<(), ConfigError> {
    // L-2 parity: apply the same non-empty guard as merge_user_into so that
    // `MARQUE_CLASSIFIER_ID=""` does not silently overwrite a populated
    // local-config value with an empty string. For a security tool where
    // classifier identity ends up in the audit record, that is a meaningful
    // correctness hole.
    if let Ok(id) = std::env::var("MARQUE_CLASSIFIER_ID") {
        if !id.trim().is_empty() {
            config.user.classifier_id = Some(id);
        }
    }
    // Propagate parse failures. `MARQUE_CONFIDENCE_THRESHOLD=0.9o` must
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
    // Parse MARQUE_DEFAULT_TIMEZONE as an ISO 8601 UTC offset.
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
    // MARQUE_CLOSURE_RULES_* env-var namespace.
    //
    // Naming convention (wire-string form): the env-var
    // suffix encodes the closure-rule key segments separated by `__`
    // (double-underscore); single `_` within a segment becomes `-`.
    // The first segment is the scheme; subsequent segments are the
    // dot-separated predicate parts. Result joins as
    // `<scheme>:<seg1>.<seg2>…`. See `env_var_to_closure_rule_name`
    // for the encoder. Examples:
    //   MARQUE_CLOSURE_RULES_CAPCO__CLOSURE__DISSEM__NOFORN_IF_CAVEATED=warn
    //     → "capco:closure.dissem.noforn-if-caveated"
    //   MARQUE_CLOSURE_RULES_CAPCO__CLOSURE__NATO__REL_TO_USA_NATO_IF_NATO_CLASSIFICATION=off
    //     → "capco:closure.nato.rel-to-usa-nato-if-nato-classification"
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
