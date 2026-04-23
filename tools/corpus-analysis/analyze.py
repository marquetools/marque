#!/usr/bin/env -S uv run --script
# SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
#
# SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0

"""
Token frequency analyzer for classification marking vocabularies.

Given a token vocabulary (JSON) and a text corpus, measures how often each
token appears in normal (non-IC) English text and in what structural contexts.
The output is a frequency table that Rust build scripts consume to set
empirical base rates for the probabilistic recognition engine.

Usage:
    # Default: download Enron corpus, analyze against CAPCO tokens
    python analyze.py

    # Custom corpus path
    python analyze.py --corpus /path/to/text/files/

    # Custom token vocabulary
    python analyze.py --tokens tokens/my-vocab.json

    # Custom corpus URL (tar.gz of text files)
    python analyze.py --corpus-url https://example.com/corpus.tar.gz
"""

import argparse
import json
import os
import re
import sys
import tarfile
import email
import email.policy
from collections import Counter
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
        try:
            msg = email.message_from_string(text, policy=email.policy.default)
            body = msg.get_body(preferencelist=("plain",))
            if body:
                content = body.get_content()
                if isinstance(content, str):
                    return content
        except Exception:
            pass
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

    # // stats
    docs_with_dslash = 0
    total_dslash = 0
    dslash_in_url = 0
    dslash_not_url = 0

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
    }

    return output


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def main():
    parser = argparse.ArgumentParser(
        description="Measure token frequencies in a text corpus for classification marking analysis."
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
        help="Output JSON path. Default: stdout.",
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

    # Run analysis
    results = run_analysis(corpus_path, tokens_by_category, args.max_docs)

    # Add metadata
    results["metadata"] = {
        "vocabulary_file": str(args.tokens),
        "corpus_path": str(corpus_path),
        "token_count": len(flat_tokens),
        "category_count": len(tokens_by_category),
    }

    # Output
    output_json = json.dumps(results, indent=2)

    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(output_json)
        print(f"Results written to {args.output}", file=sys.stderr)
    else:
        print(output_json)


if __name__ == "__main__":
    main()
