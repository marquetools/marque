// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

//! Negative property test: non-monotone closure-catalog bugs produce
//! an observable monotonicity violation.
//!
//! ## What this file actually tests
//!
//! Three tests exercise the negative property in increasing fidelity:
//!
//! 1. `monotone_catalog_satisfies_monotonicity` — positive reference
//!    case (a `MonotoneScheme` with an unconditional A→B rule satisfies
//!    `closure(m1) ⊑ closure(m2)` for `m1 ⊑ m2`).
//! 2. `non_monotone_scenario_is_detectable` — hand-computed asserts
//!    that the violation can be EXPRESSED as a data structure;
//!    asserts the disjoint-suppressor invariant on the monotone
//!    catalog (no suppressor token appears in any cone).
//! 3. `non_monotone_synthetic_scheme_violates_monotonicity_observably`
//!    — constructs a `NonMonotoneScheme` with the `A→B suppressed by C`
//!    token-presence rule (C is in the marking universe and can be
//!    added by other rules, so the disjoint-suppressor invariant is
//!    violated); observes that `closure({A}) = {A, B}` but
//!    `closure({A, C}) = {A, C}` — a real monotonicity violation
//!    produced by walking the synthetic scheme's `closure()` impl.
//!
//! The third test is the load-bearing one: it exercises the negative
//! property through the closure operator itself rather than via
//! hand-rolled bool arithmetic. The synthetic closure rule has a
//! non-monotone suppressor, and the test asserts the closure operator's
//! monotonicity property fails.

use marque_scheme::{
    Category, Citation, Constraint, ConstraintViolation, FactRef, JoinSemilattice, MarkingScheme,
    MeetSemilattice, PageRewrite, Parsed, RenderContext, Scope, SectionLetter, Template, TokenId,
    TokenRef, closure::ClosureRule, severity::Severity,
};
use proptest::prelude::*;

// Sentinel test citation — non-monotone proptest fixtures; routes through
// `AuthoritativeSource::EngineInternal` so Display renders `[engine-internal]`.
const NON_MONOTONE_CITATION: Citation = Citation::new(
    marque_scheme::AuthoritativeSource::EngineInternal,
    marque_scheme::SectionRef::new(SectionLetter::A),
    match core::num::NonZeroU16::new(1) {
        Some(n) => n,
        None => unreachable!(),
    },
);

// ---------------------------------------------------------------------------
// Bitset marking (same as proptest_closure.rs but standalone).
// ---------------------------------------------------------------------------

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

    /// `m1 ⊑ m2` in the bitset lattice.
    fn le(&self, other: &Self) -> bool {
        (self.bits & other.bits) == self.bits
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

const TOK_A: TokenId = TokenId(0);
const TOK_B: TokenId = TokenId(1);
const TOK_C: TokenId = TokenId(2);

fn bit_index(id: TokenId) -> Option<u8> {
    match id {
        TokenId(0) => Some(0),
        TokenId(1) => Some(1),
        TokenId(2) => Some(2),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// A PROPERLY monotone catalog for reference (no suppressor).
// A→B, always. Adding TOK_A always adds TOK_B.
// ---------------------------------------------------------------------------

static MONOTONE_RULES: &[ClosureRule<MonotoneScheme>] = &[ClosureRule {
    name: "stub/a-implies-b",
    display_label: "Monotone fixture A implies B",
    label: NON_MONOTONE_CITATION,
    triggers: &[TokenRef::Token(TOK_A)],
    suppressors: &[], // No suppressor — unconditional.
    cone: &[TokenRef::Token(TOK_B)],
    cone_derived: None,
    default_severity: Severity::Info,
}];

struct MonotoneScheme;

impl MarkingScheme for MonotoneScheme {
    type Token = TokenId;
    type Marking = BitMarking;
    type ParseError = ();
    type OpenVocabRef = core::convert::Infallible;
    type Parsed<'src> = ();
    type Canonical = ();

    fn name(&self) -> &str {
        "monotone-stub"
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
    fn evaluate_custom(
        &self,
        _: &'static str,
        _: &Self::Marking,
        _: marque_scheme::FactBitmask,
    ) -> Vec<ConstraintViolation> {
        Vec::new()
    }
    fn render_canonical(
        &self,
        m: &Self::Marking,
        _: &RenderContext,
        out: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        write!(out, "bits={:08b}", m.bits)
    }
    fn closure_rules(&self) -> &[ClosureRule<Self>] {
        MONOTONE_RULES
    }
    fn iter_present_tokens<'m>(
        &self,
        marking: &'m Self::Marking,
    ) -> Box<dyn Iterator<Item = TokenRef> + 'm> {
        let bits = marking.bits;
        Box::new(
            [TOK_A, TOK_B, TOK_C]
                .into_iter()
                .filter(move |t| bit_index(*t).is_some_and(|n| (bits >> n) & 1 == 1))
                .map(TokenRef::Token),
        )
    }
    fn closure(&self, marking: Self::Marking) -> Self::Marking {
        let mut working = marking;
        for _iter in 0..marque_scheme::closure::MAX_CLOSURE_ITERATIONS {
            let prev = working.bits;
            for rule in MONOTONE_RULES {
                if rule.should_fire(self, &working) {
                    for id in rule.cone_token_ids() {
                        if let Some(n) = bit_index(id) {
                            working.bits |= 1 << n;
                        }
                    }
                    if let Some(derived_fn) = rule.cone_derived {
                        for fact_ref in derived_fn(&working) {
                            // `OpenVocabRef = Infallible` makes `Cve` the only
                            // inhabitable variant — irrefutable destructure.
                            let FactRef::Cve(id) = fact_ref;
                            if let Some(n) = bit_index(id) {
                                working.bits |= 1 << n;
                            }
                        }
                    }
                }
            }
            if working.bits == prev {
                return working;
            }
        }
        panic!(
            "closure did not converge in {} iterations",
            marque_scheme::closure::MAX_CLOSURE_ITERATIONS
        );
    }
}

// ---------------------------------------------------------------------------
// Tests demonstrating the catalog properties.
// ---------------------------------------------------------------------------

/// A properly monotone catalog is monotone: m1 ⊑ m2 ⟹ closure(m1) ⊑ closure(m2).
///
/// This is the "positive" reference case — confirm the monotone catalog
/// behaves correctly before demonstrating what a violation looks like.
#[test]
fn monotone_catalog_satisfies_monotonicity() {
    let scheme = MonotoneScheme;

    // m1 = {A}, m2 = {A, C}: m1 ⊑ m2
    let m1 = BitMarking::with(0b001); // bit 0 = TOK_A
    let m2 = BitMarking::with(0b101); // bits 0,2 = TOK_A, TOK_C

    assert!(m1.le(&m2), "test setup: m1 must be ⊑ m2");

    let c1 = scheme.closure(m1);
    let c2 = scheme.closure(m2);

    // closure({A}) = {A, B} (A→B fires)
    // closure({A, C}) = {A, B, C} (A→B fires; C has no implication)
    // {A, B} ⊑ {A, B, C} — monotonicity holds.
    assert!(
        c1.le(&c2),
        "monotone catalog: closure(m1)={:08b} must be ⊑ closure(m2)={:08b}",
        c1.bits,
        c2.bits
    );
}

/// Demonstrates what a non-monotone catalog violation looks like.
///
/// We construct a violation scenario manually: a rule where the suppressor
/// is present in m2 but not in m1, causing closure(m2) to add FEWER facts
/// than closure(m1) for some axis — which would violate monotonicity.
///
/// The monotone catalog doesn't have this issue because suppressors in the
/// catalog are all token-presence checks (adding a token to m2 doesn't remove
/// a suppressor — suppressors are stable tokens, not computed predicates).
///
/// This test shows that we CAN construct the violation scenario as a data
/// structure, and that the monotone catalog correctly avoids it.
#[test]
fn non_monotone_scenario_is_detectable() {
    // Scenario: m1 ⊑ m2, but m2 has the "suppressor" token (TOK_C).
    // If the rule were "A→B suppressed by C", then:
    //   closure(m1 = {A}) = {A, B}   (suppressor absent, fires)
    //   closure(m2 = {A, C}) = {A, C} (suppressor present, DOESN'T fire)
    // This would give closure(m1) = {A,B} ⊄ closure(m2) = {A,C} — violation!
    //
    // The key insight: a suppressor shaped as "presence of a fact that can
    // be in the cone of a different rule" creates a non-monotone catalog.
    // The CAPCO catalog avoids this by ensuring suppressors are always
    // "dominator tokens" that are NEVER in the cone of any closure rule
    // (the FD&R dominators set is disjoint from all cones).

    // We demonstrate the violation exists conceptually by hand-computing
    // what a non-monotone catalog would produce. The companion test
    // `non_monotone_synthetic_scheme_violates_monotonicity_observably`
    // (test #3, below) constructs `NonMonotoneScheme` and runs its
    // `closure()` impl to observe the violation through the operator —
    // not through hand-rolled bool arithmetic. The closure operator does
    // NOT panic on a non-monotone catalog; it converges to a fact-set
    // that exposes the monotonicity break to the assertion in test #3.
    // The hand-computation below is kept for didactic clarity:

    // m1 = {A}: no suppressor
    let m1_has_a = true;
    let m1_has_c = false; // suppressor absent
    let m1_closure_adds_b = m1_has_a && !m1_has_c; // fires

    // m2 = {A, C}: suppressor present
    let m2_has_a = true;
    let m2_has_c = true; // suppressor present
    let m2_closure_adds_b = m2_has_a && !m2_has_c; // suppressed

    // The violation: closure({A}) adds B, closure({A,C}) does NOT add B
    // Conceptually: {A,B} ⊄ {A,C}
    assert!(
        m1_closure_adds_b && !m2_closure_adds_b,
        "test setup: non-monotone scenario must exhibit the violation"
    );

    // The key property: in the ACTUAL catalog, suppressors MUST be tokens
    // that are never added by any rule's cone. This is the "disjoint
    // suppressor" invariant that makes the CAPCO catalog monotone.
    //
    // Per docs/plans/2026-05-01-lattice-design.md §4.7.3 table-design
    // property: the FD&R dominators set is maintained as a closed set
    // disjoint from all cone tokens, guaranteeing monotonicity by
    // construction. This is what the proptest in proptest_closure.rs
    // verifies for the CAPCO closure catalog.

    // Assert the disjoint suppressor invariant for our test catalog:
    // None of the MONOTONE_RULES' suppressors appear in any rule's cone.
    for rule in MONOTONE_RULES {
        let suppressors: Vec<_> = rule.suppressors.iter().collect();
        for other_rule in MONOTONE_RULES {
            for cone_token in other_rule.cone {
                assert!(
                    !suppressors.contains(&cone_token),
                    "disjoint-suppressor invariant violated: token {:?} appears in both \
                     a suppressor and a cone — this creates a non-monotone catalog",
                    cone_token
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Synthetic non-monotone scheme: catalog rule A→B suppressed by C.
//
// This violates the disjoint-suppressor invariant from §4.7.3 (C is in
// `NON_MONOTONE_RULES`'s suppressor set AND TOK_C can be added by other
// rules in a future catalog — i.e., C is not a "stable dominator"). We
// build it deliberately to exercise the synthetic-catalog ⇒
// observable-violation path per Copilot PR 3.7 review #8.
// ---------------------------------------------------------------------------

static NON_MONOTONE_RULES: &[ClosureRule<NonMonotoneScheme>] = &[ClosureRule {
    name: "stub/a-implies-b-suppressed-by-c",
    display_label: "Non-monotone fixture A implies B suppressed by C",
    label: NON_MONOTONE_CITATION,
    triggers: &[TokenRef::Token(TOK_A)],
    // SUPPRESSOR shape that creates non-monotonicity: C is a token that
    // CAN be present in a marking (it's in our 3-token universe), so a
    // marking with `{A}` fires the rule but a marking with `{A, C}` does
    // not. Adding a fact (C) to a marking REMOVES facts (B) from the
    // closure result — that's the non-monotone violation.
    suppressors: &[TokenRef::Token(TOK_C)],
    cone: &[TokenRef::Token(TOK_B)],
    cone_derived: None,
    default_severity: Severity::Info,
}];

struct NonMonotoneScheme;

impl MarkingScheme for NonMonotoneScheme {
    type Token = TokenId;
    type Marking = BitMarking;
    type ParseError = ();
    type OpenVocabRef = core::convert::Infallible;
    type Parsed<'src> = ();
    type Canonical = ();

    fn name(&self) -> &str {
        "non-monotone-stub"
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
    fn evaluate_custom(
        &self,
        _: &'static str,
        _: &Self::Marking,
        _: marque_scheme::FactBitmask,
    ) -> Vec<ConstraintViolation> {
        Vec::new()
    }
    fn render_canonical(
        &self,
        m: &Self::Marking,
        _: &RenderContext,
        out: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        write!(out, "bits={:08b}", m.bits)
    }
    fn closure_rules(&self) -> &[ClosureRule<Self>] {
        NON_MONOTONE_RULES
    }
    fn iter_present_tokens<'m>(
        &self,
        marking: &'m Self::Marking,
    ) -> Box<dyn Iterator<Item = TokenRef> + 'm> {
        let bits = marking.bits;
        Box::new(
            [TOK_A, TOK_B, TOK_C]
                .into_iter()
                .filter(move |t| bit_index(*t).is_some_and(|n| (bits >> n) & 1 == 1))
                .map(TokenRef::Token),
        )
    }
    fn closure(&self, marking: Self::Marking) -> Self::Marking {
        let mut working = marking;
        for _iter in 0..marque_scheme::closure::MAX_CLOSURE_ITERATIONS {
            let prev = working.bits;
            for rule in NON_MONOTONE_RULES {
                let trigger_fired = rule.triggers.iter().any(|t| self.satisfies(&working, t));
                let suppressor_fired = rule.suppressors.iter().any(|s| self.satisfies(&working, s));
                if trigger_fired && !suppressor_fired {
                    for cone_ref in rule.cone {
                        if let TokenRef::Token(id) = cone_ref {
                            if let Some(n) = bit_index(*id) {
                                working.bits |= 1 << n;
                            }
                        }
                    }
                    if let Some(derived_fn) = rule.cone_derived {
                        for fact_ref in derived_fn(&working) {
                            // `OpenVocabRef = Infallible` makes `Cve` the only
                            // inhabitable variant — irrefutable destructure.
                            let FactRef::Cve(id) = fact_ref;
                            if let Some(n) = bit_index(id) {
                                working.bits |= 1 << n;
                            }
                        }
                    }
                }
            }
            if working.bits == prev {
                return working;
            }
        }
        panic!(
            "non-monotone closure did not converge in {} iterations",
            marque_scheme::closure::MAX_CLOSURE_ITERATIONS
        );
    }
}

/// Observable monotonicity violation: the synthetic `NonMonotoneScheme`
/// (rule `A→B suppressed by C`) violates the monotone property for the
/// specific scenario `m1 = {A}, m2 = {A, C}`. This is the load-bearing
/// test that verifies the negative property end-to-end — the prior
/// `non_monotone_scenario_is_detectable` test only hand-asserted the
/// violation conceptually.
#[test]
fn non_monotone_synthetic_scheme_violates_monotonicity_observably() {
    let scheme = NonMonotoneScheme;

    let m1 = BitMarking::with(0b001); // TOK_A
    let m2 = BitMarking::with(0b101); // TOK_A + TOK_C

    assert!(m1.le(&m2), "test setup: m1 must be ⊑ m2");

    let c1 = scheme.closure(m1);
    let c2 = scheme.closure(m2);

    assert_eq!(c1.bits, 0b011, "closure({{A}}) should be {{A, B}}");
    assert_eq!(
        c2.bits, 0b101,
        "closure({{A, C}}) should be {{A, C}} (rule suppressed)"
    );
    assert!(
        !c1.le(&c2),
        "expected monotonicity violation: closure(m1)={:08b} should NOT be ⊑ closure(m2)={:08b} \
         (TOK_B is in c1 but not c2), but le() returned true",
        c1.bits,
        c2.bits
    );
}

proptest! {
    /// Property-based extension of the hardcoded-pair test above:
    /// the hardcoded check uses one fixed pair of inputs and imports
    /// no proptest strategies, so this generalizes it.
    ///
    /// For random `a` and `c` byte-pattern inputs constructed so
    /// `m1 = a_only` and `m2 = a_only | c_only` always satisfy
    /// `m1 ⊑ m2`, the `NonMonotoneScheme`'s `closure()` impl must
    /// produce a result that demonstrably violates monotonicity
    /// whenever C is in m2 and m1 has no C bit — which is the
    /// failure mode the synthetic rule `A→B suppressed by C`
    /// encodes. Property: for any (a_present, c_present) pair
    /// where C is present in m2 only, closure(m1) ⊄ closure(m2).
    ///
    /// This covers the broader negative-property class without
    /// committing to enumerated bits.
    #[test]
    fn non_monotone_synthetic_scheme_violation_proptest(
        // Construct m1 with bit 0 (TOK_A) set; m2 with bits 0 and 2 (TOK_A + TOK_C).
        // Other bits 1, 3-7 are random for m1, and m2 = m1 | TOK_C-bit.
        m1_extra_bits in any::<u8>().prop_map(|b| b & 0b1111_1010), // exclude TOK_A (bit 0) and TOK_C (bit 2)
    ) {
        let scheme = NonMonotoneScheme;
        let m1 = BitMarking::with(0b001 | m1_extra_bits); // TOK_A + arbitrary
        let m2 = BitMarking::with(m1.bits | 0b100);       // m1 + TOK_C

        prop_assert!(m1.le(&m2), "test setup: m1 = {:08b} must be ⊑ m2 = {:08b}", m1.bits, m2.bits);

        let c1 = scheme.closure(m1.clone());
        let c2 = scheme.closure(m2.clone());

        // m1 doesn't carry TOK_C, so the rule fires and adds TOK_B (bit 1).
        // m2 carries TOK_C, so the rule is suppressed.
        prop_assert!(
            c1.has_token(1),
            "non-monotone rule should fire on m1 (no TOK_C suppressor): m1={:08b}, c1={:08b}",
            m1.bits, c1.bits
        );
        // c2's bit 1 is only set if m1 already had it (bit 1 in m1_extra_bits).
        // If m1 didn't carry TOK_B, c2 won't have TOK_B either (rule suppressed).
        if !m1.has_token(1) {
            prop_assert!(
                !c2.has_token(1),
                "non-monotone rule should NOT fire on m2 (TOK_C suppressor present): m2={:08b}, c2={:08b}",
                m2.bits, c2.bits
            );
            // The monotonicity violation: c1 has TOK_B but c2 doesn't.
            prop_assert!(
                !c1.le(&c2),
                "expected monotonicity violation: c1={:08b} should NOT be ⊑ c2={:08b}",
                c1.bits, c2.bits
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Derived-cone fixtures — exercise `ClosureRule::cone_derived`.
//
// These two schemes mirror `MonotoneScheme` and `NonMonotoneScheme` above but
// route their cone facts through the `cone_derived` callback shape. They are
// load-bearing: without them, the JOINT row (the first production-side
// consumer of `cone_derived`) could ship with a silent monotonicity defect
// because no test would exercise the derived path.
//
// MonotoneDerivedScheme:
//   Rule has a `cone_derived` fn that returns one fact per bit set in the
//   marking (bit 0 → TOK_A, bit 1 → TOK_B, bit 2 → TOK_C). Adding bits to
//   `m` only adds facts to `derived(m)` — strictly monotone.
//
// NonMonotoneDerivedScheme:
//   Rule has a `cone_derived` fn that emits DISJOINT facts on complementary
//   bit-0/bit-1 inputs:
//     - bit 0 set AND bit 1 unset → emit TOK_C (bit 2)
//     - bit 0 set AND bit 1 set   → emit nothing
//   For m1 = 0b001 ⊑ m2 = 0b011, closure(m1) acquires TOK_C while closure(m2)
//   does not — closure(m1).le(&closure(m2)) returns false through the
//   operator, surfacing the violation independent of the bit-OR join.
//
//   This mirrors the static `NonMonotoneScheme` pattern (rule fires on m1
//   but not m2, cone fact ends up in c1 but not c2) translated onto the
//   `cone_derived` callback.
// ---------------------------------------------------------------------------

use smallvec::{SmallVec, smallvec};

const CAT_X: marque_scheme::category::CategoryId = marque_scheme::category::CategoryId(0);

fn monotone_derived_cone(m: &BitMarking) -> SmallVec<[FactRef<MonotoneDerivedScheme>; 2]> {
    let mut out: SmallVec<[FactRef<MonotoneDerivedScheme>; 2]> = SmallVec::new();
    if m.has_token(0) {
        out.push(FactRef::Cve(TOK_A));
    }
    if m.has_token(1) {
        out.push(FactRef::Cve(TOK_B));
    }
    if m.has_token(2) {
        out.push(FactRef::Cve(TOK_C));
    }
    out
}

fn non_monotone_derived_cone(m: &BitMarking) -> SmallVec<[FactRef<NonMonotoneDerivedScheme>; 2]> {
    // Two complementary trigger predicates emit DISJOINT cone facts:
    //   - bit 0 set AND bit 1 unset → emit TOK_C
    //   - bit 0 set AND bit 1 set   → emit nothing
    //
    // For m1 = 0b001 ⊑ m2 = 0b011, the first branch fires on m1 only and
    // adds TOK_C (bit 2). m2 carries TOK_B (bit 1), which is NOT in the
    // cone, so the closure operator never sets bit 2 for m2. The fact
    // emitted under m1 is therefore absent in closure(m2) — the violation
    // surfaces through the operator regardless of the bit-OR join.
    if m.has_token(0) && !m.has_token(1) {
        smallvec![FactRef::Cve(TOK_C)]
    } else {
        SmallVec::new()
    }
}

static MONOTONE_DERIVED_RULES: &[ClosureRule<MonotoneDerivedScheme>] = &[ClosureRule {
    name: "stub/derived-monotone",
    display_label: "Monotone derived-cone fixture",
    label: NON_MONOTONE_CITATION,
    // Unconditional firing — the cone_derived fn does the marking-shape work.
    triggers: &[],
    suppressors: &[],
    cone: &[],
    cone_derived: Some(monotone_derived_cone),
    default_severity: Severity::Info,
}];

static NON_MONOTONE_DERIVED_RULES: &[ClosureRule<NonMonotoneDerivedScheme>] = &[ClosureRule {
    name: "stub/derived-non-monotone",
    display_label: "Non-monotone derived-cone fixture",
    label: NON_MONOTONE_CITATION,
    triggers: &[],
    suppressors: &[],
    cone: &[],
    cone_derived: Some(non_monotone_derived_cone),
    default_severity: Severity::Info,
}];

struct MonotoneDerivedScheme;

#[cfg_attr(coverage_nightly, coverage(off))]
impl MarkingScheme for MonotoneDerivedScheme {
    type Token = TokenId;
    type Marking = BitMarking;
    type ParseError = ();
    type OpenVocabRef = core::convert::Infallible;
    type Parsed<'src> = ();
    type Canonical = ();

    fn name(&self) -> &str {
        "monotone-derived-stub"
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
    fn evaluate_custom(
        &self,
        _: &'static str,
        _: &Self::Marking,
        _: marque_scheme::FactBitmask,
    ) -> Vec<ConstraintViolation> {
        Vec::new()
    }
    fn render_canonical(
        &self,
        m: &Self::Marking,
        _: &RenderContext,
        out: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        write!(out, "bits={:08b}", m.bits)
    }
    fn closure_rules(&self) -> &[ClosureRule<Self>] {
        MONOTONE_DERIVED_RULES
    }
    fn iter_present_tokens<'m>(
        &self,
        marking: &'m Self::Marking,
    ) -> Box<dyn Iterator<Item = TokenRef> + 'm> {
        let bits = marking.bits;
        Box::new(
            [TOK_A, TOK_B, TOK_C]
                .into_iter()
                .filter(move |t| bit_index(*t).is_some_and(|n| (bits >> n) & 1 == 1))
                .map(TokenRef::Token),
        )
    }
    fn closure(&self, marking: Self::Marking) -> Self::Marking {
        let mut working = marking;
        for _iter in 0..marque_scheme::closure::MAX_CLOSURE_ITERATIONS {
            let prev = working.bits;
            for rule in MONOTONE_DERIVED_RULES {
                if rule.should_fire(self, &working) {
                    for id in rule.cone_token_ids() {
                        if let Some(n) = bit_index(id) {
                            working.bits |= 1 << n;
                        }
                    }
                    if let Some(derived_fn) = rule.cone_derived {
                        for fact_ref in derived_fn(&working) {
                            // `OpenVocabRef = Infallible` makes `Cve` the only
                            // inhabitable variant — irrefutable destructure.
                            let FactRef::Cve(id) = fact_ref;
                            if let Some(n) = bit_index(id) {
                                working.bits |= 1 << n;
                            }
                        }
                    }
                }
            }
            if working.bits == prev {
                return working;
            }
        }
        panic!(
            "monotone-derived closure did not converge in {} iterations",
            marque_scheme::closure::MAX_CLOSURE_ITERATIONS
        );
    }

    fn token_category(&self, id: TokenId) -> Option<marque_scheme::category::CategoryId> {
        bit_index(id).map(|_| CAT_X)
    }
}

struct NonMonotoneDerivedScheme;

#[cfg_attr(coverage_nightly, coverage(off))]
impl MarkingScheme for NonMonotoneDerivedScheme {
    type Token = TokenId;
    type Marking = BitMarking;
    type ParseError = ();
    type OpenVocabRef = core::convert::Infallible;
    type Parsed<'src> = ();
    type Canonical = ();

    fn name(&self) -> &str {
        "non-monotone-derived-stub"
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
    fn evaluate_custom(
        &self,
        _: &'static str,
        _: &Self::Marking,
        _: marque_scheme::FactBitmask,
    ) -> Vec<ConstraintViolation> {
        Vec::new()
    }
    fn render_canonical(
        &self,
        m: &Self::Marking,
        _: &RenderContext,
        out: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        write!(out, "bits={:08b}", m.bits)
    }
    fn closure_rules(&self) -> &[ClosureRule<Self>] {
        NON_MONOTONE_DERIVED_RULES
    }
    fn iter_present_tokens<'m>(
        &self,
        marking: &'m Self::Marking,
    ) -> Box<dyn Iterator<Item = TokenRef> + 'm> {
        let bits = marking.bits;
        Box::new(
            [TOK_A, TOK_B, TOK_C]
                .into_iter()
                .filter(move |t| bit_index(*t).is_some_and(|n| (bits >> n) & 1 == 1))
                .map(TokenRef::Token),
        )
    }
    fn closure(&self, marking: Self::Marking) -> Self::Marking {
        let mut working = marking;
        for _iter in 0..marque_scheme::closure::MAX_CLOSURE_ITERATIONS {
            let prev = working.bits;
            for rule in NON_MONOTONE_DERIVED_RULES {
                if rule.should_fire(self, &working) {
                    for id in rule.cone_token_ids() {
                        if let Some(n) = bit_index(id) {
                            working.bits |= 1 << n;
                        }
                    }
                    if let Some(derived_fn) = rule.cone_derived {
                        for fact_ref in derived_fn(&working) {
                            // `OpenVocabRef = Infallible` makes `Cve` the only
                            // inhabitable variant — irrefutable destructure.
                            let FactRef::Cve(id) = fact_ref;
                            if let Some(n) = bit_index(id) {
                                working.bits |= 1 << n;
                            }
                        }
                    }
                }
            }
            if working.bits == prev {
                return working;
            }
        }
        panic!(
            "non-monotone-derived closure did not converge in {} iterations",
            marque_scheme::closure::MAX_CLOSURE_ITERATIONS
        );
    }

    fn token_category(&self, id: TokenId) -> Option<marque_scheme::category::CategoryId> {
        bit_index(id).map(|_| CAT_X)
    }
}

proptest! {
    /// Derived-cone monotone catalog satisfies monotonicity through the operator.
    ///
    /// Mirrors `closure_is_monotone` in `proptest_closure.rs` but exercises the
    /// `cone_derived` path. The `monotone_derived_cone` fn returns one fact per
    /// set bit, so closure(m) ⊇ m for all m and the inclusion is monotone in m.
    /// Run the operator on m1 ⊑ m2 and assert closure(m1) ⊑ closure(m2).
    #[test]
    fn monotone_derived_scheme_satisfies_proptest(bits1 in any::<u8>(), bits2 in any::<u8>()) {
        let scheme = MonotoneDerivedScheme;
        // Restrict to the 3-bit universe (TOK_A, TOK_B, TOK_C).
        let m1 = BitMarking::with((bits1 & bits2) & 0b111);
        let m2 = BitMarking::with(bits2 & 0b111);

        prop_assert!(m1.le(&m2), "test setup: m1 = {:08b} must be ⊑ m2 = {:08b}", m1.bits, m2.bits);

        let c1 = scheme.closure(m1);
        let c2 = scheme.closure(m2);

        prop_assert!(
            c1.le(&c2),
            "monotone-derived monotonicity violation: closure({:08b}) = {:08b}, \
             closure({:08b}) = {:08b}, but le() returned false",
            (bits1 & bits2) & 0b111, c1.bits,
            bits2 & 0b111, c2.bits
        );
    }
}

proptest! {
    /// Derived-cone non-monotone catalog produces an observable violation
    /// through the `closure()` operator.
    ///
    /// Mirrors `non_monotone_synthetic_scheme_violation_proptest` above but
    /// exercises the `cone_derived` path. The `non_monotone_derived_cone` fn
    /// emits TOK_C when TOK_A is present AND TOK_B is absent, and emits
    /// nothing when both TOK_A and TOK_B are present — two complementary
    /// trigger predicates with DISJOINT cones.
    ///
    /// For m1 ⊑ m2 with m1 carrying TOK_A only and m2 carrying TOK_A + TOK_B,
    /// the rule fires on m1 (adding TOK_C to the closure) and is suppressed
    /// on m2 (TOK_C never enters the closure). Because TOK_C is not in m2's
    /// input either, `closure(m1)` carries a token `closure(m2)` lacks and
    /// `closure(m1).le(&closure(m2))` returns false — the violation surfaces
    /// through the operator, independent of the bit-OR join.
    ///
    /// **For a JOINT row**: this proptest already exercises the
    /// monotonicity violation through `closure()`, but on a bit-OR lattice.
    /// `JointSet` exposes a structurally different family of violations —
    /// `UnanimousProducers{A} ⊔ UnanimousProducers{B}` collapses to
    /// `DisunityCollapse{union_non_us}`, a *different variant* that strips
    /// USA. A JOINT `cone_derived` proptest must compose `JointSet`
    /// directly so the variant transmutation actually happens during the
    /// join (it cannot be modeled by a bit-OR fixture). The `JointSet`
    /// hazard note on `ClosureRule::cone_derived` in
    /// `crates/scheme/src/closure.rs` is the load-bearing pointer.
    #[test]
    fn non_monotone_derived_scheme_violation_proptest(
        // Random high bits (3..=7) for m1 — bit 0 (TOK_A) is forced set,
        // bit 1 (TOK_B) is forced unset, bit 2 (TOK_C) is forced unset so
        // the derived cone fact is observably absent from m2's closure.
        m1_high_bits in any::<u8>().prop_map(|b| b & 0b1111_1000),
    ) {
        let scheme = NonMonotoneDerivedScheme;
        // m1: bit 0 set, bit 1 unset, bit 2 unset, high bits arbitrary.
        let m1 = BitMarking::with(0b001 | m1_high_bits);
        // m2: m1 | TOK_B (bit 1). Still no TOK_C (bit 2) in the input.
        let m2 = BitMarking::with(m1.bits | 0b010);

        prop_assert!(m1.le(&m2), "test setup: m1 = {:08b} must be ⊑ m2 = {:08b}", m1.bits, m2.bits);
        // Sanity: TOK_C must be absent from both inputs, otherwise the
        // closure can't distinguish the rule's contribution from the input.
        prop_assert!(!m1.has_token(2), "test setup: m1 must not carry TOK_C: m1 = {:08b}", m1.bits);
        prop_assert!(!m2.has_token(2), "test setup: m2 must not carry TOK_C: m2 = {:08b}", m2.bits);

        let c1 = scheme.closure(m1.clone());
        let c2 = scheme.closure(m2.clone());

        // closure(m1): predicate "bit 0 set AND bit 1 unset" matches → emits
        //   TOK_C → c1 acquires bit 2. Fixpoint at m1 | TOK_C.
        prop_assert!(
            c1.has_token(2),
            "non-monotone-derived rule should fire on m1 (TOK_A set, TOK_B absent): \
             m1={:08b}, c1={:08b}",
            m1.bits, c1.bits
        );
        // closure(m2): predicate fails immediately (bit 1 set) → emits nothing
        //   → c2 == m2. TOK_C remains absent.
        prop_assert!(
            !c2.has_token(2),
            "non-monotone-derived rule should NOT fire on m2 (TOK_B suppresses the predicate): \
             m2={:08b}, c2={:08b}",
            m2.bits, c2.bits
        );

        // Operator-level monotonicity violation: closure(m1) carries TOK_C,
        // closure(m2) does not, so closure(m1) ⊄ closure(m2) for m1 ⊑ m2.
        prop_assert!(
            !c1.le(&c2),
            "non-monotone-derived violation: closure(m1) = {:08b} should NOT be ⊑ closure(m2) = {:08b} \
             (TOK_C is in c1 but not c2) for m1 = {:08b} ⊑ m2 = {:08b}",
            c1.bits, c2.bits, m1.bits, m2.bits
        );
    }
}
