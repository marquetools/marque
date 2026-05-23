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
— natural-log odds, ≈ 5× probability ratio); below that threshold
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

This margin applies to single-letter portion candidates only.
Multi-letter portion candidates (`(NU)`, `(NC)`, `(NR)`, `(TS)`,
`(SI)`, ...) and banner-form candidates (`UNCLASSIFIED`,
`CONFIDENTIAL`, etc.) bypass the null filter entirely: their shapes
are long enough that English prose doesn't fabricate them by glyph
coincidence, and pinning any positive margin on them would reject
legitimate NATO/IC abbreviation recovery (NU at marking `-8.43`,
prose `-8.34`, delta `-0.09`; NC at marking `-8.43`, prose `-5.89`,
delta `-2.54`) where the marking stratum has zero examples but the
strict grammar still recognizes the token.

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
| `mod.rs` | Re-export hub + cross-file constants (`K_MAX_CANDIDATES`, `UNAMBIGUOUS_LOG_MARGIN`, `NULL_HYPOTHESIS_LOG_MARGIN`) + legacy test block (deferred per-sub-file split) |
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
| `recovery/rel_to.rs` | REL TO recovery (structural + trigraph fuzzy + USA injection) |
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
4. Add a co-located `#[cfg(test)] mod tests` to `recovery/persona.rs`.
5. Add a fixture to
   `crates/engine/tests/decoder_split_byte_identity.rs` if the new
   pass exercises a corpus pattern.

Three small files touched (the new file, recovery/mod.rs,
candidates.rs). The existing recovery files don't move; the existing
tests don't move.
