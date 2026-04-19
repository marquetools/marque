<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR Review: #9 — feat: Phase 5, 6, 7 — configurable severity, WASM web worker, polish & quality gates

**Reviewed**: 2026-04-12
**Author**: bashandbone (Adam Poulemanos)
**Branch**: 001-marque-mvp → main
**Decision**: APPROVE

## Summary
Clean implementation of Phase 7 quality gates on top of previously-merged Phase 5/6 work. All prior HIGH/MEDIUM review findings addressed. Copilot's follow-up changes (portable Python parsing, exact-size input padding, actual-length throughput reporting) are correct. 246 tests pass, clippy clean, no security issues.

## Findings

### CRITICAL
None

### HIGH
None

### MEDIUM
None

### LOW

**L-1**: `baseline.json` uses misleading `p50_us/p95_us/p99_us` key names
- **File**: `benches/baseline.json`
- **Description**: Values are criterion CI bounds (lower/mean/upper), not percentile distribution samples. The bench-check.sh script has a comment acknowledging this, but the JSON keys themselves mislead.
- **Suggested fix**: Rename to `lower_ci_us/mean_us/upper_ci_us` or add a `_note` field.

**L-2**: `BenchmarkId::from_parameter(size)` label mismatch for 1KB scaling point
- **File**: `crates/engine/benches/linear_scaling.rs:48`
- **Description**: The 1KB point is labeled "1000" but actual input is ~828 bytes after block-aligned truncation. Throughput calculation is correct (uses `input.len()`), only the axis label is off.
- **Suggested fix**: Use `BenchmarkId::from_parameter(input.len())` for accurate labels.

**L-3**: `BASELINE_P95` interpolated into Python without numeric validation
- **File**: `scripts/bench-check.sh:92`
- **Description**: If `baseline.json` were corrupted, a non-numeric value could inject into the Python `-c` argument. Low risk (dev-tool, version-controlled file).
- **Suggested fix**: Add `[[ "$BASELINE_P95" =~ ^[0-9]+$ ]]` guard.

**L-4**: `TEST-WASM-42` passes SC-006 scanner by coincidence of raw-string parsing
- **File**: `crates/wasm/tests/native_parity.rs:348`
- **Description**: The value avoids detection because `r#"` parsing extracts `{` (length 1 < 5), not because it's allow-listed. Reformatting to a regular string literal would trigger a false positive.
- **Suggested fix**: Add `TEST-WASM-42` to `ALLOWED_SENTINELS` in `no_classifier_id_in_commits.rs`.

## Validation Results

| Check | Result |
|---|---|
| Clippy | Pass (zero warnings) |
| Tests | Pass (246/246) |
| Build | Pass |
| Fmt | Pass |

## Files Reviewed
| File | Change |
|------|--------|
| `CLAUDE.md` | Modified — MVP status update |
| `Cargo.toml` | Modified — fuzz exclude |
| `benches/baseline.json` | Added — performance baseline |
| `crates/capco/tests/rules_us1.rs` | Modified — C001 fixture skip |
| `crates/engine/benches/linear_scaling.rs` | Modified — implemented scaling benchmark |
| `crates/engine/benches/lint_latency.rs` | Modified — implemented latency benchmark |
| `crates/engine/fuzz/Cargo.toml` | Added — fuzz workspace |
| `crates/engine/fuzz/fuzz_targets/lint.rs` | Added — fuzz target |
| `crates/engine/tests/corpus_accuracy.rs` | Added — accuracy harness |
| `crates/wasm/Cargo.toml` | Modified — release-wasm wasm-opt |
| `crates/wasm/tests/native_parity.rs` | Modified — full corpus parity |
| `scripts/bench-check.sh` | Added — regression gate |
| `scripts/check.sh` | Modified — bench gate integration |
| `tests/corpus/invalid/corrections_map_typo*.{txt,expected.json}` | Added — C001 fixtures (6 files) |
