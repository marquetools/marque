<!-- SPDX-FileCopyrightText: 2026 Knitli Inc. -->
<!-- SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0 -->

# PR 4b-F Rust Preflight — Signature Cleanup for join_via_lattice / project Pipeline

**Date:** 2026-05-18  
**Reviewer role:** Read-only Rust-semantic risk register  
**Branch:** `pr-4b-f-residue-cleanup` (post-PR-4b-E, at `c3f544d6`)  
**Paired document:** architect's tactical plan (parallel artifact)

---

## Pre-flight Status

`cargo check --workspace`: **clean**  
`cargo +stable clippy --workspace -- -D warnings`: **clean**  
`cargo fmt --check`: not run (no source edits on this branch yet — verify pre-merge)  
`cargo test`: not run (requires implementer to execute)

The workspace currently compiles without warnings or errors. The cleanup
territory described below is all _future work_ on this branch; the risk
register characterizes what can go wrong during that work.

---

## §1 API-Stability Classification

### The six entry points under review

| Function | Location | Current visibility | Risk grade | Notes |
|---|---|---|---|---|
| `CapcoMarking::join_via_lattice` | `marking.rs:187` | `pub` | **HIGH** | Public API; external consumers possible post-publish. The trait-path `MarkingScheme::project` calls `project_from_attrs_slice` → `project_attrs_pipeline_with_context` → `join_via_lattice_with_context`. Tests in `lattice_vs_scheme_parity.rs:82` call `join_via_lattice` directly. Signature change requires coordinated test update. |
| `CapcoMarking::join_via_lattice_with_context` | `marking.rs:230` | `pub(crate)` | **MEDIUM** | Crate-internal. Only callers are `join_via_lattice` (same file, line 197) and `project_attrs_pipeline_with_context` (marking_scheme_impl.rs:748). Safe to change with a crate-local grep. |
| `CapcoMarking::join_via_lattice_body` | `marking.rs:315` | `fn` (private) | **LOW** | Private; only called from `join_via_lattice_with_context`. No external surface. |
| `CapcoScheme::project_from_attrs_slice` | `marking_scheme_impl.rs:676` | `pub(crate)` | **MEDIUM** | Crate-internal. Only caller is `MarkingScheme::project` trait body at `marking_scheme_impl.rs:254`. |
| `CapcoScheme::project_from_page_context` | `marking_scheme_impl.rs:700` | `pub` | **HIGH** | Public API. Called from `engine.rs:4523` (production hot path), `benches/profile_project.rs:184,219` (benchmarks). Parameter type `&marque_ism::PageContext` is load-bearing — the engine passes its document-local `PageContext` accumulator here. A parameter-type change (e.g., to `&[CanonicalAttrs]`) would require engine-crate edits subject to Constitution VII §IV authorization. |
| `CapcoScheme::project_attrs_pipeline_with_context` | `marking_scheme_impl.rs:720` | `fn` (private) | **LOW** | Private body. Only callers are `project_from_attrs_slice` (line 685) and `project_from_page_context` (line 704). Safe to refactor freely. |

### API-stability conclusion

**The key determination for PR 4b-F**: the `_tmp_ctx` parameter in
`join_via_lattice_body` (private) does NOT appear in any public or `pub(crate)`
signature. The `pub fn join_via_lattice(portions)` signature does not carry a
`PageContext` at all. The `pub(crate) fn join_via_lattice_with_context(portions,
page_ctx)` does carry `page_ctx` — but its doc comment (marking.rs:226-229)
explicitly anticipates removing it once the body no longer reads it.

Removing `_tmp_ctx` from `join_via_lattice_body` (private, LOW risk) requires
also updating `join_via_lattice_with_context` (pub(crate), MEDIUM risk) if that
function's debug-assert is relocated per §2 Option C. The `pub fn
project_from_page_context` signature stays untouched regardless of which §2
option is chosen — the debug-assert lives inside `project_attrs_pipeline_with_context`
(private), not in the public entry point.

---

## §2 Debug-Assert Relocation Analysis

### Current state

The same-slice contract ("portions == page_ctx.portions()") is currently
asserted at two levels:

1. **`join_via_lattice_with_context`** (marking.rs:245-256): explicit
   `#[cfg(debug_assertions)]` block using `if != { panic!(...) }` with
   counts-only message (G13 compliant).
2. **`project_attrs_pipeline_with_context`** (marking_scheme_impl.rs:745-763):
   a different contract — asserting that `closure()` did not mutate `raw`.
   This is also `#[cfg(debug_assertions)]` and counts-only (G13 compliant).

The `_tmp_ctx` in `join_via_lattice_body` is dead (prefixed `_`; not read).
The only reason the body still receives it is "signature stability with the
engine's hot path" (module-level doc, marking.rs:60-62). But
`project_from_page_context` passes `page_ctx` through `project_attrs_pipeline_with_context`
which in turn passes it to `join_via_lattice_with_context` — so the engine's
hot path does touch `join_via_lattice_with_context`, not `join_via_lattice_body`
directly. The body's receipt of `_tmp_ctx` is a precautionary fiction.

### Three options, ranked

**Option A — Drop `_tmp_ctx` from body; contract stays in `_with_context`** (Recommended)

Drop the `_tmp_ctx: &PageContext` parameter from `join_via_lattice_body` entirely.
The same-slice contract continues to live in `join_via_lattice_with_context`,
which is the only reachable caller of the body (besides `join_via_lattice` which
builds its own tmp_ctx and then checks the contract via `_with_context`).

- Risk: **LOW**. Only the private `fn` signature changes. The `pub(crate)` caller
  is updated in the same commit.
- G13: unaffected. The `panic!` message stays in `_with_context` with its
  current counts-only text.
- `#[cfg(debug_assertions)]` interaction: preserved. The block stays in
  `_with_context`; no cfg gating is needed on the body.
- Doc-comment consequence: the doc on `join_via_lattice_body` at line 266
  ("as the per-axis input and `tmp_ctx` for the residue-axis accessor surface
  that PageContext still bridges (PR 4b-E retires the residue bridge)") becomes
  stale and needs updating (see §4).

**Option B — Drop `_tmp_ctx` from body; move contract to `project_attrs_pipeline_with_context`** (Not recommended)

Drop `_tmp_ctx` from both the body and `_with_context`; assert the contract in
`project_attrs_pipeline_with_context` only, where `raw` and `page_ctx.portions()`
are also available for the closure-mutation sentinel.

- Risk: **MEDIUM**. Removes the same-slice contract from `join_via_lattice_with_context`,
  which could be called directly in tests or future in-crate callers. Promotion
  to `pub` later (per the doc comment: "If an out-of-crate caller's use case
  requires it, promote back to pub") would surface a version without the
  same-slice contract. The contract would silently fail to fire on a hypothetical
  future direct caller of `_with_context`.
- Rejected for the 5-year maintainability reason: the contract documents a
  semantic invariant about the pair `(portions, page_ctx)` — it belongs at
  the entry point that accepts both, not buried in the pipeline body.

**Option C — Promote `_with_context` to a thin contract-only wrapper; drop param from body** (Option A is simpler)

Retains `_with_context` explicitly as a named contract-enforcement layer. This
is what Option A already does — `_with_context` is already a thin wrapper that
asserts and delegates. No structural change from Option A.

### Summary

Choose Option A. Drop `_tmp_ctx` from `join_via_lattice_body`, update the doc
comment on the body to remove the stale "residue bridge" claim, update
`join_via_lattice` to pass `&tmp_ctx` to `_with_context` unchanged, and update
the `_with_context` call inside `project_attrs_pipeline_with_context` to still
pass `page_ctx`. No behavioral change at any call site.

### G13 preservation requirement

The current `panic!` in `join_via_lattice_with_context` uses only `portions.len()`
and `page_ctx.portions().len()` — counts only, no document content. Any edit to
that panic message must preserve this property. Search for the string
`"join_via_lattice_with_context: portions slice"` to find the message; verify
the replacement message contains no format placeholders that would print
`CanonicalAttrs` or its fields.

---

## §3 Engine-Crate Touch Authorization Map

The proposed cleanup touches `marque-capco` only (private body signature
change + `pub(crate)` entry point update). No `marque-engine`, `marque-ism`,
`marque-scheme`, `marque-core`, or `marque-rules` edits are required.

| File | Change | Engine-crate? | Auth required? | Precedent |
|---|---|---|---|---|
| `crates/capco/src/scheme/marking.rs` | Drop `_tmp_ctx` from `join_via_lattice_body`; update doc comments | No (marque-capco) | Unrestricted | Unrestricted |
| `crates/capco/src/scheme/marking_scheme_impl.rs` | Update call to `join_via_lattice_with_context` if body param changes (no change if Option A); update doc comments | No (marque-capco) | Unrestricted | Unrestricted |
| `crates/capco/src/scheme/marking.rs:187-197` | `join_via_lattice` wrapper: if `_with_context` no longer passes a separate ctx, this wrapper builds tmp_ctx and passes it (unchanged under Option A) | No (marque-capco) | Unrestricted | Unrestricted |
| `crates/ism/src/page_context.rs` | No changes anticipated | Yes (marque-ism, restricted) | Would require within-006 authorization | PR 4b-B/C/E precedent; but NO changes needed for this PR |
| `crates/engine/src/engine.rs` | No changes anticipated | Yes (marque-engine, restricted) | Would require within-006 authorization | PR 4b-D.2 precedent; but NO changes needed |

**Conclusion: PR 4b-F requires zero engine-crate edits if the signature cleanup
is scoped to removing `_tmp_ctx` from `join_via_lattice_body` (Option A). The
`pub fn project_from_page_context(&self, page_context: &marque_ism::PageContext)`
signature stays intact — the engine passes `&page_context` to this method and
will continue to do so.**

If the architect's plan proposes collapsing `project_from_page_context` to
accept `&[CanonicalAttrs]` instead of `&PageContext`, that would require editing
`crates/engine/src/engine.rs:4523` — a marque-engine touch requiring within-006
authorization (precedent: PR 4b-D.2 hot-path flip). Flag this as OQ-1 below.

---

## §4 Doc-Comment / Citation Drift List

Every reference that will become stale or misleading after the `_tmp_ctx` removal:

### In `crates/capco/src/scheme/marking.rs`

| Line(s) | Current text | Problem after edit | Required action |
|---|---|---|---|
| 60-62 (module doc) | "`_tmp_ctx` parameter is retained at the function boundary for signature stability with the engine's hot path; the body no longer reads it." | Describes current state — must change once `_tmp_ctx` is dropped | Update to reflect the parameter is gone; engine path calls `_with_context` not `_body` directly. |
| 188-197 (`join_via_lattice` body) | "Build a one-shot tmp_ctx for residue-axis accessor calls and delegate to the borrowed-context variant." | "Residue-axis accessor calls" is stale post-4b-E; the only reason for tmp_ctx is the same-slice contract | Update to: "Build a one-shot tmp_ctx to satisfy the `join_via_lattice_with_context` same-slice contract; the body itself does not read it." |
| 266-268 (`join_via_lattice_body` doc) | "as the per-axis input and `tmp_ctx` for the residue-axis accessor surface that PageContext still bridges (PR 4b-E retires the residue bridge — see the module-level doc)." | After removal, the body no longer has a `tmp_ctx` parameter at all | Delete this sentence from the body doc comment. |
| 273-274 (`join_via_lattice_body` doc) | "Clippy's `too_many_lines` lint fires on this function at ~423 LOC (function body spans `crates/capco/src/scheme/marking.rs` **lines 284-706 in the current revision**)" | **HARDCODED LINE NUMBERS.** Any edit that inserts or removes lines before the function body makes this stale. | Replace with a prose description of the function's purpose that does not anchor to line numbers. E.g.: "the function body is ~420 LOC" with no "current revision" anchor. |
| 317-323 (`join_via_lattice_body` parameters comment) | "PR 4b-E: `_tmp_ctx` retained at the boundary so the engine's hot path keeps passing a `&PageContext` reference (no signature churn for the caller). The body no longer reads it..." | After removal, the entire comment block becomes dead | Delete this comment block entirely when the `_tmp_ctx` parameter is removed. |
| 243-244 (contract block in `_with_context`) | "the `check_portions_unchanged` pattern at `crates/engine/src/engine.rs:4540-4574`" | **HARDCODED LINE NUMBERS** in a cross-file reference. `check_portions_unchanged` is at `engine.rs:4559` (verified), not 4540-4574 — already stale. Any engine refactor will break this silently. | Replace with a function-name reference: "the `check_portions_unchanged` pattern in `crates/engine/src/engine.rs`" (no line numbers, per user feedback `feedback_avoid_line_number_anchoring.md`). |
| 454-459 (comment in `join_via_lattice_body`) | "PR 4b-D.2 Commit 7+: tmp_ctx is now received by reference from the caller... the engine path skips that round via `join_via_lattice_with_context`." | After `_tmp_ctx` is removed from the body, this comment describes a parameter that no longer exists. | Delete or rewrite to: "The engine path calls `project_from_page_context` → `project_attrs_pipeline_with_context` → `join_via_lattice_with_context`; this body is the final hot-path composition step." |

### In `crates/capco/src/scheme/marking_scheme_impl.rs`

| Line(s) | Current text | Problem | Required action |
|---|---|---|---|
| 676-685 (`project_from_attrs_slice` doc/body) | "Build a one-shot tmp_ctx for residue-axis accessors and delegate to the borrowed-context pipeline." | "residue-axis accessors" is stale post-4b-E | Update to reflect that tmp_ctx is only needed for the same-slice debug-assert in `join_via_lattice_with_context`. |
| 709-719 (`project_attrs_pipeline_with_context` doc) | "`raw` and `page_ctx.portions()` MUST refer to the same slice (caller's contract — debug-asserted by `join_via_lattice_with_context`)." | Accurate — but will need to be kept in sync if Option A drops `_tmp_ctx` from the body while `_with_context` retains the assert. | Keep as-is under Option A. |

### Hardcoded line-number risk summary

Two hardcoded line-number anchors require removal regardless of whether `_tmp_ctx`
is dropped:

1. `marking.rs:273-274`: "lines 284-706 in the current revision"
2. `marking.rs:243-244`: "engine.rs:4540-4574" (already stale; actual position is 4559)

Both violate the project convention against line-number anchoring in code
comments (`feedback_avoid_line_number_anchoring.md`). The PR 4b-F cleanup
should fix both, independent of the `_tmp_ctx` decision.

---

## §5 Test Impact

### `crates/capco/tests/lattice_vs_scheme_parity.rs` (parity gate)

The parity gate calls `CapcoMarking::join_via_lattice(portions)` at line 82.
This is the `pub` entry point — its signature is `(portions: &[CanonicalAttrs])
-> CanonicalAttrs` with no `PageContext` parameter. **No signature update
needed.** The gate exercises the full lattice pipeline indirectly through
`join_via_lattice` → `join_via_lattice_with_context` (tmp_ctx built internally)
→ `join_via_lattice_body`. Post-cleanup the gate continues to work identically.

### `crates/capco/tests/scheme_equivalence.rs`

Does **not** exist post-PR-4b-E (confirmed deleted: `ls` returned nothing).
No action needed.

### `crates/engine/benches/profile_project.rs`

Calls `scheme.project_from_page_context(&page_context)` at lines 184 and 219.
Public `pub fn` stays intact. **No update needed.**

### Tests in `crates/capco/src/scheme/marking.rs` (inline `#[cfg(test)]`)

No inline tests in `marking.rs` (file is 894 lines; no `#[cfg(test)]` module
was found). Inline tests for `join_via_lattice_body` behavior, if they existed,
would need parameter updates.

### `crates/ism/tests/send_sync.rs`

`assert_impl_all!(PageContext: Send, Sync)` and `assert_impl_all!(CanonicalAttrs:
Send, Sync)` at lines 21-22. `PageContext` is not modified by PR 4b-F. These
assertions continue to pass. **No action needed.**

### `crates/capco/tests/lattice_vs_scheme_parity.rs` — residue comment

Line 10 of the test file's module doc comment references `join_via_lattice`
alongside the retired `PageContext::expected_*` path as historical context.
If the module doc is edited as part of the 4b-F cleanup, ensure it stays
historically accurate about the three-path evolution. No functional change
needed.

---

## §6 Stable-Clippy / Nightly Drift Risks

### Verified clean

`cargo +stable clippy --workspace -- -D warnings` passes on the current branch
(confirmed run above). No pre-existing stable-clippy issues.

### `#[allow(clippy::too_many_lines)]` survival

The `#[allow(clippy::too_many_lines, reason = "...")]` attribute at
`marking.rs:310-314` is directly above `fn join_via_lattice_body`. Removing
`_tmp_ctx` from the body does not affect the line count in a way that would
make the lint stop firing — the body is ~422 lines regardless of the one-line
parameter drop. The `#[allow]` attribute must be **retained** after the edit;
it is explicitly documented as permanent in the body's doc comment (line 304).

The `reason` qualifier (`reason = "..."`) on `#[allow]` requires Rust 1.81+.
The workspace's `rust-version = "1.85"` in `Cargo.toml` satisfies this.
Stable clippy on 1.85 will not reject it.

### `clippy::allow_attributes` / `clippy::allow_attributes_without_reason`

On stable Rust 1.85, `clippy::allow_attributes_without_reason` is a lint that
fires when a `#[allow]` attribute lacks a `reason`. The existing `#[allow]` on
`join_via_lattice_body` already carries a `reason` string (line 312), so this
lint will not fire.

### Stable vs nightly drift on `too_many_lines`

The `too_many_lines` lint threshold is 100 lines on both stable and nightly
clippy. The 422-LOC body far exceeds this; the `#[allow]` is load-bearing on
both toolchain tracks.

### `dead_code` risk

If `_tmp_ctx` is dropped from `join_via_lattice_body`, and `join_via_lattice`
continues to build a `tmp_ctx` to pass to `_with_context`, no new dead-code
warnings arise — `tmp_ctx` is still used in `join_via_lattice`. If a future
refactor also eliminates `join_via_lattice`'s tmp_ctx build, verify that
`join_via_lattice_with_context` still has callers before removing its contract.

---

## §7 Open Questions for PM

### OQ-1 — `project_from_page_context` parameter type

**Context:** The architect's tactical plan may propose collapsing
`project_from_page_context(&self, page_context: &PageContext)` to take
`&[CanonicalAttrs]` directly, rendering `PageContext` optional on the
`project_*` call chain.

**Impact:** Any change to `pub fn project_from_page_context`'s parameter type
is a **breaking change to a public API** and requires editing `marque-engine`
(`engine.rs:4523` calls `scheme.project_from_page_context(page_context)`). A
marque-engine edit requires within-006 authorization per Constitution VII §IV.

**Options:**
- A. Keep `pub fn project_from_page_context(&self, page_context: &PageContext)`
  unchanged. The engine hot-path continues to pass `&PageContext`. No engine
  touch needed. (Recommended for PR 4b-F scope discipline.)
- B. Add a new overload `pub fn project_from_portions(&self, portions: &[CanonicalAttrs])`
  alongside `project_from_page_context`; deprecate the old form in a separate PR.
  (Pre-users, so no deprecation phasing needed per memory — just rename and update
  the single engine call site.)
- C. Replace `project_from_page_context` entirely with `project_from_portions`.
  Requires marque-engine touch; authorize via within-006 precedent.

**PM decision needed:** is `project_from_page_context`'s parameter type in scope
for 4b-F, or is it frozen for a later PR once `PageContext` retirement is on the
roadmap?

### OQ-2 — `join_via_lattice_with_context` visibility post-cleanup

**Context:** Once `_tmp_ctx` is dropped from `join_via_lattice_body` and the
same-slice contract stays in `_with_context`, the `pub(crate)` visibility of
`_with_context` could be lowered to `fn` (fully private) since its only callers
are `join_via_lattice` (line 197) and `project_attrs_pipeline_with_context`
(marking_scheme_impl.rs:748) — both in the same crate.

**Options:**
- A. Keep `pub(crate)` as-is. The doc comment already explains why it's not
  `pub` (same-slice contract would be unguarded for cross-crate callers). Keeping
  `pub(crate)` leaves the door open for future in-crate callers without a full
  visibility change.
- B. Lower to `fn` (private). Slightly tighter encapsulation; callers remain
  the same two sites. Any future in-crate caller that needs the `_with_context`
  form would need a visibility change.

**Recommendation:** Option A. The `pub(crate)` visibility is already the right
trade-off documented in the existing doc comment. Don't tighten further unless
there is a positive reason to lock out future in-crate callers.

**PM decision needed:** confirm whether narrowing `_with_context` to private is
desired, or whether `pub(crate)` is preferred for future in-crate flexibility.

### OQ-3 — `join_via_lattice` wrapper: drop tmp_ctx entirely?

**Context:** `join_via_lattice` (the public entry) currently builds a one-shot
`tmp_ctx` just to pass to `_with_context` for the debug-assert. Post-Option A,
`_tmp_ctx` is dead in the body — the only consumption of `tmp_ctx` in
`join_via_lattice` is the `_with_context` call's second argument, which is
needed solely for the debug-assert.

If `_with_context` becomes private (OQ-2, Option B), the wrapper could call
`_body` directly (bypassing `_with_context` and its contract) and move the
same-slice check into `join_via_lattice` itself. But `join_via_lattice` only
receives `portions: &[CanonicalAttrs]` — there is no "page_ctx" to check
against; the tmp_ctx IS the "page_ctx" built from `portions`. The same-slice
contract degenerates to `portions == tmp_ctx.portions()` which is trivially
true by construction — so the assert becomes vacuous and can be dropped
entirely on this code path.

**This is safe**: `join_via_lattice` is the path where no pre-built `PageContext`
exists. The same-slice invariant is only meaningful for the hot-path where
the engine's existing `PageContext` (built from a potentially-different slice)
is being passed alongside `portions`. That path goes through
`project_from_page_context` → `project_attrs_pipeline_with_context` →
`join_via_lattice_with_context`, where the assert is non-trivial.

**Options:**
- A. Keep `join_via_lattice` building tmp_ctx and passing to `_with_context`.
  The vacuous assert fires in debug builds but is harmless. Simpler code path.
- B. Have `join_via_lattice` call `_body` directly (bypassing `_with_context`)
  and eliminate the tmp_ctx build entirely. Saves n×clone per call on the non-hot-path
  `join_via_lattice` entry. Requires verifying no other semantic difference
  between the two call paths (currently none — `_with_context` only asserts).

**PM decision needed:** is eliminating the n×clone in `join_via_lattice` worth
the added structural complexity?

---

## Fixpoint: adjacent risks not in the stated scope

The following are pre-existing issues in the cleanup territory that were not
introduced by PR 4b-F but that a future reviewer will encounter in these files.
They do not block PR 4b-F but are flagged per the "walk-adjacent-callsites"
discipline:

1. **`project_from_attrs_slice` doc comment** (marking_scheme_impl.rs:676-685)
   references "residue-axis accessors" — stale post-4b-E. This is in the same
   function that PR 4b-F may touch; fix opportunistically.

2. **`join_via_lattice_body` `#[allow]` `reason` field** (marking.rs:312-313):
   the reason string ends with `"see doc comment above."` — after editing the
   doc comment per §4, verify the `reason` string still accurately summarizes
   the justification. The `reason` attribute is displayed in IDE diagnostics;
   a stale reason is a minor mislead.

3. **`closure.rs:318`** references `CapcoScheme::project_attrs_pipeline_with_context`
   (the shared body). If the private function is renamed in a future PR, this
   comment will drift. No action for PR 4b-F but worth noting.

4. **Interior-mutability confirmation**: `_tmp_ctx` is `&marque_ism::PageContext`
   (shared reference, not `&mut`). `PageContext` has no `UnsafeCell<_>` in its
   fields (`portions: Vec<CanonicalAttrs>` is plain owned data). There is no
   interior mutability via `_tmp_ctx` and no risk of unexpected state modification
   through the dead parameter. This is not a risk — confirming it for the record.

---

## Risk Summary Table

| Rank | Item | Severity | Blocking? |
|---|---|---|---|
| 1 | **Hardcoded line-number anchors** in `marking.rs:273-274` ("lines 284-706") and `marking.rs:243-244` ("engine.rs:4540-4574") | HIGH | No — stale now, worse after edits |
| 2 | **`project_from_page_context` parameter type** (OQ-1) — if architect plans a type change, engine-crate touch authorization required | HIGH | PM decision required before architect plan is finalized |
| 3 | **`_tmp_ctx` removal under Option A** is LOW-risk by itself, but the `pub(crate) join_via_lattice_with_context` doc comment references "Engine::fix_inner / check_portions_unchanged patterns" that apply to the contract's semantics — these doc-comment references must be updated atomically with the `_tmp_ctx` removal to avoid stale rationale | MEDIUM | No — cosmetic, but Constitution VIII citation-fidelity applies to code comments too |
| 4 | **`#[allow(clippy::too_many_lines)]` must survive** the body edit; confirm the attribute is not accidentally dropped | MEDIUM | No — would immediately surface as a stable-clippy failure |
| 5 | **Parity gate (`lattice_vs_scheme_parity.rs`)** calls `join_via_lattice` (public) not `join_via_lattice_with_context` — public signature must stay `(portions: &[CanonicalAttrs])` | HIGH | No (signature is not changing) — but verify |
