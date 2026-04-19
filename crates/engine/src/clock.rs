// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Clock abstraction for deterministic timestamps in audit records.
//!
//! `Engine` uses `dyn Clock` exclusively for `AppliedFix::timestamp`.
//! Production code injects `SystemClock`; tests inject `FixedClock`
//! so snapshot tests of audit NDJSON are deterministic.

use std::time::SystemTime;

/// Abstraction over `SystemTime::now()` for testability.
pub trait Clock: Send + Sync {
    fn now(&self) -> SystemTime;
}

/// Production clock — delegates to `SystemTime::now()`.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// Test clock — always returns the same instant.
pub struct FixedClock(SystemTime);

impl FixedClock {
    /// Construct a fixed clock that always returns `instant`.
    pub const fn new(instant: SystemTime) -> Self {
        Self(instant)
    }
}

impl Clock for FixedClock {
    fn now(&self) -> SystemTime {
        self.0
    }
}
