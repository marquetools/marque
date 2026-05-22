# T044 — PM Decisions Addendum

**Date:** 2026-05-22
**Author:** PM session
**Status:** binding for the T044 implementation PR
**Companion:** `docs/refactor-006/2026-05-22-T044-rule-id-tuple-plan.md`

The architect's plan (linked above) carried strong recommendations on 8 open
decisions (OD-1 … OD-8). The PM has reviewed every one. **All eight recommendations stand**, with one refinement on OD-1.

---

## Decision summary

| OD | Decision | Notes |
|----|----------|-------|
| OD-1 | **B with refinement** — `ClosureRule.name` renames to wire-string form, with `closure` as a fifth permissible `<surface>` segment | See refinement below |
| OD-2 | **A (committed)** — JSON shape is structured object `"rule": {"scheme": ..., "predicate_id": ...}` | Spec-aligned |
| OD-3 | **A** — `Diagnostic` keeps a typed `rule: RuleId` (no field split) | Type carries the invariant |
| OD-4 | **A** — Engine sentinels DROP the `r001` / `r002` numeric prefix | Use `recognition.decoder-recognized` and `fix.reparse-failed` |
| OD-5 | **A** — Legacy ID strings survive ONLY in `legacy-rule-id-map.md` + git history | No alias map, no message-body retention |
| OD-6 | **A** — `RuleId::new` constructor is REPLACED (single 2-arg form) | Pre-users; rewrite freely |
| OD-7 | **A** — Severity-override config keys use the wire string in `[rules]` | `"capco:portion.dissem.noforn-conflicts-rel-to" = "off"` |
| OD-8 | **A** — Bridge dispatcher becomes a no-op pass-through | Catalog row labels ARE the predicate IDs; translation table deleted |

---

## OD-1 refinement: closure-rule predicate IDs

The architect's plan §1.3 defined `<surface>` as `{ banner, portion, page, marking }`.
Closure rules don't fire at a document surface — they're page-level inferences over the marking lattice. Forcing them into `page.<category>.<predicate>` would conflate them with strict page-banner rules at the predicate level.

**PM refinement:** add `closure` as a fifth permissible `<surface>` value.

Final surface enumeration:
- `banner` — banner-level (whole-page) rules
- `portion` — single-portion rules
- `page` — page-level (multi-portion) rules that aren't banner-equality checks
- `marking` — token-level rules that don't fit a surface split (e.g., corpus typo corrections)
- `closure` — closure-operator inferences (audit-note channel, not diagnostic channel)

Concrete closure-rule renames:

| Old `ClosureRule.name` | New wire-string form |
|---|---|
| `capco/noforn-if-caveated` | `capco:closure.dissem.noforn-if-caveated` |
| `capco/noforn-if-sar` | `capco:closure.dissem.noforn-if-sar` |
| `capco/noforn-if-aea-rd` | `capco:closure.dissem.noforn-if-aea-rd` |
| `capco/noforn-if-ucni` | `capco:closure.dissem.noforn-if-ucni` |
| `capco/noforn-if-fgi` | `capco:closure.dissem.noforn-if-fgi` |
| `capco/noforn-if-orcon` | `capco:closure.dissem.noforn-if-orcon` |
| `capco/noforn-if-imcon-dsen` | `capco:closure.dissem.noforn-if-imcon-dsen` |
| `capco/noforn-if-non-ic-controls` | `capco:closure.dissem.noforn-if-non-ic-controls` |
| (extend the table with the remaining closure rows in `crates/capco/src/scheme/closure.rs`) | … |

The `[closure_rules]` `.marque.toml` section keys also adopt the wire-string form. Section isolation (D19 B) is preserved by the section header itself, not by key-shape disambiguation.

The `AuditNote.structural.row_name` field — currently `&'static str` carrying the old slash form — also migrates to the new wire-string `&'static str`. Type unchanged; content updates.

---

## Engine-sentinel scheme: `"engine"`

Reserved. Not a valid `MarkingScheme` registration target. A grep-fence in `crates/rules/src/lib.rs` doc-comment names it explicitly.

Final sentinels:
- `R001` (decoder recognition): `("engine", "recognition.decoder-recognized")`
- `R002` (re-parse failure): `("engine", "fix.reparse-failed")`

Wire string forms: `"engine:recognition.decoder-recognized"`, `"engine:fix.reparse-failed"`.

---

## Test-fixture scheme: `"test"`

Also reserved. Used by `#[cfg(test)]` and `tests/` integration files for synthetic rule IDs (`E997`, `E998`, `E999`, `S999`, `RECORD`, `PARSED_CACHE_TEST`, `R999`). The grep-fence in `crates/rules/src/lib.rs` lists both `"engine"` and `"test"` as reserved.

---

## Branch + PR

- **Branch:** `feat/T044-rule-id-tuple-migration` (off `staging`)
- **PR title:** `feat(rules): T044 — RuleId 2-tuple migration + marque-1.0 → marque-2.0 audit-schema bump`
- **Target:** `staging`
- **FR-049 unfreeze:** explicitly authorized for this PR; the `CLAUDE.md` "Post-006 Stable Surface" section moves the rule-ID 2-tuple line from "Not frozen" to "Frozen as of T044" as part of the PR.

---

## Test-coverage discipline

This is a structural refactor — most coverage is preserved by existing tests reshaped to the new ID form. But the PR adds the following new code surfaces that MUST get explicit unit coverage:

- `RuleId::Display` impl — produces canonical wire string `"<scheme>:<predicate_id>"`
- `RuleId::scheme()` / `RuleId::predicate_id()` accessors
- `RuleIdJson<'_>` (the audit-record JSON view) — serialization round-trip
- Engine constraint-bridge no-op pass-through (verifies catalog row `name` IS the predicate ID; no string manipulation)
- `ExpectedRuleId` test-utils deserialization — both directly from JSON and via the corpus fixture path
- Reserved-scheme grep-fence — a unit test asserting `RuleId::new("engine", …)` and `RuleId::new("test", …)` are the only uses of those two strings, scanning the workspace at compile time (or a runtime-discovered allow-list check)

The user pattern is "if you add code, you cover it." Apply.

---

## 5-year maintainability commitments

Per the user's standard:

1. **Predicate-ID convention generalizes** to `marque-cui` / `marque-nato` without amendment (§5 R-1).
2. **JSON object form** absorbs future metadata additions without a schema break (§5 R-2).
3. **`legacy-rule-id-map.md` is a living document.** Appended to, never silently rewritten (§5 R-4).
4. **`marque-2.0` is the freeze inflection.** Subsequent renames require coordinated `marque-2.1` (additive) / `marque-3.0` (breaking) audit-schema bumps.

---

*Implementation runs from §4 of the plan with this addendum binding.*
