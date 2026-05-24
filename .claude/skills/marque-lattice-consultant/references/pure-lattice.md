# Pure Lattice & Order Theory — Catalog Reference

**Audience.** Claude, in lattice-consultant mode, scanning for the closest named construction matching a problem the user just described in informal English.

**How to use.** Locate the catalog entry whose definition matches (or nearly matches) the construction in front of you. Each entry tags which consultant outcome it most often supports — `(a)` order-theory-adapted, `(b)` pivot toward a known pattern, `(c)` refuse / redirect — and ends with a "When this comes up" hook so you can route to it from a question shape.

**Companion files in this skill.**
- `abstract-interp.md` (Agent C) — covers Galois connections, fixed-point iteration, and abstract interpretation in depth. This file gives the algebraic surface; that one gives the program-analysis machinery.
- `marque-applied.md` (Agent E) — translates the catalog entries here into the marque-specific design questions.

---

## Table of Contents

1. [Partially ordered set (poset)](#1-partially-ordered-set-poset) — reflexive antisymmetric transitive; the foundation.
2. [Meet-semilattice / join-semilattice](#2-meet-semilattice--join-semilattice) — every pair has a meet, *or* every pair has a join, but not necessarily both.
3. [Lattice](#3-lattice) — both meets and joins; algebraic axioms (absorption, commutativity, associativity, idempotence).
4. [Bounded lattice](#4-bounded-lattice) — top and bottom; what makes a lattice "complete enough to talk about emptiness."
5. [Complete lattice](#5-complete-lattice) — arbitrary subsets have meets and joins; powers Knaster-Tarski.
6. [Distributive lattice](#6-distributive-lattice) — meet distributes over join; Birkhoff's M3/N5 forbidden-sublattice theorem.
7. [Modular lattice](#7-modular-lattice) — Dedekind's weakened distributivity; "no N5."
8. [Boolean lattice / Boolean algebra](#8-boolean-lattice--boolean-algebra) — distributive plus complemented; Stone duality.
9. [Heyting algebra](#9-heyting-algebra) — relative pseudocomplements; intuitionistic logic.
10. [Free lattice on n generators](#10-free-lattice-on-n-generators) — what unconstrained lattice expressions look like; Whitman's word-problem solution.
11. [Product lattice](#11-product-lattice) — coordinatewise meet/join; how independent policies compose.
12. [Sum / disjoint-union lattice](#12-sum--disjoint-union-lattice) — coproducts; when components don't interact.
13. [Dual lattice](#13-dual-lattice) — flip the order; integrity-vs-confidentiality is the canonical instance.
14. [Sublattice / convex sublattice](#14-sublattice--convex-sublattice) — closure under meet and join; what a subset must satisfy to inherit lattice structure.
15. [Lattice homomorphism, embedding, isomorphism](#15-lattice-homomorphism-embedding-isomorphism) — the morphism notions.
16. [Lattice congruence](#16-lattice-congruence) — quotients; equivalence relations that respect meet and join.
17. [Galois connection / adjunction](#17-galois-connection--adjunction) — antitone vs. monotone forms; the universal abstraction-and-concretization pattern.
18. [Closure operator / interior operator](#18-closure-operator--interior-operator) — fixed points form a complete lattice.
19. [Knaster-Tarski theorem](#19-knaster-tarski-theorem) — every monotone op on a complete lattice has lfp/gfp.
20. [Kleene fixed-point theorem](#20-kleene-fixed-point-theorem) — Scott-continuous case; lfp computed by iterating from bottom.
21. [Chain / antichain / Dilworth's theorem](#21-chain--antichain--dilworths-theorem) — width = minimum chain decomposition.
22. [Well-founded order / Noetherian poset](#22-well-founded-order--noetherian-poset) — termination arguments; descending chain condition.
23. [CPO / dcpo / Scott topology](#23-cpo--dcpo--scott-topology) — minimal sketch; deep treatment lives in `abstract-interp.md`.

[Common pitfalls — diagnosing whether something is actually a lattice](#common-pitfalls---diagnosing-whether-something-is-actually-a-lattice)

[How to read the consultant tags](#how-to-read-the-consultant-tags)

---

## 1. Partially ordered set (poset)

**Definition.** A *poset* is a pair `(P, ≤)` where `≤` is a binary relation on `P` satisfying, for all `x, y, z ∈ P`:

- **Reflexivity:** `x ≤ x`.
- **Antisymmetry:** `x ≤ y ∧ y ≤ x ⇒ x = y`.
- **Transitivity:** `x ≤ y ∧ y ≤ z ⇒ x ≤ z`.

A *preorder* drops antisymmetry (so `x ≤ y ≤ x` is allowed without forcing equality); a *strict order* `<` is the irreflexive transitive part. See `[davey-priestley-2002]` ch. 1, `[nation-lattice-notes]` ch. 1.

**Example.** `(P(X), ⊆)` — the subsets of any set ordered by inclusion. `(ℕ, |)` — naturals ordered by divisibility (e.g. `2 | 6` but `2 ∤ 9`).

**Non-example.** `(ℕ, ≠)` — irreflexive and not transitive; no order. `(People, knows)` — typically not antisymmetric and not transitive.

**Citation.** `[davey-priestley-2002]` Definition 1.2; `[nation-lattice-notes]` §1.1.

**Consultant tags.** `(a)` baseline — every entry below is a poset with extra structure. `(c)` — when a user describes a relation that fails reflexivity, antisymmetry, or transitivity, name the failure and stop pretending it's a lattice problem.

> **When this comes up.** A user asks whether some "dependency relation" or "policy precedence" between objects forms a lattice. First check it's even a poset: are there cycles (failure of antisymmetry), incomparable-but-related elements joined by some non-transitive linkage, etc.? Many "almost-lattices" turn out to be preorders or DAGs that need a different framework.

---

## 2. Meet-semilattice / join-semilattice

**Definition.** A poset `(P, ≤)` is a *meet-semilattice* if every pair `x, y ∈ P` has a *greatest lower bound* (meet, infimum) `x ∧ y`. Equivalently, a meet-semilattice is a commutative idempotent semigroup `(P, ∧)` (the order is recovered as `x ≤ y ⇔ x ∧ y = x`).

A *join-semilattice* is the order-dual: every pair has a *least upper bound* (join, supremum) `x ∨ y`.

A *bounded* meet-semilattice has a top `⊤`; a *bounded* join-semilattice has a bottom `⊥`. (The terminology is occasionally inverted in the literature — check the source.)

**Laws (for, say, a meet-semilattice).**
- Idempotence: `x ∧ x = x`.
- Commutativity: `x ∧ y = y ∧ x`.
- Associativity: `(x ∧ y) ∧ z = x ∧ (y ∧ z)`.

**Example.** The natural numbers under `gcd`: `(ℕ⁺, gcd)` is a meet-semilattice. Every pair has a `gcd`, but `lcm` of `{2, 3, 5, …}` (all primes) doesn't exist in any finite-precision sense — the join of an infinite set may fail.

The set of *finite* subsets of an infinite set under `∩` is a meet-semilattice; under `∪` it is also a join-semilattice; together it forms a lattice without a top.

**Non-example.** A poset where some pair has multiple incomparable lower bounds, none greatest: `{a, b, c, d}` with `c, d ≤ a` and `c, d ≤ b` and no order between `c, d` — the meet `a ∧ b` doesn't exist.

**Citation.** `[davey-priestley-2002]` Definition 2.1 and §2.5; `[burris-sankappanavar-1981]` §I.1, particularly Definition 1.1; `[nation-lattice-notes]` §2.

**Consultant tags.** `(a)` — when only one of the two operations is needed (typical of "rolling up a set of markings into a least-restrictive supremum"), name it as a join-semilattice and don't promise lattice-level laws. `(b)` — if the user's structure is a meet-semilattice they keep insisting must be a lattice, suggest either restricting to a sublattice where joins exist or admitting that no top means certain rules can never have a maximum.

> **When this comes up.** A user describes "I take the union of these tagged sets across a page; that's how I aggregate." That's a join. They may not need meets at all — and *demanding* the structure be a full lattice often forces an artificial top element ("∞-classified") that has no operational meaning. Recommend join-semilattice (with optional bottom for the empty union) as the precise type.

---

## 3. Lattice

**Definition (order-theoretic).** A poset `(L, ≤)` in which every pair `x, y ∈ L` has both a meet `x ∧ y` and a join `x ∨ y`. Equivalently, `L` is both a meet-semilattice and a join-semilattice with the same underlying order.

**Definition (algebraic / equational).** An algebra `(L, ∧, ∨)` where `∧` and `∨` are binary operations satisfying, for all `x, y, z ∈ L`:

- **Idempotence:** `x ∧ x = x`, `x ∨ x = x`.
- **Commutativity:** `x ∧ y = y ∧ x`, `x ∨ y = y ∨ x`.
- **Associativity:** `(x ∧ y) ∧ z = x ∧ (y ∧ z)`, `(x ∨ y) ∨ z = x ∨ (y ∨ z)`.
- **Absorption:** `x ∧ (x ∨ y) = x`, `x ∨ (x ∧ y) = x`.

The two definitions are equivalent: from the algebraic one, define `x ≤ y ⇔ x ∧ y = x` (or equivalently `x ∨ y = y`) and recover a poset whose meets and joins are the algebraic ones. See `[davey-priestley-2002]` Theorem 2.9, `[burris-sankappanavar-1981]` Theorem I.3.3.

**Notable consequence.** Idempotence is *not* a free consequence of the other axioms — it must be checked or derived. From absorption alone, with idempotence, you get `x ∧ x = x ∧ (x ∨ (x ∧ x)) = x` only after assuming the inner idempotence. In practice: a fold-based `meet` implementation can violate idempotence silently if it accidentally counts duplicates.

**Example.** `(P(X), ⊆)` with `∩, ∪` — the canonical lattice. `(ℕ⁺, |)` with `gcd`, `lcm` — divisibility lattice. `(Sub(G), ⊆)` for a group `G` — the lattice of subgroups, with `∩` for meet and the generated subgroup for join.

**Non-example (semilattice masquerading as lattice).** Finite subsets of `ℕ` with `∩` and `∪`: this *is* a lattice (without a top). But finite subsets ordered by reverse inclusion, with `∪` as meet and `∩` as join: also a lattice (now without a bottom). The danger is mixing these conventions and ending up with operations whose absorption laws fail.

**Citation.** `[davey-priestley-2002]` Definition 2.4 and Theorem 2.9; `[burris-sankappanavar-1981]` Definition I.3.1 and Theorem I.3.3; `[birkhoff-1967]` ch. I §6; `[nlab-lattice]`.

**Consultant tags.** `(a)` baseline — most marque shape-questions resolve into "is this construction a lattice?" `(b)` — if the user's binary ops fail one of the four equational laws, propose either redefining them via the order or identifying the structure as a semilattice. `(c)` — if both equational laws and the order definitions disagree, the construction is contradictory and needs redesign before any lattice machinery applies.

> **When this comes up.** A user defines `meet` and `join` independently as Rust functions and then asks "is this a lattice?" Walk through all four equational laws (especially **absorption** — it's the easy-to-miss one); if any fails, the construction may still be "two semilattices that happen to share a domain" but is not a lattice, and theorems like Knaster-Tarski cannot be invoked.

---

## 4. Bounded lattice

**Definition.** A lattice `(L, ∧, ∨)` is *bounded* if it has elements `⊤` (top) and `⊥` (bottom) with `⊥ ≤ x ≤ ⊤` for all `x ∈ L`. Equivalently, `⊤` is the identity for `∧` and `⊥` is the identity for `∨`:

- `x ∧ ⊤ = x`, `x ∨ ⊥ = x` for all `x`.

Some authors fold boundedness into the definition of "lattice" (so `(L, ∧, ∨, ⊤, ⊥)` is the signature); others do not. `[davey-priestley-2002]` distinguishes between *lattice* (no top/bottom required) and *bounded lattice*, while `[nlab-lattice]` also distinguishes between "bounded lattice" and "pseudolattice." When a user says "lattice" colloquially, ask which they mean.

**Example.** Any finite lattice is bounded (the meet of all elements is the bottom, the join is the top). `(P(X), ⊆)` is bounded with `⊥ = ∅`, `⊤ = X`.

**Non-example (lattice without top).** Finite subsets of `ℕ` ordered by `⊆` — bottom is `∅`, no top exists. This is a perfectly good lattice, just not bounded. Adding a freshly-invented `⊤ = ℕ` would change what membership means and break operational interpretation.

**Open-set / agency-extensible warning.** When a "vocabulary" can be extended by an outside authority (new compartment names, new SAR programs, new partner-national codes), there is *no top element*. The construction is at best a join-semilattice; it is a lattice only on each fixed instantiation. An interface like `BoundedLattice::top()` for such a domain is a category error.

**Citation.** `[davey-priestley-2002]` Definition 2.13; `[nlab-lattice]`.

**Consultant tags.** `(a)` when the domain is finite or has a meaningful absolute. `(c)` when the domain is genuinely open — name it as "join-semilattice with bottom, no top" rather than torturing it into a bounded lattice.

> **When this comes up.** "We need a `top` for our `Lattice` impl, but the domain is agency-extensible." That's a `(c)` — the structure is a (non-bounded) join-semilattice, not a `BoundedLattice`. Pushing for a top element silently changes the semantics; refuse and redirect to the correct trait surface.

---

## 5. Complete lattice

**Definition.** A lattice `L` is *complete* if every subset `S ⊆ L` (including the empty set and infinite subsets) has both a join `⋁S` and a meet `⋀S`. Equivalently, every subset has a supremum (so it's a complete join-semilattice — and a classical theorem of order theory says that this alone implies completeness as a lattice, since meets are recovered as joins of lower-bound sets).

By taking `S = L`: a complete lattice has `⋁ L = ⊤` and `⋀ L = ⊥`, so every complete lattice is bounded.

**Theorem (every complete join-semilattice is a complete lattice).** If `(L, ≤)` has a join for every subset, then it has a meet for every subset, given by `⋀ S = ⋁ {x ∈ L : ∀ s ∈ S, x ≤ s}`. See `[davey-priestley-2002]` Theorem 2.31, `[wikipedia-complete-lattice]`.

**Example.** `(P(X), ⊆)` is complete: arbitrary unions and intersections are sets. `(ℝ ∪ {-∞, +∞}, ≤)` is a complete lattice (the extended real line). The lattice of closed sets of any topological space is complete.

**Non-example.** `(ℕ, ≤)` — bounded below by 0, but the set `ℕ` itself has no upper bound in `ℕ`, so the join of `ℕ` doesn't exist. `(ℚ, ≤)` — the set `{q ∈ ℚ : q² < 2}` has no rational supremum (this is exactly the gap Dedekind cuts plug to construct `ℝ`).

**Why it matters.** Knaster-Tarski (entry 19) requires completeness. So does the existence of arbitrary closures, free constructions in many algebraic settings, and the Galois-connection induced closure operators on either side (entry 17–18). When a user wants to talk about "the smallest fixed point of this operator," completeness is what licenses the talk.

**Citation.** `[davey-priestley-2002]` Definition 2.31, Theorem 2.31; `[burris-sankappanavar-1981]` §I.4; `[nation-lattice-notes]` ch. 2; `[wikipedia-complete-lattice]`.

**Consultant tags.** `(a)` when the user wants fixed-point arguments — completeness is the precondition. `(b)` when the structure is a finite lattice (automatically complete) and the user wants to apply infinitary results without rederiving them.

> **When this comes up.** "We want a unique smallest fixed point of this monotone operator." Verify the underlying lattice is complete — typically yes for finite-height domains. If yes, cite Knaster-Tarski (entry 19). If no (e.g., `ℚ` or open-vocabulary domains), the lfp may not exist; redirect to either a completion or a different formalism.

---

## 6. Distributive lattice

**Definition.** A lattice `L` is *distributive* if for all `x, y, z ∈ L`:

- `x ∧ (y ∨ z) = (x ∧ y) ∨ (x ∧ z)`,
- `x ∨ (y ∧ z) = (x ∨ y) ∧ (x ∨ z)`.

(Either law implies the other in any lattice — see `[davey-priestley-2002]` Lemma 4.3, `[burris-sankappanavar-1981]` Theorem I.7.5.)

**Birkhoff's M3-N5 theorem.** A lattice `L` is distributive iff it contains *neither* `M₃` (the diamond, three pairwise-incomparable elements between bottom and top) *nor* `N₅` (the pentagon, the modular but non-distributive 5-element lattice) as a sublattice. See `[birkhoff-1937]` (the original "Rings of sets" paper) and `[davey-priestley-2002]` Theorem 4.10.

**Birkhoff's representation theorem.** Every finite distributive lattice `L` is isomorphic to the lattice of *down-sets* of the poset of join-irreducible elements of `L`. So finite distributive lattices ARE (up to iso) lattices of down-sets, equivalently subsets of some poset closed under "going down." See `[birkhoff-1937]` and `[wikipedia-birkhoff-rep]`.

**Cancellation characterization.** A lattice is distributive iff it satisfies the cancellation law: `x ∧ y = x ∧ z ∧ x ∨ y = x ∨ z ⇒ y = z`. See `[nlab-distributive-lattice]`, `[davey-priestley-2002]` Theorem 4.4.

**Example.** `(P(X), ⊆)` with `∩, ∪` — distributive (set-theoretic distributivity). Any chain (totally ordered set) is distributive (trivially, since the lattice is already a chain). `(ℕ⁺, |)` with `gcd, lcm` — distributive; this is the classical "fundamental theorem of arithmetic" wearing a lattice hat.

**Non-example (M3 — diamond).** Five elements `{⊥, a, b, c, ⊤}` with `a, b, c` pairwise incomparable, all above `⊥` and below `⊤`. Then `a ∧ (b ∨ c) = a ∧ ⊤ = a` but `(a ∧ b) ∨ (a ∧ c) = ⊥ ∨ ⊥ = ⊥`. Distribution fails. M3 is nonetheless modular (entry 7).

**Non-example (N5 — pentagon).** Five elements `{⊥, a, b, c, ⊤}` with `b ≤ c`, `a` incomparable to `b, c`. Then `a ∨ (b ∧ c) = a ∨ b = ⊤` but `(a ∨ b) ∧ c = ⊤ ∧ c = c`. Distribution fails. N5 is also non-modular (and so non-distributive).

**Citation.** `[birkhoff-1937]` (original); `[davey-priestley-2002]` ch. 4; `[burris-sankappanavar-1981]` §I.7; `[nlab-distributive-lattice]`; `[wikipedia-distributive-lattice]`; `[nation-lattice-notes]` ch. 8.

**Consultant tags.** `(a)` when distributivity buys the user something — Stone duality, normal form for lattice expressions, predictable interaction with set-like operations. `(c)` when the user *thinks* they have a distributive lattice but a confidentiality-and-integrity composition embeds an M3 or N5 — check explicitly with three or five sample elements.

> **When this comes up.** "We compose two policy lattices and assume distribution holds for the joint operations." Check by testing on a small instance whether `a ∧ (b ∨ c) = (a ∧ b) ∨ (a ∧ c)`. If three pairwise-incomparable mid-lattice elements exist, you may be looking at M3; that often signals the composition is genuinely modular but not distributive (entry 7) and certain proof strategies need adjusting.

---

## 7. Modular lattice

**Definition (Dedekind's modular law).** A lattice `L` is *modular* if for all `a, b, c ∈ L` with `a ≤ b`:

- `a ∨ (c ∧ b) = (a ∨ c) ∧ b`.

This is a self-dual condition — it reads the same with `∧, ∨` swapped given the antisymmetric form. Modularity is strictly weaker than distributivity: every distributive lattice is modular (set `a, b, c` arbitrary in the distributive law and use `a ≤ b` to simplify), but not conversely.

**Dedekind's N5 characterization.** A lattice is modular iff it does *not* contain `N₅` (the pentagon) as a sublattice. So the M3/N5 dichotomy splits cleanly: containing N5 means non-modular; containing only M3 (no N5) means modular but not distributive. See `[wikipedia-modular-lattice]`, `[davey-priestley-2002]` Theorem 4.10.

**Diamond isomorphism theorem.** In a modular lattice, for any `a, b ∈ L`, the maps `[a ∧ b, b] → [a, a ∨ b]` given by `x ↦ x ∨ a` and `y ↦ y ∧ b` are mutually inverse order isomorphisms. Equivalently: a lattice is modular iff this isomorphism holds for all pairs `a, b`. See `[nation-lattice-notes]` ch. 9; `[davey-priestley-2002]` Theorem 4.13.

**Example.** The lattice of subgroups of an abelian group (or, more generally, of any group's *normal* subgroups). The lattice of submodules of any module. The lattice of normal subgroups of any group. M3 itself.

**Non-example.** N5. The lattice of subgroups of `S₃` (the symmetric group on three elements) — contains N5 because the subgroup generated by a transposition and a 3-cycle is a non-modular configuration.

**Why it matters.** When the lattice is modular but not distributive, several proof techniques degrade gracefully — many results that need distributivity have modular-lattice analogs. Crucially, modular lattices have the diamond isomorphism theorem (a strong form of the second isomorphism theorem from group theory transferred to lattices).

**Citation.** `[dedekind-1894]` (original, in German — "Über die von drei Moduln erzeugte Dualgruppe"); `[davey-priestley-2002]` §4.2; `[birkhoff-1967]` ch. I §7; `[nation-lattice-notes]` ch. 9; `[wikipedia-modular-lattice]`.

**Consultant tags.** `(a)` when the user's lattice arises from algebraic substructures (subgroups, submodules, ideals) — these are typically modular by structural theorems. `(b)` when the user *wants* distributivity but only modular holds — most distributive theorems have a modular-lattice analog with a "subject to a containment hypothesis" caveat. `(c)` when neither holds; many fixed-point and order-iteration arguments still go through (Knaster-Tarski needs only completeness), so don't over-claim that "non-modular = no theorems available."

> **When this comes up.** A user has a lattice that fails distributivity but doesn't contain N5. That's modular-but-not-distributive. Tell them they keep most algebraic isomorphism results (diamond isomorphism) but lose the M3-free representation theorem (Birkhoff representation, entry 6).

---

## 8. Boolean lattice / Boolean algebra

**Definition.** A *Boolean lattice* is a bounded distributive lattice in which every element `x` has a *complement* `¬x` satisfying:

- `x ∧ ¬x = ⊥`,
- `x ∨ ¬x = ⊤`.

In a distributive lattice, complements (when they exist) are unique. A *Boolean algebra* is a Boolean lattice considered as an algebraic structure `(B, ∧, ∨, ¬, ⊥, ⊤)` — the same object with a richer signature.

**Properties.**
- De Morgan's laws hold: `¬(x ∧ y) = ¬x ∨ ¬y`, `¬(x ∨ y) = ¬x ∧ ¬y`.
- Double negation: `¬¬x = x` (this fails in Heyting algebras — entry 9).
- Excluded middle: `x ∨ ¬x = ⊤` (axiomatic).

**Stone's representation theorem.** Every Boolean algebra is isomorphic to a *field of sets* — the algebra of clopen sets of its Stone space (a compact totally disconnected Hausdorff space). For finite Boolean algebras, this collapses to: every finite Boolean algebra is isomorphic to `(P(X), ∩, ∪, ∁, ∅, X)` for some finite set `X`. See `[stone-1936]`, `[davey-priestley-2002]` ch. 11.

**Example.** `(P(X), ∩, ∪, ∁, ∅, X)` — the canonical Boolean algebra. `({0,1}^n, ∧, ∨, ¬)` — the n-fold Boolean cube; isomorphic to `P({1, …, n})`. The Lindenbaum-Tarski algebra of classical propositional logic modulo provable equivalence.

**Non-example.** Open sets of a topological space form a Heyting algebra (entry 9) but generally not a Boolean algebra: the complement of an open set is closed, not open, so `¬U` (in the lattice-theoretic sense, the largest open set disjoint from `U`) is `int(X ∖ U)`, not `X ∖ U`. Then `U ∨ ¬U = U ∪ int(X ∖ U)` is generally a proper subset of `X`.

**Citation.** `[stone-1936]`; `[davey-priestley-2002]` ch. 4 §4 and ch. 11; `[burris-sankappanavar-1981]` §IV.1; `[wikipedia-boolean-algebra]`; `[nlab-distributive-lattice]`.

**Consultant tags.** `(a)` when the user has classical-logic / set-theoretic semantics with negation. `(c)` when complement is partial (not every element has one) — that's a *complemented lattice* but not necessarily Boolean.

> **When this comes up.** "We want negation on our marking lattice." Boolean only if the lattice is bounded, distributive, and every element has a complement. Otherwise consider Heyting algebras (entry 9), where you get an *implication* operator without requiring `¬¬x = x` (which would be unnatural for asymmetric notions like security clearance).

---

## 9. Heyting algebra

**Definition.** A *Heyting algebra* is a bounded lattice `H` equipped with a binary operation `⇒` (relative pseudocomplement, or implication) satisfying the adjointness condition: for all `x, a, b ∈ H`,

- `x ∧ a ≤ b ⇔ x ≤ (a ⇒ b)`.

Equivalently, `H` is a bounded lattice such that for every `a, b ∈ H` the set `{x : x ∧ a ≤ b}` has a greatest element, namely `a ⇒ b`. Negation is defined as `¬x = (x ⇒ ⊥)`.

A Heyting algebra is automatically distributive. It is a Boolean algebra iff *excluded middle* holds: `x ∨ ¬x = ⊤` for all `x`. Equivalently, iff `¬¬x = x`. See `[nlab-heyting-algebra]`, `[davey-priestley-2002]` ch. 5.

**Properties.**
- Distributivity: free, as a consequence of the adjointness.
- `x ⇒ y = ⊤ ⇔ x ≤ y`.
- `¬¬x ≥ x` always; equality holds iff `x` is *regular*.
- `(x ⇒ y) ∧ x ≤ y` (modus ponens at the lattice level).

**Logical interpretation.** A Heyting algebra is the algebraic semantics of intuitionistic propositional logic. Implication becomes meaningful (as a residuation) without forcing classical excluded middle. Boolean algebras correspond to classical logic; Heyting algebras to intuitionistic logic.

**Example.** The lattice of *open sets* of any topological space is a Heyting algebra: `U ⇒ V := int(U^c ∪ V) = int((X ∖ U) ∪ V)`, where `int` is interior. Negation is `¬U = int(X ∖ U)`. Generally `U ∨ ¬U ≠ X` (the boundary of `U` is missing).

The *frame* (complete Heyting algebra) of opens of any space is the central object of pointless topology / locale theory.

**Non-example.** Any non-distributive lattice — Heyting algebras must be distributive.

**Citation.** `[nlab-heyting-algebra]`; `[davey-priestley-2002]` ch. 5 (titled "Heyting Algebras and Boolean Algebras"); `[johnstone-stone-spaces]` ch. I; `[burris-sankappanavar-1981]` §IV.6.

**Consultant tags.** `(a)` when the user has an asymmetric "informs about" or "downgrades to" relation that resists classical negation. `(b)` when the user is forcing a Boolean structure where intuitionistic semantics would be more honest. `(c)` when the lattice fails distributivity — Heyting structure requires distributivity, so fix that first.

> **When this comes up.** A user wants implication "if marked X then must mark Y" as a lattice operation. Heyting `⇒` is the principled construction *if* the lattice is distributive. Boolean negation is too strong — it asserts a complement exists for every state, which is rarely true for security markings.

---

## 10. Free lattice on n generators

**Definition.** The *free lattice* `FL(X)` on a set `X` is the lattice with the universal property that any function `f : X → L` from `X` into any lattice `L` extends uniquely to a lattice homomorphism `FL(X) → L`. Concretely, elements are equivalence classes of well-formed lattice expressions over `X` modulo the four lattice laws (idempotence, commutativity, associativity, absorption).

**Whitman's word problem.** Whitman gave an algorithm in `[whitman-1941a]` (Annals of Mathematics 42, pp. 325–329) and `[whitman-1941b]` ("Free Lattices II," Annals of Mathematics 43, pp. 104–115) for deciding whether two lattice terms are equal in every lattice.

**Whitman's condition.** In any free lattice, `a ∧ b ≤ c ∨ d` holds *only* in one of the four trivial ways:

1. `a ≤ c ∨ d`, or
2. `b ≤ c ∨ d`, or
3. `a ∧ b ≤ c`, or
4. `a ∧ b ≤ d`.

This recursive condition characterizes which inequalities can hold in `FL(X)` — a recursive procedure on term structure decides equality. See `[freese-nation-lectures]`.

**Structure.** The free lattice on 0 or 1 generator is degenerate (empty or one-element). On 2 generators it has 4 elements. On 3 generators it is *infinite* (a famous result of Whitman). The free lattice on `n ≥ 3` generators is uncountable... no, the free lattice on a *countable* set of generators is countably infinite; the free *complete* lattice on 3 generators is a different beast (and is known to be a proper class in some formulations — see `[hales-1964]`).

**Why it matters.** Free constructions are the universal scaffolding for "lattice expressions before any quotienting." When a user defines lattice operations equationally and asks whether their construction is well-defined, the question reduces to: does my construction factor through the free lattice quotiented by the equations I impose? If yes, it's a lattice; if no, the equations are inconsistent.

**Example.** The free lattice on `{x, y}` consists of `{x, y, x ∧ y, x ∨ y}` — everything else collapses by absorption. The free lattice on `{x, y, z}` is infinite; `(x ∨ y) ∧ z`, `(x ∨ y) ∧ z ∧ (x ∨ y)`, and various interleavings are all distinct.

**Citation.** `[whitman-1941a]`, `[whitman-1941b]`; `[freese-nation-lectures]`; `[wikipedia-free-lattice]`; `[nation-lattice-notes]` ch. 6.

**Consultant tags.** `(b)` mostly — if a user's "candidate lattice operations" yield an infinite or pathological structure under iteration, the free-lattice perspective tells them why: there's no equational law strong enough to collapse the expressions, so they need to identify or impose more relations. `(c)` when the user wants a *finite* lattice but their generators yield an infinite free lattice — that's a structural obstruction, not a polish issue.

> **When this comes up.** "We have these primitive markings and these operations; do we get a lattice or some open-ended thing?" Frame as: their primitives are generators, their identities are relations, and the resulting structure is the free lattice on those generators *modulo* those relations. If the relations are insufficient, infinite expressions remain inequivalent.

---

## 11. Product lattice

**Definition.** Given lattices `L₁, …, L_n`, the *product lattice* `L₁ × … × L_n` has underlying set the Cartesian product, with operations defined coordinatewise:

- `(x₁, …, x_n) ∧ (y₁, …, y_n) := (x₁ ∧₁ y₁, …, x_n ∧_n y_n)`,
- `(x₁, …, x_n) ∨ (y₁, …, y_n) := (x₁ ∨₁ y₁, …, x_n ∨_n y_n)`,
- `(x₁, …, x_n) ≤ (y₁, …, y_n) ⇔ ∀i, x_i ≤_i y_i`.

The construction generalizes to arbitrary index sets. The product is bounded iff each factor is, with `⊤ = (⊤₁, …)` and `⊥ = (⊥₁, …)`. Distributive iff each factor is. Modular iff each factor is. Complete iff each factor is.

**Categorical characterization.** The product lattice is the categorical product in the category of lattices (with lattice homomorphisms as morphisms). Universal property: for any lattice `M` and homomorphisms `f_i : M → L_i`, there's a unique homomorphism `M → ∏ L_i` whose composition with the i-th projection is `f_i`.

**Example.** `Cl × Cat` where `Cl` is the chain `Unclassified ≤ Confidential ≤ Secret ≤ TopSecret` and `Cat` is `(P(C), ⊆)` for a set `C` of compartments. The product is the canonical "level + compartment" classification lattice.

`{0,1}^n` is the n-fold product of the 2-element lattice — it's the n-cube, a Boolean lattice.

**Non-example (looks like product but isn't).** "Two coordinates with a *constraint* between them" — for example, "level + categories, but TS forbids unrestricted compartments." That's a *sublattice* of the product (entry 14), not the full product, and it may fail to be a sublattice if the constraint is not closed under meet and join.

**Citation.** `[davey-priestley-2002]` §1.20; `[burris-sankappanavar-1981]` §II.1; `[nation-lattice-notes]` ch. 4.

**Consultant tags.** `(a)` whenever two genuinely independent dimensions need to compose. `(b)` when the user has an inter-dimension constraint — propose a product first and then identify whether the constraint defines a sublattice or congruence (entry 16). `(c)` when the constraint is *not* compatible with coordinatewise meet/join — the result isn't a lattice at all.

> **When this comes up.** "We have classification level and compartment set; their composition behaves coordinatewise." Confirm: take two pairs and compute meet/join coordinatewise; verify each coordinate is independently a lattice. If any cross-coordinate dependency exists, name it explicitly and check whether the resulting subset is a sublattice (closed under product meet/join) before claiming "lattice."

---

## 12. Sum / disjoint-union lattice

**Definition.** Given lattices `L₁, L₂`, the *sum* (or coproduct in the category of bounded lattices) is constructed by gluing the bottoms and the tops: take the disjoint union, identify `⊥₁ = ⊥₂ = ⊥`, identify `⊤₁ = ⊤₂ = ⊤`, and otherwise keep elements from `L₁` and `L₂` mutually incomparable.

In the category of (unbounded) lattices the coproduct is more subtle and is typically the *free lattice* on `L₁ ⊔ L₂` modulo the lattice operations of `L₁` and `L₂` — generally infinite even when `L₁, L₂` are finite (Whitman, entry 10).

**Disjoint-union (linear sum) variant.** A simpler construction: place `L₁` strictly below `L₂`, with `⊤₁ < ⊥₂` (or possibly `⊤₁ = ⊥₂`). This is the *ordinal sum* `L₁ ⊕ L₂` — a totally vertical concatenation. Modularity, distributivity, completeness all transfer.

**Example.** Two independent classification systems for two non-overlapping document categories, joined only by a global top "Restricted" and a global bottom "Public."

**Non-example.** When the two systems share semantic elements (both know about NOFORN, both know about ORCON), the disjoint-sum construction is dishonest — those elements should be unified, which is a *pushout*, not a coproduct.

**Citation.** `[davey-priestley-2002]` §3.20 (linear sum); `[burris-sankappanavar-1981]` §II.4 (coproducts in varieties); `[nation-lattice-notes]` ch. 4.

**Consultant tags.** `(a)` for genuinely-disjoint composition. `(b)` when the user's structures have overlap — propose a pushout (gluing along a shared sublattice) instead of a coproduct.

> **When this comes up.** "We have two unrelated marking schemes and we want a single lattice." Disjoint sum if there's truly no shared vocabulary. If there's overlap, name the pushout/gluing explicitly — it usually means the user has a shared sub-lattice that needs identification.

---

## 13. Dual lattice

**Definition.** Given a lattice `(L, ∧, ∨, ≤)`, the *dual lattice* (or *opposite lattice*) `L^op` has the same underlying set with the opposite order: `x ≤^op y ⇔ y ≤ x`. Meets and joins swap: `x ∧^op y = x ∨ y` and `x ∨^op y = x ∧ y`. Top and bottom swap: `⊤^op = ⊥` and `⊥^op = ⊤`.

**Duality principle.** Every theorem about lattices has a dual obtained by swapping `≤ ↔ ≥`, `∧ ↔ ∨`, `⊤ ↔ ⊥`. If `P(L)` holds in every lattice, so does `P(L^op)` — in other words, `P^op(L)` holds in every lattice. See `[davey-priestley-2002]` §1.4.

**Properties.** Distributivity is self-dual (the two distributive laws are dual to each other). Modularity is self-dual (Dedekind's law `a ≤ b ⇒ a ∨ (c ∧ b) = (a ∨ c) ∧ b` is unchanged under duality). Boolean is self-dual. Heyting is *not* self-dual — the dual of a Heyting algebra is a *coHeyting* (or "Brouwer") algebra, which has a *subtraction* operator instead of implication.

**Example.** `(P(X), ⊆)` and `(P(X), ⊇)` are dual lattices. They are isomorphic via complementation `S ↦ X ∖ S`, but the isomorphism is *antitone* under `⊆`.

**Where this matters.** *Confidentiality* and *integrity* lattices are duals. A confidentiality lattice has high-classification at the top; an integrity lattice has high-trustworthiness at the top. The Bell-LaPadula confidentiality model and the Biba integrity model are formal duals — a "no read up" rule in one becomes a "no read down" rule in the other. See `[denning-1976]` for the canonical secure-information-flow lattice formulation; `[biba-1977]` for the integrity counterpart.

**Citation.** `[davey-priestley-2002]` §1.4 and ch. 11; `[burris-sankappanavar-1981]` §I.5; `[birkhoff-1967]` ch. I §6.

**Consultant tags.** `(a)` when the user has two related domains with reversed orientation — name the duality explicitly to license transferring theorems. `(b)` when the user is recomputing a result for the dual that already holds by duality from the primal.

> **When this comes up.** "We need a lattice for trustworthiness *and* a lattice for confidentiality, and they look like mirror images." They are formal duals. Prove things on one side and lift to the other; don't duplicate proofs.

---

## 14. Sublattice / convex sublattice

**Definition.** A *sublattice* of a lattice `L` is a subset `S ⊆ L` closed under both `∧` and `∨` (computed in `L`): for all `x, y ∈ S`, `x ∧ y ∈ S` and `x ∨ y ∈ S`. A sublattice with the inherited order is itself a lattice.

A *convex* (or *order-convex*) subset of a poset `P` is a subset `C ⊆ P` such that for all `x, y ∈ C` and `z ∈ P`, `x ≤ z ≤ y ⇒ z ∈ C`. A *convex sublattice* is a subset that is both a sublattice and convex.

**Theorem.** For a subset `S ⊆ L`, the following are equivalent:
1. `S` is a sublattice and order-convex;
2. `S = I ∩ F` for some ideal `I` (down-closed sublattice) and filter `F` (up-closed sublattice).

See `[wikipedia-sublattice]`, `[davey-priestley-2002]` §2.18.

**Inheritance.** A sublattice automatically satisfies idempotence, commutativity, associativity, absorption (inherited from `L`). Distributivity, modularity, completeness need to be checked: a sublattice of a distributive lattice is distributive (distributivity is hereditary); a sublattice of a modular lattice is modular; a sublattice of a complete lattice is *not* automatically complete (e.g., `(ℚ, ≤)` is a sublattice of `(ℝ, ≤)` but not complete).

**Example.** The set of finite subsets of `ℕ` is a sublattice of `(P(ℕ), ⊆)` but not convex (e.g., `∅ ⊆ {1,2,3,…} ⊆ ℕ` but `ℕ` isn't finite). The interval `[a, b] = {x : a ≤ x ≤ b}` in any lattice is a convex sublattice.

**Non-example (closed under one op but not the other).** In `(P(ℕ), ⊆)`, the set `{∅, {1}, {2}, {1,2,3}}` is closed under `∪` (joins are present) but not under `∩` (`{1} ∩ {2} = ∅` — okay, that's there; but `{1,2,3} ∩ {1} = {1}` — okay; actually this *is* a sublattice). A real non-example: `{∅, {1}, {2}, {3}}` — `{1} ∪ {2} = {1,2}` is missing, so not closed under join.

**Citation.** `[davey-priestley-2002]` §2.18, ch. 4 §1; `[burris-sankappanavar-1981]` §I.5; `[nlab-lattice]`.

**Consultant tags.** `(a)` when the user has a subset and wants to know if it inherits lattice structure. `(c)` when closure fails — it's just a subset, not a sublattice; "the lattice structure on this subset" is ill-defined.

> **When this comes up.** "We have a subset of valid markings and want lattice operations on it." Check closure under `∧` and `∨`. Common failure: the meet of two valid markings yields an invalid marking (e.g., meeting two compatible-with-different-policies elements gives a marking that no policy permits). That subset is *not* a sublattice — the operations need to be defined on the parent lattice and the subset lives as a check on top, or the operations need redefinition.

---

## 15. Lattice homomorphism, embedding, isomorphism

**Definition.** A *lattice homomorphism* `f : L → M` is a function preserving meets and joins: `f(x ∧ y) = f(x) ∧ f(y)` and `f(x ∨ y) = f(x) ∨ f(y)` for all `x, y ∈ L`. A homomorphism between bounded lattices that also preserves `⊤, ⊥` is a *bounded-lattice homomorphism*.

A *lattice embedding* is an injective lattice homomorphism. A *lattice isomorphism* is a bijective lattice homomorphism (its inverse is automatically also a lattice homomorphism).

**Properties.** A lattice homomorphism is automatically *order-preserving* (monotone): `x ≤ y` in `L` means `x ∧ y = x`, so `f(x) ∧ f(y) = f(x)`, so `f(x) ≤ f(y)` in `M`. The converse fails: a monotone map is not generally a lattice homomorphism.

A lattice embedding `L ↪ M` makes `f(L)` a sublattice of `M` isomorphic to `L`. Birkhoff's representation theorem (entry 6) gives a canonical embedding of any finite distributive lattice into a power set lattice.

**Example.** The inclusion `Sub(G) ↪ P(G)` (subgroups into all subsets) is monotone but *not* a lattice homomorphism: `H ∨_Sub K = ⟨H ∪ K⟩` (the generated subgroup) generally exceeds `H ∪ K` (the set union).

The complementation map `S ↦ X ∖ S` on `P(X)` is a lattice *anti*-isomorphism between `(P(X), ⊆)` and `(P(X), ⊇)` — order-reversing, swaps `∧ ↔ ∨`. Equivalently, it's an isomorphism `P(X) → P(X)^op`.

**Citation.** `[davey-priestley-2002]` Definition 2.16, §2.20; `[burris-sankappanavar-1981]` §II.6 (homomorphisms in universal algebra); `[planetmath-lattice-hom]`; `[wikipedia-lattice-order]`.

**Consultant tags.** `(a)` when the user defines a structure-preserving map between two lattices and wants to know what theorems transfer. Check both meet and join — a "monotone" map is not enough.

> **When this comes up.** "We map our marking lattice to a normalized form and want to know if the map respects operations." Check `f(x ∧ y) = f(x) ∧ f(y)` and `f(x ∨ y) = f(x) ∨ f(y)`. If only one direction holds, you have a *meet-homomorphism* or *join-homomorphism* (semilattice-only), which is weaker.

---

## 16. Lattice congruence

**Definition.** A *lattice congruence* on `L` is an equivalence relation `θ ⊆ L × L` compatible with both operations: `x θ x' ∧ y θ y' ⇒ (x ∧ y) θ (x' ∧ y') ∧ (x ∨ y) θ (x' ∨ y')`. The *quotient lattice* `L / θ` has equivalence classes as elements, with operations induced from `L` (well-defined exactly because `θ` is a congruence).

The set of congruences on `L` itself forms a complete lattice under inclusion: meet is intersection, join is the congruence generated by union (the smallest congruence containing it). See `[burris-sankappanavar-1981]` §II.5.

**First isomorphism theorem.** For any lattice homomorphism `f : L → M`, the kernel `ker(f) := {(x, y) : f(x) = f(y)}` is a congruence, and `L / ker(f) ≅ image(f)`. Conversely, every congruence arises as the kernel of some homomorphism (e.g., the projection `L → L/θ`).

**Example.** On `(ℤ, +, ·)` (a ring; the lattice example: `(ℤ, gcd, lcm)` ordered by divisibility), the relation "`x ≡ y mod n`" is a congruence — meet (gcd) and join (lcm) are well-defined modulo `n`. The quotient is a finite divisor lattice.

In `(P(X), ⊆)`, the relation "`A △ B is finite`" (symmetric difference is finite) is a lattice congruence (and an equivalence) — quotient gives the lattice of "subsets up to finite difference."

**Non-example.** "`x R y ⇔ |x| = |y|`" on subsets of an infinite set — equivalence relation, but not compatible with `∪`: `{1} R {2}` and `{1} R {3}` but `{1} ∪ {1} = {1}` while `{2} ∪ {3} = {2,3}`, so `({1} ∪ {1}) R ({2} ∪ {3})` requires `1 = 2`, false.

**Citation.** `[davey-priestley-2002]` §6.1, ch. 6 generally; `[burris-sankappanavar-1981]` §II.5; `[gratzer-2011]` ch. III.

**Consultant tags.** `(a)` when the user wants to identify "operationally equivalent" markings and project to a normal form. `(b)` when the user has a partial equivalence (e.g., compatible with meet but not join) — that's a *meet-congruence*, weaker, and the quotient is a meet-semilattice not a lattice.

> **When this comes up.** "We canonicalize markings (e.g., sort tetragraphs) and want operations to factor through the canonical form." That's well-defined iff the canonicalization-equivalence is a lattice congruence — check both meet and join compatibility. Often canonicalization is *not* compatible with one operation and you need to canonicalize *after* the operation, not before.

---

## 17. Galois connection / adjunction

**Definition (monotone form / adjunction).** A *monotone Galois connection* between posets `(P, ≤)` and `(Q, ⊑)` is a pair of order-preserving maps `f : P → Q` and `g : Q → P` such that for all `p ∈ P, q ∈ Q`:

- `f(p) ⊑ q ⇔ p ≤ g(q)`.

In categorical language, `f ⊣ g` (`f` is left adjoint to `g`); `f` preserves all joins and `g` preserves all meets.

**Definition (antitone form / Galois correspondence).** A *Galois connection* in Ore's classical sense `[ore-1944]` consists of order-*reversing* maps `f : P → Q`, `g : Q → P` such that:

- `p ≤ g(f(p))` for all `p ∈ P`,
- `q ⊑ f(g(q))` for all `q ∈ Q`.

The two formulations are interconvertible by replacing `Q` with `Q^op` (entry 13).

**Closure operators on each side.** Either form induces a closure operator: `g ∘ f : P → P` is monotone, extensive (`p ≤ g(f(p))`), and idempotent. Dually `f ∘ g : Q → Q` is also a closure operator (or interior, depending on form). The fixed points of `g ∘ f` and `f ∘ g` are in order-preserving bijection — the *Galois closed elements*. See entry 18.

**Universal example.** Field extension `E ⊇ F`: let `P =` set of intermediate fields between `F` and `E`, `Q =` set of subgroups of `Aut(E/F)`, and `f, g` the maps "fix" and "automorphism group." This is the original Galois connection of Galois theory; the closed elements are exactly the *Galois sub-extensions* `[ore-1944]`.

**Universal example (abstraction-concretization).** In abstract interpretation, an abstraction map `α : Concrete → Abstract` and concretization map `γ : Abstract → Concrete` form a Galois connection `α ⊣ γ`. Soundness of the abstract analysis is exactly the adjunction inequality. See `[cousot-cousot-1977]` and `abstract-interp.md` for the full development.

**Example (formal concept analysis).** Given a context `(O, A, I)` (objects, attributes, incidence relation), the maps `S ↦ S' = {a : ∀o ∈ S, oIa}` and `T ↦ T' = {o : ∀a ∈ T, oIa}` form an antitone Galois connection `(P(O), ⊆) ↔ (P(A), ⊆)`. Closed elements are *formal concepts*.

**Citation.** `[ore-1944]`; `[erne-koslowski-melton-strecker-1993]` (the standard primer; cite-and-link only — see `sources/SOURCES.md` for the author URL); `[davey-priestley-2002]` ch. 7; `[nlab-galois-connection]`; `[wikipedia-galois-connection]`.

**Consultant tags.** `(a)` whenever the user has an "abstraction + concretization" pair, or a "view + materialize" pair, or any antitone-pair-of-monotone-maps. `(a)` for the adjunction laws as soundness conditions in any program-analysis-shaped problem. `(b)` when the user has a single monotone map and wants its "approximate inverse" — propose finding an adjoint.

> **When this comes up.** "We abstract a complex marking into a simpler summary and lift back; what's the formal name?" Galois connection. The adjunction inequality `α(c) ≤ a ⇔ c ≤ γ(a)` is the soundness condition. Composing closures yields a complete lattice of "stable" abstractions — see entry 18 and `abstract-interp.md`.

---

## 18. Closure operator / interior operator

**Definition.** A *closure operator* on a poset `P` is a self-map `c : P → P` satisfying, for all `x, y ∈ P`:

- **Monotone (isotone):** `x ≤ y ⇒ c(x) ≤ c(y)`.
- **Extensive (inflationary):** `x ≤ c(x)`.
- **Idempotent:** `c(c(x)) = c(x)`.

An *interior operator* (or *kernel operator*) `k : P → P` is the dual: monotone, *deflationary* (`k(x) ≤ x`), idempotent.

**Theorem (fixed points form a complete lattice).** Let `c` be a closure operator on a complete lattice `L`. Then the set `Fix(c) = {x ∈ L : c(x) = x}` is itself a complete lattice — meets in `Fix(c)` are inherited from `L`, joins are computed by taking the `L`-join and applying `c`. See `[davey-priestley-2002]` Theorem 7.2; `[wikipedia-closure-operator]`.

**Galois-connection bridge.** Closure operators on `P` are in bijection with Galois connections from `P` to some other poset (entry 17). Concretely, given any monotone Galois connection `f ⊣ g`, the composite `g ∘ f` is a closure operator on the source. Conversely, every closure operator factors as `g ∘ f` where `f` projects to the closed elements and `g` is the inclusion.

**Example.** Topological closure on `P(X)`: `S ↦ S̄` is a closure operator (extensive, monotone, idempotent — Kuratowski axioms). Fixed points are the *closed sets*, forming a complete lattice (intersection arbitrary, finite union, plus `∅` and `X`).

The *generated subgroup* operator on `P(G)`: `S ↦ ⟨S⟩`. Fixed points are subgroups, forming a complete lattice (entry 7 modular).

The *transitive closure* of a relation on `P(X × X)`: extensive, monotone, idempotent.

**Non-example.** A monotone idempotent map that *isn't* extensive: `f(x) = x ∧ a` for fixed `a ≠ ⊤`. Idempotent (`f(f(x)) = (x ∧ a) ∧ a = x ∧ a = f(x)`), monotone, but `x ≤ f(x)` fails for `x = ⊤`. This is a *retraction*, not a closure.

**Citation.** `[davey-priestley-2002]` ch. 7; `[burris-sankappanavar-1981]` §II.2; `[erne-koslowski-melton-strecker-1993]`; `[wikipedia-closure-operator]`; `[nlab-closure-operator]`.

**Consultant tags.** `(a)` when the user wants to "compute a normalized form" and the operation should idempotently saturate. `(b)` when the user has an extensive monotone map but it's not idempotent — iterate until fixed (use Kleene, entry 20) and the limit is a closure operator.

> **When this comes up.** "We expand markings to include all transitively-implied markings." That's a closure operator if (a) extensive (only adds, never removes), (b) monotone (more inputs → more outputs), (c) idempotent (saturates after one application). Verify each property separately; non-idempotent rule applications need a fixed-point iteration.

---

## 19. Knaster-Tarski theorem

**Statement.** Let `L` be a *complete lattice* and `f : L → L` a *monotone* map. Then:

1. The set of fixed points `Fix(f) = {x ∈ L : f(x) = x}` is non-empty.
2. `Fix(f)` is itself a complete lattice (with order inherited from `L`).
3. The *least fixed point* is `μf = ⋀ {x ∈ L : f(x) ≤ x}` (the smallest pre-fixed point).
4. The *greatest fixed point* is `νf = ⋁ {x ∈ L : x ≤ f(x)}` (the largest post-fixed point).

Critically, `f` need not be continuous — only monotone. Continuity (entries 20, 23) gets you the Kleene-iteration construction; Knaster-Tarski proves existence without it.

**Proof sketch.** Let `M = {x : f(x) ≤ x}`. Define `μ = ⋀ M`. Then `μ ∈ L` since `L` is complete. Show `f(μ) ≤ μ` (`μ ∈ M`) and `μ ≤ f(μ)` (using monotonicity), so `f(μ) = μ`. Dually for `ν`. The fixed points form a complete lattice by a similar inheritance argument. See `[davey-priestley-2002]` Theorem 8.20; `[wikipedia-knaster-tarski]`; `[nlab-knaster-tarski]`.

**History.** Knaster proved the case for power-set lattices in 1928; Tarski generalized to arbitrary complete lattices in `[tarski-1955]`. Bourbaki and others have given alternate proofs.

**Example.** The reachability of states in a transition system: `f(S) = S ∪ {y : ∃x ∈ S, x → y}`. Monotone, on the complete lattice `(P(States), ⊆)`. The least fixed point starting from initial states is the reachable set; the greatest fixed point of a different map (the "co-reachability" / "safety") gives the largest invariant.

The recursive equation `X = E ∪ f(X)` for `f` monotone has lfp = "smallest set closed under `f` containing `E`" — this is the basic recipe for inductively-defined sets.

**Counterexamples to "lfp exists" without completeness.** On `(ℕ, ≤)`, `f(n) = n + 1` is monotone, has no fixed point. The lattice isn't complete (`ℕ` itself has no upper bound in `ℕ`). Add a top element `∞` and `f(∞) = ∞` is the lfp.

**Citation.** `[tarski-1955]` (the canonical statement); `[davey-priestley-2002]` Theorem 8.20; `[wikipedia-knaster-tarski]`; `[nlab-knaster-tarski]`; `[nation-lattice-notes]` ch. 2.

**Consultant tags.** `(a)` for any "smallest set closed under these rules" or "largest set satisfying these constraints" question — the canonical lfp/gfp construction.

> **When this comes up.** "Does this recursive definition terminate?" or "Is there a unique smallest/largest object satisfying these closure rules?" If the underlying set forms a complete lattice and the rule is monotone, Knaster-Tarski guarantees the existence of lfp and gfp. Termination as a *computation*, however, requires Kleene continuity (entry 20).

---

## 20. Kleene fixed-point theorem

**Statement.** Let `(L, ⊑)` be a *directed-complete partial order* (dcpo) with a least element `⊥`, and `f : L → L` *Scott-continuous* (preserves directed suprema). Then `f` has a least fixed point computed as:

- `μf = ⨆_n f^n(⊥)`,

where the chain `⊥ ⊑ f(⊥) ⊑ f(f(⊥)) ⊑ …` is the *ascending Kleene chain*.

Scott continuity is strictly stronger than monotonicity: a Scott-continuous function is monotone, but a monotone function on a dcpo need not be Scott-continuous. (Continuity demands preservation of *directed* suprema; monotonicity demands only preservation of order.)

**Compared to Knaster-Tarski.**

| | Knaster-Tarski | Kleene |
|---|---|---|
| Lattice | Complete lattice | dcpo with `⊥` |
| Operator | Monotone | Scott-continuous |
| Result | lfp exists | lfp exists *and* equals the chain supremum |
| Computation | Non-constructive (impredicative meet) | Iterate from `⊥` |

For *finite-height* lattices (e.g., subsets of a finite set), every monotone function is automatically Scott-continuous, and the Kleene chain stabilizes after finitely many steps — this is the practical case for most program-analysis and rule-iteration shapes.

**Example (program analysis).** Forward dataflow analysis: the abstract state at each program point is computed by iterating the transfer function until stable. On a finite-height abstract domain, Kleene termination is guaranteed by finite ascending chain length. Widening operators are needed for infinite-height domains where naive iteration may not terminate.

**Example (rule application).** Rule rewriting of a marking until no more rules fire. If each rule is monotone (only adds information) and the marking lattice has finite height, Kleene iteration terminates.

**Counterexample to Kleene without continuity.** On a dcpo with `⊥`, define `f(⊥) = a`, `f(x) = ⊤` for `x ≠ ⊥`, where `a` is below the supremum of the directed set `{a_n}` whose join is `⊤`. Then `f(⨆ a_n) = ⊤ ≠ ⨆ f(a_n) = a` if `f(a_n) = a` for all finite `n`. Monotone but not Scott-continuous; Kleene chain stabilizes at the wrong place.

**Citation.** `[davey-priestley-2002]` ch. 8; `[abramsky-jung-handbook]`; `[wikipedia-kleene-fixed-point]`; `[nlab-kleene-fixed-point]`; `[cousot-cousot-1977]` for the abstract-interpretation application.

**Consultant tags.** `(a)` for any iteration-to-fixed-point on a finite-height domain. `(b)` when the height is infinite — propose widening or chain-acceleration techniques. `(c)` when Scott continuity fails — naive iteration may converge to the wrong limit.

> **When this comes up.** "We iterate this rule until stable. Does it terminate? Does it reach the right limit?" If the lattice is finite-height and the rule is monotone, yes to both — Kleene iteration in disguise. If the lattice is infinite-height, you need either widening (abstract interpretation) or a stronger termination argument (well-foundedness, entry 22).

---

## 21. Chain / antichain / Dilworth's theorem

**Definitions.** In a poset `P`:

- A *chain* is a subset totally ordered by `≤`: any two elements are comparable.
- An *antichain* is a subset of pairwise incomparable elements.
- The *width* of `P` is the supremum of antichain cardinalities.
- The *height* of `P` is the supremum of chain cardinalities (often counted by length, i.e., one less than cardinality, in the literature).

**Dilworth's theorem.** In any finite poset `P`, the width equals the minimum number of chains needed to partition `P`. So: maximum antichain size = minimum chain decomposition. See `[dilworth-1950]`.

**Mirsky's theorem (dual).** The height equals the minimum number of antichains needed to partition `P`. See `[mirsky-1971]`.

**Example.** Subsets of `{1,2,3}` ordered by inclusion: width = 3 (the antichain `{{1}, {2}, {3}}` or `{{1,2},{1,3},{2,3}}` — actually 3 is the right answer here — both have size 3). Height = 4 (chain `∅ ⊂ {1} ⊂ {1,2} ⊂ {1,2,3}`). The boolean lattice `2^n` has width `(n choose ⌊n/2⌋)` (Sperner's theorem) and height `n+1`.

**Why it matters in marque shapes.** When a user asks "what's the longest sequence of markings escalating in restrictiveness?" the answer is the *height* — and the chain in question is a `≤`-chain in the marking lattice. When they ask "how many independent classification dimensions do we need to encode this?" they're asking about the *width* and the minimum chain decomposition.

**Citation.** `[dilworth-1950]` (original, `Annals of Mathematics` 51); `[mirsky-1971]`; `[wikipedia-dilworth-theorem]`; open lecture notes: Inkulu (IIT Guwahati) at `https://www.iitg.ac.in/rinkulu/note/dilwposets-note.pdf`; CMU Math at `https://www.math.cmu.edu/~af1p/Teaching/Combinatorics/F03/Class14.pdf`.

**Consultant tags.** `(a)` for poset combinatorics questions about maximum/minimum chain or antichain. `(b)` when the user wants to "decompose" a complex marking lattice — Dilworth gives a principled decomposition into the minimum number of totally-ordered chains.

> **When this comes up.** "What's the longest escalation sequence?" That's height. "How many independent axes do we need?" That's width / minimum chain decomposition. Dilworth's theorem gives an existence proof; for finite lattices the constants are computable.

---

## 22. Well-founded order / Noetherian poset

**Definition.** A poset `(P, ≤)` is *well-founded* if there is no infinite *strictly descending* chain `x_0 > x_1 > x_2 > …`. Equivalently (under dependent choice), every non-empty subset has a *minimal* element. The dual notion — no infinite strictly *ascending* chain — defines a *Noetherian* poset (also called *upward well-founded*); rewriting-systems literature simply calls this "terminating."

A *well-order* is a total order that is well-founded — every non-empty subset has a least element. Examples: `(ℕ, ≤)`, ordinal numbers.

**Connection to lattice machinery.** Every Noetherian poset has the *ascending chain condition* (ACC): every ascending chain `x_0 ≤ x_1 ≤ …` eventually stabilizes. Every monotone function on a Noetherian lattice with bottom converges in finitely many Kleene iterations (entry 20). This is the practical motivation: ACC is the abstract condition under which fixed-point iteration *as a computation* terminates.

**Dually**, *descending chain condition* (DCC) — every descending chain stabilizes — is equivalent to well-foundedness. DCC enables Noetherian induction: to prove `P(x)` for all `x`, show `P(x)` follows from `P(y)` for all `y < x`. This is the abstract form of structural induction, and works on any well-founded order.

**Example.** `(ℕ, ≤)` has DCC (every chain `n_0 > n_1 > …` is finite) but not ACC — `0 < 1 < 2 < …` is an infinite ascending chain. So `ℕ` is well-founded but not Noetherian.

`(ℕ, ≥)` (reversed) has ACC but not DCC — Noetherian but not well-founded.

Subsets of a finite set ordered by inclusion: both ACC and DCC, since the poset is finite.

The lattice of ideals in a Noetherian ring (e.g., `ℤ[x]`): ACC by definition of Noetherian.

**Non-example.** `(ℝ, ≤)`: neither ACC nor DCC — `1, 1/2, 1/4, …` descends infinitely, `0, 1, 2, …` ascends infinitely.

**Why it matters.** Without DCC or ACC, Kleene iteration (entry 20) may not terminate even when Knaster-Tarski guarantees a fixed point exists. *Existence* of an lfp is one thing; *computing* it via iteration is another, and that requires a chain condition.

**Citation.** `[davey-priestley-2002]` §2.30, ch. 8; `[burris-sankappanavar-1981]` §VIII.2; `[wikipedia-well-founded-relation]`; `[wikipedia-acc]`.

**Consultant tags.** `(a)` for any "does this iteration terminate?" question. `(c)` when the user has an unbounded-height domain — name the concern explicitly; Knaster-Tarski says lfp exists, but Kleene iteration may not reach it.

> **When this comes up.** "Will this rewrite loop terminate?" If the rewrite is monotone-decreasing under some lattice order with DCC, yes (Noetherian induction). If the order isn't well-founded (e.g., `ℝ` or `ℚ`), naive iteration may not terminate even when a fixed point exists. Different framework needed: convergence in some metric, or widening operators in abstract interpretation.

---

## 23. CPO / dcpo / Scott topology

(This is a deliberately minimal sketch; the deep treatment is in `abstract-interp.md`.)

**Definitions.**
- A *directed* set in a poset `P` is a non-empty subset `D ⊆ P` such that every two elements of `D` have an upper bound *in* `D`.
- A *directed-complete partial order* (*dcpo*) is a poset in which every directed subset has a supremum.
- A *complete partial order* (*CPO*) is a dcpo with a bottom element `⊥`. (Some authors require *ω-completeness* — sup of every ascending ω-chain — instead of full directed completeness; most standard accounts use the dcpo definition.)
- A function `f : P → Q` between dcpos is *Scott-continuous* if it preserves directed suprema: for every directed `D ⊆ P`, `f(⨆ D) = ⨆ f(D)`. Scott continuity implies monotonicity but is strictly stronger.
- The *Scott topology* on a dcpo has open sets `U` that are (a) up-closed and (b) inaccessible by directed suprema (if `⨆ D ∈ U` then some `d ∈ D` is in `U`). Continuity with respect to the Scott topology coincides with Scott continuity defined order-theoretically.

**Why it matters.** Domain theory uses dcpos as the standard semantic universe: function spaces between dcpos are dcpos, and recursive definitions are interpreted as least fixed points (Kleene, entry 20). The denotational semantics of programming languages — including λ-calculus and recursive types — is built on this.

**Citation.** `[abramsky-jung-handbook]`; `[davey-priestley-2002]` ch. 8 §3; `[wikipedia-scott-continuity]`; `[wikipedia-cpo]`. For the consultant's purpose of marque-shape questions, `abstract-interp.md` is the better reference.

**Consultant tags.** `(a)` rarely on its own — usually subsumed by the "complete lattice" or "finite-height" cases that dominate marque shapes. Refer to `abstract-interp.md` for fixed-point computation in program-analysis-shaped problems.

> **When this comes up.** Almost never directly in marque shapes — most marking lattices are finite, hence trivially complete and Scott-continuous-monotone-equivalent. The CPO / dcpo machinery is needed when the domain is genuinely infinite-height (e.g., recursive type systems, infinite-state symbolic execution); refer the user to `abstract-interp.md` and the domain-theory sources cited there.

---

## Common pitfalls — diagnosing whether something is actually a lattice

When a user describes a candidate construction informally, run through this checklist before granting "yes, it's a lattice":

### 1. Is it even a poset?

Check reflexivity, antisymmetry, transitivity. Many "almost-lattices" turn out to be preorders (fail antisymmetry — equivalent elements not identified) or DAGs with cycles (fail antisymmetry differently). A preorder can be quotiented by its symmetric kernel to yield a poset; a DAG with cycles may need topological sorting or strongly-connected-component collapse first. If the construction can't be turned into a poset, it can't be turned into a lattice.

### 2. Idempotence is easy to forget when ops are folds.

If `meet` is implemented as `let m = elements.fold(top, |a, b| min(a, b))`, idempotence holds *only* if `min(a, a) = a` for all `a`. Sounds trivial — but if the underlying type has structural duplication semantics (e.g., a multiset rather than a set; a list of "tags" that double-counts; a counter that increments on each application), idempotence fails silently. Test explicitly: `meet(a, a) == a` for representative `a`.

### 3. Absorption fails silently when meet and join are defined independently.

Define `meet` and `join` as separate operations, never check absorption: a common bug. The two operations must satisfy `x ∧ (x ∨ y) = x` and `x ∨ (x ∧ y) = x`. The cleanest way to ensure this: define the order `≤` first, derive `∧` and `∨` from the order, and test commutativity / associativity / idempotence as inherited. Test absorption by hand on small cases; if it fails, you have *two semilattices* sharing a domain, not a lattice.

### 4. "Every pair has a meet" ≠ "all subsets have a meet."

A lattice is the binary case; a complete lattice is the arbitrary-subset case. They differ on infinite domains. `(ℕ, ≤)` has every binary meet (`min`) and every binary join (`max`), but the join of `ℕ` itself doesn't exist in `ℕ` — so it's a lattice but not a complete lattice. Knaster-Tarski (entry 19) needs *complete*; Kleene (entry 20) needs dcpo. Don't apply infinitary theorems to merely-binary lattices.

### 5. Open / agency-extensible domains have no top.

When a vocabulary can be extended by an outside party (new compartments, new partner-national codes, new tags), there is *no top element* — the supremum over a possibly-extended set isn't well-defined within the current vocabulary. Such structures are at best *join-semilattices*, possibly with bottom (`empty set` is well-defined; `everything` is not). Calling them *bounded* lattices is a category error and downstream code that relies on `top()` will produce nonsense for extensions.

### 6. Equal-depth meet policies are usually quotients or products in disguise.

When you have a multi-level structure (e.g., compartments with sub-compartments) and you "intersect at every level but want to preserve depth," check whether you're describing:
- A *product* of lattices (one per level), where `meet` is coordinatewise — this is entry 11.
- A *quotient* of some larger lattice by an equivalence that identifies "structurally equivalent at the same depth" — this is entry 16.
- A *closure operator* that re-canonicalizes after each operation — this is entry 18.

Naming the construction matters: each of those choices has different theorems available (e.g., distributivity transfers through products iff each factor is distributive; quotients of a distributive lattice need *not* be distributive in general).

### 7. The "absorption" of multi-valued generators is structural.

If a user defines lattice operations on terms in a free algebra without reducing to canonical form, the result may not be a lattice — Whitman's word-problem solution (entry 10) tells them whether two expressions are equal in *every* lattice. If equality requires more than the four lattice laws, the structure isn't a lattice; some additional relation is being assumed implicitly.

### 8. Confidentiality vs. integrity dual confusion.

When a user has both a confidentiality and an integrity model, the orders are *dual* to each other (entry 13). It's tempting to bake them into a single lattice with a single `≤`, but the operations have *opposite* semantics: meet in confidentiality is "least common upper-bound classification" (most-restrictive), while meet in integrity is "least common lower-bound trustworthiness" (least-trusted). Conflating them produces operations that don't respect either model. Use a *product* of the confidentiality lattice and the integrity-lattice-as-dual; or, equivalently, two lattices with two operation sets and an explicit translation.

### 9. "Lattice" used colloquially.

"It's a lattice" is sometimes used to mean "a partially-ordered structure" with no commitment to meets and joins existing. Always pin down: do *all* binary pairs have meets and joins, or only some? If only some, the structure is at most a poset; binary results may not be definable for every input pair.

---

## How to read the consultant tags

Every entry above ends with a short tag indicating which consultant outcome it most often supports:

- **(a) order-theory-adapted approach.** The construction described in the entry IS a typical answer to "what's the right structure for this?" The consultant proposes it directly.
- **(b) pivot toward a known pattern.** The construction is what we'd recommend the user move *toward* when their current design is close-but-not-quite. The consultant names the gap.
- **(c) refuse / redirect.** The entry is sometimes cited to explain *why* a construction isn't a lattice problem at all — distributivity-failure as a structural obstacle, well-foundedness as a termination concern, open vocabulary as a no-top obstacle.

A single entry can support multiple modes; that's the design intent. Most entries are dual-use:
- **Lattice (3), Bounded lattice (4), Complete lattice (5)** — primary `(a)` answers; `(c)` when the user's structure fails the laws.
- **Distributive (6), Modular (7), Boolean (8), Heyting (9)** — `(a)` when you need the property; `(b)` when one is true but the user wants the stronger; `(c)` when the structure embeds an obstruction (M3 / N5).
- **Galois connection (17), closure/interior (18), Knaster-Tarski (19), Kleene (20)** — almost always `(a)` for the abstraction-and-fixed-point shape of question; `(b)` when iteration termination needs a stronger hypothesis (Scott continuity, well-foundedness).
- **Free lattice (10), congruence (16)** — mostly `(b)`, used when explaining why a candidate construction is over- or under-constrained.
- **Well-founded / Noetherian (22)** — `(c)` is the key tag here: when iteration termination is in question and the lattice doesn't have ACC/DCC, the consultant should *not* promise convergence.

The marque-specific question shapes that route to each entry are codified in `marque-applied.md` (Agent E). This file gives the algebraic surface; that one gives the marque-flavored translation.

---

*End of catalog.*
