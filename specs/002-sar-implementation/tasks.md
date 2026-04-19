<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# SAR Implementation Tasks

Task numbering aligns with the harness `TaskCreate` IDs on branch `feat/sar-implementation`.

## P1 — Data model (task #2)

- [ ] Remove `SarIdentifier` enum emission from `crates/marque-ism/build.rs` (delete lines ~402–421 in the current file; replace with a comment explaining why the CVE file is intentionally unused).
- [ ] Add `SarMarking`, `SarIndicator`, `SarProgram`, `SarCompartment` types to `crates/marque-ism/src/attrs.rs`.
- [ ] Change `IsmAttributes.sar_identifiers: Box<[SarIdentifier]>` → `IsmAttributes.sar_markings: Option<SarMarking>`.
- [ ] Add `TokenKind::SarIndicator`, `::SarProgram`, `::SarCompartment`, `::SarSubCompartment`. Mark `TokenKind::SarIdentifier` `#[deprecated(note = "use SarIndicator/SarProgram/SarCompartment/SarSubCompartment")]`.
- [ ] Delete the `SarIdentifier` re-export from `attrs.rs`.
- [ ] Update `marque-core/src/parser.rs` consumers: the `sar: Vec<SarIdentifier>` accumulator becomes state for the new subparser (or is removed entirely once P2 lands).
- [ ] Update `marque-capco/src/rules.rs` consumers: the `TokenKind::SarIdentifier => sar.push(...)` arms in `reorder_marking` and block-ordering switch to `TokenKind::SarIndicator`.
- [ ] `cargo check --workspace` green.

## P2 — Subparser (task #3)

- [ ] Add `fn parse_sar_category(text: &str, base: usize) -> Option<(SarMarking, Vec<TokenSpan>)>` to `crates/marque-core/src/parser.rs`.
- [ ] Grammar per `spec.md` §R2. Recursive-descent, no regex.
- [ ] Helper `split_once_hyphen_preserving` to separate `PROG-COMP SUB` tokens without swallowing the hyphen.
- [ ] Dispatch in `IsmAttributes::from_marking_bytes` when a block text starts with `SAR-` or `SPECIAL ACCESS REQUIRED-`.
- [ ] Unit tests (put in `#[cfg(test)] mod sar_parse_tests` in `parser.rs`):
  - `SAR-BP` → one program, no compartments.
  - `SAR-BP/CD/XR` → three programs, no compartments.
  - `SAR-BP-J12` → one program with one compartment.
  - `SAR-BP-J12 J54` → one program, one compartment with one sub.
  - `SAR-BP-J12 J54-K15/CD-YYY 456 689/XR-XRA RB` (§H.5 p100 canonical).
  - `SPECIAL ACCESS REQUIRED-BUTTER POPCORN`.
  - Rejects: `SAR` (no hyphen), `SAR-` (empty program), `SAR-BP//CD` (double slash — different test case for E030).

## P3 — Rules (task #4)

- [ ] E026 `sar-portion-form` — portion uses `SPECIAL ACCESS REQUIRED-` → diagnostic with fix `SAR-<first N chars of ID as abbrev>`. Since we can't invent an abbrev, the fix confidence is low (<0.6) — surface as suggestion only.
- [ ] E027 `sar-classification` — banner/portion with `MarkingClassification::Us(Unclassified)` and `sar_markings.is_some()` → diagnostic, no fix.
- [ ] E028 `sar-program-order` — walk `programs` slice, detect out-of-order adjacent pairs via the shared sort helper; fix reorders.
- [ ] E029 `sar-compartment-order` — per program, walk compartments; per compartment, walk sub-compartments.
- [ ] E030 `sar-indicator-repeat` — detect adjacent category blocks both starting with a SAR indicator; fix coalesces into one block with `/`.
- [ ] Register all five in `CapcoRuleSet::new()`.
- [ ] Unit tests per rule, co-located at the bottom of `rules.rs` following existing conventions.
- [ ] All citations use `CAPCO-2016 §H.5` or `CAPCO-2016 §A.6` as noted in spec §Rules.

## P4 — Page roll-up (task #5)

- [ ] `PageContext::expected_sar_marking() -> Option<SarMarking>` in `marque-ism/src/page_context.rs`.
- [ ] Union semantics: program-id-keyed hashmap → merged compartments → merged sub-compartments. Rendered in sort order.
- [ ] Extend `render_expected_banner()` to insert the SAR block between SCI and AEA (matching CAPCO category order).
- [ ] E031 `sar-banner-rollup` rule in `marque-capco/src/rules.rs` — compares observed banner SAR block against `ctx.page_context`'s expected.
- [ ] Tests: single portion with one program; two portions with different programs merging; banner missing a portion's program.

## P5 — Corpus + harness (task #6)

- [ ] `tests/corpus/valid/sar_abbrev_banner.txt` — `TOP SECRET//SAR-BP//NOFORN`
- [ ] `tests/corpus/valid/sar_full_banner.txt` — `TOP SECRET//SPECIAL ACCESS REQUIRED-BUTTER POPCORN//NOFORN`
- [ ] `tests/corpus/valid/sar_multi_program.txt` — §H.5 Table 7 canonical
- [ ] `tests/corpus/valid/sar_portion.txt` — `(TS//SAR-BP//NF)`
- [ ] `tests/corpus/invalid/sar_unclassified.txt` — `UNCLASSIFIED//SAR-BP` → expect E027
- [ ] `tests/corpus/invalid/sar_bad_order.txt` — `SECRET//SAR-CD/BP//NOFORN` → expect E028
- [ ] `tests/corpus/invalid/sar_indicator_repeat.txt` — `SECRET//SAR-BP//SAR-CD//NOFORN` → expect E030
- [ ] `tests/corpus/invalid/sar_portion_full_form.txt` — `(TS//SPECIAL ACCESS REQUIRED-BP//NF)` → expect E026
- [ ] `tests/corpus/invalid/sar_banner_missing_program.txt` — portion has `SAR-CD` but banner has only `SAR-BP` → expect E031
- [ ] Wire each invalid fixture's expected rule IDs into the harness annotation format already used for E001–E025.

## P6 — Docs (inline with P5)

- [ ] `CLAUDE.md` — add SAR subsection under Architecture / Two-Layer Rule Architecture.
- [ ] `README.md` — update rule count.
- [ ] `crates/marque-ism/README.md` — migration note on `SarIdentifier` → `SarMarking`.

## Side-fix — E004 same-category `//` (task #7)

The existing rule had the direction inverted. Per §A.6 Figure 2, `/` is the within-category separator and `//` is the category separator, so the bug to flag is `//` appearing **between two same-category tokens**, not `/` within a category.

- [ ] Delete the existing "missing separators (single `/` not part of `//`)" branch in `SeparatorCountRule::check` — a single `/` within a block is valid syntax.
- [ ] Keep the `////`-style "redundant separator" branch unchanged.
- [ ] Add a new branch that walks adjacent category blocks and classifies each block's lead token (via `SciControl::parse`, `DissemControl::parse`, `AeaMarking::parse`, `NonIcDissem::parse`, trigraph check, SarIndicator check). When both blocks resolve to the same CAPCO category, emit an E004 diagnostic on the intervening `//` separator with fix → `/`. Confidence 0.95.
- [ ] Positive tests (should fire):
  - `SECRET//SI//TK//NOFORN` → fix `SECRET//SI/TK//NOFORN`
  - `SECRET//ORCON//NOFORN` → fix `SECRET//ORCON/NOFORN`
- [ ] Negative tests (should not fire):
  - `SECRET//SI//NOFORN` (different categories)
  - `SECRET//SI/TK//NOFORN` (already correct)
  - `SECRET//XYZZY//NOFORN` (unknown token, can't classify)
- [ ] `SECRET////NOFORN` still produces one E004 from the existing `////` branch.
- [ ] Commit separately from SAR commits on this branch.
- [ ] Open a follow-up issue: should there be a separate rule for genuinely missing separators (`SECRET/NOFORN` — different categories, only one slash)? That needs different logic and isn't in this branch's scope.
