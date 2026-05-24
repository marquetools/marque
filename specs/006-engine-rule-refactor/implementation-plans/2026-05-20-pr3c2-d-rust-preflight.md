<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 3c.2.D — Rust-Tactical Preflight Plan

**Date**: 2026-05-20
**Branch**: `refactor-006-pr3c2-d-atomic-cutover` (off `staging@fae9e334`)
**Status**: draft for PM review; complements the architect's tactical
plan at `docs/plans/2026-05-20-pr3c2-d-architect-preflight.md` (parallel
authorship, no formal ordering).

**Master plan**: `docs/plans/2026-05-19-pr3c2-plan-and-decisions.md` §1 row
**D** + Appendix B (the `marque-1.0` target JSON shape) + §2 PM decisions
D25.1–D25.7 + §4 risk register + §9 open questions OQ-1..OQ-6.

**Charter sources**:
- `specs/006-engine-rule-refactor/spec.md` FR-002, FR-004, FR-035,
  FR-035a, FR-037, FR-040, FR-049, FR-052, FR-053
- `specs/006-engine-rule-refactor/contracts/audit-record.md` body §
  ("NDJSON record shape" under "Contract: Audit Record (NDJSON, schema
  `marque-1.0`)")
- `.specify/memory/constitution.md` Principle II (Zero-Copy), V
  (Audit-First Compliance, G13), VII (Crate Discipline)

This document is the Rust-mechanical companion to the architect plan.
The architect covers scope, decomposition, risk register, reviewer
attestation. This plan covers type signatures, derive macros, serde
representation, the cargo dep graph, lifetimes, generics, visibility,
WASM build implications, and the test-fixture migration mechanics.

---

## 0. Charter recap (Rust-tactical scope)

PR 3c.2.D performs the **atomic** schema cutover from `marque-mvp-3` to
`marque-1.0`. The lift covers:

1. Build-time const flip (`crates/engine/build.rs`) — accept-list +
   default both move to `["marque-1.0"]` / `"marque-1.0"`.
2. `AUDIT_SCHEMA_IS_V3` boolean retires (replace with
   `AUDIT_SCHEMA_IS_V1_0`); all four call sites flip atomically.
3. `SchemaVersionId` enum (in `marque-scheme`) gains a `V1_0` variant
   and rotates the default; every match site is forced to update via
   `#[non_exhaustive]` exhaustiveness.
4. `AppliedFix<S>` v2 reshape — drop `AppliedFixProposal<S>` envelope,
   introduce `AppliedFixDetail<S>` + `AppliedReplacement<S>` carrying
   `Canonical<S>` + `Discriminant` + `Blake3Hash` digest fields.
5. Non-marking text corrections split into a separate type
   `AppliedTextCorrection` with its own engine-only constructor
   (PM-D-4 resolution recommended below; OQ-4 / OQ-5).
6. BLAKE3 digesting wired — real `blake3::hash(...)` calls replace
   `Blake3Hash::zero()` placeholders at construction sites.
7. Audit NDJSON emit refactored to render `Message`-typed
   `Diagnostic.message` as `{"template": "...", "args": {...}}`.
8. T055 deterministic NDJSON canary added — verifies no document
   bytes appear verbatim in any NDJSON record across the corpus
   (G13 type-invariant proof).
9. Every test-fixture `__engine_promote` site (8 sites at HEAD)
   migrates to the v2 shape atomically.
10. Audit-record contract doc edits — `contracts/audit-record.md` §0
    retires, §1+ becomes the active spec.

Out-of-scope for D: 2-tuple `RuleId` migration (post-PR-10 per
FR-049); R001/R002 `"engine"` sentinel scheme labels (same).

---

## 1. Inventory verification (HEAD reconciliation, R-4)

Re-grepped at HEAD `fae9e334` (PR 3c.2.C merge). Reconciled against
PR-0's `docs/refactor-006/promote-callsite-inventory.md` (PR 0 baseline:
3 production sites + 3 test sites at PR 0 HEAD). Drift across PRs 7–
3c.2.C is significant: the inventory has grown to **4 production
`__engine_promote` sites + 1 production token-mint helper + 9
test-fixture sites + 2 PR 3.7 `AuditNote::__engine_promote` test
sites** (a different sealing target — separate ledger entry).

### 1.1 Production `__engine_promote` sites (allowed)

| File:line at HEAD | Surface | Function | Lint disposition |
|---|---|---|---|
| `crates/engine/src/engine.rs:2189` | `AppliedFix::__engine_promote_text_correction` | `Engine::apply_text_corrections` | ALLOWED — production carve-out |
| `crates/engine/src/engine.rs:3177` | `AppliedFix::__engine_promote` | `TwoPassFixer::apply_pass_fixes` (was `Engine::fix_inner` at PR 0; refactored to the two-pass shape during PR 7b) | ALLOWED |
| `crates/engine/src/engine.rs:3709` | `EnginePromotionToken::__engine_construct` | `engine_promotion_token()` helper | ALLOWED — the single grant site per PR 0 doc |

**Delta vs PR-0 baseline (3 sites → 3 sites)**:
- Net production count unchanged at 3.
- The PR 0 baseline listed 3 `__engine_promote` sites in `Engine::fix_inner`
  (apply mode + dry-run mode + apply_text_corrections). PR 7b's two-pass
  rewrite collapsed the apply/dry-run distinction into a single
  promotion site at `TwoPassFixer::apply_pass_fixes:3177` that handles
  both modes (the `dry_run` arg flips per-call). The text-correction
  site moved from `Engine::apply_text_corrections:1211` (PR 0) to
  `Engine::apply_text_corrections:2189` (HEAD); the function name is
  unchanged, the file location drifted ~1000 lines down.

### 1.2 Test-fixture `__engine_promote` sites (carve-out)

| File:line at HEAD | Surface | Carve-out marker present? | Consumer (constraint-2 verification) |
|---|---|---|---|
| `crates/engine/src/engine.rs:7632` | `AppliedFix::__engine_promote` | Yes (`crates/engine/src/engine.rs:7627-7630`) | `contributing_pass1_rule_ids_*` unit tests in same `#[cfg(test)]` mod |
| `crates/engine/tests/audit.rs:478-479` | `AppliedFix::__engine_promote_text_correction` (token mint adjacent) | Yes (`crates/engine/tests/audit.rs:467-477`) | `fabricate_leaky_fix` → G13 sentinel-sweep `#[should_panic]` |
| `crates/rules/tests/engine_promotion_seal.rs:140, 142` | token mint + `AppliedFix::__engine_promote` | Yes (file-level doc) | Seal-acceptance test exercising the type-level token gate |
| `marque/src/render.rs:1131-1132` | token mint + `AppliedFix::__engine_promote` | Yes (`marque/src/render.rs:1130`) | `render_audit_record_produces_valid_ndjson` |

**Delta vs PR-0 baseline (3 sites → 4 sites)**:
- Net test-fixture count grew by 1.
- The new site is `crates/engine/src/engine.rs:7632` — a
  `#[cfg(test)]`-gated `synth_applied_fix` helper inside the engine
  crate's unit-test module, feeding the `assemble_r002_result` /
  `contributing_pass1_rule_ids_*` test suite (PR 7b added these).
  The carve-out comment is present at lines 7627–7630 and is
  lint-conformant.

### 1.3 `AuditNote::__engine_promote` sites (separate sealing target)

PR 3.7 introduced `AuditNote<S>` at `crates/rules/src/audit_note.rs`
as a parallel-but-distinct audit-line type for engine-internal notes.
It carries its own `__engine_promote` constructor that reuses the
same `EnginePromotionToken` seal. **These sites are unaffected by PR
3c.2.D's `AppliedFix` reshape** — they sit alongside, on the same
seal mechanism, with their own JSON shape contract. Inventory:

| File:line at HEAD | Surface | Disposition |
|---|---|---|
| `crates/rules/src/audit_note.rs:225` | `AuditNote::__engine_promote` definition | Untouched in PR 3c.2.D |
| `crates/engine/tests/audit_note_sealing_carve_out.rs:96, 147` | Test fixtures | Untouched in PR 3c.2.D |

The carve-out comments at `:94` and `:145` mention "marque-mvp-3 →
marque-1.0 alongside BLAKE3 digesting" — these are forward-looking
doc-comment references that need a doc-comment update in PR 3c.2.D
to reflect the cutover landing, but no code change to the
`__engine_promote` call itself.

### 1.4 `Blake3Hash` consumers (HEAD)

```text
crates/rules/src/message.rs:87        — type definition + Display + zero() ctor
crates/rules/src/message.rs:404       — `MessageArgs.digest: Option<Blake3Hash>` field
crates/rules/src/message.rs:628, 645  — unit tests
crates/rules/src/lib.rs:100           — re-export
crates/rules/tests/message_args_closed_set.rs:29, 50, 77 — closed-set destructuring pin
```

**`Blake3Hash::zero()` call sites today**: 2 (`message.rs:628` test
+ `message_args_closed_set.rs:77` test). Both are in test code. No
production code calls `zero()`; the placeholder has never been wired
into a real audit record. PR 3c.2.D adds the production wire-up;
the `zero()` constructor itself stays (test-fixture utility).

### 1.5 `marque-mvp-3` literal references (HEAD)

24 distinct references across `*.rs` + `*.toml` + `*.md`:
- `crates/engine/build.rs:24-25` — accept-list + default (build-time
  validation surface; PR 3c.2.D's primary edit point).
- `crates/engine/src/lib.rs:75, 90, 98` — `AUDIT_SCHEMA_VERSION` /
  `AUDIT_SCHEMA_IS_V3` doc + impl.
- `crates/engine/src/text_correction.rs:8` — historical doc.
- `crates/engine/tests/audit_schema_accept_list.rs:25, 31, 42, 47` —
  drift gate tests asserting the accept-list contains exactly
  `["marque-mvp-3"]`.
- `crates/engine/tests/audit_note_sealing_carve_out.rs:18` —
  forward-looking doc comment.
- `crates/wasm/src/lib.rs:440, 518, 548` — WASM audit-record JSON
  shape comment + emission site + drift-gate.
- `marque/src/main.rs:101-102` — CLI `--version` help text
  (currently lists `marque-mvp-1` | `marque-mvp-2` only; doc is
  stale relative to mvp-3 landing — separate cleanup, not D's
  primary work).
- `marque/src/render.rs:396, 400, 410, 426, 642, 677` — render
  pipeline doc comments + `applied_fix_to_audit_json_v3` impl.
- `marque/tests/cli_fix.rs:449, 472, 475` — CLI integration test
  that runs the binary with `MARQUE_AUDIT_SCHEMA=marque-mvp-X` to
  verify rejection of out-of-list values (NB: lines 472, 475 still
  reference `mvp-2`; stale fixture, drift gate).
- `crates/rules/src/lib.rs:51, 64, 706, 772, 1118` — doc references
  in `AppliedFix` / `Diagnostic` doc comments.
- `crates/scheme/src/render_context.rs:169, 170, 184, 246` —
  `SchemaVersionId::MarqueMvp3` enum variant + `as_str()` + test.
- `crates/rules/src/audit_note.rs:60, 61, 141, 142` — `AuditNote`
  doc-comment references to the cutover.
- `crates/capco/src/rules.rs:9379` — `#[cfg(any())]`-gated stub
  doc comment.
- `crates/engine/src/decoder.rs:1208, 3471` — historical doc
  references to `marque-mvp-2` (stale, separate cleanup; not D's
  primary work but should be updated in the doc-edit sweep).

**Edit scope at D**: every `marque-mvp-3` literal in `*.rs` flips to
`marque-1.0` atomically. Every `AUDIT_SCHEMA_IS_V3` identifier
renames to `AUDIT_SCHEMA_IS_V1_0`. `SchemaVersionId::MarqueMvp3`
gains a sibling `V1_0`, the `as_str()` arm adds `"marque-1.0"`, and
the engine-side default flips.

### 1.6 `Canonical<S>` consumers (HEAD)

```text
crates/scheme/src/canonical.rs:187    — Canonical<S> definition (bytes / source / scope / _scheme PhantomData)
crates/scheme/src/canonical.rs:194    — impl block
crates/scheme/src/canonical.rs:245    — Canonical::from_cve(token, scope, bytes) — public ctor
crates/scheme/src/canonical.rs:263    — Canonical::from_render(...) — pub(crate)
crates/scheme/src/canonical.rs:467    — EngineConstructor::build_open_vocab impl (sealed)
crates/scheme/tests/canonical_unconstructable.rs:125-126, 152 — compile-fail proofs
```

**Production consumers at HEAD**: zero. The type ships in
`marque-scheme` but no rule emits it, no engine path reads it. PR
3c.2.D is the first production wire-up.

---

## 2. Type definitions (v2 shape)

### 2.1 Where the v2 types live

**Recommendation: all v2 marking-side types co-locate in `marque-rules`
alongside `AppliedFix<S>`.** `AppliedTextCorrection` co-locates in the
same crate.

**Rationale**:
1. The engine-only seal (`EnginePromotionToken`) lives in `marque-
   rules`; the new constructors need to consume it (FR-040 contract).
   Splitting the types into `marque-engine` would require either a
   re-export shim or a cross-crate seal redesign.
2. `marque-engine` already depends on `marque-rules`; the reverse
   would violate Constitution VII (`marque-rules` is leaf-side of
   the rule chain).
3. Consumer surfaces (`marque/src/render.rs`,
   `crates/wasm/src/lib.rs`, `crates/engine/src/output.rs`) all
   import `AppliedFix` from `marque-rules` today — no path changes.
4. Audit-record JSON serialization stays in the surface crates
   (`marque/src/render.rs` for CLI, `crates/wasm/src/lib.rs` for
   WASM); the wire-format projection is consumer-controlled, the
   data shape is type-controlled.

### 2.2 `Discriminant` enum

New closed enum in `marque-rules`:

```rust
/// Replacement provenance discriminator. Distinguishes strict-
/// recognizer-derived fixes from decoder-fallback fixes per
/// `contracts/audit-record.md` `marque-1.0` shape.
///
/// The "strict" arm covers every `Recognizer` impl that returns a
/// single deterministic canonical (today: `StrictRecognizer`); the
/// "decoder" arm covers probabilistic recognition
/// (`DecoderRecognizer`).
///
/// # Why closed (no `#[non_exhaustive]`)
///
/// Closed enum: adding a variant is an audit-schema bump. The
/// `MARQUE_AUDIT_SCHEMA` accept-list at `crates/engine/build.rs`
/// MUST bump in lockstep with any new variant. Matches the
/// closed-set discipline on [`crate::MessageTemplate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Discriminant {
    /// Strict recognizer produced this fix from a deterministic
    /// canonical lookup (no posterior). `confidence.recognition = 1.0`.
    Strict,
    /// Decoder produced this fix from a probabilistic posterior.
    /// `confidence.recognition < 1.0`; the runner-up-ratio + feature
    /// contributions are populated.
    Decoder,
}

impl Discriminant {
    /// Audit-emit string. Pinned by
    /// `crates/rules/tests/discriminant_audit_string.rs` to prevent
    /// silent rename. Matches `contracts/audit-record.md` shape.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Decoder => "decoder",
        }
    }
}
```

**Derive macros**: `Debug, Clone, Copy, PartialEq, Eq, Hash`. All
trivially derivable; the enum is plain-data and Copy by construction.

**Visibility**: `pub` (exported via `marque-rules` prelude).

**`#[non_exhaustive]`**: NO. Closed-set discipline per the
`MessageTemplate` precedent — adding a variant is a coordinated
audit-schema bump.

### 2.3 `AppliedReplacement<S>`

```rust
/// Provenance + canonical-replacement payload inside an
/// [`AppliedFixDetail<S>`].
///
/// Carries the [`Discriminant`] (strict vs decoder) and the
/// [`Canonical<S>`] value the engine rendered. Replaces the current
/// `proposal: AppliedFixProposal<S>` envelope's `FixIntent` arm for
/// the marking-side audit-record path.
///
/// # Type parameter
///
/// `S: MarkingScheme` — `Canonical<S>` is scheme-typed; this type
/// inherits the scheme parameter. The `S` bound matches the
/// `AppliedFix<S>` outer bound exactly so no per-impl re-declaration
/// is needed.
///
/// # Why no `#[non_exhaustive]`
///
/// Pure data struct; field set is the v2 shape per
/// `contracts/audit-record.md`. Adding a field is an audit-schema
/// bump (closed-set discipline; matches [`MessageArgs`]). External
/// brace construction is already blocked by `Canonical<S>` being
/// sealed via `EngineConstructor` — `AppliedReplacement` cannot be
/// brace-constructed by an external crate even without
/// `#[non_exhaustive]`, because no public path exists to construct
/// the `canonical` field outside the engine.
#[derive(Debug)]
pub struct AppliedReplacement<S: MarkingScheme> {
    /// Strict vs decoder provenance.
    pub discriminant: Discriminant,
    /// The engine-rendered canonical replacement. Sealed
    /// construction per `marque_scheme::canonical`.
    pub canonical: Canonical<S>,
    /// Confidence snapshot at promotion time (cloned from the
    /// originating `FixIntent.confidence`).
    pub confidence: Confidence,
}

// Manual Clone — see the AppliedFix<S> rationale; the derive
// over-constrains to `S: Clone`. Canonical<S> already implements
// Clone via its own derive (it does not require `S: Clone`).
impl<S: MarkingScheme> Clone for AppliedReplacement<S> {
    fn clone(&self) -> Self {
        Self {
            discriminant: self.discriminant,
            canonical: self.canonical.clone(),
            confidence: self.confidence.clone(),
        }
    }
}
```

**Derive macros**: `Debug` derived. `Clone` is **manual** (matches
the existing `AppliedFix<S> / AppliedFixProposal<S>` pattern — the
derive macro over-constrains to `S: Clone`, which would break
`S = CapcoScheme` since `CapcoScheme` is intentionally not `Clone`).
`PartialEq` / `Eq` deliberately NOT derived — `Canonical<S>` derives
`PartialEq` per `canonical.rs:186`, but `Confidence` only derives
`PartialEq` via `derive(PartialEq, Clone, Debug)` (verify at
implementation time); if both are `PartialEq` then `AppliedReplacement`
can opt in for test ergonomics. **Tactical recommendation**: defer
the `PartialEq` derive until a test surface needs it; YAGNI.

**Visibility**: `pub`. The fields are `pub` because the audit-record
JSON projection in `marque/src/render.rs` reads them directly (no
accessor method needed). Constitution VII allows this — the type
is in `marque-rules`, accessed by `marque` (CLI) which depends on
`marque-rules` transitively through `marque-engine`.

### 2.4 `AppliedFixDetail<S>`

```rust
/// The "marking" arm of an [`AppliedFix<S>`] — replaces the
/// `AppliedFixProposal::FixIntent` variant of the pre-v2 envelope.
///
/// # Why a separate struct (not inlined into AppliedFix)
///
/// The audit-record contract at `contracts/audit-record.md` shapes
/// the JSON as `{ "fix": { "replacement": {...}, "original_span":
/// ..., "original_digest": ... } }` — `fix` is a nested object,
/// not a flat field set. Matching the JSON shape at the type level
/// (rather than via custom `Serialize`) keeps the relationship
/// debuggable and the serde derive trivial.
#[derive(Debug)]
pub struct AppliedFixDetail<S: MarkingScheme> {
    /// The canonical replacement payload + provenance.
    pub replacement: AppliedReplacement<S>,
    /// Byte span the fix targeted in the source buffer.
    pub original_span: Span,
    /// BLAKE3 digest of the pre-fix bytes at `original_span`.
    /// Computed by the engine at promotion time. Constitution V
    /// Principle V — the digest is the audit anchor for "which
    /// bytes were rewritten" without storing the bytes themselves.
    pub original_digest: Blake3Hash,
}

// Manual Clone — see the AppliedFix<S> rationale.
impl<S: MarkingScheme> Clone for AppliedFixDetail<S> {
    fn clone(&self) -> Self {
        Self {
            replacement: self.replacement.clone(),
            original_span: self.original_span,
            original_digest: self.original_digest,
        }
    }
}
```

**Derive macros**: `Debug` derived, `Clone` manual.

**Visibility**: `pub` with `pub` fields (rationale per
`AppliedReplacement` above).

### 2.5 `AppliedFix<S>` v2

```rust
/// A promoted [`FixIntent<S>`] with runtime context — the
/// marking-side audit record.
///
/// Constructed **only** by `Engine::fix_inner` at the moment a fix
/// meets the confidence threshold. See [`Self::__engine_promote`]
/// for the engine-only contract and the Constitution V Principle V
/// test-fixture carve-out.
///
/// # Wire format
///
/// Serializes to the `marque-1.0` NDJSON shape per
/// `contracts/audit-record.md` body §. The renderer in
/// `marque/src/render.rs` (CLI) and `crates/wasm/src/lib.rs` (WASM)
/// projects this type into the `{ "type": "applied_fix", ... }`
/// NDJSON record. The two emitters produce byte-identical output
/// (SC-008 parity invariant).
///
/// # Generic over the marking scheme
///
/// `AppliedFix<S>` is generic so the embedded [`Canonical<S>`] (via
/// `AppliedReplacement::canonical`) preserves the scheme-typed
/// payload across crate boundaries.
#[non_exhaustive]
#[derive(Debug)]
pub struct AppliedFix<S: MarkingScheme> {
    /// Rule ID. Snapshot at the top level for audit-emit ergonomics
    /// (the renderer does not have to descend into `fix.replacement`
    /// for the audit-cardinality field).
    pub rule: RuleId,
    /// Severity at promotion time (snapshot from the originating
    /// `Diagnostic.severity`; survives the lint-post-pass severity
    /// rewrite at FR-008 / D-7.6).
    pub severity: Severity,
    /// Byte span in the original source buffer.
    pub span: Span,
    /// The marking-side fix detail (replacement + digest +
    /// original_span). NEW in v2 — replaces the
    /// `proposal: AppliedFixProposal<S>` envelope's
    /// `FixIntent(intent)` arm.
    pub fix: AppliedFixDetail<S>,
    /// Provenance of the originating rule emission.
    pub source: FixSource,
    /// Diagnostic message — closed-template, closed-args. Snapshot
    /// from the originating `Diagnostic.message: Message`. Audit
    /// emitters render via `Message::template()` + `Message::args()`
    /// accessors.
    pub message: Message,
    /// Timestamp of application (clock-injected).
    pub timestamp: SystemTime,
    /// Classifier identity from runtime config.
    pub classifier_id: Option<Arc<str>>,
    /// `true` if produced under `--dry-run`.
    pub dry_run: bool,
    /// Caller-supplied input identifier (file path, "-" for stdin).
    pub input: Option<Arc<str>>,
}

// Manual Clone preserved verbatim from v1 shape.
impl<S: MarkingScheme> Clone for AppliedFix<S> {
    fn clone(&self) -> Self {
        Self {
            rule: self.rule.clone(),
            severity: self.severity,
            span: self.span,
            fix: self.fix.clone(),
            source: self.source,
            message: self.message.clone(),
            timestamp: self.timestamp,
            classifier_id: self.classifier_id.clone(),
            dry_run: self.dry_run,
            input: self.input.clone(),
        }
    }
}
```

**Field deletions vs v1**:
- `proposal: AppliedFixProposal<S>` → REMOVED (replaced by `fix:
  AppliedFixDetail<S>` on the marking path; text corrections move
  to their own type per §2.6).
- `confidence: Confidence` → MOVED to `fix.replacement.confidence`
  (avoids duplication; the JSON contract puts it inside the `fix`
  sub-object).
- `migration_ref: Option<&'static str>` → REMOVED. The `marque-1.0`
  contract per `contracts/audit-record.md` body § does not emit a
  top-level `migration_ref` field. Migration provenance flows
  through `message.args.token` / `expected_token` (the closed-set
  audit channel) — `migration_ref: Some("§F.1 p41")` was a stand-in
  for citation provenance that PR 3c.2.C's typed `Citation` on
  `Diagnostic` already covers cleanly. Carrying it on
  `AppliedFix` is redundant.

**Field additions vs v1**:
- `severity: Severity` → ADDED. The `marque-1.0` contract emits
  `"severity": "error"` at the top level (per the example in
  `contracts/audit-record.md` body §). v1 omitted it; v2 carries it.
- `message: Message` → ADDED. The `marque-1.0` contract emits
  `{"message": {"template": "...", "args": {...}}}` at the top
  level. v1 omitted it (the diagnostic stream and audit stream were
  parallel-but-disjoint surfaces).

**`#[non_exhaustive]`**: KEEP. Matches v1; reserves grow-path for a
future hash-axis addition or audit field. Combined with the closed
constructor (engine-only `__engine_promote`), external code cannot
brace-construct an `AppliedFix` even before `#[non_exhaustive]` —
the attribute is belt-and-suspenders here.

### 2.6 `AppliedTextCorrection` (separate NDJSON line type)

PM-D-4 resolution (OQ-4): **adopt the Rust type split** — a separate
`AppliedTextCorrection` type with its own `__engine_promote_text_correction`
constructor, NOT a thin emit-time enum at the renderer.

**Rationale**:
1. Constitution V Principle V (G13) clarity: marking-side audit
   records and text-correction audit records carry different
   permitted identifiers. Marking records carry token canonicals,
   category IDs, BLAKE3 digests, confidence scalars. Text-correction
   records carry corpus-derived canonical replacement strings (e.g.
   `"SECRET"` replacing `"SERCET"`). The type-level separation
   makes the audit-content boundary checkable at compile time —
   `AppliedTextCorrection` has no `canonical: Canonical<S>` field
   because it never carries a `Canonical<S>`; it carries a raw
   corpus canonical string.
2. The `Discriminant::Strict | Discriminant::Decoder` distinction
   is about marking recognition provenance. Text corrections sit
   outside that discriminator — they run pre-scanner. Forcing a
   `Discriminant::TextCorrection` arm into the marking enum would
   semantically misuse the discriminator.
3. NDJSON consumers can dispatch on `{"type": "applied_fix"}` vs
   `{"type": "text_correction"}` cheaply without descending into
   sub-object structure.

```rust
/// Engine-internal text-correction audit record (C001 /
/// `[corrections]` map, and the closely-shaped E006 deprecation-
/// migration path).
///
/// Distinct from [`AppliedFix<S>`] (marking-side) — text
/// corrections run pre-scanner and carry corpus-derived canonical
/// replacement strings rather than [`Canonical<S>`] payloads.
///
/// # Not generic over the scheme
///
/// `AppliedTextCorrection` is NOT generic over `S`. The text-
/// correction path operates on raw bytes pre-scanner; no scheme-
/// typed payload is involved.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct AppliedTextCorrection {
    /// Rule ID (typically C001 for `[corrections]`-map matches;
    /// rule-emitted text corrections carry their own ID).
    pub rule: RuleId,
    /// Severity at promotion time.
    pub severity: Severity,
    /// Byte span the correction targeted in the source buffer.
    pub span: Span,
    /// BLAKE3 digest of the pre-correction bytes at `span`.
    pub original_digest: Blake3Hash,
    /// Canonical replacement bytes — corpus-derived token canonical
    /// (Constitution V's permitted-identifier list).
    pub replacement: SmolStr,
    /// Provenance.
    pub source: FixSource,
    /// Confidence snapshot.
    pub confidence: Confidence,
    /// Migration reference (§-citation, for E006 deprecation path);
    /// `None` for C001 corrections-map matches.
    pub migration_ref: Option<&'static str>,
    /// Diagnostic message — closed template, closed args. Text-
    /// correction records emit `MessageTemplate::CorrectionsApplied`
    /// (C001) or `MessageTemplate::SupersededToken` (E006).
    pub message: Message,
    /// Timestamp of application.
    pub timestamp: SystemTime,
    /// Classifier identity.
    pub classifier_id: Option<Arc<str>>,
    /// Dry-run flag.
    pub dry_run: bool,
    /// Caller-supplied input identifier.
    pub input: Option<Arc<str>>,
}

impl AppliedTextCorrection {
    /// Engine-only promotion path.
    ///
    /// # Reserved name (FR-040 lint contract)
    ///
    /// `__engine_promote_text_correction` is the FR-040 reserved
    /// name. The promote-callsite-lint at
    /// `tools/promote-callsite-lint/src/callsite.rs` flags every
    /// call to this method outside the engine's production carve-
    /// out and the Constitution V Principle V test-fixture carve-
    /// out. See the parallel doc on
    /// [`AppliedFix::__engine_promote`].
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn __engine_promote_text_correction(
        rule: RuleId,
        severity: Severity,
        span: Span,
        original_digest: Blake3Hash,
        replacement: SmolStr,
        source: FixSource,
        confidence: Confidence,
        migration_ref: Option<&'static str>,
        message: Message,
        timestamp: SystemTime,
        classifier_id: Option<Arc<str>>,
        dry_run: bool,
        input: Option<Arc<str>>,
        _token: EnginePromotionToken,
    ) -> Self {
        Self {
            rule,
            severity,
            span,
            original_digest,
            replacement,
            source,
            confidence,
            migration_ref,
            message,
            timestamp,
            classifier_id,
            dry_run,
            input,
        }
    }
}
```

**FR-040 lint coverage**: the lint at
`tools/promote-callsite-lint/src/callsite.rs:11` already greps the
name `__engine_promote_text_correction` (last-segment match per
the doc comment at `crates/rules/src/lib.rs:858`). The PR D edit
**relocates** the function from `impl AppliedFix<S>` to `impl
AppliedTextCorrection` — the function-name match still fires, no
lint surface change needed. **Verify** at implementation time: the
lint's existing test suite passes against the relocated signature.

### 2.7 `AppliedFix::__engine_promote` (v2 signature)

```rust
impl<S: MarkingScheme> AppliedFix<S> {
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn __engine_promote(
        rule: RuleId,
        severity: Severity,
        span: Span,
        // The original FixIntent — engine reads `.confidence`,
        // `.source`, `.message`, and `.replacement` from it. The
        // engine ALSO computes:
        //   - original_digest = blake3::hash(&source[span])
        //   - canonical = render_intent_to_canonical(intent, scheme, ctx)
        // and threads both into the AppliedFixDetail.
        intent: FixIntent<S>,
        // Pre-fix bytes at `span`. The engine slices `source` at
        // promotion time and passes them; AppliedFix never stores
        // the bytes — the digest is computed at construction and
        // the slice is dropped.
        original_bytes: &[u8],
        // Engine renders the canonical at promotion time and
        // passes it in. Constructing Canonical<S> requires an
        // EngineConstructor<S> (sealed), so this signature keeps
        // the construction path engine-only by argument shape.
        canonical: Canonical<S>,
        discriminant: Discriminant,
        timestamp: SystemTime,
        classifier_id: Option<Arc<str>>,
        dry_run: bool,
        input: Option<Arc<str>>,
        _token: EnginePromotionToken,
    ) -> Self {
        let original_digest = Blake3Hash(*blake3::hash(original_bytes).as_bytes());
        let replacement = AppliedReplacement {
            discriminant,
            canonical,
            confidence: intent.confidence,
        };
        let fix = AppliedFixDetail {
            replacement,
            original_span: span,
            original_digest,
        };
        Self {
            rule,
            severity,
            span,
            fix,
            source: intent.source,
            message: intent.message,
            timestamp,
            classifier_id,
            dry_run,
            input,
        }
    }
}
```

**Signature changes vs v1**:
- ADDS `severity: Severity` (carries the post-lint-pass severity
  for top-level audit-emit).
- ADDS `original_bytes: &[u8]` (engine slices and hashes at
  promotion time; never stored).
- ADDS `canonical: Canonical<S>` (engine renders pre-promotion).
- ADDS `discriminant: Discriminant`.
- DROPS `intent`'s move into `AppliedFixProposal::FixIntent` —
  the intent is consumed for `.confidence` / `.source` /
  `.message` / `.replacement` and dropped.

**Parameter ordering rationale**: keep `intent: FixIntent<S>`
adjacent to `original_bytes` + `canonical` + `discriminant` because
all four flow through the marking-promotion arm together. The
clock + classifier + dry-run + input + token suffix preserves the
v1 ordering — minimizes diff churn at the 4 production + 4
test-fixture call sites.

**OQ-5 resolution** (decoder-path canonical construction): the
engine's `apply_pass_fixes` at `engine.rs:3177` receives a
`FixIntent<S>` from either the strict or decoder recognizer. The
caller knows which one fired (the `Recognizer` impl is the active
strategy at fix time). PR D plumbs a `Discriminant` argument from
the recognizer dispatch site to the promotion site. Implementation
sketch: store `Discriminant` alongside each `Diagnostic` in the
two-pass `TwoPassFixer` state, or thread it through `KeptFix`
(the engine's internal per-fix scratch shape at the kept-fixes
collector). Mechanically cheaper: thread through `KeptFix`. The
choice is engine-internal and does not affect the `marque-rules`
type surface.

---

## 3. Serde representation (audit JSON shape)

### 3.1 The JSON contract (target shape)

Per `contracts/audit-record.md` body §, the `marque-1.0`
applied_fix line:

```jsonc
{
  "schema": "marque-1.0",
  "rule": "E054",
  "severity": "error",
  "span": { "start": 1024, "end": 1037 },
  "fix": {
    "replacement": {
      "discriminant": "strict",
      "canonical": {
        "source": "cve",
        "token_id": "Classification.Secret",
        "bytes_digest": "blake3:0e2c…"
      },
      "confidence": { "recognition": 0.95, "rule": 1.00, "combined": 0.95, "region": null, "runner_up_ratio": null, "features": ["StrictExactMatch"] }
    },
    "original_span": { "start": 1024, "end": 1037 },
    "original_digest": "blake3:b78f…"
  },
  "message": { "template": "BannerMissingClassification", "args": { "expected_token": "Classification.Secret", "category": "Classification" } },
  "timestamp": "2026-05-02T14:32:11Z",
  "classifier_id": "12345",
  "dry_run": false
}
```

### 3.2 Recommendation: hand-written JSON projection, NOT `#[derive(Serialize)]` on the in-memory types

**Reasoning**:
1. The in-memory `AppliedFix<S>` carries `Arc<str>` (classifier_id,
   input), `SystemTime` (timestamp, needs RFC3339 formatting),
   `Canonical<S>` (sealed type with no `Serialize` impl), and
   `Confidence` (custom serialization with the `region`,
   `runner_up_ratio`, `features` partial-emit logic). A direct
   `Serialize` derive would either require `Serialize` impls on
   every nested type (including `Canonical<S>`, which sits in
   `marque-scheme`) or `#[serde(serialize_with = "...")]`
   attributes on every field.
2. The renderer at `marque/src/render.rs:617` already follows the
   "intermediate JSON-shaped struct + `derive(Serialize)`" pattern
   (`AuditRecordJsonV3` → `serde_json::to_vec(&v3)`). PR D
   redoes the JSON struct under the new shape:
   `AuditRecordJsonV1_0` carrying `FixJson` carrying
   `ReplacementJson` carrying `CanonicalJson { source, token_id_or_category_etc, bytes_digest }`.
3. The CLI and WASM emitters share the same render module
   (`marque/src/render.rs` for CLI; `crates/wasm/src/lib.rs`
   imports the relevant projection helpers); keeping the JSON shape
   centralized in the renderer crate avoids duplicate Serialize
   impls and matches the SC-008 parity invariant.

### 3.3 Canonical projection (the trickiest piece)

`Canonical<S>` is sealed and has no `Serialize` impl. Audit
emission needs to project it into:

```jsonc
"canonical": {
  "source": "cve",          // when TokenSource::Cve
  "token_id": "Classification.Secret",
  "bytes_digest": "blake3:…"
}
```

OR

```jsonc
"canonical": {
  "source": "open_vocab",   // when TokenSource::OpenVocab
  "category": "SciSubCompartment",
  "render_call_site": "marque-capco/src/render.rs:142",
  "bytes_digest": "blake3:…"
}
```

The `bytes_digest` is `blake3::hash(canonical.bytes())` —
**always emitted**, never the raw bytes. The `token_id` field
projection (`"Classification.Secret"`) requires looking up the
`TokenId` against the active `Vocabulary<S>` to produce the
namespaced string form. Today the audit-record contract example
shows it as `"Classification.Secret"`, but **`Vocabulary<S>` does
not yet expose a `token_to_qualified_string` accessor** — that
projection helper needs to land in PR 3c.2.D.

**Implementation sketch**:
```rust
fn project_canonical_to_json<S: MarkingScheme>(
    canonical: &Canonical<S>,
    vocab: &S::Vocabulary,
) -> serde_json::Value {
    let bytes_digest = blake3::hash(canonical.bytes().as_bytes());
    let digest_str = Blake3Hash(*bytes_digest.as_bytes()).to_audit_string();
    match canonical.source() {
        TokenSource::Cve(token_id) => {
            // Vocabulary lookup — produces "Category.TokenName" form
            let token_label = vocab.qualified_token_label(*token_id);
            serde_json::json!({
                "source": "cve",
                "token_id": token_label,
                "bytes_digest": digest_str,
            })
        }
        TokenSource::OpenVocab { category, render_call_site } => {
            let category_label = vocab.category_label(*category);
            let call_site_str = format!(
                "{}:{}",
                render_call_site.file(),
                render_call_site.line(),
            );
            serde_json::json!({
                "source": "open_vocab",
                "category": category_label,
                "render_call_site": call_site_str,
                "bytes_digest": digest_str,
            })
        }
    }
}
```

The `render_call_site` projection uses `&'static Location<'static>` —
that's `#[track_caller]`-captured at `EngineConstructor::
build_open_vocab`. The `Location::file()` returns
`&'static str`; the `format!(...)` here is the only string
allocation on the audit-emit path per record. Constitution V
Principle V permits emitting the call site (it is engine source
code metadata, not document content).

### 3.4 Message projection

`Message` is `template: MessageTemplate` + `args: MessageArgs`.
Per the JSON contract:

```jsonc
"message": {
  "template": "BannerMissingClassification",
  "args": {
    "expected_token": "Classification.Secret",
    "category": "Classification"
  }
}
```

Empty optional fields elide:
- `args.token: Option<TokenId>` → emit `"token": "..."` when
  `Some`; omit when `None`.
- `args.feature_ids: SmallVec<[FeatureId; 4]>` → emit
  `"feature_ids": ["..."]` when non-empty; omit when empty.
- Same for `expected_token`, `actual_token`, `category`, `span`,
  `digest`, `confidence`, `contributing_rule_ids`.

The renderer iterates over `args` fields and conditionally inserts
JSON keys. `serde_json::Map<String, Value>` is the right shape;
build it directly.

### 3.5 Note on `MessageTemplate`'s wire string

The renderer emits `MessageTemplate::as_str()` (e.g.
`"BannerMissingClassification"`). The contract example shows
`"BannerMissingClassification"`, NOT `"banner_rollup_mismatch"` or
similar — verify at implementation time that the existing
`MessageTemplate::BannerRollupMismatch.as_str() == "BannerRollupMismatch"`
matches what `contracts/audit-record.md` expects. **Likely mismatch**:
the contract example writes `"BannerMissingClassification"` but the
existing enum variant is `BannerRollupMismatch`. **Disposition**:
treat the contract example as illustrative, not pinned — the
`as_str()` form on `MessageTemplate::BannerRollupMismatch` ships
as the wire-form. Architect plan reviewer: flag if PM disagrees.

---

## 4. Crate-graph impact

### 4.1 `blake3` dep (per-crate)

Workspace `Cargo.toml:145` already declares:
```toml
blake3 = { version = "1", default-features = false, features = ["pure"] }
```

(Verified at HEAD via `grep -rn 'blake3' --include='Cargo.toml'`.)

PR D adds the per-crate consumer entries:

| Crate | Reason to add |
|---|---|
| `marque-rules` | `AppliedFix::__engine_promote` calls `blake3::hash(...)` |
| `marque-engine` | The engine slices `source[span]` and feeds `__engine_promote` (the hash happens inside `marque-rules`, but engine code may construct `Blake3Hash` directly for `original_digest` if the slice-then-hash is moved engine-side — defer to implementation) |

**Recommendation**: keep the `blake3::hash(...)` call inside
`AppliedFix::__engine_promote`'s body in `marque-rules`. The
engine passes `original_bytes: &[u8]`; the rules crate does the
hash. This co-locates the digest-construction logic with the
type that owns the digest field and keeps the engine's audit-
promotion logic readable as plumbing.

The `marque-wasm` crate consumes `AppliedFix` transitively
through `marque-engine`; it does not need a direct `blake3` dep
unless its renderer projects `Canonical::bytes()` separately.
**Verify at implementation time**: if
`crates/wasm/src/lib.rs:440-548` reads from `Canonical<S>::bytes()`
(rather than the precomputed `bytes_digest`), it needs `blake3`.
Otherwise the dep flows transitively.

### 4.2 `blake3` WASM-safety verification

The workspace dep already specifies `default-features = false,
features = ["pure"]`. That selects the pure-Rust SIMD-free
implementation. blake3 v1.x `["pure"]` mode is known to compile to
`wasm32-unknown-unknown`. Constitution III WASM-safe set
(`marque-ism`, `marque-core`, `marque-rules`, `marque-scheme`,
`marque-capco`) — `marque-rules` is in the set, so adding
`blake3` to `marque-rules`'s direct deps is a Constitution III
risk if the `pure` mode regresses. **Mitigation**: PR 3c.2.A
already established the workspace dep; the wasm-pack build job
at `.github/workflows/ci.yml:498-506` will exercise the
`marque-rules` → `blake3` path at first build. Watch the build
log; on regression, gate by `cfg(target_arch = "wasm32")` —
unlikely to be needed but the contingency is one line.

### 4.3 `Canonical<S>` consumer path

`Canonical<S>` is defined in `marque-scheme`. `AppliedReplacement<S>`
references it. `marque-rules` depends on `marque-scheme` (verified
at HEAD via the existing `Canonical` re-exports through
`marque_rules`). No new dependency edge needed.

The two open-vocab construction paths
(`EngineConstructor::build_open_vocab` and
`Canonical::from_render`) both stay where they are. PR D adds the
engine-side renderer call site that *invokes* `build_open_vocab`
(today there are zero production callers). The implementer
threads an `EngineConstructor<S>` through `Engine::fix_inner`'s
KeptFix-to-AppliedFix promotion path.

### 4.4 `MarkingScheme::canonicalize` path

PR 3c.2.B / 3c.2.C added `MarkingScheme::canonicalize(ParsedAttrs<'_>)
-> CanonicalAttrs`. PR D does NOT touch this trait method. The
canonicalize path is parser-output → canonical-attrs; PR D's
canonical-rendering path is rule-emission → `Canonical<S>` →
audit record. Distinct surfaces.

### 4.5 Constitution VII boundary check

- `marque-rules` adds `Discriminant` + `AppliedReplacement<S>` +
  `AppliedFixDetail<S>` + `AppliedTextCorrection` types. `marque-rules`
  already depends on `marque-scheme` (via `MarkingScheme` trait); no
  new edge.
- `marque-engine` consumes the new types via its existing
  `marque-rules` dep. No new edge.
- `marque-scheme` adds `SchemaVersionId::V1_0` variant.
  `#[non_exhaustive]` on the enum means downstream crates re-match
  the new variant; that's intended ripple.
- `marque-capco`, `marque-core`, `marque-ism` are untouched (they
  don't construct `AppliedFix`).
- `marque/src/render.rs` (CLI) — touches the rendering layer
  (consumer of `marque-rules` via `marque-engine`).
- `crates/wasm/src/lib.rs` — same.
- `crates/server/src/...` — untouched (the server crate carries no
  `AppliedFix`-shape-specific code per `grep`).

No circular deps. No new edges.

---

## 5. Migration mechanics (per test-fixture site)

OQ-4 resolution: **per-site sequential migration**, not codemod.

**Rationale**: Each of the 4 test-fixture sites carries a unique
combination of (a) carve-out comment shape, (b) the synthetic
`AppliedFix` value's intended downstream use, and (c) constraint-2
verification (never commingled). A codemod (rust-analyzer,
morphllm) would migrate the type construction mechanically but
cannot verify the constraint-2 invariant. Per-site review is the
correct discipline here.

Each migration step:

1. Read the test fixture's enclosing scope (the `#[test]` fn or
   the `#[should_panic]` fn that consumes the fabricated
   `AppliedFix`).
2. Identify the new fields the v2 constructor requires:
   - `severity` (snapshot from the test's expected severity, or
     `Severity::Error` if unspecified).
   - `original_bytes` (the test's source buffer slice at `span`).
   - `canonical` (construct via `Canonical::from_cve(...)` — the
     public path; tests don't need `EngineConstructor` because the
     test is not exercising the seal).
   - `discriminant` (`Discriminant::Strict` for every test today;
     decoder tests are downstream).
   - `message` (the test's expected message; today the v1 shape
     has `Message::new(...)` on `Diagnostic` — propagate the same).
3. Verify the carve-out comment is still 1-line-above the call
   per FR-040 lint window.
4. Run `cargo test -p <crate>` to confirm the fixture compiles
   and the assertions still hold.

### 5.1 Per-site migration list

| Site | Migration complexity | Notes |
|---|---|---|
| `crates/engine/src/engine.rs:7632` (`synth_applied_fix`) | LOW | Helper fn already exists; signature expands. The two consuming tests (`contributing_pass1_rule_ids_dedupes_and_sorts`, `contributing_pass1_rule_ids_caps_at_inline_capacity_4`) test the assembler, not the audit shape — they should pass after the constructor expansion. |
| `crates/engine/tests/audit.rs:478-479` (`fabricate_leaky_fix`) | MEDIUM | `fabricate_leaky_fix` produces a deliberately G13-violating `AppliedFix` for the sentinel-sweep `#[should_panic]` test. v2 makes G13 violations harder to fabricate (no public path for an unvocabulary-bound `Canonical<S>`); the test fixture may need to switch to fabricating `AppliedTextCorrection` (which carries the freeform `replacement: SmolStr`) rather than `AppliedFix<S>`. **Action**: read the test's expected panic message and decide which audit type the leak should land on. |
| `crates/rules/tests/engine_promotion_seal.rs:140-142` | LOW | Seal-acceptance test exercising the token-gate; mechanical type-shape update. |
| `marque/src/render.rs:1131-1132` (`render_audit_record_produces_valid_ndjson`) | MEDIUM-HIGH | This test validates the NDJSON wire shape via `serde_json::Value` field lookups (`v["proposal"]["kind"]`, etc.). The v2 shape changes the JSON keys — the assertions update wholesale to the new contract (`v["fix"]["replacement"]["discriminant"]`, `v["fix"]["original_digest"]`, `v["message"]["template"]`, etc.). |

### 5.2 Discovery mechanism

`cargo check --tests --workspace` after the type change surfaces
every site that still expects the v1 shape. The v2 shape is a
**structural breaking change** — every `AppliedFix { proposal,
confidence, migration_ref, .. }` brace pattern at a non-`pub(crate)`
boundary fails compilation. No silent migration possible.

**rust-analyzer is NOT useful for codemod here** because the
`AppliedFix { proposal, .. }` field shape change forces compile
errors at every consumer site; `cargo check --tests` is the
discovery tool. **Do NOT add a `cargo xtask` migration script** —
the 4 sites are small enough that per-site sequential edits are
cheaper than the script.

### 5.3 `AuditNote` doc-comment cleanup

`crates/rules/src/audit_note.rs:60-61, 141-142` carries doc-comment
references to "`marque-mvp-3`; a future precursor PR bumps to
`marque-1.0` per the PR 3.7 plan". The doc text needs an update
to reflect that the cutover has now landed. **NOT a code change**,
but should land in the same PR for doc-coherence.

---

## 6. WASM / CI risks

### 6.1 `wasm32-unknown-unknown` build coverage

`.github/workflows/ci.yml:498-506` runs:
```yaml
- wasm-pack build crates/wasm --target web --profiling
- wasm-pack test --node crates/wasm --test parity
```

The job uses the `--profiling` profile (not `release-web`), which
preserves debug symbols but still compiles `marque-rules` → `blake3`
to `wasm32-unknown-unknown`. PR 3c.2.A landed the workspace dep;
the wasm build will exercise it as soon as a downstream crate
adds a direct dep entry. PR D's per-crate Cargo.toml edits trigger
the first WASM compile of `blake3` `["pure"]`.

**Failure modes**:
- `blake3 1.x` requires Rust 1.66+; workspace MSRV is 1.85
  (`rust-version = "1.85"`); MSRV satisfied.
- `["pure"]` feature path uses no platform intrinsics; WASM-safe
  by construction.
- Risk: a transitive build-script dep that drags in a non-WASM-safe
  crate. **Mitigation**: `cargo tree -p marque-rules --target wasm32-unknown-unknown`
  reveals the actual graph; run as a verification step at PR open.

### 6.2 WASM size budget (PM D25.7)

PM-D7 set a ≤5% WASM size budget on the cutover. The dominant
addition is `blake3` `["pure"]` (~50–80 KB compressed estimate
per the PM contract). The size measurement happens at the
`wasm-pack build crates/wasm --target web --profile release-web`
step (separate `release-web` profile per `Cargo.toml:130-140`
WASM-size discipline reservation). The CI surface lives at
`.github/workflows/ci.yml:512-522`. Report the delta in the PR
description; per D25.7 the measurement is at D only (A/B/C/E
don't materially change WASM size).

### 6.3 `marque --version` schema-discoverability

`marque/src/main.rs:101-102` carries `--version` help text that
still references `mvp-1 | mvp-2` (stale relative to mvp-3). PR D
updates this to `marque-1.0` (and only `marque-1.0` — the
accept-list is single-value per FR-014). **Verify**: the help-text
update is a doc surface, not a behavior surface; do not promote
to a feature flag. Single-line edit.

### 6.4 `MARQUE_AUDIT_SCHEMA` env-var rejection test

`marque/tests/cli_fix.rs:449, 472, 475` carries a CLI integration
test that runs the binary with `MARQUE_AUDIT_SCHEMA=<value>` and
asserts rejection for out-of-list values. The test today references
`mvp-2` (line 472, 475) as a known-rejected value. PR D updates
the test data to reference `marque-mvp-3` as the new known-rejected
value (now that `marque-mvp-3` is no longer accepted).

### 6.5 Build-script `MARQUE_AUDIT_SCHEMA` mismatch panic

`crates/engine/build.rs:24-25` panics at build time on an
out-of-list `MARQUE_AUDIT_SCHEMA`. Developers who set the env var
explicitly (e.g., to debug a prior schema's shape) hit the panic
post-merge. **Mitigation**: the panic message at
`crates/engine/build.rs:24` carries the accept-list contents; the
message becomes self-explanatory after the flip.

---

## 7. Compile-fail doctests (Rust-mechanical invariants)

Propose adding these doctests to pin v2 invariants:

### 7.1 No `From<v1> for v2`

`crates/rules/src/lib.rs` (on `AppliedFix<S>`):

```rust
/// **No `From<AppliedFixProposal<S>> for AppliedFixDetail<S>` impl.**
/// The v1 envelope is gone; there is no automatic conversion.
///
/// ```compile_fail
/// # use marque_rules::AppliedFixDetail;
/// # // AppliedFixProposal no longer exists at marque-1.0
/// let _: AppliedFixDetail<()> = todo!();
/// ```
```

Stronger form: since `AppliedFixProposal<S>` is **deleted** in PR D,
any reference to it from outside the crate is automatically a
compile error. The compile-fail proof is the deletion itself.

### 7.2 No `Default for AppliedFix<S>`

```rust
/// **No `Default for AppliedFix<S>` impl.** AppliedFix is engine-
/// promoted only.
///
/// ```compile_fail
/// # use marque_rules::AppliedFix;
/// # struct StubScheme;
/// # impl marque_scheme::MarkingScheme for StubScheme { /* ... */ }
/// let _: AppliedFix<StubScheme> = AppliedFix::default();
/// ```
```

### 7.3 External crates cannot brace-construct `AppliedFix`

```rust
/// **External crates cannot brace-construct `AppliedFix`.**
/// `#[non_exhaustive]` rejects literal-construction from outside
/// the defining crate.
///
/// ```compile_fail
/// # use marque_rules::{AppliedFix, RuleId, Severity};
/// # use marque_ism::Span;
/// let _: AppliedFix<()> = AppliedFix {
///     rule: RuleId::new("E001"),
///     severity: Severity::Error,
///     span: Span::new(0, 0),
///     fix: todo!(),
///     source: todo!(),
///     message: todo!(),
///     timestamp: todo!(),
///     classifier_id: None,
///     dry_run: false,
///     input: None,
/// };
/// ```
```

(Doctests compile as separate crates; `#[non_exhaustive]` rejects
the brace pattern at the doctest crate boundary.)

### 7.4 No `Serialize for Canonical<S>`

`marque-scheme` is the right place for this doctest. `Canonical<S>`
must not gain a `Serialize` impl — that would invite consumers to
serialize the raw `bytes` rather than the digest, defeating G13.

```rust
/// **No `Serialize for Canonical<S>` impl.** The audit-emit path
/// projects `Canonical<S>` to a structured JSON shape with a
/// `bytes_digest` field rather than serializing the raw bytes.
///
/// ```compile_fail
/// # use marque_scheme::canonical::Canonical;
/// # use marque_scheme::MarkingScheme;
/// fn _take<S: MarkingScheme>() -> Vec<u8> {
///     let c: Canonical<S> = todo!();
///     serde_json::to_vec(&c).unwrap()
/// }
/// ```
```

### 7.5 `AppliedTextCorrection` and `AppliedFix<S>` are disjoint types

```rust
/// **`AppliedTextCorrection` is not coercible to `AppliedFix<S>`.**
/// The two audit-record types are distinct by construction; no
/// `From` impl exists.
///
/// ```compile_fail
/// # use marque_rules::{AppliedFix, AppliedTextCorrection};
/// fn _convert<S: marque_scheme::MarkingScheme>(t: AppliedTextCorrection) -> AppliedFix<S> {
///     t.into()
/// }
/// ```
```

### 7.6 `Discriminant` does not include a text-correction variant

```rust
/// **`Discriminant` is a 2-variant closed enum.** Adding a third
/// variant for text correction would conflate marking provenance
/// with fix kind (PM-D-4 rationale).
///
/// ```compile_fail
/// # use marque_rules::Discriminant;
/// let _ = Discriminant::TextCorrection;
/// ```
```

### 7.7 `__engine_promote_text_correction` is on `AppliedTextCorrection`, not `AppliedFix`

```rust
/// **`__engine_promote_text_correction` relocates to
/// `AppliedTextCorrection` at v2.** The old method-resolution path
/// (`AppliedFix::__engine_promote_text_correction(...)`) is gone.
///
/// ```compile_fail
/// # use marque_rules::AppliedFix;
/// let _ = AppliedFix::<()>::__engine_promote_text_correction(
///     /* args */
/// );
/// ```
```

(This compile-fail doctest is **load-bearing**: the FR-040 lint
flags the call by name, but the lint runs in CI; the doctest
catches at compile-test time inside `cargo test --doc`.)

---

## 8. OQ resolutions (per master plan §9)

### OQ-1: `MarkingScheme::canonicalize` default impl location

**Out of PR D's scope.** PR 3c.2.B already resolved this at the
trait-level; PR D doesn't touch the canonicalize path.

### OQ-2: `RenderContext::schema_version` typing

**Resolved by PR 3c.2.A landing**: enum (`SchemaVersionId`).
PR D adds the `V1_0` variant; no shape change.

### OQ-3: `Citation` placement

**Out of PR D's scope.** PR 3c.2.A / C landed `Citation` in
`marque-rules`. PR D leaves the placement unchanged.

### OQ-4: Test-fixture migration in D

**Recommended: per-site sequential**, NOT codemod. See §5 of this
plan.

**Rust-mechanical rationale**: the 4 sites are small (4 sites at
HEAD vs the PR 0 baseline of 3); each carries a unique consumer
purpose; the v2 shape is a structural breaking change that
`cargo check --tests` surfaces deterministically. A codemod
adds tooling overhead without saving review cycles.

### OQ-5: `Discriminant::Strict | Decoder` flow

**Resolved**: thread `Discriminant` through `KeptFix` (engine-internal
scratch shape) from the recognizer-dispatch site (`Recognizer<S>`
trait dispatch in `Engine::lint`) to the promotion site
(`TwoPassFixer::apply_pass_fixes:3177`). Plumbing only; no new
trait surface.

The decision point: at the recognizer call, the engine knows
which `Recognizer` impl returned the parsed candidate.
`StrictRecognizer` returns `Discriminant::Strict`;
`DecoderRecognizer` returns `Discriminant::Decoder`;
`StrictOrDecoderRecognizer` (the default dispatcher) returns
whichever path actually fired.

**Rust mechanic**: add a `discriminant: Discriminant` field to
`KeptFix` (or whatever the engine's per-fix-promotion scratch
struct is called at HEAD). The engine's pass-2 re-lint reads
the diagnostic's source-recognizer (today done implicitly via the
diagnostic shape, e.g. `R001` for decoder-recognized canonicals);
PR D makes the read explicit via the new field.

### OQ-6: WASM CI for `blake3`

**Existing `.github/workflows/ci.yml:498-506` covers it**: the
`wasm-pack build crates/wasm --target web --profiling` step
compiles every WASM-safe crate (including `marque-rules` after
PR D's per-crate dep add). No new CI job needed.

**Additional defensive verification at PR open**: run
`cargo tree -p marque-rules --target wasm32-unknown-unknown` and
attach the output to the PR description so reviewers can confirm
no non-WASM-safe transitive deps slipped in.

---

## 9. Hot-path perf note

Per D25.6 (bench informational, not blocking), capture and report
but do not auto-revert:

- `blake3::hash(&source[span])` per fix. At ~256 bytes/fix typical
  span size, the SIMD-free `["pure"]` path runs ~1–2 GB/s on
  modern x86; per-fix cost ~100–500 ns. Aggregate across
  ~10–100 fixes per 10 KB document: ~10–50 μs added to `fix_10kb`.
- Vocabulary lookup for `Canonical<S>` projection at audit-emit:
  one HashMap lookup per fix. Audit-emit happens AFTER `Engine::
  fix_inner` returns; not on the lint hot path.

Total expected delta on `fix_10kb`: ≤ 5% (well below the SC-001
16ms ceiling).

`lint_10kb` is unaffected (no audit promotion in lint path).

---

## 10. Canonicalization timing (Rust-mechanical OQ)

**Where does `blake3::hash(original_bytes)` happen?**

**Recommendation: at promotion time, inside `AppliedFix::__engine_promote`.**

Alternatives considered:

| Option | Pros | Cons |
|---|---|---|
| At promotion time (RECOMMENDED) | Engine has the source slice in scope (`splice_fixes_forward` already reads it); single allocation point; closes G13 at type-construction time. | Audit-emit reads digest from struct field — already cached. |
| At audit-emit time (in `marque/src/render.rs`) | Lazy — `--dry-run` without audit emit avoids the hash. | Requires `AppliedFix` to carry the original bytes, or a closure, defeating G13. |
| At rule-emission time | Pre-computed by rule. | Rule crates would need the source slice; they don't have it. Constitution VII boundary violation. |

**Where does `blake3::hash(canonical.bytes())` happen?**

Same answer: at promotion time, when the engine constructs the
`Canonical<S>` via `EngineConstructor::build_open_vocab(...)` or
`Canonical::from_cve(...)`. The digest is precomputed and stored
in the `AppliedReplacement` struct; audit-emit reads the stored
field. (Or — leaner — store only `Canonical<S>` and re-hash at
emit time, since `Canonical::bytes()` is a borrowed view. The
audit-emit path is off-hot-path; trade memory savings for one
hash recomputation.)

**Tactical recommendation**: store `bytes_digest` separately on
`AppliedReplacement` so the audit-emit path is allocation-free
beyond the JSON projection. The extra 32 bytes per record is
negligible compared to the convenience of `derive(Serialize)`
on the projection struct.

---

## 11. Crate-graph touch ledger (Constitution VII §IV check)

PR D touches the following crates:

| Crate | Touch type | Constitution VII justification |
|---|---|---|
| `marque-engine` | Bug-fix + signature changes at `engine.rs:2189, 3177, 3709` | Within-006 precedent: PR 7b refactored the same promotion sites for the two-pass model; PR D extends them for the v2 shape. Engine-crate touch is unavoidable for the engine-promotion seal mechanism. |
| `marque-rules` | New types (`Discriminant`, `AppliedReplacement<S>`, `AppliedFixDetail<S>`, `AppliedTextCorrection`); v2 reshape on `AppliedFix<S>`; new `__engine_promote` signature | The rules crate owns the audit-record types; this is its core responsibility. |
| `marque-scheme` | `SchemaVersionId::V1_0` variant | Trait surface gains an enum variant; ripple to every match site is the intended Constitution effect (forces every consumer to update). |
| `marque/src/render.rs` (CLI) | JSON projection refactor | Consumer-side wire-format projection; not an engine-crate edit. |
| `crates/wasm/src/lib.rs` | JSON projection refactor | Same. |
| `crates/engine/build.rs` | Build-time const flip | The single sanctioned engine-crate edit for the schema cutover. |
| `crates/engine/src/lib.rs` | `AUDIT_SCHEMA_IS_V3` → `AUDIT_SCHEMA_IS_V1_0` rename | Bookkeeping rename. |
| `crates/engine/tests/audit_schema_accept_list.rs` | Test data flip from `marque-mvp-3` to `marque-1.0` | Test-data update. |
| `marque/tests/cli_fix.rs` | Test-data flip | Test-data update. |
| `crates/rules/src/audit_note.rs` | Doc-comment update only | Doc-only. |
| `crates/capco/src/rules.rs:9379` | Doc-comment update only (`#[cfg(any())]`-gated stub) | Doc-only. |
| `crates/engine/src/decoder.rs:1208, 3471` | Doc-comment update (stale `mvp-2` references) | Doc-only. |
| `tools/promote-callsite-lint/` | Possibly: update test fixture data to reflect v2 signature | Verify whether the lint's own test suite at `tools/promote-callsite-lint/tests/callsite_test.rs:35-407` mocks the old 6-arg signature; if so, update to match the v2 8-arg signature. |

No new crate. No moved crate boundaries. Constitution VII upheld.

---

## 12. Implementation order (Rust-mechanical sequencing)

Within PR D, the implementation order that minimizes "type X is
broken but its consumer Y compiles transiently" is:

1. **Types first** (no consumer impact): add `Discriminant`,
   `AppliedReplacement<S>`, `AppliedFixDetail<S>`,
   `AppliedTextCorrection` alongside the existing v1 types in
   `marque-rules`. Add `SchemaVersionId::V1_0`. Add
   `Blake3Hash`-replacing-`zero()` ergonomics. Compile clean at
   this point (additive types only).
2. **Build-time const flip**: `crates/engine/build.rs:24-25` to
   `["marque-1.0"]` / `"marque-1.0"`. Rename
   `AUDIT_SCHEMA_IS_V3` → `AUDIT_SCHEMA_IS_V1_0`. **At this point
   `cargo check --workspace` breaks** at every consumer of the
   old const name; the breakage list IS the migration list.
3. **`AppliedFix::__engine_promote` signature change**: relocate
   the existing v1 body to a `__engine_promote_v1`-shaped private
   helper (or delete outright — there's no parallel-shipping
   period in pre-users land per
   `feedback_pre_users_no_deprecation_phasing.md`). Add the v2
   constructor. At this point `cargo check --workspace --tests`
   breaks at every `__engine_promote` call site (4 production +
   8 test-fixture).
4. **Production call sites migrate**: update
   `engine.rs:2189, 3177` to construct the v2 shape (pass
   `severity`, `original_bytes`, `canonical`, `discriminant`).
5. **Renderer migrate**: refactor `marque/src/render.rs` to emit
   the v2 JSON shape; refactor `crates/wasm/src/lib.rs` to match.
6. **Test fixtures migrate**: per §5 sequence; each fixture's
   `cargo test -p <crate>` run validates the migration.
7. **Doc-comment sweep**: every stale `marque-mvp-3` reference in
   `*.rs` doc comments updates to `marque-1.0` (PR D primary
   surface) or to historical-context phrasing (deeper code
   comments).
8. **T055 canary**: add the deterministic NDJSON scan that
   verifies no document bytes appear verbatim in any NDJSON
   record across the corpus. The canary lives at
   `crates/engine/tests/audit_g13_canary.rs` (new file).
9. **Contracts doc edit**: `specs/006-engine-rule-refactor/contracts/audit-record.md`
   §0 retires; the body section is the active spec.

This sequencing keeps `cargo check --workspace` informative at
each step (each break is a planned migration target).

---

## 13. Brief-back to PM

### Inventory delta

PR-0 baseline (`docs/refactor-006/promote-callsite-inventory.md`):
3 production + 3 test-fixture `__engine_promote` sites. **HEAD**:
3 production + 4 test-fixture sites + 1 PR 3.7-added `AuditNote`
parallel surface (untouched by PR D). Net delta: +1 test fixture
(`engine.rs:7632`, PR 7b's `synth_applied_fix` helper). All
carve-out comments lint-conformant at HEAD.

### Type-placement recommendation

**All four new types** (`Discriminant`, `AppliedReplacement<S>`,
`AppliedFixDetail<S>`, `AppliedTextCorrection`) **co-locate in
`marque-rules`** alongside the existing `AppliedFix<S>`. The
engine-only seal (`EnginePromotionToken`) lives in `marque-rules`;
splitting the types into `marque-engine` would require cross-crate
seal redesign and Constitution VII boundary changes. The renderer
crates (`marque`, `marque-wasm`) consume them via the existing
re-export chain.

### Highest Rust-mechanical risk

**`Vocabulary<S>::qualified_token_label` does not exist at HEAD.**
The audit JSON shape per `contracts/audit-record.md` shows
`"token_id": "Classification.Secret"` — a namespaced form. The
existing `Vocabulary<S>` surface (`marque-scheme`) exposes
per-token metadata (authority, owner, deprecation, URN, schema
version, portion/banner forms) but no "qualified label" accessor
that produces `Category.TokenName` form. PR D either (a) adds the
accessor (`fn qualified_token_label(&self, t: TokenId) -> &'static
str`) on the `Vocabulary` trait, or (b) renders `TokenId(N)` as
its numeric debug form in the audit record and lets consumers
resolve via the vocabulary surface separately. **Recommendation**:
option (a) — add the accessor, since the audit-record contract
already specifies the namespaced form. Implementation footprint:
~50 LOC in `marque-scheme` + the per-token-table build.rs entry
in `marque-ism`.

### Compile-fail doctests proposed

Seven (per §7), pinning:
1. No `From<AppliedFixProposal<S>> for AppliedFixDetail<S>`
2. No `Default for AppliedFix<S>`
3. External crates cannot brace-construct `AppliedFix<S>`
4. No `Serialize for Canonical<S>`
5. `AppliedTextCorrection` and `AppliedFix<S>` are disjoint
6. `Discriminant` excludes a text-correction variant
7. `__engine_promote_text_correction` is on `AppliedTextCorrection`,
   not on `AppliedFix`

Each is ≤10 lines; aggregate doctest cost negligible.

---

## Appendix A — Files PR 3c.2.D edits (count + classification)

| Classification | Count | Files |
|---|---|---|
| Type definitions | 1 | `crates/rules/src/lib.rs` (+ split candidate: extract new types into a `audit.rs` submodule if the file size exceeds the project's 800-line norm — `lib.rs` is already 1500+ lines so the split is recommended) |
| Trait-surface enum | 1 | `crates/scheme/src/render_context.rs` (`SchemaVersionId::V1_0`) |
| Build-time const | 1 | `crates/engine/build.rs` |
| Engine re-export const | 1 | `crates/engine/src/lib.rs` |
| Engine promotion sites | 1 | `crates/engine/src/engine.rs` |
| JSON renderers | 2 | `marque/src/render.rs`, `crates/wasm/src/lib.rs` |
| Test-fixture sites | 4 | See §5.1 |
| Drift-gate tests | 2 | `crates/engine/tests/audit_schema_accept_list.rs`, `marque/tests/cli_fix.rs` |
| Doc-only updates | 7+ | per §1.5 inventory |
| New test files | 1 | `crates/engine/tests/audit_g13_canary.rs` (T055) |
| Contract spec | 1 | `specs/006-engine-rule-refactor/contracts/audit-record.md` |
| Per-crate Cargo.toml | 1–2 | `crates/rules/Cargo.toml` (+ possibly `crates/engine/Cargo.toml`, `crates/wasm/Cargo.toml`) |

Aggregate: ~20–25 files touched; ~600–900 LOC net change (mostly
in the renderer's JSON projection refactor and the new T055
canary).
