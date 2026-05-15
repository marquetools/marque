<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# CIA CREST Corpus Pipeline

Builds the multi-page synthetic-positive document fixtures at
`tests/corpus/documents/`. The pipeline pulls declassified CIA CREST PDFs from
Internet Archive, cleans MinerU's OCR output, and overlays synthetic CAPCO
portion marks and banners from a curated catalog.

The body prose is real declassified text (now public domain through the
declassification process). The markings are **synthetic** — drawn from the
public CAPCO marking vocabulary in `portions.toml` and overlaid on the prose.
No marking in the rendered corpus reflects what was originally on the source
document.

## Pipeline

```
fetch_crest.py     → work/pdfs/*.pdf + work/manifest.json
[MinerU, external] → work/md/<stem>/ocr/<stem>_content_list.json
extract_clean.py   → work/clean/<stem>.md          (sanity preview)
make_specs.py      → tests/corpus/documents/specs/<stem>.md
                     (per-page scaffolds with `(?)` portion + `banner: ???`)
randomize_portions → rewrites the specs in place: each file samples N marks
                     from portions.toml and assigns them to paragraphs
[human]            → fill in `banner:` rollups and CAB fields by hand
render_corpus.py   → tests/corpus/documents/marked/<stem>.md
                     tests/corpus/documents/ground_truth.json (aggregate)
                     tests/corpus/documents/<stem>.expected.json (per doc)
```

`fetch_crest.py`, `extract_clean.py`, and `make_specs.py` are one-shot
construction steps. They live here for reproducibility; the corpus output
under `tests/corpus/documents/` is what gets committed.

`randomize_portions.py` and `render_corpus.py` are the live re-run path —
edit a spec by hand, re-render, ship.

## Quick Start

```sh
uv --version

# One-time construction (requires MinerU separately):
uv run --script fetch_crest.py
# ... run MinerU on work/pdfs/*.pdf, output to work/md/<stem>/ ...
uv run --script extract_clean.py
uv run --script make_specs.py

# Re-run path (after editing a spec):
uv run --script randomize_portions.py     # optional: re-shuffle portion marks
uv run --script render_corpus.py
```

CLI flags worth knowing:

- `randomize_portions.py --marks-per-file N` — change pool size per file (default 4).
- `randomize_portions.py --seed N` — shift the deterministic seed and reshuffle.
- `randomize_portions.py --only <stem> [<stem>...]` — restrict to specific docs.
- `render_corpus.py --corpus-dir DIR` — render somewhere other than the default.

## Files

| File                    | Purpose                                                          |
|-------------------------|------------------------------------------------------------------|
| `fetch_crest.py`        | Curated Internet Archive pull (themed queries, year-bucketed).   |
| `extract_clean.py`      | Flat one-file-per-doc cleaning preview.                          |
| `cleaner.py`            | Shared regex helpers (stamps, banners, cable headers).           |
| `make_specs.py`         | Per-page editable spec scaffolds.                                |
| `randomize_portions.py` | Synthetic portion-mark assignment from `portions.toml`.          |
| `render_corpus.py`      | specs → marked/ + ground_truth.json + per-doc .expected.json.    |
| `portions.toml`         | 39-mark CAPCO catalog used by `randomize_portions.py`.           |

## MinerU

`make_specs.py` and `extract_clean.py` expect MinerU output at
`work/md/<stem>/ocr/<stem>_content_list.json` for each PDF in
`work/manifest.json`. Run MinerU separately:

```sh
mineru -p work/pdfs/<file>.pdf -o work/md/
```

The MinerU build itself isn't pinned here — install per its own instructions.

## Provenance and Compliance

See [`tests/corpus/CORPUS_PROVENANCE.md`](../../tests/corpus/CORPUS_PROVENANCE.md)
for the corpus-wide provenance statement. The CIA CREST documents are
declassified releases from the CIA Records Search Tool; the markings overlaid
on top are synthetic.
