<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Tasks: Per-Document Deadlines

**Spec**: [spec.md](spec.md)
**Plan**: [plan.md](plan.md)

**Format**: `- [ ] [ID] [P?] Description with file path`. `[P]` = parallelizable with other `[P]` tasks in the same phase.

**Tests**: First-class deliverables. Each requirement in `spec.md` §R7 lands alongside the implementation it tests, not as a follow-up.

---

## Phase 1: Foundational types (zero behavior change)

- [ ] T001 Create `crates/rules/src/options.rs` with `LintOptions { deadline: Option<Instant> }`, `#[non_exhaustive]`, `derive(Debug, Clone, Default)`. Re-export from `crates/rules/src/lib.rs` (`pub mod options; pub use options::LintOptions;`).
- [ ] T002 [P] Extend `LintResult` in `crates/rules/src/output.rs` (or wherever it lives) with `truncated: bool`, `candidates_processed: usize`, `candidates_total: usize`. Default values for back-compat: `false / 0 / 0`. Verify `LintResult` is `#[non_exhaustive]`; if not, make it so.
- [ ] T003 [P] Add `EngineError::DeadlineExceeded { partial_lint: LintResult }` variant to `crates/engine/src/errors.rs`. `Display` impl: `"engine deadline exceeded after processing N/M candidates"`. `Error::source` returns `None`. Variant is `#[non_exhaustive]`-protected on the enum.
- [ ] T004 Add `Engine::lint_with_options` and `Engine::fix_with_options` signatures with **bodies that ignore `opts` and delegate to existing `lint` / `fix` paths** in `crates/engine/src/engine.rs`. Phase 1 lands the surface; Phase 2 fills the bodies. Existing `Engine::lint` / `Engine::fix` rewired as one-line shims over `_with_options(..., &LintOptions::default())`.
- [ ] T005 [P] Phase 1 unit test `lint_options_default_yields_no_deadline` in `crates/engine/tests/deadline.rs` (new file): `LintOptions::default().deadline.is_none()`.
- [ ] T006 [P] Phase 1 unit test `back_compat_shim_produces_identical_result_to_with_options_default` in `crates/engine/tests/deadline.rs`: `Engine::lint(src) == Engine::lint_with_options(src, &LintOptions::default())` for a fixture document.

**Phase 1 checkpoint**: workspace compiles, all existing tests pass, new tests pass, no behavior change observable.

---

## Phase 2: Engine-side cooperative cancellation

- [ ] T007 Implement pre-pass deadline check at the top of `Engine::lint_with_options` in `crates/engine/src/engine.rs`: if `opts.deadline.is_some_and(|d| Instant::now() >= d)`, return `LintResult { diagnostics: vec![], truncated: true, candidates_processed: 0, candidates_total: 0, ... }` immediately.
- [ ] T008 Add per-candidate deadline check at the top of the candidate iteration loop (currently `engine.rs:352`): break with `truncated: true` and the count-so-far populated.
- [ ] T009 Set `candidates_total = candidates.len()` after the scanner pass, before the rule loop starts. Set `candidates_processed += 1` at the bottom of each successful candidate iteration.
- [ ] T010 Implement pre-fix-loop deadline check in `Engine::fix_inner`: return `Err(EngineError::DeadlineExceeded { partial_lint })` where `partial_lint` is the `LintResult` produced before the fix loop.
- [ ] T011 Add per-fix-application check at the top of the fix-application for-loop: same abort path. Note: the partial_lint at this point includes ALL diagnostics (the lint pass completed); only the fix application is partial. The audit-integrity invariant holds — no partial `FixResult` is ever constructed.
- [ ] T012 Update `Engine::fix` and `Engine::fix_with_threshold` shims to call `expect("fix without deadline cannot return DeadlineExceeded")` and document the invariant inline.
- [ ] T013 [P] Test `lint_with_already_expired_deadline_returns_immediately_truncated` in `crates/engine/tests/deadline.rs`: pass `Instant::now() - 1s` as deadline; assert `truncated: true, candidates_processed: 0, candidates_total: 0`.
- [ ] T014 [P] Test `lint_truncates_mid_document_at_deadline_boundary` in `crates/engine/tests/deadline.rs`: synthesize a long document, set `deadline = Instant::now() + 1ms`, assert `truncated: true` and `candidates_processed > 0` and `candidates_processed < candidates_total`.
- [ ] T015 [P] Test `lint_with_generous_deadline_runs_to_completion_no_truncation` in `crates/engine/tests/deadline.rs`: set `deadline = Instant::now() + 1h`, assert `truncated: false` and `candidates_processed == candidates_total`.
- [ ] T016 [P] Test `fix_with_already_expired_deadline_returns_DeadlineExceeded` in `crates/engine/tests/deadline.rs`: assert `Err(EngineError::DeadlineExceeded { partial_lint })` with empty `partial_lint.diagnostics` (lint pass aborted before producing any).
- [ ] T017 [P] Test `fix_with_deadline_during_apply_loop_returns_DeadlineExceeded_with_partial_lint` in `crates/engine/tests/deadline.rs`: deadline crafted to fall AFTER the lint pass but BEFORE all fixes are applied; assert `Err(DeadlineExceeded)` carrying the full lint diagnostics.
- [ ] T018 New Criterion bench `crates/engine/benches/deadline_overhead.rs`: compares `Engine::lint(src)` (no deadline) vs `Engine::lint_with_options(src, &LintOptions { deadline: Some(Instant::now() + 1h), ..Default::default() })`. Median overhead MUST be ≤ 2% on the standard 10 KB corpus document. Bench is gated by an in-CI threshold check (similar pattern to existing `bench-check`).
- [ ] T019 Add `deadline_overhead` to the regression-gate workflow under `.github/workflows/ci.yml` (mirror existing benchmark gates).

**Phase 2 checkpoint**: deadline behavior fully wired in `Engine`. SC-001 bench unchanged. All Phase 2 tests + Phase 1 tests pass.

---

## Phase 3a: CLI surface wiring

- [ ] T020 Add `humantime` as a direct dep in `marque/Cargo.toml` (verify it's a transitive dep already; pin to a workspace-consistent version).
- [ ] T021 Add `--deadline <DURATION>` flag to the CLI in `marque/src/main.rs` (or wherever the clap definition lives), documented as "Maximum wall-clock budget for processing each input document. Format: humantime, e.g. '30s', '2m'."
- [ ] T022 Validate `--deadline 0` rejection at parse time: clap value parser returns an error → `EX_USAGE` (64).
- [ ] T023 Convert the parsed `Duration` to `Instant::now() + duration` per invocation; pass into `Engine::lint_with_options` / `fix_with_options`.
- [ ] T024 Render truncated `LintResult` from `lint`: existing renderer + final stderr line `"⚠ deadline exceeded: covered N/M candidates"` printed when `result.truncated`.
- [ ] T025 Handle `EngineError::DeadlineExceeded` from fix: print partial-lint diagnostics to stderr, exit `EX_TEMPFAIL` (75) with a clear stderr explanation.
- [ ] T026 [P] Test `cli_deadline_zero_exits_with_EX_USAGE` in `marque/tests/cli.rs`.
- [ ] T027 [P] Test `cli_deadline_truncates_check_output_with_warning` in `marque/tests/cli.rs`: invoke `marque check --deadline 1ms` against a fixture with many candidates, assert truncated output + warning line.
- [ ] T028 [P] Test `cli_deadline_fix_exits_EX_TEMPFAIL` in `marque/tests/cli.rs`: invoke `marque fix --deadline 1ms`, assert exit 75.

---

## Phase 3b: Server surface wiring

- [ ] T029 Add `humantime` as a direct dep in `crates/server/Cargo.toml`.
- [ ] T030 Implement `X-Marque-Deadline` header parsing in `crates/server/src/lib.rs`: humantime-format. Out-of-range / unparseable returns `400 Bad Request`. Empty / absent uses per-endpoint default.
- [ ] T031 Implement `MARQUE_MAX_DEADLINE` env var resolution in `marque-server`'s `resolve_deadline_cap` (mirror the `resolve_body_limit` pattern from gap #6 closure). Default cap: 60 s. Reject below 1 ms / above max.
- [ ] T032 Per-endpoint default deadline: 30 s for `/v1/lint` and `/v1/fix`. Document in §10.2.
- [ ] T033 Convert parsed deadline to `Instant::now() + duration` per request; pass to `Engine::lint_with_options` / `fix_with_options`.
- [ ] T034 Truncated lint response: HTTP 200 with payload + `Marque-Truncated: true` response header.
- [ ] T035 `EngineError::DeadlineExceeded` from fix: HTTP 504 with body containing the partial-lint diagnostics (existing `LintResult` JSON shape, plus a top-level `truncated_by` indicator).
- [ ] T036 [P] Test `header_driven_deadline_truncates_lint_response` in `crates/server/tests/http.rs`: POST `/v1/lint` with `X-Marque-Deadline: 1ms` and a multi-candidate body; assert 200 + `Marque-Truncated: true` header.
- [ ] T037 [P] Test `out_of_range_deadline_header_returns_400` in `crates/server/tests/http.rs`: deadline > cap, deadline < 1 ms, unparseable string.
- [ ] T038 [P] Test `lint_without_header_uses_endpoint_default` in `crates/server/tests/http.rs`: omit header, assert lint runs to completion (default 30 s is generous on the test fixture).
- [ ] T039 [P] Test `fix_deadline_exceeded_returns_504_with_partial_lint_body` in `crates/server/tests/http.rs`.

---

## Phase 3c: WASM surface wiring

- [ ] T040 Add `deadline_ms: Option<f64>` to the JS-side options object in `crates/wasm/src/lib.rs` (matching the existing options shape; if no options object exists yet, this lands as part of one).
- [ ] T041 wasm-bindgen shim: validate `deadline_ms.is_finite() && deadline_ms >= 0.0` (negative / NaN / Inf rejected with a clear JS-thrown error).
- [ ] T042 Convert valid `deadline_ms` to `Instant::now() + Duration::from_millis(deadline_ms as u64)` inside the wasm function body; pass to `Engine::lint_with_options`.
- [ ] T043 Add Constitution III analysis to `crates/wasm/src/lib.rs` doc comment: confirm `deadline_ms` does not introduce a new recognizer codepath or alter posteriors (it's a runtime budget cap, not a vocabulary or scoring change).
- [ ] T044 [P] Test `wasm_deadline_ms_truncates_lint_output_byte_identically_to_native_cli` in `crates/wasm/tests/parity.rs`: same fixture + same deadline → byte-identical NDJSON between WASM and native CLI (extends existing SC-008 parity).

---

## Phase 3d: BatchEngine surface wiring

- [ ] T045 New `BatchOptions { per_doc_deadline: Option<Duration> }` struct in `crates/engine/src/batch.rs`. `#[non_exhaustive]`, `derive(Debug, Clone, Default)`. Doc comment documents that the per-doc budget starts when the document acquires its permit, not when `lint_many_with_options` is called (composes correctly with `ConcurrencyController` wait time).
- [ ] T046 Add `BatchEngine::lint_many_with_options(..., opts: &BatchOptions)` and `fix_many_with_options(..., opts: &BatchOptions)`. Existing bare versions become shims over `_with_options(..., &BatchOptions::default())`.
- [ ] T047 Add `BatchError::DocumentDeadlineExceeded { partial_lint: LintResult }` variant. `is_deadline_exceeded()` predicate. `Display` carries "document deadline exceeded after N/M candidates."
- [ ] T048 Per-doc execution: convert `per_doc_deadline: Option<Duration>` to `LintOptions { deadline: Some(Instant::now() + d) }` after permit acquisition; thread to `Engine::lint_with_options` / `fix_with_options`. Map `Err(EngineError::DeadlineExceeded)` from fix to `BatchError::DocumentDeadlineExceeded`.
- [ ] T049 [P] Test `batch_per_doc_deadline_isolates_one_slow_doc_from_rest` in `crates/engine/tests/batch_deadline.rs` (new file): submit 3 docs (one slow, two fast), set `per_doc_deadline: Some(50ms)`, assert the slow one yields `DocumentDeadlineExceeded` and the fast two yield successful `LintResult`s.
- [ ] T050 [P] Test `batch_deadline_exceeded_lands_as_per_doc_BatchError_variant` in `crates/engine/tests/batch_deadline.rs`: assert the new variant is matchable, `is_deadline_exceeded()` returns `true`, and `is_panic()` / `is_shutdown()` return `false`.

---

## Phase 4: Whitepaper updates + gap closure

- [ ] T051 Rewrite `docs/security/WHITEPAPER.md` §9.7 from `[NON-GOAL]` to `[LANDED]`. New body: cooperative-cancellation model, per-candidate + per-fix granularity, asymmetric response shape (truncated `LintResult` for lint, `Err(DeadlineExceeded)` for fix per Constitution V Principle V), per-surface wiring summary.
- [ ] T052 Update §10.2 server section: `X-Marque-Deadline` header, `MARQUE_MAX_DEADLINE` env var, per-endpoint default 30 s, cap default 60 s, 400/504 response codes.
- [ ] T053 Update §10.3 WASM section: `deadline_ms` parameter, Constitution III runtime-config-restriction analysis, parity invariant preserved.
- [ ] T054 Update §9 Status footer: §9.7 flipped from `[NON-GOAL]` to `[LANDED]`.
- [ ] T055 Strike gap register row #7 in §17 with PR / commit reference; mirror the format of rows 6, 8, 10 (the prior P1 closures).
- [ ] T056 Add Appendix C v0.13 changelog entry summarizing the four-phase landing.

**Phase 4 checkpoint**: `gh issue close 139` with PR reference; gap register down to one open P1 (none — this is the last); whitepaper §9.7 fully landed.

---

## Cross-phase verification

- [ ] T057 Final `cargo test --workspace --no-fail-fast` — all green.
- [ ] T058 Final `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- [ ] T059 Final `cargo deny check` — clean (verify `humantime` is on the WASM-safe allow-list if it transitively reaches WASM-safe crates; otherwise constrain to `marque-cli` and `marque-server`).
- [ ] T060 Verify SC-001 unchanged: re-run `crates/engine/benches/lint_latency.rs`, compare against pre-merge baseline.
- [ ] T061 Verify SC-008 parity unchanged: re-run `crates/wasm/tests/parity.rs` against the full corpus.
- [ ] T062 Verify SC-005 linear scaling unchanged: re-run `crates/engine/benches/linear_scaling.rs`.

## Estimation

Phase 1: ~1 day (mechanical type additions + shim wiring).
Phase 2: ~2 days (engine-loop wiring + tests + bench).
Phase 3a/b/c/d: ~1 day each (parallelizable; ~2 days total wall-clock with parallelism).
Phase 4: ~0.5 day (docs).

Total: ~5–6 days wall-clock, ~7–8 days agent-time if phases run sequentially.
