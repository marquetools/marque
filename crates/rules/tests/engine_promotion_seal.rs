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
use marque_rules::{AppliedFix, Confidence, EnginePromotionToken, FixProposal, FixSource, RuleId};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

#[test]
fn documented_door_can_mint_token_from_outside_marque_rules() {
    // Test-fixture carve-out per Constitution V Principle V: the
    // synthetic `AppliedFix` exists only inside `tests/` and is
    // never commingled with engine output. The point of the test is
    // to prove the documented engine-only door is usable across the
    // crate boundary.
    let proposal = FixProposal::new(
        RuleId::new("E001"),
        FixSource::BuiltinRule,
        Span::new(0, 4),
        "TEST",
        "DONE",
        Confidence::strict(1.0),
        None,
    );
    let token = EnginePromotionToken::__engine_construct();
    let applied = AppliedFix::__engine_promote(
        proposal,
        UNIX_EPOCH + Duration::from_secs(0),
        Some(Arc::<str>::from("test")),
        false,
        Some(Arc::<str>::from("-")),
        token,
    );
    assert_eq!(applied.proposal.rule.as_str(), "E001");
    assert!(!applied.dry_run);
}
