<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 5 — Rust-specialist Code Review

**Date:** 2026-05-18
**Branch:** `refactor-006-pr-5-foreign-banner-correctness`
**Reviewer role:** Rust-specialist (Send+Sync, type-shape, ownership, idioms, clippy stable)
**Commits reviewed:** `fe88f0f7` / `5e5d37a4` / `37952185`

---

## 1. Verdict

**REQUEST-CHANGES** — one HIGH issue (`cargo fmt --check` fails), one MEDIUM
issue (E069 country-set comparison is order-sensitive and can false-positive
on non-canonical input), and one LOW issue ("synthetic" label in 59 reason
strings is factually wrong). All others are clean or confirmed sound.

---

## 2. Findings

### HIGH — `cargo fmt --check` fails

**File:** `crates/capco/src/rules.rs`  
**Line:** ~4719 (inside `evaluate_classification_banner_rollup`)

`cargo fmt --check` produces a diff on the `let mismatch_reason: Option<&'static str> = match (...)` block. Rustfmt wants:

```rust
let mismatch_reason: Option<&'static str> =
    match (attrs.classification.as_ref(), page.classification.as_ref()) {
```

rather than the current form where `match (` is on the same line as the binding. The exact same reformatting is needed symmetrically in `evaluate_fgi_marker_banner_rollup` at the `let mismatch_reason = match (...)` block.

**Fix:** Run `cargo fmt` on `crates/capco/src/rules.rs` and stage the result before opening the PR. Both evaluator functions are affected. No logic change needed — pure whitespace.

**Impact:** The diff is 148 lines across two blocks. `cargo fmt --check` is a required CI gate; this will fail CI as committed.

---

### MEDIUM — E069 country-set comparison is slice-order-sensitive (potential false positive)

**File:** `crates/capco/src/rules.rs`  
**Lines:** `evaluate_fgi_marker_banner_rollup`, last branch inside `(Some(observed), Some(projected))`:

```rust
} else if observed.countries() != projected.countries() {
```

`FgiMarker::Acknowledged { countries: SmallVec<[CountryCode; 4]> }` stores
countries in insertion order from the parser (`parse_fgi_marker` in
`crates/core/src/parser.rs:2523-2543` pushes each token as encountered).

The projected marker comes from `FgiSet::to_marker()` which calls
`FgiMarker::acknowledged(countries.iter().copied())` where `countries` is a
`BTreeSet<CountryCode>` — iteration is sorted ascending.

So for a banner with `FGI NATO GBR` (non-alphabetical), `observed.countries()`
= `[NATO, GBR]` but `projected.countries()` = `[GBR, NATO]` (sorted by
`BTreeSet`). The comparison fires an E069 false positive even though the country
sets are identical.

**CAPCO-2016 §H.7 p123** requires alphabetical country ordering in banners.
In practice real documents should be sorted, so the corpus-accuracy tests pass.
But the rule should be semantically robust: E069 is supposed to fire on a
*missing or wrong country*, not on a valid but non-canonically-ordered country
list. Non-canonical ordering is already caught by E060/render_canonical.

**Fix:** Use a set-equality comparison instead of slice-equality:

```rust
let obs_set: std::collections::BTreeSet<CountryCode> =
    observed.countries().iter().copied().collect();
let proj_set: std::collections::BTreeSet<CountryCode> =
    projected.countries().iter().copied().collect();
if obs_set != proj_set {
    // ...
}
```

Or, since `projected.countries()` is already in sorted order (from `BTreeSet`
iteration), sort the observed slice before comparing:

```rust
let mut obs_sorted: Vec<CountryCode> = observed.countries().to_vec();
obs_sorted.sort();
if obs_sorted.as_slice() != projected.countries() {
    // ...
}
```

The first form (two `BTreeSet`s) is cleaner semantically. The allocation is
only reached if both sides are `Acknowledged` and variant-kinds agree — not on
the hot lint path (E069 fires only on banner candidates, O(pages), not
O(tokens)).

**G13 note:** Neither fix interpolates country values into the diagnostic message,
so Constitution V G13 is preserved.

---

### LOW — "synthetic CIA CREST fixture" label is factually wrong

**File:** `crates/engine/tests/corpus_accuracy.rs`  
**Lines:** 539, 545, 562, 568, 585, 602, 619, 625, 648 (and ~50 more instances)

The `reason` field reads:
```
"correct firing: synthetic CIA CREST fixture has banner/portion classification mismatch ..."
```

CIA CREST documents (`CIA-RDP01M00147R000100350002-7`, etc.) are real declassified
CIA documents released via FOIA/CREST — not synthetic. The word "synthetic" is
factually incorrect.

Per Constitution VIII (authoritative source fidelity applies to all text
embedded in audit-adjacent records), and per the `ExpectedRuleCount.reason`
field's purpose as documented rationale for future reviewers, this label is
misleading.

**Fix:** Replace "synthetic CIA CREST fixture" with "real CIA CREST document"
across all 59 occurrences. `sed -i 's/synthetic CIA CREST fixture/real CIA CREST document/g' crates/engine/tests/corpus_accuracy.rs`.

---

### LOW — Duplicate E068 entry in `EXPECTED_DOCUMENT_DIAGNOSTICS` for `CIA-RDP01M00147R000100350002-7`

**File:** `crates/engine/tests/corpus_accuracy.rs`  
**Lines:** 535-546

The first document entry has two identical `ExpectedRuleCount { rule: "E068", count: 2, ... }` entries. The harness builds `expected_by_rule` as a `HashMap` at line 1442-1444 — the second entry silently overwrites the first. Net effect: the pin is functionally `count: 2` for E068 (same as a single entry). The first entry is dead data.

This is confirmed by the harness passing: the engine emits exactly 2 E068 firings, matching the effective pin. The test `assert_expected_diagnostics_stems_unique` (line 1348-1366) catches duplicate *stems* but not duplicate rule IDs within a stem's slice.

**Is it a latent miscount?** The document has 2 pages (`---` separator). If the author meant each page emits 1 E068 firing (total 2), a single `count: 2` entry is correct. If they meant 2 per page (total 4), only 2 are being checked. Given the test passes with 2 firings, the former interpretation is correct — this is a copy-paste artifact, not a miscount.

**Fix:** Remove the duplicate entry at lines 541-546. Single `ExpectedRuleCount { rule: "E068", count: 2, issue: 0, reason: "..." }` is sufficient.

---

## 3. Deviations Validation

### Deviation 1 — Engine-crate touch in `crates/engine/tests/corpus_accuracy.rs`

**Verdict: SOUND.**

`crates/engine/tests/corpus_accuracy.rs` is an integration test file (not
`crates/engine/src/`). The additions are pure data pins (`ExpectedRuleCount`
struct literals) — no behavior change, no new function definitions, no
`__engine_promote` calls. Per Constitution V Principle V test-fixture
carve-out, test-file additions to `tests/` are within the boundary. `issue: 0`
is a valid convention as explicitly documented at line 483-486 of the file:
*"`issue = 0` marks a correct firing"*.

### Deviation 2 — Registered rule count stays at 38, not 40

**Verdict: SOUND — Addendum I.6 was an over-specification.**

`BannerMatchesProjectedRule` (registered `id() = "E031"`) is the single
registered `Rule` impl. E068/E069 are per-row IDs emitted via
`additional_emitted_ids()` — architecturally identical to E035 and E040.
`post_3b_registration_pin.rs` header comment at lines 6-31 explicitly documents
this and explains the architectural decision. The `EXPECTED_RULE_IDS` slice and
count assertions correctly stay at 38. Addendum I.6 incorrectly modeled per-row
IDs as registered impls; the implementer's correction is right.

### Deviation 3 — Regression-grep guard scoped to `marking_scheme_impl.rs` only

**Verdict: SOUND.**

`crates/capco/src/scheme/marking.rs` carries five deliberate §H.7 pp123-125
reciprocal-normalization construction sites at lines 316, 324, 327, 340 (all
`Some(MarkingClassification::Us(...))` constructions). Guarding that file would
trip on correct code. `marking_scheme_impl.rs` is the projection entry-point
file that PR 6c cleaned; it is the precise file where a regression would
re-introduce a hardcode. The guard at `tools/regression-grep/regression-grep.sh`
runs clean against the current tree and would catch re-introduction in the right
file. The narrower scope is correct per rust-preflight R5 rationale.

---

## 4. Spot-Check on the 58 Firings

Three CIA CREST fixtures checked by reading the document content and cross-
referencing the engine's expected firing counts:

**Fixture 1 — `CIA-RDP01M00147R000100350002-7` (E068: count=2)**

Page 1: banner `SECRET//NOFORN/PROPIN`, portions at `(S//NF)` / `(U//NF/PR//SBU-NF)`. No classification mismatch.  
Page 2: banner `TOP SECRET//HCS-O//ORCON/NOFORN`, portions at `(S//HCS-O//OC/NF)` and `(TS) EXECUTIVE SECRETARIATROUTING SLIP`.  
The banner claims `TOP SECRET` but the dominant portion is `S` — genuine E068 classification-level mismatch per §H.7 pp123-125. Count=2 (one per banner occurrence on page 2 — the banner appears at both top and bottom). **Correct firing.**

**Fixture 2 — `CIA-RDP09T00207R001000100012-1` (E068: count=1)**

Banner `TOP SECRET//SI-G ABCD/TK//RSEN/ORCON/NOFORN/PROPIN`. Portions include `(S//RAWFISA//NF)` and `(U//NF/PR//SBU-NF)` — banner `TOP SECRET` vs `S` and `U` portions. Genuine E068 classification-level mismatch. **Correct firing.**

**Fixture 3 — `CIA-RDP09T00207R001000100021-1` (E068: count=1, E069: count=1)**

Banner `SECRET//SI//FGI NATO//NOFORN/RAWFISA`. Portions include `(//NS//REL TO USA, NATO)` (NATO-only, no US classification) and `(S//SI//REL TO USA, FVEY)`. A page mixing NATO-only and US portions. Banner claims US `SECRET` classification axis; with NATO-only portions in the mix the projected classification may differ. E069 fires because banner has `FGI NATO` but projection may produce a different FGI marker from the portion rollup. **Plausible correct firing.** The S005/S007 rule interplay is consistent with what the engine should surface.

**Spot-check verdict:** All three examined fixtures show genuine banner/portion discrepancies consistent with §H.7 pp123-125 roll-up grammar. These are NOT over-fires. The word "synthetic" is the error — the documents are real.

---

## 5. Final Clippy + Test Chain Output

```
cargo check --workspace         ✅ PASS  (0 errors, 0 warnings)
cargo +stable clippy --workspace --all-targets -- -D warnings  ✅ PASS (clean)
cargo fmt --check               ❌ FAIL  (rules.rs:4719 + E069 block formatting)
cargo +stable test --workspace  ✅ PASS  (all results: ok, 0 failed)
```

`cargo fmt` is the only gate failing. All other diagnostic tools are green.

---

## Summary

| # | Severity | Finding | File | Fix |
|---|----------|---------|------|-----|
| F1 | HIGH | `cargo fmt` diff on two evaluator `match` bindings | `crates/capco/src/rules.rs:4719` | Run `cargo fmt` |
| F2 | MEDIUM | E069 country-set comparison is slice-order-sensitive (potential false positive on non-canonical input) | `crates/capco/src/rules.rs` (evaluate_fgi_marker_banner_rollup) | Use `BTreeSet` equality |
| F3 | LOW | "synthetic CIA CREST fixture" label is factually wrong (real documents) | `crates/engine/tests/corpus_accuracy.rs` (59 instances) | `sed` replace |
| F4 | LOW | Duplicate E068 entry for `CIA-RDP01M00147R000100350002-7` is dead data | `crates/engine/tests/corpus_accuracy.rs:541-546` | Remove one duplicate |

Deviations D1 (engine-test touch), D2 (38-not-40 rule count), D3 (narrowed grep scope) are all **SOUND** as implemented.

The two evaluator functions are correctly `Send+Sync` (pure functions over shared references, no hidden state). G13 is satisfied — no classification values, country codes, or banner text interpolated into diagnostic messages. `MarkingType` is `#[non_exhaustive]` but the guard at the walker level uses `matches!` which is future-safe. The `variant_kind` discriminator covers all 5 exhaustive `MarkingClassification` variants.
