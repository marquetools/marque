#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = [
#     "pyyaml>=6.0.3",
# ]
# ///
# SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
"""
render_corpus.py — read hand-classified specs/*.md, emit the marked corpus.

For each spec file:
  - Parse YAML frontmatter (identifier, title, year, source_pdf, cab.*)
  - Parse page blocks separated by `=== page N ===` lines
  - Each page has a `banner:` line and one or more paragraphs
  - Each paragraph optionally begins with `(MARK)` — anything else is treated
    as an unmarked / unclassified paragraph (a warning is emitted)

Outputs (all under ``tests/corpus/documents/`` by default):
  - marked/<stem>.md        rendered document with per-page banners,
                            portion-marked paragraphs, CAB, and a footer
                            banner per page
  - ground_truth.json       aggregate ground truth (one record per doc)
  - <stem>.expected.json    per-doc fixture metadata in the marque test-corpus
                            schema: ``{"diagnostics": [], "ground_truth": {...}}``

Validation is gentle: unfilled banners or portion marks become warnings, not
errors. That lets you iterate — render early, see what's missing, fill in
more, render again.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

import yaml

TOOL_ROOT = Path(__file__).resolve().parent
REPO_ROOT = TOOL_ROOT.parents[1]
DEFAULT_CORPUS_DIR = REPO_ROOT / "tests" / "corpus" / "documents"

FRONTMATTER_RE = re.compile(r"^---\s*\n(.*?)\n---\s*\n", re.S)
PAGE_HEADER_RE = re.compile(r"^===\s*page\s+(\d+)\s*===\s*$", re.M)
BANNER_LINE_RE = re.compile(r"^banner:\s*(.+?)\s*$", re.M)
PORTION_PREFIX_RE = re.compile(r"^\(([^)]+)\)\s*(.*)$", re.S)

UNFILLED_BANNER = "???"
UNFILLED_PORTION = "?"


@dataclass
class ParsedParagraph:
    mark: str | None
    text: str
    is_table_block: bool = False


@dataclass
class ParsedPage:
    page_num: int
    banner: str
    paragraphs: list[ParsedParagraph] = field(default_factory=list)


@dataclass
class ParsedSpec:
    identifier: str
    title: str
    year: int | None
    source_pdf: str | None
    cab: dict | None
    pages: list[ParsedPage]
    warnings: list[str] = field(default_factory=list)


# ---- parsing ------------------------------------------------------------


def _split_paragraphs(body: str) -> list[str]:
    """Split a page body into paragraph chunks (blank-line separated).
    Fenced ```table blocks are kept intact even if they contain blank lines.
    """
    paras: list[str] = []
    buf: list[str] = []
    in_fence = False
    for line in body.splitlines():
        if line.strip().startswith("```"):
            in_fence = not in_fence
            buf.append(line)
            continue
        if in_fence:
            buf.append(line)
            continue
        if not line.strip():
            if buf:
                paras.append("\n".join(buf).rstrip())
                buf = []
        else:
            buf.append(line)
    if buf:
        paras.append("\n".join(buf).rstrip())
    return paras


def _extract_mark(paragraph: str) -> tuple[str | None, str]:
    """Return (mark, remaining_text) for a paragraph that may start with (X)."""
    m = PORTION_PREFIX_RE.match(paragraph)
    if not m:
        return None, paragraph
    return m.group(1).strip(), m.group(2).lstrip()


def _normalize_cab(raw_cab: object) -> dict[str, str] | None:
    if not isinstance(raw_cab, dict):
        return None
    normalized: dict[str, str] = {}
    for key, value in raw_cab.items():
        if value is None:
            continue
        text = str(value).strip()
        if text:
            normalized[str(key)] = text
    return normalized or None


def parse_spec(path: Path) -> ParsedSpec:
    text = path.read_text(encoding="utf-8")
    fm = FRONTMATTER_RE.match(text)
    if not fm:
        raise ValueError(f"{path.name}: missing YAML frontmatter")
    front = yaml.safe_load(fm.group(1)) or {}
    front_raw = yaml.load(fm.group(1), Loader=yaml.BaseLoader) or {}
    body = text[fm.end() :]

    parts = PAGE_HEADER_RE.split(body)
    warnings: list[str] = []
    pages: list[ParsedPage] = []
    if len(parts) < 3:
        warnings.append(
            "no `=== page N ===` markers found; treating whole body as page 1"
        )
        page_iter = [(1, body)]
    else:
        page_iter = list(zip(parts[1::2], parts[2::2]))

    for raw_num, raw_body in page_iter:
        page_num = int(raw_num) if isinstance(raw_num, str) else raw_num
        bm = BANNER_LINE_RE.search(raw_body)
        if not bm:
            banner = UNFILLED_BANNER
            warnings.append(f"page {page_num}: missing `banner:` line")
        else:
            banner = bm.group(1).strip()
            if banner == UNFILLED_BANNER:
                warnings.append(f"page {page_num}: banner left as `???`")
            raw_body = raw_body[: bm.start()] + raw_body[bm.end() :]

        page = ParsedPage(page_num=page_num, banner=banner)
        for raw_para in _split_paragraphs(raw_body):
            mark, rest = _extract_mark(raw_para)

            if rest.startswith("```table"):
                mark, rest = _extract_mark(raw_para)
                inner = rest.split("\n", 1)[1] if rest.startswith("```") else rest
                if inner.rstrip().endswith("```"):
                    inner = inner.rsplit("```", 1)[0].rstrip()
                page.paragraphs.append(
                    ParsedParagraph(mark=mark, text=inner, is_table_block=True)
                )
                continue

            if mark is None:
                warnings.append(
                    f"page {page_num}: paragraph has no leading (mark) — "
                    f"first 60 chars: {raw_para[:60]!r}"
                )
            elif mark == UNFILLED_PORTION:
                warnings.append(f"page {page_num}: portion mark left as `(?)`")
            page.paragraphs.append(ParsedParagraph(mark=mark, text=rest))
        pages.append(page)

    return ParsedSpec(
        identifier=front.get("identifier", path.stem),
        title=front.get("title", ""),
        year=front.get("year"),
        source_pdf=front.get("source_pdf"),
        cab=_normalize_cab(front_raw.get("cab")),
        pages=pages,
        warnings=warnings,
    )


# ---- rendering ----------------------------------------------------------


def render_cab(cab: dict | None) -> str:
    if not cab:
        return ""
    parts = []
    for key, label in [
        ("classified_by", "Classified By"),
        ("derived_from", "Derived From"),
        ("reason", "Reason"),
        ("declassify_on", "Declassify On"),
    ]:
        val = cab.get(key)
        if val:
            parts.append(f"{label}: {val}")
    return "\n".join(parts)


def render_marked(spec: ParsedSpec) -> str:
    out: list[str] = []
    for i, page in enumerate(spec.pages):
        out.append(page.banner)
        out.append("")
        for para in page.paragraphs:
            mark = (
                f"({para.mark}) " if para.mark and para.mark != UNFILLED_PORTION else ""
            )
            if para.is_table_block:
                out.append(mark.rstrip())
                out.append(para.text)
            else:
                out.append(f"{mark}{para.text}")
            out.append("")
        if i == len(spec.pages) - 1:
            cab = render_cab(spec.cab)
            if cab:
                out.append(cab)
                out.append("")
        out.append(page.banner)
        out.append("")
        if i < len(spec.pages) - 1:
            out.append("---")
            out.append("")
    return "\n".join(out).rstrip() + "\n"


def truth_record(spec: ParsedSpec) -> dict:
    return {
        "identifier": spec.identifier,
        "title": spec.title,
        "year": spec.year,
        "source_pdf": spec.source_pdf,
        "cab": spec.cab,
        "pages": [
            {
                "page_num": p.page_num,
                "banner": p.banner,
                "paragraphs": [
                    {"mark": pp.mark, "text": pp.text, "is_table": pp.is_table_block}
                    for pp in p.paragraphs
                ],
            }
            for p in spec.pages
        ],
        "warnings": spec.warnings,
    }


def expected_record(truth: dict) -> dict:
    """Per-doc fixture metadata in the marque test-corpus schema.

    The harness expects ``{"diagnostics": [...]}``; for these synthetic-positive
    documents, no diagnostics should fire. We tuck the structural ground truth
    into a ``ground_truth`` field that richer harnesses can consume.
    """
    return {"diagnostics": [], "ground_truth": truth}


# ---- main ---------------------------------------------------------------


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--corpus-dir",
        type=Path,
        default=DEFAULT_CORPUS_DIR,
        help="corpus root containing specs/ and where marked/ + JSON outputs land",
    )
    args = ap.parse_args()

    specs_dir = args.corpus_dir / "specs"
    marked_dir = args.corpus_dir / "marked"
    truth_path = args.corpus_dir / "ground_truth.json"

    if not specs_dir.exists():
        print(
            f"specs directory not found at {specs_dir} — run make_specs.py first",
            file=sys.stderr,
        )
        return 1
    marked_dir.mkdir(parents=True, exist_ok=True)

    truths: list[dict] = []
    total_warnings = 0
    parse_failures = 0
    for spec_path in sorted(specs_dir.glob("*.md")):
        try:
            spec = parse_spec(spec_path)
        except Exception as e:
            print(f"[fail] {spec_path.name}: {e}")
            parse_failures += 1
            continue
        marked = render_marked(spec)
        (marked_dir / spec_path.name).write_text(marked, encoding="utf-8")

        truth = truth_record(spec)
        truths.append(truth)

        # Per-doc .expected.json sibling under the corpus root.
        expected = expected_record(truth)
        expected_path = args.corpus_dir / f"{spec_path.stem}.expected.json"
        expected_path.write_text(json.dumps(expected, indent=2, default=str))

        if spec.warnings:
            total_warnings += len(spec.warnings)
            print(f"[warn] {spec_path.stem}: {len(spec.warnings)} warning(s)")
            for w in spec.warnings[:3]:
                print(f"       - {w}")
            if len(spec.warnings) > 3:
                print(f"       (+{len(spec.warnings) - 3} more)")
        else:
            print(f"[ok]   {spec_path.stem}")

    if parse_failures:
        print(
            f"\nfailed to parse {parse_failures} spec(s); aborting without writing aggregate ground truth",
            file=sys.stderr,
        )
        return 1

    truth_path.write_text(json.dumps(truths, indent=2, default=str))
    print(f"\nwrote {len(truths)} marked docs to {marked_dir}")
    print(f"ground truth: {truth_path}")
    print(f"per-doc fixtures: {args.corpus_dir}/<stem>.expected.json")
    if total_warnings:
        print(
            f"⚠  {total_warnings} warnings across the corpus (mostly unfilled ??? / (?))"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
