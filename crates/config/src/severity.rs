// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::ConfigError;
use marque_rules::Severity;
use std::collections::HashMap;

pub(crate) fn validate_rule_overrides(
    overrides: &HashMap<String, String>,
) -> Result<(), ConfigError> {
    for (rule, value) in overrides {
        if Severity::parse_config(value).is_none() {
            return Err(ConfigError::UnknownSeverity {
                rule: rule.clone(),
                value: value.clone(),
            });
        }
    }
    Ok(())
}

pub(crate) fn validate_closure_rule_overrides(
    overrides: &HashMap<String, String>,
) -> Result<(), ConfigError> {
    for (rule, value) in overrides {
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
            Some(_) => {}
        }
    }
    Ok(())
}
