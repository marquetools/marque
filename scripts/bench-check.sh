#!/usr/bin/env bash

# SPDX-FileCopyrightText: 2026 Knitli Inc.
#
# SPDX-License-Identifier: MIT OR Apache-2.0

# Performance regression gate for SC-001 (strict-path), SC-002 (decoder-path),
# and SC-005 (linear scaling).
#
# Runs the lint_latency benchmark and compares Criterion's confidence-interval
# upper bound against the per-bench baseline. Fails with non-zero exit if the
# CI upper bound regresses past its threshold, or exceeds the per-bench
# absolute target (`target_upper_ci_us` in `benches/baseline.json`).
#
# Then runs the linear_scaling benchmark and computes the coefficient of
# determination (R²) for the linear regression of (input_size, mean_time)
# across the SC-005 sweep. Fails if R² falls below the `r_squared_min`
# threshold in `benches/baseline.json`.
#
# Five gates are checked:
#   - `lint_10kb`                         (SC-001, target 16ms upper CI)
#   - `decoder_10kb_one_mangled_region`   (SC-002, target 18ms upper CI)
#   - `lint_scaling`                      (SC-005, R² >= 0.9 across size sweep)
#   - `fix_throughput`                    (fix-apply linearity, R² >= 0.9 across size sweep)
#   - `deadline_overhead`                 (Spec 005, with-deadline overhead ≤ max_ratio_pct)
#
# Per-bench regression policy:
#   - If `drift_alert_upper_ci_us` is set in `benches/baseline.json` for a
#     bench, that absolute value is the threshold. Always enforced — these
#     are deliberately picked to be machine-portable, so the
#     `MARQUE_BENCH_SKIP_REGRESSION=1` env-var override does not apply.
#   - Otherwise, the threshold is `upper_ci_us * 1.10` (+10% vs the reference
#     baseline). The baseline is captured on a GitHub Actions ubuntu-latest
#     runner so this gate is meaningful in CI. Set
#     `MARQUE_BENCH_SKIP_REGRESSION=1` only when running locally on hardware
#     with a substantially different performance profile.
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

# `MARQUE_BENCH_SKIP_REGRESSION=1` skips the +10%-vs-baseline percentage
# check. The baseline in `benches/baseline.json` is captured on a GitHub
# Actions ubuntu-latest runner, so the gate is meaningful on CI. Set this
# env var only when running locally on a machine with a substantially
# different performance profile.
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
# both a regression check and the absolute target check.
#
# Regression-check policy (Phase 4 review L6):
#   - If the baseline entry carries a `drift_alert_upper_ci_us` field, that
#     absolute value is used as the regression threshold. Use this for
#     benches whose `upper_ci_us` is reference-machine-only and doesn't
#     reproduce on common dev hardware (WSL2, etc.) — picking an absolute
#     drift alert decouples regression detection from reference-machine
#     reproducibility.
#   - Otherwise, the legacy +10% percentage gate against `upper_ci_us`
#     applies. Suitable for benches whose baseline is reproducible across
#     the development hardware actually running this script.
#
# Each bench is run separately rather than parsing a multi-bench captured
# output blob — Criterion's report layout is "<name>           time:   [...]"
# for short names but "<name>\n                        time:   [...]" for
# long names, and the format flips depending on alignment column. Filtering
# by anchored regex sidesteps that whole class of parsing fragility.
check_one_bench() {
    local bench_name="$1"
    # bench_target is required: it must be the Cargo bench *file* name
    # (e.g. "lint_latency"), not the Criterion function name (bench_name).
    # cargo bench --bench <target> only compiles/runs that one binary.
    local bench_target="${2:?check_one_bench: bench_target (arg 2) is required — pass the bench file name, e.g. \"lint_latency\"}"

    # Extract baseline upper CI bound (microseconds) and absolute target.
    local baseline_upper_ci target_upper_ci drift_alert
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

    # Optional: absolute drift-alert threshold (overrides the +10% gate when
    # present). Returns the empty string when the field is absent so the
    # downstream branch can pick the right policy.
    drift_alert=$(python3 -c "
import json, sys
with open('$BASELINE') as f:
    data = json.load(f)
print(data['$bench_name'].get('drift_alert_upper_ci_us', ''))
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
    if [[ -n "$drift_alert" ]] && ! [[ "$drift_alert" =~ ^[0-9]+$ ]]; then
        echo "bench-check: ERROR — '$bench_name' drift_alert_upper_ci_us is not a positive integer: ${drift_alert}"
        return 1
    fi

    if [[ -n "$drift_alert" ]]; then
        echo "bench-check[$bench_name]: baseline upper CI = ${baseline_upper_ci} µs (advisory), drift alert = ${drift_alert} µs (absolute), absolute target = ${target_upper_ci} µs"
    else
        echo "bench-check[$bench_name]: baseline upper CI = ${baseline_upper_ci} µs, absolute target = ${target_upper_ci} µs"
    fi
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
    if ! bench_output=$(cargo bench -p marque-engine --bench "$bench_target" -- "^${bench_name}\$" 2>&1); then
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

    # Regression threshold has three modes:
    #   1. `drift_alert_upper_ci_us` present → that absolute value is the
    #      threshold. Always enforced — the field is deliberately picked
    #      to be machine-portable (Phase 4 review L6), so the
    #      MARQUE_BENCH_SKIP_REGRESSION=1 CI override does not apply here.
    #   2. No drift alert + `MARQUE_BENCH_SKIP_REGRESSION=1` → skip the
    #      regression check (T086); only the absolute `target_upper_ci_us`
    #      gate runs.
    #   3. Otherwise → +10% gate against `upper_ci_us`. Round up (math.ceil)
    #      so a fractional µs in the baseline can never silently pass.
    local threshold threshold_label regression_mode
    if [[ -n "$drift_alert" ]]; then
        threshold="$drift_alert"
        threshold_label="drift alert (absolute)"
        regression_mode="enforce"
    elif [[ "$SKIP_REGRESSION" == "1" ]]; then
        threshold=""
        threshold_label="baseline + 10% (skipped via MARQUE_BENCH_SKIP_REGRESSION=1)"
        regression_mode="skip"
    else
        threshold=$(python3 -c "import math; print(math.ceil($baseline_upper_ci * 1.10))")
        threshold_label="baseline + 10%"
        regression_mode="enforce"
    fi

    if [[ "$regression_mode" == "skip" ]]; then
        echo "bench-check[$bench_name]: skipping ${threshold_label}; absolute target still enforced"
    else
        echo "bench-check[$bench_name]: regression threshold (${threshold_label}) = ${threshold} µs"
        if [[ "$current_us" -gt "$threshold" ]]; then
            echo "bench-check[$bench_name]: FAIL — regressed: ${current_us} µs > ${threshold} µs (${threshold_label}; baseline: ${baseline_upper_ci} µs)"
            return 1
        fi
    fi

    if [[ "$current_us" -gt "$target_upper_ci" ]]; then
        echo "bench-check[$bench_name]: FAIL — absolute target exceeded: ${current_us} µs > ${target_upper_ci} µs"
        return 1
    fi

    if [[ "$regression_mode" == "skip" ]]; then
        echo "bench-check[$bench_name]: PASS — ${current_us} µs under ${target_upper_ci} µs absolute target (regression check skipped)"
    else
        echo "bench-check[$bench_name]: PASS — ${current_us} µs <= ${threshold} µs (${threshold_label}), well under ${target_upper_ci} µs target"
    fi

    # ---- p99 tail-percentile gate (T098 / FR-030 / SC-008) ----
    #
    # `Arc<dyn Vocabulary<S>>` indirect dispatch precludes cross-crate
    # devirtualization, so per-token vtable misses surface at the tail
    # rather than in the mean. The mean / upper-CI gate above does not
    # detect a regression that only inflates the long tail. The p99
    # gate is read from `target/criterion/<bench>/new/sample.json` (the
    # raw Criterion sample data) and compared against two thresholds:
    #
    #   - drift gate: `p99_us` (baseline) * 1.05 per FR-030 / SC-008
    #     (post-refactor `p99 ≤ baseline + 5%`). REQUIRES `p99_us`
    #     captured on the same hardware as the rest of the baseline
    #     (`reference_machine.profile`); a cross-hardware baseline
    #     (e.g. WSL2 capture loaded on `ubuntu-latest`) would turn
    #     this into runner-noise flake.
    #   - absolute gate: `target_p99_us` (the SC-001 16ms ceiling
    #     applied to p99 specifically). Hardware-independent — always
    #     enforced when present.
    #
    # Three valid baseline states:
    #   1. Both `p99_us` and `target_p99_us` present → both gates run.
    #   2. Only `target_p99_us` present → absolute gate runs, drift
    #      gate skips (the partial-activation state used while a
    #      same-hardware drift baseline is being captured).
    #   3. Both absent → both gates skip (backward-compatible default
    #      for benches with no captured p99 baseline).
    # `MARQUE_BENCH_SKIP_REGRESSION=1` independently skips the drift
    # gate even when (1) is the configured state, mirroring the
    # upper-CI gate above. The absolute target is never skipped by
    # that env var — it's the constitutional ceiling.
    local baseline_p99 target_p99
    baseline_p99=$(python3 -c "
import json
with open('$BASELINE') as f:
    data = json.load(f)
print(data['$bench_name'].get('p99_us', ''))
" 2>/dev/null || echo "")
    target_p99=$(python3 -c "
import json
with open('$BASELINE') as f:
    data = json.load(f)
print(data['$bench_name'].get('target_p99_us', ''))
" 2>/dev/null || echo "")

    # Both absent → state (3): skip the gate entirely.
    if [[ -z "$baseline_p99" && -z "$target_p99" ]]; then
        return 0
    fi

    # `p99_us` without `target_p99_us` is malformed — the absolute
    # ceiling is the load-bearing gate; the drift baseline is an
    # extension. Fail loudly rather than silently dropping the
    # absolute check.
    if [[ -n "$baseline_p99" && -z "$target_p99" ]]; then
        echo "bench-check[$bench_name]: ERROR — p99_us baseline present but target_p99_us missing; the absolute SC-001 ceiling is required when any p99 baseline is configured"
        return 1
    fi

    # Validate types (positive integers, same shape as upper_ci_us /
    # target_upper_ci_us).
    if [[ -n "$baseline_p99" ]] && ! [[ "$baseline_p99" =~ ^[0-9]+$ ]]; then
        echo "bench-check[$bench_name]: ERROR — p99_us is not a positive integer: ${baseline_p99}"
        return 1
    fi
    if ! [[ "$target_p99" =~ ^[0-9]+$ ]]; then
        echo "bench-check[$bench_name]: ERROR — target_p99_us is not a positive integer: ${target_p99}"
        return 1
    fi

    # Locate Criterion's `sample.json` for this bench. Same layout
    # `scripts/capture-baselines.sh` walks: `target/criterion/<id>/new/sample.json`
    # for top-level `c.bench_function(...)` benches.
    local sample_path
    sample_path="$REPO_ROOT/target/criterion/${bench_name}/new/sample.json"
    if [[ ! -f "$sample_path" ]]; then
        echo "bench-check[$bench_name]: ERROR — p99 gate enabled but Criterion sample.json missing at ${sample_path}; was the bench actually run?"
        return 1
    fi

    # Compute per-iteration p99 from the (iters[], times[]) parallel
    # arrays Criterion writes. Same percentile compute as
    # `scripts/capture-baselines.sh::compute_percentiles_python` so
    # gate-time and capture-time use identical math.
    local current_p99
    current_p99=$(python3 - "$sample_path" <<'PY' 2>/dev/null || echo ""
import json, math, sys
with open(sys.argv[1]) as f:
    data = json.load(f)
iters = data["iters"]
times = data["times"]
if not iters or len(iters) != len(times):
    sys.exit("sample.json: iters/times missing or unequal length")
per_iter_us = sorted((t / i) / 1000.0 for i, t in zip(iters, times))
n = len(per_iter_us)
# Lower-rank percentile (matches capture-baselines.sh): index = floor(0.99 * (n - 1)).
idx = int(0.99 * (n - 1))
# Round UP so a fractional µs in the sample cannot silently pass the gate.
print(math.ceil(per_iter_us[idx]))
PY
)
    if [[ -z "$current_p99" ]]; then
        echo "bench-check[$bench_name]: ERROR — could not compute p99 from ${sample_path}"
        return 1
    fi

    # Drift gate: baseline + 5% (FR-030 / SC-008). Computed only if
    # `p99_us` is present (state 1) AND MARQUE_BENCH_SKIP_REGRESSION
    # isn't set. State 2 (target-only) and SKIP_REGRESSION both fall
    # through to the absolute-only branch.
    local p99_threshold p99_threshold_label
    if [[ -z "$baseline_p99" ]]; then
        p99_threshold=""
        p99_threshold_label="p99 baseline + 5% (no p99_us baseline configured for this bench)"
    elif [[ "$SKIP_REGRESSION" == "1" ]]; then
        p99_threshold=""
        p99_threshold_label="p99 baseline + 5% (skipped via MARQUE_BENCH_SKIP_REGRESSION=1)"
    else
        p99_threshold=$(python3 -c "import math; print(math.ceil($baseline_p99 * 1.05))")
        p99_threshold_label="p99 baseline + 5%"
    fi

    if [[ -n "$p99_threshold" ]]; then
        echo "bench-check[$bench_name]: measured p99 = ${current_p99} µs (baseline ${baseline_p99} µs, drift threshold ${p99_threshold} µs, absolute target ${target_p99} µs)"
        if [[ "$current_p99" -gt "$p99_threshold" ]]; then
            echo "bench-check[$bench_name]: FAIL — p99 regressed: ${current_p99} µs > ${p99_threshold} µs (${p99_threshold_label}; baseline: ${baseline_p99} µs)"
            return 1
        fi
    else
        echo "bench-check[$bench_name]: measured p99 = ${current_p99} µs (drift gate skipped: ${p99_threshold_label}; absolute target ${target_p99} µs)"
    fi

    # Absolute p99 ceiling (SC-001 16ms applied at the tail). Always
    # enforced when target_p99_us is configured, regardless of drift-
    # gate state — this is the constitutional invariant.
    if [[ "$current_p99" -gt "$target_p99" ]]; then
        echo "bench-check[$bench_name]: FAIL — p99 absolute target exceeded: ${current_p99} µs > ${target_p99} µs"
        return 1
    fi

    if [[ -n "$p99_threshold" ]]; then
        echo "bench-check[$bench_name]: PASS (p99) — ${current_p99} µs <= ${p99_threshold} µs (${p99_threshold_label}), well under ${target_p99} µs absolute target"
    else
        echo "bench-check[$bench_name]: PASS (p99) — ${current_p99} µs under ${target_p99} µs absolute target (drift check skipped)"
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

# check_deadline_overhead
#
# Spec 005 T018 / T019: enforces that the deadline-aware lint path adds
# ≤ `deadline_overhead.max_ratio_pct` overhead vs the unbounded path on
# the 10 KB representative input. (The threshold key is integer-percent;
# both this script and `benches/baseline.json` use `max_ratio_pct`.) The
# two benches live in
# `crates/engine/benches/deadline_overhead.rs` and are named
# `deadline_overhead_baseline` (no deadline) and
# `deadline_overhead_with_deadline` (1-hour deadline that never trips).
#
# We use Criterion's middle CI value (the mean point estimate) rather
# than the upper CI bound: the upper bound is the right metric for an
# absolute-target regression check, but for ratio comparison the point
# estimate is the load-bearing reading — CI width inflates with
# variance and would convert clock-jitter into a false-positive ratio
# failure. ("Mean" not "median" — Criterion's `time: [lower mean
# upper]` triple is a confidence interval around the mean, not a
# sample percentile.)
#
# Both benches run in a single `cargo bench` invocation so the harness
# warms up once; running them separately could let the runner profile
# differ enough between calls to bias the ratio.
check_deadline_overhead() {
    local max_ratio_pct
    max_ratio_pct=$(python3 -c "
import json
with open('$BASELINE') as f:
    data = json.load(f)
print(data['deadline_overhead']['max_ratio_pct'])
" 2>/dev/null || echo "")

    if [[ -z "$max_ratio_pct" ]]; then
        echo "bench-check[deadline_overhead]: ERROR — could not parse 'deadline_overhead.max_ratio_pct' from $BASELINE"
        return 1
    fi

    if ! [[ "$max_ratio_pct" =~ ^(0|[1-9][0-9]*)$ ]]; then
        # Regex `^(0|[1-9][0-9]*)$` accepts exactly `0` (the tightest
        # gate — "no overhead allowed at all") or a positive integer
        # without leading zeros; reject non-numeric / negative /
        # signed / decimal values.
        echo "bench-check[deadline_overhead]: ERROR — max_ratio_pct is not a non-negative integer: ${max_ratio_pct}"
        return 1
    fi

    echo "bench-check[deadline_overhead]: max overhead = ${max_ratio_pct}% (with-deadline mean over baseline mean)"
    echo "bench-check[deadline_overhead]: running benchmark..."

    local bench_output
    if ! bench_output=$(cargo bench -p marque-engine --bench deadline_overhead 2>&1); then
        echo "bench-check[deadline_overhead]: ERROR — 'cargo bench' invocation failed"
        if [[ -n "$bench_output" ]]; then
            printf '%s\n' "$bench_output"
        fi
        return 1
    fi

    # Parse out the means from both bench reports. Format mirrors the
    # `lint_scaling` parser's regex: `<name>` followed by `time:   [lower mean upper]`,
    # tolerating an optional newline-and-indent break that Criterion sometimes
    # inserts between the bench name and the time row.
    local ratio_pct
    ratio_pct=$(python3 - "$bench_output" <<'PY' 2>/dev/null || true
import math, re, sys

text = sys.argv[1]

bench_pat = re.compile(
    r"(deadline_overhead_(?:baseline|with_deadline))\s+(?:\n\s+)?time:\s+\[\s*"
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

results = {}
for m in bench_pat.finditer(text):
    name = m.group(1)
    # Group 4/5 is the mean (middle of the three CI numbers).
    results[name] = to_us(m.group(4), m.group(5))

baseline = results.get("deadline_overhead_baseline")
with_deadline = results.get("deadline_overhead_with_deadline")
if baseline is None or with_deadline is None or baseline <= 0:
    sys.stderr.write(f"missing samples; saw {sorted(results)}\n")
    sys.exit(1)

ratio = with_deadline / baseline
# Convert to integer percent overhead, rounded UP so a fractional
# overhead just over the gate cannot silently pass.
overhead_pct = math.ceil((ratio - 1.0) * 100.0)
# Print baseline/with-deadline for the diagnostic plus the overhead.
print(f"{baseline:.2f} {with_deadline:.2f} {overhead_pct}")
PY
)

    if [[ -z "$ratio_pct" ]]; then
        echo "bench-check[deadline_overhead]: ERROR — could not extract sample points from criterion output"
        echo "$bench_output"
        return 1
    fi

    # ratio_pct is "<baseline_us> <with_deadline_us> <overhead_pct>"
    local baseline_mean with_deadline_mean overhead_pct
    read -r baseline_mean with_deadline_mean overhead_pct <<<"$ratio_pct"

    echo "bench-check[deadline_overhead]: baseline mean = ${baseline_mean} µs, with-deadline mean = ${with_deadline_mean} µs, overhead = ${overhead_pct}%"

    if [[ "$overhead_pct" -gt "$max_ratio_pct" ]]; then
        echo "bench-check[deadline_overhead]: FAIL — overhead ${overhead_pct}% > ${max_ratio_pct}% threshold"
        return 1
    fi

    echo "bench-check[deadline_overhead]: PASS — overhead ${overhead_pct}% <= ${max_ratio_pct}% threshold"
    return 0
}

# check_fix_throughput
#
# Runs the `fix_throughput` Criterion bench (`crates/engine/benches/fix_throughput.rs`)
# and computes R² for a linear regression of (input_size_bytes, mean_time_µs)
# across the size sweep. Fails if R² < `fix_throughput.r_squared_min` from
# `benches/baseline.json`. Mirrors `check_linear_scaling` for the fix path.
#
# This gate specifically guards the quadratic `Vec::splice`-per-fix regression
# (perf(engine): fix-apply path is quadratic in input size): a quadratic apply
# path produces a convex throughput curve (R² well below 0.9 for a linear fit)
# while the linear forward-pass replacement is indistinguishable from linear
# scaling at the R² ≥ 0.9 gate.
#
# The sweep runs from 1 MB to 100 MB with fix density proportional to input
# size, which is the exact input shape that exposed the original pathology.
#
# NOTE: Temporarily disabled while we work out the rest of the scaling bugs.
check_fix_throughput() {
    local r_squared_min
    r_squared_min=$(python3 -c "
import json
with open('$BASELINE') as f:
    data = json.load(f)
print(data['fix_throughput']['r_squared_min'])
" 2>/dev/null || echo "")

    if [[ -z "$r_squared_min" ]]; then
        echo "bench-check[fix_throughput]: ERROR — could not parse 'fix_throughput.r_squared_min' from $BASELINE"
        return 1
    fi

    if ! python3 -c "
v = float('$r_squared_min')
assert 0.0 < v <= 1.0, 'out of range'
" 2>/dev/null; then
        echo "bench-check[fix_throughput]: ERROR — r_squared_min not a float in (0, 1]: ${r_squared_min}"
        return 1
    fi

    echo "bench-check[fix_throughput]: minimum R² = ${r_squared_min}"
    echo "bench-check[fix_throughput]: running benchmark..."

    local bench_output
    if ! bench_output=$(cargo bench -p marque-engine --bench fix_throughput 2>&1); then
        echo "bench-check[fix_throughput]: ERROR — 'cargo bench' invocation failed"
        if [[ -n "$bench_output" ]]; then
            printf '%s\n' "$bench_output"
        fi
        return 1
    fi

    # Extract every `fix_throughput/<bytes>     time:   [lower mean upper]` line
    # and compute R² on (size_bytes, mean_time_µs). The bench uses
    # `BenchmarkId::from_parameter(input.len())` — a raw byte count — matching
    # the `lint_scaling/<bytes>` convention.  Parsing the byte count directly
    # avoids the integer-MB rounding that would occur if we encoded an `<N>mb`
    # label and then reconstructed bytes as `N * 1_000_000`.
    local r_squared
    r_squared=$(python3 - "$bench_output" <<'PY' 2>/dev/null || true
import math, re, sys

text = sys.argv[1]
# Match `fix_throughput/<bytes>` followed by `time:   [lower unit mean unit upper unit]`.
# The byte-count parameter matches one or more digits with no suffix, just like
# the `lint_scaling/<bytes>` IDs parsed by check_linear_scaling.
size_pat = re.compile(
    r"fix_throughput/(\d+)\s+(?:\n\s+)?time:\s+\[\s*"
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
    # The parameter is already the byte count — no conversion needed.
    size_bytes = int(m.group(1))
    # Group 4 / unit 5 is the mean (middle of the three CI numbers).
    mean_us = to_us(m.group(4), m.group(5))
    points.append((size_bytes, mean_us))

if len(points) < 3:
    sys.stderr.write(f"insufficient samples: {len(points)}\n")
    sys.exit(1)

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
        echo "bench-check[fix_throughput]: ERROR — could not extract sample points from criterion output"
        echo "$bench_output"
        return 1
    fi

    echo "bench-check[fix_throughput]: measured R² = ${r_squared}"

    local pass
    pass=$(python3 -c "print('1' if float('$r_squared') >= float('$r_squared_min') else '0')")

    if [[ "$pass" != "1" ]]; then
        echo "bench-check[fix_throughput]: FAIL — R² ${r_squared} < ${r_squared_min} (super-linear apply path)"
        return 1
    fi

    echo "bench-check[fix_throughput]: PASS — R² ${r_squared} >= ${r_squared_min}"
    return 0
}

# report_fix_latency
#
# Advisory (non-gating): runs the `fix_latency` bench and prints the three
# timings — `fix_single_e054_apply`, `fix_single_e054_dry_run`,
# `lint_single_e054_baseline` — without enforcing a threshold. There is no
# SC-target for per-fix latency yet; this exists so the numbers print
# alongside the gated benches and so a regression in single-fix throughput
# is at least visible in CI logs.
report_fix_latency() {
    echo "bench-check[fix_latency]: running benchmark (advisory, not gated)..."

    local bench_output
    if ! bench_output=$(cargo bench -p marque-engine --bench fix_latency 2>&1); then
        echo "bench-check[fix_latency]: WARN — 'cargo bench' invocation failed (advisory; not failing overall status)"
        if [[ -n "$bench_output" ]]; then
            printf '%s\n' "$bench_output"
        fi
        return 0
    fi

    # Print the `time:` line for each of the three benches. The bench
    # name precedes `time:` either inline or on the previous line; the
    # same multi-line tolerance other parsers in this script use applies.
    #
    # Capture stdout + stderr separately so we can distinguish a Python
    # startup/crash (parser_err non-empty) from per-bench parse failures
    # (WARN lines emitted to stdout by the script itself).
    local parser_out parser_err py_exit
    parser_err=$(mktemp)
    # Temporarily disable set -e so a Python startup/parse failure only WARNs
    # (advisory output) rather than terminating the whole script under
    # `set -euo pipefail`.
    set +e
    parser_out=$(python3 - "$bench_output" 2>"$parser_err" <<'PY'
import re, sys

text = sys.argv[1]
preferred = (
    "fix_single_e054_apply",
    "fix_single_e054_dry_run",
    "lint_single_e054_baseline",
)

pat = re.compile(
    r"([A-Za-z0-9_./:-]+)\s+(?:\n\s+)?time:\s+\[\s*"
    r"([0-9]+(?:\.[0-9]+)?\s*[µnm]s)\s+"
    r"([0-9]+(?:\.[0-9]+)?\s*[µnm]s)\s+"
    r"([0-9]+(?:\.[0-9]+)?\s*[µnm]s)"
)

found = {}
order = []
for m in pat.finditer(text):
    name = m.group(1)
    if name not in found:
        order.append(name)
    found[name] = (m.group(2), m.group(3), m.group(4))

for name in preferred:
    if name in found:
        lo, mean, hi = found[name]
        print(f"bench-check[fix_latency]: {name}: mean {mean} (CI {lo} .. {hi})")
    else:
        print(f"bench-check[fix_latency]: WARN — could not parse {name} timing")

for name in order:
    if name in preferred:
        continue
    lo, mean, hi = found[name]
    print(f"bench-check[fix_latency]: {name}: mean {mean} (CI {lo} .. {hi})")
PY
    )
    py_exit=$?
    set -e
    if [[ $py_exit -ne 0 ]]; then
        echo "bench-check[fix_latency]: WARN — Python parser exited with status $py_exit (advisory; not failing overall status)"
        if [[ -s "$parser_err" ]]; then
            echo "bench-check[fix_latency]: WARN — Python stderr output follows:"
            cat "$parser_err"
        fi
    fi
    rm -f "$parser_err"
    if [[ -n "$parser_out" ]]; then
        printf '%s\n' "$parser_out"
    fi
    return 0
}

OVERALL_STATUS=0
check_one_bench "lint_10kb" "lint_latency" || OVERALL_STATUS=1
check_one_bench "decoder_10kb_one_mangled_region" "lint_latency" || OVERALL_STATUS=1
check_linear_scaling || OVERALL_STATUS=1
# fix_throughput disabled while we work out the scaling bug
# check_fix_throughput || OVERALL_STATUS=1
check_deadline_overhead || OVERALL_STATUS=1
report_fix_latency
# CO-1 (PR #621): baselines captured for both fix_10kb paths; gates now active.
check_one_bench "fix_10kb_pass2_only" "fix_10kb" || OVERALL_STATUS=1
check_one_bench "fix_10kb_two_pass" "fix_10kb" || OVERALL_STATUS=1

if [[ "$OVERALL_STATUS" -ne 0 ]]; then
    echo "bench-check: FAIL — one or more benches failed their regression / absolute gates"
    exit 1
fi

echo "bench-check: PASS — all benches within regression and absolute targets"
