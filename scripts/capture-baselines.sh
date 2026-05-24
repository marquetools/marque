#!/usr/bin/env bash

# SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
#
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

# scripts/capture-baselines.sh — Capture pre-refactor benchmark baselines.
#
# This is the capture mechanism for T001 of the engine-rule refactor
# (specs/006-engine-rule-refactor) and the implementation of the R-5
# decision in that spec's research.md (FR-030..FR-033, plan deliverable
# D8). It is intended to be run by the bench-runner owner on pinned
# hardware (per D8) — NOT by every contributor on every push, and NOT
# inside CI. The output JSON is the immutable baseline against which
# `scripts/bench-check.sh` (and successor regression gates) compare in
# subsequent refactor PRs.
#
# Workflow:
#   1. Bench-runner owner checks out the PR-0 commit on a GitHub
#      Actions ubuntu-latest hosted runner (per the D8 amendment;
#      hardware pinning beyond the runner family is out of project
#      budget).
#   2. Runs `bash scripts/capture-baselines.sh`.
#   3. Reviews the resulting `benches/baselines/2026-05-pre-refactor.json`
#      and fills in any free-form `reference_machine.*` fields the
#      script could not infer (e.g. CPU model string, host kernel).
#   4. Commits the populated JSON in a separate, reviewed commit.
#
# Recommended baseline-robustness procedure (per D8 owner observation):
#   On the marque project's GitHub-hosted runner family, observed
#   bench drift on prior PRs has routinely reached 10% and at times
#   tipped into 11% — a known baseline-quality signal, not a runtime
#   regression. To capture a baseline that the FR-050 ≤10% cumulative
#   gate can hold against, the bench-runner owner SHOULD:
#     a. Run this script multiple times across the calendar day,
#        including known-busy windows (e.g., US business hours when
#        the GitHub-hosted runner pool is most contended) and quiet
#        windows. 5–10 captures is a reasonable starting point.
#     b. Aggregate the captures using one of:
#          - Worst-observed (conservative — every per-bench p95 is
#            the slowest seen across the captures).
#          - Median across captures (robust).
#          - Slowest-decile per-bench across captures (adversarial,
#            biases the baseline toward the noisy upper tail).
#        The choice of aggregation is a one-time decision; record it
#        in the JSON's `_note` field so subsequent FR-050 evaluations
#        compare against the same shape.
#     c. Re-capture if a runner-image rotation produces clearly
#        anomalous deltas (per D8).
#   This script runs ONE capture per invocation; the multi-capture
#   aggregation is currently a manual procedure (or a wrapper script
#   the owner may add later). Multi-capture support is intentionally
#   not in this script today — the choice of aggregation strategy is
#   policy, and policy belongs in the owner's decision record (D8),
#   not in the capture mechanism.
#
# What it does:
#   * Runs each Criterion bench listed in `BENCH_TARGETS` once via
#     `cargo bench` (no filter — captures every bench function in the
#     target).
#   * Walks `target/criterion/` for the bench-id directories produced
#     by those harnesses, reads `estimates.json` (Criterion's mean
#     point estimate) and `sample.json` (raw `iters[]` + `times[]`
#     arrays, both nanoseconds) for each, and computes per-iteration
#     timings in microseconds.
#   * Aggregates p50/p95/p99 percentiles + mean + sample count for
#     every bench-id into a single JSON document at
#     `benches/baselines/2026-05-pre-refactor.json` matching the
#     `marque-bench-baseline-1.0` schema.
#   * Captures `git rev-parse HEAD`, ISO-8601 UTC timestamp, and
#     auto-discoverable `reference_machine.*` fields.
#
# Properties:
#   * Idempotent: re-running overwrites the output JSON. Never auto-
#     commits — the bench-runner owner reviews and commits manually.
#   * Portable shell: POSIX-ish bash + `jq` + `awk`. If `jq` is missing
#     it falls back to a `python3` one-liner; if both are missing the
#     script exits non-zero with a clear message.
#   * `set -euo pipefail` everywhere; every error path prints a
#     diagnostic and exits non-zero.
#
# Schema produced (per research.md R-5 lines 247–250):
#   {
#     "schema": "marque-bench-baseline-1.0",
#     "captured_at": "<ISO-8601 UTC>",
#     "git_sha": "<HEAD>",
#     "reference_machine": {
#       "cpu": "...",
#       "profile": "...",
#       "rust_version": "...",
#       "criterion_version": "...",
#       "host_kernel": "...",
#       "bench_runner_owner": "..."
#     },
#     "benches": [
#       {
#         "bench": "<group/id> or <fn-name>",
#         "p50": <microseconds>,
#         "p95": <microseconds>,
#         "p99": <microseconds>,
#         "mean": <microseconds>,
#         "samples": <int>,
#         "criterion_estimates_path": "target/criterion/.../estimates.json"
#       },
#       ...
#     ]
#   }

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CRITERION_DIR="$REPO_ROOT/target/criterion"
OUTPUT_PATH="$REPO_ROOT/benches/baselines/2026-05-pre-refactor.json"

# The set of bench targets to capture. These are the Cargo `--bench`
# names (one binary per file under `crates/engine/benches/*.rs`); the
# resulting Criterion bench-id directories under `target/criterion/`
# are walked after the run completes. Extending this list is the only
# change needed when a new bench is added to the captured set.
BENCH_TARGETS=(
    lint_latency
    fix_throughput
    fix_latency
    linear_scaling
    deadline_overhead
    decoder_10kb_rel_to_invariant
    decoder_trigraph_priors
)

# ---------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------

die() {
    printf 'capture-baselines: ERROR — %s\n' "$*" >&2
    exit 1
}

info() {
    printf 'capture-baselines: %s\n' "$*"
}

# Detect the JSON-extraction backend once. `jq` is the preferred path;
# `python3` is the documented fallback. Either tool can read both
# `estimates.json` (Criterion's point estimates) and `sample.json` (raw
# per-batch sample data) and compute percentiles. The downstream
# helpers branch on `$JSON_BACKEND`.
detect_json_backend() {
    if command -v jq >/dev/null 2>&1; then
        JSON_BACKEND=jq
        return
    fi
    if command -v python3 >/dev/null 2>&1; then
        JSON_BACKEND=python3
        return
    fi
    die "neither 'jq' nor 'python3' is available; one is required to read Criterion JSON"
}

# Auto-discover reference_machine fields. The CPU model string and
# host kernel are best-effort: on Linux they come from /proc/cpuinfo
# and `uname`; on macOS they come from `sysctl`/`uname`. Anything we
# cannot detect is left as `null` for the bench-runner owner to fill
# in manually before committing.
detect_cpu_model() {
    if [[ -r /proc/cpuinfo ]]; then
        awk -F': ' '/^model name/ { print $2; exit }' /proc/cpuinfo
        return
    fi
    if command -v sysctl >/dev/null 2>&1; then
        sysctl -n machdep.cpu.brand_string 2>/dev/null && return
    fi
    printf ''
}

detect_host_kernel() {
    if command -v uname >/dev/null 2>&1; then
        uname -srm
        return
    fi
    printf ''
}

detect_rust_version() {
    if command -v rustc >/dev/null 2>&1; then
        rustc --version
        return
    fi
    printf ''
}

# Read the criterion crate version from Cargo.lock so the recorded
# version matches what actually ran. Falls back to the empty string if
# the lock file is missing or the entry is not present (e.g. fresh
# checkout that never ran `cargo build`).
detect_criterion_version() {
    local lock="$REPO_ROOT/Cargo.lock"
    if [[ ! -r "$lock" ]]; then
        printf ''
        return
    fi
    awk '
        /^\[\[package\]\]/ { in_pkg = 1; name = ""; version = ""; next }
        in_pkg && /^name = "criterion"/ { name = "criterion"; next }
        in_pkg && /^version = / {
            gsub(/version = |"/, "")
            version = $0
            next
        }
        /^$/ {
            if (in_pkg && name == "criterion" && version != "") {
                print version
                exit
            }
            in_pkg = 0
        }
    ' "$lock"
}

# ---------------------------------------------------------------------
# Bench execution
# ---------------------------------------------------------------------

# Run every bench target listed in BENCH_TARGETS in a single
# `cargo bench` invocation. Criterion writes its estimates+sample JSON
# to target/criterion/.../new/ as a side effect of each run; we walk
# those after the run completes.
#
# Reproducibility note: Criterion writes results under the benchmark
# *function or group name* (`criterion_group!` / `c.bench_function`),
# not under the Cargo bench-target file name — so the bench targets
# in `BENCH_TARGETS` are NOT a 1:1 prefix for the directory layout
# under `target/criterion/`. To keep the captured baseline reproducible
# from a fresh checkout (and to avoid folding in stale bench-ids from
# an earlier branch's run), we wipe `target/criterion/` before
# invoking `cargo bench` so the post-run walk finds *only* this run's
# output.
run_benches() {
    if [[ -d "$CRITERION_DIR" ]]; then
        info "wiping stale Criterion output at $CRITERION_DIR"
        rm -rf "$CRITERION_DIR"
    fi

    local -a cargo_args=(bench --workspace)
    local target
    for target in "${BENCH_TARGETS[@]}"; do
        cargo_args+=(--bench "$target")
    done

    info "running benches: ${BENCH_TARGETS[*]}"
    info "command: cargo ${cargo_args[*]}"

    if ! ( cd "$REPO_ROOT" && cargo "${cargo_args[@]}" ); then
        die "'cargo bench' failed; aborting before writing baseline JSON"
    fi
}

# ---------------------------------------------------------------------
# Per-bench JSON extraction
# ---------------------------------------------------------------------

# Read Criterion's `estimates.json` mean point estimate (nanoseconds)
# and convert to microseconds.
read_mean_us_jq() {
    local estimates_path="$1"
    jq -r '.mean.point_estimate / 1000.0' "$estimates_path"
}

read_mean_us_python() {
    local estimates_path="$1"
    python3 - "$estimates_path" <<'PY'
import json, sys
with open(sys.argv[1]) as f:
    data = json.load(f)
print(data["mean"]["point_estimate"] / 1000.0)
PY
}

# Compute per-iteration timings (microseconds) from Criterion's
# `sample.json`. The file contains parallel arrays `iters[]` and
# `times[]` (both length N), where each pair represents one Criterion
# batch: `iters[i]` is the inner-loop count and `times[i]` is the
# total nanoseconds for that batch. The per-iteration time is
# `times[i] / iters[i]` and the conversion to microseconds divides by
# 1000.
#
# Returns three space-separated microsecond values: p50 p95 p99 (each
# rounded to 3 decimals) and the sample count, in the form
# "p50 p95 p99 samples".
compute_percentiles_jq() {
    local sample_path="$1"
    # `. as $s` binds the root sample object so the array constructor
    # can index both `$s.times` and `$s.iters` in parallel — without
    # the binding, `range(...)` rewrites the pipeline input and the
    # subsequent `.times[.]` lookup fails with "Cannot index number".
    jq -r '
        . as $s
        | [ range(0; ($s.times | length)) | ($s.times[.] / $s.iters[.]) / 1000.0 ]
        | sort as $sorted
        | ($sorted | length) as $n
        | def pct(p):
            if $n == 0 then 0
            else $sorted[ ((p * ($n - 1)) | floor) ]
            end;
        [
            (pct(0.50) | . * 1000 | round / 1000),
            (pct(0.95) | . * 1000 | round / 1000),
            (pct(0.99) | . * 1000 | round / 1000),
            $n
        ]
        | @tsv
    ' "$sample_path"
}

compute_percentiles_python() {
    local sample_path="$1"
    python3 - "$sample_path" <<'PY'
import json, sys

with open(sys.argv[1]) as f:
    data = json.load(f)

iters = data["iters"]
times = data["times"]
if len(iters) != len(times) or not iters:
    sys.exit("sample.json: iters/times arrays empty or unequal length")

per_iter_us = sorted((t / i) / 1000.0 for i, t in zip(iters, times))
n = len(per_iter_us)

def pct(p):
    # Lower-rank percentile (matches the jq branch): index = floor(p*(n-1)).
    idx = int(p * (n - 1))
    return per_iter_us[idx]

print(
    "{:.3f}\t{:.3f}\t{:.3f}\t{}".format(pct(0.50), pct(0.95), pct(0.99), n)
)
PY
}

# Dispatch to the chosen JSON backend.
read_mean_us() {
    case "$JSON_BACKEND" in
        jq)      read_mean_us_jq "$1" ;;
        python3) read_mean_us_python "$1" ;;
        *)       die "unreachable: JSON_BACKEND=$JSON_BACKEND" ;;
    esac
}

compute_percentiles() {
    case "$JSON_BACKEND" in
        jq)      compute_percentiles_jq "$1" ;;
        python3) compute_percentiles_python "$1" ;;
        *)       die "unreachable: JSON_BACKEND=$JSON_BACKEND" ;;
    esac
}

# ---------------------------------------------------------------------
# Bench-id directory discovery
# ---------------------------------------------------------------------

# A "bench-id directory" is any directory under target/criterion/ that
# contains a `new/estimates.json` and a `new/sample.json`. This shape
# covers BOTH the flat `criterion/<fn>/new/` layout used by
# top-level `c.bench_function(...)` AND the nested
# `criterion/<group>/<id>/new/` layout used by sweep benches with
# `BenchmarkGroup::bench_with_input(BenchmarkId::from_parameter(...))`.
#
# The `report/` and `change/` subdirectories Criterion creates for its
# HTML output and run-over-run comparisons are filtered out by the
# requirement that BOTH `new/estimates.json` and `new/sample.json`
# exist.
#
# Emits one bench-id per line, formatted as the path RELATIVE to
# `target/criterion/`, with forward slashes. Examples:
#   lint_10kb
#   fix_throughput/100000000
#   lint_scaling/10000
discover_bench_ids() {
    if [[ ! -d "$CRITERION_DIR" ]]; then
        die "no target/criterion/ directory; did 'cargo bench' run?"
    fi
    # `run_benches` wipes `$CRITERION_DIR` before invoking `cargo bench`,
    # so every directory under it after the run belongs to THIS capture.
    # That lets us walk the whole tree without filtering by bench-target
    # name — Criterion writes results under benchmark *function/group
    # names* (e.g. `lint_10kb`, `decoder_10kb_one_mangled_region`,
    # `fix_throughput/<bytes>`) which do NOT match the Cargo bench
    # *target* names in `BENCH_TARGETS` (e.g. `lint_latency`,
    # `decoder_10kb_rel_to_invariant`, `fix_throughput`). A target-name
    # filter would silently skip every bench whose function name
    # diverges from its target file name — see the bench-id table in
    # `tools/`'s capture documentation.
    #
    # `find` walks the directory; the inner test-and-print emits the
    # parent of any matching `new/estimates.json` whose sibling
    # `new/sample.json` also exists.
    find "$CRITERION_DIR" -type f -name estimates.json -path '*/new/estimates.json' -print0 \
        | while IFS= read -r -d '' estimates_file; do
            local new_dir bench_dir sample_file rel_path
            new_dir="$(dirname "$estimates_file")"
            sample_file="$new_dir/sample.json"
            if [[ ! -f "$sample_file" ]]; then
                continue
            fi
            bench_dir="$(dirname "$new_dir")"
            rel_path="${bench_dir#"$CRITERION_DIR"/}"
            printf '%s\n' "$rel_path"
        done \
        | sort -u
}

# ---------------------------------------------------------------------
# Aggregation
# ---------------------------------------------------------------------

# Build the JSON array of bench entries by walking each discovered
# bench-id, extracting mean (from estimates.json) and percentiles
# (from sample.json), and emitting a JSON object per bench. The
# entries are written line-by-line to a temp file as standalone
# JSON objects; the final aggregator joins them via `jq -s` or
# python3 to produce the final array.
build_bench_entries() {
    local entries_file="$1"
    local count=0
    : > "$entries_file"

    # Per-bench failures (missing files, unparseable output) abort the
    # capture. The baseline JSON is load-bearing: downstream regression
    # gates compare against the entries here, and a silently-incomplete
    # baseline means the omitted bench-IDs would have NO reference data
    # at all in CI — drift on those IDs would never trigger a gate.
    # Fail-loud is the only correct behavior; the bench-runner owner
    # diagnoses the missing/unparsable result and re-captures.
    while IFS= read -r bench_id; do
        if [[ -z "$bench_id" ]]; then
            continue
        fi
        local estimates_path sample_path mean_us pct_line p50 p95 p99 samples rel_estimates
        estimates_path="$CRITERION_DIR/$bench_id/new/estimates.json"
        sample_path="$CRITERION_DIR/$bench_id/new/sample.json"

        if [[ ! -f "$estimates_path" || ! -f "$sample_path" ]]; then
            die "bench-id '$bench_id': missing estimates.json or sample.json under $CRITERION_DIR/$bench_id/new/ — re-run cargo bench"
        fi

        if ! mean_us=$(read_mean_us "$estimates_path"); then
            die "bench-id '$bench_id': failed to read mean from $estimates_path"
        fi

        if ! pct_line=$(compute_percentiles "$sample_path"); then
            die "bench-id '$bench_id': failed to compute percentiles from $sample_path"
        fi

        # pct_line is "p50<TAB>p95<TAB>p99<TAB>samples".
        IFS=$'\t' read -r p50 p95 p99 samples <<<"$pct_line"

        # Path relative to the repo root for traceability — the
        # bench-runner owner can rerun the same percentile compute
        # against this file.
        rel_estimates="target/criterion/$bench_id/new/estimates.json"

        # Append one JSON object per line; the aggregator joins.
        printf '{"bench":"%s","p50":%s,"p95":%s,"p99":%s,"mean":%s,"samples":%s,"criterion_estimates_path":"%s"}\n' \
            "$bench_id" "$p50" "$p95" "$p99" "$mean_us" "$samples" "$rel_estimates" \
            >> "$entries_file"
        count=$((count + 1))
    done < <(discover_bench_ids)

    if [[ $count -eq 0 ]]; then
        die "no bench-id directories found under $CRITERION_DIR; cargo bench produced no usable output"
    fi

    info "aggregated $count bench entries"
}

# Compose the final JSON document. The schema mirrors the scaffold
# already checked in at $OUTPUT_PATH (per research.md R-5).
write_output_json() {
    local entries_file="$1"
    local git_sha captured_at cpu host_kernel rust_version criterion_version

    git_sha="$(cd "$REPO_ROOT" && git rev-parse HEAD)"
    captured_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    cpu="$(detect_cpu_model)"
    host_kernel="$(detect_host_kernel)"
    rust_version="$(detect_rust_version)"
    criterion_version="$(detect_criterion_version)"

    # Use the chosen JSON backend to assemble the document so we
    # produce one that's syntactically valid regardless of the
    # entries' exact numeric formatting. Pulling everything through a
    # JSON encoder (jq -n or python3 json) avoids the classic
    # printf-templating pitfalls where a stray quote in a CPU model
    # string would corrupt the file.
    case "$JSON_BACKEND" in
        jq)
            jq -n \
                --slurpfile entries "$entries_file" \
                --arg git_sha "$git_sha" \
                --arg captured_at "$captured_at" \
                --arg cpu "$cpu" \
                --arg host_kernel "$host_kernel" \
                --arg rust_version "$rust_version" \
                --arg criterion_version "$criterion_version" \
                '{
                    schema: "marque-bench-baseline-1.0",
                    status: "captured",
                    captured_at: $captured_at,
                    git_sha: $git_sha,
                    reference_machine: {
                        cpu: ($cpu | select(length > 0) // null),
                        profile: null,
                        rust_version: ($rust_version | select(length > 0) // null),
                        criterion_version: ($criterion_version | select(length > 0) // null),
                        host_kernel: ($host_kernel | select(length > 0) // null),
                        bench_runner_owner: null
                    },
                    benches: $entries
                }' \
                > "$OUTPUT_PATH"
            ;;
        python3)
            ENTRIES_FILE="$entries_file" \
            GIT_SHA="$git_sha" \
            CAPTURED_AT="$captured_at" \
            CPU="$cpu" \
            HOST_KERNEL="$host_kernel" \
            RUST_VERSION="$rust_version" \
            CRITERION_VERSION="$criterion_version" \
            OUTPUT_PATH="$OUTPUT_PATH" \
            python3 - <<'PY'
import json, os

def opt(name):
    v = os.environ.get(name, "")
    return v if v else None

entries = []
with open(os.environ["ENTRIES_FILE"]) as f:
    for line in f:
        line = line.strip()
        if not line:
            continue
        entries.append(json.loads(line))

document = {
    "schema": "marque-bench-baseline-1.0",
    "status": "captured",
    "captured_at": os.environ["CAPTURED_AT"],
    "git_sha": os.environ["GIT_SHA"],
    "reference_machine": {
        "cpu": opt("CPU"),
        "profile": None,
        "rust_version": opt("RUST_VERSION"),
        "criterion_version": opt("CRITERION_VERSION"),
        "host_kernel": opt("HOST_KERNEL"),
        "bench_runner_owner": None,
    },
    "benches": entries,
}

with open(os.environ["OUTPUT_PATH"], "w") as f:
    json.dump(document, f, indent=2)
    f.write("\n")
PY
            ;;
    esac
}

# Print the per-bench summary line the task statement requires.
print_summary() {
    info "wrote baseline to $OUTPUT_PATH"

    local entry_count
    case "$JSON_BACKEND" in
        jq)
            jq -r '
                .benches[]
                | "  \(.bench): p50=\(.p50)µs  p95=\(.p95)µs  p99=\(.p99)µs  mean=\(.mean)µs  n=\(.samples)"
            ' "$OUTPUT_PATH"
            entry_count="$(jq -r '.benches | length' "$OUTPUT_PATH")"
            ;;
        python3)
            python3 - "$OUTPUT_PATH" <<'PY'
import json, sys
with open(sys.argv[1]) as f:
    doc = json.load(f)
for b in doc["benches"]:
    print(
        "  {bench}: p50={p50}us  p95={p95}us  p99={p99}us  "
        "mean={mean}us  n={samples}".format(**b)
    )
PY
            entry_count="$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1]))["benches"]))' "$OUTPUT_PATH")"
            ;;
        *)
            entry_count='?'
            ;;
    esac
    info "captured ${#BENCH_TARGETS[@]} bench targets, ${entry_count} bench-id entries"
}

# ---------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------

# Per-bench entry temp file. Declared at script scope (rather than as
# a `local` in main) so the EXIT trap below can reference it without
# tripping `set -u` after main returns.
ENTRIES_TMP=""

cleanup() {
    if [[ -n "$ENTRIES_TMP" && -f "$ENTRIES_TMP" ]]; then
        rm -f "$ENTRIES_TMP"
    fi
}

main() {
    detect_json_backend
    info "JSON backend: $JSON_BACKEND"

    if [[ ! -d "$REPO_ROOT/benches/baselines" ]]; then
        die "expected output directory does not exist: $REPO_ROOT/benches/baselines (PR-0 scaffold not in place?)"
    fi

    run_benches

    ENTRIES_TMP="$(mktemp)"

    build_bench_entries "$ENTRIES_TMP"
    write_output_json "$ENTRIES_TMP"

    print_summary
    info "DONE — baseline JSON is ready for review and manual commit"
}

trap cleanup EXIT

main "$@"
