# Abstract Interpretation — Catalog Reference

**Audience.** Claude, in lattice-consultant mode, scanning for the closest named construction matching a problem the user just described in informal English.

**Scope of this file.** *Applied* lattice algebra: what the abstract-interpretation (AI) literature builds *on top of* the order-theory primitives. The pure order-theory primitives (Knaster-Tarski, Kleene chain, Galois connection at the algebraic level, CPO/dcpo, Scott continuity, well-foundedness) live in `pure-lattice.md`. This file does not redefine them. It cites forward and tells you what AI adds: program-analysis framing, soundness proofs, widening, narrowing, abstract domains.

**How to use.** Locate the catalog entry whose definition matches (or nearly matches) the construction in front of you. Each entry tags which consultant outcome it most often supports — `(a)` order-theory-adapted, `(b)` pivot toward a known pattern, `(c)` refuse / redirect — and ends with a "When this comes up" hook so you can route to it from a question shape.

**Companion files.**
- `pure-lattice.md` (Agent A) — order-theory primitives. Cited heavily here.
- `marque-applied.md` (Agent E) — translates these entries into marque-specific design questions.

**The key bias of this file.** Most marque problems are NOT abstract-interpretation problems. They are finite-height-lattice + monotone-rule problems, which is one corner case of AI but rarely needs the full machinery (Galois connections, widening, narrowing, etc.). The diagnostic section "When AI is the right framework — and when it isn't" is the most consulted part of this file. Read it before you propose AI machinery.

---

## Table of Contents

1. [Galois connection as program-analysis abstraction](#1-galois-connection-as-program-analysis-abstraction) — α/γ pair forming an adjunction; the soundness scaffold.
2. [Galois insertion vs. Galois connection](#2-galois-insertion-vs-galois-connection) — the stronger property `α∘γ = id`; when it matters.
3. [Best abstraction / optimal transfer functions](#3-best-abstraction--optimal-transfer-functions) — `α∘f∘γ` is the most precise sound abstraction.
4. [Soundness of an abstract operator](#4-soundness-of-an-abstract-operator) — `α∘f ⊑ f^♯∘α`; the inequality that *is* the proof obligation.
5. [Kleene iteration on a finite-height lattice](#5-kleene-iteration-on-a-finite-height-lattice) — chain stabilizes after finitely many steps; the cheap path.
6. [Iteration on infinite-height lattices](#6-iteration-on-infinite-height-lattices) — fixpoint is transfinite; why widening is needed.
7. [Widening operator ∇](#7-widening-operator) — over-approximation + termination guarantee; precision-vs-termination trade-off.
8. [Narrowing operator Δ](#8-narrowing-operator) — refining a widened over-approximation; when it helps and when it doesn't.
9. [Monotone framework / data-flow analysis](#9-monotone-framework--data-flow-analysis) — Kildall's classical setup; transfer functions per program point with meet-over-paths.
10. [Distributive vs. non-distributive frameworks](#10-distributive-vs-non-distributive-frameworks) — when MOP = MFP and when it doesn't (Kildall '73, Kam-Ullman).
11. [Fixed-point combinators on monotone operators](#11-fixed-point-combinators-on-monotone-operators) — `lfp`, `gfp`; AI usage of Knaster-Tarski.
12. [Constant-propagation lattice](#12-constant-propagation-lattice) — the canonical worked example; flat lattice ⊥ / constants / ⊤.
13. [Sign abstract domain](#13-sign-abstract-domain) — five elements; the smallest interesting AI domain.
14. [Interval abstract domain](#14-interval-abstract-domain) — Cousot-Cousot 1976/77; the canonical infinite-height domain.
15. [Polyhedral abstract domain](#15-polyhedral-abstract-domain) — Cousot-Halbwachs 1978; relational, expensive, expressive.
16. [Composition of abstract domains (reduced product)](#16-composition-of-abstract-domains-reduced-product) — combining two abstractions; when the result is finer than either.
17. [Concrete vs. abstract semantics — soundness, completeness, precision](#17-concrete-vs-abstract-semantics--soundness-completeness-precision) — why "exact" is too strong a goal.
18. [Topological scheduling as a fixed-point computation](#18-topological-scheduling-as-a-fixed-point-computation) — when ordered rewrites converge; the link to Knaster-Tarski.
19. [Confidence / posterior propagation as abstract interpretation](#19-confidence--posterior-propagation-as-abstract-interpretation) — score lattices and probabilistic AI.
20. [Termination via finite-height (the cheap path)](#20-termination-via-finite-height-the-cheap-path) — when widening is unnecessary.

[Diagnostic — When AI is the right framework, and when it isn't](#diagnostic--when-ai-is-the-right-framework-and-when-it-isnt)

[How to read the consultant tags](#how-to-read-the-consultant-tags)

---

## 1. Galois connection as program-analysis abstraction

**Definition (the AI specialization).** A *Galois connection* between a *concrete domain* `(C, ≤)` and an *abstract domain* `(A, ⊑)` is a pair of monotone maps `α : C → A` (abstraction) and `γ : A → C` (concretization) satisfying the adjunction

```
α(c) ⊑ a   ⇔   c ≤ γ(a)        for all c ∈ C, a ∈ A.
```

Notation: `α ⊣ γ` (`α` is left adjoint to `γ`). `α` preserves all joins; `γ` preserves all meets. The order-theoretic underlying machinery is `pure-lattice.md` entry 17 — this file uses the *program-analysis convention*: monotone form, `α` is on the left (concrete-to-abstract), `γ` is on the right.

**The soundness theorem this licenses.** Let `f : C → C` be a concrete operator (a "step" in the concrete semantics). Let `f^♯ : A → A` be a candidate abstract operator. The pair `(α, γ)` makes `f^♯` *sound* iff

```
α ∘ f  ⊑  f^♯ ∘ α
```

(equivalently `f ∘ γ ≤ γ ∘ f^♯` by adjunction). This says: abstracting then stepping in `A` over-approximates stepping in `C` then abstracting. See entry 4 for the dual formulation.

**Why this matters.** Once `(α, γ)` is fixed and `f^♯` proven sound, the lfp of `f^♯` over-approximates the lfp of `f` (provided both lattices are complete and the operators are monotone — `pure-lattice.md` entry 19). That's the entire scaffolding of program analysis: replace an intractable concrete fixed-point computation with a tractable abstract one whose result is sound by construction.

**Citation.** `[cousot-cousot-1977]` introduced the framework; `[cousot-cousot-1979]` systematized the design of analyses *from* a Galois connection; `[moller-schwartzbach-spa]` ch. 4 and ch. 12 give the modern textbook treatment; `[wikipedia-abstract-interpretation]`; `[erne-koslowski-melton-strecker-1993]` for the algebraic background.

**Consultant tags.** `(a)` whenever the user is doing "abstract over-approximation of a concrete computation" with both sides being lattices. `(b)` when the user has only `α` *or* only `γ` and wants the other — propose finding the adjoint (when it exists; not all monotone maps have one). `(c)` when the concrete domain isn't a lattice or the question isn't a fixed-point question.

> **When this comes up.** "We summarize a complex marking into a simpler view; how do we know the summary is sound?" If both sides are lattices, the question is whether `(α, γ)` form a Galois connection and whether the abstract step is sound. State the adjunction inequality and check it on the operations involved.

---

## 2. Galois insertion vs. Galois connection

**Definition.** A Galois connection `α ⊣ γ` is a *Galois insertion* if additionally

```
α ∘ γ  =  id_A
```

i.e., abstracting the concretization of an abstract value gives back exactly the same abstract value. Equivalently: every element of `A` is in the image of `α`, i.e., `γ` is injective. Equivalently: there are no "redundant" abstract elements (no two abstract values with the same concretization).

The unconditional law of any Galois connection is the *closure* `γ∘α : C → C` (extensive, idempotent on its image — `pure-lattice.md` entry 18) and the *kernel* `α∘γ : A → A` (deflationary, idempotent). For an *insertion*, the kernel is the identity; for a general connection it is merely a kernel (interior) operator.

**Why the stronger property matters when it does.** Galois insertion is the right setting for arguing that the abstract domain is *minimal* — every abstract element corresponds to a distinct concrete subset. Without it, the abstract domain may have "redundant" elements that no `α(c)` ever produces. Many design papers in AI assume insertion implicitly to avoid carrying around junk elements.

**Why the weaker property is usually enough.** The soundness theorem (entry 4) goes through with a Galois connection; it does not require insertion. If you only need sound over-approximation, insertion is a nice-to-have, not a requirement. `[cousot-cousot-1992]` "Comparing the Galois connection and widening/narrowing approaches" explicitly works with non-insertion connections.

**How to obtain insertion from a connection.** Quotient `A` by the equivalence `a₁ ~ a₂ ⇔ γ(a₁) = γ(a₂)`. The result is the Galois insertion you wanted; see `[cousot-cousot-1979]`.

**Citation.** `[cousot-cousot-1979]`; `[wikipedia-galois-connection]` (explicit definition of Galois insertion as the case `FG = id`); `[moller-schwartzbach-spa]` ch. 12.

**Consultant tags.** `(a)` rarely the headline answer — `α∘γ = id` is not usually the question being asked. `(b)` when the user has built an abstract domain with redundant elements and is confused by why two distinct abstract values give the same concretization — the cure is to quotient down to a Galois insertion. `(c)` when the user is *demanding* insertion and the abstract domain genuinely needs over-approximation slack (e.g., enumerated abstract states with explicit `⊤` for "all of these collapse together").

> **When this comes up.** "Should our abstract pairs `(α, γ)` satisfy `α∘γ = id`?" Probably not strictly required, but if you find yourself with two abstract values that always concretize to the same set, you've got an unnecessarily redundant domain — quotient it to a Galois insertion.

---

## 3. Best abstraction / optimal transfer functions

**Definition.** Given a Galois connection `α ⊣ γ` and a concrete operator `f : C → C`, the *best abstraction* (also "optimal abstract transfer function") of `f` is

```
f^♯_opt  :=  α ∘ f ∘ γ.
```

This is the most precise sound abstract operator: any sound `f^♯` satisfies `f^♯_opt ⊑ f^♯`. The lfp of `f^♯_opt` is the most precise sound over-approximation of the lfp of `f` — in fact, when the connection is an insertion, `lfp(f^♯_opt) = α(lfp(f))`.

**Practical reality: usually not computable.** `α ∘ f ∘ γ` requires materializing `γ(a)` (a concrete set, possibly infinite or undecidable) and then re-abstracting. Most analyses build a *sound but suboptimal* `f^♯` that is computable but coarser than `f^♯_opt`. The standard pattern: define `f^♯` directly on abstract values, then prove `f^♯_opt ⊑ f^♯` by exhibiting a witness.

**Compositionality caution.** `f^♯_opt ∘ g^♯_opt` is sound for `f ∘ g` but not necessarily *optimal*: composition of best abstractions need not be the best abstraction of the composition. Composing analyses introduces irreducible precision loss; this is one of the standard sources of "completeness defects" in AI.

**Citation.** `[cousot-cousot-1977]` Theorem 7.1.0.4; `[cousot-cousot-1979]` for the systematic design that builds `f^♯` from `f` and `α`/`γ`; `[moller-schwartzbach-spa]` ch. 12 §12.4.

**Consultant tags.** `(a)` when designing a new analysis: the formula `α ∘ f ∘ γ` is the design target — even if you can't compute it, knowing it tells you what you're approximating. `(b)` when a hand-rolled `f^♯` is plausibly suboptimal — propose deriving the optimum and comparing.

> **When this comes up.** "Is our abstract step the most precise possible?" Compute (or sketch) `α ∘ f ∘ γ` for a small representative input. If your `f^♯` agrees, you have the optimum. If your `f^♯` is strictly coarser, you've identified a precision gap — sometimes acceptable, sometimes worth fixing.

---

## 4. Soundness of an abstract operator

**Definition.** An abstract operator `f^♯ : A → A` is *sound* with respect to a concrete operator `f : C → C` and a Galois connection `α ⊣ γ` iff

```
α ∘ f  ⊑  f^♯ ∘ α       (forward / lower-adjoint formulation)
```

Equivalently (by adjunction):

```
f ∘ γ  ≤  γ ∘ f^♯       (backward / upper-adjoint formulation)
```

These say the same thing: every concrete step is approximated above by an abstract step.

**The soundness theorem.** If `f^♯` is sound and both lattices are complete with `f` and `f^♯` monotone, then

```
α(lfp f)  ⊑  lfp f^♯       (and dually  lfp f  ≤  γ(lfp f^♯))
```

So the abstract analysis result over-approximates the concrete reality. See `[cousot-cousot-1977]` Theorem 7.1.0.4, `[moller-schwartzbach-spa]` Theorem 12.4.

**Why the inequality is one-sided.** Soundness does not promise *equality*. The abstract `lfp` may be strictly larger than `α(lfp f)` — it can include "ghost" reachable states that aren't actually reachable. That gap is *imprecision*; if the gap is zero, the analysis is *complete* (entry 17, very rare in practice).

**Citation.** `[cousot-cousot-1977]`; `[moller-schwartzbach-spa]` ch. 12; `[wikipedia-abstract-interpretation]`.

**Consultant tags.** `(a)` for "is this abstract operation correct?" — the soundness inequality *is* the proof obligation; check it explicitly on the operations involved. `(c)` when the user expects equality (completeness) and has built only soundness — the precision gap is real and not a bug.

> **When this comes up.** "How do we prove our abstract step doesn't miss anything?" Write down `f` and `f^♯`, then verify `α ∘ f ⊑ f^♯ ∘ α` pointwise on a few small concrete inputs. If it fails on any input, `f^♯` is unsound and the analysis can give a wrong answer.

---

## 5. Kleene iteration on a finite-height lattice

**Statement.** On a complete lattice `L` of *finite height* (the longest chain `⊥ ⊑ x₁ ⊑ x₂ ⊑ … ⊑ ⊤` has finite length), every monotone `f : L → L` reaches its least fixed point in finitely many steps:

```
lfp f  =  f^k(⊥)   for some k ≤ height(L).
```

The iteration is `⊥, f(⊥), f²(⊥), …`, and it stabilizes the first time `f^{k+1}(⊥) = f^k(⊥)`. See `pure-lattice.md` entry 20 for the general Kleene theorem; this entry highlights the AI-specific corollary: *finite-height lattices automatically satisfy Kleene's continuity hypothesis*, so monotonicity alone suffices.

**Proof sketch.** The chain `⊥ ⊑ f(⊥) ⊑ f²(⊥) ⊑ …` is ascending (by monotonicity of `f` plus `⊥ ⊑ f(⊥)`). On a finite-height lattice every ascending chain stabilizes. Once stable, the value is a fixed point; since each `f^i(⊥) ⊑ x` for any pre-fixed point `x`, it's the *least* fixed point. See `[moller-schwartzbach-spa]` Theorem 4.4 (which they attribute to Kleene `[kleene-1952]`); `[davey-priestley-2002]` Theorem 8.22 gives the order-theoretic version.

**The "naive fixed-point algorithm".** Iterate `x ← f(x)` from `x = ⊥` until `x` doesn't change. Terminates in `≤ height(L)` iterations. This is what most data-flow analyses (entry 9) actually compute. Worklist algorithms are optimizations; the Kleene chain is the spec.

**The bottleneck.** For an `n`-variable, height-`h` problem, naive Kleene is `O(n · h · time(f))`. For a powerset lattice on `m` elements, `h = m`, so finite height is not the same as small height. Worklist orderings (entry 9) and chaotic iteration `[bourdoncle-1993]` reduce the constant factor.

**Citation.** `[kleene-1952]` (original); `[moller-schwartzbach-spa]` ch. 4 §4.4 Theorem 4.4 (textbook proof); `[cousot-cousot-1977]` §6 (AI specialization); `[davey-priestley-2002]` Theorem 8.22.

**Consultant tags.** `(a)` whenever the lattice is finite-height and the operator is monotone — Kleene iteration is the answer, and it terminates. `(b)` when the lattice is "almost finite-height" (e.g., bounded by a constant per program) — propose materializing the bound and using Kleene unmodified.

> **When this comes up.** "Will iterating this rule until stable terminate?" Check finite-height + monotonicity. If both, yes (Kleene). If not finite-height, you need entry 6 / widening. If not monotone, fixpoint may not exist at all — re-examine the operator.

---

## 6. Iteration on infinite-height lattices

**The problem.** On a complete lattice without finite height, Knaster-Tarski (`pure-lattice.md` entry 19) guarantees `lfp f` exists — but the Kleene chain `⊥, f(⊥), f²(⊥), …` may not reach it in finitely many steps. The supremum may be reached only at a transfinite ordinal.

**Example (interval domain).** Concrete program `x = 0; while (...) x = x + 1;`. Concrete step `f(σ) = σ ∪ {σ(x) + 1}`. Abstract over the *interval* domain (entry 14): `f^♯([l, h]) = [l, h] ⊔ [l+1, h+1] = [l, h+1]` (joining the previous interval with its post-step image). Iterate from `⊥`: `[0, 0] ⊑ [0, 1] ⊑ [0, 2] ⊑ …` — never stabilizes. The lfp in the lattice is `[0, ∞]`, but the Kleene chain reaches it only at ordinal `ω`. See `[moller-schwartzbach-spa]` §6.1.

**Why this matters.** AI's value proposition is "compute fixed-point approximations on a smaller abstract domain." If that abstract domain has infinite height, the Kleene iteration is a non-starter for actual computation. You need either:

1. A different abstract domain with finite height (sometimes possible — e.g., bounded intervals).
2. A *widening* operator (entry 7) that forces termination at the cost of precision.
3. A *chain-acceleration* technique (e.g., `[bourdoncle-1993]` strategy iteration) that finds a better approximant after finitely many steps.

The widening route is the standard one; the literature has 50 years of widening operator design for specific abstract domains.

**Citation.** `[cousot-cousot-1977]` §6.2; `[moller-schwartzbach-spa]` ch. 6; `[cousot-cousot-1992]` for the comparison of widening vs. Galois-connection-based extrapolation.

**Consultant tags.** `(a)` for diagnosing why naive iteration doesn't terminate — name "infinite-height lattice" as the cause. `(b)` when the user can plausibly switch to a finite-height domain (e.g., bounded intervals) — propose that before reaching for widening. `(c)` when the user has an open-ended vocabulary AND wants iterative roll-up — flag that no finite construction will give an exact answer.

> **When this comes up.** "We iterate forever and never converge." Check the lattice height. If it's finite, the operator might be non-monotone; if infinite, you need either a coarser domain or a widening operator.

---

## 7. Widening operator ∇

**Definition.** A *widening* on a lattice `L` is a binary operator `∇ : L × L → L` satisfying two conditions:

1. **Over-approximation (extrapolation):** for all `x, y ∈ L`, `x ⊑ x ∇ y` and `y ⊑ x ∇ y`. (`x ∇ y` is an upper bound of both, possibly *larger* than the join `x ⊔ y` — that's what makes it widening rather than join.)

2. **Termination:** for any chain `y₀, y₁, y₂, …`, the sequence
   `x₀ = y₀,   x_{n+1} = x_n ∇ y_{n+1}`
   eventually stabilizes (`x_n = x_{n+1}` for large enough `n`).

The widening is *applied* at fixed-point iteration: instead of `x ← f(x)`, compute `x ← x ∇ f(x)`. Condition 1 makes the result a sound over-approximation; condition 2 forces termination.

**Critical property: not necessarily commutative or associative.** Widening is *not* a join. `x ∇ y` may differ from `y ∇ x`; `(x ∇ y) ∇ z` need not equal `x ∇ (y ∇ z)`. This is intentional: widening is *biased* toward extrapolating from the first argument, treating the second as "the new addition." In iteration, `x` is the accumulated result and `y` is the latest update.

**Canonical example: interval widening.** On the interval lattice `Interval = {[l, h] : l ≤ h, l ∈ ℤ ∪ {-∞}, h ∈ ℤ ∪ {+∞}}` plus `⊥`:

```
[l₁, h₁] ∇ [l₂, h₂]  =  [l₁ if l₁ ≤ l₂ else -∞,
                         h₁ if h₁ ≥ h₂ else +∞]
```

Reading: if the lower endpoint is decreasing, slam it to `-∞`; if the upper endpoint is increasing, slam it to `+∞`. After at most two applications, both endpoints are at `±∞` — termination guaranteed in two steps. See `[cousot-cousot-1976]` (the original interval domain) and `[moller-schwartzbach-spa]` §6.2.

**Refinement: bounded widening.** Pick a finite "thresholds" set `T ⊆ ℤ ∪ {±∞}` (e.g., constants appearing in the program). Define `∇` to snap endpoints to the nearest threshold rather than `±∞`. Termination holds because the image of `∇` has finite cardinality. Precision is much better; this is the form most static analyzers actually use.

**The trade-off.** Widening trades precision for termination. The post-widening fixed point is sound but typically strictly larger than the lfp of `f`. Once widening fires, naive iteration stops climbing — recovering precision requires *narrowing* (entry 8).

**Citation.** `[cousot-cousot-1977]` §6.2 (introduction); `[cousot-cousot-1976]` (interval-specific); `[cousot-cousot-1992]` (comparison with non-widening alternatives); `[moller-schwartzbach-spa]` §6.2 (textbook treatment with code).

**Consultant tags.** `(a)` for "this iteration doesn't terminate on an infinite-height lattice" — widening is the standard answer. `(b)` when the user is reaching for widening on a finite-height lattice — point out widening is unnecessary and merely loses precision. `(c)` when the user expects widening to be precision-neutral — it isn't; the over-approximation is intrinsic.

> **When this comes up.** "Our iteration won't terminate on this infinite-height domain." Reach for a widening operator. State the two conditions explicitly and verify both for the candidate `∇`. Note the precision loss; consider entry 8 to recover.

---

## 8. Narrowing operator Δ

**Definition.** A *narrowing* on a lattice `L` is a binary operator `Δ : L × L → L` satisfying:

1. **Refinement:** for all `x, y ∈ L` with `y ⊑ x`, `y ⊑ (x Δ y) ⊑ x`. (Narrowing pulls `x` *down*, but never below `y`.)

2. **Termination:** for any chain `y₀ ⊒ y₁ ⊒ y₂ ⊒ …`, the sequence `x₀ = y₀, x_{n+1} = x_n Δ y_{n+1}` eventually stabilizes.

Narrowing is applied *after* widening has terminated: take the widened post-fixed-point `x*`, then iterate `x ← x Δ f(x)` until stable. Each step refines `x` toward `lfp f` (since `f(x*) ⊑ x*`), but cannot fall below `f(x*)` — preserving soundness.

**Canonical example: interval narrowing.** On intervals:

```
[l₁, h₁] Δ [l₂, h₂]  =  [l₂ if l₁ = -∞ else l₁,
                         h₂ if h₁ = +∞ else h₁]
```

If widening produced `[-∞, h]` and the next iterate is `[5, h]`, narrowing replaces `-∞` with `5` — the actual lower bound found by the analysis. Termination is one step (since `±∞` can only be replaced once before stabilizing).

**When narrowing helps.** When widening was overly aggressive — extrapolated to `±∞` for an endpoint that the actual computation would have stayed bounded for. Narrowing recovers a tighter bound by re-examining `f` at the widened point.

**When narrowing doesn't help.** When the widened bound is already tight. Also when the operator `f` itself produces `±∞` — narrowing has nothing to refine to. Some analyses skip narrowing entirely because the imprecision recovered is small relative to the engineering cost.

**Order of phases.** Cousot-Cousot 1977 specifies: ascending widening phase (until stable above `lfp f`), then descending narrowing phase (until stable above `lfp f`, but tighter). Each phase terminates by its own termination condition. See `[cousot-cousot-1977]` §6.2.

**Citation.** `[cousot-cousot-1977]` §6.2; `[cousot-cousot-1992]`; `[moller-schwartzbach-spa]` §6.2.

**Consultant tags.** `(a)` rarely the headline — usually mentioned as the post-widening cleanup pass. `(b)` when a widened analysis is "too imprecise to use" — propose adding narrowing before redesigning the abstract domain.

> **When this comes up.** "Widening converged but the answer is `[-∞, +∞]` — useless." Add a narrowing pass. If after narrowing the answer is still useless, the abstract domain is wrong, not the iteration.

---

## 9. Monotone framework / data-flow analysis

**Definition (Kildall's monotone framework).** A *monotone data-flow analysis framework* is a tuple `(L, F, F^♯)` where:

- `L` is a complete lattice of finite height ("the lattice of analysis values"; e.g., sets of variables, sign abstractions, intervals).
- `F` is the *control-flow graph* with edges between program points.
- For each program statement `s`, a *transfer function* `[[s]] : L → L` is monotone.

The analysis assigns a value `[v] ∈ L` to each program point `v`. For a *forward* analysis:

```
[v]  =  ⊔ { [[s]]([u]) : edge (u, v) in F via statement s }
```

(or the dual for backward analyses). Solve simultaneously across all `v` — this is a system of monotone equations on `L^|V|`, lfp computed by Kleene (entry 5). Kildall's worklist algorithm `[kildall-1973]` is the standard implementation.

**Why monotone framework is an AI special case.** Identify the concrete domain with traces of program states; `α` collapses each trace to its program-point value; `γ` materializes the set of reachable concrete states. The transfer function `[[s]]` is the abstract operator for statement `s`. Soundness (entry 4) is what you prove once when you design the framework.

**The MOP solution.** Define the *meet over all paths* (MOP) at point `v`:

```
MOP(v)  =  ⊔ { [[π]](init) : π is a path from entry to v }
```

where `[[π]] = [[s_n]] ∘ … ∘ [[s_1]]` for path `π = s_1, …, s_n`. MOP is the most precise sound analysis result — it joins exactly the contributions from each actual concrete execution path.

**The MFP solution.** The lfp of the equation system above. By construction, `MOP(v) ⊑ MFP(v)` — the iterative join over predecessors may merge paths "earlier" than MOP would, losing precision.

**Citation.** `[kildall-1973]` (original framework + worklist algorithm); `[kam-ullman-1977]` (MOP vs. MFP analysis); `[moller-schwartzbach-spa]` ch. 5 (textbook treatment); `[nielson-nielson-hankin-1999]` ch. 2 (book-length treatment).

**Consultant tags.** `(a)` whenever a problem looks like "iterate per-statement transformations on a CFG-like graph until stable" — this is the canonical pattern; reuse the framework rather than reinventing. `(b)` when the user has built an iteration pattern that resembles a monotone framework but is missing the lattice-of-values structure — propose factoring it into the framework shape.

> **When this comes up.** "We propagate per-position information through a graph of dependencies, joining at confluences." That's the monotone framework. Identify the lattice, the transfer functions, the join. If finite-height + monotone, Kleene gives termination.

---

## 10. Distributive vs. non-distributive frameworks

**Definition.** A monotone framework is *distributive* if every transfer function preserves binary joins:

```
[[s]](x ⊔ y)  =  [[s]](x) ⊔ [[s]](y)       for all x, y ∈ L.
```

**The MOP = MFP theorem (Kildall '73).** If the framework is distributive, then `MOP(v) = MFP(v)` for every program point. Iterative analysis loses no precision relative to the path-by-path semantics.

**The MOP < MFP gap (Kam-Ullman '77).** If the framework is *non*-distributive — only monotone — then `MOP(v) ⊑ MFP(v)` strictly in general. The MFP solution is sound but strictly less precise than MOP. `[kam-ullman-1977]` constructed examples; `[muchnick-1997]` gives an accessible exposition.

**Examples of distributive frameworks.**
- Reaching definitions, live variables, available expressions, very busy expressions — all classical bit-vector analyses.
- Anything where transfer functions are union-of-translations on a powerset lattice.

**Examples of non-distributive frameworks.**
- Constant propagation (entry 12). `[[x = a + b]](top, top) = top` but `[[x = a + b]](1, 2) ⊔ [[x = a + b]](3, 4) = {3} ⊔ {7} = top`. The transfer over the join is `top`; the join of transfers also `top`; equal here. But `[[x = a + b]]({1}, {2}) ⊔ [[x = a + b]]({3}, {4}) = {3, 7}` while `[[x = a + b]]({1, 3}, {2, 4}) = {3, 5, 7}` if you take cross-products — non-distributive.
- Most "relational" abstract domains (intervals don't decompose along join, polyhedra even less so).

**Why this matters in practice.** Distributivity buys you the right to reorder MOP into MFP without precision loss. Non-distributive frameworks are still useful — Kleene iteration still terminates and the answer is sound — but the user must accept that iterative analysis is strictly less precise than path-by-path enumeration.

**Citation.** `[kildall-1973]` (distributive case, worklist algorithm); `[kam-ullman-1977]` (non-distributive case, MOP-MFP gap); `[moller-schwartzbach-spa]` §12.4; `[nielson-nielson-hankin-1999]` §2.4.

**Consultant tags.** `(a)` when the user worries about "is iterative analysis as precise as path-by-path?" — answer: yes if distributive, no in general. `(b)` when the user has built a non-distributive analysis and is surprised by precision loss — name the cause (Kam-Ullman gap). `(c)` when the user *requires* MOP precision and the framework is non-distributive — that requires a different algorithm (path enumeration, IFDS for the special case of distributive subset problems, or a coarser abstract domain).

> **When this comes up.** "Why is our iterative analysis less precise than computing per-path?" Check distributivity of the transfer functions. If non-distributive, the gap is intrinsic; either accept it, switch domains, or use a path-enumeration algorithm.

---

## 11. Fixed-point combinators on monotone operators

**The lfp / gfp pair.** For a monotone `f : L → L` on a complete lattice (`pure-lattice.md` entry 19, Knaster-Tarski):

```
lfp f  =  ⨅ { x : f(x) ⊑ x }    (least pre-fixed-point)
gfp f  =  ⨆ { x : x ⊑ f(x) }    (greatest post-fixed-point)
```

Both are fixed points; the set of fixed points forms a complete sublattice with bottom `lfp f` and top `gfp f`.

**AI usage.** Forward (reachability-style) analyses compute `lfp` of an operator `F(X) = init ⊔ step(X)` — "smallest set closed under the step, containing the initial points." Backward (safety / co-reachability) analyses compute `gfp` of an operator describing "largest set closed under the property of interest." See `[cousot-cousot-1977]` §4 for the systematic treatment.

**Practical computation.**
- `lfp` via Kleene iteration from `⊥` (entry 5).
- `gfp` via *dual* Kleene iteration from `⊤`: `⊤, f(⊤), f²(⊤), …` is *descending*, stabilizes on a finite-height lattice. Equivalent statement of Kleene for descending chains by order-duality.
- Both terminate when the lattice is finite-height; widening (entry 7) is needed for `lfp` on infinite-height domains; the dual notion of *narrowing-from-above* applies to `gfp`.

**Citation.** `[tarski-1955]` (general theorem; cited at `pure-lattice.md` entry 19); `[cousot-cousot-1977]` §4 (AI specialization); `[moller-schwartzbach-spa]` §4.4 (Theorem on `lfp`).

**Consultant tags.** `(a)` for any "smallest set closed under …" or "largest set such that …" question. `(b)` when the user has a recursive equation and isn't sure which fixed point is intended — name the choice and its semantics.

> **When this comes up.** "Smallest set of markings closed under this propagation rule." That's `lfp`. "Largest set of markings such that no rule fires." That's `gfp`. They are different fixed points and need different iteration strategies (from `⊥` vs. from `⊤`).

---

## 12. Constant-propagation lattice

**Definition.** For each variable, track a value in the *flat lattice* over a value type `V`:

```
ConstLat(V)  =  {⊥} ∪ V ∪ {⊤}
```

with `⊥ ⊑ v ⊑ ⊤` for all `v ∈ V`, and elements of `V` mutually incomparable. Read: `⊥` = "uninitialized / unreachable", `v` = "definitely the constant `v`", `⊤` = "could be any value" (not a constant).

Lattice height = 2. Total number of elements = `|V| + 2`. Per-variable lattice; the analysis state is the *product* over all variables (entry 16 / `pure-lattice.md` entry 11).

**Transfer function for `x = e`.**

```
[[x = c]](σ)   =  σ[x ↦ c]                               (literal)
[[x = y]](σ)   =  σ[x ↦ σ(y)]                            (copy)
[[x = y + z]](σ) = σ[x ↦ if σ(y) ∈ V ∧ σ(z) ∈ V
                          then σ(y) + σ(z)
                          else if σ(y) = ⊥ ∨ σ(z) = ⊥
                          then ⊥
                          else ⊤]                        (operation)
[[x = input]](σ) = σ[x ↦ ⊤]                              (unknown)
```

**Why this is non-distributive.** `[[x = y + z]]({y ↦ 1, z ↦ 2}) ⊔ [[x = y + z]]({y ↦ 3, z ↦ 4}) = {x ↦ 3} ⊔ {x ↦ 7} = {x ↦ ⊤}` (since `3 ≠ 7` are incomparable in the flat lattice). But `[[x = y + z]]({y ↦ 1, z ↦ 2} ⊔ {y ↦ 3, z ↦ 4}) = [[x = y + z]]({y ↦ ⊤, z ↦ ⊤}) = {x ↦ ⊤}`. Equal in this example, but the path-enumeration result `{3, 7}` is more precise than what either iterative join captures.

**Citation.** `[kam-ullman-1977]` (the canonical non-distributive example); `[moller-schwartzbach-spa]` §5.2; `[nielson-nielson-hankin-1999]` §2.4.

**Consultant tags.** `(a)` for "when we know the value, propagate it; otherwise mark unknown" — that's literally constant propagation. `(b)` when the user wants to propagate single-element-set abstractions in a generic way — flat lattice over the value type is the canonical recipe.

> **When this comes up.** "Per-variable, we know the value or we don't." That's the flat lattice. The transfer function for an operation reads each operand's value and combines; if any operand is `⊤`, the result is `⊤`. Total precision per variable = constant value, otherwise none.

---

## 13. Sign abstract domain

**Definition.** Abstract numeric values into one of five elements:

```
Sign  =  {⊥, NEG, ZERO, POS, ⊤}
```

where `NEG` = "definitely negative", `ZERO` = "definitely 0", `POS` = "definitely positive", `⊤` = "could be any sign", `⊥` = "unreachable / unknown". Order: `⊥ ⊑ NEG, ZERO, POS ⊑ ⊤`; `NEG, ZERO, POS` mutually incomparable. This is the flat-lattice construction `flat({NEG, ZERO, POS})` (see entry 12 above for the same pattern over arbitrary value sets `V`). Lattice height = 2.

**Abstract operations.** Defined by the obvious "sign rules":

```
NEG + NEG = NEG
POS + POS = POS
NEG + POS = ⊤      (could be any sign)
ZERO + x = x
NEG * NEG = POS
NEG * POS = NEG
ZERO * x = ZERO
…
```

`α(n) = NEG if n < 0, ZERO if n = 0, POS if n > 0`. `γ(NEG) = (-∞, 0)`, etc. Galois connection between integer-set lattice and `Sign`.

**Why sign analysis is the AI 101 example.** Smallest non-trivial abstract domain. Finite-height, distributive (each transfer is a single-output computation, not a join of cases). Kleene iteration trivially terminates. The textbook presentation in `[moller-schwartzbach-spa]` ch. 4 uses sign analysis as the introductory worked example.

**Citation.** `[cousot-cousot-1977]` §1 (motivating example); `[moller-schwartzbach-spa]` §4.1 and §5.1; `[nielson-nielson-hankin-1999]` §1.

**Consultant tags.** `(a)` rarely directly applicable to marque-shape problems (numerics aren't the domain), but useful as a *teaching* example when explaining what an abstract domain is. `(b)` for any "track 'one of N exclusive states or unknown' per variable" pattern — sign domain is the small-N flat-lattice instance.

> **When this comes up.** "We want to track one of {a few mutually-exclusive states} per location, with 'unknown' as a fallback." That's a flat abstract domain in the sign-domain shape.

---

## 14. Interval abstract domain

**Definition.** Abstract a numeric value into a closed interval over `ℤ ∪ {-∞, +∞}`:

```
Interval  =  {⊥} ∪ { [l, h] : l ≤ h, l ∈ ℤ ∪ {-∞}, h ∈ ℤ ∪ {+∞} }
```

with order `[l₁, h₁] ⊑ [l₂, h₂] ⇔ l₂ ≤ l₁ ∧ h₁ ≤ h₂` (subset on intervals). `⊤ = [-∞, +∞]`. Per-variable lattice; the analysis state is the product over all variables.

**Lattice height: infinite.** `[0, 0] ⊑ [0, 1] ⊑ [0, 2] ⊑ …` is an unbounded ascending chain. *This is the canonical example demonstrating why widening is necessary*. See `[moller-schwartzbach-spa]` §6.1.

**Abstract operations.** Defined by interval arithmetic:

```
[l₁, h₁] +̂ [l₂, h₂]  =  [l₁ + l₂, h₁ + h₂]
[l₁, h₁] *̂ [l₂, h₂]  =  [min(l₁l₂, l₁h₂, h₁l₂, h₁h₂), max(...)]
…
```

**Widening (entry 7) is mandatory.** Naive Kleene from `⊥` doesn't terminate on programs with unbounded loops. Standard widening: snap endpoints to `±∞` when growing; refinements use threshold sets.

**Citation.** `[cousot-cousot-1976]` (original "Static Determination of Dynamic Properties of Programs"); `[cousot-cousot-1977]` §6 (interval-domain widening); `[moller-schwartzbach-spa]` ch. 6.

**Consultant tags.** `(a)` whenever the user wants "tight numeric bounds per variable" — interval is the standard answer. `(c)` when the user expects iteration to terminate without widening on this domain — name the issue explicitly.

> **When this comes up.** "Track lower and upper bounds for numeric variables." Interval domain. Note infinite height; widening is required; precision can be tuned via threshold sets.

---

## 15. Polyhedral abstract domain

**Definition (sketch).** Abstract a tuple of numeric variables `(x₁, …, x_n)` by the *convex polyhedron* of values they could jointly take, expressed as a finite conjunction of linear inequalities:

```
Poly  =  { ⋂ᵢ { v : aᵢ · v ≤ bᵢ } : finite conjunction }
```

Order = inclusion of polyhedra. Operations `⊔` (convex hull) and `⊓` (intersection) computable but expensive; complexity exponential in the worst case. `[cousot-halbwachs-1978]` introduced the domain and gave the algorithms.

**Compared to intervals.** Polyhedra capture *relations* between variables (`x ≤ y + 3`, `x + 2y ≥ 0`), which intervals cannot. This is "relational" vs. "non-relational" precision: the polyhedral domain is *much* more precise but *much* more expensive.

**Restricted relational domains (computational compromises).**
- *Octagons* (Miné 2001): conjunctions of `±xᵢ ± xⱼ ≤ c`. Polynomial-time operations. Standard in industrial static analyzers.
- *Zones* / DBMs (difference-bound matrices): `xᵢ - xⱼ ≤ c` only. Used heavily in timed-systems verification.

**Widening for polyhedra.** Bertrand-style widening: keep only inequalities preserved across the join, drop the rest. Termination by bounded number of inequality "shapes". `[cousot-halbwachs-1978]` original; refinements: `[bagnara-hill-zaffanella-2005]` Parma Polyhedra Library.

**Citation.** `[cousot-halbwachs-1978]` (original polyhedral domain); `[mine-2001]` (octagons); `[mine-2006]` (overview of weakly relational domains); `[moller-schwartzbach-spa]` ch. 7 (path sensitivity, brief mention of relational analyses).

**Consultant tags.** `(a)` rarely the answer for marque-shape problems (numeric polytope reasoning isn't the domain). `(c)` when the user wants relational precision but the domain isn't numeric — point out polyhedral machinery doesn't transfer.

> **When this comes up.** Almost never directly in marque shapes. Listed for completeness as the canonical "expensive but precise" abstract domain — useful as a contrast-of-trade-offs example.

---

## 16. Composition of abstract domains (reduced product)

**Definition (direct product).** Given Galois connections `(α₁, γ₁) : C ⇄ A₁` and `(α₂, γ₂) : C ⇄ A₂`, the *direct product* domain is `A₁ × A₂` with coordinatewise order, and abstraction `α(c) = (α₁(c), α₂(c))`, concretization `γ(a₁, a₂) = γ₁(a₁) ⊓ γ₂(a₂)`. Soundness inherited from each component (`pure-lattice.md` entry 11 for the lattice product).

**The precision shortfall of direct product.** `(α₁(c), α₂(c))` may have more "junk" abstract values than necessary. Example: `A₁` = sign domain, `A₂` = parity domain (even/odd/unknown). The pair `(NEG, EVEN)` rules out `c = -1`, `(NEG, ODD)` rules out `c = -2`, but the direct product treats both as independent and doesn't propagate the cross-constraint.

**Reduced product `A₁ ⊗ A₂` (Cousot-Cousot 1979).** Quotient `A₁ × A₂` by an equivalence: `(a₁, a₂) ≡ (a₁', a₂')` iff `γ(a₁, a₂) = γ(a₁', a₂')`. The *reduction operator* `ρ : A₁ × A₂ → A₁ × A₂` projects each pair to a canonical "tightest" representative. Operations are computed in the direct product, then `ρ`-reduced.

**Why reduction matters.** Without it, the cross-constraints between domains aren't communicated. With it, abstract operations exchange information across components — e.g., a *sign × interval* reduced product can use sign info to refine intervals: `POS ⊗ [-5, 10]` reduces to `[1, 10]`.

**The cost.** The reduction operator is often the entire engineering challenge. Cousot-Cousot 1979 specifies what reduction *is*, not how to compute it for any particular pair of domains. For sign × interval it's straightforward; for two relational domains, computing optimal reduction can dominate analysis time.

**Open product, smashed product, glb-based product.** Variants in the literature with different precision/cost trade-offs. See `[cousot-cousot-1979]` for the original taxonomy and `[cousot-2014]` for a modern survey.

**Citation.** `[cousot-cousot-1979]` §10 (introduction of reduced product); `[cousot-2014]` (modern survey of domain composition); `[nielson-nielson-hankin-1999]` §4.3.

**Consultant tags.** `(a)` for "we want to combine two analyses and have them share information" — reduced product is the formal recipe. `(b)` for "we built two analyses independently and they don't talk to each other" — propose reduced product; warn about the engineering cost of the reduction operator.

> **When this comes up.** "We have analysis X and analysis Y, and we want them to refine each other's results." Reduced product. The construction is straightforward; the reduction operator is the work.

---

## 17. Concrete vs. abstract semantics — soundness, completeness, precision

**Three properties.** For an abstract operator `f^♯` against a concrete `f`:

- **Soundness.** `α ∘ f ⊑ f^♯ ∘ α`. The abstract over-approximates the concrete. Required.
- **Completeness.** `α ∘ f = f^♯ ∘ α`. The abstract is *exact* — same as the concrete on every input. Rare in practice.
- **Precision.** Informally: `f^♯` is "as close to" `α ∘ f ∘ γ` (entry 3) as possible. This is a quality metric, not a binary property; partial orders of "more vs. less precise" exist but a unique "most precise computable" rarely does.

**Why "exact" is too strong.** AI's value is replacing an undecidable concrete computation with a decidable abstract approximation. Demanding completeness (exactness) typically forces the abstract domain to be as expressive as the concrete — at which point you've gained nothing. Soundness + acceptable precision is the practical target.

**Sources of imprecision.**
1. **Coarse abstraction.** `α` collapses too many concrete states. (Choice of abstract domain.)
2. **Approximate transfer.** `f^♯` over-approximates the optimum `α ∘ f ∘ γ`. (Choice of `f^♯`.)
3. **Non-distributive joining.** MFP > MOP (entry 10). (Iteration strategy.)
4. **Widening over-extrapolation.** Widening (entry 7) is intrinsically over-approximating.

**Completeness in special cases.** Exists in narrow contexts: *forward-complete* analyses in some game-semantic abstractions; *backward-complete* analyses for certain safety properties. See `[giacobazzi-quintarelli-2001]` for the formal taxonomy of completeness. Most production analyses are sound but neither forward- nor backward-complete.

**Citation.** `[cousot-cousot-1977]` §7 (soundness theorem); `[cousot-cousot-1979]` §11 (completeness analysis); `[giacobazzi-quintarelli-2001]` (formal taxonomy); `[ranzato-tapparo-2007]` (completeness refinement).

**Consultant tags.** `(c)` whenever the user demands an "exact" analysis — explain why exactness usually forces equivalence to the concrete and gains nothing. `(a)` for diagnosing precision issues — name which of the four sources is dominant.

> **When this comes up.** "The analysis is sound but useless because it returns `⊤` everywhere." Diagnose the imprecision source: too-coarse domain (entry 13–15 to swap), too-coarse transfer (entry 3 to refine), non-distributive (entry 10 to switch domains or algorithm), or aggressive widening (entry 8 to add narrowing).

---

## 18. Topological scheduling as a fixed-point computation

**The shape of the question.** A finite set of *rewrites* `{r₁, r₂, …, r_k}`, each acting on a shared state `s ∈ L` (a complete lattice). Does iterating them to convergence give a well-defined result, independent of order?

**The framing as Knaster-Tarski.** Define the joint operator

```
F(s)  =  s ⊔ r₁(s) ⊔ r₂(s) ⊔ … ⊔ r_k(s).
```

If each `rᵢ` is monotone and inflationary (`s ⊑ rᵢ(s)`), then `F` is monotone and inflationary. Knaster-Tarski (`pure-lattice.md` entry 19) on the complete lattice `L` guarantees `lfp F` exists. Order-independence: any iteration strategy that fairly applies each `rᵢ` reaches `lfp F`.

**Adding read/write annotations: the topological-schedule case.** When each rewrite `rᵢ` declares "axes I read" and "axes I write", a partial order on `{rᵢ}` arises: `rᵢ → rⱼ` if `rⱼ` reads what `rᵢ` writes. Topological sort gives a *deterministic* schedule — writers before readers. If the read/write graph is acyclic, the topological sort is well-defined; the result is the same as `lfp F` evaluated by Kleene with that schedule.

**Cycles in the read/write graph.** If `rᵢ` reads what `rⱼ` writes *and* `rⱼ` reads what `rᵢ` writes, no topological order exists. Two responses:

1. **Reject.** The rewrite set is genuinely cyclic; iteration order matters; the system may be inconsistent. Refuse to schedule.
2. **Iterate.** Run the cycle to a fixed point. If each `rᵢ` is monotone, the cycle still has a `lfp` by Knaster-Tarski — order doesn't change the limit, only the path.

The marque codebase (per `CLAUDE.md`) takes option 1: cycles in the rewrite read/write graph fail at `Engine::new` with `EngineConstructionError::RewriteCycle`. This is a design choice, not a forced one — a Kleene-iteration-with-cycles version is also defensible.

**Citation.** `[knuth-1973]` Vol. 1 §2.2.3 (topological sort algorithms); `pure-lattice.md` entry 19 (Knaster-Tarski); `[cousot-cousot-1977]` §6 (chaotic iteration as an alternative scheduling).

**Consultant tags.** `(a)` for "do these ordered rewrites converge?" — yes if monotone on a complete lattice, by Knaster-Tarski. `(b)` for "we want a deterministic schedule" — propose topological sort with cycle-rejection or chaotic iteration with cycle-tolerance, depending on whether order matters semantically. `(c)` for "do these rewrites *commute*?" — distinct question; commutativity is not the same as order-independence-in-the-limit.

> **When this comes up.** "Are these page-level rewrites guaranteed to converge regardless of order?" If each is monotone and the lattice is complete, yes (Knaster-Tarski). The topological-sort scheduler is one *strategy* for reaching the limit; chaotic iteration is another. Cycle-detection is a separate design choice, not a correctness requirement.

---

## 19. Confidence / posterior propagation as abstract interpretation

**The pattern.** A scoring decoder propagates confidence values through a graph of dependencies. At each "join" (multiple predecessors converging), confidence values are combined by some operator: `max`, `min`, weighted sum, log-product, etc. The question is whether this propagation is well-defined and whether iteration converges.

**When it is AI.** If the score domain forms a lattice with a meaningful order ("higher score is more confident"), and the per-step combiner is monotone in its inputs, *and* the graph is acyclic (or you accept `lfp` for cycles), then propagation is exactly a monotone framework analysis (entry 9). Lattice values = confidence elements; transfer functions = score combiners; lfp computed by Kleene.

**Common score lattices.**
- `([0, 1], ≤)` — closed unit interval. Complete (with `inf` / `sup`). Infinite descending and ascending chains; widening needed for naive iteration unless you discretize.
- `([0, 1], ≥)` — reversed (treating "lower is more confident" e.g. as a cost). Dual lattice (`pure-lattice.md` entry 13).
- Discretized score sets `{0.0, 0.1, …, 1.0}` — finite-height. Kleene iteration terminates trivially.
- Log-likelihood lattice `(ℝ ∪ {-∞, +∞}, ≤)` — infinite-height; widening needed.
- Boolean-valued indicator (confidence ≥ τ for fixed τ) — Boolean lattice, two elements, height 1.

**When it is *not* AI.** If the combiner is non-monotone (e.g., averaging is monotone in each argument; multiplying by an arbitrary signed weight is not), or the score domain isn't a lattice (free real numbers without a meaningful comparison), or the question is one-shot ("compute this score, no iteration"), then AI machinery doesn't apply — even if the surface looks similar.

**Probabilistic abstract interpretation.** A specialized literature: `[monniaux-2001]`, `[cousot-monerau-2012]` — abstract domains over probability distributions. Adapts AI to probabilistic semantics; treats distributions over concrete states as the abstract values. Beyond what marque-shape problems usually need.

**Citation.** `[cousot-cousot-1977]` (general framework); `[monniaux-2001]` (probabilistic AI introduction); `[cousot-monerau-2012]` ("Probabilistic Abstract Interpretation"); `[di-pierro-wiklicky-2000]` (early probabilistic AI).

**Consultant tags.** `(a)` for "we propagate confidence scores through a monotone combiner over a finite score set" — that's a finite-height monotone framework; Kleene works. `(b)` for "infinite continuous score range" — propose discretization or widening. `(c)` for "the combiner is non-monotone" — AI doesn't apply; need a different framework (probabilistic graphical models, explicit factorization, etc.).

> **When this comes up.** "Decoder propagates posterior scores through a graph; we want a fixed-point semantics." Identify the score lattice and the combiner. If finite + monotone, you have a monotone framework; cite `[kildall-1973]` and ship. If continuous + monotone, discretize or widening. If non-monotone, AI is the wrong toolbox.

---

## 20. Termination via finite-height (the cheap path)

**The cheap version of the convergence argument.** If the lattice has *finite height* by construction — i.e., the longest chain `⊥ ⊑ x₁ ⊑ x₂ ⊑ … ⊑ ⊤` has finitely many distinct elements — then *every* monotone operator's Kleene iteration terminates in at most `height(L)` steps (entry 5). No widening needed. No narrowing needed. No Galois connections needed (in the sense of: you can verify termination locally without invoking the AI scaffolding).

**Why this is the marque sweet spot.** Most marque marking lattices are constructed from:
- Bounded enumerations (classification levels: `U ⊏ C ⊏ S ⊏ TS`, height = 3).
- Finite-vocabulary subsets (compartments, dissem-control sets, FGI trigraphs at a fixed instantiation — finite by the schema).
- Flat lattices over pre-registered enumerations.
- Products of all of the above, which are still finite-height (`pure-lattice.md` entry 11).

Open-vocabulary domains (agency-extensible compartments, partner-national codes) are finite *per fixed instance* — at any given moment the vocabulary is a finite set, even if it can grow between schema versions. So as long as the engine instantiates against a fixed schema, the lattice is finite-height for that engine instance.

**The diagnostic.** Before reaching for any of entries 7–8 (widening / narrowing) or 14–15 (interval / polyhedral), ask: is the lattice finite-height by construction? If yes, *the cheap path is the right path*. The full AI machinery is overkill for finite-height domains and adds complexity without buying convergence (which you already have).

**The exception.** Score lattices over `[0, 1]` or `ℝ` (entry 19) are infinite-height even on a fixed instance. *That's* when widening becomes relevant. Discretizing the score range is the simplest mitigation.

**Citation.** `[moller-schwartzbach-spa]` ch. 4 (finite-height + monotone = Kleene terminates); `[davey-priestley-2002]` Theorem 8.22.

**Consultant tags.** `(a)` whenever the user has a finite-vocabulary domain and is reaching for AI machinery — name "finite-height + monotone = Kleene = done" and stop. `(c)` when the user is reaching for widening on a finite-height lattice — flag it as overkill and explain.

> **When this comes up.** "Should we add widening to this convergence loop?" Check finite-height first. If the lattice is finite-height by construction, no widening is needed — Kleene terminates by itself. Widening is for genuinely infinite-height domains (intervals, real-valued scores), not for "this iteration feels slow."

---

## Diagnostic — When AI is the right framework, and when it isn't

Most marque problems are *not* abstract-interpretation problems. The full AI framework — concrete semantics, abstract semantics, Galois connection, soundness theorem, widening, narrowing — is overkill for the typical marque shape. This diagnostic helps the consultant choose:

- **Reach for AI** when you have all four of: (1) a concrete domain that *is* a lattice or could be quotiented to one, (2) an abstract domain you're trying to relate to it via a Galois connection, (3) monotone transfer functions, (4) a fixed-point question about iteration convergence.
- **Reach for `pure-lattice.md` directly** (Knaster-Tarski, Kleene, no Galois connection) when you have a single lattice and want to iterate a monotone operator to a fixed point. No abstraction is involved; AI scaffolding is unnecessary.
- **Reach for non-lattice tools** when the concrete domain isn't a lattice, the question isn't a fixed point, or the operations aren't monotone.

### When AI is the right framework

1. **You have a concrete domain that is a lattice.** Powerset of states, set of program executions, set of reachable markings, set of tagged variable values. The order is "subset" or "more information." If the domain isn't a lattice (free-text content, structured documents-as-strings without an order, file-format streams), AI doesn't apply.

2. **You have an abstract domain related to it via α and γ.** The abstraction throws away information; the concretization recovers a sound over-approximation. If you have only one of α or γ, see entry 1.

3. **The transfer functions are monotone.** Each abstract step preserves order — bigger input means bigger output. Many operations are: union, max, intersection, "add a constraint." Some aren't: subtraction-from-set, "remove until empty," operations that depend on the *exact* current value rather than the order.

4. **The question is about iterative convergence.** "Does iterating this rule terminate? Does it reach the right limit? Is the limit sound?" These are AI questions.

If all four hold, AI applies. Cite Cousot-Cousot 1977 and use the framework.

### When AI is *not* the right framework

1. **The concrete domain isn't a lattice.** Free-text content (no meaningful order), document formats as streams, regular-expression matches against a corpus. AI machinery cannot help. Use string algorithms, parser combinators, or a probabilistic-recognition framework instead.

2. **The question is one-shot, not iterative.** "Validate this single marking against this single rule — is it conformant?" That's a *predicate evaluation*, not a fixed-point computation. AI is overkill; just evaluate the predicate.

3. **Operations are non-monotone.** Subtraction, deletion, "if-condition-true-replace-else-don't," anything that breaks `x ⊑ y ⇒ f(x) ⊑ f(y)`. Without monotonicity, Knaster-Tarski doesn't apply, lfp may not exist, and the AI soundness scaffold collapses. Re-examine the operations: sometimes "non-monotone in this direction" is "monotone in the dual direction" (entry 11 / `pure-lattice.md` entry 13).

4. **The question is about correctness in a non-iterative sense.** "Are these two rules logically consistent?" — that's a model-theoretic question (a constraint solver, an SMT instance, a proof assistant). "Does this rewrite preserve some invariant?" — that's a Hoare-logic question. Neither needs the AI machinery; both need different tools.

### "Doing AI" vs. "having a lattice with a fixpoint"

Many marque-shape problems use Knaster-Tarski (`pure-lattice.md` entry 19) directly: there's a single lattice, a monotone operator, a fixpoint to compute. *That's not AI*. AI specifically means: *two* lattices related by a Galois connection, with a soundness theorem connecting concrete and abstract behavior. If you don't have the abstraction, you don't need the AI vocabulary.

The marque page-rewrite scheduler (entry 18) is *just* Knaster-Tarski. Calling it "abstract interpretation" overstates what's happening: there's one lattice, one monotone operator, one fixed point. No abstraction, no soundness theorem, no Galois connection. Resist the urge to lift terminology when the simpler theorem suffices.

### Termination strategy hierarchy

1. **Finite-height lattice (entry 20).** Cheapest. No widening, no narrowing. Kleene terminates by chain length.
2. **Well-founded operator (`pure-lattice.md` entry 22).** Termination by descending chain condition; structural induction works.
3. **Widening (entry 7).** Forces termination on infinite-height lattices at the cost of precision. Standard for numeric domains.
4. **Cousot's "termination as abstract interpretation" framework.** A whole research program treating termination *itself* as a fixed-point question on a different lattice. `[cousot-cousot-2012]` is the canonical reference. Out of scope for marque shapes.

Most marque problems land at level 1. A few (score lattices) might want level 3. Level 4 is essentially never needed; if the user is asking about it, they're probably overshooting.

### What AI literature offers and what it doesn't

**It offers:**
- Soundness as a *proven* property, not an unverified hope.
- A vocabulary for talking about the precision-cost trade-off in analyses.
- A 50-year catalog of abstract domains for specific concrete domains.
- Widening / narrowing as principled techniques for taming infinite-height lattices.

**It doesn't offer:**
- Free precision. Soundness is an inequality, not equality.
- A way to handle non-monotone or non-lattice concrete domains.
- Termination guarantees beyond what the underlying lattice/operator already provides.
- A substitute for understanding the structure of the problem. The framework formalizes; it doesn't solve.

### The consultant's bias

When AI machinery would help, name it explicitly: "this is a Galois connection," "you need widening here," "the MOP-MFP gap is intrinsic." When AI machinery wouldn't help, *say so*. The user is not best served by a paragraph of `α∘γ` notation when the answer is "this is just a finite-height lattice with a monotone operator — Knaster-Tarski applies, no Galois connection needed." Honesty about scope is part of the consultant's value.

---

## How to read the consultant tags

Every entry above ends with a short tag indicating which consultant outcome it most often supports:

- **(a) order-theory-adapted approach.** The construction is the typical answer to a class of marque-shape questions. Propose it directly.
- **(b) pivot toward a known pattern.** The construction is what we'd recommend the user move *toward* when their current design is close-but-not-quite. Name the gap.
- **(c) refuse / redirect.** The entry is sometimes cited to explain *why* a construction isn't a fit — the underlying problem isn't an AI problem, the lattice doesn't have the structure required, or the operation isn't monotone.

A single entry can support multiple modes. Most AI entries are dual-use:
- **Galois connection (1), Galois insertion (2), best abstraction (3), soundness (4)** — `(a)` when the user is doing genuine abstract over-approximation; `(c)` when they're actually doing direct Knaster-Tarski (no abstraction) and the AI vocabulary is overkill.
- **Kleene on finite-height (5), termination via finite-height (20)** — primary `(a)` answers for the marque sweet spot; the most-traveled entries.
- **Infinite-height iteration (6), widening (7), narrowing (8)** — `(a)` only when the lattice is genuinely infinite-height; `(c)` when reaching for them on finite-height domains.
- **Monotone framework (9), distributive vs. non-distributive (10)** — `(a)` for "do iterative rule loops have the same precision as path enumeration?" — usually yes if distributive, no in general.
- **Constant prop (12), sign (13), interval (14), polyhedral (15)** — `(a)` rarely directly applicable to marque shapes (numeric domains aren't the target); `(b)` as worked examples for explaining what an abstract domain is.
- **Reduced product (16)** — `(a)` for "we have two analyses and want them to refine each other"; `(b)` when the user has built two independent analyses and wonders why they don't.
- **Soundness/completeness/precision (17)** — `(c)` is the dominant tag — explaining why "exact analysis" is usually too strong a goal.
- **Topological scheduling (18)** — primarily `(a)` for the marque page-rewrite shape; cite Knaster-Tarski rather than full AI machinery.
- **Confidence propagation (19)** — `(a)` for finite-discretized scores; `(c)` for non-monotone combiners.

The marque-specific question shapes that route to each entry are codified in `marque-applied.md` (Agent E). This file gives the AI surface; that one gives the marque-flavored translation.

---

*End of catalog.*
