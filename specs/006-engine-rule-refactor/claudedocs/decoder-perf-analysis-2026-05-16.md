# DecoderRecognizer Performance Analysis (2026-05-16)

Read-only analysis of the marque `DecoderRecognizer` hot path. No code changes.
All citations: absolute paths + line numbers.

## 1. Hot-path map

Per-candidate flow when the strict path misses:

1. `Engine::lint` per-candidate loop, builds `ParseContext` per candidate and
   calls `self.recognizer.recognize(bytes, &parse_cx)` —
   `crates/engine/src/engine.rs:815-837`.
2. `StrictOrDecoderRecognizer::recognize` clones `cx` twice (once for the
   strict inner cx, once for the decoder cx), calls strict first, dispatches —
   `crates/engine/src/decoder.rs:4587-4647`.
3. `DecoderRecognizer::recognize` —
   `crates/engine/src/decoder.rs:150-551`:
   - `generate_candidate_bytes(bytes)` materializes ≤16 `CanonicalAttempt`s
     (line 198, body at 709-1114).
   - For each attempt: `Parser::new(&CapcoTokenSet)` then `parser.parse(...)`
     (lines 210-230). **`Parser::new` is called inside the loop** even though
     the token-set is stateless.
   - `from_parsed_unchecked` (line 260) converts borrowed `ParsedAttrs` to
     owned `CanonicalAttrs` per attempt.
   - `score_candidate(attempt, marking, kind)` —
     `crates/engine/src/decoder.rs:4257-4340` — sums `token_log_prior` +
     `token_prose_log_prior` over `canonical_tokens_for(...)` plus `rel_to`
     codes.
   - `canonical_tokens_for` —
     `crates/engine/src/decoder.rs:4466-4532` — allocates a `BTreeSet<&'static
     str>` and then `tokens.into_iter().collect()` into a `Vec<&'static str>`
     per scored candidate.
   - Sort + truncate to K=8 (lines 431-432); pick top, build
     `DecoderProvenance` (line 518).
4. Token lookups are binary-search over sorted `&'static` arrays —
   `crates/capco/src/priors.rs:114, 150, 204, 233`.
5. Fuzzy correction inside `generate_candidate_bytes`:
   `FuzzyVocabMatcher::new` called twice per input
   (`crates/engine/src/decoder.rs:878, 1045`); each `correct()` allocates
   two `Vec<u8>` scratch rows (`crates/core/src/fuzzy.rs:160-161`) and
   walks the full `EXTENDED_CORRECTION_VOCAB`.

## 2. Findings table

| # | Location | Observation | Est. impact | Risk | Recommendation |
|---|----------|-------------|-------------|------|----------------|
| F1 | `decoder.rs:210-211` | `Parser::new(&CapcoTokenSet)` constructed inside `for attempt` loop; stateless. | low | low | Hoist above loop (or `LazyLock`). One construction per `recognize()` call. |
| F2 | `decoder.rs:4466-4532` | `canonical_tokens_for` returns `Vec<&'static str>` via `BTreeSet::into_iter().collect()`. Called per scored candidate (≤16). Heap alloc + log-n insertion. | **medium** | low | `SmallVec<[&'static str; 16]>` + sort + dedup-in-place, OR fold the prior summation inline (no allocation, dedup via small linear scan since N≤~10 distinct tokens per marking). |
| F3 | `decoder.rs:4315-4323` | `seen_rel_to_codes: BTreeSet<&str>` allocates a fresh tree on every `score_candidate`. `rel_to` is typically 1-5 codes. | medium | low | `SmallVec<[&str; 8]>` linear-search dedup; identical semantics, no heap. |
| F4 | `decoder.rs:1159-1183` | `normalize_delimiters_and_case`: 7-pattern `String::replace` chain + `to_ascii_uppercase` always allocates. Most inputs are already canonical so the contains-checks short-circuit but the initial `text.to_owned()` always runs. | medium | low | Return `Cow<'_, str>`; only allocate when a replacement actually changes bytes. Same approach for the uppercase pass (check `had_lowercase` first against a borrowed view). |
| F5 | `decoder.rs:1227-1348` (`fuzzy_correct_tokens`) | `String::with_capacity(text.len())` always allocated. On well-formed pass-through (most calls), the function copies every token unchanged into a fresh `String`. | medium | low | `Cow<'_, str>` short-circuit: track a "modified" bool and only build the new `String` lazily after the first replacement. |
| F6 | `decoder.rs:878 + 1045` | `FuzzyVocabMatcher::new(vocab)` constructed twice per `generate_candidate_bytes` call. Cheap (a single `&[&'static str]` field) but the `correction_vocab()` `LazyLock` deref inside it is observable. | very low | low | Already optimal: matcher is essentially zero-cost. Skip. (Anti-rec candidate.) |
| F7 | `decoder.rs:4377-4416` (`absorbs_hard_splitter_in_sar_or_sci`) | Triple-nested loop over SAR programs/compartments/sub-compartments and SCI compartments/sub-compartments; each leaf calls `is_hard_splitter(s) \|\| s.split_whitespace().any(is_hard_splitter)`. Called per scored candidate. | low-medium | low | Early-return as soon as a hit is found (already done) but the `split_whitespace + is_hard_splitter` walk over multi-word program identifiers (`BUTTER POPCORN`) is fine. Main win: only call for candidates that *have* `sar_markings.is_some()` or non-empty `sci_markings` — guard with `if attrs.sar_markings.is_some() \|\| !attrs.sci_markings.is_empty()` before the function call site at `decoder.rs:4328`. |
| F8 | `decoder.rs:4587-4647` (`StrictOrDecoderRecognizer::recognize`) | `cx.clone()` runs twice (lines 4588, 4633) even when the strict-complete branch fires and never reaches the decoder. `ParseContext` contains `Option<Arc<str>>` (`as_of`) — clone is one Arc bump but still measurable on the strict-only common case. | **medium** | low | Reorder dispatch: do the cheap `infer_marking_type` + strict-result-complete check on the borrowed `cx` first; only construct/clone the strict inner cx if the strict_evidence flag actually differs from `cx.strict_evidence`. Most call sites pass `strict_evidence=false`, so the inner clone-with-override is only needed for the decoder leg. |
| F9 | `decoder.rs:431` | `scored.sort_by(\|a, b\| b.posterior.total_cmp(&a.posterior))` after a per-element `retain` pass. Common case is K≤4. | very low | low | Already adequate. Skip. (Anti-rec.) |
| F10 | `decoder.rs:282` | `attrs.token_spans = Box::new([])` allocates an empty Box per candidate. `Box::default()` for an empty `Box<[T]>` is technically a no-op pointer, but the explicit `Box::new([])` triggers a thin-pointer construction; cheap but per-candidate. | very low | low | Use `Box::<[TokenSpan]>::default()` or a static empty slice; minor. Skip unless benchmarked. |
| F11 | `decoder.rs:884, 897, 938, 965, 989, 1049, 1075, 1104` | `let mut features = delim_features.clone();` on every attempt-emit site. `delim_features: Vec<FeatureEntry>` typically has 0-2 entries; up to 8 clones per `generate_candidate_bytes` call. | low | low | Convert `delim_features` to `SmallVec<[FeatureEntry; 4]>` so the clones stay inline (no heap). Each clone already small, but stack copies are strictly cheaper. |
| F12 | `decoder.rs:1159, 3660, 3667-3669` etc. | `format!()` and `String::from_utf8` not used on the hot path — the `format!("{prefix}...{suffix}")` in `try_canonical_reorder` runs only on a successful reorder, off the common path. | n/a | n/a | No action. |
| F13 | `decoder.rs:225-360` | `SmallVec<[ScoredCandidate; 4]>` with `ScoredCandidate` ~200 bytes ⇒ ~800 B inline. Inline 4 is right; spillover rare. | n/a | n/a | Already tuned. Skip. |

## 3. Top 3 prioritized wins

### Win A — `canonical_tokens_for`: drop `BTreeSet`+`Vec`, fold into score (F2)

`canonical_tokens_for` allocates a `BTreeSet` and a `Vec` per scored candidate
(up to 16/call) to enumerate canonical tokens. Every consumer in
`score_candidate` walks the result once. Sort/dedup is only needed because
`dissem_iter()` may yield duplicates — but the prior is additive over distinct
tokens.

Code shape:

```rust
// Replace canonical_tokens_for + the for-loop in score_candidate with a
// fold that pushes (prior, null_prior) deltas directly, using a small
// inline dedup buffer.
let mut seen: SmallVec<[&'static str; 16]> = SmallVec::new();
let mut push_token = |tok: &'static str, prior: &mut f32, null: &mut f32| {
    if !seen.iter().any(|&t| t == tok) {
        seen.push(tok);
        *prior += marque_capco::priors::token_log_prior(tok)
            .unwrap_or(MISSING_TOKEN_LOG_PRIOR);
        *null  += marque_capco::priors::token_prose_log_prior(tok)
            .unwrap_or(marque_capco::priors::MISSING_PROSE_LOG_PRIOR);
    }
};
// classification token, sci_controls, dissem_iter, non_ic_dissem, AEA, FGI...
```

Expected wins: kill 1-2 heap allocations per scored candidate (B-tree node
allocations are the bigger half). N is small (≤~10 distinct tokens), so the
linear-search dedup is cache-friendly and cheaper than `BTreeSet` insertion.
**No audit-record content change** — same tokens hit the prior sum, same
floats result.

### Win B — `normalize_delimiters_and_case` returns `Cow<'_, str>` (F4) + `fuzzy_correct_tokens` returns `Cow` (F5)

Both functions unconditionally allocate `String`s even when the input is
already canonical. On clean input, `normalize_delimiters_and_case` runs
`text.to_owned()` then 7 `String::contains` (which short-circuit), and
`fuzzy_correct_tokens` walks the text token-by-token copying each unchanged
slice into a fresh capacity-pre-sized `String`.

Code shape:

```rust
fn normalize_delimiters_and_case(text: &str) -> (Cow<'_, str>, SmallVec<[FeatureEntry; 2]>) {
    let need_delim = REPLACEMENTS.iter().any(|(f, _)| text.contains(f));
    let need_case  = text.bytes().any(|b| b.is_ascii_lowercase());
    if !need_delim && !need_case {
        return (Cow::Borrowed(text), SmallVec::new()); // common path, zero alloc
    }
    // ... existing allocating path, wrapped in Cow::Owned
}
```

Same shape for `fuzzy_correct_tokens`: walk-and-count first; if every token
canonicalizes unchanged, return `Cow::Borrowed(text)`.

Expected wins: on already-canonical inputs that the dispatcher still routes
through the decoder fallback (decoder fires because `strict_parse_is_complete`
returned false for an *unrelated* reason — e.g., one unknown token in a
mostly-clean banner), the "raw emit" allocates nothing extra. Strict-recovers
inputs that touch the decoder twice via dispatch retries see compounding wins.
**No semantic change** — features only emit when the input actually required
cleanup, matching today's behavior.

### Win C — Reorder `StrictOrDecoderRecognizer::recognize` to avoid the two `cx.clone()` on the strict-complete fast path (F8)

The dispatcher pays two `ParseContext::clone` (each one bumps an `Arc<str>`
refcount) per candidate even when the strict path returns a complete result.
Real-world docs are ≥99% strict-complete portions; the clone tax compounds
linearly with document size — the user-reported "persistent slowdowns as
document size scales up" matches this signal closely.

Code shape:

```rust
fn recognize(&self, bytes: &[u8], cx: &ParseContext) -> Parsed<CapcoMarking> {
    // Try strict with the caller's cx unmodified when possible: the strict
    // recognizer ignores strict_evidence anyway (see StrictRecognizer impl).
    let strict_result = self.strict.recognize(bytes, cx);
    if cx.strict_evidence { return strict_result; }

    let Some(kind) = infer_marking_type(bytes) else { return strict_result; };
    if matches!(&strict_result, Parsed::Unambiguous(m) if strict_parse_is_complete(m, kind)) {
        return strict_result; // hot path: 0 clones
    }
    if matches!(&strict_result, Parsed::Ambiguous { candidates } if !candidates.is_empty()) {
        return strict_result;
    }
    // Cold path: only clone for the decoder leg.
    let mut decoder_cx = cx.clone();
    decoder_cx.strict_evidence = false;
    match self.decoder.recognize(bytes, &decoder_cx) {
        ok @ Parsed::Unambiguous(_) => ok,
        _ => strict_result,
    }
}
```

`StrictRecognizer::recognize` already ignores `strict_evidence` per its impl
(`crates/engine/src/recognizer.rs:69-118`), so passing the unmodified `cx`
through is semantically identical.

Expected wins: removes 2× `ParseContext::clone` (each ~1 Arc clone on the
`as_of` field) per candidate on the common path. Most candidates are
strict-complete portions; this is dead weight on every one of them. Likely the
single largest contributor to the size-scaling slowdown.

## 4. Anti-recommendations (things that look like wins but aren't)

- **Switch priors lookup to `phf` / `ahash`** — `TOKEN_BASE_RATES` etc. are
  sorted `&'static [TokenPrior]`. Binary search over ~hundreds of entries is
  already cache-friendly and branch-predictor-friendly; `phf` would compile
  but is unlikely to beat binary search on tables this small. The CI test
  `tables_are_sorted_by_name` (`crates/capco/src/priors.rs:296`) is the
  invariant that makes the current implementation already optimal.
- **`Parser::new` hoisting (F1)** — looks like a hot-loop win but `Parser::new`
  just wraps a `&dyn TokenSet`. The structure is essentially a fat pointer;
  the construction cost is in the noise. Hoisting is a readability nit, not a
  perf win. Skip unless benchmark says otherwise.
- **SIMD-fy the log-posterior sum loop** — `score_candidate`'s `for token in
  tokens { prior += ...; null_prior += ...; }` looks vector-able, but the
  loop body is *2 binary searches + 2 conditional sums* per token. The
  serial dependency chain through the binary searches dominates; SIMD has no
  data to chew on. Real gain would be to *avoid* tokens, which is what Win A
  does at a higher level.
- **Replace `to_ascii_uppercase` with `make_ascii_uppercase` in
  `normalize_delimiters_and_case`** — also looks attractive but only saves
  on the cold-path where uppercase was actually needed. Win B's `Cow` change
  subsumes this — it avoids the allocation entirely when the input is already
  uppercase.
- **`FuzzyVocabMatcher::new` cache** (F6) — the constructor just stores a
  `&[&'static str]`. There is nothing to cache. Skip.

## 5. Measurement plan

For each top-3 win, **before** ⇒ **after**:

### Win A (canonical_tokens_for fold)

- **Bench**: extend `crates/engine/benches/lint_latency.rs` with a new variant
  `decoder_deep_scan_mangled_10kb` that seeds the input with one mangled
  portion every ~500 bytes (forces decoder K=8 path). Existing `lint_10kb`
  baseline 828 µs (per memory `project_bench_baseline_staleness`) measures
  the strict-fast-path-dominant case and won't move much on Win A.
- **Expected delta**: −5 to −15 µs on `decoder_deep_scan_mangled_10kb`;
  negligible on `lint_10kb`.
- **Validation gate**: existing `score_candidate_*` unit tests
  (`crates/engine/src/decoder.rs:5232, 5287, 5338`) verify scoring invariance.

### Win B (Cow on normalize + fuzzy_correct_tokens)

- **Bench**: same `decoder_deep_scan_mangled_10kb` variant, plus a new
  `decoder_clean_input_through_fallback_10kb` that arranges for the strict
  recognizer to return zero-candidate Ambiguous on clean banners (e.g.,
  unrecognized custom SCI compartment letters), forcing decoder entry on
  otherwise-canonical text — exercises the Cow-stays-borrowed path.
- **Expected delta**: −10 to −25 µs on
  `decoder_clean_input_through_fallback_10kb`; allocator
  pressure (`MARQUE_LOG=trace`, watching `dhat` or
  `cargo flamegraph`) shows the eliminated `String` allocations.
- **Validation gate**: existing `normalize_*` and `fuzzy_correct_tokens_*`
  tests must continue to pass (`crates/engine/src/decoder.rs:4831+`).

### Win C (dispatcher clone reorder)

- **Bench**: this is the load-bearing win. Use the existing
  `lint_10kb` baseline directly — the dispatcher fires on every candidate.
  Measure `cargo bench --bench lint_latency -- lint_10kb`. Add a new
  `lint_100kb` variant to capture the linear-scaling story (SC-005) the user
  has been seeing degrade.
- **Expected delta**: −2 to −5 µs/candidate; at ~100 portion candidates per
  10KB doc that is **−200 to −500 µs at 10KB** and proportionally larger at
  100KB. This is likely the single biggest win.
- **Validation gate**: `core_error_isolation.rs` + `corpus_accuracy.rs` (both
  pin specific recognizer behavior) must stay green; the dispatcher's
  observable behavior is unchanged.

### Bench harness regression-guard

After the three wins land, refresh the `lint_10kb` baseline (the memory note
flags it as stale at 828 µs with current measurements 880-930 µs already
sitting on the 911 µs threshold). The combined three wins should leave
headroom; if not, the regression-gate threshold itself needs to move
in coordination — but baseline-refresh should land in its own PR per the
memory's "re-run via gh run rerun --failed; refresh after PR 4b" guidance.
