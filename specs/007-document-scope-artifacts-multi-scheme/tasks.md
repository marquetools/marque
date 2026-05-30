# Tasks: Document-Scope Artifacts & Multi-Scheme Co-Residence

**Input**: `spec.md`, `plan.md`, `research.md`, `data-model.md`, `contracts/`.

## Format: `[ID] [P?] [Phase] Description`
- **[P]**: parallelizable (different files/crates, no dependency on an unfinished task in the group).
- **[Phase]**: 0 / A / B / C / D / E / F / G / H, or **DEF** (deferred #823/#824).
- Each implementation task carries tests-first per Constitution V workflow ("tests before the
  implementation is considered complete"). Citations verified against `crates/capco/docs/CAPCO-2016.md`.

## Path conventions
WASM-safe crates: `crates/{scheme,ism,core,rules,capco}`. Engine: `crates/engine`. Config:
`crates/config`. Surfaces: `marque` (CLI), `crates/{server,wasm}`. Corpus/tooling: `tools/`,
`tests/corpus/`.

---

## Phase 0 — Domain-neutral scaffolding (BLOCKING foundation) 🎯

*New type surface in the WASM-safe leaf crates; testable in isolation. Gates everything. Per the
rewrite-freely posture (research D13), Phase 0 is the start of a single Phase-0/B breaking window:
the T006 `ReplacementIntent` edit (new `prior` field + `Relocate` variant) is source-breaking and
lands as a plain breaking change with all in-tree match/construction sites updated in the same
change — not deferred, not shimmed. No deprecation aliases anywhere in this feature; only the
audit-record schema and the lattice trait surface stay stable.*

- [ ] T001 [P] [0] Add `Scope::Bundle` variant to `crates/scheme/src/scope.rs`; update all
  exhaustive matchers; doc the additive-minor-version rationale (research D11). Test: exhaustive
  match compiles; `From/TryFrom<RecanonScope>` unaffected.
- [ ] T002 [P] [0] Add `ArtifactKind`, `ArtifactState<P>` enums to `crates/scheme/src/` (new
  `artifact.rs` module). `ArtifactState` has **five** states — the presence × requirement product
  incl. `PresentNotRequired` (research D12/LV1); it is a status enum, not a lattice. Tests:
  `AbsentButRequired` vs `AbsentNotRequired` vs `PresentNotRequired`; assert no join/meet impl
  exists for `ArtifactState`.
- [ ] T003 [P] [0] Add `RecognitionProvenance` and `ValueDerivation` enums (two orthogonal axes,
  research D5). Tests: independence (a node `DerivedMaxOverSources` × `DocumentContent`).
- [ ] T004 [0] Add `DerivationEdge`, `DerivationRelation`, `FiringPredicate`, `EdgeId` to
  `crates/scheme/src/` (depends T001 for `Scope`). Tests: edge declares `reads`/`writes`.
- [ ] T005 [0] Add `DocumentArtifact<S>` + the `SchemeArtifacts: MarkingScheme` **extension
  trait** carrying `type ArtifactPayload`, plus **defaulted** `document_artifacts()` /
  `derivation_edges()` methods on `MarkingScheme` itself (`crates/scheme/src/scheme.rs`). The
  extension trait keeps Phase 0 purely additive — `MarkingScheme`'s frozen surface gains only
  defaulted methods; the required associated type lives on the opt-in `SchemeArtifacts` (see
  contracts/document-artifact.md for the additive-vs-breaking staging). Source location uses
  `marque_scheme::Span` directly (defined in `crates/scheme/src/span.rs`; no `Loc`/`ByteRange`
  workaround needed). Tests: a stub scheme declares one artifact; a scheme that does NOT impl
  `SchemeArtifacts` still compiles.
- [ ] T006 [P] [0] `marque-scheme`: add reserved reversibility pre-state — `Recanonicalize { prior }`
  field + `Relocate { .. }` variant + `RecanonPriorState<S>`/`RelocatePriorState<S>` to
  `crates/scheme/src/fix_intent.rs` (where `ReplacementIntent`/`FactRef` already live — kept in
  scheme, not rules, to avoid a scheme↔rules cycle; content-ignorant fields only). Tests:
  token-level round-trip invert; G13 surface unchanged. *(#824 rough-in)*
- [ ] T007 [0] Impl `SchemeArtifacts` for `CapcoScheme` (with a placeholder `ArtifactPayload`)
  and leave the defaulted `document_artifacts()`/`derivation_edges()` empty for now (no behavior
  change yet). Tests: existing capco suite still green.
- [ ] T008 [0] Doc + registration-pin update note: confirm no rule-count change (the 32-rule pin
  in `crates/capco/tests/post_3b_registration_pin.rs` is untouched by Phase 0).

**Checkpoint**: scheme/rules/capco compile to WASM; `cargo test -p marque-scheme -p marque-capco`
green; no engine edits yet.

---

## Phase A — Input boundary (#643, #641 T1-8, #176) — gates on 0

- [ ] T010 [P] [A] Add `#[non_exhaustive]` to `ParseContext` (`crates/scheme/src/recognizer.rs`)
  — #176 staging step 1 (own commit).
- [ ] T011 [A] Add `InputSource` (promoted) + `InputContext<'a>` to `marque-scheme`. Tests: default
  is `DocumentContent`.
- [ ] T012 [A] Add `InputAdapter` trait + `StructuredDocument`/`DocumentLayer`/`RepairKind`
  (`crates/scheme/src/`). Tests: `adapt_document` default delegates to `adapt`.
- [ ] T013 [A] Engine pipeline routing by `InputContext::source` (`crates/engine/src/pipeline.rs`);
  `DocumentContent` branch unchanged. Tests: routing table (3 rows).
- [ ] T014 [A] Decoder lone-case confidence reads `InputSource` per the #176 matrix
  (`StructuredField`→0.95, `DocumentContent` lone→~0.50). Tests: 2×2 matrix (field/content ×
  in-context/lone) — this is SC-010 (with T012 `SchemaDocument` bypass + T015 WASM pin).
- [ ] T015 [P] [A] Entry-point opt-in for **trusted callers only**: CLI `--input-source`, server
  per-request. **WASM pins `InputSource::DocumentContent` and exposes NO `InputSource` parameter**
  (FR-031 WASM stance / Constitution III — `StructuredField` raises recognizer posteriors). Tests:
  WASM build rejects/omits the parameter; CLI+server accept it.

**Checkpoint**: structured-field assertive recovery works; raw-text path byte-identical to before.

---

## Phase B — Multi-scheme generification (#641 T1/T2/T3/T4) — gates on 0

- [ ] T020 [B] T1-1/T1-2: generify `Rule::check(&S::Canonical, &RuleContext<'_, S>)`; make
  `RuleContext<S>` generic; remove `pub use marque_ism::{..}` re-exports
  (`crates/rules/src/lib.rs`). Tests: a stub non-CAPCO rule compiles.
- [ ] T021 [B] T1-3: generify `Engine<S>`; eliminate the `drop(scheme)` bridge
  (the `drop(scheme)` construction bridge in `crates/engine/src/engine.rs`). Tests: a custom `S`
  reaches `page_rewrites()`.
- [ ] T022 [P] [B] T1-4: `scheme.constraint_rule_id(label) -> RuleId`; `bridge_constraint_diagnostic`
  delegates. Tests: two label namespaces map distinctly.
- [ ] T023 [P] [B] T1-5/T1-6: `ScanStrategy`/`ParseStrategy` injection points
  (`crates/core/src/{scanner,parser}.rs`); CAPCO strategy unchanged. Tests: candidate carries
  originating-grammar tag.
- [ ] T024 [P] [B] T2-1/T2-2: grammar-erase (or generify) `LintResult`/`FixResult`/`Sink`
  (`crates/engine/src/{output,pipeline}.rs`).
- [ ] T025 [P] [B] T2-3/T2-4: `MessageTemplate`/`FeatureId` `#[non_exhaustive]` + `Grammar {
  grammar_id, variant }` escape (`crates/rules/src/{message,confidence}.rs`). **Audit-schema note**:
  confirm additive-only; coordinate per CLAUDE.md Stable API surface.
- [ ] T026 [B] T3 renames (straight breaking renames — no shims, no aliases; research D13):
  `Zone::Cab`→`Custom`/`#[non_exhaustive]`, `classification_floor`→`rank_floor`,
  `OwnerProducerKind`/`FormSet`/`FormKind`/`EmissionForm` renames,
  `render_portion/banner`→`render_item/summary`, `is_fdr_dissem`→`IcMarkingVocabulary` sub-trait.
  All callers updated in this change. Tests: CAPCO still renders byte-identically post-rename.
- [ ] T027 [P] [B] T4-1: `Config.grammar_schema` generic; `scheme.validate_schema_version`
  (`crates/config/src/lib.rs`). T4-2/T4-3/T4-4: CLI/server/WASM grammar registration helpers;
  health schema version via `engine.grammar_schema_version()`.
- [ ] T028 [B] Object-safe `ErasedEngine` trait + blanket `impl<S: MarkingScheme> ErasedEngine
  for Engine<S>` (`crates/engine/src/`). `MarkingScheme` is NOT object-safe (associated types), so
  this shim is the load-bearing co-residence design — erase `lint`/`resolve`/`claims` to `&[u8]` +
  grammar-erased `Diagnostic` (contracts/multi-scheme.md, C2). Tests: two distinct concrete `S`
  coexist behind `Box<dyn ErasedEngine>`; grammar tag round-trips on each diagnostic.
- [ ] T029 [B] `MultiGrammarEngine` skeleton holding `Vec<Box<dyn ErasedEngine>>` (no coherence
  rules yet — that's Phase E; **no translator registry** — `Translate` is cut, research D7).
  Tests: two grammars register; single-scheme rules run independently.

**Checkpoint**: a second (stub) grammar registers and lints without editing CAPCO rule bodies.

---

## Phase C — Document-scope derivation layer (#799) — gates on 0, A

- [ ] T030 [C] `DocumentContext` shape in `marque-ism` (analogue of `PageContext`). Page→document
  rollup reuses the observational-state lattice types (`DissemSet` w/ `relido_observed_unanimous`,
  `JointSet`), NOT a naive re-union (research D12/LV3). Tests: document rollup (max class across
  pages); RELIDO-unanimity and NOFORN-supersession survive the page→doc fold.
- [ ] T031 [C] Engine `DocumentContext` accumulator above the per-page accumulator
  (`crates/engine/src/engine/page_context.rs` pattern). A document boundary is the **input
  boundary** (one call = one document); `DocumentContext` is built fresh per input and resets its
  page accumulators before parsing each page-break candidate — there is no in-buffer
  document-delimiter (contracts/document-artifact.md). Tests: page-reset invariant (malformed
  page-break cannot block reset); fresh `DocumentContext` per input.
- [ ] T032 [C] Extend `crates/engine/src/scheduler.rs` to schedule `DerivationEdge`s in the same
  Kahn pass as `PageRewrite`s; cycles rejected at `Engine::new`. Tests: cycle → `RewriteCycle`;
  writers-before-readers order.
- [ ] T033 [C] Resolve-decoupled-from-fix: `resolve_document` always runs; firing-predicate gating
  (incl. mode placeholder). Tests: resolution present with fixing off.
- [ ] T034 [C] Fixability-follows-derivability (research D4): absent+edge→FixProposal,
  absent+no-edge→Diagnostic. Tests: the SC-007 paired harness.
- [ ] T035 [C] Cascade-record derivations via existing `DecisionSink`
  (`DecisionEvent::triggered_by`). Tests: cascade chain reconstructs; G13 canary green.
- [ ] T036 [C] Reverse validation + "classified up to" `FrontMarking` node via `DiffInput` at
  `Scope::Document`. Tests: front-marking vs all-pages divergence reported (the #799 motivating
  `(TS//SI-G//OC/RELIDO)` case now fires) — this is the reverse-validation half of SC-012.

**Checkpoint**: the two #799 motivating unwired-edge cases produce diagnostics; derivations audited.

---

## Phase D — CAB decoupling (#799 CAB specifics) — gates on C

- [ ] T040 [D] Define the `Cab` artifact payload type; set `CapcoScheme::ArtifactPayload`. Tests:
  original-CAB vs derivative-CAB as two inbound edges into one node.
- [ ] T041 [D] Remove CAB-only fields from `CanonicalAttrs` as marking fields; delete the "page
  aggregate, not a CAB" null-out in `crates/ism/src/projected.rs`. Update all readers.
- [ ] T042 [D] `parse_cab` (in `crates/core/src/parser.rs`) produces the `Cab` artifact node +
  state instead of a `MarkingType::Cab`-tagged `CanonicalAttrs`. Tests: well-formed CAB → `Present`;
  malformed declassify → `PresentNonCanonical`.
- [ ] T043 [D] Declassify-on node with multiple inbound edges (structural / derived-max[reserved] /
  canned / historical). Value is `Product<DeclassInstruction, CannedAnnotationSet>` (research
  D12/LV2; `security-lattice.md` §8): `DeclassInstruction` = `MaxDate` date chain + exemption codes
  as a flat antichain above all dates (NOT `OrdMax<DeclassEvent>` — exemption codes are
  incomparable); `CannedAnnotationSet` = `FlatSet` of §C.4/§C.5 scope-qualifier strings. Seed the
  date side from `ProjectedMarking.declassify_on`; the exemption-antichain join is the
  #266-deferred extension. Tests: a line carrying *both* a date and a §C.4 canned string resolves
  to both components present; date axis takes the max, two distinct exemption codes stay
  incomparable (no false total-order collapse), annotation axis unions.
- [ ] T044 [D] CAB normalizer/serializer — forward-evaluable (build a `Declassify On` line from
  structured state, not just parse it). Tests: round-trip parse→serialize.
- [ ] T045 [D] Verify SC-001 (type-level test: no CAB fields on pivot type + CAB→`DocumentArtifact`
  node test) + SC-008a (single-scheme latency/throughput gates, no regression).

**Checkpoint**: CAB is a node; `ProjectedMarking` has no CAB-only fields; benches green.

---

## Phase E — CUI co-residence (#641 co-reside, #128) — gates on B, C *(validated vs synthetic `StubScheme`; real CUI source-pending)*

- [ ] T049 [E] Define the synthetic test-only `StubScheme` (an invented non-IC control, no claimed
  NARA/ISOO/CAPCO authority) used to exercise co-residence (FR-026). It is a test fixture, NOT a
  shipped grammar; it asserts no real CUI semantic. Tests: registers alongside `CapcoScheme`.
- [ ] T050 [E] Add `CoherenceRule`/`CoherenceContext`/`CoherenceDiagnostic` trait surface to
  `marque-scheme` (#641 T1-7). **`Translate`/`TranslationProposal` are cut** (research D7 — no
  in-scope consumer). Tests: stub coherence rule over two canonicals.
- [ ] T051 [E] Portion-scope ownership routing (research D8): before junk-recovery, offer rejected
  tokens to co-active schemes. Tests: no-silent-loss (SC-004).
- [ ] T052 [E] `(S//CUI)` cross-grammar conflict: error, no auto-fix, relocate suggestion
  (human-confirmed). Uses the `Relocate` reserved variant from T006. Tests: high recognition
  confidence / low resolution confidence (research D8).
- [ ] T053 [E] Document-scope releasability reconciliation in `marque-engine`:
  `Product<StubReleasability, CapcoIcDissem>` componentwise join + monotone NOFORN closure
  (research D6). The `Product` implements `JoinSemilattice` only (+ `BoundedJoinSemilattice` via
  factor bottoms), **never `BoundedLattice`** — the non-IC factor is agency-extensible/open, no
  top (research D6/D12/LV4). Each regime renders its own projection. Tests: a `StubScheme`
  non-IC-control portion + `C//RELIDO` → banner floors to `CONFIDENTIAL//NOFORN`, the non-IC
  control is escrowed on its own projection, RELIDO superseded. **Asserts the `Product`+closure
  mechanism only — no real FEDCON⇒NOFORN mapping (FR-026, Constitution VIII).**
- [ ] T056 [E] `multi_scheme_latency` benchmark: establish the two-scheme p95 budget on the hot
  path (SC-008b). The single-scheme 16 ms gate is NOT assumed to hold under the O(schemes)
  multiplier; record the measured budget as the CI gate.
- [ ] T054 [E] `CaveatLayer` artifact ≡ #128 second-banner-line caveats (vocabulary source-pending).
  Tests: caveat layer rendered on its own line, distinct from dissem block — the caveat-layer half
  of SC-012 (FR-052).
- [ ] T055 [E] `MultiGrammarEngine` runs coherence rules over joint output (completes T029). Tests:
  coherence diagnostic carries both grammars' representations.

**Checkpoint**: two schemes co-resident; releasability escrow demonstrated; no silent token loss.

---

## Phase F — Mode taxonomy (#645) — gates on C (M4/M5 also gate on #206)

- [ ] T060 [P] [F] M1: `[engine] severity_cap`; apply in `fast_path_severities`
  (`effective = override.unwrap_or(default).min(cap)`). Tests: caps Fix→Suggest; per-rule wins.
  (T060 + T061 + T064 together verify SC-011: cap, zone-gating, and `ValidateForEra` suppression.)
- [ ] T061 [P] [F] M2: `Rule::target_zones` + `[engine] fix_zones`; gate fix promotion before
  `__engine_promote`. Tests: body-only fix promotion; diagnostics still emit for all zones.
- [ ] T062 [F] M3: `DeploymentContext` (interactive/batch/boundary/archival) defaults profile,
  each field independently overridable. Tests: profile defaults + override precedence.
- [ ] T063 [F] M4: wire `as_of` engine→recognizer→`RuleContext` (depends #206). Tests: era anchor
  reaches a rule.
- [ ] T064 [F] M5/M6: `ArchivalIntent` (Update/PreserveWithMetadata/ValidateForEra) +
  `GrammarEra`/`MarkingScheme::era_at`/`vocabulary_at`. Tests: ValidateForEra suppresses
  post-`as_of` rules, no rewrites.
- [ ] T065 [F] Mode-gated apply hook (firing predicate consults deployment mode) — sets up #824 M3.

**Checkpoint**: audit-only / zone-targeted / archival deployments configurable without per-rule edits.

---

## Phase G — Concrete artifact rules (#266, #420) — gates on D, E

- [ ] T070 [G] Page-level AEA / NATO presence flags on `PageContext`/`DocumentContext` (#266
  prerequisite 3).
- [ ] T071 [G] §C.4 rule: any RD/FRD/TFNI portion ⇒ `Declassify On` must contain "N/A to
  [RD/FRD/TFNI, as appropriate] portions. See source list for NSI portions." High-confidence fix.
  Citation: CAPCO-2016 §C.4 p33 (verified line 683). Declarative `CannedString` edge.
- [ ] T072 [G] §C.5 rule: any NATO portion in a US-classified document ⇒ "N/A to NATO portions. See
  source list for NSI portions." Citation: §C.5 p33 (verified line 687). Combined AEA+NATO form per
  §C.4 p33 when both present.
- [ ] T073 [P] [G] #420: absence-detect recognizers for missing portion-marks/banners
  (`crates/core`); nested-bullet handling; entirely-`(U)` exemption. Populate `AbsentButRequired`;
  flag-only (D4). Tests: missing-mark fires; all-`(U)` document does not.
- [ ] T074 [G] Verify SC-009 (canned-string citations re-checked against the manual). Add a
  dedicated `absence_scan` benchmark and verify SC-008a holds with the #420 whole-document scan
  active (detecting *missing* marks is new O(blocks) work not present today).

**Checkpoint**: end-to-end document-artifact rules prove the node-state + derivation model.

---

## Phase H — Per-grammar corpus & tooling (#640) — gates on B

- [ ] T080 [P] [H] Directory namespace: `tests/corpus/capco/` (move valid/invalid/prose/…);
  `grammar_corpus_root(grammar)` + back-compat `corpus_root()` alias in `crates/test-utils`.
- [ ] T081 [P] [H] `analyze.py` grammar extension profile (`tools/corpus-analysis/grammars/capco.json`);
  CAPCO default unchanged.
- [ ] T082 [P] [H] Per-grammar priors schema naming (`capco-priors-N`); `build.rs` accept-list.
- [ ] T083 [H] `run_corpus_accuracy(engine, grammar)` parameterized harness; CAPCO test calls it
  with `"capco"`. SC-002/003 thresholds per-grammar, not relaxed.

**Checkpoint**: a new grammar can land fixtures/priors without namespace collision.

---

## Deferred — gates noted, NOT implemented in this feature

- [ ] D823 [DEF] ICD-206 source-list generation (`Derived From: Multiple Sources` + source list +
  source-derived `Declassify On`). **Gated on**: Phase A structured source-metadata `InputAdapter`
  + the Phase 0 reserved bundle-scope edge (`Scope::Bundle`, `DerivationRelation::SourceDerived`,
  `ValueDerivation::DerivedMaxOverSources`). Runs the same edge forward (authoring) and backward
  (validation).
- [ ] D824 [DEF] Reversible applied fixes realization: the reversal pass + additive `marque-3.x`
  audit-schema bump consuming the Phase-0 reserved pre-state fields (T006) and the Phase-F
  mode-gated apply hook (T065). **Must preserve** the G13 canary.

---

## Dependency summary

```
0 ──> A ──┐
0 ──> B ──┼──> E ──┐
0 ──> C ──┴──> D ──┴──> G
        C ──> F ──> (D824)
        A ──┐
        C ──┴──> (D823)
        B ──> H
```

- **0 blocks everything.** A and B fan out and run in parallel. C needs 0+A. D needs C. E needs
  B+C. F needs C (M4/M5 also #206). G needs D+E. H needs B.
- Within a phase, `[P]` tasks touch disjoint files and may run concurrently.
- Tests precede "done" for every implementation task (Constitution V workflow).
