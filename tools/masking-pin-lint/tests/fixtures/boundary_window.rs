// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use std::sync::Arc;

// This fixture exercises the 5-line lookback boundary. Marker on line 12,
// call on line 17 — exactly 5 lines apart, the inclusive boundary.

fn build_engine() -> Engine {
    Engine::default()
}
// MASKING-PIN: tracks #100 — boundary-window check.



fn within_window() -> Engine {
    build_engine().with_recognizer(Arc::new(StrictRecognizer::new()))
}

// This second pin tests the OTHER side of the boundary. Marker on line 23,
// call on line 30 — 7 lines apart, outside the 5-line window.

// MASKING-PIN: tracks #200 — far outside window, should not match.





fn outside_window() -> Engine {
    build_engine().with_recognizer(Arc::new(StrictRecognizer::new()))
}
