// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Phase 3 US1 — Declarative constraint evaluator tests (T023–T025).
//!
//! Exercises [`marque_scheme::constraint::evaluate`] against a local
//! `StubScheme` whose marking carries a presence set over sentinel
//! [`TokenId`]s. The evaluator is deterministic, declaration-ordered,
//! and copies each triggering constraint's citation into the emitted
//! violation.

use marque_scheme::{
    Category, CategoryId, Constraint, ConstraintViolation, Lattice, MarkingScheme, PageRewrite,
    Parsed, Scope, Template, TokenId, TokenRef, constraint::evaluate,
};

// ---------------------------------------------------------------------------
// StubScheme + StubMarking
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Default)]
struct StubMarking {
    tokens: Vec<TokenId>,
    category_members: Vec<(CategoryId, TokenId)>,
}

impl Lattice for StubMarking {
    fn join(&self, _: &Self) -> Self {
        self.clone()
    }
    fn meet(&self, _: &Self) -> Self {
        self.clone()
    }
}

struct StubScheme {
    constraints: Vec<Constraint>,
}

impl StubScheme {
    fn new(constraints: Vec<Constraint>) -> Self {
        Self { constraints }
    }
}

impl MarkingScheme for StubScheme {
    type Token = TokenId;
    type Marking = StubMarking;
    type ParseError = ();
    fn name(&self) -> &str {
        "stub"
    }
    fn schema_version(&self) -> &str {
        "v0"
    }
    fn categories(&self) -> &[Category] {
        &[]
    }
    fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }
    fn templates(&self) -> &[Template] {
        &[]
    }
    fn parse(&self, _: &str) -> Result<Parsed<Self::Marking>, Self::ParseError> {
        Err(())
    }
    fn satisfies(&self, marking: &Self::Marking, token_ref: &TokenRef) -> bool {
        match token_ref {
            TokenRef::Token(id) => marking.tokens.contains(id),
            TokenRef::AnyInCategory(cat) => marking
                .category_members
                .iter()
                .any(|(c, _)| c == cat),
        }
    }
    fn evaluate_custom(
        &self,
        name: &'static str,
        _marking: &Self::Marking,
    ) -> Vec<ConstraintViolation> {
        // Return a sentinel violation so the custom dispatch is
        // observable from tests without needing real predicate logic.
        vec![ConstraintViolation {
            constraint_label: "custom-stub",
            message: format!("custom fired: {name}"),
            citation: "stub-custom",
        }]
    }
    fn project(&self, _: Scope, _: &[Self::Marking]) -> Self::Marking {
        StubMarking::default()
    }
    fn page_rewrites(&self) -> &[PageRewrite<Self>] {
        &[]
    }
    fn render_portion(&self, _: &Self::Marking) -> String {
        String::new()
    }
    fn render_banner(&self, _: &Self::Marking) -> String {
        String::new()
    }
}

const TOK_A: TokenId = TokenId(10);
const TOK_B: TokenId = TokenId(11);
const CAT_FOO: CategoryId = CategoryId(1);

// ---------------------------------------------------------------------------
// T023 — Determinism across threads.
// ---------------------------------------------------------------------------

#[test]
fn evaluate_is_deterministic() {
    let scheme = StubScheme::new(vec![
        Constraint::Conflicts {
            name: "test/ab-conflict",
            left: TokenRef::Token(TOK_A),
            right: TokenRef::Token(TOK_B),
            label: "TEST §1",
        },
        Constraint::Requires {
            name: "test/a-requires-foo",
            left: TokenRef::Token(TOK_A),
            right: TokenRef::AnyInCategory(CAT_FOO),
            label: "TEST §2",
        },
    ]);
    let marking = StubMarking {
        tokens: vec![TOK_A, TOK_B],
        category_members: vec![],
    };

    let a_handle = std::thread::spawn({
        let m = marking.clone();
        let s = StubScheme::new(scheme.constraints.clone());
        move || evaluate(&s, &m)
    });
    let b_handle = std::thread::spawn({
        let m = marking.clone();
        let s = StubScheme::new(scheme.constraints.clone());
        move || evaluate(&s, &m)
    });
    let a = a_handle.join().unwrap();
    let b = b_handle.join().unwrap();
    assert_eq!(a.len(), b.len());
    for (va, vb) in a.iter().zip(b.iter()) {
        assert_eq!(va.constraint_label, vb.constraint_label);
        assert_eq!(va.citation, vb.citation);
        assert_eq!(va.message, vb.message);
    }
    // Conflicts fires (both A and B present); Requires fires (A present,
    // but category FOO is empty). Two violations.
    assert_eq!(a.len(), 2);
}

// ---------------------------------------------------------------------------
// T024 — Empty constraint set returns empty.
// ---------------------------------------------------------------------------

#[test]
fn empty_constraints_returns_empty() {
    let scheme = StubScheme::new(vec![]);
    let marking = StubMarking {
        tokens: vec![TOK_A],
        category_members: vec![],
    };
    let v = evaluate(&scheme, &marking);
    assert!(v.is_empty());
}

// ---------------------------------------------------------------------------
// T025 — Conflict violation carries the declared citation verbatim.
// ---------------------------------------------------------------------------

#[test]
fn conflict_violation_preserves_citation() {
    let scheme = StubScheme::new(vec![Constraint::Conflicts {
        name: "test/conflict",
        left: TokenRef::Token(TOK_A),
        right: TokenRef::Token(TOK_B),
        label: "CAPCO-2016 §H.4",
    }]);
    let marking = StubMarking {
        tokens: vec![TOK_A, TOK_B],
        category_members: vec![],
    };
    let v = evaluate(&scheme, &marking);
    assert_eq!(v.len(), 1);
    assert_eq!(
        v[0].citation,
        "CAPCO-2016 §H.4",
        "citation must be copied verbatim from the triggering constraint"
    );
    assert_eq!(
        v[0].constraint_label, "test/conflict",
        "constraint_label must be the declared `name` — not a generic 'conflicts' string"
    );
}

#[test]
fn constraint_label_maps_to_declared_name_per_entry() {
    // Two `Conflicts` constraints with the same variant but different
    // names — verify that violations carry the right `constraint_label`
    // for each, so a downstream consumer can trace a violation back
    // to the specific declared entry.
    let scheme = StubScheme::new(vec![
        Constraint::Conflicts {
            name: "test/ab-conflict",
            left: TokenRef::Token(TOK_A),
            right: TokenRef::Token(TOK_B),
            label: "TEST §1",
        },
        Constraint::Conflicts {
            name: "test/foo-conflict",
            left: TokenRef::Token(TOK_A),
            right: TokenRef::AnyInCategory(CAT_FOO),
            label: "TEST §2",
        },
    ]);
    let marking = StubMarking {
        tokens: vec![TOK_A, TOK_B],
        category_members: vec![(CAT_FOO, TOK_A)],
    };
    let v = evaluate(&scheme, &marking);
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].constraint_label, "test/ab-conflict");
    assert_eq!(v[1].constraint_label, "test/foo-conflict");
}

// ---------------------------------------------------------------------------
// Bonus — Implies / Supersedes are quiet; Custom dispatches.
// ---------------------------------------------------------------------------

#[test]
fn implies_does_not_emit_diagnostics() {
    let scheme = StubScheme::new(vec![Constraint::Implies {
        name: "test/implies",
        left: TokenRef::Token(TOK_A),
        right: TokenRef::Token(TOK_B),
        label: "TEST §implies",
    }]);
    let marking = StubMarking {
        tokens: vec![TOK_A],
        category_members: vec![],
    };
    assert!(evaluate(&scheme, &marking).is_empty());
}

#[test]
fn supersedes_does_not_emit_diagnostics() {
    let scheme = StubScheme::new(vec![Constraint::Supersedes {
        name: "test/supersedes",
        left: TokenRef::Token(TOK_A),
        right: TokenRef::Token(TOK_B),
        label: "TEST §supersedes",
    }]);
    let marking = StubMarking {
        tokens: vec![TOK_A, TOK_B],
        category_members: vec![],
    };
    assert!(evaluate(&scheme, &marking).is_empty());
}

#[test]
fn custom_dispatches_through_scheme_and_normalizes_identifiers() {
    // StubScheme::evaluate_custom returns a violation with
    // constraint_label="custom-stub" and citation="stub-custom".
    // The evaluator MUST override both so the violation surfaces the
    // declared Constraint's `name` and `label` — that is the
    // traceability invariant called out in the `Constraint` docs.
    let scheme = StubScheme::new(vec![Constraint::Custom {
        name: "my-custom",
        label: "TEST §custom",
    }]);
    let marking = StubMarking::default();
    let v = evaluate(&scheme, &marking);
    assert_eq!(v.len(), 1);
    assert_eq!(
        v[0].constraint_label, "my-custom",
        "constraint_label must be overridden to the declared name"
    );
    assert_eq!(
        v[0].citation, "TEST §custom",
        "citation must be overridden to the declared label"
    );
    assert!(
        v[0].message.contains("my-custom"),
        "the scheme's message survives the override"
    );
}
