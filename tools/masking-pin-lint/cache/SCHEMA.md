# Cache JSON Schema

Each cache file lives at:

```
tools/masking-pin-lint/cache/<owner>__<repo>__<NNN>.json
```

Where `<owner>__<repo>` is the GitHub repo slug with `/` replaced by `__`, and
`<NNN>` is the issue number from the `// MASKING-PIN: tracks #NNN` marker.

Schema identifier: **`marque-masking-pin-cache-1.1`**.

## Field reference

```json
{
  "schema": "marque-masking-pin-cache-1.1",
  "repo": "marquetools/marque",
  "issue_number": 257,
  "state": "open",
  "terminal_state": "Open",
  "meta_issue_warning": false,
  "closed_at": null,
  "closed_as_duplicate_of": null,
  "refreshed_at": "2026-05-04T03:14:00Z",
  "chain": [257]
}
```

| Field | Type | Description |
|---|---|---|
| `schema` | `string` | Pinned to `marque-masking-pin-cache-1.1`. Future schema changes require a coordinated bump. |
| `repo` | `string` | `<owner>/<name>`. Redundant with the filename for readability. |
| `issue_number` | `u32` | The starting issue (from the pin marker), not the terminal issue if the chain redirects. |
| `state` | `string` | Coarse-grained `"open"` or `"closed"` — kept for quick checks; new readers should prefer `terminal_state`. |
| `terminal_state` | `string` | Structured terminal classification: `"Open"`, `"ClosedAsCompleted"`, `"ClosedNotDuplicate"`, or `"Cycle"`. **Required field**; on cache fallback the lint matches against this so a duplicate-chain cycle and an ordinary closed issue produce different diagnostics. **Schema-1.1 addition.** |
| `meta_issue_warning` | `bool` | True if any issue traversed in the `closed_as_duplicate_of` chain looked like a `[meta]` / "tracking" issue. FR-039 rule 4 surfaces this as a warning. **Schema-1.1 addition.** |
| `closed_at` | `RFC-3339 / null` | Terminal close timestamp; `null` if open. |
| `closed_as_duplicate_of` | `u32 / null` | Issue the terminal entry duplicates, if any. Authoritative path is `chain`. |
| `refreshed_at` | `RFC-3339` | Time of the last fresh API write. Drives the 24h staleness check. |
| `chain` | `[u32]` | Ordered chain of `closed_as_duplicate_of` traversal, starting with `issue_number`. Length 1 for issues with no duplicate redirect. |

## Schema migration notes

**1.0 → 1.1** (round-9): added `terminal_state` + `meta_issue_warning`.
The pre-1.1 schema collapsed every non-open terminal classification
into the string `"closed"`, so the cache-fallback path could not
distinguish a duplicate-chain cycle from an ordinary closed issue
and lost the chain-meta-issue warning state that FR-039 rule 4
surfaces. Schema 1.1 preserves the full `TerminalState` enum value
plus the meta-issue flag so the API-unavailable fallback evaluation
in `evaluate_cached_state` produces byte-identical diagnostics to
the API-available path. The cache reader strict-validates the
schema string; pre-1.1 cache files fail the strict check and the
caller is expected to run `--mode refresh-cache` to re-bootstrap.

## Cache lifecycle (per D11 / R-10)

1. **PR-time**: lint attempts API call (5s timeout). On success, this file is
   atomically rewritten.
2. **API failure**: lint reads this file. If `Utc::now() - refreshed_at < 24h`,
   silent fallback. Otherwise, CI emits a warning (still passes).
3. **Cache miss + API failure**: hard error — the lint cannot verify the issue
   state and refuses to let the PR through.
4. **Daily refresh**: a scheduled CI job runs `--mode refresh-cache`, which
   re-queries every tracked issue and rewrites the cache files.

## Schema-bump procedure

1. Pick a new schema string (e.g. `marque-masking-pin-cache-2.0`).
2. Update the `CACHE_SCHEMA` constant in `src/cache.rs`.
3. Update the example in `README.md` and this file in lockstep.
4. Run `--mode refresh-cache` in the same PR to repopulate the cache directory
   (old cache files will be rejected by the new schema check on read).
