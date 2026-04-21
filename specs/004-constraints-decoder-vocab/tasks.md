# Tasks: Declarative Rule Expression, Probabilistic Recovery, and Full Vocabulary Metadata (Phases C–E)

**Input**: Design documents from `specs/004-constraints-decoder-vocab/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included. The spec's acceptance scenarios, success criteria (SC-001 – SC-010), and quickstart walk-throughs all name specific tests (corpus-parity harness, decoder-accuracy harness, WASM compile-fail, Send+Sync static assertions, content-ignorance greps). Tests are FIRST-class deliverables for this feature.

**Organization**: Tasks are grouped by user story. Each story is independently testable and delivers value on its own per the spec.

## Format: `- [ ] [ID] [P?] [Story?] Description with file path`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Scaffolding that every user story consumes.

- [X] T001 Add `corpus-override`, `corpus-harness`, `decoder-harness` Cargo features across relevant crates (`crates/capco/Cargo.toml`, `crates/config/Cargo.toml`, `crates/engine/Cargo.toml`, `crates/wasm/Cargo.toml`, `marque/Cargo.toml`), with WASM target explicitly disabling `corpus-override` via `default-features = false`
- [X] T002 [P] Create mangled-fixture directory tree `tests/fixtures/mangled/{typo,reordering,missing-delimiter,superseded-token,wrong-case,garbled-delimiter}/` with a top-level `tests/fixtures/mangled/README.md` documenting the six classes and fixture schema
- [X] T003 [P] Create corpus-priors directory `crates/capco/corpus/` with a `README.md` describing the priors JSON format and regeneration command (`python3 tools/corpus-analysis/analyze.py --mode priors --output crates/capco/corpus/priors.json`)
- [X] T004 [P] Extend `tools/corpus-analysis/analyze.py` (Python tool, not a Cargo crate — the Enron corpus analysis lives in Python and marque consumes its JSON output at build time per foundational-plan §4.4 and §6.1) to expose two new modes: `--mode mangled` emits labeled mangled-marking fixtures to `tests/fixtures/mangled/<class>/`, and `--mode priors` emits the corpus-derived priors table to a caller-specified JSON path. Update `tools/corpus-analysis/README.md` and `requirements.txt` accordingly
- [ ] T004a [P] **DEFERRED to Phase 4 alongside T042.** The build.rs fails-closed-on-missing contract conflicts with T042 being the task that actually produces `crates/capco/corpus/priors.json`. Landing T004a in Phase 1 as originally written would break `cargo build` on every clean checkout until someone with Enron-corpus access runs the Python tool. Deferring keeps Phase 1 unblocking of downstream work while preserving the closed-loop invariant once T042 lands the artifact. Original task body: "Extend `crates/capco/build.rs` to read `crates/capco/corpus/priors.json` via `serde_json` at compile time and emit `&'static` corpus-priors tables into `OUT_DIR/priors.rs` (included via `crates/capco/src/priors.rs`). Build fails closed if the JSON is missing or malformed — matches foundational-plan line 679 ('build.rs emits the baked frequency table'). No runtime JSON parsing; no runtime `serde_json` runtime dep."
- [X] T005 [P] Add compile-time audit-schema selection via build script env var in `crates/engine/build.rs` (`MARQUE_AUDIT_SCHEMA`, defaulting to `marque-mvp-2`), emitting `cargo:rustc-env=MARQUE_AUDIT_SCHEMA=...`

**Checkpoint**: Feature flags, fixture tree, priors directory, and audit-schema plumbing exist. No logic yet.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Type and trait surface that every user story builds on. No logic here — just the shapes that let compilation succeed in parallel across all three stories.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [ ] T006 [P] Extend the existing `Constraint` enum in `crates/scheme/src/constraint.rs` to carry citation information per variant — add a `label: &'static str` (or equivalent per-variant field) holding the authoritative-source passage (e.g., `"CAPCO-2016 §H.4"`). Do NOT introduce `SourceCitation` or `SourceRef` types — foundational-plan §7a stores citations as plain `&'static str`. Do NOT reshape `Constraint` into a generic struct `Constraint<S>` — the Phase-B non-generic enum is authoritative per foundational-plan §12 Phase C
- [ ] T007 [P] Add a `citation: &'static str` field to `ConstraintViolation` in `crates/scheme/src/constraint.rs` (preserves `constraint_label` and `message` fields unchanged) — carrying the triggering constraint's authoritative-source passage verbatim (FR-021 + Constitution VIII). No `DiagnosticTemplate` type — rule IDs and severities live in existing rule-emission plumbing, not a new type
- [ ] T008 [P] Extend `PageRewrite` in `crates/scheme/src/page_rewrite.rs` (existing type) with `reads: &'static [CategoryId]` and `writes: &'static [CategoryId]` fields; extend `CategoryAction` with the `Promote { from: CategoryId, to: CategoryId, transform: fn(&S::Marking) -> S::Marking }` variant (additive — preserves existing `Clear`, `Replace`, `Custom`). Keep `citation: &'static str` (do not migrate to `SourceCitation`). Constructor stubs only (`declarative`/`custom`); derivation logic deferred to US1
- [ ] T009 [P] Define `Recognizer<S>` trait with `Send + Sync` supertrait bound and `ParseContext { strict_evidence, zone, position }` in `crates/scheme/src/recognizer.rs`. **Reuse the existing `Parsed<M>` type from `crates/scheme/src/ambiguity.rs`** — two variants (`Unambiguous(M)`, `Ambiguous { candidates: Vec<Candidate<M>> }`) per foundational-plan line 520-527; the zero-candidate case is `Parsed::Ambiguous { candidates: vec![] }` per foundational-plan line 609-612 ("Never a silent fallthrough"). Do NOT add a third `Unrecognized` variant or flatten `Candidate<M>` to `(M, f64)` — the rich `Candidate<M>` carries `evidence: Vec<EvidenceFeature>` and `prior_log_odds: f32` that the G5 provenance chain depends on
- [ ] T010 [P] Define `Vocabulary<S>` trait, `TokenMetadataFull`, `Authority`, `OwnerProducer`, `PointOfContact`, and `Deprecation { since, replacement }` in `crates/scheme/src/vocabulary.rs`
- [ ] T011 [P] Define `Codec<S>` trait (`encode`/`decode`) and `CodecError` (`Malformed`, `UnsupportedFormat`, `SchemaMismatch`) in `crates/scheme/src/codec.rs`
- [ ] T012 [P] Expose the new modules from `crates/scheme/src/lib.rs` (`pub mod recognizer; pub mod vocabulary; pub mod codec;`) and re-export key types. Note: `constraint` and `page_rewrite` modules already exist from Phase B; no new `citation` module is created (plain `&'static str` citation fields obviate it)
- [ ] T013 [P] Define `Confidence { recognition: f32, rule: f32, region: Option<f32>, runner_up_ratio: Option<f32>, features: Vec<FeatureContribution> }`, `FeatureContribution { id: FeatureId, delta: f32 }`, and closed `FeatureId` enum (EditDistance1, EditDistance2, TokenReorder, SupersededToken, BaseRateCommonMarking, StrictContextClassification, CorpusOverrideInEffect) in `crates/rules/src/confidence.rs` and expose from `crates/rules/src/lib.rs`. Name is `Confidence` (not `FixConfidence`) and precision is `f32` throughout per foundational-plan line 739-757
- [ ] T014 [P] Add `FixSource::DecoderPosterior` variant to the existing enum in `crates/rules/src/lib.rs` (preserving the existing `BuiltinRule`, `CorrectionsMap`, and `MigrationTable` variants — do not rename or drop any). Extend `FixProposal` to carry `Confidence` (replacing scalar `confidence: f32`); update all existing call sites with `Confidence::strict(rule_confidence)` helper
- [ ] T015 [P] Extend `AppliedFix` struct with v2 fields (`confidence: Confidence`, `source: FixSource`) in `crates/rules/src/lib.rs`; update `AppliedFix::__engine_promote` signature accordingly (engine-only; `pub #[doc(hidden)]`)
- [ ] T016 [P] Add `Send + Sync` supertrait bound to the existing `Rule` trait in `crates/rules/src/lib.rs` (Constitution VI + FR-023)
- [ ] T017 Add `EngineConstructionError::RewriteCycle { axis: CategoryId, members: &'static [RewriteId] }` and `EngineConstructionError::UnannotatedCustomAxes { rewrite: RewriteId }` variants in `crates/engine/src/errors.rs`. `members` is a slice (not `[RewriteId; 2]`) because cycles ≥3 rewrites are a legitimate failure mode per foundational-plan line 1066. If `crates/engine/src/errors.rs` doesn't yet exist, create it as part of this task and expose from `crates/engine/src/lib.rs`
- [ ] T018 [P] `static_assertions`-style compile test proving `Box<dyn Recognizer<CapcoScheme>>` is `Send + Sync` in `crates/scheme/tests/send_sync.rs` (Rule trait equivalent test already implied by FR-023; extend here for Recognizer)

**Checkpoint**: Workspace compiles. Every user story's implementation can now land against stable shapes.

---

## Phase 3: User Story 1 — Declarative constraints + page-rewrites (Priority: P1) 🎯 MVP

**Goal**: Migrate ~15 of CAPCO's 39 hand-written rules to declarative `Constraint` / `PageRewrite` data evaluated by a single evaluator in `marque-scheme` and scheduled deterministically by `Engine::new`. Corpus output stays byte-identical.

**Independent Test**: Run the CAPCO corpus harness on `main` (baseline) and on this branch (post-migration); `diff` the diagnostic output — it is byte-identical. Rule count drops from 39 to ~24.

### Tests for User Story 1 (write first, ensure they fail, then implement)

- [ ] T019 [P] [US1] Scheduler cycle rejection test `cyclic_rewrite_pair_fails_construction` in `crates/engine/tests/scheduler.rs` (synthetic A writes X/reads Y, B writes Y/reads X; expect `RewriteCycle` with `members` slice containing both A and B). A second subtest exercises a 3-rewrite cycle to verify the slice handles `members.len() > 2`
- [ ] T020 [P] [US1] Scheduler unannotated-custom rejection test `unannotated_custom_rewrite_fails_construction` in `crates/engine/tests/scheduler.rs` (builds `PageRewrite::custom` with `reads: &[]`; expect `UnannotatedCustomAxes`)
- [ ] T021 [P] [US1] Scheduler declaration-order independence test `scheduled_order_independent_of_declaration` in `crates/engine/tests/scheduler.rs` (permute three real rewrites; assert identical scheduled order and identical diagnostic output)
- [ ] T022 [P] [US1] Scheduler real producer-consumer edge test `joint_promotion_before_fgi_absorption` in `crates/engine/tests/scheduler.rs` (JOINT-promotion writes `fgi`; FGI-absorption reads `fgi`; verify scheduled order)
- [ ] T023 [P] [US1] Evaluator determinism test `evaluate_is_deterministic` in `crates/scheme/tests/evaluator.rs` (same input on two threads returns identical violation vec)
- [ ] T024 [P] [US1] Evaluator empty-constraints test `empty_constraints_returns_empty` in `crates/scheme/tests/evaluator.rs`
- [ ] T025 [P] [US1] Evaluator cite-on-violation test `conflict_violation_preserves_citation` in `crates/scheme/tests/evaluator.rs` (triggers a `Conflicts` constraint; asserts `ConstraintViolation.citation` equals the declared `&'static str` citation on the triggering constraint verbatim)
- [ ] T026 [P] [US1] Corpus-parity baseline-vs-branch harness runner script in `crates/capco/tests/corpus_parity.rs::phase_c_migration_is_byte_identical` (reads baseline manifest from `tests/corpus/baseline-pre-phase-c.manifest` and compares current run)

### Implementation for User Story 1

- [ ] T027 [US1] Implement `evaluate<S: MarkingScheme>(constraints, marking) -> Vec<ConstraintViolation>` free function in `crates/scheme/src/constraint.rs` (deterministic, declaration-ordered, no per-constraint allocation beyond output vec)
- [ ] T028 [US1] Wire `MarkingScheme::validate` to call `evaluate(self.constraints(), marking)` and append scheme-specific non-constraint validations in `crates/scheme/src/scheme.rs`
- [ ] T029 [US1] Implement `PageRewrite::declarative(id, trigger, action, citation) -> Self` const constructor deriving `reads`/`writes` from `trigger`/`action` enum variants in `crates/scheme/src/rewrite.rs`
- [ ] T030 [US1] Implement `PageRewrite::custom(id, trigger, action, reads, writes, citation) -> Self` const constructor with const-fn guard rejecting empty `reads` or `writes` (compile-fail path) in `crates/scheme/src/rewrite.rs`
- [ ] T031 [US1] Implement topological sort of rewrites at `Engine::new` (Kahn's algorithm over read/write axis edges) returning `EngineConstructionError::RewriteCycle` or `::UnannotatedCustomAxes` on failure in `crates/engine/src/engine.rs`
- [ ] T032 [US1] Store scheduled rewrite order on `Engine` and drive `PageContext` roll-up through the pre-computed order in `crates/engine/src/engine.rs` (no per-document re-sort)
- [ ] T033 [P] [US1] Author 12 declarative `Constraint<CapcoScheme>` entries in `crates/capco/src/scheme.rs`, one per migrated rule. Migration set:
  - E010 `BareHcsRule` — `HCS` requires a qualifying variant (`HCS-P` or `HCS-O`); cite CAPCO-2016 §H.4 HCS subsystem table
  - E012 `DualClassificationRule` — US classification conflicts with concurrent foreign classification; cite §I.3 "Dual/Mixed Classification"
  - E015 `NonUsMissingDissemRule` — non-US classification REQUIRES dissem control; cite §K "Foreign Government Information" p61
  - E016 `JointRestrictedRule` — JOINT conflicts with RESTRICTED; cite §K.2 "Joint Classification"
  - E017 `JointFgiRule` — JOINT conflicts with FGI marker; cite §K.2
  - E018 `JointIcDissemRule` — JOINT conflicts with IC dissem controls (except REL TO); cite §K.2 p66
  - E019 `JointNonIcDissemRule` — JOINT conflicts with non-IC dissem; cite §K.2 p66
  - E021 `AeaNofornRule` — RD/FRD REQUIRES NOFORN; cite §H.1 "Atomic Energy Information"
  - E022 `CnwdiConstraintRule` — CNWDI REQUIRES TS or S classification (implication with classification floor); cite §H.1
  - E024 `RdPrecedenceRule` — RD SUPERSEDES FRD/TFNI when both present; cite §H.1
  - E025 `UcniClassificationRule` — UCNI CONFLICTS with classified markings; cite §H.1 UCNI subsection
  - W002 `CominglingWarningRule` — US classification alongside FGI marker (warning severity); cite §K.2
- [ ] T034 [P] [US1] Author 3 declarative `PageRewrite<CapcoScheme>` entries using `PageRewrite::declarative` in `crates/capco/src/scheme.rs`:
  - `capco/noforn-clears-rel-to` — NOFORN supersedes REL TO at page scope; reads `rel_to` + `dissem`, writes `rel_to`; cite §F.2 p43
  - `capco/joint-promotion` — JOINT countries promote into REL TO (subsumes E014 `JointRelToRule`'s requirement logic); reads `joint_countries`, writes `rel_to`; cite §K.2
  - `capco/fgi-absorption` — FGI tokens roll up from portions to banner; reads `fgi`, writes `fgi`; cite §K p61
- [ ] T035 [US1] Retire the 12 corresponding hand-written `Rule` impls from `crates/capco/src/rules.rs` (`BareHcsRule`, `DualClassificationRule`, `NonUsMissingDissemRule`, `JointRestrictedRule`, `JointFgiRule`, `JointIcDissemRule`, `JointNonIcDissemRule`, `AeaNofornRule`, `CnwdiConstraintRule`, `RdPrecedenceRule`, `UcniClassificationRule`, `CominglingWarningRule`) and remove their registrations from `CapcoRuleSet::new()` at `crates/capco/src/rules.rs:58`. Retire E014 `JointRelToRule` and the lattice-side NOFORN⊐REL-TO / FGI-absorption code as it is subsumed by the three rewrites from T034. Net: 15 retirements (12 + E014 + 2 lattice behaviors → 3 `PageRewrite` entries; rule count drops 39 → 24).
- [ ] T036 [US1] Register the new `Constraint` + `PageRewrite` sets on `CapcoScheme` via `constraints()` and `rewrites()` methods in `crates/capco/src/scheme.rs`
- [ ] T037 [US1] Capture the corpus-parity baseline BEFORE any Phase-C migration edit lands. Check out the branch-off commit (SHA recorded in `tests/corpus/baseline-pre-phase-c.manifest` header as `baseline_sha: <short-sha>`), run `cargo test -p marque-capco --features corpus-harness -- --nocapture`, and commit the captured output to `tests/corpus/baseline-pre-phase-c.manifest`. The baseline is pinned — it does not drift if `main` advances.
- [ ] T038 [US1] Run corpus harness on this branch; resolve any drift from baseline until diff is empty; commit result

**Checkpoint**: US1 delivers independent value — rule count reduced, maintenance cost down, declarative pattern set for future schemes. Corpus diagnostics unchanged byte-for-byte.

---

## Phase 4: User Story 2 — Probabilistic recognizer with audit provenance (Priority: P2)

**Goal**: Introduce `DecoderRecognizer` behind the `Recognizer` trait, with K=8 bounded candidate generation and full audit provenance. Resolve ≥85% of a ≥200-case labeled mangled-marking fixture at aggregate confidence ≥0.85. CLI accepts corpus override; server rejects it; WASM excludes it at compile time.

**Independent Test**: Run decoder-accuracy harness against the generated fixture; verify ≥85% resolution at the confidence threshold. Inspect audit records from decoder-driven fixes; verify they carry recognition confidence, runner-up ratio, and enumerated feature IDs.

### Fixture + priors preparation

- [ ] T039 [P] [US2] Implement `--mode mangled` in `tools/corpus-analysis/analyze.py` (Python) — reads the Enron corpus from `$MARQUE_ENRON_CORPUS`, applies the six labeled mangling transforms (typo, reordering, missing-delimiter, superseded-token, wrong-case, garbled-delimiter), and emits one JSON file per case under `tests/fixtures/mangled/<class>/` with fields `observed`, `expected`, `mangling_class`, `source_confidence`
- [ ] T040 [P] [US2] Implement `--mode priors` in `tools/corpus-analysis/analyze.py` (Python) — produces `crates/capco/corpus/priors.json` containing token base rates, template base rates, and strict-context priors derived from the Enron high-confidence subset. This is the JSON artifact consumed by `crates/capco/build.rs` at compile time (T004a)
- [ ] T041 [US2] Run `python3 tools/corpus-analysis/analyze.py --mode mangled --output tests/fixtures/mangled/ --min-cases 200` to produce ≥200 labeled cases spread across the six mangling classes; commit `tests/fixtures/mangled/**/*.json` (depends on T039)
- [ ] T042 [US2] Run `python3 tools/corpus-analysis/analyze.py --mode priors --output crates/capco/corpus/priors.json` to produce `crates/capco/corpus/priors.json`; commit this artifact (depends on T040). **Bundled with this task: land T004a (deferred from Phase 1) — extend `crates/capco/build.rs` with the `serde_json` compile-time codepath that reads the priors JSON and emits `OUT_DIR/priors.rs`. Must land in the same commit as the artifact so `cargo build` stays green from the moment the artifact is committed; the fails-closed-on-missing contract then holds thereafter.**

### Tests for User Story 2 (write first)

- [ ] T043 [P] [US2] StrictRecognizer + DecoderRecognizer compile-time Send+Sync assertion in `crates/capco/tests/recognizer_send_sync.rs`
- [ ] T044 [P] [US2] Zero-candidate signal test (FR-015) `fits_no_template_returns_zero_candidate_ambiguous` in `crates/capco/tests/decoder_no_template.rs` (input `FROBNITZ//WIBBLE` returns `Parsed::Ambiguous { candidates: vec![] }` — the zero-candidate form per foundational-plan line 609-612, NOT a third `Unrecognized` variant, NOT a strict-path error, NOT a lower-confidence guess)
- [ ] T045 [P] [US2] Strict-context floor test (FR-011) `strict_confidential_blocks_c_as_copyright` in `crates/capco/tests/decoder_strict_context.rs` (document has `(S)` elsewhere + isolated `(C)`; decoder's candidate set excludes the copyright resolution entirely)
- [ ] T046 [P] [US2] Typo-to-canonical test (US2.1) `sercet_resolves_to_secret_with_features` in `crates/capco/tests/decoder_typo.rs` (asserts `FeatureId::EditDistance1` appears in feature trace)
- [ ] T047 [P] [US2] Banner-reordering test (US2.3) `dissem_before_sci_canonicalized` in `crates/capco/tests/decoder_reorder.rs` (asserts `FeatureId::TokenReorder` appears)
- [ ] T048 [P] [US2] Interactive-no-decoder test (US2.4) `no_deep_scan_skips_decoder` in `crates/capco/tests/decoder_optin.rs` (runs `Engine::lint` without `--deep-scan`; asserts decoder never fires and latency stays in strict-path envelope)
- [ ] T049 [P] [US2] Server rejects corpus-override body field test `rejects_corpus_override_body` in `crates/server/tests/http.rs` (POST `/v1/fix` with `"corpus_override": {...}` JSON body → `400`)
- [ ] T050 [P] [US2] Server rejects corpus-override header test `rejects_corpus_override_header` in `crates/server/tests/http.rs` (POST `/v1/fix` with `X-Marque-Corpus-Override: ...` → `400`)
- [ ] T051 [P] [US2] WASM compile-fail test `no_corpus_override_in_wasm` in `crates/wasm/tests/no_corpus_override.rs` (uses `#[cfg(feature = "corpus-override")] compile_error!` and verifies `cargo build --target wasm32-unknown-unknown -p marque-wasm` succeeds only without the feature)
- [ ] T052 [P] [US2] Audit v2 strict-path record test `strict_path_record_shape` in `crates/engine/tests/audit.rs` (asserts `confidence.recognition == 1.0_f32`, `confidence.runner_up_ratio == None`, `confidence.features == vec![]`, `source ∈ {BuiltinRule, CorrectionsMap, MigrationTable}`)
- [ ] T053 [P] [US2] Audit v2 decoder-path record test `decoder_path_record_shape` in `crates/engine/tests/audit.rs` (asserts `confidence.recognition < 1.0_f32`, non-empty `confidence.features` with enum-typed `FeatureId`, `source == DecoderPosterior`, `confidence.runner_up_ratio == Some(r)` with finite `r`)
- [ ] T054 [P] [US2] Audit v2 back-compat parse test `v1_records_parse_in_v2_consumer` in `crates/engine/tests/audit.rs` (fixture v1 record from pre-Phase-D engine deserializes without error)
- [ ] T055 [P] [US2] Audit v2 single-version invariant test `single_schema_per_build` in `crates/engine/tests/audit.rs` (no v1 records appear in v2-build output stream)
- [ ] T056 [P] [US2] Audit v2 content-ignorance test `no_document_text_leaks_into_audit` in `crates/engine/tests/audit.rs` (runs corpus harness, greps audit output stream for any document-text fragment outside marking tokens, asserts zero matches — Constitution V + G13 invariant)
- [ ] T057 [P] [US2] Decoder accuracy harness test `resolution_rate_at_0_85` in `crates/capco/tests/decoder_accuracy.rs` (reads `tests/fixtures/mangled/**/*.json`, runs DecoderRecognizer with confidence ≥0.85, asserts ≥85% resolution rate) — SC-004 gate

### Implementation for User Story 2

- [ ] T058 [US2] Implement `StrictRecognizer<CapcoScheme>` wrapping the existing parser from `marque-core` behind the `Recognizer` trait in `crates/capco/src/scheme.rs`. If the current `marque-core` parser entry point is module-private, promote the minimum surface needed (prefer a single `pub fn parse_candidate(span, zone) -> IsmAttributes`) to `pub` in `crates/core/src/lib.rs`. Do NOT widen the surface further; record any `pub` promotions in the PR body so reviewers can audit.
- [ ] T059 [US2] Implement bounded `CandidateGenerator` (edit-distance ≤2 with per-token length cap; token reordering; superseded-token substitution; case normalization) producing at most K=8 candidates per grammar template in `crates/capco/src/decoder.rs`
- [ ] T060 [US2] Bake corpus priors from `crates/capco/corpus/priors.json` into `&'static` Rust tables at build time in `crates/capco/build.rs`, emitting `OUT_DIR/priors.rs` for inclusion from `crates/capco/src/priors.rs`
- [ ] T061 [US2] Implement `DecoderRecognizer::recognize` — bag-of-tokens Bayesian scoring over candidates, log-posterior combination, `runner_up_ratio` computation, feature-contribution accumulation with enumerated `FeatureId` — in `crates/capco/src/decoder.rs`
- [ ] T062 [US2] Wire FR-011 strict-context floor: `DecoderRecognizer` reads `ParseContext.strict_evidence`, rejects any candidate below the observed strict classification floor before scoring in `crates/capco/src/decoder.rs`
- [ ] T063 [US2] Rewire `Engine::lint_inner` to dispatch parsing via `Box<dyn Recognizer<S>>` (strict by default; decoder when `deep_scan` is set or a rule escalates a region) in `crates/engine/src/engine.rs`
- [ ] T064 [US2] Add `--deep-scan` CLI flag + `fix --deep-scan` subcommand plumbing in `marque/src/cli.rs` (or equivalent entry point) and thread through to `Engine`
- [ ] T065 [US2] Add `--corpus-override <file>` CLI flag behind `#[cfg(feature = "corpus-override")]` in `marque/src/cli.rs` and corpus-override parsing in `crates/config/src/corpus_override.rs`
- [ ] T066 [US2] Implement handler-level rejection of corpus-override in request body / headers / query params in `crates/server/src/handlers.rs` (returns `400` with audit log entry; attempted override contents NOT logged)
- [ ] T067 [US2] Configure `crates/wasm/Cargo.toml` to omit `corpus-override` feature on WASM target and add `compile_error!` guard in `crates/wasm/src/lib.rs` under `#[cfg(all(target_arch = "wasm32", feature = "corpus-override"))]`
- [ ] T067a [P] [US2] Expose `lint_deep_scan(bytes)` and `fix_deep_scan(bytes)` wasm-bindgen exports in `crates/wasm/src/lib.rs`; dispatch via DecoderRecognizer with baked-in priors. Neither export accepts any parameter beyond the byte buffer (FR-013a + Gate 2 enforcement).
- [ ] T067b [P] [US2] WASM deep-scan parity test `wasm_deep_scan_matches_cli` in `crates/wasm/tests/deep_scan_parity.rs` — given a mangled input, `lint_deep_scan` emits the same diagnostics and audit fields as `marque fix --deep-scan` (SC-008 parity extended to decoder path)
- [ ] T067c [P] [US2] WASM API-surface test `wasm_exports_have_no_prior_config_parameter` in `crates/wasm/tests/api_surface.rs` — compile-time assertion that `lint_deep_scan` / `fix_deep_scan` signatures take only `&[u8]` (no config struct parameter)
- [ ] T068 [US2] Emit v2 audit records in `Engine::fix_inner` populating `confidence` (including `runner_up_ratio` and `features` within it) and `source` for every `AppliedFix` (strict-path fixes set `recognition=1.0_f32`, `runner_up_ratio=None`, `features=vec![]`, `source ∈ {BuiltinRule, CorrectionsMap, MigrationTable}`) in `crates/engine/src/engine.rs` (or `fix.rs` if extracted). `Engine::fix_inner` already exists per the current code at `crates/engine/src/engine.rs:325`; this task extends it, not creates it
- [ ] T069 [P] [US2] Add `CorpusOverrideInEffect` feature contribution emitted when CLI `--corpus-override` is active in `crates/engine/src/fix.rs`
- [ ] T070 [P] [US2] Add `decoder_10kb_one_mangled_region` Criterion bench in `crates/engine/benches/lint_latency.rs` with p95 ≤18 ms regression gate — SC-002

**Checkpoint**: US2 delivers independent value — compliance staff can run backlog reconciliation with auditable posterior scores per fix. The corpus-override threat (T3) is enforced at the right place per target.

---

## Phase 5: User Story 3 — Full vocabulary metadata + codec scaffolding (Priority: P3)

**Goal**: Restore every ODNI-published per-term metadata field (authority, owner/producer, POC, deprecation, URN, schema version, portion/banner forms) through a single trait surface with zero runtime allocation. Remove the factually-wrong `FOUO → CUI` migration entry. Publish the `Codec<S>` trait surface with no implementations.

**Independent Test**: Every active CAPCO token returns populated metadata via the `Vocabulary` trait; FOUO remains active and unmigrated; corpus diagnostics stay byte-identical after the migration-entry removal; `Codec<S>` compiles.

### Tests for User Story 3 (write first)

- [ ] T071 [P] [US3] Every-active-token-has-authority test `every_active_token_has_authority` in `crates/capco/tests/vocabulary.rs` (iterates the full CAPCO token set; asserts `authority()`, `owner_producer()`, `point_of_contact()`, `portion_form()`, `banner_form()` all return populated data)
- [ ] T072 [P] [US3] Authority-points-to-ODNI test `authority_points_to_odni_for_ism_tokens` in `crates/capco/tests/vocabulary.rs` (asserts `authority(T).source` equals `"ODNI ISM-v2022-DEC"` as a `&'static str` for ISM tokens — `Authority` stores source provenance as plain string fields, not a structured enum)
- [ ] T073 [P] [US3] Deprecated-tokens-carry-deprecation test `deprecated_tokens_carry_deprecation` in `crates/capco/tests/vocabulary.rs`
- [ ] T074 [P] [US3] Replacement-when-known test `deprecation_replacement_when_known` in `crates/capco/tests/vocabulary.rs` (e.g., `NF → NOFORN`)
- [ ] T075 [P] [US3] FOUO-not-migrated test `fouo_is_not_migrated` in `crates/ism/tests/migrations.rs` (migration table lookup for FOUO returns `None`; FOUO has no `deprecation` entry)
- [ ] T076 [P] [US3] FOUO-remains-active test `fouo_remains_active_dissem_control` in `crates/capco/tests/vocabulary.rs` (runs a FOUO-bearing document through the corpus harness; asserts zero CUI-migration suggestions)
- [ ] T077 [P] [US3] Zero-allocation metadata query test `metadata_query_is_zero_alloc` in `crates/capco/tests/vocabulary.rs` (uses allocation-counter instrumentation; asserts repeated `metadata()` calls allocate 0 bytes)
- [ ] T078 [P] [US3] Codec surface compile test `codec_compiles_without_impls` in `crates/scheme/tests/codec_surface.rs` (just asserts `marque-scheme` compiles with `Codec<S>` defined and no concrete impls)
- [ ] T089b [P] [US3] Phase-F readiness stub test `second_scheme_builds_without_engine_edits` in `crates/scheme/tests/adoption_readiness.rs` — define a minimal `StubScheme` exercising every Phase-E trait (`MarkingScheme`, `Vocabulary`, `Codec` surface, `Recognizer`, declarative `Constraint` + `PageRewrite`); compile test asserts no engine-side crate is imported and no trait gap blocks construction. Does NOT implement a real grammar — just shows the surface is closed (SC-010 pre-verification).
- [ ] T079 [P] [US3] Migration-audit URNs test `migration_audit_has_both_urns` in `crates/engine/tests/audit.rs` (an applied fix that maps `NF → NOFORN` emits an audit record whose source and replacement URNs are both present)

### Implementation for User Story 3

- [ ] T080 [US3] Extend `crates/ism/build.rs` with a JSON codepath reading the ODNI ISM JSON sidecar from `crates/ism/schemas/ISM-v2022-DEC/` (via `serde_json`) and collecting per-term `TokenMetadataFull` records
- [ ] T081 [US3] Emit `TokenMetadataFull` const tables + per-`TokenId` lookup (`phf` or match-based) into `OUT_DIR/vocabulary.rs`; include via `crates/ism/src/generated.rs`
- [ ] T082 [US3] Keep the existing XML codepath (XSD + Schematron predicates) unchanged in `crates/ism/build.rs`; the JSON and XML codepaths are both active — neither is a fallback (FR-018 + R5)
- [ ] T083 [US3] Remove the `FOUO → CUI` migration table entry from `crates/ism/build.rs` migration-emission logic (FR-020); verify no other consumer relied on it
- [ ] T084 [US3] Implement `impl Vocabulary<CapcoScheme> for CapcoScheme` in `crates/capco/src/vocabulary.rs` — every accessor returns `&'static` data via direct index into the generated tables; expose `CapcoScheme::vocabulary()` where needed
- [ ] T085 [US3] Run the existing CAPCO corpus harness after the FOUO migration removal; confirm byte-identical diagnostics (FR-020 + US3.3)

**Checkpoint**: US3 delivers independent value — rule code and audit records can cite the full authority chain; the factual error in the migration table is corrected; Phase G has a pinned codec surface to implement against.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Regression gates, documentation, and end-to-end validation across all three stories.

- [ ] T086 [P] Add or update SC-001 strict-path p95 regression gate (`strict_lint_10kb_p95 ≤ 16 ms`) in `crates/engine/benches/lint_latency.rs`
- [ ] T087 [P] Verify SC-005 linear batch scaling (≥0.9 linearity coefficient on throughput vs. worker count) in `crates/engine/benches/linear_scaling.rs`
- [ ] T088 [P] Run WASM parity harness against the full CAPCO corpus via `wasm-pack test --node` in `crates/wasm/tests/parity.rs`; assert byte-identical NDJSON diagnostics vs. native `marque check` output (SC-008 parity, extended to post-Phase-D output)
- [ ] T089 Citation-verification pass: for every new/modified `Constraint`, `PageRewrite`, and `TokenMetadataFull` entry added in this branch, verify its `&'static str` citation points to a real passage in `crates/capco/docs/CAPCO-2016.md` or `crates/ism/schemas/ISM-v2022-DEC/`; remove (do not defer) any citation that cannot be traced — Constitution VIII + FR-021 + SC-009
- [ ] T089a Add a "Scheme-adoption PR checklist" section to `CONTRIBUTING.md` (create if absent) listing FR-022's invariants for Phase F onward: (1) adoption PR MUST NOT edit `marque-engine`, `marque-scheme`, `marque-core`, `marque-rules`, `marque-ism`; (2) engine gaps MUST land in a separate predecessor PR; (3) every new scheme crate follows the `build.rs` → generated-predicates pattern; (4) every new vocabulary entry cites a verified passage in its scheme's primary source
- [ ] T090 [P] Update `CLAUDE.md` sections "Active Technologies", "Recent Changes", "Two-Layer Rule Architecture", and "Key Types" to reflect `Recognizer`, `Vocabulary`, `Codec<S>`, `Confidence`, audit v2, and the topological scheduler
- [ ] T091 [P] Update `crates/capco/README.md` with the declarative `Constraint` / `PageRewrite` pattern and a worked example
- [ ] T092 Execute the full quickstart (`specs/004-constraints-decoder-vocab/quickstart.md`) end-to-end — all three US walk-throughs and the final CI one-liner — and record results
- [ ] T093 Final corpus regression: run `cargo test -p marque-capco --features corpus-harness` against the full CAPCO corpus; confirm diagnostic output is byte-identical to the pre-branch baseline captured in T037

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies. Start immediately.
- **Phase 2 (Foundational)**: Depends on Phase 1 (specifically T001 Cargo features). BLOCKS all user stories.
- **Phase 3 (US1)**: Depends on Phase 2. Delivers MVP independently.
- **Phase 4 (US2)**: Depends on Phase 2. Can proceed in parallel with US1 and US3 after Phase 2 completes. Fixture tasks (T039–T042) gate the decoder-accuracy test (T057) but are independent of US1/US3.
- **Phase 5 (US3)**: Depends on Phase 2. Can proceed in parallel with US1 and US2 after Phase 2 completes.
- **Phase 6 (Polish)**: Depends on all three user stories being complete.

### User Story Dependencies

All three user stories are independent in principle: each delivers value on its own. In practice:

- **US1 (P1)**: MVP. Must land first to unlock the "declarative data, not code" claim and provide the byte-identical corpus baseline that US2 and US3 preserve.
- **US2 (P2)**: Depends on Phase 2 only. The audit-v2 schema bump affects US1's migrated rules (they emit v2 records) but the plumbing (T015, T068) handles that — US1 does not need to finish before US2 starts coding.
- **US3 (P3)**: Depends on Phase 2 only. The FOUO-removal regression (T085) must pass the same corpus baseline US1 establishes, but the engineering can happen in parallel.

### Within Each User Story

- Tests MUST be written first and MUST FAIL before implementation starts (TDD per spec's implied contract).
- Types/constructors before logic (already satisfied by Phase 2).
- Fixture generation before decoder-accuracy test (T039 → T041 → T057).
- Priors generation before `DecoderRecognizer::recognize` (T040 → T042 → T061).

### Parallel Opportunities

- **Phase 1**: T002, T003, T004, T005 run in parallel after T001 lands.
- **Phase 2**: T006–T016 all `[P]` — different files, no interdependencies. T017 waits on `errors.rs` being touched; T018 waits on T009.
- **Phase 3 (US1)**: All test files (T019–T026) are `[P]`. Evaluator (T027) + scheme adapter (T028) and scheduler (T031, T032) can proceed in parallel with constraint/rewrite authoring (T033, T034).
- **Phase 4 (US2)**: All tests (T043–T057) are `[P]`. Fixture generation (T039, T040) parallelizes with recognizer implementation tasks (T058–T062). CLI / server / WASM / audit tasks (T064–T069) mostly land in different files and are `[P]` with each other.
- **Phase 5 (US3)**: All tests (T071–T079) are `[P]`. Implementation (T080–T084) chains through `build.rs` → `generated.rs` → vocabulary impl; T083 (FOUO removal) is independent.
- **Phase 6**: T086, T087, T088, T090, T091 all `[P]`. T089 and T092 run sequentially at the end.

### Cross-Story Integration Points

- The audit-v2 schema (T015, T068) affects every user story's emitted fixes. US1's migrated rules emit v2 records automatically; no US1-specific wiring needed.
- The corpus harness baseline (T037) is captured once in US1 and re-validated in T085 (US3) and T093 (Polish).

---

## Parallel Example: Phase 2 (Foundational)

```bash
# After T001 Cargo features land, launch all foundational type/trait tasks in parallel:
Task T006: Add citation field to existing Constraint enum in crates/scheme/src/constraint.rs
Task T007: Add citation field to ConstraintViolation in crates/scheme/src/constraint.rs
Task T008: Extend PageRewrite with reads/writes + Promote variant in crates/scheme/src/page_rewrite.rs
Task T009: Define Recognizer trait (reusing existing Parsed<M>) in crates/scheme/src/recognizer.rs
Task T010: Define Vocabulary trait + TokenMetadataFull in crates/scheme/src/vocabulary.rs
Task T011: Define Codec<S> trait in crates/scheme/src/codec.rs
Task T013: Define Confidence + FeatureId in crates/rules/src/confidence.rs
Task T016: Add Send + Sync bound to Rule trait in crates/rules/src/lib.rs
```

## Parallel Example: User Story 1 Tests

```bash
# All seven test files different — launch together:
Task T019: crates/engine/tests/scheduler.rs (cyclic_rewrite_pair_fails)
Task T020: crates/engine/tests/scheduler.rs (unannotated_custom_fails)
Task T021: crates/engine/tests/scheduler.rs (order_independence)
Task T022: crates/engine/tests/scheduler.rs (joint_before_fgi)
Task T023: crates/scheme/tests/evaluator.rs (determinism)
Task T024: crates/scheme/tests/evaluator.rs (empty_constraints)
Task T025: crates/scheme/tests/evaluator.rs (citation_preserved)
```

(Note: T019–T022 share a file, so they can be authored in parallel but land in the same file — the `[P]` applies across files primarily. When a single file hosts several tests, coordinate locally.)

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1 (Setup).
2. Complete Phase 2 (Foundational) — CRITICAL, blocks all stories.
3. Complete Phase 3 (US1): declarative constraint + rewrite migration. Verify byte-identical corpus diagnostics.
4. **STOP and VALIDATE**: Run `cargo test -p marque-capco --features corpus-harness`; diff against baseline.
5. If US2 and US3 slip, US1 still ships as a coherent milestone: ~15 rules retired, scheme-author pattern established.

### Incremental Delivery

1. Phases 1 + 2 → foundation ready.
2. Phase 3 (US1) → MVP: declarative rule pattern shipped.
3. Phase 4 (US2) → decoder + audit v2 shipped; commercial batch-reconciliation use case unlocked.
4. Phase 5 (US3) → full vocabulary metadata + codec surface shipped; Phase F (CUI) unblocked.
5. Phase 6 → regression gates, docs, quickstart validation.

### Parallel Team Strategy

With three developers:

1. Team completes Phases 1 + 2 together (Phase 2 tasks are heavily `[P]`).
2. Once Phase 2 is done:
   - Developer A: Phase 3 (US1) — declarative migration.
   - Developer B: Phase 4 (US2) — decoder + fixture + audit v2.
   - Developer C: Phase 5 (US3) — vocabulary + codec + FOUO fix.
3. Each story tests independently against its own corpus/accuracy gate.
4. Phase 6 lands once all three stories are green.

---

## Notes

- Tests live in the crate whose logic they exercise. Scheduler tests in `crates/engine/tests/` (not `crates/scheme/tests/`) because `marque-scheme` does not depend on `marque-engine`.
- `tests/fixtures/mangled/**/*.json` are committed artifacts; the Enron source corpus is author-supplied and not committed (see research.md R6).
- `crates/capco/corpus/priors.json` is a committed build input consumed by `crates/capco/build.rs` via `serde_json` at compile time (no runtime JSON dep). It is regenerated by `python3 tools/corpus-analysis/analyze.py --mode priors` when the Enron source is available.
- `tools/corpus-analysis/` is a **Python** tool (not a Cargo crate). Marque consumes its JSON output at build time — the Rust↔Python boundary is the priors/fixture JSON artifacts, not a cross-language crate dependency.
- `[P]` means "different file, no dependency on incomplete sibling tasks." Tests within the same file are not `[P]` with each other but can be authored together during the same unit of work.
- Every `Constraint`, `PageRewrite`, and `TokenMetadataFull` entry carries a `&'static str` citation verified at commit time (Constitution VIII + FR-021). T089 re-verifies across the whole touched set; a citation that does not resolve is removed, never deferred.
- Verify tests fail BEFORE implementation (Red → Green → Refactor).
- Commit after each task or tight logical group. Do not batch commits across user stories.
- Avoid edits to grammar-independent crates (`marque-engine`, `marque-scheme`, `marque-core`, `marque-rules`, `marque-ism`) in a future grammar-adoption PR (FR-022). This branch IS the engine-infrastructure PR that makes such isolation possible for Phase F and later.
