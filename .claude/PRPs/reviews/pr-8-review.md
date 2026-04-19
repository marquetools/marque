<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR Review: #8 — feat: Phase 5 & 6 — configurable severity, corrections map, WASM web worker

**Reviewed**: 2026-04-12
**Author**: bashandbone (Adam Poulemanos)
**Branch**: 001-marque-mvp → main
**Decision**: APPROVE

## Summary
Solid implementation spanning two spec phases (US3 + US4). The Copilot commit (4dd20f1) improves the pre-scanner corrections from O(n*m) to O(n+m) via AhoCorasick automaton and cleanly resolves the workspace dependency approach. All 239 tests pass, clippy clean, wasm32 compiles. No CRITICAL or HIGH findings.

## Findings

### CRITICAL
None

### HIGH
None

### MEDIUM

**M-1**: AhoCorasick automaton rebuilt on every `lint()` call
- **File**: `crates/engine/src/engine.rs:217`
- **Description**: The `AhoCorasick::new(&patterns)` call inside `lint()` builds a new automaton from the corrections map on every lint invocation. For repeated calls with the same config (e.g., batch processing or WASM hot path), this is wasteful — the corrections map doesn't change after engine construction.
- **Impact**: Performance — automaton build is O(total pattern bytes). For small corrections maps this is negligible; for large ones it could be measurable.
- **Suggested fix**: Build the automaton once in `Engine::with_clock()` alongside `corrections_arc` and store as `Option<AhoCorasick>`. This is an optimization, not a correctness issue — defer to Phase 7 perf work.

**M-2**: `AhoCorasick::new` error silently skipped
- **File**: `crates/engine/src/engine.rs:217`
- **Description**: `if let Ok(ac) = AhoCorasick::new(&patterns)` silently swallows build errors. If the automaton fails to build (e.g., pattern too large, memory exhaustion), the pre-scanner pass is silently skipped — no diagnostics, no warning.
- **Impact**: Silent data loss in an edge case. In practice, AhoCorasick::new only fails on truly pathological inputs, but silent failure is inconsistent with the engine's error philosophy.
- **Suggested fix**: Log a `tracing::warn!` on the `Err` branch, or propagate the error. LOW urgency since the failure mode is extremely unlikely with real corrections maps.

### LOW

**L-1**: `corrections_map.rs` test file included in PR diff but unchanged
- **File**: `crates/capco/tests/corrections_map.rs`
- **Description**: This file appears in the PR diff but the changes are from the previously merged Phase 5 PR #6, not from the Phase 5→6 commits in this PR. This is due to the branch carrying forward from the last merge base. Not a code issue — just noise in the diff.

**L-2**: `marque-capco` still referenced via `path =` in CLI and server
- **File**: `marque/Cargo.toml:18`, `crates/server/Cargo.toml:18`
- **Description**: `marque-capco = { path = "../crates/capco" }` uses a path dep instead of `workspace = true`. Inconsistent with other workspace deps. Not a bug — both resolve to the same crate.
- **Suggested fix**: Change to `workspace = true` for consistency. Low priority.

## Validation Results

| Check | Result |
|---|---|
| Clippy | Pass (zero warnings) |
| Tests | Pass (239/239) |
| Build | Pass |
| wasm32 compilation | Pass |
| Fmt | Pass |

## Files Reviewed
| File | Change |
|------|--------|
| `Cargo.toml` | Modified — workspace marque-engine default-features |
| `Cargo.lock` | Modified — console_error_panic_hook added |
| `benches/wasm_latency.md` | Added — SC-001b measurement doc |
| `crates/capco/tests/corrections_map.rs` | Modified — Phase 5 tests (from prior merge base) |
| `crates/engine/Cargo.toml` | Modified — batch feature flag, aho-corasick dep |
| `crates/engine/src/engine.rs` | Modified — AhoCorasick pre-scanner refactor |
| `crates/engine/src/lib.rs` | Modified — cfg(feature = "batch") gate |
| `crates/server/Cargo.toml` | Modified — features = ["batch"] |
| `crates/wasm/Cargo.toml` | Modified — cleaned deps, console_error_panic_hook |
| `crates/wasm/examples/harness.html` | Added — interactive WASM harness |
| `crates/wasm/src/lib.rs` | Modified — rewritten for NDJSON parity |
| `crates/wasm/tests/native_parity.rs` | Added — SC-008 parity tests |
| `crates/wasm/tests/no_io.rs` | Added — FR-013 dep audit |
| `marque/Cargo.toml` | Modified — features = ["batch"] |
