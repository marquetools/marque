# Corpus Analysis Tool

Measures how often classification marking tokens appear in general (non-IC) English text. The output provides empirical base rates for the probabilistic recognition engine — specifically, P(token | not a marking) for each token in a vocabulary.

## Quick Start

```sh
# Install dependencies
pip3 install -r requirements.txt

# Run against Enron corpus (downloads ~423MB on first run, cached after)
python3 analyze.py --output output/enron-full.json

# Quick test with limited docs
python3 analyze.py --max-docs 1000 --output output/enron-sample.json

# Custom corpus
python3 analyze.py --corpus /path/to/text/files/ --output output/custom.json

# Custom token vocabulary
python3 analyze.py --tokens tokens/my-vocab.json
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
