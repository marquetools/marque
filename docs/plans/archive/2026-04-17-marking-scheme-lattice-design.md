# Marque: Marking-Scheme Lattice Design

**Date**: 2026-04-17
**Status**: archived — §§3–5 superseded by `2026-04-19-recursive-lattice-and-decoder.md` (trait surface, probabilistic disambiguation, migration sequence). §§0–2 (problem statement, core algebra, Phase A scaffolding) carry forward via the 04-19 plan and remain authoritative. Implementation landed across Phases A, B, and Phase 3 of `specs/004-constraints-decoder-vocab/`. Kept for historical context.
**Branch**: `claude/marking-scheme-lattice-design-FzkPx`

This document consolidates four prior design discussions and establishes the
authoritative shape of the abstraction that makes marque a general-purpose
rule engine over arbitrary structured marking systems. The originals remain
as historical context and are not being deleted:

- `docs/plans/2026-03-11-marque-design.md` — original CAPCO-centric design
  (pipeline, crate graph, rule architecture). Everything below the
  trait-and-lattice layer still applies.
- `docs/plans/2026-04-16-probabilistic-recognition.md` — three-layer
  recognition model + empirical corpus base rates from Enron. The
  numbers are authoritative; the architectural framing is recast here.
- `docs/plans/vocabulary-provider-domain-notes.md` — CAPCO constraints
  that any abstraction must be expressive enough to carry. All items
  flagged there are addressed below.
- `docs/plans/vocabulary-provider-signal-model.md` — signal-codebook
  metaphor. Demoted: we keep "projection is lossy compression" as a
  useful intuition and drop the Fourier / SNR / matched-filter framings
  from the architecture. Those were reaching, and the grammar-plus-
  lattice framing covers the same ground with less machinery.
- `docs/plans/vocabulary-provider-sketch.rs` — early trait sketch. The
  `marque-scheme` crate landed in this PR is the evolved form.

---

## 1. Problem statement

CAPCO semantics are hardcoded in four places today:

| Location | What's baked in |
|----------|-----------------|
| `marque-ism::attrs::IsmAttributes` | The pivot struct has CAPCO-shaped fields: `sci_controls`, `sar_markings`, `aea_markings`, `rel_to: Box<[Trigraph]>`, etc. |
| `marque-ism::page_context::PageContext` | Hand-written aggregation — `max()` for classification, union for SCI, intersection for REL TO, max-date for declassify-on, SAR sort key. |
| `marque-capco::rules` | 39 rules as hand-coded `impl Rule`. Many encode constraints that are structurally identical across marking systems (NOFORN∥REL TO, RD⇒NOFORN, JOINT∥FGI). |
| `marque-core::parser` | Structural parser with CAPCO grammar knowledge (`parse_sci_block`, `parse_sar_category`, etc.). |

There is no interface to accept anything that isn't CAPCO-shaped. CUI,
NATO, and future corporate/medical schemes have no home. A bug-fix in
one aggregation rule is a search-and-replace across both `PageContext`
and the rule set. This PR lays the foundation to fix both without
tearing up the working CAPCO stack.

## 2. Core abstraction

A marking system is a **typed algebra over a bounded lattice**, with a
constraint predicate and a lossy projection operator, plus local
probabilistic disambiguation at a small number of decision points.

That one sentence contains five claims:

1. **Bounded lattice.** Classification levels form a partially ordered
   set where every pair has a unique least upper bound (*join*, `⊔`) and
   greatest lower bound (*meet*, `⊓`). Bounded means there's a top (⊤)
   and a bottom (⊥). ⊥ = UNCLASSIFIED; ⊤ = TOP SECRET (or the scheme's
   ceiling); `join` is "the most restrictive classification that
   dominates both inputs." This is the literal mathematical structure
   classification levels obey — not a metaphor (Denning 1976). Schemes
   that aren't strictly linear (CUI categories) still fit; a lattice
   doesn't have to be a chain.

2. **Typed.** A marking has internal structure: classification, SCI
   controls, dissemination controls, FGI, REL TO, declassify-on, ....
   Each category is its own mini-lattice with its own join operator.
   A full marking is the *product lattice* over these categories, with
   component-wise join.

3. **Constraints.** Not every point in the product lattice is a valid
   marking. NOFORN and REL TO are mutually exclusive. HCS requires
   NOFORN. The constraint set is a predicate `valid: Marking → bool`
   applied after any join so combinations that produce invalid markings
   surface as diagnostics.

4. **Projection.** Portion → banner is a lossy projection: given a set
   of portion markings, produce a banner marking. Lossy because the
   portion structure is not recoverable from the banner. This is the
   single design move that operationalizes "it's an encoding" — the
   projection is an explicit, swappable component of the scheme, so a
   corporate sensitivity scheme can use mode instead of max, and a
   medical scheme handling multiple PHI categories can union on
   everything.

5. **Local probabilistic disambiguation.** The grammar is deterministic
   for >99% of real input (Enron: 58 of 93 CAPCO tokens are marking-
   exclusive; `(X//Y)` where both are known tokens has 0 false positives
   in 22M lines). Ambiguity is local to a small number of decision
   points — `(C)` being the canonical case. Those get Bayesian log-odds
   over hand-engineered evidence features, not a full PCFG.

## 3. Trait surface (`marque-scheme`)

The `marque-scheme` crate is domain-neutral: zero runtime deps, no
dependency on `marque-ism` or `marque-capco`. It defines:

```rust
pub trait Lattice: Sized + Clone + Eq {
    fn join(&self, other: &Self) -> Self;   // least upper bound
    fn meet(&self, other: &Self) -> Self;   // greatest lower bound
}
pub trait BoundedLattice: Lattice {
    fn top() -> Self;
    fn bottom() -> Self;
}

pub trait MarkingScheme {
    type Token;
    type Marking: Lattice;
    type ParseError;

    fn name(&self) -> &str;
    fn schema_version(&self) -> &str;

    fn categories(&self) -> &[Category];
    fn constraints(&self) -> &[Constraint];
    fn templates(&self) -> &[Template];

    fn parse(&self, input: &str) -> Result<Parsed<Self::Marking>, Self::ParseError>;
    fn validate(&self, m: &Self::Marking) -> Vec<ConstraintViolation>;
    fn project_banner(&self, portions: &[Self::Marking]) -> Self::Marking;

    fn render_portion(&self, m: &Self::Marking) -> String;
    fn render_banner(&self, m: &Self::Marking) -> String;
}
```

Supporting types (`Category`, `AggregationOp`, `Constraint`, `Template`,
`Parsed`, `Candidate`, `EvidenceFeature`) live in sibling modules. See
`crates/scheme/src/` for the authoritative definitions. The rest
of this section is the conceptual tour.

### 3.1 `Category` and `AggregationOp`

Each category (classification, SCI, dissem, REL TO, declassify-on)
declares its aggregation operator. The operator is the per-category
join used during `project_banner`:

| Operator | Use | Lattice |
|----------|-----|---------|
| `Max` | Classification level | Total order U<R<C<S<TS |
| `Union` | SCI, SAR, AEA, dissem (before supersession) | Powerset by set inclusion |
| `Intersect` | REL TO country list | Powerset by reverse inclusion |
| `UnionWithSupersession` | Dissem with NOFORN⊐REL-TO supersession | Union quotiented by the supersession relation |
| `MaxDate` | Declassify-on | Total order on dates |
| `Mode` | Future: corporate sensitivity (pick the most common) | Deferred — surfaces in CUI-adjacent designs |
| `Custom` | Escape hatch | Caller-defined |

### 3.2 Non-obvious cases from the domain notes

The domain-notes document flagged several CAPCO constraints that any
abstraction must be expressive enough to carry. Each is addressed here:

- **REL TO tetragraph intersection (FVEY ∩ NATO).** Tetragraphs
  (FVEY, NATO, ACGU, ...) are compositional: they stand for country
  sets. Intersection happens *at the country level*, not the tetragraph
  level, because FVEY ∩ NATO is neither FVEY (loses AUS, NZL) nor NATO
  (loses 25+ non-Five-Eyes members). The `Category` carries an optional
  `expansion: ExpansionFn` that expands composite tokens to atomic
  tokens before the aggregation operator runs. Re-compression back to
  a tetragraph (when the result happens to match one) is the scheme's
  `render_banner` job.

- **Intra-category ordering (USA-first, then alphabetical).** Distinct
  from the `ordering_rank` that orders one category relative to others.
  `IntraOrdering::FixedFirst { first: "USA", then: Alphabetical }`
  captures REL TO's rule. Composes over tetragraph expansion.

- **Context-dependent tokens (bare `REL`).** `REL` without a country
  list defers its full meaning to the page/document context. The
  scheme expresses this by flagging the token as `ContextDependent`; the
  engine resolves it during `project_banner` by reading the enclosing
  page's REL TO list. A token with `ContextDependent = true` is a
  parse-time placeholder that aggregation must resolve.

- **Position-dependent token semantics (trigraphs as non-US class source
  vs REL TO target vs FGI indicator).** A trigraph is a token in
  several categories at once; the *structural template* (portion vs
  banner, wrapping, prefix) disambiguates. The `Template` struct
  records per-position category presence, so the parser can resolve
  `GBR` to FGI-source in one position and REL-TO-target in another.

- **Document-level requirements (FISA notice).** Token presence can
  trigger document-wide obligations beyond the marking itself. These
  are out of the lattice — they land in `MarkingScheme::validate`
  with a `ConstraintViolation::DocumentRequirement` variant. The
  engine surfaces them at the end of the document pass, not per-
  marking.

### 3.3 `Constraint`

A declarative list of dyadic and n-ary invariants the scheme enforces:

```rust
pub enum Constraint {
    Conflicts(TokenRef, TokenRef),   // NOFORN ∦ REL TO
    Requires(TokenRef, TokenRef),    // HCS ⇒ NOFORN
    Implies(TokenRef, TokenRef),     // RD ⇒ NOFORN (default)
    Supersedes(TokenRef, TokenRef),  // NOFORN ⊐ REL TO at banner level
    Custom(&'static str),            // label; scheme's validate() dispatches it
}
```

The dyadic variants are fully evaluable by a generic engine that only
knows how to check token/category presence. `Custom` is the escape
hatch: its payload is a stable label, and the scheme's
`MarkingScheme::validate` implementation matches on the label and runs
the scheme-specific predicate. CAPCO has a handful of rules that can't
be expressed as a binary relation (SIGMA ordering requires numeric
sort; CNWDI requires classification ≥ S). Those land as `Custom`. The
design goal is that most of the 39 CAPCO rules become dyadic
`Constraint` data; a minority stay as `Custom` labels whose predicates
live in the scheme. This PR doesn't do the migration — it proves the
expressiveness on a three-constraint sample.

A future alternative variant (e.g. `DynCustom(Arc<dyn Fn(&Marking) ->
Vec<ConstraintViolation>>)`) can be added alongside when an
engine-side generic evaluator becomes useful. Phase A keeps the
`'static`-friendly shape so `constraints()` can return `&[Constraint]`
without lifetime gymnastics.

## 4. Probabilistic disambiguation

The engine is deterministic for the overwhelming majority of input. The
Enron corpus analysis (510K docs, 134M words) shows:

- 58 of 93 CAPCO tokens are effectively marking-exclusive.
- `(X//Y)` where both tokens are known CAPCO vocabulary has zero false
  positives across 22M lines.
- Two or more CAPCO tokens adjacent near `//` is overwhelming marking
  evidence.

Ambiguity is local to specific productions. The `(C)` case is the
paradigm: it could be a CONFIDENTIAL portion marking or a copyright
symbol, with completely opposite escalation implications. Enron has
4,766 copyright `(C)` and 0 marking `(C)`.

The abstraction:

```rust
pub enum Parsed<M> {
    Unambiguous(M),
    Ambiguous { candidates: Vec<Candidate<M>> },
}
pub struct Candidate<M> {
    pub marking: M,
    pub evidence: Vec<EvidenceFeature>,
    pub prior_log_odds: f32,
}
```

The parser emits `Ambiguous` *only* at enumerated decision points. The
resolver — a separate, pluggable pass not included in this PR — combines
evidence features (nearby year → copyright, document has higher-
classified portions → collateral, `(C)` clusters near list markers →
decorative) into a log-odds score and either resolves to one candidate
above a confidence threshold or surfaces the ambiguity to the user as a
verification request.

This is Bayesian inference with hand-engineered features, not a neural
model. Explainable by construction: every resolution decomposes as a
sum of evidence contributions. That matters because the deployment
environment (classified-document workflows) requires the tool to
explain why it took an action.

The corpus-derived base rates that drive the priors live in
`tools/corpus-analysis/output/enron-full.json` (already generated in a
prior session). Wiring them into the resolver is a later-phase task.

## 5. Migration sequence

```
Phase A (this PR)
  ├── docs/plans/2026-04-17-marking-scheme-lattice-design.md
  ├── crates/scheme/   (trait + lattice + data types, no adopters)
  ├── crates/capco/src/scheme.rs   (CapcoScheme adapter)
  └── equivalence tests: CapcoScheme agrees with PageContext
Phase B
  └── Replace PageContext internals with scheme-driven aggregation.
      Public API of PageContext stays stable; existing rules unchanged.
      Prove equivalence on the full corpus fixture set.
Phase C
  └── Move declarative CAPCO constraints (NOFORN/REL TO, RD/NOFORN,
      JOINT constraints, HCS/NOFORN, ...) to Constraint data. Each
      corresponding hand-written rule becomes a thin iteration over the
      scheme's constraints and an emitter of Diagnostics. Retire the
      one-rule-one-file pattern for constraint-style rules.
Phase D
  └── Implement CUI as a second MarkingScheme to validate genericity.
      NARAs CUI has ~125 categories; many combine more freely than
      CAPCO's. If CUI exposes expressiveness gaps, iterate the trait.
Phase E
  └── Wire the fuzzy token resolver (the marque-detect work described
      in 2026-04-16-probabilistic-recognition.md) behind the scheme
      trait so it works for every scheme, not just CAPCO.
```

Each phase is a self-contained PR. Phase B is the most invasive (it
touches every page-context consumer) and is gated on Phase A landing
with green equivalence tests.

## 6. Explicitly deferred decisions

These are real open questions that this PR does not try to resolve:

- **Severity × confidence interaction.** A rule at `severity: fix` with
  region confidence 0.7 — auto-apply? Downgrade to warn? Product of
  fix confidence × region confidence against a single threshold? The
  corpus analysis needs to land first so we know the empirical spread.

- **Edit-distance cutoff for fuzzy token resolution.** Distance 1
  catches most typos; distance 2 catches severe ones at the cost of
  false positives. The answer depends on per-token length and base
  rate — short, high-base-rate tokens need tighter cutoffs.

- **Baked-in vs runtime frequency tables.** Baked into the WASM binary
  is simpler; runtime-loaded allows per-deployment customization.
  Probably baked-in with an override path, but this doesn't need to be
  decided in Phase A.

- **How `Parsed::Ambiguous` interacts with auto-fix.** Never auto-fix
  an ambiguous parse; always surface as a verification request. The
  trait supports this today (`Parsed::Ambiguous` is a distinct variant);
  what's deferred is the *UI* for verification requests, which depends
  on how CLI, server, and Office add-ins each want to surface them.

## 7. What we dropped

The signal-codebook framing in `vocabulary-provider-signal-model.md`
was a useful thinking aid: the banner-from-portions projection is a
lossy compression over independent category channels. We keep that as
motivation. We *drop* from the architecture:

- Fourier/frequency-domain analogies. They don't fit — the categories
  are not basis functions of an inner-product space.
- SNR framings. The signal-in-noise intuition is correct but the
  algebraic framing (markings = lattice points, noise = non-marking
  text with high per-token false-positive rate) carries the idea
  without the metaphor.
- Matched-filter / correlation-detector vocabulary. The real
  mechanism — exact-match fast path in an Aho-Corasick automaton with
  a fuzzy fallback gated on region confidence — is already what the
  existing scanner/parser do. No new signal-processing primitives are
  needed.

These framings were genuinely clarifying at the discussion stage; they
just don't belong in the trait design.

## 8. Appendix: unit-test matrix

The `marque-scheme` crate carries property-style tests for the lattice
and aggregation primitives:

- `Lattice` laws on a four-element classification lattice
  (U < C < S < TS): idempotency (`a ⊔ a = a`), commutativity
  (`a ⊔ b = b ⊔ a`), associativity (`(a ⊔ b) ⊔ c = a ⊔ (b ⊔ c)`),
  absorption (`a ⊔ (a ⊓ b) = a`).
- `BoundedLattice`: `top ⊔ a = top`, `bottom ⊔ a = a`.
- `AggregationOp::Max` reduces mixed slices to the peak.
- `AggregationOp::Union` deduplicates.
- `AggregationOp::Intersect` returns the common subset; empty on
  disjoint inputs.
- `AggregationOp::UnionWithSupersession` drops superseded entries when
  the superseding token is present.

The adapter in `marque-capco` carries equivalence tests that compare
`CapcoScheme::project_banner` output against `PageContext`'s derived
banner characteristics on the same fixture inputs. Those tests are the
acceptance criterion for the abstraction: if the existing CAPCO
behavior falls out of the trait unchanged, the abstraction is right.
