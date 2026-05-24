# Frames and Locales — Catalog Reference

**Audience.** Claude, in lattice-consultant mode, scanning for whether a "complete-lattice-of-things-that-compose-under-arbitrary-unions" is structurally a *frame*.

**Why this file is short.** Frames and locales are pointless topology. They belong in this skill for one narrow but important diagnostic: given a complete lattice that arises in marque (e.g., "the lattice of policies on a subject", "the lattice of rewrites that fire on a page"), does the meet distribute over *arbitrary* joins? That is the law that separates a complete lattice from a frame, and it is the law that licenses many compositional arguments. Most of the time the answer in marque is "no, you have a complete lattice but not a frame, here is what you cannot do." That refusal is the whole reason this file exists.

**Companion files.**
- `pure-lattice.md` (Agent A) — definitions of poset, lattice, complete lattice, distributive lattice, Heyting algebra, Boolean algebra. Read those first; this file *cites forward* to them rather than redefining.
- `universal-algebra.md` (sibling) — when the question is whether a structure even forms a lattice in the equational sense.
- `abstract-interp.md` (Agent C) — Galois connections, Scott topology; some of the depth on "topology of opens" lives there because that is where it intersects program analysis.

---

## Table of Contents

1. [Frame](#1-frame) — complete lattice with finite meets distributing over arbitrary joins.
2. [Locale](#2-locale) — a frame, viewed via the dual category; "pointless topology."
3. [Complete lattice vs. frame: the canonical non-example](#3-complete-lattice-vs-frame-the-canonical-non-example) — when distributivity fails for arbitrary joins.
4. [Frame morphism / locale morphism](#4-frame-morphism--locale-morphism) — preserves arbitrary joins and finite meets / its dual.
5. [Open-set lattice of a topological space](#5-open-set-lattice-of-a-topological-space) — the motivating example; why it IS a frame.
6. [Sublocales and sub-frames](#6-sublocales-and-sub-frames) — closure under operations.
7. [Frame-of-policies framing](#7-frame-of-policies-framing) — when a security-policy lattice models opens-on-a-subject; brief, mostly a pointer.
8. [Subframe-of-a-product](#8-subframe-of-a-product) — when a product of frames is a frame, and the structural constraints.
9. [Diagnosis: "Is this construction a frame?"](#9-diagnosis-is-this-construction-a-frame) — a short test the consultant can run on a candidate marque construction.

[How to read the consultant tags](#how-to-read-the-consultant-tags)

---

## 1. Frame

**Definition.** A *frame* is a poset `(F, ≤)` that has

- arbitrary joins `⋁ᵢ xᵢ` for every (small) family `(xᵢ)ᵢ∈I`, including the empty family (which gives the bottom `⊥`);
- finite meets `x ∧ y` (and the empty meet, which is the top `⊤`); and
- satisfies the *infinite distributive law*:

  `x ∧ (⋁ᵢ yᵢ) = ⋁ᵢ (x ∧ yᵢ)`  for all `x ∈ F` and all families `(yᵢ)ᵢ∈I` in `F`.

Equivalently — and this is the most-cited equivalence — a frame is exactly a complete Heyting algebra. Every frame `F` automatically has, for each `x ∈ F`, a Heyting implication `x ⇒ (–) : F → F` defined as the right adjoint to `x ∧ (–)`; the right adjoint exists because `x ∧ (–)` preserves arbitrary joins, by the infinite distributive law, and the adjoint functor theorem for posets says a join-preserving monotone map between complete lattices has a right adjoint. See `[nlab-frame]`, `[picado-pultr-2012]` ch. I §1, `[johnstone-stone-spaces]` ch. II.

So the chain of containments is:

`{frames}` ⊂ `{complete distributive lattices}` ⊂ `{distributive lattices}` ⊂ `{lattices}`

with the leftmost containment strict (see entry 3 for the canonical non-example) and `{frames} = {complete Heyting algebras}` (see `[nlab-heyting-algebra]`, `[picado-pultr-2012]` Proposition III.3.1.1).

**Equivalent characterizations.**

- "Complete lattice in which finite meets distribute over arbitrary joins" — the standard textbook definition, e.g., `[davey-priestley-2002]` ch. 11 §11.5.
- "Complete Heyting algebra" — `[nlab-heyting-algebra]`, `[picado-pultr-2012]` III.3.1.
- A poset with all small colimits (joins) and all finite limits (meets) such that finite limits commute with small colimits — this is the categorical-poset reading; see `[nlab-frame]`. Useful when relating to topos theory but not load-bearing for marque.

**Note.** "Finite meets" in the definition is the operative phrase. A frame need NOT have arbitrary meets distribute over arbitrary joins. The dual law — `x ∨ (⋀ᵢ yᵢ) = ⋀ᵢ (x ∨ yᵢ)` — generally fails in a frame. A poset where BOTH infinite distributive laws hold is called a *completely distributive lattice* (a strictly stronger condition; see `[davey-priestley-2002]` §11.45). Frames are the right notion for "topology"; completely distributive lattices are the right notion for things like "lattices of down-sets."

**Citation.** `[picado-pultr-2012]` ch. I (definition); `[johnstone-stone-spaces]` II.1.2; `[nlab-frame]`; `[davey-priestley-2002]` §11.5; `[vickers-1989]` ch. 3.

**Consultant tags.** `(a)` when a construction satisfies the infinite distributive law and the user wants to invoke pointless-topology machinery. `(c)` more often, because most marque constructions are complete lattices that fail the infinite distributive law — see entry 3 and entry 9.

> **When this comes up.** A user describes a complete lattice of "policies that fire on a subject" or "open conditions on a marking" and asks whether arbitrary unions of these compose well with finite intersections. If the infinite distributive law holds — verify with a concrete pair `x` and family `(yᵢ)` — they have a frame and the catalog of frame-theoretic constructions (Heyting implication, sublocales, frame morphisms) becomes available. If it fails, redirect to entry 3.

---

## 2. Locale

**Definition.** A *locale* `X` is, definitionally, a frame `O(X)` viewed in the opposite category. Equivalently, the category `Loc` of locales is `Frmᵒᵖ`. A locale is "what the frame is the algebra of"; given a locale `X` we write `O(X)` for its frame, called the *frame of opens of X*. See `[nlab-locale]`, `[picado-pultr-2012]` ch. I §3, `[johnstone-stone-spaces]` ch. II.

**The pointless-topology framing.** Classical topology starts with a set of points and defines opens as a family of subsets. Locale theory starts with a frame of opens directly and lets the points (if any) be a derived notion. Some frames have "enough points" (they are *spatial*: they arise as `O(X)` for an actual topological space `X`); others do not. Constructive mathematics, intuitionistic logic, and topos theory routinely produce locales without enough points, which is part of why locale theory is the framework of choice in those settings.

For marque this almost never matters directly. The reason locales appear in this file at all is that *the dual-category framing flips the direction of morphisms*, which shows up in some constructions where you want to compose policies "the other way" (entry 4).

**Locale morphism** = continuous map of locales. A continuous map `f : X → Y` of locales is, by definition, a frame homomorphism `f^* : O(Y) → O(X)` going *the other way*. The arrow flip is what the duality buys you. See `[nlab-locale]`, `[johnstone-stone-spaces]` II.1.

**Citation.** `[picado-pultr-2012]` ch. I §3; `[johnstone-stone-spaces]` ch. II §1; `[nlab-locale]`; `[vickers-1989]` ch. 3.

**Consultant tags.** `(c)` mostly. The locale framing is rarely the right consulting answer for a marque question — the user almost never needs the dual-category machinery. If the question is genuinely "are my pointless opens a frame?", redirect to entry 1.

> **When this comes up.** A user asks "is there a notion of `open subspace` of a security domain that has no points?" or "I want to talk about classifications without committing to a specific document carrying them." The locale framing is what they are reaching for, but the operative algebraic content is the frame underneath. Recommend reading the frame entry; only invoke the locale duality if the user is doing something genuinely categorical.

---

## 3. Complete lattice vs. frame: the canonical non-example

**The diagnostic question.** Given a complete lattice `L`, when does it FAIL to be a frame?

Failure happens precisely when the infinite distributive law fails:

`x ∧ (⋁ᵢ yᵢ) ≠ ⋁ᵢ (x ∧ yᵢ)`  for some `x` and some family `(yᵢ)`.

Note that `x ∧ (⋁ᵢ yᵢ) ≥ ⋁ᵢ (x ∧ yᵢ)` always holds in a complete lattice (the right side is bounded above by `x ∧ (⋁ᵢ yᵢ)` because each `x ∧ yⱼ ≤ x ∧ (⋁ᵢ yᵢ)`). So the only direction that can fail is `≤`. The canonical failure: there is some `z` with `z ≤ x` and `z ≤ ⋁ᵢ yᵢ` but for no individual `yⱼ` is `z ≤ yⱼ`.

**Canonical non-example: the closed-set lattice of a non-discrete topological space.** Let `X = ℝ` with the usual topology. The lattice of *closed* subsets of `ℝ`, ordered by inclusion, is complete (arbitrary intersections of closed sets are closed; the join of a family of closed sets is the closure of the union). But finite meets do NOT distribute over arbitrary joins:

Take `x = {0}` (a single closed point) and the family `yₙ = [1/n, 1]` for `n = 1, 2, 3, ...`. The join in the closed-set lattice is `closure(⋃ₙ [1/n, 1]) = [0, 1]`. Then `x ∧ (⋁ₙ yₙ) = {0} ∩ [0, 1] = {0}`. But each `x ∧ yₙ = {0} ∩ [1/n, 1] = ∅`, so `⋁ₙ (x ∧ yₙ) = ∅`. The two are unequal.

The closed-set lattice is therefore a complete lattice (and even a complete distributive *finite-meets-and-finite-joins-distribute* one) but NOT a frame. The dual lattice — the lattice of *opens* — IS a frame; that is exactly the "open-set lattice" of entry 5. The asymmetry between opens and closeds is one of the central features of locale theory: opens form a frame, closeds form a *coframe* (the order-dual structure, where finite *joins* distribute over arbitrary *meets*).

See `[picado-pultr-2012]` ch. I, `[johnstone-stone-spaces]` II.1.3.

**Marque-shape canonical non-example.** A complete lattice of "all rewrites that could fire on a page" with `∧` = "rewrites that both fire" and `∨` = "rewrites where at least one fires" is *generally* not a frame. Suppose rewrite `r` triggers when *some* member of an infinite family `{r₁, r₂, ...}` triggers; meeting `r` against an unrelated rewrite `s` may give different answers depending on which `rₙ` happens to coincide with `s`'s firing condition. The infinite distributive law is the mathematical statement that "the meet of `s` with the union of triggers is the union of meets" — and that is exactly the property that fails when triggers interact in non-local ways. See entry 9 for the diagnostic.

**Citation.** `[picado-pultr-2012]` ch. I; `[johnstone-stone-spaces]` II.1.3; `[davey-priestley-2002]` §11.5.

**Consultant tags.** `(c)` heavily. This is the entry to cite when refusing to call a structure a frame.

> **When this comes up.** A user assumes their complete lattice is a frame because it is "complete and distributive in the finite case." Run the diagnostic: pick a concrete `x` and a concrete infinite family `(yᵢ)`, compute both sides, see whether they agree. If they don't, name what fails and tell the user which frame-theoretic constructions become unavailable (Heyting implication, sublocales) so they can revise the design. Do not silently assume the structure is a frame.

---

## 4. Frame morphism / locale morphism

**Definition (frame morphism).** A *frame homomorphism* `h : F → G` between frames is a function preserving:

- arbitrary joins: `h(⋁ᵢ xᵢ) = ⋁ᵢ h(xᵢ)`,
- finite meets: `h(x ∧ y) = h(x) ∧ h(y)`, and
- the top: `h(⊤) = ⊤`.

(The bottom `⊥` is the empty join, so its preservation is automatic from the first clause. The top `⊤` is the empty meet, so its preservation is automatic from the second; some authors list it separately for emphasis.)

A frame homomorphism is *not* required to preserve arbitrary meets; that is the point. The asymmetry between finite-meet preservation and arbitrary-join preservation is what makes frame morphisms different from complete-lattice morphisms (which would preserve both arbitrary meets and arbitrary joins).

See `[nlab-frame]`, `[picado-pultr-2012]` I.1.4, `[johnstone-stone-spaces]` II.1.

**Definition (locale morphism).** A *continuous map of locales* `f : X → Y` is, by definition, a frame homomorphism `f^* : O(Y) → O(X)` going in the opposite direction. The arrow reversal is what the dual category buys you; functorially this matches the way a continuous map between actual topological spaces gives an inverse-image map on opens.

Locale morphisms have a natural right adjoint `f_*` to the frame homomorphism `f^*`; the adjoint exists because `f^*` preserves arbitrary joins, which is the join-side of being join-continuous; see `[picado-pultr-2012]` II.5, `[johnstone-stone-spaces]` II.1.

**Citation.** `[nlab-frame]`; `[picado-pultr-2012]` I.1.4 and II.5; `[johnstone-stone-spaces]` II.1; `[vickers-1989]` ch. 3.

**Consultant tags.** `(a)` when a marque morphism between two policy structures preserves arbitrary unions and finite intersections — that is exactly a frame homomorphism. `(c)` more often: a marque morphism that "approximately preserves unions" or "preserves only finite unions" is not a frame morphism; name the failure rather than calling the morphism by the frame name.

> **When this comes up.** A user defines a function between two policy structures and wants to know "is this a structure-preserving map?" Walk through the three preservation laws. Most marque candidates preserve finite unions, finite intersections, top, and bottom — that is a *bounded-lattice* morphism (`pure-lattice.md` entry 15) but not a frame morphism. Frame morphism requires arbitrary joins; if the structures are not even frames, the question is moot.

---

## 5. Open-set lattice of a topological space

**Definition.** Let `(X, τ)` be a topological space, where `τ ⊆ P(X)` is the topology (the family of open subsets). The *open-set lattice* `(τ, ⊆)` is a frame:

- joins are unions: `⋁ᵢ Uᵢ = ⋃ᵢ Uᵢ`. Arbitrary unions of opens are open by the topology axiom.
- finite meets are intersections: `U ∧ V = U ∩ V`. Finite intersections of opens are open.
- top is `X`, bottom is `∅`.
- the infinite distributive law `U ∩ (⋃ᵢ Vᵢ) = ⋃ᵢ (U ∩ Vᵢ)` is *set-theoretic* distributivity, which holds for any family.

Every topological space `X` gives a frame `O(X)`. Going the other way is more delicate: every frame `F` has a *spectrum* (the set of frame morphisms `F → 2` with the topology induced by `F`); spatiality of `F` is the property that this round-trip recovers `F`. Spatial frames correspond to sober topological spaces; this is the *Stone duality for locales* extending Stone's classical duality for Boolean algebras to a duality between the categories of locales and (sober) topological spaces. See `[johnstone-stone-spaces]` ch. II §1, `[picado-pultr-2012]` ch. II.

**Why this is the motivating example.** Frame theory was developed precisely to abstract away "the set of points" from topological-space theory. The open-set lattice of a space is the canonical frame; the question "what is a frame?" is operationally answered by "anything with the algebraic properties of an open-set lattice." That is also why frames have all the niceties (Heyting implication, completeness) of topological lattice-of-opens.

**Citation.** `[picado-pultr-2012]` Examples I.1.5; `[johnstone-stone-spaces]` II.1; `[vickers-1989]` ch. 3; `[nlab-locale]`.

**Consultant tags.** `(a)` when the user has something genuinely "topology-shaped" — opens on some space — and wants to invoke Heyting algebra / locale theory.

> **When this comes up.** A user describes a domain where "the things we union are some kind of open conditions, and the meet is a refinement that respects topology." If the conditions form a topology in the technical sense, the open-set lattice is a frame and the user has access to the full toolkit. If the structure is more like a closed-set lattice (closed under arbitrary meets but not arbitrary joins), pivot to the *coframe* structure, which is order-dual, or admit the structure is just a complete lattice (entry 3).

---

## 6. Sublocales and sub-frames

**Sub-frame.** A *sub-frame* of a frame `F` is a subset `S ⊆ F` closed under arbitrary joins (including the empty join, so `⊥ ∈ S`) and finite meets (including the empty meet, so `⊤ ∈ S`). Equivalently, the inclusion `S ↪ F` is a frame homomorphism. See `[picado-pultr-2012]` II.2, `[johnstone-stone-spaces]` II.2.

**Sublocale.** Sublocales of a locale `X` are NOT in bijection with sub-frames of `O(X)`. Instead, a sublocale corresponds to a *frame quotient* of `O(X)` (a surjective frame homomorphism `O(X) ↠ Q`), or equivalently to a *nucleus* on `O(X)` — a closure operator `j : O(X) → O(X)` with `j(x ∧ y) = j(x) ∧ j(y)`. The fixed points of `j` form the frame `Q`. See `[picado-pultr-2012]` ch. III, `[johnstone-stone-spaces]` II.2.

The asymmetry — sublocales correspond to quotients, not sub-objects — is one of the more subtle features of locale theory and is why locale theory's formalism is more expressive than naïve "sub-frames everywhere" thinking.

**Marque relevance.** The closure-operator framing of sublocales is structurally the same shape as the closure operators in `pure-lattice.md` entry 18 — fixed points form a complete lattice. If a user describes a closure-operator that respects finite meets, they have the data of a *nucleus*, which is the same as a sublocale.

**Citation.** `[picado-pultr-2012]` chs. II–III; `[johnstone-stone-spaces]` II.2; `[nlab-locale]`.

**Consultant tags.** `(a)` for nuclei when the user describes a closure operator preserving finite meets. `(c)` for "I have a sub-frame" when in fact they have a sublocale (or vice versa); name the asymmetry.

> **When this comes up.** A user describes a "subspace" of a policy domain and wants to know what algebraic structure that subspace inherits. If the subspace is given by a closure operator that respects intersection, name it as a nucleus/sublocale. If the subspace is just "a subset closed under the operations," it is a sub-frame. Different question, different answer.

---

## 7. Frame-of-policies framing

**The pattern (mostly a pointer).** In some literature on access-control and information-flow lattices, the lattice of *policies* on a subject is modeled as a frame. The intuition is: a policy is a "set of permitted operations" — closed under arbitrary union (any subset of permitted operations is itself a policy) and finite intersection (the conjunction of two policies is permitted operations under both).

This framing is most explicit in semantic-security / topos-theoretic treatments of information flow. The classical access-control lattices of `[denning-1976]` (cited in `pure-lattice.md` and `marque-applied.md`) and Bell-LaPadula are usually finite distributive lattices, which are trivially frames; but the frame perspective is invoked when one wants to talk about *infinitary* compositions of policies.

**Practical caveat for marque.** Most marque constructions are NOT frames. The reason is the same one as in entry 3: the structures involve unions of "things that interact non-locally" (compartments, dissem controls, FGI scopes), and the infinite distributive law fails. A marque-internal `Lattice` instance — `SciSet`, `SarSet`, `FgiSet` — almost never has the property that meeting against an arbitrary union equals the union of pairwise meets. Don't reach for the frame name lightly.

**Citation.** `[picado-pultr-2012]` ch. I (general framework); `[denning-1976]` (the classical lattice access-control reference, not framed as locales but the historical root); `[vickers-1989]` ch. 3 (logic-of-finite-observations framing, which is the right philosophical entry point if a user is reaching for the locale framing for security policies).

**Consultant tags.** `(c)` very often. The frame-of-policies framing sounds plausible and is sometimes pursued in the academic literature, but for marque-shape constructions it usually doesn't deliver; the user almost certainly has a complete lattice that is not a frame.

> **When this comes up.** A user reads about pointless topology and wonders if their security-policy lattice is a frame. Walk them through the diagnostic in entry 9 first. If it passes, great — they have access to Heyting implication and sublocale machinery. If it fails (the typical case), name the failure and pivot to the complete-lattice catalog in `pure-lattice.md`.

---

## 8. Subframe-of-a-product

**Product of frames.** Given frames `F` and `G`, the product poset `F × G` (with coordinatewise order) is itself a frame, with coordinatewise joins and meets. The infinite distributive law lifts coordinatewise. See `[picado-pultr-2012]` I.2, `[davey-priestley-2002]` §11.5.

**Subframe of a product.** A subset `S ⊆ F × G` closed under arbitrary joins and finite meets is a sub-frame of `F × G`. Some natural constructions land here: e.g., the lattice of pairs `(U, V)` with `U ⊆ V` (a "linked" product) is a sub-frame of `O(X) × O(X)`. See `[johnstone-stone-spaces]` ch. II.

**Caution about combining structure.** When a user says "I want to take two of my marque lattices and compose them," the question is which categorical operation they mean. The product `F × G` always exists in the lattice category, but if `F` and `G` are not frames, the product is not a frame either; it is at most a complete lattice. The correct question is at the level of `pure-lattice.md` entry 11 (product lattice), with the additional observation that if both factors *are* frames, the product is too.

**Citation.** `[picado-pultr-2012]` I.2; `[davey-priestley-2002]` §11.5; `[johnstone-stone-spaces]` II.

**Consultant tags.** `(a)` when both factors are confirmed frames and the user wants product-of-frames machinery. `(b)` when the user wants product-of-lattices machinery; redirect to `pure-lattice.md` entry 11.

> **When this comes up.** A user composes two domains and asks whether the composition has frame structure. Verify both factors are frames (entry 1 plus the diagnostic in entry 9), then name the product as a frame. If even one factor fails the infinite distributive law, the product fails too.

---

## 9. Diagnosis: "Is this construction a frame?"

A short procedure the consultant can run on a candidate marque construction. Each step is either "passes — proceed" or "fails — stop and name the failure."

**Step 1: Is it a poset?** Reflexive, antisymmetric, transitive. (See `pure-lattice.md` entry 1.) If preorder only — e.g., a "policy precedence" relation that is reflexive and transitive but allows `x ≤ y ≤ x` without `x = y` — stop. Frame theory does not extend cleanly to preorders without a quotient.

**Step 2: Is it complete?** Every subset has a join. (See `pure-lattice.md` entry 5.) For finite-domain marque constructions this is automatic; for open-vocabulary domains (extensible compartments, agency-extensible SAR programs, partner-national codes) the answer can be no. If incomplete, stop — frame theory requires completeness. Pivot to "join-semilattice" (see `pure-lattice.md` entry 2) or "complete partial order" (see `pure-lattice.md` entry 23 / `abstract-interp.md`).

**Step 3: Are finite meets defined?** For each pair `x, y`, does `x ∧ y` exist? (Recovered from completeness via `x ∧ y = ⋁{z : z ≤ x ∧ z ≤ y}`.) Generally yes if completeness holds. If meets are not well-defined — e.g., the construction is a join-semilattice with no natural meet — stop. The user has a complete join-semilattice, not a complete lattice.

**Step 4: Does finite-meet distribute over finite-join?** `x ∧ (y ∨ z) = (x ∧ y) ∨ (x ∧ z)` for all `x, y, z`. (See `pure-lattice.md` entry 6.) If this fails — e.g., the lattice contains an `M₃` or `N₅` sub-lattice — stop. The structure is non-distributive; certainly not a frame. Pivot to `pure-lattice.md` entry 7 (modular) or just "complete non-distributive lattice."

**Step 5: Does finite-meet distribute over ARBITRARY joins?** This is the crucial step.

`x ∧ (⋁ᵢ yᵢ) = ⋁ᵢ (x ∧ yᵢ)` for all `x` and all small families `(yᵢ)ᵢ∈I`.

This is *strictly stronger* than step 4. A complete distributive lattice can satisfy step 4 but fail step 5 — the closed-set lattice of `ℝ` is exactly such a construction (entry 3). The classical proof technique: pick a witness `z` with `z ≤ x` and `z ≤ ⋁ᵢ yᵢ` such that for no individual `yⱼ` is `z ≤ yⱼ`. If you can find such a `z`, the law fails.

For marque constructions, the failure typically comes from non-local interactions between members of an infinite (or large finite) family. Test on a concrete instance: pick three or four candidate elements `x, y₁, y₂, y₃` and compute both sides of the distributive law explicitly. If any are unequal, the structure is not a frame.

**Step 6: If step 5 passes — name the frame.** The structure is a frame. By the equivalence with complete Heyting algebras, the user automatically has Heyting implication. They can talk about sublocales (entry 6), frame morphisms (entry 4), and combine with other frames via products (entry 8). Most of the catalog in `[picado-pultr-2012]` and `[johnstone-stone-spaces]` becomes available.

**Step 6': If step 5 fails — name what they have.** "You have a complete lattice. You have finite distributivity. You do NOT have infinite distributivity. The frame name is wrong; here is what you can and cannot invoke." See `pure-lattice.md` entries 5 (complete lattice) and 6 (distributive lattice) for the right vocabulary. Specifically:

- *Cannot* invoke: Heyting implication exists for *every* element (the right adjoint `x ∧ (–)` exists in a complete lattice only if the join-distributivity holds; the partial implication exists only on the elements where the adjoint formula gives an answer).
- *Cannot* invoke: sublocales / nuclei (the structural theory needs the frame law to bind closure to meet preservation).
- *Cannot* invoke: spatiality / Stone duality for locales.
- *Can* still invoke: Knaster-Tarski (only needs completeness; see `pure-lattice.md` entry 19), closure operators (the abstract version; entry 18), the Galois-connection toolkit (`pure-lattice.md` entry 17 and `abstract-interp.md`).

**Citation.** `[picado-pultr-2012]` ch. I; `[johnstone-stone-spaces]` II.1.

**Consultant tags.** This whole entry is `(c)`-flavored — it is the structured refusal to call something a frame.

> **When this comes up.** A user asks "is this a frame?" Run the six-step diagnostic. Most marque candidates fail step 5. When they fail, do not negotiate — name the failure and redirect to the complete-lattice catalog.

---

## How to read the consultant tags

- `(a)` order-theory-adapted: the catalog entry is the right name for what the user has. Apply directly.
- `(b)` pivot toward a known pattern: the user's structure isn't quite the entry, but a redesign would land it inside; name the trade-off.
- `(c)` refuse / redirect: the entry is what the user thinks they have but doesn't; the right answer is to name the gap and point elsewhere.

Most entries in this file are `(c)`-flavored. That is by design: this file's main job is to gatekeep the frame name. When a marque construction genuinely is a frame, you'll know — entry 9 step 5 passes, and the entry 1 / entry 6 machinery is unlocked. The rest of the time, the right consulting answer lives back in `pure-lattice.md`.

---

*Bibliography entries appended to `references/bibliography.md`. Vendored sources (none added by this file beyond what Agent A has) noted in `sources/SOURCES.md`.*
