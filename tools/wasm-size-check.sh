#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Knitli Inc.
#
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
#
# PR 3d (FR-053 / FR-054) WASM-binary-size regression gate.
#
# Builds `crates/wasm` with `wasm-pack build --target web --profile
# release-web`, captures the resulting `pkg/marque_wasm_bg.wasm` byte
# count, compares to the baseline in `tools/wasm-size-baseline.txt`,
# and emits a regression line. Fails (exit 1) if the post-build size
# exceeds the baseline by more than 5% (Constitution III + T058i
# quality gate).
#
# ## What this gate measures vs what CI ships
#
# This gate measures the `release-web` profile's PRE-`wasm-opt`
# artifact size. The main `.github/workflows/ci.yml` `wasm` job
# additionally runs `wasm-pack build --profiling` and post-processes
# with `wasm-opt -O3` flags; the ship artifact is therefore smaller
# than the number this gate enforces. The gate exists to catch
# Rust-side bloat (new vocabulary tables, new `Box<dyn>` paths,
# accidentally-pulled-in deps) at the source — `wasm-opt` cannot
# rescue every regression, and a 5% pre-opt regression is the
# right canary for an upstream-side bloat event.
#
# ## Why we measure the pre-`wasm-opt` artifact
#
# `wasm-pack`'s integrated `wasm-opt` pass fails on the current
# repo because the `wasm-opt` build the dev environment ships does
# not enable the `bulk-memory-opt` / `simd` features the produced
# `.wasm` requires. This is unrelated to PR 3d. The pre-opt
# `marque_wasm_bg.wasm` file IS produced before the failing
# `wasm-opt` step runs, so we measure that. When the dev-env
# `wasm-opt` is upgraded, the post-opt size will be smaller than
# the pre-opt size, but the relative regression check still holds
# (we measure delta, not absolute).
#
# ## Usage
#
# - `tools/wasm-size-check.sh` — build, measure, compare, report.
#   Returns nonzero on regression > 5%.
# - `tools/wasm-size-check.sh --update-baseline` — re-measure and
#   overwrite `tools/wasm-size-baseline.txt` with the new size.
#   Use after intentional binary-size changes.
#
# ## Baseline-measurement environment is CI, not local
#
# `tools/wasm-size-baseline.txt` MUST reflect the byte count produced
# by CI's release-web build (Ubuntu runner + pinned rustc + the
# pinned `jetli/wasm-pack-action` revision the `.github/workflows/ci.yml`
# WASM job uses). Local builds produce a meaningfully different
# artifact size — observed at ~100 KB delta between local and CI
# even when the source tree is identical, driven by rustc inlining
# decisions / LLVM version / wasm-pack version drift. Running this
# script locally is fine for a sanity check, but the committed
# baseline value MUST come from CI to keep the regression-gate
# comparison apples-to-apples.
#
# When updating the baseline: open a draft PR, push the change,
# read the CI WASM-build job's "current size" log line, commit that
# value as the new baseline, and push again. The `--update-baseline`
# flag is the local fallback for emergency unblocks but the resulting
# value is NOT authoritative until CI confirms it.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BASELINE_FILE="${SCRIPT_DIR}/wasm-size-baseline.txt"
WASM_ARTIFACT="${REPO_ROOT}/crates/wasm/pkg/marque_wasm_bg.wasm"

cd "${REPO_ROOT}"

UPDATE_BASELINE=false
if [[ "${1:-}" == "--update-baseline" ]]; then
    UPDATE_BASELINE=true
fi

# `wasm-pack`'s integrated wasm-opt pass currently fails on this
# repo (see file-level comment) — but every OTHER failure mode
# (rustc errors, codegen panics, target-not-installed, etc.) must
# still fail the gate. We delete any pre-existing artifact before
# building so a successful "wasm-opt failed AFTER producing the .wasm"
# run can be distinguished from "build failed BEFORE producing it",
# and we inspect the build log for the specific wasm-opt failure
# string to decide whether to tolerate a nonzero exit.
rm -f "${WASM_ARTIFACT}"

echo "[wasm-size-check] building crates/wasm (release-web profile)..."
BUILD_LOG=$(mktemp)
trap 'rm -f "${BUILD_LOG}"' EXIT
set +e
wasm-pack build crates/wasm --target web --profile release-web >"${BUILD_LOG}" 2>&1
BUILD_STATUS=$?
set -e
tail -5 "${BUILD_LOG}"

if [[ ${BUILD_STATUS} -ne 0 ]]; then
    # Only the integrated wasm-opt failure is tolerated — any other
    # nonzero exit signals a real build break and must fail the gate.
    if ! grep -q "wasm-opt" "${BUILD_LOG}"; then
        echo "[wasm-size-check] ERROR: wasm-pack build failed (exit ${BUILD_STATUS}); failure is not the known wasm-opt issue. See log above." >&2
        exit "${BUILD_STATUS}"
    fi
    echo "[wasm-size-check] (wasm-opt step failed as expected on this repo — proceeding with pre-opt artifact)"
fi

if [[ ! -f "${WASM_ARTIFACT}" ]]; then
    echo "[wasm-size-check] ERROR: ${WASM_ARTIFACT} not produced by wasm-pack." >&2
    exit 2
fi

# POSIX-portable byte count (`stat -c '%s'` is GNU-only; macOS BSD
# stat uses `stat -f '%z'`). `wc -c` works identically on every
# POSIX environment and avoids the Linux/macOS split.
CURRENT_SIZE=$(wc -c <"${WASM_ARTIFACT}" | tr -d ' ')
echo "[wasm-size-check] current size: ${CURRENT_SIZE} bytes"

if "${UPDATE_BASELINE}"; then
    printf '%s\n' "${CURRENT_SIZE}" >"${BASELINE_FILE}"
    echo "[wasm-size-check] baseline updated to ${CURRENT_SIZE} bytes (${BASELINE_FILE})."
    exit 0
fi

if [[ ! -f "${BASELINE_FILE}" ]]; then
    echo "[wasm-size-check] ERROR: baseline file ${BASELINE_FILE} missing." >&2
    echo "[wasm-size-check] run with --update-baseline to seed it." >&2
    exit 2
fi

BASELINE_SIZE=$(<"${BASELINE_FILE}")
echo "[wasm-size-check] baseline size: ${BASELINE_SIZE} bytes (${BASELINE_FILE})"

# `MARQUE_WASM_SKIP_REGRESSION=1` skips the +5%-vs-baseline drift gate
# (parallels the `MARQUE_BENCH_SKIP_REGRESSION=1` override in
# `scripts/bench-check.sh`). The build-failed-for-non-wasm-opt-reason
# branch above and the artifact-not-produced branch above STILL fail
# the gate — only the drift comparison is skipped. Set this env var
# only on branches that have explicit PM authorization to ship a
# WASM-size regression (e.g., the `refactor-006-pr-4b-perf-closeout`
# diagnosis branch, which surfaces the regression as the deliverable
# rather than masking it).
if [[ "${MARQUE_WASM_SKIP_REGRESSION:-0}" == "1" ]]; then
    DELTA=$((CURRENT_SIZE - BASELINE_SIZE))
    echo "[wasm-size-check] delta: ${DELTA} bytes (drift gate skipped via MARQUE_WASM_SKIP_REGRESSION=1)"
    echo "[wasm-size-check] OK (skipped by env var on this branch)"
    exit 0
fi

DELTA=$((CURRENT_SIZE - BASELINE_SIZE))
# 5% regression threshold per T058i quality gate. Bash arithmetic
# is integer-only, so we compute the percentage at 1000x (i.e.,
# PERCENT_X1000 = round-down(percentage * 1000)) and compare against
# 5000 (= 5.0% × 1000).
PERCENT_X1000=$((DELTA * 100000 / BASELINE_SIZE))

# Format the signed percentage. Bash integer division truncates
# toward zero, so for |PERCENT_X1000| < 1000 the integer-part division
# yields 0 and would drop the sign. Track the sign explicitly and
# format the magnitude unsigned.
if [[ ${PERCENT_X1000} -lt 0 ]]; then
    SIGN="-"
    ABS_PERCENT_X1000=$((-PERCENT_X1000))
else
    SIGN=""
    ABS_PERCENT_X1000=${PERCENT_X1000}
fi
PERCENT_DISPLAY="${SIGN}$((ABS_PERCENT_X1000 / 1000)).$(printf '%03d' $((ABS_PERCENT_X1000 % 1000)))"

echo "[wasm-size-check] delta: ${DELTA} bytes (${PERCENT_DISPLAY}%)"

# Regression = positive delta exceeding 5%.
if [[ ${PERCENT_X1000} -gt 5000 ]]; then
    echo "[wasm-size-check] FAIL: wasm binary regressed by more than 5%." >&2
    echo "[wasm-size-check] If the increase is intentional (e.g., a new" >&2
    echo "[wasm-size-check] vocabulary table grew the static data), run" >&2
    echo "[wasm-size-check] 'tools/wasm-size-check.sh --update-baseline'" >&2
    echo "[wasm-size-check] and commit the new baseline alongside the change." >&2
    exit 1
fi

echo "[wasm-size-check] OK"
