<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Corpus Contract

This document codifies the test corpus requirements for the marque MVP.

## Composition Requirements

### Known-Bad Fixtures (`invalid/`)

The corpus MUST contain **at least 3 known-bad fixtures per rule**:

| Rule | Min Fixtures | Description |
|------|-------------|-------------|
| E001 | 3 | Banner uses portion abbreviation |
| E002 | 3 | REL TO list missing USA trigraph |
| E003 | 3 | Misordered banner blocks |
| E004 | 3 | Separator count normalization |
| E005 | 3 | Declassification info misplaced (banner or portion; belongs in CAB) |
| E006 | 3 | Deprecated dissem control |
| E007 | 3 | X-shorthand declassification date |
| E008 | 3 | Unrecognized token in marking |
| W001 | 3 | Deprecated marking (warning) |
| C001 | 3 | Corrections-map typo replacement |

**Total minimum**: 30 known-bad fixtures (plan target: >= 40).

Each known-bad fixture MUST have a sibling `.expected.json` pinning:
- Rule IDs expected to fire
- Exact byte spans of each diagnostic

### Known-Good Fixtures (`valid/`)

The corpus MUST contain **at least 20 known-good fixtures** with correctly
formed markings spanning all marking types (portion, banner, CAB).

Each known-good fixture MUST have a sibling `.expected.json` containing
`{ "diagnostics": [] }`.

### Clean Prose Corpus (`prose/`)

The corpus MUST contain **at least 1000 lines** of body prose
containing no markings. The prose MUST include at least 20 incidental
parenthesized single-letter tokens (e.g., `(S)`, `(a)`) in mid-sentence
positions to exercise the disambiguation heuristic.

The prose corpus is wired into the accuracy harness as a zero-diagnostic
precision gate.

### Document Fixtures (`documents/`)

Multi-page synthetic-positive document fixtures derived from declassified
CIA CREST releases (1990–2010) with synthetic CAPCO markings overlaid on
the prose. Each fixture is a full document — multiple pages, banner + CAB +
portion-marked paragraphs — and stresses end-to-end document handling
rather than per-rule diagnostics.

The corpus MUST contain **at least 40 document fixtures** with:

- A `specs/<stem>.md` source-of-truth file (YAML frontmatter + page blocks).
- A rendered `marked/<stem>.md` produced by `render_corpus.py`.
- A per-doc `<stem>.expected.json` declaring `{"diagnostics": []}` and a
  structural `ground_truth` field (banner per page, portion mark per
  paragraph, CAB).
- An aggregate `ground_truth.json` for crawler-style iteration.

Document fixtures are valid (synthetic-positive); zero diagnostics is the
expectation. Pipeline lives at `tools/cia-crest-corpus/`; see
`documents/README.md` for format details.

## Tagging

The corpus is tagged `mvp-corpus-v1`; the accuracy gates are measured
against exactly that tag.

## Accuracy Thresholds

- **Lint accuracy**: >= 95% match against `.expected.json` (per-rule AND overall)
- **Fix accuracy**: >= 95% of known-bad fixtures produce zero remaining violations
  after `Engine::fix` (per-rule AND overall)
- **Prose precision**: Zero diagnostics on the prose corpus
