#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Knitli Inc.
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
"""
Read inferno-collapse-perf folded stacks on stdin, print top-N
inclusive-time frames as a markdown table on stdout.

Usage:
  cat folded.txt | top-n-inclusive.py N [LABEL] [--root SUBSTR]

--root SUBSTR
    Restrict attribution to stacks containing a frame whose name
    contains SUBSTR. For matching stacks, frames *above* the first
    matching frame are dropped (only leaf-side frames are credited).
    Denominator is the sum of counts of matching stacks. Use this
    to strip criterion harness / libc start / process bootstrap noise.

Inclusive time of frame F = sum of sample counts for every (truncated)
stack containing F. A frame appearing multiple times in one stack
(recursion) is credited once per stack.
"""

from __future__ import annotations

import re
import sys
from collections import defaultdict


def _parse_args(argv: list[str]) -> tuple[int, str, str | None]:
    usage = "usage: top-n-inclusive.py N [LABEL] [--root SUBSTR]"
    if len(argv) < 2:
        print(usage, file=sys.stderr)
        raise SystemExit(2)
    try:
        top_n = int(argv[1])
    except ValueError:
        print(
            f"{usage}\n"
            f"       (N must be a positive integer; got {argv[1]!r})",
            file=sys.stderr,
        )
        raise SystemExit(2)
    if top_n <= 0:
        print(
            f"{usage}\n"
            f"       (N must be a positive integer; got {top_n})",
            file=sys.stderr,
        )
        raise SystemExit(2)
    label = ""
    root: str | None = None
    i = 2
    while i < len(argv):
        if argv[i] == "--root" and i + 1 < len(argv):
            root = argv[i + 1]
            i += 2
        else:
            if not label:
                label = argv[i]
            i += 1
    return top_n, label, root


def main() -> int:
    top_n, label, root = _parse_args(sys.argv)

    inclusive: dict[str, int] = defaultdict(int)
    total = 0
    line_count = 0
    skipped_no_root = 0
    for raw in sys.stdin:
        line = raw.rstrip("\n")
        if not line:
            continue
        try:
            stack_part, count_part = line.rsplit(" ", 1)
            count = int(count_part)
        except ValueError:
            continue
        frames = [f.strip() for f in stack_part.split(";") if f.strip()]
        if root is not None:
            cut = None
            for idx, frame in enumerate(frames):
                if root in frame:
                    cut = idx
                    break
            if cut is None:
                skipped_no_root += 1
                continue
            frames = frames[cut:]
        line_count += 1
        total += count
        seen: set[str] = set()
        for frame in frames:
            if frame in seen:
                continue
            seen.add(frame)
            inclusive[frame] += count

    if total == 0:
        print(
            f"empty profile after filtering "
            f"(folded lines: {line_count + skipped_no_root}, "
            f"skipped-no-root: {skipped_no_root})",
            file=sys.stderr,
        )
        return 1

    rows = sorted(inclusive.items(), key=lambda kv: (-kv[1], kv[0]))[:top_n]

    print(
        f"<!-- folded lines kept: {line_count} "
        f"(skipped-no-root: {skipped_no_root}), "
        f"total samples in kept set: {total}"
        f"{', root=' + repr(root) if root else ''} -->"
    )
    if label:
        print(f"### Top {top_n} inclusive frames — `{label}`")
    else:
        print(f"### Top {top_n} inclusive frames")
    print()
    if root:
        print(
            f"_Total samples in stacks containing `{root}`: {total:,} · "
            f"Truncated folded stacks: {line_count:,}_"
        )
    else:
        print(f"_Total samples: {total:,} · Folded stacks: {line_count:,}_")
    print()
    print("| Rank | Inclusive % | Samples | Frame |")
    print("|---:|---:|---:|---|")
    for rank, (frame, samples) in enumerate(rows, start=1):
        pct = samples / total * 100
        display = re.sub(r"::h[0-9a-f]{16}$", "", frame)
        display = display.replace("|", r"\|")
        print(f"| {rank} | {pct:.2f}% | {samples:,} | `{display}` |")
    return 0


if __name__ == "__main__":
    sys.exit(main())
