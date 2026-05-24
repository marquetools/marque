// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use std::sync::Arc;

fn build_engine() -> Engine {
    Engine::default()
}

fn bad() -> Engine {
    // MASKING-PIN: missing-issue-number
    build_engine().with_recognizer(Arc::new(StrictRecognizer::new()))
}
