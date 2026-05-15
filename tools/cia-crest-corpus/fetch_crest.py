#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
# SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
"""
fetch_crest.py — pull a curated set of CIA CREST PDFs (1990–2010) from Internet Archive.

Why IA and not cia.gov/readingroom?
- cia.gov is behind Akamai Bot Manager interstitial; every search/collection page
  requires solving a JS PoW challenge.
- Internet Archive mirrors individual CREST docs as separate items
  (e.g. https://archive.org/details/CIA-RDP96-00792R000400330013-4) with the
  original PDF and pre-OCR'd text — no bot protection, direct downloads.

Output goes under ``work/`` next to this script:
  work/pdfs/<identifier>.pdf
  work/manifest.json
"""

from __future__ import annotations

import json
import random
import re
import sys
import time
from pathlib import Path
from urllib.parse import quote, urlencode
from urllib.request import Request, urlopen

TOOL_ROOT = Path(__file__).resolve().parent
WORK_DIR = TOOL_ROOT / "work"
PDF_DIR = WORK_DIR / "pdfs"
MANIFEST = WORK_DIR / "manifest.json"

UA = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
IA_SEARCH = "https://archive.org/advancedsearch.php"
IA_DOWNLOAD = "https://archive.org/download/{ident}/{fname}"

# Queries chosen to surface a varied mix of 1990–2010 era docs across topics and formats.
# Each query is run separately so we can spread selection across themes.
QUERIES: list[tuple[str, str]] = [
    (
        "gulf_iraq",
        'creator:"Central Intelligence Agency" AND year:[1990 TO 1995] AND (title:"iraq" OR title:"kuwait" OR title:"saddam" OR title:"gulf")',
    ),
    (
        "ussr_collapse",
        'creator:"Central Intelligence Agency" AND year:[1990 TO 1995] AND (title:"soviet" OR title:"USSR" OR title:"yeltsin" OR title:"gorbachev" OR title:"russia")',
    ),
    (
        "balkans",
        'creator:"Central Intelligence Agency" AND year:[1992 TO 2000] AND (title:"yugoslav" OR title:"bosnia" OR title:"kosovo" OR title:"milosevic" OR title:"macedonia" OR title:"gligorov" OR title:"sarajevo" OR title:"croatia" OR title:"serbia")',
    ),
    (
        "korea",
        'creator:"Central Intelligence Agency" AND year:[1990 TO 2005] AND (title:"korea" OR title:"pyongyang" OR title:"DPRK")',
    ),
    (
        "china",
        'creator:"Central Intelligence Agency" AND year:[1990 TO 2005] AND (title:"china" OR title:"PRC" OR title:"taiwan")',
    ),
    (
        "africa",
        'creator:"Central Intelligence Agency" AND year:[1992 TO 2002] AND (title:"rwanda" OR title:"somalia" OR title:"sudan" OR title:"congo" OR title:"angola" OR title:"liberia")',
    ),
    (
        "terrorism",
        'creator:"Central Intelligence Agency" AND year:[1993 TO 2002] AND (title:"terror" OR title:"qaeda" OR title:"qa\'ida" OR title:"bin laden" OR title:"hijack")',
    ),
    (
        "oversight_congress",
        'creator:"Central Intelligence Agency" AND year:[1995 TO 2002] AND (title:"NRO" OR title:"oversight" OR title:"congress" OR title:"GAO" OR title:"legislative")',
    ),
    (
        "cia_internal",
        'creator:"Central Intelligence Agency" AND year:[1990 TO 2000] AND (title:"directorate" OR title:"organization" OR title:"policy" OR title:"administration")',
    ),
    (
        "lat_am_caribbean",
        'creator:"Central Intelligence Agency" AND year:[1990 TO 2002] AND (title:"haiti" OR title:"panama" OR title:"peru" OR title:"colombia" OR title:"noriega" OR title:"cuba" OR title:"nicaragua")',
    ),
    (
        "middle_east",
        'creator:"Central Intelligence Agency" AND year:[1990 TO 2005] AND (title:"iran" OR title:"syria" OR title:"lebanon" OR title:"israel" OR title:"palestine" OR title:"libya")',
    ),
    (
        "south_asia",
        'creator:"Central Intelligence Agency" AND year:[1990 TO 2005] AND (title:"afghanistan" OR title:"pakistan" OR title:"india" OR title:"kashmir")',
    ),
    ("intelligence_studies", 'collection:"cia-reports" AND mediatype:"texts"'),
    ("misc_1990_2010", 'creator:"Central Intelligence Agency" AND year:[1990 TO 2010]'),
]

# Target: 40+ documents, capped to keep disk usage and MinerU time reasonable.
TARGET_DOCS = 42
PER_BUCKET_MAX = 14
FALLBACK_BUCKET = "misc_1990_2010"

# PDF size guardrails (in bytes). Some scanned items are huge multi-volume releases.
PDF_MIN_SIZE = 30_000
PDF_MAX_SIZE = 25_000_000  # 25 MB

DATE_IN_TITLE = re.compile(
    r"\b("
    r"\d{1,2}[/-]\d{1,2}[/-](?:19|20)\d{2}"  # MM/DD/YYYY or MM-DD-YYYY
    r"|(?:19|20)\d{2}"  # bare YYYY
    r")\b"
)


def http_get_json(url: str) -> dict:
    req = Request(url, headers={"User-Agent": UA, "Accept": "application/json"})
    with urlopen(req, timeout=30) as r:  # noqa: S310  trusted IA host
        return json.load(r)


def ia_search(query: str, rows: int = 100) -> list[dict]:
    params = {
        "q": query,
        "fl[]": ["identifier", "title", "date", "year", "subject", "description"],
        "rows": rows,
        "output": "json",
        "sort[]": "date asc",
    }
    qs = urlencode(params, doseq=True)
    data = http_get_json(f"{IA_SEARCH}?{qs}")
    return data.get("response", {}).get("docs", [])


def parse_doc_year(doc: dict) -> int | None:
    # Prefer the explicit `date` field, but skip the IA-bulk default "YYYY-01-01"
    # when a more specific date isn't available — those years are unreliable.
    date = doc.get("date") or ""
    if date and not date.startswith("0000"):
        title_year = None
        m = DATE_IN_TITLE.search(doc.get("title", ""))
        if m:
            chunk = m.group(1)
            ty = re.search(r"(19|20)\d{2}", chunk)
            if ty:
                title_year = int(ty.group(0))
        if date.endswith("-01-01T00:00:00Z") and title_year:
            return title_year
        return int(date[:4])
    return None


def fetch_metadata(identifier: str) -> dict | None:
    try:
        return http_get_json(f"https://archive.org/metadata/{identifier}")
    except Exception as e:
        print(f"  ! metadata fetch failed for {identifier}: {e}", file=sys.stderr)
        return None


def pick_pdf_file(meta: dict) -> tuple[str, int] | None:
    """Return (filename, size_bytes) for the best PDF on the IA item, if any.

    Prefer source=original; fall back to any *.pdf (some items only have a derivative).
    """
    files = meta.get("files", [])
    candidates = [f for f in files if f.get("name", "").lower().endswith(".pdf")]
    if not candidates:
        return None
    candidates.sort(key=lambda f: 0 if f.get("source") == "original" else 1)
    f = candidates[0]
    try:
        size = int(f.get("size", "0"))
    except ValueError:
        size = 0
    return f["name"], size


def download(url: str, dest: Path) -> int:
    req = Request(url, headers={"User-Agent": UA})
    with urlopen(req, timeout=120) as r, dest.open("wb") as f:  # noqa: S310  trusted IA host
        total = 0
        while True:
            chunk = r.read(64 * 1024)
            if not chunk:
                break
            f.write(chunk)
            total += len(chunk)
    return total


def main() -> int:
    PDF_DIR.mkdir(parents=True, exist_ok=True)
    candidates: list[dict] = []
    seen: set[str] = set()
    rng = random.Random(20260514)  # deterministic but seeded variety

    for bucket, query in QUERIES:
        print(f"\n[query] {bucket}\n  q={query}")
        try:
            docs = ia_search(query, rows=200)
        except Exception as e:
            print(f"  ! search failed: {e}", file=sys.stderr)
            continue

        ranged = []
        for d in docs:
            y = parse_doc_year(d)
            if y is None or not (1990 <= y <= 2010):
                continue
            if d["identifier"] in seen:
                continue
            d["_year"] = y
            d["_bucket"] = bucket
            ranged.append(d)

        rng.shuffle(ranged)
        ranged.sort(key=lambda d: d["_year"])
        cap = len(ranged) if bucket == FALLBACK_BUCKET else PER_BUCKET_MAX
        chosen = ranged[:cap]
        print(f"  -> {len(ranged)} candidates, taking {len(chosen)}")

        for d in chosen:
            seen.add(d["identifier"])
            candidates.append(d)

    print(f"\n[selected] {len(candidates)} candidates before download verification")

    manifest = []
    for doc in candidates:
        if len(manifest) >= TARGET_DOCS:
            break
        ident = doc["identifier"]

        print(f"\n[doc] {ident}  ({doc['_year']})  {doc.get('title', '')[:70]}")
        meta = fetch_metadata(ident)
        if not meta:
            continue

        pdf = pick_pdf_file(meta)
        if not pdf:
            print("  ! no original PDF in item")
            continue
        fname, size = pdf
        if size < PDF_MIN_SIZE or size > PDF_MAX_SIZE:
            print(f"  ! pdf size out of range ({size} bytes)")
            continue

        dest = PDF_DIR / f"{ident}.pdf"
        if dest.exists() and dest.stat().st_size == size:
            print(f"  = already on disk ({size} bytes)")
        else:
            url = IA_DOWNLOAD.format(ident=ident, fname=quote(fname))
            try:
                wrote = download(url, dest)
                print(f"  + downloaded {wrote} bytes")
            except Exception as e:
                print(f"  ! download failed: {e}")
                if dest.exists():
                    dest.unlink()
                continue

        manifest.append({
            "identifier": ident,
            "title": doc.get("title", ""),
            "year": doc["_year"],
            "ia_date": doc.get("date"),
            "bucket": doc["_bucket"],
            "pdf_filename": dest.name,
            "pdf_size": size,
            "ia_url": f"https://archive.org/details/{ident}",
            "pdf_url": IA_DOWNLOAD.format(ident=ident, fname=quote(fname)),
        })

        time.sleep(0.6)  # polite pacing

    MANIFEST.write_text(json.dumps(manifest, indent=2))
    print(f"\n[done] {len(manifest)} PDFs in {PDF_DIR}; manifest at {MANIFEST}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
