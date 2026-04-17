# VocabularyProvider — Domain Notes from Design Discussion

**Date**: 2026-04-16
**Context**: Design critique of the VocabularyProvider trait sketch. These notes capture domain-specific constraints that the trait must be able to express.

---

## 1. REL / REL TO Shorthand

`REL` by itself (without `TO <countries>`) is acceptable as a portion marking shorthand when the entire document is REL TO the same country set. The full `REL TO USA, GBR, ...` list lives in the banner; portions just say `REL`. This is a **style recommendation**, not a hard error — include as a style/warning rule.

**Implication for trait**: The engine needs to understand that `REL` in a portion is a valid shorthand that defers to the banner/document context for its full meaning. Token resolution can't be purely local — some tokens are context-dependent references.

## 2. Trigraph/Tetragraph Disambiguation

A trigraph or known tetragraph (NATO, FVEY, ACGU, etc.) indicates one of three things:

1. **Non-US classification** (second category): the trigraph identifies the country/org whose classification system applies. Look for a classification level next (S, C, TS, R, NS, etc. including NATO equivalents). The country/org comes first, classification follows. Only one country/org per non-US block.

2. **FGI (Foreign Government Information)**: unambiguous as a token, but can appear in either:
   - The non-US block (undisclosed country) 
   - The FGI category
   - The differentiator is classification presence and positioning

3. **REL TO country list**: trigraphs after `REL TO` are release targets, not classification sources.

**Implication for trait**: Token meaning depends on position within the marking. A trigraph isn't just "a country" — it's "a country in a specific role." The category system needs to express that the same token (e.g., `GBR`) can appear in different categories with different semantics depending on structural position.

## 3. FGI as Banner-Only (Style Rule)

FGI should generally only appear in banner markings, not portion markings. It's frowned upon to comingle US and foreign information within a single portion because it becomes difficult or impossible to discern which information comes from which source. Finished intelligence sometimes does it, but requires clear citations linking the foreign source.

**Current rule**: There's already a warning rule for FGI in portions.

## 4. Document Context vs Page Context

The system needs three levels of context aggregation:

| Level | What it tracks | Example |
|-------|---------------|---------|
| **Marking** | A single marking's tokens and structure | `(TS//SI-G//NF)` |
| **Page** | Aggregate of all portions on a page → page banner | Max classification, union SCI, intersection REL TO |
| **Document** | Aggregate across all pages → document-level banner | Common practice: an aggregate marking at the document's beginning, e.g., a box stating "Document classified up to TOP SECRET//SI//REL TO USA, NOR" |

On a page marked UNCLASSIFIED, you might still have a document-level box saying the overall document goes up to TOP SECRET//SI//REL TO USA, NOR. The page is UNCLASSIFIED but it's within a classified document.

**Implication for trait**: The VocabularyProvider needs to express roll-up rules — how markings aggregate at page and document level. This is the map/reduce operation described below.

## 5. Roll-Up as Map/Reduce

Marking aggregation (portion → page banner → document banner) is essentially a map/reduce:

- **Map**: For each portion marking, extract the relevant fields (classification, SCI controls, dissem controls, REL countries, etc.)
- **Reduce**: Combine per a set of roll-up rules:
  - Classification: **max** (TS > S > C > U)
  - SCI controls: **union** (if any portion has SI-G, the page has SI-G)
  - Dissem controls: generally **union**, but with supersession rules
  - REL TO countries: **intersection** (only countries common to all REL portions)
  - Declassify-on dates: **max date** (furthest out)

Some reduce operations eliminate the need to track further:
- Once NOFORN appears in the banner, country tracking is irrelevant for that page/document — NOFORN supersedes any REL TO list
- Tetragraph reduction may similarly collapse (see below)

**Implication for trait**: Roll-up rules should be expressible as data — a reduce function per category that the engine applies. The VocabularyProvider defines: "for this category, the aggregation strategy is MAX / UNION / INTERSECTION / SUPERSEDED_BY."

## 6. Tetragraph Intersection (the FVEY/NATO Problem)

When a page or document contains both FVEY and NATO REL material, the banner's REL TO list is the **intersection** of the two membership lists:

- FVEY = {USA, GBR, CAN, AUS, NZL}
- NATO = {USA, GBR, CAN, ... + 28 other members}
- Intersection = {USA, GBR, CAN} (AUS and NZL are FVEY-only, so NATO knocks them out; the 25+ NATO-only members are knocked out by FVEY)

More complex with operational tetragraphs (e.g., ISAF, KFOR) because they're often "NATO + these specific others" — the membership lists need to be known to compute the intersection.

This also means:
- FVEY + NATO ≠ FVEY (because 2 members are lost)
- FVEY + NATO ≠ NATO (because 25+ members are lost)
- The result is neither tetragraph — it's a raw country list

**Implication for trait**: The VocabularyProvider needs:
1. A way to express tetragraph → member-country expansion
2. The engine needs to know that REL TO aggregation intersects at the country level, not the tetragraph level — you can't just intersect tetragraph names, you have to expand to members, intersect, then potentially re-compress to a tetragraph if the result happens to match one

## 7. Country List Ordering

REL TO country lists always put USA first, then remaining countries in alphabetical order. If multiple portions' country lists make it into the banner, it's their intersection, but the ordering rule still applies to the result.

**Implication for trait**: Ordering within a category isn't always the same as ordering between categories. The category schema needs to express intra-category ordering rules (e.g., "USA always first, then alphabetical") separately from inter-category ordering (e.g., "SCI before dissem before REL TO").

## 8. FISA Notices

FISA documents should carry a FISA notice (beyond just the FISA dissem control token). This is an example of a document-level requirement triggered by the presence of a specific token — not a marking validation rule per se, but a document completeness check.

**Implication for trait**: Some tokens trigger document-level requirements. This probably lives in `post_validation` rather than in the core token/category schema.

## 9. The "REL" + PageContext Dependency

When a portion uses bare `REL` (without countries), the engine needs to track countries from other portions that DO specify them, so it can assemble a correct banner. This means:
- Portion-level parsing is not self-contained for `REL`
- The aggregation context (page/document) feeds back into how individual markings are interpreted
- The VocabularyProvider needs to express that certain tokens are "context-dependent" — their full meaning resolves during aggregation, not during initial parsing

---

## Summary: What the Trait Sketch Is Missing

Based on this discussion, the current sketch needs:

1. **Position-dependent token semantics** — a trigraph means different things depending on where it appears in the marking structure (non-US classification source vs REL TO target vs FGI indicator)

2. **Aggregation/roll-up rules** as data — per-category reduce strategies (max, union, intersection, supersession) that the engine applies during page/document context assembly

3. **Tetragraph expansion tables** — mapping tetragraphs to their member country lists so the engine can compute intersections at the country level

4. **Intra-category ordering rules** — separate from inter-category ordering (USA first in country lists, alphabetical after)

5. **Context-dependent token resolution** — some tokens (bare `REL`) defer their full meaning to page/document context

6. **Document-level requirements** — tokens that trigger document-wide obligations (FISA notice, CAB requirements)
