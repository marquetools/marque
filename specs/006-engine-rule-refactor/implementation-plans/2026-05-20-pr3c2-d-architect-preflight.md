# PR 3c.2.D Architect Preflight

> File path: `/home/knitli/marque/docs/plans/2026-05-20-pr3c2-d-architect-preflight.md`
> Author: architect preflight, 2026-05-20
> Branch: `refactor-006-pr3c2-d-atomic-cutover` (off `staging@fae9e334`)
> Status: Preflight contract for PR 3c.2.D implementation. **Not** an implementation plan — implementation agents act on this; PM ratifies or overrides the resolved OQs.
> Master PM contract: `docs/plans/2026-05-19-pr3c2-plan-and-decisions.md` (§1 row 4, D25.4, D25.5, D25.6, D25.7, OQ-4/5/6).
> Predecessor preflights: 3c.2.A `pr3c2-a-pm-decisions.md`; 3c.2.B `pr3c2-b-{architect-preflight,pm-decisions,tactical-plan}.md`; 3c.2.C `pr3c2-c-{architect-preflight,pm-decisions,rust-preflight}.md`.
> Target contract: `specs/006-engine-rule-refactor/contracts/audit-record.md` body §94–446 (the post-keystone `marque-1.0` shape).

---

## 0. Scope verification (against master plan + post-3c.2.C HEAD)

PR 3c.2.D is the **atomic schema-cutover** sub-PR. It is the ONLY sub-PR in the
3c.2 series that changes the audit wire format. Pre-D: `marque-mvp-3`
envelope (FixIntent | TextCorrection discriminated `proposal`). Post-D:
`marque-1.0` envelope (marking-only `AppliedFix.fix: AppliedFixDetail`,
separate `{"type":"text_correction", ...}` line type for non-marking
corrections, BLAKE3 digesting, closed `MessageTemplate` JSON, schema label
flip).

### Verified-at-preflight inventory (`grep` runs 2026-05-20 on HEAD)

| Surface | HEAD state | PR D obligation |
|---|---|---|
| `blake3` workspace dep | **Present** at `Cargo.toml:145` with `default-features = false, features = ["pure"]`. **Zero crate-level pulls.** | Add `blake3 = { workspace = true }` to `crates/rules/Cargo.toml` (D2 lands the type) AND `crates/engine/Cargo.toml` (D3 wires the hot path). |
| `MARQUE_AUDIT_SCHEMA` accept-list | `crates/engine/build.rs:24-25` — `ACCEPTED = &["marque-mvp-3"]`, `DEFAULT = "marque-mvp-3"`. Regression pin at `crates/engine/tests/audit_schema_accept_list.rs`. | Flip ACCEPTED to `&["marque-1.0"]`, DEFAULT to `"marque-1.0"`. Update pin. |
| `AUDIT_SCHEMA_VERSION` const | `crates/engine/src/lib.rs:87` — re-exports `env!("MARQUE_AUDIT_SCHEMA")`. | No code change — the const auto-updates from the env. |
| `AUDIT_SCHEMA_IS_V3` bool | `crates/engine/src/lib.rs:98` — folds `AUDIT_SCHEMA_VERSION == "marque-mvp-3"`. Used only at `marque/src/render.rs:649` as a `let _ =` ignore (dispatch is single-arm today). | Rename → `AUDIT_SCHEMA_IS_V1_0` (or delete — there is no dispatch branch). **Decision below (D-D-3).** |
| `AppliedFixProposal<S>` enum | `crates/rules/src/lib.rs:728-746` — `{ FixIntent(FixIntent<S>), TextCorrection { replacement: SmolStr } }`. | Splits: rule-emitted FixIntent stays on `AppliedFix.fix`; TextCorrection moves to a separate `TextCorrectionRecord` type. |
| `AppliedFix<S>` struct | `crates/rules/src/lib.rs:796-823` — flat shape with `proposal`, top-level `confidence`/`source`/`migration_ref`. | Reshape to per-contract: `fix: AppliedFixDetail { replacement: { discriminant, canonical: Canonical<S> }, original_span: Span, original_digest: Blake3Hash }` + flattened message/severity. |
| `__engine_promote` call sites (production) | 3 sites: `engine.rs:2189` (text-correction promote), `engine.rs:3177` (FixIntent promote), `engine.rs:7632` (test-only synth helper inside `#[cfg(test)]`). | Migrate all three. The `engine.rs:7632` site is `cfg(test)` so falls under the carve-out. |
| `__engine_promote` test-fixture call sites (`AppliedFix`) | 3 sites: `crates/rules/tests/engine_promotion_seal.rs:142`, `crates/engine/tests/audit.rs:479` (text-correction variant), `marque/src/render.rs:1132`. | Migrate all three to the v2 shape with carve-out comments preserved. |
| `AuditNote::__engine_promote` test-fixture call sites | 2 sites: `crates/engine/tests/audit_note_sealing_carve_out.rs:96, :147`. | **Untouched.** `AuditNote` is a separate NDJSON line type already (PR 3.7) with its own carve-out tests. D's reshape is on `AppliedFix`, not `AuditNote`. |
| `applied_fix_to_audit_json_v3` emitter | **Duplicated**: `marque/src/render.rs:617` (CLI) AND `crates/wasm/src/lib.rs:515` (WASM). Both build the same `AuditRecordJsonV3` shape independently. | Both surfaces migrate; PR D MUST touch both. SC-008 CLI/WASM byte-identity is a hard gate. |
| `#257` masking pin | `crates/engine/tests/core_error_isolation.rs:92` — strict-recognizer pin masking decoder canonical-bytes leak. | Remove pin once T055 canary closes the channel structurally. |
| `Canonical<S>` type | `crates/scheme/src/canonical.rs` — type exists; `from_cve(token, scope, bytes)` and `EngineConstructor::build_open_vocab(category, bytes, scope)` paths sealed. **Zero production callers** at HEAD (`grep -rn 'Canonical::from_cve\|build_open_vocab' crates --include='*.rs' | grep -v tests | grep -v doc-comment` returns 0 source lines outside `canonical.rs` itself). | Wire into `Engine::fix_inner` (promotion path) and surface on `AppliedFixDetail.replacement.canonical`. |
| `RenderContext { scope, emission_form, schema_version }` | Landed PR 3c.2.A at `crates/scheme/src/render_context.rs`. `MarkingScheme::render_canonical(marking, &RenderContext, &mut dyn Write)` signature live. | D wires `RenderContext` construction inside `Engine::fix_inner` (T048b — currently open per tasks.md status). |
| `Diagnostic.message: Message` | Landed PR 3c.2.C. `crates/rules/src/lib.rs:1142`. | NDJSON projection at audit-emit time per contract §296-324. |
| `Diagnostic.citation: Citation` | Landed PR 3c.2.C. `crates/rules/src/lib.rs:1158`. | Note: `AppliedFix` does NOT carry citation per contract §168-171. Citation stays on `Diagnostic` (lint phase). |
| `Severity::Suggest` variant | Already shipped (FR-042). NDJSON serializes lowercase `"suggest"`. | No new D work — already in PR 3.7. |
| `T055 canary scan` | Not started. Target: `crates/engine/tests/canary_scan.rs`. | D lands the canary. |
| `marque --version` schema discoverability | `marque --version` does NOT expose `AUDIT_SCHEMA_VERSION` today (verify in D — `grep AUDIT_SCHEMA_VERSION marque/src/main.rs` returns 0 matches at preflight). | D surfaces the schema name per contract §415-446. |
| WASM CI matrix | `.github/workflows/ci.yml:498-506` — `wasm32-unknown-unknown` target installed + `wasm-pack build` runs. SC-008 parity test runs at `:519`. | **OQ-6 closed.** No new CI job required; `blake3` `pure` feature is WASM-compat. |
| `from_parsed_unchecked` adapter | 37 references at HEAD. **NOT in D's scope** — defers to PR 3c.2.E (per master plan §1 row 5). | D does not delete; D-internal call sites use whichever path post-3c.2.B already migrated to. |
| Audit-record contract docs | `contracts/audit-record.md` §0 (active mvp-3) + body (post-keystone marque-1.0 target). | D retires §0 and promotes body to active. |

### New findings beyond master plan §1

1. **Emit-logic duplication** is more pervasive than master plan's "audit-record contract docs" note hints. Both `marque/src/render.rs` AND `crates/wasm/src/lib.rs` carry independent `applied_fix_to_audit_json_v3` functions with separate `AuditRecordJsonV3` structs. The SC-008 byte-identity gate constrains the migration: both must move atomically OR a shared emitter must be extracted into a third crate. **OQ-D-1 below.**

2. **`AppliedFix.input` field** (`crates/rules/src/lib.rs:822`) carries the caller-supplied input identifier (file path / "-" for stdin / None). The contract §107-178 audit-record shape does NOT mention this field. Either the field stays at the top level (extends contract) or moves into a metadata sub-object. **OQ-D-2 below.**

3. **`FixSource` enum** has 5 variants (`BuiltinRule | CorrectionsMap | MigrationTable | DecoderPosterior | DecoderClassificationHeuristic`). The contract's `discriminant: "strict"|"decoder"` 2-arm shape collapses this. The 5 → 2 collapse needs an explicit mapping (CorrectionsMap is the text-correction line type; MigrationTable is strict; both decoder variants are decoder). **OQ-D-3 below.**

4. **`ConstraintViolation.message` / `bridge_constraint_diagnostic`** (the path that turns scheme-emitted violations into `Diagnostic` values). 3c.2.C resolved OQ-C7 by migrating `ConstraintViolation.message: String → Message`. D should re-verify this migration landed cleanly so audit emit sees structured `Message` everywhere — no `format!`-shaped strings reach NDJSON serialization. (Re-grep at D implementation start; no action expected.)

5. **`AppliedFix::__engine_promote` line drift from PR 0 inventory**. The inventory at `docs/refactor-006/promote-callsite-inventory.md:18,25,32,39` cites lines 1080/1099/1211/1248 in `engine.rs`. Real HEAD lines: **2189/3177/7632** (the 7632 site is a test helper not catalogued in the PR 0 inventory). This is normal drift from 6+ months of PRs; D's implementation-first action is to re-grep before migrating. Per master plan §4 R-4.

6. **`AUDIT_SCHEMA_IS_V3` is dead-equivalent code at HEAD**. The const is `pub` but the only consumer at `marque/src/render.rs:649` is `let _ = AUDIT_SCHEMA_IS_V3;` — a comment-only "kept so a future schema bump can land via the same dispatch shape without restructuring callers". Either re-purpose as `AUDIT_SCHEMA_IS_V1_0` (preserves the dispatch-shape comment) or delete. **OQ-D-4 below.**

---

## 1. Sub-PR commit decomposition

D is the atomic cutover but internal commits can land progressively under
`marque-mvp-3` until the final schema flip. The shape mirrors 3c.2.B's 6+
commit pattern (preflight, prep, migration batches, flip, canary, cleanup,
docs). PR D is larger than B/C: it touches 4 crates' Cargo.toml, 3
production promote sites, 5+ test-fixture sites (3 `AppliedFix` + 2
`AuditNote` left alone), 2 duplicated emitters, the schema accept-list, a
new test file, and 2 docs. **Recommendation: 11 commits.**

### Commit dependency graph

```text
D0 (build-green housekeeping; clippy/format)
  ↓
D1 (workspace blake3 wiring; Cargo.toml only — additive, no code)
  ↓
D2 (AppliedFix v2 type + AppliedFixDetail / Blake3Hash + TextCorrectionRecord; under #[cfg(test)] feature-flag gate to keep callers green)
  ↓
D3 (Engine::fix_inner emit migration — production sites at engine.rs:2189, :3177; Canonical<S> wired; RenderContext constructed per T048b)
  ↓
D4 (CLI render migration — marque/src/render.rs; AuditRecordJsonV1_0 replacing JsonV3; text_correction split-line emitter added)
  ↓
D5 (WASM render migration — crates/wasm/src/lib.rs; same v1.0 shape; SC-008 parity)
  ↓
D6 (test-fixture migration: crates/rules/tests/engine_promotion_seal.rs, crates/engine/tests/audit.rs, marque/src/render.rs)
  ↓
D7 (atomic schema flip — build.rs accept-list mvp-3 → 1.0; audit_schema_accept_list.rs pin updated; AUDIT_SCHEMA_IS_V3 renamed → AUDIT_SCHEMA_IS_V1_0 or deleted; remove the gate flag from D2)
  ↓
D8 (T055 canary scan at crates/engine/tests/canary_scan.rs; #257 masking pin retirement in core_error_isolation.rs)
  ↓
D9 (marque --version surface; main.rs wires AUDIT_SCHEMA_VERSION into the version-printing path; integration test pins the grep-target shape per contract §415-446)
  ↓
D10 (audit-record contract docs: §0 retired; body section promoted; data-model.md AppliedFix v2 update; CHANGELOG; data-model status flip; review attestation footer)
```

**Sequential constraint**: D1–D2 must precede everything (type machinery).
D3–D5 can land in any order but ALL must be green before D7 (the flip).
D6 must precede D7 too (so the schema flip doesn't break tests). D8/D9/D10
are post-flip cleanup and can land in any order but each is its own
reviewer-attested commit.

**Two-stage build-greenness invariant**: Stages D1–D6 keep
`MARQUE_AUDIT_SCHEMA=marque-mvp-3` (no env change). Stage D7 atomically
flips. Stage D8–D10 confirm. A reviewer hitting any commit in D1–D6
should still build + test green on stable Rust.

**Why not fewer commits?** D2 (type) and D3 (engine wiring) cannot collapse
because the type must exist before the engine references it. D4/D5
duplicate the emit logic and must be visibly migrated separately for code
review. D6 (test fixtures) must come before D7 (flip) to keep tests green
during the flip commit. D8 (canary) must come after D7 because the canary
asserts the post-flip wire format. D9/D10 are independent surface area.

**Why not more commits?** Could split D3 by promote-site (2189 vs 3177)
but the two production sites are tightly coupled to the same
`engine_promotion_token()` helper and `AppliedFixDetail` construction; a
two-commit split would force `__engine_promote` to accept both shapes
simultaneously which violates the "either pre-cutover or post-cutover, no
mix" property.

### What lands in each commit

- **D0**: Housekeeping — branch-from-staging baseline check, format/clippy
  green. No code change beyond mechanical formatting if any lint surfaces.
- **D1**: `blake3 = { workspace = true }` added to `crates/rules/Cargo.toml`
  (host crate of `Blake3Hash` type) and `crates/engine/Cargo.toml` (host of
  promote sites computing the digest). No `.rs` changes. Build green.
- **D2**: `AppliedFixDetail<S> { replacement: FixReplacement<S>, original_span: Span, original_digest: Blake3Hash }`,
  `FixReplacement<S> { discriminant: ReplacementDiscriminant, canonical: Canonical<S>, confidence: Confidence }`,
  `enum ReplacementDiscriminant { Strict, Decoder }`,
  `pub struct Blake3Hash([u8; 32])` (or `pub type Blake3Hash = blake3::Hash;`
  — see D-D-5), `pub struct TextCorrectionRecord { ... }` (separate from
  `AppliedFix<S>` per D25.4). New variants `pub struct AppliedFix<S>`
  reshape. Construction sealed via re-used `EnginePromotionToken`. To keep
  callers green, the old `AppliedFix<S>::__engine_promote` and
  `__engine_promote_text_correction` signatures are RENAMED with a
  `_legacy` suffix (not deleted) and the NEW v2-shape `__engine_promote`
  constructor lives alongside. D3–D6 migrate callers; D7 deletes the
  `_legacy` variants atomically with the schema flip.
- **D3**: Engine `fix_inner` and `apply_text_corrections` migrate. The
  splice-buffer original-bytes BLAKE3 digest is computed here (single
  source of truth — emit-time digest is forbidden, see D-D-6).
  `RenderContext` is constructed per fix (T048b lands here);
  `Canonical<S>` flows from `MarkingScheme::render_canonical` via
  `EngineConstructor::build_open_vocab` (open-vocab path) and
  `Canonical::from_cve` (closed-CVE path). Text-correction promotion
  produces `TextCorrectionRecord` instead of `AppliedFix`; the engine's
  output channel for text corrections splits — `FixResult.applied` stays
  for marking fixes, a new `FixResult.text_corrections:
  Vec<TextCorrectionRecord>` field is added (or the two collapse into a
  single enum stream — see D-D-7).
- **D4**: `marque/src/render.rs` — replace `AuditRecordJsonV3` +
  `proposal_to_json` with `AuditRecordJsonV1_0` matching contract §107-178.
  Add `text_correction_to_json` + `render_text_correction_record` for the
  new line type. `MessageTemplate` JSON projection wired (closed-enum
  variant-name serialization per contract §296-324). `Citation` JSON
  projection skipped — contract §168-171 says AppliedFix carries no
  citation.
- **D5**: `crates/wasm/src/lib.rs` — same migration as D4, byte-identical
  JSON. SC-008 parity test re-runs green.
- **D6**: Test-fixture migration:
  - `crates/rules/tests/engine_promotion_seal.rs:142` — exercise new
    constructor, preserve carve-out comment.
  - `crates/engine/tests/audit.rs:479` — text-correction synth helper for
    `fabricate_leaky_fix`; migrate to `TextCorrectionRecord` construction.
  - `marque/src/render.rs:1132` — render-NDJSON unit test; migrate to v2
    shape.
  - `crates/engine/src/engine.rs:7632` — `synth_applied_fix` inside
    `#[cfg(test)]`; migrate.
  - `AuditNote` carve-out tests at
    `crates/engine/tests/audit_note_sealing_carve_out.rs:96, :147`
    UNTOUCHED — `AuditNote` is a separate line type per PR 3.7 and is not
    affected by D's `AppliedFix` reshape.
- **D7**: **Atomic schema flip.** `crates/engine/build.rs:24-25`
  `ACCEPTED = &["marque-1.0"]`, `DEFAULT = "marque-1.0"`. Pin update at
  `crates/engine/tests/audit_schema_accept_list.rs`. `AUDIT_SCHEMA_IS_V3`
  renamed → `AUDIT_SCHEMA_IS_V1_0`. The `_legacy` constructors from D2
  delete here. Wire format flips byte-identically (single binary, single
  schema per FR-014).
- **D8**: `crates/engine/tests/canary_scan.rs` — T055 canary harness.
  Iterates the five corpora (`tests/corpus/{valid,mangled,prose,prose-positive,lattice}/`),
  serializes every `AppliedFix` + `TextCorrectionRecord` to NDJSON, and
  scans each line for any contiguous ≥4-byte sequence appearing in the
  input but not in the permitted-identifier list (span numerals, BLAKE3
  hex prefixes, closed-enum tokens). Two assertions: (a) zero leaks on
  HEAD post-D, (b) a synthetic regression (e.g.,
  `format!("{}", replacement)` injected into a message arg) fires the
  canary. The `#257` masking pin at
  `crates/engine/tests/core_error_isolation.rs:92` is retired in the same
  commit — the canary structurally closes the channel.
- **D9**: `marque --version` surface. `marque/src/main.rs` modifies the
  version-printing path to include `audit_schema: <AUDIT_SCHEMA_VERSION>`
  in the output. Integration test at `marque/tests/cli_version.rs` (new)
  pins the grep-target shape (`marque --version | grep "^audit_schema:"`
  returns `audit_schema: marque-1.0`).
- **D10**: Documentation cutover. `contracts/audit-record.md` §0 deletes;
  body promoted to active spec (§94-178 becomes the live shape).
  `data-model.md` `AppliedFix v2` section updated (verify presence). The
  `Post-marque-1.0 RuleId migration` section retained as forward-looking
  note (FR-049 / R-6). CHANGELOG entry + version-printing changelog note
  per contract §437. T041, T052, T055 task checkboxes flipped in
  `specs/006-engine-rule-refactor/tasks.md`.

---

## 2. Resolved decisions for PM ratification

Numbered `D-D-N` so the master PM contract can carry them forward.

### D-D-1 — Emit-logic placement: keep CLI + WASM duplication, migrate atomically

**Decision**: Keep `applied_fix_to_audit_json_v3` (renamed `_v1_0`) duplicated
between `marque/src/render.rs` and `crates/wasm/src/lib.rs`. Migrate both in
D4/D5 as separate commits to preserve diff reviewability. **Do NOT extract a
shared `marque-audit-render` crate as part of D.**

**Rationale**: Extracting a shared emitter is a real refactor that touches the
crate graph (Constitution VII), adds a maintenance surface, and is
orthogonal to D's atomic-cutover work. The duplication is verbatim and
small (~200 LoC per side); SC-008 byte-identity is the existing
correctness gate. Code-review-time consistency burden is mitigated by
landing D4 and D5 in sequence with the same diff shape.

**Rejected**: Shared crate extraction — increases D blast radius, decouples
review of CLI vs WASM behavior, opens a question about whether `Citation`
JSON projection belongs in that crate (contract §168-171 says no, but a
shared crate would invite it). Defer to a post-PR-10 refactor PR.

### D-D-2 — `AppliedFix.input` field placement: stays at top level

**Decision**: `AppliedFix.input: Option<Arc<str>>` stays at the top level of
the v2 shape, NOT inside `AppliedFixDetail`. The contract §107-178 sample
shape will be extended to show it.

**Rationale**: `input` is per-promotion runtime context (caller-supplied
file path / stdin marker). It's not part of the marking-fix substance; it
mirrors `timestamp` / `classifier_id` / `dry_run` which all live at the
top level. Demoting to a sub-object would force the same demotion on the
three sibling fields for symmetry, which is a needless contract change.

**Rejected**: Moving to a `meta: AuditMeta { input, timestamp, classifier_id, dry_run }`
sub-object — adds a layer of JSON nesting for callers that today read
`record.timestamp` directly.

### D-D-3 — `FixSource` → `discriminant` mapping: 5-to-2 collapse with TextCorrection split

**Decision**:

| `FixSource` variant | `marque-1.0` destination |
|---|---|
| `BuiltinRule` | `discriminant: "strict"` (inside `AppliedFixDetail.replacement`) |
| `MigrationTable` | `discriminant: "strict"` |
| `DecoderPosterior` | `discriminant: "decoder"` |
| `DecoderClassificationHeuristic` | `discriminant: "decoder"` |
| `CorrectionsMap` | **`TextCorrectionRecord.source: "corrections_map"`** (separate line type, NOT discriminant) |

The `discriminant` enum is `Strict | Decoder` (2 variants, sealed
`#[non_exhaustive]` for future grow-path per FR-035a). The
`TextCorrectionRecord.source` is a separate field that today's only
producer (the C001 corrections-map path) populates with `"corrections_map"`;
a future text-correction source (e.g., the deprecation migration path if
it ever surfaces non-marking text corrections) would extend the enum.

**Rationale**: The discriminant question is "did this fix come from
type-safe parser output (strict) or probabilistic recovery (decoder)?" —
both `BuiltinRule` and `MigrationTable` produce fixes from the strict
parse path; both `DecoderPosterior` and `DecoderClassificationHeuristic`
come from the decoder. `CorrectionsMap` is structurally different — it's
pre-scanner text replacement, not marking-shape — and matches D25.4's
text-correction split.

**Rejected**:
- 5-arm discriminant — semantically conflates marking-provenance with
  source-channel.
- 3-arm discriminant (`Strict | Decoder | TextCorrection`) — would put
  TextCorrection inside `AppliedFix` instead of as a separate line type,
  contradicting D25.4.

### D-D-4 — `AUDIT_SCHEMA_IS_V3` const disposition: rename to `AUDIT_SCHEMA_IS_V1_0`

**Decision**: At D7, rename `AUDIT_SCHEMA_IS_V3` →
`AUDIT_SCHEMA_IS_V1_0`. Keep `let _ = AUDIT_SCHEMA_IS_V1_0;` in
`render_audit_record` (preserves the dispatch-shape comment for the next
schema bump).

**Rationale**: The const is documented as forward-looking
infrastructure ("the const exists to give downstream code a stable
shape-discriminant across future schema bumps" — `crates/engine/src/lib.rs:93-97`).
Deleting it removes the precedent for the next schema bump's dispatch.
Renaming preserves the precedent at zero cost.

**Rejected**: Delete the const — discards the documented dispatch-shape
discipline.

### D-D-5 — `Blake3Hash` type: alias to `blake3::Hash` (not newtype)

**Decision**: `pub type Blake3Hash = blake3::Hash;` (re-export) at
`crates/rules/src/lib.rs` (or a dedicated `crates/rules/src/digest.rs`).
NOT a newtype wrapper.

**Rationale**:
- `blake3::Hash` already implements `Debug`, `Clone`, `Copy`, `PartialEq`,
  `Eq`, `Hash`, and `Display` (lowercase-hex). All the type properties D
  needs are free.
- `blake3::Hash::from_bytes([u8; 32])` is the canonical construction; a
  newtype would re-export it through a method that adds zero invariants.
- The serialization to `"blake3:<64-hex>"` per contract §137/§151 is a
  thin `format!("blake3:{}", hash.to_hex())` — fits in the emit-time
  projection function, no newtype required.
- Constitution VII: type stays in the WASM-safe set (`marque-rules` is
  WASM-safe). `blake3` `pure` feature is WASM-compatible (verified —
  workspace dep already gates pure).

**Rejected**: Newtype `pub struct Blake3Hash(blake3::Hash)` — adds API
surface (methods to wrap and unwrap) and a `Debug` impl that would
either delegate or diverge. Both options are inferior to the alias.

### D-D-6 — Digest computation site: promotion-time, not emit-time

**Decision**: `original_digest` (BLAKE3 of pre-fix bytes) is computed
INSIDE `Engine::fix_inner` (and `Engine::apply_text_corrections`) at the
point of promotion, AS PART of `AppliedFix::__engine_promote` argument
construction. The hash flows through the constructor; the emit-time
projection is a `format!`-only operation.

For `canonical.bytes_digest`: computed AT THE SAME PROMOTION SITE from
`Canonical::bytes()`. Stored in the `Canonical<S>` itself OR alongside in
`AppliedFixDetail`. **Recommendation: store on `Canonical<S>` via a new
`pub fn bytes_digest(&self) -> Blake3Hash` method** — keeps the digest
adjacent to the bytes it digests, prevents the "digest of one set of bytes
attached to a different set of bytes" bug class.

**Rationale**:
- Constitution I (uncompromising performance): emit-time digest
  computation runs once per emit call. Promotion-time computation runs
  once per fix and is then cheap to clone (Hash is `Copy`). For batched
  emit (FixResult with N fixes) the savings are linear.
- The emit-time path runs in the CLI's stderr-write loop (line 825) and
  the WASM `serde_json::to_string` path (`crates/wasm/src/lib.rs:550`);
  putting BLAKE3 hashing inline at the JSON-serialization boundary would
  silently slow both.
- Per-promotion computation is auditably one-to-one: each `__engine_promote`
  call computes exactly two digests (original_digest + canonical.bytes_digest).

**Rejected**:
- Emit-time computation — perf + auditability concerns above.
- Lazy `OnceCell<Blake3Hash>` on `AppliedFix` — added complexity for a
  cheap computation; `AppliedFix` is meant to be immutable post-promotion.

### D-D-7 — TextCorrectionRecord channel: shared `Vec<AuditLine>` enum stream

**Decision**: Add a new sum type `AuditLine<S> { AppliedFix(AppliedFix<S>) | TextCorrection(TextCorrectionRecord) }`
in `marque-rules`. `FixResult.applied: Vec<AppliedFix<S>>` becomes
`FixResult.audit_lines: Vec<AuditLine<S>>` — single stream, two arms.
Emit dispatch iterates the stream and routes each line to its NDJSON
projection. NDJSON wire format preserves the line-per-record property
(both arms emit one line).

**Rationale**:
- Preserves promotion order across the two record types (FR-016 invariant
  generalized: an audit reader reading the stream sees fixes and text
  corrections in the order the engine promoted them).
- Single emit loop on the CLI/WASM side reduces emit-path complexity (one
  iteration over `audit_lines`, dispatch per arm).
- The contract §107-178 + §388-402 sample shapes are independent records
  on the wire — the in-memory sum type is invisible to NDJSON consumers.

**Rejected**:
- Two parallel `Vec`s (`applied` + `text_corrections`) — loses promotion
  order across the two channels, requires consumers to merge by timestamp
  (timestamp is at second resolution; collisions are plausible).
- Single `Vec<AppliedFix<S>>` with TextCorrection as an AppliedFix
  variant — violates D25.4 (TextCorrection becomes own NDJSON line type).

### D-D-8 — Test-fixture migration: single PR (D6) with grouped commits if size warrants

**Decision (resolves OQ-4)**: Single test-fixture migration commit D6
covering all 4 sites (3 `AppliedFix` test-fixtures + the engine.rs:7632
test helper). If commit-size review surfaces friction, split D6 into
D6a (rules + engine fixtures) and D6b (CLI render fixture). PM's
mid-flight directive favors quality-with-multiple-agents over single-pass;
the implementation agent has authority to split D6 if they judge a single
commit unreviewable.

**Rationale (master plan OQ-4)**: The 4 sites are all `__engine_promote`
calls with similar shape. The migration is mechanical (rewrite construction
to v2 shape, preserve carve-out comments). A single commit keeps the
"test fixtures migrated atomically" property visible in git log. The
split-on-friction escape valve respects the user's
"quality > doggedness" directive.

**Rejected**: Per-site commit (4 commits) — bookkeeping overhead exceeds
review-cycle benefit. Per-test-suite split (CAPCO / engine / core / CLI) —
no real boundary; `audit.rs` and `engine.rs` are both under `crates/engine/`.

### D-D-9 — Decoder/strict dispatch (resolves OQ-5): provenance derived from `FixSource`, not recognizer-time dispatch

**Decision**: The `discriminant: "strict"|"decoder"` field is populated at
audit-emit time by deriving from `AppliedFix.source: FixSource` per the
mapping in D-D-3. No separate code path in `Engine::fix_inner` is needed;
recognizer dispatch (`StrictOrDecoderRecognizer` vs explicit
`StrictRecognizer`) is upstream of fix synthesis and the FixSource is
already populated correctly by the rule (decoder-emitted fixes carry
`FixSource::DecoderPosterior` or `DecoderClassificationHeuristic`;
strict-emitted carry `BuiltinRule` or `MigrationTable`).

**Rationale**:
- HEAD already has this discrimination — `FixSource::DecoderPosterior` is
  populated at the decoder bridge (`crates/engine/src/engine.rs:4061`).
  The discriminant is a re-projection of existing data, not new data.
- Putting discriminant on the wire is a renaming + 5-to-2 collapse, both
  of which are pure projections.
- Constitution VI (dataflow pipeline): adding a new discriminator at the
  emit stage avoids threading a new field through every promote call
  site.

**Rejected**: Dedicated `discriminant: ReplacementDiscriminant` field
threaded through every `__engine_promote` argument list — needless
argument-count growth; the data is already on `FixSource`.

### D-D-10 — WASM build verification (resolves OQ-6): no new CI job, smoke-test in D5

**Decision**: No new CI job needed. The existing `wasm-pack build` job
(`.github/workflows/ci.yml:504-506`) and SC-008 parity test (`:519`)
cover the WASM target. D5's WASM emit migration adds a regression test at
`crates/wasm/tests/audit_v1_0_parity.rs` (new) that exercises the
v1.0-shape JSON round-trip on a representative fixture and asserts
byte-identity against the CLI's emit on the same input.

**Rationale**:
- `blake3` `pure` feature is already declared in the workspace dep; CI
  has been building WASM with `blake3` as a transitive presence (via
  unused dep paths) for months.
- The actual NDJSON byte-identity check is the load-bearing one for
  SC-008; the existing parity test framework is the right home.

**Rejected**: Dedicated `cargo build --target wasm32-unknown-unknown -p marque-rules`
CI step — duplicates what the workspace WASM job already covers.

---

## 3. Risk register (extending master plan R-1 through R-6)

### R-D-1 — BLAKE3 `pure` feature WASM payload size

`blake3` with `default-features = false, features = ["pure"]` adds ~30-50 KB
gzipped to the WASM binary (estimate based on `blake3` source size minus
SIMD asm paths). Master plan D25.7 sets a ≤5% size budget measured at D;
the budget is likely to bind.

**Mitigation**: Measure WASM binary size at D5 (the WASM migration commit)
against `staging` HEAD baseline. If the delta exceeds 5%, fall back
options in priority order:
1. Verify `wasm-opt -Oz` is in the WASM build pipeline (it should be —
   check `crates/wasm/.cargo/config.toml` and `wasm-pack` defaults).
2. Evaluate gate-only computation — compute BLAKE3 in WASM only on emit,
   not on every fix (would contradict D-D-6; only if budget binds hard).
3. Replace `blake3::Hash` with a smaller-footprint hash on WASM only
   (Constitution III says no — single source of truth).
4. Escalate to PM with measurement evidence.

The master plan D25.7 already names this risk; this entry sharpens the
measurement gate to D5 specifically (not "at D atomic flip" — D5 is the
earliest commit where the WASM binary size delta is observable).

### R-D-2 — Audit-stream consumer surprise on text_correction split

D25.4 splits text corrections into a separate `{"type":"text_correction", ...}`
NDJSON line. Pre-D, audit-stream consumers iterating `applied:
Vec<AppliedFix>` see text corrections as `AppliedFixProposal::TextCorrection`
arms. Post-D, those consumers must learn the second line type.

**Mitigation**: There are no external consumers per `decisions.md` D4 (no
deployment, pre-users). Internal consumers are catalogued in this preflight:
- CLI emit at `marque/src/main.rs:825` — migrated in D9 (via the
  AuditLine sum-type dispatch).
- WASM `applied_fix_to_audit_json_v3` at `crates/wasm/src/lib.rs:550` —
  migrated in D5.
- Test snapshots — migrated in D6.

No surprise can land outside the migration. Adding the changelog entry
in D10 (mandatory per contract §437) provides the operational signal for
the no-users-yet case.

### R-D-3 — T055 canary false positives on legitimate span integer overlap

The canary scans for ≥4-byte sequences from the input appearing in NDJSON
output. Span integers (`{"start": 1024, "end": 1037}`) and BLAKE3 hex
prefixes are explicitly excluded. **But the input might contain "1024" or
"1037" verbatim** — a document with a port number, a year, a measurement.
The canary would false-positive.

**Mitigation**: The canary's "permitted sequence" list must be span-aware
— a numeric sequence inside a JSON value position whose JSON key is
`start` or `end` is permitted. Implementation strategy: parse the NDJSON
line into a `serde_json::Value`, walk the JSON tree, and only check
string-valued JSON leaves (and inside non-`start`/`end` numeric leaves,
treat as a permitted identifier IFF the numeric value matches the
recorded span). A simpler alternative: when scanning the NDJSON line,
exclude the substring under any JSON-encoded `"start": N` or `"end": N`
key. The test must demonstrate the exclusion works on a fixture
containing the digit-substring "1024" in both the input and a span
field (positive case: no false positive).

### R-D-4 — `AppliedFixDetail` placement decision under-specified at PM ratification

D2 introduces `AppliedFixDetail` as the `replacement + original_span +
original_digest` triple. The contract §123-152 sample shape suggests the
type lives at the JSON layer (a nested object on the wire). Rust-side
placement options:
- (a) `pub struct AppliedFixDetail<S>` in `marque-rules` alongside
  `AppliedFix<S>` — both the type and the field are public.
- (b) `pub struct AppliedFixDetail<S>` in `marque-engine` since only the
  engine constructs it — but then `AppliedFix<S>.fix` references a type
  the rules crate can't name, breaking the
  `AppliedFix<S>: Send + Sync` compile-time check in
  `crates/capco/tests/send_sync.rs`.

**Mitigation**: Place in `marque-rules` (option a). The type co-locates
with `AppliedFix<S>` (mirrors `AppliedFixProposal<S>` at HEAD), keeps
construction sealed via the existing `EnginePromotionToken` pattern, and
satisfies Constitution VII (rules crate has no new deps).

**Decision**: D-D-1bis — place in `marque-rules`. (Adding here to the
risk register rather than a separate decision because it's a placement
question with a clear right answer once stated.)

### R-D-5 — Schema atomicity vs `_legacy` constructor window

D2 introduces NEW v2 `__engine_promote` alongside RENAMED `_legacy`
variants. D3-D6 migrate callers to v2. D7 deletes the legacy. Between
D2-D6 the codebase has TWO promote shapes simultaneously. A new promote
call landed via parallel PR during that window could use the legacy
shape silently.

**Mitigation**: D2's `_legacy`-suffix rename is paired with a `#[deprecated(note = "use AppliedFix::__engine_promote (v2) post-3c.2.D2")]` attribute
on the legacy constructors. The lint `tools/promote-callsite-lint/`
already flags every `__engine_promote*` call by last-segment matching;
extending it to flag `_legacy`-suffix specifically is a 5-line change.
The window closes at D7 (delete).

If review-time concern surfaces, fold D2-D7 into a single commit (worst
case 1500 LoC diff). The 11-commit decomposition is the
review-quality-of-life choice, not a correctness requirement.

### R-D-6 — `Canonical::bytes_digest` introduction on the type (D-D-6 implication)

D-D-6 recommends adding `pub fn bytes_digest(&self) -> Blake3Hash` to
`Canonical<S>`. This is a public API addition to a sealed-construction
type. Adding the method requires `blake3` dependency in `marque-scheme`
(currently has no blake3 dep — verified at preflight).

**Mitigation**: Add `blake3 = { workspace = true }` to
`crates/scheme/Cargo.toml`. The crate is WASM-safe; `blake3 pure` is
WASM-compat. Adds ~30KB to WASM payload (counted once across the
workspace, not per-crate). Constitution VII allows `marque-scheme`
crate to depend on workspace crates within the WASM-safe set; `blake3`
is a workspace dep, not a `marque-*` crate, so the graph stays clean.

Alternative: compute bytes_digest at promotion time at the engine
boundary, store on `AppliedFixDetail.canonical_bytes_digest` directly
(skip the `Canonical<S>` method). Saves the dep on `marque-scheme` at
the cost of moving the "digest of bytes" association one level away from
the bytes.

**Recommendation**: PM ratifies — the dep cost is small and the
association is the right one. If PM rejects on scheme-leaf purity
grounds, switch to the alternative.

### R-D-7 — Test fixture path drift from PR 0 inventory

The PR 0 inventory at `docs/refactor-006/promote-callsite-inventory.md`
cites line numbers that have drifted (1080→2189, 1099→3177, 1211→2189-text,
1248→engine_promotion_token helper). Master plan R-4 names this. The
implementation agent's first action is `grep -rn '__engine_promote(' crates/ --include='*.rs'`
to reconcile against current HEAD.

**Mitigation**: D's preflight (this document) records the HEAD inventory
at §0 in section 0's table. The implementation agent re-grep before D2
implementation start; any new sites surfaced (a parallel PR landed a
fourth promote site) are caught by the existing
`tools/promote-callsite-lint/` and force a deliberate decision per
Constitution V.

---

## 4. Constitution check

| Principle | PR-D-specific gate state | Result |
|---|---|---|
| **I (Uncompromising Performance)** | D-D-6 places digest computation at promote-time (not emit-time) to keep emit-loop overhead constant. SC-001 16ms ceiling not threatened by promote-time BLAKE3 (per-fix ~µs cost). D5 WASM size budget per D25.7. | PASS (conditional on D5 size measurement) |
| **II (Zero-Copy)** | `original_digest` replaces any byte-content storage in audit records (per master plan §3). No new heap allocation on the hot path (BLAKE3 hash is `[u8; 32]`-sized Copy type per D-D-5). `secrecy::SecretSlice<u8>` for `FixResult.source` preserved unchanged. | PASS |
| **III (Format-Agnostic Core / WASM Safety)** | `blake3 pure` feature WASM-compat verified at workspace level. No new format dependencies introduced. `crates/wasm` migrated atomically with CLI (D4/D5). WASM-runtime-config invariant preserved (D doesn't introduce runtime configurability of the new audit fields). | PASS |
| **IV (Two-Layer Rule Architecture)** | No Layer 1 (generated) / Layer 2 (hand-written) boundary change. `RenderContext` already lives in `marque-scheme` per PR #627. | PASS |
| **V (Audit-First Compliance)** | **The principle's central concern.** G13 becomes a type+canary invariant: `AppliedFix` v2 carries no document bytes (type-level); T055 canary verifies no input leaks at corpus scale (deterministic check). `__engine_promote` carve-out comments preserved across migration. `#257` masking pin retires concurrently with canary green (structural channel closure). | PASS |
| **VI (Dataflow Pipeline Model)** | No phase change. `RenderContext` slots into the existing promotion path; `AuditLine<S>` sum type preserves promotion order across both record types (D-D-7). | PASS |
| **VII (Crate Discipline)** | `blake3` workspace dep added to `marque-rules` + `marque-engine` (D1) + `marque-scheme` (D-D-6 conditional). All three are WASM-safe; no new domain coupling. `TextCorrectionRecord` lives in `marque-rules` (no new crate needed). | PASS (conditional on D-D-6 dep placement) |
| **VIII (Authoritative Source Fidelity)** | No CAPCO §-citations migrated. `Citation` JSON projection stays out of `AppliedFix` per contract §168-171 — citation lives on `Diagnostic` (lint phase), not in audit records. Mechanical citation-lint continues to verify `Citation` values at compile time. | PASS |

---

## 5. Reviewer attestation checklist (extends master plan §5)

Per-commit reviewer attestation in D:

- [ ] CAPCO §-citations in `§X.Y pNN` form only — no bare `§NN`, no `file:line` anchors (Constitution VIII)
- [ ] Adjacent code paths walked — the migration touched 2 emitters (CLI + WASM), verify both updated symmetrically (D-D-1)
- [ ] Constitution VII crate boundary preserved — `marque-scheme`, `marque-rules`, `marque-engine` are WASM-safe; verify `blake3 pure` flag flowing through Cargo.toml additions
- [ ] Constitution V Principle V — no production `__engine_promote` wire-up outside `Engine::fix_inner` / `Engine::apply_text_corrections` / `engine_promotion_token()` helper; test-fixture carve-out comments preserved at all 4 sites
- [ ] **G13 canary green** — `cargo test --test canary_scan` passes on D8 (post-flip); a synthetic regression injecting input bytes into `MessageArgs.token` fires the canary
- [ ] **Schema atomicity** — `git log --grep="MARQUE_AUDIT_SCHEMA"` between staging and D7-HEAD shows EXACTLY ONE commit changing the accept-list (D7); no `_legacy` constructors remain post-D7 in production code
- [ ] **`AUDIT_SCHEMA_VERSION` single source of truth** — `grep -r "marque-1.0" crates marque` shows only the env-derived const flowing; no hardcoded literal `"marque-1.0"` outside `crates/engine/build.rs` and test fixtures
- [ ] **No audit-stream content channel reopened** — `grep -rn 'format!.*input\|format!.*bytes\|format!.*replacement' crates/engine crates/rules marque/src crates/wasm/src` outside test code returns zero new sites
- [ ] **No 2-tuple `RuleId` leakage** — `rule` field stays string form per master plan §4 R-6; verify by grepping `serde_json::json!\(\{"scheme":` returns 0 hits outside the post-PR-10 forward-looking note in contract docs
- [ ] **`marque --version` schema discoverability** — `marque --version | grep "^audit_schema:"` returns `audit_schema: marque-1.0` post-D9; integration test at `marque/tests/cli_version.rs` pins this
- [ ] **WASM binary size delta ≤5%** per D25.7 — measured at D5 against staging baseline, reported in PR description; if exceeded, R-D-1 mitigation triggered
- [ ] **SC-008 byte-identity** — CLI + WASM emit byte-identical NDJSON on the regression corpus; verified by `crates/wasm/tests/audit_v1_0_parity.rs` (new) and the existing parity job
- [ ] **Citation propagation re-verified** at D10 — any §-citation moving from `audit-record.md` body §0 into the active section, or into `data-model.md`, re-checked against `crates/capco/docs/CAPCO-2016.md`
- [ ] "Will we want to maintain this for 5 years?" durability standard

---

## 6. Coverage strategy

PR D adds non-trivial new audit-emit code paths:

- `AppliedFix v2` type machinery (`marque-rules`)
- `AppliedFixDetail` + `FixReplacement` + `ReplacementDiscriminant` types
- `TextCorrectionRecord` type + emit
- `AuditLine<S>` sum type
- CLI `applied_fix_to_audit_json_v1_0` (replaces v3 emit)
- WASM `applied_fix_to_audit_json_v1_0` (replaces v3 emit)
- T055 canary harness
- `marque --version` schema-discoverability surface

>80% coverage is required per standing brief. The coverage strategy maps
to each new surface:

### New test files (D commits create these)

| Test file | Coverage target | Lands in |
|---|---|---|
| `crates/engine/tests/canary_scan.rs` | T055 deterministic canary; positive (no leaks) and negative (synthetic regression fires) | D8 |
| `crates/wasm/tests/audit_v1_0_parity.rs` | CLI/WASM byte-identity on v1.0 envelope | D5 |
| `marque/tests/cli_version.rs` | `marque --version` schema name + grep-target shape per contract §415-446 | D9 |
| `crates/rules/tests/applied_fix_v2_shape.rs` | `AppliedFix` v2 round-trip, constructor seal, `EnginePromotionToken` carve-out compile-fail proofs (parallel to existing `engine_promotion_seal.rs`) | D2 |
| `crates/rules/tests/text_correction_record_shape.rs` | `TextCorrectionRecord` construction, sealing, NDJSON round-trip | D2 |

### Extended test files (D modifies existing)

| Test file | Extension | Lands in |
|---|---|---|
| `crates/engine/tests/audit.rs` | Migrate `fabricate_leaky_fix` to v2; add v1.0 envelope round-trip tests; preserve PROSE_SENTINELS sweep | D6 |
| `crates/engine/tests/audit_schema_accept_list.rs` | Pin updated mvp-3 → 1.0 | D7 |
| `crates/engine/tests/fix_pipeline.rs` | Update v2 audit-shape assertions (line 224 references stale `applied_fix_to_audit_json_v2`) | D4 (parallel migration) |
| `crates/rules/tests/engine_promotion_seal.rs` | Migrate `documented_door_can_mint_token_from_outside_marque_rules` to v2 shape | D6 |
| `crates/engine/src/engine.rs` `#[cfg(test)] synth_applied_fix` | Migrate to v2 shape | D6 |
| `marque/src/render.rs` `#[cfg(test)] render_audit_record_produces_valid_ndjson` | Migrate to v2 expected NDJSON | D6 |
| `crates/wasm/tests/parity.rs` | Update fixtures to v1.0 envelope | D5 |
| `crates/engine/tests/core_error_isolation.rs` | Remove `#257 masking pin`; rely on `StrictOrDecoderRecognizer` default | D8 |

### Coverage measurement gates

- `cargo llvm-cov --workspace --lcov --output-path /tmp/lcov.info` runs in CI; CodeCov reports the delta.
- D2's type machinery has cyclomatic complexity ~0 (data shapes); branch coverage matches line coverage.
- D3's engine wiring is the highest-complexity surface (RenderContext construction, Canonical from_cve / build_open_vocab dispatch, BLAKE3 computation, splice-buffer hashing). Aim for >85% line coverage on `Engine::fix_inner` and `Engine::apply_text_corrections` post-D3.
- D8 canary is the load-bearing surface — coverage gate >90% (the canary itself, not the corpus it sweeps).

### Performance regression watch

Per master plan D25.6 bench is informational not blocking. But D-D-6
introduces per-fix BLAKE3 computation (two hashes per `AppliedFix`); the
existing `lint_10kb` / `fix_throughput` benches at
`crates/engine/benches/lint_latency.rs` will measure the delta. Expected
overhead: a few hundred ns per hash on x86_64 with AVX-512 SIMD, ~2-5µs
per hash on the WASM `pure` feature path. The fix-pipeline 16ms ceiling
(SC-001) has substantial headroom; this won't bind. Report deltas in PR
description per D25.6 discipline.

---

## 7. Out-of-scope items (explicit non-coverage)

These items are NOT addressed in PR D and remain open:

| Item | Reason | Disposition |
|---|---|---|
| `from_parsed_unchecked` adapter deletion | Per master plan §1, this is PR 3c.2.E's scope | Tracked in master plan §1 row 5 + tasks.md T054 |
| 2-tuple `(scheme, predicate_id)` `RuleId` form | Post-PR-10 per FR-049 (stability-freeze rule) | Forward-looking note in `contracts/audit-record.md` body §202-249 |
| R001/R002 sentinel `"engine"` scheme labels | Same post-PR-10 PR as 2-tuple migration | Same |
| `MarkingScheme::evaluate_custom` ctx extension | Tracked in `specs/006-engine-rule-refactor/followups/constraint-context-extension.md` | Out of refactor scope |
| Admonition channel (S005 split) | Tracked in `followups/admonition-channel.md` | Deferred from PR 3c; not blocking |
| Shared `marque-audit-render` crate | Per D-D-1, defers to post-PR-10 refactor | Recorded here |
| `Severity::Suggest` introduction | Already shipped (FR-042) — D ratifies the existing NDJSON serialization | No D work |
| Kleene PR (Constraint::Custom coverage) | Active separate work on `staging` per user instruction | Verify on D rebase; D does not block on it |
| Coverage-equivalence assertion for #257 retirement | Task T055 instrumentation includes this; D8 implementation covers | Explicitly included in D8 attestation |
| Cumulative perf regression analysis (PRs 4–6) | Tracked in PR 4b-perf umbrella #582 | Post-PR-5 dedicated perf-analysis pass |

---

## 8. Implementation cadence (D-specific)

Per user's standing process discipline:

1. **Preflight** (this document + rust-specialist parallel preflight if scheduled) — surface tactical decisions
2. **PM decisions** — ratify or override D-D-1 through D-D-10 in `pr3c2-d-pm-decisions.md`
3. **Implementation** — agent works the commits in dependency order (D0 → D10), 11 commits per §1; carve-out comments preserved on every test-fixture migration; full `crates/capco/CAPCO-CONTEXT.md` brief + standing constraints brief
4. **3-reviewer pass per commit** — rust-reviewer + code-reviewer + (architect for D2/D3/D8 architectural touches, code-reviewer alternate for the mechanical commits D1/D6/D9/D10)
5. **Submit PR + monitor** — armed persistent monitor; typical 2–5 Copilot rounds; suppressed Copilot comments historically high signal
6. **PR D ratification** — single PR against `staging` (the umbrella). The 11 commits land as a single PR per master plan §1 atomicity contract for D. Reviewer must verify the schema-flip commit (D7) is the atomic boundary.

**Standing constraints carried in every implementation brief**:

- No force-push without explicit user authorization
- No `--no-gpg-sign` / `--no-verify`
- All PRs against `staging`
- >80% test coverage; CodeCov deny triggers test suite expansion (per §6)
- "Will we maintain this for 5 years?" durability standard
- Walk adjacent code paths — both emitters (CLI + WASM) migrated symmetrically
- The mid-flight quality-vs-doggedness directive: D6 may split if the
  implementation agent judges a single commit unreviewable
- Constitution V Principle V test-fixture carve-out comments preserved
  verbatim at every migrated test site

---

## 9. Open questions still requiring PM resolution

The decisions above (D-D-1 through D-D-10) are tactical-implementation
choices the architect has authority to resolve. The PM should explicitly
ratify or override each in `pr3c2-d-pm-decisions.md` before D2 implementation
begins. The following are higher-stakes items that may require PM
re-evaluation:

1. **D-D-6** (digest computation at promote-time, type carries `bytes_digest`
   method) — adds a `blake3` dep to `marque-scheme`. PM may prefer alternative
   placement; weigh against R-D-6.
2. **D-D-7** (`AuditLine<S>` sum type for promotion-order preservation) — this
   adds API surface to `FixResult`. If PM prefers two parallel `Vec`s, document
   the timestamp-merge requirement explicitly so consumers know how to interleave.
3. **WASM size budget at D5** — if R-D-1 mitigation fails and the budget binds,
   the PR D blast radius expands. PM should pre-authorize the escape valve
   (e.g., "exceed 5% if necessary, document in PR description") or commit to
   stop-the-line if exceeded.

---

## Appendix A — Verified `__engine_promote` call site inventory at HEAD

This supersedes the PR 0 inventory at
`docs/refactor-006/promote-callsite-inventory.md` (line numbers drifted).

**Production sites** (allowed per FR-040 carve-out):

| File:line | Function | Notes |
|---|---|---|
| `crates/engine/src/engine.rs:2189` | `Engine::apply_text_corrections` → `__engine_promote_text_correction` | Splits into TextCorrectionRecord at D3 |
| `crates/engine/src/engine.rs:3177` | `TwoPassFixer::build_applied` → `__engine_promote` | Marking-fix promote; v2 shape at D3 |
| `crates/engine/src/engine.rs:7632` | `synth_applied_fix` inside `#[cfg(test)]` | Test helper, falls under carve-out |

**`engine_promotion_token()` helper**: not catalogued at the same depth
in this preflight; the PR 0 inventory line `1248` corresponds to a
production helper that remains a single source of truth for token
minting. Implementation agent re-greps at D3.

**Test-fixture sites** (Constitution V carve-out, comments verified at
preflight authorship):

| File:line | Carve-out present | Migrates to |
|---|---|---|
| `crates/rules/tests/engine_promotion_seal.rs:142` | Yes | v2 shape |
| `crates/engine/tests/audit.rs:479` | Yes (`fabricate_leaky_fix` doc-comment + line markers) | `TextCorrectionRecord` |
| `marque/src/render.rs:1132` | Yes | v2 shape |

**`AuditNote` test-fixture sites** (NOT migrated in D):

| File:line | Reason untouched |
|---|---|
| `crates/engine/tests/audit_note_sealing_carve_out.rs:96` | `AuditNote` is a separate line type (PR 3.7); D reshapes `AppliedFix`, not `AuditNote` |
| `crates/engine/tests/audit_note_sealing_carve_out.rs:147` | Same |

**`promote-callsite-lint` test corpus** (`tools/promote-callsite-lint/tests/callsite_test.rs`):
NOT migrated. The lint's own test corpus exercises the lint's pattern-matching;
the calls are intentionally malformed test inputs to the lint, not real
audit-record promotions. Stable across D.

---

## Appendix B — Master plan §9 OQ resolution summary

| OQ | Resolution | Located at |
|---|---|---|
| OQ-4 (test-fixture migration strategy) | Single D6 commit, split if friction; per user mid-flight directive | D-D-8 |
| OQ-5 (decoder vs strict dispatch shape) | Provenance derived from `FixSource` at emit time, no recognizer-time branch | D-D-9 |
| OQ-6 (WASM build verification for blake3) | No new CI job; existing `wasm-pack build` + SC-008 parity covers; new parity test at D5 | D-D-10 |

---

**End of architect preflight.**
