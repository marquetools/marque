#!/usr/bin/env -S uv run --script
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
import json
import math
import os
import random
import re
import string
import sys
import tarfile
import email
import email.policy
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

# ---------------------------------------------------------------------------
# Corpus loading
# ---------------------------------------------------------------------------

ENRON_URL = "https://www.cs.cmu.edu/~enron/enron_mail_20150507.tar.gz"
ENRON_CACHE_DIR = Path(__file__).parent / ".cache"
ENRON_EXTRACT_DIR = ENRON_CACHE_DIR / "enron_mail"


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


def iter_corpus_texts(corpus_path: Path, max_docs: Optional[int] = None):
    """
    Yield (doc_id, text) tuples from a corpus directory.

    Handles both raw text files and RFC 2822 email files (Enron format).
    """
    count = 0
    for root, _dirs, files in os.walk(corpus_path):
        for fname in files:
            if fname.startswith("."):
                continue
            fpath = Path(root) / fname
            try:
                raw = fpath.read_bytes()
                text = extract_body(raw)
                if text and len(text.strip()) > 20:
                    doc_id = str(fpath.relative_to(corpus_path))
                    yield doc_id, text
                    count += 1
                    if max_docs and count >= max_docs:
                        return
            except (UnicodeDecodeError, OSError):
                continue

    if count == 0:
        print(f"Warning: no documents found in {corpus_path}", file=sys.stderr)


def extract_body(raw: bytes) -> Optional[str]:
    """
    Extract the text body from raw file bytes.

    If it looks like an RFC 2822 email, parse it and extract the text/plain
    body. Otherwise treat the whole thing as plain text.
    """
    try:
        text = raw.decode("utf-8", errors="replace")
    except Exception:
        return None

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

# Match a ``REL TO`` header followed by comma- or whitespace-separated
# entries until the block terminator. CAPCO §H.8 says ``//`` is the
# authoritative end-of-category separator (project memory:
# `project_capco_separator_conventions`); the parser also stops at
# end-of-line or end-of-portion ``)`` so the regex doesn't run away on
# malformed inputs. The body group is non-greedy and width-capped to
# keep pathological inputs from blowing up the regex backtracker.
_REL_TO_BLOCK_RE = re.compile(
    r"REL\s+TO\s+([A-Z][A-Z0-9_,\s]{0,200})",
    re.IGNORECASE,
)
# A trigraph in a REL TO body is 2-4 ASCII uppercase / digit / underscore
# chars (matches ``CountryCode`` invariants in
# ``crates/ism/src/attrs.rs``: ``EU`` is the shortest, longer forms like
# ``AUSTRALIA_GROUP`` are accepted but rare). Kept narrow because a
# corpus-level analyzer that admits 5+ char tokens absorbs prose words
# (``"USA, ALSO,"`` would lift ``ALSO`` into the prior).
_REL_TO_TRIGRAPH_TOKEN_RE = re.compile(r"\b[A-Z][A-Z0-9_]{1,3}\b")


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
    corpus_path: Path,
    tokens_by_category: dict,
    max_docs: Optional[int] = None,
) -> dict:
    """Run the full analysis over a corpus. Returns the frequency table."""
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
    # The counter key space includes 2-char codes (``EU``), 3-char
    # trigraphs, 4-char tetragraphs (``FVEY``, ``ACGU``, ``NATO``),
    # and group codes — whatever the upstream walker collects.
    rel_to_trigraph_hits = Counter()

    print("Analyzing corpus...", file=sys.stderr)

    for doc_id, text in iter_corpus_texts(corpus_path, max_docs):
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
    }

    return output


# ---------------------------------------------------------------------------
# Phase D: corpus-derived priors output
# ---------------------------------------------------------------------------

PRIORS_SCHEMA_VERSION = "marque-priors-2"

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


def derive_priors(analysis: dict, tokens_by_category: dict) -> dict:
    """
    Reshape a baseline analysis result into the priors.json schema.

    The baseline analysis gives per-token raw counts and contextual
    signals. The priors schema needs: token base rates (with precomputed
    log-priors), template base rates, and strict-context priors.

    Token log-prior: ``log_prior = log((hits + 1) / (total_tokens + |V|))``
    Laplace-smoothed so zero-count tokens don't map to ``-inf``. The
    smoothing constant matches what the Rust decoder assumes at scoring
    time; changing it here requires changing it there in lockstep.
    """
    tokens = analysis["tokens"]
    total_hits = sum(t.get("raw_count", 0) for t in tokens.values())
    vocab_size = max(1, len(tokens))

    # Laplace smoothing: prior = (hits + 1) / (total + |V|)
    denom = float(total_hits + vocab_size)

    token_base_rates = {}
    for token, data in tokens.items():
        hits = int(data.get("raw_count", 0))
        numerator = float(hits + 1)
        log_prior = math.log(numerator / denom)
        token_base_rates[token] = {
            "count": hits,
            "log_prior": round(log_prior, 6),
        }

    # Template base rates: we approximate by counting co-occurrence
    # patterns. The exact template detection is CAPCO-specific and will
    # be refined by the Rust decoder at scoring time; priors here give
    # the base rates a generic-enough shape for build.rs to consume.
    total_dslash = analysis["corpus_stats"]["double_slash"]["not_in_urls"]
    cooccurrences = analysis.get("cooccurrence_pairs", {})
    total_cooc = sum(cooccurrences.values()) or 1

    template_base_rates = {
        "classification": {
            "count": sum(
                tokens.get(t, {}).get("raw_count", 0)
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
    # blocks observed in the corpus, summed with the baseline ratios
    # in ``_REL_TO_COUNTRY_CODE_BASELINE`` so the decoder always has a
    # finite log-prior for FVEY partners and known fuzzy lookalikes.
    # The emitted table covers all CAPCO country-code shapes — 2-char
    # codes (``EU``), 3-char trigraphs, 4-char tetragraphs (``FVEY``,
    # ``ACGU``, ``NATO``), and group codes — even though the legacy
    # baseline name still says "trigraph". Smoothing follows the same
    # Laplace formula as the token table so the two are directly
    # comparable inside the decoder's ``score_candidate``:
    # ``log_prior(USA) - log_prior(UZB)`` swamps a single
    # edit-distance-1 advantage when USA's hit count exceeds UZB's by
    # orders of magnitude.
    raw_country_code_hits = analysis.get("rel_to_trigraph_hits", {}) or {}
    country_code_counts = Counter(_REL_TO_COUNTRY_CODE_BASELINE)
    country_code_counts.update(raw_country_code_hits)
    total_country_code_hits = sum(country_code_counts.values())
    country_code_vocab_size = max(1, len(country_code_counts))
    country_code_denom = float(total_country_code_hits + country_code_vocab_size)
    country_code_base_rates = {}
    for tok, hits in country_code_counts.items():
        log_prior = math.log(float(hits + 1) / country_code_denom)
        country_code_base_rates[tok] = {
            "count": int(hits),
            "log_prior": round(log_prior, 6),
        }

    return {
        "schema_version": PRIORS_SCHEMA_VERSION,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "token_base_rates": token_base_rates,
        "template_base_rates": template_base_rates,
        "country_code_base_rates": country_code_base_rates,
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
    corpus_path: Path,
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
    for _doc_id, text in iter_corpus_texts(corpus_path, max_docs=max_docs):
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
_MARKING_PORTION_RE = re.compile(r"\(([A-Z]{1,3}(?://[A-Z][A-Z0-9 ,/-]+)+)\)")
_MARKING_BANNER_RE = re.compile(
    r"(?:^|(?<=\s))"
    r"((?:UNCLASSIFIED|CONFIDENTIAL|SECRET|TOP SECRET|RESTRICTED)"
    r"(?://[A-Z][A-Z0-9 ,/-]+)+)"
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
    corpus_path: Path,
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

    ``corpus_path`` is resolved through ``_resolve_canonical_source`` so
    a mixed-validity test corpus (``tests/corpus/`` with ``valid/`` +
    ``invalid/`` + ``prose/``) transparently narrows to ``valid/``. Pass
    a homogeneous corpus directly if you want a different behavior.
    """
    corpus_path = _resolve_canonical_source(corpus_path)
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
    for _doc_id, text in iter_corpus_texts(corpus_path, max_docs):
        scanned_docs += 1
        canonicals.update(extract_candidate_markings(text))
        # Stop scanning once we have enough distinct canonicals that
        # every class can hit its share.
        if len(canonicals) >= min_cases * 2:
            break

    if not canonicals:
        raise RuntimeError(
            f"No canonical-looking markings found in corpus {corpus_path}. "
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
    parser.add_argument(
        "--corpus",
        type=Path,
        default=None,
        help="Path to corpus directory of text files. Default: download Enron.",
    )
    parser.add_argument(
        "--corpus-url",
        type=str,
        default=None,
        help="URL to download a corpus tar.gz (instead of Enron default).",
    )
    parser.add_argument(
        "--max-docs",
        type=int,
        default=None,
        help="Limit analysis to N documents (for quick test runs).",
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

    # Resolve corpus
    if args.corpus:
        corpus_path = args.corpus
    else:
        corpus_path = download_enron()

    if not corpus_path.exists():
        print(f"Error: corpus path {corpus_path} does not exist", file=sys.stderr)
        sys.exit(1)

    if args.mode in ("baseline", "priors"):
        results = run_analysis(corpus_path, tokens_by_category, args.max_docs)
        results["metadata"] = {
            "vocabulary_file": str(args.tokens),
            "corpus_path": str(corpus_path),
            "token_count": len(flat_tokens),
            "category_count": len(tokens_by_category),
        }

        if args.mode == "priors":
            priors = derive_priors(results, tokens_by_category)
            priors["corpus_fingerprint"] = _corpus_fingerprint(corpus_path)
            payload = priors
        else:
            payload = results

        output_json = json.dumps(payload, indent=2)
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(output_json)
            print(f"Results written to {args.output}", file=sys.stderr)
        else:
            print(output_json)
        return

    if args.mode == "heuristic-frequency":
        results = measure_heuristic_trigger_frequency(corpus_path, args.max_docs)
        # Intentionally omit `corpus_path` from committed output —
        # absolute developer-environment paths leak machine-specific
        # detail and churn diffs across machines. The
        # `corpus_fingerprint` (SHA-512 over file metadata, content-
        # ignorant per Constitution V) is the reproducible identifier.
        results["corpus_fingerprint"] = _corpus_fingerprint(corpus_path)
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

    # mangled mode
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
            corpus_path=corpus_path,
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
