<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# marque

A fast, rule-driven text linter, formatter, and transformer. Ships with CAPCO/ISM classification-marking rules.

`marque` is the command-line front end to the Marque engine — a rule-based text
processor designed for perceptual instantaneity at any scale. The default
ruleset enforces U.S. Government IC classification markings (CAPCO/ISM), but
the underlying engine is general-purpose: rules consume parsed attributes and
emit diagnostics with optional confidence-scored fixes.

## Install

```sh
cargo install marque
```

Or build from source:

```sh
cargo build --release -p marque
```

## Usage

```sh
# Lint files (or stdin)
marque check path/to/file.txt
marque check < input.txt
echo "(S) text" | marque check -

# Lint and apply fixes (atomic in-place writes)
marque fix path/to/file.txt

# Preview without writing
marque fix --dry-run path/to/file.txt

# Read from stdin, write fixed text to stdout
marque fix < input.txt > output.txt
```

### Common flags

| Flag | Purpose |
|---|---|
| `--config <PATH>` | Override the project config search; uses `<PATH>` directly instead of walking up from cwd. |
| `--confidence-threshold <FLOAT>` | Override the auto-apply threshold for fixes (`0.0..=1.0`). |
| `--format human\|json` | Diagnostic output format. Defaults to `human` on a TTY, `json` otherwise. |
| `--no-color` | Disable ANSI color in human format (also honored: `NO_COLOR`, `TERM=dumb`). |
| `-q`, `--quiet` | Suppress non-diagnostic stderr narration. |
| `-v`, `--verbose` | Promote logs to `marque=debug`. |
| `--explain-config` | Dump the merged configuration as JSON and exit. |
| `--dry-run` | (`fix` only) Emit audit records without writing. |
| `--in-place` / `--write-stdout` | (`fix` only) Output routing. |
| `--fixed-timestamp <RFC3339>` | (`fix` only, gated by `MARQUE_ALLOW_FIXED_CLOCK=1`) Inject a deterministic timestamp into audit records. Intended for test snapshots. |

### Exit codes

| Code | Meaning |
|---|---|
| `0` | Clean (or fixes applied with no remaining issues). |
| `1` | Errors found (or fix-severity diagnostics remain). |
| `2` | Warnings only. |
| `64` | Usage error. |
| `65` | Bad config or input data. |
| `69` | Feature not yet available (e.g. `metadata` subcommand). |
| `74` | I/O error or non-UTF-8 input. |

## Configuration

`marque` reads `.marque.toml` from the project (committed) and merges
`.marque.local.toml` (gitignored, classifier identity) over it. Environment
variables and CLI flags take final precedence. See `marque-config` for the
full schema.

```toml
# .marque.toml
[capco]
version = "ISM-v2022-DEC"

[rules]
banner-abbreviation = "fix"
missing-usa-trigraph = "warn"

[corrections]
"SERCET" = "SECRET"
```

## Subcommands

- `check` — lint and report.
- `fix` — apply confidence-gated fixes. Emits an NDJSON audit record on
  stderr for every applied fix.
- `metadata` — document metadata report. (Pending — exit 69 today.)

## For maintainers

See [`CLAUDE.md`](../CLAUDE.md) at the workspace root for architecture, the
processing pipeline, and the architectural invariants that govern the
engine/rules contract.

## License

Apache-2.0.
