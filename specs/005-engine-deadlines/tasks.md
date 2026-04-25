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

- [x] T001 Create `crates/engine/src/options.rs` with `LintOptions { deadline: Option<Instant> }` and `FixOptions { deadline: Option<Instant>, threshold_override: Option<f32> }`, both `#[non_exhaustive]`, `derive(Debug, Clone, Default)`. Re-export from `crates/engine/src/lib.rs`. (Placement is under `marque-engine` because `LintResult` / `FixResult` already live there; relocating those into `marque-rules` is out of scope.)
- [x] T002 [P] Extend `LintResult` in `crates/engine/src/output.rs` (current definition site; re-exported from `marque-engine`) with `truncated: bool`, `candidates_processed: usize`, `candidates_total: usize`. Default values for back-compat: `false / 0 / 0`. Add `#[non_exhaustive]` to the struct (currently absent). Update existing tests / fixtures that brace-construct `LintResult` to use struct-update syntax (`LintResult { diagnostics, ..Default::default() }`).
- [x] T003 [P] Introduce a new `EngineError` enum in `crates/engine/src/errors.rs` alongside the existing `EngineConstructionError` (which stays unchanged — runtime/build-time errors are intentionally separate types). Variants: `DeadlineExceeded { partial_lint: LintResult }` and `InvalidThreshold(InvalidThreshold)` (wrapping the existing standalone struct). `#[non_exhaustive]`. `Display` impl for `DeadlineExceeded`: `"engine deadline exceeded after processing N/M candidates"` (counts pulled from `partial_lint`); for `InvalidThreshold(it)` delegate to `it`'s `Display`. `Error::source` returns `None` for `DeadlineExceeded` and `Some(&InvalidThreshold)` for `InvalidThreshold(_)`. Provide `From<InvalidThreshold> for EngineError`.
- [x] T004 Add `Engine::lint_with_options(&[u8], &LintOptions) -> LintResult` and `Engine::fix_with_options(&[u8], FixMode, &FixOptions) -> Result<FixResult, EngineError>` in `crates/engine/src/engine.rs`. Phase 1 bodies: `lint_with_options` ignores `opts.deadline` and delegates to the existing lint path; `fix_with_options` ignores `opts.deadline` but already honors `opts.threshold_override` (delegating to the existing `fix_inner` with the threshold), mapping any returned `InvalidThreshold` through `EngineError::InvalidThreshold` via the `From` impl. Phase 2 fills in the deadline checks. Rewire the back-compat shims: `Engine::lint(&[u8])` calls `lint_with_options(.., &LintOptions::default())`; `Engine::fix(&[u8], FixMode)` calls `fix_with_options(.., &FixOptions::default()).expect("fix() default options cannot fail: no deadline + pre-validated config threshold")`; `Engine::fix_with_threshold(&[u8], FixMode, Option<f32>) -> Result<FixResult, InvalidThreshold>` keeps its public signature and internally constructs `FixOptions { threshold_override, ..Default::default() }`, mapping `EngineError::InvalidThreshold(it) → Err(it)` and using `unreachable!(...)` for `EngineError::DeadlineExceeded` (no caller of `fix_with_threshold` can set a deadline through its signature).
- [x] T005 [P] Phase 1 unit test `lint_options_default_yields_no_deadline` in `crates/engine/tests/deadline.rs` (new file): `LintOptions::default().deadline.is_none()`. Plus a `fix_options_default_yields_no_deadline_and_no_threshold_override` test for the companion struct.
- [x] T006 [P] Phase 1 unit test `back_compat_shim_produces_identical_result_to_with_options_default` in `crates/engine/tests/deadline.rs`: `Engine::lint(src)` produces an equivalent `LintResult` to `Engine::lint_with_options(src, &LintOptions::default())` for a fixture document. Companion test exercising `Engine::fix_with_threshold` against `Engine::fix_with_options` for both the Ok and `InvalidThreshold` paths.

**Phase 1 checkpoint**: workspace compiles, all existing tests pass, new tests pass, no behavior change observable.

---

## Phase 2: Engine-side cooperative cancellation

- [x] T007 Implement pre-pass deadline check at the top of `Engine::lint_with_options` in `crates/engine/src/engine.rs`: if `opts.deadline.is_some_and(|d| Instant::now() >= d)`, return `LintResult { diagnostics: vec![], truncated: true, candidates_processed: 0, candidates_total: 0, ... }` immediately.
- [x] T008 Add per-candidate deadline check at the top of the candidate iteration loop (currently `engine.rs:352`): break with `truncated: true` and the count-so-far populated.
- [x] T009 Set `candidates_total = candidates.len()` after the scanner pass, before the rule loop starts. Set `candidates_processed += 1` at the bottom of each successful candidate iteration.
- [x] T010 Implement pre-fix-loop deadline check in `Engine::fix_inner`: return `Err(EngineError::DeadlineExceeded { partial_lint })` where `partial_lint` is the `LintResult` produced before the fix loop.
- [x] T011 Add per-fix-application check at the top of the fix-application for-loop: same abort path. Note: the partial_lint at this point includes ALL diagnostics (the lint pass completed); only the fix application is partial. The audit-integrity invariant holds — no partial `FixResult` is ever constructed.
- [x] T012 Update the back-compat shims in `crates/engine/src/engine.rs` to reflect the deadline behavior:
  - `Engine::fix` keeps its `expect("fix() default options cannot fail: ...")` invariant (deadline: None + pre-validated config threshold). The expect message documents both invariants inline.
  - `Engine::fix_with_threshold` is **not** an `expect`-based shim because it is already fallible (`Result<FixResult, InvalidThreshold>`). Internal flow: construct `FixOptions { threshold_override, ..Default::default() }`, call `fix_with_options`, and explicitly map the returned `EngineError`: `EngineError::InvalidThreshold(it) → Err(it)`, `EngineError::DeadlineExceeded { .. } → unreachable!("fix_with_threshold cannot set a deadline through its signature")`. The mapping is documented inline so the invariant cannot be silently broken by a future signature change.
- [x] T013 [P] Test `lint_with_already_expired_deadline_returns_immediately_truncated` in `crates/engine/tests/deadline.rs`: pass `Instant::now() - 1s` as deadline; assert `truncated: true, candidates_processed: 0, candidates_total: 0`.
- [x] T014 [P] Test `lint_truncates_mid_document_at_deadline_boundary` in `crates/engine/tests/deadline.rs`: synthesize a long document, set a tight deadline, assert `truncated: true` and `candidates_processed < candidates_total`.
- [x] T015 [P] Test `lint_with_generous_deadline_runs_to_completion_no_truncation` in `crates/engine/tests/deadline.rs`: set `deadline = Instant::now() + 1h`, assert `truncated: false` and `candidates_processed == candidates_total`.
- [x] T016 [P] Test `fix_with_already_expired_deadline_returns_deadline_exceeded` in `crates/engine/tests/deadline.rs`: assert `Err(EngineError::DeadlineExceeded { partial_lint })` with empty `partial_lint.diagnostics` (lint pass aborted before producing any).
- [x] T017 [P] Test `fix_with_deadline_during_fix_call_returns_deadline_exceeded` in `crates/engine/tests/deadline.rs`: deadline at half the warm `fix` baseline. Asserts `Err(DeadlineExceeded)` and the internal consistency invariant `partial_lint.candidates_processed <= partial_lint.candidates_total`. The test accepts either trip path — post-lint check (T010) or per-fix-application check (T011) — because which one fires is hardware-dependent: on slow CI runners where lint dominates `fix` runtime, the deadline trips inside the candidate loop and the post-lint check converts that to Err; on fast machines where apply is observable, the per-fix check fires inside the apply loop. Both produce the same `Err(DeadlineExceeded)` shape, which is the load-bearing behavior. (The earlier version asserted specifically the apply-loop-trip path with `truncated: false` partial_lint, but that was unreachable on CI hardware where apply completes in microseconds — there's no margin window where `deadline > lint_time && deadline < lint_time + apply_time`.)
- [x] T018 New Criterion bench `crates/engine/benches/deadline_overhead.rs`: compares `Engine::lint(src)` (no deadline) vs `Engine::lint_with_options(src, &LintOptions { deadline: Some(Instant::now() + 1h), ..Default::default() })`. Spec design target is ≤ 2% overhead on the 10 KB corpus document. The bench gate in `benches/baseline.json` is set permissively to 10% as a starting point — Instant::now() cost varies dramatically by host (~10-30ns native vDSO, ~500-700ns under WSL2 hypervisor clock), and the 2% target is reachable on native Linux but masked by WSL2 jitter. Tightening the gate to 2% is a follow-up once a CI-runner baseline is captured.
- [x] T019 Add `deadline_overhead` to the regression-gate workflow under `.github/workflows/ci.yml`. The CI bench-check job already invokes `scripts/bench-check.sh` which now calls `check_deadline_overhead`; the workflow comment block was updated to document the new gate.

**Phase 2 checkpoint**: deadline behavior fully wired in `Engine`. SC-001 bench unchanged. All Phase 2 tests + Phase 1 tests pass.

---

## Phase 3a: CLI surface wiring

- [x] T020 Reuse the existing `humantime = { workspace = true }` dependency already declared in `marque/Cargo.toml`; no new dependency entry needed for the CLI flag. (Confirmed at `marque/Cargo.toml:30`.)
- [x] T021 Add `--deadline <DURATION>` flag to the CLI in `marque/src/main.rs` (or wherever the clap definition lives), documented as "Maximum wall-clock budget for processing each input document. Format: humantime, e.g. '30s', '2m'."
- [x] T022 Validate `--deadline 0` rejection at parse time: humantime parse failure (e.g. `--deadline 0` with no unit) and an explicit `Duration::ZERO` (e.g. `--deadline 0s`) both surface as `EX_USAGE` (64). Validation lives in `validate_deadline()`, mirroring the post-parse `validate_threshold()` pattern.
- [x] T023 Convert the parsed `Duration` to `Instant::now() + duration` per invocation; pass into `Engine::lint_with_options` / `fix_with_options`.
- [x] T024 Render truncated `LintResult` from `lint`: existing renderer + final stderr line `"⚠ deadline exceeded: covered N/M candidates"` printed when `result.truncated`.
- [x] T025 Handle `EngineError::DeadlineExceeded` from fix: print partial-lint diagnostics to stderr, exit `EX_TEMPFAIL` (75) with a clear stderr explanation.
- [x] T026 [P] Test `cli_deadline_zero_exits_with_ex_usage` (and `_zero_seconds_` and `_unparseable_` variants) in `marque/tests/cli_deadline.rs`.
- [x] T027 [P] Test `cli_deadline_truncates_check_output_with_warning` in `marque/tests/cli_deadline.rs`: invoke `marque check --deadline 1ms` against a 4 000-banner stdin input, assert stderr warning + exit 0/1/2.
- [x] T028 [P] Test `cli_deadline_fix_exits_ex_tempfail` in `marque/tests/cli_deadline.rs`: invoke `marque fix --deadline 1ms`, assert exit 75 and stderr explanation.

---

## Phase 3b: Server surface wiring

- [x] T029 (resolved by Q6) — server does **not** add `humantime` as a dep. The header is parsed as unsigned-integer milliseconds via `str::parse::<u64>()`. Confirmed in `crates/server/Cargo.toml`.
- [x] T030 `resolve_request_deadline()` in `crates/server/src/lib.rs` parses `X-Marque-Deadline` as `u64` ms, rejecting negative / non-numeric / overflow / below 1 ms / above `state.deadline_cap` with `400 Bad Request`. Empty / absent uses the per-endpoint default.
- [x] T031 `resolve_deadline_cap()` mirrors `resolve_body_limit()`. Default cap 60 s; rejects below 1 ms (`MIN_DEADLINE_MS`) / above 10 min (`MAX_DEADLINE_CAP_MS`). Pure decision logic factored into `classify_deadline_cap_var` for unit-test reachability.
- [x] T032 Per-endpoint default 30 s (`DEFAULT_ENDPOINT_DEADLINE_MS`) shared by `/v1/lint` and `/v1/fix`. Recorded on every server startup line as `deadline_cap_ms`.
- [x] T033 Each handler stamps `Instant::now() + duration` per request and threads it through `Engine::lint_with_options` / `fix_with_options` via `LintOptions` / `FixOptions`.
- [x] T034 Truncated lint: HTTP 200 + `Marque-Truncated: true` response header + body fields `truncated`, `candidates_processed`, `candidates_total`.
- [x] T035 `EngineError::DeadlineExceeded` from fix: HTTP 504 with `DeadlineExceededBody { truncated_by, diagnostics, error_count, warn_count, fix_count, candidates_processed, candidates_total }` JSON.
- [x] T036 [P] `header_driven_deadline_truncates_lint_response` in `crates/server/tests/http_deadline.rs`.
- [x] T037 [P] `deadline_header_zero/non_numeric/negative/above_cap/just_above_configured_cap/overflow_returns_400` covering each rejection path.
- [x] T038 [P] `lint_without_header_uses_endpoint_default` (and asserts no `Marque-Truncated` header on the happy path).
- [x] T039 [P] `fix_deadline_exceeded_returns_504_with_partial_lint_body`.

---

## Phase 3c: WASM surface wiring

- [x] T040 Added `deadline_ms: Option<f64>` to `WasmConfig` in `crates/wasm/src/lib.rs`. Carried through `parse_wasm_config` alongside the existing `Config`-level fields.
- [x] T041 Validation in `parse_deadline_ms`: `is_finite() && >= 0.0`; negative / NaN / Inf rejected with a structured JS error string. `serde_json` handles the bulk of the rejection at JSON-parse time; the explicit `is_finite()` check is the second line of defense.
- [x] T042 `stamp_deadline()` converts a parsed `Duration` to `Instant::now().checked_add(d)`, mapping overflow to a JS error. Passed into `Engine::lint_with_options` / `fix_with_options`. `EngineError::DeadlineExceeded` from fix returns `Err(...)` carrying a JSON-serialized `DeadlineExceededBodyJson` (mirrors the server's 504 response shape).
- [x] T043 Constitution III analysis added to the crate-level doc comment. Confirms `deadline_ms` does not introduce a new recognizer codepath, does not alter posteriors, and does not change the vocabulary surface. Notes that `lint_deep_scan` / `fix_deep_scan` (Gate 2) deliberately remain byte-only.
- [x] T044 [P] `wasm_deadline_ms_generous_matches_native_full_lint` and `wasm_deadline_ms_zero_yields_empty_ndjson_byte_identical_to_native` in `crates/wasm/tests/deadline_parity.rs` (new file). Verifies byte-identical NDJSON across two deterministic deadline shapes. Mid-pass truncation parity intentionally not tested — `Instant::now()` is sampled independently per call.

**Required infrastructure**: added `web-time` workspace dep + engine dep; replaced `use std::time::Instant` with `use web_time::Instant` in `crates/engine/src/options.rs` and `crates/engine/src/engine.rs`. `web_time::Instant` is a literal `pub use` of `std::time::Instant` on native targets and a `Performance.now()` polyfill on `wasm32-unknown-unknown` — without this, the engine's per-candidate `Instant::now()` deadline check would panic in the WASM target. Re-exported as `marque_engine::Instant` so the WASM crate doesn't need its own web-time dep.

---

## Phase 3d: BatchEngine surface wiring

- [ ] T045 Extend the existing `BatchOptions` struct (`crates/engine/src/batch.rs:167`) with `per_doc_deadline: Option<Duration>`. The struct is already passed to `BatchEngine::new(engine, options)`; preserve `#[non_exhaustive]` (verify it is set; if not, add it as part of this task), update `Default` to `per_doc_deadline: None`, and document inline that the per-doc budget starts when the document acquires its permit (not when `lint_many_with_options` is called) so it composes correctly with `ConcurrencyController` wait time.
- [ ] T046 Add `BatchEngine::lint_many_with_options(...)` and `fix_many_with_options(...)` variants that consume the new `per_doc_deadline` field on `BatchOptions`. Existing `lint_many` / `fix_many` methods are unchanged (the `BatchOptions` already passed to `BatchEngine::new` carries the deadline at construction time, so per-call options are not needed for the non-`_with_options` variants — they pick up the deadline from the engine struct's owned options).
- [ ] T047 Add `BatchError::DocumentDeadlineExceeded { partial_lint: LintResult }` variant. `is_deadline_exceeded()` predicate. `Display` carries "document deadline exceeded after N/M candidates."
- [ ] T048 Per-doc execution: read `BatchOptions.per_doc_deadline` and convert it to `FixOptions { deadline: Some(Instant::now() + d), ..Default::default() }` (and `LintOptions { deadline: Some(...) }`) after permit acquisition; thread to `Engine::lint_with_options` / `fix_with_options`. Map `Err(EngineError::DeadlineExceeded { partial_lint })` from fix to `BatchError::DocumentDeadlineExceeded { partial_lint }`. (Threshold overrides remain a per-doc concern outside `BatchOptions`'s scope; if a future spec needs per-doc threshold overrides, that lands as a separate addition to `BatchOptions`.)
- [ ] T049 [P] Test `batch_per_doc_deadline_isolates_one_slow_doc_from_rest` in `crates/engine/tests/batch_deadline.rs` (new file): submit 3 docs (one slow, two fast), set `per_doc_deadline: Some(50ms)`, assert the slow one yields `DocumentDeadlineExceeded` and the fast two yield successful `LintResult`s.
- [ ] T050 [P] Test `batch_deadline_exceeded_lands_as_per_doc_BatchError_variant` in `crates/engine/tests/batch_deadline.rs`: assert the new variant is matchable, `is_deadline_exceeded()` returns `true`, and `is_panic()` / `is_shutdown()` return `false`.

---

## Phase 4: Whitepaper updates + gap closure

- [ ] T051 Rewrite `docs/security/WHITEPAPER.md` §9.7 from `[NON-GOAL]` to `[LANDED]`. New body: cooperative-cancellation model, per-candidate + per-fix granularity, asymmetric response shape (truncated `LintResult` for lint, `Err(DeadlineExceeded)` for fix per Constitution V Principle V), per-surface wiring summary.
- [ ] T052 Update §10.2 server section: `X-Marque-Deadline` header (unsigned-integer milliseconds, e.g. `30000`), `MARQUE_MAX_DEADLINE` env var (also milliseconds), per-endpoint default 30 s (= `30000`), cap default 60 s (= `60000`), 400/504 response codes.
- [ ] T053 Update §10.3 WASM section: `deadline_ms` parameter, Constitution III runtime-config-restriction analysis, parity invariant preserved.
- [ ] T054 Update §9 Status footer: §9.7 flipped from `[NON-GOAL]` to `[LANDED]`.
- [ ] T055 Strike gap register row #7 in §17 with PR / commit reference; mirror the format of rows 6, 8, 10 (the prior P1 closures).
- [ ] T056 Add Appendix C v0.13 changelog entry summarizing the four-phase landing.

**Phase 4 checkpoint**: `gh issue close 139` with PR reference; gap register down to one open P1 (none — this is the last); whitepaper §9.7 fully landed.

---

## Cross-phase verification

- [ ] T057 Final `cargo test --workspace --no-fail-fast` — all green.
- [ ] T058 Final `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- [ ] T059 Final `cargo deny check` — clean. `humantime` is used only by `marque-cli` (already a direct dep). Server uses `str::parse::<u64>()` per Q6, so no transitive humantime leak into the server graph. Verify `humantime` does not reach the WASM-safe allow-list via any path; if it does, that's a regression to investigate.
- [ ] T060 Verify SC-001 unchanged: re-run `crates/engine/benches/lint_latency.rs`, compare against pre-merge baseline.
- [ ] T061 Verify SC-008 parity unchanged: re-run `crates/wasm/tests/parity.rs` against the full corpus.
- [ ] T062 Verify SC-005 linear scaling unchanged: re-run `crates/engine/benches/linear_scaling.rs`.

## Estimation

Phase 1: ~1 day (mechanical type additions + shim wiring).
Phase 2: ~2 days (engine-loop wiring + tests + bench).
Phase 3a/b/c/d: ~1 day each (parallelizable; ~2 days total wall-clock with parallelism).
Phase 4: ~0.5 day (docs).

Total: ~5–6 days wall-clock, ~7–8 days agent-time if phases run sequentially.
