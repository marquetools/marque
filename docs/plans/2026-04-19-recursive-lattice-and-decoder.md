<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Marque: recursive lattices, lossless tokens, and the probabilistic decoder

**Date:** 2026-04-19
**Status:** proposed — supersedes §5 (Migration sequence) of
`2026-04-17-marking-scheme-lattice-design.md` and reframes §3 (Trait
surface) and §4 (Probabilistic disambiguation).
**Builds on:** `2026-04-17-marking-scheme-lattice-design.md` —
problem statement, core algebra, and phase A scaffolding are unchanged.

## 0. Why another design doc

The 2026-04-17 doc settled the math: a marking system is a typed algebra
over a bounded lattice, with a constraint predicate and a lossy
projection. Phase A shipped the scaffolding and proved equivalence with
the CAPCO `PageContext` on a subset of categories.

Working through what Phase B should actually *be* surfaced three gaps
the prior doc glossed over:

1. The `AggregationOp` enum is the wrong abstraction for the long tail.
   CAPCO already needs operator variants that don't generalize (FGI
   concealment supersession, classification-conditional FOUO drop,
   "most specific" for declass exemptions), and CUI will add more. The
   right primitive is the lattice itself, composed recursively; the
   operator enum is at best a shorthand for the common cases.
2. Token metadata is richer than `TokenId(u32)` admits. The ODNI ISM
   XML carries owner/producer, authority, source, deprecation, point
   of contact, schema version, and human-readable descriptions for
   every term. Losing that metadata is losing marque's claim to being
   authoritative. The trait needs a vocabulary surface that preserves
   it losslessly.
3. The probabilistic work was scoped as a future phase addressing one
   production (`(C)` vs copyright). It's actually the second half of
   the recognizer — one that handles severely mangled markings
   (`S noforn SI,fouo` → `(S//SI//NF)`) via Bayesian decoding from a
   token bag. That capability is table-stakes for batch cleanup over
   historical corpora and should ship alongside the strict parser,
   not years later.

This doc captures the design goals explicitly, reframes the
architecture around those three gaps, and revises the phase sequence
accordingly.

## 1. Design goals

Numbered for citability in later docs, PRs, and code review.

- **G1. Grammar-agnostic engine.** The engine crate (`marque-engine`)
  knows about `MarkingScheme` and nothing else. Every CAPCO/ISM detail
  lives in the `marque-capco` adapter. A new scheme is a new adapter;
  no engine edits required.
- **G2. Low-ceremony scheme authoring.** A simple grammar (flat token
  sets, ordinal levels, dates) should be expressible in one screen of
  declarative data. Authors should not be forced to implement the
  lattice trait themselves for categories that don't need structure.
- **G3. High-ceiling scheme expressiveness.** CAPCO's structural
  categories (SCI compartment trees, SAR program hierarchies, FGI
  concealment) must fit inside the trait without escape-hatch
  `Custom(fn)` reducers. Whatever CAPCO needs, future schemes can
  adopt.
- **G4. Robust recognition of mangled markings.** When tokens are in
  the right neighborhood but the grammar is broken (wrong order,
  missing delimiters, incompatible tokens), marque should still
  resolve to the most likely correct marking when the corpus supports
  it. Clean input takes the fast path; dirty input takes a
  probabilistic decoder; both produce the same output shape.
- **G5. Provenance-tagged fixes.** Every auto-applied correction
  carries enough probabilistic context (posterior log-odds, runner-up
  ratio, feature contributions, base rates) for a downstream batch
  pipeline to raise or lower its confidence threshold and accept a
  known error rate.
- **G6. Perceptual performance.** p95 lint latency stays ≤16 ms on 10
  KB of text. The strict path runs at SIMD speed; the decoder runs
  only on candidate regions the strict path rejects or flags
  low-confidence.
- **G7. Honor the common case.** Most markings in real documents are
  correct or near-correct. The strict path handles them at full speed
  without invoking the decoder at all. The decoder is a fallback, not
  a default.
- **G8. Bidirectional operation.** The same engine serves (a)
  incremental authoring (banner/portion markings get validated and
  fixed as typed) and (b) batch reconciliation (banner checked
  against portions, historical markings mapped to current
  equivalents, reply markings diffed against parent email).
- **G9. Maximum automatic correction.** If marque has high posterior
  confidence that a region is a specific marking, it fixes it —
  without prompting. The audit trail is how we learn about edge cases
  where the prior mis-fired. A system that asks the user on every
  ambiguity is worse than a system that decides and explains.
- **G10. Proactive feedback.** Marque emits warnings for structural
  issues a human author would miss — reply weakening parent
  classification (REL → NOFORN between email and reply), banner that
  doesn't cover its portions, derivative classification missing source
  citations. These are two-marking diff rules on top of the lattice
  algebra.
- **G11. Style flexibility.** Banner abbreviation preference, zero-pad
  dates, REL vs RELEASABLE TO — style choices are rules-layer config,
  not engine assumptions. Default off or conservative; organizations
  opt in.
- **G12. Parallelism.** Batch mode uses all available cores with
  bounded backpressure. No rule crate is allowed to hold global state
  that blocks concurrent invocation.
- **G13. Content ignorance.** Marque retains only tokens, spans, and
  classifier identity. No document content crosses the engine
  boundary into audit records or telemetry. This is enforced by type
  discipline in `AppliedFix` and by convention elsewhere; any
  regression is a compliance bug.

Sub-goals that drop out of the above but are worth naming:

- **G1a.** The CAPCO corpus accuracy harness (SC-002/SC-003, ≥95% per
  rule) is preserved across every phase transition. Each phase's PR
  runs it as a gate.
- **G9a.** Fix confidence is a product of recognition confidence,
  rule confidence, and (optionally) region confidence. The interaction
  is explicit in code, not implicit in rule severity.

## 2. Architecture overview

```
  ┌─────────────────────────────────────────────────────────────┐
  │                   marque-engine (grammar-agnostic)           │
  │   ┌────────┐   ┌────────┐   ┌────────────┐   ┌────────────┐ │
  │   │ strict ├──►│decoder ├──►│  validate  ├──►│   apply    │ │
  │   │ parser │   │(probab.)│  │constraints │   │    fixes   │ │
  │   └────────┘   └────────┘   └────────────┘   └────────────┘ │
  │        ▲            ▲              ▲                ▲       │
  │        │            │              │                │       │
  │        ▼            ▼              ▼                ▼       │
  └───────────┬─────────────────────────────────────────────────┘
              │ MarkingScheme trait + Vocabulary + ControlBlock
              │
  ┌───────────┴─────────────────────────────────────────────────┐
  │          marque-scheme (trait + lattice primitives)          │
  │                                                               │
  │   trait Lattice / BoundedLattice        ◄── recursive        │
  │   built-in constructors: OrdMax, FlatSet, IntersectSet,       │
  │       MaxDate, SupersessionSet                                │
  │   trait Vocabulary<Token>                                     │
  │   trait MarkingScheme                                         │
  │   trait ControlBlock (separate from MarkingScheme)            │
  │   enum Parsed<M> { Unambiguous | Ambiguous { candidates } }   │
  └────────────────────────────┬──────────────────────────────────┘
                               │ implemented by
         ┌─────────────────────┼─────────────────────┐
         ▼                     ▼                     ▼
   marque-capco           (future) cui          (future) nato
   CapcoScheme            CuiScheme              NatoScheme
   CapcoCab               CuiCab                 NatoCab
```

Three orthogonal axes:

1. **Recognizer axis** (strict → decoder). Both produce `Parsed<M>`.
2. **Scheme axis** (CAPCO → CUI → NATO → ...). Each defines its
   lattice, vocabulary, constraints, templates, and control block.
3. **Scope axis** (portion → page → document → diff). Reductions
   parameterized by scope; the same scheme describes how markings
   compose at each level.

## 3. Recursive lattices with progressive disclosure

### 3.1 The principle

**Every category is a lattice.** Full stop. The engine's hot path
reduces a category by calling `Lattice::join` on its values.

**Authors pick their ceremony level.** A flat token set is one line
using a built-in constructor. A compartment tree is a struct with a
hand-written `impl Lattice`. Both plug into the same engine.

No operator enum sits between the category and the engine in the
runtime path. `AggregationOp` variants survive only as a *constructor
DSL* — shorthand that resolves to a built-in lattice type at scheme
build time. An author never writes "this category uses
`AggregationOp::UnionWithSupersession`"; they write a declarative
constructor like `union_with_supersession([(NOFORN, REL_TO)])` which
produces a `SupersessionSet<TokenId>` value implementing `Lattice`.
The enum is a facade on top of the trait, not a runtime dispatch
point.

### 3.2 Tier 1: built-in lattice constructors

Ships in `marque-scheme`. Covers probably 80% of category shapes
across CAPCO, CUI, NATO:

| Constructor              | Lattice shape                                | Use cases                    |
| ------------------------ | -------------------------------------------- | ---------------------------- |
| `OrdMax<T: Ord>`         | total order, join = `max`, meet = `min`      | classification ladder        |
| `OrdMin<T: Ord>`         | total order, join = `min`, meet = `max`      | "most specific" semantics    |
| `FlatSet<T>`             | powerset, join = union, meet = intersect     | SCI, SAR, AEA, dissem        |
| `IntersectSet<T>`        | inverted powerset, join = intersect          | REL TO (before expansion)    |
| `SupersessionSet<T>`     | union with post-filter by pair table         | NOFORN ⊐ REL TO              |
| `MaxDate`                | dates, join = later, bottom = `None`         | declassify-on                |
| `ModeSet<T>`             | multiset, join = most-frequent               | corporate sensitivity        |
| `OptionalSingleton<L>`   | lifts `L` to `Option<L>` with absent bottom  | optional single-value fields |
| `Product<L1, L2>`        | pairs, join pairwise                         | composing sub-lattices       |

Each is a generic struct with `impl Lattice` and, where meaningful,
`impl BoundedLattice`. A scheme author writes:

```rust
// Flat-set category in three lines, no lattice trait knowledge needed.
Category::flat_set(
    CategoryId(4), "dissem", ordering_rank: 4,
    expansion: None,
)
```

### 3.3 Tier 2: custom `impl Lattice`

For genuinely structural categories, the escape hatch is implementing
the trait directly. CAPCO needs this for at least three categories:

```rust
// Sketch; actual types live in marque-capco.
pub struct SciSet {
    // Control system -> compartments -> sub-compartments.
    systems: BTreeMap<ControlSystem, CompartmentTree>,
}

impl Lattice for SciSet {
    fn join(&self, other: &Self) -> Self { /* union trees */ }
    fn meet(&self, other: &Self) -> Self { /* intersect trees */ }
}

impl BoundedLattice for SciSet {
    fn bottom() -> Self { Self::empty() }
    fn top() -> Self { Self::all_systems_all_compartments() /* if meaningful */ }
}
```

FGI concealment is a finite lattice with three ordered states
(`Open ⊏ KnownProducers ⊏ Hidden`); SAR programs/compartments/sub-
compartments mirror SCI's tree shape; CUI specified categories will
grow their own tree when NARA's categorization is encoded.

### 3.4 Why not keep `AggregationOp`

The enum had one property a trait can't match: it's *data*,
inspectable at runtime by tools that don't want to call into
scheme-specific code (e.g., a scheme-exploration UI). We preserve
that via a separate `CategoryShape` descriptor returned from
`Category::shape()`:

```rust
pub enum CategoryShape {
    Ordinal,                                      // OrdMax / OrdMin
    FlatSet,                                      // FlatSet
    IntersectSet,                                 // IntersectSet
    Supersession,                                 // SupersessionSet
    Date,                                         // MaxDate
    Mode,                                         // ModeSet
    Optional(Box<CategoryShape>),                 // OptionalSingleton
    Product(Vec<CategoryShape>),                  // Product
    Custom,                                       // bespoke impl
}
```

A scheme exploration tool can walk `scheme.categories()` and render
its shape tree without instantiating any marking values. The engine
doesn't consult this in the hot path; it just calls `join`.

### 3.5 Recursion

Structural categories are themselves lattices whose *values* are
compositions of sub-lattices. A `SciSet`'s compartment tree is a
`FlatSet<Compartment>` at each system; a compartment's
sub-compartments are another `FlatSet<SubCompartment>`. This is the
"patterns repeating at multiple levels" the user observed — and once
categories are lattices, nothing stops an author from nesting the
primitives. The built-in `Product<L1, L2>` constructor exists
specifically to make nested composition ergonomic without a
hand-written impl.

## 4. Lossless token vocabulary

### 4.1 What the ISM XML carries

The ODNI ISM CVE files (see `crates/ism/schemas/ISM-v2022-DEC/CVE_ISM/`)
are authoritative for vocabulary. Per term, the JSON form carries:

```json
{
  "Value":       { "text": "HCS-O",           "ism:classification": "U", "ism:ownerProducer": "USA" },
  "Description": { "text": "HCS OP",          "ism:classification": "U", "ism:ownerProducer": "USA" },
  "ism:classification": "U",
  "ism:ownerProducer":  "USA"
}
```

Per enumeration (`CVE` root), the file carries:

- `CVE:IRM.URN` — stable URN (`urn:us:gov:ic:cvenum:ism:dissem`)
- `CVE:IRM.Title`
- `CVE:IRM.Description` (prose)
- `CVE:IRM.Created` — ISO-8601 timestamp
- `CVE:IRM.Source` — authority ("IC Systems Register and Manual")
- `CVE:IRM.PointOfContact` — name + email
- `CVEVersion`, `specVersion`, `ism:DESVersion`, `ism:ISMCATCESVersion`
- `Enumeration.multivalue` — whether the category is multi-value
- ISO-8601 `ism:createDate` at the enumeration level

`Schematron/ISM_XML.sch` and `Schema/IC-ISM.xsd` add deprecation dates
and cross-enumeration constraint predicates.

### 4.2 Why we preserve it

Marque's legitimacy comes from being *authoritative*: an agency
reviewer should be able to point at marque's rationale for a fix and
trace it back through the originator's authority and point of
contact. Flattening every term to an opaque `TokenId(u32)` erases
that chain.

Once preserved, the metadata also unlocks:

- **Schema-version reporting.** "This fix depends on ISM 2022-DEC; the
  next CAPCO revision supersedes HCS-O semantics." Known today but
  only loosely surfaced.
- **Authority-scoped rules.** A rule can be gated to tokens owned by a
  specific producer (e.g., "CIA-originated SCI requires X").
- **Deprecation provenance.** When a deprecated term is auto-replaced,
  the audit record names the replacement source and date.
- **Human-readable rendering.** `Description` gives the unabbreviated
  banner form directly from the source of record, not a hand-curated
  table that drifts from the spec.

### 4.3 Vocabulary surface

`marque-scheme` adds:

```rust
pub trait Vocabulary {
    type Token: Copy + Eq + Hash;
    type Metadata: TokenMetadata;

    fn lookup(&self, token: Self::Token) -> &Self::Metadata;
    fn by_canonical(&self, text: &str) -> Option<Self::Token>;
    fn by_alias(&self, text: &str) -> Option<Self::Token>;
    fn deprecated_replacement(&self, token: Self::Token) -> Option<Self::Token>;
    fn iter(&self) -> impl Iterator<Item = (Self::Token, &Self::Metadata)>;
}

pub trait TokenMetadata {
    fn canonical(&self) -> &str;            // "HCS-O"
    fn description(&self) -> &str;          // "HCS OP"
    fn owner_producer(&self) -> &str;       // "USA"
    fn metadata_classification(&self) -> &str;  // "U"
    fn authority(&self) -> &Authority;      // source + URN + version
    fn point_of_contact(&self) -> Option<&PointOfContact>;
    fn deprecation(&self) -> Option<&Deprecation>;
    fn category(&self) -> CategoryId;

    // Display-form hooks the engine and rules consume.
    fn portion_form(&self) -> &str;
    fn banner_form(&self) -> &str;
    fn banner_abbreviation(&self) -> Option<&str>;

    // Scheme-specific extensions addressable by name.
    fn extension(&self, key: &str) -> Option<&str>;
}
```

Scheme authors can extend `Metadata` with a struct specific to their
CVE (e.g., SCI's control-system membership, SAR's compartment
hierarchy). The engine only reads what's in `TokenMetadata`; rules may
downcast via `Any` for scheme-specific fields.

### 4.4 Build-time generation, runtime immutability

`marque-ism`'s `build.rs` parses the ISM JSON (and falls back to XML
for schema files JSON doesn't cover, like XSDs and Schematron). The
output is `const` tables of `TokenMetadataFull { ... }` values in
`OUT_DIR`, included via `generated.rs`. No runtime I/O, no allocation
on vocabulary lookup, no divergence from ODNI's source of truth.

Each metadata field is `&'static str`, so lookups cost a direct array
index. Aliases (portion vs banner vs description) live in a
perfect-hash table generated at build time.

A scheme-level schema version (`ism-schema-version` in
`crates/ism/Cargo.toml`) pins the source. Tokens carry their own
`CVEVersion` / `specVersion` for provenance; bumping the schema is a
deliberate action, not a silent refresh.

## 5. Strict parser + probabilistic decoder

### 5.1 The dual-recognizer design

Two recognizers, one output type:

```rust
pub enum Parsed<M> {
    Unambiguous(M),
    Ambiguous { candidates: Vec<Candidate<M>> },
}

pub trait Recognizer {
    type Scheme: MarkingScheme;
    fn recognize(
        &self,
        input: &[u8],
        region: Span,
    ) -> Result<Parsed<<Self::Scheme as MarkingScheme>::Marking>, RecognizeError>;
}
```

- **Strict recognizer** — today's Aho-Corasick + structural parser.
  Fast (SIMD candidate detection, zero-allocation grammar walk).
  Succeeds when the region matches the grammar. On success, emits
  `Unambiguous` or (for known local ambiguities like `(C)`)
  `Ambiguous` with corpus-derived candidates.
- **Probabilistic decoder** — bag-of-tokens Bayesian recognizer.
  Triggered on strict failure for a candidate region, or on
  low-confidence strict results flagged by a rule (e.g., the banner
  disagrees with ⋁ portions). Collects observed tokens and
  proximities, scores candidate markings against the scheme's
  marking space, returns the top candidate if its posterior exceeds
  threshold — otherwise `Ambiguous { candidates }` for the resolver.

Both paths emit `Parsed<Marking>`. The engine's downstream stages
(`validate`, `apply_fixes`) don't distinguish which recognizer
produced the parse; they branch on `Unambiguous` vs `Ambiguous` and
on the attached confidence score.

### 5.2 Decoder mechanics

Input to the decoder for a candidate region:
- Set of tokens observed in the region (regardless of delimiters).
- Proximity/adjacency structure (e.g., "SI and TK appear before
  NOFORN in the source").
- Surrounding context features the feature-engineering layer emits
  (line structure, CAB nearby, portion vs banner location).

The decoder enumerates a small number of candidate markings seeded
by the observed token set: each candidate is a scheme-legal marking
that *could* have produced the observed tokens (under some set of
edits — wrong order, missing delimiter, spurious token, superseded
token). Candidate generation is bounded — no unbounded search; the
candidate factory is scheme-provided and declares its budget.

For each candidate, the decoder computes:

```
log_posterior(candidate | observation)
  = log_prior(candidate)                         // base rate from corpus
  + Σ log_likelihood(feature | candidate)        // evidence features
  - log_normalizer                                // only affects ratios
```

All log-likelihoods are hand-engineered, corpus-estimated, and
per-feature explainable. No neural model. The resolver compares the
top two candidates' posterior ratio; above a configured threshold,
it emits `Unambiguous(top)`. Below, `Ambiguous { candidates }`.

Compared to the scope in `2026-04-16-probabilistic-recognition.md`,
this doc promotes the decoder from "`(C)` disambiguation" to the
general fallback for any unparseable region.

### 5.3 When the decoder fires

To preserve G6 (performance) and G7 (honor the common case), the
decoder does **not** run on every region. Gates:

- The strict parser returned `Err` on this region.
- OR a validate-stage rule flagged the region as low confidence
  (e.g., banner doesn't cover portions, non-canonical token
  adjacency).
- OR the caller passed `--deep-scan` explicitly (batch mode).

For typical interactive authoring, the decoder runs on 0-3 regions
per document. Latency budget: ≤2 ms per decoded region on 10 KB
input.

### 5.4 Honoring the observed corpus

The corpus analysis that already ships in
`tools/corpus-analysis/output/enron-full.json` gives us base rates
over:

- Token unigrams, bigrams, trigrams.
- Category co-occurrences.
- Typical portion/banner shapes.

The decoder's `log_prior` is derived from these. The build process
bakes the frequency tables into the WASM binary by default, with an
override path for deployments that want their own corpus.

### 5.5 Variants in the automaton: no

A natural adjacent idea — "add likely-incorrect variants to the
Aho-Corasick automaton" — is rejected. The strict path's speed and
precision come from an automaton that only matches canonical tokens
plus the short hand-curated corrections list. Inflating it with
edit-distance variants explodes false positives and destroys the
"candidate region" invariant the parser depends on. Mangled input is
the decoder's job, and the decoder has access to context features
the automaton doesn't (proximity, category priors, region type).

The corrections map stays: it's a small set of empirically-observed
misspellings (`SERCET`, etc.) with high posterior and no ambiguity.

## 6. Provenance and audit

### 6.1 FixProposal + AppliedFix shape

`FixProposal` today carries `confidence: f32` and a `source` tag. It
grows to carry explicit recognizer provenance:

```rust
pub struct FixProposal {
    pub span: Span,
    pub replacement: String,
    pub confidence: Confidence,        // below
    pub source: FixSource,
    pub migration_ref: Option<&'static str>,
}

pub struct Confidence {
    pub recognition: f32,              // strict = 1.0; decoder = posterior
    pub rule: f32,                     // rule-declared confidence
    pub region: Option<f32>,           // optional, from a detection layer
    pub runner_up_ratio: Option<f32>,  // present when recognizer was the decoder
    pub features: Vec<FeatureContribution>, // present for decoder provenance
}

pub enum FixSource {
    StrictParse,                       // exact grammar match
    CorrectionsMap,                    // SERCET → SECRET
    DecoderPosterior,                  // decoder's top candidate
    RuleDerived { rule: &'static str },
}
```

The aggregate score `recognition × rule × region.unwrap_or(1.0)`
drives the fix/threshold decision in `Engine::fix`. The `features`
vec is the chain of evidence a compliance reviewer can walk to
understand *why* a fix was chosen; it's the realization of G5.

### 6.2 Audit record

`AppliedFix` (engine-only constructor, already architecture-invariant)
embeds the promoted `FixProposal` plus runtime state (timestamp,
classifier id, dry_run). A batch reviewer can set a policy
("accept fixes with aggregate confidence ≥ 0.85, surface the rest")
and replay the corpus to measure actual error rates — then tune.

Every field of `AppliedFix` is content-ignorant: spans, token
canonicals, posterior scalars, feature labels, nothing from the
source text. G13 remains intact.

## 7. Scope-parameterized projection

The prior doc described `project_banner(portions[])`. Generalize:

```rust
pub enum Scope {
    Portion,                           // individual marking
    Page,                              // page-level rollup (banner / CAB)
    Document,                          // document-level rollup
    Diff { from: MarkingRef, to: MarkingRef },  // diff-rule context
}

pub trait MarkingScheme {
    // ...
    fn project(
        &self,
        scope: Scope,
        markings: &[Self::Marking],
    ) -> Self::Marking;
}
```

For CAPCO, `Scope::Page` and `Scope::Document` typically coincide on
single-page documents and diverge on multi-page. `Scope::Diff` is
how two-marking comparison rules (§8) ask the scheme to compute
"from ⊔ to" or "from ⊓ to" in scheme-aware terms.

`PageContext` in `marque-ism` remains the existing page-level
aggregator; Phase B rewires its internals to drive through
`scheme.project(Scope::Page, &portions)`. Its public API stays
stable so rules don't change.

## 8. Control blocks as a separate trait

CABs (Classification Authority Blocks in CAPCO, Controlled By blocks
in CUI, equivalents elsewhere) aren't reductions over markings. They
carry fields the markings don't imply:

- Classified By (identity, not a marking)
- Derived From (citation list or "multiple sources")
- Reason (EO 13526 category, or equivalent)
- Declassify On (sometimes derivable from the markings' declass
  dates, often not — overridden by the classifier)
- Source citations (XML metadata can populate this deterministically
  when present; human-authored docs rarely carry it)

Because a CAB requires inputs that aren't markings, it gets its own
trait:

```rust
pub trait ControlBlock {
    type Scheme: MarkingScheme;
    type Cab;
    type CabTemplate;

    fn template(&self) -> Self::CabTemplate;

    // Best-effort derivation from available context.
    // Returns a PartialCab listing what marque could compute and what
    // the user still has to fill in.
    fn derive_from(
        &self,
        document_markings: &[<Self::Scheme as MarkingScheme>::Marking],
        provenance: &Provenance,
    ) -> PartialCab<Self::Cab>;

    fn render(&self, cab: &Self::Cab) -> String;
    fn validate(&self, cab: &Self::Cab) -> Vec<Diagnostic>;
}

pub struct PartialCab<C> {
    pub filled: C,                          // fields we computed
    pub required: Vec<CabField>,            // fields the user must provide
    pub suggestions: Vec<CabSuggestion>,    // guesses with rationale
}
```

Phase G (§10) builds this out. Until then, CAB handling stays in the
existing CAPCO rules, unchanged.

### 8.1 Deterministic CABs from XML metadata

When a document arrives as XML/JSON in an ISM-compliant format, the
source citations are in the metadata. `ControlBlock::derive_from`
can return a fully-`filled` `Cab` with an empty `required` vector.
This is the "multi-source derivative citation list" described in
the user's original notes, but expressed as one implementor of a
general derivation mechanism — not a CAPCO-specific feature.

## 9. Codec / serialization surface (deferred)

For XML/JSON round-trip, add:

```rust
pub trait Codec<S: MarkingScheme> {
    type Format;
    type Error;
    fn encode(&self, m: &S::Marking) -> Result<Self::Format, Self::Error>;
    fn decode(&self, f: Self::Format) -> Result<Parsed<S::Marking>, Self::Error>;
}
```

Implementations live outside the core crate. The trait exists so that
Phase E can add `CapcoXmlCodec` and `CapcoJsonCodec` without engine
edits, and so external consumers (downstream systems that ingest
ISM XML metadata) have a stable interface.

Not implemented in Phase B. Included here to pin the shape.

## 10. Proactive feedback and diff rules

The lattice gives us cheap primitives for two-marking checks:

```rust
// Banner does not cover its portions.
if banner < portions.iter().fold(M::bottom(), |acc, p| acc.join(p)) { ... }

// Reply weakens sender.
if reply < parent { emit_warning(...) }

// Superset check for CUI re-disclosure.
if disclosed < original { block(...) }
```

Once `Marking: BoundedLattice`, these are one-liners. Phase H wires
them as rules consuming two-marking inputs via `Scope::Diff` and
emits warnings (never auto-fixes, because the correct resolution
depends on author intent).

## 11. Style flexibility

Unchanged from existing design. Style rules live in the rules crate,
configured via `.marque.toml`, and operate on the output of the
engine. The engine only knows about correctness-bearing
transformations (fixes driven by confidence); style transformations
are a separate rule category.

Future addition: a `StyleRule` trait alongside `Rule` that explicitly
marks a rule as style-only, so the engine can suppress style rules in
modes where only correctness matters (CI gate, audit replay).

## 12. Revised phase sequence

Each phase is a self-contained PR. Gate on equivalence tests +
corpus accuracy harness. Prior phase names are retained where the
intent matches.

### Phase B — Recursive category lattices

Goal: every CAPCO category is a `Lattice`. `PageContext` internals
are driven by `scheme.project(Scope::Page, ...)` dispatching through
per-category `join`.

- Introduce built-in lattice constructors in `marque-scheme`:
  `OrdMax`, `FlatSet`, `IntersectSet`, `SupersessionSet`, `MaxDate`,
  `OptionalSingleton`, `Product`.
- Promote the scheme's category descriptors to return a `Category`
  that owns an `impl Lattice` marking (or a constructor for one).
- Port CAPCO's structural types (`SciMarking`, `SarMarking`,
  `FgiMarker`) to `impl Lattice` + `impl BoundedLattice` on a
  `marque-capco`-local struct.
- Rewire `PageContext::expected_*` to call through
  `scheme.project(Scope::Page, ...)`. Public API unchanged.
- Retire `AggregationOp::Custom`. Any remaining custom logic lives
  in the per-category lattice impl.
- Full CAPCO corpus equivalence run + existing Phase A equivalence
  tests. Gate.

### Phase C — Declarative constraints

Goal: move the constraint-style CAPCO rules (NOFORN∥REL TO,
RD⇒NOFORN, JOINT⇔FGI, HCS system rules, etc.) to declarative
`Constraint` data consumed by a generic constraint-checker rule.
Hand-written constraint rules retire.

- Complete the `Constraint` enum (already sketched Phase A):
  `Conflicts`, `Requires`, `Implies`, `Supersedes`, and a
  purpose-built `Custom(&'static str)` that dispatches to a
  registered fn pointer.
- Rewrite ~15 of the 39 CAPCO rules as `Constraint` entries.
  Equivalence test: same diagnostics produced on the corpus.
- Everything that can't express declaratively stays as `Rule`
  (non-constraint rules, e.g., banner-abbreviation preference).

### Phase D — Probabilistic decoder (moved earlier than Phase A doc)

Goal: ship the decoder end-to-end so mangled inputs fix automatically
with provenance. This is the `marque-detect` work the prior plan
deferred to Phase E; promoting it here because CUI (Phase F) will
exercise it heavily, and the `Parsed::Ambiguous` plumbing needs real
end-to-end traffic to stabilize.

- Add a `Recognizer` trait + strict and decoder implementors.
- Build the candidate generator for CAPCO (bounded edits to observed
  tokens). Scheme-specific.
- Wire corpus-derived priors into the decoder. Frequency tables
  baked into WASM by default; override path for custom corpora.
- Extend `FixProposal::Confidence` per §6.1.
- Latency budget: p95 ≤18 ms on 10 KB (slightly above current 16 ms
  target to reflect the decoder's cost on genuinely-mangled inputs;
  still perceptually instantaneous).
- New accuracy metric on corpus: "mangled-marking resolution rate"
  with per-confidence-bucket error rates.

### Phase E — Vocabulary consolidation + codec scaffolding

Goal: full ISM metadata surfaced through `Vocabulary` + provisional
codec traits.

- `marque-ism::build.rs` parses ISM JSON (XML fallback for schemas
  JSON doesn't cover) and generates `TokenMetadataFull` const tables.
- `Vocabulary` trait + CAPCO implementation. Rules that consume
  metadata (authority, owner/producer, deprecation replacement) start
  using it; legacy opaque-`TokenId` paths stay as fallbacks during
  migration.
- `Codec<S>` trait in `marque-scheme`. No implementations yet — the
  surface is pinned so Phase G can wire XML/JSON round-trip.

### Phase F — CUI as second scheme

Goal: implement NARA CUI as a second `MarkingScheme` to validate
genericity.

- `marque-cui` crate, separate from `marque-capco`.
- CUI's ~125 categories encoded declaratively via built-in lattice
  constructors where possible; escape to custom impls where not.
- If CUI exposes expressiveness gaps in the trait surface, they're
  addressed here — back-ported to `marque-scheme` as trait
  extensions.
- Engine gains multi-scheme dispatch: a document can declare its
  governing scheme via config, header, or auto-detection.

### Phase G — ControlBlock trait + CAB derivation

Goal: generalize CAB handling. CAPCO and CUI both implement
`ControlBlock`. Deterministic CAB derivation lands for XML-metadata
sources.

- `ControlBlock` trait per §8.
- `CapcoCab` and `CuiCab` as concrete impls.
- `derive_from` with provenance tracking: XML metadata → full
  derivation; plain text → partial with explicit user fields.
- Rules that today validate CABs move to
  `ControlBlock::validate` + thin rule adapters.

### Phase H — Diff rules and proactive feedback

Goal: two-marking comparison rules using the lattice algebra.

- `Scope::Diff` wired through `project`.
- Rules: reply-weakens-parent (email thread analysis),
  banner-doesn't-cover-portions (already exists; now expressed via
  the diff primitive), CUI re-disclosure check, historical-marking
  supersession.
- Warnings only. No auto-fix on diff rules; the resolution depends
  on author intent and marque shouldn't guess.

## 13. Open questions

Called out explicitly so we don't silently drift.

- **Q1. Confidence threshold interaction with `Severity`.** A rule at
  `severity: fix` combined with aggregate confidence 0.72 — fix?
  warn? Probably product of confidence × severity-weight against a
  single threshold, but the weighting scheme needs corpus validation
  before it's frozen. Currently hand-wave.
- **Q2. Edit-distance cutoff for decoder candidate generation.**
  Likely 1–2 edits, with a per-token cap based on token length and
  base rate. Concrete table derived from the corpus; set in Phase D.
- **Q3. Baked-in vs runtime frequency tables.** Baked in by default;
  override path via config. Open: file format for override (CSV?
  JSON? MessagePack?).
- **Q4. Multi-scheme documents.** A document with both CAPCO portion
  markings and CUI banners — rare but possible. Phase F decision:
  one scheme per document with override, or allow sub-regions to
  declare a different scheme? Lean toward the first; revisit if
  real-world cases require otherwise.
- **Q5. Custom `impl Lattice` authoring ergonomics.** How much can we
  reduce the boilerplate via derive macros (e.g., `#[derive(Lattice)]`
  for product types)? Nice-to-have; not blocking.
- **Q6. Decoder explainability UX.** When a fix is driven by the
  decoder, the audit record has the features — what's the
  presentation layer? Inline comment? Separate report file? Deferred
  to the server/CLI layer, not the engine.
- **Q7. Partial CAB → interactive completion.** When `derive_from`
  returns `required` fields, how does marque surface them to the
  user in CLI / server / Office add-in contexts? Per-frontend;
  `PartialCab` just has to be machine-readable.

## 14. What we dropped

For the record, so future readers don't re-propose them.

- **`AggregationOp` as a runtime dispatch point.** Preserved as
  build-time shorthand (`Category::flat_set(...)`) and runtime
  metadata via `CategoryShape`; retired from the hot path.
- **Edit-distance variants in the Aho-Corasick automaton.** See §5.5.
- **Extending the `MarkingScheme` trait with CAB methods.** CABs
  aren't markings and don't derive purely from markings; they get
  their own trait.

## 15. Mapping to prior plans

- `2026-03-11-marque-design.md` — motivation and scope; unchanged.
- `2026-04-16-probabilistic-recognition.md` — specific `(C)` case
  study. Generalized: the decoder from §5 subsumes it. The empirical
  base rates from that doc are direct inputs to the decoder's
  prior (Phase D).
- `2026-04-17-marking-scheme-lattice-design.md` — problem framing,
  core algebra, Phase A scaffolding. Unchanged, except: §3 (trait
  surface) is superseded by §3 here (recursive lattices); §4
  (probabilistic) is superseded by §5 here; §5 (migration
  sequence) is superseded by §12 here.
- `vocabulary-provider-domain-notes.md` and
  `vocabulary-provider-signal-model.md` — retained as historical
  context. The signal/codebook framing remains motivation (a lossy
  projection over independent channels); the formal machinery stays
  lattice-theoretic.
