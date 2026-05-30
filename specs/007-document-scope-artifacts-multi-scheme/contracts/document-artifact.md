# Contract: Document-Artifact Node & Derivation Layer

Trait/type surface sketches — **signatures, not implementations**. Reconciled against current
code (`crates/scheme/src/{scope,recognizer,decision,page_rewrite}.rs`, `crates/ism/src/projected.rs`,
`crates/engine/src/{scheduler.rs,engine/page_context.rs}`). Exact types fixed in implementation.

## Node model (`marque-scheme`, Phase 0)

```rust
#[non_exhaustive]
pub enum ArtifactKind { AuthorityBlock, DeclassifyInstruction, Notice, CaveatLayer, FrontMarking }

pub enum ArtifactState<P> {
    Present(P),
    PresentNonCanonical(P),
    AbsentButRequired,
    AbsentNotRequired,
}

pub struct DocumentArtifact<S: MarkingScheme + ?Sized> {
    pub kind: ArtifactKind,
    pub scope: Scope,                              // Document | Bundle
    pub state: ArtifactState<S::ArtifactPayload>,  // new associated type on MarkingScheme
    pub derivation: ValueDerivation,
    pub inbound: Box<[DerivationEdge]>,
}
```

**Source location**: `DocumentArtifact` carries `Option<marque_scheme::Span>`. `Span` is defined
in `marque-scheme` (`crates/scheme/src/span.rs`) and re-exported by `marque-ism`, so the leaf
crate already owns the type — no `Loc` associated type or `ByteRange` workaround is needed.

**New `MarkingScheme` members (additive)**:
```rust
trait MarkingScheme {
    type ArtifactPayload;                                  // scheme's parsed artifact value
    fn document_artifacts(&self) -> &[ArtifactDecl];        // declared artifact catalog
    fn derivation_edges(&self) -> &[DerivationEdge] { &[] } // declared inbound edges (default none)
    // existing: categories/constraints/templates/parse/project/render_*/page_rewrites/closure_rules
}
```

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
- **Reset invariant** (extends Constitution VI): the engine resets the `DocumentContext`
  accumulator at scanner-emitted document boundaries BEFORE attempting to parse the boundary
  candidate, mirroring the existing `PageContext` reset-before-parse guarantee.

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
