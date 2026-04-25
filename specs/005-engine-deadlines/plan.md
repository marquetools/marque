<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Plan: Per-Document Deadlines

**Status**: Approved — Phase 1 ready to start (all Q1–Q7 resolved 2026-04-25 in PR #159 review; see `spec.md` § Open questions resolution)
**Spec**: [spec.md](spec.md)
**Branch**: `feat/engine-deadlines` (proposed)

## Phasing

The work breaks into four phases. Each is independently mergeable and reviewable.

### Phase 1 — Foundational types (zero behavior change)

Land the type surface without wiring it through. The workspace stays green; no caller-visible behavior changes.

- New `LintOptions { deadline: Option<Instant> }` and `FixOptions { deadline: Option<Instant>, threshold_override: Option<f32> }` structs in `crates/engine/src/options.rs` (new module), re-exported from `crates/engine/src/lib.rs`. Both `#[non_exhaustive]`, `Default + Clone + Debug`.
- New `EngineError` enum in `crates/engine/src/errors.rs` (alongside the existing `EngineConstructionError`, which stays unchanged). Variants: `DeadlineExceeded { partial_lint: LintResult }`, `InvalidThreshold(InvalidThreshold)`. `#[non_exhaustive]`. The existing standalone `InvalidThreshold` struct is preserved for `Engine::fix_with_threshold`'s public signature; `From<InvalidThreshold> for EngineError` is provided.
- New fields on `LintResult` (currently in `crates/engine/src/output.rs`, **not** `marque-rules`): `truncated: bool`, `candidates_processed: usize`, `candidates_total: usize`. Default values preserve back-compat (every existing `LintResult` carries `truncated: false` + zeroed counts). Add `#[non_exhaustive]` to the struct (currently absent) so future additions are not semver-breaking.
- `Engine::lint_with_options(&[u8], &LintOptions) -> LintResult` and `Engine::fix_with_options(&[u8], FixMode, &FixOptions) -> Result<FixResult, EngineError>` signatures with thin bodies that delegate to the existing `lint` / `fix_with_threshold` paths, ignoring `opts.deadline`. `fix_with_options` honors `opts.threshold_override` from day one (it's a non-deadline concern that fits naturally into the new structure).
- Existing `Engine::lint` / `Engine::fix` rewired as back-compat shims over the `_with_options` variants. `Engine::fix_with_threshold` keeps its existing `Result<FixResult, InvalidThreshold>` signature; internally it now calls `fix_with_options` and maps `EngineError::InvalidThreshold(it) → it`, with `unreachable!(...)` on `EngineError::DeadlineExceeded` (no caller of `fix_with_threshold` can set a deadline).
- Unit tests: `LintOptions::default()` yields `None` deadline; `Engine::lint(...)` produces identical `LintResult` to `Engine::lint_with_options(..., &LintOptions::default())`; `Engine::fix_with_threshold` produces identical results to its pre-spec behavior across the InvalidThreshold + Ok paths.

**Why this is its own phase**: every downstream call site (engine internals, CLI, server, WASM, batch, tests, benches) gets touched in Phase 2; reviewing the type surface in isolation first keeps Phase 2's diff focused on the deadline plumbing.

### Phase 2 — Engine-side cooperative cancellation

Wire the actual deadline checks into the engine's loops.

- Per-candidate check at the top of `Engine::lint_with_options`'s candidate loop (currently `crates/engine/src/engine.rs:352`).
- Pre-pass check at the start of `lint_with_options` (returns `truncated: true, _processed: 0, _total: 0` immediately on past `Instant`).
- Pre-fix-loop check in `Engine::fix_inner` returning `Err(DeadlineExceeded { partial_lint })`.
- Per-fix-application check inside `fix_inner`'s for-loop. Same abort path.
- `LintResult.candidates_processed / candidates_total` populated correctly. `candidates_total` MUST be set before any rule pass (so it reflects the scanner's total, not the partial count).
- `Engine::fix` (back-compat shim) calls `expect("...")` since `LintOptions::default().deadline = None`. Document why with a `// SAFETY-equivalent` comment.

Tests for Phase 2 (per spec.md R7):

- `lint_with_already_expired_deadline_returns_immediately_truncated`
- `lint_truncates_mid_document_at_deadline_boundary`
- `lint_with_generous_deadline_runs_to_completion_no_truncation`
- `fix_with_already_expired_deadline_returns_DeadlineExceeded`
- `fix_with_deadline_during_apply_loop_returns_DeadlineExceeded_with_partial_lint`

Plus the deadline-overhead Criterion bench (`crates/engine/benches/deadline_overhead.rs`) gating SC-001 regressions.

### Phase 3 — Surface wiring (CLI / server / WASM / batch)

Each surface lands as its own commit-set so reviewers can cleanly trace "what changed in marque-cli" vs "what changed in marque-server."

- **3a (CLI)**: `--deadline <humantime>` flag, `EX_USAGE` for `0`, truncated-output rendering, `EX_TEMPFAIL` for fix DeadlineExceeded. CLI tests in `marque/tests/cli.rs`.
- **3b (Server)**: `X-Marque-Deadline` header parser (unsigned-integer milliseconds via `str::parse::<u64>()` — no humantime dep needed for the server), `MARQUE_MAX_DEADLINE` env var resolution, per-endpoint default, `400` for out-of-range / non-integer / overflow, `Marque-Truncated` response header, `504` for fix DeadlineExceeded. Tests in `crates/server/tests/http.rs`.
- **3c (WASM)**: `deadline_ms: f64` field on JS-side options. wasm-bindgen wiring. Constitution III analysis added to crate doc comment. Tests in `crates/wasm/tests/parity.rs`.
- **3d (BatchEngine)**: `BatchOptions` struct, `_with_options` variants, `BatchError::DocumentDeadlineExceeded { partial_lint }` variant, `is_deadline_exceeded()` predicate. Tests in `crates/engine/tests/batch_deadline.rs` (new file).

### Phase 4 — Whitepaper updates + gap closure

- §9.7 rewritten: `[NON-GOAL]` → `[LANDED]`. New body describes the cooperative-cancellation model, granularity, asymmetric response shape, per-surface wiring.
- §10.2 server section: add `X-Marque-Deadline` / `MARQUE_MAX_DEADLINE` controls alongside the body-limit pattern.
- §10.3 WASM section: confirm Constitution III compliance for `deadline_ms`.
- Gap register row #7 struck through.
- Appendix C v0.13 entry.

## Sequence

```
Phase 1  ──→  Phase 2  ──→  Phase 3a  ──┐
                       ─→  Phase 3b  ──┤
                       ─→  Phase 3c  ──┼──→  Phase 4
                       ─→  Phase 3d  ──┘
```

Phases 3a / 3b / 3c / 3d are parallelizable once Phase 2 lands. They can land as separate PRs or as a single PR with one commit per surface; the choice is review-bandwidth-driven.

## Risk register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Per-candidate `Instant::now()` regresses SC-001 | Low | High (Constitution I violation) | R7 #6 deadline-overhead bench gates the merge; abort if median overhead > 2% |
| Truncated `LintResult` confuses downstream consumers that don't read the new fields | Medium | Medium (correctness for them, not us) | New fields are additive; default values preserve back-compat. `#[non_exhaustive]` is added to `LintResult` as part of Phase 1 (it is currently absent). Document the additions in CHANGELOG and call out the `#[non_exhaustive]` change for consumers using struct-update syntax. |
| Server `MARQUE_MAX_DEADLINE` cap interacts with body-size cap unexpectedly | Low | Low | Document the precedence order in §10.2; deadline-cap is enforced first (parsing the header) before body is read |
| `humantime` parsing breadth (CLI side) | Low | Low | `humantime::parse_duration` accepts a wide range of forms (`30s`, `2m`, `1h30m`); restrict to a documented subset in `marque-cli`'s `--deadline` value-parser if a future test surface drift. Server uses plain `str::parse::<u64>()` for milliseconds, so this risk is CLI-only. |
| `deadline_ms: f64` overflow on huge values from JS | Low | Low (NaN / negative produces immediate-truncate behavior, which is safe) | Validate at the wasm shim: `deadline_ms.is_finite() && deadline_ms >= 0.0` before converting |
| `BatchEngine`-level deadline composes incorrectly with row/byte semaphore wait | Low | Medium | Per-doc deadline starts at `Instant::now()` *after* the document acquires its permit, not before. Document this in `BatchOptions::per_doc_deadline`'s doc comment. |

## Constitution check (re-affirm after Phase 2 lands)

The detailed analysis is in `spec.md` § Constitution check. The Phase 2 review should re-confirm:

- SC-001 bench has not regressed (the load-bearing Principle I gate)
- No heap allocation introduced on the hot path (the load-bearing Principle II gate)
- WASM target compiles and the `deadline_ms` shim works (Principle III)
- The asymmetric response shape is honored — `fix_with_options` returns `Err(DeadlineExceeded)` not a partial `FixResult` (Principle V)

## Out-of-scope follow-ups (capture for future)

- **Cancellation token**: a `tokio::sync::CancellationToken` field on `LintOptions` would let server handlers proactively abort on client disconnect. Defer to a future spec; the deadline alone is sufficient for the gap.
- **Memory budget**: a `max_memory_bytes` field would gate pathological allocations. Separate concern; not coupled to time.
- **Per-rule deadline**: granular timing per `Rule::check` invocation could detect specific rule pathologies. The current rule-panic-isolation harness already catches infinite loops via `catch_unwind`; a deadline at this level would be strictly more granular but also more invasive. Defer.
- **Whole-batch deadline**: `BatchEngine` currently only supports per-doc deadlines. A whole-run SLA would require new bookkeeping. Defer.
