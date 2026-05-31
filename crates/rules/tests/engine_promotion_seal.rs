// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Acceptance test for the type-level seal on [`AppliedFix::__engine_promote`]
//! (Constitution V Principle V).
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
//!
//! This test exercises the [`marque_rules::audit::AppliedFix`] type
//! via its `__engine_promote` constructor.

use marque_rules::audit::AppliedFix as AuditAppliedFix;
use marque_rules::{
    EnginePromotionToken, FixIntent, FixSource, Message, MessageArgs, MessageTemplate, Recognition,
    RuleId, Severity,
};
use marque_scheme::canonical::Canonical;
use marque_scheme::{
    MarkingScheme, ReplacementIntent, Scope, Span, TokenId,
    ambiguity::Parsed,
    category::Category,
    constraint::Constraint,
    fix_intent::RecanonScope,
    lattice::{BoundedJoinSemilattice, BoundedMeetSemilattice, JoinSemilattice, MeetSemilattice},
    template::Template,
};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

// Local stub scheme so the test compiles without depending on
// `marque-capco`. `AppliedFix<S>` is generic over the marking scheme;
// the seal test exercises only the promotion path, so the scheme
// choice is incidental.
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
    fn render_item(&self, _m: &Self::Marking) -> String {
        String::new()
    }
    fn render_summary(&self, _m: &Self::Marking) -> String {
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

#[test]
fn documented_door_can_mint_token_from_outside_marque_rules() {
    // Test-fixture carve-out per Constitution V Principle V: the
    // synthetic `AppliedFix` exists only inside `tests/` and is
    // never commingled with engine output. The point of the test is
    // to prove the documented engine-only door is usable across the
    // crate boundary, via
    // [`marque_rules::audit::AppliedFix::__engine_promote`].
    let intent: FixIntent<StubScheme> = FixIntent {
        replacement: ReplacementIntent::Recanonicalize {
            scope: RecanonScope::Portion,
            prior: None,
        },
        confidence: Recognition::strict(),
        feature_ids: Default::default(),
        message: Message::new(
            // Engine-promotion-seal fixture (synthetic test rule);
            // `UnrecognizedToken` is the generic closed-set template
            // used by the engine's `stub_message()` fixture pattern.
            // The point of this test is the seal mechanism, not the
            // template (the choice aligns with the audit-record
            // contract — issue #709).
            MessageTemplate::UnrecognizedToken,
            MessageArgs::default(),
        ),
        source: FixSource::BuiltinRule,
        migration_ref: None,
    };
    // The promotion path needs a `Canonical<StubScheme>`. The public
    // `from_cve` constructor takes a `TokenId` + `Scope` + `Box<str>`;
    // no scheme-specific token surface is required, so the seal test
    // passes through here cleanly without depending on `marque-capco`.
    let canonical: Canonical<StubScheme> =
        Canonical::from_cve(TokenId(0), Scope::Portion, Box::from("(S)"));
    let original_bytes: &[u8] = b"(S)";
    // Test-fixture carve-out per Constitution V
    let token = EnginePromotionToken::__engine_construct();
    // Test-fixture carve-out per Constitution V
    let applied: AuditAppliedFix<StubScheme> = AuditAppliedFix::__engine_promote(
        // Constitution V Principle V test-fixture carve-out exercising
        // the seal mechanism — the rule identity is incidental, so use
        // the reserved `test` scheme.
        RuleId::new("test", "synthetic.engine-promotion-seal-fixture"),
        Severity::Fix,
        Span::new(0, 4),
        intent,
        original_bytes,
        canonical,
        UNIX_EPOCH + Duration::from_secs(0),
        Some(Arc::<str>::from("test")),
        false,
        Some(Arc::<str>::from("-")),
        token,
    );
    // The wire-string form via `Display` is the user-facing identifier:
    // `"<scheme>:<predicate_id>"`.
    assert_eq!(
        applied.rule.to_string(),
        "test:synthetic.engine-promotion-seal-fixture"
    );
    assert!(!applied.dry_run);
}
