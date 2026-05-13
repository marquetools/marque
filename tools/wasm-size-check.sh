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
# repo (see file-level comment) — capture the failure code without
# aborting so we can still read the pre-opt artifact.
echo "[wasm-size-check] building crates/wasm (release-web profile)..."
wasm-pack build crates/wasm --target web --profile release-web 2>&1 | tail -5 || true

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

DELTA=$((CURRENT_SIZE - BASELINE_SIZE))
# 5% regression threshold per T058i quality gate. Bash arithmetic
# is integer-only so we compute the percentage at 1000x and compare
# against 50 (= 5.0%).
PERCENT_X1000=$((DELTA * 100000 / BASELINE_SIZE))
PERCENT_DISPLAY="$((PERCENT_X1000 / 1000)).$(printf '%03d' $((PERCENT_X1000 % 1000 < 0 ? -PERCENT_X1000 % 1000 : PERCENT_X1000 % 1000)))"

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
