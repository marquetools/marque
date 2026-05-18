// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! PR 4b-E: compile-time `Send + Sync` checks on `PageContext` and
//! `CanonicalAttrs`.
//!
//! `PageContext` is wrapped in [`std::sync::Arc`] and handed to rules via
//! [`marque_rules::RuleContext::with_page_context`]; Constitution Principle
//! VI says rule implementations MUST be `Send + Sync`. The fields are
//! structurally `Send + Sync` (`Vec<CanonicalAttrs>` over owned types),
//! but no explicit static assertion previously pinned the contract.
//! This file is the pin — a change that introduces an `Rc<_>`,
//! `RefCell<_>`, or any other non-thread-safe field fails to compile.
//!
//! Per `docs/plans/2026-05-18-pr4b-E-rust-preflight.md` Risk #7.

use marque_ism::{CanonicalAttrs, PageContext};
use static_assertions::assert_impl_all;

assert_impl_all!(PageContext: Send, Sync);
assert_impl_all!(CanonicalAttrs: Send, Sync);
