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

**SCI canonical storage.** `SciSet` (in `marque_capco::lattice`, the lattice form of SCI state) is the canonical page-context storage: it implements `JoinSemilattice + MeetSemilattice` (i.e. `Lattice` via blanket impl), round-trips with `[SciMarking]` via `SciSet::from_markings` / `SciSet::to_markings`, and composes through `CapcoScheme::project(Scope::Page, ...)`. `SciSet` (and `SarSet`) deliberately do **not** implement `BoundedLattice` — SCI control systems and SAR program identifiers are both agency-extensible open sets, so no lawful finite `top` exists. Use `SciSet::empty()` / `SciSet::default()` when you need the lattice bottom. `IsmAttributes::sci_controls` stays populated for rules that currently read it, but is a compatibility view scheduled for removal once no rule references it. New rules that need compartment / sub-compartment semantics should read `sci_markings` or construct an `SciSet`; rules that just need "which bare control systems appear" can stay on `sci_controls` until the migration closes.

Banner roll-up for SCI uses `PageContext::expected_sci_markings()`, which unions compartments and sub-compartments across all portions on the page and sorts per §A.6 p15 (numeric first, alpha after). Authority: CAPCO-2016 §A.6 (grammar, canonical example p16) + §H.4 (per-system banner precedence).

**NATO SAPs.** `SciControlSystem::NatoSap(NatoSap)` is the canonical home for `BOHEMIA` and `BALK` (CAPCO-2016 §G.2 p40 + §H.7 p127). They render standalone (no `SAR-` prefix) in the SCI block position — e.g. `(//CTS//BOHEMIA)` or `(//CTS//BALK/BOHEMIA)`. BALK sorts before BOHEMIA alphabetically per §H.7 p127 worked example. NATO SAPs are CAPCO-only (no ODNI ISM CVE entry) — the third `SciControlSystem` variant keeps `Published(SciControlBare)` ODNI-faithful and `Custom(SmolStr)` reserved for agency-allocated `[A-Z0-9]{2,5}` identifiers per §A.6 p15. Legacy `CTS-B` / `CTS-BALK` text and the banner-form equivalents canonicalize through the strict parser into bare CTS class + SCI NatoSap companion; a recanonicalization rule emits a Recanonicalize FixIntent so the source text is re-rendered to the canonical multi-block form.

### SAR (Special Access Required)

SAR (Special Access Required) markings are modeled structurally, not as a CVE-derived enum. The ODNI public `CVEnumISMSAR.xml` is empty because SAR program identifiers are agency-assigned codewords not centrally registered. `marque-ism::SarMarking` captures the full hierarchy — programs, compartments, sub-compartments — parsed by a hand-written subparser in `marque-core` (see `parse_sar_category`). The SAR rules validate syntax, ordering, classification constraints, and banner roll-up per CAPCO-2016 §H.5.

### ATOMAL (NATO AEA)

ATOMAL is a NATO AEA marking — Atomic Energy Act information shared with NATO+UK under bilateral §123/§144 sharing agreements. Per CAPCO-2016 §G.2 p40 (Table 5: ARH by Registered Marking) ATOMAL is a registered standalone control marking; the §H.7 p122 worked example (`SECRET//RD/ATOMAL//FGI NATO//NOFORN`) places ATOMAL in the AEA axis alongside RD/FRD/TFNI — **not** as a NATO classification portion-suffix.

`AeaMarking::Atomal(AtomalBlock)` is the canonical home. The block is empty (no registered sub-markings) but mirrors `RdBlock`/`FrdBlock` so a future CAPCO grammar extension is a planned migration. The strict parser canonicalizes legacy compound text (`CTSA`, `CTS-A`, `NSAT`, `NS-A`, `NCA`, `NC-A`, banner-form `COSMIC TOP SECRET ATOMAL`, etc.) into bare NATO class + AEA ATOMAL companion at parse time; a recanonicalization rule emits a Recanonicalize FixIntent that re-renders to the canonical multi-block form (`(//CTS//ATOMAL)`, etc.) per the §G.2 p40 Table 5 registration. Per project memory `remark-on-derivative-use-is-marque-autofix`, Marque automates the canonical re-marking the manual permits doing by hand. The legacy fused `NatoClassification::*Atomal` variants (`NatoConfidentialAtomal`, `NatoSecretAtomal`, `CosmicTopSecretAtomal`) and the corresponding `*Bohemia` / `*Balk` variants were retired.

### Key Types

- `IsmAttributes` (`marque-ism`) — the pivot type. Every source format normalizes to this struct before rule validation. Fields use `Box<[T]>` (not `Vec`) to avoid over-allocation. Field types (`SciControl`, `DissemControl`, etc.) are generated enums from ODNI CVE XML.
- `Span` (`marque-ism`) — byte offset range into the original source buffer. Never copies content; spans reference the original `&[u8]`.
- `Diagnostic` (`marque-rules`) — a violation with `rule`, `severity`, `span`, `message`, `citation`, and optional `FixProposal`.
- `FixProposal` (`marque-rules`) — `span` + `replacement` + `confidence` + `source` + `migration_ref`. Pure data; no timestamp or classifier identity. Suggestions until promoted by `Engine::fix`.
- `AppliedFix` (`marque-rules`) — a promoted `FixProposal` with `timestamp`, `classifier_id`, `dry_run`, `input`. Constructed only by `Engine::fix`. Serves as the audit record.
- `RuleContext` (`marque-rules`) — position context passed to rules alongside attributes (`MarkingType`, `Zone`, `DocumentPosition`). Also carries an optional `Arc<PageContext>` for banner/CAB candidates so banner-validation rules can compare the observed banner against the composite expected from all preceding portions.
- `PageContext` (`marque-ism`) — page-level aggregation of portion markings: `max()` for classification, union for SCI/SAR/dissem controls, intersection (with NOFORN supersession) for `REL TO`, max-date for `declassify_on`. The engine builds this incrementally during `lint()` and hands banner/CAB rules an `Arc<PageContext>` via `RuleContext`.
- `Recognizer<S>` (trait in `marque-scheme`; impls in `marque-engine`) — pluggable first stage of the engine. Turns a byte slice + `ParseContext` into `Parsed<S::Marking>`. The trait lives in `marque_scheme::recognizer`; the three shipped concrete implementations are `marque_engine::StrictRecognizer` (zero-FP header-only, the existing structural parser), `marque_engine::DecoderRecognizer` (probabilistic / bag-of-tokens), and `marque_engine::StrictOrDecoderRecognizer` (the strict-first / decoder-fallback dispatcher installed by default in `Engine::new`). Callers that need strict-only dispatch (the interactive-latency benchmark, tests asserting strict behavior) install `StrictRecognizer` explicitly via `Engine::with_recognizer`. Trait is domain-neutral: depends only on the scheme's `Marking` and the `Parsed` / `Candidate` / `EvidenceFeature` primitives in `marque_scheme::ambiguity`.
- `Vocabulary<S>` (`marque-scheme`) — per-token metadata surface (authority, owner/producer, point of contact, deprecation, URN, schema version, portion/banner forms). Returns `&'static` data, zero runtime allocation. Implemented for `CapcoScheme` from build-time-generated tables; rules read this instead of hardcoding metadata.
- `Codec<S>` (`marque-scheme`) — pinned trait surface for grammar serialization (encode/decode round-trip). No production/library impls ship in-tree yet; only test stubs exist. XML and JSON are planned. `Codec::decode` returns `Parsed<S::Marking>` so ambiguity preserves through the codec layer.
- `Recognition` + `FeatureId` (`marque-rules`) — audit-provenance payload attached to every `FixProposal` (renamed from `Confidence` at PR B). Carries a single `recognition` axis, optional `runner_up_ratio`, and a closed list of named `FeatureId` contributions. `Recognition::combined()` is a thin accessor returning `recognition` (was `recognition × rule` pre-PR-B). `f32` at the audit boundary (`f64` internally in the decoder). Adding a `FeatureId` variant requires a coordinated bump of `MARQUE_AUDIT_SCHEMA`.
- Topological scheduler (`marque_engine::scheduler`) — runs Kahn's algorithm over `PageRewrite::reads` / `writes` once at `Engine::new` to produce a deterministic rewrite order (writers before readers). Cycles fail with `EngineConstructionError::RewriteCycle`; `Custom` rewrites with empty axis annotations fail with `UnannotatedCustomAxes`. The cached order drives per-document evaluation without re-sorting.

### Architectural Invariants (do not bypass)

These contracts are enforced by convention and code review, not by the type system. A new crate or refactor that breaks one of them silently compromises the correctness or compliance guarantees of the tool.

- **`AppliedFix::__engine_promote` is engine-only in production code.** The constructor is `pub #[doc(hidden)]` because `marque-rules` is a dependency of `marque-engine` (not the other way around), so there is no way to seal it inside the engine crate at the visibility level. In **production code** (anything reachable from a `cfg(not(test))` build) it **must only be called from `Engine::fix_inner`**. Calling it from a rule crate, CLI binary, or downstream consumer bypasses the confidence-threshold gate, the fix-ordering sort, and the overlap guard, and injects arbitrary entries into the audit log. The audit log is the compliance output — arbitrary injection is a data-integrity failure, not just a bug. If you are writing a crate that needs to produce fixes, produce `FixProposal` values and let `Engine` promote them. **Test-fixture carve-out**: `#[cfg(test)]` modules, `tests/` integration files, and `dev-dependencies`-gated test-utility crates MAY call `__engine_promote` to fabricate synthetic `AppliedFix` fixtures for unit-testing audit emitters, sentinel checks, and renderers — scoped per Constitution V Principle V (test-fixture construction only, never commingled with engine output, never `cfg(not(test))`-reachable). Each test call site should carry a comment naming the carve-out so a future reviewer doesn't have to re-derive the policy. See the doc comment on `AppliedFix::__engine_promote` for the full three-constraint definition.
- **`FixProposal` is pure data.** No timestamps, no classifier identity, no runtime context. Rule crates construct it; the engine snapshots runtime state into `AppliedFix` at promotion time. Keeping `FixProposal` pure is what lets tests snapshot rule output without a clock or user identity.
- **`RuleContext.zone` and `RuleContext.position` are `Option`-typed.** Both fields are `Option<Zone>` and `Option<DocumentPosition>`; the engine populates them as `None` until a structural scanner pass can prove a value (header vs footer detection, document position from extracted-document metadata). Rules that read either field MUST handle `None`. An earlier hardcoded `Body` default was a silent lie — making the type carry the uncertainty makes it impossible to misuse.
- **`PageContext` resets at scanner-emitted page-break candidates.** The scanner emits `MarkingType::PageBreak` (form-feed `\f` and `\n\n\n+` heuristic). The engine resets its `PageContext` accumulator BEFORE attempting to parse the page-break candidate, so a corrupted or malformed candidate cannot block the reset. Banner/CAB rules on a new page see only that page's portions, not the whole document. Note: the scanner heuristic is conservative — `\n\n` (a normal paragraph break) does NOT trip the reset.
- **`Severity::Off` is a non-firing state, not a suppression.** A rule configured at `Off` is skipped in the rule loop, so no diagnostic is produced: an `Off`-severity diagnostic is unrepresentable.

### Batch Processing

`BatchEngine` wraps `Engine` behind `Arc` and uses `marque-utils::ConcurrencyController` for row + byte semaphore backpressure. CPU-bound work goes to `tokio::task::spawn_blocking`. Results stream out in **completion order**, not submission order — correlate via the echoed `id`.

### Incremental Cache (planned for v0.2)

LMDB (`heed` crate) at `.marque/cache.lmdb`. Cache key = `blake3(content) ++ schema_version ++ config_hash`. Only `LintResult` is cached, never `FixResult`. Opt-in via `--cache` flag. Behind `cache` feature flag in `marque-engine`.

## Configuration

`.marque.toml` (committed, project/org policy):
```toml
[capco]
version = "2023.1"

[rules]
# Wire-string `<scheme>:<predicate_id>` keys.
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
3. Rule IDs are 2-tuples `RuleId::new("<scheme>", "<surface>.<category>.<predicate>")`. For CAPCO rules `<scheme>` is `"capco"`; `<surface>` ∈ `{ banner, portion, page, marking, closure }`; `<category>` matches the lattice axis (`classification | sci | sar | dissem | fgi | nato | aea | declassification | fouo | banner-rollup | metadata`); `<predicate>` is descriptive English-with-hyphens. The default-severity tier is encoded via `Severity::Error | Severity::Warn | Severity::Suggest | Severity::Info` on the `Rule` trait, not via an ID prefix. `docs/refactor-006/legacy-rule-id-map.md` maps the older `E### / W### / S### / C###` IDs to their current wire strings if you encounter one.
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

## Stable API Surface

The following surfaces are committed. Changing any of them requires a
coordinated audit-schema bump — `marque-3.1` for additive changes,
`marque-4.0` for breaking ones. The current audit schema is
`marque-3.0` (PR B retired the two-axis `Confidence` payload and the
unused `region` field in favor of a single `Recognition` axis;
the `marque-2.0 → marque-3.0` cutover is the live inflection).

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
- **`RuleId` 2-tuple form** — the
  `(scheme: &'static str, predicate_id: &'static str)` shape with
  the canonical `<scheme>:<predicate_id>` wire string produced by
  `RuleId::Display`. Predicate IDs follow
  `<surface>.<category>.<predicate>` where `<surface>` ∈
  `{ banner, portion, page, marking, closure }` (the `closure`
  surface keeps closure-operator inferences from conflating with
  strict page-banner rules at the predicate level). Reserved
  schemes: `"engine"` (synthetic engine-minted diagnostics) and
  `"test"` (test fixtures); neither is a valid `MarkingScheme`
  registration target. Engine sentinels:
  `("engine", "recognition.decoder-recognized")` and
  `("engine", "fix.reparse-failed")`.
  `docs/refactor-006/legacy-rule-id-map.md` maps the older
  flat-string IDs to their current wire strings.
- **Typed `Citation`** in `marque-scheme` — `Citation::new` plus
  ergonomic helpers (`capco` / `capco_section` /
  `capco_table`); `SectionRef` + `SectionLetter` + `PageNumber`;
  `AuthoritativeSource` enum with `Capco2016` / `Config` /
  `EngineInternal` variants.
- **`AppliedFix<S>` audit-record envelope** — sealed
  `__engine_promote` constructor (Constitution V Principle V); the
  `marque-3.0` JSON wire format (`MARQUE_AUDIT_SCHEMA = "marque-3.0"`);
  structured 2-tuple `"rule"` field; BLAKE3 digest field; closed
  `MessageTemplate` JSON projection; single-axis `Recognition`
  confidence sub-object (post-PR-B `recognition` / `combined` /
  `runner_up_ratio` / `features` only — no `rule`, no `region`).
- **Audit content-ignorance invariant** — the canary
  scan at `crates/engine/tests/audit_g13_canary.rs` is the
  type-system + corpus-regression form of the invariant. Adding
  a free-form string surface to any audit-side type breaks it.

**Not frozen** (open scope):

- **v0.2 LMDB incremental cache** (`crates/engine` `cache` feature)
  — the `LintResult` cache surface is a separable v0.2 line, not
  part of this stable surface.
- **`marque-extract` format-extraction backend** — Kreuzberg
  integration is gated on a licensing decision; the scaffolded
  `Extractor` / `ExtractedDocument` / `ExtractionOptions` /
  `MetadataReport` surface is frozen, but the backend is open.
- **Server auth + structured logging middleware** in
  `marque-server` — Tower-layer surface is frozen; specific
  middleware implementations are still open.

Upstream-source bumps: pinned via
`[package.metadata.marque]` in `crates/ism/Cargo.toml`
(`ism-schema-version` / `ism-data-version` /
`ismcat-tetra-version`) and via the matching `[build-dependencies]`
versions on `ism` / `ism-ismcat`. ODNI schema revisions are
deliberate, reviewed migrations per Constitution VIII —
re-verify every cited authority against the new source before
the migration lands.

## Current Status

MVP complete. Full lint → fix → audit pipeline for raw text with **32 registered CAPCO rules** (issues #261/#250/#251/#501/#545/#677 and PR #578). The exact set is authoritatively gated by the registration pin in `crates/capco/tests/post_3b_registration_pin.rs` against the 2-tuple wire strings; `crates/capco/README.md` provides the narrative rule inventory, and `docs/refactor-006/legacy-rule-id-map.md` decodes older `E### / W### / S### / C###` IDs. CLI (`check`, `fix`) and WASM (`lint`, `fix`) produce byte-identical NDJSON diagnostics. Configurable severity overrides, corrections map, and confidence thresholds. Batch processing via `BatchEngine` with concurrency control. Criterion benchmarks measure interactive latency p95 ≤ 16 ms on 10 KB single-portion inputs and multi-page projection + two-pass overhead; the `fix_throughput` linear-scaling gate is active (R² = 0.994 measured; O(N²) accumulation fixed in PR #674, closing #306). Corpus accuracy harness enforces ≥ 95% lint and fix accuracy per-rule against the invalid-fixtures corpus. `cargo-fuzz` target exercises `Engine::lint` on arbitrary `&[u8]`.

**Not yet built**: `marque-extract` is scaffolded (workspace member with `Extractor`, `ExtractedDocument`, `ExtractionOptions`, `MetadataReport` surface) but the Kreuzberg backend is stubbed — `crates/extract/src/extractor.rs` reads raw text only and `crates/extract/Cargo.toml` keeps `kreuzberg` commented out pending a licensing decision. Also outstanding: `metadata` CLI subcommand, incremental LMDB cache (v0.2), server auth middleware.

## Active Technologies
- Rust 1.85+ (edition 2024) — `rust-version = "1.85"` in workspace `Cargo.toml`; constitution Tech Stack pins the floor
- `memchr` 2 — SIMD candidate detection (Phase 1 scanner)
- `aho-corasick` 1 — token matching (Phase 2 parser) + pre-scanner text corrections; used on both native and WASM. The constitution Tech Stack reserves `daachorse` for the WASM target as a future binary-size optimization, not yet wired
- `quick-xml` — build-time ODNI XSD/Schematron parsing
- `serde` + `serde_json` — build-time JSON codepath for per-term vocabulary data (runtime deserialization not required; data is emitted as `&'static` const tables by `build.rs`)
- `phf` — compile-time replacement lookup (perfect hash)
- `criterion` 0.8 — benchmarking (interactive-latency and linear-scaling gates)
- `libfuzzer-sys` 0.4 — fuzz target (requires nightly, not CI-gated)
- `tokio` (async runtime, `BatchEngine`), `axum` + `tower` (server middleware), `static_assertions` (compile-time `Send + Sync` checks), `blake3` (audit-record digests), `wasm-pack` (WASM target), `secrecy` (zeroize/grepable call sites on all content), `zeroize` (securely dropping internal buffers)
- No runtime cache on the hot path. Build-time cache via Cargo `OUT_DIR`. The planned LMDB `LintResult` cache is a future v0.2 line.

**Build-time inputs**: ODNI XML pulled from the `ism` and `ism-ismcat` build-deps (vendored in [`marquetools/ism-data`](https://github.com/marquetools/ism-data) at snapshot `20230609.0.0`, package label `ISM-v2022-DEC`); `crates/capco/docs/CAPCO-2016.md` (authoritative manual, vendored); `crates/capco/corpus/` (corpus-derived priors produced by `tools/corpus-analysis/`, regenerated when the corpus changes). **Test inputs**: `tests/fixtures/mangled/` (≥200 labeled mangled cases generated from Enron-corpus high-confidence markings; generator checked in, artifact regenerable).

**Audit schema**: `MARQUE_AUDIT_SCHEMA` env var pinned at build time, validated against the closed accept-list `["marque-3.0"]` and defaulting to `"marque-3.0"`. The audit envelope carries a structural `proposal: FixIntent | TextCorrection` sub-object (no free-form content, keeping audit records content-ignorant), a BLAKE3 digest, a closed `MessageTemplate` JSON projection, and the single-axis `Recognition` confidence sub-object (post-PR-B). Re-exported as `marque_engine::AUDIT_SCHEMA_VERSION`. A single binary emits exactly one schema.

## Recent Changes
