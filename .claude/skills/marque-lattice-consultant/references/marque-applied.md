# Marque Consolidation Bridge — Applied Diagnoses

**Audience.** Claude, in lattice-consultant mode, scanning for the answer to a question that names a marque construct (a Rust trait, a CAPCO rule ID, a phrase from the recursive-lattice plan) and wants to know whether the literature already names that construct, whether the marque implementation is sound, and what to recommend.

**Scope.** This file is the bridge between the five catalog files (`pure-lattice.md`, `security-lattice.md`, `abstract-interp.md`, `frames-locales.md`, `universal-algebra.md`) and the marque codebase. Where a catalog entry covers a marque construct cleanly, point at it and stop. Where the construct doesn't fit any catalog entry, surface the gap as an open question for the user — do **not** invent.

**Reading order.**
- §1 — inventory of marque's lattice-shaped surface, codebase paths, no analysis.
- §2 — bridge: each construct, its closest catalog entry, the (a)/(b)/(c) verdict.
- §3 — the PR 3b stall walked through as a lattice-algebra exercise. §3.0 holds two governance principles ("form is not shape" + "structure rules vs other-purpose rules"). §3.4.1–§3.4.6 carry the transmutation roster, family-predicate RELIDO incompatibility, cross-axis FGI rollup, indestructibility framing, RELOPT round-trip + auto-collapse fixpoint, and per-token classification floors. §3.7 covers passthrough policy + NNPI bounded confidence. §3.10 overlays the user's Phase A/B/C structural framing on top of the bucket view (§3.1–§3.8) and gives the rule-count moves to land in the 8–18 band.
- §4 — Phase-A structural-resolution primitives: the §3.3a equal-depth meet (§4.1–4.6), the closure operator for implied-fact propagation (§4.7), and the FGI/JOINT-attribution lattice / FlatSet-with-disagreement (§4.8).
- §5 — open vocabulary and the no-top decision.
- §6 — the topological page-rewrite scheduler.
- §7 — NOFORN-clears-REL TO as a `PageRewrite`.
- §8 — decoder confidence propagation.
- §9 — aggregated open questions for the user.
- §10 — recommended next moves to unblock PR 3b.

**Citation discipline.** Every claim about marque code is cited by absolute file path. Every claim about lattice theory is cited by `[bibkey]` (resolves in `bibliography.md`) plus catalog file + entry name (e.g., `security-lattice.md` §6 "Supersession algebra"). Every claim about CAPCO-2016 marking semantics — banner roll-up, FD&R precedence, per-marking commingling — should be cross-checked against `capco-context.md` (the vendored CAPCO-2016 snapshot) before propagating. Per the brief, this file does not coin new constructions; where the literature is silent or contradictory, the question is surfaced for the user.

---

## §1. Marque construct inventory

A flat enumeration of marque's lattice-shaped surface. No analysis here — that's §2. File paths are absolute.

### 1.1 The traits in `marque-scheme`

| Construct | File | Brief description |
|---|---|---|
| `Lattice` (trait) | `/home/knitli/marque/.worktrees/fix-gemini/crates/scheme/src/lattice.rs:37` | Two methods (`join`, `meet`); doc comment says implementors "are expected to satisfy the standard lattice laws" but the trait does not enforce them. |
| `BoundedLattice` (trait) | `/home/knitli/marque/.worktrees/fix-gemini/crates/scheme/src/lattice.rs:46` | Adds `top()` and `bottom()` constants; sub-trait of `Lattice`. |
| `MarkingScheme` (trait) | `/home/knitli/marque/.worktrees/fix-gemini/crates/scheme/src/scheme.rs:29` | The marking-scheme trait. Holds `categories()`, `constraints()`, `templates()`, `parse`, `validate`, `project(scope, &[Marking])`, `page_rewrites()`, `render_*`. Associated type `Marking: Lattice`. |
| `Scope` (enum) | `/home/knitli/marque/.worktrees/fix-gemini/crates/scheme/src/scope.rs:34` | Four variants: `Portion`, `Page`, `Document`, `Diff`. |
| `DiffInput<M>` | `/home/knitli/marque/.worktrees/fix-gemini/crates/scheme/src/scope.rs:56` | Two markings + `DiffRelation` (e.g., `BannerOverPortions`, `ReplyOverParent`). |
| `Constraint` (enum) | `/home/knitli/marque/.worktrees/fix-gemini/crates/scheme/src/constraint.rs:67` | Five variants: `Conflicts`, `Requires`, `Implies`, `Supersedes`, `Custom`. Each carries a stable `name`, a citation `label`, and (for the four dyadic variants) two `TokenRef`s. |
| `evaluate` (free function) | `/home/knitli/marque/.worktrees/fix-gemini/crates/scheme/src/constraint.rs:180` | Walks a scheme's declarative constraints, fires diagnostics for `Conflicts`/`Requires`, no-ops on `Implies`/`Supersedes`, dispatches `Custom` to scheme-specific predicates. |
| `Category` (struct) | `/home/knitli/marque/.worktrees/fix-gemini/crates/scheme/src/category.rs:136` | Per-category descriptor with `aggregation`, `cardinality`, `intra_ordering`, `expansion`. |
| `AggregationOp` (enum) | `/home/knitli/marque/.worktrees/fix-gemini/crates/scheme/src/category.rs:90` | Seven variants: `Max`, `Union`, `Intersect`, `UnionWithSupersession`, `MaxDate`, `Mode`, `Custom`. **Phase B retired this from runtime dispatch**; it survives as build-time shorthand and inspection metadata. |
| `CategoryShape` (enum) | `/home/knitli/marque/.worktrees/fix-gemini/crates/scheme/src/category.rs:164` | Inspection-only descriptor: `Ordinal`, `FlatSet`, `IntersectSet`, `Supersession`, `Date`, `Mode`, `Optional(Box<...>)`, `Product(Vec<...>)`, `Custom`. |
| `PageRewrite<S>` | `/home/knitli/marque/.worktrees/fix-gemini/crates/scheme/src/page_rewrite.rs:65` | Trigger + action + `reads`/`writes` axes, run after per-category projection. |
| `CategoryPredicate<S>` | `/home/knitli/marque/.worktrees/fix-gemini/crates/scheme/src/page_rewrite.rs:269` | `Contains` / `Empty` / `Custom(fn)`. |
| `CategoryAction<S>` | `/home/knitli/marque/.worktrees/fix-gemini/crates/scheme/src/page_rewrite.rs:282` | `Clear` / `Replace` / `Promote { from, to, transform }` / `Custom(fn)`. |
| `Projection<M>` (trait) | `/home/knitli/marque/.worktrees/fix-gemini/crates/scheme/src/projection.rs:28` | Documentation-only single-method trait; not consumed in hot path. |

### 1.2 The built-in lattice constructors

In `/home/knitli/marque/.worktrees/fix-gemini/crates/scheme/src/builtins.rs`:

| Constructor | Signature | `BoundedLattice`? | Lines |
|---|---|---|---|
| `OrdMax<T: Ord + Clone>` | newtype over `T`, `join = max`, `meet = min` | No — generic `T` has no canonical top/bottom | 55 |
| `OrdMin<T: Ord + Clone>` | dual of `OrdMax`; `join = min`, `meet = max` | No | 83 |
| `FlatSet<T: Ord + Clone>` | sorted `Vec<T>`, `join = merge-union`, `meet = intersection` | No — open universe | 120 |
| `IntersectSet<T: Ord + Clone>` | sorted `Vec<T>` with **flipped operators**: `join = intersection`, `meet = union` | No | 227 |
| `SupersessionSet<T>` | union with post-filter by `&'static [(T, T)]` table | No | 328 |
| `ModeSet<T>` | `BTreeMap<T, u32>` count multiset; `join = per-key max` (idempotent) | No (top would need infinite-count sentinel) | 451 |
| `MaxDate` | wraps `Option<NaiveDate>`; `join = later` | **Yes** — `top = NaiveDate::MAX`, `bottom = None` | 557 |
| `OptionalSingleton<L: Lattice>` | wraps `Option<L>`; `None` is bottom; `Some(a) ⊔ Some(b) = Some(a.join(b))` | Yes when `L: BoundedLattice` | 661 |
| `Product<A: Lattice, B: Lattice>` | tuple of two lattices; coordinatewise join/meet | Yes when both factors are bounded | 721 |

### 1.3 The CAPCO structural lattices

In `/home/knitli/marque/.worktrees/fix-gemini/crates/capco/src/lattice.rs`:

| Construct | Lines | Description |
|---|---|---|
| `SciSet` | 64 | `BTreeMap<SystemKey, BTreeMap<String, BTreeSet<String>>>` — system → compartment → sub-compartments. `join = component-wise union`. `meet = §3.3a equal-depth intersection` (policy (b)). **Does not implement `BoundedLattice`** by deliberate decision (lines 264–272). |
| `SystemKey` (private enum) | 75 | Stable ordering key: `Published(SciControlBare) | Custom(String)`. |
| `SarSet` | 288 | `BTreeMap<String, BTreeMap<String, BTreeSet<String>>>` — program → compartment → sub-compartment. `join = union`, `meet = equal-depth intersection`. **Does not implement `BoundedLattice`** (lines 401–406). |
| `FgiSet` | 423 | Two-variant enum: `None` and `Present { concealed: bool, countries: BTreeSet<CountryCode> }`. Concealed supersedes acknowledged on `join`. **Does** implement `BoundedLattice` (lines 556–568) with `top = Present { concealed: true, countries: ∅ }`. |
| `SciSet::overlaps` | 176 | Helper for "do these share at least one control system?" |
| `SciSet::common_compartments` | 182 | Helper for "(system, compartment) pairs in both." |

### 1.4 The CAPCO scheme adapter

In `/home/knitli/marque/.worktrees/fix-gemini/crates/capco/src/scheme.rs`:

| Construct | Approximate line | Description |
|---|---|---|
| `CapcoScheme` | 392 | Holds a `Vec<Constraint>`, a `Vec<PageRewrite<CapcoScheme>>`, and the per-category metadata. |
| `build_constraints()` | 736 | Returns the declarative constraint catalog. ~30 entries; many are `Constraint::Custom` because the predicate is n-ary. |
| `build_page_rewrites()` | 453 | Returns three `PageRewrite` entries: `capco/noforn-clears-rel-to`, `capco/joint-promotion`, `capco/fgi-absorption`. The latter two are `Custom` triggers stubbed `never_fires` pending Phase D/E. |
| `CapcoScheme::project` | ~1380 | Implements `MarkingScheme::project(Scope, &[Marking])`. Currently delegates to `PageContext` for back-compat (per `2026-05-02` plan §1.2 this *violates* lattice laws and is the focus of PR 4 cleanup). |

### 1.5 The 56 CAPCO rules

Counted across three files:

- `/home/knitli/marque/.worktrees/fix-gemini/crates/capco/src/rules.rs` — ~35 rules (E001–E041, S001–S006, W003, C001 — exact list at lines 261–6109).
- `/home/knitli/marque/.worktrees/fix-gemini/crates/capco/src/rules_declarative.rs` — 14 rules (E010, E012, E014, E015, E016, E021, E022, E024, E025, E036, E037, E038, E053, W002).
- `/home/knitli/marque/.worktrees/fix-gemini/crates/capco/src/rules_sci_per_system.rs` — 10 rules (E042–E051).

**Total**: 49 rules per the consolidated plan's "PR 3b acceptance" (slightly less than the 56 figure used colloquially; the difference is bookkeeping — retired/folded vs. counted-as-collapsed). PR 3b targets the **8–18 band** per `/home/knitli/marque/.worktrees/fix-gemini/specs/006-engine-rule-refactor/plan.md:230` (D13).

### 1.6 The §3.3a equal-depth meet policy

Defined in module docstring at `/home/knitli/marque/.worktrees/fix-gemini/crates/capco/src/lattice.rs:14–39` ("# Policy (§3.3a of the Phase B design doc)") and implemented in `SciSet::meet` (`lattice.rs:234–261`). Specified in `/home/knitli/marque/.worktrees/fix-gemini/docs/plans/2026-04-19-recursive-lattice-and-decoder.md:313–339`.

### 1.7 Open-vocab compartments

CAPCO/ISM enumerated tokens (`SciControlBare`, `DissemControl`, etc.) are generated from the ODNI XML at build time. Agency-extensible compartment names (e.g., custom SCI control systems matching `[A-Z0-9]{2,5}`, SAR program codewords) are NOT in the CVE; they are admitted at runtime. The `SystemKey::Custom` variant (`crates/capco/src/lattice.rs:78`) and the `String`-keyed compartment map encode this. The agency-extensibility argument is the documented justification for omitting `BoundedLattice` from `SciSet` and `SarSet`.

### 1.8 NOFORN-clears-REL TO

Declared as a `PageRewrite::declarative` at `/home/knitli/marque/.worktrees/fix-gemini/crates/capco/src/scheme.rs:486–498`:

```text
trigger:  CategoryPredicate::Contains { category: CAT_DISSEM, token: TOK_NOFORN }
action:   CategoryAction::Clear { category: CAT_REL_TO }
reads:    [CAT_DISSEM, CAT_REL_TO]
writes:   [CAT_REL_TO]
citation: "CAPCO-2016 §D.2 Table 3 + §H.8 p145"
```

The `reads` includes both `CAT_DISSEM` (where the trigger looks) and `CAT_REL_TO` (so the scheduler orders this rewrite *after* any rewrite that *writes* REL TO — i.e., after `joint-promotion`, see scheme.rs:454–469).

### 1.9 The REL TO intersection-with-blackball pattern

Two pieces compose:

1. The `CAT_REL_TO` category uses `AggregationOp::Intersect` (per `crates/capco/src/scheme.rs:715`) — meaning the per-portion REL TO sets are intersected during page projection.
2. The `capco/noforn-clears-rel-to` `PageRewrite` clears the result to ∅ if any portion contributed NOFORN.

The combination is the "intersection-with-blackball" idiom: REL TO is the *meet* (intersection) of country sets, with NOFORN acting as an absorbing override applied **after** the intersection.

### 1.10 The topological page-rewrite scheduler

Implemented in `marque-engine` (the consolidated plan and CLAUDE.md both reference `marque-engine::scheduler` and the `Engine::new`-time topological sort). Cycle detection raises `EngineConstructionError::RewriteCycle`; an unannotated `Custom` rewrite raises `EngineConstructionError::UnannotatedCustomAxes` (`/home/knitli/marque/.worktrees/fix-gemini/crates/scheme/src/page_rewrite.rs:144–168` defines the `try_custom` validation path; the engine-side scheduler enforces the same invariant at `Engine::new`).

### 1.11 The decoder

`StrictRecognizer` and `DecoderRecognizer` ship in `marque-engine`, dispatched via `StrictOrDecoderRecognizer` (per CLAUDE.md "Recent Changes"). `Confidence` (in `marque-rules`) carries `recognition: f32`, `rule: f32`, and an enumerated `FeatureId` list. Engine combines via the product `combined() = recognition * rule`. A `FixProposal` carries `Confidence`; the engine threshold-filters fixes by `combined()`.

### 1.12 The pivot type

`IsmAttributes` (`crates/ism/src/attrs.rs`, ~365). The 2026-05-02 plan §1.1 names this as the central problem: it does parser output, post-canonical form, and page roll-up output simultaneously. PR 3a splits it into `ParsedAttrs<'src>` / `CanonicalAttrs` / `ProjectedMarking`. This is structurally upstream of the lattice question but worth surfacing because **it is what the page roll-up produces** — the type that `project(Scope::Page, ...)` should return.

---

## §2. Bridge: marque construct ↔ catalog entry

For each construct from §1, this section records the closest literature match, the (a)/(b)/(c) verdict, and any open question for the user.

### 2.1 `Lattice` trait

**What it does.** Two methods (`join`, `meet`) with documented expectation that implementors satisfy the four standard lattice laws. The trait is the engine's interface to per-category reduction.

**Closest catalog entry.** `pure-lattice.md` §3 "Lattice" — algebraic definition (idempotence, commutativity, associativity, absorption).

**Match quality: (a) exact match.** The trait surface is the algebraic definition stripped to the operations. The doc comment correctly names the four laws.

**Caveat the consultant should surface.** The trait does not enforce the laws; per `[davey-priestley-2002]` Definition 2.4 / Theorem 2.9 (also `pure-lattice.md` §3 "Notable consequence"), idempotence is not a free consequence of the others, and absorption is the easy-to-miss one. The marque test fixture at `crates/scheme/src/lattice.rs:60–166` exercises the four laws on a tiny chain (`Level::U/C/S/TS`); per-implementation property tests (`crates/capco/tests/lattice_laws.rs`, `crates/capco/tests/proptest_lattice.rs`) cover the structural lattices. **Verdict: this trait *is* the algebraic lattice signature; rely on it directly.**

### 2.2 `BoundedLattice` trait

**What it does.** Adds `top()` and `bottom()` constants.

**Closest catalog entry.** `pure-lattice.md` §4 "Bounded lattice" — `⊥ ≤ x ≤ ⊤`, `x ∧ ⊤ = x`, `x ∨ ⊥ = x`. Source: `[davey-priestley-2002]` Definition 2.13.

**Match quality: (a) exact match** for the algebraic surface. The "open-set / agency-extensible warning" in `pure-lattice.md` §4 is exactly the marque rule that `SciSet` and `SarSet` must NOT implement this trait. Marque's discipline (lines 264–272 of `crates/capco/src/lattice.rs`, lines 401–406) honors the warning.

**Verdict.** The trait is correctly designed and correctly excluded from open-vocabulary types. No redesign needed. See §5 for the related `top()`-correctness audit.

### 2.3 `OrdMax` / `OrdMin`

**What they do.** `OrdMax<T>` is a total-order lattice with `join = max`, `meet = min`. `OrdMin` is the dual.

**Closest catalog entry.** `pure-lattice.md` §11 "Product lattice" example (the level-chain `U < C < S < TS`); `security-lattice.md` §1 "Denning's classification-with-categories lattice" for the canonical use (the chain factor of the Denning product). Sources: `[davey-priestley-2002]` §1.20, `[denning-1976]` §2.

**Match quality: (a) exact match.** A total order is automatically a distributive lattice (`pure-lattice.md` §6 "Example" — chains are trivially distributive). Both impls are sound: `max` and `min` over `Ord` satisfy idempotence/commutativity/associativity/absorption mechanically.

**Verdict.** Use directly for bounded-finite chains (classification level, integrity tier). For NATO-vs-US classifications that are *not* a single chain, see §3 of the May 1 lattice plan and the open question in §9 below — `OrdMax<MarkingClassification>` requires a total order on `MarkingClassification`, which the May 1 plan §2 Open Question 1 explicitly flags as unresolved.

### 2.4 `FlatSet<T>`

**What it does.** Powerset lattice with `join = union`, `meet = intersection`. Sorted-`Vec` storage; deduplicated.

**Closest catalog entry.** `pure-lattice.md` §3 "Lattice" example (`(P(X), ⊆)`); `security-lattice.md` §5 "Need-to-know / compartmented mode" — categories form `(P(C), ⊆, ∩, ∪)`, a Boolean algebra. Sources: `[davey-priestley-2002]` Definition 2.4, `[denning-1976]` §3, `[sandhu-1993-lbac]` "Lattices with Categories."

**Match quality: (a) exact match.** This is the canonical Boolean algebra of subsets. Distributive (`pure-lattice.md` §6); Boolean if bounded (`pure-lattice.md` §8). Marque correctly omits `BoundedLattice` because the universe `T` is generic and unbounded (per `crates/scheme/src/builtins.rs:115–118` doc).

**Verdict.** The standard interpretation. Use for SCI compartments, dissem-control sets at the token level, AEA, FGI countries — anything where "more tokens = more restrictive" by membership.

### 2.5 `IntersectSet<T>`

**What it does.** Powerset with **flipped operators**: `join = intersection`, `meet = union`. Used for REL TO, where the page-level releasable countries are the *intersection* of per-portion REL TO sets.

**Closest catalog entry.** `pure-lattice.md` §13 "Dual lattice" — flipping the order swaps meet and join. `security-lattice.md` §7 "Intersection-with-blackball (REL TO style)" gives the operational reading. Sources: `[davey-priestley-2002]` §1.18 (dual order); `[wikipedia-absorbing-element]`.

**Match quality: (a) exact match (with subtle naming hazard).** Mathematically, `IntersectSet<T>` is the *order dual* of `FlatSet<T>` — same elements, flipped order, hence flipped operators. The dual of a Boolean algebra is a Boolean algebra (`pure-lattice.md` §13: "integrity-vs-confidentiality is the canonical instance").

**Caveat.** A reviewer reading the `IntersectSet` source might be confused that `join` shrinks the set rather than growing it. The doc comment at `crates/scheme/src/builtins.rs:213–225` correctly explains: the engine calls `join` during `project`, and for REL TO the projection must shrink. Naming the type `IntersectSet` foregrounds the *operator* rather than the *order*; an alternate name would be `RelToLattice` or `OrderDualSet`. This is a doc/naming question, not a correctness question.

**Verdict.** Sound. Surface `pure-lattice.md` §13 if a future maintainer asks "why are the operators swapped?"

### 2.6 `SupersessionSet<T>`

**What it does.** Union with a post-filter that drops any token whose superseding peer is present. Configured by `&'static [(T, T)]` table.

**Closest catalog entry.** `security-lattice.md` §6 "Supersession algebra (NOFORN-style)" — Framing 1 (absorbing element / lifted lattice) for **intra-category** supersession. Sources: `[wikipedia-absorbing-element]`, `[denning-1976]` §3.

**Match quality: (b) partial match — important caveat.** Within a single category, `SupersessionSet` is a sound lattice operation: it's the union (a join) followed by a deterministic projection that retains only the maximal antichain under the supersession order. As long as the supersession table is acyclic and consistent, idempotence/commutativity/associativity hold (verified by the `debug_assert!` at `builtins.rs:389` and the equality-of-tables invariant). The shipped `meet` is plain intersection without supersession, which is sound because the spec never defines a "meet with supersession" (per the doc comment at line 403–408).

**The gap.** `security-lattice.md` §6 names *three* framings (absorbing element, quotient, non-lattice rewrite). `SupersessionSet` is **Framing 1 confined to intra-category use**. CAPCO's NOFORN-clears-REL TO is *cross*-category, which `SupersessionSet` cannot express by construction (the doc at lines 322–326 acknowledges this). That cross-category case is handled by `PageRewrite`, which is Framing 3 (deterministic non-lattice rewrite) — see §7 below.

**Verdict.** Sound for intra-category supersession. The cross-category case is correctly delegated to `PageRewrite`. Recommend the doc comment at `crates/scheme/src/builtins.rs:322–326` cite `security-lattice.md` §6 explicitly for future readers who want to know what algebraic structure they're using.

**Specific question to verify on the supersession table.** Is the CAPCO supersession relation transitively closed in the table, or does the engine rely on transitivity of the underlying order? E.g., if `A` supersedes `B` and `B` supersedes `C`, does the table need `(A, C)` explicitly, or does the post-filter naturally drop `C` when `A` is present (because `B` is dropped, leaving `A` to suppress `C` via the `(B, C)` pair which no longer fires)? Reading `apply_supersession` at lines 366–376: the loop builds a *flat* drop list from the present-superseding tokens, *then* filters. If `(B, C)` is in the table and `B` is dropped (because `A` is present and `(A, B)` fires), `(B, C)` does NOT fire because `B` is no longer "present" in the set used for the drop test... wait — actually, looking at the code more carefully (lines 369–374), the test is `set.iter().any(|u| u == superseding)` against the **input** set, before any drops. So `(B, C)` *would* still fire on the input set, even though `B` is itself being dropped. This means the user must transitively close the supersession table (or accept that intermediate-link tokens transit but their downstream tokens are also dropped). **This is a question worth raising with the user** — see §9, Q-2.6.

### 2.7 `ModeSet<T>`

**What it does.** Per-key max of observation counts. `join = component-wise max` (idempotent). `mode()` extracts the value with the highest count.

**Closest catalog entry.** No precise catalog entry. The closest is `pure-lattice.md` §11 "Product lattice" — `ModeSet` is `(P(T) → ℕ)` with coordinatewise max, which is a product of `OrdMax<u32>` over `T`.

**Match quality: (a) exact match by re-derivation.** `ModeSet<T>` is structurally `Product` indexed over `T` of `OrdMax<u32>`. Per `pure-lattice.md` §11, "Distributive iff each factor is" — `OrdMax<u32>` is a chain, hence distributive, hence `ModeSet<T>` is a distributive lattice. The doc comment at `crates/scheme/src/builtins.rs:436–445` correctly explains why `join` is per-key max rather than per-key sum (idempotence requires it).

**Verdict.** Sound. Used by hypothetical "corporate / medical" schemes that prefer "most common sensitivity" over "most restrictive." No CAPCO use today; flag as exercised by Phase F (CUI) work or later.

### 2.8 `MaxDate`

**What it does.** Wraps `Option<NaiveDate>`; `join = later`. `top = NaiveDate::MAX`, `bottom = None`.

**Closest catalog entry.** `security-lattice.md` §8 "Declassification / exemption orderings" — date chain with max-as-join.

**Match quality: (a) exact match.** Dates form a total order; max is the join in the chain. Marque correctly implements `BoundedLattice` because `NaiveDate::MAX` *is* enumerable.

**Caveat the consultant should surface.** Per `security-lattice.md` §8, declassification is more than dates: exemption codes (`50X1-HUM`, `25X1-human`) sit *above* all dates as a flat antichain, and the composite is a join-semilattice without a meaningful meet. `MaxDate` only handles the date side; the exemption-code side is not yet modeled. The May 1 lattice plan §8 acknowledges this: "AEA exemption strings... supersession ordering — #266 deferred."

**Verdict.** Sound for what it covers. The exemption-code lattice extension is open work; per `security-lattice.md` §8 the right algebraic name for the composite is "bounded join-semilattice with adjoined antichain top." Surface this if/when the user asks how to extend.

### 2.9 `OptionalSingleton<L>`

**What it does.** Wraps `Option<L>`; `None` is bottom; `Some(a) ⊔ Some(b) = Some(a.join(b))`.

**Closest catalog entry.** Pattern recognized in domain theory as the *lifting* of a lattice with an artificial bottom. `pure-lattice.md` doesn't have a dedicated entry (it's a small categorical construction); see `abstract-interp.md` §12 "Constant-propagation lattice" for the related "flat lattice" pattern (`{⊥} ∪ V ∪ {⊤}`), which is `OptionalSingleton` plus a synthetic top.

**Match quality: (a) exact match for the "lifted bottom" construction.** The forwarding semantics — `Some(a) ⊔ Some(b)` lifts the inner `join` — is correct. `OptionalSingleton<L>` is `BoundedLattice` iff `L: BoundedLattice` (per `crates/scheme/src/builtins.rs:701–714`).

**Verdict.** Sound. Standard pattern; no redesign needed.

### 2.10 `Product<A, B>`

**What it does.** Pair of two lattices; coordinatewise join and meet.

**Closest catalog entry.** `pure-lattice.md` §11 "Product lattice" — exact construction.

**Match quality: (a) exact match.** Per `pure-lattice.md` §11 "Categorical characterization": this is the categorical product in the category of lattices. Inherits boundedness, distributivity, modularity, completeness from the factors.

**Verdict.** Sound. The standard composition primitive. Note that "constraint between coordinates" is *not* a product but a sublattice (or congruence) — see `pure-lattice.md` §11 "Non-example" for the warning.

### 2.11 `SciSet`

**What it does.** Per-control-system, per-compartment, per-sub-compartment hierarchy. `join = component-wise union` (per-system, per-compartment, per-sub-comp). `meet = §3.3a equal-depth intersection`.

**Closest catalog entry.** `security-lattice.md` §18 "SCI / compartmented information as a hierarchical lattice" — meet/join-semilattice on `P(CompartmentTokens)` keyed by control system, no top because compartments are agency-extensible. Sources: CAPCO-2016 §A.6 (SCI grammar), `[dni-capco-register]` §B (registered control systems), `[davey-priestley-2002]` §2.5 (semilattices).

**Match quality: (b) partial match — needs the §3.3a question diagnosed.**

The `join` is a sound lattice operation: union at each level. Idempotent, commutative, associative, absorbing. `pure-lattice.md` §11 "Product lattice" is the formal home (a `Product` of `FlatSet`s indexed by `system × compartment`).

The `meet` is the issue. The §3.3a doc comment (`crates/capco/src/lattice.rs:14–39`) and the recursive-lattice plan §3.3a (`docs/plans/2026-04-19-recursive-lattice-and-decoder.md:313–339`) both acknowledge that "tree intersection is not unique" and that the implementation picks one of three reasonable policies. A standard lattice meet is uniquely determined by the order; if the meet on `SciSet` is not uniquely determined, then either (a) the order is being underspecified in the docs, or (b) the operation called `meet` is not actually a lattice meet on the natural order. See §4 below for the full diagnosis.

**Verdict.** `join` is sound. `meet` requires either accepting that it is *not* a lattice meet (rename it; expose `overlaps` / `common_compartments` as the actual operations callers want) or pinning down the order under which the policy-(b) behavior *is* the meet. See §4.

**Open question for the user.** See §9, Q-4.

### 2.12 `SarSet`

**What it does.** Same shape as `SciSet` but for SAR programs (program → compartment → sub-compartment).

**Closest catalog entry.** `security-lattice.md` §19 "SAR / Special Access Required as a forest of programs" — same structural argument as SCI: agency-extensible, no top, meet/join-semilattice on `P(SAR-tokens)`.

**Match quality.** Same verdict as `SciSet`. The `join` is sound; the `meet` inherits the §3.3a issue. Operationally, CAPCO `SAR` is even more open than SCI (the public CVE is intentionally empty per §H.5; the May 1 plan §5 confirms this).

**Verdict.** Same as `SciSet`. See §4 for the meet diagnosis.

### 2.13 `FgiSet`

**What it does.** Two-state enum (`None | Present { concealed, countries }`) with `join` that supersedes acknowledged-with-countries by concealed-without-countries. `meet` does the dual.

**Closest catalog entry.** Two compose:
- `security-lattice.md` §17 "Foreign-government-information (FGI) and tetragraph lattices" for the country-set Boolean algebra side.
- `security-lattice.md` §6 "Supersession algebra" Framing 1 (absorbing element) for the concealed-supersedes-acknowledged behavior.

**Match quality: (b) partial match — the supersession-as-absorbing-top design is unusual.**

`FgiSet` *does* implement `BoundedLattice` (`crates/capco/src/lattice.rs:556–568`). The top is `Present { concealed: true, countries: ∅ }`. The justification is operational: source-concealed FGI dominates source-acknowledged FGI (revealing the country list would compromise the concealed source).

This is the **absorbing-element Framing 1** from `security-lattice.md` §6 — the structure is engineered to make supersession into a top element, lifting the lattice to a bounded one. Per §6: "all lattice laws hold including idempotence and commutativity, but the resulting structure has an artificial bottom whose semantics ... is operationally distinct from 'empty REL TO'." The same caveat applies in reverse for `FgiSet`'s top.

**Caveat the consultant should surface.** The FGI top is operationally meaningful (it's "this document has FGI from a concealed source, no list possible") rather than synthetic. So the analogy to `security-lattice.md` §6's "artificial bottom" is partial. The construction is more honestly named as a *bounded distributive lattice* that *happens* to encode the concealment supersession in its top.

**Subtle bug worth verifying.** In `FgiSet::meet` at lines 515–553, when both sides are `Present` with no shared countries and neither is concealed, the result collapses to `Self::None`. The doc comment at lines 537–543 acknowledges this as a representation-level fall-back ("falls back to None as the 'no shared FGI' answer"). Lattice-wise: `Present { concealed: false, countries: {GBR} } ⊓ Present { concealed: false, countries: {DEU} } = None`. But `None ⊑ Present` in the natural lattice order, so the result is correct *as a lower bound*. The question is whether `None` is the *greatest* lower bound — i.e., whether there's an `FgiSet` value `X ≠ None` such that `X ⊑ Present { GBR }` and `X ⊑ Present { DEU }`. The answer: no, because any `Present { countries: S }` with `S ≠ ∅` requires `S ⊆ {GBR}` and `S ⊆ {DEU}`, hence `S ⊆ ∅`, contradiction. So `None` is correct. Verified informally; a property test would lock this in.

**Verdict.** `FgiSet` is a sound bounded distributive lattice. The mapping to `security-lattice.md` §6 is "Framing 1 with a non-synthetic top" — flag this in the doc comment for future maintainers.

### 2.14 `Category` + `AggregationOp` + `CategoryShape`

**What they do.** `Category` is the per-category descriptor; `AggregationOp` is build-time shorthand naming a built-in lattice constructor; `CategoryShape` is inspection metadata returned from `Category::shape()`.

**Closest catalog entry.** No single entry; this is the marque-internal mapping from "category description" to a Tier-1/Tier-2 lattice (per recursive-lattice plan §3.2/§3.3). The closest analogue is `universal-algebra.md` §2 "Signature" — `AggregationOp` is the build-time signature of the per-category lattice.

**Match quality: (c) not a lattice problem.** This is a metadata-and-shape-routing layer, not a lattice itself. The decision (per recursive-lattice plan §3.4 "Why not keep `AggregationOp`") to retire `AggregationOp` from runtime dispatch and have the engine call `Lattice::join` directly is correct: it removes a category of indirection bug where the dispatch enum and the lattice impl could disagree.

**Verdict.** Sound design. Note the lingering `AggregationOp::Custom` for SAR/AEA/FGI categories at `crates/capco/src/scheme.rs:637/653/681` — these categories *do* have lattice impls (`SarSet`, `FgiSet`) but the build-time `Category` declaration says `Custom` because the lattice isn't expressible by a single built-in constructor. The Phase B doc anticipates this: "`Custom` flags this for Phase B so the engine does not silently replace `PageContext::expected_*` with a plain union." This is a back-compat scaffold; the May 1 lattice plan PR 4 cuts it.

### 2.15 `Scope` enum

**What it does.** Four variants tagging the projection mode: `Portion`, `Page`, `Document`, `Diff`.

**Closest catalog entry.** `pure-lattice.md` §16 "Lattice congruence" is loosely related (different scopes are different *quotient* views of the same underlying marking lattice — a portion, a page, a document each pick a different equivalence on portion sets). But this is more naturally a **monoid action** indexing — `Scope` is essentially "which fold over the per-portion list to apply."

**Match quality: (c) not a lattice problem.** `Scope` is a mode selector, not an algebraic structure. Treat it as a simple enum.

**Verdict.** Sound. The decision to put `Diff` behind a separate `DiffInput<M>` rather than as a `Scope` variant carrying references (per recursive-lattice plan §7.1) is correct: it avoids forcing every `Scope` value to carry a lifetime.

### 2.16 `PageRewrite` and the topological scheduler

**What it does.** Post-aggregation cross-category mutations on a page-projected marking. The engine sorts rewrites topologically by `reads`/`writes` axes at `Engine::new` time and runs them in that order.

**Closest catalog entries.**
- `security-lattice.md` §6 "Supersession algebra (NOFORN-style)" Framing 3 (non-lattice deterministic rewrite) — operational fit.
- `abstract-interp.md` §18 "Topological scheduling as a fixed-point computation" — the formal framing.
- `pure-lattice.md` §19 "Knaster-Tarski" — what convergence relies on if iteration were used (it isn't; the scheduler runs each rewrite once in topological order).

**Match quality: (a) exact match for the Knaster-Tarski formalism, (b) partial match for what is actually shipping.**

The shipped scheduler runs each rewrite *once* in topological order. This is correct iff the rewrite set is "rooted" — each rewrite's effect doesn't reactivate an earlier rewrite's trigger. Iff the rewrites are monotone *and* the read/write graph is acyclic, the Kahn-sort-then-once-each schedule reaches `lfp F` (where `F = s ⊔ r₁(s) ⊔ … ⊔ rₖ(s)`) without iteration.

The shipped CAPCO rewrites are:
1. `noforn-clears-rel-to`: writes REL TO (clear), reads dissem and REL TO.
2. `joint-promotion`: writes REL TO (promote from joint-classification), reads joint-classification.
3. `fgi-absorption`: writes FGI (transform from FGI), reads FGI.

The dependency graph is: `joint-promotion → noforn-clears-rel-to` (joint-promotion writes REL TO, noforn reads REL TO). `fgi-absorption` is independent. Topological sort yields `[joint-promotion, noforn-clears-rel-to, fgi-absorption]` (or a permutation that puts fgi-absorption anywhere).

**Verdict.** Sound *if* each rewrite is monotone and idempotent. The current `joint-promotion` and `fgi-absorption` use `never_fires` triggers (per scheme.rs:507–514, 535–536), so the question of monotonicity is currently moot for them — they are no-ops. When their bodies land in Phase D/E, the implementer must verify monotonicity (`s ⊑ r(s)` for the inflationary lift, OR more precisely `r(s) ⊑ s ⊔ r(s)` since these rewrites *can* shrink categories like REL TO). See §6 below for the full diagnosis.

**Architectural recommendation.** Cite `abstract-interp.md` §18 ("topological-schedule case") in the engine's scheduler doc-comment so a future maintainer can find the formal justification. The scheduler is sound as-stated; the citation prevents re-deriving the proof under pressure.

### 2.17 `Constraint` enum

**What it does.** Five variants declaring scheme invariants. Dyadic variants (`Conflicts`, `Requires`, `Implies`, `Supersedes`) are evaluated by the generic `evaluate()` walker; `Custom` dispatches to scheme-specific predicates.

**Closest catalog entry.** No single entry; constraints are about *forbidding combinations of lattice elements*, not about lattice operations themselves. Relevant comparisons:
- `pure-lattice.md` §11 "Non-example" — "Two coordinates with a *constraint* between them ... that's a *sublattice* of the product, not the full product." Constraints carve sublattices.
- `security-lattice.md` "When a security policy is NOT a lattice" §4 (mutually-exclusive markings) — the canonical "constraints are guards, not lattice elements" framing.
- `universal-algebra.md` §6 "Identities and equational theory" — declarative invariants are equational identities the algebra is meant to satisfy.

**Match quality: (c) not a lattice problem.** Constraints are a separate algebraic surface from the lattice. They are *guards* on the lattice (which combinations are admitted); the lattice itself is the algebra of admitted combinations.

**Verdict.** Sound. The split between the lattice (the algebra) and the constraint catalog (the guards) is the correct shape and matches the literature consensus that mutually-exclusive markings live outside the lattice (per `security-lattice.md` "When a security policy is NOT a lattice" §4).

**Open question for the user.** Many CAPCO rules will collapse to `Constraint::Custom` because the predicate is n-ary or context-dependent. The risk is that `Custom` becomes a junk-drawer that hides what would otherwise be cleaner cross-axis lattice invariants. See §3 for the PR 3b walk-through.

### 2.18 `MarkingScheme` trait

**What it does.** The aggregate trait combining lattice (via `Marking: Lattice`), categories, constraints, page rewrites, parsing, validation, projection, and rendering.

**Closest catalog entry.** No single entry; this is a *signature* combining a lattice, a constraint catalog, a rewrite set, and parse/render. The closest analogue is `universal-algebra.md` §2 "Signature" + §3 "Variety" — `MarkingScheme` declares a signature; instances of the trait are algebras of that signature; the variety is "all markings the scheme admits."

**Match quality: (b) partial fit to a universal-algebra framing, but mostly (c).** `MarkingScheme` is application infrastructure; calling it a "variety" overstates the algebraic claim. Verdict: name it as a **multi-sorted signature with guarded operations** if the user wants formal vocabulary, but don't reach for the variety machinery — it doesn't pay off here.

**Verdict.** Design is sound. The decision to put `Marking: Lattice` as the bound (rather than a richer constraint like `BoundedLattice` or `Distributive`) correctly admits open-vocabulary domains.

### 2.19 `project()` method

**What it does.** Reduces a slice of `Marking` values to a single `Marking` under a `Scope`. For `Scope::Page`, the documented semantics is "component-wise category joins, then run `page_rewrites()` in declaration order" (per scheme.rs:131 doc comment). The engine's scheduler runs them in *topological* order, not declaration order — the doc comment lags.

**Closest catalog entry.** `pure-lattice.md` §15 "Lattice homomorphism" — `project` is *not* a homomorphism in general (rewrites can break the structure-preservation property). For the pure category-wise reduction (without rewrites), each category's reduction is the lattice fold (`reduce f .join` over the per-portion values), which is a left-fold of the binary join — sound by associativity.

**Match quality: (a) exact match for the reduction phase, (c) for the rewrite phase (rewrites are not lattice operations).**

**Caveat.** Per the May 1 plan §1.2 and the 2026-05-02 plan §1.2, `CapcoMarking::join` currently delegates to `PageContext` instead of doing component-wise lattice joins per category. The 2026-05-02 plan calls this out as **violating the lattice laws** — PR 4 fixes it. Until then, the trait doc and the implementation disagree.

**Verdict.** Per the plans, this is a known pre-PR-4 problem. After PR 4 the reduction phase is sound; the rewrite phase is correctly framed as deterministic-order, not as a lattice op (see §6).

### 2.20 §3.3a equal-depth meet

See §4 below for the full applied diagnosis. Spoiler: the meet is **not** a lattice meet on the natural inclusion order; it is a *structurally-aligned partial intersection* that approximates the categorical meet on the subsets-of-trees structure. The user's policy choice (b) is defensible operationally but should not be called `meet` without a footnote; the alternative is to rename it (e.g., `aligned_intersection`) and let `Lattice::meet` either delegate or pick a different policy.

### 2.21 NOFORN clears REL TO

See §7 below. Spoiler: this is `security-lattice.md` §6 Framing 3 (deterministic non-lattice rewrite), correctly implemented as a `PageRewrite`, NOT as an absorbing-element lattice extension.

### 2.22 REL TO intersection-with-blackball

The composition of (1) `IntersectSet` join + (2) `noforn-clears-rel-to` page rewrite. Mapped onto `security-lattice.md` §7 "Intersection-with-blackball (REL TO style)". Match quality: **(a) exact**. This is the textbook formalization of the operational REL TO rule. Per `security-lattice.md` §7: the country-set Boolean algebra under intersection IS the meet of `P(Countries)`; the absorbing-element extension is the NOFORN override.

**Verdict.** Sound. Consultant should cite `security-lattice.md` §7 if a user asks "what's the formal name for our REL TO algebra?"

### 2.23 Open-vocab compartments

See §5. Spoiler: this is exactly the `pure-lattice.md` §4 "open-set / agency-extensible warning" + `security-lattice.md` §18 "agency-extensibility ⇒ no top" + `universal-algebra.md` §11 "almost-lattice diagnostic" Axis F "no top." Marque correctly omits `BoundedLattice` from `SciSet` and `SarSet`. The audit in §5 verifies no operation conflates `top()` with "any plausible upper bound."

### 2.24 Decoder confidence propagation

See §8. Spoiler: this is `abstract-interp.md` §19 "Confidence / posterior propagation as abstract interpretation" with a finite-discretized score lattice. Marque's `combined() = recognition * rule` is monotone and the score domain is `[0, 1]`. Per `abstract-interp.md` §19: "If the score domain forms a lattice with a meaningful order ... and the per-step combiner is monotone ..., then propagation is exactly a monotone framework analysis."

### 2.25 The pivot type (`IsmAttributes` → `ParsedAttrs`/`CanonicalAttrs`/`ProjectedMarking`)

**What it does (post-PR 3a).** Three distinct types representing the parser's output, the canonical-form-after-canonicalization, and the page-roll-up output.

**Closest catalog entry.** `abstract-interp.md` §1 "Galois connection as program-analysis abstraction" — there *is* an abstraction relationship between these types (parser output is the concrete; canonical form abstracts it; the projected marking abstracts further). This is a (b) fit: the literature offers Galois-connection vocabulary if the user wants to formalize the relationship between the three types.

**Match quality: (b) partial match.** The pivot-type split is *primarily* a type-system soundness move, not a lattice move. The Galois-connection framing is available if needed. Verdict: don't over-formalize; the type split is its own justification.

**Open question.** Whether `MarkingScheme::canonicalize: ParsedAttrs<'_> → CanonicalAttrs` (per 2026-05-02 plan §3.1) is monotone in any meaningful order on `ParsedAttrs`. If yes, the canonicalization-and-projection composition is a monotone operator, and Knaster-Tarski-style reasoning becomes available for "is this fix point unique?" questions. This is speculative — flag as future work (see §9, Q-2.25).

---

## §3. The PR 3b stall — applied diagnosis

PR 3b is the rule-collapse PR: 49 (or "~56" per the looser figure) hand-written rules collapse to 8–18, each with a single CAPCO citation and ≤3 internal branches in the predicate body (per `/home/knitli/marque/.worktrees/fix-gemini/specs/006-engine-rule-refactor/plan.md:230` D13). The stall is that the team has the lattice constructors but lacks the formal apparatus to decide which rules collapse onto which lattice operations.

This section walks the collapse as a lattice-algebra exercise. Rules group by what they actually test:

### 3.0 Two governance principles for the collapse

Two principles from the user's structural map govern how rules are sorted into buckets. Both are routinely-violated heuristics in PR-3b's prior iterations; making them explicit in this section keeps the §3.1–§3.8 binning honest.

**3.0.a — Form is not shape.**

Delimiter choice (`/` vs. `//` vs. `,` vs. space-separated), token sort order (alphabetical, numeric-then-alpha, USA-first), banner-vs-portion abbreviation (`REL TO` vs. `REL`, `EYES ONLY` vs. `EYES`), and inter-category position within a banner are **renderer concerns**, not lattice axes. They are the *form* of a marking, not its *shape*. Two markings that differ only in form are *lattice-equal* in every relevant axis; the renderer chooses a canonical representative.

The user's claim, quoted: "The grammar is unique enough that order doesn't matter in >99% of cases." Two consequences for the consultant's verdicts:

- **Don't bucket form rules into Phase A or Phase B.** A rule that fires on "trigraphs out of alphabetical order" is a *renderer correctness* check (Phase C), not a lattice-law test (Phase A) and not a constraint over the fact-set (Phase B). It belongs in §3.5 / §3.6, then in Phase C, then in renderer property tests.
- **Form drift over time is a parser concern, not a shape concern.** Historical CAPCO documents use comma-vs-`/` separation in REL TO, EYES alone vs. spelled-out lists, and AEA/SAP at different banner positions. The decoder / corrections channel handles form drift; the lattice never sees it because parser normalization happens upstream.

This principle is what makes the §3.4.5 RELOPT (REL portion abbreviation) entry tricky: it has a *parser-side round-trip obligation* that crosses the form/shape boundary. See §3.4.5 for the resolution.

**3.0.b — Structure rules are categorically distinct from other-purpose rules.**

The user's map separates "system rules" / "structure rules" (the algebraic shape of the marking system) from rules that serve *different purposes*:

| Rule purpose | Goes where | Example |
|---|---|---|
| Structure / system | Phase A (lattice + closure + transmutation) — §3.1, §3.4 | "FGI on contact with US-class promotes to FGI [LIST]" |
| Constraint / acceptance | Phase B (`Constraint::Conflicts` / `Requires` / abhors-company) — §3.2, §3.3 | "RELIDO incompatible with NOFORN" |
| Style / suggestion | Phase C (renderer) — §3.5, §3.6 | "Banner mixes abbreviated and long forms" |
| Accompanying requirement | Out-of-engine (admonition emitter, audit trailer) — §3.4.4 sidebar | "Page carries RD must include RD warning notice" |
| Conflict resolution / disambiguation | Decoder / corrections — outside §3 entirely | "SI - EXCEPTIONALLY CONTROLLED INFORMATION → SI" |
| Override / customization | Configuration surface — outside the lattice | User opts out of implicit-NOFORN closure for specific markings |

**Why this matters for PR 3b's count.** A rule that does two of these jobs — say, *both* "compute the canonical render order" and "warn if input doesn't match" — is double-counted by the bucket view in §3.9. The phase view in §3.10 is explicit about which job each rule does *primarily*; jobs that don't fit Phase A/B/C are out-of-scope for the collapse target and should be retired, deferred, or moved to a separate channel rather than counted as surviving rules.

**Override requirement (orthogonal to the bucket).** The user's design constraint: "every rule must be overridable at compile time, and severity adjustable at runtime." The constitution V (audit-first compliance) plus FOIA-induced spec lag (the user's "we know CAPCO 2016 isn't the latest because ISM has tokens that aren't in our version") means the runtime config surface must accept severity overrides for *every* rule in the catalog — not just legacy ones. This is a configuration-surface invariant, not a Phase A/B/C concern, but it constrains the choice between "single walker rule + declarative entries" and "one rule per entry" in Move 3 (see §3.10.3): the catalog-entry form must support per-entry severity override even if the *rule* is one walker.

### 3.1 Rules that test a lattice-law identity (collapse target: lattice property)

**Pattern.** "Is the banner the join of the per-portion classifications?" "Are the SCI compartments on the banner the join of the per-portion compartments?" These are *monotonicity* / *correctness-of-join* tests — they check that the engine's projection equals the algebraic join.

**Examples.**
- **E035 (SCI banner roll-up)**: tests that `banner.sci_set == join_over_portions(p.sci_set)`. After PR 4, this is *automatic* — `Engine::project(Scope::Page, ...)` produces the join by construction. The rule disappears as a property of the engine, NOT as a rule.
- **E040 (NODIS/EXDIS banner roll-up)**: similar shape; tests that the dissem set on the banner equals the join (with supersession) of the per-portion dissem sets. After PR 4 with `SupersessionSet` in the dissem category, this is also automatic.
- **E031 (SAR banner roll-up)**: same shape for `SarSet`. Automatic post-PR 4.

**Catalog citation.** `pure-lattice.md` §15 "Lattice homomorphism" — `project` should be a homomorphism on each category; tests that it is can be replaced by property tests of the underlying lattice impl.

**Recommendation.** These rules collapse to **lattice property tests** — which are NOT runtime rules, they are `cargo test` assertions. They disappear from the rule count entirely once the property tests in `crates/capco/tests/lattice_laws.rs` and `proptest_lattice.rs` cover them. **Estimated count: 5–7 rules disappear (banner roll-up rules across SCI, SAR, dissem, FGI, REL TO, declassify-on, classification).**

### 3.2 Rules that enforce a `Constraint::Conflicts` invariant

**Pattern.** "These two tokens cannot co-occur in one marking." Examples: NOFORN ∦ REL TO, RD ∦ FRD, NODIS-and-EXDIS-in-portion (E041), conflicting SCI control systems.

**Examples.**
- **E041 (NODIS supersedes EXDIS in portion)**: `Conflicts(NODIS, EXDIS)` at portion scope — actually this fires only when both are in the same portion; rule is "drop EXDIS." More precisely a `Supersedes` than a `Conflicts`.
- **E022 / E024 / E025**: token-level conflicts already declarative per `crates/capco/src/rules_declarative.rs:578/618/670`. Already collapsed.
- **E037 (NODIS-EXDIS portion conflict)**: see entry in `rules_declarative.rs:761`.

**Catalog citation.** `security-lattice.md` "When a security policy is NOT a lattice" §4 "Mutually-exclusive markings" — the canonical "constraint, not lattice element" pattern. `pure-lattice.md` §11 "Non-example" warns that constraints carve sublattices.

**Recommendation.** These collapse to **`Constraint::Conflicts` entries** in the scheme's constraint catalog. The generic `evaluate()` function fires diagnostics. **Estimated count: 8–12 rules collapse to ~8–12 `Conflicts` entries (no compression).**

### 3.3 Rules that enforce a `Constraint::Requires` invariant

**Pattern.** "If A is present, B must also be present." Examples: HCS requires NOFORN (E021 in `rules_declarative.rs:541`), SAR requires NOFORN (Conditional under §B.3 — see May 1 plan §5 Open Question 2; not a blanket rule), CNWDI requires classification ≥ S, JOINT requires REL TO with all participants.

**Catalog citation.** `security-lattice.md` §6 "Supersession algebra" Framing 1 (Implies as the dual of supersession on the lattice).

**Recommendation.** These collapse to `Constraint::Requires` entries. The generic `evaluate()` fires diagnostics; the engine separately tries to repair (insert the missing token) via `FixIntent` (post-PR 3c). **Estimated count: 4–6 rules collapse to 4–6 `Requires` entries.**

### 3.4 Rules whose collapse target is a `PageRewrite`

**Pattern.** "On the banner, if A is present in category X, drop B from category Y" — cross-axis transformations, not constraints.

**Examples.**
- **NOFORN clears REL TO**: already `PageRewrite::declarative` per scheme.rs:486–498. Rule retired (was implicit in `PageContext::expected_rel_to`).
- **JOINT promotes foreign countries to FGI**: stubbed at scheme.rs:515–526 pending Phase D/E.
- **Unattributed FGI absorbs attributed FGI**: stubbed at scheme.rs:537–548. NOTE: this is a *within-axis* rewrite (FGI → FGI), not cross-axis.
- **E039 (NODIS/EXDIS clears banner REL TO)**: structurally analogous to NOFORN-clears-REL TO; should collapse to a `PageRewrite::declarative`.

**Catalog citation.** `security-lattice.md` §6 Framing 3 (deterministic non-lattice rewrite) + `abstract-interp.md` §18 (topological scheduling). See §6/§7 below for the full diagnoses.

**Recommendation.** These collapse to `PageRewrite` entries. **Estimated count: 4–6 rules become 4–6 declarative `PageRewrite` entries.** Each is a single declaration, not a hand-written predicate.

#### 3.4.1 The transmutation roster (six declarative entries)

The user's structural map (received during PR 3b consultation, mirrored here for reference) inventories six "Transmutes on Contact" rewrites. Each is a deterministic cross-axis transformation: the trigger fires on a fact-set predicate, the rewrite removes one fact and emits a different one. They share the `PageRewrite` shape; together they comprise §3.4's declarative roster.

A `PageRewrite` carries `reads` and `writes` axis annotations so the topological scheduler in `marque-engine::scheduler` can order them after the per-axis joins they consume and before any rewrites that consume their output. Citations cite `[capco-2016]` (the manual is vendored at `crates/capco/docs/CAPCO-2016.md`); cross-check `capco-context.md` for the per-marking matrix that grounds each entry.

**Entry 1. Bare-FGI portion contacts US-class portion → roll up to `//FGI [list]//`.**

```text
reads:    [Class, FgiAttribution]
writes:   [Class, FgiAttribution]
guard:    page.class.has_us_classification ∧ ∃ portion. portion.attribution = bare(FGI-bare, _, _)
apply:    page.fgi_attribution := ⊤(union of trigraphs from all bare FGI atoms)
          page.class := reciprocal_raise(page.class) — see Note (i) below
```

Citation: `capco-context.md` §3.1 step 3 (FGI banner roll-up: "if all portions have unconcealed FGI, banner is FGI [LIST]; if any portion has concealed FGI, banner is bare FGI"); §H.7 prose. The cross-axis trigger is the user's clarification: "you only ever see [bare] in a banner if the entire document is [that bare form]."

**Entry 2. Bare-FGI-R portion contacts US-class portion → US ≥ C with FGI [list].**

```text
reads:    [Class, FgiAttribution]
writes:   [Class, FgiAttribution]
guard:    page.class.has_us_classification ∧ ∃ portion. portion.attribution = bare(FGI-bare, {R}, _)
apply:    page.class := max(page.class, C)        — R is RESTRICTED, lifts to ≥ C in US-equivalent
          page.fgi_attribution := ⊤(trigraphs ∪ {countries with R-class})
```

Citation: `capco-context.md` §H.7; user's map "[FGI/TRIGRAPH] R" transmutation.

**Entry 3. JOINT [list] portion contacts non-US-class portion → US // FGI [non-US JOINT members]//.**

```text
reads:    [Class, FgiAttribution]
writes:   [Class, FgiAttribution]
guard:    ∃ portion. portion.attribution = bare(JOINT, list, class) ∧
          ∃ other portion. (other.class ≠ portion.class ∨ other.attribution ∉ {⊥, this same JOINT bare}))
apply:    page.class := reciprocal_raise(max(page.class, list_max_class))
          page.fgi_attribution := ⊤(list \ {USA})  — drop the US member; non-US members become FGI tags
```

Citation: `capco-context.md` §5.2 (JOINT banner-precedence prose: "JOINT marking at portion stays at portion; *does not roll up* to banner in US documents — banner becomes the highest US class with FGI [LIST]"); user's clarification on JOINT/USA-implicit.

**Entry 4. FRD-SIGMA # contacts RD-SIGMA → consolidated RD-SIGMA + FRD-SIGMA #.**

```text
reads:    [AEA]
writes:   [AEA]
guard:    page.aea ⊇ {RD-SIGMA #_a} ∧ page.aea ⊇ {FRD-SIGMA #_b}
apply:    page.aea := page.aea ∪ {RD-SIGMA tracking [#_a, #_b]}  — RD-SIGMA wins as parent;
          FRD-SIGMA-numbers carry forward as RD-SIGMA-numbers per §H.6 p113 (FRD evicted by RD)
```

Citation: `capco-context.md` §5.5 row "FRD-SIGMA [#]" — "RD-SIGMA wins over FRD-SIGMA in banner"; §H.6 p113 (FRD evicted from banner if any RD portion present, p104). User's map confirms.

**Entry 5. ORCON-NATO contacts US-class → US ORCON.**

```text
reads:    [Class, Dissem]
writes:   [Dissem]
guard:    page.class.has_us_classification ∧ page.dissem ⊇ {ORCON-NATO}
apply:    page.dissem := (page.dissem \ {ORCON-NATO}) ∪ {ORCON}
```

Citation: `capco-context.md` §5.7 row "ORCON" — "ORCON wins over ORCON-USGOV in banner"; user's map confirms ORCON-NATO transmutes to US ORCON on US-class contact.

**Entry 6. {LES-NF, SBU-NF} contacts any IC marking → NOFORN // {LES, SBU}.**

```text
reads:    [Class, Dissem, NonIcDissem]
writes:   [Dissem, NonIcDissem]
guard:    portion.non_ic ⊇ {LES-NF}  ∨  portion.non_ic ⊇ {SBU-NF}
          ∧ portion.has_ic_marking      — any IC SCI/SAP/AEA/IC-dissem fact
apply:    if portion.non_ic ⊇ {LES-NF}:
              portion.dissem ∪= {NOFORN}; portion.non_ic := (portion.non_ic \ {LES-NF}) ∪ {LES}
          if portion.non_ic ⊇ {SBU-NF}:
              if portion.class ≠ U:        — SBU abhors classification per user's map
                  portion.non_ic := portion.non_ic \ {SBU-NF}  — drop SBU entirely on classified portion
              else:
                  portion.dissem ∪= {NOFORN}; portion.non_ic := (portion.non_ic \ {SBU-NF}) ∪ {SBU}
```

Citation: `capco-context.md` §5.8 rows "SBU-NF" and "LES-NF"; user's map "Transmutes on Contact" / "SBU abhors classification" notes. This is a **portion-level** rewrite (operates within a single portion as it's commingled with IC content), unlike entries 1–5 which are page-level. The scheduler should run portion-level rewrites before any per-axis page-level join.

**Note (i): Reciprocal class raise as a portion-parse-time normalization.**

Entries 1–3 reference `reciprocal_raise(class)`. The user's map: "U.S. reciprocates foreign classification levels, so when something transmutes to U.S. classification, it always resolves to the highest level overall." Concretely:

```text
class chain (total order, descending restrictiveness):
   TS > CTS > S > NS > C > NC > R > NR > U > NU
   (US always wins on tie — see §3.4.2 Note ii)
reciprocal_raise(non-US-class) returns the equivalent US-side class:
   CTS → TS, NS → S, NC → C, NR → R, NU → U
reciprocal_raise(US-class) is identity.
```

Two implementation options. **(a)** Run reciprocal raise at portion-parse-time: a `(//CTS//) ...` portion parses to fact-set `{Class = TS, FGI-attribution = bare(FGI-bare, {NATO}, CTS)}` — class is already raised, FGI atom retains the original-class for renderer use. **(b)** Run reciprocal raise as a class-axis `PageRewrite` that runs before `OrdMax` (i.e., on every portion individually). Option (a) is simpler and avoids a same-axis rewrite with self-feedback; the consultant's recommendation is (a). The class-axis `OrdMax` then operates over the chain TS > CTS > S > … with tie-break naturally embedded by the chain ordering (TS dominates CTS in the chain, so US-TS wins over reciprocally-raised-to-TS NATO-CTS by chain ordering of the original level).

**Note (ii): "US wins when tied" is encoded in the chain order.**

The chain `TS > CTS > S > NS > C > NC > R > NR > U > NU` has US and non-US levels at *adjacent* (not equal) positions. So `OrdMax(TS, CTS) = TS` by chain-strict-greater. The user's clarification "US always wins when tied" is satisfied by the chain ordering itself; no separate tie-break logic is needed in the lattice — the order already encodes the policy.

#### 3.4.2 RELIDO incompatibility roster (Constraint::Conflicts with family predicates)

The user's revised domination table broadens RELIDO incompatibilities. RELIDO is dominated by:

```text
NOFORN / LES-NF / SBU-NF / JOINT / FGI [any] / NATO [any] / DISPLAY ONLY > RELIDO
```

By definition (CAPCO-2016 §H.8 p154; `capco-context.md` §5.7 row "RELIDO") any release decision must go to the originator/owning-country, so an Information Disclosure Official (IDO) cannot make a release decision for content that carries foreign equity or NOFORN-style restrictions. The conflict has two natural groupings: **FD&R-family domination** (RELIDO is below NOFORN-style FD&R restrictions in the FD&R supersession chain — see §3.0.a "form is not shape" applied to the supersession axis) and **non-US-family conflict** (RELIDO is structurally incompatible with FGI/JOINT/NATO equity, since the IDO has no authority to release foreign-owned content).

**Family-predicate framing (recommended).** Express the roster as **two `Constraint::Conflicts` entries** with a family-predicate RHS, each covering one of the two groupings:

| Entry | LHS | RHS family | Scope | Citation | Rationale |
|-----|-----|-----------|-------|----------|-----------|
| F1 | `RELIDO` | `is_fdr_dominator(t)` — `t ∈ {NOFORN, LES-NF, SBU-NF, DISPLAY ONLY}` | Portion + Page | §H.8 p154 | FD&R supersession chain: NOFORN-style restrictions exclude foreign release; IDO cannot override |
| F2 | `RELIDO` | `is_non_us_atom(t)` — `t ∈ {any FGI atom, any FGI[list], any JOINT atom, any JOINT[list], any NATO atom (CTS / NS / NC / NR / NU / ATOMAL / BOHEMIA / BALK / ORCON-NATO)}` | Portion + Page | §H.7, §H.3 p56, §H.7 Appendix B, §H.8 p154 | release decision belongs to originating country / JOINT participants / NATO channels |

The generic `evaluate()` walker fires one diagnostic per (LHS, matching-RHS-token) pair; no per-conflict `impl Rule` is needed. **Recommended primitive extension** (the user's verdict on family-predicate framing for Move 3 in §3.10.3): extend `Constraint::Conflicts` with an `RhsFamily(family_predicate: fn(&TokenRef) -> bool)` variant alongside the existing single-token RHS. The two RELIDO entries above are the canonical first consumer.

**Phase-E engine-PR dependency.** `Constraint::RhsFamily(predicate)` is a new surface on `marque-scheme::constraint`. Per Constitution Principle IV (rule architecture preserves stateless declarativity) and Principle VII (acyclic deps; `marque-scheme` stays leaf), the variant addition is **engine-side**, not scheme-side: a separate engine-PR adds the variant + walker, lands against the corpus regression harness, then any scheme can consume it. Until that engine-PR lands, the fallback is the **enumerated form** below, which works without primitive extensions but is verbose.

**Enumerated fallback (single-token RHS).** If `RhsFamily` doesn't land before PR 3b, enumerate one `Conflicts` entry per RHS token:

| LHS | RHS | Scope | Citation |
|-----|-----|-------|----------|
| `RELIDO` | `NOFORN` | Portion + Page | §H.8 p154 |
| `RELIDO` | `LES-NF` | Portion + Page | §H.9 p185, §H.8 p154 |
| `RELIDO` | `SBU-NF` | Portion + Page | §H.9 p178, §H.8 p154 |
| `RELIDO` | `DISPLAY ONLY` | Portion + Page | §H.8 p146, §H.8 p154 |
| `RELIDO` | one entry per FGI atom + `FGI[list]` family | Portion + Page | §H.7, §H.8 p154 |
| `RELIDO` | one entry per JOINT atom + `JOINT[list]` family | Portion + Page | §H.3 p56, §H.8 p154 |
| `RELIDO` | one entry per NATO atom (CTS, NS, NC, NR, NU, ATOMAL, BOHEMIA, BALK, ORCON-NATO) | Portion + Page | §H.7 Appendix B, §H.8 p154 |

The enumerated form is ~15–20 entries, all citing CAPCO-2016 §H.8 p154 (with secondary citations per family). Mechanical, no new primitives, but inflates the catalog volume and requires every future-added FGI/JOINT/NATO atom to add its own conflict row.

**Open question.** Q-3.4.2 (carries forward to §9): commit to the family-predicate path now (and depend on the engine-PR), or land the enumerated form in PR 3b and migrate later? The user's verdict on the structural map: **family-predicate is the recommended path**; PR 3b should call out the engine-PR dependency in its acceptance criteria.

#### 3.4.3 Cross-axis FGI rollup rewrite (one declarative entry)

The "US-presence forces FGI-attribution rollup" rewrite drafted in §4.8.4 belongs in this section as a seventh declarative entry (counted separately from the §3.4.1 transmutation roster because it operates on per-axis post-join state, not on per-portion fact sets):

```text
reads:    [Class, FgiAttribution]
writes:   [FgiAttribution]
guard:    page.class.has_us_classification           — any portion's class is a US-side fact
apply:    if page.fgi_attribution = bare(_, C, _):  — promote to rolled-up form
              page.fgi_attribution := ⊤(C)
```

Citation: §H.7 prose ("FGI [LIST] in banner unless concealed required; mixing concealed + acknowledged → FGI without LIST"); user's map ("you only ever see [bare] in a banner if the entire document is [that bare form]"). The scheduler runs this *after* §3.4.1 entries 1–3 have completed (they may already promote `bare` to `⊤`); the rewrite is idempotent on already-promoted state.

#### 3.4.4 Indestructibility — what the absorbing-element framing covers

The user's revised structural map ("Always Included" section) inventories the **canonical absorbing-element catalog** — markings that are top-of-axis on their relevant lattice and therefore survive any join with weaker peers. Indestructibility is a *per-marking* property (top of the marking's specific lattice), **not** a per-row property of any "domination" table; markings on the LHS of `{...} > FOUO` are not all indestructible, only those whose own per-axis lattice has them at top. The §4.7 closure operator preserves indestructibility automatically (closure adds facts; the relevant axis-joins are absorbing at the indestructible marking; no special case is needed in `Cl_supp`).

**Canonical absorbing catalog (per user's "Always Included" list).**

| Marking | Per-axis location of absorber | Why absorbing | Citation |
|---|---|---|---|
| `TS` (and reciprocal-raised `CTS`) | Class axis (`OrdMax`) — top of TS > CTS > S > NS > C > NC > R > NR > U > NU chain | Chain top by definition | §H.1 p20 |
| All SCI controls / compartments / sub-compartments | SCI axis (`SciSet` join — `BTreeMap` component-wise union) | Union semantics: every contributed compartment survives; cardinal-extensible carrier (no top, every member is preserved) | §H.4, §A.6 |
| All SAR programs / compartments / sub-compartments | SAR axis (`SarSet` join — same shape as SciSet) | Union semantics; agency-extensible carrier | §H.5 |
| `RD` (and `-SIGMA #` / `-CNWDI` extensions) | AEA axis (`SupersessionSet` chain RD > FRD > TFNI) | Chain top; evicts FRD and TFNI on contact | §H.6 p104, p113 |
| NATO programs: `ATOMAL` (AEA), `BOHEMIA` / `BALK` (SCI control) | AEA / SCI axes (per-axis union, family-membership absorbing) | NATO program identity is preserved through any join with US-only content; transmutes to canonical form on US contact | §H.7 Appendix B, §H.4 |
| `ORCON` (US) | Dissem axis (`SupersessionSet` chain ORCON > ORCON-USGOV > ORCON-NATO) | Chain top; evicts ORCON-USGOV and ORCON-NATO on contact | §H.8 p136, §H.7 |
| `RSEN` | Dissem axis (always rolls up) | Always conveys (TS//TK//RSEN admonition surface) | §H.8 p149 |
| `IMCON` | Dissem axis (always rolls up) | Always conveys (SAT warning surface) | §H.8 p144 |
| `PROPIN` | Dissem axis (always rolls up) | Always conveys (originator-control on proprietary content) | §H.8 p148 |
| `NOFORN` | FD&R supersession axis (`SupersessionSet` chain NOFORN / {LES,SBU}-NF > DISPLAY ONLY > REL TO / REL / EYES) | Chain top; clears REL TO via §1.8 declarative `PageRewrite` | §H.8 p145 |
| `DSEN` | Dissem axis (always rolls up) | Always conveys regardless of class | §H.8 p159 |
| `RAWFISA` (when present alone or with FISA) | Dissem axis (always rolls up) | Always conveys; if both `FISA` and `RAWFISA` are present, see §3.7 NNPI sidebar (similar bounded-confidence framing) | §H.8 p162 (FISA) — RAWFISA itself unmapped in CAPCO-2016, treated by analogy |
| `FISA` | Dissem axis (always rolls up) | Always conveys | §H.8 p161 |
| `NODIS` | Non-IC dissem axis (`SupersessionSet` chain NODIS > EXDIS) | Chain top; supersedes EXDIS via §H.9 p174 | §H.9 p173 |
| `LES` | Non-IC dissem axis (always rolls up; transmutes shape on classification) | Always conveys; on classified contact transmutes to `[class]//NOFORN//LES` per §3.4.1 entry 6 | §H.9 p184 |

**Markings on the LHS of `{...} > FOUO` that are NOT uniformly indestructible.**

The user's correction: not every marking that dominates FOUO is indestructible. Several are *conditionally evict-able* — they roll up unconditionally only at `class = U`; on classified content they either drop from the banner or transmute shape.

| Marking | Why NOT indestructible | What evicts / transmutes it | Citation |
|---|---|---|---|
| `LIMDIS` | U-only banner marking; on classified content the marking does not appear in the banner | class promotion above U | §H.9 p182 |
| `SBU` | U-only marking; SBU "abhors classification" per user's structural map (drops from banner on class > U) | class promotion above U | §H.9 p178 |
| `SBU-NF` | post-transmutation: drops from non-IC axis, contributes NOFORN to dissem axis | §3.4.1 entry 6 transmutation on IC contact | §H.9 p178 |
| `LES-NF` | post-transmutation: transmutes to `NOFORN//{LES, SBU}` shape | §3.4.1 entry 6 transmutation on IC contact | §H.9 p185 |
| `EXDIS` | dominated by `NODIS` in the supersession chain | `NODIS` supersession (§H.9 p174) | §H.9 p173 |
| `DOD UCNI`, `DOE UCNI` | U-only in banner; class promotion drops them entirely from banner | class promotion above U | §H.10 p197, p202 |

**Markings without inherent class identity (per user's "No Classification 'Identity'" list).**

These markings have no inherent classification tag of their own — they apply to either unclassified or classified content depending on what they're paired with. They are *axis-orthogonal* to the class axis: their per-axis behavior (always-conveys / FD&R / etc.) is independent of whatever class level the content has.

| Marking | Behavior |
|---|---|
| `FGI` (without classification list) | Concealed-attribution form on FGI axis; survives consensus or rolls to `FGI [list]` per §4.8 |
| `PROPIN` / `PR` | Dissem axis (always-rolls-up; absorbing per above) |
| `NOFORN` / `NF` | FD&R axis (top, absorbing per above) |
| `DISPLAY ONLY` | FD&R axis (mid-chain) |
| `REL TO [list]` | FD&R axis (bottom of chain), with country list lattice as `IntersectSet<Trigraph>` |
| `RAWFISA` | Dissem axis (always-rolls-up) |
| `FISA` | Dissem axis (always-rolls-up) |

**Always-NOFORN-AND-ORCON catalog (compound implicit closure).** Two markings imply *both* NOFORN and ORCON — they're handled by the §4.7 closure operator, not by the absorbing-element framing, but listed here for completeness:

| Marking | Implies | Citation |
|---|---|---|
| `HCS-O` (any) | `{NOFORN, ORCON}` | §H.4 |
| `HCS-P [sub]` (subcompartment present) | `{NOFORN, ORCON}` | §H.4 |

**Always-NOFORN catalog (implicit closure, single-axis).**

| Marking | Implies | Citation |
|---|---|---|
| `NOFORN` | `{}` (already at top of FD&R axis) | §H.8 p145 |
| `HCS-O` | `{NOFORN}` | §H.4 |
| `HCS-P [sub]` | `{NOFORN}` | §H.4 |
| `TK-BLFH` | `{NOFORN}` (also classification floor `TS`; see §3.4.6) | §H.4 |
| `TK-KAND` | `{NOFORN}` | §H.4 |
| `TK-IDIT` | `{NOFORN}` | §H.4 |
| `NODIS` | `{NOFORN}` | §H.9 p173 |
| `EXDIS` | `{NOFORN}` | §H.9 p173 |
| `RAWFISA` | `{NOFORN}` | (analogy to FISA; rule unconfirmed — see §3.7 NNPI/RAWFISA bounded-confidence sidebar) |
| `LES-NF` | `{NOFORN}` | §H.9 p185 |
| `SBU-NF` | `{NOFORN}` | §H.9 p178 |

These all wire up via the §4.7 closure operator — see §4.7.5 for the worked example covering the implicit-default trio (NOFORN-if-no-FD&R, RELIDO-if-no-FD&R-and-not-incompat, REL-USA-NATO-if-no-FD&R-and-NATO).

**Sidebar — accompanying requirements (out of scope for the lattice).** Several indestructible markings carry *document-level admonition / warning notice* obligations (e.g., RD warning, RAWFISA notice, IMCON SAT warning). Per §3.0.b "structure rules vs other-purpose rules", these belong in a separate emit channel (admonition emitter), not in the lattice or constraint catalog. The user's structural map lists this as "Requires admonition (out of scope for now)"; the consultant flags but does not bin them.

**Implication for §4.7 closure operator.** Indestructibility is a per-marking property (top-of-axis under the marking's relevant lattice), not a per-row property. The closure operator preserves it because closure adds facts and the relevant axis-joins are absorbing at the indestructible marking; no special case is needed in `Cl_supp`.

**Implication for the §3.9 collapse projection.** Add the §3.4.1 (six transmutation entries) + §3.4.2 (two family-predicate RELIDO conflicts, or ~15–20 enumerated entries — see §3.4.2) + §3.4.3 (one cross-axis rewrite) to the declarative-entry counts. They are not new rules; they are declarative entries that *replace* what would otherwise be ~13 hand-written `Rule` impls.

#### 3.4.5 RELOPT — REL/EYES portion abbreviation: round-trip + auto-collapse fixpoint

The user's "REVERSE LIST" entry covers the optional portion abbreviation where `REL TO [list]` becomes `REL` (and `[list] EYES ONLY` becomes `EYES`) when the same `[list]` appears in the page banner and every REL/EYES portion on the page agrees. This is structurally interesting because it crosses the form/shape boundary established in §3.0.a:

> **The crux.** Per §3.0.a, sort order and abbreviation are renderer concerns and don't belong in the lattice. But RELOPT abbreviation has a *parser-side* obligation: when the parser sees `(U//REL)` in a portion, it must reconstruct the full country list from the page banner *before* the lattice sees a fact set. Without that reconstruction, the FD&R axis aggregation produces a fact set that disagrees with what the user intended.

The resolution: split RELOPT into **two distinct concerns**, each with a different home.

**(a) Round-trip obligation (parser, mandatory).** The parser MUST expand `REL` / `EYES` portion abbreviations to the full `REL TO [list]` / `[list] EYES ONLY` against the page's `PageContext` before producing the canonical fact set. Three preconditions for valid input (per user's structural map):

1. The banner includes a `REL TO [LIST]` or `EYES ONLY [LIST]`.
2. Every REL/EYES portion on the page has the *same* list as the banner.
3. No portion reduces or changes the list's membership (implied by 1 and 2 but stated explicitly to make the constraint fixed during round-trip).

If preconditions hold, parser expansion is total and lossless: `(U//REL)` → `(U//REL TO USA, FVEY)` (or whatever the banner's list says). If they fail, the parser surfaces a diagnostic (the bare `REL` is ambiguous given the page state) and treats the portion as having no FD&R fact, letting normal FD&R axis aggregation produce the page-level result.

**FVEY-implicit historical case.** A specific historical edge case the user calls out: pages whose portions and banner are all `EYES` *with no expanded list* should be interpreted as `USA/AUS/CAN/GBR/NZL EYES ONLY` (the FVEY group). The migration `EYES → REL TO [list]` per the user's deprecation/migration table folds this into `REL TO USA, FVEY`. The decoder/corrections channel handles the historical-form recognition; the lattice never sees `EYES` directly.

**(b) Auto-collapse style rule (renderer, optional, EXPERIMENTAL).** The opposite direction — automatically rewriting `REL TO [list]` portions to `REL` when the banner already lists `[list]` and every REL portion agrees — is a renderer style choice, not a parser obligation. Per §3.0.a, this is Phase C, not Phase A or Phase B. **But** the user notes a non-trivial subtlety:

> "In some cases this 'reduce and roll up' action may cause a new portion (or more) to flow into the page context (you're removing potentially a lot of characters). Marque would need to re-evaluate the expression to see if either the portion needs to be aligned with the `REL` structure. The new additions may also require the change to revert completely. In that case, we would need a guard to avoid infinite loops."

**Why this is a fixpoint problem.** The auto-collapse changes character counts. Character counts can shift portion boundaries (e.g., a paragraph that was on page 2 reflows to page 1). Page-context membership can change. A formerly-uniform `REL TO USA, FVEY` portion may now sit on the same page as a `REL TO USA, FVEY, NATO` portion, breaking precondition 2 — and the auto-collapse must revert. The composition of these effects can cycle.

The standard lattice-theoretic apparatus does not directly apply: this rewrite is **not monotone** in any obvious sense (collapsing adds character-count headroom, expanding consumes it; each can trigger the other). Per the user's design constraint and the consultant's reading of `abstract-interp.md` §7 (widening), three viable termination strategies:

| Strategy | How it works | When to prefer |
|---|---|---|
| **Step bound** | Run auto-collapse N times (typical N = 3–5); if not stable, leave the page in its last consistent state and emit a diagnostic | Simplest; always terminates; user-visible if the bound is hit |
| **Width-monotone projection** | Project the page state onto a derived lattice (e.g., "which portions are abbreviated") and verify monotonicity in *that* projection; abort if non-monotone | Theoretically clean; per `abstract-interp.md` §7, this is widening; complex to implement |
| **Off-by-default + diagnostic-only** | Don't auto-rewrite; emit a diagnostic suggesting the abbreviation is possible; user accepts manually | Safest for a v1 ship; the user's "EXPERIMENTAL" flag matches this posture |

The user's framing — "let's do it but mark it EXPERIMENTAL" — reads as preference for **off-by-default + step-bound** in v1, with the option to upgrade to width-monotone projection in a later phase.

**Where this lives.** Round-trip (a) is parser responsibility — `marque-core::parser` calls a `PageContext::expand_rel_to_short_form` helper. Auto-collapse (b) is renderer responsibility (Phase C), gated behind a feature flag and a step bound. Neither is a runtime `Rule`; both are configuration-and-correctness obligations on the parser/renderer. **Counts contributed to PR 3b: zero rules** (one parser correctness check + one renderer style option).

**Open questions** (carry forward to §9):
- Q-3.4.5a: Auto-collapse step bound. What's the safe N? (3 likely sufficient; needs corpus testing.)
- Q-3.4.5b: Width-monotone projection — is the "which portions are abbreviated" projection actually monotone? (Counterexample search would resolve; the consultant's read is "probably not, because of the page-reflow coupling".)
- Q-3.4.5c: Should `EYES`-alone-without-list be a parser auto-correction (treat as `REL TO USA, FVEY`) or a diagnostic? (User's note suggests auto-correction; severity-overridable per §3.0.b.)

#### 3.4.6 Per-token classification floors as `Constraint::Requires` invariants

The user's "Always TS" / "Always S OR TS" / "Always classified" / "Unknown but minimally classified" tables encode **per-marking classification floors**: presence of marking M requires the page's class level to be at least F(M). This is *not* part of the lattice axis itself (the class chain is `OrdMax(TS > CTS > S > NS > C > NC > R > NR > U > NU)`); it's a *constraint* over the joint fact-set: the page is malformed if M is present and class level is below F(M).

Per §3.0.b "structure rules vs other-purpose rules" / §3.3 ("Rules that enforce a `Constraint::Requires` invariant"), these belong in **Phase B** as `Constraint::Requires(M, ClassLevel ⊒ F(M))` entries. The generic `Constraint::Requires` walker fires diagnostics; the engine separately tries to repair by raising the class via `FixIntent` (post-PR 3c).

**Floor catalog.** Each entry is a single `Constraint::Requires` row in the scheme catalog.

| Marking M | Floor F(M) | Citation | Notes |
|---|---|---|---|
| `HCS-[comp][sub]` (HCS with full compartment + subcompartment) | `TS` | §H.4 | All HCS subcompartments are TS-only |
| `SI-[comp]` (SI with compartment) | `TS` | §H.4 | SI compartments are TS-only |
| `TK-BLFH` | `TS` | §H.4 | Specific TK subcompartment, TS-only |
| `BALK` (NATO/CTS) | `TS` (CTS = reciprocal-raised TS per §3.4.1 Note i) | §H.7 Appendix B | NATO program; class is reciprocal-raised |
| `BOHEMIA` (NATO/CTS) | `TS` | §H.7 Appendix B | Same as BALK |
| `HCS-[comp]` (HCS with compartment, no subcompartment) | `S` | §H.4 | Compartment-level minimum |
| `RSV-[comp]` | `S` | §H.4 | Compartment required (compartment absence = `Constraint::Requires`, not floor) |
| `TK` (bare) | `S` | §H.4 | TK without compartment minimum |
| `RD-SG` (RD-SIGMA) | `S` | §H.6 p113 | SIGMA-extended RD elevates floor |
| `FRD-SG` | `S` | §H.6 p113 | Same as RD-SG |
| `RD-CNWDI` | `S` | §H.6 p104 | CNWDI-extended RD |
| `RSEN` | `S` | §H.8 p149 | Always-rolls-up + class floor |
| `IMCON` | `S` | §H.8 p144 | Always-rolls-up + class floor |
| `SI` (bare control) | `C` | §H.4 | Bare SI minimum |
| `SAP` (any program) | `C` | §H.5 | All SAR programs require classification |
| `RD` (bare) | `C` | §H.6 p104 | "RD ⇒ classified" |
| `FRD` (bare) | `C` | §H.6 p104 | Same as RD |
| `TFNI` | `C` | §H.6 p107 | AEA chain bottom but still classified |
| `ATOMAL` (NATO) | `C` | §H.7 Appendix B | NATO AEA-equivalent, class follows reciprocal raise |
| `ORCON` (US) | `C` | §H.8 p136 | Originator-control floor |
| `ORCON-USGOV` | `C` | §H.8 p136 | Subordinate ORCON |
| `EYES` / `[LIST] EYES ONLY` | `C` | §H.8 p152 | EYES-style FD&R requires classification |

**Unknown-floor sub-catalog (passthrough — see §3.7).** Per the user's "Unknown but minimally classified" list:

| Marking | Provisional floor | Source of uncertainty |
|---|---|---|
| `BUR`, `BUR-BLG`, `BUR-WRG`, `BUR-DTP` | `C` (minimal) | Known to exist in ISM; specific floor not in CAPCO-2016 |
| `HCS-X` | `C` (minimal) | Known SCI control; specific floor not enumerated |
| `KLM` / `KLAMATH`, `KLM-R` | `C` (minimal) | Known SCI control |
| `MVL` / `MARVEL` | `C` (minimal) | Known SCI control |

For the unknown-floor entries, the `Constraint::Requires` row sets `F(M) = C` (minimal classified) and emits a diagnostic with a citation pointing to "ISM-known, CAPCO-2016 unmapped — see §3.7 passthrough policy".

**Counts contributed to PR 3b.** This catalog is **~25 declarative `Constraint::Requires` entries**, all consumed by the existing generic `Constraint::Requires` walker (Phase B / Move 4 in §3.10.3). It does not add new rules; it adds entries to the catalog the walker already consumes.

**Where this came from in the rule list.** The current `crates/capco/src/rules.rs` and `rules_declarative.rs` carry several rules that test "if marking M is present, class must be ≥ F(M)" as separate `impl Rule` blocks (e.g., `CnwdiRequiresClassificationS`, `SciSubcompartmentRequiresTopSecret`, etc., scattered across the codebase). The phase-overlay framing collapses all of them into the catalog above + the generic walker — they are §3.3-bucket rules (Move 4 leverage).

**Open questions** (carry forward to §9):
- Q-3.4.6a: Catalog generation. Should the floor catalog be generated at build time from CVE/Schematron metadata (the ODNI XML lists per-token class minimums in some fields), or hand-curated in `marque-capco` against CAPCO-2016 §H? The build-time path is preferred per Constitution Principle IV (two-layer rule architecture: generated predicates from CVE), but only if the CVE actually carries this data uniformly. Hand-curated with citation is the fallback.
- Q-3.4.6b: Unknown-floor handling. For `BUR` / `KLM` / `MVL` / `HCS-X`, default floor = `C` (minimal classified) is conservative. Should the engine emit a diagnostic flagging "this is a passthrough marking with unknown specific floor; user should verify with current ODNI manual"? (Default per §3.0.b "override/customization is configuration-surface, not lattice": yes, severity-overridable.)

### 3.5 Rules that test grammar / formatting / style

**Pattern.** "This portion uses long-form when banner-form is preferred" / "Banner mixes abbreviated and long forms." Examples: S001 (prefer-banner-abbreviation), S002 (banner-consistent-form), S003 (joint-usa-first), E001 (portion-mark-in-banner).

**Catalog citation.** None — this is text-formatting territory, not lattice algebra. Per `universal-algebra.md` §11 "almost-lattice diagnostic" Axis A: "is the operation total?" — these aren't even lattice operations; they're *renderer correctness checks* and *style preferences*.

**Recommendation.** These DO NOT collapse to lattice/constraint primitives. They remain as `Rule` impls (or move to `Renderer` checks if a renderer-trait surface is built). The May 1 lattice-design plan and the May 2 consolidated engine-refactor plan (the governing docs for PR 3b) acknowledge this implicitly: "the remaining rules are non-constraint rules (banner-abbreviation preference, etc.) and stay as `Rule` impls" — see also recursive-lattice plan §12 Phase C for the original framing.

**Estimated count: 6–8 rules stay as `Rule` impls.**

### 3.6 Rules that test ordering / canonicalization

**Pattern.** "SCI compartments must appear in numeric-then-alpha order" (E033), "SAR programs must be alphabetized" (E028), "REL TO must lead with USA" (E020), "AEA SIGMA compartments must be numerically sorted" (E023 / `SigmaValidationRule`).

**Catalog citation.** `pure-lattice.md` §15 — render order is a *choice of representative* in the lattice equivalence class. Two markings that differ only in token order are lattice-equal; the renderer picks a canonical representative.

**Recommendation.** These collapse to **`MarkingScheme::render_canonical` correctness** — the renderer is responsible for choosing the canonical representative. Once `render_canonical` is correct, the diagnostic becomes "your input does not match the canonical form" which is a *normalization fix*, not a rule. **Estimated count: 4–6 rules collapse to renderer-canonical-form work + a single "non-canonical input" rule (or auto-fix without diagnostic, depending on severity policy).**

### 3.7 Rules that detect a non-lattice violation (open-vocab / agency-extensibility / bounded-confidence)

**Pattern.** "This SCI compartment doesn't match the agency-allocated shape" / "This custom control system uses an unrecognized format." Example: W034 (`SciCustomControlInfoRule` at `rules.rs:5378`).

**Catalog citation.** `frames-locales.md` §9 "Diagnosis: Is this construction a frame?" Step 1 — admission rules that depend on a *generative* shape (not membership in a closed set) live outside the lattice. `universal-algebra.md` §11 Axis A — partiality.

**Recommendation.** These stay as `Rule` impls because the predicate is a *shape check*, not a lattice law. They cannot collapse to `Constraint::Conflicts` or `Requires` because the relation is "this token shape is admissible" — a single-token property — not a between-token relation. **Estimated count: 2–3 rules stay as `Rule` impls.**

#### 3.7.1 Open-vocabulary / passthrough policy (carrier-set extensibility)

The user's structural map adds a stronger framing: the marking-system carrier is **not closed**. ODNI's ISM XML describes tokens that CAPCO-2016 does not enumerate (the user notes: "we know that CAPCO 2016 isn't the latest because ISM has tokens that it says are in the manual that aren't in our version"). FOIA-induced spec lag is the operating reality, and the engine must accommodate it.

**Passthrough catalog (markings known to exist but rules unknown).** Per the user's "Markings We Know Exist from ISM but don't know their rules" list:

| Token | Axis | Known from |
|---|---|---|
| `BUR` | SCI control | ISM CVE |
| `BUR-BLG` | SCI compartment | ISM CVE |
| `BUR-DTP` | SCI compartment | ISM CVE |
| `BUR-WRG` | SCI compartment | ISM CVE |
| `HCS-X` | SCI compartment | ISM CVE |
| `KLM` / `KLAMATH` | SCI control | ISM CVE |
| `KLM-R` | SCI compartment | ISM CVE |
| `MVL` / `MARVEL` | SCI control | ISM CVE |
| `RAWFISA` / `RAW FISA INFORMATION` | Dissem | ISM CVE; behavior inferred from FISA |
| `NNPI` / `Naval Nuclear Propulsion Information` | Non-IC dissem | ISM CVE; specific behavior bounded-confidence (see §3.7.2) |

**The lattice-shape implication.** The carrier set for `SciSet`, `SarSet`, and the dissem axes must be **open-extensible** — `String`-keyed maps that admit any well-formed token, not closed enums. Marque already encodes this via `SystemKey::Custom(String)` (`crates/capco/src/lattice.rs:78`); the §3.7 verdict confirms this design choice.

**The passthrough rule.** When the engine encounters a token in the passthrough catalog (or any well-formed token not in the closed ODNI CVE), it should:

1. **Preserve verbatim** — the token survives the lattice operations. No rewrite, no canonicalization.
2. **Apply the most conservative known constraint** — for ISM-known SCI controls, "all SCI controls are classified" (§3.4.6 unknown-floor sub-catalog: floor = `C`); for ISM-known dissem controls, treat as RELIDO-default-eligible per §4.7.5 implicit-RELIDO list.
3. **Emit a diagnostic** at `info` severity flagging "this marking is in ISM but unmapped in CAPCO-2016; behavior follows passthrough policy and conservative defaults; user should verify with current ODNI manual." Severity-overridable per §3.0.b.

This is **a single rule**, not one per token — the rule is "passthrough-applies-to-unmapped-tokens" and the catalog is data. Counts contributed to PR 3b: **1 rule** (the passthrough handler), or 0 rules if folded into the existing W034 shape-check rule.

#### 3.7.2 NNPI bounded-confidence sidebar

Naval Nuclear Propulsion Information (`NNPI`) is the canonical bounded-confidence case: we know one fact for certain, and have two reasonable hypotheses for the rest. Per §3.0.b, this is *not* a structure rule we should commit to one way; it's a configuration surface the user steers.

**Definitely known** (singleton fact):

> **NNPI evicts FOUO.** The universal FOUO rule per the user's structural map: "any dissemination control, IC or non-IC, evicts FOUO." NNPI is a non-IC dissem control. Therefore `NNPI ⇒ FOUO is dropped from any joint marking with NNPI`. This is a **`Constraint::Conflicts(NNPI, FOUO)` entry** with citation to user's structural map (deduced from §H.10 prose on FOUO + §H.10 NNPI as non-IC dissem). No further commitment.

**Bounded hypotheses** (record both, default to suggesting #1):

| Hypothesis | Behavior | Plausibility | Source of analogy |
|---|---|---|---|
| **#1 Always-conveys (LES-like)** — preferred default | NNPI rolls up regardless of class to preserve Navy ownership / equity information | **Higher** — NNPI protects "foundational operational capability central to an effective nuclear triad" (user's framing); ownership-equity protection mirrors LES exactly | §H.9 p184 (LES) by analogy |
| **#2 Unclassified-only (UCNI-like)** | NNPI conveys in unclassified markings; class promotion drops it | Lower — UCNI's framing ("any classification exceeds protection afforded by UCNI") is about the *technical-data* protection floor; nuclear-triad operational sensitivity exceeds that floor | §H.10 p197 (DOD UCNI) by analogy |

**The consultant's recommendation (per user's verdict on A).** Mirror the RAWFISA approach: do not commit. Record both hypotheses in the catalog. When the engine encounters NNPI, emit a diagnostic suggesting the most likely interpretation is hypothesis #1 (always-conveys), but flag the ambiguity and link to the user-overridable severity / behavior toggle. Citation: "Per ODNI ISM CVE, NNPI is a known non-IC dissem control. Specific behavior unmapped in CAPCO-2016; consult the current IC Markings Register for authoritative guidance."

**Why this matters for the lattice.** Hypothesis #1 (always-conveys) makes NNPI an absorbing element of the non-IC dissem axis (top of a single-element supersession). Hypothesis #2 (unclassified-only) makes NNPI a non-absorbing per-axis element with `Constraint::Conflicts(NNPI, ClassLevel ⊒ C)`. The two hypotheses produce **different lattice shapes**, which is why the consultant should not commit silently — the user's lead is the source of truth until the spec lag closes.

**Counts contributed to PR 3b.** The NNPI bounded-confidence framing produces **1 `Constraint::Conflicts(NNPI, FOUO)` entry** (in the existing walker) plus a passthrough-style diagnostic emitter (folded into §3.7.1's passthrough rule). No new `impl Rule` blocks.

### 3.8 Rules whose collapse target is unclear

**Pattern.** Rules with body shape that doesn't fit any of the above clearly. These are the rules that PR 3b should escalate as **open questions for the user**, not silently assign to `Constraint::Custom` (which would defeat the collapse purpose).

**Examples (candidates — verify each).**
- **E003 (`MisorderedBlocksRule`)**: tests that the `//` separator structure is correct between major categories. This is a *grammatical structure* check, not a lattice law. Likely collapses to a renderer / grammar check (3.5/3.6 hybrid).
- **E007 / E008 (`XShorthandDateRule`, `UnknownTokenRule`)**: parser-error surfacing. Not lattice; these are "the parser failed to recognize this token" diagnostics. Collapse target: the engine-synthetic R001/R002 channel post-PR 3c.
- **C001 (`CorrectionsMapRule`)**: user-configurable typo replacements. Not lattice; this is text-substitution. Collapse target: a separate "corrections" trait surface, NOT a `Rule`.
- **E052 (`RelToNoDuplicatesRule`)**: tests that REL TO has no duplicate trigraphs. After PR 4 with the `IntersectSet` lattice (which dedupes by storage), this is automatic. Collapse target: lattice property test.

**Recommendation.** Each unclear rule deserves an explicit decision in the PR 3b PR description. **Estimated count: 4–6 rules in this bucket; each requires a one-line classification before the count lands.**

### 3.9 Aggregated PR 3b collapse projection

| Bucket | Source rules | Collapse target | Surviving count |
|---|---|---|---|
| 3.1 Lattice-law (banner roll-up) | E031, E035, E040, E045, etc. | Lattice property test | **0** (move to test suite) |
| 3.2 `Conflicts` invariant | E022, E024, E025, E037, E041, … | `Constraint::Conflicts` entry | **8–10** |
| 3.3 `Requires` invariant | E021, … | `Constraint::Requires` entry | **3–5** |
| 3.4 `PageRewrite` | E039 (and the 3 already shipped) | `PageRewrite::declarative` | **3–4** |
| 3.5 Grammar/formatting/style | E001, S001, S002, S003 | Stay as `Rule` | **4–6** |
| 3.6 Ordering/canonicalization | E020, E023, E028, E033 | Renderer-canonical + 1 rule | **1–2** |
| 3.7 Non-lattice shape check | W034 | Stay as `Rule` | **2–3** |
| 3.8 Unclear (open question) | E003, E007, E008, C001, E052 | Decide per-rule | **0–4** |

**Projected total: 21–34 surviving rules** if every bucket lands at the high end. The plan's 8–18 band requires aggressive consolidation in buckets 3.2/3.3 (multiple rules folding into a single constraint with multiple `TokenRef::AnyInCategory(...)` entries) and 3.5 (folding S001+S002+S003 into a single style rule with a parameter).

**The path to the 8–18 band.**
1. Collapse all banner-roll-up rules into the lattice property test (bucket 3.1) — drops the count by ~5.
2. Collapse all SCI-system-specific rules (E042–E051 in `rules_sci_per_system.rs`) into a generic `Constraint::Custom` per-system rule, dispatched by the scheme's `evaluate_custom` against the `SciSet` structure — drops 10 rules to 1 generic rule.
3. Collapse all "conflicts" rules into a single `Constraint::Conflicts` walker if the catalog covers them all (bucket 3.2 → 1 declarative iterator, not 8–10 individual rules). The `evaluate()` function already does this; the surviving "rule" is the catalog entry itself.

After the aggressive consolidation: **lattice property tests (0 rules) + 1 conflicts walker + 1 requires walker + 1 page-rewrite walker + 4–6 style rules + 1 renderer-non-canonical rule + 2–3 shape-check rules ≈ 9–12 surviving rules**, comfortably in the 8–18 band.

**The user-facing bottleneck.** The aggressive consolidation requires committing to "the constraint catalog is the rule source-of-truth, and the surviving `Rule` impls are the residual that doesn't fit declarative form." That commit is a design move, not a code move. The PR 3b reviewer attestation (per plan.md:236–242) requires single CAPCO-§ citation per rule and ≤3 internal branches; the consolidated "conflicts walker" satisfies "single citation" only if every conflicts entry shares a single §-citation, which it doesn't (NOFORN ∦ REL TO is §H.8; HCS-O ⇒ NOFORN is §H.4; SIGMA ordering is §H.6). **Open question Q-3.9 in §9: is "single citation per rule" interpreted as "single citation per declarative entry" (which the per-Conflicts entry satisfies) or "single citation per `impl Rule` block" (which the consolidated walker fails)?**

### 3.10 The Phase A/B/C overlay — re-binning by structural role

The user's structural map (cf. §4.7 / §4.8 / §3.4.1) introduces a higher-level organizing principle: every CAPCO rule today is doing one of three jobs at three different layers of the pipeline. Re-binning by phase clarifies which collapse target is appropriate and gives a tighter count projection than the bucket-only view in §3.9.

**Phase A — structural resolution (per-axis lattices + closure + transmutations).** Rules that compute "what facts are present on the page after parsing, closure, and transmutation." The output is the canonical fact-set; the rules disappear because the engine produces the canonical fact-set by construction (lattice properties + declarative transmutations). The rule survivors here are the *property tests* that exercise the lattice impls; the runtime rule count contribution is **zero**.

**Phase B — constraint checking (predicates over the canonical fact-set).** Rules that are about *acceptance* or *advisory*: "is this combination of facts allowed in a portion?" "is this combination required to carry an admonition?" These are `Constraint::Conflicts`, `Constraint::Requires`, the abhors-company portion-isolation checks, the requires-admonition document-level legal-notice obligations, and the §3.4.2 RELIDO incompatibility roster. They survive as *declarative catalog entries*, not `impl Rule` blocks; a single generic `evaluate()` walker handles them all.

**Phase C — rendering (style, ordering, delimiters).** Rules that are about *form* of the rendered banner / portion: alphabetical sort, hyphen-vs-space-vs-comma separators, banner-form-vs-portion-form, abbreviation preference. These are not constraints on the *fact-set*; they are constraints on the *render output*. They live in the renderer, not the rule engine. The runtime rule count contribution is *one* "non-canonical input" rule (or zero, depending on whether mismatch with canonical render is a diagnostic or a silent normalization).

The phase classification is **orthogonal** to the §3.1–§3.8 bucket classification: a rule is in exactly one phase and exactly one bucket, but multiple buckets feed each phase (e.g., Phase A is fed by §3.1 lattice-law buckets *and* by §3.4 transmutation buckets and §3.6 ordering-canonicalization moves to Phase C).

#### 3.10.1 Bin the 56 rules by phase

| Phase | Source buckets (§3.1–§3.8) | Surviving form | Count |
|---|---|---|---|
| **A — structural resolution** | §3.1 (lattice-law banner roll-up) | property tests in `proptest_lattice.rs`, `proptest_closure.rs`, `proptest_fgi_attribution.rs` | **0 rules** |
| | §3.4 (PageRewrites — incl. §3.4.1 6 transmutation entries + §3.4.3 1 cross-axis rewrite) | declarative `PageRewrite` rows | **0 rules** (7 declarative entries) |
| **B — constraint checking** | §3.2 (`Conflicts` invariants) + §3.4.2 (2 family-predicate RELIDO conflicts, or ~15–20 enumerated entries) + §3.7.2 NNPI-FOUO conflict | declarative `Constraint::Conflicts` rows + 1 generic walker | **1 rule** (the walker) + ~10–22 declarative entries |
| | §3.3 (`Requires` invariants — incl. closure-implied requirements not auto-derived) + §3.4.6 (~25 per-token class-floor entries) | declarative `Constraint::Requires` rows + 1 generic walker | **1 rule** (the walker) + ~29–31 declarative entries |
| | abhors-company portion-isolation checks (`marque-applied.md` §3.4.2 / user's structural map) | 1 generic per-portion walker over the abhors-company set | **1 rule** + ~12 declarative entries |
| | requires-admonition (document-scope legal notices — deferred per §3.0.b "accompanying requirement", separate emit channel) | tracked but not yet emitted; eventual document-scope channel | **0 rules now**; +1 deferred |
| **C — rendering** | §3.5 (grammar/formatting/style) + §3.4.5 (b) auto-collapse (renderer style, off-by-default) | renderer + 1 "non-canonical form" rule (or auto-fix without diagnostic) | **0–1 rule** |
| | §3.6 (ordering/canonicalization) | renderer + (subsumed under §3.5 "non-canonical" rule) | **0 rules** |
| **Parser correctness** | §3.4.5 (a) RELOPT round-trip (parser MUST expand REL/EYES from PageContext) | parser correctness check (not a runtime `Rule`) | **0 rules** |
| **Stays as `Rule` impls** | §3.7.1 (passthrough policy for ISM-known, CAPCO-unmapped tokens) + §3.7 (non-lattice shape checks) | `impl Rule` (passthrough handler + generative-shape predicates) | **1–3 rules** |
| | §3.8 (unclear, decide per-rule) | TBD per-rule | **0–4 rules** |

**Projected total surviving runtime rules: 5–11 rules** in the Phase A/B/C frame:

- Phase A: 0 rules (structural facts produced by construction; ~7 declarative `PageRewrite` rows + closure operator)
- Phase B: 3 rules (Conflicts walker, Requires walker — now ~29–31 entries with §3.4.6 floors, abhors-company walker)
- Phase C: 0–1 rule (non-canonical-form rule, or none if auto-fix)
- Parser correctness: 0 rules (RELOPT round-trip is a parser obligation, not a `Rule`)
- §3.7 / §3.8: 1–7 rules (passthrough handler + residual shape-checks + unclear)

**This is squarely inside the 8–18 acceptance-criteria band.** The phase reframing is what clears the 8–18 budget honestly: §3.1 + §3.4 (Phase A) drops ~12 rules to 0; the abhors-company walker captures ~12 portion-level checks as declarative entries with one walker rule; §3.4.6 collapses ~10–15 separate "if M then class ⊒ F(M)" rules into the existing Requires walker; §3.6 (Phase C) drops ~4 ordering rules to renderer responsibility; §3.7.1 collapses passthrough to a single rule.

#### 3.10.2 Where this is more conservative than §3.9

§3.9's projection (21–34 surviving) included rules that the phase reframing eliminates entirely:

- Banner-roll-up rules across SCI/SAR/dissem/FGI/REL TO/declassify-on/class (§3.1) → **§3.9 said "0 surviving"; phase view confirms "0 (move to property tests)"** — same outcome.
- Conflicts rules + RELIDO conflicts (§3.2 + §3.4.2) → **§3.9 said "8–10 entries"; phase view groups them under one walker rule with 14–16 declarative entries** — count by `impl Rule` blocks: 1 rule (walker), not 8–10.
- Ordering / canonicalization (§3.6) → **§3.9 said "1–2"; phase view says "0"** — moving to renderer eliminates them as runtime rules entirely.

The phase reframing is more aggressive in two specific ways:

1. **Generic walkers count as one rule.** §3.9 hedged on whether `Conflicts` should be 8–10 entries or 1 walker; the phase view commits to "1 walker, declarative entries are catalog data, not rules." This depends on Q-3.9 (single-citation interpretation) — see §9.
2. **Phase C is not a rule budget.** §3.9 left the renderer-canonical idea half-articulated; the phase view commits to "Phase C is renderer responsibility, not rule responsibility." Style / ordering / delimiter checks live in the renderer's correctness contract, exercised by render-tests, not runtime rules.

#### 3.10.3 The path from 59 to ~10 across PRs 3b / 3.7 / 4 / 5+

> **Re-sequenced 2026-05-07** per
> `docs/plans/2026-05-07-pr3b-consultation-verdict.md`. The original
> framing of this section scheduled Moves 1–7 inside PR 3b;
> two of those moves (Move 3 family-predicate variant; Move 6 closure
> operator wiring) require new primitives that don't ship at PR 3b
> time. The corrected sequencing distributes the moves across four
> stages — see §3.11 for the per-stage table.
>
> **Re-baselined source count:** **59** (`grep -c '^impl Rule for'
> rules.rs rules_declarative.rs rules_sci_per_system.rs`), not the
> "~56" approximation prior versions of this section carried.

Ordered by leverage. Each move is independently committable; counts are cumulative. Per-move PR-home tag indicates the stage where the move lands.

1. **Move 1 (PR 3b — T026a + retires fully in PR 4).** Convert the §3.1 banner-roll-up rules to ONE generic walker calling `MarkingScheme::project(Scope::Page, ...)`; the walker retires when PR 4's per-category Lattice impls + property tests in `proptest_lattice.rs` land. Drops E031, E034, E035, E040, E045, plus the FGI within-axis rollup rules. **Δ: −5 to −7 individual rules → +1 walker. Net −4 to −6; running total: 59 → 53 to 55** (PR 3b); finalizes to **−1 walker rule** in PR 4.
2. **Move 2 (PR 3b — T026b).** Wire the §3.4.1 transmutation roster as 6 declarative `PageRewrite` rows + the §3.4.3 cross-axis FGI rollup as a 7th. Retire the corresponding hand-written rules. **Δ: −3 to −5 rules; running total: 53 to 55 → 48 to 52.**
3. **Move 3 (PR 3b — T026c enumerated form; PR 4 — compaction to family rows).** PR 3b lands §3.2 conflicts + §3.4.2 RELIDO conflicts as ~15–20 enumerated `Constraint::Conflicts` rows with single-token RHS (existing variant). Single generic walker. PR 4 compacts to 2 family rows once T108b's `RhsFamily(predicate)` variant ships in PR 3.7. Q-3.9 resolved per `2026-05-07-pr3b-consultation-verdict.md`: per declarative entry. **Δ: −7 to −9 rules; running total: 48 to 52 → 39 to 45.**
4. **Move 4 (PR 3b — T026d; PR 4 — closure re-classification).** PR 3b lands §3.4.6 per-token class floors as ~25 `Constraint::Requires` rows (single generic walker). Closure-implied requirements stay as `Requires` rows in PR 3b; the §4.7 closure operator (T108c, lands in PR 3.7) re-classifies the implication-shaped entries from `Requires` to closure entries in PR 4. Residual `Requires` covers structural validity like "RSV requires a compartment" and "JOINT [list] requires REL TO [matching list]". **Δ: −10 to −15 rules; running total: 39 to 45 → 24 to 35.**
5. **Move 5 (consolidation — split per `2026-05-07` correction).** The "abhors-company" framing splits into (a) RELIDO family conflicts (already counted in Move 3) and (b) class-conditional drops (SBU, LIMDIS, DOD UCNI, DOE UCNI eviction at class > U). The latter folds into Move 2's PageRewrite roster as additional entries (or into a single parameterized "u-only-drop" rewrite). **No separate "abhors walker" needed.** Δ counted under Moves 2 and 3.
6. **Move 6 (PR 3.7 — T108c primitive; PR 4 — wiring).** Land the §4.7 closure operator primitive in PR 3.7. Wire CAPCO's `ImplTable` (the implicit-default trio + per-marking unconditional implications) in PR 4 alongside per-category Lattice impls. Retire the rules that today encode "if X then Y must also be present" (always-NOFORN, HCS-O ⇒ {NOFORN, ORCON}, TK-BLFH ⇒ NOFORN, etc.). The closure operator handles automatic derivation, leaving §3.3 to handle structural validity only. **Δ: −5 to −8 catalog entries (compaction); rule count unchanged; running total: 24 to 35** (the rule count was already collapsed in Move 4; closure compacts the catalog).
7. **Move 7 (PR 3b — T026f single-walker fallback; PR 5+ — renderer takeover).** PR 3b retains ONE "non-canonical input" walker covering E020 / E023 / E028 / E033 ordering checks; PR 5+ lifts these into renderer correctness when the renderer trait surface lands, retiring the walker. The §3.4.5 (a) RELOPT round-trip becomes a parser correctness obligation (not a runtime `Rule`); §3.4.5 (b) auto-collapse stays off-by-default with step-bound termination per Q-3.4.5a. **Δ: −4 individual rules → +1 walker (PR 3b); −1 walker (PR 5+). Net −4 cumulative.**

**End-state cumulative target.** Stage 1 (PR 3b proper): 13–18 surviving rules. Stage 4 (post-renderer takeover, post-closure wiring, post-RELIDO compaction): **9–11 surviving rules**. The 8–18 D13 band remains the end-state acceptance gate; 13–18 is the PR 3b proper acceptance gate per `plan.md` D13 addendum.

**The wide cumulative ranges reflect the catalog-compaction effect.** Moves 3 / 6 / 7 reduce *catalog entries* (data, not rules) more than they reduce *rule count* once the walkers are in place. Resolving Q-3.4.6a at the build-time end (catalog auto-generated from CVE) does not change the rule count but reduces the maintenance burden. Resolving Q-FgiSet-vs-§4.8 at the no-new-primitive end (existing `FgiSet` already models §4.8) saves one primitive landing in PR 3.7 without changing the count.

#### 3.10.4 Which moves matter most for the PR 3b acceptance criteria

The acceptance criteria (D13: post-collapse rule count in 8–18, single CAPCO-§ citation per rule, ≤3 internal branches per predicate body):

- **Rule count (8–18).** Moves 1–7 land in the band. Moves 1, 2, 3, 5, 7 carry the most leverage (cumulatively ~25–33 rules retired); Moves 4, 6 are smaller but unlock the closure-operator framework.
- **Single CAPCO-§ citation per rule.** Moves 1, 2, 4 satisfy this trivially (each declarative entry cites one §). Move 3 hinges on Q-3.9 (does "rule" mean walker or per-entry?). Move 5 has multiple §-citations across the abhors-company set; expressed as a per-row walker citing per-row, it satisfies if "rule = entry"; otherwise the walker citation is "§H per the abhors-company roster" which is weak.
- **≤3 internal branches per predicate body.** Moves 1–7 all produce predicate bodies that are simple presence-checks; the branching budget is for combining axes (e.g., FGI-attribution rewrite branches on class-axis presence + FGI-attribution shape, 2 branches). The walker bodies have one branch per declarative-entry kind (Conflicts vs. Requires vs. AbhorsCompany), which is ≤3 if the catalog stays homogeneous.

**Recommendation.** Land Moves 1, 2, 7 first (independent, low-risk, retire the most rules). Land Move 3 only after Q-3.9 is resolved with the user. Land Moves 4, 5, 6 in any order after Move 3.

> **Q-3.9 resolved 2026-05-07** (per declarative entry; see
> `docs/plans/2026-05-07-pr3b-consultation-verdict.md`). The
> ordering recommendation above is preserved as historical
> guidance; the canonical sequencing is in §3.11.

### 3.11 Stage sequencing (locked 2026-05-07)

The canonical mapping of §3.10.3 Moves 1–7 to PR-time-availability of primitives. This section is the source of truth that `plan.md` D13 addendum + `tasks.md` T026a–T026f and T108b–T108d implement.

**Why a separate section.** §3.10.3's prior framing scheduled all Moves inside PR 3b. Two of those moves (Move 3 family-predicate variant; Move 6 closure operator wiring) require new primitives that don't ship at PR 3b time — the bridge was honest about this in prose but the move list did not surface the dependency. §3.11 makes the staging explicit.

**Stage 1 — PR 3b proper (declarative-catalog moves over existing primitives).**

| Sub-move | T-task | Primitive(s) used | Source rules retired |
|---|---|---|---|
| 3b.A — banner walker | T026a | `MarkingScheme::project(Scope::Page, ...)` (Phase B) | E031, E034, E035, E040, E045 (5 rules) → 1 walker |
| 3b.B — transmutation `PageRewrite` roster | T026b | `PageRewrite` + topological scheduler (Phase B + #69) | 3–5 hand-written rules → 7 declarative rows |
| 3b.C — RELIDO `Constraint::Conflicts` enumerated | T026c | `Constraint::Conflicts` walker (Phase 4+) | 7–9 hand-written conflict rules → ~15–20 catalog rows + walker |
| 3b.D — class-floor `Constraint::Requires` catalog | T026d | `Constraint::Requires` walker (Phase 4+) | 10–15 hand-written floor rules → ~25 catalog rows + walker |
| 3b.E — SCI per-system collapse | T026e | `Constraint::Custom` walker | E042–E051 (10 rules) → 1 walker |
| 3b.F — non-canonical input walker (fallback) | T026f | New `impl Rule` block | E020, E023, E028, E033 (4 rules) → 1 walker |

**Stage-1 acceptance**: 13–18 surviving rules (consultation verdict; `plan.md` D13 addendum).

**Stage 2 — PR 3.7 (new primitives + lattice-design.md fill-in).**

| Deliverable | T-task | What lands |
|---|---|---|
| `Constraint::Conflicts::RhsFamily(predicate)` variant | T108b | Surface change on `marque-scheme::constraint`; walker dispatch extension; `proptest_constraint_rhs_family.rs` |
| §4.7 closure operator primitive | T108c | `MarkingScheme::closure(...)` trait method (default no-op); `ImplTable<S>` shape; CAPCO `ImplTable` hand-curated with §-citations; `proptest_closure.rs` (monotone + extensive + idempotent + suppression-doesn't-break-monotonicity) |
| §4.8 FGI/JOINT consensus verification | T108d | Verify §4.8.5 worked example against existing `FgiSet`; if mismatch, land `FgiAttributionLattice` per shape (β); doc-comment update |
| Lattice-design.md §§2–8 fill-in | T104–T108a | Per-category §-citations, formal join semantics, worked examples, property-test fixtures (PR 3.7's existing scope) |

**Stage-2 acceptance**: rule count unchanged (13–18); catalog compaction in flight; new primitives land; lattice-design.md gate cleared.

**Stage 3 — PR 4 (lattice impls + closure wiring + RELIDO compaction).**

| Deliverable | T-task | What lands |
|---|---|---|
| Per-category `Lattice` impls | T112 | Per-category formal `Lattice` impls satisfying assoc/comm/idem/identity-with-bottom |
| RELIDO Conflicts compaction | (alongside T112) | T026c's enumerated rows compact to 2 family rows using T108b's `RhsFamily(predicate)` variant |
| Closure operator wiring | (alongside T112) | T108c's primitive wired into `Engine::project` per §4.7.4 pipeline; T026d's implication-shaped entries flip from `Requires` to closure entries |
| Banner walker retirement | (alongside T116) | T026a walker becomes property-test-only sanity check (or deletes outright if PR 4 reviewer accepts property-test coverage) |

**Stage-3 acceptance**: 10–15 surviving rules.

**Stage 4 — PR 5+ (renderer + RELOPT round-trip).**

| Deliverable | What lands |
|---|---|
| Renderer trait surface | Phase C renderer absorbs E020 / E023 / E028 / E033 via canonical-form rendering; T026f walker retires |
| RELOPT round-trip in parser | Parser obligation per §3.4.5 (a); not a runtime `Rule` |
| RELOPT auto-collapse (off-by-default) | Optional renderer style with step-bound termination per Q-3.4.5a default `N=3` |

**Stage-4 acceptance**: 9–11 surviving rules. End-state 8–18 D13 band cleared.

**Cumulative table (the same staging in count form)**:

| Stage | PR | Cumulative surviving rules |
|---|---|---|
| Pre-collapse | — | **59** |
| Stage 1 | PR 3b | **13–18** |
| Stage 2 | PR 3.7 | 13–18 (catalog compaction only) |
| Stage 3 | PR 4 | **10–15** |
| Stage 4 | PR 5+ | **9–11** |

**What changed relative to §3.10.3's prior framing.** Move 1 (banner roll-up) lands as ONE walker in PR 3b that retires (or reduces) in PR 4 — not as immediate property-test conversion. Move 3 splits across PR 3b (enumerated) + PR 4 (family-predicate compaction). Move 5 (abhors-company) absorbs into Moves 2 and 3 with no separate walker. Move 6 lands as primitive in PR 3.7 + wiring in PR 4. Move 7 lands as fallback walker in PR 3b + renderer takeover in PR 5+.

**Audibles permitted** (per `2026-05-07-pr3b-consultation-verdict.md` §7):

- PR 3b reviewer MAY skip 3b.A if PR 4 reviewer accepts property-test-only coverage.
- PR 3b implementer MAY land sub-moves as separate sub-PRs (3b1 / 3b2 / 3b3) if review bandwidth requires.
- PR 3.7 implementer MAY discover `FgiSet` already models §4.8 and skip T108d's primitive landing in favor of doc-comment amendment only.

Audibles beyond these require a dated amendment to the verdict doc.

---

## §4. The per-axis lattice operations — what CAPCO actually defines

CAPCO defines roll-up operations on SCI, REL TO, FGI, and class. It does **not** define their duals. Each axis is therefore named here by the operation CAPCO requires; the section that previously proved lattice-completeness for the unrequired SCI dual is demoted to §4.6.

### 4.1 What CAPCO defines and what it doesn't

From `[capco-2016]` §A.6 p15 (SCI roll-up), §H.7 p138–141 (FGI roll-up), §H.8 p150–151 (REL TO roll-up), and the OrdMax classification chain (§B):

- **SCI across portions**: union compartments and sub-compartments per system; stack different systems side-by-side. The banner is the *most restrictive* aggregate.
- **REL TO across portions**: intersect country lists. The banner releases only to countries every portion authorizes.
- **FGI (acknowledged) across portions**: union country lists. Every country with equity is named on the banner. FGI says nothing about *who can read* the document — that's REL TO / NOFORN / EYES territory.
- **Classification across portions**: max along the chain (U ≤ C ≤ S ≤ TS). Foreign classifications normalize to US-equivalents at portion-parse time (`[capco-2016]` §H.7 reciprocal-classification rule).

What CAPCO **does not** define:

- "Meet" of two SCI markings — there is no "least restrictive common access" concept; SCI controls are indestructible.
- "Meet" of two FGI country sets — equity is additive, not subtractive.
- A dual for any of the page-level operations above.

The §3.3a doc's framing — "tree intersection is not unique, here are three policies (a)(b)(c)" — was answering a question CAPCO doesn't ask. The previous §4.2/§4.3 walked through which policy is the "right" meet under various orders; the walk is mathematically clean but operationally vacuous. See §4.6 for the cleanup.

### 4.2 SCI: join-semilattice (indestructibility)

**Operational claim.** Banner SCI = union of per-portion SCI markings. To read the document, you must be read on to every system listed, every compartment under every system, and meet handling requirements for every sub-compartment. Any one missing and you can't access — hence "indestructible."

**Lattice claim.** `SciSet` is a `JoinSemilattice` with no top (compartments are agency-extensible, per §5) and no operationally meaningful meet (CAPCO has no "less restrictive" semantics).

**Join law.** Fact-set union under prefix-inclusion order, where a marking `SI-G ABCD` contributes the facts `{SI present, SI-G present, SI-G-ABCD present}`. Join = set-union of these fact sets. Grammar `CONTROL-COMP (SPACE SUB-COMP)*(-COMP (SPACE SUB-COMP)*)*` (CAPCO §A.6) means compartments stack under a single control via repeated `-COMP`; different control systems are separated by `/`.

| Inputs (per-portion markings) | Join (banner roll-up) | Why |
|---|---|---|
| `SI` and `SI-G` | `SI-G` | same control, add compartment |
| `SI-G` and `SI-G ABCD` | `SI-G ABCD` | same control + compartment, add sub-comp |
| `SI-G ABCD` and `SI-G DEFG` | `SI-G ABCD DEFG` | same control + compartment, union sub-comps (alpha order, §A.6 p15) |
| `HCS-O` and `HCS-P MNOP` | `HCS-O-P MNOP` | same control, stack compartments via `-COMP`; sub-comp rides on P only |
| `SI-G ABCD` and `SI-H DEFG` | `SI-G ABCD-H DEFG` | same control, stack two compartments; grammatically valid, rare in practice |
| `SI-G` and `TK-BLFH ABCD` | `SI-G/TK-BLFH ABCD` | different controls, `/` separator |

**Lattice laws on join.** Idempotence (`a ⊔ a = a`), commutativity, and associativity all hold trivially because set-union is well-behaved. There is no meet to verify absorption against — the operation isn't defined.

**Code reference.** `PageContext::expected_sci_markings()` (per CLAUDE.md "Banner roll-up for SCI (E035)") computes exactly this union, sorted per §A.6 p15 (numeric first, alpha after). This is the production banner-roll-up path; no caller invokes the trait-impl meet (see §4.6).

### 4.3 REL TO: meet-semilattice with order-flipped join

**Operational claim.** Page REL TO = intersection of portion REL TO country lists. The page can release to a country only if every portion authorizes that country.

**Lattice claim.** `IntersectSet` is a bounded meet-semilattice. Marque chooses the order-flip convention where set-intersection plays the role of join, so the lattice's "up" direction (`⊒`) corresponds to "fewer countries = more restrictive." Empty set is the top; full ISO 3166 country space is the bottom.

**Join law.** `{USA, GBR} ⊔ {USA, FRA} = {USA}`. Idempotence, commutativity, associativity all hold by set-intersection's properties.

**Code reference.** `IntersectSet<CountryCode>` in `crates/scheme/src/builtins.rs`; `crates/capco/src/lattice.rs` for the REL TO instantiation.

### 4.4 FGI (acknowledged): union of equity, not of access

**Operational claim.** Banner FGI country list = union of portion FGI country lists. Every country with equity in the aggregate document is named.

**Critical point.** FGI is about *equity, not access*. An FGI marking says "this country (or countries) has rights over this information — declassification authority, change control, return-to-originator obligations." It says nothing about who can read the document. A very common pattern is `S//FGI [trigraph]//NOFORN`: a country gave us information with the explicit instruction not to release it back to *their own* government (because internal trust within their government is limited, and REL TO has no unit-level granularity to express "release to Service X but not Ministry Y"). The FGI attribution preserves their equity; the NOFORN closes off access — including back to them. The two are orthogonal axes of the marking, not in tension.

**Lattice claim.** The FGI country set is a join-semilattice under union (bounded only by the closed ISO 3166 country space, so `BoundedLattice` is permitted but the top rarely matters operationally). FGI does NOT enter the access-control reasoning that governs REL TO / NOFORN / EYES.

**Code reference.** `FgiSet` in `crates/capco/src/lattice.rs`. The acknowledged-FGI country join is union; the concealment-supersession layer is described in §5.3.

### 4.5 FGI form selection: consensus-or-fallback (rendering, not lattice)

The FGI *country set* (§4.4) is a clean join-semilattice. The FGI *banner form* — bare `//DEU TS//` (foreign-classified-only) vs. rolled-up `TS//FGI DEU//` (US-classified with FGI equity) — is a separate question, and it is **not** a lattice operation: it's a *rendering choice* driven by two operational facts.

Bare survives at the banner iff:

1. Every portion's FGI attribution agrees on the same `(form, countries)` pair, AND
2. No portion contributes a US classification — i.e., the OrdMax class axis is exclusively foreign-classified.

Either condition failing forces the rolled-up form `<US-class>//FGI [country union]//`. The country union is the §4.4 join; the form decision is a cross-axis read of the OrdMax class axis. The §4.8 lattice models this as a flat-join-semilattice with disagreement-at-top carrying the rolled-up country set; the bare-vs-rolled distinction is then a renderer concern keyed on the FGI lattice element plus the class axis state.

**This is the §4.8 primitive.** Detailed mathematical treatment, lattice laws, and worked example are preserved in §4.8 (with the carrier revised to drop `class` — §4.8.2 below).

**JOINT semantics — research needed.** Operationally, `//JOINT S AUS USA//` appears to imply `REL TO ⊇ {AUS, USA}` — joint-produced documents typically release to all listed parties by default, and the rare "JOINT but not REL'd to self" pattern (the FGI/NOFORN equivalent for joint partners) has been observed approximately zero times in HUMINT practice. **This implication is a hypothesis, not a confirmed CAPCO rule.** The user's operational experience may be conflating empirical pattern with normative requirement. Before encoding `joint-implies-rel-to` as a closure rule (§4.7), verify against `[capco-2016]` §H.3 + §H.7 text. If confirmed, the rule belongs in the closure catalog with `trigger: JOINT(countries) present` and `cone: REL TO ⊇ countries`. The topological scheduler's `writes: [rel_to]` ordering will sequence it before `noforn-clears-rel-to` automatically.

JOINT classification (genuinely co-produced documents) is rare outside NATO contexts — NATO uses NATO markings, not JOINT. Even bilateral operations typically have a single owner. The structural question of whether JOINT lives in the FGI lattice's `bare(form, countries)` carrier or wants its own axis is deferred to the Stage 4+ incompatibility-class reframe per the user's MEMORY note (`project_incompatibility_class.md`).

### 4.6 Footnote: `SciSet::meet` and why it's operationally vacuous

`SciSet` currently implements the full `Lattice` trait, which requires both a join and a meet. The meet implementation (`crates/capco/src/lattice.rs:234–261`) computes fact-set intersection under the prefix-inclusion order:

- `SI ⊓ SI-G ABCD = SI`
- `SI-G ABCD ⊓ SI-H DEFG = SI`
- `SI-G ⊓ SI-G ABCD = SI-G`

These compute exactly what they advertise, and they satisfy idempotence, commutativity, associativity, and absorption under that order. The previous §4.2/§4.3 verdict ("policy (b) IS the categorical meet under fact-set order") is mathematically true. The order is the canonical "structural inclusion" order on tree-shaped data — equivalent to `pure-lattice.md` §6 "Birkhoff's representation theorem" applied to the down-set structure. Source: `[birkhoff-1937]`; `[davey-priestley-2002]` Theorem 5.12.

**No production code path calls `SciSet::meet`.** The operation exists to satisfy the trait surface. Per §4.2, CAPCO defines no "least restrictive common SCI" operation — SCI indestructibility is exactly the statement that there is no operationally meaningful "down" direction.

**Recommendation.** Demote `SciSet` from `Lattice` to a hypothetical `JoinSemilattice` trait once the trait surface supports the split. Until then, document the meet as "defined for trait-impl completeness; no CAPCO semantics; not invoked by any rule." Tighten the doc comment at `crates/capco/src/lattice.rs:14–39` accordingly, and keep `SciSet::overlaps` and `SciSet::common_compartments` as documented alternative views for callers who want different interpretations.

The same caveat applies to any meet on `FgiSet` country sets — equity is additive, not subtractive, and CAPCO defines no "intersection of FGI country sets" operation. The FgiSet meet (if implemented) is similarly trait-completeness-only.

### 4.7 The closure operator (Phase-A: implied-fact propagation)

A second algebraic primitive surfaces from the user's structural map (the marking-relationship roster they assembled while the consultant skill was being built): many CAPCO rules currently encoded as runtime "if X is present, Y must also be present" checks are not constraints — they are **implications**. The fact `Y` is *derived* from the fact `X`, not independently asserted by the document. Treating this derivation as a closure operator on the joint fact-set turns Phase-A into "parse → close → fold," and the runtime check rules disappear because the implied facts are already present after closure.

This is a new primitive, not in the existing `marque-scheme::builtins` roster. It belongs alongside the per-axis lattices, not inside any one of them — closure rules cross axes (e.g., `SI-G` on the SCI axis implies `ORCON` on the dissem axis and `ClassLevel ⊒ TS` on the class axis).

#### 4.7.1 The problem this solves — and the canonical worked example: the implicit-default trio

The user's revised structural map crystallizes a pattern that was previously diffused across the "always-NOFORN" / "ORCON-ish" / "always-classified" lists: many CAPCO rules currently encoded as runtime "if X is present, Y must also be present" checks share **a single closure shape with a single suppression predicate**. The cleanest worked example is the **implicit-default trio**:

| Implicit default | Fires when | Suppressed when | What it adds |
|---|---|---|---|
| **NOFORN-if-no-FD&R** | A "prefers NOFORN" marking is present in the page state | The page already carries any FD&R marking (anything in the FD&R supersession axis) | `{NOFORN}` to the dissem axis |
| **RELIDO-if-no-FD&R-and-not-incompat** | A "RELIDO-default" marking is present | The page already carries any FD&R marking OR carries any RELIDO-incompatible marking (per §3.4.2 family predicates) | `{RELIDO}` to the dissem axis |
| **REL-USA-NATO-if-no-FD&R-and-NATO** | The page contains any NATO portion (banner CTS / NS / NC / NR / NU / ATOMAL / BOHEMIA / BALK / ORCON-NATO) | The page already carries any FD&R marking | `{REL TO USA, NATO}` to the FD&R axis |

All three share the **same suppression predicate**: `has_fdr(x)` — "does the page state already carry any FD&R marking?" The three differ only in their *trigger* (which markings cause them to fire) and their *added cone* (NOFORN, RELIDO, or REL TO USA, NATO).

This unification simplifies the closure-operator implementation: a single `Cl_supp` instance with three implications, all sharing one suppressor. Rules that today encode "if X then NOFORN must be present" become catalog data (a `(trigger_predicate, added_fact, suppressor)` tuple); the engine fires them automatically; no `impl Rule` blocks needed.

**The implicit-NOFORN trigger list** (per user's revised structural map):

```text
implicit_NOFORN.triggers = {
  // SCI / SAP / AEA territory (the originator-control implicit)
  any SAP program,
  RD, FRD, TFNI,
  DOD UCNI, DOE UCNI,

  // Foreign-equity / origination (the foreign-source implicit)
  FGI,
  any FGI [trigraph not NATO],
  any [trigraph (incl. bare FGI) class not NATO],
  ORCON, ORCON-USGOV,
  IMCON,
  DSEN,

  // Distribution-restricted (the no-public-release implicit)
  LIMDIS,
  LES,
  SBU,
  SSI,
  NNPI,                              // see §3.7.2 NNPI bounded confidence
}
suppressor: has_fdr(page)
added_fact: {NOFORN ∈ Dissem}
```

**The implicit-RELIDO trigger list** (per user's revised structural map):

```text
implicit_RELIDO.triggers = {
  any SCI control (except where NF is required by §4.7.5 SI-G / HCS-O / HCS-P[sub] / TK-BLFH / TK-KAND / TK-IDIT — these go through the implicit-NOFORN path),
  US unclassified or collateral classification (U / C / S / TS, with no other dissem),
  RSEN,
  FOUO,
}
suppressor: has_fdr(page) ∨ has_relido_incompatible(page)
added_fact: {RELIDO ∈ Dissem}

// has_relido_incompatible(x) = RELIDO conflict per §3.4.2 family predicates
//   x carries any of: NOFORN, LES-NF, SBU-NF, DISPLAY ONLY,
//                     any FGI atom, any JOINT atom, any NATO atom
```

**The implicit-REL-USA-NATO trigger list** (per user's revised structural map):

```text
implicit_REL_USA_NATO.triggers = {
  any NATO portion or banner — any of:
    CTS / NS / NC / NR / NU,
    ATOMAL, BOHEMIA, BALK,
    ORCON-NATO,
    REL TO [any list including NATO],
}
suppressor: has_fdr(page)
added_fact: {REL TO USA, NATO ∈ FD&R}
```

These three implicit defaults are the **canonical worked example** for the closure operator: each is monotone-extensive-idempotent (verified below), each has a clean per-axis location for the added fact, and the suppressor is a presence-check on the joint fact-set (so monotone-in-`x`).

**Per-marking implications (the rest of the implication table).** Beyond the trio, the user's map also provides per-marking implications that are *not* gated on FD&R-suppression:

- `HCS-O` ⇒ `{NOFORN, ORCON}` — both axes; not gated (NOFORN/ORCON pair fires regardless of other FD&R)
- `HCS-P [sub]` ⇒ `{NOFORN, ORCON}` — same
- `SI-G` ⇒ `{ORCON, ClassLevel ⊒ TS}` — ORCON unconditional; class floor is §3.4.6 territory
- `SI-[any compartment]` ⇒ `{ClassLevel = TS}` — class floor (§3.4.6)
- `HCS` ⇒ `{ClassLevel ⊒ S}` — class floor (§3.4.6)
- `TK-BLFH` ⇒ `{NOFORN, ClassLevel = TS}` — NOFORN unconditional; class floor (§3.4.6)
- `TK-KAND` / `TK-IDIT` ⇒ `{NOFORN}` — NOFORN unconditional
- `BOHEMIA` / `BALK` ⇒ `{ClassLevel = CTS}` — class floor (§3.4.6)
- `RD` ⇒ `{Classified}` — class floor (§3.4.6)

The unconditional NOFORN/ORCON implications (HCS-O, HCS-P[sub], TK-BLFH, TK-KAND, TK-IDIT) are *not* part of the implicit-default trio because they don't have an FD&R suppressor — even when FD&R is already present, these markings still pull NOFORN. The conditional FGI-based implications (any FGI [trigraph not NATO]) are part of the trio.

**Transmutations are NOT closure** (corrected from prior versions): `LES-NF` (in IC context) ⇒ `{NOFORN, drop bare LES-NF, emit LES}` is a *transmutation* (it removes a fact); see §3.4.1 entry 6 + §4.8, not closure.

**The pattern is "presence of X implies presence of Y," monotonically.** The implicit-default trio (gated implications) and the per-marking unconditional implications are both **monotone** (more facts in → more facts out), **extensive** (input facts are preserved), and **idempotent** (closing twice = closing once, given the implication table is finite and the joint fact lattice is finite-height). Three conditions; each one matches the `pure-lattice.md` §18 closure-operator definition.

#### 4.7.2 Mathematical definition

Let `F = Class × Dissem × Equity × SCI × SAR × FGI × ...` be the joint fact lattice across all CAPCO axes (cartesian product of per-axis lattices, `pure-lattice.md` §11; meet/join coordinatewise).

Define `Cl : F → F` by

```text
Cl(x) = x ⊔ ⊔_{(p, c) ∈ ImplTable, p(x)} c
```

where `ImplTable` is a static list of `(predicate, fact-cone)` pairs — one per row of the implication table above. The predicate `p(x)` is a presence check on `x` (e.g., "does `x.SCI` contain `SI-G`?"); `c` is a fact-cone (e.g., `{ORCON ∈ Dissem, ClassLevel ⊒ TS}`).

**Three closure laws to verify:**

- **Monotone** (`x ⊑ y ⇒ Cl(x) ⊑ Cl(y)`). Each `p` in `ImplTable` is upward-closed (if `p(x)` and `x ⊑ y`, then `p(y)` — since presence checks are preserved by the inclusion order). So the union over firing implications can only grow. ✓
- **Extensive** (`x ⊑ Cl(x)`). `Cl(x)` is `x` joined with extra facts; the join in `F` is coordinatewise upper bound. ✓
- **Idempotent** (`Cl(Cl(x)) = Cl(x)`). The implication table is fixed and finite; once all implications have fired, no new ones fire on the second pass. Formally: every implied fact-cone `c` is a *fixed point* of every other implication's predicate (because the table is closed under chaining — see §4.7.3). ✓ given table-closedness; if the table isn't closed under chaining, iterate to fixed point via Kleene (`pure-lattice.md` §20) on the finite-height lattice `F`, and the resulting `Cl*` is the genuine closure.

**Citation.** `pure-lattice.md` §18 (closure operator); `[davey-priestley-2002]` ch. 7 Theorem 7.2; `[burris-sankappanavar-1981]` §II.2; the Galois-bridge with `pure-lattice.md` §17 — every closure operator factors through some Galois connection, here induced by `(p, c)` pairs. Closed elements (`Fix(Cl)`) form their own complete lattice (`pure-lattice.md` §18 Theorem); banner aggregation lives in this sublattice.

#### 4.7.3 Closure-with-suppression: the FD&R override

The implicit-default trio (§4.7.1) is the canonical closure-with-suppression case: implications fire only when the document does **not** already carry an explicit Foreign Disclosure / Release authority. The shared-suppressor structure is:

```text
Cl_supp(x) = x ⊔ ⊔_{(trigger, cone, supp) ∈ ImplTable, trigger(x) ∧ ¬supp(x)} cone
```

Implementations partition into two sub-tables:

```text
ImplTable_unconditional = [
  (HCS-O,         {NOFORN, ORCON},               always_false),
  (HCS-P[sub],    {NOFORN, ORCON},               always_false),
  (TK-BLFH,       {NOFORN, ClassLevel ⊒ TS},     always_false),
  (TK-KAND,       {NOFORN},                       always_false),
  (TK-IDIT,       {NOFORN},                       always_false),
  // ... per §4.7.1 per-marking unconditional implications
]

ImplTable_fdr_suppressed = [
  (any implicit_NOFORN.trigger,        {NOFORN ∈ Dissem},         has_fdr),
  (any implicit_REL_USA_NATO.trigger,  {REL TO USA, NATO ∈ FD&R}, has_fdr),
  (any implicit_RELIDO.trigger,        {RELIDO ∈ Dissem},         has_fdr_or_relido_incompatible),
]
```

The two tables share machinery; they differ only in whether the suppressor is `always_false` (unconditional firing) or `has_fdr(x)` / `has_fdr_or_relido_incompatible(x)` (FD&R-gated firing).

**Why the shared FD&R suppressor is monotone.** The suppressor `has_fdr(x)` is a presence-check on the joint fact-set: `has_fdr(x) ⇔ ∃ t ∈ {NOFORN, LES-NF, SBU-NF, DISPLAY ONLY, REL TO [any list]} . t ∈ x`. Presence-checks are upward-closed (if `t ∈ x` and `x ⊑ y`, then `t ∈ y`), so `has_fdr(x) ⇒ has_fdr(y)` whenever `x ⊑ y`. Similarly for `has_fdr_or_relido_incompatible(x)` (union of two upward-closed predicates remains upward-closed). The suppressor *grows* as `x` grows; equivalently, the *un*-suppressor `¬supp(x)` shrinks as `x` grows; equivalently, the firing set of suppressed implications shrinks as `x` grows.

**Net effect on monotonicity** (`x ⊑ y ⇒ Cl_supp(x) ⊑ Cl_supp(y)`). Decompose into three cases:

1. **Implication's trigger fires on both `x` and `y`, and is unsuppressed on both.** Both sides contribute `cone`; preserved by `⊔` monotonicity.
2. **Implication's trigger fires on both, unsuppressed on `x`, suppressed on `y`.** Left side contributes `cone`; right side does not. But suppressor is upward-closed, so the suppressing fact `t ∈ {NOFORN, LES-NF, ...}` is in `y \ x`. By the **table-design property** (verified per-row below), `t` either *contains* `cone` or makes `cone` redundant — so `Cl_supp(y) ⊒ Cl_supp(x)` still holds because `t ⊒ cone` in the relevant axis.
3. **Implication's trigger doesn't fire on `x` but fires on `y`** (more facts allow more triggers). Right side contributes `cone`; left side doesn't. Right side is the larger Cl_supp result; monotonicity preserved.

The table-design property (case 2): "when the suppressor fires, its content covers the suppressed cone's intent." For the implicit-default trio:

- `has_fdr(x) ⇒` page state contains a manifest FD&R decision (`REL TO`, `DISPLAY ONLY`, `NOFORN`, etc.). The suppressed cone (`NOFORN` or `REL TO USA, NATO`) is an *implicit* default; the manifest FD&R supersedes the implicit default by definition. Cite §H.8 p145 (NOFORN top of FD&R chain) + §H.8 p152 (REL TO operative whenever explicitly marked).
- `has_fdr_or_relido_incompatible(x) ⇒` page state either has explicit FD&R *or* carries a marking that excludes RELIDO (NOFORN-style or non-US-equity per §3.4.2). The suppressed cone (`RELIDO`) cannot fire because RELIDO is incompatible with the marking; no `cone`-contribution is missed.

For all three trio entries, the per-row verification holds — the suppressor's content is the suppressed implication's intent.

**Verdict on suppression.** Closure operator confirmed. Monotonicity, extensivity, idempotence all hold for `Cl_supp` with the shared-FD&R-suppressor design. The proof is per-row of the implication table (case 2 above), not generic; but the user's three trio entries plus the per-marking unconditional implications all check out. **Property-test obligation**: `proptest_closure.rs` should exercise idempotence and monotonicity across random fact-sets and verify that the FD&R suppressor doesn't break either law on the trio entries specifically.

**Q-4.7-Cl_supp (carry forward to §9).** Should the implicit-default trio be implemented with **a single shared FD&R-presence predicate** (the cleanest option — one predicate covers NOFORN-trio and REL-USA-NATO-trio, a slight extension for RELIDO-trio's incompatibility check), or with **per-row suppressors** (more flexible, supports future implications with non-FD&R suppressors)? The consultant's recommendation: shared FD&R predicate as the primary implementation; per-row override available for future implications that need it. This is consistent with the §4.7.1 table-design unification.

#### 4.7.4 Interaction with per-axis join (Phase-A pipeline ordering)

Closure commutes with monotone joins in the inclusion direction: `Cl(a) ⊔ Cl(b) ⊑ Cl(a ⊔ b)`. The inclusion can be strict — closing the join may surface implications that didn't fire on either factor alone. (Example: portion 1 has `HCS` alone, portion 2 has `P` alone; neither has `HCS-P`, so neither closes to NOFORN; but `Cl(portion1 ⊔ portion2)` doesn't add NOFORN either, because `HCS` and `P` need to be in the same portion's SCI structure to form `HCS-P`. So in this case the two sides agree. The strictness shows up in other implications — e.g., if class-axis aggregation pushes a portion above TS, and a subsequent implication only fires at TS+.)

The Phase-A pipeline is therefore:

```text
parse_portion → Cl_supp → ... (one closed portion fact-set)
fold portions per-axis (join over closed portions)
Cl_supp once more on the page-level fact-set
```

The double-close is cheap by idempotence: closing already-closed inputs runs through `ImplTable` once, fires only the implications that depend on the joined-up state (typically the cross-portion ones), then stabilizes.

#### 4.7.5 What CAPCO rules collapse into closure

A roster of rules that disappear when the §4.7 closure operator is in place, organized by the three-trio framing (§4.7.1).

**Trio 1 — implicit NOFORN (FD&R-suppressed).** Rules of the shape "if any of {SAP, RD, FRD, TFNI, DOD UCNI, DOE UCNI, FGI [trigraph not NATO], ORCON, ORCON-USGOV, IMCON, DSEN, LIMDIS, LES, SBU, SSI, NNPI} is present, NOFORN must be present unless FD&R-marked":

- "If any SAP program → NOFORN unless FD&R-marked" → closure fires NOFORN.
- "If `RD` / `FRD` / `TFNI` → NOFORN unless FD&R-marked" → closure fires NOFORN; class is independently raised by §3.4.6 floor.
- "If `DOD UCNI` / `DOE UCNI` → NOFORN unless FD&R-marked" → closure fires NOFORN; class is bounded by `U` per the user's "U-only marking" framing in §3.4.4.
- "If `FGI [trigraph not NATO]` → NOFORN unless FD&R-marked" → closure fires NOFORN.
- "If `ORCON` / `ORCON-USGOV` → NOFORN unless FD&R-marked" → closure fires NOFORN.
- "If `IMCON` / `DSEN` → NOFORN unless FD&R-marked" → closure fires NOFORN.
- "If `LIMDIS` / `LES` / `SBU` / `SSI` → NOFORN unless FD&R-marked" → closure fires NOFORN.
- "If `NNPI` → NOFORN unless FD&R-marked" — under hypothesis #1 (always-conveys-LES-like, the user's preferred default). See §3.7.2 NNPI bounded confidence; closure entry is provisional.

**Trio 2 — implicit RELIDO (FD&R + RELIDO-incompatible-suppressed).** Rules of the shape "if any of {SCI control (where NF not required), US unclass + collateral, RSEN, FOUO} is present and not RELIDO-incompatible, RELIDO must be present unless FD&R-marked":

- "If a SCI control (e.g., bare `SI`, `TK`, `BUR`, `KLM`, `MVL`) is present and the document is not RELIDO-incompatible → RELIDO unless FD&R-marked" → closure fires RELIDO. **Excludes** SCI controls that already carry NOFORN implication: SI-G, HCS-O, HCS-P[sub], TK-BLFH, TK-KAND, TK-IDIT — those go through Trio 1.
- "If US unclass / collateral classification only (no other dissem) → RELIDO unless FD&R-marked" → closure fires RELIDO on `(U)`, `(C)`, `(S)`, `(TS)`-only portions.
- "If `RSEN` → RELIDO unless FD&R-marked" → closure fires RELIDO.
- "If `FOUO` → RELIDO unless FD&R-marked" → closure fires RELIDO. Note that FOUO is itself FD&R-bottom and gets evicted by any other dissem control (§3.4.4) — the implicit-RELIDO addition only fires on bare-FOUO portions.

**Trio 3 — implicit REL TO USA, NATO (FD&R-suppressed).** Rules of the shape "if any NATO portion or banner is present and the document is not FD&R-marked, REL TO USA, NATO must be present":

- "If any NATO classification (`CTS` / `NS` / `NC` / `NR` / `NU`) → REL TO USA, NATO unless FD&R-marked" → closure fires REL TO USA, NATO.
- "If `ATOMAL` / `BOHEMIA` / `BALK` → REL TO USA, NATO unless FD&R-marked" → closure fires REL TO USA, NATO.
- "If `ORCON-NATO` → REL TO USA, NATO unless FD&R-marked" → closure fires REL TO USA, NATO.

**Per-marking unconditional implications.** Rules that fire regardless of FD&R state — these have an unconditional `always_false` suppressor in `ImplTable_unconditional`:

- "If `SI-G`, then `ORCON` must be present" → closure fires `ORCON`.
- "If `HCS-O`, then `NOFORN` and `ORCON` must be present" → closure fires both.
- "If `HCS-P [sub]`, then `NOFORN` and `ORCON` must be present" → closure fires both.
- "If `TK-BLFH`, then `NOFORN` (class floor `TS` handled by §3.4.6)" → closure fires `NOFORN`.
- "If `TK-KAND` / `TK-IDIT`, then `NOFORN`" → closure fires `NOFORN`.

**Class-floor implications go to §3.4.6.** Rules of the shape "if X is present, ClassLevel must be ⊒ F(X)" are class floor checks, not closure additions. They live as `Constraint::Requires(X, ClassLevel ⊒ F(X))` entries per §3.4.6, fired by the generic `Requires` walker. The closure operator does not raise the class level; the constraint walker emits a diagnostic and `FixIntent` (post-PR 3c) proposes the class promotion. Rules of this shape: SI-G ⇒ TS, SI-[any] ⇒ TS, HCS ⇒ S, HCS-[comp] ⇒ S, HCS-[comp][sub] ⇒ TS, RD ⇒ C, BOHEMIA / BALK ⇒ CTS (via reciprocal raise), TK-BLFH ⇒ TS, etc. See §3.4.6 catalog.

**The "RSV requires a compartment" distinction.** "If `RSV`, then compartment present" is **NOT closure**. Closure is for facts that should be *added* when implied; this is a *requires* constraint where the missing fact indicates an error rather than an implicit assumption. RSV without a compartment is *malformed*, not augmentable. Belongs in `Constraint::Requires`, not closure. The two primitives serve different roles:

| Pattern | Closure (this section) | `Constraint::Requires` |
|---|---|---|
| Trigger | M is present | M is present |
| Missing fact | Y is *added* automatically | Diagnostic emitted; fix is applied via `FixIntent` |
| User-visible | Silent (or info-level diagnostic) | Diagnostic emitted at warn/error level |
| When to use | Y is an *implicit consequence* of M (M alone is well-formed; we just propagate the implication) | Y is *required* for M to be well-formed at all (M without Y is malformed input) |

The implicit-default trio belongs in closure (the document is well-formed without explicit NOFORN; we just add it). The class-floor implications belong in `Constraint::Requires` (the document is malformed if M is present without sufficient class — the user has to fix this, not just accept silent augmentation).

#### 4.7.6 Where this primitive lives in marque-scheme

Open design question. Two viable shapes:

- **(α) Trait surface on `MarkingScheme`**: add `fn closure(&self, x: ProjectedMarking) -> ProjectedMarking` as a required (or default-no-op) method. Each scheme adapter (`CapcoScheme`, future `CuiScheme`, etc.) provides its own `ImplTable`. The engine invokes `closure` after parsing and after page-level join.
- **(β) Standalone `Closure<S>` type** in `marque-scheme::builtins`, alongside `OrdMax` / `FlatSet` / `IntersectSet` / etc. Schemes attach a `Closure` instance to their adapter via a method like `fn closure(&self) -> &'static Closure<Self>`.

Shape (α) is simpler at the trait level but couples each scheme's closure to its adapter; shape (β) is more compositional (closures could be unioned, or scoped per axis) but adds another type to the surface. I lean (α) for v1: closure is fundamentally a per-scheme concern, and the implication table is a static list, not a recursive structure that benefits from compositional types.

#### 4.7.7 Open questions

- Q-4.7a: Adopt `Cl_supp` as a `MarkingScheme` trait method? (Default: yes, shape α.)
- Q-4.7b: Where does the `ImplTable` live — generated by `build.rs` from CVE/Schematron metadata, or hand-curated in `marque-capco`? (CAPCO-2016 doesn't formalize implications uniformly; some are in §A.6 (SCI), some in §H.5 (SAR), some in §H.6 (AEA). Hand-curated with citations seems likeliest.)
- Q-4.7c: Should closure run at portion granularity, page granularity, or both? (Both — see §4.7.4 — but worth confirming the engine's call sites.)
- Q-4.7d: Property-test obligation: `proptest_closure.rs` should verify monotone + extensive + idempotent on random fact-sets, plus the suppression-doesn't-break-monotonicity property in §4.7.3.

#### 4.7.8 Consultant tags

`(a)` whenever the user has "if X is present, Y must also be present" rules and the rule list is monotone (Y is added, never replaced). The closure-operator construction is the canonical algebraic shape; the runtime rules become redundant once closure is wired. `(b)` when the implication is non-monotone (Y *replaces* X — that's a transmutation, see §4.8 / §3.4). `(c)` when the rules form a graph with cycles that don't stabilize on a finite-height lattice — closure still exists per Knaster-Tarski (`pure-lattice.md` §19), but practical termination requires an analysis of the cycle structure.

> **When this comes up.** "We have a bunch of rules that fire warnings when X-is-present-but-Y-is-missing — can we stop checking and just add Y automatically?" If the implication is monotone, yes — closure operator. The runtime checks disappear, and the property is enforced by construction. If the implication has a "unless Z" escape, closure-with-suppression still works as long as Z is monotone-detectable in the fact-set.

---

### 4.8 The FGI/JOINT-attribution lattice (FlatSet-with-disagreement)

The second new primitive surfaced by the user's map: FGI/JOINT banner attribution doesn't fit any existing per-axis lattice. The required behavior — "bare form survives at banner iff every portion agrees on the same attribution; otherwise roll up to `//FGI [list]//`" — is a **consensus-or-fallback** pattern that needs a refinement of `FlatSet`.

#### 4.8.1 The problem and the consensus-or-fallback pattern

User's framing (paraphrased from the structural map): you only see bare `//DEU TS//` or `//JOINT S AUS USA//` in a banner if **every portion** of the document is exactly that attribution. If any portion disagrees (different country, different JOINT membership, US-only portion mixed in), the banner uses the rolled-up form `US-class // FGI [country list] // [NOFORN if no REL intersection]`.

This is not a `FlatSet<Attribution>` (which would `union` the attributions and lose the consensus distinction); it is not `IntersectSet<Attribution>` (which would intersect and lose disagreeing portions); it is not `Mode<Attribution>` (which only returns the "majority" — not what's wanted). The right shape is a **flat lattice** where atoms are bare attributions and the top is "Disagreement, fall back to rolled-up form."

The flat lattice is a textbook construction in domain theory (`pure-lattice.md` §12 — sum / disjoint-union lattice; `[davey-priestley-2002]` Definition 1.31): carrier `{⊥} ∪ Atoms ∪ {⊤}`, with `⊥ ≤ a ≤ ⊤` for every atom and atoms pairwise incomparable. Joins: `a ∨ a = a`, `a ∨ b = ⊤` for distinct atoms `a ≠ b`, `⊥ ∨ x = x`. The marque refinement is to carry data at the top — the country list — for the renderer.

#### 4.8.2 Mathematical definition: bounded join-semilattice

Define `FgiAttribution` over the carrier:

```text
{⊥}                              -- "no FGI/JOINT attribution on this portion"
∪ {bare(form, countries)}        -- "this portion is exactly form X with country-set Y"
                                   --   form ∈ {FGI-bare, JOINT}
∪ {⊤(countries: Set<Trigraph>)}  -- "rolled-up FGI list (renderer emits //FGI [countries]//)"
```

Class is **not** part of the carrier. Per §4.4, FGI is purely about equity; the class on the banner comes from the OrdMax axis (with foreign classifications normalized to US-equivalents at portion-parse time per `[capco-2016]` §H.7 reciprocal-classification).

Order: `⊥ ⊑ a ⊑ ⊤(s)` for every atom `a` such that `trigraphs(a) ⊆ s`; atoms pairwise incomparable.

Join law:

- `⊥ ∨ x = x ∨ ⊥ = x`
- `bare(f, C) ∨ bare(f, C) = bare(f, C)`  (consensus: same form, same countries)
- `bare(f₁, C₁) ∨ bare(f₂, C₂) = ⊤(C₁ ∪ C₂)`  (disagreement on form OR countries)
- `⊤(s) ∨ ⊤(t) = ⊤(s ∪ t)`
- `bare(f, C) ∨ ⊤(s) = ⊤(C ∪ s)`

The atom→`⊤` collapse drops the form (FGI-bare vs JOINT). The country-set is the only data that survives in the lattice; `JOINT` membership semantics ("USA member is implicit, drops from list") is applied at the join boundary by the `trigraphs(·)` extractor. Distinct atoms collapse whenever EITHER form OR country-set differs — same form with different country sets is still disagreement (consensus is "every portion's exact attribution"), so e.g. `bare(FGI-bare, {DEU}) ∨ bare(FGI-bare, {AUS}) = ⊤({AUS, DEU})`.

This is a **bounded join-semilattice**: it has `⊥` (bottom) and `⊤(All-countries)` (top of the chain of `⊤`s). It is *not* a full lattice — the meet of two distinct atoms isn't well-defined in this domain (and isn't needed, because Phase A only uses joins for banner aggregation). If a meet is required for the trait surface, define `bare(...) ∧ bare(...) = ⊥` for distinct atoms and `⊤(s) ∧ ⊤(t) = ⊤(s ∩ t)` (with `⊤(∅) = ⊥`); this gives a lattice but the meet has no operational meaning in Phase A.

#### 4.8.3 Lattice laws on the join

- **Idempotence** (`x ∨ x = x`): `bare ∨ bare = bare` by reflexive equality; `⊤(s) ∨ ⊤(s) = ⊤(s)` by `s ∪ s = s`. ✓
- **Commutativity** (`x ∨ y = y ∨ x`): set union is commutative; the disagreement collapse is symmetric (`bare(f₁) ∨ bare(f₂) = ⊤(C₁ ∪ C₂) = ⊤(C₂ ∪ C₁) = bare(f₂) ∨ bare(f₁)`). ✓
- **Associativity** (`(x ∨ y) ∨ z = x ∨ (y ∨ z)`): set union is associative. The atom-collapse case: `(bare₁ ∨ bare₂) ∨ bare₃ = ⊤(C₁ ∪ C₂) ∨ bare₃ = ⊤(C₁ ∪ C₂ ∪ C₃)`, vs. `bare₁ ∨ (bare₂ ∨ bare₃) = bare₁ ∨ ⊤(C₂ ∪ C₃) = ⊤(C₁ ∪ C₂ ∪ C₃)`. ✓
- **Absorption** (`x ∨ (x ∧ y) = x`, `x ∧ (x ∨ y) = x`): under the meet defined above (`bare₁ ∧ bare₂ = ⊥` for distinct atoms), absorption holds trivially because `bare ∧ bare = bare`, `bare ∨ ⊥ = bare`, and `bare ∧ (bare ∨ bare') = bare ∧ ⊤(C ∪ C') = bare` (since `bare ⊑ ⊤(C ∪ C')`). ✓ — but only if we define the meet as above; if the meet is omitted, "absorption" is moot and we have a join-semilattice instead.

**Verdict.** Bounded join-semilattice. Lattice if the meet is defined as in §4.8.2. Either way, all four standard laws hold.

#### 4.8.4 Interaction with cross-axis rewrites

The banner-form decision (bare survives vs. rolled-up form) depends on **another axis**: if the class axis carries any US-classified portion, the FGI-attribution must roll up regardless of consensus on the FGI-attribution axis itself. (User's clarification: "you only ever see [bare FGI/JOINT] in a banner if the *entire document* is, in this case, DEU TS or JOINT S AUS USA.")

This is **not** captured by the per-axis lattice alone; it's a cross-axis `PageRewrite`. Sketch:

```text
// Cross-axis rewrite: US presence forces FGI-attribution rollup.
PageRewrite {
  reads: [Class],
  writes: [FgiAttribution],
  guard: page.class.has_us_classification,
  apply: |page| {
    page.fgi_attribution = match page.fgi_attribution {
      ⊥ => ⊥,
      bare(_, C, _) => ⊤(C),    // promote bare to rolled-up
      ⊤(s) => ⊤(s),
    };
  }
}
```

This rewrite consumes both the class axis and the FGI-attribution axis, produces a rewritten FGI-attribution. The topological scheduler (`marque-scheme::PageRewrite::reads/writes`) orders it after class-axis aggregation and FGI-attribution aggregation are both complete. Cite §1.10 (topological scheduler) and `crates/scheme/src/page_rewrite.rs`.

The renderer (Phase C) then emits:

- `bare(FGI-bare, {DEU}, TS)` → `//DEU TS//`  (the bare form survives)
- `bare(JOINT, {AUS, USA}, S)` → `//JOINT S AUS USA//`
- `⊤({GBR})` → `<US-class>//FGI GBR//`  (US class from the class axis; renderer reads both)
- `⊤({AUS, DEU})` → `<US-class>//FGI AUS DEU//`
- `⊥` → no FGI section in banner

#### 4.8.5 Worked example: C//NF + //GBR-TS → TOP SECRET//FGI GBR//NOFORN

Per-axis decomposition of a page with two portions, `(C//NF) ...` (US Confidential, NOFORN) and `(//GBR-TS//) ...` (UK Top Secret).

| Axis | Portion 1 | Portion 2 | Per-axis join | Rewrite (cross-axis) | Final |
|---|---|---|---|---|---|
| Class (`OrdMax`) | C | TS (raised reciprocally on US side) | TS | — | TS |
| FGI-attribution (this primitive) | ⊥ | bare(FGI-bare, {GBR}) | bare(FGI-bare, {GBR}) | US-presence rewrite: ⊤({GBR}) | ⊤({GBR}) |
| Dissem (`SupersessionSet`, NOFORN-top) | {NOFORN} | ∅ | {NOFORN} | closure adds NOFORN from FGI-no-FD&R (redundant; already present) | {NOFORN} |
| REL TO (`IntersectSet`) | ∅ (NOFORN supersedes) | ∅ (no REL info) | ∅ | — | ∅ |

Renderer assembles: class `TS` + FGI-attribution `⊤({GBR})` + dissem `{NOFORN}` → `TOP SECRET//FGI GBR//NOFORN`. ✓

Two things to note from this walkthrough:

1. **The reciprocal-class raise** (GBR-TS → US-TS for join purposes) is **not** performed by the FGI-attribution lattice itself; it's a separate per-axis transformation on the class axis that runs *before* the cross-portion join. CAPCO's reciprocal-classification rule (`[capco-2016]` §H.7) treats foreign classifications as their US-equivalent for class-axis purposes. Either model this as a portion-level normalization (parser emits the US-equivalent class), or as a per-axis `PageRewrite` on the class axis that runs before the `OrdMax` fold. The user's map suggests the former: "U.S. applies reciprocal classifications" reads as a portion-parse-time transform.

2. **NOFORN preservation** is automatic: closure operator (§4.7) fires NOFORN on portion 2 (FGI without FD&R ⇒ NOFORN), and the dissem axis is `SupersessionSet` with NOFORN at the top — once joined in, it stays. The "indestructible NOFORN" property the user emphasized is a theorem of the dissem lattice + closure structure, not a special case.

#### 4.8.6 Where this primitive lives in marque-scheme

Open design question. Two viable shapes:

- **(α) Add to `marque-scheme::builtins`** as `FlatLattice<T>` or `ConsensusLattice<T>` — a generic bounded-join-semilattice with disagreement-top, parameterized by the atom type. CAPCO instantiates it with `FgiAttribution` (the `bare(form, countries)` ADT; class lives on a separate OrdMax axis per §4.4). Domain-neutral; future schemes (CUI compartments?) might reuse it.
- **(β) Keep in `marque-capco`** as `FgiAttributionLattice`, a CAPCO-specific type. Don't expose as a built-in unless a second use case appears.

Shape (α) is compositional; shape (β) is conservative. Lean (β) for v1 — the refinement that makes this useful (the country-set at the top, the form/class drop on disagreement) is CAPCO-specific, and a generic `FlatLattice<T>` would underspecify what data lives at the top. If a second use case appears, lift to `marque-scheme`.

#### 4.8.7 Open questions

- Q-4.8a: Confirm the carrier shape — does `bare(form, countries)` capture every distinct attribution that survives consensus, or are there finer distinctions (e.g., does `//DEU TS//REL TO USA, DEU` differ from `//DEU TS//REL TO USA, FVEY` for consensus purposes)? My read per §4.4: REL-TO is a separate axis; consensus on FGI-attribution is at the `(form, countries)` granularity and class lives on the OrdMax axis. But this should be checked against §H.7 examples.
- Q-4.8b: The reciprocal-class raise — portion-parse-time normalization or `PageRewrite` on the class axis? (Default per §4.8.5 note 1: portion-parse-time.)
- Q-4.8c: Is meet definable / required? (Default: no — Phase A only uses joins. If the trait surface requires `Lattice` rather than `JoinSemilattice`, define meet as `⊥` for distinct atoms + intersection for `⊤`s; meet is operationally meaningless but algebraically clean.)
- Q-4.8d: Lift to `marque-scheme::builtins`, or keep in `marque-capco`? (Default per §4.8.6: keep in `marque-capco`.)
- Q-4.8e: Property-test obligation: `proptest_fgi_attribution.rs` exercises the join laws and the renderer's bare-vs-rollup form selection on random portion sets, including the cross-axis interaction with the class axis.

#### 4.8.8 Consultant tags

`(a)` whenever the user has a "consensus-or-fallback" pattern: bare form survives at the aggregate level iff all factors agree, otherwise fall back to a rolled-up summary. The flat-lattice-with-data-at-top construction is the canonical algebraic shape. `(b)` when the user wants "majority wins" (`Mode`) or "strict union" (`FlatSet`) — those are different patterns and use other built-ins. `(c)` when the consensus check depends on multiple axes — that's a `PageRewrite` on top of the per-axis lattice, not a lattice operation alone.

> **When this comes up.** "We want X to appear in the aggregate iff all inputs agree on X; if any disagree, we want a rolled-up summary instead." That's a flat lattice with disagreement-at-top, refined to carry the rolled-up data at the top. Verify three things: (1) the consensus check is over the *same* atoms (not "compatible" atoms — that's a different lattice with a partial order in the middle); (2) the rolled-up form is computable from the disagreeing inputs (and you know what data needs to ride along at the top); (3) any cross-axis dependencies (e.g., "rollup also fires when *another* axis carries certain content") are expressed as `PageRewrite`s, not folded into the lattice itself.

---

## §5. Open vocab / agency extensibility — applied diagnosis

`SciSet`/`SarSet` deliberately don't implement `BoundedLattice`. The justification in the marque code (`crates/capco/src/lattice.rs:264–272` and `:401–406`) is that compartments / SAR programs are agency-extensible (no enumerable upper bound).

### 5.1 Catalog match

Three converging entries:

- `pure-lattice.md` §4 "Bounded lattice" — open-set / agency-extensible warning: "When a 'vocabulary' can be extended by an outside authority ..., there is *no top element*. The construction is at best a join-semilattice; it is a lattice only on each fixed instantiation. An interface like `BoundedLattice::top()` for such a domain is a category error."
- `security-lattice.md` §18 "SCI / compartmented information as a hierarchical lattice" — exact same argument applied to SCI specifically: "agency-extensible ⇒ no top ⇒ meet-semilattice (or lattice-without-top), not BoundedLattice."
- `universal-algebra.md` §11 "almost-lattice diagnostic" Axis F — formal diagnostic: "no top → 'lattice (or semilattice) with bottom but no top' or 'join-semilattice with bottom.' The right Rust trait is `Lattice` without `BoundedLattice`."

**Verdict.** **(a) exact match across three independent catalog entries.** Marque's discipline (omitting `BoundedLattice` from `SciSet` and `SarSet`) is correct. The decision is well-documented in the source.

### 5.2 Audit: does any operation conflate `top()` with "any plausible upper bound"?

Per the brief, "are all operations consistent with the no-top? Specifically, anywhere the code conflates `top()` with 'any plausible upper bound' is a bug."

I scanned for usages of `top()` in marque-capco. The audit:

- `SciSet`: no `top()` impl, no `SciSet::top` calls. ✓
- `SarSet`: no `top()` impl, no `SarSet::top` calls. ✓
- `FgiSet`: implements `BoundedLattice` (FGI is *not* open-vocabulary in the same sense — country trigraphs are ISO 3166 closed; what's open is whether the source is concealed or acknowledged). The `top` is `Present { concealed: true, countries: ∅ }`. Verified at `crates/capco/src/lattice.rs:556–568`.
- `OptionalSingleton<L>`: `BoundedLattice` only when `L: BoundedLattice` (`builtins.rs:701–714`). So `OptionalSingleton<SciSet>` would NOT compile as `BoundedLattice` because `SciSet` doesn't impl it. ✓ The constraint propagates.
- `Product<A, B>`: same — `BoundedLattice` only when both factors are bounded. `Product<SciSet, FgiSet>` would not compile as `BoundedLattice`. ✓

**No conflations found.** The constraint propagates correctly through the type system.

### 5.3 Subtle case: `FgiSet`'s `top()`

`FgiSet::top()` is `Present { concealed: true, countries: ∅ }`. Is this *really* the top under the natural order? Per `FgiSet::join`:

- `Present { concealed: true, countries: ∅ } ⊔ Present { concealed: false, countries: {GBR} }` = `Present { concealed: true, countries: ∅ }` (concealed supersedes). ✓
- `Present { concealed: true, countries: ∅ } ⊔ None` = `Present { concealed: true, countries: ∅ }`. ✓
- `Present { concealed: true, countries: ∅ } ⊔ Present { concealed: true, countries: ∅ }` = same. ✓ (idempotent)

What about `Present { concealed: true, countries: {GBR} }` — is that representable, and is it `⊑ top`? Per the doc (line 432) and the `join` body (line 497–502), `concealed = true` always forces `countries = ∅`. So the variant `Present { concealed: true, countries: {GBR} }` is *not* a reachable state under the lattice operations; the type allows it (no enum-side enforcement) but the lattice never produces it. **This is a representable-but-unreachable state.** A Rust newtype invariant or a `Present::new_concealed()` constructor would lock it down; today it's a doc-only invariant.

**Open question for the user.** Q-5.3: Should `FgiSet::Present` be refactored to make `concealed: true ⇒ countries: ∅` a type-system invariant, e.g., with `enum FgiSet { None, Concealed, Acknowledged(BTreeSet<CountryCode>) }`? (Default: **yes** — reachable states should be type-representable; unreachable states should be unreachable. Per `pure-lattice.md` §11 "Non-example" — constraints between coordinates that aren't enforced by type create silent footguns.)

### 5.4 Verdict on §5

The marque labeling is correct: `SciSet` and `SarSet` are meet/join-semilattices without top, fitting `Lattice` but not `BoundedLattice`. `FgiSet` is a bounded distributive lattice with a non-synthetic top encoding the concealment supersession. No `top()` conflations were found in the production code.

**Recommendation.**
- **No structural changes needed.** The discipline is correct.
- **Cite the three catalog entries** (pure §4, security §18, universal-algebra §11 Axis F) in the `SciSet` and `SarSet` doc comments so a future maintainer asking "why no `BoundedLattice`?" finds the answer in one read.
- **Consider the `FgiSet` representable-but-unreachable issue** as a separate cleanup — type-system enforcement of invariants is a Phase 9 / consolidated-plan PR-2 priority anyway (per `2026-05-02` plan PR 2 — `FgiMarker::SourceConcealed | Acknowledged`).

---

## §6. PageRewrite + topological scheduler — applied diagnosis

The topological page-rewrite scheduler runs Kahn's algorithm over `PageRewrite::reads`/`writes` axes at `Engine::new`. Cycles fail with `EngineConstructionError::RewriteCycle`; unannotated `Custom` axes fail with `UnannotatedCustomAxes`.

### 6.1 Is this a well-defined lattice construction?

The scheduler itself is *not* a lattice construction; it's a scheduling discipline. The lattice question is whether the *result* of running the scheduled rewrites is well-defined and equals some canonical fixed point.

### 6.2 Catalog match: `abstract-interp.md` §18

`abstract-interp.md` §18 "Topological scheduling as a fixed-point computation" gives the formal framing (sources: `[tarski-1955]`, `[knuth-1973]` Vol. 1 §2.2.3, `[cousot-cousot-1977]` §6):

> Define the joint operator `F(s) = s ⊔ r₁(s) ⊔ r₂(s) ⊔ … ⊔ r_k(s)`. If each `rᵢ` is monotone and inflationary (`s ⊑ rᵢ(s)`), then `F` is monotone and inflationary. Knaster-Tarski (`pure-lattice.md` §19) on the complete lattice `L` guarantees `lfp F` exists. Order-independence: any iteration strategy that fairly applies each `rᵢ` reaches `lfp F`.
>
> When each rewrite `rᵢ` declares "axes I read" and "axes I write", a partial order on `{rᵢ}` arises: `rᵢ → rⱼ` if `rⱼ` reads what `rᵢ` writes. Topological sort gives a *deterministic* schedule — writers before readers. If the read/write graph is acyclic, the topological sort is well-defined; the result is the same as `lfp F` evaluated by Kleene with that schedule.

**Match quality: (a) exact** for the formal framing.

### 6.3 But wait — are CAPCO rewrites monotone?

The Knaster-Tarski guarantee requires each `rᵢ` to be monotone (and inflationary, for the inflationary-lift formulation). Are the CAPCO rewrites?

- **`noforn-clears-rel-to`** is `CategoryAction::Clear { category: CAT_REL_TO }`. This is **NOT inflationary** — clearing REL TO *shrinks* the marking. Concretely, if `s = (dissem: {NOFORN}, rel_to: {USA, GBR})`, then `r(s) = (dissem: {NOFORN}, rel_to: ∅)`, and `s ⋢ r(s)` (because `r(s).rel_to = ∅ ⊏ s.rel_to = {USA, GBR}` under `IntersectSet`'s flipped order... actually wait, under `IntersectSet` the order is flipped so `∅` is the **top**, not the bottom. So under `IntersectSet`'s order, `r(s).rel_to = ∅ ⊒ s.rel_to = {USA, GBR}`, and the rewrite IS inflationary in REL TO. Good.)

So **noforn-clears-rel-to is monotone-and-inflationary under the per-category order**, where `IntersectSet` flips the operator.

- **`joint-promotion`** (stubbed `never_fires`): when implemented, it promotes JOINT countries into REL TO. Adding countries to REL TO under `IntersectSet`'s order *shrinks* the lattice element (more countries = lower). This is **deflationary** under `IntersectSet`'s order, NOT inflationary. The formal framing breaks.

- **`fgi-absorption`** (stubbed): unattributed-FGI absorbs attributed. Per `FgiSet::join`, concealed supersedes acknowledged, so absorption *raises* the FGI marking. This **IS inflationary** under `FgiSet`'s order.

**The mixed monotonicity is a real concern.** If `joint-promotion` is deflationary and `noforn-clears-rel-to` is inflationary, the joint operator `F = id ⊔ r_joint ⊔ r_noforn` is *not* monotone in general. Knaster-Tarski's existence guarantee fails. The scheduler's "writers before readers" discipline still produces a deterministic answer — but the answer is not the lattice `lfp F`; it's a sequence of operations whose result depends on the schedule.

**Crucially, this is not a bug in the marque scheduler.** It's a clarification: the scheduler is **not** computing a Knaster-Tarski fixed point. It's executing a deterministic sequence of operations. The result is well-defined for a given schedule; the topological sort makes the schedule canonical (writers before readers). But it's not a lattice operation — the formal framing in `abstract-interp.md` §18 with the inflationary lift does NOT apply.

### 6.4 The honest framing

The page-rewrite scheduler implements a **deterministic-order rewrite system** (`security-lattice.md` §6 Framing 3 + "When a security policy is NOT a lattice" §5 "Order-dependent rewrites"). The topological order pins the deterministic-order to "writers before readers," which is a sensible operational choice.

The fact that the result equals some `lfp` for an inflationary monotone operator would be a *bonus* — and it does hold for the all-inflationary case, like the all-clearing version of CAPCO's rewrites. For mixed inflationary/deflationary, the result is just "the well-defined output of running these operations in this order."

### 6.5 Recommendation

**Don't claim Knaster-Tarski semantics for the scheduler in the doc.** Instead:

1. Cite `security-lattice.md` "When a security policy is NOT a lattice" §5 ("Order-dependent rewrites") as the operational name: the rewrites are deterministic-order operations, not lattice ops.
2. Cite `abstract-interp.md` §18 as the *aspirational* framing: if all rewrites are monotone-and-inflationary, the scheduler's output equals `lfp F`. This is true for the currently shipped `noforn-clears-rel-to`. It would also be true if `joint-promotion` is reformulated as "annotate REL TO with promoted-from-JOINT countries" (which adds rather than subtracts; depends on the order convention).
3. Verify monotonicity per-rewrite as part of the Phase D/E implementation review for `joint-promotion` and `fgi-absorption`. If a rewrite is genuinely non-monotone, document it explicitly and accept that the scheduler is a deterministic-order sequencing tool, not a fixed-point engine.

**The sound part.** Cycle detection at `Engine::new` is correct: a cycle in the read/write graph means topological sort fails, which means no deterministic schedule exists. Marque's `RewriteCycle` error is the right behavior. Per `abstract-interp.md` §18 "Cycles in the read/write graph": marque takes Option 1 (reject); Option 2 (iterate to convergence via Kleene) would require monotonicity of every rewrite, which marque does not currently establish.

**Open question for the user.** Q-6.5: When `joint-promotion` and `fgi-absorption` are implemented (Phase D/E), should the implementer formally verify monotonicity-and-inflationarity per-rewrite? (Default: **yes** — even if the scheduler doesn't *require* it, knowing the rewrites are monotone unlocks confluence guarantees and lets the scheduler be reordered for performance without changing semantics.)

---

## §7. NOFORN clears REL TO — applied diagnosis

This is `capco/noforn-clears-rel-to`, declared as a `PageRewrite::declarative` at `crates/capco/src/scheme.rs:486–498`. The brief asks: is this a meet on `RelToSet`, a `SupersessionSet` op, or a non-lattice page-level rewrite?

### 7.1 Walk through each candidate framing

**Candidate A: meet on `RelToSet`.** A meet operation would have to be commutative: `noforn ⊓ rel_to = rel_to ⊓ noforn`. But NOFORN is in the *dissem* category and REL TO is in its own *rel_to* category (per `crates/capco/src/scheme.rs:686/710`). They are *cross-category*; there is no single category whose meet would express this. **Rejected.**

Even within a hypothetical merged category that contained both NOFORN and REL TO entries, absorption would fail. Per `pure-lattice.md` §3 "Lattice" definition: absorption requires `x ∧ (x ∨ y) = x`. If `NOFORN ∧ {USA} = NOFORN` (absorbing-element behavior), then `NOFORN ∨ {USA} = ?` — for absorption, we need `NOFORN ∧ (NOFORN ∨ {USA}) = NOFORN`. If `NOFORN ∨ {USA} = NOFORN ∪ {USA}`, then `NOFORN ∧ (NOFORN ∪ {USA}) = NOFORN` — which is consistent if NOFORN is the meet's identity / top in the merged-set lattice. But this requires NOFORN to behave as a *top* on the dissem axis (everything dominated by NOFORN), which doesn't match operational semantics — NOFORN is dominated by the higher-classification top, not a global maximum.

**Candidate B: `SupersessionSet` operation.** `SupersessionSet` per `crates/scheme/src/builtins.rs:328` requires the superseding and superseded tokens to be **in the same set's storage**. NOFORN is in `dissem` (`Vec<DissemControl>`); REL TO countries are in `rel_to` (`Vec<CountryCode>`). The supersession table would need an entry like `(DissemControl::NOFORN, CountryCode::*)` — but the type signature `&'static [(T, T)]` requires both sides to be the same `T`. **Rejected by the type system**; the marque doc comment at `builtins.rs:322–326` explicitly acknowledges this.

**Candidate C: non-lattice page-level rewrite that drops REL TO to ∅.** This is what marque ships: `PageRewrite { trigger: Contains(NOFORN, dissem), action: Clear(rel_to) }`. **Accepted.**

### 7.2 Catalog match

`security-lattice.md` §6 "Supersession algebra (NOFORN-style)" Framing 3 ("non-lattice algebraic rewrite"):

> Implement as a *side-effecting page-level rewrite* — e.g., "if any portion is NOFORN, drop REL TO from the banner" — and that rewrite doesn't compose associatively or commutatively with other page rewrites... The marque codebase's `PageRewrite` mechanism (per the project's `docs/plans/2026-04-19-recursive-lattice-and-decoder.md` and the `marque-engine::scheduler` topological order) is *explicitly* framing NOFORN-clears-REL-TO as a page-level rewrite, with a topological sort that defines a deterministic order — i.e., the third framing, with deterministic order substituted for the (failed) commutativity. The scheduler's existence suggests the project has accepted this is not a lattice operation but a posetal-rewrite operation requiring deterministic ordering.

**Match quality: (a) exact match.** The Agent B catalog called this exact case. The marque implementation aligns with the catalog's recommended framing. Sources: `[wikipedia-absorbing-element]` (the absorbing-element formalism), CAPCO-2016 §H.8 (the operational rule), `[denning-1976]` §3 (the underlying lattice analysis).

### 7.3 The composite: REL TO intersection-with-blackball

The full operational rule is: (1) intersect the per-portion REL TO sets via `IntersectSet::join`, then (2) apply `noforn-clears-rel-to` to drop the result if NOFORN is present. The composite is `security-lattice.md` §7 "Intersection-with-blackball (REL TO style)":

> Without the blackball, `P(Countries)` is a Boolean algebra ... under intersection-as-meet and union-as-join. With NOFORN, see entry 6 above for the three options. In particular, "intersection across portions" *is* a lattice meet operation in the pure Boolean algebra; "intersection-with-blackball" is the absorbing-element-extended version of that meet.

Marque's implementation cleanly separates the two operations: the intersection is a category-wise lattice op (clean), and the blackball is a `PageRewrite` (clean). The composite is the formal "intersection-with-blackball" as described in the catalog. **(a) exact match for the composite.**

### 7.4 Verdict

The marque design is correct and matches the literature. The team has chosen the right framing.

**Recommendation.**
- Cite `security-lattice.md` §6 (Framing 3) in the `noforn-clears-rel-to` `PageRewrite` doc comment.
- Cite `security-lattice.md` §7 in the doc comment on the REL TO category aggregation (`crates/capco/src/scheme.rs:710–723`).
- This naming makes the "this is not a lattice meet" property visible to future readers without forcing them to re-derive it from first principles.

**No open questions.** The design is correct; the documentation work is what's missing.

---

## §8. Decoder confidence propagation — applied diagnosis

The brief asks (briefly) whether the decoder's posterior propagation is a lattice fold, an abstract-interpretation analysis, both, or neither.

### 8.1 What the decoder does

Per CLAUDE.md "Recent Changes" and `marque-engine`:

- `StrictRecognizer` produces `Parsed::Unambiguous(M)` or `Parsed::Ambiguous { candidates }`.
- `DecoderRecognizer` (Phase D) produces a posterior over candidate markings; combines features (`FeatureId` enum) with corpus-derived priors.
- `StrictOrDecoderRecognizer` dispatches strict-first, decoder-fallback.
- `Confidence` carries `recognition: f32`, `rule: f32`; `combined() = recognition * rule`.
- Engine's threshold filter: a `FixProposal` is auto-applied iff `combined() ≥ threshold`.

### 8.2 Catalog match

`abstract-interp.md` §19 "Confidence / posterior propagation as abstract interpretation" (sources: `[cousot-cousot-1977]` general framework; `[monniaux-2001]` and `[cousot-monerau-2012]` for probabilistic AI):

> If the score domain forms a lattice with a meaningful order ("higher score is more confident"), and the per-step combiner is monotone in its inputs, *and* the graph is acyclic (or you accept `lfp` for cycles), then propagation is exactly a monotone framework analysis (entry 9). Lattice values = confidence elements; transfer functions = score combiners; lfp computed by Kleene.

The marque score domain is `[0, 1]` (a chain) or its log-likelihood form `(ℝ ∪ {-∞, +∞}, ≤)`. Both are total orders (chains), hence distributive lattices.

The `combined() = recognition * rule` combiner is monotone on `[0, 1]` in each argument (multiplication preserves order on non-negatives).

**Match quality: (a) for the score lattice + monotone combiner**, but with a caveat: the engine doesn't *iterate* the combiner. It just computes a single product per `FixProposal`. So it's not fixed-point iteration; it's a single fold. The Knaster-Tarski / Kleene machinery doesn't fire because there's no recursion.

### 8.3 The honest framing

The decoder confidence propagation is a **monotone fold over a distributive chain lattice** — sound by construction (multiplication preserves order on `[0, 1]`). It is NOT abstract interpretation in the technical sense (no Galois connection between two lattices, no fixed-point iteration). Per `abstract-interp.md` "Diagnostic — When AI is the right framework, and when it isn't" §"Doing AI" vs "having a lattice with a fixpoint":

> Many marque-shape problems use Knaster-Tarski (`pure-lattice.md` entry 19) directly: there's a single lattice, a monotone operator, a fixpoint to compute. *That's not AI*. AI specifically means: *two* lattices related by a Galois connection, with a soundness theorem connecting concrete and abstract behavior.

**Verdict.** The decoder is *just* a monotone score combiner on a chain lattice. It does NOT need the AI framework. The score-combination is sound by construction; no Galois connection is needed.

### 8.4 Where AI machinery WOULD apply

If multiple `Recognizer` paths produced candidate markings and the engine needed to compute "the most-precise sound over-approximation of the parse" by combining strict-and-decoder evidence — that would be a Galois-connection problem. Per `abstract-interp.md` §16 "Composition of abstract domains (reduced product)":

> The reduced product is the smallest abstract domain containing both `A₁` and `A₂` such that the combined α₁₂ and γ₁₂ are still a Galois connection.

This *might* be relevant for combining `StrictRecognizer` (a precise abstraction) and `DecoderRecognizer` (a fuzzy abstraction over the same concrete domain of mangled inputs). But per CLAUDE.md, marque uses simple **dispatch** (strict-first, decoder-fallback), not reduced product. If the dispatch ever changes to "compute both, pick the higher-confidence answer," that's still a fold, not a Galois product.

### 8.5 Verdict on §8

**(a) for the score lattice as a distributive chain lattice with monotone combiner.** Don't reach for AI machinery; the simpler theorem suffices.

**Recommendation.**
- The decoder is sound by construction. No formal change needed.
- If a future maintainer asks "is the decoder doing abstract interpretation?" — say no, it's a monotone fold on a chain lattice. AI requires two lattices and a Galois connection; marque has one lattice (the score) and a fold.

**No open questions** — the design is correct as-is.

---

## §9. Aggregated open questions

A consolidated list of unresolved decisions surfaced in §§2–8. Each is phrased so the user can answer with a yes/no or a multiple-choice.

### Q-2.6 (SupersessionSet table transitivity)

When the supersession table contains `(A, B)` and `(B, C)` and the input set is `{A, B, C}`, the current `apply_supersession` (`crates/scheme/src/builtins.rs:366–376`) drops `B` (because `A` is present) AND drops `C` (because `B` is in the **input**, not the **filtered** set). Verify: is this the intended behavior? If yes, document that supersession is *transitive in the input direction* but *non-transitive in the filtered direction*. If no, the user must transitively close the table.

**Default.** Verify against actual CAPCO use cases. If no supersession chain is in active use, defer the question.

### Q-2.25 (Galois connection between pivot types)

Is there a Galois connection between `ParsedAttrs<'src>`, `CanonicalAttrs`, and `ProjectedMarking`? Specifically: is `MarkingScheme::canonicalize` monotone in some natural order on `ParsedAttrs`?

**Default.** Defer until PR 3a/3c lands and the types settle. The Galois-connection framing is available if needed; not load-bearing today.

### Q-3.9 (PR 3b "single citation per rule" interpretation)

Does the PR 3b D13 acceptance criterion ("single CAPCO-§ citation per rule") mean (a) one citation per declarative entry (so the consolidated `Constraint::Conflicts` walker can hold many entries each with its own citation) or (b) one citation per `impl Rule` block (so the consolidated walker has a single citation that covers all entries, which it can't)?

**Default.** Pick (a) — the citation discipline is per-entry, not per-impl. The consolidated walker is one `impl Rule` that delegates to the catalog; the catalog entries each have their own citation. This is consistent with how `evaluate()` works today.

### Q-4.5-JOINT-implies-REL (verify CAPCO normativity)

Operationally, `//JOINT [class] [countries]//` appears to imply `REL TO ⊇ countries`. User experience with HUMINT-volume JOINT production reports approximately zero counterexamples, but cannot separate "CAPCO rule" from "I've only ever seen it this way." Before encoding `joint-implies-rel-to` as a closure rule (§4.7), verify against `[capco-2016]` §H.3 + §H.7 text.

**Default.** Defer encoding the closure rule until CAPCO normativity is confirmed. If verified, the rule slots into the closure catalog with `trigger: JOINT(countries) present` and `cone: REL TO ⊇ countries`; topological scheduler sequences it before `noforn-clears-rel-to` automatically via `writes: [rel_to]`.

### Q-4.6 (SciSet trait demotion to JoinSemilattice)

Demote `SciSet` from `Lattice` to a hypothetical `JoinSemilattice` trait once the trait surface supports the split, and tighten the doc comment at `crates/capco/src/lattice.rs:14–39` to state "meet is defined for trait-impl completeness; no CAPCO semantics; not invoked by any rule"?

**Default.** Yes. The previous Q-4.6a / Q-4.6b ("adopt fact-set order; rename 'not a lattice meet' language") are answered by §4.6: the meet is mathematically clean under the prefix-inclusion order but operationally unused. The right fix is a trait demotion rather than a doc gloss that legitimizes an operation CAPCO never invokes.

### Q-5.3 (FgiSet representable-but-unreachable state)

Refactor `FgiSet::Present { concealed: bool, countries: BTreeSet<...> }` to a three-variant enum that makes `concealed: true ⇒ countries: ∅` a type-system invariant?

**Default.** Yes — this aligns with the consolidated plan PR 2 (`FgiMarker::SourceConcealed | Acknowledged`) and removes a representable-but-unreachable state.

### Q-6.5 (PageRewrite monotonicity for Phase D/E)

When `joint-promotion` and `fgi-absorption` are implemented, formally verify monotonicity-and-inflationarity per-rewrite in the PR description?

**Default.** Yes — even though the scheduler doesn't *require* it, monotonicity unlocks confluence and reorderability arguments. If a rewrite is genuinely non-monotone, document it as a deterministic-order operation per `security-lattice.md` §6 Framing 3.

### Q-2.6 / Q-3.x (Constraint::Custom as a junk drawer)

Many CAPCO rules will collapse to `Constraint::Custom` because the predicate is n-ary (SIGMA ordering, CNWDI classification floor, JOINT-participants-in-REL-TO). Is the user comfortable with `Custom` becoming the dominant Constraint variant, or should the scheme catalog grow more named variants (e.g., `Constraint::Ordered`, `Constraint::Floor`)?

**Default.** Comfortable for now; revisit if `Custom` exceeds 2/3 of the catalog. The plan's Phase F (CUI as second scheme) will surface what additional dyadic variants are worth adding.

### Q-3.8 (Per-rule classification for unclear rules)

For each of E003 (`MisorderedBlocksRule`), E007/E008 (parser-error rules), C001 (corrections-map), E052 (REL TO duplicates) — what's the intended collapse target? Each requires a one-line classification before PR 3b can land in the 8–18 band.

**Default.** Offer the classifications proposed in §3.8 (each line gives a recommended target) and have the user accept/override per rule.

### Q-7 (Document NOFORN-clears-REL-TO framing)

Cite `security-lattice.md` §6 Framing 3 in the `noforn-clears-rel-to` doc comment, and `security-lattice.md` §7 in the REL TO category aggregation doc?

**Default.** Yes — citation prevents future maintainers from re-debating the framing.

### Q-Master (Adopt this consultant's verdict on §3.3a?)

The §3.3a equal-depth meet IS a lattice meet under the fact-set inclusion order (§4 above). Accept this verdict and update the doc?

**Default.** Yes — the alternative ("the meet is one of three reasonable interpretations, none of which is canonical") is harder to defend and harder to reason about. Pinning the order makes the algebra unambiguous.

### Q-3.4.2 (Family-predicate Conflicts variant)

Should `Constraint::Conflicts` gain an `RhsFamily(predicate)` variant, with the §3.4.2 RELIDO entries (FD&R-family + non-US-family) as the first consumer? Or land the enumerated form (~15–20 single-token entries) in PR 3b and migrate later?

**Default.** Family-predicate (per user's verdict on the structural map's revision). PR 3b's acceptance criteria should call out the engine-PR dependency for the variant addition.

### Q-3.4.5a (RELOPT auto-collapse step bound)

What's the safe step bound `N` for the auto-collapse style rule (§3.4.5 (b))? The fixpoint is non-monotone in any obvious sense; a step bound is the simplest termination guarantee.

**Default.** `N = 3`, validated against a corpus of multi-page CAPCO-formatted documents. If the bound is hit, leave the page in its last consistent state and emit a diagnostic.

### Q-3.4.5b (RELOPT width-monotone projection)

Is the "which portions are abbreviated" projection actually monotone? If yes, abstract-interpretation widening (`abstract-interp.md` §7) could replace the step bound with a theoretically clean termination guarantee.

**Default.** Probably not (the consultant's read: page-reflow coupling makes the projection non-monotone). Defer to a future consultation; v1 ships with the step bound.

### Q-3.4.5c (EYES-alone historical recognition)

Should the parser auto-correct `EYES`-alone-without-list to `REL TO USA, FVEY` (per the FVEY-implicit historical case), or emit a diagnostic asking the user to confirm?

**Default.** Auto-correct, severity-overridable per §3.0.b. The historical case is well-defined enough to act on; users who want stricter behavior can override the severity.

### Q-3.4.6a (Class-floor catalog source)

Should the §3.4.6 per-token classification floors be generated at build time from CVE/Schematron metadata, or hand-curated in `marque-capco` against CAPCO-2016 §H?

**Default.** Build-time generation if the ODNI XML carries this data uniformly (Constitution Principle IV preference); hand-curated with explicit citations as a verified fallback. Investigation of the actual CVE field coverage required before committing.

### Q-3.4.6b (Unknown-floor diagnostic)

For passthrough markings (BUR / KLM / MVL / HCS-X) with provisional `C` floor, should the engine emit a diagnostic flagging "passthrough marking with unknown specific floor; verify with current ODNI manual"?

**Default.** Yes, severity-overridable per §3.0.b.

### Q-3.7-NNPI (NNPI propagation hypothesis)

NNPI behavior beyond FOUO eviction is bounded by two hypotheses: (#1) always-conveys (LES-like, equity-preservation); (#2) unclassified-only (UCNI-like). Mirror RAWFISA framing: record both, suggest hypothesis #1 as the most likely.

**Default.** Record both, do not commit. Default suggestion when the engine encounters NNPI: "behavior consistent with equity-preservation (analogous to LES); user should verify with current ODNI manual." Severity / behavior toggle exposed via configuration. The lattice shape under hypothesis #1 (always-conveys, absorbing in non-IC dissem) and hypothesis #2 (unclassified-only with `Constraint::Conflicts(NNPI, ClassLevel ⊒ C)`) differ — so the recommendation is shipped behind a config flag rather than wired into the lattice directly.

### Q-4.7-Cl_supp (Single shared FD&R suppressor or per-row suppressors?)

Should the §4.7 closure operator's implicit-default trio implement with a single shared FD&R-presence predicate (the cleanest option), or per-row suppressors (more flexible)?

**Default.** Shared FD&R predicate as primary; per-row override available for future implications that need it. Consistent with the §4.7.1 table-design unification.

---

## §10. Recommended next moves

Given the (a)/(b)/(c) verdicts above, here are the moves the consultant would recommend the user make first to unblock PR 3b. Ordered by leverage.

### Move 1 (highest leverage): Document the §3.3a meet as a lattice meet under the fact-set order

**Why first.** This unblocks the team's confidence that `SciSet`/`SarSet` lattice impls are actually lattices, which unblocks the PR 4 (lattice impls) work, which is gated by PR 3.7 (the lattice §-resolution spike). PR 3b doesn't directly depend on this, but the team's confidence does.

**Action.**
1. Edit `crates/capco/src/lattice.rs:14–39` doc comment per §4.6 above.
2. Update `2026-04-19-recursive-lattice-and-decoder.md:313–339` to clarify that "not unique" was a statement about *order choice*, not about *meet underdetermination given an order*.
3. Add a citation reference: "see `pure-lattice.md` §6 (Birkhoff's representation theorem on down-sets)" — the fact-set order is the down-set order on the SCI-tree poset.

**Justifying entries.** `pure-lattice.md` §3, §6, §15. `security-lattice.md` §18.

**Estimated cost.** 1 hour. No code changes; documentation only.

### Move 2: Classify the unclear rules in §3.8

**Why second.** PR 3b cannot land in the 8–18 band without a per-rule decision for E003, E007, E008, C001, E052 and similar. Each is a one-line "this rule's collapse target is X" decision.

**Action.** Walk the §3.8 list with the user; for each rule, accept the recommended target or override. The output is a small table that lives in the PR 3b PR description.

**Estimated cost.** 30 minutes if the user is available; longer if back-and-forth is needed.

### Move 3: Convert all banner-roll-up rules to lattice property tests

**Why third.** Removing 5–7 rules from the source-of-truth count makes the 8–18 band achievable. The mechanism is to delete the runtime rule and add a `proptest_lattice.rs` entry that exercises `Engine::project(Scope::Page, ...)` for each category.

**Action.**
1. For each of E031, E035, E040, E045 (and the other banner-roll-up rules), verify that PR 4's lattice impl will produce the same banner as the rule expects.
2. Move the rule's positive test cases to `proptest_lattice.rs`.
3. Delete the rule's `impl Rule` block in PR 3b.

**Justifying entries.** `pure-lattice.md` §15 (lattice homomorphism — `project` is a homomorphism per category). `abstract-interp.md` §9 (monotone framework — banner roll-up is the monotone fold).

**Estimated cost.** 4–6 hours of mechanical refactoring + property test authoring per rule. Could be parallelized.

### Move 4 (parallel with 3): Consolidate SCI per-system rules (E042–E051) into a single generic walker

**Why.** Ten of the 49 rules live in `rules_sci_per_system.rs`. They are all of the form "for SCI control system X, validate the per-system constraints from CAPCO §H.4 row Y." Per the May 1 lattice plan §4 Open Question 2 ("per-system canonicalization"), these can collapse to a single `Constraint::Custom` per-system walker that dispatches via `evaluate_custom`.

**Action.**
1. Build a per-system constraint catalog (data-only, in `crates/capco/src/scheme.rs`).
2. Wire `evaluate_custom("sci-per-system", &marking)` to walk the catalog.
3. Delete the 10 individual `impl Rule` blocks in PR 3b.

**Justifying entries.** `security-lattice.md` §18, §"When a security policy is NOT a lattice" §4.

**Estimated cost.** 1–2 days. The catalog is already partially extracted into `rules_sci_per_system.rs`; the work is to package it as `Constraint::Custom` data.

### Move 5: Surface Q-3.9 (citation-per-rule interpretation) before PR 3b implementation begins

**Why.** The PR 3b reviewer attestation requires "single CAPCO-§ citation per rule." If the consolidated walker is interpreted as one rule, it cannot satisfy single-citation. If the catalog entries are interpreted as the rules, single-citation is automatic.

**Action.** Open a brief PR (or amend the `specs/006-engine-rule-refactor/plan.md` D13 entry) clarifying the interpretation. Default to "single citation per declarative entry."

**Estimated cost.** 15 minutes for the clarification.

### Other moves (lower leverage, can defer)

- Cite `security-lattice.md` §6 Framing 3 + §7 in the marque code comments (Move 7 above). Cheap; pure documentation.
- Refactor `FgiSet::Present` to remove the representable-but-unreachable state (Q-5.3). Aligns with the consolidated plan PR 2; can land in that PR rather than ahead of PR 3b.
- Verify monotonicity of Phase D/E rewrites (Q-6.5). Defer until those rewrites are implemented.

### What NOT to do

1. **Do not invent new lattice constructors.** The 9 built-in constructors plus the 3 CAPCO structural impls cover the marque needs. Adding a new constructor (e.g., a "tetragraph-aware set lattice") should require a strong justification; per `pure-lattice.md` §11 the Product constructor handles most composition needs.

2. **Do not re-litigate the §3.3a "not unique" framing.** Per §4 above, the meet IS unique under the natural order. The "three reasonable choices" framing in the plan is misleading; once the order is documented, the meet is unique by construction.

3. **Do not reach for abstract-interpretation machinery.** The decoder is a monotone fold on a chain lattice; the page-rewrite scheduler is a deterministic-order rewrite system; the page projection is a per-category lattice fold. None of these need the Cousot-Cousot Galois-connection framework. Per `abstract-interp.md` "Diagnostic — When AI is the right framework": "AI specifically means *two* lattices related by a Galois connection. ... The marque page-rewrite scheduler ... is *just* Knaster-Tarski. Calling it 'abstract interpretation' overstates what's happening."

4. **Do not propose to make `SciSet` or `SarSet` `BoundedLattice`.** Per §5 above and the three converging catalog entries, this would be a category error. The current discipline is correct; defend it against future "engineering convenience" pressure.

---

## Summary of verdicts

| Construct | Verdict | Catalog citation |
|---|---|---|
| `Lattice` trait | (a) exact | `pure-lattice.md` §3 |
| `BoundedLattice` trait | (a) exact | `pure-lattice.md` §4 |
| `OrdMax`/`OrdMin` | (a) exact | `pure-lattice.md` §11 example |
| `FlatSet` | (a) exact | `pure-lattice.md` §3, §8 |
| `IntersectSet` | (a) exact (with naming caveat) | `pure-lattice.md` §13 |
| `SupersessionSet` | (b) partial — intra-category only | `security-lattice.md` §6 Framing 1 |
| `ModeSet` | (a) exact (re-derived) | `pure-lattice.md` §11 |
| `MaxDate` | (a) exact (with deferred extension) | `security-lattice.md` §8 |
| `OptionalSingleton` | (a) exact | `abstract-interp.md` §12 (related) |
| `Product` | (a) exact | `pure-lattice.md` §11 |
| `SciSet` | (a) join-semilattice; meet is trait-impl-only and operationally vacuous (§4.6) | `security-lattice.md` §18 |
| `SarSet` | (a) join-semilattice; same indestructibility shape as SciSet | `security-lattice.md` §19 |
| `FgiSet` | (b) — bounded distributive lattice | `security-lattice.md` §6 Framing 1 |
| `Category`/`AggregationOp`/`CategoryShape` | (c) not a lattice problem | — |
| `Scope` | (c) not a lattice problem | — |
| `PageRewrite` + scheduler | (a) for the scheduler (§18); (b) for the algebra | `abstract-interp.md` §18 + `security-lattice.md` §6 Framing 3 |
| `Constraint` | (c) not a lattice problem | `security-lattice.md` "Not a lattice" §4 |
| `MarkingScheme` | (c) signature, not algebra | — |
| `project()` | (a) per-category fold | `pure-lattice.md` §15 |
| §3.3a meet | (a) mathematically clean under fact-set order; (c) operationally — CAPCO has no meet semantics for SCI, only join (§4.2, §4.6) | `pure-lattice.md` §6 (Birkhoff) |
| Open-vocab no-top | (a) exact across 3 catalog entries | pure §4 + sec §18 + univ §11 Axis F |
| NOFORN clears REL TO | (a) exact | `security-lattice.md` §6 Framing 3 + §7 |
| Decoder confidence | (a) — monotone fold on chain lattice | `abstract-interp.md` §19 |

**Counts.**
- Constructs surveyed: 25
- (a) verdicts: 17
- (b) verdicts: 6
- (c) verdicts: 5 (`Category`/`Scope`/`Constraint`/`MarkingScheme` aren't lattice problems; `MaxDate` extension is partially (c) for exemption codes)
- Open questions surfaced: 10 (Q-2.6, Q-2.25, Q-3.8, Q-3.9, Q-4.5-JOINT-implies-REL, Q-4.6, Q-5.3, Q-6.5, Q-Master, plus the deferred composite Constraint::Custom volume question)

---

*End of bridge.*
