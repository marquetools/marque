<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 5 — Architect Preflight (Foreign Banner Correctness)

**Date:** 2026-05-18
**Branch:** `refactor-006-pr-5-foreign-banner-correctness` off `origin/staging` HEAD `7d4ad231` (post-PR-6c).
**Author:** ecc:architect (preflight pair with `2026-05-18-pr5-rust-preflight.md`).
**Status:** Pre-implementation — feeds PM decisions doc `2026-05-18-pr5-pm-decisions.md`.

> The agent's own operating instructions forbid writing analysis `.md`
> files directly, so this content was returned inline in the agent's
> result and persisted by the PM to the worktree. Content reproduced
> verbatim modulo formatting.

## (a) Bug status — empirical

**The three #276 cases and #261 are all empirically open today, but for a different reason than the original 2026-05-02 framing.** The lattice (`crates/capco/src/scheme/marking.rs:225-625`) already computes correct foreign-banner state: `FgiSet::from_attrs_iter` (`crates/capco/src/lattice.rs:3197-3273`) unions per-portion `fgi_marker` with classification-derived producers; the §H.7 pp123-128 reciprocal-raise, concealed-dominates, and G-4c source-loss-reconstruction phases are all live. `CapcoScheme::project(Scope::Page, …)` (`crates/capco/src/scheme/marking_scheme_impl.rs:205-258`) drives this. The classification renderer (`crates/capco/src/render/render_classification.rs:60-148`) renders `//FGI`, `//DEU SECRET`, `//NATO SECRET`, etc. correctly.

**What's missing is a rule that compares the observed banner against `ctx.page_marking` on the classification + FGI-marker axes.** `BannerMatchesProjectedRule` (`crates/capco/src/rules.rs:4080-4156` + catalog at `:4187-4221`) covers only SAR (E031), SCI (E035), and Non-IC dissem (E040). When a portion-set computes a projected banner of `SECRET//FGI DEU//NOFORN` and the document's banner reads `SECRET//NOFORN`, no rule fires — the banner-classification and banner-FGI-marker rows do not exist in `BANNER_CATEGORY_CATALOG`.

This is what changed: the 2026-05-02 plan blamed `MarkingClassification::Us` at `scheme.rs:365`. That hardcode and `expected_classification` are both **already deleted** (PR 4b-E commit `ef7de07f`, PR 6c commit `6fee9818`). `ProjectedMarking.classification: Option<MarkingClassification>` (`crates/ism/src/projected.rs:72`) is the post-deletion shape T059 named. **T059 + T060 are structurally complete by deletion, not by edit.** The substantive bug closure work is at the rule-detection layer, not the projection layer.

#261 (`(//FGI DEU R//REL TO USA, DEU)` → drop `FGI`) is a **renderer/parser-canonicalization** concern: the parser admits both `//FGI DEU` and `//DEU` shapes; the canonical form per §H.7 p123 is "trigraph present ⇒ drop FGI." `FgiClassification` already carries `.countries`; the parser path can fold `FGI + countries` → `Fgi(FgiClassification { countries })` at admission. Alternatively, a `Severity::Fix` rule scans portion-mark source spans for a `//FGI [LIST]` shape and emits a `FactRemove(TOK_FGI_MARKER)` over the bare `FGI` token span.

## (b) Reframed task list

| Old | Status | New |
|---|---|---|
| T059 widen `expected_classification` to `Option` + delete hardcode | **Complete by retirement (4b-E)** | Mark Done in `tasks.md`; cite PR 4b-E commit `ef7de07f` |
| T060 update `page_context_to_attrs` to preserve foreign | **Complete by retirement (6c)** | Mark Done; cite PR 6c commit `6fee9818` (rename to `project_from_attrs_slice`) |
| T061 `FgiSet::render_canonical` drop FGI when trigraph | **Open — relocated** | **T061a**: parser-side canonicalization of `//FGI [LIST]` portion form → `//[LIST]` (canonical per §H.7 p123 worked example p126 `(//CAN DEU S)` shape). Source-fix-target: `crates/core/src/parser.rs::parse_fgi_marker` + `parse_fgi_classification`. **Alternative: C-class rule** in `marque-capco`. |
| T062 `pure_foreign_banner.json` corpus fixture | **Open** | Stays. Add to `tests/corpus/foreign/` (new directory) |
| T063 `joint_us_uk.json`, `nato_only_page.json` | **Open** | Stays |
| T063a `mixed_us_foreign_rollup.json` | **Open** | Stays — verified worked example at §H.7 p129 (`TOP SECRET//FGI CAN DEU//NOFORN`) backs the four invariants |
| T064 CI grep guard | **Open — reframed** | **T064a**: CI grep for `MarkingClassification::Us` *literal construction* inside `project*`, `join_via_lattice*`, and the engine accumulator paths. Today's code is clean; the guard prevents reintroduction |
| — | **NEW** | **T059a**: extend `BannerMatchesProjectedRule`'s `BANNER_CATEGORY_CATALOG` with two rows: `evaluate_classification_banner_rollup` (rule `E068`) and `evaluate_fgi_marker_banner_rollup` (rule `E069`). Each row compares `attrs.classification` / `attrs.fgi_marker` against `page.classification` / `page.fgi_marker` and emits Error-severity no-fix diagnostics when they disagree. Walker count: 39 → 41. |
| — | **NEW** | **T059b**: NOFORN-supremacy case verification for the projection path. The §H.7 + §H.8 p145 `capco/noforn-clears-rel-to` PageRewrite must compose correctly with FGI preservation — verified by the T063a fixture. No code change expected; behavioral assertion only. |

## (c) Per-commit shape (5 atomic commits)

1. **C1 — Mark T059/T060 Done + reframe T064 in `tasks.md`**. Plan + spec edits only; zero code touch. Cites retirement commits.
2. **C2 — New corpus fixtures** (`tests/corpus/foreign/{pure_foreign_banner,joint_us_uk,nato_only_page,mixed_us_foreign_rollup,fgi_concealed,fgi_redundant_token}.json`). Pre-fix capture: fixtures land "red" with no E068/E069 in observed diagnostics so C3 turns them green.
3. **C3 — Add classification + fgi_marker banner-rollup catalog rows** (`crates/capco/src/rules.rs::BANNER_CATEGORY_CATALOG`; add `evaluate_classification_banner_rollup` + `evaluate_fgi_marker_banner_rollup`). Update `EXPECTED_RULE_IDS` in `post_3b_registration_pin.rs`; update count pin (39 → 41). Fixtures from C2 turn green. Zero engine-crate touch.
4. **C4 — #261 FGI-redundant-token canonicalization** (`crates/core/src/parser.rs::parse_fgi_classification`). Touches `marque-core` (Constitution VII §IV). Update `crates/capco/tests/parse_render_roundtrip.rs` and `crates/core/tests/fgi_silent_skip_guard.rs` round-trip cases.
5. **C5 — CI grep guard + plan close-out** (`tools/regression-grep/regression-grep.sh`: add pattern `MarkingClassification::Us\s*[({]` inside `crates/capco/src/scheme/` and `crates/engine/src/`).

## (d) Constitution VII §IV authorization

**C1 + C2 + C3 + C5: no engine-crate touch.** C3 is a domain rule-crate addition.

**C4 touches `marque-core` (engine crate per Principle VII).** Within-006 precedent is consistently `marque-ism` (4b-B C2 / 4b-C C5 / 4b-D.2 / 4b-E), not `marque-core`. **Request PM approval for C4 specifically** — if PM declines, fall back to a C-class rule (`C002 fgi-redundant-token-suggest`) in `marque-capco`.

## (e) §-citation pre-verification

All citations re-verified against `crates/capco/docs/CAPCO-2016.md` on 2026-05-18:

- **§H.7 p122** lines 3024-3036 — "FGI portion marks always start with a double forward slash" + grammar definition.
- **§H.7 p123** lines 3047-3053 — banner abbreviation `FGI [LIST]` (acknowledged) vs `FGI` (concealed); portion-mark `[LIST] [Non-US Classification Portion Mark]`.
- **§H.7 p124** line 3099 — "If any document contains portions of both source-concealed FGI … and source-acknowledged FGI, then only the 'FGI' marking without the source trigraph(s)/tetragraph(s) must appear in the banner line".
- **§H.7 p126** line 3131 worked example `TOP SECRET//FGI CAN DEU//REL TO USA, CAN, DEU`.
- **§H.7 p127** line 3142 worked example `TOP SECRET//BOHEMIA//FGI AUS CAN DEU NATO//NOFORN`.
- **§H.7 p128** line 3153 worked example `SECRET//FGI//NOFORN` from `(S//RELIDO) + (//DEU S//NF) + (//DEU C//REL TO USA, CAN, DEU) + (//FGI S//NF)`.
- **§H.7 p129** line 3168 worked example `TOP SECRET//FGI CAN DEU//NOFORN`.

## (f) Test coverage plan

- **Unit (lattice)**: `crates/capco/tests/rollup_golden.rs` already exercises `render_banner_from_portions` via the scheme path; add four golden tests using the three #276 reproductions + the §H.7 p128 worked-example portion-set.
- **Rule fire/no-fire**: `crates/capco/tests/foreign_banner_rules.rs` (new) — asserts E068 fires on missing-FGI classification and E069 fires on missing FGI marker; asserts neither fires on correctly-formed banners. AAA structure.
- **Parity gate**: `crates/capco/tests/lattice_vs_scheme_parity.rs` — add 4 fixtures.
- **Round-trip (#261)**: `crates/capco/tests/parse_render_roundtrip.rs`.
- **Behavior over implementation**: every test asserts banner-string output or diagnostic ID emission.
- **Coverage target ≥ 80%**: six fixture cases × two evaluators × {match, mismatch} branches → full branch coverage.

## (g) Top OQs for PM

1. **OQ-1 (BLOCKING C4): Parser-side canonicalization vs. C-class rule for #261?** Parser-side requires `marque-core` touch and PM authorization under Principle VII §IV. C-class rule avoids engine-crate touch but pays per-portion-scan overhead. The bugfix-class precedent for `marque-core` touches within 006 is thinner than for `marque-ism`.
2. **OQ-2 (BLOCKING C3 severity choice): E068/E069 severity = Error no-fix, or Warn with structured-fix-suggest?** SAR's no-block case uses Error/no-fix because "byte-positioning a new SAR block from rule context alone is unsafe."
3. **OQ-3 (CLARIFICATION): Should T064 (CI grep guard) cover only `MarkingClassification::Us` construction, or also `expected_classification`-shaped APIs that could be reintroduced via convenience accessors?**

## Citations

Worktree-relative paths only. Cite specific file paths and line numbers from `/home/knitli/marque/.claude/worktrees/pr-5-foreign-banner/`.
