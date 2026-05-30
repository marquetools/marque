# Implementation Plan: Document-Scope Artifacts & Multi-Scheme Co-Residence

**Branch**: `007-document-scope-artifacts-multi-scheme` | **Date**: 2026-05-30 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `specs/007-document-scope-artifacts-multi-scheme/spec.md`

## Summary

Decouple document-scoped artifacts (CAB, `Declassify On`, notices, caveats) from the marking
pivot type and model them as typed nodes in a static derivation DAG with a five-state node model
(`Present | PresentNonCanonical | PresentNotRequired | AbsentButRequired | AbsentNotRequired`);
add a document-scope
aggregate (`DocumentContext`) analogous to `PageContext`; and land the domain-neutral
infrastructure for two grammars to co-reside on one document (scheme-set container, two-scope
cross-scheme reconciliation, `Product`+monotone-closure releasability per the lattice-consultant
verdict in `research.md`). Sequenced as a phased program honoring the Constitution's
feature-development order. #823 and #824 are deferred — this feature reserves their seams
(`Scope::Bundle`, fix-intent pre-state fields).

## Technical Context

**Language/Version**: Rust 1.85+ (edition 2024).
**Primary Dependencies**: existing workspace deps only (memchr, aho-corasick, smallvec, blake3,
serde, tokio/axum at the engine/server boundary). No new runtime deps; no copyleft (Constitution
Tech Stack / `deny.toml`).
**Storage**: N/A on the hot path (build-time `OUT_DIR` only).
**Testing**: `cargo test` per-crate; corpus-accuracy harness; criterion latency/throughput gates;
`audit_g13_canary` content-ignorance test.
**Target Platform**: native + WASM. The WASM-safe set (`marque-ism`, `marque-core`,
`marque-rules`, `marque-scheme`, `marque-capco`) MUST stay WASM-safe (Constitution III).
**Project Type**: Rust workspace (compiler/library + CLI + server + WASM).
**Performance Goals**: interactive p95 ≤ 16 ms; linear fix throughput. Single-scheme no-regression gated by SC-008a (incl. the new #420 absence-scan bench); the multi-scheme O(schemes) hot-path multiplier budgeted separately by SC-008b (`multi_scheme_latency` bench).
**Constraints**: zero-copy streaming core; audit content-ignorance (G13); acyclic crate graph.
**Scale/Scope**: a phased program across all WASM-safe crates + engine + integration surfaces;
~9 phases (0, A–H) plus two deferred groups.

## Constitution Check

*GATE: must pass before Phase 0 research and re-checked after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Performance | PASS (gated) | New document-scope pass reuses the cached topological order (scheduler) and the existing per-page accumulator pattern; SC-008a enforces no single-scheme p95/throughput regression (incl. a dedicated bench for the new #420 whole-document absence scan). SC-008b budgets the multi-scheme O(schemes) hot-path multiplier with its own `multi_scheme_latency` bench — the single-scheme 16 ms gate is NOT assumed to hold unchanged under co-residence. |
| II. Zero-copy / lifecycle wipe | PASS | Artifact nodes hold `Span` offsets, not content copies; any new owned content buffer wipes on drop (`secrecy`/`zeroize`). Reversibility pre-state stores canonicals/digests, not free-form text. |
| III. Format-agnostic / WASM | PASS | All new types in the WASM-safe set are I/O-free. `InputAdapter` is a trait in `marque-scheme`; concrete schema-reading adapters live in non-WASM crates. WASM runtime-config restriction honored: no new recognizer codepath loadable at runtime, **and the WASM build pins `InputSource::DocumentContent`** — `StructuredField`/`SchemaDocument` raise recognizer posteriors and so are withheld from the WASM runtime opt-in (FR-031 WASM stance), exposed only to trusted CLI/server callers. |
| IV. Two-layer rule arch | PASS | Node detection/derivation declared as data (`Constraint`/`PageRewrite`-style catalog + derivation edges); §C.4/§C.5 strings are Layer-2 rules citing the manual. |
| V. Audit-first | PASS | Derivations recorded via the content-ignorant `DecisionSink` cascade; reversibility pre-state uses only audit-permitted terms (token canonicals, category IDs, spans, BLAKE3). `__engine_promote` stays engine-only. |
| VI. Dataflow pipeline | PASS | Document-scope is a new roll-up layer above page roll-up, not a collapsed function; reset-before-parse invariant extended to document boundaries; rules/recognizers stay `Send + Sync`, no global mutable state. |
| VII. Crate discipline | PASS | `marque-scheme` stays the leaf (no `marque-ism` dep); cross-scheme reconciliation lands in `marque-engine` (model b). New CUI grammar (later) sits **alongside** `marque-ism` as a peer. |
| VIII. Source fidelity | PASS | §C.4/§C.5/§H.7/§H.8 citations verified against `crates/capco/docs/CAPCO-2016.md`; CUI claims flagged source-pending. |

**Gate decision**: PASS. One sequencing rule from Principle IV is load-bearing: *a
scheme-adoption PR MUST NOT edit the engine crates*. Therefore the domain-neutral infrastructure
(Phases 0/A/B/C/D/F) lands **before** any scheme adoption (CUI), and the CUI co-residence work
(Phase E) is gated on those engine seams existing. Phase E lands against a **synthetic
`StubScheme`** (FR-026), not a real CUI grammar, so it asserts no source-pending CUI semantics
(Constitution VIII) and adopts no real scheme into the engine. **Breaking-change posture**:
marque is pre-users, so source-breaking edits land freely in a single Phase-0/B breaking window
(research D13) — no deprecation shims; only the audit-record schema and the lattice trait surface
stay stable. No Complexity-Tracking violations.

## Project Structure

### Documentation (this feature)

```text
specs/007-document-scope-artifacts-multi-scheme/
├── plan.md              # This file
├── spec.md              # Prioritized user stories, FRs, success criteria
├── research.md          # Resolved design decisions (memo "Open items" + #641 tiers + lattice verdict)
├── data-model.md        # Every new/changed type, by crate and phase
├── contracts/
│   ├── document-artifact.md   # node trait + state machine + DocumentContext
│   ├── input-adapter.md       # InputAdapter / StructuredDocument / RepairKind / InputSource
│   ├── multi-scheme.md        # scheme-set container / ErasedScheme / CoherenceRule (Translate cut → #829)
│   └── reversibility.md       # fix-intent inverse-record surface (#824 rough-in)
└── tasks.md             # Dependency-ordered tasks grouped by phase
```

### Source Code (repository root — touched crates)

```text
crates/
├── scheme/      # Phase 0/A/B(T3)/E primitives: ArtifactState, DocumentArtifact, DerivationEdge,
│                #   Scope::Bundle, InputSource/InputContext/InputAdapter, RecognitionProvenance,
│                #   ValueDerivation, CoherenceRule (Translate cut → #829), T3 renames, AND the fix-intent
│                #   pre-state fields (#824) — ReplacementIntent lives in scheme/src/fix_intent.rs.
│                #   LEAF — no marque-ism dep.
├── rules/       # Phase B: Rule<S> generification (T1-1/T1-2), MessageTemplate/FeatureId
│                #   #[non_exhaustive] + Grammar escape (T2).
├── ism/         # Phase D: CAB node off CanonicalAttrs; DocumentContext shape; declassify-on node.
├── core/        # Phase D/G: parse_cab → artifact-node producer; absence-detect recognizers (#420).
├── capco/       # Phase D/E/G: CapcoScheme artifact/edge declarations; §C.4/§C.5 rules; co-residence.
├── engine/      # Phase B/C/E/F: Engine<S>, MultiGrammarEngine, DocumentContext accumulator,
│                #   derivation scheduler extension, EngineConfig mode fields, reconciliation (model b).
├── config/      # Phase F: severity_cap, fix_zones, deployment, grammar_schema (#641 T4-1).
└── (cui/)       # FUTURE peer crate — out of scope here; only the seams it needs are landed.
tools/ + tests/corpus/   # Phase H: per-grammar corpus/priors/harness (#640).
```

**Structure Decision**: changes follow the existing crate graph exactly; no new crate lands in
this feature (the `marque-cui` peer is future work). The phase ordering below maps onto the
Constitution's feature-development sequence (scheme/rules/ism → core → capco → engine →
integration surfaces).

## Phased Roadmap

Each phase is independently land-able as one or more PRs. Build order:

```mermaid
graph TD
    P0["Phase 0 — Domain-neutral scaffolding<br/>node-state model, 2 provenance axes,<br/>fix-intent pre-state, Scope::Bundle"]
    PA["Phase A — Input boundary<br/>#643 / #641-T1-8 / #176"]
    PB["Phase B — Multi-scheme generification<br/>#641 T1/T2/T3/T4"]
    PC["Phase C — Document-scope derivation layer<br/>#799"]
    PD["Phase D — CAB decoupling<br/>#799 CAB specifics"]
    PE["Phase E — CUI co-residence<br/>#641 co-reside / #128"]
    PF["Phase F — Mode taxonomy<br/>#645"]
    PG["Phase G — Concrete artifact rules<br/>#266 / #420"]
    PH["Phase H — Per-grammar corpus/tooling<br/>#640"]
    D823["#823 ICD-206 source list (deferred)"]
    D824["#824 reversible applied fixes (deferred)"]

    P0 --> PA & PB & PC
    PA --> PC
    PB --> PE & PH
    PC --> PD & PE & PF
    PD --> PG
    PE --> PG
    PA -.reserved edge.-> D823
    PC -.reserved edge.-> D823
    P0 -.pre-state hooks.-> D824
    PF -.mode-gated apply.-> D824
```


| Phase | Scope | Issues | Crates | Gates on |
|-------|-------|--------|--------|----------|
| **0** | Domain-neutral scaffolding: `ArtifactState`, `DocumentArtifact`, `DerivationEdge`, `Scope::Bundle`, two provenance axes, fix-intent pre-state fields | memo "must honor now"; #824/#823 rough-in | scheme | — |
| **A** | Input boundary: `InputAdapter`, `StructuredDocument`/`DocumentLayer`/`RepairKind`, promote `InputSource` + `InputContext`, #176 confidence calibration | #643, #641 T1-8, #176 | scheme, engine | 0 |
| **B** | Multi-scheme generification: `Rule<S>::check(&S::Canonical,…)`, `RuleContext<S>`, `Engine<S>`, scheme-set container, T2 `#[non_exhaustive]`+`Grammar` escapes, T3 renames, T4 config/entry wiring | #641 T1/T2/T3/T4 | rules, engine, config, capco, wasm, server, cli | 0 |
| **C** | Document-scope derivation layer: `DocumentContext`, derivation DAG (extend scheduler), absence-as-state, cascade-recorded derivations, reverse validation, "classified up to" front marking | #799 | scheme, engine | 0, A |
| **D** | CAB decoupling: CAB off `CanonicalAttrs` → `DocumentArtifact`; CAB normalizer/serializer (forward-evaluable); original-vs-derivative as two inbound edges; declassify-on node w/ multiple provenances | #799, memo CAB specifics | ism, core, capco, engine | C |
| **E** | CUI co-residence: two-scope reconciliation, `Product`+monotone NOFORN closure, relocate-not-evict, `(S//CUI)` conflict, #128 ≡ LDC value set | #641 co-reside, #128 | engine, capco | B, C |
| **F** | Mode taxonomy: `severity_cap`, `fix_zones`/`target_zones`, `DeploymentContext`, `as_of` wiring, `ArchivalIntent`, `GrammarEra` | #645 (M4/M5 dep #206) | config, scheme, engine | C |
| **G** | Concrete artifact rules: §C.4/§C.5 canned `Declassify On` strings; missing portion-mark/banner detection | #266, #420 | core, capco | D, E |
| **H** | Per-grammar corpus/tooling: directory namespace, `analyze.py` profile, per-grammar priors, harness | #640 | tools, tests, capco build | B |
| **Deferred** | #823 ICD-206 source-list generation (gated on A + C reserved edge); #824 reversible-applied-fixes realization (audit-schema bump; uses 0's pre-state fields + F's mode gating) | #823, #824 | — | A, C / 0, F |

### Phase detail pointers

- **Phase 0** is the blocking foundation that must land before everything else. Most of it is new
  type-surface in the WASM-safe leaf crates, testable in isolation; the one source-breaking piece
  is the `ReplacementIntent` edit (new `prior` field + `Relocate` variant). Per the rewrite-freely
  posture (research D13), Phase 0 simply **is** the start of a single Phase-0/B breaking window —
  the `ReplacementIntent` edit lands as a plain breaking change with all in-tree sites updated, not
  deferred and not shimmed. (Earlier drafts called Phase 0 "additive" while it contained this
  breaking edit — that contradiction is removed.) `ErasedScheme`/`ErasedEngine` object-safety (the
  load-bearing co-residence design, see `contracts/multi-scheme.md`) is a Phase-B prerequisite.
- **Phases A and B fan out** from 0 and can proceed in parallel (different crates/seams).
- **Phase C → D** (derivation layer must exist before CAB becomes a node consuming it) and
  **B,C → E** (co-residence needs both the generic engine and the document-scope layer).
- **#823/#824 stay deferred** but their seams are landed in Phase 0 (pre-state fields,
  `Scope::Bundle`) so adding them later is additive, not a breaking change.

## Complexity Tracking

No Constitution violations requiring justification. Two structural risks, both gated:
1. **Single-scheme latency** — adding a document-scope pass, the derivation-DAG evaluation, and
   the new #420 whole-document absence scan without regressing the 16 ms p95. Mitigated by reusing
   the cached topological scheduler order and the per-page accumulator pattern; gated by SC-008a
   (with a dedicated #420 absence-scan bench).
2. **Multi-scheme latency** — co-residence runs N scheme engines + reconciliation on the hot path,
   an O(schemes) multiplier the single-scheme gate does not measure. Not waved away as "no
   regression": SC-008b establishes a separate `multi_scheme_latency` budget as the gate.
