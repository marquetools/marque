<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/marquetools/marque/refs/heads/main/docs-site/src/assets/images/marque_logomark-dark.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/marquetools/marque/refs/heads/main/docs-site/src/assets/images/marque_logomark.svg">
  <img alt="marque logo" src="https://raw.githubusercontent.com/marquetools/marque/refs/heads/main/docs-site/src/assets/images/marque_logomark.svg" height="150px">
</picture>

[![codecov](https://codecov.io/gh/marquetools/marque/graph/badge.svg?token=7WQKZM1DA9)](https://codecov.io/gh/marquetools/marque)

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
| [`marque-capco`](./crates/capco/) | 44 hand-written CAPCO rules (E001–E016, E020–E041, S001–S003, W002–W003, C001) consuming `marque-ism` predicates. Includes SAR (Special Access Required) validation per §H.5, structural SCI compartment + sub-compartment support per §A.6, and NODIS/EXDIS mutual-exclusion + NOFORN-required + page-level roll-up + portion-supersession constraints per §H.9. |
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

### Is it Open Source?

That depends on who you ask. The Open Source Initiative (OSI) and Free Software Foundation (FSF) both would say **no**, because their definitions don't consider any provision that limits uses as open source or libre [^libre].

Marque *is* arguably open source under U.S. law[^open-source].

However you classify it, the Marque License is what is known as "fair code" or "source available." It tries to balance a few competing interests:

- **Source is right here in the open.** An engineer can read, understand, and test it. I think that's a fundamental consumer right — you deserve to know what's under the hood and how meticulous I am about ensuring a high-quality product. You should also be able to freely conduct your own security testing, weighing the trade-off decisions I made against your own specific situation. That's nearly impossible with most enterprise software.

- **You can freely use it for a lot of things.** You can deploy Marque on an internal server, run the demo for your office, and much more. I built this because I knew the problem was solvable — the existing solutions just reflect what happens when nobody with real domain expertise is building the tools.

- **You can't use Marque against me.** I put a lot of time, care, and consideration into every detail of Marque. It's built on deep experience with marking systems. I let people use it for a lot of things for free — but I need to live, and I deserve some benefit from it too. I also don't want to see vendors who couldn't build this — and didn't — use it to undercut the people actually solving problems their customers have been waiting years for them to fix.

---

[^libre]: The definitions are very similar, but OSI uses the term 'open source' while the FSF uses 'libre'.
[^open-source]: [Public Law 115-232](https://www.congress.gov/115/plaws/publ232/PLAW-115publ232.pdf) defines open source software as "software for which the human-readable source code is available for use, study, re-use, modification, enhancement, and re-distribution by the users of such software." Marque's source code is publicly available and meets this definition for internal use. Marque does not meet the [Open Source Definition](https://opensource.org/osd) maintained by the Open Source Initiative or the [Free Software Definition](https://www.gnu.org/philosophy/free-sw.html) maintained by the Free Software Foundation, because it restricts commercial redistribution.
