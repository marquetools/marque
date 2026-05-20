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
    Category, CategoryId, Constraint, ConstraintViolation, JoinSemilattice, MarkingScheme,
    MeetSemilattice, PageRewrite, Parsed, Scope, Template, TokenId, TokenRef, constraint::evaluate,
};

// ---------------------------------------------------------------------------
// StubScheme + StubMarking
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Default)]
struct StubMarking {
    tokens: Vec<TokenId>,
    category_members: Vec<(CategoryId, TokenId)>,
}

impl JoinSemilattice for StubMarking {
    fn join(&self, _: &Self) -> Self {
        self.clone()
    }
}

impl MeetSemilattice for StubMarking {
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
    type OpenVocabRef = core::convert::Infallible;
    // PR 3c.2.A — GAT + plain associated type bindings. This stub
    // never exercises the canonicalize path, so `()` is the
    // lowest-information binding (the `unimplemented!()` default is
    // unreachable from this stub's code paths).
    type Parsed<'src> = ();
    type Canonical = ();
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
            TokenRef::AnyInCategory(cat) => marking.category_members.iter().any(|(c, _)| c == cat),
        }
    }
    fn evaluate_custom(
        &self,
        name: &'static str,
        _marking: &Self::Marking,
        _bits: marque_scheme::FactBitmask,
    ) -> Vec<ConstraintViolation> {
        // Return a sentinel violation so the custom dispatch is
        // observable from tests without needing real predicate logic.
        vec![ConstraintViolation {
            constraint_label: "custom-stub",
            message: format!("custom fired: {name}"),
            citation: "stub-custom",
            span: None,
            severity: None,
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
    fn render_canonical(
        &self,
        _: &Self::Marking,
        _: &marque_scheme::RenderContext,
        _: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        Ok(())
    }
}

const TOK_A: TokenId = TokenId(10);
const TOK_B: TokenId = TokenId(11);
const TOK_C: TokenId = TokenId(12);
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
            severity: None,
            span_anchor: None,
        },
        Constraint::Requires {
            name: "test/a-requires-foo",
            left: TokenRef::Token(TOK_A),
            right: TokenRef::AnyInCategory(CAT_FOO),
            label: "TEST §2",
            severity: None,
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
        severity: None,
        span_anchor: None,
    }]);
    let marking = StubMarking {
        tokens: vec![TOK_A, TOK_B],
        category_members: vec![],
    };
    let v = evaluate(&scheme, &marking);
    assert_eq!(v.len(), 1);
    assert_eq!(
        v[0].citation, "CAPCO-2016 §H.4",
        "citation must be copied verbatim from the triggering constraint"
    );
    assert_eq!(
        v[0].constraint_label, "test/conflict",
        "constraint_label must be the declared `name` — not a generic 'conflicts' string"
    );
}

#[test]
fn dyadic_arm_violations_default_to_none_span_and_severity() {
    // Sentinel test (review-pass MEDIUM, PR 3c.B Commit 7.2): the
    // dyadic `Conflicts` / `Requires` arms of `evaluate` MUST emit
    // violations with `span: None` and `severity: None`. The engine's
    // constraint-catalog bridge (`crates/engine/src/engine.rs:776-851`)
    // skips such violations as advisory-only — they are detected but
    // not surfaced as `Diagnostic`s.
    //
    // A future PR that flips the dyadic arms to emit populated
    // `Option<Span>` / `Option<Severity>` (giving them user-facing
    // diagnostics) would silently change the engine's behavior — the
    // bridge would start emitting Diagnostics for every dyadic catalog
    // constraint that fires, which is NOT today's intent. This test
    // pins the property: the dyadic arms emit advisory-only signals
    // by construction; only `Constraint::Custom`-arm catalog rows
    // populated by the scheme's `evaluate_custom` may produce
    // user-facing diagnostics through the bridge.
    let scheme = StubScheme::new(vec![
        Constraint::Conflicts {
            name: "test/conflict",
            left: TokenRef::Token(TOK_A),
            right: TokenRef::Token(TOK_B),
            label: "CAPCO-2016 §H.4",
            severity: None,
            span_anchor: None,
        },
        Constraint::Requires {
            name: "test/requires",
            left: TokenRef::Token(TOK_A),
            right: TokenRef::Token(TOK_C),
            label: "CAPCO-2016 §H.5",
            severity: None,
        },
    ]);
    let marking = StubMarking {
        tokens: vec![TOK_A, TOK_B],
        category_members: vec![],
    };
    let v = evaluate(&scheme, &marking);
    assert_eq!(v.len(), 2, "both dyadic constraints must fire");
    for violation in &v {
        assert!(
            violation.span.is_none(),
            "dyadic-arm violations MUST emit None span (advisory-only); \
             got Some({:?}) on {:?}",
            violation.span,
            violation.constraint_label,
        );
        assert!(
            violation.severity.is_none(),
            "dyadic-arm violations MUST emit None severity (advisory-only); \
             got Some({:?}) on {:?}",
            violation.severity,
            violation.constraint_label,
        );
    }
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
            severity: None,
            span_anchor: None,
        },
        Constraint::Conflicts {
            name: "test/foo-conflict",
            left: TokenRef::Token(TOK_A),
            right: TokenRef::AnyInCategory(CAT_FOO),
            label: "TEST §2",
            severity: None,
            span_anchor: None,
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
// Bonus — Supersedes is quiet; Custom dispatches.
// Note: Constraint::Implies was retired in PR 3.7 T108g (decisions.md D19 C).
// Fact-propagation is now handled by the closure operator (ClosureRule).
// ---------------------------------------------------------------------------

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
