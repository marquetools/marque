# Implementation Spec — PR 3c.B Commit 10: `FixProposal` cleanup + audit schema bump to `marque-mvp-3`

**Branch**: `refactor-006-pr-3c-b-commit-10-fixproposal-audit-schema-mvp3`
**Authority**: Constitution V Principle V, Constitution VII, Constitution VIII; consolidated plan §"Commit 10 — `FixProposal` cleanup + audit-schema bump"; Decision 9 (Path C amendment); user-memory `feedback_pre_users_no_deprecation_phasing.md`.
**Parent**: `staging` (post-#395 merge; structurally independent of #395)
**Architect**: 2026-05-12 spec dispatch + 2026-05-13 user-directive overrides

## 0. User directives (override architect leans)

The architect's spec defaulted to conservative back-compat retention. User directives (2026-05-13, answering §12 open questions):

1. **Deprecation phasing**: **DELETE `mvp-1` and `mvp-2` entirely**. Accept-list becomes `["marque-mvp-3"]` only. Delete `AuditRecordJsonV1`, `AuditRecordJsonV2`, `applied_fix_to_audit_json_v1`, `applied_fix_to_audit_json_v2`, `AUDIT_SCHEMA_IS_V2` const, all `@marque-mvp-1.snap` and `@marque-mvp-2.snap` snapshot fixtures, and any downgrade-build CI matrix references. Per `feedback_pre_users_no_deprecation_phasing.md` — marque is pre-users, rewrite freely, no schema-bump-for-back-compat.
2. **`replacement` field in audit JSON envelope**: **DELETE**. Top-level audit record carries only structural identifiers (span, fix_intent, classifier_id, etc.); engine reconstructs canonical replacement bytes on demand if a caller needs them.
3. **C001 `apply_text_corrections` migration**: **add `FactRef::TextCorrection(String)` variant** to `marque-scheme::FactRef` (architect's lean (a)). The variant carries the canonical-replacement string (e.g., "SECRET" replacing a typo "SERCET"); this is corpus-derived, not document content, so G13 is satisfied.
4. **OpenVocab JSON shape**: **defer**. Use `serde::Serialize` derive default for `mvp-3`; lock a shape in a follow-up PR when the first consumer emits an OpenVocab `FactRef`. No current consumer in tree.

These directives REDUCE total surface area vs the architect's spec — the cleanup becomes more aggressive (more deletions, no back-compat noise). Net diff estimate shifts more negative (further reduction).

## 0a-resolved. User selected option 3 (defer C001 migration); revised concrete scope below

**User decision (2026-05-13)**: Path 3 — defer C001 migration. `apply_text_corrections` keeps using its existing internal types; Commit 10 scope is rule-emission paths + audit schema bump.

### Concrete implementation shape (option 3)

1. **`FixProposal` lifecycle**: cut from `crates/rules/src/lib.rs` public API; paste into `crates/engine/src/text_correction.rs` as `pub(crate) struct TextCorrectionProposal`. The struct keeps the four fields C001 needs: `span`, `replacement: Box<str>`, `confidence: Confidence`, `source: FixSource`. Migrate `migration_ref`, `original` only if C001 actually uses them — drop the rest of `FixProposal`'s surface (specifically `original`, since C001 has no use for the original-bytes field; the audit only needs the canonical replacement).

2. **`AppliedFixProposal<S>` reshape**:
   - From: `Legacy(FixProposal)` / `New { intent: FixIntent<S>, synthesized: FixProposal }`
   - To: `FixIntent(FixIntent<S>)` / `TextCorrection { replacement: Box<str> }`
   - The `TextCorrection` variant is constructed only by `apply_text_corrections` inside `marque-engine`. The variant body carries the canonical replacement string (corpus-derived; G13-permitted per Constitution V).

3. **Audit JSON v3 envelope** (replaces top-level `original` + `replacement` fields):
   ```json
   "proposal": {
     "kind": "FixIntent",      // or "TextCorrection"
     "intent": { "kind": "FactRemove", "scope": "Page", "facts": [...] }
     // for TextCorrection:
     // "replacement": "SECRET"
   }
   ```
   Discriminant via `kind` (matches `AppliedFixProposal` enum discriminant). For C001 records, the `replacement` field carries the corpus-derived canonical token — this IS document-replacement bytes, but corpus-derived and on the permitted-identifier list (Constitution V Principle V: "token canonicals"). For rule fixes, `intent` carries the structural fix-set delta.

4. **`Diagnostic.fix_intent → fix`** rename: rules emit `Diagnostic { fix: Option<FixIntent<S>>, ... }`. Old `Diagnostic.fix: Option<FixProposal>` and `Diagnostic::with_fix_and_intent` retire.

5. **Deleted infrastructure** (option 3 retains everything option (a) did, except the `FactRef::TextCorrection` variant):
   - `pub struct FixProposal` (moved internal, renamed)
   - `pub enum AppliedFixProposal::{Legacy, New}` (reshaped — different variants, same enum name)
   - `__engine_promote_legacy`
   - `fix_intent_to_legacy_proposal` (already dead)
   - `intent_index` dual-population
   - `AUDIT_SCHEMA_IS_V2` (replaced by `IS_V3`)
   - `AuditRecordJsonV1`, `AuditRecordJsonV2`, `applied_fix_to_audit_json_v1`, `applied_fix_to_audit_json_v2` (CLI + WASM)
   - `@marque-mvp-1.snap`, `@marque-mvp-2.snap` snapshot fixtures
   - `mvp-1` and `mvp-2` from build.rs accept-list

6. **Untouched**: `marque-scheme::FactRef` enum stays at two variants (`Cve`, `OpenVocab`). No new variants. No phase-boundary semantic violation.

## 0a-original-finding. Preflight reviewer HIGH finding (for archival)

Preflight code-reviewer (2026-05-13) flagged HIGH on §0 directive 3 (`FactRef::TextCorrection(String)` variant):

> `FactRef` is documented as "A reference to a token in the **projected fact set**." Text corrections (`apply_text_corrections` at `crates/engine/src/engine.rs:1818-1888`) run on the raw byte buffer BEFORE parsing — there is no projected fact set at that pipeline phase. Adding `FactRef::TextCorrection(String)` would mean "this fix refers to a token in the projected fact set whose value is X," but no fact set exists yet. This is a **semantic-coherence violation** — not a Constitution VII crate-graph violation (no new edge is added) and not a G13 audit-content violation (the payload is a permitted canonical string per Constitution V's permitted-identifier list), but the `marque-scheme` trait surface accretes domain-engine vocabulary it shouldn't carry.
>
> The architect's option (b) — small internal-only engine-side struct replacing `FixProposal` only within `apply_text_corrections` — is correct. The audited C001 emission already carries `FixSource::CorrectionsMap` for provenance. The `mvp-3` JSON envelope for C001 records can use a parallel discriminant (e.g., `"kind": "TextCorrection"` on `fix_intent`, or a dedicated `text_correction` envelope) without forcing `FactRef` to model a concept that exists entirely outside the fact-set pipeline phase.

**User decision needed**. The user said "indifferent, so (a)" without being told about the projected-fact-set contradiction. Three paths forward:

1. **Confirm (a) anyway** — semantic-coherence concern is acknowledged but pragmatic ergonomics of a single emit path through `FixIntent` win. Accepts that `FactRef::TextCorrection` is a slight semantic violation.
2. **Switch to (b)** — small internal-only struct in `marque-engine`; `mvp-3` JSON for C001 uses a parallel `text_correction` envelope alongside `fix_intent`. Keeps `marque-scheme` semantically pure.
3. **Defer C001 migration entirely** — `apply_text_corrections` keeps using `FixProposal` (internal-only) and emits `mvp-3` records that include a legacy-shape `text_correction` sub-object. Commit 10 still cleans up rule-emission paths and bumps the schema but leaves the pre-scanner C001 path on its existing internal types.

Recommend path (2) or (3); both are structurally cleaner than (1). Path (2) does more of the cleanup in this PR; path (3) defers the C001 cleanup but leaves Commit 10 scope smaller.

## 0b. Spec internal consistency fixup (2026-05-13)

§0 directive 1 says delete v1 + v2 entirely, but earlier-drafted §3 (row 10), §6.1, §6.8, §7 (row 2), §8 (items 2-3) still reference "downgrade-build CI matrix" and "keep `mvp-1` / `mvp-2` in the accept-list." Those references are STALE per §0 directive 1; the load-bearing answer is §0. The mass-update across the spec body is not yet applied — implementation reviewers should treat §0 as authoritative if a conflict appears.

---

## 1. Scope decision — SINGLE PR, atomic cutover

**Recommendation: land cleanup and schema bump as ONE commit / one PR.** This is not architect's preference; it is the binding constraint set by Decision 9 Path C and FR-014.

Rationale:
- The schema bump's *only* legitimate justification is that the underlying `AppliedFix.proposal` payload structurally changes. The structural change IS the `FixProposal → FixIntent<S>` cleanup (`AppliedFixProposal::Legacy` retires, `__engine_promote_legacy` retires, `AppliedFixProposal` enum collapses, `Diagnostic.fix: Option<FixProposal>` retires). Splitting would create one of two failure modes:
  1. **Cleanup first, bump later**: between merge of cleanup-PR and merge of bump-PR, the binary emits `marque-mvp-2` JSON whose `proposal.original` is structurally always `""` (because the synthesized projection enforces G13 on the new path). Downstream consumers pinned to `mvp-2` would see a silent semantic shift in `original` without a schema bump. FR-014 violation.
  2. **Bump first, cleanup later**: re-creates the exact "two structurally-distinct shapes under one schema name" hazard that Decision 9 Path C exists to avoid (rejected from Path A).
- Decision 9 Path C explicitly mandates atomicity: "Commit 10 atomically (a) removes `FixProposal` from `marque-rules`, (b) flips `MARQUE_AUDIT_SCHEMA` default to `marque-mvp-3`, (c) extends the accept-list, (d) removes the engine-side legacy-emit conversion, and (e) lands the `FixIntent`-shape JSON as the `marque-mvp-3` shape."

**One PR, one merge commit, atomic cutover.** The PR diff will be larger than the typical 8.x sub-PR (estimate §9 below) but the alternative is a constitutional violation window.

---

## 2. What `marque-mvp-3` adds (the structural delta from `mvp-2`)

### 2.1 Naming: `marque-mvp-3`, not `marque-1.0`

The audit-record.md contract spec describes a *future* `marque-1.0` schema with a fundamentally different shape (`{ "rule": { "scheme": "capco", "predicate_id": "..." } }`, `replacement.canonical.source` discriminant, BLAKE3 digests, closed `MessageTemplate`, no `original` bytes). That is the **post-keystone** target.

Commit 10 does NOT land `marque-1.0`. It lands `marque-mvp-3`, which is the intermediate bump per the consolidated plan Decision 9 Path C amendment. The `mvp-N → 1.0` renaming retires later (per `quickstart.md` and `spec.md` FR-034, scheduled within the broader 006 refactor — *not* this PR).

If a reviewer asks "why not jump straight to `marque-1.0`?": the `mvp-3 → 1.0` work requires `MessageTemplate` JSON serialization, `(scheme, predicate-id)` rule encoding, `Canonical<S>` provenance shape, BLAKE3 digesting, and content-ignorance canary tooling. None of that is in scope for Commit 10. Conflating them risks shipping `marque-1.0` half-built.

### 2.2 Structural shape delta `mvp-2 → mvp-3`

Today (`mvp-2`, AuditRecordJsonV2 in `marque/src/render.rs:413-440` and `crates/wasm/src/lib.rs:369-388`):

```json
{
  "schema": "marque-mvp-2",
  "rule": "E054",
  "source": "BuiltinRule",
  "span": { "start": 12, "end": 25 },
  "original": "SECRET//RELIDO//REL TO GBR",
  "replacement": "SECRET//REL TO GBR",
  "confidence": 0.95,
  "migration_ref": null,
  "timestamp": "...",
  "classifier_id": "12345",
  "dry_run": false,
  "input": "/path/file.txt",
  "recognition": 0.95,
  "runner_up_ratio": null,
  "features": []
}
```

After Commit 10 (`mvp-3`):

```json
{
  "schema": "marque-mvp-3",
  "rule": "E054",
  "source": "BuiltinRule",
  "fix_intent": {
    "kind": "FactRemove",
    "scope": "Page",
    "facts": [ { "kind": "Cve", "token_id": 17 } ]
  },
  "span": { "start": 12, "end": 25 },
  "confidence": 0.95,
  "migration_ref": null,
  "timestamp": "...",
  "classifier_id": "12345",
  "dry_run": false,
  "input": "/path/file.txt",
  "recognition": 0.95,
  "runner_up_ratio": null,
  "features": []
}
```

**What `mvp-3` adds**:
1. New top-level `fix_intent` sub-object (replaces `original` + `replacement`). Discriminated by `kind`. The closed `ReplacementIntent` variant set determines the rest of the shape.
2. **`original` field deletion** — load-bearing G13 change. Today `original` always carries document bytes on the legacy path (the synthesized projection forces `""` only on the new path during commits 2–9; legacy-emitting rules still ship bytes). Post-cutover no rule emits to the legacy path. `original` becomes structurally unrepresentable in the audit record, closing the most recent pre-existing G13 channel by construction.
3. **`replacement` field semantic change** — architect's lean: **delete from top-level**. Today it carries the engine-synthesized replacement bytes (canonical-vocabulary only on `FixIntent`-emitting rules; potentially document-influenced on legacy rules). Post-cutover not present as a top-level field; reconstructable by the engine at audit-render time from `(scheme, fix_intent, scope)`. Downstream consumers needing rendered bytes call back into the engine.

**Central justification**: the structural envelope of `mvp-3` carries strictly less document content than `mvp-2` (no `original`, no `replacement` bytes). The bump captures (a) the architectural restatement of fixes from "byte-span replacements" to "structural fact-set deltas", and (b) the G13 closure on the legacy emission path. Without the bump, the new shape would lie about being `mvp-2`.

**Alternative variant** (if reviewer rejects the `replacement`-field deletion): preserve `replacement` (engine reconstructs from intent at emit time, ships the bytes). Pro: easier downstream migration. Con: re-opens the G13 reconstruct-from-canonical channel at the audit boundary. Architect's lean is **delete** because canonical-vocabulary-only output is already a Constitution V permitted identifier; consumers that want the bytes can call the renderer themselves. **Open for preflight reviewer input.**

### 2.3 `confidence`, `source`, `recognition`, `runner_up_ratio`, `features`

Unchanged from `mvp-2`. Top-level snapshot fields on `AppliedFix<S>` already exist; only the `proposal` sub-object reshapes.

---

## 3. Cleanup inventory

Each item is dead/redundant *post-cutover*. Removal is gated on the schema flip — they cannot retire before Commit 10's atomic boundary.

| # | Item | Location (file:line approx) | Why safe |
|---|------|------------------------------|----------|
| 1 | `pub struct FixProposal` + `impl FixProposal::new` + 8 unit tests | `crates/rules/src/lib.rs:307–381, 1191–1367` | No rule emits via legacy path post-Commit-9; engine no longer needs to wrap it |
| 2 | `pub enum AppliedFixProposal<S>` (`Legacy(_)` + `New { intent, synthesized }`) + manual `Clone` + `Deref` | `crates/rules/src/lib.rs:411–477` | The `Legacy` variant retires; `New`'s wrapper becomes unnecessary — collapse to `pub proposal: FixIntent<S>` on `AppliedFix<S>` |
| 3 | `pub fn AppliedFix::__engine_promote_legacy` | `crates/rules/src/lib.rs:654–674` | All engine call sites switch to `__engine_promote(intent)` |
| 4 | `__engine_promote` `_rule_id` + `synthesized: FixProposal` parameters | `crates/rules/src/lib.rs:733–759` | Signature simplifies: `__engine_promote(intent, timestamp, classifier_id, dry_run, input, token)` |
| 5 | `Diagnostic.fix: Option<FixProposal>` field + `Diagnostic::new` + `with_fix_and_intent` + dual-population logic | `crates/rules/src/lib.rs:870, 905–923, 1023–1042` | All rules emit via `fix_intent` only post-Commit-9. Rename `fix_intent → fix` to reclaim the ergonomic name |
| 6 | `fn fix_intent_to_legacy_proposal` (currently `unimplemented!()` body, `#[allow(dead_code)]`) | `crates/engine/src/engine.rs:1984–2014` | Already dead code; the engine-prereq replaced its single-intent shape with `synthesize_intent_only_fixes`. Delete entirely |
| 7 | `intent_index: HashMap<(RuleId, Span), &FixIntent>` dual-population paired-promotion code | `crates/engine/src/engine.rs:1456–1522, 1685–1759` | After cleanup the engine has only one promotion path (intent). The `intent_index` lookup vanishes; `intent_only_synthesized` becomes *the* synthesis path |
| 8 | `Engine::apply_text_corrections` `__engine_promote_legacy` call site | `crates/engine/src/engine.rs:1877–1885` | Migrate C001 emissions to `FixIntent::FactAdd { token: FactRef::CorrectionsMap(...), scope: Portion }` or equivalent. **NOTE**: this is non-trivial — C001 corrections are pre-scanner text-level, not category-level. May need a `FactRef::TextCorrection` variant on `FactRef<S>`. Investigate at implementation; if intractable, scope-defer (see §8) |
| 9 | `applied_fix_to_audit_json_v1` / `applied_fix_to_audit_json_v2` (CLI + WASM) | `marque/src/render.rs:467–517`, `crates/wasm/src/lib.rs:396–454` | Rewrite as `applied_fix_to_audit_json_v3` reading `fix.proposal: FixIntent<S>` directly |
| 10 | `AUDIT_SCHEMA_IS_V2: bool` const + `const_str_eq` helper, dispatch branches | `crates/engine/src/lib.rs:90–105`, `marque/src/render.rs:531–535`, `crates/wasm/src/lib.rs:465–470` | Replaced by `AUDIT_SCHEMA_IS_V3: bool`. `IS_V2` stays during the deprecation window only if `mvp-2` stays in the accept-list (it does — see §2.1, §7) |
| 11 | `AuditRecordJsonV1` struct (CLI + WASM) | `marque/src/render.rs:~360–394`, `crates/wasm/src/lib.rs:~340–362` | Retained: `mvp-1` stays in the accept-list (downgrade-build use case). Architect's lean: **keep**; deletion is a separate decision (per Decision 03 §358) |
| 12 | Doc-comment migration noise referencing "Commit 2–9 transition", "Path C", "PR 3c.B Commit 10 retires this", etc. | `crates/rules/src/lib.rs` (many lines), `crates/rules/src/fix_intent.rs:25–46, 83–88`, `crates/engine/src/engine.rs:1389–1500` | Replace with present-tense post-cutover docs; no transition-window language |
| 13 | Snapshot fixtures `crates/engine/tests/snapshots/fix_pipeline__audit_record_snapshot_*@marque-mvp-2.snap` | `crates/engine/tests/snapshots/` | Add `@marque-mvp-3.snap` siblings; `@mvp-2` snapshots retained for downgrade-build CI matrix (see §6) |

**Items NOT in scope to remove**:
- `FixSource` enum — still needed; ships as a top-level field on `AppliedFix`. No structural change.
- `Confidence`, `FeatureId`, `FeatureContribution` — unchanged.
- `Message`, `MessageTemplate`, `MessageArgs` — unchanged (the JSON serialization of `Message` is a separate, `marque-1.0`-scoped concern).
- `Severity::Suggest` channel — unchanged.
- `EnginePromotionToken` + `__engine_construct` — unchanged (the seal continues to gate the single `__engine_promote` constructor).
- `synthesize_intent_only_fixes` — unchanged; becomes *the* synthesis path (currently it lives alongside dual-population).

---

## 4. Cross-crate coordination

Atomic-merge sites that all flip together in one commit:

| Crate | File | Edit |
|-------|------|------|
| `marque-rules` | `crates/rules/src/lib.rs` | Delete `FixProposal`, `AppliedFixProposal`, `__engine_promote_legacy`; reshape `Diagnostic`, `AppliedFix::__engine_promote` signature; rename `Diagnostic.fix_intent → fix` |
| `marque-rules` | `crates/rules/src/fix_intent.rs` | Doc-comment cleanup (retire transition-window language at lines 25–46, 83–88) |
| `marque-engine` | `crates/engine/build.rs` | Extend `ACCEPTED` to `["marque-mvp-1", "marque-mvp-2", "marque-mvp-3"]`; flip `DEFAULT` to `"marque-mvp-3"` |
| `marque-engine` | `crates/engine/src/lib.rs` | Add `AUDIT_SCHEMA_IS_V3` const; doc-comment update on `AUDIT_SCHEMA_VERSION` (lines 70–90); keep `AUDIT_SCHEMA_IS_V2` (downgrade builds still emit v2) |
| `marque-engine` | `crates/engine/src/engine.rs` | Delete `fix_intent_to_legacy_proposal` (already unimplemented), `intent_index`, `__engine_promote_legacy` call sites; collapse promotion to single `__engine_promote(intent, ...)` path; migrate `apply_text_corrections` (see Inventory item 8) |
| `marque-engine` | `crates/engine/tests/audit.rs` | Add `mvp-3` schema fixture (parallel to lines 598–619); keep `mvp-1` and `mvp-2` fixtures for downgrade-build coverage |
| `marque-engine` | `crates/engine/tests/fix_pipeline.rs` | Update `applied_fix_to_json` to emit v3 shape when `AUDIT_SCHEMA_IS_V3`; preserve v1/v2 branches for downgrade builds |
| `marque-engine` | `crates/engine/tests/snapshots/*.snap` | Add `@marque-mvp-3.snap` files; do **not** delete `@marque-mvp-2.snap` |
| `marque-engine` | `crates/engine/tests/audit_completeness.rs`, `intent_only_byte_identity.rs` | Audit-record reads update from `fix.proposal.{span,original,replacement,source}` to `fix.proposal: FixIntent<S>` destructure |
| `marque` (CLI) | `marque/src/render.rs` | Add `applied_fix_to_audit_json_v3` + `AuditRecordJsonV3` struct; extend `render_audit_record` dispatch; keep v1/v2 paths for downgrade-build CI |
| `marque` (CLI) | `marque/src/main.rs` | Verify `marque --version` output mentions `AUDIT_SCHEMA_VERSION` (per audit-record.md §D3; pre-existing — re-verify after bump) |
| `marque-wasm` | `crates/wasm/src/lib.rs` | Same as CLI: add `applied_fix_to_audit_json_v3` + `AuditRecordJsonV3`; extend dispatch in `serialize_applied_fix`; keep v1/v2 paths |
| `marque-server` | `crates/server/` | If a `proposal.*` reader exists, update. **Pre-flight grep required** — server is sparsely populated today and may not have an audit-emit path yet |
| `marque-capco` | `crates/capco/tests/*.rs` (≥7 test files use `proposal.*`) | Update fixture assertions to read via `FixIntent` variants |
| `marque-capco` | `crates/capco/src/rules.rs`, `rules_declarative.rs` | **No edits expected.** Rules already emit `fix_intent` post-Commit-9. Confirm with grep |
| Workspace | `CLAUDE.md` "Active Technologies" section | Update `MARQUE_AUDIT_SCHEMA` accept-list and default; add `marque-mvp-3` to the line currently reading `["marque-mvp-1", "marque-mvp-2"]` |
| Workspace | `CHANGELOG.md` | New entry under unreleased: `marque-mvp-2 → marque-mvp-3`, rationale, structural-shape delta, downstream migration note |
| Workspace | `specs/006-engine-rule-refactor/contracts/audit-record.md` | Add `mvp-3` section above the existing `marque-1.0` content. Mark `mvp-3` as the active schema and `1.0` as the post-keystone target |

**Sequence within the single commit**: edits must compile at the end-state. There is no incremental-compile path — `cargo check --workspace` is green only after every edit lands together. Reviewer should expect a large diff with no intermediate green states. This is unavoidable; the cleanup is structural.

---

## 5. Constitution check

### V Principle V (audit-record content ignorance, G13)

**Strengthened post-cutover.** The pre-cutover `mvp-2` JSON carries `original` and `replacement` as document/canonical bytes. The post-cutover `mvp-3` JSON deletes `original` entirely and (per §2.2 architect-leaned variant) deletes `replacement` from the top-level field set. The audit record's structural envelope shrinks to: span offsets, BLAKE3-digestable identifiers (deferred to `marque-1.0`), enumerated discriminants (`FactAdd`/`FactRemove`/`Recanonicalize`, `Portion`/`Page`, `Cve`/`OpenVocab`), token IDs, posterior scalars, classifier ID. Every field is on Constitution V's permitted-identifier list.

**Carve-out preserved**: the test-fixture carve-out language (`crates/rules/src/lib.rs:614–637`) survives unchanged. `__engine_promote`'s signature changes; the engine-only contract does not. The `EnginePromotionToken` seal stays load-bearing.

**Audit-completeness test** (`crates/engine/tests/audit_completeness.rs`): re-run after cutover; confirm every fix promoted by `Engine::fix` lands in `applied`. Constitution V Principle V requires regardless-of-confidence-level audit promotion; this property is structural to `Engine::fix_inner` and does not change with the schema bump.

### V Principle V (`AppliedFix::__engine_promote` is engine-only)

`__engine_promote`'s signature changes (drops `_rule_id`, `synthesized`). The function name stays `__engine_promote` — the `promote-callsite-lint` (`tools/promote-callsite-lint/`) still flags every external call. The `__engine_promote_legacy` name retires entirely; verify post-cutover no source file references it.

**Pre-flight grep gate**: `rg '__engine_promote(_legacy)?' --type rust` must return only engine-internal call sites + test-fixture carve-out sites. Verify before merge.

### VII (crate discipline)

No graph edges added or removed. `marque-rules` still depends on `marque-scheme` and `marque-ism`. `marque-engine` still convergences. `FixProposal`'s deletion removes a type from `marque-rules`'s public surface but introduces no new crate.

### VIII (citation fidelity)

No CAPCO citations touched. Verify the audit-record.md edits do not introduce or re-state any CAPCO-2016 citations without re-verification against the source PDF (the file mostly references FR-NNN and the constitution, not CAPCO directly).

### III (WASM safety)

`marque-wasm` continues to read `AppliedFix<CapcoScheme>` and serialize to JSON. The new `fix_intent` field is serialized via existing `serde::Serialize` machinery — no new I/O surface, no runtime config expansion, no recognizer change. Constitution III invariant preserved.

### VI (pipeline phases)

Engine pipeline shape unchanged. `Engine::fix_inner` still: scanner → parser → rules → page roll-up → fix application. The only loop-body simplification is "no more dual-promotion dispatch".

---

## 6. Test plan

### 6.1 Regression-pin tests (NEW)

**`crates/engine/tests/audit_schema_accept_list.rs`** (new file, ~60 LoC):

```rust
// Pins the closed accept-list for MARQUE_AUDIT_SCHEMA per Constitution V
// Principle V and FR-014. Pre-Commit-10: ["marque-mvp-1", "marque-mvp-2"].
// Post-Commit-10: ["marque-mvp-1", "marque-mvp-2", "marque-mvp-3"].
// Adding or removing a value MUST coordinate with audit-emit paths;
// this test fails if the accept-list drifts silently.

#[test]
fn accept_list_pinned() {
    // Re-read crates/engine/build.rs at compile time via include_str! and
    // assert it contains exactly the expected ACCEPTED line. Pinned
    // verbatim because the accept-list IS the contract.
}

#[test]
fn audit_schema_version_is_mvp3_by_default() {
    assert_eq!(marque_engine::AUDIT_SCHEMA_VERSION, "marque-mvp-3");
}

#[test]
fn audit_schema_is_v3_const_matches_version() {
    assert!(marque_engine::AUDIT_SCHEMA_IS_V3);
    assert!(!marque_engine::AUDIT_SCHEMA_IS_V2);
}
```

The first test is the regression-pin. Pattern: read the build.rs literal text via `include_str!`, assert exact match against the expected accept-list string. This catches:
- Accidental removal of `mvp-1` or `mvp-2`
- Accidental drift of the default value
- Hand-edits to the build.rs constants without coordinating audit-emit paths

### 6.2 Snapshot tests (UPDATE existing + ADD new)

`crates/engine/tests/snapshots/fix_pipeline__audit_record_snapshot_e002_apply@marque-mvp-3.snap` (new file): expected v3 shape.

`crates/engine/tests/snapshots/fix_pipeline__audit_record_snapshot_e002_apply@marque-mvp-2.snap` (existing): **retained**. The downgrade-build CI matrix (`MARQUE_AUDIT_SCHEMA=marque-mvp-2 cargo test`) still asserts v2 shape.

Same pattern for `e002_dry_run`.

### 6.3 Audit-record fixture coverage (UPDATE)

`crates/engine/tests/audit.rs` currently tests v1 (line 598). Add a parallel `mvp-3` fixture block testing the new shape. Keep v1 and v2 fixtures.

### 6.4 G13 closure test (WIDEN scope)

`crates/capco/tests/g13_closure_fix_intent.rs` currently asserts G13 on `FixIntent`-shape payloads only (Path C scope restriction). Per consolidated plan §"Commit 10 acceptance criteria":

> The strengthened `g13_closure` test from commit 3 now applies system-wide (no scope restriction needed — every `AppliedFix` carries `FixIntent<S>` directly, the structural envelope is uniform).

Delete the scope restriction; assert across all production rules' emission paths.

### 6.5 CLI ↔ WASM byte-identity cross-check (PER CONSOLIDATED PLAN VERIFICATION GATE)

The plan's PR 3c.B-specific verification gate (line 1285–1292) requires: "applied_fix_to_audit_json_v2 paths in crates/marque/src/main.rs (CLI) and crates/wasm/src/lib.rs (WASM) must produce identical NDJSON output for a shared E054 / E057 / E021 / E024 / E058-row / E060 fixture set."

After cutover, replace v2 with v3. Locate the existing parity test (or write one if absent — pre-flight grep `applied_fix_to_audit_json_v2.*parity` and `crates/engine/tests/intent_only_byte_identity.rs`). Confirm CLI and WASM emit byte-identical v3 records for the same fixture set.

### 6.6 Acceptance grep gates (per consolidated plan §1176)

CI script or manual pre-merge check:
- `rg 'FixProposal' crates/ --type rust` returns empty (or only matches in documentation/changelog migration notes).
- `rg 'AppliedFixProposal' crates/ --type rust` returns empty.
- `rg 'fix_intent_to_legacy_proposal' crates/ --type rust` returns empty.
- `rg '__engine_promote_legacy' crates/ --type rust` returns empty.
- `rg 'proposal\.original\|proposal\.replacement' crates/ --type rust` returns empty (no v2-shape audit reads survive).

### 6.7 Bench gate

`bench-check.sh` (or `cargo bench -p marque-engine`): SC-001 latency stays within 10% of pre-cutover baseline. The cutover should be neutral or marginally faster (removes the `intent_index` HashMap construction in the hot path).

### 6.8 Test execution matrix

```
# Default build: mvp-3
cargo +stable test --workspace

# Downgrade: mvp-2
MARQUE_AUDIT_SCHEMA=marque-mvp-2 cargo +stable test --workspace

# Legacy: mvp-1
MARQUE_AUDIT_SCHEMA=marque-mvp-1 cargo +stable test --workspace

# Invalid: should panic at build.rs
MARQUE_AUDIT_SCHEMA=marque-mvp-99 cargo +stable build 2>&1 | grep "is not a recognized schema"
```

All four must succeed (the last via the build-failure assertion).

---

## 7. Risks & mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| External audit-log parsers exist (compliance tooling outside the repo) and break on `mvp-2 → mvp-3` shape change | LOW per Decision 03 §365 ("there are no records, no users") but BINDING per Constitution V if any classified-marking deployment exists | Add CHANGELOG entry explicitly stating: `mvp-2` records and `mvp-3` records are NOT interoperable. The accept-list retains `mvp-2` so a deployment can pin its build to `mvp-2` and continue emitting the legacy shape until its consumers migrate. Audit-record.md "Schema discoverability (D3)" mechanism (`marque --version` exposes `AUDIT_SCHEMA_VERSION`) is the consumer's signal |
| Closed accept-list shape: adding `mvp-3` to the list is deliberate (it IS the new value), but a future PR might tempt-add an unsupported value | MEDIUM | The new `audit_schema_accept_list.rs` regression-pin (§6.1) catches drift. The pin pattern (assert verbatim build.rs literal) means any edit forces a coordinated test update — the cost is the load-bearing review signal |
| `apply_text_corrections` C001 migration to `FixIntent` is intractable (Inventory item 8) | MEDIUM | Pre-flight investigation: does `FactRef<S>` have a `TextCorrection` variant or equivalent? If not, two options: (a) add the variant to `marque-scheme::FactRef` (small extension, ~10 LoC); (b) scope-defer — Commit 10 retires `FixProposal` from rule emission paths but C001 keeps a small internal `FixProposal`-equivalent struct *inside* `marque-engine`. **Architect's lean: (a)** — Constitution VII permits `marque-scheme` to grow; adding a variant is forward-compatible |
| `marque-server` has an audit-emit path not yet audited | MEDIUM | Pre-flight grep gate: `rg 'AppliedFix\|proposal\.' crates/server/ --type rust`. If hits, update in lock-step; if zero hits, server is unaffected and the PR ships without server edits |
| Snapshot test churn: every audit-record snapshot fixture needs an `@mvp-3.snap` sibling | LOW | `insta` auto-generates missing snapshots on first failing run; reviewer accepts each one against the documented v3 shape contract (§2.2). The volume is ~6–10 snapshot files based on the existing tree |
| Mid-cutover commit cannot reach a green state (atomic-merge constraint) | LOW (planning constraint, not a runtime risk) | Author the commit on a feature branch with `git rebase -i` discipline; the final commit is squashed. CI runs on the squashed merge commit only |
| `AUDIT_SCHEMA_IS_V2` and `AUDIT_SCHEMA_IS_V3` both need const-eval; if either evaluates incorrectly at compile time, downgrade-build mode silently emits v3 (or vice versa) | LOW | The `const_str_eq` helper is already proven (lines 92–105). Add `AUDIT_SCHEMA_IS_V3` parallel to `IS_V2`; the new `audit_schema_accept_list.rs` test pins the relationship |
| Open-vocab `FactRef::OpenVocab(...)` JSON serialization unspecified | MEDIUM | The `FactRef<S>` enum has `S::OpenVocabRef` payloads; `mvp-3` JSON shape for these is undefined in this spec. Default to whatever the `Serialize` derive produces; if no derive, **add to spec at implementation** — likely `{ "kind": "OpenVocab", "category": "<CategoryId>", "render": "<rendered-canonical-bytes>" }`. Pre-flight: check whether any current rule emits `OpenVocab`-form `FactRef`; if no current consumer, ship with a minimal shape and refine when the first consumer lands |
| `marque-1.0` schema work (audit-record.md contract) leaks expectations onto `mvp-3` | LOW | Architect-flag: `mvp-3` is **not** `marque-1.0`. Reviewers tempted to add `(scheme, predicate-id)` rule encoding, `Canonical<S>` provenance, BLAKE3 digests, etc. should be redirected to a future PR. §8 lists all `marque-1.0`-shaped items explicitly as out-of-scope |

---

## 8. Out-of-scope (explicit)

The following are *plausibly* in scope for "Commit 10 cleanup + bump" but explicitly belong to follow-up work, NOT this PR:

1. **`marque-1.0` schema cutover.** The `mvp-N` → `1.0` rename, `(scheme, predicate-id)` rule encoding, `Canonical<S>` provenance, BLAKE3 digesting, closed `MessageTemplate` JSON serialization, content-ignorance canary tooling. Tracked at `specs/006-engine-rule-refactor/contracts/audit-record.md`; ships in a later 006 PR, post-PR-3c.B.
2. **Retiring `mvp-1` from the accept-list.** Per Decision 03 §358: separate decision, gated on whether v1-byte snapshot tests remain load-bearing for a real consumer. Not this PR.
3. **`mvp-2` snapshot fixture deletion.** Kept for downgrade-build CI; only retire when `mvp-2` itself leaves the accept-list.
4. **Renaming `AUDIT_SCHEMA_IS_V2` → something more general.** The boolean dispatch shape works for two schema values; a future PR with three or more active discriminants (very unlikely — accept-list is supposed to contract over time) revisits.
5. **Audit-record migration tooling.** No `marque-audit-reader` crate (per audit-record.md §"Pre-cutover compatibility" / FR-037). External consumers branch on `"schema"` field; no in-tree migration utility.
6. **`marque-server` middleware** for audit-stream auth/rate-limiting. Out of scope per consolidated plan.
7. **Re-snapshotting SC-008 byte-identity baseline.** Per consolidated plan verification gate (line 1278): SC-008 baseline re-snapshots at end of Commit 6 — already done. Commit 10's per-commit byte-identity check against Commit 9's baseline is the *gate*, not a new snapshot.
8. **C001-aware `FactRef::TextCorrection` variant if `marque-scheme` extension is intractable**: investigate at implementation; if a clean variant addition is blocked by the trait surface, the fallback per §7 is to retain a small internal-only `FixProposal`-equivalent in `marque-engine` — that fallback IS in scope for this PR (the cleanup is rule-emission-path only; engine-internal helpers may keep legacy shape).

---

## 9. Net diff estimate

Per consolidated plan §1203–1217 estimate: **~−390 LoC net.** Refined per pre-flight investigation:

| Surface | Adds | Deletes | Net |
|---------|------|---------|-----|
| `crates/rules/src/lib.rs` | +30 | −280 | **−250** |
| `crates/rules/src/fix_intent.rs` | +5 | −30 | **−25** |
| `crates/engine/build.rs` | +2 | −2 | **0** |
| `crates/engine/src/lib.rs` | +12 | −5 | **+7** |
| `crates/engine/src/engine.rs` | +20 | −180 | **−160** |
| `marque/src/render.rs` | +60 | −5 | **+55** |
| `crates/wasm/src/lib.rs` | +55 | −5 | **+50** |
| `crates/engine/tests/audit_schema_accept_list.rs` | +60 | 0 | **+60** |
| `crates/engine/tests/audit.rs` | +50 | 0 | **+50** |
| `crates/engine/tests/fix_pipeline.rs` | +30 | −5 | **+25** |
| `crates/engine/tests/snapshots/*.snap` | +6 to +10 new files (~150 LoC total) | 0 | **+150** |
| `crates/capco/tests/*.rs` | +30 | −30 | **0** |
| `crates/engine/tests/audit_completeness.rs`, `intent_only_byte_identity.rs` | +15 | −15 | **0** |
| `CHANGELOG.md` | +35 | 0 | **+35** |
| `CLAUDE.md` | +2 | −1 | **+1** |
| `specs/006-engine-rule-refactor/contracts/audit-record.md` | +60 | 0 | **+60** |
| **TOTAL** | **+~570** | **−~560** | **+10 (≈ neutral)** |

The plan's ~−390 LoC estimate underestimates added test fixtures, new CLI/WASM JSON struct definitions, and CHANGELOG/spec doc additions. Actual net is closer to **neutral** (±50 LoC). The *reduction* in core types is real (~−420 LoC across `lib.rs` + `engine.rs`); the addition is concentrated in tests, JSON shape definitions, and documentation.

---

## 10. Dependencies on other PRs

**PR #395 (8.F-engine-gap, currently in auto-merge)**: structurally independent. PR #395 lands `PageContext` alignment changes for Pattern A NOFORN-supremacy; it edits the engine's per-page roll-up logic, not the audit-record shape or `FixProposal`/`FixIntent` surface. Commit 10 lands on top of #395's merged state with no conflict expected.

**Pre-merge sequence**:
1. Wait for PR #395 to merge to `006-engine-rule-refactor` (or `staging`, depending on the umbrella's rebase strategy).
2. Rebase Commit-10 work onto the post-#395 tip.
3. Open PR; dispatch pre-flight reviewers (rust-reviewer + code-reviewer) BEFORE marking ready (per user's `feedback_run_reviewer_before_pr_open.md` memory).
4. After CRITICAL/HIGH findings resolved, mark ready; auto-merge or manual merge per umbrella convention.

**Subtle dependencies the architect did NOT find** (pre-flight grep was clean):
- No queued PR edits `FixProposal`, `AppliedFixProposal`, `__engine_promote_legacy`, or the audit accept-list.
- No queued PR adds a new `FixSource` variant (would force `mvp-3` JSON shape extension).
- No queued PR touches `MARQUE_AUDIT_SCHEMA` validation or `AUDIT_SCHEMA_VERSION`.

**Forward dependencies** (things that follow Commit 10):
- All other 006-refactor PRs targeting `mvp-3` or `marque-1.0` audit shape.
- The Stage-3 (PR 4) and Stage-4 (PR 5+) work that further reduces the 47-rule count and absorbs more into `Constraint::Custom` / renderer machinery — those PRs may emit new `FactRef`/`ReplacementIntent` variant shapes, which would extend `mvp-3` JSON additively.

---

## 11. Pre-flight reviewer dispatch (current step)

Two reviewers in parallel:

- `ecc:rust-reviewer` — verify Rust idioms across the cleanup (signature changes on `AppliedFix::__engine_promote`, the rename `Diagnostic.fix_intent → fix`, the deletions of `FixProposal` / `AppliedFixProposal::Legacy`). Verify the atomic-cutover constraint doesn't introduce a soundness gap. Verify no `unsafe` paths created.
- `ecc:code-reviewer` — verify Constitution V Principle V (G13 strengthened, carve-out preserved, audit-completeness pin), VII (crate discipline), VIII (citation fidelity), III (WASM safety), VI (pipeline phases). Verify the `replacement`-field deletion (architect's lean) is defensible per Constitution V — or recommend the alternative variant that preserves it. Verify scope boundaries (§8) are correctly drawn.

Apply all CRITICAL and HIGH findings before implementation. MEDIUM findings get folded into commit notes; LOW deferred unless trivial.

## 12. Open architectural questions for user input

If preflight reviewers do not resolve these, surface to user before implementation:

1. **`replacement` field deletion vs preservation in `mvp-3` envelope** (§2.2). Architect's lean is delete (G13 stronger). Alternative preserves it (downstream migration easier). User input would weigh: how aggressive is the G13 closure stance vs how disruptive to external consumers (per Decision 03 "no records, no users today", deletion is currently safe).

2. **C001 `apply_text_corrections` migration** (§3 item 8, §7 row 3). Architect's lean is (a) add `FactRef::TextCorrection` variant. Fallback (b) keeps internal-only FixProposal-equivalent in marque-engine. Either is implementable; the choice is structural.

3. **OpenVocab JSON shape** (§7 row 7). Spec defaults to `Serialize` derive; minimal shape proposed if no current consumer. User input could lock the shape now or defer.

## 13. Post-impl reviewer dispatch (after implementation lands)

Same two reviewers in parallel on the landed diff. Address all CRITICAL/HIGH before PR open. Open PR with full review trail in the description.

---

## Closing note

The bump is justified by the structural shape change (intent-shape `proposal` sub-object, deletion of `original`/`replacement` byte fields, G13 closure on the legacy emission path). The cleanup is justified by the post-Commit-9 redundancy of every retired item (each is verifiably dead code post-cutover). Atomicity is mandated by FR-014. Single PR, single squashed commit.
