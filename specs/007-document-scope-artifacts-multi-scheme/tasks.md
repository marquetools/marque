# Tasks: Document-Scope Artifacts & Multi-Scheme Co-Residence

**Input**: `spec.md`, `plan.md`, `research.md`, `data-model.md`, `contracts/`.

## Format: `[ID] [P?] [Phase] Description`
- **[P]**: parallelizable (different files/crates, no dependency on an unfinished task in the group).
- **[Phase]**: 0 / A / B / C / D / E / F / G / H, or **DEF** (deferred #823/#824).
- Each implementation task carries tests-first per Constitution V workflow ("tests before the
  implementation is considered complete"). Citations verified against `crates/capco/docs/CAPCO-2016.md`.
- **Numeric gaps in task IDs are intentional spacing, not missing tasks** (e.g. T008→T010,
  T045→T049). Phases are blocked out in tens; intra-phase `b`/`c` suffixes (e.g. T009b, T012c)
  are coverage tasks slotted into an already-spaced range.

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
- [ ] T009 [0] SC-006 round-trip test — `Recanonicalize { prior: Some(_) }` and `Relocate` invert
  from audit-permitted pre-state (token canonicals/spans/BLAKE3); `Recanonicalize { prior: None }`
  explicitly out of round-trip scope until #824. G13 canary green.
- [ ] T009b [0] SC-005 compile-gate test — the declassify-on node declares a
  `DerivationRelation::SourceDerived` inbound edge at `Scope::Bundle` that COMPILES with no #823
  source-metadata adapter present.
- [ ] T009c [P] [0] Executed WASM build gate — `cargo build -p marque-scheme -p marque-rules
  -p marque-capco --target wasm32-unknown-unknown` (or wasm-pack) MUST succeed at the Phase-0
  checkpoint and is re-run after Phase C adds `DocumentContext` to `marque-ism` (Constitution III;
  not just a "compiles to WASM" assertion).

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
- [ ] T012b [A] `InputAdapter` error contract — define `AdaptError` variants + `DocumentStructure`
  validation (page-span ordering/non-overlap, front-matter sub-span, registered `scheme_id`); test
  that feeding malformed adapter output fails closed. Input validation at boundaries is
  CRITICAL-class.
- [ ] T012c [A] `StructuredDocument`/`DocumentLayer` content lifecycle — content-bearing canonical
  fields holding caller-document content wipe on drop via secrecy/zeroize (Constitution II), OR a
  documented argument that `S::Canonical` is token-only for every shipped scheme; add a G13-style
  test that the `StructuredDocument` chain holds only spans + lattice values + canonicals (no
  verbatim content).
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
- [ ] T028b [B] `ErasedEngine` working spike (BEFORE Phase E) — a real `impl<S: MarkingScheme>
  ErasedEngine for Engine<S>` + minimal `CoherenceRegistry` that compiles and round-trips a grammar
  tag; confirm erasure boxes at most once per scheme per document (not per span/diagnostic); capture
  dispatch overhead in a Phase-B smoke bench. De-risks the object-safety "hard problem" before
  Phase E depends on it.
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
- [ ] T032b [C] Scheduler mis-annotation guard — a `DerivationEdge` consuming a `PageRewrite`'s
  output but omitting that axis from its `reads` MUST be rejected (or proven scheduled-after); test
  that a known-axis under-annotation cannot produce a stale-value read.
- [ ] T033 [C] Resolve-decoupled-from-fix: `resolve_document` always runs; firing-predicate gating
  (incl. mode placeholder). Tests: resolution present with fixing off.
- [ ] T033b [C] FR-014 firing-predicate test — a mode-inactive derivation edge REMAINS in the DAG
  validated at `Engine::new` and is skipped only at firing time (never a topology swap).
- [ ] T034 [C] Fixability-follows-derivability (research D4): absent+edge→FixProposal,
  absent+no-edge→Diagnostic. Tests: the SC-007 paired harness.
- [ ] T034b [C] Derived-value fill test (US2 Scenario 1) — `resolve_document` returns the DERIVED
  VALUE (not just fix-vs-flag classification) for an absent node with an inbound rollup edge; the
  source-date-max path is explicitly #823-deferred.
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
  canned / historical). Value is `OrdMax<DeclassInstruction>` — a single-valued chain (CAPCO-2016
  §E.3 p32 "Only a single value must be used on the 'Declassify On' line"; §E.4/§E.5 p33 say the
  canned N/A commingling string REPLACES any date/event). `DeclassInstruction` = one enum with a
  hand-written **TOTAL** `Ord` over the §E.3 nine-tier precedence hierarchy, keyed lexicographically
  on `(tier 1-9, resolved-protection-date via IsmDate::end_cmp, lowest exemption number)`; bottom =
  `Unset`/absent (join identity), top = the single `Commingled` tier-1 point ("N/A … see source
  list", no date) which "takes precedence over all" (§E.3 p32) ⇒ implements
  `BoundedJoinSemilattice` (lawful: a closed finite hierarchy with a genuine maximum, unlike the
  open SciSet/SarSet). The AEA-only / NATO-only / combined choice AMONG the §E.4/§E.5 N/A strings is
  a **render concern** keyed on the document's AEA-present / NATO-present flags (T070), NOT a
  sub-lattice inside `DeclassInstruction` — tier 1 is the single `Commingled` lattice point and
  which exact string renders is downstream. Generalizes the existing date-only
  `crates/capco/src/lattice/declassify_on.rs` (`Option<IsmDate>` `max_by(end_cmp)`) to the full
  9-tier carrier; seed the date tiers from `ProjectedMarking.declassify_on`. Tests: `Ord`
  totality / antisymmetry / transitivity across all 9 tiers; `OrdMax` join
  idempotence / commutativity / associativity + bottom-identity + top-absorption; §E.3
  worked-precedence oracle fixtures (`50X1-HUM ⊔ 25X-dated == 50X1-HUM`;
  `Commingled ⊔ anything == Commingled`), each citing its §E.3 source. **NOT
  `Product<DeclassInstruction, CannedAnnotationSet>` and NOT `Product<MaxDate, FlatSet<ExemptionCode>>`**
  — both are category errors (`Product::join` joins factors independently, making the illegal
  "date + canned coexist" state representable, and `FlatSet` models accumulating incomparable
  atoms): §E.3 exemptions COMPETE in one total order and §E.4 says the canned string REPLACES the
  date. One slot, one total order; there is NO separate `CannedAnnotationSet`/`FlatSet` axis.
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
- [ ] T051b [E] FR-023 determinism test — exactly-one acceptor routes; two-or-more acceptors →
  cross-grammar conflict (never a silent pick); zero acceptors → junk-recovery at unchanged
  `DocumentContent` confidence; AND swapping scheme registration order does NOT change the outcome.
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
- [ ] T054 [E] `CaveatLayer` artifact ≡ #128 second-banner-line caveats (vocabulary source-pending).
  Tests: caveat layer rendered on its own line, distinct from dissem block — the caveat-layer half
  of SC-012 (FR-052).
- [ ] T055 [E] `MultiGrammarEngine` runs coherence rules over joint output (completes T029). Tests:
  coherence diagnostic carries both grammars' representations.
- [ ] T056 [E] `multi_scheme_latency` benchmark: establish the two-scheme p95 budget on the hot
  path (SC-008b). The single-scheme resolution ceiling is **p95 ≤ 2 ms** on a 10 KB input (absolute,
  not a soft target; replaces the retired 16 ms placeholder). The committed two-scheme ceiling is
  number-of-schemes × the single-scheme 2 ms ceiling (two schemes → **4 ms** p95), recorded as the
  CI gate — NOT a self-ratifying "measured budget is the gate".

**Checkpoint**: two schemes co-resident; releasability escrow demonstrated; no silent token loss.

---

## Phase F — Mode taxonomy (#645) — gates on C (M4/M5 also gate on #337)

- [ ] T060 [P] [F] M1: `[engine] severity_cap`; apply in `fast_path_severities`
  (`effective = override.unwrap_or(default).min(cap)`). Tests: caps Fix→Suggest; per-rule wins.
  (T060 + T061 + T064 together verify SC-011: cap, zone-gating, and `ValidateForEra` suppression.)
- [ ] T061 [P] [F] M2: `Rule::target_zones` + `[engine] fix_zones`; gate fix promotion before
  `__engine_promote`. Tests: body-only fix promotion; diagnostics still emit for all zones.
- [ ] T062 [F] M3: `DeploymentContext` (interactive/batch/boundary/archival) defaults profile,
  each field independently overridable. Tests: profile defaults + override precedence.
- [ ] T063 [F] M4: wire `as_of` engine→recognizer→`RuleContext` (depends #337: historical-as-valid
  evaluation mode — consume `Deprecation::valid_from`/`valid_until` in rule context). Tests: era
  anchor reaches a rule.
- [ ] T064 [F] M5/M6: `ArchivalIntent` (Update/PreserveWithMetadata/ValidateForEra) +
  `GrammarEra`/`MarkingScheme::era_at`/`vocabulary_at`. Tests: ValidateForEra suppresses
  post-`as_of` rules, no rewrites.
- [ ] T065 [F] Mode-gated apply hook (firing predicate consults deployment mode) — sets up #824 M3.

**Checkpoint**: audit-only / zone-targeted / archival deployments configurable without per-rule edits.

---

## Phase G — Concrete artifact rules (#266, #420) — gates on D, E

- [ ] T070 [G] Page-level AEA / NATO presence flags on `PageContext`/`DocumentContext` (#266
  prerequisite 3).
- [ ] T071 [G] §E.4 rule: any RD/FRD/TFNI portion ⇒ `Declassify On` must contain the byte-exact
  AEA canned string "N/A to [RD/FRD/TFNI, as appropriate] portions. See source list for NSI
  portions." (the bracket MUST contain ", as appropriate" — dropping it is drift). High-confidence
  fix. Citation: CAPCO-2016 §E.4 p33. Declarative `CannedString` edge. Gate the fix on a byte-exact
  fixture transcribed from the §E.1 valid-values list p31.
- [ ] T072 [G] §E.5 rule: any NATO portion in a US-classified document ⇒ the byte-exact NATO canned
  string "N/A to NATO portions. See source list for NSI portions." Citation: CAPCO-2016 §E.5 p33.
  When both AEA and NATO portions are present, write the combined byte-exact form "N/A to
  [RD/FRD/TFNI, as appropriate] [and NATO, if appropriate] portions. See source list for NSI
  portions." per §E.4 p33. Gate the fix on a byte-exact fixture transcribed from the §E.1
  valid-values list p31.
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

## FR → Task → SC traceability matrix

Every FR-001..FR-061 mapped to its covering task(s), the SC it satisfies, and the phase. New
coverage tasks added by this revision (T009/T009b/T009c/T012b/T012c/T028b/T032b/T033b/T034b/T051b)
are called out where they close a gap. "—" in the SC column means the FR is infrastructure with no
dedicated SC line; it is exercised transitively by the listed tasks.

| FR | Task(s) | SC | Phase |
|----|---------|----|----|
| FR-001 | T040, T041, T045 | SC-001 | D |
| FR-002 | T002 | SC-001 (state surface) | 0 |
| FR-003 | T030, T031 | SC-002 (doc aggregate) | C |
| FR-010 | T004, T032, **T032b** | SC-007 | 0, C |
| FR-011 | T033 | SC-007 | C |
| FR-012 | T035 | SC-007 (cascade) | C |
| FR-013 | T034, **T034b** | SC-007 | C |
| FR-014 | T033, **T033b** | SC-011 (mode gating) | C, F |
| FR-015 | T036 | SC-012 | C |
| FR-020 | T024, T028, **T028b**, T029 | SC-002 | B |
| FR-021 | T050, T051, T053 | SC-003 | E |
| FR-022 | T053 | SC-003 | E |
| FR-023 | T051, **T051b** | SC-004 | E |
| FR-024 | T052 | SC-004 | E |
| FR-025 | T050, T053 | SC-003 | B (scheme-neutral), E |
| FR-026 | T049, T053 | SC-003 | E |
| FR-030 | T012, **T012b**, **T012c** | SC-010 | A |
| FR-031 | T011, T014, T015 | SC-010 | A |
| FR-032 | T012 | SC-010 | A |
| FR-040 | T060 | SC-011 | F |
| FR-041 | T061 | SC-011 | F |
| FR-042 | T062 | SC-011 | F |
| FR-043 | T063 (depends #337), T064 | SC-011 | F |
| FR-050 | T070, T071, T072 | SC-009 | G |
| FR-051 | T073, T074 | SC-007 (flag-only), SC-008a (absence bench) | G |
| FR-052 | T054 | SC-012 | E |
| FR-060 | T006, **T009** | SC-006 | 0 |
| FR-061 | T004, **T009b**, D823 | SC-005 | 0, DEF |

SC-only / cross-cutting coverage not tied to a single FR:

| SC | Task(s) | Phase |
|----|---------|----|
| SC-005 | **T009b** (compile-gate), T004 (`Scope::Bundle`), D823 | 0, DEF |
| SC-006 | **T009** (round-trip), T006 | 0 |
| SC-008a | T045, T074 | D, G |
| SC-008b | T056 (2 ms single / 4 ms two-scheme CI gate) | E |
| SC-012 | T036 (reverse-validation half), T054 (caveat-layer half) | C, E |
| (WASM-safety, Constitution III) | **T009c** (executed wasm32 build gate) | 0 |

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
  B+C. F needs C (M4/M5 also #337). G needs D+E. H needs B.
- Within a phase, `[P]` tasks touch disjoint files and may run concurrently.
- Tests precede "done" for every implementation task (Constitution V workflow).
