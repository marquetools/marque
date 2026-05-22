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
| `marque-scheme` | Domain-neutral trait surface for structured marking schemes. Defines `MarkingScheme`, `JoinSemilattice`, `MeetSemilattice`, `Lattice` (blanket-impl marker), `BoundedJoinSemilattice`, `BoundedMeetSemilattice`, `BoundedLattice` (blanket-impl marker), `Category`/`AggregationOp`/`CategoryShape`, `Constraint`, `Parsed<M>`, `Scope`, `PageRewrite`, and built-in lattice constructors (`OrdMax`, `OrdMin`, `FlatSet`, `IntersectSet`, `SupersessionSet`, `ModeSet`, `MaxDate`, `OptionalSingleton`, `Product`). The `Lattice` trait split (issue #456 / PR #502) divided `Lattice` into `JoinSemilattice + MeetSemilattice` halves; `DissemSet`, `JointSet`, and `SupersessionSet` implement only `JoinSemilattice`. One permitted runtime dep: `smallvec` (inline-2 buffer for `ReplacementIntent::FactRemove::facts`; keeps single-fact removals heap-free per #348). No dependency on `marque-ism`. Phase B landed the recursive-lattice surface — see `docs/plans/2026-04-19-recursive-lattice-and-decoder.md`. |
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

**Phase B canonicalization.** Post-Phase-B, `SciSet` (in `marque_capco::lattice`, the lattice form of SCI state) is the canonical page-context storage: it implements `JoinSemilattice + MeetSemilattice` (i.e. `Lattice` via blanket impl), round-trips with `[SciMarking]` via `SciSet::from_markings` / `SciSet::to_markings`, and composes through `CapcoScheme::project(Scope::Page, ...)`. `SciSet` (and `SarSet`) deliberately do **not** implement `BoundedLattice` — SCI control systems and SAR program identifiers are both agency-extensible open sets, so no lawful finite `top` exists. Use `SciSet::empty()` / `SciSet::default()` when you need the lattice bottom. `IsmAttributes::sci_controls` stays populated for rules that currently read it, but is a compatibility view scheduled for removal in Phase C or D when no rule references it. New rules that need compartment / sub-compartment semantics should read `sci_markings` or construct an `SciSet`; rules that just need "which bare control systems appear" can stay on `sci_controls` until the migration closes.

Banner roll-up for SCI (E035) uses `PageContext::expected_sci_markings()`, which unions compartments and sub-compartments across all portions on the page and sorts per §A.6 p15 (numeric first, alpha after). Authority: CAPCO-2016 §A.6 (grammar, canonical example p16) + §H.4 (per-system banner precedence).

**NATO SAPs (PR 9c.1 T134).** `SciControlSystem::NatoSap(NatoSap)` is the canonical home for `BOHEMIA` and `BALK` (CAPCO-2016 §G.2 p40 + §H.7 p127). They render standalone (no `SAR-` prefix) in the SCI block position — e.g. `(//CTS//BOHEMIA)` or `(//CTS//BALK/BOHEMIA)`. BALK sorts before BOHEMIA alphabetically per §H.7 p127 worked example. NATO SAPs are CAPCO-only (no ODNI ISM CVE entry) — the third `SciControlSystem` variant keeps `Published(SciControlBare)` ODNI-faithful and `Custom(SmolStr)` reserved for agency-allocated `[A-Z0-9]{2,5}` identifiers per §A.6 p15. Legacy `CTS-B` / `CTS-BALK` text and the banner-form equivalents canonicalize through the strict parser into bare CTS class + SCI NatoSap companion; rule E066 emits a Recanonicalize FixIntent so the source text is re-rendered to the canonical multi-block form.

### SAR (Special Access Required)

SAR (Special Access Required) markings are modeled structurally, not as a CVE-derived enum. The ODNI public `CVEnumISMSAR.xml` is empty because SAR program identifiers are agency-assigned codewords not centrally registered. `marque-ism::SarMarking` captures the full hierarchy — programs, compartments, sub-compartments — parsed by a hand-written subparser in `marque-core` (see `parse_sar_category`). The six SAR rules (E026–E031) validate syntax, ordering, classification constraints, and banner roll-up per CAPCO-2016 §H.5.

### ATOMAL (NATO AEA)

ATOMAL is a NATO AEA marking — Atomic Energy Act information shared with NATO+UK under bilateral §123/§144 sharing agreements. Per CAPCO-2016 §G.2 p40 (Table 5: ARH by Registered Marking) ATOMAL is a registered standalone control marking; the §H.7 p122 worked example (`SECRET//RD/ATOMAL//FGI NATO//NOFORN`) places ATOMAL in the AEA axis alongside RD/FRD/TFNI — **not** as a NATO classification portion-suffix.

PR 9c.1 T134 introduced `AeaMarking::Atomal(AtomalBlock)` as the canonical home. The block is empty (no registered sub-markings) but mirrors `RdBlock`/`FrdBlock` so a future CAPCO grammar extension is a planned migration. The strict parser canonicalizes legacy compound text (`CTSA`, `CTS-A`, `NSAT`, `NS-A`, `NCA`, `NC-A`, banner-form `COSMIC TOP SECRET ATOMAL`, etc.) into bare NATO class + AEA ATOMAL companion at parse time; rule E066 emits a Recanonicalize FixIntent that re-renders to the canonical multi-block form (`(//CTS//ATOMAL)`, etc.) per the §G.2 p40 Table 5 registration. Per project memory `remark-on-derivative-use-is-marque-autofix`, Marque automates the canonical re-marking the manual permits doing by hand. The legacy fused `NatoClassification::*Atomal` variants (`NatoConfidentialAtomal`, `NatoSecretAtomal`, `CosmicTopSecretAtomal`) and the corresponding `*Bohemia` / `*Balk` variants were retired in PR 9c.1 Commit 5.

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
# Wire-string `<scheme>:<predicate_id>` keys per T044 (FR-026).
"capco:banner.classification.portion-mark-in-banner" = "fix"   # off | suggest | info | warn | error | fix
"capco:banner.classification.usa-trigraph" = "fix"

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
3. Rule IDs are 2-tuples `RuleId::new("<scheme>", "<surface>.<category>.<predicate>")` (post-T044). For CAPCO rules `<scheme>` is `"capco"`; `<surface>` ∈ `{ banner, portion, page, marking, closure }`; `<category>` matches the lattice axis (`classification | sci | sar | dissem | fgi | nato | aea | declassification | fouo | banner-rollup | metadata`); `<predicate>` is descriptive English-with-hyphens. The default-severity tier is encoded via `Severity::Error | Severity::Warn | Severity::Suggest | Severity::Info` on the `Rule` trait, not via an ID prefix. See `docs/refactor-006/legacy-rule-id-map.md` for the historic `E### / W### / S### / C###` rename table.
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

## Post-006 Stable Surface

PR 10 merge began FR-049's API stability freeze; T044 unfroze the
surface for a single atomic PR (2026-05-22) that landed the
`RuleId` 2-tuple form and bumped the audit schema
`marque-1.0 → marque-2.0`. The freeze re-engaged at T044's merge.
Once this PR lands, the following surfaces are committed and require
a coordinated `marque-2.1` (additive) or `marque-3.0` (breaking)
audit-schema bump per FR-049 for any change:

- **Crate dependency graph** per Constitution VII §IV — the
  canonical graph diagram in this file's `Crate Dependency Graph`
  section is the frozen shape. A future second scheme (CUI, NATO,
  partner-national) sits alongside `marque-ism` as a peer
  foundation; it does NOT modify the engine convergence node.
- **`MarkingScheme` trait** in `marque-scheme` — `Marking`
  associated type bound (`JoinSemilattice`); `parse` / `project` /
  `render_portion` / `render_banner` / `render_canonical` /
  `categories` / `constraints` / `closure_rules` / `templates`
  surface; `Scope` enum; `CategoryShape` / `Constraint` /
  `PageRewrite` / `ClosureRule` types.
- **`Rule<S>` trait** in `marque-rules` — `id` / `name` /
  `default_severity` / `check` / `phase` /
  `additional_emitted_ids` / `trusted` / `cited_authorities`
  surface; `Phase` non-exhaustive enum; `RuleContext<'a>`
  `#[non_exhaustive]` shape + `new` / `with_*` constructors.
- **`RuleId` 2-tuple form** (FROZEN at T044, 2026-05-22) — the
  `(scheme: &'static str, predicate_id: &'static str)` shape with
  the canonical `<scheme>:<predicate_id>` wire string produced by
  `RuleId::Display`. Predicate IDs follow
  `<surface>.<category>.<predicate>` where `<surface>` ∈
  `{ banner, portion, page, marking, closure }` (the `closure`
  surface was added at T044 per PM OD-1 so closure-operator
  inferences don't conflate with strict page-banner rules at
  the predicate level). Reserved schemes: `"engine"` (synthetic
  engine-minted diagnostics) and `"test"` (test fixtures); neither
  is a valid `MarkingScheme` registration target. Engine sentinels:
  `("engine", "recognition.decoder-recognized")` (R001) and
  `("engine", "fix.reparse-failed")` (R002). See
  `docs/refactor-006/legacy-rule-id-map.md` for the 114-row
  rename table from the pre-T044 flat-string form.
- **Typed `Citation`** in `marque-scheme` — `Citation::new` plus
  ergonomic helpers (`capco` / `capco_section` /
  `capco_table`); `SectionRef` + `SectionLetter` + `PageNumber`;
  `AuthoritativeSource` enum with `Capco2016` / `Config` /
  `EngineInternal` variants.
- **`AppliedFix<S>` audit-record envelope** — sealed
  `__engine_promote` constructor (Constitution V Principle V); the
  `marque-2.0` JSON wire format pinned at T044
  (`MARQUE_AUDIT_SCHEMA = "marque-2.0"`); structured 2-tuple
  `"rule"` field per FR-026; BLAKE3 digest field; closed
  `MessageTemplate` JSON projection.
- **G13 audit-content-ignorance invariant** — the SC-001 canary
  scan at `crates/engine/tests/audit_g13_canary.rs` is the
  type-system + corpus-regression form of the invariant. Adding
  a free-form string surface to any audit-side type breaks the
  freeze.

**Not frozen** (open scope for post-006 work):

- **v0.2 LMDB incremental cache** (`crates/engine` `cache` feature)
  — the `LintResult` cache surface (FR-052 onward) is scope for a
  separable v0.2 line, not for the 006 stability freeze.
- **`marque-extract` format-extraction backend** — Kreuzberg
  integration is gated on a licensing decision; the scaffolded
  `Extractor` / `ExtractedDocument` / `ExtractionOptions` /
  `MetadataReport` surface is frozen, but the backend is open.
- **Server auth + structured logging middleware** in
  `marque-server` — Tower-layer surface is frozen; specific
  middleware implementations land post-006.

Upstream-source bumps: pinned via
`[package.metadata.marque]` in `crates/ism/Cargo.toml`
(`ism-schema-version` / `ism-data-version` /
`ismcat-tetra-version`) and via the matching `[build-dependencies]`
versions on `ism` / `ism-ismcat`. ODNI schema revisions are
deliberate, reviewed migrations per Constitution VIII —
re-verify every cited authority against the new source before
the migration lands.

## Current Status

MVP complete. Full lint → fix → audit pipeline for raw text with **28 registered CAPCO rules** at HEAD (post-PR #578 + issues #261/#250/#251 + T044 rule-ID migration) — `38 post-PR-4b umbrella − 15 PR #578 declarative-wrapper retirements + S008 #559-C1 + E071 #261 + S009 #250 + S010 #251 + E072 #251 = 28`; the post_3b_registration_pin gates this exact set against the 2-tuple wire strings. See `crates/capco/README.md` for the authoritative rule inventory and `docs/refactor-006/legacy-rule-id-map.md` for the T044 rename table from the prior `E### / W### / S### / C###` flat-string form. Rule-collapse history (using the historical pre-T044 IDs as archaeological references — see legacy-rule-id-map for current wire strings): W001 retired in T035c-14 per CAPCO-2016 §F; E052 added in issue #234 PR-B per §H.8; S004 trigraph-suggest added in PR-C of #186 per §H.8 (first consumer of the suggest-don't-fix channel); PR 3b.A collapsed three banner roll-up rules (E031/E035/E040) into the `BannerMatchesProjectedRule` walker; PR 3b.D collapsed class-floor rules (E022/E025/E027) into the `DeclarativeClassFloorRule` walker (was E058, now `capco:banner.classification.floor-*` predicates); PR 3b.E collapsed the SCI per-system rules (E042–E051) into the `DeclarativeSciPerSystemRule` walker (was E059, now `capco:marking.sci.*` predicates); PR 3b.F collapsed the ordering rules (E020/E023/E028/E033) into the `DeclarativeNonCanonicalInputRule` walker (was E060). CLI (`check`, `fix`) and WASM (`lint`, `fix`) produce byte-identical NDJSON diagnostics. Configurable severity overrides, corrections map, and confidence thresholds. Batch processing via `BatchEngine` with concurrency control. Criterion benchmarks measure interactive latency p95 ≤ 16 ms on 10 KB single-portion inputs (SC-008) and multi-page projection + two-pass overhead (SC-009 composite); the `fix_throughput` linear-scaling R² sub-gate within SC-009 is active (R² = 0.994 measured; O(N²) accumulation fixed in PR #674, closing #306). Corpus accuracy harness enforces ≥ 95% lint and fix accuracy per-rule against the invalid-fixtures corpus. `cargo-fuzz` target exercises `Engine::lint` on arbitrary `&[u8]`.

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

**Audit schema**: `MARQUE_AUDIT_SCHEMA` env var pinned at build time, validated against the closed accept-list `["marque-2.0"]`. Defaults to `"marque-2.0"` as of T044 (2026-05-22) — the T044 PR atomically landed the `RuleId` 2-tuple migration and the coordinated `marque-1.0 → marque-2.0` bump under FR-049 unfreeze. Prior cutovers: PR 3c.2.D landed `mvp-3 → 1.0` (Canonical wire-up + BLAKE3 + closed `MessageTemplate` JSON); PR 3c.B Commit 10 landed `mvp-2 → mvp-3` when `FixProposal` retired and the audit envelope took its structural `proposal: FixIntent | TextCorrection` sub-object form, closing the G13 audit-content-ignorance channel. Re-exported as `marque_engine::AUDIT_SCHEMA_VERSION`. A single binary emits exactly one schema (FR-014).

## Recent Changes
- **T044 — RuleId 2-tuple migration + `marque-1.0 → marque-2.0` audit-schema bump** (post-PR-10, FR-049 unfreeze, 2026-05-22): dedicated post-PR-10 PR that unfroze FR-049's stability commitment for a single atomic change carrying the FR-026 / FR-044 `RuleId` 2-tuple migration. The freeze re-engages at T044's merge with `marque-2.0` as the new inflection. Wire-string form `<scheme>:<predicate_id>` (e.g., `"capco:portion.dissem.noforn-conflicts-rel-to"`) for text contexts (`.marque.toml` `[rules]` keys, CLI text output, log lines, grep targets) via the `RuleId::Display` impl; JSON audit records serialize the structured 2-tuple shape `{"scheme": "...", "predicate_id": "..."}` per PM OD-2 (the CLI's `DiagnosticJson` emits struct-order via typed `RuleIdJson<'a>` in `marque/src/render.rs`; the audit-record NDJSON path emits the same struct-declaration order via typed `#[derive(Serialize)]` on `RuleIdJson<'a>` — `scheme` field first, `predicate_id` second; consumers MAY treat the order as stable so long as they parse via `serde_json` rather than position-indexing). **Predicate-ID convention**: `<surface>.<category>.<predicate>` lowercase-with-hyphens, three-segment minimum. `<surface>` ∈ `{ banner, portion, page, marking, closure }` — the `closure` value was added at T044 per PM OD-1 refinement so closure-operator inferences don't conflate with strict page-banner rules at the predicate level. `<category>` matches the lattice / axis category (`classification | sci | sar | dissem | fgi | nato | aea | declassification | fouo | banner-rollup | metadata`). **Reserved schemes**: `"engine"` (synthetic engine-minted diagnostics per FR-044) and `"test"` (test fixtures); neither is a valid `MarkingScheme` registration target. **Engine sentinels**: per PM OD-4 the numeric `r001`/`r002` placeholders from the pre-T044 spec wording were dropped — `R001` is now `RuleId::new("engine", "recognition.decoder-recognized")` and `R002` is `RuleId::new("engine", "fix.reparse-failed")`. The `scheme = "engine"` tuple already carries the cross-version anchor; descriptive `<class>.<predicate>` reads better at audit-log triage. **Bridge dispatcher** at `crates/engine/src/engine.rs` simplified to a no-op pass-through per PM OD-8: catalog row labels ARE the predicate IDs (no translation table). **Surface inventory**: 28 active CAPCO rules + 27 class-floor catalog rows (`capco:banner.classification.floor-*`) + 5 SCI per-system catalog rows (`capco:marking.sci.*`) + 10 `ClosureRule` rows (`capco:closure.dissem.*` per PM OD-1) + 15 retired declarative-wrapper IDs (PR #578 lineage) + `R001`/`R002` engine sentinels all migrated; 67 corpus `expected.json` fixtures updated to the structured JSON shape; `EXPECTED_RULE_IDS` pin at `crates/capco/tests/post_3b_registration_pin.rs` re-pinned to wire-string canonical form. **Atomic with audit-schema bump**: `MARQUE_AUDIT_SCHEMA` build-time accept-list flipped from `["marque-1.0"]` to `["marque-2.0"]` in `crates/engine/build.rs`; `marque_engine::AUDIT_SCHEMA_VERSION` re-exports the new value; pre-cutover envelopes (`mvp-1` / `mvp-2` / `mvp-3` / `marque-1.0`) unreadable by post-cutover binaries per FR-037 clean-break. **PM decisions**: OD-1 through OD-8 all closed in `docs/refactor-006/2026-05-22-T044-pm-decisions.md`; the architect plan with the full predicate-ID convention, sequence of structural changes, and the parallelization plan lives at `docs/refactor-006/2026-05-22-T044-rule-id-tuple-plan.md`; the 114-row rename table is `docs/refactor-006/legacy-rule-id-map.md` (living document — appended-to, never silently rewritten per plan §5 R-4).
- PR 10.B — final polish (006 T123 / T124 / T125 / T136 / T137 / T138 / T138a / T139 / T140 / T141 / T146, 2026-05-21): bookkeeping-only PR closing the PR 10 series. **T138a + T141** — `tools/audit-cleanup-check.sh` FR-037 absence-check (no `crates/audit-reader/` dir, no audit-reader Cargo entry, no `marque_engine::reader::*` surface) wired into the Format+Lint CI job; `cargo check --workspace --all-targets --all-features` + WASM rust-side compile verified locally. **T140** — `specs/006-engine-rule-refactor/quickstart.md` "How to add a new rule" example rewritten against actual symbols at HEAD (the prior version drifted across keystone window: `evaluate` → `check`, `target_span()` → `candidate_span`, made-up `MessageTemplate::PortionUnknownDissem` → real `ConflictsWith`, `RuleId(2-tuple)` → `RuleId::new("E###")` per FR-049 deferral, `FixIntent` shape fixed). **T124 + T125** — `docs/refactor-006/revertability-discipline.md` documents the per-PR revertability contract (PR 0 → PR 10 table with sub-PR umbrella summarization) + the keystone CI matrix verification (T025 / T029 / T145 jobs cover the `{3a-only}` / `{3a + 3b}` / `{3a + 3b + 4b}` subsequences per SC-014). T056 (literal `pr-3c-corpus-regression`) documented as structurally subsumed by post-3c sub-PR jobs; T126 documented NOT APPLICABLE per the PR 6 sub-PR sequencing bypass. **T123 + T136 + T137** — data-driven SC completion: `docs/refactor-006/sc-completion.toml` (14 `[[sc]]` entries, source of truth) + `tools/sc-completion-report/render.py` (Python stdlib `tomllib`; out-of-workspace per Constitution III) + `docs/refactor-006/sc-completion-report.md` (rendered cold-storage). Status honestly recorded as `regressed` for SC-008 (cumulative `lint_10kb` drift past +10% gate, SC-001 16ms ceiling intact) and `partial` for SC-009 (`fix_throughput` R² gate disabled per #306) and SC-010 (`tests/corpus/mangled/threshold.toml` still "pending" — operational SC-004 0.85 floor stands in). **T139** — CLAUDE.md `Post-006 Stable Surface` section added (FR-049 freeze inventory: crate graph, `MarkingScheme`, `Rule<S>`, typed `Citation`, `AppliedFix<S>` + `MARQUE_AUDIT_SCHEMA = "marque-1.0"`, G13 invariant); Recent Changes entries dated 2026-05-01 or earlier archived to `docs/refactor-006/recent-changes-archive.md`. **T138 + T146** — `tasks.md` status notes refreshed: T138 VERIFIED (#257 closed 2026-05-20 in PR 3c.2.D; zero surviving MASKING-PIN tags at PR 10.B HEAD); T146 ISSUE FILED (#665, engine-crate touch out of scope for closeout per Constitution VII §IV). **Closing block** at the bottom of `tasks.md` inventories every task PR 10.B touched. Refactor 006 functionally complete at PR 10.B merge; FR-049 stability freeze begins.
- **Issue #261** (2026-05-20): `FgiExplicitWithTrigraphRule` (E071) — FGI with explicit trigraph when concealment intended or acknowledgment contradicted per CAPCO-2016 §H.7 p124. Detection: `tok_text.split_whitespace().next() == Some("FGI")` on the classification token span with non-empty `fgi.countries` (parser drops "FGI" token from `FgiClassification.countries` silently; raw text is the only signal without ISM-crate changes). Four-case behavioral spec: **Case A** (countries ⊆ REL TO) → Error + fix (drop "FGI " prefix, `"FGI DEU R"` → `"DEU R"`); **Case B** (bare FGI, no trigraph) → valid, no diagnostic; **Case C** (countries ∩ REL TO = ∅) → Warn (conceal form fix) + Suggest (acknowledged form) + optional NF Suggest (unacknowledged FGI is caveated per IC convention, §B.3 Table 2 p21 Row 0); **Case D** (partial overlap) → Error (no fix) + Suggest ack-all + Suggest conceal-all + optional NF Suggest. `Phase::WholeMarking` (NF companion uses `FactAdd(NOFORN, Scope::Portion)` — crosses token boundary). Registration pin updated 24 → 25; `phase_assignment.rs` allowlist updated; 12-test integration file at `crates/capco/tests/e071_fgi_explicit_with_trigraph.rs`. §H.7 p124 citation verified against `crates/capco/docs/CAPCO-2016.md` per Constitution VIII.
- **PR 4 tests closeout** (006 T116/T117/T117a/T118/T119, 2026-05-19): bookkeeping-only PR closing the five PR-4 test tasks that PR #558's checkbox audit flipped without filling the real gaps. Zero rule-logic edits; zero engine-crate edits (Constitution VII scheme-adoption boundary observed). **T116** — 4 new modules in `crates/capco/tests/category_lattice_laws.rs` (`sci_set` + `sar_set` proptest harnesses capped at ≤3×3×3 per PM doc D-2; `non_ic_dissem_set` brute-force compositional-invariance over `from_attrs_iter`; `display_only_block` 6-state brute-force exhaustion of the 4-variant absorbing-element pattern) + 3 join-side dominance tests extending the existing `fgi_set_concealed_top` module per §H.7 p128. Algebraic-law coverage now consolidates all 12 lattice types in the canonical file (proptest_lattice.rs retains parallel coverage). **T117** — 4 new tests in `crates/capco/tests/cross_axis_dominance.rs` covering the non-AEA fixture classes via `scheme.project(Scope::Page, ...)`: `class_evicts_fouo_via_classification_ascent` (§H.8 p134 classified sub-clause); `non_fdr_control_evicts_fouo` (§H.8 p134 UNCLASSIFIED-with-other-control sub-clause; LES non-FD&R per §H.9 p181 + FDR_DOMINATORS list); `fgi_banner_rollup_retains_marker_on_cross_classified_page` (§H.7 pp123-129); `sci_cross_system_canonicalization` (§H.4 p61 + §A.6 pp15-17). **T117a** — 1 new test `us_reciprocates_equivalent_protection_for_foreign_portion` mirroring `tests/corpus/foreign/mixed_us_foreign_rollup.expected.json` (#276 corpus ground truth) at the property-test level per §H.7 pp123-129. **T118** — new `crates/capco/tests/lattice_corpus_runner.rs` driving the 5 worked-example fixtures under `tests/corpus/lattice/` through `CapcoScheme::project(Scope::Page, ...)` + `Engine::lint(...)` with byte-identity assertions against 5 new `.expected.json` sidecars; CAB-vs-portion-banner shape dispatch per PM doc D-5. Sidecar `_note` fields carry §-citations re-verified against `crates/capco/docs/CAPCO-2016.md` at authorship per Constitution VIII (D-8): §H.8 p134 (fouo-eviction-class + fouo-eviction-non-fdr); §H.7 pp123-129 (fgi-banner-rollup); §H.4 p61 + §A.6 pp15-17 (sci-cross-system; banner includes closure-injected ORCON/NOFORN per §H.4 p80 + §H.4 p87 example banners); §E.4 pp33-34 (aea-commingling — CabCommingling shape, lint-only). The 4 E059 diagnostics on the SCI fixture pin existing engine behavior (SI-G requires ORCON; TK-BLFH requires NOFORN). **T119** — re-scoped per PM doc D-6: probe-first wiring of `tests/corpus/documents/marked/*.md` (40 CIA CREST fixtures) into `Engine::lint`. The probe (`probe_documents_lint_clean`, `#[ignore]`-gated) ships in-tree as the regression replay surface; the assertion gate did NOT land because the probe surfaced **160 diagnostics across 40 fixtures (0/40 clean)** at authorship — rule IDs firing E008/E031/E035/E040/E068/E069 (real engine behaviors with valid CAPCO citations; ground-truth sidecars claiming `"diagnostics": []` are unverified). Per PM doc D-6 "drift" branch: assertion gate deferred to follow-up issue with per-document triage. `serde` + `serde_json` added to `crates/capco/Cargo.toml` `[dev-dependencies]` for the new runner's local sidecar deserialization (parallel `LatticeExpectedFixture` type per PM doc D-3, NOT an extension of `marque_test_utils::ExpectedFixture`). See `docs/plans/2026-05-19-pr4-tests-closeout-pm-decisions.md` for the operative contract.
- **PR 4b umbrella closeout** (006 T142-T145, 2026-05-19): bookkeeping-only PR aggregating the nine-sub-PR umbrella attestation. **T142** — single-§-citation discipline (D13) re-verified at this PR's authorship per Constitution VIII across 12 lattice types (`SciSet`/`SarSet`/`FgiSet`/`AeaSet`/`ClassificationLattice`/`NatoClassLattice`/`DeclassifyOnLattice`/`NatoDissemSet`/`RelToBlock` with both halves; `DissemSet`/`JointSet`/`DisplayOnlyBlock` Join-only per issue #456 (PR #502) / PR #538) + 27 `PageRewrite` rows + 10 `ClosureRule` rows + 39 `Constraint::Custom` rows + W004 (registered count 38 → 39 in 4b-B); engine-crate touch ledger documents 5 within-006 Constitution VII precedent breaches (4b-B Commit 2 OC-USGOV/RELIDO PageContext bugfixes; 4b-C Commit 5 FOUO Step 3 + UCNI strip retirement; 4b-D.2 hot-path flip + `MarkingScheme::Marking: JoinSemilattice` bound relaxation; 4b-D.3 S007 `ProjectedMarking::is_solely_nato_classified` addition; 4b-E `assert_impl_all!(CanonicalAttrs: Send, Sync)` + `sar_sort_key` relocation); per-axis net-delta math 3 → 12 Join impls (pre-4b: Sci/Sar/Fgi; +AeaSet 4b-A; +7 in 4b-B; +DisplayOnlyBlock 4b-E) / 3 → 9 Meet impls (pre-4b: Sci/Sar/Fgi; +AeaSet 4b-A; +5 in 4b-B with Joint/Dissem Join-only per issue #456 (PR #502)) / 0 → 2 BoundedJoin impls (ClassificationLattice + NatoClassLattice; 4b-B) / 0 → 2 BoundedMeet impls (same pair; 4b-B) / **14 → 27 PageRewrites** (pre-4b breakdown: 4 pattern_a + 2 noforn_clears + 8 transmutation_stubs = 14; 4b adds: 4b-C +9 (Pattern-B 2 + Pattern-C 7) / 4b-D.2 +1 (`noforn-clears-display-only-to`) / #541 +1 (`sbu-nf-evicted-by-classified`) / #552 +1 (`sbu-nf-supersedes-sbu`) / #555 +1 (`les-nf-supersedes-les`) = 13 new rows; 14 + 13 = 27) / 0 → 10 ClosureRules / 38 → 38 registered rules (W004 added 4b-B, W002 retired PR #507 closing #470 — net-zero across the umbrella; post_3b pin gates the exact set). **T143** — compile-time pin at `crates/capco/tests/lattice_static_assertions.rs` locking the 12+9+2+2 trait-impl shape via `assert_impl_all!` plus `assert_not_impl_any!(MeetSemilattice)` for the three Join-only types (catches D3 type-bound drift at build time). `static_assertions` added to `crates/capco/Cargo.toml` `[dev-dependencies]` (workspace dep already declared). **T144** — runtime triple-pin at `crates/capco/tests/post_4b_lattice_inventory_pin.rs` with three sub-assertions: positional list of 27 `PageRewrite` IDs (row order load-bearing for Kahn's scheduler per `build_page_rewrites` doc-comment); positional list of 10 `ClosureRule` names (Kleene-fixpoint walk order load-bearing per `CAPCO_CLOSURE_RULES` doc-comment); sorted set of 39 `Constraint::Custom` labels (5 SCI-per-system + 27 class-floor + 7 core-catalog). Catches rename-at-same-count + swap-at-same-count + reorder-at-same-count drift. **T145** — new `pr-4b-corpus-regression` CI job branch-filtered to `refactor-006-pr-4b*`, mirroring T025 (3a) / T029 (3b) — SC-014 keystone-subsequence verification for {3a + 3b + 4b}. Zero rule-logic edits; zero engine-crate edits (Constitution VII scheme-adoption boundary observed for the closeout itself — the closeout aggregates within-006 precedent, does not extend it). T146 deferred: `SupersessionSet` Join-only compile-time pin lives in `marque-scheme` and requires authorized engine-crate touch; tracked as follow-up. See `docs/plans/2026-05-19-pr4b-closeout-pm-decisions.md` (PM contract) + `docs/plans/2026-05-19-pr4b-closeout-architect-plan.md` (sub-PR inventory) + `docs/plans/2026-05-19-pr4b-closeout-rust-preflight.md` (drift-class taxonomy).
- PR 4b-E (006 T112, 2026-05-18): retired `PageContext::expected_*` accessor surface (17 methods + `render_expected_banner` + `project` + `is_classified` + `is_solely_nato_classified` + supporting helpers; ~3457-line deletion) now that PR 4b-D.2 flipped the hot path to `scheme.project(Scope::Page, ...)`. Added 5 lattice helpers (`DisplayOnlyBlock::from_attrs_iter` parallel to `RelToBlock`, `FgiSet::from_attrs_iter` per §H.7 p122 + p123 + p128 unioning per-portion markers with classification-derived producers, `NonIcDissemSet::from_attrs_iter` carrying `needs_nf` per §H.9 p172/p174/p178/p185, `DeclassExemptionAccumulator` for last-observed exemption (renamed from `DeclassExemptionLattice` during PR 4b-E review fix-up — the type is a projection accumulator, not a lattice; the prior `JoinSemilattice` impl was non-commutative and was dropped per `NonIcDissemSet`'s precedent), free `sci_controls_from_markings` for the flat CVE projection) so `crates/capco/src/scheme/marking.rs::join_via_lattice_with_context` could migrate off the residue accessors. WASM `compute_banner_native` migrated to `scheme.render_banner(scheme.project(Scope::Page, &markings))`; `generate_cab_native` inlined the per-portion `declass_exemption` accumulator and migrated `is_classified` to a projected-marking read (OQ-1 option a; no new `CabProjection` type). S005 (`analyze_uncertain_reduction`) migrated to `NonIcDissemSet::from_attrs_iter` + `RelToBlock::from_attrs_iter`. The dead `marque_capco_test_support` module retired (its only `#[cfg(any())]`-gated consumers were already disabled pending PR 3c.2). Parity gate at `crates/capco/tests/page_context_lattice_parity.rs` renamed to `lattice_vs_scheme_parity.rs`; the three pre-PR-4b-E documented divergences (G-3 pure-NATO, joint_unanimous_two_portions, joint_single_portion_no_us) all CONVERGED to byte-identity (OQ-7 blocking discipline satisfied). 12 new fixtures exhibit a `dissem_us` divergence on the lattice-vs-scheme comparison from the §B.3 Table 2 p21 `CLOSURE_NOFORN_CAVEATED` closure rule firing on the scheme path — correct CAPCO behavior, annotated per fixture. `sar_sort_key` relocated to `crates/ism/src/sar_sort.rs` (T069 readiness); re-export at `marque_ism::sar_sort_key` preserved. New `assert_impl_all!(CanonicalAttrs: Send, Sync)` compile-time check in `crates/ism/tests/send_sync.rs` (Constitution VI) — assertion target retargeted from `PageContext` to `CanonicalAttrs` during PR 4b-E review fix-up; PR 6c (T069) then retired `PageContext` entirely so `CanonicalAttrs` is the surviving foundational type the engine wraps in `Arc<Box<[_]>>` for cross-task dispatch. `actions/page_context.rs` (the `page_context_to_attrs` helper) deleted. `crates/ism/tests/rollup_golden.rs` + `proptest_page_context.rs` moved to `crates/capco/tests/` (lattice helpers live in `marque-capco`; `marque-ism` cannot dev-depend on it). `crates/capco/tests/scheme_equivalence.rs` deleted entirely per OQ-3 (absorbed into the renamed parity gate). Engine-crate touch authorization: Constitution VII §IV within-006 precedent (PR 4b-B Commit 2 / 4b-C Commit 5 / 4b-D.2 + 4b-D.3 — bugfix-class deletions in `marque-ism`, no new scheme adopted). See `docs/plans/2026-05-18-pr4b-E-page-context-deletion-plan.md` for the operative plan + `docs/plans/2026-05-18-pr4b-E-rust-preflight.md` for the risk register.
- PR #502 (2026-05-17, tracked as #456): Split `Lattice` into `JoinSemilattice + MeetSemilattice` halves in `marque-scheme`. `Lattice` and `BoundedLattice` are now blanket-impl markers (`impl<T: JoinSemilattice + MeetSemilattice> Lattice for T {}`). `DissemSet`, `JointSet`, and `SupersessionSet` in `marque-capco::lattice` implement only `JoinSemilattice` — these types have join-side observational state (`relido_observed_unanimous`, `Mixed`/`DisunityCollapse`, and the post-join supersession overlay respectively) that makes `meet` either undefined or non-idempotent. The type system now statically rejects `.meet()` calls on those types; five tests in `category_lattice_laws.rs` that verified partial-absorption behavior were removed (the type-system rejection is stronger than a runtime test). All other lattice types (`SciSet`, `SarSet`, `FgiSet`, `AeaSet`, `ClassificationLattice`, `NatoClassLattice`, `DeclassifyOnLattice`, `NatoDissemSet`, `RelToBlock`) implement both halves. `MarkingScheme::type Marking` bound relaxed to `JoinSemilattice`. No engine-semantics changes; no CAPCO `§` citations added. See `docs/plans/2026-05-01-lattice-design.md` section 12 for the addendum.
- PR 4b-C (006 T112, 2026-05-16): Pattern-B + Pattern-C declarative PageRewrite rows + imperative PageContext branch deletions (registered rule count unchanged at 39; catalog row count 14 → 23). **Pattern-C (Commit 3)** — 7 declarative rows on `CapcoScheme` covering the §H.6 / §H.8 / §H.9 classified-strip semantics: `capco/limdis-evicted-by-classified` (§H.9 p170), `capco/sbu-evicted-by-classified` (§H.9 p176), four UCNI rows declared in promote-before-strip order so the NOFORN-promotion predicate sees UCNI before the strip removes it (`capco/dod-ucni-promotes-noforn-when-classified` + `capco/dod-ucni-evicted-by-classified` + DOE mirrors at §H.6 p116 / p118), and `capco/fouo-evicted-by-classified` (§H.8 p134 classified sub-clause). The four UCNI rows fix the pre-PR-4b-C `expected_aea_markings` bug where classified-context UCNI stripping silently dropped the §H.6 NOFORN-promotion clause. **Pattern-B (Commit 4)** — 2 declarative rows per the PM-confirmed structural reading of §H.8 p134 verbatim ("FOUO is not conveyed in the banner line if the document is UNCLASSIFIED with FOUO and other dissemination control markings, excluding any FD&R markings"): `capco/classification-evicts-fouo` (classified-document sub-clause) + `capco/non-fdr-control-evicts-fouo` (UNCLASSIFIED-with-other-non-FD&R-control sub-clause; "non-FD&R" uses `Vocabulary::is_fdr_dissem`'s broad semantic which INCLUDES RELIDO — distinct from `is_fdr_dominator`'s narrow semantic). **Commit 1** also added four vocab sentinels (`TOK_PROPIN=143` / `TOK_FISA=144` / `TOK_RAWFISA=145` / `TOK_NNPI=146`) closing issue #407 for the NNPI sentinel and giving PROPIN / FISA / RAWFISA the predicate-resolution surface they need. **Commit 5** retired two imperative PageContext branches (FOUO Step 3 at `expected_dissem_us:594-599` + UCNI strip at `expected_aea_markings:1085-1093`); PageContext remains the transitional banner-validation driver until PR 4b-D wires `scheme.project(Scope::Page, ...)` as the production path. Engine-crate touch authorization: Constitution VII §IV within-006 precedent (PR 4b-B Commit 2 / §7.B; bugfix-class deletions in `marque-ism`). **Commit 6** adds 16 parity-gate fixtures driving the new `project_via_scheme` helper through `CapcoScheme::project(Scope::Page, ...)`. The two PR 4b-B G-1 (FOUO classified) + G-2 (UCNI classified) divergences are closed (renamed `*_pagecontext_and_lattice_both_keep_*_pending_pr_4b_d`); 4 documented divergences remain (G-3 pure-NATO + RELIDO+NF supersession + 2 pure-JOINT cases). Each §-citation re-verified against `crates/capco/docs/CAPCO-2016.md` at authorship per Constitution VIII. Pattern D deferred to PR 4b-D alongside the closure-operator runtime activation per lattice-design §3 (e). See `docs/plans/2026-05-16-pr4b-C-pattern-c-strip-rows-plan.md` for the operative plan (with the PM Corrections A/B/C addendum at the top).
- PR 4b-B (006 T112, 2026-05-15): rest-of-the-seven per-category Lattice impls in `marque-capco::lattice` plus two PageContext bugfixes and one new Warn rule. Lattice types: `ClassificationLattice` (bounded OrdMax over the five-level `Unclassified < Restricted < Confidential < Secret < TopSecret` chain, variant-preserving with deterministic same-variant payload union per C-7; §H.1 pp47-54 + §H.7 pp123-125 + §H.2 p55), `NatoClassLattice` (bounded OrdMax over `NU<NR<NC<NS<CTS`; §H.2 p55), `JointSet` (four-variant state — `Bottom` / `UnanimousProducers` / `DisunityCollapse` / `Mixed`; C-3 split `Mixed` out of `Bottom` so the absorbing JOINT+non-JOINT state keeps `join` associative; §H.3 p56 + §H.3 p57 + §H.7 p123), `DissemSet` (single-bag IC dissem with three supersession overlays — OC-USGOV / RELIDO-unanimity / NOFORN-dominates — and manual `Default` agreeing with `empty()` per C-8; §H.8 p136/p140/p145/pp155-156 + §D.2 Table 3), `NatoDissemSet` (trivial union; p41 reciprocity), `RelToBlock` (four-variant IntersectSet — `Bottom` / `Lattice{countries}` / `Empty` / `NofornSuperseded`; C-2 split `Empty` out of `Bottom` so the absorbing empty-intersection state keeps `join` associative; §H.8 pp150-151 + §D.2 Table 3 rows 9-13 + §H.9 p172/p174), `DeclassifyOnLattice` (MaxDate semilattice, no top; §H.6 p104). Two PageContext bugfixes landed atomically in Commit 2: OC-USGOV supersession (replaces unanimity-drop per §H.8 p136 + p140) and RELIDO observed-unanimity at banner roll-up (§H.8 pp155-156; Layer 1 only — Layer 2 FD&R inference defers to PR 4b-D). New Warn rule `W004 joint-disunity-collapse-to-FGI` per §H.3 p57 + §H.7 p123 (CV-4 PR 4b-B 8th-pass updated from §H.3 p56 — the migration trigger lives on p57's "Derivative Use" bullets; §H.3 p56 still grounds the JOINT classification grammar separately) (registered rule count 38 → 39). `CapcoMarking::join_via_lattice` composes the lattice types component-wise, with G-8 routing cross-axis NOFORN injection through `DissemSet::with_noforn_injected` so the §H.8 p145 NOFORN-dominates overlay strips dominated controls; production `JoinSemilattice::join` still delegates to PageContext until PR 4b-D flips the hot path. Parity gate at `crates/capco/tests/page_context_lattice_parity.rs` (51 `#[test]` fixtures — 45 byte-identity + 6 documented divergences, each carrying a `§X.Y pNN` citation; enumerated in `crates/capco/CAPCO-CONTEXT.md` §3). `lint_10kb` bench measured 594-613µs (well under the 900µs gate; lattice work not on hot path). See `docs/plans/2026-05-15-pr4b-B-lattice-impls-rest-plan.md` + `docs/plans/2026-05-01-lattice-design.md` §11 for the operative plan + addendum.
- PR 9a (006 T131 + T135a, 2026-05-14): within-category Separator span emission + token canonicalization rules. T131 (Commit 1, #106): parser emits `TokenKind::Separator` spans for within-category `/` byte sequences (disambiguated from between-category `//` by `text` field — `"/"` vs `"//"`), with an engineering relaxation that consumes adjacent ASCII whitespace into the Separator span (Marque tolerance, NOT a §A.6-permitted variant — §A.6 p16 forbids interjected whitespace for SAP/AEA/dissem/non-IC dissem alike). SAR keeps a strict 1-byte separator span. T135a Commits 2–5: SCI long-form deprecated-token recognizer (Commit 2) + `DeprecatedSciLongFormRule` walker (Commit 3, E065) + bare HCS at C / bare HCS at S/TS suggest / bare RSV rules (Commit 4, E061 + E062 + E063 per §H.4 p62 + p70) + EYES / EYES ONLY → REL TO conversion (Commit 5, E064 per §H.8 p157-158, text_correction at compound block span, cross-axis migration is text_correction-route, not FixIntent — `ReplacementIntent::FactAdd`/`FactRemove`/`Recanonicalize` are single-axis-scoped). Net rule count change: 31 → 36 (5 added; no retirements in PR 9a — Stage 4 walker collapses land in subsequent PRs).
- `FeatureId::PrecedingFixPenalty` retired (PM decision, 2026-05-14): the mechanism was misunderstanding-derived (the user's original concern was a decoder-specific confidence-loop pathology, not a generalized cross-pass penalty) and the path was independently confirmed dead code today (`pass1.applied` is always empty under current `Phase::Localized` rules, which all emit via `Diagnostic::text_correction`). The variant, engine-applied multiplicative `rule` reduction, `FeatureContribution` audit-trace entry, `PRECEDING_FIX_PENALTY_DELTA` constant, and watchdog test suite are removed in PR 7c. `RuleContext<'a>` + `pre_pass_1_attrs` field + pre-pass-1 attrs cache + FR-023 disambiguation + I-18 overlap demotion stay (load-bearing for the two-pass model, independent of the penalty). The decoder confidence-loop concern remains an open research item, deferred to a future statistical design pass (see D-7.22).
- PR 3c.2 carved out + `marque-1.0` deferral (PM decision, 2026-05-14): the `marque-mvp-3 → marque-1.0` audit-schema cutover originally bundled into PR 3c (per the original FR-035) deferred to a dedicated **PR 3c.2** that lands the four structural commitments atomically (Canonical wired into audit emit, BLAKE3 audit-record digesting, closed `MessageTemplate` JSON serialization, `from_parsed_unchecked` adapter deletion). A 2026-05-14 inventory across four parallel Explore agents confirmed all four commitments are fully reserved slots with no production wire-up: `blake3` not in any Cargo.toml; `AppliedFix` has no digest field; `AuditRecordJsonV3` emits no `message` field; 27 surviving `from_parsed_unchecked` call sites. PR 7 series stays on `marque-mvp-3` (originally because PR 7c's planned `FeatureId::PrecedingFixPenalty` variant would have filled a reserved slot; that mechanism was retired 2026-05-14 per D-7.22, so PR 7c neither fills nor needs the slot — but the PR-7-stays-on-mvp-3 stance survives). The `(scheme, predicate-id)` 2-tuple `RuleId` form is **NOT** part of PR 3c.2's scope — it defers further still, to its own post-PR-10 PR per FR-049 (stability freeze begins at PR 10 merge). Plan-of-record amendments: spec FR-035 (revised) + new FR-035a; consolidated plan §4 table (new PR 3c.2 row, amended PR 7 row, two-stage audit-schema cutover table) + §10.2 (revised cutover composition); `contracts/audit-record.md` §0 + §1 redirected to PR 3c.2; PM decisions D-7.18 (defer) / D-7.19 (engine-applied `PrecedingFixPenalty`, not E003-applied — E003 was retired in PR 3b.F → E060) / D-7.20 / D-7.21. PR 7c proceeds without the bump.
- PR 3b umbrella closeout (T027 / T028 / T029, 2026-05-08): bookkeeping commit completing the PR 3b umbrella after the six functional sub-PRs (3b.A #319 / 3b.B #320 / 3b.C #321 / 3b.D #324 / 3b.E #326 / 3b.F #327) merged to `staging`. Zero rule-logic edits and zero engine-crate edits (Constitution VII §IV scheme-adoption restriction). T027: umbrella reviewer attestation aggregated into the PR description — D13 single-§-citation discipline (per-row in each declarative catalog), ≤3 branches per `impl Rule` body, and net-rule-delta math (59 → 47 across the six sub-moves). T028: new exact-rule-ID-set pin at `crates/capco/tests/post_3b_registration_pin.rs` complementing the existing count pin at `corpus_parity.rs:170-194`. The count pin alone catches "rule was added/removed"; the new exact-set pin catches "rule X renamed to rule Y at the same count" and "rule X deleted, rule Z added at the same count" — the drift classes the umbrella's structural commitment to a closed 47-rule set actually depends on. T029: new `pr-3b-corpus-regression` CI job mirroring T025's body, prefix-match-filtered to `refactor-006-pr-3b*` branches (covers the umbrella + all six sub-PR branches + the closeout branch). Final registered count: 47 (above the earlier ~38–44 Stage-1 estimate by 3 rules; D13 numeric band retired 2026-05-07 per the addendum precisely because the literal sub-move retirements were known to land outside any numeric band by construction — the qualitative gate "stayed within the sub-move's authorized primitive scope" is satisfied). End-state target ~10 surviving rules across all four stages remains binding; heavy lifting toward that target lands in Stage 3 (PR 4) and Stage 4 (PR 5+). See `docs/plans/2026-05-08-pr3b-closeout-T027-T028-T029-plan.md`.
- PR 3b.F (T026f) — Non-canonical input walker (2026-05-08): collapsed four hand-written ordering-validation rules — `CountryCodeOrderingRule` (E020, REL TO + JOINT alpha), `SigmaValidationRule` (E023, AEA SIGMA numeric sort), `SarProgramOrderRule` (E028, SAR program ascending), `SciCompartmentOrderRule` (E033, SCI compartment + sub-compartment numeric-then-alpha) — into a single `DeclarativeNonCanonicalInputRule` walker (rule ID `E060`) dispatching over a 5-row private `&'static [NonCanonicalRow]` catalog (`NON_CANONICAL_CATALOG`) inside `crates/capco/src/rules_declarative.rs`. The catalog is **structurally different** from PR 3b.D / 3b.E walkers: it is NOT a `Constraint::Custom` catalog on `CapcoScheme` — these are renderer-canonical-form concerns (per `marque-applied.md` §3.6 + §3.10 Move 7) absorbed by `MarkingScheme::render_canonical` once the renderer trait surface lands in PR 5+ (Stage 4 of the engine refactor); the walker retires cleanly when that lands. Per-row §-citations: REL TO USA-first alpha (§H.8 p150-151), JOINT alpha (§H.3 p56), AEA SIGMA numeric sort (§H.6 p108), SAR program ascending alpha (§H.5 p99), SCI compartment + sub-compartment numeric-then-alpha (§H.4 p61). Diagnostics emit with `Diagnostic.rule = "E060"`; per-row identification flows via the diagnostic message text + the `Diagnostic.citation` field (which is preserved verbatim from the retired rules so existing audit-stream consumers continue to work). Per-row severity preserved: `Severity::Fix` for rows 1-4 (REL TO/JOINT/SIGMA/SAR), `Severity::Error` for row 5 (SCI). Walker `default_severity()` = `Severity::Error` (strictest-of-rows precedent from PR 3b.A banner walker; OQ-3 PM-resolved). The legacy E020/E023/E028/E033 IDs are intentionally NOT preserved as severity-config aliases (per `feedback_pre_users_no_deprecation_phasing.md`: marque is pre-users; rewrite freely). One R-1 lex-tiebreaker behavior change documented in `tests/rel_to_invariants.rs`: pre-rename E020 won the FR-016 tiebreaker against E052 (`'E020' < 'E052'`) and produced canonical output in one fix pass; post-rename E052 wins (`'E052' < 'E060'`) and produces dedup-only output, with canonical reached on the second pass via E060. The fixed point is reached in ≤2 passes (idempotent thereafter). Net delta: 4 retired + 1 walker added = net −3 (registered rule count 50 → 47). See `docs/plans/2026-05-08-pr3b-F-non-canonical-input-walker-plan.md`.
- PR 3b.E (T026e) — SCI per-system catalog walker (2026-05-08): collapsed the 10 hand-written rules in `crates/capco/src/rules_sci_per_system.rs` (E042–E051; HCS-O / HCS-P / SI-G / TK companion-required + forbid-companion + range-ceiling rules) into a single `DeclarativeSciPerSystemRule` walker (rule ID `E059`) dispatching over a 5-row `Constraint::Custom("sci-per-system/...", ...)` catalog on `CapcoScheme` at CAPCO-2016 §H.4 family granularity: HCS-O companions (§H.4 p64), HCS-P NOFORN (§H.4 p66), HCS-P sub-compartment companions (§H.4 p68), SI-G companions (§H.4 p80), TK-{BLFH,IDIT,KAND} NOFORN (§H.4 p87 + p91 + p95). The class-floor portions of E044/E045/E046/E048/E049/E050 are absorbed by PR 3b.D's class-floor catalog rows (`class-floor/HCS-comp-sub`, `class-floor/HCS-comp`, `class-floor/SI-comp`, `class-floor/RSV-comp`, `class-floor/TK`, `class-floor/TK-BLFH`); no class-floor rows are added in PR 3b.E. Diagnostics emit with `Diagnostic.rule = "E059"`; per-row identification flows via the catalog row's `name` field. Severity-overridable per-walker via `[rules] E059 = "off|warn|error|..."`. Walker uses the same 3-layer optimization shape as PR 3b.D (axis-presence early-out + direct row dispatch + DRY emit helper). `crates/capco/src/rules_sci_per_system.rs` deleted. Two intentional regressions documented (PM-approved): (1) the actionable `S → TS` class-upgrade fix from E044/E046/E050 is lost — class promotion is FixIntent territory under PR 3c+; (2) the Warn-no-fix ambiguity-guidance text from E045/E048/E049 is lost as severity escalates Warn → Error per PR D's class-floor diagnostic. Net delta: 10 retired + 1 walker added = net −9 (registered rule count 59 → 50). See `docs/plans/2026-05-08-pr3b-E-sci-per-system-collapse-plan.md`.

Older entries archived to [`docs/refactor-006/recent-changes-archive.md`](docs/refactor-006/recent-changes-archive.md).
