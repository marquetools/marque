#!/usr/bin/env bash

# SPDX-FileCopyrightText: 2026 Knitli Inc.
#
# SPDX-License-Identifier: MIT OR Apache-2.0

# Performance regression gate for SC-001 (strict-path) and SC-002 (decoder-path).
#
# Runs the lint_latency benchmark and compares Criterion's confidence-interval
# upper bound against the per-bench baseline. Fails with non-zero exit if the
# CI upper bound regresses by >10% versus baseline, or exceeds the per-bench
# absolute target (`target_upper_ci_us` in `benches/baseline.json`).
#
# Two benches are checked:
#   - `lint_10kb`                         (SC-001, target 16ms)
#   - `decoder_10kb_one_mangled_region`   (SC-002, target 18ms)
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
    local bench_output time_line
    bench_output=$(cargo bench -p marque-engine --bench lint_latency -- "^${bench_name}\$" 2>&1)
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
    echo "bench-check[$bench_name]: regression threshold (baseline + 10%) = ${threshold} µs"

    if [[ "$current_us" -gt "$threshold" ]]; then
        echo "bench-check[$bench_name]: FAIL — regressed: ${current_us} µs > ${threshold} µs (baseline: ${baseline_upper_ci} µs)"
        return 1
    fi

    if [[ "$current_us" -gt "$target_upper_ci" ]]; then
        echo "bench-check[$bench_name]: FAIL — absolute target exceeded: ${current_us} µs > ${target_upper_ci} µs"
        return 1
    fi

    echo "bench-check[$bench_name]: PASS — ${current_us} µs <= ${threshold} µs (baseline + 10%), well under ${target_upper_ci} µs target"
    return 0
}

OVERALL_STATUS=0
check_one_bench "lint_10kb" || OVERALL_STATUS=1
check_one_bench "decoder_10kb_one_mangled_region" || OVERALL_STATUS=1

if [[ "$OVERALL_STATUS" -ne 0 ]]; then
    echo "bench-check: FAIL — one or more benches failed their regression / absolute gates"
    exit 1
fi

echo "bench-check: PASS — all benches within regression and absolute targets"
