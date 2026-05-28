<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Contract: Audit Record (NDJSON, schema `marque-3.0`)

**Active schema**: `marque-3.0` (was `marque-2.0` pre-PR-B).
**Active as of**: PR B merge (recognition-axis cutover, 2026-05-28).
**Spec FRs**: FR-002, FR-004, FR-026, FR-034, FR-035, FR-035a, FR-037, FR-041, FR-044, FR-049
**Audience**: compliance auditors, NDJSON consumers (CLI piping, WASM postMessage embedders, log-aggregation pipelines), security/integrity reviewers.

PR B (2026-05-28) bumped the schema from `marque-2.0` to `marque-3.0` to carry the `Confidence → Recognition` two-PR cleanup. The audit-record `"confidence"` sub-object drops the `rule` and `region` fields — strict-path emissions pin `recognition = 1.0` (PR A landed the value collapse, PR B retired the surface). The 2-tuple `RuleId` structured-object `"rule"` field carries forward unchanged from `marque-2.0`. Pre-PR-B binaries cannot read `marque-3.0` records and vice versa; the cutover is a clean break under FR-037.

T044 (2026-05-22) was the prior atomic cutover from `marque-1.0` to `marque-2.0`, carrying the FR-026 / FR-044 `RuleId` 2-tuple migration. The rule field shifted from a flat string (`"rule": "E054"`) to a structured `{"scheme": "...", "predicate_id": "..."}` object. T044 unfroze FR-049's stability commitment for a single atomic PR; the freeze re-engaged at T044's merge.

The earlier PR 3c.2.D cutover (`marque-mvp-3` → `marque-1.0`, 2026-05-20) baked in the four FR-035a structural commitments — `Canonical<S>` provenance wired into audit emit, BLAKE3 digesting of pre-fix and canonical bytes, closed-set `MessageTemplate` JSON serialization, and the `AppliedFix` v2 reshape with the `AppliedTextCorrection` split — all of which carry forward unchanged into `marque-3.0`.

Per FR-037 every pre-cutover envelope (`mvp-1` / `mvp-2` / `mvp-3` / `marque-1.0` / `marque-2.0`) is not interoperable with `marque-3.0` binaries (clean break, no `marque-audit-reader` crate scheduled).

---

## Schema identifier

```text
"schema": "marque-3.0"
```

`MARQUE_AUDIT_SCHEMA` is build-time-pinned to a single value via
`marque-engine::AUDIT_SCHEMA_VERSION` (FR-034). One binary emits
exactly one schema. The build-time accept-list at HEAD is
`["marque-2.0"]` (was `["marque-1.0"]` pre-T044); pre-cutover records
are unreadable by post-cutover binaries (FR-037 — clean break, no
`marque-audit-reader` crate scheduled).

---

## NDJSON record shape

One JSON object per line. Records are append-only and emitted in
construction order by `Engine::fix_inner`; never reordered post-
promotion (I-5).

```jsonc
{
  "schema": "marque-2.0",

  "rule": {
    "scheme": "capco",
    "predicate_id": "portion.dissem.noforn-conflicts-rel-to"
  },

  "severity": "error",                // "off" | "suggest" | "info" | "warn" | "error" | "fix"

  "span": { "start": 1024, "end": 1037 },

  "fix": {
    "replacement": {
      "discriminant": "strict",        // "strict" | "decoder"

      "canonical": {
        "source": "cve",                // "cve" | "open_vocab"

        // when source = "cve":
        "token_id": "Classification.Secret",

        // when source = "open_vocab":
        // "category": "SciSubCompartment",
        // "render_call_site": "marque-capco/src/render.rs:142",

        "bytes_digest": "blake3:0e2c…"  // BLAKE3 of rendered bytes; bytes themselves never in record
      },

      "confidence": {
        "recognition": 0.95,
        "rule": 1.00,
        "combined": 0.95,
        "region": null,                  // optional posterior region marker
        "runner_up_ratio": null,         // optional
        "features": ["StrictExactMatch"] // closed-set FeatureId labels
      }
    },

    "original_span": { "start": 1024, "end": 1037 },   // span only, no bytes (FR-004)
    "original_digest": "blake3:b78f…"                   // BLAKE3 of pre-fix bytes
  },

  "message": {
    "template": "BannerRollupMismatch",          // closed enum (FR-003)
    "args": {                                            // closed-set scalar/ID types only
      "token": null,                                     // generic token slot (data-model MessageArgs.token)
      "expected_token": "Classification.Secret",
      "actual_token": null,
      "category": "Classification",
      "span": null,
      "digest": null,
      "confidence": null,
      "feature_ids": []
    }
  },

  // Note: AppliedFix carries no `citation` field. Citation lives on
  // `Diagnostic` (the lint-phase surface) and is mechanically verified
  // by the citation lint (FR-018), not propagated into the audit
  // record. See data-model.md §AppliedFix v2.

  "timestamp": "2026-05-02T14:32:11Z",

  "classifier_id": "12345",              // present when MARQUE_CLASSIFIER_ID is set
  "dry_run": false
}
```

---

## Rule ID encoding

`marque-2.0` uses the 2-tuple `(scheme, predicate_id)` structured-object
form (FR-026 / FR-044 / R-3; landed at T044, 2026-05-22):

```jsonc
"rule": {
  "scheme": "capco",
  "predicate_id": "banner.classification.usa-trigraph"
}
```

- `scheme`: lowercase short name. `"capco"` for the only in-tree CAPCO
  scheme today; future schemes use their own short name (`"cui"`,
  `"nato"`, etc.). **`"engine"`** and **`"test"`** are reserved sentinel
  schemes (FR-044 + T044 PM decisions); neither is a valid
  `MarkingScheme` registration target. `"engine"` is used exclusively
  for synthetic engine-minted diagnostics (R001, R002, …); `"test"`
  is used by `#[cfg(test)]` fixtures and never reaches production
  audit output.
- `predicate_id`: dot-separated `<surface>.<category>.<predicate>`
  lowercase string. `<surface>` ∈
  `{ banner, portion, page, marking, closure }` for scheme rules
  (the `closure` surface was added by T044 PM OD-1 refinement so
  closure-operator inferences don't conflate with strict page-banner
  rules at the predicate level). `<category>` matches the lattice /
  axis category for surface rules
  (`classification | sci | sar | dissem | fgi | nato | aea |
  declassification | fouo | banner-rollup | metadata`).
  `<predicate>` is descriptive English-with-hyphens. For the
  `"engine"` sentinel scheme, predicate IDs follow `<class>.<predicate>`
  shape — e.g., `recognition.decoder-recognized`, `fix.reparse-failed`.

Engine-minted synthetic diagnostics carry the sentinel `"engine"`
scheme per FR-044:

```jsonc
"rule": { "scheme": "engine", "predicate_id": "recognition.decoder-recognized" }   // R001
"rule": { "scheme": "engine", "predicate_id": "fix.reparse-failed" }                // R002
```

Note: per T044 PM OD-4, the `r001` / `r002` numeric placeholders from
the pre-T044 spec wording were dropped. The `scheme = "engine"` tuple
already carries the cross-version anchor; descriptive
`<class>.<predicate>` reads better at audit-log triage.

Rationale for the sentinel scheme: R001/R002 are minted by
`marque-engine`, not by a `MarkingScheme` impl. Inheriting the active
scheme's namespace (e.g., `("capco", "engine.r001.…")`) would lie
about provenance — the diagnostic is *about* a CAPCO marking but isn't
*from* CAPCO. The sentinel scheme keeps `("capco", …)` cleanly
meaning "from a CAPCO rule" and is forward-compatible with future
schemes.

**Canonical wire-string form** (text contexts only — `.marque.toml`
`[rules]` keys, CLI text output, log lines): `<scheme>:<predicate_id>`
with a colon separator, produced by the `RuleId::Display` impl. JSON
audit records always use the structured 2-tuple shape, never the wire
string.

**JSON field-ordering note**: the CLI's `DiagnosticJson` emits
struct-order (`scheme` first, then `predicate_id`) because it
serializes through a typed `RuleIdJson<'a>` struct in
`marque/src/render.rs`. The audit-record NDJSON path that flows
through `serde_json::Value` (a `BTreeMap`-backed shape) emits
alphabetical order (`predicate_id` first, then `scheme`). Both shapes
are valid JSON and parse identically; consumers that branch on
field order MUST handle both.

The one-time mapping table from the legacy flat-string `E###` /
`W###` / `S###` / `C###` / `R###` / catalog-row-label /
test-fixture-id forms to their 2-tuple successors lives at
`docs/refactor-006/legacy-rule-id-map.md`. The map is a living
document — appended-to, never silently rewritten (see T044 plan
§5 R-4).

---

## T044 cutover history (2026-05-22, FR-049 unfreeze)

Historical context for archaeologists reading pre-T044 audit logs.

Before T044, `RuleId` was a 1-tuple `(&'static str)` wrapping a flat
string like `"E054"`, `"W003"`, `"R001"`. The `marque-1.0` audit
schema emitted `"rule": "E054"` as a flat JSON string. T044
(2026-05-22) unfroze FR-049 for a single atomic PR that:

1. Reshaped `RuleId` to a 2-tuple `(scheme, predicate_id)` struct in
   `crates/rules/src/lib.rs`.
2. Bumped `MARQUE_AUDIT_SCHEMA` from `marque-1.0` to `marque-2.0` in
   `crates/engine/build.rs` (single-value accept-list per FR-034).
3. Migrated 114 rule IDs across `marque-capco`, `marque-engine`,
   `marque-rules`, the CLI, and WASM — see
   `docs/refactor-006/legacy-rule-id-map.md` for the rename rows.
4. Migrated 67 corpus `expected.json` fixtures to the structured
   shape.
5. Migrated the engine sentinels: `"R001"` →
   `("engine", "recognition.decoder-recognized")` and `"R002"` →
   `("engine", "fix.reparse-failed")` (T044 PM OD-4: numeric prefix
   dropped).
6. Simplified the engine constraint-bridge dispatcher at
   `crates/engine/src/engine.rs` from a translation table to a
   no-op pass-through (T044 PM OD-8): catalog row labels ARE the
   predicate IDs.

The freeze re-engaged at T044's merge. Subsequent renames require a
coordinated `marque-2.1` (additive) or `marque-3.0` (breaking)
audit-schema bump.

---

## `replacement.canonical.source` discriminator

Two cases distinguish closed-CVE (high trust) from open-vocab
(trust-on-render-site):

**`source: "cve"`**:
```jsonc
"canonical": {
  "source": "cve",
  "token_id": "<Vocabulary<S>::TokenId display form>",
  "bytes_digest": "blake3:..."
}
```

The `token_id` field is the display form of the `TokenId` (e.g.,
`"Classification.Secret"`, `"DissemControl.Noforn"`). Auditors can
mechanically verify the rendered bytes from `(token_id, scope)` against
`Vocabulary<S>::lookup` and `MarkingScheme::render_canonical`.

**`source: "open_vocab"`**:
```jsonc
"canonical": {
  "source": "open_vocab",
  "category": "<CategoryId display form>",
  "render_call_site": "marque-capco/src/render.rs:142",
  "bytes_digest": "blake3:..."
}
```

The `render_call_site` is the source location of the
`MarkingScheme::render_canonical` call that produced the bytes. This is
the explicit accommodation of the "open-vocab residual" — the audit
consumer can inspect the render call site to evaluate trust in the
specific render path.

**`bytes_digest`** is mandatory for both source variants. The
deterministic content-ignorance canary scan (SC-001) verifies by
construction that the literal canonical bytes never appear in the
NDJSON output — only their BLAKE3 digest does.

---

## `message.template` closed enum

`MessageTemplate` (FR-003) is a closed Rust enum bake-extracted from
the existing diagnostic catalog at PR 3c implementation per R-2. The
JSON wire form is `MessageTemplate::as_str()` returning the Rust
variant name verbatim (PM-D-12). The examples below are real
variants shipping in `crates/rules/src/message.rs`:

```jsonc
"template": "BannerRollupMismatch"
"template": "ClassificationFloorViolated"
"template": "NonCanonicalOrder"
"template": "ConflictsWith"
"template": "RequiredByPresence"
"template": "SupersededToken"
"template": "ReparseFailed"             // R002 (PR 7)
"template": "DecoderRecognized"         // R001
/* ... see crates/rules/src/message.rs for the full closed catalog */
```

Adding a new variant requires a coordinated `MARQUE_AUDIT_SCHEMA` bump
(would constitute a `marque-1.1` per semver — out of scope for this
refactor, but the mechanism is in place).

`message.args` carries only the closed permitted scalar/ID types:
`token`, `expected_token`, `actual_token`, `category`, `span`,
`digest`, `confidence`, `feature_ids`. (Field name is `token` per
data-model.md §MessageArgs and source plan §8.3 — distinct from
`canonical.token_id` which is a separate field on the canonical
replacement.) No `String`, no `&str`, no input-byte-derived field.
Unused args are `null` (or omitted via JSON serialization options —
implementer's call at PR 3c).

---

## Content-ignorance canary

The deterministic NDJSON canary scan (FR-002, SC-001):

```text
For each AppliedFix record emitted by Engine::fix_inner during a
corpus regression sweep:
  - Serialize to NDJSON.
  - Scan the serialized line for any contiguous byte sequence ≥ 4 bytes
    long that appears in the corresponding input document but does NOT
    appear within:
      - a JSON numeric span value (start/end integers)
      - a "blake3:..." digest hex string
      - a closed-set enumerated identifier value (token_id, category,
        template, etc.)
  - Any match is a leak. Fail the canary.
```

Test-fabricated `AppliedFix` records constructed under the Constitution
V Principle V carve-out (`#[cfg(test)]` modules, `tests/` integration
files) are excluded from the canary by construction — the canary
operates on `Engine::fix_inner`'s emitted `Vec<AppliedFix>` stream
only.

---

## Permitted identifier types (Constitution V Principle V)

Per Constitution V Principle V (G13 invariant), permitted
identifier types in audit output are:

- Token canonicals (rendered via `Vocabulary<S>::render_canonical`,
  trusted)
- Category IDs (e.g., `Classification`, `SciSubCompartment`)
- Span offsets (start/end byte indices into the original buffer)
- BLAKE3 digests (of canonical bytes; of original bytes)
- Posterior scalars (recognition / rule / combined confidence values)
- Enumerated `FeatureId` labels (closed Rust enum)
- Enumerated `MessageTemplate` labels (closed Rust enum, FR-003)

Forbidden in audit output:
- Document content bytes (verbatim or decoded)
- Document metadata field values (Subject, Author, etc.)
- Subject-claim free-form text
- Unenumerated identifier strings
- `format!`-interpolated content

---

## `Severity::Suggest` variant (FR-042)

The `marque-1.0` schema introduced a `Severity::Suggest` variant
distinct from the prior set (`Off | Info | Warn | Error | Fix`); this
variant carries forward unchanged into `marque-2.0`. Its
NDJSON serialization is the lowercase string `"suggest"`. Semantics:

- **Source**: produced by `Engine::fix_inner` when a pass-2
  `Phase::WholeMarking` rule's fix span overlaps a pass-1 fix span (I-18
  / FR-022 demotion). Surfaced in CLI / IDE / WASM output as a
  user-actionable suggestion that the engine declined to auto-apply.
- **Sort order**: below `Info` for severity-comparison purposes
  (advisory, not informational).
- **Exit-code semantics**: does NOT trigger `EX_DIAG_WARN`.
  `Severity::Suggest` is non-blocking — distinct from `Severity::Warn`
  which exits with `EX_DIAG_WARN` per `marque/src/main.rs`. CI pipelines
  treating warnings as failures continue to do so; suggestions remain
  advisory regardless.
- **Audit-record presence**: `Severity::Suggest` records appear in the
  audit log alongside auto-applied fixes (the suggestion happened — the
  engine just chose not to splice it). Auditors can distinguish
  suggestions from auto-applied fixes by the `severity` field.

The full post-refactor severity set is `Off | Suggest | Info | Warn |
Error | Fix`.

---

## Pre-cutover compatibility

There is none. Per FR-037:
- No `marque-audit-reader` crate is scheduled.
- Pre-cutover envelopes (`marque-mvp-2`, `marque-mvp-3`, `marque-1.0`)
  are unreadable by post-cutover `marque-2.0` binaries.
- This is a type-level guarantee, not a runtime concern: there are no
  pre-cutover records (no users, no deployment) at the time of cutover.
- The window for clean-break refactor closes when external consumers
  attach (per spec Assumptions); this refactor is the last clean-break
  window. After `marque-2.0`, subsequent bumps follow the semver
  cadence below — additive minor bumps stay forward-readable; the next
  breaking bump (`marque-3.0`) would again clean-break.

## Schema discoverability (D3)

Per **decision D3** in `decisions.md`, the active audit schema name
MUST be discoverable by external consumers without parsing audit
records.

**Per-record discoverability** (already in place): every record's
first field is `"schema": "marque-2.0"` (mandatory, FR-035 / T044).
Streaming NDJSON consumers detect schema by reading the first record
they see.

**Per-binary discoverability**: `marque --version` MUST expose the
active audit schema name in its output. Format choice (JSON,
key/value lines, or human-readable) is implementer's call; the
binding constraint is that the schema name appears such that:

- A shell script can grep for `marque-2.0` in `marque --version`
  output and detect schema-major changes.
- The schema name shown matches the value baked into
  `marque_engine::AUDIT_SCHEMA_VERSION` (FR-034) — single source of
  truth.

**Cutover changelog** (T044): the changelog entry MUST explicitly
state that the audit schema's `"schema"` field is the discriminator
external consumers should branch on, and that pre-cutover binaries
producing `marque-1.0` / `marque-mvp-*` records are not interoperable
with post-cutover `marque-2.0` binaries.

This closes the discoverability gap left by FR-037's "no reader crate"
posture: external consumers who do exist (per the no-consumers
attestation in D4 — none expected) get a mechanical signal that the
schema changed, even though no compatibility shim is provided.

---

## Schema-bump policy

`marque-2.0` is the active audit schema as of T044 (2026-05-22).
Subsequent schema bumps (`marque-2.1`, `marque-2.2`, `marque-3.0`)
follow semver:

- **Minor bump (`marque-2.x`)**: additive — new `MessageTemplate`
  variant, new `FeatureId` variant, new `MessageArgs` field type
  (still closed-set), additive `RuleId` predicate renames recorded
  in `docs/refactor-006/legacy-rule-id-map.md`. Reader compatibility
  is forward-only; `MARQUE_AUDIT_SCHEMA` validates against the exact
  bumped value.
- **Major bump (`marque-3.0`)**: structural — `RuleId` shape change,
  `Canonical<S>` shape change, removal of a permitted identifier type.
  Constitutes a clean-break event of the same magnitude as the prior
  `marque-1.0 → marque-2.0` (T044) and `marque-mvp-3 → marque-1.0`
  (PR 3c.2.D) cutovers.

PR 8's `marque-priors-3` is a separate priors-bake schema, not the
audit schema (FR-036). Audit-schema bumps and priors-bake schema bumps
are independent.
