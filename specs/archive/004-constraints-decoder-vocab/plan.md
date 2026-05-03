# Implementation Plan: Declarative Rule Expression, Probabilistic Recovery, and Full Vocabulary Metadata (Phases C–E)

**Branch**: `004-constraints-decoder-vocab` | **Date**: 2026-04-20 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/004-constraints-decoder-vocab/spec.md`
**Primary source (technical detail):** [`docs/plans/2026-04-19-recursive-lattice-and-decoder.md`](../../../docs/plans/2026-04-19-recursive-lattice-and-decoder.md) §§4–7a, 12 (Phases C, D, E)

## Summary

Promote three foundations of the `marque` engine simultaneously, in one branch, because each depends on the other two landing together:

1. **Declarative constraint + page-rewrite surface (Phase C).** Migrate ~15 of the 39 hand-written CAPCO rule implementations to shared `Constraint` / `PageRewrite` data evaluated by a single evaluator in `marque-scheme`. Requires the axis-annotated `PageRewrite` refactor (`reads`/`writes` fields) and an engine-side topological scheduler that rejects cycles and unannotated custom rewrites at `Engine::new`.
2. **Probabilistic recognizer with audit provenance (Phase D).** Wrap the current strict parser behind a `Recognizer` trait; add a `DecoderRecognizer` that performs bag-of-tokens Bayesian decoding with a bounded candidate generator (K=8 per grammar template). Bump the audit record schema (`marque-mvp-1` → `marque-mvp-2`) so every auto-applied fix carries recognition confidence, runner-up ratio, and feature-contribution trace. Corpus-override is CLI-only — rejected by the server and compile-time-excluded from WASM (T3 enforcement).
3. **Vocabulary metadata surface + codec trait (Phase E).** Stop flattening the ODNI ISM metadata (authority, owner/producer, POC, deprecation, URN, schema version). Consume the schema through both a JSON codepath (per-term data) and the existing XML codepath (XSD + Schematron predicates). Publish a `Codec<S>` trait surface with zero implementations so Phase G can round-trip XML/JSON without further trait evolution. Remove the factually-wrong `FOUO → CUI` migration table entry (§14).

The combined surface turns marque from "one tool with 39 hardcoded CAPCO rules and a token-id lookup table" into "an engine where any grammar is declarative data flowing from its authoritative source, with a probabilistic fallback for real-world messy inputs and full metadata fidelity back to ODNI."

All three phases are gated on the existing ≥95% per-rule accuracy floor and byte-identical CAPCO corpus diagnostics across any refactor; neither gate moves.

## Technical Context

**Language/Version**: Rust ≥ 1.85 (edition 2024). Pinned by Constitution Tech Stack.
**Primary Dependencies**: `memchr` 2 (scanner), `aho-corasick` 1 (token matching; `daachorse` on WASM per Tech Stack), `quick-xml` (build-time ODNI XSD/Schematron parsing, already present), `serde` + `serde_json` (build-time JSON codepath for per-term vocabulary data; runtime deserialization not required — data is emitted as Rust const tables by `build.rs`), `phf` (compile-time replacement lookup, already present). No new runtime crates introduced by Phase D's decoder — log-posterior scoring uses `f64` and Rust standard ops. Corpus-derived priors baked in as `&'static [T]` tables at build time.
**Storage**: None at runtime. Build-time inputs: `crates/ism/schemas/ISM-v2022-DEC/` (ODNI XML, vendored), `crates/capco/docs/CAPCO-2016.md` (authoritative manual, vendored), `crates/capco/corpus/` (corpus-derived priors produced by `tools/corpus-analysis/`, regenerated when the corpus changes). Test inputs: `tests/fixtures/mangled/` (≥200 labeled mangled cases generated from Enron-corpus high-confidence markings; generator checked in, artifact regenerable).
**Testing**: `cargo test` (unit + integration across workspace), Criterion benchmarks under `benches/` (SC-001 strict p95 ≤16 ms; SC-002 with-mangled-region p95 ≤18 ms), corpus accuracy harness (SC-003 ≥95% per-rule accuracy floor, byte-identical diagnostic equivalence before/after Phase C migration), `cargo-fuzz` target for `Engine::lint` (already present). WASM parity test against the same corpus subset via `wasm-pack test`.
**Target Platform**: Linux server (primary), macOS + Windows for dev, WASM (browser, via `wasm-pack`) as a first-class target. `marque-extract` (non-WASM only) is unaffected by this work.
**Project Type**: Rust cargo workspace; compiler / rule-engine with multiple integration binaries (CLI `marque`, `marque-server` axum service, `marque-wasm` web-worker artifact). Existing crate graph preserved — this work lands in `marque-scheme` (trait surface additions), `marque-capco` (Constraint/PageRewrite data entries + `DecoderRecognizer` + full `Vocabulary` impl), `marque-ism` (dual JSON+XML build.rs codepath + `TokenMetadataFull` tables), and `marque-engine` (`Recognizer` trait + `lint_inner` rewire + scheduler construction). Per FR-022 / Constitution Principle IV, any engine-side gap discovered during this work is closed in a separate predecessor PR; this branch does not conflate grammar-adoption work with engine edits.
**Performance Goals**: p95 ≤ 16 ms strict-path lint on 10 KB input (SC-001; no regression from pre-Phase-C baseline). p95 ≤ 18 ms lint on 10 KB input with one mangled region requiring decoder recovery (SC-002). Linear batch throughput scaling preserved (existing SC-005). Mangled-marking resolution rate ≥ 85% at aggregate confidence ≥ 0.85 on a ≥200-case labeled fixture (SC-004).
**Constraints**: Strict-path remains zero-allocation on hot path (Constitution II). Runtime corpus override is CLI-only; the server binary rejects override on HTTP requests; the WASM artifact compiles out the override codepath entirely (Constitution III + FR-013 + T3). Audit records stay content-ignorant — no document text, metadata field values, or subject-claim free-form strings cross the boundary (Constitution V + FR-012). Rule and `Recognizer` implementations stay `Send + Sync` with no mutable global state (Constitution VI + FR-023). Vocabulary queries allocate zero runtime memory — token metadata is exposed via `&'static` data (SC-008). Citations in rule sources, migration entries, and diagnostic messages are verified against the vendored primary source at commit time and removed rather than retained pending follow-up (Constitution VIII + FR-021 + SC-009).
**Scale/Scope**: ~15 of 39 CAPCO rules retire in Phase C (declarative `Constraint` / `PageRewrite` entries replace hand-written `Rule` impls for the constraint/rewrite-shaped subset). Mangled-marking fixture ≥ 200 labeled cases across six mangling classes (typo, reordering, missing-delimiter, superseded-token, wrong-case, garbled-delimiter). Vocabulary surface exposes authority + owner/producer + POC + deprecation + URN + schema version + portion/banner forms for every active ISM-v2022-DEC token (several hundred terms). ODNI schema pin stays at `ISM-v2022-DEC` for the duration of the branch — any bump is a separate deliberate migration per Constitution Principle IV. No change to the crate graph topology; two new public trait surfaces (`Recognizer`, `Codec<S>`) and one expanded surface (`Vocabulary`) are added.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Check | Evidence in spec |
|---|---|---|
| **I. Uncompromising Performance** | p95 budgets declared and Criterion-measurable | SC-001 (≤16 ms strict), SC-002 (≤18 ms with mangled region) |
| **II. Zero-Copy, Streaming Core** | No new heap allocations on hot path; vocabulary queries return `&'static` data | FR-016 "static data requiring no allocation", SC-008; decoder candidate set bounded at K=8 per §5.2 of primary-source plan |
| **III. Format-Agnostic / WASM Safety** | No format deps enter WASM-safe crates; runtime corpus override compile-time excluded from WASM | FR-013 third clause; threat-model T3 enforcement; compile-time check (not runtime) per constitution Principle III amendment |
| **IV. Two-Layer Rule Architecture + no engine edits in *scheme-adoption* PRs** | Phase C constraint data is hand-written Layer-2 against Layer-1 generated predicates; this branch is the engine-infrastructure PR (not a scheme-adoption PR), so FR-022's engine-edit prohibition binds future Phase-F-onward adoption work, not this work | FR-022 (revised) explicit; `Recognizer` trait, scheduler, audit v2, and vocabulary tables all land here so Phase F onward can honor FR-022 |
| **V. Audit-First Compliance** | Single audit schema version per build; content-ignorance preserved; evidence labels enum-typed not free-string | FR-009 (posterior + runner-up + features in audit record), FR-012 (`FeatureId` enum, not free string), FR-014 (single-schema-version-per-build + back-compat parse), Assumption "Audit-record content-ignorance already holds today and is preserved" |
| **VI. Dataflow Pipeline Model** | Scheduler sits between scheme construction and per-document processing; Recognizer fits the existing Scanner → Parser → Rules → PageContext pipeline without restructuring | FR-007 (topological scheduling), Acceptance Scenario US1.4 (order independence); rules + Recognizer `Send + Sync` per FR-023 |
| **VII. Crate Discipline and Dependency Hygiene** | No new crates; no dependency-graph cycles; `marque-rules` stays trait-only | Existing graph preserved; Constraint/PageRewrite data lives in `marque-capco`; trait surface lives in `marque-scheme` |
| **VIII. Authoritative Source Fidelity** | Every constraint, rewrite, vocabulary entry cites a verified CAPCO-2016 / ODNI passage; citations verified at commit | FR-021, SC-009, Edge Case "rule implementer cites a passage" |

**Gate result:** PASS. No violations require justification in Complexity Tracking. The three highest-leverage discipline points — WASM runtime-config exclusion (III), engine-edit prohibition in grammar PRs (IV), and citation verification (VIII) — are encoded as functional requirements, not just acceptance scenarios, so a reviewer can reject a PR that drifts on any of them without re-reading the spec.

## Project Structure

### Documentation (this feature)

```text
specs/004-constraints-decoder-vocab/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
│   ├── constraint-evaluator.md
│   ├── page-rewrite-scheduler.md
│   ├── recognizer-trait.md
│   ├── vocabulary-trait.md
│   ├── codec-trait.md
│   ├── audit-record-v2.md
│   └── cli-server-wasm-gates.md
├── checklists/
│   └── requirements.md  # Spec quality gates (already complete)
└── tasks.md             # Phase 2 output (/speckit.tasks — NOT created by /speckit.plan)
```

### Source Code (repository root)

This work extends the existing workspace without changing the crate graph topology. Files added or substantially modified:

```text
crates/
├── scheme/                          # marque-scheme (trait surface)
│   └── src/
│       ├── constraint.rs            # EXTEND: shared Constraint evaluator (FR-002)
│       ├── rewrite.rs               # EXTEND: PageRewrite axis annotations, Promote action, scheduler errors (FR-003/004/005/007)
│       ├── recognizer.rs            # NEW: Recognizer trait (FR-008), Parsed<M>, Ambiguous result (FR-015)
│       ├── vocabulary.rs            # NEW: Vocabulary trait + TokenMetadataFull (FR-016/017)
│       └── codec.rs                 # NEW: Codec<S> trait surface, no impls (FR-019)
│
├── ism/                             # marque-ism (vocabulary crate)
│   ├── build.rs                     # EXTEND: dual JSON+XML codepath (FR-018), TokenMetadataFull emission, remove FOUO→CUI migration (FR-020)
│   ├── schemas/ISM-v2022-DEC/       # UNCHANGED (schema pin stays)
│   └── src/
│       └── generated.rs             # include!() expanded to include vocabulary metadata tables
│
├── capco/                           # marque-capco (CAPCO adapter)
│   ├── docs/CAPCO-2016.md           # AUTHORITATIVE SOURCE (Constitution VIII)
│   ├── corpus/                      # corpus-derived priors (baked into build)
│   └── src/
│       ├── scheme.rs                # EXTEND: declarative Constraint entries (~12 rules), PageRewrite entries (~3 rules), retire matching hand-written Rule impls
│       ├── vocabulary.rs            # NEW: CapcoScheme as Vocabulary impl (FR-016)
│       ├── decoder.rs               # NEW: DecoderRecognizer (FR-008), K=8 candidate generator, log-posterior scoring
│       └── rules.rs                 # SHRINK: ~15 rule impls removed, remaining rules unchanged
│
├── rules/                           # marque-rules (trait-only — no implementations)
│   └── src/
│       └── lib.rs                   # EXTEND: Confidence struct (f32 precision; FR-009), FixSource::DecoderPosterior variant (preserving BuiltinRule/CorrectionsMap/MigrationTable)
│
├── engine/                          # marque-engine (orchestration)
│   └── src/
│       ├── engine.rs                # EXTEND: Engine::new runs page-rewrite topological sort (FR-007); lint_inner drives Recognizer (FR-008)
│       ├── errors.rs                # EXTEND: EngineConstructionError::RewriteCycle, ::UnannotatedCustomAxes (FR-004/005)
│       └── fix.rs                   # EXTEND: audit-record v2 emission with confidence + features + runner-up (FR-009, schema bump to marque-mvp-2)
│
├── config/                          # marque-config (layered config)
│   └── src/
│       └── lib.rs                   # EXTEND: --corpus-override CLI-only parsing; server rejects on HTTP (T3 enforcement, FR-013)
│
├── wasm/                            # marque-wasm (web worker target)
│   └── src/
│       └── lib.rs                   # UNCHANGED runtime API; build-time rejection of corpus-override codepath via feature gate + compile-fail test (FR-013)
│
└── server/                          # marque-server (axum)
    └── src/
        └── handlers.rs              # EXTEND: reject caller-supplied corpus override from POST /v1/lint + /v1/fix (FR-013)

tests/
├── fixtures/
│   └── mangled/                     # NEW: ≥200 labeled mangled cases across six mangling classes (FR-008, SC-004)
│       ├── typo/
│       ├── reordering/
│       ├── missing-delimiter/
│       ├── superseded-token/
│       ├── wrong-case/
│       └── garbled-delimiter/
├── corpus/                          # existing CAPCO corpus — equivalence gate (SC-003, SC-005)
└── wasm-parity/                     # existing WASM parity harness — scaled to full corpus already

tools/
└── corpus-analysis/                 # EXTEND: generator emits tests/fixtures/mangled/ from Enron high-confidence markings + corpus-derived priors for Phase D

crates/engine/benches/
├── lint_latency.rs                  # EXTEND: SC-001 (strict p95) + SC-002 (with-mangled-region p95) gates
└── linear_scaling.rs                # UNCHANGED (SC-005)

docs/
└── plans/
    └── 2026-04-19-recursive-lattice-and-decoder.md   # PRIMARY TECHNICAL SOURCE
```

**Structure Decision**: existing Rust cargo workspace preserved exactly. No new crates, no crate-graph edges added or reversed. The three-phase bundle fits within existing crate responsibilities:

- Trait surface additions → `marque-scheme` (already the domain-neutral trait crate per Constitution VII)
- Domain data (constraint entries, PageRewrite entries, Vocabulary impl, DecoderRecognizer) → `marque-capco`
- Build-time code generation (dual JSON+XML codepath, TokenMetadataFull tables, FOUO→CUI migration removal) → `marque-ism/build.rs`
- Audit schema bump + orchestration changes → `marque-engine`

Rationale for not introducing a new crate: the new trait surfaces (`Recognizer`, `Codec<S>`, extended `Vocabulary`) are natural members of `marque-scheme` (domain-neutral, WASM-safe, no new runtime dependencies). A `marque-decoder` crate was considered and rejected — the decoder implementation is CAPCO-specific (reads CAPCO token semantics, CAPCO base rates, CAPCO template shapes); splitting it into its own crate would force either (a) an engine-visible decoder trait that every adapter implements separately or (b) a crate that's just a module rename of code that already belongs with its adapter. Keeping the decoder in `marque-capco` preserves the "shallow adapter" shape (Constitution IV) and leaves room for a future abstract decoder trait to be hoisted into `marque-scheme` once a second adapter (CUI, Phase F) actually exercises the pattern.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

No violations. Left intentionally empty.
