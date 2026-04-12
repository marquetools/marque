#!/usr/bin/env bash
# SC-001a — Performance regression gate.
#
# Runs the lint_latency benchmark and compares against the baseline.
# Fails with non-zero exit if p95 regresses by >10% versus baseline.
#
# Usage:
#   bash scripts/bench-check.sh           # run benchmark and check
#   bash scripts/bench-check.sh --skip    # skip (for local dev without bench)

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

# Extract baseline upper CI bound in microseconds.
# Note: baseline fields are labeled p50/p95/p99 but are actually
# lower_ci/mean/upper_ci from criterion's confidence interval.
BASELINE_P95=$(python3 -c "
import json, sys
with open('$BASELINE') as f:
    data = json.load(f)
print(data['lint_10kb']['p95_us'])
" 2>/dev/null || echo "")

if [[ -z "$BASELINE_P95" ]]; then
    echo "bench-check: ERROR — could not parse baseline p95 from $BASELINE"
    exit 1
fi

echo "bench-check: baseline upper CI = ${BASELINE_P95} µs"
echo "bench-check: running lint_latency benchmark..."

# Run benchmark and capture output
BENCH_OUTPUT=$(cargo bench -p marque-engine --bench lint_latency 2>&1)

# Parse criterion's time line: "lint_10kb  time:  [276.03 µs 280.01 µs 284.99 µs]"
# Extract the upper bound (third value + unit) as a conservative regression proxy.
TIME_LINE=$(echo "$BENCH_OUTPUT" | grep "time:" | head -1)
echo "bench-check: criterion output: $TIME_LINE"

if [[ -z "$TIME_LINE" ]]; then
    echo "bench-check: ERROR — no 'time:' line found in criterion output"
    echo "$BENCH_OUTPUT"
    exit 1
fi

# Extract the last "number unit" pair (upper bound of CI)
UPPER_VAL=$(echo "$TIME_LINE" | grep -oP '[\d.]+\s*[µnm]s' | tail -1 || echo "")

if [[ -z "$UPPER_VAL" ]]; then
    echo "bench-check: ERROR — could not parse timing from criterion output"
    echo "$BENCH_OUTPUT"
    exit 1
fi

# Convert to microseconds
VALUE=$(echo "$UPPER_VAL" | grep -oP '[\d.]+')
UNIT=$(echo "$UPPER_VAL" | grep -oP '[µnm]s')

if [[ "$UNIT" == "ns" ]]; then
    CURRENT_US=$(python3 -c "print(int($VALUE / 1000))")
elif [[ "$UNIT" == "µs" ]]; then
    CURRENT_US=$(python3 -c "print(int($VALUE))")
elif [[ "$UNIT" == "ms" ]]; then
    CURRENT_US=$(python3 -c "print(int($VALUE * 1000))")
else
    echo "bench-check: ERROR — unexpected unit: $UNIT"
    exit 1
fi

echo "bench-check: measured upper CI = ${CURRENT_US} µs"

# Check for >10% regression vs baseline
THRESHOLD=$(python3 -c "print(int($BASELINE_P95 * 1.10))")
echo "bench-check: regression threshold (baseline + 10%) = ${THRESHOLD} µs"

if [[ "$CURRENT_US" -gt "$THRESHOLD" ]]; then
    echo "bench-check: FAIL — regressed: ${CURRENT_US} µs > ${THRESHOLD} µs (baseline: ${BASELINE_P95} µs)"
    exit 1
fi

# Check absolute SC-001 target
TARGET_P95=16000
if [[ "$CURRENT_US" -gt "$TARGET_P95" ]]; then
    echo "bench-check: FAIL — SC-001 target exceeded: ${CURRENT_US} µs > ${TARGET_P95} µs"
    exit 1
fi

echo "bench-check: PASS — ${CURRENT_US} µs <= ${THRESHOLD} µs (baseline + 10%), well under ${TARGET_P95} µs SC-001 target"
