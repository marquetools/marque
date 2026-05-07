# Universal Algebra — Catalog Reference

**Audience.** Claude, in lattice-consultant mode, scanning for whether a marque structure satisfies a particular algebraic law, what variety it lives in, and what to call it when it almost-but-not-quite forms a lattice.

**Why this file exists.** Universal algebra gives you the formal vocabulary for the question "does this structure satisfy law `L`?" When marque has an `(M, ∨, ∧)` algebra and the question is "is this a lattice?" or "is this a distributive lattice?" or "what should I call this if it's missing a top?" — universal algebra is where you get the precise answer. The centerpiece is the *almost-lattice diagnostic* (entry 11), which agents calling into the consultant rely on to label structures correctly.

**Companion files.**
- `pure-lattice.md` (Agent A) — the lattice catalog. This file's relationship to it: pure-lattice gives the *named structures*; universal-algebra gives the *machinery for diagnosing which structure you actually have*.
- `frames-locales.md` (sibling) — when the structure is a complete lattice and the question is whether the meet distributes over arbitrary joins.
- `abstract-interp.md` (Agent C) — fixed-point machinery on lattices.
- `marque-applied.md` (Agent E) — translates these diagnostics into marque-specific design questions.

---

## Table of Contents

1. [Algebra (in the universal-algebra sense)](#1-algebra-in-the-universal-algebra-sense) — set with operations of stated arities; the free-est of all formalisms.
2. [Signature](#2-signature) — the data of an algebraic theory before identities are imposed.
3. [Variety](#3-variety) — class of algebras satisfying a fixed set of identities; closed under HSP.
4. [Birkhoff's HSP theorem](#4-birkhoffs-hsp-theorem) — the algebraic characterization of varieties.
5. [Free algebra in a variety](#5-free-algebra-in-a-variety) — generators with no relations beyond the variety axioms.
6. [Identities and equational theory](#6-identities-and-equational-theory) — what set of equations axiomatizes a variety.
7. [Term algebra and the word problem](#7-term-algebra-and-the-word-problem) — when expression-equivalence is decidable; Whitman's solution for free lattices.
8. [Quasi-variety vs. variety](#8-quasi-variety-vs-variety) — when implications matter; brief.
9. [Subdirect product / subdirect irreducibility](#9-subdirect-product--subdirect-irreducibility) — Birkhoff's structural decomposition.
10. [Congruence lattice](#10-congruence-lattice) — every algebra has one; for distributive lattices it's well-behaved.
11. [The "almost-lattice" diagnostic](#11-the-almost-lattice-diagnostic) — what to call a structure missing a law; the centerpiece.
12. [Quotient algebra by a congruence](#12-quotient-algebra-by-a-congruence) — when a coarser equivalence yields a cleaner structure.
13. [Direct product, subdirect product, ultraproduct](#13-direct-product-subdirect-product-ultraproduct) — the three composition operations.
14. [Polynomial functions vs. term functions on a lattice](#14-polynomial-functions-vs-term-functions-on-a-lattice) — what counts as "expressible by `∨` and `∧`."
15. [Equational reasoning](#15-equational-reasoning) — using identities mechanically.
16. [Modular lattices and the M3-N5 theorem](#16-modular-lattices-and-the-m3-n5-theorem) — the classical sub-lattice characterization, in equational form.
17. [Distributive lattices and Stone duality](#17-distributive-lattices-and-stone-duality) — the duality, generalized.
18. [Order-sorted / many-sorted algebras](#18-order-sorted--many-sorted-algebras) — when operations are typed.
19. [Diagnosis: "What variety does this structure live in?"](#19-diagnosis-what-variety-does-this-structure-live-in) — flowchart-style test for marque structures.

[How to read the consultant tags](#how-to-read-the-consultant-tags)

---

## 1. Algebra (in the universal-algebra sense)

**Definition.** An *algebra* (universal-algebra sense) is a pair `A = (A, F^A)` where:

- `A` is a non-empty set, the *universe* (or *carrier*) of the algebra.
- `F` is a *signature*: a set of operation symbols with stated arities (entry 2).
- `F^A` is the *interpretation*: for each `f ∈ F` of arity `n`, a function `f^A : Aⁿ → A`.

A nullary operation (arity 0) is a constant of `A`. A unary operation is a function `A → A`. A binary operation is `A × A → A`. The arity tells you how many arguments the symbol takes. Multiple operations can share a name across algebras only if their arities match.

**Example.** A *lattice* in the universal-algebra sense is an algebra `(L, ∨, ∧)` where `∨` and `∧` are binary operations satisfying the four lattice identities (idempotence, commutativity, associativity, absorption — see `pure-lattice.md` entry 3). A *bounded lattice* has signature `(∨, ∧, ⊤, ⊥)` with two binary and two nullary operations. A *Boolean algebra* has signature `(∨, ∧, ¬, ⊤, ⊥)` with the additional unary complement.

**Example (group).** `(G, ·, e, ⁻¹)` — a binary `·`, nullary `e`, unary `⁻¹`. Three operations.

**Crucial point about identity-of-signature.** Two algebras of "the same kind" must agree on signature. A meet-semilattice `(M, ∧)` and a meet-semilattice-with-top `(M, ∧, ⊤)` are algebras of *different* signatures, even if the second is "the first with extra structure." The choice of signature determines what counts as a homomorphism, what counts as a subalgebra, and what variety the algebra lives in. Choosing the right signature is a design decision, not bookkeeping.

**Citation.** `[burris-sankappanavar-1981]` Definition II.1.1 and Definition II.1.2; `[bergman-2011]` ch. 1; `[gratzer-1979]` §1.

**Consultant tags.** `(a)` baseline — the question "is this an algebra?" is usually trivially yes. The interesting part is which signature, which identities.

> **When this comes up.** A user describes a structure with operations and asks "is this a lattice?" The first question is "what's the signature?" — pinning down which operations are part of the structure (and which are derived) tells you what category of algebras the structure even *belongs* to before you ask which variety inside that category.

---

## 2. Signature

**Definition.** A *signature* `F` is a set of operation symbols, each tagged with a non-negative integer arity. Formally, `F = ⋃ₙ Fₙ` where `Fₙ` is the set of arity-`n` operation symbols. A *type* (older terminology used by `[burris-sankappanavar-1981]`) is essentially the same thing.

**Examples for marque-relevant structures.**

| Structure | Signature |
|---|---|
| Semigroup | `(·)` — one binary |
| Monoid | `(·, e)` — one binary, one nullary |
| Group | `(·, e, ⁻¹)` — one binary, one nullary, one unary |
| Meet-semilattice | `(∧)` — one binary |
| Join-semilattice | `(∨)` — one binary |
| Bounded meet-semilattice | `(∧, ⊤)` |
| Bounded join-semilattice | `(∨, ⊥)` |
| Lattice | `(∨, ∧)` — two binary |
| Bounded lattice | `(∨, ∧, ⊤, ⊥)` |
| Distributive lattice | same as lattice; the distributive law is an *identity* on the same signature |
| Heyting algebra | `(∨, ∧, ⇒, ⊤, ⊥)` — three binary, two nullary |
| Boolean algebra | `(∨, ∧, ¬, ⊤, ⊥)` |

**Subtle point: same signature, different variety.** Lattices, distributive lattices, and modular lattices share the signature `(∨, ∧)`. They differ only in which identities they satisfy. The signature determines *vocabulary*; the variety determines *meaning*.

**Citation.** `[burris-sankappanavar-1981]` Definition II.1.1; `[bergman-2011]` ch. 1; `[nlab-variety-of-algebras]`.

**Consultant tags.** `(a)` — most consulting questions reduce to picking the right signature and reading off which variety the user's identities pin down.

> **When this comes up.** A user has, say, `Lattice` and `BoundedLattice` traits in code and is unsure which to implement. The signature question gives a clean answer: if the user's domain has a meaningful `⊤`, sign for `BoundedLattice`; if not, sign for `Lattice`. Forcing a `BoundedLattice` impl on a domain without a true top is a category error (see `pure-lattice.md` entry 4).

---

## 3. Variety

**Definition.** A *variety* of algebras is a class of algebras of a fixed signature `F` that is closed under:

- **H** (homomorphic images): if `A` is in the class and `α : A → B` is a surjective homomorphism, then `B` is in the class.
- **S** (subalgebras): if `A` is in the class and `B ⊆ A` is closed under all operations of `F`, then `B` (with the inherited operations) is in the class.
- **P** (direct products): if `(Aᵢ)ᵢ∈I` is a family in the class, then `∏ᵢ Aᵢ` is in the class.

Equivalently — and this is the deeper characterization — a variety is the class of all algebras of signature `F` satisfying a fixed set `Σ` of identities (entry 6); see `[burris-sankappanavar-1981]` Theorem II.11.9, the celebrated *Birkhoff variety theorem*.

**Examples.**

- The class of all groups (signature `(·, e, ⁻¹)`, identities for associativity, identity, inverse). Variety.
- The class of all Abelian groups. Variety.
- The class of all lattices (signature `(∨, ∧)`, four lattice identities). Variety.
- The class of all distributive lattices. Sub-variety of lattices, defined by adding the distributive identity.
- The class of all Boolean algebras. Variety.
- The class of all *finite* groups. NOT a variety — direct products of finite groups can be infinite, so closure under `P` fails.
- The class of all *fields*. NOT a variety — the multiplicative inverse is undefined at zero, which can't be expressed equationally without partial operations.

**Why this matters for marque.** When a user has a structure they think is a lattice, they're claiming membership in a variety. Membership has consequences: HSP closure means *every* sub-structure, every quotient, every product is also a lattice. If their structure is genuinely a lattice, those derived structures get the same theorems for free. If membership is wrong — if, say, their "meet" doesn't satisfy absorption — then derived structures won't be lattices either, and any theorem the user invokes about lattices may not apply.

**Citation.** `[burris-sankappanavar-1981]` Definition II.9.3 and Theorem II.11.9; `[bergman-2011]` ch. 4; `[nlab-variety-of-algebras]`.

**Consultant tags.** `(a)` baseline — most marque consulting routes through the question "what variety is this in?"

> **When this comes up.** A user wants to invoke a theorem ("every distributive lattice has property P") and the consultant needs to verify the user's structure is in the variety being quantified over. If the structure is *not* a distributive lattice — even though it shares signature with one — the theorem doesn't apply, and the user needs either a different theorem or a different structure.

---

## 4. Birkhoff's HSP theorem

**Theorem (Birkhoff 1935; Tarski's restatement is what's usually cited).** A class `K` of algebras of signature `F` is a variety iff `K` is closed under homomorphic images, subalgebras, and direct products. In symbols:

`K` is a variety  ⇔  `K = HSP(K)`,

where `HSP(K)` is the smallest class containing `K` and closed under `H`, `S`, and `P`. The `=`-formulation is in `[burris-sankappanavar-1981]` Theorem II.9.5 (citing Tarski 1946); the equational characterization (every variety is the class of models of some set of identities) is `[burris-sankappanavar-1981]` Theorem II.11.9.

**Two-line consequence.** A class is a variety iff it is *equationally definable*. So the only way to define a class of algebras as a variety is by writing down a set of identities; conversely, any set of identities cuts out a variety.

**Two-line proof sketch.** The non-trivial direction (HSP-closed implies equationally definable) goes by constructing the *free algebra* on a generating set in the would-be variety as a quotient of the term algebra by the equational closure; see entry 5 and `[burris-sankappanavar-1981]` ch. II §11. The other direction (equationally definable implies HSP-closed) is straightforward: identities are preserved by `H`, `S`, and `P`.

**Why this matters for marque.** When the user wants to "extend" a lattice with a new operation while still having the laws hold, the right question is: does adding the new operation keep the structure inside the variety? If the new operation is a *term operation* — expressible by composition of `∨` and `∧` — yes (entry 14). If it's a *new primitive*, the user has changed signature and is asking about a different variety.

**Citation.** `[birkhoff-1935]` (original); `[burris-sankappanavar-1981]` Theorems II.9.5 and II.11.9; `[bergman-2011]` ch. 4 §4.4; `[mckenzie-mcnulty-taylor-1987]`.

**Consultant tags.** `(a)` when verifying that a sub-construction inherits the laws from the parent variety. `(c)` when a user claims a class is a variety but it's not closed under one of `H`, `S`, `P` (e.g., "the class of finite lattices") — name the failure.

> **When this comes up.** "I want my new construction to satisfy the same laws as the parent lattice." HSP closure tells you when this is automatic (the new construction is built from `H`, `S`, `P` of the parent) and when you need to verify the laws explicitly (you're outside HSP closure).

---

## 5. Free algebra in a variety

**Definition.** Let `V` be a variety of signature `F` and let `X` be a set of generators. The *free algebra* `F_V(X)` in `V` over `X` is an algebra of `V` together with a function `η : X → F_V(X)` such that for any algebra `A ∈ V` and any function `f : X → A`, there is a *unique* homomorphism `f̄ : F_V(X) → A` extending `f` (i.e., `f̄ ∘ η = f`).

Concretely, `F_V(X)` is the term algebra `T(X)` (entry 7) modded out by the smallest congruence forcing the variety identities to hold. Elements of `F_V(X)` are equivalence classes of `(F, X)`-terms under provable-from-identities equality.

The universal property says `F_V(X)` is the algebra of "formal expressions in generators `X`" with no relations beyond what `V`'s identities force.

**Example.** The free Boolean algebra on `n` generators has `2^(2ⁿ)` elements. For `n = 1` (one generator `x`): `{⊥, x, ¬x, ⊤}`, four elements. For `n = 2` (`x` and `y`): `2^4 = 16` elements (all truth-table outputs of two variables).

The free *lattice* on `n` generators is *infinite* for `n ≥ 3`. Whitman proved (`[whitman-1941a]`) that the word problem for free lattices is solvable; see entry 7 for Whitman's algorithm. For `n = 1`: `{x}`. For `n = 2`: `{x, y, x ∨ y, x ∧ y, x ∨ y ∨ (x ∧ y) = x ∨ y, x ∧ (x ∨ y) = x, ...}` — using absorption, this collapses to four distinct expressions: `{x, y, x ∨ y, x ∧ y}`. For `n = 3`: countably infinite. For `n ≥ 3` in distributive lattices: also countably infinite but structurally tractable (the free distributive lattice on `n` generators has the Dedekind-number-many elements; see `[davey-priestley-2002]` §8.18).

**Free Boolean algebra on infinite generators.** Exists; sizes are `2^(2^|X|)` for finite `X`; for infinite `X` more delicate. Free *complete* Boolean algebras don't exist for `|X| ≥ ω` due to size issues — see `[hales-1964]`.

**Citation.** `[burris-sankappanavar-1981]` ch. II §10–§11; `[bergman-2011]` ch. 4; `[freese-nation-lectures]`; `[nlab-variety-of-algebras]`.

**Consultant tags.** `(a)` when a user wants to know "what does my structure look like before I impose any specific values?" The free algebra answer pins down the *generic* structure. `(b)` when the user has a specific concrete algebra and wants to know what equations it satisfies that aren't forced by the variety axioms — the free algebra is the place to compare against.

> **When this comes up.** A user defines a marque construction on top of three abstract compartment-symbols `a`, `b`, `c` and asks "what equalities must hold among arbitrary expressions?" The answer in the free lattice on three generators tells them: the absorption-and-distributive forced equalities, but no others. If their construction satisfies *more* equalities than the free distributive lattice on three generators forces, they've added structure beyond "distributive lattice."

---

## 6. Identities and equational theory

**Definition.** An *identity* (or *equation*) over signature `F` and variables `X` is a pair `(s, t)` where `s, t` are `(F, X)`-terms. We write the pair as `s ≈ t`. The identity *holds* in algebra `A` if for every assignment `X → A`, the two terms evaluate to the same element.

**The lattice identities.**

- L1 (commutativity): `x ∨ y ≈ y ∨ x` and `x ∧ y ≈ y ∧ x`.
- L2 (associativity): `(x ∨ y) ∨ z ≈ x ∨ (y ∨ z)` and `(x ∧ y) ∧ z ≈ x ∧ (y ∧ z)`.
- L3 (idempotence): `x ∨ x ≈ x` and `x ∧ x ≈ x`.
- L4 (absorption): `x ∨ (x ∧ y) ≈ x` and `x ∧ (x ∨ y) ≈ x`.

(Burris-Sankappanavar's Definition I.1.1 lists these; see also `pure-lattice.md` entry 3.)

**The distributive identity (added to lattice identities for distributive lattices).**

- `x ∧ (y ∨ z) ≈ (x ∧ y) ∨ (x ∧ z)`.

(The dual `x ∨ (y ∧ z) ≈ (x ∨ y) ∧ (x ∨ z)` is then derivable from this and L4; see `[burris-sankappanavar-1981]` Theorem I.7.5 — well, the theorem on equivalence of the two distributive forms, classical.)

**The modular identity.**

- `x ≤ y → x ∨ (y ∧ z) ≈ y ∧ (x ∨ z)`. As an unconditional identity over the lattice signature: `x ∨ (y ∧ (x ∨ z)) ≈ (x ∨ y) ∧ (x ∨ z)`. (See `[burris-sankappanavar-1981]` Definition I.7.1.)

**The Boolean identities (added to bounded distributive lattice identities).**

- `x ∨ ¬x ≈ ⊤` (excluded middle).
- `x ∧ ¬x ≈ ⊥` (non-contradiction).

**The Heyting identities (added to bounded distributive lattice identities, for the implication operation `⇒`).**

- `(x ⇒ x) ≈ ⊤`.
- `x ∧ (x ⇒ y) ≈ x ∧ y`.
- `y ∧ (x ⇒ y) ≈ y`.
- `x ⇒ (y ∧ z) ≈ (x ⇒ y) ∧ (x ⇒ z)`.

(See `[davey-priestley-2002]` ch. 11.)

**The equational theory of a class** is the set of all identities holding in *every* member of the class. Two classes have the same equational theory iff they generate the same variety.

**Citation.** `[burris-sankappanavar-1981]` ch. II §11 and §14; `[bergman-2011]` ch. 4; `[davey-priestley-2002]` ch. 11.

**Consultant tags.** `(a)` for confirming a structure satisfies a particular set of identities. `(c)` for refuting — when a user *thinks* their structure is distributive but a single counterexample triple `(x, y, z)` shows the law fails.

> **When this comes up.** "Does my construction satisfy the distributive law?" Pick three concrete elements and compute. If you find a triple where `x ∧ (y ∨ z) ≠ (x ∧ y) ∨ (x ∧ z)`, the structure isn't distributive — and any theorem about distributive lattices doesn't apply. The single counterexample is conclusive.

---

## 7. Term algebra and the word problem

**Definition (term algebra).** Let `F` be a signature and `X` a set of variables. The *term algebra* `T(X)` of signature `F` over `X` is the algebra whose universe is the set of all `(F, X)`-terms — finite trees with internal nodes labeled by operation symbols and leaves labeled by variables in `X` — and whose operations are syntactic: applying `f` to terms `t₁, ..., tₙ` produces the term `f(t₁, ..., tₙ)`.

`T(X)` is the free algebra in the *variety of all algebras of signature `F`* (no identities imposed). For a sub-variety `V` of signature `F`, the free algebra `F_V(X)` is `T(X)` modded out by the smallest congruence respecting `V`'s identities (entry 5).

**Definition (word problem).** Given a finitely presented algebra `A = ⟨G | R⟩` in variety `V` (where `G` is a finite generating set and `R` is a finite set of relations), the *word problem* for `A` asks: given two terms `s` and `t` over `G`, is `s = t` in `A`? See `[burris-sankappanavar-1981]` ch. V §5 (definition of word problem).

The word problem for the *free algebra* `F_V(X)` itself is: given two terms over `X`, is `s = t` provable from `V`'s identities? Solvability of this is equivalent to decidability of the equational theory of `V`.

**Whitman's solution for free lattices.** `[whitman-1941a]` proved the word problem for free lattices is solvable, with an explicit recursive algorithm. *Whitman's condition* (sometimes called Whitman's law): for any elements `a, b, c, d` in a free lattice,

`a ∧ b ≤ c ∨ d`  ⇒  `a ≤ c ∨ d`  or  `b ≤ c ∨ d`  or  `a ∧ b ≤ c`  or  `a ∧ b ≤ d`.

Whitman gave the recursive procedure for deciding `s ≤ t` in the free lattice; equality is `s ≤ t` and `t ≤ s`. The algorithm runs in time polynomial in the term size — see `[freese-nation-lectures]` for the modern treatment.

**Word problem for free distributive lattices.** Solvable; the free distributive lattice on `n` generators is finite for finite `n` (size is the *Dedekind number* `M(n)`, exponential in `n`), and equality reduces to checking the same truth-function-like normal form. See `[davey-priestley-2002]` §8.18.

**Word problem for varieties — general.** Decidable for the variety of all lattices (Whitman), all distributive lattices, all Boolean algebras, all groups (trivially — but not for finitely presented groups; see Novikov-Boone). Undecidable for some varieties: the variety of all modular lattices has an undecidable word problem (`[burris-sankappanavar-1981]` ch. V §5 discusses this — it's a Freese / Hutchinson result; see also `[freese-nation-lectures]`).

**Citation.** `[burris-sankappanavar-1981]` ch. II §10 and ch. V §5; `[whitman-1941a]`, `[whitman-1941b]`; `[freese-nation-lectures]`; `[bergman-2011]` ch. 4.

**Consultant tags.** `(a)` when the user wants to mechanize equality on a lattice expression — pull in Whitman's algorithm. `(c)` when a user assumes equality is decidable in a variety where it isn't (modular lattices); name the obstacle.

> **When this comes up.** A user wants to canonicalize lattice expressions in marque (e.g., normalizing two equivalent rule conditions to the same form). For free distributive lattices the answer is the canonical disjunctive-normal-form. For free general lattices, Whitman's algorithm is the standard solution; cite `[freese-nation-lectures]` for an implementation-friendly account.

---

## 8. Quasi-variety vs. variety

**Definition.** A *quasi-variety* is a class of algebras axiomatized by *quasi-identities*: implications of the form `(s₁ ≈ t₁ ∧ ⋯ ∧ sₙ ≈ tₙ) → s ≈ t` (a finite conjunction of equations implies an equation). Varieties are quasi-varieties (with no antecedent); not every quasi-variety is a variety.

**Equivalent characterization.** A class is a quasi-variety iff it is closed under `I` (isomorphic copies), `S`, and `P_U` (ultraproducts) and contains the trivial algebra; or equivalently iff it is closed under `S`, `P`, and ultraproducts (Mal'cev). See `[bergman-2011]` ch. 5, `[gratzer-1979]`.

**Examples.**

- Cancellative semigroups: closed under `S` and `P` (and obviously `I`), but NOT under `H` — quotients of cancellative semigroups need not be cancellative. So this is a quasi-variety, not a variety.
- Torsion-free Abelian groups: similarly a quasi-variety; quotients can introduce torsion.

**For marque.** Quasi-varieties matter when the user wants to express conditional axioms (e.g., "if `x` and `y` are both compartments, then `x ∧ y` is also a compartment, but I'm not making that claim for arbitrary lattice elements"). Most marque designs sit inside genuine varieties (lattices, semilattices, posets); quasi-varieties are rare. Brief mention only.

**Citation.** `[bergman-2011]` ch. 5; `[gratzer-1979]`.

**Consultant tags.** Mostly `(c)` — flag if a user is reaching for quasi-variety semantics when a variety would do; or vice versa.

> **When this comes up.** A user has a structure axiomatized by implications rather than identities. Note that conditional reasoning loses HSP closure (specifically `H`-closure usually fails), so the structure is more delicate. Often the right move is to find an equational reformulation that recovers full variety status; `[bergman-2011]` ch. 5 has worked examples.

---

## 9. Subdirect product / subdirect irreducibility

**Definition (subdirect product).** An algebra `A` is a *subdirect product* of a family `(Aᵢ)ᵢ∈I` if there is an embedding `A ↪ ∏ᵢ Aᵢ` such that each composition with the projection `πⱼ : ∏ᵢ Aᵢ → Aⱼ` is surjective onto `Aⱼ`.

**Definition (subdirectly irreducible).** An algebra `A` is *subdirectly irreducible* if for every subdirect-product representation `A ↪ ∏ᵢ Aᵢ`, one of the projections `πⱼ : A → Aⱼ` is already an isomorphism. Equivalently — and this is the more useful characterization — `A` is subdirectly irreducible iff its congruence lattice `Con(A)` (entry 10) has a *unique smallest non-trivial congruence* (a unique atom above the trivial congruence). See `[burris-sankappanavar-1981]` Theorem II.8.4.

**Birkhoff's subdirect-product representation theorem.** Every algebra `A` is isomorphic to a subdirect product of subdirectly irreducible algebras (`[burris-sankappanavar-1981]` Theorem II.8.6). Equivalently, every variety is generated by its subdirectly irreducible members (`[burris-sankappanavar-1981]` Corollary II.9.7).

This is the structural decomposition: the SI algebras play the role of "primes" in the variety, and every algebra in the variety is built from them by subdirect product.

**Subdirectly irreducible distributive lattices.** Just `{⊥, ⊤}` (the two-element lattice). So every distributive lattice is a subdirect product of copies of the two-element lattice — equivalently, a subdirect product of `2`'s — which is exactly the statement that distributive lattices are sub-direct products of Boolean algebras. See `[burris-sankappanavar-1981]` ch. IV §1.

**Subdirectly irreducible lattices (general).** There are many; e.g., `M₃` is subdirectly irreducible; `N₅` is subdirectly irreducible. The variety of all lattices is generated by infinite subdirect products of these and others.

**For marque.** Most marque structures are subdirect products of small "atomic" structures: an SCI lattice is roughly the subdirect product of the lattices for each control system; an FGI lattice is roughly the subdirect product per country. This decomposition is sometimes useful for explaining "why the law holds" — if it holds in each subdirectly irreducible factor, it holds in the subdirect product.

**Citation.** `[burris-sankappanavar-1981]` ch. II §8; `[bergman-2011]` ch. 5; `[gratzer-1979]`.

**Consultant tags.** `(a)` when decomposition explains why a law holds. `(b)` when a user wants to "build the lattice from atoms" — this is the right framework, but it has caveats (the irreducibles can be infinite, the embedding need not be onto each factor).

> **When this comes up.** A user wants to argue "this structure is built from these simpler pieces." Subdirect product is the precise language. If the user can identify the subdirectly irreducible members, the rest of the structure follows. For distributive lattices specifically, this is just "every distributive lattice is a subdirect product of `2`'s," which is the algebraic content of Stone duality (entry 17).

---

## 10. Congruence lattice

**Definition.** A *congruence* on an algebra `A` is an equivalence relation `θ ⊆ A × A` that is closed under all operations: for every `n`-ary operation `f` and every choice of pairs `(a₁, b₁), ..., (aₙ, bₙ)` with `aᵢ θ bᵢ`, we have `f(a₁, ..., aₙ) θ f(b₁, ..., bₙ)`. See `[burris-sankappanavar-1981]` Definition II.5.1.

The set of all congruences on `A`, ordered by inclusion of relations, is a complete lattice — the *congruence lattice* `Con(A)`. The bottom is the trivial congruence (only `(a, a)` pairs); the top is the universal congruence (everything related to everything). See `[burris-sankappanavar-1981]` Theorem II.5.3.

**Quotient by a congruence.** Given `A` and `θ ∈ Con(A)`, the quotient `A/θ` has universe the equivalence classes of `θ`; the operations are well-defined precisely because `θ` is a congruence. See entry 12.

**Congruence-distributive variety.** A variety `V` is *congruence-distributive* if `Con(A)` is a distributive lattice for every `A ∈ V`. The variety of all lattices is congruence-distributive; this is one of the deepest classical results in lattice theory (`[burris-sankappanavar-1981]` Theorem II.7 — discussed in the section on Mal'cev conditions). The variety of all groups is NOT congruence-distributive; congruences in groups correspond to normal subgroups, and the lattice of normal subgroups need not be distributive.

**Congruence-modular variety.** A variety where `Con(A)` is always modular (`pure-lattice.md` entry 7). All groups, all rings, all modules — congruence-modular but not congruence-distributive. Lattices, distributive lattices, Heyting algebras — congruence-distributive (a strictly stronger condition).

**For marque.** When a user says "I want to identify some elements of my lattice as 'equivalent'" — say, FGI markings whose actual difference doesn't matter for some downstream rule — they're constructing a congruence. The set of "all valid such identifications" is the congruence lattice. Two facts to use: (1) any quotient by a congruence is again in the same variety (so the resulting structure is again a lattice); (2) for lattices, the congruence lattice is itself distributive, so reasoning about "which equivalence respects the lattice operations" is unusually tractable.

**Citation.** `[burris-sankappanavar-1981]` ch. II §5 and §7; `[gratzer-2011]` ch. III; `[bergman-2011]` ch. 5; `[davey-priestley-2002]` §6.

**Consultant tags.** `(a)` when the user wants to mod out by an equivalence; congruence is the right tool. `(b)` when the user has an equivalence that *isn't* a congruence (doesn't respect operations) and wants to make it one — find the smallest congruence containing it.

> **When this comes up.** "I want to treat some markings as the same for purpose `P`." Verify that the equivalence respects `∨` and `∧` (the compatibility property). If yes, it's a congruence and the quotient is a lattice. If not, the smallest congruence containing the equivalence may collapse more elements than the user wanted; flag the trade-off.

---

## 11. The "almost-lattice" diagnostic

**This is the centerpiece.** When a marque structure looks lattice-shaped but the user isn't sure which laws hold, walk through the diagnostic. Each axis names a structure that's *missing* a particular law from the lattice signature/identities and gives the right name for the structure that remains.

Below, "the structure" refers to whatever the user has handed in: a set `M` with binary operations they're calling `∨` and `∧` and possibly some constants.

### Axis A: Are the operations even total?

**If the operation is partial** (e.g., `x ∧ y` is undefined for some pairs): the structure is a *partial algebra*. Universal-algebra results don't apply directly; partial-algebra theory (`[bergman-2011]` ch. 1, `[gratzer-1979]`) is the right framework, but it's a much weaker toolkit. **Consultant move:** ask the user whether the partiality can be eliminated by adding a "no-op" or `⊤` element, or whether it's essential. If essential, flag that this isn't a lattice and pivot to "poset with partial meet/join" (entry 18 may apply if the partiality is type-driven).

### Axis B: Do the operations satisfy idempotence (`x ∧ x = x`, `x ∨ x = x`)?

**If idempotence fails** (e.g., `x ∧ x ≠ x` for some `x`): the operation is NOT a meet/join in any reasonable sense. It's a binary operation, possibly a semigroup or monoid operation. **Consultant move:** stop calling it `∧`. Identify the operation by what it actually is (if associative and commutative: a commutative semigroup; if also has identity: a commutative monoid; if neither: just a magma). If the operation is "additive-like" (e.g., adds counts), the right algebraic structure is a semigroup or monoid acting on the carrier, and the order structure (if any) is separate. **Mode (c) refusal:** "your `∧` is not a meet."

### Axis C: Do the operations satisfy commutativity (`x ∧ y = y ∧ x`)?

**If commutativity fails:** likewise, the operation is not a meet/join. Could be a non-commutative monoid action; could be a quasigroup operation. **Consultant move:** name the failure, redirect to non-commutative algebra (`[bergman-2011]` ch. 1).

### Axis D: Do the operations satisfy associativity (`(x ∧ y) ∧ z = x ∧ (y ∧ z)`)?

**If associativity fails:** the operation is a *magma*. Even further from lattice-shape. **Consultant move:** verify the user really wants this — non-associative algebraic structures (Lie algebras, loops, quasigroups) are highly specialized and only chosen for specific reasons. If the user "didn't mean for associativity to fail," the bug is in their definition; help fix it.

### Axis E: Do the operations satisfy absorption (`x ∧ (x ∨ y) = x`, `x ∨ (x ∧ y) = x`)?

**If absorption fails but L1–L3 hold:** this is the *most subtle* failure. The structure has two commutative idempotent associative binary operations, but they don't satisfy the absorption laws. The structure is "two semilattices on the same carrier that don't agree on the order." Specifically: from `∧` you derive an order `x ≤_∧ y ⇔ x ∧ y = x`; from `∨` you derive an order `x ≤_∨ y ⇔ x ∨ y = y`. If absorption fails, these two orders DISAGREE — the structure is not a single ordered set with two operations, it's two ordered sets on the same carrier.

**Consultant move:** name this precisely. The structure is "two semilattices with no compatibility between their orders." The user almost certainly wanted the orders to agree, in which case the bug is in `∨` or `∧`. If the user genuinely wants two unrelated semilattices on the same carrier, that's a `(M, ∨, ∧)` algebra without lattice structure — give it a non-lattice name. **Mode (c) refusal with specific repair.** See `[burris-sankappanavar-1981]` Definition I.1.1 — absorption is one of the four lattice laws; without it, no lattice.

### Axis F: Is there a top `⊤`?

**If no top:** the structure is a lattice without a top. This is the "open-vocabulary" failure mode that comes up constantly in marque — `SciSet`, `SarSet`, `FgiSet` are all lattices (or join-semilattices) without a top because the vocabulary is agency-extensible.

**Consultant move:** name it as a "lattice (or semilattice) with bottom but no top" or "join-semilattice with bottom." The right Rust trait is `Lattice` without `BoundedLattice`. Implementing `BoundedLattice::top()` for an open-vocabulary domain is a category error — see `pure-lattice.md` entries 2 and 4, and `marque-applied.md` for the marque-specific recommendation. **Mode (c) refusal:** "you have a lattice; you do not have a bounded lattice; an artificial top would be semantically wrong."

### Axis G: Is there a bottom `⊥`?

**If no bottom:** "lattice with top but no bottom." Less common in marque; typically the empty set is a meaningful bottom. If the user genuinely has no bottom, it's a meet-semilattice with top.

### Axis H: Is the structure complete (arbitrary subsets have meets/joins)?

**If not complete:** lattice but not complete lattice. Common for finite lattices (which are auto-complete) and uncommon for infinite lattices. Open-vocabulary domains are usually NOT complete lattices in any natural sense — even if every fixed instantiation is complete, the "extensible" version is not. **Consultant move:** if the user wants Knaster-Tarski (entry 19 in `pure-lattice.md`), completeness is required. If the structure isn't complete, redirect to Kleene's fixed-point theorem (`pure-lattice.md` entry 20) which only requires Scott-continuity and a CPO.

### Axis I: Is the structure distributive?

**If distributivity fails:** non-distributive lattice. This is *fine* — non-distributive lattices are perfectly good objects of study; modular lattices (`pure-lattice.md` entry 7) are an important class. The failure here is rarely a bug; it's a classification.

**Consultant move:** name the structure as a non-distributive lattice. Don't escalate to "this isn't a lattice." Many marque-shape constructions are non-distributive (the SCI compartment lattice composes in ways that contain `M₃`-shaped triples; the FGI lattice contains tetragraph relations that may embed `N₅`). Non-distributive theorems apply (Knaster-Tarski, congruence theory, modular-law machinery); distributive-specific theorems (Birkhoff's representation theorem, Stone duality) do not.

### Axis J: Is the structure complemented?

**If unique complements don't exist:** the structure is not a Boolean algebra. If it's bounded distributive, it's a *bounded distributive lattice*; further, it might be a Heyting algebra (relative pseudocomplements exist) but not Boolean. **Consultant move:** name it as bounded distributive lattice (or Heyting algebra if relative pseudocomplements exist) — both are perfectly good named categories, but neither has the laws of Boolean algebra. See `pure-lattice.md` entries 8 (Boolean) and 9 (Heyting).

### Combined diagnostic: the "what variety?" tree

```
Start: a structure (M, *_1, *_2, possibly constants)
  │
  ▼ Idempotent? (Axis B)
  ├── No  → Not a lattice. Likely a semigroup or magma. Redirect.
  └── Yes
      │
      ▼ Commutative? (Axis C)
      ├── No  → Not a lattice. Non-commutative algebraic structure. Redirect.
      └── Yes
          │
          ▼ Associative? (Axis D)
          ├── No  → Magma. Highly specialized. Redirect.
          └── Yes
              │
              ▼ Absorption? (Axis E)
              ├── No  → Two semilattices on the same carrier. Not a lattice.
              └── Yes
                  │ → IT IS A LATTICE
                  ▼
                  Bounded? (Axes F, G)
                  ├── No top, no bottom → "lattice"
                  ├── Bottom only        → "lattice with bottom"
                  ├── Top only           → "lattice with top"
                  └── Both               → "bounded lattice"
                      │
                      ▼ Complete? (Axis H)
                      ├── Yes → "complete lattice"; can apply Knaster-Tarski
                      └── No  → "bounded lattice", not complete; use Kleene with CPO if applicable
                          │
                          ▼ Distributive? (Axis I)
                          ├── No  → "non-distributive lattice"; modular if Dedekind's law holds
                          └── Yes → "distributive lattice"
                              │
                              ▼ Complemented + unique? (Axis J)
                              ├── No  → "bounded distributive lattice"
                              │   │ Heyting implication exists?
                              │   ├── Yes → "Heyting algebra"
                              │   └── No  → "bounded distributive lattice"
                              └── Yes → "Boolean algebra"
```

**Citation.** `[burris-sankappanavar-1981]` ch. I (lattice axioms) and ch. II §1–§9 (algebra structure); `[davey-priestley-2002]` chs. 2, 4, 8, 11; `[bergman-2011]` ch. 1; `[gratzer-1979]`; for the partial-algebra side, `[gratzer-1979]`.

**Consultant tags.** This whole entry is the most-cited entry in this file. `(a)` when one of the diagnostic answers is "yes — what they have is exactly an X." `(b)` when one of the answers is "they're missing one law and adding it would give the variety they actually want." `(c)` when the structure is too far from any variety to call it lattice-shaped.

> **When this comes up.** Almost every marque structural question routes through this entry. Walk the diagnostic in order; stop at the first failing axis; name the structure precisely. Most marque candidates fail at axis F (no top) or axis I (non-distributive) — both *fine*, both producing valid named structures, both with their own theorem-toolkits.

---

## 12. Quotient algebra by a congruence

**Definition.** Let `A` be an algebra of signature `F` and `θ ∈ Con(A)`. The *quotient algebra* `A/θ` has:

- Universe: the set `A/θ` of equivalence classes of `θ`.
- Operations: for each `f ∈ F` of arity `n`, `f^{A/θ}([a₁], ..., [aₙ]) = [f^A(a₁, ..., aₙ)]`.

Well-definedness is exactly the compatibility property of `θ` (entry 10): if `aᵢ θ bᵢ` for all `i`, then `f(a₁, ..., aₙ) θ f(b₁, ..., bₙ)`, so the operation on equivalence classes is independent of representative choice. See `[burris-sankappanavar-1981]` Definition II.5.2.

**Universal property.** `A/θ` is the unique (up to isomorphism) algebra `B` such that there is a surjective homomorphism `q : A → B` with `ker(q) = θ`. Every quotient of `A` is `A/θ` for some `θ ∈ Con(A)`.

**Quotient stays in the variety.** If `A ∈ V` and `θ ∈ Con(A)`, then `A/θ ∈ V`. (This is `H`-closure of variety from entry 3.) So quotients of lattices are lattices, quotients of distributive lattices are distributive lattices, etc.

**For marque.** Quotients are how the user "merges" elements of a lattice. The most natural marque example: collapsing a vocabulary to a coarser canonical form (e.g., treating "ABCD" and "abcd" as the same compartment for case-insensitive comparison). The equivalence "case-insensitive equality" is a congruence iff lattice operations don't depend on case — which they don't, in marque, because case is presentation, not semantics. So the case-folded vocabulary is a quotient lattice of the case-sensitive one.

**Citation.** `[burris-sankappanavar-1981]` ch. II §5; `[gratzer-2011]` ch. III; `[davey-priestley-2002]` §6; `[bergman-2011]` ch. 5.

**Consultant tags.** `(a)` baseline — quotients are routine lattice-theory; the structure carries through.

> **When this comes up.** "I want to canonicalize." If the canonicalization respects `∨` and `∧`, it's a congruence; the canonical form lives in the quotient lattice; theorems about lattices apply.

---

## 13. Direct product, subdirect product, ultraproduct

**Direct product.** Given algebras `(Aᵢ)ᵢ∈I` of the same signature, the *direct product* `∏ᵢ Aᵢ` has universe the Cartesian product `∏ᵢ Aᵢ` (sequences of elements, one from each `Aᵢ`) and coordinatewise operations: `f^∏(((aᵢ¹)ᵢ, ..., (aᵢⁿ)ᵢ)) = (f^{Aᵢ}(aᵢ¹, ..., aᵢⁿ))ᵢ`. See `[burris-sankappanavar-1981]` Definition II.7.1.

**Subdirect product.** Defined in entry 9. An embedding into a direct product such that each coordinate is surjective.

**Ultraproduct.** Given algebras `(Aᵢ)ᵢ∈I` and an ultrafilter `U` on `I`, the *ultraproduct* `(∏ᵢ Aᵢ)/U` is the quotient of the direct product by the equivalence relation "agreeing on a `U`-large set of coordinates." The ultraproduct of finite algebras can be infinite; ultraproducts preserve first-order logical formulas (Łoś's theorem). See `[burris-sankappanavar-1981]` ch. V §2.

**For marque.** Direct products are the most-used composition: `SciSet × SarSet × FgiSet` is a direct product of lattices (giving a lattice). Subdirect products are mostly theoretical (entry 9). Ultraproducts essentially never appear in marque-shaped questions; cited here only for completeness.

**Variety closure.** Varieties are closed under direct products by definition (entry 3). Varieties are also closed under subdirect products (an embedding into a direct product, then a subalgebra, all in the variety). Varieties are NOT in general closed under ultraproducts (a quasi-variety property — entry 8).

**Citation.** `[burris-sankappanavar-1981]` ch. II §7 and ch. V §2; `[bergman-2011]` ch. 5.

**Consultant tags.** `(a)` for direct products — most marque compositions land here.

> **When this comes up.** "I want to combine two policy lattices." The direct product is the canonical combination; the result is a lattice (since lattices are a variety, closed under products). Distributivity, modularity, completeness, boundedness all lift coordinatewise — see `pure-lattice.md` entry 11.

---

## 14. Polynomial functions vs. term functions on a lattice

**Definition (term function).** Let `A` be an algebra and `t(x₁, ..., xₙ)` an `(F, X)`-term. The associated *term function* `t^A : Aⁿ → A` evaluates the term in `A`. Term functions are precisely the functions expressible by composing the fundamental operations.

**Definition (polynomial function).** A *polynomial function* on `A` is a term function on the algebra obtained from `A` by adding all elements of `A` as new constants. Equivalently, polynomial functions allow constants from `A` to appear in the expression. See `[burris-sankappanavar-1981]` ch. II §13 and `[gratzer-1979]`.

**Example.** On the lattice `(P({a, b, c}), ⊆, ∩, ∪)`:

- `f(x) = x ∪ x` is a *term function*: expressible from `x` by `∪`.
- `f(x) = x ∪ {a}` is a *polynomial function* but NOT a term function: it uses the constant `{a}`.

**Why this distinction matters.** Term functions are uniformly definable across the variety; polynomial functions are specific to the chosen algebra. When a user writes `fn merge(x: SciSet) -> SciSet { x ∪ specific_set }`, the function is polynomial but not a term function; it makes sense on the specific instance but not on every member of the variety.

For congruence theory: every term function preserves every congruence; every polynomial function preserves every congruence (because constants are fixed). For *first-order definability*: term functions are first-order-definable using only the algebra's signature; polynomial functions need the constants.

**Citation.** `[burris-sankappanavar-1981]` ch. II §13; `[gratzer-1979]`; `[bergman-2011]` ch. 4.

**Consultant tags.** `(a)` when distinguishing "this operation is part of the lattice structure" (term function) from "this operation depends on specific values" (polynomial function). `(c)` when a user's "lattice operation" is actually a polynomial function and they're treating it as if it lifts to the variety; flag the failure to lift.

> **When this comes up.** "Is `fn merge_with_default(x) -> x ∪ DEFAULT_SET` a lattice operation?" No — it's a polynomial function, not a term function. It depends on `DEFAULT_SET`. It doesn't make sense in the abstract free distributive lattice; it only makes sense once you've fixed a specific instantiation with `DEFAULT_SET` available.

---

## 15. Equational reasoning

**Birkhoff's completeness theorem (`[burris-sankappanavar-1981]` Theorem II.14.19).** For any signature `F`, the following are equivalent for an identity `s ≈ t`:

- `s ≈ t` is a *consequence* of a set of identities `Σ` — every algebra satisfying `Σ` also satisfies `s ≈ t`.
- `s ≈ t` is *provable* from `Σ` using the rules of equational logic: reflexivity (`s ≈ s`), symmetry (`s ≈ t ⇒ t ≈ s`), transitivity (`s ≈ t, t ≈ u ⇒ s ≈ u`), substitution (`s ≈ t ⇒ s[u/x] ≈ t[u/x]`), and replacement / congruence (`s₁ ≈ t₁, ..., sₙ ≈ tₙ ⇒ f(s₁,...,sₙ) ≈ f(t₁,...,tₙ)`).

So semantic and syntactic consequence agree. Equational reasoning is sound and complete for varieties.

**For lattices specifically.** The four lattice laws (L1–L4) generate all lattice identities by these inference rules. So when a user wants to verify `t ≈ s` holds in every lattice, they can reduce to applying L1–L4 (and the inference rules) — no semantic argument needed.

**Practical use in marque.** When a user writes a lattice expression and wants to canonicalize it:

- For free lattices: Whitman's algorithm (entry 7).
- For free distributive lattices: reduce to disjunctive normal form (a.k.a. *join of meets of generators*); see `[davey-priestley-2002]` §8.10.
- For free Boolean algebras: reduce to the canonical truth-table representation; see `[burris-sankappanavar-1981]` ch. IV §1.

These canonicalizations are mechanically applicable; cite `[freese-nation-lectures]` for the lattice case.

**Citation.** `[burris-sankappanavar-1981]` ch. II §14; `[bergman-2011]` ch. 4 §4.5; `[davey-priestley-2002]` §8.

**Consultant tags.** `(a)` for verifying a lattice identity. `(b)` for canonicalization tasks; the catalog above gives the right normal form by sub-variety.

> **When this comes up.** A user wants to prove that two marque lattice expressions denote the same element. If the expressions are over a free lattice (no specific values), use equational reasoning / Whitman; if over a free distributive lattice, use DNF canonicalization. If over a specific finite lattice instance, just enumerate.

---

## 16. Modular lattices and the M3-N5 theorem

**Modular law (Dedekind, 1894 — `[dedekind-1894]`).** A lattice `L` is *modular* if for all `a, b, c ∈ L` with `a ≤ b`:

`a ∨ (c ∧ b) = (a ∨ c) ∧ b`.

(See `pure-lattice.md` entry 7 for the order-theoretic perspective; this entry covers the equational/structural angle.)

**M3-N5 theorem (Dedekind / Birkhoff `[birkhoff-1937]`).** A lattice `L` is:

- *Distributive* iff `L` contains *neither* `M₃` (the diamond) *nor* `N₅` (the pentagon) as a sublattice.
- *Modular* iff `L` does not contain `N₅` as a sublattice. (`M₃` is allowed in modular lattices but not distributive.)

This is the *forbidden-sublattice characterization*. To check distributivity / modularity of a candidate marque structure: search for `M₃`-shaped or `N₅`-shaped 5-element configurations.

`M₃` (the diamond): five elements `{⊥, a, b, c, ⊤}` with `a, b, c` pairwise incomparable, all above `⊥` and all below `⊤`. Modular but not distributive.

`N₅` (the pentagon): five elements `{⊥, a, b, c, ⊤}` with `b ≤ c`, `a` incomparable to both `b` and `c`. Neither modular nor distributive.

**For marque.** Modular non-distributive lattices show up in classification-marking lattices that combine "incomparable-but-related" controls. The SCI compartment lattice can embed `M₃` whenever three compartments are pairwise incomparable but all above some shared base. This is fine — modular lattices are well-studied — but the user needs to know they don't get distributive-lattice-specific theorems (Birkhoff representation, Stone duality).

**Equational form.** Modular law as identity (over the lattice signature, no antecedent):

`x ∨ (y ∧ (x ∨ z)) ≈ (x ∨ y) ∧ (x ∨ z)`.

This holds in every modular lattice (and is implied by but does not imply distributivity).

**Citation.** `[dedekind-1894]` (original); `[birkhoff-1937]`; `[burris-sankappanavar-1981]` Definition I.7.1 and Theorem I.7.5; `[davey-priestley-2002]` §4.7–§4.10; `[gratzer-2011]` ch. III.

**Consultant tags.** `(a)` for naming a structure as modular non-distributive when the user has incomparable-but-related elements. `(c)` when the user assumes distributivity but a 5-element `M₃` or `N₅` shows up; explicitly construct the embedding.

> **When this comes up.** "Is my SCI/compartment lattice distributive?" Look for three pairwise-incomparable compartments above a common base — that's `M₃`, and distributivity fails. The structure may still be modular (no `N₅`), in which case modular-lattice theorems still apply.

---

## 17. Distributive lattices and Stone duality

**Stone duality (Stone 1936 — `[stone-1936]`).** There is a *contravariant equivalence* between:

- The category `BoolAlg` of Boolean algebras with Boolean homomorphisms.
- The category `Stone` of Stone spaces (compact totally disconnected Hausdorff spaces) with continuous maps.

Concretely: every Boolean algebra `B` is isomorphic to the Boolean algebra of clopen subsets of its Stone space `S(B)` (the space of ultrafilters on `B`). And every Stone space `X` is the Stone space of the Boolean algebra of its clopen subsets.

**Generalization to distributive lattices.** Priestley duality (Priestley 1970, see `[davey-priestley-2002]` ch. 11): a contravariant equivalence between bounded distributive lattices and *Priestley spaces* (compact, totally order-disconnected, ordered topological spaces). The Boolean case is the special case where the order is trivial.

**For marque.** The duality matters when the user wants to think of lattice elements as "subsets of a hidden space." For finite distributive lattices, Birkhoff's representation theorem (`[birkhoff-1937]`) gives a clean version: every finite distributive lattice is the lattice of down-sets of some poset (the poset of its join-irreducibles).

The takeaway for consulting: when you have a finite distributive lattice and you want to "concrete-ize" it, the join-irreducibles give you a poset whose down-sets recover the lattice. This is sometimes useful for visualization or for reducing a complicated lattice question to a simpler poset question.

**Citation.** `[stone-1936]`; `[birkhoff-1937]`; `[davey-priestley-2002]` chs. 5 and 11; `[johnstone-stone-spaces]` ch. II; `[burris-sankappanavar-1981]` ch. IV §4.

**Consultant tags.** `(a)` for finite distributive lattices when the user wants a concrete representation. `(b)` for general distributive lattices when the user wants Priestley-space machinery — but this is rarely the right tool for marque-shaped problems.

> **When this comes up.** A user has a finite distributive lattice and asks "what does the structure look like?" Compute the poset of join-irreducibles, draw the Hasse diagram, recover the lattice as down-sets of that poset. This often clarifies structural questions that look complicated when written as lattice expressions.

---

## 18. Order-sorted / many-sorted algebras

**Many-sorted algebra.** A *many-sorted signature* is a pair `(S, F)` where `S` is a set of sort symbols and `F` is a set of operation symbols, each symbol assigned an *arity profile* `(s₁, ..., sₙ; s)` with `sᵢ` (input sorts) and `s` (output sort) drawn from `S`. A *many-sorted algebra* over `(S, F)` is a family `(A_s)_{s ∈ S}` of carrier sets indexed by sorts, plus interpretations `f^A : A_{s₁} × ⋯ × A_{sₙ} → A_s` for each operation. See `[bergman-2011]` ch. 11; the standard reference for many-sorted equational logic is `[goguen-meseguer-1992]`.

**Order-sorted algebra.** A many-sorted algebra where the sort set carries a partial order, and operations are *subsort polymorphic* — an operation defined for sort `s'` is also defined for any subsort `s ≤ s'`. See `[goguen-meseguer-1992]`. This is the formalism behind type systems with subtyping.

**For marque.** Compartments come in flavors: SCI compartments, SAR compartments, FGI codes, dissem controls. Operations (`∨`, `∧`) are defined within each flavor but not always across flavors — meeting an SCI control with an FGI tetragraph is meaningful only after interpretation. Universally a many-sorted algebra of signature `(SCI, SAR, FGI, Dissem; ∨, ∧)`, with each `∨`, `∧` operating within a single sort.

The sub-sort relationship matters when the user wants to express "every SCI compartment is also a Dissem control" or similar inclusion. If the inclusion is honest (operations restrict cleanly), order-sorted algebra captures it; if not, the user has separate algebras per sort and a relation between them, but no algebraic inclusion.

**Practical guidance.** For most marque consulting questions, single-sorted lattice theory suffices because each lattice (`SciSet`, `SarSet`, `FgiSet`) is treated independently. The many-sorted framework matters when the user wants to express "the marking-lattice as a whole" with operations spanning sorts; for that, order-sorted algebra (or a tagged sum) is the formal home.

**Citation.** `[goguen-meseguer-1992]`; `[bergman-2011]` ch. 11; `[meinke-tucker-1992]` for an introduction; `[nlab-many-sorted-algebra]`.

**Consultant tags.** `(a)` when the user explicitly wants typed operations (SCI-meet vs. FGI-meet are different operations, with different domain/codomain). `(b)` when the user is reaching for "one big lattice" but really has several typed sub-lattices; pivot to many-sorted formalism.

> **When this comes up.** "I have multiple kinds of compartments and operations only make sense within a kind." That's many-sorted. Don't artificially merge them into one carrier set. The formal tool is many-sorted equational logic; in Rust the natural realization is separate types implementing `Lattice`, with `IsmAttributes` as the typed product.

---

## 19. Diagnosis: "What variety does this structure live in?"

A short procedure the consultant runs on a candidate marque structure. Output: the most-specific variety the structure *is* in, plus the laws that fail (if any) before reaching the more-specific varieties the user might have asked about.

**Step 1: What is the signature?** Identify the operations (binary, unary, nullary) and constants. Pin down whether `⊤` and `⊥` are part of the signature (then the structure can be a *bounded* something) or derived (then they're not in the signature even if the structure happens to have a top element).

**Step 2: Run the almost-lattice diagnostic (entry 11).** Walk through axes B, C, D, E, F, G, H, I, J in order, stopping at the first failing axis. The axis that fails names the laws that fail; everything before it succeeds and pins down the variety.

**Step 3: Read off the variety.**

- Failed at B (idempotence): not a lattice; commutative semigroup at best.
- Failed at C or D (commutativity / associativity): not a lattice; magma or semigroup.
- Failed at E (absorption): not a lattice; two unrelated semilattices on the same carrier.
- Passed B–E, failed at F or G (no top, no bottom, or both missing): lattice or semilattice; not bounded.
- Passed F and G, failed at H (incomplete): bounded lattice; not complete.
- Passed F, G, H, failed at I (non-distributive): bounded complete (or not) lattice; modular if Dedekind's law holds, just lattice if not.
- Passed everything through I, failed at J (no unique complement): bounded distributive lattice; Heyting algebra if relative pseudocomplements exist.
- Passed all axes including J: Boolean algebra.

**Step 4: For each "passed everything up to here" assessment, name what's available.**

- "Lattice" gets you: HSP closure of the variety of all lattices, distributive *and* non-distributive theorems on the structural side, Whitman's algorithm for the free version.
- "Distributive lattice" adds: Birkhoff's representation, Stone duality (for bounded versions), DNF normal form.
- "Heyting algebra" adds: relative pseudocomplements, intuitionistic logic interpretation.
- "Boolean algebra" adds: classical logic, Stone duality with Stone spaces.
- "Bounded lattice" adds: well-defined empty-meet and empty-join, completeness for finite versions.
- "Complete lattice" adds: Knaster-Tarski (`pure-lattice.md` entry 19), arbitrary closures.
- "Frame" (also a complete Heyting algebra) adds: Heyting implication everywhere, sublocale machinery — see `frames-locales.md`.

**Step 5: Verify by sample computation.** Pick three to five concrete elements and verify the laws the assessment claims hold, especially the distributive law (often the most failure-prone). One concrete counterexample is conclusive against a claimed law.

**Citation.** `[burris-sankappanavar-1981]` chs. I–II; `[davey-priestley-2002]` chs. 1–11; `[bergman-2011]` chs. 1–5.

**Consultant tags.** This is the most-cited entry in this file, alongside entry 11. The diagnostic is a summary of how to use the rest of the catalog. `(a)` and `(b)` and `(c)` outcomes are all possible depending on which axis fails.

> **When this comes up.** Almost any structural question. Walk the diagnostic, output the variety, name the unlocked theorems, name the laws that fail and what they cost.

---

## How to read the consultant tags

- `(a)` order-theory-adapted: the catalog entry IS the right name for the user's structure. Use directly.
- `(b)` pivot: the user's structure isn't quite the entry but a redesign would land it inside. Name the trade-off.
- `(c)` refuse / redirect: the entry is what the user thinks they have but doesn't; the right answer is to name the gap and redirect to a different entry, file, or framework.

---

*Bibliography entries appended to `references/bibliography.md`. No new vendored sources beyond Agent A's `burris-sankappanavar-universal-algebra.pdf`, which is the primary reference for this file.*
