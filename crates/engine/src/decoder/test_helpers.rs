// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Shared test fixtures for `decoder/` sub-modules.
//!
//! `#[cfg(test)]`-only. `CapcoScheme::new()` builds non-trivial `Vec`
//! tables; constructing it once via `LazyLock` and borrowing
//! `&*TEST_SCHEME` avoids repeated allocation across the (large) unit
//! test surface in this directory. `deep_cx()` returns the canonical
//! deep-scan `ParseContext` (strict-evidence off, whitespace-preceded
//! on) used by every decoder-recognizer test.

use std::sync::LazyLock;

use marque_capco::CapcoScheme;
use marque_scheme::recognizer::ParseContext;

/// Shared scheme instance for the decoder test surface.
pub(super) static TEST_SCHEME: LazyLock<CapcoScheme> = LazyLock::new(CapcoScheme::new);

/// Canonical deep-scan `ParseContext`: strict-evidence off,
/// whitespace-preceded on. Mirrors the value the engine builds when
/// dispatching to the decoder after the strict recognizer returns
/// zero candidates.
pub(super) fn deep_cx() -> ParseContext {
    // `ParseContext` is `#[non_exhaustive]` (#176 staging step 1) — even
    // in-crate, prefer `default()` + field assignment so this fixture
    // mirrors the construction idiom external callers must use.
    let mut cx = ParseContext::default();
    cx.strict_evidence = false;
    cx.preceded_by_whitespace = true;
    cx
}
