<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# CAPCO Rule Archaeology

Retirement provenance for the CAPCO rule set. Each entry records which
PR retired a rule, what concern it owned at retirement time, and where
that concern lives now (renderer, declarative catalog, bridge dispatch,
or simply gone with no replacement).

## Scope

These files document **why retired rule IDs no longer appear in the
registered rule set**. They are pure history; nothing here is consulted
at runtime.

- [`retirement-history.md`](./retirement-history.md) — the comprehensive
  per-rule retirement record, organized by retirement PR. This is the
  former top-of-file `//!` block from `crates/capco/src/rules.rs`
  (extracted by issue #561 split).
- [`rule-id-cross-refs.md`](./rule-id-cross-refs.md) — extracted inline
  comments that documented "rule X (now retired) used to relate to live
  rule Y." Grouped by the **live** rule that the cross-ref previously
  annotated, so a developer reading current code can trace the
  historical context that was once inlined next to it.

## Relationship to `docs/refactor-006/legacy-rule-id-map.md`

That file is the **T044 wire-string ↔ legacy-ID translation table** —
114 rows mapping every pre-T044 flat-string rule ID
(`E060`, `S004`, etc.) to its post-T044 2-tuple
(`("capco", "portion.dissem.rel-to-...")`). It is identifier
translation.

The files in this directory are **retirement provenance** — when a
rule retired, into what mechanism (catalog, renderer, bridge), and on
what authority. Identifier translation and retirement provenance are
orthogonal: a wire string can still be live (translated from a still-
registered legacy ID), and a retired rule can have a wire string that
no longer appears in the registered set.

No content duplication between the two surfaces.

## Editing discipline

Per Constitution Principle VIII (Authoritative Source Fidelity):

- Every `§X.Y pNN` citation in these files was verified once when the
  original comment was authored. The split (issue #561) **moves** the
  comments byte-identical; it does not re-author them.
- If a future change requires editing one of these archaeology entries
  — correcting a retirement-PR reference, replacing a stale §-citation
  with a current one — the editor MUST re-verify the new citation
  against `crates/capco/docs/CAPCO-2016.md` at the point of edit.
  Propagation re-verification per Principle VIII applies equally to
  archaeology and to live rule code.
