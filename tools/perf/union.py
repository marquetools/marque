#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Knitli Inc.
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
"""Compute the union of inclusive samples across a set of target frames.

Reads inferno-style folded stacks on stdin (`stack;stack;stack count`
per line) and reports the number of samples whose stack contains at
least one frame matching any of the supplied targets.

Usage:
  union.py TARGET1[,TARGET2,...] [ROOT_SUBSTR] < folded.txt

TARGETS are **substrings** matched against each frame name (NOT exact
frame names — Rust's symbolicated names contain templates, paths, and
trait-impl decorations that make exact matching painful in practice;
substring matching lets a single target like
'CanonicalAttrs as core::clone::Clone' catch both the base mono and
its `.NNNN` callsite suffixes). Be aware that a short target may
match more frames than intended; pick targets long enough to be
unambiguous in your folded data.

ROOT_SUBSTR (optional) restricts attribution to stacks containing a
frame whose name contains the given substring. For matching stacks,
frames above the first occurrence of ROOT_SUBSTR are dropped. Use
this to strip criterion harness / libc start frames.

Output:
  - Total samples (in the rooted set, or all stacks if no root)
  - Union count: stacks containing ANY target
  - Per-target inclusive count and percentage (these OVERLAP and
    summing them double-counts call-chain ancestry — that's the
    over-counting figure)
  - Overlap: sum-of-inclusive minus union (the parent-child
    call-chain overlap revealed by this analysis)
"""

from __future__ import annotations

import sys
from collections import Counter


def main() -> int:
    if len(sys.argv) < 2:
        print("usage: union.py TARGET1[,TARGET2,...] [ROOT_SUBSTR] < folded.txt",
              file=sys.stderr)
        return 2
    targets = sys.argv[1].split(",")
    root = sys.argv[2] if len(sys.argv) > 2 else None

    total = 0
    union_count = 0
    per_frame: Counter[str] = Counter()
    for raw in sys.stdin:
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        try:
            stack_part, count_part = line.rsplit(" ", 1)
            count = int(count_part)
        except ValueError:
            continue
        frames = [f.strip() for f in stack_part.split(";") if f.strip()]
        if root:
            cut = None
            for idx, frame in enumerate(frames):
                if root in frame:
                    cut = idx
                    break
            if cut is None:
                continue
            frames = frames[cut:]
        total += count
        hits = any(t in f for t in targets for f in frames)
        if hits:
            union_count += count
        for t in targets:
            if any(t in f for f in frames):
                per_frame[t] += count

    if total == 0:
        scope = f"stacks containing root substring {root!r}" if root else "folded input"
        print(f"empty profile: no samples in {scope}", file=sys.stderr)
        return 1

    print(f"Total samples (rooted): {total}")
    print(f"Union count: {union_count}  ({union_count / total * 100:.2f}%)")
    print("Per-frame inclusive counts (substring match; with overlap):")
    for t in sorted(per_frame.keys()):
        print(f"  {per_frame[t]:6d}  ({per_frame[t] / total * 100:.2f}%)  {t}")
    sum_inclusive = sum(per_frame.values())
    print(
        f"Sum-of-inclusive (the over-counting figure): "
        f"{sum_inclusive}  ({sum_inclusive / total * 100:.2f}%)"
    )
    print(
        f"Overlap: {sum_inclusive - union_count} samples "
        f"({(sum_inclusive - union_count) / total * 100:.2f}% of total)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
