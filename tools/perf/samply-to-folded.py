#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Knitli Inc.
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
"""
Convert samply's Firefox Profiler JSON to inferno-style folded
stacks: one line per leaf stack as `frame_root;frame...;frame_leaf count`.

Usage:
  samply-to-folded.py PROFILE.json[.gz] [--syms SYMS.json] [--thread NAME_SUBSTR]

If `--syms` is provided (sidecar from `samply record --unstable-presymbolicate`)
frames are symbolicated via per-library RVA -> symbol lookup.

If `--thread` is omitted, the thread with the most samples is selected
(usually the bench's main thread). The 'samply' control thread is always
excluded.

Pair with `tools/perf/top-n-inclusive.py` to produce the markdown tables
under `docs/perf/<DATE>-diagnosis/`. See `lint-flamegraph-top15.md`
methodology section for the full capture pipeline.
"""

from __future__ import annotations

import bisect
import gzip
import json
import sys
from collections import Counter


def _load_json(path: str) -> dict:
    if path.endswith(".gz"):
        with gzip.open(path, "rt", encoding="utf-8") as fh:
            return json.load(fh)
    with open(path, "r", encoding="utf-8") as fh:
        return json.load(fh)


def _build_sym_resolver(syms: dict) -> dict:
    """
    Return a dict {debug_name: (sorted_rvas, sizes, symbol_strings)}.
    Each library is a parallel-array store so we can binary-search by
    RVA in O(log n) and slice into the corresponding symbol name in O(1).
    """
    string_table: list[str] = syms["string_table"]
    out: dict[str, tuple[list[int], list[int], list[str]]] = {}
    for entry in syms.get("data", []):
        name = entry["debug_name"]
        st = entry["symbol_table"]
        st_sorted = sorted(st, key=lambda s: s["rva"])
        rvas = [s["rva"] for s in st_sorted]
        sizes = [s["size"] for s in st_sorted]
        names = [string_table[s["symbol"]] for s in st_sorted]
        out[name] = (rvas, sizes, names)
    return out


def _resolve_symbol(
    resolver: dict, debug_name: str, rva: int
) -> str | None:
    entry = resolver.get(debug_name)
    if entry is None or rva < 0:
        return None
    rvas, sizes, names = entry
    idx = bisect.bisect_right(rvas, rva) - 1
    if idx < 0:
        return None
    if rva < rvas[idx] + sizes[idx]:
        return names[idx]
    # In the gap between two symbols; report the closest preceding with offset.
    return f"{names[idx]}+0x{rva - rvas[idx]:x}"


def _pick_thread(threads: list[dict], hint: str | None) -> dict:
    candidates = [
        t for t in threads
        if t.get("name") and t["name"] != "samply"
        and len(t.get("samples", {}).get("stack", [])) > 0
    ]
    if not candidates:
        raise SystemExit("no bench threads with samples found")
    if hint:
        match = [t for t in candidates if hint in t.get("name", "")]
        if match:
            candidates = match
    return max(candidates, key=lambda t: len(t["samples"]["stack"]))


def _resolve_frame_name(
    thread: dict,
    profile_libs: list[dict],
    resolver: dict,
    frame_idx: int,
) -> str:
    rva = thread["frameTable"]["address"][frame_idx]
    func_idx = thread["frameTable"]["func"][frame_idx]
    func_name = thread["stringArray"][thread["funcTable"]["name"][func_idx]]
    res_idx = thread["funcTable"]["resource"][func_idx]
    if resolver and res_idx >= 0 and rva >= 0:
        lib_idx = thread["resourceTable"]["lib"][res_idx]
        if 0 <= lib_idx < len(profile_libs):
            debug_name = (
                profile_libs[lib_idx].get("debugName")
                or profile_libs[lib_idx].get("name")
            )
            if debug_name:
                sym = _resolve_symbol(resolver, debug_name, rva)
                if sym:
                    return sym
    return func_name


def _walk_stack(
    thread: dict,
    profile_libs: list[dict],
    resolver: dict,
    stack_idx: int,
    cache: dict[int, tuple[str, ...]],
) -> tuple[str, ...]:
    if stack_idx in cache:
        return cache[stack_idx]
    parents: list[int] = []
    cur: int | None = stack_idx
    while cur is not None:
        parents.append(cur)
        cur = thread["stackTable"]["prefix"][cur]
    parents.reverse()
    frames: list[str] = []
    for sidx in parents:
        fidx = thread["stackTable"]["frame"][sidx]
        frames.append(_resolve_frame_name(thread, profile_libs, resolver, fidx))
    result = tuple(frames)
    cache[stack_idx] = result
    return result


def main() -> int:
    args = sys.argv[1:]
    syms_path: str | None = None
    thread_hint: str | None = None
    profile_path: str | None = None
    i = 0
    while i < len(args):
        a = args[i]
        if a == "--syms" and i + 1 < len(args):
            syms_path = args[i + 1]
            i += 2
        elif a == "--thread" and i + 1 < len(args):
            thread_hint = args[i + 1]
            i += 2
        elif not a.startswith("-") and profile_path is None:
            profile_path = a
            i += 1
        else:
            print(f"unexpected arg: {a}", file=sys.stderr)
            return 2
    if profile_path is None:
        print("usage: samply-to-folded.py PROFILE [--syms SYMS] [--thread NAME]",
              file=sys.stderr)
        return 2

    profile = _load_json(profile_path)
    if not isinstance(profile, dict) or "threads" not in profile:
        raise SystemExit("input is not a Firefox-profiler / samply JSON")

    resolver: dict = {}
    if syms_path:
        syms = _load_json(syms_path)
        resolver = _build_sym_resolver(syms)

    thread = _pick_thread(profile["threads"], thread_hint)
    samples = thread["samples"]["stack"]
    profile_libs = profile.get("libs", [])

    folded: Counter[tuple[str, ...]] = Counter()
    cache: dict[int, tuple[str, ...]] = {}

    for stack_idx in samples:
        if stack_idx is None:
            continue
        frames = _walk_stack(thread, profile_libs, resolver, stack_idx, cache)
        if frames:
            folded[frames] += 1

    for frames, count in folded.items():
        print(";".join(frames) + " " + str(count))
    print(
        f"# samply-to-folded: thread={thread['name']!r} "
        f"samples={sum(folded.values())} unique_stacks={len(folded)} "
        f"resolver={'syms' if resolver else 'none'}",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
