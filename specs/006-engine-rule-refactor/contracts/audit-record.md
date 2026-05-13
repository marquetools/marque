<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Contract: Audit Record (NDJSON)

**Active schema**: `marque-mvp-3` (PR 3c.B Commit 10 — see §0).
**Post-keystone target**: `marque-1.0` (see body sections below).
**Spec FRs**: FR-002, FR-004, FR-026, FR-034, FR-035, FR-037, FR-041
**Audience**: compliance auditors, NDJSON consumers (CLI piping, WASM postMessage embedders, log-aggregation pipelines), security/integrity reviewers.

---

## §0. Active schema (`marque-mvp-3`) — PR 3c.B Commit 10

Landed in PR 3c.B Commit 10 atomically with the `FixProposal` cleanup. The
mvp-3 envelope deletes the legacy top-level `original` / `replacement` byte
fields and replaces them with a discriminated `proposal` sub-object that
carries either a structural `FixIntent` (rule-emission) or a
`TextCorrection` (engine-internal C001 path). The legacy `mvp-1` / `mvp-2`
shapes retired entirely; the accept-list is `["marque-mvp-3"]`.

```jsonc
{
  "schema": "marque-mvp-3",
  "rule": "E054",
  "source": "BuiltinRule",
  "span": { "start": 12, "end": 25 },
  "proposal": {
    "kind": "FixIntent",
    "intent": {
      "kind": "FactRemove",
      "scope": "Page",
      "facts": [{ "kind": "Cve", "token_id": 104 }]
    }
  },
  "confidence": 0.95,
  "migration_ref": null,
  "timestamp": "2026-05-13T12:34:56Z",
  "classifier_id": "12345",
  "dry_run": false,
  "input": "/path/file.txt",
  "recognition": 0.95
  // `runner_up_ratio` omitted when None (strict-path fix); only
  // emitted by decoder-path R001 records.
  // `features` omitted when empty; emitted as an array of
  // `{"id": "...", "delta": <f32>}` when the decoder contributes
  // explicit feature deltas.
}
```

For C001 text-correction records, `proposal` carries the canonical
replacement string (corpus-derived token canonical — Constitution V's
permitted-identifier list):

```jsonc
"proposal": {
  "kind": "TextCorrection",
  "replacement": "SECRET"
}
```

**Constitution V Principle V (G13) closure**: the audit envelope carries
no `original` byte field. The structural FixIntent variants carry no
document content; the TextCorrection variant carries only corpus-derived
canonical tokens. Audit records are content-ignorant by construction.

**FR-014 (single schema per build)**: `crates/engine/build.rs` validates
`MARQUE_AUDIT_SCHEMA` against the closed accept-list `["marque-mvp-3"]`.
The CLI and WASM emitters compile against the same const so byte-identical
records flow from both surfaces.

The sections below describe the **post-keystone `marque-1.0` target**,
which adds `(scheme, predicate_id)` rule encoding, `Canonical<S>`
provenance, BLAKE3 digesting, closed `MessageTemplate` JSON serialization,
and content-ignorance canary tooling. The keystone work is scheduled in
follow-up 006-refactor PRs; this section governs what binaries today
emit.

---

# Contract: Audit Record (NDJSON, schema `marque-1.0`) — POST-KEYSTONE TARGET

**Lands at**: PR 3c (single cutover; clean break)
**Spec FRs**: FR-002, FR-004, FR-026, FR-034, FR-035, FR-037, FR-041
**Source-plan refs**: §10 (audit clean break), §10.2.1 (JSON shape sketch)
**Audience**: compliance auditors, NDJSON consumers (CLI piping, WASM postMessage embedders, log-aggregation pipelines), security/integrity reviewers.

---

## Schema identifier

```text
"schema": "marque-1.0"
```

`MARQUE_AUDIT_SCHEMA` is build-time-pinned to a single value via
`marque-engine::AUDIT_SCHEMA_VERSION` (FR-034). One binary emits
exactly one schema. There is **no accept-list**; pre-cutover records
are unreadable by post-cutover binaries (FR-037 — clean break, no
`marque-audit-reader` crate scheduled).

---

## NDJSON record shape

One JSON object per line. Records are append-only and emitted in
construction order by `Engine::fix_inner`; never reordered post-
promotion (I-5).

```jsonc
{
  "schema": "marque-1.0",

  "rule": {
    "scheme": "capco",
    "predicate_id": "banner.classification.usa-trigraph"
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
    "template": "BannerMissingClassification",          // closed enum (FR-003)
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

`(scheme, predicate-id)` per FR-026 / R-3:

```jsonc
"rule": {
  "scheme": "capco",
  "predicate_id": "banner.classification.usa-trigraph"
}
```

- `scheme`: lowercase short name. `"capco"` for the only in-tree CAPCO
  scheme today; future schemes use their own short name (`"cui"`,
  `"nato"`, etc.). **`"engine"` is a reserved sentinel scheme** (FR-044)
  used exclusively for synthetic engine-minted diagnostics (R001, R002,
  …); it is not a valid `MarkingScheme` registration target.
- `predicate_id`: dot-separated `<surface>.<category>.<predicate>`
  lowercase string. For CAPCO and other real schemes, `<surface>` is one
  of `banner | portion | page`; `<category>` matches the lattice
  category where applicable (`classification | sci | sar | dissem | fgi
  | nato | fouo | aea | declassification`); `<predicate>` is descriptive
  English-with-hyphens. For the `"engine"` sentinel scheme, the
  predicate_id is the engine diagnostic identifier in lowercase
  (`r001.decoder-recognized`, `r002.reparse-failed`).

Engine-minted synthetic diagnostics (R001 decoder recognition, R002
re-parse failure) carry the sentinel `"engine"` scheme per FR-044
(see consolidated source plan §9.4):

```jsonc
"rule": { "scheme": "engine", "predicate_id": "r001.decoder-recognized" }
"rule": { "scheme": "engine", "predicate_id": "r002.reparse-failed" }
```

Rationale: R001/R002 are minted by `marque-engine`, not by a
`MarkingScheme` impl. Inheriting the active scheme's namespace (e.g.,
`("capco", "engine.r001.…")`) would lie about provenance — the
diagnostic is *about* a CAPCO marking but isn't *from* CAPCO. The
sentinel scheme keeps `("capco", …)` cleanly meaning "from a CAPCO
rule" and is forward-compatible with future schemes.

Legacy `E###` / `W###` / `S###` / `C###` IDs do not appear in
`marque-1.0` records. The one-time mapping table lives at
`docs/refactor-006/legacy-rule-id-map.md` (R-3) for archaeological
reference only.

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
JSON serialization uses the variant identifier as a string:

```jsonc
"template": "BannerMissingClassification"
"template": "PortionUnknownDissem"
"template": "FouoEvictedByClassification"
"template": "FouoEvictedByNonFdrDissem"
"template": "NofornSupersedesRelTo"
"template": "ReparseFailed"             // R002 (PR 7)
"template": "DecoderRecognized"         // R001
/* ... */
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

The `marque-1.0` schema introduces a `Severity::Suggest` variant
distinct from the prior set (`Off | Info | Warn | Error | Fix`). Its
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
- Pre-cutover `marque-mvp-2` records are unreadable by post-cutover
  `marque-1.0` binaries.
- This is a type-level guarantee, not a runtime concern: there are no
  pre-cutover records (no users, no deployment) at the time of cutover.
- The window for clean-break refactor closes when external consumers
  attach (per spec Assumptions); this refactor is the last clean-break
  window.

## Schema discoverability (D3)

Per **decision D3** in `decisions.md`, the active audit schema name
MUST be discoverable by external consumers without parsing audit
records.

**Per-record discoverability** (already in place): every record's
first field is `"schema": "marque-1.0"` (mandatory, FR-035). Streaming
NDJSON consumers detect schema by reading the first record they see.

**Per-binary discoverability** (NEW at PR 3c): `marque --version` MUST
expose the active audit schema name in its output. Format choice
(JSON, key/value lines, or human-readable) is implementer's call at
PR 3c; the binding constraint is that the schema name appears such
that:

- A shell script can grep for `marque-1.0` in `marque --version`
  output and detect schema-major changes.
- The schema name shown matches the value baked into
  `marque_engine::AUDIT_SCHEMA_VERSION` (FR-034) — single source of
  truth.

**Cutover changelog** (PR 3c): the changelog entry MUST explicitly
state that the audit schema's `"schema"` field is the discriminator
external consumers should branch on, and that pre-cutover binaries
producing `marque-mvp-2` records are not interoperable with
post-cutover binaries.

This closes the discoverability gap left by FR-037's "no reader crate"
posture: external consumers who do exist (per the no-consumers
attestation in D4 — none expected) get a mechanical signal that the
schema changed, even though no compatibility shim is provided.

---

## Schema-bump policy

After this refactor, `marque-1.0` is the audit schema. Subsequent
schema bumps (`marque-1.1`, `marque-1.2`, `marque-2.0`) follow semver:

- **Minor bump (`marque-1.x`)**: additive — new `MessageTemplate`
  variant, new `FeatureId` variant, new `MessageArgs` field type
  (still closed-set). Reader compatibility is forward-only;
  `MARQUE_AUDIT_SCHEMA` validates against the exact bumped value.
- **Major bump (`marque-2.0`)**: structural — rule-ID shape change,
  `Canonical<S>` shape change, removal of a permitted identifier type.
  Constitutes a clean-break event of the same magnitude as
  `marque-mvp-2 → marque-1.0`.

PR 8's `marque-priors-3` is a separate priors-bake schema, not the
audit schema (FR-036). Audit-schema bumps and priors-bake schema bumps
are independent.
