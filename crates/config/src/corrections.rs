// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::Config;
use std::collections::HashMap;

pub(crate) fn merge_into(config: &mut Config, corrections: HashMap<String, String>) {
    config.corrections.extend(corrections);
}
