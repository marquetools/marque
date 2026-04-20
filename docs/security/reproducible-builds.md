<!-- SPDX-FileCopyrightText: 2026 Knitli Inc. -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Reproducible Builds

This document describes Marque's approach to build reproducibility for Rust and WASM targets.

## Why Reproducible Builds Matter

For a security tool processing classified information markings, build reproducibility provides:

1. **Verification** — anyone can rebuild from source and confirm the output matches the published artifact
2. **Tamper detection** — if a published binary doesn't match a clean rebuild, something was injected
3. **Audit trail** — government customers can independently verify that the tool they're running corresponds to the reviewed source code

## Current State

### Rust (Native Targets)

| Factor | Status | Notes |
|--------|--------|-------|
| `Cargo.lock` committed | ✅ | Pins exact dependency versions |
| `cargo-deny` blocks git deps | ✅ | All deps from crates.io only |
| Toolchain pinned in CI | ✅ | `dtolnay/rust-toolchain` at SHA pin, toolchain `1.89` |
| `profile.release` deterministic | ⚠️ | `lto = "fat"`, `codegen-units = 1` helps, but Rust does not guarantee bitwise reproducibility across environments |
| Build-time code generation | ⚠️ | `build.rs` parses vendored schemas — output depends on schema content (pinned) but `OUT_DIR` paths embed absolute paths |

**Known limitations:**

- Rust does not currently guarantee bitwise-identical output across different machines, even with identical inputs. The main sources of non-determinism are:
  - Absolute paths embedded in debug info and `file!()` / `line!()` macros
  - Timestamps in some metadata formats
  - HashMap iteration order in the compiler (mitigated but not eliminated)
- Setting `CARGO_BUILD_INCREMENTAL=0` and building in a consistent directory path (e.g., `/build/marque`) improves reproducibility.

**Recommended verification procedure:**

```bash
# Clone at the exact release tag
git clone --branch v0.2.0 https://github.com/marquetools/marque
cd marque

# Build with deterministic settings
CARGO_BUILD_INCREMENTAL=0 cargo build --release -p marque

# Compare SHA-256 of the output binary
sha256sum target/release/marque
```

### WASM Target

| Factor | Status | Notes |
|--------|--------|-------|
| `wasm-pack` version pinned | ✅ | SHA-pinned in CI |
| `wasm-opt` version pinned | ✅ | SHA-pinned in CI |
| `profile.release-wasm` set | ✅ | `opt-level = "s"`, inherits release profile |
| SRI hashes published | ✅ | SHA-384 hashes generated per release |
| Bitwise reproducibility | ⚠️ | WASM output is more reproducible than native, but `wasm-opt` passes may vary across versions |

**WASM builds are closer to reproducible** because:
- WASM is a portable target — no machine-specific codegen
- `wasm-opt` is deterministic for a given version + flags
- The `release-wasm` profile uses fixed `opt-level` and `codegen-units = 1`

**Recommended verification procedure:**

```bash
# Clone at the exact release tag
git clone --branch v0.2.0 https://github.com/marquetools/marque
cd marque

# Build WASM
wasm-pack build crates/wasm --target web --profile release-wasm

# Compare against published SRI hash
openssl dgst -sha384 -binary crates/wasm/pkg/marque_wasm_bg.wasm \
  | openssl base64 -A \
  | sed 's/^/sha384-/'
```

Compare the output against the `WASM-SRI-HASHES.txt` artifact published with the release.

### NPM Package

| Factor | Status | Notes |
|--------|--------|-------|
| `package-lock.json` committed | ✅ | Pins exact versions |
| `npm ci` used in CI | ✅ | Installs from lockfile |
| Provenance attestation | ✅ | `--provenance` flag in publish workflow |
| `ignore-scripts=true` in `.npmrc` | ✅ | Prevents install hook attacks |

NPM provenance (SLSA Build L3) provides the strongest guarantee: it cryptographically proves the package was built from a specific commit via the GitHub Actions OIDC provider.

## Improving Reproducibility (Roadmap)

### Short-term

- [ ] Pin `cargo-smart-release` and `git-cliff` to exact versions in the release workflow
- [ ] Add a CI job that rebuilds from a release tag and compares WASM SRI hashes

### Medium-term

- [ ] Explore `cargo-auditable` to embed dependency info directly in binaries
- [ ] Set `RUSTFLAGS='--remap-path-prefix=$(pwd)=/build/marque'` in release builds to normalize paths
- [ ] Investigate Docker-based build containers for fully reproducible native builds

### Long-term

- [ ] Track Rust's `reproducible-builds` working group progress
- [ ] Consider Nix or Bazel for hermetic builds if government customers require bitwise reproducibility

## References

- [Reproducible Builds](https://reproducible-builds.org/)
- [Rust Reproducibility Tracking Issue](https://github.com/rust-lang/rust/issues/34902)
- [SLSA Framework](https://slsa.dev/)
- [npm Provenance](https://docs.npmjs.com/generating-provenance-statements)
- [EO 14028 — Improving the Nation's Cybersecurity](https://www.whitehouse.gov/briefing-room/presidential-actions/2021/05/12/executive-order-on-improving-the-nations-cybersecurity/)
- [W3C Subresource Integrity](https://www.w3.org/TR/SRI/)
