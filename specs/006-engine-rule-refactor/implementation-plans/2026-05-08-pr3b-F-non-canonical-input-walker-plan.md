<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Implementation Plan: PR 3b.F — Non-Canonical Input Walker (T026f)

**Target file**: `docs/plans/2026-05-08-pr3b-F-non-canonical-input-walker-plan.md`
**Branch**: `refactor-006-pr-3b-non-canonical-input` (worktree `/home/knitli/marque-pr3b-F/`, off `origin/staging` at `9d72a7a1` — PR 3b.E just merged)
**Base PR**: against `staging` (NOT `main`)
**Predecessors landed in staging**: PR 3b.A (#319 banner walker), PR 3b.B (#320 transmutations), PR 3b.C (#321 RELIDO conflicts), PR 3b.D (#324 class-floor catalog), PR 3b.E (#326 SCI per-system walker)
**Status**: PM-APPROVED 2026-05-08 (PM signature: marque PM coordinator session `8b9d47e0`). All three resolution-required open questions resolved per the recommendations in §9; see §11 for the resolution record. Plan §2.3 / §4.1 default-severity references corrected to `Error` to align with the OQ-3 resolution.

---

## 0. One-paragraph summary

PR 3b.F is the sixth and final functional sub-move of PR 3b. It retires the four hand-written ordering-validation rules — `CountryCodeOrderingRule` (E020, REL TO + JOINT alpha at `crates/capco/src/rules.rs:3065-3187`), `SigmaValidationRule` (E023, AEA SIGMA numeric sort + valid-set check at `4151-4246`), `SarProgramOrderRule` (E028, SAR program ascending alpha at `4379-4447`), and `SciCompartmentOrderRule` (E033, SCI compartment + sub-compartment alpha at `4989-5161`) — into a single hand-written walker `DeclarativeNonCanonicalInputRule` (rule ID `E060`, the next free `E###` slot after PR 3b.E took `E059`; verified by inventory grep at the time of plan authorship — all of E001–E059 are assigned, E060 is unused). The walker is **structurally different** from 3b.D / 3b.E: it is **not** a `Constraint::Custom` catalog on `CapcoScheme`. Per `2026-05-07-pr3b-consultation-verdict.md` Q-Move-7-timing default and `marque-applied.md` §3.6 + §3.10 Move 7, these checks are *renderer-canonical-form* concerns absorbed by `MarkingScheme::render_canonical` once the renderer trait surface lands in PR 5+ (Stage 4); the simpler the per-row data structure now, the easier the Stage-4 retirement will be. The walker therefore stores its rows in a private `&'static [NonCanonicalRow]` table inside the same Rust file as the walker (no public scheme surface, no `evaluate_custom_by_attrs` dispatch). Net rule delta: **4 retired (E020, E023, E028, E033) + 1 walker added (E060) = net −3; running registered-rule count 50 → 47.** Per-row identification flows via per-row `Diagnostic.rule = E060` plus a stable `kind` discriminator embedded in each diagnostic's catalog-row name field (`"non-canonical/rel-to-usa-first"`, etc.) and in the diagnostic message text. The walker preserves byte-for-byte the existing diagnostic message text, citation strings, severities (heterogeneous: `Fix` for E020/E023/E028, `Error` for E033), and `FixProposal` shapes (span + replacement + confidence + source) of the four retired rules so corpus-fixture churn is minimized — the only test churn is rule-ID rename (`E020`/`E023`/`E028`/`E033` → `E060`).

---

## 1. Architectural option (LOCKED — hand-written walker + private static catalog)

**Approved by PM:** 2026-05-08 (marque PM coordinator session `8b9d47e0`). See §11 for the resolution record on OQ-1 / OQ-2 / OQ-3.

Land PR 3b.F as ONE hand-written `impl Rule for DeclarativeNonCanonicalInputRule` block backed by a private `&'static [NonCanonicalRow]` catalog table inside `crates/capco/src/rules_declarative.rs` (or a new sibling file `rules_non_canonical.rs` — see §4.1 for the file-placement decision and rationale). The walker's `check` body is: (a) one axis-presence early-out gating into the catalog walk, (b) one `for row in NON_CANONICAL_CATALOG` loop, (c) one `match row.kind` dispatch inside the per-row `evaluate` fn-pointer call. ≤3 internal branches per D13.

### 1.1 Why this is structurally different from 3b.D / 3b.E

3b.D and 3b.E declared their rows as `Constraint::Custom { name, label }` on `CapcoScheme` because their invariants (class-floor partial-order thresholds; SCI per-system companion-required + forbid-companion) are *cross-axis predicates* over canonical attributes — natural fits for a scheme-level constraint surface that PR 4's per-category Lattice impls can absorb. PR 3b.F's invariants are not predicates over canonical attributes; they are *non-canonical input detection* — the invariant fires when the input's surface form (token order in the source bytes) differs from the canonical representative, not when the canonical attributes violate an algebraic law. The MarkingScheme::project canonical form already sorts these axes correctly; a renderer that encodes the same sort produces a string that won't trip the walker. Once PR 5+ wires `MarkingScheme::render_canonical`, the diagnostic becomes "your input does not match the canonical render" — a *normalization fix* in the renderer's correctness surface, not a rule.

Two decisions follow from this framing:

1. **No `Constraint::Custom` catalog row on `CapcoScheme` for these rules.** Adding rows would couple the walker to the trait/validate-path machinery (`evaluate_custom_by_attrs`) that exists to surface scheme-level invariants to non-engine callers. Non-canonical-input is an engine-only concern (an external structural-validation caller wants the canonical attributes; it does not care whether the input was written in canonical surface form). Keeping the catalog private to the walker file avoids polluting the scheme surface with rows that have to be torn out cleanly when the renderer absorbs them in PR 5+.
2. **The per-row data structure stays minimal.** Each row carries: a `kind` enum variant, a `presence` predicate fn-pointer (deciding whether the row applies to the current attributes), an `evaluate` fn-pointer (returning `Vec<Diagnostic>`), a `&'static str` citation, and a `Severity`. No fix-shape struct, no closure-capturing context — the per-row evaluate fn handles everything. Modeling the four ordering checks as more uniformly-structured rows (e.g., adding a `sort_axis: AxisRef` field) would over-fit the data structure to today's four rows when the renderer absorbs them in a few months.

### 1.2 Forward-link comment

The implementation pastes the following comment block into the walker module near the catalog declaration so the Stage-4 retirement is discoverable to future agents:

```text
PR 3b.F (T026f) — Non-canonical input walker.

This walker exists as a STAGE-1 INTERIM. The four ordering rules
collapsed here (E020 REL TO + JOINT alpha, E023 SIGMA numeric sort,
E028 SAR ascending alpha, E033 SCI compartment + sub-compartment
alpha) are renderer-canonical-form concerns per `marque-applied.md`
§3.6 + §3.10 Move 7. Once `MarkingScheme::render_canonical` lands
in PR 5+ (Stage 4 of the engine refactor) the renderer absorbs
canonical-form rendering, and "your input doesn't match the
canonical form" becomes a normalization fix in the renderer's
correctness surface, not a `Rule`.

When that happens, this entire walker — `DeclarativeNonCanonicalInputRule`,
the `NON_CANONICAL_CATALOG` table, and the per-row evaluators — retires
cleanly. The audit-stream consumers must keep working through the
transition: the renderer-emitted normalization fix carries a
`FixProposal` with the same shape as today's walker emits (span +
replacement + confidence + source), and `Engine::fix_inner` continues
to be the sole `AppliedFix::__engine_promote` caller. See
`docs/plans/2026-05-08-pr3b-F-non-canonical-input-walker-plan.md` for
the architectural rationale; `docs/plans/2026-05-02-engine-refactor-
consolidated.md` Stage 4 + `tasks.md` T026f checkbox for the
retirement plan.
```

The comment names PR 5+ as the migration vehicle and explicitly preserves the `FixProposal`-shape contract so the audit stream survives the rendererization.

---

## 2. The verified ordering catalog

Verification methodology per Constitution Principle VIII:

- **Read each rule's prose** in `crates/capco/docs/CAPCO-2016.md` and confirm the ordering language at the cited page anchor exists.
- **Verify each `begin page NNN` anchor is present** in the vendored markdown via `grep -n "begin page <N>" crates/capco/docs/CAPCO-2016.md`.
- **Page numbers only** (per `feedback_citations_use_page_numbers.md`). Form `CAPCO-2016 §X.Y pNN`.

### 2.1 Catalog rows (5 rows — PM decision OQ-1 below; 4 if folded)

The recommended shape splits E020 into a 5th row for JOINT alphabetical ordering, because the JOINT rule cites a different §-passage (§H.3 p56) than REL TO (§H.8 p150-151) and Constitution VIII (single citation per declarative entry; D13) is satisfied more cleanly with one §-citation per row. A 4-row alternative folds JOINT and REL TO under a single row with a multi-page citation `§H.3 p56 + §H.8 p150-151`; this is structurally sound but mixes two distinct passages in one citation field. See OQ-1 for the decision tradeoff.

| # | `kind` variant | §-citation | Family pattern | Invariant kind | Fix proposal shape | Default severity |
|---|---|---|---|---|---|---|
| 1 | `RelToUsaFirstAlpha` | `CAPCO-2016 §H.8 p151` | `attrs.rel_to.len() >= 2` AND `attrs.rel_to[0] == USA` (USA-first guard; if USA missing or not first, E002 fires; see §3.1) | REL TO trigraphs alphabetical after USA, then tetragraphs alphabetical (per §H.8 p151 prose: *"After 'USA', list the required one or more trigraph country codes in alphabetical order followed by tetragraph codes listed in alphabetical order."*) | Span = single `RelToBlock` token span; replacement = `canonicalize_trigraph_list(&attrs.rel_to, true)` joined `, `; confidence = `1.0`; `FixSource::BuiltinRule`. **Multi-block REL TO suppression preserved**: when `rel_to_blocks.len() > 1`, emit a no-fix diagnostic at the first block span with the suppression message (per existing E020 behavior, lines 3110–3133). | `Severity::Fix` (matches existing E020) |
| 2 | `JointAlphabetical` | `CAPCO-2016 §H.3 p56` | `attrs.classification` is `Joint(j)` AND `j.countries.len() >= 2` | JOINT participants alphabetical (no USA-first carve-out per §H.3 p56 prose: *"Country trigraph codes are listed alphabetically followed by tetragraph codes in alphabetical order."*) | Span = via `check_trigraph_ordering(...)` helper inside the joint classification token; replacement constructed via `canonicalize_trigraph_list(&j.countries, false)` — `usa_first = false`. Confidence = `1.0`; `FixSource::BuiltinRule`. | `Severity::Fix` (matches existing E020 — same default for both REL TO and JOINT) |
| 3 | `SigmaNumericSort` | `CAPCO-2016 §H.6 p108` | `attrs.aea_markings` contains an `Rd { sigma }` or `Frd { sigma }` with `sigma.len() >= 2` AND `sigma` is not in numerical order. **§H.6 p108 (RD-SIGMA template) is the authoritative ordering passage**; §H.6 p113 (FRD-SIGMA template) restates the same numerical-order rule by reference. The walker's row cites p108 only, matching existing E023 behavior at line 4235. | AEA SIGMA numbers ascending numeric (per §H.6 p108 prose: *"Multiple SIGMA numbers shall be listed in numerical order with a space preceding each value."* Verified at line 2652 of the vendored markdown — within the §H.6 p108 begin/end-page anchors at lines 2622 / 2660.) | Span = first `AeaMarking` token span; replacement = sorted `sigma` list joined by `" "`. Original = unsorted `sigma` list. Confidence = `1.0`; `FixSource::BuiltinRule`. **Two emits per AEA marking when both invalid + misordered**: existing E023 emits an "invalid-set" no-fix diagnostic AND a "misorder" fix diagnostic. The walker preserves both branches for byte-identity. The "invalid-set" emit (citation `§H.6 p108`) is part of this same row's evaluate fn — the walker does NOT spawn a separate row, because (a) splitting would make the row count 6 rather than 5 and (b) the invalid-set check is structurally tied to the same SIGMA inspection pass. | `Severity::Fix` (matches existing E023) |
| 4 | `SarProgramAscendingSort` | `CAPCO-2016 §H.5 p99` | `attrs.sar_markings.is_some()` AND `programs.len() >= 2` AND programs are not in ascending sort order | SAR programs ascending sort, numeric first then alphabetic (per §H.5 p99 prose: *"Multiple program identifiers are listed in ascending sort order with numbered values first, followed by alphabetic values."* Verified at line 2391 of the vendored markdown — within §H.5 p99 begin/end-page anchors at lines 2386 / 2399.) | Span = `sar_block_span(attrs)`; replacement = whole-block rewrite via `render_sar_block(sar.indicator, &sorted)` where `sorted` is the programs sorted by `sar_sort_key` AND each program's compartments + sub-compartments also sorted (preserving the existing E028 single-pass-canonical behavior — applying the fix alone fully normalizes the block even when the retired E029 violations are present). Confidence = `0.85`; `FixSource::BuiltinRule`. | `Severity::Fix` (matches existing E028) |
| 5 | `SciCompartmentNumericThenAlpha` | `CAPCO-2016 §H.4 p61` | `attrs.sci_markings` non-empty AND any marking has compartments out-of-order (numeric first then alpha) OR any compartment has sub-compartments out-of-order | SCI compartment numeric-then-alpha ascending; SCI sub-compartment numeric-then-alpha ascending (per §H.4 p61 prose verified at lines 1342-1346 of the vendored markdown — within §H.4 p61 begin/end-page anchors at lines 1335 / 1356: *"Multiple compartments within an SCI control system must be listed in ascending sort order with numbered values first followed by alphabetic values separated by a hyphen ... Multiple sub-compartments must be listed in ascending sort order with numbered values first followed by alphabetic values separated by a space."*) | Per-marking emit (`out.push(make_fix_diagnostic(...))` once per out-of-order marking, mirroring existing E033 behavior at lines 5139-5153). Span = whole compartment+sub-compartment region of the marking; replacement = `render_comps(&sorted_comps)` after sorting compartments + sub-compartments together. Confidence = `0.85`; `FixSource::BuiltinRule`. **Two citation strings** (existing E033 splits compartment-level vs sub-compartment-level): the walker preserves both via `(level, citation)` selection inside the evaluate fn — both citations cite §H.4 p61 but with parenthetical specificity ("SCI compartments: ascending..." vs "SCI sub-compartments: ascending..."). Both are `§H.4 p61`. | `Severity::Error` (matches existing E033) |

### 2.2 Citation verification

Each citation re-grepped against `crates/capco/docs/CAPCO-2016.md`:

```
begin page 56   → line 1232  (verified: §H.3 JOINT prose at line 1262: "Country trigraph codes are listed alphabetically followed by tetragraph codes in alphabetical order.")
begin page 61   → line 1335  (verified: §H.4 SCI compartment/sub-compartment ordering prose at lines 1344, 1346)
begin page 99   → line 2386  (verified: §H.5 SAR program ascending-sort prose at line 2391)
begin page 108  → line 2622  (verified: §H.6 RD-SIGMA numerical-order prose at line 2652)
begin page 151  → line 3709  (verified: §H.8 REL TO USA-first + alphabetical prose at line 3714)
```

All five `begin page NNN` anchors are present in the vendored markdown. All five operative ordering passages are present at the cited page anchors. No citation drift.

### 2.3 Default severity heterogeneity

The four retired rules have **two different default severities**: E020, E023, E028 are `Severity::Fix`; E033 is `Severity::Error`. The catalog row therefore stores `severity: Severity` per-row rather than at the walker level. The walker's `Rule::default_severity()` is **`Severity::Error`** (matches the strictest of the per-row defaults; PM-resolved per OQ-3). The walker-level default is what `[rules] E060 = ...` engages when used as a coarse-grained override anchor — `[rules] E060 = "off"` skips the walker entirely; `[rules] E060 = "warn"` downgrades every emitted diagnostic to Warn per the engine's severity-override layer; the per-row severity (`Fix` for rows 1–4, `Error` for row 5) is what's emitted when no override is set.

This is structurally identical to PR 3b.A's banner walker pattern (per-row `severity` field, walker-level default = strictest-of-rows). The walker default `Severity::Error` matches the PR 3b.A precedent and ensures a config that uses E060 as the override anchor cannot accidentally weaken any row below its authoring intent without an explicit user choice.

---

## 3. Rule-by-rule retire-vs-reroute analysis

Each subsection covers one retired rule. Classification per the 3b.E pattern: **(a) retire entirely with no replacement**, **(b) convert to a walker row**, or **(c) mixed**.

### 3.1 E020 `CountryCodeOrderingRule` (rules.rs:3065–3187)

**Predicate body** (verbatim from the source):

The rule contains **two structurally distinct sub-checks**:

1. **REL TO ordering** (lines 3081–3151). Guards: `attrs.rel_to.len() >= 2` AND `attrs.rel_to[0] == USA` (USA-first; if USA is missing or not first, E002 fires for those cases and its fix produces a fully-canonical list, so E020 silently absorbs into E002 — see the existing rule's doc comment at lines 3050–3057). Multi-block REL TO suppression at lines 3110–3133: when `rel_to_blocks.len() > 1`, emit a no-fix diagnostic with the message `"REL TO country codes must be alphabetically ordered (USA first when present): [...] → [...] (multiple REL TO blocks present; fix suppressed to avoid cross-block corruption — resolve manually)"`. Single-block path delegates to `check_trigraph_ordering(&attrs.rel_to, "REL TO", ..., true /* usa_first */, ...)`.
2. **JOINT ordering** (lines 3164–3183). Guards: `Some(MarkingClassification::Joint(j)) = &attrs.classification` AND `j.countries.len() >= 2`. Delegates to `check_trigraph_ordering(&j.countries, "JOINT", ..., false /* usa_first */, ...)`.

**Citations**: REL TO uses `concat!("CAPCO-2016 §H.8 p150–151 ", "(REL TO: trigraphs alpha, then tetragraphs alpha, USA first)")`. JOINT uses `concat!("CAPCO-2016 §H.3 p56 ", "(JOINT: trigraphs alpha, then tetragraphs alpha)")`. **Two distinct §-citations** in one rule.

**Fix shape**: per `make_fix_diagnostic` at lines 4112-4128 in the `check_trigraph_ordering` helper: span = supplied (RelToBlock for REL TO, none/computed for JOINT), confidence = `1.0`, `FixSource::BuiltinRule`, `original` and `replacement` joined comma-space. Multi-block REL TO suppression is the only no-fix path.

**Severity**: `Severity::Fix`.

**Scope guard**: portion-or-banner-or-CAB (no marking-type filter at the rule level — the rule fires on any candidate type that has a populated `attrs.rel_to` or `attrs.classification`).

**Decision: (b) convert to TWO walker rows** (rows 1 + 2 in §2.1). The two sub-checks have distinct §-citations, distinct fix-construction paths, and distinct guards — folding them under one row would force a multi-page citation `§H.3 p56 + §H.8 p150-151` and an internal `if classification.is_joint() { ... } else { ... }` branch in the row's evaluate fn, increasing per-row complexity. Splitting cleanly satisfies D13 (single CAPCO-§ citation per declarative catalog entry) without contortion.

**Alternative considered (folded)**: a single `CountryCodeAlphabetical` row that internally dispatches on classification. Rejected for the citation-cleanliness reason above, but explicitly surfaced as **OQ-1** for PM decision; if PM prefers the 4-row catalog, splitting is reversible at implementation time.

### 3.2 E023 `SigmaValidationRule` (rules.rs:4151–4246)

**Predicate body**: iterate `attrs.aea_markings`; for each `Rd { sigma } | Frd { sigma }` with non-empty `sigma`:

1. **Invalid-set check** (lines 4193–4212): values outside `[14, 15, 18, 20]` → emit a no-fix `Diagnostic` with citation `"CAPCO-2016 §H.6 p108"`. Authority: §H.6 p108 specifies *"SIGMA # currently represents one or more of the following numbers: 14, 15, 18, and 20."* (verified at line 2657 of vendored markdown).
2. **Numerical-order check** (lines 4215–4242): `sigma.len() >= 2` AND `sigma != sorted_dedup(sigma)` → emit a fix `Diagnostic` with citation `"CAPCO-2016 §H.6 p108"`, span = first `AeaMarking` token span, replacement = sorted+deduped `sigma` joined by space. Confidence `1.0`, `FixSource::BuiltinRule`.

**Note on §H.6 p113 (FRD-SIGMA)**: §H.6 p113 (verified at line 2765 of the vendored markdown) restates the same numerical-order rule for FRD-SIGMA but is a **template-body restatement**, not a separate authority. The existing E023 cites only §H.6 p108. **The walker's row 3 follows the existing rule's choice — single citation `§H.6 p108`** to maintain byte-identity. If §H.6 p113 were also relevant, the walker would split the row into RD-SIGMA-vs-FRD-SIGMA — but the existing E023 does not, and PR 3b.F is byte-identity-preserving across the four retiring rules. **No row split for RD vs FRD SIGMA.**

**Fix shape**: see row 3 in §2.1. Two emit paths per AEA marking with `sigma`: invalid-set (no-fix) + misorder (fix). Both preserved verbatim under one walker row.

**Severity**: `Severity::Fix`.

**Scope guard**: no marking-type filter; fires on any candidate with AEA markings.

**Decision: (b) convert to walker row 3 (`SigmaNumericSort` kind)**. Both emit branches (invalid-set + misorder) live inside row 3's evaluate fn — splitting them across rows would force a 6-row catalog and add complexity for no citation-cleanness benefit (both branches cite §H.6 p108).

### 3.3 E028 `SarProgramOrderRule` (rules.rs:4379–4447)

**Predicate body**: `attrs.sar_markings.is_some()` AND `sar.programs.len() >= 2` AND programs not in ascending order (`sar_sort_key`-based pairwise comparison).

**Fix shape**: whole-block rewrite at `sar_block_span(attrs)`. The fix sorts `sar.programs` and *also* normalizes per-program compartments and sub-compartments in the same pass — see the existing rule's doc comment at lines 4373-4378 explaining why: "applying E028's fix fully normalizes the block even when E029 violations are present." This single-pass-canonical behavior is preserved under row 4. Confidence `0.85`, `FixSource::BuiltinRule`.

**Citation**: `"CAPCO-2016 §H.5 p99 (programs: ascending, numeric first, then alpha)"`. Authority verified at line 2391 of vendored markdown — *"Multiple program identifiers are listed in ascending sort order with numbered values first, followed by alphabetic values."*

**Severity**: `Severity::Fix`.

**Scope guard**: no marking-type filter.

**Decision: (b) convert to walker row 4 (`SarProgramAscendingSort` kind)**. Single-pass-canonical fix preserved verbatim — the row's evaluate fn calls into the existing `render_sar_block` + `sar_sort_key` helpers (which remain in `rules.rs` since E029 / `SarCompartmentOrderRule` continues to use them).

### 3.4 E033 `SciCompartmentOrderRule` (rules.rs:4989–5161)

**Predicate body**: iterate `attrs.sci_markings` and for each marking:

- Compute `comps_ok`: compartment ordering check (`n_comps < 2 || windows(2).all(...)` per `sar_sort_key`).
- Compute `subs_ok`: per-compartment sub-compartment ordering check.
- If `comps_ok && subs_ok`, advance cursors, continue.
- Otherwise emit one diagnostic per out-of-order marking (not per level) — see the existing rule's doc comment at lines 4971-4983 explaining the choice: per-marking emit avoids overlap-guard conflicts between compartment-level and sub-compartment-level fixes within the same marking, and supersedes cleanly under E032's whole-block span (FR-016 ordering).

**Fix shape**: whole compartment+sub-compartment region span (lines 5043-5082). Replacement = `render_comps(&sorted_comps)` where `sorted_comps` is compartments sorted with their sub-compartments sorted in-place. Confidence `0.85`, `FixSource::BuiltinRule`.

**Citations**: TWO citation strings (one per level), selected at lines 5120-5137 based on `(comps_ok, subs_ok)`:

- Compartment-level: `concat!("CAPCO-2016 §H.4 p61 ", "(SCI compartments: ascending, numeric first, then alpha)")`.
- Sub-compartment-level: `concat!("CAPCO-2016 §H.4 p61 ", "(SCI sub-compartments: ascending, numeric first, then alpha)")`.

Both cite `§H.4 p61` (the operative SCI ordering page; verified at lines 1344-1346 of vendored markdown). The two citations differ only in parenthetical specificity ("SCI compartments" vs "SCI sub-compartments"), so both fall under the single `§H.4 p61` citation for D13 purposes — the parenthetical specificity is a UX detail of the diagnostic message that helps auditors land on the right sentence. The walker's row 5 carries citation `"CAPCO-2016 §H.4 p61"` and the per-level parenthetical is constructed in the evaluate fn.

**Severity**: `Severity::Error` (the only `Error`-default among the four retiring rules).

**Scope guard**: no marking-type filter.

**Decision: (b) convert to walker row 5 (`SciCompartmentNumericThenAlpha` kind)**. Per-marking emit semantics + two-citation parenthetical preserved inside the row's evaluate fn.

### 3.5 Per-row summary table

| Row | Rule retired | Classification | Walker row kind | Severity | Citation |
|---|---|---|---|---|---|
| 1 | E020 (REL TO sub-check) | (b) | `RelToUsaFirstAlpha` | `Fix` | `CAPCO-2016 §H.8 p151` |
| 2 | E020 (JOINT sub-check) | (b) | `JointAlphabetical` | `Fix` | `CAPCO-2016 §H.3 p56` |
| 3 | E023 | (b) | `SigmaNumericSort` | `Fix` | `CAPCO-2016 §H.6 p108` |
| 4 | E028 | (b) | `SarProgramAscendingSort` | `Fix` | `CAPCO-2016 §H.5 p99` |
| 5 | E033 | (b) | `SciCompartmentNumericThenAlpha` | `Error` | `CAPCO-2016 §H.4 p61` |

**Tally**: 4 rules retired, 5 walker rows. Net rule delta: 4 retired + 1 walker = **net −3; running registered-rule count 50 → 47.**

---

## 4. Implementation outline

### 4.1 Files touched

**File-placement decision**: place `DeclarativeNonCanonicalInputRule` and the `NON_CANONICAL_CATALOG` table in **`crates/capco/src/rules_declarative.rs`** rather than a new sibling file `rules_non_canonical.rs`. Rationale: 3b.A / 3b.D / 3b.E all placed their walkers in `rules_declarative.rs` (see line 1538 for `DeclarativeClassFloorRule` and line 1724 for `DeclarativeSciPerSystemRule`); siblings would fragment the walker family. The walker module gets a new section header `// PR 3b.F (T026f) — Non-canonical input walker (E060)` after the SCI per-system walker. The deletion of the four ordering rules from `rules.rs` is the bulk file-delta; `rules_declarative.rs` grows by ~250-400 LOC for the walker + catalog + per-row evaluator fns.

**`crates/capco/src/rules_declarative.rs`**:

- **Add** new section `// PR 3b.F (T026f) — Non-canonical input walker (E060)` after the SCI per-system walker block (after line ~1770).
- **Add** the forward-link comment block from §1.2.
- **Add** the catalog struct and enum types (private to the file unless a test needs accessor; see §6 for the test-helper strategy):

  ```rust
  /// One catalog row per non-canonical-input ordering invariant. Ordering of
  /// rows controls only emit order for a single candidate; correctness is
  /// independent of row order.
  struct NonCanonicalRow {
      kind: NonCanonicalKind,
      /// Stable per-row identifier; used in diagnostic-content searches and
      /// catalog-pin tests. Not emitted as the diagnostic's `rule` field —
      /// that's `E060` via `Rule::id()`. Contributes to the audit-stream
      /// traceability invariant per §1.1.
      name: &'static str,
      severity: Severity,
      citation: &'static str,
      /// Quick presence check; gates the per-row evaluate fn so the hot-path
      /// early-out skips rows whose axis is empty for this candidate.
      presence: fn(&CanonicalAttrs) -> bool,
      /// Per-row evaluator. Returns the diagnostics this row produces for
      /// the given attributes + context.
      evaluate: fn(&CanonicalAttrs, &RuleContext, &NonCanonicalRow) -> Vec<Diagnostic>,
  }

  enum NonCanonicalKind {
      RelToUsaFirstAlpha,
      JointAlphabetical,
      SigmaNumericSort,
      SarProgramAscendingSort,
      SciCompartmentNumericThenAlpha,
  }
  ```

- **Add** the catalog table:

  ```rust
  const NON_CANONICAL_CATALOG: &[NonCanonicalRow] = &[
      NonCanonicalRow {
          kind: NonCanonicalKind::RelToUsaFirstAlpha,
          name: "non-canonical/rel-to-usa-first",
          severity: Severity::Fix,
          citation: "CAPCO-2016 §H.8 p151",
          presence: presence_rel_to_usa_first_candidate,
          evaluate: evaluate_rel_to_usa_first_alpha,
      },
      // ... rows 2-5 in the order matching §2.1
  ];
  ```

- **Add** the per-row presence + evaluate fns. The evaluate fns are **verbatim moves** of the bodies of the retiring rules' `check` methods, with the only structural change being parameter name (`row` instead of `self.id()` everywhere; row.severity instead of `self.default_severity()`; row.citation instead of inline strings). This preserves byte-identity for diagnostic message text + fix shapes — the corpus-fixture churn is reduced to the rule-ID rename only.

- **Add** the walker:

  ```rust
  pub(crate) struct DeclarativeNonCanonicalInputRule;

  impl Rule for DeclarativeNonCanonicalInputRule {
      fn id(&self) -> RuleId {
          RuleId::new("E060")
      }
      fn name(&self) -> &'static str {
          "non-canonical-input"
      }
      fn default_severity(&self) -> Severity {
          // Strictest of the per-row defaults (matches PR 3b.A banner walker
          // precedent). Walker-level overrides (`[rules] E060 = "off"|"warn"|
          // "error"`) engage at this level via the engine's severity-override
          // layer. The per-row `severity` field is what's emitted when no
          // override is set: `Fix` for rows 1-4 (REL TO / JOINT / SIGMA / SAR),
          // `Error` for row 5 (SCI). PM-resolved per OQ-3.
          Severity::Error
      }

      fn check(&self, attrs: &CanonicalAttrs, ctx: &RuleContext) -> Vec<Diagnostic> {
          // Layer 1 (axis-presence early-out): bail when none of the four
          // ordering axes are populated. On prose body text this is the
          // dominant case and the catalog walk is skipped entirely.
          if !axis_presence_any(attrs) {
              return Vec::new();
          }
          // Layer 2 (direct row dispatch): per-row presence guard + evaluate.
          let mut diags = Vec::new();
          for row in NON_CANONICAL_CATALOG {
              if (row.presence)(attrs) {
                  diags.extend((row.evaluate)(attrs, ctx, row));
              }
          }
          diags
      }

      fn additional_emitted_ids(&self) -> &'static [(&'static str, &'static str)] {
          // Severity-config compatibility for the legacy IDs is intentionally
          // NOT preserved — per `feedback_pre_users_no_deprecation_phasing.md`,
          // marque is pre-users; rewrite freely. Returning an empty slice
          // here means `[rules] E020 = ...` (and E023/E028/E033 likewise) are
          // rejected at engine construction with the standard "unknown rule
          // ID" error, forcing users to migrate to `[rules] E060 = ...`.
          &[]
      }
  }
  ```

- **Add** axis-presence helper:

  ```rust
  fn axis_presence_any(attrs: &CanonicalAttrs) -> bool {
      // Skip when none of the four ordering axes are populated. Each
      // sub-check is O(1) (slice/option-is-empty + classification-is-joint).
      !attrs.rel_to.is_empty()
          || matches!(&attrs.classification, Some(MarkingClassification::Joint(_)))
          || !attrs.aea_markings.is_empty()
          || attrs.sar_markings.is_some()
          || !attrs.sci_markings.is_empty()
  }
  ```

  This bypasses the catalog walk on the dominant prose-body case (no markings present at all) and on candidates with markings whose axis isn't covered by the walker.

**`crates/capco/src/rules.rs`**:

- **Delete** four `impl Rule for ...` blocks: `CountryCodeOrderingRule` (3065-3187), `SigmaValidationRule` (4151-4246), `SarProgramOrderRule` (4379-4447), `SciCompartmentOrderRule` (4989-5161). Total LOC retired: ~487.
- **Delete** four corresponding `Box::new(...)` registrations in `CapcoRuleSet::new()` at lines 143 (`CountryCodeOrderingRule`), 145 (`SigmaValidationRule`), 169 (`SarProgramOrderRule`), 173 (`SciCompartmentOrderRule`).
- **Add** one `Box::new(DeclarativeNonCanonicalInputRule)` registration in a position that preserves the engine's natural rule order. The walker covers checks across REL TO / JOINT / AEA / SAR / SCI axes — none of which is a single home position. **Recommendation**: place after `DeclarativeSciPerSystemRule` (line 232) so all the post-3b walker family sit together, and before the SAR / SCI structural rules (`SciSystemOrderRule` line 172 etc. — but those are still in their current position because they handle structural validity, not ordering). The implementation agent verifies that this placement does not alter the engine's effective rule-evaluation order in a way that breaks an existing FR-016 overlap-guard test (e.g., E028 vs E031 banner-rollup walker; E020 vs E052 REL TO no-duplicates).
- **Update** the rule-list doc comment at the top of `rules.rs` to reflect the four retirements + walker addition.
- **Preserve** the helpers used by the retired rules: `canonicalize_trigraph_list`, `dedup_country_codes`, `check_trigraph_ordering`, `make_fix_diagnostic`, `FixDiagnosticParams`, `sar_block_span`, `sar_block_source`, `render_sar_block`, `sar_sort_key`, `render_sci_block`. **All of these are still used** by other rules (E002, E029, E032, E052) — they remain in `rules.rs` as `pub(crate)` items. The walker calls them through their existing visibility.
- **Delete** any rule-private helpers used only by the retired rules. **Verification needed at implementation time**: `grep -n "fn " crates/capco/src/rules.rs | grep -E '^[0-9]+:fn '` and trace usage; helpers private to the four retiring rules can be deleted, helpers shared with E002/E029/E032/E052/etc. stay.

**`crates/capco/src/rules_declarative.rs`** (additional):

- **Add** `use marque_ism::MarkingClassification` if not already imported (E020 uses it; verify the import is present).
- **Update** the rule-list doc comment at the top of the file (if any).

**`crates/capco/src/lib.rs`**: probably unchanged — `rules_declarative` is already declared. Verify.

**`crates/capco/Cargo.toml`**: no change expected.

**`crates/capco/tests/non_canonical_input_walker.rs` (new)** — see §6 for full test plan.

**`crates/capco/tests/corpus_parity.rs`**:

- **Update** the rule-count pin from `50` to `47`.
- **Add** a comment block in the `rule_count_reflects_registration_changes` test documenting the T026f delta:

  ```text
  // T026f (PR 3b Sub-move F): retired four ordering rules
  // (E020 CountryCodeOrderingRule, E023 SigmaValidationRule,
  // E028 SarProgramOrderRule, E033 SciCompartmentOrderRule) into
  // the DeclarativeNonCanonicalInputRule walker (rule ID E060)
  // dispatching over a 5-row internal catalog (NON_CANONICAL_CATALOG)
  // covering REL TO USA-first alpha (§H.8 p151), JOINT alpha
  // (§H.3 p56), AEA SIGMA numeric sort (§H.6 p108), SAR program
  // ascending alpha (§H.5 p99), and SCI compartment + sub-compartment
  // numeric-then-alpha (§H.4 p61). Diagnostics emit with
  // `Diagnostic.rule = "E060"`; per-row identification flows via the
  // diagnostic message text (which preserves the existing rule's
  // human-readable phrasing verbatim). The walker retires when the
  // Phase C renderer trait surface lands in PR 5+ (Stage 4).
  // Net delta: -3 rules (4 retired + 1 walker added). Final: 50 - 3 = 47.
  ```

- **Update** the `assert_eq!` at line 156-157.

**`crates/capco/tests/rel_to_invariants.rs`**:

- **Update** all `E020`-asserting tests: change `"E020"` literals to `"E060"` (8 occurrences via `grep -c '"E020"' crates/capco/tests/rel_to_invariants.rs`).
- **Verify** that the tests still pass with the new walker emitting on `E060`. The test logic asserts FR-016 overlap-guard interaction between E020 and E052 (REL TO no-duplicates) — the lex-tiebreaker changes when the rule ID changes from `E020` to `E060`. **R-1: critical correctness check at implementation time**: re-run `rel_to_invariants.rs` and verify the C-1 overlap guard still admits exactly one of {E060, E052} on the duplicate-and-misordered fixture; if the FR-016 lex tiebreaker now resolves the other way (E052 < E060 lexicographically? compare: `E052` < `E060` lexicographically because '5' < '6'), the test's assertion of which rule wins must be updated, but the invariant ("exactly one survives") must continue to hold. **Document the new winner in the test comment.** This is a behavior change visible in audit logs (the `AppliedFix.proposal.rule` field changes from `E020` to either `E060` or `E052`) but does not change the canonical fixed output.

**`crates/capco/tests/banner_rollup_walker_overlap_guard.rs`**:

- **Update** all `E028`-asserting tests: change `"E028"` literals to `"E060"` (per `grep -c '"E028"' ` — about 5-7 occurrences). Same FR-016 lex-tiebreaker review as above; the overlap-guard test interacts with the banner-rollup walker emitting `E031`. `E031` < `E060` lexicographically, so the existing `vec!["E031", "E028"]` assertion at line 173 changes to `vec!["E031", "E060"]`. The test's correctness invariant ("E031 walker row coexists with E028, both fix paths fire on non-overlapping spans") survives the rename.
- **Verify** at implementation time that no other behavioral assertion breaks.

**`crates/capco/tests/banner_rollup_walker.rs`**:

- **Update** the `E028` reference at line 94 (from a comment / assertion).

**`crates/wasm/tests/parity_corpus.json`**:

- **Update** all `E028` and `E033` references — three entries in the JSON (line 146 with `"rule":"E028"`, lines 188 and 200 with `"rule":"E033"`). Change each to `"rule":"E060"`.
- **Preserve** the diagnostic message text and citation strings verbatim (the walker emits byte-identical output to the retired rules). **Critical**: this means the `expected_lint` JSON values for these three corpus entries change ONLY in the `rule` field — message and citation stay the same. The implementation agent verifies via `cargo test -p marque-wasm` that the parity corpus still passes after the rename.

**`tests/corpus/invalid/sar_bad_program_order.expected.json`**:

- **Update** the `"rule": "E028"` to `"rule": "E060"` (line 3).

**`tests/corpus/invalid/sci_compartment_order.expected.json`**:

- **Update** the `"rule": "E033"` to `"rule": "E060"` at line 4.
- **Update** the `_note` field at line 2 to reflect the rename.

**`tests/corpus/`** at workspace root: grep for `"rule": "E020"`, `"rule": "E023"`, `"rule": "E028"`, `"rule": "E033"` across all `*.expected.json` files and rename all to `"rule": "E060"`. Use:

```sh
grep -rn '"rule": "E020"\|"rule": "E023"\|"rule": "E028"\|"rule": "E033"' tests/corpus/ crates/wasm/tests/
```

at implementation time to enumerate every occurrence; rename each.

**`crates/capco/README.md`**:

- **Update** rule-inventory paragraph: E020, E023, E028, E033 retire; E060 added.
- **Bump** rule count: 50 → 47 (or whatever the README's current notation is — verify at implementation time).

**`specs/006-engine-rule-refactor/tasks.md`**:

- **Flip** T026f checkbox from `[ ]` to `[x]` at line 107.
- **Update** the T026f line with the parenthetical "(COMPLETED — 5 rows + 1 walker; see `docs/plans/2026-05-08-pr3b-F-non-canonical-input-walker-plan.md`)" matching the 3b.D / 3b.E completion patterns.

**`specs/006-engine-rule-refactor/plan.md`**:

- **Update** the 3b.F status line at lines 325-331 to reflect the landing.
- **Update** the running rule-count narrative (50 → 47).

**`CLAUDE.md`** "Recent Changes" section:

- **Add** a PR 3b.F entry at the top of the section, matching the 3b.E entry's format.

### 4.2 Walker rule-ID and per-row identification

**Single walker rule-ID: `E060`.** Verified next-free slot:

```sh
$ grep -hrEo 'RuleId::new\("E0[0-9]+"\)' crates/capco/src/ | sort -u
RuleId::new("E001") .. RuleId::new("E059")
```

E001–E059 are all assigned; E060 is unused.

All 5 catalog rows emit diagnostics with `Diagnostic.rule = RuleId::new("E060")`. Per-row identification is via:

- **Diagnostic.message text**: each row preserves the existing rule's diagnostic message verbatim. A reviewer reading "REL TO country codes must be alphabetically ordered (USA first when present): [...]" can still tell which kind of violation fired — same phrasing as today's E020.
- **Catalog row `name` field**: used in catalog-pin tests + an audit-traceability test that verifies per-row identifiability via message-text content (since the catalog itself is private to the walker module, downstream consumers don't have direct access to the row names). See §6 for the test-helper strategy that avoids `pub fn` + `#[doc(hidden)]`.
- **Citation field**: the row's citation propagates onto the `Diagnostic.citation` field via the per-row evaluate fn. A reviewer grepping audit logs for `§H.8 p151` finds REL TO USA-first violations specifically.

**Severity-config compatibility for the legacy IDs (E020, E023, E028, E033) is intentionally NOT preserved.** Per `feedback_pre_users_no_deprecation_phasing.md`: marque is pre-users; rewrite freely. `.marque.toml` files keying ordering severity overrides MUST migrate to `E060`. The walker's `additional_emitted_ids()` returns `&[]`, so the engine's `canonicalize_rule_overrides` path will reject `[rules] E020 = ...` etc. with the standard "unknown rule ID" error.

`[rules] E060 = "off"` toggles the entire walker off (FR-008-correct).

### 4.3 Walker's `Rule::check` body — branch count

The walker's `check` body has exactly **2 branches**:

1. **Axis-presence early-out**: `if !axis_presence_any(attrs) { return Vec::new(); }` — one branch.
2. **Catalog-walk loop**: `for row in NON_CANONICAL_CATALOG { if (row.presence)(attrs) { diags.extend((row.evaluate)(attrs, ctx, row)); } }` — one branch (the `if (row.presence)(attrs)` is a per-iteration guard inside the loop, not a separate top-level branch in the rule's `check` body).

Inside each per-row evaluator, the body is the verbatim move of the retired rule's `check` body — those bodies have variable branch counts (e.g., E020's body has 2 sub-branches: REL TO check + JOINT check; E033's body has nested branches for compartment-vs-subcompartment), but they are inside the per-row evaluator fn-pointer call, not the walker's `Rule::check` body. **The D13 ≤3-branch attestation applies to the walker's `Rule::check` body — satisfied.** Per §1 of this plan and per the D13 interpretation in 3b.A's banner walker (which has 3 branches: marking-type guard + page-context guard + dispatch loop), the walker meets the gate.

**Note on the per-row evaluator branch counts.** Each per-row evaluator inherits its branching from the retired rule it replaces. E020's `RelToUsaFirstAlpha` evaluator has 2 internal sub-branches (multi-block REL TO suppression + single-block fix path). E023's `SigmaNumericSort` evaluator has 2 internal sub-branches (invalid-set + misorder). E028's and E033's evaluators have 1 each. None of these per-row evaluators are themselves an `impl Rule` block, so D13 does not apply to them — D13 is "≤3 branches per `impl Rule` block." The walker is the only `impl Rule` block in scope; its body has 2 branches.

**Reviewer-attestation phrasing** for the PR description: *"The walker's `Rule::check` body has 2 internal branches (axis-presence early-out + catalog-walk loop). Per-row evaluators are fn-pointer calls inside the loop and are not `impl Rule` blocks; D13 ≤3-branch cap satisfied."*

### 4.4 Per-row presence predicates

```rust
fn presence_rel_to_usa_first_candidate(attrs: &CanonicalAttrs) -> bool {
    // Precondition: REL TO has 2+ entries AND USA is first.
    // If USA is missing or not first, E002 fires for those cases.
    attrs.rel_to.len() >= 2
        && attrs.rel_to.first().is_some_and(|t| *t == marque_ism::CountryCode::USA)
}

fn presence_joint_alphabetical(attrs: &CanonicalAttrs) -> bool {
    matches!(&attrs.classification, Some(MarkingClassification::Joint(j)) if j.countries.len() >= 2)
}

fn presence_sigma_numeric_sort(attrs: &CanonicalAttrs) -> bool {
    use marque_ism::AeaMarking;
    attrs.aea_markings.iter().any(|aea| match aea {
        AeaMarking::Rd(rd) => !rd.sigma.is_empty(),
        AeaMarking::Frd(frd) => !frd.sigma.is_empty(),
        _ => false,
    })
}

fn presence_sar_program_ascending_sort(attrs: &CanonicalAttrs) -> bool {
    attrs
        .sar_markings
        .as_ref()
        .is_some_and(|sar| sar.programs.len() >= 2)
}

fn presence_sci_compartment_numeric_then_alpha(attrs: &CanonicalAttrs) -> bool {
    !attrs.sci_markings.is_empty()
        && attrs
            .sci_markings
            .iter()
            .any(|m| m.compartments.len() >= 2 || m.compartments.iter().any(|c| c.sub_compartments.len() >= 2))
}
```

Each predicate is O(1) or O(n) over the relevant axis with cheap comparisons. The axis-presence early-out is the dominant optimization — when `attrs` has no REL TO / JOINT / AEA / SAR / SCI axis populated, the catalog walk is skipped entirely.

---

## 5. Performance budget

### 5.1 The bench gate

CI enforces baseline+10% on:

- `lint_10kb` (Criterion bench, lint a 10KB document — proxy for SC-001 perceptual instantaneity)
- `decoder_10kb_one_mangled_region` (Criterion bench, lint a 10KB document with one mangled marking — proxy for SC-002 deep-scan latency)

PR 3b.E landed within budget.

### 5.2 Expected delta from PR F

PR F's net change to the walker hot path:

- **Removes** 4 separate `impl Rule for ...` `check()` calls per portion (E020, E023, E028, E033). Each existing rule does its own axis-presence guard internally (e.g., E020 reads `attrs.rel_to.len() >= 2`; E023 iterates `attrs.aea_markings`; etc.).
- **Adds** 1 walker `check()` call per portion. Body: 1 axis-presence early-out (5 axis-presence reads, all O(1) modulo iteration) + (when an axis is present) catalog walk with per-row presence predicate.

Old per-portion cost (4 separate rules): each rule does its own presence guard, possibly iterating the relevant axis once. Total ~4 separate iterations, each with its own early-out logic and helper-fn dispatch.

New per-portion cost (1 walker, 5 rows): 1 axis-presence-any check (5 O(1) reads) + (when ANY axis present) up to 5 row presence-predicate calls (each iterating the row's axis once). Per-portion total: 5 axis presence reads + up to 5 row iterations.

**Expected outcome on prose body text** (the dominant case in a 10KB document): axis-presence-any returns false → catalog walk skipped entirely. **Faster than today** because today's 4 separate rules each do their own presence guards (4 `check()` dispatches each doing one axis-presence read, vs the walker's 1 dispatch + 5 reads — but the dispatch overhead alone makes the walker faster on this dominant case).

**Expected outcome on marking-dense fixtures**: equivalent to today (same number of axis iterations, with the row-presence guard adding small overhead per iteration that is dominated by the per-row evaluator fn-pointer call). The catalog walker pattern adds at most ~50ns per portion of dispatch overhead (6 fn-pointer calls vs 4 direct method calls). For a 10KB document with ~50 portions, that's ~2.5µs total — well within the +10% gate (which on PR 3b.E's `lint_10kb` baseline of ~830µs allows ~83µs of delta).

### 5.3 Sanity-check command

```sh
cargo bench --bench lint_latency -- --save-baseline pr-f-sanity
# compare against pre-PR-F baseline:
cargo bench --bench lint_latency -- --baseline pr-f-sanity
```

Expected delta: within ±5% of pre-PR-F for both `lint_10kb` and `decoder_10kb_one_mangled_region`. **Never propose bumping the baseline** — that's a PM decision.

### 5.4 Risk: rule-evaluation order changes

The four retired rules occupy specific positions in `CapcoRuleSet::new()`'s registration order. Some FR-016 overlap-guard tests assume a specific lex-tiebreaker ordering between rules — the rename `E020 → E060` shifts the lex order at the point where the walker's emitted diagnostic interacts with E002 / E052 / E031 (and others that share spans). See §4.1 `rel_to_invariants.rs` and `banner_rollup_walker_overlap_guard.rs` notes.

**R1 critical correctness check at implementation time**: every overlap-guard interaction involving E020 / E023 / E028 / E033 must be re-verified against the new lex order. If the FR-016 tiebreaker resolves to a different rule than today (e.g., E060 now beats E052 instead of E020 beating E052), the test must update its expected-survivor assertion AND the rule-evaluation registration order in `CapcoRuleSet::new()` must be inspected to confirm that no behavior regression slips through.

---

## 6. Test plan

### 6.1 Test file structure

New file: `crates/capco/tests/non_canonical_input_walker.rs`. Mirror the shape of `crates/capco/tests/sci_per_system_catalog.rs` (the closest 3b.E precedent). Smaller — 5 rows × 3-branch fixtures, plus per-rule behavior ports.

### 6.2 Per-row behavior triplet (15 tests minimum, 3 per row)

For each of the 5 catalog rows:

1. **`test_<row>_fires_on_violation`**: presence-predicate fires AND ordering is wrong → emit non-empty diagnostics, all carrying `Diagnostic.rule = "E060"` AND the diagnostic message identifies the offending invariant (asserts on substring match against the row's authored message phrasing).
2. **`test_<row>_does_not_fire_when_satisfied`**: presence-predicate fires AND ordering is correct → zero diagnostics from E060 for this row.
3. **`test_<row>_does_not_fire_when_marking_absent`**: presence-predicate doesn't fire → zero diagnostics.

### 6.3 Per-row citation-fidelity test

**`test_non_canonical_walker_emits_authored_citations`**: lint a fixture containing all 5 row violations; assert each emitted `Diagnostic.citation` matches one of:

```
["CAPCO-2016 §H.8 p151", "CAPCO-2016 §H.3 p56", "CAPCO-2016 §H.6 p108",
 "CAPCO-2016 §H.5 p99", "CAPCO-2016 §H.4 p61"]
```

(Or substring matches if the existing per-row citations include parenthetical specificity, which they do for E020 and E033 — adapt the assertion to substring contains the page anchor.)

### 6.4 Catalog-pin test

**`test_non_canonical_walker_declares_5_rows`**: pin the catalog row count via a hardcoded `EXPECTED_KINDS: &[&str]` constant in the integration test that reconciles against the `Rule::check` output.

**Test-helper strategy** (per `feedback_pub_doc_hidden_is_still_public_api.md`): the catalog table `NON_CANONICAL_CATALOG` is private to the walker module. The integration test does NOT need direct access to it — the catalog-pin test instead drives the walker through the engine on a fixture that contains exactly one violation per row, asserts that exactly 5 diagnostics fire (one per row), and asserts each diagnostic's message + citation matches the expected set:

```rust
const EXPECTED_KINDS: &[(&str, &str)] = &[
    ("REL TO country codes must be alphabetically ordered", "CAPCO-2016 §H.8 p151"),
    ("JOINT country codes must be alphabetically ordered",  "CAPCO-2016 §H.3 p56"),
    ("SIGMA numbers must be in numerical order",             "CAPCO-2016 §H.6 p108"),
    ("SAR programs must be in ascending order",              "CAPCO-2016 §H.5 p99"),
    ("SCI compartments must be listed in ascending order",   "CAPCO-2016 §H.4 p61"),
];

#[test]
fn test_non_canonical_walker_declares_5_rows() {
    // Lint a fixture containing one violation per row.
    let diags = lint_pinned_fixture(/* see test-fixture helper */);
    let e060: Vec<&Diagnostic> = diags.iter().filter(|d| d.rule.as_str() == "E060").collect();
    assert_eq!(e060.len(), EXPECTED_KINDS.len(),
        "expected {} E060 diagnostics from the pinned catalog fixture, got {}",
        EXPECTED_KINDS.len(), e060.len());
    for (expected_msg, expected_cite) in EXPECTED_KINDS {
        assert!(e060.iter().any(|d| d.message.contains(expected_msg) && d.citation.contains(expected_cite)),
            "expected diagnostic with message containing {expected_msg:?} and citation {expected_cite:?}");
    }
}
```

This pattern uses **zero new public API on the walker** — the catalog stays private, the test uses only the engine's existing public surface (lint a fixture, inspect diagnostics).

### 6.5 Overlap-guard test against E002 (REL TO multi-block fix suppression)

**`test_e060_rel_to_multi_block_suppression_preserves_e002_interaction`**: lint a fixture with multiple REL TO blocks where one block is misordered. Expected: E060's `RelToUsaFirstAlpha` row fires with the multi-block suppression message (no fix); E002 fires with its own behavior. Verbatim port of any existing test exercising this in `rel_to_invariants.rs` with rule-ID rename.

### 6.6 Overlap-guard test against E052 (REL TO no-duplicates)

**`test_e060_e052_overlap_guard_admits_one_winner`**: verbatim port of the existing E020 + E052 overlap-guard test from `rel_to_invariants.rs` lines 76-104. After rename, the FR-016 lex tiebreaker resolves between `E060` and `E052`. Per ASCII lex order: `E052` < `E060` (`'5' < '6'`), so **E052 may now win the tiebreaker** that previously E020 won (`E020` < `E052`). The test's invariant ("exactly one of {E060, E052} survives") holds; the specific winner changes. The test comment is updated to document the new winner; the canonical fixed output is unchanged because both rules' fixes converge on the same canonical REL TO list (sorted unique).

**R1 critical**: confirm at implementation time that the canonical fixed output is byte-identical regardless of which rule wins the tiebreaker. If E052's fix produces a different output than E060's (e.g., E052 dedup-only vs E060 sort-and-dedup), the test must assert the canonical-equivalent invariant is preserved by whichever rule wins. The existing E020 doc comments at lines 4083-4109 already document that E020's fix-output is sorted-and-deduped to maintain single-pass idempotency under the C-1 overlap guard against E052; the walker preserves this verbatim.

### 6.7 JOINT alphabetical correctness test

**`test_e060_joint_pure_alpha_no_usa_first`**: lint a fixture with `//JOINT TOP SECRET CAN ISR USA//REL TO USA, CAN, ISR` (or similar with USA mid-list in JOINT but USA first in REL TO). Expected: zero E060 diagnostics from the JOINT row when USA appears in correct alphabetical position in JOINT (`CAN ISR USA` is correct alphabetical for {CAN, ISR, USA}). E060 fires for JOINT only when the JOINT list is non-alphabetical. Verifies that E060's `JointAlphabetical` row does NOT impose a USA-first carve-out, distinguishing it from the `RelToUsaFirstAlpha` row.

A second test with `//JOINT TOP SECRET ISR CAN USA//...` (out-of-order alphabetical) MUST fire JOINT-row diagnostic; the fix replacement is `CAN ISR USA`.

### 6.8 Parity test — byte-identical NDJSON

**`test_e060_walker_emits_byte_identical_to_retired_rules`**: pin a representative input set in a `tests/fixtures/non_canonical_inputs/` directory (5 fixtures, one per row). Pre-rename, capture the existing E020/E023/E028/E033 NDJSON output for each fixture (saved as `expected.ndjson` next to each fixture). Post-rename, the walker's output for each fixture MUST match the pre-rename `expected.ndjson` byte-for-byte EXCEPT for the `rule` field (`E020`/`E023`/`E028`/`E033` → `E060`).

Implementation: the test loads the `expected.ndjson`, regex-replaces `"rule":"E020"` / `"E023"` / `"E028"` / `"E033"` with `"rule":"E060"` in-memory, then asserts equality against the walker's actual output. If any other field drifts (message text, citation, span, fix replacement, fix confidence), the test fails — telling the implementation agent that byte-identity has slipped.

Five fixtures (one per row) are sufficient for this test; the per-row behavior triplet (§6.2) covers the broader correctness surface.

### 6.9 Corpus-parity rule-count pin update

**Existing**: `crates/capco/tests/corpus_parity.rs:155-176` asserts `rule_set.rules().len() == 50`.

**Updated**: change to `47`. Add a comment block per §4.1.

### 6.10 WASM `parity_corpus.json` verification

The `parity_corpus.json` file (`crates/wasm/tests/parity_corpus.json`) hardcodes `expected_lint` strings that include `"rule":"E028"` and `"rule":"E033"` (3 entries; see §4.1). After rename:

- The `expected_lint` strings are updated to `"rule":"E060"`.
- The diagnostic message text and citation are preserved verbatim (the walker emits byte-identical output to the retired rules — that's the point of preserving message + citation in the per-row evaluator).
- `cargo test -p marque-wasm` re-runs the parity check.

If the implementation agent finds any other field drift (e.g., a span shift due to a helper-fn change), it's a regression — triage immediately.

### 6.11 `Severity::Off` override test

**`test_e060_off_severity_skips_walker`**: configure `.marque.toml [rules] E060 = "off"`, lint a document with all 5 row violations → zero E060 diagnostics. Verifies FR-008.

### 6.12 Audit-stream traceability test

**`test_e060_per_row_message_identifiability`**: lint a fixture containing all 5 row violations; assert each emitted `Diagnostic.message` contains a substring uniquely identifying its row. The `EXPECTED_KINDS` constant from §6.4 doubles as the per-row-message pin. Verifies that an audit-stream consumer grepping for "REL TO ... must be alphabetically ordered" still finds REL TO ordering violations after the rename.

### 6.13 Total test count estimate

- Per-row triplet: 5 rows × 3 = **15**
- Citation fidelity: **1**
- Catalog pin: **1**
- E002 multi-block suppression: **1**
- E052 overlap guard: **1**
- JOINT alpha (no USA-first): **2** (in-order + out-of-order)
- Byte-identical NDJSON parity: **1** (across 5 sub-fixtures)
- `Severity::Off` override: **1**
- Audit-stream traceability: **1**

**Subtotal: 24 new tests in `non_canonical_input_walker.rs`.**

Plus verbatim ports of the existing E020/E023/E028/E033 unit tests from `crates/capco/src/rules.rs` (the embedded `#[cfg(test)] mod tests` block). At implementation time, the agent enumerates these and ports them with the rule-ID assertion updated `E020`/`E023`/`E028`/`E033` → `E060`. Estimated **~30-40 ported tests** based on the volume of existing test coverage for these four rules.

**Grand total: ~50-65 tests** in the new file.

---

## 7. Migration notes — retired rule IDs

Per `feedback_pre_users_no_deprecation_phasing.md`: marque is pre-users; rewrite freely. Retired IDs are permanently retired with no alias maps, no severity-config back-compat, no schema-bump migration shims.

**Retirement policy precedent** (from PR 3b.D and 3b.E):

- 3b.D retired E022 (CNWDI), E025 (UCNI), E027 (SAR-class-floor) and replaced them with the E058 walker. The legacy IDs are removed entirely; the walker's `additional_emitted_ids()` returns `&[]`.
- 3b.E retired E042–E051 (ten SCI per-system rules) and replaced them with the E059 walker. Same policy.

**PR 3b.F policy: same.** E020, E023, E028, E033 are permanently retired. Users with existing `.marque.toml` files keying these IDs MUST migrate to `E060`. The CLAUDE.md "Recent Changes" entry documents the rename so users have a discoverable migration breadcrumb.

**Tests to update with rule-ID rename** (compiled list):

- `crates/capco/tests/rel_to_invariants.rs` — multiple `E020` literals (~14 occurrences); all rename to `E060`. R-1 lex-tiebreaker review with E052 (§4.1).
- `crates/capco/tests/banner_rollup_walker_overlap_guard.rs` — `E028` literals (~5-7 occurrences); rename. R-1 lex-tiebreaker review with E031 (§4.1).
- `crates/capco/tests/banner_rollup_walker.rs` — comment-level `E028` reference at line 94.
- `crates/capco/tests/corpus_parity.rs` — rule-count pin from 50 → 47.
- `crates/wasm/tests/parity_corpus.json` — `"rule":"E028"` (line 146) and `"rule":"E033"` (lines 188, 200) → `"rule":"E060"`.
- `tests/corpus/invalid/sar_bad_program_order.expected.json` — `"rule": "E028"` → `"rule": "E060"`.
- `tests/corpus/invalid/sci_compartment_order.expected.json` — `"rule": "E033"` → `"rule": "E060"`. Update the `_note` field too.
- `tests/corpus/` (workspace root) — grep for any other `E020` / `E023` / `E028` / `E033` references and rename.

**Tests purely deleted (no replacement)**: none. All four retired rules' behavior is preserved verbatim via the walker; every existing test asserting on these rule IDs needs to be either (a) updated for the rename, or (b) ported into the new walker test file.

**Embedded `#[cfg(test)] mod tests` block in `rules.rs`**: each retired rule has an embedded test module. These tests are deleted alongside the rule; their behavior is re-asserted in the new `non_canonical_input_walker.rs` file via the per-row triplet + ports (§6.13).

---

## 8. Reviewer attestation requirements

The PR description MUST declare each of (a)–(c) per `plan.md` D13 addendum:

**(a) Single CAPCO-§ citation per declarative catalog entry.** All 5 PR-F catalog rows carry one verified `CAPCO-2016 §X.Y pNN` citation each. Page anchors verified present in the vendored markdown (see §2.2):

- Row 1 (`RelToUsaFirstAlpha`) → `§H.8 p151` (REL TO USA-first + alpha prose at line 3714)
- Row 2 (`JointAlphabetical`) → `§H.3 p56` (JOINT pure-alpha prose at line 1262)
- Row 3 (`SigmaNumericSort`) → `§H.6 p108` (RD-SIGMA numerical-order prose at line 2652)
- Row 4 (`SarProgramAscendingSort`) → `§H.5 p99` (SAR ascending-sort prose at line 2391)
- Row 5 (`SciCompartmentNumericThenAlpha`) → `§H.4 p61` (SCI compartment + sub-compartment ordering prose at lines 1344-1346)

**(b) Predicate body of every `impl Rule` block has ≤3 internal branches.** The new `DeclarativeNonCanonicalInputRule::check` body has exactly 2 branches: (1) axis-presence early-out, (2) catalog-walk loop. Per-row evaluators are fn-pointer calls inside the loop — they are NOT `impl Rule` blocks; D13 ≤3-branch cap satisfied.

**(c) Net rule delta and running count.**

- Pre-PR-F: 50 `impl Rule` blocks (per PR 3b.E's running count: 59 → 50 after retiring E042–E051 + adding E059 walker).
- PR F delta: 4 retired (`CountryCodeOrderingRule` E020, `SigmaValidationRule` E023, `SarProgramOrderRule` E028, `SciCompartmentOrderRule` E033) + 1 walker added (`DeclarativeNonCanonicalInputRule` E060) = **net −3; running count 50 → 47.**
- No catalog deltas on `CapcoScheme` (the PR-F catalog is private to the walker module — no `Constraint::Custom` rows added; see §1.1 for the architectural rationale).

---

## 9. Open questions / PM resolutions needed

### OQ-1 (P0 — affects catalog row count): JOINT subbranch — split or fold?

**Question.** Should JOINT alphabetical ordering live in a dedicated row (`JointAlphabetical` — kind 2 of a 5-row catalog), or fold under the `RelToUsaFirstAlpha` row (4-row catalog with a multi-page citation `§H.3 p56 + §H.8 p150-151` and an internal classification-is-Joint guard)?

**Tradeoff.**

- **(a) Split (5 rows; recommended).** Each row has one §-citation; cleaner D13 attestation. Cost: one more row + one more presence predicate + one more evaluator; but the JOINT evaluator and the REL TO evaluator already differ structurally (REL TO has multi-block suppression; JOINT does not), so most of the code separation already exists in the existing E020 body. Splitting the row mirrors that natural separation.
- **(b) Fold (4 rows).** One row's citation is multi-page (`§H.3 p56 + §H.8 p150-151`). Cost: weaker D13 attestation phrasing ("each row carries one OR MORE §-citations"). Saves one row's worth of code (~20 LOC).

**Recommendation: (a) split.** Constitution VIII (citation integrity) prefers single, traceable §-citations per claim; D13 codifies "single CAPCO-§ citation per declarative catalog entry"; both are satisfied trivially with the split. The 20 LOC saved by folding is not worth the citation-cleanness regression.

PM resolution requested: ___________

### OQ-2 (P1 — does not block PR scope): Walker rule-ID — E060 or resurrect a retired ID?

**Question.** Should the walker's rule ID be `E060` (next free slot) or one of the retired IDs (e.g., `E020`)?

**Tradeoff.**

- **(a) E060 (next free slot, recommended).** Matches PR 3b.A / 3b.D / 3b.E precedent (each walker took the next free `E###`). Auditors / consumers see a clean rename and a one-time migration; `E020`-or-prior IDs are permanently retired. No alias maps, no back-compat — per `feedback_pre_users_no_deprecation_phasing.md`.
- **(b) Resurrect E020.** Retains the most-historical of the four retiring rule IDs as the walker's ID. **Rejected.** The walker is structurally a different rule (covers REL TO + JOINT + SIGMA + SAR + SCI under one rule, not just REL TO + JOINT). Reusing `E020` would obscure that change and confuse `.marque.toml` consumers who expected `E020` to be REL-TO-or-JOINT-specific. Fundamentally violates the "rewrite freely" pre-users posture by trying to preserve a concept (rule = "country code ordering") that no longer applies.

**Recommendation: (a) E060.** Verified next-free slot.

PM resolution requested: ___________

### OQ-3 (P2 — design choice): Walker default severity

**Question.** What's the walker's `default_severity()`?

**Context.** The four retired rules have heterogeneous defaults: E020/E023/E028 are `Severity::Fix`; E033 is `Severity::Error`. The catalog row stores `severity: Severity` per-row. The walker's `default_severity()` is a coarse-grained override anchor used when `[rules] E060 = ...` engages.

**Tradeoff.**

- **(a) `Severity::Fix` (most-permissive, recommended).** Matches PR 3b.A's banner walker default semantics: "the walker-level default severity is the strictest of the per-row severities so a config that uses [the walker] as the override anchor cannot accidentally weaken any row below its authoring intent." Wait — that's the OPPOSITE rationale. PR 3b.A used `Severity::Error` as walker default because Error is strictest. For PR F, the **strictest** default would be `Severity::Error` (matches E033). The most-permissive would be `Severity::Fix` (matches E020/E023/E028).

  Re-reading PR 3b.A's banner walker (`crates/capco/src/rules.rs:5597-5604`): walker default severity is `Severity::Error` — the strictest of the three rows (which are Fix/Error/Error). The rationale is preserved here: "a config that uses BannerMatchesProjectedRule as the override anchor cannot accidentally weaken any row below its authoring intent."

- **Therefore (b) `Severity::Error` (strictest, matches PR 3b.A precedent).** A user keying `[rules] E060 = "warn"` downgrades all rows to Warn at engine evaluation time; setting `[rules] E060 = "error"` is a no-op (everything already at-or-above Error). Setting `[rules] E060 = "off"` skips the walker entirely (FR-008). Matching the PR 3b.A precedent makes the per-walker semantics consistent across the 3b walker family.

**Recommendation: (b) `Severity::Error`** (matches PR 3b.A banner walker precedent).

PM resolution requested: ___________

### OQ-4 (P2 — known divergence): Per-row severity emission

**Question.** When the walker emits a diagnostic for a row whose `severity` is `Fix` (e.g., row 1 REL TO) but the user has set `[rules] E060 = "warn"`, what severity does the emitted `Diagnostic` carry?

**Resolution.** The engine's severity-override layer runs after rule emission and replaces the emitted `Diagnostic.severity` with the user-configured value. So:

- Default: row 1 emits at `Severity::Fix` (its per-row authored value); row 5 emits at `Severity::Error` (its per-row authored value). Mixed within a single walker invocation.
- `[rules] E060 = "warn"`: every row's emitted severity becomes `Warn` regardless of per-row authoring intent.
- `[rules] E060 = "off"`: walker is skipped entirely; no emission.

This matches PR 3b.A / 3b.D / 3b.E behavior. **No PM decision needed**; documenting for completeness.

### OQ-5 (P0 — quick verify at implementation time): R1 — FR-016 lex-tiebreaker behavior changes

**Question.** Do the rule-ID renames cause behavior regressions in the FR-016 overlap-guard tiebreaker?

**Context.** Existing tests assert specific winners in overlap-guard interactions:

- `rel_to_invariants.rs`: E020 + E052 — current winner is E020 (`E020` < `E052`). After rename, the tiebreaker is `E060` vs `E052` — `E052` < `E060`, so **E052 may now win**.
- `banner_rollup_walker_overlap_guard.rs`: E028 + E031 walker — current order is `E031` first (from the banner walker, no boundary), then E028 second (after the boundary because E028's span ≤ next_window_end). After rename, the order becomes E031 first then E060 second (`E031` < `E060`).

**Resolution.** The implementation agent verifies at implementation time:

1. Re-runs both test files after rename (with rule-ID literals updated `E020`→`E060`, `E028`→`E060`).
2. Confirms the test-stated invariants ("exactly one of {x, y} survives the C-1 overlap guard"; "both fire on non-overlapping spans") still hold.
3. Updates the test's expected-winner assertion if the lex tiebreaker resolves differently.
4. Verifies the canonical fixed output is byte-identical regardless of which rule wins. If E060's fix differs from the retired rule's fix on the same fixture (it shouldn't — the per-row evaluator is a verbatim move), the byte-identical-NDJSON test (§6.8) catches it.

**No PM decision needed at plan time**; documenting as an implementation-time correctness check.

### OQ-6 (P2 — perf-only): Bench baseline review

**Question.** Should the walker's bench gate use PR 3b.E's baseline (`lint_10kb` ~830µs) or a fresh PR 3b.F baseline?

**Resolution.** Use PR 3b.E's baseline; the +10% gate gives 83µs of headroom which is far more than the ~2.5µs of dispatch overhead the walker adds. **No PM decision needed**.

### OQ-7 (P2 — known divergence): No `Constraint::Custom` rows on `CapcoScheme`

**Question.** Confirmed at plan time that PR-F does NOT add `Constraint::Custom` rows on `CapcoScheme` (per §1.1)?

**Resolution.** Confirmed. Trait/validate-path callers (`MarkingScheme::validate`) do not see PR-F's invariants — they fire only inside the engine's rule-evaluation loop. This matches the §3.6 framing: non-canonical-input is an engine-only concern; an external structural-validation surface (e.g., a tooling caller wanting "is this attribute set valid?") doesn't care about token order in the source bytes.

**No PM decision needed**; documenting for completeness.

---

## 10. Workflow timeline

The implementation flow MUST follow:

1. **Fetch latest staging**: `git fetch origin staging` from inside the worktree.
2. **Confirm base commit**: `git log --oneline -1 origin/staging` matches `9d72a7a1` (PR 3b.E merge commit). If staging has advanced (e.g., a hotfix landed), abort and resync the plan.
3. **Dispatch implementation agent** (Opus, foreground, full CAPCO-CONTEXT inline). The agent's prompt includes this plan as its spec, the verified citations from §2.2, the file-delta list from §4.1, the test plan from §6, and the reviewer attestations from §8.
4. **Implementation execution**:
   a. Add the walker module section to `rules_declarative.rs` (catalog table, kinds enum, presence predicates, evaluator fns, walker `impl Rule` block). Use **verbatim moves** of the retired rules' check bodies into the per-row evaluators — preserve byte-identity of message text, citations, fix shapes, and severities.
   b. Delete the four retired rules from `rules.rs` (lines 3065-3187, 4151-4246, 4379-4447, 4989-5161).
   c. Update `CapcoRuleSet::new()`: remove four `Box::new(...)` registrations, add one `Box::new(DeclarativeNonCanonicalInputRule)`.
   d. Update doc comments at the top of `rules.rs` to reflect retirements.
   e. Create `crates/capco/tests/non_canonical_input_walker.rs` with the test plan from §6.
   f. Update `corpus_parity.rs` rule-count pin from 50 → 47 with the comment block.
   g. Update `rel_to_invariants.rs`, `banner_rollup_walker_overlap_guard.rs`, `banner_rollup_walker.rs` rule-ID literals; perform R-1 lex-tiebreaker review per OQ-5.
   h. Update `crates/wasm/tests/parity_corpus.json` (3 entries) and the `tests/corpus/invalid/*.expected.json` files (2 entries) for the rule-ID rename.
   i. Update `crates/capco/README.md` rule-inventory paragraph and rule count.
   j. Update `specs/006-engine-rule-refactor/tasks.md` T026f checkbox.
   k. Update `specs/006-engine-rule-refactor/plan.md` 3b.F status line.
   l. Update `CLAUDE.md` "Recent Changes" section with PR 3b.F entry.
5. **Pre-flight chain** (run BEFORE PR-open):

   ```sh
   cargo check --workspace
   cargo +stable clippy --workspace --all-targets -- -D warnings
   cargo +stable fmt --check
   cargo test --workspace
   wasm-pack build crates/wasm --target web --profile release-web
   cargo doc --workspace --no-deps
   typos .
   ```

   **Note**: `+stable` is required on `clippy` and `fmt --check` per `feedback_clippy_nightly_vs_stable_drift.md` — local nightly clippy 0.1.97 misses lints that CI catches on stable (`clippy::const_is_empty`, etc.). PR 3b.E's commit `26fe98c5` was a CI fix-up specifically because of this.
6. **Reviewer dispatch** (BEFORE PR-open, per `feedback_run_reviewer_before_pr_open.md`): the PM dispatches `rust-reviewer` + `code-reviewer` in parallel (both on Opus). Both review the implementation against this plan + the constitution. Either reviewer's CRITICAL or HIGH finding blocks PR-open until addressed.
7. **GPG-signed commits** (per `feedback_run_reviewer_before_pr_open.md` and Constitution): never `--no-gpg-sign`.
8. **PR-open**: implementation agent opens the PR against `staging` (NOT `main`) with the reviewer-attestation block from §8 and the net-rule-delta math from §8(c) declared in the description. PR title: `refactor-006 PR 3b.F: non-canonical input walker (T026f)`.
9. **Copilot reviews** until merged. Address Copilot findings in fix-pass commits (NOT amend; new commits per the constitution — `feedback_run_reviewer_before_pr_open.md` does not retroactively license amend behavior).
10. **Merge**: PM merges to staging. Updates running rule count narrative globally.

---

## 11. PM resolutions

Resolved by the PM (marque PM coordinator session `8b9d47e0`) on 2026-05-08:

- **OQ-1 (split JOINT into a dedicated row vs fold under REL TO)**: **APPROVED — split.** 5-row catalog. Rationale: Constitution VIII (citation integrity) prefers single, traceable §-citations per claim; D13 codifies "single CAPCO-§ citation per declarative catalog entry"; both are satisfied trivially with the split. The JOINT and REL TO evaluators already differ structurally (REL TO has multi-block suppression; JOINT does not), so the split mirrors natural code separation. The ~20 LOC saved by folding is not worth the citation-cleanness regression.
- **OQ-2 (walker rule ID = E060)**: **APPROVED — E060.** Verified next-free slot at plan-authoring time. Per `feedback_pre_users_no_deprecation_phasing.md`: marque is pre-users; rewrite freely. E020/E023/E028/E033 are permanently retired with no alias, matching the PR 3b.D / 3b.E precedent.
- **OQ-3 (walker default severity = Error vs Fix)**: **APPROVED — `Severity::Error`.** Matches PR 3b.A banner walker precedent (strictest-of-rows). Per-row severity is preserved (`Fix` for rows 1–4, `Error` for row 5); the walker's `default_severity()` only engages when a user keys `[rules] E060 = ...` for a coarse-grained override. **Implementation note**: §2.3 and §4.1 of this plan have been corrected to reflect this resolution; both previously read `Fix`. The resolution stands at `Error`.
- **OQ-4 (per-row severity emission)**: documentation-only; no decision needed.
- **OQ-5 (R1 FR-016 lex-tiebreaker review at implementation time)**: implementation-agent obligation; the agent MUST re-run `rel_to_invariants.rs` and `banner_rollup_walker_overlap_guard.rs` after the rule-ID rename and update test-stated-winner assertions if the lex tiebreaker resolves differently. The byte-identical-NDJSON parity test (§6.8) is the safety net for regression detection.
- **OQ-6 (bench baseline = PR 3b.E baseline)**: documentation-only; no decision needed. Use `lint_10kb` ~830µs and `decoder_10kb_one_mangled_region` baseline carried forward from PR 3b.E.
- **OQ-7 (no `Constraint::Custom` rows on `CapcoScheme`)**: confirmed at plan time; no decision needed.

**Additional PM directives**:

1. The implementation agent MUST receive `crates/capco/CAPCO-CONTEXT.md` verbatim in its prompt (not as a link reference). The PM's mandate covers all CAPCO-rule-touching agents.
2. The implementation agent MUST run rust-reviewer + code-reviewer in parallel (Opus, foreground) BEFORE pushing the PR open. Per `feedback_run_reviewer_before_pr_open.md` — the pre-flight chain is necessary but not sufficient; reviewer dispatch is a hard prerequisite for PR-open.
3. Pre-flight clippy MUST use `cargo +stable clippy --workspace --all-targets -- -D warnings` (NOT bare `cargo clippy`). Per `feedback_clippy_nightly_vs_stable_drift.md` — the local default is nightly clippy 0.1.97 which silently misses lints CI catches on stable.
4. No `#[doc(hidden)] pub fn` for test-only helpers. Per `feedback_pub_doc_hidden_is_still_public_api.md`. Use `pub(crate)` + unit tests OR hardcoded `EXPECTED` constants in integration tests with a citation-fidelity reconciliation test.
5. GPG-signed commits required. Never `--no-gpg-sign`.
6. The R-1 FR-016 lex-tiebreaker concern from OQ-5 is the highest-risk implementation-time correctness check. The agent MUST verify both `rel_to_invariants.rs` and `banner_rollup_walker_overlap_guard.rs` test invariants survive the rule-ID rename, and update test-stated-winner assertions where the lex tiebreaker resolves differently. Canonical fixed output preservation is the C-1 acceptance criterion; per-test winner updates are routine.
7. Plan §2.3 and §4.1 default-severity references have been corrected to `Error` as part of this approval. The implementation agent's spec is the post-correction plan.

PM-approved on: 2026-05-08
PM signature / handle: marque PM coordinator (session `8b9d47e0`)

---

## 12. Acceptance criteria

PR 3b.F is mergeable when:

1. The 5 PR-F catalog rows land as a private `&'static [NonCanonicalRow]` table inside `crates/capco/src/rules_declarative.rs`, with verified `CAPCO-2016 §X.Y pNN` citations per §2.2.
2. `DeclarativeNonCanonicalInputRule` (rule ID `E060`) walks the catalog and emits diagnostics carrying `Diagnostic.rule = "E060"` plus per-row identifying message text + citation matching the retired rule's verbatim wording.
3. The four retired rules (`CountryCodeOrderingRule` E020, `SigmaValidationRule` E023, `SarProgramOrderRule` E028, `SciCompartmentOrderRule` E033) and their corresponding `Box::new(...)` registrations in `CapcoRuleSet::new()` are deleted; the registration is replaced with the single walker entry.
4. The behavior tests from the embedded `#[cfg(test)] mod tests` blocks in `rules.rs` (for E020/E023/E028/E033) are ported into `crates/capco/tests/non_canonical_input_walker.rs` with rule-ID assertions updated `E020`/`E023`/`E028`/`E033` → `E060` and message-text assertions preserved verbatim.
5. The new catalog-shape tests in `tests/non_canonical_input_walker.rs` pass (per-row triplet, citation-fidelity, catalog pin, E002 multi-block suppression, E052 overlap guard, JOINT alpha, byte-identical NDJSON parity, `Severity::Off`, audit-traceability) per §6.
6. `cargo check --workspace`, `cargo +stable clippy --workspace --all-targets -- -D warnings`, `cargo +stable fmt --check`, `cargo test --workspace --no-fail-fast` all pass.
7. `wasm-pack build crates/wasm --target web --profile release-web` builds clean.
8. `cargo doc --workspace --no-deps` builds clean.
9. `typos .` passes.
10. Coverage on the per-row evaluator fns ≥ 80%.
11. Criterion `lint_10kb` and `decoder_10kb_one_mangled_region` benches stay within the existing baseline+10% gate.
12. Corpus parity passes; SC-002 ≥95% per-rule accuracy preserved (E060 inherits the per-rule accuracy from its 5 catalog rows).
13. PR description includes reviewer-attestation (a)–(c) per §8.
14. `crates/capco/README.md` rule-inventory paragraph updated; rule count bumped 50 → 47.
15. `specs/006-engine-rule-refactor/tasks.md` T026f checkbox flipped.
16. `specs/006-engine-rule-refactor/plan.md` 3b.F status line updated.
17. `CLAUDE.md` "Recent Changes" section gets a PR 3b.F entry.
18. CI passes.
19. GPG-signed commits.
20. Reviewer dispatch (`rust-reviewer` + `code-reviewer`) completed BEFORE PR-open per `feedback_run_reviewer_before_pr_open.md`.

---

## 13. Implementation status: PM-APPROVED — ready for implementation dispatch

This plan is the sixth 3b sub-PR plan in the staged-collapse path; the structural template is `2026-05-08-pr3b-E-sci-per-system-collapse-plan.md` adapted for the architectural difference (private walker module vs `Constraint::Custom` catalog).

All three resolution-required open questions are resolved per §11: OQ-1 (split JOINT into row 2; 5-row catalog), OQ-2 (walker rule ID `E060`), OQ-3 (walker `default_severity()` = `Severity::Error`). OQ-4 / OQ-6 / OQ-7 are documentation-only and confirmed; OQ-5 (R-1 FR-016 lex-tiebreaker review) is an implementation-time obligation called out for the implementation agent.

The implementation agent has the full spec needed to execute end-to-end without further ambiguity. Workflow: §10 (timeline) + §11 (PM directives) + §12 (acceptance criteria) define the executable scope.
