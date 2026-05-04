# masking-pin-lint

AST-based CI lint that enforces **FR-039 masking-pin discipline** for the
marque engine refactor (`specs/006-engine-rule-refactor/`).

## What this lint enforces

Every test pin in the workspace that explicitly opts a test out of the
default decoder dispatch by calling
`Engine::with_recognizer(...StrictRecognizer...)` MUST carry a comment
marker within five lines of the call. Two markers are valid:

- `// MASKING-PIN: tracks #NNN — <reason>` — the pin masks a known
  open issue. The lint queries the GitHub API to verify the issue is
  open (mandatory, not optional, per FR-039 rule 4) and follows
  `closed_as_duplicate_of` chains until terminal close.
- `// INTENTIONAL-STRICT: <reason>` — the pin is by design (e.g.,
  the test asserts strict-path behavior in contrast to the default
  dispatcher). No GitHub-API check is performed.

Source-plan §6 discipline rules (verbatim from
`docs/plans/2026-05-02-engine-refactor-consolidated.md` §6):

1. Every masking pin: `// MASKING-PIN: tracks #NNN — remove when issue closes.` within five lines.
2. Every intentional pin: `// INTENTIONAL-STRICT: <reason>` within five lines.
3. Unmarked pins fail CI.
4. **Mandatory** GitHub-API check: tracked issue is open; follows
   `closed_as_duplicate_of` chains until terminal close;
   cascade-close-via-meta-issue is flagged.
5. **Closure protocol**: when an issue closes, the pin is removed in
   the same PR; the pin-removal PR includes a regression test that
   demonstrates fix necessity (must fail on pre-fix HEAD).
6. A third masking pin requires a team-review approval comment.

## Why this crate is out-of-workspace

Per **Constitution III** (WASM safety), the WASM-safe crate set
(`marque-ism`, `marque-core`, `marque-rules`, `marque-scheme`,
`marque-capco`) MUST compile to WebAssembly without modification and
MUST have zero runtime I/O dependencies. This lint depends on
`octocrab`, which transitively pulls `reqwest`, `tokio`, and a TLS
stack — none of which are WASM-safe. Adding this crate to the
workspace member graph at `/home/user/marque/Cargo.toml` would
contaminate the WASM-safe closure even when WASM builds explicitly
exclude this crate, because cargo resolves the entire workspace's
dependency graph during `cargo metadata`.

The crate's `Cargo.toml` includes an empty `[workspace]` table at the
bottom to prevent cargo's parent-directory walk from re-attaching the
crate to the parent workspace.

## Invocation

CI mode (default — fail on lint violations, prefer fresh API state,
fall back to cache on API failure):

```bash
cargo run --manifest-path tools/masking-pin-lint/Cargo.toml -- \
    --workspace-dir . \
    --mode ci
```

Daily cache-refresh mode (run by a scheduled CI job — repopulates
`tools/masking-pin-lint/cache/`):

```bash
cargo run --manifest-path tools/masking-pin-lint/Cargo.toml -- \
    --workspace-dir . \
    --mode refresh-cache
```

Optional flags:

- `--cache-dir <path>` — override cache directory (default
  `tools/masking-pin-lint/cache`).
- `--github-token <token>` — explicit token; otherwise read from the
  `GITHUB_TOKEN` environment variable. Unauthenticated calls work but
  are heavily rate-limited (60/hr per IP) — CI should always set a
  token.
- `--repo <owner/name>` — override repo (default
  `marquetools/marque`).

## Cache-with-fallback design (per decision D11)

The lint follows the **API-first / cache-fallback** pattern:

1. PR-time CI run attempts a GitHub API call with a 5-second
   per-issue timeout.
2. On success, the response replaces the cached state for that
   issue.
3. On failure (timeout / rate-limit / network error), the lint
   reads the cached state from
   `cache/<owner>__<repo>__<NNN>.json`.
4. Cache age `< 24 h`: silent fallback.
5. Cache age `≥ 24 h`: emit a CI warning (not an error).
6. Cache miss + API unavailable: **error** — the lint cannot
   verify the issue state and cannot let the PR through. Recommend
   the contributor run `--mode refresh-cache` locally with a token.

A scheduled CI job runs `--mode refresh-cache` once per day to keep
the cache warm. The 24-hour staleness window is the failure-mode
detection horizon: a stale "open" cache for a closed issue is caught
at the next fresh API call.

### `closed_as_duplicate_of` chain following

Per FR-039 rule 4, when an issue is closed the lint follows
`closed_as_duplicate_of` cross-references until it hits a terminal
close. The chain is recorded in the cache for audit and to detect
cycles.

If the terminal-close chain visits a "meta" or "tracking" issue
(title prefixed `[meta]` or contains the word `tracking`), the lint
emits a CI **warning** so a reviewer can assess whether the
cascade-close is appropriate. This implements the
"cascade-close-via-meta-issue" flag from FR-039 rule 4.

A cycle in the chain (a duplicate-of pointer that revisits a
previously-seen issue) is an **error** — manual review required.

## Cache JSON schema

```json
{
  "schema": "marque-masking-pin-cache-1.0",
  "repo": "marquetools/marque",
  "issue_number": 257,
  "state": "open",
  "closed_at": null,
  "closed_as_duplicate_of": null,
  "refreshed_at": "2026-05-04T03:14:00Z",
  "chain": [257]
}
```

Fields:

- `schema`: pinned to `marque-masking-pin-cache-1.0`. Future schema
  changes require a coordinated bump.
- `repo`: `<owner>/<name>`, redundant with the filename for
  human-readability.
- `issue_number`: the **starting** issue (the one referenced by the
  pin marker), not the terminal issue if the chain redirects.
- `state`: `"open"` or `"closed"`.
- `closed_at`: ISO-8601 timestamp when the terminal issue closed, or
  `null` if open.
- `closed_as_duplicate_of`: the issue number the terminal issue is a
  duplicate of, or `null` if not closed-as-duplicate. This field
  exists for audit; the chain itself is the authoritative path.
- `refreshed_at`: ISO-8601 timestamp when this cache entry was last
  written from a fresh API call.
- `chain`: ordered list of issue numbers visited following
  `closed_as_duplicate_of`, starting with `issue_number`. Length 1
  for issues with no duplicate redirect.

The full schema documentation lives at `cache/SCHEMA.md`.

## Testing

```bash
cargo test --manifest-path tools/masking-pin-lint/Cargo.toml
```

Integration tests under `tests/` exercise the AST scanner against
synthetic Rust source files in `tests/fixtures/`. The cache and GitHub
modules carry unit tests in `src/cache.rs` and `src/github.rs` that
cover roundtrip serialization, schema-mismatch rejection, the meta-issue
title heuristic, and similar pure logic — no live GitHub API calls
happen in any test invocation. End-to-end exercise of the API path
relies on the CI runner using the GitHub-injected `secrets.GITHUB_TOKEN`
when the workflow at `.github/workflows/ci.yml` invokes the lint binary
in `--mode ci`.

## References

- `specs/006-engine-rule-refactor/spec.md` — FR-039
- `specs/006-engine-rule-refactor/research.md` — R-1, R-10
- `specs/006-engine-rule-refactor/decisions.md` — D11
- `docs/plans/2026-05-02-engine-refactor-consolidated.md` — §6
  (masking-pin discipline rules)
- `.specify/memory/constitution.md` — Principle III (WASM safety;
  rationale for out-of-workspace placement)
