# PR 3c.2 Plan and PM Decisions

**Date**: 2026-05-19
**Branch**: `docs/006-pr3c2-plan` (off `origin/staging@a8b734ac`)
**Base PR**: `staging`
**Status**: LOCKED 2026-05-19 — PM contract; preflight agents proceed against this scope.

**Predecessor decisions register entries**: `specs/006-engine-rule-refactor/decisions.md` D25 (this set).

**Spec anchors**:
- `specs/006-engine-rule-refactor/spec.md` FR-035, FR-035a, FR-037, FR-043, FR-052, FR-053
- `specs/006-engine-rule-refactor/tasks.md` T041, T042, T043, T046, T048, T048a, T048b, T048c, T050, T054, T055
- `specs/006-engine-rule-refactor/contracts/audit-record.md` §0 (active `marque-mvp-3`) + §1 (post-keystone `marque-1.0` target)
- `docs/plans/2026-05-02-engine-refactor-consolidated.md` §10.2 (cutover composition)

---

## 0. Scope

PR 3c.2 lands the four atomic structural commitments deferred from PR 3c.B Commit 10 per FR-035a, plus the implied prerequisites. Schema bump `marque-mvp-3 → marque-1.0` at the atomic cutover sub-PR (3c.2.D); preceding sub-PRs land prep work additively under the active `marque-mvp-3` schema.

**Four FR-035a commitments**:

1. `Canonical<S>` provenance wired into audit emit — `discriminant: strict|decoder` + structured `canonical` sub-object with `source: cve|open_vocab`.
2. BLAKE3 audit-record digesting — `blake3` added to workspace dep graph; `Blake3Hash::zero()` placeholder replaced with real `blake3::hash(...)`.
3. Closed-set `MessageTemplate` JSON in audit output — `Diagnostic.message: Box<str>` → `Message` (closed `MessageTemplate` + `MessageArgs`); renderer emits `{"template", "args"}`.
4. `from_parsed_unchecked` adapter deletion — 25 surviving call sites (mixed production + test) migrated to `<S as MarkingScheme>::canonicalize(parsed)`.

**Implied prerequisites that land alongside**:

- `MarkingScheme::canonicalize(ParsedAttrs<'_>) -> CanonicalAttrs` trait method (does not exist at HEAD — sole post-keystone path per FR-043).
- `RenderContext { scope, emission_form, schema_version }` + `#[non_exhaustive] enum EmissionForm { Auto, Portion, BannerTitle, BannerAbbreviation }` (FR-052).
- `Citation { section: SectionRef, page: PageNumber, document: AuthoritativeSource }` struct with `const fn new(...)` constructor.
- T055 deterministic NDJSON canary scan (proves G13 content-ignorance is a type invariant; retires `core_error_isolation.rs:92` masking pin #257).

**Explicitly NOT in scope** (deferred elsewhere):

- 2-tuple `(scheme, predicate_id)` `RuleId` form — post-PR-10 per FR-049 (stability-freeze rule).
- R001/R002 sentinel `"engine"` scheme labels — same post-PR-10 PR as the 2-tuple migration.

---

## 1. Sub-PR decomposition

Five-sub-PR series, all against `staging`. Each sub-PR is independently revertable per Constitution US8. PR 4b's 9-sub-PR umbrella is the precedent shape.

| Sub-PR | Schema | Scope | Closes |
|---|---|---|---|
| **3c.2.A — Scaffolding** | `marque-mvp-3` (no bump) | `blake3` workspace dep + `canonicalize` trait method + `RenderContext`/`EmissionForm` + `Citation` const-fn struct + `render_canonical` signature update + EmissionForm tests | T043 (groundwork), T048, T048a, T048b, T048c |
| **3c.2.B — Call-site migration** | `marque-mvp-3` (no bump) | ~25 call sites of `from_parsed_unchecked` → `canonicalize`; adapter retained | T054 (groundwork) |
| **3c.2.C — Diagnostic reshape** | `marque-mvp-3` (no bump) | `Diagnostic.message: Box<str> → Message`; `Diagnostic.citation: &'static str → Citation`; engine.rs:1389 decoder `format!` closed; ~5–6 `format!`-built `Diagnostic.message` sites in `crates/capco/src/rules*.rs` migrated | T042, T046, T050 |
| **3c.2.D — Atomic cutover** | **`marque-1.0`** (bump) | `AppliedFix` v2 (Canonical sub-object + real BLAKE3 + discriminant); `FixIntent \| TextCorrection` envelope dropped; non-marking text corrections become own `{"type":"text_correction", ...}` NDJSON line; T055 canary + #257 masking pin retirement; every `__engine_promote` test fixture migrated; audit-record contract docs updated | T041, T055, FR-035a (a)+(b)+(c), SC-001, #257 |
| **3c.2.E — Adapter deletion** | `marque-1.0` (no bump) | Delete `from_parsed_unchecked` in `crates/ism/src/canonical.rs`; drop path-based promote-callsite-lint carve-out; doc-comment cleanups | T054, FR-043 sole-path invariant |

### Sequencing

```text
3c.2.A (scaffolding)
  ↓
3c.2.B (call-site migration)
  ↓
3c.2.C (diagnostic reshape) ← can land in parallel with B if scheduling allows
  ↓
3c.2.D (atomic cutover) — depends on B + C complete
  ↓
3c.2.E (adapter delete) — depends on B complete
```

**Parallel opportunity**: B and C touch disjoint surfaces (B = parser→canonicalize path; C = Diagnostic emission surface). After A lands, B and C can land concurrently if review-chain capacity allows.

**Atomicity contract**: D is the only sub-PR that changes the audit wire format. A/B/C are additive — pre-cutover audit consumers continue to parse output unchanged. E is dead-code removal.

---

## 2. PM Decisions

### D25.1 — Sub-PR decomposition: five-PR series (A/B/C/D/E)

**Decision**: Split PR 3c.2 into five sub-PRs per the table in §1.

**Rationale**:
- Constitution US8 revertability — each sub-PR can be reverted without losing the others' work.
- Precedent: PR 4b's 9-sub-PR umbrella demonstrated that fine-grained decomposition reduces review-cycle risk and accelerates Copilot turnaround.
- The schema bump (atomicity property) only needs atomicity at D; A/B/C are independent additive landings under `marque-mvp-3`.

**Rejected alternatives**:
- 4-sub-PR (fold E into D): increases D's blast radius without saving meaningful review cycles.
- 3-sub-PR (combine A+B prep work, C messages, D+E atomic): conflates trait-surface additions with call-site migrations; harder to revert call-site work if a regression surfaces.
- Single mega-PR: high risk, long review cycles, hard to bisect.

### D25.2 — `Citation` struct: `const fn` constructor, no runtime validation

**Decision**: `Citation::new(section: SectionRef, page: PageNumber, document: AuthoritativeSource)` is a `const fn` that just stores fields. No runtime validation in the constructor. Citation-lint at `tools/citation-lint/` catches drift at CI time.

**Rationale**:
- Threat model for runtime validation is purely citation drift (stale §, wrong page after source revision). This is exactly what citation-lint already enforces for `&'static str` citations today — porting to the struct shape is mechanical.
- WASM size discipline (Constitution III): runtime validation code would ship to the WASM module. Const-fn construction ships only the data layout (~0 bytes).
- Aligns with FR-018 mechanical-verifiability surface — citation correctness is a CI gate, not a runtime check.

**Rejected alternatives**:
- Runtime validation (`Result<Self, CitationError>`): WASM overhead, redundant with citation-lint.
- `macro_rules! citation!(...)` compile-time validation: harder to express §-range validation in const context; citation-lint handles the same property post-build with simpler implementation.

### D25.3 — `Diagnostic.message`: atomic field-type change in C, no transitional dual-field

**Decision**: `Diagnostic.message: Box<str>` → `Diagnostic.message: Message` in PR 3c.2.C as a single field-type change. No transitional `message_legacy: Box<str>` alongside.

**Rationale**:
- Audit wire format doesn't change in C (Diagnostic flows to CLI/WASM output, not directly to `AppliedFix`); the wire-format bump happens atomically at D.
- Marque is pre-users — no deprecation phasing needed per `feedback_pre_users_no_deprecation_phasing.md`.
- Per-rule `format!`-built sites in `crates/capco/src/rules*.rs` are ~5–6 today (verified via grep); mechanical migration scope is small enough that a transitional dual-field would add more code churn than it saves.

**Rejected alternatives**:
- Transitional dual-field: more code churn, two type changes (add then drop) instead of one.
- Defer all `Diagnostic.message` work to a follow-up PR: FR-003 invariant ("`format!` of input bytes is unrepresentable in Diagnostic") would remain open; defeats the purpose of the marque-1.0 closure.

### D25.4 — `AppliedFix` v2: drop `FixIntent | TextCorrection` envelope; text corrections become own NDJSON line type

**Decision**: At PR 3c.2.D, `AppliedFix` becomes marking-only. The current `AppliedFix.proposal: FixIntent<S> | TextCorrection` envelope is replaced with `fix: AppliedFixDetail { replacement: { discriminant: "strict"|"decoder", canonical: Canonical<S> }, original_span: Span, original_digest: Blake3Hash }` per `contracts/audit-record.md` §1. Non-marking text corrections (from `.marque.toml` corrections map) emit as their own NDJSON line type `{"type":"text_correction", ...}`, distinct from `{"type":"applied_fix", ...}`.

**Rationale**:
- Mirrors the PR 3.7 `AuditNote` precedent at `crates/rules/src/audit_note.rs` (separate NDJSON line type for a distinct audit kind).
- The `discriminant: "strict"|"decoder"` discriminator is about marking provenance, not fix kind — conflating "text correction" as a third discriminant value would semantically misuse the discriminator field.
- Audit consumers gain a cleaner separation: applied_fix records are about marking integrity; text_correction records are about pre-scanner string replacements. Distinct schemas, distinct downstream filters.

**Rejected alternatives**:
- Keep envelope, add Canonical sub-object only to FixIntent branch: AppliedFix shape becomes asymmetric, audit consumers must handle two shapes within one record type.
- Three-value discriminant (strict/decoder/text_correction): conflates marking provenance with fix kind.

### D25.5 — Test-fixture migration strategy at D: atomic via `__engine_promote` sites; T009a inventory is the target list

**Decision**: Every test fixture constructing `AppliedFix` via `__engine_promote` is migrated to the v2 shape atomically in PR 3c.2.D. The Constitution V Principle V test-fixture carve-out comments are preserved at each site. The inventory at `docs/refactor-006/promote-callsite-inventory.md` (per T009a) is the migration target list.

**Rationale**:
- Atomic migration aligns with the schema bump — pre-cutover and post-cutover shapes are unreadable to each other (FR-037 clean break).
- T009a already inventoried all call sites with the three-constraint carve-out scope; no new audit needed.
- No need for an intermediate `AppliedFix::test_fixture(...)` helper layer — `__engine_promote` is the existing entry point and is already path-gated by promote-callsite-lint.

**Rejected alternatives**:
- Test-utility helper crate: adds a maintained layer for marginal benefit.
- Auto-migrate via codemod (rust-analyzer / morphllm): risks pattern-matching errors; per-site review still needed; carve-out comments need human verification.

### D25.6 — Bench gates: informational only (not blocking)

**Decision**: PR 3c.2 series bench measurements are reported but do NOT block merge per FR-033. Each sub-PR runs the bench suite and surfaces deltas in the PR description, but a >5% mean OR p99 regression does not auto-back-out the change.

**Rationale**:
- Bench-gating "came and went with PR 4" per the user's posture: benchmarks are all well below ceiling (SC-001 16ms, SC-002 18ms not violated).
- Marque does not produce correct results without this refactor — correctness debt outranks perf optimization. Integration first; optimize after PR 5 close per `project_perf_baseline_pr5_trigger` memory.
- PR 4b-perf umbrella (#582) and the perf candidate pipeline already track perf regression separately from per-PR gates.
- Constitution I (uncompromising performance) is preserved by the SC-001/SC-002 ceiling — bench-gate enforcement is a discipline mechanism for PR-to-PR drift, not a constitutional requirement.

**Rejected alternatives**:
- Strict per-PR enforcement: would block the 3c.2 series on perf drift that the user has already classified as deferred to post-PR-5 dedicated perf work.
- Bench-gate enforcement at D only: compromise position; D's blast radius is already high, additional bench discipline at D doesn't catch regressions introduced by A/B/C.

### D25.7 — WASM size budget: ≤5% delta, measured at D

**Decision**: PR 3c.2 series WASM size delta target is ≤5%, measured at the D atomic cutover. The `blake3` workspace dep (added at A) is the dominant size contributor (~50–80 KB compressed estimate).

**Rationale**:
- Mirrors T058i's PR 3d WASM size budget per Constitution III WASM-safety discipline.
- WASM size is a Constitution III concern (preserving browser-extension / Office-add-in distribution channel viability).
- Measuring at D rather than per-sub-PR keeps the discipline overhead reasonable while still gating the cumulative delta.

**Rejected alternatives**:
- Per-sub-PR WASM size measurement: most sub-PRs (B, C, E) don't materially change WASM size; measurement overhead exceeds signal.
- No WASM size budget: violates Constitution III "WASM binary size MUST be considered" surface.

---

## 3. Constitution check

| Principle | Check | Result |
|---|---|---|
| I (Uncompromising Performance) | SC-001 16ms ceiling preserved; bench drift reported but not blocking per D25.6 | PASS |
| II (Zero-Copy) | `original_digest: Blake3Hash` replaces byte-content storage; no new heap allocation on the hot path | PASS |
| III (WASM-Safe) | `blake3` confirmed WASM-compatible (Tech Stack table); size budget set per D25.7; `Citation` ships data-only per D25.2 | PASS |
| IV (Two-Layer Rule Architecture) | No Layer 1 / Layer 2 boundary change; `MarkingScheme::canonicalize` is a trait-surface extension on Layer 2 trait, not a Layer 1 change | PASS |
| V (Audit-First Compliance) | G13 becomes type invariant via T055 canary at 3c.2.D; `Diagnostic.message: Message` makes `format!` of input bytes unrepresentable per FR-003 | PASS |
| VI (Dataflow Pipeline Model) | No phase change; `MarkingScheme::canonicalize` slots between parser output and rule input at the scheme layer | PASS |
| VII (Crate Discipline) | Sub-PR boundaries respect crate graph: A touches `crates/scheme/` + `crates/rules/`; B touches consumers across `crates/{engine,core,wasm,ism,rules}/`; C touches `crates/{capco,rules,engine}/`; D touches `crates/engine/` audit emit; E touches `crates/ism/`. No new circular deps. | PASS |
| VIII (Authoritative Source Fidelity) | `Citation` struct gives every diagnostic a typed §-reference that citation-lint can verify against `crates/capco/docs/CAPCO-2016.md`; propagation rule preserved | PASS |

---

## 4. Risk register

### R-1: `MarkingScheme::canonicalize` default impl recursion

The default `canonicalize` impl in PR 3c.2.A delegates to `from_parsed_unchecked` (preserves behavior). PR 3c.2.E deletes `from_parsed_unchecked`. Between B (call-site migration to `canonicalize`) and E (adapter deletion), the default impl points to a function that's about to vanish.

**Mitigation**: In PR 3c.2.B, override `canonicalize` on `CapcoScheme` with the real implementation (not the default delegation). The default impl on the trait is for schemes that haven't migrated; for CAPCO specifically, the override is the production path from B forward. E then deletes the adapter and the trait-level default impl is updated to whatever the new sole-path expression is (or removed entirely if no scheme uses it).

### R-2: `Diagnostic.message: Message` field-type change is a breaking change for any external consumer

Marque is pre-users — no external consumers exist. Per `feedback_pre_users_no_deprecation_phasing.md`, breaking changes are fine.

**Mitigation**: None needed. Internal call sites are migrated atomically in PR 3c.2.C.

### R-3: BLAKE3 hashing overhead on audit-emit hot path

Real `blake3::hash(...)` calls at audit-emit boundary are a new CPU cost. BLAKE3 is SIMD-accelerated and fast, but per-fix hashing is still nonzero work.

**Mitigation**: Bench measurements at PR 3c.2.D report `lint_10kb` / `fix_10kb` deltas. Per D25.6 the deltas are informational; per D25.7 WASM size is gated separately. If `fix_throughput` regresses materially, defer to post-PR-5 perf-analysis pass per `project_perf_baseline_pr5_trigger`.

### R-4: `__engine_promote` test-fixture migration scope underestimate

T009a's `docs/refactor-006/promote-callsite-inventory.md` was authored at PR 0; the file count may have drifted.

**Mitigation**: First action in PR 3c.2.D implementation is to re-grep `__engine_promote` call sites and reconcile against the T009a inventory. Drift becomes a PR-internal bookkeeping item, not a blocker.

### R-5: Audit-record contract documentation drift

`contracts/audit-record.md` currently has §0 (active `marque-mvp-3`) + §1+ (post-keystone `marque-1.0` target). At PR 3c.2.D, §0 retires and §1+ becomes the active spec.

**Mitigation**: Doc edit is part of PR 3c.2.D's scope (per the table in §1). Reviewer must verify the §0/§1 transition lands cleanly with no contradictory passages remaining.

### R-6: 2-tuple `RuleId` form leakage from `contracts/audit-record.md`

`contracts/audit-record.md` §1+ documents the post-keystone target with 2-tuple `RuleId` JSON shape. PR 3c.2 explicitly excludes the 2-tuple migration per FR-049. If a reviewer or implementer follows the §1+ shape literally, they may try to land the 2-tuple change.

**Mitigation**: PR 3c.2.D's contract update must (a) flip §0 active schema to `marque-1.0`, (b) carry forward §1+'s 4-commitment changes (Canonical sub-object + BLAKE3 + Message + canonical/discriminant), (c) preserve the 1-tuple `RuleId` string form, (d) keep the §"Post-`marque-1.0` RuleId migration" section as a forward-looking note. PM contract: 1-tuple form is `marque-1.0`'s wire shape.

---

## 5. Reviewer attestation checklist

For each sub-PR's 3-reviewer pass:

- [ ] CAPCO §-citations in `§X.Y pNN` form only — no bare `§NN`, no `file:line` anchors
- [ ] Every §-citation re-verified against `crates/capco/docs/CAPCO-2016.md` at point of authorship (Constitution VIII propagation rule)
- [ ] Adjacent code paths walked — if a fix surfaces in one place, check related callsites for the same issue
- [ ] Constitution VII crate boundary preserved — see §3 Crate Discipline row for the per-sub-PR boundary
- [ ] Constitution V Principle V — no production wire-up of `__engine_promote`; test-fixture carve-out comments preserved
- [ ] Bench delta reported (not blocking per D25.6); WASM size delta reported at D against ≤5% budget per D25.7
- [ ] "Will we want to maintain this for 5 years?" durability standard

---

## 6. Out-of-scope items (explicit non-coverage)

These items are NOT addressed in PR 3c.2 and remain open:

| Item | Reason | Disposition |
|---|---|---|
| 2-tuple `(scheme, predicate_id)` `RuleId` form | Post-PR-10 per FR-049 (stability-freeze rule) | Tracked in `contracts/audit-record.md` §"Post-`marque-1.0` RuleId migration" |
| R001/R002 sentinel `"engine"` scheme labels | Same post-PR-10 PR as the 2-tuple migration | Same |
| `MarkingScheme::evaluate_custom` ctx extension | Tracked in `specs/006-engine-rule-refactor/followups/constraint-context-extension.md` | Out of refactor scope; gated on `render_canonical` and admonition emitter |
| Admonition channel (S005 split) | Tracked in `specs/006-engine-rule-refactor/followups/admonition-channel.md` | Deferred from PR 3c; not blocking |
| `T119c` #307 Group C Table 3 rollup gaps | Independent declarative `PageRewrite` work | Separate PR; can land in parallel with 3c.2 |
| PR 10 F.1 corpus-fidelity maturation | Depends on PR 3c.2 + PR 9 landed | Sequential after 3c.2 |
| Cumulative perf regression analysis (PRs 4–6) | Tracked in PR 4b-perf umbrella #582 | Dedicated post-PR-5 perf-analysis pass per `project_perf_baseline_pr5_trigger` |

---

## 7. Predecessor / successor relationships

**Predecessors** (must be merged before PR 3c.2.A starts):

- PR 3c.B Commit 10 (#398) — `marque-mvp-2 → marque-mvp-3` bump; `FixIntent | TextCorrection` envelope landed. ✅ MERGED.
- PR 3c.1 (#404 or equivalent) — `Canonical<S>` type + `Message`/`MessageTemplate`/`MessageArgs` types + sealing patterns. ✅ MERGED.
- PR 3d (vocabulary `FormSet`) — `Vocabulary<S>::forms()` accessor + per-token `banner_title` / `banner_abbreviation` distinction. ✅ MERGED (per tasks.md audit summary: "PRs 0/0.5/0.6/1/2/3a/3b/3c.1/3d/...").
- PR 7c — `FeatureId::PrecedingFixPenalty` retirement, dead-code consumer removed. ✅ MERGED.

**Successors** (gated on PR 3c.2 complete):

- PR 10 — F.1 corpus-fidelity maturation depends on the `marque-1.0` schema being live.
- Future 2-tuple `RuleId` migration PR — defers per FR-049 to post-PR-10.

**Parallel-safe** (no ordering dependency):

- T119c #307 Group C Table 3 rollup PageRewrites.
- T119a / T119b #307 predicate catalog completion.
- PR 5 follow-up #261 deferral (post-PR-3c.B render audit-trail re-evaluation; task #47 pending).

---

## 8. Implementation cadence

Per the user's standing process discipline:

1. **Preflight** (architect + rust-specialist in parallel) — surface decision points and tactical plan
2. **PM decisions** — resolve preflight findings; brief implementation agent
3. **Implementation** — agent works the plan; receives full `crates/capco/CAPCO-CONTEXT.md` (not linked) + standing constraints brief + lattice-consultant skill if any work touches lattice surfaces (unlikely for 3c.2 specifically)
4. **3-reviewer pass** — rust-reviewer + code-reviewer + (architect for A, lattice-consultant if any cycle adjacent to lattice work, else code-reviewer alternate)
5. **Submit PR + monitor** — armed persistent monitor; typical 2–5 Copilot rounds; suppressed Copilot comments (low-confidence) historically high signal

Bench gates run on each sub-PR; results reported in PR description; not blocking per D25.6.

Standing constraints carried in every implementation brief:

- No force-push without explicit user authorization
- No `--no-gpg-sign` / `--no-verify`
- All PRs against `staging`
- >80% test coverage if no explicit test PR; CodeCov deny triggers test suite expansion
- "Will we maintain this for 5 years?" durability standard
- Walk adjacent code paths — if a fix surfaces in one place, check callsite/related logic for the same issue

---

## 9. Open questions deferred to preflight

The following are not pre-resolved here; the architect + rust-specialist preflight for PR 3c.2.A should surface tactical decisions:

- **OQ-1**: Where does the `MarkingScheme::canonicalize` default impl live — in `crates/scheme/src/scheme.rs` trait definition (visible to all schemes) or only as an override on `impl MarkingScheme for CapcoScheme`?
- **OQ-2**: Is `RenderContext::schema_version` typed (`enum SchemaVersionId { Mvp3, V1_0 }`) or `&'static str`? The const evaluation surface differs.
- **OQ-3**: Does `Citation` live in `marque-rules` (alongside `Diagnostic`) or `marque-scheme` (so non-CAPCO schemes can express it without depending on `marque-rules`)? Default recommendation: `marque-rules`.
- **OQ-4**: Test-fixture migration in D — single mechanical pass or batched by test-suite (CAPCO tests, engine tests, core tests)?
- **OQ-5**: For the `discriminant: "strict"|"decoder"` field — does decoder-recognized output flow through a different code path in `Engine::fix_inner`, or is it a runtime check on the active recognizer? Implications for the audit-emit boundary.
- **OQ-6**: WASM build verification for `blake3` — is `wasm32-unknown-unknown` covered by the existing CI matrix, or does PR 3c.2.A need to add an explicit build job?

These are tactical-implementation decisions, not PM-level decisions; preflight agents have authority to resolve and brief back.

---

## Appendix A — Task ID cross-reference

Surviving tasks in `specs/006-engine-rule-refactor/tasks.md` mapped to sub-PRs:

| Task | Description | Sub-PR | Status today |
|---|---|---|---|
| T041 | Reshape `AppliedFix` v2 | 3c.2.D | PARTIAL (mvp-3 shape landed; full v2 deferred) |
| T042 | Reshape `Diagnostic` v2 | 3c.2.C (Message/Citation) + 3c.2.D (downstream) | PARTIAL (`Diagnostic<S>` reshape landed; Message/Citation types deferred) |
| T043 | Define `Citation::new` struct | 3c.2.A (definition) + 3c.2.C (rule migration) | NOT STARTED |
| T046 | Migrate diagnostic message construction to `Message::new` | 3c.2.C | PARTIAL (`FixIntent.message` uses MessageTemplate; `Diagnostic.message` still `format!`-built) |
| T048 | `render_canonical` signature update | 3c.2.A | PARTIAL (body landed; signature change deferred) |
| T048a | Define `RenderContext` + `EmissionForm` | 3c.2.A | NOT STARTED |
| T048b | Wire `RenderContext` into `Engine::fix_inner` | 3c.2.A | NOT STARTED |
| T048c | EmissionForm selector tests | 3c.2.A | NOT STARTED |
| T050 | Delete engine.rs:1389 decoder `format!` | 3c.2.C | PARTIAL (span numerals only leak channel closed) |
| T052 | Bump `MARQUE_AUDIT_SCHEMA` | 3c.2.D | mvp-3 landed; 1.0 cutover pending |
| T053 | Reserved-slot baking | (already landed in mvp-3) | DONE |
| T054 | Delete `from_parsed_unchecked` | 3c.2.E (B does migration; E deletes adapter) | NOT STARTED |
| T055 | NDJSON canary scan | 3c.2.D | NOT STARTED |

---

## Appendix B — Audit-record JSON shape (PR 3c.2.D target)

Per `contracts/audit-record.md` §1, the post-cutover NDJSON record shape:

```jsonc
{
  "schema": "marque-1.0",
  "rule": "E054",                            // 1-tuple form per FR-049 (2-tuple defers post-PR-10)
  "severity": "error",
  "span": { "start": 1024, "end": 1037 },
  "fix": {
    "replacement": {
      "discriminant": "strict",              // "strict" | "decoder"
      "canonical": {
        "source": "cve",                     // "cve" | "open_vocab"
        "token_id": "Classification.Secret", // when source = "cve"
        // OR (when source = "open_vocab"):
        // "category": "SciCompartment",
        // "render_call_site": "marque-capco/src/render.rs:142",
        "bytes_digest": "blake3:0e2c..."     // BLAKE3 of rendered bytes; bytes themselves never in record
      },
      "confidence": {
        "recognition": 0.95,
        "rule": 1.00,
        "combined": 0.95,
        "region": null,
        "runner_up_ratio": null,
        "features": ["StrictExactMatch"]
      }
    },
    "original_span": { "start": 1024, "end": 1037 },
    "original_digest": "blake3:b78f..."      // BLAKE3 of pre-fix bytes
  },
  "message": {
    "template": "BannerMissingClassification",
    "args": {
      "expected_token": "Classification.Secret",
      "category": "Classification"
      // closed-set scalar/ID types only
    }
  },
  "timestamp": "2026-05-19T14:32:11Z",
  "classifier_id": "12345",
  "dry_run": false
}
```

Non-marking text correction record (separate line type per D25.4):

```jsonc
{
  "type": "text_correction",
  "schema": "marque-1.0",
  "span": { "start": 234, "end": 240 },
  "original_digest": "blake3:c3a1...",
  "replacement": "SECRET",
  "source": "corrections_map",
  "timestamp": "2026-05-19T14:32:12Z",
  "classifier_id": "12345",
  "dry_run": false
}
```

(Exact `text_correction` field shape TBD at PR 3c.2.D implementation; the above is illustrative.)
