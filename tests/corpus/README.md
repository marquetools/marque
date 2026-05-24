<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Test Corpus

Shared fixtures for marque's unit, integration, and accuracy tests.

## Directory Structure

```
tests/corpus/
  documents/   Known-good marked prose
  valid/       Known-good marking fixtures (zero expected diagnostics)
  invalid/     Known-bad marking fixtures (one or more expected diagnostics)
  prose/       Clean body prose with no markings (prose precision gate)
```

## Fixture Format

`valid/`, `invalid/`, and `prose/` fixtures are plain `.txt` files containing
raw text.

In those directories, every fixture has a sibling `.expected.json` with the
same stem, e.g.:

```
invalid/banner_abbrev.txt
invalid/banner_abbrev.expected.json
```

### `.expected.json` schema

The `rule` field is the structured 2-tuple shape
`{"scheme": "...", "predicate_id": "..."}`.

```json
{
  "diagnostics": [
    {
      "rule": { "scheme": "capco", "predicate_id": "portion.dissem.rel-to-missing-usa" },
      "span": { "start": 0, "end": 18 },
      "severity": "error"
    }
  ]
}
```

For `valid/` fixtures, `.expected.json` contains `{ "diagnostics": [] }`.

For `documents/` fixtures, source specs live under `documents/specs/*.md`,
rendered marked documents under `documents/marked/*.md`, per-document expected
JSON at `documents/<stem>.expected.json`, and aggregate structural ground truth
at `documents/ground_truth.json`.

## Naming Convention

- `invalid/<rule_id_or_scenario>.txt` — e.g., `banner_abbrev.txt`, `missing_usa_trigraph.txt`
- `valid/<scenario>.txt` — e.g., `clean_banner.txt`, `clean_portion.txt`
- `prose/<scenario>.txt` — e.g., `lorem_with_parens.txt`

## Provenance

All fixtures are synthetic. See `CORPUS_PROVENANCE.md` and `CORPUS_CONTRACT.md`.
