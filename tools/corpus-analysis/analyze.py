#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["requests==2.33.1"]
# ///
# SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
#
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

"""
Corpus analysis tool for classification marking vocabularies.

Three modes of operation, selected via ``--mode``:

``baseline`` (default)
    Token-frequency analysis. Given a token vocabulary (JSON) and a text
    corpus, measures how often each token appears in normal (non-IC)
    English text and in what structural contexts. Back-compatible with
    the original tool behavior — this was the only mode before Phase C.

``priors``
    Corpus-derived priors for the Phase-D decoder. Runs the baseline
    analysis, then reshapes the result into the schema consumed by
    ``crates/capco/build.rs`` at compile time (see
    ``crates/capco/corpus/README.md``). Output goes to a single
    ``priors.json`` file.

``mangled``
    Generates labeled mangled-marking fixtures for the Phase-D decoder
    accuracy harness. Walks a corpus, identifies high-confidence
    classification markings via pattern matching, applies one of six
    labeled mangling transforms per observed marking, and emits one
    JSON file per case under ``tests/fixtures/mangled/<class>/``. See
    ``tests/fixtures/mangled/README.md`` for schema and transform set.

Usage:
    # Baseline token frequencies (default mode)
    python analyze.py --output output/enron-full.json

    # Corpus-derived priors for the decoder (Phase D)
    python analyze.py --mode priors --output crates/capco/corpus/priors.json

    # Labeled mangled-marking fixtures (Phase D, SC-004 gate)
    python analyze.py --mode mangled \\
        --output tests/fixtures/mangled/ --min-cases 200

    # Custom corpus, any mode
    python analyze.py --corpus /path/to/text/files/ --mode mangled \\
        --output tests/fixtures/mangled/
"""
import contextlib

import argparse
import hashlib
import io
import json
import math
import os
import random
import re
import string
import sys
import tarfile
import time
import email
import email.policy
import zipfile as _zipfile  # stdlib zipfile; named to avoid shadowing locals
from collections import Counter
from datetime import date, datetime, timedelta, timezone
from pathlib import Path
from typing import Generator, Optional

# ---------------------------------------------------------------------------
# Corpus loading
# ---------------------------------------------------------------------------

ENRON_URL = "https://www.cs.cmu.edu/~enron/enron_mail_20150507.tar.gz"
ENRON_CACHE_DIR = Path(__file__).parent / ".cache"
ENRON_EXTRACT_DIR = ENRON_CACHE_DIR / "enron_mail"

# Congressional Record (via GovInfo.gov public bulk data — no API key required)
# Each daily package is ~30 MB compressed ZIP containing HTM + PDF files.
# Only the HTM files (~470 KB/day) are extracted; PDFs are discarded.
CREC_GOVINFO_BASE = "https://www.govinfo.gov/content/pkg"
CREC_CACHE_DIR_NAME = "congressional_record"

# GAO Reports (via GovInfo.gov public sitemaps — no API key required)
# Each report is fetched as a single ~66 KB HTML file.
# Sitemaps are available for report years 1989–2008; later years not indexed.
GAO_SITEMAP_BASE = "https://www.govinfo.gov/sitemap"
GAO_CACHE_DIR_NAME = "gao_reports"

# Polite delay between successive GovInfo HTTP requests.
_GOVINFO_DELAY_S = 0.3

# CIA CREST documents via Internet Archive (no API key required).
# Documents are indexed under identifier:CIA-RDP* with OCR'd text available
# as {identifier}_djvu.txt (DjVu text layer) or {identifier}.txt.
# The IA Advanced Search JSON API is at https://archive.org/advancedsearch.php.
CREST_IA_SEARCH_URL = "https://archive.org/advancedsearch.php"
CREST_IA_DOWNLOAD_BASE = "https://archive.org/download"
CREST_CACHE_DIR_NAME = "crest_documents"
_IA_DELAY_S = 0.5  # Internet Archive is a nonprofit; be generous with delays


# ---------------------------------------------------------------------------
# HTML text extraction (GovInfo HTML wraps content in <pre> tags)
# ---------------------------------------------------------------------------


def strip_html(html_text: str) -> str:
    """
    Extract plain text from GovInfo HTML.

    Both CREC and GAO reports from GovInfo wrap their full text inside a
    ``<pre>`` block.  This function pulls that content out and strips any
    inline tags (the only one present in practice is ``<a href>``) so that
    downstream analysis sees clean ASCII prose.

    Falls back to a generic "strip all tags" pass for HTML that does not
    follow the ``<pre>`` pattern.
    """
    pre_match = re.search(r"<pre[^>]*>(.*?)</pre>", html_text, re.DOTALL | re.IGNORECASE)
    if pre_match:
        inner = pre_match.group(1)
        # Strip inline tags (e.g. <a href="…">link</a> → link)
        return re.sub(r"<[^>]+>", " ", inner)
    # Generic fallback
    text = re.sub(r"<[^>]+>", " ", html_text)
    return re.sub(r"\s+", " ", text).strip()


def download_enron() -> Path:
    """Download and extract the Enron email corpus. Returns path to maildir root."""
    if ENRON_EXTRACT_DIR.exists():
        print(f"Using cached Enron corpus at {ENRON_EXTRACT_DIR}", file=sys.stderr)
        return ENRON_EXTRACT_DIR

    import requests

    tar_path = ENRON_CACHE_DIR / "enron_mail_20150507.tar.gz"
    ENRON_CACHE_DIR.mkdir(parents=True, exist_ok=True)

    if not tar_path.exists():
        print(f"Downloading Enron corpus from {ENRON_URL}...", file=sys.stderr)
        print("(~423 MB, this will take a few minutes)", file=sys.stderr)
        resp = requests.get(ENRON_URL, stream=True)
        resp.raise_for_status()
        total = int(resp.headers.get("content-length", 0))
        downloaded = 0
        with open(tar_path, "wb") as f:
            for chunk in resp.iter_content(chunk_size=1 << 20):
                f.write(chunk)
                downloaded += len(chunk)
                if total:
                    pct = downloaded * 100 // total
                    print(
                        f"\r  {pct}% ({downloaded >> 20} / {total >> 20} MB)",
                        end="",
                        file=sys.stderr,
                    )
        print(file=sys.stderr)

    print("Extracting...", file=sys.stderr)
    with tarfile.open(tar_path, "r:gz") as tar:
        tar.extractall(ENRON_CACHE_DIR, filter="data")

    # The Enron tarball extracts to maildir/ with user subdirectories
    # Find the actual root
    for candidate in [
        ENRON_CACHE_DIR / "enron_mail_20150507" / "maildir",
        ENRON_CACHE_DIR / "maildir",
        ENRON_EXTRACT_DIR,
    ]:
        if candidate.exists():
            # Rename to our canonical path if needed
            if candidate != ENRON_EXTRACT_DIR:
                candidate.rename(ENRON_EXTRACT_DIR)
            break

    print(f"Enron corpus ready at {ENRON_EXTRACT_DIR}", file=sys.stderr)
    return ENRON_EXTRACT_DIR


def _generate_weekdays(year: int) -> list[date]:
    """Return all Mon–Fri dates in *year* in calendar order."""
    start = date(year, 1, 1)
    end = date(year, 12, 31)
    result: list[date] = []
    current = start
    while current <= end:
        if current.weekday() < 5:  # 0=Mon … 4=Fri
            result.append(current)
        current += timedelta(days=1)
    return result


def download_congressional_record(
    year: int = 2023,
    max_packages: int = 20,
    cache_dir: Path = ENRON_CACHE_DIR,
) -> Path:
    """
    Download Congressional Record daily packages from GovInfo.gov and extract
    their HTML content to *cache_dir/congressional_record/{year}/*.

    Each session day is available as a single ZIP at
    ``https://www.govinfo.gov/content/pkg/CREC-{YYYY-MM-DD}.zip``
    (~30 MB compressed, mostly PDF).  Only the ``.htm`` files (~470 KB/day)
    are extracted; PDFs are discarded.  Non-session weekdays (recesses,
    holidays) return HTTP 404 and are silently skipped.

    Args:
        year: Calendar year to download (default 2023).
        max_packages: Maximum number of session-day packages to download.
            Each is ~30 MB compressed; 20 packages ≈ 600 MB download budget.
        cache_dir: Root cache directory (shared with Enron cache).

    Returns:
        Path to the directory containing the extracted ``.txt`` files.
    """
    out_dir = cache_dir / CREC_CACHE_DIR_NAME / str(year)
    if out_dir.exists() and any(out_dir.rglob("*.txt")):
        print(
            f"Using cached Congressional Record ({year}) at {out_dir}",
            file=sys.stderr,
        )
        return out_dir
    out_dir.mkdir(parents=True, exist_ok=True)

    import requests

    weekdays = _generate_weekdays(year)
    downloaded = 0

    print(
        f"Downloading Congressional Record ({year}, up to {max_packages} session days)…",
        file=sys.stderr,
    )
    for session_date in weekdays:
        if downloaded >= max_packages:
            break
        date_str = session_date.strftime("%Y-%m-%d")
        pkg_id = f"CREC-{date_str}"
        url = f"{CREC_GOVINFO_BASE}/{pkg_id}.zip"
        try:
            resp = requests.get(url, timeout=90)
            if resp.status_code == 404:
                continue  # not a session day
            resp.raise_for_status()
        except Exception as exc:
            print(f"  Warning: {pkg_id}: {exc}", file=sys.stderr)
            continue

        try:
            with _zipfile.ZipFile(io.BytesIO(resp.content)) as zf:
                saved = 0
                for name in zf.namelist():
                    if not name.endswith(".htm"):
                        continue  # skip large PDFs and XML metadata
                    raw = zf.read(name)
                    text = strip_html(raw.decode("utf-8", errors="replace"))
                    if not text or len(text.strip()) < 100:
                        continue
                    # Flatten ZIP member path to a safe filename
                    out_name = name.replace("/", "_").replace(".htm", ".txt")
                    (out_dir / out_name).write_text(text, encoding="utf-8")
                    saved += 1
        except _zipfile.BadZipFile:
            print(f"  Warning: {pkg_id}: corrupt ZIP, skipping", file=sys.stderr)
            continue

        downloaded += 1
        print(
            f"  {pkg_id}: {saved} sections extracted ({downloaded}/{max_packages})",
            file=sys.stderr,
        )
        time.sleep(_GOVINFO_DELAY_S)

    print(
        f"Congressional Record: {downloaded} session days → {out_dir}",
        file=sys.stderr,
    )
    return out_dir


def _gao_package_ids_from_sitemap(year: int) -> list[str]:
    """
    Return GovInfo package IDs for GAO reports indexed in *year*'s sitemap.

    Sitemaps exist for report years 1989–2008.  Later years return HTTP 404
    and produce an empty list.
    """
    import requests

    url = f"{GAO_SITEMAP_BASE}/GAOREPORTS_{year}_sitemap.xml"
    try:
        resp = requests.get(url, timeout=30)
        if resp.status_code != 200:
            return []
        # Each <loc> looks like:
        #   https://www.govinfo.gov/app/details/GAOREPORTS-GAO-08-613T
        return re.findall(r"/details/(GAOREPORTS-[^<\s]+)", resp.text)
    except Exception as exc:
        print(f"  Warning: GAO sitemap {year}: {exc}", file=sys.stderr)
        return []


def download_gao_reports(
    years: Optional[list[int]] = None,
    max_reports: int = 500,
    cache_dir: Path = ENRON_CACHE_DIR,
) -> Path:
    """
    Download GAO report HTML from GovInfo.gov and cache to
    *cache_dir/gao_reports/*.

    Each report is fetched as a single HTML file (~66 KB) directly from
    ``https://www.govinfo.gov/content/pkg/{pkg_id}/html/{pkg_id}.htm``
    — no ZIP download needed.  Package IDs are discovered via GovInfo
    XML sitemaps (available for report years 1989–2008).

    Args:
        years: Report years to pull sitemaps from.
            Default: [2004, 2005, 2006, 2007, 2008].
        max_reports: Maximum number of reports to download (default 500,
            ≈ 33 MB total).
        cache_dir: Root cache directory.

    Returns:
        Path to the directory containing the downloaded ``.txt`` files.
    """
    if years is None:
        years = [2004, 2005, 2006, 2007, 2008]
    out_dir = cache_dir / GAO_CACHE_DIR_NAME

    existing = list(out_dir.glob("*.txt")) if out_dir.exists() else []
    if len(existing) >= max_reports:
        print(
            f"Using cached GAO reports ({len(existing)} reports) at {out_dir}",
            file=sys.stderr,
        )
        return out_dir
    out_dir.mkdir(parents=True, exist_ok=True)

    import requests

    # Collect package IDs from sitemaps across requested years
    package_ids: list[str] = []
    for year in years:
        ids = _gao_package_ids_from_sitemap(year)
        package_ids.extend(ids)
        print(f"  GAO sitemap {year}: {len(ids)} reports found", file=sys.stderr)

    already_have = {p.stem for p in existing}
    to_fetch = [p for p in package_ids if p not in already_have]
    to_fetch = to_fetch[: max(0, max_reports - len(existing))]

    print(
        f"Downloading GAO reports ({len(to_fetch)} to fetch,"
        f" {len(existing)} already cached)…",
        file=sys.stderr,
    )
    downloaded = len(existing)
    for pkg_id in to_fetch:
        url = f"{CREC_GOVINFO_BASE}/{pkg_id}/html/{pkg_id}.htm"
        try:
            resp = requests.get(url, timeout=30)
            if resp.status_code != 200:
                continue
        except Exception as exc:
            print(f"  Warning: {pkg_id}: {exc}", file=sys.stderr)
            continue

        text = strip_html(resp.text)
        if text and len(text.strip()) > 100:
            (out_dir / f"{pkg_id}.txt").write_text(text, encoding="utf-8")
            downloaded += 1
            if downloaded % 100 == 0:
                print(f"  GAO: {downloaded} reports downloaded", file=sys.stderr)
        time.sleep(_GOVINFO_DELAY_S)

    print(f"GAO reports: {downloaded} total → {out_dir}", file=sys.stderr)
    return out_dir


def _ia_crest_identifiers(max_results: int = 600) -> list[str]:
    """
    Return Internet Archive identifiers for CIA CREST documents.

    Searches the IA Advanced Search API for ``identifier:CIA-RDP*`` items with
    ``mediatype:texts``.  The CIA-RDP prefix is the canonical CREST identifier
    pattern — every document in the 25-year program archive uses it.

    Returns at most *max_results* identifiers; the API caps a single page at
    10 000 rows, so we request ``min(max_results, 500)`` per call (practical
    limit for fast response and politeness).
    """
    import requests

    params = {
        "q": "identifier:CIA-RDP* AND mediatype:texts",
        "fl[]": "identifier",
        "output": "json",
        "rows": min(max_results, 500),
        "page": 1,
        "sort[]": "downloads desc",  # prefer well-accessed documents first
    }
    try:
        resp = requests.get(CREST_IA_SEARCH_URL, params=params, timeout=30)
        resp.raise_for_status()
        docs = resp.json().get("response", {}).get("docs", [])
        return [d["identifier"] for d in docs if "identifier" in d]
    except Exception as exc:
        print(f"  Warning: IA CREST search failed: {exc}", file=sys.stderr)
        return []


def download_crest_corpus(
    max_documents: int = 200,
    cache_dir: Path = ENRON_CACHE_DIR,
) -> Path:
    """
    Download CIA CREST documents from Internet Archive and cache to
    *cache_dir/crest_documents/*.

    Each CREST document is fetched as a plain-text OCR file.  The IA stores
    DjVu text layers (full-page OCR) at ``{id}_djvu.txt`` and sometimes a
    separate ``.txt`` file.  Both are tried in order; documents where neither
    is available or the text is too short (< 200 characters) are skipped.

    The CIA CREST documents are declassified records that retain their
    original classification banners and portion marks, making them ideal
    source material for ``--mode mangled`` fixture generation.

    Args:
        max_documents: Maximum number of documents to cache (default 200).
            200 documents ≈ 5–10 MB of text, sufficient to exceed the
            SC-004 gate of 200 mangled cases.
        cache_dir: Root cache directory.

    Returns:
        Path to the directory containing the downloaded ``.txt`` files.
    """
    out_dir = cache_dir / CREST_CACHE_DIR_NAME
    existing = list(out_dir.glob("*.txt")) if out_dir.exists() else []
    if len(existing) >= max_documents:
        print(
            f"Using cached CREST corpus ({len(existing)} docs) at {out_dir}",
            file=sys.stderr,
        )
        return out_dir
    out_dir.mkdir(parents=True, exist_ok=True)

    import requests

    # Fetch more candidates than needed — many IA items won't have text files.
    identifiers = _ia_crest_identifiers(max_results=max_documents * 4)
    already_have = {p.stem for p in existing}
    to_fetch = [i for i in identifiers if i not in already_have]

    print(
        f"Downloading CIA CREST documents from Internet Archive"
        f" ({len(to_fetch)} candidates for {max_documents} target)…",
        file=sys.stderr,
    )
    downloaded = len(existing)

    for identifier in to_fetch:
        if downloaded >= max_documents:
            break

        fetched = False
        for suffix in ("_djvu.txt", ".txt"):
            url = f"{CREST_IA_DOWNLOAD_BASE}/{identifier}/{identifier}{suffix}"
            try:
                resp = requests.get(url, timeout=30)
                if resp.status_code != 200:
                    continue
                text = resp.text.strip()
                if len(text) < 200:
                    continue  # OCR failure or empty document
                dest = out_dir / f"{identifier}.txt"
                dest.write_text(text, encoding="utf-8", errors="replace")
                downloaded += 1
                if downloaded % 50 == 0:
                    print(
                        f"  CREST: {downloaded}/{max_documents} documents downloaded",
                        file=sys.stderr,
                    )
                fetched = True
                break
            except Exception as exc:
                print(f"  Warning: {identifier}{suffix}: {exc}", file=sys.stderr)

        if fetched:
            time.sleep(_IA_DELAY_S)

    print(f"CREST corpus: {downloaded} documents → {out_dir}", file=sys.stderr)
    return out_dir


def iter_corpus_texts(
    corpus_path: Path,
    max_docs: Optional[int] = None,
    min_length: int = 20,
):
    """
    Yield (doc_id, text) tuples from a corpus directory.

    Handles both raw text files and RFC 2822 email files (Enron format).

    *min_length* is a stripped-text minimum below which documents are
    dropped. Defaults to 20 (filters near-empty Enron mail bodies and
    HTML wrappers); the marking-stratum caller of `run_analysis`
    overrides to 1 because canonical CAPCO portions and short banners
    legitimately fit in fewer than 20 bytes (`(S)`, `SECRET//NF`,
    `S//NF` are all valid marking fixtures and must contribute to
    `token_base_rates`).

    *max_docs* is treated with `is not None` semantics: a literal `0`
    means "yield zero documents" (used by the priors-mode budget
    arithmetic when the marking stratum has already consumed the
    full ``--max-docs`` budget). A truthiness check (`if max_docs`)
    would treat 0 as falsy and yield the entire corpus uncapped.
    """
    if max_docs is not None and max_docs <= 0:
        return
    count = 0
    for root, _dirs, files in os.walk(corpus_path):
        for fname in files:
            if fname.startswith("."):
                continue
            fpath = Path(root) / fname
            try:
                raw = fpath.read_bytes()
                text = extract_body(raw)
                if text and len(text.strip()) >= min_length:
                    doc_id = str(fpath.relative_to(corpus_path))
                    yield doc_id, text
                    count += 1
                    if max_docs is not None and count >= max_docs:
                        return
            except (UnicodeDecodeError, OSError):
                continue

    if count == 0:
        print(f"Warning: no documents found in {corpus_path}", file=sys.stderr)


def iter_corpus_texts_multi(
    corpus_paths: list[Path],
    max_docs: Optional[int] = None,
    min_length: int = 20,
) -> Generator:
    """
    Yield ``(doc_id, text)`` pairs from multiple corpus directories in order.

    Applies *max_docs* as a global cap across all sources combined, not
    per-source, so a small *max_docs* will be drawn entirely from the first
    source.  Pass ``None`` (default) to iterate all documents in all sources.

    *max_docs* uses `is not None` semantics: a literal `0` short-
    circuits with no docs yielded. A truthiness check would
    silently treat 0 as "unlimited" and scan the whole corpus.

    *min_length* is forwarded to ``iter_corpus_texts``; see that
    function's docstring for the marking-vs-prose stratum tradeoff.
    """
    if max_docs is not None and max_docs <= 0:
        return
    count = 0
    for corpus_path in corpus_paths:
        for doc_id, text in iter_corpus_texts(corpus_path, min_length=min_length):
            yield doc_id, text
            count += 1
            if max_docs is not None and count >= max_docs:
                return


def extract_body(raw: bytes) -> Optional[str]:
    """
    Extract the text body from raw file bytes.

    Handles three formats:
    - GovInfo HTML (CREC, GAO): text wrapped in ``<pre>`` tags — stripped via
      ``strip_html()``.
    - RFC 2822 email (Enron maildir format): text/plain body extracted.
    - Plain text: returned as-is.
    """
    try:
        text = raw.decode("utf-8", errors="replace")
    except Exception:
        return None

    # HTML detection: GovInfo pages start with <html or <!DOCTYPE
    raw_prefix = raw[:200].lower()
    if b"<html" in raw_prefix or b"<!doctype" in raw_prefix:
        stripped = strip_html(text)
        return stripped if stripped and len(stripped.strip()) > 20 else None

    # Quick heuristic: if it has email-like headers, parse as email
    if text[:200].count(":") >= 2 and any(
        h in text[:500] for h in ("From:", "Subject:", "Date:", "Message-ID:")
    ):
        with contextlib.suppress(Exception):
            msg = email.message_from_string(text, policy=email.policy.default)
            if (body := msg and msg.get_body(preferencelist=("plain",))) and (content := body.get_content()) and isinstance(content, str):
                return content
        # Fallback: strip headers manually
        header_end = text.find("\n\n")
        if header_end > 0:
            return text[header_end + 2 :]

    return text


# ---------------------------------------------------------------------------
# Token vocabulary loading
# ---------------------------------------------------------------------------


def load_tokens(token_path: Path) -> dict:
    """Load a token vocabulary JSON file. Returns {category: [tokens]}."""
    with open(token_path) as f:
        data = json.load(f)

    tokens_by_category = {}
    for cat_name, cat_data in data["categories"].items():
        tokens_by_category[cat_name] = cat_data["tokens"]

    return tokens_by_category


def all_tokens_flat(tokens_by_category: dict) -> list[str]:
    """Flatten all tokens into a single deduplicated list."""
    seen = set()
    result = []
    for tokens in tokens_by_category.values():
        for t in tokens:
            if t not in seen:
                seen.add(t)
                result.append(t)
    return result


# ---------------------------------------------------------------------------
# Analysis engine
# ---------------------------------------------------------------------------

# Contextual windows (characters)
PROXIMITY_WINDOW = 30  # how close tokens must be to count as "near" each other
DOUBLE_SLASH_WINDOW = 15  # how close to "//" to count as "near double slash"


def analyze_document(
    text: str,
    token_list: list[str],
    token_set: set[str],
) -> dict:
    """
    Analyze a single document for token frequencies and contexts.

    Returns a dict with per-token counts and contextual signals.
    """
    results = {
        "word_count": 0,
        "token_hits": Counter(),  # token → raw count
        "context_after_paren": Counter(),  # token appeared right after (
        "context_near_double_slash": Counter(),  # token within N chars of //
        "context_line_start_caps": Counter(),  # token at start of line, all caps context
        "context_inside_parens": Counter(),  # token inside (...)
    }

    words = text.split()
    results["word_count"] = len(words)

    # Find all token occurrences with positions
    # We need case-sensitive matching (CAPCO tokens are uppercase)
    # but also track case-insensitive matches separately
    token_positions = []  # (start, end, token, case_exact)

    for token in token_list:
        # Exact case match
        start = 0
        while True:
            idx = text.find(token, start)
            if idx == -1:
                break
            # Word boundary check: ensure we're matching whole tokens,
            # not substrings of longer words. For short tokens like "S", "C", "U"
            # we need this to avoid counting every 'S' in prose.
            if _is_word_boundary(text, idx, idx + len(token), token):
                token_positions.append((idx, idx + len(token), token, True))
            start = idx + 1

    # Precompute // positions for proximity checks
    dslash_positions = []
    start = 0
    while True:
        idx = text.find("//", start)
        if idx == -1:
            break
        # Exclude URLs: check if preceded by "http:" or "https:"
        prefix = text[max(0, idx - 6) : idx].lower()
        if not ("http:" in prefix or "https:" in prefix or "ftp:" in prefix):
            dslash_positions.append(idx)
        start = idx + 2

    # Precompute ( and ) positions for paren context
    open_parens = [i for i, ch in enumerate(text) if ch == "("]

    # Score each token occurrence in context
    for pos_start, pos_end, token, case_exact in token_positions:
        results["token_hits"][token] += 1

        # Context: after opening paren?
        # Check if there's a ( within 1-2 chars before the token
        for pp in open_parens:
            if 0 <= pos_start - pp <= 2:
                results["context_after_paren"][token] += 1
                break

        # Context: near //?
        for dp in dslash_positions:
            dist = min(abs(pos_start - dp), abs(pos_end - dp))
            if dist <= DOUBLE_SLASH_WINDOW:
                results["context_near_double_slash"][token] += 1
                break

        # Context: at line start in all-caps context?
        line_start = text.rfind("\n", 0, pos_start)
        line_start = line_start + 1 if line_start >= 0 else 0
        prefix_on_line = text[line_start:pos_start].strip()
        if len(prefix_on_line) <= 2:  # token is near the start of the line
            # Check if the rest of the line is mostly uppercase
            line_end = text.find("\n", pos_end)
            if line_end == -1:
                line_end = len(text)
            line_text = text[line_start:line_end]
            alpha_chars = [c for c in line_text if c.isalpha()]
            if (
                alpha_chars
                and sum(1 for c in alpha_chars if c.isupper()) / len(alpha_chars) > 0.7
            ):
                results["context_line_start_caps"][token] += 1

        # Context: inside parentheses?
        # Find the most recent ( before this position
        last_open = -1
        for pp in open_parens:
            if pp < pos_start:
                last_open = pp
            else:
                break
        if last_open >= 0:
            # Is there a ) after the token and before the next ( ?
            close = text.find(")", pos_end)
            next_open = text.find("(", pos_end)
            if close >= 0 and (next_open == -1 or close < next_open):
                results["context_inside_parens"][token] += 1

    return results


def _is_word_boundary(text: str, start: int, end: int, token: str) -> bool:
    """
    Check if the token at text[start:end] is at a word boundary.

    For multi-char tokens (SECRET, NOFORN, SI-G, etc.), we check that the
    character before and after are not alphanumeric.

    For single-char tokens (S, C, U, R, G), we require stricter boundaries:
    must be surrounded by non-alpha characters AND the token must be uppercase
    in context. This prevents counting every 'S' in English prose.
    """
    before_ok = start == 0 or not text[start - 1].isalnum()
    after_ok = end >= len(text) or not text[end].isalnum()

    if not (before_ok and after_ok):
        return False

    # Single-char tokens need extra strictness
    if len(token) == 1:
        # Must be surrounded by non-alpha (not just non-alnum)
        before_alpha = start > 0 and text[start - 1].isalpha()
        after_alpha = end < len(text) and text[end].isalpha()
        if before_alpha or after_alpha:
            return False
        # In a paren context like (S//...) it's fine
        # But standalone "S" at end of sentence is noise
        # Require a structural marker nearby: //, (, ), -
        nearby = text[max(0, start - 3) : min(len(text), end + 3)]
        if "//" in nearby or "(" in nearby or ")" in nearby:
            return True
        return False

    return True


def analyze_cooccurrence(
    text: str,
    token_list: list[str],
    window: int = PROXIMITY_WINDOW,
) -> Counter:
    """
    Count co-occurrences of vocabulary token pairs within a character window.

    Returns a Counter of (token_a, token_b) tuples (sorted so a < b).
    """
    cooccurrences = Counter()

    # Find all token positions (reuse logic but simplified)
    positions = []
    for token in token_list:
        start = 0
        while True:
            idx = text.find(token, start)
            if idx == -1:
                break
            if _is_word_boundary(text, idx, idx + len(token), token):
                positions.append((idx, token))
            start = idx + 1

    # Sort by position
    positions.sort()

    # Check all pairs within the window
    for i, (pos_a, tok_a) in enumerate(positions):
        for j in range(i + 1, len(positions)):
            pos_b, tok_b = positions[j]
            if pos_b - pos_a > window:
                break
            if tok_a != tok_b:
                pair = tuple(sorted([tok_a, tok_b]))
                cooccurrences[pair] += 1

    return cooccurrences


# ---------------------------------------------------------------------------
# REL TO trigraph extraction (issue #233)
# ---------------------------------------------------------------------------

# Match a standalone ``REL TO`` header followed by comma- or
# whitespace-separated entries until the block terminator. CAPCO §H.8
# says ``//`` is the authoritative end-of-category separator (project
# memory: `project_capco_separator_conventions`); the analyzer also
# stops at end-of-line or end-of-portion ``)`` so the regex doesn't run
# away on malformed inputs (post-processed in
# `_extract_rel_to_trigraphs`, not in the regex itself). The body group
# is greedy with a hard width cap of 200 chars to keep pathological
# inputs from blowing up the regex backtracker; the post-processing cut
# at ``//``/``\n``/``)`` narrows the captured text to the actual REL TO
# body. The leading word boundary prevents matching ``REL TO`` inside
# larger words such as ``SQUIRREL TO``.
_REL_TO_BLOCK_RE = re.compile(
    r"\bREL\s+TO\s+([A-Z][A-Z0-9_,\s]{0,200})",
    re.IGNORECASE,
)
# Country codes inside a REL TO body land in this regex's matched set
# at lengths 2-4 ASCII uppercase / digit / underscore — covering 2-char
# (``EU``), 3-char trigraphs, and 4-char tetragraphs (``FVEY``, ``ACGU``,
# ``NATO``). Longer ``CountryCode`` forms (e.g. ``AUSTRALIA_GROUP`` at
# 15 chars) exist in the schema but are deliberately excluded here:
# admitting 5+ char tokens absorbs prose words (``"USA, ALSO,"`` would
# lift ``ALSO`` into the prior) and the longer codes appear too rarely
# in real REL TO blocks to justify the false-positive risk. Adjust the
# upper bound (and re-validate the prose-absorption risk) if a future
# corpus shows meaningful frequency for the long codes.
_REL_TO_TRIGRAPH_TOKEN_RE = re.compile(
    r"\b[A-Z][A-Z0-9_]{1,3}\b",
    re.IGNORECASE,
)


def _extract_rel_to_trigraphs(text: str):
    """
    Yield trigraph tokens found in ``REL TO`` blocks within ``text``.

    Conservative: only emits tokens that match the CAPCO-permissible
    country-code byte set (uppercase ASCII / digit / underscore, length
    2-4) and that appear in the body of a ``REL TO …`` header. Stops at
    ``//`` because CAPCO §H.8 marks that as the category terminator;
    stops at end-of-line / ``)`` to avoid bleeding into prose. Yields
    every occurrence (does not deduplicate per document) so the
    Counter receives true frequency.
    """
    for header_match in _REL_TO_BLOCK_RE.finditer(text):
        body = header_match.group(1)
        # Cut at the first block terminator. Keeping the cuts in
        # priority order: ``//`` (category boundary), then newline, then
        # ``)`` (portion-form close). Whichever appears earliest wins.
        cut = len(body)
        for term in ("//", "\n", ")"):
            idx = body.find(term)
            if 0 <= idx < cut:
                cut = idx
        body = body[:cut]
        for m in _REL_TO_TRIGRAPH_TOKEN_RE.finditer(body):
            yield m.group(0).upper()


# ---------------------------------------------------------------------------
# Main analysis loop
# ---------------------------------------------------------------------------


def run_analysis(
    corpus_paths: list[Path],
    tokens_by_category: dict,
    max_docs: Optional[int] = None,
    min_length: int = 20,
) -> dict:
    """Run the full analysis over one or more corpora. Returns the frequency table.

    *min_length* is the per-document stripped-text floor used by
    ``iter_corpus_texts``. Marking-stratum callers should pass
    ``min_length=1`` so canonical short fixtures
    (``(S)``, ``SECRET//NF``, single-portion lines) contribute to
    the marking-side counts; the prose stratum keeps the default
    ``20`` to filter near-empty Enron mail bodies and HTML
    wrappers."""
    token_list = all_tokens_flat(tokens_by_category)
    token_set = set(token_list)

    # Aggregate counters
    total_docs = 0
    total_words = 0
    docs_containing = Counter()  # token → number of docs containing it
    total_hits = Counter()  # token → total occurrences
    total_after_paren = Counter()
    total_near_dslash = Counter()
    total_line_start_caps = Counter()
    total_inside_parens = Counter()
    total_cooccurrence = Counter()
    # Per-country-code occurrence counts within REL TO blocks. Issue
    # #233: baked into ``country_code_base_rates`` so the Phase-D
    # decoder can disambiguate fuzzy candidates (e.g., USA vs UZB,
    # AUS vs ASM) by corpus frequency rather than edit distance alone.
    # The counter key space is whatever ``_REL_TO_TRIGRAPH_TOKEN_RE``
    # collects: 2-char codes (``EU``), 3-char trigraphs, and 4-char
    # tetragraphs (``FVEY``, ``ACGU``, ``NATO``). Longer ``CountryCode``
    # forms (``AUSTRALIA_GROUP`` etc.) are deliberately excluded — see
    # the regex comment for the prose-absorption rationale.
    rel_to_trigraph_hits = Counter()
    # Per-country-code occurrence counts at any position in the
    # document — independent of REL TO context. The prose stratum
    # needs these so ``country_code_prose_base_rates`` reflects the
    # actual frequency of country-code spellings in prose (proper-
    # noun "(USA)", "GBR", etc.), not just inside REL TO blocks
    # (which prose corpora effectively never contain).
    # ``rel_to_trigraph_hits`` above stays REL TO-scoped because the
    # marking-side use case is "how often does this code appear *as*
    # a REL TO entry" — that's the discriminator between USA-as-
    # country-code vs USA-as-prose-mention. The token counter for
    # capco.json vocabulary picks up canonical country-code codes
    # that ARE in the vocab (USA, GBR, AUS, etc.), but the
    # ``_REL_TO_COUNTRY_CODE_BASELINE`` baseline includes codes
    # outside the vocab (EU, BEL, POL, TUR, AUT, UZB, ASM, …)
    # which would otherwise get a hard-coded zero in the prose
    # table — flagged in PR #312 review.
    country_code_global_hits = Counter()
    _country_code_re = re.compile(
        r"\b(?:" + "|".join(re.escape(c) for c in _REL_TO_COUNTRY_CODE_BASELINE) + r")\b"
    )
    # ``//`` (CAPCO category-separator) statistics. These feed the
    # template_base_rates table downstream — `total_dslash` (non-URL
    # occurrences) is the count attributed to the
    # ``classification//dissem`` template shape. The counters were
    # previously referenced without initialization, which would crash
    # on the first document containing ``//``; initialize explicitly.
    docs_with_dslash = 0
    total_dslash = 0
    dslash_in_url = 0
    dslash_not_url = 0

    print("Analyzing corpus...", file=sys.stderr)

    for doc_id, text in iter_corpus_texts_multi(corpus_paths, max_docs, min_length=min_length):
        total_docs += 1

        if total_docs % 10000 == 0:
            print(f"  {total_docs} documents processed...", file=sys.stderr)

        # Per-document analysis
        doc_result = analyze_document(text, token_list, token_set)
        total_words += doc_result["word_count"]

        for token, count in doc_result["token_hits"].items():
            total_hits[token] += count
            docs_containing[token] += 1

        for token, count in doc_result["context_after_paren"].items():
            total_after_paren[token] += count
        for token, count in doc_result["context_near_double_slash"].items():
            total_near_dslash[token] += count
        for token, count in doc_result["context_line_start_caps"].items():
            total_line_start_caps[token] += count
        for token, count in doc_result["context_inside_parens"].items():
            total_inside_parens[token] += count

        # Co-occurrence (skip for very large docs to keep runtime sane)
        if len(text) < 50000:
            cooc = analyze_cooccurrence(text, token_list)
            total_cooccurrence += cooc

        # // statistics
        dslash_count = text.count("//")
        if dslash_count > 0:
            docs_with_dslash += 1
            total_dslash += dslash_count
            # Count URL vs non-URL //
            for m in re.finditer(r"//", text):
                prefix = text[max(0, m.start() - 6) : m.start()].lower()
                if "http:" in prefix or "https:" in prefix or "ftp:" in prefix:
                    dslash_in_url += 1
                else:
                    dslash_not_url += 1

        # REL TO country-code counts (issue #233). Walks each REL TO
        # block and tallies the comma-separated tokens. The decoder's
        # `country_code_base_rates` table consumes these counts so
        # common codes (USA, GBR, AUS, FVEY, …) outweigh rare ones
        # (UZB, ASM, AUT) when fuzzy candidates collide on edit
        # distance alone.
        for trigraph in _extract_rel_to_trigraphs(text):
            rel_to_trigraph_hits[trigraph] += 1

        # Global country-code scan — counts every BASELINE country
        # code at any word-boundary position in the document. Used
        # by the prose stratum to populate
        # `country_code_prose_base_rates` for codes outside the
        # canonical-token vocabulary (issue #258 review fix).
        for m in _country_code_re.finditer(text):
            country_code_global_hits[m.group(0)] += 1

    print(f"Done. {total_docs} documents, {total_words} words.", file=sys.stderr)

    # Build output
    per_million = total_words / 1_000_000 if total_words > 0 else 1

    token_results = {}
    for token in token_list:
        hits = total_hits.get(token, 0)
        doc_count = docs_containing.get(token, 0)
        token_results[token] = {
            "raw_count": hits,
            "per_million_words": round(hits / per_million, 4) if hits > 0 else 0,
            "doc_frequency": round(doc_count / total_docs, 6) if total_docs > 0 else 0,
            "docs_containing": doc_count,
            "contexts": {
                "after_paren": total_after_paren.get(token, 0),
                "near_double_slash": total_near_dslash.get(token, 0),
                "line_start_caps": total_line_start_caps.get(token, 0),
                "inside_parens": total_inside_parens.get(token, 0),
            },
        }

    # Top co-occurrences (pairs that appear together)
    top_cooccurrences = {}
    for (tok_a, tok_b), count in total_cooccurrence.most_common(100):
        top_cooccurrences[f"{tok_a}+{tok_b}"] = count

    # Category-level token lookup for the output
    token_to_category = {}
    for cat, tokens in tokens_by_category.items():
        for t in tokens:
            token_to_category[t] = cat

    output = {
        "corpus_stats": {
            "document_count": total_docs,
            "total_words": total_words,
            "double_slash": {
                "docs_containing": docs_with_dslash,
                "total_occurrences": total_dslash,
                "in_urls": dslash_in_url,
                "not_in_urls": dslash_not_url,
                "doc_frequency": round(docs_with_dslash / total_docs, 6)
                if total_docs > 0
                else 0,
            },
        },
        "tokens": token_results,
        "token_categories": token_to_category,
        "cooccurrence_pairs": top_cooccurrences,
        # Issue #233: per-trigraph hit counts inside REL TO blocks.
        # Sorted by token name for stable output across runs (the priors
        # baker re-sorts anyway, but a deterministic intermediate keeps
        # the analyzer's JSON diff-clean under VCS).
        "rel_to_trigraph_hits": dict(sorted(rel_to_trigraph_hits.items())),
        # Issue #258: per-country-code hit counts at any document
        # position (not REL TO-scoped). Consumed by `derive_priors`
        # for prose-stratum `country_code_prose_base_rates` so codes
        # outside the canonical-token vocabulary still get real
        # prose-side counts.
        "country_code_global_hits": dict(sorted(country_code_global_hits.items())),
    }

    return output


# ---------------------------------------------------------------------------
# Phase D: corpus-derived priors output
# ---------------------------------------------------------------------------

PRIORS_SCHEMA_VERSION = "marque-priors-3"

# REL TO trigraph baseline counts, in REL TO blocks per million such
# blocks. The Phase-D decoder uses these to break fuzzy ties between
# high-frequency trigraphs (USA, GBR, AUS, …) and low-frequency
# lookalikes (ASM, UZB, AUT-as-Austria) — see issue #233 (extracted
# from #186 sub-feature 1).
#
# **Why a baseline rather than corpus-only counts.** The Enron corpus
# is the engine's negative corpus (how often SECRET / NOFORN / REL TO
# show up in unclassified English prose) and contains effectively
# zero genuine REL TO blocks. Counting REL TO occurrences in Enron
# would give every trigraph a 0 hit count, collapse the Laplace
# smoothing to a single shared log-prior, and defeat the entire
# point of a frequency table. Authoritative IC publication
# frequency data is not openly redistributable in machine-readable
# form, so this baseline encodes the order-of-magnitude ratios the
# issue calls out (USA » FVEY partners » other NATO » rare ISO
# trigraphs). The exact counts are heuristic; what matters is the
# log-prior delta between popular and rare entries, which the decoder
# uses as a tiebreaker against edit-distance differences.
#
# When a corpus is observed to contain real REL TO blocks (a future
# corpus swap, a controlled-distribution dataset), the observed
# counts add on top of these baselines via ``Counter`` addition in
# ``derive_priors`` so genuine evidence ratchets the priors up
# without losing the fuzzy-disambiguation guarantees.
#
# Citation: CAPCO-2016 §H.8 (REL TO syntax and FVEY-priority listing)
# governs which trigraphs the rule itself blesses. The numeric ratios
# below are not citations from that document — they are the priors
# baker's statement of "USA appears at orders-of-magnitude higher
# rate than UZB in real markings", which CAPCO does not codify.
_REL_TO_COUNTRY_CODE_BASELINE = {
    # FVEY core: by far the most-frequent REL TO entries in real IC
    # markings (CAPCO §H.8 mandates US-first ordering).
    "USA": 10000,
    "GBR": 4000,
    "CAN": 4000,
    "AUS": 4000,
    "NZL": 3500,
    # FVEY collective tetragraph (CAPCO §H.8 p169-170). The
    # tetragraph form is rarer than spelling out each member, but
    # high enough above the noise floor that it must score above
    # arbitrary-token edit distance.
    "FVEY": 1500,
    "ACGU": 800,
    # NATO and the EU group — common in multinational programs.
    "NATO": 1200,
    "EU": 800,
    # Tier-1 NATO partners frequently named in REL TO blocks.
    "DEU": 600,
    "FRA": 600,
    "NLD": 500,
    "NOR": 500,
    "DNK": 400,
    "ITA": 400,
    "ESP": 300,
    "BEL": 300,
    "POL": 300,
    "TUR": 250,
    # Indo-Pacific and Middle East partners.
    "JPN": 700,
    "KOR": 600,
    "ISR": 500,
    # Lookalikes that ARE legitimate ISO trigraphs and would otherwise
    # not appear in the table at all. Without these the decoder treats
    # them as ``MISSING_TOKEN_LOG_PRIOR`` (= -12.0), which is more
    # punitive than the Laplace-smoothed log-prior of a rare entry.
    # Putting them in at low counts gives the decoder a finite,
    # well-below-FVEY log-prior so fuzzy USB→USA wins on prior delta
    # while genuine AUT (Austria) in a hand-typed marking still scores
    # as "rare-but-known" rather than "unknown token".
    "AUT": 50,  # Austria — fuzzy lookalike of AUS
    "UZB": 5,   # Uzbekistan — fuzzy lookalike of USA
    "ASM": 1,   # American Samoa — fuzzy lookalike of AUS
}


def _corpus_fingerprint(corpus_path: Path) -> str:
    """
    Deterministic, content-ignorant fingerprint of a corpus directory.

    Hashes only file metadata (relative path, size, mtime) — never file
    contents — so the fingerprint shifts when the corpus shifts without
    ever absorbing document bytes into the priors JSON, preserving the
    content-ignorance invariant enforced at audit time (Constitution V)
    at the priors-pipeline level too.

    SHA-512 is used rather than something faster (BLAKE3, etc.):
    fingerprinting a corpus is a one-time build step, not a runtime
    path, and SHA-512 is available without an optional dependency.
    """
    h = hashlib.sha512()
    for root, dirs, files in os.walk(corpus_path):
        dirs.sort()
        for fname in sorted(files):
            if fname.startswith("."):
                continue
            fpath = Path(root) / fname
            try:
                rel = fpath.relative_to(corpus_path)
                rel_key = rel.as_posix()
                stat = fpath.stat()
                h.update(f"{rel_key}\0{stat.st_size}\0{int(stat.st_mtime)}\n".encode())
            except (OSError, ValueError):
                continue
    return f"sha512:{h.hexdigest()}"


# Phase-D template identifiers are defined inline where priors are
# emitted (``derive_priors`` below) so there is no separate key list
# that can drift from the actual ``template_base_rates`` payload
# consumed downstream.


def _laplace_log_prior_table(token_counts: dict[str, int]) -> dict[str, dict]:
    """
    Build a ``{token: {count, log_prior}}`` table from raw counts using
    Laplace smoothing so zero-count tokens map to a finite log-prior.

    ``log_prior = log((hits + 1) / (total + |V|))``

    The smoothing constant matches what the Rust decoder assumes at
    scoring time; changing it here requires changing it there in
    lockstep.
    """
    total_hits = sum(token_counts.values())
    vocab_size = max(1, len(token_counts))
    denom = float(total_hits + vocab_size)

    table: dict[str, dict] = {}
    for token, hits in token_counts.items():
        log_prior = math.log(float(int(hits) + 1) / denom)
        table[token] = {
            "count": int(hits),
            "log_prior": round(log_prior, 6),
        }
    return table


def derive_priors(
    marking_analysis: dict,
    prose_analysis: dict,
    tokens_by_category: dict,
) -> dict:
    """
    Reshape stratified analysis results into the priors.json schema.

    Issue #258 split the corpus into two strata: marking-bearing
    material (``tests/corpus/valid/`` plus future mock-classified
    fixtures) and prose-only material (Enron, CIA CREST declassified,
    Congressional Record, GAO reports — all confirmed prose-dominant
    with effectively zero portion-marking hits per project owner). The
    decoder needs both halves to compute the per-token "marking-y"
    score ``log P(token|marking) − log P(token|prose)`` that lets a
    null hypothesis ("this isn't a marking, it's prose") compete
    against CAPCO interpretations during recognition.

    ``token_base_rates`` is derived from the marking stratum only —
    previously this was a mixture distribution because the analyzer
    aggregated all sources into one global counter, which silently
    treated the marking-side prior as if it were P(token|marking)
    when it was closer to P(token). With the split, the marking-side
    prior is now a clean P(token|marking).

    ``token_prose_base_rates`` is derived from the prose stratum only,
    giving the decoder its first explicit P(token|prose) signal.

    ``country_code_base_rates`` continues to mix the marking-stratum
    REL TO trigraph hits with ``_REL_TO_COUNTRY_CODE_BASELINE`` so
    FVEY partners always score above the noise floor regardless of
    corpus coverage. ``country_code_prose_base_rates`` is derived
    from the prose stratum only with no baseline mixin — country
    codes that show up in prose (proper-noun mentions of "USA",
    "GBR", etc.) are exactly what we need to push back against
    over-confident REL TO recovery.

    ``template_base_rates`` is derived from the marking stratum only
    (templates by definition only appear in marking-bearing text).

    ``strict_context_priors`` are heuristic defaults independent of
    the stratum split.
    """
    marking_tokens = marking_analysis["tokens"]
    prose_tokens = prose_analysis["tokens"]

    # Token base rates — marking stratum (clean P(token|marking))
    token_base_rates = _laplace_log_prior_table(
        {token: int(data.get("raw_count", 0)) for token, data in marking_tokens.items()}
    )

    # Token prose base rates — prose stratum (P(token|prose))
    token_prose_base_rates = _laplace_log_prior_table(
        {token: int(data.get("raw_count", 0)) for token, data in prose_tokens.items()}
    )

    # Template base rates: approximate by counting co-occurrence
    # patterns from the marking stratum. The exact template detection
    # is CAPCO-specific and refined by the Rust decoder at scoring
    # time; priors here give the base rates a generic-enough shape
    # for build.rs to consume.
    total_dslash = marking_analysis["corpus_stats"]["double_slash"]["not_in_urls"]
    cooccurrences = marking_analysis.get("cooccurrence_pairs", {})
    total_cooc = sum(cooccurrences.values()) or 1

    template_base_rates = {
        "classification": {
            "count": sum(
                marking_tokens.get(t, {}).get("raw_count", 0)
                for t in tokens_by_category.get("classification_abbrev", [])
                + tokens_by_category.get("classification_full", [])
            ),
        },
        "classification//dissem": {"count": total_dslash},
        "classification//sci-block//dissem": {"count": total_cooc // 3},
        "classification//dissem//rel-to": {"count": total_cooc // 4},
        "classification//sci-block": {"count": total_cooc // 5},
    }
    # Fill in log-priors for templates with the same smoothing
    total_tpl = sum(d["count"] for d in template_base_rates.values())
    tpl_denom = float(total_tpl + len(template_base_rates))
    for key, data in template_base_rates.items():
        data["log_prior"] = round(math.log(float(data["count"] + 1) / tpl_denom), 6)

    # Strict-context priors: floors for FR-011. These are heuristic
    # defaults pending corpus refinement — the decoder uses these as the
    # lower bound when a document shows strict-path evidence at a given
    # classification level. Adjust when corpus measurement gives a
    # better empirical estimate.
    strict_context_priors = {
        "confidential_floor": 0.97,
        "secret_floor": 0.99,
        "top_secret_floor": 0.995,
    }

    # Country-code base rates (issue #233). Counts come from REL TO
    # blocks observed in the marking stratum, summed with the baseline
    # ratios in ``_REL_TO_COUNTRY_CODE_BASELINE`` so the decoder always
    # has a finite log-prior for FVEY partners and known fuzzy
    # lookalikes. The emitted table covers all CAPCO country-code
    # shapes — 2-char codes (``EU``), 3-char trigraphs, 4-char
    # tetragraphs (``FVEY``, ``ACGU``, ``NATO``), and group codes —
    # even though the legacy baseline name still says "trigraph".
    # Smoothing follows the same Laplace formula as the token table so
    # the two are directly comparable inside the decoder's
    # ``score_candidate``: ``log_prior(USA) - log_prior(UZB)`` swamps a
    # single edit-distance-1 advantage when USA's hit count exceeds
    # UZB's by orders of magnitude.
    marking_country_hits = marking_analysis.get("rel_to_trigraph_hits", {}) or {}
    country_code_counts = Counter(_REL_TO_COUNTRY_CODE_BASELINE)
    country_code_counts.update(marking_country_hits)
    country_code_base_rates = _laplace_log_prior_table(dict(country_code_counts))

    # Country-code prose base rates (issue #258). Counts come from
    # the prose stratum's `country_code_global_hits` counter — a
    # word-boundary scan over every code in
    # `_REL_TO_COUNTRY_CODE_BASELINE`, regardless of whether the
    # code is also in the canonical-token vocabulary
    # (`tools/corpus-analysis/tokens/capco.json`). Codes like EU,
    # BEL, POL, TUR, AUT, UZB, ASM are *not* in the canonical
    # vocabulary, so the previous `prose_tokens`-based fix
    # (review-1) silently emitted them at count 0 even when the
    # corpus actually contained them — flagged in PR #312 review-2.
    #
    # Restricting to the BASELINE keys (rather than scanning all
    # alphabetic word boundaries) keeps the regex bounded and
    # focuses on codes the decoder actually consults at score time.
    # No `_REL_TO_COUNTRY_CODE_BASELINE` count mixin: the baseline
    # encodes marking-side frequency *ratios* that would corrupt
    # the prose-side signal. Codes the prose corpus never observed
    # get the Laplace-smoothed zero-count log-prior, which
    # correctly says "we never saw this in prose."
    prose_country_global = (
        prose_analysis.get("country_code_global_hits", {}) or {}
    )
    prose_country_counts: Counter[str] = Counter()
    for code in _REL_TO_COUNTRY_CODE_BASELINE:
        prose_country_counts[code] = int(prose_country_global.get(code, 0))
    country_code_prose_base_rates = _laplace_log_prior_table(
        dict(prose_country_counts)
    )

    return {
        "schema_version": PRIORS_SCHEMA_VERSION,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "token_base_rates": token_base_rates,
        "token_prose_base_rates": token_prose_base_rates,
        "template_base_rates": template_base_rates,
        "country_code_base_rates": country_code_base_rates,
        "country_code_prose_base_rates": country_code_prose_base_rates,
        "strict_context_priors": strict_context_priors,
    }


# ---------------------------------------------------------------------------
# Issue #133 PR 4: heuristic-trigger frequency analysis
# ---------------------------------------------------------------------------
#
# The position-aware short-token classification heuristic added in PR 2
# (`marque-engine::decoder::try_classification_heuristic_fix`) fires only
# when:
#   1. The decoder is invoked (deep-scan mode opt-in)
#   2. The strict parser fails on the input
#   3. The leading 1-2 char token is in classification position of a
#      portion or banner shape (i.e., the input is in marking-shape
#      context — there's a `//` or other marking signal nearby)
#
# So the relevant FP rate isn't "trigger appears in arbitrary prose"
# but "trigger appears as a standalone token in a context that ALSO
# contains marking-token signals (`//`, NOFORN, ORCON, SECRET, etc.)
# within proximity". The conditional measurement is what informs the
# `HEURISTIC_RULE_AXIS_CAP` confidence value in the engine.

HEURISTIC_FREQUENCY_SCHEMA_VERSION = "marque-heuristic-frequency-1"

# Trigger tokens for the position-aware classification heuristic.
# Keep in sync with `try_classification_heuristic_fix` in
# `crates/engine/src/decoder.rs` — a divergence between this list
# and the Rust helper means the analysis no longer measures what it
# claims to measure. The cross-language sync is convention only;
# there is no automated test that asserts the two lists match.
# Reviewers updating the Rust helper's trigger table MUST update
# this list at the same time.
HEURISTIC_TRIGGERS_1CHAR = ("A", "W", "E", "Z", "V", "F", "X")
HEURISTIC_TRIGGERS_2CHAR = tuple(
    f"{first}{second}" for first in "TRYHGF" for second in "AWEZS"
)
ALL_HEURISTIC_TRIGGERS = HEURISTIC_TRIGGERS_1CHAR + HEURISTIC_TRIGGERS_2CHAR

# Marking-shape signal tokens: when one of these appears within
# `MARKING_CONTEXT_WINDOW` chars of a trigger, we count that trigger
# occurrence as in-marking-context. The list combines the long-form
# IC dissem entries (PR 1's `EXTENDED_CORRECTION_VOCAB` additions),
# classification full-words, and a few SCI/REL TO starters.
MARKING_SHAPE_SIGNALS = (
    # Classifications (full forms only — short forms `S`, `C`, `U`,
    # `R`, `TS` would self-match against the trigger search).
    "SECRET",
    "TOP SECRET",
    "CONFIDENTIAL",
    "UNCLASSIFIED",
    "RESTRICTED",
    # Dissem long forms
    "NOFORN",
    "ORCON",
    "PROPIN",
    "IMCON",
    "RELIDO",
    "RSEN",
    "EYESONLY",
    "EXDIS",
    "NODIS",
    "LIMDIS",
    "FOUO",
    "FISA",
    # SCI compound starters
    "HCS-P",
    "HCS-O",
    "SI-G",
    "SI-EU",
    "SI-NK",
    # Phrases
    "REL TO",
)

# Distance (in chars) within which a marking signal must appear for
# a trigger to be counted as in-marking-context. 30 chars is roughly
# the length of one short marking (e.g., `(SECRET//NOFORN)` = 17
# chars), wide enough to catch the trigger when the rest of the
# marking is intact, narrow enough to exclude prose where signal and
# trigger are coincidentally in the same paragraph.
MARKING_CONTEXT_WINDOW = 30


def measure_heuristic_trigger_frequency(
    corpus_paths: list[Path],
    max_docs: Optional[int] = None,
) -> dict:
    """
    Walk `corpus_path` and count standalone uppercase trigger-token
    occurrences under two conditions:

    - **unrestricted**: every word-boundary occurrence of the trigger
    - **marking_context**: occurrences within `MARKING_CONTEXT_WINDOW`
      chars of a marking-shape signal (`//` outside URLs, or any
      [`MARKING_SHAPE_SIGNALS`] entry)

    The marking-context count is the conditional denominator the
    decoder heuristic actually faces. A trigger with low
    marking-context FP rate is safe to fire at high confidence; a
    trigger with high rate would risk noise even with `Severity::Warn`.

    Returns a dict with the schema documented in
    `tools/corpus-analysis/output/heuristic_frequencies.json`.
    """
    # Case-insensitive matching mirrors the decoder's runtime
    # behavior. `marque-engine::decoder::normalize_delimiters_and_case`
    # uppercases inputs that contain any lowercase character before
    # running the heuristic, so a real-world input of `(ys//noforn)`
    # becomes `(YS//NOFORN)` and the heuristic fires on the uppercase
    # `YS`. A case-sensitive analyzer (matching only uppercase
    # triggers) would undercount the empirical denominator —
    # specifically, lowercase appearances of trigger tokens (`re` in
    # "regarding", `we`, `at`) AND lowercase marking signals
    # (`secret`, `noforn`) would be invisible to the analyzer but
    # eligible for runtime heuristic firing post-uppercase. The
    # `re.IGNORECASE` flag closes that gap.
    trigger_pats = {
        t: re.compile(
            r"(?<![A-Za-z0-9])" + re.escape(t) + r"(?![A-Za-z0-9])",
            re.IGNORECASE,
        )
        for t in ALL_HEURISTIC_TRIGGERS
    }
    signal_pat = re.compile(
        r"(?<![A-Za-z0-9])("
        + "|".join(re.escape(s) for s in MARKING_SHAPE_SIGNALS)
        + r")(?![A-Za-z0-9])",
        re.IGNORECASE,
    )
    slash_pat = re.compile(r"//")

    unrestricted = Counter()
    marking_context = Counter()

    docs_processed = 0
    for _doc_id, text in iter_corpus_texts_multi(corpus_paths, max_docs=max_docs):
        # Find positions of marking signals (and `//` not in URLs).
        signal_positions: list[tuple[int, int]] = []
        for m in signal_pat.finditer(text):
            signal_positions.append((m.start(), m.end()))
        for m in slash_pat.finditer(text):
            prefix = text[max(0, m.start() - 6) : m.start()].lower()
            if "http" in prefix or "ftp:" in prefix:
                continue
            signal_positions.append((m.start(), m.end()))
        signal_positions.sort()

        for trigger, pat in trigger_pats.items():
            for m in pat.finditer(text):
                t_pos = m.start()
                unrestricted[trigger] += 1
                # Linear scan is fine — typical signal_positions count
                # per file is small and the early-exit on first match
                # keeps the worst case bounded.
                for s_start, s_end in signal_positions:
                    if (
                        abs(s_start - t_pos) <= MARKING_CONTEXT_WINDOW
                        or abs(s_end - t_pos) <= MARKING_CONTEXT_WINDOW
                    ):
                        marking_context[trigger] += 1
                        break

        docs_processed += 1

    # Build per-trigger records with both metrics + the rule the
    # trigger fires for.
    triggers_payload: dict[str, dict] = {}
    for t in ALL_HEURISTIC_TRIGGERS:
        if len(t) == 2:
            rule_target = "TS"
        elif t in "AWEZX":
            rule_target = "S"
        else:
            rule_target = "C"
        triggers_payload[t] = {
            "rule_target": rule_target,
            "length": len(t),
            "unrestricted_count": unrestricted[t],
            "marking_context_count": marking_context[t],
        }

    return {
        "schema_version": HEURISTIC_FREQUENCY_SCHEMA_VERSION,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "docs_processed": docs_processed,
        "marking_context_window_chars": MARKING_CONTEXT_WINDOW,
        "marking_shape_signals": list(MARKING_SHAPE_SIGNALS),
        "triggers": triggers_payload,
        # Aggregate summary for quick consumption.
        "summary": {
            "total_triggers": len(ALL_HEURISTIC_TRIGGERS),
            "triggers_with_zero_marking_context": sum(
                1 for t in ALL_HEURISTIC_TRIGGERS if marking_context[t] == 0
            ),
            "max_marking_context_count": (
                max(marking_context.values()) if marking_context else 0
            ),
        },
    }


# ---------------------------------------------------------------------------
# Phase D: mangled-marking fixture generation
# ---------------------------------------------------------------------------

MANGLING_CLASSES = (
    "typo",
    "reordering",
    "missing-delimiter",
    "superseded-token",
    "wrong-case",
    "garbled-delimiter",
)

# Deprecated → live token pairs that CAPCO-2016 records as actual
# supersession. These are NOT banner/portion-form abbreviation
# differences — `NF` (portion) and `NOFORN` (banner) are both
# authorized canonical forms in their respective contexts per the ODNI
# CVE register (`crates/ism/schemas/ISM-v2022-DEC/CVE_ISM/
# CVEnumISMDissem.csv`). Form-mismatch mangling would need its own
# class; this class is for tokens CAPCO explicitly retired.
#
# Every entry MUST cite a passage in
# `crates/capco/docs/CAPCO-2016.md` per Constitution VIII. An entry
# without a verified citation is a correctness defect (wrong-provenance
# fixtures), not a stylistic choice.
#
# Map direction: ``live_token → retired_synonym``. The mangling
# transform finds ``live_token`` inside a canonical marking extracted
# from the corpus and substitutes the retired form to produce the
# ``observed`` mangled string; the original canonical stays as the
# ``expected``. Consequence: classes generate fewer fixtures than
# other transforms if the corpus does not contain the live tokens,
# which is honest — genuine CAPCO supersessions are rare.
SUPERSEDED_TOKEN_MAP = {
    # CAPCO-2016 ~line 5136 (§A.6 note): "The COMINT title for the
    # Special Intelligence (SI) control system is no longer valid."
    # SI is the current title, COMINT is the retired one.
    "SI": "COMINT",
}

# Matches a canonical-looking marking: portion or banner with at least
# one `//` delimiter. Intentionally conservative — prefers false
# negatives (skip a genuine marking we didn't match) over false
# positives (treat random `FOO//BAR` text as a marking). High-confidence
# shapes only:
#   (CLASS//DISSEM)
#   CLASS//DISSEM
#   CLASS//SCI//DISSEM
# where CLASS is one of the known classification tokens.
# NOTE: GitHub's security scanning flags these regexes as ReDoS risks.
# They're safe in this context because the input corpus text is trusted,
# we match it to an SHA-512 fingerprint, and this script is **not** used in production
# It's a pre-compilation step that runs once on a known corpus, not a runtime path.
_MARKING_PORTION_RE = re.compile(
    r"\("
    r"("
    # Non-US/JOINT portion marks start with // inside the parens:
    #   (//NS//ATOMAL//OC)  (//CTS-B//NOFORN)  (//JOINT S CAN GBR//REL)
    r"(?://[A-Z][A-Z0-9 ,/-]+(?://[A-Z][A-Z0-9 ,/-]+)*)"
    r"|"
    # US portion marks: 1–3 capital letters then at least one //CATEGORY:
    #   (S//NF)  (TS//SI//NOFORN)
    r"(?:[A-Z]{1,3}(?://[A-Z][A-Z0-9 ,/-]+)+)"
    r")"
    r"\)"
)
_MARKING_BANNER_RE = re.compile(
    r"(?:^|(?<=\s))"
    r"("
    # Non-US/JOINT banners lead with // (no US classification prefix).
    # Three sub-families:
    #
    # 1. NATO full and abbreviated classification levels:
    #    //COSMIC TOP SECRET//BOHEMIA   //NATO SECRET//ATOMAL//ORCON
    #    //CTS//BOHEMIA  //NS//ATOMAL//OC  //NIS//REL TO USA, ISAF, NATO
    #
    # 2. JOINT classification (full and abbreviated levels + country list):
    #    //JOINT SECRET CAN GBR USA//REL TO USA, CAN, GBR
    #    //JOINT S GBR USA//REL TO USA, FVEY
    #
    # 3. FGI country/org code + level abbreviation:
    #    //AUS S//REL TO USA, AUS   //DEU C//REL TO USA, CAN, DEU
    #    //FGI S//NF                //FGI TS
    r"(?://"
    r"(?:"
    # NATO full classification levels (with optional SAP suffix)
    r"COSMIC TOP SECRET(?:-(?:BOHEMIA|BALK)|\s+ATOMAL)?"
    r"|NATO\s+(?:UNCLASSIFIED|RESTRICTED|CONFIDENTIAL(?:\s+ATOMAL)?|SECRET(?:\s+ATOMAL)?)"
    # NATO abbreviated classification tokens
    r"|CTSA?"           # CTS / CTSA (CTS+ATOMAL)
    r"|CTS-(?:B|BALK)"  # CTS-B (BOHEMIA), CTS-BALK
    r"|NS(?:AT)?"       # NS / NSAT (NATO SECRET / NATO SECRET ATOMAL)
    r"|NCA?"            # NC / NCA (NATO CONFIDENTIAL / +ATOMAL)
    r"|NR"              # NATO RESTRICTED
    r"|NU"              # NATO UNCLASSIFIED
    r"|NIS"             # NATO Intelligence Support
    # JOINT classification (full or abbreviated level, then optional country list)
    r"|JOINT\s+(?:TOP SECRET|SECRET|CONFIDENTIAL|UNCLASSIFIED|TS|S|C|U)(?:\s+[A-Z]{2,4})*"
    # FGI: one or more trigraph/tetragraph country codes + level abbreviation
    #   //AUS S  //CAN DEU S  //FGI TS  (FGI acts as a country-group tetragraph here)
    r"|(?:[A-Z]{2,4}\s+)+(?:TOP SECRET|SECRET|CONFIDENTIAL|TS|S|C)"
    r")"
    r"(?://[A-Z][A-Z0-9 ,/-]+)*"  # optional additional //CATEGORY blocks
    r")"
    r"|"
    # US banners: classification level first, then at least one //CATEGORY:
    #   SECRET//NOFORN  TOP SECRET//SI//REL TO USA, GBR
    r"(?:(?:UNCLASSIFIED|CONFIDENTIAL|SECRET|TOP SECRET|RESTRICTED)(?://[A-Z][A-Z0-9 ,/-]+)+)"
    r")"
    r"(?=\s|$)",
    re.MULTILINE,
)


def extract_candidate_markings(text: str) -> list[str]:
    """
    Return high-confidence marking strings observed in ``text``.

    Each returned string is an exact substring of the input. Only
    canonical-looking markings (classification + at least one `//`
    delimiter + at least one controlled token) are emitted; ambiguous
    shapes like bare ``(C)`` are deliberately skipped because they
    collide with copyright in non-IC text.
    """
    candidates = set()
    for m in _MARKING_PORTION_RE.finditer(text):
        candidates.add(m.group(1))
    for m in _MARKING_BANNER_RE.finditer(text):
        candidates.add(m.group(1))
    return sorted(candidates)


def _apply_typo(canonical: str, rng: random.Random) -> Optional[str]:
    """Single-char edit-distance-1 typo. Swap, drop, insert, or substitute."""
    if len(canonical) < 2:
        return None
    idx = rng.randrange(len(canonical))
    choice = rng.choice(("swap", "drop", "insert", "substitute"))
    if choice == "swap" and idx < len(canonical) - 1:
        return (
            canonical[:idx] + canonical[idx + 1] + canonical[idx] + canonical[idx + 2 :]
        )
    if choice == "drop":
        return canonical[:idx] + canonical[idx + 1 :]
    if choice == "insert":
        # Insert a random uppercase letter at a random boundary. Insertion
        # stays edit-distance-1 and preserves the marking character class
        # (markings are upper-case ASCII by convention).
        insert_at = rng.randrange(len(canonical) + 1)
        insert_ch = rng.choice(string.ascii_uppercase)
        return canonical[:insert_at] + insert_ch + canonical[insert_at:]
    if choice == "substitute":
        ch = canonical[idx]
        if ch.isalpha():
            # Substitute with an adjacent letter in the alphabet (keeps
            # edit distance to 1 and keeps the character class).
            sub = chr(((ord(ch.upper()) - ord("A") + 1) % 26) + ord("A"))
            return canonical[:idx] + sub + canonical[idx + 1 :]
    return None


def _apply_reordering(canonical: str, rng: random.Random) -> Optional[str]:
    """Permute `//`-separated segments into a non-canonical order."""
    segments = canonical.split("//")
    if len(segments) < 2:
        return None
    # Retry a few times to ensure we produce a non-identity permutation.
    for _ in range(8):
        shuffled = segments.copy()
        rng.shuffle(shuffled)
        if shuffled != segments:
            return "//".join(shuffled)
    return None


def _apply_missing_delimiter(canonical: str, rng: random.Random) -> Optional[str]:
    """Drop one `//` boundary, keeping the surrounding tokens."""
    if "//" not in canonical:
        return None
    # Replace one occurrence with a single space — models a human typing a
    # delimiter as whitespace.
    parts = canonical.split("//")
    if len(parts) < 2:
        return None
    boundary = rng.randrange(len(parts) - 1)
    rebuilt = parts[0]
    for i in range(1, len(parts)):
        sep = " " if (i - 1) == boundary else "//"
        rebuilt += sep + parts[i]
    return rebuilt


def _apply_superseded_token(canonical: str, rng: random.Random) -> Optional[str]:
    """Replace one canonical token with a superseded synonym."""
    for canonical_tok, superseded_tok in SUPERSEDED_TOKEN_MAP.items():
        if canonical_tok in canonical:
            return canonical.replace(canonical_tok, superseded_tok, 1)
    return None


def _apply_wrong_case(canonical: str, rng: random.Random) -> Optional[str]:
    """Correct tokens, wrong case."""
    choice = rng.choice(("lower", "title"))
    if choice == "lower":
        return canonical.lower()
    # Title-case each word, which is also non-canonical for markings.
    return canonical.title()


def _apply_garbled_delimiter(canonical: str, rng: random.Random) -> Optional[str]:
    """Replace `//` with a malformed glyph or spacing."""
    if "//" not in canonical:
        return None
    replacements = (" // ", "/ /", " / / ", "∕∕", " //", "// ")
    return canonical.replace("//", rng.choice(replacements), 1)


_MANGLING_TRANSFORMS = {
    "typo": _apply_typo,
    "reordering": _apply_reordering,
    "missing-delimiter": _apply_missing_delimiter,
    "superseded-token": _apply_superseded_token,
    "wrong-case": _apply_wrong_case,
    "garbled-delimiter": _apply_garbled_delimiter,
}


def _resolve_canonical_source(corpus_path: Path) -> Path:
    """
    Pin the mangled-fixture source to the canonical subset of a corpus.

    The ``mangled`` mode's fixture ``expected`` strings MUST be canonical
    CAPCO markings — they are the ground truth the decoder is graded
    against in the SC-004 accuracy gate. If the corpus mixes canonical
    and non-canonical text (as ``tests/corpus/`` does, with ``valid/``,
    ``invalid/``, and ``prose/`` siblings), walking the whole tree
    would pull regex-extractable shapes out of the ``invalid/`` fixtures
    — files like ``SECRET//SERCET//NOFORN`` — and treat the embedded
    typo as canonical, poisoning the decoder gate.

    Resolution rule: if ``corpus_path/valid/`` exists, pin there.
    Otherwise fall back to ``corpus_path`` unchanged so a homogeneous
    corpus (e.g., an Enron maildir) still works.
    """
    valid_subdir = corpus_path / "valid"
    if valid_subdir.is_dir():
        print(
            f"mangled mode: restricting corpus source to {valid_subdir} "
            "(skipping invalid/ and prose/ siblings so fixture `expected` "
            "strings stay canonical)",
            file=sys.stderr,
        )
        return valid_subdir
    return corpus_path


def generate_mangled_fixtures(
    corpus_paths: list[Path],
    output_dir: Path,
    min_cases: int,
    max_docs: Optional[int],
    seed: int,
) -> dict:
    """
    Produce labeled mangled-marking fixtures under ``output_dir``.

    Returns a summary dict with per-class case counts. Raises ``RuntimeError``
    if the total case count falls below ``min_cases`` after exhausting the
    corpus, since the SC-004 gate depends on ≥200 cases.

    Each path in ``corpus_paths`` is resolved through
    ``_resolve_canonical_source`` so a mixed-validity test corpus
    (``tests/corpus/`` with ``valid/`` + ``invalid/`` + ``prose/``)
    transparently narrows to ``valid/``. Pass homogeneous corpora directly
    (e.g. Enron maildir, CREST docs) for full-tree scanning.
    """
    resolved = [_resolve_canonical_source(p) for p in corpus_paths]
    rng = random.Random(seed)
    # One directory per class, created eagerly so downstream checks
    # that inspect directory existence find what they expect. Clear
    # any stale generated fixtures from prior runs so the resulting
    # fixture set is reproducible for the current corpus/seed without
    # disturbing non-JSON files such as README documentation or the
    # `.gitkeep` that keeps empty class dirs tracked in git.
    for cls in MANGLING_CLASSES:
        class_dir = output_dir / cls
        class_dir.mkdir(parents=True, exist_ok=True)
        for fixture_path in class_dir.glob("*.json"):
            fixture_path.unlink()

    # Collect unique canonical markings from the corpus. We dedupe because
    # a real corpus repeats common markings many times — a fixture with
    # 500 copies of `SECRET//NOFORN` is not 500 cases, it's one case with
    # inflated weight. We want coverage, not volume.
    canonicals: set[str] = set()
    scanned_docs = 0
    for _doc_id, text in iter_corpus_texts_multi(resolved, max_docs):
        scanned_docs += 1
        canonicals.update(extract_candidate_markings(text))
        # Stop scanning once we have enough distinct canonicals that
        # every class can hit its share.
        if len(canonicals) >= min_cases * 2:
            break

    if not canonicals:
        raise RuntimeError(
            f"No canonical-looking markings found in corpus paths: "
            f"{[str(p) for p in resolved]}. "
            f"Expected at least some `(CLASS//DISSEM)` or `CLASS//DISSEM` "
            f"shapes. Check the corpus contents."
        )

    # Distribute across classes. Each (canonical, class) pair is
    # sampled up to ``SAMPLES_PER_CLASS_PER_CANONICAL`` times per seed.
    # Because the curated canonical source (``tests/corpus/valid/``) is
    # small (~10 extractable shapes) and we want ≥200 fixtures for the
    # SC-004 gate, per-pair resampling multiplies coverage without
    # broadening the source. The RNG state advances on every call, so
    # repeated invocations of the same transform on the same canonical
    # typically yield distinct mangled outputs (different typo
    # position, different permutation, etc.). Duplicates that collapse
    # into the same (observed, expected) digest are dropped — the cap
    # is an upper bound, not a target. Deterministic transforms like
    # ``superseded-token`` and ``wrong-case(lower/title)`` cap out at
    # one or two distinct outputs per canonical regardless of
    # resampling, which is honest — we don't fabricate variation.
    SAMPLES_PER_CLASS_PER_CANONICAL = 16
    counts = Counter()
    case_digest_seen: set[str] = set()

    for canonical in sorted(canonicals):
        for cls in MANGLING_CLASSES:
            transform = _MANGLING_TRANSFORMS[cls]
            for _ in range(SAMPLES_PER_CLASS_PER_CANONICAL):
                observed = transform(canonical, rng)
                if observed is None or observed == canonical:
                    continue
                # Confidence heuristic: longer markings + more structure
                # make the (observed, expected) mapping more defensible.
                # Rounded to 2 decimals so IEEE-754 float artifacts
                # (0.80 + 0.01*2 = 0.8200000000000001) don't end up in
                # committed fixture JSON and churn diffs across
                # Python-version or platform changes. The decoder reads
                # this as an advisory source-weight; 2-decimal precision
                # is more than the heuristic actually asserts.
                source_confidence = round(
                    min(0.99, 0.80 + 0.01 * canonical.count("//")), 2
                )
                record = {
                    "observed": observed,
                    "expected": canonical,
                    "mangling_class": _class_to_pascal(cls),
                    "source_confidence": source_confidence,
                }
                # Stable filename derived from a content digest. For the
                # same corpus and the same ``--seed``, invocations
                # produce the same filenames, so committing the fixture
                # set is reproducible; different seeds generally produce
                # different observed strings and therefore different
                # filenames (the transforms themselves are RNG-driven).
                digest = hashlib.sha256(
                    f"{cls}\0{observed}\0{canonical}".encode()
                ).hexdigest()[:16]
                if digest in case_digest_seen:
                    continue
                case_digest_seen.add(digest)
                out_path = output_dir / cls / f"{digest}.json"
                out_path.write_text(json.dumps(record, indent=2) + "\n")
                counts[cls] += 1

    total = sum(counts.values())
    summary = {
        "scanned_docs": scanned_docs,
        "unique_canonicals": len(canonicals),
        "total_cases": total,
        "per_class": dict(counts),
    }
    if total < min_cases:
        raise RuntimeError(
            f"Generated {total} cases, below --min-cases {min_cases}. "
            f"Per-class distribution: {dict(counts)}. "
            f"Consider a larger corpus or a lower --min-cases threshold."
        )
    return summary


def _class_to_pascal(cls: str) -> str:
    """'missing-delimiter' -> 'MissingDelimiter' (matches Rust enum name)."""
    return "".join(part.capitalize() for part in cls.split("-"))


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def main():
    parser = argparse.ArgumentParser(
        description=(
            "Corpus analysis tool for classification marking "
            "vocabularies. See module docstring for the three modes."
        )
    )
    parser.add_argument(
        "--mode",
        choices=("baseline", "priors", "mangled", "heuristic-frequency"),
        default="baseline",
        help=(
            "baseline: token-frequency analysis (default, pre-Phase-C behavior). "
            "priors: corpus-derived priors for the Phase-D decoder. "
            "mangled: labeled mangled-marking fixtures for the decoder accuracy harness. "
            "heuristic-frequency: per-trigger frequency analysis for the issue #133 PR 2 "
            "position-aware classification heuristic — used to set HEURISTIC_RULE_AXIS_CAP."
        ),
    )
    parser.add_argument(
        "--tokens",
        type=Path,
        default=Path(__file__).parent / "tokens" / "capco.json",
        help="Path to token vocabulary JSON (default: tokens/capco.json)",
    )

    # ---- corpus source arguments ----
    corpus_group = parser.add_argument_group(
        "corpus sources",
        "Specify a custom corpus path or one or more named sources to auto-download.\n"
        "Issue #258 introduced the marking/prose stratum split — --mode priors\n"
        "now requires both strata. --corpus defaults to the marking stratum;\n"
        "named sources (--corpus-source enron|...) are all prose-dominant per\n"
        "issue #258 confirmation and go in the prose stratum. Use\n"
        "--marking-corpus / --prose-corpus to add stratum-tagged paths\n"
        "explicitly.",
    )
    corpus_group.add_argument(
        "--corpus",
        type=Path,
        default=None,
        help=(
            "Path to a corpus directory of text files.  "
            "Accepts Enron maildir trees, GovInfo extracted directories, "
            "or any directory of plain-text files.  "
            "Stratum defaults to 'marking' (override via --corpus-stratum). "
            "Overrides --corpus-source for the same stratum slot."
        ),
    )
    corpus_group.add_argument(
        "--corpus-stratum",
        choices=("marking", "prose"),
        default="marking",
        help=(
            "Stratum tag for --corpus (default: marking). The natural use "
            "case for --corpus is pointing at the marking-bearing fixtures "
            "in tests/corpus/valid/, so 'marking' is the default. Use "
            "'prose' for an external prose corpus."
        ),
    )
    corpus_group.add_argument(
        "--marking-corpus",
        type=Path,
        action="append",
        default=[],
        metavar="PATH",
        help=(
            "Add a marking-bearing corpus path (repeatable). The marking "
            "stratum supplies token_base_rates / country_code_base_rates."
        ),
    )
    corpus_group.add_argument(
        "--prose-corpus",
        type=Path,
        action="append",
        default=[],
        metavar="PATH",
        help=(
            "Add a prose-only corpus path (repeatable). The prose stratum "
            "supplies token_prose_base_rates / country_code_prose_base_rates "
            "(issue #258 — the null hypothesis for decoder recognition)."
        ),
    )
    corpus_group.add_argument(
        "--corpus-url",
        type=str,
        default=None,
        help="URL to download a corpus tar.gz (instead of Enron default).",
    )
    corpus_group.add_argument(
        "--corpus-source",
        choices=("enron", "congressional-record", "gao", "crest", "all"),
        action="append",
        dest="corpus_sources",
        metavar="SOURCE",
        help=(
            "Named corpus to auto-download and include in the analysis. "
            "May be specified multiple times to combine sources. "
            "Choices: enron, congressional-record, gao, crest, all. "
            "All four named sources are prose-dominant (per issue #258 "
            "owner confirmation: effectively zero portion-marking hits) "
            "and go in the prose stratum. "
            "Default for --mode priors when no source given: "
            "tests/corpus/valid/ (marking) + Enron (prose). "
            "crest: CIA CREST declassified documents from Internet Archive — "
            "recommended for --mode mangled (real classification marking artifacts)."
        ),
    )

    # ---- Congressional Record options ----
    crec_group = parser.add_argument_group(
        "Congressional Record options (--corpus-source congressional-record)"
    )
    crec_group.add_argument(
        "--crec-year",
        type=int,
        default=2023,
        metavar="YEAR",
        help="Calendar year of Congressional Record to download (default: 2023).",
    )
    crec_group.add_argument(
        "--crec-max-packages",
        type=int,
        default=20,
        metavar="N",
        help=(
            "Maximum number of session-day packages to download. "
            "Each package is ~30 MB compressed (HTM content is ~470 KB/day). "
            "Default: 20 (≈ 600 MB download budget)."
        ),
    )

    # ---- GAO report options ----
    gao_group = parser.add_argument_group(
        "GAO report options (--corpus-source gao)"
    )
    gao_group.add_argument(
        "--gao-years",
        type=str,
        default="2004,2005,2006,2007,2008",
        metavar="YEARS",
        help=(
            "Comma-separated report years to pull from GovInfo sitemaps. "
            "Sitemaps are available for 1989–2008. "
            "Default: 2004,2005,2006,2007,2008."
        ),
    )
    gao_group.add_argument(
        "--gao-max-reports",
        type=int,
        default=500,
        metavar="N",
        help=(
            "Maximum number of GAO reports to download. "
            "Each report is ~66 KB HTML; 500 reports ≈ 33 MB total. "
            "Default: 500."
        ),
    )

    # ---- CREST options ----
    crest_group = parser.add_argument_group(
        "CIA CREST options (--corpus-source crest)"
    )
    crest_group.add_argument(
        "--crest-max-docs",
        type=int,
        default=200,
        metavar="N",
        help=(
            "Maximum number of CIA CREST documents to download from Internet "
            "Archive. Each document is a DjVu OCR text file (~25 KB average). "
            "200 documents ≈ 5 MB total; sufficient to exceed the SC-004 gate "
            "of 200 mangled cases. Default: 200."
        ),
    )

    # ---- common options ----
    parser.add_argument(
        "--max-docs",
        type=int,
        default=None,
        help="Limit analysis to N documents total (for quick test runs).",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help=(
            "Output path. baseline / priors: JSON file (stdout if omitted). "
            "mangled: directory (one JSON per case under <class>/)."
        ),
    )
    parser.add_argument(
        "--min-cases",
        type=int,
        default=200,
        help=(
            "Mangled mode: minimum total case count. Generator fails if the "
            "corpus produces fewer than this many distinct cases. Default: 200 "
            "(matches SC-004 gate)."
        ),
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=0,
        help=(
            "Mangled mode: RNG seed for deterministic transform output. "
            "Same seed + same corpus = same fixture contents."
        ),
    )
    args = parser.parse_args()

    # Load tokens
    tokens_by_category = load_tokens(args.tokens)
    flat_tokens = all_tokens_flat(tokens_by_category)
    print(
        f"Loaded {len(flat_tokens)} tokens in {len(tokens_by_category)} categories",
        file=sys.stderr,
    )

    # ---------------------------------------------------------------------------
    # Resolve corpus paths (stratified per issue #258)
    # ---------------------------------------------------------------------------
    # The marking stratum supplies P(token|marking); the prose stratum
    # supplies P(token|prose). --mode priors needs both. Other modes
    # (baseline, mangled, heuristic-frequency) operate on a single
    # combined path list and are stratum-agnostic.
    #
    # Priority for stratum tagging:
    #   1. --marking-corpus / --prose-corpus (explicit per-stratum paths,
    #      repeatable)
    #   2. --corpus + --corpus-stratum (single path with stratum tag,
    #      defaults to marking)
    #   3. --corpus-source (named auto-download — all prose stratum per
    #      issue #258 owner confirmation that all four sources are
    #      prose-dominant with effectively zero portion-marking hits)
    #   4. Default for --mode priors: tests/corpus/valid/ (marking) +
    #      Enron (prose) so a developer regen does not require flags.
    marking_paths: list[Path] = list(args.marking_corpus)
    prose_paths: list[Path] = list(args.prose_corpus)

    # Track which strata received an explicit `--corpus` path so we
    # can preserve the pre-stratification precedence rule "--corpus
    # takes precedence over --corpus-source" on a per-stratum basis.
    # Otherwise `--corpus /tmp/prose --corpus-stratum prose
    # --corpus-source enron` would silently mix `/tmp/prose` and
    # Enron into the prose stratum instead of letting the explicit
    # path override the named source for that stratum.
    explicit_corpus_strata: set[str] = set()
    if args.corpus:
        if args.corpus_stratum == "marking":
            marking_paths.append(args.corpus)
            explicit_corpus_strata.add("marking")
        else:
            prose_paths.append(args.corpus)
            explicit_corpus_strata.add("prose")

    if args.corpus_sources:
        gao_years_parsed = [int(y.strip()) for y in args.gao_years.split(",") if y.strip()]
        sources: set[str] = set()
        for s in args.corpus_sources:
            if s == "all":
                sources.update({"enron", "congressional-record", "gao", "crest"})
            else:
                sources.add(s)
        # All four named sources are prose-dominant (issue #258 owner
        # confirmation): Enron is corporate prose, CIA CREST documents
        # are post-classification text with markings stripped, and the
        # Congressional Record / GAO Reports surfaced effectively zero
        # portion-marking hits in prior analysis runs. They contribute
        # to the prose stratum unless the user has already supplied an
        # explicit `--corpus` for that stratum (in which case the
        # explicit path wins per the documented precedence).
        if "prose" in explicit_corpus_strata:
            print(
                "Note: --corpus targets the prose stratum and takes precedence "
                "over --corpus-source for that stratum; named prose sources "
                f"{sorted(sources)} ignored.",
                file=sys.stderr,
            )
        else:
            if "enron" in sources:
                prose_paths.append(download_enron())
            if "congressional-record" in sources:
                prose_paths.append(
                    download_congressional_record(
                        year=args.crec_year,
                        max_packages=args.crec_max_packages,
                    )
                )
            if "gao" in sources:
                prose_paths.append(
                    download_gao_reports(
                        years=gao_years_parsed,
                        max_reports=args.gao_max_reports,
                    )
                )
            if "crest" in sources:
                prose_paths.append(
                    download_crest_corpus(
                        max_documents=args.crest_max_docs,
                    )
                )

    # Per-mode defaults when no source flags are supplied:
    # - `--mode priors` needs both strata, so it pulls the marking
    #   stratum from `tests/corpus/valid/` and the prose stratum
    #   from Enron.
    # - `--mode baseline` / `mangled` / `heuristic-frequency` keep
    #   their legacy single-path Enron default (the stratum tag is
    #   irrelevant for these modes — they iterate `corpus_paths`,
    #   the combined list, and don't separate marking from prose).
    repo_root = Path(__file__).resolve().parents[2]
    default_marking = repo_root / "tests" / "corpus" / "valid"
    if not marking_paths and not prose_paths and not args.corpus_sources and not args.corpus:
        if args.mode == "priors":
            if default_marking.exists():
                marking_paths.append(default_marking)
            prose_paths.append(download_enron())
        else:
            # baseline / mangled / heuristic-frequency
            prose_paths.append(download_enron())

    corpus_paths: list[Path] = list(marking_paths) + list(prose_paths)
    missing = [p for p in corpus_paths if not p.exists()]
    if missing:
        print(f"Error: corpus paths do not exist: {missing}", file=sys.stderr)
        sys.exit(1)

    def _combined_fp(paths: list[Path]) -> str:
        if not paths:
            return "sha512:" + ("0" * 128)
        if len(paths) == 1:
            return _corpus_fingerprint(paths[0])
        return (
            "sha512:"
            + hashlib.sha512(
                "\n".join(_corpus_fingerprint(p) for p in paths).encode()
            ).hexdigest()
        )

    if args.mode == "priors":
        if not marking_paths or not prose_paths:
            print(
                "Error: --mode priors requires both marking and prose "
                "corpus sources (issue #258).\n"
                "  Provide --marking-corpus PATH and --prose-corpus PATH, or\n"
                "  --corpus PATH (defaults to marking) plus a prose source\n"
                "  via --corpus-source enron|congressional-record|gao|crest, or\n"
                "  run with no source args to use tests/corpus/valid/ + Enron.",
                file=sys.stderr,
            )
            sys.exit(1)

        # `--max-docs` is documented as a total document cap. With
        # the per-stratum `run_analysis` calls below, applying the
        # raw `args.max_docs` to each call would double the actual
        # cap (a `--max-docs 1000` invocation could process 2000
        # docs total). Honor the contract by capping the marking
        # stratum at the full budget, then giving the prose stratum
        # `--max-docs - actual_marking_docs` so the total stays
        # bounded — and crucially using the *actual* count of
        # processed documents from `marking_results`, not a
        # heuristic file count (which undercounts on
        # custom marking corpora that use Enron-shaped or HTML-
        # wrapped inputs without a `.txt` extension and would let
        # the total exceed the budget).
        marking_max = args.max_docs

        # Marking stratum: drop the default 20-byte minimum so short
        # canonical fixtures (`(S)`, `SECRET//NF`, single-portion
        # lines) contribute to the marking-side counts. Without this
        # the regenerated priors emit `UNCLASSIFIED`, `SECRET`, and
        # other short banner-form fixtures at count 0 even though
        # they exist on disk — flagged in PR #312 review.
        marking_results = run_analysis(
            marking_paths, tokens_by_category, marking_max, min_length=1
        )

        # Compute the prose budget from the actual marking document
        # count. `0` is a meaningful value (means "marking already
        # consumed the budget, prose gets nothing") — see the
        # `is not None` handling in `iter_corpus_texts*` above which
        # treats 0 as a hard zero rather than falsy/unlimited.
        if args.max_docs is None:
            prose_max = None
        else:
            marking_doc_count = int(
                marking_results.get("corpus_stats", {}).get("document_count", 0)
            )
            prose_max = max(0, args.max_docs - marking_doc_count)
            if prose_max == 0:
                print(
                    f"Note: --max-docs={args.max_docs} consumed entirely by the "
                    f"marking stratum ({marking_doc_count} docs); prose stratum "
                    "will be skipped. The build-time fail-closed check in "
                    "crates/capco/build.rs will reject the resulting priors.json. "
                    "Raise --max-docs to give prose a non-zero budget.",
                    file=sys.stderr,
                )

        prose_results = run_analysis(prose_paths, tokens_by_category, prose_max)

        priors = derive_priors(marking_results, prose_results, tokens_by_category)
        priors["corpus_fingerprint"] = _combined_fp(corpus_paths)
        priors["marking_corpus_fingerprint"] = _combined_fp(marking_paths)
        priors["prose_corpus_fingerprint"] = _combined_fp(prose_paths)
        # Path metadata is relativized to repo root so the committed
        # `priors.json` is mechanically reproducible across worktrees
        # and developers — flagged in PR #312 review. Repo-relative
        # paths still identify which corpus directories produced this
        # artifact (e.g., `tests/corpus/valid` for the marking
        # stratum) without leaking absolute filesystem paths like
        # `/home/<user>/...`. The download-cache paths used by the
        # named sources (Enron / GovInfo / GAO / CREST under
        # `tools/corpus-analysis/.cache/`) live below the repo root
        # by construction, so they relativize cleanly too.
        repo_root = Path(__file__).resolve().parents[2]

        def _rel_path(p: Path) -> str:
            try:
                return str(p.resolve().relative_to(repo_root))
            except ValueError:
                # Path lies outside the repo (e.g., a developer-
                # supplied --marking-corpus pointing elsewhere). Fall
                # back to the basename rather than leaking the
                # absolute path; the corpus_fingerprint still
                # identifies the input precisely.
                return p.name

        priors["metadata"] = {
            "vocabulary_file": _rel_path(args.tokens),
            "marking_corpus_paths": [_rel_path(p) for p in marking_paths],
            "prose_corpus_paths": [_rel_path(p) for p in prose_paths],
            "token_count": len(flat_tokens),
            "category_count": len(tokens_by_category),
        }
        payload = priors

        output_json = json.dumps(payload, indent=2)
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(output_json)
            print(f"Results written to {args.output}", file=sys.stderr)
        else:
            print(output_json)
        return

    if args.mode == "baseline":
        results = run_analysis(corpus_paths, tokens_by_category, args.max_docs)
        results["metadata"] = {
            "vocabulary_file": str(args.tokens),
            "corpus_paths": [str(p) for p in corpus_paths],
            "marking_corpus_paths": [str(p) for p in marking_paths],
            "prose_corpus_paths": [str(p) for p in prose_paths],
            "token_count": len(flat_tokens),
            "category_count": len(tokens_by_category),
        }

        output_json = json.dumps(results, indent=2)
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(output_json)
            print(f"Results written to {args.output}", file=sys.stderr)
        else:
            print(output_json)
        return

    if args.mode == "heuristic-frequency":
        results = measure_heuristic_trigger_frequency(corpus_paths, args.max_docs)
        # Intentionally omit corpus paths from committed output —
        # absolute paths leak machine-specific detail across machines.
        # The corpus_fingerprint (SHA-512 over file metadata, content-
        # ignorant per Constitution V) is the reproducible identifier.
        combined_fp = _corpus_fingerprint(corpus_paths[0]) if len(corpus_paths) == 1 else (
            "sha512:"
            + hashlib.sha512(
                "\n".join(_corpus_fingerprint(p) for p in corpus_paths).encode()
            ).hexdigest()
        )
        results["corpus_fingerprint"] = combined_fp
        output_json = json.dumps(results, indent=2)
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(output_json)
            print(
                f"Heuristic-frequency results written to {args.output}", file=sys.stderr
            )
        else:
            print(output_json)
        # Print a quick human-readable summary on stderr.
        print(
            f"\nDocs processed: {results['docs_processed']}",
            file=sys.stderr,
        )
        s = results["summary"]
        print(
            f"Triggers with zero marking-context hits: "
            f"{s['triggers_with_zero_marking_context']} / {s['total_triggers']}",
            file=sys.stderr,
        )
        print(
            f"Max marking-context count for any trigger: "
            f"{s['max_marking_context_count']}",
            file=sys.stderr,
        )
        return

    # mangled mode — multi-corpus aware; scans all resolved paths for canonicals
    if not args.output:
        print(
            "Error: --mode mangled requires --output <dir>",
            file=sys.stderr,
        )
        sys.exit(2)
    output_dir = args.output
    output_dir.mkdir(parents=True, exist_ok=True)
    try:
        summary = generate_mangled_fixtures(
            corpus_paths=corpus_paths,
            output_dir=output_dir,
            min_cases=args.min_cases,
            max_docs=args.max_docs,
            seed=args.seed,
        )
    except RuntimeError as exc:
        print(f"Error: {exc}", file=sys.stderr)
        sys.exit(3)

    print(
        f"Generated {summary['total_cases']} mangled fixtures across "
        f"{len(summary['per_class'])} classes "
        f"from {summary['unique_canonicals']} unique canonicals "
        f"({summary['scanned_docs']} docs scanned).",
        file=sys.stderr,
    )
    for cls, count in sorted(summary["per_class"].items()):
        print(f"  {cls:22s} {count:5d}", file=sys.stderr)


if __name__ == "__main__":
    main()
