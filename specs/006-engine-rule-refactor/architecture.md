# Architecture Restatement: Bag-of-Tokens

**Status.** Architectural commitment. Not a plan, not a migration spec. Grounds any future plan that touches the engine, the scheme surface, or the rule architecture.

**Date.** 2026-05-09.

**Provenance.** Recovered after PR 3c.2 stalled on a rule-by-rule migration that drifted from the pivot-type triple committed to in PR 3a (see `docs/plans/2026-05-02-engine-refactor-consolidated.md` Appendix D and the recursive-lattice design at `docs/plans/2026-04-19-recursive-lattice-and-decoder.md`). The drift introduced span-surgery as the fix model and a 17-variant `CapcoRenderDirective` enum to dispatch renderer canonicalization from rule call sites. This document re-grounds the architecture in the framing the codebase already had at PR 3a/3b — fact-set propagation through the lattice, with the renderer as the single source of canonical form.

---

## The pipeline

```text
bytes ──▶ ParsedAttrs<'src> ──▶ CanonicalAttrs ──▶ ProjectedMarking ──▶ bytes
        (parser)            (canonicalize)      (project)            (render_canonical)
```

Four functions. Three intermediate types. Two trait surfaces (`MarkingScheme::canonicalize`, `MarkingScheme::render_canonical`). Every other engine concern — rules, constraints, page rewrites, audit records, decoder, fix proposals — sits beside this pipeline, not inside it.

The pipeline is the architectural backbone. Crate boundaries, trait surfaces, type definitions, and audit-record content are derived from it.

---

## The four stages

### Parser: `bytes → ParsedAttrs<'src>`

Delivers what the input contains. Tokens, recognized and unrecognized; spans into the source buffer; structural shape (portion vs banner vs CAB; SCI compositional grammar per §A.6; SAR program/compartment/sub-compartment hierarchy; FGI / JOINT / NATO classification forms). Borrows from the source buffer (`'src`).

The parser does not normalize, canonicalize, or correct. It commits to nothing about meaning. An `EXDIS//REL TO USA` portion parses to a token set including both `EXDIS` and `REL TO USA` — the parser does not adjudicate whether that combination is admissible.

The parser's two outputs that downstream stages depend on:
1. The token set (the "bag of tokens"), with structural typing where the grammar admits structure (e.g., `SciMarking` rather than raw bytes).
2. Spans into the source buffer, for diagnostic location reporting only.

Open-vocabulary axes (SAR program identifiers, custom SCI compartments, FGI tetragraphs) are delivered with their **structural** form intact — `SarProgram { identifier, compartments, sub_compartments }`, not raw `Box<str>`. Open-vocab does not mean "shapeless"; it means "extensible within a known shape."

### Canonicalize: `ParsedAttrs → CanonicalAttrs`

The CAPCO scheme's `MarkingScheme::canonicalize` GAT lifts parsed tokens into canonical fact-set form: registered tokens to their CVE entries; unregistered structurally-typed tokens preserved with their structure; deprecated forms migrated forward (per the ODNI migration table); historical aliases (e.g., `EYES` standalone) resolved against the page context.

Owned data; no source-buffer lifetime. Per-portion shape; no page-level reconciliation has happened yet — `CanonicalAttrs` is "this portion's facts, normalized."

What this stage does NOT do:
- Cross-axis reasoning (NOFORN clearing REL TO; NODIS implying NOFORN).
- Page-level joins (banner = join of portions).
- Page-level rewrites.

Those live in the next stage.

### Project: `Vec<CanonicalAttrs> → ProjectedMarking`

The lattice operates here. Three operations compose:

1. **Per-axis joins** via `Lattice::join` for each marking category. Class is the chain max. SCI/SAR sets are component-wise unions over their structural hierarchies. REL TO is intersection (`IntersectSet`). Dissem is union with intra-axis supersession (`SupersessionSet`). FGI is the bounded `FgiSet` lattice with concealed-supersedes-acknowledged.

2. **Closure operator** — implied-fact propagation. NODIS implies NOFORN. EXDIS implies NOFORN. HCS-O implies NOFORN + ORCON. Per-token classification floors (HCS-comp-sub requires class ≥ TS; SAR requires class ≥ C; SI requires class ≥ C). The closure runs to fixpoint (it's monotone and the per-axis lattices are finite-height up to open-vocab carriers).

3. **Page rewrites** in topological order over their `reads`/`writes` annotations. `noforn-clears-rel-to`. The transmutation roster (see `marque-applied.md` §3.4.1, mirrored from the user's structural map): bare-FGI-on-US-class promotion; FGI-R class-floor; JOINT-on-mixed promotion; FRD-SIGMA + RD-SIGMA consolidation; ORCON-NATO on US contact; LES-NF / SBU-NF transmutation. Each is a deterministic non-lattice rewrite (Framing 3 per `security-lattice.md` §6).

The output is `ProjectedMarking`: **the canonical interpretation of the page** — what the marking system says the page actually means once portions are reconciled, implied facts propagated, and rewrites applied.

### Render: `ProjectedMarking → bytes`

The renderer (`MarkingScheme::render_canonical`) emits the canonical form. It chooses delimiters (`/` within category, `//` between categories per §A.6 Figure 2, comma between trigraphs). It chooses sort orders (REL TO USA-first then alpha; SCI compartment numeric-then-alpha; SAR program ascending). It chooses abbreviations (banner long form vs portion abbrev). It chooses banner roll-up shape (full list, abbreviated list, FGI [LIST] vs concealed `FGI`, JOINT-on-mixed eviction).

Form lives here. **Two `ProjectedMarking`s that are lattice-equal render to byte-identical output.** Two pages whose inputs differ only in form (`/` vs `,`; alphabetical vs USA-first; `EYES` vs `EYES ONLY`) project to the same `ProjectedMarking` and render to the same canonical bytes.

The renderer is the single source of canonical form. There is no second canonicalization layer above it (no closed enum of `directive` variants dispatched from rules; no per-rule rebuild fragments embedded in `Rule::check` bodies). The renderer encodes the form rules directly, expressed as per-axis canonicalization functions.

---

## What rules are

A rule is a **divergence detector**. Its job is to compare two views of the page:

- the input view: `Vec<CanonicalAttrs>` — what the page's portions say after canonicalization
- the canonical view: `ProjectedMarking` — what the marking system says the page means

A divergence on any axis is a diagnostic. The rule emits the citation, the message, and (optionally) a structural delta to reach canonical form.

A rule's signature, schematically:

```rust
fn check(
    parsed:    &Vec<CanonicalAttrs>,
    projected: &ProjectedMarking,
    ctx:       &RuleContext,
) -> Vec<Diagnostic>;
```

Rules do not:

- Operate on `bytes`. That's the parser's input and the renderer's output.
- Operate on `ParsedAttrs<'src>` directly. That's pre-canonical; the lifetime alone disqualifies it from rule signatures.
- Construct replacement strings. The renderer is the single source of canonical form.
- Do span surgery. The renderer has the canonical projection and renders from it.

A rule that "moves a declassify token to the CAB" is a divergence detector on the declassify axis: input says declassify is in the banner; projection says it should be in the CAB; emit the diagnostic, suggest re-render. The rule does not move bytes; the renderer renders from the projection.

A rule that fires on `EXDIS//REL TO USA` is a divergence detector on the REL TO axis: input says REL TO contains USA; projection says REL TO is empty (because EXDIS implies NOFORN via closure, and `noforn-clears-rel-to` clears REL TO); emit the diagnostic citing §H.9, suggest re-render. No multi-span surgery; no "intent ambiguity" — the canonical interpretation of the page is unambiguous.

---

## What fixes are

Three kinds, structurally complete.

### `FactAdd { token, scope }`

Add a token to the fact set on `scope`. Repairs `Constraint::Requires` violations:
- "HCS-O without NOFORN; add NOFORN."
- "SI compartment without classification ≥ C; raise class to C."
- "Class-floor violation: HCS-comp-sub requires TS; raise."

Closed vocab (`token` references either a CVE atom or a structurally-typed open-vocab atom from canonicalize). Renderer re-renders from the new projection.

### `FactRemove { token_ref, scope }`

Remove a token from the fact set on `scope`. Repairs `Constraint::Conflicts` violations:
- "RELIDO with NOFORN; drop RELIDO."
- "EXDIS with NODIS in same portion; drop EXDIS (NODIS supersedes)."
- "ORCON-NATO with US class; drop ORCON-NATO (transmutes to ORCON via the rewrite)."

`token_ref` identifies the token in the fact set, not bytes in the input. The renderer renders without it.

### `Recanonicalize { scope }`

Input form diverges from canonical form on `scope`. The fact set is correct; the rendering isn't. Subsumes:
- Delimiter normalization (`,` → space in JOINT lists; `////` → `//`; `//` → `/` between same-category siblings).
- Sort canonicalization (REL TO USA-first; SCI compartment numeric-then-alpha; SAR program alpha; AEA SIGMA numeric).
- Abbreviation canonicalization (`EYES` ↔ `EYES ONLY`; portion abbreviation; banner full form).
- Block reordering (CAPCO §A.6 ordinal sequence).
- Banner roll-up form (FGI [LIST] vs concealed; JOINT-on-mixed eviction; deduplication of REL TO trigraphs).

The renderer re-renders the scope. No directive payload; the renderer canonicalizes per axis and per scope by construction.

### Type sketch

Illustrative shapes — names and lifetimes may differ in the final implementation; the structural commitment is the variant set and the field types.

```rust
// In marque-rules. Generic over the marking scheme so FactRef and the
// open-vocab carrier are scheme-specific without leaking domain knowledge
// into the engine. Closed enum; no `__Migration` escape hatch.
pub enum FixIntent<S: MarkingScheme> {
    FactAdd        { token:     FactRef<S>,   scope: Scope },
    FactRemove     { token_ref: FactRef<S>,   scope: Scope },
    Recanonicalize { scope:     RecanonScope                 },
}
// Note: BOTH FactAdd and FactRemove name tokens via FactRef<S> — the
// fact-set-position type. `TokenRef<S>` (a constraint-query type in
// `marque-scheme::constraint` covering `Token | AnyInCategory`) is the
// wrong shape here: a rule cannot meaningfully emit `FactAdd { token:
// AnyInCategory(...) }` ("add any token in this category"). Both
// emission variants identify a specific lattice entry.

// FactRef identifies a token in the fact set, NOT bytes in the input.
// This is what makes FactRemove source-buffer-agnostic: the engine names
// what to remove by its place in the projected lattice, never by an
// input span.
pub enum FactRef<S: MarkingScheme> {
    /// CVE-registered token; resolves to a unique entry in the fact set.
    Cve(TokenId),
    /// Open-vocab structural reference (SAR program identifier;
    /// SCI compartment / sub-compartment path; FGI tetragraph).
    /// The scheme's canonicalize step produces these; the renderer
    /// consumes them. `S::OpenVocabRef` is a scheme-side associated
    /// type union covering its open-vocab carriers.
    OpenVocab(S::OpenVocabRef),
}

// RecanonScope is a narrowing of marque-scheme's Scope. `Scope::Diff`
// is not a meaningful recanonicalization target (Diff is a rule-context
// query mode, not a projection-output scope), so the renderer's accepted
// scope set is the three positional variants only.
pub enum RecanonScope { Portion, Page, Document }

// AppliedFix.proposal is the engine-promoted form. The engine snapshots
// runtime state (timestamp, classifier identity, dry-run flag) onto the
// rule's pure-data FixIntent at promotion time. The rule never carries
// runtime context; the engine never carries domain logic.
pub struct AppliedFix<S: MarkingScheme> {
    pub rule_id:        RuleId,
    pub proposal:       FixIntent<S>,        // structural fact-set delta
    pub confidence:     f32,                 // engine compares vs config threshold
    pub timestamp:      SystemTime,          // engine-snapshotted
    pub classifier_id:  Option<ClassifierId>,
    pub dry_run:        bool,
}
```

The sketch encodes four invariants the architecture commits to:

- **No `Box<str>` payloads in fixes.** `FactRemove` names tokens via `FactRef`, not via input-derived bytes. This is what G13 audit-content-ignorance (Constitution V Principle V) requires; the audit record stores structural references, never document content.
- **No multi-span carriers.** Each variant operates on one `scope`. Multi-span document-level rewrites (declassify-token relocation across banner→CAB; banner-rebuild from portion projections) are *projection plus re-render*, not multi-span fix payloads — the projection handles scope coverage, the renderer handles the bytes.
- **`Scope` is the lifetime boundary.** Rules emit `FixIntent` with no source-buffer lifetime; the engine's `AppliedFix` owns its data outright. Spans live only on `Diagnostic` for location reporting, never on fix payloads.
- **Scheme-generic dispatch.** The engine routes `FixIntent<S>` through the scheme's `render_canonical` and `project` traits; the same engine code serves CAPCO today, CUI / NATO / partner-national tomorrow without specialization.

That is the full vocabulary. Three variants. No `Box<str>` payloads. No multi-span carriers. No 17-variant taxonomy. The directive enum was the wrong abstraction layer.

---

## What was lost during PR 3c.1

The directive-enum design re-introduced span surgery as the fix model. It positioned rules as constructors of `(span, replacement_bytes)` pairs and required a closed enum (`CapcoRenderDirective`) to dispatch renderer canonicalization machinery from rule call sites. Every consequence of that choice — open-vocab `Box<str>` payloads where TokenId failed; multi-span carrier gaps; two-branch rules that don't fit a single row; "intent-ambiguous" findings on operations the lattice already disambiguates — is the foundation telling the migration that the directive enum is the wrong abstraction layer.

The pivot-type triple (`ParsedAttrs<'src>` / `CanonicalAttrs` / `ProjectedMarking`) was always the answer. The lattice, the closure operator, the PageRewrite scheduler, the renderer trait surface — all the machinery already exists or was correctly scoped. What got mis-designed in PR 3c.1 was the type that flows from rules to the engine for fix promotion: it was modeled on input-byte surgery (closed enum with `Box<str>` escape hatches) instead of fact-set delta + re-render.

---

## The §3.0.b purpose split — where each rule lives

A rule's purpose determines its home. The purpose split is from `marque-applied.md` §3.0.b, derived from the user's prior structural-map consultation; it predates this document and is the reference taxonomy.

| Purpose | Home | Emission |
|---|---|---|
| Banner = join of portions (correctness) | property test on `Lattice::join` impl | none — disappears |
| Cross-axis transmutation (NOFORN clears REL TO; FGI rollup; JOINT promote; ORCON-NATO transmute; LES-NF / SBU-NF transmute) | `PageRewrite` declarative entry on `CapcoScheme` | none — runs in projection |
| Mutual exclusion (RELIDO ∦ NOFORN / FGI / JOINT / NATO; NODIS ∦ EXDIS portion) | `Constraint::Conflicts` declarative entry | `FactRemove` |
| Implication / requirement (HCS requires NOFORN; per-token classification floor) | `Constraint::Requires` declarative entry | `FactAdd` |
| Form (delimiter, sort, abbrev, separator collapse, RELOPT round-trip / auto-collapse) | `render_canonical` body | `Recanonicalize` |
| Admonition / warning notice (RD warning, RAWFISA notice, IMCON SAT warning) | admonition emitter (separate channel) | n/a |
| Decoder / corrections (mangled tokens, EYES-without-list historical, fuzzy match) | recognizer (`StrictRecognizer` / `DecoderRecognizer`) | already R001 |

Rules that don't fit one of these purposes don't belong in the rule catalog. `Constraint::Custom` exists for genuinely n-ary or context-dependent invariants; it is a rare exception, not a junk drawer.

---

## The "form is not shape" principle (§3.0.a)

Two markings that differ only in delimiter, sort order, abbreviation, or inter-category position are **lattice-equal** on every axis. The renderer chooses one canonical representative. Form-divergence is a renderer-correctness concern, not a lattice-axis concern and not a constraint-catalog concern.

Practical consequence: most of the 47 hand-written rules' fix logic (delimiter normalize, sort canonical, abbreviate-or-expand, separator collapse, JOINT-USA-first, REL TO trigraph-list canonicalization) collapses into the renderer body. They retire as rules; their canonicalization knowledge moves into `render_canonical`.

---

## What this commits us to

- The pipeline shape — bytes → parsed → canonical → projected → bytes — is the architectural backbone. Every type, trait, and crate boundary respects it.
- `ProjectedMarking` is the source of truth for "what the page means." Rules read it; renderers consume it; audit records reference it (token canonicals, category IDs, posterior scalars, BLAKE3 digests — never document content; G13 closure preserved).
- Rules are pure divergence detectors: stateless, side-effect-free, fact-set-delta-emitting, `Send + Sync` (Constitution VI).
- Fixes are structurally typed at the fact-set level (`FactAdd` / `FactRemove` / `Recanonicalize`), not at the byte-span level.
- The renderer (`render_canonical`) is the single source of canonical form. Form rules retire into it.
- Open-vocabulary axes (SAR program IDs, SCI compartments beyond pre-registered compounds, FGI tetragraphs) carry their structural form (typed structs, not raw bytes) through canonicalize and project; the renderer canonicalizes them per axis.

## What this does NOT commit us to

- A migration order, sub-PR sequence, or timeline.
- A retirement schedule for `__Migration` / `legacy_text` / `legacy_migration` / `from_proposal_legacy` escape hatches.
- Specific `MessageTemplate` variants or `MessageArgs` field choices.
- Specific `RuleId` schemes, numbering, or the legacy-rule-id mapping.
- How walker rules (E031 / E058 / E059 / E060) retire. Their end-state is "absorbed by the renderer plus per-row Constraint declaratives," but how they get there is a planning question.
- Specific primitive extensions on `marque-scheme` (`Constraint::RhsFamily`; new `PageRewrite` trigger shapes; etc.).

These are planning questions. This document is the structural commitment they ground in.

---

## Architectural invariants this restates

The Constitution principles already in force, restated in pipeline-shape terms:

- **Principle II (zero-copy / streaming):** Spans flow `'src`-borrowed through the parser; canonicalize is the lifetime boundary; projected and rendered output own their data. `IsmAttributes` field types use `Box<[T]>` at the canonical/projected boundaries.
- **Principle III (WASM-safe core):** The pipeline is WASM-safe end to end. Format extraction (`marque-extract`) sits before bytes enter the parser; it's not in the WASM build.
- **Principle IV (two-layer rules):** Layer 1 (generated CVE predicates from `build.rs`) feeds canonicalize. Layer 2 (hand-written rules) reads `Vec<CanonicalAttrs> + ProjectedMarking`. Declarative rules (`Constraint`s, `PageRewrite`s) are scheme-data, not rule code.
- **Principle V (audit-first):** `FactAdd` / `FactRemove` / `Recanonicalize` carry structural information only — token references, category IDs, scope tags. No document bytes. The engine's promotion path snapshots runtime state into `AppliedFix`. G13 closure preserved.
- **Principle VI (dataflow pipeline):** The four-stage pipeline is what Principle VI names. Each stage is independently testable. `BatchEngine` layers async concurrency above the per-document pipeline; rules and recognizers are `Send + Sync`.
- **Principle VII (crate discipline):** `marque-scheme` carries the lattice / scheme / projection / rewrite traits; `marque-ism` carries the pivot-type triple plus CVE vocabulary; `marque-rules` carries `Diagnostic` / `FactAdd` / `FactRemove` / `Recanonicalize` / `MessageTemplate` / `RuleContext`; `marque-capco` provides the scheme adapter, the constraint catalog, the rewrite catalog, and the rules; `marque-engine` orchestrates. No new edges.
- **Principle VIII (source fidelity):** Every `Constraint`, `PageRewrite`, and rule cites the authoritative passage in `crates/capco/docs/CAPCO-2016.md` (or the ODNI ISM schemas for tokens too new to be in the manual). Citations propagate through `MessageTemplate` to the diagnostic stream.

---

## Next concrete artifact

A rule-body audit binned by §3.0.b purpose, not by §3.4 fix shape. For each of the 47 rules:

1. Which purpose-row does it occupy (lattice property / page rewrite / Conflicts / Requires / form / admonition / decoder)?
2. What `ProjectedMarking` axis does it read?
3. Does the rule's existing fix logic match the purpose's emission shape (none / `FactAdd` / `FactRemove` / `Recanonicalize`)?
4. If form: what canonicalization does the renderer need to encode to absorb it?
5. If the rule serves multiple purposes, what's the natural split?

Output: one table, ~47 rows, ~5 columns. Rules whose purpose matches their current row become declarative entries plus optional pre-canonical divergence detection. Rules with structural mismatches get split, retired, or absorbed by the renderer. The end-state count falls out of the table; it is not a target.

That table is what the next plan starts from.
