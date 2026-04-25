<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Contributing to marque

`marque` is a general-purpose rule engine for fast text processing. The MVP
ships a CAPCO/ISM classification-marking rule set, but the engine is
domain-neutral. See `CLAUDE.md` and `.specify/memory/constitution.md` for
project depth.

## Required reading

Before opening a non-trivial PR, read:

- **`.specify/memory/constitution.md`** — binding principles (performance,
  zero-copy, WASM safety, two-layer rules, audit-first compliance,
  pipeline phases, crate discipline, authoritative-source fidelity).
- **`CLAUDE.md`** — current architecture, crate responsibilities, key
  types, build/test commands.
- **The relevant `specs/<feature>/`** — when working on an in-flight
  feature, read its `spec.md`, `plan.md`, and `tasks.md`.
- **The crate-level `README.md`** for any crate you are touching.

## Pre-PR checklist

Run before opening a PR. CI runs the same gates and will block on failure.

- `cargo fmt --check` — formatting is enforced, not advisory.
- `cargo clippy --workspace --benches -- -D warnings` — no clippy warnings.
- `cargo nextest run --workspace` (or `cargo test --workspace`) — full
  workspace test suite.
- `./scripts/check.sh` runs all three. Pass `--bench` to additionally
  run the performance regression gate (`scripts/bench-check.sh`).

For changes that touch hot paths (scanner, parser, decoder, engine
dispatch), run `cargo bench` locally and compare against the SC-001
(p95 ≤ 16 ms on 10 KB strict input) and SC-005 (linear scaling)
baselines.

## Citation discipline (Constitution Principle VIII)

Every citation in code, docs, plans, or diagnostics MUST refer to a real
passage in the authoritative source, accurately reflect what that
passage says, and be re-verifiable by any reviewer with the source in
hand. For ISM/CAPCO that source is `crates/capco/docs/CAPCO-2016.md`
(plus the ODNI XML schemas in `crates/ism/schemas/ISM-v2022-DEC/`).
Citations use page numbers, not line numbers (`§H.8 p145`, not
`§H.8 line 9488`).

When propagating a citation from one file to another (rule comment →
docs file, plan → plan, diagnostic → README), re-verify it at the
point of propagation. Stale citations accrete across moves if the
discipline lapses at any single step. Fabricated, hallucinated, or
unverifiable citations MUST be removed, not left in place pending
follow-up — they are a correctness defect, not a style issue.

This applies equally to citations written by humans and by AI
assistance. Neither is exempt from verification.

## Scheme-adoption PR checklist

A *scheme-adoption PR* lands a new marking scheme — Phase F's CUI
adapter, or a future NATO / FGI / JOINT / partner-national adapter.
The four invariants below come from FR-022 of
`specs/004-constraints-decoder-vocab/spec.md` and Constitution
Principle IV.

A scheme-adoption PR MUST satisfy all four. A reviewer who cannot
check every box on the first read should request changes.

1. **The adoption PR MUST NOT edit `marque-engine`, `marque-scheme`,
   `marque-core`, `marque-rules`, or `marque-ism`.** These are the
   grammar-independent crates. A scheme adapter that needs to touch
   them is signaling an engine gap, not a scheme detail.

2. **Engine gaps MUST land in a separate predecessor PR.** If the new
   scheme reveals that a trait surface is missing, that a category
   shape is unsupported, or that the scheduler cannot express a
   needed dataflow, the gap is closed first in its own PR (against
   the corpus regression harness so existing schemes stay
   byte-identical). The scheme-adoption PR then lands cleanly against
   the post-gap trait surface.

3. **Every new scheme crate follows the `build.rs` →
   generated-predicates pattern established by `marque-ism`.** The
   crate parses its primary source (XSD, JSON registry, structured
   manual) at build time and emits binary valid/invalid predicates
   into `OUT_DIR/`. Hand-written `Rule` implementations consume those
   predicates. The active source version is pinned explicitly in
   `[package.metadata.marque]` using a scheme-specific key (e.g.,
   `ism-schema-version` for the `marque-ism` crate today; future
   crates pick a parallel key for their own primary source). Bumps
   are intentional, never silent. The crate stays WASM-safe (no
   runtime I/O, no format adapters).

4. **Every new vocabulary entry cites a verified passage in its
   scheme's primary source.** A `citation` field with the right
   shape is not enough; the passage referenced MUST exist, MUST say
   what the entry claims it says, and MUST be re-checkable by a
   reviewer holding the source. This is Principle VIII applied at
   vocabulary granularity. Entries whose citation cannot be verified
   are removed, not retained.

The current branch (`004-constraints-decoder-vocab`) is *not* a
scheme-adoption contribution — it lands the engine infrastructure
(trait surfaces, scheduler, audit v2, vocabulary tables) that future
scheme-adoption PRs will consume without touching the engine crates.

## Commit message conventions

Commit subjects follow the pattern used in recent main history:

- `feat(<crate>): <subject>` — new functionality.
- `fix(<crate>): <subject>` — bug fix.
- `chore(<crate>): <subject>` — non-functional change.
- `docs(<area>): <subject>` — documentation only.
- `test(<crate>): <subject>` — test-only change.
- `ci(<area>): <subject>` — CI / tooling change.
- `Phase <N> PR-<M>: <subject> (#<issue>)` — phased feature work
  with a tracking issue.

Bodies are optional but encouraged when the diff is non-obvious.
Prefer many small commits over one large commit; each commit should
leave the workspace in a buildable, test-passing state.

Do not skip hooks (`--no-verify`) and do not bypass signing
(`--no-gpg-sign`). If a hook fails, fix the underlying issue.

## Licensing and contributions

All `marque` source code is distributed under the terms in
[`LICENSE.md`](./LICENSE.md) (Marque License 1.0,
`LicenseRef-MarqueLicense-1.0`). The constitution's Tech Stack
section records the rationale behind retiring the prior
permissive-core / commercial-integrations split.

**Contributions are governed by [`CLA.md`](./CLA.md), not by a
blanket "every contribution lands under ML-1.0" claim.** `CLA.md`
covers two paths:

- **Official U.S. Government contributions** — government employees
  and qualifying federal contractors take a public-domain path and
  cannot agree to the CLA. The required statement and the procedure
  are spelled out in `CLA.md` under "Official Contributions"; Knitli
  redistributes those contributions under the Marque License (and
  any other license at Knitli's choice, since public-domain works
  are freely relicensable) and applies a "Public Domain" label so
  other government projects can find them.
- **Commercial and unofficial contributions** — by submitting, you
  agree to the CLA's terms (Knitli may use, modify, and relicense
  your contribution; you grant a patent license; you represent that
  you have the right to contribute). Read `CLA.md` end-to-end before
  opening a non-trivial PR.

Marque crates may depend on permissively-licensed crates (Apache-2.0,
MIT, BSD-2/3-Clause, ISC, Unicode-3.0, Zlib, CC0-1.0, MIT-0). They
MUST NOT depend on copyleft licenses (GPL, LGPL, AGPL, MPL) or
competing source-available licenses (Elastic License 2.0, Business
Source License, SSPL). The authoritative allow-list is `deny.toml`.
