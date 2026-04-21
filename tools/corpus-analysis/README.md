# Corpus Analysis Tool

Three modes of operation over a text corpus, selected via `--mode`:

- **`baseline`** — token-frequency analysis. Measures how often
  classification marking tokens appear in general (non-IC) English text
  and in what structural contexts. Output: JSON frequency table
  (`corpus_stats`, `tokens`, `cooccurrence_pairs`, …). This is the
  original behavior and the default mode.
- **`priors`** — corpus-derived priors for the Phase-D decoder. Runs
  the baseline analysis, then reshapes into the schema consumed by
  `crates/capco/build.rs` at compile time (see
  `crates/capco/corpus/README.md`). Output: single `priors.json`.
- **`mangled`** — labeled mangled-marking fixtures for the decoder
  accuracy harness. Walks a corpus, finds high-confidence classification
  markings, applies one of six labeled mangling transforms, and emits
  one JSON file per case under `tests/fixtures/mangled/<class>/` (see
  `tests/fixtures/mangled/README.md`). Required case count matches the
  SC-004 gate (default `--min-cases 200`).

## Quick Start

```sh
# Install dependencies
pip3 install -r requirements.txt

# Baseline: token frequencies against Enron (downloads ~423MB first run)
python3 analyze.py --output output/enron-full.json

# Baseline with a limited doc count
python3 analyze.py --max-docs 1000 --output output/enron-sample.json

# Baseline against a custom corpus
python3 analyze.py --corpus /path/to/text/files/ --output output/custom.json

# Custom token vocabulary
python3 analyze.py --tokens tokens/my-vocab.json

# Priors: corpus-derived priors for the Phase-D decoder build.rs
python3 analyze.py --mode priors \
    --output ../../crates/capco/corpus/priors.json

# Mangled fixtures: produce ≥200 labeled cases for the decoder harness
MARQUE_ENRON_CORPUS=/path/to/enron \
  python3 analyze.py --mode mangled \
    --output ../../tests/fixtures/mangled/ \
    --min-cases 200 --seed 0
```

## Token Vocabulary

Token lists are JSON files in `tokens/`. The default is `tokens/capco.json` containing ~93 CAPCO classification marking tokens organized by category (classification levels, SCI controls, dissemination controls, trigraphs, etc.).

To analyze a different vocabulary (CUI, NATO, French classifications), create a new JSON file following the same schema:

```json
{
  "vocabulary": "my-vocab-v1",
  "categories": {
    "category_name": {
      "description": "What these tokens are",
      "tokens": ["TOKEN1", "TOKEN2"]
    }
  }
}
```

## Output

JSON with:
- **`corpus_stats`**: document count, word count, `//` frequency (URL vs non-URL breakdown)
- **`tokens`**: per-token raw count, per-million-words rate, document frequency, and contextual signals (after `(`, near `//`, at line start in caps, inside parentheses)
- **`cooccurrence_pairs`**: how often pairs of vocabulary tokens appear within 30 characters of each other
- **`token_categories`**: maps each token to its vocabulary category

## Key Findings (Enron, 510K docs, 134M words)

- 58 of 93 CAPCO tokens have <0.1 occurrences per million words — marking-exclusive
- `(classification//control)` has **zero** false positives for `(S//`, `(C//`, `(TS//`
- `//` outside URLs: ~1500/M words, but paired with a known token it's near-certain
- `(C)` is the one genuinely ambiguous token (copyright vs CONFIDENTIAL)
- Co-occurrence of 2+ CAPCO tokens near `//` is effectively zero in non-IC text

See `docs/plans/2026-04-16-probabilistic-recognition.md` for the full analysis and architecture design.

## Phase-D artifacts

### `priors.json` schema

See `crates/capco/corpus/README.md` for the full schema contract. In
short: `schema_version` (pinned; `build.rs` refuses unknown versions),
`token_base_rates` (count + precomputed Laplace-smoothed `log_prior`),
`template_base_rates`, and `strict_context_priors` (FR-011 floors).
Output floats are rounded to 6 decimal places for diff stability across
runs with the same corpus.

### Mangled fixture set

Six classes, one directory each. Per-fixture schema is
`{observed, expected, mangling_class, source_confidence}` — see
`tests/fixtures/mangled/README.md`. The generator is deterministic
given the same corpus and `--seed`, so committing the fixture set is
reproducible.

The generator:
- Only emits fixtures for **canonical-looking** markings it finds in
  the corpus (portions with `(CLASS//DISSEM)` or banners with
  `CLASS//…`). Bare `(C)` and other ambiguous shapes are intentionally
  skipped — they collide with copyright and aren't useful for
  accuracy training.
- Skips identity transforms (transform produces output == input).
- Deduplicates by content digest so distinct seeds run against the
  same corpus converge to the same fixture set.
- Raises if fewer than `--min-cases` fixtures materialize across the
  six classes combined.
