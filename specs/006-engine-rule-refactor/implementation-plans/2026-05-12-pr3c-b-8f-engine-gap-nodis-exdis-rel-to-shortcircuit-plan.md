# PR 3c.B Sub-PR 8.F-engine-gap: NODIS/EXDIS short-circuit in `PageContext::expected_rel_to`

**Branch**: `refactor-006-pr-3c-b-8f-engine-gap-noforn-implies-clears-rel-to`
**Parent**: `staging` (8.F + 8.F.2 merged: #393, #394)
**Issue tag**: Engine-gap for §H.9 NODIS/EXDIS NOFORN-supremacy.

## 1. The gap

The just-merged sub-PRs (8.F #393 / 8.F.2 #394) declared four `PageRewrite`s for the §H.9 Pattern A family — `capco/{nodis,exdis,sbu-nf,les-nf}-implies-noforn`. Each adds NOFORN to `CAT_DISSEM` at `Scope::Page` when its trigger appears in any portion. The existing `capco/noforn-clears-rel-to` rewrite then clears `CAT_REL_TO`. The composition is sound at the **`scheme.project(Scope::Page, ...)` layer**.

But marque has a parallel read API — `PageContext::expected_rel_to()` / `expected_non_ic_dissem()` (`crates/ism/src/page_context.rs`) — that rules consult directly when they need the page-projected REL TO list. It walks raw portion data and applies supersession logic **inline**, not by routing through the scheme. Today that inline logic short-circuits on:

- Any portion carrying `DissemControl::Nf` (line 396-403)
- The non-IC SBU-NF/LES-NF split injecting NF in classified context (line 405-409, via `needs_nf_from_split`)

It **does not** short-circuit on a portion carrying `NonIcDissem::Nodis` or `NonIcDissem::Exdis`, despite two explicit CAPCO-2016 §H.9 passages — one per token — establishing the requirement:

- **§H.9 p172 (EXDIS)** (`crates/capco/docs/CAPCO-2016.md:4241`): *"REL TO is not authorized in the banner line if any portion contains EXDIS information. In this case, NOFORN would convey in the banner line."*
- **§H.9 p174 (NODIS)** (`crates/capco/docs/CAPCO-2016.md:4301`): *"REL TO is not authorized in the banner line if any portion contains NODIS information. In this case, NOFORN would convey in the banner line."*

These are the same source-doc passages that justify the just-merged `capco/{exdis,nodis}-implies-noforn` PageRewrites. Both passages are verbatim and re-verifiable against the vendored CAPCO-2016 manual.

Net effect of the gap:
- `expected_rel_to()` returns the intersection of portion REL TO lists even when NODIS/EXDIS is present, which contradicts the page-projected `CAT_REL_TO` value.
- `render_expected_banner` reads `expected_rel_to()` and `expected_non_ic_dissem()` (line 804-823); without the fix it produces a banner that has REL TO **and** NODIS/EXDIS but no NOFORN — the precise condition E039 flags.
- `CapcoScheme::project(Scope::Page, ...)` (via `page_context_to_attrs` at `crates/capco/src/scheme.rs:961-980`) reads both APIs to build the projected `CanonicalAttrs`, so the gap propagates into the *very projection layer* the PageRewrites operate on. The PageRewrites then *do* clear `CAT_REL_TO`, so the final scheme output is right; the gap is the **intermediate** snapshot read by callers that touch `expected_rel_to()` directly. That includes S005/S006 (`crates/capco/src/rules.rs:2311-2314, 2320`) and any test or future rule that consults PageContext as ground truth.

The "enables E039 retirement" hook in the queue tag refers to a follow-on: once `expected_rel_to()` agrees with the page-projected `CAT_REL_TO`, a future banner-roll-up walker row for REL TO becomes the natural detector for the "banner has REL TO + portion has NODIS/EXDIS" mismatch, making E039 redundant. **E039 retirement is NOT in scope for this PR.** E039 stays in place; the gap-close just makes the eventual retirement safe.

## 2. The change (mechanical)

### 2.1 `crates/ism/src/page_context.rs::PageContext::expected_non_ic_dissem`

After the existing SBU-NF / LES-NF classified-context split block (lines 727-738), and **deliberately outside the `if classified` guard** (per the source-doc passages, which do not gate on classification level), add:

```rust
// NODIS / EXDIS imply NOFORN in the banner per CAPCO-2016 §H.9.
// Source passages, verbatim:
//
//   §H.9 p172 (EXDIS) — "REL TO is not authorized in the banner
//   line if any portion contains EXDIS information. In this case,
//   NOFORN would convey in the banner line."
//
//   §H.9 p174 (NODIS) — "REL TO is not authorized in the banner
//   line if any portion contains NODIS information. In this case,
//   NOFORN would convey in the banner line."
//
// NODIS and EXDIS themselves stay in the non-IC dissem set (they
// roll up to the banner per the non-IC banner-roll-up rule); we
// only flag that NF must also be injected into CAT_DISSEM. Unlike
// SBU-NF / LES-NF this is not a split — NODIS/EXDIS tokens are
// NOT removed or renamed. The flag is purely additive for
// downstream consumers (the renderer at `render_expected_banner`,
// the REL TO short-circuit at `expected_rel_to`).
//
// Classification-independent for the NODIS/EXDIS triggers — the
// §H.9 passages above do not gate on classification level. This
// block is intentionally placed AFTER the `if classified` SBU-NF /
// LES-NF split block so it runs in both unclassified and
// classified contexts.
if seen.contains(&NonIcDissem::Nodis) || seen.contains(&NonIcDissem::Exdis) {
    needs_nf = true;
}
```

Plus rename the local `needs_nf_from_split` → `needs_nf` inside the function body (the "split" language no longer accurately describes the trigger surface — NODIS/EXDIS imply NF without splitting), and update the tuple-return doc-comment to say:

```text
/// Returns a tuple `(non_ic_controls, needs_nf)` where `needs_nf` is
/// `true` if NF must be added to dissem controls at banner roll-up.
/// `needs_nf` is set when:
/// - The SBU-NF / LES-NF classified-context split fires (§H.9
///   p178 / p185), OR
/// - Any portion carries NODIS or EXDIS (§H.9 p172 / p174).
///
/// `needs_nf` does NOT depend on classification level for the
/// NODIS/EXDIS triggers — those passages do not gate on
/// classification. The SBU-NF/LES-NF split IS classification-gated
/// (the split only fires in classified context per the existing
/// `if classified` guard inside the function body).
```

That's it for the function body of `expected_non_ic_dissem`. The behavior change in `expected_rel_to()` is automatic: line 405-409 already reads `needs_nf` from `expected_non_ic_dissem()` and bails when true.

### 2.2 `crates/capco/src/rules.rs` — comment updates AND caller-side rename

The S005/S006 "atom-semantics intersection" duplicate-supersession check at lines 2285-2314 reads the same flag. There are **two** caller-side updates here:

1. **Line 2311 (caller binding)** — currently reads:
   ```rust
   let (_expected_non_ic, needs_nf_from_split) = page.expected_non_ic_dissem();
   if needs_nf_from_split {
   ```
   Rename the caller's local binding `needs_nf_from_split` → `needs_nf`. Positional destructuring means this is not a semantic break (the binding name in the caller is independent of the callee's local name), but leaving the stale `needs_nf_from_split` here misleads readers into thinking only the split case bails the rule. Rename for naming consistency with the callee.

2. **Lines 2285-2314 (the supersession comment block)** — currently lists only NOFORN and SBU-NF/LES-NF as the supersession triggers. Update the comment to add NODIS/EXDIS as additional triggers, each citing §H.9 p172 / p174 verbatim per §1.

The test-comment at line 4904-4912 (`s005_does_not_fire_when_non_ic_split_injects_nf`) — its prose says "the non-IC SBU-NF/LES-NF split forces NF injection." After this PR's change, the bail at line 2311-2314 is broader than the prose claims, but the test fixture itself only exercises SBU-NF and is still valid. Two options:

- **Option A**: Refresh the prose to add NODIS/EXDIS as additional bail triggers (keeps one comment, broadens its scope).
- **Option B**: Leave the existing test prose scoped to SBU-NF and add new tests for NODIS/EXDIS with parallel prose (§3.3 tests #7 and #8).

**Decision**: Option B — leave existing tests scoped, add new regression-pin tests for NODIS and EXDIS (test additions don't require changes to existing tests). This keeps the diff smaller and the per-test scope sharper.

### 2.3 `crates/capco/src/scheme.rs:961-980` — no source change, ADD explanatory comment

`page_context_to_attrs` line 976 reads `(non_ic, _needs_nf) = ctx.expected_non_ic_dissem()` and intentionally ignores `_needs_nf`. After this PR, the NOFORN injection into `out.dissem_controls` should also happen here when `needs_nf` is true, so that the page projection routed through `scheme.project(Scope::Page, ...)` matches what `render_expected_banner` produces.

But — that's a **scheme-adoption-style** change touching `crates/capco/`, and the just-merged 8.F PageRewrites *already* inject NOFORN into CAT_DISSEM at the scheme.project layer. So the routes are:

- Pre-PR-3c.B-8F.engine.gap: `page_context_to_attrs` produces `dissem_controls` without NOFORN → PageRewrite `capco/{nodis,exdis,sbu-nf,les-nf}-implies-noforn` injects it → `capco/noforn-clears-rel-to` clears REL TO. Net result: correct page projection.
- Post-PR: `expected_rel_to()` ALSO returns empty (the gap-close). `page_context_to_attrs` STILL produces `dissem_controls` without NOFORN (because we don't touch it here), and the PageRewrites still inject it. Net result: still correct, just with the intermediate snapshot now consistent.

**Conclusion**: `scheme.rs:961-980` does NOT need a source change. The PageRewrites are sufficient for the final-projection layer; this PR only closes the intermediate-snapshot gap that S005/S006 and the banner renderer depend on.

**ADD an explanatory comment** at `scheme.rs:976` so future readers understand why `_needs_nf` is intentionally discarded (rust-reviewer MEDIUM finding):

```rust
// `_needs_nf` (second tuple element) is intentionally discarded
// here. NOFORN injection into `out.dissem_controls` for the
// non-IC dissem trigger family (SBU-NF/LES-NF split, NODIS/EXDIS
// imply-NF) is handled at the final-projection layer by the
// PageRewrites `capco/{sbu-nf,les-nf,nodis,exdis}-implies-noforn`
// (declared in `CapcoScheme::page_rewrites`). Adding a second
// injection path here would duplicate work the PageRewrite
// already does and split the "what does the projected page
// look like?" answer across two code paths. The PageRewrite is
// authoritative for final mutations on CAT_DISSEM; this function
// only assembles the intermediate snapshot from raw portion
// reads, and `out.rel_to` (set on the line above) is consistent
// with the post-rewrite state via the `expected_rel_to`
// short-circuit per §H.9 p172 / p174.
let (non_ic, _needs_nf) = ctx.expected_non_ic_dissem();
```

This is a deliberate scoping decision: editing `page_context_to_attrs` to inject NOFORN directly would be a second mutation path doing the same work the PageRewrite does, which is what Constitution VII §IV's scheme/engine separation tries to avoid. Keep the mutation in the scheme layer (PageRewrites) and the read-API consistency in the engine layer (this PR).

### 2.4 `crates/ism/src/page_context.rs:805` — caller-side rename for naming consistency

`render_expected_banner` at line 805 currently destructures as:
```rust
let (non_ic, needs_nf_from_non_ic) = self.expected_non_ic_dissem();
```

After the callee rename, the canonical name is `needs_nf` (covering both split and NODIS/EXDIS triggers). Rename this caller-side binding `needs_nf_from_non_ic` → `needs_nf` for naming consistency with the callee and with the other caller at `rules.rs:2311` (per §2.2). Positional destructuring means this is not a semantic break; this is naming hygiene only.

The downstream use at line 821 currently reads:
```rust
if needs_nf_from_non_ic && !dissem_parts.iter().any(|p| p == "NOFORN") {
    dissem_parts.push("NOFORN".to_owned());
}
```

Update the variable name to `needs_nf` and refresh the preceding comment at line 820:

```rust
// If the non-IC dissem family implies NF at banner roll-up — the
// SBU-NF/LES-NF split (§H.9 p178 / p185) OR a portion carrying
// NODIS/EXDIS (§H.9 p172 / p174) — inject NOFORN.
if needs_nf && !dissem_parts.iter().any(|p| p == "NOFORN") {
    dissem_parts.push("NOFORN".to_owned());
}
```

(code-reviewer HIGH finding: the existing comment at line 820 says "SBU-NF/LES-NF split injected NOFORN" — which becomes stale after this PR because NODIS/EXDIS now also trigger the injection without a split. The refresh above closes the staleness.)

## 3. The change (tests)

### 3.1 New unit tests in `crates/ism/src/page_context.rs`

Add 4 tests in the existing `#[cfg(test)] mod tests` module:

1. `expected_rel_to_empty_when_nodis_in_portion` — unclassified context: `(U//NODIS)\n(U//REL TO USA, GBR)` → `expected_rel_to()` returns empty. Pin the inverse: without NODIS the intersection would be `[USA, GBR]`.
2. `expected_rel_to_empty_when_exdis_in_portion` — symmetric, classified context: `(S//EXDIS)\n(S//REL TO USA, GBR)` → empty.
3. `expected_non_ic_dissem_signals_needs_nf_on_nodis` — `(C//NODIS)` portion → tuple second element is `true`, first element contains NODIS.
4. `expected_non_ic_dissem_signals_needs_nf_on_exdis` — symmetric for EXDIS.

### 3.2 New integration tests in `crates/ism/tests/rollup_golden.rs`

Two tests verifying `render_expected_banner` injects NOFORN when NODIS/EXDIS portion present:

5. `banner_renders_noforn_when_portion_has_nodis` — portions `(S//NODIS)\n(S//REL TO USA, GBR)` → rendered banner includes `//NOFORN` and excludes `//REL TO ...`. Cite §H.9 p174.
6. `banner_renders_noforn_when_portion_has_exdis` — symmetric for EXDIS. Cite §H.9 p172.

### 3.3 New S005/S006 regression-pin tests in `crates/capco/src/rules.rs`

Two tests at the existing S005/S006 test cluster, parallel in shape to `s005_does_not_fire_when_non_ic_split_injects_nf` and `s005_does_not_fire_when_a_portion_carries_noforn`:

7. `s005_does_not_fire_when_portion_has_nodis` — fixture `(S//NODIS)\n(S//REL TO USA, GBR, RSMA)\n(S//REL TO USA, AUS, GBR)\nSECRET//NODIS//NOFORN`. Pre-PR this would have computed `portions_with_rel_to.len() == 2`, `expected_set = {}` (NODIS supersession now flows through `needs_nf`), and fired a misleading empty-intersection diagnostic. Post-PR the `needs_nf` bail at line 2311-2314 stops it.
8. `s005_does_not_fire_when_portion_has_exdis` — symmetric for EXDIS.

### 3.4 Composition test in a new file `crates/capco/tests/pattern_a_nodis_exdis_page_context_alignment.rs`

The just-merged 8.F PageRewrites + this PR's PageContext alignment must compose. Three tests verifying alignment between `scheme.project(Scope::Page, ...)` and `PageContext::expected_rel_to()`:

9. `nodis_portion_clears_rel_to_via_page_rewrite_AND_page_context_agrees` — assert `scheme.project(Scope::Page, &portions)` produces empty `CAT_REL_TO`, AND `PageContext::expected_rel_to()` on the same portions returns empty. Both routes must agree. Cite §H.9 p174.
10. `exdis_portion_clears_rel_to_via_page_rewrite_AND_page_context_agrees` — symmetric for EXDIS. Cite §H.9 p172.
11. **NEW (rust-reviewer MEDIUM finding)** — `rel_to_intersection_preserved_when_no_nodis_or_exdis_present` — inverse / negative case. Fixture `(S//REL TO USA, GBR)\n(S//REL TO USA, GBR, FRA)` (no NODIS or EXDIS, no NOFORN). Assert `PageContext::expected_rel_to()` returns `[USA, GBR]` (the intersection), AND `scheme.project(Scope::Page, ...)` produces `CAT_REL_TO = {USA, GBR}`. This pins that the new `seen.contains` clause does NOT accidentally fire when neither NODIS nor EXDIS is in the portion set — guards against an over-broad regression where any non-empty `non_ic_dissem` triggers the bail.

(New file rather than appending to `pattern_a_sbu_nf_les_nf_supremacy.rs`: 8.F's NODIS/EXDIS coverage lives in `transmutation_rewrites.rs` and `scheme_equivalence.rs`, not in the SBU-NF/LES-NF file, so a dedicated file is the right scope.)

### 3.5 E039 behavior preservation

E039 (`NodisExdisClearsBannerRelToRule` at `rules.rs:2854`) is **not retired** in this PR but must continue emitting the same diagnostic for "banner has REL TO + portion has NODIS/EXDIS." Its internals read `attrs.rel_to` (literal banner REL TO list) and `page.expected_non_ic_dissem()` first element (the NODIS/EXDIS set) — not `expected_rel_to()`. So E039's logic is unaffected by this PR.

Add a regression-pin test if one doesn't already exist:

12. `e039_still_fires_after_engine_gap_close` — fixture `(S//NODIS)\nSECRET//NODIS//REL TO USA` → E039 fires with the §H.9 p172 + p174 citation. This is the load-bearing "E039 stays in place" pin.

(Survey of `rules.rs:6218-6291` already shows E039 has regression tests for NODIS + EXDIS triggers. Verify the existing tests still pass; add a doc-comment noting the engine-gap close doesn't affect E039's check path.)

## 4. Constitution check

- **VII §IV (scheme-adoption restriction)**: Does NOT apply — this is an **engine-crate** edit, not a scheme-adoption PR. We edit `crates/ism/` (engine-crate, allowed) and `crates/capco/` (rules/test comments only, no rule logic changes). Constitution VII §IV explicitly permits engine-crate edits when the scheme reveals an engine gap; this PR IS that engine-crate edit.
- **V Principle V (G13 audit content-ignorance)**: No `FixProposal` paths affected. PageContext read methods don't produce fixes. No `AppliedFix.proposal.original` impact.
- **VIII (Citation fidelity)**: All new doc-comments and tests cite CAPCO-2016 §H.9 p172 / p174 — the same passages 8.F's PageRewrites cite. Citations are vendored at `crates/capco/docs/CAPCO-2016.md` lines 4241 (p172 EXDIS) / 4301 (p174 NODIS); verified verbatim against the source per §1. No new derivation block required — the §H.9 passages are explicit and stand on their own. Per-passage quoting (separate verbatim quotes for EXDIS and NODIS) MUST be preserved in all downstream doc-comments and test prose — no composite paraphrase that merges them.
- **IV (Two-layer rule architecture)**: No rules added or removed. E039 retained.
- **VI (Dataflow pipeline)**: No pipeline phase reorganized. The change is internal to `PageContext::expected_non_ic_dissem`, a read API consulted by both rules (phase 3) and the scheme (phase 3 via `page_context_to_attrs`).

## 5. Net diff estimate

| Surface | LOC |
|---------|-----|
| `crates/ism/src/page_context.rs` §2.1 (logic + doc + var rename inside function body) | +20 |
| `crates/ism/src/page_context.rs` §2.4 (line 805+821 var rename + comment refresh) | +5 |
| `crates/capco/src/rules.rs` §2.2 (line 2311 caller rename + supersession-comment update) | +10 |
| `crates/capco/src/scheme.rs` §2.3 (explanatory comment on `_needs_nf` discard) | +13 |
| `crates/ism/src/page_context.rs` (new unit tests #1-4) | +60 |
| `crates/ism/tests/rollup_golden.rs` (new tests #5-6) | +50 |
| `crates/capco/src/rules.rs` (new S005/S006 regression tests #7-8) | +60 |
| `crates/capco/tests/pattern_a_nodis_exdis_page_context_alignment.rs` (new file, tests #9-11) | +120 |
| `crates/capco/src/rules.rs` (E039 preservation test #12) | +25 |
| **Total** | **~365 LOC** |

Small surgical change. All in-tree consumers identified; no callers outside `crates/ism/` and `crates/capco/` touch `expected_rel_to` or `expected_non_ic_dissem`.

## 6. Risks & mitigations

| Risk | Mitigation |
|------|-----------|
| Existing tests in `rollup_golden.rs` / `scheme_equivalence.rs` / `tetragraph_consolidation.rs` may now fail if they exercise NODIS/EXDIS + REL TO combinations and asserted the old (gap-present) behavior. | Run full `cargo test -p marque-ism -p marque-capco` after the change; fix any expectation-shift in existing tests by updating to the new (post-gap-close) behavior. Document each updated test with a `// PR 3c.B-8F-engine-gap:` comment naming why the expectation moved. |
| The `needs_nf` flag rename is NOT purely local — `rules.rs:2311` binds it by name (`needs_nf_from_split`) at the destructure, and `rules.rs:4911` test-prose comment references it. (Rust-reviewer HIGH finding.) Positional destructuring means the binding-name change is not a semantic break, but stale names confuse future readers. | Rename in one pass across all three sites: callee local at `page_context.rs::expected_non_ic_dissem`, caller binding at `rules.rs:2311`, caller binding at `page_context.rs:805` (per §2.4). Update the test-prose comment at `rules.rs:4911` to use the new name. Grep verify no stale `needs_nf_from_split` references remain anywhere in tree before opening the PR. |
| S005/S006 tests at `rules.rs:4902` use `needs_nf_from_split` in test prose comments. | Comment refresh per §2.2 (the existing test scoped to SBU-NF stays; new NODIS/EXDIS tests added per §3.3). |
| `render_expected_banner` comment at `page_context.rs:820` says "SBU-NF/LES-NF split injected NOFORN" — becomes stale after this PR. (Code-reviewer HIGH finding.) | Refresh per §2.4 to mention all four trigger families (SBU-NF/LES-NF split + NODIS/EXDIS imply-NF). |
| `_needs_nf` discard at `scheme.rs:976` is unexplained; future reader will wonder why NOFORN injection isn't routed here. (Rust-reviewer MEDIUM finding.) | Add the explanatory comment per §2.3. |
| Future PR retiring E039 needs a BannerMatchesProjectedRule REL TO row (the natural detector). Not in scope here. | Out of scope. The task tag says "enables E039 retirement"; this PR satisfies the precondition. |

## 7. Out-of-scope (explicit)

1. E039 retirement (follow-on PR; needs a BannerMatchesProjectedRule REL TO row first).
2. Editing `crates/capco/src/scheme.rs::page_context_to_attrs` to inject NOFORN directly. PageRewrites already handle this at the final projection layer. Adding a second mutation path would duplicate work and violate the "mutation in scheme layer, read consistency in engine layer" division.
3. Refactoring `expected_rel_to` to consult `scheme.project(Scope::Page, ...)` instead of walking portions directly. That's the cleaner architectural fix but a larger refactor (PageContext would need scheme access). Deferred.
4. Cross-page semantics. NODIS/EXDIS interactions still scoped per-page (`PageContext` resets at `MarkingType::PageBreak` per Constitution VI).

## 8. Pre-flight reviewer dispatch (current step)

After this spec is written, dispatch two reviewers in parallel:

- `ecc:rust-reviewer` — verify Rust idioms, ownership, error-handling correctness; check the rename doesn't break any clippy lints; verify no unsafe paths.
- `ecc:code-reviewer` — verify Constitution compliance (V Principle V audit content-ignorance, VII §IV scope, VIII citations); verify spec scope is minimal; verify tests cover the critical regression channels (S005/S006, rollup_golden, scheme equivalence, E039 preservation).

Apply all CRITICAL and HIGH findings before implementation. MEDIUM findings get folded into commit notes; LOW deferred unless trivial.

## 9. Post-impl reviewer dispatch (after implementation lands)

Same two reviewers in parallel on the landed diff. Address all CRITICAL/HIGH before PR open. Open PR with full review trail in the description.
