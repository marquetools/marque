# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

`marque` is a linter, formatter, and auto-fixer for IC (Intelligence Community) classification markings — portion markings, banner markings, and Classification Authority Blocks (CABs) — in the style of `ruff`. It targets CAPCO/ODNI ISM specifications (currently ISM-v2022-DEC). The tool operates on raw text and 75+ document formats (via Kreuzberg), and is designed for perceptual instantaneity at any scale.

## Build Commands

```bash
# Build the workspace
cargo build

# Build CLI binary only
cargo build -p marque

# Build server only
cargo build -p marque-server

# Build WASM target (requires wasm-pack)
wasm-pack build crates/marque-wasm --target web --profile release-wasm

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
marque-ism  ←  marque-core  ←  marque-rules  ←  marque-capco
                                    ↓
                             marque-engine  ←  marque-config
                              ↑          ↑
                     marque-extract    marque-wasm
                              ↑
                       marque-server
                              ↑
                           marque (CLI)
```

### Crate Responsibilities

| Crate | Role |
|-------|------|
| `marque-ism` | ISM vocabulary types + generated CVE enums + `Span` + `IsmAttributes`. **WASM-safe** — build-time XML parsing only, no runtime I/O. Owns `build.rs` + ODNI schemas. |
| `marque-core` | Scanner + parser. **WASM-safe** — no I/O, no format deps, operates on `&[u8]`. Produces `IsmAttributes` from byte buffers. |
| `marque-rules` | Trait definitions only: `Rule`, `Diagnostic`, `FixProposal`, `Severity`, `AppliedFix`. No implementations. |
| `marque-capco` | CAPCO Layer 2 rule implementations. Consumes generated predicates from `marque-ism`. |
| `marque-engine` | Pipeline orchestration: `Engine` (single doc) and `BatchEngine` (async concurrent). |
| `marque-extract` | Kreuzberg wrapper for 75+ document formats + OCR + metadata extraction. **Not in WASM.** |
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

- **Layer 1 (generated)**: `marque-ism/build.rs` parses ODNI ISM XML schemas at build time → `OUT_DIR/{values,validators,migrations}.rs`, included via `marque-ism/src/generated.rs`. Outputs binary valid/invalid predicates only.
- **Layer 2 (hand-written)**: `Rule` implementations in `marque-capco/src/rules.rs` that consume Layer 1 predicates from `marque-ism`, classify *why* a violation occurred, determine fixes and confidence levels, and cite the CAPCO section.

### Key Types

- `IsmAttributes` (`marque-ism`) — the pivot type. Every source format normalizes to this struct before rule validation. Fields use `Box<[T]>` (not `Vec`) to avoid over-allocation. Field types (`SciControl`, `DissemControl`, etc.) are generated enums from ODNI CVE XML.
- `Span` (`marque-ism`) — byte offset range into the original source buffer. Never copies content; spans reference the original `&[u8]`.
- `Diagnostic` (`marque-rules`) — a violation with `rule`, `severity`, `span`, `message`, `citation`, and optional `FixProposal`.
- `FixProposal` (`marque-rules`) — `span` + `replacement` + `confidence` + `source` + `migration_ref`. Pure data; no timestamp or classifier identity. Suggestions until promoted by `Engine::fix`.
- `AppliedFix` (`marque-rules`) — a promoted `FixProposal` with `timestamp`, `classifier_id`, `dry_run`, `input`. Constructed only by `Engine::fix`. Serves as the audit record.
- `RuleContext` (`marque-rules`) — position context passed to rules alongside attributes (`MarkingType`, `Zone`, `DocumentPosition`).

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
banner-abbreviation = "fix"    # Severity: fix | warn | error | off
missing-usa-trigraph = "fix"

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

`marque-ism/build.rs` reads ODNI ISM schema files from `crates/marque-ism/schemas/ISM-v2022-DEC/` and generates code into `OUT_DIR/`, consumed via `include!()` in `crates/marque-ism/src/generated.rs`. The schemas are present (ODNI package version `2022-DEC`, built June 2023).

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

The active schema version is pinned in `crates/marque-ism/Cargo.toml` under `[package.metadata.marque] ism-schema-version`. Bump intentionally when ODNI publishes a new package.

## Adding a New Rule

1. Add a zero-size struct implementing `Rule` in `crates/marque-capco/src/rules.rs`.
2. Register it in `CapcoRuleSet::new()`.
3. Rule IDs follow: `E###` = error, `W###` = warning, `C###` = correction.
4. Rules are stateless; all config-dependent behavior (severity overrides, classifier ID injection) is handled by the engine.
5. Fixes with `confidence < threshold` are surfaced as suggestions; those at or above are auto-applied by `Engine::fix`.

## REST API Surface

```
POST /v1/lint       → diagnostics
POST /v1/fix        → fixed text + audit log
POST /v1/metadata   → metadata report
POST /v1/batch      → batch results
GET  /v1/health
GET  /v1/schema/version
```

## Current Status

Pre-MVP. Core pipeline (scanner → parser → engine → rules) is functional end-to-end for raw text. `marque-extract` (Kreuzberg integration) is stubbed. `build.rs` emits placeholder generated code — actual ODNI schema parsing is not yet implemented. The incremental batch cache and server auth middleware are planned but not built.

## Active Technologies
- Rust 1.85+ (edition 2024) — pinned by constitution Tech Stack + `memchr` (Phase 1 scanner), `aho-corasick` (native Phase 2) (001-marque-mvp)
- None for the MVP. The LMDB incremental cache is explicitly out of (001-marque-mvp)

## Recent Changes
- 001-marque-mvp: Added Rust 1.85+ (edition 2024) — pinned by constitution Tech Stack + `memchr` (Phase 1 scanner), `aho-corasick` (native Phase 2)
