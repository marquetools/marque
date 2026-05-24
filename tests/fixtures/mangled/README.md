<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Mangled-marking fixtures

Labeled mangled CAPCO markings used by the decoder accuracy
harness (`cargo test -p marque-capco --features decoder-harness`) and by
the SC-004 regression gate (≥85% resolution at aggregate confidence
≥0.85 across ≥200 cases).

## Provenance

These fixtures are **generated artifacts**, not hand-crafted test data.
The generator lives at `tools/corpus-analysis/analyze.py` and is invoked
as:

```bash
python3 tools/corpus-analysis/analyze.py \
    --mode mangled \
    --corpus tests/corpus \
    --output tests/fixtures/mangled/ \
    --min-cases 200 --seed 0
```

The generator reads high-confidence CAPCO marking canonicals from
`tests/corpus/` (curated CAPCO-2016 marking fixtures committed to this
repo — see `tests/corpus/CORPUS_PROVENANCE.md`), applies one of the six
labeled mangling transforms below, and emits one JSON file per case
under the class directory that names its transform.

**Source-narrowing invariant.** `tests/corpus/` is a mixed-validity
tree: `valid/` holds canonical markings, `invalid/` holds intentional
rule-violation fixtures (`SECRET//SERCET//NOFORN`,
`SECRET//XYZZY//NOFORN`, etc.), and `prose/` holds non-marking text.
If the generator walked the whole tree, its regex would yank
canonical-*looking* shapes out of `invalid/` and treat the embedded
typos as ground truth — silently poisoning the `expected` field in
the fixture and, by extension, the SC-004 accuracy gate. To prevent
that, `generate_mangled_fixtures` resolves the corpus path through
`_resolve_canonical_source`: if `<corpus>/valid/` exists, the walk is
pinned there and the `invalid/` / `prose/` siblings are skipped.
Homogeneous corpora (no `valid/` subdir) are used unchanged, so an
Enron-style maildir still works as-is.

**Corpus choice rationale.** Chosen priors reflect the test and data output goals:
  - `crates/capco/corpus/priors.json` are derived from the Enron corpus because Enron measures how often CAPCO tokens appear in *non-IC business prose* — that's the question the priors answer (region-identification base rates).
  - Mangled fixtures answer a different question: "given a span already flagged by the Scanner as a marking candidate but failing strict parse, can the
  decoder resolve to the intended canonical?" That test wants
  representative CAPCO-2016 canonical *shapes* as the mangling source,
  which `tests/corpus/` provides directly from curated marking shapes.
  Running the generator against Enron yielded only 5 usable canonicals
  (a real-world reflection of how rare classified markings are in
  non-IC email) — far below SC-004's ≥200-case floor.
  - Valid and invalid fixtures test the null hypothesis.
  - Prose and document priors test detection accuracy and false positives/negatives.

Regenerate whenever `tests/corpus/` canonicals change, the transform
set changes, or the minimum-case count changes. The commit that
updates the fixtures MUST re-run the decoder accuracy harness and
update any per-case expectations that shifted.

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

  ## License

  All fixtures in `tests/` are licensed under the [Marque Licenso 1.0](../../../LICENSE.md). See root [REUSE.toml](../../../REUSE.toml).
