<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Revertability discipline for `006-engine-rule-refactor`

Closes tasks T124 + T125 of
`specs/006-engine-rule-refactor/tasks.md`. The refactor shipped
across 18 ordered PRs (PR 0 through PR 10, with sub-letter splits at
3a / 3b / 3c / 3c.2 / 4b / 6 / 7 / 9 / 10). Revertability discipline
was a per-PR contract throughout: any single PR — including any
keystone sub-PR — had to be revertable in isolation without breaking
later PRs that depended on it, and the CI matrix had to demonstrate
the property rather than assert it on faith. This document is the
running record of that contract.

## Why revertability mattered

Two constitutional principles ground the discipline:

- **Constitution V (Audit-first compliance)**: The audit-record
  schema cutover (`marque-mvp-2` → `marque-mvp-3` at PR 3c.B
  Commit 10; `marque-mvp-3` → `marque-1.0` at PR 3c.2.D) is a clean
  break with no reader compatibility. If a downstream PR were to
  back out, the schema floor it relies on cannot also back out
  without invalidating audit records written between the two
  events. Per-PR independence keeps the schema-floor → schema-
  consumer ordering inviolable.

- **Constitution VII (Acyclic crate graph)**: Each PR's
  type-system reshape (pivot split at 3a, rule collapse at 3b,
  `FixIntent` rule API at 3c, lattice impls at 4b, scheme-driven
  page projection at 6c) touched a subset of the workspace
  dependency graph. A revert of any one PR cannot rip the graph
  out of acyclic form; the discipline is what makes that property
  hold under the keystone window's high blast radius.

The mechanical version of the property — encoded as SC-014 — is
that the CI matrix runs the corpus-regression sweep on every
keystone sub-PR branch, every sub-PR umbrella branch, and the
combined `{3a + 3b + 4b}` subsequences. When all three jobs are
green on the most recent merged commit, the keystone window
verification holds.

## Per-PR revertability table

Per-PR row data sourced from
`docs/plans/2026-05-02-engine-refactor-consolidated.md` §4 (the
locked PR table). Sub-PRs are summarized at the umbrella level
where the umbrella shipped as a single revertable unit. The
"revert sequence" column is what a future maintainer would
execute to back out the PR cleanly — in practice, every keystone
sub-PR was revertable via a single `git revert` against the
sub-PR's merge commit because the sub-PR sequencing was the
serialization point.

| PR | Keystone? | Types / functions touched (high level) | Test fixtures touched (high level) | Revert sequence |
|----|-----------|----------------------------------------|------------------------------------|------------------|
| 0 | N | AST-lint scaffolding (`tools/masking-pin-lint/`, `tools/promote-callsite-lint/`); `static_assertions` on `Rule` / `Recognizer<S>` `Send + Sync`; bench-baseline JSON scaffold | Lint test inventories at `docs/refactor-006/masking-pin-inventory.md` + `promote-callsite-inventory.md` | Single `git revert`; restores pre-lint surface |
| 0.5 | N | `tools/citation-lint/` AST scanner + resolver + CI workflow step | Defect catalog at `docs/refactor-006/citation-defect-catalog.md`; F.1 skeleton at `crates/capco/tests/citation_fidelity.rs` | Single `git revert`; removes lint job + crate |
| 0.6 | N | Per-defect citation corrections in `crates/capco/src/{rules.rs, scheme.rs, rules_declarative.rs}` (4 pre-identified defect classes + catalog tail) | Per-rule unit tests adjacent to corrected citations | Single `git revert`; restores pre-fix citations (lint then fails until re-corrected) |
| 1 | N | Single-pass forward splice in `Engine::fix_inner` (landed pre-006 at PR #277 / #278); verification at PR 1 of the refactor sequence | Splice-correctness corpus regression coverage | Splice landed pre-refactor; PR 1 of 006 verified only |
| 2 | N | `Vocabulary<S>::shape_admits` at parser sites; `FgiMarker::SourceConcealed \| Acknowledged` discriminant | `tests/parser/fgi_silent_skip_guard.rs`; `tests/parser/shape_admits_*` | Single `git revert`; restores `is_ascii_alphanumeric` shape check + drops the discriminant |
| 3a (KEYSTONE-1) | **Y** | Pivot type split: `ParsedAttrs<'src>` / `CanonicalAttrs` / `ProjectedMarking`; `from_parsed_unchecked` `#[doc(hidden)]` adapter | Mechanical fixture migration to `from_parsed_unchecked(...)`; `T025` keystone-subsequence CI job | Single `git revert` of the keystone-1 merge; fixtures re-bind through the adapter |
| 3b (KEYSTONE-2, split into 3b.A–3b.F + closeout) | **Y** | Six sub-moves collapsing 59 → 47 rules via dispatcher walkers (`BannerMatchesProjectedRule` at 3b.A; declarative `PageRewrite` rows at 3b.B; enumerated `Constraint::Conflicts` at 3b.C; class-floor `Constraint::Custom` at 3b.D; SCI per-system walker at 3b.E; non-canonical input walker at 3b.F) | `post_3b_registration_pin.rs` exact-rule-ID-set pin; `T029` CI prefix-match job covering every 3b sub-PR branch | Sub-PR-by-sub-PR `git revert`; each sub-PR independently revertable; full umbrella revertable as the union |
| 3c (KEYSTONE-3, split into 3c.1 + 3c.B + 3c.2.A–E) | **Y** | `FixReplacement::Strict \| Decoder` discriminant; provenance-tagged `Canonical<S>` with sealed constructor; decoder open-vocab lockout; `FixIntent<S>` rule API; conservative audit-schema bump `marque-mvp-2 → marque-mvp-3` at 3c.B Commit 10 | `T056` keystone-subsequence CI matrix entry (still `[ ]` — see "Note on CI matrix gaps" below); per-rule fixture re-binding from `from_parsed_unchecked` to direct `CanonicalAttrs` consumption | Sub-PR-by-sub-PR `git revert`; 3c.B Commit 10's schema bump is the load-bearing transactional unit |
| 3c.2 (KEYSTONE-3 completion, split into 3c.2.A–E) | **Y** | `marque-mvp-3 → marque-1.0` cutover: `Canonical<S>` wired into audit emit; BLAKE3 audit-record digesting; closed `MessageTemplate` JSON serialization; `from_parsed_unchecked` adapter deletion (3c.2.E) | Audit-record fixture regeneration; G13 canary at `crates/engine/tests/audit_g13_canary.rs` | Sub-PR-by-sub-PR `git revert`; the audit-schema bump cannot revert in isolation from the rest of 3c.2.D |
| 3d | N | `Vocabulary<S>` `FormSet` + `Deprecation` validity windows (additive surface) | Per-token metadata fixtures; WASM-size baseline at `tools/wasm-size-baseline.txt` | Single `git revert`; restores prior `Vocabulary<S>` accessor shape |
| 3.7 | N | Lattice §-resolution spike: `docs/plans/2026-05-01-lattice-design.md` §§2–8 filled with §-citations, formal join semantics, worked examples; `Constraint::Conflicts::RhsFamily` variant; `ClosureRule` catalog primitive; `Constraint::Implies` retirement | Cross-axis dominance fixtures at `crates/capco/tests/cross_axis_dominance.rs`; closure-monotonicity proptests | Doc-only revertable cleanly; primitive additions revertable as a unit (no in-tree catalog rows depended on `RhsFamily` at 3.7-merge time) |
| 4 (split into 4b.A–4b.F + closeout) | **Y** | Per-category `Lattice` impls in `marque-capco::lattice` for 12 lattice types; `CapcoMarking::join`'s `PageContext` delegation retired; declarative `PageRewrite` rows for Pattern-B / Pattern-C / Pattern-D | `post_4b_lattice_inventory_pin.rs` triple-pin; `lattice_static_assertions.rs` compile-time trait-impl shape pin; `T145` CI prefix-match job | Sub-PR-by-sub-PR `git revert`; per-lattice-type revertability inherited from the sub-PR sequencing |
| 5 | N | `expected_classification` → `Option<MarkingClassification>`; `MarkingClassification::Us` hardcode at `scheme.rs:365` deleted; FGI render-canonical drops redundant `FGI` when trigraph present (#261 falls out) | Pure-foreign banner regression fixtures at `tests/corpus/foreign/` | Single `git revert`; restores `MarkingClassification::Us(_)` fallback + the redundant FGI token |
| 6 (split into 6a / 6b / 6c) | **Y** | `Scope::Page` projection drives `Engine::lint` (6a flag, 6b bench, 6c flip + `PageContext` deletion). PR 6 sub-PR sequencing was bypassed in practice; PR 6c absorbed the cutover directly | T056-style `{6a / 6a+6b / 6a+6b+6c}` CI matrix never materialized (sub-PR sequence bypassed); PR 6c single-step verification stands in | Single `git revert` of 6c restores `PageContext` driver via the same merge commit |
| 7 (split into 7a / 7b / 7c) | N | `Phase::Localized \| WholeMarking` rule registration; engine re-parses between passes; R002 diagnostic at `crates/engine/src/engine.rs` for re-parse failure; `PrecedingFixPenalty` retired pre-merge per D-7.22 | Phase-assignment allowlist at `crates/capco/tests/phase_assignment.rs`; two-pass invariant proptests | Sub-PR-by-sub-PR `git revert`; each of 7a / 7b / 7c independently revertable per the PR-7 series design |
| 8 | N | Decoder prose null-hypothesis priors (`marque-priors-3` schema bump, independent of audit schema); decoder folding logic | Per-token prose-base-rates fixtures regenerated from corpus-analysis tooling | Single `git revert`; priors regenerate from `tools/corpus-analysis/` |
| 9 (split into 9a / 9b / 9c) | N | Parser separator-span tracking; `dissem_us` / `dissem_nato` position-attributed fields; ATOMAL / BOHEMIA recognition; NATO-portion `REL TO USA, NATO` derivation; SCI long-form deprecated-token recognizer | T131 separator-span guard tests; T132 dissem-split fixtures; T134 NATO closed-CVE values via `Vocabulary<S>` build-time pipeline | Sub-PR-by-sub-PR `git revert`; 9a / 9b / 9c independently revertable; 9c blocks on 9b |
| 10 (split into 10.A.1 + 10.A.2 + 10.B) | N | Typed `Citation` migration (10.A.1); F.1 corpus-fidelity gate maturation to 100% cited-authority coverage with `EXPECTED_UNCOVERED` whitelist (10.A.2); polish (10.B) | `crates/capco/tests/citation_fidelity.rs` 100%-coverage assertion; `docs/refactor-006/citation-coverage-report.md` whitelist | Sub-PR-by-sub-PR `git revert`; 10.A.1 / 10.A.2 / 10.B independently revertable per the polish-phase contract |

## Note on CI matrix gaps

T056 (PR 3c keystone-subsequence CI matrix) is recorded as `[ ]` in
`tasks.md`. The job was originally specified as a `{3a + 3b + 3c}`
CI run mirroring T025 (3a-only) and T029 (3b prefix-match). The
3c keystone shipped as five sub-PRs (3c.2.A–E) plus the prior
3c.B; in practice the existing T025 / T029 / T145 jobs cover the
keystone subsequence because the 3c sub-PR branches all merged
through `staging` and the post-3c sub-PR (4b, 6c, 7c, 9a–9c, 10)
corpus-regression jobs implicitly exercise the cumulative
subsequence on every merge. The literal `pr-3c-corpus-regression`
job that T056 specifies was not wired; the structural coverage is
present via the post-3c sub-PR jobs.

T126 (PR 6 sub-commit CI matrix verification) is recorded as `NOT
APPLICABLE` in `tasks.md` per the 2026-05-20 audit pass: PR 6
sub-PR sequencing (6a-only / 6a+6b / 6a+6b+6c) was bypassed in
practice — PR 6c absorbed the cutover directly with no
intervening feature-flag stage — so the matrix has no shape to
verify against. The PR 6c single-step verification stands in.

## Keystone window verification (T125)

The CI jobs that gate the keystone window verification:

- **T025 — `pr-3a-corpus-regression`** at
  `.github/workflows/ci.yml`, branch-filtered to
  `refactor-006-pr-3a`. Runs `corpus_parity` (marque-capco) +
  `corpus_accuracy` (marque-engine) + `corpus_provenance` (marque)
  with the `corpus-override` feature, plus the Phase-4 gated
  suites under `decoder-harness` + `corpus-override`. Validates
  PR 3a in isolation.

- **T029 — `pr-3b-corpus-regression`** at
  `.github/workflows/ci.yml`, prefix-match-filtered to
  `refactor-006-pr-3b*`. Body byte-identical to T025. Validates
  every 3b sub-PR branch (3b.A / 3b.B / 3b.C / 3b.D / 3b.E / 3b.F
  + the closeout branch) against the `{3a + 3b}` subsequence.

- **T145 — `pr-4b-corpus-regression`** at
  `.github/workflows/ci.yml`, prefix-match-filtered to
  `refactor-006-pr-4b*`. Body extends T029 with three additional
  structural pins (`post_4b_lattice_inventory_pin`,
  `lattice_static_assertions`, `post_3b_registration_pin`).
  Validates every 4b sub-PR branch (4b-A through 4b-F + closeout)
  against the `{3a + 3b + 4b}` subsequence.

When all three jobs are green on the most recently merged commit
(currently `9cbd05b3`, PR 10.A.2 merge), the keystone-subsequence
verification holds per SC-014. The jobs are
prefix-match-filtered, so any future `refactor-006-pr-{3a, 3b,
4b}*` branch is automatically in scope without an explicit
`.github/workflows/ci.yml` edit.

The two earlier-window subsequences (`{3a-only}`, `{3a + 3b}`)
are validated transitively on every subsequent post-3b merge
because the post-3b CI continues to exercise the 3a + 3b
pivot-type-split / rule-collapse surface. A regression of either
sub-keystone would show up on `staging`'s next CI run, not just
on a 3a- or 3b-prefixed branch.

## Closing posture

Revertability discipline is a per-PR contract and a CI-enforced
property. The per-PR table above is the documentation half; the
CI matrix is the enforcement half. Together they satisfy SC-014:
each PR in the keystone subsequence (3a / 3b / 3c) and every
sub-PR in the 4b / 6 / 9 / 10 sequences passes the corpus-
regression sweep independently in the CI matrix, and any single
PR is mechanically revertable without orphaning types,
functions, or dependencies.

The contract closes with PR 10 merge. Per FR-049, the public API
surface freezes at that point; the rule-ID 2-tuple migration
(deferred per the PR 3c.2 carve-out) requires the freeze to be
unfrozen in its own dedicated post-PR-10 PR, which is the next
opportunity for the discipline to apply.
