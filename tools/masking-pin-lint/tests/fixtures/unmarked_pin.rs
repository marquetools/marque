// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

// Synthetic test fixture — not compiled. Should be flagged as Unmarked.

use std::sync::Arc;

fn build_engine() -> Engine {
    Engine::default()
}

fn no_marker_at_all() -> Engine {
    build_engine().with_recognizer(Arc::new(StrictRecognizer::new()))
}
