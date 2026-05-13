# PR 3c.B Sub-PR 8.F.2 — Pattern A NOFORN-supremacy for SBU-NF and LES-NF — design spec

- **Target branch:** `refactor-006-pr-3c-b-8f2-sbu-nf-les-nf-pattern-a`
- **Author:** architect agent (preflight)
- **Reviewers expected:** rust-reviewer, code-reviewer
- **Prior art:** PR #393 (Sub-PR 8.F, merged `6e867ef6`) — NODIS + EXDIS Pattern A
- **Pattern A framing (inlined for reviewer reproducibility):** "X implies NOFORN" — a family of CAPCO §H.9 entries where the presence of token X on the page mandates NOFORN on the projected dissem axis. The non-IC dissem subfamily covers NODIS (§H.9 p174), EXDIS (§H.9 p172), SBU-NF (§H.9 p178), and LES-NF (§H.9 p185). The SCI subfamily (HCS-O §H.4 p64, HCS-P-sub §H.4 p68, TK-IDIT §H.4 p87, TK-BLFH §H.4 p91, TK-KAND §H.4 p95) is deferred to a follow-on sub-PR. 8.F (PR #393) landed NODIS + EXDIS; 8.F.2 (this PR) lands SBU-NF + LES-NF. All four entries share identical structural shape — `Contains(CAT_NON_IC_DISSEM, TOK_*)` trigger, `Intent(FactAdd { Cve(TOK_NOFORN), Scope::Page })` action, `[CAT_NON_IC_DISSEM]` reads, `[CAT_DISSEM]` writes — designed for a Stage-4 reframe collapse into one declarative-table walker.
- **Reviewer attestation:** rust-reviewer and code-reviewer both `APPROVE WITH FIXES` (see §11 for the consolidated patch set folded into this spec).

---

## §1. Scope decision

**Scope option chosen: 1c — Pattern A is conceptually orthogonal to the transmutation rewrites; both fire; they are additive, not redundant.** Add **two** `PageRewrite` entries — `capco/sbu-nf-implies-noforn` (§H.9 p178) and `capco/les-nf-implies-noforn` (§H.9 p185) — declared after the existing 8.F entries `capco/nodis-implies-noforn` and `capco/exdis-implies-noforn`.

**Investigation outcome by option:**

- **(a) Pattern A fires before transmutation in scheduler order:** **Confirmed possible and required.** The transmutation rewrites (6a / 6b) write `CAT_DISSEM` but **do not yet have working trigger/action implementations** — they are Phase-3 stubs (`PageRewrite::custom` with `never_fires` predicate and `noop_action` action; verified at `crates/capco/src/scheme.rs:1412-1413` for 6a and `:1653-1654` for 6b). They are scheduler-validated placeholders, not active rewrites. Pattern A's `Contains(CAT_NON_IC_DISSEM, TOK_*)` triggers will resolve through `capco_category_contains` once extended (§3); 6a/6b's stub predicate never fires regardless of order.

- **(b) "Class > U drops SBU-NF entirely" loses NOFORN:** **Confirmed real, but not a Pattern A injection concern.** §H.9 p178 says (CAPCO-2016.md:4421): "If the portion is classified, the classification level of the portion adequately protects the SBU information, so SBU is not reflected in the portion mark; **however a NOFORN marking must be added** to the portion mark, e.g., (C//NF)." This is a portion-mark canonicalization concern (drop SBU but inject NF), which is what the eventual 6a transmutation implementation will encode. Pattern A's page-level FactAdd(NOFORN) is **additive** — it does not "lose" anything; it ensures NOFORN is present on the page dissem axis even before the portion-mark transmutation runs.

- **(c) Pattern A is orthogonal to transmutation:** **Confirmed — this is the load-bearing framing.** Transmutation = portion-mark rewrite + banner consolidation (e.g., `SECRET//NOFORN//LES` for LES-NF). Pattern A = page-level dissem-axis invariant (NOFORN must be in projected page dissem when any portion carries SBU-NF or LES-NF, regardless of classification). The two cover different surfaces and compose monotonically.

- **(d) Pattern A is fully redundant:** **Refuted.** The existing `PageContext::expected_dissem_controls` injection does inject NF, but **only via `expected_non_ic_dissem`'s `needs_nf_from_split` flag**, which **only fires in classified docs** (`crates/ism/src/page_context.rs:726` — `if classified { ... }` gate verified directly). For `(U//SBU-NF)` and `(U//LES-NF)` — the canonical unclassified portion forms — `PageContext` does NOT inject NF on the dissem axis. §H.9 p178/p185 both require NOFORN; Pattern A closes this unclassified-stratum gap at the scheme-projection layer.

**Net deliverable:** two new PageRewrite entries; 11 → 13 rewrites total.

---

## §2. Citation verification table

Both citations verified verbatim against `crates/capco/docs/CAPCO-2016.md` (vendored CAPCO-2016 manual, Constitution VIII primary source).

| Rewrite | Citation | Verbatim source quote | Source line |
|---|---|---|---|
| `capco/sbu-nf-implies-noforn` | `CAPCO-2016 §H.9 p178` | Authority bundle (two passages anchor "NOFORN is the operative dissem consequence of SBU-NF"): (a) banner-form heading at the top of the §H.9 p178 entry — *"SENSITIVE BUT UNCLASSIFIED NOFORN … Authorized Banner Line Marking Title: SENSITIVE BUT UNCLASSIFIED NOFORN … Authorized Banner Line Abbreviation: SBU NOFORN … Authorized Portion Mark: SBU-NF"* — establishing NOFORN as a structural component of the marking. (b) Commingling rule: *"The SBU-NF marking is conveyed in the portion mark only if the commingled portion is unclassified and there is no other NOFORN information included in the portion. If there is other NOFORN information in the commingled portion, the 'SBU' marking is used and a NOFORN marking is added, e.g., (U//NF//SBU)."* | `CAPCO-2016.md:4388-4398`, `:4420` |
| `capco/les-nf-implies-noforn` | `CAPCO-2016 §H.9 p185` | Authority bundle: (a) banner-form heading at the top of the §H.9 p185 entry — *"LAW ENFORCEMENT SENSITIVE NOFORN … Authorized Banner Line Marking Title: LAW ENFORCEMENT SENSITIVE NOFORN … Authorized Banner Line Abbreviation: LES NOFORN … Authorized Portion Mark: LES-NF"*. (b) Precedence Rules for Banner Line Guidance, classified-banner case: *"When a classified document contains portions of U//LES-NF, the 'LES' marking is used in the banner line and the NOFORN marking is applied as a Dissemination Control Marking. For example: SECRET//NOFORN//LES."* | `CAPCO-2016.md:4532-4542`, `:4558` |

**Page-pin verification:**

- SBU-NF entry brackets: `begin page 178` at `CAPCO-2016.md:4386`, `end page 178` at `:4425`. Authority quotes at `:4388-4398` and `:4420` are inside the bracket. ✓
- LES-NF entry brackets: `begin page 185` at `:4530`, `end page 185` at `:4562`. Authority quotes at `:4532-4542` and `:4558` are inside the bracket. ✓

**Citation-strength note (D13 / Constitution VIII).** Unlike NODIS/EXDIS (§H.9 p172/p174), where a single "Requires NOFORN." line in the Relationship(s) stanza is the operative authority, SBU-NF (§H.9 p178) and LES-NF (§H.9 p185) do **not** have a "Requires NOFORN." sentence. The "NOFORN is required" semantic flows from the marking's structural identity (the banner-form heading literally names it "SBU NOFORN" / "LES NOFORN", and the portion-mark suffix `-NF` is the NOFORN half). The Commingling Rule (SBU-NF) and the Precedence Rule (LES-NF) reinforce that even after transmutation strips the source token, NOFORN must remain. Citing both the banner-form heading and the reinforcing rule is intentional; a reviewer should be able to trace the authority to the entry's identity and to the textual confirmation that NOFORN survives transmutation.

**Cross-reference (not the operative citation):** §D.2 Table 3 rows 3/4/5 (SBU-NF) and 6/7/8 (LES-NF) at `CAPCO-2016.md:590-595` list NOFORN as the IC-dissem banner consequence for both markings — a back-reference verification, but the per-entry §H.9 anchors are the primary citation per the per-row §-citation convention established in PR 3b.A/D/E/F.

**Reviewer fix (C1, was Q1 — code-reviewer CRITICAL).** Each new `PageRewrite::declarative` entry's doc-comment in `scheme.rs` MUST include an explicit derivation block explaining why "Requires NOFORN." is absent from the source and how the NF implication is structurally derived. Required doc-comment template:

```text
/// CAPCO-2016 §H.9 p178 (SBU-NF) does NOT contain a "Requires NOFORN."
/// sentence — unlike NODIS (§H.9 p174) and EXDIS (§H.9 p172). The NF
/// implication is derived from three structural anchors:
///   (a) Banner-form heading at `CAPCO-2016.md:4388-4398`: the marking's
///       Authorized Banner Line Marking Title literally names it
///       "SENSITIVE BUT UNCLASSIFIED NOFORN"; portion mark is `SBU-NF`.
///       NOFORN is a structural component of the marking's identity.
///   (b) Commingling Rule at `CAPCO-2016.md:4420-4421`: confirms NOFORN
///       persists after transmutation strips the SBU half — even when
///       the source token is dropped, the NF must remain in the portion.
///   (c) §D.2 Table 3 row 3-5 at `CAPCO-2016.md:590-595`: lists NOFORN as
///       the FD&R banner consequence for SBU-NF. Back-reference confirms
///       the page-level dissem-axis invariant.
```

…and the mirror block for LES-NF citing `:4532-4542`, `:4558` (Precedence Rule), and §D.2 Table 3 rows 6-8. The single-string citation `"CAPCO-2016 §H.9 p178"` / `"CAPCO-2016 §H.9 p185"` in the `PageRewrite::declarative` call is the canonical anchor; the derivation belongs in the doc-comment per Constitution VIII (citation discipline requires the authority chain be reviewable in-tree).

**OCR-artifact note (L4 — code-reviewer LOW).** `CAPCO-2016.md:4558` contains the rendered text `U//LES- NF` (space between `LES-` and `NF`) — an OCR artifact in the vendored markdown source. Quote it faithfully in the doc-comment with a `// note: source has whitespace OCR artifact "LES- NF" rendered with a space; canonical token is LES-NF` annotation so a future reviewer doesn't read the space as a citation error.

---

## §3. Predicate + action design

### Token-constant decision: **two new `TokenId` constants required.**

`grep` against `crates/capco/src/scheme.rs` confirms `TOK_SBU_NF` / `TOK_LES_NF` constants do not exist today. The pending-task description's claim that 8.F.2 "requires new `TOK_SBU_NF` / `TOK_LES_NF` constants" is correct; the secondary claim about "coordination with PageContext split logic" is **wrong** — PageContext is NOT edited (see §7).

Investigation alternatives:

1. **Add `TOK_SBU_NF` / `TOK_LES_NF` constants** (next free `TokenId`s after `TOK_REL_TO = TokenId(128)`: 129, 130). Wire into `capco_token_category` mapping to `CAT_NON_IC_DISSEM`. Extend `capco_category_contains` with the corresponding arms scanning `attrs.non_ic_dissem` for `NonIcDissem::SbuNf` / `NonIcDissem::LesNf`. This is the **shape-symmetric** option that mirrors 8.F's NODIS/EXDIS extension and aligns with the eventual Pattern C `classified-strips-{sbu,fouo,limdis,...}` family.
2. **Reuse `Contains` only via `capco_category_contains` matching on the predicate's `(category, token)` pair against new private sentinel constants scoped to this PR**. Less symmetric; would need a separate private mapping.
3. **Use `CategoryPredicate::Custom`** with a hand-written closure scanning `non_ic_dissem` for `SbuNf` / `LesNf`. Loses declarative shape; falls back to the `unannotated-custom-axes` failure mode unless `reads`/`writes` are annotated.

**Decision: option 1.** Two new sentinel `TokenId` constants — `TOK_SBU_NF: TokenId = TokenId(129)` and `TOK_LES_NF: TokenId = TokenId(130)`. Add at the bottom of the existing const block in `crates/capco/src/scheme.rs` immediately after `TOK_REL_TO`, with a doc comment naming this sub-PR.

**Verify at implementation:** confirm no existing const between `TOK_REL_TO` and the start of any other module block uses 129 / 130. Use the next free pair if conflicts exist.

### `capco_token_category` extension

Extend the existing `CAT_NON_IC_DISSEM` arm at `crates/capco/src/scheme.rs:424` to include the two new tokens:

```text
TOK_NODIS | TOK_EXDIS | TOK_SBU_NF | TOK_LES_NF => Some(CAT_NON_IC_DISSEM),
```

This routes any future `FactRemove`/`FactAdd` on the SBU-NF/LES-NF tokens through `apply_fact_remove`'s `CAT_NON_IC_DISSEM` branch (`scheme.rs:761-770`), which already handles `NonIcDissem::Nodis` and `NonIcDissem::Exdis`. **No `apply_fact_remove` extension is required for 8.F.2** because neither new rewrite emits FactRemove against SBU-NF/LES-NF tokens (the rewrites emit FactAdd of `TOK_NOFORN`). `apply_fact_add`'s `CAT_NON_IC_DISSEM` arm remains at `IntentInapplicable` (`scheme.rs:678-682`) — also not required for 8.F.2.

### `capco_category_contains` extension

Extend the existing `CAT_NON_IC_DISSEM` block at `scheme.rs:336-349` with two additional arms scanning the `non_ic_dissem` axis:

```text
if category == CAT_NON_IC_DISSEM {
    if token == TOK_NODIS { /* existing */ }
    if token == TOK_EXDIS { /* existing */ }
    if token == TOK_SBU_NF {
        return attrs.non_ic_dissem.iter().any(|d| matches!(d, NonIcDissem::SbuNf));
    }
    if token == TOK_LES_NF {
        return attrs.non_ic_dissem.iter().any(|d| matches!(d, NonIcDissem::LesNf));
    }
}
```

**Critical invariant (carry-over from 8.F).** Without these arms, the `Contains` triggers silently fall through to `false` and the new rewrites never fire — a no-op masquerading as a fix. Reviewers must verify the arms are present before approving.

### `apply_fact_remove` forward-compatibility doc-comment

**Reviewer note (rust-reviewer H1).** The `capco_token_category` extension routes future `FactRemove(TOK_SBU_NF | TOK_LES_NF)` through `apply_fact_remove`'s `CAT_NON_IC_DISSEM` branch at `scheme.rs:761-770`. Since 8.F.2 emits no such `FactRemove` (Pattern A is FactAdd-only on `CAT_DISSEM`), no implementation extension is required, but a TODO comment at `apply_fact_remove`'s `CAT_NON_IC_DISSEM` arm is required so a future Pattern C `classified-strips-{sbu,les}` rewrite (which will emit FactRemove on these tokens) doesn't silently fall through:

```text
// TODO(8.F.2): TOK_SBU_NF / TOK_LES_NF are routed here by
// capco_token_category but the match arm currently only handles
// TOK_NODIS / TOK_EXDIS. Add SbuNf / LesNf variants when Pattern C
// classified-strips-{sbu,les} rewrites land. 8.F.2 emits FactAdd only,
// so this gap is non-triggering today.
```

### Two new PageRewrite entries

Declared in `CapcoScheme::build_page_rewrites()`, inserted **immediately after** the existing `capco/exdis-implies-noforn` entry and **before** `capco/noforn-clears-rel-to`:

```text
PageRewrite::declarative(
    "capco/sbu-nf-implies-noforn",
    "CAPCO-2016 §H.9 p178",
    CategoryPredicate::Contains {
        category: CAT_NON_IC_DISSEM,
        token: TOK_SBU_NF,
    },
    CategoryAction::Intent(ReplacementIntent::FactAdd {
        token: FactRef::Cve(TOK_NOFORN),
        scope: Scope::Page,
    }),
    SBU_NF_IMPLIES_NF_READS,   // &[CAT_NON_IC_DISSEM]
    SBU_NF_IMPLIES_NF_WRITES,  // &[CAT_DISSEM]
)
```

…and the LES-NF mirror, citing `CAPCO-2016 §H.9 p185`.

Construction form: `ReplacementIntent::FactAdd { token, scope }` struct-literal (no `fact_add` method; only `fact_remove` exists per `crates/scheme/src/fix_intent.rs:244-261`). `#[non_exhaustive]` does not block struct-literal construction from `marque-capco`.

---

## §4. Scheduler ordering verification

### Dataflow declarations (new entries only)

| Rewrite | `reads` | `writes` |
|---|---|---|
| `capco/sbu-nf-implies-noforn` | `[CAT_NON_IC_DISSEM]` | `[CAT_DISSEM]` |
| `capco/les-nf-implies-noforn` | `[CAT_NON_IC_DISSEM]` | `[CAT_DISSEM]` |

### Cycle / order verification

- **No new cycles.** No existing rewrite writes `CAT_NON_IC_DISSEM` (verified by reading every `WRITES` const block in `crates/capco/src/scheme.rs:1100-1226`). The new edge connects to nothing upstream. The two 8.F entries (`nodis-implies-noforn` and `exdis-implies-noforn`) also read `CAT_NON_IC_DISSEM` — they are siblings in the DAG with the two new entries, no ordering dep between them.
- **Same `CAT_DISSEM` writer cohort as 8.F.** The four existing writers of `CAT_DISSEM` (entry 5 ORCON-NATO, entries 6a/6b SBU-NF/LES-NF transmutations, both 8.F entries) and the two new 8.F.2 entries are **all** sibling writers of `CAT_DISSEM`, all ordered **before** `capco/noforn-clears-rel-to` (`CAT_DISSEM` reader). The scheduler `phase_3_noforn_clearer_runs_after_dissem_transmutations` test extends to seven writers (two from 8.F.2 added to its existing five-writer list).
- **No `Custom` actions.** Both new rewrites use `Contains` + `Intent` (declarative). `UnannotatedCustomAxes` N/A.
- **Intent payloads valid.** `FactRef::Cve(TOK_NOFORN)` routes via `capco_token_category(TOK_NOFORN) = Some(CAT_DISSEM)`; `validate_intent_rewrites` at `crates/engine/src/scheduler.rs:50-71` accepts.
- `Engine::new` catches any violation at construction time (covered by the existing `transmutation_rewrites.rs::engine_construction_succeeds_with_full_rewrite_table` test, count-bumped to 13).

### Idempotence path (reviewer fix — rust-reviewer H2)

**The idempotence `IntentInapplicable` path fires at the `CAT_DISSEM` arm at `crates/capco/src/scheme.rs:656-670`, NOT at the `CAT_NON_IC_DISSEM` fallthrough at `:678-682`.** Both Pattern A rewrites emit `FactAdd(FactRef::Cve(TOK_NOFORN), Scope::Page)`. `TOK_NOFORN` maps to `CAT_DISSEM` via `capco_token_category`. The action target is `CAT_DISSEM`, so the second FactAdd's idempotence check runs against the `dissem_controls` axis. If a portion has both `non_ic_dissem = [SbuNf]` and `dissem_controls = [Nf]` already, Pattern A's `FactAdd(NOFORN)` hits the `dissem_controls` membership check at `:670` and returns `IntentInapplicable` — no panic, no duplicate. Test #7 verifies this. Do NOT confuse the idempotence path with the `CAT_NON_IC_DISSEM` fallthrough; the latter exists for forward-compatibility (per the `apply_fact_remove` note above) but is not on the 8.F.2 execution path.

### Compositional invariant for unclassified inputs

For an `(U//SBU-NF)` portion alone on a page, the new `capco/sbu-nf-implies-noforn` rewrite fires and adds `NOFORN` to the page dissem axis. `capco/noforn-clears-rel-to` then fires (REL TO is already empty → no-op). The transmutation rewrite 6a does NOT fire (its predicate is the `never_fires` stub; even when implemented, `(U//SBU-NF)` is the canonical unclassified form so transmutation is a no-op there per §H.9 p178). Pattern A is the only path that adds NF on the unclassified stratum — this is the load-bearing gap-closure for 8.F.2.

---

## §5. Runtime execution gap

Identical posture to 8.F: scheduler-validated but execution-deferred.

- `Engine::new` validates intent payloads + scheduler ordering (catches authoring bugs at construction time). ✓
- `scheme.project(Scope::Page, ...)` reflects the rewrites when called directly by test code or downstream consumers. ✓
- `Engine::lint` / `Engine::fix` output is **unchanged** by 8.F.2's additions. `PageContext`-driven banner validation is unaffected; the existing classified-stratum split logic at `page_context.rs:716-741` continues to do its job (NF injection for classified SBU-NF/LES-NF). The unclassified-stratum gap (where Pattern A closes the invariant at the scheme-projection layer) is also visible only through `scheme.project`.

Test plan implication: all behavioral assertions drive `scheme.project(Scope::Page, &[...])` directly, NOT `Engine::lint`/`Engine::fix`. The G13 content-ignorance test follows the same execution-deferred pattern as 8.F's test #9, but with a different positive-control choice (see §8).

---

## §6. Stage-4 reframe alignment

8.F.2 deliberately mirrors 8.F's structural shape so the future Stage-4 reframe (per `project_incompatibility_class.md`) can collapse Pattern A uniformly:

- Same `Contains(CAT_NON_IC_DISSEM, TOK_*)` trigger shape.
- Same `Intent(FactAdd { Cve(TOK_NOFORN), Scope::Page })` action.
- Same `[CAT_NON_IC_DISSEM]` → `[CAT_DISSEM]` axis flow.
- Same per-row §H.9 page citation discipline.

Once the SCI Pattern A follow-on lands (HCS-O §H.4 p64, HCS-P-sub §H.4 p68, TK-IDIT §H.4 p87, TK-BLFH §H.4 p91, TK-KAND §H.4 p95 — five more entries reading `[CAT_SCI]` writing `[CAT_DISSEM]`), the four `capco/{nodis,exdis,sbu-nf,les-nf}-implies-noforn` and the five SCI entries can be re-expressed as a **single** Stage-4 declarative table walked by one rewrite implementation. Each entry contributes `(category_to_scan, token, citation)`. 8.F.2 does not pre-commit to that collapse — Stage 3's mechanical shape is preserved — but Stage 4 will not need to re-shape the 8.F.2 entries.

**No debt compounded:** declaration-order remains stable; scheduler dependencies remain monotone; no new `Custom` action introduced.

---

## §7. PageContext split-logic coordination

This is the load-bearing complexity 8.F did not face.

### Status before 8.F.2

- `PageContext::expected_non_ic_dissem` at `crates/ism/src/page_context.rs:716-741` runs a hand-rolled classified-context split: SBU-NF → SBU + NF flag; LES-NF → LES + NF flag. The flag flows into `expected_dissem_controls` which injects `DissemControl::Nf`. The split fires **only when `self.is_classified()` is true** (verified at `page_context.rs:726`).
- For unclassified docs containing `(U//SBU-NF)` or `(U//LES-NF)`, the split does NOT fire; NF is NOT injected on the dissem axis; `expected_rel_to` does NOT short-circuit; if any portion carried REL TO, the banner would incorrectly carry it.

### How 8.F.2 composes

The new rewrites are **scheme-projection-layer mutations**. They do NOT touch `PageContext`. The existing classified-stratum split logic stays unchanged. For consumers reading via `scheme.project`:

- Classified `(S//SBU-NF)` portion: Pattern A adds NF to projected dissem. The (future, currently-stub) entry-6a transmutation also adds NF and drops SBU-NF. Both converge on "NF present in projected dissem"; the second FactAdd hits idempotence (`apply_fact_add → IntentInapplicable`). No conflict.
- Unclassified `(U//SBU-NF)` portion: Pattern A adds NF to projected dissem. Transmutation 6a does NOT fire (unclassified is the canonical SBU-NF context). PageContext split does NOT fire (unclassified). **Pattern A is the only NF-injection path** — this is the surface 8.F.2 covers that 8.F did not.
- Classified document with `(U//SBU-NF)` portion (the §H.9 p178 example case `(U//NF//SBU)`): Pattern A adds NF unconditionally based on token presence (classification-agnostic predicate). PageContext split also adds NF via the classified-doc Step 4 path. Both add NF; idempotence resolves to one NF entry.

### No engine-crate edit needed

Constitution VII §IV restricts scheme-adoption PRs from editing engine crates. 8.F.2 stays scheme-side: edits are confined to `crates/capco/src/scheme.rs` (token constants, `capco_token_category`, `capco_category_contains`, `build_page_rewrites`) and `crates/capco/tests/` (new test file, pin-site updates). `crates/ism/src/page_context.rs` is **NOT** edited.

The fact that `PageContext` and `scheme.project` produce slightly different projection outputs for unclassified SBU-NF/LES-NF (PageContext: no NF; scheme.project: NF injected) is **the correct posture for an execution-deferred Stage-3 sub-PR**. The two converge when Phase D/E switches the engine to drive banner validation through `scheme.project`. At that point, the unclassified-stratum gap closes for engine-level consumers; until then, only direct-`scheme.project` consumers see the corrected behavior.

---

## §8. Test plan

All under `crates/capco/tests/`. Constitution VII §IV bars engine-crate test files (`crates/engine/tests/scheduler.rs` is generic and does not need edits).

### New file vs. extend existing

**Decision: new file `crates/capco/tests/pattern_a_sbu_nf_les_nf_supremacy.rs`.** Justification: keeps each sub-PR's behavioral tests in a dedicated file with a self-contained citation anchor block (the existing `pattern_a_noforn_supremacy.rs` is anchored to §H.9 p172/p174 for NODIS/EXDIS — splitting keeps the per-§-citation D13 discipline observable at the file level). The Stage-4 reframe may consolidate both files into a single declarative-table test driver; until then, the per-source-page split is the right granularity for sub-PR review.

### Tests (9 total)

All tests drive `scheme.project(Scope::Page, &[portion_attrs])` directly. Test #5 split into two tests per rust-reviewer L1 (mirror-symmetry with 8.F's per-token composition tests).

1. **`sbu_nf_portion_projects_noforn_to_page_dissem_unclassified`** — `(U//SBU-NF)`; assert projected page `dissem_controls.contains(&DissemControl::Nf)`. The load-bearing unclassified-stratum test (§H.9 p178 case; per `:4410` SBU-NF "May only be used with UNCLASSIFIED" — this is the canonical valid form).
2. **`sbu_nf_malformed_classified_still_injects_noforn`** *(renamed from `…_classified_commingled` per code-reviewer H3 / rust-reviewer M1)* — `(C//SBU-NF)`; assert NF in projected page dissem. **Doc-comment MUST note:** `(C//SBU-NF)` is a **malformed pre-transmutation input** — §H.9 p178 at `CAPCO-2016.md:4410` says SBU-NF "May only be used with UNCLASSIFIED." The Commingling Rule at `:4420-4421` describes that the canonical CORRECTED form for classified portions is `(C//NF)` (SBU dropped, NF added — `(U//NF//SBU)` is the §H.9 example with U classification, NOT `(C//SBU-NF)`). This test verifies Pattern A fires **defensively** on the malformed input; the eventual classified-strips-sbu rule (Pattern C, not in 8.F.2 scope) will transmute the portion to `(C//NF)`. Mechanically, Pattern A's predicate is classification-agnostic — it scans `non_ic_dissem` for `SbuNf` regardless of classification — so the assertion holds.
3. **`les_nf_portion_projects_noforn_to_page_dissem_unclassified`** — `(U//LES-NF)`; assert NF in projected page dissem.
4. **`les_nf_portion_projects_noforn_to_page_dissem_classified`** — `(S//LES-NF)`; assert NF in projected page dissem. **Per `CAPCO-2016.md:4554` LES-NF "May be used with TOP SECRET, SECRET, CONFIDENTIAL, or UNCLASSIFIED"** — unlike SBU-NF, LES-NF is a **valid form** in classified portions. Doc-comment should cite `:4554` and `:4558` (Precedence Rule producing `SECRET//NOFORN//LES` banner).
5. **`sbu_nf_portion_composes_with_noforn_clears_rel_to`** — `(U//SBU-NF)` paired with synthetic prior REL TO; assert projected page has `dissem_controls` containing NF AND `rel_to` empty. Load-bearing composition test — proves the new rewrite runs BEFORE `noforn-clears-rel-to` in the scheduled order.
6. **`les_nf_portion_composes_with_noforn_clears_rel_to`** — mirror of test #5 for `(U//LES-NF)`. Two tests (not one combined) per rust-reviewer L1 — preserves per-token observability matching 8.F's `phase_2_nodis_clears_rel_to` / `phase_2_exdis_clears_rel_to` precedent at `pattern_a_noforn_supremacy.rs:155-227`.
7. **`portion_without_sbu_nf_or_les_nf_does_not_inject_noforn`** — `(U//SBU)` (plain SBU, not SBU-NF) plus `(U//LES)` (plain LES, not LES-NF); assert NF is NOT injected. Catches an over-eager predicate that would scan for any `non_ic_dissem` entry rather than specifically `SbuNf`/`LesNf`.
8. **`sbu_nf_portion_with_noforn_already_present_is_idempotent`** — `(C//SBU-NF/NF)` (synthetic; `non_ic_dissem` has SbuNf AND `dissem_controls` has Nf); assert exactly one NOFORN in projected dissem, no panic. **Exercises the `CAT_DISSEM` arm idempotence at `apply_fact_add → IntentInapplicable` at `scheme.rs:656-670`** (per rust-reviewer H2: idempotence fires on `dissem_controls` membership check, NOT on the `CAT_NON_IC_DISSEM` fallthrough).
9. **`pattern_a_sbu_nf_les_nf_rewrites_emit_no_applied_fix`** — G13 content-ignorance test mirroring 8.F's test #9.

### Test #9 positive-control — pinned per reviewer fix (code-reviewer HIGH 2)

8.F's test #9 used `E038` (NODIS/EXDIS-requires-NOFORN) as the positive control proving the fix pipeline executed on the input. For SBU-NF/LES-NF, **there is no equivalent diagnostic** — `(U//SBU-NF)` and `(U//LES-NF)` are valid portion forms with NF intrinsically embedded; no rule fires saying "NOFORN is missing." `grep`-confirmed: no `E*` rule in `crates/capco/src/rules.rs` or `crates/capco/src/rules_declarative.rs` emits a diagnostic on these inputs.

**Pinned fixture and assertion (code-reviewer HIGH 2; rust-reviewer L1 confirmed E002 severity is `Severity::Fix`).** Test #9 uses the literal byte string `"(U//SBU-NF)\n(S//REL TO GBR)\n"` as input to `Engine::fix`. The second portion `(S//REL TO GBR)` has `rel_to = [GBR]`, `has_usa = false`, which deterministically triggers **E002** (`missing-usa-trigraph`) at `Severity::Fix`. The test must include this exact assertion block to prevent vacuous-pass:

```text
// Positive control: E002 must fire on the second portion (missing USA
// trigraph in REL TO). This is the load-bearing assertion proving the
// fix pipeline executed; without it, the assertions below could pass
// vacuously if Engine::fix silently failed.
let applied_ids: Vec<&str> = result.applied.iter().map(|a| a.proposal.rule.as_ref()).collect();
assert!(
    applied_ids.contains(&"E002"),
    "E002 must appear in audit stream for input `(U//SBU-NF)\\n(S//REL TO GBR)\\n` \
     (second portion missing USA trigraph in REL TO). Without E002 firing, \
     the `no Pattern A AppliedFix` assertions below risk passing vacuously. \
     applied rules: {applied_ids:?}",
);
assert!(
    !applied_ids.contains(&"capco/sbu-nf-implies-noforn"),
    "Pattern A `capco/sbu-nf-implies-noforn` MUST NOT appear in audit stream \
     under current Engine::lint execution-deferred posture (PageRewrites are \
     scheduler-validated but not iterated at lint time). \
     TODO(Phase D/E): flip this assertion to require AppliedFix entries with \
     `proposal.original == \"\"` once banner-validation drives through scheme.project."
);
assert!(
    !applied_ids.contains(&"capco/les-nf-implies-noforn"),
    "Pattern A `capco/les-nf-implies-noforn` MUST NOT appear (same TODO)."
);
```

**Fixture-format verification gate.** The implementation agent MUST verify that `Engine::fix` parses `"(U//SBU-NF)\n(S//REL TO GBR)\n"` as two distinct portions. The 8.F precedent (`pattern_a_noforn_supremacy.rs:480`) uses identical newline-separated portion format. If for any reason the engine's scanner does not recognize `\n` as a portion separator on this input, fall back to BannerLine-prefixed format: `"SECRET//REL TO USA, GBR\n(U//SBU-NF) Para 1.\n(S//REL TO GBR) Para 2.\n"`. Use the 8.F fixture pattern as the prototype.

**Why E002 (not another rule):** (a) E002 is `Severity::Fix` (confirmed by rust-reviewer at `rules.rs:378`), so it produces a non-empty `result.applied`. (b) E002's predicate is deterministic and stateless. (c) E002 is structurally unrelated to Pattern A — verifying that an unrelated rule fired confirms the fix pipeline ran without confounding the Pattern A assertion. (d) E002 does NOT depend on any banner roll-up, page-context lookup, or scheduler-executed PageRewrite; it operates on the portion's `rel_to` axis in isolation.

### TODO comment for Phase D/E flip

When the engine wiring switches `Engine::lint` / `Engine::fix` to drive banner-validation through `scheme.project`, test #8's assertions flip: instead of asserting no AppliedFix carries the new rewrite IDs, assert `applied.proposal.original == ""` (G13 content-ignorance). Same TODO pattern as 8.F's test #9 (see `crates/capco/tests/pattern_a_noforn_supremacy.rs:472-498` for the doc-comment template).

### Updated test files — pin-site bumps

5 pin sites identified per PR #393 lessons. Counts bump **11 → 13**.

- **`crates/capco/tests/corpus_parity.rs`** — 3 sites:
  - Site 1 (`phase_3_declares_eleven_page_rewrites_with_citations`): `assert_eq!(rewrites.len(), 11, ...)` bumps to 13. Function rename to `phase_3_declares_thirteen_page_rewrites_with_citations`. Update the explanation string to enumerate the two new entries with their §H.9 p178/p185 citations.
  - Site 2 (`phase_3_scheduler_exposes_eleven_scheduled_rewrites`): `assert_eq!(scheduled.len(), 11)` bumps to 13. Function rename. Extend the sorted-names array (currently 11 elements) to 13, inserting `"capco/les-nf-implies-noforn"` and `"capco/sbu-nf-implies-noforn"` in alphabetical sort position (verify alpha sort at implementation).
  - Site 3 (`phase_3_noforn_clearer_runs_after_dissem_transmutations`): extend the `for dissem_writer in [...]` loop (currently five writers) with `"capco/sbu-nf-implies-noforn"` and `"capco/les-nf-implies-noforn"`. Update the doc-comment listing DISSEM writers.

- **`crates/capco/tests/scheme_equivalence.rs`** — 1 site (`scheme_declares_phase3_rewrites`):
  - `assert_eq!(rewrites.len(), 11)` → 13.
  - The `ids` array extends to 13. Architect decision (Q2): **append** the two new IDs after 8.F's two entries (preserves 8.F's declaration order; matches §H.9 page-order). Final `ids` order: `[nodis-implies-noforn, exdis-implies-noforn, sbu-nf-implies-noforn, les-nf-implies-noforn, noforn-clears-rel-to, frd-sigma-..., fgi-rollup..., fgi-restricted..., joint-cross-class..., us-presence..., orcon-nato..., sbu-nf-transmutes..., les-nf-transmutes...]`.
  - Citation array: currently 11 indexed assertions. Extend to 13. Citations at positions [2] and [3]: `"CAPCO-2016 §H.9 p178"` and `"CAPCO-2016 §H.9 p185"`.
  - **Reviewer fix (rust-reviewer H3 — dual-page-citation pattern).** After the insert, the citation array WILL contain **two pairs of identical page citations at different positions**: positions [2] and [11] both cite `"CAPCO-2016 §H.9 p178"` (`sbu-nf-implies-noforn` at [2], `sbu-nf-transmutes-on-classified-contact` at [11]); positions [3] and [12] both cite `"CAPCO-2016 §H.9 p185"` (`les-nf-implies-noforn` at [3], `les-nf-transmutes-on-classified-contact` at [12]). This is CORRECT — the §H.9 p178 entry covers both the implication and the transmutation in the same source page; the same is true for §H.9 p185. The implementation agent MUST NOT "deduplicate" these to different page numbers. Add a doc-comment to the test function explicitly noting the dual-citation pattern so a future reviewer doesn't flag it as a copy-paste error.
  - **All index-shifted citations must be re-verified** at implementation time. The implementation MUST grep the citation strings against `crates/capco/docs/CAPCO-2016.md` to confirm each `§X.Y pNN` anchor is valid before merging. This is the Constitution VIII enforcement gate for this PR.

- **`crates/capco/tests/transmutation_rewrites.rs`** — 1 site (`engine_construction_succeeds_with_full_rewrite_table`): `assert_eq!(engine.scheduled_rewrites().len(), 11)` → 13. Update the `expect()` message to name "thirteen-row rewrite table" and list the two new entries.

- **No edits to**: `crates/capco/tests/post_3b_registration_pin.rs` (rule-count pin — no rules added), `crates/capco/tests/category_action_intent.rs` (intent-routing test — no new token routing exercised), `crates/engine/tests/scheduler.rs` (engine-crate; generic).

---

## §9. File-by-file change inventory

Estimates calibrated against 8.F's actuals (PR #393: +214 product / +525 test / 5 pin sites).

```
crates/capco/src/scheme.rs                              +220 lines
  - Two new token constants (TOK_SBU_NF, TOK_LES_NF)    +14 lines (declaration + doc comments)
  - capco_token_category extension                      +2 lines (extend existing arm)
  - capco_category_contains extension                   +16 lines (two new sub-arms)
  - build_page_rewrites: two PageRewrite::declarative   +180 lines (entries + per-entry doc-comments + axis-slice consts)
  - apply_intent_to_marking: doc-comment update only    +8 lines

crates/capco/tests/pattern_a_sbu_nf_les_nf_supremacy.rs NEW, +540 lines, 9 tests
  - Module-level doc comment + citation anchor block (with derivation block per §2)
  - Test fixtures (portion_with_non_ic, portion_with_rel_to, gbr helper)
  - Engine fixture for test #9 (G13 test with pinned E002 fixture)
  - Nine tests per §8 inventory (test #5 split into per-token mirror tests)

crates/capco/tests/corpus_parity.rs                     +/- 28 lines
  - Site 1: function rename + count bump + extended explanation
  - Site 2: function rename + count bump + sorted-names array extension
  - Site 3: dissem-writer list extension + doc-comment update

crates/capco/tests/scheme_equivalence.rs                +/- 22 lines
  - Count bump + ids array extension + 13 index-shifted citation assertions
  - Doc-comment update naming the two new entries

crates/capco/tests/transmutation_rewrites.rs            +/- 10 lines
  - Engine-construction count bump + expect() message update

# Unchanged but verified
crates/capco/src/rules.rs                       (unchanged — no new rule)
crates/capco/src/rules_declarative.rs           (unchanged)
crates/capco/tests/post_3b_registration_pin.rs  (unchanged — rule count stays)
crates/capco/tests/pattern_a_noforn_supremacy.rs (unchanged — 8.F tests stay isolated)
crates/ism/                                     (unchanged — Constitution VII)
crates/scheme/                                  (unchanged — engine-prereq #392 still suffices)
crates/engine/                                  (unchanged — scheduler still generic)
```

**Estimated totals:** +240 lines product (includes derivation doc-comments per C1), +540 lines test (9 tests, not 8), +60 lines pin-site edits. Net registered rule count unchanged. Page-rewrite count 11 → 13.

---

## §10. Open questions — resolution log

### CRITICAL — RESOLVED

**Q1 — Citation strength acceptable under Constitution VIII?** **RESOLVED via §2 doc-comment derivation block (reviewer fix C1).** Both reviewers agree the single-string `"CAPCO-2016 §H.9 p178"` / `"CAPCO-2016 §H.9 p185"` citation is sufficient AT THE API LEVEL, provided the in-tree derivation block in the `PageRewrite::declarative` doc-comment makes the structural authority explicit (banner-form name embedding NOFORN + commingling/precedence rule + §D.2 Table 3 cross-check). Code-reviewer required this explicitly; rust-reviewer concurred (M2). The implementation MUST NOT omit the derivation block.

### HIGH — RESOLVED

**Q2 — Declaration order for the four `*-implies-noforn` entries.** **RESOLVED: append.** Preserves 8.F's declaration order; matches §H.9 page ordering (172 < 174 < 178 < 185). `scheme_equivalence.rs` ids-array updated per §8.

**Q3 — Test #9 (formerly #8) positive control.** **RESOLVED: option (c) with pinned fixture and assertion block.** Fixture: `"(U//SBU-NF)\n(S//REL TO GBR)\n"`. Positive control: E002 (missing USA trigraph, `Severity::Fix`, confirmed by rust-reviewer at `rules.rs:378`). Full assertion block specified in §8 above; implementation MUST copy it verbatim.

### MEDIUM

**Q4 — Scope choice.** Both rewrites use `Scope::Page`. ✓ (Same as 8.F; correct for page-rewrite.)

**Q5 — Future Pattern C interaction.** Pattern C `classified-strips-sbu` (when it lands) will be a PageRewrite reading `[CAT_CLASSIFICATION]` writing `[CAT_NON_IC_DISSEM]` — it removes SBU from the non-IC dissem axis when classification > U. Pattern A `sbu-nf-implies-noforn` reads `[CAT_NON_IC_DISSEM]`. The scheduler will need to order Pattern C BEFORE Pattern A (Pattern C writes the axis Pattern A reads), OR the two will run independently if Pattern C's predicate doesn't fire (unclassified case). Verify at Pattern C implementation time that the order resolves cleanly; for 8.F.2 this is a forward-compatibility note, not a blocker.

**Q6 — SBU/SbuNf vs SBU-NF/SbuNf distinction.** `NonIcDissem::Sbu` and `NonIcDissem::SbuNf` are distinct enum variants per `crates/ism/src/attrs.rs`. Pattern A `sbu-nf-implies-noforn` triggers ONLY on `SbuNf`, NOT on `Sbu`. The corresponding `capco_category_contains` extension must match exactly. Test #6 (`portion_without_sbu_nf_or_les_nf_does_not_inject_noforn`) verifies the negative case for plain `Sbu`.

### LOW

**Q7 — Diagnostic emission.** Silent mutations (no `Diagnostic` for the implication itself). User-facing concerns are covered by W003 (banner-form SBU NOFORN in classified — strips per §H.9 p178) and the eventual transmutation 6a/6b implementations. Pattern A is invariant-preservation, not user-facing.

**Q8 — Performance.** Two new rewrites cost O(portions × |non_ic_dissem|) per `scheme.project` call (typical 0–1 entries). No SC-001 impact.

---

## §11. Reviewer attestation — summary of folded fixes

Both rust-reviewer and code-reviewer ran independently. Verdict: **APPROVE WITH FIXES** (both). All fixes folded into this spec.

| Finding | Severity | Reviewer | Resolved in section |
|---|---|---|---|
| C1: §H.9 p178/p185 lacks "Requires NOFORN." sentence — doc-comment derivation required | CRITICAL | code-reviewer | §2 (citation-strength note + derivation template) |
| H1: `apply_fact_remove` forward-compatibility TODO comment needed | HIGH | rust-reviewer | §3 (apply_fact_remove forward-compatibility doc-comment) |
| H2: Idempotence path is `CAT_DISSEM`, not `CAT_NON_IC_DISSEM` | HIGH | rust-reviewer | §4 (Idempotence path subsection) + test #8 doc-comment |
| H3: `scheme_equivalence.rs` will have dual-page-citation pattern after insert | HIGH | rust-reviewer | §8 (scheme_equivalence.rs bullet) |
| H2 (code-reviewer): Test #9 positive-control fixture must be pinned in spec | HIGH | code-reviewer | §8 (Test #9 positive-control — pinned fixture and assertion block) |
| H3 (code-reviewer): Test #2 framing — `(C//SBU-NF)` is malformed pre-transmutation input | HIGH | code-reviewer | §8 test #2 (renamed `sbu_nf_malformed_classified_still_injects_noforn`) |
| M1: SBU-NF vs. LES-NF classification-applicability asymmetry | MEDIUM | both | §8 test #4 doc-comment cites `:4554` allowing classified LES-NF |
| L1: Test #5 should be two tests per per-token observability convention | LOW | rust-reviewer | §8 (test #5 + test #6 split — 9 tests total) |
| L4: OCR artifact `LES- NF` in source | LOW | code-reviewer | §2 (OCR-artifact note) |

### What reviewers must verify at implementation time

1. **Constitution VIII citation strength** — re-grep `crates/capco/docs/CAPCO-2016.md:4386-4425` (SBU-NF) and `:4530-4562` (LES-NF). Confirm the banner-form headings, Commingling Rule, Precedence Rule, and §D.2 Table 3 rows 3-8 quoted in §2 are accurate. Confirm the page-pin brackets. **Verify the derivation block per C1 is present in the doc-comment of both new entries.**
2. **§3 token-constant decision** — verify `TOK_SBU_NF = TokenId(129)` and `TOK_LES_NF = TokenId(130)` do not collide with any existing constant in `crates/capco/src/scheme.rs` (last allocated: `TOK_REL_TO = TokenId(128)`). Both reviewers confirmed no collision at spec time.
3. **§3 `capco_category_contains` extension** — verify the two new sub-arms scan `NonIcDissem::SbuNf` and `NonIcDissem::LesNf` correctly. rust-reviewer confirmed variants exist at `crates/ism/src/attrs.rs:1163` and `:1168`.
4. **§3 `apply_fact_remove` TODO comment** — verify the forward-compatibility comment is present at the `CAT_NON_IC_DISSEM` arm (rust-reviewer H1).
5. **§4 scheduler invariant** — verify no existing rewrite writes `CAT_NON_IC_DISSEM` (re-grep `WRITES` constants in `scheme.rs:1100-1226`).
6. **§4 idempotence path** — verify test #8's comment correctly cites `scheme.rs:656-670` (the `CAT_DISSEM` arm), NOT `:678-682` (the `CAT_NON_IC_DISSEM` fallthrough) (rust-reviewer H2).
7. **§7 PageContext non-edit** — verify `crates/ism/src/page_context.rs` is NOT edited (Constitution VII compliance). Confirm `expected_non_ic_dissem` at `:726` retains the `if classified` gate.
8. **§8 test #9 positive-control** — verify the pinned fixture `(U//SBU-NF)\n(S//REL TO GBR)\n` triggers E002 at `Severity::Fix` and that the assertion block matches the spec template verbatim (code-reviewer HIGH 2).
9. **§8 test #2 framing** — verify the renamed test `sbu_nf_malformed_classified_still_injects_noforn` has a doc-comment correctly explaining `(C//SBU-NF)` is a malformed pre-transmutation input per `CAPCO-2016.md:4410` (code-reviewer HIGH 3).
10. **§8 dual-page-citation in `scheme_equivalence.rs`** — verify the test function's doc-comment notes the dual-citation pattern (positions [2]/[11] both `§H.9 p178`, positions [3]/[12] both `§H.9 p185`) so a future reviewer doesn't flag as copy-paste (rust-reviewer H3).
11. **§8 9-test count** — verify the new test file contains exactly 9 tests (test #5 split into per-token mirror tests per rust-reviewer L1).
12. **§9 pin-site list** — verify all 5 pin sites identified; bump count `11 → 13` everywhere it appears.
13. **OCR artifact** — verify the doc-comment quoting `:4558` preserves the `U//LES- NF` whitespace artifact with an annotation noting it (code-reviewer LOW 4).
