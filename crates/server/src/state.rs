// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use crate::DEFAULT_DEADLINE_CAP_MS;
use marque_engine::CapcoEngine;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<CapcoEngine>,
    /// Upper bound for a caller-supplied `X-Marque-Deadline` header.
    /// When the caller omits the header, each
    /// endpoint applies its own default — 30 s for lint and fix in
    /// MVP — so this field is only consulted when the header is
    /// present and must be range-checked.
    pub deadline_cap: Duration,
}

impl AppState {
    /// Construct an `AppState` with the default deadline cap. Tests
    /// and embedders that want to control the cap should use
    /// `AppState { engine, deadline_cap: ... }` directly.
    pub fn new(engine: Arc<CapcoEngine>) -> Self {
        Self {
            engine,
            deadline_cap: Duration::from_millis(DEFAULT_DEADLINE_CAP_MS),
        }
    }
}
