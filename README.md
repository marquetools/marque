<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/marquetools/marque/refs/heads/main/docs-site/src/assets/images/marque_logomark-dark.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/marquetools/marque/refs/heads/main/docs-site/src/assets/images/marque_logomark.svg">
  <img alt="marque logo" src="https://raw.githubusercontent.com/marquetools/marque/refs/heads/main/docs-site/src/assets/images/marque_logomark.svg" height="150px">
</picture>

## **A fast, rule-driven text linter, formatter, and transformer.** 

`marque` is a general-purpose rule engine for fast text processing — rules
produce warnings, errors, fixes, and transformations, each with a confidence
score the engine uses to decide what to auto-apply vs. surface as a
suggestion. Built in the style of [`ruff`](https://github.com/astral-sh/ruff):
designed for perceptual instantaneity at any scale, operating on raw byte
buffers with SIMD-accelerated scanning and an Aho-Corasick parser.

That's a fancy way of saying it's *extremely fast*.
It can scan and fix a page faster than you can perceive the change.

The MVP ships a CAPCO/ISM classification-marking rule set targeting ODNI
ISM-v2022-DEC (marque-ism and marque-capco crates). That's **one application** of the engine. 

The roadmap expands into other U.S. Government control markings
(CUI), foreign and multinational classification systems (e.g. NATO),
and general-purpose text lint/transformation domains.

The next big feature upgrade will bring metadata curation, correction, extraction, and removal.

## Design Philosophy

**marque must be**:

- **exceptionally fast**
- **hardened and secure by design**
- **correct**
- **completely auditable**
- **helpful**

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
[`marque-config`](./crates/config/README.md) for the full schema.

## Workspace

```text
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

Read `A ←── B` as "`B` depends on `A`". `marque-engine` is the sole
convergence point for the scanner chain (`marque-core`) and the rule chain
(`marque-capco`). `marque-scheme` is the domain-neutral trait surface.

| Crate | Role |
|---|---|
| [`marque`](./marque/) | CLI binary — `check`, `fix`, `metadata`. |
| [`marque-core`](./crates/core/) | Format-agnostic scanner + parser. SIMD via `memchr`, token matching via Aho-Corasick. WASM-safe. |
| [`marque-rules`](./crates/rules/) | Trait definitions only: `Rule`, `Diagnostic`, `FixProposal`, `AppliedFix`, `Severity`. |
| [`marque-engine`](./crates/engine/) | Pipeline orchestration. `Engine` (sync) + `BatchEngine` (async concurrent). Confidence gate + audit log live here. |
| [`marque-config`](./crates/config/) | Layered config loading (CLI > env > `.marque.local.toml` > `.marque.toml`) with hard-fail validators. |
| [`marque-ism`](./crates/ism/) | ISM vocabulary types + generated CVE enums from ODNI schemas. Build-time codegen, no runtime I/O. |
| [`marque-capco`](./crates/capco/) | 39 hand-written CAPCO rules (E001–E035, W001–W003, C001) consuming `marque-ism` predicates. Includes SAR (Special Access Required) validation per §H.5 and structural SCI compartment + sub-compartment support per §A.6. |
| [`marque-extract`](./crates/extract/) | Document text + metadata extraction. **Stub** — Kreuzberg integration pending. |
| [`marque-wasm`](./crates/wasm/) | WASM target via `wasm-pack`. Byte-identical NDJSON output to the CLI. |
| [`marque-server`](./crates/server/) | axum REST microservice wrapping `marque-engine`. |

**Domain scoping:** `marque-ism` and `marque-capco` are the only crates that
are ISM/CAPCO-specific. Everything else is general-purpose infrastructure
designed to host additional rule sets (CUI, NATO, KFC recipe) without
changes.

## Status

**MVP complete**. Full lint → fix → audit pipeline for raw text with 39 CAPCO
rules, including structural SAR validation per §H.5 and structural SCI
compartment + sub-compartment validation per §A.6. CLI and WASM produce
byte-identical NDJSON diagnostics. Configurable
severity overrides, corrections map, and confidence thresholds. Batch
processing via `BatchEngine` with concurrency control. Criterion benchmarks
validate p95 ≤16ms on 10KB inputs.

Not yet built: `marque-extract` (extraction from a wide range of media), the
`metadata` CLI subcommand, incremental LMDB cache (v0.2), server auth
middleware.

## License

This project is licensed under the [Marque License 1.0 (`LicenseRef-MarqueLicense-1.0`)](./LICENSE.md). Copyright 2026, Knitli Inc.

## Why... Classification Markings?

Because I lived them for 18.5 years, and I'm unusually encyclopedic about them.

I'm building what I wanted for 18.5 years everytime I used a poorly designed classification tool that slowed me down without helping me. I believe classifying a document correctly should take milliseconds, not 15+ minutes, and  `marque` is living proof. I can give that time back to the millions of people who have to classify or add control markings to documents every day.

### It should go without saying, but all classification markings are for testing and illustration purposes

Some folks need to hear that, it seems.
