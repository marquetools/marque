#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
"""Render `docs/refactor-006/sc-completion.toml` into a human-readable
markdown report at `docs/refactor-006/sc-completion-report.md`.

Renders the success-criteria completion report. The TOML is the source
of truth; this script renders the cold-storage markdown form that
reviewers see without running the script.

Python (stdlib only) over Rust because:

  1. Zero new workspace-member contamination (Constitution III: WASM-
     safe crate closure stays clean; the script is out-of-workspace
     in the same shape `tools/citation-lint/` / `tools/masking-pin-lint/`
     are out-of-workspace, except with no Rust crate at all).
  2. CI already has Python 3.11+ via the standard `ubuntu-latest`
     runner; no new toolchain pin required.
  3. The TOML schema is closed; tomllib (stdlib since Python 3.11)
     parses it with no third-party dependency.

Requires Python 3.11+ for the stdlib `tomllib` module. Older
interpreters fail with a helpful error message rather than an
opaque ImportError.

Usage:

    python3 tools/sc-completion-report/render.py
"""

from __future__ import annotations

import sys

if sys.version_info < (3, 11):
    sys.exit(
        "ERROR: tools/sc-completion-report/render.py requires Python 3.11+ "
        "for the stdlib `tomllib` module. "
        f"Current interpreter is {sys.version_info.major}."
        f"{sys.version_info.minor}.{sys.version_info.micro}. "
        "CI's ubuntu-latest runner provides 3.11+; if you hit this "
        "locally, run via `python3.11 tools/sc-completion-report/render.py` "
        "or upgrade your default `python3`."
    )

import tomllib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
TOML_PATH = REPO_ROOT / "docs/refactor-006/sc-completion.toml"
OUTPUT_PATH = REPO_ROOT / "docs/refactor-006/sc-completion-report.md"

STATUS_SYMBOL = {
    "verified": "verified",
    "verified-recent": "verified (recent merge)",
    "regressed": "regressed (carry-forward)",
    "partial": "partial",
    "manual-verified": "verified by hand",
    "n/a": "n/a",
}

HEADER = """<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Success-criteria completion report

**Generated from `sc-completion.toml`. Do not hand-edit. Re-render via
`python3 tools/sc-completion-report/render.py` after editing the TOML.**

This report documents the success-criteria completion state. Each
row's `status` value
derives from a real artifact (corpus regression, Criterion bench,
AST lint, compile-fail test); the `evidence` column names the artifact
so a future reviewer can re-run the check without re-deriving where
to look.

The discipline:

- `verified` — the CI gate exercising this SC is green at HEAD.
- `verified-recent` — green on the most recent merged commit; the verifier
  did not re-run locally.
- `regressed` — known regression carried forward; the `notes` column
  documents the carry-forward (typically a perf-drift item that does
  not violate the constitutional ceiling).
- `partial` — some sub-criteria green, some deferred; `notes` documents
  which sub-criterion is deferred and why.
- `manual-verified` / `n/a` — what they look like.

`status` values are deliberately *not* sycophantic. A perf bench that
drifts past the +10% drift gate but stays two decimal orders under the
constitutional 2ms ceiling is honestly `regressed`, not `verified`,
even though the load-bearing assertion still holds.

## Summary
"""


def render() -> None:
    if not TOML_PATH.exists():
        sys.exit(f"ERROR: {TOML_PATH} does not exist")

    with TOML_PATH.open("rb") as f:
        data = tomllib.load(f)

    meta = data.get("meta", {})
    sc_entries = data.get("sc", [])
    if not sc_entries:
        sys.exit("ERROR: no [[sc]] entries in TOML")

    counts: dict[str, int] = {}
    for sc in sc_entries:
        status = sc["status"]
        counts[status] = counts.get(status, 0) + 1

    lines: list[str] = [HEADER]
    lines.append(
        f"- **Captured at**: {meta.get('captured_at', '<unset>')} "
        f"({meta.get('captured_at_pr', '<unset>')})"
    )
    lines.append(f"- **Total SCs**: {len(sc_entries)}")
    for status in sorted(counts):
        label = STATUS_SYMBOL.get(status, status)
        lines.append(f"- **{label}**: {counts[status]}")
    lines.append("")

    # Per-SC table.
    lines.append("## Per-SC status\n")
    lines.append("| SC | Name | Status | Check kind | Check ref |")
    lines.append("|----|------|--------|------------|-----------|")
    for sc in sc_entries:
        status_label = STATUS_SYMBOL.get(sc["status"], sc["status"])
        # Escape pipe characters in column values that might contain them.
        name = sc["name"].replace("|", r"\|")
        check_ref = sc["check_ref"].replace("|", r"\|")
        lines.append(
            f"| {sc['id']} | {name} | {status_label} | "
            f"`{sc['check_kind']}` | `{check_ref}` |"
        )
    lines.append("")

    # Per-SC detail blocks.
    lines.append("## Detail\n")
    for sc in sc_entries:
        status_label = STATUS_SYMBOL.get(sc["status"], sc["status"])
        lines.append(f"### {sc['id']} — {sc['name']}\n")
        lines.append(f"- **Status**: {status_label}")
        lines.append(f"- **Check kind**: `{sc['check_kind']}`")
        lines.append(f"- **Check ref**: `{sc['check_ref']}`")
        lines.append(f"- **Evidence**: {sc['evidence']}")
        if sc.get("notes"):
            lines.append(f"- **Notes**: {sc['notes']}")
        lines.append("")

    lines.append("---\n")
    lines.append(
        "*Edit the source TOML at `docs/refactor-006/sc-completion.toml`; "
        "this report is generated.*"
    )
    lines.append("")

    OUTPUT_PATH.write_text("\n".join(lines), encoding="utf-8")
    print(f"Rendered {len(sc_entries)} SC entries to {OUTPUT_PATH}")


if __name__ == "__main__":
    render()
