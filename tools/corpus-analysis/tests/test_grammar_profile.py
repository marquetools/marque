#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Knitli Inc.
#
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

"""Tests for the per-grammar analyzer profile (T081).

The pure-JSON tests read the profile directly (zero deps). The fail-closed
test exercises the CLI's `sys.exit` path end-to-end, so it invokes
``analyze.py`` as a subprocess rather than importing it. It prefers
``uv run`` (which resolves the inline-script deps for a full analysis run)
and otherwise falls back to the current interpreter — safe because
``analyze.py`` imports ``requests`` lazily inside the corpus downloaders,
and the missing-grammar run exits during profile loading before any
downloader runs. No network is required.
"""

import json
import shutil
import subprocess
import sys
from pathlib import Path

TOOL_DIR = Path(__file__).resolve().parent.parent
GRAMMARS_DIR = TOOL_DIR / "grammars"
ANALYZE_PY = TOOL_DIR / "analyze.py"


def test_capco_profile_has_required_keys_and_composes_schema_version():
    """The CAPCO profile declares every key the analyzer reads and the
    composed priors schema version matches the Rust-side accept-list."""
    profile = json.loads((GRAMMARS_DIR / "capco.json").read_text())

    required = {
        "grammar",
        "description",
        "tokens",
        "priors_schema_prefix",
        "priors_schema_generation",
        "default_marking_corpus",
        "default_prose_source",
    }
    missing = required - profile.keys()
    assert not missing, f"capco profile missing required keys: {sorted(missing)}"

    composed = (
        f"{profile['priors_schema_prefix']}-priors-"
        f"{profile['priors_schema_generation']}"
    )
    assert composed == "capco-priors-3", (
        "composed priors schema version must equal the Rust-side "
        f"SUPPORTED_PRIORS_SCHEMA_VERSIONS entry, got {composed!r}"
    )


def test_capco_profile_tokens_path_resolves_to_existing_file():
    """The profile's `tokens` path (resolved against the tool dir, as the
    analyzer does) must point at a real vocabulary file."""
    profile = json.loads((GRAMMARS_DIR / "capco.json").read_text())
    tokens_path = TOOL_DIR / profile["tokens"]
    assert tokens_path.is_file(), f"tokens path does not resolve: {tokens_path}"


def _analyze_runner():
    """Return an argv prefix that runs analyze.py. Prefers `uv run`
    (resolves the inline-script deps for a full analysis run); otherwise
    falls back to the current interpreter. The fallback is safe for the
    missing-grammar test because analyze.py imports `requests` lazily
    inside the corpus downloaders, and that run exits during profile
    loading before any downloader executes."""
    if shutil.which("uv"):
        # `--script` forces PEP-723 inline-metadata mode, matching
        # analyze.py's own `uv run --script` shebang; bare `uv run <file>`
        # can resolve against the project env instead and flake.
        return ["uv", "run", "--script", str(ANALYZE_PY)]
    return [sys.executable, str(ANALYZE_PY)]


def test_missing_grammar_profile_fails_closed():
    """A nonexistent grammar profile must exit nonzero (fail-closed),
    not silently fall back to a default. `--max-docs 0` keeps the run
    cheap and `--mode baseline` avoids requiring both strata; the
    profile load happens before any corpus work, so this exits before
    touching the network."""
    runner = _analyze_runner()
    result = subprocess.run(
        runner
        + ["--grammar", "__nonexistent__", "--mode", "baseline", "--max-docs", "0"],
        capture_output=True,
        text=True,
        cwd=str(TOOL_DIR),
    )
    assert result.returncode != 0, (
        "analyze.py must fail closed on a missing grammar profile; "
        f"got returncode {result.returncode}.\nstdout:\n{result.stdout}\n"
        f"stderr:\n{result.stderr}"
    )
    assert "__nonexistent__" in result.stderr, (
        "fail-closed error should name the missing profile; "
        f"stderr was:\n{result.stderr}"
    )
