# SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
"""
randomize_portions.py — assign random portion marks from portions.toml
to every portion-marked paragraph in the spec corpus.

Per-file pool: each spec file samples N marks (default 4) from the
catalog without replacement, then each portion-marked paragraph in
that file gets a random pick from that 4-mark pool. This keeps the
rollup banner decipherment tractable while still varying the marks
across the corpus.

Leaves alone:
  - YAML frontmatter (everything between leading --- ... ---)
  - `=== page N ===` markers
  - `banner: ...` lines  (user's job)
  - blank lines and continuation lines

Replaces:
  - Any line starting with `(...)` followed by whitespace + content.
    The leading `(...)` is swapped for a random pick from the file's pool.

Determinism: seeded by file stem so re-runs reproduce. Pass --seed to
shift the whole run, or --no-deterministic for fresh randomness.
"""

from __future__ import annotations

import argparse
import random
import re
import sys
import tomllib
from pathlib import Path

TOOL_ROOT = Path(__file__).resolve().parent
REPO_ROOT = TOOL_ROOT.parents[1]
PORTIONS_TOML = TOOL_ROOT / "portions.toml"
DEFAULT_SPECS = REPO_ROOT / "tests" / "corpus" / "documents" / "specs"

# Match `(anything-not-paren) <whitespace>`, capturing the trailing space we keep.
PORTION_LINE_RE = re.compile(r"^(\([^)\n]+\))(\s+)(\S.*)$")


def load_portions(path: Path) -> list[str]:
    data = tomllib.loads(path.read_text())
    portions = data.get("portions") or []
    if not portions:
        raise SystemExit(f"no [portions] found in {path}")
    # de-duplicate while preserving order (the TOML has at least one repeat)
    seen: set[str] = set()
    unique: list[str] = []
    for p in portions:
        if p not in seen:
            seen.add(p)
            unique.append(p)
    return unique


def randomize_spec(
    path: Path,
    rng: random.Random,
    portions: list[str],
    marks_per_file: int,
) -> tuple[int, int, list[str]]:
    """Rewrite spec file in place.

    Returns (replacements, paragraphs_seen, pool_used).
    """
    pool_size = min(marks_per_file, len(portions))
    pool = rng.sample(portions, pool_size)
    text = path.read_text()
    lines = text.splitlines(keepends=False)
    out: list[str] = []
    in_frontmatter = False
    frontmatter_done = False
    replacements = 0
    paragraphs_seen = 0

    for i, line in enumerate(lines):
        stripped = line.strip()

        # Frontmatter: between the first two --- markers
        if not frontmatter_done:
            if i == 0 and stripped == "---":
                in_frontmatter = True
                out.append(line)
                continue
            if in_frontmatter:
                if stripped == "---":
                    in_frontmatter = False
                    frontmatter_done = True
                out.append(line)
                continue

        # Page marker / banner line / blank: untouched
        if (
            stripped.startswith("=== page ")
            or stripped.startswith("banner:")
            or stripped == ""
        ):
            out.append(line)
            continue

        m = PORTION_LINE_RE.match(line)
        if m:
            paragraphs_seen += 1
            new_mark = rng.choice(pool)
            out.append(f"{new_mark}{m.group(2)}{m.group(3)}")
            replacements += 1
        else:
            out.append(line)

    path.write_text("\n".join(out) + ("\n" if text.endswith("\n") else ""))
    return replacements, paragraphs_seen, pool


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--seed", type=int, default=0, help="seed offset for determinism")
    ap.add_argument(
        "--marks-per-file",
        type=int,
        default=4,
        help="number of distinct marks to sample per file (default: 4)",
    )
    ap.add_argument(
        "--no-deterministic",
        action="store_true",
        help="use system randomness instead of per-file seeded RNG",
    )
    ap.add_argument(
        "--only",
        nargs="*",
        help="restrict to these spec stems (e.g. CIAPolicyOnGAOOversight)",
    )
    ap.add_argument(
        "--portions",
        type=Path,
        default=PORTIONS_TOML,
        help="path to portions.toml",
    )
    ap.add_argument(
        "--specs",
        type=Path,
        default=DEFAULT_SPECS,
        help="path to the specs directory (default: tests/corpus/documents/specs)",
    )
    args = ap.parse_args()

    portions = load_portions(args.portions)
    spec_files = sorted(args.specs.glob("*.md"))
    if args.only:
        wanted = set(args.only)
        spec_files = [p for p in spec_files if p.stem in wanted]
        if not spec_files:
            print("no specs matched --only filter", file=sys.stderr)
            return 1

    total_repl = 0
    total_paras = 0
    for path in spec_files:
        if args.no_deterministic:
            rng = random.Random()
        else:
            rng = random.Random(f"{args.seed}:{path.stem}")
        repl, paras, pool = randomize_spec(path, rng, portions, args.marks_per_file)
        total_repl += repl
        total_paras += paras
        print(f"{path.name}: {repl} portions from pool {pool}")

    print(
        f"\n{len(spec_files)} files · {total_repl} portion marks assigned · "
        f"{args.marks_per_file} marks/file pool from {len(portions)}-mark catalog"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
