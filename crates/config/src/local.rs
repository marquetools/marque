// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::Config;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Default)]
pub(crate) struct UserConfigFile {
    pub(crate) classifier_id: Option<String>,
    pub(crate) classification_authority: Option<String>,
    pub(crate) default_reason: Option<String>,
    pub(crate) derived_from_default: Option<String>,
}

pub(crate) fn merge_user_into(config: &mut Config, user: Option<UserConfigFile>) {
    fn non_empty(s: Option<String>) -> Option<String> {
        s.filter(|v| !v.trim().is_empty())
    }

    if let Some(user) = user {
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
