<!--
SPDX-FileCopyrightText: 2026 Knitli Inc. <knitli@knitli.com>
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# Implementation Plan: PR 3b.E — Per-SCI-System Constraint Collapse (T026e)

**Target file**: `docs/plans/2026-05-08-pr3b-E-sci-per-system-collapse-plan.md`
**Branch**: `refactor-006-pr-3b-sci-per-system` (worktree `/home/knitli/marque-pr3b-E/`, off `origin/staging` at `c460ac93` — PR 3b.D just merged).
**Base PR**: against `staging` (NOT `main`).
**Predecessors landed in staging**: PR 3b.A (#319 banner walker), PR 3b.B (#320 transmutations), PR 3b.C (#321 RELIDO conflicts), PR 3b.D (#324 class-floor catalog).
**Status**: PM review pending. Plan-only round; no code changes attempted.

---

## 0. One-paragraph summary

PR 3b.E retires the 10 hand-written SCI per-system rules in `crates/capco/src/rules_sci_per_system.rs` (E042–E051) into a single declarative walker `DeclarativeSciPerSystemRule` (rule ID `E059`, the next free `E###` after PR 3b.D took `E058`) backed by a per-system catalog of `Constraint::Custom("sci-per-system/...", ...)` rows on `CapcoScheme`. The catalog covers exactly the §H.4 invariants that PR 3b.D's class-floor catalog does **not** already cover: companion-required (ORCON, NOFORN), forbid-companion (ORCON-USGOV), and one no-fix-Warn ambiguity row. Every floor-checking function in E042–E051 is already covered by a specific PR-D `class-floor/...` row — those subsets retire with **no replacement** in PR E (zero new rows for floor-only behavior; emitting a duplicate floor row would double-fire). Net rule delta: **10 retired (E042–E051) + 1 walker added (E059) = net −9; running rule count 53 → 44.** Catalog deltas: **+5 `Constraint::Custom` rows on `CapcoScheme`** at §H.4 family granularity (HCS-O companions, HCS-P NOFORN, HCS-P sub-companions, SI-G companions, TK-{BLFH,IDIT,KAND} NOFORN). The HCS classification "ceiling" Warn-no-fix surface (the ambiguous TS-or-S below-S case for HCS-O / bare HCS-P, RSV, and TK family) is intentionally **not** carried into PR E — PR D's class-floor `Error`-severity diagnostic already fires on the same below-floor input and supersedes the per-system Warn diagnostic; ambiguity guidance moves to the diagnostic message text on PR D's `class-floor/HCS-comp` / `class-floor/RSV-comp` / `class-floor/TK` rows in a follow-up PR (out of scope here, see §8 open question). The walker stays within Constitution VII §IV (no engine edits), reuses PR D's three-layer hot-path optimization (axis-presence early-out, direct row dispatch, DRY emit helper), and preserves the audit-stream traceability invariant via per-row diagnostic message text + `ConstraintViolation.constraint_label`.

---

## 1. Architectural option (LOCKED — `Constraint::Custom` rows + walker, mirroring PR 3b.D)

**Reasoning anchored in Constitution VII §IV and the PR 3b.D pattern.** Scheme-adoption PRs MUST NOT edit `crates/engine`, `crates/scheme`, `crates/core`, `crates/rules`, `crates/ism`. Adding a new `Constraint` enum variant for "companion required" / "forbid companion" would touch `crates/scheme/src/constraint.rs`, violating this. The same logic that PR 3b.D used to land class-floor invariants under `Constraint::Custom` applies here: the §H.4 per-system invariants are domain-specific predicates that don't map onto a generic existing primitive (`Requires` / `Conflicts` / `Implies` / `Supersedes`), so `Constraint::Custom` is the existing-primitive choice authorized by the consultation verdict §1 ("PR 3b ships the declarative-catalog moves over existing primitives only").

PR D's implementation shape is reused verbatim for the data model and the walker:

- **`Constraint::Custom { name, label }` rows** with per-row dispatch in `evaluate_custom_by_attrs` (in `crates/capco/src/scheme.rs`). Catalog table is `&'static [SciPerSystemRow]` paralleling `&'static [ClassFloorRow]`.
- **One walker `impl Rule for DeclarativeSciPerSystemRule`** with `Rule::id() = "E059"` (next free slot after PR D's E058 — verified against the rule-ID inventory grep, see §8 OQ-3).
- **Per-row prefix in the `name` field** — every catalog row's `name` starts with `sci-per-system/` for O(1) prefix dispatch (`is_sci_per_system_catalog_name`). Mirrors PR D's `class-floor/` prefix invariant.
- **DRY emit helper** `sci_per_system_emit(attrs, ctx, &row) -> Vec<Diagnostic>` shared by the walker hot path (`sci_per_system_eval_row`) and the trait/validate path (`sci_per_system_catalog_eval`). One source of truth for citation, message-text, fix-shape, and severity per row.
- **Axis-presence early-out**: `if attrs.sci_markings.is_empty() { return Vec::new(); }` at the head of the walker. SCI per-system rules are uniformly SCI-axis-only — no AEA, SAR, dissem, or NATO-class axis covered by this walker — so a single boolean is sufficient (no axis bitfield needed; simpler than PR D's 5-axis enum).

**Forward-link comment in `scheme.rs`** next to the `sci-per-system` catalog block:

```text
SCI per-system catalog rows are declared as Constraint::Custom because
the §H.4 invariants are companion-presence (ORCON, NOFORN) + companion-
forbid (ORCON-USGOV) + per-row fix-shape (zero-width insertion at the
end of the IC dissem block, or a span replacement on the dominated
token) — none of which fit the existing primitive surface. PR 4 (per-
category Lattice impls per Stage 3 of plan.md:263) MAY revisit and
re-classify to a `CompanionRequired<Set>` / `Forbid<Set>` primitive on
`marque-scheme` when those primitives land. The walker stays until
that retirement. See docs/plans/2026-05-08-pr3b-E-sci-per-system-
collapse-plan.md §3 for the rule-by-rule analysis; tasks.md T026e for
the walker landing.
```

This makes the choice auditable and the migration path discoverable for the next refactor agent.

**Severity convention.** Every catalog row defaults to `Severity::Warn` (matches the existing E042–E051 fix-and-warn pattern per the `rules_sci_per_system.rs` module-header rationale). Companion-insertion rows escalate per row to `Severity::Error` no-fix when the portion has no IC dissem block (no anchor for `//`-block insertion). This per-row escalation is encoded in the row's emit body, not the row's static `severity` field; the row stores the *non-escalated* severity (`Warn`) and the emit helper produces an `Error`-severity diagnostic with `fix: None` when the no-IC-dissem branch fires.

---

## 2. The verified §H.4 catalog at family granularity (5 rows)

### 2.1 Verification methodology

Every row's citation must satisfy two conditions per Constitution Principle VIII:
1. The marking-body section in `crates/capco/docs/CAPCO-2016.md` clearly contains the cited invariant language ("Requires ORCON and NOFORN", "May not be used with ORCON-USGOV", etc.) at the cited page anchor.
2. The cited page exists in the manual (the `begin page NNN` anchor is present in the markdown).

PR D's PR R1 lesson applies: when a citation in `marque-applied.md` cites an operative-authority page rather than a marking-template-body page, the `marque-applied.md` citation is accepted as authoritative — but for PR E, every row already cites a §H.4 marking-template-body page directly, so no operative-authority remapping is needed. The existing rule sources (`rules_sci_per_system.rs`) already cite `§H.4 p64` (HCS-O), `§H.4 p66` (HCS-P), `§H.4 p68` (HCS-P [SUB]), `§H.4 p80` (SI-G), `§H.4 p87 + p91 + p95` (TK-{BLFH,IDIT,KAND}). All page anchors verified present in the vendored markdown:

```
begin page 64  → line 1413  (HCS-O body)
begin page 66  → line 1468  (HCS-P body)
begin page 68  → line 1524  (HCS-P [SUB] body)
begin page 80  → line 1839  (SI-G body)
begin page 87  → line 2043  (TK-BLFH body)
begin page 91  → line 2158  (TK-IDIT body)
begin page 95  → line 2272  (TK-KAND body)
```

### 2.2 The 5 catalog rows

| # | §H.4 Row Y citation | Family pattern | Invariant kind | Fix proposal shape | Default severity |
|---|---|---|---|---|---|
| 1 | `CAPCO-2016 §H.4 p64` (HCS-O) | HCS-anchored marking with compartment "O" | companion-required (ORCON + NOFORN); forbid-companion (ORCON-USGOV) | (a) missing-ORCON: zero-width insertion `/OC` (or `/ORCON`) at end of last dissem token; (b) missing-NOFORN: zero-width insertion `/NF` (or `/NOFORN`); (c) ORCON-USGOV present: span replacement of `OC-USGOV` token → `OC` (or `ORCON`). Companion-insertion fixes carry `confidence = 0.9`; replacement fix carries `confidence = 0.9`. **Error no-fix fallback** when the portion has no IC dissem block at all (companion-insertion would need to synthesize a whole `//`-separated category from rule context, which is unsafe; same policy as E040). | `Severity::Warn` (companion-insertion + replacement); `Severity::Error` no-fix when no dissem block exists. |
| 2 | `CAPCO-2016 §H.4 p66` (HCS-P) | HCS-anchored marking with compartment "P" (with or without sub-compartment) | companion-required (NOFORN) | Missing-NOFORN: zero-width insertion `/NF` (or `/NOFORN`) at end of last dissem token. `confidence = 0.9`. **Error no-fix fallback** when no IC dissem block exists. | `Severity::Warn`; `Severity::Error` no-fix when no dissem block exists. |
| 3 | `CAPCO-2016 §H.4 p68` (HCS-P [SUB]) | HCS-anchored marking with at least one HCS compartment carrying ≥1 sub-compartment | companion-required (ORCON); forbid-companion (ORCON-USGOV) | (a) missing-ORCON: zero-width insertion `/OC` (or `/ORCON`); (b) ORCON-USGOV present: span replacement → `OC` (or `ORCON`). NOFORN requirement is **NOT duplicated here** — Row 2 already covers HCS-P (with or without sub-compartment) and emits the NOFORN requirement, so HCS-P [SUB] inherits it. **Error no-fix fallback** for the ORCON insertion when no dissem block exists. | `Severity::Warn`; `Severity::Error` no-fix when no dissem block exists. |
| 4 | `CAPCO-2016 §H.4 p80` (SI-G) | SI-anchored marking with compartment "G" (with or without sub-compartments) | companion-required (ORCON); forbid-companion (ORCON-USGOV) | (a) missing-ORCON: zero-width insertion `/OC` (or `/ORCON`); (b) ORCON-USGOV present: span replacement → `OC` (or `ORCON`). **Error no-fix fallback** for the ORCON insertion when no dissem block exists. | `Severity::Warn`; `Severity::Error` no-fix when no dissem block exists. |
| 5 | `CAPCO-2016 §H.4 p87 (TK-BLFH) + p91 (TK-IDIT) + p95 (TK-KAND)` | TK-anchored marking carrying any of the compartments BLFH, IDIT, or KAND (with or without sub-compartments) | companion-required (NOFORN) | Missing-NOFORN: zero-width insertion `/NF` (or `/NOFORN`). `confidence = 0.9`. **Error no-fix fallback** when no IC dissem block exists. | `Severity::Warn`; `Severity::Error` no-fix when no dissem block exists. |

**Note on companion-form (`OC` vs `ORCON`, `NF` vs `NOFORN`)**: the existing `infer_companion_form` helper (see `rules_sci_per_system.rs:168-179`) inspects the first dissem token's text to decide whether the portion is using abbreviated or full form. Implementation moves this helper into `crates/capco/src/scheme.rs` (or `rules_declarative.rs`) since `rules_sci_per_system.rs` is being deleted; behavior is preserved verbatim.

### 2.3 Why only 5 rows (not 10)

Of the 10 §H.4 invariants enumerated in `rules_sci_per_system.rs`, **the floor-only invariants are already covered by PR 3b.D's class-floor catalog** and don't need PR-E catalog rows. See §3 for the per-rule retirement analysis.

| §H.4 invariant kind | Where it lives after PR E |
|---|---|
| Class-floor (TS-only or TS-or-S) | PR D `class-floor/...` rows (already shipped) |
| Companion-required (ORCON / NOFORN) | PR E `sci-per-system/...` rows (this plan, 5 rows) |
| Forbid-companion (ORCON-USGOV) | Folded into PR E rows that also handle ORCON requirement |
| Range-ceiling Warn-no-fix ambiguity | Retired entirely (PR D's class-floor diagnostic already covers below-floor cases at Error severity; ambiguity message moves to the class-floor diagnostic in a follow-up PR — see §8 OQ-2) |

---

## 3. Rule-by-rule retire-vs-reroute analysis

For each retired rule, the classification is one of:
- **(a) Retire entirely with NO PR-E catalog row** — rule's only function is class-floor checking already covered by PR D.
- **(b) Convert to a PR-E per-system catalog row** — rule covers companion-insertion or forbid-companion not covered by PR D.
- **(c) Mixed** — some functions retire with no replacement, others become PR-E rows.

### 3.1 E042 `HcsOCompanionsRule` — Classification (b)

**Predicate body**: HCS-O present ⇒ ORCON required + NOFORN required + ORCON-USGOV forbidden.
**Fix shape**: 3 emit branches (insert ORCON, insert NOFORN, replace OC-USGOV → OC).
**Severity**: `Warn`; `Error` no-fix when no IC dissem block exists.
**Scope guard**: `us_level(attrs).is_none()` → skip (§H.4 is US-only-scoped).
**Citation**: `CAPCO-2016 §H.4 p64`.

**Floor-checking content**: None. E042 does NOT include a class-floor check (the HCS-O TS-or-S floor is covered by E045 separately; E042 only handles companions).

**Decision**: **(b)** — becomes PR-E catalog row #1 (HCS-O companions). Implementation reproduces all 3 emit branches in the row's emit body. Severity escalation to `Error`-no-fix preserved.

### 3.2 E043 `HcsPRequiresNofornRule` — Classification (b)

**Predicate body**: HCS-P present ⇒ NOFORN required.
**Fix shape**: 1 emit branch (insert NOFORN).
**Severity**: `Warn`; `Error` no-fix when no IC dissem block exists.
**Scope guard**: `us_level(attrs).is_none()` → skip.
**Citation**: `CAPCO-2016 §H.4 p66`.

**Floor-checking content**: None.

**Decision**: **(b)** — becomes PR-E catalog row #2 (HCS-P NOFORN).

### 3.3 E044 `HcsPSubcompartmentTsOnlyRule` — Classification (c)

**Predicate body**: HCS-P with sub-compartment ⇒ TS classification + ORCON required + ORCON-USGOV forbidden.
**Fix shape**: 3 emit branches (class upgrade to TS via `build_class_upgrade_fix`, insert ORCON, replace OC-USGOV → OC).
**Severity**: `Warn`; `Error` no-fix when no IC dissem block (ORCON insertion only — class-upgrade always has the classification token to rewrite).
**Scope guard**: `us_level(attrs).is_none()` → skip.
**Citation**: `CAPCO-2016 §H.4 p68`.

**Floor-checking content**: The class-upgrade-to-TS branch.

**Decision**: **(c) Mixed**.
- **Class-floor portion** retires with **no PR-E replacement**. PR D's `class-floor/HCS-comp-sub` row (in `scheme.rs:3253-3263`, presence `presence_hcs_comp_sub` = "any HCS-anchored marking carrying a compartment that has at least one sub-compartment", policy `AtLeast(TopSecret)`, severity `Error`, citation `CAPCO-2016 §H.4`) covers exactly the HCS-P-sub TS-only floor. Reviewer attestation (d): `class-floor/HCS-comp-sub` row.
- **Companion portion** (ORCON required, ORCON-USGOV forbidden) becomes PR-E catalog row #3 (HCS-P sub-companions). NOFORN requirement is NOT duplicated in row #3 — row #2 already covers HCS-P (with or without sub-compartment).
- **Note on the lost class-upgrade fix**: the existing E044 carries an actionable `build_class_upgrade_fix` that auto-upgrades `S` → `TS`. PR D's `class-floor/HCS-comp-sub` row currently emits a no-fix Error (PR D §4.2: "the per-system rule carries the fix; the catalog row carries the §-cited descriptor. PR 3b.E (T026e) retires the per-system rules and the catalog row becomes the sole emitter"). After PR E, the actionable class-upgrade fix is **lost** — the user receives the class-floor Error diagnostic but no automated fix. **This is intentional per PR D §4.2 PM decision** (class promotion is FixIntent-territory under PR 3c; the fix returns when FixIntent lands). Documented in §8 OQ-1.

### 3.4 E045 `HcsClassificationCeilingRule` — Classification (a)

**Predicate body**: HCS-O or bare HCS-P with sub-S classification ⇒ Warn-no-fix "ambiguous resolution".
**Fix shape**: None (no-fix Warn).
**Severity**: `Warn`.
**Scope guard**: pre-empts HCS-P-sub (E044 covers it with actionable upgrade); skips when `us_level(attrs).is_none()`; skips when `level >= Secret`.
**Citation**: `CAPCO-2016 §H.4 p64 + p66`.

**Floor-checking content**: This rule IS purely floor-checking (TS-or-S floor for HCS-O / bare HCS-P). The Warn-no-fix design exists because the resolution is ambiguous (upgrade to S? to TS? remove the HCS marking?).

**Decision**: **(a) Retire entirely with no PR-E catalog row**. PR D's `class-floor/HCS-comp` row (in `scheme.rs:3318-3327`, presence `presence_hcs_comp_only` = "HCS-anchored marking with compartment but no sub-compartment, excluding HCS-X", policy `AtLeast(Secret)`, severity `Error`, citation `CAPCO-2016 §H.4`) covers exactly the same scope. PR D fires at `Error`, not `Warn` — this is a **severity escalation** from the existing E045. The ambiguity-guidance message ("upgrade the classification or remove the HCS marking") that E045 carries is **lost** unless we move it onto PR D's `class-floor/HCS-comp` row's diagnostic message, which is out of scope for PR E (a 1-PR-D-row text edit cannot land via PR E without violating the "one architectural shape per PR" discipline; tracked as §8 OQ-2 follow-up). Reviewer attestation (d): `class-floor/HCS-comp` row.

### 3.5 E046 `SiCompartmentTopSecretRule` — Classification (a)

**Predicate body**: SI compartment present (any SI-G, SI-ECRU, SI-NONBOOK, etc.) with sub-TS classification ⇒ class upgrade to TS.
**Fix shape**: 1 emit branch (class upgrade via `build_class_upgrade_fix`).
**Severity**: `Warn`.
**Scope guard**: skips when `us_level(attrs).is_none()`; skips when `level >= TopSecret`.
**Citation**: `CAPCO-2016 §H.4 p76 + p80 + p81`.

**Floor-checking content**: Entirely a class-floor check (with an actionable upgrade fix).

**Decision**: **(a) Retire entirely with no PR-E catalog row**. PR D's `class-floor/SI-comp` row (in `scheme.rs:3264-3274`, presence `presence_si_comp` = "any SI-anchored marking with non-empty compartments", policy `AtLeast(TopSecret)`, severity `Error`, citation `CAPCO-2016 §H.4`) covers exactly this. The actionable class-upgrade fix is lost; same caveat as E044 (§3.3, §8 OQ-1). Reviewer attestation (d): `class-floor/SI-comp` row.

### 3.6 E047 `SiGammaCompanionsRule` — Classification (b)

**Predicate body**: SI-G present ⇒ ORCON required + ORCON-USGOV forbidden.
**Fix shape**: 2 emit branches (insert ORCON, replace OC-USGOV → OC).
**Severity**: `Warn`; `Error` no-fix when no IC dissem block.
**Scope guard**: `us_level(attrs).is_none()` → skip.
**Citation**: `CAPCO-2016 §H.4 p80 + p81`.

**Floor-checking content**: None. The TS-only floor for SI compartments is covered by E046 (which retires under §3.5).

**Decision**: **(b)** — becomes PR-E catalog row #4 (SI-G companions). Note: SI requires SI presence as a companion (per §H.4 p80), but since the parser models SI-G as an SI marking with compartment "G", SI presence is automatic and the existing E047 doesn't separately check it; the PR-E row preserves this behavior.

### 3.7 E048 `RsvClassificationCeilingRule` — Classification (a)

**Predicate body**: RSV present (with or without compartment) with sub-S classification ⇒ Warn-no-fix.
**Fix shape**: None.
**Severity**: `Warn`.
**Scope guard**: skips when `us_level(attrs).is_none()`; skips when `level >= Secret`.
**Citation**: `CAPCO-2016 §H.4 p70 + p72`.

**Floor-checking content**: Entirely floor-checking.

**Decision**: **(a) Retire entirely with no PR-E catalog row**. PR D's `class-floor/RSV-comp` row (in `scheme.rs:3328-3337`, presence `presence_rsv_comp` = "any RSV-anchored marking with compartment", policy `AtLeast(Secret)`, severity `Error`, citation `CAPCO-2016 §H.4`) covers this. Note: §H.4 p70 / p72 of the manual explicitly state "RSV is not used alone; requires compartment" (line 1607: "the RSV marking may not be used alone and requires the associated compartment") — so "bare RSV without compartment" is an invalid SCI structural form caught by other rules, not an §H.4 floor concern. PR D's `presence_rsv_comp` covers the entire valid §H.4 RSV surface. Severity escalates from `Warn` to `Error`; ambiguity-message lost (same caveat as E045, §8 OQ-2). Reviewer attestation (d): `class-floor/RSV-comp` row.

### 3.8 E049 `TkClassificationCeilingRule` — Classification (a)

**Predicate body**: TK family (bare, TK-IDIT, TK-KAND — explicitly excludes TK-BLFH which E050 covers) with sub-S classification ⇒ Warn-no-fix.
**Fix shape**: None.
**Severity**: `Warn`.
**Scope guard**: skips when `has_tk_non_blfh` is false; skips when `us_level(attrs).is_none()`; skips when `level >= Secret`.
**Citation**: `CAPCO-2016 §H.4 p85`.

**Floor-checking content**: Entirely floor-checking.

**Decision**: **(a) Retire entirely with no PR-E catalog row**. PR D's `class-floor/TK` row (in `scheme.rs:3339-3349`, presence `presence_tk_family` = "TK-anchored marking, excluding any with BLFH compartment", policy `AtLeast(Secret)`, severity `Error`, citation `CAPCO-2016 §H.4`) covers this exactly — the BLFH exclusion is mirrored. Severity escalates from `Warn` to `Error`; ambiguity-message lost. Reviewer attestation (d): `class-floor/TK` row.

### 3.9 E050 `TkBlfhTopSecretRule` — Classification (a)

**Predicate body**: TK-BLFH (with or without sub-compartments) with sub-TS classification ⇒ class upgrade to TS.
**Fix shape**: 1 emit branch (class upgrade via `build_class_upgrade_fix`).
**Severity**: `Warn`.
**Scope guard**: skips when `us_level(attrs).is_none()`; skips when `level >= TopSecret`.
**Citation**: `CAPCO-2016 §H.4 p87`.

**Floor-checking content**: Entirely floor-checking (with actionable upgrade fix).

**Decision**: **(a) Retire entirely with no PR-E catalog row**. PR D's `class-floor/TK-BLFH` row (in `scheme.rs:3275-3285`, presence `presence_tk_blfh` = "any TK-anchored marking carrying a BLFH compartment", policy `AtLeast(TopSecret)`, severity `Error`, citation `CAPCO-2016 §H.4`) covers this. Actionable class-upgrade fix is lost; same caveat as E044/E046 (§8 OQ-1). Reviewer attestation (d): `class-floor/TK-BLFH` row.

### 3.10 E051 `TkCompartmentRequiresNofornRule` — Classification (b)

**Predicate body**: TK-{BLFH, IDIT, KAND} present (with or without sub-compartments) ⇒ NOFORN required.
**Fix shape**: 1 emit branch (insert NOFORN).
**Severity**: `Warn`; `Error` no-fix when no IC dissem block.
**Scope guard**: `us_level(attrs).is_none()` → skip.
**Citation**: `CAPCO-2016 §H.4 p87 + p91 + p95`.

**Floor-checking content**: None. (The class floors for TK-BLFH (TS-only) and TK family (TS-or-S) are E050/E049's territory, both retiring under §3.8/§3.9.)

**Decision**: **(b)** — becomes PR-E catalog row #5 (TK compartment NOFORN).

### 3.11 Summary table

| Rule | Classification | Fate | PR-E catalog row | PR-D class-floor row covering retired floor portion |
|---|---|---|---|---|
| E042 HcsOCompanions | (b) | Convert | Row #1 (HCS-O companions) | — (E042 has no floor portion) |
| E043 HcsPRequiresNoforn | (b) | Convert | Row #2 (HCS-P NOFORN) | — |
| E044 HcsPSubcompartmentTsOnly | (c) Mixed | Floor retires (no row); companions convert | Row #3 (HCS-P sub-companions) | `class-floor/HCS-comp-sub` |
| E045 HcsClassificationCeiling | (a) | Retire entirely | — | `class-floor/HCS-comp` |
| E046 SiCompartmentTopSecret | (a) | Retire entirely | — | `class-floor/SI-comp` |
| E047 SiGammaCompanions | (b) | Convert | Row #4 (SI-G companions) | — |
| E048 RsvClassificationCeiling | (a) | Retire entirely | — | `class-floor/RSV-comp` |
| E049 TkClassificationCeiling | (a) | Retire entirely | — | `class-floor/TK` |
| E050 TkBlfhTopSecret | (a) | Retire entirely | — | `class-floor/TK-BLFH` |
| E051 TkCompartmentRequiresNoforn | (b) | Convert | Row #5 (TK compartment NOFORN) | — |

**Tally**: 5 retire-with-PR-E-row (b) + 4 retire-entirely (a) + 1 mixed (c) = 10 rules retired; 5 PR-E catalog rows added; 1 walker added; 0 floor rows added (PR D covers all). Net delta confirmed: 10 retired + 1 walker = net −9 `impl Rule` blocks.

---

## 4. Implementation outline

### 4.1 Files touched

**`crates/capco/src/scheme.rs`** (the largest changes — analogous to PR D's diff):

- **Add** the SCI per-system catalog under a new section header `// PR 3b.E (T026e) — SCI per-system catalog (§H.4)`:
  - `SciPerSystemRow` struct with fields: `name: &'static str` (must start with `sci-per-system/`), `marking_label: &'static str`, `presence: fn(&CanonicalAttrs) -> bool`, `kind: SciPerSystemKind`, `severity: Severity` (the non-escalated default), `citation: &'static str`, `primary_kind: Option<TokenKind>` (for span anchoring; uniformly `Some(TokenKind::SciSystem)` for the 5 rows).
  - `SciPerSystemKind` enum — keep ≤ 3 variants per Reviewer Attestation (b):
    - `CompanionRequired { dissem: DissemControl, banner_form: &'static str, abbrev_form: &'static str }` — for rows that just need NOFORN inserted (rows #2, #5). Row stores the dissem control to require; emit body inspects `attrs.dissem_controls`, emits insertion if absent.
    - `Custom(fn(&CanonicalAttrs, &RuleContext) -> Vec<Diagnostic>)` — for rows that need multi-branch emit logic (rows #1, #3, #4). The closure captures the row's static citation/severity context and produces the emit list directly. Justification for using `Custom` instead of a richer `MultiPart` variant: rows #1 / #3 / #4 each have 2-3 emit branches with row-specific text and span logic; encoding that uniformly in a static struct adds more LOC than just embedding the closure. Branch count constraint (≤3) is satisfied because the `match row.kind` body has exactly 2 arms.
    - **Decision**: pick **2 variants** (CompanionRequired + Custom) to stay under the ≤3-branch attestation. The 3 multi-branch rows (#1, #3, #4) all use Custom; the 2 single-branch rows (#2, #5) use CompanionRequired. (See §8 OQ-4 for the alternative all-Custom design.)
- **Add** family-presence predicates (5 functions): `presence_hcs_o`, `presence_hcs_p_any` (covers row #2 — HCS-P with or without sub), `presence_hcs_p_sub`, `presence_si_g`, `presence_tk_compartment_noforn` (TK with BLFH/IDIT/KAND). Note: `presence_hcs_p_sub` already exists as the call site for PR-D's `class-floor/HCS-comp-sub` row uses `presence_hcs_comp_sub` which is the *family*-level "any HCS with sub-compartment" predicate; PR-E needs a P-specific version. **Verify reuse**: if `presence_hcs_comp_sub` is exactly equivalent to "HCS-anchored marking with at least one sub-compartmented compartment", and the only HCS compartment that can carry sub-compartments per §H.4 is P (the manual lists no other HCS sub-compartmented variants), then `presence_hcs_p_sub` is a strict subset that is in practice identical. Choose: **reuse `presence_hcs_comp_sub`** for row #3 to avoid drift between rows; the §H.4 grammar restricts sub-compartments to HCS-P at the manual level, so the predicates coincide. Document the reuse decision in the row's static comment.
- **Add** dispatch helpers:
  - `is_sci_per_system_catalog_name(name) -> bool` — O(1) prefix check (`name.starts_with("sci-per-system/")`).
  - `sci_per_system_row_by_name(name) -> Option<&'static SciPerSystemRow>` — linear scan (5-row catalog → ≪1 µs).
  - `sci_per_system_catalog() -> &'static [SciPerSystemRow]` — full-catalog iterator for the walker.
  - `sci_per_system_emit(attrs, ctx, row) -> Vec<Diagnostic>` — single source of truth, mirrors `class_floor_emit`. Owns the Warn-vs-Error severity escalation logic (consults `last_dissem_span(attrs)` to decide; `None` → Error no-fix).
  - `sci_per_system_eval_row(attrs, ctx, row) -> Vec<Diagnostic>` — walker hot-path entry; thin wrapper around `sci_per_system_emit`.
  - `sci_per_system_catalog_eval(attrs, ctx, name) -> Vec<ConstraintViolation>` — trait/validate-path entry; resolves row by name then forwards to a `ConstraintViolation`-producing variant of `sci_per_system_emit`. **Note**: `ConstraintViolation` doesn't carry a fix — but PR-E rows produce fixes. The trait/validate path drops the fix; only the walker path emits it. Document this divergence; see §8 OQ-5.
- **Move** the helpers from `rules_sci_per_system.rs` (which is being deleted): `anchors_on`, `has_compartment`, `compartment_has_sub`, `is_tk_noforn_compartment`, `first_sci_span`, `us_level`, `classification_token`, `last_dissem_span`, `dissem_token_span`, `infer_companion_form`, `CompanionForm` enum, `emit_companion_insert`. Two destination options:
  - **Option A** (preferred): move them into `scheme.rs` as `pub(crate)` items in a new module-internal section "SCI per-system helpers". Keeps the catalog and helpers co-located.
  - **Option B**: move them into `rules_declarative.rs` next to the walker. Less locality with the catalog.
  Choose **Option A**.
- **Add** `Constraint::Custom { name, label }` declarations (5 entries) to `CapcoScheme::build_constraints` under a new `// PR 3b.E (T026e) — SCI per-system catalog (§H.4)` section header. Each row's `name` matches the catalog row's `name`; the `label` is the §-citation.
- **Update** `evaluate_custom_by_attrs` to dispatch into `sci_per_system_catalog_eval` when `is_sci_per_system_catalog_name(name)` returns true (mirrors PR D's `is_class_floor_catalog_name` dispatch line):
  ```rust
  fn evaluate_custom_by_attrs(attrs: &CanonicalAttrs, name: &'static str) -> Vec<ConstraintViolation> {
      if is_class_floor_catalog_name(name) {
          return class_floor_catalog_eval(attrs, name);
      }
      if is_sci_per_system_catalog_name(name) {
          return sci_per_system_catalog_eval(attrs, name);  // NEW
      }
      match name {
          // existing 9 entries unchanged (E010, E012, E014, E021, E024, W002, capco/joint-requires-usa, E038)
      }
  }
  ```
  **Verify**: `evaluate_custom_by_attrs` does not currently take a `&RuleContext` parameter; the trait/validate path runs without `RuleContext`. The walker path has it. Implication: the trait/validate path of PR-E catalog rows cannot emit class-upgrade-style fixes (fine — none of the 5 PR-E rows need `RuleContext` for fix construction; companion-insertion fix anchors at `last_dissem_span` regardless of marking type, since both portion and banner have IC dissem blocks at the same structural anchor). Document in the trait/validate-path branch.

**`crates/capco/src/rules_declarative.rs`**:

- **Add** new section `// PR 3b.E (T026e) — SCI per-system catalog walker (E059)`:
  - `pub(crate) struct DeclarativeSciPerSystemRule;` with `impl Rule`. `id() = RuleId::new("E059")`. `name() = "sci-per-system-catalog"`. `default_severity() = Severity::Warn` (matches the majority of catalog row defaults; per-row severity overrides via the engine's severity-override layer apply at walker level, identical to PR D's E058 design).
  - `check(&self, attrs, ctx) -> Vec<Diagnostic>`:
    - **Axis-presence early-out**: `if attrs.sci_markings.is_empty() { return Vec::new(); }` — single boolean, simpler than PR D's 5-axis enum because all PR-E rows are SCI-axis-only.
    - **Direct row dispatch**: walk `crate::scheme::sci_per_system_catalog()`; for each row whose `(row.presence)(attrs)` fires, call `crate::scheme::sci_per_system_eval_row(attrs, ctx, row)` and append its diagnostics to the output. Each appended diagnostic carries `Diagnostic.rule = "E059"` and `Diagnostic.severity = row.severity` (or the escalated `Error` when the no-IC-dissem branch fires).

**`crates/capco/src/rules.rs`**:

- **Remove** the 10 `Box::new(crate::rules_sci_per_system::...)` registration entries (lines ≈216–225).
- **Update** the surrounding doc comment that says "T035d: per-SCI-system constraint rules (E042–E051) implementing §H.4 class-ceiling and required-companion constraints under the fix-and-warn pattern. See `rules_sci_per_system` module doc." to reflect the PR-E retirement.
- **Add** `Box::new(DeclarativeSciPerSystemRule)` in the appropriate position in the rule-set ordering. Since the walker covers companion-presence checks, place it near the other companion-axis rules (after the existing SCI structural / order rules — `SciSystemOrderRule`, `SciCompartmentOrderRule`, `SciCustomControlInfoRule` — and after `DeclarativeAeaNofornRule` and before `RelToNoDuplicatesRule`).
- **Update** the `use rules_declarative::{ ... }` import line to add `DeclarativeSciPerSystemRule`.
- **Update** the rule-list doc comment at the top of `rules.rs` to bump the rule count.

**`crates/capco/src/rules_sci_per_system.rs`**:

- **Delete the file entirely.**
- **Update** `crates/capco/src/lib.rs` to remove the `pub(crate) mod rules_sci_per_system;` declaration.

**`crates/capco/src/lib.rs`** (verify): may need a minor module-list update.

**`crates/capco/Cargo.toml`** (verify): no change expected.

**`crates/capco/tests/sci_per_system_catalog.rs` (new)** — see §6 for full test plan.

**`crates/capco/tests/corpus_parity.rs` and corpus accuracy harnesses**: rerun, update fixtures if any reference `E042`–`E051` specifically (most tests use diagnostic-content matching, not rule ID; but any `assert_eq!(d.rule.as_str(), "E042")`-style assertions need updating to `"E059"`).

**`tests/corpus/invalid/*.expected.json`** (or similar fixture files): grep for `E042`–`E051` and update each occurrence to `E059`. Since the diagnostic message text changes (no longer per-rule-named), the expected diagnostic message text MUST be regenerated too. Plan a regeneration pass.

**`crates/wasm/tests/parity_corpus.json`**: same E042–E051 → E059 rename + message-text regen.

**`tests/corpus/`** at workspace root: same rename + regen.

**`crates/capco/README.md`**: update rule-inventory paragraph (E042–E051 retire; E059 added). Bump rule count.

**`specs/006-engine-rule-refactor/tasks.md`**: T026e checkbox flip.

**`specs/006-engine-rule-refactor/plan.md`**: rule-count band update.

**`CLAUDE.md`** "Recent Changes" section: add a PR 3b.E entry.

### 4.2 Walker rule-ID and per-row identification

**Single walker rule-ID**: `E059`.
- Inventory grep confirms `E001`–`E058` are all assigned (PR D took E058 as its walker ID). `E034` was previously noted as unused but PR-A took it. **`E059` is the next free slot** (verified via `grep -hrEo 'RuleId::new\("E0[0-9]+"\)' crates/capco/src/ build.rs | sort -u`).

All 5 catalog rows emit diagnostics with `Diagnostic.rule = RuleId::new("E059")`. Per-row identification is via:
- `Constraint::Custom { name }` field carries a stable per-row identifier under the `sci-per-system/` prefix:
  - `sci-per-system/HCS-O-companions`
  - `sci-per-system/HCS-P-NOFORN`
  - `sci-per-system/HCS-P-sub-companions`
  - `sci-per-system/SI-G-companions`
  - `sci-per-system/TK-compartment-NOFORN`
- The `Diagnostic.message` text incorporates the family identifier (the row's `marking_label`, e.g., "HCS-O", "TK-BLFH/IDIT/KAND"). Within each row, the multi-branch emit body further includes the specific branch ("requires ORCON", "requires NOFORN", "forbids ORCON-USGOV") so a diagnostic stream reader can identify which sub-branch fired.
- `ConstraintViolation.constraint_label` (trait/validate path) propagates the catalog row name verbatim.

**Naming-prefix invariant** (mirrors PR D's `class-floor/` invariant): every row's `name` MUST start with `sci-per-system/`. Build-time enforcement: `sci_per_system_catalog_naming_convention` test in `crates/capco/tests/sci_per_system_catalog.rs` asserts every row's name has the prefix.

**Severity-config compatibility**: legacy IDs `E042`–`E051` are NOT preserved as severity-override aliases. Per project memory `feedback_pre_users_no_deprecation_phasing.md`: marque is pre-users; rewrite freely. Users keying `.marque.toml [rules] E042 = "off"` (or any of E043–E051) MUST migrate to `[rules] E059 = "off"`.

### 4.3 Severity defaults

- **Companion-required rows (#2, #5)**: default `Severity::Warn`. Escalate to `Severity::Error` no-fix when `last_dissem_span(attrs)` is `None` (no IC dissem block to anchor zero-width insertion).
- **Multi-branch rows (#1, #3, #4)**: default `Severity::Warn`. Escalate per-emit-branch to `Severity::Error` no-fix on the same no-IC-dissem condition. Note: the ORCON-USGOV-replacement branch never escalates because by definition the dissem block exists if OC-USGOV is present.

The walker's `Rule::default_severity()` is `Severity::Warn` (matches the per-row authoring intent). When a user sets `[rules] E059 = "error"`, the engine's severity-override layer replaces every emitted `Diagnostic.severity` with `Error` regardless of per-row authoring intent — including the no-IC-dissem-already-Error rows (no behavior change). When a user sets `[rules] E059 = "off"`, the engine skips the walker entirely (FR-008 — `Off`-severity diagnostic is unrepresentable; matches PR D pattern).

### 4.4 Diagnostic message shape

Mirror the existing E042–E051 message templates (preserving user-visible text minimizes corpus-fixture churn):

- HCS-O missing ORCON: `"HCS-O requires ORCON (§H.4 p64)"`
- HCS-O missing NOFORN: `"HCS-O requires NOFORN (§H.4 p64)"`
- HCS-O ORCON-USGOV: `"HCS-O forbids ORCON-USGOV (§H.4 p64) — replace with ORCON"`
- HCS-P missing NOFORN: `"HCS-P requires NOFORN (§H.4 p66)"`
- HCS-P sub missing ORCON: `"HCS-P sub-compartment requires ORCON (§H.4 p68)"`
- HCS-P sub ORCON-USGOV: `"HCS-P sub-compartment forbids ORCON-USGOV (§H.4 p68) — replace with ORCON"`
- SI-G missing ORCON: `"SI-G requires ORCON (§H.4 p80)"`
- SI-G ORCON-USGOV: `"SI-G forbids ORCON-USGOV (§H.4 p80) — replace with ORCON"`
- TK-{BLFH,IDIT,KAND} missing NOFORN: `"TK-{BLFH|IDIT|KAND} require NOFORN (§H.4 p87, p91, p95)"`

`Diagnostic.citation` carries the verbatim `CAPCO-2016 §H.4 pXX` (with the multi-page form for row #5). The HCS-P sub-compartment row removes the original E044's "If this should be SECRET, remove the HCS-P sub-compartment" phrase since the class-upgrade fix is no longer the primary action (PR D's `class-floor/HCS-comp-sub` Error diagnostic is now the primary action). Same applies to E046's SECRET/CONFIDENTIAL guidance and E050's SECRET guidance.

### 4.5 Span anchoring

Diagnostic span = the offending SCI marking token (HCS-O, HCS-P, SI-G, TK-BLFH/IDIT/KAND), via `first_sci_span(attrs)` / row's `primary_kind = Some(TokenKind::SciSystem)`. The fix span (zero-width insertion or replacement) differs from the diagnostic span — preserves the existing E042 diagnostic-vs-fix-span split (`emit_companion_insert` puts the diagnostic caret on the SCI token but applies the edit at the dissem-block anchor).

### 4.6 Custom-dispatch perf (3-layer optimization, mirroring PR D)

```rust
fn evaluate_custom_by_attrs(attrs: &CanonicalAttrs, name: &'static str) -> Vec<ConstraintViolation> {
    if is_class_floor_catalog_name(name) {
        return class_floor_catalog_eval(attrs, name);
    }
    if is_sci_per_system_catalog_name(name) {
        return sci_per_system_catalog_eval(attrs, name);
    }
    match name {
        // existing entries unchanged (E010, E012, E014, E021, E024, W002, capco/joint-requires-usa, E038)
    }
}

fn is_sci_per_system_catalog_name(name: &str) -> bool {
    name.starts_with("sci-per-system/")
}
```

Walker hot-path optimization (3 layers, copied from PR D):
- **Layer 1 (axis-presence early-out)**: `if attrs.sci_markings.is_empty() { return Vec::new(); }` at the head of `DeclarativeSciPerSystemRule::check`. On prose body text (most of a 10KB document), `sci_markings` is empty and the catalog walk is skipped entirely.
- **Layer 2 (direct row dispatch)**: walker reads `(row.presence)(attrs)` and dispatches to `sci_per_system_eval_row` with the row in hand — zero string-keyed lookup.
- **Layer 3 (DRY emit helper)**: `sci_per_system_emit` is the single source of truth for citation, message-text, fix-shape, and severity escalation. Both walker (`sci_per_system_eval_row`) and trait/validate (`sci_per_system_catalog_eval`) converge through it.

The `sci_per_system_row_by_name` linear-scan lookup (5-row catalog) on the trait/validate path is ≪1 µs and is not worth the complexity of a `phf::Map`. Defer until profiling shows the trait path as a hotspot.

### 4.7 Test coverage

- ≥80% line coverage on the new `sci_per_system_emit` helper and per-row presence predicates (Constitution VII testing rules).
- All existing E042–E051 unit tests in `rules_sci_per_system.rs` (the `#[cfg(test)] mod tests` block at lines 1027–1585 — comprehensive: E042 has ~6 tests, E043–E051 have 3-5 each, total ~50 tests) **must be ported** to `tests/sci_per_system_catalog.rs` with the rule-ID assertions updated from `E042`–`E051` to `E059` and message-text assertions preserved verbatim. This is the largest test-migration cost of the PR.

---

## 5. Performance budget

### 5.1 The bench gate

CI enforces baseline+10% on:
- `lint_10kb` (criterion bench, lint a 10KB document — proxy for SC-001 perceptual instantaneity)
- `decoder_10kb_one_mangled_region` (criterion bench, lint a 10KB document with one mangled marking — proxy for SC-002 deep-scan latency)

PR D landed below baseline after the 3-layer optimization: `lint_10kb` = 823 µs (gate 911 µs); `decoder_10kb_one_mangled_region` = 996 µs (gate 1113 µs).

### 5.2 Expected delta from PR E

PR E's net change to the walker hot path:
- **Removes** 10 `impl Rule for ...` `check()` calls per portion (the existing E042–E051 each iterate `attrs.sci_markings`, build helper data, and emit).
- **Adds** 1 `impl Rule for DeclarativeSciPerSystemRule::check()` call per portion. Body: 1 axis-presence check + (when SCI present) 5-row catalog walk with per-row presence predicate.

Old per-portion cost (10 separate rules): each rule iterates `attrs.sci_markings` independently — total ~10 iterations + 10 `us_level()` calls + 10 `dissem_controls.contains(...)` calls.

New per-portion cost (1 walker, 5-row catalog): 1 `sci_markings.is_empty()` check (O(1)) + (when non-empty) 5 presence-predicate calls (each iterates `sci_markings` once, ~5 iterations total) + per-row emit dispatch.

**Expected outcome**: 5 iterations vs 10 iterations on the SCI-present hot path, plus a O(1) early-out on prose body text. Net: should be at-or-below current; certainly under the +10% gate. The early-out is the dominant win on a 10KB document where most portions are prose body text (no SCI markings); per-portion cost drops to a single boolean check.

### 5.3 Sanity-check command

```sh
cargo bench --bench lint_latency -- --save-baseline pr-e-sanity
# compare against pre-PR-E baseline:
cargo bench --bench lint_latency -- --baseline pr-e-sanity
```

Expected delta: within ±5% of pre-PR-E for both `lint_10kb` and `decoder_10kb_one_mangled_region`. **Never propose bumping the baseline** — that's a PM decision.

### 5.4 Risk: per-portion cost when SCI markings ARE present

A document dense with SCI markings (e.g., a corpus fixture with many HCS-O / SI-G portions) exercises the 5-row catalog walk. Each row's presence predicate iterates `attrs.sci_markings` once — total 5 iterations × N markings = O(5N). The previous design was 10 separate rules each iterating once = O(10N). PR E is approximately 2× faster on the SCI-present path. The existing benches don't isolate SCI-dense documents; if profiling reveals a hotspot, consider amortizing the `sci_markings` iteration into a single pass with a presence-bitmap (deferred unless measured).

---

## 6. Test plan

### 6.1 Test file structure

New file: `crates/capco/tests/sci_per_system_catalog.rs`. Mirror the shape of `crates/capco/tests/class_floor_catalog.rs` (1463 LOC, ~91 tests) — but materially smaller because PR E has 5 rows × 3-branch fixtures, not 27 rows.

### 6.2 Per-row behavior triplet (15 tests minimum, 3 per row)

For each of the 5 catalog rows:

1. **`test_<row>_fires_on_violation`**: presence-predicate fires AND constraint not satisfied → emit non-empty diagnostics, all carrying `Diagnostic.rule = "E059"` AND the diagnostic message identifies the offending invariant.
2. **`test_<row>_does_not_fire_when_satisfied`**: presence-predicate fires AND constraint satisfied → zero diagnostics from E059 for this row.
3. **`test_<row>_does_not_fire_when_marking_absent`**: presence-predicate doesn't fire → zero diagnostics.

### 6.3 Multi-branch row fan-out tests (3 rows × 2-3 branches = 6-7 tests)

For rows #1 (HCS-O), #3 (HCS-P sub), #4 (SI-G):
- Each emit branch tested independently (missing-ORCON only / missing-NOFORN only / ORCON-USGOV-only / multiple-missing-companions).
- Verify the diagnostic span anchors at the SCI token (not the dissem token), and the fix span is the zero-width insertion at end-of-last-dissem (or the OC-USGOV span for replacement).

### 6.4 Severity escalation tests (5 tests, 1 per row)

For each row whose CompanionRequired or Custom branch produces a companion-insertion fix:
- **`test_<row>_no_dissem_block_escalates_to_error_no_fix`**: portion lacks any IC dissem block → diagnostic emitted at `Severity::Error` with `fix: None`. Mirrors the existing `e042_no_dissem_block_escalates_to_error_no_fix` pattern (verbatim port).

### 6.5 Scope-guard correctness tests (5 tests)

For each row that has a `us_level(attrs).is_none()` scope guard:
- **`test_<row>_does_not_fire_on_pure_foreign_classification`**: portion is `(NS//HCS-O)` (NATO classification, no US level) → zero E059 diagnostics. Verifies the §H.4-is-US-only-scoped invariant.

### 6.6 PR-D overlap-fire test (1 test, R6 in PR D §7(e))

**`test_pr_d_class_floor_and_pr_e_companion_both_fire_distinctly`**: portion `(S//HCS-O//OC)` (HCS-O on SECRET, missing NOFORN). Expected: PR D's `class-floor/HCS-comp` does NOT fire (S satisfies the S-floor); PR E's `sci-per-system/HCS-O-companions` fires once (NOFORN missing). Confirms the two catalogs don't double-fire when the floor is satisfied but the companion is missing.

**`test_pr_d_class_floor_only_fires_when_companion_satisfied`**: portion `(C//HCS-P//OC/NF)` (HCS-P on CONFIDENTIAL, both companions correct). Expected: PR D's `class-floor/HCS-comp` fires once (C is below the S floor); PR E `sci-per-system/HCS-P-NOFORN` does NOT fire (NOFORN present). Confirms one-diagnostic-per-violation when the violation is purely floor.

**`test_pr_d_class_floor_and_pr_e_companion_both_fire_when_both_violated`**: portion `(C//HCS-O)` (HCS-O on CONFIDENTIAL, no companions). Expected: PR D's `class-floor/HCS-comp` fires (Error); PR E's `sci-per-system/HCS-O-companions` fires for both ORCON and NOFORN (Error no-fix because no IC dissem block). Confirms the two catalogs fire side-by-side without overlap-guard interference.

### 6.7 Audit-stream traceability test (1 test)

**`test_e059_diagnostic_stream_per_row_identifiable`**: lint a fixture containing all 5 row violations; assert each emitted `Diagnostic.message` contains its row's marking label (HCS-O / HCS-P / HCS-P sub / SI-G / TK-{BLFH|IDIT|KAND}). Pins per-row identifiability.

### 6.8 Catalog-naming-convention test (1 test)

**`test_sci_per_system_catalog_naming_convention`**: every row in `sci_per_system_catalog()` has `name.starts_with("sci-per-system/")`. Mirrors PR D's `class_floor_catalog_naming_convention` test.

### 6.9 Severity::Off override test (1 test)

**`test_e059_off_severity_skips_walker`**: configure `.marque.toml [rules] E059 = "off"`, lint a document with HCS-O missing companions → zero E059 diagnostics. Verifies FR-008.

### 6.10 Citation-fidelity snapshot (1 test)

**`test_sci_per_system_catalog_citations`**: every row's `citation` field parses cleanly + matches one of `["CAPCO-2016 §H.4 p64", "CAPCO-2016 §H.4 p66", "CAPCO-2016 §H.4 p68", "CAPCO-2016 §H.4 p80", "CAPCO-2016 §H.4 p87 + p91 + p95"]`. Pins the verified citation set against drift.

### 6.11 Corpus parity

Existing `crates/capco/tests/corpus_parity.rs` and corpus accuracy harnesses must continue to pass. SC-002 (≥95% per-rule accuracy on the corpus) is preserved by re-running the gate post-implementation.

### 6.12 Total test count estimate

- Per-row triplet: 5 rows × 3 = 15
- Multi-branch fan-out: ~6-7
- Severity escalation: 5
- Scope guard: 5
- PR-D-overlap: 3
- Audit traceability: 1
- Naming convention: 1
- `Severity::Off`: 1
- Citation fidelity: 1

**Total: ~38 tests**, materially smaller than PR D's 91-test class-floor suite. Plus the ~50-test verbatim port from `rules_sci_per_system.rs`'s existing `mod tests` block (rule-ID assertions updated `E042`–`E051` → `E059`, message-text assertions preserved verbatim) — bringing the total to ~88 tests in the new file.

---

## 7. Reviewer attestation requirements

The PR description MUST declare each of (a)–(e):

**(a) Single CAPCO-§ citation per declarative catalog entry.** All 5 PR-E catalog rows carry one verified `CAPCO-2016 §H.4 pXX` citation. Page anchors verified present in the vendored markdown (see §2.1):
- Row #1 → `§H.4 p64` (HCS-O body, line 1413)
- Row #2 → `§H.4 p66` (HCS-P body, line 1468)
- Row #3 → `§H.4 p68` (HCS-P [SUB] body, line 1524)
- Row #4 → `§H.4 p80` (SI-G body, line 1839)
- Row #5 → `§H.4 p87 + p91 + p95` (TK-BLFH body line 2043, TK-IDIT body line 2158, TK-KAND body line 2272 — multi-page form per the existing E051 citation, justified because all three TK compartments share the NOFORN requirement)

**(b) Predicate body of every `impl Rule` block has ≤3 internal branches.** The new `DeclarativeSciPerSystemRule::check` body has exactly 2 branches: (1) axis-presence early-out, (2) catalog-walk loop. Inside the loop, `match row.kind` has 2 arms (`CompanionRequired` and `Custom`) — under the ≤3-branch cap. Justification for not unifying `Custom` with `CompanionRequired` into a single richer variant: rows #1, #3, #4 each have 2-3 distinct emit branches (insert ORCON, insert NOFORN, replace OC-USGOV) where the per-branch text and span logic differs; encoding that in a static struct (e.g., `MultiPart { branches: [&'static EmitBranch] }`) would add more LOC and obscurity than embedding the row-specific logic in a `Custom` closure.

**(c) Net rule delta and running count.**
- Pre-PR-E: 53 `impl Rule` blocks (per PR D's running count: 55 → 53 after retiring E022/E025/E027 + adding E058 walker).
- PR E delta: 10 retired (`HcsOCompanionsRule` E042, `HcsPRequiresNofornRule` E043, `HcsPSubcompartmentTsOnlyRule` E044, `HcsClassificationCeilingRule` E045, `SiCompartmentTopSecretRule` E046, `SiGammaCompanionsRule` E047, `RsvClassificationCeilingRule` E048, `TkClassificationCeilingRule` E049, `TkBlfhTopSecretRule` E050, `TkCompartmentRequiresNofornRule` E051) + 1 walker added (`DeclarativeSciPerSystemRule` E059) = **net −9; running count 53 → 44.**
- Catalog deltas: **+5 `Constraint::Custom` rows on `CapcoScheme`** at §H.4 family granularity.

**(d) No orphaned class-floor enforcement.** Every retired rule's class-floor function is covered by a specific PR-D class-floor row:
- E044 floor (HCS-P-sub TS-only) → `class-floor/HCS-comp-sub`
- E045 floor (HCS-O / bare HCS-P TS-or-S) → `class-floor/HCS-comp`
- E046 floor (SI compartment TS-only) → `class-floor/SI-comp`
- E048 floor (RSV TS-or-S) → `class-floor/RSV-comp`
- E049 floor (TK family TS-or-S) → `class-floor/TK`
- E050 floor (TK-BLFH TS-only) → `class-floor/TK-BLFH`

**(e) No double-fire on overlap.** The PR-E catalog walker emits no class-floor diagnostics. Every PR-E row is companion-presence-only; no row's predicate or emit body checks classification level. Test fixtures pin this for the three "Floor TS" overlaps with PR D (`class-floor/HCS-comp-sub`, `class-floor/SI-comp`, `class-floor/TK-BLFH`) — see §6.6 (`test_pr_d_class_floor_and_pr_e_companion_both_fire_distinctly`, `test_pr_d_class_floor_only_fires_when_companion_satisfied`, `test_pr_d_class_floor_and_pr_e_companion_both_fire_when_both_violated`).

---

## 8. Open questions / PM decisions needed

### OQ-1 (P0 — affects PR scope): Lost actionable class-upgrade fixes from E044 / E046 / E050

The retired rules E044, E046, and E050 each carry a `build_class_upgrade_fix` actionable fix (auto-upgrade `S` → `TS`). PR D's corresponding `class-floor/HCS-comp-sub` / `class-floor/SI-comp` / `class-floor/TK-BLFH` rows emit no-fix `Error` diagnostics. After PR E retires E044/E046/E050, the actionable class-upgrade fix is **lost** — users see the class-floor Error but no automated correction.

PR D §4.2 explicitly anticipated this: *"the per-system rule carries the fix; the catalog row carries the §-cited descriptor. PR 3b.E (T026e) retires the per-system rules and the catalog row becomes the sole emitter — its reviewer-attestation includes 'no orphaned class-floor enforcement after E044-E050 retirement; the catalog row covers it.'"* This treats the lost fix as a PR-D-then-PR-E-retire-as-planned trajectory, with class promotion landing later via FixIntent under PR 3c+.

**PM decision needed**:
- **(a)** Accept the regression as planned per PR D §4.2 (class-upgrade fix is FixIntent-territory under PR 3c+; users get the diagnostic, not the fix, until then). PR E ships as planned.
- **(b)** Block PR E until PR 3c (FixIntent) lands so the actionable class-upgrade fix is preserved. This re-orders the engine refactor stages.
- **(c)** Add a fix-emitting variant to PR D's class-floor catalog rows (`ClassFloorRow.fix: Option<...>`) that PR E populates for the three actionable rows. This expands PR E's scope to include a PR-D `scheme.rs` change and arguably violates the "one architectural shape per PR" discipline.

**Recommendation**: (a). PR D's plan and the PR-D R6 reviewer attestation explicitly authorize the temporary regression. The engine refactor's staging table (`plan.md:263`) commits to PR 3c+ as the FixIntent landing — the regression is a known interim cost.

### OQ-2 (P1 — does not block PR E): Lost Warn-no-fix ambiguity guidance from E045 / E048 / E049

E045 / E048 / E049 emit `Severity::Warn` no-fix diagnostics with the message *"<MARKING> requires TOP SECRET or SECRET; resolve by upgrading the classification or removing the <MARKING>"*. PR D's corresponding `class-floor/HCS-comp` / `class-floor/RSV-comp` / `class-floor/TK` rows emit `Severity::Error` with the simpler message *"<MARKING> requires classification ≥ S (CAPCO-2016 §H.4); current classification is <CURRENT>"*.

The user-visible regression: (1) severity escalates from `Warn` to `Error` (a stricter pipeline behavior, arguably an improvement per the Constitution's "Quality > Speed" principle); (2) the ambiguity-guidance phrase ("upgrade the classification or removing the marking") is lost.

**PM decision needed**:
- **(a)** Accept the regression (severity escalation is correct; ambiguity-guidance is informational and can be added to PR D rows in a follow-up cleanup PR — not in PR E's scope).
- **(b)** Move the ambiguity-guidance text onto PR D's `class-floor/HCS-comp` / `RSV-comp` / `TK` rows in this PR, by parameterizing `class_floor_emit` with an optional ambiguity-message. Adds ~30 LOC to PR E in `scheme.rs` and is a simple text-only change to PR D rows.

**Recommendation**: (a). The PR-D rows' message format is a separate concern from PR E's collapse. A follow-up PR can add ambiguity-guidance to PR D rows uniformly.

### OQ-3 (resolved): Walker rule-ID

`E059` is the next free `E###` slot after PR D took E058. Verified via `grep -hrEo 'RuleId::new\("E0[0-9]+"\)' crates/capco/src/`: E001–E058 are all assigned. **No PM decision needed**; documenting for completeness.

### OQ-4 (P2 — design choice, internal): `SciPerSystemKind` enum cardinality

Picked **2 variants** (`CompanionRequired` + `Custom`) to stay under the ≤3-branch reviewer-attestation cap. Alternative designs:
- **All-Custom**: every row stores a `fn(&CanonicalAttrs, &RuleContext) -> Vec<Diagnostic>` closure; no enum at all. Simpler but loses the static "this row is a single-companion-required check" property that the trait/validate path could otherwise leverage.
- **Richer enum**: split `Custom` into `CompanionAndForbid { required: Set<DissemControl>, forbidden: Set<DissemControl>, replacement: ReplacementMap }`. Pushes more logic into static data, fewer closures. But `Set<DissemControl>` adds a build-time table type and the per-row authoring becomes verbose for what's effectively bespoke logic in 3 rows.

**Recommendation**: stick with 2 variants. The chosen design encodes the simple cases (rows #2, #5) statically and lets the bespoke cases (rows #1, #3, #4) keep their per-row clarity.

### OQ-5 (P2 — known divergence): Trait/validate path drops the fix

PR-E rows produce `FixProposal` values (companion-insertion or replacement). The trait/validate path (`MarkingScheme::validate` → `marque_scheme::constraint::evaluate` Custom-arm) emits `ConstraintViolation` (no fix field). The walker hot path emits `Diagnostic` (with fix). This is the same divergence PR D has — the trait/validate path is a structural-validation surface (used by tooling that wants a yes/no compliance check), not an audit-record-producing surface.

**No PM decision needed**; documenting for completeness. The engine path is the only path that produces `AppliedFix` records, and the engine path always uses the walker.

### OQ-6 (P2 — verify no engine gap): `RuleContext` propagation through trait/validate path

The PR D `evaluate_custom_by_attrs` function does not take `&RuleContext`. PR E rows #1, #3, #4 (multi-branch) currently use `&RuleContext` only for `marking_type` to choose between portion vs banner classification-token form (in `build_class_upgrade_fix`). After PR E, no PR-E row uses `RuleContext` for class-upgrade fix construction (the class-upgrade fix is dropped — see OQ-1). The remaining `RuleContext` usage is for fix-form-from-`marking_type` (banner vs portion form for the inserted token, e.g., `OC` vs `ORCON`).

But the existing `infer_companion_form` helper inspects the dissem-token text directly (not `marking_type`), so portion-vs-banner form is determined from the dissem block, not from `RuleContext`. **Verify**: `RuleContext` is unused by PR-E rows. If true, the trait/validate path's lack of `RuleContext` is fine. If false (some row needs `marking_type`), document the workaround (use `infer_companion_form` from the dissem block, not `RuleContext.marking_type`).

**Recommendation**: implementation agent verifies this in the first read-through. If `RuleContext` is genuinely needed by any row, that row's emit body falls back to `MarkingType::Portion` on the trait/validate path (a benign default — banner form `ORCON` vs portion form `OC` differs only in the inserted token, not in correctness of the violation flagging). No engine gap.

### OQ-7 (P0 — quick verify): `make_fix_diagnostic` and `FixDiagnosticParams` visibility

The retired rules call `crate::rules::{FixDiagnosticParams, make_fix_diagnostic}`. After deletion of `rules_sci_per_system.rs`, the call site moves to `scheme.rs` (per §4.1 helper-relocation Option A). Verify `FixDiagnosticParams` and `make_fix_diagnostic` are `pub(crate)` (or higher) so `scheme.rs` can call them. Spot-check from `rules.rs:379`–`rules.rs:1749` (multiple existing call sites in `rules.rs`) suggests they are at least `pub(crate)`. **No PM decision needed**; routine verification at implementation time.

---

## 9. Out of scope

- **Engine edits.** Constitution VII §IV. No edits to `crates/engine`, `crates/scheme`, `crates/core`, `crates/rules`, `crates/ism`. The only `crates/capco/scheme.rs` and `crates/capco/rules_declarative.rs` changes are scheme-adoption-side (catalog rows + walker).
- **`Constraint` enum variants.** No `Constraint::CompanionRequired` / `Constraint::Forbid` primitives — that's PR 4's territory once `marque-scheme` lattice work expands.
- **PR 3.7 primitives.** No `TokenRef::ClassAtLeast(ClassLevel)` — PR 3.7 (T108b) territory.
- **PR 4 per-category Lattice impls.** The walker remains until PR 4 lands `CapcoScheme` `Lattice` projections that retire the walker entirely.
- **FixIntent re-introduction of class-upgrade fixes.** Per OQ-1, the actionable class-upgrade fix from E044 / E046 / E050 is lost in PR E and returns when FixIntent lands under PR 3c+. Out of scope here.
- **Ambiguity-guidance text on PR D class-floor rows.** Per OQ-2, the "upgrade the classification or remove the marking" phrase from E045 / E048 / E049 is not migrated onto PR D rows in this PR. Follow-up PR.
- **Doc-only PRs.** Nothing in this PR ships docs-without-code or vice-versa.

---

## 10. Risks and mitigations

### R1: Citation drift from rule-source to PR-E catalog
**Risk**: existing E042–E051 rule sources cite `§H.4 pXX` pages; if a citation in the source is wrong (despite having passed earlier code review), copying it into PR E catalog rows propagates the error.
**Mitigation**: §2 of this plan re-verifies every cited page against the vendored markdown's `begin page NNN` anchors. The implementation agent verifies one more time at implementation time by re-grepping for each citation in `crates/capco/docs/CAPCO-2016.md` and confirming the marking-body language matches.

### R2: Helper relocation breaks unrelated rules
**Risk**: the helpers being moved from `rules_sci_per_system.rs` to `scheme.rs` (`anchors_on`, `has_compartment`, `compartment_has_sub`, `is_tk_noforn_compartment`, `first_sci_span`, `us_level`, `classification_token`, `last_dissem_span`, `dissem_token_span`, `infer_companion_form`, `CompanionForm`, `emit_companion_insert`) might be referenced by `rules_declarative.rs` or `rules.rs` indirectly. A move that breaks visibility breaks compilation.
**Mitigation**: implementation agent runs `cargo check --workspace` immediately after the move, before adding the catalog rows. The pre-flight chain catches this. `grep -r "rules_sci_per_system::" crates/capco/src/` should show no references after the deletion.

### R3: `build_class_upgrade_fix` orphaning
**Risk**: `build_class_upgrade_fix` is declared in `rules_sci_per_system.rs:206-221`. After deletion, the function is gone. No PR-D code uses it (PR D's class-floor rows emit no-fix), but a careful audit is needed in case some other rule indirectly imports it.
**Mitigation**: `grep -r "build_class_upgrade_fix" crates/capco/src/` confirms only `rules_sci_per_system.rs` defines/uses it. Safe to drop with the file deletion. (If the implementation agent finds an unexpected reference, it's a planning gap — surface immediately.)

### R4: Performance regression on SCI-dense fixtures
**Risk**: 5-row catalog walk per portion adds overhead vs the previous 10 separate rules. The early-out optimizes prose body text, but SCI-dense fixtures might regress.
**Mitigation**: the 3-layer optimization (axis-presence early-out, direct row dispatch, DRY emit helper) is mandatory from the start. The criterion bench gate detects regressions ≥10%. If a regression appears, profile and amortize the `sci_markings` iteration into a single-pass presence-bitmap (not in scope for the initial PR; deferred to a follow-up perf PR if needed).

### R5: Test count explosion
**Risk**: porting all ~50 existing E042–E051 unit tests + adding ~38 catalog-shape tests = ~88 tests in one new file. Maintenance cost.
**Mitigation**: per-row triplet pattern (mirrors PR D) keeps each row's tests in a contiguous block. Comments explicitly cite the source rule's test (e.g., `// Verbatim port of e042_no_dissem_block_escalates_to_error_no_fix`). Future maintenance is simplified by the fact that the catalog row IS the source of truth, not the rule.

### R6: Corpus fixture churn
**Risk**: corpus fixtures (`tests/corpus/`, `crates/wasm/tests/parity_corpus.json`) hardcode rule IDs `E042`–`E051` AND diagnostic message text. PR E renames all 10 to `E059` AND changes some message text (specifically, removes the "If this should be SECRET, remove the compartment" guidance from E044/E046/E050 since the class-upgrade fix is no longer the primary action — see §4.4). Renaming is mechanical; message-text drift requires expected-output regeneration.
**Mitigation**: implementation agent runs the corpus harness after the rename, regenerates expected outputs, diffs the regen against the prior expected file, and confirms each diff is one of (a) `E042`–`E051` → `E059` rule-ID rename, or (b) the documented message-text simplification. Any other diff is a behavior regression and must be triaged before commit.

### R7: Citation-methodology drift
**Risk**: the §H.4 multi-page citation form for row #5 (`§H.4 p87 + p91 + p95`) is preserved verbatim from existing E051. If a reviewer challenges the multi-page form (citing PR D's preference for single-page citations per §3.4.6), the row decomposes into 3 separate rows (one per TK compartment).
**Mitigation**: §2.2 row #5 explicitly justifies the multi-page form ("all three TK compartments share the NOFORN requirement; the §3.4.6 author's design favors family-granularity, which the existing E051 follows"). If the PM decides to split, that's an OQ-8 we surface; for now we follow the existing design.

### R8: `typos` lint failure
**Risk**: PR D got tripped by `labelled` (British). PR E's diagnostic messages might introduce another typo.
**Mitigation**: pre-flight `typos .` is in the chain. Implementation agent runs it as a final pass before commit.

---

## 11. Acceptance criteria

PR 3b.E is mergeable when:

1. The 5 PR-E catalog rows land as `Constraint::Custom` rows on `CapcoScheme`, named `sci-per-system/<row-id>`, with verified `CAPCO-2016 §H.4 pXX` citations per §2.
2. `DeclarativeSciPerSystemRule` (rule ID `E059`) walks the catalog and emits diagnostics carrying `Diagnostic.rule = "E059"` plus per-row identifying message text.
3. The 10 retired rules (`HcsOCompanionsRule` E042 through `TkCompartmentRequiresNofornRule` E051) and the file `crates/capco/src/rules_sci_per_system.rs` are deleted; the registration entries in `rules.rs` are replaced with the single walker entry.
4. The behavior tests for E042–E051 (verbatim ports from the deleted file's `mod tests`) pass against the catalog walker with rule-ID assertions updated `E042`–`E051` → `E059`.
5. The new catalog-shape tests in `tests/sci_per_system_catalog.rs` pass (per-row triplet, multi-branch, severity-escalation, scope-guard, PR-D-overlap, audit-traceability, naming convention, `Severity::Off`, citation fidelity).
6. `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo test --workspace --no-fail-fast` all pass.
7. `wasm-pack build crates/wasm --target web` builds clean.
8. `cargo doc --no-deps --workspace` builds clean.
9. `typos .` passes.
10. Coverage on `sci_per_system_emit` and per-row presence predicates ≥80%.
11. Criterion `lint_10kb` and `decoder_10kb_one_mangled_region` benches stay within the existing baseline+10% gate.
12. Corpus parity passes; SC-002 ≥95% per-rule accuracy preserved (E059 inherits the per-rule accuracy from its 5 catalog rows).
13. PR description includes reviewer-attestation (a)–(e) per §7.
14. `crates/capco/README.md` rule-inventory paragraph updated; rule count bumped 53 → 44.
15. `specs/006-engine-rule-refactor/tasks.md` T026e checkbox flipped.
16. `specs/006-engine-rule-refactor/plan.md` rule-count band updated.
17. `CLAUDE.md` "Recent Changes" section gets a PR 3b.E entry.
18. CI passes.
19. GPG-signed commits.

---

## 12. Implementation status: PM APPROVED — 2026-05-08

Plan reviewed and approved by PM. Implementation may proceed.

### PM resolutions on open questions

- **OQ-1 (P0) — Lost actionable class-upgrade fixes from E044/E046/E050: ACCEPT (option a).**
  Per PR D §4.2 staging table, the actionable class-upgrade fix returns under PR 3c+ via FixIntent. Accepting the temporary regression on staging keeps the "one architectural shape per PR" discipline intact. Verified against worktree: PR D's `class_floor_emit` (`crates/capco/src/scheme.rs:2786`) returns `Option<ConstraintViolation>` only — no `FixProposal` is emitted by any class-floor row. The retired rules' fix-emission is genuinely orphaned by PR E and is not recoverable without a PR-D row-shape change (rejected as scope creep). User-visible regression is documented; users still receive the Error diagnostic, they just have to write `TS` manually until PR 3c+.

- **OQ-2 (P1) — Lost ambiguity-guidance from E045/E048/E049: ACCEPT (option a).**
  Severity escalation from `Warn` (E045/E048/E049) to `Error` (PR D class-floor rows) is the correct pipeline behavior — the floor violation is unambiguous and should fire at Error severity. The "upgrade or remove the marking" guidance is informational and migrates onto PR D rows in a follow-up cleanup PR (out of scope for PR E).

- **OQ-4 (P2) — `SciPerSystemKind` cardinality: STICK WITH 2 VARIANTS.**
  `CompanionRequired` + `Custom` is the cleanest decomposition. Walker `match row.kind` body has 2 arms — under the ≤3-branch reviewer-attestation cap. Bespoke logic in rows #1/#3/#4 stays per-row-clear via the closure variant.

- **OQ-3 (resolved) — Walker rule-ID `E059`: VERIFIED.**
  PM verified `grep -hrEo 'RuleId::new\("E[0-9]+"\)' crates/capco/src/ build.rs`: highest assigned is E058 (PR D walker). E059 is the next free slot.

- **OQ-5 / OQ-6 / OQ-7: NO PM ACTION; routine implementation-time verification.** Confirmed `make_fix_diagnostic` + `FixDiagnosticParams` are `pub(crate)` in `crates/capco/src/rules.rs:5410` / `:5423` — `scheme.rs` (same crate) can call them after helper relocation.

### Additional PM directives for implementation

1. **Consolidate verbatim test ports where they duplicate per-row triplet coverage.** The plan §6.12 estimates ~88 tests (~50 verbatim ports + ~38 catalog-shape tests). Where a verbatim port from `rules_sci_per_system.rs::tests` duplicates a catalog-shape per-row triplet test (§6.2), drop the port and rely on the triplet. Target: keep total around 60–75 tests if possible without sacrificing behavior coverage. The litmus test: "would dropping this port leave a behavior un-tested?" If no, drop it. If yes, keep it but cross-link to the catalog-shape test it complements.

2. **Helper relocation ordering.** The plan §4.1 lists "delete `rules_sci_per_system.rs`" and "move helpers to `scheme.rs`" — execute in this order: (a) move helpers first into `scheme.rs` under a new `// SCI per-system helpers` section, (b) update all call sites in the file being deleted to compile against the moved helpers (intermediate state — file still exists), (c) verify `cargo check --workspace` passes, (d) only then delete `rules_sci_per_system.rs` and update `lib.rs`. This preserves the bisect-ability of the diff.

3. **GPG-signed commits.** Mandatory. Never use `--no-gpg-sign`.

4. **Doc-drift sweep — final pass.** Beyond the §4.1 list (`README.md`, `tasks.md`, `plan.md`, `CLAUDE.md`), grep for any other doc that mentions `rules_sci_per_system` or counts rules in the 50-something range. Examples to check:
   - `docs/plans/2026-05-02-engine-refactor-consolidated.md` (PR 3b.E section, expected rule-count band)
   - `docs/plans/2026-05-08-pr3b-D-class-floor-catalog-plan.md` (the PR D-to-PR E transition note in §4.2 / §0)
   - `crates/capco/CAPCO-CONTEXT.md` (probably no count to update; verify)
   - Any `docs/decisions/` entries
   - `crates/capco/docs/` per-rule index if it exists

5. **Pre-flight chain (mandatory before commit, in order).** `cargo check --workspace` → `cargo clippy --workspace --all-targets -- -D warnings` → `cargo fmt --all --check` → `cargo test --workspace --no-fail-fast` → `wasm-pack build crates/wasm --target web` → `cargo doc --no-deps --workspace` → `typos .`.

6. **PR description must declare reviewer attestations (a)–(e) per §7.** Cross-link this plan doc; don't restate the body.

7. **Bench sanity-check is informational, not blocking.** Run the criterion benches locally per §5.3 to catch regressions before pushing; CI's bench-check gate is the binding signal.

**No blockers.** No engine-gap discovered during analysis. All 10 retirement decisions cleanly map onto either a PR-D class-floor row (full coverage of floor portion) or a PR-E catalog row (companion portion); no §H.4 invariant falls through both.
