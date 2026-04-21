<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Marque: recursive lattices, lossless tokens, and the probabilistic decoder

**Date:** 2026-04-19 (revised), 2026-04-20 (amended 7a, 12 phase C, 14)
**Status:** proposed — supersedes §§3–5 of
`2026-04-17-marking-scheme-lattice-design.md` end-to-end (trait
surface, probabilistic disambiguation, migration sequence). The phase
letters in §12 below are re-scoped: old Phase D ("Implement CUI") is
re-lettered to Phase F here, old Phase E ("Fuzzy resolver") is folded
into Phase D here. Where this doc and the 2026-04-17 doc name the
same phase letter, **this document wins**.
**Builds on:** `2026-04-17-marking-scheme-lattice-design.md` §§0–2 —
problem statement, core algebra, and Phase A scaffolding are
unchanged.
**Revision history:** 2026-04-19 initial draft; 2026-04-19 (this file)
incorporates ultraplan review — fixes factual errors (F1–F3), adds
§5.1a recognizer adapter, §6a threat model, §7a cross-category
rewrites, pins `DiffInput` / `CabSuggestion` / `SciSet::meet` / `K=8`
decoder bound / `Send + Sync` contract, reconciles perf budget,
retracts schema-derived deprecation claim, adds `marque-extract`
integration, `--deep-scan` authoring gate, agency/CUI config gates,
Q8 (no WASM corpus override), and the `FOUO → CUI` migration
correction.

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
   token bag. That capability is table stakes for batch cleanup over
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
- **G7a. Authoring-mode latency envelope.** During interactive
  keystroke authoring, the engine runs the strict path only. The
  probabilistic decoder is opt-in per session (`--deep-scan` / server
  config) or triggered by an explicit batch reconciliation entry
  point. Interactive latency is bounded by the strict parser alone,
  not by whatever the decoder costs on the hardest region in the
  document.
- **G9a.** Fix confidence is a product of recognition confidence,
  rule confidence, and (optionally) region confidence. The interaction
  is explicit in code, not implicit in rule severity.

## 2. Architecture overview

```
 ┌──────────────┐
 │marque-extract│  (Kreuzberg wrapper — noisy by design: OCR, 75+ doc formats)
 └──────┬───────┘
        │ raw text bytes + provenance metadata (unchanged on the wire)
        ▼
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
  │   built-in constructors: OrdMax, OrdMin, FlatSet,            │
  │       IntersectSet, MaxDate, SupersessionSet, ModeSet,       │
  │       OptionalSingleton, Product                             │
  │   trait Vocabulary                                           │
  │   trait MarkingScheme                                         │
  │   trait ControlBlock (separate from MarkingScheme)            │
  │   enum Parsed<M> { Unambiguous | Ambiguous { candidates } }   │
  └────────────────────────────┬──────────────────────────────────┘
                               │ implemented by
         ┌─────────────────────┼─────────────────────┐
         ▼                     ▼                     ▼
   marque-capco           (future) CUI          (future) NATO
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

### 2.1 Input sources and the extract boundary

`marque-extract` (Kreuzberg wrapper) is explicitly in-scope as a
producer of noisy input — OCR, PDF text-layer extraction, scanned
fax workflows. Its output feeds the engine unchanged; the engine
decides strict-vs-decoder per candidate region. The decoder is
precisely the failure-mode's target consumer: when OCR mangles a
banner, the strict path fails on that region, the decoder takes
over, and the aggregate document latency stays under budget because
only the mangled regions pay the decoder cost.

### 2.2 Performance budget (single source of truth)

One number governs the regression gate:

> **p95 lint latency ≤ 16 ms on 10 KB of strict-path input.**

The decoder's cost is budgeted *into* that total, not on top of it.
Concretely:

- Strict-only path (no decoded regions): ≤ 16 ms — the G6 envelope.
- With one decoded region: ≤ 18 ms total — a single decoder
  invocation costs ≤ 2 ms end-to-end (candidate enumeration + feature
  extraction + posterior ranking). The 18 ms number that appears in
  Phase D (§12) is this worst-case total, not a separate target.
- With N decoded regions: a soft upper bound of `16 + 2·N` ms. Beyond
  three concurrent decoded regions in one document, the engine drops
  remaining decoder invocations and marks them as `Parsed::Ambiguous`
  without evidence — the strict path never sees the degradation.

The regression gate runs against a strict-only corpus to avoid
letting decoder tuning paper over strict-path regressions. The
decoder has its own accuracy gate (Phase D, §12).

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
    CategoryId(4), "dissem", 4, // 3rd arg is ordering-rank
    None, // expansion
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
    /// Component-wise union. For each control system present in either
    /// operand, include it in the result; within a system, union its
    /// compartments; within a compartment, union its sub-compartments.
    fn join(&self, other: &Self) -> Self { /* union trees */ }

    /// Component-wise intersection with explicit orphan semantics.
    /// See the note below — CAPCO does not define an unambiguous meet
    /// on arbitrary SCI trees, so this implementation picks a single
    /// policy and the doc comment warns that it is not a lattice meet
    /// in every usage.
    fn meet(&self, other: &Self) -> Self { /* see §3.3a */ }
}

impl BoundedLattice for SciSet {
    fn bottom() -> Self { Self::empty() }
    fn top() -> Self { Self::all_systems_all_compartments() /* scheme-defined */ }
}
```

FGI concealment is a finite lattice with three ordered states
(`Open ⊏ KnownProducers ⊏ Hidden`); SAR programs/compartments/sub-
compartments mirror SCI's tree shape; CUI specified categories will
grow their own tree when NARA's categorization is encoded.

### 3.3a A note on `SciSet::meet`

Tree intersection is not unique. Given `SI-G ABCD` on the left and
plain `SI` on the right, the meet could reasonably be (a) `SI-G
ABCD` (right's "SI" is the broadest ancestor and survives), (b) just
`SI` (drop everything the right side doesn't explicitly name), or
(c) empty (only identical leaves survive). CAPCO does not settle
the question because it never describes a "meet" operation — the
only operation the spec defines on SCI across portions is the join
(roll-up, §A.6 p15).

Phase B picks policy (b): meet keeps only elements present at the
same depth in both operands. That gives `SI ⊓ SI-G ABCD = SI` (not
`SI-G ABCD`), and is the interpretation closest to the plain lattice
definition (`x ⊓ y ≤ x` and `x ⊓ y ≤ y`). The implementation's doc
comment states the policy and names the two rejected alternatives.

Callers that need a different interpretation (primarily the
constraint-evaluator in Phase C, when asking "do these two portions
share any SCI compartment?") use scheme-specific helpers
(`SciSet::overlaps`, `SciSet::common_compartments`) rather than
`Lattice::meet`. Naming is explicit about which semantics a caller
is asking for.

Same reasoning applies to `SarSet::meet` and `FgiMarker::meet`. The
PR documents the policy in each type's doc comment and tests the
boundary cases with property tests.

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
- **Human-readable rendering.** `Description` gives the unabbreviated
  banner form directly from the source of record, not a hand-curated
  table that drifts from the spec.

Deprecation *replacements* stay hand-maintained in
`marque-ism/build.rs::MIGRATIONS`. The Schematron files encode
validity predicates, not policy judgments about which successor
codeword replaces which deprecated one at what confidence. That
mapping is narrower than ODNI chooses to encode in the schema and
needs editorial discretion (see `M-FouoBug` in §14 for a concrete
example of why the policy layer must sit above the schema). The
vocabulary surface exposes whether a term is deprecated and the URN
of its replacement *when the build-time migration table knows one*
— not by parsing deprecation XSD annotations.

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

Phase E's `marque-ism/build.rs` uses two parallel codepaths because
the ODNI distribution ships two parallel formats, not because one is
a fallback for the other:

- **JSON codepath** (`serde_json`) — parses the per-enumeration
  `CVE_ISM/*.json` and `CVE_ISMCAT/*.json` files for term values,
  descriptions, classification/owner metadata, IRM headers (URN,
  authority, POC, created date, CVE/spec/DES versions), and
  enumeration-level `multivalue` flags. This is the primary path
  for per-term data.
- **XML codepath** (`quick-xml`, as today) — parses the XSD
  (`CVE_ISMCAT/CVEGenerated/CVEnumISMCATRelTo.xsd`, `Schema/
  IC-ISM.xsd`) and Schematron (`Schematron/ISM_XML.sch`,
  `Schematron/Lib/*.sch`) files for validity predicates and
  attribute structures. These files have no JSON equivalent.

Both codepaths exist in the tree; no format switch. Output is `const`
tables of `TokenMetadataFull { ... }` values in `OUT_DIR`, included
via `generated.rs`. No runtime I/O, no allocation on vocabulary
lookup.

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

### 5.1a Integration with today's parser

Today `marque_core::Parser::parse(candidate, source)` returns a
`ParsedMarking` — not a `Parsed<M>`. The `Recognizer` trait is new
and wrapping the existing parser requires an adapter. Design:

- The adapter lives in **`marque-engine`**, not in `marque-core` or
  `marque-scheme`. Rationale: the adapter depends on scheme +
  recognizer + the existing parser simultaneously; only the engine
  crate already sits at that intersection.
- `pub struct StrictRecognizer<S: MarkingScheme> { core: marque_core::Parser, scheme: S }`
  with `impl<S> Recognizer for StrictRecognizer<S>`. The `recognize`
  method calls `core.parse(...)`, and on success converts the
  `ParsedMarking` into `Parsed::Unambiguous(S::Marking)` via a
  scheme-provided `from_parsed_marking` hook (Phase B adds this to
  `MarkingScheme`).
- `marque-core::Parser` stays unchanged — no crate sees a new
  dependency on `marque-scheme` except the engine and adapters. G1
  (grammar-agnostic engine) is preserved: the engine holds the
  adapter; the parser does not know about schemes.
- The existing `(C)` ambiguity path inside `marque-core` produces a
  `ParsedMarking` with a flag we convert to `Parsed::Ambiguous`
  before leaving the adapter. The scheme-level resolver never sees
  `ParsedMarking`.

Phase B lands the adapter and flips `Engine::lint_inner` to drive
through `Recognizer::recognize`. The flip is invisible to rules —
`Diagnostic` and `FixProposal` are unchanged on this axis.

### 5.2 Decoder mechanics

Input to the decoder for a candidate region:
- Set of tokens observed in the region (regardless of delimiters).
- Proximity/adjacency structure (e.g., "SI and TK appear before
  NOFORN in the source").
- Surrounding context features the feature-engineering layer emits
  (line structure, CAB nearby, portion vs banner location).

The decoder enumerates a small number of candidate markings seeded
by the observed token set. Each candidate is a scheme-legal marking
that *could* have produced the observed tokens under some set of
edits — wrong order, missing delimiter, spurious token, superseded
token.

**Candidate bounds (concrete, Phase D).**
For each scheme-declared **template** (CAPCO: `Portion`, `Banner`,
`JointPortion`, `JointBanner`, `NatoPortion`, `NatoBanner`,
`FgiPortion`, `FgiBanner`, `NonUsPortion`, `NonUsBanner`), the
candidate generator:

1. Assigns observed tokens to the template's category slots in every
   ordering consistent with the template's grammar.
2. Scores each slot assignment by prior × per-slot compatibility.
3. Retains at most **K = 8** top candidates per template, trims
   across templates to the top `K` overall.

If the observed token bag doesn't fit any template (e.g., tokens from
two different schemes), the decoder returns `Parsed::Ambiguous` with
zero candidates — explicitly "we see signal but can't resolve."
Never a silent fallthrough to the strict-path error.

`K = 8` is picked to keep per-region decoder work bounded (§2.2:
≤ 2 ms budget) and is tunable at scheme level via
`MarkingScheme::decoder_budget()`. Q2 (§13) settles the per-template
edit-distance cutoffs against the corpus.

**Concurrency.** `Recognizer` impls are `Send + Sync` and the decoder
carries no mutable global state. Corpus tables are baked-in at
compile time as `&'static` slices; feature extractors allocate
only in per-invocation scratch space. G12 (parallelism) is
preserved: `BatchEngine` can run N decoder invocations concurrently
with no contention beyond allocator pressure.

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
decoder does **not** run on every region. Three execution modes:

| Mode                  | Strict | Decoder | Trigger                                |
| --------------------- | ------ | ------- | -------------------------------------- |
| Interactive authoring | yes    | no      | default CLI / server / WASM            |
| Batch reconciliation  | yes    | yes     | `--deep-scan` flag or batch API        |
| Rule-escalated region | yes    | yes     | validate-stage rule flagged the region |

In interactive authoring, strict failure on a region produces a
diagnostic (not a decoded fix); the user types and the author sees
the strict-path error without the engine burning decoder time on
every keystroke. G7a formalizes this.

In batch reconciliation, the decoder fires on strict-path `Err`
regions and on regions a rule explicitly escalates (e.g., "banner
doesn't cover ⋁ portions" escalates the banner region for
resolution).

Latency budget per decoded region: ≤ 2 ms (§2.2). Per-document
total under all modes respects the 16-ms regression gate.

### 5.4 Honoring the observed corpus

Base rates for the decoder come from corpus analysis output, not
from a committed fixture. `tools/corpus-analysis/output/` is
`.gitignore`d — `enron-full.json` is a local artifact the author
regenerates on demand via `python3 analyze.py --output
output/enron-full.json` (see `tools/corpus-analysis/README.md`).
The artifact is supplied by the author at Phase D build time, not
shipped in-tree.

Phase D's `marque-capco/build.rs` emits the baked frequency table
from an author-supplied JSON path. The build fails closed if the
JSON is missing: a Phase-D-and-later `cargo build -p marque-capco`
without the corpus artifact errors with a clear message pointing at
the regeneration command. The compiled-in tables live behind a
const-or-`None` shape, and the decoder is enabled only when they are
`Some`.

Corpus content used:

- Token unigrams, bigrams, trigrams.
- Category co-occurrences.
- Typical portion/banner shapes.

Frequency tables are baked into every build target (CLI, server,
WASM) as `&'static` slices. Runtime override is limited per §6a; the
WASM target never allows override (Q8).

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

Today's `marque_rules::FixSource` (in
`crates/rules/src/lib.rs:171`) has three variants:

```rust
pub enum FixSource {
    BuiltinRule,
    CorrectionsMap,
    MigrationTable,
}
```

Phase D extends `FixSource` to carry recognizer provenance **without
removing existing variants**:

```rust
pub enum FixSource {
    // Existing — retained for back-compat with marque-mvp-1 audit records.
    BuiltinRule,
    CorrectionsMap,
    MigrationTable,
    // New — decoder path landed with Phase D.
    DecoderPosterior,
}
```

`FixProposal` today carries `confidence: f32` and a `source` tag. It
grows to carry explicit recognizer provenance:

```rust
pub struct FixProposal {
    pub span: Span,
    pub replacement: String,
    pub confidence: Confidence,        // below, replaces scalar f32
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
```

The aggregate score `recognition × rule × region.unwrap_or(1.0)`
drives the fix/threshold decision in `Engine::fix`. The `features`
vec is the chain of evidence a compliance reviewer can walk to
understand *why* a fix was chosen; it's the realization of G5.

**Audit contract schema bump.** Replacing `confidence: f32` with the
`Confidence` struct and adding `FixSource::DecoderPosterior` are
both breaking changes to `contracts/audit-record.json` (today at
`specs/001-marque-mvp/contracts/audit-record.json`, `schema:
"marque-mvp-1"`). Phase D bumps the `schema` const to
`"marque-mvp-2"` and ships a migration note in the phase PR. Readers
of older records continue to parse `marque-mvp-1` records from
before the bump; writers emit only `marque-mvp-2` after the bump.
No two schema versions are emitted by the same engine build.

### 6.2 Audit record

`AppliedFix` (engine-only constructor, already architecture-invariant)
embeds the promoted `FixProposal` plus runtime state (timestamp,
classifier id, dry_run). A batch reviewer can set a policy
("accept fixes with aggregate confidence ≥ 0.85, surface the rest")
and replay the corpus to measure actual error rates — then tune.

Every field of `AppliedFix` is content-ignorant *on the recognizer
axis*: posterior scalars, feature labels, token canonicals — no new
content leaks from the decoder. The pre-existing `original` field
on the audit record continues to contain the exact source slice
covered by the fix span; that predates this doc and is not expanded
here. G13 remains intact on everything this phase adds.

### 6a. Threat model

Three surfaces become attackable when the decoder and richer
provenance ship. Each has a specific mitigation:

**T1. Prior-manipulation attacks on `(C)` and other local
disambiguations.** An adversary drafts prose that statistically
biases the decoder's prior toward a benign interpretation (e.g.,
lots of copyright-flavored context around a `(C)` that is actually
a CONFIDENTIAL portion marking) to suppress detection.

Mitigation: the decoder never *downgrades* confidence in an
already-strict-classified region. If the strict path finds any
portion at CONFIDENTIAL or higher elsewhere in the document,
`(C)` in the same document resolves to CONFIDENTIAL without
consulting the decoder. Only documents with no classified
context at all allow the decoder to consider copyright.

This is a conservative rule — it may cost a small number of
true copyright symbols flagged as portion markings on mixed
documents — but the threat model inverts the error direction
we can tolerate: false positives on classification are cheap to
review, missed classifications are expensive.

**T2. Content leakage through `features: Vec<FeatureContribution>`.**
The decoder's evidence features are scheme vocabulary identifiers
plus structural-context labels ("year-pattern-nearby",
"list-marker-context") — emphatically *not* raw surface bytes.
Feature labels are `&'static str` enumerated at scheme build time.

Mitigation: `FeatureContribution::label` is typed as a
`FeatureId` (enum) at compile time, not a free-form string.
Scheme authors can add new features but cannot emit content
through the label channel. A CI check verifies the audit
record's `features[].label` field never exceeds a whitelisted
length or character set.

**T3. Runtime corpus override as a trust boundary.** If an
operator supplies a custom `log_prior` table via `.marque.toml`,
the decoder's posteriors are under operator control. In server
deployments, a malicious table biases fixes toward a specific
outcome. In WASM deployments, a custom table could arrive over
a web postMessage and silently flip the decoder's behavior.

Mitigation: `--corpus-override` is a CLI flag only, available
in single-operator deployments (the user running `marque`
locally chose the override). The server binary does not accept
corpus overrides from HTTP requests. The WASM target has no
filesystem and the build rejects any attempt to consume a
corpus override at runtime — the only corpus table in WASM is
the one baked at compile time. See Q8 in §13.

T1 and T2 are enforced in the engine; T3 is enforced at the binary
level (different CLI/server/WASM targets have different capability
surfaces). All three are covered by explicit integration tests in
Phase D.

## 7. Scope-parameterized projection

The prior doc described `project_banner(portions[])`. Generalize:

```rust
pub enum Scope {
    Portion,                           // individual marking
    Page,                              // page-level rollup (banner / CAB)
    Document,                          // document-level rollup
    Diff,                              // diff-rule context; see DiffInput below
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
single-page documents and diverge on multi-page.

### 7.1 Diff rules: `DiffInput`

Two-marking diff rules (reply-weakens-parent, banner-vs-portions,
CUI re-disclosure) need a second marking the engine loads
separately. We don't put references inside the `Scope` enum — that
would force `Scope` to carry a lifetime, and most scopes have no
second marking. Instead, a dedicated input type:

```rust
pub struct DiffInput<M: Lattice> {
    pub from: Parsed<M>,
    pub to: Parsed<M>,
    pub relation: DiffRelation,
}

pub enum DiffRelation {
    BannerOverPortions,                // one document, banner vs its portions
    ReplyOverParent,                   // email thread, reply vs parent
    DisclosureOverOriginal,            // CUI re-disclosure
    Historical,                        // current marking vs historical equivalent
    Custom(&'static str),              // scheme-specific
}
```

The caller (CLI batch mode, server diff endpoint) constructs the
`DiffInput` and passes it to the engine's diff rule entry point.
The engine does not fetch second markings — it only evaluates the
relation against the caller-supplied pair. This keeps the engine
content-ignorant and lets the caller scope how "parent" is looked up
(email thread traversal, version-control diff, etc.).

`MarkingScheme::project(Scope::Diff, markings)` is defined as the
scheme-specific reduction of the two markings in `DiffInput`. For
most schemes, this is `markings[0].join(&markings[1])` — i.e., the
composite marking that should cover both. Diff rules consult the
comparison via the `<=` primitive in §10, not via `project`.

### 7.2 PageContext migration

`PageContext` in `marque-ism` remains the existing page-level
aggregator; Phase B rewires its internals to drive through
`scheme.project(Scope::Page, &portions)`. Its public API stays
stable so rules don't change.

### 7a. Cross-category rewrites

Three categories of CAPCO aggregation can't be expressed as
single-category lattice joins:

- **Cross-axis supersession.** NOFORN lives in `dissem`; REL TO lives
  in `rel_to`. When NOFORN is present in a page's dissem roll-up, the
  REL TO roll-up for the same page clears entirely.
- **Cross-axis promotion.** When a JOINT classification carries US
  presence alongside non-US partners, the non-US partner content is
  pulled out of the classification axis and re-routed into the FGI
  axis as attributed entries (`FGI(GBR)`, `FGI(DEU)`, etc.).
- **Within-axis absorption.** When FGI carries `unattributed`
  alongside any attributed entries, the attributed entries collapse
  into the unattributed top element.

The first shipped in Phase B as a `PageRewrite` (see
`crates/capco/src/scheme.rs`); the promotion and absorption rewrites
land in Phase C.

Model these explicitly at `Scope::Page` time:

```rust
pub struct PageRewrite<S: MarkingScheme> {
    pub id: &'static str,                             // "capco/noforn-clears-rel-to"
    pub trigger: CategoryPredicate<S>,                // "NOFORN in dissem"
    pub action: CategoryAction<S>,                    // "clear rel_to"
    pub citation: &'static str,                       // "CAPCO-2016-§H.2"
    /// Categories the trigger inspects. Drives the dependency graph in §7a.2.
    pub reads: &'static [CategoryId],
    /// Categories the action mutates. Drives the dependency graph in §7a.2.
    pub writes: &'static [CategoryId],
}

pub enum CategoryPredicate<S: MarkingScheme> {
    Contains { category: CategoryId, token: S::Token },
    Empty    { category: CategoryId },
    Custom(fn(&S::Marking) -> bool),
}

pub enum CategoryAction<S: MarkingScheme> {
    Clear   { category: CategoryId },
    Replace { category: CategoryId, with: S::Marking },
    /// Move content from one axis to another, optionally transforming
    /// the value during the move. Used for JOINT → FGI promotion.
    Promote {
        from: CategoryId,
        to: CategoryId,
        transform: fn(&S::Marking) -> S::Marking,
    },
    Custom(fn(&mut S::Marking)),
}
```

For non-`Custom` variants, the `reads` and `writes` slices are
derivable from each enum's `category` / `from` / `to` fields. The
`PageRewrite::declarative` constructor populates them at scheme build
time:

```rust
impl<S: MarkingScheme> PageRewrite<S> {
    /// Construct a rewrite with `reads` and `writes` derived from the
    /// predicate and action enum variants. Compile error if either
    /// uses `Custom` — Custom requires the explicit constructor below.
    pub const fn declarative(
        id: &'static str,
        trigger: CategoryPredicate<S>,
        action: CategoryAction<S>,
        citation: &'static str,
    ) -> Self;

    /// Construct a rewrite with explicitly-declared axes for `Custom`
    /// variants. The closure body is opaque to the engine, so the
    /// engine can't infer the dependency without help.
    pub const fn custom(
        id: &'static str,
        trigger: CategoryPredicate<S>,
        action: CategoryAction<S>,
        citation: &'static str,
        reads: &'static [CategoryId],
        writes: &'static [CategoryId],
    ) -> Self;
}
```

`MarkingScheme::page_rewrites() -> &[PageRewrite<Self>]` returns the
scheme's post-aggregation rewrite table. The engine runs
`project(Scope::Page, ...)` first (category-wise joins), then applies
the rewrites in scheduler-determined order (§7a.2), then runs
validate. The declaration is inspectable: tooling can render "this
page will have NOFORN ⇒ REL-TO-cleared applied" without calling scheme
code.

#### 7a.1 Producers and consumers

Rewrites fall into two patterns with different ordering implications:

- **Producers** write to an axis they don't read from. JOINT-promotion
  reads `classification`, writes `fgi` — pulls non-US partner content
  out of `JOINT(USA, GBR)` into `FGI(GBR)` when US is present.
- **Consumers** read an axis they (also) write to. FGI-absorption
  reads `fgi` and writes `fgi` — collapses
  `{unattributed, FGI(GBR), FGI(DEU)}` to `{unattributed}` when the
  unattributed top element is present.

Producer-then-consumer matters operationally. Take the canonical
mixed-source page:

```
(JOINT S USA GBR//REL TO USA, GBR) joint-op portion
(FGI S//REL TO USA, GBR)            unattributed-source portion
(TS//SI//REL TO USA, FVEY)          US-only portion with SI
```

After the category-wise joins, the relevant pre-rewrite axes are:
`classification = TS` (with portion 1's JOINT structure still carrying
GBR as an unmigrated partner), `sci = {SI}`, `fgi = {unattributed}`,
`rel_to = {USA, GBR}` (FVEY's expansion to include GBR is intersected
back against portions 1 and 2). JOINT-promotion fires because US
classification is present on the page; it pulls GBR from portion 1's
JOINT structure into FGI as attributed. State: `fgi = {unattributed,
FGI(GBR)}`. FGI-absorption fires because unattributed is present
alongside attributed; it collapses to `fgi = {unattributed}`. Banner:
`TOP SECRET//SI//FGI//REL TO USA, GBR`.

If absorption ran before JOINT-promotion, the trace would end at
`fgi = {unattributed, FGI(GBR)}` — wrong. The analyst would expect the
collapse, and the engine wouldn't have produced it.

Note the survival of `REL TO USA, GBR` in the banner despite
unattributed FGI being present. Many analysts would expect NOFORN
there, but absorption operates strictly within FGI — it doesn't write
to `dissem`. Portion 2's explicit REL TO is the (unknown) source's
authorization, and the intersection-on-REL-TO carries it through. The
"unattributed FGI typically NOFORNs" pattern is a social convention
among classifiers, not a CAPCO rule, and a marking engine that
imposed it would be wrong on this case. Phase H's diff/feedback rules
can warn the analyst that the combination looks unusual; the engine
itself produces the algebraically correct result. (`R-FgiExplicitRel`
in §12 Phase C verification locks this in as a property test.)

#### 7a.2 Scheduling

The engine treats `page_rewrites()` as a dependency graph: rewrite
`B` depends on rewrite `A` if `A`'s `writes` set intersects `B`'s
`reads` set. A topological sort over the graph produces the run
order. Declaration order is the tiebreaker among rewrites with no
dependency edge between them.

Two failure modes are caught at scheme construction, not per-document
evaluation:

1. **Read-after-write cycles.** If A reads from an axis B writes and
   B reads from an axis A writes (or any longer cycle), no
   topological order exists. The engine constructor returns
   `EngineConstructionError::RewriteCycle { axis, members }`. Schemes
   either break the cycle by re-expressing the rewrites or — if the
   cycle is genuine policy — encode the resolution as a single
   combined rewrite with explicit semantics.
2. **Unannotated `Custom` axes.** `PageRewrite::custom(...)` always
   takes explicit `reads` and `writes` parameters, so the relevant
   construction-time failure is not omission at the call-site but an
   empty or otherwise underspecified axis declaration for a `Custom`
   rewrite. Scheme construction rejects such cases with
   `EngineConstructionError::UnannotatedCustomAxes`. Non-`Custom`
   variants get the annotations for free from
   `PageRewrite::declarative`.

Confluence beyond what topological ordering guarantees is not
required. As long as the dependency graph is acyclic and the schedule
respects it, the result is deterministic regardless of how the
tiebreaker resolves independents — independents, by construction,
don't see each other's writes.

Property tests in `marque-capco` cover the realistic combinations
end-to-end. The topo sort catches structural ordering bugs (a
producer scheduled after its consumer) but not axis-tagging bugs (a
producer mistakenly tagged as not writing the axis it actually
writes). Only an end-to-end test with the right input shape will
flag the latter.

The Phase-B-shipped NOFORN ⊐ REL TO `PageRewrite` is a single-node
graph; Phase C retrofits it with the new `reads` / `writes`
annotations during the same PR that introduces the scheduler. Phase C
also declares JOINT-promotion and FGI-absorption, exercising the
producer-before-consumer ordering path.

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

pub struct CabSuggestion {
    /// Which field this suggestion addresses.
    pub field: CabField,
    /// The proposed value, rendered as a string the user can accept
    /// verbatim. For structured fields (e.g., declassify-on dates),
    /// the suggestion is a canonical form, not a parse tree.
    pub value: String,
    /// Why this suggestion was produced. Parallels a rule citation:
    /// "CAPCO-2016-§D.3 derivative classification from ⋁ portions".
    pub citation: &'static str,
    /// Provenance in the same shape as a FixProposal's Confidence.
    /// Rule-derived suggestions carry `recognition = 1.0` and the
    /// rule ID; corpus-derived suggestions (rare in CABs) carry
    /// decoder-style posteriors.
    pub confidence: Confidence,
    /// Whether accepting this suggestion unambiguously satisfies the
    /// field's requirement, or whether the user must still confirm.
    /// "Multiple sources" over a plain-text document is always
    /// `NeedsConfirmation`; a citation list derived from XML metadata
    /// is `Authoritative`.
    pub disposition: SuggestionDisposition,
}

pub enum SuggestionDisposition {
    Authoritative,                          // accept-as-is is correct
    NeedsConfirmation,                      // best guess; user must confirm
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
fn leq<M: Lattice>(lhs: &M, rhs: &M) -> bool {
    lhs.join(rhs) == rhs.clone()
}

// Banner does not cover its portions.
if !leq(
    &banner,
    &portions.iter().fold(M::bottom(), |acc, p| acc.join(p)),
) { ... }

// Reply weakens sender.
if leq(&reply, &parent) && reply != parent { emit_warning(...) }

// Superset check for CUI re-disclosure.
if !leq(&original, &disclosed) { block(...) }
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
corpus accuracy harness. The phase letters below are re-scoped from
the 2026-04-17 doc — see the header. Where letters collide, this
document wins.

| Letter | This doc                          | 2026-04-17 doc                  |
| ------ | --------------------------------- | ------------------------------- |
| A      | (shipped) scheme scaffolding      | (shipped) scheme scaffolding    |
| B      | Recursive category lattices       | PageContext scheme-driven       |
| C      | Declarative constraints + rewrites | Declarative constraints         |
| D      | Probabilistic decoder             | CUI                             |
| E      | Vocabulary + codec scaffolding    | Fuzzy resolver behind trait     |
| F      | CUI as second scheme              | —                               |
| G      | ControlBlock + CAB derivation     | —                               |
| H      | Diff rules + proactive feedback   | —                               |

Each phase block below lists: **Goal**, **Deliverables**,
**Verification**, **Gate**.

### Phase B — Recursive category lattices

- **Goal.** Every CAPCO category is a `Lattice`. `PageContext`
  internals are driven by `scheme.project(Scope::Page, ...)`
  dispatching through per-category `join`.
- **Deliverables.**
  - Built-in lattice constructors in `marque-scheme`: `OrdMax`,
    `OrdMin`, `FlatSet`, `IntersectSet`, `SupersessionSet`,
    `ModeSet`, `MaxDate`, `OptionalSingleton`, `Product`.
  - Category descriptors promoted to own an `impl Lattice` marking
    (or a constructor for one).
  - CAPCO structural types (`SciMarking`, `SarMarking`, `FgiMarker`)
    ported to `impl Lattice` + `impl BoundedLattice`. Meet semantics
    per §3.3a (policy (b): equal-depth intersection). Helpers
    `SciSet::overlaps` and `SciSet::common_compartments` for
    consumers who want alternative semantics.
  - **SCI storage canonicalization.** Post-Phase-B, `SciSet`
    (lattice form) is the canonical page-context storage.
    `IsmAttributes::sci_controls` (flat CVE enum projection) stays
    populated for rules that currently read it; a Phase-B migration
    note in CLAUDE.md says "new rules read `sci_markings` /
    `SciSet`; `sci_controls` is a compatibility view scheduled for
    removal in Phase C or D when no rule references it."
  - `PageContext::expected_*` methods rewired to call through
    `scheme.project(Scope::Page, ...)`. Public API unchanged.
  - `PageRewrite` table per §7a. CAPCO declares NOFORN⊐REL TO as
    the first entry.
  - Expansion tables (FVEY, ACGU, NATO tetragraph membership) live
    in `marque-capco::vocab` as hand-curated `&'static [...]`
    const tables. Not derived from ODNI XML (the membership data
    is not in CVE files). Unit-tested with exhaustive fixtures.
  - `AggregationOp::Custom` retired from the runtime path.
- **Verification.** Full CAPCO corpus equivalence run + existing
  Phase A equivalence tests pass byte-identical before/after. New
  property tests for `SciSet` / `SarSet` / `FgiMarker` lattice laws
  (modulo the §3.3a meet policy).
- **Gate.** ≥95% per-rule accuracy (SC-002/SC-003) preserved.
  Strict-path p95 ≤16 ms on 10 KB preserved.

### Phase C — Declarative constraints + rewrites

- **Goal.** Constraint-style CAPCO rules (NOFORN∥REL TO, RD⇒NOFORN,
  JOINT⇔FGI, HCS system rules) move to declarative `Constraint`
  data; page-level rewrites move to `PageRewrite`. Hand-written
  constraint rules retire.
- **Deliverables.**
  - `Constraint` enum completed (Phase A sketch): `Conflicts`,
    `Requires`, `Implies`, `Supersedes`, `Custom(&'static str)`.
  - **Shared constraint evaluator** in `marque-scheme` (not
    per-scheme): `fn evaluate(constraints: &[Constraint<S>], m:
    &S::Marking) -> Vec<ConstraintViolation>`. Replaces the
    per-variant match in `CapcoScheme::validate`.
  - ~15 of the 39 CAPCO rules rewritten as `Constraint` + `PageRewrite`
    entries (depending on shape). The remaining rules are
    non-constraint rules (banner-abbreviation preference, etc.) and
    stay as `Rule` impls.
  - Cross-category supersession (NOFORN⊐REL TO) moves from
    `PageContext::expected_rel_to` into a `PageRewrite` declaration.
    The `TODO(Phase C)` comment in `crates/capco/src/scheme.rs`
    line 380 is resolved.
    - **`PageRewrite` axis annotations** (refactor of the
      Phase-B-shipped type). `PageRewrite` gains
      `reads: &'static [CategoryId]` and
      `writes: &'static [CategoryId]` fields per §7a. Two constructors:
      `PageRewrite::declarative` derives the annotations from the
      predicate/action enum variants at scheme build time;
      `PageRewrite::custom` requires explicit declaration. The
      Phase-B-shipped NOFORN ⊐ REL TO entry switches to
      `PageRewrite::declarative` (annotations derived:
      `reads = &[dissem]`, `writes = &[rel_to]`). Existing CAPCO behavior
      unchanged; the corpus accuracy harness must stay byte-identical
      across the refactor.
    - **`CategoryAction::Promote`** variant added per §7a. Move content
      from one axis to another, optionally transforming the value during
      the move. Used by JOINT-promotion below.
    - **Engine-side scheduler** per §7a.2: topological sort over the
      read/write dependency graph, run at `Engine::new` not per-document.
      `EngineConstructionError::RewriteCycle` and
      `EngineConstructionError::UnannotatedCustomAxes` cover the two
      static failure modes. Unit tests in `marque-scheme` schedule (a)
      the pre-Phase-C single-entry rewrite set (trivial schedule),
      (b) the full Phase C rewrite set (three entries, real
      producer-consumer edge), and (c) a synthetic 4-rewrite cyclic set
      to exercise the cycle path.
    - **JOINT-promotion** (`capco/joint-promotes-foreign-to-fgi`)
      declared as a `PageRewrite` via `PageRewrite::declarative`, using
      the new `CategoryAction::Promote { from: classification, to: fgi,
      transform }` variant. Trigger:
      `CategoryPredicate::Contains { category: classification, token:
      US_PRESENCE }`, where `US_PRESENCE` is a **planned Phase C
      placeholder name** for a structural marker emitted/derived by the
      parser when any portion of the page carries US classification,
      including as the US member of a JOINT marking. This is not a
      current token name in the codebase; the implementation should use
      the existing CAPCO sentinel-token mechanism (today including
      tokens such as `TOK_USA` and `TOK_JOINT`) or introduce a clearly
      named equivalent when Phase C is implemented. Citation: CAPCO §J
      — confirm exact subsection in the PR.
    - **FGI-absorption** (`capco/unattributed-fgi-absorbs-attributed`)
      declared as a `PageRewrite` via `PageRewrite::custom` with explicit
      `reads = &[fgi]` / `writes = &[fgi]`. The within-axis collapse
      isn't expressible as `Clear` or `Replace` so the action is
      `Custom`; the trigger is `Custom` because "unattributed" is a
      structural marker on the FGI value rather than a literal token.
      Citation: CAPCO §E — confirm exact subsection in the PR.
    - **`R-FgiExplicitRel` property test.** A portion contributing
      `FGI(unattributed)` to the page-level FGI axis, alongside any
      portion with explicit REL TO content, must produce a banner that
      retains the explicit REL TO and does *not* introduce NOFORN. Locks
      in the §7a.1 invariant that absorption is FGI-axis-only and does
      not propagate restriction to `dissem`. Test corpus includes the
      three-portion canonical example from §7a.1.
    - **Scheduler exercise.** Tests verify (a) swapping declaration
      order of the FGI pair produces the same scheduled run order, (b)
      inserting NOFORN ⊐ REL TO at any position in the declaration list
      doesn't change either the schedule or the banner output
      (independence verified), and (c) introducing a synthetic
      read-after-write cycle at scheme build time fails with
      `RewriteCycle` and names both members.

    Phase C verification adds `R-FgiExplicitRel` and the scheduler
    exercise to its accuracy gate. Phase C gating still requires
    ≥95% per-rule accuracy on SC-002/SC-003 with no regression from the
    Phase-B-shipped CAPCO behavior on the existing NOFORN ⊐ REL TO entry
    across the `PageRewrite` refactor.

- **Verification.** Equivalence test: same diagnostics produced on
  the corpus before/after the rule migration.
- **Gate.** Rule count reduction does not introduce regression on
  SC-002/SC-003.

### Phase D — Probabilistic decoder (moved earlier than prior plan)

- **Goal.** Ship the decoder end-to-end so mangled inputs fix
  automatically with provenance. This is the `marque-detect` work
  the prior plan deferred to Phase E; promoting it here because CUI
  (Phase F) will exercise it heavily, and the `Parsed::Ambiguous`
  plumbing needs real end-to-end traffic to stabilize.
- **Deliverables.**
  - `Recognizer` trait + `StrictRecognizer` adapter in
    `marque-engine` per §5.1a. `Engine::lint_inner` drives through
    `Recognizer::recognize`.
  - `DecoderRecognizer` with candidate generator for CAPCO: bounded
    edits to observed tokens, K = 8 per template, `Send + Sync` per
    §5.2. Corpus-derived priors baked into the build per §5.4.
  - `--deep-scan` CLI flag + server batch-endpoint option. G7a
    enforced: interactive authoring paths don't invoke the decoder
    unless the flag is set.
  - `FixProposal::Confidence` struct per §6.1. Engine aggregate
    score drives fix/threshold decisions.
  - `FixSource::DecoderPosterior` variant added (existing variants
    retained).
  - **Audit schema bump** to `marque-mvp-2` per §6.1. Migration note
    shipped in PR.
  - **Threat-model enforcement** per §6a:
    - T1: strict-path classification anywhere in document
      suppresses decoder consideration of `(C)` as copyright.
    - T2: `FeatureId` as an enum, not a free string; CI test caps
      audit-record `features[].label` length and charset.
    - T3: `--corpus-override` available on CLI only; server
      binary rejects override from HTTP; WASM build fails at
      compile time if an override codepath is introduced.
  - **Mangled-marking corpus fixture.** `tests/fixtures/mangled/`
    with **N ≥ 200** cases labeled with expected marking,
    mangling class (typo / reordering / missing-delimiter /
    superseded-token / wrong-case / garbled-delimiter), and
    source confidence. Generator script in
    `tools/corpus-analysis/` produces the fixture from the
    Enron corpus's high-confidence markings by applying known
    mangling transforms.
- **Verification.**
  - New accuracy metric: "mangled-marking resolution rate" with
    per-confidence-bucket error rates. Target ≥ 85% resolution at
    aggregate confidence ≥ 0.85; per-bucket error budgets
    documented in the PR.
  - `marque-mvp-1` records from pre-Phase-D continue to parse in
    downstream consumers (back-compat verified).
- **Gate.**
  - Strict-only p95 ≤ 16 ms preserved (§2.2).
  - Decoder p95 ≤ 18 ms on 10 KB with one mangled region
    (worst-case per §2.2).
  - ≥ 95% per-rule accuracy on clean corpus (SC-002/SC-003)
    preserved.

### Phase E — Vocabulary consolidation + codec scaffolding

- **Goal.** Full ISM metadata surfaced through `Vocabulary` +
  provisional codec traits. Also: remove the `FOUO → CUI`
  migration that was never correct CAPCO policy (see §14).
- **Deliverables.**
  - `marque-ism::build.rs` dual codepath per §4.4. JSON for term
    data, XML for XSD/Schematron artifacts.
  - `TokenMetadataFull` const tables; `Vocabulary` trait + CAPCO
    implementation. Rules that consume metadata (authority,
    owner/producer, deprecation replacement) migrate to
    `Vocabulary` queries; legacy opaque-`TokenId` paths stay as
    fallbacks during migration.
  - `Codec<S>` trait in `marque-scheme`. No implementations yet —
    the surface is pinned so Phase G can wire XML/JSON round-trip.
  - **FOUO → CUI migration entry removed** from
    `crates/ism/build.rs`. FOUO remains valid in CAPCO ISM (active
    in `DissemControl`). Demo-scene and README updated to use a
    different migration (`NF → NOFORN`) for scene 1. Any future
    "suggest CUI for FOUO" behavior lands in Phase F via config
    gates.
- **Verification.** Existing CAPCO corpus runs with no change in
  diagnostic set after the FOUO migration is removed (verify
  `cargo test -p marque-capco` stays green; the `FOUO drops in
  classified` tests in `crates/ism/tests/rollup_golden.rs` are
  unrelated to the migration and must stay green).
- **Gate.** `Vocabulary` adoption preserves ≥95% per-rule accuracy.

### Phase F — CUI as second scheme

- **Goal.** Implement NARA CUI as a second `MarkingScheme` to
  validate genericity.
- **Deliverables.**
  - `marque-cui` crate, separate from `marque-capco`. Depends on
    `marque-scheme` only.
  - CUI's ~125 categories encoded declaratively via built-in
    lattice constructors where possible; custom `impl Lattice`
    where not.
  - Engine gains multi-scheme dispatch: a document declares its
    governing scheme via `.marque.toml` (`[scheme] name = "capco"`
    or `"cui"`), explicit header, or auto-detection fallback.
    Mixed-scheme documents (Q4) defer to Phase I — one scheme per
    document in Phase F.
  - **Agency / CUI config gates** (M-FouoConfig):
    - `[agency] is_ic_member = true | false` in `.marque.toml`.
      IC members keep FOUO unconditionally. Non-IC agencies with
      CUI adoption surface the migration suggestion.
    - `[cui] migrate_fouo = false` — explicit override. Default
      `false` (conservative).
    - The migration suggestion surfaces through the CUI adapter,
      not through `marque-capco`. CAPCO stays CAPCO.
  - If CUI exposes expressiveness gaps in the trait surface, they
    are addressed here and back-ported to `marque-scheme`.
- **Verification.** CUI corpus accuracy harness (new) ≥ 95%
  per-rule. CAPCO corpus regression guard.
- **Gate.** No `marque-capco` diagnostic shape changes.

### Phase G — ControlBlock trait + CAB derivation

- **Goal.** Generalize CAB handling. CAPCO and CUI both implement
  `ControlBlock`. Deterministic CAB derivation lands for
  XML-metadata sources.
- **Deliverables.**
  - `ControlBlock` trait per §8.
  - `CapcoCab` and `CuiCab` concrete impls. `PartialCab` /
    `CabSuggestion` / `SuggestionDisposition` per §8.
  - `derive_from` with provenance tracking: XML/JSON ISM metadata
    → full derivation (empty `required`); plain text → partial
    with explicit user fields and `NeedsConfirmation` suggestions.
  - CAB-validating rules move to `ControlBlock::validate` + thin
    rule adapters.
- **Verification.** Existing CAB diagnostics preserved; new
  XML-metadata-driven derivations covered by golden tests.
- **Gate.** ≥95% per-rule accuracy on CAB-bearing corpus entries.

### Phase H — Diff rules and proactive feedback

- **Goal.** Two-marking comparison rules using the lattice algebra.
- **Deliverables.**
  - `DiffInput<M>` + `DiffRelation` per §7.1. CLI and server diff
    entry points.
  - Rules: reply-weakens-parent (email thread analysis),
    banner-doesn't-cover-portions (re-expressed via the diff
    primitive), CUI re-disclosure check, historical-marking
    supersession.
  - Warnings only. No auto-fix on diff rules; resolution depends
    on author intent.
- **Verification.** Diff-rule test suite with curated
  email-thread fixtures.
- **Gate.** No false-positive regression on single-document corpus
  (diff rules must not fire on single-document input).

## 13. Open questions

Called out explicitly so we don't silently drift.

- **Q1. Confidence threshold interaction with `Severity`.** A rule at
  `severity: fix` combined with aggregate confidence 0.72 — fix?
  warn? Probably product of confidence × severity-weight against a
  single threshold, but the weighting scheme needs corpus validation
  before it's frozen. Currently a hand-wave.
- **Q2. Edit-distance cutoff for decoder candidate generation.**
  Likely 1–2 edits, with a per-token cap based on token length and
  base rate. Concrete table derived from the corpus; set in Phase D.
- **Q3. Baked-in vs runtime frequency tables.** Baked in by default;
  override path via config. Open: file format for override (CSV?
  JSON? MessagePack?).
- **Q4. Multi-scheme documents.** A document with both CAPCO portion
  markings and CUI banners — rare but possible. Phase F decision:
  one scheme per document with override. Mixed-scheme documents
  defer to a future Phase I. The engine's multi-scheme dispatch in
  Phase F makes the single-scheme-per-document constraint
  explicit in config; mixed-scheme handling is additive.
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
- **Q8. WASM corpus override: forbidden.** The WASM target never
  accepts a runtime-supplied frequency table. The only corpus
  table in a WASM build is the one baked at compile time. Q3's
  override path applies to CLI self-operator mode only (see T3 in
  §6a). This is settled as of this revision; listed here so the
  constraint is visible to anyone touching the WASM build.

## 14. What we dropped

For the record, so future readers don't re-propose them.

- **`AggregationOp` as a runtime dispatch point.** Preserved as
  build-time shorthand (`Category::flat_set(...)`) and runtime
  metadata via `CategoryShape`; retired from the hot path.
- **Edit-distance variants in the Aho-Corasick automaton.** See §5.5.
- **Extending the `MarkingScheme` trait with CAB methods.** CABs
  aren't markings and don't derive purely from markings; they get
  their own trait.
- **Deriving deprecation replacements from ODNI XML annotations.**
  The Schematron files encode validity predicates, not policy
  judgments about which codeword replaces which. Deprecation policy
  stays hand-maintained in `marque-ism/build.rs::MIGRATIONS`. See
  §4.2.
- **Runtime corpus override in WASM.** Q8 / T3 in §6a.
- **The `FOUO → CUI` migration in `crates/ism/build.rs`.** This
  entry was factually wrong: the IC never transitioned off FOUO,
  FOUO remains valid in CAPCO ISM, and CUI is a separate marking
  system under NARA jurisdiction. Phase E removes the entry. Any
  future "suggest CUI for FOUO on non-IC documents" behavior lands
  in Phase F via the `[agency] is_ic_member` / `[cui]
  migrate_fouo` config gates (M-FouoConfig), not as a blanket
  CAPCO-level migration.
- **`Scope::Diff { from, to }` as a variant.** Embedding references
  in the `Scope` enum forces every scope to carry a lifetime and
  burdens the 99% of call sites that don't care about diff.
  Replaced by the dedicated `DiffInput<M>` type per §7.1. `Scope::Diff`
  survives as a marker variant only.
- **Confluence as a stronger property than topological-acyclic
  scheduling.** Acyclic + topo order suffices for determinism;
  confluence (any order produces the same result) is a stronger
  claim the scheduler doesn't verify and CAPCO's rewrite set doesn't
  need. Property tests cover the actual combinations end-to-end;
  confluence-as-an-invariant would be over-specification.
- **Per-document `EngineContext` for context-conditional rewrites**
  (e.g., `[agency] is_ic_member` gating a rewrite on/off). The
  `page_rewrites()` signature returns a static slice; threading
  per-document context through the rewrite phase is an additive
  change Phase F's CUI work is the natural place to consider.
  Deferred to whatever doc covers Phase F config gates.
- **Parser-side ambiguity** (jumbled SCI tokens with unpublished
  symbols, country lists detached from their parent identifier).
  Already covered by §5's `Parsed::Ambiguous { candidates }` plus
  aggregate-confidence shape. The parser-side and rewrite-side
  ordering questions are different concerns.
- **Renaming `Lattice::meet` on `SciSet` to flag the policy choice.**
  Flagged in review as a documentation hazard (the trait method is
  the obvious thing to reach for, but `SciSet::common_compartments`
  is usually what callers want). Worth doing, but orthogonal to the
  scheduling work in this doc; either land it in the same Phase C PR
  or in a follow-up note.
- **A separate "Phase B.1" cleanup PR for the scheduler
  infrastructure alone, ahead of Phase C.** Considered; rejected on
  the grounds that the scheduler infrastructure has no consumer
  until JOINT-promotion and FGI-absorption land, so shipping it
  alone would introduce dead code with no behavioral test coverage.
  The Phase C PR is the right place for both.

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
