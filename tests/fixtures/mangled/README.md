<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: CC-BY-4.0
-->

# Mangled-marking fixtures

Labeled mangled CAPCO markings used by the Phase-D decoder accuracy
harness (`cargo test -p marque-capco --features decoder-harness`) and by
the SC-004 regression gate (≥85% resolution at aggregate confidence
≥0.85 across ≥200 cases).

## Provenance

These fixtures are **generated artifacts**, not hand-crafted test data.
The generator lives at `tools/corpus-analysis/analyze.py` and is invoked
as:

```bash
MARQUE_ENRON_CORPUS=/path/to/enron \
  python3 tools/corpus-analysis/analyze.py \
    --mode mangled \
    --output tests/fixtures/mangled/ \
    --min-cases 200
```

The generator reads high-confidence CAPCO markings from the Enron corpus
(author-supplied, not committed to this repo — see
`tests/corpus/CORPUS_PROVENANCE.md`), applies one of the six labeled
mangling transforms below, and emits one JSON file per case under the
class directory that names its transform. Fixtures are committed; the
Enron source artifact is not.

Regenerate whenever the Enron source, the transform set, or the
minimum-case count changes. The commit that updates the fixtures MUST
re-run the decoder accuracy harness and update any per-case expectations
that shifted.

## Six mangling classes

Each directory holds JSON files for one class. The class name is the
transform the generator applied to produce the observed form.

| Class directory | Transform | Example `observed` → `expected` |
|---|---|---|
| `typo/` | Single-character edit-distance-1 typo (swap, drop, insert, substitute) | `SERCET` → `SECRET` |
| `reordering/` | Banner tokens presented out of canonical order | `REL TO USA, GBR//SI` → `SI//REL TO USA, GBR` |
| `missing-delimiter/` | Portion or banner delimiter dropped | `S REL TO USA` → `S//REL TO USA` |
| `superseded-token/` | A token CAPCO-2016 explicitly retired in place of its live replacement | `SECRET//COMINT` → `SECRET//SI` |
| `wrong-case/` | Correct tokens, wrong case | `secret//noforn` → `SECRET//NOFORN` |
| `garbled-delimiter/` | Delimiter present but malformed (wrong glyph, spacing) | `S ∕∕ NOFORN` → `S//NOFORN` |

**Note on `superseded-token/`:** this class is narrower than it may
sound. It represents **actual CAPCO-2016 supersessions** (e.g.,
`COMINT` retired in favor of `SI`, CAPCO-2016 ~line 5136), NOT
banner/portion form pairs like `NOFORN`/`NF` — both of those are live
authorized forms per the ODNI CVE register
(`crates/ism/schemas/ISM-v2022-DEC/CVE_ISM/CVEnumISMDissem.csv`) and
would belong to a separate "wrong marking form" class. Genuine CAPCO
supersessions are rare, so this class typically produces fewer
fixtures than its siblings. Any entry added to
`SUPERSEDED_TOKEN_MAP` in `tools/corpus-analysis/analyze.py` MUST
cite a specific passage in `crates/capco/docs/CAPCO-2016.md`
(Constitution VIII).

## Fixture schema

Each `*.json` file is a single `MangledMarkingFixture` record (see
`specs/004-constraints-decoder-vocab/data-model.md`):

```json
{
  "observed": "SERCET//NOFORN",
  "expected": "SECRET//NOFORN",
  "mangling_class": "Typo",
  "source_confidence": 0.97
}
```

Field contract:

- `observed` — the mangled marking as it would appear in a document.
  `&'static str` when loaded into Rust.
- `expected` — the canonical marking the decoder SHOULD resolve to at
  confidence ≥0.85.
- `mangling_class` — one of `Typo`, `Reordering`, `MissingDelimiter`,
  `SupersededToken`, `WrongCase`, `GarbledDelimiter`. Matches the
  directory the fixture lives under.
- `source_confidence` — confidence (from the generator) that
  `(observed, expected)` is a genuine mangling pair (i.e., that the
  Enron source really did intend the expected marking). Uses `f64` in
  the JSON schema; the `FeatureContribution` and `Confidence` types the
  decoder produces are `f32` per foundational-plan line 739-757.

## Invariants (enforced by decoder accuracy harness)

- Class directory name matches `mangling_class` field in every file
  inside it.
- Total case count across the six classes is ≥200 (FR-008, SC-004).
- Distribution across the six classes is not uniform but every class
  has at least one case.
- Fixture files contain ONLY marking tokens in `observed` /
  `expected` — never surrounding document content, metadata field
  values, or subject-claim free-form text (Constitution V,
  content-ignorance invariant; mirrors the same invariant the audit
  stream enforces at T056).
