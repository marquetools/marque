# Follow-up — PR 3c.B Commit 10 test rewrites

**Status**: tracking artifact for merge-blocking follow-up work
**Created**: 2026-05-13
**Parent commit**: PR 3c.B Commit 10 (`FixProposal` cleanup + audit schema bump to `marque-mvp-3`)
**Authority**: pre-launch reviewer findings (rust-reviewer + code-reviewer) on the Commit 10 PR

## Scope

The Commit 10 atomic-cutover refactor structurally closes G13 (Constitution V Principle V) on the legacy emission path by deleting the byte-carrying fields (`FixProposal.original`, `AppliedFixProposal::Legacy`, top-level `AppliedFix.proposal.original` / `.replacement`) from the type system. Audit content-ignorance is now enforced *by construction* — a future regression cannot reopen the channel without a compile-error.

However, several test files that previously enforced the same invariants at the **per-rule** level were gated with `#![cfg(any())]` because their field-reads target the retired shape. The structural protection is stronger than the per-rule coverage, but the per-rule coverage is load-bearing for accuracy regression detection — not just for G13.

### Files gated `#![cfg(any())]` in Commit 10

Source: `git diff 69b43698~1 69b43698 -- '**/*.rs' | rg '^[+ ]#!\[cfg\(any\(\)\)\]'`.

| File | LoC | Constitutional role | Re-enablement cost |
|------|-----|---------------------|---------------------|
| `crates/capco/tests/g13_closure_fix_intent.rs` | 458 | G13 per-rule envelope walker (Constitution V Principle V) | HIGH — needs full rewrite against new `AppliedFix` shape; the structural envelope walker has to descend into `AppliedFixProposal::FixIntent(_)` and assert no Box<str> payloads carry document bytes. The walker pattern survives; the field paths change. |
| `crates/capco/tests/s004_audit_content_ignorance.rs` | 184 | S004 suggest-channel audit content-ignorance (Constitution V Principle V) | MEDIUM — narrower scope, similar rewrite pattern. |
| `crates/capco/tests/corpus_parity.rs` | 323 | SC-002 / SC-003 corpus accuracy (≥95% per-rule, ≥85% overall) | HIGH — the assertion patterns need to descend into the new fix payload shape; `corpus_parity.json` was regenerated in Commit 10 so the artifact is current, but the test reader was not. |
| `crates/capco/tests/byte_identity_pr3c.rs` | ? | PR 3c.B byte-identity gate (legacy-path output preservation) | RETIRE — this test's purpose was Path C transition byte-identity; post-cutover Path C is gone. The byte-identity property is structurally no longer applicable (FixIntent rendering ≠ FixProposal byte splicing). Recommend retiring entirely. |
| `crates/capco/tests/fix_intent_round_trip.rs` | ? | FixIntent round-trip vs `FixProposal` byte-identity | RETIRE — same rationale as `byte_identity_pr3c.rs`. The round-trip property the test asserted does not exist post-cutover. |
| `crates/capco/tests/e010_intent_only_engine.rs` | ? | E010 intent-only engine path | LOW — small per-rule integration test; rewrite against new shape. |
| `crates/capco/tests/e012_conscious_defer.rs` | ? | E012 conscious-defer (no-fix) path | LOW — small per-rule test. |
| `crates/capco/tests/e014_fact_add_engine.rs` | ? | E014 FactAdd engine path | LOW — small per-rule test. |
| `crates/capco/tests/e024_atomic_cluster.rs` | ? | E024 multi-remove cluster engine path | MEDIUM — multi-intent batch test. |
| `crates/capco/tests/e038_intent_only_engine.rs` | ? | E038 NODIS+EXDIS NOFORN-add engine path | LOW. |
| `crates/capco/tests/e053_intent_only_engine.rs` | ? | E053 NOFORN-clears-REL-TO page-rewrite engine path | LOW. |
| `crates/capco/tests/relido_conflicts.rs` | ? | E054–E057 RELIDO conflict family engine paths | MEDIUM. |
| `crates/capco/tests/rules_us1.rs` | ? | US1 / S004 path | LOW. |
| `crates/capco/tests/s004_engine_fix.rs` | ? | S004 engine path | LOW. |
| `crates/capco/tests/sci_per_system_catalog.rs` | ? | SCI per-system catalog engine path | MEDIUM. |
| `crates/capco/tests/pattern_a_noforn_supremacy.rs` | ? | Pattern A NOFORN-supremacy engine path | MEDIUM. |
| `crates/engine/tests/intent_only_byte_identity.rs` | ? | Intent-only byte-identity vs legacy | RETIRE — same rationale as the capco byte-identity tests. |
| `crates/capco/src/rules.rs` `mod tests` | ~3800 LoC | Inline rule unit tests | HIGH — the inline `mod tests` reads pre-cutover fields heavily; rewrite is the largest single piece of scope here. |

### What protections remain ACTIVE in the meantime

Three independent gates continue to provide coverage:

1. **Structural enforcement (compile-time)** — the byte-carrying fields no longer exist on `AppliedFix`, `AppliedFixProposal`, or `FixIntent`. A future regression that reintroduces document-byte interpolation would require *adding a new field or a new audit emitter path* — both visible at PR review and both flagged by `rg`-based acceptance gates (§6.6 of the parent spec).

2. **Active integration tests** — `crates/engine/tests/audit.rs` contains a **prose-sentinel leak detector** (`audit_stream_uses_only_one_schema_version` + the prose-sentinel block at the top of the file) that lints every `AppliedFix` emitted by the workspace's full lint-and-fix pipeline against a labeled prose corpus. This is workspace-level G13 coverage. It uses the new shape and is GREEN.

3. **The 1741+ unit and integration tests that pass with the new shape** — they read the new top-level fields (`AppliedFix.rule`, `.span`, `.source`, `.proposal: AppliedFixProposal::{FixIntent, TextCorrection}`) and would fail if the new shape misrepresented the audit envelope.

### Re-enablement priority

**Merge-blocking for main**:
- `g13_closure_fix_intent.rs` (HIGH constitutional)
- `corpus_parity.rs` (HIGH accuracy gate)

**Should land in the next sub-PR (3c.C or similar)**:
- `s004_audit_content_ignorance.rs`
- `crates/capco/src/rules.rs` inline `mod tests` (rewrite vs delete the largest portion)

**Land later (low priority, narrow scope)**:
- All `e*_intent_only_engine.rs`, `relido_conflicts.rs`, `pattern_a_noforn_supremacy.rs`, `sci_per_system_catalog.rs`

**Retire (do not rewrite)**:
- `byte_identity_pr3c.rs`
- `fix_intent_round_trip.rs`
- `crates/engine/tests/intent_only_byte_identity.rs`

These three tested an invariant (byte-identity between FixProposal output and synthesized FixIntent output) that does not exist post-cutover by design — the whole point of the Commit 10 cutover is to retire the byte-precise emission path.

### Suggested process

1. Open one or more sub-PRs (3c.C series) sized to the re-enablement table above.
2. Each sub-PR: lift the `#![cfg(any())]` gate, rewrite field reads against the new shape, re-run the test, confirm green.
3. PR `g13_closure_fix_intent.rs` is the must-have-before-main-merge sub-PR. The remainder MAY land after main-merge if the closure walker confirms G13 is structurally intact.

## Closing note

The Commit 10 cutover deliberately accepts a transient test-coverage deficit in exchange for the constitutional invariant being enforced structurally. This is the same trade-off Constitution V Principle V's test-fixture carve-out reasons about: the *type-level seal* (private `_seal: ()` field on `EnginePromotionToken`) is the load-bearing protection, and the test-fixture coverage is a check on the carve-out behaving correctly. The same logic applies here at a larger scope: the deleted-field type-level protection is load-bearing for G13; the per-rule tests are a check that no future regression undermines it. We pay down the per-rule check in follow-up; the load-bearing protection is intact today.
