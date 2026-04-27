# Marque — Design Document
**Date**: 2026-03-11
**Status**: archived — superseded by `2026-04-17-marking-scheme-lattice-design.md` (and again by `2026-04-19-recursive-lattice-and-decoder.md`). Original CAPCO-centric design implemented across Phases 1–9, the SCI/SAR specs, and the recursive-lattice work — see CLAUDE.md "Recent Changes" for the full landing trail. Pipeline + crate-graph framing remain accurate; rule-set scope and trait surface have moved on. Kept for historical context.
**Working directory**: `/home/knitli/classified`

---

## 1. Concept

`marque` is a linter, formatter, and auto-fixer for IC classification markings — portion markings and banner markings — in the style of `ruff`. The defining characteristic is **perceptual instantaneity**: it should feel like magic, whether validating a single web form field or processing a million archival documents.

Secondary capability: document metadata extraction and sanitization, integrated into the same pipeline pass.

### The Problem

Classification marking errors are pervasive across the IC and DoD:
- Deprecated markings that should have been updated years ago (declassification markings still appearing in banners, retired caveats, wrong abbreviations)
- Structural violations (misordered blocks, wrong separator count, missing `USA` in trigraph lists)
- Typos in marking strings (`SERCET`, `NOFRON`, missing `//`)
- Metadata leakage — sensitive authorship, revision history, GPS data in embedded images — widely underappreciated outside niche security circles

Existing implementations of ODNI's Schematron schemas are **binary** — valid or invalid, with no remediation guidance. `marque` adds the intelligence layer: why it's wrong, how to fix it, with what confidence.

The labor math is significant: ~1M cleared personnel performing marking-related tasks ~12 times per day, with per-task time savings of 1–20 minutes. The data quality angle is increasingly critical as IC AI initiatives require clean, validated classification metadata as foundational infrastructure.

### Name

`marque` — French for mark/brand, and an oblique reference to *letter of marque* (government-authorized operators in gray zones, an uncomfortably accurate metaphor for IC contracting). Crates.io availability confirmed.

---

## 2. Guiding Principles

### 1. Uncompromising Speed
Blistering speed is the feature. Every performance decision is made against benchmarked comparison, never assumption. The tool never blocks — async-first throughout. Targets: perceptually instant for interactive use (< 16ms for typical inputs); linear scaling for batch.

### 2. Pluggable, Configurable, Amendable
Easily extensible with thin UI/API wrappers and middleware (auth, validation, logging, rate limiting). The pipeline is a **dataflow model** — a chain of async streams where stages are traits, not a call stack. New rule sets, format adapters, and integration surfaces slot in without modifying core.

### 3. Low Memory — Zero-Copy, Streaming by Default
Documents stream through the pipeline in chunks; never held whole in memory. Spans are byte offsets into original buffers, never copies. Cacheable (rule automata, config) but capable of zero memory retention for sensitive content. Longer-term: integration with secure computing processor enclaves (SGX/TrustZone).

### 4. Format-Agnostic Core
`marque-core` knows nothing about file formats. It operates on `TextStream` — an async stream of byte chunks with position metadata. Format adapters are separate, optional crates. The WASM build has zero format dependencies; native builds pull in the extraction layer.

---

## 3. Architecture — Crate Structure

```
marque/                         (Cargo workspace)
├── crates/
│   ├── marque-core/            scanner + AST + parser (WASM-safe, no format deps)
│   ├── marque-rules/           Rule/Diagnostic/Fix trait definitions
│   ├── marque-capco/           CAPCO rule implementations (code-generated from ODNI specs)
│   ├── marque-engine/          pipeline orchestration: core + rules → diagnostics + fixes
│   ├── marque-extract/         Kreuzberg wrapper — text + metadata extraction, OCR
│   ├── marque-config/          config loading, validation, layered precedence
│   ├── marque-wasm/            wasm-pack target, web worker API
│   └── marque-server/          axum microservice
└── marque/                     CLI binary (thin shell over marque-engine)
```

### Dependency Graph (one-way, no cycles)

```
marque-core  ←  marque-rules  ←  marque-capco
                     ↓
              marque-engine  ←  marque-config
               ↑          ↑
      marque-extract    marque-wasm
               ↑
        marque-server
               ↑
            marque (CLI)
```

### Key Crate Notes

**`marque-core`** — WASM-safe. No format knowledge. Operates on `&[u8]` / `TextStream`. Contains scanner, parser, `IsmAttributes` AST node, span types.

**`marque-rules`** — Trait definitions only. No implementations. `Rule`, `Diagnostic`, `Fix`, `Severity`, `AuditRecord`. The contract every rule crate must satisfy.

**`marque-capco`** — Code-generated from ODNI ISM schemas via `build.rs`. See §5. Versioned independently — pinned in downstream config.

**`marque-extract`** — Wraps [Kreuzberg](https://github.com/kreuzberg-dev/kreuzberg): Rust-core, SIMD-optimized, streaming, 75+ formats, OCR for scanned documents. Handles metadata extraction for the sanitization feature. **Not included in WASM build.**

**`marque-wasm`** — `wasm-pack` target. Exposes `lint(text, config)` → `LintResult` and `fix(text, config)` → `FixResult` for web worker use. Format extraction is the caller's responsibility.

**`marque-server`** — `axum` + `tokio`. REST API wrapping `marque-engine`. Auth/logging/rate-limiting as Tower middleware layers (slots cleanly into the dataflow model).

---

## 4. Pipeline — Dataflow Model

The pipeline is a chain of async streams. Each stage is a `Stream` impl. Middleware inserts between stages. CLI, WASM, and server are different `Source`/`Sink` configurations wired to the same middle.

```
Source (file / stream / network / string)
  ↓ [marque-extract — native only]
TextStream  (format-agnostic byte chunks + position info)
  ↓ [scanner — marque-core]
SpanStream  (candidate marking locations, zero-copy byte offsets)
  ↓ [parser — marque-core]
AttributeStream  (IsmAttributes + DocumentContext per span)
  ↓ [validator — marque-engine, rules from marque-capco]
DiagnosticStream  (violations + proposed fixes + confidence + audit)
  ↓ [fixer / reporter]
Output  (fixed text | diagnostic report | audit log | JSON)
```

Backpressure is natural. Large batch jobs apply the same pipeline with a file-list source and parallel worker pool. The WASM web worker uses the same pipeline with a string source.

---

## 5. Incremental Batch Cache

Large archival corpora (millions of documents) are re-processed repeatedly: schema updates, rule changes, corrections map tuning. Without a cache, every run is a full scan. With a cache, unchanged documents are instant.

### Design

The cache is an LMDB database (`heed` crate — safe Rust bindings to Lightning Memory-Mapped Database). It lives at `.marque/cache.lmdb` alongside `.marque.toml`, opt-in, gitignored.

**Cache key** (composite):
```
blake3(document_content)
  ++ capco_schema_version     // from CURRENT_VERSION / config pin
  ++ rule_config_hash         // blake3 of the serialized [rules] + [corrections] config
```

All three components must match for a cache hit. This means:
- Document content change → cache miss, full reprocess
- Schema version bump → all entries stale, full corpus reprocess on next run
- Rule severity / corrections change → all entries stale

**Cache value**: serialized `LintResult` (diagnostics + spans) via MessagePack (`rmp_serde`). MessagePack chosen over JSON for compact binary representation — roughly 2–5× smaller for typical diagnostic lists, and zero parsing overhead on read.

Only `LintResult` is cached, never `FixResult`. Fixes modify the document; caching the modified bytes would create a stale-document problem. The correct pattern is: read from cache → if hit, apply fixes from cached diagnostics on-demand (fixes are deterministic from diagnostics).

**Key invalidation flow** (from CocoIndex v1's `use_or_invalidate_component_memoization` pattern):
```
read_txn → lookup composite key
  → hit + fingerprint match  → return cached LintResult
  → hit + fingerprint mismatch → delete entry, return cache miss
  → miss                      → return cache miss
```

Write path: after processing, write `(composite_key → msgpack(LintResult))` in a batched write transaction. Batching writes avoids per-document transaction overhead.

### Integration with BatchEngine

`BatchEngine` gains an optional `cache: Option<Arc<LmdbCache>>`. When present:

```
for each document:
  fingerprint = blake3(content)
  composite_key = fingerprint ++ schema_version ++ config_hash
  if cache.get(composite_key) → Some(result):
      yield (id, result)          // cache hit: zero scan/parse/validate
  else:
      spawn_blocking lint         // cache miss: full pipeline
      cache.put(composite_key, result)
      yield (id, result)
```

The `ConcurrencyController` byte semaphore applies only to cache misses — cache hits are effectively free.

### Technology

| Component | Crate | Notes |
|-----------|-------|-------|
| LMDB bindings | `heed` | Safe Rust API, memory-mapped, ACID, embedded |
| Key/value serialization | `rmp_serde` | MessagePack — compact, fast, serde-compatible |
| Document fingerprint | `blake3` (via `recoco-utils::fingerprint`) | Already in dep tree |
| Config hash | `blake3` | Same hasher, keyed over serialized config bytes |

### Design Decisions

- **Opt-in** — batch runs without `--cache` (or `cache.enabled = true` in config) never touch LMDB; no lock files, no side effects
- **Single-writer** — LMDB handles concurrent readers natively; the `BatchEngine` is the sole writer per run
- **No eviction policy** — the cache grows monotonically. Invalidation is whole-corpus (schema/rule change) or per-document (content change). Manual `marque cache clear` for explicit purge.
- **Span fidelity** — cached spans are byte offsets into the *original* document content. The fingerprint check guarantees the document hasn't changed, so spans remain valid.
- **Crate home** — `marque-engine` behind a `cache` feature flag. Not in `marque-core` (no LMDB in WASM) and not in a separate crate (it's a BatchEngine concern).

---

## 6. Rule Engine — Two-Layer Architecture

### Layer 1: Schema Validation (code-generated)

`marque-capco/build.rs` parses ODNI ISM specification files at compile time:

```
marque-capco/schemas/ISM-v2023.1/
├── CVEValues.xml        controlled vocabulary enumerations (all valid token values)
├── ISM-XML.xsd          attribute structure and constraints
├── ISM-XML.sch          Schematron cross-attribute validation rules
└── ISM-XML.rng          RelaxNG (redundant, retained for cross-validation)
```

`build.rs` generates `src/generated/`:
- `values.rs` — Rust enums for every CVE enumeration (classification levels, SCI controls, SAR identifiers, dissem controls, country trigraphs, handling instructions)
- `rules.rs` — validation predicates compiled from Schematron assertions
- `migrations.rs` — deprecated value → replacement mappings with version references

Binary output: **valid / invalid** predicates only. The schema says what is correct. No remediation.

### Layer 2: Diagnostic Intelligence (hand-written)

Hand-written `Rule` implementations that:
- Consume Layer 1 predicates to detect violations
- Classify *why* the violation occurred
- Determine the fix, confidence level, and migration reference
- Respect configured severity

This is the product differentiation. Examples of Layer 2 rules covering high-confidence, unambiguous fixes:

| Rule ID | Violation | Fix | Confidence |
|---------|-----------|-----|------------|
| `E001` | Banner uses portion abbreviation (`S`, `TS`, `NF`) | Expand to full word | 1.0 |
| `E002` | Misordered blocks (e.g., SAP after dissem) | Reorder per CAPCO spec | 1.0 |
| `E003` | `//` separator count wrong (1 or 3 slashes) | Normalize to `//` | 1.0 |
| `E004` | Missing `USA` in REL TO trigraph list | Insert `USA` first | 1.0 |
| `E005` | Wrong trigraph order (`USA` not first) | Reorder | 1.0 |
| `E006` | Declassification marking in banner | Move to CAB `Declassify On:` field | 0.95 |
| `E007` | Deprecated marking with named replacement | Apply replacement per migration table | 0.98 |
| `E008` | Deprecated marking, move to CAB | Move and reformat | 0.92 |
| `W001` | Missing portion marking on paragraph | Flag — cannot auto-fix, intent unknown | — |
| `C001` | Typo correction (e.g., `SERCET` → `SECRET`) | Apply from corrections map | 1.0 |

### Rule Trait

```rust
pub trait Rule: Send + Sync {
    fn id(&self) -> RuleId;
    fn name(&self) -> &'static str;
    fn default_severity(&self) -> Severity;
    fn check(&self, attrs: &IsmAttributes, ctx: &DocumentContext) -> Vec<Diagnostic>;
}

pub struct Diagnostic {
    pub rule: RuleId,
    pub severity: Severity,           // from config, defaulted by rule
    pub span: Span,                   // byte offsets into source
    pub message: String,
    pub fix: Option<Fix>,
}

pub struct Fix {
    pub replacement: String,
    pub confidence: f32,              // 0.0–1.0
    pub audit: AuditRecord,           // always generated, even for 1.0 confidence
    pub migration_ref: Option<&'static str>,   // e.g. "CAPCO-2016 §A.6" (CAPCO Register and Manual, 2016)
}

pub struct AuditRecord {
    pub rule: RuleId,
    pub original: String,
    pub replacement: String,
    pub confidence: f32,
    pub timestamp: SystemTime,
    pub classifier_id: Option<String>,    // from user config
}
```

---

## 7. Core Data Model — Scanner & Parser

### `IsmAttributes` (the pivot type)

Mirrors the ISM XML attribute model. Whether the source is XML (attributes parsed directly from DOM) or free text (scanned and parsed from marking strings), everything normalizes to this struct.

```rust
pub struct IsmAttributes {
    pub classification: Classification,
    pub sci_controls: Vec<SciControl>,
    pub sar_identifiers: Vec<SarIdentifier>,
    pub dissem_controls: Vec<DissemControl>,
    pub rel_to: Vec<Trigraph>,
    pub declassify_on: Option<DeclassDate>,
    pub classified_by: Option<String>,
    pub derived_from: Option<String>,
    pub declass_exemption: Option<DeclassExemption>,
}
```

All enum types (`Classification`, `SciControl`, etc.) are code-generated from CVE at build time.

### Phase 1 — Candidate Detection

`memchr` (SIMD-accelerated) for `(` boundary scanning. A lightweight state machine walks candidate boundaries. Zero heap allocation. Output: `Vec<Span>` tagged as `PortionCandidate`, `BannerCandidate`, or `CabCandidate`.

### Phase 2 — Token Extraction

Compile-time Aho-Corasick automaton built from every CVE token:

```rust
// Generated by build.rs; CVE_TOKENS: &[&str] from generated values.rs
static MARQUE_AC: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasick::new(CVE_TOKENS).unwrap()
});
```

Consider `daachorse` (double-array Aho-Corasick) for the WASM target — more memory-compact. Benchmark against `aho-corasick` for WASM binary size and speed.

Single pass over each candidate span. Extracts all recognized tokens with positions. Unrecognized tokens within a candidate boundary are themselves a diagnostic.

### Phase 3 — Replacement Lookup

Two maps, different characteristics:

- **User corrections** (runtime config): `HashMap` with `rapidhash` from `thread-utils`. Built at startup from `.marque.toml` `[corrections]` table.
- **Compiled deprecated marking replacements** (compile-time known): `phf` (perfect hash functions) — zero collisions, faster for static key sets.

Both maps checked in order: user corrections take precedence over compiled defaults.

### Document Context

```rust
pub struct DocumentContext {
    pub position: DocumentPosition,    // Start | Body | End
    pub zone: Zone,                    // Header | Footer | Body | CAB
    pub paragraph_offset: usize,       // position within paragraph
    pub marking_type: MarkingType,     // Portion | Banner | CAB
}
```

Context gates parser invocation — `(S)` mid-paragraph body text is a likely portion marking; `(S)` in a table caption requires additional heuristics.

---

## 8. Config Model

Three layers, evaluated in precedence order (highest last wins):

### `.marque.toml` — Project/Org Level (committed)
```toml
[capco]
version = "2023.1"          # pins ISM schema version; opt-in to upgrades

[rules]
deprecated-marking     = "fix"
banner-abbreviation    = "fix"
missing-portion        = "error"
misordered-block       = "fix"
missing-usa-trigraph   = "fix"
declassify-in-banner   = "fix"
missing-cab            = "warn"

[corrections]
"SERCET"  = "SECRET"
"SECRECT" = "SECRET"
"NOFRON"  = "NOFORN"
"CONFIDENTAL" = "CONFIDENTIAL"
```

### `.marque.local.toml` — User Level (gitignored)
```toml
[user]
classifier_id            = "12345"
classification_authority = "EO 13526"
default_reason           = "1.4(a)"
derived_from_default     = "SCG-PROGRAM-2023"
```

### Environment / CLI (runtime, highest precedence)
```
MARQUE_CLASSIFIER_ID=...
marque check --strict
marque fix --dry-run
marque fix --confidence-threshold 0.95
```

### Design decisions
- **User identity is always local** — classifier IDs never in committed config
- **Rule severity is org policy** — committed, auditable, reviewable in version control
- **Schema version is pinned** — prevents silent behavior changes on `marque-capco` updates
- **`--fix` is a mode flag** — cleanly separates "this rule *can* fix" (rule definition) from "apply fixes *now*" (invocation intent)
- **`--dry-run`** — shows what would be fixed without writing; always produces audit output

---

## 9. WASM & Server API Surface

### WASM (web worker)

```typescript
interface LintRequest {
  text: string;
  config?: Partial<Config>;
  context?: 'portion' | 'banner' | 'full-document' | 'form-field';
}

interface LintResult {
  diagnostics: Diagnostic[];
  fixes: Fix[];
  metadata_warnings?: MetadataWarning[];   // only if metadata flag set
}

interface Fix {
  span: [number, number];
  original: string;
  replacement: string;
  confidence: number;
  rule_id: string;
  migration_ref?: string;
}
```

### Server (axum REST)

```
POST /v1/lint           text or document → diagnostics
POST /v1/fix            text or document → fixed text + audit log
POST /v1/metadata       document → metadata report
POST /v1/batch          { documents: [...] } → batch results
GET  /v1/health
GET  /v1/schema/version → active CAPCO schema version
```

Auth, logging, and rate limiting as Tower middleware layers — plugs into the dataflow model at the server boundary without touching engine code.

---

## 10. Format Support

### WASM Build
Input: raw text string (caller-provided). No format dependencies.

### Native / Server Build
Via `marque-extract` (Kreuzberg wrapper):
- **Batch priority**: `.docx`, `.pdf` (digital + OCR for scanned), raw text buffers
- **Office suite**: `.docx`, `.xlsx`, `.pptx`, `.msg` (Outlook)
- **Web/data**: HTML, XML (including IC-XML native formats — direct ISM attribute parsing, no scanning needed)
- **75+ additional formats** via Kreuzberg

Metadata extraction (document properties, EXIF, XMP, PDF metadata, embedded image EXIF) handled by Kreuzberg in the same extraction pass. Metadata warnings are surfaced as a separate output channel — always reported, stripping is opt-in.

---

## 11. Rule Crate Extensibility

Future rule crates follow the same `build.rs` → generated code pattern:

```toml
# Future workspace additions
marque-cui      # CUI marking validation (NARAs 125+ categories — complex)
marque-ntk      # Need-To-Know metadata (IC-NTK spec)
marque-tdf      # Trusted Data Format validation (IC-TDF spec)
```

Any IC technical specification with CVE + Schematron can become a rules crate with minimal hand-written code. The traits, pipeline, and config model are unchanged.

---

## 12. Technology Stack Summary

| Layer | Choice | Rationale |
|-------|--------|-----------|
| Language | Rust | Performance, WASM target, memory safety (NSA/CISA guidance), ATO narrative |
| Async runtime | Tokio | Ecosystem standard, axum integration |
| WASM | wasm-pack | Best-in-class WASM compilation story for Rust |
| HTTP server | axum | Tower middleware ecosystem, async-native |
| Phase 1 scanner | memchr | SIMD-accelerated, zero-allocation boundary detection |
| Phase 2 token matching | aho-corasick / daachorse | Compile-time automaton from CVE tokens |
| Replacement lookup (runtime) | rapidhash via thread-utils | Fastest available; existing SIMD functions reused |
| Replacement lookup (compile-time) | phf | Perfect hash, zero collisions for static key sets |
| Rule schema parsing | quick-xml (build.rs) | Parse CVE/XSD/Schematron at build time |
| Format extraction + metadata | Kreuzberg | Rust-core, 75+ formats, streaming, OCR, SIMD |
| Config parsing | toml + serde | Ecosystem standard |
| Schematron→Rust | build.rs code generation | Compile-time, WASM-safe, no runtime interpreter |
| Incremental cache store | heed (LMDB) | Embedded, memory-mapped, ACID; no server process |
| Cache serialization | rmp_serde (MessagePack) | Compact binary; 2–5× smaller than JSON for diagnostic lists |
| Batch concurrency control | recoco-utils (concur_control) | Row + byte semaphores for backpressure |

---

## 13. Business Model

**Open core** with commercial enterprise tier.

- **Engine + CAPCO rules** (`marque-core`, `marque-rules`, `marque-capco`): Apache 2.0 — maximum IC adoption, no procurement friction, pulls cleanly from crates.io mirror
- **Integration layer** (Office add-ins, managed API, enterprise dashboard): Elastic License 2.0 or BSL — source-visible for auditors, contributions possible, no "provide as competing service"
- **Enterprise tier**: Commercial license with SLA

**Revenue streams**:
1. Managed rule update subscription — authoritative CAPCO schema updates pushed to subscribers
2. Office add-ins (Word, Outlook, PowerPoint, Excel)
3. Enterprise support / SLA contracts
4. AI pipeline middleware — classification-aware document chunking and metadata normalization for IC AI initiatives
5. SBIR Phase I/II → IDIQ/GSA MAS contract vehicle

**Moats** (stronger than license):
- Domain authority — the canonical expert on CAPCO edge cases
- Rule update velocity — staying ahead of the spec
- Integration depth — Office add-ins take time to build; partnering is cheaper than forking
- Liability clarity — commercial customers have recourse

The FOUO rules gap (a small number of non-public CAPCO rules whose classification is unjustified) is a potential commercial offering: a complete, commercially-maintained ruleset available under NDA.

---

## 14. Integration Roadmap

**MVP**: `marque-core` + `marque-rules` + `marque-capco` + `marque-wasm` + `marque-server`
- Core pipeline working end-to-end on raw text input
- CAPCO rules covering highest-frequency violations
- WASM web worker API
- REST microservice

**v0.2**: CLI + `marque-extract` integration
- `marque check file.docx`
- `marque fix --batch *.pdf`
- Metadata reporting
- Incremental batch cache (`--cache` flag, LMDB-backed, opt-in)

**v0.3**: Browser extension

**v0.4**: Office add-ins (Outlook → Word → PowerPoint → Excel)

**v1.0**: Managed rule update service, enterprise dashboard, audit log export

**Post-v1**: `marque-cui`, LSP server, `marque-ntk`

---

*Generated from brainstorming session 2026-03-11. Pre-implementation — details subject to revision.*
