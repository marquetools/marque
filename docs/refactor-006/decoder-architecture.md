<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Decoder architecture (engine internal)

This document captures architectural decisions and rationale that
influenced the current shape of `crates/engine/src/decoder/`. It is
informational — for the authoritative on-the-fly behavior, read the
code (`crates/engine/src/decoder/mod.rs` and its sub-modules). When
this document and the code diverge, the code is the source of truth.

The document is keyed by surviving code symbols, not by line numbers
(line numbers drift immediately after a refactor).

## Module overview

The decoder is the deep-scan half of the strict/deep-scan recognizer
split. When the engine is configured for deep-scan mode (batch
reconciliation, rule-escalated region, `--deep-scan` CLI flag) and
the strict recognizer (`crate::recognizer::StrictRecognizer`) returns
zero candidates for a marking region, the engine falls back to
`decoder::DecoderRecognizer` to recover mangled markings that are
one of a small set of canonical-shape deviations away from a real
CAPCO-2016 marking.

The default `StrictOrDecoderRecognizer` (in `decoder/dispatcher.rs`)
runs the strict path first and falls back to the decoder when the
strict parse is empty or incomplete. Callers that need strict-only
dispatch (the SC-001 interactive-latency benchmark, tests asserting
strict behavior) install `StrictRecognizer` explicitly via
`Engine::with_recognizer`.

## Crate-placement rationale

`marque-capco` may not depend on `marque-core` (Constitution VII), but
the decoder needs `marque-core`'s fuzzy-vocab matcher and strict
parser to materialize candidates. `marque-engine` is the sole crate
where both chains converge — making it the only valid home for the
decoder.

The original tasks.md placement (T059/T061) was amended at PR-2
landing time when the dependency-graph implication became clear.
Future contributors who consider moving the decoder elsewhere should
re-verify the crate-graph constraint first; it has not loosened.

## Scoring approach (foundational-plan §5.2)

The doc-comments on `UNAMBIGUOUS_LOG_MARGIN` and
`NULL_HYPOTHESIS_LOG_MARGIN` in `decoder/mod.rs` cross-reference back
to this section and the next one; tune the corpus-derived rationale
here.

For each candidate the decoder computes:

```text
log_posterior(candidate | observed)
  = log_prior(candidate)                      // baked corpus priors
  + Σ log_likelihood(feature | candidate)     // enumerated scored features
```

The decoder scores the candidate-shape features it records from the
closed `marque_rules::confidence::FeatureId` enum:
`EditDistance1`, `EditDistance2`, `TokenReorder`, `SupersededToken`,
and `BaseRateCommonMarking`. Each contributes a fixed log-odds delta
documented at the feature's call site (today: in
`decoder/normalize.rs`, `decoder/candidates.rs`, and the recovery
sub-modules).

`FeatureId::StrictContextClassification` is part of the audit-schema
enum but is **not** currently a scored-feature term: classification-
level context is enforced through the separate
`ParseContext::classification_floor` hard filter (FR-011), which
rejects below-floor candidates before scoring rather than adding a
likelihood term to the posterior. `FeatureId::CorpusOverrideInEffect`
is reserved for the corpus-override wiring; the decoder does not emit
it today. Turning either into an actual scored contributor requires a
coordinated audit-schema bump (`MARQUE_AUDIT_SCHEMA`) per
`marque-rules/src/confidence.rs` doc.

The top candidate wins when its posterior exceeds the runner-up by a
configured ratio (`UNAMBIGUOUS_LOG_MARGIN = 1.6` in `decoder/mod.rs`
— natural-log odds, ≈ 5× odds ratio `P(top)/P(runner_up)` — not a
probability ratio); below that threshold
the decoder returns `Parsed::Ambiguous { candidates }` so the engine
can surface a diagnostic rather than auto-apply a close call.
`Candidate::prior_log_odds` carries the prior alone (sum of token
log-priors); the per-feature log-odds deltas live only in
`Candidate::evidence[i].log_odds`. A resolver that reconstructs
`prior_log_odds + Σ evidence.log_odds` recovers the decoder's
internal posterior exactly, without double-counting.

## Null-hypothesis dispatch (issues #258 and #472)

The decoder dispatches a candidate to the rule layer only when its
marking-side posterior beats the prose-side null hypothesis by at
least `NULL_HYPOTHESIS_LOG_MARGIN = 2.5` (in `decoder/mod.rs`). The
prose-prior computation, the line-position / bullet-anchor /
lowercase-context feature extractor, and the constants that tune all
three live in `decoder/null_hypothesis.rs`.

Before issue #472, the null prior was summed over canonical tokens
(post fuzzy correction), which silently shifted null-side mass when
fuzzy correction landed on a rare CAPCO token (e.g., observed
`(CMS)` → canonical `CTS`). The current path
(`observed_prose_log_prior`, landed for #472) walks the original
`bytes` slice to produce a bag of *observed* tokens, restoring the
symmetric marking-vs-prose comparison.

The `NULL_HYPOTHESIS_LOG_MARGIN` magnitude of `2.5` (e^2.5 ≈ 12.2×)
is the smallest margin that suppresses the SC-003a Federalist `(s)`
regression at its measured marking-vs-null delta of +2.21 (`S`:
marking `-3.28`, prose `-5.49`). `(c)` at `+1.08` and most other
single-letter portions are rejected at the same threshold by
construction. `(u)` at `+2.86` survives this margin — a lowercase
`(u)` mid-prose canonicalizing to UNCLASSIFIED is the residual
false-positive surface; it has not been observed in the test corpus,
and the prose-glue heuristic (`preceded_by_whitespace = false`)
suppresses the much more common `letter(s)` / `function(c)` cases
independently.

The recognizer applies this margin to every `MarkingType::Portion`
input EXCEPT:

- portions containing `//` (`has_double_slash`) — the category
  separator is a marking-grammar signal English prose does not
  produce, so the marking interpretation is the only plausible
  reading;
- portions whose inner content is exactly a canonical classification
  token (`is_bare_classification_shape`): the whitelist is `(U)`,
  `(C)`, `(S)`, `(TS)`, `(R)` plus the NATO abbreviations `(NU)`,
  `(NR)`, `(NC)`, `(NS)`, `(CTS)`. Pinning a positive margin on
  these would reject legitimate NATO/IC abbreviation recovery (NU
  at marking `-8.43`, prose `-8.34`, delta `-0.09`; NC at marking
  `-8.43`, prose `-5.89`, delta `-2.54`) where the marking stratum
  has zero examples but the strict grammar still recognizes the
  token.

Banner and CAB shapes bypass the filter entirely. Multi-letter
Portion candidates outside the classification whitelist (e.g.,
`(SI)`, `(HCS)`) ARE subject to the margin.

## What the decoder is NOT

- **Not a full template-matching grammar engine.** The decoder
  materializes candidates by canonicalizing observed tokens and
  round-tripping through the strict parser — the strict parser is
  the arbiter of "is this a CAPCO-shape marking." If the canonicalized
  bytes strict-parse, we have a candidate; if not, we discard.
- **Not a learning system.** All priors are compile-time-baked
  `&'static` tables from `marque_capco::priors` (Constitution III:
  no runtime corpus override on WASM).
- **Not a fix applier.** The decoder proposes `CapcoMarking`
  candidates; the engine applies them through the normal
  `Diagnostic` / `FixProposal` path with
  `FixSource::DecoderPosterior` (or
  `FixSource::DecoderClassificationHeuristic` for heuristic-source
  candidates which the engine downgrades to `Severity::Warn` and
  caps `Confidence::rule` at 0.80).

## Retired mechanisms

### `LENIENT_REL_PREFIX_PENALTY` (removed)

Under the current architecture, `try_rel_to_structural_repair` in
`decoder/recovery/rel_to.rs` runs as preprocessing on the normalized
text before any candidate is emitted, so `RELT O ` / `REL OT `
patterns at a token boundary are rewritten to canonical `REL TO `
before scoring sees them. The defense-in-depth scorer penalty
originally introduced to break a tie between competing raw vs.
repaired *candidates* no longer makes sense — the repair is not a
separate candidate.

Re-introducing a similar mechanism would be redundant under the
current preprocessing-shape. If a new defense-in-depth penalty is
ever wanted, the design must first establish that the preprocessing
model is being abandoned.

The accuracy harness (`resolution_rate_at_0_85`,
`resolution_rate_does_not_regress`, per-class floors) is the
load-bearing regression gate for this recovery path. Issue #186
(REL TO trigraph corpus-weighted recovery) is the followup that
handles the remaining lenient-header cases via priors rather than
scorer penalties.

## Sub-module map

| Sub-module | Responsibility |
|-----------|---------------|
| `mod.rs` | Re-export hub + cross-file constants (`K_MAX_CANDIDATES`, `UNAMBIGUOUS_LOG_MARGIN`, `NULL_HYPOTHESIS_LOG_MARGIN`) + small cross-cutting tests (Send/Sync). Per-sub-file tests live alongside their owning sub-module; oversized groups are pulled in via `#[path = "tests/..."] #[cfg(test)] mod tests;`. |
| `types.rs` | `ScoredCandidate`, `FeatureEntry`, `CanonicalAttempt`, `feature_entry_to_evidence` |
| `shape.rs` | Shape predicates — `infer_marking_type`, `is_cab_head`, fast-path helpers, `is_nontrivial_marking` (pub), `strict_parse_is_complete` |
| `candidates.rs` | `generate_candidate_bytes` (master orchestrator) + `diagnostic_canonical_attempts` (`decoder-harness`-gated) |
| `normalize.rs` | `normalize_delimiters_and_case`, `fuzzy_correct_tokens`, `SUPERSEDED_TOKEN_MAP` |
| `heuristic.rs` | Position-aware classification heuristic (1/2/3-char keyboard-proximity table) |
| `scoring.rs` | `score_candidate` + structural penalties (`MISSING_TOKEN_LOG_PRIOR`, `HARD_SPLITTER_ABSORPTION_PENALTY`, `CUSTOM_SCI_MARKING_PENALTY`) + `is_hard_splitter` |
| `null_hypothesis.rs` | `observed_prose_log_prior`, `compute_context_features`, position/bullet/lowercase constants |
| `recognizer.rs` | `DecoderRecognizer` struct + `impl Recognizer<CapcoScheme>` body |
| `dispatcher.rs` | `StrictOrDecoderRecognizer` struct + `impl Recognizer<CapcoScheme>` body |
| `recovery/delimiter.rs` | Missing-`//` delimiter insertion |
| `recovery/sar.rs` | SAR indicator-keyword structural repair (§H.5 p100) |
| `recovery/stray.rs` | Stray-character `/X/` recovery |
| `recovery/rel_to.rs` | REL TO structural repair (header + entry normalization, §H.8 grammar) |
| `recovery/rel_to_trigraph.rs` | REL TO trigraph fuzzy expansion + USA injection (corpus-weighted priors) |
| `recovery/sci.rs` | SCI delimiter recovery (`HCSP` → `HCS-P` shape) |
| `recovery/nato.rs` | NATO longhand → portion fold (`NATO SECRET` → `NS`) per §G.1 Table 4 |
| `recovery/reorder.rs` | Canonical reorder + non-US prefix injection + `meets_classification_floor` |

## Adding a new recovery pass

The decoder's recovery pipeline is a series of preprocessing /
candidate-emitting transforms applied inside
`decoder::candidates::generate_candidate_bytes`. To add a new pass
(say, for a future CAPCO supplement that introduces 'PERSONA-' SAR
identifiers):

1. Add `decoder/recovery/persona.rs` mirroring the existing recovery
   files.
2. Add the new function's `pub(in crate::decoder) use` re-export to
   `decoder/recovery/mod.rs`. Use `pub(in crate::decoder)` —
   visibility precisely scoped to the `decoder/` subtree;
   `pub(super)` would only reach `recovery/`'s parent and won't
   satisfy the re-export consumed by `decoder/candidates.rs`.
3. Add the corresponding `if let Some(persona_repaired) = ...` block
   to `generate_candidate_bytes` in `decoder/candidates.rs` between
   the existing recovery dispatches.
4. Add tests for the new pass.

   **Default**: a co-located `#[cfg(test)] mod tests { use super::*; }`
   block inside `recovery/persona.rs`. Six of the seven existing
   recovery sub-files follow this shape (`sar.rs`, `stray.rs`,
   `nato.rs`, `reorder.rs`, `sci.rs`, `delimiter.rs`).

   **Escape valve** when the combined source-plus-tests file approaches
   the 800-line gate: externalize the test body to
   `crates/engine/src/decoder/tests/persona_recovery_tests.rs` and pull
   it in from the source file with:

   ```rust
   #[path = "../tests/persona_recovery_tests.rs"]
   #[cfg(test)]
   #[cfg_attr(coverage_nightly, coverage(off))]
   #[allow(unused_imports)]
   mod tests;
   ```

   The `#[cfg_attr(coverage_nightly, coverage(off))]` line keeps the
   test bodies out of coverage measurement so they don't inflate the
   production-code denominator. Today only `recovery/rel_to.rs` uses
   the externalized form; `recovery/rel_to_trigraph.rs` carries no
   co-located tests at present and exercises through corpus fixtures
   instead.
5. Add a fixture to
   `crates/engine/tests/decoder_split_byte_identity.rs` if the new
   pass exercises a corpus pattern.

Three small files touched for the default pattern (the new file,
`recovery/mod.rs`, `candidates.rs`); the escape-valve path adds a
fourth (`decoder/tests/<persona>_recovery_tests.rs`). The existing
recovery files don't move; the existing tests don't move.

### REL TO recovery — historical archaeology

This section preserves decision-archaeology for the REL TO recovery passes
relocated from `recovery/rel_to.rs` inline comments in issue #718 (split at the
structural / trigraph seam). Citations are intact per Constitution VIII.

**Deferred #186 note** (formerly `rel_to.rs` lines 30-34):

The riskier per-trigraph fuzzy-correction cluster (e.g., `USB → USA`, `AUT →
AUS`) was deferred from the original structural-repair implementation because it
requires corpus-weighted priors + block-level CAPCO §H.8 invariants to
disambiguate safely. Issue #186 is the tracking issue. The fuzzy / prior-weighted
trigraph correction cluster now lives in the sibling `rel_to_trigraph.rs`, which
was split from `rel_to.rs` at the line-443 seam in issue #718.

**Trigraph dedup rationale** (formerly `rel_to.rs` lines 507-525):

Drop candidates that would duplicate a trigraph already present elsewhere in this
REL TO block. CAPCO-2016 §H.8 does not state "no duplicates" as an explicit
textual prohibition — the REL TO grammar (§A.6 / §H.8 p131-150) describes a list
of country codes ordered USA-first then ascending alphabetic, which structurally
implies a set of distinct codes but does not forbid repetition in so many words.
The reason we drop duplicates here is mechanical, not citational: the bag-of-tokens
scorer happens to *reward* duplicates (each instance adds its log-prior again), so
without this filter an ambiguous typo adjacent to a popular trigraph could collapse
to "REL TO USA, USA, GBR" because USA's log-prior contribution is additive.
Emitting a duplicate-creating candidate would therefore be structurally redundant
and cause the scorer to erroneously favor it. The block's other entries are computed
by re-walking `block.split(',')` and taking the trigraph form of any 3-char
ASCII-uppercase entry that's in the CVE recognition set.

**PR-A / PR-B partition rationale** (formerly `rel_to.rs` lines 622-643):

`try_rel_to_usa_injection_candidates` complements `try_rel_to_fuzzy_trigraph_candidates`
(both in `rel_to_trigraph.rs` as of issue #718) by handling 1- and 2-char first
entries that fall below the fuzzy function's 3-char floor (`MIN_FUZZY_LEN = 3`).
`phf`-style fuzzy matching is unreliable on inputs shorter than 3 chars — a 2-char
input is edit-distance-1 from many distinct trigraphs and the mapper has no signal
to break the tie. For REL TO specifically, the §H.8 p150–151 grammar gives a
stronger signal: USA must always appear first. So when the first entry is 1- or
2-chars ASCII-uppercase, the USA-injection path emits the substitution candidate
and lets the corpus-weighted prior (issue #233) decide at score time. The partition
is shape-based (1/2-char vs. 3-char floor), not vocabulary-based. The fixture at
`tests/fixtures/mangled/typo/ad2bcfe3ac0b0765.json` (`REL TO SA, AUS, GBR` →
`REL TO USA, AUS, GBR`) is the canonical motivating case.
