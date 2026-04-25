<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Per-Document Deadlines on `Engine::lint` and `Engine::fix`

**Status**: Draft — design decisions pending
**Branch**: `feat/engine-deadlines` (proposed)
**Closes**: #139 (P1 gap-register row 7)
**Authority**: Whitepaper §9.7 (`docs/security/WHITEPAPER.md`), Constitution Principle I (uncompromising performance), Principle V (audit-first compliance), Principle VI (dataflow pipeline model)
**Related**:
  - PR #158 closed gap-register rows #5 and #9; row #7 is the last remaining open P1.
  - Whitepaper §10.2 server section calls out body-size cap (closed #6) and timeout (open #7) as the two halves of the deployment-DoS posture.
  - `BatchOptions` does not currently exist as a struct; `BatchEngine` is configured via constructor args. This spec introduces `BatchOptions` opportunistically.

## Problem

`Engine::lint` and `Engine::fix` block the caller thread synchronously until completion. A pathological document — deeply-nested portion structures, a corrections-map that triggers exponential matching, a future decoder bug, an adversarial input crafted to maximize backtracking — can pin a CPU thread indefinitely.

This is a real DoS surface for the deployed surfaces:

- **`marque-server`**: HTTP handlers wrap `Engine::lint` in `tokio::task::spawn_blocking`. `tokio::time::timeout` cancels the *future*; the blocking thread keeps running. A single 30-minute document silently consumes a worker thread for 30 minutes regardless of any handler-side deadline.
- **`marque-cli`**: A user running `marque check` against a hostile file blocks their shell indefinitely.
- **`BatchEngine`**: Per-document worst-case directly multiplies by `max_concurrent_docs` — one bad document degrades the entire batch run.
- **`marque-wasm`**: A web worker pinned indefinitely cannot be cancelled by the JS host (postMessage doesn't preempt synchronous code).

The whitepaper currently lists this as `[NON-GOAL]` at the engine layer, deferred to deployment concerns. That deferral is no longer tenable: deployment-side `tokio::time::timeout` does not preempt the underlying CPU work, so the protection is illusory.

## Threat surface (informs design)

| Surface | Caller risk if no deadline | Bypass surface today |
|---|---|---|
| `marque-server` handlers | One bad request pins a worker thread; capacity drops by 1 per attack request until restart | `tokio::time::timeout` on the spawn_blocking future cancels the future, NOT the thread |
| `BatchEngine.lint_many` | Stuck doc holds a `ConcurrencyController` permit forever; throughput collapses | Same — async cancellation does not preempt CPU work |
| `marque-cli` | UX-only; user can SIGINT | None at engine layer |
| `marque-wasm` | Web worker hangs; main thread responsive but worker unusable | None at engine layer; postMessage cannot preempt sync code |

## Scope

This spec covers a per-document deadline parameter threaded through `Engine::lint` and `Engine::fix`, with cooperative-cancellation enforcement at scanner-candidate granularity and per-fix granularity.

**In scope:**

1. New `LintOptions` struct carrying `deadline: Option<Instant>` and a stable shape for future engine-level options.
2. New `Engine::lint_with_options(&[u8], &LintOptions) -> LintResult` and `Engine::fix_with_options(&[u8], FixMode, &LintOptions) -> Result<FixResult, EngineError>`. Existing `Engine::lint(&[u8])` becomes a thin shim over `lint_with_options(&[u8], &LintOptions::default())`; same for `Engine::fix`.
3. Cooperative cancellation at scanner-candidate granularity (in the per-candidate loop in `Engine::lint`) and per-fix granularity (in `Engine::fix_inner`'s apply loop).
4. Asymmetric timeout response (see [D3](#d3-timeout-response-shape) below): `lint` returns a truncated `LintResult` with a flag; `fix` returns `Err(EngineError::DeadlineExceeded)` so a deadline-hit during compliance-output construction cannot be silently shipped.
5. Per-surface wiring:
   - `marque-cli`: `--deadline <duration>` flag (humantime; e.g., `--deadline 30s`); not set by default.
   - `marque-server`: `X-Marque-Deadline` HTTP header (humantime), capped by config; per-endpoint default for cases where the header is absent.
   - `marque-wasm`: `deadline_ms` field on the JS-side options object.
   - `BatchEngine`: new `BatchOptions { per_doc_deadline: Option<Duration>, ... }` parameter on `lint_many` / `fix_many`.
6. Test coverage: deadline-already-passed, deadline-hit-mid-document, deadline-clear (no truncation under generous deadline), per-surface wiring tests, deadline-overhead bench.
7. Whitepaper §9.7 flips from `[NON-GOAL]` to `[LANDED]`; gap row 7 struck through.

**Out of scope:**

- Preemptive cancellation (watchdog thread + `pthread_kill` / signal-based interrupt). Breaks borrow-checker invariants, requires `unsafe`, platform-specific, and not justified for the threat surface.
- Memory-budget gating (separate concern; a future spec).
- Cancellation token threaded through `tokio::sync::CancellationToken` for proactive caller-driven abort. The deadline is sufficient for the immediate gap; a future spec can extend the `LintOptions` shape.
- Whole-batch deadlines that span multiple documents. Batch-level SLA is a deployment concern; per-document deadlines compose adequately.
- Decoder-internal deadline (the deep-scan codepath). Phase D's decoder has its own K=8 bound and per-template guarantees; if a single decoder invocation exceeds the per-document deadline, the per-candidate check at the next iteration will catch it.

## Design decisions (PENDING — author confirmation requested)

The issue (#139) calls out four design questions. This section presents each with the analysis and the spec author's recommendation. **None of these are committed until you confirm.** Implementation cannot start until each is resolved.

### D1 — Cooperative vs preemptive cancellation

**Options:**

- **(a) Cooperative**: scanner / parser / rule loop polls a `should_abort` flag periodically. Implemented as `Instant::now() < deadline` checks at known granularity boundaries.
- **(b) Preemptive**: spawn a watchdog thread that interrupts. Requires `unsafe`, breaks borrow-checker assumptions, platform-specific signal handling.

**Recommendation: (a) cooperative.** Preemptive is essentially never the right Rust answer; the borrow checker assumes single-owner mutation, and signal-based interrupts violate that with no way to discharge the obligation safely. Cooperative is the universal Rust idiom (`tokio::time::timeout`, `crossbeam::channel::select_timeout`, `std::sync::mpsc`'s `recv_timeout`).

**Granularity sub-decision:**

| Granularity | Check cost | Abort latency worst case | Practicality |
|---|---|---|---|
| Per-byte | Branch + clock per byte (defeats SIMD) | 0 | ❌ kills SC-001 |
| Per-candidate | Branch + clock per candidate (~100/doc) | One candidate's scanner+parser+rule pass (~0.16 ms median, < 100 ms worst case) | ✅ best fit |
| Per-rule-per-candidate | ~50 rules × N candidates checks | Sub-candidate | Overkill; rule loop is already O(rules) per candidate |
| Per-page | One check per scanner page-break | Whole page worth of work | ❌ pages can carry thousands of portions |
| Pre-pass only | One check at start of `lint` | Entire document | ❌ defeats the purpose |

**Sub-recommendation: per-candidate in the main lint loop, per-fix in the fix-application loop.** Median 0.16 ms per scanner candidate (extrapolated from SC-001 corpus: p95 ≤ 16 ms on a 10 KB document with ~100 candidates) means a `deadline + 1s` end-to-end abort guarantee holds even for pathological per-candidate work; per-fix in the apply loop covers the post-lint compliance-output path.

### D2 — Where does the deadline parameter live?

**Options:**

- **(a) Direct parameter**: `Engine::lint(source, deadline: Option<Instant>)`. Every future option becomes another arg.
- **(b) Separate method**: `Engine::lint_with_deadline(source, Instant)`. Doubles the method surface; future options need yet more methods.
- **(c) Options struct**: `Engine::lint_with_options(source, &LintOptions)`. Existing `lint(source)` calls into `lint_with_options(source, &LintOptions::default())`.

**Recommendation: (c) options struct.** Future engine-level options (audit-identity injection point, content-ignorance level, opt-in per-rule classification floor override, etc.) all want this shape. The existing `Engine::lint(&[u8]) -> LintResult` signature is preserved as a backward-compat shim.

```rust
// crates/rules/src/lib.rs OR crates/engine/src/options.rs (placement decision in plan.md)
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct LintOptions {
    /// Wall-clock deadline for this document. `None` = no deadline.
    /// Past `Instant`s cause the engine to abort before any candidate
    /// is processed.
    pub deadline: Option<std::time::Instant>,
    // Future fields land here — `#[non_exhaustive]` prevents adding
    // them from being a semver break.
}
```

`#[non_exhaustive]` means downstream callers must use struct-update syntax (`LintOptions { deadline: Some(..), ..Default::default() }`), which keeps the API stable across additions.

### D3 — Timeout response shape

**Options:**

- **(a) Truncated result**: `lint` and `fix` both return their normal types with new fields:
  - `LintResult { ..., truncated: bool, candidates_processed: usize, candidates_total: usize }`
  - `FixResult { ..., truncated_lint: bool, ... }`
  - Caller must check the flag to know the run was incomplete.
- **(b) Result variant**: both return `Result<_, EngineError::DeadlineExceeded>`. Caller cannot ignore the error.
- **(c) Asymmetric**: `lint` returns truncated `LintResult`; `fix` returns `Result<FixResult, EngineError::DeadlineExceeded { partial_lint: LintResult }>`.

**Recommendation: (c) asymmetric.** The compliance-integrity argument:

- `lint`'s output is diagnostics. Partial diagnostics are *useful* — a UI renderer or CI gate that says "we found these 47 errors before the budget ran out, you may have more" is more useful than a hard error. The truncation flag is non-load-bearing for downstream correctness.
- `fix`'s output is an `AppliedFix` audit stream — Constitution V Principle V's compliance output. A partial lint that misses a violation may produce a `FixResult` that applies fixes the document didn't need, or fails to apply fixes that other (un-evaluated) rules would have suppressed. The audit trail then reads "engine fixed X" without the trail saying "...but skipped half the document." That is the kind of silent compliance failure the audit record exists to prevent.

Asymmetric design forces compliance code paths to handle deadline expiry explicitly; advisory paths can keep the simpler shape.

```rust
// crates/engine/src/errors.rs
#[non_exhaustive]
pub enum EngineError {
    // ... existing variants ...
    /// `Engine::fix_with_options` aborted because the deadline expired
    /// before the fix-application loop completed.
    DeadlineExceeded {
        /// The partial `LintResult` produced before the deadline hit.
        /// Useful for rendering "we got this far" UIs while preserving
        /// the compliance integrity invariant on the missing
        /// `FixResult`.
        partial_lint: LintResult,
    },
}

// crates/rules/src/output.rs (or wherever LintResult lives)
#[non_exhaustive]
pub struct LintResult {
    // ... existing fields ...
    /// `true` when the document was not fully processed because the
    /// `LintOptions::deadline` expired. `diagnostics` contains
    /// whatever was produced before abort; `candidates_processed` /
    /// `candidates_total` describe how far the lint pass got.
    pub truncated: bool,
    pub candidates_processed: usize,
    pub candidates_total: usize,
}
```

`candidates_total` is the count produced by the scanner pass (which runs to completion before the per-candidate rule loop starts — scanner is bounded by document length and is not subject to mid-document abort in this design). `candidates_processed` is the number that survived their full rule pass before abort. The pair lets a renderer compute "we covered 47% of the document."

If you prefer (a) symmetric truncated-flag everywhere — cleaner API, weaker compliance gate — I'll switch the recommendation. The audit-integrity argument is the load-bearing one for (c).

### D4 — Per-surface wiring details

The issue's surface-by-surface plan is uncontroversial; one nuance worth flagging.

**`marque-cli`:**
- New `--deadline <duration>` flag, parsed via `humantime::parse_duration` (e.g., `--deadline 30s`, `--deadline 2m`). Already a transitive dep via `tracing-subscriber`; verify with `cargo tree`.
- No default deadline (interactive use should not surprise the user).
- A nuance: `--deadline 0` is rejected at the CLI boundary as `EX_USAGE` (64). Otherwise the engine immediately aborts before any candidate is processed, producing an empty `truncated` `LintResult` — surprising and useless.
- Truncated `LintResult` from `lint`: render as usual + a final stderr line `"⚠ deadline exceeded: covered N/M candidates"`.
- `EngineError::DeadlineExceeded` from `fix`: exit `EX_TEMPFAIL` (75) — "transient failure, try again" — and print the partial-lint diagnostics to stderr.

**`marque-server`:**
- `X-Marque-Deadline` HTTP header on `/v1/lint` and `/v1/fix`, parsed via `humantime`. Capped by a `MARQUE_MAX_DEADLINE` env var (default 60 s) to prevent a hostile client from DoS'ing themselves over a long deadline that holds a worker thread.
- Per-endpoint default deadline (proposal: 30 s) when the header is absent.
- Header value below 1 ms or above the cap returns `400 Bad Request` (consistent with the body-limit pattern in §10.2).
- Truncated lint response: HTTP 200 with the truncated payload + a `Marque-Truncated: true` response header for clients that don't deserialize the full body.
- `EngineError::DeadlineExceeded` from fix: HTTP 504 Gateway Timeout, body carries the partial-lint diagnostics.

**`marque-wasm`:**
- `deadline_ms` field on the JS-side options object passed to the `lint`/`fix` wasm-bindgen functions.
- `Instant` doesn't cross the wasm-bindgen boundary cleanly; the wasm binding parses `deadline_ms: f64` and converts it to `Instant::now() + Duration::from_millis(...)` inside the wasm shim.
- Per Constitution III's WASM runtime-config restriction (no surface that introduces a new recognizer codepath or alters posteriors): a deadline does not change recognizer behavior, only the run's wallclock budget. Confirming this is permitted under the constitution's letter.

**`BatchEngine`:**
- New `BatchOptions { per_doc_deadline: Option<Duration>, ... }` struct. Existing `BatchEngine::lint_many` / `fix_many` gain `_with_options` variants; the bare versions become shims.
- `Duration` (not `Instant`): each per-doc invocation computes its own `Instant::now() + per_doc_deadline` so a slow document doesn't eat into a fast one's budget.
- Per-doc `EngineError::DeadlineExceeded` lands as a per-document `BatchError::DocumentDeadlineExceeded { partial_lint }` variant — distinct from `is_panic()` and `is_shutdown()`, with a matching `is_deadline_exceeded()` predicate, so dashboards can track timeout rate as a separate signal.

## Requirements

### R1 — `LintOptions` struct (foundational)

The engine MUST accept a `LintOptions` struct as a per-call configuration carrier. The Phase 1 shape contains exactly one field: `deadline: Option<Instant>`. The struct MUST be `#[non_exhaustive]` so future fields can land without semver-breaking downstream callers. The struct MUST derive `Default` and `Clone`.

Placement: `crates/rules/src/lib.rs` re-exporting from a sibling `options` module. (Not `crates/engine/`, because Constitution VII keeps the `LintResult` / `FixResult` / `LintOptions` types in `marque-rules` so other potential engine implementations can share the contract.)

### R2 — `Engine::lint_with_options` / `Engine::fix_with_options`

The engine MUST expose:

```rust
impl Engine {
    pub fn lint_with_options(&self, source: &[u8], opts: &LintOptions) -> LintResult;
    pub fn fix_with_options(
        &self,
        source: &[u8],
        mode: FixMode,
        opts: &LintOptions,
    ) -> Result<FixResult, EngineError>;

    // Existing signatures preserved as backward-compat shims:
    pub fn lint(&self, source: &[u8]) -> LintResult {
        self.lint_with_options(source, &LintOptions::default())
    }
    pub fn fix(&self, source: &[u8], mode: FixMode) -> FixResult {
        self.fix_with_options(source, mode, &LintOptions::default())
            .expect("fix without deadline cannot return DeadlineExceeded")
    }
    pub fn fix_with_threshold(
        &self,
        source: &[u8],
        mode: FixMode,
        threshold: f32,
    ) -> FixResult {
        // existing body, with internal call updated to use options
    }
}
```

`Engine::fix` calling `expect(...)` is safe because `LintOptions::default().deadline = None` makes deadline-expiry impossible. The expect-message documents that invariant for any future reader.

### R3 — Cooperative cancellation granularity

The lint loop in `Engine::lint_with_options` MUST check `Instant::now() < deadline` at the following points:

1. **Pre-pass**: once at the start of `lint_with_options`, before any work. A past `Instant` returns immediately with `truncated: true, candidates_processed: 0, candidates_total: 0`.
2. **Per scanner candidate**: at the top of the for-loop in `Engine::lint`'s candidate iteration (currently `engine.rs:352`). Aborts mid-loop with `truncated: true` and the count so far.
3. **Pre-fix-loop** in `Engine::fix_inner`: once before the fix-application for-loop. Aborts with `Err(EngineError::DeadlineExceeded { partial_lint })`.
4. **Per fix application**: at the top of the fix-application for-loop. Same abort path.

The deadline check MUST NOT appear inside the hot path of the scanner or parser themselves (Constitution I — interactive p95 ≤ 16 ms). The per-candidate boundary is the natural granularity because it sits between the scanner's SIMD work and the rule loop's per-candidate dispatch.

### R4 — Asymmetric response shape

`Engine::lint_with_options` MUST return a `LintResult` regardless of deadline expiry; the truncation MUST be exposed via new fields `truncated: bool`, `candidates_processed: usize`, `candidates_total: usize`. (A deadline-exceeded `LintResult` is otherwise indistinguishable from a complete one — it has the partial diagnostic set and is safe to render.)

`Engine::fix_with_options` MUST return `Err(EngineError::DeadlineExceeded { partial_lint: LintResult })` when the deadline expires. The `partial_lint` carries the diagnostics produced before abort so callers can render the partial state without re-running the lint pass; no `FixResult` is constructed when the deadline expires.

### R5 — `EngineError::DeadlineExceeded` variant

`marque-engine::EngineError` (existing enum at `crates/engine/src/errors.rs`) MUST gain a `DeadlineExceeded { partial_lint: LintResult }` variant. The enum MUST remain `#[non_exhaustive]` (already is, per gap-register row #8 closure pattern). Display impl: `"engine deadline exceeded after processing N/M candidates"`. Error::source returns `None`.

### R6 — Per-surface wiring (CLI / server / WASM / batch)

Each surface MUST plumb the deadline through to `lint_with_options` / `fix_with_options`:

- **CLI**: `--deadline <humantime>` flag. `0` rejected as `EX_USAGE` (64). Truncated lint renders normally + final stderr warning. `DeadlineExceeded` from fix exits `EX_TEMPFAIL` (75) with partial diagnostics on stderr.
- **Server**: `X-Marque-Deadline` header (humantime), capped by `MARQUE_MAX_DEADLINE` env var (default 60 s). Per-endpoint default 30 s when absent. Out-of-range header returns `400 Bad Request`. Truncated lint returns 200 with `Marque-Truncated: true` response header. `DeadlineExceeded` from fix returns 504 with partial-lint body.
- **WASM**: `deadline_ms: f64` on the JS-side options. Internal conversion to `Instant::now() + Duration::from_millis(...)`. Constitution III WASM-restriction analysis added to the WASM crate's `lib.rs` doc comment.
- **BatchEngine**: new `BatchOptions { per_doc_deadline: Option<Duration> }` parameter on `lint_many_with_options` / `fix_many_with_options`. Per-doc `EngineError::DeadlineExceeded` lands as `BatchError::DocumentDeadlineExceeded { partial_lint }` with matching `is_deadline_exceeded()` predicate.

### R7 — Test coverage

The implementation MUST land with at least the following tests:

1. **Unit (`crates/engine/tests/deadline.rs`)**:
   - `lint_with_already_expired_deadline_returns_immediately_truncated`
   - `lint_truncates_mid_document_at_deadline_boundary`
   - `lint_with_generous_deadline_runs_to_completion_no_truncation`
   - `fix_with_already_expired_deadline_returns_DeadlineExceeded`
   - `fix_with_deadline_during_apply_loop_returns_DeadlineExceeded_with_partial_lint`
   - `LintOptions::default_yields_no_deadline`
   - `Engine::lint_calls_lint_with_options_with_default_options` (regression on the back-compat shim)
2. **Server (`crates/server/tests/http.rs`)**:
   - `header_driven_deadline_truncates_lint_response`
   - `out_of_range_deadline_header_returns_400`
   - `lint_without_header_uses_endpoint_default`
   - `fix_deadline_exceeded_returns_504_with_partial_lint_body`
3. **CLI (`marque/tests/cli.rs` or equivalent)**:
   - `cli_deadline_zero_exits_with_EX_USAGE`
   - `cli_deadline_truncates_check_output_with_warning`
   - `cli_deadline_fix_exits_EX_TEMPFAIL`
4. **WASM (`crates/wasm/tests/parity.rs` extension)**:
   - `wasm_deadline_ms_truncates_lint_output_byte_identically_to_native_cli`
5. **Batch (`crates/engine/tests/batch_deadline.rs`)**:
   - `batch_per_doc_deadline_isolates_one_slow_doc_from_rest`
   - `batch_deadline_exceeded_lands_as_per_doc_BatchError_variant`
6. **Bench (`crates/engine/benches/deadline_overhead.rs`, new)**:
   - `lint_no_deadline_vs_deadline_1h` — overhead must be in noise (≤ 2% on 10 KB corpus median).
   - Asserts SC-001 (interactive p95 ≤ 16 ms) is not regressed by the per-candidate clock check.

### R8 — Whitepaper updates

`docs/security/WHITEPAPER.md` MUST update:

- §9.7 flips from `[NON-GOAL]` to `[LANDED]`. New body describes the cooperative-cancellation model, granularity, and asymmetric response shape.
- §10.2 server section updated with the `X-Marque-Deadline` / `MARQUE_MAX_DEADLINE` controls.
- §10.3 WASM section updated to confirm Constitution III compliance.
- Gap register row #7 struck through with PR / commit reference.
- Appendix C changelog entry added.

## Constitution check

- **Principle I (uncompromising performance)**: The `Instant::now()` check is ~10 ns; per-candidate granularity puts that against ~0.16 ms median per-candidate work, so the overhead is in noise. R7 includes a Criterion bench (`deadline_overhead.rs`) that asserts no SC-001 regression. ✓
- **Principle II (zero-copy streaming core)**: `LintOptions` is a `&'_ LintOptions` reference; no heap allocation introduced on the hot path. `Instant` is `Copy`. ✓
- **Principle III (format-agnostic core / WASM safety)**: `LintOptions` is a pure-data struct; no I/O. Lives in `marque-rules`, which is WASM-safe. WASM accepts `deadline_ms: f64` and converts internally; this is a runtime config that does not introduce a new recognizer codepath or alter posteriors, so it satisfies the constitution's WASM runtime-config restriction. ✓
- **Principle IV (two-layer rule architecture)**: No rule-layer changes; deadline enforcement lives in the engine's loop, not in `Rule::check`. Rules remain stateless. ✓
- **Principle V (audit-first compliance)**: Asymmetric design. `fix` returns `Err(DeadlineExceeded)` rather than a partial `FixResult`, so a deadline-hit during compliance-output construction cannot ship a silently-incomplete audit stream. Partial `LintResult` is exposed inside the error variant for renderer use. ✓
- **Principle VI (dataflow pipeline model)**: Per-candidate granularity sits exactly at the scanner→parser→rule boundary, which is already the engine's natural phase boundary. No phase-collapsing. ✓
- **Principle VII (crate discipline)**: `LintOptions` lands in `marque-rules` (the dependency root); `EngineError::DeadlineExceeded` and the wiring land in `marque-engine`. No cycles. ✓
- **Principle VIII (authoritative source fidelity)**: N/A — this is an engine-infrastructure feature, not a grammar feature. No CAPCO citations apply.

## Acceptance criteria

The implementation closes #139 when:

1. `Engine::lint_with_options` and `Engine::fix_with_options` are public API surfaces in `marque-engine`.
2. `LintOptions` is `#[non_exhaustive]`, derives `Default + Clone`, and lives in `marque-rules`.
3. `EngineError::DeadlineExceeded { partial_lint }` exists.
4. Per-candidate and per-fix deadline checks are wired in `Engine::lint_with_options` and `Engine::fix_inner`.
5. CLI, server, WASM, and BatchEngine surfaces all expose the deadline.
6. All R7 tests pass; the deadline-overhead bench (R7 #6) passes the no-regression gate against the SC-001 baseline.
7. Whitepaper §9.7, §10.2, §10.3 updated; gap row #7 struck through; Appendix C entry added.
8. `cargo test --workspace --no-fail-fast` and `cargo clippy --workspace --all-targets -- -D warnings` clean.

## Open questions for the author (please confirm before plan.md / tasks.md land)

- **Q1**: Confirm D1 — cooperative cancellation, per-candidate + per-fix granularity. (Recommended: yes)
- **Q2**: Confirm D2 — `LintOptions` struct, `lint_with_options` shim. (Recommended: yes)
- **Q3**: Confirm D3 — asymmetric response (truncated `LintResult` for lint, `Err(DeadlineExceeded)` for fix). Or do you want symmetric (truncated everywhere)? (Recommended: asymmetric for compliance-integrity reasons)
- **Q4**: Confirm D4 surface defaults: server endpoint default 30 s, server cap 60 s, CLI no default. Adjust as needed.
- **Q5**: Should `LintOptions` carry a `started_at: Option<Instant>` for downstream timing-budget composition (e.g., a server handler passing its remaining budget to the engine)? Or is that overengineering — let the caller compute `started_at + budget` themselves and pass the resulting `Instant`? (My prior: the latter; keep `LintOptions` minimal.)
- **Q6**: Should the server `X-Marque-Deadline` header use a humantime string (`"30s"`) or an integer milliseconds (`"30000"`)? Humantime is more readable; integer is more universal. (My prior: humantime, matching the CLI for consistency.)
