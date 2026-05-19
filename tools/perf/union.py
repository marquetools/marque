#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Knitli Inc.
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
"""Compute the union of inclusive samples across a set of frames."""
import sys
from collections import Counter

if len(sys.argv) < 2:
    print("usage: union.py FRAME1[,FRAME2,...] < folded.txt", file=sys.stderr)
    sys.exit(2)
targets = set(sys.argv[1].split(","))
root = sys.argv[2] if len(sys.argv) > 2 else None

total = 0
union_count = 0
per_frame = Counter()
for line in sys.stdin:
    line = line.strip()
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
        for idx, f in enumerate(frames):
            if root in f:
                cut = idx; break
        if cut is None:
            continue
        frames = frames[cut:]
    total += count
    hits = {f for f in frames if any(t in f for t in targets)}
    if hits:
        union_count += count
        for t in targets:
            if any(t in f for f in frames):
                per_frame[t] += count
print(f"Total samples (rooted): {total}")
print(f"Union count: {union_count}  ({union_count/total*100:.2f}%)")
print("Per-frame inclusive counts (with overlap):")
for t in sorted(per_frame.keys()):
    print(f"  {per_frame[t]:6d}  ({per_frame[t]/total*100:.2f}%)  {t}")
print(f"Sum-of-inclusive (the over-counting figure): {sum(per_frame.values())}  ({sum(per_frame.values())/total*100:.2f}%)")
print(f"Overlap: {sum(per_frame.values()) - union_count} samples ({(sum(per_frame.values())-union_count)/total*100:.2f}% of total)")
