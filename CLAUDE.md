<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> [!IMPORTANT]
> ## Project Constitution
>
>  The project constitution is the authoritative source for principles governing all maintenance.

@.specify/memory/constitution.md

## What This Is

`marque` is a **general-purpose rule engine for fast text processing** — rules produce warnings, errors, fixes, and transformations, each with a confidence score the engine uses to decide what to apply vs. surface as a suggestion. Built in the style of `ruff`: designed for perceptual instantaneity at any scale, operating on raw byte buffers with SIMD-accelerated scanning and an Aho-Corasick parser.

The MVP ships a CAPCO/ISM classification-marking rule set (`marque-ism` + `marque-capco`) targeting ODNI ISM-v2022-DEC, but that is **one application** of the engine, not its identity. The roadmap expands into other U.S. Government control markings (CUI), foreign and multinational classification/control systems (NATO, FGI, JOINT), and general-purpose text lint/transformation domains. Any crate named `marque-*` other than `marque-ism`/`marque-capco` is domain-neutral infrastructure and should stay that way.

Support for a wide range of document formats via `marque-extract` (Kreuzberg wrapper) is in progress.

## Build Commands

```bash
# Build the workspace
cargo build

# Build CLI binary only
cargo build -p marque

# Build server only
cargo build -p marque-server

# Build WASM target (requires wasm-pack)
wasm-pack build crates/wasm --target web --profile release-wasm

# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p marque-core
cargo test -p marque-capco

# Run a single test by name
cargo test -p marque-core scanner::tests::detects_portion_marking

# Run the CLI
cargo run -p marque -- check <file>
cargo run -p marque -- fix <file>
cargo run -p marque -- fix --dry-run <file>

# Lint the workspace
cargo clippy --workspace

# Check compilation without linking
cargo check --workspace
```

**Logging**: Set `MARQUE_LOG=marque=debug` (or `trace`) to increase verbosity.
**Classifier ID**: Set `MARQUE_CLASSIFIER_ID=<id>` to inject classifier identity into audit records.

## Architecture

### Crate Dependency Graph

```
marque-ism    ←── marque-core ────────────────────┐
marque-ism    ←── marque-rules ←── marque-capco ──┤
marque-scheme ←──────────────────  marque-capco ──┤
                                                  ↓
                                            marque-engine ←── marque-config
                                            ↑    ↑
                                   marque-wasm  marque-extract (non-WASM only)
                                            ↑
                                      marque-server
                                            ↑
                                       marque (CLI)
```

Read `A ←── B` as "`B` depends on `A`". `marque-rules` does NOT depend on
`marque-core`. `marque-capco` does NOT depend on `marque-core`. `marque-engine`
is the sole convergence point that pulls both chains together. `marque-scheme`
has no runtime deps on `marque-ism`/`marque-core`/`marque-rules`.

### Crate Responsibilities

| Crate | Role |
|-------|------|
| `marque-ism` | ISM vocabulary types + generated CVE enums + `Span` + `IsmAttributes`. **WASM-safe** — build-time XML parsing only, no runtime I/O. Owns `build.rs` + ODNI schemas. |
| `marque-core` | Scanner + parser. **WASM-safe** — no I/O, no format deps, operates on `&[u8]`. Produces `IsmAttributes` from byte buffers. |
| `marque-rules` | Trait definitions only: `Rule`, `Diagnostic`, `FixProposal`, `Severity`, `AppliedFix`. No implementations. |
| `marque-scheme` | Domain-neutral trait surface for structured marking schemes. Defines `MarkingScheme`, `Lattice`, `BoundedLattice`, `Category`/`AggregationOp`/`CategoryShape`, `Constraint`, `Parsed<M>`, `Scope`, `PageRewrite`, and built-in lattice constructors (`OrdMax`, `OrdMin`, `FlatSet`, `IntersectSet`, `SupersessionSet`, `ModeSet`, `MaxDate`, `OptionalSingleton`, `Product`). Zero runtime deps; no dependency on `marque-ism`. Phase B landed the recursive-lattice surface — see `docs/plans/2026-04-19-recursive-lattice-and-decoder.md`. |
| `marque-capco` | CAPCO Layer 2 rule implementations. Consumes generated predicates from `marque-ism`. Also hosts `CapcoScheme`, the `marque-scheme` adapter over `IsmAttributes`; `SciSet`/`SarSet`/`FgiSet` lattice types (`marque_capco::lattice`); and tetragraph expansion tables (`marque_capco::vocab`). |
| `marque-engine` | Pipeline orchestration: `Engine` (single doc) and `BatchEngine` (async concurrent). |
| `marque-extract` | Kreuzberg wrapper for 75+ document formats + OCR + metadata extraction. Alternately a narrowing custom or pieced together use of other libraries (Kreuzberg has some licensing complication) **Not in WASM.** |
| `marque-config` | Layered config loading from `.marque.toml` → `.marque.local.toml` → env vars. |
| `marque-wasm` | `wasm-pack` target. Exposes `lint`/`fix` to web workers. Format extraction is caller's responsibility. |
| `marque-server` | axum REST microservice wrapping `marque-engine`. Auth/logging via Tower middleware. |
| `marque` | Thin CLI binary. Subcommands: `check`, `fix`, `metadata`. |

### Processing Pipeline

```
Source → [marque-extract] → TextStream → [Scanner] → SpanStream
       → [Parser] → AttributeStream → [Rules] → DiagnosticStream → Output
```

- **Phase 1 (Scanner)**: `memchr`-based SIMD candidate detection — finds portions `(...)`, banners, and CABs with zero heap allocation.
- **Phase 2 (Parser)**: Aho-Corasick automaton over CVE token list extracts `IsmAttributes` from each candidate span.
- **Phase 3 (Engine)**: Each `Rule` receives `(&IsmAttributes, &RuleContext)` and returns `Vec<Diagnostic>`. Fixes carry a confidence score (0.0–1.0); the engine applies those at or above the threshold in reverse span order.

### Two-Layer Rule Architecture

- **Layer 1 (generated)**: `marque-ism/build.rs` parses ODNI ISM XML schemas at build time → `OUT_DIR/{values,validators,migrations}.rs`, included via `marque-ism/src/generated.rs`. Outputs binary valid/invalid predicates only. Phase 5 added vocabulary metadata generation from the ODNI JSON sidecar — authority, owner/producer, deprecation, URN, schema version, and portion/banner forms — exposed through `Vocabulary<S>` (see Key Types).
- **Layer 2 (hand-written and declarative)**: `Rule` implementations in `crates/capco/src/rules.rs` consume Layer 1 predicates from `marque-ism`, classify *why* a violation occurred, determine fixes and confidence levels, and cite the CAPCO section. Phase 4+ added a *declarative* second form: dyadic invariants (conflict, requires, implies, supersedes) and page-level rewrites are declared as `Constraint` / `PageRewrite` data on `CapcoScheme` (see `crates/capco/src/scheme.rs`) rather than as procedural rule bodies. The shared evaluator in `marque-scheme` runs them; the engine's topological scheduler (`marque-engine::scheduler`) orders rewrites by their `reads` / `writes` axes and rejects cycles or unannotated `Custom` axes at `Engine::new`. See `crates/capco/README.md` for the worked example.

### SCI Compartments (Hybrid CVE + Structural)

SCI markings need more than a flat CVE enum because CAPCO-2016 §A.6 defines a compositional grammar: `CONTROL-COMP (SPACE SUB-COMP)*(-COMP (SPACE SUB-COMP)*)*` (e.g. `SI-G ABCD DEFG-MMM AACD` where `SI` is the control, `G` and `MMM` are compartments, `ABCD`/`DEFG` are sub-compartments of `G`, and `AACD` is a sub-compartment of `MMM`). Pure CVE lookup cannot round-trip this — the vocabulary only lists pre-registered compounds (`SI-G`, `HCS-P`, etc.), not the open-ended compartment/sub-compartment tail.

The hybrid approach: the CVE vocabulary generated from `CVEnumISMSCIControls.xml` gives bare-system recognition and the set of pre-registered compounds; a structural subparser (`parse_sci_block` in `marque-core/src/parser.rs`) handles the full §A.6 grammar and emits `SciMarking` entries. The subparser is dispatched before the CVE exact-match path and gated on `contains('-') || contains('/') || is_bare_cve_value || (custom-control shape ∧ ¬ known non-SCI token)` so plain two-letter tokens (NF, RD) still fall through to the dissem/non-IC/SAR/AEA chain, while standalone custom controls like `99` (e.g., `TOP SECRET//99//NOFORN`) reach the structural path.

`IsmAttributes` exposes both `sci_markings: Box<[SciMarking]>` (authoritative structural form — control system + compartments + sub-compartments) and the original `sci_controls: Box<[SciControl]>` (CVE enum projection) for back-compat with existing consumers. `canonical_enum` on a `SciMarking` is populated only when the bare control or `{ctrl}-{first_comp}` matches a CVE value AND no sub-compartments are present; anything richer is structural-only.

**Phase B canonicalization.** Post-Phase-B, `SciSet` (in `marque_capco::lattice`, the lattice form of SCI state) is the canonical page-context storage: it implements `Lattice`, round-trips with `[SciMarking]` via `SciSet::from_markings` / `SciSet::to_markings`, and composes through `CapcoScheme::project(Scope::Page, ...)`. `SciSet` (and `SarSet`) deliberately do **not** implement `BoundedLattice` — SCI control systems and SAR program identifiers are both agency-extensible open sets, so no lawful finite `top` exists. Use `SciSet::empty()` / `SciSet::default()` when you need the lattice bottom. `IsmAttributes::sci_controls` stays populated for rules that currently read it, but is a compatibility view scheduled for removal in Phase C or D when no rule references it. New rules that need compartment / sub-compartment semantics should read `sci_markings` or construct an `SciSet`; rules that just need "which bare control systems appear" can stay on `sci_controls` until the migration closes.

Banner roll-up for SCI (E035) uses `PageContext::expected_sci_markings()`, which unions compartments and sub-compartments across all portions on the page and sorts per §A.6 p15 (numeric first, alpha after). Authority: CAPCO-2016 §A.6 (grammar, canonical example p16) + §H.4 (per-system banner precedence).

### SAR (Special Access Required)

SAR (Special Access Required) markings are modeled structurally, not as a CVE-derived enum. The ODNI public `CVEnumISMSAR.xml` is empty because SAR program identifiers are agency-assigned codewords not centrally registered. `marque-ism::SarMarking` captures the full hierarchy — programs, compartments, sub-compartments — parsed by a hand-written subparser in `marque-core` (see `parse_sar_category`). The six SAR rules (E026–E031) validate syntax, ordering, classification constraints, and banner roll-up per CAPCO-2016 §H.5.

### Key Types

- `IsmAttributes` (`marque-ism`) — the pivot type. Every source format normalizes to this struct before rule validation. Fields use `Box<[T]>` (not `Vec`) to avoid over-allocation. Field types (`SciControl`, `DissemControl`, etc.) are generated enums from ODNI CVE XML.
- `Span` (`marque-ism`) — byte offset range into the original source buffer. Never copies content; spans reference the original `&[u8]`.
- `Diagnostic` (`marque-rules`) — a violation with `rule`, `severity`, `span`, `message`, `citation`, and optional `FixProposal`.
- `FixProposal` (`marque-rules`) — `span` + `replacement` + `confidence` + `source` + `migration_ref`. Pure data; no timestamp or classifier identity. Suggestions until promoted by `Engine::fix`.
- `AppliedFix` (`marque-rules`) — a promoted `FixProposal` with `timestamp`, `classifier_id`, `dry_run`, `input`. Constructed only by `Engine::fix`. Serves as the audit record.
- `RuleContext` (`marque-rules`) — position context passed to rules alongside attributes (`MarkingType`, `Zone`, `DocumentPosition`). Also carries an optional `Arc<PageContext>` for banner/CAB candidates so banner-validation rules can compare the observed banner against the composite expected from all preceding portions.
- `PageContext` (`marque-ism`) — page-level aggregation of portion markings: `max()` for classification, union for SCI/SAR/dissem controls, intersection (with NOFORN supersession) for `REL TO`, max-date for `declassify_on`. The engine builds this incrementally during `lint()` and hands banner/CAB rules an `Arc<PageContext>` via `RuleContext`.
- `Recognizer<S>` (trait in `marque-scheme`; impls in `marque-engine`) — pluggable first stage of the engine. Turns a byte slice + `ParseContext` into `Parsed<S::Marking>`. The trait lives in `marque_scheme::recognizer`; the two shipped concrete implementations are `marque_engine::StrictRecognizer` (zero-FP header-only, the existing structural parser) and `marque_engine::DecoderRecognizer` (Phase D probabilistic / bag-of-tokens, dispatched via `--deep-scan`). Trait is domain-neutral: depends only on the scheme's `Marking` and the `Parsed` / `Candidate` / `EvidenceFeature` primitives in `marque_scheme::ambiguity`.
- `Vocabulary<S>` (`marque-scheme`) — per-token metadata surface (authority, owner/producer, point of contact, deprecation, URN, schema version, portion/banner forms). Returns `&'static` data, zero runtime allocation (SC-008). Implemented for `CapcoScheme` from build-time-generated tables; rules read this instead of hardcoding metadata.
- `Codec<S>` (`marque-scheme`) — pinned trait surface for grammar serialization (encode/decode round-trip). No concrete impls in-tree; Phase G lands XML and JSON. `Codec::decode` returns `Parsed<S::Marking>` so ambiguity preserves through the codec layer (FR-019, SC-010).
- `Confidence` + `FeatureId` (`marque-rules`) — Phase D audit-provenance payload attached to every `FixProposal`. Carries `recognition` and `rule` confidence axes (combined as their product), optional `region` and `runner_up_ratio`, and a closed list of named `FeatureId` contributions. `f32` at the audit boundary (`f64` internally in the decoder). Adding a `FeatureId` variant requires a coordinated bump of `MARQUE_AUDIT_SCHEMA`.
- Topological scheduler (`marque_engine::scheduler`) — runs Kahn's algorithm over `PageRewrite::reads` / `writes` once at `Engine::new` to produce a deterministic rewrite order (writers before readers). Cycles fail with `EngineConstructionError::RewriteCycle`; `Custom` rewrites with empty axis annotations fail with `UnannotatedCustomAxes`. The cached order drives per-document evaluation without re-sorting.

### Architectural Invariants (do not bypass)

These contracts are enforced by convention and code review, not by the type system. A new crate or refactor that breaks one of them silently compromises the correctness or compliance guarantees of the tool.

- **`AppliedFix::__engine_promote` is engine-only in production code.** The constructor is `pub #[doc(hidden)]` because `marque-rules` is a dependency of `marque-engine` (not the other way around), so there is no way to seal it inside the engine crate at the visibility level. In **production code** (anything reachable from a `cfg(not(test))` build) it **must only be called from `Engine::fix_inner`**. Calling it from a rule crate, CLI binary, or downstream consumer bypasses the confidence-threshold gate, the FR-016 sort, and the C-1 overlap guard, and injects arbitrary entries into the audit log. The audit log is the compliance output — arbitrary injection is a data-integrity failure, not just a bug. If you are writing a crate that needs to produce fixes, produce `FixProposal` values and let `Engine` promote them. **Test-fixture carve-out**: `#[cfg(test)]` modules, `tests/` integration files, and `dev-dependencies`-gated test-utility crates MAY call `__engine_promote` to fabricate synthetic `AppliedFix` fixtures for unit-testing audit emitters, sentinel checks, and renderers — scoped per Constitution V Principle V (test-fixture construction only, never commingled with engine output, never `cfg(not(test))`-reachable). Each test call site should carry a comment naming the carve-out so a future reviewer doesn't have to re-derive the policy. See the doc comment on `AppliedFix::__engine_promote` for the full three-constraint definition.
- **`FixProposal` is pure data.** No timestamps, no classifier identity, no runtime context. Rule crates construct it; the engine snapshots runtime state into `AppliedFix` at promotion time. Keeping `FixProposal` pure is what lets tests snapshot rule output without a clock or user identity.
- **`RuleContext.zone` and `RuleContext.position` are `Option`-typed.** Phase 3 made both fields `Option<Zone>` and `Option<DocumentPosition>` and the engine populates them as `None` until a structural scanner pass can prove a value (header vs footer detection, document position from extracted-document metadata). Rules that read either field MUST handle `None`. The previous Phase-2 hardcoded `Body` was a silent lie — making the type carry the uncertainty makes it impossible to misuse.
- **`PageContext` resets at scanner-emitted page-break candidates.** Phase 3 added `MarkingType::PageBreak` to the scanner (form-feed `\f` and `\n\n\n+` heuristic). The engine resets its `PageContext` accumulator BEFORE attempting to parse the page-break candidate, so a corrupted or malformed candidate cannot block the reset. Banner/CAB rules on a new page see only that page's portions, not the whole document. Note: the scanner heuristic is conservative — `\n\n` (a normal paragraph break) does NOT trip the reset.
- **`Severity::Off` is a non-firing state, not a suppression.** A rule configured at `Off` is skipped in the rule loop, so no diagnostic is produced. This is the FR-008 invariant: an `Off`-severity diagnostic is unrepresentable.

### Batch Processing

`BatchEngine` wraps `Engine` behind `Arc` and uses `recoco-utils::ConcurrencyController` for row + byte semaphore backpressure. CPU-bound work goes to `tokio::task::spawn_blocking`. Results stream out in **completion order**, not submission order — correlate via the echoed `id`.

### Incremental Cache (planned for v0.2)

LMDB (`heed` crate) at `.marque/cache.lmdb`. Cache key = `blake3(content) ++ schema_version ++ config_hash`. Only `LintResult` is cached, never `FixResult`. Opt-in via `--cache` flag. Behind `cache` feature flag in `marque-engine`.

## Configuration

`.marque.toml` (committed, project/org policy):
```toml
[capco]
version = "2023.1"

[rules]
E001 = "fix"                   # portion-mark-in-banner; off | info | warn | error | fix
E002 = "fix"                   # missing-usa-trigraph

[corrections]
"SERCET" = "SECRET"
```

`.marque.local.toml` (gitignored, user identity — never committed):
```toml
[user]
classifier_id = "12345"
classification_authority = "EO 13526"
```

Precedence (highest wins): CLI flags → env vars → `.marque.local.toml` → `.marque.toml`

## CAPCO Schema Code Generation

`marque-ism/build.rs` reads ODNI ISM schema files from `crates/ism/schemas/ISM-v2022-DEC/` and generates code into `OUT_DIR/`, consumed via `include!()` in `crates/ism/src/generated.rs`. The schemas are present (ODNI package version `2022-DEC`, built June 2023).

**Actual schema layout** (the ODNI ZIP extracts to an `ISM/` root; subdirs were remapped on copy):
```
ZIP root: ISM/
  CVE/ISM/              → schemas/ISM-v2022-DEC/CVE_ISM/
  CVE/CveSchema/ISMCAT/ → schemas/ISM-v2022-DEC/CVE_ISMCAT/
  Schema/ISM/           → schemas/ISM-v2022-DEC/Schema/
  Schematron/ISM/       → schemas/ISM-v2022-DEC/Schematron/
```

`CVE_ISM/` contains one XML file per CVE enumeration (classification levels, SCI controls, dissem controls, SAR identifiers, etc.). `CVE_ISMCAT/` contains XSD/RNG/RNC for country trigraphs (RelTo/FGI). `Schema/` contains `IC-ISM.xsd`, `ISM.rng`, and generated XSDs. `Schematron/` contains `ISM_XML.sch` and `Lib/*.sch`.

Key files for `build.rs` to parse when implementing full code generation:
- `CVE_ISM/CVEnumISMClassificationAll.xml` — classification levels
- `CVE_ISM/CVEnumISMSCIControls.xml` — SCI controls
- `CVE_ISM/CVEnumISMDissem.xml` — dissemination controls (includes deprecation markers)
- `CVE_ISM/CVEnumISMSAR.xml` — SAR identifiers
- `CVE_ISM/CVEnumISMExemptFrom.xml` — declassification exemptions
- `CVE_ISMCAT/CVEGenerated/CVEnumISMCATRelTo.xsd` — country trigraphs for REL TO
- `Schema/IC-ISM.xsd` — attribute structure + deprecation annotations
- `Schematron/ISM_XML.sch` + `Schematron/Lib/*.sch` — validation predicates

`build.rs` currently emits placeholder generated files so the workspace compiles. Full CVE XML and Schematron parsing is the next implementation milestone.

The active schema version is pinned in `crates/ism/Cargo.toml` under `[package.metadata.marque] ism-schema-version`. Bump intentionally when ODNI publishes a new package.

## Adding a New Rule

1. Add a zero-size struct implementing `Rule` in `crates/capco/src/rules.rs`.
2. Register it in `CapcoRuleSet::new()`.
3. Rule IDs follow: `E###` = error, `W###` = warning, `C###` = correction.
4. Rules are stateless; all config-dependent behavior (severity overrides, classifier ID injection) is handled by the engine.
5. Fixes with `confidence < threshold` are surfaced as suggestions; those at or above are auto-applied by `Engine::fix`.

## REST API Surface

```
POST /v1/lint       → diagnostics
POST /v1/fix        → fixed text + audit log
GET  /v1/health
GET  /v1/schema/version
```

Planned (not yet wired in `marque-server`): `POST /v1/metadata`, `POST /v1/batch`, auth + structured logging middleware.

## Current Status

MVP complete. Full lint → fix → audit pipeline for raw text with 54 CAPCO rules (E001–E016, E020–E051, S001–S003, W002–W003, C001; W001 retired in T035c-14 per CAPCO-2016 §F). CLI (`check`, `fix`) and WASM (`lint`, `fix`) produce byte-identical NDJSON diagnostics (SC-008 parity). Configurable severity overrides, corrections map, and confidence thresholds. Batch processing via `BatchEngine` with concurrency control. Criterion benchmarks validate p95 ≤16ms on 10KB inputs (SC-001) and linear throughput scaling (SC-005). Corpus accuracy harness enforces ≥95% per-rule accuracy (SC-002/SC-003). `cargo-fuzz` target exercises `Engine::lint` on arbitrary `&[u8]`.

**Not yet built**: `marque-extract` (Kreuzberg integration for 75+ formats), `metadata` CLI subcommand, incremental LMDB cache (v0.2), server auth middleware.

## Active Technologies
- Rust 1.85+ (edition 2024) — `rust-version = "1.85"` in workspace `Cargo.toml`; constitution Tech Stack pins the floor
- `memchr` 2 — SIMD candidate detection (Phase 1 scanner)
- `aho-corasick` 1 — token matching (Phase 2 parser) + pre-scanner text corrections; used on both native and WASM. The constitution Tech Stack reserves `daachorse` for the WASM target as a future binary-size optimization, not yet wired
- `quick-xml` — build-time ODNI XSD/Schematron parsing
- `serde` + `serde_json` — build-time JSON codepath for per-term vocabulary data (runtime deserialization not required; data is emitted as `&'static` const tables by `build.rs`)
- `phf` — compile-time replacement lookup (perfect hash)
- `criterion` 0.5 — benchmarking (SC-001, SC-005)
- `libfuzzer-sys` 0.4 — fuzz target (requires nightly, not CI-gated)
- No new runtime crates introduced by Phase D's decoder — log-posterior scoring uses `f64` and Rust standard ops. Corpus-derived priors baked in as `&'static [T]` tables at build time.

**Build-time inputs**: `crates/ism/schemas/ISM-v2022-DEC/` (ODNI XML, vendored), `crates/capco/docs/CAPCO-2016.md` (authoritative manual, vendored), `crates/capco/corpus/` (corpus-derived priors produced by `tools/corpus-analysis/`, regenerated when the corpus changes). **Test inputs**: `tests/fixtures/mangled/` (≥200 labeled mangled cases generated from Enron-corpus high-confidence markings; generator checked in, artifact regenerable).

**Audit schema**: `MARQUE_AUDIT_SCHEMA` env var pinned at build time, validated against the closed accept-list `["marque-mvp-1", "marque-mvp-2"]`. Defaults to `"marque-mvp-2"` (Phase D, decoder + provenance). Re-exported as `marque_engine::AUDIT_SCHEMA_VERSION`. A single binary emits exactly one schema (FR-014).

## Recent Changes
- Phase 5 (vocabulary surface + trait-surface completion): build-time generation of per-token metadata tables (T080–T082); `impl Vocabulary<CapcoScheme> for CapcoScheme` (PR-2); FOUO regression guards confirming FOUO stays an active dissem control (FR-020, no `FOUO → CUI` migration entry); `Codec<S>` trait surface published with no concrete impls (T078, FR-019); `T089b` readiness stub exercising every Phase-E trait surface as if building a minimal second scheme (SC-010 deferred-verifiable check). Phase 5 PR-1 (#141) → PR-3 (#146).
- Phase 4 (probabilistic recognition + audit v2): compile-time corpus priors bake (PR-1, #111); `Box<dyn Recognizer<S>>` dispatch with `StrictRecognizer` as the default path (PR-2, #112); `DecoderRecognizer` + `--deep-scan` flag for probabilistic recovery (PR-3 #114, PR-4b #127); `MARQUE_AUDIT_SCHEMA` env-pinned at build time, `marque-mvp-2` audit records emit `Confidence` provenance (PR-4, #122); SC-002 deep-scan latency bench + SC-004 mangled-corpus accuracy gate at 0.85 threshold (PR-6, #135); corpus-override security gates (PR-5, #131); fuzzy CAPCO-token corrector (#96).
- Phase 9 (S003 + T035c-21 PR-B): S003 `joint-usa-first` style rule per §H.3/§H.8 + E039 (NODIS/EXDIS clears banner REL TO) + E040 (NODIS/EXDIS banner roll-up) + E041 (NODIS supersedes EXDIS in portion) per §H.9 p172–174. Rule count: 41 → 44.
- Phase B (recursive lattice & decoder plan, §12): built-in lattice constructors (`OrdMax`, `OrdMin`, `FlatSet`, `IntersectSet`, `SupersessionSet`, `ModeSet`, `MaxDate`, `OptionalSingleton`, `Product`); `Scope` / `DiffInput` / `CategoryShape` / `PageRewrite` trait-surface additions; `SciSet`/`SarSet`/`FgiSet` lattice types in `marque-capco` with §3.3a equal-depth meet policy; `CapcoScheme::project(Scope, ...)` taking over from `project_banner`; `capco/noforn-clears-rel-to` declared as the first `PageRewrite`; tetragraph expansion tables consolidated in `marque-capco::vocab`; `AggregationOp::Custom` retired from runtime dispatch (build-time shorthand only). Phase 3 of 004 (#69) added the topological page-rewrite scheduler with cycle and unannotated-axis detection.
- Phase 7: Criterion benchmarks (lint_latency, linear_scaling), corpus accuracy harness, WASM parity scaling to full corpus, cargo-fuzz target, bench-check regression gate
- Phase 6: WASM web worker build with SC-008 parity, `batch` feature flag, CachedAhoCorasick optimization
- Phase 5: Configurable severity overrides, corrections map with AhoCorasick pre-scanner
- SCI compartments (#003): structural subparser + `SciMarking` data model, E032–E035 rules, banner roll-up via `PageContext::expected_sci_markings()` (rule count 35 → 39)
- Phase 8: SAR implementation — structural `SarMarking` type (replaces empty `SarIdentifier` CVE enum), six new rules E026–E031 covering portion form, classification constraint, ordering, indicator-repeat coalescing, and banner roll-up per CAPCO-2016 §H.5
- Phase 3-4: Full lint/fix/audit pipeline, 29 CAPCO rules (E001–E025, W001–W003, C001), CLI with check/fix subcommands
- Phase 1-2: marque-ism crate extraction, test corpus scaffolding, benchmark stubs
