<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: CC-BY-4.0
-->

# CAPCO corpus-derived priors

Build-time inputs for the Phase-D decoder. A single
`priors.json` file lives here once the generator has been run against a
real CAPCO corpus; `crates/capco/build.rs` reads it at compile time and
emits `&'static` Rust tables into `OUT_DIR/priors.rs` (included via
`crates/capco/src/priors.rs`). No runtime JSON parsing, no runtime
`serde_json` dependency — the decoder reads plain Rust const tables
(Constitution II, SC-008).

## What lives here

| File | Role | Committed? |
|---|---|---|
| `priors.json` | Corpus-derived token base rates, template base rates, strict-context priors | Yes (build input) |
| `README.md` | This file | Yes |

`priors.json` is absent until the first successful run of the generator
against a usable corpus. The expected shape is documented below; the
build script treats the file as authoritative and fails closed on
malformed input — but only once the file exists (see §"Phase 1 / Phase
4 deferral" below).

## Regenerating the priors

Priors are produced by the Python analysis tool:

```bash
MARQUE_ENRON_CORPUS=/path/to/enron \
  python3 tools/corpus-analysis/analyze.py \
    --mode priors \
    --output crates/capco/corpus/priors.json
```

The generator reads high-confidence CAPCO markings from the Enron corpus
(author-supplied, not committed to this repo — see
`tests/corpus/CORPUS_PROVENANCE.md`) and emits token/template base rates
plus strict-context priors. Re-run whenever the corpus contents change
or the decoder's scoring shape changes.

## JSON schema

```json
{
  "schema_version": "marque-priors-1",
  "generated_at": "2026-04-21T08:00:00Z",
  "corpus_fingerprint": "blake3:…",
  "token_base_rates": {
    "SECRET":   { "count": 12345, "log_prior": -2.14 },
    "NOFORN":   { "count":  4567, "log_prior": -3.21 }
  },
  "template_base_rates": {
    "classification":                        { "count": 20000, "log_prior": -1.10 },
    "classification//dissem":                { "count":  8000, "log_prior": -2.00 },
    "classification//sci-block//dissem":     { "count":   400, "log_prior": -4.10 }
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
  mismatched shape.
- `corpus_fingerprint` — BLAKE3 of the corpus input that produced this
  file. Not load-bearing at build time; used by downstream analysts who
  want to correlate a priors file back to a specific corpus snapshot.
- `token_base_rates` — one entry per canonical token. `count` is the
  raw occurrence count; `log_prior` is the precomputed log-prior the
  decoder uses at scoring time (saves a per-query `ln()`). `log_prior`
  is `f64` in JSON and gets downcast to `f32` when baked into the
  `&'static` table (foundational-plan line 739-757).
- `template_base_rates` — one entry per grammar template shape the
  generator observed. Keys are template identifiers matching the
  `GrammarTemplate` surface the decoder consumes (T059).
- `strict_context_priors` — scalar floors used by FR-011. Each floor is
  the probability that a classification token at that level in one
  portion of a document implies other portions share at least that
  floor.

Any field beyond this set is ignored (forward-compatible with future
generator additions). A field in this set missing or malformed is a
build failure.

## Phase 1 / Phase 4 deferral

Phase 1 created this directory and this README but **does not** yet
commit `priors.json`. The `build.rs` codepath that reads this artifact
(task T004a in `specs/004-constraints-decoder-vocab/tasks.md`) has been
deferred to Phase 4 alongside T042 — the task that produces the JSON
from the Enron corpus. Landing T004a in Phase 1 as originally written
would break every clean checkout until someone with corpus access ran
the generator. Deferring keeps the scaffolding non-destructive.

Until T042 lands, the decoder doesn't exist yet and nothing needs the
priors. After T042, `priors.json` is committed in the same commit that
lands T004a's `build.rs` changes, and the fails-closed contract takes
effect.

## Invariants

- Contents are reproducible given the same corpus input and generator
  version — not cryptographically, but mechanically (deterministic
  generator).
- Contents are treated as a build input, not as runtime data. A change
  to `priors.json` is a recompile (build.rs re-runs when this file
  changes; the build script emits a `cargo:rerun-if-changed` directive
  at T004a).
- Content-ignorance applies transitively: `priors.json` contains only
  token frequencies and log-priors, never document-level text fragments
  from the source corpus (Constitution V).
