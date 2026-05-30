# Data Model

Every new or changed type, grouped by crate and tagged with the phase it lands in, the issue/tier
it satisfies, and its WASM-safety status. Signatures are **sketches** — exact field types are
fixed during implementation against the current code. CUI-specific shapes are **source-pending**.

Legend: **WASM** = must compile to WASM (Constitution III); **native** = engine/integration only.

---

## `marque-scheme` (leaf — WASM; MUST NOT depend on `marque-ism`)

### Phase 0 — document-artifact node model

```rust
/// What kind of document-scoped artifact a node represents.
/// Domain-neutral; schemes map their own artifacts onto these.
#[non_exhaustive]
pub enum ArtifactKind {
    AuthorityBlock,      // CAB (CAPCO) / CUI designation block (CUI) — scheme decides shape
    DeclassifyInstruction,
    Notice,              // US-Person notice, distribution statement, etc.
    CaveatLayer,         // #128 second-banner-line caveats
    FrontMarking,        // document "classified up to" overall (#799)
}

/// The four-state node model. `P` is the scheme's parsed artifact payload.
/// Absence is a state, not a separate rule family (research D2).
pub enum ArtifactState<P> {
    Present(P),
    PresentNonCanonical(P),     // parsed but diverges from canonical form
    AbsentButRequired,          // an inbound requirement edge demands it
    AbsentNotRequired,
}

/// A scope-tagged, typed document-scope node carrying its state, its
/// value-derivation, and its inbound edges. `ArtifactPayload` lives on the
/// `SchemeArtifacts: MarkingScheme` extension trait (NOT on `MarkingScheme`
/// itself) so the frozen `MarkingScheme` surface stays unbroken — see
/// contracts/document-artifact.md for the additive-vs-breaking staging.
pub struct DocumentArtifact<S: SchemeArtifacts + ?Sized> {
    pub kind: ArtifactKind,
    pub scope: Scope,                       // Document or Bundle
    pub state: ArtifactState<S::ArtifactPayload>,   // ArtifactPayload from SchemeArtifacts
    pub derivation: ValueDerivation,        // how the value was computed
    pub inbound: Box<[DerivationEdge]>,      // declared edges into this node
    pub span: Option<Span>,                 // present-node source location; None when absent
}
```
*Satisfies*: FR-001, FR-002 (US1). *Issues*: #799, #420, #128. **WASM.**

> `Span` is **defined in `marque-scheme`** (`crates/scheme/src/span.rs`) and re-exported by
> `marque-ism`, so `DocumentArtifact` uses `marque_scheme::Span` directly with no leaf-boundary
> issue and no `Loc`/`ByteRange` workaround.

### Phase 0 — derivation edges & provenance axes

```rust
/// An inbound derivation relation into a DocumentArtifact node. Static
/// topology; firing is conditional (research D3).
pub struct DerivationEdge {
    pub id: EdgeId,                         // &'static-label, like RewriteId
    pub relation: DerivationRelation,
    pub reads: Box<[CategoryId]>,           // feeds the scheduler (writers-before-readers)
    pub writes: Box<[CategoryId]>,
    pub firing: FiringPredicate,            // always-declared; predicate gates firing (incl. mode)
}

pub enum DerivationRelation {
    Rollup,            // banner derived from portions
    Requirement,       // X present ⇒ Y required (notice-iff-token)
    SourceDerived,     // bundle → document (#823, reserved)
    CannedString,      // §C.4/§C.5 mandated literal
    Passthrough,
}

/// Recognition provenance (adapter axis) — "how sure am I this span IS this
/// node?" Licenses fix-assertiveness. This is #176's InputSource promoted here.
#[non_exhaustive]
pub enum RecognitionProvenance { StructureRead, StructuredField, DocumentContent }

/// Value derivation (DAG-node axis) — "how was this node's VALUE computed?"
/// Drives the derivation record and emit-if-absent. Orthogonal to recognition.
#[non_exhaustive]
pub enum ValueDerivation {
    Authored,                 // OCA-authored (original CAB)
    DerivedMaxOverSources,    // #823 (reserved)
    MethodologyDriven,        // HUMINT → 50X1-HUM
    CannedPolicyString,       // §C.4/§C.5
    RolledUp,                 // from portions
}
```
*Satisfies*: FR-010, FR-014, FR-061 (reserved), D5. *Issues*: #799, #823. **WASM.**

### Phase 0 — `Scope::Bundle`

```rust
pub enum Scope { Portion, Page, Document, Bundle, Diff }   // Bundle is the new additive variant
```
Additive minor-version variant; `Scope` is intentionally not `#[non_exhaustive]` (research D11).
Reserves the #823 bundle→document edge. `RecanonScope` MAY gain `Bundle` later if needed.
*Satisfies*: FR-061, SC-005. **WASM.**

### Phase 0 — fix-intent reversibility pre-state (#824 rough-in)

`ReplacementIntent`/`FactRef`/`RecanonScope` live in `marque-scheme`
(`crates/scheme/src/fix_intent.rs`) — kept here, not in `marque-rules`, to avoid a scheme↔rules
dependency cycle.

```rust
pub enum ReplacementIntent<S: MarkingScheme + ?Sized> {
    FactAdd { token: FactRef<S>, scope: Scope /*, inverse implicit: remove token */ },
    FactRemove { facts: SmallVec<[FactRef<S>; 2]>, scope: Scope /* inverse implicit: re-add */ },
    Recanonicalize { scope: RecanonScope, prior: Option<RecanonPriorState<S>> },   // NEW reserved field
    Relocate { from: Scope, to: Scope, token: FactRef<S>, prior: RelocatePriorState<S> }, // NEW variant (D8)
}
pub struct RecanonPriorState<S: MarkingScheme + ?Sized> { prior_tokens: Box<[FactRef<S>]>, prior_span: Span, digest: [u8; 32] }
pub struct RelocatePriorState<S: MarkingScheme + ?Sized> { token: FactRef<S>, origin_span: Span, digest: [u8; 32] }
```
`RecanonPriorState`/`RelocatePriorState` carry only audit-permitted terms (token canonicals,
category IDs, `Span` offsets, BLAKE3 digests) — content-ignorant. Realization (the reversal pass +
`marque-3.x` audit-schema bump) is **deferred (#824)**; Phase 0 only lands the reserved fields so
the later bump is additive.
*Satisfies*: FR-060, SC-006. **WASM.**

### Phase A — input boundary

```rust
#[non_exhaustive] #[derive(Default)]
pub enum InputSource { #[default] DocumentContent, StructuredField, SchemaDocument }

pub struct InputContext<'a> {
    pub parse: ParseContext,         // existing recognizer context
    pub source: InputSource,
    pub adapter_label: Option<&'static str>,
    _phantom: PhantomData<&'a ()>,
}

pub trait InputAdapter<S: MarkingScheme>: Send + Sync {
    type Input; type Error: std::error::Error + Send + Sync + 'static;
    fn adapt(&self, input: &Self::Input) -> Result<S::Canonical, Self::Error>;
    fn adapt_document(&self, input: &Self::Input)
        -> Result<StructuredDocument<S>, Self::Error> { /* default: single layer */ }
    fn input_source(&self) -> InputSource;
}

pub struct StructuredDocument<S: MarkingScheme> { pub layers: Vec<DocumentLayer<S>> }
pub struct DocumentLayer<S: MarkingScheme> { pub canonical: S::Canonical, pub repair_kind: RepairKind, pub label: &'static str }
pub enum RepairKind { TextSpan, SchemaAttribute { field_path: &'static str }, StructuredEmit }
```
Also add `#[non_exhaustive]` to `ParseContext` (#176 staging step 1).
*Satisfies*: FR-030, FR-031, FR-032 (US4). *Issues*: #643, #641 T1-8, #176. **WASM** (trait surface;
concrete schema-reading adapters are native).

### Phase B — T3 naming de-coupling (additive-with-deprecation where feasible)

| Current (CAPCO-coupled) | New (domain-neutral) | File |
|---|---|---|
| `Zone::Cab` | `Zone::Custom(&'static str)` + `#[non_exhaustive]` (short-term alias `StructuralBlock`) | `recognizer.rs` |
| `ParseContext::classification_floor` | `rank_floor` (scheme-defined ordering) | `recognizer.rs` |
| `OwnerProducerKind::Nato`/`::Fgi` | `InternationalBody`/`ForeignGovernment` + `Custom(&'static str)` + `#[non_exhaustive]` | `vocabulary.rs` |
| `FormSet::{portion,banner}` | `{short_form,long_form,abbreviated_form}` | `vocabulary.rs` |
| `FormKind::IsmDescriptionTitle` | `StandardDescriptionTitle` | `vocabulary.rs` |
| `Vocabulary::is_fdr_dissem()` | move to `IcMarkingVocabulary` sub-trait | `vocabulary.rs` |
| `EmissionForm::{Portion,BannerTitle,BannerAbbreviation}` | `{ShortForm,LongForm,AbbreviatedForm}` | `render_context.rs` |
| `MarkingScheme::{render_portion,render_banner,project_banner}` | `{render_item,render_summary,project_summary}` (deprecated shims) | `scheme.rs` |
*Satisfies*: FR-025; #641 T3. **WASM.**

### Phase E — multi-scheme co-residence surface

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
```
The releasability `Product<CuiReleasability, CapcoIcDissem>` + monotone NOFORN closure is **not** a
new leaf type — it composes the existing `Product` constructor (`builtins.rs`) with a closure rule;
the cross-scheme wiring lives in `marque-engine` (model b, research D6/D7).
*Satisfies*: FR-021, FR-022, FR-025 (US3). *Issues*: #641 T1-7. **WASM** (trait surface).
**Source-pending**: `CuiReleasability` shape, LDC ordering, non-IC-control set.

---

## `marque-rules` (WASM)

### Phase B — generification (#641 T1-1/T1-2, T2-3/T2-4)

```rust
pub trait Rule<S: MarkingScheme> {
    fn check(&self, attrs: &S::Canonical, ctx: &RuleContext<'_, S>) -> Vec<Diagnostic<S>>;
    // id/name/default_severity/phase/additional_emitted_ids/trusted/cited_authorities unchanged
    fn target_zones(&self) -> Option<&'static [Zone]> { None }   // Phase F (#645 M2)
}
pub struct RuleContext<'a, S: MarkingScheme> { /* page_portions: Arc<[S::Canonical]>, page_marking: Option<S::Marking>, ... */ }
```
Remove `pub use marque_ism::{DocumentPosition, MarkingType, Zone};` re-exports. `MessageTemplate`
and `FeatureId` gain `#[non_exhaustive]` + `Grammar { grammar_id: &'static str, variant: u32 }`.
*Satisfies*: FR-020; #641 T1-1/T1-2/T2-3/T2-4. **WASM.**

> The fix-intent reversibility pre-state (#824 rough-in) is **not** in `marque-rules` —
> `ReplacementIntent`/`FactRef` are defined in `marque-scheme` (`crates/scheme/src/fix_intent.rs`,
> to avoid a scheme↔rules cycle). See that subsection under `marque-scheme` above.

---

## `marque-ism` (foundational vocabulary — WASM; MAY depend on `marque-scheme`)

### Phase D — CAB off the pivot type

- Remove CAB-only fields (`classified_by`, `derived_from`, `declass_exemption`, `token_spans`,
  parsed `declassify_on` instruction) from `CanonicalAttrs` as marking fields; relocate into a new
  `Cab` artifact payload type that `CapcoScheme::ArtifactPayload` references.
- Delete the "page aggregate, not a CAB" null-out in `ProjectedMarking::from_canonical`
  (`crates/ism/src/projected.rs`) — there is nothing to null once the fields are gone.
- Introduce the `DocumentContext` shape (analogue of `PageContext`) holding the artifact nodes and
  the document-scope rollup (max classification across pages, etc.).
- The declassify-on node has multiple inbound edges (structural field, derived-max #823 reserved,
  §C.4/§C.5 canned, historical trailing-banner). `ProjectedMarking.declassify_on` (a `MaxDate`
  rollup) is the seed.
*Satisfies*: FR-001, FR-003, SC-001; #799 CAB specifics, #266 seed. **WASM.**

---

## `marque-core` (scanner/parser — WASM)

### Phase D — `parse_cab` becomes an artifact-node producer

`parse_cab` (in `crates/core/src/parser.rs`) stops emitting a `CanonicalAttrs` tagged
`MarkingType::Cab` and instead produces the `Cab` artifact payload + node state.

### Phase G — absence-detect recognizers (#420)

Recognizers that detect *missing* portion marks/banners (paragraph/figure/caption boundary
detection, nested-bullet handling, entirely-`(U)` exemption). These populate `AbsentButRequired`
node states; they do not invent content (D4 → flag-only).
*Satisfies*: FR-051 (US6). **WASM.**

---

## `marque-capco` (CAPCO domain — WASM)

### Phase D/E/G

- `CapcoScheme::ArtifactPayload = Cab` (+ declassify/notice payloads); declarative artifact/edge
  catalog (original-CAB vs derivative-CAB as two inbound edges into one node, research D1/CAB
  specifics).
- §C.4/§C.5 canned `Declassify On` rules (Phase G, #266) — declarative `DerivationRelation::CannedString`
  edges firing on page-level AEA / NATO presence flags; high-confidence fix proposing the literal
  mandated string. Citations: CAPCO-2016 §C.4 p33 (verified), §C.5 p33 (verified).
- Co-residence reconciliation hooks for the future `marque-cui` peer (the engine owns the
  two-scheme knowledge; capco exposes the IC-side projection).
- #128 caveat layer ≡ CUI `LDC` value set (source-pending vocabulary).
*Satisfies*: FR-050, FR-052 (US6); #266, #128. **WASM.**

---

## `marque-engine` (convergence — native)

### Phase B — generification & scheme-set

- `Engine<S>` actually uses `S` (eliminate the `drop(scheme)` construction bridge in `engine.rs`,
  #641 T1-3). `LintResult`/`FixResult` carry grammar-erased or generic diagnostics (T2-1).
- `MultiGrammarEngine` wrapping multiple `Engine<S>` instances + a translator registry; runs
  single-grammar rules independently then coherence rules over the joint result (T1-7).
- `bridge_constraint_diagnostic` delegates to `scheme.constraint_rule_id(label)` (T1-4).
- `ScanStrategy`/`ParseStrategy` injection points (T1-5/T1-6).

### Phase C — document-scope derivation layer

- `DocumentContext` accumulator threaded above the per-page `PageContext` accumulator
  (`crates/engine/src/engine/page_context.rs` is the pattern). Reset at scanner document
  boundaries BEFORE parse (extends the existing `PageContext` reset invariant).
- Extend `marque-engine::scheduler` (`scheduler.rs`) to schedule `DerivationEdge`s alongside
  `PageRewrite`s: one Kahn's-algorithm pass over the union of `reads`/`writes`, cycles rejected at
  `Engine::new`. Mode-gated edges are declared but firing-predicated (research D3).
- Resolution decoupled from fixing (FR-011); reverse validation + "classified up to" front marking
  (FR-015). Derivations recorded via the existing `DecisionSink` cascade (FR-012).

### Phase E — reconciliation (model b)

- Two-scope cross-scheme reconciliation: portion-scope ownership routing (D8) + document-scope
  releasability join+closure (D6). The `(S//CUI)` cross-grammar conflict path.

### Phase F — mode taxonomy (#645)

- `EngineConfig { severity_cap, fix_zones, deployment }`; `severity_cap` applied in
  `fast_path_severities` (`effective = override.unwrap_or(default).min(cap)`); `fix_zones` gates
  fix promotion before `__engine_promote`; `DeploymentContext` defaults profile;
  `as_of` wired engine→recognizer→`RuleContext`; `ArchivalIntent`, `GrammarEra`
  (`MarkingScheme::era_at`, `vocabulary_at`). M4/M5 depend on #206 `as_of` wiring.
*Satisfies*: FR-020/021/022/040/041/042/043. *Issues*: #641, #799, #645. **native.**

---

## `marque-config` (native)

### Phase B/F

- `Config.grammar_schema` generic; schema-version validation delegated to
  `scheme.validate_schema_version` (#641 T4-1). TOML `[engine] severity_cap / fix_zones /
  deployment` (#645 M1/M2/M3).
*Satisfies*: FR-040/041/042; #641 T4-1, #645. **native.**

---

## Cross-cutting: issue → type coverage matrix

| Issue | Primary types | Phase |
|-------|---------------|-------|
| #799 | `DocumentArtifact`, `ArtifactState`, `DerivationEdge`, `DocumentContext`, scheduler ext. | 0, C |
| #641 | `Rule<S>`, `RuleContext<S>`, `Engine<S>`, `MultiGrammarEngine`, T3 renames, `Translate`/`CoherenceRule` | B, E |
| #643 | `InputAdapter`, `StructuredDocument`, `DocumentLayer`, `RepairKind` | A |
| #176 | `InputSource`, `InputContext`, `ParseContext` `#[non_exhaustive]` | A |
| #645 | `EngineConfig` mode fields, `target_zones`, `DeploymentContext`, `ArchivalIntent`, `GrammarEra` | F |
| #640 | (tooling/corpus, not types) | H |
| #266 | declassify-on node, `CannedString` edge, §C.4/§C.5 rules | D, G |
| #420 | absence-detect recognizers, `AbsentButRequired` state | G |
| #128 | `CaveatLayer` artifact ≡ LDC value set | D, E |
| #823 (deferred) | `Scope::Bundle`, `DerivationRelation::SourceDerived`, `ValueDerivation::DerivedMaxOverSources` (reserved) | 0 (seam), later |
| #824 (deferred) | `ReplacementIntent` pre-state fields, `Relocate` variant (reserved) | 0 (seam), later |
