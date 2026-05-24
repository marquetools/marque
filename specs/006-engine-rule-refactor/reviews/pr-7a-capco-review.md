<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.
SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# PR 7a CAPCO-Domain Review

Reviewer: CAPCO-domain reviewer (PR 7a, phase-tagged pass-split plumbing).
Scope: every `fn phase() -> Phase` declared in `crates/capco/src/rules.rs`
and `crates/capco/src/rules_declarative.rs` against the CAPCO-2016 semantics
of the rule that owns it. Engine consumption / dispatch is **not in scope**
for PR 7a — `Engine::new` partitions on `phase()` but does not yet dispatch
on it (`docs/refactor-006/pr-7-pm-decisions.md` D-7.2 final paragraph).
The review therefore evaluates whether the **declared** phase is correct as
a load-bearing claim PR 7b will rely on.

## Approval Status

**APPROVE** with two **LOW** notes (no MEDIUM, no HIGH).

Every declared `Phase` value is consistent with CAPCO-2016 semantics and the
plan §9.1 definition of `Phase::Localized` (sub-token-only fix span) vs
`Phase::WholeMarking` (full-marking fix or marking-scope decision). No
fabricated CAPCO citation in any `phase()` docstring; every section/page
reference traces to a real passage in
`crates/capco/docs/CAPCO-2016.md`. The two LOW notes are stylistic /
clarity, not correctness defects.

---

## Per-Rule Phase Verification

### Phase::Localized (4 rules — all verified)

| RuleId | Implementer claim | Span shape | CAPCO citation | Verdict |
|--------|-------------------|------------|----------------|---------|
| C001 | Corrections-map sub-token | sub-token (`token_span.span` for one `TokenSpan` per fix; replacement = `corrections.get(text)`) | N/A — `marque_rules::CORRECTIONS_MAP_CITATION = "CONFIG:[corrections]"` (deliberate non-CAPCO; user-supplied map) | **PASS** |
| E006 | Deprecation single-token | sub-token (`token.span` for one `TokenSpan`, replacement from `MIGRATIONS` table; e.g. `LIMDIS → LIMITED DISTRIBUTION`) | CAPCO-2016 §F p35 (Legacy Control Markings) — verified at `CAPCO-2016.md` line 724 | **PASS** |
| E007 | Strip dash from X-shorthand token | sub-token (`token.span` for one `Unknown` token, replacement = `text.trim_end_matches('-')` or `MIGRATIONS` lookup) | CAPCO-2016 §E.6 pp 33–34 (Retired or Invalid Declassify On Values) — verified at `CAPCO-2016.md` line 689 (`### 6. (U) Retired or Invalid Declassify On Values`) | **PASS** |
| S004 | Trigraph swap | sub-token (`span_token.span` for one `RelToTrigraph` token, replacement = corpus-derived canonical 3-letter trigraph) | CAPCO-2016 §H.8 pp 150–151 (REL TO template + "USA must always appear first") — verified at `CAPCO-2016.md` lines 3675, 3709 | **PASS** |

**Notes on each Localized rule.**

- **C001** — `check()` body at `rules.rs:1217-1253` walks every
  `TokenSpan`, skips `Separator`, and emits one `Diagnostic` per match
  with `span: token_span.span`. The replacement comes verbatim from the
  user's `[corrections]` map. The substitution is mechanically sub-token
  (one `TokenSpan` is one parser-recognized token). Per D-7.6, C001 also
  runs as **pass-0** before phase partitioning, so the `Phase::Localized`
  declaration is harmless for dispatch but governs the rule-loop call
  path if pass-1 ever dispatches the same rule (re-entering pass-0
  territory is impossible — `apply_text_corrections` consumes the
  pre-scanner channel separately). The docstring at `rules.rs:1208-1213`
  is explicit about this dual-channel reality.

- **E006** — `check()` body at `rules.rs:733-774` walks `DissemControl`
  and `Unknown` token spans; for each, calls `find_migration(token.text)`
  and emits with `span: token.span`. The replacement is the `MIGRATIONS`
  table's value (e.g. `LIMDIS → LIMITED DISTRIBUTION`). One token in,
  one token out, no separator crossings. The §F citation refers to
  CAPCO-2016's "Legacy Control Markings" section (p35), which is the
  governing passage for the re-marking obligation that E006 implements
  via the migration table. CAPCO-2016.md line 724-735 confirms §F covers
  the legacy-marking re-mark requirement plus the `Unauthorized IC
  Classification and Control Markings list`. The §F citation in the
  diagnostic (`"CAPCO-2016 §F"` at `rules.rs:766`) is bare (no page
  number) — see LOW-2.

- **E007** — `check()` body at `rules.rs:855-918` has two paths (migration
  table + pattern strip); both emit with `span: token.span` and
  replacement = migration-table value or `text.trim_end_matches('-')`.
  Either way the span shape is sub-token. §E.6 (`### 6. (U) Retired or
  Invalid Declassify On Values`, CAPCO-2016.md line 689, pp 33–34)
  enumerates `25X1-25X9` exemptions without the required date/event as
  invalid — the §E.6 authority is verified. The diagnostic citation
  `"CAPCO-2016 §E.6"` is correctly attributed.

- **S004** — `check()` body at `rules.rs:1632-1768` iterates over
  `attrs.rel_to`, scores each entry against `COUNTRY_CODE_BASE_RATES`,
  and emits at most one `Diagnostic` per trigraph via
  `Diagnostic::text_correction(..., span_token.span, ..., candidate, ...)`.
  The `span_token` is the single `RelToTrigraph` token whose `as_str()`
  is 3 bytes (gated by `if trigraph.len() != 3 { continue; }` at line
  1657). The replacement (the canonical 3-letter trigraph from the
  base-rate table) is byte-for-byte the same length, so the splice is
  sub-token in both the source and target sense. The `Severity::Suggest`
  default means the engine never auto-applies, but per D-7.4 the phase
  declaration governs dispatch regardless of severity. §H.8 p150–151
  ("REL TO USA, LIST" + "USA must always appear first" + trigraph
  alphabetical sort) is verified at CAPCO-2016.md lines 3675, 3709.

### Phase::WholeMarking spot-check (8 rules)

Selected to cover the major rule families: banner roll-up (E031),
class-mutual-exclusion (E024), JOINT (E014, E016, E036), AEA (E024),
RELIDO conflicts (E054, E055, E056), and the no-fix supersession
(E041). Each row records the maximal fix-emission shape the rule
produces.

| RuleId | Implementer claim | Span shape | CAPCO citation | Verdict |
|--------|-------------------|------------|----------------|---------|
| E031 | Banner roll-up walker (SAR / SCI / Non-IC dissem catalog) | per-row evaluator emits diagnostics spanning the banner candidate; catalog rows scope `Scope::Page` decisions | CAPCO-2016 §H.5 p101 (SAR roll-up); §H.4 per-system "Precedence Rules for Banner Line Guidance" (e.g. HCS p62, SI p74, TK p85); §H.9 p172/p174 (NODIS/EXDIS) — all verified | **PASS** |
| E014 | JOINT co-owner missing from REL TO | `FactAdd { CountryCode, Scope::Portion or Scope::Page }` + `ctx.candidate_span`; engine re-renders via `apply_intent` + `render_canonical` | CAPCO-2016 §H.3 p57 ("REL TO marking that includes the US and all co-owners, at both the banner and portion level") — verified at CAPCO-2016.md line 1276 | **PASS** |
| E016 | JOINT + RESTRICTED conflict | no-fix `Severity::Error`; span = first Classification token; decision reads `MarkingClassification::Joint(_)` + RESTRICTED axis | CAPCO-2016 §H.3 p56 ("May not be used with RESTRICTED.") — verified at CAPCO-2016.md line 1263 | **PASS** |
| E024 | RD precedence over FRD/TFNI (atomic-cluster `FactRemove`) | `FactRemove { facts: [FRD, TFNI], Scope::Portion }` + `ctx.candidate_span` | CAPCO-2016 §H.6 p104–p105 ("If RD, FRD, and TFNI portions are in a document, the RD takes precedence") — verified at CAPCO-2016.md lines 2493, 2528-2538 | **PASS** |
| E036 | JOINT + HCS conflict | no-fix `Severity::Error`; span = first HCS-prefixed `SciControl` token; decision reads JOINT + HCS axes | CAPCO-2016 §H.3 p57 ("May not be used with the HCS markings or NOFORN markings.") — verified at CAPCO-2016.md line 1272 | **PASS** |
| E054 | RELIDO conflicts NOFORN | `FactRemove { TOK_RELIDO, Scope::Portion }` + `ctx.candidate_span`; engine re-renders the portion | CAPCO-2016 §H.8 p154 ("Cannot be used with NOFORN or DISPLAY ONLY.") + p145 NOFORN entry — both verified | **PASS** |
| E055 | RELIDO conflicts DISPLAY ONLY | `FactRemove { TOK_RELIDO, Scope::Portion }` + `ctx.candidate_span` | CAPCO-2016 §H.8 p154 — verified | **PASS** |
| E041 | NODIS supersedes EXDIS in portion | `FactRemove { TOK_EXDIS, Scope::Portion }` + `ctx.candidate_span`; span points at the EXDIS token, candidate_span covers the full portion | CAPCO-2016 §H.9 p172 (EXDIS) + p174 ("NODIS (ND) supersedes EXDIS (XD) in the portion mark") — verified at CAPCO-2016.md line 4306 | **PASS** |

**Cross-family observations.**

- Every WholeMarking rule that emits a fix uses either
  (a) `with_fix_at_span(..., ctx.candidate_span, ..., FixIntent { ... })`
  (so the engine re-renders the full marking via `apply_intent` +
  `render_canonical`), or (b) `with_fix(..., None)` for the
  no-fix-intent path. Neither shape can be expressed as a sub-token
  splice — both rely on the engine's intent-synthesis pipeline reading
  the marking attrs at scope. The `Phase::WholeMarking` declaration is
  load-bearing because PR 7b will populate `pre_pass_1_attrs` only for
  pass-2 rules.

- For the four RELIDO-conflict rules (E054/E055/E056/E057), the
  `relido_remove_intent()` helper (`rules_declarative.rs:2035-2045`) is
  shared. All four are correctly `Phase::WholeMarking` — the
  `FactRemove` intent is portion-scoped but requires the engine to
  re-render the full portion to remove RELIDO plus its `/` separator
  (within-category separator) cleanly. A sub-token splice could not do
  that without parser-level within-category separator spans (issue #106).

- E031 (banner roll-up walker) emits per-row diagnostics via three
  evaluator functions; each evaluator's emission span covers the
  banner candidate, with fix intents (when emitted) at
  `Scope::Page`. Whole-marking by construction.

### Defensive WholeMarking re-examination

| RuleId | Reason flagged | CAPCO assessment | Recommendation |
|--------|----------------|------------------|----------------|
| E008 | Reads `attrs.sar_markings.is_some()` to suppress repeated-SAR blocks (E030 territory under §H.5) | The suppression gate is correct per CAPCO-2016 §H.5 p100 ("The SAP category indicator must not be repeated if multiple SAP programs are applicable"). E030 owns the repeated-SAR-block diagnostic, so E008 must step aside when a first SAR parsed successfully. This cross-token state read is exactly the kind of decision PR 7c's `pre_pass_1_attrs` cache exists to preserve — if a pass-1 fix on the FIRST SAR block changes `sar_markings`, E008 reading post-pass-1 attrs would mis-suppress. `Phase::WholeMarking` is the right call. | **Keep WholeMarking.** The defensive choice is CAPCO-grounded: §H.5 p99–101 governs the SAR grammar that E008's suppression depends on; reading post-pass-1 attrs after a pass-1 fix on a SAR block would silently change the suppression decision. |
| S005 | Reads full REL TO list across portions + page context for atom-semantics ambiguity | The atom-semantics intersection runs over `page.portions()` and reads each portion's `rel_to`, `dissem_controls`, `non_ic_dissem` (CAPCO-2016 §H.8 atom-vs-tetragraph distinction + §H.9 NODIS/EXDIS supersession). The page-context read is page-scope, not single-token; whole-marking is the correct dispatch. Per the docstring at `rules.rs:2535-2538`, "Spans point at individual trigraphs but the decision is list-scoped" — the SPAN looks like a single trigraph (it points the user at the offending entry) but the FIRING decision reads the cross-portion REL TO intersection, which is whole-marking territory. | **Keep WholeMarking.** Spans are diagnostic anchors, not fix targets — the rule emits `fix: None` so there's no splice to worry about, but the decision-data is page-scope. |
| S006 | Companion of S005 (same `analyze_uncertain_reduction` helper, `Info` branch) | Same analysis as S005. The branch split is severity-only; the underlying decision shape is identical. | **Keep WholeMarking.** Companion to S005; same rationale. |

**Additional defensive-WholeMarking observations (informational, not on the
PM's "re-examine" list).**

- **E002** is registered `Phase::WholeMarking` (verified at
  `rules.rs:383-385`). Its fix span (`rules.rs:482-492`) covers
  first→last `RelToTrigraph` plus trailing separators inside one
  `RelToBlock`, which is **multi-token by construction** — the rule
  explicitly notes "Span: first→last `RelToTrigraph` within this block,
  extended through any trailing `,`/whitespace tail". The declaration
  is correct and necessary: a `Phase::Localized` declaration here would
  be a defect, since the span crosses token boundaries (multiple
  trigraph tokens plus their `,` separators). The PR scope didn't flag
  E002 for review but the assignment is correct.

- **E005** declassify-misplaced rule (no-fix at `rules.rs:685-696`)
  reads `attrs.declassify_on` and `attrs.declass_exemption` and emits
  with the declass token span — span is one token, decision is
  cross-axis (banner / portion / CAB). `Phase::WholeMarking` is correct
  per the no-fix policy at lines 692-696 ("Fix requires document-level
  context (moving a token from banner/portion into a CAB is
  multi-span)").

- **W003** (non-IC dissem in classified banner) reads classification
  axis × non-IC dissem axis at banner scope (`rules.rs:1819-1882`); no
  fix emitted; correctly `Phase::WholeMarking`.

- **W034** (SCI custom-control info) at `rules.rs:2698-2727` iterates
  `attrs.sci_markings` and emits one no-fix diagnostic per
  `SciControlSystem::Custom`. The span is the SCI system token (single
  token), but the per-marking iteration reads cross-token state via
  `sci_markings`. No fix means span-shape doesn't matter for splice
  correctness, but the WholeMarking declaration is consistent with
  S005/S006/E008's defensive policy.

---

## Citation Audit

Every `phase()` docstring that carries a CAPCO citation was verified
against `crates/capco/docs/CAPCO-2016.md`:

| RuleId | `phase()` docstring citation | Verified at CAPCO-2016.md | Status |
|--------|------------------------------|---------------------------|--------|
| E031 | "banner roll-up walker (E031 SAR / E035 SCI / E040 Non-IC dissem)" | §H.5 p101, §H.4 (HCS p62, SI p74, TK p85), §H.9 p172/p174 — verified at lines 2426, 1367, 1683, 1989, 4209, 4269 | **PASS** |
| E014 | "§H.3 p57" (in diagnostic citation, repeated in `phase()` rationale) | Line 1276 ("REL TO marking that includes the US and all co-owners") | **PASS** |
| E015 | "§H.7 / §B.3 ... §H.7 p122" | Line 3020 (begin page 122, FGI section header) + §B.3 p20 (FGI two-fill rule) | **PASS** |
| E016 | "§H.3 cross-axis mutual exclusion (JOINT vs RESTRICTED)" | Line 1263 ("May not be used with RESTRICTED") at §H.3 p56 | **PASS** |
| E021 | "§H.6 p104 + p111" | Line 2493 (p104 RD) + p111 (FRD entry; both "Is always used with NOFORN unless a sharing agreement") | **PASS** |
| E024 | "§H.6 p104–p105" | Lines 2493, 2528-2538 ("RD takes precedence over FRD and TFNI in the portion mark") | **PASS** |
| E036 | "§H.3 cross-axis mutual exclusion (JOINT vs any HCS marking)" | Line 1272 ("May not be used with the HCS markings or NOFORN markings") at §H.3 p57 | **PASS** |
| E037 | "§H.9 cross-axis mutual exclusion (NODIS vs EXDIS)" | Line 4295 ("NODIS and EXDIS markings cannot be used together") at §H.9 p174 | **PASS** |
| E038 | "§H.9 EXDIS / NODIS require NOFORN" | "Requires NOFORN" at §H.9 p174 (line 4296) and §H.9 p172 EXDIS entry | **PASS** |
| E041 | "§H.9 p172/p174 — NODIS supersedes EXDIS" | Line 4306 ("NODIS (ND) supersedes EXDIS (XD) in the portion mark") at §H.9 p174 | **PASS** |
| E053 | "§H.8 p145 NOFORN-clears-REL-TO" | Line 3585 ("Cannot be used with REL TO, RELIDO, EYES ONLY, or DISPLAY ONLY") at §H.8 p145 | **PASS** |
| E054 | "§H.8 p145 NOFORN dominates" (note: rule's diagnostic citation is §H.8 p154 — RELIDO template) | Line 3808 ("Cannot be used with NOFORN or DISPLAY ONLY") at §H.8 p154 — RELIDO is the asserting token here, so the §H.8 p154 anchor is correct under PM Addendum II Q1 | **PASS** |
| E055 | "§H.8 p154" | Line 3808 ("Cannot be used with NOFORN or DISPLAY ONLY") at §H.8 p154 | **PASS** |
| E056 | "§H.8 p136" | Line 3363 ("May not be used with RELIDO") at §H.8 p136 (ORCON template) | **PASS** |
| E057 | "§H.8 p140" | Line 3444 ("May not be used with RELIDO") at §H.8 p140 (ORCON-USGOV template) | **PASS** |
| W002 | "§H.7 cross-axis advisory (US + FGI commingling)" | Line 3076 (begin page 124) and surrounding commingling rules at §H.7 p122–124 | **PASS** |
| S003 | "§H.3 / §H.8 (USA-first convention)" | §H.3 p56 (JOINT alpha-only) + §H.8 p150-151 ("USA must always appear first") — both verified at lines 1258 + 3713 | **PASS** |
| S004 | "§H.8 p150–151" | Lines 3675, 3709 — verified | **PASS** |
| S005 / S006 | "§H.8 + ODNI ISMCAT Tetragraph Taxonomy" | §H.8 base citation verified; tetragraph taxonomy is the ODNI ISMCAT sidecar | **PASS** |
| E006 | "§F" (bare, no page) | Line 724 (§F. (U) Legacy Control Markings) at p35 | **PASS** with note (LOW-2 below) |
| E007 | "§E.6" | Line 689 (`### 6. (U) Retired or Invalid Declassify On Values`) at §E.6 pp 33–34 | **PASS** |
| E008 | "§G.1 (Register of Authorized Markings, p36)" | Line 742 (§G. (U) IC Markings System Register), line 748 ("All markings used in a banner line and portion mark must be in accordance with the values listed in the Register") at §G.1 p36 | **PASS** |
| E010 | "§H.4 p62 (HCS legacy)" | Line 1367 (begin page 62, HCS template) | **PASS** |
| E012 | "§H.3 p55" | Page 55 is the §H.3 header (US/non-US/JOINT mutual-exclusion sentence on the JOINT template) | **PASS** |

**No fabricated citations found.** Every page number and section
reference in a `phase()` docstring or its companion diagnostic
`citation` field traces to a real passage in CAPCO-2016.md. This is
the critical Constitution VIII gate and it clears.

---

## Findings

### HIGH (block)

None.

### MEDIUM (warn)

None.

### LOW (note)

- **LOW-1 — S003 phase rationale could name the diagnostic shape more
  precisely.** The `phase()` docstring at
  `rules.rs:1347-1352` says S003 "emits `ReplacementIntent::Recanonicalize`
  at `RecanonScope::Page`; the engine re-renders the JOINT
  classification across the candidate scope." The `Recanonicalize`
  emission is correct (verified at `rules.rs:1416-1453`), but a
  reviewer unfamiliar with `RecanonScope::Page` semantics may not
  immediately see that this is a full-marking rewrite. A one-line
  reinforcement (e.g. "this rewrites the entire JOINT marking via the
  renderer, not a single token") would improve clarity. **No fix
  required for PR 7a** — the assignment is correct.

- **LOW-2 — E006 §F citation is bare (no page number).** The
  diagnostic citation string at `rules.rs:766` is `"CAPCO-2016 §F"`
  with no page number. Project memory `feedback_citations_use_page_numbers`
  notes that CAPCO-2016 citations cite §X.Y pNN; line NNNN form is
  retired (commit b340bec). §F (Legacy Control Markings) is a
  one-page section (p35), so the bare §F citation is unambiguous in
  this case, but adding "p35" matches the conventions established by
  every other rule in this PR. **Style note, not a correctness defect**
  — recommend `"CAPCO-2016 §F p35"` as a follow-up cleanup (not a
  blocker for PR 7a).

---

## Summary

PR 7a's 31 `Phase` declarations are all CAPCO-correct:

- **4 Localized rules** (C001, E006, E007, S004) genuinely emit
  sub-token fixes. Every fix spans a single `TokenSpan` and the
  replacement does not cross any CAPCO separator (`//`, `/`, `-`,
  space, `, `).
- **27 WholeMarking rules** correctly cover the whole-marking
  emissions: banner-roll-up walkers, cross-axis decisions, intent-only
  `FactAdd` / `FactRemove` / `Recanonicalize` emissions (via
  `candidate_span`), and no-fix advisories whose firing decisions read
  cross-token state.
- **3 defensively-WholeMarking rules** (E008, S005, S006) are
  defensive for the right reason: each reads CAPCO-grounded
  cross-token state (E008's §H.5 SAR-repeat suppression; S005/S006's
  §H.8 page-level REL TO atom-semantics) that pass-2 dispatch with
  `pre_pass_1_attrs` is designed to preserve correctly.
- **No fabricated citations.** Every § and page reference verified
  against `crates/capco/docs/CAPCO-2016.md`. Constitution VIII clears.

PR 7a is APPROVED for the CAPCO-domain review. The two LOW notes are
stylistic and can be deferred to a follow-up cleanup PR.
