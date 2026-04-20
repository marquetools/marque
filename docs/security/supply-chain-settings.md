<!-- SPDX-FileCopyrightText: 2026 Knitli Inc. -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Supply Chain Security Settings Guide

Comprehensive rollup of all supply chain security measures implemented across Phases 1–3 of the Marque security audit. This document serves as both a checklist for verifying the current state and a guide for maintainers.

> **Last updated:** 2026-04-20 (Phase 3 complete)

---

## Phase 1 — Critical (Implemented)

### A1: Dependabot Configuration
- **File:** `.github/dependabot.yml`
- **What:** Automated dependency update PRs for Cargo, npm, and GitHub Actions
- **Settings:**
  - Cargo: weekly on Monday, 10 PR limit, `chore(deps):` prefix
  - npm (demo/): weekly on Monday, 5 PR limit, `chore(deps):` prefix
  - GitHub Actions: weekly on Monday, 10 PR limit, `chore(ci):` prefix
- **Verify:** Check that Dependabot PRs appear weekly in the repository

### A2 / F4: CODEOWNERS
- **File:** `.github/CODEOWNERS`
- **What:** Enforced mandatory review for security-critical paths
- **Protected paths:**
  - `.github/workflows/` — CI/CD pipelines
  - `.github/dependabot.yml` — dependency configuration
  - `Cargo.toml`, `Cargo.lock`, `deny.toml` — Rust dependency policy
  - `demo/package.json`, `demo/package-lock.json` — npm dependencies
  - `crates/ism/build.rs`, `crates/ism/schemas/` — build-time code generation
  - `SECURITY.md` — security policy
- **Verify:** Confirm CODEOWNERS is enforced in branch protection settings

### F1: Security Policy
- **File:** `SECURITY.md`
- **What:** Vulnerability disclosure policy with contact info and response SLAs
- **Key details:**
  - GitHub Private Vulnerability Reporting (preferred)
  - Encrypted email as backup channel
  - 24-hour acknowledgment SLA
  - 5 business day triage SLA
  - Clear scope definition (rule engine, supply chain, WASM, server, npm)

### C1: NPM Security Configuration
- **File:** `demo/.npmrc`
- **What:** Hardened npm settings
- **Settings:**
  - `ignore-scripts=true` — blocks transitive install hooks
  - `audit=true` — auto-runs npm audit
  - `package-lock=true` — enforces lockfile

### B1: Release Workflow SHA Fixes
- **File:** `.github/workflows/release.yml`
- **What:** Fixed `v`-prefixed SHA typos on action references
- **Verify:** All `uses:` entries in release.yml are bare commit SHAs (no `v` prefix)

### A3 / G3: Release Publish Error Handling
- **File:** `.github/workflows/release.yml`
- **What:** Removed `continue-on-error: true`; added per-crate sequential publishing with retry logic
- **Behavior:** Publishes crates one-by-one in dependency order; fails immediately on error with a summary of published vs. remaining crates

### G2: Publish Guard for Test Utils
- **File:** `crates/test-utils/Cargo.toml`
- **What:** `publish = false` to prevent accidental publication
- **Verify:** `grep 'publish' crates/test-utils/Cargo.toml`

### B3: Stray File Removal
- **What:** Removed `execution.rs` from repository root (dead code from CodeQL PR)

### E2: Info Disclosure Fix
- **File:** `demo/bin/serve.js`
- **What:** Removed absolute filesystem path from 404 error response
- **Verify:** 404 responses now return `404 Not Found` without path info

### B5: Unsafe Code Controls
- **What:** Added `#[forbid(unsafe_code)]` to crates that don't need unsafe; `#[deny(unsafe_code)]` with explicit allowlists for those that do
- **Crates with `#[forbid(unsafe_code)]`:** Most crates
- **Crates with `#[deny(unsafe_code)]`:** `marque-ism` (for `Trigraph::as_str()`)

---

## Phase 2 — Important (Implemented)

### C2: NPM Publish Workflow with Provenance
- **File:** `.github/workflows/npm-publish.yml`
- **What:** Automated npm publishing with SLSA Build L3 provenance attestation
- **Key features:**
  - `npm publish --provenance --access public`
  - OIDC token exchange via `id-token: write` permission
  - Protected `npm` environment
  - Dry-run support
- **Prerequisites:**
  - `NPM_TOKEN` secret configured with publish access to `@marque` scope
  - npm account has 2FA enabled
  - GitHub Actions OIDC provider trusted by npm

### D1 / D2: Schema Integrity Checksums
- **Files:** `crates/ism/schemas/ISM-v2022-DEC/SHA256SUMS`, CI job in `ci.yml`
- **What:** SHA-256 checksums for vendored ODNI ISM schema files, verified in CI
- **CI job:** `schema-integrity` — runs `sha256sum --check --strict SHA256SUMS`
- **Verify:** `cd crates/ism/schemas/ISM-v2022-DEC && sha256sum --check SHA256SUMS`

### B4: Cargo-deny Bans Hardened
- **File:** `deny.toml`
- **What:** `multiple-versions = "deny"` (was `"warn"`)
- **Behavior:** CI fails if duplicate crate versions are pulled in transitively
- **Override:** Add entries to `skip = []` with justification for legitimate duplicates

### B2: Cargo-vet Integration
- **Files:** `supply-chain/config.toml`, `supply-chain/audits.toml`, `supply-chain/imports.lock`
- **CI job:** `vet` in `ci.yml`
- **What:** `cargo vet` trust auditing for third-party crates
- **Verify:** `cargo vet` passes locally

### A4: Environment Protection Documentation
- **File:** `.github/workflows/release.yml` (comments)
- **What:** Documented required GitHub Settings for the `cargo` environment:
  - Required reviewers (at least one maintainer)
  - Deployment branches restricted to `main`
  - Wait timer recommendation (5 min cooldown)

### A5: Check ISM Schema Concurrency Guard
- **File:** `.github/workflows/check-ism-schema.yml`
- **What:** Added `concurrency: group: check-ism-schema` with `cancel-in-progress: false`
- **Verify:** Concurrent runs are queued, not duplicated

### C3: Demo Package Publishing Decision
- **File:** `.github/workflows/npm-publish.yml` (comments)
- **What:** Documented that `@marque/marque-demo` is intentionally NOT `"private": true` because it's published via the npm workflow. Manual `npm publish` is not sanctioned.

---

## Phase 3 — Hardening (Implemented)

### E1: WASM SRI Hashes
- **Files:** `.github/workflows/wasm-sri.yml`, release workflow addition
- **What:** Generates SHA-256, SHA-384, and SHA-512 Subresource Integrity hashes for WASM artifacts
- **Artifacts:** `WASM-SRI-HASHES.txt` (human-readable), `wasm-sri.json` (machine-readable)
- **Triggers:** On release tag push (`v*`) and manual dispatch
- **Usage:** Consumers add `integrity="sha384-<hash>"` attribute when loading WASM from CDN
- **Distribution:** Uploaded as workflow artifacts (365-day retention) AND attached to GitHub Releases for permanent availability
- **Verify:** Download the SRI hashes from the GitHub Release page or from workflow artifacts

### A6: Claude Code Plugin Marketplace Trust Decision
- **File:** `.github/workflows/claude-code-review.yml` (documented inline)
- **Decision:** ACCEPTED — the plugin marketplace URL (`https://github.com/anthropics/claude-code.git`) is resolved at runtime (not SHA-pinned), but:
  - The action itself is SHA-pinned
  - The workflow runs with read-only permissions
  - Anthropic is the vendor of Claude Code
- **Risk level:** MEDIUM-LOW
- **Review trigger:** Re-evaluate if the workflow gains write permissions, the marketplace URL changes, or Anthropic deprecates the mechanism
- **Last reviewed:** 2026-04-20

### Supply Chain Dependency & Publisher Review
- **File:** `.github/workflows/supply-chain-review.yml`
- **What:** Periodic (1st and 15th of month) review of dependency versions, sources, and crates.io publisher/owner metadata via `cargo supply-chain`
- **Features:**
  - Generates dependency tree snapshot from `cargo metadata`
  - Verifies all dependencies are from crates.io
  - Queries crates.io publisher/owner info via `cargo supply-chain publishers` and `cargo supply-chain update`
  - Compares against previous snapshot to detect additions/removals
  - Uploads full report (deps, publishers, update history) as artifact
- **Verify:** Check workflow run history for the "Supply Chain Review" workflow

### SBOM Generation
- **File:** `.github/workflows/release.yml` (added steps)
- **What:** CycloneDX and SPDX SBOM generation during release
- **Tool:** `cargo-cyclonedx` pinned to version `0.5.9`
- **Formats:**
  - CycloneDX JSON (`marque-sbom.cdx.json`)
  - CycloneDX XML (`marque-sbom.cdx.xml`)
  - SPDX JSON stub (`marque-sbom.spdx.json`)
- **Compliance:** Addresses Executive Order 14028 (U.S. Government SBOM requirement)
- **Distribution:** Uploaded as workflow artifacts (365-day retention) AND attached to GitHub Releases for permanent availability
- **Verify:** Download the SBOM from the GitHub Release page or from workflow artifacts

### Reproducible Builds Documentation
- **File:** `docs/security/reproducible-builds.md`
- **What:** Documents Marque's approach to build reproducibility
- **Covers:**
  - Current state for Rust native, WASM, and npm targets
  - Known limitations (Rust bitwise reproducibility)
  - Verification procedures for each target
  - Roadmap items (cargo-auditable, path remapping, hermetic builds)

---

## Outside Measures (GitHub Settings — Not Code)

These settings must be configured in the GitHub UI or via the REST API. They cannot be enforced via workflow files.

### Required Status Checks
- [ ] **Setting:** Repository → Settings → Branches → main → "Require status checks to pass before merging"
- [ ] **Checks to require:** `check` (Format + Lint), `test` (Test), `deny` (Supply chain audit), `vet` (Crate trust audit), `schema-integrity` (Schema integrity check)
- [ ] **Why:** Without this, cargo-deny advisories and cargo-vet failures don't block merges

### PR Review Requirements
- [ ] **Setting:** Repository → Settings → Branches → main → "Require a pull request before merging"
- [ ] **Configuration:** Require at least 1 approval, dismiss stale reviews on new pushes
- [ ] **Why:** Prevents unreviewed changes to any branch-protected path

### Commit Signing
- [ ] **Setting:** Repository → Settings → Branches → main → "Require signed commits"
- [ ] **Why:** Ensures commit provenance; critical for a security tool targeting government customers
- [ ] **Note:** The `github-actions[bot]` commits in the release workflow are automatically signed

### Actions Permissions
- [ ] **Setting:** Repository → Settings → Actions → General
- [ ] **Configuration:** "Require approval for all outside collaborators"
- [ ] **Why:** Prevents untrusted forks from running workflows that consume secrets

### Branch Push Restrictions
- [ ] **Setting:** Repository → Settings → Branches → main → "Restrict who can push to matching branches"
- [ ] **Configuration:** Limit to maintainers only
- [ ] **Why:** Prevents direct pushes bypassing PR review

### Dependabot Alerts
- [ ] **Setting:** Repository → Settings → Code security → Dependabot
- [ ] **Configuration:** Enable Dependabot alerts, Dependabot security updates
- [ ] **Why:** Complements Dependabot version updates with vulnerability alerts

### Crates.io Trusted Publishing
- [ ] **Setting:** crates.io account → Settings → Trusted Publishers
- [ ] **Configuration:** Add GitHub Actions OIDC provider for the `cargo` environment
- [ ] **Why:** Ensures only CI can publish crates, not individual developer tokens

### NPM Access Controls
- [ ] **Setting:** npmjs.com → @marque scope → Settings
- [ ] **Configuration:** 2FA required for publishing, granular access tokens
- [ ] **Why:** Prevents unauthorized publishes to the `@marque` scope

### Secret Scanning
- [ ] **Setting:** Repository → Settings → Code security → Secret scanning
- [ ] **Configuration:** Enable secret scanning and push protection
- [ ] **Why:** Prevents accidental commit of API keys, tokens, or credentials

### Private Vulnerability Reporting
- [ ] **Setting:** Repository → Settings → Code security → Private vulnerability reporting
- [ ] **Configuration:** Enable
- [ ] **Why:** Allows security researchers to report issues privately (referenced in SECURITY.md)

---

## Verification Commands

Quick commands to verify the supply chain security posture locally:

```bash
# Verify all cargo-deny checks pass
cargo deny check

# Verify cargo-vet audits pass
cargo vet

# Verify schema integrity
cd crates/ism/schemas/ISM-v2022-DEC && sha256sum --check SHA256SUMS && cd -

# Verify no git dependencies in lockfile
grep -c 'source = "git+' Cargo.lock  # Should output 0

# Verify npm lockfile integrity
cd demo && npm ci && cd -

# Check for known vulnerabilities
cargo deny check advisories

# List all third-party dependencies
cargo metadata --format-version 1 \
  | jq -r '.packages[] | select(.source != null) | "\(.name) \(.version)"' \
  | sort

# Verify WASM SRI hash (after building)
wasm-pack build crates/wasm --target web --profile release-wasm
openssl dgst -sha384 -binary crates/wasm/pkg/marque_wasm_bg.wasm \
  | openssl base64 -A | sed 's/^/sha384-/'
```

---

## Audit Trail

| Phase | Date | PR | Summary |
|-------|------|-----|---------|
| 1 | 2026-04 | — | Dependabot, CODEOWNERS, SECURITY.md, .npmrc, release fixes, unsafe controls |
| 2 | 2026-04 | — | NPM publish + provenance, schema checksums, deny hardening, cargo-vet, environment docs |
| 3 | 2026-04 | — | WASM SRI hashes, SBOM generation, supply chain review, reproducible builds docs, trust decisions |
