# PR 4b-F Residue Cleanup — Architect Plan

**Date:** 2026-05-18
**Author:** PR 4b-F architect (per parent dispatch)
**Status:** Plan draft for PM review — no source edits performed.

---

## §1 Scope (commit-by-commit decomposition)

PR 4b-F closes the trailing boundary residue left after PR 4b-E retired the `PageContext::expected_*` accessor surface. The deletion landed cleanly, but the *signatures* on the lattice-fold and scheme-projection entry points still encode a dependency that no longer exists. PR 4b-F resolves the residue at the type-system level, then ticks the long-stale PR-4 task bookkeeping. Six commits, each independently reviewable.

The structural insight: there are three nested signature surfaces, each carrying a `&PageContext` parameter that no body actually reads after PR 4b-E:

```
Engine::project_page_marking(scheme, &PageContext)
  └─ CapcoScheme::project_from_page_context(&PageContext)
       └─ project_attrs_pipeline_with_context(&[CanonicalAttrs], &PageContext)  ← page_ctx unused
            └─ CapcoMarking::join_via_lattice_with_context(&[CanonicalAttrs], &PageContext)  ← page_ctx debug-assert only
                 └─ join_via_lattice_body(&[CanonicalAttrs], &PageContext)  ← _tmp_ctx underscore-prefixed (dead)
```

The same shape exists at `project_from_attrs_slice` → `project_attrs_pipeline_with_context`, where a one-shot `tmp_ctx` is built solely to satisfy the inner signature.

### Commit 1 — Retire `_tmp_ctx` from `join_via_lattice_body`

**File:** `crates/capco/src/scheme/marking.rs`

Drop the `_tmp_ctx: &marque_ism::PageContext` parameter from `fn join_via_lattice_body`. The body has not read it since PR 4b-E (the function-doc and parameter-name `_tmp_ctx` both attest). Update the call site in `join_via_lattice_with_context` to pass only `portions`. The debug-assert contract on `portions == page_ctx.portions()` stays in `join_via_lattice_with_context` — that's the layer that owns the same-slice contract.

Update doc comments:
- Module-level (lines 50-62) — drop the "retained at the function boundary for signature stability" hedge; the residue is gone.
- `join_via_lattice_body` doc (lines 260-309) — drop the line "and `tmp_ctx` for the residue-axis accessor surface that PageContext still bridges". Pipeline shape is now `portions → per-axis lattice composition → out`.

**Quality bar**: 5-year maintainer reading `join_via_lattice_body` should not see an underscore-prefixed parameter that says "this used to mean something". They should see a function that takes the only input it needs.

### Commit 2 — Retire `page_ctx` from `project_attrs_pipeline_with_context`

**File:** `crates/capco/src/scheme/marking_scheme_impl.rs`

Body uses `page_ctx` only to forward to `join_via_lattice_with_context` for the same-slice debug-assert. Two options analyzed:

- **Option A (collapse the pipeline body to take only `raw: &[CanonicalAttrs]`)**: rename `project_attrs_pipeline_with_context` → `project_attrs_pipeline`, drop the `page_ctx` parameter, and have it call `CapcoMarking::join_via_lattice(raw)` (the wrapper that builds a one-shot tmp_ctx internally). The wrapper exists today. **Loses the same-slice debug-assert** for the `project_from_page_context` entry — but that callsite owns the assertion itself.
- **Option B (keep `page_ctx`, retain the same-slice contract at this layer)**: signature unchanged; only Commit 1's `_tmp_ctx` retirement.

**Decision — Option A with the assert moved up.** Architecturally, `project_attrs_pipeline_with_context` is the wrong layer to own the same-slice contract once `join_via_lattice_body` no longer reads `page_ctx`. The contract exists because *callers must not mix attrs from one slice with PageContext state from another*. After Commit 1 there is no PageContext state being mixed — the pipeline is purely a `&[CanonicalAttrs] → CanonicalAttrs` function. The same-slice contract is now vacuous at this layer.

Net change:
- Rename `project_attrs_pipeline_with_context` → `project_attrs_pipeline` (signature: `(&self, raw: &[CanonicalAttrs]) -> CanonicalAttrs`).
- Body calls `CapcoMarking::join_via_lattice(raw)` directly (no `_with_context` round trip).
- The `#[cfg(debug_assertions)] let raw_snapshot: Vec<CanonicalAttrs> = raw.to_vec();` block and the post-closure check are **load-bearing G13-compliant sentinels** for the closure-mutates-input-slice invariant (D23 / §3 (e.1) read-only-attrs), independent of PageContext. **Keep them.** The G13 panic message stays counts-only.

Update `project_from_attrs_slice` (line 676) to call `self.project_attrs_pipeline(portions)` directly — no more `tmp_ctx` build, no more delegation through `_with_context`. The `pub(crate) fn project_from_attrs_slice` shrinks from 11 lines to a 2-line delegate (or inlines entirely; see Commit 3).

### Commit 3 — Collapse `project_from_attrs_slice` into the trait body

**File:** `crates/capco/src/scheme/marking_scheme_impl.rs`

`project_from_attrs_slice` is `pub(crate)` and has exactly one caller post-Commit-2: the trait `MarkingScheme::project` body at line 254. With Commit 2 it becomes a 2-line wrapper around `self.project_attrs_pipeline(portions)`. Inline it.

The trait body at line 215-256 becomes:

```rust
Scope::Page | Scope::Document | Scope::Diff => {
    let raw: Vec<CanonicalAttrs> = markings.iter().map(|m| m.0.clone()).collect();
    let out_attrs = self.project_attrs_pipeline(&raw);
    CapcoMarking::new(out_attrs)
}
```

Delete the standalone `pub(crate) fn project_from_attrs_slice`. Update doc on `project_from_page_context` (still public) to name `project_attrs_pipeline` as its sibling shared-body, not `project_from_attrs_slice`.

**Why this is right**: `project_from_attrs_slice` was an artifact of the n×clone-elimination ladder; PR 4b-D.2 added it to skip the wrap-then-unwrap round in the trait body. With the inner pipeline now consuming only `&[CanonicalAttrs]`, the trait body and the engine fast-path entry have two clean, symmetric shapes:

- Trait path: `Vec<CapcoMarking>` → `markings.iter().map(|m| &m.0).collect::<Vec<_>>()` → `project_attrs_pipeline`.
- Engine path: `&PageContext` → `page_context.portions()` → `project_attrs_pipeline`.

No third entry point survives — only the two genuinely-different shapes the codebase needs.

### Commit 4 — Retire `_with_context` suffix from `join_via_lattice`

**File:** `crates/capco/src/scheme/marking.rs`

Post-Commit-1, `join_via_lattice_body` is now a function that takes only `portions`. The two-entry-point distinction (`pub fn join_via_lattice(portions)` vs `pub(crate) fn join_via_lattice_with_context(portions, page_ctx)`) collapses to: the `_with_context` variant exists *only* for the same-slice debug-assert.

Decision tree:

- **Keep both**: preserves a debug-assert checkpoint that catches "engine called with a `PageContext` whose `portions()` disagrees with the slice it also passed." But post-Commit 3 the engine's only entry through this layer is `project_from_page_context`, which **derives** `raw = page_context.portions()` itself — the mismatch the assert catches is impossible by construction.
- **Collapse to one entry**: delete `join_via_lattice_with_context`. The engine fast-path calls `CapcoMarking::join_via_lattice(page_context.portions())`. The same-slice contract becomes a property of how the engine call is structured, not a runtime check.

**Decision — collapse.** The debug-assert was load-bearing when the engine's hot path threaded a `&PageContext` through *multiple* layers that each derived their own view; after Commits 1-3 there is exactly one derivation site (`project_from_page_context`), and that site uses `page_context.portions()` literally as the slice. There is no "mismatched slice" failure mode left to guard.

Net change:
- Delete `pub(crate) fn join_via_lattice_with_context` (lines 200-258 in `marking.rs`).
- `pub fn join_via_lattice(portions: &[CanonicalAttrs])` becomes the sole entry point. Its body is now `Self::join_via_lattice_body(portions)` directly (no tmp_ctx build).
- The one-shot tmp_ctx code in `join_via_lattice` (lines 188-198) goes away — it was building a `PageContext` only to satisfy the `_with_context` signature. With no `_with_context` variant, no tmp_ctx is needed.
- `project_from_page_context` (line 700) calls `CapcoMarking::join_via_lattice(page_context.portions())` then drives the rest of the pipeline (closure + page rewrites) inline. Reuse Commit 2's `project_attrs_pipeline` shape.

The body of `project_attrs_pipeline` (post-Commit-3) is now identical for both entries: `let joined = CapcoMarking::new(CapcoMarking::join_via_lattice(raw)); let mut out = self.closure(joined); ...page rewrites...`. Both the trait path and the engine fast-path land here naturally.

### Commit 5 — Engine call-site cleanup + workspace `_tmp_ctx` audit

**Files:**
- `crates/engine/src/engine.rs` — `project_page_marking` callsite (line 4504).
- Workspace-wide grep for `_tmp_ctx` / `_page_context` / `_page_ctx` / underscored `_ctx` params.

The engine's `project_page_marking` helper (engine.rs:4504-4525) currently takes `&PageContext` and forwards to `scheme.project_from_page_context(page_context)`. After Commits 1-4 the underlying chain accepts `&[CanonicalAttrs]` naturally. **PM decision required** — see §3 OQ-1 — on whether `project_page_marking` should:

- **Option A**: Keep its `&PageContext` parameter (engine API stays stable). It just calls `scheme.project_from_page_context(page_context)` and bridges to `ProjectedMarking`. This is what it does today.
- **Option B**: Take `&[CanonicalAttrs]` directly. The engine derives `page_context.portions()` at the call site (engine.rs:1219, 4298).

**Recommendation — Option A.** Two reasons: (i) `project_page_marking`'s callers are inside `Engine::lint`, which *holds* the `PageContext` as the accumulator across portions; naming the borrowed value at the call boundary keeps Engine's internal API self-documenting ("we project page state — here's the page state"). (ii) The engine has TWO call sites for `project_page_marking` (engine.rs:1219 lazy-init, engine.rs:4298 force-init in `dispatch_page_finalization`); changing both to thread `page_context.portions()` adds two `.portions()` calls and gains nothing the scheme layer would benefit from. The `&PageContext`-shaped engine boundary is correct.

So `project_from_page_context` survives as a `pub` API on `CapcoScheme` — engine still calls it, scheme still wraps `page_context.portions()` and forwards to `project_attrs_pipeline`. Body becomes:

```rust
pub fn project_from_page_context(&self, page_context: &marque_ism::PageContext) -> CanonicalAttrs {
    self.project_attrs_pipeline(page_context.portions())
}
```

**Workspace audit** for residue underscore-prefixed parameters that signal post-4b-E dead bridges:

- Walk `crates/*/src/**/*.rs` for `_tmp_ctx`, `_page_context`, `_page_ctx`. Expect: the two sites already named (retired in Commits 1+2). Stretch goal: any third site that surfaces is a bug — file an inline comment + retire-in-this-PR if mechanical, escalate if structural.
- Walk for `_ctx: &PageContext` style. Expect: none post-Commits-1+2.
- The pattern `_ctx: &RuleContext` is legitimate (rule implementations that don't read ctx); excluded from this audit.

### Commit 6 — `tasks.md` bookkeeping (T111-T115) + doc-comment sweep + parity-gate stale-citation update

**Files:**
- `specs/006-engine-rule-refactor/tasks.md` — tick T111-T115.
- `crates/capco/src/lattice.rs` — stale doc-comment refs to `PageContext::expected_*` (lines 2060-2095).
- `crates/capco/CAPCO-CONTEXT.md` — verify §3 reflects post-4b-E state (PR 4b-E already updated; spot-check for any 4b-F-relevant drift).

#### T111-T115 resolution table (verified 2026-05-18):

| Task | Description | Closed in PR | Resolution note |
|------|-------------|--------------|-----------------|
| T111 | Extend `Vocabulary<S>` with `is_fdr_dissem(token)` | Pre-4b series (Phase 5 PR-2 / #146) | Already shipped — `Vocabulary::is_fdr_dissem` / `is_fdr_dominator` live on `CapcoScheme`. CAPCO-CONTEXT.md memory line confirms `is_fdr_dissem` is in production use. |
| T112 | Per-category `Lattice` impls | PR 4b-A + PR 4b-B | Per-category lattice types landed across PR 4b-A (SciSet/SarSet/AeaSet/FgiSet) and PR 4b-B (ClassificationLattice/NatoClassLattice/JointSet/DissemSet/NatoDissemSet/RelToBlock/DeclassifyOnLattice). PR #456 split `Lattice` into halves; PR 4b-E added `DisplayOnlyBlock` / `NonIcDissemSet` / `DeclassExemptionAccumulator`. |
| T113 | Wire FOUO `SupersessionSet` over dissem axis via `is_fdr_dissem` | PR 4b-C (Pattern B) | `capco/non-fdr-control-evicts-fouo` PageRewrite on `CapcoScheme` uses `Vocabulary::is_fdr_dissem` for the non-FD&R sub-clause per CAPCO-2016 §H.8 p134. |
| T114 | Wire cross-axis FOUO eviction by classification > U | PR 4b-C (Pattern B) | `capco/classification-evicts-fouo` PageRewrite per §H.8 p134 classified-document sub-clause. |
| T115 | Delete `CapcoMarking::join`'s `PageContext` delegation; clean break | PR 4b-D.2 Commit 11 + completed in PR 4b-F | `impl JoinSemilattice for CapcoMarking` retired (Copilot R1 D24); the `_with_context` fast path called `PageContext` for residue axes until PR 4b-E migrated them to lattice-native helpers. **PR 4b-F removes the last `&PageContext` parameter from the body chain** — completing the structural intent of T115. |

#### Doc-comment sweep targets:

- `crates/capco/src/lattice.rs:2060-2095` (DissemSet doc) — three bullet items reference `page_context.rs:511 / 573-578 / 1085-1093` (retired files / line numbers). Rewrite as the post-PR-4b-E + post-PR-4b-C state:
  - "**Overlay 4 (NOFORN dominates) lives on the lattice path.**" (No PageContext divergence post-4b-E.)
  - "**FOUO classification-gate eviction lives on `scheme.project(Scope::Page, ...)`** via the `capco/classification-evicts-fouo` + `capco/non-fdr-control-evicts-fouo` PageRewrites declared on `CapcoScheme` (§H.8 p134)." (Drop the file:line ref.)
  - "**UCNI classification-gate strip lives on `scheme.project(Scope::Page, ...)`** via the `capco/{dod,doe}-ucni-evicted-by-classified` + `capco/{dod,doe}-ucni-promotes-noforn-when-classified` PageRewrites (§H.6 p116 / p118)." (Drop the file:line ref.)
- `crates/capco/src/scheme/actions/fgi.rs:20-37` — references `page_context.rs` line numbers that no longer correspond to anything. Re-anchor to symbolic refs ("`expected_fgi_marker`'s country-extraction step, retired in PR 4b-E").
- `crates/capco/src/scheme/marking.rs` lines 50-62 (module doc) — drop the "`_tmp_ctx` parameter is retained at the function boundary for signature stability" passage. Replace with: "PR 4b-F retired the last `&PageContext` parameter from the lattice fold body; the pipeline now consumes `&[CanonicalAttrs]` end-to-end."
- `crates/capco/src/scheme/marking.rs:244` — references `engine.rs:4540-4574`; the function `check_portions_unchanged` is actually at engine.rs:4559 (already stale before 4b-F). Replace with symbolic name only per memory `feedback_avoid_line_number_anchoring`.
- `crates/capco/src/scheme/marking.rs:274` — references `marking.rs lines 284-706 in the current revision`. Will drift further with PR 4b-F edits. Replace with prose count only ("~420 LOC").

Constitution VIII: every §-citation in the rewritten doc-comments re-verified at point of authorship against `crates/capco/docs/CAPCO-2016.md`. Use the `CAPCO-2016_citation_index.yml` finder per the memory line; never scroll-search the manual.

---

## §2 Risk register

| # | Risk | Severity | Mitigation |
|---|------|----------|------------|
| 1 | Removing the same-slice debug-assert weakens a structural invariant | LOW | The invariant becomes vacuous post-Commits-1-4 — there is only one derivation path for `raw`, from `page_context.portions()` at the engine. Document the structural property in `project_from_page_context`'s doc-comment so a future engineer reintroducing a parallel slice has a written contract to violate. |
| 2 | The parity gate at `crates/capco/tests/lattice_vs_scheme_parity.rs` calls `CapcoMarking::join_via_lattice(portions)` (the public wrapper). Signature change to internal callers (in Commit 4) could break tests if any test directly calls `_with_context`. | LOW | Grep `join_via_lattice_with_context` workspace-wide before Commit 4 lands. Today: 1 production caller, 0 test callers. Verify at implementation. |
| 3 | `project_from_attrs_slice` deletion (Commit 3) — any external consumer? | LOW | `pub(crate)`, in-crate only. Trait-path entry at `MarkingScheme::project` is the sole caller; inlines cleanly. Compiler will refuse if missed. |
| 4 | `project_page_marking` engine helper signature decision (Option A vs B) ripples to engine call sites. | LOW | Option A (recommended) is no engine signature change. Option B is two-callsite mechanical edit. Either is safe; stylistic / API-stability question, not correctness. |
| 5 | Stale doc-comment references to retired `PageContext::expected_*` accessors / `page_context.rs:NNN` line numbers persist in `lattice.rs` and `actions/fgi.rs`. Future maintainer follows a dead reference. | MEDIUM | Commit 6 sweeps the documented sites. Add a final grep for `page_context.rs:\d+` and `expected_dissem_us`, `expected_aea_markings`, etc. in `crates/capco/src/`. Any survivors are migrations missed in PR 4b-E — fix here. |
| 6 | `tasks.md` resolution notes might cite a PR number incorrectly | LOW | Investigate per task before claiming the PR number. The table in §1 Commit 6 is the verified mapping; re-verify at edit time. |
| 7 | Constitution VII §IV authorization scope (`marque-ism` not touched; `marque-engine` touched only if Option B for risk #4) | LOW | Precedent established by 4b-B/4b-C/4b-D.2/4b-D.3/4b-E. Document precedent in PR body. |
| 8 | `_tmp_ctx` workspace-wide grep might surface a third site we don't know about | LOW–MEDIUM | If a third site surfaces and is *mechanical* (post-PR-4b-E residue), fix in this PR. If *structural*, escalate as PM decision (§3 OQ-2). |
| 9 | Parity-gate test counts shift after doc-comment sweep — none, since no test code touched in 4b-F | NEGLIGIBLE | Test bodies unchanged. |
| 10 | `portion_count` and `is_empty` on PageContext — both called outside their own tests | NEGLIGIBLE | Both retain. No deletion candidate post-4b-E audit. |
| 11 | `#[allow(clippy::too_many_lines)]` attribute at `marking.rs:310-314` must not be dropped during signature edits | MEDIUM | Stable clippy errors immediately without it. Implementation MUST preserve. |
| 12 | Hardcoded line-number anchors in `marking.rs` already stale (e.g., `engine.rs:4540-4574` references `check_portions_unchanged` at actual line 4559) | HIGH | Commit 6 must replace with symbolic refs unconditionally — fix is independent of the signature work but rides in the same PR. |

---

## §3 PM decisions (resolved 2026-05-18)

**OQ-1: `project_page_marking` engine signature.** **RESOLVED: Option A.** Keep `&PageContext` parameter on the engine-internal helper. Engine internals self-document the page-state hand-off; zero engine-crate touches needed. `pub fn project_from_page_context(&PageContext)` survives as the engine-facing scheme API; body becomes a one-line forward to `self.project_attrs_pipeline(page_context.portions())`.

**OQ-2: Workspace `_tmp_ctx` audit — escalation policy.** **RESOLVED: (a) fix-in-PR if mechanical.** If a third `_tmp_ctx` / `_page_context` site surfaces and the body provably doesn't read the parameter (mirrors the known two sites), retire in this PR with an inline commit-message line naming the site. **If there's any structural ambiguity (parameter is read conditionally, or under a `cfg(...)`, or the function is `pub` and may have out-of-crate callers), STOP and surface to PM** — don't gold-plate by extending scope.

**OQ-3: Retire `PageContext` struct entirely (close T069)?** **RESOLVED: DEFER.** PageContext post-4b-F is a typed accumulator with documented invariant (issue #430 pre-size on `Arc<PageContext>::clone()` for the banner/CAB rule hand-off). Full retirement crosses Constitution VII §IV precedent envelope (4-crate ripple including `marque-rules` `RuleContext` field type); properly belongs to its own PR per task text "PR 6c". Stay surgical.

**OQ-4: tasks.md resolution-note PR-citation form.** **RESOLVED: both forms.** Cite as `PR 4b-X / #YYY` per task — the marque-internal series is what every plan / decisions / CAPCO-CONTEXT.md doc uses; the GitHub number is what `gh pr view` resolves. Mapping table in Commit 6 §1 above already captures both columns where verified.

**OQ-5: Constitution VII §IV authorization framing.** **RESOLVED: yes.** PR 4b-F is the structural close of the engine refactor's signature-cleanup arc. Within-006 precedent satisfied verbatim (no `marque-ism` touches, no `marque-engine` touches under OQ-1 Option A). Name precedent in PR body verbatim from the §5 authorization argument.

**Implementation discipline (PM directive — not optional)**:

1. **Walk the adjacent callsites.** Per memory `feedback_audit_predicates_against_source`, agents historically take fixes and don't walk the logical code paths adjacent to those fixes. If you change `join_via_lattice_body`'s signature, the doc-comment in the same function, the module-level doc-comment at lines 50-62, the doc-comments in `marking_scheme_impl.rs` referencing `_with_context`, and the doc-comments in `lattice.rs` / `actions/fgi.rs` all need to march together. If something big was overlooked in one place, the same wide-net discipline applies to every other callsite.

2. **Constitution VIII at every citation rewrite.** Every §-citation in a rewritten doc-comment MUST be re-verified against `crates/capco/docs/CAPCO-2016.md` at point of authorship, using the `CAPCO-2016_citation_index.yml` finder. Page-number form only (`§X.Y pNN`).

3. **Per-commit `cargo +stable clippy --workspace -- -D warnings`**, not just final. The CI proxy is stable clippy (per memory `feedback_clippy_nightly_vs_stable_drift`) — local nightly clippy can lull you into thinking a commit is clean when CI will reject it.

4. **G13 panic-message discipline**: any panic-message edit MUST stay counts-only. Reference the existing `check_portions_unchanged` pattern at `crates/engine/src/engine.rs` (`fn check_portions_unchanged` — use the symbolic name, NOT a line number, per memory `feedback_avoid_line_number_anchoring`).

5. **"Will we want to maintain this for 5 years?"** is the quality bar. If yes, you did it right. If no, re-do until yes.

---

## §4 Test plan

The parity gate at `crates/capco/tests/lattice_vs_scheme_parity.rs` is the load-bearing regression catch. Both paths (`CapcoMarking::join_via_lattice` and `scheme.project(Scope::Page, ...)`) land at `project_attrs_pipeline`; must produce identical output to pre-4b-F runs.

### Required pre-merge checks:

1. **Parity-gate green.** `cargo test --test lattice_vs_scheme_parity -p marque-capco` — all 74 fixtures.
2. **Corpus regression sweep.** `cargo test -p marque-engine --test corpus_*` — all five corpora.
3. **Engine internal tests** at `crates/engine/tests/` — especially `closure_hotpath.rs` (calls `CapcoMarking::join_via_lattice` directly) and `rule_panic_isolation.rs`.
4. **PageContext shim_tests** at `crates/ism/src/page_context.rs:188-274` — 6 tests pinning the issue #430 pre-size invariant.
5. **Lattice law tests** at `crates/capco/tests/category_lattice_laws.rs`.
6. **Compile-time `Send + Sync`** at `crates/ism/tests/send_sync.rs`.
7. **`cargo check --workspace`** + **`cargo +stable clippy --workspace -- -D warnings`** clean per commit.
8. **Bench drift** — `lint_10kb` baseline ~828µs. Expected delta: ≤1µs (signature-only). Pass within ±5%.

### Debug-assert relocation:

- The `read-only-attrs sentinel` at `project_attrs_pipeline_with_context` (current marking_scheme_impl.rs:745-763) — survives Commit 2 verbatim. Snapshots `raw` against itself across `closure()`; unrelated to `page_ctx`. G13 counts-only panic preserved.
- The `same-slice contract debug-assert` at `join_via_lattice_with_context` (current marking.rs:245-256) — retires in Commit 4 alongside `_with_context`. Structural contract becomes "engine derives `raw` from `page_context.portions()` at call site" — documented in `project_from_page_context`'s doc-comment, not runtime-checked.

---

## §5 Constitution check

### Principle VII §IV (engine-crate touch authorization)

PR 4b-F touches:
- `marque-capco` — primary scope (scheme/marking.rs, scheme/marking_scheme_impl.rs, src/lattice.rs doc-comments, scheme/actions/fgi.rs doc-comments).
- `marque-engine` — only if Option B for OQ-1 (NOT recommended); otherwise zero edits.
- `marque-ism` — zero edits in 4b-F (PageContext shim correct as-is; T069 deferred per OQ-3).
- `specs/006-engine-rule-refactor/tasks.md` — bookkeeping; not an engine crate.

**Authorization argument** (for PR body):

> PR 4b-F closes the signature residue left by PR 4b-E's `PageContext::expected_*` deletion. After 4b-E, three layers of the lattice + projection pipeline carry `&PageContext` parameters whose bodies no longer read them. PR 4b-F retires the dead parameters and consolidates the pipeline to a single `&[CanonicalAttrs] → CanonicalAttrs` shape end-to-end. Constitution VII §IV blocks scheme-adoption PRs from editing engine crates; this PR is the engine refactor's structural close, not a scheme adoption. Within-006 precedent: PR 4b-B Commit 2, PR 4b-C Commit 5, PR 4b-D.2 / .3, PR 4b-E. PR 4b-F touches no `marque-ism` source (only `marque-capco`); the precedent envelope strictly accommodates this scope.

### Principle V Constitution G13 (audit-record content-ignorance)

PR 4b-F preserves G13 by construction. The two surviving panic sites are content-ignorant:
1. `project_attrs_pipeline`'s read-only-attrs sentinel — counts only.
2. `check_portions_unchanged` (engine.rs) — counts only.

The retiring `join_via_lattice_with_context` debug-assert was also counts-only. No regression.

### Principle VIII (citation fidelity)

Doc-comment sweep in Commit 6 re-anchors stale `page_context.rs:NNN` and `engine.rs:NNNN` line-number references to symbolic refs. Every §-citation in rewritten doc-comments re-verified against `crates/capco/docs/CAPCO-2016.md` at point of authorship per the `CAPCO-2016_citation_index.yml` finder convention. Page-numbers-only form (`§X.Y pNN`), never bare `§NN` or `line NNNN`.

### Principle IV (Two-Layer Rule Architecture) + Principle VI (Dataflow Pipeline Model)

No Layer 1 (generated predicates) or Layer 2 (rule implementations) edits other than doc-comments. Pipeline simplification *strengthens* Principle VI's single-source-of-truth claim.

---

## §6 Reviewer-attestation checklist

For inclusion in the PR description body.

### Pre-merge gates

- [ ] `cargo check --workspace` passes after every commit.
- [ ] `cargo test --workspace` passes — parity gate, lattice law tests, engine integration tests, corpus regression sweep.
- [ ] `cargo +stable clippy --workspace -- -D warnings` passes per memory `feedback_clippy_nightly_vs_stable_drift`.
- [ ] Bench `lint_10kb` within ±5% of staging baseline.
- [ ] Workspace grep for `_tmp_ctx` / `_page_context` / `_page_ctx` returns zero matches in `crates/*/src/`.
- [ ] Workspace grep for `page_context.rs:\d+` (file:line refs) in `crates/*/src/` returns zero matches.
- [ ] Workspace grep for `expected_dissem_us` / `expected_aea_markings` / `expected_classification` / `render_expected_banner` returns zero matches in `crates/*/src/`.

### Architectural attestations

- [ ] **Pipeline shape**: `join_via_lattice_body` consumes only `&[CanonicalAttrs]`; `project_attrs_pipeline` consumes only `&[CanonicalAttrs]`; `join_via_lattice_with_context` is retired.
- [ ] **Public API surface**: `CapcoScheme::project_from_page_context` survives unchanged (engine-facing API stability). `CapcoMarking::join_via_lattice` survives unchanged (test + parity gate stability). `CapcoScheme::project_from_attrs_slice` retires (it was `pub(crate)`).
- [ ] **`PageContext` shim**: untouched. Surface: `Default`, `Clone`, `new`, `add_portion`, `portion_count`, `is_empty`, `portions`. Issue #430 pre-size invariant preserved.
- [ ] **Parity gate**: file `lattice_vs_scheme_parity.rs` unchanged.

### Citation discipline (Constitution VIII)

- [ ] Every §-citation in 4b-F's doc-comment rewrites re-verified against `crates/capco/docs/CAPCO-2016.md`.
- [ ] Page-number form only (`§X.Y pNN`).
- [ ] Stale `page_context.rs:NNN` and `engine.rs:NNNN` line-number references retired or re-anchored to symbolic refs.

### tasks.md bookkeeping

- [ ] T111-T115 ticked with one-line resolution notes naming both the marque-internal PR series (PR 4b-X) and the GitHub PR number.
- [ ] T069 stays unchecked. Resolution note: "PR 4b-F architect investigated; deferred per OQ-3."

### Final sentinel

- [ ] PR description names the within-006 Constitution VII §IV authorization precedent verbatim.
- [ ] PR title: "PR 4b-F: retire `&PageContext` residue parameters from lattice fold + close PR-4 tasks bookkeeping."

---

**End of plan.**
