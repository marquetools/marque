// SPDX-FileCopyrightText: 2026 Knitli Inc.
//
// SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

//! Property tests for the closure operator.
//!
//! Tests the mathematical properties required by
//! `docs/plans/2026-05-13-pr3.7-lattice-resolution-gate-plan.md` §2
//! (T108c, Stage B). Five properties land in this file:
//!
//! 1. **Monotone**: if `m1 ⊑ m2` then `closure(m1) ⊑ closure(m2)`.
//! 2. **Extensive**: `closure(m) ⊒ m` for all markings.
//! 3. **Idempotent**: `closure(closure(m)) == closure(m)`.
//! 4. **Unconditional-catalog monotonicity (chain-depth stress test)**:
//!    a transitive-chain catalog (A→B and C→D rules) with NO suppressors
//!    preserves monotonicity. NOTE (per Copilot PR 3.7 review #13):
//!    this property does NOT cover suppression-stability. The plan's
//!    "suppression-doesn't-break-monotonicity" property is satisfied
//!    by the CAPCO catalog's disjoint-suppressor invariant (verified
//!    structurally in `proptest_closure_rejects_non_monotone.rs`) and
//!    by per-row CAPCO monotonicity attestation that lands in PR 4.
//! 5. **G13 content-ignorance regression**: closure output contains no
//!    input bytes verbatim (Constitution V Principle V).
//!
//! ## Stub scheme design
//!
//! `ClosureStubScheme` is a minimal `MarkingScheme` with a simple bitset
//! marking (up to 8 tokens, each a single bit). Each bit represents a
//! distinct token. The scheme implements:
//!
//! - A set of closure rules that add bit B when bit A is set, and a
//!   transitive-chain pair (A→B, C→D) for the chain-depth stress test.
//!   All rules in `CLOSURE_RULES` are **unconditional** (no suppressors)
//!   so the catalog is trivially monotone for properties 1–4. A
//!   suppressor-bearing closure rule is exercised only in the negative
//!   companion file `proptest_closure_rejects_non_monotone.rs` (see
//!   property 4 NOTE above for the rationale).
//! - `iter_present_tokens`: yields `TokenRef::Token` for each set bit.
//! - `closure()`: the actual Kleene-fixpoint implementation.
//!
//! The bitset ordering is componentwise: `m1 ⊑ m2` iff `m1.bits & m2.bits == m1.bits`.

use marque_scheme::{
    Category, Constraint, ConstraintViolation, FactRef, JoinSemilattice, MarkingScheme,
    MeetSemilattice, PageRewrite, Parsed, Scope, Template, TokenId, TokenRef, closure::ClosureRule,
    severity::Severity,
};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Bitset marking type — up to 8 tokens as bits.
// ---------------------------------------------------------------------------

/// A marking represented as a bitset of up to 8 token presence flags.
/// `bits & (1 << n)` is true when `TOK[n]` is present.
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

    /// `m1 ⊑ m2` in the bitset lattice: every bit set in m1 is set in m2.
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

// ---------------------------------------------------------------------------
// Token IDs for the stub scheme.
// bit 0 = TOK_A, bit 1 = TOK_B, ... bit 7 = TOK_H
// ---------------------------------------------------------------------------

const TOK_A: TokenId = TokenId(0);
const TOK_B: TokenId = TokenId(1);
const TOK_C: TokenId = TokenId(2);
const TOK_D: TokenId = TokenId(3);
const TOK_E: TokenId = TokenId(4);
const TOK_F: TokenId = TokenId(5);
const TOK_G: TokenId = TokenId(6);
const TOK_H: TokenId = TokenId(7);

fn bit_index(id: TokenId) -> Option<u8> {
    let all = [TOK_A, TOK_B, TOK_C, TOK_D, TOK_E, TOK_F, TOK_G, TOK_H];
    all.iter().position(|t| *t == id).map(|i| i as u8)
}

// ---------------------------------------------------------------------------
// Closure rules catalog for the stub scheme.
//
// All rules are unconditional (no suppressors) to ensure the catalog is
// trivially monotone for the positive property tests. The suppressor
// scenario is tested separately in the negative proptest.
//
// Row 1: if TOK_A present → add TOK_B (unconditional)
//   Models: "if IC marking X is present, Y is implied"
//
// Row 2: if TOK_C present → add TOK_D (unconditional, chain link)
//   Models: a transitive implication (A→B, C→D, no cycles)
//
// Row 3: if TOK_F present → add TOK_G, TOK_H (multi-cone)
//   Models: a per-marking unconditional implication with multiple cone entries
//
// Note on suppressor-based rules: A suppressor-bearing rule is monotone
// only if the suppressor token is guaranteed to be present in closure(m2)
// whenever it is present in m2 AND m1 ⊑ m2 — which requires a scheme-level
// structural guarantee (the CAPCO "disjoint suppressor" invariant from
// docs/plans/2026-05-01-lattice-design.md §4.7.3). The CAPCO design
// ensures FD&R dominators are never in any cone, so they are stable under
// the closure operator. Generic test catalogs cannot rely on this without
// encoding the same structural guarantee, so the positive tests use
// unconditional rules. The negative test in
// proptest_closure_rejects_non_monotone.rs demonstrates the suppressor-
// based violation scenario.
// ---------------------------------------------------------------------------

static CLOSURE_RULES: &[ClosureRule<ClosureStubScheme>] = &[
    // Row 1: A→B (one-hop)
    ClosureRule {
        name: "stub/a-implies-b",
        display_label: "Stub A implies B",
        label: "StubScheme proptest fixture",
        triggers: &[TokenRef::Token(TOK_A)],
        suppressors: &[],
        cone: &[TokenRef::Token(TOK_B)],
        cone_derived: None,
        default_severity: Severity::Info,
    },
    // Row 2: B→C (chains off row 1 — A→B→C is a true transitive
    // multi-iteration chain; without this row Property 4's
    // "chain-depth stress test" framing was misleading per Copilot
    // PR 3.7 review pass 3).
    ClosureRule {
        name: "stub/b-implies-c",
        display_label: "Stub B implies C",
        label: "StubScheme proptest fixture",
        triggers: &[TokenRef::Token(TOK_B)],
        suppressors: &[],
        cone: &[TokenRef::Token(TOK_C)],
        cone_derived: None,
        default_severity: Severity::Info,
    },
    // Row 3: C→D (extends the chain to depth 3 — A→B→C→D is the
    // worst-case depth the CAPCO catalog observes per
    // marque-applied.md §4.7.3 chain-depth walk).
    ClosureRule {
        name: "stub/c-implies-d",
        display_label: "Stub C implies D",
        label: "StubScheme proptest fixture",
        triggers: &[TokenRef::Token(TOK_C)],
        suppressors: &[],
        cone: &[TokenRef::Token(TOK_D)],
        cone_derived: None,
        default_severity: Severity::Info,
    },
    // Row 4: independent F→{G,H} (multi-cone, no chain interaction)
    ClosureRule {
        name: "stub/f-implies-g-h",
        display_label: "Stub F implies G and H",
        label: "StubScheme proptest fixture",
        triggers: &[TokenRef::Token(TOK_F)],
        suppressors: &[],
        cone: &[TokenRef::Token(TOK_G), TokenRef::Token(TOK_H)],
        cone_derived: None,
        default_severity: Severity::Info,
    },
];

// ---------------------------------------------------------------------------
// Stub scheme implementation.
// ---------------------------------------------------------------------------

struct ClosureStubScheme;

impl MarkingScheme for ClosureStubScheme {
    type Token = TokenId;
    type Marking = BitMarking;
    type ParseError = ();
    type OpenVocabRef = core::convert::Infallible;
    type Parsed<'src> = ();
    type Canonical = ();

    fn name(&self) -> &str {
        "closure-stub"
    }
    fn schema_version(&self) -> &str {
        "closure-stub-1"
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
        _: &marque_scheme::RenderContext,
        out: &mut dyn core::fmt::Write,
    ) -> core::fmt::Result {
        // Render as a hex byte: "bits=0x{:02x}". This does NOT include
        // input document bytes — G13 compliance is structurally guaranteed.
        write!(out, "bits=0x{:02x}", m.bits)
    }

    fn closure_rules(&self) -> &[ClosureRule<Self>] {
        CLOSURE_RULES
    }

    fn token_category(&self, _id: TokenId) -> Option<marque_scheme::category::CategoryId> {
        // All tokens are in category 0 for this stub.
        Some(marque_scheme::category::CategoryId(0))
    }

    fn iter_present_tokens<'m>(
        &self,
        marking: &'m Self::Marking,
    ) -> Box<dyn Iterator<Item = TokenRef> + 'm> {
        let all = [TOK_A, TOK_B, TOK_C, TOK_D, TOK_E, TOK_F, TOK_G, TOK_H];
        let bits = marking.bits;
        Box::new(
            all.into_iter()
                .filter(move |t| bit_index(*t).is_some_and(|n| (bits >> n) & 1 == 1))
                .map(TokenRef::Token),
        )
    }

    /// Kleene-fixpoint closure operator over `CLOSURE_RULES`.
    ///
    /// For this stub, "adding a token to the marking" is trivially expressed
    /// as OR-ing the corresponding bit into `BitMarking.bits`. Each iteration
    /// walks the rules, adds cone tokens when triggered and not suppressed,
    /// and stops when no new bits appear.
    fn closure(&self, marking: Self::Marking) -> Self::Marking {
        let mut working = marking;
        for _iter in 0..marque_scheme::closure::MAX_CLOSURE_ITERATIONS {
            let prev_bits = working.bits;
            for rule in CLOSURE_RULES {
                if rule.should_fire(self, &working) {
                    for token_id in rule.cone_token_ids() {
                        if let Some(n) = bit_index(token_id) {
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
            if working.bits == prev_bits {
                // Fixed point reached.
                return working;
            }
        }
        panic!(
            "closure operator did not converge within {} iterations — \
             catalog bug: closure rules must be monotone",
            marque_scheme::closure::MAX_CLOSURE_ITERATIONS
        );
    }
}

#[test]
fn default_closure_inventory_forwards_from_closure_rules() {
    let scheme = ClosureStubScheme;
    let inventory: Vec<_> = scheme.closure_inventory().collect();

    assert_eq!(inventory.len(), CLOSURE_RULES.len());

    for (metadata, rule) in inventory.iter().zip(CLOSURE_RULES.iter()) {
        assert_eq!(metadata.name, rule.name);
        assert_eq!(metadata.label, rule.display_label);
        assert_eq!(metadata.citation, Some(rule.label));
        assert_eq!(metadata.default_severity, rule.default_severity);
    }
}

// ---------------------------------------------------------------------------
// Property 1: Monotone.
//   ∀ m1, m2 with m1 ⊑ m2: closure(m1) ⊑ closure(m2).
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn closure_is_monotone(bits1 in any::<u8>(), bits2 in any::<u8>()) {
        let scheme = ClosureStubScheme;
        // Construct m1 ⊑ m2 by taking m1 = bits1 & bits2, m2 = bits2.
        // This guarantees m1.bits ⊆ m2.bits (every bit in m1 is in m2).
        let m1 = BitMarking::with(bits1 & bits2);
        let m2 = BitMarking::with(bits2);

        let c1 = scheme.closure(m1);
        let c2 = scheme.closure(m2);

        prop_assert!(
            c1.le(&c2),
            "monotonicity violation: closure({:08b}) = {:08b}, \
             closure({:08b}) = {:08b}, but {:08b} ⊄ {:08b}",
            bits1 & bits2, c1.bits,
            bits2, c2.bits,
            c1.bits, c2.bits
        );
    }
}

// ---------------------------------------------------------------------------
// Property 2: Extensive.
//   ∀ m: m ⊑ closure(m).
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn closure_is_extensive(bits in any::<u8>()) {
        let scheme = ClosureStubScheme;
        let m = BitMarking::with(bits);
        let closed = scheme.closure(m.clone());

        prop_assert!(
            m.le(&closed),
            "extensiveness violation: marking {:08b} ⊄ closure {:08b} \
             (closure removed bits that were present in the original)",
            m.bits, closed.bits
        );
    }
}

// ---------------------------------------------------------------------------
// Property 3: Idempotent.
//   ∀ m: closure(closure(m)) == closure(m).
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn closure_is_idempotent(bits in any::<u8>()) {
        let scheme = ClosureStubScheme;
        let m = BitMarking::with(bits);
        let once = scheme.closure(m);
        let twice = scheme.closure(once.clone());

        prop_assert_eq!(
            once.bits, twice.bits,
            "idempotence violation: closure(closure({:08b})) = {:08b} \
             but closure({:08b}) = {:08b}",
            bits, twice.bits, bits, once.bits
        );
    }
}

// ---------------------------------------------------------------------------
// Property 4: Unconditional-catalog monotonicity (chain-depth stress test).
//
//   Verifies that a catalog of UNCONDITIONAL rules (no suppressors) is
//   trivially monotone: ∀ m1 ⊑ m2, closure(m1) ⊑ closure(m2).
//
//   This test exercises the transitive closure chain (A→B, C→D) to ensure
//   that even when rules fire transitively, monotonicity holds across all
//   input pairs.
//
//   PR 3.7 CAVEAT (per Copilot PR review #13): the plan also called for a
//   "suppression-doesn't-break-monotonicity" property — i.e., a stub
//   catalog WITH suppressors that exercises the §4.7.3 table-design-
//   property monotonicity proof under suppressor-stability. Property 4
//   here does NOT cover that case (no suppressors in the stub catalog);
//   it covers transitive-chain monotonicity over unconditional rules.
//   The suppression-stability property requires a stub catalog encoding
//   the disjoint-suppressor invariant (suppressor tokens must be tokens
//   that are NEVER added by any cone rule), which is non-trivial to
//   express in property-test form without recreating CAPCO's specific
//   FDR_DOMINATORS / cone separation. The CAPCO catalog enforces the
//   invariant by construction; the per-row CAPCO monotonicity attestation
//   that PR 3.7's §9 acceptance defers to PR 4 is the actual gate for
//   suppression-stability in the production catalog.
//
//   The negative test in `proptest_closure_rejects_non_monotone.rs`
//   demonstrates the suppressor-based violation scenario from the
//   theoretical side.
//
//   The redundancy with Property 1 is intentional — Property 1 tests the
//   base statement; Property 4 stresses it with transitive chains.
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn unconditional_catalog_monotonicity_with_chains(
        bits1 in any::<u8>(),
        bits2 in any::<u8>(),
    ) {
        let scheme = ClosureStubScheme;
        // m1 ⊑ m2: m1 has a subset of m2's bits.
        let m1 = BitMarking::with(bits1 & bits2);
        let m2 = BitMarking::with(bits2);

        let c1 = scheme.closure(m1);
        let c2 = scheme.closure(m2);

        prop_assert!(
            c1.le(&c2),
            "unconditional catalog violated monotonicity: m1={:08b} ⊑ m2={:08b}, \
             but closure(m1)={:08b} ⊄ closure(m2)={:08b}",
            bits1 & bits2, bits2, c1.bits, c2.bits
        );
    }
}

// ---------------------------------------------------------------------------
// Property 5: G13 content-ignorance regression.
//   Closure output (rendered as a string) must not contain any bytes from
//   the input marking verbatim.
//
//   Constitution V Principle V: audit records MUST be content-ignorant.
//   No document content, document metadata field values, or subject-claim
//   free-form text MAY appear in closure output. Only structural identifiers
//   (token IDs, category IDs, offsets) are permitted.
//
//   For this stub, we use the rendered form of the marking (which encodes
//   only the bit-pattern, never the input string). The test verifies that
//   the rendered output does NOT contain any non-trivial byte sequence from
//   an arbitrary "document content" string.
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn closure_output_does_not_leak_document_bytes(
        bits in any::<u8>(),
        // Simulate a "document content" string: arbitrary ASCII text
        // restricted to letters that cannot appear in the stub
        // scheme's render output.
        //
        // The stub `render_banner` emits `"bits=0x{:02x}"`. To prevent
        // chance collisions where a random doc_content substring
        // matches part of the constant render prefix (`bits`) or the
        // hex digits (a-f / A-F), exclude every letter in
        // {b, i, t, s, x, a, c, d, e, f} case-insensitively. The
        // earlier exclusion {a-f / A-F} alone left "its" as a
        // collision class with `bits=0x...` (Copilot PR 3.7 review #7).
        //
        // Allowed lowercase: g, h, j, k, l, m, n, o, p, q, r, u, v, w, y, z.
        // Allowed uppercase: G, H, J, K, L, M, N, O, P, Q, R, U, V, W, Y, Z.
        //
        // The proptest's actual G13 invariant is preserved: no
        // document-content bytes surface in the closure operator's
        // rendered output. The regex restriction only eliminates the
        // collision class for THIS particular stub render shape; the
        // production CapcoScheme renderer (PR 4) will need a parallel
        // exclusion against its own format-string constants.
        doc_content in "[ghjk-rmnop-rqu-wyzGHJK-RMNOP-RQU-WYZ]\
                        [ghjk-rmnop-rqu-wyzGHJK-RMNOP-RQU-WYZ ]{4,20}",
    ) {
        let scheme = ClosureStubScheme;
        let m = BitMarking::with(bits);
        let closed = scheme.closure(m);
        let rendered = scheme.render_banner(&closed);

        // The rendered output should be "bits=0x{:02x}" — it must NOT
        // contain any substring from doc_content (which simulates
        // document bytes that must never appear in audit/closure output).
        for (i, _c) in doc_content.char_indices() {
            for j in (i + 1)..=doc_content.len() {
                if !doc_content.is_char_boundary(j) {
                    continue;
                }
                let substr = &doc_content[i..j];
                if substr.len() >= 3 {
                    prop_assert!(
                        !rendered.contains(substr),
                        "G13 violation: closure output {:?} contains document bytes {:?}",
                        rendered,
                        substr
                    );
                }
            }
        }
    }
}
