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
//! supertrait bounds ever stopped enforcing those bounds we would
//! only discover it later as a compile-time error at every call site
//! that tried to send the trait object across a task boundary. The
//! `assert_impl_all!` macros below front-load that failure here so
//! the regression surfaces at this file's compile, not scattered
//! across every consumer of `Arc<dyn Rule>`.
//!
//! Companion file: `crates/scheme/tests/send_sync.rs` already pins the
//! `Recognizer` trait-object form. This file closes the equivalent
//! gap for `Rule` and `RuleSet` (Phase 4 review M2).

use std::sync::Arc;

use marque_rules::{Rule, RuleContext, RuleId, RuleSet};
use marque_scheme::ambiguity::Parsed;
use marque_scheme::category::Category;
use marque_scheme::constraint::Constraint;
use marque_scheme::lattice::{
    BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice,
};
use marque_scheme::template::Template;
use marque_scheme::{MarkingScheme, Scope};
use static_assertions::assert_impl_all;

// Local stub scheme for the trait-object Send + Sync proof. PR 3c.B
// made `Rule<S>` and `RuleSet<S>` generic over the marking scheme;
// the trait-object form is `Arc<dyn Rule<S>>` / `Arc<dyn RuleSet<S>>`
// for some concrete scheme. The Send + Sync bound flows through the
// supertrait declaration regardless of `S`, so a bound proof for any
// one scheme proves the property for every scheme.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct StubMarking;

impl JoinSemilattice for StubMarking {
    fn join(&self, _other: &Self) -> Self {
        StubMarking
    }
}

impl MeetSemilattice for StubMarking {
    fn meet(&self, _other: &Self) -> Self {
        StubMarking
    }
}

impl BoundedJoinSemilattice for StubMarking {
    fn bottom() -> Self {
        StubMarking
    }
}

impl BoundedMeetSemilattice for StubMarking {
    fn top() -> Self {
        StubMarking
    }
}

struct StubScheme;

impl MarkingScheme for StubScheme {
    type Token = ();
    type Marking = StubMarking;
    type ParseError = ();
    type OpenVocabRef = core::convert::Infallible;
    type Parsed<'src> = ();
    type Canonical = ();

    fn name(&self) -> &str {
        "StubScheme"
    }
    fn schema_version(&self) -> &str {
        "0.0.1"
    }
    fn categories(&self) -> &[Category] {
        &[]
    }
    fn constraints(&self) -> &[Constraint] {
        &[]
    }
    fn templates(&self) -> &[Template] {
        &[]
    }
    fn parse(&self, _input: &str) -> Result<Parsed<Self::Marking>, Self::ParseError> {
        Ok(Parsed::Unambiguous(StubMarking))
    }
    fn project(&self, _scope: Scope, _markings: &[Self::Marking]) -> Self::Marking {
        StubMarking
    }
    fn render_portion(&self, _m: &Self::Marking) -> String {
        String::new()
    }
    fn render_banner(&self, _m: &Self::Marking) -> String {
        String::new()
    }
    fn render_canonical(
        &self,
        _m: &Self::Marking,
        _ctx: &marque_scheme::RenderContext,
        _out: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        Ok(())
    }
}

assert_impl_all!(Box<dyn Rule<StubScheme>>: Send, Sync);
assert_impl_all!(Arc<dyn Rule<StubScheme>>: Send, Sync);
assert_impl_all!(Box<dyn RuleSet<StubScheme>>: Send, Sync);
assert_impl_all!(Arc<dyn RuleSet<StubScheme>>: Send, Sync);

// T044 (rust-reviewer M3) — pin `RuleId: Send + Sync + Copy`. The
// 2-tuple `RuleId { scheme, predicate_id }` has only `&'static str`
// fields, so the property holds today by construction. The pin
// makes the safety property machine-verifiable rather than assumed
// from the derive list.
assert_impl_all!(RuleId: Send, Sync, Copy);

// PR 6c (T069) compile-time pin on `RuleContext: Send + Sync`.
//
// PR 6c deleted the `assert_impl_all!(PageContext: Send, Sync)` pin
// in `crates/ism/tests/send_sync.rs` when the `PageContext` newtype
// was retired. The new field type on `RuleContext` is
// `Option<Arc<Box<[CanonicalAttrs]>>>`, which is `Send + Sync` iff
// `CanonicalAttrs: Send + Sync` (asserted in
// `crates/ism/tests/send_sync.rs`). This file closes the gap by
// asserting the property on `RuleContext` itself.
//
// `RuleContext<'a>` is lifetime-parameterized so `assert_impl_all!`
// (which requires `'static`) cannot be applied directly. The HRTB
// function-bound form below proves the property for every `'a` at
// compile time.
fn _rule_context_is_send_sync<'a>()
where
    RuleContext<'a>: Send + Sync,
{
}
