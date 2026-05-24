// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

use std::sync::Arc;

fn build_engine() -> Engine {
    Engine::default()
}

fn dual_marker() -> Engine {
    // MASKING-PIN: tracks #999 — confused about why this is here.
    // INTENTIONAL-STRICT: also confused.
    build_engine().with_recognizer(Arc::new(StrictRecognizer::new()))
}
