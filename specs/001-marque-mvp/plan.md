# Implementation Plan: Marque MVP — CAPCO Marking Linter and Fixer

**Branch**: `001-marque-mvp` | **Date**: 2026-04-08 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-marque-mvp/spec.md`

## Summary

Deliver the smallest end-to-end slice of `marque` that lints raw text input for
CAPCO portion, banner, and CAB marking violations; auto-applies high-confidence
fixes (default threshold `0.95`) with a complete audit trail; honors layered
project/local/env/CLI configuration; and exposes the same lint and fix capabilities
through a CLI binary (file paths + stdin) and a WASM web-worker build, with
byte-identical diagnostics across both. The slice deliberately excludes file-format
extraction, the REST server, the LMDB cache, Office add-ins, and metadata
sanitization — those are tracked as future feature slices.

The crate skeleton already exists in the workspace. The principal work for this
slice is (a) introducing a new `marque-ism` crate to own the ISM vocabulary types
and generated enums (resolving the circular dependency that prevented
`IsmAttributes` from using strongly-typed CVE fields), (b) replacing the
placeholder ODNI schema code-generation in `marque-ism/build.rs` with real
CVE/Schematron parsing, (c) implementing the hand-written Layer 2 rule set
covering the highest-frequency CAPCO violations, (d) wiring `Engine` and the
CLI/WASM front ends to the configuration model, and (e) standing up a
representative test corpus and benchmark harness sufficient to verify the success
criteria.

## Technical Context

**Language/Version**: Rust 1.85+ (edition 2024) — pinned by constitution Tech Stack
**Primary Dependencies**: `memchr` (Phase 1 scanner), `aho-corasick` (native Phase 2)
+ `daachorse` (WASM Phase 2), `phf` (compile-time replacement maps),
`rapidhash` via `recoco-utils::thread-utils` (runtime replacement maps),
`quick-xml` (build-time CVE/Schematron parsing), `serde` + `toml` (config),
`tokio` (async runtime, native targets only), `tower` (CLI middleware seams,
deferred to v0.2 for server), `wasm-bindgen` + `wasm-pack` (WASM target),
`clap` (CLI), `tracing` (structured logging via `MARQUE_LOG`).
**Storage**: None for the MVP. The LMDB incremental cache is explicitly out of
scope. Diagnostic and audit output goes to stdout/stderr or to a caller-provided
sink. Configuration is read from TOML files on disk; the spec's FR-013 forbids
retaining document content beyond the processing pass.
**Testing**: `cargo test` for unit and integration tests inside each crate;
`insta` snapshot tests for diagnostic and audit-record outputs;
`criterion` benchmarks for the SC-001 ≤16ms p95 target and the SC-005 linear
scaling target; a curated `tests/corpus/` of known-bad and known-good marking
fixtures shared across crates via a `dev-dependency` corpus loader.
**Target Platform**: Linux/macOS x86_64 and aarch64 for the CLI (developer
workstations and CI); browser WASM (Chromium-class engines) for the web-worker
build. Windows is not blocked but is not in the MVP test matrix.
**Project Type**: Cargo workspace producing one CLI binary, one WASM artifact,
and a set of internal libraries. No frontend application, no backend service in
this slice.
**Performance Goals**:
- ≤16ms p95 lint latency on inputs ≤10KB of raw text on commodity dev hardware
  (SC-001). Reference baseline: x86_64 ≥3.0 GHz single-thread (e.g. modern
  laptop-class CPU), warm cache, `--release` build, no tracing subscriber.
  Benchmarks in `benches/` document the exact machine they were last measured on.
- Zero heap allocation per candidate span detected in the scanner phase
  (constitution Principle II).
- Linear throughput scaling across at least one order of magnitude of input size
  with no super-linear slowdown (SC-005).
- Native and WASM builds produce byte-identical diagnostics (rule IDs, spans,
  messages) for the same input (SC-008).
**Constraints**:
- No I/O, no format dependencies, no platform-specific code in `marque-core`
  (constitution Principle III; required for WASM safety).
- `marque-core`, `marque-rules`, `marque-capco`, `marque-engine`, `marque-wasm`
  remain Apache-2.0 and MUST NOT gain dependencies that violate that license.
- Classifier identity MUST live only in `.marque.local.toml` or env vars; no
  fixture or example may include it (SC-006, FR-010).
- `Fix` records and `AuditRecord`s exist for every applied change without
  exception, including 1.0-confidence fixes (FR-005, constitution Principle V).
- Pinned ODNI schema version `ISM-v2022-DEC` per `marque-capco/Cargo.toml`
  `[package.metadata.marque] ism-schema-version` (FR-011, design doc §5).
**Scale/Scope**:
- Documents up to ~10KB for the interactive latency target; larger documents
  must still process correctly but are not subject to SC-001.
- Initial rule set: ~10 rules covering banner abbreviation, separator-count
  normalization, missing/misplaced USA trigraph, misordered blocks,
  declassification-in-banner, deprecated-marking conversions (including X-shorthand
  date markings), and the corrections-map typo path (E001–E008, W001, C001 from
  the design doc §6 table).
- Test corpus initial size: ≥40 known-bad fixtures + ≥20 known-good fixtures
  spanning the rule set, sized to make SC-002 (≥95%) and SC-003 (≥95%)
  meaningful.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

This MVP slice was scoped against the constitution from the start. Each principle
is checked below.

| Principle | Status | Notes |
|-----------|--------|-------|
| I — Uncompromising Performance | **Pass** | SC-001 commits to ≤16ms p95 ≤10KB; SC-005 commits to linear scaling. `criterion` benchmarks are part of Phase 1 contracts. |
| II — Zero-Copy, Streaming Core | **Pass** | The scanner produces `Span` byte offsets only; `IsmAttributes` uses `Box<[T]>`; FR-013 forbids content retention. No stage holds whole documents. |
| III — Format-Agnostic Core / WASM Safety | **Pass** | `marque-core` has no format deps; the slice explicitly excludes `marque-extract`. The WASM build is part of the MVP and SC-008 enforces native/WASM parity. |
| IV — Two-Layer Rule Architecture | **Pass** | Phase 0 research item: replace placeholder `marque-capco/build.rs` with real CVE + Schematron parsing. Layer 2 rules consume only Layer 1 predicates and are stateless. |
| V — Audit-First Compliance | **Pass** | FR-005 + FR-010 + SC-004 + SC-006 collectively require complete audit records, classifier identity hygiene, and audit-completeness verification. `--dry-run` is FR-006. |
| VI — Dataflow Pipeline Model | **Pass** | The CLI and WASM front ends share `Engine` as the pipeline core; the configuration of source/sink at the boundary is the only difference. The server slice is deferred. |
| VII — Crate Discipline and Dependency Hygiene | **Pass** | Touched crates: `marque-ism` (new — leaf dependency, no upward edges), `marque-core`, `marque-rules`, `marque-capco`, `marque-engine`, `marque-config`, `marque-wasm`, and the `marque` CLI. `marque-ism` extracts ISM vocabulary types and build.rs codegen from `marque-core`/`marque-capco` to resolve a circular dependency; it sits below both in the graph. `marque-extract` and `marque-server` are untouched. |

**Tech-stack constraints check**: The MVP introduces one new crate (`marque-ism`)
but no new locked-tier dependencies — all deps (`quick-xml`, `phf`,
`aho-corasick`, `memchr`) are already in the workspace. All Phase 1 design
choices fall inside the constitution's existing "Required Choice" table. No
constitution amendment required for this slice.

**Verdict**: Constitution Check **passes** with no violations. Complexity Tracking
section below remains empty.

## Project Structure

### Documentation (this feature)

```text
specs/001-marque-mvp/
├── plan.md              # This file
├── spec.md              # Feature specification (already authored)
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── cli.md           # CLI command shapes and exit codes
│   ├── diagnostic.json  # JSON Schema for the structured diagnostic stream
│   └── audit-record.json# JSON Schema for the audit record stream
├── checklists/
│   └── requirements.md  # Spec quality checklist (already authored)
└── tasks.md             # Phase 2 output (NOT created by /speckit.plan)
```

### Source Code (repository root)

The crate skeleton already exists. The MVP slice touches the crates marked with
`(MVP)` below; the others are present but untouched in this slice.

```text
crates/
├── marque-ism/          (MVP)  ISM vocabulary types, generated CVE enums, build.rs codegen
│   ├── build.rs         (MVP) CVE XML + Schematron → OUT_DIR/{values,validators,migrations}.rs
│   ├── schemas/ISM-v2022-DEC/ (already in tree; moved from marque-capco)
│   ├── src/
│   │   ├── lib.rs       (MVP) pub mod + re-exports
│   │   ├── span.rs      (MVP) Span, MarkingCandidate, MarkingType, Zone, DocumentPosition
│   │   ├── attrs.rs     (MVP) IsmAttributes (strongly-typed generated enum fields)
│   │   ├── generated.rs (MVP) include!() wrappers for OUT_DIR generated code
│   │   └── token_set.rs (MVP) TokenSet trait + CapcoTokenSet (Aho-Corasick from generated data)
│   └── tests/
│
├── marque-core/         (MVP)  scanner + parser (produces IsmAttributes from byte buffers)
│   ├── src/
│   │   ├── lib.rs
│   │   ├── scanner/     (MVP) memchr-based candidate detection
│   │   ├── parser/      (MVP) aho-corasick token extraction
│   │   └── error.rs
│   └── tests/
│       ├── scanner_smoke.rs
│       └── parser_smoke.rs
│
├── marque-rules/        (MVP)  Rule trait + Diagnostic + FixProposal + Severity + AppliedFix
│   └── src/lib.rs       (MVP) trait definitions only, no implementations
│
├── marque-capco/        (MVP)  CAPCO Layer 2 hand-written rules
│   ├── src/
│   │   ├── lib.rs       (MVP) CapcoRuleSet::new() registration
│   │   └── rules.rs     (MVP) E001..E008, W001, C001
│   └── tests/
│       ├── rules_e001_to_e008.rs
│       └── corrections_map.rs
│
├── marque-engine/       (MVP)  Engine pipeline orchestration (single-doc path only)
│   ├── src/
│   │   ├── lib.rs
│   │   ├── engine.rs    (MVP) Engine::lint, Engine::fix, dry-run mode
│   │   ├── clock.rs     (MVP) Clock trait, SystemClock, FixedClock
│   │   ├── overlap.rs   (MVP) deterministic reverse-order span fix application
│   │   └── audit.rs     (MVP) audit emission
│   └── tests/
│       ├── lint_pipeline.rs
│       ├── fix_pipeline.rs
│       └── audit_completeness.rs
│
├── marque-config/       (MVP)  layered config loader
│   ├── src/lib.rs       (MVP) project + local + env + CLI precedence
│   └── tests/precedence.rs
│
├── marque-wasm/         (MVP)  wasm-pack target exposing lint/fix
│   └── src/lib.rs       (MVP) wasm-bindgen wrapper around Engine
│
├── marque-extract/      (DEFERRED — not touched in MVP slice)
└── marque-server/       (DEFERRED — not touched in MVP slice)
│
└── marque/              (MVP)  CLI binary
    ├── src/main.rs      (MVP) clap setup, file/stdin input, exit codes
    ├── src/render.rs    (MVP) human + JSON diagnostic renderers
    └── tests/cli_smoke.rs

tests/corpus/            (MVP)  shared known-bad / known-good fixtures
├── valid/
└── invalid/

benches/                 (MVP)  criterion benchmarks for SC-001 / SC-005
├── lint_latency.rs
└── linear_scaling.rs
```

**Structure Decision**: Workspace-level Cargo project. The MVP introduces one new
crate (`marque-ism`) to own ISM vocabulary types and generated enums, resolving
the circular dependency that prevented `IsmAttributes` from using strongly-typed
CVE fields. The MVP touches nine components (eight crates + the CLI binary) and
adds two new top-level directories (`tests/corpus/` and `benches/`) for shared
fixtures and benchmarks.

## Phase 0: Research

The plan-level unknowns that need resolution before Phase 1 design are recorded
in `research.md`. Summary of items dispatched and resolved there:

- **R-1**: How to parse the ODNI ISM CVE XML enumerations in `build.rs` and emit
  Rust enum types with stable identifiers.
- **R-2**: How to parse the ODNI Schematron rules and emit predicate functions
  that the Layer 2 rules can consume without re-interpreting at runtime.
- **R-3**: How to handle the CAPCO deprecated-marking migration table
  (specifically the X-shorthand date markings) at confidence `≥0.95` so they fall
  inside the default auto-fix threshold per the spec's clarification.
- **R-4**: Concrete shape of the structured diagnostic and audit-record JSON
  output (see `contracts/`).
- **R-5**: Audit log destination strategy (stdout vs stderr vs file) and how to
  expose it through CLI flags without leaking classifier identity into committed
  test fixtures.
- **R-6**: Test corpus sourcing strategy — how to assemble ≥40 known-bad +
  ≥20 known-good fixtures that exercise the rule set without checking in
  classified content (and without inviting a counter-intel review of the repo).
- **R-7**: WASM Phase 2 token-matching engine selection between `aho-corasick`
  and `daachorse` measured against binary size, build time, and runtime cost.
- **R-8**: How `recoco-utils::ConcurrencyController` integrates with the
  single-doc `Engine` path (it is a `BatchEngine` concern; confirm it does not
  leak into the MVP slice).

**Output**: `research.md` with each item resolved as `Decision / Rationale /
Alternatives considered`.

## Phase 1: Design & Contracts

**Prerequisites**: `research.md` complete.

### Data model

`data-model.md` enumerates the canonical types implemented by `marque-core` and
`marque-rules`, derived directly from the spec's "Key Entities" section and the
design doc §6 trait sketches:

- `Span` — `{ start: usize, end: usize }` byte range; `Copy`.
- `MarkingCandidate` — `{ span: Span, kind: CandidateKind }` where
  `CandidateKind ∈ { Portion, Banner, Cab }`.
- `IsmAttributes` — classification, SCI controls, SAR identifiers, dissem
  controls, REL TO trigraphs, declassify info, classified-by, derived-from,
  declass-exemption. Fields use `Box<[T]>` (constitution Principle II).
- `RuleContext` — `{ marking_type, zone, position }`.
- `Diagnostic` — `{ rule, severity, span, message, citation, fix? }`.
- `Fix` — `{ replacement, confidence, audit, migration_ref? }` with
  `confidence: f32 ∈ [0.0, 1.0]`.
- `AuditRecord` — `{ rule, original, replacement, confidence, timestamp,
  classifier_id? }`.
- `Configuration` — merged view of project + local + env + CLI layers, exposing
  rule severities, corrections map, classifier identity, schema version, and the
  active confidence threshold (default `0.95`).

State transitions: only `Diagnostic.fix` has a meaningful transition —
`Suggested → Applied` when its confidence ≥ the configured threshold and the
engine is invoked in fix mode (not dry-run). `AuditRecord` is immutable on
creation.

### Contracts

`contracts/` captures the externally observable interfaces of the MVP slice:

- **`cli.md`** — `marque check [PATH...|-]` and `marque fix [PATH...|-]` with
  flags for `--dry-run`, `--confidence-threshold`, `--config`, `--format
  human|json`, plus exit codes (`0` clean, `1` diagnostics found,
  `2` warnings only (no errors), `64` invalid usage, `65` config error,
  `74` I/O error). The CLI accepts file path arguments and stdin via `-` per
  FR-014a.
- **`diagnostic.json`** — JSON Schema for the structured diagnostic stream
  emitted under `--format json`. Stable field names: `rule`, `severity`, `span`,
  `message`, `citation`, `fix` (nullable). This is the contract SC-008 binds
  native and WASM outputs against.
- **`audit-record.json`** — JSON Schema for the audit record stream. Required
  fields: `rule`, `original`, `replacement`, `confidence`, `timestamp_iso8601`,
  `classifier_id` (nullable). FR-005 + SC-004.

### Quickstart

`quickstart.md` is the smoke test a contributor can run end-to-end after a
`cargo build` to convince themselves the slice is alive:

1. Lint a known-bad raw text fixture from `tests/corpus/invalid/` and observe a
   diagnostic with the expected rule ID and span.
2. Run the same input through `marque fix --dry-run` and observe an audit record
   with no file changes.
3. Run again without `--dry-run` and observe (a) the file modified, (b) the
   audit record on stderr, (c) a re-lint pass exits clean.
4. Build the WASM target with `wasm-pack build crates/marque-wasm --target web
   --profile release-wasm` and verify a manual `lint(text)` call from the
   bundled HTML harness produces byte-identical diagnostics to the CLI output.

### Agent context update

After Phase 1 artifacts are written, run
`.specify/scripts/bash/update-agent-context.sh claude` to refresh
`CLAUDE.md` with the MVP-relevant tech additions (none new beyond what is
already documented; the script is idempotent).

### Re-evaluation of Constitution Check post-design

The Phase 1 design introduces one new crate (`marque-ism`) as a leaf dependency
to resolve the circular dependency between `marque-core` (owns `IsmAttributes`)
and `marque-capco` (generates the enum types `IsmAttributes` fields need). This
adds no upward edges — `marque-ism` sits below both in the graph. No new
locked-tier dependencies are introduced. The constitution check from
§"Constitution Check" remains **Pass** with no violations.

## Complexity Tracking

> *No constitution violations to justify. This section intentionally left empty.*
