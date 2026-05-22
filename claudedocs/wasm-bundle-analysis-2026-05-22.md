# WASM Bundle Analysis — 2026-05-22

Investigation for issue #689 (WASM-size regression filed during PR #687 CI).
Findings posted at https://github.com/marquetools/marque/issues/689#issuecomment-4521024925
and copied here for posterity.

## Summary

| measurement | bytes |
|---|---|
| local release-web (pre-opt), staging HEAD `a75ee17a` | **1,380,568** |
| CI baseline at PR #498 (2026-05-17) | 1,386,447 |
| PR #687 CI run that failed | 1,458,275 |
| PR 3d / #408 first recorded baseline | 1,234,106 |
| PR 4 peak (per user recall) | >1,700,000 |
| pre-refactor `main` (per user recall) | ~700,000 |

Bundle has roughly doubled from pre-refactor `main` and is just inside the
+5% gate vs the committed baseline. PR #687's CI tripped the gate at +5.18%.

## Where the bytes are (real code, excluding analysis-side DWARF)

Methodology: cargo build with debug info + `wasm-bindgen --keep-debug` +
twiggy. The pre-existing `tools/wasm-monoaudit.sh` does not preserve DWARF
(wasm-pack 0.14 doesn't plumb `dwarf-debug-info=true` through to
wasm-bindgen); filed as issue #692.

Working recipe:

```bash
CARGO_PROFILE_RELEASE_MONOAUDIT_DEBUG=2 CARGO_PROFILE_RELEASE_MONOAUDIT_STRIP=none \
  cargo build -p marque-wasm --target wasm32-unknown-unknown --profile release-monoaudit

WB=~/.cache/.wasm-pack/wasm-bindgen-6100c0c263093c56/wasm-bindgen   # 0.2.120
$WB --target web --keep-debug \
  --out-dir /tmp/wasm-twiggy --out-name marque_wasm \
  target/wasm32-unknown-unknown/release-monoaudit/marque_wasm.wasm

twiggy top   -n 30 /tmp/wasm-twiggy/marque_wasm_bg.wasm
twiggy monos -n 30 /tmp/wasm-twiggy/marque_wasm_bg.wasm
```

### Top single functions (shallow bytes)

| bytes | symbol |
|---|---|
| 49,334 | `marque_engine::decoder::generate_candidate_bytes` |
| 36,394 | `marque_core::parser::Parser::parse_marking_string` |
| 28,713 | `marque_engine::engine::TwoPassFixer::run` |
| 15,516 | `marque_engine::engine::Engine::lint_with_options_internal_with_cache` |
| 14,929 | `marque_engine::decoder::DecoderRecognizer::recognize` |
| 13,402 | `marque_engine::engine::Engine::with_clock<CapcoScheme>` |
| 11,833 | `marque_capco::scheme::marking::CapcoMarking::join_via_lattice` |
| 11,111 | `marque_engine::engine::Engine::with_clock_prepared` |
| 10,687 | `marque_capco::scheme::adapter::CapcoScheme::evaluate_custom` |
| 10,089 | `marque_wasm::parse_wasm_config` |
|  9,778 | `marque_capco::rules::PreferTetragraphCollapseRule::check` |

### Top monomorphization clusters

| approx. bloat | total | family |
|---|---|---|
| **114,057** | 122,695 | `core::slice::sort::stable::quicksort` (30+ instantiations) |
| **53,220** | 56,512 | `core::slice::sort::stable::drift::sort` |
| 32,049 | 33,542 | `core::ptr::drop_in_place` (auto-generated drop glue) |
| 15,024 | 23,745 | `core::slice::sort::unstable::quicksort` |
|  8,109 | 15,975 | `aho_corasick::automaton::try_find_fwd` |
|  8,005 |  9,016 | `core::slice::sort::shared::smallsort::insertion_sort_shift_left` |
|  5,647 | 10,492 | `aho_corasick::automaton::try_find_overlapping_fwd` |

**Sort family alone = ~190 KB of bloat.** Each typed sort closure
(`sort_by(|a, b| ...)` and `sort_by_key(|x| ...)`) produces its own copy
of the sort algorithm specialized for the (slice type, closure type) pair.
Render functions in `marque_capco::render::*` and lattice helpers in
`marque_capco::lattice` (`SciSet::to_markings`, `SarSet::to_marking`,
`DisplayOnlyBlock::to_vec`, etc.) account for most of the duplication.

### Static-data section (.rodata)

- 139,626 bytes total (10.11%)
- Generated source contributions for wasm32:
  - `marque-ism/.../vocabulary.rs` 47 KB (347 `TokenMetadataEntry` rows +
    24 `CveFileMetadata` rows)
  - `marque-ism/.../values.rs` 44 KB (CVE enum derivations)
  - `marque-capco/.../priors.rs` 34 KB (from `priors.json`, decoder priors)
  - migrations + validators: ~6.5 KB
- The wasm32 elision (#453) is in effect: `source`, `poc_name`, `poc_email`,
  per-token `description` already dropped on wasm32 targets.

## Candidate paths to recover bytes

| ID | What | Estimate | Risk |
|----|------|----------|------|
| **R1** | Sort consolidation across render + lattice (CHOSEN, PR cycle in progress) | 80–130 KB | low (byte-identity tests cover render outputs) |
| R2 | `parse_wasm_config` serde reduction (40-line struct → 10 KB) | 5–8 KB (estimated) / **7,759 bytes (measured, landed)** | low |
| R3 | Decoder code-size budget / feature-gate question | 20–60 KB | strategic |
| R4 | `parse_marking_string` / `TwoPassFixer::run` `#[inline(never)]` splits | 10–20 KB | medium (perf-sensitive paths) |
| R5 | Render-function dispatch consolidation (overlaps R1) | 30–60 KB | medium |
| R6 | Externalize `TOKEN_METADATA` to JSON | 8–15 KB | medium (init-contract change) |
| R7 | Externalize priors.json (lazy decoder init) | 5–15 KB | medium (decoder availability) |
| R8 | Externalize per-token vocab adapter | 5–10 KB | medium |

## Decisions taken in this investigation

- **R1 first** (PM decision 2026-05-22). Highest impact, lowest risk, cleanest test coverage. PR cycle in progress (preflight → synthesis → implementation → review → PR).
- **R2 landed** as PR #689 follow-up — see "R2 results" below.
- **R3 deferred** pending strategic decision on whether the default WASM target needs the probabilistic decoder.
- **#692 filed** for the `wasm-monoaudit.sh` script gap so future investigations don't rediscover the wasm-pack 0.14 DWARF plumbing issue.

## R2 results (2026-05-22)

| measurement | bytes |
|---|---|
| pre-R2 (post-R1, `origin/staging` `7af85fb2`) | 1,372,791 |
| post-R2 (`refactor/r2-wasm-config-serde-reduction`) | 1,365,032 |
| **delta** | **−7,759 (−0.57%)** |

`wasm-pack build crates/wasm --target web --release` with the system
`wasm-opt`. Note: `wasm-pack 0.14` did NOT honor the
`package.metadata.wasm-pack.profile.release-web` flag list — wasm-opt
was invoked with just `-O` rather than the full SIMD-enabled flag set;
`--release` worked. The raw delta is what we measured; a full
`release-web` build (or `wasm-pack 0.15`) may give a different absolute
size but the relative R2 delta should be in the same band.

**Above the synthesis-brief 4 KB go/no-go gate.** Landed.

R2 cuts:

- `#[derive(Deserialize)] WasmConfig` (4 fields including
  `HashMap<String, String>` and `Option<f32>` / `Option<f64>`) →
  retired. Replaced with explicit `serde_json::Value::as_object().get(...)`
  extraction in a new private `wasm_config_from_value` helper.
- `#[derive(Serialize)] WasmConfigCacheKey<'a>` (3 borrowed-field
  struct with `skip_serializing_if = "Option::is_none"`) → retired
  entirely. Replaced with direct `serde_json::Map` construction in
  `build_cache_key`.

Bench impact:

- `lint_10kb` pre: `[422.35 µs 428.08 µs 434.97 µs]` (low / median / high)
- `lint_10kb` post: `[421.49 µs 432.21 µs 447.64 µs]`
- Median +0.45%, p=0.88. Criterion reports "no statistically significant
  change". Config parsing is once-per-call, not on the hot path.

Adjacent code that intentionally stays put:

- `BatchEntry`'s `#[derive(Deserialize)]` at lib.rs:811 (two `String`
  fields, no `Option`, no nested types). Estimated <1 KB; out-of-scope
  per the R2 synthesis brief. File R2b if a measurable cut is wanted.
- All `#[derive(Serialize)]` impls on OUTPUT-side types (FixResultJson,
  the diagnostic/audit projection structs at lines 284-437): in scope
  ONLY if the JS-API contract is explicitly broken open for a separate
  PR. R2 deliberately did not touch them.

f32 byte-identity gotcha worth noting for future serde-reduction work:

- `Value::Number` is f64-only. `Number::from_f64(threshold as f64)`
  widens before formatting and emits the long-form
  `"0.8500000238418579"` for the 0.85 bit-pattern — NOT the
  shortest-roundtrip `"0.85"` that `#[derive(Serialize)]` for a struct
  with an `f32` field would have produced via serde's `serialize_f32`.
- The R2 byte-identity tests caught this on first build. Fix: route
  the f32 through `serde_json::to_string(&f32)` (which goes through
  `serialize_f32`) and then parse back via `serde_json::from_str` so
  the resulting `Value::Number` retains the shortest-f32 string. One
  short alloc + one parse, only on engine-cache-miss.

## Related issues

- #689 — original regression report (this investigation)
- #692 — `tools/wasm-monoaudit.sh` DWARF preservation fix (filed during this investigation)
- #585 — prior sort consolidation in SAR lattice helpers (precedent for R1's pattern)
- #453 — wasm32 elision of per-token prose strings (already in effect)
- PR #498 — last bench/wasm baseline refresh (2026-05-17)
- PR #687 — CI run that surfaced the gate trip (run 26297691777, +5.18%)
