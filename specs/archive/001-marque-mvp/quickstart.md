<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Quickstart: Marque MVP

**Branch**: `001-marque-mvp` | **Date**: 2026-04-08

A contributor smoke test that exercises the full lint → fix → audit → re-lint
loop end-to-end on a known-bad fixture, plus the WASM parity check that backs
SC-008. Run this after `cargo build` to convince yourself the slice is alive.

---

## Prerequisites

- Rust 1.85+ with the 2024 edition (constitution Tech Stack).
- `wasm-pack` installed (only for the WASM steps).
- The repository checked out on the `001-marque-mvp` branch.

---

## 1. Build the workspace

```sh
cargo build -p marque
```

This compiles `marque-ism`, `marque-core`, `marque-rules`, `marque-capco`,
`marque-engine`, `marque-config`, and the `marque` CLI. The `marque-ism` build
step runs `build.rs`, which parses the ODNI ISM CVE XML under
`crates/ism/schemas/ISM-v2022-DEC/` and generates ISM vocabulary types.
If the schema directory is missing or `build.rs` fails, fix that first — every
other step depends on it.

---

## 2. Lint a known-bad fixture

```sh
cargo run -q -p marque -- check tests/corpus/invalid/E001-banner-abbreviation.txt
```

**Expected**: a single diagnostic with `rule = E001`, severity `error` (or
`fix`, depending on the project config), and a span pointing at the
abbreviation token. Exit code `1`.

The fixture is Lorem-Ipsum prose wrapping a synthetic banner that uses
portion-style abbreviations (`S//NF`). The diagnostic message should suggest
the expanded form.

---

## 3. Lint the same fixture with JSON output

```sh
cargo run -q -p marque -- check tests/corpus/invalid/E001-banner-abbreviation.txt --format json
```

**Expected**: one NDJSON line on stdout, conforming to
`specs/001-marque-mvp/contracts/diagnostic.json`. Validate it against the
schema with any JSON Schema validator if you have one installed.

---

## 4. Dry-run a fix and capture audit records

```sh
cargo run -q -p marque -- fix --dry-run \
    tests/corpus/invalid/E001-banner-abbreviation.txt \
    2> /tmp/marque-audit.ndjson
```

**Expected**:
- The fixture file on disk is unchanged (`git status` clean).
- `/tmp/marque-audit.ndjson` contains exactly one record with
  `applied: false`, all required fields populated, and `classifier_id: null`
  (assuming you have not exported `MARQUE_CLASSIFIER_ID`).
- Exit code is the same as a non-dry-run that didn't apply (i.e. `1` because
  the diagnostic still exists).

This satisfies **FR-006**.

---

## 5. Apply the fix and re-lint

```sh
cp tests/corpus/invalid/E001-banner-abbreviation.txt /tmp/banner.txt
cargo run -q -p marque -- fix /tmp/banner.txt 2> /tmp/marque-audit.ndjson
cargo run -q -p marque -- check /tmp/banner.txt
```

**Expected**:
- After step 2, `/tmp/banner.txt` is rewritten with the abbreviation expanded.
- `/tmp/marque-audit.ndjson` contains one record with `applied: true` and the
  same `original` / `replacement` as the dry-run produced.
- After step 3, the re-lint exits `0` with no diagnostics.

This is the SC-003 round-trip in miniature.

---

## 6. Verify the classifier-identity hardening

```sh
mkdir -p /tmp/marque-bad-config
cat > /tmp/marque-bad-config/.marque.toml <<'EOF'
[capco]
version = "2022-DEC"

[user]
classifier_id = "should-not-be-here"
EOF

(cd /tmp/marque-bad-config && cargo run -q -p marque -- check < /dev/null)
echo "exit code: $?"
```

**Expected**: exit code `65`, with a single-line stderr error identifying the
`[user]` section in `.marque.toml`. This is the **R-5 hard-fail** that backs
**SC-006** and **FR-010**.

```sh
rm -rf /tmp/marque-bad-config
```

---

## 7. Build and exercise the WASM target

```sh
wasm-pack build crates/wasm --target web --profile release
```

Open the bundled HTML harness (whichever path the WASM build emits — typically
`crates/wasm/pkg/index.html` if a harness is committed; otherwise the
generated `pkg/marque_wasm.js` is the public surface). From the browser
DevTools console:

```js
import init, { lint } from './marque_wasm.js';
await init();
const text = "(S//NF) Lorem ipsum dolor sit amet.";
const native_output = /* run `marque check - --format json` against the same text */;
console.log(JSON.stringify(lint(text), null, 2));
```

**Expected**: the diagnostics returned from `lint(text)` are byte-identical to
the native CLI output for the same input — same rule IDs, same byte spans,
same messages. This is the **SC-008** parity property.

---

## 8. Run the test suite

```sh
cargo test --workspace
```

**Expected**: all tests pass, including:

- `marque-capco::rules::tests` — every rule has at least one positive and one
  negative case from the corpus.
- `marque-engine::tests::audit_completeness` — every applied fix produces a
  complete audit record (SC-004).
- `marque-engine::tests::overlap_determinism` — overlapping fixes apply in
  reverse byte order without corruption (FR-016).
- `marque-config::tests::precedence` — config layering walks the four-tier
  precedence chain correctly (FR-007).

---

## 9. Run the benchmarks (optional)

```sh
cargo bench -p marque-engine --bench lint_latency
cargo bench -p marque-engine --bench linear_scaling
```

**Expected**:
- `lint_latency` reports a p95 below 16ms for inputs ≤10KB (SC-001) on
  reasonably modern developer hardware.
- `linear_scaling` shows throughput growth that is linear (within noise) across
  at least one order of magnitude of input size (SC-005).

If either benchmark fails its acceptance bar on your hardware, capture the CPU
model and the `criterion` report and open a discussion before tuning — the
constitution says perf is measured, not assumed.

---

## What success looks like

A clean run of steps 1–8 means the MVP slice is healthy. Step 9 is advisory
for the perf bar but not gated on every contribution. If any step fails,
re-read the corresponding spec section (FR or SC referenced above) before
patching — the test is the source of truth, the spec is the contract.
