# Corpus Provenance (SC-002a)

## Provenance Statement

Every fixture in this corpus is **synthetic**. No fixture was derived from,
copied from, or inspired by any classified, controlled, or sensitive document.

All markings use only **publicly documented CAPCO marking syntax** from ODNI
publications. Prose content is either Lorem Ipsum, manifestly fictional
(e.g., "Project Unicorn"), or generic technical writing.

## Sources

- **Marking syntax**: ODNI ISM Specification (ISM-v2022-DEC), publicly
  available from the ODNI website.
- **Prose filler**: Lorem Ipsum generators, fictional project names, and
  generic text. No corpus fixture contains real organizational names,
  real classifier identifiers, or real program names.
- **Country trigraphs**: Standard ISO 3166-1 alpha-3 codes as published
  in the ODNI CVE XML enumerations.

## Review

- **Reviewer**: (TBD — to be filled before `mvp-corpus-v1` tag)
- **Review date**: (TBD)
- **Scope**: All files under `tests/corpus/` were reviewed to confirm:
  1. No classified or sensitive content
  2. No real classifier identifiers or PII
  3. All marking syntax is drawn from public ODNI documentation
  4. All prose is synthetic or Lorem Ipsum

## Constraints

- No fixture may contain a real `classifier_id` value (SC-006)
- No fixture may contain token strings outside the generated CVE
  enumerations in `marque_ism::generated::values` (SC-002a)
- This document must be updated and the reviewer line filled before
  the `mvp-corpus-v1` tag is created
