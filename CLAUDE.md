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
wasm-pack build crates/wasm --target web --profile release-web

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
| `marque-ism` | ISM vocabulary types + generated CVE enums + `Span` + `IsmAttributes`. **WASM-safe** — build-time XML parsing only, no runtime I/O. `build.rs` consumes ODNI schemas via the `ism` and `ism-ismcat` build-dependencies from [`marquetools/ism-data`](https://github.com/marquetools/ism-data). |
| `marque-core` | Scanner + parser. **WASM-safe** — no I/O, no format deps, operates on `&[u8]`. Produces `IsmAttributes` from byte buffers. |
| `marque-rules` | Trait definitions only: `Rule`, `Diagnostic`, `FixProposal`, `Severity`, `AppliedFix`. No implementations. |
| `marque-scheme` | Domain-neutral trait surface for structured marking schemes. Defines `MarkingScheme`, `Lattice`, `BoundedLattice`, `Category`/`AggregationOp`/`CategoryShape`, `Constraint`, `Parsed<M>`, `Scope`, `PageRewrite`, and built-in lattice constructors (`OrdMax`, `OrdMin`, `FlatSet`, `IntersectSet`, `SupersessionSet`, `ModeSet`, `MaxDate`, `OptionalSingleton`, `Product`). One permitted runtime dep: `smallvec` (inline-2 buffer for `ReplacementIntent::FactRemove::facts`; keeps single-fact removals heap-free per #348). No dependency on `marque-ism`. Phase B landed the recursive-lattice surface — see `docs/plans/2026-04-19-recursive-lattice-and-decoder.md`. |
| `marque-capco` | CAPCO Layer 2 rule implementations. Consumes generated predicates from `marque-ism`. Also hosts `CapcoScheme`, the `marque-scheme` adapter over `IsmAttributes`; `SciSet`/`SarSet`/`FgiSet` lattice types (`marque_capco::lattice`); and tetragraph expansion tables (`marque_capco::vocab`). |
| `marque-engine` | Pipeline orchestration: `Engine` (single doc) and `BatchEngine` (async concurrent). |
| `marque-extract` | Kreuzberg wrapper for 75+ document formats + OCR + metadata extraction. Alternately a narrowing custom or pieced together use of other libraries (Kreuzberg has some licensing complication) **Not in WASM.** |
| `marque-config` | Layered config loading from `.marque.toml` → `.marque.local.toml` → env vars. |
| `marque-wasm` | `wasm-pack` target. Exposes `lint`/`fix` to web workers. Format extraction is caller's responsibility. |
| `marque-server` | axum REST microservice wrapping `marque-engine`. Auth/logging via Tower middleware. |
| `marque` | Thin CLI binary. Subcommands: `check`, `fix`, `metadata`. |

`crates/cui/` is a placeholder for a future CUI rule crate — currently holds the vendored 2019 NARA CUI Marking Handbook (`docs/`) and `REUSE.toml` only, **not** a workspace member, no `Cargo.toml`, no source. When the CUI crate lands it MUST follow the `build.rs` → generated-predicates pattern established by `marque-ism` (Principle IV) and remain WASM-safe (Principle III).

### Processing Pipeline

```
Source → [marque-extract] → TextStream → [Scanner] → SpanStream
       → [Parser] → AttributeStream → [Rules] → DiagnosticStream → Output
```

- **Phase 1 (Scanner)**: `memchr`-based SIMD candidate detection — finds portions `(...)`, banners, and CABs with zero heap allocation.
- **Phase 2 (Parser)**: Aho-Corasick automaton over CVE token list extracts `IsmAttributes` from each candidate span.
- **Phase 3 (Engine)**: Each `Rule` receives `(&IsmAttributes, &RuleContext)` and returns `Vec<Diagnostic>`. Fixes carry a confidence score (0.0–1.0); the engine applies those at or above the threshold in reverse span order.

### Two-Layer Rule Architecture

- **Layer 1 (generated)**: `marque-ism/build.rs` parses ODNI ISM XML schemas (consumed via the `ism` and `ism-ismcat` build-dependencies from [`marquetools/ism-data`](https://github.com/marquetools/ism-data); schemas are no longer vendored locally) at build time → `OUT_DIR/{values,validators,migrations}.rs`, included via `marque-ism/src/generated.rs`. Outputs binary valid/invalid predicates only. Phase 5 added vocabulary metadata generation from the ODNI JSON sidecar — authority, owner/producer, deprecation, URN, schema version, and portion/banner forms — exposed through `Vocabulary<S>` (see Key Types).
- **Layer 2 (hand-written and declarative)**: `Rule` implementations in `crates/capco/src/rules.rs` consume Layer 1 predicates from `marque-ism`, classify *why* a violation occurred, determine fixes and confidence levels, and cite the CAPCO section. Phase 4+ added a *declarative* second form: dyadic invariants (conflict, requires, implies, supersedes) and page-level rewrites are declared as `Constraint` / `PageRewrite` data on `CapcoScheme` (see `crates/capco/src/scheme.rs`) rather than as procedural rule bodies. The shared evaluator in `marque-scheme` runs them; the engine's topological scheduler (`marque-engine::scheduler`) orders rewrites by their `reads` / `writes` axes and rejects cycles or unannotated `Custom` axes at `Engine::new`. See `crates/capco/README.md` for the worked example.

### SCI Compartments (Hybrid CVE + Structural)

SCI markings need more than a flat CVE enum because CAPCO-2016 §A.6 defines a compositional grammar: `CONTROL-COMP (SPACE SUB-COMP)*(-COMP (SPACE SUB-COMP)*)*` (e.g. `SI-G ABCD DEFG-MMM AACD` where `SI` is the control, `G` and `MMM` are compartments, `ABCD`/`DEFG` are sub-compartments of `G`, and `AACD` is a sub-compartment of `MMM`). Pure CVE lookup cannot round-trip this — the vocabulary only lists pre-registered compounds (`SI-G`, `HCS-P`, etc.), not the open-ended compartment/sub-compartment tail.

The hybrid approach: the CVE vocabulary generated from `CVEnumISMSCIControls.xml` gives bare-system recognition and the set of pre-registered compounds; a structural subparser (`parse_sci_block` in `marque-core/src/parser.rs`) handles the full §A.6 grammar and emits `SciMarking` entries. The subparser is dispatched before the CVE exact-match path and gated on `contains('-') || contains('/') || is_bare_cve_value || (custom-control shape ∧ ¬ known non-SCI token)` so plain two-letter tokens (NF, RD) still fall through to the dissem/non-IC/SAR/AEA chain, while standalone custom controls like `99` (e.g., `TOP SECRET//99//NOFORN`) reach the structural path.

`IsmAttributes` exposes both `sci_markings: Box<[SciMarking]>` (authoritative structural form — control system + compartments + sub-compartments) and the original `sci_controls: Box<[SciControl]>` (CVE enum projection) for back-compat with existing consumers. `canonical_enum` on a `SciMarking` is populated only when the bare control or `{ctrl}-{first_comp}` matches a CVE value AND no sub-compartments are present; anything richer is structural-only.

**Phase B canonicalization.** Post-Phase-B, `SciSet` (in `marque_capco::lattice`, the lattice form of SCI state) is the canonical page-context storage: it implements `Lattice`, round-trips with `[SciMarking]` via `SciSet::from_markings` / `SciSet::to_markings`, and composes through `CapcoScheme::project(Scope::Page, ...)`. `SciSet` (and `SarSet`) deliberately do **not** implement `BoundedLattice` — SCI control systems and SAR program identifiers are both agency-extensible open sets, so no lawful finite `top` exists. Use `SciSet::empty()` / `SciSet::default()` when you need the lattice bottom. `IsmAttributes::sci_controls` stays populated for rules that currently read it, but is a compatibility view scheduled for removal in Phase C or D when no rule references it. New rules that need compartment / sub-compartment semantics should read `sci_markings` or construct an `SciSet`; rules that just need "which bare control systems appear" can stay on `sci_controls` until the migration closes.

Banner roll-up for SCI (E035) uses `PageContext::expected_sci_markings()`, which unions compartments and sub-compartments across all portions on the page and sorts per §A.6 p15 (numeric first, alpha after). Authority: CAPCO-2016 §A.6 (grammar, canonical example p16) + §H.4 (per-system banner precedence).

**NATO SAPs (PR 9c.1 T134).** `SciControlSystem::NatoSap(NatoSap)` is the canonical home for `BOHEMIA` and `BALK` (CAPCO-2016 §G.2 p41 + §H.7 p127). They render standalone (no `SAR-` prefix) in the SCI block position — e.g. `(//CTS//BOHEMIA)` or `(//CTS//BALK/BOHEMIA)`. BALK sorts before BOHEMIA alphabetically per §H.7 p127 worked example. NATO SAPs are CAPCO-only (no ODNI ISM CVE entry) — the third `SciControlSystem` variant keeps `Published(SciControlBare)` ODNI-faithful and `Custom(SmolStr)` reserved for agency-allocated `[A-Z0-9]{2,5}` identifiers per §A.6 p15. Legacy `CTS-B` / `CTS-BALK` text and the banner-form equivalents canonicalize through the strict parser into bare CTS class + SCI NatoSap companion; rule E066 emits a Recanonicalize FixIntent so the source text is re-rendered to the canonical multi-block form.

### SAR (Special Access Required)

SAR (Special Access Required) markings are modeled structurally, not as a CVE-derived enum. The ODNI public `CVEnumISMSAR.xml` is empty because SAR program identifiers are agency-assigned codewords not centrally registered. `marque-ism::SarMarking` captures the full hierarchy — programs, compartments, sub-compartments — parsed by a hand-written subparser in `marque-core` (see `parse_sar_category`). The six SAR rules (E026–E031) validate syntax, ordering, classification constraints, and banner roll-up per CAPCO-2016 §H.5.

### ATOMAL (NATO AEA)

ATOMAL is a NATO AEA marking — Atomic Energy Act information shared with NATO+UK under bilateral §123/§144 sharing agreements. Per CAPCO-2016 §G.2 p41 (Table 5: ARH by Registered Marking) ATOMAL is a registered standalone control marking; the §H.7 p123 worked example (`SECRET//RD/ATOMAL//FGI NATO//NOFORN`) places ATOMAL in the AEA axis alongside RD/FRD/TFNI — **not** as a NATO classification portion-suffix.

PR 9c.1 T134 introduced `AeaMarking::Atomal(AtomalBlock)` as the canonical home. The block is empty (no registered sub-markings) but mirrors `RdBlock`/`FrdBlock` so a future CAPCO grammar extension is a planned migration. The strict parser canonicalizes legacy compound text (`CTSA`, `CTS-A`, `NSAT`, `NS-A`, `NCA`, `NC-A`, banner-form `COSMIC TOP SECRET ATOMAL`, etc.) into bare NATO class + AEA ATOMAL companion at parse time; rule E066 emits a Recanonicalize FixIntent that re-renders to the canonical multi-block form (`(//CTS//ATOMAL)`, etc.) per the §G.2 p41 Table 5 registration. Per project memory `remark-on-derivative-use-is-marque-autofix`, Marque automates the canonical re-marking the manual permits doing by hand. The legacy fused `NatoClassification::*Atomal` variants (`NatoConfidentialAtomal`, `NatoSecretAtomal`, `CosmicTopSecretAtomal`) and the corresponding `*Bohemia` / `*Balk` variants were retired in PR 9c.1 Commit 5.

### Key Types

- `IsmAttributes` (`marque-ism`) — the pivot type. Every source format normalizes to this struct before rule validation. Fields use `Box<[T]>` (not `Vec`) to avoid over-allocation. Field types (`SciControl`, `DissemControl`, etc.) are generated enums from ODNI CVE XML.
- `Span` (`marque-ism`) — byte offset range into the original source buffer. Never copies content; spans reference the original `&[u8]`.
- `Diagnostic` (`marque-rules`) — a violation with `rule`, `severity`, `span`, `message`, `citation`, and optional `FixProposal`.
- `FixProposal` (`marque-rules`) — `span` + `replacement` + `confidence` + `source` + `migration_ref`. Pure data; no timestamp or classifier identity. Suggestions until promoted by `Engine::fix`.
- `AppliedFix` (`marque-rules`) — a promoted `FixProposal` with `timestamp`, `classifier_id`, `dry_run`, `input`. Constructed only by `Engine::fix`. Serves as the audit record.
- `RuleContext` (`marque-rules`) — position context passed to rules alongside attributes (`MarkingType`, `Zone`, `DocumentPosition`). Also carries an optional `Arc<PageContext>` for banner/CAB candidates so banner-validation rules can compare the observed banner against the composite expected from all preceding portions.
- `PageContext` (`marque-ism`) — page-level aggregation of portion markings: `max()` for classification, union for SCI/SAR/dissem controls, intersection (with NOFORN supersession) for `REL TO`, max-date for `declassify_on`. The engine builds this incrementally during `lint()` and hands banner/CAB rules an `Arc<PageContext>` via `RuleContext`.
- `Recognizer<S>` (trait in `marque-scheme`; impls in `marque-engine`) — pluggable first stage of the engine. Turns a byte slice + `ParseContext` into `Parsed<S::Marking>`. The trait lives in `marque_scheme::recognizer`; the three shipped concrete implementations are `marque_engine::StrictRecognizer` (zero-FP header-only, the existing structural parser), `marque_engine::DecoderRecognizer` (Phase D probabilistic / bag-of-tokens), and `marque_engine::StrictOrDecoderRecognizer` (the strict-first / decoder-fallback dispatcher installed by default in `Engine::new`). Callers that need strict-only dispatch (the SC-001 interactive-latency benchmark, tests asserting strict behavior) install `StrictRecognizer` explicitly via `Engine::with_recognizer`. Trait is domain-neutral: depends only on the scheme's `Marking` and the `Parsed` / `Candidate` / `EvidenceFeature` primitives in `marque_scheme::ambiguity`.
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

`marque-ism/build.rs` reads ODNI ISM schema files via two `[build-dependencies]` from the [`marquetools/ism-data`](https://github.com/marquetools/ism-data) workspace and generates code into `OUT_DIR/`, consumed via `include!()` in `crates/ism/src/generated.rs`. Schemas are not vendored in this repo.

| Build-dep | Provides | `package_root()` |
|-----------|----------|------------------|
| [`ism`](https://crates.io/crates/ism) | The ODNI ISM-Public-Standalone.zip tree (CVE_ISM XML/JSON, IC-ISM.xsd, Schematron rules) | `data/ISM/` |
| [`ism-ismcat`](https://crates.io/crates/ism-ismcat) | Standalone ISMCAT package (Tetragraph Taxonomy, RelTo trigraph CVE) | `data/ISMCAT/` |

Both crates carry a SHA-256 manifest of every file under `data/` and re-hash on every consumer compile (gated by their default `verify-on-build` feature). A single tampered byte in either crate refuses the build.

Key files marque-ism currently consumes:

- `ism::package_root().join("CVE/ISM/CVEnumISMClassificationAll.xml")` — classification levels
- `ism::package_root().join("CVE/ISM/CVEnumISMSCIControls.xml")` — SCI controls
- `ism::package_root().join("CVE/ISM/CVEnumISMDissem.xml")` — dissemination controls (includes deprecation markers)
- `ism::package_root().join("CVE/ISM/CVEnumISMSAR.xml")` — SAR identifiers (intentionally empty in public ODNI packages)
- `ism::package_root().join("CVE/ISM/CVEnumISMExemptFrom.xml")` — declassification exemptions
- `ism::package_root().join("CVE/ISM/CVEnum*.json")` — JSON sidecars for per-token vocabulary metadata
- `ism::package_root().join("Schematron/ISM/ISM_XML.sch")` — Schematron rules
- `ism_ismcat::package_root().join("Schema/ISMCAT/CVEGenerated/CVEnumISMCATRelTo.xsd")` — country trigraphs
- `ism_ismcat::package_root().join("Taxonomy/ISMCAT/TetragraphTaxonomyDenormalized.xml")` — tetragraph membership (V2022-NOV)

Three independent version pins live in `crates/ism/Cargo.toml` under `[package.metadata.marque]`, all cross-checked at build time:

| Pin | Meaning |
|-----|---------|
| `ism-schema-version` | Upstream ODNI ISM package label (e.g. `ISM-v2022-DEC`) — what ODNI calls the publication |
| `ism-data-version` | Snapshot version of the `ism-data` workspace this build uses (`YYYYMMDD.MAJOR.PATCH`, e.g. `20230609.0.0`) |
| `ismcat-tetra-version` | ISMCAT Tetragraph Taxonomy revision (e.g. `2022-NOV`, independent of the ISM bundle) |

Bump intentionally when ODNI publishes updates AND the `ism-data` workspace is re-vendored to that snapshot. The corresponding `[build-dependencies]` versions in `crates/ism/Cargo.toml` and the workspace `Cargo.toml` must move in lock-step.

A monthly canary in [`marquetools/ism-data`](https://github.com/marquetools/ism-data/.github/workflows/) HEAD-checks ODNI's published ZIP URLs against the snapshot baseline; marque doesn't run its own canary anymore.

## Adding a New Rule

1. Add a zero-size struct implementing `Rule` in `crates/capco/src/rules.rs`.
2. Register it in `CapcoRuleSet::new()`.
3. Rule IDs follow: `E###` = error, `W###` = warning, `C###` = correction.
4. Rules are stateless; all config-dependent behavior (severity overrides, classifier ID injection) is handled by the engine.
5. Fixes with `confidence < threshold` are surfaced as suggestions; those at or above are auto-applied by `Engine::fix`.
6. Cite the authoritative section in the rule (e.g., `CAPCO-2016 §H.5 p99`) and verify the citation against the primary source — `crates/capco/docs/CAPCO-2016.md` — before opening the PR. **Constitution Principle VIII (Authoritative Source Fidelity)** treats a fabricated, hallucinated, misattributed, or silently-drifted citation as a correctness defect of the same severity as a wrong predicate. A citation that cannot be traced to a real passage MUST be removed, not left in place pending follow-up.

## REST API Surface

```
POST /v1/lint       → diagnostics
POST /v1/fix        → fixed text + audit log
GET  /v1/health
GET  /v1/schema/version
```

Planned (not yet wired in `marque-server`): `POST /v1/metadata`, `POST /v1/batch`, auth + structured logging middleware.

## Current Status

MVP complete. Full lint → fix → audit pipeline for raw text with **36 registered CAPCO rules** post-PR-9a (31 post-PR-3c.B + 5 PR 9a additions E061–E065; see `crates/capco/README.md` for the authoritative rule inventory). Rule-collapse history: W001 retired in T035c-14 per CAPCO-2016 §F; E052 added in issue #234 PR-B per §H.8; S004 trigraph-suggest added in PR-C of #186 per §H.8 (first consumer of the suggest-don't-fix channel); PR 3b.A collapsed three banner roll-up rules (E031/E035/E040) into the `BannerMatchesProjectedRule` walker (top-level `Rule::id() = E031`, still emitting all three per-row IDs); PR 3b.D collapsed class-floor rules (E022/E025/E027) into the `DeclarativeClassFloorRule` walker (E058); PR 3b.E collapsed the SCI per-system rules (E042–E051) into the `DeclarativeSciPerSystemRule` walker (E059); PR 3b.F collapsed the ordering rules (E020/E023/E028/E033) into the `DeclarativeNonCanonicalInputRule` walker (E060). CLI (`check`, `fix`) and WASM (`lint`, `fix`) produce byte-identical NDJSON diagnostics (SC-008 parity). Configurable severity overrides, corrections map, and confidence thresholds. Batch processing via `BatchEngine` with concurrency control. Criterion benchmarks validate p95 ≤16ms on 10KB inputs (SC-001) and linear throughput scaling (SC-005). Corpus accuracy harness enforces ≥95% per-rule accuracy (SC-002/SC-003). `cargo-fuzz` target exercises `Engine::lint` on arbitrary `&[u8]`.

**Not yet built**: `marque-extract` is scaffolded (workspace member with `Extractor`, `ExtractedDocument`, `ExtractionOptions`, `MetadataReport` surface) but the Kreuzberg backend is stubbed — `crates/extract/src/extractor.rs` reads raw text only and `crates/extract/Cargo.toml` keeps `kreuzberg` commented out pending a licensing decision. Also outstanding: `metadata` CLI subcommand, incremental LMDB cache (v0.2), server auth middleware.

## Active Technologies
- Rust 1.85+ (edition 2024) — `rust-version = "1.85"` in workspace `Cargo.toml`; constitution Tech Stack pins the floor
- `memchr` 2 — SIMD candidate detection (Phase 1 scanner)
- `aho-corasick` 1 — token matching (Phase 2 parser) + pre-scanner text corrections; used on both native and WASM. The constitution Tech Stack reserves `daachorse` for the WASM target as a future binary-size optimization, not yet wired
- `quick-xml` — build-time ODNI XSD/Schematron parsing
- `serde` + `serde_json` — build-time JSON codepath for per-term vocabulary data (runtime deserialization not required; data is emitted as `&'static` const tables by `build.rs`)
- `phf` — compile-time replacement lookup (perfect hash)
- `criterion` 0.8 — benchmarking (SC-001, SC-005)
- `libfuzzer-sys` 0.4 — fuzz target (requires nightly, not CI-gated)
- No new runtime crates introduced by Phase D's decoder — log-posterior scoring uses `f64` and Rust standard ops. Corpus-derived priors baked in as `&'static [T]` tables at build time.
- Rust 1.85+ (edition 2024); workspace `rust-version = "1.85"` floor pinned in workspace `Cargo.toml` per Constitution Technology Stack. + `tokio` (async runtime, `BatchEngine`), `axum` + `tower` (server middleware), `memchr` 2 (Phase 1 SIMD scanner), `aho-corasick` 1 (Phase 2 token matching, native + WASM), `quick-xml` (build-time ODNI XSD/Schematron), `serde` + `serde_json` (build-time JSON sidecar), `phf` (compile-time replacement lookup), `criterion` 0.8 (benches), `static_assertions` (compile-time `Send + Sync` checks — FR-038), `blake3` (audit-record digests — FR-002/FR-004), `heed` (LMDB, planned v0.2 cache; not in scope here), `wasm-pack` (WASM target). (006-engine-rule-refactor)
- N/A on the hot path. Build-time cache via Cargo `OUT_DIR`. The planned LMDB `LintResult` cache is out of scope for this refactor. (006-engine-rule-refactor)

**Build-time inputs**: ODNI XML pulled from the `ism` and `ism-ismcat` build-deps (vendored in [`marquetools/ism-data`](https://github.com/marquetools/ism-data) at snapshot `20230609.0.0`, package label `ISM-v2022-DEC`); `crates/capco/docs/CAPCO-2016.md` (authoritative manual, vendored); `crates/capco/corpus/` (corpus-derived priors produced by `tools/corpus-analysis/`, regenerated when the corpus changes). **Test inputs**: `tests/fixtures/mangled/` (≥200 labeled mangled cases generated from Enron-corpus high-confidence markings; generator checked in, artifact regenerable).

**Audit schema**: `MARQUE_AUDIT_SCHEMA` env var pinned at build time, validated against the closed accept-list `["marque-mvp-3"]`. Defaults to `"marque-mvp-3"` (PR 3c.B Commit 10 — `FixProposal` retired; audit envelope carries a structural `proposal: FixIntent | TextCorrection` sub-object instead of top-level `original`/`replacement` byte fields, closing the G13 audit-content-ignorance channel). Re-exported as `marque_engine::AUDIT_SCHEMA_VERSION`. A single binary emits exactly one schema (FR-014).

## Recent Changes
- PR 9a (006 T131 + T135a, 2026-05-14): within-category Separator span emission + token canonicalization rules. T131 (Commit 1, #106): parser emits `TokenKind::Separator` spans for within-category `/` byte sequences (disambiguated from between-category `//` by `text` field — `"/"` vs `"//"`), with an engineering relaxation that consumes adjacent ASCII whitespace into the Separator span (Marque tolerance, NOT a §A.6-permitted variant — §A.6 p16 forbids interjected whitespace for SAP/AEA/dissem/non-IC dissem alike). SAR keeps a strict 1-byte separator span. T135a Commits 2–5: SCI long-form deprecated-token recognizer (Commit 2) + `DeprecatedSciLongFormRule` walker (Commit 3, E065) + bare HCS at C / bare HCS at S/TS suggest / bare RSV rules (Commit 4, E061 + E062 + E063 per §H.4 p62 + p70) + EYES / EYES ONLY → REL TO conversion (Commit 5, E064 per §H.8 p157-158, text_correction at compound block span, cross-axis migration is text_correction-route, not FixIntent — `ReplacementIntent::FactAdd`/`FactRemove`/`Recanonicalize` are single-axis-scoped). Net rule count change: 31 → 36 (5 added; no retirements in PR 9a — Stage 4 walker collapses land in subsequent PRs).
- `FeatureId::PrecedingFixPenalty` retired (PM decision, 2026-05-14): the mechanism was misunderstanding-derived (the user's original concern was a decoder-specific confidence-loop pathology, not a generalized cross-pass penalty) and the path was independently confirmed dead code today (`pass1.applied` is always empty under current `Phase::Localized` rules, which all emit via `Diagnostic::text_correction`). The variant, engine-applied multiplicative `rule` reduction, `FeatureContribution` audit-trace entry, `PRECEDING_FIX_PENALTY_DELTA` constant, and watchdog test suite are removed in PR 7c. `RuleContext<'a>` + `pre_pass_1_attrs` field + pre-pass-1 attrs cache + FR-023 disambiguation + I-18 overlap demotion stay (load-bearing for the two-pass model, independent of the penalty). The decoder confidence-loop concern remains an open research item, deferred to a future statistical design pass (see D-7.22).
- PR 3c.2 carved out + `marque-1.0` deferral (PM decision, 2026-05-14): the `marque-mvp-3 → marque-1.0` audit-schema cutover originally bundled into PR 3c (per the original FR-035) deferred to a dedicated **PR 3c.2** that lands the four structural commitments atomically (Canonical wired into audit emit, BLAKE3 audit-record digesting, closed `MessageTemplate` JSON serialization, `from_parsed_unchecked` adapter deletion). A 2026-05-14 inventory across four parallel Explore agents confirmed all four commitments are fully reserved slots with no production wire-up: `blake3` not in any Cargo.toml; `AppliedFix` has no digest field; `AuditRecordJsonV3` emits no `message` field; 27 surviving `from_parsed_unchecked` call sites. PR 7 series stays on `marque-mvp-3` (originally because PR 7c's planned `FeatureId::PrecedingFixPenalty` variant would have filled a reserved slot; that mechanism was retired 2026-05-14 per D-7.22, so PR 7c neither fills nor needs the slot — but the PR-7-stays-on-mvp-3 stance survives). The `(scheme, predicate-id)` 2-tuple `RuleId` form is **NOT** part of PR 3c.2's scope — it defers further still, to its own post-PR-10 PR per FR-049 (stability freeze begins at PR 10 merge). Plan-of-record amendments: spec FR-035 (revised) + new FR-035a; consolidated plan §4 table (new PR 3c.2 row, amended PR 7 row, two-stage audit-schema cutover table) + §10.2 (revised cutover composition); `contracts/audit-record.md` §0 + §1 redirected to PR 3c.2; PM decisions D-7.18 (defer) / D-7.19 (engine-applied `PrecedingFixPenalty`, not E003-applied — E003 was retired in PR 3b.F → E060) / D-7.20 / D-7.21. PR 7c proceeds without the bump.
- PR 3b umbrella closeout (T027 / T028 / T029, 2026-05-08): bookkeeping commit completing the PR 3b umbrella after the six functional sub-PRs (3b.A #319 / 3b.B #320 / 3b.C #321 / 3b.D #324 / 3b.E #326 / 3b.F #327) merged to `staging`. Zero rule-logic edits and zero engine-crate edits (Constitution VII §IV scheme-adoption restriction). T027: umbrella reviewer attestation aggregated into the PR description — D13 single-§-citation discipline (per-row in each declarative catalog), ≤3 branches per `impl Rule` body, and net-rule-delta math (59 → 47 across the six sub-moves). T028: new exact-rule-ID-set pin at `crates/capco/tests/post_3b_registration_pin.rs` complementing the existing count pin at `corpus_parity.rs:170-194`. The count pin alone catches "rule was added/removed"; the new exact-set pin catches "rule X renamed to rule Y at the same count" and "rule X deleted, rule Z added at the same count" — the drift classes the umbrella's structural commitment to a closed 47-rule set actually depends on. T029: new `pr-3b-corpus-regression` CI job mirroring T025's body, prefix-match-filtered to `refactor-006-pr-3b*` branches (covers the umbrella + all six sub-PR branches + the closeout branch). Final registered count: 47 (above the earlier ~38–44 Stage-1 estimate by 3 rules; D13 numeric band retired 2026-05-07 per the addendum precisely because the literal sub-move retirements were known to land outside any numeric band by construction — the qualitative gate "stayed within the sub-move's authorized primitive scope" is satisfied). End-state target ~10 surviving rules across all four stages remains binding; heavy lifting toward that target lands in Stage 3 (PR 4) and Stage 4 (PR 5+). See `docs/plans/2026-05-08-pr3b-closeout-T027-T028-T029-plan.md`.
- PR 3b.F (T026f) — Non-canonical input walker (2026-05-08): collapsed four hand-written ordering-validation rules — `CountryCodeOrderingRule` (E020, REL TO + JOINT alpha), `SigmaValidationRule` (E023, AEA SIGMA numeric sort), `SarProgramOrderRule` (E028, SAR program ascending), `SciCompartmentOrderRule` (E033, SCI compartment + sub-compartment numeric-then-alpha) — into a single `DeclarativeNonCanonicalInputRule` walker (rule ID `E060`) dispatching over a 5-row private `&'static [NonCanonicalRow]` catalog (`NON_CANONICAL_CATALOG`) inside `crates/capco/src/rules_declarative.rs`. The catalog is **structurally different** from PR 3b.D / 3b.E walkers: it is NOT a `Constraint::Custom` catalog on `CapcoScheme` — these are renderer-canonical-form concerns (per `marque-applied.md` §3.6 + §3.10 Move 7) absorbed by `MarkingScheme::render_canonical` once the renderer trait surface lands in PR 5+ (Stage 4 of the engine refactor); the walker retires cleanly when that lands. Per-row §-citations: REL TO USA-first alpha (§H.8 p150-151), JOINT alpha (§H.3 p56), AEA SIGMA numeric sort (§H.6 p108), SAR program ascending alpha (§H.5 p99), SCI compartment + sub-compartment numeric-then-alpha (§H.4 p61). Diagnostics emit with `Diagnostic.rule = "E060"`; per-row identification flows via the diagnostic message text + the `Diagnostic.citation` field (which is preserved verbatim from the retired rules so existing audit-stream consumers continue to work). Per-row severity preserved: `Severity::Fix` for rows 1-4 (REL TO/JOINT/SIGMA/SAR), `Severity::Error` for row 5 (SCI). Walker `default_severity()` = `Severity::Error` (strictest-of-rows precedent from PR 3b.A banner walker; OQ-3 PM-resolved). The legacy E020/E023/E028/E033 IDs are intentionally NOT preserved as severity-config aliases (per `feedback_pre_users_no_deprecation_phasing.md`: marque is pre-users; rewrite freely). One R-1 lex-tiebreaker behavior change documented in `tests/rel_to_invariants.rs`: pre-rename E020 won the FR-016 tiebreaker against E052 (`'E020' < 'E052'`) and produced canonical output in one fix pass; post-rename E052 wins (`'E052' < 'E060'`) and produces dedup-only output, with canonical reached on the second pass via E060. The fixed point is reached in ≤2 passes (idempotent thereafter). Net delta: 4 retired + 1 walker added = net −3 (registered rule count 50 → 47). See `docs/plans/2026-05-08-pr3b-F-non-canonical-input-walker-plan.md`.
- PR 3b.E (T026e) — SCI per-system catalog walker (2026-05-08): collapsed the 10 hand-written rules in `crates/capco/src/rules_sci_per_system.rs` (E042–E051; HCS-O / HCS-P / SI-G / TK companion-required + forbid-companion + range-ceiling rules) into a single `DeclarativeSciPerSystemRule` walker (rule ID `E059`) dispatching over a 5-row `Constraint::Custom("sci-per-system/...", ...)` catalog on `CapcoScheme` at CAPCO-2016 §H.4 family granularity: HCS-O companions (§H.4 p64), HCS-P NOFORN (§H.4 p66), HCS-P sub-compartment companions (§H.4 p68), SI-G companions (§H.4 p80), TK-{BLFH,IDIT,KAND} NOFORN (§H.4 p87 + p91 + p95). The class-floor portions of E044/E045/E046/E048/E049/E050 are absorbed by PR 3b.D's class-floor catalog rows (`class-floor/HCS-comp-sub`, `class-floor/HCS-comp`, `class-floor/SI-comp`, `class-floor/RSV-comp`, `class-floor/TK`, `class-floor/TK-BLFH`); no class-floor rows are added in PR 3b.E. Diagnostics emit with `Diagnostic.rule = "E059"`; per-row identification flows via the catalog row's `name` field. Severity-overridable per-walker via `[rules] E059 = "off|warn|error|..."`. Walker uses the same 3-layer optimization shape as PR 3b.D (axis-presence early-out + direct row dispatch + DRY emit helper). `crates/capco/src/rules_sci_per_system.rs` deleted. Two intentional regressions documented (PM-approved): (1) the actionable `S → TS` class-upgrade fix from E044/E046/E050 is lost — class promotion is FixIntent territory under PR 3c+; (2) the Warn-no-fix ambiguity-guidance text from E045/E048/E049 is lost as severity escalates Warn → Error per PR D's class-floor diagnostic. Net delta: 10 retired + 1 walker added = net −9 (registered rule count 59 → 50). See `docs/plans/2026-05-08-pr3b-E-sci-per-system-collapse-plan.md`.
- Decoder per-token prose null-hypothesis priors (#258): corpus-analysis stratified into marking (`tests/corpus/valid/`) and prose (Enron / CIA CREST / Congressional Record / GAO Reports — all confirmed prose-dominant per #258 owner confirmation); `priors.json` schema bumped `marque-priors-2 → marque-priors-3` with `token_prose_base_rates` and `country_code_prose_base_rates` tables; `marque_capco::priors::token_prose_log_prior` / `country_code_prose_log_prior` lookup APIs landed alongside the marking-side ones; `MISSING_PROSE_LOG_PRIOR` floor mirrors `MISSING_TOKEN_LOG_PRIOR` so unknown tokens contribute a neutral marking-y delta (zero); `decoder.rs::score_candidate` now returns `(prior, posterior, null_posterior)` with `null_posterior` summing the prose-side priors over the same canonical tokens (no feature deltas, no structural penalties); the `recognize` dispatch now treats `top.null_posterior` as a virtual runner-up — if it beats `top.posterior` the decoder returns zero candidates (FR-015, no R001 emitted on prose), if it loses it competes with `scored[1].posterior` for the runner-up that flows into `recognition_score`. Lifted the `StrictRecognizer` pin in `corpus_accuracy.rs::make_engine` — SC-003a precision (`tests/corpus/prose/article.txt`, Federalist-corpus `Notwithstanding (s) the early prevalence` case) now enforces zero diagnostics under the dispatcher default, the load-bearing test for this PR. Marking-stratum coverage caveat: `tests/corpus/valid/` is currently ~34 short fixtures, so marking-side priors are sparse; accuracy improves as the marking corpus grows. Document-level priors and region detection deferred to follow-up issues. The closed `proposal.replacement` canonical contamination channel (#257) is unaffected by this PR.
- Decoder default-on (#259): `Engine::new` installs `StrictOrDecoderRecognizer` (strict-first / decoder-fallback dispatch); `--deep-scan` CLI flag + `Engine::with_deep_scan()` retired; `Engine::with_recognizer(Arc<dyn Recognizer<CapcoScheme>>)` added for callers that pin a specific recognizer (typically `StrictRecognizer` for SC-001 strict-latency bench / `core_error_isolation.rs` / `corpus_accuracy.rs`). WASM `lint_deep_scan_native` / `fix_deep_scan_native` deleted; the regular `lint_native` / `fix_native` exercise the dispatcher transparently. Live-typing surfaces concerned with per-keystroke latency are expected to debounce calls into the engine. Closed two leak channels in the same PR: the R001 diagnostic message no longer interpolates input bytes (`format!("decoder-recognized canonical form: {replacement:?}")`), and `AppliedFix.proposal.original` is set to the empty string for decoder-path fixes (Constitution V Principle V / G13). Remaining tracked items: `proposal.replacement` canonical contamination (#257) — decoder-canonicalization sometimes uppercases unrecognized middle tokens — and the decoder's case-canonicalization producing a precision regression on `(s)` in prose contexts (the SC-003a corpus, gated by pinning `corpus_accuracy.rs` to `StrictRecognizer` until per-token null-hypothesis priors land via #258). `feat/preceded-by-whitespace` (#262) closed a related precision channel — single-letter portions glued to a preceding word and bare `Us(Restricted)` markings — but the mid-prose null-hypothesis case still requires #258.
- Phase 5 (vocabulary surface + trait-surface completion): build-time generation of per-token metadata tables (T080–T082); `impl Vocabulary<CapcoScheme> for CapcoScheme` (PR-2); FOUO regression guards confirming FOUO stays an active dissem control (FR-020, no `FOUO → CUI` migration entry); `Codec<S>` trait surface published with no concrete impls (T078, FR-019); `T089b` readiness stub exercising every Phase-E trait surface as if building a minimal second scheme (SC-010 deferred-verifiable check). Phase 5 PR-1 (#141) → PR-3 (#146).
- Phase 4 (probabilistic recognition + audit v2): compile-time corpus priors bake (PR-1, #111); `Box<dyn Recognizer<S>>` dispatch with `StrictRecognizer` as the default path (PR-2, #112); `DecoderRecognizer` for probabilistic recovery (PR-3 #114, PR-4b #127); `MARQUE_AUDIT_SCHEMA` env-pinned at build time, `marque-mvp-2` audit records emit `Confidence` provenance (PR-4, #122); SC-002 deep-scan latency bench + SC-004 mangled-corpus accuracy gate at 0.85 threshold (PR-6, #135); corpus-override security gates (PR-5, #131); fuzzy CAPCO-token corrector (#96). The R001 message + `AppliedFix.proposal.original` leak channels were closed in the decoder-default-on flip (#259, see entry above); `proposal.replacement` canonical contamination remains tracked as #257.
- Phase 9 (S003 + T035c-21 PR-B): S003 `joint-usa-first` style rule per §H.3/§H.8 + E039 (NODIS/EXDIS clears banner REL TO) + E040 (NODIS/EXDIS banner roll-up) + E041 (NODIS supersedes EXDIS in portion) per §H.9 p172–174. Rule count: 41 → 44.
- Phase B (recursive lattice & decoder plan, §12): built-in lattice constructors (`OrdMax`, `OrdMin`, `FlatSet`, `IntersectSet`, `SupersessionSet`, `ModeSet`, `MaxDate`, `OptionalSingleton`, `Product`); `Scope` / `DiffInput` / `CategoryShape` / `PageRewrite` trait-surface additions; `SciSet`/`SarSet`/`FgiSet` lattice types in `marque-capco` with §3.3a equal-depth meet policy; `CapcoScheme::project(Scope, ...)` taking over from `project_banner`; `capco/noforn-clears-rel-to` declared as the first `PageRewrite`; tetragraph expansion tables consolidated in `marque-capco::vocab`; `AggregationOp::Custom` retired from runtime dispatch (build-time shorthand only). Phase 3 of 004 (#69) added the topological page-rewrite scheduler with cycle and unannotated-axis detection.
- Phase 7: Criterion benchmarks (lint_latency, linear_scaling), corpus accuracy harness, WASM parity scaling to full corpus, cargo-fuzz target, bench-check regression gate
- Phase 6: WASM web worker build with SC-008 parity, `batch` feature flag, CachedAhoCorasick optimization
- Phase 5: Configurable severity overrides, corrections map with AhoCorasick pre-scanner
- SCI compartments (#003): structural subparser + `SciMarking` data model, E032–E035 rules, banner roll-up via `PageContext::expected_sci_markings()` (rule count 35 → 39)
- Phase 8: SAR implementation — structural `SarMarking` type (replaces empty `SarIdentifier` CVE enum), six new rules E026–E031 covering portion form, classification constraint, ordering, indicator-repeat coalescing, and banner roll-up per CAPCO-2016 §H.5
- Phase 3-4: Full lint/fix/audit pipeline, 29 CAPCO rules (E001–E025, W001–W003, C001), CLI with check/fix subcommands
- Phase 1-2: marque-ism crate extraction, test corpus scaffolding, benchmark stubs
