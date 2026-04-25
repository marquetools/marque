#!/usr/bin/env bash

# SPDX-FileCopyrightText: 2026 Knitli Inc.
#
# SPDX-License-Identifier: MIT OR Apache-2.0

# Performance regression gate for SC-001 (strict-path), SC-002 (decoder-path),
# and SC-005 (linear scaling).
#
# Runs the lint_latency benchmark and compares Criterion's confidence-interval
# upper bound against the per-bench baseline. Fails with non-zero exit if the
# CI upper bound regresses by >10% versus baseline, or exceeds the per-bench
# absolute target (`target_upper_ci_us` in `benches/baseline.json`).
#
# Then runs the linear_scaling benchmark and computes the coefficient of
# determination (R²) for the linear regression of (input_size, mean_time)
# across the SC-005 sweep. Fails if R² falls below the `r_squared_min`
# threshold in `benches/baseline.json`.
#
# Three gates are checked:
#   - `lint_10kb`                         (SC-001, target 16ms upper CI)
#   - `decoder_10kb_one_mangled_region`   (SC-002, target 18ms upper CI)
#   - `lint_scaling`                      (SC-005, R² >= 0.9 across size sweep)
#
# SC-005 unit-of-analysis note: the constitution and SC-005 spec both call
# for "linear scaling" of the lint hot path. The existing `linear_scaling.rs`
# bench sweeps INPUT SIZE (1KB → 100KB) and measures per-size time, which is
# the per-document hot-path measurement. Worker-count scaling (BatchEngine /
# Tokio) is a separate, layered concern and would require new infrastructure;
# T087 picks input-size scaling because (a) it matches what the existing
# bench measures, (b) it is what SC-001's interactive p95 generalizes to at
# larger inputs, and (c) the task is polish, not new instrumentation.
#
# Usage:
#   bash scripts/bench-check.sh           # run benchmarks and check both gates
#   bash scripts/bench-check.sh --skip    # skip (for local dev without bench)
#
# Called by scripts/check.sh --bench (opt-in for local dev, required in CI).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BASELINE="$REPO_ROOT/benches/baseline.json"

# Skip the +10%-vs-baseline regression check when the runner profile does
# not match the captured baseline. The baseline lives in `benches/baseline.json`
# under `reference_machine.cpu` and is captured on a WSL2 dev machine; a
# GitHub Actions `ubuntu-latest` runner is a different profile entirely
# (different CPU, different scheduler noise, different cache topology) and
# +10% vs that baseline is meaningless. The absolute `target_upper_ci_us`
# gate (16ms / 18ms) and the SC-005 R² floor still run — those are the
# load-bearing constitution-level checks.
#
# CI sets `MARQUE_BENCH_SKIP_REGRESSION=1` until a CI-machine baseline is
# captured in a follow-up PR. Local dev runs without the env var and
# enforces +10%.
SKIP_REGRESSION="${MARQUE_BENCH_SKIP_REGRESSION:-0}"

if [[ "${1:-}" == "--skip" ]]; then
    echo "bench-check: skipped (--skip flag)"
    exit 0
fi

if [[ ! -f "$BASELINE" ]]; then
    echo "bench-check: ERROR — baseline file not found: $BASELINE"
    echo "Run the benchmark and create benches/baseline.json first."
    exit 1
fi

# check_one_bench BENCH_NAME
#
# Reads the named bench's baseline upper_ci_us and target_upper_ci_us from
# baseline.json, runs Criterion with a `^<name>$`-anchored filter so the
# captured output contains exactly one `time:` line, parses it, and applies
# both the +10% regression check and the absolute target check.
#
# Each bench is run separately rather than parsing a multi-bench captured
# output blob — Criterion's report layout is "<name>           time:   [...]"
# for short names but "<name>\n                        time:   [...]" for
# long names, and the format flips depending on alignment column. Filtering
# by anchored regex sidesteps that whole class of parsing fragility.
check_one_bench() {
    local bench_name="$1"

    # Extract baseline upper CI bound (microseconds) and absolute target.
    local baseline_upper_ci target_upper_ci
    baseline_upper_ci=$(python3 -c "
import json, sys
with open('$BASELINE') as f:
    data = json.load(f)
print(data['$bench_name']['upper_ci_us'])
" 2>/dev/null || echo "")

    target_upper_ci=$(python3 -c "
import json, sys
with open('$BASELINE') as f:
    data = json.load(f)
print(data['$bench_name']['target_upper_ci_us'])
" 2>/dev/null || echo "")

    if [[ -z "$baseline_upper_ci" || -z "$target_upper_ci" ]]; then
        echo "bench-check: ERROR — could not parse '$bench_name' upper_ci_us / target_upper_ci_us from $BASELINE"
        return 1
    fi

    # L-3: Validate that the extracted values are positive integers.
    if ! [[ "$baseline_upper_ci" =~ ^[0-9]+$ ]]; then
        echo "bench-check: ERROR — '$bench_name' baseline upper_ci_us is not a positive integer: ${baseline_upper_ci}"
        return 1
    fi
    if ! [[ "$target_upper_ci" =~ ^[0-9]+$ ]]; then
        echo "bench-check: ERROR — '$bench_name' target_upper_ci_us is not a positive integer: ${target_upper_ci}"
        return 1
    fi

    echo "bench-check[$bench_name]: baseline upper CI = ${baseline_upper_ci} µs, absolute target = ${target_upper_ci} µs"
    echo "bench-check[$bench_name]: running benchmark..."

    # Anchored filter so the bench harness runs exactly this one bench. With a
    # single bench in the captured output there is exactly one `time:` line and
    # `grep | head -1` is unambiguous. Criterion accepts the filter as a regex
    # after `--`.
    #
    # Capture exit status explicitly: under `set -e`, a bare command-substitution
    # failure exits the script before the error-reporting branch runs, so a
    # missing/broken bench would surface as a generic shell exit instead of the
    # named bench failure. `if !` keeps the captured stderr+stdout for the
    # diagnostic.
    local bench_output time_line
    if ! bench_output=$(cargo bench -p marque-engine --bench lint_latency -- "^${bench_name}\$" 2>&1); then
        echo "bench-check[$bench_name]: ERROR — 'cargo bench' invocation failed"
        if [[ -n "$bench_output" ]]; then
            printf '%s\n' "$bench_output"
        fi
        return 1
    fi
    time_line=$(echo "$bench_output" | grep "time:" | head -1)

    if [[ -z "$time_line" ]]; then
        echo "bench-check[$bench_name]: ERROR — no 'time:' line found in criterion output"
        echo "$bench_output"
        return 1
    fi
    echo "bench-check[$bench_name]: criterion output: $time_line"

    # Extract the last "number unit" pair (upper bound of CI) and convert to
    # microseconds. Uses Python instead of grep -oP (PCRE) for portability across
    # macOS/BSD/Linux. Rounds up (math.ceil) so fractional µs values never
    # undercount a regression.
    local current_us
    current_us=$(python3 -c "
import math, re, sys
line = sys.argv[1]
matches = re.findall(r'([0-9]+(?:\.[0-9]+)?)\s*([µnm]s)', line)
if not matches:
    sys.exit(1)
value, unit = matches[-1]
value = float(value)
if unit == 'ns':
    print(math.ceil(value / 1000))
elif unit == 'µs':
    print(math.ceil(value))
elif unit == 'ms':
    print(math.ceil(value * 1000))
else:
    sys.exit(2)
" "$time_line" 2>/dev/null || echo "")

    if [[ -z "$current_us" ]]; then
        echo "bench-check[$bench_name]: ERROR — could not parse timing from criterion output"
        echo "$bench_output"
        return 1
    fi
    echo "bench-check[$bench_name]: measured upper CI = ${current_us} µs"

    # +10% regression threshold vs baseline. Round up (math.ceil) so a fractional
    # µs in the baseline can never silently pass a regression.
    local threshold
    threshold=$(python3 -c "import math; print(math.ceil($baseline_upper_ci * 1.10))")

    if [[ "$SKIP_REGRESSION" == "1" ]]; then
        echo "bench-check[$bench_name]: skipping +10% baseline check (MARQUE_BENCH_SKIP_REGRESSION=1); absolute target still enforced"
    else
        echo "bench-check[$bench_name]: regression threshold (baseline + 10%) = ${threshold} µs"
        if [[ "$current_us" -gt "$threshold" ]]; then
            echo "bench-check[$bench_name]: FAIL — regressed: ${current_us} µs > ${threshold} µs (baseline: ${baseline_upper_ci} µs)"
            return 1
        fi
    fi

    if [[ "$current_us" -gt "$target_upper_ci" ]]; then
        echo "bench-check[$bench_name]: FAIL — absolute target exceeded: ${current_us} µs > ${target_upper_ci} µs"
        return 1
    fi

    if [[ "$SKIP_REGRESSION" == "1" ]]; then
        echo "bench-check[$bench_name]: PASS — ${current_us} µs under ${target_upper_ci} µs absolute target (regression check skipped)"
    else
        echo "bench-check[$bench_name]: PASS — ${current_us} µs <= ${threshold} µs (baseline + 10%), well under ${target_upper_ci} µs target"
    fi
    return 0
}

# check_linear_scaling
#
# Runs the `linear_scaling` Criterion bench (`crates/engine/benches/linear_scaling.rs`)
# and computes R² for a linear regression of (input_size_bytes, mean_time_µs)
# across the SC-005 size sweep. Fails if R² < `lint_scaling.r_squared_min`
# from `benches/baseline.json`.
#
# Uses Criterion's `mean` value (the middle of the three numbers in the
# `time:` output) rather than the upper CI bound — the upper CI is the right
# metric for regression-vs-baseline (it's the conservative reading), but for
# linear regression we want the point estimate, since CI width inflates with
# variance and would mask a genuine non-linearity. Reads each `lint_scaling/<size>`
# line independently.
check_linear_scaling() {
    local r_squared_min
    r_squared_min=$(python3 -c "
import json
with open('$BASELINE') as f:
    data = json.load(f)
print(data['lint_scaling']['r_squared_min'])
" 2>/dev/null || echo "")

    if [[ -z "$r_squared_min" ]]; then
        echo "bench-check[lint_scaling]: ERROR — could not parse 'lint_scaling.r_squared_min' from $BASELINE"
        return 1
    fi

    # Validate it parses as a float in (0, 1].
    if ! python3 -c "
v = float('$r_squared_min')
assert 0.0 < v <= 1.0, 'out of range'
" 2>/dev/null; then
        echo "bench-check[lint_scaling]: ERROR — r_squared_min not a float in (0, 1]: ${r_squared_min}"
        return 1
    fi

    echo "bench-check[lint_scaling]: minimum R² = ${r_squared_min}"
    echo "bench-check[lint_scaling]: running benchmark..."

    # Same `set -e` capture pattern as `check_one_bench` — a `cargo bench`
    # failure under bare command substitution would exit the script silently.
    local bench_output
    if ! bench_output=$(cargo bench -p marque-engine --bench linear_scaling 2>&1); then
        echo "bench-check[lint_scaling]: ERROR — 'cargo bench' invocation failed"
        if [[ -n "$bench_output" ]]; then
            printf '%s\n' "$bench_output"
        fi
        return 1
    fi

    # Extract every `lint_scaling/<size>      time:   [lower mean upper]` line
    # and compute R² on (size, mean_time_µs). Sizes are byte counts emitted by
    # `BenchmarkId::from_parameter(input.len())`; means are converted to
    # microseconds for unit consistency with the SC-001 / SC-002 gates.
    local r_squared
    r_squared=$(python3 - "$bench_output" <<'PY' 2>/dev/null || true
import math, re, sys

text = sys.argv[1]
# Match `lint_scaling/<size>` followed by `time:   [lower unit mean unit upper unit]`.
# The Criterion print spans two lines for short benchmark IDs sometimes; the
# `time:` token always appears on the same line as either the ID or its
# continuation, so search the whole blob with a multi-line tolerant regex.
size_pat = re.compile(
    r"lint_scaling/(\d+)\s+(?:\n\s+)?time:\s+\[\s*"
    r"([0-9]+(?:\.[0-9]+)?)\s*([µnm]s)\s+"
    r"([0-9]+(?:\.[0-9]+)?)\s*([µnm]s)\s+"
    r"([0-9]+(?:\.[0-9]+)?)\s*([µnm]s)"
)

def to_us(value, unit):
    v = float(value)
    if unit == "ns":
        return v / 1000.0
    if unit == "µs":
        return v
    if unit == "ms":
        return v * 1000.0
    raise ValueError(f"unknown unit: {unit}")

points = []
for m in size_pat.finditer(text):
    size = int(m.group(1))
    # Group 4 / unit 5 is the mean (middle of the three CI numbers).
    mean_us = to_us(m.group(4), m.group(5))
    points.append((size, mean_us))

if len(points) < 3:
    sys.stderr.write(f"insufficient samples: {len(points)}\n")
    sys.exit(1)

# Standard ordinary-least-squares R².
n = len(points)
sx = sum(x for x, _ in points)
sy = sum(y for _, y in points)
sxx = sum(x * x for x, _ in points)
syy = sum(y * y for _, y in points)
sxy = sum(x * y for x, y in points)

mx = sx / n
my = sy / n
ss_xy = sxy - n * mx * my
ss_xx = sxx - n * mx * mx
ss_yy = syy - n * my * my

if ss_xx <= 0 or ss_yy <= 0:
    sys.stderr.write("zero variance in samples\n")
    sys.exit(1)

r2 = (ss_xy * ss_xy) / (ss_xx * ss_yy)
print(f"{r2:.6f}")
PY
)

    if [[ -z "$r_squared" ]]; then
        echo "bench-check[lint_scaling]: ERROR — could not extract sample points from criterion output"
        echo "$bench_output"
        return 1
    fi

    echo "bench-check[lint_scaling]: measured R² = ${r_squared}"

    # Compare R² against the minimum threshold using python (bash's `[[ -lt ]]`
    # only handles integers).
    local pass
    pass=$(python3 -c "print('1' if float('$r_squared') >= float('$r_squared_min') else '0')")

    if [[ "$pass" != "1" ]]; then
        echo "bench-check[lint_scaling]: FAIL — R² ${r_squared} < ${r_squared_min} (sub-linear or noisy scaling)"
        return 1
    fi

    echo "bench-check[lint_scaling]: PASS — R² ${r_squared} >= ${r_squared_min}"
    return 0
}

OVERALL_STATUS=0
check_one_bench "lint_10kb" || OVERALL_STATUS=1
check_one_bench "decoder_10kb_one_mangled_region" || OVERALL_STATUS=1
check_linear_scaling || OVERALL_STATUS=1

if [[ "$OVERALL_STATUS" -ne 0 ]]; then
    echo "bench-check: FAIL — one or more benches failed their regression / absolute gates"
    exit 1
fi

echo "bench-check: PASS — all benches within regression and absolute targets"
