<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 8 T130 Pre-Fix HEAD Failure Proof

This document records the failures observed when the T130 regression tests
were run against the pre-fix HEAD (before the `try_nato_fold` helper was
added to `crates/engine/src/decoder.rs`). Satisfies FR-039 Rule 5
requirement.

## Test execution

Command:
```
cargo test -p marque-engine --test decoder_recovery 2>&1
```

## Results: 7 failing tests

### `nato_u_portion_folds_to_nu`

Input: `(NATO U)`

Failure:
```
thread panicked at crates/engine/tests/decoder_recovery.rs:1597:9:
pre-fix failure: `(NATO U)` should fold to `(//NU)` and decode to
NatoUnclassified (T129 — decoder NATO longhand fold)
```

Root cause: `(NATO U)` has no `//` prefix so `try_add_non_us_prefix` fires,
but `is_non_us_classification_segment("NATO U")` returns `false` because
`NATO_ABBREVS` does not contain `"U"` and the hard-coded `starts_with` checks
only cover `"NATO SECRET"`, `"NATO CONFIDENTIAL"`, `"NATO RESTRICTED"`, and
`"NATO UNCLASSIFIED"` (full words). The bare abbrev `U` is not in the list.
Decoder returns `Parsed::Ambiguous { candidates: vec![] }`.

### `nato_r_portion_folds_to_nr`

Input: `(NATO R)`

Same root cause as above — `is_non_us_classification_segment("NATO R")` returns
`false`. `NR` is also absent from the `NATO_ABBREVS` constant in that function.

### `nato_c_portion_folds_to_nc`

Input: `(NATO C)`

Same root cause. `NC` is not in `NATO_ABBREVS`.

### `nato_s_portion_folds_to_ns`

Input: `(NATO S)`

Same root cause. `is_non_us_classification_segment("NATO S")` returns `false`
because `"S"` is not in `NATO_ABBREVS` and the check does not include
`seg.starts_with("NATO S")` (only `"NATO SECRET"` is checked).

Note: `(NATO SECRET//NF)` already works via the existing
`try_canonical_reorder` → `is_non_us_classification_segment("NATO SECRET")`
path. This pre-fix test failure is specifically for the single-letter
abbreviation form.

### `nato_ts_portion_folds_to_cts`

Input: `(NATO TS)`

`"TS"` is not in `NATO_ABBREVS`. `is_non_us_classification_segment("NATO TS")`
returns `false`.

### `nato_top_secret_long_form_folds_to_cts`

Input: `(NATO TOP SECRET//NF)`

`is_non_us_classification_segment` has explicit checks for `"NATO SECRET"`,
`"NATO CONFIDENTIAL"`, `"NATO RESTRICTED"`, `"NATO UNCLASSIFIED"` but NOT
`"NATO TOP SECRET"`. The two-word `TOP SECRET` compound is not covered.

### `nato_fold_emits_superseded_token_feature`

Input: `(NATO S)`

The test expects `FeatureId::SupersededToken` in the decoder provenance.
Pre-fix, the fold doesn't exist, so the decode returns zero candidates and
`Parsed::Unambiguous` is never reached — the test panics at the destructure.

## Tests that pass pre-fix (guards and idempotent cases)

- `nato_in_rel_to_list_is_not_folded` — `(S//REL TO USA, NATO)` decodes via
  the existing US-classification path. NATO in the REL TO country list doesn't
  interfere.
- `nato_in_fgi_list_is_not_folded` — `(//FGI USA NATO C)` is handled or
  returns zero candidates; neither outcome injects `Nato` classification from a
  fold (which doesn't exist yet).
- `nato_secret_long_form_folds_to_ns` — `(NATO SECRET//NF)` already works via
  `try_canonical_reorder` + `is_non_us_classification_segment("NATO SECRET")`,
  which covers the full two-word form. This test continues to pass after the
  fold lands; the fold is idempotent on this input (it replaces `NATO SECRET`
  with `NS` in the segment, which produces the same valid result via a
  different code path).
- `already_canonical_ns_is_idempotent` — `(//NS//NF)` is canonical and the
  strict recognizer handles it; no fold needed.

## Corpus test

The `nato_longhand_portion.txt` fixture (`(//NATO S//NOFORN)\n`) produces zero
diagnostics pre-fix (decoder returns no candidates) but the expected.json
requires `R001` at span `0..18`. The corpus accuracy test
`lint_accuracy_invalid_fixtures` fails because the expected R001 is not found.

Federalist-10 prose fixture: the `precision_prose_zero_diagnostics` test
passes pre-fix because `article.txt` already covers the `(s)` case. The new
fixture adds a second independent guard for the #258 carve-out.
