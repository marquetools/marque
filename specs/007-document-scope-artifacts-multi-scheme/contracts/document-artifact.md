# Contract: Document-Artifact Node & Derivation Layer

Trait/type surface sketches — **signatures, not implementations**. Reconciled against current
code (`crates/scheme/src/{scope,recognizer,decision,page_rewrite}.rs`, `crates/ism/src/projected.rs`,
`crates/engine/src/{scheduler.rs,engine/page_context.rs}`). Exact types fixed in implementation.

## Node model (`marque-scheme`, Phase 0)

```rust
#[non_exhaustive]
pub enum ArtifactKind { AuthorityBlock, DeclassifyInstruction, Notice, CaveatLayer, FrontMarking }

// Status enum (NOT a lattice — states of one node are never joined; LV1, research D12).
// The state set = presence (absent / present-canonical / present-non-canonical)
//               × requirement (required / not-required).
pub enum ArtifactState<P> {
    Present(P),               // present, canonical, required
    PresentNonCanonical(P),   // present, parsed, diverges from canonical
    PresentNotRequired(P),    // present but superfluous (e.g. §E.5 string in a pure-NATO doc)
    AbsentButRequired,
    AbsentNotRequired,
}

pub struct DocumentArtifact<S: SchemeArtifacts + ?Sized> {
    pub kind: ArtifactKind,
    pub scope: Scope,                              // Document | Bundle
    pub state: ArtifactState<S::ArtifactPayload>,  // ArtifactPayload from the SchemeArtifacts ext trait
    pub derivation: ValueDerivation,
    pub inbound: Box<[DerivationEdge]>,
    pub span: Option<marque_scheme::Span>,          // present-node source location; None when absent
}
```

**Source location**: `DocumentArtifact` carries `Option<marque_scheme::Span>`. `Span` is defined
in `marque-scheme` (`crates/scheme/src/span.rs`) and re-exported by `marque-ism`, so the leaf
crate already owns the type — no `Loc` associated type or `ByteRange` workaround is needed.

**New `MarkingScheme` members — mixed additive / breaking; stage to keep the frozen surface
intact.** Adding a **required associated type** to a public trait is a *breaking* change for every
downstream implementor (associated-type defaults are not stable Rust), and `MarkingScheme` is a
frozen stable-API surface (CLAUDE.md). The defaulted *methods* are additive; the associated type
is not. Preferred staging — put the payload behind an **extension trait** so `MarkingScheme`
itself stays unbroken:

```rust
// MarkingScheme itself gains only defaulted methods (additive):
trait MarkingScheme {
    fn document_artifacts(&self) -> &[ArtifactDecl] { &[] }   // default: no artifacts
    fn derivation_edges(&self) -> &[DerivationEdge] { &[] }    // default: no edges
    // existing: categories/constraints/templates/parse/project/render_*/page_rewrites/closure_rules
}

// The artifact payload type lives on an opt-in extension trait — non-breaking for schemes
// that don't model document artifacts (a future minimal scheme, test stubs):
trait SchemeArtifacts: MarkingScheme {
    type ArtifactPayload;                                  // scheme's parsed artifact value
}
```

**Alternative** (acceptable, but breaking): fold `type ArtifactPayload` directly onto
`MarkingScheme` *inside the Phase-B breaking window* — Phase B already breaks the trait surface
(`Rule<S>`/`Engine<S>` generification), and the only in-tree implementors are `CapcoScheme` plus
test stubs (no external implementor exists — CUI is not yet a crate). The phase/versioning note
in plan.md and the data-model entry MUST reflect whichever path is chosen; do not label the
associated-type addition "additive."

## Derivation edges & scheduling (`marque-scheme` + `marque-engine`, Phase 0/C)

```rust
pub struct DerivationEdge {
    pub id: EdgeId,                       // &'static label, mirrors RewriteId
    pub relation: DerivationRelation,     // Rollup | Requirement | SourceDerived | CannedString | Passthrough
    pub reads: Box<[CategoryId]>,
    pub writes: Box<[CategoryId]>,
    pub firing: FiringPredicate,          // always declared; gates firing (incl. deployment mode)
}
```

The engine scheduler (`crates/engine/src/scheduler.rs`) extends `schedule_rewrites` to a single
Kahn's pass over the union of `PageRewrite` and `DerivationEdge` `reads`/`writes`. **Invariants
preserved**: writers-before-readers; cycles rejected at `Engine::new`
(`EngineConstructionError::RewriteCycle`); unannotated `Custom` axes rejected. Mode-gated edges
are *declared in the topology* and skipped at firing time — never removed from the graph
(research D3, preserves the construction-time cycle check).

## Document-scope aggregate (`marque-ism` + `marque-engine`, Phase C/D)

`DocumentContext` is the analogue of `PageContext`:
- Holds the `DocumentArtifact` nodes + the document-level rollup (max classification across pages,
  unioned controls, max-date declassify seed).
- Built incrementally during `lint`, threaded to document-scope rules via `RuleContext`
  (analogous to the existing `Arc<PageContext>` plumbing).
- **Reset invariant** (extends Constitution VI): a *document* boundary is the **input boundary**
  — one `lint`/`fix` call = one document = one batch `id`. The scanner emits only
  `MarkingType::PageBreak` (form-feed / `\n\n\n+`); there is **no** in-buffer document-delimiter
  candidate, and a concatenated multi-document buffer is a caller error, not a supported input.
  So `DocumentContext` is constructed fresh per input; *within* that input it resets its
  page-level accumulators at page boundaries BEFORE parsing the boundary candidate, mirroring the
  existing `PageContext` reset-before-parse guarantee. (Earlier drafts said "scanner-emitted
  document boundaries" — corrected: page boundaries are scanner-emitted, document boundaries are
  input-emitted.)
- **Rollup laws** (LV3, research D12): page→document rollup is the same join ops one level up,
  lawful by associativity/commutativity/idempotence. It MUST reuse the observational-state
  lattice types (`DissemSet` with `relido_observed_unanimous`, `JointSet`) — a naive re-union
  would drop RELIDO-unanimity and NOFORN-supersession across the fold.

## Resolution / fix decoupling & fixability (Phase C)

```rust
// Resolution always runs (even with fixing off): the engine computes what each node SHOULD be.
// Fixing is optional application of the resolution to text.
fn resolve_document(...) -> ResolvedDocument;     // node states + derived values, no mutation

// Fixability follows derivability (research D4):
//   AbsentButRequired + inbound edge that can produce the value  → fixable (FixProposal)
//   AbsentButRequired + no producing edge                        → flag-only (Diagnostic)
```

Every fired derivation emits a `DecisionEvent` through the existing `DecisionSink`
(`crates/scheme/src/decision.rs`), content-ignorant, with `triggered_by` linking the cascade.
No new audit-side free-form surface (G13 canary preserved).

## Reverse validation & front marking (Phase C, #799)

```rust
// "classified up to" overall front marking: a FrontMarking artifact node validated in REVERSE
// against the union of all pages' markings. DiffRelation::BannerOverPortions already exists
// (crates/scheme/src/scope.rs) for the banner-vs-portions case; document-vs-all-pages reuses the
// DiffInput mechanism at Scope::Document/Bundle.
```
