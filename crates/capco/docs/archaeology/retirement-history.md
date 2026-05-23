<!--
SPDX-FileCopyrightText: 2026 Knitli Inc.

SPDX-License-Identifier: LicenseRef-MarqueLicense-1.0
-->

# CAPCO Rule Retirement History

Per-rule retirement provenance, organized by retirement PR. Extracted
verbatim from the top-of-file `//!` block in `crates/capco/src/rules.rs`
by issue #561 (rules.rs split). The entries trace which rule was
retired in which PR, what authority backed the retirement, and what
mechanism replaced the rule's concern (renderer, catalog, bridge, or
no replacement).

> **Rule-ID conventions.** Legacy IDs use the pre-T044 flat-string
> form (`E060`, `S004`, etc.). The post-T044 (2026-05-22) 2-tuple
> wire strings are the current canonical form; see
> [`docs/refactor-006/legacy-rule-id-map.md`](../../../../docs/refactor-006/legacy-rule-id-map.md)
> for the translation table. Where this file refers to a rule by its
> legacy ID, that is the form the original retirement comment used.

> **Authority verification.** Every `§X.Y pNN` citation here was
> verified once when the original retirement comment was authored
> (per Constitution Principle VIII). Issue #561 moves the comments
> byte-identical; it does not re-author. Future edits trigger
> propagation re-verification per Principle VIII §"Propagation
> requires re-verification."

## PR 3c.B Commit 6 — form-bucket migration into the renderer

The following 13 hand-written rules + the `E060` walker were retired
because their concerns are absorbed by the renderer
(`MarkingScheme::render_canonical`) by construction. After this commit
`lint` no longer surfaces these divergences; `fix` (renderer) still
produces canonical output.

| Legacy ID | Rule | Authority |
|-----------|------|-----------|
| E001 | `PortionMarkInBannerRule` | §H.8 portion-in-banner |
| E003 | `MisorderedBlocksRule` | §A.6 block order |
| E004 | `SeparatorCountRule` | §A.6 / §D.1 separators |
| E009 | `PortionAbbreviationRule` | §H.1 / §H.8 / §H.9 portion forms |
| S001 | `PreferBannerAbbreviationRule` | §A.6 banner-abbrev preference |
| S002 | `BannerConsistentFormRule` | §A.6 banner-form consistency |
| E011 | `MissingNonUsPrefix` | §A.6 / §H.3 non-US `//` prefix |
| E013 | `DelimiterMismatchRule` | §H.3 / §H.8 list delimiters |
| E026 | `SarPortionFormRule` | §H.5 SAR portion form |
| E029 | `SarCompartmentOrderRule` | §H.5 SAR compartment order |
| E030 | `SarIndicatorRepeatRule` | §H.5 SAR indicator repeat |
| E032 | `SciSystemOrderRule` | §H.4 SCI sort order |
| E052 | `RelToNoDuplicatesRule` | §H.8 list dedup |
| E060 | `DeclarativeNonCanonicalInputRule` (walker — REL TO/JOINT/SIGMA/SAR/SCI ordering) | §H.3 p56 + §H.4 p61 + §H.5 p99 + §H.6 p108 + §H.8 p150-151 |

See `docs/plans/2026-05-10-pr3c-consolidated-plan.md` lines 788–862 for
the architectural commitment. `E002` and `S003` stay (separate dual-pop
migration); all other unrelated rules are unaffected.

## PR #470 close-out — W002 retired

`W002 DeclarativeCominglingWarningRule` retired. CAPCO §H.7 p123
authorizes the canonical `(US-CLASS//FGI [LIST]//NF)` shape as the
commingled-with-US-classification form for acknowledged foreign
sources; the §H.7 p124 segregation rule applies only to non-ICD-206
documents, a doc-level property the engine cannot determine from a
single portion. The predicate fired indiscriminately on the canonical
shape and produced noise rather than signal.

## T035a — declarative wrappers introduced

Declarative wrappers landed for `E010` / `E012` / `E014` / `E015` /
`E016` / `E021` / `E022` / `E024` / `E025`. Catalog in `crate::scheme`
owns the predicate; wrappers own span / message / fix construction.

## T035b — over-restrictive JOINT rules retired

`E017` / `E018` / `E019` retired entirely (over-restrictive per
CAPCO §H.3 lines 4140-4146 — the current edition cites
§H.3 p57 as the authority for the surviving JOINT rules). Replacement:
`E036 joint-hcs` (the only specific JOINT exclusion §H.3 p57 actually
names).

## T035c-14 — W001 retired

`W001 DeprecatedMarkingWarningRule` retired. CAPCO-2016 §F "Legacy
Control Markings" (p35) treats legacy markings as unauthorized — an
error category owned by `E006` / `E008` — not "deprecated but still
legal." §I "Banner Line Syntax History" (p192–193 Table 8) is
syntax-history, not token-deprecation guidance, and is non-normative
for citations. No CAPCO-2016 passage sanctions a warning-severity
"legal but preferred-newer" vocabulary tier, so the rule stub had no
authoritative ground to populate. If org-policy deprecations
(FOUO-style transitional warnings) later need a home, that is a
separate rule with org-config authority, not CAPCO §F.

## PR #578 — declarative wrappers retired into the engine bridge

The following 13 declarative wrappers retired. Their predicates,
spans, and severities now fire through the engine's constraint-catalog
bridge directly — see `CapcoScheme::constraints()` and the
`token_span` / `severity` implementation on `MarkingScheme` for the
consolidated dispatch path.

| Legacy ID | Rule |
|-----------|------|
| E010 | `DeclarativeBareHcsRule` |
| E012 | `DeclarativeDualClassificationRule` |
| E014 | `DeclarativeJointRelToRule` |
| E015 | `DeclarativeNonUsMissingDissemRule` |
| E016 | `DeclarativeJointRestrictedRule` |
| E036 | `DeclarativeJointHcsRule` |
| E021 | `DeclarativeAeaNofornRule` |
| E024 | `DeclarativeRdPrecedenceRule` |
| E053 | `DeclarativeNofornRelToConflictRule` |
| E054 | `DeclarativeRelidoNofornConflictRule` |
| E055 | `DeclarativeRelidoDisplayOnlyConflictRule` |
| E056 | `DeclarativeOrconRelidoConflictRule` |
| E057 | `DeclarativeOrconUsgovRelidoConflictRule` |

S004 stays a registered walker rule — unlike the other 15 retired
wrappers (which emit static `FixIntent` values the bridge can
synthesize from `(name, attrs)` alone), S004's replacement is a
corpus-derived candidate trigraph computed during evaluation. The
bridge's `fix_intent_by_name` shape cannot return that candidate
without re-running the evaluator, so the walker keeps ownership of
both the predicate and the `text_correction` emission.

## PR 3c.B Commit 7.3 — class-floor catalog migration

`DeclarativeClassFloorRule` (rule ID `E058`) retired. The 27
class-floor catalog rows now fire through the engine's
constraint-catalog bridge directly — `class_floor_emit` populates
`ConstraintViolation::{span, severity}`, and the bridge folds
`E058/<purpose>` and `class-floor/<marking>` row names to
`Diagnostic.rule = "E058"` so audit-stream consumers and
`[rules] E058 = "off"` config overrides keep working. The 23 family
rows (`class-floor/<marking>`) plus the 4 walker-prefixed rows
(`E058/CNWDI`, `E058/SAR`, `E058/DOD-UCNI`, `E058/DOE-UCNI`) remain
declared as `Constraint::Custom` entries in
`CapcoScheme::build_constraints()`. See
`specs/006-engine-rule-refactor/decisions/06-commit-7-subdivision.md`
for the architectural rationale.

## PR 3c.B Commit 7.4 — SCI per-system catalog migration

PR 3b.E (T026e) → PR 3c.B Commit 7.4: retired the 10 hand-written
per-SCI-system rules `E042`–`E051` (PR 3b.E walker
`DeclarativeSciPerSystemRule`, ID `E059`) into the engine's
constraint-catalog bridge. The catalog rows still fire — they emit via
`CapcoScheme::bridge_sci_per_system_diagnostics` with
`Diagnostic.rule = "E059"` and full `FixProposal` payloads attached
(companion-insertion at the dissem-block anchor, ORCON-USGOV → ORCON
replacement). The 5 catalog rows
(`sci-per-system/{HCS-O, HCS-P-NOFORN, HCS-P-sub, SI-G,
TK-compartment-NOFORN}-*`) remain declared as `Constraint::Custom`
entries in `CapcoScheme::build_constraints()` for documentation /
dispatch parity with class-floor; the bridge takes the inherent-method
shortcut. See
`specs/006-engine-rule-refactor/decisions/06-commit-7-subdivision.md`.

## PR #488 — S005/S006 Suggest/Info collapse

Issue #206 / PR #488: REL TO membership-uncertain reduction. PR #488
collapsed the original `S005`/`S006` Suggest/Info split into one
Suggest-severity rule under `Phase::PageFinalization`. The pre-#488
split was an engine-workaround (per-rule severity override was the
only way to surface two severities for one trigger); CAPCO-2016 §H.8 +
§D.2 Table 3 rule 21 don't distinguish "active validation" from
"consistent case." See the rule's doc comment for the retirement
rationale and the admonition-channel future home for the
per-emission-severity signal.

## PR 3b.F (T026f) — ordering walker collapse

`E020` (REL TO USA-first + alpha) + `E023` (SIGMA ordering) + `E028`
(SAR program ordering) + `E033` (SCI compartment ordering) rolled into
the `DeclarativeNonCanonicalInputRule` walker (rule ID `E060`). `E060`
itself subsequently retired in PR 3c.B Commit 6 (see above) into the
renderer.

## PR 3b Sub-move A (T026a) — banner-roll-up walker

`E031 SarBannerRollupRule` + `E035 SciBannerRollupRule` +
`E040 NodisExdisBannerRollupRule` retired into a single
`BannerMatchesProjectedRule` walker. Emitted diagnostics carry per-row
IDs (`E031` / `E035` / `E040`) for audit-stream continuity. The
walker still ships; the three retired struct IDs do not.

Per-row authorities:

- `E031` SAR banner rollup — §H.5 p101
- `E035` SCI banner rollup — §H.4 per-system citations (§H.4 p61 anchor)
- `E040` NODIS / EXDIS banner rollup — §H.9 p172 + p174

## PR 9a T135a (issue #307 Group D) — deprecated SCI long-form canonicalization

`DeprecatedSciLongFormRule` walker added for HUMINT → HCS,
COMINT / SPECIAL INTELLIGENCE → SI, ECI `<COMP>` → SI-`<COMP>`,
EL / ENDSEAL `<COMP>` → SI-`<COMP>`, KDK / KLONDIKE-`<COMP>` →
TK-`<COMP>`. Catalog ordered longer-prefix-first inside
`rules_declarative.rs`. Authority: CAPCO-2016 §H.4 pp 61, 62, 74, 76,
78, 85.

## Rule ID assignment table (current live set)

The full pre-T044 rule ID assignment table, including the retired
entries above and the still-registered rules. This is the historical
"why is this ID gone?" surface; for the current registered set see
`crates/capco/README.md` and the registration pin at
`crates/capco/tests/post_3b_registration_pin.rs`.

- E001 = retired in PR 3c.B Commit 6 (form-bucket migration) —
  portion-mark-in-banner absorbed by
  `MarkingScheme::render_canonical`.
- E002 = REL TO missing USA trigraph (T031); dual-pop migration
  tracked separately, retained for now.
- E003 = retired in PR 3c.B Commit 6 — block ordering absorbed by the
  renderer.
- E004 = retired in PR 3c.B Commit 6 — separator normalization
  absorbed by the renderer.
- E005 = declassification misplaced (banner or portion; belongs in
  CAB) (T034).
- E006 = deprecated dissem control (T035).
- E007 = X-shorthand declass date (T036).
- E008 = unrecognized token (T037).
- E009 = retired in PR 3c.B Commit 6 — banner→portion form
  normalization absorbed by the renderer.
- E010 = bare HCS without compartment suffix.
- E011 = retired in PR 3c.B Commit 6 — `//`-prefix normalization on
  non-US classification absorbed by the renderer.
- E012 = dual classification (US + foreign conflict).
- E013 = retired in PR 3c.B Commit 6 — list-delimiter normalization
  absorbed by the renderer.
- E014 = JOINT participants missing from REL TO.
- E015 = non-US classification without dissem control.
- W001 = retired in T035c-14 (CAPCO-2016 §F treats legacy markings as
  unauthorized, not "deprecated but legal"; no authoritative bucket
  for a warning-severity rule).
- W002 = retired in PR closing #470 — CAPCO §H.7 p123 authorizes the
  canonical `(US-CLASS//FGI [LIST]//NF)` shape as the
  commingled-with-US-classification form for acknowledged foreign
  sources; the §H.7 p124 segregation rule applies only to non-ICD-206
  documents, a doc-level property the engine cannot determine from a
  single portion. The predicate fired indiscriminately on the canonical
  shape and produced noise rather than signal.
- E016 = RESTRICTED not allowed with JOINT.
- E017 = retired in T035b (over-restrictive per CAPCO §H.3 p57).
- E018 = retired in T035b (over-restrictive per CAPCO §H.3 p57).
- E019 = retired in T035b (over-restrictive per CAPCO §H.3 p57).
- E020 = retired in PR 3b.F (T026f) — country code list ordering
  rolled into E060; E060 retired in PR 3c.B Commit 6 into the
  renderer.
- E021 = RD/FRD requires NOFORN (configurable to warn).
- E022 = CNWDI only with TS or S RD.
- E023 = retired in PR 3b.F (T026f) — SIGMA ordering rolled into
  E060; E060 retired in PR 3c.B Commit 6 into the renderer.
- E024 = RD precedence over FRD/TFNI.
- E025 = UCNI only with UNCLASSIFIED.
- E026 = retired in PR 3c.B Commit 6 — SAR portion form absorbed by
  the renderer.
- E028 = retired in PR 3b.F (T026f) — SAR program ordering rolled
  into E060; E060 retired in PR 3c.B Commit 6 into the renderer.
- E029 = retired in PR 3c.B Commit 6 — SAR compartment ordering
  absorbed by the renderer.
- E030 = retired in PR 3c.B Commit 6 — SAR indicator repetition
  absorbed by the renderer.
- W003 = non-IC dissem in classified banner.
- E032 = retired in PR 3c.B Commit 6 — SCI sort order absorbed by the
  renderer.
- E033 = retired in PR 3b.F (T026f) — SCI compartment ordering rolled
  into E060; E060 retired in PR 3c.B Commit 6.
- W034 = SCI custom (unpublished) control-system audit visibility.
- E035 = SCI banner rollup (missing compartments from portions).
- E036 = JOINT may not be used with HCS markings (T035b, replaces
  E017-E019).
- E037 = NODIS and EXDIS must not coexist (T035c-21 PR-A).
- E038 = NODIS / EXDIS require NOFORN (T035c-21 PR-A).
- E039 = REL TO not allowed in banner with NODIS/EXDIS portion
  (T035c-21 PR-B).
- E040 = banner must roll up NODIS (or EXDIS if no NODIS) (T035c-21
  PR-B).
- E041 = NODIS supersedes EXDIS in portion (T035c-21 PR-B).
- S001 = retired in PR 3c.B Commit 6 — banner-abbrev preference
  absorbed by the renderer.
- S002 = retired in PR 3c.B Commit 6 — banner-form consistency
  absorbed by the renderer.
- S003 = JOINT country list should lead with USA (style, follow-up
  from #97); dual-pop migration tracked separately.
- S004 = REL TO trigraph suggest-don't-fix (issue #235 / #186 PR-3).
- E052 = retired in PR 3c.B Commit 6 — REL TO duplicates absorbed by
  the renderer.
- E053 = NOFORN conflicts with REL TO (§H.8 p145, declarative wrapper).
- E054 = RELIDO conflicts with NOFORN — subtractive fix removes RELIDO
  (§H.8 p154, declarative wrapper — PR 3b.C).
- E055 = RELIDO conflicts with DISPLAY ONLY — subtractive fix removes
  RELIDO (§H.8 p154, declarative wrapper — PR 3b.C).
- E056 = ORCON conflicts with RELIDO — subtractive fix removes RELIDO
  (§H.8 p136, declarative wrapper — PR 3b.C).
- E057 = ORCON-USGOV conflicts with RELIDO — subtractive fix removes
  RELIDO (§H.8 p140, declarative wrapper — PR 3b.C).
- E060 = retired in PR 3c.B Commit 6 — non-canonical-input walker (5
  ordering rows: REL TO USA-first §H.8 p150-151, JOINT alpha §H.3 p56,
  AEA SIGMA numeric sort §H.6 p108, SAR program ascending alpha §H.5
  p99, SCI compartment + sub-compartment numeric-then-alpha §H.4 p61)
  absorbed by `MarkingScheme::render_canonical`.
- S005 = REL TO membership-uncertain reduction (issue #206; PR #488
  collapsed the S005/S006 Suggest/Info split — S006 retired, S005
  migrated to `Phase::PageFinalization`).
- S007 = bare NATO classification in a US-classified document should
  carry `REL TO USA, NATO` (PR 9c.2 / FR-048, §H.7 p127).
- S008 = RELIDO implied by closure (#559 close-out C1, §H.8 p154 +
  §D.2 Table 3 rule 17).
- E071 = FGI with explicit trigraph when concealment intended (issue
  #261, §H.7 p124).
- S009 = prefer-tetragraph-collapse — suggest replacing explicit
  member trigraph lists with a compact tetragraph when all members are
  present (issue #250, §H.8 p150). Default: Off.
- S010 = collapse-uniform-rel-portions — suggest bare REL when all
  portions carry the same REL TO list as the banner (issue #251,
  §H.8 p150). Default: Off.
- E072 = bare-rel-portion-divergence — warns when bare-REL and
  explicit-REL-TO portions coexist with an inconsistent list (issue
  #251, §H.8 p150-151). Default: Warn.
- C001 = corrections-map typo (T058, Phase 5).
