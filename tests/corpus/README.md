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
  prose/       Clean body prose with no markings (SC-003a precision gate)
```

## Fixture Format

Each fixture is a plain `.txt` file containing raw text.

Every fixture has a sibling `.expected.json` file with the same stem, e.g.:

```
invalid/banner_abbrev.txt
invalid/banner_abbrev.expected.json
```

### `.expected.json` schema

```json
{
  "diagnostics": [
    {
      "rule": "E001",
      "span": { "start": 0, "end": 18 },
      "severity": "error"
    }
  ]
}
```

For `valid/` fixtures, `.expected.json` contains `{ "diagnostics": [] }`.

## Naming Convention

- `invalid/<rule_id_or_scenario>.txt` — e.g., `banner_abbrev.txt`, `missing_usa_trigraph.txt`
- `valid/<scenario>.txt` — e.g., `clean_banner.txt`, `clean_portion.txt`
- `prose/<scenario>.txt` — e.g., `lorem_with_parens.txt`

## Provenance

All fixtures are synthetic. See `CORPUS_PROVENANCE.md` and `CORPUS_CONTRACT.md`.
