// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Compile-time proof that `Rule` and `RuleSet` carry `Send + Sync`
//! correctly through their trait objects.
//!
//! Both traits declare `Send + Sync` as supertraits in
//! `crates/rules/src/lib.rs` (Constitution VI). The engine and
//! `BatchEngine` hold rules and rule-sets behind `Arc<dyn Rule>` /
//! `Arc<dyn RuleSet>` for cross-task dispatch; if the trait's
//! supertrait bounds ever stopped enforcing those bounds we would only
//! find out at runtime when cross-task dispatch stopped compiling at
//! the call site. The `assert_impl_all!` macros below turn that into
//! a compile-time failure here instead.
//!
//! Companion file: `crates/scheme/tests/send_sync.rs` already pins the
//! `Recognizer` trait-object form. This file closes the equivalent
//! gap for `Rule` and `RuleSet` (Phase 4 review M2).

use std::sync::Arc;

use marque_rules::{Rule, RuleSet};
use static_assertions::assert_impl_all;

assert_impl_all!(Box<dyn Rule>: Send, Sync);
assert_impl_all!(Arc<dyn Rule>: Send, Sync);
assert_impl_all!(Box<dyn RuleSet>: Send, Sync);
assert_impl_all!(Arc<dyn RuleSet>: Send, Sync);
