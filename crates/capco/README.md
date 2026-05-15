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

39 rules currently implemented (post-PR-4b-B, 006 T112): errors `E002`/`E005`–`E008`/`E010`/`E012`/`E014`–`E016`/`E021`/`E024`/`E031`/`E036`–`E039`/`E041`/`E053`–`E057`/`E061`–`E066`, style `S003`–`S007` (`S004` is the first suggest-don't-fix-channel rule per issue #235 / #186 PR-3; `S005`/`S006` cover REL TO membership-uncertain reduction per issue #206; `S007` covers bare NATO classification per PR 9c.2 / FR-048), warnings `W002`/`W003`/`W004`/`W034` (`W001` retired in T035c-14; `W004` added in PR 4b-B for joint-disunity-collapse per §H.3 p56 + §H.7 p123), corrections `C001`.

## Lattice Types (PR 4b-B)

Per-category lattice types live in `marque_capco::lattice` and round-trip with the corresponding `marque-ism` storage types:

- `ClassificationLattice` — bounded OrdMax over the US chain with variant-preservation; §H.1 pp47-54 + §H.7 pp123-125.
- `NatoClassLattice` — bounded OrdMax over NU<NR<NC<NS<CTS; §H.2 p55.
- `JointSet` — three-variant state with producer-disunity collapse; §H.3 p56 + §H.7 p123.
- `DissemSet` — single-bag IC dissem with three overlays (OC-USGOV supersession, RELIDO observed-unanimity, NOFORN dominates); §H.8 p136/p140/p145/pp155-156 + §D.2 Table 3.
- `NatoDissemSet` — trivial union over NATO-attributed dissem; CAPCO p41 reciprocity.
- `RelToBlock` — three-variant IntersectSet with NOFORN supersession + tetragraph expansion; §H.8 pp150-151 + §D.2 Table 3 rows 9-13 + §H.9 p172/p174.
- `DeclassifyOnLattice` — MaxDate semilattice (no top); §H.6 p104.
- `SciSet` / `SarSet` / `FgiSet` / `AeaSet` — PR 4b-A precedent (per-category Lattice impls over their structural storage types).

`CapcoMarking::join_via_lattice` composes the lattice types component-wise; the production `Lattice::join` still delegates to PageContext until PR 4b-D flips the hot path. The parity gate at `crates/capco/tests/page_context_lattice_parity.rs` covers 23 synthetic-fixture cases plus 7 documented-divergence fixtures (each carrying a `§X.Y pNN` citation); corpus-fixture parity is deferred to PR 4b-D when `CapcoScheme::project(Scope::Page, ...)` flips to the lattice path. PR 9a (issue #307) added five rules: `E061` (bare HCS at CONFIDENTIAL → contact originator, §H.4 p62), `E062` (bare HCS at S/TS → suggest HCS-O / HCS-P / HCS-O-P, §H.4 p62), `E063` (bare RSV requires compartment, §H.4 p70), `E064` (EYES / EYES ONLY → REL TO conversion, §H.8 p157 + p158), and `E065` (`DeprecatedSciLongFormRule` — HUMINT / COMINT / ECI / EL / ENDSEAL / KDK / KLONDIKE canonicalization, §H.4 pp 61, 62, 74, 76, 78, 85). PR 3c.B Commit 7.3 + 7.4 retired `DeclarativeClassFloorRule` (E058) and `DeclarativeSciPerSystemRule` (E059); the 27 + 5 catalog rows still fire via the engine's constraint-catalog bridge (E058 via the `ConstraintViolation` envelope path; E059 via `CapcoScheme::bridge_sci_per_system_diagnostics` so `FixProposal` survives). PR 3c.B Commit 6 (form-bucket migration) retired 13 hand-written form rules (`E001`/`E003`/`E004`/`E009`/`S001`/`S002`/`E011`/`E013`/`E026`/`E029`/`E030`/`E032`/`E052`) plus the E060 non-canonical input walker into `MarkingScheme::render_canonical`. Previously retired: `E017`/`E018`/`E019` in T035b; `E022`/`E025`/`E027` in PR 3b.D into the (now-retired) `E058` class-floor catalog walker; `E042`–`E051` in PR 3b.E into the (now-retired) `E059` SCI per-system catalog walker; `E020`/`E023`/`E028`/`E033` in PR 3b.F into the (now-retired) `E060` walker; `E035`/`E040` emitted as per-row IDs by the `E031` banner-roll-up walker. ID prefix encodes default severity (`E` = error, `W` = warning, `S` = style/info/suggest, `C` = correction). The 39 count matches the registered-rule pin in `crates/capco/tests/corpus_parity.rs::rule_count_reflects_registration_changes` and the exact-rule-ID-set pin in `crates/capco/tests/post_3b_registration_pin.rs::post_pr_4b_b_registers_exact_39_rule_ids`. Use `CapcoRuleSet::new()` or the `capco_rules()` entry point to obtain the full set.

PR 3b.D (T026d) added one walker rule `E058` (`DeclarativeClassFloorRule`) dispatched over a 27-row class-floor catalog declared as `Constraint::Custom` rows on `CapcoScheme` per `marque-applied.md` §3.4.6 family granularity. The catalog covers per-marking classification floors (HCS-[comp][sub] / SI-[comp] / TK-BLFH / NATO BALK + BOHEMIA at TS; HCS-[comp] / RSV-[comp] / TK / RD-SG / FRD-SG / CNWDI / RSEN / IMCON at S; SI / SAR / RD / FRD / TFNI / NATO ATOMAL / ORCON-family / EYES ONLY at C; DOD UCNI + DOE UCNI ceiling at =U; BUR / HCS-X / KLM / MVL passthrough at provisional C). Diagnostics emit with `Diagnostic.rule = "E058"`; per-row identification flows via the catalog row's `name` field (`"E058/CNWDI-classification-floor"`, `"E058/SAR-classification-floor"`, `"E058/DOD-UCNI-classification-ceiling"`, `"E058/DOE-UCNI-classification-ceiling"`, `"class-floor/<marking>"` for new rows). Severity-overridable per-walker via `[rules] E058 = "off|warn|error|..."`. See `docs/plans/2026-05-08-pr3b-D-class-floor-catalog-plan.md` for the architectural rationale; the forward-link comment in `crates/capco/src/scheme.rs` flags PR 3.7 (T108b) as the migration vehicle if `TokenRef::ClassAtLeast(ClassLevel)` lands as a primitive in `marque-scheme`.

PR 3b.C (T026c) added four RELIDO incompatibility rules (`E054`–`E057`) as declarative wrappers over `Constraint::Conflicts` entries in `CapcoScheme::constraints()`:
- **E054** — RELIDO ⊥ NOFORN (§H.8 p154)
- **E055** — RELIDO ⊥ DISPLAY ONLY (§H.8 p154)
- **E056** — ORCON ⊥ RELIDO (§H.8 p136; asserting side is the ORCON template)
- **E057** — ORCON-USGOV ⊥ RELIDO (§H.8 p140; asserting side is the ORCON-USGOV template)

All four emit a **subtractive `FixProposal`** that removes RELIDO from the dissem block (replacement = `""`, confidence = 0.95, `FixSource::BuiltinRule`). 0.95 clears the engine's default `Config::confidence_threshold = 0.95` (`crates/config/src/lib.rs:156`; auto-apply gate is `confidence >= threshold`), so the fix auto-applies under default config — matching the user-stated guidance behavior. The fix span covers RELIDO **plus an adjacent `/` separator** so the post-fix dissem block is well-formed (no dangling `//`, no leading or trailing `/`). RELIDO is the unambiguous remove-target in every case because the other token in each pair carries the binding §-cited authority — NOFORN dominates per FD&R supersession (§D.2 Table 3), DISPLAY ONLY is a positive disclosure decision that pre-empts RELIDO's deferred-release semantic, and ORCON / ORCON-USGOV explicitly assert "may not be used with RELIDO" on their §H.8 templates. The subtractive-fix pattern applies to dissem-axis `Constraint::Conflicts` rules **only**; non-dissem axis conflicts (classification, JOINT cross-system, SCI grammar) remain "user resolves" because the fix direction cannot be inferred without policy input. See PM Addendum II in `docs/plans/2026-05-07-pr3b-C-relido-conflicts-plan.md` for the full design rationale (including the 2026-05-08 confidence calibration from the initial 0.9 to 0.95).

The broader §3.4.2 family roster (RELIDO ⊥ {LES-NF, SBU-NF, FGI atoms, JOINT atoms, NATO atoms}) is deferred to PR 3.7 (T108b) where `Constraint::Conflicts::RhsFamily(predicate)` ships. All four citations are directly verified against the vendored `crates/capco/docs/CAPCO-2016.md` (D13 single-citation discipline, Constitution VIII).

PR 3b.E (T026e) added one walker rule `E059` (`DeclarativeSciPerSystemRule`) dispatched over a 5-row SCI per-system catalog declared as `Constraint::Custom` rows on `CapcoScheme` per CAPCO-2016 §H.4 family granularity. The catalog covers the §H.4 companion-required and forbid-companion invariants that PR 3b.D's class-floor catalog does not already cover: HCS-O companions (ORCON + NOFORN required, ORCON-USGOV forbidden; §H.4 p64), HCS-P NOFORN (§H.4 p66), HCS-P sub-compartment companions (ORCON required, ORCON-USGOV forbidden; §H.4 p68), SI-G companions (ORCON required, ORCON-USGOV forbidden; §H.4 p80), and TK-{BLFH,IDIT,KAND} NOFORN (§H.4 p87 + p91 + p95). The class-floor portions of E044/E045/E046/E048/E049/E050 are absorbed by PR 3b.D's class-floor catalog rows (`class-floor/HCS-comp-sub`, `class-floor/HCS-comp`, `class-floor/SI-comp`, `class-floor/RSV-comp`, `class-floor/TK`, `class-floor/TK-BLFH`); no class-floor rows are added in PR 3b.E. Diagnostics emit with `Diagnostic.rule = "E059"`; per-row identification flows via the catalog row's `name` field (`sci-per-system/<purpose>`). The walker preserves the legacy fix-and-warn pattern: `Severity::Warn` paired with a `FixProposal` (companion-insertion or OC-USGOV → OC replacement) escalating to `Severity::Error` no-fix when no IC dissem block exists. See `docs/plans/2026-05-08-pr3b-E-sci-per-system-collapse-plan.md` for the architectural rationale.

PR 3b.F (T026f) added one walker rule `E060` (`DeclarativeNonCanonicalInputRule`) dispatched over a 5-row **private** `&'static [NonCanonicalRow]` catalog inside `crates/capco/src/rules_declarative.rs`. The catalog is **structurally different** from PR 3b.D / 3b.E walkers: it is NOT a `Constraint::Custom` catalog on `CapcoScheme` because these are renderer-canonical-form concerns (per `marque-applied.md` §3.6 + §3.10 Move 7) that `MarkingScheme::render_canonical` will absorb once the renderer trait surface lands in PR 5+ (Stage 4 of the engine refactor); the walker retires cleanly when that lands. The catalog covers four ordering-validation invariants retired from hand-written rules: REL TO USA-first alpha (§H.8 p150-151, retired E020 REL TO arm), JOINT alpha (§H.3 p56, retired E020 JOINT arm), AEA SIGMA numeric sort (§H.6 p108, retired E023), SAR program ascending alpha (§H.5 p99, retired E028), SCI compartment + sub-compartment numeric-then-alpha (§H.4 p61, retired E033). Diagnostics emit with `Diagnostic.rule = "E060"`; per-row identification flows via the diagnostic message text + the `Diagnostic.citation` field (preserved verbatim from the retired rules so audit-stream consumers continue to work). Per-row severity preserved: `Severity::Fix` for rows 1-4, `Severity::Error` for row 5. Walker `default_severity()` = `Severity::Error` (strictest-of-rows precedent from PR 3b.A banner walker). The legacy E020/E023/E028/E033 IDs are intentionally NOT preserved as severity-config aliases (per `feedback_pre_users_no_deprecation_phasing.md`). See `docs/plans/2026-05-08-pr3b-F-non-canonical-input-walker-plan.md` for the architectural rationale.

**Migration note for E020 + E052 fixed-point convergence (R-1 in plan §6.6 / OQ-5 in §11):** before PR 3b.F, E020 and E052 resolved in one pass under the FR-016 lex tiebreaker (`E020 < E052`); E020's fix produced a fully canonical REL TO list (USA-first, alpha-sorted, deduped) in the same emit. Post-PR-3b.F, walker E060 + E052 reaches a fixed point in at most two passes: `E052 < E060` lex-orders, so E052 wins the first pass and dedups, then E060 fires on the now-misordered post-dedup list to sort. The final document state is identical (canonical REL TO list either way), but audit-stream consumers see two `AppliedFix.rule` entries (`E052` and `E060`) where they previously saw one (`E020`). This is an intentional, documented behavior change — see `docs/plans/2026-05-08-pr3b-F-non-canonical-input-walker-plan.md` §6.6 (R-1) and §11 OQ-5 for the resolution record.

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

The data lives in `src/scheme.rs` — there is no procedural rule body. The trigger's `Contains { CAT_DISSEM, TOK_NOFORN }` fires whenever NOFORN is present in the rolled-up dissem set; the action's `Clear { CAT_REL_TO }` empties REL TO. The dataflow annotation reads BOTH axes the rewrite touches: `CAT_DISSEM` (where it looks for the trigger token — and a real dataflow dep on the entry-5/6a/6b transmutations that write `CAT_DISSEM` per PR 3b.B) AND `CAT_REL_TO` (a defensive self-edge for any future rewrite that writes REL TO; no current entry does). The scheduler orders this rewrite AFTER the DISSEM writers per Kahn's algorithm.

At engine startup, `marque-engine::scheduler::schedule_rewrites` runs Kahn's algorithm over every rewrite's `reads` / `writes` axes to produce a deterministic order (writers before readers); cycles or unannotated `Custom` axes abort `Engine::new` with `EngineConstructionError`. The cached scheduled order lives on the engine. **Phase 3 runtime**: `Engine::lint` / `Engine::fix` consult the hand-coded `PageContext` aggregator directly for page roll-up; `CapcoScheme::project(Scope::Page, ...)` is invoked by tests and scheme-exploration tools and iterates `self.page_rewrites` in declaration order (per `crates/capco/src/scheme.rs::project`). Phase D / Phase E reconciles this — the engine switches to scheme-driven roll-up and `project()` consumes the engine's scheduled order. The `id` and `citation` travel into the audit record so a reviewer can see exactly which rewrites shaped the final banner.

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
