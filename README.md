# marque

**A fast, rule-driven text linter, formatter, and transformer.** Ships with CAPCO/ISM classification-marking rules.

`marque` is a general-purpose rule engine for fast text processing — rules
produce warnings, errors, fixes, and transformations, each with a confidence
score the engine uses to decide what to auto-apply vs. surface as a
suggestion. Built in the style of [`ruff`](https://github.com/astral-sh/ruff):
designed for perceptual instantaneity at any scale, operating on raw byte
buffers with SIMD-accelerated scanning and an Aho-Corasick parser.

The MVP ships a CAPCO/ISM classification-marking rule set targeting ODNI
ISM-v2022-DEC — but that's **one application** of the engine, not its
identity. The roadmap expands into other U.S. Government control markings
(CUI), foreign and multinational classification systems (NATO, FGI, JOINT),
and general-purpose text lint/transformation domains.

## Demo

_Coming soon._

## Install

```sh
cargo install marque
```

Or build from source:

```sh
git clone https://github.com/marquetools/marque
cd marque
cargo build --release -p marque
```

## Quick start

```sh
# Lint a file (or stdin)
marque check report.txt
echo "(SERCET) draft content" | marque check -

# Preview fixes without writing
marque fix --dry-run report.txt

# Apply fixes in place; audit records stream to stderr as NDJSON
marque fix report.txt
```

Configuration lives in `.marque.toml` (project, committed) and
`.marque.local.toml` (per-user identity, gitignored). See the
[CLI README](./marque/README.md) for flags and exit codes, and
[`marque-config`](./crates/marque-config/README.md) for the full schema.

## Workspace

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

| Crate | Role |
|---|---|
| [`marque`](./marque/) | CLI binary — `check`, `fix`, `metadata`. |
| [`marque-core`](./crates/marque-core/) | Format-agnostic scanner + parser. SIMD via `memchr`, token matching via Aho-Corasick. WASM-safe. |
| [`marque-rules`](./crates/marque-rules/) | Trait definitions only: `Rule`, `Diagnostic`, `FixProposal`, `AppliedFix`, `Severity`. |
| [`marque-engine`](./crates/marque-engine/) | Pipeline orchestration. `Engine` (sync) + `BatchEngine` (async concurrent). Confidence gate + audit log live here. |
| [`marque-config`](./crates/marque-config/) | Layered config loading (CLI > env > `.marque.local.toml` > `.marque.toml`) with hard-fail validators. |
| [`marque-ism`](./crates/marque-ism/) | ISM vocabulary types + generated CVE enums from ODNI schemas. Build-time codegen, no runtime I/O. |
| [`marque-capco`](./crates/marque-capco/) | 29 hand-written CAPCO rules (E001–E025, W001–W003, C001) consuming `marque-ism` predicates. |
| [`marque-extract`](./crates/marque-extract/) | Document text + metadata extraction. **Stub** — Kreuzberg integration pending. |
| [`marque-wasm`](./crates/marque-wasm/) | WASM target via `wasm-pack`. Byte-identical NDJSON output to the CLI. |
| [`marque-server`](./crates/marque-server/) | axum REST microservice wrapping `marque-engine`. |

**Domain scoping:** `marque-ism` and `marque-capco` are the only crates that
are ISM/CAPCO-specific. Everything else is general-purpose infrastructure and
is designed to host additional rule sets (CUI, NATO, FGI, JOINT, …) without
changes.

## Status

MVP complete. Full lint → fix → audit pipeline for raw text with 29 CAPCO
rules. CLI and WASM produce byte-identical NDJSON diagnostics. Configurable
severity overrides, corrections map, and confidence thresholds. Batch
processing via `BatchEngine` with concurrency control. Criterion benchmarks
validate p95 ≤16ms on 10KB inputs.

Not yet built: `marque-extract` (Kreuzberg integration for 75+ formats), the
`metadata` CLI subcommand, incremental LMDB cache (v0.2), server auth
middleware.

## For maintainers

See [`CLAUDE.md`](./CLAUDE.md) at the workspace root for architecture, the
processing pipeline, and the architectural invariants that govern the
engine/rules contract. AI-agent maintainers are primary contributors;
framing and invariants there are load-bearing.

## License

Apache-2.0.
