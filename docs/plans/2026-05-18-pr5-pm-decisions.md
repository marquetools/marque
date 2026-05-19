<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 5 — PM Decisions (Foreign Banner Correctness)

**Date:** 2026-05-18
**Branch:** `refactor-006-pr-5-foreign-banner-correctness` off `origin/staging` HEAD `7d4ad231` (post-PR-6c).
**Companions:**
- `2026-05-18-pr5-architect-plan.md` (architect strategic plan)
- `2026-05-18-pr5-rust-preflight.md` (rust risk register)
**Status:** Locked — feeds implementation agent.

> **Amended 2026-05-18 (Addendum I).** Implementation-time discovery
> by the first implementer pass surfaced three load-bearing surprises
> that invalidate the original C3/C4 contract and require a scope
> collapse. The implementer correctly halted at zero commits made
> rather than fabricate. See §"Addendum I — Implementation-time
> surprises" at the bottom of this document for the corrections; the
> body of the doc below is preserved verbatim for audit continuity,
> but Addendum I is operative.

## Reconciliation of the two preflights

The two preflight agents reached **partially-overlapping conclusions at different layers**. Empirical inspection of `crates/capco/src/scheme/marking.rs:270-282` against the three #276 reproductions settles the load-bearing question:

| #276 Case | `has_us_class` | `has_non_us_class` | `solely_non_us` | Reciprocal-raise fires? | Projected classification | Verdict |
|---|---|---|---|---|---|---|
| 1 — Pure-foreign `(//DEU S//REL TO USA, DEU)` only | `false` | `true` | `true` | NO | `Fgi(Secret, [DEU])` | **Correct** |
| 2 — Commingled `(//DEU S) + (S)` | `true` | `true` | `false` | YES (correct per §H.7 pp123-125) | `Us(Secret)` + FgiSet carries `DEU` | **Correct** |
| 3 — Commingled with NF | `true` | `true` | `false` | YES | `Us(Secret)` + FgiSet carries `DEU` + NOFORN clears RELTO | **Correct** |

**Conclusion:** The architect's diagnosis wins. **Projection is correct for the three #276 reproductions.** The bug is what the architect named: **no rule fires when the observed banner disagrees with `ProjectedMarking.classification` / `ProjectedMarking.fgi_marker`** because `BANNER_CATEGORY_CATALOG` (`rules.rs:4187-4221`) covers only SAR/SCI/Non-IC dissem.

The rust-specialist's R3 (`solely_non_us` gate predicate / `Joint(_)` counting) is **a latent but separate question** affecting a different malformed-input class (e.g., `Joint(GBR, DEU)` with no USA, which is itself a §H.3 p56 violation). It is **out of scope for PR 5** and lands as a separate tracked issue if a fixture exposes it.

The rust-specialist's R2 (T061 surface = `marque-capco`-internal, NOT `FgiSet::render_canonical` and NOT a `MarkingScheme` trait method) **also wins** — the task plan's literal naming was wrong. The clean shape is a `marque-capco`-internal extension to PR 3b.F's existing `DeclarativeNonCanonicalInputRule` walker (E060) since #261 is a non-canonical-input concern (input bytes ≠ canonical render) and that walker already exists for exactly this class of fix.

## PM Decisions

### D-5.1 — Scope frame

**T059 + T060 are complete by retirement.** PR 4b-E (#539, commit `ef7de07f`) deleted the `expected_*` accessor surface; PR 6c (#547, commit `6fee9818`) deleted `PageContext` itself. The literal targets named in T059/T060 do not exist. Mark both `[X]` in `tasks.md` with the retirement-commit citations.

**T061 is reframed to T061a (renamed in tasks.md):** drop the redundant `FGI` token when a trigraph is present per §H.7 p123. Implementation surface is a **new declarative row in PR 3b.F's `DeclarativeNonCanonicalInputRule` walker (E060)** at `crates/capco/src/rules_declarative.rs`, NOT `FgiSet::render_canonical` (which conflates an axis-lattice helper with a `MarkingScheme` trait method — rust-specialist R2). The walker already supports per-row severity (`Severity::Fix` for rows 1-4 per the existing catalog).

**T064 is reframed to T064a:** CI grep guard for `MarkingClassification::Us\s*[({]` construction inside specific files only — `crates/capco/src/scheme/marking.rs`, `crates/capco/src/scheme/marking_scheme_impl.rs`, and any future `project*` / `join_via_lattice*` files. Discriminator matches (`Some(MarkingClassification::Us(_))`) are pattern matches with `_`, not construction — the regex anchors `[({]` after `Us` which discriminator matches don't have.

**NEW T059a:** Add E068 (`banner-classification-mismatch`) + E069 (`banner-fgi-marker-mismatch`) catalog rows to `BANNER_CATEGORY_CATALOG`. Walker count: 39 → 41 — update `EXPECTED_RULE_IDS` in `post_3b_registration_pin.rs` and the count pin.

**NEW T059b:** NOFORN-supremacy composition assertion (`mixed_us_foreign_rollup.json` fixture). No code change — pure behavioral assertion via the corpus fixture.

### D-5.2 — Constitution VII §IV authorization

**No engine-crate touch.** Per rust-specialist's recommended posture: all five commits land inside `marque-capco` + `tests/corpus/foreign/` + `tools/regression-grep/`. This is a scheme-adoption PR (Constitution VII §IV) staying inside the scheme-adoption boundary.

The architect's C4 alternative (parser-side `marque-core` touch) is **rejected**. The rust-specialist's framing of #261 as a non-canonical-input concern handled by extending E060 is cleaner: (a) leverages the PR 3b.F walker pattern, (b) keeps the work scheme-internal, (c) emits a Fix diagnostic with full audit-record provenance, (d) ships fix confidence ≥ 0.95 (auto-applies under default config). The architect's parser-side approach would have folded `FGI` silently and made the audit-record story muddier.

### D-5.3 — OQ Resolutions

| OQ Source | Question | Resolution |
|---|---|---|
| Architect OQ-1 | Parser vs C-class for #261 | **C-class via E060 walker extension.** No `marque-core` touch. Constitution VII §IV preserved. |
| Architect OQ-2 | E068/E069 severity (Error/no-fix vs Warn/suggest) | **`Severity::Error` no-fix** for both E068 + E069, matching the SAR no-block precedent at `rules.rs:4337-4360`. Cross-axis byte-positioning a missing classification or FGI block from rule context is unsafe; deterministic fix requires renderer-level coordination not yet wired. Warn/suggest would surface a partial replacement that could lose foreign-source provenance — Error/no-fix is the safer default. |
| Architect OQ-3 / Rust OQ-3 | CI grep guard scope | **Pin construction sites in specific files** (rust-specialist's narrower recommendation). Pattern: `MarkingClassification::Us\s*[({]` scoped to `crates/capco/src/scheme/` + `crates/engine/src/{engine,decoder,recognizer}.rs`. The discriminator-match shape `Some(MarkingClassification::Us(_))` lacks the `[({]` follow which the regex requires, so legitimate matches don't trip. |
| Rust OQ-1 | `solely_non_us` gate semantics (`Joint(_)` counting) | **No change in PR 5.** The current §H.3 p56 reading is correct (JOINT requires USA in producer list; `Joint(_)` counting as US-bearing matches the spec). Latent question for a separate follow-up issue if a fixture exposes a genuine misbehavior. |
| Rust OQ-2 | T061 surface (FgiSet::render_canonical vs render_fgi.rs vs E060) | **Extend E060 `DeclarativeNonCanonicalInputRule` walker** with a 6th declarative row for FGI-redundant-token canonicalization. Rust-specialist's `render_fgi.rs` shape is also viable but less consistent with the established walker pattern. |
| Rust OQ-4 | Proptest extension | **Include in PR 5.** Extending `arb_classification` in `proptest_page_rollup.rs` to include FGI/NATO/JOINT variants is ~10 LoC and load-bearing for catching foreign-classification regressions. Cheap to add. |

### D-5.4 — Operative commit shape (locked, 5 atomic commits)

Each commit MUST be tree-green: `cargo +stable test --workspace` and `cargo +stable clippy --workspace --all-targets -- -D warnings` pass on every commit. All commits GPG-signed.

1. **C1 — tasks.md + planning docs.** Mark T059/T060 as `[X]` with retirement-commit citations; rename T061 → T061a and update body to point at E060 walker; rename T064 → T064a and update body to scope grep regex; add T059a (E068 + E069 catalog rows); add T059b (NOFORN-supremacy composition assertion). **Zero code touch.**

2. **C2 — Corpus fixtures + proptest extension** (`tests/corpus/foreign/{pure_foreign_banner,joint_us_uk,nato_only_page,mixed_us_foreign_rollup,fgi_concealed,fgi_redundant_token}.json`; `crates/capco/tests/proptest_page_rollup.rs` extended with FGI/NATO/JOINT variants in `arb_classification_any_variant`). Pre-fix capture: the `*.expected.json` files for the four #276-pattern fixtures land "red" — the expected diagnostics list includes E068 / E069 but no rule yet emits them, so the fixtures fail until C3. Acceptable per the "fixtures land red, code turns them green" precedent from PR 4b-C Commit 6 parity-gate landing. Zero engine touch.

3. **C3 — Add E068 + E069 banner-rollup catalog rows** (`crates/capco/src/rules.rs::BANNER_CATEGORY_CATALOG`; add `evaluate_classification_banner_rollup` + `evaluate_fgi_marker_banner_rollup` evaluators). Update `EXPECTED_RULE_IDS` in `crates/capco/tests/post_3b_registration_pin.rs` (47 → 49); update `rule_count_reflects_registration_changes` pin in `crates/capco/tests/corpus_parity.rs` (39 → 41). C2's red fixtures turn green. Per-evaluator unit tests in `crates/capco/tests/foreign_banner_rules.rs` (new). Zero engine touch.

4. **C4 — #261 via E060 walker extension.** Add a 6th `NonCanonicalRow` to `NON_CANONICAL_CATALOG` in `crates/capco/src/rules_declarative.rs` covering FGI-redundant-token canonicalization (`//FGI [LIST]` → `//[LIST]` when LIST is non-empty per §H.7 p123). Per-row §-citation: §H.7 p123 (verified). Severity: `Fix`. Confidence: ≥ 0.95 to auto-apply. Update `crates/capco/tests/parse_render_roundtrip.rs` with the redundant-FGI round-trip case. Zero engine touch.

5. **C5 — CI grep guard + plan close-out** (`tools/regression-grep/regression-grep.sh`: add the construction-site pattern). Verify `cargo test --workspace` is green; close out tasks.md with commit refs.

### D-5.5 — Citation discipline (Constitution VIII)

Every §-citation embedded in PR 5 code MUST be re-verified at edit time against `crates/capco/docs/CAPCO-2016.md`. The architect's pre-verification list (architect plan §e) is the canonical set; propagation into rule doc-comments, diagnostic messages, and the catalog `Constraint::Custom("class-floor/...", ...)` style citation MUST re-verify. Single §-per-row D13 discipline applies — one operative authority per row.

Citations expected:
- E068: `CAPCO-2016 §H.7 p124` (banner classification roll-up grammar; line 3032 "If a US document has portions with FGI markings ... roll-up the foreign control markings to the applicable marking category in the banner line after any US controls in that category").
- E069: `CAPCO-2016 §H.7 p126` (worked example line 3131 `TOP SECRET//FGI CAN DEU//REL TO USA, CAN, DEU`) OR `§H.7 p129` (worked example line 3168 `TOP SECRET//FGI CAN DEU//NOFORN`). Pick one, justify in the row comment.
- C4 (#261): `CAPCO-2016 §H.7 p123` (FGI portion-mark canonical form `[LIST] [Non-US Classification Portion Mark]`).

### D-5.6 — Test coverage targets

- **>80% branch coverage** on `evaluate_classification_banner_rollup` + `evaluate_fgi_marker_banner_rollup`. Pure functions over `(&CanonicalAttrs, &ProjectedMarking, &BannerCategoryRow) → Vec<Diagnostic>`. Six fixture cases × two evaluators × {match, mismatch} branches = full branch coverage.
- **Corpus fixtures** at `tests/corpus/foreign/` exercising all three #276 reproductions + source-concealed (§H.7 p124 line 3099) + commingled-concealed (§H.7 p128 line 3153) + redundant FGI (#261 case).
- **Parity gate** (`lattice_vs_scheme_parity.rs`): each new corpus fixture wires through the gate; any divergence MUST be documented with §-citation per PR 4b-E OQ-7 BLOCKING discipline.
- **Behavior over implementation**: every test asserts banner-string output or emitted-diagnostic-ID, never internal lattice state.

### D-5.7 — Performance budget

PR 5 adds two evaluator functions to the banner walker (called once per banner candidate) and one declarative row to the E060 walker (called once per portion). All work is **O(portions)** per page; no inner loops over byte arrays. `lint_10kb` baseline ~828µs with 10% threshold ~911µs (memory `project_bench_baseline_staleness`). PR 5 should land well under the threshold.

If `lint_10kb` fires close to threshold, follow the established mitigation: `gh run rerun <id> --failed` once; if it persists, request baseline refresh per memory `project_perf_baseline_pr5_trigger` (PR 5 is the trigger PR after which user commissions dedicated perf-analysis work).

### D-5.8 — Implementation agent brief

The implementation agent receives:
- This PM-decisions doc (locked contract).
- The architect plan (`2026-05-18-pr5-architect-plan.md`).
- The rust risk register (`2026-05-18-pr5-rust-preflight.md`).
- The full `crates/capco/CAPCO-CONTEXT.md` (mandatory PM directive, not as a link).
- The plan-mandate text from `docs/plans/2026-05-02-engine-refactor-consolidated.md` §4 PR 5 row + the consolidated plan §3 / §5 / §6 sections.
- The vendored `crates/capco/docs/CAPCO-2016.md` (for re-verifying every §-citation at edit time).

The agent's standing brief includes the PM directives:
- 5-year-maintenance test on every choice.
- Walk logical code-path adjacencies for every fix.
- Constitution V G13 content-ignorance binding.
- Constitution VIII citation-fidelity binding.
- Constitution VII §IV — D-5.2 above locks "no engine-crate touch" for PR 5.
- All commits GPG-signed.
- Each commit tree-green.
- `cargo +stable clippy --workspace --all-targets -- -D warnings` clean per commit.

## End (pre-amendment)

PR 5 lands as 5 atomic, GPG-signed, scheme-internal commits closing #276 (E068 + E069 banner-rollup) and #261 (E060 walker extension) with no engine-crate touch. The literal T059/T060 work is acknowledged as complete by retirement; T061/T064 are reframed; T059a/T059b are added. Foreign-banner correctness becomes a load-bearing invariant of the engine surface, enforced by rule + corpus + parity-gate + grep guard.

---

## Addendum I — Implementation-time surprises (2026-05-18)

Pre-implementation reading by the first implementer pass surfaced three
load-bearing discoveries that invalidate the original C3/C4 contract:

### I.1 — E060 walker is retired in PR 3c.B Commit 6

Per `crates/capco/src/rules.rs:96-101`:

> *"E060 = retired in PR 3c.B Commit 6 — non-canonical-input walker (5 ordering rows: REL TO USA-first §H.8 p150-151, JOINT alpha §H.3 p56, AEA SIGMA numeric sort §H.6 p108, SAR program ascending alpha §H.5 p99, SCI compartment + sub-compartment numeric-then-alpha §H.4 p61) absorbed by `MarkingScheme::render_canonical`"*

Plus a negative-set guard test at `rules.rs:5386-5387` actively asserting
`!ids.contains(&"E060")`. `NON_CANONICAL_CATALOG` / `NonCanonicalRow`
symbols do not exist in the worktree. The original PM contract C4 had no
extension point.

**Architectural commitment from PR 3c.B Commit 6:** `lint` no longer
surfaces non-canonical-input divergences; `fix` (renderer) still
produces canonical output. Adding a new Fix-emitting rule for the
#261 case (`(//FGI DEU R)` → `(//DEU R)`) would **re-litigate this
architectural commitment** — that's a separate concern from foreign
banner correctness and does not belong in PR 5.

### I.2 — Registered rule count is 38, not 47

`crates/capco/tests/post_3b_registration_pin.rs:139` asserts
`raw_len == 38`. `crates/capco/tests/corpus_parity.rs:241` asserts
`rule_set.rules().len() == 38`. The original PM contract C3's "47 → 49"
math was off by 9 rules (PRs 3c.B Commit 6 form-bucket migration +
Commit 7.3/7.4 bridge migration retired E058/E059/E020/E060 + ~13 form
rules; PR 4b-B added W004; PRs 9a/9c.1/9c.2 added E064-E067/S007; PR
#488 retired S006; PR closing #470 retired W002 — net 38).

### I.3 — `docs/refactor-006/regression-guards.md` does not exist

The path was a forward reference. Need to either create it as part of
C5 or drop the reference from tasks.md.

### I.4 — Corrected scope (operative for re-launch)

**PR 5 scope-collapses to #276-only.** #261 defers to a follow-up PR
that re-evaluates the renderer audit-trail story post-PR-3c.B Commit 6.

Rationale per "marque is pre-users — rewrite freely" (memory
`feedback_pre_users_no_deprecation_phasing`): #261's pre-PR-3c.B
framing (Fix-emitting rule with audit record) conflicts with PR 3c.B
Commit 6's architectural commitment. Re-litigating that commitment in
PR 5 expands scope and risks drift. Defer #261 to a dedicated PR after
auditing the `fix`-path audit-record story; if the renderer's
canonicalization writes audit records (likely — `AppliedFix` is engine-
promoted via `__engine_promote` from `Engine::fix_inner` which is where
the renderer runs), then #261 is closed architecturally and the
remaining concern is lint-time visibility only.

**Operative commit shape (locked, 4 atomic commits):**

1. **C1 — tasks.md + planning docs** (zero code touch)
   - Mark T059 + T060 as `[X]` with retirement-commit citations (T059 → PR 4b-E commit `ef7de07f`; T060 → PR 6c commit `6fee9818`).
   - **DROP T061 / T061a entirely from PR 5 scope.** Mark T061 with a deferral note pointing to a follow-up PR re #261, with §-citation to PR 3c.B Commit 6 retirement.
   - Rename T064 → T064a; scope grep regex `MarkingClassification::Us\s*[({]` to specific files.
   - Add T059a: E068 + E069 catalog rows. **Rule-count delta: 38 → 40.**
   - Add T059b: NOFORN-supremacy composition assertion via `mixed_us_foreign_rollup.json` fixture.
   - DROP T061a deferral note from tasks.md (#261 deferral).

2. **C2 — Corpus fixtures + proptest extension** (zero engine touch)
   - Five fixtures (drop the `fgi_redundant_token` fixture from PR 5):
     1. `pure_foreign_banner.{txt,expected.json}`
     2. `joint_us_uk.{txt,expected.json}`
     3. `nato_only_page.{txt,expected.json}`
     4. `mixed_us_foreign_rollup.{txt,expected.json}` (T059b anchor)
     5. `fgi_concealed.{txt,expected.json}` (§H.7 p124 line 3099 + p128 line 3153)
   - Extend `arb_classification` in `proptest_page_rollup.rs` with FGI/NATO/JOINT variants.
   - **Verify the corpus harness behavior with red fixtures.** Per `crates/capco/tests/corpus_parity.rs` inspection, decide whether to merge C2 + C3 if the harness requires a passing `.expected.json` companion at every fixture.

3. **C3 — E068 + E069 banner-rollup catalog rows** (zero engine touch)
   - Edit `BANNER_CATEGORY_CATALOG` to add two new `BannerCategoryRow` entries.
   - Update `EXPECTED_RULE_IDS` in `post_3b_registration_pin.rs`: **38 → 40** (add `"E068"`, `"E069"` — preserve alpha ordering in the slice).
   - Update `raw_len` and `expected.len()` and `actual.len()` assertions at `post_3b_registration_pin.rs:139` / `:158` / `:166`: **38 → 40**.
   - Update `rule_count_reflects_registration_changes` in `corpus_parity.rs:241`: **38 → 40** plus update the running-count derivation comment to add the E068+E069 line.
   - Create `crates/capco/tests/foreign_banner_rules.rs` with AAA-pattern per-evaluator unit tests for >80% branch coverage.
   - Verify `cargo +stable test --workspace` passes; C2 red fixtures (if any) turn green.

4. **C4 — CI grep guard + plan close-out** (renumbered from old C5)
   - Edit `tools/regression-grep/regression-grep.sh` (or the established CI tooling — verify the file exists, follow the existing pattern).
   - Pattern: `MarkingClassification::Us\s*[({]` scoped to specific files.
   - **For T064a deliverable:** since `docs/refactor-006/regression-guards.md` does not exist, either (a) create it as part of C4 with the regression guard documented, OR (b) drop the path reference from tasks.md. Implementer picks the simpler shape and surfaces the choice in the commit message.
   - Verify the guard catches the original `scheme.rs:365` synthetic shape; verify it does NOT trip on the current clean tree.

### I.5 — Updated D-5.3 OQ resolutions

| OQ Source | Question | Updated Resolution |
|---|---|---|
| Architect OQ-1 | Parser vs C-class for #261 | **#261 DEFERRED to a follow-up PR.** Neither parser nor C-class rule lands in PR 5. |
| Rust OQ-2 | T061 implementation surface | **Moot for PR 5** — T061 deferred with #261. |

All other OQ resolutions in D-5.3 stand unchanged.

### I.6 — Rule-count math for the implementer

The exact deltas for C3:
- `crates/capco/tests/post_3b_registration_pin.rs:139` line: `38` → `40`.
- `crates/capco/tests/post_3b_registration_pin.rs:158` line: `38` → `40` (EXPECTED_RULE_IDS expected length sanity).
- `crates/capco/tests/post_3b_registration_pin.rs:166` line: `38` → `40` (registered rule cardinality assertion).
- `crates/capco/tests/post_3b_registration_pin.rs:119` test function name: rename `post_pr_470_registers_exact_38_rule_ids` → `post_pr5_registers_exact_40_rule_ids`.
- Header comment derivation line at `:65-67` (the running count math): append PR 5 line "PR 5 (006 T059a / closing #276) adds E068 (banner-classification-rollup) + E069 (banner-fgi-marker-rollup) per §H.7 p124 + §H.7 p127 (38 → 40)."
- EXPECTED_RULE_IDS slice: insert `"E068"` after `"E067"`, `"E069"` after `"E068"`. (Preserve alphabetical ordering within the E0xx range.)
- `crates/capco/tests/corpus_parity.rs:241` count pin: `38` → `40`. Header running-count derivation: same append as above.

### I.7 — Why this collapse is the right call

1. **5-year-maintenance test.** A future maintainer reading PR 5's commit history would see clean #276 closure with no architectural debt. A re-litigation of PR 3c.B Commit 6 in the same PR would obscure both intentions.
2. **PM directive: avoid agents not walking adjacent code paths.** The implementer's stop is the PM cycle working — surface load-bearing surprises rather than fabricate. Recompose the plan.
3. **#276 is the stated user-facing bug.** It's been carried as a constitutional concern since PR 4b-A; closing it cleanly is the value PR 5 delivers.
4. **#261 is a real bug but post-PR-3c.B-architectural-decision**, not post-rule-coverage-gap. Different surface, different PR.

## End (operative)

PR 5 lands as **4 atomic, GPG-signed, scheme-internal commits** closing
#276 (E068 + E069 banner-rollup) with no engine-crate touch. T059/T060
are complete by retirement; T064 reframed; T059a/T059b added; T061 +
#261 deferred to a follow-up PR. Rule count: 38 → 40.
