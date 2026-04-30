<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Plan: Phase 7 — Polish & Cross-Cutting Concerns (T067–T074)

## Summary
Performance verification, corpus accuracy harness, fuzz target, WASM parity scaling, and final quality gates that span all four user stories. This phase validates every success criterion (SC-001 through SC-008) with measured evidence and establishes regression detection for ongoing development.

## User Story
As a marque contributor,
I want measurable evidence that every success criterion passes,
So that the MVP can ship with confidence.

## Problem → Solution
Benchmarks are stubbed, corpus accuracy is tested per-fixture but not aggregated with per-rule thresholds, WASM parity covers minimum fixtures (not full corpus), and no fuzz target exists → All SC gates pass with captured baselines, regression detection scripts, full-corpus harnesses, and a cargo-fuzz target.

## Metadata
- **Complexity**: Large
- **Source PRD**: `specs/001-marque-mvp/`
- **PRD Phase**: Phase 7 — Polish & Cross-Cutting Concerns
- **Estimated Files**: 12-15 files created/updated

---

## UX Design

N/A — internal change. No user-facing UX transformation.

---

## Mandatory Reading

| Priority | File | Lines | Why |
|---|---|---|---|
| P0 | `crates/engine/benches/lint_latency.rs` | all | Stub to implement |
| P0 | `crates/engine/benches/linear_scaling.rs` | all | Stub to implement |
| P0 | `crates/engine/src/engine.rs` | 1-110 | Engine struct, constructor, CachedAhoCorasick |
| P0 | `crates/engine/src/engine.rs` | 149-353 | lint() implementation |
| P0 | `crates/test-utils/src/lib.rs` | all | Corpus loader utilities |
| P1 | `crates/wasm/tests/native_parity.rs` | all | T061 parity harness to extend for T070 |
| P1 | `crates/capco/src/rules.rs` | 33-60 | CapcoRuleSet registration (10 rules) |
| P1 | `crates/engine/Cargo.toml` | all | Bench config, dev-deps |
| P2 | `marque/src/main.rs` | all | CLI entry point for quickstart validation |
| P2 | `marque/src/render.rs` | all | NDJSON rendering for parity checks |
| P2 | `specs/001-marque-mvp/quickstart.md` | all | End-to-end validation script |
| P2 | `scripts/check.sh` | all | Current CI script to extend |
| P2 | `CLAUDE.md` | all | Agent context to update (T071) |

## External Documentation

| Topic | Source | Key Takeaway |
|---|---|---|
| criterion benchmarks | crates.io/crates/criterion | `BenchmarkGroup::throughput(Throughput::Bytes(n))` for scaling; `criterion_group!` macros |
| cargo-fuzz | github.com/rust-fuzz/cargo-fuzz | `cargo +nightly fuzz init`, target in `fuzz/fuzz_targets/`, `libfuzzer_sys::fuzz_target!` |
| cargo-fuzz Cargo.toml | rust-fuzz docs | Separate `fuzz/Cargo.toml` workspace with `[[bin]]` entries, dep on parent crate |

---

## Patterns to Mirror

### BENCHMARK_STRUCTURE
```rust
// SOURCE: crates/engine/benches/lint_latency.rs:1-22
// Criterion bench skeleton — already scaffolded with criterion_group!/criterion_main!
use criterion::{Criterion, criterion_group, criterion_main};

fn lint_latency_benchmark(_c: &mut Criterion) {
    // TODO: implement
}

criterion_group!(benches, lint_latency_benchmark);
criterion_main!(benches);
```

### ENGINE_CONSTRUCTION
```rust
// SOURCE: crates/wasm/tests/native_parity.rs:74-84
fn engine_lint_to_ndjson(source: &[u8]) -> String {
    let engine = Engine::new(Config::default(), vec![Box::new(capco_rules())]);
    let result = engine.lint(source);
    // ...serialize diagnostics...
}
```

### CORPUS_LOADING
```rust
// SOURCE: crates/test-utils/src/lib.rs:37-50
pub fn fixtures_in(subdir: &str) -> Vec<PathBuf> {
    let dir = corpus_root().join(subdir);
    // ...read_dir, filter .txt, sort...
}
// Also: invalid_fixtures(), valid_fixtures(), prose_fixtures(), load_expected(), load_fixture()
```

### EXPECTED_JSON_FORMAT
```json
// SOURCE: tests/corpus/invalid/banner_abbrev.expected.json
{
  "diagnostics": [
    {"rule": "E001", "span": {"start": 16, "end": 18}}
  ]
}
```

### PARITY_TEST_PATTERN
```rust
// SOURCE: crates/wasm/tests/native_parity.rs:110-136
#[test]
fn lint_parity_invalid_fixtures() {
    let txt_files = txt_files_in(&corpus_dir().join("invalid"));
    assert!(txt_files.len() >= 10, "T061 requires >=10 corpus fixtures");
    for path in &txt_files {
        let source = load_fixture(path);
        let native_ndjson = engine_lint_to_ndjson(&source);
        let wasm_ndjson = marque_wasm::lint_native(text, None).unwrap();
        assert_eq!(native_ndjson, wasm_ndjson, "SC-008 parity failure on {}", ...);
    }
}
```

### CHECK_SCRIPT
```bash
# SOURCE: scripts/check.sh:1-24
#!/usr/bin/env bash
set -euo pipefail
cargo fmt --check
cargo clippy --workspace --benches -- -D warnings
# ... nextest or cargo test ...
```

---

## Files to Change

| File | Action | Justification |
|---|---|---|
| `crates/engine/benches/lint_latency.rs` | UPDATE | T067: implement p95 latency benchmark |
| `crates/engine/benches/linear_scaling.rs` | UPDATE | T068: implement scaling sweep benchmark |
| `benches/baseline.json` | CREATE | T067a: capture p50/p95/p99 baseline |
| `scripts/bench-check.sh` | CREATE | T067a: regression detection script |
| `scripts/check.sh` | UPDATE | T067a: integrate bench-check |
| `tests/corpus_accuracy.rs` | CREATE | T069: corpus accuracy harness (workspace-level integration test) |
| `crates/wasm/tests/native_parity.rs` | UPDATE | T070: extend to full corpus |
| `CLAUDE.md` | UPDATE | T071: reflect MVP completion status |
| `crates/engine/fuzz/Cargo.toml` | CREATE | T072a: fuzz target workspace |
| `crates/engine/fuzz/fuzz_targets/lint.rs` | CREATE | T072a: fuzz target implementation |
| `crates/engine/Cargo.toml` | UPDATE | T072a: add `arbitrary` feature if needed |

## NOT Building

- Incremental LMDB cache (v0.2 scope)
- `marque-extract` Kreuzberg integration (separate phase)
- Metadata CLI subcommand implementation (depends on extract)
- CI pipeline YAML (scripts only; CI wiring is ops, not code)
- `daachorse` WASM engine (MVP uses `aho-corasick` for both; T070 note about different engines is future scope)
- Auto-updating baselines (T067a: baseline re-capture is manual, reviewed commit)

---

## Step-by-Step Tasks

### Task 1: T067 — Lint Latency Benchmark (SC-001)
- **ACTION**: Implement the criterion benchmark in `crates/engine/benches/lint_latency.rs`
- **IMPLEMENT**:
  1. Build a ~10KB representative input by concatenating invalid corpus fixtures (or repeating a representative marking block with surrounding prose)
  2. Create `Engine::new(Config::default(), vec![Box::new(capco_rules())])`
  3. Use `c.bench_function("lint_10kb", |b| b.iter(|| engine.lint(black_box(&input))))`
  4. Input construction happens OUTSIDE the benchmark closure
  5. Do NOT assert p95 in the bench itself (criterion doesn't expose p95 programmatically in the bench function) — the assertion lives in `bench-check.sh` (T067a)
- **MIRROR**: BENCHMARK_STRUCTURE, ENGINE_CONSTRUCTION
- **IMPORTS**: `criterion::{Criterion, criterion_group, criterion_main, black_box}`, `marque_capco::capco_rules`, `marque_config::Config`, `marque_engine::Engine`
- **GOTCHA**: Engine construction is expensive (AhoCorasick build). Create it ONCE outside the bench closure, not per-iteration. Use `criterion::black_box` to prevent optimization of lint result.
- **VALIDATE**: `cargo bench -p marque-engine --bench lint_latency` runs and produces criterion output with timing data

### Task 2: T067a — Performance Regression Gate (SC-001a)
- **ACTION**: Capture baseline and create regression detection script
- **IMPLEMENT**:
  1. Run `cargo bench -p marque-engine --bench lint_latency -- --save-baseline mvp` to generate criterion data
  2. Create `benches/baseline.json` with manually recorded p50/p95/p99 from criterion output
  3. Create `scripts/bench-check.sh` that:
     - Runs `cargo bench -p marque-engine --bench lint_latency -- --output-format bencher 2>&1`
     - Parses the output for the mean time
     - Compares against baseline p95 from `benches/baseline.json`
     - Exits non-zero if p95 regresses by >10%
  4. Update `scripts/check.sh` to call `scripts/bench-check.sh` (gated on `--bench` flag so it's opt-in for regular dev, required in CI)
- **MIRROR**: CHECK_SCRIPT
- **IMPORTS**: N/A (shell scripts)
- **GOTCHA**: Criterion's `--output-format bencher` gives machine-parseable output. The baseline JSON is a manual snapshot — never auto-updated on green. Different machines have different performance; document the reference machine spec in the baseline file.
- **VALIDATE**: `bash scripts/bench-check.sh` exits 0 on current hardware

### Task 3: T068 — Linear Scaling Benchmark (SC-005)
- **ACTION**: Implement the criterion scaling sweep in `crates/engine/benches/linear_scaling.rs`
- **IMPLEMENT**:
  1. Define a representative marking block (~100 bytes) with a mix of valid and invalid markings
  2. For each size in `[1_000, 2_000, 5_000, 10_000, 20_000, 50_000, 100_000]`:
     - Repeat the block to fill the target size
     - Use `BenchmarkGroup` with `throughput(Throughput::Bytes(size as u64))`
     - `group.bench_with_input(BenchmarkId::from_parameter(size), &input, |b, input| b.iter(|| engine.lint(black_box(input))))`
  3. Engine constructed once outside the group
  4. Criterion's HTML report shows throughput curves — visual inspection for linearity
- **MIRROR**: BENCHMARK_STRUCTURE, ENGINE_CONSTRUCTION
- **IMPORTS**: `criterion::{Criterion, criterion_group, criterion_main, black_box, BenchmarkGroup, BenchmarkId, Throughput}`
- **GOTCHA**: Use `group.throughput()` so criterion reports bytes/sec, making linearity visible in the HTML report. Don't include engine construction time in the measurement.
- **VALIDATE**: `cargo bench -p marque-engine --bench linear_scaling` produces throughput data for all 7 sizes; visual check that throughput doesn't degrade

### Task 4: T069 — Corpus Accuracy Harness (SC-002/SC-003/SC-003a)
- **ACTION**: Create `tests/corpus_accuracy.rs` as a workspace-level integration test
- **IMPLEMENT**:
  1. **Lint accuracy on invalid fixtures (SC-002)**:
     - For each `.txt` in `tests/corpus/invalid/`, load fixture and its `.expected.json`
     - Run `Engine::lint(source)` and compare diagnostics against expected
     - Match by `rule` and `span` (start, end)
     - Track per-rule match counts: `HashMap<RuleId, (matched, total)>`
     - Assert >=95% match rate per-rule AND overall
  2. **Fix accuracy on invalid fixtures (SC-003)**:
     - For each invalid fixture, run `Engine::fix(source, FixMode::Apply)`
     - Re-lint the fixed output
     - Track per-rule: how many fixtures have zero remaining violations after fix
     - Assert >=95% per-rule AND overall
  3. **Precision on prose (SC-003a)**:
     - Load every `.txt` in `tests/corpus/prose/`
     - Lint each line independently (split by `\n`)
     - Assert zero diagnostics on clean prose lines
     - This prevents over-firing on legitimate text
  4. Print summary table on failure showing per-rule accuracy
- **MIRROR**: CORPUS_LOADING, EXPECTED_JSON_FORMAT, ENGINE_CONSTRUCTION
- **IMPORTS**: `marque_test_utils::{invalid_fixtures, prose_fixtures, load_expected, load_fixture}`, `marque_capco::capco_rules`, `marque_config::Config`, `marque_engine::{Engine, FixMode}`, `std::collections::HashMap`
- **GOTCHA**:
  - The expected.json only has `rule` and `span` — match on those two fields, ignore severity/message (those are presentation, not correctness)
  - Prose lint: lint entire file as one input (not line-by-line) since the scanner operates on full documents. A zero-diagnostic result on the whole file is the correct assertion.
  - Per-rule measurement prevents aggregate bar from being stuffed with easy-case rules
  - Some expected.json files may have optional `severity` field — ignore it in matching
- **VALIDATE**: `cargo test --test corpus_accuracy` passes with >=95% accuracy logged

### Task 5: T070 — Native/WASM Parity Scaling (SC-008)
- **ACTION**: Extend `crates/wasm/tests/native_parity.rs` to cover full corpus
- **IMPLEMENT**:
  1. Update `lint_parity_invalid_fixtures` to remove the `>= 10` minimum assertion (it was a T061 minimum; T070 requires ALL)
  2. Add `lint_parity_prose_fixtures` test covering `tests/corpus/prose/`
  3. Add `fix_parity_valid_fixtures` test (valid fixtures should produce empty fix output)
  4. All existing edge-case tests remain
- **MIRROR**: PARITY_TEST_PATTERN
- **IMPORTS**: Existing imports sufficient
- **GOTCHA**: The `>= 10` assert was a T061 minimum guarantee. For T070, just iterate all fixtures — the assert is implicit (if zero fixtures exist, the loop is empty and that's a different problem caught by T069). Keep the `assert!(!txt_files.is_empty())` as a sanity guard instead.
- **VALIDATE**: `cargo test -p marque-wasm --test native_parity` passes on all fixtures

### Task 6: T071 — Agent Context Update (CLAUDE.md)
- **ACTION**: Update `CLAUDE.md` "Current Status" section
- **IMPLEMENT**:
  1. Change "Pre-MVP" to "MVP complete" (or appropriate status)
  2. Update the description of what's functional: full lint/fix/audit pipeline, 10 CAPCO rules, WASM parity, benchmarks
  3. Add Phase 7 artifacts to "Recent Changes" section
  4. Ensure "Active Technologies" reflects actual deps (criterion for benchmarks, cargo-fuzz for fuzzing)
- **MIRROR**: N/A — documentation
- **IMPORTS**: N/A
- **GOTCHA**: Don't overstate — say "MVP" not "production-ready". The incremental cache, extract integration, and server auth are still planned.
- **VALIDATE**: Read CLAUDE.md and verify accuracy against actual codebase state

### Task 7: T072 — Quickstart Validation (All SC)
- **ACTION**: Validate `specs/001-marque-mvp/quickstart.md` end-to-end
- **IMPLEMENT**:
  1. Run each step from quickstart.md in sequence:
     - `cargo build -p marque`
     - Lint a known-bad fixture (use actual corpus path, not the placeholder `E001-banner-abbreviation.txt`)
     - Lint with `--format json`
     - Dry-run fix with audit capture
     - Apply fix and re-lint
     - WASM build with `wasm-pack build crates/wasm --target web --profile release`
     - `cargo test --workspace`
  2. Fix any quickstart.md references to fixture paths that don't exist (the spec references `E001-banner-abbreviation.txt` but actual corpus uses `banner_abbrev.txt`)
  3. If any step fails, fix the underlying issue (not the quickstart)
- **MIRROR**: N/A — validation
- **IMPORTS**: N/A
- **GOTCHA**: The quickstart references `tests/corpus/invalid/E001-banner-abbreviation.txt` but actual fixture names use short forms like `banner_abbrev.txt`. Either rename fixtures or update quickstart. Updating quickstart is simpler since it's in `specs/` not `tests/`.
- **VALIDATE**: All quickstart steps succeed on current branch

### Task 8: T072a — Fuzzing Target (Robustness)
- **ACTION**: Add `cargo-fuzz` target for `Engine::lint` on arbitrary `&[u8]`
- **IMPLEMENT**:
  1. Create `crates/engine/fuzz/Cargo.toml`:
     ```toml
     [package]
     name = "marque-engine-fuzz"
     version = "0.0.0"
     publish = false
     edition = "2024"

     [package.metadata]
     cargo-fuzz = true

     [dependencies]
     libfuzzer-sys = "0.4"
     marque-engine = { path = ".." }
     marque-capco = { path = "../../marque-capco" }
     marque-config = { path = "../../marque-config" }

     [[bin]]
     name = "lint"
     path = "fuzz_targets/lint.rs"
     test = false
     doc = false
     ```
  2. Create `crates/engine/fuzz/fuzz_targets/lint.rs`:
     ```rust
     #![no_main]
     use libfuzzer_sys::fuzz_target;
     use marque_capco::capco_rules;
     use marque_config::Config;
     use marque_engine::{Engine, FixMode};

     fuzz_target!(|data: &[u8]| {
         if data.len() > 65_536 { return; } // 64KB bound

         let engine = Engine::new(Config::default(), vec![Box::new(capco_rules())]);

         // (a) lint never panics
         let result = engine.lint(data);

         // (b) every Span is within bounds and start <= end
         for d in &result.diagnostics {
             assert!(d.span.start <= d.span.end, "span start > end");
             assert!(d.span.end <= data.len(), "span end exceeds input length");
         }

         // (c) fix-then-lint idempotency
         let fixed = engine.fix(data, FixMode::Apply);
         let relint = engine.lint(&fixed.source);
         // After fixing, re-linting should produce no fixable diagnostics
         // (remaining diagnostics are below-threshold suggestions only)
     });
     ```
  3. Add `crates/engine/fuzz` to root workspace `exclude` (fuzz targets use their own workspace)
- **MIRROR**: N/A — new pattern (cargo-fuzz)
- **IMPORTS**: `libfuzzer-sys`, `marque_engine`, `marque_capco`, `marque_config`
- **GOTCHA**:
  - Fuzz target workspace must NOT be in the main workspace `members` — add to `exclude`
  - Engine construction per iteration is expensive but acceptable for fuzzing (correctness > speed)
  - The 64KB bound prevents OOM on pathological inputs
  - Fix-then-lint idempotency: don't assert ZERO diagnostics (some may be below-threshold suggestions), assert no panics and valid spans
  - Requires nightly: `cargo +nightly fuzz run lint`
- **VALIDATE**: `cargo +nightly fuzz run lint -- -max_total_time=10` runs for 10s without panics (if nightly available; otherwise validate compilation only)

### Task 9: T073 — Code Quality Gates
- **ACTION**: Run final clippy and fmt checks, fix any findings
- **IMPLEMENT**:
  1. `cargo fmt --check` — fix any formatting issues
  2. `cargo clippy --workspace --benches -- -D warnings` — fix any lint findings
  3. Ensure the new benchmark code and test code also pass clippy
- **MIRROR**: CHECK_SCRIPT
- **IMPORTS**: N/A
- **GOTCHA**: The `--benches` flag is important — clippy must also lint benchmark code. New benchmark files may trigger clippy warnings about unused variables in criterion closures.
- **VALIDATE**: Both commands exit 0

### Task 10: T074 — Final Classifier ID Audit (SC-006)
- **ACTION**: Comprehensive scan for classifier-id-shaped strings
- **IMPLEMENT**:
  1. Grep entire repository for patterns that look like classifier IDs:
     - Strings matching `classifier_id\s*=\s*"[^"]+"`  in non-test, non-spec files
     - Hardcoded classifier ID values in source code (not test code)
     - Any `.marque.local.toml` files that shouldn't be committed
  2. Verify all test fixtures use `null` or test-specific values (`TEST-WASM-42`, etc.) that are clearly synthetic
  3. Verify `.gitignore` includes `.marque.local.toml`
  4. Confirm zero leaks
- **MIRROR**: N/A — audit
- **IMPORTS**: N/A
- **GOTCHA**: Test code legitimately contains classifier IDs (`TEST-WASM-42`, etc.) — only flag non-test source files. The audit is about preventing real classifier IDs from being committed.
- **VALIDATE**: Grep returns zero matches in non-test source files

---

## Testing Strategy

### Unit Tests

| Test | Input | Expected Output | Edge Case? |
|---|---|---|---|
| lint_latency_benchmark | ~10KB marking text | Criterion timing data, p95 < 16ms | No |
| linear_scaling sizes | 1KB-100KB repeated blocks | Linear throughput curve | Large input (100KB) |
| corpus_accuracy_lint | All invalid fixtures | >=95% per-rule match vs expected.json | Fixtures with multiple diagnostics |
| corpus_accuracy_fix | All invalid fixtures | >=95% per-rule zero-remaining after fix | Mixed-confidence fixtures |
| corpus_precision_prose | prose/article.txt (1010 lines) | Zero diagnostics | Clean prose with no markings |
| parity_prose | prose/*.txt | Native == WASM output | Yes |
| fuzz_lint | Random &[u8] <=64KB | No panics, valid spans | All (fuzzer-generated) |

### Edge Cases Checklist
- [ ] Empty input (0 bytes) — lint and fix must not panic
- [ ] Maximum size input (100KB) — benchmark and fuzz must handle
- [ ] Non-UTF-8 input — fuzz target receives arbitrary &[u8]
- [ ] Fixture with zero diagnostics expected — accuracy harness handles correctly
- [ ] Fixture with multiple rules triggered — per-rule counting works
- [ ] Prose with embedded marking-like text — precision gate catches over-firing

---

## Validation Commands

### Static Analysis
```bash
cargo clippy --workspace --benches -- -D warnings
```
EXPECT: Zero warnings

### Formatting
```bash
cargo fmt --check
```
EXPECT: No formatting issues

### Unit Tests
```bash
cargo test --workspace
```
EXPECT: All existing 239+ tests pass, plus new corpus_accuracy tests

### Benchmarks
```bash
cargo bench -p marque-engine --bench lint_latency
cargo bench -p marque-engine --bench linear_scaling
```
EXPECT: Both produce criterion output with timing/throughput data

### Regression Gate
```bash
bash scripts/bench-check.sh
```
EXPECT: Exit 0 (no regression vs baseline)

### Corpus Accuracy
```bash
cargo test --test corpus_accuracy
```
EXPECT: >=95% per-rule and overall accuracy

### WASM Parity
```bash
cargo test -p marque-wasm --test native_parity
```
EXPECT: All fixtures produce identical native/WASM output

### Fuzz (optional, requires nightly)
```bash
cargo +nightly fuzz run lint -- -max_total_time=30
```
EXPECT: No panics, no assertion failures

### Full Suite
```bash
bash scripts/check.sh
```
EXPECT: All checks pass

---

## Acceptance Criteria
- [ ] T067: `lint_latency` benchmark implemented and producing criterion data
- [ ] T067a: `benches/baseline.json` committed with reference measurements; `scripts/bench-check.sh` exits 0
- [ ] T068: `linear_scaling` benchmark sweeps 1KB-100KB with throughput reporting
- [ ] T069: `corpus_accuracy` harness passes >=95% per-rule lint accuracy, >=95% per-rule fix accuracy, zero diagnostics on prose
- [ ] T070: Native/WASM parity tests cover ALL corpus fixtures (not just minimum 10)
- [ ] T071: CLAUDE.md reflects MVP completion status accurately
- [ ] T072: Quickstart.md steps validated end-to-end
- [ ] T072a: Fuzz target compiles and runs without panics on initial seed
- [ ] T073: `cargo clippy` and `cargo fmt --check` both exit 0
- [ ] T074: Zero classifier-id leaks in non-test source files

## Completion Checklist
- [ ] All 10 tasks completed (T067-T074 including sub-tasks)
- [ ] All validation commands pass
- [ ] New tests written and passing
- [ ] No clippy warnings
- [ ] No fmt issues
- [ ] Benchmark baselines captured
- [ ] CLAUDE.md updated
- [ ] No scope creep beyond Phase 7

## Risks
| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| p95 exceeds 16ms on dev hardware | Medium | Medium | Document reference machine; the 16ms target is for "commodity dev hardware" per spec — if close, it's acceptable with documentation |
| Corpus accuracy <95% for a specific rule | Low | High | Fix the rule or add corpus fixtures — Phase 7 reveals gaps, it doesn't paper over them |
| cargo-fuzz finds panics | Medium | Low | Good — that's the point. Fix panics found by fuzzer before shipping |
| Criterion output format changes | Low | Low | bench-check.sh parses simple text; pin criterion version |
| nightly not available for fuzz | Low | Low | Fuzz target is NOT CI-gated in MVP; manual nightly run is sufficient |

## Notes
- **Task parallelization**: T067+T068 (benchmarks), T069 (accuracy), T070 (parity), T072a (fuzz) are all independent and can be implemented in parallel. T067a depends on T067. T071, T072, T073, T074 are final gates.
- **Prose corpus**: `tests/corpus/prose/article.txt` has 1010 lines — meets the ">=1000 lines" requirement from T026a.
- **Fixture counts**: 20 valid + 27 invalid + 1 prose = 48 total .txt fixtures.
- **Existing test count**: 239 tests passing. Phase 7 adds corpus_accuracy tests + extended parity tests.
- **Quickstart fixture naming**: The spec's quickstart.md references `E001-banner-abbreviation.txt` but actual corpus uses `banner_abbrev.txt`. Will update quickstart references during T072.
- **daachorse**: The spec mentions "different Phase-2 token-matching engines" for T070, but MVP uses aho-corasick for both native and WASM (per R-7 research decision). daachorse is deferred. T070 still validates parity — just both paths use the same engine.
