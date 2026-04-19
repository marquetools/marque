<!-- SPDX-FileCopyrightText: 2026 Knitli Inc. -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Security Policy

> [!IMPORTANT]
> Please **do not report security vulnerabilities in our public issues or discussions**. This may allow attackers to exploit the vulnerability before we have a chance to fix it.

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 0.2.x   | ✅ Current release |
| < 0.2   | ❌ Not supported   |

## Marque's Security Design

We intentionally designed Marque for zero trust environments. It's 'content ignorant' by design, looking only for tokens it cares about based on its rule system, and never moving content to memory.

But we're human. That architecture might have flaws, and we know that there are many attack vectors to protect from.

## Reporting a Vulnerability

Our customers expect uncompromising security, and we're fully committed to it. We take security vulnerabilities seriously and appreciate responsible disclosure.

### How to Report

**Please do NOT open a public GitHub issue for security vulnerabilities.**

Instead, use one of the following channels:

1. **GitHub Private Vulnerability Reporting (preferred)**
   Go to [Security Advisories](https://github.com/marquetools/marque/security/advisories/new) and report your identified vulnerability there. With this method, you may optionally submit a private pull request with a fix. Learn more [in the GitHub docs](https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/privately-reporting-a-security-vulnerability).

2. **Encrypted Email**
   Send a detailed report to: **[adam@knitli.com](mailto:adam@knitli.com)**. **Please encrypt your email.** You can find our public PGP key [here](https://knitli.com/.well-known/pgp-key.txt)

### What to Include

Please include as much of the following information as possible in your report:

1. **Type of issue** (e.g.cross-site scripting, authentication bypass, etc.)
2. **Full paths of source file(s) at the root of the issue**
3. **The location of the affected source code** (tag/branch/commit or direct URL)
4. **Any special configuration required to reproduce the issue**
5. **Step-by-step instructions to reproduce the issue**
6. **Proof-of-concept or exploit code** (if possible)
7. **Impact of the issue**, including how an attacker might exploit the issue
8. **Suggested fix or mitigation** (if you have one)
9. **Your contact information** (email, Twitter, etc.) so we can follow up with you if we need more information or when the issue is resolved

### Response Timeline

Response times will vary by severity and complexity. The following are the longest we would expect to take:

| Stage                    | Target      |
|--------------------------|-------------|
| Acknowledgment           | 24 hours    |
| Initial triage           | 5 business days[^1] |
| Fix development          | Varies by severity |
| Public disclosure (coordinated) | After fix is released |

[^1]: We like to pretend we have weekends. We don't.

### Scope

The following are in scope for security reports:

- **Rule engine correctness** — incorrect markings, missed violations, wrong fixes
- **Supply chain** — compromised dependencies, build script vulnerabilities, CI/CD injection, upstream vulnerabilities
- **Indicators** — known or planned targeting, build provenance misalignment (mismatched hashes), attack infrastructure (i.e. domain acquisition, package squatting)
- **WASM modules** — sandbox escapes, memory safety issues
- **Server (`marque-server`)** — authentication bypass, injection, denial of service
- **NPM package (`@marque/marque-demo`)** — malicious install hooks, dependency confusion

### Out of Scope

- Marking grammar/specification disagreements or differences in interpretation
- Performance issues (unless they enable denial of service)
- Issues in dependencies not maintained by this project (report upstream, notify us)

## Our Response Process

**We take all vulnerability reports seriously and will respond to you promptly**. We may ask follow-up questions, and we will keep you updated on our progress as we investigate and fix the issue. We will work to resolve the issue as quickly as possible, but please understand that some issues may take longer to fix than others depending on their complexity and severity.

## Appreciation

**We appreciate your help in keeping our project secure.** If you report a vulnerability to us, we will credit you in our security advisories and release notes when we fix the issue (unless you request to remain anonymous). We will also include you in our security hall of fame on our website (doesn't exist yet because we've never had one -- you can be first!). Thank you for helping us make our project safer for everyone!

(And no, we don't offer bounties. I'm a solo dev bootstrapping a startup; money is scarce. You'll have my deepest appreciation. When I have customers, I'll re-evaluate.)

## Security Practices

- `cargo-deny` (RustSec advisory database) audits all dependencies
- GitHub Actions workflows use SHA-pinned action references
- Dependabot monitors Cargo, npm, and GitHub Actions dependencies
- CodeQL runs on every PR and weekly for Rust, Python, and Actions
- `Cargo.lock` is committed for reproducible builds
- SBOM generation and publication in multiple formats is planned for future releases
- `deny.toml` blocks non-crates.io registry sources and git dependencies
- We have multiple other routine security scans and efforts to proactively identify vulnerabilities or weaknesses.
- We actively seek to keep dependencies fresh and updated, and continually re-evaluate for better-maintained or more secure alternatives.