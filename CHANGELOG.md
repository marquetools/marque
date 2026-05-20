<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **Audit schema flipped from `marque-mvp-3` to `marque-1.0` (PR 3c.2.D).**
  Single-binary, single-schema per FR-014. Pre-cutover NDJSON records
  are not interoperable with post-cutover binaries (FR-037 clean break;
  no `marque-audit-reader` crate is scheduled). External NDJSON
  consumers branch on the `"schema"` field — see
  `contracts/audit-record.md` §"Schema discoverability (D3)"
  (§415-446).
- Audit-record JSON shape reshaped per contract §107-178: `fix` is now
  a nested object with `replacement.{discriminant, canonical,
  confidence}` + `original_span` + `original_digest`. Text corrections
  are a separate NDJSON line type (`{"type": "text_correction", ...}`)
  per PM-D-4.
- `marque --version` now exposes `audit_schema: marque-1.0` per Schema
  Discoverability D3. Shell scripts grep `^audit_schema:` to detect
  schema-major changes between binaries.
- BLAKE3 digesting of canonical replacement bytes + pre-fix bytes is
  now wired at promotion time inside `AppliedFix::__engine_promote`
  per Constitution V Principle V — bytes are never stored, only
  digests appear in audit records.

### Removed

- `AppliedFix.proposal: AppliedFixProposal<S>` envelope (replaced by
  `fix: AppliedFixDetail<S>`).
- `AppliedFix.confidence` top-level (moved to
  `fix.replacement.confidence`).
- `AppliedFix.migration_ref` top-level (superseded by typed `Citation`
  on `Diagnostic`; the marque-1.0 audit-record contract per §168-171
  does not emit a top-level `migration_ref`).
- `AUDIT_SCHEMA_IS_V3` const (renamed to `AUDIT_SCHEMA_IS_V1_0`).
- `AppliedFixProposal<S>` enum type.
- `FixResult.applied: Vec<AppliedFix<S>>` field (sole audit-output
  channel is now `FixResult.audit_lines: Vec<AuditLine<S>>`).
- Pre-cutover `marque-mvp-1` / `marque-mvp-2` / `marque-mvp-3`
  envelope shapes — accept-list contracts to a single value
  `["marque-1.0"]`.

### Added

- `AppliedTextCorrection` type (separate NDJSON line for non-marking
  corrections, PM-D-4).
- `AuditLine<S>` sum type (preserves cross-record promotion order
  between marking-fix and text-correction lines, PM-D-8 generalizes
  FR-016).
- `Discriminant` enum (closed `Strict | Decoder`; replacement
  provenance discriminator derived at audit-emit time from
  `FixSource` via the PM-D-7 5-to-2 collapse).
- T055 G13 content-ignorance canary at
  `crates/engine/tests/audit_g13_canary.rs`. Sweeps the regression
  corpora through `Engine::fix` and asserts no ≥4-byte input
  substring appears in any v1.0 NDJSON record outside the
  permitted-identifier list.
- `FixResult::applied_fixes()` and `FixResult::applied_text_corrections()`
  accessors for consumers that read only one arm of the
  `audit_lines` sum-type stream.

### Retired

- `#257` strict-recognizer masking pin at
  `crates/engine/tests/core_error_isolation.rs` — the T055 canary
  structurally closes the decoder canonical-bytes leak channel the
  pin was masking. The default `StrictOrDecoderRecognizer` applies
  in `core_error_isolation`'s engine setup.
