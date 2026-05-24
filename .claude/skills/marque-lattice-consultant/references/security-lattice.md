# Security & Information-Flow Lattices — Catalog Reference

**Audience.** Claude, in lattice-consultant mode, scanning for the closest named construction matching a problem the user just described in informal English. The user's domain is U.S. Government classification markings (CAPCO/ISM-v2022-DEC, with a roadmap into CUI, NATO, FGI, JOINT). This file is the heaviest-lifting catalog in the skill — most marque shape-questions about classification policy will resolve against an entry here.

**How to use.** Locate the catalog entry whose definition matches (or nearly matches) the construction in front of you. Each entry tags which consultant outcome it most often supports — `(a)` order-theory-adapted, `(b)` pivot toward a known pattern, `(c)` refuse / redirect — and ends with a "When this comes up" hook so you can route to it from a question shape.

**Companion files in this skill.**
- `pure-lattice.md` (Agent A) — pure lattice / order theory: posets, lattices, complete/distributive/modular lattices, free lattices, Galois connections, Knaster-Tarski. The algebraic substrate every entry below sits on.
- `abstract-interp.md` (Agent C, planned) — Galois connections, fixed-point iteration, abstract interpretation. Useful for "is iteration to a fixed point what we need?" questions.
- `marque-applied.md` (Agent E) — translates the entries here into the marque-specific design questions about ISM/CAPCO/JOINT/SCI/SAR/REL TO.

**Caveats.**
- Many of the original sources are vendored as paywalled-but-archival; we cite-and-link, paraphrase, and quote sparingly per the skill's sources policy.
- The 1973 Bell-LaPadula MITRE technical reports (MTR-2547 and MTR-2997) and the 1977 Biba MITRE TR are public-domain DTIC documents but are scanned to image-PDFs, so quotes here are paraphrased from secondary sources (Sandhu, Bishop, Anderson) cross-checking the same definitions.

---

## Table of Contents

1. [Denning's classification-with-categories lattice](#1-dennings-classification-with-categories-lattice) — the seminal levels × powerset(categories) lattice; everything else generalizes from this.
2. [Bell-LaPadula as lattice constraints](#2-bell-lapadula-as-lattice-constraints) — simple-security ("no read up") and ⋆-property ("no write down") as monotonicity constraints over Denning's lattice.
3. [Biba integrity model](#3-biba-integrity-model) — the dual lattice. Confidentiality flips into integrity.
4. [Lipner's combined model](#4-lipners-combined-model) — confidentiality lattice × integrity lattice; the engineering case for product structure.
5. [Need-to-know / compartmented mode](#5-need-to-know--compartmented-mode) — the categories component as an antichain that adds discriminating power without changing the level order.
6. [Supersession algebra (NOFORN-style)](#6-supersession-algebra-noforn-style) — when one element collapses related operations to ⊥; absorbing-element vs quotient vs non-lattice override.
7. [Intersection-with-blackball (REL TO style)](#7-intersection-with-blackball-rel-to-style) — meet of country sets with a single null-cell that absorbs everything.
8. [Declassification / exemption orderings](#8-declassification--exemption-orderings) — max-date join, with discrete exemption codes layered over.
9. [Sandhu's lattice-based access control (LBAC)](#9-sandhus-lattice-based-access-control-lbac) — the unifying framework that recovers BLP, Biba, Lipner, and CW as instances.
10. [Chinese Wall / Brewer-Nash](#10-chinese-wall--brewer-nash) — the cautionary entry. CW's COI structure is a forest-of-antichains, not a lattice; Sandhu's recasting is what makes it lattice-shaped.
11. [RBAC role hierarchy as poset / lattice](#11-rbac-role-hierarchy-as-poset--lattice) — NIST ANSI INCITS 359-2004; senior-most-role join.
12. [Decentralized Label Model (DLM)](#12-decentralized-label-model-dlm) — labels = (owner → readers) policies; pre-order whose equivalence classes form a distributive lattice.
13. [Information-flow type systems](#13-information-flow-type-systems) — Volpano-Smith, FlowCaml, Jif. Type systems whose security types form a lattice; soundness via non-interference.
14. [Non-interference (Goguen-Meseguer)](#14-non-interference-goguen-meseguer) — the formal definition of "secure information flow" relative to a lattice. The "what soundness means" reference.
15. [Multi-level / multi-lateral / multi-policy lattices](#15-multi-level--multi-lateral--multi-policy-lattices) — Anderson's framing. When one lattice doesn't capture policy and you compose lattices.
16. [JOINT / coalition lattices](#16-joint--coalition-lattices) — two-or-more-organization joint markings; product lattice with intersection of release rules.
17. [Foreign-government-information (FGI) and tetragraph lattices](#17-foreign-government-information-fgi-and-tetragraph-lattices) — country sets with named tetragraph subsets (NATO, FVEY, ACGU, TEYE) as canonical joins.
18. [SCI / compartmented information as a hierarchical lattice](#18-sci--compartmented-information-as-a-hierarchical-lattice) — control system × compartments × sub-compartments; agency-extensibility ⇒ no top.
19. [SAR / Special Access Required as a forest of programs](#19-sar--special-access-required-as-a-forest-of-programs) — programs and sub-programs form a forest (poset) but not a bounded lattice without a least-upper-bound construction.
20. [Audit / log lattices and provenance](#20-audit--log-lattices-and-provenance) — provenance graphs as DAGs, lattice operations along paths.

[When a security policy is NOT a lattice](#when-a-security-policy-is-not-a-lattice) — the diagnostic section. High-value (c) consultant material.

[How to read the consultant tags](#how-to-read-the-consultant-tags)

---

## 1. Denning's classification-with-categories lattice

**Definition.** Denning [denning-1976] models secure information flow as a 5-tuple `⟨N, P, SC, ⊕, →⟩`: objects `N`, processes `P`, security classes `SC`, a binary class-combining operator `⊕: SC × SC → SC`, and a flow relation `→ ⊆ SC × SC` (read `a → b` as "information may flow from class `a` to class `b`"). When `SC` is finite and `→` is a partial order with greatest lower bounds and least upper bounds (the latter computed by `⊕`), Denning calls `(SC, →, ⊕, ⊗)` an *information-flow lattice*.

The four conditions Denning identifies as making `SC` a finite lattice (the "Denning axioms," sometimes phrased differently across sources; the version below tracks Sandhu's restatement [sandhu-1993-lbac]):

1. `SC` is finite.
2. `→` is a partial order on `SC` (reflexive, antisymmetric, transitive).
3. `SC` has a least element (lower bound) under `→`.
4. `⊕` is total and is the least upper bound: `a ⊕ b` is the least `c` with `a → c` and `b → c`.

Conditions 1–4 imply existence of a greatest lower bound `⊗` (meet) and a top element. Hence `(SC, →, ⊕, ⊗, ⊥, ⊤)` is a finite bounded lattice. See `pure-lattice.md` entries 4 (bounded lattice) and 5 (complete lattice).

**Canonical construction.** The U.S. military / IC instance: `SC = L × P(C)` where `L` is the totally ordered set of clearance levels (`UNCLASSIFIED < CONFIDENTIAL < SECRET < TOP SECRET`) and `C` is the set of categories (compartments). The order is componentwise:

- `(ℓ₁, K₁) ≤ (ℓ₂, K₂)` iff `ℓ₁ ≤ ℓ₂` (in `L`) and `K₁ ⊆ K₂` (in `P(C)`).

This is the *product lattice* of the totally-ordered level chain and the powerset Boolean algebra over categories. The join is `(max(ℓ₁, ℓ₂), K₁ ∪ K₂)`; the meet is `(min(ℓ₁, ℓ₂), K₁ ∩ K₂)`. The bottom is `(UNCLASSIFIED, ∅)`; the top is `(TOP SECRET, C)`. See `pure-lattice.md` entry 11 (product lattice) and entry 8 (Boolean lattice).

**Laws.** Inherits all bounded distributive lattice laws from the product of a chain (totally ordered, hence distributive) with `P(C)` (Boolean, hence distributive). Both operations are commutative, associative, idempotent, absorbing.

**Example.** For two-level military system `L = {U, C, S, TS}` (totally ordered) and categories `C = {CRYPTO, SIGINT, NUCLEAR}` (independent, antichain), `SC` has `4 × 2³ = 32` distinct labels. `(SECRET, {SIGINT}) ∨ (CONFIDENTIAL, {CRYPTO}) = (SECRET, {CRYPTO, SIGINT})`. `(TOP SECRET, {CRYPTO}) ∧ (SECRET, {CRYPTO, SIGINT}) = (SECRET, {CRYPTO})`.

**Non-example.** A "level" set that is *not* totally ordered (e.g., independent levels with no comparability) — the resulting product is still a lattice, but the level component is a Boolean algebra rather than a chain. That's fine if intentional, but Denning's military instance specifically depends on the chain structure of `L`.

**Citation.** [denning-1976] §2 for the 5-tuple and the four-axiom statement. [sandhu-1993-lbac] §"Denning's Axioms" for the Hasse-diagram restatement and proof that the axioms imply a finite lattice. [bishop-2018] App. 34 (lattice background) and Ch. 5 (information flow). [anderson-securityengineering] Ch. 9 §"Compartmentation and the lattice model."

**Consultant tags.** `(a)` primary — when the user describes "levels and categories," this is the construction. `(b)` — if the user's "levels" are not totally ordered (e.g., a partial order of integrity tiers), redesign as a product of the actual level poset with `P(categories)`; still a lattice, but no longer a chain × Boolean. `(c)` — if `→` is not antisymmetric (e.g., bidirectional flows), this isn't even a poset; recommend a graph-theory framing.

> **When this comes up.** A user asks "we have classification levels (UNCLASSIFIED through TOP SECRET) plus compartments — is there a formal name for the structure?" Answer: it's the canonical Denning-1976 lattice, the product of a chain and a Boolean algebra. Every other entry in this file specializes, generalizes, or contrasts with this one.

---

## 2. Bell-LaPadula as lattice constraints

**Definition.** Bell-LaPadula (BLP) [bell-lapadula-1973] is a state-machine security model layered on top of a Denning-style lattice. Subjects `S` and objects `O` each carry a security class drawn from the lattice `SC`. The two mandatory access-control rules are stated as constraints on subject/object class pairs:

- **Simple-security property ("no read up").** Subject `s` may read object `o` only if `class(s) ≥ class(o)` in the lattice (i.e., `s`'s clearance dominates `o`'s classification). [bell-lapadula-1973] Vol. I.
- **⋆-property ("no write down").** Subject `s` (not in the trusted set) may write object `o` only if `class(s) ≤ class(o)` (i.e., the subject's *current* level is dominated by the object's). The ⋆-property is what prevents a high-cleared subject from declassifying via copy.

A system is "secure" iff every reachable state satisfies both properties (plus a discretionary-security rule). The Basic Security Theorem [bell-lapadula-1975-unified] says: if the rules of state transition all preserve simple-security, ⋆-property, and discretionary-security, and the initial state satisfies them, then every reachable state does too. (See [csl-rushby-blp] for a careful exposition.)

**Lattice connection.** The properties express *monotonicity* with respect to the lattice order. `class` is a labelling function; the properties say that legal `(subject, object)` pairs must lie in particular up-sets / down-sets of the lattice. In information-flow terms, if `info` flowed `o → s → o'`, the simple-security and ⋆-property together force `class(o) ≤ class(o')`, which is the Denning information-flow rule.

**Laws / what BLP gives you.** Soundness with respect to "no information flows downward in the lattice." What it does *not* directly capture: covert channels (timing, storage), aggregation problems (low-classified facts that combine to higher-classified inference), or the difference between "information flowed" and "subject was permitted to read." Those weaknesses motivated noninterference (entry 14) and the language-based work (entry 13).

**Example.** Subject `s` cleared `(SECRET, {SIGINT})`. Object `o₁` classified `(CONFIDENTIAL, ∅)`. `o₂` classified `(SECRET, {SIGINT})`. `o₃` classified `(SECRET, {CRYPTO})`. `s` may read `o₁` (`SECRET ≥ CONFIDENTIAL` and `{SIGINT} ⊇ ∅`) and `o₂` (equal); `s` may *not* read `o₃` (`{SIGINT} ⊉ {CRYPTO}` — incomparable). Symmetrically for writes via ⋆-property.

**Non-example.** "Trusted subjects" (Bell-LaPadula's escape hatch) are explicitly *not* bound by the ⋆-property. They can break the lattice constraints because they're trusted to do so safely. This is the primary critique that motivated Biba (which keeps the lattice but inverts the direction) and Clark-Wilson / Lipner (which add transactional discipline).

**Citation.** [bell-lapadula-1973] MTR-2547 Vol. I (Mathematical Foundations) for the original simple-security and ⋆-property. [bell-lapadula-1975-unified] MTR-2997 (Unified Exposition and Multics Interpretation) for the Basic Security Theorem and Schiller's "current security level" simplification of ⋆-property. [csl-rushby-blp] for a clean restatement. [bell-2005-looking-back] for a retrospective discussion of the boolean-lattice generalization.

**Consultant tags.** `(a)` primary — when the user is reasoning about "who can read what" in a level/compartment system, frame the rule as "monotonicity over the Denning lattice." `(b)` — if the user wants more than confidentiality (e.g., integrity), add Biba (entry 3) and combine in Lipner-style (entry 4).

> **When this comes up.** A user asks whether a clearance check is "really" a lattice operation. Answer: yes — `class(subject) ≥ class(object)` is the lattice's `≥` relation, and the simple-security rule is just that relation. This is the most basic shape-question the consultant gets and it almost always resolves to "Denning + BLP." If the user is confused about which direction "no read up" runs, walk them through the lattice diagram.

---

## 3. Biba integrity model

**Definition.** Biba [biba-1977] models *integrity* with the same lattice machinery as BLP, but with the order reversed. Subjects and objects carry an integrity class in a lattice `(IC, ≤_I)`. Strict Integrity Policy:

- **Simple-integrity property ("no read down").** Subject `s` may read object `o` only if `i(s) ≤_I i(o)` (subject can only read objects of equal or higher integrity).
- **Integrity ⋆-property ("no write up").** Subject `s` may write object `o` only if `i(o) ≤_I i(s)` (subject can only write objects of equal or lower integrity).

Biba also defines weaker policies (Subject Low-Water-Mark, Object Low-Water-Mark, Ring), but Strict Integrity is "the Biba model" in colloquial use.

**Lattice connection.** Biba's lattice `IC` is structurally identical to BLP's `SC` — typically a chain × powerset product. The *only* difference is direction. Where BLP says high subjects can read low and write high (information flows up), Biba says high subjects can read high and write low (integrity flows down). A formal way to state it: the integrity lattice is the *order dual* of the confidentiality lattice — see `pure-lattice.md` entry 13 (dual lattice). If `(L, ≤)` is the confidentiality lattice, then `(L, ≥) = L^op` is the integrity lattice, with the same elements but inverted order.

The intuition Sandhu draws out [sandhu-1993-lbac] §"Integrity Lattices": both confidentiality and integrity are "no information flow down in the relevant order." For confidentiality "down" means toward less-classified; for integrity "down" means toward less-trusted. Same machinery, different semantics.

**Laws.** Inherits everything from the underlying lattice. Same product-of-chain-and-powerset structure when integrity has tiers + categories.

**Example.** Integrity tiers `IC = {Untrusted, User, Programmer, Administrator, System}` (chain). Subject `s` at `Programmer` can read `Administrator`-trusted code (it's safe to consume) but cannot write to it (would risk corrupting it). Subject can write to `User` files (downgrade is allowed if `s` is willing to lower its own confidence). Symmetric to BLP modulo direction.

**Non-example.** Biba's policy is *not* directly comparable to confidentiality in the same model — confidentiality and integrity are orthogonal axes. They do not share labels and do not constrain each other. To enforce both simultaneously, you compose lattices (see Lipner, entry 4).

**Citation.** [biba-1977] MITRE TR ESD-TR-76-372, available DTIC AD-A039324. [sandhu-1993-lbac] §"Integrity Lattices" for the dual-lattice framing and the equivalence of "Biba lattice" with "BLP lattice with order inverted." [bishop-2018] §6.2.

**Consultant tags.** `(a)` primary — when the user wants to model "who can corrupt what," recommend the Biba dual structure. `(b)` — if the user has BLP working and wants integrity bolted on, recommend Lipner-style product (entry 4) rather than a stand-alone Biba lattice. `(c)` — if the user proposes "integrity is just the same as confidentiality, we'll use one lattice for both," correct: they're duals on dual orders. Conflating them yields a model where high-confidentiality is also high-integrity, which is the wrong invariant.

> **When this comes up.** A user describes integrity tiers and asks if it's a "second classification system." Frame it as: same lattice machinery as Denning, but the order runs the other way. Cite Biba [biba-1977] for the formal model and Sandhu [sandhu-1993-lbac] for the dual-lattice unification. If they want both confidentiality *and* integrity, route to Lipner (entry 4).

---

## 4. Lipner's combined model

**Definition.** Lipner [lipner-1982] proposed combining BLP confidentiality with Biba integrity for commercial use cases (the canonical example being a separation between development, test, audit, and production code). Each subject/object carries *two* labels: a confidentiality label `h ∈ H` and an integrity label `w ∈ W`. The combined mandatory rules:

- Subject `s` may read object `o` iff `h(s) ≥ h(o)` (BLP) **and** `w(s) ≤ w(o)` (Biba simple-integrity).
- Subject `s` may write object `o` iff `h(s) ≤ h(o)` (BLP ⋆-property) **and** `w(s) ≥ w(o)` (Biba ⋆-integrity).

**Lattice connection.** The combined label space is the *product lattice* `H × W^op` — the confidentiality lattice times the dual of the integrity lattice. The two access rules are exactly: read iff label of subject dominates label of object in `H × W^op`; write iff dominated. So Lipner's model is just BLP applied to the product `H × W^op`. See `pure-lattice.md` entry 11 (product lattice).

Sandhu [sandhu-1993-lbac] computes the size of Lipner's specific instance: 3 integrity levels × 2 integrity categories × 2 confidentiality levels × 2³ confidentiality categories = 192 distinct labels, of which Lipner uses only 9. The 9-label restriction is a *sublattice* (entry 14 in `pure-lattice.md`) of the full 192-label lattice, chosen because most of the 192 labels have no operational meaning.

**Laws.** Distributive bounded lattice (product of two distributive bounded lattices). Inherits all the absorption / commutativity / associativity laws.

**Example.** Two confidentiality levels {Public, Confidential} × one confidentiality category {Audit} × three integrity tiers {Untrusted, Production, System}. A development tool would sit at low integrity (`w = Untrusted`) so that it cannot write production code (`w = Production`) — Biba ⋆-integrity. Audit log sits at high confidentiality (`h = (Confidential, {Audit})`) and high integrity simultaneously, restricted to system managers.

**Non-example.** Trying to "merge" confidentiality and integrity into a single label that runs from "low-everything" to "high-everything" — that conflates the two orthogonal axes and produces nonsensical rules (e.g., that `Production` integrity is "more secret" than `Untrusted`, which it isn't). The product structure preserves orthogonality; merging breaks it.

**Citation.** [lipner-1982] for the original construction. [sandhu-1993-lbac] §"The Lipner Lattice" for the lattice-product analysis and the Lipner-as-BLP-on-product equivalence. [anderson-securityengineering] Ch. 9.

**Consultant tags.** `(a)` primary — when the user wants both confidentiality and integrity, this is the standard product. `(b)` — if the user has insisted on a single integrated lattice, propose Lipner's product-with-projection as the way to keep the two axes separable (and propose a sublattice of the 192-label space if their actual operational labels are fewer than the full product). `(c)` — if the user's "integrity" is order-incompatible with Biba (e.g., context-dependent trust that doesn't form a partial order), Lipner doesn't help and the right answer is to model integrity differently, possibly via Clark-Wilson transactions outside the lattice framework.

> **When this comes up.** A user asks "can we combine BLP confidentiality with Biba integrity in one system?" Answer: yes, and the result is a product lattice `H × W^op` (with Biba's order dualized so that "dominate" means the same on both axes). Cite Lipner [lipner-1982] and the Sandhu unification [sandhu-1993-lbac]. The 192-vs-9 sublattice point is worth raising — the full product is rarely all needed in practice.

---

## 5. Need-to-know / compartmented mode

**Definition.** "Need-to-know" is the operational principle that a person must have a specific business reason (not just a clearance) to access information. Formalized in Denning-style lattices via the *categories* (compartments) component: a clearance `(SECRET, {SIGINT})` says you may access SIGINT, but does not say you may access CRYPTO even though both are SECRET. The category set `C` is treated as an *antichain* — categories are mutually independent and non-comparable.

**Lattice connection.** The categories component contributes a Boolean algebra `(P(C), ⊆, ∩, ∪, ∅, C)` to the product lattice `L × P(C)`. Independence among categories means the categories themselves form an antichain in the underlying generator set, but the *subsets* of `C` (the actual labels) form a Boolean algebra under inclusion. See `pure-lattice.md` entry 8 (Boolean algebra). The set inclusion order on subsets is what gives need-to-know its lattice structure: more categories ⇒ stricter, fewer ⇒ more permissive at fixed level.

**Why it's separate from levels.** A common confusion is to treat categories as "more levels." They're not — they're an *orthogonal* axis. Adding a category never changes the level; adding a level never changes the category set. The product structure formally encodes this orthogonality.

**Laws.** Pure Boolean algebra: complemented, distributive, two-element atoms. Each category corresponds to an atom of the Boolean algebra `P(C)`.

**Example.** Categories `C = {SI, TK, HCS}` (toy SCI compartment names). Two clearances:
- `(SECRET, {SI, TK})` — can read SI and TK material at secret or below.
- `(SECRET, {SI, HCS})` — can read SI and HCS material at secret or below.
Their meet is `(SECRET, {SI})` — the common ground. Their join is `(SECRET, {SI, TK, HCS})` — the "if both subjects pooled" cap. Operationally, the meet is "what both subjects can see" (intersection) and the join is the "minimum clearance to dominate both" (union).

**Non-example.** Treating categories as a chain (totally ordered) — that is the wrong mathematical structure for need-to-know. There's no "more SIGINT-ish than CRYPTO." Someone proposing a chain is conflating need-to-know with level.

**Citation.** [denning-1976] for the original formulation. [sandhu-1993-lbac] §"Lattices with Categories." [anderson-securityengineering] Ch. 9 §"Compartmentation and the lattice model" for an excellent informal treatment. CAPCO/ISM treatment in DNI Authorized Classification and Control Markings Register [dni-capco-register].

**Consultant tags.** `(a)` primary — when the user has compartments / categories / need-to-know controls in their model, this is the construction (subsumed by entry 1, but worth surfacing separately). `(b)` — if the user is treating categories as additional levels, redirect: "categories are an antichain, levels are a chain; the product is what gives you Denning's lattice."

> **When this comes up.** A user asks why CRYPTO and SIGINT aren't ordered with each other. Answer: because they're independent compartments, modelled as antichain elements of the categories set `C`. The order lives in `P(C)` (set inclusion), not on the categories themselves. This routes to entries 1 and 17 (FGI-style country sets are structurally identical).

---

## 6. Supersession algebra (NOFORN-style)

**Definition.** A supersession marking is one that, when present, *overrides* or *clears* one or more related fields. The motivating example in the user's domain is CAPCO NOFORN ("Not Releasable to Foreign Nationals"): when a portion or banner carries NOFORN, any associated REL TO list is invalid (per CAPCO-2016 §F.4 — "REL TO shall not be used with NOFORN ... in the banner line"; if portions disagree, the banner uses NOFORN and the REL TO from the other portions is dropped, per [cdse-noforn-relto]). [Cite CAPCO-2016 directly for the precise wording; this is the user's domain.]

The algebraic question: is "page-level union of portion markings" a lattice operation when NOFORN is present?

**Lattice connection — three candidate framings.**

1. **NOFORN as absorbing element for REL TO meet.** If the page-level REL TO is computed as a *meet* (intersection of country sets), and any NOFORN portion forces the page-level REL TO to ∅, then NOFORN behaves as an absorbing element (a "zero" — see [wikipedia-absorbing-element]) for the REL TO meet operation. In a bounded lattice, the bottom is absorbing for meet. Modelling it this way: the REL TO domain is `P(Countries) ∪ {NOFORN}` where `NOFORN` is the new bottom and acts as `NOFORN ∩ X = NOFORN` for any `X`. This is a lattice — a Smyth-style "lifted" semilattice with an artificial bottom that absorbs.

2. **NOFORN as a quotient.** Interpret NOFORN as an equivalence-class collapse: any two REL TO sets are equivalent in the presence of NOFORN. This is a *lattice congruence* (`pure-lattice.md` entry 16) — formally, the equivalence relation `x ≡ y` iff "either neither has NOFORN and `x = y`, or both have NOFORN." The quotient is well-defined and is a lattice. Useful when you want NOFORN to literally erase REL TO from the data structure.

3. **NOFORN as a non-lattice override.** If "NOFORN clears REL TO" is implemented as a *side-effecting page-level rewrite* — e.g., "if any portion is NOFORN, drop REL TO from the banner" — and that rewrite doesn't compose associatively or commutatively with other page rewrites, then it's not a lattice operation at all. It's an algebraic operation on a poset, but the operation may fail idempotence (depending on the rewrite's interaction with prior state) or commutativity (if the order of rewrites matters).

The marque codebase's `PageRewrite` mechanism (per the project's `docs/plans/2026-04-19-recursive-lattice-and-decoder.md` and the `marque-engine::scheduler` topological order) is *explicitly* framing NOFORN-clears-REL-TO as a page-level rewrite, with a topological sort that defines a deterministic order — i.e., the third framing, with deterministic order substituted for the (failed) commutativity. The scheduler's existence suggests the project has accepted this is not a lattice operation but a posetal-rewrite operation requiring deterministic ordering.

**Laws — which hold in each framing.**

- Framing 1 (absorbing bottom): all lattice laws hold including idempotence and commutativity, but the resulting structure has an artificial bottom whose semantics ("NOFORN") is operationally distinct from "empty REL TO" — that may or may not be acceptable.
- Framing 2 (quotient): all lattice laws hold, but you've lost the ability to distinguish "NOFORN" from "any REL TO" — the equivalence has collapsed them.
- Framing 3 (non-lattice rewrite): need to specify the rewrite explicitly. Idempotence usually holds (applying the rewrite twice is the same as once); commutativity typically does *not* hold if multiple rewrites interact with the same axes; absorption against ∨ is undefined.

**Example.** Page has portions `(S//REL TO USA, FRA)` and `(S//NOFORN)`. In Framing 1, the page REL TO is `{USA, FRA} ∩ NOFORN = NOFORN`. In Framing 2, the two portions are equivalent in the quotient and the page banner picks NOFORN as the maximum-restrictive class. In Framing 3, you compute REL TO union first, then a NOFORN-detection rewrite drops REL TO from the banner.

**Non-example.** Treating "NOFORN supersedes REL TO" as a *meet* of REL TO with `{∅}` — that's mathematically the same as removing REL TO, but it conflates "no countries" (REL TO with empty list, syntactically forbidden) with "NOFORN" (which is a distinct marking with operational meaning). Don't.

**Citation.** [wikipedia-absorbing-element] for the formal "zero element" framework. [denning-1976] §3 for the original lattice constraint analysis. CAPCO-2016 §F (in user's vendored copy) for the operational rule. The marque project's plan `docs/plans/2026-04-19-recursive-lattice-and-decoder.md` for the rewrite-based engineering choice.

**Consultant tags.** `(a)` if the user is willing to accept "NOFORN is the bottom of a lifted REL TO semilattice," recommend Framing 1 (absorbing element). It's clean and lattice-shaped. `(b)` — Framing 3 (the rewrite-based approach the marque project chose) is the most operationally honest and the consultant should *recognize* it as not-a-lattice-but-poset-with-rewrite, so the user knows what they've signed up for: deterministic-order rewrite, not algebraic identity. `(c)` — if a user proposes "NOFORN is just another country," redirect: it's a meta-marking that *operates on* the country lattice, not an element of it.

> **When this comes up.** A user asks "we have a marking that wipes out a related field — what's the formal name?" Three answers depending on commitment: (a) absorbing bottom of a lifted lattice; (b) lattice quotient; (c) non-lattice algebraic rewrite. Walk them through which laws each preserves, and pick. The marque project chose (c), implemented via topological scheduler — that's a respectable choice but should be *named* as not-a-lattice so future maintainers don't expect lattice algebraic identities.

---

## 7. Intersection-with-blackball (REL TO style)

**Definition.** A "REL TO" marking lists countries authorized to receive a portion. The page-level REL TO across multiple portions is computed by *intersecting* the per-portion lists (a country must appear in every portion's REL TO to be on the page banner). A single portion that is *not* releasable to anyone — typically signalled by NOFORN, but conceptually any "blackball" — collapses the page-level REL TO to the empty set, which (by CAPCO syntax) means the banner is NOFORN, not "REL TO {}".

This is structurally the *intersection* (meet) operation in `(P(Countries), ⊆)`, with a special-case override when an absorbing element is present.

**Lattice connection.** Without the blackball, `P(Countries)` is a Boolean algebra (entry 5, entry 8 of `pure-lattice.md`) under intersection-as-meet and union-as-join. With NOFORN, see entry 6 above for the three options. In particular, "intersection across portions" *is* a lattice meet operation in the pure Boolean algebra; "intersection-with-blackball" is the absorbing-element-extended version of that meet.

**Why "blackball" is a useful mental model.** In the original sense (one black ball in a vote means rejection regardless of white balls), one NOFORN portion in a page means no foreign release regardless of how many friendly REL TO portions are present. This is the same logic as multiplying by zero: any value × 0 = 0. NOFORN is the zero of the REL TO semigroup. See [wikipedia-absorbing-element].

**Tetragraph wrinkle.** REL TO doesn't always operate on individual country trigraphs. Tetragraph codes (NATO, FVEY, ACGU, TEYE) are pre-defined country *sets* — e.g., FVEY = {USA, AUS, CAN, GBR, NZL}. Per CAPCO-2016, banner-level roll-up may *expand* tetragraphs to their constituent trigraphs to compute the intersection. The expansion is a homomorphism `tetragraph → P(trigraphs)` that commutes with the lattice meet. So `REL TO {FVEY} ∩ REL TO {USA, GBR}` after expansion is `{USA, AUS, CAN, GBR, NZL} ∩ {USA, GBR} = {USA, GBR}`. The marque crate has this expansion in `marque-capco::vocab` per the project's CLAUDE.md.

**Laws.** Pure Boolean algebra plus optional absorbing bottom. All lattice laws hold (idempotence, commutativity, associativity, absorption). Tetragraph expansion is a lattice homomorphism (`pure-lattice.md` entry 15) when defined as set membership.

**Example.** Three portions: `(S//REL TO USA, FRA, GBR)`, `(S//REL TO USA, GBR)`, `(S//REL TO USA, FRA)`. Page-level REL TO = `{USA, FRA, GBR} ∩ {USA, GBR} ∩ {USA, FRA} = {USA}`. Add a fourth portion `(S//NOFORN)`. Page-level REL TO = `{USA} ∩ NOFORN = NOFORN` (blackball absorbs).

**Non-example.** Computing the page REL TO as the *union* of portion REL TOs — that's wrong: union allows release to a country that some portion forbids. Lattice-wise, you want the meet (intersection), not the join (union).

**Citation.** [wikipedia-absorbing-element] for the absorbing-element formalism. [denning-1976] for the lattice operations. CAPCO-2016 §H (banner roll-up rules) for the operational REL TO intersection convention. [dni-capco-register] for tetragraph definitions.

**Consultant tags.** `(a)` primary — when the user describes "we intersect country lists across portions, and one bad portion can kill the whole list," the consultant should immediately name "meet operation in `P(Countries)` plus an absorbing element." Cite [denning-1976] for the meet, [wikipedia-absorbing-element] for the absorbing element, and entry 6 above for the NOFORN-specific case.

> **When this comes up.** A user asks for the formal name of "a marking that, when present, drops the country list to empty." That's an absorbing element (a "zero") for the meet operation. If they want just the country-intersection without the override, that's the meet of `P(Countries)`. Almost every IC release-control rule routes to one of these two.

---

## 8. Declassification / exemption orderings

**Definition.** A declassification marking attaches an event or date by which a portion may be downgraded. The page-level rule is: pick the *latest* (max) declassification date across all portions. This is `max` in the chain of dates ordered chronologically — a *join* in the totally ordered date lattice.

Exemptions complicate this. Per ISOO/CAPCO, certain exemption codes (50X1-HUM, 25X1-human, etc.) signal "do not declassify on schedule" — they *escape* the date-based ordering. The page-level rule with exemptions: an exemption-coded portion overrides the date computation, and the page banner shows the most-restrictive exemption (or the latest date, if no portion is exempt).

**Lattice connection.** Three composing structures:
1. **Date chain.** `(Dates ∪ {∞}, ≤)` is a totally ordered set; `max` is the join. ∞ is the top (never declassify). See `pure-lattice.md` entry 1 (poset) and entry 2 (semilattice).
2. **Exemption set.** `(Exemptions, ≼)` is a finite poset of exemption codes, often a flat antichain (no exemption is "more restrictive" than another, just different).
3. **Combined.** Lift the date chain by adjoining the exemption codes as new top elements: `Dates ∪ Exemptions ∪ {nothing}` with an order where every exemption dominates every date and exemptions are an antichain among themselves. This is a *bounded join-semilattice* with a top of "indefinitely classified" and a bottom of "declassified now." The join takes the max-date, replaced by the strictest exemption if one is present.

The composite is a join-semilattice but *not* a meet-semilattice in any operationally meaningful way: there's no natural "minimum" of two declassification dates that has compliance meaning. (Can ad-hoc define `min`, but no rule asks for it.) This is the case for join-semilattice (`pure-lattice.md` entry 2) without lattice closure — perfectly acceptable.

**Laws.** Join is associative, commutative, idempotent (max). No meet needed; treating the structure as a *meet-semilattice missing a top* is the wrong way to model it (entry 18 below covers the canonical "no top" case).

**Example.** Three portions: declassify dates `(2030-01-01)`, `(2025-06-15)`, exempt `50X1-HUM`. Page-level: 50X1-HUM dominates both dates ⇒ banner is exempt 50X1-HUM. Without the exemption, page-level is `max(2030, 2025) = 2030-01-01`.

**Non-example.** Treating exemptions as additional dates "in the future" — that conflates the chain structure (dates are continuous and comparable) with the antichain structure (exemptions are categorical). The composite needs to keep them separate.

**Citation.** [denning-1976] for max-as-join in chains. ISOO Marking Booklet [isoo-marking] for the operational declassification rules. CAPCO-2016 §G (declassification) for the precise exemption codes.

**Consultant tags.** `(a)` primary — when the user has "max-date wins" rules across portions, frame it as a join in the chain. `(b)` — when exemptions are added, frame as a join-semilattice with adjoined antichain-top. `(c)` — if user expects meet operations on dates, redirect: there's no operational meet here, only join.

> **When this comes up.** A user asks how to compute the page-level declassification banner from portions. Walk through: max-date is the join of a totally ordered date chain; exemptions sit above all dates as a flat antichain; the composite is a bounded join-semilattice. No meet needed; if the user thinks they need one, ask what operational rule they're trying to capture — they probably don't.

---

## 9. Sandhu's lattice-based access control (LBAC)

**Definition.** Sandhu [sandhu-1993-lbac] unifies BLP, Biba, Lipner, and Chinese Wall under a single framework: a *lattice-based access control model* is one where the security-class set is a finite lattice satisfying Denning's axioms, and access rules are stated as monotonicity constraints over the lattice.

Sandhu's framing recovers each prior model as an instance:

- BLP confidentiality: the standard Denning lattice with simple-security (≥) and ⋆-property (≤).
- Biba integrity: the dual of the BLP lattice, same rules.
- Lipner combined: the product `H × W^op` with read/write constraints projecting to each axis.
- Chinese Wall (Sandhu's reading; see entry 10): a lattice of (current-COI-class-set, access-history) pairs, where access rules are monotonicity over an artificially-constructed lattice that captures CW's history-dependence.

The deep claim of LBAC: any access-control model that prevents downward information flow can be cast as a lattice problem. Implication: if your policy resists this casting, it's *not* a downward-information-flow model — it's something else (typically a transactional / well-formed-transaction model like Clark-Wilson, which LBAC explicitly does not subsume).

**Lattice connection.** LBAC is the meta-construction. Every entry 1–8 above is an instance.

**Laws.** Whatever the underlying lattice provides. The framework adds the meta-rule: "subject's class dominates object's class for read; object's class dominates subject's for write." Soundness theorems (Sandhu §5) show this rule is sufficient to enforce information-flow non-interference, with caveats about covert channels.

**Example.** Bell-LaPadula = LBAC instance with `SC = L × P(C)` and the dominance order. Biba = LBAC with the dual order. Lipner = LBAC with the product `H × W^op`. Sandhu's contribution is showing all three are *the same theorem* about lattice monotonicity.

**Non-example.** Models that fundamentally need *temporal* / *history* / *transactional* state (Clark-Wilson, "well-formed transactions," BMA medical-records) cannot be cast as LBAC. They're not lattice problems.

**Citation.** [sandhu-1993-lbac] is the IEEE Computer 1993 paper, the canonical reference. [sandhu-1992-cwlattice] for the Chinese Wall recasting. [bishop-2018] §5 for textbook treatment.

**Consultant tags.** `(a)` primary — when the consultant has identified that a model is a lattice instance, citing Sandhu's LBAC framework gives the user the unified theory. `(c)` — when a model fails to fit, citing Sandhu lets the consultant say "Sandhu showed BLP, Biba, Lipner, and CW all fit LBAC; if your model doesn't, that's a strong signal it's not an information-flow problem."

> **When this comes up.** Anytime the user is comparing classification policies across domains (military / medical / commercial / coalition) and wondering whether they share structure. Cite [sandhu-1993-lbac]. The framework is the bridge between marque's CAPCO/ISM domain and any future domain (CUI, NATO, JOINT, FGI) — they all sit inside the same theory.

---

## 10. Chinese Wall / Brewer-Nash

**Definition.** Brewer & Nash [brewer-nash-1989] modelled commercial conflict-of-interest constraints (the canonical case being financial-sector consultants who cannot advise competing clients). The model partitions company datasets into *conflict-of-interest (COI) classes*; access to one company in a COI class precludes future access to other companies in the same COI class. The access function is *history-dependent* — what `s` may access at time `t` depends on what `s` has previously accessed.

Brewer & Nash claimed CW *cannot* be represented in BLP. Their argument: BLP is state-dependent in the lattice domination sense, but CW's restrictions are derived from the access history, which is a different form of state-dependence.

**The lattice question.** Is the conflict-of-interest structure a lattice?

- The COI classes themselves are *mutually disjoint*. That's an antichain — no COI class is "more conflict-of-interest-ish" than another. Ordered by inclusion of "datasets accessible," the structure is a *forest* of antichains: each COI class is an independent component, each company within a COI class is a sibling of others.
- Within a single COI class, the structure is "having accessed company A precludes accessing company B." That's not a lattice either — it's a bipartite "accessed A" vs "accessed B" exclusion that doesn't define meet/join cleanly.

Sandhu [sandhu-1992-cwlattice] showed the policy *can* be cast as a lattice — but at the cost of artificially expanding the label space to include "access history" and applying BLP's dominance rule on the expanded labels. Brewer & Nash's claim was true for the *natural* CW model; Sandhu's claim is true for an *engineered* CW model with the right label encoding.

**Laws — natural CW (Brewer-Nash).**
- Idempotence: yes (accessing A again doesn't change state).
- Commutativity: *no* — accessing A then B and accessing B then A produce different states (and, depending on COI classes, may differ in which is forbidden).
- Associativity: not directly applicable; access is sequential.
- Bounded: no obvious top or bottom.

**Laws — engineered CW (Sandhu).**
- Inherits BLP's lattice properties on the engineered label.

**Example — Brewer-Nash failure mode.** Consultant `s` accesses Bank A. Now `s` may *not* access Bank B (same COI class) but *may* access Manufacturer M (different COI class). Now imagine `s` had instead accessed Bank B first; same COI exclusion, but different state. The two states are not equal, and there's no obvious lattice meet/join between them. *History matters*, and a stateless access lattice cannot capture it.

**Example — Sandhu engineered.** Encode access state in the label: subject `s` carries label `(initial, history(s))` where `history(s) ⊆ Companies`. Object `o` (company `C`) is accessible iff `C` is in the same COI class as nothing in `history(s)`. Lattice-wise, `(initial, history)` is the product of the (trivial single-element) initial chain with the powerset `P(Companies)`. The access rule is now lattice-shaped, but the lattice has size `2^|Companies|`, which is impractical for nontrivial deployments.

**Non-example.** Treating COI classes as if they were security categories. Not the same — categories are *cumulative* (more compartments = stricter), COI classes are *exclusive* (one company per class).

**Citation.** [brewer-nash-1989] for the original. [sandhu-1992-cwlattice] for the lattice recasting. [foley-1992] for the noninterference reformulation. [meadows-1990] for a multilevel extension.

**Consultant tags.** `(c)` primary — when the user has dynamic / history-dependent / mutually-exclusive access rules, this is the entry that explains why naive CW *isn't* a lattice. `(b)` — if the user is willing to expand the label space to include history, Sandhu's recasting *is* a lattice but at exponential cost. Often Clark-Wilson-style transactions or stateful access matrices are the right answer.

> **When this comes up.** A user describes "access rule depends on what subject has previously accessed" or "two markings are mutually exclusive." Brewer-Nash is the cautionary tale: in its natural form it's not a lattice. You can force it into one (Sandhu) but at exponential cost. If neither option is operationally palatable, route to a non-lattice framework. This is the canonical "this isn't a lattice problem" entry.

---

## 11. RBAC role hierarchy as poset / lattice

**Definition.** Role-Based Access Control (RBAC), formalized by Ferraiolo & Kuhn 1992 and refined into the NIST/ANSI INCITS 359-2004 standard [nist-rbac-standard], assigns permissions to *roles* and assigns roles to *users*. The RBAC family is layered:
- **RBAC₀ (core / flat):** users, roles, permissions, sessions.
- **RBAC₁ (hierarchical):** adds a role hierarchy — a partial order `RH ⊆ Roles × Roles` defining seniority. A senior role inherits a junior role's permissions; junior roles inherit a senior role's user assignments. The hierarchy is *exactly a partial order* (reflexive, antisymmetric, transitive).
- **RBAC₂ (constrained):** adds separation-of-duty (SoD) and other constraints.
- **RBAC₃ (symmetric):** RBAC₁ ∪ RBAC₂.

The role hierarchy is, mathematically, a poset. The NIST standard distinguishes:
- **General Hierarchical RBAC**: arbitrary partial order (allowing multiple inheritance).
- **Limited Hierarchical RBAC**: tree or inverted-tree only (no multiple inheritance).

**Lattice connection.** General Hierarchical RBAC permits the role hierarchy to be a lattice — and many real-world hierarchies *are* lattices (e.g., Engineer1 and Engineer2 both inherit from Engineer; Director inherits from both ProjectLead1 and ProjectLead2 ⇒ each pair has a meet at Engineer or join at Director). But the standard does not require the role hierarchy to be a lattice; it requires only a partial order.

When the role hierarchy *is* a lattice, the senior-most-role join (least upper bound of two roles) gives the role with all the union of their permissions. In tree hierarchies (Limited RBAC), only one branch is shared above any pair; pairs in different branches have no upper bound except the root, so the structure may not be a lattice (a tree without cross-branch joins).

**Laws.** RBAC₁ requires only partial-order axioms. Lattice laws hold *if and only if* the deployment chose to make the hierarchy a lattice (most do, for engineering convenience).

**Example.** Roles `{Employee, Engineer, ProductionEngineer1, ProductionEngineer2, QualityEngineer1, QualityEngineer2, ProjectLead1, ProjectLead2, Director}` with the natural inheritance — `Director` is the join of any pair of `ProjectLead`s; `Engineer` is the join of any pair of specialised engineers; `Employee` is the bottom. This forms a lattice (with one inheritance branch). Compare to a flat tree where each project lead has independent reports and there's no shared role between them above `Director` — still a poset, may not be a lattice if any pair lacks a common ancestor below `Director`.

**Non-example.** A role graph with cycles (`A` inherits `B`, `B` inherits `A`) is not a partial order at all — antisymmetry fails. Any inheritance loop requires breaking before applying RBAC.

**Citation.** [sandhu-1996-rbac] for the framework paper. [nist-rbac-standard] for the proposed standard. [ferraiolo-kuhn-1992] for the original.

**Consultant tags.** `(a)` when the user has "roles and inheritance," recommend RBAC₁ / NIST standard if the partial order is genuine. `(b)` if the user wants role-pair joins/meets, recommend extending the hierarchy to a full lattice (often only requires one or two synthetic top/bottom roles). `(c)` if the user has cycles in role inheritance, redirect: that's not a poset, fix the cycles first.

> **When this comes up.** A user describes "managers inherit employee permissions," or "senior roles are 'higher' than junior roles." RBAC₁ is the framework name; cite NIST/ANSI INCITS 359-2004. If they want lattice operations on roles (find the join of two roles), make the hierarchy a lattice explicitly — most deployments do.

---

## 12. Decentralized Label Model (DLM)

**Definition.** Myers & Liskov [myers-liskov-2000] generalize Denning's lattice to support *mutually distrusting principals*. A label is a finite set of *policies* of the form `o → R` where `o` is the *owner* and `R ⊆ Principals` is the set of *readers* the owner permits. A label is "no more restrictive than" another, written `L₁ ⊑ L₂`, iff every policy in `L₂` has a counterpart in `L₁` whose owner acts-for the new policy's owner and whose reader set is a superset (i.e., `L₁`'s policy is at least as permissive). Formally:

`L₁ ⊑ L₂` iff for every `o₂ → R₂ ∈ L₂`, there exists `o₁ → R₁ ∈ L₁` with `o₂` acts-for `o₁` and `R₁ ⊆ R₂`.

[myers-liskov-2000] §3.2 / [zdancewic-2003-typesys-decl] §3.

**Lattice connection.** `⊑` is a *pre-order* (reflexive and transitive but *not* antisymmetric — different syntactic labels can be `⊑`-equivalent). The equivalence classes under `≡ := ⊑ ∩ ⊒` form a *distributive lattice*. The join `⊔` combines policies (intersection of reader sets per owner — that's correct: combining two labels = enforcing both restrictions = each reader must be permitted by both):
- `(o → R₁) ⊔ (o → R₂) = o → R₁ ∩ R₂`
- For different owners, just take the union of policies.

The meet `⊓` is dual: union of reader sets per owner; intersection of owners. Myers & Liskov note the meet is *not* operationally useful in their setting — relabeling-by-meet would relax restrictions, and they prefer to gate any relaxation behind explicit declassification by an authorized principal.

Integrity policies in DLM are dual to confidentiality policies (just like Biba is dual to BLP). Combined labels = confidentiality lattice × integrity lattice (just like Lipner). See `pure-lattice.md` entry 13 (dual lattice).

**Laws.** Distributive lattice on equivalence classes. Idempotent / commutative / associative joins and meets.

**Example.** Label `{Alice → {Bob, Carol}}` (Alice permits Bob and Carol). Combining with `{Alice → {Bob, Dave}}`: `{Alice → {Bob}}` — only Bob is permitted by both Alice-policies. Now combine with `{Bob → {Alice, Carol, Dave}}`: still need Bob in Alice-policy *plus* satisfy Bob-policy ⇒ `{Alice → {Bob}, Bob → {Alice, Carol, Dave}}`. Only Bob can read in the union (and only with Alice and Bob's joint blessing).

**Non-example.** Trying to interpret labels as pure sets of (owner, reader) pairs without per-owner policy structure — that loses the multi-principal essence. The "policy per owner" structure is what supports declassification by individual owners.

**Citation.** [myers-liskov-2000] for the original. [zdancewic-2003-typesys-decl] §3 for the lattice analysis. [jif-dlm-doc] for the working language. The DLM is the foundation of the Jif programming language [myers-1999-jflow].

**Consultant tags.** `(a)` primary — when the user has multiple "owners" with independent confidentiality policies on the same data, this is the construction. `(b)` — if the user has tried to flatten owner-based policies into a single lattice, recommend the per-owner-policy DLM structure. `(c)` — if the user's principals have *temporal* trust changes (i.e., who-acts-for-whom changes over time), DLM as stated assumes a fixed principal hierarchy; add a layer.

> **When this comes up.** A user describes a setting where multiple parties (e.g., two countries in a JOINT marking, two corporations in a coalition agreement) each have independent restrictions on the same data, and you need to combine those restrictions. DLM is the formal name. Cite [myers-liskov-2000]. Note that DLM's join is *intersection* of readers per owner — that matches the REL TO intersection rule (entry 7) but with per-owner accounting.

---

## 13. Information-flow type systems

**Definition.** A static type system that tags every variable, expression, and channel with a security type drawn from a lattice; well-typed programs satisfy a noninterference theorem (entry 14). The line of work:

- **Volpano-Smith-Irvine 1996** [volpano-smith-irvine-1996]: A sound type system for a simple imperative language with `letvar`. Types are pairs `(τ, ℓ)` where `τ` is the data type and `ℓ` is the security level (in a two-element lattice or general lattice). Soundness theorem: well-typed programs satisfy noninterference.
- **FlowCaml** [pottier-simonet-2003] [simonet-flowcaml-nutshell]: Extension of Objective Caml. Types annotated with security levels in a user-defined lattice. Constraint-based with full type inference. Implements many lattice operations as subtyping.
- **Jif** [myers-1999-jflow] [jif-dlm-doc]: Java Information Flow. Builds on Java with DLM labels. Static information-flow checking for mutable objects, subclassing, dynamic dispatch, and exceptions.

All three frame "information flow" as: the security type of an expression must be *no higher* (in the lattice) than the security type of any variable it can affect. The type system's well-formedness rule is `e.label ⊑ x.label` for assignment `x := e`.

**Lattice connection.** The security types form a lattice, and the type system uses it directly. Subtyping = lattice order. Type-checking is essentially constraint solving over the lattice. See [denning-1976] for the original lattice-of-classes idea Denning suggested for compile-time enforcement.

The deep theoretical content: well-typed-ness implies the noninterference theorem (entry 14), via either logical relations [volpano-smith-1998] or bisimulation. The proof depends on the lattice structure — specifically distributivity and the ⊔ / ⊓ rules.

**Laws.** Whatever the underlying lattice provides. The type system requires the lattice to be at minimum a *meet-semilattice* (for combining info from multiple sources) and at most a *bounded distributive lattice* (for reasoning about declassification).

**Example.** In FlowCaml, `let x : int@high = secret_value in let y : int@low = x` is a type error — `high ⊄ low`. The type system rejects the program at compile time.

In DLM-based Jif: `int{Alice:Bob}` is "an int owned by Alice and readable by Bob." `int{Alice:} = secret` is the most-restrictive label (Alice permits only herself).

**Non-example.** Type systems that operate on a flat type domain (no lattice) — e.g., "tainted" / "untainted" without intermediate states — are degenerate cases (two-element lattice). They work but offer only minimal expressiveness.

**Citation.** [volpano-smith-irvine-1996] for the soundness proof. [pottier-simonet-2003] for FlowCaml. [myers-1999-jflow] for Jif. [sabelfeld-myers-2003] for the comprehensive survey (147 references).

**Consultant tags.** `(a)` when the user wants compile-time enforcement of information-flow rules, this family is the answer. The type system encodes the lattice structure directly. `(b)` if the user has runtime label tracking, recommend a hybrid — static where types are known, runtime checks where labels depend on dynamic data (e.g., DLM's runtime label values).

> **When this comes up.** A user asks "can we statically check that no SECRET value reaches an UNCLASSIFIED variable?" That's exactly what these type systems do. Cite [denning-1976] for the foundational idea, [volpano-smith-irvine-1996] for the first soundness proof, and [sabelfeld-myers-2003] for the survey. These don't directly address marque's text-marking domain (marque operates on byte buffers, not programs) but if marque ever extends to *generating code that emits markings*, the type-system literature applies.

---

## 14. Non-interference (Goguen-Meseguer)

**Definition.** Goguen & Meseguer [goguen-meseguer-1982] gave the first formal definition of "no illegal information flow." The state-machine model: states `S`, users `U`, commands `SC`, transition function `do : S × U × SC → S`, output function `out : S × U → Out`, initial state `s₀`. For two user groups `G, G'`:

> `G` does not interfere with `G'` iff for any sequence of commands `w`,
>  `View_G'(w) = View_G'(P_G(w))`
> where `P_G(w)` is `w` with all commands by users in `G` purged.

Translation: `G'`'s observable behavior is unchanged whether `G` ran any commands or not. Equivalently: `G` cannot influence what `G'` sees.

**Lattice connection.** A *security policy* is a set of noninterference assertions of the form `G ↛ G'` ("does not interfere with"). When the user groups are linearly ordered by clearance ("HIGH" doesn't interfere with "LOW"), noninterference recovers the BLP information-flow rule. When users are ordered by a Denning-style lattice, noninterference is the lattice-relative *soundness* property: a system is noninterference-secure iff information only flows up the lattice. See [sabelfeld-myers-2003] for the connection.

Noninterference is what type systems (entry 13) prove their well-typed programs satisfy. It's also what Bell-LaPadula (entry 2) tries to enforce via state-transition rules, modulo covert channels.

**Laws.** Noninterference is a *safety property* in the Lamport sense — once violated, no subsequent action makes it un-violated. This makes it amenable to inductive proof via the unwinding theorem [goguen-meseguer-1984].

**Example.** A read of HIGH followed by a write to LOW (the simple-security-property violation in BLP) generally violates noninterference: the LOW user's view depends on the HIGH command being run. Hence the ⋆-property.

**Non-example.** "Possibilistic" noninterference for nondeterministic systems — a strict generalization that opens probabilistic and timing covert channels. There's a substantial literature on weakening noninterference to handle declassification, intransitive flows, robust declassification, etc. [sabelfeld-myers-2003] surveys.

**Citation.** [goguen-meseguer-1982] for the original. [goguen-meseguer-1984] for the unwinding theorem. [rushby-1992] for the canonical generalization.

**Consultant tags.** `(a)` primary — when the user wants a formal *definition* of "secure information flow," cite Goguen-Meseguer. `(c)` — when the user has a system that "feels" secure but they can't articulate the property, walk through the noninterference definition to surface the actual claim and ground its proof in a lattice.

> **When this comes up.** A user asks "how do we *prove* the marking system enforces information flow?" Goguen-Meseguer noninterference is the property to prove. Cite [goguen-meseguer-1982] and the survey [sabelfeld-myers-2003]. Note this is not directly applicable to marque (which is a marking-validity checker, not an information-flow type system) — but it's the right name when the user is reasoning about *why* marking validity matters.

---

## 15. Multi-level / multi-lateral / multi-policy lattices

**Definition.** Anderson [anderson-securityengineering] §9 distinguishes:
- **Multilevel security**: one lattice, multiple levels (BLP-style).
- **Multilateral security**: multiple independent constraints across compartments / categories / sectors. Need-to-know is a special case.
- **Multi-policy security**: composition of distinct policies (e.g., confidentiality + integrity, BLP + Clark-Wilson, BLP + Chinese Wall) where each policy may have its own lattice or non-lattice structure.

Anderson's framing: when "one lattice" is too narrow, you compose. The composition machinery is a *product* (entry 11 of `pure-lattice.md`) when the policies are independent, a *sum* / coproduct when they live on disjoint domains, or an ad-hoc rewrite when they interact.

**Lattice connection.** Multilevel = single Denning lattice. Multilateral = product of one-or-more category-axis lattices with the level lattice. Multi-policy = composition of multiple lattices (or non-lattice structures like Clark-Wilson transactional rules) under explicit composition operators.

**Laws / when composition is or isn't a lattice.**
- Product of lattices = lattice (always). `pure-lattice.md` entry 11.
- Disjoint sum / coproduct of lattices = lattice (with new top and bottom adjoined). `pure-lattice.md` entry 12.
- Lattice × non-lattice (e.g., BLP × Clark-Wilson) = generally *not* a lattice. The composition is a layered system where each component contributes its own constraints, but no single lattice captures both.

**Example.** A coalition deployment combines BLP confidentiality (lattice) + Lipner integrity (lattice) + Brewer-Nash COI (non-lattice) + RBAC role-hierarchy (poset/lattice). The composite is a *layered* system, not a single lattice. Each access decision goes through all layers.

**Non-example.** Treating "composition of constraints" as if it were "intersection of permitted operations" on a single lattice. That works *if* all the constraints are over the same lattice; usually they aren't.

**Citation.** [anderson-securityengineering] Ch. 9. [bishop-2018] §6 for textbook treatment. [denning-1976] for the original recognition that lattice composition is a foundational operation.

**Consultant tags.** `(a)` primary — when the user has multiple independent policies, name them as separate composable lattices/structures, not one mega-lattice. `(b)` — if the user has tried to merge into one lattice and is hitting friction, propose the composition framing. `(c)` — if some component is genuinely not a lattice (CW, transactional integrity), say so clearly: "this composes BLP with non-lattice CW, and the composite is not a single lattice."

> **When this comes up.** A user has more than one "axis" of policy — confidentiality, integrity, coalition release, role-based access, transaction state. Walk through Anderson's framework: which axes are lattices (compose as products), which are not (compose as layered checks). Don't force a single lattice to hold all of it; that's how you end up with the 192-label-but-only-9-used Lipner mistake at scale.

---

## 16. JOINT / coalition lattices

**Definition.** Coalition / multi-national markings (e.g., CAPCO JOINT, NATO JOINT, ACGU/FVEY/TEYE coalition releases) capture co-ownership: data is jointly owned by two or more nations or organizations. Per CAPCO-2016 and the DNI Authorized Classification and Control Markings Register [dni-capco-register], JOINT markings restrict release to the co-owners; further release requires consent from *all* co-owners.

The *page-level* joint marking is computed by intersecting the per-portion JOINT lists. JOINT data extracted into a US document inherits the JOINT structure as a derived REL TO list (or NOFORN if the originating country forbids).

**Lattice connection.** Two natural framings:

1. **Joint-as-product-lattice.** Treat each owner's release policy as an independent lattice (e.g., USA's REL TO list × FRA's REL TO list × GBR's REL TO list). The combined release rule is meet across owners: `JOINT(USA, FRA) = USA-policy ⊓ FRA-policy`. Releasability requires both. This is Denning lattice product (entry 1, entry 11 of `pure-lattice.md`). When all owners are in the JOINT list, the combined policy is the meet of all their individual REL TO lists.

2. **Joint-as-DLM-instance.** Each owner is a DLM principal with their own policy; the JOINT label is `{USA → R_USA, FRA → R_FRA}` per Myers-Liskov (entry 12). The DLM machinery already handles per-owner intersection-of-readers as the default join.

Both framings yield lattice structure. DLM is more flexible if owners' policies might evolve independently (a country adds/removes itself from the JOINT); the product-lattice framing is simpler when the owner set is fixed.

**Laws.** Distributive lattice (inherited from product or DLM). Idempotent, commutative, associative meets and joins.

**Tetragraph note.** Some tetragraphs (FVEY, ACGU, TEYE) signal pre-defined JOINT relationships. They're shorthand for the canonical JOINT product. Expansion is a lattice homomorphism.

**Example.** Portion `(S//JOINT TS USA, GBR)` and `(S//JOINT S USA, FRA)`. Page-level: `JOINT TS USA, GBR ⊓ JOINT S USA, FRA = JOINT S USA` (lower classification, intersection of country sets). All four constraints (TS dominates S in level; USA appears in both REL TOs) compose into the single JOINT S USA banner.

**Non-example.** Treating JOINT as if it were just a REL TO list — that loses the "co-ownership requires all-party consent" semantics. JOINT carries *additional* constraint beyond release (further release approval), which a flat REL TO doesn't.

**Citation.** [dni-capco-register] §G for FD&R / JOINT operational rules. [myers-liskov-2000] for DLM as the formal model. [bell-2005-looking-back] for CANUKUS retrospective discussion of multilateral sharing.

**Consultant tags.** `(a)` when the user is modelling multi-country co-ownership, recommend either product lattice or DLM. `(b)` — if the project has only chain × powerset structure and JOINT doesn't fit cleanly, propose extending to DLM (entry 12) for first-class per-owner policy. `(c)` — if the user is treating JOINT as a single-owner policy with a static country list, redirect: that loses the per-owner consent semantics.

> **When this comes up.** A user describes "two-or-more-country co-owned markings" and asks for the formal name. Two answers: product lattice if the owners' policies are independent and static; DLM if the owners are first-class principals with potentially-evolving policies. Most CAPCO JOINT use cases fit the product framing; CUI/NATO/partner-national futures may need the DLM framing.

---

## 17. Foreign-government-information (FGI) and tetragraph lattices

**Definition.** FGI markings indicate information originating from a foreign government, with country trigraphs (ISO 3166) identifying the source. CAPCO/ISM uses FGI markings (`FGI {trigraph(s)}`) and tetragraph codes for canonical country sets (NATO, FVEY, ACGU, TEYE). The page-level FGI marking is computed by union (in some contexts) or intersection (in others) of per-portion FGI sources, depending on the operational rule.

**Lattice connection.** The country-set domain is `(P(Countries), ⊆, ∩, ∪, ∅, Countries)` — a Boolean algebra. Same structure as the categories component of the Denning lattice (entry 5). The order is set inclusion. The lattice operations are intersection (meet) and union (join).

Tetragraphs are *named subsets* of `Countries`. FVEY = {USA, AUS, CAN, GBR, NZL}. NATO = {USA, GBR, FRA, ...} (29+ members). The tetragraph homomorphism `tetra → P(trigraph)` commutes with lattice operations: `expand(t₁) ∪ expand(t₂) = expand(t₁ ∪ t₂)` if the union has a tetragraph name; otherwise the expansion produces a trigraph-list directly.

**The mutual-exclusion wrinkle.** Per CAPCO-2016 §F, FGI in the trigraph position is mutually exclusive with naming the source. That is, you can mark `FGI {trigraph}` to attribute, OR `FGI` without trigraphs to anonymize, but not mix. This is a *constraint* on labels (in the marque-scheme sense), not a lattice operation. It's why entry 19 of "When a security policy is NOT a lattice" matters: mutually-exclusive markings are constraints, not lattice elements.

**Laws.** Pure Boolean algebra on the country-set side; constraints layered on top.

**Example.** Three portions: `(S//FGI GBR, FRA)`, `(S//FGI GBR)`, `(S//FGI FRA)`. Page-level (intersection / "common attribution"): `{GBR, FRA} ∩ {GBR} ∩ {FRA} = ∅`. With ∅, the FGI marking on banner becomes anonymized FGI (or NOFORN, depending on rule). Page-level (union / "all sources contributing"): `{GBR, FRA} ∪ {GBR} ∪ {FRA} = {GBR, FRA}`. Which to use depends on the operational interpretation.

**Non-example.** Treating "FGI" without trigraphs as the *meet* of all attributed FGI markings — that conflates "anonymized attribution" (a category-level marking) with "intersection of country sets" (a lattice operation). The former is a metalevel choice; the latter is a per-level computation.

**Citation.** [dni-capco-register] §C, §G. CAPCO-2016 §F (in the user's vendored copy). [bell-2005-looking-back] for Bell's discussion of multilateral / foreign-government policy.

**Consultant tags.** `(a)` for plain country-set operations, recommend Boolean algebra of `P(Countries)`. `(b)` for tetragraph use, propose a homomorphism from tetragraph names to country sets (the marque project already has this). `(c)` for the mutual-exclusion of FGI-with-trigraph vs FGI-anonymous, redirect: that's a constraint, not a lattice element.

> **When this comes up.** A user describes country-set unions/intersections in classification context. Three sub-questions: (a) the country-set Boolean algebra is straightforward; (b) tetragraphs are named subsets and operations on them are homomorphic to operations on country sets; (c) mutually-exclusive markings (anonymous-FGI vs attributed-FGI) live outside the lattice and need explicit constraint machinery (`Constraint` in marque-scheme).

---

## 18. SCI / compartmented information as a hierarchical lattice

**Definition.** Sensitive Compartmented Information (SCI) per DoD/IC convention is structured as a *hierarchy*: control system × compartments × sub-compartments. Per CAPCO-2016 §A.6, the grammar is `CONTROL-COMP (SPACE SUB-COMP)* (-COMP (SPACE SUB-COMP)*)*` — e.g. `SI-G ABCD DEFG-MMM AACD` is "SI control system, with G compartment (sub-compartments ABCD, DEFG), and MMM compartment (sub-compartment AACD)".

Each control system (SI, TK, HCS, etc.) is its own root. Compartments below a control system are children. Sub-compartments below a compartment are grandchildren. The structure is a *forest* (multiple roots) of *trees* (each control system is a tree of compartments).

**Lattice connection — the fundamental obstruction.** SCI compartments are *agency-extensible*: agencies can register new compartments and sub-compartments without coordination. There is no enumerable upper bound on the set of compartments. Consequently:

- The SCI structure has *no* top element. There's no "all SCI" marking that dominates every possible compartment, because new compartments can be added.
- The SCI structure is a *meet-semilattice* (you can compute "what compartments are in both portions" — intersection) **with no top**, hence not a bounded lattice. See `pure-lattice.md` entry 2 (meet-semilattice) and entry 4 (bounded lattice).

This is the structural reason `marque-capco`'s `SciSet` deliberately does not implement `BoundedLattice` (per the user's CLAUDE.md): "SCI control systems and SAR program identifiers are both agency-extensible open sets, so no lawful finite top exists."

**What is the right structure?** A *meet-semilattice* on `P(CompartmentTokens)` where the order is set inclusion and the meet is set intersection. There's a bottom (the empty compartment set, ⊥). There's no top.

**Banner roll-up.** Per CAPCO-2016 §H.4 and §A.6 p15-16, banner SCI is computed by *unioning* compartments and sub-compartments across all portions on the page, then sorting per the ordering rule (numeric first, alpha after). That's a *join* operation, not a meet — the join of `P(CompartmentTokens)` (the same Boolean algebra without top, but with all joins of finite sets defined). So banner roll-up is a *join-semilattice* operation.

The combined structure is therefore a *lattice without top* — meets and joins are both well-defined for any finite set of elements, but there's no upper bound in the entire space. This is fine for finite-portion documents; it just means the consultant should not invoke "the top of the lattice" because there isn't one.

**Laws.** Meet-semilattice and join-semilattice properties (associativity, commutativity, idempotence). No `BoundedLattice` because there's no top. The bottom is ∅.

**Example.** Portion 1: `SI-G ABCD`. Portion 2: `SI-G DEFG`. Banner roll-up via §H.4: union of compartments = `SI-G ABCD DEFG` (sorted). Meet across portions = `SI-G` (the common compartment, no sub-compartments shared).

**Non-example.** Asserting an "all SCI" top element — that's defining a label ("all compartments") that has no operational counterpart and breaks at every agency-registration event. Don't.

**Citation.** CAPCO-2016 §A.6 (the SCI grammar) and §H.4 (banner roll-up); the user has the vendored doc. [dni-capco-register] §B for the registered control systems. [wikipedia-sci] for the public summary.

**Consultant tags.** `(a)` primary — when the user has SCI / compartmented data, the formal name is "meet/join-semilattice on `P(Compartments)` with no top." `(b)` — if the user wants `BoundedLattice` for engineering convenience, recommend Smyth-style adjoining of an artificial top, with the caveat that the top has no operational meaning. `(c)` — if the user insists on "all SCI" as a real top, redirect to the agency-extensibility argument: no finite top is consistent with agency-driven extension.

> **When this comes up.** A user asks "is SCI a lattice?" or "why doesn't SciSet have a top?" The answer is the agency-extensibility argument: no upper bound on the compartment set ⇒ no top ⇒ meet-semilattice (or lattice-without-top), not BoundedLattice. This is one of the most important entries for marque specifically; cite the CAPCO sections directly when explaining to the user.

---

## 19. SAR / Special Access Required as a forest of programs

**Definition.** Special Access Required (SAR) markings identify Special Access Programs (SAPs) — DoD-specific compartments that protect particularly sensitive technologies, methods, or capabilities. Per the DoD SAP Security Manual (DoDM 5205.07), SAR program identifiers are codewords/nicknames assigned by SAP authorities. Each program may have *compartments* (denoted by hyphen) and *sub-compartments* (separated by spaces), in alphanumeric order.

Like SCI, SAR is *agency-extensible*: programs are created and retired without central registration in a public-facing CVE. The DoD Authorized Classification and Control Markings Register intentionally leaves the SAR enumeration empty in public form.

**Lattice connection.** Same structural argument as entry 18:
- SAR program × compartment × sub-compartment forms a forest (multiple roots, each a tree).
- Programs are mutually exclusive in some operational senses but mutually independent in the structural sense (different programs can co-exist on the same data).
- Agency extensibility ⇒ no top.
- Therefore: meet-semilattice on `P(SAR-tokens)` plus the forest structure on individual tokens.

The marque project's `marque_capco::lattice::SarSet` reflects this: `SarSet` is a lattice element under set operations on tokens, with a bottom (∅) but no top (no `BoundedLattice` impl).

**Laws.** Meet-semilattice + join-semilattice properties; no top. Forest-of-trees substructure on individual tokens (a token is a path from program-root through compartments to sub-compartments).

**Example.** Three portions: `(S//SAR-NICKNAME ALPHA)`, `(S//SAR-NICKNAME BRAVO)`, `(S//SAR-OTHER)`. Banner roll-up: union of all SAR tokens → `SAR-NICKNAME ALPHA BRAVO/SAR-OTHER`. Meet across portions: empty (no token shared).

**Non-example.** Treating SAR programs as if they were SCI control systems. The grammars are similar (program-compartment-subcompartment) but the operational policies differ: SAR is administered by DoD; SCI by IC; the registries are independently managed.

**Citation.** [dod-sap-manual] DoDM 5205.07 Vol. 2 / Vol. 4. [cdse-sap-markings] for working-level marking guidance. CAPCO-2016 §H.5 (in the user's vendored copy) for the SAR portion grammar.

**Consultant tags.** `(a)` for SAR token-set operations: meet-semilattice on `P(SAR-tokens)`. `(b)` if the user is modeling SCI and SAR with the same lattice-shaped trait, point out they share structure (both meet-semilattices with no top) but have different vocabularies. `(c)` if the user expects a `BoundedLattice` for SAR, redirect to the agency-extensibility argument (same as SCI).

> **When this comes up.** Same shape-question as SCI but for the DoD SAP world: "is SAR a lattice?" Same answer: meet/join-semilattice on `P(tokens)`, no top, agency-extensible. The two domains share the structural argument; if marque needs to handle CUI / NATO / partner-national markings in the future, the same argument generally applies (any agency-extensible registry produces a no-top semilattice).

---

## 20. Audit / log lattices and provenance

**Definition.** Information-flow audit treats *provenance* — where data came from, what processes touched it, what other data influenced it — as first-class structure. The standard model:
- Data items carry *labels* drawn from a security lattice.
- Provenance records form a *directed acyclic graph (DAG)*: nodes are data/process events, edges are flow relationships.
- Audit-record labels are computed along provenance paths via lattice operations: an output's label is the join of all inputs' labels (the join is "any of these contributed information").

The lattice machinery validates that observed flows respect declared policy. Non-compliance = a flow whose output label exceeds the policy-allowed lattice level.

**Lattice connection.** The provenance graph is *not* itself a lattice — it's a DAG, generally with neither meet nor join structure on graph nodes. But the *labels along the provenance graph* are lattice-valued, and the audit machinery operates on labels:
- Per node: label = `⊔` (join) of all incoming-edge labels.
- Per audit query: "did label `X` flow to a node with policy class `Y`?" iff `X ⊑ Y` in the lattice.

The composition is **DAG (graph) + lattice-valued labelling**. The DAG carries the structural relationships; the lattice carries the order-theoretic semantics. See [pasquier-2016] for the formal framework.

**Marque-specific note.** The `marque-engine` audit-record framework is documented in the project's CLAUDE.md and constitution: every applied fix produces an `AppliedFix` with rule-id, original/replacement text, confidence, timestamp, classifier-id. This is provenance in the audit sense — but it's *event-driven*, not flow-driven. The marque audit records do not currently apply lattice operations to track flow across multiple fixes; they record discrete events. If marque ever needs to compute "did SECRET text flow through the engine into UNCLASSIFIED output?", the audit-as-lattice framework would apply.

**Laws.** Lattice on the labels (whatever the underlying lattice provides). DAG structure on the provenance graph (acyclic, transitive-closure-defined reachability).

**Example.** Provenance graph: `Sensor1 → Aggregator → Report`. Labels: `Sensor1 → SECRET`, `Aggregator` inherits `SECRET ⊔ Other_inputs`, `Report` inherits `SECRET ⊔ ... = SECRET (or higher)`. Audit query: "is the Report classified ≥ SECRET?" yes by the join.

**Non-example.** Treating the provenance graph itself as a lattice. The graph is generally not a lattice — two events may both depend on a common ancestor (a meet) and both feed into a common descendant (a join), but those graph-theoretic meets/joins are usually the wrong operations to use; what you care about are *label-set* meets/joins, not graph-node meets/joins.

**Citation.** [pasquier-2016] for information-flow audit. [denning-1976] for the foundational lattice. [marque-constitution] (the user's own project) §V (Audit-First Compliance).

**Consultant tags.** `(a)` when the user wants flow-aware audit (rare in marque today, possible in future). `(c)` when the user has a provenance graph and confuses graph-meet with label-meet — redirect.

> **When this comes up.** A user asks how audit records compose under information flow. Answer: the labels form a lattice; the provenance graph is a DAG; you compute output labels as the join of input labels along DAG edges; non-compliance is `output_label ⊑ policy_class` failure. Cite [pasquier-2016] and [denning-1976]. This entry is mostly here for completeness; marque's current audit machinery is event-driven, not flow-driven, but the framework is the right name if/when marque extends.

---

## When a security policy is NOT a lattice

A high-value diagnostic section: when the user's "policy" doesn't fit any lattice in this catalog, the consultant should be able to explain *why* and recommend alternative formal frameworks. Patterns:

### 1. Forest, not lattice (Brewer-Nash, naive CW)

When the policy partitions data into *mutually exclusive* classes (conflict-of-interest classes; mutually-exclusive markings) and access is dependent on which class is chosen, the structure is typically a *forest of antichains* — multiple disjoint components, each an antichain. Forests are partial orders; they're not lattices (pairs from different components have no upper bound short of an artificial root).

**Recommendation.** Stay in the poset/forest framework, or pay the exponential cost of Sandhu's history-encoding to force-fit a lattice. See entry 10.

### 2. Open-ended / agency-extensible compartments (SCI, SAR)

When the set of compartments is open and grows without coordination (agency-extensible), there is no enumerable top. The structure is a meet-semilattice (or lattice-without-top), *not* a bounded lattice.

**Recommendation.** Use `Lattice` trait but *not* `BoundedLattice`; the bottom is `∅`, the top doesn't exist. Use `default()` for the bottom; never call `top()`. This is exactly the discipline marque's `SciSet` / `SarSet` enforce. See entries 18, 19.

### 3. Override / supersession that breaks idempotence-or-absorption

When one marking *overrides* another (NOFORN supersedes REL TO; NODIS supersedes EXDIS), and the override doesn't satisfy the absorption laws, it's not a lattice meet. It might be:
- An *absorbing element* (entry 6 Framing 1): lattice-shaped if you adjoin a special bottom whose meet with anything is the bottom.
- A *quotient* (entry 6 Framing 2): lattice-shaped if you accept equivalence-class collapse.
- A *non-lattice rewrite* (entry 6 Framing 3): not a lattice; needs deterministic ordering.

**Recommendation.** Pick a framing explicitly; don't pretend the operation is "just like a meet." See entry 6.

### 4. Mutually-exclusive markings (FGI-attributed vs FGI-anonymous; some JOINT cases)

When two markings cannot co-exist on the same data, the constraint is *external* to the lattice — it's a *Constraint* in the marque-scheme sense. The lattice elements are individual markings; the constraint says certain elements cannot be combined. Constraints are *guards*, not lattice operations.

**Recommendation.** Keep the lattice for the elements that *can* be combined; layer constraints on top to forbid invalid combinations. Don't try to encode mutual exclusion in the lattice order.

### 5. Order-dependent rewrites (page-level rewrite operations)

If `apply(a, apply(b, x)) ≠ apply(b, apply(a, x))`, the operation is *non-commutative*. Lattice meets/joins are commutative by construction. Order-dependence means you have a non-commutative monoid action, not a lattice operation.

**Recommendation.** If the rewrites are intended to be applied in a deterministic order (as in marque's `PageRewrite` topological scheduler), name the structure as "deterministically-ordered rewrite over a poset," not "lattice operation." Use the topological order to *define* the canonical evaluation; don't pretend commutativity holds.

### 6. State-dependent / history-dependent access (dynamic CW; transactional Clark-Wilson)

When access depends on what's *previously* happened, the access-control function is `S × H → permitted`, where `H` is a history. Lattices in `S` alone don't capture this. Either:
- Encode history into the label space (Sandhu's CW recasting; explodes label space).
- Use a state-machine model (BLP-style Basic Security Theorem).
- Use a transactional model (Clark-Wilson) with well-formed transactions.

**Recommendation.** Don't try to lattice-ify history. Acknowledge the state machine and operate at the BLP layer or move to Clark-Wilson.

### 7. Mixed lattice + non-lattice compositions

When part of the policy is a lattice (BLP confidentiality) and part isn't (Chinese Wall, Clark-Wilson), the composite *is not* a lattice. The right framing is *layered enforcement* — each policy layer applies its own check; an access is permitted iff all layers approve.

**Recommendation.** Don't try to merge into one lattice. Anderson [anderson-securityengineering] §9 and Bishop [bishop-2018] §5 both walk through this clearly.

---

## How to read the consultant tags

- `(a)` = "*this entry is the answer*." Use when the user describes a problem whose shape closely matches this entry's definition. The consultant proposes the construction.
- `(b)` = "*pivot toward this entry*." Use when the user's problem is *close* to this entry's definition but doesn't fit cleanly. The consultant proposes a redesign — possibly minor (use a sublattice, adjoin a top) or major (move to a different formal framework).
- `(c)` = "*this entry explains the refusal*." Use when the user is forcing a fit that doesn't exist. The consultant points at this entry to explain why the problem isn't a lattice problem (or isn't *this* lattice problem) and redirects to a different framework or expert.

---

## Cross-references

- Entry 1 (Denning lattice) is the foundation. Entries 2 (BLP), 3 (Biba), 4 (Lipner), 5 (need-to-know), 7 (REL TO), 8 (declassification dates), 16 (JOINT), 17 (FGI) are all instances or compositions of entry 1.
- Entry 9 (Sandhu LBAC) is the meta-construction containing entries 1–8.
- Entry 10 (Chinese Wall) is the cautionary entry — explicitly *not* fitting cleanly into the LBAC framework. Sandhu's recasting forces a fit at exponential cost.
- Entries 18 (SCI) and 19 (SAR) share the "agency-extensible no-top" structural argument; future CUI / NATO / FGI marking domains are very likely to share it too.
- Entries 13 (type systems) and 14 (noninterference) connect the *static enforcement* (compile-time checking) story; they're not directly used by marque (which is a runtime marking-validity checker, not a type system) but explain *why* lattice structure matters for soundness.
- Entry 20 (audit) is the natural extension of marque's current audit-record framework if it ever needs flow-aware composition.

The "When a security policy is NOT a lattice" diagnostic section is high-traffic for consultant mode (c) — it's the section to point at when the user is forcing a fit that doesn't exist.
