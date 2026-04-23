<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Typos Evaluation and Fuzzy Vocabulary Matcher

**Date:** 2026-04-23
**Status:** implemented — `marque-core::fuzzy` shipped; Phase D integration queued.
**Issue:** #94 — Evaluate and adapt crate-ci/typos for typo correction in markings
**Relates to:** `2026-04-16-probabilistic-recognition.md` §3 (Layer 3 token resolution)

---

## 1. What is `typos` and why is it so good?

[`crate-ci/typos`](https://github.com/crate-ci/typos) is a source-code
spellchecker used in CI pipelines for ~26,000 projects. The key crates:

| Crate | Role |
|---|---|
| `typos` (library) | Core: `Dictionary` trait, `check_bytes`/`check_str`, `Typo` type |
| `typos-dict` | Pre-built phf map: `InsensitiveStr → &[&str]` (corrections) |
| `typos-cli` | CLI binary wrapping the library |

The library API is minimal and clean:

```rust
pub trait Dictionary: Send + Sync {
    fn correct_ident<'s>(&'s self, ident: Identifier<'_>) -> Option<Status<'s>>;
    fn correct_word<'s>(&'s self, word: Word<'_>) -> Option<Status<'s>>;
}
pub enum Status<'c> {
    Valid,
    Invalid,
    Corrections(Vec<Cow<'c, str>>),
}
pub fn check_bytes(buf: &[u8], tokenizer: &Tokenizer, dict: &dyn Dictionary)
    -> impl Iterator<Item = Typo>;
```

### What makes typos low false-positive?

1. **Pre-built closed-world dictionary.** `typos-dict` was built by scanning
   26K+ repositories and cataloguing *actual observed typos*. It only flags
   words that have been seen as typos in real code — not everything within
   edit distance of any English word. Unknown words return `None` (not
   flagged).

2. **Ambiguity suppression.** When a word matches more than one correction
   candidate, `Status::Invalid` is returned instead of `Status::Corrections`.
   No auto-fix on ambiguous targets.

3. **Word-level matching.** The tokenizer splits on identifier boundaries, not
   arbitrary substrings. `NOFORN` embedded in `NOFORNICATION` is a separate
   identifier match.

4. **Locale-variant aware.** `typos-vars` distinguishes en-US vs en-GB
   variants and respects the configured locale. "colour" is not flagged as a
   typo in en-GB mode.

5. **Token-first check.** If the full identifier (e.g., a camelCase symbol
   name) is in the dictionary, the result short-circuits before word splitting.

---

## 2. Why the `typos` library is not the right fit for marque

### Mismatch 1 — wrong vocabulary domain

`typos-dict` contains ~50,000 general English word corrections. It does not
know any CAPCO vocabulary. Running `typos` over `SERCET//NF` would:
- Flag `SERCET` → ??? (probably not in typos-dict, so NOT flagged)
- Leave `NF` → not flagged (too short, correct abbreviation in typos terms)
- Possibly flag nothing at all, defeating the purpose

The CAPCO token `NOFORN` would not appear in `typos-dict`; neither would
`DISPLAYONLY`, `WAIVED`, `IMCON`, `RSEN`, or any SCI control symbol. The
library would be silent on precisely the tokens marque cares about.

### Mismatch 2 — English words collide with CAPCO tokens

Some CAPCO tokens are real English words or abbreviations (`SECRET`,
`CONFIDENTIAL`, `RESTRICTED`, `REL`, `PR`, `RS`, `AC`, `SI`, `OC`). The
`typos` dictionary would treat them as prose and might suggest unwanted
corrections for near-miss variants. Worse, a future `typos-dict` update
could introduce a correction for one of these tokens without marque knowing.

### Mismatch 3 — dependency weight

`typos-dict` is a ~2 MB compiled data structure. Adding it as a runtime
dependency of `marque-core` (which is WASM-safe) would inflate the WASM
artifact significantly and introduce a crate-graph dependency that serves
no purpose — the dict is irrelevant to our domain.

### Mismatch 4 — no context awareness

`typos` operates on free text and cannot know whether a near-miss token is
inside a `(...)//` marking structure or in free prose. Marque's correction
value comes precisely from this context signal — a distance-1 miss next to
`//` is almost certainly a typo; the same string in open prose is almost
certainly a word.

---

## 3. What we need instead

The CAPCO vocabulary is **closed**: every valid token is enumerated in the
ODNI ISM CVE XML (`ALL_CVE_TOKENS` in `marque-ism`). At compile time we know
all ~52 non-trigraph CVE tokens. This closed-world property unlocks a much
simpler and more effective approach:

```
Unknown token → edit distance to every known token → correction or None
```

**False-positive prevention mechanisms** (same principles as `typos`, adapted
for our domain):

| Mechanism | typos | marque fuzzy |
|---|---|---|
| Closed-world "known typos only" | Pre-built dict of 50k observed typos | Vocabulary is the closed set; any unknown token that is close → candidate |
| Ambiguity suppression | `Status::Invalid` when multiple targets | Return `None` when ≥2 vocab entries are equidistant |
| Minimum length guard | No correction for 1-2 char identifiers | `MIN_FUZZY_LEN = 3`; single-char tokens excluded |
| Distance bound | Exact dict match only (no fuzzy at all!) | `MAX_EDIT_DISTANCE = 2`; distance 3+ suppressed |
| Context gating | N/A (operates on all text uniformly) | Engine applies +0.10–0.15 context factor for marking-region signals |
| Confidence scoring | Binary (flag or don't) | Continuous `[0.0, 1.0]` confidence, scales with token length and distance |

The key insight: because CAPCO vocabulary is a *closed set*, we can afford to
apply edit-distance search against *all* known tokens. With ~52 tokens,
exhaustive linear search over the vocab takes microseconds — no need for a
pre-built typo dictionary.

---

## 4. Implementation shipped

### `marque-core::fuzzy` module

```rust
pub struct FuzzyVocabMatcher { vocab: &'static [&'static str] }

pub struct FuzzyCorrection {
    pub token: &'static str,    // suggested canonical token
    pub distance: u8,           // Levenshtein distance
    pub confidence: f32,        // base confidence, before context scaling
}

impl FuzzyVocabMatcher {
    pub fn new(vocab: &'static [&'static str]) -> Self;
    pub fn correct(&self, token: &str) -> Option<FuzzyCorrection>;
}
```

**Algorithm** (inside `correct`):

1. `vocab.binary_search(token)` — if found, return `None` (valid, no correction).
2. `token.chars().count() < MIN_FUZZY_LEN` → return `None` (too short).
3. For each `candidate` in `vocab`:
   - Skip if `|len(token) - len(candidate)| > MAX_EDIT_DISTANCE` (fast filter).
   - Compute `levenshtein(token, candidate)`.
   - Track best distance + ambiguity flag.
4. If ambiguous (two candidates tie) or best distance > `MAX_EDIT_DISTANCE` → `None`.
5. Return `FuzzyCorrection { token: best, distance, confidence }`.

**Confidence formula:**

```rust
// Distance 1:  base 0.55 + 0.05 per char over 3 (capped at 6) → [0.55, 0.70]
// Distance 2:  base 0.40 + 0.05 per char over 5 (capped at 8) → [0.40, 0.55]
// Below MIN_USEFUL_CONFIDENCE (0.45): suppressed even if unambiguous
```

Callers multiply by a context factor before thresholding:
- Inside `(...)` or adjacent to `//`: ×1.10–1.15
- Open prose: ×1.0 (no boost)

### `marque-ism::token_set::TokenSet` extension

A new method was added to the `TokenSet` trait:

```rust
fn correction_vocab(&self) -> &'static [&'static str];
```

`CapcoTokenSet::correction_vocab()` returns `ALL_CVE_TOKENS` — the sorted,
deduplicated CVE token slice emitted by `build.rs`. This makes the fuzzy
matcher injectable in tests with any custom vocabulary.

---

## 5. Integration plan (Phase D)

The `FuzzyVocabMatcher` is the foundation of the Phase D probabilistic
decoder (see `2026-04-19-recursive-lattice-and-decoder.md`). The wiring path:

### Immediate (shipped)

- `marque-core::fuzzy` module public surface: `FuzzyVocabMatcher`,
  `FuzzyCorrection`, `MIN_FUZZY_LEN`, `MAX_EDIT_DISTANCE`. The
  `levenshtein` helper is `pub(crate)` (exposed for in-crate tests, not
  API) and `correction_confidence` is a private implementation detail
  of `FuzzyVocabMatcher::correct`.
- `marque-ism::TokenSet::correction_vocab()` — exposes vocab for injection.
- Tests: 17 unit tests covering distance computation, correction logic,
  ambiguity suppression, confidence scaling.

### Phase D integration (queued)

The engine's pre-scanner step (`Engine::lint`, after AhoCorasick corrections
map) should call:

```rust
// Pseudo-code — not yet wired
let matcher = FuzzyVocabMatcher::new(token_set.correction_vocab());
for candidate_region in scanner_rejected_regions {
    for unknown_token in tokenize(candidate_region) {
        if let Some(fix) = matcher.correct(unknown_token) {
            let ctx_factor = if is_marking_region(candidate_region) { 1.15 } else { 1.0 };
            let effective_confidence = fix.confidence * ctx_factor;
            // emit C001 diagnostic with confidence-gated FixProposal
        }
    }
}
```

The Phase D decoder (`Parsed<M>::Ambiguous { candidates }`) consumes multiple
`FuzzyCorrection` results and applies Bayesian log-posterior scoring per
§§4–5 of the 2026-04-19 plan to produce a single ranked candidate.

### Org-specific exact corrections still take priority

The existing `[corrections]` HashMap in `.marque.toml` feeds the AhoCorasick
pre-scanner pass (C001, `FixSource::CorrectionsMap`). That pass runs **before**
the fuzzy matcher. Org-specific aliases always override generic fuzzy guesses —
if an org configures `"SERCET" = "SECRET"`, the exact match fires at confidence
1.0 and the fuzzy pass never needs to run on that token.

---

## 6. Performance notes

| Operation | Cost | Notes |
|---|---|---|
| `FuzzyVocabMatcher::new` | O(1) | Stores a static slice reference |
| `correct(token)` on 52-entry vocab | ~2–5 µs | Linear scan, each call runs levenshtein on ≤52 pairs |
| Levenshtein on CAPCO token pair | ~50–300 ns | Input length 2–20 chars; rolling 2-row array |
| AhoCorasick pre-scanner (existing) | ~O(n) | Runs first; fuzzy only on scanner-rejected regions |

The fuzzy matcher is gated on scanner-rejected regions — it never runs on
text that the strict scanner and parser handle correctly. The fast path
(well-formed markings) is unchanged.

---

## 7. Scope of OCR noise covered

The `FuzzyVocabMatcher` addresses typo/OCR mutations of individual tokens:

| Mutation type | Example | Edit distance | Covered? |
|---|---|---|---|
| Adjacent transposition | `NOFRON` → `NOFORN` | 2 | ✓ |
| Single char deletion | `CONFIDETIAL` → `CONFIDENTIAL` | 1 | ✓ |
| Single char insertion | `SECRRET` → `SECRET` | 1 | ✓ |
| Adjacent transposition | `SERCET` → `SECRET` | 2 | ✓ |
| Two-char substitution | `SECRECT` → `SECRET` | 2 | ✓ (if unambiguous) |
| Wrong delimiter spacing | `( S )` → `(S)` | structural | ✗ (Phase D scanner) |
| Split word OCR | `S ECRET` | structural | ✗ (Phase D scanner) |
| Case folding | `secret` → `SECRET` | structural | ✗ (corrections map) |
| Trigraph typos | `FVEY` vs `FVEYS` | — | Excluded (trigraphs use dedicated sub-parser) |

Structural mutations (wrong delimiters, space-split tokens, OCR spacing) are
in scope for the Phase D scanner extensions, not the token-level fuzzy matcher.

---

## 8. Decision record: library vs. vendor vs. custom

| Option | Pros | Cons | Decision |
|---|---|---|---|
| Use `typos` as runtime library | Zero maintenance of algo code | Wrong vocabulary domain, 2 MB dict overhead, no context awareness, WASM weight | **Rejected** |
| Vendor `typos` core (no dict) | Own the tokenizer | Complex tokenizer not needed; CAPCO tokens don't benefit from word-splitting | **Rejected** |
| Custom focused matcher | Tiny, WASM-safe, closed-vocabulary, context-aware, confidence-scored | We own the code | **Adopted** ✓ |

`typos` as a *development tool* (CI spellcheck via `_typos.toml`) stays as-is —
it catches typos in comments, docs, and identifier names. It is not used at
runtime.
