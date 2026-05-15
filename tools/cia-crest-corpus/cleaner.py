#!/usr/bin/env -S uv run --script
# ///script
# requires-python: ">=3.10"
# ///
# SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
"""
cleaner.py — page-aware extraction of clean prose from MinerU output.

Shared by extract_clean.py (flat one-file-per-doc) and make_specs.py
(structured per-page scaffold). The regexes here strip declassification
furniture so the prose can be re-marked by hand against known ground truth.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path
import json

# ---- patterns to strip --------------------------------------------------

# Stamps applied by the declass office at release time — never part of the doc.
STAMP_LINES = re.compile(
    r"""(?ix)
    ^\s*(
       (sanitized\s+copy\s+)?approved\s+for\s+release\s+\d{4}\s*[/\-]\s*\d{2}\s*[/\-]\s*\d{2}
       (\s*:\s*CIA-?RDP[\w\-]+)?\s*\)?[\w\-]*
       | declassified\s+(in\s+part|and\s+approved)\b.*
       | sanitized\s+copy\s+approved\s+for\s+release\b.*
    )\s*$
    """,
)

# A bare release identifier on its own line (sometimes the OCR splits it off)
BARE_RDP = re.compile(r"^\s*CIA-?RDP[\w\-]{8,}\s*$", re.I)

_CLASS_LEVEL = r"(?:TOP\s+SECRET|SECRET|CONFIDENTIAL|UNCLASSIFIED|RESTRICTED)"
_DISSEM = r"(?:NOFORN|REL\s+TO\s+[A-Z, ]+|FOUO|ORCON|PROPIN|RELIDO|IMCON|FISA|DSEN)"
_SCI = r"(?:HCS|SI|TK|G|HCS-[A-Z]+|SI-G|SI-ECI-[A-Z]+|TK-[A-Z]+)"
STANDALONE_BANNER = re.compile(
    rf"""(?ix)
    ^\s*
    (?:\(?\s*)?
    (?:{_CLASS_LEVEL}|UNCLASS(?:IFIED)?|U)
    (?:\s*//\s*(?:{_SCI}|{_DISSEM}|[A-Z]+))*
    (?:\s*\)?)?\s*
    $
    """,
)

LEADING_PORTION = re.compile(
    r"""(?ix)
    ^\s*
    \(\s*
    (?:TS|S|C|U|R)
    (?:\s*//\s*[A-Z0-9/ ,_-]+)?
    \s*\)
    \s+
    """,
)

HANDLING_BOILERPLATE = re.compile(
    r"""(?ix)
    ^\s*(
       warning\s*notice\s*:?\s*intelligence\s+sources.*
       | this\s+document\s+contains\s+classified\s+national\s+security\s+information.*
       | not\s+releasable\s+to\s+foreign\s+nationals\s*$
       | handle\s+via\s+[A-Z]+\s+channels?\s+only.*
       | copy\s+\d+\s+of\s+\d+\s*$
       | next\s+\d+\s+page\(s\)\s+in\s+document\s+denied\s*$
       | sanitized\s*$
       # Cable-style classification + station/DTG mashed onto one line:
       #   "CONFIDENTIALLIMA O625", "SECRETWASHDC 1234"
       | (?:top\s+secret|secret|confidential|unclassified|restricted)
         [A-Z]{2,}\s+\w+\s*
       # Numbered classification declaration: "1. CONFIDENTIAL - ENTIRE TEXT."
       | \d+\.\s*(?:top\s+secret|secret|confidential|unclassified|restricted)
         \s*-\s*entire\s*text\.?\s*
    )\s*$
    """,
)


def _line_should_drop(line: str) -> bool:
    return bool(
        STAMP_LINES.match(line)
        or BARE_RDP.match(line)
        or STANDALONE_BANNER.match(line)
        or HANDLING_BOILERPLATE.match(line)
    )


def clean_line(line: str) -> str | None:
    """Return cleaned line, or None if it should be dropped entirely."""
    if not line.strip():
        return ""
    if _line_should_drop(line):
        return None
    return LEADING_PORTION.sub("", line)


def clean_paragraph(text: str) -> str:
    """Apply line-level cleaning across a paragraph; return joined text."""
    out: list[str] = []
    for line in text.splitlines():
        cl = clean_line(line)
        if cl is None:
            continue
        out.append(cl)
    return "\n".join(out).strip()


# ---- page-aware extraction ----------------------------------------------


@dataclass
class Paragraph:
    text: str  # cleaned prose (may include rendered tables as HTML)
    kind: str  # "text" | "title" | "table"


@dataclass
class Page:
    page_idx: int  # 0-based MinerU page index
    paragraphs: list[Paragraph]


def extract_pages(
    content_list_path: Path,
    max_pages: int | None = 2,
    min_chars_per_page: int = 0,
) -> list[Page]:
    """Read MinerU content_list.json, return a list of cleaned Pages.

    max_pages caps how many MinerU pages we return; None means "all".
    min_chars_per_page is a guard: pages with less than this many surviving
    characters get rolled forward (the next page is appended) to avoid
    one-paragraph "pages" that are just an artifact of MinerU layout splits.
    """
    items = json.loads(Path(content_list_path).read_text())
    page_buckets: dict[int, list[Paragraph]] = {}

    for item in items:
        page = item.get("page_idx", 0)
        t = item.get("type")
        if t == "text":
            txt = clean_paragraph(item.get("text", ""))
            if txt:
                page_buckets.setdefault(page, []).append(Paragraph(txt, "text"))
        elif t == "title":
            txt = clean_paragraph(item.get("text", ""))
            if txt:
                page_buckets.setdefault(page, []).append(
                    Paragraph(f"## {txt}", "title")
                )
        elif t == "table":
            html = (item.get("table_body") or item.get("html") or "").strip()
            if html:
                cleaned_html = STAMP_LINES.sub("", html).strip()
                if cleaned_html:
                    page_buckets.setdefault(page, []).append(
                        Paragraph(cleaned_html, "table")
                    )
        # 'header', 'footer', 'image' are intentionally dropped — header/footer
        # are the OCR'd banner stamps; images aren't useful for this corpus.

    # Build ordered list, optionally merging too-short pages into the next one
    ordered = sorted(page_buckets.keys())
    pages: list[Page] = []
    carry: list[Paragraph] = []
    for pi in ordered:
        paras = carry + page_buckets[pi]
        carry = []
        char_count = sum(len(p.text) for p in paras)
        if min_chars_per_page and char_count < min_chars_per_page:
            carry = paras
            continue
        pages.append(Page(pi, paras))
        if max_pages is not None and len(pages) >= max_pages:
            break

    # If there's a carry left and we haven't hit max_pages yet, attach it
    if carry and (max_pages is None or len(pages) < max_pages):
        pages.append(Page(ordered[-1], carry))

    return pages
