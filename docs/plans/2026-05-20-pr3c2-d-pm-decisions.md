<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 3c.2.D — PM Decisions

**Date**: 2026-05-20
**Branch**: `refactor-006-pr-3c2-d-atomic-cutover` (off `staging@fae9e334`)
**Status**: LOCKED — PM contract; implementation agents act on this.

**Master plan**: `docs/plans/2026-05-19-pr3c2-plan-and-decisions.md` (D25.1–D25.7, OQ-1..OQ-6)
**Preflights**:
- Architect: `docs/plans/2026-05-20-pr3c2-d-architect-preflight.md` (D-D-1..D-D-10, R-D-1..R-D-7)
- Rust: `docs/plans/2026-05-20-pr3c2-d-rust-preflight.md` (type defs, serde shape, crate-graph, compile-fail doctests, OQ-1..OQ-6)

---

## 0. Scope confirmation

PR 3c.2.D is the atomic schema cutover `marque-mvp-3 → marque-1.0`. Single PR
against `staging` with 11 commits per architect's decomposition (D0 → D10),
atomic flip at D7. Scope per master plan §1 row D and FR-035a's four
structural commitments:

1. `Canonical<S>` provenance wired into audit emit
2. BLAKE3 audit-record digesting (real `blake3::hash(...)`)
3. Closed-set `MessageTemplate` JSON in audit output
4. `AppliedFix` v2 reshape + `AppliedTextCorrection` split

Plus implied prereqs: `Vocabulary<S>::qualified_token_label` accessor (NEW
scope — see PM-D-10), T055 canary, `marque --version` schema discoverability,
`#257` masking pin retirement, contract docs §0 retirement.

---

## 1. PM Decisions

Numbered `PM-D-N` to fold into the master plan's D25 register.

### PM-D-1 — Sub-PR shape: single PR, 11 commits, atomic flip at D7

**Decision**: RATIFY architect's D0→D10 decomposition (preflight §1). Single
PR against `staging`. Atomic flip at D7. D6 may split into D6a/D6b on the
implementation agent's discretion per architect D-D-8 (consistent with the
user's standing quality-over-doggedness directive).

**Implementation cadence**: dispatch the work across sequential
focused agents to preserve the quality-over-single-agent directive. Proposed
batching: agent 1 (D0–D2 foundation), agent 2 (D3 engine), agent 3 (D4–D5
renderers), agent 4 (D6 test fixtures), agent 5 (D7–D10 flip + canary + docs).
Each agent verifies build-green at boundary. PM dispatches with full standing
brief.

---

### PM-D-2 — All v2 types co-locate in `marque-rules`

**Decision**: RATIFY rust preflight §2.1 and architect R-D-4. New types
(`Discriminant`, `AppliedReplacement<S>`, `AppliedFixDetail<S>`,
`AppliedTextCorrection`, `Blake3Hash` alias) land in `marque-rules`
alongside the existing `AppliedFix<S>`.

**Rationale**: `EnginePromotionToken` seal lives in `marque-rules`;
splitting types into `marque-engine` would require cross-crate seal
redesign. `marque-engine` already depends on `marque-rules` — reverse
edge would violate Constitution VII.

**File-level placement**: rust preflight §13 Appendix A notes
`crates/rules/src/lib.rs` is already 1500+ lines; the v2 types may be
extracted into `crates/rules/src/audit.rs` submodule if the file size
exceeds the 800-line norm. Implementation agent decides per house style.

---

### PM-D-3 — `Discriminant` is a closed 2-variant enum

**Decision**: RATIFY architect D-D-3 and rust preflight §2.2.
`Discriminant::Strict | Discriminant::Decoder`. No `#[non_exhaustive]` —
closed-set discipline per `MessageTemplate` precedent. Adding a variant
requires a coordinated audit-schema bump.

**Wire form**: `Discriminant::as_str()` returns `"strict"` / `"decoder"`.
Pinned by a new test at `crates/rules/tests/discriminant_audit_string.rs`.

---

### PM-D-4 — `AppliedTextCorrection` is a separate Rust type

**Decision**: RATIFY rust preflight §2.6 and architect D-D-3 / D-D-7.
Non-marking text corrections live in their own type, NOT as a third
`Discriminant` variant.

**Rationale**:
- Constitution V Principle V clarity: marking records carry
  `Canonical<S>` + token canonicals + BLAKE3; text-correction records
  carry corpus-derived `SmolStr` replacements. The type-level separation
  makes the G13 boundary checkable at compile time.
- `Discriminant::Strict | Decoder` is about marking recognition
  provenance; text corrections sit outside that channel (pre-scanner).
  A `Discriminant::TextCorrection` arm would conflate provenance with
  fix kind.
- NDJSON consumers dispatch on `{"type": "applied_fix"}` vs
  `{"type": "text_correction"}` cheaply.

**Compile-fail proof**: rust preflight §7.5 and §7.6 doctests pin the
disjoint-type and 2-variant-`Discriminant` invariants.

---

### PM-D-5 — `Blake3Hash` is a type alias, not a newtype

**Decision**: RATIFY architect D-D-5 and rust preflight §1.4.
`pub type Blake3Hash = blake3::Hash;` (re-export) lands in
`crates/rules/src/lib.rs` (or `crates/rules/src/digest.rs` per house
style). NOT `pub struct Blake3Hash([u8; 32]);` newtype.

**Wire form**: serialize as `"blake3:<64-hex>"` via
`format!("blake3:{}", hash.to_hex())` at the JSON projection.

**Note**: the existing `Blake3Hash::zero()` ctor on `crates/rules/src/message.rs:87`
is a NEWTYPE today (`pub struct Blake3Hash([u8; 32])`). PR D's PM-D-5
change is a TYPE-LEVEL REPLACEMENT: replace the newtype with the alias.
Existing test consumers (`message.rs:628`, `message_args_closed_set.rs:77`)
update to `blake3::Hash::from_bytes([0u8; 32])` or
`blake3::Hash::from([0u8; 32])`. The `zero()` ctor disappears; tests
use the upstream API directly.

---

### PM-D-6 — Digest storage on `AppliedReplacement` / `AppliedFixDetail`; OVERRIDE architect D-D-6

**Decision**: OVERRIDE architect's D-D-6 recommendation (which proposed
adding `pub fn bytes_digest(&self) -> Blake3Hash` on `Canonical<S>` in
`marque-scheme`). ADOPT rust preflight §10 alternative (which the
architect themselves flagged as the workable fallback in R-D-6).

- `original_digest: Blake3Hash` is a field on `AppliedFixDetail<S>`.
- `bytes_digest: Blake3Hash` is a field on `AppliedReplacement<S>`,
  precomputed at promotion time from `Canonical::bytes()`.
- `blake3` dep is added to `marque-rules` (consumer of the hash type
  + the hashing call). NOT added to `marque-scheme`.

**Rationale for override**:
1. Constitution VII spirit: `marque-scheme` is the graph leaf; keeping
   it free of `blake3` preserves the minimal-dep posture even though
   `blake3` is a runtime util (not a `marque-*` crate). Future schemes
   reusing the trait surface don't inherit a hashing dep.
2. WASM size budget (D25.7, ≤5%): adding `blake3` to `marque-scheme`
   would pull it into more WASM-built crates than necessary. Per-crate
   minimization matters at the size budget.
3. The bug class architect named ("digest of one set of bytes attached
   to a different set of bytes") is mitigated by `AppliedReplacement`
   being engine-promotion-only: the digest and the `Canonical<S>` are
   constructed in the same `__engine_promote` body, from the same
   bytes, in a sealed engine carve-out. No external code path can
   desync them.
4. The architect explicitly flagged the alternative in R-D-6 as
   workable; rust preflight independently arrived at the same
   conclusion.

**Computation site**: at promotion time, inside
`AppliedFix::__engine_promote`. The `original_bytes: &[u8]` parameter
(engine slices `source[span]` and passes them) is hashed inline; the
`canonical.bytes()` is hashed inline. Both digests flow into the
`AppliedFixDetail` constructor and the slices/views drop at end of
the function body. Constitution V Principle V preserved at construction
time.

---

### PM-D-7 — `Discriminant` derived at emit time from `FixSource` (RATIFY architect D-D-9)

**Decision**: RATIFY architect D-D-9. The `Discriminant` field is
populated at audit-emit time by deriving from `AppliedFix.source: FixSource`
via the 5-to-2 mapping in architect D-D-3:

| `FixSource` | `Discriminant` |
|---|---|
| `BuiltinRule` | `Strict` |
| `MigrationTable` | `Strict` |
| `DecoderPosterior` | `Decoder` |
| `DecoderClassificationHeuristic` | `Decoder` |
| `CorrectionsMap` | (routes to `AppliedTextCorrection`, not `Discriminant`) |

**OVERRIDE rust preflight's OQ-5 resolution** (which proposed threading
`Discriminant` through `KeptFix` from the recognizer dispatch site).

**Rationale for override**:
- The mapping is a pure function `fn discriminant_from_source(s: FixSource) -> Discriminant`.
  No data flows from the recognizer that isn't already on `FixSource`.
- Threading a new field through every promote-call argument list adds
  noise without adding signal.
- The architect's approach is the simpler implementation; rust
  preflight's threading approach is reserved for cases where the
  upstream data isn't already available.

**Implementation site**: a `fn discriminant(&self) -> Discriminant` helper on
`AppliedFix<S>` that maps `self.source` per the table above. The
helper is called at audit-emit-time JSON projection, not stored as a
field on `AppliedReplacement`.

**Schema impact**: `AppliedReplacement<S>` does NOT carry a
`discriminant: Discriminant` field. The JSON projection emits the
discriminant string at the wire-format boundary. This reduces
`AppliedReplacement` to `{ canonical: Canonical<S>, confidence: Confidence,
bytes_digest: Blake3Hash }` — three fields, all data-bearing.

**Doctest update**: rust preflight §2.3's `AppliedReplacement` definition
listing `pub discriminant: Discriminant` is OVERRIDDEN here — the field
drops. Compile-fail doctest §7.6 (no `Discriminant::TextCorrection`
variant) stays.

---

### PM-D-8 — `AuditLine<S>` sum type for cross-record promotion order (RATIFY architect D-D-7)

**Decision**: RATIFY architect D-D-7. Add a new sum type in `marque-rules`:

```rust
#[non_exhaustive]
#[derive(Debug)]
pub enum AuditLine<S: MarkingScheme> {
    AppliedFix(AppliedFix<S>),
    TextCorrection(AppliedTextCorrection),
}
```

`FixResult.applied: Vec<AppliedFix<S>>` becomes
`FixResult.audit_lines: Vec<AuditLine<S>>`. Emit dispatch iterates the
stream and routes each line to its NDJSON projection. NDJSON wire
format preserves the line-per-record property (both arms emit one line).

**Rationale**: preserves promotion order across the two record types
(FR-016 invariant generalized). Single emit loop. Timestamp resolution
collisions would otherwise force consumers to merge with stale tie-breaking.

**Manual Clone**: same `S: MarkingScheme` (not `S: Clone`) discipline
as the rest of the v2 types (rust preflight §2.5).

---

### PM-D-9 — `_legacy`-suffix rename window during D2-D7 is permitted; NO `#[deprecated]` attribute

**Decision**: RATIFY architect D2's tactical `_legacy`-suffix rename of
v1 constructors during the D2-D7 window. OVERRIDE architect's R-D-5
proposal to attach `#[deprecated]` attribute — `feedback_pre_users_no_deprecation_phasing.md`
applies (marque is pre-users; rename is internal mechanical bookkeeping,
not user-facing deprecation).

**Mechanics**:
- D2 renames existing `__engine_promote` → `__engine_promote_legacy`
  (mechanically, not via `#[deprecated]`); adds NEW v2 `__engine_promote`
  alongside. `cargo check --workspace` stays green between D2-D7.
- D7 deletes `__engine_promote_legacy` atomically with schema flip.
- FR-040 lint (`tools/promote-callsite-lint/`) matches reserved
  names on **exact-equality of the last path segment**, not on
  prefix containment. The closed reserved-name list at HEAD is
  `__engine_promote`, `__engine_promote_text_correction` (added in
  PR 3c.2.D fixup F-1 for the PM-D-4 text-correction split), and
  `__engine_construct`. The back-compat name
  `__engine_promote_legacy` is a distinct identifier and is
  deliberately NOT covered — see
  `engine_promote_legacy_is_not_caught_by_suffix_match` in the
  lint test suite for the pin. Adding a fourth reserved name
  requires an explicit edit to the matcher AND a corresponding
  test case (mirror the F-1 pattern).

**Alternative considered (rejected)**: outright deletion of v1
constructors at D2 (rust preflight's preference). REJECTED because
keeping `cargo check --workspace` informative at each commit boundary
is normal review-quality discipline; the `_legacy` rename is the
tactical device that makes per-commit review tractable. The rename
adds ~20 LOC of mechanical churn; review-cycle savings vastly outweigh.

---

### PM-D-10 — `Vocabulary<S>::qualified_token_label` accessor: ADD it (NEW SCOPE)

**Decision**: ADD the accessor in `marque-scheme` per rust preflight
§13's option (a). Implementation footprint: ~50 LOC in `marque-scheme`
(new trait method on `Vocabulary<S>` with default impl that constructs
the namespaced form from `category_label(token.category()) + "." +
token.name()`) + per-token-table build.rs entry in `marque-ism` (if
the existing per-token tables don't already carry the constituent
parts).

**Rationale**:
- The audit-record contract per `contracts/audit-record.md` body §
  specifies `"token_id": "Classification.Secret"` — the namespaced
  form. Falling back to numeric `TokenId(N)` debug form would force
  audit consumers to resolve via separate vocabulary lookup, violating
  the per-record self-describing property the contract pins.
- This is part of FR-035a's first commitment (Canonical provenance
  wired into audit emit). The accessor is the resolution surface that
  makes the canonical-provenance namespacing observable in the audit.
- Architect didn't surface this; rust preflight caught it. Architect's
  plan is otherwise sound; this is an additive scope to FR-035a's
  cargo, not a re-scoping.

**Placement**: trait method on `Vocabulary<S>` (already in
`marque-scheme`) with a default impl that builds the
`"Category.Token"` string from existing accessors. Specific schemes
override only if their category/token names need custom formatting.
For `CapcoScheme`, the default impl suffices.

**Commit-level**: lands in D2 alongside the rest of the new types
(it's a precursor for D3's engine wiring — the engine needs the
accessor to construct `AppliedReplacement.canonical.token_id` JSON
field).

**Out-of-scope clarification**: the accessor is a wire-format
projection helper, not a new lattice surface. It does NOT extend
`MarkingScheme` or `Lattice`. Constitution IV / VII preserved.

---

### PM-D-11 — `AppliedFix<S>` v2 field set (RATIFY rust preflight §2.5)

**Decision**: RATIFY rust preflight §2.5's field set for `AppliedFix<S>`
v2:

**Added vs v1**:
- `severity: Severity` (top level — contract §107-178 emits `"severity": "..."` at top level)
- `message: Message` (top level — contract emits `"message": {...}` at top level)

**Removed vs v1**:
- `proposal: AppliedFixProposal<S>` (replaced by `fix: AppliedFixDetail<S>` on marking path; text corrections move to `AppliedTextCorrection` per PM-D-4)
- `confidence: Confidence` (moved to `fix.replacement.confidence`)
- `migration_ref: Option<&'static str>` (deleted — superseded by typed `Citation` on `Diagnostic`; audit-record contract per §168-171 does not emit a top-level `migration_ref`)

**Retained**:
- `rule: RuleId`, `span: Span`, `source: FixSource`, `timestamp: SystemTime`,
  `classifier_id: Option<Arc<str>>`, `dry_run: bool`, `input: Option<Arc<str>>`.

**`#[non_exhaustive]`**: kept.

**Manual `Clone`**: kept (`S: MarkingScheme` not `S: Clone` discipline).

---

### PM-D-12 — `MessageTemplate::as_str()` is the wire form; contract example is illustrative

**Decision**: RATIFY rust preflight §3.5. `MessageTemplate::as_str()`
returns the variant name verbatim (e.g.,
`MessageTemplate::BannerRollupMismatch.as_str() == "BannerRollupMismatch"`).
The contract example showing `"BannerMissingClassification"` is
illustrative — the existing enum variant ships as the wire form.

**Implication for `contracts/audit-record.md`**: D10's contract-doc edit
clarifies that the `template` JSON value is the Rust enum variant name
verbatim, and replaces the illustrative variant in the JSON sample with
an actual current variant (e.g., `BannerRollupMismatch` or another
shipping name). The renaming-the-enum-to-match-contract path is out of
scope.

---

### PM-D-13 — T055 canary file location: `crates/engine/tests/audit_g13_canary.rs`

**Decision**: RATIFY rust preflight §12's file name. T055 lands at
`crates/engine/tests/audit_g13_canary.rs` (not `canary_scan.rs` —
the `g13` namespace anchors the canary's purpose).

**Implementation**: deterministic scan per architect §6 (D8 commit
notes). JSON-aware exclusion per architect R-D-3 (parse NDJSON into
`serde_json::Value`, walk the tree, only check string-valued leaves
and numeric leaves outside span fields). Two assertions:

1. Positive (zero leaks on HEAD post-D): corpus sweep emits no `≥4-byte`
   input sequence outside the permitted-identifier list.
2. Negative (synthetic regression fires the canary): a fabricated
   `Message::new(Template::X, MessageArgs { /* with input bytes leaked */ })`
   causes the canary to detect the violation. Test-fixture carve-out
   per Constitution V Principle V; the regression test fabricates an
   AppliedFix using `__engine_promote` from inside the canary's own
   `#[cfg(test)]` module.

**Corpus inputs**: `tests/corpus/{valid,mangled,prose,prose-positive,lattice}/`
per architect §6 D8 spec. Same corpus the existing accuracy harness
uses; no new corpus required.

---

### PM-D-14 — `#257` masking pin retirement at D8 (architect's commit decomposition)

**Decision**: RATIFY architect's commit decomposition. `crates/engine/tests/core_error_isolation.rs:92`
masking pin retires in D8, atomically with T055 canary green. The
canary's positive assertion structurally closes the channel #257
was masking.

**Validation**: D8 reviewer attestation verifies
`crates/engine/tests/core_error_isolation.rs` no longer contains
the strict-recognizer pin nor any equivalent. `grep -n '257\|decoder.*proposal.replacement' crates/engine/tests/`
returns zero hits.

---

### PM-D-15 — `marque --version` schema discoverability surface

**Decision**: RATIFY architect's D9 commit. `marque --version` output
adds `audit_schema: <AUDIT_SCHEMA_VERSION>` line per contract §"Schema
discoverability (D3)" §415-446.

**Integration test**: new `marque/tests/cli_version.rs` pins:

```bash
marque --version | grep "^audit_schema:"
# returns: audit_schema: marque-1.0
```

**Concurrent cleanup**: rust preflight §6.3 notes
`marque/src/main.rs:101-102` carries stale `--version` help text
referencing `mvp-1 | mvp-2`. Update to `marque-1.0` (single-value
accept-list).

---

### PM-D-16 — Architect's R-D-1 through R-D-7 risk register: RATIFY all mitigations

**Decision**: All risks in architect's §3 risk register accepted with
their stated mitigations:

- **R-D-1** (WASM size delta): measure at D5, report in PR description.
  If >5%, escalate to PM with measurement evidence; fallback options
  in priority order per architect §3.
- **R-D-2** (audit-stream consumer surprise on text_correction split):
  no external consumers (per `decisions.md` D4); internal consumers
  catalogued and migrated.
- **R-D-3** (T055 canary false positives on span integer overlap):
  JSON-aware exclusion per architect §3.
- **R-D-4** (`AppliedFixDetail` placement): in `marque-rules` per
  PM-D-2.
- **R-D-5** (schema atomicity vs `_legacy` constructor window):
  `_legacy` rename per PM-D-9; window closes at D7.
- **R-D-6** (`Canonical::bytes_digest()` introduction → blake3 in
  marque-scheme): SUPERSEDED by PM-D-6 (digest stored on
  `AppliedReplacement`, blake3 NOT added to marque-scheme).
- **R-D-7** (test fixture path drift): re-grep before D2 implementation;
  the rust preflight §1 inventory at HEAD is the authoritative count.

---

## 2. Implementation dispatch plan

Per the master plan §8 and the user's standing quality-over-doggedness
directive, PR D's 11 commits dispatch across **5 sequential
implementation agents**, each carrying full standing brief +
`crates/capco/CAPCO-CONTEXT.md` (the Vocabulary accessor in D2 touches
CAPCO-adjacent build.rs tables). Each agent verifies build-green at
boundary; PM reviews briefly before dispatching the next.

| Agent | Commits | Scope |
|---|---|---|
| **D-A1** | D0–D2 | Housekeeping → blake3 wire (Cargo.toml) → new v2 types (`Discriminant`, `AppliedReplacement<S>`, `AppliedFixDetail<S>`, `AppliedTextCorrection`, `AuditLine<S>`, `Blake3Hash` alias) + `Vocabulary<S>::qualified_token_label` accessor (PM-D-10) + `SchemaVersionId::V1_0` + `_legacy`-suffix rename of v1 constructors (PM-D-9). Compile-fail doctests (§7 of rust preflight, 7 total). Compile clean at end. |
| **D-A2** | D3 | Engine emit migration: `engine.rs:2189` (text-correction → `AppliedTextCorrection`), `engine.rs:3177` (marking → v2 `AppliedFix`), `Canonical<S>` wired, `RenderContext` constructed per T048b, `original_bytes` + `bytes_digest` computed at promotion time, `FixResult.audit_lines: Vec<AuditLine<S>>` field added. |
| **D-A3** | D4–D5 | CLI renderer (`marque/src/render.rs`) + WASM renderer (`crates/wasm/src/lib.rs`). New JSON projection structs: `AuditRecordJsonV1_0`, `FixJson`, `ReplacementJson`, `CanonicalJson`, `TextCorrectionRecordJson`. Discriminant emit-time derivation per PM-D-7. SC-008 parity test at `crates/wasm/tests/audit_v1_0_parity.rs`. |
| **D-A4** | D6 | Test-fixture migration: 4 sites per architect §1 + rust preflight §5. Each site preserves carve-out comments verbatim. Split into D6a/D6b on agent discretion per PM-D-1. |
| **D-A5** | D7–D10 | Atomic schema flip (`build.rs` accept-list + `AUDIT_SCHEMA_IS_V3` → `_V1_0`) + delete `_legacy` constructors + T055 canary at `audit_g13_canary.rs` + `#257` pin retirement + `marque --version` surface + contract docs edit + CHANGELOG. |

**Agent brief contents** (delivered verbatim to each):
- This PM decisions doc (full)
- The architect preflight (full)
- The rust preflight (full)
- The master plan §1 + §4 + §5 + §8
- `contracts/audit-record.md` (full body)
- Standing brief constraints (full)
- For D-A1 and D-A5 only: `crates/capco/CAPCO-CONTEXT.md` (full content, not link) — D-A1's Vocabulary accessor touches CAPCO-adjacent tables; D-A5's canary corpus sweep runs against CAPCO-marked fixtures.

---

## 3. Constitution check (extends architect §4)

| Principle | PR-D-specific gate | Result |
|---|---|---|
| **I (Performance)** | PM-D-6 keeps digest computation at promotion-time inside `AppliedFix::__engine_promote`. Emit-loop cost constant. SC-001 16ms ceiling preserved. D5 WASM size measurement per PM-D-16/R-D-1. | PASS (conditional on D5 size measurement) |
| **II (Zero-Copy)** | `original_digest` replaces byte-content storage. `Blake3Hash` is `Copy` (32-byte struct alias). `original_bytes: &[u8]` passes through `__engine_promote` body only — never stored. Constitution II "wipe on drop" preserved on `FixResult.source: SecretSlice<u8>`. | PASS |
| **III (WASM Safety)** | `blake3 ["pure"]` confirmed WASM-compat. PM-D-6 keeps blake3 out of `marque-scheme` (preserves leaf-crate minimalism). PM-D-7 keeps `Discriminant` derivation pure-functional (no runtime dispatch). WASM-runtime-config invariant preserved (no new runtime audit field config). | PASS |
| **IV (Two-Layer Architecture)** | PM-D-10 adds `Vocabulary<S>::qualified_token_label` accessor as a wire-format projection helper, NOT a new Layer 1/2 boundary. Default impl handles `CapcoScheme` (no per-rule override required). | PASS |
| **V (Audit-First Compliance)** | **The principle's central concern.** G13 becomes a type+canary invariant: PM-D-4 makes `AppliedFix` vs `AppliedTextCorrection` disjoint by construction; PM-D-13's T055 canary verifies no input leaks at corpus scale. `__engine_promote` carve-out comments preserved at all 4 fixture sites; `_legacy` rename window (PM-D-9) closes at D7. #257 masking pin retires concurrently with canary green (PM-D-14). | PASS |
| **VI (Dataflow Pipeline)** | No phase change. `RenderContext` slots into existing promotion path; `AuditLine<S>` sum type preserves promotion order (PM-D-8). | PASS |
| **VII (Crate Discipline)** | `blake3` added to `marque-rules` + `marque-engine` only (PM-D-6 keeps it OUT of `marque-scheme`). `marque-scheme` gains `SchemaVersionId::V1_0` variant + `Vocabulary::qualified_token_label` accessor — both are within the scheme-trait-surface scope, no new external dep. Constitution VII §IV within-006 precedent: PR D's engine-crate touches are bug-fix + signature changes at established promotion sites; no new scheme adopted. | PASS |
| **VIII (Authoritative Source Fidelity)** | No CAPCO §-citations migrated in PR D's primary work. `Citation` stays on `Diagnostic` (lint phase), not on `AppliedFix` (audit phase) per contract §168-171. Mechanical citation-lint continues at compile time. D10 contract-doc edit re-verifies any §-citation moving between contract sections per Constitution VIII propagation rule. | PASS |

---

## 4. Reviewer attestation checklist (extends architect §5)

Per-commit reviewer attestation:

- [ ] CAPCO §-citations in `§X.Y pNN` form only — no bare `§NN`, no `file:line` anchors
- [ ] Adjacent code paths walked — CLI + WASM emitters migrated symmetrically (PM-D-1)
- [ ] Constitution VII crate boundary preserved — verify `blake3` did NOT land in `marque-scheme` per PM-D-6
- [ ] Constitution V Principle V — no production `__engine_promote` wire-up outside `Engine::fix_inner` / `Engine::apply_text_corrections` / `engine_promotion_token()`; test-fixture carve-out comments preserved at all 4 sites
- [ ] **G13 canary green** — `cargo test --test audit_g13_canary` passes on D8 post-flip; synthetic regression fires the canary
- [ ] **Schema atomicity** — `git log --grep="MARQUE_AUDIT_SCHEMA"` between staging and D7 HEAD shows exactly ONE commit changing the accept-list (D7); no `_legacy` constructors remain in production code post-D7
- [ ] **AUDIT_SCHEMA_VERSION single source of truth** — no hardcoded literal `"marque-1.0"` outside `crates/engine/build.rs` and test fixtures
- [ ] **No audit-stream content channel reopened** — `grep -rn 'format!.*input\|format!.*bytes\|format!.*replacement' crates/engine crates/rules marque/src crates/wasm/src` outside test code returns zero new sites
- [ ] **No 2-tuple `RuleId` leakage** — `rule` field stays string form per master plan R-6
- [ ] **`marque --version` schema discoverability** — `marque --version | grep "^audit_schema:"` returns `audit_schema: marque-1.0` post-D9
- [ ] **WASM binary size delta ≤5%** per D25.7 — measured at D5; if exceeded, R-D-1 mitigation triggered with PM escalation
- [ ] **SC-008 byte-identity** — CLI + WASM emit byte-identical NDJSON; verified by `crates/wasm/tests/audit_v1_0_parity.rs` + existing parity job
- [ ] **`Discriminant::Strict | Decoder` closed** — no `#[non_exhaustive]`; compile-fail doctest §7.6 (rust preflight) green
- [ ] **`Vocabulary<S>::qualified_token_label` accessor green** on `CapcoScheme` — produces `"Classification.Secret"` form for known CVE tokens
- [ ] **`Blake3Hash` is `blake3::Hash` alias** — no newtype wrapper; `Blake3Hash::zero()` ctor removed; existing test consumers updated
- [ ] **AuditLine<S> sum type green** — `FixResult.audit_lines` iterates correctly; both arms emit one NDJSON line each
- [ ] **`AppliedFix.severity` populated** at promotion time from originating `Diagnostic.severity` (FR-008 / D-7.6 invariant preserved)
- [ ] **Compile-fail doctests** (rust preflight §7, 7 total) all rejected by `cargo test --doc`
- [ ] **`AppliedFixProposal<S>` deleted** at D7 — `grep -rn AppliedFixProposal crates --include='*.rs'` returns zero hits post-D7
- [ ] **`migration_ref` removed from AppliedFix** — top-level field deletion per PM-D-11
- [ ] **`confidence` moved to fix.replacement.confidence** per PM-D-11
- [ ] "Will we want to maintain this for 5 years?" durability standard

---

## 5. Out-of-scope (extends master plan §6)

PR D does NOT address:

| Item | Reason | Disposition |
|---|---|---|
| `from_parsed_unchecked` adapter deletion | PR 3c.2.E's scope | Master plan §1 row 5 + tasks.md T054 |
| 2-tuple `RuleId` form | Post-PR-10 per FR-049 | Forward-looking note in contract |
| R001/R002 sentinel `"engine"` scheme labels | Same as 2-tuple | Same |
| `MarkingScheme::evaluate_custom` ctx extension | Tracked in `followups/constraint-context-extension.md` | Out of refactor scope |
| Admonition channel (S005 split) | Deferred from PR 3c | Not blocking |
| Shared `marque-audit-render` crate extraction | Per PM-D-1 / architect D-D-1 | Defers to post-PR-10 |
| `Severity::Suggest` introduction | Already shipped (FR-042) | No D work |
| Renaming `MessageTemplate` variants to match contract example | Per PM-D-12 | Out of scope |
| `Vocabulary<S>` accessor breadth beyond `qualified_token_label` | Future scope as new audit fields land | Tracked here |
| Kleene PR (Constraint::Custom coverage) | Active separate work on staging | Verify on rebase; D does not block |
| Cumulative perf analysis (PRs 4–6) | Tracked in PR 4b-perf umbrella #582 | Post-PR-5 dedicated pass |

---

## 6. Cross-references

- **Master plan**: `docs/plans/2026-05-19-pr3c2-plan-and-decisions.md` (PM contract for the 5-sub-PR series)
- **Architect preflight**: `docs/plans/2026-05-20-pr3c2-d-architect-preflight.md` (11-commit decomposition, risk register, OQ-4/5/6 resolution)
- **Rust preflight**: `docs/plans/2026-05-20-pr3c2-d-rust-preflight.md` (type defs, serde shape, crate-graph impact, compile-fail doctests)
- **Audit-record contract**: `specs/006-engine-rule-refactor/contracts/audit-record.md` (target shape)
- **Promote-callsite inventory at HEAD**: rust preflight §1 (supersedes PR-0 `docs/refactor-006/promote-callsite-inventory.md`)

---

**End of PM decisions.**
