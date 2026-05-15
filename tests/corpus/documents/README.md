<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Document Corpus

Multi-page synthetic-positive document fixtures: 42 declassified CIA CREST
documents (1990–2010) with **synthetic** CAPCO portion marks and banners
overlaid on the prose. These are end-to-end document fixtures — multiple
pages, full banner + CAB + portion-marked paragraphs — complementing the
per-rule micro-fixtures in `../valid/`, `../invalid/`, and `../prose/`.

## Directory Structure

```
documents/
  README.md                  this file
  specs/                     hand-curated source-of-truth (42 files)
  marked/                    rendered marked docs (42 files)
  ground_truth.json          aggregate ground truth (one record per doc)
  <stem>.expected.json       per-doc fixture metadata (42 files)
```

## Specs (source of truth)

Each `specs/<stem>.md` is a YAML-frontmatter + page-separated markdown file:

```
---
identifier: CIA-RDP01M00147R000100350002-7
title: "..."
year: 1990
source_pdf: https://archive.org/download/.../<stem>.pdf
cab:
  classified_by: 187902
  derived_from: CIA-SCG-1234 10 May 2010
  declassify_on: 20370402
---

=== page 1 ===
banner: SECRET//NOFORN/PROPIN

(S//NF) First paragraph...
(U//FOUO) Second paragraph...

=== page 2 ===
banner: TOP SECRET//SI//NOFORN
...
```

Specs are the editable source. Re-running `render_corpus.py` regenerates
everything else.

## Marked (rendered output)

`marked/<stem>.md` is the rendered document with portion marks inline,
banners at the top and bottom of each page, and the CAB on the final page.
This is what a parser consumes as a "document" input.

## Ground Truth

Each `<stem>.expected.json` follows the marque test-corpus schema with an
extension for structural ground truth:

```json
{
  "diagnostics": [],
  "ground_truth": {
    "identifier": "...",
    "title": "...",
    "year": 1990,
    "source_pdf": "...",
    "cab": { "classified_by": ..., "derived_from": ..., "declassify_on": ... },
    "pages": [
      {
        "page_num": 1,
        "banner": "SECRET//NOFORN/PROPIN",
        "paragraphs": [
          { "mark": "S//NF", "text": "...", "is_table": false }
        ]
      }
    ]
  }
}
```

- `diagnostics: []` declares these are valid fixtures — the parser/validator
  should report no diagnostics against the rendered `marked/<stem>.md`.
- `ground_truth` is the structural extraction target (banner per page,
  portion mark per paragraph, CAB on the document).

`ground_truth.json` at the corpus root is the aggregate of all 42 per-doc
ground truths in a single file for crawler-style iteration.

## Provenance

Source prose comes from the **CIA Records Search Tool (CREST)** declassified
release archive, mirrored on Internet Archive. The documents have completed
the declassification review process and are public domain.

The classification **markings** overlaid on this prose are **synthetic**:
they are drawn from the public CAPCO marking vocabulary in
`tools/cia-crest-corpus/portions.toml` and assigned by
`tools/cia-crest-corpus/randomize_portions.py`. **No marking in this corpus
reflects the original classification of any source document.**

See [`../CORPUS_PROVENANCE.md`](../CORPUS_PROVENANCE.md) for the corpus-wide
provenance statement.

## Regenerating

```sh
cd tools/cia-crest-corpus
python3 render_corpus.py
```

To re-shuffle the synthetic portion-mark assignments:

```sh
python3 randomize_portions.py --seed 1
python3 render_corpus.py
```

Note that re-shuffling portion marks invalidates the hand-filled rollup
banners; you'll need to redo banner work for any spec where the portion-mark
pool changes.
