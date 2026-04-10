# Tasks: Marque MVP — CAPCO Marking Linter and Fixer

**Input**: Design documents from `/specs/001-marque-mvp/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Included. The spec's success criteria (SC-002/SC-003 ≥95% corpus accuracy, SC-004 audit completeness, SC-008 native/WASM parity) are only verifiable through fixture-driven tests, and the plan explicitly commits to `cargo test` + `insta` snapshots + `criterion` benchmarks as Phase 1 contracts.

**Organization**: Tasks are grouped by user story so each story can be implemented and validated independently.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: User story label (US1, US2, US3, US4)
- File paths are absolute from repo root

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Workspace-level scaffolding shared across every crate touched by the MVP slice.

- [ ] T000 [P] Create `crates/marque-ism/` crate with Cargo.toml (deps: `thiserror`, `memchr`, `phf`, `aho-corasick`; build-deps: `quick-xml`, `anyhow`; `[package.metadata.marque] ism-schema-version = "ISM-v2022-DEC"`). Move `schemas/ISM-v2022-DEC/` from `crates/marque-capco/`, move `src/span.rs` and `src/attrs.rs` from `crates/marque-core/`, move `src/token_set.rs` and `build.rs` from `crates/marque-capco/`, create `src/generated.rs` with `include!()` wrappers. Update all imports across workspace (`marque_core::Span` → `marque_ism::Span`, `marque_core::IsmAttributes` → `marque_ism::IsmAttributes`, etc.). Add `marque-ism` to workspace members in root `Cargo.toml`. Wire `marque-core`, `marque-rules`, and `marque-capco` Cargo.toml deps to depend on `marque-ism`. Verify `cargo check --workspace` passes after the move.
- [ ] T001 Verify the nine MVP crate directories exist and compile under `cargo check --workspace` (`crates/marque-ism`, `crates/marque-core`, `crates/marque-rules`, `crates/marque-capco`, `crates/marque-engine`, `crates/marque-config`, `crates/marque-wasm`, `crates/marque`); record any missing `Cargo.toml` and add a stub if absent.
- [ ] T002 [P] Create `tests/corpus/{valid,invalid}/` directory tree at repo root with a placeholder `README.md` describing the fixture format (filename, expected rule IDs, expected spans) per plan §"Source Code".
- [ ] T002b [P] Author `tests/corpus/CORPUS_PROVENANCE.md` codifying the SC-002a provenance contract: every fixture is synthetic, wrapped in Lorem Ipsum or manifestly fictional prose, and uses only public CAPCO marking syntax from ODNI documentation. The doc names the reviewer who spot-checked the corpus before the `mvp-corpus-v1` tag and records the review date. Blocks the tag; blocks Phase 7.
- [ ] T002a [P] Author `tests/corpus/CORPUS_CONTRACT.md` codifying SC-002a: the corpus MUST contain ≥3 known-bad fixtures per rule (E001–E008, W001, C001), ≥20 known-good fixtures, and a ≥1000-line clean-prose corpus for SC-003a. Each known-bad fixture has a sibling `.expected.json` pinning rule IDs + byte spans. The corpus is tagged `mvp-corpus-v1` before Phase 7 begins; SC-002/SC-003/SC-008 are measured against exactly that tag. This task produces the contract doc and the empty directory scaffolding referenced by US1 (T025/T026) and SC-003a (new T026a).
- [ ] T003 [P] Create `benches/` directory at repo root with empty `lint_latency.rs` and `linear_scaling.rs` skeletons referenced by `Cargo.toml [[bench]]` entries.
- [ ] T004 [P] Add `insta`, `criterion`, and a workspace `dev-dependencies` corpus loader stub at `crates/marque-test-corpus/` (or as a `dev-dependency` path) so every MVP crate can load fixtures uniformly.
- [ ] T005 [P] Confirm `clippy --workspace -- -D warnings` and `cargo fmt --check` run clean on the empty skeleton; wire both into a single `scripts/check.sh`.
- [ ] T005a [P] Adopt `cargo-nextest` as the canonical test runner: add a `.config/nextest.toml` with the workspace's default profile, document `cargo nextest run --workspace` in `scripts/check.sh` and `quickstart.md`, and use `nextest` (not `cargo test`) in CI. All test tasks below assume `nextest` for execution.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The shared types, generated code, and pipeline plumbing every user story depends on. ⚠️ No US1–US4 task may begin until this phase is complete.

### Generated Layer 1 (marque-ism build script)

- [ ] T006 Implement `crates/marque-ism/build.rs` CVE XML parser using `quick-xml` to read every `crates/marque-ism/schemas/ISM-v2022-DEC/CVE_ISM/CVEnumISM*.xml` and emit closed Rust enums into `OUT_DIR/values.rs` (Classification, SciControl, SarIdentifier, DissemControl, DeclassExemption) per research item R-1, consumed via `crates/marque-ism/src/generated.rs`.
- [ ] T007 Extend `crates/marque-ism/build.rs` to parse `crates/marque-ism/schemas/ISM-v2022-DEC/CVE_ISMCAT/CVEGenerated/CVEnumISMCATRelTo.xsd` and emit the `Trigraph` enum into `OUT_DIR/values.rs`.
- [ ] T008 Extend `crates/marque-ism/build.rs` to parse `crates/marque-ism/schemas/ISM-v2022-DEC/Schematron/ISM_XML.sch` and `Schematron/Lib/*.sch` and emit binary predicate functions into `OUT_DIR/validators.rs` per research item R-2. Scope: fixed XPath vocabulary only (attribute presence, equality, set membership, cardinality); ~70% of assertions covered; remainder skipped with build-time warning. Layer 2 rules in `marque-capco` import these predicates from `marque-ism` as their authoritative correctness foundation.
- [ ] T009 Extend `crates/marque-ism/build.rs` to emit the deterministic deprecated-marking migration table (including X-shorthand `25X1-`-style date markings) into `OUT_DIR/migrations.rs` with `confidence ≥ 0.95` per FR-004a and research item R-3.
- [ ] T010 Pin and verify the active schema version: assert in `build.rs` that `[package.metadata.marque] ism-schema-version` in `crates/marque-ism/Cargo.toml` equals `ISM-v2022-DEC` and emit a `SCHEMA_VERSION` `&'static str` into `OUT_DIR/values.rs` (FR-011).

### Core types (marque-ism + marque-rules)

- [ ] T011 [P] Define `Span`, `MarkingCandidate`, `CandidateKind`, `MarkingType`, `Zone`, `DocumentPosition`, and `RuleContext` in `crates/marque-ism/src/span.rs` per `data-model.md` §Span / §RuleContext (all `Copy`, no allocation).
- [ ] T012 [P] Define `IsmAttributes` in `crates/marque-ism/src/attrs.rs` using the generated enum types from `crates/marque-ism/src/generated.rs` (SciControl, SarIdentifier, DissemControl, DeclassExemption, Trigraph). Fields use `Box<[T]>` per constitution Principle II. Mark `#[non_exhaustive]`. This is straightforward because both `IsmAttributes` and its field types live in the same crate — no circular dependency.
- [ ] T013 Define `Severity`, `RuleId`, `Diagnostic`, `FixSource`, `FixProposal`, and `AppliedFix` in `crates/marque-rules/src/lib.rs` (include the `FixSource` enum `{BuiltinRule, CorrectionsMap, MigrationTable}`; `FixProposal` carries a `source: FixSource` field; `AppliedFix` carries `input: Option<Box<str>>` populated by the CLI at the boundary) per `data-model.md` §Severity..§AppliedFix, including the trait surface: `pub trait Rule { fn id(&self) -> RuleId; fn check(&self, attrs: &IsmAttributes, ctx: &RuleContext) -> Vec<Diagnostic>; }`. Types `IsmAttributes`, `Span`, `RuleContext` are imported from `marque-ism` (not `marque-core`). `Diagnostic.fix` is typed `Option<FixProposal>`; `AppliedFix` wraps a `FixProposal` plus timestamp, classifier_id, and `dry_run: bool` and is constructible only by `marque-engine` (document this — no public constructor from `marque-rules`).
- [ ] T014 Add `FixProposal` invariant guards in `crates/marque-rules/src/lib.rs` (`debug_assert!(0.0 <= confidence && confidence <= 1.0 && !confidence.is_nan())`) and document that every `AppliedFix` constructed by the engine — including `confidence == 1.0` fixes — carries a complete audit payload (FR-005, constitution Principle V). Suggestions never produce an `AppliedFix`.
- [ ] T014a Define a `Clock` trait in `crates/marque-engine/src/clock.rs` with `fn now(&self) -> SystemTime`, a `SystemClock` default implementation, and a `FixedClock(SystemTime)` test implementation. `Engine` holds `Arc<dyn Clock>` (or a generic `<C: Clock>`) and uses it exclusively to stamp `AppliedFix::timestamp` — no direct `SystemTime::now()` calls anywhere in the engine or rule crates. Required by `data-model.md` §AppliedFix "Clock seam" and by the deterministic snapshot tests in T046.

### Scanner + Parser (marque-core pipeline phases 1–2)

- [ ] T015 Implement the `memchr`-based candidate scanner in `crates/marque-core/src/scanner/mod.rs`, emitting an iterator of `MarkingCandidate` over `&[u8]` with zero heap allocation per candidate (constitution Principle II); cover portion `(...)`, banner full-line, and CAB block detection.
- [ ] T016 Implement the `aho-corasick` (native) Phase-2 token extractor in `crates/marque-core/src/parser/mod.rs`, consuming `MarkingCandidate` and producing `IsmAttributes`. Use a `cfg(target_arch = "wasm32")` switch to `daachorse` per research item R-7.
- [ ] T017 Wire scanner + parser smoke tests in `crates/marque-core/tests/scanner_smoke.rs` and `crates/marque-core/tests/parser_smoke.rs` against fixtures from `tests/corpus/`.

### Engine pipeline (marque-engine)

- [ ] T018 Implement `Engine::lint(text: &str, cfg: &Configuration) -> Vec<Diagnostic>` in `crates/marque-engine/src/engine.rs`: scan → parse → run every registered `Rule`, applying `cfg.rule_severities` overrides.
- [ ] T019 Implement deterministic total-order fix application in `crates/marque-engine/src/overlap.rs` per FR-016: sort fixes by `(span.end DESC, span.start DESC, rule_id ASC, replacement ASC)` so reverse-byte application preserves earlier-span offsets AND equal-span ties break deterministically; include a property test that two runs on the same input produce byte-identical post-fix text and byte-identical audit NDJSON.
- [ ] T020 Implement `Engine::fix(text, cfg, FixMode::{Apply,DryRun}) -> FixResult` in `crates/marque-engine/src/engine.rs`, producing the post-fix text plus the audit-record stream; gate auto-application on `confidence >= cfg.confidence_threshold` (FR-004).
- [ ] T021 Implement audit emission in `crates/marque-engine/src/audit.rs`: every `FixProposal` that clears the confidence threshold is promoted to an `AppliedFix` (using the injected `Clock` from T014a and `Configuration::classifier_id`) and emitted on the audit stream; sub-threshold proposals are carried only in the diagnostic stream and NEVER produce an `AppliedFix`. In `--dry-run` mode the text is unchanged but the audit stream is byte-identical to a real run except for the `dry_run: true` field on every record (FR-006, SC-004).
- [ ] T021a Enforce FR-013 (no content retention beyond the processing pass) in `crates/marque-engine/tests/no_retention.rs`: assert `Engine` holds no `&str`/`Vec<u8>` document references after `lint`/`fix` returns (lifetime-bound API + Miri/drop-check test); documents the API contract that callers own buffers.

### Configuration (marque-config)

- [ ] T022 Implement layered loader in `crates/marque-config/src/lib.rs`: `.marque.toml` → `.marque.local.toml` → env vars (`MARQUE_CLASSIFIER_ID`, `MARQUE_CONFIDENCE_THRESHOLD`, `MARQUE_LOG`) → CLI flag overrides, returning `Configuration` per `data-model.md` §Configuration (FR-007).
- [ ] T023 Add hard-fail validators in `crates/marque-config/src/lib.rs`: refuse to load any `.marque.toml` containing a `[user]` section (FR-010, SC-006); refuse a `[capco] version` mismatch with `marque_ism::SCHEMA_VERSION` (FR-011); refuse `confidence_threshold` outside `[0.0, 1.0]`. All three exit `65 EX_DATAERR` per `contracts/cli.md`.

### Rule registration scaffolding

- [ ] T024 Define `CapcoRuleSet::new() -> Vec<Box<dyn Rule>>` in `crates/marque-capco/src/lib.rs` as an empty registration point that US1 will populate; expose `pub fn default_ruleset() -> Vec<Box<dyn Rule>>` from `marque-engine` so the CLI and WASM share one entry point.

**Checkpoint**: Foundation ready — US1 may now begin. US2/US3/US4 may begin in parallel after their own US1-implied dependencies (rule set, audit, config) land in their respective phases.

---

## Phase 3: User Story 1 — Lint a document for marking errors (Priority: P1) 🎯 MVP

**Goal**: A reviewer runs `marque check <file>` (or `cat file | marque check -`) and receives precise diagnostics — rule ID, span, message, CAPCO citation, optional fix suggestion — for every marking violation in the document.

**Independent Test**: Feed `tests/corpus/invalid/banner_abbrev.txt` (a banner using `S//NF`) to `marque check`; verify the output contains rule `E001`, the exact byte span of `S//NF`, the human message, the citation `CAPCO-ISM-v2022-DEC-§3.x`, and a suggested expansion. Run against `tests/corpus/valid/clean.txt` and verify zero diagnostics + exit `0`.

### Tests for User Story 1

- [ ] T025 [P] [US1] Populate `tests/corpus/invalid/` to satisfy the SC-002a corpus contract (T002a): at least 3 known-bad fixtures per rule for E001–E008, W001, and C001, including the canonical named fixtures (`banner_abbrev.txt`, `missing_usa_trigraph.txt`, `misordered_blocks.txt`, `separator_count.txt`, `unknown_token.txt`, `mixed_confidence.txt`). Each fixture has a sibling `.expected.json` listing expected rule IDs and exact byte spans. Total count ≥40 (plan §Scale/Scope) but composition — not headline count — is the gate.
- [ ] T026 [P] [US1] Add ≥20 known-good fixtures under `tests/corpus/valid/` with empty `.expected.json` files (zero diagnostics).
- [ ] T026a [P] [US1] Add the SC-003a clean-prose corpus under `tests/corpus/prose/`: ≥1000 lines of body prose containing no markings, including at least 20 incidental parenthesized single-letter tokens (`(S)`, `(a)`, etc.) in mid-sentence positions to exercise the disambiguation heuristic. Wired into the corpus accuracy harness (T069) as a zero-diagnostic precision gate.
- [ ] T027 [P] [US1] Write `crates/marque-capco/tests/rules_us1.rs` driving every fixture from T025/T026 through `Engine::lint` and asserting rule ID + span equality.
- [ ] T028 [P] [US1] Write `crates/marque-engine/tests/lint_pipeline.rs` covering the FR-001/FR-002/FR-003 happy path and the spec edge cases: empty document, whitespace-only, mid-sentence `(S)` body prose disambiguation, unknown token reported (FR-012).
- [ ] T029 [P] [US1] Add `insta` snapshot tests under `crates/marque-engine/tests/lint_pipeline.rs` for the human-readable and JSON diagnostic formats against `contracts/diagnostic.json`.

### Implementation for User Story 1

- [ ] T030 [P] [US1] Implement rule `E001` (banner uses portion abbreviation, e.g. `S//NF`) in `crates/marque-capco/src/rules.rs`; suggest expanded form with `confidence = 1.0`; cite the relevant CAPCO section.
- [ ] T031 [P] [US1] Implement rule `E002` (REL TO list missing `USA` trigraph) in `crates/marque-capco/src/rules.rs`; suggest insertion at the correct ordinal position with `confidence = 0.97`.
- [ ] T032 [P] [US1] Implement rule `E003` (misordered banner blocks: classification → SCI → SAR → dissem) in `crates/marque-capco/src/rules.rs`; suggest reordered banner with `confidence = 0.6` (kept as suggestion under default threshold).
- [ ] T033 [P] [US1] Implement rule `E004` (separator-count normalization, e.g. extra/missing `//`) in `crates/marque-capco/src/rules.rs` with `confidence = 0.99`.
- [ ] T034 [P] [US1] Implement rule `E005` (declassification info appearing inside a banner instead of the CAB) in `crates/marque-capco/src/rules.rs` with `confidence = 0.55` (suggestion only).
- [ ] T035 [P] [US1] Implement rule `E006` (deprecated dissem control per generated migration table) in `crates/marque-capco/src/rules.rs` with `confidence = 0.97` and a `migration_ref` to the generated entry.
- [ ] T036 [P] [US1] Implement rule `E007` (X-shorthand declassification date marking, e.g. `25X1-`) in `crates/marque-capco/src/rules.rs` with `confidence = 0.97` per FR-004a / research R-3.
- [ ] T037 [P] [US1] Implement rule `E008` (unrecognized token inside marking candidate boundary) in `crates/marque-capco/src/rules.rs`, severity error, no fix offered (FR-012).
- [ ] T038 [P] [US1] Implement rule `W001` (deprecated marking still valid in current schema) in `crates/marque-capco/src/rules.rs`, severity warn, with `confidence = 0.97` migration suggestion.
- [ ] T039 [US1] Register `E001..E008` and `W001` in `crates/marque-capco/src/lib.rs::CapcoRuleSet::new()` and update `marque-engine::default_ruleset()` to wire it in.
- [ ] T040 [US1] Implement `marque check` subcommand in `crates/marque/src/main.rs` using `clap`: accept `[PATH...]` plus `-` stdin sentinel (FR-014a), `--config`, `--confidence-threshold`, `--format human|json`, `--no-color`, `-q`, `-v`; honor exit codes from `contracts/cli.md` (`0` clean, `1` error diags, `2` warn-only, `64`/`65`/`74` failure modes).
- [ ] T041 [US1] Implement the `human` and `json` (NDJSON) diagnostic renderers in `crates/marque/src/render.rs`, conforming to `contracts/diagnostic.json` and citing the CAPCO section per FR-003.
- [ ] T042 [US1] Add CLI smoke test `crates/marque/tests/cli_smoke.rs` that runs `marque check` against ≥3 corpus fixtures and asserts stdout, stderr, and exit code.
- [ ] T042a [US1] Implement `--explain-config` in `crates/marque/src/main.rs` per `contracts/cli.md`: before touching any input, serialize the merged `Configuration` (rule severities, corrections-map keys, confidence threshold, schema version, and a boolean `classifier_id_present` — never the value) as JSON to stdout and exit `0`. Enforce mutual exclusion with input paths and with `fix` (`64 EX_USAGE` otherwise). Add a CLI smoke test asserting the JSON shape and that `classifier_id` itself never appears in the output even when set.

**Checkpoint**: US1 (lint) is fully functional, byte-precise, and corpus-validated. The MVP is demoable at this point against raw text input.

---

## Phase 4: User Story 2 — Auto-fix high-confidence violations with audit trail (Priority: P2)

**Goal**: `marque fix [--dry-run] <file>` applies every fix at or above the configured confidence threshold (default `0.95`), leaves lower-confidence fixes as suggestions, and emits a complete audit record (rule, original, replacement, confidence, timestamp, optional classifier id) for every fix — applied or suggested — to stderr as NDJSON.

**Independent Test**: Run `marque fix tests/corpus/invalid/mixed_confidence.txt` where one violation has `confidence = 1.0` and another has `confidence = 0.6`; verify (a) only the high-confidence fix mutated the file, (b) stderr NDJSON contains both records, (c) `--dry-run` produces identical NDJSON without modifying the file, (d) the modified file passes a subsequent `marque check` cleanly.

### Tests for User Story 2

- [ ] T043 [P] [US2] Add `tests/corpus/invalid/mixed_confidence.txt` plus `.expected_fix.json` listing the expected post-fix text, applied audit records, and suggested-only audit records.
- [ ] T044 [P] [US2] Write `crates/marque-engine/tests/fix_pipeline.rs` covering acceptance scenarios from spec US2: 1.0 typo + 0.6 structural, `--dry-run` parity, missing classifier identity, and the spec edge case "fix would overlap another fix" (deterministic reverse-order application, FR-016).
- [ ] T045 [P] [US2] Write `crates/marque-engine/tests/audit_completeness.rs` enforcing SC-004: every `AppliedFix` emitted in any run has a complete serialization (no missing fields, no orphans) validated programmatically against `contracts/audit-record.json`, and asserts the inverse — no sub-threshold `FixProposal` ever appears in the audit stream.
- [ ] T046 [P] [US2] Add `insta` snapshot tests in `crates/marque-engine/tests/fix_pipeline.rs` for the NDJSON audit stream shape, using `FixedClock` (T014a) so snapshots are deterministic across runs and machines.

### Implementation for User Story 2

- [ ] T047 [US2] Implement `marque fix` subcommand in `crates/marque/src/main.rs`: flags `--dry-run`, `--in-place` (default for paths), `--write-stdout` (default for stdin), mutual-exclusion enforcement returning `64 EX_USAGE`.
- [ ] T048 [US2] Implement atomic temp-file rename for `--in-place` writes in `crates/marque/src/main.rs` so a crash mid-write never leaves a partially-written file (per `contracts/cli.md` §Input handling).
- [ ] T049 [US2] Implement the audit NDJSON writer in `crates/marque/src/render.rs` emitting one `AuditRecord` per line on stderr, conforming to `contracts/audit-record.json` (schema version `marque-mvp-1`); ensure `-q` does NOT suppress audit lines. Per FR-005a: each record is serialized to an in-memory buffer and flushed with a single `write_all` ending in `\n` (no partial records); every record carries `"schema": "marque-mvp-1"`; on serialization failure, emit a single `{"schema":"marque-mvp-1","error":"<code>","rule":"<rule-id>"}` error frame and return a nonzero exit code. Add a test that injects a forced serialization failure and asserts the error frame shape.
- [ ] T050 [US2] Wire `Engine::fix` (T020) and audit emission (T021) into the CLI `fix` path; on completion, run `Engine::lint` against the post-fix text and use the result to set the exit code (`0` clean re-lint, `1` errors remain, `2` warnings remain).
- [ ] T051 [US2] Add CLI integration tests in `crates/marque/tests/cli_smoke.rs` for `fix`, `fix --dry-run`, and the `--in-place` vs `--write-stdout` paths against the US2 corpus fixtures.
- [ ] T051a [US2] Expose the `Clock` seam (T014a) through the CLI as `--fixed-timestamp <RFC3339>`, gated on the `MARQUE_ALLOW_FIXED_CLOCK=1` environment variable per `contracts/cli.md`. Without the env var, the flag exits `64 EX_USAGE` with a message noting that the fixed-clock seam is off by default to prevent accidental audit-log falsification. With the env var, the CLI constructs a `FixedClock` from the parsed timestamp and hands it to `Engine`, making audit-record NDJSON fully reproducible for CI golden-file tests. Add an integration test asserting (a) the env-var gate rejects the flag by default, (b) with the env var set, two successive `fix` runs against the same input produce byte-identical audit NDJSON.

**Checkpoint**: US1 and US2 both work independently. The tool is now a usable lint+fix CLI with a complete audit trail.

---

## Phase 5: User Story 3 — Configure rule severity and corrections per project (Priority: P2)

**Goal**: A program office commits a `.marque.toml` overriding rule severities and adding a corrections map; individual users keep classifier identity in `.marque.local.toml` (gitignored) or env vars; CLI flags override both. The classifier identity NEVER appears in any committed file.

**Independent Test**: Place a `.marque.toml` downgrading `E001` to `warn` and adding `[corrections] SERCET = "SECRET"`; place a `.marque.local.toml` with `classifier_id = "12345"`; run `marque check` on a fixture violating `E001` and `marque fix` on a fixture containing `SERCET`; verify (a) `E001` is reported as `warn`, (b) `SERCET` is corrected with the corrections-map source cited in the audit record, (c) the audit records carry `classifier_id = "12345"`, (d) no committed test fixture contains a classifier id (SC-006 automated check passes).

### Tests for User Story 3

- [ ] T052 [P] [US3] Write `crates/marque-config/tests/precedence.rs` covering the four-layer precedence chain (committed → local → env → CLI) per FR-007 with explicit assertions for which value wins at each layer.
- [ ] T053 [P] [US3] Add `crates/marque-config/tests/precedence.rs` cases for the three hard-fail scenarios from `contracts/cli.md` §"Hard-fail at config load": `[user]` in `.marque.toml`, schema version mismatch, threshold out of range. Each must return the documented error and exit code.
- [ ] T054 [P] [US3] Write `crates/marque-capco/tests/corrections_map.rs` exercising FR-009: when both a built-in rule and a user correction match the same span, the user correction wins; the audit record cites `corrections-map` as the source.
- [ ] T055 [P] [US3] Add a repo-wide automated SC-006 check as `tests/no_classifier_id_in_commits.rs` that scans every file under `tests/corpus/`, `crates/*/tests/`, and `crates/*/examples/` for classifier-id-shaped strings and fails if any are found.
- [ ] T055a [P] [US3] Add the SC-002a corpus provenance scan as `tests/corpus_provenance.rs`: (a) every file under `tests/corpus/` matches a registered path pattern (`invalid/*.txt`, `invalid/*.expected.json`, `valid/*.txt`, `valid/*.expected.json`, `prose/*.txt`, `CORPUS_CONTRACT.md`, `CORPUS_PROVENANCE.md`, `README.md`); (b) `CORPUS_PROVENANCE.md` exists and contains a reviewer line; (c) no fixture contains any classifier-id-shaped string (reuses T055 scanner); (d) no fixture contains token strings outside the generated CVE enumerations in `marque_ism::generated::values`. Blocks merge on failure. Runs in CI on every PR.

### Implementation for User Story 3

- [ ] T056 [US3] Extend `crates/marque-config/src/lib.rs` to surface rule severity overrides into `Configuration::rule_severities` and apply them in `Engine::lint` (T018) so changing severity in `.marque.toml` is reflected on the next run with no other edits (SC-007, FR-008).
- [ ] T057 [US3] Implement the `[corrections]` parser in `crates/marque-config/src/lib.rs` that builds a `phf` map at runtime (or `rapidhash` table per plan) and exposes it on `Configuration::corrections`.
- [ ] T058 [US3] Implement rule `C001` (corrections-map typo replacement) in `crates/marque-capco/src/rules.rs` consuming `Configuration::corrections`; emit a `FixProposal` with `confidence = 1.0` and `migration_ref = Some("corrections-map")` so the resulting `AppliedFix` carries the same ref through to the audit stream; user correction takes precedence over any built-in match on the same span (FR-009, spec edge case).
- [ ] T058a [US3] Register `C001` in `crates/marque-capco/src/lib.rs::CapcoRuleSet::new()` alongside E001–E008/W001 (extends T039) so the corrections-map rule is wired into `default_ruleset()` for both CLI and WASM front ends.
- [ ] T059 [US3] Add the `MARQUE_CLASSIFIER_ID`, `MARQUE_CONFIDENCE_THRESHOLD`, and `MARQUE_LOG` env-var bindings in `crates/marque-config/src/lib.rs` with documented precedence over file layers.
- [ ] T060 [US3] Wire `Configuration::classifier_id` through the engine into every `AuditRecord` constructed by `marque-engine::audit` (T021); when absent, the field is `None` and the NDJSON renderer (T049) emits explicit `null` rather than omitting the key.

**Checkpoint**: US1, US2, and US3 all work independently. The tool now matches real organizational policy without leaking PII into version control.

---

## Phase 6: User Story 4 — Lint and fix raw text from a web context (Priority: P3)

**Goal**: A WASM build of `marque-wasm` exposes `lint(text: &str) -> JsValue` and `fix(text: &str, threshold: f32) -> JsValue` to a web worker. Diagnostics are byte-identical to the CLI for the same input. No file system, no network.

**Independent Test**: Build `crates/marque-wasm` with `wasm-pack build --target web --profile release-wasm`; load it from a minimal `quickstart.md` HTML harness; pass the same string used by a CLI `marque check` invocation and assert the JSON diagnostics match byte-for-byte, including spans (SC-008).

### Tests for User Story 4

- [ ] T061 [P] [US4] Add a Rust-side parity test `crates/marque-wasm/tests/native_parity.rs` (gated to `cfg(not(target_arch = "wasm32"))`) that drives the same input through the native `Engine` and the WASM-export wrappers and asserts byte-equal diagnostic JSON for ≥10 corpus fixtures.
- [ ] T062 [P] [US4] Add `crates/marque-wasm/tests/no_io.rs` asserting (via `cargo deny` / dependency audit) that the WASM build's dependency tree contains no `std::fs`, no `tokio::net`, no `reqwest`-class crates — enforces FR-013 and the US4 acceptance scenario "no file system or network access is attempted".

### Implementation for User Story 4

- [ ] T063 [US4] Implement `wasm-bindgen` exports `lint(text: &str) -> JsValue` and `fix(text: &str, threshold: f32) -> JsValue` in `crates/marque-wasm/src/lib.rs`, wrapping `marque-engine::Engine` and serializing to the same NDJSON shape as the CLI per `contracts/diagnostic.json`.
- [ ] T064 [US4] Switch `marque-core/src/parser/mod.rs` (T016) to `daachorse` under `cfg(target_arch = "wasm32")` per research item R-7; verify binary size is acceptable.
- [ ] T065 [US4] Add a minimal HTML harness at `crates/marque-wasm/examples/harness.html` driving the WASM build with one fixture and printing the diagnostic JSON to the page, used by `quickstart.md` step 4.
- [ ] T066 [US4] Verify `wasm-pack build crates/marque-wasm --target web --profile release-wasm` succeeds in CI and produces an artifact ≤1MB.
- [ ] T066a [P] [US4] Measure and record WASM interactive latency per SC-001b: add `benches/wasm_latency.md` documenting the measurement method (harness page, browser/version, input sizes, p50/p95/p99 observed on the reference machine). Advisory, not CI-gating for the MVP — hard gate arrives in the browser-extension slice.

**Checkpoint**: All four user stories independently functional. The MVP can drive a CLI workflow AND a web-worker workflow from a single shared engine.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Performance verification, documentation, and the success-criteria gates that span all four stories.

- [ ] T067 [P] Implement `benches/lint_latency.rs` `criterion` benchmark measuring `Engine::lint` p95 latency on 10KB representative inputs and asserting ≤16ms p95 on commodity hardware (SC-001, constitution Principle I).
- [ ] T067a [P] Commit `benches/baseline.json` capturing p50/p95/p99 for `lint_latency` on the reference machine documented in `plan.md` §Performance Goals. Add a `scripts/bench-check.sh` that runs `lint_latency`, parses the criterion output, and fails with a non-zero exit code if p95 regresses by >10% versus `benches/baseline.json`. Wire the script into CI as a required check on every pull request (SC-001a). Document that baseline re-capture is a manual, reviewed commit — never an auto-update-on-green job.
- [ ] T068 [P] Implement `benches/linear_scaling.rs` `criterion` benchmark sweeping input size across at least one order of magnitude (1KB → 100KB) and asserting throughput stays linear with no super-linear growth (SC-005).
- [ ] T069 [P] Add a corpus-accuracy harness `tests/corpus_accuracy.rs` that runs `Engine::lint` over every fixture in `tests/corpus/invalid/` and asserts ≥95% match against `.expected.json` **both per-rule and overall** (SC-002/SC-002a), and `Engine::fix` produces zero remaining violations on ≥95% of those same fixtures per-rule and overall (SC-003). Additionally, run `Engine::lint` over every line in `tests/corpus/prose/` (T026a) and assert zero diagnostics (SC-003a precision gate). Tag the corpus `mvp-corpus-v1` as a prerequisite for running this harness in CI.
- [ ] T070 [P] Extend T061 (`crates/marque-wasm/tests/native_parity.rs`) to iterate over **every** fixture under `tests/corpus/` (not just ≥10) and assert native vs WASM diagnostic equality for the full corpus (SC-008). T061 establishes the harness; T070 scales it to full coverage in Phase 7.
- [ ] T071 [P] Update `CLAUDE.md` "Current Status" section to reflect MVP completion criteria and run `.specify/scripts/bash/update-agent-context.sh claude` per plan §"Agent context update".
- [ ] T072 [P] Validate `quickstart.md` end-to-end on a clean checkout: lint, dry-run fix, in-place fix, re-lint clean, WASM harness diagnostic match.
- [ ] T072a [P] Add a `cargo-fuzz` target at `crates/marque-engine/fuzz/fuzz_targets/lint.rs` driving `Engine::lint` on arbitrary `&[u8]` input (bounded ≤64KB per iteration). The fuzzer asserts: (a) `lint` never panics, (b) every emitted `Span` is within the input bounds and satisfies `start <= end`, (c) `fix`-then-`lint` is idempotent on fixed output. Document how to run locally (`cargo +nightly fuzz run lint`) in `quickstart.md`. Not CI-gated in the MVP; runs on a nightly cron once infra lands.
- [ ] T073 Run `cargo clippy --workspace -- -D warnings` and `cargo fmt --check` and resolve any findings introduced during MVP work.
- [ ] T074 Run the SC-006 audit (T055) one final time on the full repository and confirm zero classifier-id leaks across every committed file.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: T000 (create `marque-ism` crate) has no dependencies and blocks everything else. Remaining Phase 1 tasks have no dependencies beyond T000.
- **Phase 2 (Foundational)**: Depends on Phase 1. **Blocks every user story phase.**
- **Phase 3 (US1)**: Depends on Phase 2. Independent of US2/US3/US4.
- **Phase 4 (US2)**: Depends on Phase 2 + the rule registration scaffolding (T024) + the engine fix path (T020/T021). Independent of US3/US4 once those land.
- **Phase 5 (US3)**: Depends on Phase 2 + `Engine::fix` (T020) for the corrections-map flow. Independent of US4.
- **Phase 6 (US4)**: Depends on Phase 2 + `Engine::lint`/`Engine::fix` (T018/T020). Can proceed in parallel with US2/US3.
- **Phase 7 (Polish)**: Depends on every targeted user story being complete (US1 minimum for SC-001/SC-002/SC-003; US4 for SC-008).

### Within Phase 2 (Foundational)

- T000 (create `marque-ism`) blocks all Phase 2 tasks (types and codegen now live there).
- T006 → T007 → T008 → T009 → T010 share `marque-ism/build.rs` (sequential, same file).
- T011, T012 are `[P]` (both in `marque-ism`, different files). T013 is in `marque-rules`.
- T015 depends on T011; T016 depends on T011 + T012; T017 depends on T015 + T016 + corpus stub (T002).
- T014a (`Clock` trait) depends on T013 and lands in `marque-engine`; T021 depends on T014a (uses the injected clock for every `AppliedFix::timestamp`).
- T018 depends on T013 + T016; T019, T020, T021 share `marque-engine` and are sequential after T018.
- T022, T023 share `marque-config` and are sequential.
- T024 depends on T013.

### Within Each User Story

- Tests (T025–T029, T043–T046, T052–T055, T061–T062) come before implementation per plan-level convention but the `[P]` test fixtures (T025, T026, T043) can land first to unblock implementation tasks.
- Inside US1, T030–T038 are all `[P]` (one rule per file region; coordinate via task-level merges).
- T039 must follow T030–T038 (all rules registered together).
- T040 depends on T039 + T018 + T022.
- T041, T042 depend on T040.

### Parallel Opportunities

- All `[P]` tasks within a phase run in parallel by definition.
- The four user story phases can all run in parallel after Phase 2 completes (different developers).
- All ten rule implementations T030–T038 are parallel within US1.
- The two `criterion` benchmarks (T067, T068) and the parity/accuracy harnesses (T069, T070) all parallel in Phase 7.

---

## Parallel Example: User Story 1

```bash
# Land the corpus and tests first (all parallel):
Task: "T025 [P] [US1] Add 40 known-bad fixtures under tests/corpus/invalid/"
Task: "T026 [P] [US1] Add 20 known-good fixtures under tests/corpus/valid/"
Task: "T027 [P] [US1] Write rules_us1.rs"
Task: "T028 [P] [US1] Write lint_pipeline.rs covering edge cases"
Task: "T029 [P] [US1] Add insta snapshots for diagnostic formats"

# Then land all nine rule implementations in parallel:
Task: "T030 [P] [US1] Implement E001 banner-abbreviation rule"
Task: "T031 [P] [US1] Implement E002 missing-USA-trigraph rule"
Task: "T032 [P] [US1] Implement E003 misordered-blocks rule"
Task: "T033 [P] [US1] Implement E004 separator-count rule"
Task: "T034 [P] [US1] Implement E005 declass-in-banner rule"
Task: "T035 [P] [US1] Implement E006 deprecated-dissem rule"
Task: "T036 [P] [US1] Implement E007 X-shorthand date rule"
Task: "T037 [P] [US1] Implement E008 unrecognized-token rule"
Task: "T038 [P] [US1] Implement W001 deprecated-warn rule"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1 (Setup) → workspace healthy.
2. Phase 2 (Foundational) → CAPCO build script real, types defined, scanner+parser+engine running, config loader hard-failing on identity leaks.
3. Phase 3 (US1) → ten rules + `marque check` CLI + corpus tests passing.
4. **Stop and validate**: run quickstart steps 1–2, demo to a friendly reviewer.

### Incremental Delivery

1. Setup + Foundational → foundation ready.
2. + US1 → first usable lint demo (MVP).
3. + US2 → fix + audit trail; the tool becomes a productivity multiplier.
4. + US3 → org-policy support; the tool can ship to a program office.
5. + US4 → web-worker target; unlocks browser/extension distribution channels.
6. Polish → SC-001/002/003/005/006/008 verified; quickstart green.

### Parallel Team Strategy

After Phase 2 completes:

- Developer A drives US1 (rules + check CLI).
- Developer B drives US2 (fix CLI + audit pipeline) once `Engine::fix` lands.
- Developer C drives US3 (config precedence + corrections map).
- Developer D drives US4 (WASM exports + parity harness).

All four converge in Phase 7 for benchmarks and the final SC checks.

---

## Notes

- `[P]` = different files, no dependencies on incomplete tasks.
- `[Story]` = traceability label tying each task to a spec user story.
- Each user story is independently completable and independently testable per `spec.md` §"Independent Test" sections.
- The audit-completeness check (T045) and the no-classifier-id check (T055) are non-negotiable per constitution Principle V and SC-006.
- Avoid: vague tasks, cross-story file conflicts on `marque-capco/src/rules.rs` (coordinate the rule batch), and any new dependency outside the constitution's locked tier.
