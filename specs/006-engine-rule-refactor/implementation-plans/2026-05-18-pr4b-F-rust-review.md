<!-- SPDX-FileCopyrightText: 2026 Knitli Inc. -->
<!-- SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0 -->

# PR 4b-F Rust Review

**Date:** 2026-05-18  
**Reviewer role:** Independent Rust code reviewer (not the implementation agent)  
**Branch:** `refactor-006-pr-4b-f-residue-cleanup`  
**Base:** `staging` head  
**Review commands run:**
- `cargo check --workspace` — exit 0
- `cargo +stable clippy --workspace --all-targets -- -D warnings` — exit 0
- `cargo fmt --check` — exit 0
- `cargo test --workspace` — all 0 failures
- `cargo test --test lattice_vs_scheme_parity -p marque-capco` — 74 passed, 0 failed

---

## §1 Verdict

**APPROVED-WITH-CONCERNS**

No CRITICAL findings. One MEDIUM finding (Constitution VIII citation
accuracy drift in `render_declassify.rs`). All other findings are LOW
or NOTE. The structural work — signature simplification, function
deletion, test green — is correct and clean.

---

## §2 Hard-Check Results

| # | Check | Status | Evidence |
|---|-------|--------|---------|
| 1 | `#[allow(clippy::too_many_lines)]` survival on `join_via_lattice_body` | **PASS** | Attribute at `marking.rs:219-223`, directly above `fn join_via_lattice_body` at line 224. Both `reason` field and the attribute survived the signature edit. |
| 2 | `PageContext` shim (`crates/ism/src/page_context.rs`) untouched | **PASS** | `git diff staging..HEAD -- crates/ism/src/page_context.rs` returns 0 bytes. Byte-identical to staging. |
| 3 | `assert_impl_all!(PageContext: Send, Sync)` at `crates/ism/tests/send_sync.rs` untouched | **PASS** | `git diff staging..HEAD -- crates/ism/tests/send_sync.rs` returns 0 bytes. |
| 4 | Parity gate `crates/capco/tests/lattice_vs_scheme_parity.rs` zero modifications | **PASS** | `git diff` returns 0 bytes. `cargo test --test lattice_vs_scheme_parity -p marque-capco` exits 0: 74 passed. |
| 5 | G13 panic discipline — no `{:?}` of attrs/portions/spans in new panic messages | **PASS** | Zero new `panic!` lines in the diff (`git diff staging..HEAD -- '*.rs' | grep '^+' | grep 'panic'` returns empty). The surviving sentinel at `project_attrs_pipeline:728-736` is unchanged and counts-only (`raw_snapshot.len()` / `raw.len()`). |
| 6 | Constitution VIII citation discipline — `§X.Y pNN` form, no bare `§NN`, no `line NNNN` | **PARTIAL** | All citations in the diff use page-number form. Zero `line NNNN` anchors introduced. However one citation is semantically misplaced: `render_declassify.rs` now writes `Per CAPCO-2016 §E.1, the banner line is CLASSIFICATION//SCI//...` — §E.1 is the OCA authority-block authority, not banner-line format authority. See §3 item f and Finding F-1. |
| 7 | Constitution VII §IV — zero `marque-engine` / `marque-ism` / `marque-scheme` / `marque-rules` / `marque-core` source edits | **PASS** | `git diff staging..HEAD --name-only` shows only `crates/capco/`, `crates/wasm/`, `specs/`, `docs/`, `.github/workflows/` files. No engine-crate source touched. |
| 8 | Workspace grep cleanliness | **PASS** | `_tmp_ctx` / `_page_context` / `_page_ctx` in `crates/*/src/`: 1 match — `marking.rs:181` in a doc-comment historical note. `page_context.rs:\d+` refs: 0 matches. `expected_dissem_us` / `expected_aea_markings` / `expected_classification` / `render_expected_banner` in `crates/*/src/`: 3 matches, all in `crates/ism/src/page_context.rs:18-19` and `crates/ism/src/projected.rs:14` — deletion-record doc-comments in the out-of-scope shim, as the implementation report claims. |
| 9 | `cargo +stable clippy --workspace --all-targets -- -D warnings` clean | **PASS** | Exit 0. |
| 10 | `cargo test --workspace` clean | **PASS** | Exit 0. Zero failures across all suites. |

---

## §3 Rust-Specific Findings

### a. Signature ergonomics — `project_from_page_context`

`marking_scheme_impl.rs:673-677`:

```rust
pub fn project_from_page_context(
    &self,
    page_context: &marque_ism::PageContext,
) -> CanonicalAttrs {
    self.project_attrs_pipeline(page_context.portions())
}
```

Exactly one-line forward as the architect plan specified. No incidental
allocation, no unnecessary clones. The only work is the `portions()`
borrow, which returns `&[CanonicalAttrs]` with no allocation.
**PASS.**

### b. `pub(crate)` discipline — `project_from_attrs_slice` deletion

`project_from_attrs_slice` is completely absent from
`crates/capco/src/scheme/marking_scheme_impl.rs`. The grep
`grep -n "project_from_attrs_slice" crates/capco/src/scheme/marking_scheme_impl.rs`
returns empty. Not `#[allow(dead_code)]`-marked — actually deleted.
No new `pub(crate)` items were added in the diff
(`git diff staging..HEAD -- '*.rs' | grep '^+.*pub(crate)'` returns empty).
**PASS.**

### c. Lifetimes — no new lifetime parameters introduced

The simplified signatures (`fn join_via_lattice_body(portions:
&[CanonicalAttrs])`, `fn project_attrs_pipeline(&self, raw:
&[CanonicalAttrs])`, `fn project_from_page_context(&self,
page_context: &marque_ism::PageContext)`) all use elided lifetimes
correctly. No `'_` or explicit lifetime parameters were introduced.
The refactor moved in the right direction: removed a parameter (the
`&PageContext`) that forced a named pair of lifetimes in earlier
versions. **PASS.**

### d. `#[cfg(debug_assertions)]` survival on the closure sentinel

`marking_scheme_impl.rs:720-738`:

```rust
#[cfg(debug_assertions)]
let raw_snapshot: Vec<CanonicalAttrs> = raw.to_vec();

let joined = CapcoMarking::new(CapcoMarking::join_via_lattice(raw));
let mut out = self.closure(joined);

#[cfg(debug_assertions)]
{
    if raw != raw_snapshot.as_slice() {
        panic!(
            "closure() mutated the per-portion CanonicalAttrs slice \
             ({} portion(s) before vs {} after) ...",
            raw_snapshot.len(),
            raw.len(),
        );
    }
}
```

Both `#[cfg(debug_assertions)]` gates survived intact. Release builds
do not allocate `raw_snapshot`. The panic message is counts-only
(G13 compliant). **PASS.**

### e. Doc-comment line-number drift

The two stale anchors flagged by the preflight were retired:

- `marking.rs:273-274` (`"lines 284-706 in the current revision"`) —
  replaced with `"~420 LOC"` in the new `join_via_lattice_body` doc at
  `marking.rs:186-187`. No line-number anchor survives.
- `marking.rs:243-244` (`"engine.rs:4540-4574"`) — the entire
  `join_via_lattice_with_context` function was deleted in Commit 4,
  taking the stale anchor with it. Zero `engine.rs:\d+` matches in
  `crates/capco/src/`.

`git diff staging..HEAD -- '*.rs' | grep '^+' | grep '\.rs:[0-9]'`
returns empty — no new file:line anchors introduced. **PASS.**

### f. Stale `_tmp_ctx` rationale passages — 5 locations per preflight Risk 3

Preflight identified 5 doc-comment locations describing the `_tmp_ctx`
rationale. Verified retirement:

1. `marking.rs:60-62` (module doc) — now reads "PR 4b-F retired the last
   `&PageContext` parameter from the lattice fold body; the pipeline
   now consumes `&[CanonicalAttrs]` end-to-end." **Retired.**
2. `marking.rs:188-197` (`join_via_lattice` body comment about
   tmp_ctx build for residue-axis accessor calls) — the entire
   tmp_ctx build block is deleted; the body is now a single-line
   forward: `Self::join_via_lattice_body(portions)`. **Retired.**
3. `marking.rs:266-268` (`join_via_lattice_body` doc — "as the per-axis
   input and `tmp_ctx` for the residue-axis accessor surface") —
   the parameter sentence is gone; the function body doc is rewritten
   around the cleaner signature. **Retired.**
4. `marking.rs:317-323` (parameter comment block "PR 4b-E: `_tmp_ctx`
   retained at the boundary") — the entire block retired with the
   parameter. **Retired.**
5. `marking.rs:454-459` ("PR 4b-D.2 Commit 7+: tmp_ctx is now received
   by reference from the caller") — replaced with current pipeline
   shape description. **Retired.**

All 5 locations atomically updated. **PASS.**

### g. Adjacent callsite walk — `noforn_clears.rs`, `pattern_c.rs`, `wasm/src/lib.rs`

All three are pure doc-comment edits with no functional changes:

- `noforn_clears.rs:150-172`: comment block updated from
  "`PageContext-direct path (expected_dissem_us Step 6) handles this`"
  to `"The per-axis lattice path enforces this via
  DissemSet::with_noforn_injected"`. No logic change.
- `pattern_c.rs:82-91`: comment updated from referencing
  `"PageContext::expected_aea_markings"` to naming the regression test
  function `pattern_c_dod_ucni_classified_strip_promotes_noforn`.
  Healthy — gives a future reader a test to run.
- `wasm/src/lib.rs:1153-1162`: doc-comment de-historicized from
  "PR 4b-E: migrated from the retired `PageContext::render_expected_banner`"
  to present-tense description.

**PASS.** These are exactly the right adjacent-callsite sweeps the
architect plan mandated.

### h. `#[allow]` annotations — no new ones

`git diff staging..HEAD -- '*.rs' | grep '^+.*#\[allow'` returns empty.
No new `#[allow]` annotations introduced. The surviving
`#[allow(clippy::too_many_lines, reason = "...")]` at `marking.rs:219`
is the pre-existing permanent exemption documented in the preflight.
**PASS.**

---

## §4 Findings Table

| ID | Severity | Location | Description | Suggested fix |
|----|----------|----------|-------------|---------------|
| F-1 | MEDIUM | `crates/capco/src/render/render_declassify.rs:14-17` | Constitution VIII citation accuracy drift. The old text used `§E.1` as a parenthetical justification for the CAB-lives-separately claim: `"on its own block elsewhere on the page (typically the bottom of the cover page, per §E.1)"`. The new text restructured the sentence so `"Per CAPCO-2016 §E.1"` now introduces the claim about the banner line format: `"Per CAPCO-2016 §E.1, the banner line is CLASSIFICATION//SCI//SAR//..."`. §E.1 is the OCA Classification Authority Block authority (original classification authority block requirements per EO 13526 / ISOO) — it does not describe the banner line format. Banner line format authority is §A.6 (p15-17) and §D.1 (p27). The syntax introduced misleads a reader into thinking §E.1 governs banner line structure. | Restore the `§E.1` citation as a parenthetical for the CAB-placement claim: `"The CAB (...) lives on its own block elsewhere on the page (typically the bottom of the cover page, per §E.1)."` The banner line structure claim does not need a new citation (it is structural context for the renderer's behavior, not a rule being cited). |
| F-2 | LOW | `crates/capco/src/scheme/marking_scheme_impl.rs:241-243` | The comment `"// Commit 7 perf:"` still refers to a specific commit number rather than a behavioral attribution. Per the project's `feedback_avoid_line_number_anchoring` convention, symbolic references outlast commit numbers when history is rebased. Not blocking — this was not introduced by this PR (it is a surviving passage from a prior PR) — but the cleanup opportunity was adjacent. | Consider replacing `"Commit 7 perf:"` with `"PR 4b-D.2 perf:"` or dropping the commit attribution in favor of the functional explanation alone. |
| F-3 | LOW | `crates/capco/src/scheme/marking.rs:80-88` | The `CapcoMarking` struct doc-comment warning block (`# ⚠️ Phase A scaffolding — do not use in production`) is inherited from an early phase of the PR 4b series. This is not introduced by PR 4b-F, but the "Lattice contract" wording and the "Phase B replaces the impl" future-work language are now stale — Phase B has shipped. Not blocking; no new text was added by this PR to this section. | Stale scaffolding warning. Future maintainability: a follow-up PR should retire the `⚠️ Phase A scaffolding` language now that Phase B has shipped. Not in PR 4b-F's scope. |
| F-4 | NOTE | `crates/capco/src/lattice.rs` (DissemSet doc) | The rewritten DissemSet overlay-set doc now reads cleanly. However, the `to_vec` method doc at line 2265 was updated from `"for compatibility with existing PageContext::expected_dissem_us-shaped APIs"` to `"for callers that need the post-overlay set in Vec-shaped form (parity-gate fixtures and similar inspection sites; into_boxed_slice is the production renderer-facing API)"`. The new wording is clear and accurate. No concern. |

---

## §5 Citation Spot-Check Log

Five §-citations verified against `crates/capco/docs/CAPCO-2016.md`
using `crates/capco/docs/CAPCO-2016_citation_index.yml` as the finder.

| Citation | Location in diff | Citation index range | Manual content check | Verdict |
|----------|-----------------|---------------------|---------------------|---------|
| `§H.8 p134` (FOUO classification-gate eviction) | `lattice.rs` DissemSet doc | §H.8 FOUO: start_page 134, end_page 135 | p134 is the start of the FOUO section and covers the classification-gate eviction clause | CORRECT |
| `§H.6 p116 DOD UCNI` | `lattice.rs` DissemSet doc | `H.6: **DOD** UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION`: start_page 116, end_page 117 | p116 is the start of the DOD UCNI section | CORRECT |
| `§H.6 p118 DOE UCNI` | `lattice.rs` DissemSet doc | `H.6: **DOE** UNCLASSIFIED CONTROLLED NUCLEAR INFORMATION`: start_page 118, end_page 119 | p118 is the start of the DOE UCNI section | CORRECT |
| `§E.1` in `render_declassify.rs` (syntactically applied to the banner line) | `render_declassify.rs:14` | §E.1 `Original Classification Authority`: start_page 31, end_page 31 | §E.1 covers OCA classification authority block requirements (EO 13526 / ISOO §2001.21 / §2001.26). It establishes the CAB as a face-of-document block. It does NOT define the banner line format — that is §A.6 (start 15) and §D.1 (start 27). | MISPLACED — §E.1 is valid authority for "CAB lives separately from the banner" but not for "the banner line is CLASSIFICATION//SCI//..." as the new sentence structure implies. See Finding F-1. |
| `§H.9 p178 SBU-NF` / `§H.9 p185 LES-NF` | `lattice.rs` DissemSet doc | §H.9 SBU NOFORN: start_page 178, end_page 180; §H.9 LES NOFORN: start_page 185, end_page 188 | p178 is the start of the SBU NOFORN section; p185 is the start of the LES NOFORN section | CORRECT |

---

**End of review.**

