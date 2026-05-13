// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Acceptance test for the type-level seal on [`AppliedFix::__engine_promote`]
//! (gap register #5, Constitution V Principle V).
//!
//! `EnginePromotionToken` has a private `_seal: ()` field, so external
//! crates cannot brace-construct one. The single bypass surface is
//! `EnginePromotionToken::__engine_construct()`, which is
//! `#[doc(hidden)]` and engine-only by convention. This file pins both
//! halves:
//!
//! - The brace-construct path is enforced as a `compile_fail` doctest
//!   on `EnginePromotionToken` in `crates/rules/src/lib.rs` — see the
//!   doc comment on that type.
//! - The documented door (`__engine_construct()`) works from outside
//!   `marque-rules`, exercised below as the test-fixture carve-out
//!   per Constitution V Principle V.
//!
//! Integration tests compile as separate crates that link against the
//! library, so this file sees only the public API surface — the same
//! visibility a downstream consumer would see.

use marque_ism::Span;
use marque_rules::{
    AppliedFix, Confidence, EnginePromotionToken, FixIntent, FixSource, Message, MessageArgs,
    MessageTemplate, RuleId,
};
use marque_scheme::{
    MarkingScheme, ReplacementIntent, Scope,
    ambiguity::Parsed,
    category::Category,
    constraint::Constraint,
    fix_intent::RecanonScope,
    lattice::{BoundedLattice, Lattice},
    template::Template,
};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

// Local stub scheme so the test compiles without depending on
// `marque-capco`. `AppliedFix<S>` is generic over the marking scheme
// post-PR 3c.B; the seal test only exercises the legacy promotion
// path so the scheme choice is incidental.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct StubMarking;

impl Lattice for StubMarking {
    fn join(&self, _other: &Self) -> Self {
        StubMarking
    }
    fn meet(&self, _other: &Self) -> Self {
        StubMarking
    }
}

impl BoundedLattice for StubMarking {
    fn bottom() -> Self {
        StubMarking
    }
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
        _scope: Scope,
        _out: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        Ok(())
    }
}

#[test]
fn documented_door_can_mint_token_from_outside_marque_rules() {
    // Test-fixture carve-out per Constitution V Principle V: the
    // synthetic `AppliedFix` exists only inside `tests/` and is
    // never commingled with engine output. The point of the test is
    // to prove the documented engine-only door is usable across the
    // crate boundary.
    let intent: FixIntent<StubScheme> = FixIntent {
        replacement: ReplacementIntent::Recanonicalize {
            scope: RecanonScope::Portion,
        },
        confidence: Confidence::strict(1.0),
        feature_ids: Default::default(),
        message: Message::new(
            MessageTemplate::BannerRollupMismatch,
            MessageArgs::default(),
        ),
        source: FixSource::BuiltinRule,
        migration_ref: None,
    };
    // Test-fixture carve-out per Constitution V
    let token = EnginePromotionToken::__engine_construct();
    // Test-fixture carve-out per Constitution V
    let applied: AppliedFix<StubScheme> = AppliedFix::__engine_promote(
        RuleId::new("E001"),
        Span::new(0, 4),
        intent,
        UNIX_EPOCH + Duration::from_secs(0),
        Some(Arc::<str>::from("test")),
        false,
        Some(Arc::<str>::from("-")),
        token,
    );
    assert_eq!(applied.rule.as_str(), "E001");
    assert!(!applied.dry_run);
}
