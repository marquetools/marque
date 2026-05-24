<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Masking-pin inventory (engine-refactor-006 PR 0)

**Source**: `docs/plans/2026-05-02-engine-refactor-consolidated.md` § 6 (masking-pin discipline rules); `specs/006-engine-rule-refactor/spec.md` FR-039; `specs/006-engine-rule-refactor/tasks.md` T009.

**Lint enforcement**: `tools/masking-pin-lint/` walks every workspace test surface — workspace-root `tests/`, `crates/*/tests/`, and every top-level workspace member's `tests/` (including `marque/tests/` and any future top-level workspace member) — for `with_recognizer(...StrictRecognizer...)` calls and rejects any site that lacks a `// MASKING-PIN: tracks #NNN — <reason>` or `// INTENTIONAL-STRICT: <reason>` comment within 5 lines of the call. The workspace-member walk uses each directory's `Cargo.toml` as a marker, so new top-level members are picked up automatically without a static allow-list.

**Disposition at PR 0 HEAD**: 2 masking pins, 2 intentional-strict pins (in `tests/`), 1 documentary intentional-strict pin (in `benches/`, out of FR-039 lint scope).

## Masking pins (close-on-PR per FR-039 rule 5)

### `crates/engine/tests/corpus_accuracy.rs:50`

- **Tracks**: #258 — decoder prose null-hypothesis priors not yet baked.
- **Marker location**: line 49 (one line above the call at line 50; well within the 5-line window).
- **Marker text** (verbatim):
  ```
  // MASKING-PIN: tracks #258 — decoder prose null-hypothesis priors not yet baked (#258); pinning to strict avoids decoder mis-fires on prose corpus until PR 8 lands
  ```
- **Call site** (verbatim, line 50):
  ```rust
  .with_recognizer(Arc::new(StrictRecognizer::new()))
  ```
- **Closes at**: PR 8 (tasks.md T130 deletes the pin and adds a regression test that must fail on pre-fix HEAD).
- **Issue state at inventory time**: `open`. Verified via `tools/masking-pin-lint`'s GitHub-API probe at PR 0 HEAD; the cache snapshot at `tools/masking-pin-lint/cache/marquetools__marque__258.json` carries `state="open"`. PR 8 (tasks.md T130) closes the issue and removes this pin per FR-039 rule 5.
- **Why this is a masking pin**: the file's `make_engine()` helper (lines 42-51) is the construction surface for SC-002 / SC-003 corpus-accuracy assertions. The strict path bypasses the prose-context decoder bug — closing the pin without #258's per-token prose null-hypothesis priors landing first would re-expose the corpus_accuracy assertion to decoder mis-fires on shapes like `Notwithstanding (s) the early prevalence` (Federalist-corpus prose), which the doc comment at lines 30-41 enumerates explicitly.
- **FR-039 format conformance**: marker text matches the required `// MASKING-PIN: tracks #NNN — <reason>` shape.

### `crates/engine/tests/core_error_isolation.rs:93`

- **Tracks**: #257 — decoder canonicalization leaks input bytes into `AppliedFix.proposal.replacement`.
- **Marker location**: line 92 (one line above the call at line 93).
- **Marker text** (verbatim):
  ```
  // MASKING-PIN: tracks #257 — decoder canonicalization leaks input bytes into AppliedFix (#257); strict path isolates the test from that leak channel until PR 3c closes the carve-out
  ```
- **Call site** (verbatim, line 93):
  ```rust
  .with_recognizer(Arc::new(StrictRecognizer::new()))
  ```
- **Closes at**: PR 3c (tasks.md T058).
- **Issue state at inventory time**: `open`. Verified via `tools/masking-pin-lint`'s GitHub-API probe at PR 0 HEAD; the cache snapshot at `tools/masking-pin-lint/cache/marquetools__marque__257.json` carries `state="open"`. PR 3c (tasks.md T058) closes the issue and removes this pin per FR-039 rule 5.
- **Why this is a masking pin**: the file (`test_engine()` helper at lines 85-94) gates the `CoreError`-shaped content-isolation channel. `CoreError` is produced only on the strict path (per the doc comment at lines 77-84); the decoder fallback uses a different error shape entirely. To exercise the *named* leak channel cleanly, the test pins to `StrictRecognizer` rather than letting the default dispatcher also exercise decoder-side leak channels (which are real, but separately scoped and out of this file's gate). Once #257 closes the decoder-side carve-out at PR 3c, the masking justification dissolves and the pin must come down.
- **FR-039 format conformance**: marker text matches the required `// MASKING-PIN: tracks #NNN — <reason>` shape.

## Intentional-strict pins (`tests/` — in FR-039 lint scope)

### `crates/engine/tests/decoder_dispatch.rs:30`

- **Site**: inside `fn build_strict_engine() -> Engine` (helper, lines 28-31).
- **Marker location**: line 29 (one line above the call at line 30).
- **Marker text** (verbatim):
  ```
  // INTENTIONAL-STRICT: this helper exists specifically to construct an engine with the decoder suppressed; the test family asserts strict-path behavior in contrast to the default dispatcher
  ```
- **Call site** (verbatim, line 30):
  ```rust
  build_engine().with_recognizer(Arc::new(StrictRecognizer::new()))
  ```
- **Reason**: this helper exists specifically to construct an engine with the decoder suppressed. The `decoder_dispatch` test family asserts strict-only behavior — most prominently `explicit_strict_recognizer_never_invokes_the_decoder` (line 47) which verifies that an explicit `StrictRecognizer` install via `Engine::with_recognizer` suppresses the decoder fallback even on inputs (`(SERCET//NOFORN)`) the dispatcher would otherwise canonicalize.
- **Why not a masking pin**: no open issue is being masked; the pin documents a deliberate strict-only behavior under test, contrasting against the default dispatcher exercised by the sibling `build_engine()` helper at line 21.
- **FR-039 format conformance**: marker text matches the required `// INTENTIONAL-STRICT: <reason>` shape.

### `crates/engine/tests/audit.rs:121`

- **Site**: inside `fn test_engine() -> Engine` (helper, lines 105-122).
- **Marker location**: line 120 (one line above the call at line 121).
- **Marker text** (verbatim):
  ```
  // INTENTIONAL-STRICT: audit-trail tests pin the strict recognizer because the audit invariants tested here apply to the strict-path AppliedFix shape; decoder-path differences are exercised in decoder_diagnostic.rs
  ```
- **Call site** (verbatim, line 121):
  ```rust
  .with_recognizer(std::sync::Arc::new(marque_engine::StrictRecognizer::new()))
  ```
- **Reason**: audit-trail tests pin the strict recognizer because the audit-v2 invariants tested in T052 (the `audit_v2_strict_path_invariants` test family) apply to the strict-path `AppliedFix` shape — `confidence.recognition == 1.0_f32`, `confidence.runner_up_ratio == None`, `confidence.features.is_empty()`. The doc comment immediately above the helper (lines 106-112) explicitly notes that the engine's `StrictOrDecoderRecognizer` default would still hold the invariant on today's fixture set because no fixture trips the decoder, but a future fixture that does would silently weaken the assertion if the test relied on the default. Decoder-path audit differences are exercised in `decoder_diagnostic.rs`.
- **Why not a masking pin**: no open issue is being masked; pinning is intrinsic to what the test asserts.
- **FR-039 format conformance**: marker text matches the required `// INTENTIONAL-STRICT: <reason>` shape.

## Documentary intentional-strict pins (`benches/` — out of FR-039 lint scope)

### `crates/engine/benches/lint_latency.rs:80`

- **Site**: inside `fn lint_latency_benchmark(c: &mut Criterion)` (lines 71-85).
- **Marker location**: line 79 (one line above the call at line 80).
- **Marker text** (verbatim):
  ```
  // INTENTIONAL-STRICT: SC-001 interactive-latency bench pins the strict recognizer to measure the latency floor; the dispatcher's decoder fallback is benchmarked separately in decoder_10kb_rel_to_invariant.rs
  ```
- **Call site** (verbatim, line 80):
  ```rust
  .with_recognizer(Arc::new(StrictRecognizer::new()));
  ```
- **Reason**: the SC-001 interactive-latency bench (`lint_10kb`) pins the strict recognizer to measure the latency floor. The dispatcher's decoder fallback is benched separately in `decoder_10kb_rel_to_invariant.rs` so the two costs are isolated and a regression in either surface is attributable.
- **Note**: FR-039's lint scope is `tests/` only. The marker is documentary — `tools/masking-pin-lint/` will not visit this file at PR 0 HEAD. If the lint scope is later expanded to `benches/`, the marker already conforms; no comment update would be required.

## Verification at PR 0 HEAD

After PR 0 lands (with `tools/masking-pin-lint/` operational), verify by running the lint in CI mode:

```sh
cargo run --manifest-path tools/masking-pin-lint/Cargo.toml --release -- \
    --workspace-dir . --mode ci
```

Expected output: zero errors. The 4 in-scope sites (2 MASKING-PIN + 2 INTENTIONAL-STRICT in `tests/`) all carry their required marker comment within 5 lines of the call.

For the issue-state cross-check (FR-039 rule 5), each MASKING-PIN's tracked issue should be queried via the GitHub API (or the cache fallback per the lint's D11 cache rule). At PR 0 HEAD both #257 and #258 are expected to report `state=open`; verify before the PR merges:

```sh
gh issue view 257 --repo marquetools/marque -q '.state'
gh issue view 258 --repo marquetools/marque -q '.state'
```

**Cross-reference**: FR-039 close-on-PR rule 5 binds the MASKING-PIN sites to PRs that close their tracked issues:

- When #257 closes (at PR 3c merge per tasks.md T058), the pin at `crates/engine/tests/core_error_isolation.rs:93` and its marker at line 92 must come down in the same PR.
- When #258 closes (at PR 8 merge per tasks.md T130), the pin at `crates/engine/tests/corpus_accuracy.rs:50` and its marker at line 49 must come down in the same PR, and the PR must add a regression test that fails on pre-fix HEAD.

Failing to remove a stale pin causes the lint to fail the next PR's build (the lint queries each tracked issue's state and rejects sites pinned against a closed issue).
