# Probabilistic Recognition Architecture

**Date**: 2026-04-16
**Status**: Design discussion, pre-implementation
**Context**: Brainstorming session following demo site repair. The demo exposed that the current binary scanner/parser leaves significant capability on the table.

---

## 1. Problem Statement

The current scanner/parser pipeline is **binary**: it either recognizes a marking candidate exactly, or the text is invisible to the engine. This creates several gaps:

- **Typos of vocabulary terms** (SERCET, CONFIDETIAL) are unrecognizable because they don't match any token. The scanner never detects them, so no rule ever fires. Today this requires a user-configured corrections map — a flat `HashMap<String, String>` that feeds an AhoCorasick pre-scanner.

- **Structural malformations** (single-slash `U/FOUO` instead of `U//FOUO`, missing leading `//` on FGI markings) are invisible because the scanner requires exact grammar.

- **Garbled but unambiguous input** (`s-fouo-rsen-sI` in a form field) can't be processed at all, even though every token resolves unambiguously within the CAPCO vocabulary.

- **Contextual recognition** is absent. The engine treats a `portion_marking` form field identically to open prose, when the form field eliminates the fundamental uncertainty ("is this a marking?") and should enable much more aggressive inference.

The current corrections map approach (config-driven, exact string match) is the right mechanism for org-specific aliases but is not a substitute for the engine understanding its own vocabulary well enough to recognize misspellings, reorder tokens, and normalize delimiters.

## 2. Core Insight: Confidence at Every Layer

Classification markings exist on a confidence spectrum. Most CAPCO symbols are **unambiguous even in isolation** (SI, TK, FOUO, RSEN, HCS-P), and **nearly all are unambiguous when adjacent to other known tokens or delimiters**. The combinatorial probability of random English text producing "SI-TK" or "SAR-BP" adjacent to `//` is astronomically low.

This means confidence can be assigned to every symbol in the classified grammar, and that confidence changes based on context:

| Context | Effect on confidence |
|---------|---------------------|
| Input is a structured `portion_marking` field | Eliminates "is it a marking?" — start at ~1.0 |
| `(` at line start followed by `S\|T\|C\|U` | Very high — lines rarely start with `(S...` |
| `//` appears outside a programming context | Very high — nearly marking-exclusive in office docs |
| Multiple known tokens adjacent, separated by any delimiter | Extremely high — coincidence probability near zero |
| Single known token in open text (e.g., `SECRET`) | Low-medium — could be prose ("it's a secret") |
| Unknown token surrounded by known tokens in a marking region | High probability of being a program codeword, compartment, or misspelling |
| Token within edit distance 1-2 of known vocab, inside a marking region | High — typo |
| Same fuzzy match in open text | Very low — could be anything |

### Worked Example

Input (form field, known to be a portion marking): `s-fouo-rsen-sI`

1. Context: form field → P(marking) = 1.0, skip region detection entirely
2. Tokenize on delimiters (`-`): `[s, fouo, rsen, sI]`
3. Token resolution against vocabulary (case-insensitive):
   - `s` → `S` (classification SECRET, abbreviated) — confidence 1.0
   - `fouo` → `FOUO` (dissem control) — confidence 1.0
   - `rsen` → `RSEN` (dissem control) — confidence 1.0
   - `sI` → `SI` (SCI control) — confidence 1.0
4. All four tokens resolve. Delimiters are wrong (`-` not `//`) but irrelevant once tokens are identified.
5. Structure matcher: S // SI // FOUO // RSEN (reordered per CAPCO §C.1)
6. Output: `S//SI//RSEN` with confidence ~1.0 (FOUO handled by E006 migration)

### Worked Example: Open Text

Input: `(U//FOUO) We would need to get approval to declassify the material because it's SECRET`

1. Region detection: `(U//FOUO)` matches marking pattern → P(marking) ~0.95
2. `SECRET` in prose: standalone, preceded by "it's" (article) → P(marking token in this position) very low → treat as prose, suggest lowercase
3. Engine auto-fixes the `(U//FOUO)` portion but leaves the prose `SECRET` alone (or surfaces a low-confidence suggestion)

## 3. Three-Layer Recognition Model

### Layer 1 — Is it a marking?

**Input**: Raw text + optional context hint from caller
**Output**: Candidate regions with P(marking)

The caller can short-circuit this entirely: a `portion_marking` form field passes P(marking) = 1.0, and this layer is skipped. For open text, the detector looks for structural signals:

- `(` followed by uppercase token and `//` or `)`
- `//` outside URL/code context
- All-caps line matching banner patterns
- Line starting with `(S|T|C|U` followed by non-alphabetic

If confidence doesn't reach ~0.90 from structural signals alone, a quick check for known tokens inside the candidate region either promotes it or kills it. The kill path is short: "parentheses but no known tokens and no contiguous known tokens → not a candidate."

**This layer is separable as its own crate** (`marque-detect` or similar) because many applications don't need it — their input is already known to be markings.

### Layer 2 — What category of marking?

**Input**: Candidate region with P(marking) ≥ threshold
**Output**: Structural template classification

Given that it probably IS a marking, what kind?

- Starts with `//` → expect non-US classification (FGI trigraph/tetragraph/JOINT)
- Starts with known trigraph/tetragraph → FGI even without leading `//` (omitting `//` is one of the most common portion marking errors)
- Starts with `U|C|S|TS` → US classification
- NATO tetragraph → expect NATO classification equivalents
- `(...)` structure → portion; line/full-caps → banner

Most template choices are **deterministic given the first 1-2 tokens**. The probabilistic path only matters for malformed input.

### Layer 3 — Token identity

**Input**: Categorized marking region, tokenized on delimiters
**Output**: Resolved tokens with per-token confidence

Given the category, each token is one of a small number of possibilities:

| Token type | Identification method | Constraints |
|------------|----------------------|-------------|
| Known CVE vocabulary | Exact match or fuzzy (edit distance ≤ 2) | ~200 terms, most 2-8 chars |
| Tetragraph/trigraph | 3-4 uppercase alpha, position-constrained | Must be valid country/org code |
| SCI compartment | ~3 characters, must follow a known SCI control (SI, TK, G, HCS-O, HCS-P) | Alphanumeric, can include numbers |
| SCI sub-compartment | ~4 characters, must follow a compartment | Alphanumeric |
| SAR program | Follows `SAR-` prefix (or close variant) | Free-form codeword |
| Unknown | Process of elimination | Confidence based on what it CAN'T be |

**Key insight about order**: If known tokens are present and no misordering of categories is detected, then positional order provides additional signal for resolving unknowns. But if categories ARE misordered (suggesting the author can't be trusted on order), then order is unreliable and resolution should rely on token identity alone.

**Upward confidence propagation**: Token-level results can revise the Layer 1 and Layer 2 assessments. A region that was marginal (P(marking) = 0.85) but resolves 4/4 tokens to known vocabulary should be promoted to near-certain. A region that looked promising structurally but resolves 0/4 tokens should be demoted or killed.

## 4. Empirical Base Rates (Corpus Analysis)

**Critical dependency**: The confidence numbers above are currently intuitions. To make them defensible, we need empirical measurements of token frequencies in non-IC English text.

### What we need to measure

For each token in the CAPCO vocabulary (and future CUI, NATO, etc. vocabularies):

- P(token appears in general English office text) — the base rate
- P(token appears adjacent to `//` in non-IC text) — should be near zero
- P(token appears inside `(...)` at line start in non-IC text) — should be near zero
- P(two or more vocabulary tokens appear within N characters of each other in non-IC text) — the combinatorial signal

### Corpus selection (control group)

The corpus must be **non-IC** to establish the background rate. Good candidates:
- Enron email corpus (public, large, office communication)
- Public government documents (regulations, reports, correspondence)
- Wikipedia English text
- Business correspondence datasets

### Tool requirements

The corpus analysis tool should be:

- **A factory, not a one-off**: Given a corpus and a token list, produce a frequency table. Today the tokens are CAPCO vocabulary. Tomorrow they're CUI controls, French classifications, or NATO markings.
- **Language-agnostic in design**: The tool doesn't need to understand what the tokens mean, just how often they appear in context.
- **Probably Python**: Token frequency analysis, corpus processing, and NLP are Python's home turf. The output is a static frequency table that Rust consumes at build time (like the ISM schemas in `build.rs`).
- **Potentially already exists**: TF-IDF, PMI, corpus frequency tools are standard NLP. We may need a thin wrapper, not a new tool.

### Output format

A build-time-consumable table, probably JSON or TOML:

```json
{
  "corpus": "enron-emails-v1",
  "token_count": 517234,
  "document_count": 500000,
  "tokens": {
    "SECRET": { "doc_freq": 0.0023, "context_freq": { "after_paren": 0.00001, "near_double_slash": 0.0 } },
    "SI": { "doc_freq": 0.0089, "context_freq": { "after_paren": 0.0, "near_double_slash": 0.0 } },
    "NOFORN": { "doc_freq": 0.0, "context_freq": { "after_paren": 0.0, "near_double_slash": 0.0 } },
    "//": { "doc_freq": 0.015, "context_freq": { "inside_parens": 0.001, "in_url": 0.014 } }
  }
}
```

The Rust build.rs or a codegen step converts this into static lookup tables the engine uses at runtime.

## 5. Proposed Crate Graph Evolution

```
marque-ism          (vocabulary types, generated enums — unchanged)
marque-core         (exact-match scanner/parser — the fast path, unchanged)
marque-detect       (NEW — "is it a marking?" region detection for open text)
marque-rules        (trait definitions — add VocabularyProvider trait?)
marque-capco        (rule impls + vocabulary definitions + structural templates)
marque-engine       (orchestration + fuzzy resolver + confidence propagation)
marque-config       (unchanged — corrections map stays for org-specific overrides)
```

### New trait: VocabularyProvider

`marque-capco` implements this; the engine consumes it. Tells the engine:
- Here are my tokens and their categories
- Here are my structural templates (US portion, US banner, FGI, NATO, JOINT, etc.)
- Here are my category constraints (compartments are 3 chars, sub-compartments are 4, etc.)
- Here are my base-rate frequency tables (from corpus analysis)

This trait is what makes the engine domain-agnostic. The same fuzzy resolver and confidence propagation machinery works for CAPCO, CUI, NATO, French classifications, etc. — the vocabulary provider just changes.

The `VocabularyProvider` definition should be informed by the corpus analysis tool (what data does the engine actually need to make good confidence decisions?), which is one reason the corpus tool comes first.

### marque-detect (new crate)

Separable because:
- Form-field / structured-input applications don't need region detection at all — the input boundary IS the detection
- Open-text applications (email plugin, bulk ingest, Word plugin) need it
- Different deployment contexts may want different detection tuning (higher recall for bulk ingest, higher precision for email suggestions)
- It's a dependency of `marque-engine` but not of applications that bypass detection

### Engine pipeline (two paths)

**Structured input** (caller declares "this is a marking"):
```
Input → Token Resolver → Structure Matcher → Rules → Output
         (fuzzy match     (template           (existing
          against vocab)   selection)           rules)
```

**Open text** (need to find markings):
```
Input → marque-detect → Candidate Regions → Token Resolver → ...
         (region          (with P(marking))
          detection)
```

`marque-core`'s exact-match scanner/parser becomes the fast path inside the token resolver. Exact match = fuzzy match with distance 0 and confidence 1.0. You only go wider when exact match fails and region confidence justifies it.

## 6. Performance Considerations

Current performance: order of magnitude faster than the 16ms p95 target on 10KB inputs. This budget is large.

### Why the probabilistic path can be fast

- **The SIMD scanner still skips 99.9% of text**: Region detection looks for the same byte patterns (`(`, `//`, uppercase runs) as today's scanner. The wider net catches more candidates, but the vast majority of document bytes are still skipped.

- **Fuzzy matching on short tokens against a small vocabulary is cheap**: Most CAPCO tokens are 2-8 characters. The vocabulary is ~200 terms. Levenshtein distance on a 6-char token against 200 entries is ~12K operations — negligible. Pre-computable structures (BK-tree, first-char + length partitioning) can cut this further.

- **The fast path is still fast**: Well-formed markings hit exact matches everywhere and never enter the fuzzy path. The probabilistic machinery only activates on candidates that fail exact matching.

- **Candidate regions are small**: Even in the slow path, fuzzy resolution operates on a few dozen bytes (the marking), not the whole document.

### Budget allocation

If the current pipeline uses ~1ms of the 16ms budget, the probabilistic path could be 10x slower and still be well within target. The investment goes into correctness and coverage, not speed.

## 7. Implementation Sequence

### Phase 1: Corpus Analysis Tool
- Build the token-frequency factory (likely Python)
- Run against Enron corpus + public government docs
- Produce frequency tables for CAPCO vocabulary
- Empirical data informs all subsequent design decisions

### Phase 2: VocabularyProvider Trait
- Define the trait based on what the corpus analysis reveals the engine needs
- Implement for CAPCO in `marque-capco`
- Engine consumes the trait but initially uses it only for the existing exact-match path

### Phase 3: Token Resolver
- Fuzzy matching against vocabulary (edit distance, case normalization)
- Delimiter normalization
- Per-token confidence scoring using corpus-derived base rates
- Integrated into engine, activated when exact match fails

### Phase 4: marque-detect
- Region detection for open text
- Structural signal recognition (parens, `//`, caps patterns, line position)
- Confidence scoring for candidate regions
- Separable crate, optional dependency

### Phase 5: Structure Matcher
- Template selection (US portion, US banner, FGI, NATO, JOINT)
- Category constraint enforcement (compartment lengths, required prefixes)
- Upward confidence propagation (token results revising region confidence)

## 8. Market Context

This capability serves multiple markets with different requirements:

| Market | Detection needed? | Input type | Priority |
|--------|-------------------|------------|----------|
| Browser form validation | No — structured fields | Known marking text | High — direct pain point |
| Email/Word plugin | Yes — open text | Mixed prose + markings | High — large market (~1M cleared personnel) |
| Bulk data ingest / archival | Yes — open text | Documents at scale | High — AI/ML pipeline demand |
| API / microservice | Configurable | Both | Medium — infrastructure |

The form-field path is simpler (skip Layer 1) and could ship earlier. The open-text path (email/Word plugin) needs the full three-layer pipeline including `marque-detect`.

## 9. Open Questions

1. **Should the corpus frequency tables be baked into the WASM binary or loaded at runtime?** Baked in is simpler; loaded allows customization per deployment. Probably baked in with override capability.

2. **How does confidence interact with the existing severity system?** A rule might be `severity: fix` but the region confidence is 0.7 — does the engine downgrade to `warn`? Or does confidence multiply with fix confidence and the threshold gates the product?

3. **What's the right edit distance cutoff?** Distance 1 catches most typos. Distance 2 catches severe ones (SRCTE → SECRET) but risks false positives. The corpus base rates will inform this.

4. **Should the VocabularyProvider include ordering rules?** Today ordering is enforced by rules (E003). But if the token resolver needs to distinguish "misordered known tokens" from "unknown tokens in expected positions," it needs ordering knowledge.

5. **How does this interact with the corrections map?** The corrections map becomes a high-confidence override layer: if the user explicitly maps `SERCET → SECRET`, that's confidence 1.0 regardless of what fuzzy matching would produce. The map complements, not replaces, the probabilistic path.
