<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Tasks: Engine + Rule Architecture Refactor

**Input**: Design documents from `/home/knitli/marque/specs/006-engine-rule-refactor/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/{fix-intent.md, audit-record.md, engine-pipeline.md}, quickstart.md
**Source-plan**: `docs/plans/2026-05-02-engine-refactor-consolidated.md` (PR sequence § 4 is the authoritative implementation order)

**Tests**: This refactor is type-system-first; many invariants close as compile-fail tests, AST lints, or property tests. Test tasks are interleaved with implementation tasks where the murder-board diagnosis required them. See `contracts/engine-pipeline.md` § "Test strategy" for the five-layer architecture.

**Organization**: Phases 1–2 deliver setup + foundational infrastructure (PR 0 / 0.5 / 0.6 / 1). Phases 3–10 deliver each user story (US1–US8) in priority order. Phase 11 covers cross-cutting work (PR 6 / 8 / 9) and final polish.

**Per-PR mapping**: each task carries the source PR (PR 0, 0.5, 0.6, 2, 3a, 3b, 3c, 3.7, 4, 5, 6a/6b/6c, 7, 8, 9, 10). The PR sequence is the dependency spine — see § Dependencies & Execution Order.

**Quality gates**: two checklists govern PR review:
- [`checklists/requirements.md`](./checklists/requirements.md) — spec-form quality (already passed).
- [`checklists/correctness.md`](./checklists/correctness.md) — substance: lattice/rollup, two-pass apply, open-vocab parser, citation fidelity (mechanical + semantic-agreement-with-CAPCO), and known-defect coverage. **22 `[GATE]` items** MUST clear before the corresponding P1 PR (0.6 / 3a–3c / 5 / 6 / 3.7) merges. Each phase below cross-references the relevant `correctness.md` section under `**Quality gate**`.

## Format: `[ID] [P?] [Story?] Description (PR-N)`

- **[P]**: parallelizable (different files, no in-PR dependency on incomplete tasks)
- **[Story]**: user story tag (US1–US8); setup / foundational / polish phases carry no story tag
- **(PR-N)**: source PR per consolidated plan § 4
- File paths are absolute / repo-relative; line numbers per spec are *indicative* (re-grep at edit time)

---

## Phase 1: Setup (PR 0)

**Purpose**: Workspace-level infrastructure — bench baselines, AST-lint scaffolding, compile-time `Send + Sync` enforcement. Prerequisite for the keystone (PRs 3a/3b/3c) and for every PR that asserts against a perf gate.

- [ ] T001 Capture pre-refactor bench baselines for `fix_throughput`, `lint_latency`, and any existing benches; emit single JSON object at `benches/baselines/2026-05-pre-refactor.json` with fields `{ bench, p50, p95, p99, mean, samples, git_sha, captured_at }` (FR-030..FR-033, R-5; PR-0)
- [ ] T002 [P] Add `static_assertions::assert_impl_all!(Rule: Send + Sync)` to `crates/rules/src/lib.rs` (or rule-set constructor); compile-fail tests at `crates/rules/tests/send_sync.rs` (FR-038; PR-0)
- [ ] T003 [P] Add `static_assertions::assert_impl_all!(dyn Recognizer<CapcoScheme>: Send + Sync)` to `crates/scheme/src/recognizer.rs`; compile-fail test (FR-038; PR-0)
- [ ] T004 [P] Create `tools/masking-pin-lint/` Rust binary crate (NOT a workspace member to avoid contaminating WASM-safe closure per Constitution III); deps: `syn` 2.x + `proc-macro2` + `octocrab`; CLI invocation `cargo run --manifest-path tools/masking-pin-lint -- <workspace-dir>` (FR-039, R-1; PR-0)
- [ ] T005 [P] Implement masking-pin lint scanner: AST-walk `tests/` and `crates/*/tests/` for `with_recognizer(StrictRecognizer)` calls; require comment `// MASKING-PIN: tracks #NNN` or `// INTENTIONAL-STRICT: <reason>` within 5 lines (FR-039; PR-0)
- [ ] T006 [P] Implement masking-pin lint GitHub-API check via `octocrab`: query `repos/{owner}/{repo}/issues/{n}`, follow `closed_as_duplicate_of` chains until final close (mandatory chain-following per FR-039 rule 4); flag cascade-close-via-meta-issue (PR-0)
- [ ] T007 [P] Create `tools/promote-callsite-lint/` Rust binary crate (deps: `syn` 2.x + `proc-macro2`); AST-walk for `AppliedFix::__engine_promote` and `EnginePromotionToken::__engine_construct` calls; assert origin = `Engine::fix_inner` in production code; carve-out for `#[cfg(test)]` / `tests/` enumerated per call site with required inline comment (FR-040, Constitution V Principle V; PR-0)
- [ ] T008 [P] Add masking-pin lint and promote-callsite lint to CI workflow at `.github/workflows/` (or equivalent); fail PR on lint violation (FR-039, FR-040; PR-0)
- [ ] T009 Inventory the two masking pins (`core_error_isolation.rs` → #257; `corpus_accuracy.rs` → #258) and five intentional-strict pins; verify each carries the correct comment marker; document in `docs/refactor-006/masking-pin-inventory.md` (PR-0)
- [ ] T009a Inventory every test-fixture call site of `AppliedFix::__engine_promote` (and `EnginePromotionToken::__engine_construct` once it lands in PR 3c); verify each carries an inline comment naming the Constitution V Principle V test-fixture carve-out (e.g., `// Test-fixture carve-out per Constitution V`) and document each call site with its three-constraint scope (1: `#[cfg(test)]` / `tests/` / `dev-dependencies` gating; 2: no commingling with engine-promoted output; 3: test-fixture *construction* only — not CLI/batch/bench convenience helpers) in `docs/refactor-006/promote-callsite-inventory.md`. Complements T007's lint enforcement: T007 fails the build when a call site lacks the comment; T009a documents WHERE the carve-out is exercised so a reviewer doesn't have to re-derive the scope per call (Constitution V Principle V; FR-040; PR-0)

**Checkpoint**: Foundation lints green; baseline JSON checked in; Send+Sync compile-time guarantees in place.

---

## Phase 2: Foundational (PR 0.5 + PR 0.6 + PR 1)

**Purpose**: Citation-lint infrastructure (PR 0.5) + preemptive citation-defect fix (PR 0.6) + verification of already-landed splice (PR 1). **All three PRs must land before keystone work** because the keystone touches every rule body — citation hygiene needs to be enforced as we go. (T019's `fix_throughput` R² ≥ 0.9 perf-gate verification is a separable concern — see the checkpoint note + #306 — and explicitly does NOT block keystone.)

**⚠️ CRITICAL**: PR 0.6 is merge-gated on the citation-defect catalog being empty.

- [X] T010 Create `tools/citation-lint/` Rust binary crate (deps: `syn` 2.x + `proc-macro2` + `pulldown-cmark` for parsing `crates/capco/docs/CAPCO-2016.md`); CLI `cargo run --manifest-path tools/citation-lint/Cargo.toml -- <workspace-dir>` (NOT `cargo run -p citation-lint` — the crate is out-of-workspace per Constitution III, matching the `tools/masking-pin-lint/` and `tools/promote-callsite-lint/` pattern) (FR-018, R-1; PR-0.5)
- [X] T011 Implement citation-lint AST scanner: extract `§X.Y pNN` references from `citation:` struct fields, `message:` strings, `constraint_label:` strings, and `///`/`//!` doc-comment positions across **all `crates/*/src/**/*.rs`** (workspace-wide per source plan §4 PR-0.5 + §6 Layer 5; future-proofs for `marque-cui`, `marque-nato`, etc.) (FR-018; PR-0.5)
- [X] T012 Implement citation-lint resolver: parse `crates/capco/docs/CAPCO-2016.md`; build `(section, page)` index; assert each cited `(section, page)` resolves to a real passage; assert `section ∈ {A,B,C,D,E,F,G,H}` (normative range); assert page falls within document; reject bare `§NN`; **reject legacy `line NNNN` citation forms** (retired in commit b340bec — page numbers only) (FR-018; PR-0.5)
- [X] T013 Add citation-lint to CI workflow; emit defect catalog at `docs/refactor-006/citation-defect-catalog.md` on lint failure for downstream PR 0.6 consumption (FR-018; PR-0.5)
- [X] T014 Create F.1 corpus-fidelity test skeleton at `crates/capco/tests/citation_fidelity.rs`; for each `Constraint`/`PageRewrite`/`Rule` cited authority, assert ≥1 corpus fixture exists under `tests/corpus/` using the shared corpus contract (text input plus sibling `.expected.json`) and exercises the predicate against the canonical example from the cited passage (FR-019; PR-0.5)
- [X] T015 Run citation-lint + F.1 against existing rule catalog; capture defect catalog at `docs/refactor-006/citation-defect-catalog.md` (R-6 discovery exercise; PR-0.5)
- [X] T016 Fix the four pre-identified citation-defect classes in `crates/capco/src/scheme.rs` and `rules.rs`: (a) `§4` fabrications across multiple `scheme.rs` lines; (b) doubled `p150–151 p151` at five sites in `rules.rs`; (c) SIGMA cross-revision archaeology at `rules.rs:4053`; (d) HCS-P over-strict predicate at `scheme.rs:1839-1849` if F.1 surfaces it (FR-020; PR-0.6)
- [X] T017 Address every additional defect surfaced by PR 0.5's citation-defect catalog; add corpus fixtures for any newly-cited authority lacking one; update citations per FR-018 rules (FR-019, FR-020; PR-0.6)
- [X] T018 Verify PR 0.6 merge gate: `cargo run --manifest-path tools/citation-lint/Cargo.toml -- .` exits 0 (the crate is out-of-workspace per Constitution III; `-p citation-lint` does NOT work from the repo root); F.1 corpus fixture coverage is 100% over current rule catalog (FR-018, FR-019; PR-0.6)
- [X] T019 Verify single-pass forward splice (PR #277 / #278 already landed): splice-correctness verified via tests + corpus regression (commit `9d5e3112` merged 2026-05-02). The `fix_throughput` Criterion R² ≥ 0.9 gate verification is **deferred** because `scripts/bench-check.sh::check_fix_throughput` was disabled in commit `bd5b84de` (2026-05-03) "until we resolve the underlying issue" — verification awaits the gate re-enable tracked at #306. Full deferral rationale + scope boundary in [`docs/refactor-006/pr-1-verification.md`](../../docs/refactor-006/pr-1-verification.md). (FR-029; PR-1)

**Checkpoint**: Citation-lint green workspace-wide; F.1 corpus fixture coverage 100% over current rules; splice landed and splice-correctness verified. The `fix_throughput` R² ≥ 0.9 gate enforcement is deferred per [`docs/refactor-006/pr-1-verification.md`](../../docs/refactor-006/pr-1-verification.md) — gate-disable in commit `bd5b84de` is tracked at **#306** (issue: `scripts/bench-check.sh: re-enable check_fix_throughput after underlying scaling bug is fixed`). The keystone (PR 3a/3b/3c) does NOT depend on the gate being re-enabled, so this checkpoint does not block keystone work; #306 is a separable post-keystone hygiene item.

---

## Phase 3: User Story 1 — Audit records carry no document content (Priority: P1) 🎯 MVP

**Goal**: Make G13 (audit-record content-ignorance) a *type invariant* rather than a carve-out enforced by comments. Pivot type splits 1→3, `Canonical<S>` seals open-vocab construction, `MessageTemplate` closes message construction, decoder open-vocab lockout, audit cutover `marque-mvp-2 → marque-1.0`.

**Independent Test**: Deterministic NDJSON canary scan over the full five-corpus regression sweep (`tests/corpus/{valid,mangled,prose,prose-positive,lattice}/`) finds **zero** verbatim input bytes in any `Engine::fix_inner`-emitted `AppliedFix` JSON serialization (other than within span numerals, BLAKE3 digests, or enumerated identifier values). Test-fixture records under the Constitution V carve-out are excluded by construction. Runs at `crates/engine/tests/canary_scan.rs` (SC-001).

**Quality gate**: [`checklists/correctness.md`](./checklists/correctness.md) §6 (known-defect coverage — CHK067 #257, CHK074 R001 message, CHK075 `build_decoder_diagnostic` carve-out, CHK076 `provenance.canonical_bytes`); §4 partial (CHK033 if audit-record citation surfaces). Reviewer clears all `[GATE]` items in §6 before merging PR 3c.

### PR 3a — Pivot type split (KEYSTONE-1)

- [X] T020 [US1] Define `ParsedAttrs<'src>` in `crates/ism/src/parsed.rs` with `'src` lifetime threading and the eight `Parsed*<'src>` field types per `data-model.md` § ParsedAttrs (PR-3a)
- [X] T021 [US1] Define `CanonicalAttrs` in `crates/ism/src/canonical.rs` — owned form, `classification: Option<MarkingClassification>` (FR-007 supporting), existing lattice types (`SciSet`, `SarSet`, `FgiSet`) for set-valued fields (PR-3a)
- [ ] T022 [US1] **DEFERRED to PR 5/6** — `ProjectedMarking` has no consumer at PR 3a (`MarkingScheme::project` already returns `Self::Marking`, not `ProjectedMarking`). Defining it in `marque-ism` would force a `marque-ism → marque-scheme` dep edge, violating Constitution VII's peer-leaf placement of `marque-ism`. The type lands when its consumer (`Scope::Page` projection cutover) lands at PR 6. (was: PR-3a → now PR-5/6)
- [X] T023 [US1] Implement `from_parsed_unchecked(ParsedAttrs<'_>) -> CanonicalAttrs` adapter as `#[doc(hidden)] pub` in `crates/ism/src/canonical.rs`; transitional bridge for PR 3a → PR 3c (PR-3a)
- [X] T024 [US1] Mechanically migrate test-fixture `IsmAttributes { ... }` literals across `crates/capco/tests/`, `crates/engine/tests/`, and unit-test modules to `from_parsed_unchecked(...)` form; expect to re-touch at PR 3c (PR-3a)
- [X] T025 [P] [US1] Add CI matrix entry: corpus regression sweep × {3a-only} = 1 run validating PR 3a is independently correct (SC-014; PR-3a)

### PR 3b — Rule collapse (KEYSTONE-2)

- [ ] T026 [US1] Per #263, collapse the existing ~56 CAPCO rules in `crates/capco/src/rules.rs` to ~10–13 by merging rules whose predicates differ only in surface-token identity into a single rule parameterized over `Vocabulary<S>` lookup; the rule catalog post-collapse becomes the source of truth for the `MessageTemplate` extraction at PR 3c (PR-3b)
- [ ] T027 [US1] Update rule registration in `CapcoRuleSet::new()` to reflect the collapsed catalog; preserve every distinct citation (FR-018) and severity (PR-3b)
- [ ] T028 [P] [US1] Update test fixtures expecting rules removed by collapse; consolidate per-rule unit-test modules where rules merged (PR-3b)
- [ ] T029 [P] [US1] Add CI matrix entry: corpus regression sweep × {3a + 3b} = 1 run validating the subsequence is correct (SC-014; PR-3b)

### PR 3c — Canonical sealing + audit cutover (KEYSTONE-3)

- [ ] T030 [US1] Run `tools/message-template-extract/` mechanical extraction (R-2) against the post-3b rule catalog and `crates/engine/src/engine.rs`; emit starter at `specs/006-engine-rule-refactor/contracts/message-template-starter.md` for review (PR-3c)
- [ ] T031 [US1] Define `MessageTemplate` closed enum in `crates/ism/src/message.rs` (or `marque-rules`) per starter; hand-curate variants per R-2 review (PR-3c, FR-003)
- [ ] T032 [US1] Define `MessageArgs` closed-set struct with permitted scalar/ID fields only (TokenId, CategoryId, Span, Blake3Hash, Confidence, FeatureId, expected_token, actual_token); compile-fail tests demonstrating no `String`/`&str`/`Vec<u8>` field exists (FR-003; PR-3c)
- [ ] T033 [US1] Define `Message::new(template: MessageTemplate, args: MessageArgs) -> Self`; ensure no `Message::from_string`, no `impl From<&str>`, no public free-form constructor (FR-003; PR-3c)
- [ ] T034 [US1] Define `Canonical<S: MarkingScheme>` in `marque-scheme` (or `marque-ism`) with sealed constructors: public `Canonical::from_cve(token: TokenId, scope: Scope) -> Self`; `pub(crate) Canonical::from_render(category, bytes, scope, &'static Location) -> Self`; `_scheme: PhantomData<S>` for type-level scheme separation (FR-001; PR-3c)
- [ ] T035 [US1] Define sealed-trait pattern per R-7: private `marque_scheme::canonical::sealed::Sealed`; public `CanonicalConstructor<S>: sealed::Sealed`; crate-private `EngineConstructor<S>` impl; engine instantiates and holds the only impl (FR-001, R-7; PR-3c)
- [ ] T036 [US1] Compile-fail tests at `crates/scheme/tests/canonical_unconstructable.rs` demonstrating no `Box<str> → Canonical<S>` path exists; downstream rule crates cannot implement `CanonicalConstructor<S>` (Sealed not in scope) (SC-012; PR-3c)
- [ ] T037 [US1] Define `FixIntent<S>` in `crates/rules/src/fix_intent.rs` with `target_span`, `replacement: ReplacementIntent<S>`, `confidence: Confidence`, `feature_ids: SmallVec<[FeatureId; 4]>`, `message: Message` (FR-025; PR-3c)
- [ ] T038 [US1] Define `ReplacementIntent<S>` enum: `Cve { token, scope }`, `Render { category, directive, scope }`, `Delete` (FR-025; PR-3c)
- [ ] T039 [US1] Define `RenderDirective<S>` as scheme-associated type; for `CapcoScheme`, encode `SciMarking { control, comps, sub_comps }` and other open-vocab shapes (PR-3c)
- [ ] T040 [US1] Add `marque-scheme` dependency to `marque-rules` `Cargo.toml`; verify `cargo check --workspace` passes; verify acyclic graph (Constitution VII; consolidated plan Appendix D updated graph; PR-3c)
- [ ] T041 [US1] Reshape `AppliedFix` v2 in `crates/rules/src/applied_fix.rs`: `rule: RuleId(scheme, predicate_id)`, `severity`, `span`, `fix: AppliedFixDetail`, `message: Message`, `timestamp`, `classifier_id`, `dry_run`; `AppliedFixDetail { replacement: FixReplacement::Strict|Decoder { canonical: Canonical<CapcoScheme>, confidence }, original_span: Span, original_digest: Blake3Hash }` (FR-002, FR-004, FR-026, FR-035; PR-3c)
- [ ] T042 [US1] Reshape `Diagnostic` v2 in `crates/rules/src/diagnostic.rs`: `rule: RuleId`, `severity`, `span`, `message: Message`, `citation: Citation`, `fix: Option<FixIntent<CapcoScheme>>` (FR-003, FR-018; PR-3c)
- [ ] T043 [US1] Define `Citation::new(section: SectionRef, page: PageNumber, document: AuthoritativeSource)` with construction-time validation (section in normative range, page in document range); `SectionRef` parsed structure not raw string (FR-018; PR-3c)
- [ ] T044 [US1] Migrate rule-ID convention from `E###`/`W###`/`S###`/`C###` to `RuleId(scheme, predicate_id)` per R-3 dot-nested form across `crates/capco/src/rules.rs`; emit one-time `docs/refactor-006/legacy-rule-id-map.md` listing every retired ID with successor; **migrate the existing R001 flat string `DECODER_RULE_ID = "R001"` (`crates/engine/src/engine.rs:50`) to `RuleId("engine", "r001.decoder-recognized")` per FR-044 sentinel-scheme convention** (FR-026, FR-044, R-3; PR-3c)
- [ ] T045 [US1] Migrate every rule's `evaluate` to construct `FixIntent<CapcoScheme>` instead of `FixProposal`; rules emit closed-CVE `ReplacementIntent::Cve { token, scope }` for known tokens, `ReplacementIntent::Render { category, directive, scope }` for open-vocab (FR-025; PR-3c)
- [ ] T046 [US1] Migrate every rule's diagnostic message construction from `format!`-built strings to `Message::new(MessageTemplate::..., MessageArgs { ... })`; closed-enum dispatch only (FR-003; PR-3c)
- [ ] T047 [US1] Implement `Engine::fix_inner` promotion path: filter by `Confidence::combined() ≥ threshold`; sort + non-overlap (C-1, I-3); render `FixIntent<S>` to `Canonical<S>` via `S::render_canonical`; construct `AppliedFix` via `__engine_promote(...)`; pure `marque-engine` ownership (PR-3c)
- [ ] T048 [US1] Implement `MarkingScheme::render_canonical<C: CanonicalConstructor<Self>>(&FixIntent<Self>, &RenderContext) -> Canonical<Self>` for `CapcoScheme` in `crates/capco/src/scheme.rs`; closed-CVE branch dispatches to `Canonical::from_cve`; open-vocab branch builds via `EngineConstructor::build_open_vocab` (PR-3c, FR-001)
- [ ] T049 [US1] Delete `engine.rs::build_decoder_diagnostic` carve-out: remove `proposal.original = ""` branch around `FixProposal::new(..., "", replacement, ...)` call (currently `engine.rs:1369-1384`); decoder produces `FixIntent` like every other path (FR-028; PR-3c)
- [ ] T050 [US1] Delete `engine.rs:1389` `format!("decoder-recognized canonical form: {replacement:?}")` interpolation; replace with `Message::new(MessageTemplate::DecoderRecognized, MessageArgs { token: Some(token_id), ..MessageArgs::default() })` (FR-003; PR-3c)
- [ ] T051 [US1] Implement decoder open-vocab lockout: `DecoderRecognizer` recognizing an open-vocab token produces `Parsed::Ambiguous` with diagnostic-only output, no `FixProposal` (FR-027; PR-3c)
- [ ] T052 [US1] Cutover `MARQUE_AUDIT_SCHEMA` from `marque-mvp-2` to `marque-1.0` at build time; **single-value validation, no accept-list** (per source plan §10.1: "single value, not an accept-list"); re-export as `marque_engine::AUDIT_SCHEMA_VERSION = "marque-1.0"` (FR-034, FR-035; PR-3c)
- [ ] T053 [US1] Bake reserved slots into `marque-1.0` schema for `FeatureId::PrecedingFixPenalty` (PR 7) and `MessageTemplate::ReparseFailed` (R002, PR 7) per FR-035 (PR-3c)
- [ ] T054 [US1] Delete `from_parsed_unchecked` adapter; rules consume `&CanonicalAttrs` constructed only via the explicit `MarkingScheme::canonicalize(ParsedAttrs<'_>) -> CanonicalAttrs` path (PR-3c)
- [ ] T055 [P] [US1] Implement deterministic NDJSON canary scan at `crates/engine/tests/canary_scan.rs`: for each emitted `AppliedFix`, scan serialized line for any contiguous ≥4-byte sequence appearing in input but not in span numerals / BLAKE3 digests / enumerated identifier values; fail on any leak. **Coverage-equivalence assertion**: this canary deterministically catches the same regression class (input-byte leakage in `AppliedFix` serialization, including `proposal.original` / `proposal.replacement` / message-arg fields) that the `core_error_isolation.rs` masking pin (#257) used to mask. The masking pin can be retired in T058 only because this canary provides equivalent (in fact stronger — input-agnostic) coverage. PR 3c review MUST verify both predicates: (a) canary green on the post-3c codebase, (b) canary fires on a pre-fix HEAD synthetic regression (e.g., interpolating `replacement` into a message string) (SC-001; PR-3c)
- [ ] T056 [P] [US1] Add corpus regression × {3a + 3b + 3c} CI matrix run; validate the full keystone subsequence is correct (SC-014; PR-3c)
- [ ] T057 [US1] Run R-8 SC-010 decision tree at PR 3c: measure post-3c mangled-corpus accuracy; if ≥ 0.85 keep floor; if < 0.85 split corpus into `mangled-closed-vocab/` (≥ 0.85) and `mangled-open-vocab/` (diagnostic-only); record the decision of record in `tests/corpus/mangled/threshold.toml`; back out PR 3c at merge gate if accuracy < 0.80 or non-lockout regression (SC-010, R-8; PR-3c)
- [ ] T058 [P] [US1] Remove the masking-pin in `core_error_isolation.rs` (#257) — the carve-out closes at PR 3c; add regression test demonstrating fix necessity (**must fail on pre-fix HEAD** per source plan §6 masking-pin discipline rule 5) (FR-039 mandatory close-on-PR; PR-3c)
- [ ] T058a [P] [US1] **I-5 snapshot test** at `crates/engine/tests/audit_sequence_snapshot.rs` per source plan §5 amendment: apply a fixed input through `Engine::fix_inner`, snapshot the emitted `Vec<AppliedFix>` order, assert byte-identical across re-runs (catches monotonic-append regression — `Engine::fix_inner` reordering audit records post-promotion) (PR-3c)
- [ ] T058b [P] [US1] **I-6 mutation test** at `crates/engine/tests/confidence_threshold_mutation.rs` per source plan §5 amendment: a `cfg(test)`-gated build-flag swaps `Confidence::combined()` for `Confidence::recognition()` in the engine filter; asserts the SC-010 mangled-corpus accuracy gate regresses below baseline (catches accidental introduction of a second threshold operator) (PR-3c)

**Checkpoint**: Audit-record content-ignorance is a type invariant; canary scan over five-corpus sweep returns zero leaks; legacy rule-IDs retired; cutover to `marque-1.0` complete.

---

## Phase 4: User Story 2 — Page-level rollup is correct for foreign and joint markings (Priority: P1)

**Goal**: Delete the `MarkingClassification::Us` hardcode at `crates/capco/src/scheme.rs:365`; widen `expected_classification()` to `Option<MarkingClassification>`; drive page rollup through `MarkingScheme::project(Scope::Page, ...)`; delete `PageContext`.

**Independent Test**: A targeted corpus fixture set at `tests/corpus/foreign/` (pure_foreign_banner.json, FGI banner roll-up, NATO-only, JOINT) lints to a banner that retains foreign provenance in **100%** of cases (SC-002).

**Quality gate**: [`checklists/correctness.md`](./checklists/correctness.md) §1 (lattice/rollup — CHK001/002/003/004/006 GATE; CHK005/008/013/014); §5.1 (FD&R Tables 2/3 — CHK040 GATE; CHK041–CHK047 incl. four GATEs for §D.2 banner-precedence rules 17/23/26/27); §5.2 (Table 4 marking order — CHK048–CHK050); §6 (CHK068 #276 GATE, CHK069 #261). Reviewer clears all `[GATE]` items in §1 + §5.1 before merging PR 5 / 6.

### PR 5 — Foreign banner correctness

- [ ] T059 [US2] Widen `expected_classification()` return type to `Option<MarkingClassification>` in `crates/capco/src/scheme.rs`; delete the `MarkingClassification::Us` hardcode at `:365` (FR-007; PR-5)
- [ ] T060 [US2] Update `page_context_to_attrs` (and `Scope::Page` projection in PR 6) so a pure-foreign page produces `expected_classification() = None` or a foreign-marker-bearing classification; never silently `Us` (FR-007; PR-5)
- [ ] T061 [US2] Implement `FgiSet::render_canonical` (or equivalent in `crates/capco/src/lattice.rs`) to drop the redundant `FGI` token only when a country trigraph is present per CAPCO §H.7; otherwise retain the `FGI` marker (FR-008, #261; PR-5)
- [ ] T062 [P] [US2] Create `tests/corpus/foreign/pure_foreign_banner.json` corpus fixture: page composed entirely of `(C//FGI DEU)` portions; expected banner retains FGI DEU marker (SC-002; PR-5)
- [ ] T063 [P] [US2] Create `tests/corpus/foreign/joint_us_uk.json` and `tests/corpus/foreign/nato_only_page.json` corpus fixtures (SC-002; PR-5)
- [ ] T064 [P] [US2] Add CI grep guard against re-introduction of `MarkingClassification::Us` hardcode in projection code paths; documented in `docs/refactor-006/regression-guards.md` (US2 acceptance; PR-5)

### PR 6 — Scope::Page projection cutover (sub-PRs 6a / 6b / 6c)

- [ ] T065 [US2] PR 6a: wire `MarkingScheme::project(Scope::Page, &portions) -> ProjectedMarking` behind `cfg(feature = "scope_page_projection")` (default off); both code paths present in tree; CI runs both (FR-006; PR-6a)
- [ ] T066 [US2] PR 6a: implement `ProjectedMarking { scope, classification, sci_set, sar_set, fgi_set, dissem_us, dissem_nato, aea, declassify_on, provenance }` per data-model.md (FR-006; PR-6a)
- [ ] T067 [P] [US2] Create `benches/lint_100kb_multipage/` Criterion bench scaffolding at PR-6a; **establish the `PageContext`-path baseline by running the bench against the PageContext code path that is still default at 6a** (the bench does not exist at PR-0; baseline is captured during 6a). T068 then asserts projection-path latency ≤ this baseline + 10% at PR-6b (FR-031; PR-6a)
- [ ] T068 [US2] PR 6b: bench both code paths via `lint_100kb_multipage`; assert projection-path latency ≤ baseline + 10% (FR-031); assert `Vec<Diagnostic>` semantic equivalence between paths over fixture corpus (PR-6b)
- [ ] T069 [US2] PR 6c: flip default to projection; remove `cfg(feature = "scope_page_projection")`; delete `PageContext` struct + all references in `crates/engine/src/engine.rs`. **Note**: `CapcoMarking::join`'s `PageContext` delegation in `crates/capco/src/scheme.rs` is deleted earlier at PR-4 (T115) per FR-014 + source plan §4 PR-4 row — it MUST already be gone by the time T069 runs. (FR-006, clean break; PR-6c)
- [ ] T070 [P] [US2] Confirm `lint_100kb_multipage` post-merge measures projection-only path; assert against PR-0 baseline + 10% (FR-031; PR-6c)
- [ ] T071 [P] [US2] CI matrix: corpus regression × {6a-only, 6a + 6b, 6a + 6b + 6c} = 3 runs to verify each sub-PR independently correct (SC-014; PR-6a/6b/6c)
- [ ] T072 [P] [US2] Verify `tests/corpus/foreign/` fixtures pass at 100% post-cutover (SC-002; PR-6c)

**Checkpoint**: Foreign-banner correctness fixtures pass 100%; `PageContext` deleted; multi-page projection bench within baseline + 10%.

---

## Phase 5: User Story 3 — Pass-1 token rewrites do not corrupt pass-2 rule input (Priority: P2)

**Goal**: Phase-tag rules at registration with `Phase::Localized | WholeMarking`; engine re-parses buffer between passes; emit `R002` on re-parse failure; pre-pass-1 attrs cache for I-19 reshape-aware re-validation.

**Independent Test**: Property tests at `crates/engine/tests/two_pass_invariants.rs` shuffle pass-1 / pass-2 fix orderings and assert: (a) no overlap in promoted spans (I-18 / FR-022); (b) reshape-aware re-validation does not produce retroactive false positives (I-19 / FR-023); (c) `fix_10kb` Criterion bench shows two-pass overhead within SC-008 budget (FR-032). SC-007.

**Quality gate**: [`checklists/correctness.md`](./checklists/correctness.md) §2 (two-pass apply — CHK015 GATE, CHK018 GATE; CHK016/017/019/020/021/022/023/024). Reviewer clears CHK015 + CHK018 before merging PR 7.

### PR 7 — Phase-tagged pass split

- [ ] T073 [US3] Add `phase(&self) -> Phase` to the `Rule` trait in `crates/rules/src/lib.rs`; define `enum Phase { Localized, WholeMarking }` (FR-021; PR-7)
- [ ] T074 [US3] Declare `Phase` for every rule in `crates/capco/src/rules.rs`; rules needing both register two entries (one per phase) sharing a backend module — no `Phase::Both` escape hatch (FR-021; PR-7)
- [ ] T075 [US3] Implement engine registration check at `Engine::new`: `Phase::Localized` rule's emitted `FixIntent::target_span` must be sub-token-only; `Phase::WholeMarking` must cover full marking; reject violators with `EngineConstructionError::PhaseSpanShapeMismatch` (FR-021; PR-7)
- [ ] T076 [US3] Restructure `Engine::fix_inner` into two passes: pass 1 dispatches `Phase::Localized` rules' fixes via single-pass forward splice; re-parse the post-pass-1 buffer (PR-7)
- [ ] T077 [US3] Implement re-parse failure path: when `parse(post_pass_1_buffer)` fails, emit `R002` diagnostic carrying `contributing_pass1_fix_ids: SmallVec<[RuleId; 4]>`; retain pass-1 audit records; do not run pass 2; return pass-1 buffer as corrected document (FR-024; PR-7)
- [ ] T078 [US3] Define `R002` synthetic diagnostic: const `R002_RULE_ID = RuleId("engine", "r002.reparse-failed")` per FR-044 sentinel-scheme convention; `R002Diagnostic { contributing_pass1_fix_ids, failure_span, message }`; minted by `marque-engine`, never by rule crates (FR-024, FR-041, FR-044; PR-7)
- [ ] T079 [US3] Add `MessageTemplate::ReparseFailed` variant (already reserved at PR 3c per FR-035) with associated `MessageArgs` shape; implement display rendering (FR-003, FR-024; PR-7)
- [ ] T080 [US3] Implement pre-pass-1 attrs cache per R-4: `SmallVec<[CanonicalAttrs<'src>; 4]>` owned by `Engine::fix_inner` stack frame; populate per-marking only when the marking's span overlaps a pass-1 fix span; pass to `Phase::WholeMarking` rules via `RuleContext.pre_pass_1_attrs: Option<&CanonicalAttrs<'src>>` (FR-023, R-4; PR-7)
- [ ] T081 [US3] Implement pass 2 dispatch: for each `Phase::WholeMarking` rule, evaluate against post-pass-1 `CanonicalAttrs` plus `pre_pass_1_attrs`; I-19 reshape-aware re-validation per FR-023 (same `(scheme, predicate-id)` → no re-fire; different rule → fire); I-18 non-overlap with pass-1 spans, demote overlapping pass-2 diagnostics to `Severity::Suggest` not auto-applied (FR-022, FR-023; PR-7)
- [ ] T082 [US3] Add `FeatureId::PrecedingFixPenalty` variant (already reserved at PR 3c per FR-035); apply to E003 confidence reduction when a preceding pass-1 fix is staged (FR-035; PR-7)
- [ ] T083 [P] [US3] Property tests at `crates/engine/tests/two_pass_invariants.rs` covering FR-022 (I-18 non-overlap) and FR-023 (I-19 reshape-aware re-validation) under all fix-ordering permutations (SC-007; PR-7)
- [ ] T084 [P] [US3] Per-pass fix-invariant tests at `crates/engine/tests/fix_invariants.rs` covering Layer 3 invariants (I-1, I-2, I-4, I-18, I-19) per consolidated plan §6 (PR-7)
- [ ] T085 [P] [US3] Create `benches/fix_10kb/` Criterion bench: 10 KB document with both `Phase::Localized` and `Phase::WholeMarking` triggering; assert two-pass overhead within p95 ≤ 16 ms budget (FR-032; PR-7)

**Checkpoint**: Pass-split correctness property tests pass; `R002` emits on re-parse failure; pre-pass-1 attrs cache works; two-pass overhead within SC-008 budget.

---

## Phase 6: User Story 4 — Open-vocabulary input is never silently corrupted (Priority: P2)

> **Ordering note**: Phase 6 is numerically after Phases 3–5 but its PR (PR-2) ships **before** Phase 3 (PR-3a/3b/3c) per the dependency spine. Phase numbering here follows user-story priority; PR ordering follows the consolidated source plan §4. The `## Dependencies & Execution Order` section below is the authoritative implementation order.

**Goal**: Migrate four open-vocabulary admission sites in `parser.rs` to `Vocabulary<S>::shape_admits` — three `is_ascii_alphanumeric()` byte-class checks (`:1453`, `:1481`, `:1493`) plus the FGI trigraph silent-skip (`:1011-1024`, which uses `if token.len() == 3 { CountryCode::try_new(...) }` rather than `is_ascii_alphanumeric` but has the same fix shape); `parse_fgi_marker` returns `None` (not degraded `Some`) on shape failure; introduce `FgiMarker::SourceConcealed | Acknowledged` discriminant.

**Independent Test**: `tests/parser/fgi_silent_skip_guard.rs` asserts the four parser sites return `None` on shape-admits failure (SC-011); `crates/capco/tests/parse_render_roundtrip.rs` round-trip property catches silent semantic degradation across the strict-path corpus.

**Quality gate**: [`checklists/correctness.md`](./checklists/correctness.md) §3 (open-vocab parser — CHK026 GATE, CHK028 GATE; CHK025/027/029/030/031); §5.3 (per-marking grammar — CHK051 GATE for SCI grammar, CHK059 GATE for delimiter conflation; CHK052–CHK066). Reviewer clears CHK026 + CHK028 + CHK051 + CHK059 before merging PR 2.

### PR 2 — Parser shape_admits + FgiMarker discriminant

- [ ] T086 [US4] Extend `Vocabulary<S>` trait in `marque-scheme` with `shape_admits(category: CategoryId, bytes: &[u8]) -> bool`; total over `(CategoryId, &[u8])`; closed-CVE-only categories return `lookup(bytes).is_some()` (FR-015; PR-2)
- [ ] T087 [US4] Implement `shape_admits` for `CapcoScheme` in `crates/capco/src/scheme.rs`; bake category-shape rules from `crates/capco/docs/CAPCO-2016.md` at build time per Phase 5 metadata-surface mechanism (FR-015; PR-2)
- [ ] T088 [US4] Migrate `crates/core/src/parser.rs:1011-1024` (FGI trigraph silent-skip — currently `if token.len() == 3 { CountryCode::try_new(...) }` silently drops non-trigraph tokens; does NOT use `is_ascii_alphanumeric()` like the other three sites) to `Vocabulary<CapcoScheme>::shape_admits`-gated admission; on shape failure return `None` (per FR-016) rather than dropping the token (FR-015 + FR-016; PR-2)
- [ ] T089 [US4] Migrate `crates/core/src/parser.rs:1453` from `is_ascii_alphanumeric()` to `shape_admits` (FR-015; PR-2)
- [ ] T090 [US4] Migrate `crates/core/src/parser.rs:1481` from `is_ascii_alphanumeric()` to `shape_admits` (FR-015; PR-2)
- [ ] T091 [US4] Migrate `crates/core/src/parser.rs:1493` from `is_ascii_alphanumeric()` to `shape_admits` (FR-015; PR-2)
- [ ] T092 [US4] Add CI grep guard at `tools/regression-grep.sh` (or equivalent) flagging re-introduction of `is_ascii_alphanumeric()` in `crates/core/src/parser.rs` (FR-015; PR-2)
- [ ] T093 [US4] Modify `parse_fgi_marker` in `crates/core/src/parser.rs` to enumerate **three return cases** per source plan §2.4 + §5 I-9: (a) `None` when post-prefix bytes fail `shape_admits` (rejecting malformed input — was `Some(FgiMarker { countries: [] })`); (b) `Some(FgiMarker::SourceConcealed)` for lawful source-concealed FGI per CAPCO §H.7 p123 (no trigraphs in lawful position); (c) `Some(FgiMarker::Acknowledged { countries })` for one or more validated trigraphs. The post-failure shape MUST be unrepresentable. (FR-016, FR-017; PR-2)
- [ ] T094 [US4] Replace `FgiMarker { countries: Box<[CountryCode]> }` with discriminant `enum FgiMarker { SourceConcealed, Acknowledged { countries: SmallVec<[CountryCode; 4]> } }` in `crates/ism/src/` and `crates/capco/src/lattice.rs`; constructor for `Acknowledged` requires `countries.len() >= 1` (FR-017; PR-2)
- [ ] T095 [US4] Audit `crates/capco/src/rules.rs` for `countries.is_empty()` pattern; migrate to explicit variant matching (`FgiMarker::SourceConcealed => ...` vs. `FgiMarker::Acknowledged { countries } => ...`); verify zero surviving `is_empty()` patterns post-migration (FR-017, SC-011; PR-2)
- [ ] T096 [P] [US4] Test at `tests/parser/fgi_silent_skip_guard.rs` asserting the four cited parser sites return `None` for `shape_admits`-failing input (SC-011; PR-2)
- [ ] T097 [P] [US4] Layer 2 parse-render round-trip property test at `crates/capco/tests/parse_render_roundtrip.rs` over the full strict-path corpus (US4 acceptance; PR-2)
- [ ] T098 [P] [US4] Add `lint_latency` Criterion bench p99 tail-percentile assertion (`Arc<dyn Vocabulary<S>>` precludes cross-crate devirtualization — measure per-token vtable-miss cost at the tail) (FR-030; PR-2)

**Checkpoint**: Open-vocab parser corruption closed; FgiMarker shape-collision unrepresentable; round-trip property holds across strict-path corpus.

---

## Phase 7: User Story 5 — Citations in rules are mechanically verifiable (Priority: P2)

**Goal**: Mature the F.1 corpus-fidelity gate from sparse (one fixture per existing rule) to full per-cited-authority coverage; preserve citation hygiene through PRs 4–9.

**Independent Test**: Citation-lint passes at 100%; F.1 corpus fixture coverage at 100% over all cited authorities; SC-005 / SC-006.

**Quality gate**: [`checklists/correctness.md`](./checklists/correctness.md) §4 (citation fidelity, mechanical — CHK032 GATE, CHK077 GATE; CHK033–CHK039); §5 in full (semantic agreement with CAPCO ruleset — the citation-lint catches well-formed-but-misattributed citations, but §5 questions whether the rule TEXT actually agrees with what CAPCO says — distinct quality axis). PR 0.6 reviewer clears CHK032 + CHK077; PR 10 reviewer clears the full §4.

**NOTE**: PR 0.5 / 0.6 already shipped citation-lint scaffold + preemptive defect fix in Phase 2. This phase covers the maturation work.

### PR 10 — F.1 corpus gate maturation

- [ ] T099 [US5] Extend F.1 corpus fixture coverage at `crates/capco/tests/citation_fidelity.rs` to require ≥1 fixture per cited authority (not just per existing rule); audit every `Constraint`/`PageRewrite`/`Rule` citation (FR-019, SC-006; PR-10)
- [ ] T100 [US5] Add corpus fixtures for cited authorities lacking one (the set is unknown until Phase 7 audit runs; expect a long tail) (FR-019; PR-10)
- [ ] T101 [US5] Land 8C vendored-source registry declarative — formalize the `AuthoritativeSource::Capco2016` mapping table; pin `crates/capco/docs/CAPCO-2016.md` BLAKE3 digest at build time; CI fails on digest mismatch (FR-018; PR-10)
- [ ] T102 [P] [US5] Verify citation-lint clean across the post-refactor codebase (every PR-introduced citation passes); `cargo run --manifest-path tools/citation-lint/Cargo.toml -- .` exits 0 (the crate is out-of-workspace per Constitution III; `-p citation-lint` does NOT work from the repo root) (SC-005; PR-10)
- [ ] T103 [P] [US5] Verify F.1 100% coverage; document any remaining gaps in `docs/refactor-006/citation-coverage-report.md` (SC-006; PR-10)

**Checkpoint**: 100% citation-lint pass, 100% F.1 coverage; vendored-source pinning enforced.

---

## Phase 8: User Story 6 — Lattice projection laws hold for every marking category (Priority: P2)

**Goal**: Land per-category `Lattice` impls satisfying associativity, commutativity, idempotency, identity-with-bottom; cross-axis dominance fixtures (FOUO eviction by classification > U; FOUO eviction by non-FD&R dissem; FGI banner roll-up; SCI cross-system canonicalization; AEA exemption commingling). PR 3.7 fills the lattice design doc as a hard gate before PR 4.

**Independent Test**: Property tests at `crates/capco/tests/category_lattice_laws.rs` (assoc/comm/idem/identity per category) and `crates/capco/tests/cross_axis_dominance.rs` pass for every category in `CapcoScheme::categories()`; `tests/corpus/lattice/` corpus regression sweep covers cross-axis fixtures end-to-end. SC-003 / SC-004.

**Quality gate**: [`checklists/correctness.md`](./checklists/correctness.md) §1 (lattice algebra — CHK010 GATE for formal join semantics in §§2–8 of `2026-05-01-lattice-design.md`; CHK007/008/011/012/013/014); §5.3 (per-marking grammar quality dimensions tested against CAPCO §H — CHK051 SCI, CHK053 SAR, CHK054 AEA full eviction, CHK058–CHK066 dissem precedence). Reviewer clears CHK010 before PR 3.7 merges (the lattice §-resolution gate); PR 4 reviewer clears the rest of §1.

### PR 3.7 — Lattice §-resolution gate

- [ ] T104 [US6] Fill `docs/plans/2026-05-01-lattice-design.md` §§2–8 with §-citations to `crates/capco/docs/CAPCO-2016.md` for every category in `CapcoScheme::categories()` (FR-013; PR-3.7)
- [ ] T105 [US6] Document formal join semantics in §§2–8 — precondition / postcondition functional form, NOT prose; per-category lattice law derivation (FR-013; PR-3.7)
- [ ] T106 [US6] Document worked examples ≥2 per category in §§2–8, including edge cases the §-citation calls out (FR-013; PR-3.7)
- [ ] T107 [US6] Document property-test fixture file/test names in §§2–8 (FR-013; PR-3.7)
- [ ] T108 [US6] Resolve every §10 open question to §-citation + explicit decision; remove the "explicitly deferred to a tracked issue" escape valve from §9 acceptance (FR-013; PR-3.7)
- [ ] T108a [US6] **Amend `docs/plans/2026-05-01-lattice-design.md` itself** per consolidated source plan §11.3 lines 970-979: (a) rewrite §3 Q3 as confirm-and-document since `noforn-clears-rel-to` is already a declared `PageRewrite` per Phase B (CLAUDE.md); (b) update §9 acceptance to remove the escape valve (parallel to T108's resolution); (c) update §10 item #3 to reflect resolved status. The lattice doc is a living deliverable — PR 3.7 commits these amendments as part of the spike. (FR-013; PR-3.7)
- [ ] T109 [US6] Define cross-axis dominance fixture set for §§2–8: FOUO eviction by classification > U; FOUO eviction by non-FD&R dissem; FGI banner roll-up (#276); SCI cross-system canonicalization; AEA exemption commingling (FR-012; PR-3.7)
- [ ] T110 [US6] PR 3.7 acceptance: named reviewer in PR description who has confirmed each category's worked examples by hand against the §-citation; default owner consolidated-plan author or named successor; default deadline 2 weeks from PR 3c merge (US6 acceptance; PR-3.7)

### PR 4 — Per-category Lattice impls + property tests

- [ ] T111 [US6] Extend `Vocabulary<S>` with `is_fdr_dissem(token: TokenId) -> bool` per FR-010; bake from `crates/capco/docs/CAPCO-2016.md` §H.8 at build time (Phase 5 metadata-surface mechanism); returns false for non-dissem categories (FR-010; PR-4)
- [ ] T112 [US6] Implement per-category `Lattice` impls in `crates/capco/src/lattice.rs` for every category in `CapcoScheme::categories()` per the formal join semantics from PR 3.7 (FR-011; PR-4)
- [ ] T113 [US6] Wire FOUO `SupersessionSet` over the dissem axis through `is_fdr_dissem` so FOUO is evicted by any non-FD&R dissem token (FR-009, FR-010; PR-4)
- [ ] T114 [US6] Wire cross-axis FOUO eviction by classification > U through the `Constraint` evaluator (FR-009; PR-4)
- [ ] T115 [US6] Delete `CapcoMarking::join`'s `PageContext` delegation; clean break, no equivalence shim (FR-014; PR-4)
- [ ] T116 [P] [US6] Property tests at `crates/capco/tests/category_lattice_laws.rs` — assoc/comm/idem/identity-with-bottom per category from `CapcoScheme::categories()` (FR-011, SC-004; PR-4)
- [ ] T117 [P] [US6] Cross-axis dominance fixture tests at `crates/capco/tests/cross_axis_dominance.rs` covering the five fixture classes from T109 (FR-012, SC-003; PR-4)
- [ ] T118 [P] [US6] Create `tests/corpus/lattice/` corpus regression fixtures from PR 3.7's worked examples; end-to-end coverage of cross-axis behavior (FR-012; PR-4)
- [ ] T119 [P] [US6] Create `tests/corpus/prose-positive/` corpus fixtures: true-positive markings in prose context that MUST fire (US6 acceptance; PR-4)
- [ ] T119a [US6] **Predicate coverage catalog (#307)** — implement Groups B (FD&R Table 2 defaults: FGI/uncaveated/caveated → NOFORN/RELIDO with date-pivot context) and E (cross-axis dominance / requires: SI-G requires ORCON, SI requires class C/S/TS, SI-[comp] requires TS, TK requires TS/S, TK-{BLUEFISH/IDITAROD/KANDIK} require NOFORN+TS, RSEN requires TK, RSEN TS/S only, RD requires TS/S/C, RD requires NOFORN with §123/§144 WARN exception, CNWDI/RD-SG/FRD-SG require TS/S, TFNI requires TS/S/C, RD/FRD dominates TFNI, classified dominates DOD/DOE UCNI, ORCON dominates ORCON-USGOV, IMCON requires TS/S, OC/OC-USGOV ejects RELIDO, DISPLAY ONLY dominates RELIDO, classified dominates FOUO/LIMDIS, PROPIN/DSEN/FISA dominate FOUO, classified dominates non-IC dissem set) as declarative `Constraint` data on `CapcoScheme`; verify each citation at point of implementation per Constitution VIII. SAP-requires-class (E2.1) ALSO lands here. (#307 Groups B + E.1–E.5; PR-4)
- [ ] T119b [P] [US6] **Predicate coverage catalog (#307) Group A — required-pair fixes** — separable small PR landing BEFORE T119a if helpful: HCS-O / EXDIS / NODIS / SBU-NF / LES-NF require NOFORN (mirror of HCS-P fix from PR #303); declarative `Constraint::requires` + scheme_equivalence.rs pinning tests; closes #304 (#307 Group A; can land as own PR, or fold into T119a)
- [ ] T119c [US6] **Predicate coverage catalog (#307) Group C — Table 3 rollup gaps** — cross-references existing CAPCO-CONTEXT §3.2 gap list: Rule 17 RELIDO date-pivot (depends on B group); Rule 23 TEYE/ACGU/FVEY tetragraph expansion using `marque-capco::vocab` tables; Rule 26 REL TO + DISPLAY ONLY → DISPLAY ONLY; Rule 27 dual-channel composition. Land as `PageRewrite` declarations, NOT procedural. (#307 Group C; PR-4 or PR-5)

**Checkpoint**: Lattice laws + cross-axis dominance fixtures pass 100%; `PageContext` delegation deleted; lattice corpus regression green; #307 Groups A/B/C/E predicate catalog covered (Group D handled in PR 9 — see T135a).

---

## Phase 9: User Story 7 — Performance is preserved through the refactor (Priority: P3)

**Goal**: Cross-cutting perf-gate verification through every PR. Per-PR Criterion bench gates plus measurement-gating discipline (>5% mean OR p99 regression backs out the change).

**Independent Test**: Four Criterion benches gate the relevant PRs: `fix_throughput` R² ≥ 0.9 (FR-029); `lint_latency` p95 ≤ 16 ms + p99 ≤ baseline + 5% (FR-030); `lint_100kb_multipage` ≤ baseline + 10% (FR-031); `fix_10kb` within SC-008 budget (FR-032). Measurement-gating discipline FR-033.

**NOTE**: Per-bench tasks are interleaved into Phases 3–8 above (T067, T085, T098). Phase 9 covers cross-cutting bench discipline + the bench-check.sh regression gate.

- [ ] T120 [US7] Wire `tools/bench-check.sh` to read `benches/baselines/2026-05-pre-refactor.json` and assert against post-refactor measurements; fail PR on >5% mean OR p99 regression (FR-033; cross-cutting)
- [ ] T121 [US7] Per-PR bench gate verification: each PR in the sequence runs the relevant Criterion benches and asserts against baselines per FR-029..FR-033 (cross-cutting)
- [ ] T122 [US7] Implement measurement-gated rollback discipline at the PR-review level: a PR triggering >5% mean OR p99 regression must back out, not relax the baseline (FR-033; cross-cutting)
- [ ] T123 [P] [US7] Verify SC-008 (p95 ≤ 16 ms; p99 within baseline + 5%) post-refactor on `lint_latency`; SC-009 sub-bullets on `lint_100kb_multipage`, `fix_throughput`, `fix_10kb` (US7 acceptance; cross-cutting)

**Checkpoint**: All four perf gates green at the end of the refactor sequence; measurement-gating discipline enforced PR-by-PR.

---

## Phase 10: User Story 8 — Refactor PRs are independently revertable (Priority: P3)

**Goal**: Granular revertability — each keystone sub-PR (3a, 3b, 3c) and each PR 6 sub-commit (6a, 6b, 6c) passes corpus regression independently in CI matrix; any single PR is mechanically revertable without orphaned types / functions / dependencies.

**Independent Test**: SC-014 — CI matrix during keystone window runs corpus regression × {3a-only, 3a+3b, 3a+3b+3c} = 3 runs and × {6a-only, 6a+6b, 6a+6b+6c} = 3 runs, each passing; revert of any single PR leaves workspace buildable.

**NOTE**: Per-PR CI matrix tasks are interleaved into Phases 3–4 (T025, T029, T056, T071). Phase 10 covers cross-cutting revertability discipline.

- [ ] T124 [US8] Document per-PR revertability checklist at `docs/refactor-006/revertability-discipline.md`: for each PR in the sequence, list (a) types/functions touched, (b) test fixtures touched, (c) the revert sequence if the PR backs out (US8 acceptance; cross-cutting)
- [ ] T125 [US8] Verify keystone window CI matrix (T025 + T029 + T056) all green = SC-014 keystone subsequence verification (cross-cutting)
- [ ] T126 [US8] Verify PR 6 sub-commit CI matrix (T071) all green = SC-014 sub-commit verification (cross-cutting)

**Checkpoint**: All keystone + PR 6 sub-commits pass corpus regression independently.

---

## Phase 11: Polish & Cross-Cutting (PR 8 + PR 9 + final discipline)

**Purpose**: Outstanding work scoped to PR 8 (priors-bake — third problem class, NOT G13 closure) and PR 9 (parser separators, dissem_us/nato split, banner-validation rule migration, ATOMAL/BOHEMIA, NATO-portion declarative Constraint), plus final discipline checks.

**Quality gate**: [`checklists/correctness.md`](./checklists/correctness.md) §6 (known-defect coverage — CHK070 #271 dissem split, CHK071 #246 ATOMAL/BOHEMIA, CHK072 #265 NATO-portion `REL TO USA, NATO`, CHK073 #106 separator spans); §5.3 (CHK066 ATOMAL/BOHEMIA category consistency between FR-046 and FR-047); §7 (process discipline — CHK078 GATE, CHK079 GATE, CHK082, CHK083 for PR 10 final-polish bench drift). PR 9 reviewer clears CHK066 + CHK070–CHK073; PR 10 reviewer clears CHK078 + CHK079 (keystone revert + 3.7 stall-recovery sign-off).

### PR 8 — Decoder priors (third problem class)

- [ ] T127 Bump `marque-priors-3` priors-bake schema (independent of audit schema bump per FR-036) (PR-8)
- [ ] T128 [P] Implement decoder prose null-hypothesis priors per #258 (third problem class; PR 8 delivers logic, does NOT claim closure of #258) (PR-8)
- [ ] T129 [P] Implement decoder folding logic per #260 (third problem class; PR 8 delivers logic, does NOT claim closure of #260) (PR-8)
- [ ] T130 Remove the masking-pin in `corpus_accuracy.rs` (#258) — the carve-out closes at PR 8; add regression test demonstrating fix necessity (**must fail on pre-fix HEAD** per source plan §6 masking-pin discipline rule 5) (FR-039 mandatory close-on-PR; PR-8)

### PR 9 — Parser separators + dissem split + banner-val migration

- [ ] T131 Implement parser separator-span tracking (`/`, `//`, whitespace boundaries) as first-class `Span` values in `ParsedAttrs<'src>` per consolidated plan §11; required for downstream banner-validation rule reshape (FR-045; PR-9)
- [ ] T132 Split `dissem` field into position-attributed `dissem_us: Box<[DissemControl]>` and `dissem_nato: Box<[DissemControl]>` in `ParsedAttrs<'src>`, `CanonicalAttrs`, and `ProjectedMarking`; update rule consumers (FR-046, #271 / 7B; PR-9)
- [ ] T133 Migrate banner-validation rules to consume `&ProjectedMarking` (post PR 6 cutover; before PR 9 they consume the post-projection shape via shim) (FR-006; PR-9)
- [ ] T134 [P] Add ATOMAL / BOHEMIA NATO-specific marking handling — closed-CVE values land via the `Vocabulary<S>` build-time generation pipeline; tokens routed to `dissem_nato` per FR-046 (FR-047, #246; PR-9)
- [ ] T135 [P] Implement NATO-portion-in-US-document declarative `Constraint` requiring `REL TO USA, NATO` derivation in the banner; replaces procedural NATO-rule branches (FR-048, #265; PR-9)
- [ ] T135a [P] **Predicate coverage catalog (#307) Group D — token canonicalization** in the parser/recognizer layer. Uniform deprecation pattern (legacy long-form → abbreviation per §H.4): `HUMINT` → `HCS`; `HUMINT CONTROL SYSTEM` → `HCS`; `COMINT` → `SI`; `SPECIAL INTELLIGENCE` → `SI`; `ECI` / `EXCEPTIONALLY CONTROLLED INFORMATION` (with/without comp) → `SI`. Other recognizer-layer normalizations: `EL` / `ENDSEAL` (with/without comp) → `SI`; `KDK` / `KLONDIKE` (with/without comp) → `TK`; bare `HCS` → suggest HCS-O/HCS-P/HCS-O-P (S/TS only; bare HCS at C is legacy → suggest contact originator); bare `RSV` (require 3-alnum compartment, S/TS only); `EYES` / `EYES ONLY` `/`-delimited country list → `REL TO ` comma-delimited list (collision with dissem-category separator per CAPCO-CONTEXT §1.2). NOTE: `TALENT KEYHOLE` and `TK` are BOTH accepted (no canonicalization). Recognizer-layer normalization, NOT constraint layer. (#307 Group D; PR-9)

### Final polish

- [ ] T136 [P] Run final corpus regression sweep × five corpora × two recognizers = 10 CI runs; verify all green (cross-cutting)
- [ ] T137 [P] Verify all SC-001..SC-014 success criteria measured and met; document in `docs/refactor-006/sc-completion-report.md` (cross-cutting)
- [ ] T138 [P] Verify zero surviving MASKING-PIN tags reference closed issues (SC-013); masking-pin lint clean; promote-callsite lint clean (SC-013; cross-cutting)
- [ ] T138a [P] **FR-037 absence-check** per source plan §10.1 amendment: implement `tools/audit-cleanup-check.sh` (or fold into existing CI step) asserting (a) no `crates/audit-reader/` directory exists; (b) no `audit-reader`, `marque-audit-reader`, or analogous reader feature appears in any workspace `Cargo.toml`; (c) no public re-export under `marque_engine::reader::*` exists. Wire into CI as a polish-phase gate (FR-037; cross-cutting)
- [ ] T139 [P] Update `CLAUDE.md` workspace overview to reflect post-refactor architecture; document the post-cutover crate dependency graph (Constitution VII; cross-cutting)
- [ ] T140 [P] Run `quickstart.md` validation: walk through "How to add a new rule" example end-to-end; verify the example compiles and the audit record matches `contracts/audit-record.md` (cross-cutting)
- [ ] T141 Update workspace `Cargo.toml` `rust-version` floor verification; verify `cargo check --workspace --all-targets --all-features` passes; verify `wasm-pack build crates/wasm` passes (Constitution III; cross-cutting)

---

## Dependencies & Execution Order

### PR-sequence dependency spine (the actual implementation order)

```text
PR 0 ─┬─→ PR 0.5 ─→ PR 0.6 ─┐
      └─→ PR 1               │
                              ▼
                            PR 2 ─→ PR 3a ─→ PR 3b ─→ PR 3c ─┬─→ PR 3.7 ─→ PR 4 ─→ PR 5 ─→ PR 6a ─→ PR 6b ─→ PR 6c
                                                              ├─→ PR 7
                                                              ├─→ PR 8
                                                              └─→ PR 9
                                                                    │
                                                                    ▼
                                                                  PR 10 (F.1 maturation; runs after lattice + banner-val migration land)
```

Read `→` as "blocks". PR 3.7 absolutely gates PR 4 (lattice §-resolution spike must complete before lattice impls land); per assumption, if PR 3.7 stalls, PRs 4–10 stall. PR 7 / 8 / 9 can land in parallel after PR 3c per the consolidated plan §4 (different concern axes; non-conflicting code regions).

### Phase Dependencies

- **Phase 1 (Setup, PR 0)**: No dependencies; first.
- **Phase 2 (Foundational, PR 0.5/0.6/1)**: Depends on Phase 1 (lints in place); blocks keystone.
- **Phase 3 (US1 — Audit content-ignorance, PR 3a/3b/3c)**: Depends on Phase 2; **MVP**.
- **Phase 4 (US2 — Foreign banner, PR 5+6)**: Depends on Phase 3 (PR 3c); part of MVP.
- **Phase 5 (US3 — Pass-split, PR 7)**: Depends on Phase 3.
- **Phase 6 (US4 — Open-vocab parser, PR 2)**: Depends on Phase 1; ships BEFORE Phase 3 in PR-sequence order despite being P2 priority. (Implementation order ≠ priority order here.)
- **Phase 7 (US5 — Citation maturation, PR 10)**: Depends on Phase 8 (PR 4 land) and Phase 11 (PR 9 land).
- **Phase 8 (US6 — Lattice, PR 3.7+4)**: PR 3.7 depends on Phase 3 (PR 3c). PR 4 depends on PR 3.7.
- **Phase 9 (US7 — Performance, cross-cutting)**: Per-PR; baselines captured at Phase 1.
- **Phase 10 (US8 — Revertability, cross-cutting)**: CI matrix discipline through Phases 3 + 4.
- **Phase 11 (Polish, PR 8/9/final)**: Depends on Phase 3 (PR 3c) for PR 8/9; final tasks after all phases.

### User Story Dependencies (correctness-property layering)

- **US1 (P1)**: Foundation for every other US; the type-system reshape that makes the rest expressible.
- **US2 (P1)**: Independent of US3–US8; depends only on US1's pivot split for `ProjectedMarking`.
- **US3 (P2)**: Depends on US1 (rules emit `FixIntent`) + US4 (parser invariants stable for re-parse).
- **US4 (P2)**: Independent of US1; ships first per PR sequence.
- **US5 (P2)**: Cross-cutting through every PR introducing or moving citations.
- **US6 (P2)**: Depends on US1 (`MarkingScheme::project` surface complete) + US2 (`PageContext` deletion).
- **US7 (P3)**: Cross-cutting; per-PR bench gates.
- **US8 (P3)**: Cross-cutting; per-PR CI matrix discipline.

### Within Each User Story

- Type-system definitions (data-model entities) before consumer call-site migrations.
- Compile-fail tests where applicable (US1's `Canonical` un-constructable test, US4's `MessageArgs` no-`String` test).
- Property tests after the implementations they exercise.
- Corpus regression fixtures before the corpus runs that consume them.

### Parallel Opportunities

- **All T001–T009 [P] tasks** can run in parallel within PR 0 (different files, no inter-task deps).
- **All citation-lint scaffold tasks T010–T013** can be parallelized within PR 0.5 once T010 lands the crate scaffold.
- **PR 3a's test-fixture migration (T024)** is mechanical — sed-replaceable.
- **Within PR 3c**: T030 (extraction) sequences before T031–T032 (use the extraction); T034–T036 (Canonical sealing) parallel with T037–T039 (FixIntent definitions); T041–T043 (audit/diagnostic reshape) parallel with T044 (rule-ID migration); T045–T046 (per-rule migration) parallel after T031–T044 land.
- **Within PR 4**: T111 (Vocabulary extension) sequences before T112–T115; property tests T116–T119 [P] all parallel after impls land.
- **Within PR 7**: T073–T075 (Phase plumbing) sequences first; T076–T082 (engine pass-split + R002 + cache) sequence; T083–T085 [P] property/bench tests parallel after impls.
- **PR 7 / PR 8 / PR 9** can land in parallel after PR 3c per the consolidated plan.
- **Phase 11 polish tasks T136–T141 [P]** all parallel.

---

## Parallel Example: PR 0 Setup

```bash
# Launch all PR 0 [P] tasks in parallel (different files, no inter-task deps):
Task: "Add static_assertions::assert_impl_all!(Rule: Send + Sync) — T002"
Task: "Add static_assertions::assert_impl_all!(dyn Recognizer<CapcoScheme>: Send + Sync) — T003"
Task: "Create tools/masking-pin-lint/ Rust binary crate — T004"
Task: "Implement masking-pin lint scanner — T005"
Task: "Implement masking-pin lint GitHub-API check — T006"
Task: "Create tools/promote-callsite-lint/ Rust binary crate — T007"
Task: "Add lints to CI workflow — T008"

# Sequential dependency: T001 (baseline capture) and T009 (inventory) run independently.
```

## Parallel Example: PR 4 Lattice property tests

```bash
# After T111–T115 land (Vocabulary + per-category Lattice impls + delete PageContext delegation):
Task: "Property tests at category_lattice_laws.rs — T116"
Task: "Cross-axis dominance fixture tests at cross_axis_dominance.rs — T117"
Task: "Create tests/corpus/lattice/ regression fixtures — T118"
Task: "Create tests/corpus/prose-positive/ fixtures — T119"
```

---

## Implementation Strategy

### MVP (Phases 1 + 2 + 3 + 4 = PRs 0 / 0.5 / 0.6 / 1 / 2 / 3a / 3b / 3c / 5 / 6a / 6b / 6c)

The MVP is **US1 + US2 = P1 priorities**. Both must land for the tool to be deployable in IC/DoD contexts:

1. **Phase 1: Setup** — bench baselines, AST-lint scaffolding, Send+Sync.
2. **Phase 2: Foundational** — citation-lint scaffold + preemptive defect fix; verify splice landed.
3. **Phase 6 (US4 first per PR sequence)** — parser shape_admits + FgiMarker discriminant. (P2 priority but PR-2 sequence position; shipping it before Phase 3 is the engineering reality.)
4. **Phase 3: US1** — keystone PR 3a + 3b + 3c. Closes the G13 leak channels.
5. **Phase 4: US2** — PR 5 (foreign banner) + PR 6a/6b/6c (Scope::Page projection cutover). Closes the foreign-banner correctness defect.
6. **STOP and VALIDATE**: SC-001 canary scan green; SC-002 foreign-banner fixtures 100%; deployable to compliance auditor + foreign-document workflows.

### Incremental delivery after MVP

7. **Phase 8: US6** — PR 3.7 lattice §-resolution spike + PR 4 lattice impls. Closes lattice-law correctness.
8. **Phase 5: US3** — PR 7 phase-tagged pass split. Closes pass-1 / pass-2 corruption defect.
9. **Phase 11: PR 8** — decoder priors (third problem class; not G13 closure).
10. **Phase 11: PR 9** — parser separators + dissem split + banner-val migration.
11. **Phase 7: US5** — PR 10 F.1 maturation. Closes citation fidelity at full coverage.
12. **Phase 11: final polish** — final corpus sweep, SC completion report, lints clean.

### Parallel team strategy

Once Phase 3 (PR 3c) merges:
- **Developer A**: Phase 5 (US3 — PR 7).
- **Developer B**: Phase 4 (US2 — PR 5 + PR 6).
- **Developer C**: Phase 8 (US6 — PR 3.7 → PR 4).
- **Developer D**: Phase 11 (PR 8) and (PR 9) — these are non-conflicting code regions per the consolidated plan §4.

PR 10 (Phase 7 maturation) lands last as a single integration step after PR 4, PR 6, PR 9 all merge.

---

## Notes

- **[P] tasks**: different files, no in-PR dependency on incomplete tasks.
- **[Story] label**: maps task to user story for traceability; setup / foundational / polish carry no story label.
- **(PR-N)**: source PR per consolidated plan §4; the dependency spine.
- **Line numbers** in tasks (e.g., `parser.rs:1011-1024`, `engine.rs:1369-1384`) are *indicative* per spec Assumptions — re-grep at edit time. Defect classes are stable; line numbers are not.
- **Test-fixture migration** at PR 3a is mechanical (sed-replaceable); fixtures re-touch at PR 3c when the adapter deletes.
- **Citation hygiene**: every PR introducing or moving a citation runs `cargo run -p citation-lint -- .` before merge.
- **Bench discipline**: every PR running a bench asserts against `benches/baselines/2026-05-pre-refactor.json`; >5% mean OR p99 regression backs out the originating change (FR-033).
- **Constitution check per PR**: each PR carries a Constitution Check in the PR description per consolidated plan Appendix D; pass for all 8 principles per `plan.md` § Constitution Check.
- **Keystone CI matrix** runs corpus regression × {3a-only, 3a+3b, 3a+3b+3c} = 3 runs during the PR 3a / 3b / 3c window; PR 6 runs corpus regression × {6a-only, 6a+6b, 6a+6b+6c} = 3 runs during the PR 6 window. Both gate SC-014.
- **Avoid**: cross-PR same-file conflicts during the keystone window (use the sub-PR splits as serialization points).
