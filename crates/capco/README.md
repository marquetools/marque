<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# marque-capco

CAPCO Layer 2 rule implementations for marque.

This crate provides hand-written rule implementations that consume the generated CVE predicates from `marque-ism` and produce enriched `Diagnostic` values — classifying *why* a violation occurred, attaching CAPCO citations, and emitting `FixProposal` values with confidence scores.

This is one of two crates where CAPCO/ISM is the headline; everything else in the workspace is general-purpose. For the engine that runs these rules, see `marque-engine`. For the vocabulary types they consume, see `marque-ism`.

## Role in Marque

Marque uses a two-layer rule architecture:

- **Layer 1 (generated)**: `marque-ism/build.rs` parses ODNI ISM schemas at build time (consumed via the `ism` and `ism-ismcat` build-dependencies from [`marquetools/ism-data`](https://github.com/marquetools/ism-data)) and emits binary valid/invalid predicates.
- **Layer 2 (this crate)**: hand-written `Rule` implementations that consume Layer 1 predicates, classify the violation reason, decide whether to propose a fix, and cite the relevant CAPCO section.

Rule structs are zero-size and stateless. All config-dependent behavior (severity overrides, confidence threshold, classifier identity) is handled by the engine. Fixes are returned as `FixProposal` (pure data) — the engine snapshots runtime state into `AppliedFix` at promotion time. Rule crates must never construct `AppliedFix` directly.

## Rule Inventory

56 rules currently implemented: errors `E001`–`E052` (core CAPCO, SAR, SCI, NODIS/EXDIS, per-SCI-system constraints, REL TO list-grammar invariants; `E017`/`E018`/`E019` retired in T035b), style `S001`–`S004` (`S004` is the first suggest-don't-fix-channel rule per issue #235 / #186 PR-3), warnings `W002`–`W003` (`W001` retired in T035c-14), corrections `C001`. ID prefix encodes default severity (`E` = error, `W` = warning, `S` = style/info/suggest, `C` = correction). Use `CapcoRuleSet::new()` or the `capco_rules()` entry point to obtain the full set.

The E042–E051 cluster uses the **fix-and-warn** pattern: `Severity::Warn` paired with a `FixProposal` — the fix is applied when confidence clears threshold, AND the warn diagnostic stays in the output so the user sees exactly what was corrected and can override if the intent was actually different. See [`rules_sci_per_system`](src/rules_sci_per_system.rs) module doc for the rationale.

## Declarative rule pattern (Phase 4+)

Phase 4 introduced a second form of Layer 2 alongside hand-written `Rule` structs: dyadic invariants and page-level rewrites are declared as **data** on `CapcoScheme` rather than as procedural rule bodies. The shared evaluator in `marque-scheme` walks the catalog; the engine's topological scheduler orders rewrites by their dataflow axes. Approximately one-third of CAPCO's hand-written rules retire into this surface (SC-005), with byte-identical corpus diagnostics before and after migration.

The two declarative shapes:

- **`Constraint`** (`marque-scheme::constraint`) — `Conflicts`, `Requires`, `Implies`, `Supersedes` for dyadic relationships, plus `Custom` as the escape hatch for n-ary or scheme-specific predicates (SIGMA numeric ordering, CNWDI classification floor, etc.). Every variant carries a stable `name` (the rule identifier in diagnostics) and a `label` (the authoritative-source citation). Declared in `CapcoScheme::constraints()` (`src/scheme.rs`).
- **`PageRewrite`** (`marque-scheme::page_rewrite`) — post-aggregation cross-category transforms with explicit `reads` / `writes` axis annotations. Variants pair a `CategoryPredicate` trigger with a `CategoryAction` (`Clear`, `Replace`, `Promote`). Declared in `CapcoScheme::page_rewrites()` (`src/scheme.rs`).

### Worked example: `capco/noforn-clears-rel-to`

CAPCO-2016 §D.2 Table 3 (FD&R Markings Precedence Rules for Banner Line Roll-Up) rule 2 establishes that NOFORN supersedes REL TO at banner scope: a portion carrying `NF` paired with any other FD&R marking — including `REL TO [USA, LIST]`, `RELIDO`, `USA/[LIST] EYES ONLY`, or `DISPLAY ONLY [LIST]` — rolls up to a banner-line `NOFORN`. The §H.8 NOFORN entry (p145) back-references that table. The trigger and the effect live in different categories (`CAT_DISSEM` and `CAT_REL_TO`), so the rule cannot be expressed as a single-category lattice join. It lands as a `PageRewrite::declarative` entry on `CapcoScheme::page_rewrites()`:

```rust
PageRewrite::declarative(
    "capco/noforn-clears-rel-to",
    "CAPCO-2016 §D.2 Table 3 + §H.8 p145",
    CategoryPredicate::Contains {
        category: CAT_DISSEM,
        token: TOK_NOFORN,
    },
    CategoryAction::Clear { category: CAT_REL_TO },
    NF_READS,   // &[CAT_DISSEM, CAT_REL_TO]
    NF_WRITES,  // &[CAT_REL_TO]
)
```

The data lives in `src/scheme.rs` — there is no procedural rule body. The trigger's `Contains { CAT_DISSEM, TOK_NOFORN }` fires whenever NOFORN is present in the rolled-up dissem set; the action's `Clear { CAT_REL_TO }` empties REL TO. The dataflow annotation reads BOTH axes the rewrite touches: `CAT_DISSEM` (where it looks for the trigger token) AND `CAT_REL_TO` (so the scheduler orders this rewrite AFTER any rewrite that *writes* REL TO — e.g., `capco/joint-promotion` promotes JOINT country lists into REL TO; the NOFORN clear must observe those promoted countries before deciding to drop them).

At engine startup, `marque-engine::scheduler::schedule_rewrites` runs Kahn's algorithm over every rewrite's `reads` / `writes` axes to produce a deterministic order (writers before readers); cycles or unannotated `Custom` axes abort `Engine::new` with `EngineConstructionError`. At `lint()` / `fix()` time, `CapcoScheme::project(Scope::Page, ...)` runs each rewrite in scheduled order against the page-aggregated marking. The `id` and `citation` travel into the audit record so a reviewer can see exactly which rewrites shaped the final banner.

A dyadic example shipped on the same surface is `E036/joint-conflicts-hcs` — `Constraint::Conflicts` between `TOK_JOINT` and `TOK_HCS`, citing `"CAPCO-2016 §H.3 p57"` (the JOINT marking template's "May not be used with the HCS markings or NOFORN markings"). The shared evaluator emits the diagnostic; the rule is one struct literal.

### Why this pattern matters

- **FR-022 / Constitution Principle IV.** Future scheme crates (CUI, NATO, JOINT, partner-national) declare their constraints and rewrites as data and reuse the shared evaluator and scheduler. A scheme-adoption PR does not edit `marque-engine` or `marque-scheme`.
- **Corpus byte-identical guarantee.** Every migrated rule produces the same diagnostic stream as its hand-written predecessor, validated by the per-rule corpus accuracy harness (SC-003). Migrations that drift are caught at CI.
- **Tooling visibility.** A scheme-exploration UI, a docs generator, or a constraint-catalog renderer can walk `MarkingScheme::constraints()` and `MarkingScheme::page_rewrites()` without executing scheme code — the data form makes the full aggregation semantics introspectable.
- **Citation discipline (Constitution Principle VIII).** Every declarative entry's `label` / `citation` field carries an authoritative-source reference identifying the controlling CAPCO passage (e.g., `"CAPCO-2016 §H.6 p104"`); the shared evaluator copies it into `ConstraintViolation::citation` so source provenance travels with the diagnostic. The reference, not the verbatim passage text, is what travels — readers chase the cite to verify.

## Usage

```rust
use marque_capco::{capco_rules, SCHEMA_VERSION};

let rules = capco_rules();
println!("CAPCO rules compiled against {SCHEMA_VERSION}");
// Hand `rules` to marque_engine::Engine to lint a document.
```

## Adding a New Rule

1. Add a zero-size struct in `src/rules.rs` implementing `marque_rules::Rule`.
2. Choose an ID with the right severity prefix: `E###` error, `W###` warning, `C###` correction.
3. Register it in `CapcoRuleSet::new()`.
4. Return `FixProposal` values for fixable violations; set `confidence` honestly. The engine's threshold gate decides what is auto-applied.
5. Cite the CAPCO section in the `Diagnostic::citation` field.

## Schema Versioning

The active ISM schema version is pinned in `marque-ism/Cargo.toml` under `[package.metadata.marque]`:

| Pin | Meaning |
|-----|---------|
| `ism-schema-version` | Upstream ODNI ISM package label (e.g. `ISM-v2022-DEC`) — re-exported here as `SCHEMA_VERSION` |
| `ism-data-version` | Snapshot of the [`ism-data`](https://github.com/marquetools/ism-data) workspace whose `ism` / `ism-ismcat` build-deps `marque-ism` resolves |
| `ismcat-tetra-version` | ISMCAT Tetragraph Taxonomy revision |

Bump intentionally — and in lock-step with the corresponding `[build-dependencies]` versions — when ODNI publishes spec updates and the `ism-data` workspace is re-vendored.

## License

Marque License 1.0 (`LicenseRef-MarqueLicense-1.0`). See [LICENSE.md](./LICENSE.md).
