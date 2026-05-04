<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# flake-watch — quarantine queue for non-deterministic tests

**FR**: FR-051
**Decision**: D16 in `specs/006-engine-rule-refactor/decisions.md`
**Lands**: PR 0
**Owner**: rotating triage owner (named in PR 0 description)

---

## What this is

`flake-watch` is the bookkeeping mechanism for tests that exhibit
non-deterministic failure under unchanged code (i.e., **flakes**). Per
**FR-051** / **decision D16**, marque has **no documented flake
percentage budget**. Instead:

1. Flaky tests get tagged `#[ignore = "FLAKE-WATCH"]` (or equivalent for
   non-`#[test]` harnesses — see "Tagging" below).
2. The flaky test is appended to `issues.md` in this directory with
   metadata.
3. The queue is **capped at 10 entries**. Cap exceedance blocks PR
   merges via CI gate.

This trades probabilistic flake budget (which requires CI dashboarding
to measure) for deterministic queue management (which requires only a
markdown file and a count).

---

## What is **NOT** a flake

A property test that produces a new shrunk-input failure on a fresh
seed is **not** a flake. The property-test harness is doing its job:
discovering an edge case the assertion missed. The remediation is to
fix the assertion or the production code, not to quarantine the test.

If a property test's failures depend on a seed that varies between CI
runs (i.e., the seed is not pinned), the **failure to pin the seed** is
the flake — quarantine the unpinned-seed harness, not the discovered
property failures.

Other non-flakes:
- Tests that fail on a real regression (the production code changed).
- Tests that fail because a transient external dependency is down
  (network, GitHub API). These should be designed to mock external
  dependencies; a "real" external-dep flake should be fixed in the
  test, not quarantined here.

---

## Cap exceedance gate

The CI gate counts active queue entries — it MUST NOT line-count the
file (the scaffold itself is multiple lines before any flakes are
added). The canonical count command is:

```sh
grep -c '^## flake-' tools/flake-watch/issues.md || true
```

This matches each top-level `## flake-YYYY-MM-DD-shortname` heading
exactly once. The template entry uses a blockquote (`> ##`) so it does
not match. Cap exceedance (count > 10) MUST block PR merge until
triage clears at least one entry.

Cap-exceedance triage options:
1. Fix the underlying flake (preferred).
2. Convert the test to a smoke test (lower-resolution, deterministic).
3. Delete the test (only if the assertion is no longer load-bearing).
4. Request an exception (team-review approval comment on the PR).

Option 4 is the escape hatch and should be rare. Repeated requests
indicate the cap is mis-sized.

---

## Tagging

For Rust `#[test]` harnesses:

```rust
#[test]
#[ignore = "FLAKE-WATCH: timer race; see tools/flake-watch/issues.md#flake-2026-05-04"]
fn flaky_test() { /* ... */ }
```

For property tests:

```rust
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    // FLAKE-WATCH: unpinned seed; see tools/flake-watch/issues.md#flake-2026-05-04
    #[test]
    fn flaky_property(/* ... */) { /* ... */ }
}
```

For Criterion benches that occasionally exceed thresholds:
add a comment block immediately above the bench function naming the
flake-watch entry.

---

## File contents

- `README.md` — this file.
- `issues.md` — the quarantine queue itself. Each entry has the form
  documented in the issues.md template. The queue is cap-checked by CI.

---

## PR 0 acceptance

PR 0 absorbs decision D16 in two stages:

**Decision-locking PR (this scaffold)** — already landed:
- `tools/flake-watch/README.md` (this file).
- `tools/flake-watch/issues.md` (empty queue with entry template).

**PR 0 implementation commits** — to land:
- CI workflow / job that reads `issues.md`, counts headings via
  `grep -c '^## flake-' tools/flake-watch/issues.md`, and **fails the
  workflow if count > 10**. Implementation is shell-only — no Rust
  crate is required for the cap check.

The decision-locking PR establishes the queue's existence, schema, and
cap; the CI workflow giving the cap teeth lands alongside the other
PR 0 lint / static-assertion commits.
