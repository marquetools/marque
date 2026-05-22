#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Knitli Inc.
#
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
#
# Builds a names-preserving WASM artifact for monomorphization analysis
# and prints twiggy top + twiggy monos output.
#
# ## Why this script bypasses wasm-pack
#
# wasm-pack 0.14 does not plumb its `dwarf-debug-info = true` Cargo.toml
# metadata through to wasm-bindgen's `--keep-debug` flag, so the resulting
# artifact has no DWARF and `twiggy monos` produces 0 rows.  This script
# drives cargo + wasm-bindgen directly to guarantee DWARF is preserved.
#
# ## Cargo profile env-var overrides
#
# The `release-monoaudit` Cargo profile is overridden at runtime with:
#
#   CARGO_PROFILE_RELEASE_MONOAUDIT_DEBUG=2
#     Forces full DWARF (level 2) — overrides the profile's `debug = false`.
#
#   CARGO_PROFILE_RELEASE_MONOAUDIT_STRIP=none
#     Prevents symbol stripping — overrides the profile's `strip = "symbols"`.
#
# These follow the standard Cargo profile override convention
# (CARGO_PROFILE_<UPPERCASE_NAME>_<KEY>=<value>) and are purposely NOT
# committed to Cargo.toml to keep the ship-build profile free of debug bloat.
#
# ## wasm-bindgen version pinning
#
# The wasm-bindgen CLI version MUST exactly match the version of the
# wasm-bindgen crate resolved in Cargo.lock.  A mismatch causes a fatal
# "schema version mismatch" error.  The script extracts the expected version
# from Cargo.lock and verifies any candidate binary before use.
#
# To install the matching CLI:
#   cargo install wasm-bindgen-cli --version <VERSION>
# (the script prints the exact version on failure)
#
# Alternatively, run wasm-pack once to let it download the correct binary
# into its cache (~/.cache/.wasm-pack/wasm-bindgen-*/), then re-run this
# script — it will discover the cached binary automatically.
#
# ## Usage
#
#   tools/wasm-monoaudit.sh           — build and report
#   TWIGGY_TOP_N=50 tools/wasm-monoaudit.sh   — override top-N for `twiggy top`
#   TWIGGY_MONOS_N=100 tools/wasm-monoaudit.sh — override top-N for `twiggy monos`

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CARGO_WASM_TARGET="${REPO_ROOT}/target/wasm32-unknown-unknown/release-monoaudit/marque_wasm.wasm"
TWIGGY_TOP_N="${TWIGGY_TOP_N:-30}"
TWIGGY_MONOS_N="${TWIGGY_MONOS_N:-50}"

# ── prerequisites ─────────────────────────────────────────────────────────────

if ! command -v cargo >/dev/null 2>&1; then
    echo "[wasm-monoaudit] ERROR: cargo not found in PATH." >&2
    exit 127
fi

if ! command -v twiggy >/dev/null 2>&1; then
    echo "[wasm-monoaudit] ERROR: twiggy not found in PATH." >&2
    echo "[wasm-monoaudit]   Install with: cargo install twiggy" >&2
    exit 127
fi

# ── resolve expected wasm-bindgen version from Cargo.lock ─────────────────────

EXPECTED_WB_VERSION=$(
    awk '/^name = "wasm-bindgen"$/ { found=1 }
         found && /^version = / { gsub(/"/, ""); split($0, a, " = "); print a[2]; exit }' \
        "${REPO_ROOT}/Cargo.lock"
)

if [[ -z "${EXPECTED_WB_VERSION}" ]]; then
    echo "[wasm-monoaudit] ERROR: could not extract wasm-bindgen version from Cargo.lock." >&2
    exit 2
fi

echo "[wasm-monoaudit] expected wasm-bindgen version: ${EXPECTED_WB_VERSION}"

# ── locate wasm-bindgen binary with matching version ──────────────────────────

find_wasm_bindgen() {
    local expected_ver="$1"
    local -a candidates=()

    # 1. wasm-bindgen already on PATH
    if command -v wasm-bindgen >/dev/null 2>&1; then
        candidates+=("$(command -v wasm-bindgen)")
    fi

    # 2. wasm-pack download cache.  wasm-pack stores the wasm-bindgen binary under a
    #    hash-named subdirectory; the exact parent path varies by OS and wasm-pack version:
    #      Linux (current):  ~/.cache/.wasm-pack/wasm-bindgen-<hash>/wasm-bindgen
    #      Linux (older):    ~/.cache/wasm-pack/wasm-bindgen-<hash>/wasm-bindgen
    #      macOS (current):  ~/Library/Caches/.wasm-pack/wasm-bindgen-<hash>/wasm-bindgen
    #      macOS (older):    ~/Library/Caches/wasm-pack/wasm-bindgen-<hash>/wasm-bindgen
    #    All four roots are searched defensively so the script works across wasm-pack versions.
    local -a cache_roots=(
        "${HOME}/.cache/.wasm-pack"
        "${HOME}/.cache/wasm-pack"
        "${HOME}/Library/Caches/.wasm-pack"
        "${HOME}/Library/Caches/wasm-pack"
    )
    for cache_root in "${cache_roots[@]}"; do
        if [[ -d "${cache_root}" ]]; then
            while IFS= read -r -d '' wb_bin; do
                candidates+=("${wb_bin}")
            done < <(find "${cache_root}" -maxdepth 2 -name "wasm-bindgen" -type f -print0 2>/dev/null)
        fi
    done

    for candidate in "${candidates[@]}"; do
        local ver
        if ! ver=$("${candidate}" --version 2>/dev/null | awk '{print $2}'); then
            echo "[wasm-monoaudit] (skipping ${candidate}: failed to query version)" >&2
            continue
        fi
        if [[ "${ver}" == "${expected_ver}" ]]; then
            echo "${candidate}"
            return 0
        else
            echo "[wasm-monoaudit] (skipping ${candidate}: version ${ver} != ${expected_ver})" >&2
        fi
    done

    return 1
}

WASM_BINDGEN_BIN=""
if ! WASM_BINDGEN_BIN=$(find_wasm_bindgen "${EXPECTED_WB_VERSION}"); then
    echo "[wasm-monoaudit] ERROR: wasm-bindgen v${EXPECTED_WB_VERSION} not found." >&2
    echo "[wasm-monoaudit]" >&2
    echo "[wasm-monoaudit]   Install the matching CLI:" >&2
    echo "[wasm-monoaudit]     cargo install wasm-bindgen-cli --version ${EXPECTED_WB_VERSION}" >&2
    echo "[wasm-monoaudit]" >&2
    echo "[wasm-monoaudit]   Or let wasm-pack cache it automatically:" >&2
    echo "[wasm-monoaudit]     wasm-pack build crates/wasm --target web --profile release-monoaudit" >&2
    echo "[wasm-monoaudit]   (the binary lands in ~/.cache/.wasm-pack/wasm-bindgen-*/)" >&2
    exit 127
fi

echo "[wasm-monoaudit] using wasm-bindgen: ${WASM_BINDGEN_BIN}"

# ── build WASM with DWARF via cargo env-var overrides ─────────────────────────

echo "[wasm-monoaudit] building crates/wasm (release-monoaudit + DWARF overrides)..."
cd "${REPO_ROOT}"
env \
    CARGO_PROFILE_RELEASE_MONOAUDIT_DEBUG=2 \
    CARGO_PROFILE_RELEASE_MONOAUDIT_STRIP=none \
    cargo build -p marque-wasm --target wasm32-unknown-unknown --profile release-monoaudit

if [[ ! -f "${CARGO_WASM_TARGET}" ]]; then
    echo "[wasm-monoaudit] ERROR: expected artifact not found: ${CARGO_WASM_TARGET}" >&2
    exit 2
fi

echo "[wasm-monoaudit] raw artifact: $(wc -c <"${CARGO_WASM_TARGET}" | tr -d ' ') bytes"

# ── wasm-bindgen --keep-debug ─────────────────────────────────────────────────

TWIGGY_DIR=$(mktemp -d "${TMPDIR:-/tmp}/wasm-monoaudit.XXXXXX")
trap 'rm -rf "${TWIGGY_DIR}"' EXIT

echo "[wasm-monoaudit] running wasm-bindgen --keep-debug..."
"${WASM_BINDGEN_BIN}" \
    --target web \
    --keep-debug \
    --out-dir "${TWIGGY_DIR}" \
    --out-name marque_wasm \
    "${CARGO_WASM_TARGET}"

TWIGGY_WASM="${TWIGGY_DIR}/marque_wasm_bg.wasm"
if [[ ! -f "${TWIGGY_WASM}" ]]; then
    echo "[wasm-monoaudit] ERROR: wasm-bindgen did not produce ${TWIGGY_WASM}" >&2
    exit 2
fi

echo "[wasm-monoaudit] bindgen artifact: $(wc -c <"${TWIGGY_WASM}" | tr -d ' ') bytes"

# ── twiggy ────────────────────────────────────────────────────────────────────

echo ""
echo "[wasm-monoaudit] top ${TWIGGY_TOP_N} symbols from ${TWIGGY_WASM}:"
twiggy top -n "${TWIGGY_TOP_N}" "${TWIGGY_WASM}"

echo ""
echo "[wasm-monoaudit] top ${TWIGGY_MONOS_N} monomorphizations from ${TWIGGY_WASM}:"
twiggy monos -n "${TWIGGY_MONOS_N}" "${TWIGGY_WASM}"
