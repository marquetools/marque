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

## Rule Inventory

30 rules currently registered at HEAD. The exact set is gated by `crates/capco/tests/post_3b_registration_pin.rs` against the wire-string canonical forms. Coverage shape (legacy IDs in parentheses for archaeology — see `legacy-rule-id-map.md` for current wire strings):

- **Banner / portion / page rules**: `E002` (USA-trigraph missing), `E005`–`E008`, `E031` (banner-roll-up walker emitting per-row IDs), `E036`–`E039`/`E041`, `E061`–`E067`, `E068`–`E069`, `E071` (FGI with explicit trigraph, #261), `E072` (#251).
- **SCI per-system catalog rows** (5 active): the per-system invariants are declared as catalog rows and fire via the engine constraint-bridge.
- **Class-floor catalog rows** (27 active): the per-marking classification floors are declared as catalog rows and fire via the engine constraint-bridge.
- **Closure rules** (10 active, audit-note channel): NOFORN-if-caveated / NOFORN-if-sar / NOFORN-if-aea-rd / NOFORN-if-ucni / NOFORN-if-fgi / NOFORN-if-orcon / NOFORN-if-imcon-dsen / NOFORN-if-non-ic-controls plus two more per `crates/capco/src/scheme/closure.rs`; keyed by the `[closure_rules]` config section, not `[rules]`.
- **Style/suggest**: `S003`–`S005`/`S007`–`S010` (`S004` is the first suggest-don't-fix channel, issue #235 / #186; `S005` covers REL TO membership-uncertain reduction, issue #206, at `Phase::PageFinalization`; `S007` covers bare NATO classification; `S008` from #559; `S009`/`S010` from #250/#251).
- **Warnings**: `W003`/`W004`/`W034` (`W001` retired per CAPCO §F; `W002` retired (#470) per §H.7 p123 — the shape it warned on is now authorized by the manual; `W004` is joint-disunity-collapse per §H.3 p57 + §H.7 p123).
- **Corrections**: `C001` (text-corrections map).
- **Engine sentinels** (`"engine"` scheme): `R001` (`"recognition.decoder-recognized"`) and `R002` (`"fix.reparse-failed"`) minted by `marque-engine`, not by this crate.

## Lattice Types

Per-category lattice types live in `marque_capco::lattice` and round-trip with the corresponding `marque-ism` storage types:

- `ClassificationLattice` — bounded OrdMax over the US chain with variant-preservation; §H.1 pp47-54 + §H.7 pp123-125.
- `NatoClassLattice` — bounded OrdMax over NU<NR<NC<NS<CTS; §H.2 p55.
- `JointSet` — four-variant state (`Bottom` / `UnanimousProducers` / `DisunityCollapse` / `Mixed`) with producer-disunity collapse and a `Mixed` absorbing state for JOINT+non-JOINT pages per §H.3 p57; §H.3 p57 + §H.7 p123 (the disunity-collapse trigger lives on §H.3 p57's "Derivative Use" bullets; §H.3 p56 grounds the JOINT grammar separately).
- `DissemSet` — single-bag IC dissem with three overlays (OC-USGOV supersession, RELIDO observed-unanimity, NOFORN dominates); §H.8 p136/p140/p145/pp155-156 + §D.2 Table 3.
- `NatoDissemSet` — trivial union over NATO-attributed dissem; CAPCO p41 reciprocity.
- `RelToBlock` — four-variant IntersectSet (`Bottom` / `Lattice{countries}` / `Empty` / `NofornSuperseded`) with NOFORN supersession + tetragraph expansion. `Empty` absorbs non-Bottom operands on empty intersection (§D.2 Table 3 row 9 — the post-projection pipeline injects NOFORN via `capco/noforn-clears-rel-to`); §H.8 pp150-151 + §D.2 Table 3 rows 9-13 + §H.9 p172/p174.
- `DeclassifyOnLattice` — MaxDate semilattice (no top); §H.6 p104.
- `SciSet` / `SarSet` / `FgiSet` / `AeaSet` — per-category Lattice impls over their structural storage types.

`CapcoMarking::join_via_lattice` composes the lattice types component-wise. The production hot path drives page aggregation through `scheme.project(Scope::Page, ...)` (which composes the lattice path + closure rules + PageRewrite catalog). The parity gate at `crates/capco/tests/lattice_vs_scheme_parity.rs` compares `project_via_lattice` (per-axis lattice composition) against `project_via_scheme` (full pipeline including the PageRewrite catalog). See `CAPCO-CONTEXT.md` §3 for the enumerated divergence list — the load-bearing class is the §B.3 Table 2 p21 closure-rule asymmetry: scheme runs `CLOSURE_NOFORN_CAVEATED`, per-axis lattice doesn't.

## Declarative PageRewrite catalog

The page-rewrite catalog (30 rows) declares the §H.6 / §H.8 / §H.9 strip-plus-promote semantics as data on `CapcoScheme`. The rows are scheduler-validated at `Engine::new`, and `scheme.project(Scope::Page, ...)` is the single source of truth for page roll-up.

**Pattern-C** — 7 rows for classification-driven strip of UNCLASSIFIED-only controls: `capco/limdis-evicted-by-classified` (§H.9 p170), `capco/sbu-evicted-by-classified` (§H.9 p176), the four UCNI rows (`capco/{dod,doe}-ucni-promotes-noforn-when-classified` + `capco/{dod,doe}-ucni-evicted-by-classified` at §H.6 p116 / p118; promote rows declared before strip rows so the promote's predicate sees UCNI before the strip removes it), and `capco/fouo-evicted-by-classified` (§H.8 p134 classified sub-clause). The UCNI promote-before-strip ordering preserves the §H.6 NOFORN-promotion clause on classified-context UCNI; the load-bearing test is `pattern_c_dod_ucni_classified_strip_promotes_noforn` in the parity gate.

**Pattern-B** — 2 structural rows per §H.8 p134 verbatim: `capco/classification-evicts-fouo` (classified-document sub-clause) + `capco/non-fdr-control-evicts-fouo` (UNCLASSIFIED with other non-FD&R control sub-clause). "Non-FD&R" follows `Vocabulary::is_fdr_dissem`'s broad semantic (INCLUDES RELIDO) — distinct from `is_fdr_dominator` (which EXCLUDES RELIDO).

**Pattern D** (caveated-implies-NOFORN, §B.3 Table 2 p21) is wired via `CLOSURE_NOFORN_CAVEATED` on `CapcoScheme::closure()`.

The **class-floor catalog** (27 rows) and **SCI per-system catalog** (5 rows) are declared as `Constraint::Custom` rows on `CapcoScheme` and fire via the engine's constraint-catalog bridge (the class-floor rows via the `ConstraintViolation` envelope path; the SCI per-system rows via `CapcoScheme::bridge_sci_per_system_diagnostics` so the `FixProposal` survives). The class-floor catalog covers per-marking classification floors (HCS-[comp][sub] / SI-[comp] / TK-BLFH / NATO BALK + BOHEMIA at TS; HCS-[comp] / RSV-[comp] / TK / RD-SG / FRD-SG / CNWDI / RSEN / IMCON at S; SI / SAR / RD / FRD / TFNI / NATO ATOMAL / ORCON-family / EYES ONLY at C; DOD UCNI + DOE UCNI ceiling at =U; BUR / HCS-X / KLM / MVL passthrough at provisional C). The SCI per-system catalog covers the §H.4 companion-required / forbid-companion invariants: HCS-O companions (ORCON + NOFORN required, ORCON-USGOV forbidden; §H.4 p64), HCS-P NOFORN (§H.4 p66), HCS-P sub-compartment companions (ORCON required, ORCON-USGOV forbidden; §H.4 p68), SI-G companions (ORCON required, ORCON-USGOV forbidden; §H.4 p80), and TK-{BLFH,IDIT,KAND} NOFORN (§H.4 p87 + p91 + p95).

The **RELIDO incompatibility** rows live in `relido_clears.rs` as subtractive page rewrites that remove RELIDO from the dissem block: `capco/display-only-clears-relido` (§H.8 p154), `capco/orcon-clears-relido` (§H.8 p136), `capco/orcon-usgov-clears-relido` (§H.8 p140), plus a `Constraint::Conflicts` row for RELIDO ⊥ NOFORN (§H.8 p154). RELIDO is the unambiguous remove-target because the other token in each pair carries the binding §-cited authority — NOFORN dominates per FD&R supersession (§D.2 Table 3), DISPLAY ONLY is a positive disclosure decision that pre-empts RELIDO's deferred-release semantic, and ORCON / ORCON-USGOV explicitly assert "may not be used with RELIDO" on their §H.8 templates. The subtractive-fix pattern applies to dissem-axis conflicts **only**; non-dissem axis conflicts (classification, JOINT cross-system, SCI grammar) remain "user resolves" because the fix direction cannot be inferred without policy input.

The registered rule count is gated by the count pin in `crates/capco/tests/corpus_parity.rs::rule_count_reflects_registration_changes` and the exact-ID-set pin in `crates/capco/tests/post_3b_registration_pin.rs` (keyed by 2-tuple wire strings). Use `CapcoRuleSet::new()` or the `capco_rules()` entry point to obtain the full set.

## Declarative rule pattern

A second form of Layer 2 sits alongside hand-written `Rule` structs: dyadic invariants and page-level rewrites are declared as **data** on `CapcoScheme` rather than as procedural rule bodies. The shared evaluator in `marque-scheme` walks the catalog; the engine's topological scheduler orders rewrites by their dataflow axes. Roughly one-third of CAPCO's hand-written rules live on this surface, with byte-identical corpus diagnostics before and after migration.

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

The data lives in `src/scheme.rs` — there is no procedural rule body. The trigger's `Contains { CAT_DISSEM, TOK_NOFORN }` fires whenever NOFORN is present in the rolled-up dissem set; the action's `Clear { CAT_REL_TO }` empties REL TO. The dataflow annotation reads BOTH axes the rewrite touches: `CAT_DISSEM` (where it looks for the trigger token — and a real dataflow dep on the transmutation rows that write `CAT_DISSEM`) AND `CAT_REL_TO` (a defensive self-edge for any future rewrite that writes REL TO; no current entry does). The scheduler orders this rewrite AFTER the DISSEM writers per Kahn's algorithm.

At engine startup, `marque-engine::scheduler::schedule_rewrites` runs Kahn's algorithm over every rewrite's `reads` / `writes` axes to produce a deterministic order (writers before readers); cycles or unannotated `Custom` axes abort `Engine::new` with `EngineConstructionError`. The cached scheduled order lives on the engine. **Runtime**: `Engine::lint` / `Engine::fix` accumulate per-page portion attributes into an inline `Vec<CanonicalAttrs>` (one allocation per page, pre-sized to `DEFAULT_PORTIONS_CAPACITY = 8`), then drive page-rollup through `CapcoScheme::project_from_attrs_slice` (the engine fast-path that delegates to the same shared `project_attrs_pipeline` body the trait-level `MarkingScheme::project` uses). The pipeline iterates `self.page_rewrites` in declaration order. The `id` and `citation` travel into the audit record so a reviewer can see exactly which rewrites shaped the final banner.

A dyadic example shipped on the same surface is `E036/joint-conflicts-hcs` — `Constraint::Conflicts` between `TOK_JOINT` and `TOK_HCS`, citing `"CAPCO-2016 §H.3 p57"` (the JOINT marking template's "May not be used with the HCS markings or NOFORN markings"). The shared evaluator emits the diagnostic; the rule is one struct literal.

### Why this pattern matters

- **Constitution Principle IV.** Future scheme crates (CUI, NATO, JOINT, partner-national) declare their constraints and rewrites as data and reuse the shared evaluator and scheduler. A scheme-adoption PR does not edit `marque-engine` or `marque-scheme`.
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
2. Choose a 2-tuple ID `RuleId::new("capco", "<surface>.<category>.<predicate>")`. `<surface>` ∈ `{ banner, portion, page, marking, closure }`; `<category>` matches the lattice axis (`classification | sci | sar | dissem | fgi | nato | aea | declassification | fouo | banner-rollup | metadata`); `<predicate>` is descriptive English-with-hyphens, lowercase. The default-severity tier (`Severity::Error | Severity::Warn | Severity::Suggest | Severity::Info`) lives on the `Rule` trait, not in the ID.
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
