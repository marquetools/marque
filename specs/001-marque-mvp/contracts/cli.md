# CLI Contract: `marque` (MVP)

**Branch**: `001-marque-mvp` | **Date**: 2026-04-08

The MVP CLI exposes two subcommands: `check` (lint only, never modifies input)
and `fix` (lint + apply auto-fixes that meet the configured confidence
threshold). A third subcommand `metadata` is reserved for the
format-extraction slice and is **not** implemented in the MVP.

---

## Synopsis

```text
marque check  [OPTIONS] [PATH...]
marque fix    [OPTIONS] [PATH...]
marque --version
marque --help
```

If no `PATH` is supplied, both subcommands read from standard input.
A `PATH` of `-` is the explicit stdin sentinel and may be mixed with file paths.

This satisfies **FR-014a**.

---

## Common options

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--config <PATH>` | path | (auto-discover) | Override the project config search path. |
| `--confidence-threshold <FLOAT>` | f32 in [0.0, 1.0] | `0.95` | Minimum confidence for a fix to be auto-applied (FR-004). |
| `--format <human\|json>` | enum | `human` for TTY, `json` otherwise | Output format. `json` emits NDJSON per **R-4** / `contracts/diagnostic.json`. |
| `--no-color` | flag | (auto for non-TTY) | Suppress ANSI color in `human` format. The CLI also honors the `NO_COLOR` environment variable (any non-empty value) and `TERM=dumb`, each of which has the same effect as passing `--no-color` and are applied *before* TTY detection so piped CI logs are never colorized. `--no-color` set on the command line wins over anything. |
| `-q`, `--quiet` | flag | off | Suppress non-diagnostic stderr narration. Audit records still go to stderr. |
| `-v`, `--verbose` | flag | off | Increase log verbosity (equivalent to `MARQUE_LOG=marque=debug`). |
| `--explain-config` | flag | off | Before processing any input, emit the merged `Configuration` (rule severities, corrections map keys, confidence threshold, schema version, classifier-id presence as a boolean, *not* the value) as JSON on stdout and exit `0`. Diagnoses "why is rule X firing as error instead of warn?" without running a real lint. `--explain-config` is mutually exclusive with input paths and with `fix`; mixing them exits `64 EX_USAGE`. |
| `--fixed-timestamp <RFC3339>` | string | unset | Override the Clock used to stamp `AppliedFix.timestamp` with a fixed value. Intended for reproducible CI audit-log captures and integration tests. Accepted only when `MARQUE_ALLOW_FIXED_CLOCK=1` is set in the environment; otherwise the flag exits `64 EX_USAGE` with a message noting that the fixed-clock seam is off by default to prevent accidental audit-log falsification. |

---

## `check`-only options

`check` has no subcommand-specific options in the MVP beyond the common
`--explain-config` flag above. It performs lint and exits without ever
modifying input.

---

## `fix`-only options

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--dry-run` | flag | off | Compute the fix set and emit audit records, but do not modify any input. Required by **FR-006**. |
| `--in-place` | flag | on for file-path inputs | Rewrite each file in place after applying fixes. Mutually exclusive with `--dry-run`. Has no effect for stdin input (which always writes to stdout). |
| `--write-stdout` | flag | on for stdin input | Write the fixed content to stdout. Mutually exclusive with `--in-place`. |

---

## Input handling

- File path arguments are read as raw bytes and decoded as UTF-8. A non-UTF-8
  input is a fatal error (`74 EX_IOERR`).
- A `-` argument means read from standard input until EOF.
- Mixing `-` and file paths is allowed; inputs are processed left-to-right.
- For `fix` with file paths, modifications are written via temp-file rename to
  preserve atomicity (no partially-written files on crash).

---

## Output streams

The MVP follows the Unix split established in **R-5**:

| Stream | Content (`check`) | Content (`fix`) |
|--------|-------------------|-----------------|
| **stdout** | Diagnostics in the chosen format. | For stdin input: the fixed text. For file inputs with `--write-stdout`: the fixed text. Otherwise: empty. |
| **stderr** | Operator narration (file headers, summary line) and the audit-record stream (NDJSON, one record per line). Suppressible with `-q` for narration only — audit records are never suppressed. |

In `--format json` mode, the diagnostic stream on stdout is NDJSON conforming
to `contracts/diagnostic.json`. The audit stream on stderr is NDJSON conforming
to `contracts/audit-record.json`. Both schemas are versioned `marque-mvp-1`
and every audit record carries a top-level `"schema": "marque-mvp-1"` field
(per FR-005a) so consumers can detect version drift inline.

**Atomic record emission (FR-005a)**: each NDJSON record is serialized to an
in-memory buffer and then written with a single `write_all` ending in
`\n`. A partially-serialized record is never flushed. If serialization of
any record fails, the writer emits a single error frame
`{"schema":"marque-mvp-1","error":"<code>","rule":"<rule-id>"}` on the
audit stream and the process exits nonzero. Downstream NDJSON parsers can
therefore assume every line in the audit stream is a complete JSON object.

---

## Exit codes

| Code | Symbolic | Meaning |
|------|----------|---------|
| `0` | `EX_OK` | No diagnostics (clean lint), or `fix` completed and the post-fix re-lint pass is clean. |
| `1` | (diagnostics) | At least one diagnostic of severity `error` was reported. For `fix`, at least one diagnostic remains after fixes were applied. |
| `2` | (warnings only) | At least one diagnostic of severity `warn`, no errors, no remaining-after-fix errors. |
| `64` | `EX_USAGE` | Invalid CLI arguments, mutually exclusive flags combined, or unparseable flag value. |
| `65` | `EX_DATAERR` | Configuration error: malformed `.marque.toml`, classifier identity in committed config (R-5), schema version mismatch (FR-011), or non-UTF-8 input. |
| `74` | `EX_IOERR` | I/O error reading input or writing output. |

This makes `marque check` usable as a CI gate (`marque check $files || exit 1`)
and lets shell scripts distinguish "warnings only" from "errors present".

**Dry-run exit codes**: `marque fix --dry-run` computes the fix set and emits
audit records without writing, but its exit code is computed against the
*post-fix* text (as if the fixes had been applied), exactly matching a
non-dry-run `fix` invocation. This keeps CI gates of the form
`marque fix --dry-run $files || fail` honest: a zero exit means "applying
these fixes would yield a clean document," and a nonzero exit means
"violations would remain after fixes" — regardless of whether bytes were
actually written. A dry-run against an already-clean document exits `0`.

---

## Configuration discovery

`marque` walks upward from the current working directory looking for
`.marque.toml`, stopping at the **first** of: (a) a directory containing
`.marque.toml`, (b) the filesystem root (`/` on Unix, the drive root on
Windows), (c) a directory that is itself a git repository root (contains
`.git/`) — whichever comes first. If no `.marque.toml` is found, the CLI
runs with the built-in defaults. When a `.marque.toml` is found, the CLI
then looks for `.marque.local.toml` **only in that same directory** — the
local-config search is never independently walked, so a stray
`.marque.local.toml` in a parent directory cannot silently attach to a
child project's config. `--config <PATH>` short-circuits the walk and uses
the specified path as the project config; the local-config search still
applies, only in the directory containing the supplied path.

Environment variables (`MARQUE_CLASSIFIER_ID`, `MARQUE_CONFIDENCE_THRESHOLD`,
…) override anything from disk. CLI flags override env vars. Precedence chain
matches **FR-007**.

---

## Hard-fail at config load

The CLI MUST refuse to start if any of the following hold (per **R-5**):

1. A discovered `.marque.toml` contains a `[user]` section.
2. A `[capco] version` value disagrees with the value compiled into
   `marque-capco` (FR-011).
3. `--confidence-threshold` is outside `[0.0, 1.0]`.

In all three cases, the CLI exits `65 EX_DATAERR` with a single-line error on
stderr identifying the offending file and field.

---

## Examples

```sh
# Lint a file, human-readable output
marque check banner.txt

# Lint a file from a pipeline, NDJSON output
cat banner.txt | marque check - --format json | jq '.rule'

# Dry-run fix, audit records on stderr only
marque fix --dry-run banner.txt 2> audit.ndjson

# Apply fixes in place, capture audit log to file
marque fix banner.txt 2> audit.ndjson

# Tighten the threshold for a one-off run
marque fix --confidence-threshold 0.99 banner.txt
```

---

## Out of scope for the MVP CLI

- Directory globbing / recursion (`marque fix docs/`) — deferred (R-6, US3
  out-of-scope list).
- `marque metadata` subcommand — deferred to the format-extraction slice.
- `--audit-log <PATH>` flag — reserved (R-5).
- Server mode (`marque serve`) — deferred to the server slice.
- Watch mode (`marque watch`) — not requested by any user story.
