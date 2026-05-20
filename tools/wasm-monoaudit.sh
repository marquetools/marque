#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Knitli Inc.
#
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
#
# Builds a names-preserving WASM artifact for monomorphization analysis
# and prints the top 50 `twiggy monos` rows.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
WASM_ARTIFACT="${REPO_ROOT}/crates/wasm/pkg/marque_wasm_bg.wasm"

if ! command -v wasm-pack >/dev/null 2>&1; then
    echo "[wasm-monoaudit] ERROR: wasm-pack not found in PATH." >&2
    exit 127
fi

if ! command -v twiggy >/dev/null 2>&1; then
    echo "[wasm-monoaudit] ERROR: twiggy not found in PATH." >&2
    exit 127
fi

cd "${REPO_ROOT}"

rm -f "${WASM_ARTIFACT}"

echo "[wasm-monoaudit] building crates/wasm (release-monoaudit profile)..."
BUILD_LOG=$(mktemp "${TMPDIR:-/tmp}/wasm-monoaudit.XXXXXX")
trap 'rm -f "${BUILD_LOG}"' EXIT
set +e
wasm-pack build crates/wasm --target web --profile release-monoaudit >"${BUILD_LOG}" 2>&1
BUILD_STATUS=$?
set -e
tail -5 "${BUILD_LOG}"

if [[ ${BUILD_STATUS} -ne 0 ]]; then
    if ! grep -q "wasm-opt" "${BUILD_LOG}"; then
        echo "[wasm-monoaudit] ERROR: wasm-pack build failed (exit ${BUILD_STATUS})." >&2
        exit "${BUILD_STATUS}"
    fi
    echo "[wasm-monoaudit] (wasm-opt step failed; proceeding with pre-opt artifact)"
fi

if [[ ! -f "${WASM_ARTIFACT}" ]]; then
    echo "[wasm-monoaudit] ERROR: ${WASM_ARTIFACT} not produced by wasm-pack." >&2
    exit 2
fi

echo "[wasm-monoaudit] top 50 monomorphizations from ${WASM_ARTIFACT}:"
twiggy monos "${WASM_ARTIFACT}" | head -50
