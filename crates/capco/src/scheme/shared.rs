// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Cross-module helper home for `CapcoScheme` Stage-1 lift (issue #466).
//!
//! Currently holds only `impl CompanionForm` (15 LOC); per the split
//! plan §Risk 2 the larger cross-module helpers (`class_floor_emit`,
//! `emit_*_companions`, `sci_per_system_emit`, `SCI_PER_SYSTEM_CATALOG`)
//! were proposed to live here as `pub(crate)`, but Stage-1 places those
//! at their natural homes (constraints / actions / mod) with `pub(crate)`
//! visibility so siblings can call them — `shared.rs` exists for the
//! `CompanionForm` impl alone in this PR.

use super::CompanionForm;

impl CompanionForm {
    pub(crate) fn orcon(self) -> &'static str {
        match self {
            Self::Abbreviated => "OC",
            Self::Full => "ORCON",
        }
    }

    pub(crate) fn noforn(self) -> &'static str {
        match self {
            Self::Abbreviated => "NF",
            Self::Full => "NOFORN",
        }
    }
}
