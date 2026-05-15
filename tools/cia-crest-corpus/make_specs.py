# SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
"""
make_specs.py — scaffold per-doc, per-page editable spec files.

For each entry in work/manifest.json, writes ``tests/corpus/documents/specs/<stem>.md``
with:
  - YAML frontmatter (identifier, title, year, CAB placeholders)
  - One `=== page N ===` section per MinerU page, each starting with
    `banner: ???`
  - Each cleaned paragraph prefixed with `(?)` as a portion-mark placeholder

You then open each spec and replace the placeholders by hand (or run
randomize_portions.py to fill portion marks from portions.toml).
render_corpus.py reads the result.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from cleaner import Page, extract_pages

TOOL_ROOT = Path(__file__).resolve().parent
REPO_ROOT = TOOL_ROOT.parents[1]
WORK_DIR = TOOL_ROOT / "work"
MD_ROOT = WORK_DIR / "md"
MANIFEST = WORK_DIR / "manifest.json"
DEFAULT_SPECS = REPO_ROOT / "tests" / "corpus" / "documents" / "specs"


FRONTMATTER_TEMPLATE = """\
---
identifier: {identifier}
title: "{title}"
year: {year}
source_pdf: {pdf_url}
# Fill these by hand. Anything left as ??? at render time becomes a warning, not an error.
cab:
  classified_by: ???
  derived_from: ???
  declassify_on: ???
---

"""


def escape_title_for_yaml(s: str) -> str:
    return s.replace('"', "'")


def render_page_block(page: Page, page_num: int) -> str:
    out: list[str] = [f"=== page {page_num} ==="]
    out.append("banner: ???")
    out.append("")
    for para in page.paragraphs:
        text = para.text
        if para.kind == "table":
            out.append("(?)")
            out.append("```table")
            out.append(text)
            out.append("```")
            out.append("")
        elif para.kind == "title":
            out.append(f"(?) {text}")
            out.append("")
        else:
            out.append(f"(?) {text}")
            out.append("")
    return "\n".join(out).rstrip() + "\n\n"


def write_spec(entry: dict, pages: list[Page], dest: Path) -> None:
    body = FRONTMATTER_TEMPLATE.format(
        identifier=entry["identifier"],
        title=escape_title_for_yaml(entry["title"]),
        year=entry["year"],
        pdf_url=entry["pdf_url"],
    )
    for i, page in enumerate(pages, start=1):
        body += render_page_block(page, i)
    dest.write_text(body, encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--force", action="store_true", help="overwrite existing specs")
    ap.add_argument("--max-pages", type=int, default=2, help="pages per spec (default 2)")
    ap.add_argument(
        "--specs",
        type=Path,
        default=DEFAULT_SPECS,
        help="output directory for spec files (default: tests/corpus/documents/specs)",
    )
    args = ap.parse_args()

    if not MANIFEST.exists():
        print(f"manifest not found at {MANIFEST}; run fetch_crest.py first", file=sys.stderr)
        return 1

    args.specs.mkdir(parents=True, exist_ok=True)
    manifest = json.loads(MANIFEST.read_text())

    created = 0
    skipped = 0
    for entry in manifest:
        stem = Path(entry["pdf_filename"]).stem
        dest = args.specs / f"{stem}.md"
        if dest.exists() and not args.force:
            skipped += 1
            continue

        cl_json = MD_ROOT / stem / "ocr" / f"{stem}_content_list.json"
        if not cl_json.exists():
            print(f"[skip] {stem}: no MinerU content_list.json")
            continue

        pages = extract_pages(cl_json, max_pages=args.max_pages, min_chars_per_page=200)
        if not pages:
            pages = extract_pages(cl_json, max_pages=None)
            if not pages:
                print(f"[skip] {stem}: no usable text")
                continue

        write_spec(entry, pages, dest)
        created += 1
        print(f"[ok] specs/{dest.name}  ({len(pages)} page(s))")

    print(f"\ncreated {created} spec files; skipped {skipped} existing (use --force to overwrite)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
