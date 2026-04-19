<!-- SPDX-FileCopyrightText: 2026 Knitli Inc. -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 0.2.x   | ✅ Current release |
| < 0.2   | ❌ Not supported   |

## Reporting a Vulnerability

Marque processes classification markings governed by CAPCO/ISM standards.
We take security vulnerabilities seriously and appreciate responsible disclosure.

### How to Report

**Please do NOT open a public GitHub issue for security vulnerabilities.**

Instead, use one of the following channels:

1. **GitHub Private Vulnerability Reporting (preferred)**
   Navigate to the [Security Advisories](https://github.com/marquetools/marque/security/advisories)
   page and click **"Report a vulnerability"**.

2. **Email**
   Send a detailed report to: **security@knitli.com**

### What to Include

- Description of the vulnerability
- Steps to reproduce
- Affected component(s): Rust crate name, WASM module, NPM package, CI/CD, etc.
- Impact assessment (e.g., incorrect classification marking, data leakage, supply chain compromise)
- Any suggested fix or mitigation

### Response Timeline

| Stage                    | Target      |
|--------------------------|-------------|
| Acknowledgment           | 48 hours    |
| Initial triage           | 5 business days |
| Fix development          | Varies by severity |
| Public disclosure (coordinated) | After fix is released |

### Scope

The following are in scope for security reports:

- **Rule engine correctness** — incorrect classification markings, missed violations, wrong fixes
- **Supply chain** — compromised dependencies, build script vulnerabilities, CI/CD injection
- **WASM module** — sandbox escapes, memory safety issues
- **Server (`marque-server`)** — authentication bypass, injection, denial of service
- **NPM package (`@marque/marque-demo`)** — malicious install hooks, dependency confusion

### Out of Scope

- CAPCO/ISM specification interpretation disagreements (open a regular issue instead)
- Performance issues (unless they enable denial of service)
- Issues in dependencies not maintained by this project (report upstream, notify us)

## Security Practices

- All dependencies are audited via `cargo-deny` (RustSec advisory database)
- GitHub Actions workflows use SHA-pinned action references
- Dependabot monitors Cargo, npm, and GitHub Actions dependencies
- CodeQL runs on every PR and weekly for Rust, Python, and Actions
- `Cargo.lock` is committed for reproducible builds
- `deny.toml` blocks non-crates.io registry sources and git dependencies
