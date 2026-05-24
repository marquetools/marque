// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

pub(super) use super::*;
use std::str::FromStr;

pub(super) fn round_trip(s: &str) -> bool {
    IsmDate::from_str(s)
        .map(|d| d.to_string() == s)
        .unwrap_or(false)
}

mod comparisons;
mod core;
mod edge_cases;
