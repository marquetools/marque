// SPDX-FileCopyrightText: 2026 Knitli Inc.
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! T037 — cross-crate smoke test for [`marque_rules::FixIntent`].
//!
//! Verifies that the rule-emission API is reachable and constructible
//! from outside `marque-rules` (the same surface a downstream rule
//! crate like `marque-capco` or a future `marque-cui` would see). The
//! richer per-variant unit tests live in `src/fix_intent.rs::tests`.
//!
//! PR 3c.B Commit 2 reshaped the variant set:
//! `Cve` / `Render` / `Delete` retired in favor of the bag-of-tokens
//! `FactAdd` / `FactRemove` / `Recanonicalize` vocabulary. See
//! `specs/006-engine-rule-refactor/architecture.md` "What fixes
//! are."

use marque_rules::{Confidence, FixIntent, Message, MessageArgs, MessageTemplate};
use marque_scheme::ambiguity::Parsed;
use marque_scheme::category::Category;
use marque_scheme::constraint::Constraint;
use marque_scheme::lattice::{BoundedLattice, Lattice};
use marque_scheme::template::Template;
use marque_scheme::{FactRef, MarkingScheme, RecanonScope, ReplacementIntent, Scope, TokenId};
use smallvec::SmallVec;

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
fn external_crate_constructs_fix_intent_with_fact_add() {
    let intent: FixIntent<StubScheme> = FixIntent {
        replacement: ReplacementIntent::FactAdd {
            token: FactRef::Cve(TokenId(1)),
            scope: Scope::Portion,
        },
        confidence: Confidence::strict(0.95),
        feature_ids: SmallVec::new(),
        message: Message::new(MessageTemplate::SupersededToken, MessageArgs::default()),
    };
    assert_eq!(intent.message.template(), MessageTemplate::SupersededToken);
    match intent.replacement {
        ReplacementIntent::FactAdd { token, scope } => {
            assert!(matches!(token, FactRef::Cve(TokenId(1))));
            assert_eq!(scope, Scope::Portion);
        }
        _ => panic!("expected FactAdd"),
    }
}

#[test]
fn external_crate_constructs_fix_intent_with_fact_remove() {
    let intent: FixIntent<StubScheme> = FixIntent {
        replacement: ReplacementIntent::FactRemove {
            token_ref: FactRef::Cve(TokenId(2)),
            scope: Scope::Page,
        },
        confidence: Confidence::strict(0.85),
        feature_ids: SmallVec::new(),
        message: Message::new(
            MessageTemplate::BannerRollupMismatch,
            MessageArgs::default(),
        ),
    };
    match intent.replacement {
        ReplacementIntent::FactRemove { token_ref, scope } => {
            assert!(matches!(token_ref, FactRef::Cve(TokenId(2))));
            assert_eq!(scope, Scope::Page);
        }
        _ => panic!("expected FactRemove"),
    }
}

#[test]
fn external_crate_constructs_fix_intent_with_recanonicalize() {
    let intent: FixIntent<StubScheme> = FixIntent {
        replacement: ReplacementIntent::Recanonicalize {
            scope: RecanonScope::Document,
        },
        confidence: Confidence::strict(1.0),
        feature_ids: SmallVec::new(),
        message: Message::new(MessageTemplate::ConflictsWith, MessageArgs::default()),
    };
    assert!(matches!(
        intent.replacement,
        ReplacementIntent::Recanonicalize {
            scope: RecanonScope::Document
        }
    ));
}
