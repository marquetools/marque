# Phase 0 — Research: Declarative Constraints, Probabilistic Decoder, Vocabulary Metadata

**Feature:** `004-constraints-decoder-vocab`
**Primary technical source:** [`docs/plans/2026-04-19-recursive-lattice-and-decoder.md`](../../../docs/plans/2026-04-19-recursive-lattice-and-decoder.md)

This document resolves the open technical questions that determine how Phases C, D, and E land. It is not a second-source design — the 2026-04-19 plan is authoritative for mechanism. Research questions here are scoped to decisions that affect the feature boundary and the Phase 1 data model.

## R1. Where does the shared constraint evaluator live?

**Decision:** In `marque-scheme`, as a free function `evaluate<S: MarkingScheme>(constraints: &[Constraint], m: &S::Marking, scheme: &S) -> Vec<ConstraintViolation>` plus a thin adapter inside `MarkingScheme::validate`. `Constraint` is the existing non-generic enum (foundational-plan §12 Phase C keeps the Phase-B shape verbatim); generics live on the evaluator, not the constraint.

**Rationale:** Constraints are domain-neutral by construction (conflict, requirement, implication, supersession all operate on `S::Marking` via `S::Category` projections that every scheme already exposes). Putting the evaluator in `marque-scheme` means a new adapter inherits it for free (FR-002). Putting it in `marque-capco` would force every future scheme to re-implement the same dispatch — a silent Constitution IV violation (two-layer rule architecture collapses into "write your evaluator from scratch, friend").

**Alternatives considered:**

- **Per-scheme evaluator.** Rejected: forces re-implementation for every adapter; violates FR-002.
- **Generic `impl<S: MarkingScheme> Scheme for ...` in `marque-engine`.** Rejected: puts domain-neutral logic upstream of `marque-scheme` and creates a dependency cycle (engine consumes scheme; scheme can't consume engine). Also pushes trait-surface concerns into the orchestration crate, which Constitution VII explicitly forbids.

## R2. How does the engine schedule page-level rewrites?

**Decision:** Topological sort over the read/write dependency graph at `Engine::new` (build-time relative to engine construction, not per-document). Cycles produce `EngineConstructionError::RewriteCycle` naming both members of the cycle; unannotated custom rewrites produce `EngineConstructionError::UnannotatedCustomAxes`. Each `PageRewrite` carries `reads: &'static [CategoryId]` and `writes: &'static [CategoryId]` annotations. Two constructors: `PageRewrite::declarative` derives annotations from the predicate/action enum variants; `PageRewrite::custom` requires explicit declaration.

**Rationale:** Deterministic scheduling, independent of declaration order (FR-007). Cycles caught at construction, not document processing — a cyclic grammar is broken *once*, not per-document. Annotated rewrites let `declarative` constructors infer axes automatically for the common case while forcing `custom` constructors to state their side effects explicitly (FR-005). This matches §7a of the primary-source plan exactly.

**Alternatives considered:**

- **Per-document scheduling.** Rejected: re-running the topological sort on every document burns cycles for no information gain; the grammar is static.
- **Confluence check instead of topological scheduling.** Rejected in primary-source plan §14 ("Confluence as a stronger property..."): topological acyclic suffices for determinism; confluence is over-specification CAPCO doesn't need.
- **Runtime fallback scheduling.** Rejected: a cyclic grammar is a correctness defect, not a recoverable runtime state. Fail at `Engine::new`.

## R3. Candidate generator bound for the probabilistic decoder

**Decision:** K = 8 candidates per grammar template. Corpus-derived priors baked into the build as `&'static [T]` tables; no runtime learning.

**Rationale:** K = 8 was pinned in primary-source §5.2 after empirical analysis of the Enron high-confidence corpus. 8 candidates covers the long-tail realistic mangling within the edit-distance budget (1–2 edits per token, per-token cap by token length). Higher K burns latency without accuracy gain (diminishing returns above 6); lower K drops recall on multi-token reorderings. Bake-in priors avoid runtime I/O on the hot path (Constitution I) and let WASM ship the same corpus tables without a runtime override codepath (Constitution III + Q8).

**Alternatives considered:**

- **Runtime-learned priors.** Rejected: violates Constitution III (expands semantic surface at runtime) and Constitution VI (requires mutable global state). A CLI `--corpus-override` is permitted for self-operator mode only (T3).
- **Variable K per template.** Rejected for Phase D scope: the primary-source plan explicitly pins K=8 as a uniform bound; variable K can be revisited in a later phase if a concrete regression emerges.
- **Unbounded beam search.** Rejected: violates SC-002 (p95 ≤18 ms with one mangled region) and Constitution I (measurable perf contract).

## R4. Audit record schema bump strategy

**Decision:** Single schema version per engine build. Current schema is `marque-mvp-1`; Phase D introduces `marque-mvp-2` with the new `confidence`, `runner_up_ratio`, and `features` fields (plus `FixSource::DecoderPosterior` when applicable). Downstream consumers continue to parse `marque-mvp-1` records from pre-Phase-D builds (back-compat). One engine build emits exactly one schema version — no in-flight version toggling.

**Rationale:** FR-014 explicitly. The single-version-per-build contract keeps the audit record format unambiguous in a given deployment; downstream compliance consumers can version-gate their ingestion without parsing heuristics. The `-mvp-N` suffix mirrors the existing naming convention in `marque-engine/src/fix.rs`. Back-compat on the downstream parser side is achieved by keeping the v1 schema as a strict subset of v2 (all v1 fields present, new fields optional — even though a v2 *emitter* always populates them).

**Alternatives considered:**

- **Multiple schema versions per build (feature-flag controlled).** Rejected: a single deployment producing mixed-schema records makes compliance ingestion ambiguous — "which records from this engine build do I trust?" A build that emits v1 and v2 simultaneously is a downstream parser hazard.
- **Extend v1 in place.** Rejected: the new required fields (`confidence`, `features`) break consumers that strict-parse v1. A bump is cheaper than debugging silent downstream truncation.

## R5. Dual JSON + XML codepath for ODNI ISM

**Decision:** Consume ODNI ISM-v2022-DEC through two separate `build.rs` codepaths: JSON for per-term vocabulary data, XML for XSD + Schematron constraint predicates. Neither is a fallback for the other — ODNI publishes both because they encode different things. JSON carries owner/producer, authority, POC, descriptions; XSD + Schematron encode validity predicates and deprecation annotations.

**Rationale:** FR-018 explicitly. Consolidating into one codepath would force synthesizing the missing side on every build, introducing drift between what `marque-ism` sees and what the authoritative source says. The schemas are vendored (`crates/ism/schemas/ISM-v2022-DEC/`); the JSON sidecar is part of that vendored set. This matches §4.4 of the primary-source plan.

**Alternatives considered:**

- **XML-only (current state).** Rejected: loses per-term metadata (authority, owner/producer, POC) that ODNI publishes in the JSON sidecar. Half the vocabulary surface would be unreachable.
- **JSON-only.** Rejected: loses Schematron predicates, which are the source of truth for constraint validation. Layer 1 of the two-layer rule architecture breaks.
- **Synthesize one side from the other.** Rejected: every synthesis pass is a citation-drift risk (Constitution VIII). The authoritative source publishes both; we consume both.

## R6. Mangled-marking fixture generator

**Decision:** Generator lives in `tools/corpus-analysis/`, reads the Enron corpus's high-confidence markings (author-supplied — not committed to the repo), applies six labeled mangling transforms (typo, reordering, missing-delimiter, superseded-token, wrong-case, garbled-delimiter), and emits the labeled fixture under `tests/fixtures/mangled/`. Fixture size ≥ 200 labeled cases across the six mangling classes (FR-008, SC-004). Fixture itself is committed; the Enron-corpus source is not.

**Rationale:** Keeps the copyrighted/PII-sensitive source artifact out of the repo while preserving reproducibility of the fixture (generator is deterministic given the same input corpus and seed). Labeling is mechanical — the transform that produced the mangled form determines the class label and the expected canonical form. Baseline-accuracy measurement (SC-004 ≥ 85% resolution at aggregate confidence ≥ 0.85) runs against this fixture.

**Alternatives considered:**

- **Hand-crafted mangled cases only.** Rejected: covers ~dozens, not 200; biased toward what the author imagines mangling looks like rather than what real OCR / hand-typing produces.
- **Ship the Enron corpus subset.** Rejected: copyright, PII, and repository bloat.
- **Crowdsourced fixture.** Rejected for Phase D scope: introduces a review-and-verification workflow that doesn't exist yet. Revisit in a later phase if accuracy targets demand more fixture diversity.

## R7. `Send + Sync` guarantee for rules and recognizers

**Decision:** Declare `Send + Sync` as a trait bound on `Rule` and `Recognizer`. Per-invocation scratch allocations allowed; `static mut`, `OnceCell<Mutex<_>>`-as-hidden-cache, and similar patterns prohibited.

**Rationale:** Constitution VI + FR-023. The `BatchEngine`'s concurrent correctness rests on every rule and recognizer being safely shareable across `tokio::task::spawn_blocking` workers. A rule with hidden mutable global state is a data race the semaphore cannot serialize — and the semaphore is backpressure, not mutual exclusion. Making `Send + Sync` a *trait bound* rather than a convention turns it into a compile error rather than a code-review question.

**Alternatives considered:**

- **Runtime check (e.g., at `BatchEngine::new`).** Rejected: the only runtime check possible is "try and observe a crash"; `Send + Sync` is statically decidable and the compiler will tell us.
- **`Send` only (no `Sync`).** Rejected: `BatchEngine` shares a single rule-set reference across workers; without `Sync` the rule-set needs per-worker cloning, which materializes the cost without buying correctness.

## R8. WASM runtime-config rejection mechanism

**Decision:** Use a Cargo feature gate (`corpus-override`) disabled on the WASM target; a `compile_fail` doctest or a `wasm32` `cfg` assertion proves the codepath is absent. The CLI binary enables the feature; the server binary also enables it but rejects caller-supplied overrides at the request handler; the WASM artifact never enables it.

**Rationale:** Constitution III amendment + FR-013. The guarantee that "the WASM target MUST NOT accept runtime configuration that expands the engine's semantic surface" has to be *enforced at compile time*, not runtime — otherwise the semantic-surface guarantee is trust-based. Compile-fail test in the WASM build ensures that an inadvertent future commit introducing an override codepath fails to build, not just misbehaves.

**Alternatives considered:**

- **Runtime assertion in WASM bindings.** Rejected: an attacker/operator could monkeypatch the export; a compile-time exclusion is unbypassable.
- **Document-only constraint.** Rejected: "constitution says don't" is not an enforcement mechanism.

## R9. Vocabulary trait: static data, not dynamic lookup

**Decision:** `Vocabulary` trait returns `&'static TokenMetadataFull`. `TokenMetadataFull` is a compile-time const table emitted by `marque-ism/build.rs` from the ODNI JSON sidecar. No runtime allocation, no runtime parsing.

**Rationale:** SC-008 + Constitution II. Every vocabulary query is a table lookup indexed by `TokenId`. The tables are emitted in `build.rs` with the correct shape (struct literals in generated Rust source); `include!()` brings them into the crate. This is how the existing CVE enums are already structured — the extension just adds more fields to the per-term record.

**Alternatives considered:**

- **Runtime JSON parsing.** Rejected: allocates, adds a runtime dependency on `serde_json`, and violates SC-008.
- **Lazy-static with per-query allocation.** Rejected: same violation, plus global mutable state tangles up Constitution VI.

## R10. How `Confidence` composes

**Decision:** `Confidence { recognition: f32, rule: f32, region: Option<f32>, runner_up_ratio: Option<f32>, features: Vec<FeatureContribution> }`. Aggregate score = `recognition * rule * region.unwrap_or(1.0)`. Engine threshold applies to aggregate. `FeatureContribution` uses an enumerated `FeatureId` — free-form label strings are rejected by the type system (FR-012). Name is `Confidence` (not `FixConfidence`) and precision is `f32` throughout per foundational-plan line 739-757.

**Rationale:** Composes cleanly with existing `FixProposal::confidence: f32` (strict-path proposals set `recognition = 1.0`, `region = None`, `runner_up_ratio = None`, `features = vec![]`). `f32` is SIMD-friendly and aligns with existing `Candidate<M>::prior_log_odds: f32`. Decoder internals may compute in `f64` and downcast at the `Confidence` boundary. The `runner_up_ratio` + `features` fields are the provenance payload that makes a decoder-driven fix auditable (FR-009, SC-007). Enumerated `FeatureId` prevents a rule from smuggling free-form strings into the audit stream (FR-012 + threat-model T2).

**Alternatives considered:**

- **Additive combiner (log-sum instead of product).** Rejected for Phase D: the product form matches what the primary-source §6.1 specifies and is the form the corpus-derived priors are calibrated against. Revisiting the combiner is a post-Phase-D experiment, not a Phase D decision.
- **Free-form feature labels with length cap.** Rejected per FR-012 and threat-model T2: length-capping a string is an ex-post mitigation; a type-system rejection is an ex-ante prevention.
