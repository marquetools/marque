#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Knitli Inc.
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
#
# Capture measured flamegraph attribution for the three load-bearing
# benches:
#
#   - lint_10kb                          (lint_latency bench binary)
#   - decoder_10kb_one_mangled_region    (lint_latency bench binary)
#   - profile_project (all 6 phases)     (profile_project bench binary)
#
# Outputs the three measured top-15 markdown tables on stdout. Raw
# samply .json.gz profiles and presymbolicated sidecars are written
# to a scratch directory and NOT committed (per PM contract D-6, raw
# SVGs/profiles are ephemeral).
#
# Prerequisites:
#
#   cargo install samply       (sampling CPU profiler, no perf binary needed)
#   sudo sh -c 'echo 1 > /proc/sys/kernel/perf_event_paranoid'
#                              (one-time WSL2 / Linux sandbox relaxation)
#
# Usage:
#
#   tools/perf/capture-flamegraphs.sh
#       — captures at the current worktree HEAD, prints tables.
#
#   OUT_DIR=/tmp/scratch DURATION=15 tools/perf/capture-flamegraphs.sh
#       — overrides scratch dir; DURATION applies to lint_10kb and
#         decoder_10kb only (profile_project is hardcoded to 5s/phase
#         so the six phases stay comparable to one another).
#
# IMPORTANT — debug-assertions:
#
#   The benches are built with `CARGO_PROFILE_BENCH_DEBUG_ASSERTIONS=false`
#   to produce release-equivalent inclusive-time attribution. The
#   workspace's `[profile.bench]` defaults to `debug-assertions = true`,
#   which activates three `raw.to_vec()` snapshots used to verify
#   PageRewrite content-ignorance + lattice immutability invariants
#   (marking_scheme_impl.rs:717, engine.rs:4434, canonical.rs:294).
#   Those snapshots inflate `CanonicalAttrs::to_vec` and
#   `drop_in_place::<Vec<CanonicalAttrs>>` inclusive percentages by
#   ~12-18 pp combined on lint_10kb — empirically demonstrated in the
#   companion doc §10 (the contamination case study). Without this
#   override the resulting top-15 misattributes debug-only work to
#   production hot paths. DO NOT REMOVE.
#
# Companion utilities:
#
#   tools/perf/samply-to-folded.py   — samply JSON → inferno-style folded
#   tools/perf/top-n-inclusive.py    — folded → top-N inclusive markdown
#   tools/perf/union.py              — true union of inclusive samples
#                                      across a set of frames (use when
#                                      summing inclusive %s would
#                                      double-count parent→child overlap)
#
# Reproduces the measurements in
# `docs/perf/2026-05-19-diagnosis/lint-flamegraph-top15.md`. See that
# file's §1 methodology section for the full pipeline write-up.

set -euo pipefail

OUT_DIR="${OUT_DIR:-/tmp/flamegraph-583}"
DURATION="${DURATION:-10}"   # seconds per bench
SAMPLE_RATE="${SAMPLE_RATE:-997}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

cd "$REPO_ROOT"
mkdir -p "$OUT_DIR"

paranoid=$(cat /proc/sys/kernel/perf_event_paranoid 2>/dev/null || echo "?")
if [[ "$paranoid" != "?" && "$paranoid" -gt 1 ]]; then
    echo "ERROR: kernel.perf_event_paranoid=$paranoid (need <= 1)" >&2
    echo "Run: sudo sh -c 'echo 1 > /proc/sys/kernel/perf_event_paranoid'" >&2
    exit 1
fi

for bin in samply; do
    if ! command -v "$bin" >/dev/null 2>&1; then
        echo "ERROR: '$bin' not on PATH. Run: cargo install $bin" >&2
        exit 1
    fi
done

echo "==> Building benches: strip=none, debug-assertions=off, overflow-checks=off" >&2
echo "    (release-equivalent attribution; see header comment for rationale)" >&2
CARGO_PROFILE_BENCH_STRIP=none \
CARGO_PROFILE_BENCH_DEBUG_ASSERTIONS=false \
CARGO_PROFILE_BENCH_OVERFLOW_CHECKS=false \
    cargo bench --bench lint_latency --no-run >/dev/null
CARGO_PROFILE_BENCH_STRIP=none \
CARGO_PROFILE_BENCH_DEBUG_ASSERTIONS=false \
CARGO_PROFILE_BENCH_OVERFLOW_CHECKS=false \
    cargo bench --bench profile_project --no-run >/dev/null

# Pick the most-recently-built bench binaries (cargo names them with a
# hash suffix). Filter for executable files specifically — cargo also
# emits `.d` depfiles and (under some configurations) `.rmeta` /
# `.rlib` artifacts that share the bench-binary prefix; `-type f
# -executable` ignores all of those by construction.
LINT_BIN=$(find target/release/deps -maxdepth 1 -type f -executable \
    -name 'lint_latency-*' -printf '%T@\t%p\n' 2>/dev/null \
    | sort -nr | head -1 | cut -f2-)
PROFILE_BIN=$(find target/release/deps -maxdepth 1 -type f -executable \
    -name 'profile_project-*' -printf '%T@\t%p\n' 2>/dev/null \
    | sort -nr | head -1 | cut -f2-)

[[ -x "$LINT_BIN" ]] || { echo "ERROR: lint_latency bench binary not found" >&2; exit 1; }
[[ -x "$PROFILE_BIN" ]] || { echo "ERROR: profile_project bench binary not found" >&2; exit 1; }

echo "==> Bench binaries:" >&2
echo "    lint_latency:    $LINT_BIN" >&2
echo "    profile_project: $PROFILE_BIN" >&2

capture () {
    local name="$1" bin="$2" filter="$3" duration="$4"
    echo "==> Capturing $name (${duration}s @ ${SAMPLE_RATE}Hz)..." >&2
    samply record --save-only --no-open --unstable-presymbolicate \
        -r "$SAMPLE_RATE" \
        -o "$OUT_DIR/${name}.json.gz" \
        -- "$bin" --bench --profile-time "$duration" "^${filter}\$" >/dev/null
}

capture "lint_10kb" "$LINT_BIN" "lint_10kb" "$DURATION"
capture "decoder_10kb" "$LINT_BIN" "decoder_10kb_one_mangled_region" "$DURATION"
# profile_project covers 6 phases; run them all together (single capture).
capture "profile_project" "$PROFILE_BIN" "phase_(a|b|c|d|e|f)" "5"

emit_topn () {
    local name="$1" label="$2" thread="$3" root_arg="$4"
    "$SCRIPT_DIR/samply-to-folded.py" \
        "$OUT_DIR/${name}.json.gz" \
        --syms "$OUT_DIR/${name}.json.syms.json" \
        --thread "$thread" 2>/dev/null \
    | "$SCRIPT_DIR/top-n-inclusive.py" 15 "$label" $root_arg
    echo ""
}

echo ""
echo "## Measured top-15 inclusive frames"
echo ""
emit_topn "lint_10kb" \
    "lint_10kb (rooted at Engine::lint, ${DURATION}s @ ${SAMPLE_RATE}Hz)" \
    "lint_latency" "--root lint_with_options_internal_with_cache"
emit_topn "decoder_10kb" \
    "decoder_10kb_one_mangled_region (rooted at Engine::lint, ${DURATION}s @ ${SAMPLE_RATE}Hz)" \
    "lint_latency" "--root lint_with_options_internal_with_cache"
emit_topn "profile_project" \
    "profile_project (all phases mixed, rooted at phase_attribution, 5s/phase @ ${SAMPLE_RATE}Hz)" \
    "profile_project" "--root phase_attribution"
