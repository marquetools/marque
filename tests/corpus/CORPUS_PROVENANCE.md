<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Corpus Provenance (SC-002a)

## Provenance Statement

Every **marking** in this corpus is synthetic. No marking was derived from,
copied from, or inspired by the classification of any classified,
controlled, or sensitive document. All markings use only **publicly
documented CAPCO marking syntax** from ODNI publications.

**Prose** comes from two sources:

1. **Synthetic** — Lorem Ipsum, manifestly fictional (e.g., "Project
   Unicorn"), or generic technical writing. Used in `valid/`, `invalid/`,
   and `prose/`.
2. **Declassified public-domain text** — body prose from CIA CREST
   declassified releases (1990–2010), mirrored on Internet Archive. Used
   only in `documents/`. These documents completed the formal
   declassification review process and are public domain. Markings on
   these fixtures are still synthetic; the original markings (which were
   redacted or struck through in the declassification release) play no
   role. This dataset seeks to emulate real-world classified messages in terms of structure and vocabulary. The portions and generated banners **do not** attempt to emulate realistic markings from content - portions were randomly assigned and in most cases don't reflect what a real-world marking would be for corresponding text. Marque doesn't evaluate or even consume content, so meaning is out of scope for this corpus.

No fixture — synthetic or document-class — contains any currently
classified, controlled, or sensitive content.

## Sources

- **Marking syntax**: ODNI ISM Specification (ISM-v2022-DEC), publicly
  available from the ODNI website.
- **Synthetic prose filler**: Lorem Ipsum generators, fictional project
  names, and generic text. No synthetic fixture contains real
  organizational names, real classifier identifiers, or real program
  names.
- **Declassified prose** (`documents/` only): CIA Records Search Tool
  (CREST) releases retrieved from Internet Archive
  (https://archive.org/details/CIA-RDP*). Construction pipeline lives at
  `tools/cia-crest-corpus/`. Real organizational names, place names, and
  historical figures may appear in this prose because the source
  documents are now public domain through the declassification process.
- **Country trigraphs**: Standard ISO 3166-1 alpha-3 codes as published
  in the ODNI CVE XML enumerations.

## Review

- **Reviewer**: (TBD — to be filled before `mvp-corpus-v1` tag)
- **Review date**: (TBD)
- **Scope**: All files under `tests/corpus/` were reviewed to confirm:
  1. No classified or sensitive content
  2. No real classifier identifiers or PII
  3. All marking syntax is drawn from public ODNI documentation
  4. All prose is synthetic, Lorem Ipsum, or adapted from publicly available information.

## Constraints

- No fixture may contain a real `classifier_id` value (SC-006)
- No fixture may contain token strings outside the generated CVE
  enumerations in `marque_ism::generated::values` (SC-002a)
- `documents/` fixtures MUST lint as zero-diagnostic valid input, except where
  explicitly designated for open-CVE coverage; they may only use prose from
  documents that have completed formal declassification review and are public
  domain, and marking content overlaid on that prose remains synthetic
- This document must be updated and the reviewer line filled before
  the `mvp-corpus-v1` tag is created
