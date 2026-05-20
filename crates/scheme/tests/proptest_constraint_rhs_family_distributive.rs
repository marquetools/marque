// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Property test: distributive expansion of `Constraint::ConflictsWithFamily`.
//!
//! Per `docs/plans/2026-05-13-pr3.7-lattice-resolution-gate-plan.md` §2
//! finding M3 (lattice-preflight):
//!
//! The family-predicate form is **distributively equivalent** to one
//! `Constraint::Conflicts` row per token in the marking that matches the
//! predicate:
//!
//! ```text
//! ∀ LHS, ∀ FamilyPredicate p, ∀ marking m:
//!   emit(ConflictsWithFamily(LHS, p)) =
//!     union_{t ∈ present_tokens(m), p(t)} emit(Conflicts(LHS, Token(t)))
//! ```
//!
//! This test verifies that:
//! 1. The `ConflictsWithFamily` evaluator produces violations exactly for
//!    the tokens present in the marking that match the predicate (and LHS
//!    is also present).
//! 2. The violation set equals the union of individual `Conflicts` rows
//!    for each matching token.
//! 3. The number of violations matches the count of matching tokens.
//!
//! ## Scheme setup
//!
//! `FamilyStubScheme` uses a bitset marking (8 bits) and a family predicate
//! `is_odd_indexed` that matches ODD-indexed tokens (bits 1, 3, 5, 7).
//! The LHS is TOK_LHS (bit 0). When TOK_LHS is present and any
//! odd-indexed token is present, the `ConflictsWithFamily` row fires once
//! per such token.

use marque_scheme::{
    Category, Citation, Constraint, ConstraintViolation, FamilyPredicate, JoinSemilattice,
    MarkingScheme, MeetSemilattice, PageRewrite, Parsed, Scope, SectionLetter, Template, TokenId,
    TokenRef, constraint::evaluate,
};
use proptest::prelude::*;

// Sentinel test citation — distributive-expansion proptest fixture; the
// `AuthoritativeSource::EngineInternal` source renders as `[engine-internal]`
// so the value carries no false CAPCO §-claim.
const PROPTEST_CITATION: Citation = Citation::new(
    marque_scheme::AuthoritativeSource::EngineInternal,
    marque_scheme::SectionRef::new(SectionLetter::A),
    match core::num::NonZeroU16::new(1) {
        Some(n) => n,
        None => unreachable!(),
    },
);

// ---------------------------------------------------------------------------
// Bitset marking.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct BitMarking {
    bits: u8,
}

impl BitMarking {
    const fn with(bits: u8) -> Self {
        Self { bits }
    }

    const fn has_bit(&self, n: u8) -> bool {
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

// ---------------------------------------------------------------------------
// Token IDs — bits 0..7.
// ---------------------------------------------------------------------------

const TOK: [TokenId; 8] = [
    TokenId(0),
    TokenId(1),
    TokenId(2),
    TokenId(3),
    TokenId(4),
    TokenId(5),
    TokenId(6),
    TokenId(7),
];

fn tok_bit(id: TokenId) -> Option<u8> {
    TOK.iter().position(|t| *t == id).map(|i| i as u8)
}

// LHS token is TOK[0] (bit 0).
const LHS_TOKEN: TokenId = TOK[0];
const LHS_BIT: u8 = 0;

// Family predicate: matches ODD-indexed tokens (bits 1, 3, 5, 7) that
// are NOT the LHS. This ensures LHS itself is never matched by the predicate,
// keeping the test semantics clean.
fn is_odd_indexed(token_ref: &TokenRef) -> bool {
    match token_ref {
        TokenRef::Token(id) => {
            let idx = TOK.iter().position(|t| *t == *id);
            // Odd index and not the LHS (index > 0 and odd)
            idx.is_some_and(|i| i % 2 == 1)
        }
        TokenRef::AnyInCategory(_) => false,
    }
}

// ---------------------------------------------------------------------------
// Family scheme: two constraints tested in parallel.
//
// FAMILY constraint: ConflictsWithFamily(LHS, is_odd_indexed)
// ENUMERATED constraints: one Conflicts(LHS, Token(t)) per odd-indexed token.
// Both should produce the same violation set.
// ---------------------------------------------------------------------------

struct FamilyScheme {
    use_family: bool,
}

impl FamilyScheme {
    fn family() -> Self {
        Self { use_family: true }
    }
    fn enumerated() -> Self {
        Self { use_family: false }
    }
}

// Static constraints for the family form.
static FAMILY_CONSTRAINTS: &[Constraint] = &[Constraint::ConflictsWithFamily {
    name: "test/lhs-conflicts-odd-family",
    left: TokenRef::Token(LHS_TOKEN),
    family: FamilyPredicate(is_odd_indexed),
    label: PROPTEST_CITATION,
    severity: None,
}];

// Static constraints for the enumerated form — one per odd-indexed token.
static ENUMERATED_CONSTRAINTS: &[Constraint] = &[
    Constraint::Conflicts {
        name: "test/lhs-conflicts-tok1",
        left: TokenRef::Token(LHS_TOKEN),
        right: TokenRef::Token(TOK[1]),
        label: PROPTEST_CITATION,
        severity: None,
        span_anchor: None,
    },
    Constraint::Conflicts {
        name: "test/lhs-conflicts-tok3",
        left: TokenRef::Token(LHS_TOKEN),
        right: TokenRef::Token(TOK[3]),
        label: PROPTEST_CITATION,
        severity: None,
        span_anchor: None,
    },
    Constraint::Conflicts {
        name: "test/lhs-conflicts-tok5",
        left: TokenRef::Token(LHS_TOKEN),
        right: TokenRef::Token(TOK[5]),
        label: PROPTEST_CITATION,
        severity: None,
        span_anchor: None,
    },
    Constraint::Conflicts {
        name: "test/lhs-conflicts-tok7",
        left: TokenRef::Token(LHS_TOKEN),
        right: TokenRef::Token(TOK[7]),
        label: PROPTEST_CITATION,
        severity: None,
        span_anchor: None,
    },
];

impl MarkingScheme for FamilyScheme {
    type Token = TokenId;
    type Marking = BitMarking;
    type ParseError = ();
    type OpenVocabRef = core::convert::Infallible;
    type Parsed<'src> = ();
    type Canonical = ();

    fn name(&self) -> &str {
        if self.use_family {
            "family-stub"
        } else {
            "enum-stub"
        }
    }
    fn schema_version(&self) -> &str {
        "v0"
    }
    fn categories(&self) -> &[Category] {
        &[]
    }
    fn constraints(&self) -> &[Constraint] {
        if self.use_family {
            FAMILY_CONSTRAINTS
        } else {
            ENUMERATED_CONSTRAINTS
        }
    }
    fn templates(&self) -> &[Template] {
        &[]
    }
    fn parse(&self, _: &str) -> Result<Parsed<Self::Marking>, Self::ParseError> {
        Err(())
    }
    fn satisfies(&self, marking: &Self::Marking, token_ref: &TokenRef) -> bool {
        match token_ref {
            TokenRef::Token(id) => tok_bit(*id).is_some_and(|n| marking.has_bit(n)),
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
        _: &marque_scheme::RenderContext,
        out: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        write!(out, "bits={:08b}", m.bits)
    }

    fn iter_present_tokens<'m>(
        &self,
        marking: &'m Self::Marking,
    ) -> Box<dyn Iterator<Item = TokenRef> + 'm> {
        let bits = marking.bits;
        Box::new(
            TOK.iter()
                .filter(move |t| tok_bit(**t).is_some_and(|n| (bits >> n) & 1 == 1))
                .copied()
                .map(TokenRef::Token),
        )
    }
}

// ---------------------------------------------------------------------------
// Helper: count expected violations manually.
//
// Expected: LHS present AND each odd-indexed token present → one violation.
// ---------------------------------------------------------------------------

fn expected_violation_count(bits: u8) -> usize {
    // LHS must be present.
    if (bits >> LHS_BIT) & 1 == 0 {
        return 0;
    }
    // Count odd-indexed bits (bits 1, 3, 5, 7) that are set.
    [1u8, 3, 5, 7]
        .iter()
        .filter(|&&n| (bits >> n) & 1 == 1)
        .count()
}

// ---------------------------------------------------------------------------
// Property: distributive expansion.
//   ∀ marking m:
//     |violations from ConflictsWithFamily| == |violations from enumerated Conflicts|
//   AND the violation count equals the number of matching present tokens.
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn family_form_equals_enumerated_form(bits in any::<u8>()) {
        let marking = BitMarking::with(bits);

        let family_scheme = FamilyScheme::family();
        let enum_scheme = FamilyScheme::enumerated();

        let family_violations = evaluate(&family_scheme, &marking);
        let enum_violations = evaluate(&enum_scheme, &marking);

        let expected = expected_violation_count(bits);

        prop_assert_eq!(
            family_violations.len(),
            enum_violations.len(),
            "marking bits={:08b}: family form emitted {} violations but \
             enumerated form emitted {}",
            bits, family_violations.len(), enum_violations.len()
        );

        prop_assert_eq!(
            family_violations.len(),
            expected,
            "marking bits={:08b}: expected {} violations (LHS present={}, \
             odd-indexed-bits={:?}), got {}",
            bits,
            expected,
            (bits >> LHS_BIT) & 1 == 1,
            [1u8, 3, 5, 7].iter().filter(|&&n| (bits >> n) & 1 == 1).collect::<Vec<_>>(),
            family_violations.len()
        );

        // IDENTITY check (Copilot PR 3.7 review #9): compare the
        // normalized violation IDENTITIES, not just counts. A broken
        // ConflictsWithFamily impl could emit the right count for the
        // wrong matching tokens (e.g., always emit one violation
        // matching token-index 1 regardless of which odd-indexed bits
        // are actually present). The identity check normalizes each
        // violation to its message string (which encodes the matching
        // RHS token's Debug shape — `TokenRef::Token(TokenId(N))`)
        // and asserts the same sorted multiset on both sides.
        let normalize = |vs: &[marque_scheme::ConstraintViolation]| -> Vec<String> {
            let mut out: Vec<String> = vs
                .iter()
                .map(|v| format!("{}|{}", v.constraint_label, v.message))
                .collect();
            out.sort();
            out
        };
        // The enumerated form's messages are `"conflicting tokens: <LHS> and <RHS>"`
        // (per `marque_scheme::constraint::evaluate`'s `Conflicts` arm); the family
        // form's messages have a trailing `" (family match)"` suffix to mark the
        // row source. The matched (LHS, RHS) pair text is verbatim in both;
        // stripping the suffix from family messages aligns them with enumerated
        // messages for an apples-to-apples multiset comparison. Same multiset →
        // same set of matching (LHS, RHS) pairs → distributive equivalence.
        let strip_family_suffix = |s: &str| -> String {
            s.strip_suffix(" (family match)").map(str::to_owned).unwrap_or_else(|| s.to_owned())
        };
        let mut family_pairs: Vec<String> = family_violations
            .iter()
            .map(|v| strip_family_suffix(&v.message))
            .collect();
        let mut enum_pairs: Vec<String> = enum_violations.iter().map(|v| v.message.clone()).collect();
        family_pairs.sort();
        enum_pairs.sort();
        prop_assert_eq!(
            family_pairs.clone(),
            enum_pairs.clone(),
            "marking bits={:08b}: family form's matched-token set differs from enumerated form's. \
             family (suffix-stripped)={:?} enumerated={:?}",
            bits, family_pairs, enum_pairs
        );
        // Silence unused-variable warning from the normalize closure
        // (kept for future identity-check variants that include the
        // constraint_label in the comparison).
        let _ = normalize(&family_violations);
    }
}

// ---------------------------------------------------------------------------
// Property: violations are correctly labeled.
//   Every family-form violation must carry the family constraint's name
//   and the citation verbatim.
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn family_violations_carry_correct_identifiers(bits in any::<u8>()) {
        let marking = BitMarking::with(bits);
        let scheme = FamilyScheme::family();
        let violations = evaluate(&scheme, &marking);

        for v in &violations {
            prop_assert_eq!(
                v.constraint_label,
                "test/lhs-conflicts-odd-family",
                "family violation must carry the catalog row's name"
            );
            prop_assert_eq!(
                v.citation,
                PROPTEST_CITATION,
                "family violation must carry the catalog row's label"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Property: no-LHS means no violations (regardless of RHS tokens).
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn no_violations_when_lhs_absent(
        // bits with LHS bit (bit 0) CLEARED — force absence of LHS.
        bits in any::<u8>().prop_map(|b| b & !1u8),
    ) {
        let marking = BitMarking::with(bits);
        let scheme = FamilyScheme::family();
        let violations = evaluate(&scheme, &marking);

        prop_assert!(
            violations.is_empty(),
            "ConflictsWithFamily must not fire when LHS (bit 0) is absent; \
             bits={:08b}, got {} violations",
            bits, violations.len()
        );
    }
}

// ---------------------------------------------------------------------------
// Property: advisory-only (span=None, severity=None).
//   ConflictsWithFamily violations MUST emit with None span and None severity,
//   matching the dyadic Conflicts arm behavior.
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn family_violations_are_advisory_only(bits in any::<u8>()) {
        let marking = BitMarking::with(bits);
        let scheme = FamilyScheme::family();
        let violations = evaluate(&scheme, &marking);

        for v in &violations {
            prop_assert!(
                v.span.is_none(),
                "ConflictsWithFamily violations must emit None span (advisory-only)"
            );
            prop_assert!(
                v.severity.is_none(),
                "ConflictsWithFamily violations must emit None severity (advisory-only)"
            );
        }
    }
}
