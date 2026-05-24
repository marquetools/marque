<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# `twiggy monos` WASM monomorphization attribution

> **Status: gap.** Captured 2026-05-19 on WSL2 dev host. `twiggy monos`
> on the release-web WASM artifact returned zero rows because the
> `release-web` profile strips the WASM name section during the
> build (via `--strip-toolchain-annotations` in `wasm-opt`, at
> `crates/wasm/Cargo.toml:67`). The `profiling` and `dev` profiles
> do NOT set `--strip-toolchain-annotations`, but they still lose
> symbol attribution for `twiggy monos` because
> `wasm-bindgen`'s `dwarf-debug-info = false` is set explicitly on
> the `profiling` profile (`Cargo.toml:30`) and defaults to false
> on `dev`. `twiggy top` worked on the dev build but returned
> anonymized `code[N]` IDs, not function names.

## Reproducer

```bash
# release-web (production-shape artifact, --strip-toolchain-annotations
# at Cargo.toml:67 strips the name section):
bash tools/wasm-size-check.sh
twiggy monos -n 30 crates/wasm/pkg/marque_wasm_bg.wasm
# returns 0 rows

# profiling (no --strip-toolchain-annotations, but dwarf-debug-info = false
# at Cargo.toml:30 still loses symbol attribution for twiggy monos):
wasm-pack build crates/wasm --target web --profile profiling --no-opt
twiggy monos -n 30 crates/wasm/pkg/marque_wasm_bg.wasm
# returns 0 rows

# dev (3.8 MB output; dwarf-debug-info defaults to false; still anonymized):
wasm-pack build crates/wasm --target web --dev
twiggy top -n 30 crates/wasm/pkg/marque_wasm_bg.wasm
# returns code[0], code[1], ..., code[N] — no function names
```

## Cause

The size-baseline file (`tools/wasm-size-baseline.txt`) and this
diagnosis's WASM measurements both come from
`wasm-pack build crates/wasm --target web --profile release-web`
(invoked via `tools/wasm-size-check.sh`). The `release-web` profile
sets `--strip-toolchain-annotations` in its `wasm-opt` flag list
(`crates/wasm/Cargo.toml:67`), which strips the WASM name section.
The two other `release-*` profiles (`release-cloud` at
`Cargo.toml:98`, `release-server` at `Cargo.toml:128`) also set
that flag. The `profiling` profile (`Cargo.toml:25`) and `dev`
profile (`Cargo.toml:33`) do NOT set `--strip-toolchain-annotations`
— their `wasm-opt` flag is `["--all-features"]` only. The
`release-web` profile additionally runs `--monomorphize` (line 61)
which folds back monomorphizations twiggy would otherwise
enumerate.

Switching the diagnostic build to `--profile profiling` or
`--dev` would preserve the name section's strip status, but those
profiles still don't surface `twiggy monos` rows because
`wasm-bindgen`'s `dwarf-debug-info = false` is set explicitly on
`profiling` (`Cargo.toml:30`) and defaults to false on `dev`. To
make `twiggy monos` work, the audit build needs both
(a) no `--strip-toolchain-annotations` AND
(b) `dwarf-debug-info = true` (or its equivalent symbol-preserving
wasm-bindgen flag). That is what the MO-3 and W-MO-1 remediation
candidates (see `../2026-05-19-diagnosis.md` §5) propose adding
via a new `release-monoaudit` profile, since the audit build is
necessarily distinct from the production `release-web` artifact
that ships. (OTHER-1, the related profiling-infra candidate, is
scoped to flamegraph capture for the lint hot path, not WASM
monomorphization attribution.)

## Remediation surface (INVESTIGATE-tier candidate)

Build a separate WASM artifact specifically for monomorphization
attribution:

```bash
# Proposed (not committed in this PR):
wasm-pack build crates/wasm --target web \
  --profile profiling --keep-debug --no-opt
twiggy monos -n 50 crates/wasm/pkg/marque_wasm_bg.wasm
```

This requires either (a) adding a new `release-monoaudit` profile to
`crates/wasm/Cargo.toml` with `keep-debug = true`, or (b) tweaking
the existing profiling profile via env-var override.

The investigation should be paired with the eventual T144-style
CI annotation gate (gate 3 in the architect preflight, deferred to
follow-up PRs) since the same artifact configuration is what the
CI annotation would consume.

## Substitute attribution via native `cargo bloat`

In the absence of working `twiggy monos`, the per-function ranking
from `cargo bloat --release -p marque -n 50` (in
`./cargo-bloat-top20.md`) provides the substitute attribution. The
native CLI binary's text section is not a perfect proxy for the WASM
binary's monomorphization profile (rustc's WASM codegen makes
different inlining decisions, and the WASM optimizer folds some
monomorphizations that the native linker keeps), but the **structure**
of the regression is shared: the 5 quicksort monomorphizations and
the `join_via_lattice` resolver dominate both.

The native bloat numbers therefore stand in for the diagnosis with
a documented confidence reduction: WASM-specific findings ranked as
MEDIUM confidence in the remediation plan instead of HIGH.

## Top WASM `code[N]` slabs (anonymized)

These slab sizes are taken from `twiggy top -n 10
crates/wasm/pkg/marque_wasm_bg.wasm` on the release-web HEAD artifact.
Without name resolution, the slabs cannot be attributed to functions;
they are recorded here so a future capture with names can compare
slab sizes positionally.

| Rank | Bytes | % | Item |
|---|---|---|---|
| 1 | 123,650 | 9.43% | `data[0]` (static data section — vocabulary tables, sentinel tables, priors) |
| 2 | 49,522 | 3.78% | `code[0]` |
| 3 | 35,236 | 2.69% | `code[1]` |
| 4 | 27,178 | 2.07% | `code[2]` |
| 5 | 16,874 | 1.29% | `code[3]` |
| 6 | 14,529 | 1.11% | `code[4]` |
| 7 | 12,577 | 0.96% | `code[5]` |
| 8 | 11,189 | 0.85% | `code[6]` |
| 9 | 9,716 | 0.74% | `code[8]` |
| 10 | 8,721 | 0.66% | `code[7]` |

Note slab 1 = `data[0]` is the static-data segment (vocabulary
tables, closure rule predicate tables, sentinel const tables — the
build-time generated tables from `marque-ism/build.rs`). Native
`cargo bloat`'s `--crates` view already covers static data as part
of the crate's section sizes.

## WASM byte-size summary at checkpoints

Captured via `bash tools/wasm-size-check.sh` (pre-opt) and manual
`wasm-opt -O3 --enable-bulk-memory --enable-bulk-memory-opt
--enable-extended-const --enable-multivalue
--enable-nontrapping-float-to-int --enable-reference-types
--enable-sign-ext --enable-simd --enable-tail-call` post-opt:

| Checkpoint | Pre-opt (release-web) | Post-opt (-O3 minimal) |
|---|---|---|
| `pre-pr4` (`18cef6c9`) | 1,218,209 B (1.16 MB) | 1,139,451 B (1.09 MB) |
| `head` (`81694384`) | 1,311,681 B (1.25 MB) | 1,229,883 B (1.17 MB) |
| Δ | +93,472 B (+7.67%) | +90,432 B (+7.93%) |

Both bases show ~+90 KB / +7-8% growth across the umbrella. The
user-reported ~1.6 MB at HEAD does not reproduce on either basis on
the WSL2 dev host. Likely measurement-basis drift: GHA-built
artifacts may render larger than WSL2-built ones due to LLVM /
wasm-bindgen version pins, or the user may have captured at a
mid-PR-4b-D era checkpoint (the lattice landing peak), or an
intermediate `wasm-opt` configuration produced a different number.
A re-capture on GHA after this PR ships is the recovery path.
