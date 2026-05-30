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

## Cross-scheme translation & coherence (Phase E, #641 T1-7)

```rust
pub trait Translate<A: MarkingScheme, B: MarkingScheme>: Send + Sync {
    fn translate(&self, from: &A::Canonical) -> Option<TranslationProposal<B>>;
    fn coherence_check(&self, a: &A::Canonical, b: &B::Canonical, ctx: &CoherenceContext)
        -> Vec<CoherenceDiagnostic>;
}
pub trait CoherenceRule<A: MarkingScheme, B: MarkingScheme>: Send + Sync {
    fn check_coherence(&self, a: &A::Canonical, b: &B::Canonical, ctx: &CoherenceContext)
        -> Vec<CoherenceDiagnostic>;
}

pub struct MultiGrammarEngine { /* Vec<Engine<_>> erased, translator registry */ }
//  - runs each grammar's single-scheme rules independently,
//  - then runs coherence rules over the joint result,
//  - routes a candidate to the grammar that claims it (D8 ownership routing).
```

`Diagnostic` is grammar-erased (or tagged) for multi-scheme `LintResult`/`FixResult` (T2-1/T2-2).

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

**Worked example** (memo; IC-side authority CAPCO-2016 §H.7/§H.8; CUI-side source-pending):
`CUI//FEDCON` + `C//RELIDO` → banner `CONFIDENTIAL//NOFORN` + CUI block `LDC: FEDCON`.

## #128 unification

The "second banner line" caveats (NOCON, ATTORNEY-CLIENT, PRIVACY ACT, …) are the CUI `LDC` value
set plus a few. `#128` and CUI commingling are **one** modeling problem — the releasability-escrow
surface (`CaveatLayer` artifact, data-model.md) — whether or not a full CUI block is present. The
caveat vocabulary is **source-pending**.

## Constitution VII guard

`marque-scheme` gains only `Translate`/`CoherenceRule` trait surfaces + the existing `Product`
constructor — no domain vocabulary, no `marque-ism` dependency. `CuiReleasability` and the
FEDCON⇒NOFORN closure are declared in the domain crates / engine, not the leaf. A future
`marque-cui` crate sits **alongside** `marque-ism` as a peer foundation (Constitution VII), not
below it.
