// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Compile-time `Send + Sync` check on `CanonicalAttrs`.
//!
//! `CanonicalAttrs` is the load-bearing axiom for the per-page
//! accumulator's thread-safety: the engine wraps a slice of these in
//! `Arc<Box<[CanonicalAttrs]>>` and hands it to rules via
//! [`marque_rules::RuleContext::with_page_portions`]. Constitution
//! Principle VI says rule implementations MUST be `Send + Sync`, and
//! `Arc<Box<[T]>>: Send + Sync` iff `T: Send + Sync` — so this pin is
//! what makes the engine's banner / CAB / PageFinalization hand-off
//! sound under cross-task dispatch. A change that introduces an
//! `Rc<_>`, `RefCell<_>`, or any other non-thread-safe field on
//! `CanonicalAttrs` fails to compile here.
//!
//! The thread-safety pin lives on the foundational type
//! (`CanonicalAttrs`); the parallel pin on `RuleContext` lives in
//! `crates/rules/tests/send_sync.rs`.

use marque_ism::CanonicalAttrs;
use static_assertions::assert_impl_all;

assert_impl_all!(CanonicalAttrs: Send, Sync);
