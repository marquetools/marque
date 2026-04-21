# Phase 1 — Data Model

**Feature:** `004-constraints-decoder-vocab`

This document captures the entities introduced or extended by Phases C, D, and E, their fields, invariants, and cross-entity relationships. Authoritative mechanism (method signatures, trait layout, serialization details) lives in the 2026-04-19 plan and the `contracts/` sibling directory. This file is the entity reference — what exists and what each field means.

## Terminology

- **Strict recognizer** — the canonical name for the `Recognizer` implementation that wraps the current zero-allocation structural parser (`StrictRecognizer<S>`). "Strict path" and "strict parser" are informal shorthand for the same thing and are used interchangeably in narrative text; code and contracts use `StrictRecognizer`.
- **Decoder recognizer** — the canonical name for `DecoderRecognizer<S>`, the probabilistic Phase-D recognizer. "Decoder" (alone) is informal shorthand; code and contracts use `DecoderRecognizer`.
- **Posterior** vs. **confidence** — `posterior` is a per-candidate probability produced by the decoder; `confidence` is the composite `Confidence` struct attached to a fix proposal (`recognition * rule * region`). They are related but distinct — do not substitute one for the other. `Confidence` is the foundational-plan name (not `FixConfidence`); precision is `f32` throughout.

## Phase C — Declarative constraints and page-level rewrites

### Constraint

Declarative expression of a binary relationship between tokens or categories. Replaces ~12 hand-written `Rule` impls in `marque-capco/src/rules.rs`.

`Constraint` is a **non-generic enum** already defined in `crates/scheme/src/constraint.rs` since Phase B:

| Variant | Meaning |
|---|---|
| `Conflicts { left, right, label }` | `left` and `right` MUST NOT both be present |
| `Requires { left, right, label }` | Presence of `left` REQUIRES presence of `right` |
| `Implies { left, right, label }` | Presence of `left` IMPLIES presence of `right` (warning severity) |
| `Supersedes { left, right, label }` | `left` displaces `right` in page roll-up |
| `Custom { name, label }` | `name` is the rule identifier dispatched by `MarkingScheme::validate`; `label` is the authoritative-source citation. Escape hatch for constraints not expressible via the above four |

**Invariants:**
- All fields (`left`, `right`, `label`, etc.) are `&'static` / const — no runtime allocation.
- `label: &'static str` points at an authoritative-source passage (e.g., `"CAPCO-2016 §H.4"`). No structured `SourceCitation` type; foundational-plan §7a uses plain `&'static str` and Phase C inherits that convention. Citation verification happens at commit time (Constitution VIII); re-verification at citation propagation (T089).
- Evaluation is stateless (FR-023): the shared evaluator in `marque-scheme` takes `&[Constraint]`, `&S::Marking`, `&S` and returns `Vec<ConstraintViolation>` with no mutable borrowed state.

### ConstraintViolation

| Field | Type | Meaning |
|---|---|---|
| `constraint_label` | `&'static str` | The `label` of the triggering constraint (already present in Phase-B code) |
| `message` | `String` | Rendered diagnostic message |
| `citation` | `&'static str` | **NEW in Phase C**: authoritative-source passage copied from the triggering constraint verbatim |

**Note:** Phase C adds a `citation: &'static str` field to the existing `ConstraintViolation` struct. It does NOT introduce a `SourceCitation` type, a `DiagnosticTemplate` type, or a `Constraint<S>` generic reshape — all three would contradict foundational-plan §12 which preserves `Constraint` as the non-generic enum.

### PageRewrite

A declarative transformation applied during page-level roll-up. Cross-category transformations like "NOFORN supersedes REL TO" live here instead of as ad hoc logic inside `PageContext`. The type already exists in `crates/scheme/src/page_rewrite.rs` since Phase B; Phase C adds `reads`/`writes` fields and the `Promote` `CategoryAction` variant.

| Field | Type | Meaning |
|---|---|---|
| `id` | `RewriteId` | Stable identifier (e.g., `capco/noforn-clears-rel-to`) |
| `trigger` | `CategoryPredicate` | When the rewrite fires |
| `action` | `CategoryAction` | What the rewrite does (`Clear`, `Replace`, `Promote { from, to, transform }`, `Custom`) |
| `reads` | `&'static [CategoryId]` | **NEW in Phase C**: axes the rewrite inspects |
| `writes` | `&'static [CategoryId]` | **NEW in Phase C**: axes the rewrite mutates |
| `citation` | `&'static str` | Authoritative-source passage (e.g., `"CAPCO-2016 §F.2 p43"`). Plain `&'static str` per foundational-plan line 943 — no structured `SourceCitation` type |

**Invariants:**
- `PageRewrite::declarative` constructor derives `reads`/`writes` from the `trigger` and `action` enum variants at scheme build time.
- `PageRewrite::custom` constructor requires explicit `reads`/`writes` — engine construction rejects a custom rewrite without them (FR-005).
- Rewrites form an acyclic read/write dependency graph. Cycles produce `EngineConstructionError::RewriteCycle` at `Engine::new` (FR-004).
- Scheduled order is determined by topological sort over the read/write graph. Declaration order does not affect scheduled order (FR-007).
- `const fn` constructor caveat: `S: MarkingScheme + ?Sized` does not compose with `const fn` trait dispatch on stable Rust. If the chosen implementation path makes `declarative`/`custom` non-const, the axis derivation happens at module load via `OnceLock` or `&'static [PageRewrite<S>]` literals — no per-document cost either way.

### CategoryAction

New variant added in Phase C.

| Variant | Meaning |
|---|---|
| `Clear` | Remove content from the written axis |
| `Replace` | Replace the written axis with a new value |
| `Promote { from, to, transform }` | Move content from `from` to `to`, optionally transforming via `transform` function |
| `Custom` | Free-form — requires explicit `reads`/`writes` on `PageRewrite::custom` |

### EngineConstructionError

Errors raised at `Engine::new` during scheme validation.

| Variant | Meaning | Trigger |
|---|---|---|
| `RewriteCycle { axis: CategoryId, members: Box<[RewriteId]> }` | A read/write cycle exists among the declared rewrites; `members` names every participating rewrite (cycles ≥3 are valid per foundational-plan line 1066). `members` is owned because cycle membership is computed at engine-construction time from the declared graph, not borrowed from a static table | FR-004 |
| `UnannotatedCustomAxes { rewrite: RewriteId }` | A `PageRewrite::custom` was declared without explicit `reads`/`writes` | FR-005 |

## Phase D — Probabilistic recognizer

### Recognizer (trait)

Trait abstraction over the parsing phase. Both the strict parser and the probabilistic decoder implement it; the engine is written against the trait.

| Method | Signature | Meaning |
|---|---|---|
| `recognize` | `fn recognize(&self, span: &[u8], context: &ParseContext) -> Parsed<S::Marking>` | Attempt to parse the span into a marking |

**Invariants:**
- `Recognizer` is `Send + Sync` (FR-023).
- No hidden mutable state (Constitution VI).

### Parsed<M>

Recognition result. This type already exists in `crates/scheme/src/ambiguity.rs` since Phase B; Phase C/D reuse it unchanged.

| Variant | Meaning |
|---|---|
| `Unambiguous(M)` | Parse succeeded; single marking returned |
| `Ambiguous { candidates: Vec<Candidate<M>> }` | Zero or more candidates with evidence + log-odds; zero-candidate `Ambiguous` is the FR-015 "we see signal, can't resolve" signal per foundational-plan line 609-612 |

**Why no third `Unrecognized` variant:** Foundational-plan explicitly rejects silent fallthrough (line 609-612). The zero-candidate case IS a meaningful ambiguity signal, not a distinct "unrecognized" state. `Parsed::Ambiguous { candidates: vec![] }` is the canonical zero-candidate form.

**Candidate<M>** preserves evidence chain:

| Field | Type | Meaning |
|---|---|---|
| `marking` | `M` | The candidate marking |
| `evidence` | `Vec<EvidenceFeature>` | Feature contributions that produced this candidate |
| `prior_log_odds` | `f32` | Log-prior score from corpus-derived base rates |

`Candidate<M>` is rich (not `(M, f64)`) because the `evidence` + `prior_log_odds` chain backs the G5 decoder-provenance invariant in audit records.

### DecoderRecognizer

Phase D's probabilistic recognizer. Implements `Recognizer`.

| Field | Type | Meaning |
|---|---|---|
| `priors` | `&'static CorpusPriors` | Compile-time-baked corpus-derived priors (FR-013 third clause: no runtime learning, no WASM override) |
| `candidate_generator` | `CandidateGenerator` | Bounded edit-distance + reordering generator, K = 8 per template |
| `template_set` | `&'static [GrammarTemplate]` | The grammar's template shapes (CAPCO-specific) |
| `strict_context` | `Option<StrictContext>` | Information threaded from strict-path decisions in the same document (e.g., "any portion ≥ CONFIDENTIAL here?") for FR-011 |

**Invariants:**
- `Send + Sync` (FR-023).
- K = 8 candidate bound per template (R3 in research.md).
- FR-011: if `strict_context` indicates any CONFIDENTIAL-or-higher strict-path evidence in the document, `(C)` cannot resolve to copyright-class candidate.
- FR-015: input fitting no template returns `Parsed::Ambiguous { candidates: vec![] }` — the zero-candidate form IS the "we see signal, can't resolve" signal (foundational-plan line 609-612), distinct from a strict-path error and distinct from `Unambiguous`.

### Confidence

Composite confidence attached to every `FixProposal`. Replaces the existing scalar `confidence: f32` field with a struct. Name is `Confidence` (not `FixConfidence`) per foundational-plan line 739-757.

| Field | Type | Meaning |
|---|---|---|
| `recognition` | `f32` | Probability the recognizer chose the right marking; strict path always sets `1.0` |
| `rule` | `f32` | Rule-level confidence (existing mechanism) |
| `region` | `Option<f32>` | Optional region-level confidence when the recognizer surfaces one |
| `runner_up_ratio` | `Option<f32>` | Top-over-second posterior ratio; `None` for strict-path fixes with no runner-up |
| `features` | `Vec<FeatureContribution>` | Evidence that drove the decision (enum-typed labels per FR-012) |

**Aggregate score:** `recognition * rule * region.unwrap_or(1.0)`.

**Precision:** `f32` throughout — SIMD-friendly and aligns with existing `Candidate<M>::prior_log_odds: f32`. Decoder internals may compute in `f64` and downcast at the `Confidence` boundary.

### FeatureContribution

One feature's contribution to the posterior.

| Field | Type | Meaning |
|---|---|---|
| `id` | `FeatureId` | Enumerated label (FR-012 — free-form strings rejected by the type system) |
| `delta` | `f32` | Log-posterior contribution |

### FeatureId (enum)

| Example variants | Meaning |
|---|---|
| `EditDistance1` | Top candidate is within one edit of the observed input |
| `TokenReorder` | Top candidate matches after canonical reordering |
| `SupersededToken` | Input token has a known replacement in the migration table |
| `BaseRateCommonMarking` | Prior favors this marking based on corpus base rates |
| `StrictContextClassification` | Other strict-path evidence in the document raises the classification floor |
| ... | Closed enum; adding a variant requires a grammar build |

**Invariant:** The enum is fixed at grammar build time (FR-012). No free-form strings.

### FixSource (extend existing enum)

Preserves the three existing variants verbatim; adds `DecoderPosterior`. No renames, no drops per foundational-plan line 716-726.

| Variant | Meaning | Origin |
|---|---|---|
| `BuiltinRule` | Existing strict-path rule decision | Pre-Phase-D (existing) |
| `CorrectionsMap` | User-configured corrections map pre-scanner hit | Pre-Phase-D (existing) |
| `MigrationTable` | Applied via a build-time migration entry | Pre-Phase-D (existing) |
| `DecoderPosterior` | Decoder-driven fix | **NEW in Phase D** |

### AuditRecord (schema bump `marque-mvp-1` → `marque-mvp-2`)

Existing record gains three fields. Pre-Phase-D records remain parseable.

| Field | Type | Added in v2? | Meaning |
|---|---|---|---|
| `rule` | `RuleId` | v1 | Rule identifier |
| `original_text` | `Box<[u8]>` | v1 | Bytes of the marking being replaced. Serialized in audit output. Content-ignorance (Constitution V) means "no surrounding document content" — the bytes contain only the marking tokens themselves, not the sentence or paragraph they appear in, not document metadata field values, not subject-claim free-form text. |
| `replacement_text` | `Box<[u8]>` | v1 | Bytes of the canonical replacement marking. Same content-ignorance invariant as `original_text`. |
| `timestamp` | `Timestamp` | v1 | When the fix was applied |
| `classifier_id` | `Option<String>` | v1 | Operator identity; sourced from `MARQUE_CLASSIFIER_ID` env var or `.marque.local.toml` (never from committed config) |
| `dry_run` | `bool` | v1 | Whether the fix was actually written |
| `confidence` | `Confidence` | **v2** | Composite confidence (replaces scalar `f32` from v1). Contains `recognition`, `rule`, `region`, `runner_up_ratio: Option<f32>`, `features` — see [Confidence](#confidence) above. `runner_up_ratio` and `features` are inside `Confidence`, not top-level `AuditRecord` fields |
| `source` | `FixSource` | **v2** | Origin of the fix: `BuiltinRule`, `CorrectionsMap`, `MigrationTable`, or `DecoderPosterior` |

**Schema invariant (FR-014):** A single engine build emits exactly one schema version. Downstream consumers parse v1 records unchanged; v2 records require v2-aware parsing.

**Content-ignorance invariant (Constitution V):** `original_text` and `replacement_text` ARE present in audit output — they are the load-bearing identifiers of what changed. Content-ignorance means the bytes contain *only the marking tokens*, not the surrounding document content, metadata field values, or subject-claim free-form text. A corpus-level integration test (T056) greps audit output for non-marking document-text fragments from the input corpus and asserts zero hits.

## Phase E — Vocabulary metadata and codec scaffolding

### Vocabulary (trait)

Exposes per-token metadata to rules. Extended from the existing minimal shape.

| Method | Returns | Meaning |
|---|---|---|
| `authority` | `&'static Authority` | Source name, URN, schema version, POC |
| `owner_producer` | `&'static OwnerProducer` | Who owns / produces the term |
| `point_of_contact` | `&'static PointOfContact` | Contact data (static) |
| `deprecation` | `Option<&'static Deprecation>` | Deprecation status (and replacement if known) |
| `portion_form` | `&'static str` | Canonical portion form (e.g., `(S)`) |
| `banner_form` | `&'static str` | Canonical banner form (e.g., `SECRET`) |
| `banner_abbreviation` | `Option<&'static str>` | Short banner form when defined |

**Invariants:**
- Every method returns `&'static` data — zero runtime allocation (SC-008).
- Active tokens always have all non-`Option` fields populated. Deprecated tokens additionally populate `deprecation`.
- The `FOUO → CUI` migration entry is **absent** from the migration table (FR-020). FOUO remains an active valid dissemination control.

### TokenMetadataFull

Per-term const record emitted by `marque-ism/build.rs` from the ODNI JSON sidecar.

| Field | Type | Meaning |
|---|---|---|
| `canonical` | `&'static str` | Canonical token name |
| `urn` | `&'static str` | ODNI URN for the term |
| `schema_version` | `&'static str` | ODNI schema version that published this form |
| `authority` | `Authority` | Source + URN + schema version of the publishing authority |
| `owner_producer` | `OwnerProducer` | Agency or body that owns/produces the term |
| `point_of_contact` | `PointOfContact` | POC for disputes / updates |
| `deprecation` | `Option<Deprecation>` | Deprecation metadata, when applicable |
| `portion_form` | `&'static str` | Canonical portion-mark form |
| `banner_form` | `&'static str` | Canonical banner form |
| `banner_abbreviation` | `Option<&'static str>` | Short banner form when defined |

### Deprecation

| Field | Type | Meaning |
|---|---|---|
| `since` | `&'static str` | Schema version at which the term was deprecated |
| `replacement` | `Option<TokenId>` | Replacement token if known; `None` if no replacement exists (FR-017) |

### Codec<S> (trait surface only, no impls)

Pinned trait surface for grammar serialization. Phase E publishes the trait; Phase G implements XML and JSON round-trip against it without further trait evolution (FR-019, SC-010).

| Method | Signature | Meaning |
|---|---|---|
| `encode` | `fn encode(&self, marking: &S::Marking) -> Result<Vec<u8>, CodecError>` | Serialize a marking |
| `decode` | `fn decode(&self, bytes: &[u8]) -> Result<Parsed<S::Marking>, CodecError>` | Parse a serialized marking; `Parsed` preserves ambiguity awareness at the codec boundary (foundational-plan §9) |

**Invariants:**
- No concrete impls ship in Phase E (FR-019).
- Sufficient for Phase G XML + JSON round-trip without trait evolution (SC-010).

## Cross-cutting entities

### Citation convention (not a type)

Phase C/D/E does NOT introduce a `SourceCitation` struct. Citations are plain `&'static str` fields on `Constraint`, `ConstraintViolation`, `PageRewrite`, and `TokenMetadataFull` entries, per foundational-plan §7a (line 943, 984, 1156). Examples:

- `"CAPCO-2016 §H.4"`
- `"CAPCO-2016 §F.2 p43"`
- `"ODNI ISM-v2022-DEC, CVEnumISMDissem.xml"`

**Invariant:** Citations are verified at the point of commit (Constitution VIII, FR-021). An unverifiable citation is removed, not retained pending. Re-verification at citation propagation is enforced by T089. A structured `SourceCitation` with a `verified_at_commit: &'static str` SHA field is a reasonable Phase-F enhancement but is out of scope for this feature; its absence here is intentional alignment with foundational-plan.

### MangledMarkingFixture

One labeled entry in `tests/fixtures/mangled/`.

| Field | Type | Meaning |
|---|---|---|
| `observed` | `&'static str` | The mangled marking as it appears in a document |
| `expected` | `&'static str` | The canonical marking the decoder should resolve to |
| `mangling_class` | `ManglingClass` | One of `Typo`, `Reordering`, `MissingDelimiter`, `SupersededToken`, `WrongCase`, `GarbledDelimiter` |
| `source_confidence` | `f64` | Confidence that the observed→expected pair is a true mangling (from fixture generator) |

**Invariant (FR-008, SC-004):** Fixture size ≥ 200 across all six mangling classes. Generator in `tools/corpus-analysis/` produces the fixture from Enron high-confidence markings + labeled transforms.
