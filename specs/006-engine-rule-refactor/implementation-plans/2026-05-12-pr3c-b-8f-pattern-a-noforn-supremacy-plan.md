# PR 3c.B Sub-PR 8.F — Pattern A NOFORN-supremacy (NODIS / EXDIS) design spec

**Branch:** `refactor-006-pr-3c-b-8f-noforn-supremacy-fouo-eviction`
**Author:** architect agent (preflight)
**Reviewers expected:** rust-reviewer, code-reviewer
**Engine prereq:** PR #392 — `CategoryAction::Intent(ReplacementIntent<S>)` (merged to staging at commit `c4a0adad`)
**Master pattern doc:** `/home/knitli/.claude/projects/-home-knitli-marque/memory/project_noforn_supremacy_composition.md`

---

## 1. Scope decision

**Confirm narrowing to NODIS + EXDIS only.** SBU-NF / LES-NF land in a follow-on 8.F.2.

**Justification.** Three independent reasons:

1. **Infrastructure parity.** `TOK_NODIS` (`crates/capco/src/scheme.rs:92`) and `TOK_EXDIS` (`:93`) exist; `capco_token_category()` (`:392`) routes both to `CAT_NON_IC_DISSEM`; `apply_fact_remove`'s `CAT_NON_IC_DISSEM` arm (`:728-746`) handles both. No vocabulary additions or routing-table edits are needed. SBU-NF / LES-NF require new `TOK_*` constants, new `capco_token_category` arms, new `NonIcDissem` arms in `apply_fact_remove` / `apply_fact_add`, and a `capco_category_contains` arm for `CAT_NON_IC_DISSEM` (currently uncovered — `capco_category_contains` at `crates/capco/src/scheme.rs:310-319` only handles `(CAT_DISSEM, TOK_NOFORN)`).

2. **Coordination with the SBU-NF/LES-NF split logic.** `PageContext::expected_non_ic_dissem()` at `crates/ism/src/page_context.rs:716-741` already runs a hand-rolled classified-context split (`SBU-NF → SBU + NF`, `LES-NF → LES + NF`). Touching SBU-NF / LES-NF in 8.F means deciding whether the new rewrite supersedes the split or composes with it; that decision belongs in the Pattern-C / classified-strips sub-PR where the SBU half is the focus, not here. The split is not buggy — it's load-bearing for `expected_dissem_controls` Step 4 NF injection (`page_context.rs:368-372`) — so "fix it in 8.F" is the wrong framing.

3. **Sub-PR cadence.** PR 3b.A–F established a 1-citation-family-per-sub-PR cadence; per-row §-citations stay traceable. NODIS (`§H.9 p174`) and EXDIS (`§H.9 p172`) are a single citation family ("DoS distribution-control NOFORN-requires") with shared §H.9 context lines. SBU-NF (`§H.9 p178`) and LES-NF (`§H.9 p185`) are separate families.

---

## 2. Citation verification table

Both citations verified against `crates/capco/docs/CAPCO-2016.md` (vendored CAPCO-2016 manual, Constitution VIII primary source for ISM/CAPCO).

| Rewrite | Citation | Verbatim source quote | Source line |
|---|---|---|---|
| `capco/nodis-implies-noforn` | `CAPCO-2016 §H.9 p174` | `"- May be used with TOP SECRET, SECRET, CONFIDENTIAL, or UNCLASSIFIED. - NODIS and EXDIS markings cannot be used together. - Requires NOFORN."` (under "(U) Relationship(s) to Other Markings:" on the NODIS entry) | `crates/capco/docs/CAPCO-2016.md:4293-4296` |
| `capco/exdis-implies-noforn` | `CAPCO-2016 §H.9 p172` | `"- May be used with TOP SECRET, SECRET, CONFIDENTIAL, or UNCLASSIFIED. - EXDIS and NODIS markings cannot be used together. - Requires NOFORN."` (under "(U) Relationship(s) to Other Markings:" on the EXDIS entry) | `crates/capco/docs/CAPCO-2016.md:4233-4236` |

Page-pin verification (`begin page N` / `end page N` markers):

- EXDIS entry sits between `begin page 172` at `crates/capco/docs/CAPCO-2016.md:4209` and `end page 172` at `:4249`. The "Requires NOFORN." line is at `:4236`. ✓ p172.
- NODIS entry sits between `begin page 174` at `crates/capco/docs/CAPCO-2016.md:4269` and `end page 174` at `:4310`. The "Requires NOFORN." line is at `:4296`. ✓ p174.

A prior sub-PR's architect mis-cited NODIS as p172 (which is actually EXDIS). Both citations independently re-verified — NODIS is p174, EXDIS is p172.

Background corroboration (not cited as primary because already covered by the "Requires NOFORN." passages):
- EXDIS §H.9 p172, line 4241: *"REL TO is not authorized in the banner line if any portion contains EXDIS information. In this case, NOFORN would convey in the banner line."*
- NODIS §H.9 p174, line 4301: *"REL TO is not authorized in the banner line if any portion contains NODIS information. In this case, NOFORN would convey in the banner line."*
- EXDIS §H.9 p172, line 4260: *"Until originator approval is obtained, mark EXDIS portions as NOFORN when an FD&R marking is required..."*

These passages confirm the framing ("NOFORN conveys the foreign-release decision in this case") but the operative authority for the rewrite is the standalone `Requires NOFORN.` line on each entry's "Relationship(s) to Other Markings" stanza.

---

## 3. Trigger predicate and action

Both rewrites are declarative (`PageRewrite::declarative` constructor at `crates/scheme/src/page_rewrite.rs:101-117`).

### `capco/nodis-implies-noforn`

```rust
PageRewrite::declarative(
    "capco/nodis-implies-noforn",
    "CAPCO-2016 §H.9 p174",
    CategoryPredicate::Contains {
        category: CAT_NON_IC_DISSEM,
        token: TOK_NODIS,
    },
    CategoryAction::Intent(ReplacementIntent::FactAdd {
        token: FactRef::Cve(TOK_NOFORN),
        scope: Scope::Page,
    }),
    NODIS_IMPLIES_NF_READS,  // &[CAT_NON_IC_DISSEM]
    NODIS_IMPLIES_NF_WRITES, // &[CAT_DISSEM]
)
```

### `capco/exdis-implies-noforn`

```rust
PageRewrite::declarative(
    "capco/exdis-implies-noforn",
    "CAPCO-2016 §H.9 p172",
    CategoryPredicate::Contains {
        category: CAT_NON_IC_DISSEM,
        token: TOK_EXDIS,
    },
    CategoryAction::Intent(ReplacementIntent::FactAdd {
        token: FactRef::Cve(TOK_NOFORN),
        scope: Scope::Page,
    }),
    EXDIS_IMPLIES_NF_READS,  // &[CAT_NON_IC_DISSEM]
    EXDIS_IMPLIES_NF_WRITES, // &[CAT_DISSEM]
)
```

**Construction form (reviewer-corrected):** `ReplacementIntent` has no `fact_add` constructor method; only `fact_remove` exists at `crates/scheme/src/fix_intent.rs:244-261`. The `FactAdd` variant must be built with struct-literal syntax. The variant's `#[non_exhaustive]` attribute does not block struct-literal construction from a different crate (only exhaustive pattern matching is constrained); `marque-capco` can construct `FactAdd` directly.

### Predicate-evaluator support (LOAD-BEARING gap — see Q2)

The `CategoryPredicate::Contains { category: CAT_NON_IC_DISSEM, token: TOK_NODIS | TOK_EXDIS }` arm needs to be reachable from `capco_category_contains` at `crates/capco/src/scheme.rs:310-319`. Today that function **only** handles `(CAT_DISSEM, TOK_NOFORN)` and falls through to `false` for everything else (conservative-disable per the doc comment). Without extension, the new rewrites' triggers will never fire under the conservative default.

**Required change in `capco_category_contains`:** extend with the two-arm `(CAT_NON_IC_DISSEM, TOK_NODIS | TOK_EXDIS)` case so the predicate actually resolves.

**Implementation note (reviewer-clarified):** the predicate dispatch takes `(category: CategoryId, token: TokenId)`, but the function body scans the parsed `CapcoMarking` (specifically `attrs.non_ic_dissem: Box<[NonIcDissem]>`) — these are two different lookup forms. The match-arm uses `TokenId` constants (`TOK_NODIS` / `TOK_EXDIS`) for dispatch, and the body uses the corresponding `NonIcDissem` enum variants for the scan. Pattern parallels the existing `(CAT_DISSEM, TOK_NOFORN)` arm at lines 312-317 (which dispatches on `TOK_NOFORN` and scans `dissem_controls` for `DissemControl::Nf`):

```rust
fn capco_category_contains(m: &CapcoMarking, category: CategoryId, token: TokenId) -> bool {
    let attrs = &m.0;
    if category == CAT_DISSEM && token == TOK_NOFORN {
        return attrs.dissem_controls.iter().any(|d| matches!(d, DissemControl::Nf));
    }
    if category == CAT_NON_IC_DISSEM {
        if token == TOK_NODIS {
            return attrs.non_ic_dissem.iter().any(|d| matches!(d, NonIcDissem::Nodis));
        }
        if token == TOK_EXDIS {
            return attrs.non_ic_dissem.iter().any(|d| matches!(d, NonIcDissem::Exdis));
        }
    }
    false
}
```

This is a single 8-line surgical extension to `marque-capco` — not a `marque-scheme` edit — and stays within the Constitution VII scheme-adoption restriction.

---

## 4. Scheduler ordering verification

### Dataflow declarations

| Rewrite | `reads` | `writes` |
|---|---|---|
| `capco/nodis-implies-noforn` | `[CAT_NON_IC_DISSEM]` | `[CAT_DISSEM]` |
| `capco/exdis-implies-noforn` | `[CAT_NON_IC_DISSEM]` | `[CAT_DISSEM]` |
| `capco/noforn-clears-rel-to` (existing) | `[CAT_DISSEM, CAT_REL_TO]` | `[CAT_REL_TO]` |
| `capco/sbu-nf-transmutes-on-classified-contact` (entry 6a) | `[CAT_CLASSIFICATION]` | `[CAT_DISSEM]` |
| `capco/les-nf-transmutes-on-classified-contact` (entry 6b) | `[CAT_CLASSIFICATION]` | `[CAT_DISSEM]` |
| `capco/orcon-nato-to-us-orcon-on-us-contact` (entry 5) | `[CAT_CLASSIFICATION]` | `[CAT_DISSEM]` |

### Kahn topological ordering

`schedule_rewrites` (Kahn) at `crates/engine/src/scheduler.rs` orders writers-of-X before readers-of-X.

- The new rewrites **write** `CAT_DISSEM`. `noforn-clears-rel-to` **reads** `CAT_DISSEM`. ⟹ Scheduler orders the new rewrites BEFORE `noforn-clears-rel-to`. ✓
- The new rewrites **read** `CAT_NON_IC_DISSEM`. No existing rewrite writes `CAT_NON_IC_DISSEM`. ⟹ The new rewrites have no upstream `CAT_NON_IC_DISSEM` writers; they can run in declaration order relative to each other. ✓
- All four `CAT_DISSEM`-writers (nodis-implies-noforn, exdis-implies-noforn, 5, 6a, 6b) are siblings in the DAG. ✓

### Cycle / unannotated-axis check

- **No new cycles.** No existing rewrite writes `CAT_NON_IC_DISSEM`. New edge connects to nothing upstream.
- **No `Custom` actions.** Both new rewrites use `Contains` + `Intent` (both declarative). `UnannotatedCustomAxes` N/A.
- **`Intent` payloads validated.** `FactRef::Cve(TOK_NOFORN)` routes via `capco_token_category(TOK_NOFORN) = Some(CAT_DISSEM)` (`scheme.rs:388`). `validate_intent_rewrites` at `crates/engine/src/scheduler.rs:50-71` passes.
- `Engine::new` catches all of the above at construction time.

---

## 5. E039 retirement — CRITICAL ARCHITECTURAL FINDING (defer)

**The user's brief mis-asserts E039 retirement consequences.** It states: "After 8.F, [E039] is impossible because: NODIS/EXDIS portion → Pattern A adds NOFORN to dissem → `noforn-clears-rel-to` fires → REL TO is gone from banner before E039 could see it."

This is **wrong** because the engine's runtime banner-validation rules (E039, E040, E031, E035 via `BannerMatchesProjectedRule`) do NOT consult `CapcoScheme::project()` output. They consult `marque_ism::PageContext::expected_*()` accessors directly — see `crates/engine/src/engine.rs:716` (`page_context.add_portion(attrs.clone())`) and the E039 body at `crates/capco/src/rules.rs:2880-2887` reading `page.expected_non_ic_dissem()`.

`expected_rel_to()` at `crates/ism/src/page_context.rs:394-409` short-circuits to empty if any portion's `dissem_controls` contains `Nf` OR if `expected_non_ic_dissem` reports `needs_nf` (the SBU-NF/LES-NF split case). It does **NOT** check NODIS or EXDIS in `non_ic_dissem`. So today, a portion with `(S//NF//ND)` and a banner with `REL TO USA, GBR` produces a non-empty `expected_rel_to` — and E039 fires on the banner-side rule.

**Consequence:** the two new rewrites mutate the scheme-projection output, but the runtime engine's PageContext-driven banner validation is unchanged. **E039 will still fire on the existing test cases** if it stays registered.

### Two options

**Option R1 (retire E039, fix the gap upstream).** Extend `PageContext::expected_rel_to()` to short-circuit when `non_ic_dissem` contains `Nodis` or `Exdis`. That fix mirrors the existing `any_noforn` short-circuit + the `needs_nf` short-circuit. E039 then becomes structurally impossible: the banner's projected REL TO is empty when any portion has NODIS/EXDIS.

**Problem with R1:** `crates/ism/` is an engine crate (foundational vocabulary). Constitution VII restricts scheme-adoption PRs from editing engine crates. R1 would require splitting 8.F into an engine-prereq edit + scheme-adoption edit.

**Option R2 (keep E039, defer retirement).** Land the two new rewrites in 8.F. They make the scheme projection correct (so a downstream consumer projecting via `scheme.project(Scope::Page, ...)` gets the right banner). E039 stays in the registered ruleset to handle the runtime PageContext-driven path. E039 retires later — when the engine switches its banner validation to drive through `scheme.project` (Phase D / Phase E), or when `PageContext::expected_rel_to` gets the NODIS/EXDIS short-circuit.

### Recommendation: **Option R2.**

**Reasoning:** Constitution VII §IV is unambiguous — "A scheme-adoption PR MUST NOT edit the engine crates (`marque-engine`, `marque-scheme`, `marque-core`, `marque-rules`, `marque-ism`)." R1's `expected_rel_to` patch lives in `marque-ism`. R2 is a clean scheme-adoption-only change.

The user's master pattern doc (`project_noforn_supremacy_composition.md`, line 91) says "Retires E039 entirely." That commitment is correct in the limit, but premature for **this** sub-PR's restricted scope. The brief itself acknowledges this exact split: "If [scheme] reveals an engine gap, the gap is fixed first in a separate PR... then the scheme lands."

### What 8.F changes for E039

**Nothing.** E039 stays registered, stays tested, stays in `EXPECTED_RULE_IDS` at `crates/capco/tests/post_3b_registration_pin.rs:53`. The new rewrites land alongside it. E039 retirement is a follow-on PR (8.F-engine-gap or part of Phase D/E).

### Runtime execution gap (reviewer-flagged, load-bearing context)

The engine's lint-time hot path does **not** currently iterate `scheduled_rewrites` or call `scheme.project()`. `Engine::new` builds and validates the scheduled-rewrite list (via `validate_intent_rewrites` and `schedule_rewrites`), but the lint inner loop drives banner validation through `marque_ism::PageContext` directly. The existing `capco/noforn-clears-rel-to` rewrite — though structurally complete with a real `Contains` trigger and a real `Clear` action — is **also** not executed by `Engine::lint` today. It only takes effect when an external caller (test code, future Phase D/E execution loop) calls `scheme.project(Scope::Page, ...)`.

This means the two new rewrites in 8.F are **scheduler-validated but execution-deferred**:

- ✓ `Engine::new` validates the rewrites' intent payloads + scheduler ordering (catches authoring bugs at construction time)
- ✓ `scheme.project(Scope::Page, ...)` reflects the rewrites when called directly by test code or downstream consumers
- ⚠ `Engine::lint` / `Engine::fix` output is unchanged by 8.F's additions

This is consistent with the broader Phase D/E phasing: 8.F lays the declarative groundwork so that, when the engine's banner-validation path switches to `scheme.project`-driven semantics, the rewrites will already be in place and will start having engine-level effect. At that point, E039 retires (no rule needed because the projected banner won't contain REL TO when NODIS/EXDIS is present in any portion).

**Test plan implication:** all behavioral assertions for the new rewrites must be written against `scheme.project(...)`, not against `Engine::lint`/`Engine::fix`. See §8 below.

---

## 6. E038 stays

E038 (`DeclarativeDosDissemNofornRule` at `crates/capco/src/rules_declarative.rs:1382-1466`) emits a portion-scope diagnostic with `FactAdd { TOK_NOFORN, Scope::Portion | Scope::Page }` fix proposal. The new page-scope rewrite is complementary:

- E038 fires when a portion (`(S//ND)` without NOFORN) lacks the required NOFORN → user-visible diagnostic + fix proposal.
- The new `nodis-implies-noforn` rewrite ensures the **projected page-level banner state** has NOFORN in `CAT_DISSEM` regardless of whether the user accepts E038's fix on the portion.

These are different surfaces, both required, both correct. **E038 stays unchanged in 8.F.**

---

## 7. W003 unchanged

W003 retirement is gated on Pattern C (`classified-strips-fouo` / `classified-strips-sbu` / `classified-strips-limdis`). Pattern A (8.F) does not interact with W003. **W003 is NOT touched in 8.F.**

---

## 8. Test plan

All under `crates/capco/tests/` (Constitution VII §IV — engine-crate test files off-limits; `crates/engine/tests/scheduler.rs` does not need edits since `validate_intent_rewrites` is generic).

### New test file: `crates/capco/tests/pattern_a_noforn_supremacy.rs`

All tests drive `scheme.project(Scope::Page, &[portion_attrs])` directly (NOT `Engine::lint`) — see Runtime execution gap above.

1. **`nodis_portion_projects_noforn_to_page_dissem`** — input portion `(S//ND)`; assert `scheme.project(Scope::Page, &[portion]).0.dissem_controls.contains(&DissemControl::Nf)`.
2. **`exdis_portion_projects_noforn_to_page_dissem`** — mirror for EXDIS.
3. **`nodis_portion_composes_with_noforn_clears_rel_to`** — input portion `(S//ND)` paired with synthetic prior REL TO; assert projected page has `dissem_controls` containing NOFORN AND `rel_to` empty. Load-bearing composition test — proves scheduler runs the new rewrite BEFORE `noforn-clears-rel-to`.
4. **`exdis_portion_composes_with_noforn_clears_rel_to`** — mirror for EXDIS.
5. **`portion_without_nodis_or_exdis_does_not_inject_noforn`** — negative test; `(S)` portion; NOFORN NOT injected. Catches an over-eager predicate.
6. **`nodis_portion_with_noforn_already_present_is_idempotent`** — `(S//NF//ND)` portion; exactly one NOFORN, no panic. Exercises `apply_fact_add → IntentInapplicable` silent-no-op path at `crates/capco/src/scheme.rs:624-639`.
7. **`unclassified_nodis_portion_still_injects_noforn`** — input `(U//ND)`; §H.9 p174 entry's Relationship(s) line says NODIS "May be used with TOP SECRET, SECRET, CONFIDENTIAL, or UNCLASSIFIED" — the rewrite must fire regardless of classification level. Mirror test for `(U//XD)`.
8. **`portion_with_both_nodis_and_exdis_is_safe`** — input `(S//ND//XD)` (semantically forbidden by E037 mutex, but the rewrite path must remain safe under accidental concurrent firing). Two `*-implies-noforn` rewrites trigger; both attempt `FactAdd(NOFORN)`; second hits `apply_fact_add → IntentInapplicable` silent no-op. Result: exactly one NOFORN in projected dissem, no panic. (Master pattern doc, `project_noforn_supremacy_composition.md:17`: "NODIS and EXDIS markings cannot be used together.")
9. **`pattern_a_rewrites_emit_no_applied_fix`** (reviewer-revised G13 test) — drive `Engine::fix` on a portion `(S//ND)` corpus; assert that no `AppliedFix` records in the audit stream carry rule-IDs from the new rewrites (rewrites are projection mutations, not engine-promoted fixes; current execution-deferred posture means they cannot emit AppliedFix). When the Phase D/E execution loop lands and rewrites DO materialize as `AppliedFix`, this test flips to assert `applied.proposal.original == ""` (G13 content-ignorance) — captured as a TODO comment in the test body referencing this PR.

### Updated tests — corpus_parity.rs (THREE pin sites, reviewer-flagged)

**`crates/capco/tests/corpus_parity.rs`** has three independent assertions that all pin the page-rewrite count:

- **Site 1 (line ~200):** `phase_3_declares_*_page_rewrites_with_citations` — `assert_eq!(rewrites.len(), 9, ...)` with an exhaustive explanation string naming all nine rewrites. Bump count `9 → 11`; extend message to enumerate the two new entries.
- **Site 2 (line ~240):** `phase_3_scheduler_exposes_*_scheduled_rewrites` — `assert_eq!(scheduled.len(), 9)`. Bump `9 → 11`.
- **Site 3 (lines ~243-256):** sorted `names: [&str; 9]` array enumerating all nine rewrite IDs by name. Extend array to length 11, insert `"capco/exdis-implies-noforn"` and `"capco/nodis-implies-noforn"` in sorted position. **Verify exact line range at implementation time — these are estimates from the reviewer; grep `corpus_parity.rs` for `assert_eq!(rewrites.len(), 9` and `assert_eq!(scheduled.len(), 9` and the sorted names array.**

Failing to update all three sites results in test failures at PR open.

### Unchanged but verified

- `crates/capco/tests/corpus_parity.rs` rule count pin — unchanged. E039 stays registered.
- `crates/capco/tests/post_3b_registration_pin.rs:53` `EXPECTED_RULE_IDS` list — unchanged.
- `crates/capco/src/rules.rs:6218-6283` existing E039 unit tests — unchanged.
- `crates/capco/tests/category_action_intent.rs` — unchanged.
- `crates/engine/tests/scheduler.rs` — unchanged.

---

## 9. File-by-file change inventory

```
crates/capco/src/scheme.rs                          +214 lines
  - capco_category_contains  (2-arm extension for CAT_NON_IC_DISSEM + NODIS/EXDIS)
  - build_page_rewrites      (2 PageRewrite::declarative entries + per-entry doc-comments + axis-slice consts)

crates/capco/tests/pattern_a_noforn_supremacy.rs    NEW, +525 lines, 9 tests
crates/capco/tests/corpus_parity.rs                 +/- 17 lines
  - 2 rewrite-count pins bumped 9 → 11
  - phase_3_noforn_clearer_runs_after_dissem_transmutations extended with 2 new DISSEM writers (PR-393 review fixup)

# Additional pin sites found at implementation time (not in original spec)
crates/capco/tests/scheme_equivalence.rs            +/- 49 lines (count + ID arrays + index-shifted citations)
crates/capco/tests/transmutation_rewrites.rs        +/- 18 lines (engine-construction count)

# Unchanged but verified
crates/capco/src/rules.rs                       (unchanged — E039 stays)
crates/capco/src/rules_declarative.rs           (unchanged — E038 stays)
crates/capco/tests/post_3b_registration_pin.rs  (unchanged — rule count stays)
crates/capco/tests/category_action_intent.rs    (unchanged)
crates/scheme/                                  (unchanged — engine-prereq #392 was the only scheme edit needed)
crates/engine/                                  (unchanged — validate_intent_rewrites + scheduler generic)
crates/ism/                                     (unchanged — Constitution VII)
```

Actual diff: **+214 lines product code**, **+525 lines test code** in the new file, **+84 lines** updates across 3 existing pin-site files. No deletions. Net registered rule count unchanged. (Test count grew from spec's 7 to final 9: tests #7 + #8 + #9 were added during reviewer correction to cover unclassified-NODIS, combined-NODIS+EXDIS, and the G13 execution-deferred posture respectively.)

---

## 10. Risks and open questions

### CRITICAL

**Q1. E039 retirement consequences.** See §5. Recommendation: defer E039 retirement; land rewrites only.

### HIGH

**Q2. `capco_category_contains` silent-disabling root-cause.** The current implementation falls through to `false` for any `(category, token)` pair other than `(CAT_DISSEM, TOK_NOFORN)`. Without the 2-arm extension for `(CAT_NON_IC_DISSEM, TOK_NODIS | TOK_EXDIS)`, the new rewrites' triggers will silently never fire — 8.F would be a no-op masquerading as a fix. The extension is a 5-line change in `marque-capco`.

**Q3. `ReplacementIntent::fact_add` arity.** The user's brief shows `fact_add(FactRef, Scope, ProvenanceLabel)` (three args). `category_action_intent.rs` test fixtures at `:82-85` use the struct-literal form `ReplacementIntent::FactAdd { token, scope }`. Verify the current constructor signature before implementation; adjust the call site to match.

### MEDIUM

**Q4. Scope choice for the FactAdd intent.** `Scope::Page` is correct for a page-rewrite. ✓

**Q5. Symmetry with the SCI Pattern A follow-on.** Future PR adds 5 more `*-implies-noforn` rewrites reading `CAT_SCI` / writing `CAT_DISSEM`. Worth noting the pattern is intentionally extensible in the 8.F doc-comment.

### LOW

**Q6. Diagnostic emission from rewrites?** Silent mutations — no `Diagnostic` for the implication itself. User-facing diagnostic is E038.

**Q7. Performance.** Two new rewrites cost ~O(portions) per `scheme.project` call (typical 0–1 entries). No SC-001 impact.

---

## What reviewers must verify

1. **Constitution VIII citations** — re-grep `crates/capco/docs/CAPCO-2016.md:4233-4236` and `:4293-4296`. Confirm both say "Requires NOFORN." verbatim. Confirm page-pin (4209/4249 brackets p172, 4269/4310 brackets p174).
2. **Q1 architectural diagnosis** — confirm or refute that runtime banner validation uses `PageContext`, not `scheme.project`. If wrong, E039 can retire in 8.F.
3. **Q2 silent-disabling root-cause** — confirm `capco_category_contains` falls through to `false` for un-handled arms.
4. **Q3 `fact_add` arity** — confirm constructor signature against post-PR #392 `marque-scheme` source.
5. **Scheduler invariant** — confirm Kahn ordering produces `{nodis,exdis,5,6a,6b} → noforn-clears-rel-to` partial order.
