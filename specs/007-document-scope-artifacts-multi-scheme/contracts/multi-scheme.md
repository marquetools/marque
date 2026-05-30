# Contract: Multi-Scheme Co-Residence (#641, #128)

Trait surface for two grammars co-resident on one document. **CUI specifics are source-pending**;
this contract fixes only the domain-neutral shape. Reconciliation lives in `marque-engine`
(model b, research D6/D7); `marque-scheme` stays the leaf.

## Generification prerequisites (Phase B, #641 Tier 1/2)

```rust
// T1-1/T1-2: Rule input is the scheme's canonical, not the ISM concrete type.
trait Rule<S: MarkingScheme> {
    fn check(&self, attrs: &S::Canonical, ctx: &RuleContext<'_, S>) -> Vec<Diagnostic<S>>;
}
// T1-3: Engine actually uses S (no silent drop(scheme)).
struct Engine<S: MarkingScheme> { /* rule_sets, scheme: S, scheduler order, ... */ }
```

## Object-safe scheme erasure (Phase B prerequisite — C2)

`MarkingScheme` has associated types (`Marking`, `Canonical`, `Fact`, `ArtifactPayload`, …) and is
therefore **not object-safe** — `Vec<Box<dyn MarkingScheme>>` / `Vec<Engine<S>>` over
heterogeneous `S` does not compile. Co-residence requires heterogeneous schemes in one container,
so an **object-safe shim** is the load-bearing design (this is the actual hard problem of
multi-scheme, called out as a Phase-B-blocking design, not an implementation detail):

```rust
// Object-safe façade over a concrete Engine<S>. Erases every associated type to bytes / a
// grammar-erased Diagnostic. The concrete S re-emerges only inside each impl.
pub trait ErasedEngine: Send + Sync {
    fn grammar_id(&self) -> &'static str;
    fn lint_erased(&self, input: &[u8], ctx: &InputContext<'_>) -> ErasedLintResult;
    fn resolve_erased(&self, input: &[u8], ctx: &InputContext<'_>) -> ErasedResolved;
    fn claims(&self, candidate: &[u8]) -> Claim;   // D8 ownership routing (Accepts / Rejects)
}
// Blanket impl for every Engine<S>: impl<S: MarkingScheme> ErasedEngine for Engine<S> { … }

pub struct MultiGrammarEngine {
    engines: Vec<Box<dyn ErasedEngine>>,           // heterogeneous schemes, object-safe
    coherence: CoherenceRegistry,                  // CoherenceRule<A,B> pairs, type-erased at the edge
}
//  - runs each grammar's single-scheme rules independently (lint_erased),
//  - routes a candidate to the grammar that claims() it (D8 ownership routing),
//  - then runs coherence rules over the joint result + document-scope releasability (model b).
```

`ErasedLintResult`/`ErasedResolved` carry a grammar-erased (tagged) `Diagnostic` for multi-scheme
`LintResult`/`FixResult` (T2-1/T2-2). The grammar tag re-associates a diagnostic with its scheme
for rendering.

`Translate<A, B>` is **NOT in this contract** — cut from the feature (research D7; tracked as
**#829**, blocker for ISM→DoD XML). Model (b)
reconciles two per-scheme lattices via `Product`+closure and never translates one canonical into
the other's, so co-residence needs only `CoherenceRule`:

```rust
pub trait CoherenceRule<A: MarkingScheme, B: MarkingScheme>: Send + Sync {
    fn check_coherence(&self, a: &A::Canonical, b: &B::Canonical, ctx: &CoherenceContext)
        -> Vec<CoherenceDiagnostic>;
}
```

## Two-scope reconciliation (Phase E)

**Scope 1 — portion ownership routing (D8, relocate-not-evict)**:
```
strict fails under both grammars
  → decoder surfaces token set, e.g. {S, CUI}
  → engine recognizes the set spans mutually-exclusive grammars
  → emits portion-scope cross-grammar conflict (error), NO auto-fix
  → suggestion: relocate the CUI signal to document scope (human-confirmed)
INVARIANT: before junk-recovery consumes a token a scheme rejects, the engine asks whether a
co-active scheme claims it. No silent marking loss (SC-004).
```

**Scope 2 — document releasability (D6, research-verified)**:
```
Releasability = Product<CuiReleasability, CapcoIcDissem>  joined COMPONENTWISE
              + monotone cross-component closure: (CUI carries non-IC control) ⇒ inject NOFORN into IC component

- Combine = JOIN (LUB / most-restrictive floor), NOT meet.            [pure-lattice §11; security-lattice §6/§7]
- FEDCON⇒NOFORN floor = cross-axis CLOSURE rule; obligation = MONOTONICITY (not meet-laws). [marque-applied §4.7; pure-lattice §18]
  It is CLOSURE_NOFORN_NONICCONTROLS lifted across schemes; additive, never removes ⇒ Kleene-monotone.
- Each regime RENDERS its own projection (a render, not a lattice op):
    classified banner → IC-expressible floor (NOFORN / REL TO / RELIDO)
    CUI block         → precise LDC (FEDCON / FED ONLY / NOCON / DL ONLY / DISPLAY ONLY / …)
- Escrow: the same attribute appears on BOTH surfaces; the CUI block preserves what the banner strips.
  (Banner side is lossy ⇒ a single shared scalar is forbidden; Product forces two retained components.)
```

**Lattice shape (LV4, research D6/D12)**: the `Product` implements `JoinSemilattice` only — plus
`BoundedJoinSemilattice` via the two factors' bottoms. It MUST NOT implement `BoundedLattice`:
`CuiReleasability`'s LDC set is agency-extensible/open (no lawful finite top), exactly as
`SciSet`/`SarSet`. The additive monotone NOFORN closure converges without a top. No lattice-trait
surface change.

**Worked example** (memo; IC-side authority CAPCO-2016 §H.8 — Dissemination Control Markings, RELIDO p154 / NOFORN p145 / REL TO p150; CUI-side source-pending):
`CUI//FEDCON` + `C//RELIDO` → banner `CONFIDENTIAL//NOFORN` + CUI block `LDC: FEDCON`. **Validated
in Phase E against a synthetic `StubScheme`** (an invented non-IC control), NOT a real CUI grammar
— the test asserts the `Product`+closure *mechanism*, never the unverified FEDCON⇒NOFORN mapping
(FR-026; encoding it would violate Constitution VIII).

## #128 unification

The "second banner line" caveats (NOCON, ATTORNEY-CLIENT, PRIVACY ACT, …) are the CUI `LDC` value
set plus a few. `#128` and CUI commingling are **one** modeling problem — the releasability-escrow
surface (`CaveatLayer` artifact, data-model.md) — whether or not a full CUI block is present. The
caveat vocabulary is **source-pending**.

## Constitution VII guard

`marque-scheme` gains only the `CoherenceRule` trait surface + the existing `Product`
constructor — no domain vocabulary, no `marque-ism` dependency, and **no new lattice-trait
surface** (the releasability `Product` uses the existing `JoinSemilattice`/`Product` machinery;
LV4). The `ErasedScheme`/`ErasedEngine` shim and `MultiGrammarEngine` live in `marque-engine`
(model b), not the leaf. `CuiReleasability` and the FEDCON⇒NOFORN closure are declared in the
domain crates / engine, not the leaf. A future `marque-cui` crate sits **alongside** `marque-ism`
as a peer foundation (Constitution VII), not below it.
