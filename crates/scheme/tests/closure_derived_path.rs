// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Unit tests for the PR 4b-D.0 `ClosureRule::cone_derived` field.
//!
//! Two scenarios:
//!
//! 1. `closure_derived_path_routes_facts` — verifies that a closure rule
//!    whose only cone source is `cone_derived` actually adds the derived
//!    fact to the marking through the operator.
//! 2. `closure_derived_none_static_path_parity` — verifies observational
//!    equivalence of the two cone-emission paths: a rule that emits TOK_Y
//!    via the static `cone` field (with `cone_derived: None`) produces the
//!    same closure() output as a rule that emits TOK_Y via `cone_derived`
//!    (with `cone: &[]`), given the same input marking. This is a stronger
//!    invariant than a vacuous "None-branch is a no-op" test: it asserts
//!    that for logically equivalent fact contributions, the two structurally
//!    distinct dispatch paths through the closure executor produce
//!    byte-identical output. The vacuous "adding `cone_derived: None` to an
//!    existing static row changes nothing" property is guaranteed by the
//!    shape of the code — the `None` branch is simply skipped — and is not
//!    independently testable.

use marque_scheme::{
    Category, ClosureRule, Constraint, ConstraintViolation, JoinSemilattice, MarkingScheme,
    MeetSemilattice, PageRewrite, Parsed, Scope, Template, TokenId, TokenRef, category::CategoryId,
    closure::MAX_CLOSURE_ITERATIONS, severity::Severity,
};
use smallvec::{SmallVec, smallvec};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct BitMarking {
    bits: u8,
}

impl BitMarking {
    const fn with(bits: u8) -> Self {
        Self { bits }
    }

    const fn has_token(&self, n: u8) -> bool {
        (self.bits >> n) & 1 == 1
    }
}

impl JoinSemilattice for BitMarking {
    fn join(&self, other: &Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl MeetSemilattice for BitMarking {
    fn meet(&self, other: &Self) -> Self {
        Self {
            bits: self.bits & other.bits,
        }
    }
}

const TOK_X: TokenId = TokenId(0);
const TOK_Y: TokenId = TokenId(1);
const CAT_X: CategoryId = CategoryId(0);

fn bit_index(id: TokenId) -> Option<u8> {
    match id {
        TokenId(0) => Some(0),
        TokenId(1) => Some(1),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Scheme 1: cone_derived emits TOK_Y unconditionally.
// ---------------------------------------------------------------------------

fn derived_cone_emits_y(_m: &BitMarking) -> SmallVec<[(CategoryId, TokenRef); 2]> {
    smallvec![(CAT_X, TokenRef::Token(TOK_Y))]
}

static DERIVED_ONLY_RULES: &[ClosureRule<DerivedOnlyScheme>] = &[ClosureRule {
    name: "derived/emits-y",
    label: "derived-only test fixture",
    triggers: &[],
    suppressors: &[],
    cone: &[],
    cone_derived: Some(derived_cone_emits_y),
    default_severity: Severity::Info,
}];

struct DerivedOnlyScheme;

impl MarkingScheme for DerivedOnlyScheme {
    type Token = TokenId;
    type Marking = BitMarking;
    type ParseError = ();
    type OpenVocabRef = core::convert::Infallible;

    fn name(&self) -> &str {
        "derived-only-stub"
    }
    fn schema_version(&self) -> &str {
        "v0"
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
    fn parse(&self, _: &str) -> Result<Parsed<Self::Marking>, Self::ParseError> {
        Err(())
    }
    fn satisfies(&self, marking: &Self::Marking, token_ref: &TokenRef) -> bool {
        match token_ref {
            TokenRef::Token(id) => bit_index(*id).is_some_and(|n| marking.has_token(n)),
            TokenRef::AnyInCategory(_) => false,
        }
    }
    fn project(&self, _: Scope, _: &[Self::Marking]) -> Self::Marking {
        BitMarking::default()
    }
    fn page_rewrites(&self) -> &[PageRewrite<Self>] {
        &[]
    }
    fn evaluate_custom(&self, _: &'static str, _: &Self::Marking) -> Vec<ConstraintViolation> {
        Vec::new()
    }
    fn render_canonical(
        &self,
        m: &Self::Marking,
        _: Scope,
        out: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        write!(out, "bits={:08b}", m.bits)
    }
    fn closure_rules(&self) -> &[ClosureRule<Self>] {
        DERIVED_ONLY_RULES
    }
    fn iter_present_tokens<'m>(
        &self,
        marking: &'m Self::Marking,
    ) -> Box<dyn Iterator<Item = TokenRef> + 'm> {
        let bits = marking.bits;
        Box::new(
            [TOK_X, TOK_Y]
                .into_iter()
                .filter(move |t| bit_index(*t).is_some_and(|n| (bits >> n) & 1 == 1))
                .map(TokenRef::Token),
        )
    }
    fn closure(&self, marking: Self::Marking) -> Self::Marking {
        let mut working = marking;
        for _iter in 0..MAX_CLOSURE_ITERATIONS {
            let prev = working.bits;
            for rule in DERIVED_ONLY_RULES {
                if rule.should_fire(self, &working) {
                    for id in rule.cone_token_ids() {
                        if let Some(n) = bit_index(id) {
                            working.bits |= 1 << n;
                        }
                    }
                    if let Some(derived_fn) = rule.cone_derived {
                        for (_cat, token_ref) in derived_fn(&working) {
                            if let TokenRef::Token(id) = token_ref {
                                if let Some(n) = bit_index(id) {
                                    working.bits |= 1 << n;
                                }
                            }
                        }
                    }
                }
            }
            if working.bits == prev {
                return working;
            }
        }
        panic!("derived-only closure did not converge");
    }
}

#[test]
fn closure_derived_path_routes_facts() {
    let scheme = DerivedOnlyScheme;
    // Start with TOK_X set, TOK_Y absent.
    let m = BitMarking::with(0b01);
    let closed = scheme.closure(m);
    // The cone_derived fn unconditionally emits TOK_Y, so the operator
    // MUST route that fact into the bitset.
    assert!(
        closed.has_token(1),
        "cone_derived fact TOK_Y was not added to the closed marking: closed.bits = {:08b}",
        closed.bits
    );
}

// ---------------------------------------------------------------------------
// Scheme 2 (parity): static `cone` carries TOK_Y; cone_derived is None.
// ---------------------------------------------------------------------------

static STATIC_PARITY_RULES: &[ClosureRule<StaticParityScheme>] = &[ClosureRule {
    name: "static/emits-y",
    label: "static-cone parity fixture",
    triggers: &[],
    suppressors: &[],
    cone: &[TokenRef::Token(TOK_Y)],
    cone_derived: None,
    default_severity: Severity::Info,
}];

struct StaticParityScheme;

impl MarkingScheme for StaticParityScheme {
    type Token = TokenId;
    type Marking = BitMarking;
    type ParseError = ();
    type OpenVocabRef = core::convert::Infallible;

    fn name(&self) -> &str {
        "static-parity-stub"
    }
    fn schema_version(&self) -> &str {
        "v0"
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
    fn parse(&self, _: &str) -> Result<Parsed<Self::Marking>, Self::ParseError> {
        Err(())
    }
    fn satisfies(&self, marking: &Self::Marking, token_ref: &TokenRef) -> bool {
        match token_ref {
            TokenRef::Token(id) => bit_index(*id).is_some_and(|n| marking.has_token(n)),
            TokenRef::AnyInCategory(_) => false,
        }
    }
    fn project(&self, _: Scope, _: &[Self::Marking]) -> Self::Marking {
        BitMarking::default()
    }
    fn page_rewrites(&self) -> &[PageRewrite<Self>] {
        &[]
    }
    fn evaluate_custom(&self, _: &'static str, _: &Self::Marking) -> Vec<ConstraintViolation> {
        Vec::new()
    }
    fn render_canonical(
        &self,
        m: &Self::Marking,
        _: Scope,
        out: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        write!(out, "bits={:08b}", m.bits)
    }
    fn closure_rules(&self) -> &[ClosureRule<Self>] {
        STATIC_PARITY_RULES
    }
    fn iter_present_tokens<'m>(
        &self,
        marking: &'m Self::Marking,
    ) -> Box<dyn Iterator<Item = TokenRef> + 'm> {
        let bits = marking.bits;
        Box::new(
            [TOK_X, TOK_Y]
                .into_iter()
                .filter(move |t| bit_index(*t).is_some_and(|n| (bits >> n) & 1 == 1))
                .map(TokenRef::Token),
        )
    }
    fn closure(&self, marking: Self::Marking) -> Self::Marking {
        let mut working = marking;
        for _iter in 0..MAX_CLOSURE_ITERATIONS {
            let prev = working.bits;
            for rule in STATIC_PARITY_RULES {
                if rule.should_fire(self, &working) {
                    for id in rule.cone_token_ids() {
                        if let Some(n) = bit_index(id) {
                            working.bits |= 1 << n;
                        }
                    }
                    if let Some(derived_fn) = rule.cone_derived {
                        for (_cat, token_ref) in derived_fn(&working) {
                            if let TokenRef::Token(id) = token_ref {
                                if let Some(n) = bit_index(id) {
                                    working.bits |= 1 << n;
                                }
                            }
                        }
                    }
                }
            }
            if working.bits == prev {
                return working;
            }
        }
        panic!("static-parity closure did not converge");
    }
}

#[test]
fn closure_derived_none_static_path_parity() {
    // Run both schemes against the same input marking; the closure() result
    // must be byte-for-byte equal. The two schemes have structurally distinct
    // rule shapes — `DerivedOnlyScheme` carries `cone: &[]` with `cone_derived:
    // Some(_)` emitting TOK_Y, while `StaticParityScheme` carries `cone:
    // &[TOK_Y]` with `cone_derived: None` — but their fact contributions are
    // logically equivalent. Byte-equal closure() output proves the two
    // dispatch paths through the executor are observationally equivalent for
    // equivalent fact sets; it is NOT a test that adding `cone_derived: None`
    // to a static row is a no-op (that property is guaranteed by code shape
    // and is not independently testable).
    let input = BitMarking::with(0b01);

    let derived = DerivedOnlyScheme;
    let static_scheme = StaticParityScheme;

    let c_derived = derived.closure(input.clone());
    let c_static = static_scheme.closure(input);

    assert_eq!(
        c_derived.bits, c_static.bits,
        "derived-cone path and static-cone path must produce byte-identical \
         closure() output for the same logical fact: derived={:08b}, static={:08b}",
        c_derived.bits, c_static.bits
    );
}
