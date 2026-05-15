<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Research: Missing Portion Mark and Banner Detection

**Branch**: `copilot/detect-missing-portion-marks` | **Date**: 2026-05-15  
**Issue**: Detect missing portion marks and banners in classified documents  
**Status**: Research only — no implementation decisions are binding

---

## 1. What the Standard Requires

The authoritative source is CAPCO-2016 §C (Portion Marks) and §D (Banner Line),
cross-referenced with 32 CFR 2001 §2001.23 and ICD 710 §D.1.g.

### 1.1 What constitutes a "portion"

Per CAPCO-2016 §C (p25 / line 499 of the vendored `CAPCO-2016.md`):

> Each portion of a document (ordinarily a **paragraph**, but also **subjects**,
> **titles**, **metadata**, **graphics**, **tables**, **charts**, **bullet
> statements**, **subparagraphs**, **classified signature blocks**, bullets and
> other portions within **slide presentations**), must be marked …

The definition is intentionally broad: anything that can be read or displayed
independently as a discrete unit of information. The parenthetical `(U)`, `(S)`,
`(TS//SI//NF)` etc. must appear **immediately preceding** that unit.

### 1.2 Nested bullets and subparagraphs — the key exemptions

CAPCO-2016 §C (lines 505–510) defines three rules for nested content:

| Rule | Condition | Consequence |
|------|-----------|-------------|
| Same-level exemption | All sub-bullets/subparas in a segment carry the *same* classification | One portion mark at the **parent** level is sufficient |
| Varies-within | Sub-bullets/subparas carry *different* classifications | Each segment must be individually portion-marked |
| Higher-child rule | A subparagraph is *more* restricted than its parent | Do NOT raise the parent's mark; mark the child separately |

This means an algorithm cannot simply flag every paragraph without a leading
`(…)` as a violation — it must first establish whether the content is a
child of a marked parent carrying the same classification, which requires
structural awareness of the nesting hierarchy.

### 1.3 Legal-style section numbering (e.g., `1.A.(2).a.(b)`)

The CAPCO manual uses this format extensively for its own subsections. The
portion mark appears **after** the section number prefix, not embedded in it.
Example of a correctly-marked numbered section:

```
1.A. (S) The assessment indicates …
```

The regex for legal section numbering spans five alternating levels:
```
(decimal).(UPPERCASE).(paren-decimal).(lowercase).(paren-lowercase)
```

A partial regex to recognize a prefix at any depth:
```regex
^(\d+)(?:\.([A-Z])(?:\.\((\d+)\)(?:\.([a-z])(?:\.\(([a-z])\))?)?)?)?\.?\s+
```

Any token matching this pattern at the start of a line is a section-number
prefix; the portion mark (if present) follows it immediately.

### 1.4 All-unclassified document exemption

CAPCO-2016 §C.1 (line 528) explicitly states:

> For **completely unclassified documents** (i.e., no control markings)
> transmitted over a classified system, the designation "UNCLASSIFIED" must be
> conspicuously placed in the banner line. However, portion marks (i.e., "(U)")
> **are not required**. When transmitting completely unclassified documents over
> unclassified systems, classification markings are not required. For hard copy
> documents that are completely unclassified, "UNCLASSIFIED" in the banner line
> is optional; portion marks are not required.

This is critical: **an all-(U) document is not in violation even if no `(U)`
marks are present**, as long as there are no control markings. Detection logic
must first infer overall document classification before raising missing-mark
diagnostics.

### 1.5 Portion marking waivers

CAPCO-2016 §C.2 (lines 532–540) lists IC-wide waiver categories that existed
through 2017. For detection purposes, marque cannot enforce portion-marking
requirements for waivered categories without a waiver-aware configuration
layer. Waived categories include:

- Complex technical/financial/engineering diagrams, graphs, mission models,
  equations, and simulations
- GEOINT graphics products
- Internal forms
- President's Daily Brief (DNI copy)
- Raw mission data

A production-grade missing-mark detector should expose a configuration key
(e.g., `[capco] portion_mark_waiver = ["diagrams", "forms"]`) to suppress
diagnostics in those zones.

### 1.6 Banner requirements

CAPCO-2016 §D (p27) requires banners:
- **Top and bottom of every page**, including interior pages
- Must contain the **highest classification + most restrictive controls** for
  that page, or the overall document classification repeated on every page
- Must be clearly distinguishable from body text

A "missing banner" diagnostic fires when a page (or the first/last page of a
document) carries no recognizable banner candidate at all, or carries only a
lower-classification banner than what the page's portions project.

The marque engine already tracks `PageContext` (accumulated per-page
classification projection) and emits E031/E035 diagnostics when the banner
doesn't match the projected aggregate. Missing-banner detection is an
extension of that same rule: when `PageContext.projected_banner` is Some and
no banner candidate exists on the page, fire the diagnostic.

---

## 2. Document Parsing Strategies

### 2.1 Plain text (current marque input)

The existing `Scanner` in `marque-core` already finds portion candidates (via
`(…)` scanning with `memchr`) and banner candidates (line-prefix heuristic).
The gap for missing-mark detection is that the scanner finds only **present**
markings. To detect **absent** markings, a complementary "structure scanner"
is needed that identifies paragraph/section boundaries and then checks whether
each boundary is followed (or preceded) by a recognized portion candidate.

**Proposed plain-text paragraph scanner approach:**

```
Line classification heuristics (applied per line in order):
  1. Empty line → paragraph boundary (weak; resets accumulator)
  2. Legal section prefix → section start (strong boundary)
  3. Bullet prefix (-, *, •, –, numbered) → list item boundary
  4. Indented line after bullet → sub-bullet or continuation
  5. Non-indented non-empty line after paragraph break → new paragraph
  6. Form-feed / triple-newline → page break (already handled)
```

A `ParagraphCandidate` type would be emitted parallel to `MarkingCandidate`,
carrying the paragraph's byte span and a nesting level (0 = top-level, 1+ =
sub-bullets). The engine then cross-references `ParagraphCandidate` offsets
against the portion candidates already found: if a paragraph has no
immediately-preceding `MarkingCandidate` within its first ~50 bytes, it is
a "potentially unmarked" candidate.

This approach is already consistent with the marque pipeline philosophy:
zero-heap scanner → candidates → engine cross-reference.

### 2.2 DOCX

DOCX files are ZIP archives containing `word/document.xml`. The DOM exposes
the document structure natively:

- `<w:p>` — paragraph
- `<w:pStyle>` — paragraph style (Heading1/Heading2/Normal/ListBullet etc.)
- `<w:numPr>` — bullet/numbered list indicator with `<w:ilvl>` nesting level
- `<w:tbl>` — table
- `<w:drawing>` / `<w:pict>` — embedded image
- `<w:sdt>` — structured document tag (could be a caption or metadata block)

The **structure is already explicit** in DOCX — there is no need for
heuristic paragraph detection. A DOCX-aware extractor can output a flat list
of `StructuredPortion { kind: PortionKind, nesting_level: u8, text: String }`
where `PortionKind` is `Paragraph | BulletItem | Heading | TableCell | Caption
| ImageCaption | Title | Metadata`.

**Rust approach**: `quick-xml` + `zip` (both already in the workspace's
build-dep graph for the `marque-ism` code-gen path) can parse `word/document.xml`
with no new dependencies. `docx-rs` (crates.io) provides a higher-level
typed DOM but is primarily a write-oriented crate; for reading, the `quick-xml`
approach is more reliable and consistent with the existing workspace idiom.

A new `marque-extract` backend for DOCX would:
1. Unzip → extract `word/document.xml` and `word/numbering.xml`
2. Parse `numbering.xml` to resolve `<w:numId>` → indent level mapping
3. Walk `document.xml`, emitting one `StructuredPortion` per `<w:p>`,
   tagging its nesting depth from `<w:ilvl>` and its `pStyle`
4. Emit the flat `StructuredPortion` list to the engine

### 2.3 PDF

PDF is the hardest format because the file format does not encode paragraph
semantics — text is stored as positioned drawing operations, not logical blocks.

**Rust options:**

| Crate | Pure Rust | Paragraph reconstruction | Positioning data | Maintenance |
|-------|-----------|--------------------------|-----------------|-------------|
| `lopdf` | ✓ | ✗ (raw tokens) | Limited | Active |
| `pdf-extract` | ✓ | Partial (heuristic) | Via `lopdf` | Moderate |
| `pdfium-render` | ✗ (C++ PDFium binary dep) | ✗ (needs post-processing) | ✓ (bounding boxes) | Good |

**Recommended strategy for PDF:**

For a research-phase prototype, use **`pdf-extract`** for basic text
extraction with layout heuristics. For production accuracy, **`pdfium-render`**
with post-processing is the most reliable option — PDFium is the same library
that Chrome, Firefox, and virtually every government PDF reader uses.

The post-processing pipeline for PDF:
```
pdfium text spans → group by y-coordinate proximity (±font-height threshold)
                 → sort by x-coordinate within each y-band
                 → split at large x-gaps or y-jumps exceeding 1.5× line-height
                 → annotate each block as Paragraph|Heading|Caption based on
                   font size ratio, bold/italic flags, and column position
```

PDF captions are typically identified by:
- Font size smaller than body text
- Position immediately below/above a drawing or image object
- Text matching patterns like "Figure N:", "Table N:", "Exhibit N:"

### 2.4 Python/external tools via subprocess

For MVP or research purposes, Python's ecosystem has stronger PDF tooling:

- **`unstructured`** (Unstructured-IO): Full pipeline — PDF, DOCX, HTML, images.
  Emits typed elements (`NarrativeText`, `Title`, `Table`, `Image`,
  `FigureCaption`, `ListItem`) that map cleanly to `PortionKind`. This is the
  most capable all-in-one tool. Apache 2.0 licensed.

- **`kreuzberg`** (already in marque's plan for `marque-extract`): Provides
  structured element extraction. The `marque-extract` stub already references
  Kreuzberg as the intended backend. It returns typed chunks that include
  paragraph, table, and figure boundaries. The licensing review deferred in
  the `marque-extract` stub is the blocking factor.

- **`pdfminer.six`**: Fine-grained PDF layout analysis. Returns `LTPage`,
  `LTTextBox`, `LTTextLine`, `LTFigure`, `LTLayoutContainer` — the named
  types map cleanly to CAPCO portion types.

**Integration approach**: marque's existing `marque-extract` design uses an
async subprocess model (see `crates/extract/src/extractor.rs`). Either of the
above Python libraries can be wrapped in a thin gRPC or stdio-JSON bridge
that the Rust engine calls via `tokio::process::Command`. This isolates the
licensing question from the Rust dep closure.

---

## 3. Algorithm Strategies

### 3.1 Top-level detection algorithm

```
Input: Document bytes + detected format
Output: Vec<MissingMarkDiagnostic>

Phase 0: Format dispatch
  → If DOCX: invoke DOCX structural extractor → StructuredPortion[]
  → If PDF: invoke PDF structural extractor → StructuredPortion[]
  → If plain text: invoke heuristic paragraph scanner → StructuredPortion[]

Phase 1: Classify overall document
  → Run existing Scanner + Parser on all text to find all MarkingCandidates
  → If zero marking candidates found AND no banner → "all-unclassified"
    → STOP: no missing-mark diagnostics warranted (CAPCO §C.1)
  → Build PageContext per page (already done by existing engine)

Phase 2: Build portion index
  → For each StructuredPortion, record its byte offset in the source
  → For each StructuredPortion, check whether a MarkingCandidate immediately
    precedes it (within a configurable byte window, e.g., 50 bytes)
  → Exclude: waivered portion types (configured), blank/separator lines,
    section-number-only lines, continuation lines

Phase 3: Emit missing-mark diagnostics
  → For each StructuredPortion without a preceding MarkingCandidate:
      - If the portion is a child of a marked parent with same classification:
          suppress (CAPCO §C nested-bullet exemption)
      - Else emit Diagnostic { rule: "E0xx", severity: Warn, span: portion_span }

Phase 4: Detect missing banners
  → For each page: if PageContext has a non-zero projected classification AND
    no Banner MarkingCandidate was found on the page → emit missing-banner
    Diagnostic { rule: "E0yy", severity: Error, span: page_start_span }
```

### 3.2 Section number prefix detection

A section number prefix is a line opener that contains only a hierarchical
number/letter sequence with no substantive text. The portion mark belongs
**after** the prefix. Example correct patterns:

```
(U) 1.  Text follows here                 ← portion mark before section number
1. (U) Text follows here                  ← portion mark after section number  ← ALSO valid per CAPCO
1.A. (S//NF) Sensitive content here      ← after multi-level prefix
(b) (C) Sub-item text                     ← after paren-letter prefix
```

The CAPCO-2016 manual shows portion marks appearing **to the right** of section
numbers in legal-style documents. Both conventions exist in the wild. The
scanner's heuristic should accept either position within the first ~80 bytes
of a paragraph.

Regex pattern for section-number prefixes (to be consumed before checking for
a portion mark):

```rust
static SECTION_PREFIX: &str = concat!(
    r"^",
    // Level 1: decimal
    r"(?:\d+\.?)",
    // Level 2: uppercase letter
    r"(?:[A-Z]\.?)?",
    // Level 3: parenthesized decimal
    r"(?:\(\d+\)\.?)?",
    // Level 4: lowercase letter
    r"(?:[a-z]\.?)?",
    // Level 5: parenthesized lowercase
    r"(?:\([a-z]\)\.?)?",
    // Optional trailing separator
    r"\s+",
);
```

### 3.3 Nesting context and parent-marking inference

To apply the "same-level exemption" (CAPCO §C rule 1), the algorithm needs to
track the current nesting stack. The cleanest data structure:

```rust
struct NestingFrame {
    level: u8,                     // 0 = top-level paragraph
    portion_mark: Option<IsmAttributes>,  // parsed marking at this level
    span: Span,                    // byte range of the parent portion
}

struct NestingContext {
    stack: Vec<NestingFrame>,
}
```

When entering a child level (increased indent or increased section depth):
- If the parent has a portion mark, push that mark onto the stack
- Children at the same classification as the top-of-stack are exempt from
  individual marks

When exiting (decreasing indent or section depth), pop the stack.

The key challenge is **inferring the nesting level** from plain text (where
indentation is the only signal) vs. DOCX (where `<w:ilvl>` is explicit) vs.
PDF (where it must be inferred from x-coordinate and font properties).

### 3.4 All-unclassified detection

Two heuristics for detecting "this document is entirely unclassified":

**Heuristic A — Banner inspection**: If the document has at least one banner
and all banners are `UNCLASSIFIED` with no control markings, the document
is all-unclassified. This is the most reliable signal and requires zero
modification to the existing pipeline.

**Heuristic B — Portion scan**: If zero portion marks are found, run the
existing scanner over the full document to check for any classification-level
tokens (`SECRET`, `TOP SECRET`, `CONFIDENTIAL`, `RESTRICTED`, SCI tokens,
etc.) appearing anywhere in the text. If none are found, declare all-unclassified.

Heuristic B is prone to false negatives (a document saying "this SECRET
program" in narrative text, not as a marking). Prefer Heuristic A when
banners are present; fall back to B only for unmarked documents without a
banner.

### 3.5 Banner-presence detection (missing banner)

The existing `Scanner::scan_banners` covers:
- US classification prefixes (`UNCLASSIFIED`, `SECRET`, `TOP SECRET`, etc.)
- Non-US `//`-prefixed lines (`//NATO SECRET//`, `//NS//`)
- `RESTRICTED//` prefix

A "missing banner" condition is:
- No Banner candidate at position `[0 .. first_page_break]` (top-page missing)
- No Banner candidate at position `[last_newline_of_page .. page_break]`
  (bottom-page missing)

The scan window for banner candidates near the page top/bottom should be
configurable (default: first/last 300 bytes of a page's text span). The engine
already has page-break candidates from `Scanner::scan_page_breaks`; the
banner check becomes a structural query over the sorted `Vec<MarkingCandidate>`
output.

---

## 4. Library Landscape Summary

### 4.1 Rust ecosystem (preferred, WASM-safe where noted)

| Library | Purpose | WASM-safe | Notes |
|---------|---------|-----------|-------|
| `quick-xml` | DOCX XML parsing | ✓ | Already in build-dep graph for `marque-ism` |
| `zip` | DOCX unzipping | ✓ (via `rc-zip`) | `rc-zip` is a WASM-safe alternative to `zip` |
| `pdf-extract` | Basic PDF text extraction | ✓ | Heuristic paragraph reconstruction only |
| `pdfium-render` | High-quality PDF with bounding boxes | ✗ (C++ dep) | Best production option for non-WASM path |
| `lopdf` | Low-level PDF object access | ✓ | Does not reconstruct paragraphs |
| `memchr` | SIMD paragraph boundary scan | ✓ | Already in `marque-core` |
| `regex` | Section prefix / bullet detection | ✓ | Already used elsewhere in workspace |

### 4.2 Python/external ecosystem (for `marque-extract` subprocess path)

| Library | Purpose | License | Notes |
|---------|---------|---------|-------|
| `kreuzberg` | Full document extraction + structure | MIT | Already planned in `marque-extract`; licensing review pending |
| `unstructured` (Unstructured-IO) | All-format extraction with element types | Apache 2.0 | Most mature; handles PDF/DOCX/HTML/images + OCR |
| `pdfminer.six` | Fine-grained PDF layout analysis | MIT | Best for pure-PDF paragraph/column structure |
| `python-docx` | DOCX paragraph/table/image access | MIT | Direct DOM access; integrates cleanly with a JSON bridge |

### 4.3 Tooling reference (government / commercial)

| Tool | Relevance | Notes |
|------|-----------|-------|
| MarkedDoc (USAF) | Auto-marking for Word | Not open source; shows the DOCX-first approach is proven |
| TITUS Classifier | Enterprise marking enforcement | Commercial; CAPCO-aware |
| Microsoft Purview / AIP | Office integration | Limited portion-mark awareness without customization |
| CMT (IC Classification Management Tool) | IC-standard automated marking | Closed-government; ICS 2008-500-05 |

---

## 5. Integration with Existing Marque Architecture

### 5.1 New pipeline stage: structural scanner

The cleanest integration point is a new **structural scanner** that runs
**before** the existing `Scanner::scan` pass and emits `StructuredPortion`
candidates alongside `MarkingCandidate` candidates:

```
Source bytes
  → StructuralScanner::scan() → Vec<StructuredPortion>   ← NEW
  → Scanner::scan()           → Vec<MarkingCandidate>     ← existing
  → Parser::parse()           → Vec<(IsmAttributes, Span)> ← existing
  → Engine::lint()            → Vec<Diagnostic>           ← existing with new rules
```

`StructuredPortion` does not need to carry parsed `IsmAttributes` — it is a
structural hint, not a parsed marking. The engine cross-references the two
streams.

### 5.2 New rule ID candidates

Following the existing rule ID convention (`E###` = error, `W###` = warning):

| Rule ID | Description | Severity | Authority |
|---------|-------------|----------|-----------|
| `E066` or `W010` | Missing portion mark on paragraph | Warn | CAPCO-2016 §C p25 |
| `E067` or `W011` | Missing portion mark on table/figure/caption | Warn | CAPCO-2016 §C p25 |
| `E068` or `E069` | Missing top banner on page | Error | CAPCO-2016 §D p27 |
| `E069` or `E070` | Missing bottom banner on page | Error | CAPCO-2016 §D p27 |

Severity rationale: Missing portion marks are typically `Warn` because the
same-level exemption makes false positives likely until the structural parser
is high confidence. Missing banners are `Error` because the document-level
obligation is absolute (no nesting exemption applies).

### 5.3 Confidence scoring

Because paragraph segmentation introduces ambiguity (especially in plain text
and PDF), missing-mark diagnostics should carry a **confidence score below 1.0**
by default (e.g., 0.75 for plain-text heuristic segmentation, 0.90+ for DOCX
structural extraction). The engine's existing `confidence < threshold → suggest`
gate means low-confidence missing-mark findings surface as suggestions rather
than auto-fixes, which is appropriate since the engine cannot synthesize the
correct classification value to insert.

### 5.4 `marque-extract` dependency implications

The structural extraction for DOCX and PDF would live in `marque-extract`, which
is currently a stub pending the Kreuzberg licensing decision. However, the plain-
text structural scanner (which covers a large portion of real IC documents,
since most classified text is passed as `.txt` through JWICS systems) can be
implemented entirely in `marque-core` without any new dependencies.

The recommended phasing is:
1. **Phase A**: Plain-text structural scanner in `marque-core` — no new deps,
   WASM-safe, covers text documents and direct-pipe workflows
2. **Phase B**: DOCX structural extractor in `marque-extract` using `quick-xml`
   + `zip` — two crates, both WASM-safe for the zip step, no licensing risk
3. **Phase C**: PDF via `pdfium-render` (non-WASM) or Kreuzberg subprocess once
   the licensing question is resolved

---

## 6. Open Questions

| # | Question | Impact | Suggested Resolution |
|---|----------|--------|----------------------|
| OQ-1 | How does the structural scanner handle continuation lines in plain text (a paragraph that wraps across lines but is logically one portion)? | High — false positives on multi-line paragraphs | Track "last non-empty line before this paragraph started a new (U)/(S)/etc. mark"; suppress if within same text block |
| OQ-2 | Should the all-unclassified exemption also suppress missing-banner warnings? | Medium — banner is technically required even on all-U docs when on a classified system | Follow §C.1 literally: UNCLASSIFIED banner is required on classified-system transmissions even if (U) portion marks are waived; suppress only missing-portion-mark, not missing-banner |
| OQ-3 | How to detect figure/image/table captions in plain text? | Medium — captions are named explicitly as portion-marked in §C | Use patterns: "Figure N:", "Table N:", "Exhibit N:", "Photo N:" etc. at start of paragraph; flag as `PortionKind::Caption` |
| OQ-4 | How should the structural scanner interact with the decoder recognizer (`DecoderRecognizer`)? | Low initially | Structural scanner runs before Phase 1 (Scanner); it does not need to know about the decoder. Output is independent |
| OQ-5 | What is the correct behaviour when a section number prefix is present but the portion mark is embedded in the numbering (e.g., `1.(U).A.` — clearly a mistake)? | Low | Treat as a misplaced portion mark, not a missing one; the existing scanner already detects `(U)` in that position |
| OQ-6 | For waivered categories (diagrams, GEOINT), how does the detector know a paragraph describes a waivered element without format-level metadata? | Medium — false positives on diagrams in plain text | In plain text, use keyword heuristics ("the figure above", "graph 3", "mission model"); in DOCX/PDF use the element type from the structural extractor |

---

## 7. References

- CAPCO-2016 §C (Portion Marks), p25–26 — vendored at `crates/capco/docs/CAPCO-2016.md`
- CAPCO-2016 §D (Banner Line), p27 — same
- 32 CFR 2001 §2001.23 (Classification Marking in the Electronic Environment)
- ICD 710 §D.1.g (Portion Marking Requirements)
- `crates/core/src/scanner.rs` — existing `Scanner::scan_portions`, `scan_banners`, `scan_page_breaks`
- `crates/engine/src/engine.rs` — `PageContext`, two-pass fix engine
- `crates/extract/src/extractor.rs` — `marque-extract` stub awaiting Kreuzberg
- `quick-xml` (crates.io) — XML streaming parser; already in `marque-ism` build-dep graph
- `pdfium-render` (crates.io) — Rust bindings for PDFium
- `pdf-extract` (crates.io) — pure-Rust heuristic PDF text extraction
- Unstructured-IO (`github.com/Unstructured-IO/unstructured`) — Python; best all-in-one
- Kreuzberg (`github.com/Goldziher/kreuzberg`) — Python; already in `marque-extract` plan
