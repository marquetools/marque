<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# T044 — `RuleId` 2-Tuple Migration: Tactical Implementation Plan

**Date**: 2026-05-22
**Scope**: Post-PR-10 dedicated PR carrying the FR-049-unfrozen `RuleId` 2-tuple migration
**Authority**: Spec FR-026, FR-044, FR-049, FR-035a; `contracts/audit-record.md` §"Post-`marque-1.0` RuleId migration"; research.md R-3; Constitution V (audit), VII (crate graph)
**Companion**: `docs/refactor-006/2026-05-22-T044-callsite-inventory.md` (~106 `RuleId::new` sites, 67 corpus fixtures, 7 in-crate JSON fixture sites)

This is a preflight plan. Implementation agents execute from it after PM closure on the open decisions in §3.

---

## Executive context

The current `RuleId` is a single-element newtype around `&'static str` carrying values like `"E054"`, `"R001"`, `"R002"`. Spec FR-026 + research.md R-3 + `contracts/audit-record.md` §"Post-`marque-1.0` RuleId migration" all agree on a 2-tuple replacement:

```rust
RuleId { scheme: &'static str, predicate_id: &'static str }
```

with the canonical surface form `<surface>.<category>.<predicate>` for predicate IDs and the reserved sentinel `("engine", "r001.decoder-recognized")` / `("engine", "r002.reparse-failed")` for engine-minted diagnostics (FR-044).

The FR-049 stability freeze that locked the 1-tuple shape lifts at this PR's merge. After this PR, a `marque-2.0` audit-schema bump pins the new shape, and the contract documents at `specs/006-engine-rule-refactor/contracts/audit-record.md` are updated to reflect what shipped (the "Post-`marque-1.0` RuleId migration" section becomes the live spec, the `marque-1.0` 1-tuple section becomes historical).

Marque is pre-users (project memory `feedback_pre_users_no_deprecation_phasing.md`). No alias map, no transitional shape, no schema-version accept-list. Atomic cutover.

---

## §1 — Predicate-ID naming convention (decided)

### §1.1 Separator: `.` (dotted), not `/`

**Decision**: dot-separated within the predicate ID. The 2-tuple itself uses no separator — `scheme` and `predicate_id` are distinct struct fields.

**Rationale**: research.md R-3 already specifies "dot-separated, lowercase, structural-not-numeric." `contracts/audit-record.md` §"Post-`marque-1.0` RuleId migration" already prints the wire-form examples with dots (`"banner.classification.usa-trigraph"`). The existing `Constraint::Custom` catalog uses slash-separated kebab-paths (`"class-floor/HCS-comp-sub"`, `"sci-per-system/HCS-O-companions"`, `"capco/noforn-conflicts-rel-to"`) — but those catalogs live below the rule-ID layer and carry a different role (catalog row labels keyed by the engine's constraint bridge, scheme-internal). Reusing slash for rule IDs while the catalog rows continue using slash would invite collision and category-error confusion at audit-log triage time.

Picking `.` for the predicate ID and leaving `/` to the existing catalog-row label convention is the clean split. The catalog rows are not exposed in audit output as rule IDs; the bridge translates them (see §1.5 below).

### §1.2 Capitalization: lowercase, hyphen-joined English

**Decision**: lowercase letters and digits only, hyphens for multi-word predicate tail. ASCII only.

**Rationale**: Inherited from R-3 and from the existing catalog convention (`class-floor/HCS-comp-sub` uses hyphens; the `<predicate>` tail is the analogous component). Hyphens read better than dots when the predicate is descriptive English (`usa-trigraph`, `noforn-supersedes-relto`, `evicted-by-non-fdr-dissem`). All-lowercase keeps grep / `.marque.toml` config keys / audit-log search deterministic.

One exception is permitted: when a CAPCO short-form token appears verbatim in the predicate (e.g., the SCI control system `hcs-p-sub`), case-insensitive matching is fine for human reading but the canonical written form stays lowercase to keep the wire shape unambiguous.

### §1.3 Encoding axis/category in the predicate ID: required, three-segment minimum

**Decision**: every predicate ID has at minimum three segments: `<surface>.<category>.<predicate>`.

- `<surface>` ∈ `{ banner, portion, page, marking }` for scheme rules, plus the engine reserves `engine.r001`, `engine.r002`, … as the first two segments for sentinel-scheme synthetic diagnostics (where there is no document surface — the engine itself emits the diagnostic).
- `<category>` matches the lattice / axis category for surface rules: `classification | sci | sar | dissem | fgi | nato | aea | declassification | fouo | banner-rollup | metadata`. The `marking` surface uses `marking.<category>.<predicate>` for rules that don't fit a banner/portion/page split (e.g., corpus typo corrections — `marking.correction.token-typo`).
- `<predicate>` is descriptive English-with-hyphens.

**Why three segments are mandatory**: audit-log triage. The murder-board citation defects the spec already documents (HCS-P fabrication; `p150–151 p151` doubling) showed that obscure rule IDs (`E028` etc.) hide the underlying predicate. A two-segment form like `("capco", "usa-trigraph")` would re-introduce the opacity the migration is intended to eliminate. The surface/category prefix lets an auditor scan an audit log and know immediately "this is a banner-level classification rule" without cross-referencing a glossary.

**Why not four segments or arbitrary depth**: pre-emptive YAGNI. The five Stage-4 patterns (Pattern A/B/C/D plus the closure operator) all fit cleanly inside three segments. If a future pattern needs an extra tier, adding it is a coordinated `marque-2.1` schema bump with a one-time migration entry — same shape as this PR. Keep the wire shape disciplined.

### §1.4 Engine sentinels: drop the `r001.` / `r002.` prefix, encode in `<category>`

**Decision**: engine sentinels use `("engine", "<class>.<descriptive-predicate>")` with no `r001`/`r002` literal in the predicate.

| Surface | Sentinel | Predicate-ID |
|---|---|---|
| R001 — decoder recognition | from spec | `recognition.decoder-recognized` |
| R002 — re-parse failure | from spec | `fix.reparse-failed` |

**Why drop the numeric prefix**: the `R001` / `R002` literals are cruft from the pre-2-tuple regime. They exist because the flat-string form needed something to distinguish them from `E###` / `W###` / `C###` / `S###` ranges. In the 2-tuple shape, `scheme = "engine"` already carries that disambiguation; the numeric token in the predicate adds no information and burns a slot that should hold descriptive English. The spec example `("engine", "r001.decoder-recognized")` in `contracts/audit-record.md` line 161 is a *placeholder during the freeze window* — research.md R-3 line 133 lists the same literal as "RESERVED for engine-minted sentinel scheme" without claiming it is the final form. This PR is exactly the place to drop the placeholder.

**Counter-argument considered (and rejected)**: keeping `r001`/`r002` as an opaque cross-version identifier would let a future audit-log reader correlate engine-minted diagnostics across schema bumps. Rejected because (a) the `("engine", …)` scheme tuple is already the cross-version anchor; (b) the audit schema is a coordinated bump anyway — the consumer migrates the reader at the same point, and a descriptive predicate is more useful at that point than an opaque numeric.

The `tasks.md` T078 + the `R002_RULE_ID` data-model line 710 both still spell `"r002.reparse-failed"`. Those references update in lockstep with this PR (see §2).

### §1.5 Walker rules with `additional_emitted_ids`

The codebase has several walkers that emit on behalf of multiple retired rule IDs:

- `BannerMatchesProjectedRule` (registered `id() = "E031"`, additionally emits `E035`, `E040`, `E068`, `E069`)
- `DeclarativeClassFloorRule` (registered `id() = "E058"`, emitted via engine constraint-bridge for 27 `class-floor/*` catalog rows)
- `DeclarativeSciPerSystemRule` (registered `id() = "E059"`, emitted via engine constraint-bridge for 5 `sci-per-system/*` catalog rows)
- `DeclarativeNonCanonicalInputRule` (registered `id() = "E060"`, retired in PR 3b.F but re-mapping required if anything still surfaces it)
- `DeprecatedSciLongFormRule` (registered `id() = "E065"`)
- Plus the engine constraint-bridge for the 15 retired declarative-wrapper IDs (PR #578).

**Decision**: each catalog row becomes its own predicate ID. The walker's registered ID and the per-row emitted IDs are independent.

The bridge in `crates/engine/src/engine.rs:2406-2438` already maps `constraint_label` → `RuleId`. Today the mapping flattens row labels back to `E058` / `E059` / per-prefix `E###`. Post-migration, the bridge changes role: it stops translating and starts forwarding the catalog row's predicate ID directly. The catalog labels and the new rule-ID predicate IDs MUST become the same string — the bridge becomes a no-op pass-through.

**Concrete mapping** for the bridged rows:

| Walker `id()` (old) | Catalog row (old) | Post-migration predicate ID |
|---|---|---|
| `E058` | `class-floor/HCS-comp-sub` | `banner.classification.floor-hcs-comp-sub` |
| `E058` | `class-floor/SI-comp` | `banner.classification.floor-si-comp` |
| `E058` | `class-floor/TK-BLFH` | `banner.classification.floor-tk-blfh` |
| `E058` | `class-floor/BALK` | `banner.classification.floor-balk` |
| `E058` | `class-floor/BOHEMIA` | `banner.classification.floor-bohemia` |
| `E058` | `class-floor/HCS-comp` | `banner.classification.floor-hcs-comp` |
| `E058` | `class-floor/RSV-comp` | `banner.classification.floor-rsv-comp` |
| `E058` | `class-floor/TK` | `banner.classification.floor-tk` |
| `E058` | `class-floor/RD-SG` | `banner.classification.floor-rd-sg` |
| `E058` | `class-floor/FRD-SG` | `banner.classification.floor-frd-sg` |
| `E058` | `class-floor/RSEN` | `banner.classification.floor-rsen` |
| `E058` | `class-floor/IMCON` | `banner.classification.floor-imcon` |
| `E058` | `class-floor/SI` | `banner.classification.floor-si` |
| `E058` | `class-floor/RD` | `banner.classification.floor-rd` |
| `E058` | `class-floor/FRD` | `banner.classification.floor-frd` |
| `E058` | `class-floor/TFNI` | `banner.classification.floor-tfni` |
| `E058` | `class-floor/ATOMAL` | `banner.classification.floor-atomal` |
| (… 10 more `class-floor/*` rows) | … | `banner.classification.floor-<token>` |
| `E059` | `sci-per-system/HCS-O-companions` | `marking.sci.hcs-o-companions` |
| `E059` | `sci-per-system/HCS-P-NOFORN` | `marking.sci.hcs-p-noforn-required` |
| `E059` | `sci-per-system/HCS-P-sub-companions` | `marking.sci.hcs-p-sub-companions` |
| `E059` | `sci-per-system/SI-G-companions` | `marking.sci.si-g-companions` |
| `E059` | `sci-per-system/TK-compartment-NOFORN` | `marking.sci.tk-compartment-noforn-required` |
| `E053` (bridge route) | `capco/noforn-conflicts-rel-to` | `portion.dissem.noforn-conflicts-rel-to` |
| `E010` … `E057` (15 bridge routes) | various | per-row predicate per the bridge mapping in `crates/engine/src/engine.rs:248-268` |

The full table for the 15 bridged declarative wrappers and the 27+5 catalog rows lives in `docs/refactor-006/legacy-rule-id-map.md` (this PR creates it).

**Walker-vs-row identity, finalized**: when a walker fires, the diagnostic carries the per-row predicate ID. The walker itself no longer has a "registered" ID distinct from its rows — the registration pin in `crates/capco/tests/post_3b_registration_pin.rs` becomes a list of *all emitted predicate IDs*, not just walker-registered ones. This is what `additional_emitted_ids` was always trying to express; the 2-tuple form makes it the natural shape.

Diagnostic citations were already per-row (FR-018, typed `Citation` at the row level), so audit traceability is preserved by construction.

### §1.6 Suggest / Info / Warn diagnostics

`W034`, `W004`, `W003`, `S003`, `S004`, `S005`, `S007`, `S008`, `S009`, `S010` all become surface-prefixed:

| Old | New |
|---|---|
| `W003` (`PortionDissemMismatchRule`) | `portion.dissem.us-mismatch-with-banner-rollup` |
| `W004` (joint-disunity-collapse-to-FGI) | `page.fgi.joint-disunity-collapses-to-fgi` |
| `W034` (`SciCustomControlInfoRule`) | `portion.sci.unpublished-custom-control` |
| `S003` (deprecated-marking suggest) | `marking.deprecation.legacy-token-suggest` |
| `S004` (`RelToTrigraphSuggestRule`) | `portion.dissem.rel-to-trigraph-suggest` |
| `S005` (`analyze_uncertain_reduction`) | `page.dissem.rel-to-uncertain-reduction` |
| `S007` (`BareNatoRequiresRelToRule`) | `portion.nato.bare-classification-needs-rel-to-nato` |
| `S008` (RELIDO-implied-by-closure) | `portion.dissem.relido-implied-by-closure` |
| `S009` (tetragraph collapse) | `portion.dissem.tetragraph-collapse-suggest` |
| `S010` (collapse uniform rel portions) | `portion.dissem.collapse-uniform-rel-portions` |

The C-class corrections-map rule `C001` becomes `marking.correction.token-typo`.

### §1.7 Test-fixture rule IDs (`E997`, `E998`, `E999`, `S999`, `RECORD`, `PARSED_CACHE_TEST`, `R999`)

These exist purely in `#[cfg(test)]` and `tests/` files (~16 sites in `crates/engine/src/engine.rs` test modules, plus `tests/` integration). They are test scaffolding, never reach production.

**Decision**: migrate to a uniform synthetic test scheme `("test", "<descriptive>")`. Examples:

| Old | New |
|---|---|
| `E997` | `("test", "synthetic.e997-fixture")` |
| `E999` | `("test", "synthetic.e999-fixture")` |
| `R999` | `("test", "synthetic.r999-fixture")` |
| `RECORD` | `("test", "synthetic.record-fixture")` |
| `PARSED_CACHE_TEST` | `("test", "synthetic.parsed-cache-test")` |

The `"test"` scheme is reserved (alongside `"engine"`) — not a valid `MarkingScheme` registration target. A grep-fence in `crates/rules/src/lib.rs` doc-comment names the two reserved schemes explicitly so future scheme authors don't collide.

### §1.8 Closure-rule names — leave the `[closure_rules]` keyspace alone

The 10 `ClosureRule`s (`capco/noforn-if-caveated`, etc.) live in a section-isolated config keyspace per D19 B. Their `name` field is a separate identifier from `RuleId`. This PR does NOT migrate `ClosureRule.name` to the 2-tuple form — the closure layer keeps `&'static str` names since they (a) aren't audit-record rule-IDs (closure firings produce `AuditNote`, not `AppliedFix`, and `AuditNote.rule: RuleId` already takes a 2-tuple by virtue of this PR's type change — `AuditNote.structural.row_name: &'static str` is the closure-side identifier and stays unchanged), (b) are keyed against the `[closure_rules]` config section, not `[rules]`, so collision risk is zero.

Future amendment may unify the two — explicitly out of scope for this PR. (5-year maintainability note in §5.)

---

## §2 — Sequence of structural changes

Strict leaf-to-consumer ordering. Each step's blocker is the step above it; everything that can parallelize is called out in §4.

### Step 2.1 — `RuleId` type definition in `crates/rules/src/lib.rs:122-143`

Replace the 1-tuple with the 2-tuple:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleId {
    scheme: &'static str,
    predicate_id: &'static str,
}

impl RuleId {
    pub const fn new(scheme: &'static str, predicate_id: &'static str) -> Self {
        Self { scheme, predicate_id }
    }
    pub const fn scheme(&self) -> &'static str { self.scheme }
    pub const fn predicate_id(&self) -> &'static str { self.predicate_id }
}

impl std::fmt::Display for RuleId {
    // "<scheme>:<predicate_id>" canonical wire string for log lines,
    // .marque.toml config keys, and any consumer that wants a single
    // string (CLI text output, error messages, fixture keys).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.scheme, self.predicate_id)
    }
}
```

`Copy` is added (both fields are `&'static str`, so `Copy` is free and lets consumers stop cloning).

`as_str()` is **removed**. Every existing `.as_str()` call site moves to either `.predicate_id()` (when the caller wants the predicate alone — rare) or `.to_string()` / `format!("{rule}")` via the new `Display` impl (when the caller wants the wire-string form — common — for JSON serialization, config-key lookup, log output). The bulk-edit pass in step 2.2 handles all `.as_str()` sites.

**Canonical wire string format**: `"<scheme>:<predicate_id>"` with a colon separator. Example: `"capco:portion.dissem.noforn-conflicts-rel-to"`, `"engine:fix.reparse-failed"`. Colon was picked over slash because slash collides with the existing constraint-label convention and over dot because the predicate ID itself contains dots and would lose the scheme boundary.

The wire string is the form `.marque.toml` users type in their `[rules]` section. CLI text output renders it. The audit-record NDJSON uses the structured 2-tuple JSON shape (§2.4), not the wire string.

**Blocker**: nothing. Foundation step.

### Step 2.2 — `RuleId::new` call-site rewrites (~106 sites)

Bulk rewrite every `RuleId::new("<old>")` to `RuleId::new("<scheme>", "<predicate>")`. The legacy-rule-id-map (§2.10) is the lookup table.

Distribution from the callsite inventory:
- `crates/capco/src/rules.rs`: 34 sites
- `crates/engine/src/engine.rs`: 36 sites (≈18 production paths inside the constraint-bridge dispatch + R001/R002 sentinels + ~16 test-only fixture sites)
- `crates/capco/src/scheme/sci_per_system.rs`: 3 sites (the `RULE_E059` const + two adjacent)
- `crates/capco/src/rules_declarative.rs`: 2 sites (E065, E067)
- `crates/engine/tests/audit*.rs`, `rule_panic_isolation.rs`, `audit_note_sealing_carve_out.rs`: ~10 sites
- `crates/rules/src/audit.rs`, `crates/rules/tests/*`, `crates/wasm/tests/audit_v1_0_parity.rs`: ~15 sites

Plus the dynamic dispatcher at `crates/engine/src/engine.rs:2406-2438`: the bridge currently parses `constraint_label.split('/').next()` to recover a literal `E###` / `W###` ID. Post-migration, the bridge **drops the prefix parse entirely**. The catalog row's `name` field IS the predicate ID; the bridge constructs `RuleId::new("capco", row.name())` directly with no string manipulation. The catalog row labels become canonical predicate IDs in step 2.3.

**Blocker**: step 2.1 lands first.

### Step 2.3 — Catalog row `name` field renames in `marque-capco`

The `class-floor/*` rows (27), `sci-per-system/*` rows (5), `capco/*` rows (7 in `core_catalog.rs` + 10 in `closure_table.rs` + …), and the 15 retired-declarative-wrapper labels all rename to their predicate-ID form (§1.5 table). The catalog row's `name` field becomes the canonical predicate ID.

Affected files (per the inventory):
- `crates/capco/src/scheme/class_floor.rs` (27 rows)
- `crates/capco/src/scheme/sci_per_system.rs` (5 rows) and `crates/capco/src/scheme/constraints/sci_per_system_catalog.rs` (mirror)
- `crates/capco/src/scheme/constraints/core_catalog.rs` (7 rows)
- `crates/capco/src/scheme/closure.rs` + `closure_table.rs` (10 closure rows — `name` field, not `RuleId`; these may rename to `<surface>.<category>.<predicate>` for consistency with the audit-note surface but they remain in `[closure_rules]` config, NOT `[rules]`, so the migration is style-only)

The closure-rule rename is a **separate sub-decision** (see §3 open decision OD-1).

**Blocker**: step 2.1 (the `RuleId::new` signature change touches the catalog construction sites; step 2.3 lands the row renames atomically with step 2.2).

### Step 2.4 — Audit-record JSON serialization

Today `crates/engine/tests/audit_g13_canary.rs:168` (and the CLI at `marque/src/render.rs:343`, the WASM at `crates/wasm/src/lib.rs:337`, etc.) emits:

```json
"rule": "E054"
```

Post-migration, the JSON shape per `contracts/audit-record.md` §"Post-`marque-1.0` RuleId migration":

```json
"rule": {
  "scheme": "capco",
  "predicate_id": "portion.dissem.noforn-conflicts-rel-to"
}
```

**Decision**: object form, not flattened string. Rationale captured in §3 OD-2 (the open decision menu) — committed here because the object form is the spec'd shape and the audit-log consumer ergonomics argument lands on its side.

Sites that emit the JSON shape:
- `marque/src/render.rs:341-360` (CLI `DiagnosticJson`), `:810-820` (`AuditFixJson`), `:835-845` (`AuditTextCorrectionJson`), `:927-933` (`audit_line_rule_str`)
- `crates/wasm/src/lib.rs:282-340` and `:660-690`
- `crates/engine/tests/audit_g13_canary.rs:163-200` (inline projection)
- `crates/engine/tests/audit.rs`, `crates/wasm/tests/audit_v1_0_parity.rs` (test fixtures asserting JSON shape)

All sites currently take `&'a str` for the `rule` field via `.as_str()`. They migrate to a struct field:

```rust
#[derive(serde::Serialize)]
pub struct RuleIdJson<'a> {
    pub scheme: &'a str,
    pub predicate_id: &'a str,
}

// in DiagnosticJson:
pub rule: RuleIdJson<'a>,
```

`From<&RuleId>` for `RuleIdJson<'_>` keeps the conversion one-line.

The wire string form (`"capco:portion.dissem...."`) is reserved for **text contexts** (CLI text output for humans, log lines, config-key lookup), never JSON. JSON consumers parse the structured shape.

**`Diagnostic` field name stays `rule: RuleId`** — the type carries the structure. No field split into `scheme + predicate` (cf. §3 OD-3 — closed here in the recommendation; rationale: keeping a single typed field preserves the `HashMap<RuleId, _>` ergonomics, severity-override lookup discipline, and avoids the bookkeeping burden of two parallel fields).

**Blocker**: step 2.1.

### Step 2.5 — `MARQUE_AUDIT_SCHEMA` bump: `marque-1.0` → `marque-2.0`

`crates/engine/build.rs:24-37`:

```rust
const ACCEPTED: &[&str] = &["marque-2.0"];
const DEFAULT: &str = "marque-2.0";
```

Single accepted value. Pre-cutover `marque-1.0` records are unreadable by post-cutover binaries (FR-037 clean-break invariant).

`crates/engine/src/lib.rs:90,102`:
```rust
pub const AUDIT_SCHEMA_VERSION: &str = env!("MARQUE_AUDIT_SCHEMA");
pub const AUDIT_SCHEMA_IS_V2_0: bool = const_str_eq(AUDIT_SCHEMA_VERSION, "marque-2.0");
```

Rename `AUDIT_SCHEMA_IS_V1_0` → `AUDIT_SCHEMA_IS_V2_0`. Any consumer asserting `marque-1.0` updates in lockstep.

Search target — every literal `"marque-1.0"` in the codebase:
- `crates/engine/src/lib.rs` (the const)
- `crates/engine/build.rs` (the accept list + default)
- `crates/engine/tests/audit_g13_canary.rs:462,591` (NDJSON fixture strings)
- `crates/rules/src/audit.rs` doc-comments (~5 references)
- `contracts/audit-record.md` text — see §2.11 below

**Blocker**: steps 2.1, 2.4 (the schema bump is atomic with the JSON shape change).

### Step 2.6 — Engine sentinels: `R002_RULE_ID`, `DECODER_RULE_ID`

`crates/engine/src/engine.rs:113,147`:

```rust
// Was: const DECODER_RULE_ID: &str = "R001";
const DECODER_RULE_ID: RuleId = RuleId::new("engine", "recognition.decoder-recognized");

// Was: pub const R002_RULE_ID: RuleId = RuleId::new("R002");
pub const R002_RULE_ID: RuleId = RuleId::new("engine", "fix.reparse-failed");
```

`DECODER_RULE_ID` becomes a real `RuleId` (no more `&'static str` exception). The downstream `RuleId::new(DECODER_RULE_ID)` call at `engine.rs:4341` collapses to a direct `DECODER_RULE_ID` clone (or `Copy`, post-step-2.1).

The `RULE_E059` const at `crates/capco/src/scheme/sci_per_system.rs:93` deletes outright — once the catalog row rename gives each row its own predicate ID, there's no longer a shared "this walker fires E059" constant. Each sci-per-system row constructs its own `RuleId` from its `name` field through the bridge.

**Blocker**: step 2.1.

### Step 2.7 — Registration pins

`crates/capco/tests/post_3b_registration_pin.rs:116-176`: `EXPECTED_RULE_IDS` becomes a list of structured tuples (or kept as strings using the wire-format canonical form — `"capco:portion.dissem.noforn-conflicts-rel-to"` — for simpler test ergonomics). The current shape is `&[&str]` of `"E058"`-style strings.

**Recommendation**: keep the test surface as `&[&str]` of canonical wire strings:

```rust
const EXPECTED_RULE_IDS: &[&str] = &[
    "capco:marking.correction.token-typo",
    "capco:banner.classification.usa-trigraph",
    // ... (28 entries post-#251)
];

// Comparison: actual.iter().map(|r| r.to_string()).collect::<BTreeSet<_>>()
```

vs. constructing real `RuleId` tuples in the test. The string form is what `.marque.toml` users see, what audit logs render in text mode, and what grep-friendly. The structured form adds verbosity for no equivalent payoff. Pick wire strings.

`crates/capco/tests/corpus_parity.rs` count-pin already counts cardinality; the count assertion is unaffected (cardinality of registered rules doesn't change).

**Blocker**: steps 2.1, 2.2, 2.3.

### Step 2.8 — CLI + WASM output

CLI at `marque/src/render.rs:299,343,472,574,815,839,929,1013`:
- `DiagnosticJson.rule: &'a str` → `DiagnosticJson.rule: RuleIdJson<'a>`
- `AuditFixJson.rule` and `AuditTextCorrectionJson.rule` get the same field-type change
- Plain-text rendering (the human-readable `marque check` output) uses `format!("{}", diagnostic.rule)` via the new `Display`

WASM at `crates/wasm/src/lib.rs:286,337,388,455,666,688`: same shape change. WASM's JSON output is byte-identical to CLI's audit-record JSON (parity test at `crates/wasm/tests/audit_v1_0_parity.rs` enforces this).

The parity test's hand-constructed expected JSON updates to the structured object form. Test fixtures at `crates/wasm/tests/audit_v1_0_parity.rs:90-120,350-360` use `RuleId::new(rule)` — those calls update with the new 2-arg signature.

**Blocker**: steps 2.1, 2.4. CLI + WASM can land in parallel (no shared file).

### Step 2.9 — G13 audit canary

`crates/engine/tests/audit_g13_canary.rs:168,202`: `"rule": f.rule.as_str()` → `"rule": rule_id_json(&f.rule)` (or inline the struct literal).

The hand-crafted NDJSON regex literals at `:462,591` (`r#"{{"type":"text_correction","schema":"marque-1.0","rule":"R999",`) update:
- `"schema":"marque-1.0"` → `"schema":"marque-2.0"`
- `"rule":"R999"` → `"rule":{"scheme":"test","predicate_id":"synthetic.r999-fixture"}`

**Blocker**: steps 2.1, 2.4, 2.5.

### Step 2.10 — Corpus expected.json fixtures (67 files)

Every `tests/corpus/**/*.expected.json` carries `"rule": "E007"`-style flat-string entries. The shape change:

```json
{ "rule": "E007", "span": {"start": 8, "end": 13} }
```

becomes either:

```json
{ "rule": {"scheme": "capco", "predicate_id": "portion.metadata.x-shorthand-date-pattern"}, "span": {"start": 8, "end": 13} }
```

or — if we choose the field-split form OD-3 — `{"scheme": "capco", "predicate": "...", "span": {...}}`.

`crates/test-utils/src/lib.rs:75-81` (`ExpectedDiagnostic`):

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedDiagnostic {
    pub rule: ExpectedRuleId,           // was: pub rule: String,
    pub span: ExpectedSpan,
    #[serde(default)]
    pub severity: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedRuleId {
    pub scheme: String,
    pub predicate_id: String,
}
```

A `serde` `untagged` deserializer COULD accept both the legacy string form and the structured form, but that re-introduces transitional state. **Atomic cutover**: the 67 fixtures rewrite in one PR step.

A one-shot `tools/migrate-corpus-rule-ids.py` script (out-of-workspace per Constitution III) handles the mechanical rewrite using the legacy-rule-id-map as the lookup table. Script + map land in step 2.11.

**Blocker**: step 2.1; can parallelize with step 2.8.

### Step 2.11 — `docs/refactor-006/legacy-rule-id-map.md`

This PR creates the file. Layout:

```markdown
# Legacy Rule-ID Map — T044 cutover (2026-05-22)

This map records the one-time rename of every flat-string rule ID
`E### / W### / C### / S### / R### / catalog-row-label / test-fixture-id`
to its 2-tuple successor `(scheme, predicate_id)`.

Post-cutover audit logs use the 2-tuple form exclusively. No runtime
translation table exists (clean break per Constitution V; FR-037).

The map exists for archaeological purposes only:
- Historical commit messages reference `E054`, `W003`, etc.
- Prior-art docs in `docs/refactor-006/` and `docs/plans/` reference
  the same.
- CAPCO §-citation cross-references in `crates/capco/CAPCO-CONTEXT.md`
  may reference the legacy IDs.

| Legacy ID | New `(scheme, predicate_id)` | Citation | Notes |
|---|---|---|---|
| `E001` | `("capco", "banner.classification.portion-mark-in-banner")` | §A.6 p17 | (retired pre-T044 — bridge-routed) |
| `E002` | `("capco", "banner.classification.usa-trigraph-missing")` | §H.7 p124 | active rule |
| ... | ... | ... | ... |
```

This is a *living* document, not one-and-archived (see §5 risk note). When a post-T044 rename happens, the map gets an extra column or an addendum table — never a silent edit of an existing row.

**Free-text description per row**: include a one-line plain-English summary so an archaeologist reading a 2027 commit message that says "fixed E054" can find the new predicate ID without context-switching to the CAPCO manual.

**Blocker**: nothing structural; needs the rename-table from steps 2.2 + 2.3 to be drafted before it's filled in. Practical authoring sequence: agent doing step 2.3 produces the map as a side effect of renaming the catalog rows.

### Step 2.12 — Contract + spec doc updates

After the structural changes land:

- `specs/006-engine-rule-refactor/contracts/audit-record.md`: the "Rule ID encoding" section becomes the structured 2-tuple form; the "Post-`marque-1.0` RuleId migration" section becomes historical (renamed "T044 cutover history" and back-dated). Lines 108-175 of the file are the affected block.
- `specs/006-engine-rule-refactor/data-model.md:330`: the placeholder `pub struct RuleId(pub &'static str /* predicate_id */);` updates to the real 2-tuple shape. Line 710's `R002_RULE_ID` example updates to drop the `r002.` prefix per §1.4.
- `specs/006-engine-rule-refactor/spec.md` FR-026, FR-044, FR-049, FR-035: tense flip from "MUST migrate" to "DONE — see legacy-rule-id-map.md". FR-049 wording about "stable within a major audit-schema version" becomes the active commitment.
- `CLAUDE.md` "Post-006 Stable Surface" section: the "Not frozen — Rule-ID 2-tuple form" bullet moves to "Frozen as of T044". `MARQUE_AUDIT_SCHEMA = "marque-2.0"` replaces `"marque-1.0"` references.
- `tasks.md`: T044 flips to `[x]`; T078 retroactively updates its `R002_RULE_ID` example.

**Blocker**: all structural steps. Doc-only commits at the end of the PR.

---

## §3 — Decision points needing PM resolution

The plan above is committed wherever spec / research / contracts already pre-decided. The remaining open decisions are listed with options + recommendations.

### OD-1 — Closure-rule names: rename for consistency or leave alone?

The 10 `ClosureRule.name` strings (`capco/noforn-if-caveated`, etc.) are technically separate from `RuleId` — they key the `[closure_rules]` config section, not `[rules]`. But they appear in audit-stream `AuditNote.structural.row_name` and in user-facing config.

**Options**:
- **A**. Leave `ClosureRule.name` unchanged (slash-separated `capco/<predicate>`). `AuditNote.rule: RuleId` carries the structured 2-tuple separately. Two naming conventions coexist.
- **B**. Rename `ClosureRule.name` to a parallel structured form: a `&'static str` containing the wire string `"capco:closure.<category>.<predicate>"`. Single convention.
- **C**. Reshape `ClosureRule.name: &'static str` into `ClosureRule.id: RuleId` (the 2-tuple). Forces the `[closure_rules]` config section to use the structured key form too.

**Recommend**: **B**. The wire-string form preserves the section-isolation property (`[closure_rules]` keyspace stays separate from `[rules]` because the section header is the disambiguator, not the key shape), gives users one mental model ("rule IDs look like X across both sections"), and avoids inflating the `ClosureRule` type with another struct field. C goes further than necessary; A leaves a permanent inconsistency users will trip on.

### OD-2 — JSON shape: structured object vs. wire string

Already committed in §2.4 to the structured object form per the spec's pre-drafted shape. PM may want to confirm.

**Options**:
- **A**. Structured object: `"rule": {"scheme": "capco", "predicate_id": "..."}` per `contracts/audit-record.md`.
- **B**. Wire string: `"rule": "capco:portion.dissem.noforn-conflicts-rel-to"`. Smaller audit log; simpler parsing.

**Recommend**: **A**. The contracts file already documents A. JSON consumers that need to filter by scheme (e.g., a compliance auditor querying "all CAPCO rule firings" or "all engine-minted sentinels") parse `.rule.scheme` as a field rather than splitting a string on `:`. Audit-log size delta is ~10-15 bytes per record, immaterial against typical NDJSON record sizes (1-3 KB).

### OD-3 — `Diagnostic` field name: stays `rule: RuleId` or split into `scheme + predicate`?

**Options**:
- **A**. `pub rule: RuleId` (the 2-tuple type carries the structure). Single field on `Diagnostic` and `AppliedFix`.
- **B**. `pub scheme: &'static str` + `pub predicate_id: &'static str` (two parallel fields, no `RuleId` type).

**Recommend**: **A**. The type system carries the invariant — a `RuleId` is always the (scheme, predicate) pair, never one without the other. Splitting into two fields lets a future bug construct a `Diagnostic` with one filled and the other empty. Plus `HashMap<RuleId, Severity>` for the severity-override lookup wants a single key type.

### OD-4 — Engine sentinel naming: drop `r001` / `r002` numeric or keep?

Already committed in §1.4 to **drop**. PM should confirm.

**Options**:
- **A**. Drop: `("engine", "recognition.decoder-recognized")`, `("engine", "fix.reparse-failed")`.
- **B**. Keep: `("engine", "r001.decoder-recognized")`, `("engine", "r002.reparse-failed")`.

**Recommend**: **A**. The `r001`/`r002` literals were placeholders during the freeze window. The `scheme = "engine"` tuple already carries the cross-version anchor. Descriptive `<class>.<predicate>` reads better at audit-log triage. Counter-case (cross-version identifier) is addressed by the (scheme, predicate) tuple itself — see §1.4.

### OD-5 — Legacy ID strings: survive anywhere?

**Options**:
- **A**. Legacy strings (`"E054"`) survive only in `docs/refactor-006/legacy-rule-id-map.md` as searchable keys and in CHANGELOG / commit-message historical references. They disappear from all source code, tests, fixtures, diagnostic messages, citation comments, etc.
- **B**. Plus: keep legacy IDs in diagnostic message bodies (`"E054: NOFORN conflicts with REL TO ..."`) for human-readable continuity.
- **C**. Plus: keep legacy IDs as a deprecated config-key alias (`[rules] E054 = "off"` keeps working through a translation layer).

**Recommend**: **A**. Pre-users; no users have `[rules] E054 = "off"` in their `.marque.toml` files that would break. Messages already moved to `MessageTemplate` (FR-003), and templates carry no rule IDs in their body text. The only place legacy IDs survive is the map doc + git history (which is immutable). C re-introduces the alias-map fragility the constitution explicitly bans for pre-users projects.

### OD-6 — `RuleId::new` constructor shape: replace or supplement?

Already committed: replace.

**Options**:
- **A**. Replace `RuleId::new(&'static str)` with `RuleId::new(&'static str, &'static str)`. Every existing call site updates.
- **B**. Supplement: add a new `RuleId::predicate(scheme, predicate)` and deprecate `RuleId::new(s)`.

**Recommend**: **A**. Pre-users per `feedback_pre_users_no_deprecation_phasing.md`. Two constructors invites a future call site to use the wrong one; one constructor with the right signature makes the misuse unrepresentable.

### OD-7 — Severity-override config keyspace: wire string or structured table?

`crates/config/src/lib.rs:236-238` defines `RuleConfig.overrides: HashMap<String, String>`. The keys are rule ID strings from `[rules]` in `.marque.toml`.

**Options**:
- **A**. Users continue writing string keys: `[rules] "capco:portion.dissem.noforn-conflicts-rel-to" = "off"`. The HashMap key is the wire-string form.
- **B**. Restructure the TOML to a nested table: `[rules.capco] "portion.dissem.noforn-conflicts-rel-to" = "off"`. The HashMap becomes `HashMap<(String, String), String>`.

**Recommend**: **A**. TOML key strings work; the wire-string form is what `Display` produces. Users typing the structured form (`[rules.capco]`) is plausible but the migration cost on user-side configs is higher and `marque-2.0` should be a structural cleanup, not a config-format break. The nested form is a follow-up if user feedback demands it.

### OD-8 — Bridge dispatcher rewrite at `crates/engine/src/engine.rs:2406-2438`

The current bridge parses `constraint_label.split('/').next()` to recover the legacy ID prefix.

**Options**:
- **A**. Bridge becomes a no-op pass-through. The catalog row's `name` is already the predicate ID; bridge constructs `RuleId::new("capco", row.name())` directly. The 15 special-case literals (`"capco/noforn-conflicts-rel-to"` → `RuleId::new("E053")`, etc.) at engine.rs:431-437 disappear because the row labels themselves carry the post-migration predicate ID.
- **B**. Bridge keeps a translation table mapping `<catalog-label> → <predicate-id>`. Allows catalog row labels to drift from predicate IDs.

**Recommend**: **A**. Decouples the bridge from naming policy. The catalog rows ARE the predicate IDs; eliminating the translation table eliminates a class of "label says one thing, rule ID says another" drift bugs the project has already hit (see CLAUDE.md "PR 3b umbrella closeout" entry about bridge-renaming gaps).

This decision cascades back into step 2.3 — the catalog row renames are now the same operation as the predicate-ID assignment.

---

## §4 — Implementation parallelization plan

Five agents in three waves. Total cycle time target: 1.5-2 days of agent work, plus PM review windows.

### Wave 1 — Foundation (sequential, single agent — call it Agent F)

**Agent F deliverables (in order)**:
1. `crates/rules/src/lib.rs` — reshape `RuleId` type (step 2.1). Add `Display` impl, drop `as_str`, add `scheme()` / `predicate_id()`. Wire-string format pinned.
2. `crates/engine/build.rs` + `crates/engine/src/lib.rs` — `MARQUE_AUDIT_SCHEMA` bump to `marque-2.0` (step 2.5).
3. `crates/engine/src/engine.rs` — `R002_RULE_ID` + `DECODER_RULE_ID` migration (step 2.6).
4. `crates/test-utils/src/lib.rs` — `ExpectedDiagnostic.rule: ExpectedRuleId` shape change (step 2.10's type side).
5. Verify `cargo check --workspace` fails predictably (every downstream call site is broken; that's the point — the type system is now the work list).

**Why sequential**: every Wave-2 agent depends on the type compiling and the schema constant being set. Wave-1 takes ~half a day of agent work; the failure surface is well-defined.

**Wave 1 ends when**: `cargo check --workspace` produces a *known* failure list (the ~106 sites). The list is the input to Wave 2.

### Wave 2 — Per-crate migrations (parallel, 4 agents)

Four parallel agents, one per affected crate cluster. None of them touch the same file as another agent.

**Agent A — `marque-capco`**:
- `crates/capco/src/rules.rs` (34 `RuleId::new` sites)
- `crates/capco/src/rules_declarative.rs` (2 sites)
- `crates/capco/src/scheme/sci_per_system.rs` (3 sites + `RULE_E059` deletion)
- `crates/capco/src/scheme/class_floor.rs` (27 catalog row renames per step 2.3)
- `crates/capco/src/scheme/constraints/core_catalog.rs` (7 catalog row renames)
- `crates/capco/src/scheme/constraints/sci_per_system_catalog.rs` (5 catalog row renames)
- `crates/capco/src/scheme/closure.rs` + `closure_table.rs` (10 closure-rule names — per OD-1.B, rename to wire-string form)
- Also produces the legacy-rule-id-map row entries for every rename touched (step 2.11 source data)

**Agent B — `marque-engine` constraint-bridge + sentinels + tests**:
- `crates/engine/src/engine.rs` — bridge rewrite per OD-8.A (step 2.2 sequel; lines 2406-2438 simplification + the doc-comment table at 248-268 updates)
- `crates/engine/src/output.rs` (uses `RuleId`)
- `crates/engine/tests/audit.rs`, `audit_g13_canary.rs`, `audit_note_sealing_carve_out.rs`, `rule_panic_isolation.rs` — all `RuleId::new` test-fixture sites + the NDJSON-literal strings (step 2.9)

**Agent C — `marque-rules` + `marque/` CLI + WASM**:
- `crates/rules/src/audit.rs` doc-comments + `RuleId::new` sites
- `crates/rules/tests/*` — `applied_text_correction_seal.rs`, `engine_promotion_seal.rs`, `message_args_closed_set.rs`
- `marque/src/render.rs` — `DiagnosticJson.rule`, `AuditFixJson.rule`, `AuditTextCorrectionJson.rule` field-type changes + JSON struct-literal sites (step 2.8)
- `crates/wasm/src/lib.rs` — same JSON shape changes
- `crates/wasm/tests/audit_v1_0_parity.rs` — expected JSON updates + `RuleId::new` sites

**Agent D — corpus fixtures + registration pin + legacy-rule-id-map**:
- 67 `tests/corpus/**/*.expected.json` files rewrite (step 2.10). Use the lookup table from Agent A's side output. The mechanical rewrite uses an out-of-workspace Python script committed under `tools/` per Constitution III.
- `crates/capco/tests/post_3b_registration_pin.rs` — `EXPECTED_RULE_IDS` updates to wire-string form (step 2.7)
- `crates/capco/tests/corpus_parity.rs` — count assertion sanity check (count unchanged at 28; no semantic change but the cardinality test runs against the new ID shape)
- `docs/refactor-006/legacy-rule-id-map.md` — full table assembly from Agent A's per-rename entries + cross-checks against Agent B's sentinel renames + Agent C's test-fixture renames

**Sequencing inside Wave 2**: Agent A is the source of truth for the rename table. Agent D depends on Agent A finishing its catalog-row + rule renames (the lookup-table side output). Agents B + C can start as soon as Wave 1 lands — they don't need Agent A's row labels (they read the legacy-rule-id-map from Agent D's draft, which lands before B + C complete).

Practical schedule:
1. Wave 1 completes → Agent A begins.
2. Agent A finishes drafting the rename table (~half-day) → Agents B, C, D begin in parallel.
3. All four converge → Wave 3.

**Merge-conflict surface**: each agent owns its files. The only shared touch point is `docs/refactor-006/legacy-rule-id-map.md` (Agent D owns; A/B/C contribute via side notes). Constitution III enforces "no shared file in the same PR commit" so we make Agent D the sole `git add` for the map.

### Wave 3 — Doc cleanup + contract sync (single agent — Agent G)

**Agent G**:
- `specs/006-engine-rule-refactor/contracts/audit-record.md` — flip the "Rule ID encoding" section (step 2.12).
- `specs/006-engine-rule-refactor/data-model.md` — RuleId placeholder + R002 example.
- `specs/006-engine-rule-refactor/spec.md` — FR-026, FR-044, FR-049 tense flips.
- `specs/006-engine-rule-refactor/tasks.md` — T044 flips to `[x]`; T078 retroactively updates.
- `CLAUDE.md` "Post-006 Stable Surface" + Recent Changes archive entry.
- Final CI sweep: `cargo test --workspace`, corpus regression, audit canary G13, WASM parity, registration pins.

**Wave 3 takes ~half a day** of focused review-bandwidth-heavy agent work. PM-review-heavy.

---

## §5 — Risks and 5-year maintainability

### Risk R-1: predicate-ID convention doesn't scale to other schemes

**Test**: when `marque-cui` or `marque-nato` lands, does the `<surface>.<category>.<predicate>` form fit?

- CUI markings have a comparable surface taxonomy (banner, portion, CUI category). Surfaces `banner | portion | page | marking` map cleanly. CUI categories (FOUO-derived, PROPIN, controlled-defense-information, etc.) map cleanly.
- NATO follows the same banner/portion structure with category overlaps for ATOMAL, BOHEMIA, BALK already discussed in project memory.
- JOINT is a CAPCO category; doesn't need its own scheme.

**Verdict**: the form scales. The `<surface>` segment may need a new value (`distribution` for distribution-statement markings; `caveat` for certain caveat-only marking systems) but adding a value is non-breaking — it doesn't change the shape.

**5-year retest**: when a second scheme actually lands, re-verify by walking the second scheme's distinctive grammar against the convention. If the convention strains, the strain is the signal to amend — not a reason to over-engineer the convention now.

### Risk R-2: JSON shape evolution for future metadata

If a future scheme needs to attach metadata to a rule ID (e.g., a CUI category designator number, a NATO security framework version):

- **Object form (committed)** absorbs additions cleanly: `{"scheme": "cui", "predicate_id": "...", "registry_url": "..."}`.
- **Wire-string form** would have collapsed metadata into the predicate ID itself, eventually requiring a sub-format.

**Verdict**: object form is the right call for 5-year forward compat.

### Risk R-3: schema-bump leaves cleanup debris

Things that could drift if not closed in this PR:
- `AUDIT_SCHEMA_IS_V1_0` const: renamed in step 2.5; ensures no stragglers.
- `marque-1.0` string literals in tests / fixtures / docs: bulk grep + replace; Wave 3 verifies.
- `R001` / `R002` literals in fixture files: covered by Agent D.

Constitution III's no-shared-file-per-PR rule discourages dual-writes that would leave debris.

### Risk R-4: legacy-rule-id-map.md as living document

**Decision**: the map IS living. Schema rationale: post-T044 renames will happen (CAPCO grammar evolves, new rules land, predicates clarify). Each rename gets a new row OR an addendum table at the bottom. Never a silent edit of an existing row.

The header doc-comment of the map fixes the discipline:

```markdown
# Legacy Rule-ID Map

## Discipline

This document is appended to, never rewritten. A rename added on date X
does NOT erase the prior name from this document — both rows survive,
with a `superseded_by` column linking forward.

The audit trail across renames lives here. Audit logs reference rule
IDs; this map is what lets a 2030 reader of a 2026 audit log find the
2030 name of the rule.
```

### Risk R-5: bridge no-op-ification regression surface

The bridge at `engine.rs:2406-2438` is hot-path. Removing the prefix-parse logic is a simplification but the corpus regression must verify no diagnostic emission changes (the diagnostic stream byte shape should be byte-identical to pre-migration except for the rule field shape).

**Mitigation**: Agent B's deliverable includes running the full corpus parity sweep with `--features audit-byte-equivalence` (or the equivalent at PR time) to confirm byte-identity modulo the rule field reshape. The G13 canary at `crates/engine/tests/audit_g13_canary.rs` covers the content-ignorance property; this PR adds an additional canary specifically for the rule-shape regression.

### Risk R-6: 5-year audit-log readability

A 2031 auditor reading a 2026 audit log encounters:

```json
{"rule": {"scheme": "capco", "predicate_id": "portion.dissem.noforn-conflicts-rel-to"}, ...}
```

Can they understand what fired? **Yes**, more readable than the legacy `"rule": "E054"` form. The predicate ID is human-language; the scheme is the namespace. The only barrier is mapping to the CAPCO §-citation, which is already on the audit record's `citation` field.

5-year verdict: predicate IDs age well precisely because they encode meaning, not version-pinned numeric identifiers.

### Risk R-7: post-merge stability freeze (FR-049 v2)

After T044 merges, the new `(scheme, predicate_id)` form becomes the stable interface. Any subsequent rename requires a coordinated `marque-2.1` (or `marque-3.0` if breaking) audit-schema bump per FR-049, semver-style.

**This PR's merge commit** is the freeze inflection. The `CLAUDE.md` "Post-006 Stable Surface" section moves the rule-ID 2-tuple from "Not frozen" to "Frozen as of T044."

A formal version-bump cadence — `marque-2.1` for additive predicate IDs, `marque-3.0` for breaking renames — is the suggested follow-up policy item but is out of scope here. The PR closes with `marque-2.0`; the version-bump cadence is a separate decision the PM signs off on (or doesn't) in a follow-up.

---

## §6 — Acceptance checklist

The PR may merge when:

- [ ] `cargo test --workspace` passes including the new structured-ID fixtures
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] G13 audit canary at `crates/engine/tests/audit_g13_canary.rs` passes against the corpus sweep
- [ ] WASM parity at `crates/wasm/tests/audit_v1_0_parity.rs` passes (CLI + WASM emit byte-identical NDJSON modulo the structured rule field)
- [ ] `crates/capco/tests/post_3b_registration_pin.rs` passes with the wire-string `EXPECTED_RULE_IDS` (cardinality unchanged at 28)
- [ ] All 67 `tests/corpus/**/*.expected.json` files use the structured JSON shape and `marque-capco` corpus parity passes
- [ ] `docs/refactor-006/legacy-rule-id-map.md` is complete: every retired flat-string ID has a row with successor + citation + free-text description
- [ ] `specs/006-engine-rule-refactor/contracts/audit-record.md` "Rule ID encoding" section reflects the live shape
- [ ] `specs/006-engine-rule-refactor/data-model.md` lines 330 + 710 reflect the live shape
- [ ] `CLAUDE.md` "Post-006 Stable Surface" section updated; Recent Changes archive entry added
- [ ] `tasks.md` T044 flipped to `[x]`; T078 retroactively updated
- [ ] PM has signed off on the open decisions OD-1..OD-8 (or the recommendations stand by default)
- [ ] Constitution Principle V (audit-record integrity), Principle VII (crate graph) verified — no engine-crate edits crossed the scheme-adoption boundary; the migration is type-and-shape only

---

## Appendix A — Why no transitional shape

Pre-users; no committed `.marque.toml` files contain legacy rule-ID keys. No audit log consumers exist that would need to parse both `marque-1.0` and `marque-2.0` records. The `cargo test --workspace` failure list IS the work-tracking artifact; no incremental migration needed.

Per project memory `feedback_pre_users_no_deprecation_phasing.md`: "marque is pre-users — no deprecation phasing." Rewrite freely. Apply.

## Appendix B — Constitutional alignment

- **Principle V** (audit integrity): `AppliedFix::__engine_promote` is unchanged; the field-shape change is upstream of promotion. G13 canary updated to scan structured JSON.
- **Principle VII** (crate graph): `marque-rules` carries `RuleId`; the type change does not cross crate boundaries. `marque-engine` consumes the type as before. Constitution VII §IV scheme-adoption restriction does not apply here — this is not a scheme adoption.
- **Principle IV** (rule IDs): the convention `E### / W### / S### / C###` is explicitly replaced by `<scheme>:<surface>.<category>.<predicate>`. The constitution amendment in §2.12 reflects this; reviewers must verify the wording matches the predicate-ID convention pinned here.
- **Principle VIII** (citation fidelity): per-row citations are unchanged — the migration is structural only. Each rename row in the legacy-rule-id-map carries the citation that was on the rule pre-migration; reviewers verify against `crates/capco/docs/CAPCO-2016.md` at this PR's authorship.

---

*End of plan. Implementation runs from §4 after PM closure on §3.*
