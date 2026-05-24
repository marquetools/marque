<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# marque-capco

CAPCO Layer 2 rule implementations for marque.

This crate provides hand-written rule implementations that consume the generated CVE predicates from `marque-ism` and produce enriched `Diagnostic` values — classifying *why* a violation occurred, attaching CAPCO citations, and emitting `FixProposal` values with confidence scores.

This is one of two crates where CAPCO/ISM is the headline; everything else in the workspace is general-purpose. For the engine that runs these rules, see `marque-engine`. For the vocabulary types they consume, see `marque-ism`.

> **Note on rule IDs**: rule IDs are 2-tuples `(scheme, predicate_id)` of the form `RuleId::new("capco", "<surface>.<category>.<predicate>")`. The canonical wire string `<scheme>:<predicate_id>` (e.g., `"capco:portion.dissem.noforn-conflicts-rel-to"`) is what users see in `.marque.toml` keys, audit-log text output, and grep targets. Some sections below reference the historical `E### / W### / S### / C###` flat-string IDs; those are **archaeological**. See [`docs/refactor-006/legacy-rule-id-map.md`](../../docs/refactor-006/legacy-rule-id-map.md) for the rename table mapping each retired flat-string ID to its 2-tuple successor.

## Role in Marque

Marque uses a two-layer rule architecture:

- **Layer 1 (generated)**: `marque-ism/build.rs` parses ODNI ISM schemas at build time (consumed via the `ism` and `ism-ismcat` build-dependencies from [`marquetools/ism-data`](https://github.com/marquetools/ism-data)) and emits binary valid/invalid predicates.
- **Layer 2 (this crate)**: hand-written `Rule` implementations that consume Layer 1 predicates, classify the violation reason, decide whether to propose a fix, and cite the relevant CAPCO section.

Rule structs are zero-size and stateless. All config-dependent behavior (severity overrides, confidence threshold, classifier identity) is handled by the engine. Fixes are returned as `FixProposal` (pure data) — the engine snapshots runtime state into `AppliedFix` at promotion time. Rule crates must never construct `AppliedFix` directly.

## Rule IDs

A rule ID is a 2-tuple `RuleId::new("capco", "<surface>.<category>.<predicate>")`, rendered as the wire string `capco:<surface>.<category>.<predicate>`. That wire string is what users see in `.marque.toml` keys, audit-log text output, and grep targets.

- `<surface>` ∈ `{ banner, portion, page, marking, closure }` — where the marking is observed.
- `<category>` matches the lattice axis (`classification`, `sci`, `sar`, `dissem`, `fgi`, `nato`, `aea`, `declassification`, `fouo`, `banner-rollup`, `metadata`).
- `<predicate>` is descriptive lowercase English-with-hyphens.

Default severity lives on the `Rule` trait (`Severity::Error | Warn | Suggest | Info`), not encoded in the ID. The reserved scheme `"engine"` covers synthetic engine-minted diagnostics; `marque-engine` mints `engine:recognition.decoder-recognized` and `engine:fix.reparse-failed`, not this crate.

The historic `E### / W### / S### / C###` flat-string IDs no longer name rules; [`docs/refactor-006/legacy-rule-id-map.md`](../../docs/refactor-006/legacy-rule-id-map.md) maps each retired form to its current wire string if you encounter an old reference.

## Rule Inventory

The crate registers **32 rules**. The exact set is pinned by `crates/capco/tests/post_3b_registration_pin.rs` (test `post_issue_677_registers_exact_32_rule_ids`); changing the registered set without updating that pin fails CI. Obtain the full set via `CapcoRuleSet::new()` or the `capco_rules()` entry point.

By surface and category:

- **Banner roll-up** — `capco:banner.banner-rollup.sar-portions-roll-up` validates SAR roll-up into the banner line. The walker emits additional per-row IDs for SCI and other banner roll-ups via `additional_emitted_ids`; those are not separately registered.
- **Page-level dissem** — REL TO and NODIS/EXDIS banner composition: `capco:page.dissem.nodis-exdis-clears-banner-rel-to`, `capco:page.dissem.non-ic-dissem-in-classified-banner`, `capco:page.dissem.bare-rel-portion-divergence` (#251), `capco:page.dissem.collapse-uniform-rel-portions` (#251, default Off), `capco:page.dissem.prefer-tetragraph-collapse` (#250, default Off), and `capco:page.dissem.rel-to-uncertain-reduction` (#206, §H.8 p150-151).
- **Portion dissem** — `capco:portion.dissem.rel-to-missing-usa`, `capco:portion.dissem.rel-to-trigraph-suggest` (suggest channel — engine never auto-applies), `capco:portion.dissem.eyes-only-convert-to-rel-to` (§H.8 p157), `capco:portion.dissem.nodis-supersedes-exdis-in-portion`, and `capco:portion.dissem.relido-implied-by-closure` (#559, §H.8 p154 + §D.2 Table 3).
- **Portion SCI** — bare-control diagnostics per §H.4: `capco:portion.sci.hcs-bare-at-confidential-legacy-remark` (§H.4 p62), `capco:portion.sci.hcs-bare-suggest-subcompartment` (§H.4 p62), `capco:portion.sci.rsv-bare-requires-compartment` (§H.4 p70), `capco:portion.sci.deprecated-long-form` (HUMINT/COMINT/ECI/etc. canonicalization), and `capco:portion.sci.unpublished-custom-control`.
- **Portion NATO / classification** — `capco:portion.nato.bare-nato-requires-rel-to-usa-nato` (bare NATO classification in a US-classified document should carry `REL TO USA, NATO`) and `capco:portion.classification.joint-usa-first-style`.
- **Portion FGI** — `capco:portion.fgi.fgi-explicit-with-trigraph` (#261) and `capco:portion.fgi.ownership-trigraph-suggest` (#545, suggest channel).
- **Marking metadata / corrections / recanonicalization** — `capco:marking.correction.token-typo` (text-corrections map), `capco:marking.metadata.unrecognized-token`, `capco:marking.deprecation.deprecated-dissem-control`, `capco:marking.fgi.invalid-ownership-token` (#501, §H.7 p123), `capco:marking.recanonicalize.legacy-nato-compound` (legacy NATO compound re-marking), and `capco:marking.recanonicalize.bare-canonical-compound` (#407, bare legacy short-forms to canonical compound portion marks).
- **Portion / page metadata + declassification** — `capco:portion.metadata.x-shorthand-date-pattern`, `capco:portion.declassification.declassify-on-misplaced`, `capco:banner.metadata.uses-portion-form` / `capco:portion.metadata.uses-banner-form` (form-mismatch detection, #677), and `capco:page.fgi.joint-disunity-collapses-to-fgi` (§H.3 p57 + §H.7 p123).

### Closure rules

The NOFORN-if-X family is keyed by the `[closure_rules]` config section, separate from `[rules]`. These cover NOFORN promotion when a portion carries a caveated marking (SAR, AEA RD, UCNI, FGI, ORCON, IMCON/DSEN, or a non-IC control). See `crates/capco/src/scheme/closure.rs`.

## Lattice Types

Per-category lattice types live in `marque_capco::lattice` and round-trip with the corresponding `marque-ism` storage types:

- `ClassificationLattice` — bounded OrdMax over the US chain with variant-preservation; §H.1 pp47-54 + §H.7 pp123-125.
- `NatoClassLattice` — bounded OrdMax over NU<NR<NC<NS<CTS; §H.2 p55.
- `JointSet` — four-variant state (`Bottom` / `UnanimousProducers` / `DisunityCollapse` / `Mixed`) with producer-disunity collapse and a `Mixed` absorbing state for JOINT+non-JOINT pages; §H.3 p57. The disunity-collapse migration trigger lives on p57's "Derivative Use" bullets; §H.3 p56 grounds the JOINT grammar separately.
- `DissemSet` — single-bag IC dissem with three overlays (OC-USGOV supersession, RELIDO observed-unanimity, NOFORN dominates); §H.8 p136/p140/p145/pp155-156 + §D.2 Table 3.
- `NatoDissemSet` — trivial union over NATO-attributed dissem; CAPCO p41 reciprocity.
- `RelToBlock` — four-variant IntersectSet (`Bottom` / `Lattice{countries}` / `Empty` / `NofornSuperseded`) with NOFORN supersession + tetragraph expansion. `Empty` absorbs non-Bottom operands on empty intersection (§D.2 Table 3 row 9 — the post-projection pipeline injects NOFORN via `capco/noforn-clears-rel-to`); §H.8 pp150-151 + §D.2 Table 3 rows 9-13 + §H.9 p172/p174.
- `DeclassifyOnLattice` — MaxDate semilattice (no top); §H.6 p104.
- `SciSet` / `SarSet` / `FgiSet` / `AeaSet` — per-category Lattice impls over their structural storage types.

`CapcoMarking::join_via_lattice` composes the lattice types component-wise. The production hot path drives page aggregation through `scheme.project(Scope::Page, ...)`, which composes the lattice path plus closure rules plus the `PageRewrite` catalog.

## Declarative rule pattern

Alongside hand-written `Rule` structs, dyadic invariants and page-level rewrites are declared as **data** on `CapcoScheme` rather than as procedural rule bodies. The shared evaluator in `marque-scheme` walks the catalog; the engine's topological scheduler orders rewrites by their dataflow axes.

The two declarative shapes:

- **`Constraint`** (`marque-scheme::constraint`) — `Conflicts`, `Requires`, `Implies`, `Supersedes` for dyadic relationships, plus `Custom` as the escape hatch for n-ary or scheme-specific predicates (SIGMA numeric ordering, CNWDI classification floor, etc.). Every variant carries a stable `name` (the rule identifier in diagnostics) and a `label` (the authoritative-source citation). Declared in `CapcoScheme::constraints()` (`src/scheme.rs`).
- **`PageRewrite`** (`marque-scheme::page_rewrite`) — post-aggregation cross-category transforms with explicit `reads` / `writes` axis annotations. Variants pair a `CategoryPredicate` trigger with a `CategoryAction` (`Clear`, `Replace`, `Promote`). Declared in `CapcoScheme::page_rewrites()` (`src/scheme.rs`).

This covers the class-floor catalog (per-marking classification floors), the SCI per-system companion-required and forbid-companion invariants (§H.4), and the §H.6 / §H.8 / §H.9 strip-plus-promote semantics (e.g. classification-driven strip of UNCLASSIFIED-only controls, FOUO eviction per §H.8 p134, UCNI promotion at §H.6 p116). Ordering-validation invariants (REL TO USA-first alpha, JOINT alpha, AEA SIGMA numeric sort, SAR program ascending alpha, SCI compartment numeric-then-alpha) are handled by `MarkingScheme::render_canonical`.

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

The data lives in `src/scheme.rs` — there is no procedural rule body. The trigger's `Contains { CAT_DISSEM, TOK_NOFORN }` fires whenever NOFORN is present in the rolled-up dissem set; the action's `Clear { CAT_REL_TO }` empties REL TO. The dataflow annotation reads both axes the rewrite touches: `CAT_DISSEM` (where it looks for the trigger token, and a real dataflow dependency on the transmutations that write `CAT_DISSEM`) and `CAT_REL_TO` (a defensive self-edge for any future rewrite that writes REL TO; no current entry does). The scheduler orders this rewrite after the DISSEM writers.

At engine startup, `marque-engine::scheduler::schedule_rewrites` runs Kahn's algorithm over every rewrite's `reads` / `writes` axes to produce a deterministic order (writers before readers); cycles or unannotated `Custom` axes abort `Engine::new` with `EngineConstructionError`. The cached scheduled order lives on the engine. At runtime, `Engine::lint` / `Engine::fix` accumulate per-page portion attributes into an inline `Vec<CanonicalAttrs>` (one allocation per page) then drive page roll-up through `CapcoScheme::project_from_attrs_slice`, the engine fast-path that delegates to the same shared pipeline body the trait-level `MarkingScheme::project` uses. The pipeline iterates `self.page_rewrites` in declaration order. The `id` and `citation` travel into the audit record so a reviewer can see exactly which rewrites shaped the final banner.

A dyadic example on the same surface is a `Constraint::Conflicts` between `TOK_JOINT` and `TOK_HCS`, citing `"CAPCO-2016 §H.3 p57"` (the JOINT marking template's "May not be used with the HCS markings or NOFORN markings"). The shared evaluator emits the diagnostic; the rule is one struct literal.

### Why this pattern matters

- **Open/closed scheme adoption.** A future scheme crate (CUI, NATO, JOINT, partner-national) declares its constraints and rewrites as data and reuses the shared evaluator and scheduler. A scheme-adoption PR does not edit `marque-engine` or `marque-scheme`.
- **Corpus byte-identical guarantee.** Every migrated rule produces the same diagnostic stream as its hand-written predecessor, validated by the per-rule corpus accuracy harness. Migrations that drift are caught at CI.
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
2. Choose a 2-tuple ID `RuleId::new("capco", "<surface>.<category>.<predicate>")`. `<surface>` ∈ `{ banner, portion, page, marking, closure }`; `<category>` matches the lattice axis (`classification | sci | sar | dissem | fgi | nato | aea | declassification | fouo | banner-rollup | metadata`); `<predicate>` is descriptive lowercase English-with-hyphens. The default-severity tier (`Severity::Error | Severity::Warn | Severity::Suggest | Severity::Info`) lives on the `Rule` trait, not in the ID.
3. Register it in `CapcoRuleSet::new()`.
4. Return `FixProposal` values for fixable violations; set `confidence` honestly. The engine's threshold gate decides what is auto-applied.
5. Cite the CAPCO section in the `Diagnostic::citation` field, and verify the citation against the primary source — `crates/capco/docs/CAPCO-2016.md` — per Constitution Principle VIII.

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
