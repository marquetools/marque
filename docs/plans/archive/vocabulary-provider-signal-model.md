# VocabularyProvider as Signal Codebook

**Date**: 2026-04-16
**Status**: archived — explicitly demoted by `2026-04-17-marking-scheme-lattice-design.md`: "we keep 'projection is lossy compression' as a useful intuition and drop the Fourier / SNR / matched-filter framings from the architecture. Those were reaching, and the grammar-plus-lattice framing covers the same ground with less machinery." Kept for historical context.
**Context**: Alternative framing of the VocabularyProvider trait, abstracted away from classification-specific terminology. Intended to help evaluate the design from outside the CAPCO domain.

---

## The Encoding Metaphor

A marking is a **compressed encoding** of metadata about a unit of information. Each symbol's presence encodes facts at multiple levels:

| Level | What's encoded | Example |
|-------|---------------|---------|
| Symbol | A specific property of this unit | `NF` = not releasable to foreign nationals |
| Category | A class of properties with shared semantics | Dissemination controls |
| Marking | The complete metadata for one unit | `(TS//SI-G//NF)` |
| Page aggregate | Distilled metadata for a collection of units | `TOP SECRET//SI-G//NOFORN` (banner) |
| Document aggregate | Distilled metadata for all pages | Overall document classification |

As you go up from the granular (symbol) to the macro (document), you **lose resolution but gain convergence**. The banner tells you the worst-case properties of the page but not which specific unit has which property.

## Parallel Category Transforms

The aggregation from marking → page → document is not one transform — it's a **bank of parallel transforms**, one per category, each with different semantics:

| Category | Transform | What's preserved | What's lost |
|----------|-----------|------------------|-------------|
| Classification level | max | The peak | Distribution across units |
| Compartment access | union | All required channels | Which unit needs which |
| Release targets | intersection | Common recipients only | Per-unit recipient lists |
| Dissem controls | union + supersession | All constraints | Redundant weaker constraints |
| Temporal constraints | max date | Latest expiration | Per-unit dates |

Some transforms interact: a supersession rule (e.g., "not releasable" absorbs "releasable to specific list") eliminates an entire category's contribution once triggered.

This is structurally similar to signal processing: decompose a complex signal into independent basis components (categories), apply a per-component transform, recompose.

## The Recognition Problem as Signal Detection

Detecting markings in text is a **signal-in-noise** problem:

- **Signal**: A marking — multiple tokens from a known codebook, structured by delimiters, in a characteristic position
- **Noise**: Normal text — may contain individual tokens that coincidentally match codebook entries

Individual tokens can be ambiguous — the noise floor for some tokens is high:
- `C` appears at ~61 per million words in English office text (copyright, list markers)
- `SECRET` appears at ~1.4 per million words (confidentiality notices)

But the **composite signal** — multiple codebook tokens from different categories, near a structural delimiter — has near-zero noise floor. The corpus data confirms this: cross-correlation between CAPCO marking structure and English prose is essentially zero.

This is exactly what matched filters and correlation detectors exploit: the individual components may be noisy, but the pattern's components are correlated (they appear together in markings) while noise is uncorrelated (individual token appearances are independent).

## The Codebook Abstraction

Instead of thinking of the VocabularyProvider as "a list of tokens and rules," think of it as defining a **signal codebook**:

### 1. Basis Components (Categories)
The independent channels that compose into a marking. Each has:
- A set of valid symbols (the alphabet for this channel)
- Encoding rules (delimiters, ordering, cardinality)
- An aggregation transform (how this channel combines across observations)
- A noise profile (base rate in non-signal text)

### 2. Signal Templates (Marking Structures)
How basis components compose into complete signals:
- Which components are required, optional, forbidden
- The inter-component delimiter
- The signal's structural envelope (parenthesized, bare, bracketed)
- The expected form of symbols in this template (abbreviated vs expanded)

### 3. Symbol Relationships
How symbols within a codebook relate:
- Equivalent forms (abbreviation ↔ expansion)
- Supersession (one symbol's presence renders another irrelevant)
- Dependency (symbol A requires symbol B's presence)
- Conflict (symbols A and B cannot co-occur)

### 4. Expansion Tables
Some symbols are composite — they represent a set of other symbols:
- A tetragraph (group identifier) expands to a set of trigraphs (member entities)
- Aggregation transforms may need to expand composites before operating

### 5. Cross-Observation Aggregation
How to combine signals observed across multiple units:
- Per-component transform (max, union, intersection, supersession)
- Transform interactions (supersession in one component may eliminate another component entirely)
- Levels of aggregation (marking → page → document)

### 6. Noise Floor (Corpus Base Rates)
Per-symbol and per-structure false positive rates in the target text environment:
- Individual symbol base rates
- Contextual rates (symbol near delimiter, symbol in structural position)
- Composite rates (multiple symbols co-occurring)

## How This Informs the Trait

The engine is a **decoder**:

1. **Detection**: Scan for correlation between input text and the codebook's structural templates. A correlation spike = candidate marking region.

2. **Decomposition**: Split the candidate into per-category components using the template's delimiter rules.

3. **Symbol resolution**: For each component, match against the category's symbol set. Exact match → high confidence. Fuzzy match → weighted by noise floor and context.

4. **Validation**: Check composition rules — required categories present, cardinality satisfied, no conflicting symbols.

5. **Aggregation**: When processing multiple markings (page/document), apply per-category transforms, expanding composites as needed.

The VocabularyProvider supplies all of steps 1-5 as **data**. The engine implements the generic decoder. Domain-specific quirks that don't fit the data model surface through behavioral override hooks on the trait.

## Why This Framing Helps

Describing the system without its domain-specific vocabulary makes it easier to:

1. **Evaluate generality**: Would this codebook model work for a different structured metadata system? If yes, the abstraction is likely right. If it requires CAPCO-specific concepts, it's leaking.

2. **Identify missing structure**: The parallel-transform model immediately reveals questions like "what happens when transforms interact across categories?" that a token-and-rules model might not surface.

3. **Borrow from mature fields**: Signal detection, coding theory, and information theory have well-understood tools for exactly these problems. We don't need to invent new math — we need to recognize which existing math applies.

4. **Avoid over-fitting**: Every example in the design discussion so far is CAPCO. This framing forces the question: "is this a property of classification markings in general, or a CAPCO-specific detail?"
