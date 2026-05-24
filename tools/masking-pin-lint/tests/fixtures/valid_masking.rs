// SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

// Synthetic test fixture — not compiled, parsed by syn for AST tests.

use std::sync::Arc;

fn build_engine() -> Engine {
    Engine::default()
}

fn engine_for_test() -> Engine {
    // MASKING-PIN: tracks #258 — fixture for masking-pin-lint integration test.
    build_engine().with_recognizer(Arc::new(StrictRecognizer::new()))
}

fn intentional_engine() -> Engine {
    // INTENTIONAL-STRICT: this fixture asserts strict-path behavior in contrast to the default dispatcher.
    build_engine().with_recognizer(Arc::new(StrictRecognizer::new()))
}
