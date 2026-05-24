---
date: 2026-05-10
status: pending follow-up to PR 3c (engine + rule architecture refactor)
parent: specs/006-engine-rule-refactor/
covers: auto-apply + confidence calibration for six no-fix Requires/Conflicts rules
authors: synthesized from decisions/02-catalog-shape.md D3 (middle-path refactor)
prerequisite: PR 3c.B merged to staging
estimated-size: 210–360 LoC, single-commit follow-up PR
---

# No-Fix Rule Auto-Apply + Confidence Calibration

## Background

PR 3c lands the bag-of-tokens architectural restatement
(`specs/006-engine-rule-refactor/architecture.md`, 2026-05-09).
`decisions/02-catalog-shape.md` D3 committed to a middle-path
treatment of the six rules that today emit `Severity::Error` with no
`FixProposal`:

| Rule ID | Audit purpose-row | Natural emission shape | Citation |
|---|---|---|---|
| **E021** | requires (AEA → NOFORN) | `FactAdd { token: NOFORN, scope: portion.dissem }` | CAPCO-2016 §H.6 |
| **E024** | conflicts (RD evicts FRD + TFNI when all three co-present) | `FactRemove { FRD, scope }` + `FactRemove { TFNI, scope }` — multi-remove in one rule firing; `scope` is whichever of banner / portion has the three-way co-presence per §H.6 p104 ("both banner and portion") | CAPCO-2016 §H.6 p104 |
| **E036** | conflicts (JOINT ⊥ HCS) | `FactRemove { HCS, scope: portion.sci }` — HCS goes; JOINT is the more-binding marking per §H.3 p57 (the exclusion specifies HCS, not JOINT) | CAPCO-2016 §H.3 p57 (line 1272: "May not be used with the HCS markings or NOFORN markings") |
| **E037** | conflicts (NODIS ⊥ EXDIS, mutual exclusion) | `FactRemove` — supersession winner per §H.9 is NODIS over EXDIS (cross-referenced by E041's intra-portion supersession) | CAPCO-2016 §H.9 p172 (EXDIS) + p174 (NODIS) |
| **E038** | requires (NODIS or EXDIS → NOFORN) | `FactAdd { token: NOFORN, scope: portion.dissem }` | CAPCO-2016 §H.9 p172 / p174 ("May be used only with NOFORN information" on both entries) |
| **E041** | conflicts (NODIS supersedes EXDIS in same portion) | `FactRemove { token: EXDIS, scope: portion.non_ic_dissem }` | CAPCO-2016 §H.9 p172 / p174 |

PR 3c performs these for each of the six rules:
- Lands the `FactAdd` / `FactRemove` / `Recanonicalize` emission-type
  vocabulary on `marque-rules` (additive on `marque-mvp-2`; no audit
  schema bump per `decisions/03-empirical-concerns.md` D9).
- Migrates the rule body to emit its natural structural shape
  through that vocabulary.
- Verifies the emission fires on the existing rule-firing tests
  (negative coverage — emission is suggested, not auto-applied).

PR 3c does **not** perform:
- Promotion of `FixProposal` from suggestion to auto-applied for
  these six rules.
- Per-rule confidence calibration.
- The audit-record idempotency / G13 closure / fixpoint-stability
  tests that the RELIDO cluster (E054–E057) carries today.

This follow-up closes those three gaps.

## Scope

Three deliverables, one commit:

1. **Per-rule confidence threshold** set in the rule definition for
   each of the six rules. Engine consumes the threshold via the
   existing `Engine::fix` promotion path
   (`marque-engine::engine::fix_inner`).
2. **Test suite per rule**, modeled on the RELIDO cluster
   (E054–E057). Four assertions per rule × six rules = 24 new
   `#[test]` functions plus fixture data.
3. **Citation re-verification** for E024 / E036 / E037 / E038 at
   the calibration commit (Constitution VIII at point of
   propagation: this commit is a propagation point because it
   asserts each rule's fix is correct, not just that it fires).

## Per-rule confidence rationale

The rule-body audit (`specs/006-engine-rule-refactor/rule-body-audit.md`
rows at lines 66, 67, 72, 74, 76, 77 plus summary line 95-96) classifies
each rule's emission shape against its §-citation. The "confidence
drift signal" note in the audit (line 151) observes that confidence
becomes a property of *shape* (`FactAdd` / `FactRemove` /
`Recanonicalize`) more than per-rule judgment when fixes move to the
canonical vocabulary — meaning the per-rule thresholds set below will
likely consolidate into shape-based policy when the long-term
confidence-calibration policy doc (review-queue item 10 of 2026-05-10)
lands. Until then, each threshold is explicit:

- **E021 (AEA → NOFORN)** — categorical Requires per §H.6.
  Closed-vocab single-token add.
  **Threshold: 1.0.**
- **E024 (RD evicts FRD + TFNI)** — categorical multi-remove per
  §H.6 p104. The two `FactRemove` emissions are atomic in one rule
  firing: the engine promotes both or rejects both as a single
  audit-record cluster (no half-applied fix on the three-way
  co-presence). E024 is the multi-token-remove novelty in this
  set; the engine's promotion path may need an explicit
  "atomic cluster" flag if it doesn't already handle this shape
  (see Verification gate step 6 below).
  **Threshold: 1.0.**
- **E036 (JOINT ⊥ HCS)** — categorical Conflicts per §H.3 p57. The
  §-text is directional ("May not be used with the HCS markings or
  NOFORN markings" appears under JOINT, naming HCS/NOFORN as the
  exclusion targets), so HCS is the token to remove; JOINT stays.
  **Threshold: 1.0.**
- **E037 (NODIS ⊥ EXDIS, mutual exclusion)** — supersession
  direction is implied by §H.9 + cross-referenced by E041's
  intra-portion supersession (NODIS wins, EXDIS goes). The
  implementer MUST verify the supersession encoding at the
  calibration commit against the §H.9 passages on p172 and p174.
  If §H.9 admits an "either may win" reading (e.g., the two pages
  use symmetric "may not be combined" wording without naming a
  winner), drop to 0.95 and document the directionality assumption
  in the rule's doc comment.
  **Threshold: 1.0 (NODIS-supersedes-EXDIS encoding confirmed); 0.95 (either-direction encoding).**
- **E038 (NODIS or EXDIS → NOFORN)** — categorical Requires per
  §H.9 p172 ("May be used only with NOFORN information") + p174
  (same phrasing for NODIS). Closed-vocab single-token add.
  **Threshold: 1.0.**
- **E041 (NODIS supersedes EXDIS in portion)** — categorical
  intra-axis supersession; closed-vocab single-token remove.
  **Threshold: 1.0.**

All six thresholds are subject to Constitution VIII re-verification
at the calibration commit (not just at audit time): the implementer
MUST Grep each §-citation against `crates/capco/docs/CAPCO-2016.md`
and confirm the passage matches the rationale above. The audit's
resolution is the baseline; re-verification at point of propagation
is the rule.

## Test template (mirrors RELIDO cluster)

For each rule, four assertions. Test file path:
`crates/capco/tests/no_fix_calibration.rs` (or per-rule files if
fixture size pushes the file over 800 lines).

```rust
#[test]
fn e0XX_fix_correctness() {
    // Given a portion known to violate the rule
    let input = "(...)";
    let expected = "(...)";
    let result = engine.fix(input);
    assert_eq!(result.fixed_text, expected,
               "E0XX did not produce the canonical fix");
}

#[test]
fn e0XX_idempotent() {
    // Fix on already-canonical input is a no-op
    let once = engine.fix(input);
    let twice = engine.fix(&once.fixed_text);
    assert!(twice.diagnostics.iter().all(|d| d.rule != "E0XX"),
            "E0XX fired on already-fixed text — fixpoint not reached");
}

#[test]
fn e0XX_audit_g13_closure() {
    // No document bytes leak into the audit record's FactAdd/FactRemove payload
    let result = engine.fix(input);
    for fix in &result.audit_log.fixes.iter().filter(|f| f.rule == "E0XX") {
        assert_no_document_bytes_in(&fix.proposal);
        // assert proposal is FactAdd { token: ..., scope: ... } or
        // FactRemove { token_ref: ..., scope: ... } — structural only
    }
}

#[test]
fn e0XX_render_canonical_stability() {
    // After fix, the fixed text is its own canonical render
    let result = engine.fix(input);
    let canonical = re_render_canonical_from(&result.fixed_text);
    assert_eq!(result.fixed_text, canonical,
               "E0XX fix did not converge to canonical form");
}
```

Per-rule fixtures: minimum 2 input/expected pairs covering
(a) the cleanest invocation of the rule and (b) one boundary case
naturally surfaced by the §-citation. RELIDO cluster uses 3–5 per
rule; aim for that volume.

## Audit-record implications

PR 3c lands `AppliedFix.proposal` as a union over `FactAdd` /
`FactRemove` / `Recanonicalize` (additive on `marque-mvp-2`;
**no schema bump per D9**). This follow-up makes no audit-schema
changes.

Each promoted fix records (via the existing engine promotion path):
- `rule_id` (e.g., `"E021"`)
- `proposal` (the structural fact-set delta — closed-vocab token
  references, category IDs, scope tags; G13-clean)
- `confidence` (the calibrated threshold above; engine compares
  against config threshold to decide promote-vs-suggest)
- `timestamp`, `classifier_id`, `dry_run` (engine-snapshotted)

## Out of scope

The following are explicitly NOT in this follow-up:

- New rule additions.
- New emission-type variants on `marque-rules` (`FactAdd` /
  `FactRemove` / `Recanonicalize` is the closed set per D5/D9).
- `marque-scheme` trait surface changes (the `ConstraintViolation
  + span + severity` extension landed in PR 3c.B commit 7).
- Renderer body changes (PR 3c's `render_canonical` is consumed
  as-is; this follow-up does not extend per-axis canonicalization).
- Walker decomposition or `Constraint` catalog edits (D2 work —
  closes in PR 3c.B commit 7).
- E005 / S005 / S006 calibration (they remain provisional
  `Constraint::Custom` per D4; their auto-apply story depends on
  the admonition channel and document-scope `Recanonicalize`,
  neither of which is in this follow-up's scope).
- Audit schema bump (D9 confirmed no bump is needed; `marque-mvp-2`
  is the schema this PR emits).

## Implementation estimate

Per `decisions/02-catalog-shape.md` D3: **210–360 LoC total**, broken
down:

| Component | Estimate |
|---|---|
| Confidence threshold values on six rule structs | ~30 LoC |
| Test scaffolding (4 tests × 6 rules + helpers) | ~150–250 LoC |
| Engine wiring for auto-apply path (if not already in PR 3c) | ~30–80 LoC |
| Doc comments on each rule justifying threshold | ~30 LoC |

Single-commit follow-up PR. Reviewable as one unit.

## Verification gate (pre-PR-open)

This PR may not open until:

1. PR 3c.B has merged to staging.
2. All six rules emit their natural structural shape on the staging
   branch (rule-firing tests still pass; emission is suggested, not
   auto-applied).
3. `cargo test -p marque-capco` and `cargo test -p marque-engine`
   are green on staging.
4. `bench-check.sh` is within thresholds (D8 confirmed ~19× headroom
   on SC-001; this follow-up adds 24 tests + ~210–360 LoC, well
   inside the regression-detection envelope).
5. The four citations resolved at spec time (E024 §H.6 p104; E036
   §H.3 p57; E037 §H.9 p172/p174; E038 §H.9 p172/p174) have been
   re-verified at the calibration commit against the current
   `crates/capco/docs/CAPCO-2016.md` (Constitution VIII at point
   of propagation). E037's supersession-direction encoding is the
   only resolution-time judgment requiring explicit calibration-
   commit confirmation; the other five are categorical.
6. E024's atomic-cluster promotion path (both `FactRemove`
   emissions land together or neither lands) has been verified in
   the engine — either by an existing test on a multi-token-remove
   rule, or by adding the test as part of this PR.

## References

- `specs/006-engine-rule-refactor/architecture.md` — bag-of-tokens
  commitment (2026-05-09).
- `specs/006-engine-rule-refactor/rule-body-audit.md` — natural
  emission shape per rule.
- `specs/006-engine-rule-refactor/decisions/02-catalog-shape.md` D3
  — middle-path refactor recommendation.
- `specs/006-engine-rule-refactor/decisions/03-empirical-concerns.md`
  D9 — no schema bump (additive on `marque-mvp-2`).
- `crates/capco/docs/CAPCO-2016.md` — primary authoritative source
  for §H.6 (E021), §H.9 p172–174 (E041), other rules TBD.
- RELIDO cluster test template — locate in `crates/capco/tests/`
  post-PR-3c-merge.
