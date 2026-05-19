<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 5 — Architect Review (3-reviewer pass)

**Date:** 2026-05-18
**Branch:** `refactor-006-pr-5-foreign-banner-correctness` at commits `fe88f0f7..37952185`.
**Reviewer:** ecc:architect (returning from the preflight authorship).

> The agent's tool subset (Read/Grep/Glob only) precluded writing this
> file directly; content reproduced verbatim from the agent's result.

## 1. Verdict

**APPROVE-WITH-NITS.** PR 5 closes #276 as scoped by PM Addendum I. Two real quality concerns surfaced (one minor, one investigative).

## 2. #276 closure verification

| Case | Verdict | Evidence |
|---|---|---|
| Case 1 (pure foreign `(//DEU S//REL TO USA, DEU)`) | **PASS** | Already correct at projection layer (PR 4b-E + PR 6c retirement of `expected_classification` hardcode). `tests/corpus/foreign/pure_foreign_banner.expected.json` documents zero diagnostics; `foreign_corpus.rs::t062_pure_foreign_banner_zero_diagnostics` asserts E068/E069 don't fire on the well-formed case. PR 5 adds defense-in-depth: a malformed banner over this projection would now be caught. |
| Case 2 (commingled `(//DEU S) + (S)`) | **PASS** | `mixed_us_foreign_rollup.txt` with portions `(S//NF) + (//DEU TS//REL TO USA, DEU)` and wrong banner `SECRET//NOFORN` asserts E068 + E069 both fire (`foreign_corpus.rs:218-249`). |
| Case 3 (commingled with NF) | **PASS** | Same fixture as Case 2. T059b NOFORN-supremacy composition is asserted indirectly: if `capco/noforn-clears-rel-to` PageRewrite were dropping the FGI marker, E069's missing-marker arm would not fire — but it does. |

The three reproductions close cleanly. PR 5 doesn't change the projection layer (already correct); it adds the rule-detection layer that previously didn't exist.

## 3. Architectural fit (walker pattern)

**PASS.** E068 / E069 are catalog ROWS of `BannerMatchesProjectedRule` per the established E031 / E035 / E040 precedent (`crates/capco/src/rules.rs:4080-4274`):

- `BannerMatchesProjectedRule::id()` returns `E031` (bookkeeping ID); per-row IDs travel on emitted diagnostics — confirmed at `rules.rs:4083-4087`.
- `additional_emitted_ids()` advertises `("E068", "banner-classification-mismatch")` and `("E069", "banner-fgi-marker-mismatch")` at `rules.rs:4149-4163`, so `.marque.toml` severity overrides resolve correctly.
- Registered rule count stays at **38** (the PM Addendum I.6 directive to bump to 40 was structurally wrong — per-row IDs of an existing walker are NOT separate `Rule` impls). `post_3b_registration_pin.rs:135-158` correctly keeps `raw_len == 38`; `corpus_parity.rs:240-242` keeps `38`. This is correct.
- Per-row `Severity::Error` no-fix is preserved on the row construction (`rules.rs:4245-4248`, `:4269-4272`).
- Citation per row is single-§ scope (D13): `§H.7 pp123-125` for E068, `§H.7 p124` (plus worked-example p126/p129 anchors) for E069. The multi-page range is acceptable given the rule covers the whole "Precedence Rules" section.
- G13 content-ignorance: both evaluators use static `&'static str` messages — no document bytes interpolated. The variant_kind discriminator helpers (`rules.rs:4712-4720` and `:4831-4836`) deliberately return `u8` discriminants, not contained values.

## 4. CIA-CREST spot-check (HIGH concern — investigative)

Cross-walked 3 of the 58 firings against the actual fixture text in `tests/corpus/documents/marked/`:

- **CIA-RDP01M00147R000100350002-7** — pinned at `E068` count=2. By hand-trace of the document text, page 1 (banner `SECRET//NOFORN/PROPIN`, portions `(U//FOUO)`, `(U//NF/PR//SBU-NF)`, `(S//NF)`) projects to Secret; banner is Secret; **E068 should NOT fire on page 1**. Page 2 (banner `TOP SECRET//HCS-O//ORCON/NOFORN`, max portion class TS) is also consistent. Pin of count=2 looks like it captures real firings the implementer accepted as "correct" without per-fixture verification. **This is the over-firing concern flagged in your task brief.**
- **topofficialsinru00wash** — pinned at `E068` count=1 and `E069` count=1. Banner `SECRET//RD-CNWDI//FGI GBR NZL//NOFORN/PROPIN`; FgiSet from `(S//FGI GBR NZL//NF)` portions = `{GBR, NZL}`. Banner shows GBR NZL. **E069 should NOT fire by hand-trace.** Could be: (a) the projection unions across all document pages rather than resetting per page-break, OR (b) one of the portion-mark forms in this document is parsed in a way that drops a country.
- **CIA-RDP01M00147R000100350002-7 — DUPLICATE ENTRY**: `corpus_accuracy.rs:535-547` has TWO identical `ExpectedRuleCount{rule:"E068", count:2}` entries. The `expected_by_rule: HashMap` insertion at line 1442-1444 silently overwrites, so the duplicate doesn't break the test — but it's a code smell that escaped review. **MEDIUM finding.**

**Recommendation**: Before merge, the implementer should re-verify the 58 firings per-fixture against §H.7 — either by hand or by attaching a "what does projection produce vs. what does banner observe" diagnostic dump per fixture. If even one fixture is genuine over-firing, that should be filed as a follow-up issue (NOT a PR 5 blocker per PM Addendum I.4 scope authorization) but documented.

## 5. Findings

- **CRITICAL**: None.
- **HIGH**: The 58 CIA-CREST E068/E069 firings claimed as "correct" warrant per-fixture verification before merge. Without running tests, my by-hand traces of 2 of 3 spot-checked fixtures suggest at least one over-firing case. (See §4.)
- **MEDIUM**: Duplicate `ExpectedRuleCount` entry at `crates/engine/tests/corpus_accuracy.rs:535-547` (two identical E068 count=2 entries). Tolerated by HashMap-insertion shadow, but should be merged to a single entry.
- **MEDIUM**: T059b "NOFORN-supremacy composition" is asserted only indirectly (via E069 missing-marker arm firing). A direct assertion on FGI marker presence in the projection would strengthen the gate.
- **LOW**: Unit test `e068_fires_when_banner_missing_classification_and_page_has_one` (line 90 of `foreign_banner_rules.rs`) admits in its doc-comment that the `(None, Some(_))` arm is unreachable via current scanner output and falls back to testing the level-mismatch case. The defensive arm in the evaluator is therefore code-coverage-claimed but not behaviorally exercised. Acceptable per its defensive role.
- **LOW**: Constitution VII §IV touch: `crates/engine/tests/corpus_accuracy.rs` is `tests/` (test-fixture carve-out per Constitution V Principle V applies); modifications are pure `ExpectedRuleCount` literal data, not engine code. PASS.
- **LOW**: `issue: 0` pattern for new E068/E069 pins is consistent with existing "no issue (correct firing)" pattern (`corpus_accuracy.rs:524`). Acceptable.

## Top concern (single line)

The 58 CIA-CREST E068/E069 firings claimed as "correct" need per-fixture verification — my hand-trace of `topofficialsinru00wash` and `CIA-RDP01M00147R000100350002-7` suggests at least one is over-firing relative to §H.7. PM scope authorization allows deferral to a follow-up issue, but the claim "correct firing" should be substantiated.

## Files touched (worktree-relative)

- `crates/capco/src/rules.rs` (E068/E069 catalog rows + evaluators at `:4229-4274`, `:4665-4903`; walker `additional_emitted_ids` at `:4149-4163`)
- `tests/corpus/foreign/*.{txt,expected.json}` (5 fixtures)
- `crates/capco/tests/foreign_corpus.rs` (5 corpus tests)
- `crates/capco/tests/foreign_banner_rules.rs` (7 unit tests)
- `crates/capco/tests/proptest_page_rollup.rs` (FGI/NATO/JOINT proptest extension at `:39-103`)
- `crates/engine/tests/corpus_accuracy.rs` (58 E068/E069 expectations; duplicate at `:535-547`)
- `tools/regression-grep/regression-grep.sh` (Guard 2 for `MarkingClassification::Us\s*[({]` at `:122-127`)
- `crates/capco/tests/post_3b_registration_pin.rs` (count stays at 38; correct architecturally despite PM Addendum I.6 saying 40)
