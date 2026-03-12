<!--
SYNC IMPACT REPORT
==================
Version change: (new) → 1.0.0
Initial ratification — no prior version.

Modified principles: N/A (initial creation)

Added sections:
  - Core Principles (I–VII)
  - Technology Stack Constraints
  - Development Workflow
  - Governance

Removed sections: N/A

Templates checked/updated:
  ✅ .specify/templates/plan-template.md — "Constitution Check" section present; gates
     align with principles below (speed, WASM safety, crate discipline, audit trail).
     No edits required; template is generic enough to remain valid.
  ✅ .specify/templates/spec-template.md — Mandatory sections (user scenarios,
     requirements, success criteria) are compatible with all seven principles.
     No edits required.
  ✅ .specify/templates/tasks-template.md — Phase/task structure supports two-layer
     rule architecture and incremental delivery. No edits required.
  ✅ .specify/templates/constitution-template.md — Source template; not modified
     (operating on memory/constitution.md per spec).

Follow-up TODOs:
  - TODO(RATIFICATION_DATE): Set to 2026-03-12 (today, first creation). Confirm
    with project owner if a different governance start date is preferred.
  - TODO(REPOSITORY_URL): Placeholder GitHub URL in Cargo.toml
    (https://github.com/placeholder/marque) — update when repo is public.
-->

# marque Constitution

## Core Principles

### I. Uncompromising Performance

Performance is the primary value proposition of `marque`. "Perceptual instantaneity" is
non-negotiable — the tool MUST feel like magic at every scale.

- Interactive use (single field, single file) MUST complete in < 16ms for typical inputs.
- Batch processing MUST scale linearly; throughput MUST be benchmarked, not assumed.
- Every performance decision MUST be backed by measurement against a benchmark.
- The tool MUST be async-first throughout — no blocking operations on the hot path.
- SIMD-accelerated primitives (`memchr`, Aho-Corasick, BLAKE3) MUST be used wherever
  the standard library provides a slower alternative.

**Rationale**: The problem domain (1M+ cleared personnel, 12+ marking tasks/day) makes
speed a multiplier on adoption and impact. A slow linter will be bypassed; a fast one
becomes invisible infrastructure.

### II. Zero-Copy, Streaming Core

The memory model is non-negotiable. `marque-core` MUST operate without heap allocation
on the hot path.

- All candidate detection MUST produce `Span` values (byte offsets into original buffers),
  never copies of content.
- Documents MUST stream through the pipeline in chunks; no stage may hold an entire
  document in memory.
- `IsmAttributes` fields MUST use `Box<[T]>` (not `Vec<T>`) to eliminate over-allocation.
- The scanner phase MUST produce zero heap allocations per candidate span detected.
- Cached `LintResult` spans MUST remain valid via fingerprint guarantee; no span
  re-computation on cache hit is permitted.

**Rationale**: Sensitive content (classified documents) MUST be minimized in memory
footprint. Zero-copy also enables future secure-enclave (SGX/TrustZone) integration
without architectural changes.

### III. Format-Agnostic Core / WASM Safety

`marque-core` knows nothing about file formats. This boundary MUST NOT be crossed.

- `marque-core` MUST compile to WASM with `wasm-pack` without modification.
- `marque-core` MUST have zero I/O dependencies, no format adapters, and no
  platform-specific code.
- All format extraction MUST live in `marque-extract` (not in WASM builds).
- The WASM API surface (`lint`, `fix`) MUST accept raw `&str` / byte buffers;
  format conversion is the caller's responsibility.
- WASM binary size MUST be considered when choosing data structure alternatives
  (e.g., `daachorse` over `aho-corasick` where more memory-compact).

**Rationale**: The WASM target enables browser extensions, Office add-ins, and
web form integrations — critical distribution channels. Coupling format logic to
core would permanently close these channels.

### IV. Two-Layer Rule Architecture

Rule implementations MUST follow the two-layer model. Collapsing layers is prohibited.

- **Layer 1 (generated)**: `marque-capco/build.rs` MUST parse ODNI ISM schema files
  at compile time and emit only binary valid/invalid predicates (`values.rs`,
  `validators.rs`, `migrations.rs`). No remediation logic belongs here.
- **Layer 2 (hand-written)**: `Rule` implementations MUST consume Layer 1 predicates
  to detect violations, classify *why* a violation occurred, determine fix confidence,
  and cite the CAPCO section. Intelligence lives here, not in generated code.
- Rule IDs MUST follow the convention: `E###` (error), `W###` (warning), `C###`
  (correction).
- Every `Rule` implementation MUST be stateless; config-dependent behavior is handled
  by the engine, not the rule.
- The active ODNI schema version MUST be pinned in `[package.metadata.marque]
  ism-schema-version` in `marque-capco/Cargo.toml`. Schema version bumps MUST be
  intentional, never silent.

**Rationale**: The separation makes generated predicates auditable against the official
spec, while keeping product differentiation (the "why" and "how to fix") in
maintainable hand-written code. Schema updates become a controlled build event.

### V. Audit-First Compliance

Every fix MUST produce a complete audit trail. Auditability is non-negotiable in
the IC/DoD compliance context.

- Every `Fix` MUST carry an `AuditRecord` regardless of confidence level, including
  1.0-confidence fixes.
- `AuditRecord` MUST record: rule ID, original text, replacement text, confidence
  score, timestamp, and classifier ID (when present).
- User identity (classifier ID, classification authority) MUST NEVER appear in
  committed configuration files. It MUST live only in `.marque.local.toml`
  (gitignored) or environment variables.
- `FixResult` MUST NOT be cached. Only `LintResult` may be cached.
- `--dry-run` MUST always produce full audit output without writing changes.

**Rationale**: Misclassification and improper fix application in the IC carry legal
and security consequences. Every automated change must be traceable to a person
and a rule version.

### VI. Dataflow Pipeline Model

The processing pipeline is a chain of async streams. It MUST NOT be implemented as
a deep call stack or a monolithic function.

- Each pipeline stage MUST be a `Stream` implementation; stages communicate via
  async stream composition, not function calls.
- Middleware (auth, logging, rate limiting, backpressure) MUST insert between stages
  as Tower layers — never inside stage implementations.
- The CLI, WASM, and server targets MUST be different `Source`/`Sink` configurations
  wired to the same shared pipeline core.
- New rule sets, format adapters, and integration surfaces MUST slot in without
  modifying existing stage code (open/closed principle).
- `BatchEngine` MUST use semaphore-based backpressure (row + byte) via
  `recoco-utils::ConcurrencyController`. Results MAY arrive in completion order,
  not submission order — callers MUST correlate via echoed `id`.

**Rationale**: The dataflow model is what makes `marque` embeddable in web workers,
CLI shells, and microservices without code duplication. It also naturally supports
future secure-enclave streaming without pipeline redesign.

### VII. Crate Discipline and Dependency Hygiene

The workspace dependency graph MUST be one-directional and acyclic.

- The canonical dependency graph is:
  `marque-core ← marque-rules ← marque-capco → marque-engine ← marque-extract`
  `marque-engine ← marque-config`
  `marque-engine ← marque-wasm`
  `marque-extract ← marque-server ← marque (CLI)`
- No crate may introduce a circular dependency. `cargo check --workspace` MUST
  pass on every commit.
- `marque-core` MUST have zero format dependencies and MUST remain WASM-safe
  (see Principle III).
- `marque-rules` MUST contain only trait definitions; no implementations.
- Every crate MUST have a single, clear responsibility documented in its `CLAUDE.md`
  or crate-level doc comment.
- New rule crate families (e.g., `marque-cui`, `marque-ntk`) MUST follow the
  `build.rs` → generated code pattern established by `marque-capco`.

**Rationale**: Acyclic dependency graphs are the foundation of independent testing,
incremental compilation, and selective inclusion (e.g., WASM build excludes
`marque-extract`). Discipline here prevents architectural debt that cannot be
refactored cheaply.

## Technology Stack Constraints

These technology choices are binding for the current major version. Changes require
a constitution amendment with migration rationale.

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
| Incremental cache store | heed (LMDB) | Embedded, memory-mapped, ACID |
| Cache serialization | rmp_serde (MessagePack) | Compact binary; 2–5× smaller than JSON |
| Document fingerprint | blake3 | Speed; already in dep tree |
| WASM packaging | wasm-pack | Best-in-class Rust→WASM compilation |

**Licensing**: `marque-core`, `marque-rules`, `marque-capco` are Apache-2.0.
Enterprise integration components (Office add-ins, managed API) use a commercial or
Elastic License 2.0 tier. This split MUST be preserved; Apache-2.0 crates MUST NOT
gain dependencies that violate that license.

## Development Workflow

### Adding a New Rule

1. Implement Layer 1 predicates in `marque-capco/build.rs` or extend the CVE parsing
   to cover new schema elements.
2. Add a zero-size struct implementing `Rule` in `crates/marque-capco/src/rules.rs`.
3. Register it in `CapcoRuleSet::new()`.
4. Assign a rule ID: `E###`, `W###`, or `C###`.
5. Write tests that verify the rule fires on known-bad inputs and passes on valid inputs
   before the implementation is considered complete.
6. Cite the CAPCO section in the rule's `name()` or documentation.

### Schema Version Updates

Schema version bumps invalidate the entire incremental cache. Announce version bumps
in the changelog. Update `[package.metadata.marque] ism-schema-version` in
`marque-capco/Cargo.toml` intentionally — never as a side effect of a dependency update.

### Configuration Hygiene

- Rule severity configuration belongs in `.marque.toml` (committed).
- User identity (classifier ID, classification authority) belongs in
  `.marque.local.toml` (gitignored) or environment variables only.
- CI pipelines MUST inject classifier identity via environment variables
  (`MARQUE_CLASSIFIER_ID`), never via committed files.

### Feature Development Sequence

1. `marque-core` and `marque-rules` changes first (pure, WASM-safe, testable in
   isolation).
2. `marque-capco` changes next (generated + hand-written rules, tests required).
3. `marque-engine` orchestration last.
4. Integration surfaces (`marque-extract`, `marque-server`, `marque` CLI,
   `marque-wasm`) after engine is stable.

## Governance

This constitution supersedes all other development practices for the `marque` project.
Any practice not addressed here defaults to the principles above; if still ambiguous,
prefer the simplest approach consistent with Principles I–VII.

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
Violations found at gate MUST be justified in the plan's "Complexity Tracking" table.

**Runtime guidance**: See `CLAUDE.md` at the workspace root for build commands,
crate responsibilities, and code generation details.

**Version**: 1.0.0 | **Ratified**: 2026-03-12 | **Last Amended**: 2026-03-12
