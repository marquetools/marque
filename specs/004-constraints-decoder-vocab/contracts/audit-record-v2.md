# Contract: Audit Record Schema v2 (`marque-mvp-2`)

**Crate:** `marque-engine` (emission) + `marque-rules` (types)
**Phase:** D
**Spec refs:** FR-009, FR-012, FR-014, SC-006, SC-007

## Intent

Every applied fix — whether produced by the strict path or the decoder — carries enough provenance for a compliance reviewer to answer "why was this chosen over the runner-up?" without reading the engine source. The schema bumps from `marque-mvp-1` to `marque-mvp-2`; back-compat is one-directional (v2-aware parsers read v1; v1 parsers do not read v2 without upgrade).

## Surface

```rust
pub struct Confidence {
    pub recognition: f32,
    pub rule: f32,
    pub region: Option<f32>,
    pub runner_up_ratio: Option<f32>,   // None for strict-path fixes with no runner-up
    pub features: Vec<FeatureContribution>,
}

pub struct FeatureContribution {
    pub id: FeatureId,   // ENUM — not String
    pub delta: f32,
}

pub enum FeatureId {
    EditDistance1,
    EditDistance2,
    TokenReorder,
    SupersededToken,
    BaseRateCommonMarking,
    StrictContextClassification,
    CorpusOverrideInEffect,
    // ... closed enum; grammar build fixes the variants
}

pub enum FixSource {
    BuiltinRule,        // existing — preserved
    CorrectionsMap,     // existing — preserved
    MigrationTable,     // existing — preserved
    DecoderPosterior,   // NEW in Phase D
}

pub struct AppliedFix {
    // existing v1 fields (preserved verbatim)
    pub rule: RuleId,
    pub original_text: Box<[u8]>,
    pub replacement_text: Box<[u8]>,
    pub timestamp: Timestamp,
    pub classifier_id: Option<String>,
    pub dry_run: bool,

    // v2 fields
    pub confidence: Confidence,
    pub source: FixSource,
}
```

**Precision:** `f32` throughout (not `f64`) per foundational-plan line 739-757. The decoder computes log-posteriors internally; surface precision is `f32` (SIMD-friendly, aligns with existing `Candidate<M>::prior_log_odds: f32` in `crates/scheme/src/ambiguity.rs`). Downcasting from `f64` decoder math to `f32` surface happens at the decoder→`Confidence` boundary.

**Naming:** `Confidence` (not `FixConfidence`) per foundational-plan. The struct replaces the current scalar `FixProposal.confidence: f32` — strict-path fixes construct `Confidence { recognition: 1.0, rule: <existing>, region: None, runner_up_ratio: None, features: vec![] }`.

**`runner_up_ratio: Option<f32>`:** `None` for strict-path fixes with no runner-up (cleaner than a sentinel `f32::INFINITY`). `Some(r)` for decoder-driven fixes where `r` is the top-over-second posterior ratio.

**`FixSource` variants:** The existing three variants (`BuiltinRule`, `CorrectionsMap`, `MigrationTable`) are preserved per foundational-plan line 716-726. Phase D adds ONLY `DecoderPosterior`. No rename of `BuiltinRule → RulePredicate`; no removal of `CorrectionsMap`.

## Contract

- **Single schema version per build (FR-014):** An engine binary emits exactly one schema version at runtime. Build flag `MARQUE_AUDIT_SCHEMA=marque-mvp-2` (or equivalent) is set at compile time, not runtime.
- **Back-compat for downstream consumers (FR-014, SC-006):** v1 records emitted by earlier builds continue to parse in v2-aware consumers. All v1 fields are a strict subset of v2.
- **Provenance sufficiency (SC-007):** For a decoder-driven fix, the combination of `confidence`, `runner_up_ratio`, and `features` MUST answer "why this candidate, not the next-best?" A compliance reviewer can accept or reject batch output based on these fields alone.
- **No free-form feature labels (FR-012):** `FeatureContribution.id` is `FeatureId` (enum). A free-form string is a type error. Threat-model T2 enforcement: the FeatureId enum is closed and every variant is a grammar-time declaration.
- **Content-ignorance (Constitution V):** `original_text` and `replacement_text` serialized in audit output contain only marking tokens — no surrounding document content, no metadata values, no subject-claim free-form text. Corpus-level integration tests grep audit output for document text and assert zero matches.
- **Classifier identity sourced from env/local config only (Constitution V):** `classifier_id` is populated from `MARQUE_CLASSIFIER_ID` or `.marque.local.toml`. Never from committed config.
- **Engine-only promotion:** Only `Engine::fix_inner` calls `AppliedFix::__engine_promote`. Rule crates, CLI binaries, and downstream consumers MUST NOT construct an `AppliedFix` directly. This invariant predates Phase D and remains unchanged.

## Failure modes

| Error | Trigger | Prevention |
|---|---|---|
| Mixed-schema output | A build emits v1 and v2 records in the same run | Compile-time schema selection; build fails if both emitters are linked |
| Document text in audit | A rule includes doc content in `original_text` / `replacement_text` | Corpus integration test greps audit output for document text |
| Free-form feature label | A rule constructs `FeatureContribution { id: SomeString, ... }` | Type error — `FeatureId` is an enum |

## Test scenarios

1. **Strict-path fix audit record:** A fix produced by the strict path has `confidence.recognition == 1.0_f32`, `confidence.region == None`, `confidence.runner_up_ratio == None`, `confidence.features == vec![]`, and `source` is one of `BuiltinRule`, `CorrectionsMap`, or `MigrationTable`.
2. **Decoder fix audit record:** A decoder-driven fix has `confidence.recognition < 1.0_f32`, `confidence.features` non-empty with all-enum `FeatureId` variants, `source == DecoderPosterior`, and `confidence.runner_up_ratio == Some(r)` where `r` is finite.
3. **Back-compat parse:** A v1 record serialized from a pre-Phase-D engine build parses in a v2-aware consumer without error; missing v2 fields are handled as optional.
4. **Single-version invariant:** A v2 build run against a document produces no v1 records in the same output stream.
5. **Content-ignorance:** Run the corpus harness; grep audit output for any document-text fragment; assert zero matches (Constitution V + test fixture for G13 invariant).
