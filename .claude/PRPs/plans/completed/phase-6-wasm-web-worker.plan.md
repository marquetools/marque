# Plan: Phase 6 — WASM Web Worker Build (US4)

## Summary
Build the WASM target of `marque-wasm` that exposes `lint()` and `fix()` to JavaScript web workers, producing byte-identical diagnostic JSON to the native CLI. Verify with a Rust-side parity test, a dependency-audit no-I/O test, a minimal HTML harness, and binary-size/latency measurements. Per research decision R-7, use `aho-corasick` for both native and WASM (single code path) — `daachorse` is deferred until measurements justify it.

## User Story
As a web application developer, I want to lint and fix classification markings from a browser web worker, so that I get the same diagnostics as the CLI without file system or network access.

## Problem → Solution
WASM crate exists with functional `lint`/`fix` exports but: (a) output shape doesn't conform to `contracts/diagnostic.json` NDJSON format — SC-008 byte-identical parity fails; (b) no parity test exists; (c) no dependency audit for I/O crates; (d) no HTML harness; (e) WASM build not verified. → All six tasks (T061–T066a) implemented and verified.

## Metadata
- **Complexity**: Medium
- **Source PRD**: `specs/001-marque-mvp/tasks.md`
- **PRD Phase**: Phase 6 (US4)
- **Estimated Files**: 8–10 files created/modified

---

## UX Design

N/A — internal change. The WASM API is consumed by JavaScript code, not a visual UI.

### Interaction Changes
| Touchpoint | Before | After | Notes |
|---|---|---|---|
| JS `lint(text)` | Returns `WasmLintResult` JSON (custom shape) | Returns NDJSON string matching `contracts/diagnostic.json` | SC-008 parity |
| JS `fix(text, threshold)` | Returns `WasmFixResult` JSON (custom shape) | Returns JSON with `fixed_text` + NDJSON diagnostics matching audit contract | Threshold is a parameter |
| WASM build | Untested | `wasm-pack build` verified, ≤1MB artifact | T066 |

---

## Mandatory Reading

| Priority | File | Lines | Why |
|---|---|---|---|
| P0 | `crates/marque-wasm/src/lib.rs` | all (138) | Current WASM implementation to rewrite |
| P0 | `specs/001-marque-mvp/contracts/diagnostic.json` | all (70) | NDJSON shape contract — WASM output must match exactly |
| P0 | `marque/src/render.rs` | 226–284 | `DiagnosticJson`, `diagnostic_to_json()`, `render_ndjson()` — the CLI's serialization logic WASM must replicate |
| P0 | `specs/001-marque-mvp/tasks.md` | 170–188 | Phase 6 task definitions |
| P1 | `specs/001-marque-mvp/research.md` | 230–252 | R-7: aho-corasick for both targets, daachorse deferred |
| P1 | `specs/001-marque-mvp/contracts/audit-record.json` | all | Audit record shape for `fix()` output |
| P1 | `crates/marque-engine/src/engine.rs` | 76, 267–284 | `Engine::lint`, `Engine::fix`, `Engine::fix_with_threshold` signatures |
| P1 | `crates/marque-engine/src/output.rs` | 5–56 | `LintResult`, `FixResult` types |
| P2 | `specs/001-marque-mvp/quickstart.md` | 126–147 | WASM harness steps |
| P2 | `crates/marque-wasm/Cargo.toml` | all (33) | Current dependencies and wasm-pack config |
| P2 | `crates/marque-rules/src/lib.rs` | 79–90, 166–297, 303–338 | Severity, FixSource, FixProposal, Diagnostic, AppliedFix types |

## External Documentation

| Topic | Source | Key Takeaway |
|---|---|---|
| wasm-pack | https://rustwasm.github.io/docs/wasm-pack/ | `wasm-pack build --target web --profile release-wasm` is the build command |
| wasm-bindgen | https://rustwasm.github.io/docs/wasm-bindgen/ | `#[wasm_bindgen]` exports; `JsValue` for error returns; `String` return for JSON |
| cargo-deny | https://embarkstudios.github.io/cargo-deny/ | Can audit dependency tree for banned crates; alternative: manual `cargo tree` grep |

---

## Patterns to Mirror

### DIAGNOSTIC_JSON_SERIALIZATION
// SOURCE: marque/src/render.rs:226-274
```rust
#[derive(Debug, Serialize)]
pub struct DiagnosticJson<'a> {
    pub rule: &'a str,
    pub severity: &'a str,
    pub span: SpanJson,
    pub message: &'a str,
    pub citation: &'a str,
    pub fix: Option<FixJson<'a>>,
}

pub fn diagnostic_to_json(d: &Diagnostic) -> DiagnosticJson<'_> {
    DiagnosticJson {
        rule: d.rule.as_str(),
        severity: d.severity.as_str(),
        span: SpanJson { start: d.span.start, end: d.span.end },
        message: d.message.as_ref(),
        citation: d.citation,
        fix: d.fix.as_ref().map(|f| FixJson {
            source: match f.source {
                FixSource::BuiltinRule => "BuiltinRule",
                FixSource::CorrectionsMap => "CorrectionsMap",
                FixSource::MigrationTable => "MigrationTable",
            },
            replacement: f.replacement.as_ref(),
            confidence: f.confidence,
            migration_ref: f.migration_ref,
        }),
    }
}
```

### NDJSON_RENDER
// SOURCE: marque/src/render.rs:277-284
```rust
pub fn render_ndjson(out: &mut dyn std::io::Write, result: &LintResult) -> std::io::Result<()> {
    for d in &result.diagnostics {
        let json = serde_json::to_string(&diagnostic_to_json(d)).map_err(std::io::Error::other)?;
        out.write_all(json.as_bytes())?;
        out.write_all(b"\n")?;
    }
    Ok(())
}
```

### ENGINE_CONSTRUCTION
// SOURCE: crates/marque-wasm/src/lib.rs:135-137
```rust
fn build_engine(config: Config) -> Engine {
    Engine::new(config, vec![Box::new(capco_rules())])
}
```

### TEST_ENGINE_HELPER
// SOURCE: crates/marque-capco/tests/corrections_map.rs:16-24
```rust
fn engine_with_corrections(corrections: HashMap<String, String>) -> Engine {
    let mut config = Config::default();
    config.corrections = corrections;
    Engine::with_clock(
        config,
        vec![Box::new(capco_rules())],
        Box::new(FixedClock::new(UNIX_EPOCH + Duration::from_secs(FIXED_TS))),
    )
}
```

### CORPUS_FIXTURE_FORMAT
// SOURCE: tests/corpus/invalid/banner_abbrev.txt + .expected.json
```
# .txt file: raw input (one marking per file, newline-terminated)
TOP SECRET//SI//NF

# .expected.json: expected diagnostics (rule + span only, for partial matching)
{
  "diagnostics": [
    {"rule": "E001", "span": {"start": 16, "end": 18}}
  ]
}
```

---

## Files to Change

| File | Action | Justification |
|---|---|---|
| `crates/marque-wasm/src/lib.rs` | **UPDATE** | T063: Rewrite lint/fix exports to produce NDJSON matching `diagnostic.json` contract |
| `crates/marque-wasm/Cargo.toml` | **UPDATE** | Add dev-dependencies for tests (marque-capco, marque-engine, marque-config, marque-rules) |
| `crates/marque-wasm/tests/native_parity.rs` | **CREATE** | T061: Parity test — same input through Engine and WASM wrappers, assert byte-equal JSON |
| `crates/marque-wasm/tests/no_io.rs` | **CREATE** | T062: Dependency audit asserting no I/O crates in WASM dep tree |
| `crates/marque-wasm/examples/harness.html` | **CREATE** | T065: Minimal HTML harness loading WASM module, linting a fixture, printing JSON |
| `crates/marque-core/src/parser.rs` | **NO CHANGE** | T064: Per R-7, aho-corasick for both targets — no daachorse switch needed in MVP |
| `benches/wasm_latency.md` | **CREATE** | T066a: Document measurement method for WASM latency |
| `Cargo.toml` (root) | **POSSIBLE UPDATE** | Add `daachorse` to workspace deps only if needed; likely no change per R-7 |

## NOT Building

- **daachorse conditional compilation** — R-7 explicitly defers this until measurements justify it. T064 is satisfied by documenting the decision and verifying aho-corasick works in WASM.
- **Full browser E2E testing** — T065 is a static HTML page, not a Playwright/Selenium test suite.
- **WASM-specific CI pipeline** — T066 verifies the build command succeeds; CI wiring is informational.
- **Streaming WASM API** — MVP returns full result strings, no streaming.
- **Config file loading in WASM** — Config is passed as JSON from JS; no file system access.

---

## Step-by-Step Tasks

### Task 1: T063 — Rewrite WASM lint/fix exports for contract-conformant output

- **ACTION**: Rewrite `crates/marque-wasm/src/lib.rs` to produce NDJSON matching `contracts/diagnostic.json` (for lint) and include `fixed_text` + NDJSON audit records (for fix). The key change: `lint()` must return the **same JSON serialization** as the CLI's `render_ndjson()`, not a custom `WasmLintResult` wrapper.
- **IMPLEMENT**:
  1. Move the `DiagnosticJson`, `SpanJson`, `FixJson`, and `diagnostic_to_json()` helper out of the CLI crate into a shared location. Two options:
     - (a) Add them to `marque-rules` (since they project `Diagnostic` types from that crate) — adds a serde_json dep to marque-rules.
     - (b) Duplicate the serialization structs in the WASM crate — simpler, but divergence risk.
     - **Preferred**: Option (b) — duplicate in WASM crate. The structs are trivial (6 fields), and the parity test (T061) will catch any divergence. Avoids adding serde_json to marque-rules.
  2. `lint(text, config_json)` returns NDJSON string: one `DiagnosticJson` per line, newline-terminated. This is the exact output of `render_ndjson()`.
  3. `fix(text, config_json)` returns a JSON object: `{ "fixed_text": "...", "applied": [...audit records...], "remaining": [...diagnostics...] }`.
     - `applied` entries use `AuditRecordJson` shape from `contracts/audit-record.json`.
     - `remaining` entries use `DiagnosticJson` shape.
  4. The `fix()` function signature changes: `fix(text: &str, threshold: f32, config_json: Option<String>)` — threshold is an explicit parameter per T063 spec.
  5. Remove the old `WasmLintResult`, `WasmDiagnostic`, `WasmFix`, `WasmFixResult` types.
  6. Add a `WasmConfig` field for `confidence_threshold` (default 0.95) and `corrections` map.
- **MIRROR**: DIAGNOSTIC_JSON_SERIALIZATION, NDJSON_RENDER, ENGINE_CONSTRUCTION
- **IMPORTS**: `marque_engine::{Engine, FixMode}`, `marque_capco::capco_rules`, `marque_config::Config`, `marque_rules::{Diagnostic, FixSource, AppliedFix, Severity}`, `serde::Serialize`, `wasm_bindgen::prelude::*`
- **GOTCHA**: The CLI serializes `Severity::as_str()` as `"error"`, `"warn"`, `"fix"`. The WASM must use the exact same strings. Check that `Severity` has `as_str()` — if not, match on the enum variants.
- **GOTCHA**: `f32` serialization in serde_json: `confidence: 1.0` may serialize as `1.0` or `1` depending on serde_json version. The parity test catches this.
- **GOTCHA**: `AppliedFix.timestamp` is `SystemTime`. In WASM, `SystemTime::now()` may panic or return epoch. Use `js_sys::Date::now()` to get a millisecond timestamp and format as RFC 3339. Or: since the Engine already injects the timestamp, just read it from the `AppliedFix`.
- **VALIDATE**: `cargo check -p marque-wasm` compiles. Manual inspection of output shape against contract.

### Task 2: T061 — Native parity test

- **ACTION**: Create `crates/marque-wasm/tests/native_parity.rs` that drives the same inputs through both the native `Engine::lint` API and the WASM crate's `lint()` wrapper (called as a plain Rust function, not through WASM), and asserts byte-equal JSON output.
- **IMPLEMENT**:
  1. Gate with `#![cfg(not(target_arch = "wasm32"))]` — runs only on native.
  2. Load ≥10 corpus fixtures from `tests/corpus/invalid/` and `tests/corpus/valid/`.
  3. For each fixture:
     - Call `Engine::lint(source)` → serialize each diagnostic with the same `diagnostic_to_json()` → collect as NDJSON string.
     - Call `marque_wasm::lint(text, None)` → get the returned NDJSON string.
     - Assert byte-equal: `assert_eq!(native_ndjson, wasm_ndjson, "parity failure on {fixture}")`.
  4. Also test `fix()` parity on a subset of invalid fixtures.
- **MIRROR**: TEST_ENGINE_HELPER, CORPUS_FIXTURE_FORMAT
- **IMPORTS**: `marque_engine::Engine`, `marque_capco::capco_rules`, `marque_config::Config`, `marque_rules::Diagnostic`
- **GOTCHA**: The WASM crate's `lint()` function signature uses `wasm_bindgen` attributes. For native testing, we need a non-wasm-bindgen entry point. Solution: add `pub fn lint_native(text: &str, config_json: Option<String>) -> Result<String, String>` that does the same logic without `JsValue`. The `#[wasm_bindgen]` export calls `lint_native` internally.
- **GOTCHA**: Corpus fixture paths are relative to workspace root. Use `env!("CARGO_MANIFEST_DIR")` and navigate to `../../tests/corpus/`.
- **VALIDATE**: `cargo test -p marque-wasm` — all parity tests pass.

### Task 3: T062 — No-I/O dependency audit test

- **ACTION**: Create `crates/marque-wasm/tests/no_io.rs` that asserts the WASM crate's dependency tree contains no I/O crates.
- **IMPLEMENT**:
  1. Gate with `#![cfg(not(target_arch = "wasm32"))]`.
  2. Run `cargo tree -p marque-wasm --no-dev-deps --prefix none` (via `std::process::Command`) and capture stdout.
  3. Assert the output does NOT contain any of: `tokio`, `reqwest`, `hyper`, `axum`, `tower`, `mio`, `socket2`, `native-tls`, `openssl`, `rustls`.
  4. Assert the output does NOT contain `marque-extract` (which has filesystem deps).
  5. Alternative approach if `cargo tree` is unreliable in test: parse `Cargo.toml` and walk the non-dev dependency graph manually. But `cargo tree` is simpler and more comprehensive.
- **MIRROR**: N/A — unique test pattern.
- **IMPORTS**: `std::process::Command`
- **GOTCHA**: `cargo tree` must be available in the test environment. Add a skip with `#[ignore]` annotation + comment if `cargo` isn't found. Or: use `cfg_attr` to conditionally compile.
- **GOTCHA**: Must use `--no-dev-deps` to exclude test-only dependencies (like marque-engine's dev-deps which might pull in tokio for bench).
- **VALIDATE**: `cargo test -p marque-wasm -- no_io` passes.

### Task 4: T064 — Verify aho-corasick works in WASM (R-7 decision: no daachorse)

- **ACTION**: Per R-7, verify `aho-corasick` compiles for `wasm32-unknown-unknown` target. Document the decision in the plan. No code change needed — the existing implementation uses aho-corasick and the `TokenSet` trait is already abstract.
- **IMPLEMENT**:
  1. Run `cargo check -p marque-wasm --target wasm32-unknown-unknown` to verify compilation.
  2. If aho-corasick fails on wasm32, THEN implement the daachorse fallback:
     - Add `daachorse` to `Cargo.toml` workspace deps.
     - In `crates/marque-ism/src/token_set.rs`, add `#[cfg(target_arch = "wasm32")]` alternative using `daachorse::DoubleArrayAhoCorasick`.
     - Both implementations must produce identical token matching results.
  3. Most likely outcome: aho-corasick 1.x compiles fine for wasm32 (it's a pure-Rust crate with optional SIMD behind feature flags).
- **MIRROR**: N/A
- **GOTCHA**: aho-corasick uses `std` by default. Check if `no_std` is needed for wasm32. The `wasm32-unknown-unknown` target supports `std` through wasm-bindgen, so this should be fine.
- **VALIDATE**: `cargo check -p marque-wasm --target wasm32-unknown-unknown` succeeds.

### Task 5: T065 — HTML harness

- **ACTION**: Create `crates/marque-wasm/examples/harness.html` — a self-contained HTML file that loads the WASM module and lints a sample marking.
- **IMPLEMENT**:
  ```html
  <!DOCTYPE html>
  <html>
  <head><meta charset="utf-8"><title>marque WASM harness</title></head>
  <body>
    <h1>marque WASM Harness</h1>
    <textarea id="input" rows="4" cols="60">TOP SECRET//SI//NF</textarea>
    <button id="lint-btn">Lint</button>
    <button id="fix-btn">Fix</button>
    <pre id="output"></pre>
    <script type="module">
      import init, { lint, fix } from '../pkg/marque_wasm.js';
      await init();
      document.getElementById('lint-btn').addEventListener('click', () => {
        const text = document.getElementById('input').value;
        try {
          const result = lint(text, null);
          document.getElementById('output').textContent = result;
        } catch (e) {
          document.getElementById('output').textContent = 'Error: ' + e;
        }
      });
      document.getElementById('fix-btn').addEventListener('click', () => {
        const text = document.getElementById('input').value;
        try {
          const result = fix(text, 0.95, null);
          document.getElementById('output').textContent = result;
        } catch (e) {
          document.getElementById('output').textContent = 'Error: ' + e;
        }
      });
    </script>
  </body>
  </html>
  ```
- **MIRROR**: N/A — standalone HTML file.
- **GOTCHA**: The `import` path `../pkg/marque_wasm.js` assumes `wasm-pack build` output is in `crates/marque-wasm/pkg/`. Verify this is the default output directory.
- **GOTCHA**: Needs a local HTTP server to serve (can't use `file://` with ES modules). Document: `python3 -m http.server 8080` from the `crates/marque-wasm/` directory.
- **VALIDATE**: After `wasm-pack build`, open `http://localhost:8080/examples/harness.html`, click "Lint", see diagnostic JSON.

### Task 6: T066 — Verify WASM build succeeds and artifact ≤1MB

- **ACTION**: Run the full `wasm-pack build` and measure the output artifact size.
- **IMPLEMENT**:
  1. Install wasm-pack if not present: `cargo install wasm-pack` (or check with `wasm-pack --version`).
  2. Install the wasm32 target: `rustup target add wasm32-unknown-unknown`.
  3. Build: `wasm-pack build crates/marque-wasm --target web --profile release-wasm`.
  4. Measure: `ls -la crates/marque-wasm/pkg/marque_wasm_bg.wasm` — must be ≤1MB.
  5. If >1MB: check if `wasm-opt` is installed and being applied (the Cargo.toml configures `-Os`). Consider: removing unused features, checking for large static data in generated code.
- **MIRROR**: N/A — build verification.
- **GOTCHA**: `wasm-pack` may not be installed in CI. Document the installation step.
- **GOTCHA**: The `--profile release-wasm` flag requires the profile to be defined in root `Cargo.toml` (already done: `[profile.release-wasm]` at line 96).
- **VALIDATE**: Build succeeds with exit code 0. Artifact size ≤1MB.

### Task 7: T066a — WASM latency measurement documentation

- **ACTION**: Create `benches/wasm_latency.md` documenting the measurement methodology for WASM interactive latency.
- **IMPLEMENT**: Document:
  - **Target**: SC-001b — `lint(text)` completes in ≤32ms p95 on ≤10KB inputs.
  - **Method**: Open `harness.html` in Chromium, use `performance.now()` around `lint()` calls, record p50/p95/p99 over 100 iterations.
  - **Input sizes**: 100B, 1KB, 5KB, 10KB representative markings.
  - **Browser**: Current Chromium-family (version noted at measurement time).
  - **Reference machine**: Same as `plan.md` §Performance Goals.
  - **Advisory gate**: Logged, not CI-blocking for MVP. Hard gate in browser-extension slice.
  - Include a measurement script snippet (JS) that can be pasted into browser console.
- **MIRROR**: N/A — documentation file.
- **VALIDATE**: File exists with all required sections.

---

## Testing Strategy

### Unit Tests

| Test | Input | Expected Output | Edge Case? |
|---|---|---|---|
| `lint_returns_ndjson_matching_contract` | `"TOP SECRET//SI//NF"` | NDJSON with E001 diagnostic, span {16,18} | No |
| `lint_empty_input_returns_empty` | `""` | Empty string (no diagnostics) | Yes |
| `fix_returns_fixed_text_and_audit` | `"SECRET//NF"` | `fixed_text: "SECRET//NOFORN"`, applied E001 | No |
| `fix_with_threshold_filters` | `"SECRET//NF"`, threshold=1.1 | No fixes applied (threshold too high) | Yes |
| `lint_config_with_corrections` | `"SECRET//NF"` + corrections `NF→NOFORN` | C001 diagnostic (not E001) | No |
| `parity_across_10_corpus_fixtures` | 10+ fixtures from corpus | Byte-equal NDJSON vs native Engine | No — SC-008 |
| `parity_fix_across_invalid_fixtures` | Invalid fixtures | Byte-equal fixed text + audit | No |
| `no_io_deps_in_wasm_tree` | Dependency tree | No tokio/reqwest/hyper/marque-extract | No |
| `wasm_config_parsing` | `{"classifier_id":"X","corrections":{"NF":"NOFORN"}}` | Config applied correctly | No |
| `wasm_config_invalid_json` | `"not json"` | Error returned | Yes |

### Edge Cases Checklist
- [x] Empty input (no diagnostics)
- [x] Maximum size input (10KB+ text — covered by parity test with corpus)
- [x] Invalid config JSON (error path)
- [x] No config provided (default behavior)
- [x] Threshold at boundary (0.0, 1.0, values outside range)
- [ ] Non-UTF-8 input — WASM `lint(text: &str)` only accepts valid UTF-8 by type system
- [x] Multiple diagnostics on same input
- [x] Fix produces no changes (all clean input)

---

## Validation Commands

### Static Analysis
```bash
cargo clippy -p marque-wasm -- -D warnings
```
EXPECT: Zero warnings

### Compilation Check (native)
```bash
cargo check -p marque-wasm
```
EXPECT: Compiles without errors

### Compilation Check (WASM target)
```bash
cargo check -p marque-wasm --target wasm32-unknown-unknown
```
EXPECT: Compiles without errors

### Unit Tests
```bash
cargo test -p marque-wasm
```
EXPECT: All tests pass, including parity tests

### Full Test Suite
```bash
cargo test --workspace
```
EXPECT: No regressions. All 226+ tests pass.

### WASM Build
```bash
wasm-pack build crates/marque-wasm --target web --profile release-wasm
```
EXPECT: Build succeeds. `pkg/marque_wasm_bg.wasm` ≤1MB.

### Binary Size Check
```bash
ls -la crates/marque-wasm/pkg/marque_wasm_bg.wasm
```
EXPECT: File size ≤1,048,576 bytes (1MB)

### Manual Validation
- [ ] Open `harness.html` in browser after WASM build
- [ ] Click "Lint" with default text — see diagnostic JSON
- [ ] Click "Fix" — see fixed text
- [ ] Compare lint output with `echo "TOP SECRET//SI//NF" | cargo run -p marque -- check --format json`

---

## Acceptance Criteria
- [ ] T063: `lint()` and `fix()` produce NDJSON conforming to `diagnostic.json` contract
- [ ] T061: Parity test passes for ≥10 corpus fixtures (SC-008)
- [ ] T062: No I/O crates in WASM dependency tree (FR-013)
- [ ] T064: aho-corasick compiles for wasm32 (R-7 decision documented)
- [ ] T065: HTML harness loads WASM and displays diagnostic JSON
- [ ] T066: `wasm-pack build` succeeds, artifact ≤1MB
- [ ] T066a: `benches/wasm_latency.md` exists with measurement methodology
- [ ] All existing tests still pass (no regressions)
- [ ] `cargo clippy --workspace -- -D warnings` clean

## Completion Checklist
- [ ] WASM output byte-identical to CLI JSON output (SC-008)
- [ ] No filesystem or network dependencies in WASM build
- [ ] Diagnostic JSON matches `contracts/diagnostic.json` schema exactly
- [ ] Fix output includes audit records matching `contracts/audit-record.json`
- [ ] Error handling returns `JsValue` errors, not panics
- [ ] Config parsing handles all fields (classifier_id, corrections, confidence_threshold)
- [ ] Tests use real corpus fixtures, not hand-written inline strings only
- [ ] HTML harness works with a local HTTP server after `wasm-pack build`
- [ ] No unnecessary scope additions
- [ ] Self-contained — no questions needed during implementation

## Risks
| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `aho-corasick` doesn't compile for wasm32 | Low | High — blocks entire phase | Fallback to `daachorse` with cfg flag (TokenSet trait already abstracts this) |
| WASM artifact >1MB | Medium | Medium — fails T066 | Enable wasm-opt, audit for large static data, strip debug info |
| `SystemTime::now()` panics in WASM | Medium | Low — affects fix() audit timestamp | Engine already sets timestamps; verify they serialize correctly |
| f32 serialization divergence between native/WASM | Low | High — breaks SC-008 parity | Parity test catches this; use identical serde_json version |
| Corpus fixture paths break in wasm crate tests | Low | Low | Use `CARGO_MANIFEST_DIR` + relative paths |

## Notes

### R-7 Decision Impact on T064
The task spec says "Switch to `daachorse` under `cfg(target_arch = "wasm32")`" but research decision R-7 explicitly says: "Use `aho-corasick` for both native and WASM builds in the MVP." T064 is satisfied by:
1. Verifying aho-corasick compiles for wasm32.
2. Measuring binary size (T066).
3. Documenting that the daachorse switch is deferred until measurements justify it.

If aho-corasick does NOT compile for wasm32 (unlikely — it's pure Rust), then T064 becomes the daachorse implementation as originally spec'd.

### Shared Serialization Strategy
The CLI's `DiagnosticJson` and `diagnostic_to_json()` live in `marque/src/render.rs` (the CLI binary crate). WASM can't depend on the CLI binary. Options:
- **Extract to a shared crate** (e.g., `marque-rules` or a new `marque-serde` crate) — clean but adds a crate.
- **Duplicate in WASM crate** — simpler, ~50 lines, parity test catches divergence.
- **Extract to `marque-engine`** — engine already depends on marque-rules and serde_json.

Recommended: Duplicate in WASM crate (option 2). The parity test is the safety net.

### WASM `SystemTime` Behavior
`std::time::SystemTime::now()` works in wasm32-unknown-unknown via wasm-bindgen's `js_sys::Date` shim. The Engine uses `FixedClock` or `SystemTime` — in WASM context, `Engine::new()` uses the default clock which calls `SystemTime::now()`. This should work because wasm-bindgen provides the shim. Verify during T063 implementation.
