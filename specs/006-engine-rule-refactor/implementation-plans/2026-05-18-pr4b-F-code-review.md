<!-- SPDX-FileCopyrightText: 2026 Knitli Inc. -->
<!-- SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0 -->

# PR 4b-F Overall Code Review

**Date:** 2026-05-18
**Reviewer role:** Overall code reviewer (not the implementation agent; parallel to rust-reviewer and lattice-consultant)
**Branch:** `refactor-006-pr-4b-f-residue-cleanup`
**Base:** `staging` head `c3f544d6`
**Plan of record:** `docs/plans/2026-05-18-pr4b-F-residue-cleanup-plan.md`

---

## §1 Verdict

**APPROVED-WITH-CONCERNS**

The core signature-retirement work is correct, complete, and well-executed. The lattice fold body simplification, pipeline renaming, and `_with_context` collapse all land cleanly with no functional hot-path changes. Two issues require attention before or immediately after merge:

1. **MEDIUM** — `.github/workflows` action SHA pins are stale in the branch HEAD, causing a version downgrade on merge. Rebase on current staging or cherry-pick the `chore(deps): update claude and gemini action versions` commit (`62c88d14`) before opening the PR.

2. **MEDIUM** — One citation in `render_declassify.rs` was degraded from its pre-PR form: `§E.1` is now presented as the authority for the banner line FORMAT, but §E.1 is "Original Classification Authority" (p31), not a banner format definition. The format attribution should be §A.6 p15-17.

No CRITICAL findings. The seven items in the findings table are three LOW (pre-existing line-number anchors in unchanged files), two LOW (minor doc phrasing), one MEDIUM (citation), and one MEDIUM (CI action pins). The bench measurement discrepancy is noted in §9 but does not constitute a blocking issue.

---

## §2 Plan-vs-Implementation Fidelity

### Commit structure

The architect plan prescribed six commits (Commits 1-5 functional + Commit 6 bookkeeping). The implementation delivered five functional commits + one bookkeeping commit. **Commit 5 was folded into Commit 6**, which the implementation report justifies in §2 deviation #2: the workspace audit found no third `_tmp_ctx` / `_page_context` site, and OQ-1 Option A means no engine call-site edit was needed. With no functional content left for Commit 5, folding the audit attestation into Commit 6's message body is the correct disposition. This is not a corner cut — it follows the plan's OQ-2 resolution rule ("if no third site, fold attestation into bookkeeping commit").

### Content fidelity

All four functional outcomes from the plan are present and verifiable in the diff:

| Plan requirement | Verified? |
|---|---|
| `join_via_lattice_body` takes `&[CanonicalAttrs]` only | YES — `fn join_via_lattice_body(portions: &[CanonicalAttrs])` at marking.rs:224 |
| `join_via_lattice_with_context` deleted | YES — not present in HEAD; confirmed 0 matches in grep |
| `project_attrs_pipeline_with_context` renamed to `project_attrs_pipeline`, `page_ctx` parameter dropped | YES — `fn project_attrs_pipeline(&self, raw: &[CanonicalAttrs])` in marking_scheme_impl.rs |
| `project_from_attrs_slice` inlined and deleted | YES — not present in HEAD; compiler-verified via `cargo check` passing |
| `project_from_page_context` public API preserved, one-line forward | YES — `pub fn project_from_page_context` at marking_scheme_impl.rs:673, body calls `self.project_attrs_pipeline(page_context.portions())` |
| Engine crate untouched | YES — confirmed via diff |
| `marque-ism` source untouched | YES — confirmed via diff |
| T111-T115 ticked in tasks.md | YES — all five tasks now carry `[x]` with resolution notes |

One minor deviation from the plan's prescribed doc-comment for the module-level text at `marking.rs:44-58`: the plan said to drop "retained at the function boundary for signature stability" and replace with a specific sentence. The implementation substituted equivalent prose rather than verbatim replacement. This is acceptable — the quality bar ("5-year maintainer does not see an underscore-prefixed parameter") is met.

---

## §3 Adjacent-Callsite Walk Findings

### The three declared incidental expansions

**`crates/capco/src/scheme/rewrites/noforn_clears.rs`** — doc-comment only. The change removes a reference to `PageContext-direct path` and `page_context_to_attrs` (retired surfaces). Replacement text correctly names `DissemSet::with_noforn_injected` as the lattice-path location and retains `scheme.project(Scope::Page, ...)` as the PageRewrite path. Pure doc-comment; no functional change. Walk discipline correct.

**`crates/capco/src/scheme/rewrites/pattern_c.rs`** — doc-comment only. Replaces a reference to the "pre-PR-4b-C UCNI classified-strip" as a `PageContext::expected_aea_markings` bug, now naming the regression test by function name `pattern_c_dod_ucni_classified_strip_promotes_noforn`. Converts a stale page-context reference to a symbolic test reference. Walk discipline correct.

**`crates/wasm/src/lib.rs`** — doc-comment only. Removes a migration note "PR 4b-E: migrated from the retired `PageContext::render_expected_banner` to the scheme's `render_canonical(Scope::Page, ...)`" and replaces with a forward-looking description of the current behavior. No functional change. Walk discipline correct. There is a residual reference in the new text to "`crates/scheme/src/scheme.rs` `render_canonical` doc" — this is a crate-path reference, not a file:line anchor, and is acceptable.

### Missed sweeps assessment

**`crates/capco/src/scheme/actions/intent.rs`** — Contains several `file:line` anchors (`scheme.rs:185-194`, `scheme.rs:624-639`, `scheme.rs:454-458`, `rules.rs:559`). These are pre-existing in unchanged code and are therefore out-of-PR-4b-F scope per the plan's hard constraint that the sweep targets only the declared sites. The files containing these are not in the PR diff. Not a 4b-F finding.

**Grep sweep for `expected_dissem_us` / `expected_aea_markings` / `render_expected_banner` in `crates/capco/src/`** — Multiple doc-comment hits in `lattice.rs`, `rules.rs`, `marking.rs`, `scheme/constraints/categories.rs`. Inspection confirms:
- `lattice.rs:14`, `lattice.rs:69`: historical-migration notes in module-level docs — acceptable (document the retired path for future maintainers).
- `marking.rs:50-51`: module-level doc lists the five retired accessor names as migration context. These names are used as symbolic references to deleted functions, not as live API calls. Acceptable.
- `rules.rs:2454`, `2475`, `2694`, `4002`, `4113`, `4225`, `4385`, `4514`: all in unchanged rules code, referencing the retired accessors in comments explaining legacy behavior. Out of 4b-F scope.
- All `crates/ism/src/page_context.rs` and `crates/ism/src/projected.rs` matches: correctly identified as the deletion-record doc-comments in the PageContext shim; cannot be touched per Constitution VII §IV.

**Surviving `file:line` anchor in `crates/capco/src/scheme/actions/intent.rs`** — Pre-existing; not introduced by 4b-F. The plan's sweep only covered the six declared sites; `intent.rs` was not a declared target. Not a 4b-F finding, but noted as cleanup debt for a future sweep PR.

---

## §4 tasks.md Bookkeeping Accuracy

### T111 citation correction

The implementation report claims T111 was closed in PR 4a / #422, not Phase 5 PR-2 / #146 as the architect plan's table stated. **Verified correct.** `git log staging --grep="is_fdr_dissem"` returns exactly one commit:

```
fc91852e PR 4a: Vocabulary<S>::is_fdr_dissem trait method (006 T111) (#422)
Date: Fri May 15 13:07:35 2026 -0400
```

The correction is accurate. The tasks.md entry reads: "closed in PR 4a / #422. `Vocabulary::is_fdr_dissem` and `is_fdr_dominator` live on `CapcoScheme`..." This matches the commit and the memory entry `project_is_fdr_dissem_vs_is_fdr_dominator`. Constitution VIII compliance: the resolved citation is now verifiable.

### T112 resolution note completeness

The T112 note cites "PR 4b-A / #426 (AeaSet), PR 4b-B / #437 (...), and PR 4b-E / #539 (...)". This is verified against the CLAUDE.md Recent Changes section, which lists PR 4b-A, PR 4b-B, PR #456, and PR 4b-E as the lattice implementation PRs. The GitHub PR numbers could not be independently confirmed via `gh pr view` from this context, but the marque-internal series mapping is internally consistent.

### T115 attribution

The T115 note correctly attributes two contributions: "PR 4b-D.2 / #527 (Copilot R1 D24 dropped `impl JoinSemilattice for CapcoMarking`) and PR 4b-F (this PR; retired the last `&PageContext` parameter from the lattice-fold body chain)."

Verification:
- `impl JoinSemilattice for CapcoMarking` is confirmed GONE from the current `marking.rs`. Lines 665-666 contain only a historical comment block: "The `impl JoinSemilattice for CapcoMarking` and `impl MeetSemilattice for CapcoMarking` blocks were dropped in PR 4b-D.2 Commit 11."
- `join_via_lattice_with_context` is confirmed deleted.
- `project_attrs_pipeline_with_context` is confirmed renamed to `project_attrs_pipeline`.
- `project_from_attrs_slice` is confirmed deleted.

The T115 attribution is accurate. The two-phase attribution (4b-D.2 for the trait-impl drop + 4b-F for the body-signature cleanup) is the correct reading of the history.

### T069 deferral note

The T069 note correctly records "PR 4b-F architect investigated; deferred per OQ-3 (full PageContext retirement requires marque-rules + marque-engine ripple; properly belongs to PR 6c)." The checkbox stays unchecked. Correct disposition.

---

## §5 Doc-Comment Quality

### `crates/capco/src/scheme/marking.rs`

**Module-level doc (lines 44-62):** Clean. The "PR 4b-F retired the last `&PageContext` parameter from the lattice fold body; the pipeline now consumes `&[CanonicalAttrs]` end-to-end" sentence accurately describes the post-4b-F state. The historical list of the five retired accessor names (lines 50-56) and their replacement functions is a useful migration record.

**`join_via_lattice` doc (lines 139-174):** Significant improvement over the pre-PR version which was dense with parity-divergence tracking language. The new doc accurately describes the post-4b-F state: two entry surfaces (trait-path and engine fast-path) both delegate through `project_attrs_pipeline`. The "Originally introduced in PR 4b-B Commit 7... PR 4b-D.2 flipped... PR 4b-F collapsed" historical summary is useful and accurate.

**`join_via_lattice_body` doc (lines 174-222):** The "~420 LOC" prose count replaces the stale `marking.rs lines 284-706 in the current revision` anchor per the plan requirement. The `#[allow(clippy::too_many_lines)]` attribute is preserved at lines 219-222. The size guideline justification is concise and reads cleanly.

**One stylistic note (not a blocker):** Line 181 reads `/// `crates/capco/src/lattice.rs`) and PR 4b-F...` — this is a crate-path reference (not a `file:NNN` line anchor), so it passes the policy. Acceptable.

### `crates/capco/src/scheme/marking_scheme_impl.rs`

**`project_from_page_context` doc (lines 641-700):** The new "Same-slice property" section is well-written and implements the plan's requirement: "Future maintenance that reintroduces a parallel derivation path MUST re-add the contract at the new fork — the invariant lives in this doc-comment, not in a runtime check." This is the correct structural substitute for the retired debug-assert.

**`project_attrs_pipeline` doc (lines 702-716):** Clean. Notes that the `page_ctx` parameter is retired and explains why the same-slice contract became vacuous. References `project_from_page_context` symbolically. No file:line anchors.

### `crates/capco/src/lattice.rs`

**`DissemSet` doc (lines 2039-2098):** Substantial improvement. The old "PARTIAL parity with PageContext::expected_dissem_us" framing with three "PageContext-ONLY" bullets is replaced with a clear statement of where each overlay lives in the post-4b-E world. The four overlays are now attributed to their actual locations (lattice path vs scheme PageRewrite catalog). All §-citations are preserved. **One concern** in the `with_noforn_injected` sub-doc at line 2278: the new text references "the NOFORN rendezvous in the `join_via_lattice` body" — this is accurate and uses the symbolic function name, not a line number. Acceptable.

**`DissemSet::to_vec` doc (line 2262):** Minor improvement — removes the stale "PageContext::expected_dissem_us-shaped APIs" reference. New text ("parity-gate fixtures and similar inspection sites") is accurate.

### `crates/capco/src/scheme/actions/fgi.rs`

**`extract_foreign_sources` doc (lines 16-63):** Good. The old `page_context.rs lines 894-921` line anchors are replaced with symbolic references: "the country-extraction step of the retired `PageContext::expected_fgi_marker` accessor (deleted with the `PageContext::expected_*` surface in PR 4b-E)" and "`FgiSet::from_attrs_iter` (see `crates/capco/src/lattice.rs`)". Two §-citations added: `§H.7 p123 + p128` and `§H.3 p56`. See §7 for citation verification.

**`merge_fgi_markers` doc (lines 108-151):** The `expected_fgi_marker` reference at line 113 is used as a historical identifier for a retired function — acceptable context for a migration note. The existing `§H.7 pp123-124` citation at line 126-136 with its "Verified 2026-05-16" attestation is unchanged and sound.

### `crates/capco/src/render/render_declassify.rs`

**Citation accuracy concern (HIGH for this specific change):** The new version at line 24 reads:

```
Per CAPCO-2016 §E.1, the banner line is
`CLASSIFICATION//SCI//SAR//AEA//FGI//DISSEM//NON-IC` — the CAB
("Classified By", "Derived From", "Declassify On") lives on its own block
```

**§E.1 is "Original Classification Authority" (pp31-31 per the citation index).** It covers what elements the OCA must include in the Classification Authority Block (Classified By, Classification Reason, Declassify On). It does not define the banner line format. The banner format (`CLASSIFICATION//SCI//...`) is defined in §A.6 p15-17 (Figure 2). The pre-PR version had `(per §E.1)` trailing after "bottom of the cover page" — that placement was also technically inaccurate (§E.1 is about the CAB's required elements, not its page location), but it was a trailing qualifier that did not assert §E.1 as the authority for the banner format.

The new version promotes `§E.1` to lead the sentence "Per CAPCO-2016 §E.1, the banner line is..." — this reads as if §E.1 defines the banner format, which it does not. This is a citation accuracy degradation under Constitution VIII. The fix is to change "Per CAPCO-2016 §E.1" to "Per CAPCO-2016 §A.6 p15-17" for the banner format claim (or restructure to make clear §E.1 is cited for the CAB element, not the banner format).

Note: The §E.1 citation in the `# Authority` block header at line 9 ("CAPCO-2016 §E.1 (Original Classification Authority) — the Classification Authority Block (CAB) line `Declassify On`") remains accurate and should be preserved.

---

## §6 Commit-Message Audit

### Commit 1 (`9af3b925`)

Subject: `refactor(capco): PR 4b-F Commit 1 — retire _tmp_ctx from join_via_lattice_body`

Body references functions by name (`join_via_lattice_body`, `join_via_lattice_with_context`, `join_via_lattice`). No `file:line` anchors. Reviewer attestation block (`Constitution VII §IV / Constitution V G13 / Constitution VIII`) is present. **PASS.**

### Commit 2 (`8358e4d7`)

Subject: `refactor(capco): PR 4b-F Commit 2 — retire page_ctx from project_attrs_pipeline`

Body uses symbolic references throughout. Notes "One stale cross-file reference fixed opportunistically: `crates/engine/src/engine.rs:4540-4574` → `crates/engine/src/engine.rs`" — this is documenting the REMOVAL of a line-number anchor, which is correct. The commit body itself contains the old anchor in context ("engine.rs:4540-4574 → engine.rs"), but only as the source of the retired reference, which is acceptable. Reviewer attestation block present. **PASS.**

### Commit 3 (`c8b0c9ed`)

Subject: `refactor(capco): PR 4b-F Commit 3 — inline project_from_attrs_slice into trait body`

Body uses symbolic function names (`project_from_attrs_slice`, `MarkingScheme::project`, `project_attrs_pipeline`, `project_from_page_context`). No line anchors. Reviewer attestation block present. **PASS.**

### Commit 4 (`28fceac7`)

Subject: `refactor(capco): PR 4b-F Commit 4 — collapse join_via_lattice_with_context`

Body uses symbolic function names throughout. The G13 provenance paragraph correctly names "the closure-mutates-input check in `project_attrs_pipeline`" as the surviving sentinel. Reviewer attestation block present. **PASS.**

### Commit 6 (`5ad6e886`)

Subject: `refactor(capco): PR 4b-F Commit 6 — tasks.md bookkeeping + doc-comment sweep`

Body is comprehensive (the longest commit message). Documents the T111 correction with the specific `git log --grep` evidence. Lists all 14 files changed. Documents the `expected_dissem_us` grep residuals and their out-of-scope justification. **One item to note**: the commit message body contains plan-document line numbers ("marking.rs:181", "engine.rs:4504-4525") in the `docs/plans/2026-05-18-pr4b-F-residue-cleanup-plan.md` verbatim sections — these are in a plan document, not in source code, and are acceptable as historical plan artifact references. **PASS with note.**

---

## §7 §-Citation Spot-Check Log

All citations spot-checked against `crates/capco/docs/CAPCO-2016.md` using `crates/capco/docs/CAPCO-2016_citation_index.yml` as the section→page-range finder.

| Citation | Location in PR diff | Citation index says | Verdict |
|---|---|---|---|
| §H.7 p123 | `fgi.rs:28` ("NATO ownership reciprocity") | §H.7 FOREIGN GOVERNMENT INFORMATION (FGI) starts p123 | VALID — p123 is within §H.7 FGI section |
| §H.7 p128 | `fgi.rs:36` (source-concealed FGI supersedes acknowledged) | §H.7 FGI subsection spans pp123-128 | VALID — p128 is within §H.7 FGI |
| §H.3 p56 | `fgi.rs:44` ("USA is implicit on the JOINT axis") | §H.3:JOINT starts p56 | VALID — p56 is the JOINT section start |
| §H.8 p134 | `lattice.rs DissemSet` ("FOUO classification-gate eviction") | §H.8:FOR OFFICIAL USE ONLY starts p134 | VALID — p134 is the FOUO section start |
| §H.6 p116 + §H.6 p118 | `lattice.rs DissemSet` ("DOD UCNI" / "DOE UCNI") | §H.6:**DOD** UCNI starts p116; §H.6:**DOE** UCNI starts p118 | VALID — exact section starts |
| §H.9 p178 | `lattice.rs DissemSet` ("SBU-NF") | §H.9:SENSITIVE BUT UNCLASSIFIED NOFORN starts p178 | VALID — exact section start |
| §H.9 p185 | `lattice.rs DissemSet` ("LES-NF") | §H.9:LAW ENFORCEMENT SENSITIVE NOFORN starts p185 | VALID — exact section start |
| §H.8 p145 | `lattice.rs DissemSet` ("NOFORN dominates") | §H.8:NOT RELEASABLE TO FOREIGN NATIONALS starts p145 | VALID — exact section start |
| §H.8 p136 + §H.8 p140 | `lattice.rs DissemSet` ("OC-USGOV supersession") | §H.8:DISSEMINATION AND EXTRACTION...ORIGINATOR starts p136; p140 is within this section | VALID — p140 falls within H.8:ORCON section (pp136-141) |
| §H.6 p104 | `render_aea.rs` ("RD > FRD > TFNI precedence") | §H.6:RESTRICTED DATA starts p104 | VALID — p104 is the RD section start; CAPCO manual line 2529 confirms "RD takes precedence" over FRD/TFNI |
| §E.1 | `render_declassify.rs:24` ("Per CAPCO-2016 §E.1, the banner line is...") | §E.1 = Original Classification Authority at p31 | INVALID ATTRIBUTION — §E.1 defines the CAB's required elements (Classified By, Classification Reason, Declassify On), NOT the banner line format. Banner format is §A.6 p15-17. This is a Constitution VIII violation. |
| §H.8 p150-151 | `render_dissem.rs:33` ("REL TO template") | §H.8:AUTHORIZED FOR RELEASE TO starts p150 | VALID — p150 is the REL TO section start |

**Summary:** 11 of 12 citations verified as accurate. One citation (`render_declassify.rs:24` §E.1 as banner format authority) is a misattribution. This is a Constitution VIII violation per Principle VIII's "accurately reflect what that passage says" requirement.

---

## §8 Findings Table

| Severity | File | Location | Description | Suggested Fix |
|---|---|---|---|---|
| MEDIUM | `crates/capco/src/render/render_declassify.rs` | Line 24 | **Citation accuracy degradation**: "Per CAPCO-2016 §E.1, the banner line is `CLASSIFICATION//SCI//...`" attributes the banner format to §E.1 (Original Classification Authority, p31), which covers CAB elements, not banner format. Banner format is §A.6 p15-17. The pre-PR trailing `(per §E.1)` after "bottom of the cover page" was also inaccurate but less misleading. Constitution VIII violation. | Change "Per CAPCO-2016 §E.1, the banner line is" to "Per CAPCO-2016 §A.6 p15-17 (Figure 2), the banner line is". The §E.1 citation in the `# Authority` header (line 9, citing the CAB's Declassify On element) is accurate and should be preserved. |
| MEDIUM | `.github/workflows/claude.yml`, `.github/workflows/gemini-dispatch.yml` | Branch HEAD vs staging | **Stale action SHA pins**: The branch was created from merge-base `c3f544d6`, before staging commit `62c88d14` ("chore(deps): update claude and gemini action versions") updated the `marquetools/.github` SHA pins from `36624ef13b4142...` to `cf142411134fd35...`. Merging this PR to staging would downgrade the pins by one version bump. This is a rebase/merge artifact, not an intentional change by the PR author. | Rebase on or merge current staging before opening the PR, to pick up commit `62c88d14`. The PR's functional diffs are entirely in `marque-capco` source and `specs/`; the rebase is mechanical. |
| LOW | `crates/capco/src/scheme/actions/intent.rs` | Lines 119, 166, 194, 244, 265, 527, 596 | **Pre-existing file:line anchors** (`scheme.rs:185-194`, `scheme.rs:624-639`, etc.) in unchanged code. Outside PR 4b-F scope per the plan's declared sweep targets. Documented here for future sweep PR. | File in a follow-up sweep PR, not required for 4b-F merge. |
| LOW | `crates/capco/src/scheme/rewrites/pattern_b.rs` | Line 86 | **Pre-existing file:line anchor** (`scheme.rs:5018-5039`) in unchanged code. Same disposition as intent.rs. | File in a follow-up sweep PR. |
| LOW | `crates/capco/src/scheme/marking.rs` | Lines 247, (inline body) | **Pre-existing file:line anchor** at `crates/ism/src/attrs.rs:521` in a non-PR-touched doc-comment line. Outside 4b-F scope. | Follow-up sweep. |
| NOTE | `docs/plans/2026-05-18-pr4b-F-implementation-report.md` | §6 bench delta | **Imprecise bench characterization**: "973µs within the 880–930µs noise band" is factually incorrect — 973µs is above the 930µs upper bound of the noise band reported in memory. The correct characterization is that 973µs is outside the noise band, consistent with the pre-existing staleness trend noted in `project_bench_baseline_staleness`, but distinct from being "within" the band. The bench drift is attributable to pre-existing regression, not to 4b-F changes (confirmed by hot-path body inspection — see §9). | The implementation report is a planning artifact, not production source. The imprecision does not affect correctness. Correct the wording to "above the noise band" in any revision of the report. |
| NOTE | `crates/capco/src/lattice.rs` | Lines 2537 | **Surviving `expected_fgi_marker` reference** in pre-existing JointSet doc-comment ("via the existing PageContext-resident `expected_fgi_marker`"). In unchanged code, out of 4b-F scope. | Follow-up sweep. |

---

## §9 Bench-Delta Verification

### Hot-path body inspection

**`join_via_lattice_body`** (`marking.rs:224`): The function body is byte-identical to the pre-PR version except for the signature (`_tmp_ctx: &marque_ism::PageContext` parameter removed). No body-arithmetic changes. The per-axis lattice composition logic is untouched.

**`project_attrs_pipeline`** (formerly `project_attrs_pipeline_with_context`, `marking_scheme_impl.rs`): The body is functionally identical — same `let joined = CapcoMarking::new(CapcoMarking::join_via_lattice(raw))` call (previously `join_via_lattice_with_context(raw, page_ctx)`), same `self.closure(joined)` call, same debug-assertion sentinel, same PageRewrite loop. The only change is the callee name (`join_via_lattice` vs `join_via_lattice_with_context`) — since `join_via_lattice` now calls `join_via_lattice_body` directly instead of building a tmp_ctx, this is actually a slight performance improvement, not a regression.

**`join_via_lattice`** itself (pre-PR): Built a one-shot tmp_ctx (n×`add_portion` clones) before delegating to `_with_context`. Post-PR: calls `join_via_lattice_body` directly. Any call through the trait path that previously paid the tmp_ctx cost now pays less. The engine fast-path was already using `join_via_lattice_with_context` which called `join_via_lattice_body`; that path now also calls `join_via_lattice_body` directly, with no intermediate tmp_ctx cost.

**Conclusion:** The 973µs measurement cannot be caused by 4b-F changes. If anything, the changes should produce a mild improvement (elimination of the tmp_ctx build cost on the trait path). The 973µs is consistent with the documented pre-existing bench regression (`project_bench_baseline_staleness`), which pre-dates this PR by several PR cycles. The implementation report's claim "PR 4b-F is signature-only: the inner pipeline body is byte-identical" is correct.

**The report's characterization "973µs is within the 880-930µs noise band" is imprecise.** 973µs is above the 930µs upper bound of the documented noise band. The correct characterization is that the measurement is outside the noise band and above the 911µs threshold gate, consistent with the drift trend that memory `project_bench_baseline_staleness` attributes to pre-existing regression from PRs 4b-A through 4b-E, and not attributable to 4b-F's changes. The constitutional ceiling (SC-001 16ms) is comfortably satisfied.

---

## Summary

| Severity | Count | Status |
|---|---|---|
| CRITICAL | 0 | pass |
| HIGH | 0 | pass |
| MEDIUM | 2 | warn |
| LOW | 3 | info |
| NOTE | 2 | note |

**Verdict: APPROVED-WITH-CONCERNS.** The functional correctness of the signature-retirement work is sound. Two MEDIUM items must be addressed before final merge: (1) rebase on current staging to pick up the action SHA pin update (`62c88d14`), and (2) fix the `render_declassify.rs:24` §E.1-as-banner-format misattribution (should be §A.6 p15-17) per Constitution VIII. Neither blocks immediate PR opening — both can be resolved as a squashed fixup commit before final merge to staging.
