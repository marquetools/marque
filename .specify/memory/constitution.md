<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

<!--
SYNC IMPACT REPORT
==================
Version change: 1.0.0 → 1.1.0

Bump type: MINOR
  - New crates added to the canonical dependency graph: `marque-ism`,
    `marque-scheme`. These are not new principles but they are materially
    expanded architectural guidance — a downstream consumer reading the old
    graph would wire dependencies wrong.
  - Engine-integrity invariants codified that were previously documented only
    in `CLAUDE.md`: `AppliedFix::__engine_promote` engine-only,
    `FixProposal` purity, `RuleContext.zone`/`position` optionality,
    `PageContext` page-break reset, `Severity::Off` as non-firing state.
  - WASM-safety extended from "marque-core" to the full WASM-safe set
    (`marque-ism`, `marque-core`, `marque-rules`, `marque-scheme`, `marque-capco`).
  - Four invariants folded in from the 2026-04-19 recursive-lattice-and-decoder
    plan and the 2026-04-20 long-horizon roadmap:
    - Content-ignorance in audit records (plan G13 / roadmap I-J2 / G16)
      → Principle V.
    - Scheme-adoption PRs MUST NOT edit the engine (roadmap I-L1)
      → Principle IV.
    - Rule and `Recognizer` impls MUST be `Send + Sync` with no global
      mutable state (plan G12 / §5.2) → Principle VI.
    - WASM target MUST reject runtime config that expands the engine's
      semantic surface (plan §6a T3, Q8) → Principle III.
  - **New Principle VIII — Authoritative Source Fidelity.** Codifies the
    two linked commitments: (a) every grammar implementation flows from a
    single authoritative source (CAPCO-2016 for ISM/CAPCO today; each
    future grammar declares its equivalent); (b) citation fabrication is a
    correctness defect, not a style issue — citations MUST be re-verified
    when added, propagated, or when the source revises. Cross-referenced
    from Principle IV and from the "Adding a New Rule" workflow.

Patch-level corrections folded into this amendment (no separate bump):
  - Principle IV: `marque-capco/build.rs` → `marque-ism/build.rs`;
    `[package.metadata.marque] ism-schema-version` pinned in
    `marque-ism/Cargo.toml` (not `marque-capco/Cargo.toml`).
  - Principle V: `AuditRecord` / `Fix` terminology replaced with the real types
    `FixProposal` (pure data, produced by rules) and `AppliedFix` (promoted by
    the engine with timestamp + classifier identity).
  - Principle VI: "chain of async streams" softened to match the actual
    architecture — the per-document pipeline is synchronous iteration; async
    concurrency lives in `BatchEngine` above it. The prohibition on monolithic
    stage-collapsed functions is preserved.

Modified principles (title unchanged, body materially expanded):
  - III. Format-Agnostic Core / WASM Safety
  - IV. Two-Layer Rule Architecture
  - V. Audit-First Compliance
  - VI. Dataflow Pipeline Model
  - VII. Crate Discipline and Dependency Hygiene

Added principles:
  - VIII. Authoritative Source Fidelity

Added sections: none (no new top-level sections; Principle VIII slots into
  the existing "Core Principles" list).
Removed sections: none.

Templates checked/updated:
  ✅ .specify/templates/plan-template.md — Constitution Check gate still
     references "principles above" without hardcoding paths; no edit needed.
  ✅ .specify/templates/spec-template.md — Mandatory sections remain
     compatible with all seven principles; no edit needed.
  ✅ .specify/templates/tasks-template.md — Phase/task taxonomy unchanged;
     no edit needed.
  ✅ .specify/templates/constitution-template.md — Source template, not
     modified (this amendment operates on memory/constitution.md).
  ✅ CLAUDE.md (workspace root) — Already carries the full invariant set and
     correct crate graph; constitution now catches up to it. No edit needed.

Follow-up TODOs:
  - TODO(REPOSITORY_URL): Placeholder GitHub URL in Cargo.toml
    (https://github.com/placeholder/marque) — update when repo is public.
  - Deferred: When `marque-cui` or a non-US marking crate (NATO/FGI/JOINT) is
    added, extend the dependency-graph diagram in Principle VII and the
    build.rs pattern note in Principle IV. The identity framing is already
    domain-neutral, so no principle amendment will be required for that step
    alone.
-->

# marque Constitution

## Core Principles

### I. Uncompromising Performance

Performance is the primary value proposition of `marque`. "Perceptual instantaneity"
is non-negotiable — the tool MUST feel like magic at every scale.

- Interactive use (single field, single file) MUST achieve p95 ≤ 16 ms on typical
  inputs (SC-001 benchmark harness governs the threshold).
- Batch processing MUST scale linearly; throughput MUST be benchmarked, not assumed
  (SC-005 governs the threshold).
- Every performance decision MUST be backed by measurement against a Criterion
  benchmark committed to the repo. "It feels fast" is not evidence.
- The tool MUST be async-first at integration boundaries (server, batch) — no
  blocking operations on the hot path where a thread could be released.
- SIMD-accelerated primitives (`memchr`, Aho-Corasick, BLAKE3) MUST be used
  wherever the standard library provides a slower alternative.

**Rationale**: The problem domain (1M+ cleared personnel, 12+ marking tasks/day)
makes speed a multiplier on adoption and impact. A slow linter will be bypassed;
a fast one becomes invisible infrastructure.

### II. Zero-Copy, Streaming Core

The memory model is non-negotiable. The format-agnostic core MUST operate without
heap allocation on the hot path.

- All candidate detection MUST produce `Span` values (byte offsets into original
  buffers), never copies of content. `Span` lives in `marque-ism` alongside the
  pivot type.
- Documents MUST stream through the pipeline in chunks; no stage may hold an
  entire document in memory.
- `IsmAttributes` fields (`marque-ism`) MUST use `Box<[T]>` (not `Vec<T>`) to
  eliminate over-allocation after parsing completes.
- The scanner phase MUST produce zero heap allocations per candidate span
  detected (validated by `--features count-allocs` harness where applicable).
- Cached `LintResult` spans MUST remain valid via fingerprint guarantee; no span
  re-computation on cache hit is permitted.

**Rationale**: Sensitive content (classified documents) MUST be minimized in
memory footprint. Zero-copy also enables future secure-enclave (SGX/TrustZone)
integration without architectural changes.

### III. Format-Agnostic Core / WASM Safety

The engine core knows nothing about file formats. This boundary MUST NOT be
crossed. The WASM-safe set is the boundary, not a single crate.

- The WASM-safe crate set MUST compile to WASM with `wasm-pack` without
  modification: `marque-ism`, `marque-core`, `marque-rules`, `marque-scheme`,
  `marque-capco`. Any future `marque-<domain>` rule crate MUST also be
  WASM-safe.
- WASM-safe crates MUST have zero runtime I/O dependencies, no format adapters,
  and no platform-specific code. Compile-time I/O inside `build.rs` (e.g.,
  `marque-ism` parsing ODNI XML) is permitted because it does not ship in the
  artifact.
- All document format extraction MUST live in `marque-extract` and MUST NOT
  appear in WASM builds.
- The WASM API surface (`lint`, `fix`) MUST accept raw `&str` / byte buffers;
  format conversion is the caller's responsibility.
- WASM binary size MUST be considered when choosing data structure alternatives
  (e.g., `daachorse` over `aho-corasick` where more memory-compact).
- The WASM target MUST NOT accept runtime configuration that expands the
  engine's semantic surface. Severity overrides and corrections maps (data
  already present in the strict-path codepath) are permitted; anything that
  introduces a new recognizer codepath or alters recognizer posteriors
  (e.g., decoder priors when the Phase D probabilistic recognizer lands)
  MUST be compiled in, not loaded at runtime. This closes a capability
  channel that CLI and server callers can open intentionally but a WASM
  embedder cannot sandbox.

**Rationale**: The WASM target enables browser extensions, Office add-ins, and
web form integrations — critical distribution channels. Coupling format logic
to the core would permanently close these channels. The runtime-config
restriction specifically exists because WASM embeddings sit behind postMessage
and similar surfaces where a caller-provided config table would be an
uninspected trust boundary.

### IV. Two-Layer Rule Architecture

Rule implementations MUST follow the two-layer model. Collapsing layers is
prohibited.

- **Layer 1 (generated)**: `marque-ism/build.rs` MUST parse ODNI ISM schema
  files at compile time and emit only binary valid/invalid predicates
  (`values.rs`, `validators.rs`, `migrations.rs`), included via
  `crates/ism/src/generated.rs`. No remediation logic belongs in generated
  code.
- **Layer 2 (hand-written)**: `Rule` implementations (e.g., `marque-capco`)
  MUST consume Layer 1 predicates to detect violations, classify *why* a
  violation occurred, determine fix confidence, and cite the authoritative
  section (e.g., CAPCO-2016 §H.5). Intelligence lives here, not in generated
  code. Citation discipline — what counts as a valid citation, how to verify
  it, how the source is versioned — is governed by Principle VIII.
- **Scheme surface** (`marque-scheme`): Domain-neutral trait definitions
  (`MarkingScheme`, `Lattice`, `BoundedLattice`, `Category`, `Constraint`,
  `Scope`, `PageRewrite`, built-in lattice constructors) MUST be reused by
  every structured marking domain. Domain crates provide the adapter
  (`CapcoScheme` for ISM/CAPCO); the scheme surface is not domain-specific
  and MUST NOT accrete domain vocabulary.
- Rule IDs MUST follow the convention: `E###` (error), `W###` (warning), `C###`
  (correction).
- Every `Rule` implementation MUST be stateless; config-dependent behavior
  (severity overrides, classifier ID injection, confidence thresholds) is
  handled by the engine, not the rule.
- Rules MUST treat `RuleContext.zone` and `RuleContext.position` as `Option`.
  Phase 3 made both fields optional because the engine cannot always prove a
  header/footer zone or a document position. Rules that hardcode a default
  are broken.
- `Severity::Off` is a non-firing state. A rule configured `Off` MUST be
  skipped by the engine; an `Off`-severity diagnostic is unrepresentable.
- The active ODNI schema version MUST be pinned in
  `[package.metadata.marque] ism-schema-version` in `marque-ism/Cargo.toml`.
  Schema version bumps MUST be intentional, never silent. New rule crate
  families (e.g., `marque-cui`, a future NATO/FGI/JOINT crate) MUST follow
  this same `build.rs` → generated-predicates pattern with an explicit
  version pin.
- A scheme-adoption PR MUST NOT edit the engine crates
  (`marque-engine`, `marque-scheme`, `marque-core`, `marque-rules`,
  `marque-ism`). If the scheme reveals an engine gap, the gap is fixed
  first in a separate PR that lands against the corpus regression harness,
  then the scheme lands. This is what makes the "shallow adapter" shape a
  product promise instead of a hope: the workflow enforces it.

**Rationale**: The separation makes generated predicates auditable against the
official spec, while keeping product differentiation (the "why" and "how to
fix") in maintainable hand-written code. Schema updates become a controlled
build event. Stateless rules + `Option`-typed context make the engine (not the
rule) responsible for uncertainty — which is the only way this stays correct
as more marking domains are added.

### V. Audit-First Compliance

Every applied fix MUST produce a complete audit record. Auditability is
non-negotiable in the IC/DoD compliance context.

- Rules MUST return `FixProposal` values only. `FixProposal` is pure data:
  span, replacement, confidence, source, migration reference. It MUST NOT
  carry a timestamp, classifier identity, or any runtime context.
- Only the engine MAY promote a `FixProposal` to an `AppliedFix` by snapshotting
  runtime state (timestamp, classifier ID, dry-run flag, input). The
  `AppliedFix::__engine_promote` constructor is `pub #[doc(hidden)]` because
  `marque-rules` is a dependency of `marque-engine` and sealing at the
  visibility level is not possible; the convention is binding. Only
  `Engine::fix_inner` MAY call it. Any other caller bypasses the
  confidence-threshold gate, fix-ordering invariants, and overlap guard, and
  corrupts the audit log — which is the compliance output, not a convenience.
- Every `AppliedFix` MUST record: rule ID, original text, replacement text,
  confidence score, timestamp, classifier ID (when present), and dry-run flag
  — regardless of confidence level, including 1.0-confidence fixes.
- User identity (classifier ID, classification authority) MUST NEVER appear in
  committed configuration. It MUST live only in `.marque.local.toml`
  (gitignored) or environment variables.
- `FixResult` MUST NOT be cached. Only `LintResult` may be cached.
- `--dry-run` MUST always produce full audit output without writing changes.
- Audit records MUST be content-ignorant. No document content, document
  metadata field values, or subject-claim free-form text MAY appear in an
  `AppliedFix` or any future audit-adjacent record (e.g., a Phase-J
  `DecisionRecord`, a Phase-K `CleaningRecord`). Permitted identifiers in
  audit output are: token canonicals, category IDs, span offsets, digests
  (BLAKE3 of content), posterior scalars, and enumerated feature labels.
  This is the G13 invariant from the 2026-04-19 recursive-lattice plan and
  the I-J2 / I-K2 invariants from the 2026-04-20 roadmap, unified. Corpus-
  level integration tests MUST verify no document text appears verbatim in
  engine output streams.

**Rationale**: Misclassification and improper fix application in the IC carry
legal and security consequences. Every automated change must be traceable to a
person and a rule version, and the pure-data / engine-promotion split is what
keeps that trace unforgeable. A rule crate that could mint audit records
would be a compliance failure, not just a bug.

### VI. Dataflow Pipeline Model

The processing pipeline MUST be a composition of phase stages. It MUST NOT be
implemented as a monolithic function that blurs scanner, parser, and rule
evaluation together.

- Per-document processing MUST flow Scanner → Parser → Rules → page-level
  roll-up (`PageContext`). Each phase MUST be independently testable; the
  scanner MUST not call into rule logic; rules MUST not mutate scanner state.
- `PageContext` MUST reset at scanner-emitted page-break candidates
  (`MarkingType::PageBreak`, covering form-feed `\f` and the conservative
  `\n\n\n+` heuristic). The engine MUST reset BEFORE attempting to parse the
  page-break candidate, so a malformed candidate cannot block the reset.
  Banner/CAB rules on a new page MUST see only that page's portions.
- Batch processing (`BatchEngine`) MUST layer async concurrency above the
  per-document pipeline via Tokio semaphores (row + byte) backed by
  `recoco-utils::ConcurrencyController`. CPU-bound work MUST run on
  `tokio::task::spawn_blocking`. Results MAY arrive in completion order
  (not submission order) — callers MUST correlate via echoed `id`.
- Middleware (auth, logging, rate limiting, backpressure) MUST insert around
  the engine as Tower layers — never inside phase implementations.
- The CLI, WASM, and server targets MUST share the same `Engine` core with
  different I/O adapters. New rule sets, format adapters, and integration
  surfaces MUST slot in without modifying phase code (open/closed principle).
- Rule implementations and `Recognizer` impls MUST be `Send + Sync` and MUST
  NOT hold mutable global state. Per-invocation scratch allocations are
  allowed; `static mut`, `OnceCell<Mutex<_>>`-as-hidden-cache, and similar
  patterns are not. This is what makes `BatchEngine`'s concurrent
  correctness-preserving property hold; a rule with hidden global state is a
  data race the semaphore cannot serialize.

**Rationale**: The phased model is what makes `marque` embeddable in web
workers, CLI shells, and microservices without code duplication. Keeping page
context and async concurrency at clearly separated layers prevents the subtle
correctness failures (e.g., banner rules seeing the previous page's portions)
that a collapsed implementation would eventually produce.

### VII. Crate Discipline and Dependency Hygiene

The workspace dependency graph MUST be one-directional and acyclic.

- The canonical dependency graph is:

  ```text
  marque-ism    ←── marque-core ────────────────────┐
  marque-ism    ←── marque-rules ←── marque-capco ──┤
  marque-scheme ←──────────────────  marque-capco ──┤
                                                    ↓
                                              marque-engine ←── marque-config
                                                    ↑
                                              marque-wasm
                                                    ↑
                                marque-extract (non-WASM only)
                                                    ↑
                                              marque-server
                                                    ↑
                                               marque (CLI)
  ```

  Read `A ←── B` as "`B` depends on `A`". `marque-ism` is the pivot crate
  (vocabulary, generated CVE enums, `Span`, `IsmAttributes`) and sits at the
  bottom of every WASM-safe chain. `marque-core` and `marque-rules` are
  parallel consumers of `marque-ism`; `marque-rules` does **not** depend on
  `marque-core`. `marque-scheme` is the domain-neutral trait surface for
  structured marking schemes and has zero runtime dependencies (it does not
  depend on `marque-ism`, `marque-core`, or `marque-rules`). `marque-capco`
  consumes `marque-ism`, `marque-rules`, and `marque-scheme` — but **not**
  `marque-core`. `marque-engine` is the convergence point that consumes all
  of `marque-ism`, `marque-core`, `marque-rules`, `marque-capco`, and
  `marque-config`; this is the only crate that pulls both the
  scanner/parser chain (via `marque-core`) and the rule chain (via
  `marque-capco`) together.
- No crate may introduce a circular dependency. `cargo check --workspace`
  MUST pass on every commit.
- The WASM-safe set (`marque-ism`, `marque-core`, `marque-rules`,
  `marque-scheme`, `marque-capco`) MUST have zero format dependencies and MUST
  remain WASM-safe (see Principle III). `marque-extract` MUST NOT be in this
  set.
- `marque-rules` MUST contain only trait definitions and shared data types
  (`RuleId`, `Severity`, `RuleContext`, `FixProposal`, `AppliedFix`,
  `Diagnostic`, `Rule`, `RuleSet`); no rule implementations.
- `marque-scheme` MUST contain only trait definitions and built-in lattice
  constructors; no domain-specific vocabulary.
- Every crate MUST have a single, clear responsibility documented in its
  `README.md` or crate-level doc comment.
- New rule crate families (e.g., `marque-cui`, a future NATO/FGI/JOINT crate)
  MUST follow the `build.rs` → generated-predicates pattern established by
  `marque-ism` (Principle IV) and MUST remain WASM-safe.

**Rationale**: Acyclic dependency graphs are the foundation of independent
testing, incremental compilation, and selective inclusion (e.g., WASM build
excludes `marque-extract`). Discipline here prevents architectural debt that
cannot be refactored cheaply. The `marque-ism` / `marque-scheme` split
specifically exists so a second marking domain (CUI, NATO, etc.) can reuse
the scheme trait surface without picking up ODNI-specific vocabulary.

### VIII. Authoritative Source Fidelity

Every grammar implementation flows from a single authoritative source.
Don't wing it. Don't fabricate citations.

- **Every grammar has a designated primary source.** For ISM/CAPCO today,
  the primary source is `crates/capco/docs/CAPCO-2016.md` (with the PDF
  original at `crates/capco/docs/original-refs/CAPCO-2016.pdf`), backed by
  the ODNI ISM XML schemas in `crates/ism/schemas/ISM-v2022-DEC/`. Each
  future grammar MUST declare its equivalent at crate creation: a NATO
  security-policy manual, a NARA CUI registry snapshot, a partner-national
  security framework, etc. The source MUST be versioned and vendored in
  `crates/<grammar>/docs/` (or the equivalent); external URLs MUST NOT be
  the primary source.
- **Source-first implementation.** Anyone implementing a rule, marking
  syntax parser, rewrite, page roll-up, or fix proposal for a grammar MUST
  consult the relevant portions of the primary source first and MUST cover
  the nuances and facts the source spells out. "The existing code looked
  like this, so I extended the pattern" is not sufficient — the existing
  code may itself be wrong or incomplete relative to the source. The
  required justification is "the source at §<section> says X, and the
  implementation covers X including edge cases Y and Z."
- **Citation integrity is non-negotiable.** Every citation embedded in a
  rule, diagnostic message, doc comment, plan, or docs file MUST (a) refer
  to a real passage in the authoritative source, (b) accurately reflect
  what that passage says, and (c) be re-verifiable by any reviewer with
  the source in hand. Fabricated, hallucinated, misattributed, or
  silently-drifted citations are a correctness defect of the same severity
  as a wrong predicate — they are not stylistic choices, and "there's a
  citation so someone checked" cannot be allowed to carry the reviewer's
  trust forward. This applies equally to citations written by humans and
  citations written by AI assistance; neither is exempt from verification.
- **Propagation requires re-verification.** When a citation moves — from
  a rule comment into a docs file, from one plan into another, from one
  diagnostic message to a restatement in another — the person (or agent)
  propagating it MUST re-verify the citation against the source at the
  point of propagation. Stale or wrong citations accrete across moves if
  the discipline lapses at any single step.
- **Source revisions are planned migrations.** When a primary source
  revises (CAPCO-2016 → a future revision; ISM-v2022-DEC → a later ODNI
  schema package), the update is a deliberate, reviewed migration — never
  a silent refresh. Every rule, citation, and test fixture that references
  the prior source MUST be re-verified against the new source before the
  migration lands; citations that no longer apply MUST be updated or
  removed. This parallels and reinforces the schema version pin in
  Principle IV.

**Rationale**: `marque` is a compliance tool. Its authority comes from
users being able to trace every rule, every fix, and every rejected
marking back to a passage in the governing source. A wrong rule is a bug;
a wrong citation is worse — it is a bug that *looks like documentation*
and survives PR review precisely because a reviewer trusts that a cited
claim has been checked. Fabrication, including the AI-assisted kind,
poisons that trust. The project cannot ship classified-marking fixes
alongside lies about their provenance and remain the thing it claims to
be. Correctness is the central property; citation discipline is how
correctness becomes auditable.

## Technology Stack Constraints

These technology choices are binding for the current major version. Changes
require a constitution amendment with migration rationale.

| Layer | Required Choice | Locked Because |
|-------|----------------|----------------|
| Language | Rust ≥ 1.85 (edition 2024) | WASM target, memory safety, NSA/CISA guidance |
| Async runtime | Tokio | axum integration, ecosystem standard |
| HTTP server | axum | Tower middleware compatibility |
| Scanner (Phase 1) | memchr | SIMD-accelerated, zero-allocation |
| Token matching (Phase 2) | aho-corasick (native), daachorse (WASM) | Compile-time automaton from CVE tokens |
| Runtime replacement lookup | rapidhash (thread-utils) | Fastest available; existing dep |
| Compile-time replacement lookup | phf | Perfect hash, zero collisions for static keys |
| Schema parsing (build.rs) | quick-xml | CVE/XSD/Schematron at compile time |
| Format extraction | Kreuzberg | 75+ formats, streaming, OCR, SIMD |
| Config parsing | toml + serde | Ecosystem standard |
| Incremental cache store (v0.2) | heed (LMDB) | Embedded, memory-mapped, ACID |
| Cache serialization (v0.2) | rmp_serde (MessagePack) | Compact binary; 2–5× smaller than JSON |
| Document fingerprint | blake3 | Speed; already in dep tree |
| WASM packaging | wasm-pack | Best-in-class Rust→WASM compilation |

**Licensing**: `marque-ism`, `marque-core`, `marque-rules`, `marque-scheme`,
`marque-capco` are Apache-2.0 (or MIT OR Apache-2.0 where dual-licensed).
Enterprise integration components (Office add-ins, managed API) use a
commercial or Elastic License 2.0 tier. This split MUST be preserved;
Apache-2.0 crates MUST NOT gain dependencies that violate that license.

## Development Workflow

### Adding a New Rule

1. Extend Layer 1 predicates in `marque-ism/build.rs` (or the relevant domain
   crate's `build.rs`) if the rule depends on a CVE/schema element not yet
   surfaced.
2. Add a zero-size struct implementing `Rule` in the relevant rule crate
   (e.g., `crates/capco/src/rules.rs`).
3. Register it in the rule-set constructor (e.g., `CapcoRuleSet::new()`).
4. Assign a rule ID: `E###`, `W###`, or `C###`.
5. Write tests that verify the rule fires on known-bad inputs and passes on
   valid inputs BEFORE the implementation is considered complete.
6. Cite the authoritative section (e.g., CAPCO-2016 §H.5) in the rule's
   `name()` / doc comment, and verify the citation against the primary
   source before opening the PR (Principle VIII). A citation that cannot
   be traced to a real passage MUST be removed, not left in place
   pending follow-up.

### Schema Version Updates

Schema version bumps invalidate the entire incremental cache. Announce version
bumps in the changelog. Update `[package.metadata.marque] ism-schema-version`
in `marque-ism/Cargo.toml` intentionally — never as a side effect of a
dependency update.

### Configuration Hygiene

- Rule severity configuration belongs in `.marque.toml` (committed).
- User identity (classifier ID, classification authority) belongs in
  `.marque.local.toml` (gitignored) or environment variables only.
- CI pipelines MUST inject classifier identity via environment variables
  (`MARQUE_CLASSIFIER_ID`), never via committed files.

### Feature Development Sequence

1. `marque-ism`, `marque-scheme`, and `marque-rules` changes first (pure,
   WASM-safe, testable in isolation).
2. `marque-core` changes next (scanner/parser against `marque-ism` types).
3. Domain rule-crate changes (`marque-capco`, future `marque-cui`, etc.) —
   generated + hand-written, tests required.
4. `marque-engine` orchestration last.
5. Integration surfaces (`marque-extract`, `marque-server`, `marque` CLI,
   `marque-wasm`) after the engine is stable.

## Governance

This constitution supersedes all other development practices for the `marque`
project. Any practice not addressed here defaults to the principles above; if
still ambiguous, prefer the simplest approach consistent with Principles I–VII.

**Amendment procedure**:

1. Open a PR with proposed changes to this file.
2. State the version bump type (MAJOR/MINOR/PATCH) and rationale.
3. List all templates and artifacts that must be updated in sync.
4. Apply version bump using semantic versioning:
   - MAJOR: Backward-incompatible principle removals or redefinitions.
   - MINOR: New principle, section, or materially expanded guidance.
   - PATCH: Clarifications, wording, typo fixes, non-semantic refinements.
5. Update `LAST_AMENDED_DATE` to the merge date.

**Compliance review**: All feature plans (`specs/*/plan.md`) MUST include a
"Constitution Check" gate before Phase 0 research and after Phase 1 design.
Violations found at gate MUST be justified in the plan's "Complexity Tracking"
table.

**Runtime guidance**: See `CLAUDE.md` at the workspace root for build commands,
crate responsibilities, and code generation details. Per-crate `README.md`
files carry crate-specific invariants.

**Version**: 1.1.1 | **Ratified**: 2026-03-12 | **Last Amended**: 2026-04-20
