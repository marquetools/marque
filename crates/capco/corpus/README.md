<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# CAPCO corpus-derived priors

Build-time inputs for the decoder. `priors.json` lives here;
`crates/capco/build.rs` reads it at compile time and emits `&'static`
Rust tables into `OUT_DIR/priors.rs` (included via
`crates/capco/src/priors.rs`). No runtime JSON parsing, no runtime
`serde_json` dependency — the decoder reads plain Rust const tables
(Constitution II).

`build.rs` treats `priors.json` as authoritative and fails closed on
malformed input: a field in the schema below that is missing or
malformed is a build failure. A change to `priors.json` triggers a
recompile via a `cargo:rerun-if-changed` directive.

## What lives here

| File | Role | Committed? |
|---|---|---|
| `priors.json` | Corpus-derived token base rates, template base rates, strict-context priors | Yes (build input) |
| `README.md` | This file | Yes |

## Regenerating the priors

Priors are produced by the Python analysis tool. Issue #258 split the
corpus into two strata so the decoder can compute the per-token
"marking-y" score `log P(token|marking) − log P(token|prose)`:

- **Marking stratum** — `tests/corpus/capco/valid/` (committed banner /
  portion / CAB fixtures) plus any future mock-classified corpus
  produced by the project owner. Supplies `token_base_rates` and
  `country_code_base_rates`.
- **Prose stratum** — Enron email, CIA CREST declassified records,
  Congressional Record, GAO Reports. All four are prose-dominant per
  issue #258 owner confirmation (effectively zero portion-marking
  hits). Supplies `token_prose_base_rates` and
  `country_code_prose_base_rates`.

Default regeneration uses `tests/corpus/capco/valid/` for marking and Enron
for prose:

```bash
python3 tools/corpus-analysis/analyze.py \
    --mode priors \
    --output crates/capco/corpus/priors.json
```

Override either stratum explicitly:

```bash
python3 tools/corpus-analysis/analyze.py \
    --mode priors \
    --marking-corpus path/to/marking-corpus \
    --prose-corpus path/to/prose-corpus \
    --corpus-source crest \
    --output crates/capco/corpus/priors.json
```

Re-run whenever the corpus contents change or the decoder's scoring
shape changes.

**Marking-stratum coverage.** The marking stratum is the union of
`tests/corpus/capco/valid/` (~34 short per-rule fixtures) and
`tests/corpus/capco/documents/marked/` (40 synthetic-positive multi-page
documents derived from declassified CIA CREST prose with synthetic
CAPCO markings overlaid). The documents stratum supplies the
banner + CAB + portion-marked paragraph distributions a per-rule
fixture set cannot — full banner roll-ups, multi-portion pages, REL
TO trigraph diversity, and real document-shaped token co-occurrence.
Together they close most of the "single-digit token counts" gap that
schema-3 (issue #258) called out when only `valid/` was wired in.
Residual gaps that don't yet appear with meaningful frequency in
either stratum (SAR program identifiers, deep SCI sub-compartments,
some FGI trigraph combinations) still fall back to the
Laplace-smoothed zero-count log-prior; expanding to a larger
mock-classified corpus remains follow-up work.

## JSON schema

```json
{
  "schema_version": "capco-priors-3",
  "generated_at": "2026-04-21T08:00:00+00:00",
  "corpus_fingerprint": "sha512:…",
  "marking_corpus_fingerprint": "sha512:…",
  "prose_corpus_fingerprint": "sha512:…",
  "token_base_rates": {
    "SECRET":   { "count": 12345, "log_prior": -2.14 },
    "NOFORN":   { "count":  4567, "log_prior": -3.21 }
  },
  "token_prose_base_rates": {
    "SECRET":   { "count":     0, "log_prior": -10.5 },
    "USA":      { "count": 15488, "log_prior":  -3.09 }
  },
  "template_base_rates": {
    "classification":                        { "count": 20000, "log_prior": -1.10 },
    "classification//dissem":                { "count":  8000, "log_prior": -2.00 },
    "classification//sci-block//dissem":     { "count":   400, "log_prior": -4.10 }
  },
  "country_code_base_rates": {
    "USA": { "count": 10000, "log_prior": -1.28 },
    "GBR": { "count":  4000, "log_prior": -2.19 },
    "UZB": { "count":     5, "log_prior": -8.69 }
  },
  "country_code_prose_base_rates": {
    "USA": { "count": 15488, "log_prior": -1.85 },
    "GBR": { "count":   220, "log_prior": -5.93 },
    "UZB": { "count":     0, "log_prior":-10.5 }
  },
  "strict_context_priors": {
    "confidential_floor": 0.97,
    "secret_floor":       0.99,
    "top_secret_floor":   0.995
  }
}
```

Field contract (what `build.rs` expects):

- `schema_version` — opaque string; bumped when the shape changes.
  `build.rs` refuses an unknown version rather than silently parsing a
  mismatched shape. The current version is `capco-priors-3` (issue
  #258 added `token_prose_base_rates` and
  `country_code_prose_base_rates`; #233 added the marking-side
  `country_code_base_rates`).
- `corpus_fingerprint` — SHA-512 fingerprint of the combined corpus
  input that produced this file, encoded as `sha512:<hex>`. Computed
  over file metadata only (relative path, size, mtime) — never over
  file contents — so the priors artifact does not accrete document
  bytes from the source corpus (content-ignorance, Constitution V).
  SHA-512 is chosen because fingerprinting is a one-time build step,
  not a runtime path; a faster algorithm would buy nothing here. Not
  load-bearing at build time; used by downstream analysts who want to
  correlate a priors file back to a specific corpus snapshot.
- `marking_corpus_fingerprint` / `prose_corpus_fingerprint` —
  per-stratum fingerprints (issue #258). Optional; ignored by
  `build.rs`. Useful for forensics — e.g., confirming a priors
  artifact was generated against a specific marking-corpus snapshot.
- `token_base_rates` — one entry per canonical token from the **marking
  stratum**. `count` is the raw occurrence count; `log_prior` is the
  precomputed log-prior the decoder uses at scoring time (saves a
  per-query `ln()`). `log_prior` is `f64` in JSON and gets downcast to
  `f32` when baked into the `&'static` table (foundational-plan line
  739-757). Pre-#258 this was a mixture distribution because the
  analyzer aggregated all sources into one global counter; the schema-3
  split makes this a clean `P(token | marking)`.
- `token_prose_base_rates` — same shape as `token_base_rates`, derived
  from the **prose stratum** only (issue #258). The decoder consumes
  this in parallel with `token_base_rates` to compute the per-token
  marking-y score `log P(token|marking) − log P(token|prose)`. Without
  this signal, the decoder's candidate set never includes a "this is
  prose, not a marking" hypothesis and saturates at
  `SOLO_RECOGNITION = 0.999999` for any single-CAPCO-candidate input.
- `template_base_rates` — one entry per grammar template shape the
  generator observed in the **marking stratum**. Keys are template
  identifiers matching the `GrammarTemplate` surface the decoder
  consumes. Templates by definition only appear in
  marking-bearing material, so there is no prose-stratum counterpart.
- `country_code_base_rates` — one entry per CAPCO country code the
  priors pipeline counted in REL TO blocks in the **marking stratum**,
  summed with the `_REL_TO_COUNTRY_CODE_BASELINE` heuristic (issue
  #233). Same shape as `token_base_rates`: `{count, log_prior}`. The
  table covers every CAPCO country-code shape — 2-char codes (e.g.,
  `EU`), 3-char trigraphs (`USA`, `GBR`, `AUS`, …), 4-char
  tetragraphs (`FVEY`, `ACGU`, `NATO`, …), and group codes — not just
  trigraphs. The decoder consumes this so REL TO fuzzy candidates are
  weighted by real-world frequency rather than collapsing to
  `MISSING_TOKEN_LOG_PRIOR` for everything. The baseline encodes
  order-of-magnitude FVEY-vs-rare ratios because the marking stratum
  contains few real REL TO blocks; see `_REL_TO_COUNTRY_CODE_BASELINE`
  in `tools/corpus-analysis/analyze.py` for the rationale and citation
  to CAPCO-2016 §H.8.
- `country_code_prose_base_rates` — same shape as
  `country_code_base_rates`, derived from the **prose stratum** only
  with no `_REL_TO_COUNTRY_CODE_BASELINE` mixin (issue #258). The
  baseline encodes marking-side frequency ratios that would corrupt
  the prose-side signal. Standalone "(USA)" in prose (proper-noun
  country mention) is exactly the case the decoder pushes back
  against; the prose-side log-prior for USA must be high enough that
  an isolated REL-TO-style mention in prose does not auto-fix.
- `strict_context_priors` — scalar probability floors. Each floor
  is the probability that a classification token at that level in one
  portion of a document implies other portions share at least that
  floor. Stratum-independent; heuristic defaults pending corpus
  refinement.

Any field beyond this set is ignored (forward-compatible with future
generator additions). A field in this set missing or malformed is a
build failure.

## Invariants

- Contents are reproducible given the same corpus input and generator
  version — not cryptographically, but mechanically (deterministic
  generator).
- Contents are treated as a build input, not as runtime data. A change
  to `priors.json` is a recompile.
- Content-ignorance applies transitively: `priors.json` contains only
  token frequencies and log-priors, never document-level text fragments
  from the source corpus (Constitution V).

  ## License

  `priors.json` is licensed under the `Marque License 1.0`.
