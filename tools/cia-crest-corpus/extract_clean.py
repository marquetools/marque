#!/usr/bin/env -S uv run --script
# ///script
# requires-python: ">=3.10"
# ///
# SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
"""
extract_clean.py — produce a 1-2 page clean text excerpt per CREST doc.

For each entry in work/manifest.json:
  - Read MinerU's _content_list.json (paragraph/table items with page_idx)
  - Take all items from page 0 (and page 1 if page 0 has too little usable text)
  - Drop the OCR'd declassification stamps, originals' banner lines,
    leading portion marks, and other declass furniture
  - Emit work/clean/<identifier>.md — text only, no markings, no images,
    suitable as input for the marking generator

Run MinerU separately to produce work/md/<stem>/ocr/<stem>_content_list.json
files; this script only consumes them.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

from cleaner import (
    BARE_RDP,
    HANDLING_BOILERPLATE,
    STAMP_LINES,
    STANDALONE_BANNER,
    clean_line,
)

TOOL_ROOT = Path(__file__).resolve().parent
WORK_DIR = TOOL_ROOT / "work"
MD_ROOT = WORK_DIR / "md"
CLEAN = WORK_DIR / "clean"
MANIFEST = WORK_DIR / "manifest.json"
CLEAN_MANIFEST = WORK_DIR / "clean_manifest.json"


def items_to_markdown(items: list[dict], target_pages: int = 2) -> tuple[str, dict]:
    """Concatenate MinerU items from the first `target_pages` pages, cleaned."""
    chunks: list[str] = []
    skipped = {"stamp": 0, "banner": 0, "rdp": 0, "boilerplate": 0}

    for item in items:
        page = item.get("page_idx", 0)
        if page >= target_pages:
            break

        t = item.get("type")
        if t == "text":
            txt = item.get("text", "")
            kept_lines: list[str] = []
            for line in txt.splitlines():
                cl = clean_line(line)
                if cl is None:
                    if STAMP_LINES.match(line):
                        skipped["stamp"] += 1
                    elif STANDALONE_BANNER.match(line):
                        skipped["banner"] += 1
                    elif BARE_RDP.match(line):
                        skipped["rdp"] += 1
                    elif HANDLING_BOILERPLATE.match(line):
                        skipped["boilerplate"] += 1
                    continue
                kept_lines.append(cl)
            cleaned = "\n".join(ln for ln in kept_lines if ln.strip() or not kept_lines)
            if cleaned.strip():
                chunks.append(cleaned.strip())
        elif t == "table":
            html = item.get("table_body") or item.get("html") or ""
            if html.strip():
                cleaned_html = STAMP_LINES.sub("", html)
                chunks.append(cleaned_html.strip())
        elif t == "title":
            txt = item.get("text", "")
            cleaned = clean_line(txt)
            if cleaned:
                chunks.append(f"## {cleaned.strip()}")

    return "\n\n".join(chunks), skipped


def main() -> int:
    if not MANIFEST.exists():
        print(
            f"manifest not found at {MANIFEST}; run fetch_crest.py first",
            file=sys.stderr,
        )
        return 1
    manifest = json.loads(MANIFEST.read_text())
    CLEAN.mkdir(parents=True, exist_ok=True)
    out = []

    for entry in manifest:
        stem = Path(entry["pdf_filename"]).stem
        cl_json = MD_ROOT / stem / "ocr" / f"{stem}_content_list.json"
        if not cl_json.exists():
            print(f"[skip] {stem}: no content_list.json")
            continue
        items = json.loads(cl_json.read_text())

        # Page 0 sometimes has only a routing slip / cover. Walk pages until we
        # collect roughly a "page or two" worth of real prose.
        page0_chars = sum(
            len(i.get("text", ""))
            for i in items
            if i.get("page_idx") == 0 and i.get("type") == "text"
        )
        target_pages = 1 if page0_chars >= 600 else 2
        md, skipped = items_to_markdown(items, target_pages=target_pages)

        # If we still don't have enough text, widen until we do or we run out
        max_page = max((i.get("page_idx", 0) for i in items), default=0)
        while len(md) < 400 and target_pages <= max_page:
            target_pages += 1
            md, skipped = items_to_markdown(items, target_pages=target_pages)

        (CLEAN / f"{stem}.md").write_text(md, encoding="utf-8")
        out.append({
            "identifier": entry["identifier"],
            "stem": stem,
            "pages_kept": target_pages,
            "stripped": skipped,
            "char_count": len(md),
        })
        print(f"[ok] {stem}: {len(md)} chars, stripped {sum(skipped.values())} lines")

    CLEAN_MANIFEST.write_text(json.dumps(out, indent=2))
    print(f"\nwrote {len(out)} cleaned excerpts to {CLEAN}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
